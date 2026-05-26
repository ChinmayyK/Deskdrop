// Deskdrop — Android Foreground Service
//
// Background execution strategy:
//   - Foreground service (mandatory, stays alive across screen-off + OEM killers)
//   - WakeLock (PARTIAL) held only during active event drain — released immediately after
//   - Doze/standby aware: heartbeat poll rate reduced in Battery Optimized mode
//   - Single IMPORTANCE_MIN persistent notification — silent, no heads-up, no badge
//   - Alerts channel (IMPORTANCE_DEFAULT) for trust requests + file receives only
//   - Zero per-clipboard-sync notifications — clipboard is ambient/invisible
//   - Notification actions: Pause Sync | Disconnect
//   - Activity feed (in-memory) replaces notification spam

package com.deskdrop

import android.app.*
import android.content.*
import android.content.ClipboardManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.Uri
import android.content.pm.ServiceInfo
import android.os.*
import android.provider.OpenableColumns
import android.provider.Settings
import android.util.Log
import android.webkit.MimeTypeMap
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import java.util.UUID

// ── JNI Bridge ────────────────────────────────────────────────────────────────
// The prebuilt .so exports Java_com_deskdrop_DeskdropJni_* symbols.
// We keep this object name to match — only user-visible strings are renamed.

object DeskdropJni {
    init { System.loadLibrary("deskdrop_core") }

    // ── Event type constants ──────────────────────────────────────────────────
    const val CR_EVENT_NONE                  = 0
    const val CR_EVENT_CLIPBOARD_TEXT        = 1   // auto-applied to local clipboard
    const val CR_EVENT_CLIPBOARD_IMAGE       = 2   // auto-applied
    const val CR_EVENT_CLIPBOARD_FILE        = 3   // auto-applied (legacy)
    const val CR_EVENT_TOFU_PROMPT           = 4
    const val CR_EVENT_PEER_CONNECTED        = 5
    const val CR_EVENT_PEER_DISCONNECTED     = 6
    const val CR_EVENT_WARNING               = 7
    const val CR_EVENT_CLIPBOARD_SYNCED      = 8
    // 9, 10 reserved
    const val CR_EVENT_CLIPBOARD_AVAILABLE   = 11  // timeline-first: in feed, not yet applied
    const val CR_EVENT_FILE_TRANSFER_INCOMING  = 12
    const val CR_EVENT_FILE_TRANSFER_PROGRESS  = 13
    const val CR_EVENT_FILE_TRANSFER_COMPLETE  = 14
    const val CR_EVENT_FILE_TRANSFER_FAILED    = 15
    const val CR_EVENT_FILE_TRANSFER_PAUSED    = 20
    const val CR_EVENT_FILE_TRANSFER_RESUMED   = 21
    const val CR_EVENT_ACTIVITY_UPDATED        = 16
    const val CR_EVENT_CALL_STATE_CHANGED       = 17
    const val CR_EVENT_CALL_ACTION              = 18
    const val CR_EVENT_BATTERY_STATE_CHANGED    = 19

    // ── Core engine ───────────────────────────────────────────────────────────
    @JvmStatic external fun start(deviceName: String?, port: Int, dataDir: String?, fileSaveDir: String?): Long
    @JvmStatic external fun stop(handle: Long)

    // ── Clipboard push ────────────────────────────────────────────────────────
    @JvmStatic external fun pushText(handle: Long, text: String): Int
    @JvmStatic external fun pushImage(handle: Long, mimeType: String, data: ByteArray): Int
    @JvmStatic external fun pushFile(handle: Long, name: String, data: ByteArray): Int
    @JvmStatic external fun pushNotification(handle: Long, id: String, packageName: String, title: String, text: String): Int
    @JvmStatic external fun pushVideoFrame(handle: Long, data: ByteArray): Int

    // ── Event poll ────────────────────────────────────────────────────────────
    @JvmStatic external fun pollEvent(handle: Long): Long
    @JvmStatic external fun eventType(event: Long): Int
    @JvmStatic external fun freeEvent(event: Long)

    // ── Common event accessors ────────────────────────────────────────────────
    @JvmStatic external fun eventText(event: Long): String?
    @JvmStatic external fun eventDeviceId(event: Long): String?
    @JvmStatic external fun eventBinaryData(event: Long): ByteArray?
    @JvmStatic external fun eventDeviceName(event: Long): String?
    @JvmStatic external fun eventMimeType(event: Long): String?
    @JvmStatic external fun eventFileName(event: Long): String?
    @JvmStatic external fun eventFingerprint(event: Long): String?

    // ── Timeline-first clipboard ──────────────────────────────────────────────
    /** 1 if the ClipboardReceived event was auto-applied; 0 if timeline-first. */
    @JvmStatic external fun eventAutoApplied(event: Long): Int
    /** Activity feed entry ID (-1 if not applicable). */
    @JvmStatic external fun eventActivityId(event: Long): Long
    /** Apply a remote clipboard item to the local clipboard by its content hash. */
    @JvmStatic external fun applyClipboardByHash(engineHandle: Long, hash: String): Int
    /** Mark a peer as trusted after the user approves the pairing prompt. */
    @JvmStatic external fun trustPeer(engineHandle: Long, deviceId: String): Int
    /** Reject a peer after the user denies the pairing prompt. */
    @JvmStatic external fun rejectPeer(engineHandle: Long, deviceId: String): Int
    /** Forget a previously connected device. */
    @JvmStatic external fun forgetPeer(engineHandle: Long, deviceId: String): Int
    /** Send a pairing request to an untrusted device. */
    @JvmStatic external fun sendPairingRequest(engineHandle: Long, deviceId: String): Int
    /** Respond to an incoming pairing request. */
    @JvmStatic external fun respondToPairing(engineHandle: Long, deviceId: String, accepted: Boolean): Int

    // ── File transfer accessors ───────────────────────────────────────────────
    @JvmStatic external fun eventTransferId(event: Long): String?
    @JvmStatic external fun eventTransferFileName(event: Long): String?
    @JvmStatic external fun eventTransferProgressPercent(event: Long): Int
    @JvmStatic external fun eventTransferBytesReceived(event: Long): Long
    @JvmStatic external fun eventTransferSpeedBps(event: Long): Long
    @JvmStatic external fun eventTransferEtaSecs(event: Long): Long
    @JvmStatic external fun eventTransferTotalBytes(event: Long): Long
    @JvmStatic external fun eventTransferDestPath(event: Long): String?
    /** Accept an incoming file transfer (identified by hex transfer ID). */
    @JvmStatic external fun acceptFileTransfer(engineHandle: Long, transferIdHex: String): Int
    /** Reject an incoming file transfer. */
    @JvmStatic external fun rejectFileTransfer(engineHandle: Long, transferIdHex: String): Int
    /** Cancel an active file transfer. */
    @JvmStatic external fun cancelFileTransfer(engineHandle: Long, transferIdHex: String): Int
    /** Pause an active file transfer. */
    @JvmStatic external fun pauseFileTransfer(engineHandle: Long, transferIdHex: String): Int
    /** Resume a paused file transfer. */
    @JvmStatic external fun resumeFileTransfer(engineHandle: Long, transferIdHex: String): Int

    /**
     * Connect to a peer discovered via Android NSD.
     * Returns 0 on success, -1 on error.
     */
    @JvmStatic external fun connectToPeer(handle: Long, ip: String, port: Int): Int
    @JvmStatic external fun disconnectPeer(handle: Long, deviceId: String): Int

    /**
     * Returns this engine's stable device UUID as a hyphenated string
     * (e.g. "550e8400-e29b-41d4-a716-446655440000"), or null on error.
     * Used to filter self-connections during NSD resolution.
     */
    @JvmStatic external fun getDeviceId(handle: Long): String?
    @JvmStatic external fun peersJson(handle: Long): String?
    @JvmStatic external fun sendFilePath(
        handle: Long,
        path: String,
        displayName: String,
        mimeType: String,
        targetDeviceId: String?
    ): Int

    /**
     * Push updated sync settings to the running engine atomically.
     * Avoids restarting the service just to update a toggle.
     * Returns 0 on success, -1 if the handle is invalid.
     */
    @JvmStatic external fun applySyncSettings(
        handle: Long,
        syncEnabled: Boolean,
        syncText: Boolean,
        syncImages: Boolean,
        syncFiles: Boolean,
    ): Int

    // ── Call continuity ───────────────────────────────────────────────────────
    /** Push phone call state (ringing/offhook/idle) to all connected peers. */
    @JvmStatic external fun pushCallState(
        handle: Long, state: String, number: String, contactName: String
    ): Int
    /** Get the call state string from a CR_EVENT_CALL_STATE_CHANGED event. */
    @JvmStatic external fun eventCallState(event: Long): String?
    /** Get the phone number from a CR_EVENT_CALL_STATE_CHANGED event. */
    @JvmStatic external fun eventCallNumber(event: Long): String?
    /** Get the contact name from a CR_EVENT_CALL_STATE_CHANGED event. */
    @JvmStatic external fun eventCallContactName(event: Long): String?
    /** Get the action string ("accept"/"decline") from a CR_EVENT_CALL_ACTION event. */
    @JvmStatic external fun eventCallAction(event: Long): String?
    // ── Battery synchronization (F20) ─────────────────────────────────────────
    @JvmStatic external fun pushBatteryStatus(
        handle: Long, level: Int, charging: Boolean
    ): Int

    // ── Network lifecycle ─────────────────────────────────────────────────────
    /**
     * Notify the Rust engine that Android's default network is available again
     * (e.g., after Doze, Wi-Fi reconnect, airplane mode toggle).
     * Triggers immediate reconnection to all known trusted peers.
     */
    @JvmStatic external fun notifyNetworkRestored(handle: Long): Int
}

// ── Activity feed model ───────────────────────────────────────────────────────

enum class ActivityKind {
    CLIPBOARD_TEXT, CLIPBOARD_IMAGE, FILE_SENT, FILE_RECEIVED,
    FILE_TRANSFER_INCOMING, FILE_TRANSFER_PROGRESS, FILE_TRANSFER_COMPLETE,
    FILE_TRANSFER_FAILED, FILE_TRANSFER_PAUSED, FILE_TRANSFER_RESUMED,
    PEER_CONNECTED, PEER_DISCONNECTED, WARNING;
}

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

// ── Battery mode ──────────────────────────────────────────────────────────────

enum class BackgroundSyncMode {
    ALWAYS_ACTIVE,    // poll at full rate, keep WakeLock during drain
    BATTERY_OPTIMIZED // reduced poll rate, no WakeLock
}

// ── Service ───────────────────────────────────────────────────────────────────

data class TransferProgress(
    val id: String,
    val fileName: String,
    val percent: Int,
    val bytesReceived: Long,
    val speedBps: Long,
    val etaSecs: Long,
    var isPaused: Boolean = false
)

class DeskdropService : Service() {

