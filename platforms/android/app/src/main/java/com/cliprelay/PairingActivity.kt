package com.cliprelay

import android.app.AlertDialog
import android.content.*
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.os.Build
import android.os.Bundle
import android.os.CountDownTimer
import android.view.*
import android.widget.*
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatButton
import androidx.core.content.ContextCompat
import kotlin.math.roundToInt

// ─── PairingActivity ──────────────────────────────────────────────────────────

class PairingActivity : AppCompatActivity() {

    companion object {
        const val EXTRA_DEVICE_ID       = "device_id"
        const val EXTRA_DEVICE_NAME     = "device_name"
        const val EXTRA_FINGERPRINT     = "fingerprint"
        const val EXTRA_PIN             = "pin"
        const val ACTION_PAIRING_RESULT = "com.cliprelay.PAIRING_RESULT"
        const val EXTRA_APPROVED        = "approved"
        private const val TIMEOUT_MS    = 30_000L
    }

    private lateinit var countdownBar:  View      // fills progress
    private lateinit var countdownTrack: FrameLayout
    private lateinit var countdownLabel: TextView
    private var timer: CountDownTimer? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val deviceId    = intent.getStringExtra(EXTRA_DEVICE_ID)   ?: return finish()
        val deviceName  = intent.getStringExtra(EXTRA_DEVICE_NAME) ?: "Unknown device"
        val fingerprint = intent.getStringExtra(EXTRA_FINGERPRINT) ?: ""
        val pin         = intent.getStringExtra(EXTRA_PIN)         ?: "------"

