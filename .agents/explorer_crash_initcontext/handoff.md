# Handoff Report — Native JNI `initContext` SIGABRT Crash Investigation

**Explorer**: Explorer 4 (`explorer_crash_initcontext`)  
**Date**: 2026-08-07  
**Scope**: Native crash (`SIGABRT`) in `Java_com_deskdrop_DeskdropJni_initContext+668`  
**Target Files**: `deskdrop-core/src/jni_android.rs`, `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`, `DeskdropJni.kt`

---

## 1. Observation

1. **Logcat Native Crash Backtrace**:
   - `Fatal signal 6 (SIGABRT), code -1 (SI_QUEUE) in tid 4918 (.deskdrop.debug), pid 4918 (.deskdrop.debug)`
   - Stack frame #13: `pc 00000000000b9404 /data/app/.../base.apk (Java_com_deskdrop_DeskdropJni_initContext+668)`
   - Invoked from `DeskdropService.kt:671` during service startup: `DeskdropJni.initContext(applicationContext)`.

2. **Native Symbol Disassembly**:
   - `nm -D platforms/android/app/src/main/jniLibs/arm64-v8a/libdeskdrop_core.so` shows symbol `Java_com_deskdrop_DeskdropJni_initContext` at entry point `0x00000000000b9168`.
   - PC offset `0xb9404` corresponds to instruction `0xb9404: bl 0x205c30 <deskdrop_trust_peer+0x1d0b8>`.
   - Function at `0x205c30` checks static initialization state and branches to internal panic/error handler `0x31efe0` if uninitialized.

