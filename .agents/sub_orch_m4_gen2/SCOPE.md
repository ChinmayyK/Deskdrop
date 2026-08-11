# Scope: Milestone M4 (C FFI Export & Swift/WinUI Integration)

## Architecture
- Module: `deskdrop-core/src/ffi.rs`
- Bridging Header: `platforms/macos/Deskdrop/DeskdropBridge-Bridging-Header.h`
- Interfacing Engine method: `Engine::send_remote_files_response`

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 4 | C FFI & Multi-platform Bindings | Add `deskdrop_send_remote_files_response` C FFI export for native desktop apps (macOS Swift, Windows C#) | M4 | PROJECT.md |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M4 | C FFI Export & Swift/WinUI Integration | Expose `deskdrop_send_remote_files_response` in `ffi.rs` and update Swift & C# FFI bridging headers | M1 | IN_PROGRESS |

## Interface Contracts
### C FFI Export (`ffi.rs`)
```c
int deskdrop_send_remote_files_response(
    DeskdropHandle* handle,
    const char* target_device_id,
    const char* request_id,
    const char* summary_json,
    const char* files_json,
    uint32_t total_matching,
    const char* error_str
);
```
- Parameters:
  - `handle`: pointer to `DeskdropHandle` (wrapping `DeskdropEngine` / `EngineHandle`)
  - `target_device_id`: null-terminated string representing target device UUID
  - `request_id`: null-terminated string representing request UUID
  - `summary_json`: null-terminated JSON string for `RemoteFilesSummary` (or NULL if optional / omitted)
  - `files_json`: null-terminated JSON string for `Vec<RemoteFileEntry>` (or NULL / empty array)
  - `total_matching`: u32 total matching count
  - `error_str`: null-terminated error string (or NULL if no error)
- Returns: `0` on success, non-zero error code on invalid handles/null pointers/parse failures.
