package com.cliprelay

import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.annotation.ColorRes
import androidx.core.content.ContextCompat
import kotlin.math.roundToInt
import android.webkit.MimeTypeMap
import android.provider.OpenableColumns
import android.widget.ProgressBar
import android.content.res.ColorStateList
import android.util.Log

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
class ClipRelayShareTarget : androidx.appcompat.app.AppCompatActivity() {

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).roundToInt()
    private fun c(@ColorRes id: Int): Int = ContextCompat.getColor(this, id)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))

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
            val dialog = com.google.android.material.bottomsheet.BottomSheetDialog(this, R.style.Theme_ClipRelay_Dialog)
            val view = buildPicker(sharedUris, sharedName, dialog)
            dialog.setContentView(view)
            dialog.setOnDismissListener { finish() }
            dialog.show()
        } else {
            Toast.makeText(this, "Nothing to push", Toast.LENGTH_SHORT).show()
            finish()
        }
    }

    private fun buildPicker(sharedUris: List<Uri>, sharedName: String?, dialog: android.app.Dialog): View {
        val peers = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
            .peerSnapshots()
            .filter { it.isConnected }

        val selectedDevice = arrayOf<String?>(null)
        val selectionCards = mutableListOf<Pair<String?, View>>()

        // Standard colors for premium look
        val brandColor = c(R.color.cr_accent)
        val brandColorLight = c(R.color.cr_accent_bg)
        val textColor1 = c(R.color.cr_text_1)
        val textColor3 = c(R.color.cr_text_3)
        val borderColor = c(R.color.cr_border)

        fun updateSelection(targetId: String?) {
            selectedDevice[0] = targetId
            selectionCards.forEach { (id, view) ->
                val selected = id == targetId
                view.background = cardBackground(selected)
                view.findViewWithTag<View>("selected_indicator")?.background =
                    GradientDrawable().also {
                        it.shape = GradientDrawable.OVAL
                        it.setColor(if (selected) brandColor else Color.TRANSPARENT)
                        it.setStroke(dp(1.5f.toInt()), if (selected) brandColor else borderColor)
                    }
            }
        }

        val container = FrameLayout(this).apply {
            setBackgroundColor(Color.TRANSPARENT)
        }

        val mainLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(24), dp(16), dp(24), dp(24))

            // Beautiful Top Pill Handle
            addView(View(this@ClipRelayShareTarget).apply {
                background = GradientDrawable().also {
                    it.cornerRadius = dp(4).toFloat()
                    it.setColor(borderColor)
                }
                layoutParams = LinearLayout.LayoutParams(dp(36), dp(5)).also {
                    it.gravity = Gravity.CENTER_HORIZONTAL
                    it.bottomMargin = dp(24)
                }
            })

            // Header Layout (Logo + Title)
            addView(LinearLayout(this@ClipRelayShareTarget).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL

                // Animated glowing ClipRelay logo icon
                addView(FrameLayout(this@ClipRelayShareTarget).apply {
                    layoutParams = LinearLayout.LayoutParams(dp(44), dp(44)).also {
                        it.marginEnd = dp(14)
                    }
                    background = GradientDrawable().also {
                        it.shape = GradientDrawable.OVAL
                        it.colors = intArrayOf(brandColor, c(R.color.cr_accent_dim))
                        it.orientation = GradientDrawable.Orientation.TL_BR
                    }
                    addView(TextView(this@ClipRelayShareTarget).apply {
                        text = "⚡"
                        textSize = 20f
                        gravity = Gravity.CENTER
                    })
                })

                addView(LinearLayout(this@ClipRelayShareTarget).apply {
                    orientation = LinearLayout.VERTICAL
                    addView(TextView(this@ClipRelayShareTarget).apply {
                        text = "Send with ClipRelay"
                        textSize = 20f
                        setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                        setTextColor(textColor1)
                    })
                    addView(TextView(this@ClipRelayShareTarget).apply {
                        val noun = if (sharedUris.size == 1) "file" else "files"
                        text = "Staging ${sharedUris.size} $noun to send"
                        textSize = 13f
                        setTextColor(textColor3)
                    })
                })
            })

            addView(space(20))

            // Sub-header indicator
            addView(TextView(this@ClipRelayShareTarget).apply {
                text = if (peers.isEmpty()) "NO PEERS AVAILABLE" else "CONNECTED PEERS (${peers.size})"
                textSize = 11f
                letterSpacing = 0.08f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.BOLD))
                setTextColor(brandColor)
            })

            addView(space(10))

            // Scrollable Peer List
            addView(ScrollView(this@ClipRelayShareTarget).apply {
                overScrollMode = View.OVER_SCROLL_NEVER
                isVerticalScrollBarEnabled = false
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    dp(220)
                )

                addView(LinearLayout(this@ClipRelayShareTarget).apply {
                    orientation = LinearLayout.VERTICAL
                    if (peers.isEmpty()) {
                        addView(emptyState())
                    } else {
                        // All Connected option
                        val allCard = selectionCard(
                            title = "All connected devices",
                            subtitle = peers.joinToString(", ") { it.name },
                            avatarLabel = "ALL",
                            gradientColors = intArrayOf(brandColor, c(R.color.cr_accent_dim))
                        ) { updateSelection(null) }
                        selectionCards += null to allCard
                        addView(allCard)

                        peers.forEach { peer ->
                            addView(space(10))
                            val isMac = peer.name.contains("mac", ignoreCase = true) || peer.name.contains("book", ignoreCase = true)
                            val isAndroid = peer.name.contains("phone", ignoreCase = true) || peer.name.contains("android", ignoreCase = true)
                            val avatarGrad = if (isMac) {
                                intArrayOf(c(R.color.cr_purple), c(R.color.cr_purple_bg))
                            } else {
                                intArrayOf(c(R.color.cr_green), c(R.color.cr_green_bg))
                            }
                            
                            val card = selectionCard(
                                title = peer.name,
                                subtitle = if (peer.trusted) "Ready to receive" else "Connected now",
                                avatarLabel = peer.name.firstOrNull()?.uppercase() ?: "?",
                                gradientColors = avatarGrad
                            ) { updateSelection(peer.id) }
                            selectionCards += peer.id to card
                            addView(card)
                        }

                        updateSelection(null)
                    }
                })
            })

            addView(space(24))

            // Buttons row
            val btnRow = LinearLayout(this@ClipRelayShareTarget).apply {
                orientation = LinearLayout.HORIZONTAL
            }

            btnRow.addView(actionButton("Cancel", filled = false) { dialog.dismiss() }, LinearLayout.LayoutParams(
                0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f
            ).also { it.marginEnd = dp(10) })

            val sendButton = actionButton("Send Files", filled = true) {
                if (peers.isEmpty()) {
                    startActivity(packageManager.getLaunchIntentForPackage(packageName))
                    dialog.dismiss()
                    return@actionButton
                }

                // Show dynamic premium loading screen while staging URIs in background!
                showLoadingOverlay(container)

                Thread {
                    val stagedUris = mutableListOf<String>()
                    sharedUris.forEachIndexed { index, uri ->
                        val stagedFileUri = stageFileInActivity(uri, index + 1)
                        if (stagedFileUri != null) {
                            stagedUris.add(stagedFileUri.toString())
                        }
                    }

                    runOnUiThread {
                        if (stagedUris.isNotEmpty()) {
                            val svc = Intent(this@ClipRelayShareTarget, ClipRelayService::class.java).apply {
                                action = ClipRelayService.ACTION_PUSH_SHARED_URI
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                putStringArrayListExtra(
                                    ClipRelayService.EXTRA_SHARED_URIS,
                                    ArrayList(stagedUris)
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
                        } else {
                            Toast.makeText(this@ClipRelayShareTarget, "Staging failed", Toast.LENGTH_SHORT).show()
                        }
                        dialog.dismiss()
                    }
                }.start()
            }
            btnRow.addView(sendButton, LinearLayout.LayoutParams(
                0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f
            ))

            addView(btnRow)
        }

        container.addView(mainLayout)
        return container
    }

    private fun showLoadingOverlay(container: FrameLayout) {
        container.removeAllViews()
        val loadingLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(dp(24), dp(60), dp(24), dp(60))

            // Premium circular loader
            addView(ProgressBar(this@ClipRelayShareTarget).apply {
                indeterminateTintList = ColorStateList.valueOf(c(R.color.cr_accent))
                layoutParams = LinearLayout.LayoutParams(dp(56), dp(56)).also {
                    it.bottomMargin = dp(20)
                }
            })

            addView(TextView(this@ClipRelayShareTarget).apply {
                text = "Preparing secure transfer..."
                textSize = 16f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(c(R.color.cr_text_1))
            })
            addView(space(6))
            addView(TextView(this@ClipRelayShareTarget).apply {
                text = "Staging files locally"
                textSize = 13f
                setTextColor(c(R.color.cr_text_3))
            })
        }
        container.addView(loadingLayout)
    }

    private fun stageFileInActivity(uri: Uri, fallbackIndex: Int): Uri? = runCatching {
        val mime = contentResolver.getType(uri) ?: "application/octet-stream"
        val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime.substringBefore(';')) ?: ""
        
        var displayName = "Shared file $fallbackIndex"
        if (uri.scheme.equals("file", ignoreCase = true)) {
            displayName = uri.path?.let { java.io.File(it).name } ?: displayName
        } else {
            contentResolver.query(uri, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                val col = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                if (col >= 0 && cursor.moveToFirst()) {
                    cursor.getString(col)?.takeIf { it.isNotBlank() }?.let { displayName = it }
                }
            }
        }
        
        val stagedDir = java.io.File(cacheDir, "shared-outgoing").also { it.mkdirs() }
        // Sanitize file name to prevent illegal chars
        val sanitizedName = displayName.replace("[\\\\/:*?\"<>|]".toRegex(), "_")
        val finalExt = if (ext.isNotEmpty()) ".$ext" else ""
        val stagedFile = java.io.File(stagedDir, if (sanitizedName.endsWith(finalExt)) sanitizedName else "$sanitizedName$finalExt")
        
        contentResolver.openInputStream(uri)?.use { input ->
            java.io.FileOutputStream(stagedFile).use { output ->
                input.copyTo(output, 256 * 1024)
            }
        } ?: return null
        
        Uri.fromFile(stagedFile)
    }.onFailure { Log.w("ClipRelayShareTarget", "Failed to stage shared URI $uri", it) }.getOrNull()

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
                it.cornerRadius = dp(24).toFloat()
                it.colors = intArrayOf(c(R.color.cr_accent), c(R.color.cr_accent_dim))
                it.orientation = GradientDrawable.Orientation.LEFT_RIGHT
            }
        } else {
            GradientDrawable().also {
                it.cornerRadius = dp(24).toFloat()
                it.setColor(c(R.color.cr_bg_inset))
                it.setStroke(dp(1), c(R.color.cr_border))
            }
        }
        setOnClickListener { onClick() }
    }

    private fun emptyState(): View = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER
        setPadding(dp(24), dp(32), dp(24), dp(32))
        background = GradientDrawable().also {
            it.cornerRadius = dp(20).toFloat()
            it.setColor(c(R.color.cr_bg_card))
            it.setStroke(dp(1), c(R.color.cr_border))
        }
        addView(TextView(this@ClipRelayShareTarget).apply {
            text = "No connected devices"
            textSize = 15f
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(c(R.color.cr_text_1))
        })
        addView(space(6))
        addView(TextView(this@ClipRelayShareTarget).apply {
            text = "Open ClipRelay on your Mac, or launch the app to search again."
            textSize = 13f
            gravity = Gravity.CENTER
            setTextColor(c(R.color.cr_text_3))
            setLineSpacing(0f, 1.35f)
        })
    }

    private fun selectionCard(
        title: String,
        subtitle: String,
        avatarLabel: String,
        gradientColors: IntArray,
        onClick: () -> Unit
    ): View = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(dp(14), dp(14), dp(14), dp(14))
        background = cardBackground(false)
        isClickable = true
        isFocusable = true
        setOnClickListener { onClick() }

        addView(FrameLayout(this@ClipRelayShareTarget).apply {
            layoutParams = LinearLayout.LayoutParams(dp(42), dp(42)).also {
                it.marginEnd = dp(14)
            }
            background = GradientDrawable().also {
                it.shape = GradientDrawable.OVAL
                it.colors = gradientColors
                it.orientation = GradientDrawable.Orientation.TL_BR
            }
            addView(TextView(this@ClipRelayShareTarget).apply {
                text = avatarLabel
                gravity = Gravity.CENTER
                textSize = if (avatarLabel.length > 1) 10f else 16f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(Color.WHITE)
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
                textSize = 12.5f
                setTextColor(c(R.color.cr_text_3))
            })
        })

        addView(View(this@ClipRelayShareTarget).apply {
            tag = "selected_indicator"
            background = GradientDrawable().also {
                it.shape = GradientDrawable.OVAL
                it.setColor(Color.TRANSPARENT)
                it.setStroke(dp(1), c(R.color.cr_border))
            }
            layoutParams = LinearLayout.LayoutParams(dp(20), dp(20))
        })
    }

    private fun cardBackground(selected: Boolean): RippleDrawable {
        val base = GradientDrawable().also {
            it.cornerRadius = dp(20).toFloat()
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
