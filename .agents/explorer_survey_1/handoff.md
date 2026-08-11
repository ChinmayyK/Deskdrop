# Handoff Report: Codebase Layout & Build Config Survey

## 1. Observation

### 1.1 Repository Structure & Directories
The Deskdrop project root is located at `/Users/chinmayk/Projects/Deskdrop`.
Key subdirectories:
- `deskdrop-core`: Rust core backend library (compiled via `cargo-ndk` to `libdeskdrop_core.so` native binaries).
- `deskdrop-cli`: Command-line tool implementation in Rust.
- `platforms/`: Cross-platform UI / host implementations:
  - `platforms/android/`: Android project root (containing module `:app`).
  - `platforms/linux/`: Linux target.
  - `platforms/macos/`: macOS target.
  - `platforms/windows/`: Windows target.
- `assets/`: Static asset files.
- `docs/`: Project documentation.
- `scripts/`: Build and automation scripts (e.g., `scripts/build-android.sh`).
- `build/` & `target/`: Build artifact outputs.

### 1.2 Gradle & Build Files Location
Gradle configuration files for Android are located under `platforms/android`:
- `platforms/android/settings.gradle`:
  ```groovy
  rootProject.name = "Deskdrop"
  include ':app'
  ```
- `platforms/android/build.gradle`:
  ```groovy
  plugins {
      id 'com.android.application' version '8.2.0' apply false
      id 'org.jetbrains.kotlin.android' version '1.9.22' apply false
  }
  ```
- `platforms/android/app/build.gradle`:
  - Namespace: `com.deskdrop` (line 7)
  - Application ID: `com.deskdrop` (line 11; suffix `.debug` applied in debug build type at line 48 -> `com.deskdrop.debug`)
  - minSdk: `26` (Android 8.0 Oreo) (line 12)
  - targetSdk: `34` (Android 14) (line 13)
  - compileSdk: `34` (line 8)
  - versionCode: `43` (line 14)
  - versionName: `1.2.4` (line 15)
  - Java compatibility / Kotlin JVM target: Java 17 (`JavaVersion.VERSION_17`, `jvmTarget = '17'`)
  - Compose compiler: `1.5.8` (line 67)
  - Supported ABIs: `arm64-v8a`, `armeabi-v7a`, `x86_64` (line 20)
  - Native JNI libs path: `src/main/jniLibs` (line 72)
- `platforms/android/gradle.properties`:
  ```properties
  android.useAndroidX=true
  android.enableJetifier=true
  org.gradle.jvmargs=-Xmx2048m -Dkotlin.daemon.jvm.options="-Xmx2048m"
  ```
- `platforms/android/gradle/wrapper/gradle-wrapper.properties`:
  `distributionUrl=https\://services.gradle.org/distributions/gradle-8.2-bin.zip`
- `platforms/android/gradlew` & `platforms/android/gradlew.bat`: Gradle wrapper executables.

### 1.3 AndroidManifest.xml Files
- Primary Source Manifest: `platforms/android/app/src/main/AndroidManifest.xml`
- Build Generated / Merged Manifests:
  - `platforms/android/app/build/intermediates/merged_manifest/debug/AndroidManifest.xml`
  - `platforms/android/app/build/intermediates/merged_manifest/release/AndroidManifest.xml`
  - `platforms/android/app/build/intermediates/bundle_manifest/release/AndroidManifest.xml`
  - `platforms/android/app/build/intermediates/merged_manifests/debug/AndroidManifest.xml`
  - `platforms/android/app/build/intermediates/merged_manifests/release/AndroidManifest.xml`
  - `platforms/android/app/build/intermediates/packaged_manifests/debug/AndroidManifest.xml`
  - `platforms/android/app/build/intermediates/packaged_manifests/release/AndroidManifest.xml`

### 1.4 Android Application Components & Kotlin Classes
Package namespace: `com.deskdrop`

- **Application Class**:
  - `com.deskdrop.DeskdropApp` (`platforms/android/app/src/main/java/com/deskdrop/DeskdropApp.kt`)

