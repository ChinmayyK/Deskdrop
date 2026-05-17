package com.cliprelay

import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.annotation.ColorRes
import androidx.core.content.ContextCompat
import kotlin.math.roundToInt

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
class ClipRelayShareTarget : Activity() {

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).roundToInt()
    private fun c(@ColorRes id: Int): Int = ContextCompat.getColor(this, id)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val sharedUris = when (intent?.action) {
            Intent.ACTION_SEND -> {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)?.let { arrayListOf(it) }
                } else {
                    @Suppress("DEPRECATION")
                    intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)?.let { arrayListOf(it) }
                }
            }
            Intent.ACTION_SEND_MULTIPLE -> {
                val items = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
                } else {
                    @Suppress("DEPRECATION")
                    intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
                }
                items
            }
            else -> null
        }
        val sharedText = when {
            sharedUris.isNullOrEmpty() &&
                intent?.action == Intent.ACTION_SEND &&
                intent.hasExtra(Intent.EXTRA_TEXT) ->
                intent.getStringExtra(Intent.EXTRA_TEXT)
            else -> null
        }
        val sharedName = intent?.getStringExtra(Intent.EXTRA_TITLE)?.takeIf { it.isNotBlank() }

        if (!sharedText.isNullOrBlank()) {
            runCatching {
                ContextCompat.startForegroundService(this, Intent(this, ClipRelayService::class.java).apply {
                    action = ClipRelayService.ACTION_PUSH_TEXT
                    putExtra("text", sharedText)
                })
            }
            Toast.makeText(this, "Pushed to ClipRelay peers", Toast.LENGTH_SHORT).show()
            finish()
        } else if (!sharedUris.isNullOrEmpty()) {
            setContentView(buildPicker(sharedUris, sharedName))
        } else {
            Toast.makeText(this, "Nothing to push", Toast.LENGTH_SHORT).show()
            finish()
        }
    }

    private fun buildPicker(sharedUris: List<Uri>, sharedName: String?): View {
        val peers = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
            .peerSnapshots()
            .filter { it.isConnected }

        val selectedDevice = arrayOf<String?>(null)
        val selectionCards = mutableListOf<Pair<String?, View>>()

        fun updateSelection(targetId: String?) {
            selectedDevice[0] = targetId
            selectionCards.forEach { (id, view) ->
                val selected = id == targetId
                view.background = cardBackground(selected)
            }
        }

        return ScrollView(this).apply {
            setBackgroundColor(c(R.color.cr_bg))
            addView(LinearLayout(this@ClipRelayShareTarget).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(dp(20), dp(24), dp(20), dp(24))

                addView(TextView(this@ClipRelayShareTarget).apply {
                    text = "Send with ClipRelay"
                    textSize = 24f
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(c(R.color.cr_text_1))
                })
                addView(space(6))
                addView(TextView(this@ClipRelayShareTarget).apply {
                    val noun = if (sharedUris.size == 1) "file" else "files"
                    text = "Choose all connected devices or one destination for ${sharedUris.size} $noun."
                    textSize = 14f
                    setTextColor(c(R.color.cr_text_3))
                    setLineSpacing(0f, 1.35f)
                })

                addView(space(18))

                if (peers.isEmpty()) {
                    addView(emptyState())
                } else {
                    val allCard = selectionCard(
                        title = "All connected devices",
                        subtitle = peers.joinToString(", ") { it.name },
                        avatarLabel = "ALL"
                    ) { updateSelection(null) }
                    selectionCards += null to allCard
                    addView(allCard)

                    peers.forEach { peer ->
                        addView(space(10))
                        val card = selectionCard(
                            title = peer.name,
                            subtitle = "Connected now",
                            avatarLabel = peer.name.firstOrNull()?.uppercase() ?: "?"
                        ) { updateSelection(peer.id) }
                        selectionCards += peer.id to card
                        addView(card)
                    }

                    updateSelection(null)
                }

                addView(space(22))
                addView(buttonRow(sharedUris, sharedName, peers, selectedDevice))
            })
        }
    }

    private fun buttonRow(
        sharedUris: List<Uri>,
        sharedName: String?,
        peers: List<PeerSnapshot>,
        selectedDevice: Array<String?>
    ): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL

        addView(actionButton("Cancel", filled = false) { finish() }, LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f
        ).also { it.marginEnd = dp(8) })

        addView(actionButton("Send", filled = true) {
            if (peers.isEmpty()) {
                startActivity(packageManager.getLaunchIntentForPackage(packageName))
                finish()
                return@actionButton
            }
            val svc = Intent(this@ClipRelayShareTarget, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_PUSH_SHARED_URI
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                putStringArrayListExtra(
                    ClipRelayService.EXTRA_SHARED_URIS,
                    ArrayList(sharedUris.map { it.toString() })
                )
                sharedName?.let { putExtra(ClipRelayService.EXTRA_SHARED_NAME, it) }
                selectedDevice[0]?.let { putExtra(ClipRelayService.EXTRA_TARGET_DEVICE_ID, it) }
            }
            runCatching { ContextCompat.startForegroundService(this@ClipRelayShareTarget, svc) }
            Toast.makeText(
                this@ClipRelayShareTarget,
                if (selectedDevice[0] == null) "Sharing to all connected devices"
                else "Sharing to selected device",
                Toast.LENGTH_SHORT
            ).show()
            finish()
        }, LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f
        ))
    }

    private fun actionButton(
        label: String,
        filled: Boolean,
        onClick: () -> Unit
    ): View = TextView(this).apply {
        text = label
        gravity = Gravity.CENTER
        textSize = 15f
        setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
        setTextColor(if (filled) c(R.color.cr_on_accent) else c(R.color.cr_text_2))
        setPadding(dp(16), dp(14), dp(16), dp(14))
        background = if (filled) {
            GradientDrawable().also {
                it.cornerRadius = dp(16).toFloat()
                it.setColor(c(R.color.cr_accent))
            }
        } else {
            GradientDrawable().also {
                it.cornerRadius = dp(16).toFloat()
                it.setColor(c(R.color.cr_bg_inset))
                it.setStroke(dp(1), c(R.color.cr_border))
            }
        }
        setOnClickListener { onClick() }
    }

    private fun emptyState(): View = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(dp(18), dp(18), dp(18), dp(18))
        background = GradientDrawable().also {
            it.cornerRadius = dp(18).toFloat()
            it.setColor(c(R.color.cr_bg_card))
            it.setStroke(dp(1), c(R.color.cr_border))
        }
        addView(TextView(this@ClipRelayShareTarget).apply {
            text = "No connected devices"
            textSize = 16f
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(c(R.color.cr_text_1))
        })
        addView(space(6))
        addView(TextView(this@ClipRelayShareTarget).apply {
            text = "Open ClipRelay on your Mac, or launch the app and scan again."
            textSize = 14f
            setTextColor(c(R.color.cr_text_3))
            setLineSpacing(0f, 1.35f)
        })
    }

    private fun selectionCard(
        title: String,
        subtitle: String,
        avatarLabel: String,
        onClick: () -> Unit
    ): View = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(dp(16), dp(16), dp(16), dp(16))
        background = cardBackground(false)
        isClickable = true
        isFocusable = true
        setOnClickListener { onClick() }

        addView(FrameLayout(this@ClipRelayShareTarget).apply {
            layoutParams = LinearLayout.LayoutParams(dp(40), dp(40)).also {
                it.marginEnd = dp(12)
            }
            background = GradientDrawable().also {
                it.shape = GradientDrawable.OVAL
                it.setColor(c(R.color.cr_accent_bg))
            }
            addView(TextView(this@ClipRelayShareTarget).apply {
                text = avatarLabel
                gravity = Gravity.CENTER
                textSize = if (avatarLabel.length > 1) 11f else 18f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(c(R.color.cr_accent))
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )
            })
        })

        addView(LinearLayout(this@ClipRelayShareTarget).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            addView(TextView(this@ClipRelayShareTarget).apply {
                text = title
                textSize = 15f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(c(R.color.cr_text_1))
            })
            addView(space(3))
            addView(TextView(this@ClipRelayShareTarget).apply {
                text = subtitle
                textSize = 13f
                setTextColor(c(R.color.cr_text_3))
            })
        })
    }

    private fun cardBackground(selected: Boolean): RippleDrawable {
        val base = GradientDrawable().also {
            it.cornerRadius = dp(18).toFloat()
            it.setColor(if (selected) c(R.color.cr_accent_bg) else c(R.color.cr_bg_card))
            it.setStroke(
                dp(1),
                if (selected) c(R.color.cr_accent) else c(R.color.cr_border)
            )
        }
        return RippleDrawable(
            android.content.res.ColorStateList.valueOf(c(R.color.cr_ripple)),
            base,
            null
        )
    }

    private fun space(height: Int): View = View(this).apply {
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            dp(height)
        )
    }
}
