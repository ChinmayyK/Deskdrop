# Investigation Report: Windows WinUI Code & Cross-Platform FFI Header/Wrapper Requirements for `deskdrop_send_remote_files_response`

## Executive Summary
This investigation analyzed the Windows WinUI codebase (`platforms/windows/`), cross-platform C bridging headers (`platforms/macos/Deskdrop/DeskdropBridge.h`), and C FFI implementations (`deskdrop-core/src/ffi.rs`) to determine the exact binding requirements for `deskdrop_send_remote_files_response` and general Remote Explorer operations.

Key Discovery: Windows WinUI supports two integration mechanisms: Named-Pipe IPC via `DaemonClient` (`WindowsIpcClient.cs`) and direct P/Invoke FFI via `NativeCore` (`NativeCore.cs`). `NativeCore.cs` currently lacks P/Invoke declarations for `deskdrop_send_remote_files_response` as well as all other Remote Explorer FFI functions and event constants (30-37). Similarly, `DeskdropBridge.h` lacks `deskdrop_send_remote_files_response`.

---

## 1. Observation

### 1.1 Windows Codebase Architecture (`platforms/windows/`)
- **Native P/Invoke Bridge (`platforms/windows/Deskdrop.WinUI/Native/NativeCore.cs`)**:
  - Contains `[DllImport("deskdrop_core", CallingConvention = CallingConvention.Cdecl)]` declarations for Rust C FFI functions.
  - String parameters use `[MarshalAs(UnmanagedType.LPUTF8Str)] string`.
  - Defines event constants `PB_EVENT_*` from `0` to `26` (`PB_EVENT_SYSTEM_HEALTH_UPDATED = 26`).
  - **Lacks** event constants `PB_EVENT_REMOTE_FILES_QUERY` (30) through `PB_EVENT_REMOTE_FILE_ACTION_REQUEST` (37).
  - **Lacks** P/Invoke declaration for `deskdrop_send_remote_files_response`.
  - **Lacks** P/Invoke declarations for `deskdrop_send_remote_files_query`, `deskdrop_send_remote_thumbnail_request`, `deskdrop_send_remote_file_pull_request`.
  - **Lacks** P/Invoke event accessors: `deskdrop_event_remote_request_id`, `deskdrop_event_remote_summary_json`, `deskdrop_event_remote_files_json`, `deskdrop_event_remote_total_matching`, `deskdrop_event_remote_file_id`, `deskdrop_event_remote_thumbnail_data`, `deskdrop_event_remote_thumbnail_len`, `deskdrop_event_remote_error`.

- **IPC Client (`platforms/windows/Deskdrop.WinUI/WindowsIpcClient.cs`)**:
  - Class `DaemonClient` handles Named-Pipe IPC (`\\.\pipe\deskdrop_<LOCALAPPDATA_hash>`) with `deskdrop-daemon`.
  - Already contains IPC helper methods: `RemoteFilesQueryAsync`, `RemoteFilePullRequest`, `RemoteFileActionRequest`.

### 1.2 Cross-Platform C Header (`platforms/macos/Deskdrop/DeskdropBridge.h`)
- Serves as the C FFI header for `deskdrop-core`.
- Defines Remote Explorer event codes (`PB_EVENT_REMOTE_FILES_QUERY` (30) to `PB_EVENT_REMOTE_THUMBNAIL_RESPONSE` (34)).
- Declares `deskdrop_send_remote_files_query`, `deskdrop_send_remote_thumbnail_request`, `deskdrop_send_remote_file_pull_request`, and event accessors.
- **Lacks** function declaration for `deskdrop_send_remote_files_response`.

### 1.3 C FFI Rust Layer (`deskdrop-core/src/ffi.rs`)
- Implements `#[no_mangle] pub unsafe extern "C" fn` exports for C-compatible interface.
- Contains `deskdrop_send_remote_files_query` (lines 1036-1125), `deskdrop_send_remote_thumbnail_request` (lines 1128-1163), `deskdrop_send_remote_file_pull_request` (lines 1166-1198).
- **Lacks** implementation for `deskdrop_send_remote_files_response`.

---

## 2. Logic Chain

