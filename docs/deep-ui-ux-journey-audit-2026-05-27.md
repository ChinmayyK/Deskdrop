# Deskdrop Second-Round Deep UI/UX and User Journey Audit

Date: May 27, 2026

## Scope

This is a second-round audit after the recent implementation pass across Android, macOS, and Windows.

This review focuses on:

- Deep interaction quality, not just first-order friction
- End-to-end user journey integrity
- Cross-platform mental-model consistency
- Where advanced UX theory should change the implementation, not just the visuals

This report combines:

- Current repo review of the latest Android, macOS, and Windows clients
- Current platform guidance from Google, Apple, and Microsoft
- Established usability heuristics and interaction-design theory

Important framing:

- Round 1 was mainly about removing obvious friction.
- Round 2 is about making the product behavior trustworthy, legible, and resilient under real-world failure.
- The key question is no longer "does the UI look better?" It is "does the product tell the truth about system state, trust, and success?"

## Executive Summary

Deskdrop is meaningfully better than the first review.

The current implementation already shows strong progress:

- Android startup permissions were narrowed substantially.
- Android and macOS now have structured onboarding.
- Diagnostics surfaces now exist on Android and macOS.
- Windows trust confirmation moved into the main app window.
- Windows also gained a less-blocking in-window toast system.

The next level of UX work is not another visual pass. It is a systems UX pass.

The biggest remaining issue is **journey integrity**:

1. The product still marks important steps complete before the system has actually proven success.
2. Pairing still looks more trustworthy than it really is in some paths.
3. Status is still too compressed, so users must infer why sync is or is not working.
4. Several actions are still too generic, which raises cognitive load and lowers confidence.
5. Recovery flows exist, but they are mostly informational instead of repair-oriented.

If I were choosing the highest-value advanced UX work now, I would do these five things first:

1. Replace click-based onboarding with evidence-based onboarding.
2. Replace pseudo-verification with a real cross-platform trust ceremony.
3. Introduce one shared `SystemHealthState` model across Android, macOS, and Windows.
4. Make activity/actions type-aware instead of generic.
5. Turn diagnostics into a repair console with direct fix actions, not a passive status page.

## What Improved Since Round 1

### Android

- Startup permission asks are much narrower than before.
- There is now a guided onboarding flow.
- There is now a dedicated diagnostics entry point.
- Call-related permissions are deferred behind a settings toggle instead of being requested immediately.

### macOS

- There is now a guided onboarding flow.
- There is now a dedicated diagnostics window.
- Menu bar utilities and dashboard interactions feel more cohesive than before.

### Windows

- TOFU moved from a separate WinForms prompt toward an in-window overlay.
- Several outcomes now use inline toasts instead of blocking message boxes.
- Manual connect now routes through the main dashboard instead of a tray-only prompt.

This is real progress. The product feels more intentional.

## Core Thesis

Deskdrop is now entering a phase where the hardest UX problems are not about styling. They are about **calibration**:

- Is the user seeing the real system state?
- Does "trusted" actually mean verified?
- Does "sent" mean queued, delivered, or applied?
- Does "done" mean "the user clicked next" or "the product proved the workflow works"?

Right now, the UI still has several places where it behaves optimistically before the underlying system has earned that confidence.

That creates a subtle but important risk:

- the interface feels smoother in the moment,
- but trust decays later when the user realizes the success message was stronger than the actual outcome.

This is the main reason I would focus the next round on journey architecture, state architecture, and proof-driven feedback.

## Critical Findings

### P0: Onboarding completion is still click-based, not proof-based

This is the most important remaining UX problem.

Current evidence:

- Android step 1 auto-advances on device selection, while the footer still offers a `Next` button, creating a mixed interaction model.
  - `platforms/android/app/src/main/java/com/deskdrop/ui/OnboardingScreen.kt:62-76`
  - `platforms/android/app/src/main/java/com/deskdrop/ui/OnboardingScreen.kt:92-99`
- Android step 3 says it sends sample text to the selected peer, but `MainActivity` actually triggers `ACTION_APPLY_CLIPBOARD`, which applies clipboard locally.
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:138-145`
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:587-610`
- Android completion copy claims immediate ongoing success without evidence.
  - `platforms/android/app/src/main/java/com/deskdrop/ui/OnboardingScreen.kt:207-222`
