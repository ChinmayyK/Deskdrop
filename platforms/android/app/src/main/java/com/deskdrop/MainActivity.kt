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
import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import com.deskdrop.ui.MainScreen
import com.deskdrop.ui.OnboardingScreen
import com.deskdrop.ui.theme.AppTheme
import com.deskdrop.ui.theme.CRTheme

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
    private val hasCompletedOnboarding = mutableStateOf(false)
    private val toastMessage = mutableStateOf("")

    private var targetDeviceIdForNextSend: String? = null

    private val filePickerLauncher = registerForActivityResult(androidx.activity.result.contract.ActivityResultContracts.GetMultipleContents()) { uris ->
        if (uris.isNotEmpty()) {
            val intent = Intent(this, DeskdropService::class.java).apply {
                action = DeskdropService.ACTION_PUSH_SHARED_URI
                putStringArrayListExtra(DeskdropService.EXTRA_SHARED_URIS, java.util.ArrayList(uris.map { it.toString() }))
                if (targetDeviceIdForNextSend != null) {
                    putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, targetDeviceIdForNextSend)
                }
            }
            ContextCompat.startForegroundService(this, intent)
            showSnack("Sending ${uris.size} file(s)...")
        }
        targetDeviceIdForNextSend = null
    }

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

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            val cm = getSystemService(android.content.ClipboardManager::class.java)
            val clip = cm.primaryClip?.getItemAt(0)?.coerceToText(this)?.toString()
            if (!clip.isNullOrBlank()) {
                DeskdropService.quickSendContextFlow.value = clip
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
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // API 30+: use the modern `display` property (defaultDisplay is deprecated)
            val displayModes = display?.supportedModes ?: emptyArray()
            val bestMode = displayModes.maxByOrNull { it.refreshRate }
            if (bestMode != null) {
                window.attributes = window.attributes.apply {
                    preferredDisplayModeId = bestMode.modeId
                }
            }
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            @Suppress("DEPRECATION")
            val displayModes = window.windowManager.defaultDisplay.supportedModes
            val bestRate = displayModes.maxByOrNull { it.refreshRate }?.refreshRate ?: 60f
            window.attributes = window.attributes.apply {
                preferredRefreshRate = bestRate
            }
        }
        super.onCreate(savedInstanceState)
        requestNotificationPermission()
        requestBatteryOptimizationExemption()

        setContent {
            val activeTransfers by DeskdropService.activeTransfersFlow.collectAsState()

            AppTheme(useDarkTheme = isDarkMode.value) {
                var showManualIpDialog by remember { mutableStateOf(false) }
                if (showManualIpDialog) {
                    var ipInput by remember { mutableStateOf("") }
                    androidx.compose.material3.AlertDialog(
                        onDismissRequest = { showManualIpDialog = false },
                        title = { Text("Enter Device IP") },
                        text = {
                            androidx.compose.material3.OutlinedTextField(
                                value = ipInput,
                                onValueChange = { ipInput = it },
                                label = { Text("e.g. 192.168.1.50") },
                                singleLine = true
                            )
                        },
                        confirmButton = {
                            androidx.compose.material3.TextButton(onClick = {
                                if (handlePairingInput(ipInput)) {
                                    showSnack("Connecting...")
                                } else {
                                    showSnack("Invalid IP format")
                                }
                                showManualIpDialog = false
                            }) { Text("Connect") }
                        },
                        dismissButton = {
                            androidx.compose.material3.TextButton(onClick = { showManualIpDialog = false }) { Text("Cancel") }
                        }
                    )
                }

                Box(modifier = Modifier.fillMaxSize()) {
                    if (!hasCompletedOnboarding.value) {
                        OnboardingScreen(
                            isDark = isDarkMode.value,
                            peers = peers.value,
                            onConnectPeer = { peer ->
                                ContextCompat.startForegroundService(this@MainActivity,
                                    Intent(this@MainActivity, DeskdropService::class.java).apply {
                                        action = DeskdropService.ACTION_SEND_PAIRING_REQUEST
                                        putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, peer.id)
                                    }
                                )
                            },
                            onSendSampleText = { peer ->
                                val svc = Intent(this@MainActivity, DeskdropService::class.java).apply {
                                    action = DeskdropService.ACTION_PUSH_CLIPBOARD
                                    putExtra(DeskdropService.EXTRA_CLIPBOARD_TEXT, "Hello from Android")
                                }
                                ContextCompat.startForegroundService(this@MainActivity, svc)
                                showSnack("Sample sent to ${peer.name}")
                            },
                            onScanQr = {
                                startQrScanner()
                            },
                            onManualIp = {
                                showManualIpDialog = true
                            },
                            onComplete = {
                                getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE).edit().putBoolean("has_completed_onboarding", true).apply()
                                hasCompletedOnboarding.value = true
                            }
                        )
                    } else {
                        MainScreen(
                        isDark = isDarkMode.value,
                        isServiceRunning = isServiceRunning.value,
                        isSyncEnabled = isSyncEnabled.value,
                        peers = peers.value,
                        feed = feed.value,
                        ambientStatus = ambientStatus.value,
                        activeTransfers = activeTransfers,
                        onStartSync = { launchService() },
                    onResumeSync = { sendAction(DeskdropService.ACTION_RESUME_SYNC) },
                    onScanNow = {
                        sendAction(DeskdropService.ACTION_SCAN_NOW)
                        showSnack("Scanning for devices...")
                    },
                    onActionPushClipboard = {
                        val cm = getSystemService(ClipboardManager::class.java)
                        val clip = cm.primaryClip?.getItemAt(0)?.coerceToText(this@MainActivity)
                        if (clip.isNullOrBlank()) {
                            showSnack("Clipboard is empty")
                        } else {
                            sendAction(DeskdropService.ACTION_PUSH_CLIPBOARD)
                            showSnack("Sending clipboard...")
                        }
                    },
                    onActionPairMagicLink = { showMagicLinkPairingDialog() },
                    onManualIp = { showManualIpDialog = true },
                    onActionPauseSync = {
                        sendAction(DeskdropService.ACTION_PAUSE_SYNC)
                        refreshDashboardState()
                    },
                    onActionDisconnectAll = {
                        sendAction(DeskdropService.ACTION_DISCONNECT_ALL)
                        refreshDashboardState()
                    },
                    onActionStopService = {
                        stopService(Intent(this@MainActivity, DeskdropService::class.java))
                        refreshDashboardState()
                    },
                    onApplyClipboard = { entry ->
                        val svc = Intent(this@MainActivity, DeskdropService::class.java).apply {
                            action = DeskdropService.ACTION_APPLY_CLIPBOARD
                            if (entry.contentHash.isNotBlank()) {
                                putExtra(DeskdropService.EXTRA_CONTENT_HASH, entry.contentHash)
                            }
                            putExtra(DeskdropService.EXTRA_CLIPBOARD_TEXT, entry.preview)
                        }
                        ContextCompat.startForegroundService(this@MainActivity, svc)
                        showSnack("Applied to clipboard")
                        rebuildFeed()
                    },
                    onTrustPeer = { peer ->
                        ContextCompat.startForegroundService(this@MainActivity,
                            Intent(this@MainActivity, DeskdropService::class.java).apply {
                                action = DeskdropService.ACTION_TRUST_PEER
                                putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, peer.id)
                            }
                        )
                        showSnack("Trusted ${peer.name}")
                        window.decorView.postDelayed({ refreshDashboardState() }, 200)
                    },
                    onRejectPeer = { peer ->
                        ContextCompat.startForegroundService(this@MainActivity,
                            Intent(this@MainActivity, DeskdropService::class.java).apply {
                                action = DeskdropService.ACTION_REJECT_PEER
                                putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, peer.id)
                            }
                        )
                        showSnack("Rejected ${peer.name}")
                        window.decorView.postDelayed({ refreshDashboardState() }, 200)
                    },
                    onSendPairingRequest = { peer ->
                        ContextCompat.startForegroundService(this@MainActivity,
                            Intent(this@MainActivity, DeskdropService::class.java).apply {
                                action = DeskdropService.ACTION_SEND_PAIRING_REQUEST
                                putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, peer.id)
                            }
                        )
                        showSnack("Pairing request sent to ${peer.name}")
                    },
                    onRespondPairing = { peer, accepted ->
                        ContextCompat.startForegroundService(this@MainActivity,
                            Intent(this@MainActivity, DeskdropService::class.java).apply {
                                action = DeskdropService.ACTION_RESPOND_TO_PAIRING
                                putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, peer.id)
                                putExtra(PairingActivity.EXTRA_APPROVED, accepted)
                            }
                        )
                        if (accepted) {
                            showSnack("Accepted pairing from ${peer.name}")
                        } else {
                            showSnack("Declined pairing from ${peer.name}")
                        }
                        window.decorView.postDelayed({ refreshDashboardState() }, 200)
                    },
                    onActionStreamCamera = {
                        startActivity(Intent(this@MainActivity, CameraStreamActivity::class.java))
                    },
                    onActionPauseTransfer = { tid ->
                        ContextCompat.startForegroundService(this@MainActivity, Intent(this@MainActivity, DeskdropService::class.java).apply {
                            action = DeskdropService.ACTION_PAUSE_FILE_TRANSFER
                            putExtra(DeskdropService.EXTRA_TRANSFER_ID, tid)
                        })
                    },
                    onActionResumeTransfer = { tid ->
                        ContextCompat.startForegroundService(this@MainActivity, Intent(this@MainActivity, DeskdropService::class.java).apply {
                            action = DeskdropService.ACTION_RESUME_FILE_TRANSFER
                            putExtra(DeskdropService.EXTRA_TRANSFER_ID, tid)
                        })
                    },
                    onActionCancelTransfer = { tid ->
                        ContextCompat.startForegroundService(this@MainActivity, Intent(this@MainActivity, DeskdropService::class.java).apply {
                            action = DeskdropService.ACTION_CANCEL_FILE_TRANSFER
                            putExtra(DeskdropService.EXTRA_TRANSFER_ID, tid)
                        })
                    },
                    onActionSendFiles = { targetId ->
                        targetDeviceIdForNextSend = targetId
                        filePickerLauncher.launch("*/*")
                    },
                    onForgetPeer = { peer ->
                        ContextCompat.startForegroundService(this@MainActivity,
                            Intent(this@MainActivity, DeskdropService::class.java).apply {
                                action = DeskdropService.ACTION_FORGET_PEER
                                putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, peer.id)
                            }
                        )
                        showSnack("Forgot ${peer.name}")
                        window.decorView.postDelayed({ refreshDashboardState() }, 200)
                    },
                    onOpenSettings = {
                        startActivity(Intent(this@MainActivity, SettingsActivity::class.java))
                    },
                    onOpenDiagnostics = {
                        startActivity(Intent(this@MainActivity, DiagnosticsActivity::class.java))
                    },
                    onReplayOnboarding = {
                        getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE).edit().putBoolean("has_completed_onboarding", false).apply()
                        hasCompletedOnboarding.value = false
                    }
                )
                }
                
                // Custom Toast Overlay
                AnimatedVisibility(
                    visible = toastMessage.value.isNotEmpty(),
                    enter = slideInVertically(initialOffsetY = { -it }) + fadeIn(),
                    exit = slideOutVertically(targetOffsetY = { -it }) + fadeOut(),
                    modifier = Modifier.align(Alignment.TopCenter).padding(top = 48.dp)
                ) {
                    CRToast(message = toastMessage.value, isDark = isDarkMode.value)
                }
                }
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

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        if (targetDeviceIdForNextSend != null) {
            outState.putString("targetDeviceIdForNextSend", targetDeviceIdForNextSend)
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
        hasCompletedOnboarding.value = prefs.getBoolean("has_completed_onboarding", false)
        
        val allPeers = prefs.peerSnapshots()
        peers.value = allPeers
        
        val isConnected = allPeers.any { it.isConnected }
        ambientStatus.value = if (isConnected) "Secure Connection  •  LAN Active" else "Looking for network..."
    }

    private fun rebuildFeed() {
        feed.value = DeskdropService.getFeedSnapshot()
    }

    private fun showSnack(message: String) {
        toastMessage.value = message
        // Auto-dismiss after 3 seconds
        feedRefreshHandler.postDelayed({
            if (toastMessage.value == message) {
                toastMessage.value = ""
            }
        }, 3000)
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

        if (needed.isNotEmpty()) {
            requestPermissions(needed.toTypedArray(), 1001)
        }
    }

    private fun requestBatteryOptimizationExemption() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val pm = getSystemService(POWER_SERVICE) as android.os.PowerManager
            if (!pm.isIgnoringBatteryOptimizations(packageName)) {
                runCatching {
                    startActivity(Intent(
                        android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                        android.net.Uri.parse("package:$packageName")
                    ))
                }
            }
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


}

@Composable
fun CRToast(message: String, isDark: Boolean) {
    Box(
        modifier = Modifier
            .padding(horizontal = 24.dp)
            .background(
                color = if (isDark) Color(0xFF1E1E1E).copy(alpha = 0.95f) else Color.White.copy(alpha = 0.95f),
                shape = RoundedCornerShape(100.dp)
            )
            .border(
                width = 0.5.dp,
                color = if (isDark) Color.White.copy(alpha = 0.1f) else Color.Black.copy(alpha = 0.05f),
                shape = RoundedCornerShape(100.dp)
            )
            .padding(horizontal = 16.dp, vertical = 10.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = message,
            color = CRTheme.textHigh(isDark),
            fontSize = 13.sp,
            fontWeight = FontWeight.Medium,
            letterSpacing = 0.5.sp
        )
    }
}
