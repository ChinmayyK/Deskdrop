package com.deskdrop

import android.content.ContentResolver
import android.content.ContentUris
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.util.Log
import android.util.Size
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

object RemoteFileManager {
    private const val TAG = "RemoteFileManager"

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

    fun queryFilesSummary(
        context: Context,
        categoryFilter: String?,
        sourceFilter: String?,
        searchQuery: String?
    ): Pair<String, Int> {
        var images = 0
        var videos = 0
        var audio = 0
        var documents = 0
        var apks = 0
        var archives = 0

        var whatsapp = 0
        var downloads = 0
        var camera = 0

        var totalMatching = 0

        val projection = arrayOf(
            MediaStore.Files.FileColumns._ID,
            MediaStore.Files.FileColumns.DISPLAY_NAME,
            MediaStore.Files.FileColumns.SIZE,
            MediaStore.Files.FileColumns.MIME_TYPE,
            MediaStore.Files.FileColumns.DATA
        )

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
                val idIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns._ID)
                val nameIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DISPLAY_NAME)
                val sizeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.SIZE)
                val mimeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.MIME_TYPE)
                val dataIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DATA)

                while (cursor.moveToNext()) {
                    val name = if (nameIdx >= 0) cursor.getString(nameIdx) ?: "" else ""
                    val size = if (sizeIdx >= 0) cursor.getLong(sizeIdx) else 0L
                    if (size <= 0L) continue
                    val mime = if (mimeIdx >= 0) cursor.getString(mimeIdx) ?: "" else ""
                    val dataPath = if (dataIdx >= 0) cursor.getString(dataIdx) ?: "" else ""

                    val cat = getCategory(mime, name)
                    val src = getSource(dataPath)

                    // Update global type and source counts for everything on device
                    when (cat) {
                        "Images" -> images++
                        "Videos" -> videos++
                        "Audio" -> audio++
                        "Documents" -> documents++
                        "Apks" -> apks++
                        "Archives" -> archives++
                    }
                    when (src) {
                        "WhatsApp" -> whatsapp++
                        "Downloads" -> downloads++
                        "Camera" -> camera++
                    }

                    if (matchesFilters(name, cat, src, categoryFilter, sourceFilter, searchQuery)) {
                        totalMatching++
                    }
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error querying files summary", e)
        }

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
        val summaryObj = JSONObject().apply {
            put("type_counts", typeCounts)
            put("source_counts", sourceCounts)
        }

        return Pair(summaryObj.toString(), totalMatching)
    }

    fun queryFilesList(
        context: Context,
        categoryFilter: String?,
        sourceFilter: String?,
        searchQuery: String?,
        offset: Int,
        limit: Int
    ): Pair<String, Int> {
        val matchingList = mutableListOf<FileMeta>()

        val projection = arrayOf(
            MediaStore.Files.FileColumns._ID,
            MediaStore.Files.FileColumns.DISPLAY_NAME,
            MediaStore.Files.FileColumns.SIZE,
            MediaStore.Files.FileColumns.MIME_TYPE,
            MediaStore.Files.FileColumns.DATE_MODIFIED,
            MediaStore.Files.FileColumns.DATA
        )

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
                val idIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns._ID)
                val nameIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DISPLAY_NAME)
                val sizeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.SIZE)
                val mimeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.MIME_TYPE)
                val dateIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DATE_MODIFIED)
                val dataIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DATA)

                while (cursor.moveToNext()) {
                    val id = if (idIdx >= 0) cursor.getLong(idIdx) else 0L
                    val name = if (nameIdx >= 0) cursor.getString(nameIdx) ?: "" else ""
                    val size = if (sizeIdx >= 0) cursor.getLong(sizeIdx) else 0L
                    if (size <= 0L) continue
                    val mime = if (mimeIdx >= 0) cursor.getString(mimeIdx) ?: "" else ""
                    val dateMod = if (dateIdx >= 0) cursor.getLong(dateIdx) else 0L
                    val dataPath = if (dataIdx >= 0) cursor.getString(dataIdx) ?: "" else ""

                    val cat = getCategory(mime, name)
                    if (cat == "Other" && categoryFilter != null && categoryFilter != "All") continue

                    val src = getSource(dataPath)

                    if (matchesFilters(name, cat, src, categoryFilter, sourceFilter, searchQuery)) {
                        val contentUri = ContentUris.withAppendedId(uri, id).toString()
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
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error querying files list", e)
        }

        val totalMatching = matchingList.size
        val pagedList = if (offset >= matchingList.size) {
            emptyList()
        } else {
            matchingList.subList(offset, (offset + limit).coerceAtMost(matchingList.size))
        }

        val jsonArray = JSONArray()
        for (item in pagedList) {
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

        return Pair(jsonArray.toString(), totalMatching)
    }

    fun getThumbnail(context: Context, fileId: Long, sizePx: Int): ByteArray? {
        val uri = ContentUris.withAppendedId(MediaStore.Files.getContentUri("external"), fileId)
        var bitmap: Bitmap? = null

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            try {
                bitmap = context.contentResolver.loadThumbnail(uri, Size(sizePx, sizePx), null)
            } catch (e: Exception) {
                Log.w(TAG, "loadThumbnail Q+ failed for uri $uri: ${e.message}")
            }
        }

        if (bitmap == null) {
            try {
                context.contentResolver.openInputStream(uri)?.use { stream ->
                    val options = BitmapFactory.Options().apply {
                        inJustDecodeBounds = true
                    }
                    BitmapFactory.decodeStream(stream, null, options)
                    options.inSampleSize = calculateInSampleSize(options, sizePx, sizePx)
                    options.inJustDecodeBounds = false
                    context.contentResolver.openInputStream(uri)?.use { stream2 ->
                        val rawBitmap = BitmapFactory.decodeStream(stream2, null, options)
                        if (rawBitmap != null) {
                            bitmap = Bitmap.createScaledBitmap(rawBitmap, sizePx, sizePx, true)
                            if (bitmap != rawBitmap) rawBitmap.recycle()
                        }
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "BitmapFactory fallback failed for uri $uri: ${e.message}")
            }
        }

        return bitmap?.let { bmp ->
            val bos = ByteArrayOutputStream()
            bmp.compress(Bitmap.CompressFormat.JPEG, 80, bos)
            bmp.recycle()
            bos.toByteArray()
        }
    }

    fun resolveFilePathAndMeta(context: Context, fileId: Long): Triple<String, String, String>? {
        val projection = arrayOf(
            MediaStore.Files.FileColumns.DISPLAY_NAME,
            MediaStore.Files.FileColumns.MIME_TYPE,
            MediaStore.Files.FileColumns.DATA
        )
        val uri = ContentUris.withAppendedId(MediaStore.Files.getContentUri("external"), fileId)
        try {
            context.contentResolver.query(uri, projection, null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val nameIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DISPLAY_NAME)
                    val mimeIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.MIME_TYPE)
                    val dataIdx = cursor.getColumnIndex(MediaStore.Files.FileColumns.DATA)

                    val name = if (nameIdx >= 0) cursor.getString(nameIdx) ?: "" else ""
                    val mime = if (mimeIdx >= 0) cursor.getString(mimeIdx) ?: "" else ""
                    val path = if (dataIdx >= 0) cursor.getString(dataIdx) ?: "" else ""

                    if (path.isNotEmpty() && File(path).exists()) {
                        return Triple(
                            path,
                            name.ifEmpty { File(path).name },
                            mime.ifEmpty { "application/octet-stream" }
                        )
                    }
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error resolving file path for id $fileId", e)
        }
        return null
    }

    private fun getCategory(mime: String, name: String): String {
        val lowerMime = mime.lowercase()
        val lowerName = name.lowercase()
        return when {
            lowerMime.startsWith("image/") -> "Images"
            lowerMime.startsWith("video/") -> "Videos"
            lowerMime.startsWith("audio/") -> "Audio"
            lowerMime == "application/vnd.android.package-archive" || lowerName.endsWith(".apk") -> "Apks"
            lowerMime in listOf(
                "application/zip", "application/x-zip-compressed", "application/x-tar",
                "application/gzip", "application/x-rar-compressed", "application/x-7z-compressed"
            ) || lowerName.endsWith(".zip") || lowerName.endsWith(".tar") || lowerName.endsWith(".gz") ||
                    lowerName.endsWith(".rar") || lowerName.endsWith(".7z") -> "Archives"
            lowerMime == "application/pdf" || lowerMime.startsWith("text/") ||
                    lowerName.endsWith(".pdf") || lowerName.endsWith(".doc") || lowerName.endsWith(".docx") ||
                    lowerName.endsWith(".xls") || lowerName.endsWith(".xlsx") || lowerName.endsWith(".ppt") ||
                    lowerName.endsWith(".pptx") || lowerName.endsWith(".txt") || lowerName.endsWith(".csv") ||
                    lowerName.endsWith(".md") -> "Documents"
            else -> "Other"
        }
    }

    private fun getSource(dataPath: String): String {
        val lower = dataPath.lowercase()
        return when {
            lower.contains("whatsapp") -> "WhatsApp"
            lower.contains("download") -> "Downloads"
            lower.contains("dcim") || lower.contains("camera") -> "Camera"
            else -> "All"
        }
    }

    private fun matchesFilters(
        name: String,
        category: String,
        source: String,
        categoryFilter: String?,
        sourceFilter: String?,
        searchQuery: String?
    ): Boolean {
        if (!categoryFilter.isNullOrEmpty() && categoryFilter != "All" && category != categoryFilter) {
            return false
        }
        if (!sourceFilter.isNullOrEmpty() && sourceFilter != "All" && source != sourceFilter) {
            return false
        }
        if (!searchQuery.isNullOrEmpty() && !name.contains(searchQuery, ignoreCase = true)) {
            return false
        }
        return true
    }

    private fun calculateInSampleSize(options: BitmapFactory.Options, reqWidth: Int, reqHeight: Int): Int {
        val (height: Int, width: Int) = options.outHeight to options.outWidth
        var inSampleSize = 1
        if (height > reqHeight || width > reqWidth) {
            val halfHeight: Int = height / 2
            val halfWidth: Int = width / 2
            while (halfHeight / inSampleSize >= reqHeight && halfWidth / inSampleSize >= reqWidth) {
                inSampleSize *= 2
            }
        }
        return inSampleSize
    }
}
