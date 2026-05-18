package com.cliprelay

import android.content.SharedPreferences
import org.json.JSONArray

const val PREF_PEER_SNAPSHOTS_JSON = "peer_snapshots_json"

data class PeerSnapshot(
    val id: String,
    val name: String,
    val status: String,
    val trusted: Boolean,
    val remembered: Boolean,
    val autoConnect: Boolean,
    val lastSeenSecs: Long?,
    val lastSyncSecs: Long?,
    val lastError: String?,
) {
    val isConnected: Boolean get() = status == "connected"
    val isConnecting: Boolean get() = status == "connecting"
    val isReconnectable: Boolean get() = trusted && remembered && autoConnect
    val needsAttention: Boolean get() = status == "failed"
    val needsTrust: Boolean get() = !trusted && (needsAttention || status == "disconnected")
    val isRejected: Boolean get() = lastError?.contains("rejected", ignoreCase = true) == true ||
        lastError?.contains("not trusted", ignoreCase = true) == true
}

fun parsePeerSnapshots(raw: String?): List<PeerSnapshot> {
    if (raw.isNullOrBlank()) return emptyList()
    val array = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
    val peers = ArrayList<PeerSnapshot>(array.length())
    for (i in 0 until array.length()) {
        val obj = array.optJSONObject(i) ?: continue
        val id = obj.optString("id")
        if (id.isBlank()) continue
        val displayName = obj.optString("display_name")
        val friendlyName = obj.optString("friendly_name")
        peers += PeerSnapshot(
            id = id,
            name = displayName.ifBlank { friendlyName }.ifBlank { "Unknown device" },
            status = obj.optString("status", "disconnected"),
            trusted = obj.optBoolean("trusted", false),
            remembered = obj.optBoolean("remembered", true),
            autoConnect = obj.optBoolean("auto_connect", true),
            lastSeenSecs = obj.takeIf { !it.isNull("last_seen") }?.optLong("last_seen"),
            lastSyncSecs = obj.takeIf { !it.isNull("last_sync") }?.optLong("last_sync"),
            lastError = obj.takeIf { !it.isNull("last_error") }?.optString("last_error"),
        )
    }
    return peers.sortedWith(
        compareBy<PeerSnapshot>(
            { if (it.isConnected) 0 else if (it.isConnecting) 1 else 2 },
            { it.name.lowercase() }
        )
    )
}

fun SharedPreferences.peerSnapshots(): List<PeerSnapshot> =
    parsePeerSnapshots(getString(PREF_PEER_SNAPSHOTS_JSON, null))
