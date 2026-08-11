# Milestone 2 Handoff Report — Stress Testing & Crash Reproduction

## 1. Observation

### Command Execution & Outputs
1. **ADB Device Verification**:
   - Command: `/opt/homebrew/share/android-commandlinetools/platform-tools/adb devices`
   - Output: Device `979116c` attached and online.
2. **Logcat Clear**:
   - Command: `/opt/homebrew/share/android-commandlinetools/platform-tools/adb logcat -c`
   - Result: Logcat buffer cleared successfully.
3. **Activity Launch**:
   - Command: `/opt/homebrew/share/android-commandlinetools/platform-tools/adb shell am start -W -n com.deskdrop.debug/com.deskdrop.MainActivity`
   - Result: `Status: ok`, `Activity: com.deskdrop.debug/com.deskdrop.MainActivity`
4. **ADB Monkey Stress Test (5000 events)**:
   - Command: `/opt/homebrew/share/android-commandlinetools/platform-tools/adb shell monkey -p com.deskdrop.debug -v 5000`
   - Output (Verbatim Crash Snippet):
     ```
     // CRASH: com.deskdrop.debug (pid 6501)
     // Short Msg: Native crash
     // Long Msg: Native crash: Aborted
     // Build Label: OnePlus/CPH2661IN/OP5E93L1:16/UKQ1.231108.001/U.R4T2.3a24ca8-1593c70-163c453:user/release-keys
     // Process name is com.deskdrop.debug, uid is 11054
     // signal 6 (SIGABRT), code -1 (SI_QUEUE), fault addr --------
     // Abort message: 'android context was not initialized'
     // 32 total frames
     // backtrace:
     //       #00 pc 000000000008a90c  /apex/com.android.runtime/lib64/bionic/libc.so (abort+160)
     //       ...
     //       #13 pc 00000000000b9404  /data/app/~~8CmUNwWDFrLR_wcqJxb5nA==/com.deskdrop.debug-pRLgpGOlG0CP_3G5UGGtyw==/base.apk (offset 0x13cd000) (Java_com_deskdrop_DeskdropJni_initContext+668)
     //       #14 pc 00000000002c23a0  /apex/com.android.art/lib64/libart.so (art_quick_generic_jni_trampoline+144)
     //       ...
     //       #19 pc 000000000001765c  [anon:dalvik-classes4.dex extract....deskdrop.debug-pRLgpGOlG0CP_3G5UGGtyw==/base.apk] (com.deskdrop.DeskdropService.onStartCommand+0)
     // ** Monkey aborted due to error.
     // Events injected: 4
     // ** System appears to have crashed at event 4 of 5000 using seed 1786252250229
     ```

### Direct Code Inspection Findings
1. **`deskdrop-core/src/jni_android.rs` (lines 39-60)**:
   ```rust
   pub extern "system" fn Java_com_deskdrop_DeskdropJni_initContext(
       env: JNIEnv,
       _class: JClass,
       context: jni::objects::JObject,
   ) {
       if ANDROID_CONTEXT.get().is_some() {
           return;
       }
       let vm = env.get_java_vm().expect("Failed to get JavaVM");
       let ctx_ref = env.new_global_ref(context).expect("Failed to create GlobalRef");
       let vm_ptr = vm.get_java_vm_pointer() as *mut std::ffi::c_void;
       let ctx_ptr = ctx_ref.as_obj().as_raw() as *mut std::ffi::c_void;
       
       if ANDROID_CONTEXT.set(ctx_ref).is_ok() {
           unsafe {
               let current_ctx = ndk_context::android_context();
               if current_ctx.vm().is_null() || current_ctx.context().is_null() {
                   ndk_context::initialize_android_context(vm_ptr, ctx_ptr);
               }
           }
       }
   }
   ```
2. **`platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`**:
   - `engineLock` is declared at line 324 (`ReentrantReadWriteLock`).
   - `writeLock()` is used during `onDestroy()` (line 787).
   - Only 2 JNI call sites in `DeskdropService.kt` acquire `readLock()` (lines 961, 973). Over 30 JNI call sites invoke JNI methods directly passing `engineHandle` without holding `engineLock.readLock()`.
3. **`platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt` (lines 286-313)**:
   - `screenshotObserver` queries `MediaStore.Images.Media.EXTERNAL_CONTENT_URI` with column `MediaStore.Images.Media.DATA` using `it.getColumnIndexOrThrow(...)` without wrapping in exception handlers for missing permissions or missing media columns.

---

## 2. Logic Chain

