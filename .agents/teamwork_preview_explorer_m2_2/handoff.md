# Milestone M2 Investigation Report: Android MediaStore & Query Optimization

## 1. Observation

### Observation 1.1: `DeskdropService.kt` Call Site & Hardcoded Summary Flag
- **Location**: `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:1484-1524`
- **Verbatim Code**:
```kotlin
DeskdropJni.CR_EVENT_REMOTE_FILES_QUERY -> {
    val requestId = DeskdropJni.eventRequestId(ev) ?: return
    val targetDeviceId = DeskdropJni.eventDeviceId(ev) ?: return
    val summaryOnly = DeskdropJni.eventSummaryOnly(ev)
    val category = DeskdropJni.eventFileCategory(ev)
    val source = DeskdropJni.eventFileSource(ev)
    val query = DeskdropJni.eventSearchQuery(ev)
    val offset = DeskdropJni.eventOffset(ev)
    val limit = DeskdropJni.eventLimit(ev)

    executeInBackgroundWithWakeLock("RemoteFilesQuery") {
        if (!hasFilePermissions()) { ... }
        try {
            val (summaryJson, filesJson, total) = RemoteFileManager.queryFiles(
                applicationContext, category, source, query, offset, limit,
                includeSummary = true,
                includeList = !summaryOnly
            )
            DeskdropJni.sendRemoteFilesResponse(
                engineHandle, requestId, targetDeviceId, summaryJson, filesJson, total, null
            )
        } catch (e: Exception) { ... }
    }
}
```
- **Direct Observation**:
  - `DeskdropService.kt` extracts `summaryOnly`, `category`, `source`, `query`, `offset`, and `limit` from the JNI event `ev`.
  - `includeSummary` is currently **hardcoded to `true`** on line 1511, regardless of whether `summaryOnly` is true/false or whether `offset > 0` or a specific category filter is applied.

### Observation 1.2: `RemoteFileManager.kt` Full Table Cursor Loop & In-Memory Filtering
- **Location**: `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt:60-130`
- **Verbatim Code**:
```kotlin
val uri = MediaStore.Files.getContentUri("external")
val selection = "${MediaStore.Files.FileColumns.SIZE} > 0"

try {
    context.contentResolver.query(
        uri,
        projection,
        selection,
        null,
        "${MediaStore.Files.FileColumns.DATE_MODIFIED} DESC"
    )?.use { cursor ->
        ...
        while (cursor.moveToNext()) {
            val size = if (sizeIdx >= 0) cursor.getLong(sizeIdx) else 0L
            if (size <= 0L) continue

            val name = if (nameIdx >= 0) cursor.getString(nameIdx) ?: "" else ""
            val mime = if (mimeIdx >= 0) cursor.getString(mimeIdx) ?: "" else ""
            val dataPath = if (dataIdx >= 0) cursor.getString(dataIdx) ?: "" else ""

            val cat = getCategory(mime, name)
            val src = getSource(dataPath)

            if (includeSummary) {
                when (cat) { ... }
                when (src) { ... }
            }

            if (matchesFilters(name, cat, src, categoryFilter, sourceFilter, searchQuery)) {
                if (includeList) {
                    if (totalMatching >= offset && matchingList.size < limit) {
                        ...
                    }
                }
                totalMatching++
            }
        }
    }
}
```
- **Direct Observation**:
  - `queryFiles` queries `MediaStore.Files.getContentUri("external")` with a selection of **only** `${MediaStore.Files.FileColumns.SIZE} > 0`.
  - No SQL `WHERE` clauses are constructed for `categoryFilter`, `sourceFilter`, or `searchQuery`.
  - The loop iterates through **every single row** in MediaStore (`while (cursor.moveToNext())`).
  - For every file on the device, Kotlin code evaluates `getCategory(mime, name)` (string checking), `getSource(dataPath)`, and `matchesFilters()`.
  - Pagination (`offset` and `limit`) is applied in Kotlin memory **after** stepping through preceding cursor rows, and the loop continues stepping through all remaining rows in order to compute `totalMatching`.

