# Orchestration Plan — Deskdrop Android Crash Fix

## Objective
Identify, debug, fix, and verify all runtime crashes in the Deskdrop Android application under stress testing (`adb shell monkey`) and maintain background service stability for at least 60 seconds without crashing.

## Strategy & Topology
Project Pattern orchestration:
1. **Phase 0: Survey & Initial Codebase Mapping**
   - Dispatch 3 Explorers (or Spec Miner / Explorers) to inspect `/Users/chinmayk/Projects/Deskdrop`.
   - Explorer 1: Inspect Android project layout, Gradle setup, dependencies, modules, build targets, and existing test setups.
   - Explorer 2: Inspect existing source code components, background services, IPC/network listeners, activity lifecycles, and known crash-prone areas.
   - Explorer 3: Inspect testing tools, ADB environment, emulator/device availability, Monkey runner capability, logcat capture tools.
   - Synthesize survey findings into `PROJECT.md`.

2. **Phase 1: Build Environment Check & Baseline Execution**
   - Dispatch Worker to execute baseline build (`./gradlew assembleDebug` or similar build command).
   - Ensure app builds cleanly and APK can be deployed or verified.

3. **Phase 2: Stress Testing & Crash Reproduction**
   - Dispatch Challenger / Worker with Android stress tools (e.g. `adb shell monkey -p <package> -v 5000` or custom test scripts).
   - Capture full `logcat` outputs, exception stack traces, ANR reports, and reproduction conditions.
   - Catalog all discovered crashes in `CRASH_INVENTORY.md`.

4. **Phase 3: Stack Trace Analysis & Structural Fixes (Iteration Loop per crash / component)**
   - Decompose fixes by crash domain/module.
   - Run Explorer -> Worker -> Reviewer -> Challenger -> Auditor iteration loop.
   - Ensure genuine structural fixes (no hardcoding, no swallowing exceptions without fix, no facade implementations).

5. **Phase 4: Verification under Stress Testing & Acceptance Validation**
   - Re-run full stress test (`adb shell monkey -p <package> -v 5000`).
   - Run background service verification (verify app maintains background service connection for >= 60s without crashing).
   - Reviewer & Forensic Auditor double-check solution integrity and correctness.
   - Report victory claim to Sentinel/Parent.