1. **Native Crash on Service Startup**:
   - **Observation**: Monkey test crashed at event 4 with `SIGABRT`, abort message `'android context was not initialized'`, in `Java_com_deskdrop_DeskdropJni_initContext`.
   - **Reasoning**: `Java_com_deskdrop_DeskdropJni_initContext` attempts to inspect `let current_ctx = ndk_context::android_context();` prior to initializing `ndk_context::initialize_android_context(vm_ptr, ctx_ptr)`.
   - **Inference**: In `ndk-context` crate v0.1, calling `android_context()` when uninitialized panics/aborts internally with `'android context was not initialized'`. Therefore, checking `current_ctx` before initialization causes the exact crash it intended to avoid.

2. **JNI Handle Concurrency & Memory Lifetime Safety**:
   - **Observation**: `DeskdropService.kt` has `engineLock` defined, but >30 JNI calls pass `engineHandle` without acquiring `engineLock.readLock()`. `jni_android.rs` drops `AndroidHandle` upon `DeskdropJni.stop()`.
   - **Reasoning**: If `stop()` executes on the main thread while background coroutines or threads execute JNI calls, `engineHandle` becomes a dangling pointer.
   - **Inference**: Dereferencing a raw pointer (`&*(handle as *const AndroidHandle)`) in Rust after `stop()` causes a Use-After-Free SIGSEGV crash.

3. **Storage & Permission Hardening**:
   - **Observation**: `screenshotObserver` calls `contentResolver.query()` and `getColumnIndexOrThrow()` directly.
   - **Reasoning**: On Android 13+ or when media permissions are revoked, querying `MediaStore.Images.Media.DATA` can throw `SecurityException` or `IllegalArgumentException`.
   - **Inference**: Uncaught runtime exceptions inside ContentObserver callbacks crash the app process.

---

## 3. Caveats

- **Device Specifics**: Tested on hardware device `979116c` (OnePlus CPH2661, Android 16 / UKQ1.231108.001).
- **Subsequent Events**: Because the Monkey stress test was aborted at event 4 due to the fatal native crash in `initContext`, subsequent UI interaction paths (e.g., deep dialog navigation, rapid QR scanning, heavy file transfers) could not complete 5000 events until `initContext` is fixed in M3.

---

## 4. Conclusion

Milestone 2 Stress Testing identified 1 Critical native abort crash that completely halts execution during initial launch/Monkey testing, 1 High risk JNI concurrency race condition vulnerability, and 1 Medium risk MediaStore exception crash path. Structural fixes for these identified issues are required in Milestone 3.

---

## 5. Verification Method

To independently verify this crash reproduction:

1. Clear logcat:
   `/opt/homebrew/share/android-commandlinetools/platform-tools/adb logcat -c`
2. Launch the app activity:
   `/opt/homebrew/share/android-commandlinetools/platform-tools/adb shell am start -W -n com.deskdrop.debug/com.deskdrop.MainActivity`
3. Execute ADB Monkey stress test:
   `/opt/homebrew/share/android-commandlinetools/platform-tools/adb shell monkey -p com.deskdrop.debug -v 5000`
4. Inspect logcat:
   `/opt/homebrew/share/android-commandlinetools/platform-tools/adb logcat -d | grep -A 10 "android context was not initialized"`
5. Invalidation condition: If Monkey test completes 5000 events with exit code 0 and no SIGABRT/native crash, the issue is resolved.

---

## Challenge Report

### Challenge Summary
**Overall risk assessment**: HIGH

### Challenges

#### 1. [Critical] Native Abort in `initContext`
- **Assumption challenged**: Calling `ndk_context::android_context()` before `initialize_android_context` returns an empty context structure.
- **Attack scenario**: Launching `DeskdropService` on startup calls `DeskdropJni.initContext(applicationContext)`, triggering `ndk_context::android_context()` which aborts the process.
- **Blast radius**: 100% app crash on startup/service launch.
- **Mitigation**: Remove `ndk_context::android_context()` check prior to `initialize_android_context(vm_ptr, ctx_ptr)` and directly call `ndk_context::initialize_android_context(vm_ptr, ctx_ptr)`.

#### 2. [High] Unguarded JNI Handle Dereference & Concurrency Race
- **Assumption challenged**: `engineHandle` remains valid across all background coroutines and threads during service destruction.
- **Attack scenario**: `onDestroy()` drops `AndroidHandle` via `DeskdropJni.stop()` while a background thread invokes `DeskdropJni.pollEvent` or `DeskdropJni.pushText`.
- **Blast radius**: Native SIGSEGV crash / process exit.
- **Mitigation**: Acquire `engineLock.readLock()` for all JNI calls in Kotlin and add handle validity checks in Rust JNI bindings.

#### 3. [Medium] MediaStore / ContentObserver Exception Crash
- **Assumption challenged**: `MediaStore.Images.Media.DATA` column is always present and accessible without runtime permission failures.
- **Attack scenario**: Screenshot observer fires when storage permission is absent or on modern Scoped Storage devices.
- **Blast radius**: `SecurityException` / `IllegalArgumentException` crashing the service.
- **Mitigation**: Wrap `contentResolver.query` and column extractions in a `try-catch` block.
