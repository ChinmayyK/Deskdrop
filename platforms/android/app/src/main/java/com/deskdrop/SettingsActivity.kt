package com.deskdrop

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.widget.EditText
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.SystemBarStyle
import androidx.compose.runtime.mutableStateOf
import androidx.core.content.ContextCompat
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.deskdrop.ui.SettingsScreen
import com.deskdrop.ui.theme.AppTheme

class SettingsActivity : ComponentActivity() {

    private val deviceName = mutableStateOf("")
    private val deviceId = mutableStateOf("")
    private val syncEnabled = mutableStateOf(true)
    private val syncText = mutableStateOf(true)
    private val syncImages = mutableStateOf(true)
    private val syncFiles = mutableStateOf(true)
    private val callContinuityEnabled = mutableStateOf(false)
    private val isDarkMode = mutableStateOf(false)
    private val peers = mutableStateOf(emptyList<PeerSnapshot>())

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
        
        loadPreferences()

        setContent {
            AppTheme(useDarkTheme = isDarkMode.value) {
                SettingsScreen(
                    deviceName = deviceName.value,
                    deviceId = deviceId.value,
                    syncEnabled = syncEnabled.value,
                    syncText = syncText.value,
                    syncImages = syncImages.value,
                    syncFiles = syncFiles.value,
                    callContinuityEnabled = callContinuityEnabled.value,
                    isDarkMode = isDarkMode.value,
                    peers = peers.value,
                    onSyncEnabledChange = {
                        syncEnabled.value = it
                        saveBooleanPref("sync_enabled", it)
                    },
                    onSyncTextChange = {
                        syncText.value = it
                        saveBooleanPref("sync_text", it)
                    },
                    onSyncImagesChange = {
                        syncImages.value = it
                        saveBooleanPref("sync_images", it)
                    },
                    onSyncFilesChange = {
                        syncFiles.value = it
                        saveBooleanPref("sync_files", it)
                    },
                    onCallContinuityChange = {
                        callContinuityEnabled.value = it
                        saveBooleanPref("call_continuity_enabled", it)
                        if (it) {
                            requestCallContinuityPermissions()
                        }
                    },
                    onDarkModeChange = {
                        isDarkMode.value = it
                        saveBooleanPref("dark_mode", it)
                    },
                    onRenameClicked = { showRenameDialog() },
                    onBatterySettingsClicked = { openBatterySettings() },
                    onForgetDevice = { deviceId -> 
                        ContextCompat.startForegroundService(this,
                            Intent(this, DeskdropService::class.java).apply {
                                action = DeskdropService.ACTION_FORGET_PEER
                                putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, deviceId)
                            }
                        )
                        // optimistic UI update
                        peers.value = peers.value.filter { it.id != deviceId }
                        Toast.makeText(this, "Device forgotten", Toast.LENGTH_SHORT).show()
                    },
                    onBack = { finish() }
                )
            }
        }
    }

    private fun loadPreferences() {
        val prefs = getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE)
        deviceName.value = prefs.getString("device_name", null)?.trim()?.takeIf { it.isNotBlank() }
            ?: prefs.getString("local_device_name", null)?.trim()?.takeIf { it.isNotBlank() }
            ?: Build.MODEL
        deviceId.value = prefs.getString("device_id", "—") ?: "—"
        syncEnabled.value = prefs.getBoolean("sync_enabled", true)
        syncText.value = prefs.getBoolean("sync_text", true)
        syncImages.value = prefs.getBoolean("sync_images", true)
        syncFiles.value = prefs.getBoolean("sync_files", true)
        callContinuityEnabled.value = prefs.getBoolean("call_continuity_enabled", false)
        isDarkMode.value = prefs.getBoolean("dark_mode", false)
        peers.value = prefs.peerSnapshots()
    }

    private fun saveBooleanPref(key: String, value: Boolean) {
        getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE).edit().putBoolean(key, value).apply()
        sendBroadcast(Intent(DeskdropService.ACTION_SETTINGS_CHANGED).setPackage(packageName))
    }

    private fun showRenameDialog() {
        val field = EditText(this).apply {
            setText(deviceName.value)
            setSelection(text.length)
            hint = "My Phone"
            textSize = 15f
            val density = resources.displayMetrics.density
            val p = (16 * density).toInt()
            setPadding(p, p, p, p)
        }
        MaterialAlertDialogBuilder(this)
            .setTitle("Rename this device")
            .setMessage("This name appears on the network to other Deskdrop devices.")
            .setView(field)
            .setPositiveButton("Save") { _, _ ->
                val name = field.text?.toString()?.trim().orEmpty()
                if (name.isNotEmpty()) {
                    getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE)
                        .edit().putString("device_name", name).apply()
                    deviceName.value = name
                    restartService()
                    Toast.makeText(this, "Renamed to \"$name\"", Toast.LENGTH_SHORT).show()
                }
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun openBatterySettings() {
        runCatching {
            startActivity(Intent(
                android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                android.net.Uri.parse("package:$packageName")))
        }.onFailure {
            runCatching {
                startActivity(Intent(
                    android.provider.Settings.ACTION_BATTERY_SAVER_SETTINGS))
            }.onFailure {
                Toast.makeText(this,
                    "Open Settings -> Battery -> Deskdrop -> disable optimisation",
                    Toast.LENGTH_LONG).show()
            }
        }
    }

    private fun restartService() {
        stopService(Intent(this, DeskdropService::class.java))
        ContextCompat.startForegroundService(this,
            Intent(this, DeskdropService::class.java).apply {
                action = DeskdropService.ACTION_START 
            }
        )
    }

    private fun requestCallContinuityPermissions() {
        val needed = mutableListOf<String>()
        if (ContextCompat.checkSelfPermission(this, android.Manifest.permission.READ_PHONE_STATE) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += android.Manifest.permission.READ_PHONE_STATE
        }
        if (ContextCompat.checkSelfPermission(this, android.Manifest.permission.READ_CONTACTS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += android.Manifest.permission.READ_CONTACTS
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && ContextCompat.checkSelfPermission(this, android.Manifest.permission.ANSWER_PHONE_CALLS) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += android.Manifest.permission.ANSWER_PHONE_CALLS
        }
        if (ContextCompat.checkSelfPermission(this, android.Manifest.permission.READ_CALL_LOG) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += android.Manifest.permission.READ_CALL_LOG
        }
        if (needed.isNotEmpty()) {
            requestPermissions(needed.toTypedArray(), 1002)
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == 1002) {
            // Notify the service to try starting the call monitor again
            ContextCompat.startForegroundService(this, Intent(this, DeskdropService::class.java).apply {
                action = DeskdropService.ACTION_SETTINGS_CHANGED
            })
        }
    }
}
