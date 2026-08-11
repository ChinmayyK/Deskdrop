# Crash & Vulnerability Inventory — Deskdrop Android

## Overview
Automated stress testing (`adb shell monkey`), logcat analysis, and code auditing discovered 4 structural crash vectors affecting application startup, JNI thread safety, and service lifecycle stability.

---

### Crash Vector 1: Native SIGABRT in `Java_com_deskdrop_DeskdropJni_initContext`
- **Severity**: CRITICAL
- **Location**: `deskdrop-core/src/jni_android.rs` (lines 56-61) & `DeskdropService.kt` (line 671)
- **Stack Trace / Error**:
  `Fatal signal 6 (SIGABRT), code -1 (SI_QUEUE) in tid (.deskdrop.debug), pid (.deskdrop.debug)`
  `Abort message: 'android context was not initialized'`
  `Backtrace: Java_com_deskdrop_DeskdropJni_initContext+668`
- **Root Cause**: `Java_com_deskdrop_DeskdropJni_initContext` calls `ndk_context::android_context()` to inspect context pointers *before* calling `ndk_context::initialize_android_context(vm_ptr, ctx_ptr)`. In `ndk-context` v0.1, calling `android_context()` when uninitialized panics internally with `'android context was not initialized'`, aborting the entire Android process.
- **Fix Strategy**: Remove the `ndk_context::android_context()` pre-check in `deskdrop-core/src/jni_android.rs`. Directly call `ndk_context::initialize_android_context(vm_ptr, ctx_ptr)` when setting the global reference.

---

### Crash Vector 2: JNI Concurrency Race Condition & Use-After-Free (SIGSEGV)
- **Severity**: HIGH
- **Location**: `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt` (>30 JNI call sites) & `deskdrop-core/src/jni_android.rs`
- **Stack Trace / Error**:
  `Fatal signal 11 (SIGSEGV), code 1 (SEGV_MAPERR) in tid (.deskdrop.debug)`
- **Root Cause**: `DeskdropService.kt` declares `engineLock` (`ReentrantReadWriteLock`), but >30 JNI methods pass `engineHandle` without acquiring `engineLock.readLock()`. When `onDestroy()` executes `DeskdropJni.stop(engineHandle)` under `writeLock()`, Rust frees `AndroidHandle` and sets `engineHandle = 0L`. Concurrent threads or event handlers calling JNI methods with cached non-zero handle values dereference dangling pointers in Rust (`unsafe { &*(handle as *const AndroidHandle) }`).
- **Fix Strategy**:
  1. In Kotlin (`DeskdropService.kt` & `DeskdropJni.kt`), wrap all JNI calls in `engineLock.readLock { if (engineHandle != 0L) { ... } }`.
  2. In Rust (`deskdrop-core/src/jni_android.rs`), check `if handle == 0 { return ... }` and safely check pointer validity before dereferencing `AndroidHandle`.

---

### Crash Vector 3: Uncaught MediaStore / ContentObserver Exceptions
- **Severity**: MEDIUM
- **Location**: `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt` (lines 286-313, `screenshotObserver`)
- **Stack Trace / Error**: `java.lang.SecurityException` / `java.lang.IllegalArgumentException`
- **Root Cause**: `screenshotObserver` queries `MediaStore.Images.Media.EXTERNAL_CONTENT_URI` with column `MediaStore.Images.Media.DATA` using `it.getColumnIndexOrThrow(...)` without exception handling. When storage permissions are denied or on modern Scoped Storage devices, ContentResolver query throws uncaught exceptions, crashing the app process.
- **Fix Strategy**: Wrap `contentResolver.query(...)` and `getColumnIndexOrThrow` calls in a `try { ... } catch (e: Exception) { Log.w(...) }` block.

---

### Crash Vector 4: Uncaught StorageStatsManager Null Cast Exception
- **Severity**: MEDIUM
- **Location**: `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt` (lines 2630-2678, `startStorageMonitor`)
- **Stack Trace / Error**: `java.lang.ClassCastException` / `java.lang.NullPointerException`
- **Root Cause**: `getSystemService(Context.STORAGE_STATS_SERVICE) as StorageStatsManager` performs an unsafe cast. On devices where `STORAGE_STATS_SERVICE` is unavailable or returns null, this throws a fatal exception.
- **Fix Strategy**: Use safe cast `getSystemService(Context.STORAGE_STATS_SERVICE) as? StorageStatsManager` and check for null before invoking methods.