- macOS has the same structural issue:
  - device selection auto-advances,
  - verification uses a placeholder code,
  - sample send advances immediately,
  - completion text promises durable success before proof.
  - `platforms/macos/Deskdrop/OnboardingView.swift:47-97`
  - `platforms/macos/Deskdrop/OnboardingView.swift:117-199`
  - `platforms/macos/Deskdrop/OnboardingView.swift:205-225`

Why this matters:

- The end of onboarding is the peak and end of the first-use experience.
- If the product says "you’re all set" before the user has truly discovered, trusted, sent, and observed receipt, the memory of the experience becomes fragile.
- A false positive at this stage is more damaging than a slower but truthful flow.

Theory behind this:

- Visibility of system status: important state must be timely and truthful.
- Goal-gradient effect: progress motivates completion, but only if progress reflects real advancement.
- Peak-end rule: people remember the strongest and final moments disproportionately.
- Progressive onboarding works best when each new step is unlocked by successful action, not by presentation alone.

Recommendation:

- Introduce a shared `JourneyOrchestrator` that listens to real engine events.
- Each onboarding step should complete only on proof:
  - `DeviceDiscovered`
  - `ConnectionEstablished`
  - `TrustConfirmedLocal`
  - `TrustConfirmedRemote`
  - `SampleDelivered`
  - `SampleObserved` or `SampleApplied` depending on platform constraints
- Replace generic completion with a verified state:
  - `Ready to send to <device>`
  - `Clipboard delivery verified`
  - `Downloads location confirmed`

This will be a more complex implementation, but it is the right one.

### P0: The trust ceremony still overstates confidence

Current evidence:

- Android and macOS onboarding use `peer.id.take(6)` / `peer.id.prefix(6)` as a "verification code".
  - `platforms/android/app/src/main/java/com/deskdrop/ui/OnboardingScreen.kt:162-175`
  - `platforms/macos/Deskdrop/OnboardingView.swift:153-169`
- Android QR pairing still attempts automatic trust when a fingerprint is present.
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:453-516`
- Windows is better here because it displays a fingerprint in the in-window TOFU prompt, but this ceremony is still isolated from a shared cross-platform pairing model.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml:454-493`
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml.cs:321-363`

Why this matters:

- Pairing is the moment where the user decides whether the system is safe enough to use.
- If the UI uses "Verify", "Trust", or "Paired" language without a real reciprocal confirmation model, the words become stronger than the protocol.
- Android auto-trust after QR is especially risky because it hides one of the few moments where the user should stay alert.

Theory behind this:

- Match between system and the real world: the UI should use trust language only when it corresponds to a real trust action.
- Error prevention: pairing is where the product should prevent the wrong device from entering the user’s mesh.
- User control and freedom: users need a clear opportunity to confirm or abandon.

Recommendation:

- Replace placeholder short codes with a real Short Authentication String ceremony shared across all platforms.
- Recommended model:
  - device discovery
  - connection attempt
  - ephemeral shared secret
  - 6-digit code or 2-word phrase derived from handshake material
  - local confirm
  - remote confirm
  - trusted state only after both sides confirm
- Remove Android QR auto-trust entirely.
- Treat QR as:
  - discovery,
  - prefill,
  - or connection bootstrap,
  but not implicit trust.

### P1: State is still too compressed, so users must infer causes

Current evidence:

- Android ambient status still collapses to either connected or "Looking for network..."
  - `platforms/android/app/src/main/java/com/deskdrop/MainActivity.kt:340-352`
- Android diagnostics are static snapshots with generic suggestions and no live repair actions.
  - `platforms/android/app/src/main/java/com/deskdrop/DiagnosticsActivity.kt:27-90`
- macOS computes richer internal state, but the user-facing status still compresses several causes into a small set of messages.
  - `platforms/macos/Deskdrop/DeskdropStore.swift:200-217`
  - `platforms/macos/Deskdrop/DeskdropStore.swift:303-322`
- macOS diagnostics remain mostly passive.
  - `platforms/macos/Deskdrop/DiagnosticsView.swift:26-66`
- Windows tray status is also coarse and mostly connection-count based.
  - `platforms/windows/Deskdrop.Windows/Program.cs:384-392`
  - `platforms/windows/Deskdrop.Windows/Program.cs:497-507`

Why this matters:

- In a cross-device utility, many failures look the same to users:
  - no peers,
  - no local network permission,
  - daemon down,
  - battery restricted,
  - firewall blocked,
  - notifications disabled,
  - trust pending,
  - delivery queued but not applied.
- If the UI shows only one compressed state, users perform their own diagnosis.
- That increases effort and lowers trust.

Theory behind this:

- Visibility of system status.
- Recognition rather than recall.
- Tesler’s Law: this complexity exists whether we acknowledge it or not; the system should absorb it instead of exporting it to the user.

Recommendation:

- Introduce a platform-agnostic `SystemHealthState` with explicit categories:
  - `Ready`
  - `NoPeers`
  - `DiscoveryInProgress`
  - `NeedsLocalNetworkPermission`
  - `NeedsNotificationsPermission`
  - `BatteryRestricted`
  - `DaemonStopped`
  - `FirewallOrNetworkBlocked`
  - `TrustPending`
  - `SyncPaused`
  - `DeliveryQueued`
  - `ActionRequired`
- Each state should define:
  - one short title,
  - one explanatory sentence,
  - one primary CTA,
  - one secondary CTA,
  - one severity level,
  - whether it is transient, persistent, or blocking.

This state model should feed:

- Android home status banner
- Android diagnostics
- macOS dashboard banner
- macOS menu bar popover header
- Windows inline status bar / InfoBar
- Windows tray tooltip text

### P1: Actions are still too generic for the underlying object type

Current evidence:

- Android timeline rows are fully clickable and always call `onApply(entry)`, even for links, files, and peer events.
  - `platforms/android/app/src/main/java/com/deskdrop/ui/MainScreen.kt:863-917`
- The dropdown action label is generic: `Open / Copy`.
  - `platforms/android/app/src/main/java/com/deskdrop/ui/MainScreen.kt:914-921`
- Android peer actions are mostly icon-only in the device list.
  - `platforms/android/app/src/main/java/com/deskdrop/ui/MainScreen.kt:1281-1352`
- Windows device management is still framed largely around connect/disconnect mechanics, not task intent.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml:344-387`

