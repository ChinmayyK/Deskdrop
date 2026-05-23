package com.deskdrop

import android.Manifest
import android.content.*
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.SystemBarStyle
import androidx.compose.runtime.mutableStateOf
import androidx.core.content.ContextCompat
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import com.deskdrop.ui.MainScreen
import com.deskdrop.ui.theme.AppTheme

class MainActivity : ComponentActivity() {

    companion object {
        private const val FEED_REFRESH_MS = 5_000L
    }

    private val isServiceRunning = mutableStateOf(false)
    private val isSyncEnabled = mutableStateOf(true)
    private val peers = mutableStateOf<List<PeerSnapshot>>(emptyList())
    private val feed = mutableStateOf<List<ActivityEntry>>(emptyList())
    private val ambientStatus = mutableStateOf("Looking for network...")
    private val isDarkMode = mutableStateOf(false)

    private val feedRefreshHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private val feedRefreshRunnable = object : Runnable {
        override fun run() {
            rebuildFeed()
            feedRefreshHandler.postDelayed(this, FEED_REFRESH_MS)
        }
    }

    private val statusReceiver = object : BroadcastReceiver() {
        override fun onReceive(ctx: Context?, intent: Intent?) {
            runOnUiThread {
                refreshDashboardState()
                rebuildFeed()
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.auto(
                android.graphics.Color.TRANSPARENT,
                android.graphics.Color.TRANSPARENT
            ),
            navigationBarStyle = SystemBarStyle.auto(
                android.graphics.Color.TRANSPARENT,
                android.graphics.Color.TRANSPARENT
            )
        )
        super.onCreate(savedInstanceState)
        requestNotificationPermission()

        setContent {
            AppTheme(useDarkTheme = isDarkMode.value) {
                MainScreen(
                    isDark = isDarkMode.value,
                    isServiceRunning = isServiceRunning.value,
                    isSyncEnabled = isSyncEnabled.value,
                    peers = peers.value,
                    feed = feed.value,
                    ambientStatus = ambientStatus.value,
                    onStartSync = { launchService() },
                    onResumeSync = { sendAction(DeskdropService.ACTION_RESUME_SYNC) },
                    onScanNow = {
                        sendAction(DeskdropService.ACTION_SCAN_NOW)
                        showSnack("Scanning for devices...")
                    },
                    onActionPushClipboard = {
                        val cm = getSystemService(ClipboardManager::class.java)
                        val clip = cm.primaryClip?.getItemAt(0)?.coerceToText(this)
                        if (clip.isNullOrBlank()) {
                            showSnack("Clipboard is empty")
                        } else {
                            sendAction(DeskdropService.ACTION_PUSH_CLIPBOARD)
                            showSnack("Sending clipboard...")
                        }
                    },
                    onActionPairMagicLink = { showMagicLinkPairingDialog() },
                    onActionPauseSync = {
                        sendAction(DeskdropService.ACTION_PAUSE_SYNC)
                        refreshDashboardState()
                    },
                    onActionDisconnectAll = {
                        sendAction(DeskdropService.ACTION_DISCONNECT_ALL)
                        refreshDashboardState()
                    },
                    onActionStopService = {
                        stopService(Intent(this, DeskdropService::class.java))
                        refreshDashboardState()
                    },
                    onApplyClipboard = { entry ->
                        val svc = Intent(this, DeskdropService::class.java).apply {
                            action = DeskdropService.ACTION_APPLY_CLIPBOARD
                            if (entry.contentHash.isNotBlank()) {
                                putExtra(DeskdropService.EXTRA_CONTENT_HASH, entry.contentHash)
                            }
                            putExtra(DeskdropService.EXTRA_CLIPBOARD_TEXT, entry.preview)
                        }
                        ContextCompat.startForegroundService(this, svc)
                        showSnack("Applied to clipboard")
                        rebuildFeed()
                    },
                    onTrustPeer = { peer ->
                        ContextCompat.startForegroundService(this,
                            Intent(this, DeskdropService::class.java).apply {
                                action = DeskdropService.ACTION_TRUST_PEER
                                putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, peer.id)
                            }
                        )
                        showSnack("Trusted ${peer.name}")
                        window.decorView.postDelayed({ refreshDashboardState() }, 200)
                    },
                    onRejectPeer = { peer ->
                        ContextCompat.startForegroundService(this,
                            Intent(this, DeskdropService::class.java).apply {
                                action = DeskdropService.ACTION_REJECT_PEER
                                putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, peer.id)
                            }
                        )
                        showSnack("Rejected ${peer.name}")
                    },
                    onOpenSettings = {
                        startActivity(Intent(this, SettingsActivity::class.java))
                    }
                )
            }
        }
        
