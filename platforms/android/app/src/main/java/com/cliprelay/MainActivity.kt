package com.cliprelay

import android.Manifest
import android.app.ActivityManager
import android.app.AlertDialog
import android.content.*
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.view.View
import android.widget.*
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatButton
import androidx.core.content.ContextCompat
import androidx.core.view.setPadding
import kotlin.math.roundToInt

class MainActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ClipRelayMain"
    }

    // Dashboard views
    private lateinit var statusHeadline: TextView
    private lateinit var statusDetail: TextView
    private lateinit var deviceSubtitle: TextView

    // Activity feed
    private lateinit var feedContainer: LinearLayout
    private lateinit var emptyFeedLabel: TextView

    // Tab state
    private var currentTab = 0 // 0=Dashboard 1=Feed

    private val statusReceiver = object : BroadcastReceiver() {
        override fun onReceive(ctx: android.content.Context?, intent: Intent?) {
            updateDashboard()
            if (currentTab == 1) refreshFeed()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        ensureNotificationPermission()
        setContentView(buildRoot())
        startClipRelayService()
        updateDashboard()
    }

    override fun onResume() {
        super.onResume()
        updateDashboard()
        if (currentTab == 1) refreshFeed()
    }

    override fun onStart() {
        super.onStart()
        ContextCompat.registerReceiver(
            this, statusReceiver,
            IntentFilter(ClipRelayService.ACTION_STATUS_CHANGED),
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
    }

    override fun onStop() {
        unregisterReceiver(statusReceiver)
        super.onStop()
    }

    // ── Root layout ───────────────────────────────────────────────────────────

    private fun buildRoot(): View {
        val frame = FrameLayout(this).apply {
            setBackgroundColor(color(R.color.pb_canvas_top))
        }

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }

        root.addView(buildTabBar())
        root.addView(buildTabContent())

        frame.addView(root)
        return frame
    }

    // ── Tab bar ───────────────────────────────────────────────────────────────

    private lateinit var tabDashboard: TextView
    private lateinit var tabFeed: TextView
    private lateinit var dashboardPane: View
    private lateinit var feedPane: View

    private fun buildTabBar(): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(color(R.color.pb_canvas_bottom))
            setPadding(dp(16), dp(12), dp(16), 0)

            tabDashboard = tabLabel("Dashboard", active = true) { switchTab(0) }
            tabFeed      = tabLabel("Activity", active = false) { switchTab(1) }

            addView(tabDashboard)
            addView(Space(this@MainActivity).apply {
                layoutParams = LinearLayout.LayoutParams(dp(24), 1)
            })
            addView(tabFeed)
        }
    }

    private fun tabLabel(label: String, active: Boolean, onClick: () -> Unit): TextView {
        return TextView(this).apply {
            text = label
            textSize = 14f
            setTypeface(Typeface.DEFAULT_BOLD)
            setTextColor(if (active) color(R.color.pb_primary) else color(R.color.pb_muted))
            setPadding(0, 0, 0, dp(10))
            setOnClickListener { onClick() }
        }
    }

    private fun switchTab(tab: Int) {
        currentTab = tab
        tabDashboard.setTextColor(if (tab == 0) color(R.color.pb_primary) else color(R.color.pb_muted))
        tabFeed.setTextColor(if (tab == 1) color(R.color.pb_primary) else color(R.color.pb_muted))
        dashboardPane.visibility = if (tab == 0) View.VISIBLE else View.GONE
        feedPane.visibility      = if (tab == 1) View.VISIBLE else View.GONE
        if (tab == 1) refreshFeed()
    }

    // ── Tab content ───────────────────────────────────────────────────────────

    private fun buildTabContent(): View {
        val host = FrameLayout(this)

        dashboardPane = buildDashboardPane()
        feedPane      = buildFeedPane().also { it.visibility = View.GONE }

        host.addView(dashboardPane)
        host.addView(feedPane)
        return host
    }

    // ── Dashboard pane ────────────────────────────────────────────────────────

    private fun buildDashboardPane(): View {
        return ScrollView(this).apply {
            isFillViewport = true
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(dp(20))

                addView(buildStatusCard())
                addView(gap())
                addView(buildControlsCard())
                addView(gap())
                addView(buildHowItWorksCard())
                addView(gap(32))
            })
        }
    }

    private fun buildStatusCard(): LinearLayout {
        return card().apply {
            orientation = LinearLayout.VERTICAL

            addView(label("Status"))
            addView(gap(10))

            deviceSubtitle = TextView(this@MainActivity).apply {
                textSize = 13f
                setTextColor(color(R.color.pb_muted))
            }
            addView(deviceSubtitle)

            addView(gap(4))

            statusHeadline = TextView(this@MainActivity).apply {
                textSize = 26f
                setTypeface(Typeface.create("serif", Typeface.BOLD))
                setTextColor(color(R.color.pb_ink))
            }
            addView(statusHeadline)

            addView(gap(6))

            statusDetail = TextView(this@MainActivity).apply {
                textSize = 14f
                setTextColor(color(R.color.pb_muted))
            }
            addView(statusDetail)

            addView(gap(18))

            // Sync mode indicator row
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL

                val modeDot = View(this@MainActivity).apply {
                    layoutParams = LinearLayout.LayoutParams(dp(8), dp(8))
                    background = GradientDrawable().apply {
                        shape = GradientDrawable.OVAL
                        setColor(color(R.color.pb_primary))
                        cornerRadius = dp(4).toFloat()
                    }
                }
                addView(modeDot)

                addView(TextView(this@MainActivity).apply {
                    textSize = 12f
                    setTextColor(color(R.color.pb_muted))
                    setPadding(dp(6), 0, 0, 0)
                    text = "Encrypted · LAN only · no cloud"
                })
            })
        }
    }

    private fun buildControlsCard(): LinearLayout {
        return card(dark = true).apply {
            orientation = LinearLayout.VERTICAL

            addView(TextView(this@MainActivity).apply {
                text = "CONTROLS"
                textSize = 11f
                letterSpacing = 0.12f
                setTypeface(Typeface.DEFAULT_BOLD)
                setTextColor(color(R.color.pb_secondary))
            })
            addView(gap(8))

            addView(btn("Start sync", primary = true) { startClipRelayService(); updateDashboard() })
            addView(gap(10))
            addView(btn("Pause sync", primary = false) { sendServiceAction(ClipRelayService.ACTION_PAUSE_SYNC); updateDashboard() })
            addView(gap(10))
            addView(btn("Disconnect all", primary = false) { sendServiceAction(ClipRelayService.ACTION_DISCONNECT_ALL); updateDashboard() })
            addView(gap(10))
            addView(btn("Stop service", primary = false) { stopService(Intent(this@MainActivity, ClipRelayService::class.java)); updateDashboard() })
            addView(gap(10))
            addView(btn("Settings", primary = false) { startActivity(Intent(this@MainActivity, SettingsActivity::class.java)) })
        }
    }

    private fun buildHowItWorksCard(): LinearLayout {
        return card().apply {
            orientation = LinearLayout.VERTICAL
            addView(label("How it works"))
            addView(noteRow("1", "Keep ClipRelay running, open the desktop app on the same Wi-Fi network."))
            addView(noteRow("2", "Trust the device fingerprint once — reconnects happen automatically."))
            addView(noteRow("3", "Clipboard text, images, and files move silently — no notification spam."))
        }
    }

    // ── Activity feed pane ────────────────────────────────────────────────────

    private fun buildFeedPane(): View {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }

        root.addView(LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(20), dp(16), dp(20), dp(8))

            addView(TextView(this@MainActivity).apply {
                text = "Recent activity"
                textSize = 16f
                setTypeface(Typeface.DEFAULT_BOLD)
                setTextColor(color(R.color.pb_ink))
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(btn("Clear", primary = false) {
                synchronized(ClipRelayService.feedLock) { ClipRelayService.activityFeed.clear() }
                refreshFeed()
            }.apply {
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                )
            })
        })

        emptyFeedLabel = TextView(this).apply {
            text = "No activity yet.\nClipboard syncs will appear here."
            textSize = 14f
            gravity = Gravity.CENTER
            setTextColor(color(R.color.pb_muted))
            visibility = View.GONE
            setPadding(dp(24))
        }
        root.addView(emptyFeedLabel)

        feedContainer = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(16), 0, dp(16), dp(16))
        }

        root.addView(ScrollView(this).apply {
            isFillViewport = true
            addView(feedContainer)
        })

        return root
    }

    private fun refreshFeed() {
        feedContainer.removeAllViews()
        val entries = ClipRelayService.getFeedSnapshot()

        if (entries.isEmpty()) {
            emptyFeedLabel.visibility = View.VISIBLE
            feedContainer.visibility  = View.GONE
            return
        }
        emptyFeedLabel.visibility = View.GONE
        feedContainer.visibility  = View.VISIBLE

        entries.forEach { entry ->
            feedContainer.addView(buildFeedRow(entry))
        }
    }

    private fun buildFeedRow(entry: ActivityEntry): View {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(14), dp(12), dp(14), dp(12))
            background = GradientDrawable().apply {
                cornerRadius = dp(14).toFloat()
                setColor(color(R.color.pb_surface))
                setStroke(dp(1), color(R.color.pb_outline))
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, dp(8)) }

            // Kind icon
            addView(TextView(this@MainActivity).apply {
                text = when (entry.kind) {
                    "text"  -> "📋"
                    "image" -> "🖼️"
                    "file"  -> "📎"
                    else    -> "📋"
                }
                textSize = 18f
                layoutParams = LinearLayout.LayoutParams(dp(36), dp(36)).apply {
                    gravity = Gravity.CENTER_VERTICAL
                }
            })

            // Text column
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                    .apply { setMargins(dp(10), 0, 0, 0) }

                addView(TextView(this@MainActivity).apply {
                    text = entry.formattedLine()
                    textSize = 13f
                    setTextColor(color(R.color.pb_ink))
                    maxLines = 1
                    ellipsize = android.text.TextUtils.TruncateAt.END
                })

                if (entry.kind == "text" && entry.preview.isNotBlank()) {
                    addView(TextView(this@MainActivity).apply {
                        text = "\"${entry.preview.take(60)}\""
                        textSize = 12f
                        setTextColor(color(R.color.pb_muted))
                        maxLines = 1
                        ellipsize = android.text.TextUtils.TruncateAt.END
                    })
                }
            })

            // Timestamp
            addView(TextView(this@MainActivity).apply {
                text = formatTime(entry.timestamp)
                textSize = 11f
                setTextColor(color(R.color.pb_muted))
            })
        }
    }

    private fun formatTime(ms: Long): String {
        val now = System.currentTimeMillis()
        val diff = now - ms
        return when {
            diff < 60_000L     -> "just now"
            diff < 3_600_000L  -> "${diff / 60_000}m ago"
            diff < 86_400_000L -> "${diff / 3_600_000}h ago"
            else               -> "${diff / 86_400_000}d ago"
        }
    }

    // ── Dashboard update ──────────────────────────────────────────────────────

    private fun updateDashboard() {
        val running  = isServiceRunning()
        val prefs    = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
        val myName   = prefs.getString("local_device_name", null)?.takeIf { it.isNotBlank() } ?: Build.MODEL
        val syncOn   = prefs.getBoolean("sync_enabled", true)
        val peers    = prefs.getStringSet("connected_names", emptySet())
            ?.filter { it.isNotBlank() }?.sorted().orEmpty()

        deviceSubtitle.text = myName

        statusHeadline.text = when {
            !running        -> "Stopped"
            !syncOn         -> "Sync paused"
            peers.isNotEmpty() -> peers.joinToString(" · ")
            else            -> "No devices nearby"
        }

        statusDetail.text = when {
            !running        -> "Tap \"Start sync\" to begin."
            !syncOn         -> "Clipboard is not syncing. Tap Resume Sync to re-enable."
            peers.isNotEmpty() -> "Clipboard sync is active with ${peers.size} device${if (peers.size > 1) "s" else ""}."
            else            -> "ClipRelay is running. Open the desktop app on the same network."
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private fun startClipRelayService() {
        runCatching {
            ContextCompat.startForegroundService(
                this,
                Intent(this, ClipRelayService::class.java).apply { action = ClipRelayService.ACTION_START }
            )
        }.onFailure { Log.e(TAG, "Failed to start service", it) }
    }

    private fun sendServiceAction(action: String) {
        runCatching {
            startService(Intent(this, ClipRelayService::class.java).apply { this.action = action })
        }
    }

    @Suppress("DEPRECATION")
    private fun isServiceRunning(): Boolean {
        val manager = getSystemService(ACTIVITY_SERVICE) as ActivityManager
        return manager.getRunningServices(Int.MAX_VALUE)
            .any { it.service.className == ClipRelayService::class.java.name }
    }

    private fun ensureNotificationPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        if (checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
        }
    }

    // ── UI primitives ─────────────────────────────────────────────────────────

    private fun card(dark: Boolean = false): LinearLayout {
        return LinearLayout(this).apply {
            background = GradientDrawable(
                GradientDrawable.Orientation.TL_BR,
                if (dark) intArrayOf(color(R.color.pb_ink), color(R.color.pb_outline))
                else intArrayOf(color(R.color.pb_surface), color(R.color.pb_surface_alt))
            ).apply {
                cornerRadius = dp(22).toFloat()
                setStroke(dp(1), if (dark) color(R.color.pb_primary_dark) else color(R.color.pb_outline))
            }
            setPadding(dp(20))
        }
    }

    private fun btn(text: String, primary: Boolean, onClick: () -> Unit): AppCompatButton {
        return AppCompatButton(this).apply {
            this.text = text
            textSize = 14f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(
                if (primary) ContextCompat.getColor(this@MainActivity, android.R.color.white)
                else color(R.color.pb_ink)
            )
            background = if (primary) {
                GradientDrawable(
                    GradientDrawable.Orientation.LEFT_RIGHT,
                    intArrayOf(color(R.color.pb_primary), color(R.color.pb_primary_dark))
                ).apply { cornerRadius = dp(16).toFloat() }
            } else {
                GradientDrawable().apply {
                    shape = GradientDrawable.RECTANGLE
                    cornerRadius = dp(16).toFloat()
                    setColor(color(R.color.pb_surface_alt))
                    setStroke(dp(1), color(R.color.pb_outline))
                }
            }
            setPadding(dp(16), dp(12), dp(16), dp(12))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
            setOnClickListener { onClick() }
        }
    }

    private fun label(text: String): TextView = TextView(this).apply {
        this.text = text.uppercase()
        textSize = 11f
        letterSpacing = 0.10f
        setTypeface(Typeface.DEFAULT_BOLD)
        setTextColor(color(R.color.pb_primary_dark))
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
                background = GradientDrawable().apply {
                    shape = GradientDrawable.RECTANGLE
                    cornerRadius = dp(14).toFloat()
                    setColor(color(R.color.pb_surface_alt))
                    setStroke(dp(1), color(R.color.pb_outline))
                }
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

    private fun gap(size: Int = 16): Space = Space(this).apply {
        layoutParams = LinearLayout.LayoutParams(1, dp(size))
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).roundToInt()
    private fun color(id: Int) = ContextCompat.getColor(this, id)
}
