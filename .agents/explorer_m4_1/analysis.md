# Technical Analysis: `deskdrop_send_remote_files_response` C FFI Export

## Executive Summary
This report details the investigation into exporting `deskdrop_send_remote_files_response` in `deskdrop-core/src/ffi.rs` for use by native desktop applications (macOS Swift and Windows WinUI C#).

We examined `deskdrop-core/src/ffi.rs`, `deskdrop-core/src/engine/mod.rs`, `deskdrop-core/src/protocol.rs`, `deskdrop-core/src/jni_android.rs`, `platforms/macos/Deskdrop/DeskdropBridge.h`, and `platforms/windows/Deskdrop.WinUI/Native/NativeCore.cs`.

Key Finding: `Engine` already provides `pub async fn send_remote_files_response(...)` in `engine/mod.rs:2033`. However, `ffi.rs` currently lacks a corresponding C-exported wrapper `deskdrop_send_remote_files_response`. Creating this export will allow native desktop bridge layers (Swift & C#) to respond to incoming `EngineEvent::RemoteFilesQueryReceived` events.

---

## 1. Codebase Investigation Details

### 1.1 Existing FFI Architecture (`deskdrop-core/src/ffi.rs`)
- **Handle Structure**: `DeskdropHandle` (`pub struct DeskdropHandle { engine: Engine, event_rx: ... }`) defined at line 31. Opaque handle passed across FFI boundary as `*mut DeskdropHandle`.
- **Runtime Execution**: Shared Tokio runtime accessed via `runtime()` (`OnceLock<Runtime>`). All FFI calls execute async engine methods using `runtime().block_on(...)`.
- **Safety Handling & Raw Pointers**:
  - `handle`: Validated using `handle.is_null()`. Dereferenced as `&*handle`.
  - Input C strings (`*const c_char`): Validated for null (`ptr.is_null()`). Converted via `unsafe { CStr::from_ptr(ptr) }` and converted to UTF-8 Rust `&str` or `String` via `.to_str()`.
  - UUID Parsing: Input UUID string pointers (`request_id`, `target_device_id`) are parsed using `uuid::Uuid::parse_str(...)`.
- **Return Code Standards**:
  - Remote Explorer FFI functions (`deskdrop_send_remote_files_query`, `deskdrop_send_remote_thumbnail_request`, `deskdrop_send_remote_file_pull_request`) return `c_int` (i32).
  - Return `1` on successful dispatch.
  - Return `0` (or `-1`) on error (null pointer, UTF-8 conversion error, or invalid UUID string).

### 1.2 Core Engine Response Method (`deskdrop-core/src/engine/mod.rs:2033`)
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
- Constructs `AppMessage::RemoteFilesResponse { request_id, summary, files, total_matching, error }`.
- Finds sender channel for `target_device` in `peer_manager.all_connected_senders()` and dispatches the message.

### 1.3 Data Models & Serde Serialization (`deskdrop-core/src/protocol.rs`)
- `RemoteFilesSummary` (lines 268-271):
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, Default)]
  pub struct RemoteFilesSummary {
      pub type_counts: RemoteFileCategoryCounts,
      pub source_counts: RemoteFileSourceCounts,
  }
  ```
- `RemoteFileEntry` (lines 238-248):
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct RemoteFileEntry {
      pub file_id: u64,
      pub display_name: String,
      pub size_bytes: u64,
      pub mime_type: String,
      pub date_modified: u64,
      pub category: RemoteFileCategory,
      pub source: RemoteFileSource,
      pub content_uri: String,
  }
  ```
- **Deserialization Strategy**:
  - `summary_json: *const c_char`:
    - Null pointer or empty string `""` -> `None`.
    - Valid JSON string -> `serde_json::from_str::<RemoteFilesSummary>(s).ok()`.
  - `files_json: *const c_char`:
    - Null pointer or empty string `""` -> `Vec::new()`.
    - Valid JSON string -> `serde_json::from_str::<Vec<RemoteFileEntry>>(s).unwrap_or_default()`.
  - `error_str: *const c_char`:
    - Null pointer or empty string `""` -> `None`.
    - Non-empty UTF-8 string -> `Some(s.to_string())`.

