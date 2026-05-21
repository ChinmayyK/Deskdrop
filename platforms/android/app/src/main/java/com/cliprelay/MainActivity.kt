package com.cliprelay

import android.Manifest
import android.content.*
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.os.Build
import android.os.Bundle
import android.text.Editable
import android.text.TextUtils
import android.text.TextWatcher
import android.view.*
import android.view.animation.AlphaAnimation
import android.view.animation.Animation
import android.widget.*
import androidx.annotation.DrawableRes
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatButton
import androidx.appcompat.widget.AppCompatImageView
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import kotlin.math.roundToInt
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode

// ─── Context helpers ──────────────────────────────────────────────────────────

private fun android.content.Context.dp(v: Int): Int =
    (v * resources.displayMetrics.density).roundToInt()
private fun android.content.Context.dp(v: Float): Int =
    (v * resources.displayMetrics.density).roundToInt()
private fun android.content.Context.cr(@androidx.annotation.ColorRes id: Int): Int =
    ContextCompat.getColor(this, id)

// ─── MainActivity ─────────────────────────────────────────────────────────────

class MainActivity : AppCompatActivity() {

    companion object {
        private const val FEED_REFRESH_MS = 5_000L  // refresh feed every 5 s while visible
    }

    // ── Tab ───────────────────────────────────────────────────────────────────
    private enum class Tab { DASHBOARD, FEED }
    private var tab = Tab.DASHBOARD

    // ── Dashboard refs ────────────────────────────────────────────────────────
    private lateinit var heroStateLabel: TextView     // "ACTIVE" / "PAUSED" etc.
    private lateinit var heroSubline: TextView        // device name
    private lateinit var heroStatusDot: View
    private lateinit var peerSection: LinearLayout
    private lateinit var peerRows: LinearLayout
    private lateinit var noPeersState: LinearLayout
    private lateinit var noPeersTitle: TextView
    private lateinit var noPeersMessage: TextView
    private lateinit var primaryActionBtn: AppCompatButton
    private lateinit var secondaryActionsContainer: LinearLayout

    // ── Feed refs ─────────────────────────────────────────────────────────────
    private lateinit var searchInput: EditText
    private lateinit var chipAll: TextView
    private lateinit var chipClip: TextView
    private lateinit var chipFiles: TextView
    private lateinit var chipPeers: TextView
    private lateinit var feedContainer: LinearLayout
    private lateinit var feedScroller: ScrollView
    private lateinit var feedEmptyState: View

    // ── Nav refs ──────────────────────────────────────────────────────────────
    private lateinit var navItemDash: LinearLayout
    private lateinit var navItemFeed: LinearLayout
    private lateinit var dashPane: View
    private lateinit var feedPane: View
    private lateinit var rootChrome: LinearLayout
    private lateinit var contentHost: FrameLayout
    private lateinit var bottomNavBar: LinearLayout

    // ── Flow preview refs ─────────────────────────────────────────────────────
    private lateinit var flowPreviewRows: LinearLayout

    // ── State ─────────────────────────────────────────────────────────────────
    private var activeFilter = "all"
    private var searchQuery  = ""

    // Periodic feed refresh — keeps timestamps ("2m ago") and progress bars live
    // while the feed tab is open, without needing a broadcast from the service.
    private val feedRefreshHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private val feedRefreshRunnable = object : Runnable {
        override fun run() {
            if (tab == Tab.FEED) {
                rebuildFeed()
                feedRefreshHandler.postDelayed(this, FEED_REFRESH_MS)
            }
        }
    }

    private val statusReceiver = object : BroadcastReceiver() {
        override fun onReceive(ctx: Context?, intent: Intent?) {
            runOnUiThread {
                refreshDashboard()
                if (tab == Tab.FEED) rebuildFeed()
            }
        }
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        requestNotificationPermission()
        setContentView(buildRoot())
        configureEdgeToEdge()
        launchService()
        refreshDashboard()
    }

    override fun onResume() {
        super.onResume()
        refreshDashboard()
        if (tab == Tab.FEED) rebuildFeed()
        try {
            startService(Intent(this, ClipRelayService::class.java))
        } catch (e: Exception) {
            android.util.Log.e("MainActivity", "Failed to refresh service onResume", e)
        }
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
        feedRefreshHandler.removeCallbacks(feedRefreshRunnable)
        super.onStop()
    }

    // ── Root ──────────────────────────────────────────────────────────────────

    private fun buildRoot(): View {
        dashPane = buildDashPane()
        feedPane = buildFeedPane().apply { visibility = View.GONE }

        contentHost = FrameLayout(this).apply {
            addView(dashPane); addView(feedPane)
        }

        bottomNavBar = buildBottomNav()
        rootChrome = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(cr(R.color.cr_bg))
            addView(contentHost, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(bottomNavBar, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, dp(66)))
        }

