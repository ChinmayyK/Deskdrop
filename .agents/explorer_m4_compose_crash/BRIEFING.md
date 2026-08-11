# BRIEFING — 2026-08-07T01:56:30Z

## Mission
Investigate Jetpack Compose UI crash `java.lang.IllegalStateException: Release should only be called once` in Deskdrop Android app and formulate a clean structural fix strategy.

## 🔒 My Identity
- Archetype: Teamwork Explorer
- Roles: Read-only investigation, root cause analysis, structural fix strategy
- Working directory: /Users/chinmayk/Projects/Deskdrop/.agents/explorer_m4_compose_crash
- Original parent: 6496732b-79a1-43aa-8316-4b84411d6818
- Milestone: Milestone 4 (Compose UI Crash Root Cause & Fix Strategy)

## 🔒 Key Constraints
- Read-only investigation — do NOT write or modify application source code directly
- Produce detailed investigation report and fix recommendation in handoff.md
- Message parent when done

## Current Parent
- Conversation ID: 6496732b-79a1-43aa-8316-4b84411d6818
- Updated: 2026-08-07T01:56:30Z

## Investigation State
- **Explored paths**: `MainScreen.kt`, `SettingsScreen.kt`, `OnboardingScreen.kt`, `PairingScreen.kt`, `MainActivity.kt`
- **Key findings**: Root cause pinpointed to `DropdownMenu` composable in `DeviceCard` (`MainScreen.kt:1402`) rendered inside a `LazyRow` (`MainScreen.kt:559-575`). When Monkey rapidly interacts/scrolls lazy items, `DropdownMenu` popup window disposal triggers focus node invalidation while `LazyLayoutPinnableItem` handle is already unpinned, causing double release `IllegalStateException`.
- **Unexplored areas**: None, scope fully investigated.

## Key Decisions Made
- Formulate dual structural fix strategy: (1) Decouple popup compositions from `LazyLayoutPinnableItem` by overriding `LocalPinnableContainer provides null` for popups/dropdowns, and (2) Hoist popup state outside lazy layout item composables to top-level screen containers.

## Artifact Index
- handoff.md — Comprehensive investigation report & structural fix strategy
- progress.md — Step-by-step progress tracking
- DISPATCH.md — Received tasks and updates