    companion object {
        private const val TAG = "Deskdrop"
        const val PREFS_NAME = "deskdrop"

        // Expose engine handle for high-throughput zero-copy JNI calls (e.g. video frames)
        @Volatile var activeEngineHandle: Long = 0L

        // Flow to expose active transfers to UI
        val activeTransfersFlow = kotlinx.coroutines.flow.MutableStateFlow<List<TransferProgress>>(emptyList())

        // Notification channels
        private const val CHAN_SERVICE = "cr_service"   // IMPORTANCE_MIN — silent persistent
        private const val CHAN_ALERTS  = "cr_alerts"    // IMPORTANCE_DEFAULT — trust/file/failure
        private const val CHAN_CALLS   = "cr_calls"     // IMPORTANCE_HIGH — incoming call banner

        // Notification IDs
        private const val NOTIF_ID_SERVICE           = 1001
        private const val NOTIF_ID_TOFU              = 1002
        private const val NOTIF_ID_FILE              = 1003
        private const val NOTIF_ID_FAILURE           = 1004
        private const val NOTIF_ID_CLIPBOARD_AVAILABLE = 1005
        private const val NOTIF_ID_FILE_BASE         = 2000  // + (tid.hashCode() and 0xFFF)
        private const val NOTIF_ID_CALL              = 3001  // incoming call banner

        // Intent actions
        const val ACTION_START              = "com.deskdrop.START"
        const val ACTION_STOP               = "com.deskdrop.STOP"
        const val ACTION_PAUSE_SYNC         = "com.deskdrop.PAUSE_SYNC"
        const val ACTION_RESUME_SYNC        = "com.deskdrop.RESUME_SYNC"
        const val ACTION_DISCONNECT_ALL     = "com.deskdrop.DISCONNECT_ALL"
        const val ACTION_PUSH_TEXT          = "com.deskdrop.PUSH_TEXT"
        const val ACTION_PUSH_SHARED_URI    = "com.deskdrop.PUSH_SHARED_URI"
        const val ACTION_SCAN_NOW           = "com.deskdrop.SCAN_NOW"
        const val ACTION_STATUS_CHANGED     = "com.deskdrop.STATUS_CHANGED"
        const val ACTION_SETTINGS_CHANGED   = "com.deskdrop.SETTINGS_CHANGED"  // re-read prefs live
        const val ACTION_PUSH_CLIPBOARD     = "com.deskdrop.PUSH_CLIPBOARD"    // send Android clipboard to peers
        const val ACTION_PUSH_NOTIFICATION  = "com.deskdrop.PUSH_NOTIFICATION"
        const val ACTION_APPLY_CLIPBOARD    = "com.deskdrop.APPLY_CLIPBOARD"
        const val ACTION_ACCEPT_FILE_TRANSFER = "com.deskdrop.ACCEPT_FILE_TRANSFER"
        const val ACTION_REJECT_FILE_TRANSFER = "com.deskdrop.REJECT_FILE_TRANSFER"
        const val ACTION_CANCEL_FILE_TRANSFER = "com.deskdrop.CANCEL_FILE_TRANSFER"
        const val ACTION_PAUSE_FILE_TRANSFER  = "com.deskdrop.PAUSE_FILE_TRANSFER"
        const val ACTION_RESUME_FILE_TRANSFER = "com.deskdrop.RESUME_FILE_TRANSFER"
        const val ACTION_CONNECT_MANUAL     = "com.deskdrop.CONNECT_MANUAL"
        const val ACTION_TRUST_PEER         = "com.deskdrop.TRUST_PEER"
        const val ACTION_REJECT_PEER        = "com.deskdrop.REJECT_PEER"
        const val ACTION_FORGET_PEER        = "com.deskdrop.FORGET_PEER"
        const val ACTION_SEND_PAIRING_REQUEST = "com.deskdrop.SEND_PAIRING_REQUEST"
        const val ACTION_RESPOND_TO_PAIRING = "com.deskdrop.RESPOND_TO_PAIRING"

        // Intent extras
        const val EXTRA_CLIPBOARD_TEXT      = "clipboard_text"
        const val EXTRA_CONTENT_HASH        = "content_hash"   // SHA-256 hex; used for full-content apply via engine
        const val EXTRA_TRANSFER_ID         = "transfer_id"
        const val EXTRA_SHARED_URI          = "shared_uri"
        const val EXTRA_SHARED_URIS         = "shared_uris"
        const val EXTRA_SHARED_NAME         = "shared_name"
        const val EXTRA_TARGET_DEVICE_ID    = "target_device_id"
        const val EXTRA_NOTIFICATION_ID     = "notification_id"
        const val EXTRA_NOTIFICATION_PKG    = "notification_pkg"
        const val EXTRA_NOTIFICATION_TITLE  = "notification_title"
        const val EXTRA_NOTIFICATION_TEXT   = "notification_text"
        const val PREF_SERVICE_RUNNING      = "service_running"

        // Poll intervals
        private const val POLL_FULL_MS      = 20L    // 50 Hz — always-active mode
        private const val POLL_REDUCED_MS   = 100L   // 10 Hz — battery-optimized mode
        private const val CLIP_FULL_MS      = 200L   // clipboard check interval (full)
        private const val CLIP_REDUCED_MS   = 500L   // clipboard check interval (reduced)
        private const val ACTIVITY_FEED_MAX = 100

        // NSD (Network Service Discovery) — mirrors the mDNS service type used by the Rust engine
        private const val NSD_SERVICE_TYPE       = "_deskdrop._tcp."
        private const val DEFAULT_DESKDROP_PORT = 47823

        // Global activity feed — readable by UI without binding to the service
        @JvmField val activityFeed = ArrayDeque<ActivityEntry>()
        @JvmField val feedLock     = Any()

        fun addToFeed(entry: ActivityEntry) {
            synchronized(feedLock) {
                activityFeed.addFirst(entry)
                while (activityFeed.size > ACTIVITY_FEED_MAX) activityFeed.removeLast()
            }
        }

        fun getFeedSnapshot(): List<ActivityEntry> = synchronized(feedLock) {
            activityFeed.toList()
        }
    }

    // ── State ─────────────────────────────────────────────────────────────────

    private var engineHandle: Long = 0L
    private val handler = Handler(Looper.getMainLooper())
    private var lastClipboardSignature: String? = null
    private var suppressNext = false
    private val connectedPeerIds = linkedMapOf<String, String>()  // deviceId → displayName
    private val engineStarted = AtomicBoolean(false)
    private val notificationManager by lazy { getSystemService(NotificationManager::class.java) }

    // NSD — peer discovery on Android (replaces stubbed Rust mDNS)
    private var nsdRegistrationListener: NsdManager.RegistrationListener? = null
    private var nsdDiscoveryListener: NsdManager.DiscoveryListener? = null

    // Self-connection filter: first 8 chars of our UUID match the NSD service name suffix.
    // Set once the engine starts; used in makeResolveListener() to skip our own advertisement.
    private var myDeviceUuidPrefix: String? = null
    private var myDeviceId: String? = null

    // Actual NSD service name as reported by onServiceRegistered (may differ from requested
    // if Android resolved a collision by appending " (2)" etc.).
    private var myActualNsdName: String? = null

    // NSD resolution queue to prevent FAILURE_ALREADY_ACTIVE
    private val pendingNsdResolves = java.util.concurrent.ConcurrentLinkedQueue<android.net.nsd.NsdServiceInfo>()
    private val isResolvingNsd = java.util.concurrent.atomic.AtomicBoolean(false)

    // Network change callback — restarts NSD when the device switches WiFi networks
    // or reconnects after being offline (e.g. waking from sleep, roaming).
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var pairingReceiverRegistered = false

    private val activeTransfers = mutableMapOf<String, TransferProgress>()

    private fun publishActiveTransfers() {
        activeTransfersFlow.value = activeTransfers.values.toList()
    }

    private var heartbeatHandler = android.os.Handler(android.os.Looper.getMainLooper())