---

## 2. Technical Specification for `deskdrop_send_remote_files_response`

### 2.1 Function Signature
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

### 2.2 Parameter Invariants & Safety Requirements
| Parameter | Type | Required? | Behavior / Safety Check |
|---|---|---|---|
| `handle` | `*mut DeskdropHandle` | Yes | Return `0` if `handle.is_null()`. Dereferenced as `&*handle`. |
| `request_id` | `*const c_char` | Yes | Return `0` if null or invalid UTF-8 or fails `Uuid::parse_str`. |
| `target_device_id` | `*const c_char` | Yes | Return `0` if null or invalid UTF-8 or fails `Uuid::parse_str`. |
| `summary_json` | `*const c_char` | Optional | If null/empty -> `None`; else deserialized via `serde_json::from_str`. |
| `files_json` | `*const c_char` | Optional | If null/empty -> `Vec::new()`; else deserialized via `serde_json::from_str`. |
| `total_matching` | `u32` | Yes | Passed directly to engine method. |
| `error_str` | `*const c_char` | Optional | If null/empty -> `None`; else converted to `Some(String)`. |

### 2.3 C Header Declaration (`DeskdropBridge.h`)
```c
int32_t deskdrop_send_remote_files_response(DeskdropHandle *handle,
                                             const char *request_id,
                                             const char *target_device_id,
                                             const char *summary_json,
                                             const char *files_json,
                                             uint32_t total_matching,
                                             const char *error_str);
```

### 2.4 Windows P/Invoke C# Declaration (`NativeCore.cs`)
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

---

## 3. Implementation Diff Patch

```patch
--- a/deskdrop-core/src/ffi.rs
+++ b/deskdrop-core/src/ffi.rs
@@ -1198,6 +1198,62 @@ pub unsafe extern "C" fn deskdrop_send_remote_file_pull_request(
     1
 }
 
+#[no_mangle]
+pub unsafe extern "C" fn deskdrop_send_remote_files_response(
+    handle: *mut DeskdropHandle,
+    request_id: *const c_char,
+    target_device_id: *const c_char,
+    summary_json: *const c_char,
+    files_json: *const c_char,
+    total_matching: u32,
+    error_str: *const c_char,
+) -> c_int {
+    if handle.is_null() || request_id.is_null() || target_device_id.is_null() {
+        return 0;
+    }
+    let req_raw = match CStr::from_ptr(request_id).to_str() {
+        Ok(s) => s,
+        Err(_) => return 0,
+    };
+    let req_uuid = match uuid::Uuid::parse_str(req_raw) {
+        Ok(u) => u,
+        Err(_) => return 0,
+    };
+    let target_raw = match CStr::from_ptr(target_device_id).to_str() {
+        Ok(s) => s,
+        Err(_) => return 0,
+    };
+    let target_uuid = match uuid::Uuid::parse_str(target_raw) {
+        Ok(u) => u,
+        Err(_) => return 0,
+    };
+
+    let summary: Option<crate::protocol::RemoteFilesSummary> = if summary_json.is_null() {
+        None
+    } else {
+        match CStr::from_ptr(summary_json).to_str() {
+            Ok(s) if !s.is_empty() => serde_json::from_str(s).ok(),
+            _ => None,
+        }
+    };
+
+    let files: Vec<crate::protocol::RemoteFileEntry> = if files_json.is_null() {
+        Vec::new()
+    } else {
+        match CStr::from_ptr(files_json).to_str() {
+            Ok(s) if !s.is_empty() => serde_json::from_str(s).unwrap_or_default(),
+            _ => Vec::new(),
+        }
+    };
+
+    let err_opt = if error_str.is_null() {
+        None
+    } else {
+        match CStr::from_ptr(error_str).to_str() {
+            Ok(s) if !s.is_empty() => Some(s.to_string()),
+            _ => None,
+        }
+    };
+
+    let h = &*handle;
+    runtime().block_on(h.engine.send_remote_files_response(
+        target_uuid,
+        req_uuid,
+        summary,
+        files,
+        total_matching,
+        err_opt,
+    ));
+    1
+}
```
