# Deskdrop Cross-OS Connectivity, Auto-Reconnect, and Feature Audit

Date: May 27, 2026

## Scope

This audit focuses specifically on cross-OS connectivity behavior and feature functionality:

- whether previously trusted devices reconnect automatically
- whether startup, boot/login, and network recovery behavior is reliable on each OS
- whether discovery, manual connect, trust, forget, pause, and recovery controls are actually wired
- where platform shells drift from the shared core connectivity contract

This review is based on the current code in `deskdrop-core`, `platforms/android`, `platforms/macos`, `platforms/windows`, `platforms/linux`, and `deskdrop-cli`.

## Executive Summary

The shared core reconnect model is good and explicit.

A previously trusted device is supposed to reconnect automatically, but only when all of the following remain true:

- the device is still `trusted`
- the device is still `remembered`
- `auto_connect` is still enabled
- the user did not explicitly disconnect it

That behavior is implemented centrally in `deskdrop-core/src/peer_manager.rs` and enforced again by the watchdog and network-change reconnect logic in `deskdrop-core/src/engine.rs`.

The platform verdict is:

- Android has the strongest runtime fault tolerance for staying alive and recovering after network loss.
- macOS has the strongest user-facing management surface for reconnectability and device control.
- Linux has a solid service/daemon story, but a thin recovery and management surface.
- Windows is the weakest platform overall, not because the core cannot reconnect, but because multiple UI recovery and settings actions are wired to unsupported or stale IPC commands.

If the question is "does Deskdrop automatically reconnect previously trusted devices across all OSes?", the answer is:

- Yes in the shared core design.
- Yes in practice on Android and macOS when the service/app is running.
- Yes on Linux when the daemon is installed and running.
- Mostly yes in the Windows engine while the tray app is running, but Windows recovery and control surfaces are currently not trustworthy enough to call the full experience robust.

## The Actual Reconnect Contract

The most important truth in the codebase is this:

- `PeerRecord::should_auto_reconnect()` returns true only when `trusted && remembered && auto_connect && !explicit_disconnect`.
  - `deskdrop-core/src/peer_manager.rs`

That creates a very clear lifecycle:

1. Trusting a device enables future reconnects.
   - `Engine::trust_peer()` marks the peer trusted and sets `auto_connect = true`.
   - `deskdrop-core/src/engine.rs`

2. Forgetting a device disables future reconnects without revoking trust.
   - `forget_device()` sets `remembered = false` and `auto_connect = false`.
   - The active session is then shut down so it does not come back automatically.
   - `deskdrop-core/src/peer_manager.rs`
   - `deskdrop-core/src/engine.rs`

3. Explicit disconnect intentionally suppresses auto-reconnect.
   - `disconnect_peer()` marks the peer as explicitly disconnected.
   - That blocks the watchdog from reconnecting it until a fresh explicit reconnect happens.
   - `deskdrop-core/src/peer_manager.rs`
   - `deskdrop-core/src/engine.rs`

4. A fresh successful connection clears explicit disconnect state.
   - `connect_once()` calls `set_explicit_disconnect(peer_id, false)` after the session is established.
   - `deskdrop-core/src/engine.rs`

5. Reconnect attempts happen in two ways.
   - The watchdog runs every 10 seconds and retries eligible offline peers with per-peer rate limiting.
   - Network-change handling also triggers reconnect of known peers.
   - `deskdrop-core/src/engine.rs`

6. There is also a continuity shortcut for identity churn.
   - If a new device ID appears with the same friendly name and same IP as a previously trusted peer, the engine may auto-trust it.
   - This is mainly there to smooth Android debug/reinstall flows.
   - `deskdrop-core/src/engine.rs`

This is a strong foundation. The main cross-OS problem is not the rule itself. The main problem is whether each platform actually starts, survives, exposes, and explains that rule properly.

## Cross-OS Matrix

| Capability | macOS | Android | Windows | Linux |
| :--- | :---: | :---: | :---: | :---: |
| Previously trusted device auto-reconnects while app/service is running | ✅ | ✅ | ✅ | ✅ |
| Explicit disconnect suppresses future auto-reconnect | ✅ | ✅ | ✅ | ✅ |
| Forget device prevents future auto-reconnect | ✅ | ✅ | Core yes, surface limited | ✅ |
| Auto-start after boot/login available | ✅ | ✅ | ✅ | ✅ |
| Manual connect available | ✅ | ✅ | ⚠️ UI wiring issue | ✅ |
| Manual rescan available | ✅ | ✅ | ⚠️ UI wiring issue | ⚠️ no clear CLI rescan |
| Per-device auto-connect toggle exposed | ✅ | ❌ | ❌ | ❌ |
| Per-device pause/resume sync exposed | ✅ | ❌ | ❌ | ⚠️ CLI peer settings only |
| Pairing request / pairing response flow exposed | ✅ | ✅ | ❌ | ❌ |
| Diagnostics/recovery surface is real, not cosmetic | ✅ | ✅ | ❌ | ❌ |

