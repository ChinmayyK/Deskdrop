# Scope: Milestone M1 (Desktop Daemon & Core Remote Query Handling)

## Architecture
- Target Binary / Library: `deskdrop-core/src/bin/daemon.rs` (Desktop Daemon), `deskdrop-core/src/engine/mod.rs` (Core Engine).
- Interface:
  - Protocol message: `AppMessage::RemoteFilesQuery` -> `EngineEvent::RemoteFilesQueryReceived` -> local filesystem scan -> `AppMessage::RemoteFilesResponse`.
  - Disconnect handling: `EngineEvent::PeerDisconnected` -> drain `shared.remote_file_waiters` & notify oneshots with error/fast-fail.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Desktop Daemon Query Event Handling | Handle `EngineEvent::RemoteFilesQueryReceived` in `daemon.rs`: scan local filesystem matching category/source/search_query, build `RemoteFilesSummary` and `Vec<RemoteFileEntry>`, send `AppMessage::RemoteFilesResponse` back to requesting peer | M1 | ORIGINAL_REQUEST & PROJECT.md |
| 2 | Core Engine Waiter Disconnect Cleanup | Update `PeerDisconnected` handler in `engine/mod.rs`: clear pending waiters in `shared.remote_file_waiters` and send error so clients fail fast instead of timing out | M1 | ORIGINAL_REQUEST & PROJECT.md |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Desktop Daemon & Core Remote Query Handling | Local filesystem scanning & response in `daemon.rs` + waiter cleanup on disconnect in `engine/mod.rs` | None | DONE |

## Interface Contracts
### `AppMessage::RemoteFilesQuery` ↔ `AppMessage::RemoteFilesResponse`
- Query params: `request_id`, `origin_device`, `summary_only`, `category`, `source`, `search_query`, `offset`, `limit`.
- Response params: `request_id`, `summary: Option<RemoteFilesSummary>`, `files: Vec<RemoteFileEntry>`, `total_matching: u32`, `error: Option<String>`.

## Code Layout
- `deskdrop-core/src/bin/daemon.rs`: Handle `EngineEvent::RemoteFilesQueryReceived` in event loop.
- `deskdrop-core/src/engine/mod.rs`: Update `PeerDisconnected` event handling to drain `remote_file_waiters`.
