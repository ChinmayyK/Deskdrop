# Master Plan — Deskdrop Remote File Query Fix

## Objective
Diagnose and permanently resolve the "Connection Interrupted - Remote files query timed out" issue occurring during remote file browsing in Deskdrop across macOS, Windows, and Android platform combinations.

## Strategy & Workflow
Following the Project Orchestration Pattern:
1. **Phase 0: Survey & Discovery**
   - Dispatch 3 parallel Explorers to inspect the codebase structure, remote file browsing mechanism, network protocol/RPC, timeout logic, and device control setup.
   - Aggregate findings into `PROJECT.md` with Feature Inventory, Architecture, Code Layout, and Milestones.

2. **Phase 1: E2E Testing Track**
   - Dispatch E2E test setup to build automated/repeatable verification scripts exercising remote file listing across platforms.
   - Publish `TEST_READY.md`.

3. **Phase 2: Implementation Track**
   - Milestone M1: Root cause fix for remote file query timeout / network protocol reliability.
   - Milestone M2: Handling large directory payloads / chunking / async streaming / UI response handling.
   - Milestone M3: Cross-platform compatibility hardening (macOS, Windows, Android).

4. **Phase 3: Verification & Auditing**
   - Execute E2E test suite.
   - Reviewer approval, Challenger empirical verification, and Forensic Audit (`teamwork_preview_auditor`) gate verification.

5. **Phase 4: Final Acceptance & Reporting**
   - Verify success criteria: browsing remote "Images" folder completes reliably within latency limits without timing out.
   - Synthesize report and present victory.
