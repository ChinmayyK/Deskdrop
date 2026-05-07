// ClipRelay for Android
// Kotlin wrapper using JNI to call the Rust core.
//
// Notification UX design:
// - ONE persistent foreground notification (quiet, minimal)
// - NO per-clipboard-sync notifications (clipboard is silent/ambient)
// - Notifications ONLY for: file received, trust request, connection loss
// - Optional "notify on remote copy" setting (OFF by default)

package com.cliprelay

import android.app.*
import android.content.*
import android.content.ClipboardManager
import android.net.Uri
import android.content.pm.ServiceInfo
import android.os.*
import android.provider.OpenableColumns
import android.provider.Settings
import android.util.Log
import android.webkit.MimeTypeMap
import androidx.core.app.NotificationCompat
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream
import java.util.Locale

// ── JNI Bridge ────────────────────────────────────────────────────────────────
// NOTE: JNI function names in the .so are Java_com_proxiboard_ClipRelayJni_*
// We keep this object in the Kotlin file under the new package but the .so
// exports use the legacy prefix for binary compatibility.

object ClipRelayJni {
    init {
        System.loadLibrary("proxiboard_core")
    }

    // Event codes (must match ffi.rs)
    const val CR_EVENT_NONE              = 0
    const val CR_EVENT_CLIPBOARD_TEXT    = 1
    const val CR_EVENT_CLIPBOARD_IMAGE   = 2
    const val CR_EVENT_CLIPBOARD_FILE    = 3
    const val CR_EVENT_TOFU_PROMPT       = 4
    const val CR_EVENT_PEER_CONNECTED    = 5
    const val CR_EVENT_PEER_DISCONNECTED = 6
    const val CR_EVENT_WARNING           = 7
    const val CR_EVENT_CLIPBOARD_SYNCED  = 8

    @JvmStatic external fun start(deviceName: String?, port: Int, dataDir: String?): Long
    @JvmStatic external fun stop(handle: Long)
    @JvmStatic external fun pushText(handle: Long, text: String): Int
    @JvmStatic external fun pushImage(handle: Long, mimeType: String, data: ByteArray): Int
    @JvmStatic external fun pushFile(handle: Long, name: String, data: ByteArray): Int
    @JvmStatic external fun pollEvent(handle: Long): Long
    @JvmStatic external fun eventType(event: Long): Int
    @JvmStatic external fun eventText(event: Long): String?
    @JvmStatic external fun eventBinaryData(event: Long): ByteArray?
    @JvmStatic external fun eventDeviceName(event: Long): String?
    @JvmStatic external fun eventMimeType(event: Long): String?
    @JvmStatic external fun eventFileName(event: Long): String?
    @JvmStatic external fun eventFingerprint(event: Long): String?
    @JvmStatic external fun freeEvent(event: Long)
}

// ── Activity Feed Entry ───────────────────────────────────────────────────────

data class ActivityEntry(
    val timestamp: Long = System.currentTimeMillis(),
    val deviceName: String,
    val kind: String,   // "text", "image", "file"
    val preview: String // short text preview or filename
)

// ── ClipRelay Service ─────────────────────────────────────────────────────────

/**
 * Foreground service that:
 * 1. Starts the Rust engine via JNI.
 * 2. Monitors the Android clipboard for changes.
 * 3. Propagates changes to peers silently (no per-copy notifications).
 * 4. Receives incoming clipboard data and applies it locally.
 * 5. Maintains an in-memory activity feed instead of noisy notifications.
 */
class ClipRelayService : Service() {

    companion object {
        private const val TAG = "ClipRelay"
        private const val PREFS_NAME = "cliprelay"
        private const val NOTIF_CHANNEL_SERVICE = "cliprelay_service"
        private const val NOTIF_CHANNEL_ALERTS  = "cliprelay_alerts"
        private const val NOTIF_ID_SERVICE = 1001  // Persistent foreground notification
        private const val NOTIF_ID_TOFU    = 1002  // Trust request
        private const val NOTIF_ID_FILE    = 1003  // File received
        private const val POLL_INTERVAL_MS = 20L
        private const val CLIP_INTERVAL_MS = 200L
        private const val ACTIVITY_FEED_MAX = 50

        const val ACTION_START = "com.cliprelay.START"
        const val ACTION_STOP  = "com.cliprelay.STOP"
        const val ACTION_PUSH_TEXT = "com.cliprelay.PUSH_TEXT"
        const val ACTION_STATUS_CHANGED = "com.cliprelay.STATUS_CHANGED"

        // In-memory activity feed (static so UI can read it)
        val activityFeed = ArrayDeque<ActivityEntry>()
    }

