package com.cliprelay

import android.content.Intent
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.widget.Toast
import androidx.core.content.ContextCompat

/**
 * Quick Settings tile — lets users toggle clipboard sync from the notification shade.
 *
 * Shows:
 *   - Active state: "ClipRelay · Syncing"
 *   - Inactive state: "ClipRelay · Paused"
 *
 * Long-press opens ClipRelay settings.
 * Does NOT show clipboard content or peer data in the tile.
 */
class ClipRelayTileService : TileService() {

    companion object {
        const val ACTION_SYNC_ENABLE  = "com.cliprelay.SYNC_ENABLE"
        const val ACTION_SYNC_DISABLE = "com.cliprelay.SYNC_DISABLE"
    }

    override fun onStartListening() {
        super.onStartListening()
        refreshTile()
    }

    override fun onStopListening() {
        super.onStopListening()
    }

    override fun onClick() {
        super.onClick()
        toggleSync()
    }

    private fun isSyncEnabled(): Boolean =
        getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
            .getBoolean("sync_enabled", true)

    private fun setSyncEnabled(enabled: Boolean) {
        getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
            .edit()
            .putBoolean("sync_enabled", enabled)
            .apply()

        val intent = Intent(this, ClipRelayService::class.java).apply {
            action = if (enabled) ACTION_SYNC_ENABLE else ACTION_SYNC_DISABLE
        }

        runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                ContextCompat.startForegroundService(this, intent)
            } else {
                startService(intent)
            }
        }
    }

    private fun refreshTile() {
        val tile = qsTile ?: return
        val enabled = isSyncEnabled()
        val prefs   = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
        val count   = prefs.getInt("connected_count", 0)

        tile.state = if (enabled) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = "ClipRelay"

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            tile.subtitle = when {
                !enabled     -> "Paused"
                count == 0   -> "No devices"
                count == 1   -> "1 device"
                else         -> "$count devices"
            }
        }

        tile.contentDescription = if (enabled) "ClipRelay clipboard sync is active" else "ClipRelay clipboard sync is paused"
        tile.updateTile()
    }

    private fun toggleSync() {
        val newState = !isSyncEnabled()
        setSyncEnabled(newState)
        refreshTile()
        Toast.makeText(
            applicationContext,
            if (newState) "ClipRelay sync enabled" else "ClipRelay sync paused",
            Toast.LENGTH_SHORT
        ).show()
    }
}

/**
 * Share target — appears in Android's share sheet, letting users push
 * any shared text directly to ClipRelay peers without opening the app.
 */
class ClipRelayShareTarget : android.app.Activity() {

    override fun onCreate(savedInstanceState: android.os.Bundle?) {
        super.onCreate(savedInstanceState)

        val text = when {
            intent?.action == Intent.ACTION_SEND &&
            intent.type?.startsWith("text/") == true ->
                intent.getStringExtra(Intent.EXTRA_TEXT)
            else -> null
        }

        if (text != null) {
            runCatching {
                startService(Intent(this, ClipRelayService::class.java).apply {
                    action = ClipRelayService.ACTION_PUSH_TEXT
                    putExtra("text", text)
                })
            }
            Toast.makeText(this, "Pushed to ClipRelay peers", Toast.LENGTH_SHORT).show()
        } else {
            Toast.makeText(this, "Nothing to push", Toast.LENGTH_SHORT).show()
        }

        finish()
    }
}
