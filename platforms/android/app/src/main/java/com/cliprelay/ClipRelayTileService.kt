package com.cliprelay

import android.content.Intent
import android.graphics.drawable.Icon
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.widget.Toast

/**
 * Quick Settings tile — shown in the Android notification shade.
 *
 * Allows the user to toggle ClipRelay clipboard syncing on/off
 * without opening the app. Long-press opens ClipRelay settings.
 *
 * Manifest registration:
 *
 * <service
 *   android:name=".ClipRelayTileService"
 *   android:icon="@drawable/ic_tile_cliprelay"
 *   android:label="@string/app_name"
 *   android:permission="android.permission.BIND_QUICK_SETTINGS_TILE"
 *   android:exported="true">
 *   <intent-filter>
 *     <action android:name="android.service.quicksettings.action.QS_TILE"/>
 *   </intent-filter>
 *   <meta-data
 *     android:name="android.service.quicksettings.ACTIVE_TILE"
 *     android:value="true"/>
 * </service>
 */
class ClipRelayTileService : TileService() {

    // ── TileService lifecycle ──────────────────────────────────────────────────

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

    // ── Tile state ────────────────────────────────────────────────────────────

    private fun isSyncEnabled(): Boolean {
        // Read from shared preferences; daemon state is authoritative,
        // but for the quick tile we use the last-known settings value.
        return getSharedPreferences("cliprelay", MODE_PRIVATE)
            .getBoolean("sync_enabled", true)
    }

    private fun setSyncEnabled(enabled: Boolean) {
        getSharedPreferences("cliprelay", MODE_PRIVATE)
            .edit()
            .putBoolean("sync_enabled", enabled)
            .apply()

        // Notify the running service.
        val intent = Intent(this, ClipRelayService::class.java).apply {
            action = if (enabled) ACTION_SYNC_ENABLE else ACTION_SYNC_DISABLE
        }
        startService(intent)
    }

    private fun refreshTile() {
        val tile = qsTile ?: return
        val enabled = isSyncEnabled()

        tile.state = if (enabled) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = if (enabled) "ClipRelay: On" else "ClipRelay: Off"
        tile.contentDescription = if (enabled)
            "ClipRelay clipboard sync is active"
        else
            "ClipRelay clipboard sync is paused"

        // Android 13+ supports subtitle
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            tile.subtitle = if (enabled) "Syncing" else "Paused"
        }

        tile.updateTile()
    }

    private fun toggleSync() {
        val newState = !isSyncEnabled()
        setSyncEnabled(newState)
        refreshTile()

        val msg = if (newState) "ClipRelay sync enabled" else "ClipRelay sync paused"
        Toast.makeText(applicationContext, msg, Toast.LENGTH_SHORT).show()
    }

    companion object {
        const val ACTION_SYNC_ENABLE  = "com.cliprelay.SYNC_ENABLE"
        const val ACTION_SYNC_DISABLE = "com.cliprelay.SYNC_DISABLE"
    }
}

/**
 * Clipboard history Quick-Share target (Android 13+).
 *
 * Appears in the share sheet and clipboard toolbar, letting the user
 * push any shared text directly to ClipRelay peers.
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
            pushTextViaDaemon(text)
            Toast.makeText(this, "📋 Pushed to ClipRelay peers", Toast.LENGTH_SHORT).show()
        } else {
            Toast.makeText(this, "No text to push", Toast.LENGTH_SHORT).show()
        }

        finish()
    }

    private fun pushTextViaDaemon(text: String) {
        // Delegate to the service which holds the engine handle.
        val intent = Intent(this, ClipRelayService::class.java).apply {
            action  = "com.cliprelay.PUSH_TEXT"
            putExtra("text", text)
        }
        startService(intent)
    }
}
