# Scope: Milestone M5 — Final E2E Test Suite & Coverage Hardening

## Overview
Milestone M5 verifies 100% pass rate on E2E test suites (Tiers 1-4: 24 Rust integration tests in `deskdrop-core/tests/remote_files_e2e_test.rs` and 3 Python IPC tests in `scripts/test_remote_files_ipc.py`). Subsequently, Phase 2 executes Tier 5 Adversarial Coverage Hardening where Challengers initiate white-box edge case testing, Workers apply fixes if needed, Reviewers review code quality, and Forensic Auditor performs integrity verification.

## Sub-Milestones & Phases
| Phase | Task | Subagents Involved | Status |
|-------|------|-------------------|--------|
| Phase 1 | Run E2E Test Suites (Tiers 1-4) & verify 100% pass | Challenger / Worker | PLANNED |
| Phase 2 | White-box Edge Case Verification (Tier 5: malformed JSON, invalid UUIDs, out-of-bounds offset limits, high-frequency query bursts) | 2 Challengers | PLANNED |
| Phase 2 | Code Remediation (if bugs/gaps exposed) | Worker | PLANNED |
| Phase 2 | Code Quality & Robustness Inspection | 2 Reviewers | PLANNED |
| Phase 2 | Forensic Integrity Verification | 1 Forensic Auditor | PLANNED |
| Phase 3 | Gate Evaluation, PROJECT.md update (M5 status -> DONE), Handoff | Orchestrator | PLANNED |

## Interface Contracts
- Cargo test target: `cargo test -p deskdrop-core --test remote_files_e2e_test`
- Python IPC test script: `python3 scripts/test_remote_files_ipc.py`
- Codebase paths:
  - `deskdrop-core/src/bin/daemon.rs`
  - `deskdrop-core/src/engine/mod.rs`
  - `deskdrop-core/src/ipc.rs`
  - `deskdrop-core/src/protocol.rs`
  - `deskdrop-core/src/ffi.rs`
  - `deskdrop-core/tests/remote_files_e2e_test.rs`
  - `scripts/test_remote_files_ipc.py`
