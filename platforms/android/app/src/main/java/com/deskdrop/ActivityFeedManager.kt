package com.deskdrop

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

enum class ActivityKind {
    CLIPBOARD_TEXT, CLIPBOARD_IMAGE, FILE_SENT, FILE_RECEIVED,
    FILE_TRANSFER_INCOMING, FILE_TRANSFER_PROGRESS, FILE_TRANSFER_COMPLETE,
    FILE_TRANSFER_FAILED, FILE_TRANSFER_PAUSED, FILE_TRANSFER_RESUMED,
    PEER_CONNECTED, PEER_DISCONNECTED, WARNING;
}

@androidx.compose.runtime.Immutable
data class ActivityEntry(
    val id: Long = System.nanoTime(),
    val timestamp: Long = System.currentTimeMillis(),
    val deviceName: String,
    val kind: ActivityKind,
    val preview: String,
    /** For clipboard items: the full text (may be empty for images). */
    val contentHash: String = "",
    /** True if this clipboard item has been applied to local clipboard. */
    val appliedLocally: Boolean = false,
    /** For file transfers: the transfer ID hex. */
    val transferId: String = "",
    /** For file transfers: total bytes. */
    val fileTotalBytes: Long = 0L,
    /** Transfer progress 0-100. */
    val progressPercent: Int = 0,
    /** Bytes written so far for an in-flight transfer. */
    val transferBytesReceived: Long = 0L,
    /** Bytes per second, or 0 if the engine has not estimated speed yet. */
    val transferSpeedBps: Long = 0L,
    /** Seconds remaining, or -1 if unknown. */
    val transferEtaSecs: Long = -1L,
    /** Final destination path (file transfers). */
    val destPath: String = ""
) {
    fun formattedLine(): String = when (kind) {
        ActivityKind.CLIPBOARD_TEXT  -> "[$deviceName] copied: $preview"
        ActivityKind.CLIPBOARD_IMAGE -> "[$deviceName] copied image"
        ActivityKind.FILE_SENT       -> "[$deviceName] sent file: $preview"
        ActivityKind.FILE_RECEIVED   -> "[$deviceName] file ready: $preview"
        ActivityKind.FILE_TRANSFER_INCOMING -> "[$deviceName] sending: $preview"
        ActivityKind.FILE_TRANSFER_PROGRESS -> "[$deviceName] $progressPercent% — $preview"
        ActivityKind.FILE_TRANSFER_PAUSED   -> "[$deviceName] paused — $preview"
        ActivityKind.FILE_TRANSFER_RESUMED  -> "[$deviceName] resumed — $preview"
        ActivityKind.FILE_TRANSFER_COMPLETE -> "[$deviceName] ✓ $preview"
        ActivityKind.FILE_TRANSFER_FAILED   -> "[$deviceName] ✗ transfer failed: $preview"
        ActivityKind.PEER_CONNECTED  -> "[$deviceName] Connected"
        ActivityKind.PEER_DISCONNECTED -> "[$deviceName] Disconnected"
        ActivityKind.WARNING         -> "$preview"
    }
    /** True if the user can tap "Apply" to write this to local clipboard. */
    val isApplicable: Boolean get() = kind == ActivityKind.CLIPBOARD_TEXT && !appliedLocally
}

object ActivityFeedManager {
    var ACTIVITY_FEED_MAX = 100

    private val _feedFlow = MutableStateFlow<List<ActivityEntry>>(emptyList())
    val feedFlow: StateFlow<List<ActivityEntry>> = _feedFlow.asStateFlow()

    fun isUserFacingActivity(kind: ActivityKind): Boolean = when (kind) {
        ActivityKind.FILE_RECEIVED,
        ActivityKind.FILE_SENT,
        ActivityKind.FILE_TRANSFER_INCOMING,
        ActivityKind.FILE_TRANSFER_PROGRESS,
        ActivityKind.FILE_TRANSFER_COMPLETE,
        ActivityKind.FILE_TRANSFER_FAILED,
        ActivityKind.FILE_TRANSFER_PAUSED,
        ActivityKind.FILE_TRANSFER_RESUMED,
        ActivityKind.CLIPBOARD_TEXT,
        ActivityKind.CLIPBOARD_IMAGE -> true
        else -> false
    }

    fun addToFeed(entry: ActivityEntry) {
        if (!isUserFacingActivity(entry.kind)) return
        _feedFlow.update { current ->
            val updated = buildList {
                add(entry)
                addAll(current)
            }
            if (updated.size > ACTIVITY_FEED_MAX) updated.take(ACTIVITY_FEED_MAX) else updated
        }
    }
    
    fun removeFromFeed(id: Long) {
        _feedFlow.update { current ->
            current.filterNot { it.id == id }
        }
    }

    fun updateFeedByTransferId(tid: String, transform: (ActivityEntry) -> ActivityEntry) {
        _feedFlow.update { current ->
            val idx = current.indexOfFirst { it.transferId == tid }
            if (idx != -1) {
                val mut = current.toMutableList()
                mut[idx] = transform(mut[idx])
                mut
            } else {
                current
            }
        }
    }

    fun getFeedSnapshot(): List<ActivityEntry> = _feedFlow.value
}
