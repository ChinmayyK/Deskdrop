# Deskdrop Full-Stack Friction, Fault Tolerance, and Connectivity Audit

Date: May 27, 2026

## Scope

This audit builds on the earlier UX, functionality, and parity docs already in `docs/`, but it shifts the lens in one important way:

- not just whether the screens look polished
- not just whether one platform has a feature
- but whether the full product is easy to trust, easy to recover, and structurally resilient when real networks and real operating systems behave badly

This review is based on the current Rust core plus the Android, macOS, Windows, and Linux platform shells in this repo.

It focuses on:

- user friction across first-run and daily use
- functional gaps between the shared core and platform wrappers
- fault tolerance and recovery behavior
- where connectivity can silently fail or feel non-deterministic
- what should change to make Deskdrop feel durable instead of merely impressive

## Executive Summary

Deskdrop has strong foundations:

- a serious Rust core
- a good local-first product thesis
- resumable file-transfer primitives
- a real trust store
- thoughtful activity-feed concepts
- visible effort toward onboarding and diagnostics

The biggest remaining problem is no longer "missing UI polish."

It is **cross-layer integrity**.

The product currently has multiple places where:

1. the core has the right abstraction, but the platform shell does not surface it
2. the UI offers an action, but the backend contract behind that action is partial or stale
3. the product says "connected", "sent", "trusted", or "ready" with more confidence than the code has actually earned
4. connectivity hardening exists in pieces, but not yet as one end-to-end operating model

If I were prioritizing the next round strictly for user impact, I would do these first:

1. Fix the Windows backend contract drift and trust approval path.
2. Ship one real shared health-state model across core, Android, macOS, and Windows.
3. Finish the file-transfer reliability work: correct progress math, reduce recovery loss windows, and actually use link-quality telemetry.
4. Remove implicit trust upgrades and force one canonical pairing model.
5. Turn diagnostics from status cards into repair consoles with direct fix actions.

## Core Thesis

Deskdrop is close to feeling premium, but users will still feel friction because the product has not fully crossed the line from:

- "feature-rich"

to:

- "operationally trustworthy"

That distinction matters more for Deskdrop than for an ordinary app.

Deskdrop lives in the category of tools users expect to feel invisible:

- copy something
- trust a device
- send a file
- recover when Wi-Fi changes
- know where received content went

When those flows feel ambiguous, users do not experience "minor bugs."

They experience:

- uncertainty
- mistrust
- fear of data loss
- repeated retries
- the sense that the product is brittle

That is the real ceiling on adoption right now.

## What Users Will Feel

### First-run friction

The current product still makes first-run feel more successful than it really is in some paths.

Users can complete onboarding before the system has unambiguously proven:

- discovery worked
- trust was established through one canonical model
- the intended target device received a real payload
- the user understands where received text and files will land

### Daily-use friction

The app often compresses many different failure states into one status message:

- no local network permission
- Wi-Fi changed
- peer offline
- firewall or LAN policy block
- daemon down
- sync paused
- background restrictions

When all of those collapse into "Looking for peers" or "Ready", users have to guess.

### Recovery friction

Diagnostics exist on some platforms, but they are still mostly snapshot surfaces rather than active repair flows.

The current product too often tells the user what is wrong without closing the loop on fixing it.

### Trust friction

Multiple trust concepts still coexist:

- pair
- trust
- reject
- legacy trust
- implicit trust upgrade

That weakens confidence because users cannot tell which path is the real one.

### File-transfer friction

Files are not handled consistently across platforms:

- destination behavior differs
- large-file behavior differs
- some wrappers stream by path while others read whole files into memory
- progress and resume semantics are not equally trustworthy everywhere

## Critical Findings

### P0: The Windows shell and the Windows IPC backend are out of contract

This is the single most important full-stack issue in the repo.

Current evidence:

- The Windows FFI-backed named-pipe server only handles `Status`, `DisconnectPeer`, `ConnectManual`, and a stubbed `SaveSettings` path. Everything else returns `"unsupported in FFI IPC"`.
  - `deskdrop-core/src/ffi.rs:74-111`
- The Windows tray and dashboard actively send commands like `push_clipboard`, `rescan_peers`, and `save_settings`.
  - `platforms/windows/Deskdrop.Windows/Program.cs:657-708`

Why this matters:

- Users can click real-looking controls that do not have a matching backend implementation.
- Diagnostics, scan, quick-send, and settings persistence can look present while being functionally partial.
- This is worse than missing UI because it teaches users that the app may not mean what it says.

What users will feel:

