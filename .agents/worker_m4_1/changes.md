# Changes Log — Worker M4-1

## Summary of Changes

### 1. `deskdrop-core/src/ffi.rs`
- Implemented `deskdrop_send_remote_files_response` C FFI export with `#[no_mangle] pub unsafe extern "C" fn`:
  - Signature: `(handle: *mut DeskdropHandle, request_id: *const c_char, target_device_id: *const c_char, summary_json: *const c_char, files_json: *const c_char, total_matching: u32, error_str: *const c_char) -> c_int`
  - Validates null handle/request_id/target_device_id pointers (returns `0` on null or invalid UUID strings).
  - Deserializes optional `summary_json` into `Option<RemoteFilesSummary>` via `serde_json::from_str`.
  - Deserializes optional `files_json` into `Vec<RemoteFileEntry>` via `serde_json::from_str`.
  - Converts optional `error_str` into `Option<String>`.
  - Executes `runtime().block_on(h.engine.send_remote_files_response(...))` and returns `1` on success.
- Added comprehensive unit tests in `mod tests`:
  - `test_send_remote_files_response_null_inputs`: verifies return code `0` on null handle, request_id, and target_device_id pointers.
  - `test_send_remote_files_response_invalid_uuid`: verifies return code `0` on malformed UUID strings.
  - `test_send_remote_files_response_valid`: verifies return code `1` on valid calls with null and non-null JSON/error arguments using isolated test engine handles.

### 2. `platforms/macos/Deskdrop/DeskdropBridge.h`
- Added Objective-C / C bridging header prototype declaration:
  ```c
  int32_t deskdrop_send_remote_files_response(DeskdropHandle *handle,
                                               const char *request_id,
                                               const char *target_device_id,
                                               const char *summary_json,
                                               const char *files_json,
                                               uint32_t total_matching,
                                               const char *error_str);
  ```

### 3. `platforms/windows/Deskdrop.WinUI/Native/NativeCore.cs`
- Added Remote Explorer event constants `PB_EVENT_REMOTE_FILES_QUERY` (30), `PB_EVENT_REMOTE_THUMBNAIL_REQUEST` (31), `PB_EVENT_REMOTE_FILE_PULL_REQUEST` (32), `PB_EVENT_REMOTE_FILES_RESPONSE` (33), `PB_EVENT_REMOTE_THUMBNAIL_RESPONSE` (34), and `PB_EVENT_REMOTE_FILE_ACTION_REQUEST` (37).
- Added `[DllImport]` declaration for `deskdrop_send_remote_files_response`:
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
- Added missing Remote Explorer P/Invoke helper method declarations and event accessors (`deskdrop_send_remote_files_query`, `deskdrop_send_remote_thumbnail_request`, `deskdrop_send_remote_file_pull_request`, `deskdrop_event_remote_request_id`, `deskdrop_event_remote_summary_json`, `deskdrop_event_remote_files_json`, `deskdrop_event_remote_total_matching`, `deskdrop_event_remote_file_id`, `deskdrop_event_remote_thumbnail_data`, `deskdrop_event_remote_thumbnail_len`, `deskdrop_event_remote_error`).