    private var engineHandle: Long = 0L
    private val handler = Handler(Looper.getMainLooper())
    private var lastClipboardSignature: String? = null
    private var suppressNext = false
    private val connectedPeerNames = linkedSetOf<String>()

    private val clipboardManager: ClipboardManager by lazy {
        getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    override fun onCreate() {
        super.onCreate()
        createNotificationChannels()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> { stopSelf(); return START_NOT_STICKY }
            ClipRelayTileService.ACTION_SYNC_DISABLE -> {
                prefs().edit().putBoolean("sync_enabled", false).apply()
                updateForegroundNotification()
                broadcastStatus()
                return START_STICKY
            }
            ClipRelayTileService.ACTION_SYNC_ENABLE -> {
                prefs().edit().putBoolean("sync_enabled", true).apply()
                updateForegroundNotification()
                broadcastStatus()
            }
        }

        return try {
            val notification = buildForegroundNotification()
            startForegroundCompat(notification)

            if (engineHandle == 0L) {
                val deviceName = resolvedDeviceName()
                val dataDir = File(applicationContext.filesDir, "cliprelay").absolutePath
                engineHandle = ClipRelayJni.start(deviceName, 0, dataDir)
                if (engineHandle == 0L) {
                    Log.e(TAG, "Engine failed to start")
                    stopSelf()
                    return START_NOT_STICKY
                }
                Log.i(TAG, "ClipRelay engine started — device: $deviceName")
                scheduleEventDrain()
                scheduleClipboardWatch()
                persistStatus()
            }

            if (intent?.action == ACTION_PUSH_TEXT) {
                intent.getStringExtra("text")?.takeIf { it.isNotBlank() }?.let { text ->
                    if (isSyncEnabled()) {
                        ClipRelayJni.pushText(engineHandle, text)
                    }
                }
            }

            START_STICKY
        } catch (error: Throwable) {
            Log.e(TAG, "Failed to start ClipRelay foreground service", error)
            stopSelf()
            START_NOT_STICKY
        }
    }