- "Scan Again" that does not actually recover discovery
- "Clipboard sent" feedback that may be optimistic
- settings that look saved but do not fully affect the engine

Recommendation:

- Stop treating Windows as a thin shell over an ad hoc FFI control surface.
- Either:
  - implement the full IPC request set in the Windows FFI server, or
  - stop issuing IPC commands from the Windows UI that are not supported
- Add a contract test that enumerates the Windows UI command set against supported IPC requests.

### P0: Windows trust approval is effectively broken and identity resolution is unsafe

Current evidence:

- `deskdrop_trust_peer` resolves the target by device name, not device ID.
  - `deskdrop-core/src/ffi.rs:722-748`
- It searches `trusted_devices()` before trying to trust or reject.
  - `deskdrop-core/src/ffi.rs:736-747`

Why this matters:

- A pending untrusted device is, by definition, not in the trusted-device list.
- That means the Windows approval path can fail to resolve the device the user just approved.
- Even if it worked, resolving by display name is not robust when two devices share a similar name.

What users will feel:

- tapping Trust and seeing no durable result
- repeated trust prompts
- uncertainty about whether the correct device was approved

Recommendation:

- Move the Windows trust flow to device-ID-based resolution only.
- Source trust actions from the pending pairing request itself, not from a trusted-device lookup.
- Remove any remaining name-based identity resolution from security-sensitive paths.

### P0: Deskdrop has a shared health-state model in core, but it is not actually shipped end-to-end

Current evidence:

- `SystemHealthState` exists in the core with states like `NeedsLocalNetworkPermission`, `FirewallOrNetworkBlocked`, `BatteryRestricted`, `TrustPending`, and `SyncPaused`.
  - `deskdrop-core/src/engine.rs:57-69`
- The Android service receives `CR_EVENT_SYSTEM_HEALTH_UPDATED` but still contains only a TODO.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1164-1165`
- Android main-screen status still compresses reality to connected vs. "Looking for network..."
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:358-369`
- macOS status is also intentionally coarse.
  - `platforms/macos/Deskdrop/DeskdropStore.swift:1008-1018`

Why this matters:

- The hardest Deskdrop problems are not visual, they are diagnostic.
- Users need the product to say why it is not working, not merely that it is not working.

What users will feel:

- "Is it my Wi-Fi, my permissions, the daemon, or the other device?"
- "Why does it say active when nothing arrives?"

Recommendation:

- Make `SystemHealthState` the single source of truth for user-facing status.
- Emit it from the core whenever discovery, permissions, trust, sync, or reconnect conditions change.
- Map each state to:
  - one primary sentence
  - one primary CTA
  - one secondary "learn more" or repair path

### P0: The transfer pipeline has the right primitives, but its telemetry and adaptation are not trustworthy enough

Current evidence:

- Chunks are now `4 MB`, but ACK frequency is still configured as `16` chunks while the comment says "ACK every 16 MB". At 4 MB per chunk, that is really every `64 MB`.
  - `deskdrop-core/src/file_transfer.rs:39-42`
- Outbound transfer progress uses `last_acked_chunk * 65536` bytes, which is off by a factor of 64 relative to the current chunk size.
  - `deskdrop-core/src/file_transfer.rs:648-652`
- The adaptive `QualityProbe` stores a recommended chunk size.
  - `deskdrop-core/src/probe.rs:146-184`
- But the engine only records RTT samples; it does not wire that chunk-size recommendation into the active transfer path.
  - `deskdrop-core/src/engine.rs:3451-3462`

Why this matters:

- Resume windows are larger than intended on unstable links.
- Progress, speed, and ETA become misleading.
- The product looks less trustworthy exactly when users are sending big files over shaky networks.

What users will feel:

- slow or jumpy progress bars
- resume behavior that appears to "lose" too much work after a drop
- transfers that do not visibly adapt to weak links

Recommendation:

- Make the ACK window byte-based, not comment-based.
- Fix progress math to use actual acknowledged bytes.
- Feed `QualityProbe.chunk_size()` into outbound transfer batching.
- Reduce loss window on unstable networks by shrinking chunk and ACK windows dynamically.

### P0: Core trust behavior still bypasses the explicit pairing ceremony in one important path

Current evidence:

- When a new untrusted device ID appears, the core will auto-trust it if a previously trusted peer exists with the same friendly name and the same IP.
  - `deskdrop-core/src/engine.rs:2752-2787`

Why this matters:

- This is understandable as a developer-convenience heuristic, especially for Android debug builds.
- But it weakens the product's conceptual integrity because the UI is teaching users that explicit pairing matters.
- Name + IP is not strong enough to silently skip a trust step on real networks.

What users will feel:

- inconsistent trust behavior
- QR/pairing flows that sometimes seem required and sometimes not
- uncertainty about whether Deskdrop is truly strict about verification

Recommendation:

- Remove auto-trust upgrades from production paths.
- If the debug-build problem is real, solve it with a clearly labeled developer-only override.

## High-Value P1 Findings

### P1: macOS clipboard image sync looks wired, but the IPC command name does not match the daemon contract

Current evidence:

- The macOS store sends `cmd: "push_clipboard_image"`.
  - `platforms/macos/Deskdrop/DeskdropStore.swift:159-167`
- The IPC contract defines `push_image`, not `push_clipboard_image`.
  - `deskdrop-core/src/ipc.rs:109`
  - `deskdrop-core/src/ipc.rs:901`

Why this matters:

- Users may copy an image on macOS and never see it arrive on the other device.
- This kind of failure is especially damaging because clipboard sync is the product's core promise.

Recommendation:

- Align the macOS sender with the real IPC contract immediately.
- Add a clipboard-image smoke test at the app-shell layer.

### P1: macOS dashboard status still overstates certainty because sync state is hardcoded

Current evidence:

- `dashboardStatus.syncEnabled` is hardcoded to `true`.
  - `platforms/macos/Deskdrop/DeskdropStore.swift:324-330`

Why this matters:

- If the user pauses sync or the daemon settings diverge, the UI can still present a healthier state than reality.

Recommendation:

- Include `sync_enabled` in the macOS status response and drive status from the daemon, not from local assumptions.

### P1: Windows device list refresh is likely reading the wrong JSON shape

Current evidence:

- `RefreshDevicesList()` tries to read `peers` directly on the root JSON object.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:343-355`
- Other Windows code paths already read daemon status under `data`.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:164-184`

Why this matters:

- The devices page is the place users go to recover connectivity and verify trust.
- If it renders stale or empty data, the recovery story collapses.

Recommendation:

- Normalize all Windows status parsing around the real IPC envelope shape.
- Add one JSON-deserialization contract test for status responses.

### P1: Windows large-file sending bypasses the resumable path and reads whole files into memory

Current evidence:

- Windows clipboard/file send paths call `File.ReadAllBytes(...)` and then pass the whole payload through `deskdrop_push_file(...)`.
  - `platforms/windows/Deskdrop.Windows/Program.cs:304-312`
  - `platforms/windows/Deskdrop.Windows/Program.cs:326-334`

Why this matters:

- This defeats the product's stronger file-transfer story:
  - path-based streaming
  - lower memory pressure
  - resumable transfer semantics
- It makes Windows the weakest platform for large-file reliability.

Recommendation:

- Add a Windows path-based send route that maps to `send_file_path`, not the raw clipboard payload route.

### P1: Android onboarding still proves UI progression more than target-device success

Current evidence:

- The onboarding sample send triggers `ACTION_PUSH_CLIPBOARD` with inline sample text.
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:152-156`
- The onboarding state still completes from coarse peer/session conditions rather than a scoped proof tied to the selected target device.
  - `platforms/android/app/src/main/java/com/deskdrop/ui/OnboardingScreen.kt:34-43`

Why this matters:

- The user is being told "we verified it works" when the product has not tied the success state to a proven delivery/observation event for that chosen device.

Recommendation:

- Track onboarding by session and by target peer.
- Require:
  - selected peer
  - trust confirmed
  - sample delivered to that peer
  - sample observed/applied

### P1: Android's JNI `pushFile` path still caps files at 32 MB

Current evidence:

- Android JNI rejects file payloads above `MAX_IMAGE_BYTES` and logs a 32 MB limit.
  - `deskdrop-core/src/jni_android.rs:183-198`

Why this matters:

- The core protocol advertises a much stronger file-transfer story than the Android JNI wrapper currently exposes.
- Users will experience Android as "bad at files" even though the core is better than that.

Recommendation:

- Stop routing Android files through the raw in-memory clipboard-file payload path for large sends.
- Prefer URI/path-based transfer initiation everywhere possible.

### P1: Android file-completion notifications may open the wrong path after the file is copied to public Downloads

Current evidence:

- The service computes `finalPath` after copying to public downloads.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1083-1091`
- But the completion notification still opens `destPath`, not `finalPath`.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1093`
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1418-1445`
- On Android Q+, `saveFileToPublicDownloads()` inserts through MediaStore and then returns a guessed filesystem path by name.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1702-1719`

Why this matters:

- Users can receive a "file complete" notification that points to the wrong file location.
- Duplicate filenames become especially risky.

Recommendation:

- Keep and use the actual MediaStore `Uri`.
- Build the completion notification from the real public destination, not the private temporary path.

### P1: Desktop file destinations are inconsistent with the product promise

Current evidence:

- The core default save path is just the platform downloads directory, not a dedicated `Deskdrop/` folder.
  - `deskdrop-core/src/file_transfer.rs:760-763`
- Android explicitly overrides into a `Deskdrop` folder, so behavior already differs by platform.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1661-1668`