3. **Source Code Inspection (`deskdrop-core/src/jni_android.rs:36-60`)**:
   ```rust
   static ANDROID_CONTEXT: OnceLock<jni::objects::GlobalRef> = OnceLock::new();

   #[no_mangle]
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

4. **Dependency Source Inspection (`~/.cargo/registry/src/.../ndk-context-0.1.1/src/lib.rs:70-73`)**:
   ```rust
   /// Main entry point to this crate. Returns an [`AndroidContext`].
   pub fn android_context() -> AndroidContext {
       unsafe { ANDROID_CONTEXT.expect("android context was not initialized") }
   }
   ```

---

## 2. Logic Chain

1. **Observation**: `DeskdropService.kt:671` calls `DeskdropJni.initContext(applicationContext)` during cold start.
2. **Observation**: `Java_com_deskdrop_DeskdropJni_initContext` in `jni_android.rs:54` executes `let current_ctx = ndk_context::android_context();` inside `if ANDROID_CONTEXT.set(ctx_ref).is_ok()`.
3. **Observation**: `ndk_context::android_context()` in `ndk-context-0.1.1/src/lib.rs:72` executes `unsafe { ANDROID_CONTEXT.expect("android context was not initialized") }`.
4. **Inference**: Because `ndk_context` has NOT been initialized prior to `Java_com_deskdrop_DeskdropJni_initContext` being called, `ndk_context::android_context()` unconditionally panics with `"android context was not initialized"`.
5. **Observation**: `Java_com_deskdrop_DeskdropJni_initContext` is exported as `extern "system" fn` (JNI C-ABI interface).
6. **Inference**: In Rust, panic unwinding across an `extern "system"` FFI boundary is illegal. The Rust runtime catches illegal FFI unwinds and invokes `abort()`, which raises `SIGABRT` (Signal 6) and immediately kills the process (`com.deskdrop.debug`).
7. **Secondary Flaw Identification**:
   - `env.get_java_vm().expect(...)` and `env.new_global_ref(...).expect(...)` will panic if JNIEnv or Context handles are invalid.
   - `ndk_context::initialize_android_context` contains `assert!(previous.is_none())`, which panics if called when `ndk_context` is already initialized.
   - The outer JNI function lacks `std::panic::catch_unwind`, leaving any potential panic unhandled.

---

## 3. Caveats

- The current project scope requires read-only investigation for Explorer agents; code modifications must be applied by an Implementer agent or Worker.
- Testing on connected device `979116c` showed immediate process death upon `am start` due to this crash.

---

## 4. Conclusion & Structural Fix Strategy

### Root Cause
`Java_com_deskdrop_DeskdropJni_initContext` in `deskdrop-core/src/jni_android.rs` attempts to check `ndk_context::android_context()` before initializing `ndk_context`. In `ndk-context` v0.1.1, `android_context()` panics if uninitialized. This panic unwinds across the `extern "system"` JNI boundary, forcing a process `abort()` (`SIGABRT`).

### Recommended Fix Strategy

#### A. Rust Fix (`deskdrop-core/src/jni_android.rs`)
1. Remove `let current_ctx = ndk_context::android_context();`.
2. Wrap the entire function execution inside `std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { ... }))` to guarantee no Rust panic ever escapes to C/Kotlin.
3. Handle JNI operations safely without `.expect(...)`.
4. Check if `context.is_null()` or `env.is_same_object(&context, &JObject::null())` before dereferencing.
5. Direct call to `ndk_context::initialize_android_context(vm_ptr, ctx_ptr)` inside `ANDROID_CONTEXT.set(ctx_ref).is_ok()`, guarded by `catch_unwind`.

**Proposed Implementation Code**:
```rust
#[no_mangle]
pub extern "system" fn Java_com_deskdrop_DeskdropJni_initContext(
    env: JNIEnv,
    _class: JClass,
    context: jni::objects::JObject,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if ANDROID_CONTEXT.get().is_some() {
            return;
        }
        if context.is_null() {
            return;
        }
        let vm = match env.get_java_vm() {
            Ok(vm) => vm,
            Err(_) => return,
        };
        let ctx_ref = match env.new_global_ref(context) {
            Ok(ref_) => ref_,
            Err(_) => return,
        };
        let vm_ptr = vm.get_java_vm_pointer() as *mut std::ffi::c_void;
        let ctx_ptr = ctx_ref.as_obj().as_raw() as *mut std::ffi::c_void;

        if ANDROID_CONTEXT.set(ctx_ref).is_ok() {
            let _ = std::panic::catch_unwind(|| unsafe {
                ndk_context::initialize_android_context(vm_ptr, ctx_ptr);
            });
        }
    }));
}
```

#### B. Kotlin Fix (`DeskdropService.kt` / `DeskdropJni.kt`)
In `DeskdropService.kt:671`, guard `initContext`:
```kotlin
try {
    DeskdropJni.initContext(applicationContext)
} catch (e: Throwable) {
    Log.e(TAG, "Failed to initialize JNI context", e)
}
```

---

## 5. Verification Method

To independently verify the fix after implementation:

1. **Rebuild Native Core & Android App**:
   ```bash
   cd /Users/chinmayk/Projects/Deskdrop/platforms/android
   ./gradlew installDebug
   ```

2. **Launch Application on Hardware Device**:
   ```bash
   /opt/homebrew/share/android-commandlinetools/platform-tools/adb shell am force-stop com.deskdrop.debug
   /opt/homebrew/share/android-commandlinetools/platform-tools/adb shell am start -W -n com.deskdrop.debug/com.deskdrop.MainActivity
   ```

3. **Verify Process Survival**:
   ```bash
   /opt/homebrew/share/android-commandlinetools/platform-tools/adb shell "ps -A | grep deskdrop.debug"
   ```
   *Expected Output*: Process `com.deskdrop.debug` remains running with an active PID.

4. **Verify Logcat**:
   ```bash
   /opt/homebrew/share/android-commandlinetools/platform-tools/adb logcat -d | grep -iE "fatal|signal 6|SIGABRT|DeskdropJni"
   ```
   *Expected Output*: Zero `SIGABRT` native crash logs.
