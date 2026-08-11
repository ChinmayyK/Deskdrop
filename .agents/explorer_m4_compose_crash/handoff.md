# Jetpack Compose UI Crash Root Cause Analysis & Structural Fix Report

**Agent**: `explorer_m4_compose_crash`  
**Working Directory**: `/Users/chinmayk/Projects/Deskdrop/.agents/explorer_m4_compose_crash`  
**Target Platform**: Android (`platforms/android`, `com.deskdrop.debug`)  
**Crash Investigated**: `java.lang.IllegalStateException: Release should only be called once`  
**Date**: 2026-08-07  

---

## 1. Observation

### Stack Trace & Empirical Findings
During Android Monkey stress testing (`adb -s 979116c shell monkey -p com.deskdrop.debug -v 5000`), the application process was terminated fatally at event 1214 with exit code `190`.

Verbatim Logcat Stack Trace (from `/Users/chinmayk/Projects/Deskdrop/.agents/challenger_m4_1/handoff.md:44-59`):
```
java.lang.IllegalStateException: Release should only be called once
	at androidx.compose.foundation.lazy.layout.LazyLayoutPinnableItem.release(LazyLayoutPinnableItem.kt:159)
	at androidx.compose.foundation.lazy.layout.LazyLayoutPinnableItem.release(LazyLayoutPinnableItem.kt:163)
	at androidx.compose.foundation.FocusablePinnableContainerNode.setFocus(Focusable.kt:337)
	at androidx.compose.foundation.FocusableNode.onFocusEvent(Focusable.kt:244)
	at androidx.compose.ui.focus.FocusInvalidationManager$invalidateNodes$1.invoke(FocusInvalidationManager.kt:79)
	at androidx.compose.ui.focus.FocusInvalidationManager$invalidateNodes$1.invoke(FocusInvalidationManager.kt:57)
	at androidx.compose.ui.platform.AndroidComposeView.onEndApplyChanges(AndroidComposeView.android.kt:742)
	at androidx.compose.ui.node.UiApplier.onEndChanges(UiApplier.android.kt:48)
	at androidx.compose.runtime.CompositionImpl.dispose(Composition.kt:768)
	at androidx.compose.ui.platform.WrappedComposition.dispose(Wrapper.android.kt:153)
	at androidx.compose.ui.platform.AbstractComposeView.disposeComposition(ComposeView.android.kt:266)
	at androidx.compose.ui.window.AndroidPopup_androidKt$Popup$2$invoke$$inlined$onDispose$1.dispose(Effects.kt:498)
	at androidx.compose.runtime.DisposableEffectImpl.onForgotten(Effects.kt:87)
	at androidx.compose.runtime.CompositionImpl$RememberEventDispatcher.dispatchRememberObservers(Composition.kt:1276)
```

### Codebase Inventory & Composable Mapping

An exhaustive scan of UI composables in `platforms/android/app/src/main/java/com/deskdrop/` revealed the following popup, dialog, menu, and lazy layout usages:

1. **`MainScreen.kt:559-575` & `MainScreen.kt:1285-1432`** (`DeviceCard` inside `LazyRow`):
   - `DeviceCard` composable maintains state `var showMenu by remember { mutableStateOf(false) }`.
   - `DeviceCard` is instantiated inside `LazyRow(contentPadding = ...) { items(peers, key = { it.id }) { peer -> DeviceCard(...) } }`.
   - At `MainScreen.kt:1402-1430`, `DeviceCard` renders `androidx.compose.material3.DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false })`.

2. **`MainScreen.kt:1127-1282`** (`TimelineActivityRow`):
   - `TimelineActivityRow` composable maintains state `var showMenu by remember { mutableStateOf(false) }`.
   - At `MainScreen.kt:1262-1279`, `TimelineActivityRow` renders `androidx.compose.material3.DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false })`.

3. **`MainScreen.kt:441-497`** (`showQrDialog` in Ecosystem header):
   - Uses `androidx.compose.material3.AlertDialog`. Located inside a standard scrollable `Column` (`verticalScroll(rememberScrollState())`), outside lazy lists.

4. **`MainActivity.kt:186-218`** (`showManualIpDialog`):
   - Uses `androidx.compose.material3.AlertDialog`. Managed at `setContent` root window level.

