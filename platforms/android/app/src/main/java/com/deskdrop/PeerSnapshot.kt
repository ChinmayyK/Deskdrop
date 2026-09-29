package com.deskdrop

import android.content.SharedPreferences
import org.json.JSONArray

const val PREF_PEER_SNAPSHOTS_JSON = "peer_snapshots_json"

/** An unpaired peer not seen for this long is treated as gone. */
const val NEARBY_WINDOW_SECS = 300L

@androidx.compose.runtime.Immutable
data class PeerSnapshot(
    val id: String,
    val name: String,
    val status: String,
    val trusted: Boolean,
    val remembered: Boolean,
    val autoConnect: Boolean,
    val explicitDisconnect: Boolean,
    val lastSeenSecs: Long?,
    val lastSyncSecs: Long?,
    val lastError: String?,
    val ip: String?,
    val pairingRequested: Boolean,
    val pairingPin: String?,
    val lifecycleState: String,
    val remoteSyncEnabled: Boolean,
) {
    val isConnected: Boolean get() = status == "connected" && trusted
    val isConnecting: Boolean get() = status == "connecting"
    val isReconnectable: Boolean get() = trusted && remembered && autoConnect && !isConnected && !explicitDisconnect
    val needsAttention: Boolean get() = status == "failed"
    /**
     * Whether the peer belongs in device lists. The core keeps every untrusted peer it has
     * ever discovered, so an unpaired one only counts while it has been seen recently.
     */
    val isListable: Boolean get() = trusted || pairingRequested || isConnecting ||
        (lastSeenSecs ?: 0L) >= System.currentTimeMillis() / 1000 - NEARBY_WINDOW_SECS
    val needsTrust: Boolean get() = !trusted && (needsAttention || status == "disconnected")
    val isRejected: Boolean get() = lastError?.contains("rejected", ignoreCase = true) == true ||
        lastError?.contains("not trusted", ignoreCase = true) == true
}

fun parsePeerSnapshots(raw: String?): List<PeerSnapshot> {
    if (raw.isNullOrBlank()) return emptyList()
    val array = runCatching { JSONArray(raw) }.getOrNull() ?: return emptyList()
    val uniquePeers = mutableMapOf<String, PeerSnapshot>()
    for (i in 0 until array.length()) {
        val obj = array.optJSONObject(i) ?: continue
        val id = obj.optString("id")
        if (id.isBlank()) continue
        val displayName = obj.optString("display_name")
        val friendlyName = obj.optString("friendly_name")
        val name = displayName.ifBlank { friendlyName }.ifBlank { "Unknown device" }
        
        val peer = PeerSnapshot(
            id = id,
            name = name,
            status = obj.optString("status", "disconnected"),
            trusted = obj.optBoolean("trusted", false),
            remembered = obj.optBoolean("remembered", true),
            autoConnect = obj.optBoolean("auto_connect", true),
            explicitDisconnect = obj.optBoolean("explicit_disconnect", false),
            lastSeenSecs = obj.takeIf { !it.isNull("last_seen") }?.optLong("last_seen"),
            lastSyncSecs = obj.takeIf { !it.isNull("last_sync") }?.optLong("last_sync"),
            lastError = obj.takeIf { !it.isNull("last_error") }?.optString("last_error"),
            ip = (obj.takeIf { !it.isNull("ips") }?.optJSONArray("ips")?.let { if (it.length() > 0) it.optString(0) else null } ?: if (!obj.isNull("ip")) obj.optString("ip") else null)
                // IPv4 peers arrive IPv6-mapped ("::ffff:192.168.1.5"); show the plain address.
                ?.removePrefix("::ffff:"),
            pairingRequested = obj.optBoolean("pairing_requested", false) || obj.optBoolean("outgoing_pairing_waiting", false),
            pairingPin = if (obj.isNull("pairing_pin")) null else obj.optString("pairing_pin"),
            lifecycleState = obj.optString("lifecycle_state", "discovered"),
            remoteSyncEnabled = obj.optBoolean("remote_sync_enabled", true),
        )
        
        val existing = uniquePeers[peer.id]
        if (existing == null) {
            uniquePeers[peer.id] = peer
        } else {
            val peerPriority = if (peer.isConnected) 2 else if (peer.isConnecting) 1 else 0
            val existingPriority = if (existing.isConnected) 2 else if (existing.isConnecting) 1 else 0
            
            if (peerPriority > existingPriority) {
                uniquePeers[peer.id] = peer
            } else if (peerPriority == existingPriority) {
                if ((peer.lastSeenSecs ?: 0) > (existing.lastSeenSecs ?: 0)) {
                    uniquePeers[peer.id] = peer
                }
            }
        }
    }
    return uniquePeers.values.sortedWith(
        compareBy<PeerSnapshot>(
            { if (it.isConnected) 0 else if (it.isConnecting) 1 else 2 },
            { it.name.lowercase() }
        )
    )
}

fun SharedPreferences.peerSnapshots(): List<PeerSnapshot> =
    parsePeerSnapshots(getString(PREF_PEER_SNAPSHOTS_JSON, null))