## Platform Findings

### macOS

macOS currently has the strongest desktop connectivity surface.

What works well:

- The app starts and supervises the daemon.
  - `AppDelegate.startDaemonIfNeeded()` and `ensureDaemonResponsive()` ping IPC and relaunch the daemon if needed.
  - `platforms/macos/Deskdrop/AppDelegate.swift`
- Launch at login exists.
  - `applyLoginItemState(enabled:)` uses `SMAppService.mainApp`.
  - `platforms/macos/Deskdrop/DeskdropStore.swift`
- The UI fully understands reconnectable peer state.
  - Status logic distinguishes connected, reconnecting, and trusted-offline peers.
  - `whenStatusLine(...)` returns `"Trusted devices ready to reconnect"` when applicable.
  - `platforms/macos/Deskdrop/DeskdropStore.swift`
- The platform exposes the important controls:
  - manual connect
  - rescan
  - forget
  - revoke trust
  - pause/resume sync
  - toggle auto-connect
  - pairing request / pairing response
  - `platforms/macos/Deskdrop/DeskdropStore.swift`
  - `platforms/macos/Deskdrop/DeskdropIPCClient.swift`

Gaps:

- The reconnect state is visible, but still coarse.
  - The UI knows a device is reconnectable, but not why it is offline.
  - There is still room to surface stronger reason states like:
    - trusted but explicitly disconnected
    - trusted but network mismatch
    - trusted but waiting on peer
    - trust revoked
- The experience still depends on the app or login item being active.
  - The reconnect model is strong once Deskdrop is running, but it is not a system daemon independent of user login.

Verdict:

- Best overall desktop implementation.
- Closest to the intended product promise.

### Android

Android currently has the strongest runtime survivability and recovery behavior in the repo.

What works well:

- Auto-start after reboot is implemented.
  - `BootReceiver` starts the foreground service on `BOOT_COMPLETED`, `LOCKED_BOOT_COMPLETED`, and `MY_PACKAGE_REPLACED`, gated by `sync_enabled`.
  - `platforms/android/app/src/main/java/com/deskdrop/BootReceiver.kt`
- The service is intentionally sticky and resilient.
  - `DeskdropService` uses `START_STICKY`.
  - `onTaskRemoved()` schedules a restart through `AlarmManager`.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`
- NSD discovery is actively managed.
  - The service advertises and browses through Android NSD.
  - It restarts discovery on network changes.
  - It keeps a multicast lock so mDNS traffic is not silently filtered by OEM Wi-Fi stacks.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`
- Android explicitly bridges network recovery into the core reconnect path.
  - `registerNetworkCallback()` calls `restartDiscoveryNow()` and `DeskdropJni.notifyNetworkRestored(...)` when the default network becomes available.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`
- Manual connect, trust, reject, forget, pairing request, and pairing response are all present.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt`

Important gaps:

- Android parses reconnectability but does not really expose control over it.
  - `PeerSnapshot` includes `remembered` and `autoConnect`, and computes `isReconnectable`.
  - But the UI mostly compresses trusted offline devices to `"Offline"` and only exposes `Forget Device`.
  - There is no user-facing auto-connect toggle.
  - `platforms/android/app/src/main/java/com/deskdrop/PeerSnapshot.kt`
  - `platforms/android/app/src/main/java/com/deskdrop/ui/MainScreen.kt`
  - `platforms/android/app/src/main/java/com/deskdrop/ui/SettingsScreen.kt`
- Connectivity status is too coarse.
  - `refreshDashboardState()` reduces the world to `"Secure Connection • LAN Active"` or `"Looking for network..."`.
  - That hides the difference between:
    - trusted devices ready to reconnect
    - no peers discovered
    - network lost
    - peer explicitly disconnected
    - peer rejected or failed
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt`
- Saved settings are not fully applied on cold start.
  - On service start, the engine is created with default settings.
  - Saved sync toggles are only pushed when `ACTION_SETTINGS_CHANGED` is broadcast later.
  - That means after reboot or fresh service start, runtime behavior can temporarily drift from saved settings until settings are touched again.
  - `deskdrop-core/src/jni_android.rs`
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`

Verdict:

- Best fault-tolerant connectivity runtime.
- Still behind macOS in explainability and per-device control.

### Windows

Windows is the main platform where core connectivity strength is being undercut by stale shell wiring.

What is solid underneath:

- The tray app can launch at login using the `Run` registry key and hidden mode.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs`
  - `platforms/windows/Deskdrop.Windows/Program.cs`
- The embedded engine should still inherit the shared core auto-reconnect behavior while the app is running.
  - The engine starts through `deskdrop_start(...)`.
  - The shared core still runs discovery and the reconnect watchdog.
  - `platforms/windows/Deskdrop.Windows/Program.cs`
  - `deskdrop-core/src/ffi.rs`
  - `deskdrop-core/src/engine.rs`
- Trust approval from the TOFU prompt is wired directly through P/Invoke and looks correct in the current code.
  - `platforms/windows/Deskdrop.Windows/Program.cs`
  - `deskdrop-core/src/ffi.rs`

Major gaps:

- The Windows named-pipe IPC surface is only partial.
  - In the FFI layer, the Windows IPC handler only supports `Status`, `DisconnectPeer`, `ConnectPeer`, and `PatchSettings`.
  - Unsupported commands return `"unsupported in FFI IPC"`.
  - `deskdrop-core/src/ffi.rs`
- The dashboard manual-connect flow uses the wrong command.
  - `MainWindow.xaml.cs` sends `connect_manual`.
  - The Windows FFI IPC handler supports `ConnectPeer`, not `ConnectManual`.
  - There is even a `DaemonClient.ConnectManual(...)` helper that already sends the supported `connect_peer` request, but the dashboard does not use it.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs`
  - `platforms/windows/Deskdrop.Windows/WindowsIpcClient.cs`
  - `deskdrop-core/src/ffi.rs`
- Tray rescan is wired to an unsupported command.
  - `OnScanDevices()` sends `rescan_peers`.
  - The Windows FFI IPC handler does not implement `RescanPeers`.
  - `platforms/windows/Deskdrop.Windows/Program.cs`
  - `deskdrop-core/src/ffi.rs`
- Settings save is wired to an unsupported command.
  - `BtnSaveSettings_Click()` sends `save_settings`.
  - The tray sync toggle also sends `save_settings`.
  - The Windows FFI IPC handler does not implement `SaveSettings`.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs`
  - `platforms/windows/Deskdrop.Windows/Program.cs`
  - `deskdrop-core/src/ffi.rs`
- Diagnostics recovery is mostly cosmetic.
  - `BtnScanAgain_Click()` does not rescan peers; it sleeps and refreshes state.
  - `BtnRestartConnection_Click()` only tells the user to restart the tray app manually.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs`
- Windows startup settings are only partially honored at runtime.
  - On app startup, registry values are read for `DeviceName` and `Port`.
  - The engine is started with those values.
  - But sync-related settings are not pushed into the engine on startup.
  - Because `save_settings` is unsupported in the FFI IPC path, the settings UI is not a trustworthy live control surface today.
  - `platforms/windows/Deskdrop.Windows/Program.cs`
  - `deskdrop-core/src/ffi.rs`

What this means in practice:

- Previously trusted devices may still auto-reconnect in the background while the app is alive.
- But if a user tries to recover manually from the Windows UI, they can easily hit buttons that do not actually perform the claimed action.

Verdict:

- Core behavior is better than the shell suggests.
- User-facing connectivity confidence on Windows is currently too weak.
- This is the biggest cross-OS gap for Deskdrop today.

### Linux

Linux has a good backend story and a sparse user story.

What works well:

- The Linux binary embeds the same shared engine and full IPC server.
  - `platforms/linux/src/main.rs`
  - `deskdrop-core/src/ipc.rs`
- The installer sets up a systemd user service with restart-on-failure.
  - `install.sh` installs and enables `deskdrop.service`.
  - The service uses `Restart=on-failure`.
  - `platforms/linux/install.sh`
  - `platforms/linux/deskdrop.service`
- The service is designed to come up after network and session availability.
  - `After=network-online.target`
  - `After=graphical-session.target`
  - `platforms/linux/deskdrop.service`
- Manual connect exists through CLI.
  - `deskdrop-cli connect <ip> [port]`
  - `deskdrop-cli/src/main.rs`
- Trust and per-device sync pause/resume exist through CLI.
  - `devices trust`
  - `devices reject`
  - `devices revoke`
  - `devices peer-settings <uuid> pause|resume`
  - `deskdrop-cli/src/main.rs`

Gaps:

- Linux does not expose a rich connectivity UI by default.
  - Most recovery and pairing flows are terminal-first.
- There is no obvious CLI rescan flow.
  - Manual connect exists, but there is no equally clear user-facing "rescan now" command.
- Auto-connect is not really surfaced as a first-class user control.
  - The engine supports it.
  - Linux users do not appear to get a clear toggle for it through current CLI help or a GUI surface.

Verdict:

- Reliable enough for power users.
- Not yet a polished cross-device experience for mainstream users.

## Does A Previously Trusted Device Reconnect Automatically?

Yes, with important conditions.