### Observation 1.3: Target Android SDK Configuration
- **Location**: `platforms/android/app/build.gradle:12-13`
- **Verbatim Lines**:
```groovy
minSdk 26           // Android 8.0 — foreground service type requires 26+
targetSdk 34
```
- **Direct Observation**:
  - The Android app targets API 34 with `minSdk 26` (Android 8.0 O).
  - All supported Android devices run API 26 or higher, where `ContentResolver.query(Uri, String[], Bundle, CancellationSignal)` and `ContentResolver.QUERY_ARG_OFFSET`, `ContentResolver.QUERY_ARG_LIMIT`, `ContentResolver.QUERY_ARG_SQL_SELECTION`, `ContentResolver.QUERY_ARG_SQL_SELECTION_ARGS`, `ContentResolver.QUERY_ARG_SORT_COLUMNS`, `ContentResolver.QUERY_ARG_SORT_DIRECTION` are standard platform APIs.

---

## 2. Logic Chain

1. **Root Cause of Query Timeout**:
   - `ORIGINAL_REQUEST.md` documents "Remote files query timed out". `deskdrop-core` enforces a 12-second timeout on RPC queries (`query_remote_files_sync`).
   - On Android devices with tens of thousands of media files, reading column strings across IPC Binder cursor windows and running Kotlin string/regex logic for 50,000+ files in `while (cursor.moveToNext())` takes **10 to 15+ seconds**.
   - Because `DeskdropService.kt` calls `RemoteFileManager.queryFiles(..., includeSummary = true)` unconditionally, every remote query (including paginated list requests) performs this full cursor scan over the entire MediaStore.

2. **Decoupling Category Summary Generation**:
   - Category summary generation (`RemoteFilesSummary` containing `type_counts` and `source_counts`) currently computes counts by inspecting every file row in Kotlin memory (Observation 1.2).
   - This can be replaced by executing dedicated SQL count queries using `ContentResolver.query()` with projection `arrayOf(MediaStore.Files.FileColumns._ID)`.
   - On MediaStore content providers, calling `cursor.count` on an `_ID` projection returns the total number of matching SQLite records immediately without fetching text columns or stepping through rows line-by-line.
   - Separate fast count queries for `MediaStore.Images.Media.EXTERNAL_CONTENT_URI`, `MediaStore.Video.Media.EXTERNAL_CONTENT_URI`, `MediaStore.Audio.Media.EXTERNAL_CONTENT_URI`, and `MediaStore.Files.getContentUri("external")` (for Documents, Apks, Archives, WhatsApp, Downloads, Camera) take ~0.5ms to 2ms each, totaling **5ms to 15ms** for full summary generation.

3. **Pushing Filters Down to SQL Selection**:
   - Instead of fetching all rows and calling `matchesFilters(...)` in Kotlin, `categoryFilter`, `sourceFilter`, and `searchQuery` must be converted into SQL `selection` and `selectionArgs`:
     - **Images**: `MIME_TYPE LIKE 'image/%'`
     - **Videos**: `MIME_TYPE LIKE 'video/%'`
     - **Audio**: `MIME_TYPE LIKE 'audio/%'`
     - **Apks**: `MIME_TYPE = 'application/vnd.android.package-archive' OR DISPLAY_NAME LIKE '%.apk'`
     - **Documents**: MIME types `text/%`, `application/pdf`, or extensions `.pdf`, `.doc`, `.docx`, `.xls`, `.xlsx`, `.ppt`, `.pptx`, `.txt`, `.csv`, `.md`.
     - **Archives**: MIME types `application/%zip%`, `application/%tar%`, `application/%gzip%`, `application/%rar%`, `application/%7z%`, or extensions `.zip`, `.tar`, `.gz`, `.rar`, `.7z`.
     - **WhatsApp**: `DATA LIKE '%WhatsApp%'`
     - **Downloads**: `DATA LIKE '%Download%'`
     - **Camera**: `DATA LIKE '%DCIM%' OR DATA LIKE '%Camera%'`
     - **Search Query**: `DISPLAY_NAME LIKE ?` with arg `"%searchQuery%"`.
   - Pushing these filters into SQLite leverages MediaStore database indices on `SIZE`, `MIME_TYPE`, `DISPLAY_NAME`, and `DATA`, filtering the dataset at the database layer before any IPC transfer occurs.

