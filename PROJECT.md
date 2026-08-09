# Project: Deskdrop Remote File Query Fix

## Architecture
- Core Engine: Rust crate `deskdrop-core` (`src/engine/mod.rs`, `src/ipc.rs`, `src/protocol.rs`, `src/ffi.rs`, `src/network.rs`).
- Daemons & CLIs: `deskdrop-daemon` (`src/bin/daemon.rs`), `deskdrop-cli` (`src/bin/cli.rs`).
- Native Platforms:
  - macOS: Swift GUI (`platforms/macos/Deskdrop/RemoteExplorerView.swift`), linking `libdeskdrop_core.dylib`.
  - Android: Kotlin app (`platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`, `RemoteFileManager.kt`), JNI `libdeskdrop_core.so`.
  - Windows: WinUI C# app (`platforms/windows/Deskdrop.WinUI/`), linking `deskdrop_core.dll` & IPC named pipes.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Desktop Query Event Handling | Handle `EngineEvent::RemoteFilesQueryReceived` in `daemon.rs` and FFI/engine to scan local filesystem and send `RemoteFilesResponse` | M1 | Survey (Explorer 1 & 2) |
| 2 | Android MediaStore Query Optimization | Add SQL category/MIME-type filtering, indexed count query, and pagination to `RemoteFileManager.kt` to prevent full cursor scans | M2 | Survey (Explorer 1 & 2) |
| 3 | Network & IPC RPC Resilience | Upgrade 12s socket timeout to dynamic/configurable timeout, clear pending waiters on peer disconnect, handle pagination/chunking | M3 | Survey (Explorer 1, 2 & 3) |
| 4 | C FFI & Multi-platform Bindings | Add `deskdrop_send_remote_files_response` C FFI export for native desktop apps (macOS Swift, Windows C#) | M4 | Survey (Explorer 1 & 2) |
| 5 | E2E Automated Verification & Testing | Implement automated test suite exercising remote file queries ("Images" folder) across macOS, Android (hardware device `979116c`), and Windows | M5 | Survey (Explorer 3) |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Desktop Daemon & Core Remote Query Handling | Implement local filesystem scanning & response handling for `EngineEvent::RemoteFilesQueryReceived` in `daemon.rs` & `engine/mod.rs`; clear waiters on peer disconnect | None | DONE |
| M2 | Android MediaStore & Query Optimization | Optimize `RemoteFileManager.kt` & `DeskdropService.kt` with SQL selection filters, fast summary counts, and pagination | None | DONE |
| M3 | RPC Protocol & Dynamic Timeout Hardening | Update `ipc.rs` & `engine/mod.rs` to support configurable timeouts, error fast-path on disconnect, and pagination handling | M1 | DONE |
| M4 | C FFI Export & Swift/WinUI Integration | Expose `deskdrop_send_remote_files_response` in `ffi.rs` and update Swift & C# FFI layers | M1 | DONE |
| M5 | Final E2E Test Suite & Coverage Hardening | Run 100% of E2E tests, pass all tier verification, and perform adversarial coverage hardening | M1, M2, M3, M4 | PLANNED |

## Interface Contracts
### Client IPC ↔ Core Engine (`ipc.rs`)
- Request: `IpcRequest::RemoteFilesQuery { target_device: Uuid, category: Option<RemoteFileCategory>, source: Option<RemoteFileSource>, search_query: Option<String>, offset: u32, limit: u32, timeout_secs: Option<u64> }`
- Response: `IpcResponse::Ok(RemoteFilesResponse)` or `IpcResponse::Err(String)`

### Core Engine ↔ Core Engine Protocol (`protocol.rs`)
- `AppMessage::RemoteFilesQuery { request_id, origin_device, summary_only, category, source, search_query, offset, limit }`
- `AppMessage::RemoteFilesResponse { request_id, summary, files, total_matching, error }`

### Core Engine ↔ C FFI (`ffi.rs`)
- `deskdrop_send_remote_files_response(engine_handle: *mut EngineHandle, request_id: *const c_char, target_device_id: *const c_char, summary_json: *const c_char, files_json: *const c_char, total_matching: u32, error_str: *const c_char) -> i32`

## Code Layout
- `deskdrop-core/src/bin/daemon.rs`: Desktop daemon event loop & RPC request/response handling.
- `deskdrop-core/src/engine/mod.rs`: Engine state, `query_remote_files_sync`, waiter map management.
- `deskdrop-core/src/ipc.rs`: Local IPC server, `IpcRequest::RemoteFilesQuery` handling.
- `deskdrop-core/src/protocol.rs`: Wire protocol enum `AppMessage`.
- `deskdrop-core/src/ffi.rs`: C FFI exports.
- `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt`: Android MediaStore queries.
- `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`: Android service event loop & JNI calls.
- `platforms/macos/Deskdrop/RemoteExplorerView.swift`: macOS SwiftUI remote explorer.
