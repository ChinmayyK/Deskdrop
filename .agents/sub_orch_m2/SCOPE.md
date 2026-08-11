# Scope: Milestone M2 (Android MediaStore & Query Optimization)

## Architecture & Responsibilities
- Target Files:
  - `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt`
  - `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`
- Key Goals:
  1. Add SQL selection filters for category / MIME-type / source to MediaStore queries.
  2. Add pagination (limit and offset) using MediaStore query args (`ContentResolver.QUERY_ARG_OFFSET`, `QUERY_ARG_LIMIT`) or cursor bounds.
  3. Optimize category summary generation so it uses fast SQL COUNT queries or efficient indexed projections instead of unindexed full table cursor iterations.
  4. Ensure Android compilation succeeds (`./gradlew assembleDebug` or `scripts/build-android.sh --debug`).
  5. Strictly eliminate full cursor scans for summary generation when serving remote file queries.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 2 | Android MediaStore Query Optimization | Add SQL category/MIME-type filtering, indexed count query, and pagination to `RemoteFileManager.kt` to prevent full cursor scans | M2 | PROJECT.md |

## Milestones & Work Items
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M2 | Android MediaStore & Query Optimization | Optimize `RemoteFileManager.kt` & `DeskdropService.kt` with SQL selection filters, fast summary counts, and pagination | None | DONE |

## Interface Contracts
- Input: Category (`Option<RemoteFileCategory>`), Source (`Option<RemoteFileSource>`), Search Query (`Option<String>`), Offset (`u32`), Limit (`u32`), `includeSummary` (`Boolean`), `includeList` (`Boolean`).
- Output: `(summaryJson: String?, filesJson: String, total: Int)`.

## Code Layout
- `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt`: Android MediaStore queries & response formatting.
- `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`: Android service event loop & JNI callbacks for remote file queries.