        return FrameLayout(this).apply {
            setBackgroundColor(cr(R.color.cr_bg))
            addView(rootChrome, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            ))
        }
    }

    private fun configureEdgeToEdge() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        window.statusBarColor = Color.TRANSPARENT
        window.navigationBarColor = Color.TRANSPARENT

        val isDark = (resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK) == android.content.res.Configuration.UI_MODE_NIGHT_YES
        WindowInsetsControllerCompat(window, rootChrome).apply {
            isAppearanceLightStatusBars = !isDark
            isAppearanceLightNavigationBars = !isDark
        }

        ViewCompat.setOnApplyWindowInsetsListener(rootChrome) { _, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            contentHost.setPadding(0, bars.top, 0, 0)
            bottomNavBar.setPadding(0, 0, 0, bars.bottom)
            insets
        }
        ViewCompat.requestApplyInsets(rootChrome)
    }

    // ── Bottom navigation ─────────────────────────────────────────────────────
    // Icons: filled square for dashboard, three-line list for feed.
    // Active tab: accent colour + 3dp pill indicator above icon.
    // Tab labels use medium weight.

    private fun buildBottomNav(): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(cr(R.color.cr_nav_bg))
            elevation = dp(8).toFloat()
            // Top hairline via background overlay
            val borderView = View(this@MainActivity).apply {
                setBackgroundColor(cr(R.color.cr_border))
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT, dp(1))
            }
            // Wrap in frame so hairline sits on top
            addView(FrameLayout(this@MainActivity).apply {
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, 1f)
                navItemDash = buildNavItem(
                    iconRes = R.drawable.ic_cr_dashboard, label = "Dashboard", active = true,
                    onClick = { switchTab(Tab.DASHBOARD) })
                addView(navItemDash, FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT))
                addView(borderView, FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT, dp(1)))
            })
            addView(FrameLayout(this@MainActivity).apply {
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.MATCH_PARENT, 1f)
                val borderView2 = View(this@MainActivity).apply {
                    setBackgroundColor(cr(R.color.cr_border))
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT, dp(1))
                }
                navItemFeed = buildNavItem(
                    iconRes = R.drawable.ic_cr_activity, label = "Activity", active = false,
                    onClick = { switchTab(Tab.FEED) })
                addView(navItemFeed, FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT))
                addView(borderView2, FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT, dp(1)))
            })
        }
    }

    private fun buildNavItem(@DrawableRes iconRes: Int, label: String, active: Boolean,
                             onClick: () -> Unit): LinearLayout {
        val on  = cr(R.color.cr_nav_on)
        val off = cr(R.color.cr_nav_off)
        val tint = if (active) on else off

        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            isClickable = true; isFocusable = true
            background = ripple(cr(R.color.cr_ripple))
            setOnClickListener { onClick() }
            installPressFeedback()

            // Active indicator pill at very top
            addView(View(this@MainActivity).apply {
                tag = "pill"
                background = GradientDrawable().also {
                    it.cornerRadius = dp(3).toFloat()
                    it.setColor(if (active) on else Color.TRANSPARENT)
                }
                layoutParams = LinearLayout.LayoutParams(dp(28), dp(3)).also {
                    it.bottomMargin = dp(8)
                    it.gravity = Gravity.CENTER_HORIZONTAL
                }
            })

            // Icon
            addView(AppCompatImageView(this@MainActivity).apply {
                tag = "icon"
                setImageResource(iconRes)
                imageTintList = android.content.res.ColorStateList.valueOf(tint)
                layoutParams = LinearLayout.LayoutParams(
                    dp(22),
                    dp(22)).also {
                    it.bottomMargin = dp(3)
                }
            })

            // Label
            addView(TextView(this@MainActivity).apply {
                tag = "label"
                text = label
                textSize = 10.5f
                gravity = Gravity.CENTER
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(tint)
            })
        }
    }

    private fun switchTab(newTab: Tab) {
        tab = newTab
        dashPane.visibility = if (newTab == Tab.DASHBOARD) View.VISIBLE else View.GONE
        feedPane.visibility  = if (newTab == Tab.FEED)      View.VISIBLE else View.GONE
        updateNavState(newTab)
        if (newTab == Tab.FEED) {
            rebuildFeed()
            // Start periodic refresh to keep timestamps and progress bars current.
            feedRefreshHandler.removeCallbacks(feedRefreshRunnable)
            feedRefreshHandler.postDelayed(feedRefreshRunnable, FEED_REFRESH_MS)
        } else {
            feedRefreshHandler.removeCallbacks(feedRefreshRunnable)
        }
    }

    private fun updateNavState(active: Tab) {
        fun applyToNav(item: LinearLayout, isActive: Boolean) {
            val tint = if (isActive) cr(R.color.cr_nav_on) else cr(R.color.cr_nav_off)
            for (i in 0 until item.childCount) {
                val child = item.getChildAt(i)
                when (child.tag) {
                    "pill"  -> (child.background as? GradientDrawable)
                                   ?.setColor(if (isActive) tint else Color.TRANSPARENT)
                    "icon"  -> (child as? AppCompatImageView)?.imageTintList =
                        android.content.res.ColorStateList.valueOf(tint)
                    "label" -> (child as? TextView)?.setTextColor(tint)
                }
            }
        }
        applyToNav(navItemDash, active == Tab.DASHBOARD)
        applyToNav(navItemFeed,  active == Tab.FEED)
    }

    // ── Dashboard pane ────────────────────────────────────────────────────────

    private fun buildDashPane(): View = ScrollView(this).apply {
        isFillViewport = true
        setBackgroundColor(cr(R.color.cr_bg))
        addView(LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.VERTICAL

            // Top chrome bar (app name + settings)
            addView(buildAppBar())

            // Full-bleed hero
            addView(buildStatusHero())
            addView(vGap(1).apply { setBackgroundColor(cr(R.color.cr_border)) })

            // Content area
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(dp(16), dp(16), dp(16), dp(24))

                addView(buildPeersSection())
                addView(vSpace(12))
                addView(buildTelemetrySection())
                addView(vSpace(12))
                addView(buildActionsSection())
                addView(vSpace(12))
                addView(buildFlowPreviewSection())
                addView(vSpace(12))
                addView(buildInfoSection())
            })
        })
    }

    // App bar — flush with screen edge, no card
    private fun buildAppBar(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setBackgroundColor(cr(R.color.cr_bg))
        setPadding(dp(20), dp(16), dp(16), dp(14))

        addView(TextView(this@MainActivity).apply {
            text = "ClipRelay"
            textSize = 20f
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_text_1))
            layoutParams = LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        })

        // Settings icon button — circle background
        addView(FrameLayout(this@MainActivity).apply {
            val sz = dp(36)
            layoutParams = LinearLayout.LayoutParams(sz, sz)
            background = GradientDrawable().also {
                it.shape = GradientDrawable.OVAL
                it.setColor(cr(R.color.cr_bg_inset))
            }
            isClickable = true; isFocusable = true
            background = ripple(cr(R.color.cr_ripple),
                GradientDrawable().also {
                    it.shape = GradientDrawable.OVAL
                    it.setColor(cr(R.color.cr_bg_inset))
                })
            setOnClickListener { startActivity(Intent(this@MainActivity, SettingsActivity::class.java)) }
            addView(AppCompatImageView(this@MainActivity).apply {
                setImageResource(R.drawable.ic_cr_settings)
                imageTintList = android.content.res.ColorStateList.valueOf(cr(R.color.cr_text_3))
                scaleType = ImageView.ScaleType.CENTER
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT)
            })
        })
    }

    // Status hero — full-width, no card border, coloured background stripe
    private fun buildStatusHero(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setBackgroundColor(cr(R.color.cr_bg_card))
        setPadding(dp(20), dp(12), dp(20), dp(16))

        // Row 1: dot + state label
        addView(LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL

            heroStatusDot = View(this@MainActivity).apply {
                background = GradientDrawable().also {
                    it.shape = GradientDrawable.OVAL
                    it.setColor(cr(R.color.cr_text_4))
                }
                layoutParams = LinearLayout.LayoutParams(dp(8), dp(8)).also {
                    it.rightMargin = dp(7)
                    it.gravity = Gravity.CENTER_VERTICAL
                }
            }
            addView(heroStatusDot)

            heroStateLabel = TextView(this@MainActivity).apply {
                text = "● 0 DEVICES CONNECTED"
                textSize = 11f
                letterSpacing = 0.10f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_3))
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            }
            addView(heroStateLabel)
        })

        addView(vSpace(6))

        // Row 2: Subtitle
        heroSubline = TextView(this@MainActivity).apply {
            text = "Ready to connect"
            textSize = 15f
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_text_1))
            maxLines = 2
            ellipsize = TextUtils.TruncateAt.END
        }
        addView(heroSubline)

        addView(vSpace(14))

        // Row 3: security tags
        addView(LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL

            addView(infoTag("Noise protocol"))
            addView(hSpace(6))
            addView(infoTag("LAN first"))
            addView(hSpace(6))
            addView(infoTag("Hotspot ready"))
        })
    }

    // Peers section (inside scroll content)
    private fun buildPeersSection(): LinearLayout {
        peerRows = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }

        noPeersState = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, dp(8), 0, dp(4))

            noPeersTitle = TextView(this@MainActivity).apply {
                text = "No devices found"
                textSize = 15f
                setTypeface(typeface, Typeface.BOLD)
                setTextColor(cr(R.color.cr_text_2))
                gravity = Gravity.CENTER
            }
            addView(noPeersTitle)
            addView(vSpace(4))
            noPeersMessage = TextView(this@MainActivity).apply {
                text = "Open the desktop app on the same Wi-Fi network"
                textSize = 13f
                setTextColor(cr(R.color.cr_text_3))
                gravity = Gravity.CENTER
                setLineSpacing(0f, 1.4f)
            }
            addView(noPeersMessage)
        }

        peerSection = card().apply {
            addView(sectionEyebrow("Connected devices"))
            addView(vSpace(14))
            addView(noPeersState)
            addView(peerRows)
        }
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(peerSection)
        }
    }


    // Telemetry section
    private fun buildTelemetrySection(): LinearLayout = card().apply {
        addView(sectionEyebrow("Telemetry"))
        addView(vSpace(14))

        addView(LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            
            fun telemetryItem(title: String, value: String): LinearLayout = LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                
                addView(TextView(this@MainActivity).apply {
                    text = title
                    textSize = 10.5f
                    letterSpacing = 0.05f
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_text_4))
                })
                addView(vSpace(4))
                addView(TextView(this@MainActivity).apply {
                    text = value
                    textSize = 14f
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_text_1))
                })
            }
            
            addView(telemetryItem("LATENCY", "12ms"))
            addView(telemetryItem("CONNECTION", "LAN"))
            addView(telemetryItem("ENCRYPTION", "Active"))
        })
    }

    // Actions section
    private fun buildActionsSection(): LinearLayout {
        primaryActionBtn = AppCompatButton(this).apply {
            text = "Start sync"
            textSize = 15.5f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_on_accent))
            background = GradientDrawable().also {
                it.cornerRadius = dp(15).toFloat()
                it.setColor(cr(R.color.cr_accent))
            }
            setPadding(dp(24), dp(15), dp(24), dp(15))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT)
            setOnClickListener { launchService(); refreshDashboard() }
            installPressFeedback()
        }

        secondaryActionsContainer = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }

        return card().apply {
            addView(sectionEyebrow("Actions"))
            addView(vSpace(12))
            addView(primaryActionBtn)
            addView(vSpace(10))
            addView(secondaryActionsContainer)
        }
    }

    private fun buildFlowPreviewSection(): LinearLayout {
        flowPreviewRows = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }

        return card().apply {
            addView(sectionEyebrow("Flow state"))
            addView(vSpace(12))
            addView(TextView(this@MainActivity).apply {
                text = "Recent continuity activity stays visible here so the app feels like a live companion, not a settings screen."
                textSize = 13f
                setTextColor(cr(R.color.cr_text_3))
                setLineSpacing(0f, 1.4f)
            })
            addView(vSpace(14))
            addView(flowPreviewRows)
        }
    }

    // Info section — 3 clean rows with dot leaders
    private fun buildInfoSection(): LinearLayout = card().apply {
        addView(sectionEyebrow("How it works"))
        addView(vSpace(14))

        listOf(
            Triple("Wi-Fi or hotspot", "Discovery now rescans on network changes, including hotspot routing.", cr(R.color.cr_accent)),
            Triple("Pair once", "Trusted devices stay remembered so reconnecting is one tap away.", cr(R.color.cr_green)),
            Triple("Direct transfer", "Clipboard and files move device-to-device without a shared folder step.", cr(R.color.cr_blue))
        ).forEachIndexed { i, (title, desc, accent) ->
            if (i > 0) {
                addView(View(this@MainActivity).apply {
                    setBackgroundColor(cr(R.color.cr_divider))
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT, dp(1)
                    ).also { it.setMargins(dp(34), dp(10), 0, dp(10)) }
                })
            }
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.TOP

                // Coloured dot
                addView(View(this@MainActivity).apply {
                    background = GradientDrawable().also {
                        it.shape = GradientDrawable.OVAL
                        it.setColor(accent)
                    }
                    layoutParams = LinearLayout.LayoutParams(dp(7), dp(7)).also {
                        it.topMargin = dp(6)
                        it.rightMargin = dp(13)
                    }
                })

                addView(LinearLayout(this@MainActivity).apply {
                    orientation = LinearLayout.VERTICAL
                    layoutParams = LinearLayout.LayoutParams(0,
                        LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                    addView(TextView(this@MainActivity).apply {
                        text = title
                        textSize = 14f
                        setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                        setTextColor(cr(R.color.cr_text_1))
                    })
                    addView(vSpace(2))
                    addView(TextView(this@MainActivity).apply {
                        text = desc
                        textSize = 13f
                        setTextColor(cr(R.color.cr_text_3))
                        setLineSpacing(0f, 1.4f)
                    })
                })
            })
        }
    }

    // ── Dashboard refresh ─────────────────────────────────────────────────────

    private fun refreshDashboard() {
        val prefs   = prefs()
        val running = prefs.getBoolean(ClipRelayService.PREF_SERVICE_RUNNING, false)
        val syncOn  = prefs.getBoolean("sync_enabled", true)
        val myName  = prefs.getString("local_device_name", null)
                          ?.takeIf { it.isNotBlank() } ?: Build.MODEL
        val peers = prefs.peerSnapshots()
        val connectedPeers = peers.filter { it.isConnected }
        val connectingPeers = peers.filter { it.isConnecting }
        val reconnectablePeers = peers.filter { !it.isConnected && it.isReconnectable }
        val attentionPeers = peers.filter { it.needsAttention || it.needsTrust }

        // Contextual subtitle (former heroSubline)
        heroSubline.text = when {
            !running -> "Sync stopped"
            !syncOn -> "Sync paused"
            connectedPeers.isNotEmpty() -> "Connected to " + connectedPeers.take(3).joinToString(", ") { it.name } +
                if (connectedPeers.size > 3) " +${connectedPeers.size - 3}" else ""
            connectingPeers.isNotEmpty() -> "Reconnecting nearby devices"
            attentionPeers.isNotEmpty() -> attentionPeers.first().name
            reconnectablePeers.isNotEmpty() -> "Trusted devices ready"
            else -> "Looking for nearby devices"
        }

        // State label + dot
        val (stateText, dotColor, stateColor) = when {
            !running -> Triple("STOPPED", cr(R.color.cr_red), cr(R.color.cr_red))
            !syncOn  -> Triple("PAUSED", cr(R.color.cr_amber), cr(R.color.cr_amber))
            connectedPeers.isNotEmpty() -> Triple(
                if (connectedPeers.size == 1) "1 DEVICE CONNECTED" else "${connectedPeers.size} DEVICES CONNECTED",
                cr(R.color.cr_green), cr(R.color.cr_green))
            connectingPeers.isNotEmpty() -> Triple("RECONNECTING", cr(R.color.cr_blue), cr(R.color.cr_blue))
            attentionPeers.isNotEmpty() -> Triple("ACTION REQUIRED", cr(R.color.cr_amber), cr(R.color.cr_amber))
            reconnectablePeers.isNotEmpty() -> Triple("READY TO RECONNECT", cr(R.color.cr_accent), cr(R.color.cr_accent))
            else -> Triple("SCANNING", cr(R.color.cr_text_3), cr(R.color.cr_text_3))
        }
        heroStateLabel.text = stateText
        heroStateLabel.setTextColor(stateColor)
        (heroStatusDot.background as? GradientDrawable)?.setColor(dotColor)
        animateStatusDot(running && syncOn && (connectedPeers.isNotEmpty() || connectingPeers.isNotEmpty()))

        // Peers
        updateNoPeersState(running, syncOn, reconnectablePeers.isNotEmpty())
        refreshPeerRows(peers, running && syncOn)
        refreshFlowPreview()

        // Primary button
        primaryActionBtn.text = when {
            !running -> "Start sync"
            !syncOn -> "Resume sync"
            else -> "Scan for devices"
        }
        primaryActionBtn.setOnClickListener {
            when {
                !running -> launchService()
                !syncOn -> sendAction(ClipRelayService.ACTION_RESUME_SYNC)
                else -> {
                    sendAction(ClipRelayService.ACTION_SCAN_NOW)
                    showSnack("Scanning for devices…")
                }
            }
            refreshDashboard()
        }

        // Secondary actions
        buildSecondaryActions(running, syncOn, peers)
    }

    private fun animateStatusDot(pulse: Boolean) {
        heroStatusDot.clearAnimation()
        if (!pulse) return
        heroStatusDot.startAnimation(AlphaAnimation(1f, 0.25f).apply {
            duration = 900; repeatMode = Animation.REVERSE; repeatCount = Animation.INFINITE
        })
    }

    private fun updateNoPeersState(running: Boolean, syncOn: Boolean, hasReconnectablePeers: Boolean) {
        val (title, message) = when {
            !running -> "Sync is stopped" to
                "Start the background service to discover devices and exchange clipboard updates."
            !syncOn -> "Sync is paused" to
                "Resume sync to reconnect to your trusted devices."
            hasReconnectablePeers -> "No device connected" to
                "Your trusted devices are remembered. Scan again to reconnect over Wi-Fi or hotspot."
            else -> "No devices found" to
                "Open ClipRelay on the other device, then scan again on the same Wi-Fi or hotspot."
        }
        noPeersTitle.text = title
        noPeersMessage.text = message
    }

    private fun refreshPeerRows(peers: List<PeerSnapshot>, serviceReady: Boolean) {
        peerRows.removeAllViews()
        if (peers.isEmpty()) {
            noPeersState.visibility = View.VISIBLE
            peerRows.visibility     = View.GONE
            return
        }
        noPeersState.visibility = View.GONE
        peerRows.visibility     = View.VISIBLE

        peers.forEachIndexed { i, peer ->
            if (i > 0) peerRows.addView(View(this).apply {
                setBackgroundColor(cr(R.color.cr_divider))
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, dp(1)
                ).also { it.setMargins(dp(52), 0, 0, 0) }
            })
            peerRows.addView(buildPeerRow(peer, serviceReady))
        }
    }

    private fun buildPeerRow(peer: PeerSnapshot, serviceReady: Boolean): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(16), dp(16), dp(16), dp(16))
            isClickable = true; isFocusable = true
            
            val accent = when {
                peer.isConnected -> cr(R.color.cr_green)
                peer.isConnecting -> cr(R.color.cr_blue)
                peer.needsTrust || peer.needsAttention -> cr(R.color.cr_amber)
                peer.isReconnectable -> cr(R.color.cr_accent)
                else -> cr(R.color.cr_text_4)
            }
            
            // Compact glowing card background
            background = GradientDrawable().also {
                it.cornerRadius = dp(16).toFloat()
                it.setColor(cr(R.color.cr_bg_card))
                it.setStroke(dp(1), cr(R.color.cr_border))
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                outlineAmbientShadowColor = accent
                outlineSpotShadowColor = accent
                elevation = dp(6).toFloat()
            }
            
            val lp = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT)
            lp.bottomMargin = dp(12)
            layoutParams = lp

            val statusLabel = when {
                peer.isConnected -> "Active"
                peer.isConnecting -> "Reconnecting..."
                peer.isRejected -> "Trust Required"
                peer.needsAttention -> "Needs Attention"
                peer.isReconnectable -> "Linked"
                else -> "Offline"
            }

            // Icon
            addView(FrameLayout(this@MainActivity).apply {
                layoutParams = LinearLayout.LayoutParams(dp(40), dp(40)).also { it.rightMargin = dp(16) }
                background = GradientDrawable().also {
                    it.shape = GradientDrawable.OVAL
                    it.setColor(accent and 0x22FFFFFF) // 13% opacity
                }
                addView(TextView(this@MainActivity).apply {
                    text = peer.name.take(1).uppercase()
                    textSize = 16f
                    gravity = Gravity.CENTER
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(accent)
                    layoutParams = FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT)
                })
                
                // Status dot indicator
                addView(View(this@MainActivity).apply {
                    layoutParams = FrameLayout.LayoutParams(dp(10), dp(10), Gravity.BOTTOM or Gravity.END).also {
                        it.bottomMargin = dp(1)
                        it.rightMargin = dp(1)
                    }
                    background = GradientDrawable().also {
                        it.shape = GradientDrawable.OVAL
                        it.setColor(accent)
                        it.setStroke(dp(2), cr(R.color.cr_bg_card))
                    }
                })
            })

            // Text column (Name + Sublabel)
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                
                addView(TextView(this@MainActivity).apply {
                    text = peer.name
                    textSize = 16f
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_text_1))
                })
                addView(vSpace(2))
                addView(TextView(this@MainActivity).apply {
                    text = statusLabel
                    textSize = 12f
                    setTextColor(accent)
                })
            })

            // Optional stats/action on the right
            val lastSync = peer.lastSyncSecs?.times(1000) ?: prefs().getLong("last_sync_${peer.name.take(32)}", 0L)
            addView(TextView(this@MainActivity).apply {
                text = if (lastSync > 0L) relativeTime(lastSync) else ""
                textSize = 11f
                setTextColor(cr(R.color.cr_text_3))
                gravity = Gravity.END
            })

            setOnClickListener {
                if (peer.isConnected) {
                    // Show a quick bounce
                    it.animate().scaleX(0.97f).scaleY(0.97f).setDuration(70)
                        .withEndAction { it.animate().scaleX(1f).scaleY(1f).setDuration(70).start() }.start()
                } else if (!peer.trusted) {
                    showTrustDialog(peer)
                } else if (serviceReady && peer.isReconnectable) {
                    sendAction(ClipRelayService.ACTION_SCAN_NOW)
                    showSnack("Scanning for ${peer.name}...")
                    it.animate().scaleX(0.95f).scaleY(0.95f).setDuration(100)
                        .withEndAction { it.animate().scaleX(1f).scaleY(1f).setDuration(100).start() }.start()
                } else {
                    it.animate().scaleX(0.97f).scaleY(0.97f).setDuration(70)
                        .withEndAction { it.animate().scaleX(1f).scaleY(1f).setDuration(70).start() }.start()
                }
            }
        }
        
    private fun buildCardButton(label: String, color: Int, onClick: () -> Unit): View {
        return TextView(this).apply {
            text = label
            textSize = 12f
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(color)
            gravity = Gravity.CENTER
            setPadding(dp(16), dp(8), dp(16), dp(8))
            background = GradientDrawable().also {
                it.cornerRadius = dp(12).toFloat()
                it.setStroke(dp(1), cr(R.color.cr_border))
            }
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            setOnClickListener { onClick() }
            isClickable = true; isFocusable = true
        }
    }

    private fun buildSecondaryActions(running: Boolean, syncOn: Boolean, peers: List<PeerSnapshot>) {
        secondaryActionsContainer.removeAllViews()
        if (!running) return

        val connectedPeers = peers.filter { it.isConnected }
        val reconnectablePeers = peers.filter { !it.isConnected && it.isReconnectable }

        val items = buildList {
            // "Send clipboard" — most common cross-device action, shown first when connected.
            // Sends the current Android clipboard text to all paired Macs.
            if (connectedPeers.isNotEmpty() && syncOn) {
                add(Triple("Send clipboard to Mac", cr(R.color.cr_accent)) {
                    val cm = getSystemService(ClipboardManager::class.java)
                    val clip = cm.primaryClip?.getItemAt(0)?.coerceToText(this@MainActivity)
                    if (clip.isNullOrBlank()) {
                        showSnack("Clipboard is empty")
                    } else {
                        sendAction(ClipRelayService.ACTION_PUSH_CLIPBOARD)
                        showSnack("Sending clipboard…")
                    }
                })
            }
            add(Triple(
                if (reconnectablePeers.isNotEmpty()) "Reconnect remembered devices" else "Scan for devices",
                cr(R.color.cr_accent)
            ) {
                sendAction(ClipRelayService.ACTION_SCAN_NOW)
                showSnack("Scanning for devices…")
            })
            add(Triple("Pair via Magic Link", cr(R.color.cr_green)) {
                showMagicLinkPairingDialog()
            })
            val toggleLabel  = if (syncOn) "Pause sync" else "Resume sync"
            val toggleAction = if (syncOn) ClipRelayService.ACTION_PAUSE_SYNC
                               else        ClipRelayService.ACTION_RESUME_SYNC
            add(Triple(toggleLabel, cr(R.color.cr_text_2)) {
                sendAction(toggleAction); refreshDashboard()
            })
            if (connectedPeers.isNotEmpty()) {
                add(Triple("Disconnect all", cr(R.color.cr_amber)) {
                    sendAction(ClipRelayService.ACTION_DISCONNECT_ALL); refreshDashboard()
                })
            }
            add(Triple("Stop service", cr(R.color.cr_red)) {
                stopService(Intent(this@MainActivity, ClipRelayService::class.java))
                refreshDashboard()
            })
        }

        items.forEachIndexed { i, (label, color, action) ->
            if (i > 0) secondaryActionsContainer.addView(View(this).apply {
                setBackgroundColor(cr(R.color.cr_divider))
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, dp(1))
            })
            secondaryActionsContainer.addView(actionRow(label, color, action))
        }
    }

    private fun refreshFlowPreview() {
        flowPreviewRows.removeAllViews()
        val preview = ClipRelayService.getFeedSnapshot().take(4)

        if (preview.isEmpty()) {
            flowPreviewRows.addView(TextView(this).apply {
                text = "Nothing has moved across your devices yet. Clipboard drops, files, and reconnects will appear here."
                textSize = 13f
                setTextColor(cr(R.color.cr_text_3))
                setLineSpacing(0f, 1.4f)
            })
            return
        }

        preview.forEachIndexed { index, entry ->
            if (index > 0) {
                flowPreviewRows.addView(View(this).apply {
                    setBackgroundColor(cr(R.color.cr_divider))
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        dp(1)
                    ).also { it.setMargins(dp(38), dp(10), 0, dp(10)) }
                })
            }
            flowPreviewRows.addView(flowPreviewRow(entry))
        }
    }

    private fun flowPreviewRow(entry: ActivityEntry): LinearLayout {
        val accent = when (entry.kind) {
            ActivityKind.CLIPBOARD_TEXT -> cr(R.color.cr_k_text_fg)
            ActivityKind.CLIPBOARD_IMAGE -> cr(R.color.cr_k_img_fg)
            ActivityKind.FILE_SENT, ActivityKind.FILE_RECEIVED,
            ActivityKind.FILE_TRANSFER_INCOMING, ActivityKind.FILE_TRANSFER_PROGRESS,
            ActivityKind.FILE_TRANSFER_COMPLETE, ActivityKind.FILE_TRANSFER_FAILED -> cr(R.color.cr_k_file_fg)
            ActivityKind.PEER_CONNECTED, ActivityKind.PEER_DISCONNECTED -> cr(R.color.cr_k_peer_fg)
            ActivityKind.WARNING -> cr(R.color.cr_k_warn_fg)
        }

        val chip = when (entry.kind) {
            ActivityKind.CLIPBOARD_TEXT, ActivityKind.CLIPBOARD_IMAGE -> "Clipboard"
            ActivityKind.FILE_SENT, ActivityKind.FILE_RECEIVED,
            ActivityKind.FILE_TRANSFER_INCOMING, ActivityKind.FILE_TRANSFER_PROGRESS,
            ActivityKind.FILE_TRANSFER_COMPLETE, ActivityKind.FILE_TRANSFER_FAILED -> "Files"
            ActivityKind.PEER_CONNECTED, ActivityKind.PEER_DISCONNECTED -> "Peers"
            ActivityKind.WARNING -> "Alert"
        }

        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.TOP

            addView(View(this@MainActivity).apply {
                background = GradientDrawable().also {
                    it.shape = GradientDrawable.OVAL
                    it.setColor(accent)
                }
                layoutParams = LinearLayout.LayoutParams(dp(9), dp(9)).also {
                    it.topMargin = dp(7)
                    it.rightMargin = dp(12)
                }
            })

            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)

                addView(TextView(this@MainActivity).apply {
                    text = flowHeadline(entry.preview)
                    textSize = 14f
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_text_1))
                    maxLines = 2
                    ellipsize = TextUtils.TruncateAt.END
                })
                addView(vSpace(3))
                addView(TextView(this@MainActivity).apply {
                    text = "$chip · ${entry.deviceName} · ${relativeTime(entry.timestamp)}"
                    textSize = 12f
                    setTextColor(cr(R.color.cr_text_3))
                })
            })
        }
    }

    private fun flowHeadline(raw: String): String {
        val trimmed = raw.trim()
        if (trimmed.isEmpty()) return raw
        return trimmed.replaceFirstChar { ch ->
            if (ch.isLowerCase()) ch.titlecase() else ch.toString()
        }
    }

    // Inline action row: label left, → right
    private fun actionRow(label: String, labelColor: Int, onClick: () -> Unit): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            isClickable = true; isFocusable = true
            background = ripple(cr(R.color.cr_ripple))
            setPadding(0, dp(13), 0, dp(13))
            setOnClickListener { onClick() }
            installPressFeedback()

            addView(TextView(this@MainActivity).apply {
                text = label
                textSize = 14.5f
                setTypeface(Typeface.create("sans-serif", Typeface.NORMAL))
                setTextColor(labelColor)
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(buildChevron())
        }

    // ── Feed pane ─────────────────────────────────────────────────────────────

    private fun buildFeedPane(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setBackgroundColor(cr(R.color.cr_bg))

        // App bar (matches dashboard)
        addView(LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(cr(R.color.cr_bg_card))
            setPadding(dp(20), dp(16), dp(20), dp(14))

            addView(TextView(this@MainActivity).apply {
                text = "Activity"
                textSize = 20f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_1))
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
        })

        // Search bar — rounded rect input field with icon
        addView(LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(cr(R.color.cr_bg_card))
            setPadding(dp(16), dp(8), dp(16), dp(12))

            val inputBg = GradientDrawable().also {
                it.cornerRadius = dp(12).toFloat()
                it.setColor(cr(R.color.cr_bg_input))
            }
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                background = inputBg
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT)
                setPadding(dp(12), dp(10), dp(12), dp(10))

                addView(TextView(this@MainActivity).apply {
                    text = "⌕"
                    textSize = 16f
                    setTextColor(cr(R.color.cr_text_3))
                    setPadding(0, 0, dp(8), 0)
                })
                searchInput = EditText(this@MainActivity).apply {
                    hint = "Search activity…"
                    textSize = 14f
                    setTextColor(cr(R.color.cr_text_1))
                    setHintTextColor(cr(R.color.cr_text_4))
                    setBackgroundColor(Color.TRANSPARENT)
                    layoutParams = LinearLayout.LayoutParams(0,
                        LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                    addTextChangedListener(object : TextWatcher {
                        override fun afterTextChanged(s: Editable?) {
                            searchQuery = s?.toString().orEmpty()
                            rebuildFeed()
                        }
                        override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
                        override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
                    })
                }
                addView(searchInput)
            })
        })

        // Filter chips
        addView(HorizontalScrollView(this@MainActivity).apply {
            isHorizontalScrollBarEnabled = false
            setBackgroundColor(cr(R.color.cr_bg_card))
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.HORIZONTAL
                setPadding(dp(16), 0, dp(16), dp(14))

                chipAll   = filterChip("All",       true)  { applyFilter("all") }
                chipClip  = filterChip("Clipboard", false) { applyFilter("clipboard") }
                chipFiles = filterChip("Files",     false) { applyFilter("files") }
                chipPeers = filterChip("Peers",     false) { applyFilter("peers") }

                addView(chipAll);                addView(hSpace(6))
                addView(chipClip);               addView(hSpace(6))
                addView(chipFiles);              addView(hSpace(6))
                addView(chipPeers)
            })
        })

        addView(View(this@MainActivity).apply {
            setBackgroundColor(cr(R.color.cr_border))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, dp(1))
        })

        // Empty state
        feedEmptyState = LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(dp(40), 0, dp(40), dp(60))

            addView(TextView(this@MainActivity).apply {
                text = "↔"
                textSize = 40f
                gravity = Gravity.CENTER
                setTextColor(cr(R.color.cr_text_4))
            })
            addView(vSpace(12))
            addView(TextView(this@MainActivity).apply {
                text = "No activity yet"
                textSize = 17f
                gravity = Gravity.CENTER
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_2))
            })
            addView(vSpace(6))
            addView(TextView(this@MainActivity).apply {
                text = "Clipboard syncs, file transfers,\nand device events appear here."
                textSize = 14f
                gravity = Gravity.CENTER
                setTextColor(cr(R.color.cr_text_3))
                setLineSpacing(0f, 1.45f)
            })
        }

        feedContainer = LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.VERTICAL
        }
        feedScroller = ScrollView(this@MainActivity).apply {
            isFillViewport = true
            addView(feedContainer)
        }

        // Pull-to-refresh: wraps the feed scroll view.
        // On refresh: triggers a manual service status broadcast and rebuilds the feed.
        val swipeRefresh = androidx.swiperefreshlayout.widget.SwipeRefreshLayout(this@MainActivity).apply {
            setColorSchemeColors(cr(R.color.cr_accent))
            setProgressBackgroundColorSchemeColor(cr(R.color.cr_bg_card))
            addView(feedScroller, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.MATCH_PARENT))
            setOnRefreshListener {
                sendBroadcast(Intent(ClipRelayService.ACTION_STATUS_CHANGED)
                    .setPackage(packageName))
                rebuildFeed()
                isRefreshing = false
            }
        }

        addView(feedEmptyState, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        addView(swipeRefresh, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        feedEmptyState.visibility = View.GONE
    }

    private fun applyFilter(kind: String) {
        activeFilter = kind
        mapOf("all" to chipAll, "clipboard" to chipClip,
              "files" to chipFiles, "peers" to chipPeers)
            .forEach { (k, chip) -> styleChip(chip, k == kind) }
        rebuildFeed()
    }

    private fun styleChip(chip: TextView, active: Boolean) {
        chip.setTextColor(if (active) cr(R.color.cr_accent) else cr(R.color.cr_text_2))
        chip.setTypeface(chip.typeface, if (active) Typeface.BOLD else Typeface.NORMAL)
        chip.background = GradientDrawable().also {
            it.cornerRadius = dp(20).toFloat()
            it.setColor(if (active) cr(R.color.cr_accent_bg) else cr(R.color.cr_bg_inset))
            if (active) it.setStroke(dp(1), alphaBlend(cr(R.color.cr_accent), 0.28f))
        }
    }

    // ── Feed rebuild ──────────────────────────────────────────────────────────

    private fun rebuildFeed() {
        val all = ClipRelayService.getFeedSnapshot()

        val filtered = all.filter { entry ->
            val kindOk = when (activeFilter) {
                "clipboard" -> entry.kind == ActivityKind.CLIPBOARD_TEXT ||
                               entry.kind == ActivityKind.CLIPBOARD_IMAGE
                "files"     -> entry.kind in listOf(
                    ActivityKind.FILE_SENT, ActivityKind.FILE_RECEIVED,
                    ActivityKind.FILE_TRANSFER_INCOMING, ActivityKind.FILE_TRANSFER_PROGRESS,
                    ActivityKind.FILE_TRANSFER_COMPLETE, ActivityKind.FILE_TRANSFER_FAILED)
                "peers"     -> entry.kind == ActivityKind.PEER_CONNECTED ||
                               entry.kind == ActivityKind.PEER_DISCONNECTED
                else        -> true
            }
            val searchOk = searchQuery.isBlank() ||
                entry.preview.contains(searchQuery, ignoreCase = true) ||
                entry.deviceName.contains(searchQuery, ignoreCase = true)
            kindOk && searchOk
        }

        feedContainer.removeAllViews()

        if (filtered.isEmpty()) {
            feedEmptyState.visibility = View.VISIBLE
            feedScroller.visibility   = View.GONE
            return
        }
        feedEmptyState.visibility = View.GONE
        feedScroller.visibility   = View.VISIBLE

        val cal = java.util.Calendar.getInstance()
        val todayKey = calKey(cal)
        cal.add(java.util.Calendar.DAY_OF_YEAR, -1)
        val yestKey = calKey(cal)

        var lastKey = ""
        filtered.forEach { entry ->
            cal.timeInMillis = entry.timestamp
            val key = calKey(cal)
            if (key != lastKey) {
                lastKey = key
                feedContainer.addView(buildDayHeader(when (key) {
                    todayKey -> "Today"
                    yestKey  -> "Yesterday"
                    else     -> java.text.SimpleDateFormat(
                        "EEEE, MMMM d", java.util.Locale.getDefault()
                    ).format(java.util.Date(entry.timestamp))
                }))
            }
            feedContainer.addView(buildFeedRow(entry))
        }
    }

    private fun buildDayHeader(label: String): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setBackgroundColor(cr(R.color.cr_bg))
        setPadding(dp(16), dp(18), dp(16), dp(8))

        addView(TextView(this@MainActivity).apply {
            text = label
            textSize = 11.5f
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_text_3))
            letterSpacing = 0.02f
        })
        addView(hSpace(10))
        addView(View(this@MainActivity).apply {
            setBackgroundColor(cr(R.color.cr_border))
            layoutParams = LinearLayout.LayoutParams(0, dp(1), 1f)
        })
    }

    private fun buildFeedRow(entry: ActivityEntry): LinearLayout {
        // Per-row expand state — false by default, toggled by tap on preview block.
        var isExpanded = false

        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(cr(R.color.cr_bg_card))
            isClickable = true; isFocusable = true
            background = ripple(cr(R.color.cr_ripple), cr(R.color.cr_bg_card))

            // ── Long-press: context menu ──────────────────────────────────────
            setOnLongClickListener {
                val items = buildList {
                    if (entry.preview.isNotBlank()) {
                        add("Copy text" to {
                            val cm = getSystemService(ClipboardManager::class.java)
                            cm.setPrimaryClip(ClipData.newPlainText("ClipRelay", entry.preview))
                            showSnack("Copied to clipboard")
                        })
                        if (entry.preview.length > 5) {
                            add("Share…" to {
                                startActivity(Intent.createChooser(
                                    Intent(Intent.ACTION_SEND).apply {
                                        type = "text/plain"
                                        putExtra(Intent.EXTRA_TEXT, entry.preview)
                                    }, null))
                            })
                        }
                    }
                    if (entry.isApplicable) {
                        add("Apply to clipboard" to {
                            val svc = Intent(this@MainActivity, ClipRelayService::class.java).apply {
                                action = ClipRelayService.ACTION_APPLY_CLIPBOARD
                                if (entry.contentHash.isNotBlank())
                                    putExtra(ClipRelayService.EXTRA_CONTENT_HASH, entry.contentHash)
                                putExtra(ClipRelayService.EXTRA_CLIPBOARD_TEXT, entry.preview)
                            }
                            ContextCompat.startForegroundService(this@MainActivity, svc)
                            rebuildFeed()
                        })
                    }
                }
                if (items.isEmpty()) return@setOnLongClickListener false
                val labels = items.map { it.first }.toTypedArray()
                com.google.android.material.dialog.MaterialAlertDialogBuilder(this@MainActivity)
                    .setItems(labels) { _, i -> items[i].second.invoke() }
                    .show()
                true
            }

            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.TOP
                setPadding(dp(16), dp(14), dp(16), dp(14))

                // Kind badge
                addView(buildKindBadge(entry.kind))
                addView(hSpace(12))

                // Content block
                addView(LinearLayout(this@MainActivity).apply {
                    orientation = LinearLayout.VERTICAL
                    layoutParams = LinearLayout.LayoutParams(0,
                        LinearLayout.LayoutParams.WRAP_CONTENT, 1f)

                    // Header: device name · time
                    addView(LinearLayout(this@MainActivity).apply {
                        orientation = LinearLayout.HORIZONTAL
                        gravity = Gravity.CENTER_VERTICAL
                        addView(TextView(this@MainActivity).apply {
                            text = entry.deviceName
                            textSize = 13.5f
                            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                            setTextColor(cr(R.color.cr_text_1))
                        })
                        addView(TextView(this@MainActivity).apply {
                            text = "  ·  ${relativeTime(entry.timestamp)}"
                            textSize = 12f
                            setTextColor(cr(R.color.cr_text_3))
                        })
                    })

                    addView(vSpace(3))

                    // Summary
                    addView(TextView(this@MainActivity).apply {
                        text = entry.formattedLine()
                        textSize = 13.5f
                        setTextColor(cr(R.color.cr_text_2))
                        maxLines = 2
                        ellipsize = TextUtils.TruncateAt.END
                        setLineSpacing(0f, 1.3f)
                    })

                    // Clipboard text preview — tap to expand up to full 400 chars
                    if (entry.kind == ActivityKind.CLIPBOARD_TEXT && entry.preview.isNotBlank()) {
                        addView(vSpace(7))
                        val previewFull  = entry.preview            // up to 400 chars from engine
                        val previewShort = previewFull.take(140)
                        val hasMore      = previewFull.length > 140

                        val previewText = TextView(this@MainActivity).apply {
                            text = previewShort
                            textSize = 12.5f
                            setTypeface(Typeface.MONOSPACE, Typeface.NORMAL)
                            setTextColor(cr(R.color.cr_text_3))
                            maxLines = 3
                            ellipsize = if (hasMore) TextUtils.TruncateAt.END else null
                            setLineSpacing(0f, 1.4f)
                            setPadding(dp(11), dp(8), dp(11), dp(8))
                            background = GradientDrawable().also {
                                it.cornerRadius = dp(8).toFloat()
                                it.setColor(cr(R.color.cr_bg_inset))
                            }
                        }
                        val expandHint = if (hasMore) TextView(this@MainActivity).apply {
                            text = "Tap to expand"
                            textSize = 11f
                            setTextColor(cr(R.color.cr_text_4))
                            setPadding(dp(11), 0, 0, 0)
                        } else null

                        previewText.setOnClickListener {
                            isExpanded = !isExpanded
                            if (isExpanded) {
                                previewText.text    = previewFull
                                previewText.maxLines = Int.MAX_VALUE
                                previewText.ellipsize = null
                                expandHint?.text = "Tap to collapse"
                            } else {
                                previewText.text    = previewShort
                                previewText.maxLines = 3
                                previewText.ellipsize = if (hasMore) TextUtils.TruncateAt.END else null
                                expandHint?.text = "Tap to expand"
                            }
                        }
                        addView(previewText)
                        expandHint?.let { addView(vSpace(3)); addView(it) }
                    }

                    // Transfer progress
                    if (entry.kind == ActivityKind.FILE_TRANSFER_PROGRESS &&
                        entry.progressPercent in 0..99) {
                        addView(vSpace(9))
                        addView(buildProgressStrip(entry.progressPercent / 100.0))
                        addView(vSpace(3))
                        addView(TextView(this@MainActivity).apply {
                            text = formatTransferProgress(entry)
                            textSize = 12f
                            setTextColor(cr(R.color.cr_text_3))
                            setLineSpacing(0f, 1.2f)
                        })
                    }

                    // Kind tag + apply button
                    addView(vSpace(9))
                    addView(LinearLayout(this@MainActivity).apply {
                        orientation = LinearLayout.HORIZONTAL
                        gravity = Gravity.CENTER_VERTICAL
                        addView(kindLabel(entry.kind))
                        if (entry.isApplicable) {
                            addView(hSpace(8))
                            addView(buildApplyButton(entry))
                        }
                    })
                })
            })

            // Row separator
            addView(View(this@MainActivity).apply {
                setBackgroundColor(cr(R.color.cr_divider))
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, dp(1)
                ).also { it.setMargins(dp(68), 0, 0, 0) }
            })
        }
    }

    // Kind badge: square-rounded tile with letter
    private fun buildKindBadge(kind: ActivityKind): FrameLayout {
        val (letter, fg, bg) = when (kind) {
            ActivityKind.CLIPBOARD_TEXT              -> Triple("T", cr(R.color.cr_k_text_fg),  cr(R.color.cr_k_text_bg))
            ActivityKind.CLIPBOARD_IMAGE             -> Triple("I", cr(R.color.cr_k_img_fg),   cr(R.color.cr_k_img_bg))
            ActivityKind.FILE_SENT,
            ActivityKind.FILE_RECEIVED,
            ActivityKind.FILE_TRANSFER_INCOMING,
            ActivityKind.FILE_TRANSFER_PROGRESS      -> Triple("F", cr(R.color.cr_k_file_fg),  cr(R.color.cr_k_file_bg))
            ActivityKind.FILE_TRANSFER_COMPLETE      -> Triple("✓", cr(R.color.cr_green),      cr(R.color.cr_green_bg))
            ActivityKind.FILE_TRANSFER_FAILED        -> Triple("✗", cr(R.color.cr_red),        cr(R.color.cr_red_bg))
            ActivityKind.PEER_CONNECTED              -> Triple("P", cr(R.color.cr_k_peer_fg),  cr(R.color.cr_k_peer_bg))
            ActivityKind.PEER_DISCONNECTED           -> Triple("P", cr(R.color.cr_text_3),     cr(R.color.cr_bg_inset))
            ActivityKind.WARNING                     -> Triple("!", cr(R.color.cr_k_warn_fg),  cr(R.color.cr_k_warn_bg))
        }
        val size = dp(42)
        return FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(size, size)
            background = GradientDrawable().also {
                it.cornerRadius = dp(13).toFloat()
                it.setColor(bg)
            }
            addView(TextView(this@MainActivity).apply {
                text = letter
                textSize = 16f
                gravity = Gravity.CENTER
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(fg)
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT)
            })
        }
    }

    private fun kindLabel(kind: ActivityKind): TextView {
        val (label, fg, bg) = when (kind) {
            ActivityKind.CLIPBOARD_TEXT           -> Triple("Text",         cr(R.color.cr_k_text_fg), cr(R.color.cr_k_text_bg))
            ActivityKind.CLIPBOARD_IMAGE          -> Triple("Image",        cr(R.color.cr_k_img_fg),  cr(R.color.cr_k_img_bg))
            ActivityKind.FILE_SENT                -> Triple("Sent",         cr(R.color.cr_k_file_fg), cr(R.color.cr_k_file_bg))
            ActivityKind.FILE_RECEIVED            -> Triple("Received",     cr(R.color.cr_k_file_fg), cr(R.color.cr_k_file_bg))
            ActivityKind.FILE_TRANSFER_INCOMING   -> Triple("Incoming",     cr(R.color.cr_k_file_fg), cr(R.color.cr_k_file_bg))
            ActivityKind.FILE_TRANSFER_PROGRESS   -> Triple("Transferring", cr(R.color.cr_k_file_fg), cr(R.color.cr_k_file_bg))
            ActivityKind.FILE_TRANSFER_COMPLETE   -> Triple("Complete",     cr(R.color.cr_green),     cr(R.color.cr_green_bg))
            ActivityKind.FILE_TRANSFER_FAILED     -> Triple("Failed",       cr(R.color.cr_red),       cr(R.color.cr_red_bg))
            ActivityKind.PEER_CONNECTED           -> Triple("Connected",    cr(R.color.cr_k_peer_fg), cr(R.color.cr_k_peer_bg))
            ActivityKind.PEER_DISCONNECTED        -> Triple("Disconnected", cr(R.color.cr_text_3),    cr(R.color.cr_bg_inset))
            ActivityKind.WARNING                  -> Triple("Warning",      cr(R.color.cr_k_warn_fg), cr(R.color.cr_k_warn_bg))
        }
        return TextView(this).apply {
            text = label
            textSize = 11f
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(fg)
            setPadding(dp(8), dp(3), dp(8), dp(3))
            background = GradientDrawable().also {
                it.cornerRadius = dp(20).toFloat(); it.setColor(bg)
            }
        }
    }

    private fun buildApplyButton(entry: ActivityEntry): TextView = TextView(this).apply {
        text = "Apply"
        textSize = 11.5f
        setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
        setTextColor(cr(R.color.cr_accent))
        setPadding(dp(10), dp(3), dp(10), dp(3))
        isClickable = true; isFocusable = true
        background = GradientDrawable().also {
            it.cornerRadius = dp(20).toFloat()
            it.setColor(cr(R.color.cr_accent_bg))
            it.setStroke(dp(1), alphaBlend(cr(R.color.cr_accent), 0.22f))
        }
        setOnClickListener {
            // Pass contentHash so the service can call applyClipboardByHash() which
            // retrieves the full content from the engine — entry.preview is only 60 chars.
            val svc = Intent(this@MainActivity, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_APPLY_CLIPBOARD
                if (entry.contentHash.isNotBlank()) {
                    putExtra(ClipRelayService.EXTRA_CONTENT_HASH, entry.contentHash)
                }
                // Include preview as fallback if engine was restarted and hash is gone.
                putExtra(ClipRelayService.EXTRA_CLIPBOARD_TEXT, entry.preview)
            }
            ContextCompat.startForegroundService(this@MainActivity, svc)
            showSnack("Applied to clipboard")
            rebuildFeed()
        }
        installPressFeedback()
    }

    private fun buildProgressStrip(fraction: Double): FrameLayout {
        val h = dp(4)
        return FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, h)
            background = GradientDrawable().also {
                it.cornerRadius = dp(2).toFloat()
                it.setColor(cr(R.color.cr_bg_inset))
            }
            val fill = View(this@MainActivity).apply {
                background = GradientDrawable().also {
                    it.cornerRadius = dp(2).toFloat()
                    it.setColor(cr(R.color.cr_accent))
                }
            }
            addView(fill, FrameLayout.LayoutParams(0, h))
            viewTreeObserver.addOnGlobalLayoutListener(
                object : android.view.ViewTreeObserver.OnGlobalLayoutListener {
                    override fun onGlobalLayout() {
                        fill.layoutParams = FrameLayout.LayoutParams(
                            (width * fraction.coerceIn(0.0, 1.0)).toInt(), h)
                        viewTreeObserver.removeOnGlobalLayoutListener(this)
                    }
                })
        }
    }

    // ── Component primitives ──────────────────────────────────────────────────

    private fun card(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        background = GradientDrawable().also {
            it.cornerRadius = dp(18).toFloat()
            it.setColor(cr(R.color.cr_bg_card))
            it.setStroke(dp(1), cr(R.color.cr_border))
        }
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT)
        setPadding(dp(18), dp(18), dp(18), dp(18))
    }

    private fun sectionEyebrow(text: String): TextView = TextView(this).apply {
        this.text = text.uppercase()
        textSize  = 10.5f
        letterSpacing = 0.08f
        setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
        setTextColor(cr(R.color.cr_text_3))
    }

    private fun infoTag(label: String): TextView = TextView(this).apply {
        text = label
        textSize = 11f
        setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
        setTextColor(cr(R.color.cr_text_3))
        setPadding(dp(9), dp(4), dp(9), dp(4))
        background = GradientDrawable().also {
            it.cornerRadius = dp(20).toFloat()
            it.setColor(cr(R.color.cr_bg_inset))
            it.setStroke(dp(1), cr(R.color.cr_border))
        }
    }

    private fun filterChip(label: String, active: Boolean, onClick: () -> Unit): TextView =
        TextView(this).apply {
            text = label
            textSize = 13f
            setTypeface(if (active) Typeface.create("sans-serif-medium", Typeface.NORMAL)
                        else Typeface.create("sans-serif", Typeface.NORMAL))
            setTextColor(if (active) cr(R.color.cr_accent) else cr(R.color.cr_text_2))
            setPadding(dp(14), dp(7), dp(14), dp(7))
            background = GradientDrawable().also {
                it.cornerRadius = dp(20).toFloat()
                it.setColor(if (active) cr(R.color.cr_accent_bg) else cr(R.color.cr_bg_inset))
                if (active) it.setStroke(dp(1), alphaBlend(cr(R.color.cr_accent), 0.25f))
            }
            isClickable = true; isFocusable = true
            setOnClickListener { onClick() }
            installPressFeedback()
        }

    private fun vSpace(size: Int) = Space(this).apply {
        layoutParams = LinearLayout.LayoutParams(1, dp(size)) }
    private fun hSpace(size: Int) = Space(this).apply {
        layoutParams = LinearLayout.LayoutParams(dp(size), 1) }
    private fun vGap(h: Int) = View(this).apply {
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, dp(h)) }

    private fun ripple(rippleColor: Int, content: android.graphics.drawable.Drawable? = null)
        : android.graphics.drawable.Drawable =
        RippleDrawable(android.content.res.ColorStateList.valueOf(rippleColor), content, null)

    private fun ripple(rippleColor: Int, bgColor: Int): android.graphics.drawable.Drawable =
        RippleDrawable(android.content.res.ColorStateList.valueOf(rippleColor),
            ColorDrawable(bgColor), null)

    private fun View.installPressFeedback() {
        stateListAnimator = null
        isHapticFeedbackEnabled = true
        val slop = ViewConfiguration.get(context).scaledTouchSlop
        var downX = 0f
        var downY = 0f
        var hapticEligible = false
        setOnTouchListener { v, event ->
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    downX = event.x
                    downY = event.y
                    hapticEligible = true
                    v.animate().cancel()
                    v.animate().scaleX(0.97f).scaleY(0.97f).alpha(0.88f).setDuration(70).start()
                }
                MotionEvent.ACTION_MOVE -> {
                    if (hapticEligible &&
                        (kotlin.math.abs(event.x - downX) > slop || kotlin.math.abs(event.y - downY) > slop)
                    ) {
                        hapticEligible = false
                        v.animate().cancel()
                        v.animate().scaleX(1f).scaleY(1f).alpha(1f).setDuration(120).start()
                    }
                }
                MotionEvent.ACTION_UP -> {
                    if (hapticEligible &&
                        event.x >= 0f && event.x <= v.width &&
                        event.y >= 0f && event.y <= v.height
                    ) {
                        v.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
                    }
                    hapticEligible = false
                    v.animate().cancel()
                    v.animate().scaleX(1f).scaleY(1f).alpha(1f).setDuration(120).start()
                }
                MotionEvent.ACTION_CANCEL -> {
                    hapticEligible = false
                    v.animate().cancel()
                    v.animate().scaleX(1f).scaleY(1f).alpha(1f).setDuration(120).start()
                }
            }
            false
        }
    }

    private fun alphaBlend(color: Int, alpha: Float): Int {
        val a = (255 * alpha).toInt().coerceIn(0, 255)
        return Color.argb(a, Color.red(color), Color.green(color), Color.blue(color))
    }

    private fun buildChevron(): AppCompatImageView =
        AppCompatImageView(this).apply {
            setImageResource(R.drawable.ic_cr_chevron_right)
            imageTintList = android.content.res.ColorStateList.valueOf(cr(R.color.cr_text_4))
            layoutParams = LinearLayout.LayoutParams(dp(18), dp(18))
        }

    private fun formatTransferProgress(entry: ActivityEntry): String {
        val parts = mutableListOf<String>()
        parts += "${entry.progressPercent}% transferred"
        if (entry.transferBytesReceived > 0L && entry.fileTotalBytes > 0L) {
            parts += "${formatBytes(entry.transferBytesReceived)} of ${formatBytes(entry.fileTotalBytes)}"
        }
        if (entry.transferSpeedBps > 0L) {
            parts += "${formatBytes(entry.transferSpeedBps)}/s"
        }
        if (entry.transferEtaSecs >= 0L) {
            parts += "ETA ${formatEta(entry.transferEtaSecs)}"
        }
        return parts.joinToString("  ·  ")
    }

    private fun formatBytes(bytes: Long): String = when {
        bytes >= 1_048_576L -> "%.1f MB".format(bytes / 1_048_576.0)
        bytes >= 1_024L -> "%.0f KB".format(bytes / 1_024.0)
        else -> "$bytes B"
    }

    private fun formatEta(seconds: Long): String = when {
        seconds < 60L -> "${seconds}s"
        seconds < 3_600L -> "${seconds / 60}m"
        else -> "${seconds / 3_600}h"
    }

    private fun relativeTime(ms: Long): String {
        val d = System.currentTimeMillis() - ms
        return when {
            d < 10_000L     -> "just now"
            d < 60_000L     -> "${d / 1_000}s ago"
            d < 3_600_000L  -> "${d / 60_000}m ago"
            d < 86_400_000L -> "${d / 3_600_000}h ago"
            else            -> "${d / 86_400_000}d ago"
        }
    }

    private fun calKey(cal: java.util.Calendar) =
        "${cal.get(java.util.Calendar.YEAR)}-${cal.get(java.util.Calendar.DAY_OF_YEAR)}"

    private fun prefs() = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)

    /** Brief non-intrusive feedback shown at the bottom of the screen. */
    private fun showSnack(message: String) =
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()

    private fun launchService() = runCatching {
        ContextCompat.startForegroundService(this,
            Intent(this, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_START })
    }

    private fun sendAction(action: String) = runCatching {
        ContextCompat.startForegroundService(this,
            Intent(this, ClipRelayService::class.java).apply { this.action = action })
    }

    private fun requestNotificationPermission() {
        val needed = mutableListOf<String>()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.POST_NOTIFICATIONS
        }

        // READ_PHONE_STATE is required for call state monitoring (API 23+).
        if (checkSelfPermission(Manifest.permission.READ_PHONE_STATE) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.READ_PHONE_STATE
        }

        // READ_CONTACTS is used to resolve caller names from the address book.
        if (checkSelfPermission(Manifest.permission.READ_CONTACTS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.READ_CONTACTS
        }

        // ANSWER_PHONE_CALLS is required to accept/decline calls from the Mac.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            checkSelfPermission(Manifest.permission.ANSWER_PHONE_CALLS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.ANSWER_PHONE_CALLS
        }

        if (checkSelfPermission(Manifest.permission.READ_CALL_LOG) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            needed += Manifest.permission.READ_CALL_LOG
        }

        if (needed.isNotEmpty()) {
            requestPermissions(needed.toTypedArray(), 1001)
        }

        // OnePlus/OxygenOS will kill sockets during sleep unless explicitly whitelisted
        val pm = getSystemService(android.os.PowerManager::class.java)
        if (pm != null && !pm.isIgnoringBatteryOptimizations(packageName)) {
            runCatching {
                startActivity(android.content.Intent(
                    android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                    android.net.Uri.parse("package:$packageName")
                ))
            }
        }
    }

    // When permissions are granted at runtime, tell the service to (re-)start the call monitor.
    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == 1001) {
            val readPhone = permissions.indexOf(Manifest.permission.READ_PHONE_STATE)
            if (readPhone >= 0 &&
                grantResults[readPhone] == android.content.pm.PackageManager.PERMISSION_GRANTED) {
                // Restart the service so it can start the call monitor now that permission is granted.
                startService(Intent(this, ClipRelayService::class.java))
            }
        }
    }

    private fun showMagicLinkPairingDialog() {
        val ctx = this
        val dialogView = LinearLayout(ctx).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(20), dp(16), dp(20), dp(16))
            val lp = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
            layoutParams = lp
        }

        val title = TextView(ctx).apply {
            text = "Magic Link Pairing"
            textSize = 18f
            setTypeface(null, Typeface.BOLD)
            setTextColor(cr(R.color.cr_text_1))
            setPadding(0, 0, 0, dp(12))
        }
        dialogView.addView(title)

        val desc = TextView(ctx).apply {
            text = "Scan the QR code on your Mac's screen, paste the pairing link, or enter its IP address directly."
            textSize = 13f
            setTextColor(cr(R.color.cr_text_3))
            setPadding(0, 0, 0, dp(16))
        }
        dialogView.addView(desc)

        val scanBtn = AppCompatButton(ctx).apply {
            text = "📷 Scan QR Code"
            textSize = 15f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_on_accent))
            background = GradientDrawable().also {
                it.cornerRadius = dp(12).toFloat()
                it.setColor(cr(R.color.cr_accent))
            }
            setPadding(dp(16), dp(12), dp(16), dp(12))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                setMargins(0, 0, 0, dp(16))
            }
            installPressFeedback()
        }
        dialogView.addView(scanBtn)

        val orDivider = TextView(ctx).apply {
            text = "— OR ENTER MANUALLY —"
            textSize = 11f
            gravity = Gravity.CENTER
            setTypeface(null, Typeface.BOLD)
            setTextColor(cr(R.color.cr_text_3))
            setPadding(0, 0, 0, dp(16))
        }
        dialogView.addView(orDivider)

        val inputField = EditText(ctx).apply {
            hint = "cliprelay://pair?name=... or 192.168.1.10"
            textSize = 14f
            setTextColor(cr(R.color.cr_text_1))
            setHintTextColor(cr(R.color.cr_text_3))
            setPadding(dp(12), dp(10), dp(12), dp(10))
            isSingleLine = true
        }
        dialogView.addView(inputField)

        val dialog = android.app.AlertDialog.Builder(ctx)
            .setView(dialogView)
            .setPositiveButton("Connect", null)
            .setNegativeButton("Cancel") { d, _ -> d.dismiss() }
            .create()

        scanBtn.setOnClickListener {
            dialog.dismiss()
            startQrScanner()
        }

        dialog.setOnShowListener {
            val connectButton = dialog.getButton(android.app.AlertDialog.BUTTON_POSITIVE)
            connectButton.setOnClickListener {
                val input = inputField.text.toString().trim()
                if (input.isBlank()) {
                    inputField.error = "Input cannot be empty"
                    return@setOnClickListener
                }

                if (handlePairingInput(input)) {
                    dialog.dismiss()
                } else {
                    inputField.error = "Invalid IP address or pairing link"
                }
            }
        }

        dialog.show()
    }

    private fun startQrScanner() {
        val options = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .enableAutoZoom()
            .build()
        val scanner = GmsBarcodeScanning.getClient(this, options)
        scanner.startScan()
            .addOnSuccessListener { barcode: Barcode ->
                val rawValue = barcode.rawValue
                if (!rawValue.isNullOrBlank()) {
                    if (handlePairingInput(rawValue)) {
                        showSnack("QR scanned successfully! Connecting...")
                    } else {
                        showSnack("Invalid QR code format")
                    }
                } else {
                    showSnack("No QR code found")
                }
            }
            .addOnFailureListener { e: java.lang.Exception ->
                showSnack("QR Scan failed: ${e.message}")
            }
    }

    private fun handlePairingInput(input: String): Boolean {
        val cleaned = input.trim()
        val ctx = this
        if (cleaned.startsWith("cliprelay://pair")) {
            val uri = android.net.Uri.parse(cleaned)
            val ip = uri.getQueryParameter("ip")
            val port = uri.getQueryParameter("port")?.toIntOrNull() ?: 47823
            val peerName = uri.getQueryParameter("name")?.let {
                java.net.URLDecoder.decode(it, "UTF-8")
            } ?: ip ?: "Mac"
            val fingerprint = uri.getQueryParameter("fingerprint") ?: ""
            if (ip != null) {
                // Step 1: Connect to the Mac
                ContextCompat.startForegroundService(ctx,
                    Intent(ctx, ClipRelayService::class.java).apply {
                        action = ClipRelayService.ACTION_CONNECT_MANUAL
                        putExtra("ip", ip)
                        putExtra("port", port)
                    }
                )
                // Step 2: Auto-trust after a brief delay to let the connection establish.
                // The QR code was generated by the Mac and scanned physically, so trust is implicit.
                if (fingerprint.isNotBlank()) {
                    android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                        // Find the newly connected peer and trust it
                        autoTrustNewPeer(ip, peerName)
                    }, 2000) // 2s delay for connection to establish
                    // Also schedule a retry at 4s in case the first attempt was too early
                    android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                        autoTrustNewPeer(ip, peerName)
                    }, 4000)
                }
                showSnack("Connecting to $peerName ($ip)...")
                return true
            }
        } else {
            val parts = cleaned.split(":")
            val ip = parts[0].trim()
            val port = if (parts.size > 1) parts[1].trim().toIntOrNull() ?: 47823 else 47823
            if (ip.matches(Regex("""\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}"""))) {
                ContextCompat.startForegroundService(ctx,
                    Intent(ctx, ClipRelayService::class.java).apply {
                        action = ClipRelayService.ACTION_CONNECT_MANUAL
                        putExtra("ip", ip)
                        putExtra("port", port)
                    }
                )
                showSnack("Connecting to $ip:$port...")
                return true
            }
        }
        return false
    }

    /**
     * After QR-based pairing, find the newly discovered peer and auto-trust it.
     * Uses the same SharedPreferences + Intent action pattern as the rest of the app.
     */
    private fun autoTrustNewPeer(ip: String, peerName: String) {
        val peers = prefs().peerSnapshots()
        // Find any connected but untrusted peer — this is the one we just QR-paired with
        val untrustedConnected = peers.firstOrNull { it.isConnected && !it.trusted }
        if (untrustedConnected != null) {
            ContextCompat.startForegroundService(this,
                Intent(this, ClipRelayService::class.java).apply {
                    action = ClipRelayService.ACTION_TRUST_PEER
                    putExtra(ClipRelayService.EXTRA_TARGET_DEVICE_ID, untrustedConnected.id)
                }
            )
            android.util.Log.i("ClipRelay", "Auto-trust QR-paired peer ${untrustedConnected.id} ($peerName)")
            showSnack("✅ Paired with $peerName — syncing!")
            refreshDashboard()
        }
    }

    private fun showTrustDialog(peer: PeerSnapshot) {
        val cachedFp = prefs().getString("fingerprint_${peer.id}", "") ?: ""
        val items = ArrayList<Pair<String, () -> Unit>>()

        if (cachedFp.isNotBlank()) {
            items.add(Pair("Verify & Trust (Open Pairing Screen)") {
                val intent = Intent(this, PairingActivity::class.java).apply {
                    putExtra(PairingActivity.EXTRA_DEVICE_ID, peer.id)
                    putExtra(PairingActivity.EXTRA_DEVICE_NAME, peer.name)
                    putExtra(PairingActivity.EXTRA_FINGERPRINT, cachedFp)
                    putExtra(PairingActivity.EXTRA_PIN, pairingPin(cachedFp))
                }
                startActivity(intent)
            })
        }

        items.add(Pair("Quick Trust (Direct)") {
            ContextCompat.startForegroundService(this,
                Intent(this, ClipRelayService::class.java).apply {
                    action = ClipRelayService.ACTION_TRUST_PEER
                    putExtra(ClipRelayService.EXTRA_TARGET_DEVICE_ID, peer.id)
                }
            )
            showSnack("Trusted ${peer.name}")
        })

        if (cachedFp.isBlank()) {
            items.add(Pair("Scan & Re-authenticate") {
                sendAction(ClipRelayService.ACTION_SCAN_NOW)
                showSnack("Scanning to re-authenticate ${peer.name}...")
            })
        }

        items.add(Pair("Reject / Remove Device") {
            ContextCompat.startForegroundService(this,
                Intent(this, ClipRelayService::class.java).apply {
                    action = ClipRelayService.ACTION_REJECT_PEER
                    putExtra(ClipRelayService.EXTRA_TARGET_DEVICE_ID, peer.id)
                }
            )
            showSnack("Rejected ${peer.name}")
        })

        val labels = items.map { it.first }.toTypedArray()
        com.google.android.material.dialog.MaterialAlertDialogBuilder(this)
            .setTitle("Trust ${peer.name}")
            .setItems(labels) { _, i -> items[i].second.invoke() }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun pairingPin(fingerprint: String): String {
        val digits = fingerprint
            .filter { it.isDigit() }
            .take(6)
            .padEnd(6, '0')
        return digits.ifBlank { "000000" }
    }
}