Why this matters:

- A link, a received file, clipboard text, and a device-availability event are not the same object.
- Giving them a shared action grammar makes the interface harder to predict.
- The user must remember what tapping something will do instead of recognizing it from the UI.

Theory behind this:

- Recognition rather than recall.
- Hick’s Law: generic menus increase choice interpretation cost.
- Fitts’s Law: icon-only controls raise target ambiguity, especially on touch.

Recommendation:

- Introduce typed primary actions:
  - clipboard text: `Copy Again`
  - clipboard image: `Copy Image`
  - link: `Open Link`
  - file received: `Show in Downloads`
  - file sent: `Send Again`
  - peer connected: `Open Device`
  - warning: `Fix Issue`
- Secondary actions can stay in menus, but primary affordance should be obvious and object-specific.
- On Android peer cards, replace or augment icon-only actions with visible labels during the untrusted state:
  - `Pair`
  - `Trust`
  - `Reject`

### P1: Recovery exists, but it is still informational rather than operational

Current evidence:

- Android diagnostics list states and suggestions, but do not offer direct repair actions.
  - `platforms/android/app/src/main/java/com/deskdrop/DiagnosticsActivity.kt:64-90`
- macOS diagnostics do the same.
  - `platforms/macos/Deskdrop/DiagnosticsView.swift:27-60`
- Windows uses tray balloons, inline toast, and overlays, but there is not yet a single "fix it from here" surface.
  - `platforms/windows/Deskdrop.Windows/Program.cs:478-485`
  - `platforms/windows/Deskdrop.Windows/Program.cs:538-572`
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml:439-493`

Why this matters:

- Status without repair forces context switching.
- Users don’t just need diagnosis; they need the shortest route back to success.

Recommendation:

- Upgrade diagnostics into a repair console.
- Every failing state should expose direct actions where the platform allows it:
  - `Scan again`
  - `Retry handshake`
  - `Open notification settings`
  - `Open battery optimization settings`
  - `Show QR`
  - `Copy magic link`
  - `Open downloads`
  - `Pause` / `Resume sync`
  - `Trust this device`
  - `Reject this device`
- When direct deep-linking is not possible, the UI should still provide step-by-step, context-specific instructions, not generic advice.

### P1: Cross-surface behavior is still fragmented

Current evidence:

- macOS now has dashboard, menu bar popover, onboarding, quick access, and diagnostics, but their roles are not fully differentiated.
  - `platforms/macos/Deskdrop/MenuBarPopoverView.swift:19-138`
- Windows still uses a mix of:
  - tray balloons,
  - inline dashboard toast,
  - in-window TOFU overlay,
  - tray menu actions.
  - `platforms/windows/Deskdrop.Windows/Program.cs:478-572`
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml:439-493`
- Android has onboarding, main dashboard, settings, and diagnostics, but state is not yet centralized.

