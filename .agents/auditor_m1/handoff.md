# Forensic Audit Report — Milestone 1 (Baseline Build & Deployment)

**Work Product**: Milestone 1 Artifacts (`app-debug.apk` and ADB Deployment on device `979116c`)  
**Profile**: General Project  
**Integrity Mode**: Development  
**Verdict**: CLEAN  

---

## 1. Observation

### Observation 1.1: APK Artifact Authenticity & Metadata
- **File Path**: `/Users/chinmayk/Projects/Deskdrop/platforms/android/app/build/outputs/apk/debug/app-debug.apk`
- **File Size**: `36,274,703 bytes` (~36.3 MB)
- **Modification Time**: `2026-08-07 01:07`
- **SHA-256 Checksum**:
  ```
  7718cd1d177ded514831ee044a6c2f78e6bf9b3b693c6d360d919a7b7be6095a
  ```
- **Metadata Output (`output-metadata.json`)**:
  ```json
  {
    "version": 3,
    "artifactType": {
      "type": "APK",
      "kind": "Directory"
    },
    "applicationId": "com.deskdrop.debug",
    "variantName": "debug",
    "elements": [
      {
        "type": "SINGLE",
        "filters": [],
        "attributes": [],
        "versionCode": 43,
        "versionName": "1.2.4",
        "outputFile": "app-debug.apk"
      }
    ],
    "elementType": "File"
  }
  ```
- **Archive Inspection (`unzip -l app-debug.apk`)**:
  - `lib/arm64-v8a/libdeskdrop_core.so` (3,398,368 bytes)
  - `lib/armeabi-v7a/libdeskdrop_core.so` (2,343,424 bytes)
  - `lib/x86_64/libdeskdrop_core.so` (3,818,792 bytes)
  - `AndroidManifest.xml` (27,152 bytes)
  - `classes.dex` through `classes7.dex`

### Observation 1.2: Physical ADB Device Verification (`979116c`)
- **Device Status Command**:
  ```bash
  /opt/homebrew/share/android-commandlinetools/platform-tools/adb devices
  ```
  - **Verbatim Output**:
    ```
    List of devices attached
    979116c	device
    ```

- **Package List Verification Command**:
  ```bash
  /opt/homebrew/share/android-commandlinetools/platform-tools/adb -s 979116c shell pm list packages | grep deskdrop
  ```
  - **Verbatim Output**:
    ```
    package:com.deskdrop.debug
    package:com.deskdrop
    ```

- **Package Path & Metadata Command**:
  ```bash
  /opt/homebrew/share/android-commandlinetools/platform-tools/adb -s 979116c shell dumpsys package com.deskdrop.debug | grep -E "versionName|lastUpdateTime|codePath"
  ```
  - **Verbatim Output**:
    ```
    codePath=/data/app/~~3JeZOSq_Km8HxUz1Hd7SIA==/com.deskdrop.debug-CH__unm-q19l-PHkD6_xTQ==
    versionName=1.2.4
    lastUpdateTime=2026-08-07 01:09:11
    ```

- **App Launch Verification Command**:
  ```bash
  /opt/homebrew/share/android-commandlinetools/platform-tools/adb -s 979116c shell am start -W -n com.deskdrop.debug/com.deskdrop.MainActivity
  ```
  - **Verbatim Output**:
    ```
    Starting: Intent { cmp=com.deskdrop.debug/com.deskdrop.MainActivity }
    Status: ok
    LaunchState: UNKNOWN (0)
    Activity: com.deskdrop.debug/com.deskdrop.MainActivity
    WaitTime: 1006
    Complete
    ```

- **Process & Logcat Verification**:
  - `ps -ef | grep deskdrop` confirmed process `com.deskdrop.debug` running on device (PID `1450`).
  - Logcat confirmed native library loading:
    `Load ... libdeskdrop_core.so using class loader ns clns-9 ... ok`

### Observation 1.3: Integrity & Facade Audit
- **Hardcoded Output Analysis**: Searched Kotlin codebase (`platforms/android/app/src/main/java/com/deskdrop/`) and Rust core (`deskdrop-core/src/`). No hardcoded false test outputs, mock return strings, or self-certifying pass checks found.
- **Facade Analysis**: Native JNI initialization in `jni_android.rs` and service startup logic in `DeskdropService.kt` contain genuine system integration (ndk-context, wake lock management, mDNS re-registration).
- **Artifact Pre-population Analysis**: No pre-populated false test results, fabricated verification logs, or attestation shortcuts were found in workspace.

---

## 2. Logic Chain

1. **Observation 1.1**: Inspecting `app-debug.apk` confirmed valid ZIP structure containing compiled Java/Kotlin bytecode (`classes.dex`–`classes7.dex`), native shared libraries (`libdeskdrop_core.so` across arm64-v8a, armeabi-v7a, x86_64), and matching `output-metadata.json`.
   **Inference**: `app-debug.apk` is an authentic, freshly compiled Android debug binary produced by Gradle.

2. **Observation 1.2**: Running `adb -s 979116c shell pm list packages` and `dumpsys package` confirmed `com.deskdrop.debug` is installed on physical device `979116c` with `lastUpdateTime=2026-08-07 01:09:11`. `am start` launched `com.deskdrop.MainActivity` successfully, creating PID 1450 and loading native Rust libraries.
   **Inference**: Deployment of `com.deskdrop.debug` to physical hardware device `979116c` is confirmed empirically.

3. **Observation 1.3**: Code review and static inspection confirmed absence of prohibited patterns (hardcoded test results, facade implementations, or pre-populated attestation files).
   **Inference**: The Milestone 1 deliverables pass all integrity checks under Development mode.

---

## 3. Caveats

- Milestone 1 is restricted to baseline build, deployment, package installation, and startup execution verification.
- Runtime crash analysis and Monkey stress testing are explicitly deferred to Milestone 2 and Milestone 3 per `PROJECT.md`.
- No caveats regarding Milestone 1 baseline build and deployment verification.

---

## 4. Conclusion

Milestone 1 work product meets all integrity standards. Built APK `app-debug.apk` is authentic, package `com.deskdrop.debug` is present and operational on physical device `979116c`, and no integrity violations (facades, false claims, or pre-populated outputs) were found.

**Verdict**: **CLEAN**

---

## 5. Verification Method

To independently re-verify this audit:

1. **Verify APK File**:
   ```bash
   ls -la /Users/chinmayk/Projects/Deskdrop/platforms/android/app/build/outputs/apk/debug/app-debug.apk
   shasum -a 256 /Users/chinmayk/Projects/Deskdrop/platforms/android/app/build/outputs/apk/debug/app-debug.apk
   ```

2. **Verify ADB Package on Physical Device**:
   ```bash
   /opt/homebrew/share/android-commandlinetools/platform-tools/adb -s 979116c shell pm list packages | grep deskdrop
   /opt/homebrew/share/android-commandlinetools/platform-tools/adb -s 979116c shell am start -W -n com.deskdrop.debug/com.deskdrop.MainActivity
   ```

3. **Invalidation Conditions**:
   - APK file missing or missing native libraries `libdeskdrop_core.so`.
   - Device `979116c` does not list `package:com.deskdrop.debug`.
   - Discovered dummy returns or fake pass claims in source code.
