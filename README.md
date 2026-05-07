# ClipRelay

**LAN-first · Encrypted · Multi-device clipboard & file relay**

ClipRelay syncs your clipboard and files across all your devices on your local network — privately, without any cloud.

---

## What's new in v0.2

### Device lifecycle model
Every device now has five independent state layers:

| Layer | Meaning |
|---|---|
| **Trust** | Cryptographically approved |
| **Remembered** | Pairing persists across restarts |
| **Connected** | Active TCP session |
| **Sync Enabled** | Clipboard data flows |
| **Auto-connect** | Reconnects on startup / network restore |

**Per-device controls:**
- **Disconnect** — end the session, keep trust
- **Pause Sync** — keep connection, stop clipboard flow
- **Resume Sync** — re-enable clipboard flow
- **Forget Device** — remove pairing, keep trust
- **Revoke Trust** — invalidate trust, disconnect immediately
- **Auto-connect toggle** — control reconnect behaviour

### Human-readable device names
Device names are shown everywhere in the UI. Internal cryptographic IDs (`e72a91b3…`) never appear in normal usage — only in the trust fingerprint dialog.

**Timeline:**
```
[Chinmay's Pixel 8] copied OTP
[MacBook Pro] copied stack trace
```

**Notifications (Android):**
```
📎 File received from Chinmay's Pixel 8
```

### True multi-device mesh
Three or more devices sync simultaneously. Clipboard fanout is peer-aware and deduplication is per-peer so no echo storms occur in large meshes.

### File transfer improvements
- Chunked streaming (no full-file memory load)
- SHA-256 checksum verification before saving
- Progress tracking (bytes transferred / percent)
- Files saved to `Downloads/ClipRelay/`
- Failure recovery (disconnect mid-transfer, timeout)

### Android notification redesign
- **ONE** quiet persistent foreground notification — "ClipRelay · 3 devices connected"
- **No** per-clipboard-sync notifications (clipboard is silent/ambient)
- Notifications only for: file received, trust request, connection alerts
- Optional "notify on remote copy" setting (OFF by default)
- In-app activity feed instead of notification spam

---

## Supported platforms

| Platform | Status |
|---|---|
| macOS | ✅ |
| Android | ✅ |
| Linux | ✅ |
| Windows | ✅ |

---

## Architecture

```
cliprelay-core/   Rust engine — networking, crypto, sync, IPC
cliprelay-cli/    Command-line interface to the daemon
platforms/
  android/        Kotlin foreground service + JNI bridge
  macos/          SwiftUI app + IPC client
  linux/          System service + tray
  windows/        WinForms app + named pipe client
```

### IPC protocol (daemon ↔ UI)
JSON over Unix socket (`~/.run/cliprelay.sock`) or Windows named pipe (`\\.\pipe\cliprelay`).

New commands in v0.2:
- `pause_sync_peer` / `resume_sync_peer`
- `forget_device`
- `set_auto_connect`

### Security
- X25519 ECDH + HKDF key derivation per session
- ChaCha20-Poly1305 AEAD encryption for all messages
- Trust-on-first-use (TOFU) with fingerprint verification
- Trust, revoke, reject are all persistent

---

## Building

```bash
# Core (all platforms)
cargo build --release -p cliprelay-core

# Android
./scripts/build-android.sh

# macOS
./scripts/build-macos.sh
```

---

## License

MIT
