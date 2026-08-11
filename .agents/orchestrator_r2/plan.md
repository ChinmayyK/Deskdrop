# Project Plan: Deskdrop E2E Exploratory Testing & Bug Resolution (Round 2)

## Architecture & System Overview
- **Android App (`platforms/android`)**: Jetpack Compose UI (`MainActivity`, `PairingActivity`, etc.), `DeskdropService`, JNI bridge (`DeskdropJni.kt` ↔ `libdeskdrop_core.so`).
- **Desktop App / Core (`deskdrop-core`, `platforms/web` / electron / desktop CLI)**: Rust core backend for P2P discovery (mdns/local-net), WebSockets, WebRTC/TCP transfer protocol, desktop/web UI.

## Feature Inventory & Requirements Mapping
| # | Feature / Requirement | Description | Milestone | Source |
|---|----------------------|-------------|-----------|--------|
| 1 | R1, R5: Environment & Infra Survey | Survey devices (`979116c`), desktop daemon, CLI, ADB setup, build configs. | M1 | Survey |
| 2 | R3: UI Navigation & State Verification | Navigate and render all primary UI views (Activity, Transfers, Devices, Settings, Clipboard) on Android & Desktop. | M2 | User Request |
| 3 | R2: Core P2P File-Sharing Capabilities | Verify exchanging text, files, and images across nodes (Android ↔ Desktop). | M3 | User Request |
| 4 | R4: Active Bug Resolution (5 Bug Vectors) | Resolve speed display underflow, UI-thread IP lookup, peer name collision, URI permission forwarding, camera handle race. | M4 | User Request |
| 5 | Acceptance Criteria & Stress Verification | Re-run Monkey test (5000 events) and repeat E2E exchange sequences without crashes or bugs. | M5 | User Request |

## Milestones Breakdown
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Environment & Infrastructure Survey | Discover connected ADB devices (`979116c`), app package `com.deskdrop.debug`, desktop daemon, CLI commands | none | DONE |
| M2 | UI & Settings Verification | Navigate Activity, Transfers, Devices, Settings, Clipboard views via ADB and UI verification | M1 | DONE |
| M3 | Core P2P Exchange Verification | End-to-end P2P sharing tests for text, file, and image payloads between nodes | M1 | DONE |
| M4 | Active Bug Resolution & Hardening | Source code fixes, rebuilds, and verification for 5 bug vectors + Compose focus crash | M2, M3 | DONE |
| M5 | Final E2E Re-verification & Acceptance | Complete Monkey stress test (5000 clean events) and regression test suite | M4 | DONE |

## Interface Contracts
- **Android ADB Interface**: App package `com.deskdrop.debug` / `com.deskdrop`, launcher activity `.MainActivity`.
- **Desktop/CLI Interface**: Desktop binary / web server / CLI for Deskdrop P2P node.
- **P2P Transfer Protocol**: Text payload, file payload, image payload via Deskdrop engine protocol.

## Code Layout
- Android: `platforms/android/`
- Rust Core & CLI/Desktop: `deskdrop-core/`
- Web / Desktop UI: `platforms/web/` or desktop wrappers