It should reconnect automatically when:

- the device was trusted
- it remained remembered
- auto-connect stayed enabled
- the user did not explicitly disconnect it
- the local app/service/daemon is running again or has auto-started

It should not reconnect automatically when:

- the user explicitly disconnected it
- the user forgot the device
- trust was revoked
- the app/service was never restarted after login/boot

OS-by-OS answer:

- macOS: yes, and this is clearly surfaced in the UI.
- Android: yes, and Android has the strongest boot/network recovery implementation.
- Windows: yes in the shared engine while the tray app is alive, but the Windows repair surface is too broken to make the experience feel trustworthy.
- Linux: yes when the service is installed and active, but the user has far less guided visibility into what is happening.

## Biggest Product Gaps

### 1. Windows connectivity actions are not trustworthy

This is the single largest platform risk.

Users can click actions that appear to:

- connect manually
- scan again
- save settings
- pause/resume sync through settings

but the underlying Windows IPC bridge does not currently implement several of those commands.

This creates the worst kind of product failure:

- the engine may be healthy
- the UI may still look polished
- but recovery actions do not actually do the recovery work

### 2. Only macOS really exposes the reconnect model properly

The core has a strong concept of:

- reconnectable trusted devices
- manual disconnect suppression
- remembered vs forgotten devices
- auto-connect on/off

But only macOS clearly exposes much of that model.

Android knows about some of it internally, but compresses it in the UI.
Linux and Windows expose even less.

### 3. Android is robust but not transparent

Android probably survives bad network conditions better than any other client, but the UI still hides too much state.

A user cannot easily tell the difference between:

- the service is healthy but no peers are nearby
- a trusted peer is ready to reconnect
- the peer was explicitly disconnected
- pairing is needed again
- discovery failed and manual connect would help

### 4. Linux is reliable but not welcoming

For power users, the systemd + CLI model is acceptable.

For most users, it is still missing:

- an obvious recovery path
- visible reconnect state
- explicit auto-connect controls
- guided diagnostics

## Recommended Priority Order

### P0

1. Fix Windows IPC command drift.
   - Align the Windows dashboard/tray commands with the commands actually supported by the Windows FFI IPC bridge, or expand the FFI IPC bridge to support the full shared IPC set.
2. Make Windows diagnostics real.
   - `Scan Again` should actually rescan.
   - `Restart Connection` should actually restart or rebind.
3. Apply saved runtime settings correctly on Windows startup and Android cold start.
   - Connectivity behavior must match persisted settings immediately after boot/login, not only after visiting Settings.

### P1

1. Standardize a shared connectivity state model across all frontends.
   - connected
   - reconnecting
   - trusted and ready to reconnect
   - explicitly disconnected
   - trust required
   - network unavailable
   - discovery failed / use manual connect
2. Expose auto-connect as a first-class user control on Android and Linux.
3. Surface manual-disconnect semantics clearly.
   - If a user disconnects intentionally, the UI should explain that auto-reconnect is now paused until they reconnect manually.

### P2

1. Add stronger cross-device continuity messaging.
   - "This device will reconnect automatically when it comes back on the same network."
   - "Reconnect paused because you manually disconnected."
2. Add cross-platform repair actions that all mean the same thing.
   - reconnect now
   - rescan network
   - clear stale discovery state
   - restart local transport

## Suggested Test Matrix

These flows should become a repeatable cross-OS smoke suite:

1. Trust a device, disconnect Wi-Fi on one side, restore Wi-Fi, verify auto-reconnect.
2. Trust a device, fully reboot/login, verify the app/service auto-starts and reconnects.
3. Explicitly disconnect a trusted peer, restore network, verify it does not reconnect automatically.
4. Reconnect that same peer manually, verify explicit-disconnect state clears and later auto-reconnect works again.
5. Forget a trusted device, restart both sides, verify it does not reconnect.
6. Change IP/network on one side, verify discovery restart and recovery.
7. Reinstall or regenerate identity on one peer, verify the same-name same-IP continuity rule behaves as expected.
8. Verify manual connect works on every OS surface that claims to support it.
9. Verify every diagnostics action performs a real transport or discovery operation.

## Final Assessment

Deskdrop already has the core logic needed for a strong cross-device connectivity story.

The main challenge now is not inventing a better reconnect rule. The main challenge is making every OS faithfully honor, expose, and explain the rule.

Today:

- Android is the runtime resilience leader.
- macOS is the management UX leader.
- Linux is backend-capable but user-thin.
- Windows is the platform where connectivity confidence drops the most because shell behavior and engine behavior have drifted apart.

If the goal is "trusted devices should just come back automatically and users should never wonder why they did or did not reconnect," the next big unlock is platform consistency, especially on Windows, not new core reconnect logic.