    override fun onDestroy() {
        handler.removeCallbacksAndMessages(null)
        if (engineHandle != 0L) {
            ClipRelayJni.stop(engineHandle)
            engineHandle = 0L
        }
        connectedPeerNames.clear()
        persistStatus()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun prefs(): SharedPreferences =
        getSharedPreferences(PREFS_NAME, MODE_PRIVATE)

    private fun isSyncEnabled(): Boolean =
        prefs().getBoolean("sync_enabled", true)

    private fun isClipboardNotifyEnabled(): Boolean =
        prefs().getBoolean("notify_on_remote_copy", false) // OFF by default

    // ── Device name resolution ─────────────────────────────────────────────────

    private fun resolvedDeviceName(): String {
        prefs().getString("device_name", null)
            ?.trim()?.takeIf { it.isNotEmpty() }
            ?.let { return it }

        Settings.Global.getString(contentResolver, "device_name")
            ?.trim()?.takeIf { it.isNotEmpty() }
            ?.let { return it }

        return humanizeDeviceName()
    }

    private fun humanizeDeviceName(): String {
        val manufacturer = Build.MANUFACTURER.orEmpty().trim()
        val model = Build.MODEL.orEmpty().trim()
        return if (model.startsWith(manufacturer, ignoreCase = true)) {
            model
        } else {
            "$manufacturer $model".trim()
        }
    }

    // ── Event drain ────────────────────────────────────────────────────────────

    private fun scheduleEventDrain() {
        handler.postDelayed(object : Runnable {
            override fun run() {
                if (engineHandle != 0L) {
                    drainEvents()
                    handler.postDelayed(this, POLL_INTERVAL_MS)
                }
            }
        }, POLL_INTERVAL_MS)
    }

    private fun drainEvents() {
        while (true) {
            val ev = ClipRelayJni.pollEvent(engineHandle)
            if (ev == 0L) break
            try { handleEvent(ev) }
            finally { ClipRelayJni.freeEvent(ev) }
        }
    }

    private fun handleEvent(ev: Long) {
        when (ClipRelayJni.eventType(ev)) {

            ClipRelayJni.CR_EVENT_CLIPBOARD_TEXT -> {
                val text = ClipRelayJni.eventText(ev) ?: return
                val from = ClipRelayJni.eventDeviceName(ev) ?: "Unknown"
                applyText(text, from)
            }

            ClipRelayJni.CR_EVENT_CLIPBOARD_IMAGE -> {
                val bytes = ClipRelayJni.eventBinaryData(ev) ?: return
                val mime = ClipRelayJni.eventMimeType(ev) ?: "image/png"
                val from = ClipRelayJni.eventDeviceName(ev) ?: "Unknown"
                applyBinaryClipboard(bytes, suggestedName = imageNameForMime(mime), mimeType = mime, from = from)
            }

            ClipRelayJni.CR_EVENT_CLIPBOARD_FILE -> {
                val bytes = ClipRelayJni.eventBinaryData(ev) ?: return
                val name = ClipRelayJni.eventFileName(ev) ?: "ClipRelay File"
                val from = ClipRelayJni.eventDeviceName(ev) ?: "Unknown"
                // Files get a notification (important event)
                applyBinaryClipboard(bytes, suggestedName = name, mimeType = null, from = from, isFile = true)
            }

            ClipRelayJni.CR_EVENT_TOFU_PROMPT -> {
                val name = ClipRelayJni.eventDeviceName(ev) ?: "Unknown"
                val fp   = ClipRelayJni.eventFingerprint(ev) ?: ""
                showTofuNotification(name, fp)
            }

            ClipRelayJni.CR_EVENT_PEER_CONNECTED -> {
                val name = ClipRelayJni.eventDeviceName(ev) ?: "Unknown"
                Log.i(TAG, "Connected: $name")
                connectedPeerNames.add(name)
                persistStatus()
                updateForegroundNotification()
                // No notification — just update the persistent status bar entry
            }

            ClipRelayJni.CR_EVENT_PEER_DISCONNECTED -> {
                val name = ClipRelayJni.eventDeviceName(ev)
                Log.i(TAG, "Peer disconnected: ${name ?: "unknown"}")
                if (name != null) connectedPeerNames.remove(name) else connectedPeerNames.clear()
                persistStatus()
                updateForegroundNotification()
            }

            ClipRelayJni.CR_EVENT_WARNING -> {
                val msg = ClipRelayJni.eventText(ev) ?: return
                Log.w(TAG, "Engine warning: $msg")
                // Persistent connection loss shown as subtitle in foreground notification
                updateForegroundNotification()
            }
        }
    }

    // ── Clipboard watch (Android → Rust) ──────────────────────────────────────

    private fun scheduleClipboardWatch() {
        handler.postDelayed(object : Runnable {
            override fun run() {
                checkClipboard()
                handler.postDelayed(this, CLIP_INTERVAL_MS)
            }
        }, CLIP_INTERVAL_MS)
    }

    private fun checkClipboard() {
        if (engineHandle == 0L || !isSyncEnabled()) return
        if (suppressNext) { suppressNext = false; return }

        val clip = clipboardManager.primaryClip ?: return
        if (clip.itemCount == 0) return
        val item = clip.getItemAt(0)

        val text = item.text?.toString()?.trim()
        if (!text.isNullOrEmpty()) {
            val signature = "text:$text"
            if (signature != lastClipboardSignature) {
                lastClipboardSignature = signature
                ClipRelayJni.pushText(engineHandle, text)
            }
            return
        }

        val uri = item.uri ?: return
        val signature = "uri:$uri"
        if (signature == lastClipboardSignature) return

        when (val payload = readClipboardUri(uri)) {
            null -> Unit
            is OutgoingClipboardPayload.Image -> {
                lastClipboardSignature = signature
                ClipRelayJni.pushImage(engineHandle, payload.mimeType, payload.data)
            }
            is OutgoingClipboardPayload.File -> {
                lastClipboardSignature = signature
                ClipRelayJni.pushFile(engineHandle, payload.name, payload.data)
            }
        }
    }

    // ── Apply incoming clipboard ───────────────────────────────────────────────

    private fun applyText(text: String, from: String) {
        suppressNext = true
        lastClipboardSignature = "text:$text"
        val clip = android.content.ClipData.newPlainText("cliprelay", text)
        clipboardManager.setPrimaryClip(clip)

        // Silently update activity feed — NO notification
        addToActivityFeed(ActivityEntry(
            deviceName = from,
            kind = "text",
            preview = text.take(80)
        ))

        // Optional notification (OFF by default)
        if (isClipboardNotifyEnabled()) {
            showQuietClipboardNotification(from, "text")
        }
    }

    private fun applyBinaryClipboard(
        data: ByteArray,
        suggestedName: String,
        mimeType: String?,
        from: String,
        isFile: Boolean = false
    ) {
        // Save to Downloads/ClipRelay for files
        val saveDir = if (isFile) getDownloadsDir() else getCacheDir()
        val file = writeIncomingBinary(suggestedName, data, mimeType, saveDir)
        val authority = "$packageName.fileprovider"
        val uri = FileProvider.getUriForFile(this, authority, file)
        suppressNext = true
        lastClipboardSignature = "uri:$uri"
        clipboardManager.setPrimaryClip(android.content.ClipData.newUri(contentResolver, file.name, uri))

        val kind = if (mimeType?.startsWith("image/") == true) "image" else "file"
        addToActivityFeed(ActivityEntry(deviceName = from, kind = kind, preview = file.name))

        if (isFile) {
            // Files always get a notification — important event
            showFileReceivedNotification(from, file.name)
        } else if (isClipboardNotifyEnabled()) {
            showQuietClipboardNotification(from, kind)
        }
    }

    private fun getDownloadsDir(): File {
        val dir = File(
            android.os.Environment.getExternalStoragePublicDirectory(
                android.os.Environment.DIRECTORY_DOWNLOADS
            ),
            "ClipRelay"
        )
        dir.mkdirs()
        return dir
    }

    // ── Activity feed ─────────────────────────────────────────────────────────

    private fun addToActivityFeed(entry: ActivityEntry) {
        synchronized(activityFeed) {
            activityFeed.addFirst(entry)
            while (activityFeed.size > ACTIVITY_FEED_MAX) activityFeed.removeLast()
        }
        broadcastStatus()
    }

    // ── File helpers ──────────────────────────────────────────────────────────

    private fun readClipboardUri(uri: Uri): OutgoingClipboardPayload? {
        return runCatching {
            val mime = contentResolver.getType(uri).orEmpty()
            val name = queryDisplayName(uri) ?: uri.lastPathSegment ?: "ClipRelay File"
            val bytes = contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: return null
            if (mime.startsWith("image/")) {
                OutgoingClipboardPayload.Image(mimeType = mime.ifEmpty { "image/png" }, data = bytes)
            } else {
                OutgoingClipboardPayload.File(name = name, data = bytes)
            }
        }.onFailure { Log.w(TAG, "Failed to read clipboard URI $uri", it) }.getOrNull()
    }

    private fun writeIncomingBinary(suggestedName: String, data: ByteArray, mimeType: String?, baseDir: File): File {
        if (!baseDir.exists()) baseDir.mkdirs()
        val ext = mimeType
            ?.let { MimeTypeMap.getSingleton().getExtensionFromMimeType(it.substringBefore(';')) }
            ?.takeIf { it.isNotBlank() }
        val safeName = sanitizeFileName(suggestedName, ext)
        var target = File(baseDir, safeName)
        var counter = 2
        while (target.exists()) {
            val stem = target.nameWithoutExtension
            val suffix = target.extension.takeIf { it.isNotBlank() }?.let { ".$it" }.orEmpty()
            target = File(baseDir, "$stem-$counter$suffix")
            counter++
        }
        FileOutputStream(target).use { it.write(data) }
        return target
    }

    private fun sanitizeFileName(raw: String, fallbackExtension: String?): String {
        val cleaned = raw.trim().replace("/", "-").replace(":", "-")
        return if (cleaned.isNotEmpty()) cleaned
        else if (fallbackExtension.isNullOrBlank()) "cliprelay-item"
        else "cliprelay-item.$fallbackExtension"
    }

    private fun queryDisplayName(uri: Uri): String? {
        val cursor = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        cursor?.use {
            val nameIndex = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (nameIndex >= 0 && it.moveToFirst()) return it.getString(nameIndex)
        }
        return null
    }

    private fun imageNameForMime(mimeType: String): String {
        val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mimeType.substringBefore(';')) ?: "png"
        return "ClipRelay Image.$ext"
    }

