package com.cliprelay

import android.Manifest
import android.app.ActivityManager
import android.content.BroadcastReceiver
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Space
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatButton
import androidx.core.content.ContextCompat
import androidx.core.view.setPadding
import kotlin.math.roundToInt

class MainActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "ClipRelayMain"
    }

    private lateinit var heroDetailView: TextView
    private lateinit var statusView: TextView
    private lateinit var statusDetailView: TextView
    private val statusReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: android.content.Context?, intent: Intent?) {
            updateStatus()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        ensureNotificationPermission()
        setContentView(buildContentView())
        startClipboardService()
        updateStatus()
    }

    override fun onResume() {
        super.onResume()
        updateStatus()
    }

    override fun onStart() {
        super.onStart()
        ContextCompat.registerReceiver(
            this,
            statusReceiver,
            IntentFilter(ClipRelayService.ACTION_STATUS_CHANGED),
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
    }

    override fun onStop() {
        unregisterReceiver(statusReceiver)
        super.onStop()
    }

    private fun buildContentView(): FrameLayout {
        val frame = FrameLayout(this).apply {
            background = GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(color(R.color.pb_canvas_top), color(R.color.pb_canvas_bottom))
            )
        }

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(24))
        }

        root.addView(statusDeck())
        root.addView(verticalGap())
        root.addView(actionsCard())
        root.addView(verticalGap())
        root.addView(notesCard())

        frame.addView(ScrollView(this).apply {
            isFillViewport = true
            addView(root)
        })

        return frame
    }

    private fun heroCard(): LinearLayout {
        return sectionCard(
            startColor = color(R.color.pb_surface),
            endColor = color(R.color.pb_surface_alt),
            strokeColor = color(R.color.pb_outline)
        ).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL

            addView(ImageView(this@MainActivity).apply {
                setImageResource(R.drawable.cliprelay_logo)
                adjustViewBounds = true
                minimumHeight = dp(148)
                maxWidth = dp(148)
                scaleType = ImageView.ScaleType.FIT_CENTER
                setPadding(0, dp(8), 0, dp(14))
            })

            addView(TextView(this@MainActivity).apply {
                text = getString(R.string.app_name)
                textSize = 32f
                setTypeface(Typeface.create("serif", Typeface.BOLD))
                setTextColor(color(R.color.pb_ink))
                gravity = Gravity.CENTER
            })

            heroDetailView = TextView(this@MainActivity).apply {
                textSize = 15f
                setTextColor(color(R.color.pb_muted))
                gravity = Gravity.CENTER
                maxLines = 3
                setPadding(0, dp(10), 0, 0)
            }
            addView(heroDetailView)
        }
    }

    private fun statusDeck(): LinearLayout {
        return sectionCard().apply {
            orientation = LinearLayout.VERTICAL

            addView(labelView("Nearby clipboard sync"))

            heroDetailView = TextView(this@MainActivity).apply {
                textSize = 14f
                setTextColor(color(R.color.pb_muted))
                maxLines = 3
                setPadding(0, dp(6), 0, dp(12))
            }
            addView(heroDetailView)

            statusView = TextView(this@MainActivity).apply {
                textSize = 28f
                setTypeface(Typeface.create("serif", Typeface.BOLD))
                setTextColor(color(R.color.pb_ink))
                maxLines = 2
                setPadding(0, 0, 0, dp(6))
            }
            addView(statusView)

            statusDetailView = TextView(this@MainActivity).apply {
                textSize = 14f
                setTextColor(color(R.color.pb_muted))
                maxLines = 4
                setPadding(0, 0, 0, dp(4))
            }
            addView(statusDetailView)

            addView(verticalGap(18))

            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                addView(metricCard("Transport", "Foreground sync", "Stays awake while trusted devices are nearby."))
                addView(verticalGap(12))
                addView(metricCard("Trust", "LAN only", "Pairs once and keeps the clipboard off cloud relays."))
            })
        }
    }

    private fun actionsCard(): LinearLayout {
        return sectionCard(
            startColor = color(R.color.pb_ink),
            endColor = color(R.color.pb_outline),
            strokeColor = color(R.color.pb_primary_dark)
        ).apply {
            orientation = LinearLayout.VERTICAL

            addView(TextView(this@MainActivity).apply {
                text = "CONTROLS"
                textSize = 11f
                letterSpacing = 0.12f
                setTypeface(Typeface.DEFAULT_BOLD)
                setTextColor(color(R.color.pb_secondary))
            })

            addView(TextView(this@MainActivity).apply {
                text = "Keep sync running, pause it for privacy, or rename this phone before pairing another device."
                textSize = 16f
                setTextColor(ContextCompat.getColor(this@MainActivity, android.R.color.white))
                setPadding(0, dp(8), 0, dp(18))
            })

            addView(actionButton(R.string.main_start_sync, primary = true) {
                startClipboardService()
                updateStatus()
            })
            addView(verticalGap(12))
            addView(actionButton(R.string.main_stop_sync, primary = false) {
                stopService(Intent(this@MainActivity, ClipRelayService::class.java))
                updateStatus()
            })
            addView(verticalGap(12))
            addView(actionButton(R.string.main_open_settings, primary = false) {
                startActivity(Intent(this@MainActivity, SettingsActivity::class.java))
            })
        }
    }

    private fun notesCard(): LinearLayout {
        return sectionCard().apply {
            orientation = LinearLayout.VERTICAL

            addView(labelView("How it works"))
            addView(noteRow("1", "Keep the Android service running while you open ClipRelay on your Mac for the first pairing."))
            addView(noteRow("2", "Trust the fingerprint once. After that, reconnects should happen automatically on the same network."))
            addView(noteRow("3", "Copied text, images, and files will move faster when both devices stay awake during the first sync."))

            addView(TextView(this@MainActivity).apply {
                text = getString(R.string.main_footer)
                textSize = 13f
                setTextColor(color(R.color.pb_muted))
                setPadding(0, dp(12), 0, 0)
            })
        }
    }

    private fun metricCard(title: String, value: String, detail: String): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
            background = roundedFill(color(R.color.pb_surface_alt), color(R.color.pb_outline), dp(22))
            setPadding(dp(16))

            addView(labelView(title))

            addView(TextView(this@MainActivity).apply {
                text = value
                textSize = 18f
                setTypeface(Typeface.create("serif", Typeface.BOLD))
                setTextColor(color(R.color.pb_ink))
                setPadding(0, dp(8), 0, dp(6))
            })

            addView(TextView(this@MainActivity).apply {
                text = detail
                textSize = 12f
                setTextColor(color(R.color.pb_muted))
                maxLines = 3
            })
        }
    }

    private fun noteRow(index: String, text: String): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.TOP
            setPadding(0, dp(10), 0, 0)

            addView(TextView(this@MainActivity).apply {
                this.text = index
                textSize = 12f
                gravity = Gravity.CENTER
                setTypeface(Typeface.DEFAULT_BOLD)
                setTextColor(color(R.color.pb_primary_dark))
                background = roundedFill(color(R.color.pb_surface_alt), color(R.color.pb_outline), dp(14))
                layoutParams = LinearLayout.LayoutParams(dp(28), dp(28))
            })

            addView(TextView(this@MainActivity).apply {
                this.text = text
                textSize = 14f
                setTextColor(color(R.color.pb_ink))
                setPadding(dp(12), dp(4), 0, 0)
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
        }
    }

    private fun labelView(text: String): TextView {
        return TextView(this).apply {
            this.text = text.uppercase()
            textSize = 11f
            letterSpacing = 0.10f
            setTypeface(Typeface.DEFAULT_BOLD)
            setTextColor(color(R.color.pb_primary_dark))
        }
    }

    private fun sectionCard(
        startColor: Int = color(R.color.pb_surface),
        endColor: Int = color(R.color.pb_surface),
        strokeColor: Int = color(R.color.pb_outline)
    ): LinearLayout {
        return LinearLayout(this).apply {
            background = GradientDrawable(
                GradientDrawable.Orientation.TL_BR,
                intArrayOf(startColor, endColor)
            ).apply {
                cornerRadius = dp(28).toFloat()
                setStroke(dp(1), strokeColor)
            }
            setPadding(dp(20))
        }
    }

    private fun actionButton(labelRes: Int, primary: Boolean, onClick: () -> Unit): AppCompatButton {
        return AppCompatButton(this).apply {
            text = getString(labelRes)
            textSize = 15f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(
                if (primary) ContextCompat.getColor(this@MainActivity, android.R.color.white)
                else color(R.color.pb_surface)
            )
            background = if (primary) {
                GradientDrawable(
                    GradientDrawable.Orientation.LEFT_RIGHT,
                    intArrayOf(color(R.color.pb_primary), color(R.color.pb_primary_dark))
                ).apply {
                    cornerRadius = dp(18).toFloat()
                }
            } else {
                roundedFill(color(R.color.pb_outline), color(R.color.pb_primary_dark), dp(18))
            }
            setPadding(dp(18), dp(16), dp(18), dp(16))
            setOnClickListener { onClick() }
        }
    }

    private fun roundedFill(fill: Int, stroke: Int, radius: Int): GradientDrawable {
        return GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            cornerRadius = radius.toFloat()
            setColor(fill)
            setStroke(dp(1), stroke)
        }
    }

    private fun verticalGap(size: Int = 18): Space {
        return Space(this).apply {
            layoutParams = LinearLayout.LayoutParams(1, dp(size))
        }
    }

    private fun startClipboardService() {
        runCatching {
            val intent = Intent(this, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_START
            }
            ContextCompat.startForegroundService(this, intent)
        }.onFailure { error ->
            Log.e(TAG, "Failed to start ClipRelay service", error)
            statusView.text = getString(R.string.main_status_failed)
            statusDetailView.text = "The service could not boot. Confirm notifications are allowed, then try again."
        }
    }

    private fun updateStatus() {
        val running = isServiceRunning()
        val prefs = getSharedPreferences("cliprelay", MODE_PRIVATE)
        val localName = prefs.getString("local_device_name", null)?.takeIf { it.isNotBlank() }
            ?: "This Android device"
        val connectedNames = prefs.getStringSet("connected_names", emptySet())
            ?.filter { it.isNotBlank() }
            ?.sorted()
            .orEmpty()

        heroDetailView.text = if (running) {
            "$localName • nearby clipboard sync is active"
        } else {
            "$localName • sync is currently offline"
        }

        statusView.text = when {
            !running -> getString(R.string.main_status_stopped)
            connectedNames.isNotEmpty() -> "Connected to ${connectedNames.joinToString(", ")}"
            else -> "Waiting for your Mac"
        }
        statusDetailView.text = when {
            !running -> "Start sync before retrying pairing, clipboard transfer, or file delivery."
            connectedNames.isNotEmpty() -> "Trusted link is live. Clipboard and file sync are ready with ${connectedNames.joinToString(", ")}."
            else -> "ClipRelay is running and discoverable. Open the Mac app on the same network to pair and connect."
        }
    }

    @Suppress("DEPRECATION")
    private fun isServiceRunning(): Boolean {
        val manager = getSystemService(ACTIVITY_SERVICE) as ActivityManager
        return manager.getRunningServices(Int.MAX_VALUE).any {
            it.service.className == ClipRelayService::class.java.name
        }
    }

    private fun ensureNotificationPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        if (checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) {
            return
        }
        requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
    }

    private fun dp(value: Int): Int {
        return (value * resources.displayMetrics.density).roundToInt()
    }

    private fun color(resId: Int): Int {
        return ContextCompat.getColor(this, resId)
    }
}