Why this matters:

- A product with several surfaces needs a clear governance model:
  - what is persistent,
  - what is transient,
  - what is educational,
  - what is corrective,
  - what is background-only.
- Without that, feedback feels scattered even if each individual surface looks decent.

Recommendation:

- Define a cross-platform surface policy:
  - persistent state: inline banner / status rail
  - transient success after user action: toast
  - critical trust/destructive choice: modal or overlay
  - optional coaching: teaching tip / contextual hint
  - background presence: tray or menu bar only

### P2: Windows still lacks a real first-success journey

Current evidence:

- Android and macOS now have explicit onboarding.
- Windows still starts more like a utility shell with devices/settings/actions, not a first-success flow.
  - `platforms/windows/Deskdrop.Windows/MainWindow.xaml:276-387`

Why this matters:

- The Windows user still has to infer the main journey:
  discover -> connect -> trust -> send -> confirm.
- That is manageable for power users, but it is not yet beginner-safe.

Recommendation:

- Add a Windows first-run checklist in the dashboard:
  - `Find nearby device`
  - `Verify trust code`
  - `Send sample clipboard`
  - `Confirm where received items appear`
  - `Deskdrop stays available in the notification area`

## Journey-Level Redesign

The right way to think about Deskdrop now is not as a set of screens, but as a set of user questions.

### Stage 1: "Is Deskdrop ready?"

User question:

- Can I use this right now, or is something blocking it?

Current gap:

- Users often see a generic connected/idle state, not the real blocker.

Required design:

- a persistent health banner,
- a compact readiness checklist,
- one primary repair action.

### Stage 2: "Which device am I talking to?"

User question:

- Which nearby device is mine?

Current gap:

- Selection, connection, and trust still blur together.

Required design:

- explicit discovery state,
- clearer device identity,
- more deliberate pairing progression.

### Stage 3: "Can I trust this device?"

User question:

- Am I pairing with the correct machine?

Current gap:

- placeholder verification and auto-trust reduce certainty.

Required design:

- real short authentication string,
- reciprocal confirmation,
- visible failure/cancel path.

### Stage 4: "Did it actually work?"

User question:

- Was my clipboard or file really delivered, and where is it?

Current gap:

- some flows currently say yes before the system knows.

Required design:

- delivery receipts,
- platform-specific proof copy,
- immediate next action:
  - `Paste now`
  - `Open Downloads`
  - `Send another`

### Stage 5: "Can I repeat this with less effort?"

User question:

- Now that I’ve done it once, how do I do it quickly?

Current gap:

- the product still behaves like every send is a fresh decision.

Required design:

- remember last target,
- expose `Send to <last device>` as a first-class action,
- keep target switching easy but secondary.

### Stage 6: "What broke, and how do I fix it?"

User question:

- Why isn’t this working now, and what exact step should I take?

Current gap:

- diagnostics explain, but rarely repair.

Required design:

- one repair console,
- one state machine,
- one canonical source of truth for next-best action.

## Advanced Implementation Blueprint

Because you said complex implementations are acceptable, I would not solve the next round with isolated UI tweaks. I would introduce a small UX architecture layer.

### 1. `JourneyOrchestrator`

Responsibility:

- Own first-run and re-entry task progress.
- Bind UI steps to real engine events.

Inputs:

- peer discovery events
- connection events
- trust events
- clipboard delivery events
- file transfer events
- permission states

Outputs:

- step status: `locked`, `available`, `in_progress`, `verified`, `failed`
- next best action
- recovery suggestion

