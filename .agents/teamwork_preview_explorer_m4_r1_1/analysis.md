# Technical Analysis: C FFI Export & Native Platform Integration for Milestone M4

## 1. Executive Summary

Milestone M4 requires exposing the C FFI function `deskdrop_send_remote_files_response` in `deskdrop-core/src/ffi.rs` and ensuring native platform bridging headers and bindings (`DeskdropBridge.h` for macOS Swift and `NativeCore.cs` for Windows WinUI C#) are fully specified and aligned with the Rust `Engine` layer.

Investigation reveals that `deskdrop_send_remote_files_response` is **fully implemented and exported** in `deskdrop-core/src/ffi.rs` (lines 1201–1267), and the corresponding declarations are **already present** in `platforms/macos/Deskdrop/DeskdropBridge.h` (lines 113–119) and `platforms/windows/Deskdrop.WinUI/Native/NativeCore.cs` (lines 151–158).

This document provides a detailed breakdown of the interface contract, parameter types, error handling, thread safety, memory management, protocol data models, and native bridging headers.

---

## 2. Codebase Investigation & Implementation Details

### 2.1 C FFI Export (`deskdrop-core/src/ffi.rs:1201`)

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
) -> c_int
```

#### Parameter Contract & Types

| Parameter | C Type | Rust Type | Description / Constraints |
|---|---|---|---|
| `handle` | `DeskdropHandle*` | `*mut DeskdropHandle` | Opaque handle pointer returned by `deskdrop_start`. Must not be NULL. |
| `request_id` | `const char*` | `*const c_char` | Null-terminated UTF-8 UUID string for request ID. Must not be NULL; must parse via `Uuid::parse_str`. |
| `target_device_id` | `const char*` | `*const c_char` | Null-terminated UTF-8 UUID string for target device ID. Must not be NULL; must parse via `Uuid::parse_str`. |
| `summary_json` | `const char*` | `*const c_char` | Null-terminated JSON string for `RemoteFilesSummary`. Optional (NULL or empty string parses to `None`). |
| `files_json` | `const char*` | `*const c_char` | Null-terminated JSON array string for `Vec<RemoteFileEntry>`. Optional (NULL or empty string parses to `Vec::new()`). |
| `total_matching` | `uint32_t` | `u32` | Total matching count for pagination/summary display. |
| `error_str` | `const char*` | `*const c_char` | Null-terminated error message string. Optional (NULL or empty string parses to `None`). |

#### Return Value
- `1` (`c_int`): Success. The request was valid and handed off to the async Tokio runtime for network dispatch.
- `0` (`c_int`): Validation error (NULL handle, NULL pointers for mandatory strings, or invalid UUID format).

---

### 2.2 Safety, Parsing & Exception Propagation

1. **Pointer Guard**:
   ```rust
   if handle.is_null() || request_id.is_null() || target_device_id.is_null() {
       return 0;
   }
   ```
2. **C String & UUID Parsing**:
   - Uses `std::ffi::CStr::from_ptr(ptr).to_str()` to extract UTF-8 slices safely without panicking on invalid UTF-8 bytes.
   - Parses UUID strings via `uuid::Uuid::parse_str`. Invalid UUIDs return `0` immediately.
3. **JSON Deserialization**:
   - `summary_json` is parsed into `Option<crate::protocol::RemoteFilesSummary>` using `serde_json::from_str`. If parsing fails or input is empty, it falls back gracefully to `None`.
   - `files_json` is parsed into `Vec<crate::protocol::RemoteFileEntry>` using `serde_json::from_str`. If parsing fails or input is empty, it falls back gracefully to `Vec::new()`.
   - Neither invalid JSON syntax nor unexpected JSON schema causes a Rust panic.
4. **Tokio Runtime Dispatch**:
   - Obtains the global Tokio runtime singleton (`runtime()`).
   - Executes `h.engine.send_remote_files_response(target_uuid, req_uuid, summary, files, total_matching, err_opt)` synchronously via `runtime().block_on(...)`.
5. **Memory Safety & Ownership**:
   - Input strings (`request_id`, `target_device_id`, `summary_json`, `files_json`, `error_str`) are owned by the caller. Rust reads them transiently and does not attempt to deallocate them.

---

### 2.3 Core Engine Interaction (`deskdrop-core/src/engine/mod.rs:2036`)

The underlying engine method signature is:

```rust
pub async fn send_remote_files_response(
    &self,
    target_device: Uuid,
    request_id: Uuid,
    summary: Option<crate::protocol::RemoteFilesSummary>,
    files: Vec<crate::protocol::RemoteFileEntry>,
    total_matching: u32,
    error: Option<String>,
)
```

The method constructs an `AppMessage::RemoteFilesResponse`:
```rust
let msg = AppMessage::RemoteFilesResponse {
    request_id,
    summary,
    files,
    total_matching,
    error,
};
```
It looks up `target_device` among connected P2P channels (`self.shared.peer_manager.all_connected_senders()`) and asynchronously transmits `msg` across the P2P connection.

---

### 2.4 Data Models (`deskdrop-core/src/protocol.rs`)

#### `RemoteFilesSummary` (line 268)
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteFilesSummary {
    pub type_counts: RemoteFileCategoryCounts,
    pub source_counts: RemoteFileSourceCounts,
}
```

