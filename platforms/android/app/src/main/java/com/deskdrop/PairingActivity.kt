package com.deskdrop

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.SystemBarStyle
import com.deskdrop.ui.PairingScreen
import com.deskdrop.ui.theme.AppTheme
import android.content.BroadcastReceiver
import android.content.Context
import android.content.IntentFilter

class PairingActivity : ComponentActivity() {

    companion object {
        const val EXTRA_DEVICE_ID       = "device_id"
        const val EXTRA_DEVICE_NAME     = "device_name"
        const val EXTRA_FINGERPRINT     = "fingerprint"
        const val EXTRA_PIN             = "pin"
        const val EXTRA_IS_INITIATOR    = "is_initiator"
        const val ACTION_PAIRING_RESULT = "com.deskdrop.PAIRING_RESULT"
        const val EXTRA_APPROVED        = "approved"
    }

    private var targetDeviceId: String? = null

    private val statusReceiver = object : BroadcastReceiver() {
        override fun onReceive(ctx: Context?, intent: Intent?) {
            if (intent?.action == "com.deskdrop.CLOSE_PAIRING_UI") {
                finish()
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
        val deviceId    = intent.getStringExtra(EXTRA_DEVICE_ID)   ?: return finish()
        targetDeviceId = deviceId

        val filter = IntentFilter().apply {
            addAction(DeskdropService.ACTION_STATUS_CHANGED)
            addAction("com.deskdrop.CLOSE_PAIRING_UI")
        }

        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(statusReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            registerReceiver(statusReceiver, filter)
        }
        val deviceName  = intent.getStringExtra(EXTRA_DEVICE_NAME) ?: "Unknown device"
        val fingerprint = intent.getStringExtra(EXTRA_FINGERPRINT) ?: ""
        val pin         = intent.getStringExtra(EXTRA_PIN)         ?: "------"
        val isInitiator = intent.getBooleanExtra(EXTRA_IS_INITIATOR, false)

        val prefs = getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE)
        val isDarkMode = prefs.getBoolean("dark_mode", false)

        setContent {
            AppTheme(useDarkTheme = isDarkMode) {
                androidx.activity.compose.BackHandler {
                    sendResult(deviceId, false)
                }
                PairingScreen(
                    isDark = isDarkMode,
                    deviceName = deviceName,
                    pin = pin,
                    fingerprint = fingerprint,
                    isInitiator = isInitiator,
                    onApprove = { sendResult(deviceId, true) },
                    onDeny = { sendResult(deviceId, false) }
                )
            }
        }
    }

    private fun sendResult(deviceId: String, approved: Boolean) {
        val intent = Intent(ACTION_PAIRING_RESULT).apply {
            setPackage(packageName)
            putExtra(EXTRA_DEVICE_ID, deviceId)
            putExtra(EXTRA_APPROVED, approved)
        }
        sendBroadcast(intent)
        finish()
    }

    override fun onDestroy() {
        super.onDestroy()
        try {
            unregisterReceiver(statusReceiver)
        } catch (e: Exception) {
            // Ignore if not registered
        }
    }
}
