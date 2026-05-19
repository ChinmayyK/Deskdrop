package com.cliprelay

import android.os.Build
import kotlin.math.roundToInt
import android.app.AlertDialog
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.*
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatButton
import androidx.core.content.ContextCompat

// ─── SettingsActivity ─────────────────────────────────────────────────────────

class SettingsActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        contentHost = buildContent()
        setContentView(ScrollView(this).apply {
            setBackgroundColor(cr(R.color.cr_bg))
            addView(contentHost)
        })
        configureEdgeToEdge()
    }

    override fun onResume() {
        super.onResume()
        // Re-inflate content list to update dynamic permission statuses dynamically
        if (::contentHost.isInitialized) {
            contentHost.removeAllViews()
            val newContent = buildContent()
            while (newContent.childCount > 0) {
                val child = newContent.getChildAt(0)
                newContent.removeViewAt(0)
                contentHost.addView(child)
            }
        }
        try {
            startService(Intent(this, ClipRelayService::class.java))
        } catch (e: Exception) {
            android.util.Log.e("SettingsActivity", "Failed to refresh service onResume", e)
        }
    }

    private lateinit var contentHost: LinearLayout

    private fun configureEdgeToEdge() {
        androidx.core.view.WindowCompat.setDecorFitsSystemWindows(window, false)
        window.statusBarColor = android.graphics.Color.TRANSPARENT
        window.navigationBarColor = android.graphics.Color.TRANSPARENT

        val rootChrome = findViewById<android.view.View>(android.R.id.content)
        val isDark = (resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK) == android.content.res.Configuration.UI_MODE_NIGHT_YES
        androidx.core.view.WindowInsetsControllerCompat(window, rootChrome).apply {
            isAppearanceLightStatusBars = !isDark
            isAppearanceLightNavigationBars = !isDark
        }

        androidx.core.view.ViewCompat.setOnApplyWindowInsetsListener(rootChrome) { _, insets ->
            val bars = insets.getInsets(androidx.core.view.WindowInsetsCompat.Type.systemBars())
            contentHost.setPadding(0, bars.top, 0, bars.bottom)
            insets
        }
        androidx.core.view.ViewCompat.requestApplyInsets(rootChrome)
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

            // Appearance
            addView(buildAppearanceSection())
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

            // Call continuity remote control bypass for Android 10+
            addView(buildCallContinuitySection())
            addView(vSpace(14))

            // About
            addView(buildAboutSection())
        })
    }

    // ── Identity ──────────────────────────────────────────────────────────────

    private fun buildIdentitySection(): LinearLayout = section("This device") {
        addView(LinearLayout(this@SettingsActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(4), 0, dp(4))

            addView(LinearLayout(this@SettingsActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                
                addView(TextView(this@SettingsActivity).apply {
                    text = resolvedName()
                    textSize = 16f
                    setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
                    setTextColor(cr(R.color.cr_text_1))
                })
                
                addView(TextView(this@SettingsActivity).apply {
                    text = "ID: " + (prefs().getString("device_id", "—")?.take(8) ?: "—")
                    textSize = 12f
                    setTypeface(Typeface.MONOSPACE, Typeface.NORMAL)
                    setTextColor(cr(R.color.cr_text_3))
                })
            })

            addView(ghostButton("Edit") { showRenameDialog() })
        })
    }

    // ── Appearance ────────────────────────────────────────────────────────────

    private fun buildAppearanceSection(): LinearLayout = section("Appearance") {
        val current = prefs().getInt("theme_mode", androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_NO)

        fun updateTheme(mode: Int) {
            val prev = prefs().getInt("theme_mode", androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_NO)
            if (prev == mode) return // no change — avoid unnecessary recreate()
            prefs().edit().putInt("theme_mode", mode).apply()
            androidx.appcompat.app.AppCompatDelegate.setDefaultNightMode(mode)
            recreate()
        }

        addView(modeCard(
            key = "light",
            title = "Light",
            desc  = "Classic clean look.",
            selected = current == androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_NO
        ) { updateTheme(androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_NO) })
        addView(vSpace(8))
        addView(modeCard(
            key = "dark",
            title = "True Black",
            desc  = "Deep black for OLED displays.",
            selected = current == androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_YES
        ) { updateTheme(androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_YES) })
        addView(vSpace(8))
        addView(modeCard(
            key = "system",
            title = "System Default",
            desc  = "Follow system settings.",
            selected = current == androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM
        ) { updateTheme(androidx.appcompat.app.AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM) })

        addView(vSpace(14))
        addView(rowDivider())
        
        val dynSwitch = Switch(this@SettingsActivity).apply {
            isChecked = prefs().getBoolean("use_dynamic_colors", true)
            isEnabled = com.google.android.material.color.DynamicColors.isDynamicColorAvailable()
            setOnCheckedChangeListener { _, checked ->
                if (isEnabled) {
                    prefs().edit().putBoolean("use_dynamic_colors", checked).apply()
                    // Restart process to apply DynamicColors thoroughly
                    Runtime.getRuntime().exit(0) 
                }
            }
        }
        addView(toggleRowCustom("Material You", "Use dynamic system colors", dynSwitch))
    }

    private fun toggleRowCustom(label: String, hint: String, accessory: View): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        setPadding(0, dp(11), 0, dp(11))
        
        addView(LinearLayout(this@SettingsActivity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)

            addView(TextView(this@SettingsActivity).apply {
                text = label
                textSize = 14.5f
                setTextColor(cr(R.color.cr_text_1))
            })
            addView(vSpace(2))
            addView(TextView(this@SettingsActivity).apply {
                text = hint
                textSize = 12.5f
                setTextColor(cr(R.color.cr_text_3))
            })
        })
        addView(accessory)
    }

    // ── Sync ─────────────────────────────────────────────────────────────────

    private fun buildSyncSection(): LinearLayout = section("Clipboard sync") {
        addView(toggleRow("Enable sync",    null,                             "sync_enabled",  true,  onChanged = { notifyServiceSettingsChanged() }))
        addView(rowDivider())
        addView(toggleRow("Sync text",      null,                             "sync_text",     true,  onChanged = { notifyServiceSettingsChanged() }))
        addView(rowDivider())
        addView(toggleRow("Sync images",    null,                             "sync_images",   true,  onChanged = { notifyServiceSettingsChanged() }))
        addView(rowDivider())
        addView(toggleRow("Sync files",     "Saved directly to your device's Downloads folder",  "sync_files",    true,  onChanged = { notifyServiceSettingsChanged() }))
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
            text = "On OnePlus, Oppo, Realme, and Xiaomi devices, the OS will freeze the background sync when your screen is turned off.\n\n" +
                   "To prevent this, go to: Settings -> Apps -> App management -> ClipRelay -> Battery, and enable both 'Allow background activity' and 'Allow auto-launch'."
            textSize = 13f
            setTextColor(cr(R.color.cr_text_3))
            setLineSpacing(0f, 1.4f)
            setPadding(0, 0, 0, dp(14))
        })
        addView(primaryButton("Open battery settings") { openBatterySettings() })
    }

    // ── Call Continuity ───────────────────────────────────────────────────────

    private fun isNotificationServiceEnabled(): Boolean {
        val pkgName = packageName
        val flat = android.provider.Settings.Secure.getString(contentResolver, "enabled_notification_listeners")
        if (!flat.isNullOrEmpty()) {
            val names = flat.split(":")
            for (name in names) {
                val cn = android.content.ComponentName.unflattenFromString(name)
                if (cn != null && cn.packageName == pkgName) {
                    return true
                }
            }
        }
        return false
    }

    private fun checkAndRequestCallPermissions() {
        val permissions = mutableListOf<String>()
        if (checkSelfPermission(android.Manifest.permission.READ_PHONE_STATE) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            permissions.add(android.Manifest.permission.READ_PHONE_STATE)
        }
        if (checkSelfPermission(android.Manifest.permission.ANSWER_PHONE_CALLS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            permissions.add(android.Manifest.permission.ANSWER_PHONE_CALLS)
        }
        if (checkSelfPermission(android.Manifest.permission.READ_CONTACTS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            permissions.add(android.Manifest.permission.READ_CONTACTS)
        }
        if (checkSelfPermission(android.Manifest.permission.READ_CALL_LOG) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED) {
            permissions.add(android.Manifest.permission.READ_CALL_LOG)
        }

        if (permissions.isNotEmpty()) {
            requestPermissions(permissions.toTypedArray(), 1002)
        } else {
            Toast.makeText(this, "All call continuity permissions are already granted!", Toast.LENGTH_SHORT).show()
        }
    }

    private fun buildCallContinuitySection(): LinearLayout = section("Call Remote Control") {
        addView(TextView(this@SettingsActivity).apply {
            text = "Android 10+ restricts background apps from accepting or declining cellular calls. " +
                   "Ensure you grant both the system telephony permissions and Notification Access to allow ClipRelay to monitor and control calls from your Mac."
            textSize = 13f
            setTextColor(cr(R.color.cr_text_3))
            setLineSpacing(0f, 1.4f)
            setPadding(0, 0, 0, dp(14))
        })

        // 1. System Call Permissions Status
        val hasPhoneState = checkSelfPermission(android.Manifest.permission.READ_PHONE_STATE) == android.content.pm.PackageManager.PERMISSION_GRANTED
        val hasAnswerCalls = checkSelfPermission(android.Manifest.permission.ANSWER_PHONE_CALLS) == android.content.pm.PackageManager.PERMISSION_GRANTED
        val hasContacts = checkSelfPermission(android.Manifest.permission.READ_CONTACTS) == android.content.pm.PackageManager.PERMISSION_GRANTED
        val hasCallLog = checkSelfPermission(android.Manifest.permission.READ_CALL_LOG) == android.content.pm.PackageManager.PERMISSION_GRANTED
        val allRuntimeGranted = hasPhoneState && hasAnswerCalls && hasContacts && hasCallLog

        addView(LinearLayout(this@SettingsActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(2), 0, dp(6))

            addView(TextView(this@SettingsActivity).apply {
                text = "System Call Access: "
                textSize = 14f
                setTextColor(cr(R.color.cr_text_2))
            })

            addView(TextView(this@SettingsActivity).apply {
                text = if (allRuntimeGranted) "Granted (Ready)" else "Missing Permissions"
                textSize = 14f
                setTypeface(Typeface.DEFAULT_BOLD)
                setTextColor(if (allRuntimeGranted) Color.parseColor("#30D158") else Color.parseColor("#FF453A"))
            })
        })

        if (!allRuntimeGranted) {
            addView(primaryButton("Grant Call Permissions") {
                checkAndRequestCallPermissions()
            })
            addView(vSpace(10))
        }

        // 2. Notification Access Status
        val enabled = isNotificationServiceEnabled()
        addView(LinearLayout(this@SettingsActivity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(4), 0, dp(12))

            addView(TextView(this@SettingsActivity).apply {
                text = "Notification Listener: "
                textSize = 14f
                setTextColor(cr(R.color.cr_text_2))
            })

            addView(TextView(this@SettingsActivity).apply {
                text = if (enabled) "Enabled (Ready)" else "Disabled (Required)"
                textSize = 14f
                setTypeface(Typeface.DEFAULT_BOLD)
                setTextColor(if (enabled) Color.parseColor("#30D158") else Color.parseColor("#FF453A"))
            })
        })

        addView(primaryButton(if (enabled) "Modify Notification Access" else "Enable Notification Access") {
            try {
                startActivity(Intent("android.settings.ACTION_NOTIFICATION_LISTENER_SETTINGS"))
            } catch (e: Exception) {
                Toast.makeText(this@SettingsActivity, "Please open Settings -> Notification Access and enable ClipRelay", Toast.LENGTH_LONG).show()
            }
        })
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
                         selected: Boolean, onClickCallback: (() -> Unit)? = null): LinearLayout =
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
            setOnClickListener {
                if (onClickCallback != null) {
                    onClickCallback()
                } else {
                    prefs().edit().putString("sync_mode", key).apply(); recreate()
                }
            }

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
        com.google.android.material.dialog.MaterialAlertDialogBuilder(this)
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