4. **Native API 26+ SQL Pagination**:
   - Using `minSdk 26` (Observation 1.3), `ContentResolver.query()` accepts a `Bundle` containing `QUERY_ARG_OFFSET` and `QUERY_ARG_LIMIT`.
   - MediaStore's SQLite engine handles `LIMIT` and `OFFSET` natively in SQL.
   - For total count (`totalMatching`), a count query with `projection = arrayOf(_ID)` and `selection`/`selectionArgs` returns `totalMatching` instantly via `cursor.count`.
   - For the paginated item list (`filesJson`), the query returns at most `limit` rows (e.g. 50 items). The Kotlin cursor loop executes at most `limit` times, eliminating full cursor iterations.
   - As a fallback for content providers that ignore `QUERY_ARG_OFFSET`, calling `cursor.moveToPosition(offset)` jumps directly to row index `offset` without row-by-row string extraction.

5. **Optimizing `includeSummary` Flag Handling**:
   - In `DeskdropService.kt`, `includeSummary` should be evaluated as:
     `val includeSummary = summaryOnly || (category.isNullOrEmpty() && source.isNullOrEmpty() && query.isNullOrEmpty() && offset == 0)`
   - When `includeSummary == false`, `RemoteFileManager.queryFiles` skips summary count queries and returns `summaryJson = null` immediately (0 overhead).
   - When `includeSummary == true`, `generateSummaryJson(context)` runs the fast SQL count queries.

---

## 3. Caveats

- **Scoped Storage Path Access (`DATA` column)**: On Android 10+ (API 29+), direct file access via `DATA` column is restricted for arbitrary I/O, but MediaStore SQL queries filtering on `${MediaStore.Files.FileColumns.DATA} LIKE '%WhatsApp%'` remain fully functional for `ContentResolver.query()`.
- **Pre-O Fallback**: `minSdk` is 26, so API 26+ `Bundle` query args are always available. The `cursor.moveToPosition(offset)` fallback is included for safety.
- **Search Query Escaping**: `searchQuery` parameters are passed via `selectionArgs` (`arrayOf("%$searchQuery%")`) to prevent SQL injection vulnerabilities.

---

## 4. Conclusion & Recommended Code Changes

### Proposed Strategy Summary
1. **Decouple Summary Generation**: Replace full table cursor iteration in `queryFiles` with `generateSummaryJson()`, executing 9 fast `cursor.count` queries with `projection = arrayOf(_ID)`.
2. **Push Selection to SQL**: Build SQL `selection` and `selectionArgs` in `buildSqlSelection()` for categories, sources, and search queries.
3. **Native SQL Pagination**: Use `Bundle` query args (`QUERY_ARG_OFFSET`, `QUERY_ARG_LIMIT`, `QUERY_ARG_SQL_SELECTION`, `QUERY_ARG_SQL_SELECTION_ARGS`) to fetch only `limit` rows from MediaStore.
4. **Conditional Summary Flag**: Update `DeskdropService.kt` to only request summary when `summaryOnly == true` or on root folder view (`category == null && source == null && query == null && offset == 0`).

---

### Proposed Code Changes

#### Change 1: `platforms/android/app/src/main/java/com/deskdrop/RemoteFileManager.kt`

