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
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatButton
import androidx.core.content.ContextCompat
import kotlin.math.roundToInt

// ─── Context helpers ──────────────────────────────────────────────────────────

private fun android.content.Context.dp(v: Int): Int =
    (v * resources.displayMetrics.density).roundToInt()
private fun android.content.Context.dp(v: Float): Int =
    (v * resources.displayMetrics.density).roundToInt()
private fun android.content.Context.cr(@androidx.annotation.ColorRes id: Int): Int =
    ContextCompat.getColor(this, id)

// ─── MainActivity ─────────────────────────────────────────────────────────────

class MainActivity : AppCompatActivity() {

    // ── Tab ───────────────────────────────────────────────────────────────────
    private enum class Tab { DASHBOARD, FEED }
    private var tab = Tab.DASHBOARD

    // ── Dashboard refs ────────────────────────────────────────────────────────
    private lateinit var heroStateLabel: TextView     // "ACTIVE" / "PAUSED" etc.
    private lateinit var heroHeadline: TextView       // peer names or status
    private lateinit var heroSubline: TextView        // device name
    private lateinit var heroStatusDot: View
    private lateinit var peerSection: LinearLayout
    private lateinit var peerRows: LinearLayout
    private lateinit var noPeersState: LinearLayout
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

    // ── State ─────────────────────────────────────────────────────────────────
    private var activeFilter = "all"
    private var searchQuery  = ""

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
        launchService()
        refreshDashboard()
    }

    override fun onResume() {
        super.onResume()
        refreshDashboard()
        if (tab == Tab.FEED) rebuildFeed()
    }

    override fun onStart() {
        super.onStart()
        ContextCompat.registerReceiver(
            this, statusReceiver,
            IntentFilter(ClipRelayService.ACTION_STATUS_CHANGED),
            ContextCompat.RECEIVER_NOT_EXPORTED
        )
    }

    override fun onStop() { unregisterReceiver(statusReceiver); super.onStop() }

    // ── Root ──────────────────────────────────────────────────────────────────

    private fun buildRoot(): View {
        dashPane = buildDashPane()
        feedPane = buildFeedPane().apply { visibility = View.GONE }

        val content = FrameLayout(this).apply {
            addView(dashPane); addView(feedPane)
        }
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(cr(R.color.cr_bg))
            addView(content, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(buildBottomNav(), LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, dp(66)))
        }
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
                    icon = "⊟", label = "Dashboard", active = true,
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
                    icon = "≡", label = "Activity", active = false,
                    onClick = { switchTab(Tab.FEED) })
                addView(navItemFeed, FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT))
                addView(borderView2, FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT, dp(1)))
            })
        }
    }

    private fun buildNavItem(icon: String, label: String, active: Boolean,
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
            addView(TextView(this@MainActivity).apply {
                tag = "icon"
                text = icon
                textSize = 20f
                gravity = Gravity.CENTER
                setTextColor(tint)
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT).also {
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
        if (newTab == Tab.FEED) rebuildFeed()
    }

    private fun updateNavState(active: Tab) {
        fun applyToNav(item: LinearLayout, isActive: Boolean) {
            val tint = if (isActive) cr(R.color.cr_nav_on) else cr(R.color.cr_nav_off)
            for (i in 0 until item.childCount) {
                val child = item.getChildAt(i)
                when (child.tag) {
                    "pill"  -> (child.background as? GradientDrawable)
                                   ?.setColor(if (isActive) tint else Color.TRANSPARENT)
                    "icon",
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
                addView(buildActionsSection())
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
            addView(TextView(this@MainActivity).apply {
                text = "⚙"
                textSize = 16f
                gravity = Gravity.CENTER
                setTextColor(cr(R.color.cr_text_3))
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
        setPadding(dp(20), dp(20), dp(20), dp(22))

        // Row 1: dot + state label + chip
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
                text = "CHECKING"
                textSize = 10f
                letterSpacing = 0.10f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_3))
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            }
            addView(heroStateLabel)
        })

        addView(vSpace(10))

        // Row 2: big headline (peer names or status phrase)
        heroHeadline = TextView(this@MainActivity).apply {
            text = "—"
            textSize = 30f
            setTypeface(Typeface.create("sans-serif", Typeface.BOLD))
            setTextColor(cr(R.color.cr_text_1))
            letterSpacing = -0.02f
            maxLines = 2
            ellipsize = TextUtils.TruncateAt.END
        }
        addView(heroHeadline)

        addView(vSpace(4))

        // Row 3: device name subline
        heroSubline = TextView(this@MainActivity).apply {
            text = ""
            textSize = 13.5f
            setTypeface(Typeface.create("sans-serif", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_text_3))
        }
        addView(heroSubline)

        addView(vSpace(18))

        // Row 4: security tags
        addView(LinearLayout(this@MainActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL

            addView(infoTag("Noise protocol"))
            addView(hSpace(6))
            addView(infoTag("LAN only"))
            addView(hSpace(6))
            addView(infoTag("Zero cloud"))
        })
    }

    // Peers section (inside scroll content)
    private fun buildPeersSection(): LinearLayout {
        peerRows = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }

        noPeersState = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, dp(8), 0, dp(4))

            addView(TextView(this@MainActivity).apply {
                text = "No devices found"
                textSize = 15f
                setTypeface(typeface, Typeface.BOLD)
                setTextColor(cr(R.color.cr_text_2))
                gravity = Gravity.CENTER
            })
            addView(vSpace(4))
            addView(TextView(this@MainActivity).apply {
                text = "Open the desktop app on the same Wi-Fi network"
                textSize = 13f
                setTextColor(cr(R.color.cr_text_3))
                gravity = Gravity.CENTER
                setLineSpacing(0f, 1.4f)
            })
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

    // Info section — 3 clean rows with dot leaders
    private fun buildInfoSection(): LinearLayout = card().apply {
        addView(sectionEyebrow("How it works"))
        addView(vSpace(14))

        listOf(
            Triple("Same network", "Keep both apps running on the same Wi-Fi.", cr(R.color.cr_accent)),
            Triple("Pair once",    "Confirm the fingerprint — devices reconnect automatically.", cr(R.color.cr_green)),
            Triple("Silent sync",  "Text, images, and files sync without any manual steps.", cr(R.color.cr_blue))
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
        val peers   = prefs.getStringSet("connected_names", emptySet())
                          ?.filter { it.isNotBlank() }?.sorted().orEmpty()

        // Hero headline
        heroHeadline.text = when {
            !running           -> "Stopped"
            !syncOn            -> "Sync paused"
            peers.isNotEmpty() -> peers.take(3).joinToString(", ") +
                                  if (peers.size > 3) " +${peers.size - 3}" else ""
            else               -> "Waiting for devices"
        }

        // Sub-line: "This device: MacBook Pro"
        heroSubline.text = "This device: $myName"

        // State label + dot
        val (stateText, dotColor, stateColor) = when {
            !running -> Triple("STOPPED", cr(R.color.cr_red), cr(R.color.cr_red))
            !syncOn  -> Triple("PAUSED",  cr(R.color.cr_amber), cr(R.color.cr_amber))
            peers.isNotEmpty() -> Triple(
                if (peers.size == 1) "1 DEVICE CONNECTED" else "${peers.size} DEVICES CONNECTED",
                cr(R.color.cr_green), cr(R.color.cr_green))
            else -> Triple("SEARCHING", cr(R.color.cr_text_3), cr(R.color.cr_text_3))
        }
        heroStateLabel.text = stateText
        heroStateLabel.setTextColor(stateColor)
        (heroStatusDot.background as? GradientDrawable)?.setColor(dotColor)
        animateStatusDot(running && syncOn && peers.isNotEmpty())

        // Peers
        refreshPeerRows(peers, running && syncOn)

        // Primary button
        primaryActionBtn.text = if (running) "Restart sync" else "Start sync"

        // Secondary actions
        buildSecondaryActions(running, syncOn)
    }

    private fun animateStatusDot(pulse: Boolean) {
        heroStatusDot.clearAnimation()
        if (!pulse) return
        heroStatusDot.startAnimation(AlphaAnimation(1f, 0.25f).apply {
            duration = 900; repeatMode = Animation.REVERSE; repeatCount = Animation.INFINITE
        })
    }

    private fun refreshPeerRows(peers: List<String>, online: Boolean) {
        peerRows.removeAllViews()
        if (peers.isEmpty()) {
            noPeersState.visibility = View.VISIBLE
            peerRows.visibility     = View.GONE
            return
        }
        noPeersState.visibility = View.GONE
        peerRows.visibility     = View.VISIBLE

        peers.forEachIndexed { i, name ->
            if (i > 0) peerRows.addView(View(this).apply {
                setBackgroundColor(cr(R.color.cr_divider))
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, dp(1)
                ).also { it.setMargins(dp(52), 0, 0, 0) }
            })
            peerRows.addView(buildPeerRow(name, online))
        }
    }

    private fun buildPeerRow(name: String, online: Boolean): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(12), 0, dp(12))
            isClickable = true; isFocusable = true
            background = ripple(cr(R.color.cr_ripple))

            // Avatar — accent-light circle with initial
            val av = dp(40)
            addView(FrameLayout(this@MainActivity).apply {
                layoutParams = LinearLayout.LayoutParams(av, av)
                background = GradientDrawable().also {
                    it.shape = GradientDrawable.OVAL
                    it.setColor(cr(R.color.cr_accent_bg))
                }
                // Initial
                addView(TextView(this@MainActivity).apply {
                    text = name.take(1).uppercase()
                    textSize = 16f
                    gravity = Gravity.CENTER
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_accent))
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT)
                })
                // Online dot (white-bordered)
                val ds = dp(12)
                addView(View(this@MainActivity).apply {
                    background = GradientDrawable().also {
                        it.shape = GradientDrawable.OVAL
                        it.setColor(if (online) cr(R.color.cr_green) else cr(R.color.cr_text_4))
                        it.setStroke(dp(2), cr(R.color.cr_bg_card))
                    }
                    layoutParams = FrameLayout.LayoutParams(ds, ds, Gravity.BOTTOM or Gravity.END)
                })
            })

            addView(hSpace(12))

            // Name + status
            addView(LinearLayout(this@MainActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)

                addView(TextView(this@MainActivity).apply {
                    text = name
                    textSize = 15f
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_text_1))
                })
                addView(vSpace(2))
                addView(LinearLayout(this@MainActivity).apply {
                    orientation = LinearLayout.HORIZONTAL
                    gravity = Gravity.CENTER_VERTICAL
                    // Live dot
                    addView(View(this@MainActivity).apply {
                        background = GradientDrawable().also {
                            it.shape = GradientDrawable.OVAL
                            it.setColor(if (online) cr(R.color.cr_green) else cr(R.color.cr_text_4))
                        }
                        layoutParams = LinearLayout.LayoutParams(dp(6), dp(6)).also {
                            it.rightMargin = dp(5)
                        }
                    })
                    addView(TextView(this@MainActivity).apply {
                        text = if (online) "Syncing · Noise encrypted" else "Offline"
                        textSize = 12.5f
                        setTextColor(if (online) cr(R.color.cr_green) else cr(R.color.cr_text_3))
                    })
                })
            })

            // Chevron
            addView(TextView(this@MainActivity).apply {
                text = "›"
                textSize = 20f
                setTextColor(cr(R.color.cr_text_4))
                gravity = Gravity.CENTER_VERTICAL
            })
        }

    private fun buildSecondaryActions(running: Boolean, syncOn: Boolean) {
        secondaryActionsContainer.removeAllViews()
        if (!running) return

        val items = buildList {
            val toggleLabel  = if (syncOn) "Pause sync" else "Resume sync"
            val toggleAction = if (syncOn) ClipRelayService.ACTION_PAUSE_SYNC
                               else        ClipRelayService.ACTION_RESUME_SYNC
            add(Triple(toggleLabel, cr(R.color.cr_text_2)) {
                sendAction(toggleAction); refreshDashboard()
            })
            add(Triple("Disconnect all", cr(R.color.cr_amber)) {
                sendAction(ClipRelayService.ACTION_DISCONNECT_ALL); refreshDashboard()
            })
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

    // Inline action row: label left, → right
    private fun actionRow(label: String, labelColor: Int, onClick: () -> Unit): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            isClickable = true; isFocusable = true
            background = ripple(cr(R.color.cr_ripple))
            setPadding(0, dp(13), 0, dp(13))
            setOnClickListener { onClick() }

            addView(TextView(this@MainActivity).apply {
                text = label
                textSize = 14.5f
                setTypeface(Typeface.create("sans-serif", Typeface.NORMAL))
                setTextColor(labelColor)
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(TextView(this@MainActivity).apply {
                text = "›"
                textSize = 20f
                setTextColor(cr(R.color.cr_text_4))
            })
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

        addView(feedEmptyState, LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        addView(feedScroller, LinearLayout.LayoutParams(
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

    private fun buildFeedRow(entry: ActivityEntry): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(cr(R.color.cr_bg_card))
            isClickable = true; isFocusable = true
            background = ripple(cr(R.color.cr_ripple), cr(R.color.cr_bg_card))

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

                    // Clipboard text preview
                    if (entry.kind == ActivityKind.CLIPBOARD_TEXT && entry.preview.isNotBlank()) {
                        addView(vSpace(7))
                        addView(TextView(this@MainActivity).apply {
                            text = entry.preview.take(140)
                            textSize = 12.5f
                            setTypeface(Typeface.MONOSPACE, Typeface.NORMAL)
                            setTextColor(cr(R.color.cr_text_3))
                            maxLines = 3
                            ellipsize = TextUtils.TruncateAt.END
                            setLineSpacing(0f, 1.4f)
                            setPadding(dp(11), dp(8), dp(11), dp(8))
                            background = GradientDrawable().also {
                                it.cornerRadius = dp(8).toFloat()
                                it.setColor(cr(R.color.cr_bg_inset))
                            }
                        })
                    }

                    // Transfer progress
                    if (entry.kind == ActivityKind.FILE_TRANSFER_PROGRESS &&
                        entry.progressPercent in 0..99) {
                        addView(vSpace(9))
                        addView(buildProgressStrip(entry.progressPercent / 100.0))
                        addView(vSpace(3))
                        addView(TextView(this@MainActivity).apply {
                            text = "${entry.progressPercent}% transferred"
                            textSize = 12f
                            setTextColor(cr(R.color.cr_text_3))
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
            startService(Intent(this@MainActivity, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_APPLY_CLIPBOARD
                putExtra(ClipRelayService.EXTRA_CLIPBOARD_TEXT, entry.preview)
            })
            rebuildFeed()
        }
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

    private fun alphaBlend(color: Int, alpha: Float): Int {
        val a = (255 * alpha).toInt().coerceIn(0, 255)
        return Color.argb(a, Color.red(color), Color.green(color), Color.blue(color))
    }

    private fun relativeTime(ms: Long): String {
        val d = System.currentTimeMillis() - ms
        return when {
            d < 60_000L     -> "just now"
            d < 3_600_000L  -> "${d / 60_000}m ago"
            d < 86_400_000L -> "${d / 3_600_000}h ago"
            else            -> "${d / 86_400_000}d ago"
        }
    }

    private fun calKey(cal: java.util.Calendar) =
        "${cal.get(java.util.Calendar.YEAR)}-${cal.get(java.util.Calendar.DAY_OF_YEAR)}"

    private fun prefs() = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)

    private fun launchService() = runCatching {
        ContextCompat.startForegroundService(this,
            Intent(this, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_START })
    }

    private fun sendAction(action: String) = runCatching {
        startService(Intent(this, ClipRelayService::class.java).apply { this.action = action })
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
        }
    }
}
