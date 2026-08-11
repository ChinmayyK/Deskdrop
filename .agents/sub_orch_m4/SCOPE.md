# Scope: Milestone M4 (C FFI Export & Swift/WinUI Integration)

## Architecture
- `deskdrop-core/src/ffi.rs`: C FFI exports for Deskdrop Engine.
- `platforms/macos/Deskdrop/DeskdropBridge-Bridging-Header.h`: Swift C FFI bridging header.
- C# WinUI native wrappers or FFI declarations if present under `platforms/windows/`.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 4 | C FFI & Multi-platform Bindings | Add `deskdrop_send_remote_files_response` C FFI export for native desktop apps (macOS Swift, Windows C#) | M4 | PROJECT.md |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M4 | C FFI Export & Swift/WinUI Integration | Expose `deskdrop_send_remote_files_response` in `ffi.rs` and update Swift & C# FFI layers | M1 | DONE |

## Interface Contracts
### C FFI (`ffi.rs`)
- Export function signature:
  `pub unsafe extern "C" fn deskdrop_send_remote_files_response(engine_handle: *mut EngineHandle, request_id: *const c_char, target_device_id: *const c_char, summary_json: *const c_char, files_json: *const c_char, total_matching: u32, error_str: *const c_char) -> i32`
- Behavior: Parse C strings safely (`CStr`), deserialize `summary_json` into `RemoteFilesSummary` (or Option if null/empty), deserialize `files_json` into `Vec<RemoteFileEntry>` (or empty vector), parse `request_id` and `target_device_id` as UUIDs/Strings, parse `error_str` optional error message, then invoke `engine.send_remote_files_response(...)`. Returns 0 on success, negative error code or non-zero on failure.
