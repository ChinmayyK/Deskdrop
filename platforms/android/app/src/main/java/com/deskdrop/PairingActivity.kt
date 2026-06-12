package com.deskdrop

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.SystemBarStyle
import com.deskdrop.ui.PairingScreen
import com.deskdrop.ui.theme.AppTheme

class PairingActivity : ComponentActivity() {

    companion object {
        const val EXTRA_DEVICE_ID       = "device_id"
        const val EXTRA_DEVICE_NAME     = "device_name"
        const val EXTRA_FINGERPRINT     = "fingerprint"
        const val EXTRA_PIN             = "pin"
        const val ACTION_PAIRING_RESULT   = "com.deskdrop.PAIRING_RESULT"
        const val ACTION_PAIRING_RESOLVED = "com.deskdrop.PAIRING_RESOLVED"
        const val EXTRA_APPROVED          = "approved"
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
        val deviceId    = intent.getStringExtra(EXTRA_DEVICE_ID)   ?: return finish()
        val deviceName  = intent.getStringExtra(EXTRA_DEVICE_NAME) ?: "Unknown device"
        val fingerprint = intent.getStringExtra(EXTRA_FINGERPRINT) ?: ""
        val pin         = intent.getStringExtra(EXTRA_PIN)         ?: "------"

        val prefs = getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE)
        val isDarkMode = prefs.getBoolean("dark_mode", false)

        setContent {
            AppTheme(useDarkTheme = isDarkMode) {
                PairingScreen(
                    isDark = isDarkMode,
                    deviceName = deviceName,
                    pin = pin,
                    fingerprint = fingerprint,
                    onApprove = { sendResult(deviceId, true) },
                    onDeny = { sendResult(deviceId, false) }
                )
            }
        }
    }

    private val receiver = object : android.content.BroadcastReceiver() {
        override fun onReceive(context: android.content.Context?, intent: Intent?) {
            if (intent?.action == ACTION_PAIRING_RESOLVED) {
                val targetId = intent.getStringExtra(EXTRA_DEVICE_ID)
                val currentId = this@PairingActivity.intent.getStringExtra(EXTRA_DEVICE_ID)
                if (targetId == null || targetId == currentId) {
                    finish()
                }
            }
        }
    }

    override fun onStart() {
        super.onStart()
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(receiver, android.content.IntentFilter(ACTION_PAIRING_RESOLVED), RECEIVER_NOT_EXPORTED)
        } else {
            registerReceiver(receiver, android.content.IntentFilter(ACTION_PAIRING_RESOLVED))
        }
    }

    override fun onStop() {
        super.onStop()
        unregisterReceiver(receiver)
    }

    private fun sendResult(deviceId: String, approved: Boolean) {
        sendBroadcast(Intent(ACTION_PAIRING_RESULT).apply {
            putExtra(EXTRA_DEVICE_ID, deviceId)
            putExtra(EXTRA_APPROVED, approved)
            setPackage(packageName)
        })
        finish()
    }
}