        launchService()
        refreshDashboardState()
    }

    override fun onResume() {
        super.onResume()
        refreshDashboardState()
        rebuildFeed()
        try {
            startService(Intent(this, DeskdropService::class.java))
        } catch (e: Exception) {
            android.util.Log.e("MainActivity", "Failed to refresh service onResume", e)
        }
    }

    override fun onStart() {
        super.onStart()
        ContextCompat.registerReceiver(
            this, statusReceiver,
            IntentFilter(DeskdropService.ACTION_STATUS_CHANGED),
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
        feedRefreshHandler.post(feedRefreshRunnable)
    }

    override fun onStop() {
        unregisterReceiver(statusReceiver)
        feedRefreshHandler.removeCallbacks(feedRefreshRunnable)
        super.onStop()
    }

    private fun refreshDashboardState() {
        val prefs = getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE)
        isServiceRunning.value = prefs.getBoolean(DeskdropService.PREF_SERVICE_RUNNING, false)
        isSyncEnabled.value = prefs.getBoolean("sync_enabled", true)
        isDarkMode.value = prefs.getBoolean("dark_mode", false)
        
        val allPeers = prefs.peerSnapshots()
        peers.value = allPeers
        
        val isConnected = allPeers.any { it.isConnected }
        ambientStatus.value = if (isConnected) "Secure Connection  •  LAN Active" else "Looking for network..."
    }

    private fun rebuildFeed() {
        feed.value = DeskdropService.getFeedSnapshot()
    }

    private fun showSnack(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
    }

    private fun launchService() = runCatching {
        ContextCompat.startForegroundService(this,
            Intent(this, DeskdropService::class.java).apply {
                action = DeskdropService.ACTION_START 
            }
        )
    }

    private fun sendAction(action: String) = runCatching {
        ContextCompat.startForegroundService(this,
            Intent(this, DeskdropService::class.java).apply { this.action = action })
    }

    private fun requestNotificationPermission() {
        val needed = mutableListOf<String>()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.POST_NOTIFICATIONS
        }

        if (checkSelfPermission(Manifest.permission.READ_PHONE_STATE) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.READ_PHONE_STATE
        }

        if (checkSelfPermission(Manifest.permission.READ_CONTACTS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.READ_CONTACTS
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            checkSelfPermission(Manifest.permission.ANSWER_PHONE_CALLS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.ANSWER_PHONE_CALLS
        }

        if (checkSelfPermission(Manifest.permission.READ_CALL_LOG) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.READ_CALL_LOG
        }

        if (needed.isNotEmpty()) {
            requestPermissions(needed.toTypedArray(), 1001)
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == 1001) {
            val readPhone = permissions.indexOf(Manifest.permission.READ_PHONE_STATE)
            if (readPhone >= 0 &&
                grantResults[readPhone] == android.content.pm.PackageManager.PERMISSION_GRANTED) {
                startService(Intent(this, DeskdropService::class.java))
            }
        }
    }

    private fun showMagicLinkPairingDialog() {
        // Fallback to directly starting QR scanner since we dropped the programmatic UI dialog
        startQrScanner()
    }

    private fun startQrScanner() {
        val options = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .enableAutoZoom()
            .build()
        val scanner = GmsBarcodeScanning.getClient(this, options)
        scanner.startScan()
            .addOnSuccessListener { barcode: Barcode ->
                val rawValue = barcode.rawValue
                if (!rawValue.isNullOrBlank()) {
                    if (handlePairingInput(rawValue)) {
                        showSnack("QR scanned successfully! Connecting...")
                    } else {
                        showSnack("Invalid QR code format")
                    }
                } else {
                    showSnack("No QR code found")
                }
            }
            .addOnFailureListener { e: java.lang.Exception ->
                showSnack("QR Scan failed: ${e.message}")
            }
    }

    private fun handlePairingInput(input: String): Boolean {
        val cleaned = input.trim()
        val ctx = this
        if (cleaned.startsWith("deskdrop://pair") || cleaned.startsWith("deskdrop://pair")) {
            val uri = android.net.Uri.parse(cleaned)
            val ip = uri.getQueryParameter("ip")
            val port = uri.getQueryParameter("port")?.toIntOrNull() ?: 47823
            val peerName = uri.getQueryParameter("name")?.let {
                java.net.URLDecoder.decode(it, "UTF-8")
            } ?: ip ?: "Mac"
            val fingerprint = uri.getQueryParameter("fingerprint") ?: ""
            if (ip != null) {
                ContextCompat.startForegroundService(ctx,
                    Intent(ctx, DeskdropService::class.java).apply {
                        action = DeskdropService.ACTION_CONNECT_MANUAL
                        putExtra("ip", ip)
                        putExtra("port", port)
                    }
                )
                if (fingerprint.isNotBlank()) {
                    android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                        autoTrustNewPeer(ip, peerName)
                    }, 2000)
                    android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                        autoTrustNewPeer(ip, peerName)
                    }, 4000)
                }
                showSnack("Connecting to $peerName ($ip)...")
                return true
            }
        } else {
            val parts = cleaned.split(":")
            val ip = parts[0].trim()
            val port = if (parts.size > 1) parts[1].trim().toIntOrNull() ?: 47823 else 47823
            if (ip.matches(Regex("""\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}"""))) {
                ContextCompat.startForegroundService(ctx,
                    Intent(ctx, DeskdropService::class.java).apply {
                        action = DeskdropService.ACTION_CONNECT_MANUAL
                        putExtra("ip", ip)
                        putExtra("port", port)
                    }
                )
                showSnack("Connecting to $ip:$port...")
                return true
            }
        }
        return false
    }

    private fun autoTrustNewPeer(ip: String, peerName: String) {
        val prefs = getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE)
        val peers = prefs.peerSnapshots()
        val untrustedConnected = peers.firstOrNull { it.isConnected && !it.trusted }
        if (untrustedConnected != null) {
            ContextCompat.startForegroundService(this,
                Intent(this, DeskdropService::class.java).apply {
                    action = DeskdropService.ACTION_TRUST_PEER
                    putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, untrustedConnected.id)
                }
            )
            showSnack("Paired with $peerName — syncing!")
            refreshDashboardState()
        }
    }
}