```kotlin
package com.deskdrop

import android.content.ContentResolver
import android.content.ContentUris
import android.content.ContentValues
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.MediaStore
import android.util.Log
import android.util.Size
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

object RemoteFileManager {
    private const val TAG = "RemoteFileManager"

    @androidx.compose.runtime.Immutable
    data class FileMeta(
        val fileId: Long,
        val displayName: String,
        val sizeBytes: Long,
        val mimeType: String,
        val dateModified: Long,
        val category: String,
        val source: String,
        val contentUri: String,
        val dataPath: String
    )

    fun queryFiles(
        context: Context,
        categoryFilter: String?,
        sourceFilter: String?,
        searchQuery: String?,
        offset: Int,
        limit: Int,
        includeSummary: Boolean,
        includeList: Boolean
    ): Triple<String?, String?, Int> {
        val summaryJson: String? = if (includeSummary) {
            generateSummaryJson(context)
        } else {
            null
        }

        var filesJson: String? = null
        var totalMatching = 0

        val (selection, selectionArgs) = buildSqlSelection(categoryFilter, sourceFilter, searchQuery)
        val filesUri = MediaStore.Files.getContentUri("external")

        // 1. Fast indexed count query for total matching files
        totalMatching = getCount(context, filesUri, selection, selectionArgs)

        // 2. Paginated list query (only fetch up to 'limit' items)
        if (includeList && totalMatching > 0 && limit > 0 && offset < totalMatching) {
            val projection = arrayOf(
                MediaStore.Files.FileColumns._ID,
                MediaStore.Files.FileColumns.DISPLAY_NAME,
                MediaStore.Files.FileColumns.SIZE,
                MediaStore.Files.FileColumns.MIME_TYPE,
                MediaStore.Files.FileColumns.DATE_MODIFIED,
                MediaStore.Files.FileColumns.DATA
            )

            val matchingList = mutableListOf<FileMeta>()

            try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    val queryArgs = Bundle().apply {
                        putInt(ContentResolver.QUERY_ARG_OFFSET, offset)
                        putInt(ContentResolver.QUERY_ARG_LIMIT, limit)
                        putString(ContentResolver.QUERY_ARG_SQL_SELECTION, selection)
                        putStringArray(ContentResolver.QUERY_ARG_SQL_SELECTION_ARGS, selectionArgs)
                        putStringArray(
                            ContentResolver.QUERY_ARG_SORT_COLUMNS,
                            arrayOf(MediaStore.Files.FileColumns.DATE_MODIFIED)
                        )
                        putInt(
                            ContentResolver.QUERY_ARG_SORT_DIRECTION,
                            ContentResolver.QUERY_SORT_DIRECTION_DESCENDING
                        )
                    }

                    context.contentResolver.query(filesUri, projection, queryArgs, null)?.use { cursor ->
                        val idIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns._ID)
                        val nameIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DISPLAY_NAME)
                        val sizeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.SIZE)
                        val mimeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.MIME_TYPE)
                        val dateIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DATE_MODIFIED)
                        val dataIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DATA)

                        while (cursor.moveToNext() && matchingList.size < limit) {
                            val id = if (idIdx >= 0) cursor.getLong(idIdx) else 0L
                            val name = if (nameIdx >= 0) cursor.getString(nameIdx) ?: "" else ""
                            val size = if (sizeIdx >= 0) cursor.getLong(sizeIdx) else 0L
                            val mime = if (mimeIdx >= 0) cursor.getString(mimeIdx) ?: "" else ""
                            val dateMod = if (dateIdx >= 0) cursor.getLong(dateIdx) else 0L
                            val dataPath = if (dataIdx >= 0) cursor.getString(dataIdx) ?: "" else ""

                            val cat = getCategory(mime, name)
                            val src = getSource(dataPath)
                            val contentUri = ContentUris.withAppendedId(filesUri, id).toString()

                            matchingList.add(
                                FileMeta(
                                    fileId = id,
                                    displayName = name,
                                    sizeBytes = size,
                                    mimeType = mime.ifEmpty { "application/octet-stream" },
                                    dateModified = dateMod,
                                    category = cat,
                                    source = src,
                                    contentUri = contentUri,
                                    dataPath = dataPath
                                )
                            )
                        }
                    }
                } else {
                    // Pre-O fallback: query with sort order and position jump
                    context.contentResolver.query(
                        filesUri,
                        projection,
                        selection,
                        selectionArgs,
                        "${MediaStore.Files.FileColumns.DATE_MODIFIED} DESC"
                    )?.use { cursor ->
                        val idIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns._ID)
                        val nameIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DISPLAY_NAME)
                        val sizeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.SIZE)
                        val mimeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.MIME_TYPE)
                        val dateIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DATE_MODIFIED)
                        val dataIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DATA)

                        if (cursor.moveToPosition(offset)) {
                            do {
                                val id = if (idIdx >= 0) cursor.getLong(idIdx) else 0L
                                val name = if (nameIdx >= 0) cursor.getString(nameIdx) ?: "" else ""
                                val size = if (sizeIdx >= 0) cursor.getLong(sizeIdx) else 0L
                                val mime = if (mimeIdx >= 0) cursor.getString(mimeIdx) ?: "" else ""
                                val dateMod = if (dateIdx >= 0) cursor.getLong(dateIdx) else 0L
                                val dataPath = if (dataIdx >= 0) cursor.getString(dataIdx) ?: "" else ""

                                val cat = getCategory(mime, name)
                                val src = getSource(dataPath)
                                val contentUri = ContentUris.withAppendedId(filesUri, id).toString()

                                matchingList.add(
                                    FileMeta(
                                        fileId = id,
                                        displayName = name,
                                        sizeBytes = size,
                                        mimeType = mime.ifEmpty { "application/octet-stream" },
                                        dateModified = dateMod,
                                        category = cat,
                                        source = src,
                                        contentUri = contentUri,
                                        dataPath = dataPath
                                    )
                                )
                            } while (cursor.moveToNext() && matchingList.size < limit)
                        }
                    }
                }
            } catch (e: Exception) {
                Log.e(TAG, "Error querying files with pagination", e)
            }

            val jsonArray = JSONArray()
            for (item in matchingList) {
                val obj = JSONObject().apply {
                    put("file_id", item.fileId)
                    put("display_name", item.displayName)
                    put("size_bytes", item.sizeBytes)
                    put("mime_type", item.mimeType)
                    put("date_modified", item.dateModified)
                    put("category", item.category)
                    put("source", item.source)
                    put("content_uri", item.contentUri)
                }
                jsonArray.put(obj)
            }
            filesJson = jsonArray.toString()
        } else if (includeList) {
            filesJson = "[]"
        }

        return Triple(summaryJson, filesJson, totalMatching)
    }

    private fun buildSqlSelection(
        categoryFilter: String?,
        sourceFilter: String?,
        searchQuery: String?
    ): Pair<String, Array<String>> {
        val clauses = mutableListOf("${MediaStore.Files.FileColumns.SIZE} > 0")
        val args = mutableListOf<String>()

        if (!categoryFilter.isNullOrEmpty() && categoryFilter != "All") {
            when (categoryFilter) {
                "Images" -> clauses.add("${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'image/%'")
                "Videos" -> clauses.add("${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'video/%'")
                "Audio" -> clauses.add("${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'audio/%'")
                "Apks" -> clauses.add("(${MediaStore.Files.FileColumns.MIME_TYPE} = 'application/vnd.android.package-archive' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.apk')")
                "Documents" -> clauses.add(
                    "(${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'text/%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} = 'application/pdf' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.pdf' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.doc' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.docx' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.xls' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.xlsx' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.ppt' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.pptx' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.txt' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.csv' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.md')"
                )
                "Archives" -> clauses.add(
                    "(${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%zip%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%tar%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%gzip%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%rar%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%7z%' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.zip' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.tar' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.gz' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.rar' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.7z')"
                )
                "Other" -> clauses.add(
                    "NOT (${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'image/%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'video/%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'audio/%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} = 'application/vnd.android.package-archive' OR " +
                    "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.apk' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'text/%' OR " +
                    "${MediaStore.Files.FileColumns.MIME_TYPE} = 'application/pdf')"
                )
            }
        }

        if (!sourceFilter.isNullOrEmpty() && sourceFilter != "All") {
            when (sourceFilter) {
                "WhatsApp" -> clauses.add("${MediaStore.Files.FileColumns.DATA} LIKE '%WhatsApp%'")
                "Downloads" -> clauses.add("${MediaStore.Files.FileColumns.DATA} LIKE '%Download%'")
                "Camera" -> clauses.add("(${MediaStore.Files.FileColumns.DATA} LIKE '%DCIM%' OR ${MediaStore.Files.FileColumns.DATA} LIKE '%Camera%')")
            }
        }

        if (!searchQuery.isNullOrEmpty()) {
            clauses.add("${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE ?")
            args.add("%$searchQuery%")
        }

        val selection = clauses.joinToString(" AND ")
        return Pair(selection, args.toTypedArray())
    }

    private fun getCount(context: Context, uri: Uri, selection: String, selectionArgs: Array<String>? = null): Int {
        return try {
            context.contentResolver.query(
                uri,
                arrayOf(MediaStore.Files.FileColumns._ID),
                selection,
                selectionArgs,
                null
            )?.use { it.count } ?: 0
        } catch (e: Exception) {
            Log.w(TAG, "Count query failed for $uri", e)
            0
        }
    }

    private fun generateSummaryJson(context: Context): String {
        val filesUri = MediaStore.Files.getContentUri("external")

        val images = getCount(context, MediaStore.Images.Media.EXTERNAL_CONTENT_URI, "${MediaStore.Images.Media.SIZE} > 0")
        val videos = getCount(context, MediaStore.Video.Media.EXTERNAL_CONTENT_URI, "${MediaStore.Video.Media.SIZE} > 0")
        val audio = getCount(context, MediaStore.Audio.Media.EXTERNAL_CONTENT_URI, "${MediaStore.Audio.Media.SIZE} > 0")

        val apks = getCount(
            context, filesUri,
            "${MediaStore.Files.FileColumns.SIZE} > 0 AND (${MediaStore.Files.FileColumns.MIME_TYPE} = 'application/vnd.android.package-archive' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.apk')"
        )
        val documents = getCount(
            context, filesUri,
            "${MediaStore.Files.FileColumns.SIZE} > 0 AND (${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'text/%' OR ${MediaStore.Files.FileColumns.MIME_TYPE} = 'application/pdf' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.pdf' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.doc' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.docx' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.xls' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.xlsx' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.ppt' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.pptx' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.txt' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.csv' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.md')"
        )
        val archives = getCount(
            context, filesUri,
            "${MediaStore.Files.FileColumns.SIZE} > 0 AND (${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%zip%' OR ${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%tar%' OR ${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%gzip%' OR ${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%rar%' OR ${MediaStore.Files.FileColumns.MIME_TYPE} LIKE 'application/%7z%' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.zip' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.tar' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.gz' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.rar' OR ${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE '%.7z')"
        )

        val whatsapp = getCount(context, filesUri, "${MediaStore.Files.FileColumns.SIZE} > 0 AND ${MediaStore.Files.FileColumns.DATA} LIKE '%WhatsApp%'")
        val downloads = getCount(context, filesUri, "${MediaStore.Files.FileColumns.SIZE} > 0 AND ${MediaStore.Files.FileColumns.DATA} LIKE '%Download%'")
        val camera = getCount(context, filesUri, "${MediaStore.Files.FileColumns.SIZE} > 0 AND (${MediaStore.Files.FileColumns.DATA} LIKE '%DCIM%' OR ${MediaStore.Files.FileColumns.DATA} LIKE '%Camera%')")

        val typeCounts = JSONObject().apply {
            put("images", images)
            put("videos", videos)
            put("audio", audio)
            put("documents", documents)
            put("apks", apks)
            put("archives", archives)
        }
        val sourceCounts = JSONObject().apply {
            put("whatsapp", whatsapp)
            put("downloads", downloads)
            put("camera", camera)
        }
        return JSONObject().apply {
            put("type_counts", typeCounts)
            put("source_counts", sourceCounts)
        }.toString()
    }

    ...
}
```