        setContentView(ScrollView(this).apply {
            setBackgroundColor(cr(R.color.cr_bg))
            addView(buildContent(deviceId, deviceName, fingerprint, pin))
        })
        startTimer(deviceId)
    }

    override fun onDestroy() { timer?.cancel(); super.onDestroy() }

    // ── Layout ────────────────────────────────────────────────────────────────

    private fun buildContent(
        deviceId: String, deviceName: String, fingerprint: String, pin: String
    ): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(dp(20), dp(28), dp(20), dp(40))

        addView(buildHeader(deviceName))
        addView(vSpace(22))
        addView(buildPinBlock(pin))
        addView(vSpace(12))
        addView(buildFingerprintBlock(fingerprint))
        addView(vSpace(18))
        addView(buildTimer())
        addView(vSpace(28))
        addView(buildButtons(deviceId))
    }

    // ── Header ────────────────────────────────────────────────────────────────

    private fun buildHeader(deviceName: String): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL

            // Avatar circle
            val av = dp(72)
            addView(FrameLayout(this@PairingActivity).apply {
                layoutParams = LinearLayout.LayoutParams(av, av).also {
                    it.gravity = Gravity.CENTER_HORIZONTAL
                }
                background = GradientDrawable().also {
                    it.shape  = GradientDrawable.OVAL
                    it.setColor(cr(R.color.cr_accent_bg))
                }
                addView(TextView(this@PairingActivity).apply {
                    text = deviceName.take(1).uppercase()
                    textSize = 28f
                    gravity = Gravity.CENTER
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_accent))
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT)
                })
            })

            addView(vSpace(18))

            addView(TextView(this@PairingActivity).apply {
                text = "Pairing request"
                textSize = 11f
                letterSpacing = 0.08f
                gravity = Gravity.CENTER
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_3))
            })

            addView(vSpace(6))

            addView(TextView(this@PairingActivity).apply {
                text = deviceName
                textSize = 26f
                gravity = Gravity.CENTER
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_1))
                letterSpacing = -0.01f
            })

            addView(vSpace(5))

            addView(TextView(this@PairingActivity).apply {
                text = "wants to join your clipboard network"
                textSize = 14.5f
                gravity = Gravity.CENTER
                setTextColor(cr(R.color.cr_text_3))
            })
        }

    // ── PIN block ─────────────────────────────────────────────────────────────

    private fun buildPinBlock(pin: String): LinearLayout = surfaceCard().apply {
        // Label row
        addView(LinearLayout(this@PairingActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(TextView(this@PairingActivity).apply {
                text = "Verbal confirmation"
                textSize = 10.5f
                letterSpacing = 0.07f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_3))
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(TextView(this@PairingActivity).apply {
                text = "Read aloud to verify"
                textSize = 11.5f
                setTextColor(cr(R.color.cr_text_3))
            })
        })

        addView(vSpace(16))

        // PIN digits — two groups of 3 separated by a dash
        addView(LinearLayout(this@PairingActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity     = Gravity.CENTER

            val chars = pin.take(6).padEnd(6, '·')
            chars.forEachIndexed { i, ch ->
                if (i == 3) {
                    addView(TextView(this@PairingActivity).apply {
                        text = " – "
                        textSize = 24f
                        setTextColor(cr(R.color.cr_text_3))
                        gravity = Gravity.CENTER_VERTICAL
                    })
                }
                addView(pinDigit(ch.toString()))
                if (i < 5 && i != 2) addView(hSpace(5))
            }
        })

        addView(vSpace(14))

        // Divider
        addView(View(this@PairingActivity).apply {
            setBackgroundColor(cr(R.color.cr_divider))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, dp(1))
        })

        addView(vSpace(12))

        addView(TextView(this@PairingActivity).apply {
            text = "Confirm the code matches what's shown on the other device before trusting"
            textSize = 12.5f
            gravity = Gravity.CENTER
            setTextColor(cr(R.color.cr_text_3))
            setLineSpacing(0f, 1.4f)
        })
    }

    private fun pinDigit(char: String): TextView {
        val size = dp(50)
        return TextView(this).apply {
            text = char
            textSize = 22f
            gravity = Gravity.CENTER
            setTypeface(Typeface.MONOSPACE, Typeface.BOLD)
            setTextColor(cr(R.color.cr_accent))
            background = GradientDrawable().also {
                it.cornerRadius = dp(12).toFloat()
                it.setColor(cr(R.color.cr_accent_bg))
            }
            layoutParams = LinearLayout.LayoutParams(size, size)
        }
    }

    // ── Fingerprint block ─────────────────────────────────────────────────────

    private fun buildFingerprintBlock(fp: String): LinearLayout = surfaceCard().apply {
        addView(LinearLayout(this@PairingActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            addView(TextView(this@PairingActivity).apply {
                text = "Device fingerprint"
                textSize = 10.5f
                letterSpacing = 0.07f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_3))
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(TextView(this@PairingActivity).apply {
                text = "Tap to copy"
                textSize = 11.5f
                setTextColor(cr(R.color.cr_accent))
            })
        })

        addView(vSpace(12))

        // Full 32-byte fingerprint grouped into 4 lines of 8 byte pairs.
        // Empty/short fingerprints fall back to a placeholder.
        val pairs = if (fp.isBlank()) emptyList()
                    else fp.replace(":", "").chunked(2)
        val formatted = if (pairs.isEmpty()) "Not available"
                        else pairs.chunked(8).joinToString("\n") { it.joinToString(":") }

        val fpView = TextView(this@PairingActivity).apply {
            text = formatted
            textSize = 12.5f
            setTypeface(Typeface.MONOSPACE, Typeface.NORMAL)
            setTextColor(cr(R.color.cr_text_1))
            letterSpacing = 0.03f
            setLineSpacing(0f, 1.65f)
            setPadding(dp(12), dp(10), dp(12), dp(10))
            background = GradientDrawable().also {
                it.cornerRadius = dp(10).toFloat()
                it.setColor(cr(R.color.cr_bg_inset))
                it.setStroke(dp(1), cr(R.color.cr_border))
            }
            isClickable = true; isFocusable = true
            setOnClickListener {
                val cm = context.getSystemService(ClipboardManager::class.java)
                cm.setPrimaryClip(ClipData.newPlainText("fingerprint", fp))
                text = "Copied!"
                postDelayed({ text = formatted }, 1_500)
            }
        }
        addView(fpView)

        addView(vSpace(8))
        addView(TextView(this@PairingActivity).apply {
            text = "Compare with the fingerprint shown on your Mac's Security settings."
            textSize = 11.5f
            setTextColor(cr(R.color.cr_text_3))
            setLineSpacing(0f, 1.4f)
        })
    }

    // ── Timer ─────────────────────────────────────────────────────────────────

    private fun buildTimer(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL

        // Progress track
        countdownTrack = FrameLayout(this@PairingActivity).apply {
            layoutParams = LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            background = GradientDrawable().also {
                it.cornerRadius = dp(4).toFloat()
                it.setColor(cr(R.color.cr_bg_inset))
            }

            countdownBar = View(this@PairingActivity).apply {
                background = GradientDrawable().also {
                    it.cornerRadius = dp(4).toFloat()
                    it.setColor(cr(R.color.cr_amber))
                }
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT, dp(5))
            }
            addView(View(this@PairingActivity).apply {          // track
                setBackgroundColor(Color.TRANSPARENT)
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT, dp(5))
            })
            addView(countdownBar)
        }
        addView(countdownTrack)
        addView(hSpace(12))

        countdownLabel = TextView(this@PairingActivity).apply {
            text = "30s"
            textSize = 12f
            setTypeface(Typeface.MONOSPACE, Typeface.NORMAL)
            setTextColor(cr(R.color.cr_text_3))
            gravity = Gravity.END
            layoutParams = LinearLayout.LayoutParams(dp(34),
                LinearLayout.LayoutParams.WRAP_CONTENT)
        }
        addView(countdownLabel)
    }

    // ── Action buttons ────────────────────────────────────────────────────────

    private fun buildButtons(deviceId: String): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL

            addView(actionButton("Deny", primary = false) {
                sendResult(deviceId, false)
            }.apply {
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(hSpace(12))
            addView(actionButton("Trust Device", primary = true) {
                sendResult(deviceId, true)
            }.apply {
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 2f)
            })
        }

    // ── Timer logic ───────────────────────────────────────────────────────────

    private fun startTimer(deviceId: String) {
        timer = object : CountDownTimer(TIMEOUT_MS, 80L) {
            override fun onTick(remaining: Long) {
                // Update label
                countdownLabel.text = "${remaining / 1000 + 1}s"

                // Shrink the bar width proportionally
                val fraction = remaining.toFloat() / TIMEOUT_MS.toFloat()
                countdownTrack.post {
                    val w = (countdownTrack.width * fraction).toInt()
                    countdownBar.layoutParams = FrameLayout.LayoutParams(w.coerceAtLeast(0), dp(5))
                }

                // Shift bar colour amber → red in last 10 s
                val barColor = if (remaining < 10_000L) cr(R.color.cr_red) else cr(R.color.cr_amber)
                (countdownBar.background as? GradientDrawable)?.setColor(barColor)
                if (remaining < 10_000L) countdownLabel.setTextColor(cr(R.color.cr_red))
            }

            override fun onFinish() {
                if (!isFinishing) {
                    Toast.makeText(this@PairingActivity,
                        "Request timed out", Toast.LENGTH_SHORT).show()
                    sendResult(deviceId, false)
                }
            }
        }.start()
    }

    private fun sendResult(deviceId: String, approved: Boolean) {
        timer?.cancel()
        sendBroadcast(Intent(ACTION_PAIRING_RESULT).apply {
            putExtra(EXTRA_DEVICE_ID, deviceId)
            putExtra(EXTRA_APPROVED, approved)
            setPackage(packageName)
        })
        finish()
    }

    // ── Primitives ────────────────────────────────────────────────────────────

    private fun actionButton(label: String, primary: Boolean,
                             onClick: () -> Unit): AppCompatButton =
        AppCompatButton(this).apply {
            text    = label
            textSize = 15.5f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            if (primary) {
                setTextColor(cr(R.color.cr_on_accent))
                background = GradientDrawable().also {
                    it.cornerRadius = dp(16).toFloat()
                    it.setColor(cr(R.color.cr_accent))
                }
            } else {
                setTextColor(cr(R.color.cr_text_2))
                background = GradientDrawable().also {
                    it.cornerRadius = dp(16).toFloat()
                    it.setColor(cr(R.color.cr_bg_card))
                    it.setStroke(dp(1), cr(R.color.cr_border))
                }
            }
            setPadding(dp(20), dp(16), dp(20), dp(16))
            setOnClickListener { onClick() }
            installPressFeedback()
        }

    private fun surfaceCard(): LinearLayout = LinearLayout(this).apply {
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

    private fun vSpace(size: Int) = Space(this).apply {
        layoutParams = LinearLayout.LayoutParams(1, dp(size)) }
    private fun hSpace(size: Int) = Space(this).apply {
        layoutParams = LinearLayout.LayoutParams(dp(size), 1) }
    private fun dp(v: Int) = (v * resources.displayMetrics.density).roundToInt()
    private fun cr(id: Int) = ContextCompat.getColor(this, id)

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
                    v.animate().scaleX(0.97f).scaleY(0.97f).alpha(0.9f).setDuration(70).start()
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
}