### 2. `SystemHealthState`

Responsibility:

- Normalize daemon, network, permission, trust, and delivery state into one user-facing model.

Inputs:

- daemon status
- local network permission
- notification permission
- battery optimization / idle restrictions
- peer count
- trust pending count
- active transfer state
- last successful sync timestamp

Outputs:

- title
- body
- severity
- primary CTA
- secondary CTA
- recommended surface

### 3. `PairingCeremonyState`

Responsibility:

- Make pairing explicit and safe.

States:

- `discovered`
- `connecting`
- `code_ready`
- `local_confirmed`
- `remote_confirmed`
- `trusted`
- `failed`
- `cancelled`

### 4. `DeliveryReceipt`

Responsibility:

- Distinguish between send initiation and user-visible outcome.

Suggested model:

- `queued`
- `sent_to_transport`
- `received_by_peer`
- `applied_automatically`
- `available_for_manual_apply`
- `failed`

This is especially important on Android, where clipboard behavior is constrained by OS policy and should not be implied as fully automatic when it is not.

### 5. `ActionTaxonomy`

Responsibility:

- Give each activity type its own verbs and affordances.

This should replace generic action labeling with a consistent mapping between object type and next step.

### 6. `NotificationPolicy`

Responsibility:

- Keep feedback channel usage coherent.

Recommended policy:

- inline banner / InfoBar: persistent state changes
- toast: user-triggered success confirmation
- modal / overlay: trust and destructive decisions
- tray/menu bar notification: background-only events that matter away from the main UI
- teaching tip: one-time guidance, never critical state

## Platform-Specific Direction

### Android

Highest-value next changes:

- Make onboarding step completion event-based, not button-based.
- Remove QR auto-trust.
- Replace placeholder verification code with a real trust code.
- Introduce a compact inline status card under the hero actions.
- Turn diagnostics into a repair console with deep links where possible.
- Make the activity list type-aware.

Additional note:

- The manifest still declares a broad set of sensitive capabilities even though runtime prompting is now more restrained.
  - `platforms/android/app/src/main/AndroidManifest.xml:37-47`
- That is not only a policy/privacy concern; it also affects trust perception when users inspect app details.

### macOS

Highest-value next changes:

- Keep onboarding optional and proof-driven.
- Use the same readiness/status language in dashboard, diagnostics, and menu bar popover.
- Make target selection more explicit when sending clipboard or files.
- Clarify destination semantics:
  - where files arrive,
  - what "clipboard sent" means,
  - whether the remote device auto-applies or presents an action.

The current macOS client is the closest to a strong power-user product. The next step is reducing ambiguity, not adding more surface area.

### Windows

Highest-value next changes:

- Add first-run onboarding or a first-success checklist in the dashboard.
- Use an in-window persistent status component for repairable issues.
- Reserve tray balloons for background notifications only.
- Add a one-time explanation that closing the window keeps Deskdrop alive in the notification area.
- Replace the fake IP placeholder with:
  - `Paste Magic Link`
  - `Scan network`
  - `Show QR`
  - and format help

Windows is much improved, but it still feels like a utility shell more than a guided experience.

## UX Theory Applied to Deskdrop

### Visibility of system status

Deskdrop should always tell the truth about:

- whether it is ready,
- what it is doing,
- and what the user should do next.

This argues for:

- persistent inline status,
- delivery-state specificity,
- proof-based onboarding.

### Recognition rather than recall

Deskdrop should not require users to remember:

- what each icon means,
- what a generic action will do,
- or why a device is pending vs trusted.

This argues for:

- typed action labels,
- clearer device-state labeling,
- in-context repair instructions.

### User control and freedom

Deskdrop should make it easy to:

- cancel pairing,
- defer permissions,
- pause sync,
- reject devices,
- undo risky actions when possible.

This argues against:

- silent trust,
- overly eager auto-advance,
- full-screen dead ends.

### Hick’s Law

The product should reduce interpretation cost by showing fewer, better-timed choices.

This argues for:

- progressive disclosure,
- beginner vs advanced separation,
- action menus that are context-specific.

### Fitts’s Law

Mobile actions should be easy to acquire physically, not just visually.

This argues for:

- larger labeled actions in trust/pairing flows,
- fewer tiny icon-only controls,
- less reliance on menus for primary actions.