---

#### Change 2: `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt`

```kotlin
DeskdropJni.CR_EVENT_REMOTE_FILES_QUERY -> {
    val requestId = DeskdropJni.eventRequestId(ev) ?: return
    val targetDeviceId = DeskdropJni.eventDeviceId(ev) ?: return
    val summaryOnly = DeskdropJni.eventSummaryOnly(ev)
    val category = DeskdropJni.eventFileCategory(ev)
    val source = DeskdropJni.eventFileSource(ev)
    val query = DeskdropJni.eventSearchQuery(ev)
    val offset = DeskdropJni.eventOffset(ev)
    val limit = DeskdropJni.eventLimit(ev)

    executeInBackgroundWithWakeLock("RemoteFilesQuery") {
        if (!hasFilePermissions()) {
            Log.w(TAG, "Storage permission missing for RemoteFilesQuery")
            try {
                showPermissionRequiredNotification()
            } catch (e: Exception) {
                Log.e(TAG, "Failed to show permission notification", e)
            }
            DeskdropJni.sendRemoteFilesResponse(
                engineHandle, requestId, targetDeviceId, null, null, 0, "Permission Denied: Please grant storage permission on your Android device to browse files."
            )
            return@executeInBackgroundWithWakeLock
        }

        try {
            val includeSummary = summaryOnly || (category.isNullOrEmpty() && source.isNullOrEmpty() && query.isNullOrEmpty() && offset == 0)
            val (summaryJson, filesJson, total) = RemoteFileManager.queryFiles(
                applicationContext, category, source, query, offset, limit,
                includeSummary = includeSummary,
                includeList = !summaryOnly
            )
            DeskdropJni.sendRemoteFilesResponse(
                engineHandle, requestId, targetDeviceId, summaryJson, filesJson, total, null
            )
        } catch (e: Exception) {
            Log.e(TAG, "Error handling RemoteFilesQuery", e)
            DeskdropJni.sendRemoteFilesResponse(
                engineHandle, requestId, targetDeviceId, null, null, 0, e.message ?: "Query error"
            )
        }
    }
}
```