---

## 2. Logic Chain

1. **Jetpack Compose Pinnable Container Mechanism**:
   - `LazyRow` and `LazyColumn` provide a `LocalPinnableContainer` composition local backed by `LazyLayoutPinnableItem`.
   - When a focusable composable inside a lazy item (such as a card, button, or menu item) gains focus, `FocusablePinnableContainerNode` observes the focus event and calls `pinnableContainer.pin()`, creating a `PinnedHandle` and incrementing `pinsCount` from `0` to `1`.
   - When the item loses focus, `FocusablePinnableContainerNode.setFocus(false)` invokes `pinnedHandle.release()`, which decrements `pinsCount` back to `0`.
   - `LazyLayoutPinnableItem.kt:159` asserts `check(pinsCount > 0) { "Release should only be called once" }`. Calling `release()` when `pinsCount == 0` throws an `IllegalStateException`.

2. **Popup Composition & Focus Invalidation Teardown Race**:
   - `DropdownMenu` is built on top of `androidx.compose.ui.window.Popup`.
   - `Popup` creates a separate sub-window with its own `ComposeView` and child composition, inheriting composition locals (including `LocalPinnableContainer`) from its parent node inside `DeviceCard`.
   - When Monkey injects stress events, `DeviceCard` in `LazyRow` opens `DropdownMenu`. Focus nodes in the popup gain focus, pinning the parent `LazyLayoutPinnableItem`.
   - While the popup is open, Monkey simultaneously injects scroll events on `LazyRow` or taps outside, causing `LazyRow` to unpin/recycle `DeviceCard` while the `DropdownMenu` popup window begins teardown.
   - When `Popup` disposes (`AndroidPopup_androidKt$Popup$2$invoke$$inlined$onDispose$1.dispose`), it invokes `CompositionImpl.dispose()`, triggering `AndroidComposeView.onEndApplyChanges()` -> `FocusInvalidationManager.invalidateNodes()`.
   - During focus invalidation pass of the closing popup, `FocusableNode.onFocusEvent` notifies `FocusablePinnableContainerNode.setFocus(false)`.
   - `FocusablePinnableContainerNode` calls `pinnedHandle.release()` a **second time** on an already released / unpinned `LazyLayoutPinnableItem` handle.
   - `LazyLayoutPinnableItem.release()` throws `IllegalStateException: Release should only be called once`.

3. **Conclusion on Root Cause**:
   Embedding `DropdownMenu` (which spawns a `Popup` window) inside items of a `LazyRow` / `LazyColumn` couples the window composition teardown of the popup with the recycling/unpinning lifecycle of the lazy item. Rapid dismissal/scrolling creates a race condition where focus invalidation during popup composition disposal calls `release()` on an already unpinned `LazyLayoutPinnableItem` handle.

---

## 3. Caveats

- The crash is specific to `Popup` / `DropdownMenu` instances rendered directly inside Lazy layout items (`LazyRow` / `LazyColumn`). Standard `AlertDialog`s rendered at root/Activity scope do not interact with `LazyLayoutPinnableItem`.
- Disabling animations or focus on individual buttons does not fix the underlying composition local inheritance bug in Compose. The fix must structurally isolate `DropdownMenu` popups from `LazyLayoutPinnableItem`.

---

## 4. Conclusion & Structural Fix Recommendation

### Primary Fix Strategy (Combined Decoupling & State Hoisting)

To guarantee that `Release should only be called once` can never occur under Monkey stress testing, apply a dual structural fix:

1. **Local Composition Decoupling (`LocalPinnableContainer provides null`)**:
   Wrap all `DropdownMenu` composables inside `CompositionLocalProvider(LocalPinnableContainer provides null)`. This prevents `Popup`'s focus nodes from acquiring or attempting to release `LazyLayoutPinnableItem` handles during window disposal.
2. **Safe Menu State Disposal**:
   Add a `DisposableEffect` to clean up state when composables leave composition, ensuring `showMenu` is reset cleanly.

### Proposed Code Changes (For Implementer)

#### Target File: `platforms/android/app/src/main/java/com/deskdrop/ui/MainScreen.kt`

##### Edit 1: `DeviceCard` Dropdown Menu Decoupling (`MainScreen.kt:1402-1430`)