### Goal-gradient effect

Progress should feel earned and motivating.

This argues for:

- visible completion markers tied to real events,
- optimistic micro-progress with later verification,
- avoiding fake success states.

### Tesler’s Law

Deskdrop cannot remove OS complexity, but it can decide who carries it.

This argues for:

- moving network/permission/background complexity into system diagnosis,
- not asking users to reverse-engineer platform limitations themselves.

## What To Measure Next

If you implement this round, measure behavior instead of just impressions.

Recommended product metrics:

- time to first verified send
- time to first trusted pair
- onboarding completion rate by verified step, not by screen progression
- false-complete rate:
  users marked complete who do not achieve a verified transfer within 5 minutes
- permission acceptance rate by context
- recovery success rate from each health state
- number of sends that go to the intended target on first attempt
- Windows tray-return rate after close
- Android failure rate by battery-restricted vs unrestricted devices

## Priority Roadmap

### P0

- Evidence-based onboarding
- Real trust ceremony
- Remove Android QR auto-trust
- Delivery receipt model

### P1

- Shared `SystemHealthState`
- Repair-console diagnostics
- Typed activity actions
- Windows first-success onboarding
- Cross-surface feedback policy

### P2

- Target memory and quick-send optimization
- Better long-term state/history semantics
- Cross-platform journey instrumentation dashboard

## Bottom Line

The product is no longer mainly suffering from "too much UI friction." It is now suffering from **too much interpretive burden**.

The user still has to answer several questions on their own:

- Is this actually paired?
- Is this actually trusted?
- Was that actually sent?
- Is the OS blocking me, or is Deskdrop broken?

The next great version of Deskdrop should make those answers explicit.

If you do only one thing next, make onboarding and status **truthful, event-driven, and shared across platforms**. That single move will improve trust, learning, supportability, and perceived polish more than another visual redesign.

## Sources

Platform guidance:

- [Android app permissions best practices](https://developer.android.com/training/permissions/usage-notes)
- [Android runtime permission workflow](https://developer.android.com/training/permissions/requesting)
- [Android notification runtime permission](https://developer.android.com/develop/ui/compose/notifications/notification-permission)
- [Android Doze and App Standby guidance](https://developer.android.com/training/monitoring-device-state/doze-standby)
- [Android 10 privacy changes and clipboard limits](https://developer.android.com/about/versions/10/privacy/changes)
- [Android local network permission](https://developer.android.com/privacy-and-security/local-network-permission)
- [Apple Human Interface Guidelines: Onboarding](https://developer.apple.com/design/human-interface-guidelines/onboarding)
- [Apple Human Interface Guidelines: Feedback](https://developer.apple.com/design/human-interface-guidelines/feedback)
- [Apple Human Interface Guidelines: Privacy](https://developer.apple.com/design/human-interface-guidelines/privacy/)
- [Apple Support: Control access to your local network on Mac](https://support.apple.com/guide/mac-help/control-access-to-your-local-network-on-mac-mchla4f49138/mac)
- [Microsoft Learn: InfoBar](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/infobar)
- [Microsoft Learn: TeachingTip](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/dialogs-and-flyouts/teaching-tip)
- [Microsoft Learn: Notifications and the notification area](https://learn.microsoft.com/en-us/windows/win32/shell/notification-area)
- [Microsoft Learn: Notification area UX guidance](https://learn.microsoft.com/en-us/windows/win32/uxguide/winenv-notification)

UX theory and heuristics:

- [Nielsen Norman Group: Ten usability heuristics summary](https://www.nngroup.com/articles/ten-usability-heuristics/)
- [NN/g heuristics summary PDF](https://media.nngroup.com/media/articles/attachments/Heuristic_Summary1_Letter-compressed.pdf)
- [Laws of UX: Goal-Gradient Effect](https://lawsofux.com/goal-gradient-effect/)
- [Laws of UX: Fitts’s Law](https://lawsofux.com/fittss-law/)
- [Laws of UX: Hick’s Law](https://lawsofux.com/hicks-law/)
- [Laws of UX: Tesler’s Law](https://lawsofux.com/teslers-law/)
- [Laws of UX: Onboarding for Active Users](https://lawsofux.com/articles/2024/onboarding-for-active-users/)