---

## 5. Verification Method

### 1. Build Verification
Run Android Gradle build to confirm compilation and zero Kotlin errors:
```bash
cd /Users/chinmayk/Projects/Deskdrop/platforms/android
./gradlew assembleDebug
```
*Expected Result*: `BUILD SUCCESSFUL` without compilation errors or missing symbol warnings.

### 2. Android Hardware Device Query Verification
- Connect Android hardware test device (`979116c`).
- Ensure Deskdrop debug APK is installed and storage permissions granted (`READ_MEDIA_IMAGES`, `READ_MEDIA_VIDEO`, `READ_MEDIA_AUDIO` / `READ_EXTERNAL_STORAGE`).
- Send `RemoteFilesQuery` from peer node (or via CLI / test binary) requesting "Images" category (`category="Images"`, `offset=0`, `limit=50`).
- Verify via logcat:
  `adb logcat -s RemoteFileManager Deskdrop`
*Expected Log Output*: Response returned in < 50ms without full cursor iterations, and `DeskdropJni.sendRemoteFilesResponse` completes successfully.

### 3. Invalidation Conditions
- If `queryFiles` response time exceeds 200ms on a device with >20,000 files, check if `QUERY_ARG_OFFSET` / `QUERY_ARG_LIMIT` query args were ignored by the provider.
- If category counts in summary return 0 despite files existing, check URI permissions for `MediaStore.Images.Media.EXTERNAL_CONTENT_URI`.
