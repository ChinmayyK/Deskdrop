# Deskdrop Cross-Platform Functionality Audit

Date: May 27, 2026

## Scope

This audit focuses on functionality rather than UX polish.

It looks for:

- broken or stale wiring
- compile-time risks
- runtime flow failures
- actions that do something different from what the UI claims
- cross-platform behavior mismatches that can break primary workflows

This review is based on the current Android, macOS, and Windows code in the repo.

## Executive Summary

The product has improved structurally, but there are still several functionality risks in core flows.

The biggest issues right now are:

1. Android still has stale or misleading behavior in onboarding and QR pairing.
2. Windows appears to have at least two code-level breakpoints in the current main window logic.
3. macOS diagnostics include at least one repair action that triggers the wrong underlying behavior.
4. Some first-run flows now look more polished than they are actually wired.

If I were prioritizing fixes strictly by functionality risk, I would do these first:

1. Fix the Windows main-window model/view mismatches.
2. Fix Android diagnostics action constants and onboarding sample-send behavior.
3. Remove Android QR auto-trust.
4. Fix the macOS diagnostics `Enable` action so it updates auto-apply instead of sync state.
5. Add smoke tests for onboarding, pairing, diagnostics, and send/receive verification.

## Findings

### P0: Windows main window references symbols that don’t appear to exist in the current implementation

This looks like a build-breaking issue.

Current evidence:

- `OnHistoryItemAdded` checks `TimelineView`, but there is no `TimelineView` in `MainWindow.xaml`.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:24-31`
  - search over `platforms/windows/Deskdrop.Windows/MainWindow.xaml`
- Home view in XAML is named `HomeView`.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml:277`

Why this matters:

- If this is not backed by generated code elsewhere, the current window logic will not compile.
- Even if it compiled previously under an older XAML shape, it is now out of sync with the current view names.

Recommended fix:

- Replace `TimelineView` checks with the actual current view/surface name.
- Audit the rest of `MainWindow.xaml.cs` for stale references introduced during the recent onboarding/dashboard refactor.

### P0: Windows onboarding state depends on a `Trusted` property that is not present on the visible peer model

This also looks build-breaking or at minimum model-stale.

Current evidence:

- Onboarding status checks `peers.Exists(p => p.Trusted)`.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:166-194`
- The `PeerViewModel` declared in the same file contains only:
  - `device_id`
  - `friendly_name`
  - `status`
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:210-215`

Why this matters:

- If `Trusted` is not defined elsewhere through a partial or regenerated model, this will fail to compile.
- If the actual status JSON already contains trust state but the local model no longer exposes it, onboarding will never complete correctly.

Recommended fix:

- Restore trust-state fields on the Windows peer model, or change onboarding to derive from the actual current shape.
- Add a small model contract test for peer deserialization.

### P0: Android diagnostics uses action constants that do not appear to exist in the service

This looks like a compile-time or immediate runtime failure in the repair surface.

Current evidence:

- `DiagnosticsActivity` uses:
  - `DeskdropService.ACTION_START_SYNC`
  - `DeskdropService.ACTION_RESCAN`
  - `platforms/android/app/src/main/java/com/deskdrop/DiagnosticsActivity.kt:73-98`
- The service currently defines:
  - `ACTION_START`
  - `ACTION_SCAN_NOW`
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:299`
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:306`

Why this matters:

- Diagnostics is supposed to be a recovery surface.
- If its CTAs are pointed at stale constants, the user loses the exact path that should unstick the product.

Recommended fix:

- Align diagnostics with the current service contract.
- Add a smoke test that opens diagnostics and verifies each CTA maps to a valid action.

### P0: Android onboarding sample-send still performs a local clipboard apply instead of a remote send

This is a primary-flow bug.

Current evidence:

- Onboarding step 3 triggers `ACTION_APPLY_CLIPBOARD` with `"Hello from Android"`.
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:140-147`
- `ACTION_APPLY_CLIPBOARD` applies clipboard content locally.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:591-610`

Why this matters:

- The UI tells the user a sample was sent to the selected peer.
- The actual implementation applies the sample text on the Android device itself.
- That means the onboarding success condition is functionally false even if the user sees a success message.

Recommended fix:

- Replace the onboarding sample action with an actual remote-send path.
- Gate completion on real remote delivery or observation.

### P0: Android QR pairing path still auto-trusts peers after manual connect

This is both a security problem and a functional consistency problem.

Current evidence:

- QR/manual pairing still schedules `autoTrustNewPeer(...)` after connect if a fingerprint is present.
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:474-480`
- `autoTrustNewPeer` immediately sends `ACTION_TRUST_PEER`.
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:504-517`
- This conflicts with the newer pairing-request flow in the service.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1137-1151`

Why this matters:

- Two pairing models are now live at once:
  - explicit trust confirmation
  - silent or semi-silent QR trust
- That can create inconsistent peer state, especially when onboarding, QR flow, and pairing prompts overlap.

Recommended fix:

- Remove `autoTrustNewPeer`.
- Force all new trust establishment through one pairing-confirmation path.

### P1: Android onboarding connect path hardcodes a port that does not match the service default

Current evidence:

- Onboarding connect uses port `8244`.
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:130-137`
- Manual connect defaults to `47823`.
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:461-489`
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:347`
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:510-516`

Why this matters:

- If peers are listening on the default Deskdrop port, onboarding can fail even when discovery and trust are otherwise healthy.
- This is especially risky because it sits in the first-run path and may look like a networking failure.

Recommended fix:

- Use the shared default port constant or a discovered peer-specific port, never a separate hardcoded value.

### P1: Windows onboarding checklist in home view is not refreshed from the peer list unless devices view is opened

Current evidence:

- `LoadHomeView()` updates onboarding visibility but does not refresh peers.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:99-110`
- `RefreshDevicesList()` is called from `LoadDevicesView()` and disconnect flow.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:133-164`
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:198-205`
- `UpdateOnboardingStatus(peers)` runs inside `RefreshDevicesList()`.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:152-159`

Why this matters:

- The onboarding checklist lives in the home view.
- But its state appears to depend on device refresh logic that only runs when the user goes to the devices view.
- That can leave the first-run checklist stale or inert.

Recommended fix:

- Refresh peer state from home view as well.
- Decouple onboarding state updates from the devices page lifecycle.

### P1: macOS diagnostics “Enable” action for clipboard auto-apply toggles sync instead of auto-apply

This is a direct action/behavior mismatch.

Current evidence:

- Diagnostics shows an `Enable` action when auto-apply is off.
  - `platforms/macos/Deskdrop/DiagnosticsView.swift:48-56`
- That action calls `store.enableAutoApply()`.
  - `platforms/macos/Deskdrop/DiagnosticsView.swift:55`
- `enableAutoApply()` currently calls `toggleSync()` instead of `setAutoApplyClipboard(enabled: true)`.
  - `platforms/macos/Deskdrop/DeskdropStore.swift:123-126`
- The actual auto-apply setter exists separately.
  - `platforms/macos/Deskdrop/DeskdropStore.swift:779-783`

Why this matters:

- The user clicks a repair action expecting clipboard auto-apply to turn on.
- Instead, sync state may toggle, leaving the underlying issue unresolved or creating a second problem.

Recommended fix:

- Change `enableAutoApply()` to call `setAutoApplyClipboard(enabled: true)`.
- Keep sync toggling separate.

### P1: macOS diagnostics “Restart Daemon” does not actually restart the daemon

Current evidence:

- Diagnostics exposes `Restart Daemon`.
  - `platforms/macos/Deskdrop/DiagnosticsView.swift:28-36`
- `restartDaemon()` only stops polling/watching and starts them again later.
  - `platforms/macos/Deskdrop/DeskdropStore.swift:110-117`
- `start()` just starts polling and clipboard watching.
  - `platforms/macos/Deskdrop/DeskdropStore.swift:99-102`

Why this matters:

- If the daemon is actually down, restarting the UI poller is not the same as restarting the daemon.
- The action label currently promises more than the implementation delivers.

Recommended fix:

- Either implement a true daemon restart path, or rename the action to match the current behavior.

### P1: macOS onboarding sample-send mutates the local clipboard before sending

Current evidence:

- Onboarding step 3 does:
  - `applyClipboardLocally(text: "Hello from Mac")`
  - `sendCurrentClipboard(to: ...)`
  - `platforms/macos/Deskdrop/OnboardingView.swift:206-210`

Why this matters:

- This overwrites the user’s current clipboard just to drive a tutorial step.
- It can cause data loss or surprise if the user had something important copied.

Recommended fix:

- Send a synthetic sample payload without mutating the user’s live clipboard, or explicitly restore the previous clipboard afterward.

### P2: Android pairing prompt logic currently has two divergent implementations

Current evidence:

- There is a new pairing-request event path that starts `PairingActivity` directly with a `pin`.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1137-1151`
- There is also an unused `showPairingPrompt(...)` path that builds a notification, passes fingerprint and a derived pin, and starts the activity.
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:2650-2679`
- Search over the service shows only the function definition for `showPairingPrompt(...)`.

Why this matters:

- Duplicate trust-entry paths are easy to let drift apart.
- One path appears to provide fingerprint context, the other currently does not.

Recommended fix:

- Collapse pairing prompt creation into one implementation path.

## Testing Gaps

The recurring pattern in these issues is not just bugs. It is missing verification around core flows.

The highest-value automated checks now would be:

- Android:
  - onboarding connect action maps to valid service action and valid port
  - diagnostics CTAs map to valid service actions
  - QR pairing does not auto-trust
  - sample onboarding send performs a remote send
- macOS:
  - diagnostics auto-apply action updates `clipboardPolicy.autoApply`
  - restart action actually revives daemon connectivity or is relabeled
  - onboarding sample does not destroy the current clipboard
- Windows:
  - main window compiles against current XAML names
  - onboarding model matches current peer deserialization shape
  - home-view onboarding status updates without navigating away

## Priority Roadmap

### P0

- Fix Windows `TimelineView` reference
- Fix Windows `PeerViewModel.Trusted` mismatch
- Fix Android diagnostics action constants
- Fix Android onboarding sample-send path
- Remove Android QR auto-trust

### P1

- Fix Android onboarding port mismatch
- Fix Windows onboarding refresh dependency on devices view
- Fix macOS diagnostics auto-apply action
- Fix macOS daemon restart action labeling or implementation
- Stop mutating the live clipboard during macOS onboarding

### P2

- Unify Android pairing prompt code paths
- Add smoke/integration tests for first-run flows

## Bottom Line

The biggest remaining functionality risk is not low-level engine behavior. It is **UI-to-engine contract drift**.

Several important surfaces now look polished, but some are still pointed at stale actions, partial model shapes, or behavior that no longer matches the user-facing language.

If you fix only one cluster next, fix the first-run and repair surfaces:

- onboarding
- pairing
- diagnostics
- Windows dashboard state wiring

Those are the places where current functionality issues are most likely to block users outright.