    private val pairingResultReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action != PairingActivity.ACTION_PAIRING_RESULT) return
            val deviceId = intent.getStringExtra(PairingActivity.EXTRA_DEVICE_ID) ?: return
            val approved = intent.getBooleanExtra(PairingActivity.EXTRA_APPROVED, false)
            val h = engineHandle
            if (h == 0L) return

            val result = if (approved) {
                DeskdropJni.trustPeer(h, deviceId)
            } else {
                DeskdropJni.rejectPeer(h, deviceId)
            }

            Log.i(TAG, "Pairing result for $deviceId approved=$approved result=$result")
            notificationManager.cancel(NOTIF_ID_TOFU)
            persistStatus()
        }
    }

    // NSD retry after all peers disconnect — exponential backoff, max 60 s.
    private val nsdRetryCount = AtomicLong(0L)
    private var nsdRetryRunnable: Runnable? = null

    // WakeLock & WifiLock — held continuously to prevent sleep disconnections.
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: android.net.wifi.WifiManager.WifiLock? = null

    // MulticastLock — held for the lifetime of the service.
    // Many OEM WiFi drivers (Samsung, Xiaomi, OnePlus, Realme) suppress
    // multicast/mDNS packets in hardware unless this lock is held.
    // Without it, NSD registration succeeds but packets are silently dropped,
    // so the Mac never sees the Android advertisement and vice versa.
    private var multicastLock: android.net.wifi.WifiManager.MulticastLock? = null
    private val clipboardManager: ClipboardManager by lazy {
        getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
    }

    // Cached prefs (reloaded on relevant changes)
    private fun prefs() = getSharedPreferences(PREFS_NAME, MODE_PRIVATE)
    private fun isSyncEnabled()           = prefs().getBoolean("sync_enabled", true)
    private fun isClipboardNotifyEnabled()= prefs().getBoolean("notify_on_remote_copy", false)
    private fun syncMode(): BackgroundSyncMode =
        if (prefs().getString("sync_mode", "always") == "battery") BackgroundSyncMode.BATTERY_OPTIMIZED
        else BackgroundSyncMode.ALWAYS_ACTIVE

    private val pollInterval  get() = if (syncMode() == BackgroundSyncMode.ALWAYS_ACTIVE) POLL_FULL_MS  else POLL_REDUCED_MS
    private val clipInterval  get() = if (syncMode() == BackgroundSyncMode.ALWAYS_ACTIVE) CLIP_FULL_MS  else CLIP_REDUCED_MS

    // ── Service lifecycle ─────────────────────────────────────────────────────

    override fun onCreate() {
        super.onCreate()
        createNotificationChannels()
        registerPairingReceiver()
        acquireWakeLockIfNeeded()
        setServiceRunning(true)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP         -> { shutdownAndStop(); return START_NOT_STICKY }

            // Settings changed live (e.g. sync toggle from SettingsActivity).
            // Re-read prefs and push them to the engine if possible.
            ACTION_SETTINGS_CHANGED -> {
                applySettingsToEngine()
                return START_STICKY
            }

            // User tapped "Send clipboard to Mac" on the dashboard.
            ACTION_PUSH_CLIPBOARD -> {
                val h = engineHandle
                if (h != 0L) {
                    if (!hasConnectedPeers()) {
                        Log.i(TAG, "PUSH_CLIPBOARD ignored: no connected peers")
                        return START_STICKY
                    }
                    val cm   = getSystemService(ClipboardManager::class.java)
                    val text = cm.primaryClip?.getItemAt(0)
                        ?.coerceToText(this)?.toString()
                    if (!text.isNullOrBlank()) {
                        val result = DeskdropJni.pushText(h, text)
                        Log.i(TAG, "PUSH_CLIPBOARD: result=$result len=${text.length}")
                        if (result == 0) {
                            addActivity(ActivityEntry(
                                deviceName = "All devices",
                                kind       = ActivityKind.CLIPBOARD_TEXT,
                                preview    = text.take(400)
                            ))
                            broadcastActivityUpdated()
                        }
                    } else {
                        Log.w(TAG, "PUSH_CLIPBOARD: clipboard is empty")
                    }
                }
                return START_STICKY
            }
            ACTION_SCAN_NOW -> {
                restartDiscoveryNow()
                return START_STICKY
            }
            ACTION_PAUSE_SYNC   -> { setSyncEnabled(false); return START_STICKY }
            ACTION_RESUME_SYNC  -> { setSyncEnabled(true);  return START_STICKY }
            ACTION_DISCONNECT_ALL -> { disconnectAllPeers(); return START_STICKY }
            ACTION_CONNECT_MANUAL -> {
                val ip = intent?.getStringExtra("ip")
                val port = intent?.getIntExtra("port", 47823) ?: 47823
                if (!ip.isNullOrBlank() && engineHandle != 0L) {
                    val result = DeskdropJni.connectToPeer(engineHandle, ip, port)
                    Log.i(TAG, "Manual connect to $ip:$port triggered, result = $result")
                }
                return START_STICKY
            }
            ACTION_TRUST_PEER -> {
                val deviceId = intent?.getStringExtra(EXTRA_TARGET_DEVICE_ID) ?: return START_STICKY
                val h = engineHandle
                if (h != 0L) {
                    val result = DeskdropJni.trustPeer(h, deviceId)
                    Log.i(TAG, "Manual trust request for $deviceId: result=$result")
                    persistStatus()
                }
                return START_STICKY
            }
            ACTION_REJECT_PEER -> {
                val deviceId = intent?.getStringExtra(EXTRA_TARGET_DEVICE_ID) ?: return START_STICKY
                val h = engineHandle
                if (h != 0L) {
                    val result = DeskdropJni.rejectPeer(h, deviceId)
                    Log.i(TAG, "Manual reject request for $deviceId: result=$result")
                    persistStatus()
                }
                return START_STICKY
            }
            ACTION_FORGET_PEER -> {
                val deviceId = intent?.getStringExtra(EXTRA_TARGET_DEVICE_ID) ?: return START_STICKY
                val h = engineHandle
                if (h != 0L) {
                    val result = DeskdropJni.forgetPeer(h, deviceId)
                    Log.i(TAG, "Manual forget request for $deviceId: result=$result")
                    persistStatus()
                }
                // Also eagerly remove from shared preferences so UI updates immediately
                val prefs = prefs()
                val peersStr = prefs.getString(PREF_PEER_SNAPSHOTS_JSON, "[]")
                try {
                    val arr = org.json.JSONArray(peersStr)
                    val newArr = org.json.JSONArray()
                    for (i in 0 until arr.length()) {
                        val obj = arr.getJSONObject(i)
                        if (obj.optString("id") != deviceId) {
                            newArr.put(obj)
                        }
                    }
                    prefs.edit().putString(PREF_PEER_SNAPSHOTS_JSON, newArr.toString()).apply()
                    sendBroadcast(Intent(ACTION_STATUS_CHANGED).setPackage(packageName))
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to update peers JSON on forget", e)
                }
                return START_STICKY
            }

            ACTION_SEND_PAIRING_REQUEST -> {
                val deviceId = intent?.getStringExtra(EXTRA_TARGET_DEVICE_ID) ?: return START_STICKY
                val h = engineHandle
                if (h != 0L) {
                    val result = DeskdropJni.sendPairingRequest(h, deviceId)
                    Log.i(TAG, "Manual pairing request for $deviceId: result=$result")
                    persistStatus()
                }
                return START_STICKY
            }
            ACTION_RESPOND_TO_PAIRING -> {
                val deviceId = intent?.getStringExtra(EXTRA_TARGET_DEVICE_ID) ?: return START_STICKY
                val accepted = intent?.getBooleanExtra(PairingActivity.EXTRA_APPROVED, false) ?: false
                val h = engineHandle
                if (h != 0L) {
                    val result = DeskdropJni.respondToPairing(h, deviceId, accepted)
                    Log.i(TAG, "Pairing response for $deviceId accepted=$accepted result=$result")
                    persistStatus()
                }
                return START_STICKY
            }

            // Timeline-first: user tapped "Apply" on a notification or feed item.
            // Prefer hash-based apply (full content via engine) over truncated preview text.
            ACTION_APPLY_CLIPBOARD -> {
                val hash = intent.getStringExtra(EXTRA_CONTENT_HASH)
                val text = intent.getStringExtra(EXTRA_CLIPBOARD_TEXT)
                if (engineHandle != 0L) {
                    val cm = getSystemService(ClipboardManager::class.java)
                    suppressNext = true
                    if (!hash.isNullOrBlank()) {
                        // Engine holds the full content by hash — apply without truncation.
                        val result = DeskdropJni.applyClipboardByHash(engineHandle, hash)
                        if (result != 1 && !text.isNullOrBlank()) {
                            // Hash not found (e.g. engine restarted) — fall back to text.
                            cm.setPrimaryClip(ClipData.newPlainText("Deskdrop", text))
                        }
                    } else if (!text.isNullOrBlank()) {
                        cm.setPrimaryClip(ClipData.newPlainText("Deskdrop", text))
                    } else {
                        return START_STICKY
                    }
                    notificationManager.cancel(NOTIF_ID_CLIPBOARD_AVAILABLE)
                    broadcastActivityUpdated()
                }
                return START_STICKY
            }

            // File transfer: user tapped Accept in notification.
            ACTION_ACCEPT_FILE_TRANSFER -> {
                val tid = intent.getStringExtra(EXTRA_TRANSFER_ID) ?: return START_STICKY
                if (engineHandle != 0L) {
                    DeskdropJni.acceptFileTransfer(engineHandle, tid)
                    notificationManager.cancel(transferNotifId(tid))
                }
                return START_STICKY
            }

            // File transfer: user tapped Reject in notification.
            ACTION_REJECT_FILE_TRANSFER -> {
                val tid = intent.getStringExtra(EXTRA_TRANSFER_ID) ?: return START_STICKY
                if (engineHandle != 0L) {
                    DeskdropJni.rejectFileTransfer(engineHandle, tid)
                    notificationManager.cancel(transferNotifId(tid))
                }
                return START_STICKY
            }

            ACTION_CANCEL_FILE_TRANSFER -> {
                val tid = intent.getStringExtra(EXTRA_TRANSFER_ID) ?: return START_STICKY
                if (engineHandle != 0L) {
                    DeskdropJni.cancelFileTransfer(engineHandle, tid)
                    notificationManager.cancel(transferNotifId(tid))
                }
                return START_STICKY
            }

            ACTION_PAUSE_FILE_TRANSFER -> {
                val tid = intent.getStringExtra(EXTRA_TRANSFER_ID) ?: return START_STICKY
                if (engineHandle != 0L) {
                    DeskdropJni.pauseFileTransfer(engineHandle, tid)
                }
                return START_STICKY
            }

            ACTION_RESUME_FILE_TRANSFER -> {
                val tid = intent.getStringExtra(EXTRA_TRANSFER_ID) ?: return START_STICKY
                if (engineHandle != 0L) {
                    DeskdropJni.resumeFileTransfer(engineHandle, tid)
                }
                return START_STICKY
            }
        }

        // Start / re-attach foreground
        return try {
            startForegroundCompat(buildForegroundNotification())
            setServiceRunning(true)

            if (!engineStarted.getAndSet(true)) {
                val deviceName = resolvedDeviceName()
                val dataDir = File(filesDir, "deskdrop").also { it.mkdirs() }.absolutePath
                val fileSaveDir = (
                    getExternalFilesDir(android.os.Environment.DIRECTORY_DOWNLOADS)
                        ?: filesDir
                    ).resolve("Deskdrop").apply { mkdirs() }
                engineHandle = DeskdropJni.start(
                    deviceName,
                    0,
                    dataDir,
                    fileSaveDir.absolutePath
                )

                if (engineHandle == 0L) {
                    Log.e(TAG, "Rust engine failed to start")
                    setServiceRunning(false)
                    stopSelf()
                    return START_NOT_STICKY
                }

                activeEngineHandle = engineHandle
                Log.i(TAG, "Engine started — $deviceName")
                scheduleEventDrain()
                scheduleClipboardWatch()
                acquireMulticastLock()  // must precede NSD so mDNS packets aren't filtered
                // Cache our own UUID prefix so NSD can filter self-connections.
                myDeviceId = DeskdropJni.getDeviceId(engineHandle)
                myDeviceUuidPrefix = myDeviceId?.take(8)
                startNsdDiscovery()   // advertise + browse so the Mac can find us
                registerNetworkCallback() // restart NSD on WiFi changes
                startCallStateMonitor()   // call continuity: relay phone state to peers
                startBatteryMonitor()     // F20: relay battery status to peers
                persistStatus()
            } else {
                // Engine was already running — permission may have just been granted.
                // Restart the call monitor in case it was skipped earlier.
                if (hasCallPermissions() && callStateReceiver == null) {
                    startCallStateMonitor()
                }
                startBatteryMonitor()
            }

            if (intent?.action == ACTION_PUSH_TEXT) {
                intent.getStringExtra("text")?.takeIf { it.isNotBlank() }?.let { text ->
                    if (engineHandle != 0L && hasConnectedPeers()) {
                        DeskdropJni.pushText(engineHandle, text)
                    } else if (engineHandle != 0L) {
                        Log.i(TAG, "PUSH_TEXT ignored: no connected peers")
                    } else {
                        Unit
                    }
                }
            }

            if (intent?.action == ACTION_PUSH_SHARED_URI) {
                val rawUri = intent.getStringExtra(EXTRA_SHARED_URI)
                val rawUris = intent.getStringArrayListExtra(EXTRA_SHARED_URIS)
                val preferredName = intent.getStringExtra(EXTRA_SHARED_NAME)
                val targetDeviceId = intent.getStringExtra(EXTRA_TARGET_DEVICE_ID)
                val uriStrings = buildList {
                    if (!rawUri.isNullOrBlank()) add(rawUri)
                    rawUris?.filter { it.isNotBlank() }?.let { addAll(it) }
                }
                if (uriStrings.isNotEmpty() && engineHandle != 0L) {
                    if (!hasConnectedPeers()) {
                        Log.i(TAG, "PUSH_SHARED_URI ignored: no connected peers")
                    } else if (targetDeviceId != null && !isPeerConnected(targetDeviceId)) {
                        Log.w(TAG, "PUSH_SHARED_URI ignored: target peer is no longer connected")
                    } else {
                        sendSharedUris(uriStrings, preferredName, targetDeviceId)
                    }
                }
            }

            if (intent?.action == ACTION_PUSH_NOTIFICATION) {
                if (engineHandle != 0L && hasConnectedPeers()) {
                    val id = intent.getStringExtra(EXTRA_NOTIFICATION_ID) ?: ""
                    val pkg = intent.getStringExtra(EXTRA_NOTIFICATION_PKG) ?: ""
                    val title = intent.getStringExtra(EXTRA_NOTIFICATION_TITLE) ?: ""
                    val text = intent.getStringExtra(EXTRA_NOTIFICATION_TEXT) ?: ""
                    DeskdropJni.pushNotification(engineHandle, id, pkg, title, text)
                }
            }

            START_STICKY
        } catch (ex: Throwable) {
            Log.e(TAG, "onStartCommand failed", ex)
            setServiceRunning(false)
            stopSelf()
            START_NOT_STICKY
        }
    }

    override fun onDestroy() {
        stopNsdDiscovery()
        stopCallStateMonitor()
        stopBatteryMonitor()
        unregisterNetworkCallback()
        cancelNsdRetry()
        releaseMulticastLock()
        handler.removeCallbacksAndMessages(null)
        if (engineHandle != 0L) {
            DeskdropJni.stop(engineHandle)
            engineHandle = 0L
            activeEngineHandle = 0L
        }
        engineStarted.set(false)
        connectedPeerIds.clear()
        releaseWakeLock()
        setServiceRunning(false)
        persistStatus()
        unregisterPairingReceiver()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    // Survive task removal (user swipes app away)
    override fun onTaskRemoved(rootIntent: Intent?) {
        // Re-schedule restart via AlarmManager for maximum reliability on OEM ROMs
        val pending = PendingIntent.getService(
            this, 1,
            Intent(this, DeskdropService::class.java).apply { action = ACTION_START },
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_ONE_SHOT
        )
        val am = getSystemService(ALARM_SERVICE) as AlarmManager
        am.set(AlarmManager.ELAPSED_REALTIME, SystemClock.elapsedRealtime() + 1_000L, pending)
        super.onTaskRemoved(rootIntent)
    }

    // ── WakeLock & WifiLock ───────────────────────────────────────────────────

    private fun acquireWakeLockIfNeeded() {
        if (wakeLock == null) {
            wakeLock = (getSystemService(POWER_SERVICE) as PowerManager)
                .newWakeLock(
                    PowerManager.PARTIAL_WAKE_LOCK,
                    "Deskdrop::ContinuousLock"
                ).apply {
                    setReferenceCounted(false)
                    acquire()
                }
            
            val wm = runCatching {
                applicationContext.getSystemService(WIFI_SERVICE) as android.net.wifi.WifiManager
            }.getOrNull()
            
            if (wm != null) {
                wifiLock = wm.createWifiLock(
                    android.net.wifi.WifiManager.WIFI_MODE_FULL_HIGH_PERF,
                    "Deskdrop::WifiLock"
                ).apply {
                    setReferenceCounted(false)
                    acquire()
                }
            }
        }
    }

    private fun releaseWakeLock() {
        runCatching {
            wakeLock?.let { if (it.isHeld) it.release() }
            wifiLock?.let { if (it.isHeld) it.release() }
        }
        wakeLock = null
        wifiLock = null
    }

    // ── Multicast lock ────────────────────────────────────────────────────────
    //
    // Held for the entire service lifetime (not just during drain) because mDNS
    // needs multicast continuously.  The overhead is negligible — it only
    // prevents the WiFi driver from filtering multicast in hardware.

    private fun acquireMulticastLock() {
        if (multicastLock?.isHeld == true) return
        val wm = runCatching {
            applicationContext.getSystemService(WIFI_SERVICE) as android.net.wifi.WifiManager
        }.getOrNull() ?: return
        multicastLock = wm.createMulticastLock("Deskdrop::NsdMulticast").apply {
            setReferenceCounted(false)
            acquire()
        }
        Log.i(TAG, "Multicast lock acquired")
    }

    private fun releaseMulticastLock() {
        runCatching { multicastLock?.let { if (it.isHeld) it.release() } }
        multicastLock = null
        Log.i(TAG, "Multicast lock released")
    }

    // ── Sync enable / disable ─────────────────────────────────────────────────

    private fun setSyncEnabled(enabled: Boolean) {
        prefs().edit().putBoolean("sync_enabled", enabled).apply()
        updateForegroundNotification()
        broadcastStatus()
    }

    private fun disconnectAllPeers() {
        val h = engineHandle
        if (h != 0L) {
            currentPeerSnapshots()
                .filter { it.isConnected }
                .forEach { peer -> DeskdropJni.disconnectPeer(h, peer.id) }
        }
        connectedPeerIds.clear()
        persistStatus()
        updateForegroundNotification()
        handler.postDelayed({
            persistStatus()
            updateForegroundNotification()
        }, 750L)
    }

    private fun shutdownAndStop() {
        disconnectAllPeers()
        stopSelf()
    }

    private fun restartDiscoveryNow() {
        if (engineHandle == 0L) return
        handler.post {
            acquireMulticastLock()
            stopNsdDiscovery()
            startNsdDiscovery()
            cancelNsdRetry()
            nsdRetryCount.set(0L)
            persistStatus()
            updateForegroundNotification()
        }
    }

    // ── Event drain (Rust → Kotlin) ───────────────────────────────────────────

    private fun scheduleEventDrain() {
        val interval = pollInterval
        handler.postDelayed(object : Runnable {
            override fun run() {
                if (engineHandle != 0L) {
                    drainEvents()
                }
                if (engineHandle != 0L) {
                    handler.postDelayed(this, pollInterval)
                }
            }
        }, interval)
    }

    private fun drainEvents() {
        try {
            while (engineHandle != 0L) {
                val ev = DeskdropJni.pollEvent(engineHandle)
                if (ev == 0L) break
                try { handleEvent(ev) } finally { DeskdropJni.freeEvent(ev) }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Exception during event drain", e)
        }
    }

    private fun handleEvent(ev: Long) {
        when (DeskdropJni.eventType(ev)) {

            // ── Clipboard text — AUTO-APPLIED (legacy or auto-apply enabled) ─
            DeskdropJni.CR_EVENT_CLIPBOARD_TEXT -> {
                val text = DeskdropJni.eventText(ev) ?: return
                val from = resolvePeerDisplayName(
                    DeskdropJni.eventDeviceId(ev),
                    DeskdropJni.eventDeviceName(ev)
                )
                // Track last-sync time per peer so dashboard can show "2m ago"
                peerLastSync[from] = System.currentTimeMillis()
                addActivity(ActivityEntry(
                    deviceName = from,
                    kind = ActivityKind.CLIPBOARD_TEXT,
                    preview = text.take(400).replace('\n', ' '),
                    appliedLocally = true
                ))
                applyText(text, from)
            }

            // ── Clipboard text — TIMELINE-FIRST (available, not auto-applied) ─
            DeskdropJni.CR_EVENT_CLIPBOARD_AVAILABLE -> {
                val text = DeskdropJni.eventText(ev) ?: return
                val from = resolvePeerDisplayName(
                    DeskdropJni.eventDeviceId(ev),
                    DeskdropJni.eventDeviceName(ev)
                )
                // Track last-sync time per peer
                peerLastSync[from] = System.currentTimeMillis()
                val autoApplied = DeskdropJni.eventAutoApplied(ev) == 1
                val activityId  = DeskdropJni.eventActivityId(ev)
                val preview = text.take(400).replace('\n', ' ')

                addActivity(ActivityEntry(
                    id = activityId.takeIf { it >= 0 } ?: System.nanoTime(),
                    deviceName = from,
                    kind = ActivityKind.CLIPBOARD_TEXT,
                    preview = preview,
                    contentHash = textContentHash(text),
                    appliedLocally = autoApplied && DeskdropApp.isAppInForeground
                ))

                if (autoApplied && DeskdropApp.isAppInForeground) {
                    applyText(text, from)
                } else {
                    // Show a dismissable notification with an "Apply" action.
                    showClipboardAvailableNotification(from, preview, text, textContentHash(text))
                }
            }

            // ── Clipboard image — AUTO-APPLIED ────────────────────────────────
            DeskdropJni.CR_EVENT_CLIPBOARD_IMAGE -> {
                val bytes = DeskdropJni.eventBinaryData(ev) ?: return
                val mime  = DeskdropJni.eventMimeType(ev) ?: "image/png"
                val from  = resolvePeerDisplayName(
                    DeskdropJni.eventDeviceId(ev),
                    DeskdropJni.eventDeviceName(ev)
                )
                addActivity(ActivityEntry(deviceName = from, kind = ActivityKind.CLIPBOARD_IMAGE,
                    preview = "image ($mime)", appliedLocally = true))
                applyBinaryClipboard(bytes, imageNameForMime(mime), mime, from, isFile = false)
            }

            // ── File received (legacy clipboard file) ─────────────────────────
            DeskdropJni.CR_EVENT_CLIPBOARD_FILE -> {
                val bytes = DeskdropJni.eventBinaryData(ev) ?: return
                val name  = DeskdropJni.eventFileName(ev) ?: "Deskdrop_file"
                val from  = resolvePeerDisplayName(
                    DeskdropJni.eventDeviceId(ev),
                    DeskdropJni.eventDeviceName(ev)
                )
                addActivity(ActivityEntry(deviceName = from, kind = ActivityKind.FILE_RECEIVED,
                    preview = name))
                applyBinaryClipboard(bytes, name, null, from, isFile = true)
            }

            // ── Dedicated file transfer: incoming ─────────────────────────────
            DeskdropJni.CR_EVENT_FILE_TRANSFER_INCOMING -> {
                val tid       = DeskdropJni.eventTransferId(ev) ?: return
                val from      = resolvePeerDisplayName(
                    DeskdropJni.eventDeviceId(ev),
                    DeskdropJni.eventDeviceName(ev)
                )
                val fileName  = DeskdropJni.eventTransferFileName(ev) ?: "file"
                val totalBytes = DeskdropJni.eventTransferTotalBytes(ev)
                addActivity(ActivityEntry(deviceName = from,
                    kind = ActivityKind.FILE_TRANSFER_INCOMING, preview = fileName,
                    transferId = tid, fileTotalBytes = totalBytes))
                
                // Automatically accept incoming file transfers from trusted connected peers for seamless multi-file sharing!
                if (engineHandle != 0L) {
                    DeskdropJni.acceptFileTransfer(engineHandle, tid)
                } else {
                    showFileTransferIncomingNotification(from, fileName, totalBytes, tid)
                }
            }

            // ── Dedicated file transfer: progress update ──────────────────────
            DeskdropJni.CR_EVENT_FILE_TRANSFER_PROGRESS -> {
                val tid           = DeskdropJni.eventTransferId(ev) ?: return
                val percent       = DeskdropJni.eventTransferProgressPercent(ev)
                val bytesReceived = DeskdropJni.eventTransferBytesReceived(ev)
                val speedBps      = DeskdropJni.eventTransferSpeedBps(ev)
                val etaSecs       = DeskdropJni.eventTransferEtaSecs(ev)
                val name          = DeskdropJni.eventTransferFileName(ev) ?: "file"
                val from          = resolvePeerDisplayName(
                    DeskdropJni.eventDeviceId(ev),
                    DeskdropJni.eventDeviceName(ev)
                )
                // Update existing activity entry in-place.
                updateActivityTransferProgress(
                    tid = tid,
                    percent = percent,
                    bytesReceived = bytesReceived,
                    speedBps = speedBps,
                    etaSecs = etaSecs
                )
                
                val isPaused = activeTransfers[tid]?.isPaused ?: false
                activeTransfers[tid] = TransferProgress(tid, name, percent, bytesReceived, speedBps, etaSecs, isPaused)
                publishActiveTransfers()
                
                updateFileTransferNotificationProgress(
                    tid = tid,
                    fileName = name,
                    percent = percent,
                    bytesReceived = bytesReceived,
                    speedBps = speedBps,
                    etaSecs = etaSecs,
                    isPaused = isPaused
                )
            }

            // ── Dedicated file transfer: complete ─────────────────────────────
            DeskdropJni.CR_EVENT_FILE_TRANSFER_COMPLETE -> {
                val tid      = DeskdropJni.eventTransferId(ev) ?: return
                val from     = resolvePeerDisplayName(
                    DeskdropJni.eventDeviceId(ev),
                    DeskdropJni.eventDeviceName(ev)
                )
                val fileName = DeskdropJni.eventTransferFileName(ev) ?: "file"
                val destPath = DeskdropJni.eventTransferDestPath(ev) ?: ""
                
                // Copy the file from private app storage to public Downloads!
                val publicFile = if (destPath.isNotEmpty()) {
                    saveFileToPublicDownloads(File(destPath))
                } else null
                
                val finalPath = publicFile?.absolutePath ?: destPath
                updateActivityTransferComplete(tid, finalPath)
                cancelFileTransferNotification(tid)
                showFileTransferCompleteNotification(from, fileName, destPath)
                activeTransfers.remove(tid)
                publishActiveTransfers()
            }

            // ── Dedicated file transfer: failed ───────────────────────────────
            DeskdropJni.CR_EVENT_FILE_TRANSFER_FAILED -> {
                val tid  = DeskdropJni.eventTransferId(ev) ?: return
                val from = resolvePeerDisplayName(
                    DeskdropJni.eventDeviceId(ev),
                    DeskdropJni.eventDeviceName(ev)
                )
                updateActivityTransferFailed(tid)
                cancelFileTransferNotification(tid)
                activeTransfers.remove(tid)
                publishActiveTransfers()
            }

            DeskdropJni.CR_EVENT_FILE_TRANSFER_PAUSED -> {
                val tid = DeskdropJni.eventTransferId(ev) ?: return
                val state = activeTransfers[tid]
                if (state != null) {
                    val newState = state.copy(isPaused = true)
                    activeTransfers[tid] = newState
                    publishActiveTransfers()
                    updateFileTransferNotificationProgress(
                        tid = tid,
                        fileName = newState.fileName,
                        percent = newState.percent,
                        bytesReceived = newState.bytesReceived,
                        speedBps = newState.speedBps,
                        etaSecs = newState.etaSecs,
                        isPaused = true
                    )
                }
            }

            DeskdropJni.CR_EVENT_FILE_TRANSFER_RESUMED -> {
                val tid = DeskdropJni.eventTransferId(ev) ?: return
                val state = activeTransfers[tid]
                if (state != null) {
                    val newState = state.copy(isPaused = false)
                    activeTransfers[tid] = newState
                    publishActiveTransfers()
                    updateFileTransferNotificationProgress(
                        tid = tid,
                        fileName = newState.fileName,
                        percent = newState.percent,
                        bytesReceived = newState.bytesReceived,
                        speedBps = newState.speedBps,
                        etaSecs = newState.etaSecs,
                        isPaused = false
                    )
                }
            }

            // ── Trust (TOFU) prompt ───────────────────────────────────────────
            DeskdropJni.CR_EVENT_TOFU_PROMPT -> {
                val deviceId = DeskdropJni.eventDeviceId(ev) ?: return
                val name = resolvePeerDisplayName(deviceId, DeskdropJni.eventDeviceName(ev))
                val fp   = DeskdropJni.eventFingerprint(ev) ?: ""
                prefs().edit().putString("fingerprint_$deviceId", fp).apply()
                
                // Auto-trust new devices on LAN (especially after QR scan)
                // This matches the macOS behavior and avoids the disruptive pairing screen.
                engineHandle?.let { h ->
                    DeskdropJni.trustPeer(h, deviceId)
                    Log.i(TAG, "Auto-trusted new peer: $name ($deviceId)")
                }
            }

            // ── Peer connected ────────────────────────────────────────────────
            DeskdropJni.CR_EVENT_PEER_CONNECTED -> {
                val deviceId = DeskdropJni.eventDeviceId(ev) ?: return
                val name = resolvePeerDisplayName(
                    deviceId,
                    DeskdropJni.eventDeviceName(ev)
                )
                Log.i(TAG, "Peer connected: $name (id=$deviceId)")
                connectedPeerIds[deviceId] = name
                addActivity(ActivityEntry(deviceName = name,
                    kind = ActivityKind.PEER_CONNECTED, preview = "Connected"))
                persistStatus()
                updateForegroundNotification()
                // Connection established — cancel any pending retry scans and
                // reset backoff so the next disconnect starts fresh.
                cancelNsdRetry()
                nsdRetryCount.set(0L)
            }

            // ── Peer disconnected ─────────────────────────────────────────────
            DeskdropJni.CR_EVENT_PEER_DISCONNECTED -> {
                val deviceId = DeskdropJni.eventDeviceId(ev)
                val name = resolvePeerDisplayName(
                    deviceId,
                    DeskdropJni.eventDeviceName(ev)
                )
                Log.i(TAG, "Peer disconnected: $name (id=$deviceId)")
                if (deviceId != null) connectedPeerIds.remove(deviceId)
                addActivity(ActivityEntry(deviceName = name,
                    kind = ActivityKind.PEER_DISCONNECTED, preview = "Disconnected"))
                persistStatus()
                updateForegroundNotification()
                // If we're now peerless, schedule a retry scan so we reconnect
                // automatically when the Mac wakes up or comes back on the network.
                if (connectedPeerIds.isEmpty()) {
                    scheduleNsdRetry()
                }
            }

            // ── Engine warning ────────────────────────────────────────────────
            DeskdropJni.CR_EVENT_WARNING -> {
                val msg = DeskdropJni.eventText(ev) ?: return
                Log.w(TAG, "Engine warning: $msg")
                addActivity(ActivityEntry(deviceName = "System",
                    kind = ActivityKind.WARNING, preview = msg.take(80)))
                if (isCriticalFailure(msg)) showFailureNotification(msg)
                updateForegroundNotification()
            }

            // ── Call continuity ───────────────────────────────────────────────
            DeskdropJni.CR_EVENT_CALL_STATE_CHANGED -> {
                // On Android we originated this event — nothing to do.
                // Other peers (macOS) will show the incoming call banner.
                Log.d(TAG, "CallStateChanged echoed (no-op on originating device)")
            }

            DeskdropJni.CR_EVENT_CALL_ACTION -> {
                val action = DeskdropJni.eventCallAction(ev) ?: return
                Log.i(TAG, "Remote call action received: $action")
                handleRemoteCallAction(action)
            }

            DeskdropJni.CR_EVENT_BATTERY_STATE_CHANGED -> {
                Log.d(TAG, "BatteryStateChanged event received (no-op on Android)")
            }
        }
    }

    // ── Activity feed helpers ─────────────────────────────────────────────────

    private fun addActivity(entry: ActivityEntry) {
        synchronized(feedLock) {
            activityFeed.addFirst(entry)
            while (activityFeed.size > ACTIVITY_FEED_MAX) activityFeed.removeLast()
        }
        broadcastActivityUpdated()
    }

    private fun updateActivityTransferProgress(
        tid: String,
        percent: Int,
        bytesReceived: Long,
        speedBps: Long,
        etaSecs: Long
    ) {
        synchronized(feedLock) {
            val idx = activityFeed.indexOfFirst { it.transferId == tid }
            if (idx >= 0) {
                activityFeed[idx] = activityFeed[idx].copy(
                    kind = ActivityKind.FILE_TRANSFER_PROGRESS,
                    progressPercent = percent,
                    transferBytesReceived = bytesReceived.coerceAtLeast(0L),
                    transferSpeedBps = speedBps.coerceAtLeast(0L),
                    transferEtaSecs = etaSecs
                )
            } else {
                return
            }
        }
        broadcastActivityUpdated()
    }

    private fun updateActivityTransferComplete(tid: String, destPath: String) {
        synchronized(feedLock) {
            val idx = activityFeed.indexOfFirst { it.transferId == tid }
            if (idx >= 0) {
                activityFeed[idx] = activityFeed[idx].copy(
                    kind = ActivityKind.FILE_TRANSFER_COMPLETE,
                    progressPercent = 100,
                    destPath = destPath
                )
            } else {
                return
            }
        }
        broadcastActivityUpdated()
    }

    private fun updateActivityTransferFailed(tid: String) {
        synchronized(feedLock) {
            val idx = activityFeed.indexOfFirst { it.transferId == tid }
            if (idx >= 0) {
                activityFeed[idx] = activityFeed[idx].copy(kind = ActivityKind.FILE_TRANSFER_FAILED)
            } else {
                return
            }
        }
        broadcastActivityUpdated()
    }

    private fun broadcastActivityUpdated() {
        sendBroadcast(Intent(ACTION_STATUS_CHANGED).setPackage(packageName))
    }

    // ── Clipboard available notification (timeline-first) ─────────────────────

    private fun showClipboardAvailableNotification(
        from: String,
        preview: String,
        fullText: String,
        contentHash: String
    ) {
        val applyIntent = Intent(ACTION_APPLY_CLIPBOARD).apply {
            `package` = packageName
            putExtra(EXTRA_CLIPBOARD_TEXT, fullText)
            putExtra(EXTRA_CONTENT_HASH, contentHash)
        }
        val applyPi = PendingIntent.getService(
            this, fullText.hashCode(),
            applyIntent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Tap notification itself → open MainActivity to see the activity feed.
        val openIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }
        val openPi = PendingIntent.getActivity(
            this, 0, openIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Show a truncated preview in collapsed state; full text (up to 400 chars)
        // in the expanded BigText style — so the user can read it before deciding.
        val bigText = if (fullText.length > 400) fullText.take(397) + "…" else fullText

        val notif = NotificationCompat.Builder(this, CHAN_ALERTS)
            .setSmallIcon(android.R.drawable.ic_menu_edit)
            .setContentTitle("$from → clipboard")
            .setContentText(preview)
            .setStyle(
                NotificationCompat.BigTextStyle()
                    .bigText(bigText)
                    .setSummaryText("Tap to open • Swipe to dismiss")
            )
            .setContentIntent(openPi)
            .addAction(android.R.drawable.ic_menu_set_as, "Apply to clipboard", applyPi)
            .setAutoCancel(true)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .build()

        notificationManager.notify(NOTIF_ID_CLIPBOARD_AVAILABLE, notif)
    }

    // ── File transfer notifications ───────────────────────────────────────────

    private fun showFileTransferIncomingNotification(
        from: String, fileName: String, totalBytes: Long, tid: String
    ) {
        val sizeStr = formatBytes(totalBytes)

        val acceptIntent = Intent(ACTION_ACCEPT_FILE_TRANSFER).apply {
            `package` = packageName
            putExtra(EXTRA_TRANSFER_ID, tid)
        }
        val rejectIntent = Intent(ACTION_REJECT_FILE_TRANSFER).apply {
            `package` = packageName
            putExtra(EXTRA_TRANSFER_ID, tid)
        }
        val acceptPi = PendingIntent.getBroadcast(this, tid.hashCode(),
            acceptIntent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val rejectPi = PendingIntent.getBroadcast(this, tid.hashCode() + 1,
            rejectIntent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

        val notif = NotificationCompat.Builder(this, CHAN_ALERTS)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle("$from wants to send a file")
            .setContentText("$fileName ($sizeStr)")
            .addAction(0, "Accept", acceptPi)
            .addAction(0, "Reject", rejectPi)
            .setOngoing(true)
            .build()
        notificationManager.notify(transferNotifId(tid), notif)
    }

    private fun updateFileTransferNotificationProgress(
        tid: String,
        fileName: String,
        percent: Int,
        bytesReceived: Long,
        speedBps: Long,
        etaSecs: Long,
        isPaused: Boolean = false
    ) {
        val cancelIntent = Intent(ACTION_CANCEL_FILE_TRANSFER).apply {
            `package` = packageName
            putExtra(EXTRA_TRANSFER_ID, tid)
        }
        val cancelPi = PendingIntent.getService(this, tid.hashCode() + 2,
            cancelIntent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

        val pauseResumeIntent = Intent(if (isPaused) ACTION_RESUME_FILE_TRANSFER else ACTION_PAUSE_FILE_TRANSFER).apply {
            `package` = packageName
            putExtra(EXTRA_TRANSFER_ID, tid)
        }
        val pauseResumePi = PendingIntent.getService(this, tid.hashCode() + 3,
            pauseResumeIntent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

        val builder = NotificationCompat.Builder(this, CHAN_ALERTS)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle("Receiving $fileName")
            .setContentText(buildTransferStatusLine(percent, bytesReceived, speedBps, etaSecs) + if (isPaused) " (Paused)" else "")
            .setProgress(100, percent, false)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .addAction(android.R.drawable.ic_media_pause, if (isPaused) "Resume" else "Pause", pauseResumePi)
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Cancel", cancelPi)

        notificationManager.notify(transferNotifId(tid), builder.build())
    }

    private fun showFileTransferCompleteNotification(from: String, fileName: String, destPath: String) {
        val openIntent = if (destPath.isNotEmpty()) {
            val uri = androidx.core.content.FileProvider.getUriForFile(
                this, "$packageName.fileprovider",
                java.io.File(destPath)
            )
            Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, contentResolver.getType(uri))
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
        } else null

        val openPi = openIntent?.let {
            PendingIntent.getActivity(this, destPath.hashCode(), it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        }

        val builder = NotificationCompat.Builder(this, CHAN_ALERTS)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle("File received from $from")
            .setContentText(fileName)
            .setAutoCancel(true)
        if (openPi != null) builder.setContentIntent(openPi)
        
        // Use a dynamic notification ID unique to the file (destPath.hashCode() and 0xFFF)
        // so multiple files don't overwrite each other!
        val notifId = NOTIF_ID_FILE_BASE + (destPath.hashCode() and 0xFFF)
        notificationManager.notify(notifId, builder.build())
    }

    private fun cancelFileTransferNotification(tid: String) {
        notificationManager.cancel(transferNotifId(tid))
    }

    private fun transferNotifId(tid: String): Int = NOTIF_ID_FILE_BASE + (tid.hashCode() and 0xFFF)

    private fun formatBytes(bytes: Long): String = when {
        bytes >= 1_048_576L -> "%.1f MB".format(bytes / 1_048_576.0)
        bytes >= 1_024L     -> "%.0f KB".format(bytes / 1_024.0)
        else                -> "$bytes B"
    }

    private fun formatEta(seconds: Long): String = when {
        seconds < 0L -> ""
        seconds < 60L -> "${seconds}s"
        seconds < 3_600L -> "${seconds / 60}m"
        else -> "${seconds / 3_600}h"
    }

    private fun buildTransferStatusLine(
        percent: Int,
        bytesReceived: Long,
        speedBps: Long,
        etaSecs: Long
    ): String {
        val parts = mutableListOf("${percent}%")
        if (bytesReceived > 0L) {
            parts += formatBytes(bytesReceived)
        }
        if (speedBps > 0L) {
            parts += "${formatBytes(speedBps)}/s"
        }
        if (etaSecs >= 0L) {
            parts += "ETA ${formatEta(etaSecs)}"
        }
        return parts.joinToString("  ·  ")
    }

    private fun isCriticalFailure(msg: String): Boolean =
        msg.contains("heartbeat timeout", ignoreCase = true) ||
        msg.contains("network lost", ignoreCase = true) ||
        msg.contains("listener rebind failed", ignoreCase = true)

    // ── Clipboard watch (Kotlin → Rust) ──────────────────────────────────────

    private fun scheduleClipboardWatch() {
        val interval = clipInterval
        handler.postDelayed(object : Runnable {
            override fun run() {
                checkClipboard()
                if (engineHandle != 0L) {
                    handler.postDelayed(this, clipInterval)
                }
            }
        }, interval)
    }

    private fun checkClipboard() {
        if (engineHandle == 0L || !isSyncEnabled()) return
        if (!hasConnectedPeers()) return
        if (suppressNext) { suppressNext = false; return }

        val clip = clipboardManager.primaryClip ?: return
        if (clip.itemCount == 0) return
        val item = clip.getItemAt(0)

        val text = item.text?.toString()?.trim()
        if (!text.isNullOrEmpty()) {
            val sig = "text:${text.hashCode()}"
            if (sig != lastClipboardSignature) {
                lastClipboardSignature = sig
                DeskdropJni.pushText(engineHandle, text)
            }
            return
        }

        val uri = item.uri ?: return
        val sig = "uri:$uri"
        if (sig == lastClipboardSignature) return

        val clipboardMime = contentResolver.getType(uri).orEmpty()
        if (!clipboardMime.startsWith("image/")) {
            val staged = stageSharedUri(uri, preferredName = null, fallbackIndex = 1)
            if (staged != null) {
                lastClipboardSignature = sig
                val result = DeskdropJni.sendFilePath(
                    engineHandle,
                    staged.localFile.absolutePath,
                    staged.displayName,
                    staged.mimeType,
                    null
                )
                if (result == 1) {
                    addToFeed(
                        ActivityEntry(
                            deviceName = "All devices",
                            kind = ActivityKind.FILE_SENT,
                            preview = staged.displayName
                        )
                    )
                    broadcastStatus()
                }
                return
            }
        }

        when (val payload = readClipboardUri(uri)) {
            null -> Unit
            is OutgoingPayload.Image -> {
                lastClipboardSignature = sig
                DeskdropJni.pushImage(engineHandle, payload.mime, payload.data)
            }
            is OutgoingPayload.File -> {
                lastClipboardSignature = sig
                DeskdropJni.pushFile(engineHandle, payload.name, payload.data)
            }
        }
    }

    private fun sendSharedUris(
        uriStrings: List<String>,
        preferredName: String?,
        targetDeviceId: String?
    ) {
        if (engineHandle == 0L) return
        if (!hasConnectedPeers()) {
            Log.i(TAG, "Ignoring shared URIs because no peers are connected")
            return
        }
        if (targetDeviceId != null && !isPeerConnected(targetDeviceId)) {
            Log.w(TAG, "Ignoring shared URIs because target peer is disconnected: $targetDeviceId")
            return
        }
        var sentAny = false
        uriStrings.forEachIndexed { index, rawUri ->
            val uri = runCatching { Uri.parse(rawUri) }.getOrNull() ?: return@forEachIndexed
            val staged = stageSharedUri(
                uri = uri,
                preferredName = preferredName?.takeIf { uriStrings.size == 1 },
                fallbackIndex = index + 1,
            )
            if (staged == null) {
                Log.w(TAG, "Unable to stage shared URI: $rawUri")
                return@forEachIndexed
            }

            val result = DeskdropJni.sendFilePath(
                engineHandle,
                staged.localFile.absolutePath,
                staged.displayName,
                staged.mimeType,
                targetDeviceId
            )
            if (result == 1) {
                sentAny = true
                Log.i(
                    TAG,
                    "Queued shared URI ${staged.displayName} (${staged.localFile.length()} bytes) for target=${targetDeviceId ?: "all"}"
                )
                val targetName = if (targetDeviceId != null) {
                    connectedPeerIds[targetDeviceId] ?: "Device"
                } else if (connectedPeerIds.size == 1) {
                    connectedPeerIds.values.first()
                } else {
                    "All devices"
                }
                addToFeed(
                    ActivityEntry(
                        deviceName = targetName,
                        kind = ActivityKind.FILE_SENT,
                        preview = staged.displayName
                    )
                )
            } else {
                Log.w(TAG, "Failed to queue staged file transfer for ${staged.displayName}")
            }
        }
        if (sentAny) {
            persistStatus()
            broadcastStatus()
        }
    }

    // ── Apply incoming clipboard ──────────────────────────────────────────────

    private fun applyText(text: String, from: String) {
        suppressNext = true
        lastClipboardSignature = "text:${text.hashCode()}"
        clipboardManager.setPrimaryClip(
            android.content.ClipData.newPlainText("deskdrop", text)
        )

        // Silently add to activity feed — zero notification
        addToFeed(
            ActivityEntry(
                deviceName = from,
                kind = ActivityKind.CLIPBOARD_TEXT,
                preview = text.take(100)
            )
        )
        broadcastStatus()

        // Respect user opt-in for clipboard copy notifications (default OFF)
        if (isClipboardNotifyEnabled()) {
            updateForegroundNotification() // update subtitle only — no new notification
        }
    }

    private fun applyBinaryClipboard(
        data: ByteArray,
        name: String,
        mime: String?,
        from: String,
        isFile: Boolean
    ) {
        val saveDir = if (isFile) getDownloadsDir() else cacheDir
        val file = writeBinaryFile(name, data, mime, saveDir)

        // Copy file to public Downloads if it is a file
        val finalFile = if (isFile) {
            saveFileToPublicDownloads(file) ?: file
        } else {
            file
        }

        val uri = FileProvider.getUriForFile(this, "$packageName.fileprovider", file) // use secure private file for URI
        suppressNext = true
        lastClipboardSignature = "uri:$uri"
        clipboardManager.setPrimaryClip(
            android.content.ClipData.newUri(contentResolver, finalFile.name, uri)
        )

        val kind = if (mime?.startsWith("image/") == true) {
            ActivityKind.CLIPBOARD_IMAGE
        } else {
            ActivityKind.FILE_RECEIVED
        }
        addToFeed(ActivityEntry(deviceName = from, kind = kind, preview = finalFile.name))
        broadcastStatus()

        if (isFile) {
            // Files always get an explicit notification — user needs to know where it landed
            showFileReceivedNotification(from, finalFile.name, uri)
        }
        // Images and clipboard binary: silent — activity feed only
    }

    // ── File I/O ──────────────────────────────────────────────────────────────

    private fun getDownloadsDir(): File {
        val base = getExternalFilesDir(android.os.Environment.DIRECTORY_DOWNLOADS) ?: filesDir
        return File(base, "Deskdrop").also { it.mkdirs() }
    }

    private fun saveFileToPublicDownloads(sourceFile: File): File? {
        if (!sourceFile.exists()) return null
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val resolver = contentResolver
            val contentValues = android.content.ContentValues().apply {
                put(android.provider.MediaStore.MediaColumns.DISPLAY_NAME, sourceFile.name)
                put(android.provider.MediaStore.MediaColumns.MIME_TYPE, "*/*")
                put(android.provider.MediaStore.MediaColumns.RELATIVE_PATH, android.os.Environment.DIRECTORY_DOWNLOADS)
            }
            val uri = resolver.insert(android.provider.MediaStore.Downloads.EXTERNAL_CONTENT_URI, contentValues) ?: return null
            try {
                resolver.openOutputStream(uri)?.use { outStream ->
                    java.io.FileInputStream(sourceFile).use { inStream ->
                        inStream.copyTo(outStream)
                    }
                }
                return File(android.os.Environment.getExternalStoragePublicDirectory(android.os.Environment.DIRECTORY_DOWNLOADS), sourceFile.name)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to copy file to public Downloads using MediaStore", e)
                return null
            }
        } else {
            // For Android 9 and below, write directly using file system
            val destDir = android.os.Environment.getExternalStoragePublicDirectory(android.os.Environment.DIRECTORY_DOWNLOADS)
            destDir.mkdirs()
            val destFile = File(destDir, sourceFile.name)
            try {
                java.io.FileInputStream(sourceFile).use { inStream ->
                    java.io.FileOutputStream(destFile).use { outStream ->
                        inStream.copyTo(outStream)
                    }
                }
                return destFile
            } catch (e: Exception) {
                Log.e(TAG, "Failed to copy file to public Downloads using file APIs", e)
                return null
            }
        }
    }

    private fun writeBinaryFile(
        name: String,
        data: ByteArray,
        mime: String?,
        dir: File
    ): File {
        dir.mkdirs()
        val ext = mime?.let {
            MimeTypeMap.getSingleton().getExtensionFromMimeType(it.substringBefore(';'))
        }?.takeIf { it.isNotBlank() }

        val safe = sanitize(name, ext)
        var target = File(dir, safe)
        var n = 2
        while (target.exists()) {
            val stem = target.nameWithoutExtension
            val suf  = target.extension.takeIf { it.isNotBlank() }?.let { ".$it" }.orEmpty()
            target = File(dir, "$stem-$n$suf")
            n++
        }
        FileOutputStream(target).use { it.write(data) }
        return target
    }

    private fun sanitize(raw: String, fallbackExt: String?): String {
        val clean = raw.trim().replace(Regex("[/:\\\\*?\"<>|]"), "-")
        if (clean.isNotEmpty()) return clean
        return if (fallbackExt.isNullOrBlank()) "deskdrop-file" else "deskdrop-file.$fallbackExt"
    }

    private fun readClipboardUri(uri: Uri): OutgoingPayload? = readOutgoingUri(uri, preferredName = null)

    private fun readOutgoingUri(uri: Uri, preferredName: String?): OutgoingPayload? = runCatching {
        val mime = resolveUriMimeType(uri).orEmpty()
        val name = resolveUriDisplayName(
            uri = uri,
            preferredName = preferredName,
            fallbackName = "file",
        )
        // Prevent OOM and protocol frame size errors for massive files (e.g. videos/panoramas)
        // Limit clipboard pushes to 32MB. Larger files must use standard file transfer.
        var size = 0L
        contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) {
                val sizeIndex = cursor.getColumnIndex(android.provider.OpenableColumns.SIZE)
                if (sizeIndex != -1) {
                    size = cursor.getLong(sizeIndex)
                }
            }
        }
        if (size > 32L * 1024 * 1024) {
            Log.w(TAG, "Skipping clipboard payload > 32MB ($size bytes). Please use 'Send Files' instead.")
            return@runCatching null
        }

        val bytes = openUriInputStream(uri)?.use { it.readBytes() } ?: return null
        if (mime.startsWith("image/")) OutgoingPayload.Image(mime.ifEmpty { "image/png" }, bytes)
        else OutgoingPayload.File(name, bytes)
    }.onFailure { Log.w(TAG, "Failed to read clipboard URI $uri", it) }.getOrNull()

    private fun imageNameForMime(mime: String): String {
        val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime.substringBefore(';')) ?: "png"
        return "Deskdrop-image.$ext"
    }

    private fun textContentHash(text: String): String {
        val digest = MessageDigest.getInstance("SHA-256")
        digest.update('T'.code.toByte())
        digest.update(text.toByteArray(Charsets.UTF_8))
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    private sealed interface OutgoingPayload {
        data class Image(val mime: String, val data: ByteArray) : OutgoingPayload
        data class File(val name: String, val data: ByteArray) : OutgoingPayload
    }

    private data class StagedOutgoingFile(
        val localFile: File,
        val displayName: String,
        val mimeType: String,
    )

    private fun stageSharedUri(
        uri: Uri,
        preferredName: String?,
        fallbackIndex: Int,
    ): StagedOutgoingFile? = runCatching {
        val mime = resolveUriMimeType(uri)
            ?.takeIf { it.isNotBlank() }
            ?: "application/octet-stream"
        val ext = MimeTypeMap.getSingleton()
            .getExtensionFromMimeType(mime.substringBefore(';'))
        val displayName = resolveUriDisplayName(
            uri = uri,
            preferredName = preferredName,
            fallbackName = "Shared file $fallbackIndex",
        )
        val stagedDir = File(cacheDir, "shared-outgoing").also { it.mkdirs() }
        cleanupStagedOutgoingFiles(stagedDir)
        val stagedFile = uniqueFileInDir(stagedDir, sanitize(displayName, ext))
        openUriInputStream(uri)?.use { input ->
            FileOutputStream(stagedFile).use { output ->
                input.copyTo(output, 256 * 1024)
            }
        } ?: return null
        StagedOutgoingFile(stagedFile, displayName, mime)
    }.onFailure { Log.w(TAG, "Failed to stage shared URI $uri", it) }.getOrNull()

    private fun resolveUriDisplayName(
        uri: Uri,
        preferredName: String?,
        fallbackName: String,
    ): String {
        preferredName?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }

        if (uri.scheme.equals("file", ignoreCase = true)) {
            uri.path
                ?.let(::File)
                ?.name
                ?.takeIf { it.isNotBlank() }
                ?.let { return it }
        }

        val cursor = contentResolver.query(
            uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null
        )
        cursor?.use {
            val col = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (col >= 0 && it.moveToFirst()) {
                it.getString(col)?.takeIf(String::isNotBlank)?.let { displayName -> return displayName }
            }
        }

        return uri.lastPathSegment?.takeIf { it.isNotBlank() } ?: fallbackName
    }

    private fun resolveUriMimeType(uri: Uri): String? {
        contentResolver.getType(uri)
            ?.takeIf { it.isNotBlank() }
            ?.let { return it }

        if (uri.scheme.equals("file", ignoreCase = true)) {
            val ext = uri.path
                ?.let(::File)
                ?.extension
                ?.lowercase()
                ?.takeIf { it.isNotBlank() }
            if (ext != null) {
                MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext)?.let { return it }
            }
        }

        return null
    }

    private fun openUriInputStream(uri: Uri): InputStream? {
        if (uri.scheme.equals("file", ignoreCase = true)) {
            val file = uri.path?.let(::File)?.takeIf(File::exists) ?: return null
            return file.inputStream()
        }

        return contentResolver.openInputStream(uri)
    }

    private fun cleanupStagedOutgoingFiles(dir: File) {
        val cutoff = System.currentTimeMillis() - 12 * 60 * 60 * 1000L
        dir.listFiles()?.forEach { file ->
            if (file.lastModified() < cutoff) {
                runCatching { file.delete() }
            }
        }
    }

    private fun uniqueFileInDir(dir: File, fileName: String): File {
        var candidate = File(dir, fileName)
        if (!candidate.exists()) return candidate

        val stem = candidate.nameWithoutExtension.ifBlank { "deskdrop-share" }
        val ext = candidate.extension.takeIf { it.isNotBlank() }?.let { ".$it" }.orEmpty()
        var index = 2
        while (candidate.exists()) {
            candidate = File(dir, "$stem-$index$ext")
            index++
        }
        return candidate
    }

    // ── Call continuity ──────────────────────────────────────────────────────
    //
    // Remote call actions (accept/decline) from the Mac are executed via TelecomManager.

    private var callStateReceiver: android.content.BroadcastReceiver? = null

    private fun onCallStateUpdate(state: Int, incomingNumber: String?) {
        val stateStr = when (state) {
            android.telephony.TelephonyManager.CALL_STATE_RINGING -> "ringing"
            android.telephony.TelephonyManager.CALL_STATE_OFFHOOK -> "offhook"
            android.telephony.TelephonyManager.CALL_STATE_IDLE    -> "idle"
            else -> return
        }
        val number  = incomingNumber.orEmpty()
        val contact = resolveContactName(number)
        Log.i(TAG, "Call state: $stateStr number=$number contact=$contact")
        val h = engineHandle
        if (h != 0L) {
            DeskdropJni.pushCallState(h, stateStr, number, contact)
        }
        // Show/dismiss the Android-side call notification
        when (stateStr) {
            "ringing" -> showIncomingCallNotification(number, contact)
            "idle", "offhook" -> notificationManager.cancel(NOTIF_ID_CALL)
        }
    }

    private fun startCallStateMonitor() {
        if (!hasCallPermissions()) {
            Log.i(TAG, "Call continuity: missing READ_PHONE_STATE permission — skipping")
            return
        }
        stopCallStateMonitor()

        val receiver = object : android.content.BroadcastReceiver() {
            override fun onReceive(context: android.content.Context, intent: Intent) {
                if (intent.action == android.telephony.TelephonyManager.ACTION_PHONE_STATE_CHANGED) {
                    val stateStr = intent.getStringExtra(android.telephony.TelephonyManager.EXTRA_STATE)
                    val number = intent.getStringExtra(android.telephony.TelephonyManager.EXTRA_INCOMING_NUMBER)
                    val state = when (stateStr) {
                        android.telephony.TelephonyManager.EXTRA_STATE_RINGING -> android.telephony.TelephonyManager.CALL_STATE_RINGING
                        android.telephony.TelephonyManager.EXTRA_STATE_OFFHOOK -> android.telephony.TelephonyManager.CALL_STATE_OFFHOOK
                        android.telephony.TelephonyManager.EXTRA_STATE_IDLE -> android.telephony.TelephonyManager.CALL_STATE_IDLE
                        else -> -1
                    }
                    if (state != -1) {
                        onCallStateUpdate(state, number)
                    }
                }
            }
        }
        val filter = android.content.IntentFilter(android.telephony.TelephonyManager.ACTION_PHONE_STATE_CHANGED)
        registerReceiver(receiver, filter)
        callStateReceiver = receiver
        Log.i(TAG, "Call state monitor started (BroadcastReceiver)")
    }

    private fun stopCallStateMonitor() {
        callStateReceiver?.let {
            unregisterReceiver(it)
            callStateReceiver = null
        }
    }

    private fun hasCallPermissions(): Boolean =
        checkSelfPermission(android.Manifest.permission.READ_PHONE_STATE) ==
            android.content.pm.PackageManager.PERMISSION_GRANTED

    private fun resolveContactName(number: String): String {
        if (number.isBlank()) return ""
        if (checkSelfPermission(android.Manifest.permission.READ_CONTACTS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) return ""
        return runCatching {
            val uri = android.net.Uri.withAppendedPath(
                android.provider.ContactsContract.PhoneLookup.CONTENT_FILTER_URI,
                android.net.Uri.encode(number)
            )
            contentResolver.query(
                uri,
                arrayOf(android.provider.ContactsContract.PhoneLookup.DISPLAY_NAME),
                null, null, null
            )?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) ?: "" else ""
            } ?: ""
        }.getOrDefault("")
    }

    /** Show a high-priority heads-up notification for an incoming call on Android. */
    private fun showIncomingCallNotification(number: String, contactName: String) {
        val callerLabel = when {
            contactName.isNotBlank() -> contactName
            number.isNotBlank()      -> number
            else                     -> "Unknown caller"
        }

        // Tapping the notification opens the app
        val openPi = PendingIntent.getActivity(
            this, 0,
            packageManager.getLaunchIntentForPackage(packageName),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        val notif = NotificationCompat.Builder(this, CHAN_CALLS)
            .setSmallIcon(android.R.drawable.stat_sys_phone_call)
            .setContentTitle("📞 Incoming call")
            .setContentText(callerLabel)
            .setSubText("Deskdrop — relaying to your Mac")
            .setCategory(NotificationCompat.CATEGORY_CALL)
            .setPriority(NotificationCompat.PRIORITY_MAX)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setOngoing(true)
            .setAutoCancel(false)
            .setContentIntent(openPi)
            .setStyle(NotificationCompat.BigTextStyle()
                .bigText("$callerLabel is calling. Your Mac will show a banner with Accept/Decline.")
                .setSummaryText("Call relay active"))
            .build()

        notificationManager.notify(NOTIF_ID_CALL, notif)
    }

    @Suppress("DEPRECATION")
    private fun handleRemoteCallAction(action: String) {
        if (action == "accept" || action == "decline") {
            Log.i(TAG, "Attempting remote call action '$action' via NotificationListener...")
            if (DeskdropNotificationListener.triggerCallAction(action)) {
                Log.i(TAG, "Remote call action '$action' successfully triggered via NotificationListener!")
                return
            }
            Log.i(TAG, "NotificationListener could not handle call action '$action' (maybe not enabled or call notif not found), falling back to TelecomManager...")
        }

        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
            val tm = getSystemService(TELECOM_SERVICE) as? android.telecom.TelecomManager ?: return
            when (action) {
                "accept" -> {
                    if (checkSelfPermission(android.Manifest.permission.ANSWER_PHONE_CALLS) ==
                        android.content.pm.PackageManager.PERMISSION_GRANTED) {
                        runCatching { 
                            tm.acceptRingingCall()
                        }
                            .onSuccess { Log.i(TAG, "Remote accept: call accepted") }
                            .onFailure { Log.w(TAG, "Remote accept failed", it) }
                    } else {
                        Log.w(TAG, "Remote accept: ANSWER_PHONE_CALLS permission not granted")
                    }
                }
                "decline" -> {
                    if (checkSelfPermission(android.Manifest.permission.ANSWER_PHONE_CALLS) ==
                        android.content.pm.PackageManager.PERMISSION_GRANTED) {
                        runCatching { tm.endCall() }
                            .onSuccess { Log.i(TAG, "Remote decline: call ended") }
                            .onFailure { Log.w(TAG, "Remote decline failed", it) }
                    } else {
                        Log.w(TAG, "Remote decline: ANSWER_PHONE_CALLS permission not granted")
                    }
                }
                "audio_earpiece" -> {
                    val am = getSystemService(android.content.Context.AUDIO_SERVICE) as? android.media.AudioManager
                    am?.isSpeakerphoneOn = false
                    am?.stopBluetoothSco()
                    am?.isBluetoothScoOn = false
                    Log.i(TAG, "Remote audio route: Earpiece")
                }
                "audio_speaker" -> {
                    val am = getSystemService(android.content.Context.AUDIO_SERVICE) as? android.media.AudioManager
                    am?.isSpeakerphoneOn = true
                    am?.stopBluetoothSco()
                    am?.isBluetoothScoOn = false
                    Log.i(TAG, "Remote audio route: Speaker")
                }
                "audio_bluetooth" -> {
                    val am = getSystemService(android.content.Context.AUDIO_SERVICE) as? android.media.AudioManager
                    am?.isSpeakerphoneOn = false
                    am?.startBluetoothSco()
                    am?.isBluetoothScoOn = true
                    Log.i(TAG, "Remote audio route: Bluetooth")
                }
                else -> Log.w(TAG, "Unknown remote call action: $action")
            }
        } else {
            Log.w(TAG, "Remote call actions require API 26+")
        }
    }

    // ── F20: Battery status monitor ────────────────────────────────────────────────────
    private var batteryReceiver: android.content.BroadcastReceiver? = null

    private fun startBatteryMonitor() {
        if (batteryReceiver != null) return
        val receiver = object : android.content.BroadcastReceiver() {
            private var lastLevel = -1
            private var lastChargingState: Boolean? = null
            override fun onReceive(context: Context, intent: Intent) {
                if (intent.action == Intent.ACTION_BATTERY_CHANGED) {
                    val rawLevel = intent.getIntExtra(android.os.BatteryManager.EXTRA_LEVEL, -1)
                    val scale = intent.getIntExtra(android.os.BatteryManager.EXTRA_SCALE, -1)
                    val status = intent.getIntExtra(android.os.BatteryManager.EXTRA_STATUS, -1)
                    
                    val level = if (rawLevel >= 0 && scale > 0) {
                        (rawLevel * 100f / scale).toInt()
                    } else {
                        rawLevel
                    }
                    
                    val charging = status == android.os.BatteryManager.BATTERY_STATUS_CHARGING ||
                                   status == android.os.BatteryManager.BATTERY_STATUS_FULL

                    val levelChanged = Math.abs(level - lastLevel) >= 1 // Update on 1% change instead of 5% for better UX
                    val statusChanged = charging != lastChargingState

                    if (levelChanged || statusChanged || lastLevel == -1) {
                        lastLevel = level
                        lastChargingState = charging

                        val h = engineHandle
                        if (h != 0L && level >= 0) {
                            Log.i(TAG, "Battery status update: level=$level charging=$charging")
                            DeskdropJni.pushBatteryStatus(h, level, charging)
                        }
                    }
                }
            }
        }
        val filter = android.content.IntentFilter(Intent.ACTION_BATTERY_CHANGED)
        registerReceiver(receiver, filter)
        batteryReceiver = receiver
        Log.i(TAG, "Battery status monitor started")
    }

    private fun stopBatteryMonitor() {
        batteryReceiver?.let {
            runCatching { unregisterReceiver(it) }
        }
        batteryReceiver = null
        Log.i(TAG, "Battery status monitor stopped")
    }

    // ── NSD (Network Service Discovery) ────────────────────────────────────────────────
    //
    // Android does not support Rust’s mdns-sd crate, so we use the
    // platform NSD API here to:
    //   1. Advertise our service (“_deskdrop._tcp”) so the Mac discovers us.
    //   2. Browse for the Mac’s _deskdrop._tcp advertisement.
    //   3. When resolved, call connectToPeer() via JNI so the Rust engine
    //      initiates a TCP handshake.

    private fun startNsdDiscovery() {
        val nm = runCatching { getSystemService(NSD_SERVICE) as NsdManager }.getOrNull()
            ?: run { Log.w(TAG, "NSD: NsdManager unavailable"); return }

        // ── 1. Register our own service so the Mac can find us ───────────────────
        //
        // Include the UUID prefix in the service name so the Mac can identify us
        // even before resolving (and so our own self-filter is reliable).
        // Format: "deskdrop-<uuid8>-<safename>"
        // Android may suffix " (2)" etc. on collision — we capture the actual name
        // in onServiceRegistered so our self-filter always matches correctly.
        val uuidPrefix = myDeviceUuidPrefix ?: engineHandle.toString().take(8)
        val safeName = resolvedDeviceName()
            .take(16)
            .replace(Regex("[^A-Za-z0-9\\-]"), "-")
            .trimEnd('-')
        val serviceInfo = NsdServiceInfo().apply {
            serviceName = "deskdrop-$uuidPrefix-$safeName"
            serviceType = NSD_SERVICE_TYPE
            port        = DEFAULT_DESKDROP_PORT
            setAttribute("id", myDeviceId ?: "")
            setAttribute("v", "3")
        }

        val regListener = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {
                // Store the ACTUAL registered name (Android may have renamed it on collision).
                // The self-filter in makeResolveListener() uses this to skip our own service.
                myActualNsdName = info.serviceName
                Log.i(TAG, "NSD: registered '${info.serviceName}'")
            }
            override fun onRegistrationFailed(info: NsdServiceInfo, code: Int) {
                Log.w(TAG, "NSD: registration failed (code=$code)")
            }
            override fun onServiceUnregistered(info: NsdServiceInfo) {
                myActualNsdName = null
                Log.i(TAG, "NSD: unregistered '${info.serviceName}'")
            }
            override fun onUnregistrationFailed(info: NsdServiceInfo, code: Int) {
                Log.w(TAG, "NSD: unregistration failed (code=$code)")
            }
        }
        nsdRegistrationListener = regListener
        runCatching { nm.registerService(serviceInfo, NsdManager.PROTOCOL_DNS_SD, regListener) }
            .onFailure { Log.w(TAG, "NSD: registerService error", it) }

        // ── 2. Browse for Deskdrop peers (the Mac, other desktops) ──────────────
        val discListener = object : NsdManager.DiscoveryListener {
            override fun onStartDiscoveryFailed(serviceType: String, code: Int) {
                Log.w(TAG, "NSD: discovery start failed (code=$code)")
            }
            override fun onStopDiscoveryFailed(serviceType: String, code: Int) {
                Log.w(TAG, "NSD: discovery stop failed (code=$code)")
            }
            override fun onDiscoveryStarted(serviceType: String) {
                Log.i(TAG, "NSD: discovery started for $serviceType")
            }
            override fun onDiscoveryStopped(serviceType: String) {
                Log.i(TAG, "NSD: discovery stopped")
            }
            override fun onServiceFound(info: NsdServiceInfo) {
                // Quick pre-filter: skip our own service by name before resolving.
                // resolveService is a limited resource on older Android — don't waste it.
                val actual = myActualNsdName
                if (actual != null && info.serviceName == actual) {
                    Log.d(TAG, "NSD: skipping self (pre-resolve) '${info.serviceName}'")
                    return
                }
                Log.i(TAG, "NSD: found '${info.serviceName}'")
                pendingNsdResolves.offer(info)
                processNextNsdResolve()
            }
            override fun onServiceLost(info: NsdServiceInfo) {
                Log.i(TAG, "NSD: lost '${info.serviceName}'")
                // If the lost service is not ours and we're now peerless, retry.
                val actual = myActualNsdName
                if (actual == null || info.serviceName != actual) {
                    if (connectedPeerIds.isEmpty()) scheduleNsdRetry()
                }
            }
        }
        nsdDiscoveryListener = discListener
        runCatching { nm.discoverServices(NSD_SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, discListener) }
            .onFailure { Log.w(TAG, "NSD: discoverServices error", it) }
    }

    private fun getLocalIpAddresses(): Set<String> {
        val ips = mutableSetOf<String>()
        try {
            val interfaces = java.net.NetworkInterface.getNetworkInterfaces()
            if (interfaces != null) {
                for (intf in interfaces) {
                    val addrs = intf.inetAddresses
                    for (addr in addrs) {
                        if (!addr.isLoopbackAddress) {
                            val hostAddr = addr.hostAddress
                            if (hostAddr != null) {
                                ips.add(hostAddr.substringBefore('%')) // Remove IPv6 scope if present
                            }
                        }
                    }
                }
            }
        } catch (ex: Exception) {
            Log.e(TAG, "Failed to get local IPs", ex)
        }
        return ips
    }

    /** Creates a one-shot resolve listener. NSD requires a unique instance per call. */
    private fun makeResolveListener(): NsdManager.ResolveListener {
        return object : NsdManager.ResolveListener {
            override fun onResolveFailed(info: NsdServiceInfo, code: Int) {
                Log.w(TAG, "NSD: resolve failed for '${info.serviceName}' (code=$code)")
                isResolvingNsd.set(false)
                handler.post { processNextNsdResolve() }
            }
            override fun onServiceResolved(info: NsdServiceInfo) {
                try {
                val ip   = info.host?.hostAddress ?: return
                val port = info.port
                Log.i(TAG, "NSD: resolved peer at $ip:$port (service='${info.serviceName}')")
                // Skip loopback addresses (self-discovery)
                if (ip.startsWith("127.") || ip == "::1") return
                
                // Bulletproof self-connection filter: check if IP is one of our own interfaces
                if (getLocalIpAddresses().contains(ip)) {
                    Log.i(TAG, "NSD: skipping self by local IP $ip")
                    return
                }
                
                // Skip IPv6 link-local — they require a scope ID the engine can't supply.
                if (ip.startsWith("fe80:") || ip.startsWith("FE80:")) {
                    Log.d(TAG, "NSD: skipping link-local address $ip")
                    return
                }
                // Skip our own service using the actual registered name (set in onServiceRegistered).
                val actual = myActualNsdName
                if (actual != null && info.serviceName == actual) {
                    Log.d(TAG, "NSD: skipping self-resolved service '${info.serviceName}'")
                    return
                }
                // Belt-and-suspenders: also skip by UUID prefix embedded in service name.
                val prefix = myDeviceUuidPrefix
                if (prefix != null && info.serviceName.contains(prefix, ignoreCase = true)) {
                    Log.d(TAG, "NSD: skipping self by UUID prefix '${info.serviceName}'")
                    return
                }

                val peerVersion = info.attributeString("v")
                if (peerVersion != null && peerVersion != "3") {
                    Log.i(TAG, "NSD: skipping ${info.serviceName} due to protocol version $peerVersion")
                    return
                }

                val peerDeviceId = info.attributeString("id")
                val myId = myDeviceId
                if (!peerDeviceId.isNullOrBlank() && !myId.isNullOrBlank()) {
                    if (peerDeviceId.equals(myId, ignoreCase = true)) {
                        Log.d(TAG, "NSD: skipping self-resolved peer id $peerDeviceId")
                        return
                    }
                    if (!shouldInitiateDiscoveredSession(myId, peerDeviceId)) {
                        Log.i(
                            TAG,
                            "NSD: $peerDeviceId should initiate against $myId; waiting for inbound session"
                        )
                        return
                    }
                }

                val h = engineHandle
                if (h != 0L) {
                    val result = DeskdropJni.connectToPeer(h, ip, port)
                    if (result == 0) {
                        Log.i(TAG, "NSD: connectToPeer($ip:$port) queued")
                        nsdRetryCount.set(0L)
                    } else {
                        Log.w(TAG, "NSD: connectToPeer($ip:$port) failed (result=$result)")
                    }
                }
                } finally {
                    isResolvingNsd.set(false)
                    handler.post { processNextNsdResolve() }
                }
            }
        }
    }

    private fun processNextNsdResolve() {
        if (!isResolvingNsd.compareAndSet(false, true)) return
        val info = pendingNsdResolves.poll()
        if (info == null) {
            isResolvingNsd.set(false)
            return
        }
        val nm = runCatching { getSystemService(NSD_SERVICE) as NsdManager }.getOrNull()
        if (nm == null) {
            isResolvingNsd.set(false)
            return
        }
        runCatching { nm.resolveService(info, makeResolveListener()) }
            .onFailure {
                Log.w(TAG, "NSD: resolveService error", it)
                isResolvingNsd.set(false)
                handler.post { processNextNsdResolve() }
            }
    }

    private fun stopNsdDiscovery() {
        val nm = runCatching { getSystemService(NSD_SERVICE) as NsdManager }.getOrNull() ?: return
        nsdDiscoveryListener?.let  { runCatching { nm.stopServiceDiscovery(it) } }
        nsdRegistrationListener?.let { runCatching { nm.unregisterService(it) } }
        nsdDiscoveryListener    = null
        nsdRegistrationListener = null
        pendingNsdResolves.clear()
        isResolvingNsd.set(false)
    }

    // ── Network change callback ───────────────────────────────────────────────
    //
    // Restarts NSD whenever the device gains a new WiFi network (e.g. waking
    // from sleep, switching APs, reconnecting after a drop).  Without this,
    // the engine stays silently disconnected until the user kills and relaunches.

    private fun registerNetworkCallback() {
        val cm = runCatching {
            getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager
        }.getOrNull() ?: return

        val cb = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                Log.i(TAG, "Network: default network available — restarting discovery + reconnecting peers")
                handler.post {
                    // Brief delay lets the IP stack settle before mDNS re-registers.
                    handler.postDelayed({
                        restartDiscoveryNow()
                        // Immediately tell the Rust engine to reconnect all known peers.
                        val h = engineHandle
                        if (h != 0L) {
                            Thread {
                                DeskdropJni.notifyNetworkRestored(h)
                            }.start()
                        }
                    }, 1_500L)
                }
            }

            override fun onLost(network: Network) {
                Log.i(TAG, "Network: default network lost — stopping discovery, scheduling retry")
                handler.post {
                    stopNsdDiscovery()
                    scheduleNsdRetry()
                }
            }
        }

        runCatching { cm.registerDefaultNetworkCallback(cb) }
            .onSuccess { networkCallback = cb }
            .onFailure { Log.w(TAG, "Network: failed to register callback", it) }
    }

    private fun unregisterNetworkCallback() {
        val cb = networkCallback ?: return
        networkCallback = null
        val cm = runCatching {
            getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager
        }.getOrNull() ?: return
        runCatching { cm.unregisterNetworkCallback(cb) }
    }

    // ── NSD retry with exponential backoff ────────────────────────────────────
    //
    // When all peers disconnect (or we lose WiFi and regain it), we schedule a
    // fresh NSD scan with exponential backoff: 5 s → 10 s → 20 s → 40 s → 60 s.
    // This covers the case where the Mac wakes up after the Android, or the
    // Android reconnects to a network before the Mac's mDNS advertisement is live.

    private fun scheduleNsdRetry() {
        cancelNsdRetry()
        val attempt = nsdRetryCount.getAndIncrement()
        val delayMs = minOf(5_000L * (1L shl attempt.coerceAtMost(3).toInt()), 60_000L)
        Log.i(TAG, "NSD retry #$attempt scheduled in ${delayMs}ms")
        val r = Runnable {
            if (engineHandle != 0L && connectedPeerIds.isEmpty()) {
                Log.i(TAG, "NSD retry: restarting discovery")
                stopNsdDiscovery()
                startNsdDiscovery()
                // Keep retrying until we connect or network is restored.
                if (connectedPeerIds.isEmpty()) scheduleNsdRetry()
            }
        }
        nsdRetryRunnable = r
        handler.postDelayed(r, delayMs)
    }

    private fun cancelNsdRetry() {
        nsdRetryRunnable?.let { handler.removeCallbacks(it) }
        nsdRetryRunnable = null
    }

    private fun NsdServiceInfo.attributeString(key: String): String? =
        attributes[key]
            ?.let { bytes -> String(bytes, StandardCharsets.UTF_8).trim() }
            ?.takeIf { it.isNotEmpty() }

    private fun shouldInitiateDiscoveredSession(myId: String, peerId: String): Boolean {
        val normalizedMine = normalizeUuidForCompare(myId) ?: return true
        val normalizedPeer = normalizeUuidForCompare(peerId) ?: return true
        return normalizedMine < normalizedPeer
    }

    private fun normalizeUuidForCompare(raw: String): String? =
        runCatching { UUID.fromString(raw) }.getOrNull()
            ?.toString()
            ?.replace("-", "")
            ?.lowercase()

    private fun registerPairingReceiver() {
        if (pairingReceiverRegistered) return
        ContextCompat.registerReceiver(
            this,
            pairingResultReceiver,
            IntentFilter(PairingActivity.ACTION_PAIRING_RESULT),
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
        pairingReceiverRegistered = true
    }

    private fun unregisterPairingReceiver() {
        if (!pairingReceiverRegistered) return
        runCatching { unregisterReceiver(pairingResultReceiver) }
        pairingReceiverRegistered = false
    }

    // ── Live settings application ─────────────────────────────────────────────
    //
    // Called when SettingsActivity broadcasts ACTION_SETTINGS_CHANGED.
    // Reads the current SharedPreferences and pushes them to the running
    // engine so changes take effect without a service restart.

    private fun applySettingsToEngine() {
        val h = engineHandle
        if (h == 0L) return
        val p = prefs()
        val syncEnabled = p.getBoolean("sync_enabled", true)
        val syncText    = p.getBoolean("sync_text",    true)
        val syncImages  = p.getBoolean("sync_images",  true)
        val syncFiles   = p.getBoolean("sync_files",   true)
        Log.i(TAG, "Applying settings: sync=$syncEnabled text=$syncText images=$syncImages files=$syncFiles")
        // Push to engine — JNI call updates the engine's sync filter flags atomically.
        DeskdropJni.applySyncSettings(h, syncEnabled, syncText, syncImages, syncFiles)
        // If sync was just disabled, cancel any pending clipboard notifications.
        if (!syncEnabled) {
            notificationManager.cancel(NOTIF_ID_CLIPBOARD_AVAILABLE)
        }
    }

    // ── Device name ───────────────────────────────────────────────────────────

    private fun resolvedDeviceName(): String {
        prefs().getString("device_name", null)?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }
        Settings.Global.getString(contentResolver, "device_name")?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }
        val mfr   = Build.MANUFACTURER.orEmpty().trim()
        val model = Build.MODEL.orEmpty().trim()
        return if (model.startsWith(mfr, ignoreCase = true)) model else "$mfr $model".trim()
    }

    // ── Notification channels ─────────────────────────────────────────────────

    private fun createNotificationChannels() {
        val nm = getSystemService(NotificationManager::class.java)

        // Channel A: persistent foreground indicator — must be as quiet as possible
        nm.createNotificationChannel(NotificationChannel(
            CHAN_SERVICE,
            "Deskdrop",
            NotificationManager.IMPORTANCE_MIN          // no sound, no vibration, no heads-up
        ).apply {
            description = "Deskdrop background sync indicator"
            setShowBadge(false)
            enableLights(false)
            enableVibration(false)
            setSound(null, null)
        })

        // Channel B: trust requests, file receives, critical failures
        nm.createNotificationChannel(NotificationChannel(
            CHAN_ALERTS,
            "Deskdrop Alerts",
            NotificationManager.IMPORTANCE_HIGH
        ).apply {
            description = "Trust requests, received files, connection failures"
            setShowBadge(true)
            enableLights(true)
            enableVibration(true)
        })

        // Channel C: incoming call relay banner — full heads-up priority
        nm.createNotificationChannel(NotificationChannel(
            CHAN_CALLS,
            "Deskdrop Calls",
            NotificationManager.IMPORTANCE_HIGH
        ).apply {
            description = "Incoming call relay notifications from your phone"
            setShowBadge(true)
            enableVibration(true)
            enableLights(true)
            setBypassDnd(true)  // show even in Do Not Disturb
        })
    }

    // ── Foreground notification ───────────────────────────────────────────────
    //
    // ONE notification, ALWAYS the same ID.
    // Silent — no sound, no vibration, no heads-up banner.
    // Two action buttons: [Pause Sync] / [Resume Sync] and [Disconnect]

    private fun buildForegroundNotification(): Notification {
        val launchPi = PendingIntent.getActivity(
            this, 0,
            packageManager.getLaunchIntentForPackage(packageName),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        val syncEnabled = isSyncEnabled()

        // Pause/Resume Sync action
        val syncActionLabel = if (syncEnabled) "Pause Sync" else "Resume Sync"
        val syncActionIntent = Intent(this, DeskdropService::class.java).apply {
            action = if (syncEnabled) ACTION_PAUSE_SYNC else ACTION_RESUME_SYNC
        }
        val syncActionPi = PendingIntent.getService(
            this, 10,
            syncActionIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        return NotificationCompat.Builder(this, CHAN_SERVICE)
            .setContentTitle(if (syncEnabled) "Deskdrop is Active" else "Deskdrop is Paused")
            .setContentText(if (connectedPeerIds.isNotEmpty()) "Secure connection established" else "Scanning for devices on LAN")
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .setVisibility(NotificationCompat.VISIBILITY_SECRET)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setContentIntent(launchPi)
            .addAction(
                if (syncEnabled) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play,
                syncActionLabel,
                syncActionPi
            )
            .build()
    }

    private fun startForegroundCompat(notification: Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIF_ID_SERVICE, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            startForeground(NOTIF_ID_SERVICE, notification)
        }
    }

    private fun updateForegroundNotification() {
        getSystemService(NotificationManager::class.java)
            .notify(NOTIF_ID_SERVICE, buildForegroundNotification())
    }

    private fun foregroundStatusText(): String {
        if (!isSyncEnabled()) return "Sync paused · tap to manage"
        return when (connectedPeerIds.size) {
            0    -> "Active · no devices nearby"
            1    -> "Active · ${connectedPeerIds.values.first()}"
            else -> "Active · ${connectedPeerIds.size} devices connected"
        }
    }

    // ── Alert notifications ───────────────────────────────────────────────────
    //
    // These use CHAN_ALERTS — they CAN make sound/vibration.
    // Only fired for: trust request, file received, critical failure.
    // NEVER fired for: clipboard text/image sync.

    private fun showPairingPrompt(deviceId: String, deviceName: String, fingerprint: String) {
        val pairingIntent = Intent(this, PairingActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
            putExtra(PairingActivity.EXTRA_DEVICE_ID, deviceId)
            putExtra(PairingActivity.EXTRA_DEVICE_NAME, deviceName)
            putExtra(PairingActivity.EXTRA_FINGERPRINT, fingerprint)
            putExtra(PairingActivity.EXTRA_PIN, pairingPin(fingerprint))
        }
        val launchPi = PendingIntent.getActivity(
            this, deviceId.hashCode(),
            pairingIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        runCatching { startActivity(pairingIntent) }

        val notif = NotificationCompat.Builder(this, CHAN_ALERTS)
            .setContentTitle("$deviceName wants to connect")
            .setContentText("Fingerprint: ${fingerprint.take(23)}…")
            .setStyle(NotificationCompat.BigTextStyle()
                .bigText("Tap to trust or deny this device.\n\nFingerprint: $fingerprint"))
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setCategory(NotificationCompat.CATEGORY_CALL)
            .setAutoCancel(true)
            .setContentIntent(launchPi)
            .build()

        getSystemService(NotificationManager::class.java).notify(NOTIF_ID_TOFU, notif)
    }

    private fun pairingPin(fingerprint: String): String {
        val digits = fingerprint
            .filter { it.isDigit() }
            .take(6)
            .padEnd(6, '0')
        return digits.ifBlank { "000000" }
    }

    private fun showFileReceivedNotification(fromDevice: String, fileName: String, uri: Uri?) {
        val openPi = uri?.let {
            val openIntent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(it, contentResolver.getType(it) ?: "*/*")
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            PendingIntent.getActivity(
                this, 30,
                Intent.createChooser(openIntent, "Open $fileName"),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
            )
        }

        val notif = NotificationCompat.Builder(this, CHAN_ALERTS)
            .setContentTitle("File received from $fromDevice")
            .setContentText(fileName)
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .setAutoCancel(true)
            .apply { if (openPi != null) setContentIntent(openPi) }
            .build()

        // Use a dynamic notification ID unique to the file (fileName.hashCode() and 0xFFF)
        // so multiple files don't overwrite each other!
        val notifId = NOTIF_ID_FILE_BASE + (fileName.hashCode() and 0xFFF)
        getSystemService(NotificationManager::class.java).notify(notifId, notif)
    }

    private fun showFailureNotification(message: String) {
        val launchPi = PendingIntent.getActivity(
            this, 40,
            packageManager.getLaunchIntentForPackage(packageName),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        val notif = NotificationCompat.Builder(this, CHAN_ALERTS)
            .setContentTitle("Deskdrop connection issue")
            .setContentText(message.take(80))
            .setSmallIcon(android.R.drawable.stat_notify_error)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_ERROR)
            .setAutoCancel(true)
            .setContentIntent(launchPi)
            .build()

        getSystemService(NotificationManager::class.java).notify(NOTIF_ID_FAILURE, notif)
    }

    // ── Status persistence ────────────────────────────────────────────────────

    // Per-peer last-sync timestamps — written when a clipboard event arrives from each peer.
    // Key: "last_sync_<peerName>", Value: System.currentTimeMillis() as String.
    private val peerLastSync = mutableMapOf<String, Long>()

    private fun currentPeerSnapshots(): List<PeerSnapshot> {
        val raw = if (engineHandle != 0L) {
            DeskdropJni.peersJson(engineHandle)
        } else {
            prefs().getString(PREF_PEER_SNAPSHOTS_JSON, null)
        }
        return parsePeerSnapshots(raw)
    }

    private fun hasConnectedPeers(): Boolean = connectedPeerIds.isNotEmpty()

    private fun isPeerConnected(deviceId: String): Boolean =
        currentPeerSnapshots().any { peer ->
            peer.isConnected && peer.id.equals(deviceId, ignoreCase = true)
        }

    private fun resolvePeerDisplayName(deviceId: String?, fallbackName: String?): String {
        val known = deviceId?.let { id ->
            currentPeerSnapshots().firstOrNull { it.id.equals(id, ignoreCase = true) }?.name
        }
        return known?.takeIf { it.isNotBlank() }
            ?: fallbackName?.takeIf { it.isNotBlank() }
            ?: "Unknown device"
    }

    private fun persistStatus() {
        val rawPeerJson = if (engineHandle != 0L) {
            DeskdropJni.peersJson(engineHandle)
        } else {
            prefs().getString(PREF_PEER_SNAPSHOTS_JSON, null)
        } ?: "[]"
        val peers = parsePeerSnapshots(rawPeerJson)
        connectedPeerIds.clear()
        peers.filter { it.isConnected }.forEach { connectedPeerIds[it.id] = it.name }
        peers.forEach { peer ->
            peer.lastSyncSecs?.let { peerLastSync[peer.name] = it * 1000L }
        }

        val editor = prefs().edit()
            .putString("local_device_name", resolvedDeviceName())
            .putString("device_id", if (engineHandle != 0L) DeskdropJni.getDeviceId(engineHandle) else null)
            .putBoolean("peer_connected", connectedPeerIds.isNotEmpty())
            .putInt("connected_count", connectedPeerIds.size)
            .putStringSet("connected_names", connectedPeerIds.values.toSet())
            .putString(PREF_PEER_SNAPSHOTS_JSON, rawPeerJson)
        // Store last-sync times so the dashboard can show "Last sync: 2m ago" per peer.
        peerLastSync.forEach { (name, ts) ->
            editor.putLong("last_sync_${name.take(32)}", ts)
        }
        editor.apply()
        broadcastStatus()
    }

    private fun broadcastStatus() {
        sendBroadcast(Intent(ACTION_STATUS_CHANGED).setPackage(packageName))
    }

    private fun setServiceRunning(running: Boolean) {
        prefs().edit()
            .putBoolean(PREF_SERVICE_RUNNING, running)
            .apply()
    }
}