1. **Observation**: Native Windows applications linking directly to `deskdrop_core.dll` rely on P/Invoke declarations in `NativeCore.cs`.
2. **Observation**: When a peer node sends a `RemoteFilesQuery`, the engine generates an event `EngineEvent::RemoteFilesQueryReceived` (`PB_EVENT_REMOTE_FILES_QUERY` = 30).
3. **Logic Step**: To respond to this query in direct FFI mode (without `deskdrop-daemon`), the Windows desktop host must invoke `deskdrop_send_remote_files_response`.
4. **Observation**: `NativeCore.cs` does not currently import `deskdrop_send_remote_files_response` or any other Remote Explorer P/Invoke functions.
5. **Logic Step**: Therefore, `NativeCore.cs` must be updated with the P/Invoke method signature for `deskdrop_send_remote_files_response` and the full suite of Remote Explorer functions & event accessors to ensure parity with `deskdrop-core/src/ffi.rs`.
6. **Observation**: C/C++ applications or Swift code using `DeskdropBridge.h` need a C prototype declaration for `deskdrop_send_remote_files_response`.
7. **Logic Step**: Therefore, `DeskdropBridge.h` must also be updated with the C function signature for `deskdrop_send_remote_files_response`.

---

## 3. Specific Binding Specifications

### 3.1 Windows P/Invoke Additions for `NativeCore.cs`

#### A. Event Constants
```csharp
public const int PB_EVENT_PEER_DISCOVERED = 27;
public const int PB_EVENT_NETWORK_STATE_CHANGED = 28;
public const int PB_EVENT_OUTGOING_PAIRING_WAITING = 29;
public const int PB_EVENT_REMOTE_FILES_QUERY = 30;
public const int PB_EVENT_REMOTE_THUMBNAIL_REQUEST = 31;
public const int PB_EVENT_REMOTE_FILE_PULL_REQUEST = 32;
public const int PB_EVENT_REMOTE_FILES_RESPONSE = 33;
public const int PB_EVENT_REMOTE_THUMBNAIL_RESPONSE = 34;
public const int PB_EVENT_SPEED_TEST_PROGRESS = 35;
public const int PB_EVENT_SPEED_TEST_COMPLETE = 36;
public const int PB_EVENT_REMOTE_FILE_ACTION_REQUEST = 37;
```

#### B. Primary Target Signature: `deskdrop_send_remote_files_response`
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

#### C. Supporting Remote Explorer P/Invoke Methods
```csharp
[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern int deskdrop_send_remote_files_query(
    IntPtr handle,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string targetDeviceId,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string requestId,
    int summaryOnly,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string? category,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string? source,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string? searchQuery,
    uint offset,
    uint limit);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern int deskdrop_send_remote_thumbnail_request(
    IntPtr handle,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string targetDeviceId,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string requestId,
    ulong fileId,
    uint sizePx);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern int deskdrop_send_remote_file_pull_request(
    IntPtr handle,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string targetDeviceId,
    [MarshalAs(UnmanagedType.LPUTF8Str)] string requestId,
    ulong fileId);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern IntPtr deskdrop_event_remote_request_id(IntPtr ev);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern IntPtr deskdrop_event_remote_summary_json(IntPtr ev);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern IntPtr deskdrop_event_remote_files_json(IntPtr ev);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern uint deskdrop_event_remote_total_matching(IntPtr ev);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern ulong deskdrop_event_remote_file_id(IntPtr ev);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern IntPtr deskdrop_event_remote_thumbnail_data(IntPtr ev);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern UIntPtr deskdrop_event_remote_thumbnail_len(IntPtr ev);

[DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
public static extern IntPtr deskdrop_event_remote_error(IntPtr ev);
```

### 3.2 C Header Update for `DeskdropBridge.h`
```c
int32_t deskdrop_send_remote_files_response(DeskdropHandle *handle,
                                             const char *request_id,
                                             const char *target_device_id,
                                             const char *summary_json,
                                             const char *files_json,
                                             uint32_t total_matching,
                                             const char *error_str);
```

---

## 4. Caveats
- Windows WinUI app primarily uses IPC (`DaemonClient`) when running alongside `deskdrop-daemon`. P/Invoke in `NativeCore.cs` is used when running in embedded library mode (`deskdrop_core.dll`).
- UTF-8 string marshalling: `[MarshalAs(UnmanagedType.LPUTF8Str)]` requires .NET 4.7+ / .NET Core / .NET 8 (which `Deskdrop.WinUI` uses via `net8.0-windows`).

---

## 5. Conclusion
To fully support `deskdrop_send_remote_files_response` across Windows and cross-platform native integrations:
1. Export `deskdrop_send_remote_files_response` in `deskdrop-core/src/ffi.rs`.
2. Add prototype to `platforms/macos/Deskdrop/DeskdropBridge.h`.
3. Add P/Invoke `[DllImport]` declaration and Remote Explorer event constants to `platforms/windows/Deskdrop.WinUI/Native/NativeCore.cs`.

---

## 6. Verification Method
- Run `cargo check -p deskdrop-core` to ensure FFI exports in `ffi.rs` build without errors.
- Verify `DeskdropBridge.h` compiles cleanly with Swift bridging headers.
- Inspect `NativeCore.cs` to confirm method signatures and calling conventions match `ffi.rs`.