#### `RemoteFileEntry` (line 239)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFileEntry {
    pub file_id: u64,         // MediaStore _ID or file identifier
    pub display_name: String, // _DISPLAY_NAME
    pub size_bytes: u64,      // _SIZE
    pub mime_type: String,    // MIME_TYPE
    pub date_modified: u64,   // epoch seconds
    pub category: RemoteFileCategory,
    pub source: RemoteFileSource,
    pub content_uri: String, // Content URI or local path
}
```

---

## 3. Platform Bridging Headers & Native Bindings

### 3.1 macOS C Header (`platforms/macos/Deskdrop/DeskdropBridge.h:113-119`)

```c
int32_t deskdrop_send_remote_files_response(DeskdropHandle *handle,
                                             const char *request_id,
                                             const char *target_device_id,
                                             const char *summary_json,
                                             const char *files_json,
                                             uint32_t total_matching,
                                             const char *error_str);
```

#### Swift Integration Usage Pattern
In Swift, this C function imports directly via the bridging header as:
```swift
let result = deskdrop_send_remote_files_response(
    handle,
    requestIdString,
    targetDeviceIdString,
    summaryJsonString,
    filesJsonString,
    totalMatching,
    errorString
)
```

### 3.2 Windows WinUI C# P/Invoke (`platforms/windows/Deskdrop.WinUI/Native/NativeCore.cs:151-158`)

```csharp
[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern int deskdrop_send_remote_files_response(
    IntPtr handle,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string requestId,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string targetDeviceId,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string? summaryJson,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string? filesJson,
    uint totalMatching,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string? errorStr);
```

#### C# Integration Usage Pattern
```csharp
int result = NativeCore.deskdrop_send_remote_files_response(
    handle,
    requestId.ToString(),
    targetDeviceId.ToString(),
    summaryJson,
    filesJson,
    totalMatching,
    errorStr
);
```

---

## 4. Parameter Order Matrix & Alignment Check

| Source | Parameter 1 | Parameter 2 | Parameter 3 | Parameter 4 | Parameter 5 | Parameter 6 | Parameter 7 |
|---|---|---|---|---|---|---|---|
| **`ffi.rs`** | `handle` | `request_id` | `target_device_id` | `summary_json` | `files_json` | `total_matching` | `error_str` |
| **`DeskdropBridge.h`** | `handle` | `request_id` | `target_device_id` | `summary_json` | `files_json` | `total_matching` | `error_str` |
| **`NativeCore.cs`** | `handle` | `requestId` | `targetDeviceId` | `summaryJson` | `filesJson` | `totalMatching` | `errorStr` |
| **`PROJECT.md`** | `engine_handle` | `request_id` | `target_device_id` | `summary_json` | `files_json` | `total_matching` | `error_str` |
| **`SCOPE.md`** (text table) | `handle` | `target_device_id` | `request_id` | `summary_json` | `files_json` | `total_matching` | `error_str` |

> **Alignment Note**: All actual codebase files (`ffi.rs`, `DeskdropBridge.h`, `NativeCore.cs`, `PROJECT.md`) use `request_id` as parameter 2 and `target_device_id` as parameter 3. This ABI order is consistent across all platform layers.

---

## 5. Implementation Guide & Verification Protocol

### Verification Suite
1. **Rust Library Unit Tests**:
   ```bash
   cargo test -p deskdrop-core --lib ffi::tests::test_send_remote_files_response
   ```
2. **Empirical Challenge & Integration Test**:
   ```bash
   cargo test --test ffi_m4_challenge_test
   ```
   Tests include:
   - `test_null_pointers`: Validates return code `0` on NULL handle/UUID strings and return code `1` on NULL optional strings.
   - `test_invalid_uuid_strings`: Verifies rejection of empty or malformed UUIDs.
   - `test_empty_json_strings`: Verifies safe fallback for empty JSON inputs.
   - `test_invalid_json_strings`: Verifies robust error suppression and safe execution when malformed JSON is passed.
   - `test_non_empty_error_strings`: Tests transmission of error strings.
   - `test_large_file_lists`: Tests stress performance with 5,000 file entries.
   - `test_special_characters_in_json`: Validates UTF-8 multi-byte characters, emojis, quotes, and newlines.
