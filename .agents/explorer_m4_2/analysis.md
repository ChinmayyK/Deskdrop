# Investigation Report: C FFI Export & Swift/macOS Integration (Milestone M4)

## Executive Summary
This investigation analyzed the macOS platform bridge headers, Swift codebase (`platforms/macos/Deskdrop/`), build scripts (`scripts/build-macos.sh`), and Rust C FFI implementations (`deskdrop-core/src/ffi.rs`). The objective is to determine the exact requirements for exporting `deskdrop_send_remote_files_response` via C FFI and updating the macOS Swift bridging header.

---

## Key Findings

### 1. macOS Bridging Header Location & Nomenclature
- **Actual Header Path**: `platforms/macos/Deskdrop/DeskdropBridge.h`.
- **Build System Integration**: `scripts/build-macos.sh` (line 67) passes `-import-objc-header "${MACOS_DIR}/${SOURCE_DIR_NAME}/DeskdropBridge.h"` to `swiftc` when building the native macOS bundle and linking against `libdeskdrop_core.dylib`.
- **Note on Scope Reference**: The milestone scope documents refer to `DeskdropBridge-Bridging-Header.h`. The actual file in the codebase is `DeskdropBridge.h`. Any updates for Swift bridging on macOS MUST target `platforms/macos/Deskdrop/DeskdropBridge.h`.

### 2. Existing C FFI Mapping (`DeskdropBridge.h` ↔ `deskdrop-core/src/ffi.rs`)
The C FFI functions for Remote Explorer in `DeskdropBridge.h` currently map to `deskdrop-core/src/ffi.rs` as follows:

| C Function Declaration (`DeskdropBridge.h`) | Rust FFI Export (`deskdrop-core/src/ffi.rs`) | Status |
|--------------------------------------------|---------------------------------------------|--------|
| `deskdrop_send_remote_files_query(...)` | `pub unsafe extern "C" fn deskdrop_send_remote_files_query(...)` | Present in both |
| `deskdrop_send_remote_thumbnail_request(...)` | `pub unsafe extern "C" fn deskdrop_send_remote_thumbnail_request(...)` | Present in both |
| `deskdrop_send_remote_file_pull_request(...)` | `pub unsafe extern "C" fn deskdrop_send_remote_file_pull_request(...)` | Present in both |
| `deskdrop_event_remote_request_id(...)` | `pub unsafe extern "C" fn deskdrop_event_remote_request_id(...)` | Present in both |
| `deskdrop_event_remote_summary_json(...)` | `pub unsafe extern "C" fn deskdrop_event_remote_summary_json(...)` | Present in both |
| `deskdrop_event_remote_files_json(...)` | `pub unsafe extern "C" fn deskdrop_event_remote_files_json(...)` | Present in both |
| `deskdrop_event_remote_total_matching(...)` | `pub unsafe extern "C" fn deskdrop_event_remote_total_matching(...)` | Present in both |
| `deskdrop_event_remote_file_id(...)` | `pub unsafe extern "C" fn deskdrop_event_remote_file_id(...)` | Present in both |
| `deskdrop_event_remote_thumbnail_data(...)` | `pub unsafe extern "C" fn deskdrop_event_remote_thumbnail_data(...)` | Present in both |
| `deskdrop_event_remote_thumbnail_len(...)` | `pub unsafe extern "C" fn deskdrop_event_remote_thumbnail_len(...)` | Present in both |
| `deskdrop_event_remote_error(...)` | `pub unsafe extern "C" fn deskdrop_event_remote_error(...)` | Present in both |
| **`deskdrop_send_remote_files_response(...)`** | **`pub unsafe extern "C" fn deskdrop_send_remote_files_response(...)`** | **MISSING in both** |

### 3. Swift Invocation Patterns (`platforms/macos/Deskdrop/`)
- **IPC Architecture**: The macOS GUI app primarily communicates with the background daemon (`deskdrop-daemon`) via Unix domain socket `/tmp/deskdrop.sock` using JSON IPC messages (`DeskdropIPCClient.swift`).
- **Direct C FFI Mode**: The macOS app and test harnesses can also link `libdeskdrop_core.dylib` directly via `DeskdropBridge.h`. Functions exported in `DeskdropBridge.h` become globally accessible functions in Swift (e.g. `deskdrop_send_remote_files_query(...)`).
- **Data Flow for Responses**: When a node receives `EngineEvent::RemoteFilesQueryReceived`, a desktop application running in direct C FFI mode needs to send back a response using `deskdrop_send_remote_files_response`.