// ─── SettingsActivity ─────────────────────────────────────────────────────────

class SettingsActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(ScrollView(this).apply {
            setBackgroundColor(cr(R.color.cr_bg))
            addView(buildContent())
        })
    }

    private fun buildContent(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL

        // ── App bar ───────────────────────────────────────────────────────────
        addView(LinearLayout(this@SettingsActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity     = Gravity.CENTER_VERTICAL
            setBackgroundColor(cr(R.color.cr_bg_card))
            setPadding(dp(20), dp(16), dp(16), dp(16))

            // Back button
            addView(FrameLayout(this@SettingsActivity).apply {
                val sz = dp(36)
                layoutParams = LinearLayout.LayoutParams(sz, sz).also {
                    it.rightMargin = dp(12)
                }
                background = ripple(cr(R.color.cr_ripple),
                    GradientDrawable().also {
                        it.shape = GradientDrawable.OVAL
                        it.setColor(cr(R.color.cr_bg_inset))
                    })
                isClickable = true; isFocusable = true
                setOnClickListener { finish() }
                addView(TextView(this@SettingsActivity).apply {
                    text = "‹"
                    textSize = 22f
                    gravity = Gravity.CENTER
                    setTextColor(cr(R.color.cr_text_2))
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT)
                })
            })

            addView(TextView(this@SettingsActivity).apply {
                text = "Settings"
                textSize = 20f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_1))
            })
        })

        addView(View(this@SettingsActivity).apply {
            setBackgroundColor(cr(R.color.cr_border))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, dp(1))
        })

        // ── Sections ──────────────────────────────────────────────────────────
        addView(LinearLayout(this@SettingsActivity).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(16), dp(20), dp(16), dp(40))

            // Device identity — top section since it's personal
            addView(buildIdentitySection())
            addView(vSpace(14))

            // Sync
            addView(buildSyncSection())
            addView(vSpace(14))

            // Notifications
            addView(buildNotificationsSection())
            addView(vSpace(14))

            // Background mode
            addView(buildBackgroundSection())
            addView(vSpace(14))

            // Battery
            addView(buildBatterySection())
            addView(vSpace(14))

            // About
            addView(buildAboutSection())
        })
    }

    // ── Identity ──────────────────────────────────────────────────────────────

    private fun buildIdentitySection(): LinearLayout = section("This device") {
        // Current name display
        addView(LinearLayout(this@SettingsActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity     = Gravity.CENTER_VERTICAL
            setPadding(0, dp(4), 0, dp(14))

            addView(LinearLayout(this@SettingsActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                addView(TextView(this@SettingsActivity).apply {
                    text = "Device name"
                    textSize = 14.5f
                    setTypeface(Typeface.create("sans-serif", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_text_1))
                })
                addView(vSpace(2))
                addView(TextView(this@SettingsActivity).apply {
                    text = resolvedName()
                    textSize = 13f
                    setTextColor(cr(R.color.cr_accent))
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                })
            })

            addView(ghostButton("Edit") { showRenameDialog() })
        })

        addView(rowDivider())

        addView(infoRow(
            label = "Device ID",
            value = prefs().getString("device_id", "—") ?: "—",
            mono  = true
        ))
    }

    // ── Sync ─────────────────────────────────────────────────────────────────

    private fun buildSyncSection(): LinearLayout = section("Clipboard sync") {
        addView(toggleRow("Enable sync",    null,                             "sync_enabled",  true,  onChanged = { notifyServiceSettingsChanged() }))
        addView(rowDivider())
        addView(toggleRow("Sync text",      null,                             "sync_text",     true,  onChanged = { notifyServiceSettingsChanged() }))
        addView(rowDivider())
        addView(toggleRow("Sync images",    null,                             "sync_images",   true,  onChanged = { notifyServiceSettingsChanged() }))
        addView(rowDivider())
        addView(toggleRow("Sync files",     "Saved to the app's Downloads/ClipRelay folder",  "sync_files",    true,  onChanged = { notifyServiceSettingsChanged() }))
    }

    /** Sends a broadcast so the running service re-reads SharedPreferences without restarting. */
    private fun notifyServiceSettingsChanged() {
        sendBroadcast(Intent(ClipRelayService.ACTION_SETTINGS_CHANGED).setPackage(packageName))
    }

    // ── Notifications ─────────────────────────────────────────────────────────

    private fun buildNotificationsSection(): LinearLayout = section("Notifications") {
        addView(toggleRow(
            "Notify on remote copy",
            "Off by default — clipboard sync is silent",
            "notify_on_remote_copy", false
        ))
        addView(rowDivider())
        addView(toggleRow(
            "Notify on file received",
            "Always on — required to open received files",
            "notify_on_file", true, locked = true
        ))
        addView(rowDivider())
        addView(toggleRow(
            "Notify on trust request",
            "Always on — required for security",
            "notify_on_trust", true, locked = true
        ))
    }

    // ── Background mode ───────────────────────────────────────────────────────

    private fun buildBackgroundSection(): LinearLayout = section("Background mode") {
        addView(TextView(this@SettingsActivity).apply {
            text = "How aggressively ClipRelay stays alive in the background."
            textSize = 13f
            setTextColor(cr(R.color.cr_text_3))
            setLineSpacing(0f, 1.4f)
            setPadding(0, 0, 0, dp(14))
        })

        val current = prefs().getString("sync_mode", "always") ?: "always"

        addView(modeCard(
            key = "always",
            title = "Always active",
            desc  = "Full poll rate. Most reliable. Slightly higher battery use.",
            selected = current == "always"
        ))
        addView(vSpace(8))
        addView(modeCard(
            key = "battery",
            title = "Battery optimised",
            desc  = "Reduced poll rate. Gentler on battery. May miss events during deep sleep.",
            selected = current == "battery"
        ))
    }

    // ── Battery ───────────────────────────────────────────────────────────────

    private fun buildBatterySection(): LinearLayout = section("Battery") {
        addView(TextView(this@SettingsActivity).apply {
            text = "On Samsung, Xiaomi, OnePlus, and Oppo devices, background apps are killed " +
                   "aggressively. Add ClipRelay to your battery whitelist for reliable sync."
            textSize = 13f
            setTextColor(cr(R.color.cr_text_3))
            setLineSpacing(0f, 1.4f)
            setPadding(0, 0, 0, dp(14))
        })
        addView(primaryButton("Open battery settings") { openBatterySettings() })
    }

    // ── About ─────────────────────────────────────────────────────────────────

    private fun buildAboutSection(): LinearLayout = section("About") {
        addView(TextView(this@SettingsActivity).apply {
            text = "ClipRelay"
            textSize = 16f
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_text_1))
        })
        addView(vSpace(5))
        addView(TextView(this@SettingsActivity).apply {
            text = "Private clipboard and file relay for your local network.\n" +
                   "No cloud. No account. No telemetry."
            textSize = 13.5f
            setTextColor(cr(R.color.cr_text_3))
            setLineSpacing(0f, 1.5f)
        })
    }

    // ── Section builder ───────────────────────────────────────────────────────

    private fun section(title: String, content: LinearLayout.() -> Unit): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().also {
                it.cornerRadius = dp(18).toFloat()
                it.setColor(cr(R.color.cr_bg_card))
                it.setStroke(dp(1), cr(R.color.cr_border))
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT)
            setPadding(dp(18), dp(16), dp(18), dp(18))

            addView(TextView(this@SettingsActivity).apply {
                text = title.uppercase()
                textSize = 10f
                letterSpacing = 0.09f
                setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                setTextColor(cr(R.color.cr_text_3))
            })
            addView(vSpace(14))
            content()
        }

    // ── Toggle row ────────────────────────────────────────────────────────────

    private fun toggleRow(
        label: String, hint: String?, key: String,
        default: Boolean, locked: Boolean = false,
        onChanged: ((Boolean) -> Unit)? = null
    ): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity     = Gravity.CENTER_VERTICAL
        setPadding(0, dp(11), 0, dp(11))
        if (!locked) {
            isClickable = true; isFocusable = true
            background = ripple(cr(R.color.cr_ripple))
        }

        addView(LinearLayout(this@SettingsActivity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.WRAP_CONTENT, 1f)

            addView(TextView(this@SettingsActivity).apply {
                text = label
                textSize = 14.5f
                setTextColor(cr(R.color.cr_text_1))
            })
            if (hint != null) {
                addView(vSpace(2))
                addView(TextView(this@SettingsActivity).apply {
                    text = hint
                    textSize = 12.5f
                    setTextColor(cr(R.color.cr_text_3))
                    setLineSpacing(0f, 1.3f)
                })
            }
        })

        addView(Switch(this@SettingsActivity).apply {
            isChecked = prefs().getBoolean(key, default)
            alpha     = if (locked) 0.45f else 1f
            isEnabled = !locked
            setOnCheckedChangeListener { _, v ->
                if (isEnabled) {
                    prefs().edit().putBoolean(key, v).apply()
                    onChanged?.invoke(v)
                }
            }
        })
    }

    // ── Info row (read-only) ──────────────────────────────────────────────────

    private fun infoRow(label: String, value: String, mono: Boolean = false): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity     = Gravity.CENTER_VERTICAL
            setPadding(0, dp(11), 0, dp(11))

            addView(TextView(this@SettingsActivity).apply {
                text = label
                textSize = 14.5f
                setTextColor(cr(R.color.cr_text_1))
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })

            addView(TextView(this@SettingsActivity).apply {
                text = value
                textSize = 13f
                setTextColor(cr(R.color.cr_text_3))
                if (mono) setTypeface(Typeface.MONOSPACE, Typeface.NORMAL)
                maxLines = 1
                ellipsize = android.text.TextUtils.TruncateAt.MIDDLE
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT).also {
                    it.leftMargin = dp(12)
                }
            })
        }

    // ── Mode selection card ───────────────────────────────────────────────────

    private fun modeCard(key: String, title: String, desc: String,
                         selected: Boolean): LinearLayout =
        LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity     = Gravity.CENTER_VERTICAL
            background  = GradientDrawable().also {
                it.cornerRadius = dp(14).toFloat()
                it.setColor(if (selected) cr(R.color.cr_accent_bg) else cr(R.color.cr_bg_inset))
                it.setStroke(
                    dp(if (selected) 2 else 1),
                    if (selected) cr(R.color.cr_accent) else cr(R.color.cr_border)
                )
            }
            setPadding(dp(14), dp(14), dp(14), dp(14))
            isClickable = true; isFocusable = true
            setOnClickListener { prefs().edit().putString("sync_mode", key).apply(); recreate() }

            // Text block
            addView(LinearLayout(this@SettingsActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0,
                    LinearLayout.LayoutParams.WRAP_CONTENT, 1f)

                addView(TextView(this@SettingsActivity).apply {
                    text = title
                    textSize = 14.5f
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(if (selected) cr(R.color.cr_accent) else cr(R.color.cr_text_1))
                })
                addView(vSpace(3))
                addView(TextView(this@SettingsActivity).apply {
                    text = desc
                    textSize = 13f
                    setTextColor(if (selected) cr(R.color.cr_accent_dim) else cr(R.color.cr_text_3))
                    setLineSpacing(0f, 1.35f)
                })
            })

            addView(hSpace(14))

            // Radio dot
            val outer = dp(22)
            addView(FrameLayout(this@SettingsActivity).apply {
                layoutParams = LinearLayout.LayoutParams(outer, outer)
                background = GradientDrawable().also {
                    it.shape = GradientDrawable.OVAL
                    it.setColor(Color.TRANSPARENT)
                    it.setStroke(dp(2),
                        if (selected) cr(R.color.cr_accent) else cr(R.color.cr_border_strong))
                }
                if (selected) addView(View(this@SettingsActivity).apply {
                    val inner = dp(12)
                    background = GradientDrawable().also {
                        it.shape = GradientDrawable.OVAL
                        it.setColor(cr(R.color.cr_accent))
                    }
                    layoutParams = FrameLayout.LayoutParams(inner, inner, Gravity.CENTER)
                })
            })
        }

    // ── Buttons ───────────────────────────────────────────────────────────────

    private fun primaryButton(label: String, onClick: () -> Unit): AppCompatButton =
        AppCompatButton(this).apply {
            text = label
            textSize = 14.5f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_on_accent))
            background = GradientDrawable().also {
                it.cornerRadius = dp(14).toFloat()
                it.setColor(cr(R.color.cr_accent))
            }
            setPadding(dp(20), dp(14), dp(20), dp(14))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT)
            setOnClickListener { onClick() }
        }

    private fun ghostButton(label: String, onClick: () -> Unit): AppCompatButton =
        AppCompatButton(this).apply {
            text = label
            textSize = 13f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(cr(R.color.cr_accent))
            background = GradientDrawable().also {
                it.cornerRadius = dp(10).toFloat()
                it.setColor(cr(R.color.cr_accent_bg))
            }
            setPadding(dp(13), dp(8), dp(13), dp(8))
            setOnClickListener { onClick() }
        }

    // ── Dialogs / actions ─────────────────────────────────────────────────────

    private fun showRenameDialog() {
        val field = EditText(this).apply {
            setText(resolvedName())
            setSelection(text.length)
            hint = "My Phone"
            textSize = 15f
            setPadding(dp(16), dp(14), dp(16), dp(14))
        }
        AlertDialog.Builder(this)
            .setTitle("Rename this device")
            .setMessage("This name appears on the network to other ClipRelay devices.")
            .setView(field)
            .setPositiveButton("Save") { _, _ ->
                val name = field.text?.toString()?.trim().orEmpty()
                if (name.isNotEmpty()) {
                    prefs().edit().putString("device_name", name).apply()
                    restartService()
                    Toast.makeText(this, "Renamed to \u201c$name\u201d", Toast.LENGTH_SHORT).show()
                    recreate()
                }
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun openBatterySettings() {
        runCatching {
            startActivity(android.content.Intent(
                android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                android.net.Uri.parse("package:$packageName")))
        }.onFailure {
            runCatching {
                startActivity(android.content.Intent(
                    android.provider.Settings.ACTION_BATTERY_SAVER_SETTINGS))
            }.onFailure {
                Toast.makeText(this,
                    "Open Settings \u2192 Battery \u2192 ClipRelay \u2192 disable optimisation",
                    Toast.LENGTH_LONG).show()
            }
        }
    }

    private fun restartService() {
        stopService(Intent(this, ClipRelayService::class.java))
        ContextCompat.startForegroundService(this,
            Intent(this, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_START })
    }

    private fun resolvedName(): String =
        prefs().getString("device_name", null)?.trim()?.takeIf { it.isNotBlank() }
            ?: prefs().getString("local_device_name", null)?.trim()?.takeIf { it.isNotBlank() }
            ?: Build.MODEL

    // ── Helpers ───────────────────────────────────────────────────────────────

    private fun rowDivider(): View = View(this).apply {
        setBackgroundColor(cr(R.color.cr_divider))
        layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, dp(1)
        ).also { it.setMargins(0, dp(3), 0, dp(3)) }
    }

    private fun ripple(rippleColor: Int,
                       content: android.graphics.drawable.Drawable? = null)
        : android.graphics.drawable.Drawable =
        RippleDrawable(android.content.res.ColorStateList.valueOf(rippleColor), content, null)

    private fun prefs() = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
    private fun vSpace(size: Int) = Space(this).apply {
        layoutParams = LinearLayout.LayoutParams(1, dp(size)) }
    private fun hSpace(size: Int) = Space(this).apply {
        layoutParams = LinearLayout.LayoutParams(dp(size), 1) }
    private fun dp(v: Int) = (v * resources.displayMetrics.density).roundToInt()
    private fun cr(id: Int) = ContextCompat.getColor(this, id)
}