Why this matters:

- Users want predictable receive locations.
- "Where did my file go?" is one of the highest-friction questions in cross-device transfer apps.

Recommendation:

- Standardize incoming files to a dedicated `Deskdrop/` subfolder on every platform, unless the user overrides it.

### P1: Linux is reliable for power users, but still not humane for everyone else

Current evidence:

- Linux remains effectively headless and pushes trust recovery into CLI instructions and `notify-send`.
  - `platforms/linux/src/main.rs:1-8`
  - `platforms/linux/src/main.rs:226-252`

Why this matters:

- This is a valid product choice if intentional.
- But if Linux is meant to feel like a first-class consumer client, it is still far behind in recoverability and discoverability.

Recommendation:

- Decide explicitly whether Linux is:
  - a power-user daemon+CLI target, or
  - a parity GUI target
- Then design accordingly instead of leaving it ambiguously in between.

## Where Deskdrop Is Lagging Behind

Deskdrop is not mainly lagging in visual polish anymore.

It is lagging in five deeper categories:

### 1. State truthfulness

The product still does not consistently tell users:

- what state it is in
- why that state exists
- what to do next

### 2. Cross-platform contract discipline

The Rust core is ahead of some platform shells.

The shells are ahead in a few UI surfaces, but not always backed by matching engine or IPC behavior.

That inversion is where trust erodes.

### 3. Large-file parity

macOS is closest to a good path-based transfer story.

Windows and Android still expose weaker routes in important paths:

- Windows reads full files into memory
- Android JNI caps file payloads at 32 MB

### 4. Repair-oriented UX

Diagnostics mostly explain.

They do not yet consistently repair.

### 5. Canonical trust model

A product in this category needs one trust story.

Deskdrop still has too many variants:

- pairing request
- trust
- reject
- implicit trust upgrade
- legacy paths

## Platform-by-Platform Deep Dive

### macOS

Strengths:

- best overall UI cohesion
- strongest dashboard mental model
- good menu-bar fit
- path-based file sending exists

Main gaps:

- image clipboard sync command drift
- sync status still over-compressed
- diagnostics are closer to useful than other platforms, but still not a real repair console

User friction:

- copied image may appear to do nothing
- paused vs healthy state can be too subtle
- "scan" and "connect" are still power-user flavored, not truly guided

### Android

Strengths:

- the most deliberate work on background resilience
- explicit service model
- OEM battery restrictions at least acknowledged
- file-receive handling is more mature than earlier passes

Main gaps:

- health state not surfaced
- onboarding still not proof-scoped to the selected device
- large-file parity still weak in JNI path
- completion/open-location contract is still fragile

User friction:

- "why can't it find my device?" still has too few concrete answers
- file receive success may not map cleanly to where the user expects the file to open
- onboarding can feel successful before it has really been proven

### Windows

Strengths:

- ambitious native surface
- tray utility behavior is appropriate for the category
- broadcast camera experimentation shows product ambition

Main gaps:

- backend contract drift
- trust path weakness
- incorrect status parsing
- in-memory file sending path

User friction:

- controls that do less than they imply
- first-run trust that does not feel deterministic
- device management that can appear stale or empty

### Linux

Strengths:

- strong headless story
- clean daemon+CLI foundation
- good fit for advanced users and self-hosters

Main gaps:

- no humane GUI repair path
- no parity story for non-technical users

User friction:

- anyone expecting a normal desktop utility will feel under-supported

## How To Make Deskdrop Easier To Use

### 1. Replace generic status with reasoned status

Users should never have to reverse-engineer whether the issue is:

- permissions
- firewall
- daemon
- peer offline
- trust pending
- battery restrictions
- sync paused

### 2. Make every recovery surface action-led

Each diagnostic card should have:

- one sentence of truth
- one primary repair action
- one optional deeper link

Examples:

- `Local network permission missing` -> `Open Privacy Settings`
- `No peers discovered on this subnet` -> `Rescan + show manual connect`
- `Trusted peer offline` -> `Retry last known endpoint`
- `Background restrictions detected` -> `Open OEM-specific settings`

### 3. Make file destinations explicit everywhere

Always show:

- where the incoming file was saved
- how to open that location
- whether the platform uses a private app folder or public downloads

### 4. Unify the "send" mental model

Users should always know:

- does this action send to all connected devices or one selected device?
- is this a clipboard push, a timeline resend, or a dedicated file transfer?

Right now that answer is too platform-dependent.

### 5. Remove legacy trust verbs from normal UI

For new devices, expose one visible path:

- pair
- compare code
- approve on both ends
- trusted

Everything else should be hidden as fallback or debug behavior.

## How To Make Connectivity Much More Fault-Proof

### 1. Ship a real connectivity state machine from the core

The core should emit explicit states like:

- `idle_no_peers`
- `permission_blocked_local_network`
- `permission_blocked_notifications`
- `background_restricted`
- `firewall_or_policy_blocked`
- `discovering`
- `connecting`
- `trust_pending`
- `connected`
- `reconnecting_backoff`
- `sync_paused`

This should not be re-inferred differently on every platform.

### 2. Keep one reconnect supervisor in charge

The reconnect story should have:

- per-peer backoff
- last-known-good endpoint
- interface-aware retries
- reason-coded disconnects
- a circuit breaker after repeated failures
- a visible transition back to discovery/manual fallback

### 3. Make link adaptation real, not aspirational

The `QualityProbe` work is valuable.

Finish it by:

- applying per-peer chunk sizes
- shrinking ACK windows on degraded links
- surfacing degraded link quality in file-transfer UX
- separating "slow but healthy" from "unstable and retrying"

### 4. Standardize streaming file paths

Prefer path/URI-based transfer initiation everywhere:

- macOS
- Android
- Windows

Avoid in-memory whole-file routes except for genuinely small payloads.

### 5. Turn trust into a strict two-sided ceremony

Connectivity becomes more fault-proof when identity is stable and unambiguous.

That means:

- QR for discovery/bootstrap only
- SAS/PIN confirmation on both ends
- no silent upgrade from "same name + same IP"

### 6. Add a repair bundle

Every GUI platform should be able to generate a small diagnostics snapshot:

- active interface
- bind address
- peer list with states
- last reconnect reason
- sync enabled/paused
- pending trust requests
- last file destination

That makes support and self-repair much easier.

## Testing Gaps

The repo has solid core tests, but the integration surface that users actually touch is still under-tested.

Most missing coverage is not crypto or protocol math.

It is shell-contract coverage:

- onboarding completion gating
- pairing request/response flow
- diagnostics CTA wiring
- Windows IPC command coverage
- platform-specific file-send paths
- network-change recovery
- file-destination correctness after receive

The highest-value tests to add next are:

1. Windows IPC contract smoke tests.
2. Android onboarding + diagnostics action tests.
3. macOS clipboard-image send test.
4. cross-platform network-change reconnect tests.
5. file-transfer progress/resume tests that assert actual byte math.

## Recommended Roadmap

### Immediate: next 7 days

1. Fix Windows IPC contract drift.
2. Fix Windows trust approval to use device IDs and pending requests.
3. Fix file-transfer byte math and ACK-size mismatch.
4. Fix macOS `push_clipboard_image` command mismatch.
5. Fix Android file-notification destination handling.
6. Remove Android JNI's 32 MB file cap from the wrong path or reroute to streaming.

### Near term: next 30 days

1. Ship `SystemHealthState` end-to-end.
2. Add one repair-first diagnostics surface on every GUI platform.
3. Standardize file destination behavior.
4. Remove implicit trust upgrades.
5. Make onboarding session-scoped and target-device-scoped.

### Medium term: next 60-90 days

1. Wire adaptive chunk sizing into live transfers.
2. Add recovery-aware metrics and support bundle export.
3. Decide Linux strategy explicitly: power-user target vs parity GUI target.
4. Finish cross-platform trust-model simplification.

## Bottom Line

Deskdrop is already beyond the "interesting prototype" stage.

The remaining work is not primarily about adding more features.

It is about making the product's claims line up with its actual behavior across every layer:

- core
- IPC
- onboarding
- diagnostics
- trust
- file transfer
- background recovery

Once those contracts are tightened, Deskdrop can feel not just polished, but dependable.

That is the point where users stop thinking about whether it is working, and simply rely on it.
