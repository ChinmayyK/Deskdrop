# Handoff Report — Deskdrop Android Architecture & Service Exploration

**Explorer**: Explorer 2 (Architecture & Service Explorer)  
**Target Project**: Deskdrop Android Application (`/Users/chinmayk/Projects/Deskdrop`)  
**Working Directory**: `/Users/chinmayk/Projects/Deskdrop/.agents/explorer_survey_2`  
**Date**: 2026-08-07  

---

## 1. Observation

Direct code observations from inspecting `/Users/chinmayk/Projects/Deskdrop`:

### A. Android Architecture & Components
1. **Application Entry Point**:
   - `platforms/android/app/src/main/java/com/deskdrop/DeskdropApp.kt` (lines 9-52): Subclasses `Application`, implements `Application.ActivityLifecycleCallbacks`. Tracks global foreground state (`isAppInForeground`), initializes AppCompat night mode and Material 3 Dynamic Colors.
   - `platforms/android/app/src/main/AndroidManifest.xml` (lines 59-67): Defines `android:name=".DeskdropApp"` and registers main foreground service `DeskdropService`.

2. **Foreground Service Implementation**:
   - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt` (lines 68-3507): Subclasses `android.app.Service`.
   - `AndroidManifest.xml` (lines 130-135): Declares `DeskdropService` with `android:foregroundServiceType="connectedDevice"` and `android:stopWithTask="false"`.
   - Permissions declared in `AndroidManifest.xml`:
     - Line 17: `<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />`
     - Line 18: `<uses-permission android:name="android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE" />`
     - Line 21: `<uses-permission android:name="android.permission.WAKE_LOCK" />`
     - Line 30: `<uses-permission android:name="android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS" />`

3. **Threading Model**:
   - **Kotlin / Java Layer**:
     - `backgroundExecutor = Executors.newCachedThreadPool()` (`DeskdropService.kt:69`): Used for async operations (e.g. `executeInBackgroundWithWakeLock`, file staging in `stageSharedUri`).
     - `serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)` (`DeskdropService.kt:321`): Manages coroutine scopes for binary clipboard writes and file saves (`saveFileToPublicDownloads`).
     - `eventDrainThread` (`DeskdropService.kt:957-996`): A dedicated Java `Thread` executing a `while(isRunning)` loop that polls native events via JNI (`DeskdropJni.pollEvent(engineHandle)`) and posts batched events to the Main Thread via `handler.post`.
     - `handler = Handler(Looper.getMainLooper())` (`DeskdropService.kt:160`): Posts UI and service state updates to the Android Main/UI thread.
     - `engineLock = ReentrantReadWriteLock()` (`DeskdropService.kt:324`): Protects `engineHandle` during `pollEvent` calls vs `onDestroy()` shutdown.
   - **Rust Native Engine Layer**:
     - `RT: OnceLock<Runtime>` (`deskdrop-core/src/jni_android.rs:23-26`): Global Tokio multi-threaded async runtime.
     - `spawn_listener_supervisor`, `spawn_discovery_supervisor`, `spawn_auto_reconnector`, `spawn_udp_beacon`, `spawn_udp_listener` (`deskdrop-core/src/engine/mod.rs:780-790`): Background Tokio tasks handling networking, discovery, and connection keepalives.

4. **Networking & IPC Mechanisms**:
   - **JNI Bridge**: `platforms/android/app/src/main/java/com/deskdrop/DeskdropJni.kt` maps Kotlin static external calls to `libdeskdrop_core.so` Rust functions exported in `deskdrop-core/src/jni_android.rs`.
   - **Discovery**: Dual-discovery strategy combining Android NSD (`NsdManager` browsing for `_deskdrop._tcp.` on port 47823, lines 2703-2900) and UDP broadcast beacons (`DESKDROP_BEACON:<uuid>:<port>:<version>` on port 47824, lines 792-854 in `engine/mod.rs`).
   - **TCP Protocol & Framing**:
     - Custom length-prefixed protocol (`[u32 LE length][payload]`) with a 40 MB max frame limit (`deskdrop-core/src/network.rs:52`).
     - Handshake via `handshake_initiator` / `handshake_responder` (lines 381-614 in `network.rs`), establishing noise/ECDH session key and identity verification.
     - TCP Socket tuning (`network.rs:99-111`): `set_nodelay(true)`, socket buffer sizes set to 8MB–16MB (`SOCKET_BUFFER_MIN` / `SOCKET_BUFFER_PREFERRED`), and TCP Keepalive enabled (`KEEPALIVE_IDLE = 10s`, `KEEPALIVE_INTERVAL = 3s`, `KEEPALIVE_RETRIES = 3`).

5. **Background Connection Maintenance (60-Second Criterion R2/AC2)**:
   - **MulticastLock**: `acquireMulticastLock()` (`DeskdropService.kt:887-897`) holds `WifiManager.MulticastLock` ("Deskdrop::NsdMulticast") continuously to prevent hardware filtering of mDNS packets by OEM Wi-Fi drivers (Samsung, Xiaomi, etc.).
   - **WifiLock**: `acquireWifiLock()` (`DeskdropService.kt:837-855`) holds `WifiManager.WifiLock` (`WIFI_MODE_FULL_HIGH_PERF`) so Wi-Fi does not disassociate during Doze mode.
   - **WakeLock**: `acquireWakeLock()` (`DeskdropService.kt:863-876`) acquires a 30-second `PARTIAL_WAKE_LOCK` during event processing and remote file browsing.
   - **Tokio Heartbeat & Sleep State**:
     - Tokio heartbeat tick (`engine/mod.rs:5816-5862`) every 5 seconds (`heartbeat_interval`).
     - Timeout: 12 seconds (`heartbeat_timeout`) when awake; relaxed to 24 hours when `isAsleep` is true (via `notifySleepState(h, isAsleep)`).
     - Grace period (`time_since_wake`): `local_last_wake` timestamp updated when tick delta > 20s, preventing false timeouts when waking from deep CPU sleep.
   - **Network Restoration**: `registerNetworkCallback()` (`DeskdropService.kt:3041-3107`) listens for default network availability, triggering `notifyNetworkRestored(h)` and `restartDiscoveryNow()` with 1.5s debouncing.
   - **Exponential Backoff Retry**: `scheduleNsdRetry()` (`DeskdropService.kt:3127-3141`) reschedules discovery when all peers disconnect (5s -> 10s -> 20s -> 40s -> 60s).

6. **Identified Potential Points of Failure & Crash Vectors**:
   - **Crash Vector 1: Native SIGSEGV Use-After-Free via Unprotected JNI Calls**:
     - *Code Location*: `DeskdropService.kt` (lines 476-765, 1000-1572, 2407-2440).
     - In `onDestroy()` (lines 787-795), `engineLock.writeLock()` is acquired, `DeskdropJni.stop(engineHandle)` drops `AndroidHandle` (`jni_android.rs:144`), and `engineHandle` is set to `0L`.
     - However, outside of `pollEvent`, JNI functions in `DeskdropService.kt` (such as `DeskdropJni.pushText`, `DeskdropJni.trustPeer`, `DeskdropJni.sendFilePath`, `DeskdropJni.sendRemoteFilesResponse`, etc.) read `engineHandle` without acquiring `engineLock.readLock()`.
     - In `jni_android.rs`, functions perform `let h = unsafe { &*(handle as *const AndroidHandle) };` without verifying if `handle` points to valid heap memory. If `stop()` executes while an async task or main thread runnable calls a JNI method with a cached `engineHandle`, Rust dereferences a dangling pointer, causing a fatal native `SIGSEGV` crash.
   - **Crash Vector 2: Uncaught MediaStore / Permission Exceptions**:
     - *Code Location*: `RemoteFileManager.kt` (lines 64-130) and `DeskdropService.kt` (lines 2630-2678).
     - `RemoteFileManager.queryFiles()` queries `MediaStore.Files.getContentUri("external")`. On specific Android versions (API 30+ / API 34+), calling ContentResolver queries without granted permissions or when MediaStore is unavailable can throw `SecurityException` or `IllegalArgumentException`.
     - `startStorageMonitor()` calls `getSystemService(Context.STORAGE_STATS_SERVICE) as StorageStatsManager`. On devices without storage stats support or below Android 8.0, casting null to `StorageStatsManager` or calling `getTotalBytes` can throw `NullPointerException` / `ClassCastException` / `SecurityException`.
   - **Crash Vector 3: ForegroundServiceStartNotAllowedException on Android 12+/14+**:
     - *Code Location*: `DeskdropService.kt` (lines 423-430) & `MainActivity.kt` (lines 600-611).
     - `MainActivity.kt` and `BootReceiver.kt` call `ContextCompat.startForegroundService(...)`. If invoked while the app is in the background (and exempt criteria are not met on Android 12+ / 14+), Android throws `ForegroundServiceStartNotAllowedException`. Catching it in `onStartCommand` without calling `startForeground()` within 10 seconds causes Android OS to throw a fatal `ForegroundServiceDidNotStartInTimeException`.

---

## 2. Logic Chain

1. **Service Architecture & Stability**:
   - `DeskdropService` is designed to run continuously in the foreground (`connectedDevice` type).
   - To maintain connection for at least 60 seconds (AC2), `DeskdropService` correctly acquires `MulticastLock` and `WifiLock`, and delegates socket keepalives and heartbeats to Tokio and `ConnectivityManager`.
2. **Analysis of Crash Scenarios during Stress / Monkey Testing**:
   - When running stress tests (`adb shell monkey`), rapid Activity creation/destruction, background/foreground toggling, screen locks, and network state changes occur.
   - During rapid service restarts or UI actions while service is stopping:
     - `onDestroy()` calls `DeskdropJni.stop(engineHandle)`, dropping the native `AndroidHandle`.
     - Concurrent handlers executing `handler.post` or `backgroundExecutor` calls invoke JNI methods using the non-zero `engineHandle` value they cached before `onDestroy()` set it to `0L`.
     - In Rust `jni_android.rs`, functions like `Java_com_deskdrop_DeskdropJni_pushText` immediately dereference `handle as *const AndroidHandle`.
     - *Step-by-step*: `DeskdropJni.stop` frees memory -> concurrent thread calls JNI method -> Rust dereferences dangling pointer -> **SIGSEGV Native Crash**.
   - During storage/telemetry queries:
     - If permission is denied or device lacks `StorageStatsManager`, unhandled exceptions in background threads crash the app process.

---

## 3. Caveats

- **Device-Specific OEM Behavior**: Behavior of `NsdManager` and `WifiLock` under Doze mode can vary slightly across OEM Android builds (e.g. MIUI, OneUI, ColorOS). The code uses `MulticastLock` and `WifiLock`, which mitigates most OEM restrictions, but exact behavior depends on hardware driver support.
- **Prebuilt Native Library**: The Rust backend is compiled into `libdeskdrop_core.so` located under `platforms/android/app/src/main/jniLibs/`. Code logic was verified against `deskdrop-core/src/jni_android.rs` and `deskdrop-core/src/engine/mod.rs`.

---

## 4. Conclusion

The Deskdrop Android application possesses a robust dual-discovery and foreground-service architecture capable of maintaining stable background connections for >60 seconds. However, it suffers from specific structural crash vulnerabilities under stress:

1. **JNI Thread Safety Violation**: Lack of `engineLock.readLock()` around all JNI calls in Kotlin, and lack of null/validity checking in Rust `jni_android.rs`, leading to `SIGSEGV` native crashes when JNI calls race against `DeskdropJni.stop()`.
2. **Uncaught Telephony & Storage Exceptions**: Potential `NullPointerException` or `SecurityException` in `StorageStatsManager`, `RemoteFileManager`, and call receivers under permission denial.
3. **Foreground Service Lifecycle Race**: Risk of `ForegroundServiceDidNotStartInTimeException` if `startForegroundCompat` fails early on Android 12+/14+.

---

## 5. Verification Method

To verify these observations and conclusions independently:

1. **Build Verification**:
   - Execute Gradle build for Android app:
     ```bash
     cd /Users/chinmayk/Projects/Deskdrop/platforms/android
     ./gradlew assembleDebug
     ```
2. **Stress / Monkey Test (Crash Reproduction)**:
   - Install APK to connected emulator/device:
     ```bash
     adb install -r app/build/outputs/apk/debug/app-debug.apk
     ```
   - Run 5,000 Monkey events to stress test Activity/Service lifecycles:
     ```bash
     adb shell monkey -p com.deskdrop -v 5000
     ```
   - Inspect `logcat` for `FATAL EXCEPTION` or native `SIGSEGV`:
     ```bash
     adb logcat *:E | grep -E "Deskdrop|FATAL|SIGSEGV"
     ```
3. **60-Second Background Connection Stability Verification**:
   - Start `DeskdropService`, connect to a desktop peer, and send app to background for 60 seconds:
     ```bash
     adb shell am start-foreground-service -a com.deskdrop.START com.deskdrop/.DeskdropService
     sleep 60
     adb logcat -d | grep "Deskdrop"
     ```
   - Confirm background service maintains active connection without crashing.