    // ── Notifications ─────────────────────────────────────────────────────────

    private fun createNotificationChannels() {
        val nm = getSystemService(NotificationManager::class.java)

        // Channel 1: Persistent service notification — silent, minimal
        nm.createNotificationChannel(NotificationChannel(
            NOTIF_CHANNEL_SERVICE,
            "ClipRelay",
            NotificationManager.IMPORTANCE_MIN
        ).apply {
            description = "Keeps ClipRelay running in the background"
            setShowBadge(false)
        })

        // Channel 2: Important alerts — trust requests, files received, connection loss
        nm.createNotificationChannel(NotificationChannel(
            NOTIF_CHANNEL_ALERTS,
            "ClipRelay Alerts",
            NotificationManager.IMPORTANCE_DEFAULT
        ).apply {
            description = "Trust requests, received files, and connection alerts"
        })
    }

    private fun buildForegroundNotification(): Notification {
        val intent = packageManager.getLaunchIntentForPackage(packageName)
        val text = foregroundStatusText()
        val builder = NotificationCompat.Builder(this, NOTIF_CHANNEL_SERVICE)
            .setContentTitle("ClipRelay")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_MIN)
            .setSilent(true)

        if (intent != null) {
            val pi = PendingIntent.getActivity(
                this, 0, intent,
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
            )
            builder.setContentIntent(pi)
        }
        return builder.build()
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
        if (!isSyncEnabled()) return "Sync paused"
        return when {
            connectedPeerNames.isEmpty() -> "Ready — no devices nearby"
            connectedPeerNames.size == 1 -> "Connected to ${connectedPeerNames.first()}"
            else -> "${connectedPeerNames.size} devices connected"
        }
    }

    // Trust request — important, uses alerts channel
    private fun showTofuNotification(deviceName: String, fingerprint: String) {
        val nm = getSystemService(NotificationManager::class.java)
        val notif = NotificationCompat.Builder(this, NOTIF_CHANNEL_ALERTS)
            .setContentTitle("Trust \"$deviceName\"?")
            .setContentText("Fingerprint: $fingerprint")
            .setSmallIcon(android.R.drawable.ic_lock_lock)
            .setPriority(NotificationCompat.PRIORITY_HIGH)
            .setAutoCancel(true)
            .build()
        nm.notify(NOTIF_ID_TOFU, notif)
    }

    // File received — always notify, saves to Downloads/ClipRelay
    private fun showFileReceivedNotification(fromDevice: String, fileName: String) {
        val nm = getSystemService(NotificationManager::class.java)
        val notif = NotificationCompat.Builder(this, NOTIF_CHANNEL_ALERTS)
            .setContentTitle("📎 File received from $fromDevice")
            .setContentText(fileName)
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setAutoCancel(true)
            .build()
        nm.notify(NOTIF_ID_FILE, notif)
    }

    // Optional quiet clipboard notification (OFF by default)
    private fun showQuietClipboardNotification(fromDevice: String, kind: String) {
        // Updates the foreground notification text — no separate pop-up
        // Only shows if the user has explicitly enabled "notify on remote copy"
        updateForegroundNotification()
    }

    // ── Status persistence ────────────────────────────────────────────────────

    private fun persistStatus() {
        prefs().edit()
            .putString("local_device_name", resolvedDeviceName())
            .putBoolean("peer_connected", connectedPeerNames.isNotEmpty())
            .putStringSet("connected_names", connectedPeerNames.toSet())
            .apply()
        broadcastStatus()
    }

    private fun broadcastStatus() {
        sendBroadcast(Intent(ACTION_STATUS_CHANGED).setPackage(packageName))
    }

    private sealed interface OutgoingClipboardPayload {
        data class Image(val mimeType: String, val data: ByteArray) : OutgoingClipboardPayload
        data class File(val name: String, val data: ByteArray) : OutgoingClipboardPayload
    }
}
