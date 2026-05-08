# ClipRelay v0.2 — Change Log

## Architecture Changes

### 1. Complete product rebrand: ClipRelay → ClipRelay
- All user-facing strings, app names, bundle IDs, package names updated
- mDNS service type: `_cliprelay._tcp.local.`
- IPC socket: `cliprelay.sock` / `\\.\pipe\cliprelay`
- Android package: `com.cliprelay`
- macOS bundle: `com.cliprelay`

### 2. Device state refactor (`peer_manager.rs`)
Replaced the binary `trusted == connected` model with a proper five-layer lifecycle:

```rust
pub struct PeerRecord {
    pub id: Uuid,            // internal routing only — never shown in UI
    pub friendly_name: String,
    pub platform: Option<String>,
    pub trusted: bool,       // layer 1 — cryptographic approval
    pub remembered: bool,    // layer 2 — persistent pairing
    pub sync_enabled: bool,  // layer 4 — clipboard data flows
    pub auto_connect: bool,  // layer 5 — reconnect on startup
    pub status: PeerConnectionState, // layer 3 — runtime connection
}
```

New methods:
- `set_sync_enabled()` — Pause / Resume Sync
- `forget_device()` — removes persistent pairing, keeps trust
- `set_auto_connect()` — toggle auto-reconnect
- `is_sync_eligible()` — trusted && sync_enabled
- `should_auto_reconnect()` — trusted && remembered && auto_connect
- `active_senders()` — only sync-eligible connected peers
- `all_connected_senders()` — all connected peers (for heartbeats)

### 3. Reconnection logic (`engine.rs`)
Old: reconnect all trusted peers.  
New: reconnect only `trusted && remembered && auto_connect == true` peers.

### 4. Human-readable device identity (`protocol.rs`, `trust.rs`)
- `DeviceMetadata` struct exchanged during handshake
- `HelloFrame` carries `metadata_json` with platform/version
- `HistoryMetadata.source_device` always a friendly name, never a UUID
- `TrustRecord.effective_name()` returns display_name override or device_name
- UUID visible only in trust fingerprint dialog

### 5. Multi-device mesh dedup (`dedup.rs`)
Replaced global `last_sent` / `last_received` with:
- `sent_hashes: HashSet<ContentHash>` with TTL eviction
- `peer_windows: HashMap<Uuid, PeerWindow>` — per-peer dedup within 5s window
- `should_apply(from_peer, hash)` — peer-aware, suppresses echoes from all peers

### 6. File transfer pipeline (`chunked.rs`, `protocol.rs`)
- SHA-256 checksum included in `ChunkMessage::Start`
- Verified before `ReassemblerOutput::Complete` is emitted
- `ReassemblerOutput::ChecksumMismatch` returned on corruption
- Progress reported as `percent: u8` in `ReassemblerOutput::InProgress`
- Files saved to `Downloads/ClipRelay/` on Android and macOS
- `FileTransferMetadata` wire type for pre-transfer announcement

### 7. IPC protocol (`ipc.rs`)
New commands:
- `PauseSyncPeer { device_id }`
- `ResumeSyncPeer { device_id }`
- `ForgetDevice { device_id }`
- `SetAutoConnect { device_id, enabled }`

### 8. Android notification UX (`ClipRelayService.kt`)
**Before:** notification on every clipboard sync.  
**After:**
- ONE quiet persistent foreground notification (`IMPORTANCE_MIN`)
- Zero per-clipboard notifications
- File received → `IMPORTANCE_DEFAULT` notification
- Trust request → `IMPORTANCE_HIGH` notification
- Clipboard changes → silent activity feed update only
- `notify_on_remote_copy` setting (default: false)
- `activityFeed: ArrayDeque<ActivityEntry>` for in-app history

### 9. macOS UI (`DashboardView.swift`, `ClipRelayStore.swift`)
- Device cards show friendly name + platform icon
- Primary actions: Pause Sync / Disconnect
- Secondary actions (menu): Rename, Auto-connect toggle, Forget, Revoke
- Connection status badge using dot indicator, not UUID text
- Trust dialog: friendly name prominent, fingerprint secondary

## Files Changed
| File | Change |
|---|---|
| `cliprelay-core/src/peer_manager.rs` | Full rewrite — five-layer device model |
| `cliprelay-core/src/dedup.rs` | Mesh-aware per-peer dedup |
| `cliprelay-core/src/protocol.rs` | DeviceMetadata, friendly names in HistoryMetadata |
| `cliprelay-core/src/chunked.rs` | SHA-256 verification, progress percent |
| `cliprelay-core/src/engine.rs` | New lifecycle methods, reconnect logic fix |
| `cliprelay-core/src/ipc.rs` | New device control commands |
| `cliprelay-core/tests/mesh_test.rs` | New mesh + lifecycle tests |
| `platforms/android/…/ClipRelayService.kt` | Notification redesign, activity feed |
| `platforms/macos/…/DashboardView.swift` | Device card with lifecycle controls |
| `platforms/macos/…/ClipRelayModels.swift` | Friendly-name-first view models |
| `platforms/macos/…/ClipRelayStore.swift` | Lifecycle action methods |
| `platforms/macos/…/ClipRelayIPCClient.swift` | New IPC command bindings |
