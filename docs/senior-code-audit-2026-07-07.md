# Deskdrop Senior Code Audit

Date: 2026-07-07

Scope:
- Rust workspace audit of `deskdrop-core`, `deskdrop-cli`, and `platforms/linux`
- Static review of transfer, settings, and IPC settings-management paths
- Build and test verification on the current workspace state

## Executive Summary

The core Rust workspace was close to healthy, but it had one immediate release-blocking defect and two high-value settings-management issues:

1. The inbound file-transfer finalize path did not compile because `BufWriter<File>` was calling `sync_all()` directly.
2. Persisted settings were not sanitized on load or patch, allowing invalid values such as `clipboard_poll_ms = 0` to leak into runtime behavior.
3. The embedded-engine IPC settings path updated runtime flags inconsistently and did not persist several settings changes to disk, creating restart regressions.

All three issues were fixed in this pass.

## Findings Fixed

### 1. Release-blocking compile failure in file-transfer finalization

Severity: Critical

File:
- `deskdrop-core/src/file_transfer.rs`

Problem:
- `InboundTransfer::finalize()` called `sync_all()` on `BufWriter<File>`, which does not provide that method.
- This broke `cargo test` and `cargo check` for the Rust workspace.

Fix:
- Flush the `BufWriter` and call `sync_all()` on the underlying `File` via `get_ref()`.

Impact:
- Restores successful compilation and keeps the intended durability step before rename.

### 2. Unsafe persisted settings could bypass runtime guardrails

Severity: High

File:
- `deskdrop-core/src/settings.rs`

Problem:
- `SettingsStore::load()` returned parsed settings directly without calling `sanitize()`.
- `SettingsStore::patch()` also accepted post-merge values without sanitizing them.
- This meant hand-edited or malformed settings could leave the app with invalid values such as:
  - `clipboard_poll_ms = 0`
  - out-of-range `history_limit`
  - empty ignore-pattern entries

Fix:
- Sanitize settings during load.
- Sanitize merged settings during patch.
- Persist the sanitized form during save.
- Added regression tests for load-time and patch-time sanitization.

Impact:
- Invalid disk state is normalized before it affects runtime behavior.

### 3. Embedded IPC settings toggles were not reliably persisted

Severity: High

Files:
- `deskdrop-core/src/engine/mod.rs`
- `deskdrop-core/src/ipc.rs`

Problem:
- In the embedded-engine IPC path, several settings operations updated runtime state without persisting to disk.
- `SetTimelineFirstMode` and `SetAutoApplyClipboard` also did not update the canonical in-memory `Settings` snapshot, only the apply policy.
- Result: settings could appear applied until restart, while `GetSettings` could return stale values.

Fix:
- Added a shared settings-persistence helper in the engine.
- Routed `patch_settings`, `save_settings_partial`, `set_sync_enabled`, `set_timeline_first_mode`, and `set_auto_apply_clipboard` through persisted sanitized settings, then reapplied them to runtime state.
- Updated IPC handling to propagate errors from these operations.

Impact:
- Runtime state, IPC responses, and persisted settings now stay aligned in the embedded engine path.

## Verification

Commands run:

```bash
cargo test -p deskdrop-core --quiet
cargo check --workspace --all-targets
```

Results:
- `deskdrop-core` tests passed: 274 passed, 0 failed
- Workspace check passed for:
  - `deskdrop-core`
  - `deskdrop-cli`
  - `deskdrop-linux`

## Remaining Risks

1. Frontend platforms were not fully build-verified in this pass.
   - Android, macOS, and Windows source trees contain local changes and were not compiled here.

2. Settings persistence still relies on the default settings path inside the embedded engine.
   - This is functional now, but a future hardening pass should make the settings path explicit in `EngineConfig`.

3. The workspace still emits `private_interfaces` warnings in `engine/mod.rs`.
   - Not a release blocker, but worth cleaning up to keep the core crate warning-free.

## Recommended Next Improvements

1. Add CI coverage for non-Rust platform wrappers so Android, macOS, and Windows regressions are caught before release.
2. Add a small integration test that exercises embedded IPC settings commands and verifies both persistence and `GetSettings` consistency.
3. Promote the settings file path into `EngineConfig` so embedded runtimes, tests, and portable builds do not rely on implicit defaults.