- **Activity Classes**:
  1. `com.deskdrop.MainActivity` (`platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt`): Main entry activity (`MAIN` / `LAUNCHER`, theme `@style/Theme.App.Starting`).
  2. `com.deskdrop.PairingActivity` (`platforms/android/app/src/main/java/com/deskdrop/PairingActivity.kt`): TOFU pairing dialog (`launchMode="singleTask"`).
  3. `com.deskdrop.DiagnosticsActivity` (`platforms/android/app/src/main/java/com/deskdrop/DiagnosticsActivity.kt`): Application diagnostics screen.
  4. `com.deskdrop.CameraStreamActivity` (`platforms/android/app/src/main/java/com/deskdrop/CameraStreamActivity.kt`): Camera streaming activity (`launchMode="singleTask"`, portrait orientation).
  5. `com.deskdrop.DeskdropShareTarget` (defined inside `platforms/android/app/src/main/java/com/deskdrop/DeskdropTileService.kt`): System share target activity (`android.intent.action.SEND` / `SEND_MULTIPLE`).

- **Background Services**:
  1. `com.deskdrop.DeskdropService` (`platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`): Primary network foreground service (`foregroundServiceType="connectedDevice"`, `stopWithTask="false"`).
  2. `com.deskdrop.DeskdropNotificationListener` (`platforms/android/app/src/main/java/com/deskdrop/DeskdropNotificationListener.kt`): Notification listener service (`BIND_NOTIFICATION_LISTENER_SERVICE`).
  3. `com.deskdrop.DeskdropAccessibilityService` (`platforms/android/app/src/main/java/com/deskdrop/DeskdropAccessibilityService.kt`): Accessibility service fallback for background clipboard sync (`BIND_ACCESSIBILITY_SERVICE`).
  4. `com.deskdrop.DeskdropTileService` (`platforms/android/app/src/main/java/com/deskdrop/DeskdropTileService.kt`): Quick Settings active tile service (`BIND_QUICK_SETTINGS_TILE`).
  5. `com.deskdrop.PushClipboardTileService` (`platforms/android/app/src/main/java/com/deskdrop/PushClipboardTileService.kt`): Quick Settings tile service for pushing clipboard to connected Mac (`BIND_QUICK_SETTINGS_TILE`).

- **Broadcast Receivers**:
  1. `com.deskdrop.BootReceiver` (`platforms/android/app/src/main/java/com/deskdrop/BootReceiver.kt`): Boot completed receiver (`BOOT_COMPLETED`, `LOCKED_BOOT_COMPLETED`, `MY_PACKAGE_REPLACED`, `directBootAware=true`).
  2. `com.deskdrop.CallStateReceiver` (`platforms/android/app/src/main/java/com/deskdrop/CallStateReceiver.kt`): Phone state receiver (`PHONE_STATE`).

- **Content Provider**:
  - `androidx.core.content.FileProvider`: Authority `${applicationId}.fileprovider` (pointing to `@xml/file_paths`).

- **UI Components (Jetpack Compose)**:
  - `com.deskdrop.ui.MainScreen` (`platforms/android/app/src/main/java/com/deskdrop/ui/MainScreen.kt`)
  - `com.deskdrop.ui.OnboardingScreen` (`platforms/android/app/src/main/java/com/deskdrop/ui/OnboardingScreen.kt`)
  - `com.deskdrop.ui.PairingScreen` (`platforms/android/app/src/main/java/com/deskdrop/ui/PairingScreen.kt`)
  - `com.deskdrop.ui.SettingsScreen` (`platforms/android/app/src/main/java/com/deskdrop/ui/SettingsScreen.kt`)
  - Theme subpackage `com.deskdrop.ui.theme`: `Color.kt`, `DesignSystem.kt`, `Theme.kt`, `Type.kt`.

- **Manager, JNI & Helper Classes**:
  - `com.deskdrop.ActivityFeedManager` (`ActivityFeedManager.kt`)
  - `com.deskdrop.DeskdropJni` (`DeskdropJni.kt`): JNI bridge declarations to `libdeskdrop_core.so`.
  - `com.deskdrop.PeerSnapshot` (`PeerSnapshot.kt`): Peer state representation.
  - `com.deskdrop.RemoteFileManager` (`RemoteFileManager.kt`)
  - `com.deskdrop.TestDnd` (`TestDnd.kt`)
  - `com.deskdrop.TransferManager` (`TransferManager.kt`): Core transfer state management.

- **Native Libraries**:
  - `platforms/android/app/src/main/jniLibs/arm64-v8a/libdeskdrop_core.so`
  - `platforms/android/app/src/main/jniLibs/arm64-v8a/libandroid_native_keyring_store-88275267ef25ea17.so`
  - `platforms/android/app/src/main/jniLibs/armeabi-v7a/libdeskdrop_core.so`
  - `platforms/android/app/src/main/jniLibs/x86_64/libdeskdrop_core.so`