**Before**:
```kotlin
        androidx.compose.material3.DropdownMenu(
            expanded = showMenu,
            onDismissRequest = { showMenu = false },
            modifier = Modifier.background(if (isDark) Color(0xFF1E1E1E) else Color.White)
        ) {
            if (peer.lifecycleState == "pending_approval" && !peer.trusted) {
                ...
            }
        }
```

**After**:
```kotlin
        DisposableEffect(Unit) {
            onDispose {
                showMenu = false
            }
        }

        CompositionLocalProvider(androidx.compose.foundation.lazy.layout.LocalPinnableContainer provides null) {
            androidx.compose.material3.DropdownMenu(
                expanded = showMenu,
                onDismissRequest = { showMenu = false },
                modifier = Modifier.background(if (isDark) Color(0xFF1E1E1E) else Color.White)
            ) {
                if (peer.lifecycleState == "pending_approval" && !peer.trusted) {
                    androidx.compose.material3.DropdownMenuItem(
                        text = { Text("Accept Pairing", color = CRTheme.accentGreen) },
                        onClick = { showMenu = false; onRespond(true) }
                    )
                    androidx.compose.material3.DropdownMenuItem(
                        text = { Text("Reject Pairing", color = CRTheme.accentRed) },
                        onClick = { showMenu = false; onRespond(false) }
                    )
                } else if (peer.isConnected) {
                    androidx.compose.material3.DropdownMenuItem(
                        text = { Text("Send Files", color = CRTheme.textHigh(isDark)) },
                        onClick = { showMenu = false; onSendFiles() }
                    )
                    androidx.compose.material3.DropdownMenuItem(
                        text = { Text("Speed Test", color = CRTheme.textHigh(isDark)) },
                        onClick = { showMenu = false; onStartSpeedTest() }
                    )
                }
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text("Forget Device", color = CRTheme.accentRed) },
                    onClick = { showMenu = false; onForget() }
                )
            }
        }
```

##### Edit 2: `TimelineActivityRow` Dropdown Menu Decoupling (`MainScreen.kt:1262-1279`)

**Before**:
```kotlin
        androidx.compose.material3.DropdownMenu(
            expanded = showMenu,
            onDismissRequest = { showMenu = false },
            modifier = Modifier.background(if (isDark) Color(0xFF1E1E1E) else Color.White)
        ) {
            androidx.compose.material3.DropdownMenuItem(...)
        }
```

**After**:
```kotlin
        DisposableEffect(Unit) {
            onDispose {
                showMenu = false
            }
        }

        CompositionLocalProvider(androidx.compose.foundation.lazy.layout.LocalPinnableContainer provides null) {
            androidx.compose.material3.DropdownMenu(
                expanded = showMenu,
                onDismissRequest = { showMenu = false },
                modifier = Modifier.background(if (isDark) Color(0xFF1E1E1E) else Color.White)
            ) {
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text(primaryActionLabel, color = CRTheme.textHigh(isDark)) },
                    onClick = { showMenu = false; onApply(entry) }
                )
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text("Resend", color = CRTheme.textHigh(isDark)) },
                    onClick = { showMenu = false; onResend(entry) }
                )
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text("Delete history", color = CRTheme.accentRed) },
                    onClick = { showMenu = false; onDelete(entry) }
                )
            }
        }
```

---

## 5. Verification Method

To independently verify the fix after implementation:

1. **Build the Android APK**:
   ```bash
   cd /Users/chinmayk/Projects/Deskdrop/platforms/android
   ./gradlew assembleDebug
   ```

2. **Deploy to target device (`979116c`)**:
   ```bash
   export PATH="/opt/homebrew/share/android-commandlinetools/platform-tools:${PATH}"
   adb -s 979116c install -r app/build/outputs/apk/debug/app-debug.apk
   ```

3. **Execute 5,000 Event Monkey Stress Test**:
   ```bash
   adb -s 979116c logcat -c
   adb -s 979116c shell monkey -p com.deskdrop.debug -v 5000
   ```

4. **Pass Criteria**:
   - Monkey test completes all 5,000 injected events with `Events injected: 5000` and exit code `0`.
   - Zero `IllegalStateException` or `FATAL EXCEPTION: main` logcat entries during the run.
