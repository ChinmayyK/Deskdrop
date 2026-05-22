package com.cliprelay

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
import com.cliprelay.ui.SettingsScreen
import com.cliprelay.ui.theme.AppTheme

class SettingsActivity : ComponentActivity() {

    private val deviceName = mutableStateOf("")
    private val deviceId = mutableStateOf("")
    private val syncEnabled = mutableStateOf(true)
    private val syncText = mutableStateOf(true)
    private val syncImages = mutableStateOf(true)
    private val syncFiles = mutableStateOf(true)

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
            AppTheme {
                SettingsScreen(
                    deviceName = deviceName.value,
                    deviceId = deviceId.value,
                    syncEnabled = syncEnabled.value,
                    syncText = syncText.value,
                    syncImages = syncImages.value,
                    syncFiles = syncFiles.value,
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
                    onRenameClicked = { showRenameDialog() },
                    onBatterySettingsClicked = { openBatterySettings() },
                    onBack = { finish() }
                )
            }
        }
    }

    private fun loadPreferences() {
        val prefs = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
        deviceName.value = prefs.getString("device_name", null)?.trim()?.takeIf { it.isNotBlank() }
            ?: prefs.getString("local_device_name", null)?.trim()?.takeIf { it.isNotBlank() }
            ?: Build.MODEL
        deviceId.value = prefs.getString("device_id", "—") ?: "—"
        syncEnabled.value = prefs.getBoolean("sync_enabled", true)
        syncText.value = prefs.getBoolean("sync_text", true)
        syncImages.value = prefs.getBoolean("sync_images", true)
        syncFiles.value = prefs.getBoolean("sync_files", true)
    }

    private fun saveBooleanPref(key: String, value: Boolean) {
        getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE).edit().putBoolean(key, value).apply()
        sendBroadcast(Intent(ClipRelayService.ACTION_SETTINGS_CHANGED).setPackage(packageName))
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
                    getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
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
        stopService(Intent(this, ClipRelayService::class.java))
        ContextCompat.startForegroundService(this,
            Intent(this, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_START 
            }
        )
    }
}