### 1.5 Key Dependencies
- AndroidX Core & UI: `androidx.core:core-ktx:1.12.0`, `androidx.appcompat:appcompat:1.6.1`, `com.google.android.material:material:1.11.0`, `androidx.swiperefreshlayout:swiperefreshlayout:1.1.0`, `androidx.core:core-splashscreen:1.0.1`
- Utilities & Scanning: `com.google.android.gms:play-services-code-scanner:16.1.0`, `com.google.zxing:core:3.5.3`
- Architecture & Background: `androidx.lifecycle:lifecycle-service:2.7.0`, `androidx.lifecycle:lifecycle-runtime-ktx:2.7.0`, `androidx.work:work-runtime-ktx:2.9.0`
- Jetpack Compose: BOM `2024.02.00` (`ui`, `material3`, `material-icons-extended`, `ui-tooling-preview`), `activity-compose:1.8.2`, `lifecycle-viewmodel-compose:2.7.0`, `kotlinx-collections-immutable:0.3.7`
- CameraX: `camera-core`, `camera-camera2`, `camera-lifecycle`, `camera-view` (v1.3.1)
- Image/Video Loading: `io.coil-kt:coil-compose:2.5.0`, `io.coil-kt:coil-video:2.5.0`

---

## 2. Logic Chain

1. **Project Layout Invalidation & Module Mapping**:
   - Examination of the top-level directory structure via directory listing confirmed that Deskdrop is a multi-platform Rust + Android workspace.
   - Rust core backend is located in `deskdrop-core`, while all Android application components reside under `platforms/android`.

2. **Build Configuration Mapping**:
   - Inspection of `platforms/android/settings.gradle` confirmed that the Android project consists of a single root project `"Deskdrop"` and single app module `:app`.
   - Inspection of `platforms/android/app/build.gradle` confirmed target SDK 34, min SDK 26, applicationId `com.deskdrop` (with debug suffix `.debug`), AGP 8.2.0, Kotlin 1.9.22, Java 17, and ABI targets `arm64-v8a`, `armeabi-v7a`, `x86_64`.
   - `scripts/build-android.sh` maps `deskdrop-core` Rust compilation via `cargo-ndk` to output `.so` native libraries directly into `platforms/android/app/src/main/jniLibs`.

3. **Manifest Component Mapping**:
   - Direct view of `platforms/android/app/src/main/AndroidManifest.xml` provided explicit declarations of 5 Activities (`MainActivity`, `PairingActivity`, `DiagnosticsActivity`, `CameraStreamActivity`, `DeskdropShareTarget`), 5 Services (`DeskdropService`, `DeskdropNotificationListener`, `DeskdropAccessibilityService`, `DeskdropTileService`, `PushClipboardTileService`), 2 Receivers (`BootReceiver`, `CallStateReceiver`), and 1 ContentProvider (`FileProvider`).

---

## 3. Caveats

- **Native Rust Rebuilds**: Building the APK manually using `./gradlew assembleDebug` assumes JNI libraries under `src/main/jniLibs` are already up to date. If `deskdrop-core` changes, `scripts/build-android.sh` or `cargo ndk` must be run first to update `.so` files.
- **Runtime Environment**: Testing stress tests (`adb shell monkey`) and service connectivity requires a running Android emulator or connected device with `adb` enabled.

---

## 4. Conclusion

The Deskdrop Android application project is well-structured under `platforms/android/app` with package `com.deskdrop`. It uses Kotlin 1.9.22, AGP 8.2.0, SDK 34 (min SDK 26), Jetpack Compose, CameraX, WorkManager, and native Rust JNI bindings (`libdeskdrop_core.so`). The main entry point is `MainActivity` and the central background engine runs inside the foreground service `DeskdropService`.

---

## 5. Verification Method

To independently verify the project setup and survey findings:

1. **Verify Gradle & Build Files**:
   Inspect `platforms/android/app/build.gradle` line 7 to line 15 to confirm package name (`com.deskdrop`), SDK levels (min 26, target 34), version code (43), and version name ("1.2.4").
2. **Verify Component Declarations**:
   Inspect `platforms/android/app/src/main/AndroidManifest.xml` lines 70 to 220 to verify Activity, Service, and Receiver component declarations.
3. **Verify Buildability**:
   Run `./gradlew assembleDebug` in `platforms/android` or `./scripts/build-android.sh --debug` from project root to ensure clean build output.
