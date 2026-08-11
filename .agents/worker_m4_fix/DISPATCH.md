## 2026-08-07T01:41:35Z
You are worker_m4_fix working in directory /Users/chinmayk/Projects/Deskdrop/.agents/worker_m4_fix.

Objective: Implement structural source code fixes for the 5 identified bug vectors in Deskdrop Android (`platforms/android`).

Required Inputs:
- Read ORIGINAL_REQUEST.md at: /Users/chinmayk/Projects/Deskdrop/.agents/ORIGINAL_REQUEST.md
- Read UI survey findings at: /Users/chinmayk/Projects/Deskdrop/.agents/explorer_m1_android_ui/handoff.md
- Read PROJECT.md at: /Users/chinmayk/Projects/Deskdrop/PROJECT.md

Scope & Code Fix Requirements:

1. **Bug Vector 1: Transfer Speed Display Underflow (`MainScreen.kt`)**:
   - Location: `platforms/android/app/src/main/java/com/deskdrop/ui/MainScreen.kt` (line ~817).
   - Fix: Format `transfer.speedBps` dynamically (e.g. `B/s`, `KB/s`, `%.1f MB/s`) so sub-MB/s transfer speeds do NOT render as `"0 MB/s"`.

2. **Bug Vector 2: `getLocalIpAddress()` Interface Selection (`SettingsScreen.kt` & `MainScreen.kt`)**:
   - Location: `platforms/android/app/src/main/java/com/deskdrop/ui/SettingsScreen.kt` & `MainScreen.kt`.
   - Fix: Prioritize active Wi-Fi (`wlan0`/`eth0`/`en0`) IPv4 interface over cellular (`rmnet0`) or VPN (`tun0`) interfaces when enumerating IP address.

3. **Bug Vector 3: Peer Snapshot Map Key Collision (`PeerSnapshot.kt`)**:
   - Location: `platforms/android/app/src/main/java/com/deskdrop/PeerSnapshot.kt` (line ~63).
   - Fix: Key `uniquePeers` map by `peer.id` (unique device UUID) rather than `name`: `uniquePeers[peer.id] = peer`.

4. **Bug Vector 4: `DeskdropShareTarget` Multi-File Uri Permission Scope (`DeskdropTileService.kt`)**:
   - Location: `platforms/android/app/src/main/java/com/deskdrop/DeskdropTileService.kt` (or share target handler).
   - Fix: Populate `ClipData` for ALL `sharedUris` in `ACTION_SEND_MULTIPLE` intent forwarding so URI read permissions are granted for every shared file.

5. **Bug Vector 5: Camera Stream Frame JNI Handle Concurrency Guard (`CameraStreamActivity.kt`)**:
   - Location: `platforms/android/app/src/main/java/com/deskdrop/CameraStreamActivity.kt`.
   - Fix: Guard `DeskdropService.activeEngineHandle` checks to prevent race conditions/segfaults if service is stopped while CameraX frame analyzer thread is pushing frames.

Execution Steps:
- Modify source code files using `replace_file_content` / `multi_replace_file_content`.
- Build native libraries and debug APK (`./scripts/build-android.sh --debug --install` or `./gradlew installDebug`).
- Run Rust workspace tests (`cargo test --workspace`) and Android lint/compilation checks to verify zero build errors or test regressions.
- Deploy to hardware device `979116c` and verify functionality.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Output:
Write full change details, build outputs, and handoff report to `/Users/chinmayk/Projects/Deskdrop/.agents/worker_m4_fix/handoff.md`. Include progress log in `/Users/chinmayk/Projects/Deskdrop/.agents/worker_m4_fix/progress.md`.
Message the parent when done.