---

## Detailed C FFI Signature Requirements

To implement `deskdrop_send_remote_files_response`, the signature must match the engine parameters defined in `Engine::send_remote_files_response` (`deskdrop-core/src/engine/mod.rs:2033`):

```rust
pub async fn send_remote_files_response(
    &self,
    target_device: Uuid,
    request_id: Uuid,
    summary: Option<RemoteFilesSummary>,
    files: Vec<RemoteFileEntry>,
    total_matching: u32,
    error: Option<String>,
)
```

### C Header Declaration (`platforms/macos/Deskdrop/DeskdropBridge.h`)
Add the following prototype under the `// ── Remote Explorer (Phase 3) ──` section:

```c
/// Send remote files query response back to the requesting remote device.
/// @param handle           Engine handle pointer returned by deskdrop_start.
/// @param request_id       UUID string of the request being responded to (non-null).
/// @param target_device_id UUID string of the remote target device (non-null).
/// @param summary_json     JSON string representing RemoteFilesSummary, or NULL if none.
/// @param files_json       JSON array string representing Vec<RemoteFileEntry>, or NULL/empty.
/// @param total_matching   Count of total matching files.
/// @param error_str        Error message string, or NULL if no error.
/// @return 1 on success, 0 on null/invalid parameters or engine error.
int32_t deskdrop_send_remote_files_response(DeskdropHandle *handle,
                                             const char *request_id,
                                             const char *target_device_id,
                                             const char *summary_json,
                                             const char *files_json,
                                             uint32_t total_matching,
                                             const char *error_str);
```

### Rust FFI Implementation Prototype (`deskdrop-core/src/ffi.rs`)

```rust
#[no_mangle]
pub unsafe extern "C" fn deskdrop_send_remote_files_response(
    handle: *mut DeskdropHandle,
    request_id: *const c_char,
    target_device_id: *const c_char,
    summary_json: *const c_char,
    files_json: *const c_char,
    total_matching: u32,
    error_str: *const c_char,
) -> c_int {
    if handle.is_null() || request_id.is_null() || target_device_id.is_null() {
        return 0;
    }
    let h = &*handle;

    let req_raw = match CStr::from_ptr(request_id).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let req_uuid = match uuid::Uuid::parse_str(req_raw) {
        Ok(u) => u,
        Err(_) => return 0,
    };

    let target_raw = match CStr::from_ptr(target_device_id).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let target_uuid = match uuid::Uuid::parse_str(target_raw) {
        Ok(u) => u,
        Err(_) => return 0,
    };

    let summary_opt = if summary_json.is_null() {
        None
    } else {
        let s = match CStr::from_ptr(summary_json).to_str() {
            Ok(s) => s,
            Err(_) => return 0,
        };
        if s.trim().is_empty() {
            None
        } else {
            serde_json::from_str::<crate::protocol::RemoteFilesSummary>(s).ok()
        }
    };

    let files_vec = if files_json.is_null() {
        Vec::new()
    } else {
        let s = match CStr::from_ptr(files_json).to_str() {
            Ok(s) => s,
            Err(_) => return 0,
        };
        if s.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str::<Vec<crate::protocol::RemoteFileEntry>>(s).unwrap_or_default()
        }
    };

    let error_opt = if error_str.is_null() {
        None
    } else {
        CStr::from_ptr(error_str)
            .to_str()
            .ok()
            .map(|s| s.to_string())
    };

    runtime().block_on(h.engine.send_remote_files_response(
        target_uuid,
        req_uuid,
        summary_opt,
        files_vec,
        total_matching,
        error_opt,
    ));

    1
}
```

---

## Action Plan for Implementer
1. Update `deskdrop-core/src/ffi.rs` by adding `deskdrop_send_remote_files_response` as defined above.
2. Update `platforms/macos/Deskdrop/DeskdropBridge.h` by adding the header prototype.
3. Verify compilation of `deskdrop-core` with `cargo build --package deskdrop-core`.
4. Verify compilation of macOS Swift package with `./scripts/build-macos.sh`.
