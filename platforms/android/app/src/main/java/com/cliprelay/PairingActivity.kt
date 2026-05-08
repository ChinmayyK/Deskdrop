package com.cliprelay

import android.app.AlertDialog
import android.content.Intent
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.view.Gravity
import android.widget.*
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatButton
import androidx.core.content.ContextCompat
import androidx.core.view.setPadding
import kotlin.math.roundToInt

/**
 * Trust / TOFU dialog — shown when a remote device requests pairing.
 *
 * Shows:
 *   - Device friendly name (prominent)
 *   - Short fingerprint for visual verification
 *   - PIN code for verbal confirmation
 *   - 30-second auto-deny timeout
 *
 * Internal device UUID is intentionally NOT shown here.
 */
class PairingActivity : AppCompatActivity() {

    companion object {
        const val EXTRA_DEVICE_ID   = "device_id"
        const val EXTRA_DEVICE_NAME = "device_name"
        const val EXTRA_FINGERPRINT = "fingerprint"
        const val EXTRA_PIN         = "pin"
        const val ACTION_PAIRING_RESULT = "com.cliprelay.PAIRING_RESULT"
        const val EXTRA_APPROVED    = "approved"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val deviceId    = intent.getStringExtra(EXTRA_DEVICE_ID) ?: return finish()
        val deviceName  = intent.getStringExtra(EXTRA_DEVICE_NAME) ?: "Unknown device"
        val fingerprint = intent.getStringExtra(EXTRA_FINGERPRINT) ?: ""
        val pin         = intent.getStringExtra(EXTRA_PIN) ?: "------"

        setContentView(ScrollView(this).apply {
            addView(buildRoot(deviceId, deviceName, fingerprint, pin))
        })
        title = "ClipRelay — Trust request"

        // Auto-deny after 30 s
        window.decorView.postDelayed({
            if (!isFinishing) {
                Toast.makeText(this, "Pairing request timed out", Toast.LENGTH_SHORT).show()
                sendResult(deviceId, approved = false)
            }
        }, 30_000L)
    }

    private fun buildRoot(
        deviceId: String,
        deviceName: String,
        fingerprint: String,
        pin: String
    ): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.pb_canvas_top))
            setPadding(dp(24))

            // ── Trust prompt card ─────────────────────────────────────────────
            addView(LinearLayout(this@PairingActivity).apply {
                orientation = LinearLayout.VERTICAL
                gravity = Gravity.CENTER_HORIZONTAL
                background = GradientDrawable(
                    GradientDrawable.Orientation.TL_BR,
                    intArrayOf(color(R.color.pb_outline), color(R.color.pb_canvas_bottom))
                ).apply {
                    cornerRadius = dp(28).toFloat()
                    setStroke(dp(1), color(R.color.pb_primary_dark))
                }
                setPadding(dp(24))

                addView(TextView(this@PairingActivity).apply {
                    text = "PAIRING REQUEST"
                    textSize = 11f
                    letterSpacing = 0.12f
                    setTypeface(Typeface.DEFAULT_BOLD)
                    setTextColor(color(R.color.pb_secondary))
                })

                addView(Space(this@PairingActivity).apply {
                    layoutParams = LinearLayout.LayoutParams(1, dp(10))
                })

                // Device name — prominent (NOT the UUID)
                addView(TextView(this@PairingActivity).apply {
                    text = deviceName
                    textSize = 28f
                    setTypeface(Typeface.create("serif", Typeface.BOLD))
                    setTextColor(color(R.color.pb_surface))
                    gravity = Gravity.CENTER
                })

                addView(TextView(this@PairingActivity).apply {
                    text = "wants to join your clipboard mesh"
                    textSize = 15f
                    gravity = Gravity.CENTER
                    setTextColor(color(R.color.pb_surface_alt))
                    setPadding(0, dp(6), 0, dp(20))
                })

                // PIN for verbal confirmation
                addView(TextView(this@PairingActivity).apply {
                    text = pin
                    textSize = 40f
                    letterSpacing = 0.18f
                    gravity = Gravity.CENTER
                    setTypeface(Typeface.create("monospace", Typeface.BOLD))
                    setTextColor(color(R.color.pb_primary))
                    background = GradientDrawable().apply {
                        cornerRadius = dp(20).toFloat()
                        setColor(color(R.color.pb_outline))
                        setStroke(dp(1), color(R.color.pb_primary_dark))
                    }
                    setPadding(dp(20), dp(20), dp(20), dp(20))
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        LinearLayout.LayoutParams.WRAP_CONTENT
                    ).apply { setMargins(0, 0, 0, dp(16)) }
                })

                // Fingerprint — secondary position (not the ID itself)
                addView(TextView(this@PairingActivity).apply {
                    text = "Fingerprint:  ${formatFingerprint(fingerprint)}"
                    textSize = 12f
                    setTypeface(Typeface.MONOSPACE)
                    setTextColor(color(R.color.pb_surface_alt))
                    gravity = Gravity.CENTER
                })

                addView(TextView(this@PairingActivity).apply {
                    text = "Only trust devices you own. This request expires in 30 seconds."
                    textSize = 13f
                    gravity = Gravity.CENTER
                    setTextColor(color(R.color.pb_muted))
                    setPadding(0, dp(14), 0, 0)
                })
            })

            addView(Space(this@PairingActivity).apply {
                layoutParams = LinearLayout.LayoutParams(1, dp(20))
            })

            // ── Action buttons ────────────────────────────────────────────────
            addView(LinearLayout(this@PairingActivity).apply {
                orientation = LinearLayout.HORIZONTAL

                addView(actionBtn("Deny", primary = false) {
                    sendResult(deviceId, approved = false)
                }.apply {
                    layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                })

                addView(Space(this@PairingActivity).apply {
                    layoutParams = LinearLayout.LayoutParams(dp(12), 1)
                })

                addView(actionBtn("Trust device", primary = true) {
                    sendResult(deviceId, approved = true)
                }.apply {
                    layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                })
            })
        }
    }

    private fun formatFingerprint(fp: String): String {
        // Format as "A4:F2:91:..." (colon-separated pairs)
        return fp.chunked(2).take(8).joinToString(":")
    }

    private fun sendResult(deviceId: String, approved: Boolean) {
        sendBroadcast(Intent(ACTION_PAIRING_RESULT).apply {
            putExtra(EXTRA_DEVICE_ID, deviceId)
            putExtra(EXTRA_APPROVED, approved)
            setPackage(packageName)
        })
        finish()
    }

    private fun actionBtn(label: String, primary: Boolean, onClick: () -> Unit): AppCompatButton {
        return AppCompatButton(this).apply {
            text = label
            textSize = 15f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(
                if (primary) ContextCompat.getColor(this@PairingActivity, android.R.color.white)
                else color(R.color.pb_surface)
            )
            background = if (primary) {
                GradientDrawable(
                    GradientDrawable.Orientation.LEFT_RIGHT,
                    intArrayOf(color(R.color.pb_primary), color(R.color.pb_primary_dark))
                ).apply { cornerRadius = dp(18).toFloat() }
            } else {
                GradientDrawable().apply {
                    cornerRadius = dp(18).toFloat()
                    setColor(color(R.color.pb_outline))
                    setStroke(dp(1), color(R.color.pb_primary_dark))
                }
            }
            setPadding(dp(18), dp(16), dp(18), dp(16))
            setOnClickListener { onClick() }
        }
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).roundToInt()
    private fun color(id: Int) = ContextCompat.getColor(this, id)
}

// ── Settings Activity ─────────────────────────────────────────────────────────

class SettingsActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(ScrollView(this).apply {
            addView(buildRoot())
        })
        title = "ClipRelay Settings"
    }

    private fun buildRoot(): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color(R.color.pb_canvas_top))
            setPadding(dp(20))

            addView(buildHeader())
            addView(gap())

            // ── Notification preferences ──────────────────────────────────────
            addView(section("Notification preferences") {

                addView(switchRow(
                    "Notify when remote device copies",
                    "OFF by default — clipboard sync is silent",
                    prefs().getBoolean("notify_on_remote_copy", false)
                ) { saveBool("notify_on_remote_copy", it) })

                addView(switchRow(
                    "Notify on file received",
                    "Always on — files saved to Downloads/ClipRelay",
                    true, enabled = false
                ) { /* always on */ })

                addView(switchRow(
                    "Notify on trust request",
                    "Always on — new device pairing requires confirmation",
                    true, enabled = false
                ) { /* always on */ })
            })

            addView(gap())

            // ── Sync settings ─────────────────────────────────────────────────
            addView(section("Sync") {

                addView(switchRow(
                    "Enable clipboard sync",
                    null,
                    prefs().getBoolean("sync_enabled", true)
                ) { saveBool("sync_enabled", it); notifyService() })

                addView(switchRow(
                    "Sync text",
                    null,
                    prefs().getBoolean("sync_text", true)
                ) { saveBool("sync_text", it) })

                addView(switchRow(
                    "Sync images",
                    null,
                    prefs().getBoolean("sync_images", true)
                ) { saveBool("sync_images", it) })

                addView(switchRow(
                    "Sync files",
                    "Files saved to Downloads/ClipRelay",
                    prefs().getBoolean("sync_files", true)
                ) { saveBool("sync_files", it) })
            })

            addView(gap())

            // ── Background sync mode ──────────────────────────────────────────
            addView(section("Background sync mode") {
                val currentMode = prefs().getString("sync_mode", "always") ?: "always"

                addView(TextView(this@SettingsActivity).apply {
                    text = "Controls how aggressively ClipRelay stays awake in the background."
                    textSize = 13f
                    setTextColor(color(R.color.pb_muted))
                    setPadding(0, 0, 0, dp(14))
                })

                // Mode radio group
                listOf(
                    Triple("always",  "Always Active",      "Full poll rate. Maximum reliability. Slightly higher battery use."),
                    Triple("battery", "Battery Optimized",  "Reduced poll rate. Lower battery use. May miss clipboard events during deep sleep.")
                ).forEach { (key, title, desc) ->
                    addView(modeRow(key, title, desc, currentMode == key) {
                        saveStr("sync_mode", key)
                        notifyService()
                    })
                    addView(gap(8))
                }
            })

            addView(gap())

            // ── Device identity ───────────────────────────────────────────────
            addView(section("Device name") {
                val resolved = resolvedDeviceName()
                addView(TextView(this@SettingsActivity).apply {
                    text = "This device appears as:"
                    textSize = 13f
                    setTextColor(color(R.color.pb_muted))
                })
                addView(TextView(this@SettingsActivity).apply {
                    text = resolved
                    textSize = 18f
                    setTypeface(Typeface.DEFAULT_BOLD)
                    setTextColor(color(R.color.pb_ink))
                    setPadding(0, dp(6), 0, dp(14))
                })
                addView(actionBtn("Rename device") { showRenameDialog() })
            })

            addView(gap())

            // ── Battery killers ───────────────────────────────────────────────
            addView(section("Battery optimisation") {
                addView(TextView(this@SettingsActivity).apply {
                    text = "On OnePlus, Xiaomi, Oppo, Vivo, and Samsung, background apps can be killed aggressively. " +
                           "Add ClipRelay to your battery whitelist for reliable background sync."
                    textSize = 13f
                    setTextColor(color(R.color.pb_muted))
                    setPadding(0, 0, 0, dp(14))
                })
                addView(actionBtn("Open battery settings") { openBatterySettings() })
            })

            addView(gap())

            // ── About ─────────────────────────────────────────────────────────
            addView(section("About") {
                addView(TextView(this@SettingsActivity).apply {
                    text = "ClipRelay v0.2\n\nPrivate clipboard and file relay for devices on the same network. No cloud, no account, no telemetry."
                    textSize = 14f
                    setTextColor(color(R.color.pb_ink))
                    lineSpacingMultiplier = 1.4f
                })
            })

            addView(gap(40))
        }
    }

    private fun buildHeader(): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(TextView(this@SettingsActivity).apply {
                text = "Settings"
                textSize = 30f
                setTypeface(Typeface.create("serif", Typeface.BOLD))
                setTextColor(color(R.color.pb_surface))
            })
            addView(TextView(this@SettingsActivity).apply {
                text = "Control how ClipRelay syncs and notifies you."
                textSize = 14f
                setTextColor(color(R.color.pb_muted))
                setPadding(0, dp(6), 0, 0)
            })
        }
    }

    private fun section(title: String, content: LinearLayout.() -> Unit): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable(
                GradientDrawable.Orientation.TL_BR,
                intArrayOf(color(R.color.pb_surface), color(R.color.pb_surface_alt))
            ).apply {
                cornerRadius = dp(22).toFloat()
                setStroke(dp(1), color(R.color.pb_outline))
            }
            setPadding(dp(20))

            addView(TextView(this@SettingsActivity).apply {
                text = title.uppercase()
                textSize = 11f
                letterSpacing = 0.10f
                setTypeface(Typeface.DEFAULT_BOLD)
                setTextColor(color(R.color.pb_primary_dark))
            })
            addView(gap(10))
            content()
        }
    }

    private fun switchRow(
        label: String,
        hint: String?,
        checked: Boolean,
        enabled: Boolean = true,
        onChange: (Boolean) -> Unit
    ): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = android.view.Gravity.CENTER_VERTICAL
            background = GradientDrawable().apply {
                cornerRadius = dp(14).toFloat()
                setColor(color(R.color.pb_surface_alt))
                setStroke(dp(1), color(R.color.pb_outline))
            }
            setPadding(dp(14))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(0, 0, 0, dp(10)) }

            addView(LinearLayout(this@SettingsActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                addView(TextView(this@SettingsActivity).apply {
                    text = label
                    textSize = 15f
                    setTextColor(color(R.color.pb_ink))
                })
                if (hint != null) {
                    addView(TextView(this@SettingsActivity).apply {
                        text = hint
                        textSize = 12f
                        setTextColor(color(R.color.pb_muted))
                        setPadding(0, dp(2), 0, 0)
                    })
                }
            })

            addView(Switch(this@SettingsActivity).apply {
                isChecked = checked
                isEnabled = enabled
                setOnCheckedChangeListener { _, v -> if (this.isEnabled) onChange(v) }
            })
        }
    }

    private fun modeRow(
        key: String,
        title: String,
        desc: String,
        selected: Boolean,
        onSelect: () -> Unit
    ): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = android.view.Gravity.CENTER_VERTICAL
            background = GradientDrawable().apply {
                cornerRadius = dp(14).toFloat()
                setColor(if (selected) color(R.color.pb_primary_dark) else color(R.color.pb_surface_alt))
                setStroke(dp(if (selected) 2 else 1), color(if (selected) R.color.pb_primary else R.color.pb_outline))
            }
            setPadding(dp(14))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
            setOnClickListener { onSelect() }

            addView(LinearLayout(this@SettingsActivity).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                addView(TextView(this@SettingsActivity).apply {
                    text = title
                    textSize = 15f
                    setTypeface(Typeface.DEFAULT_BOLD)
                    setTextColor(if (selected) color(R.color.pb_surface) else color(R.color.pb_ink))
                })
                addView(TextView(this@SettingsActivity).apply {
                    text = desc
                    textSize = 12f
                    setTextColor(if (selected) color(R.color.pb_surface_alt) else color(R.color.pb_muted))
                    setPadding(0, dp(2), 0, 0)
                })
            })

            addView(RadioButton(this@SettingsActivity).apply {
                isChecked = selected
                isClickable = false
                isFocusable = false
            })
        }
    }

    private fun actionBtn(label: String, onClick: () -> Unit): AppCompatButton {
        return AppCompatButton(this).apply {
            text = label
            textSize = 14f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(ContextCompat.getColor(this@SettingsActivity, android.R.color.white))
            background = GradientDrawable(
                GradientDrawable.Orientation.LEFT_RIGHT,
                intArrayOf(color(R.color.pb_primary), color(R.color.pb_primary_dark))
            ).apply { cornerRadius = dp(16).toFloat() }
            setPadding(dp(16), dp(12), dp(16), dp(12))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
            setOnClickListener { onClick() }
        }
    }

    private fun showRenameDialog() {
        val input = android.widget.EditText(this).apply {
            setText(resolvedDeviceName())
            setSelection(text.length)
            hint = "My Phone"
        }
        AlertDialog.Builder(this)
            .setTitle("Rename this device")
            .setMessage("This name is shown to other ClipRelay devices on the network.")
            .setView(input)
            .setPositiveButton("Save") { _, _ ->
                val name = input.text?.toString()?.trim().orEmpty()
                if (name.isNotEmpty()) {
                    prefs().edit().putString("device_name", name).apply()
                    restartService()
                    Toast.makeText(this, "Device renamed to \"$name\"", Toast.LENGTH_SHORT).show()
                }
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun openBatterySettings() {
        runCatching {
            // Try the direct battery optimisation exemption screen first
            val intent = android.content.Intent(
                android.provider.Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
                android.net.Uri.parse("package:$packageName")
            )
            startActivity(intent)
        }.onFailure {
            // Fallback: open generic battery settings
            runCatching {
                startActivity(android.content.Intent(android.provider.Settings.ACTION_BATTERY_SAVER_SETTINGS))
            }.onFailure {
                Toast.makeText(this, "Open Settings → Battery → ClipRelay and disable optimisation", Toast.LENGTH_LONG).show()
            }
        }
    }

    private fun restartService() {
        stopService(android.content.Intent(this, ClipRelayService::class.java))
        ContextCompat.startForegroundService(
            this,
            android.content.Intent(this, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_START
            }
        )
    }

    private fun notifyService() {
        // No-op — service reads prefs on next poll cycle
    }

    private fun resolvedDeviceName(): String {
        val p = prefs()
        return p.getString("device_name", null)?.trim()?.takeIf { it.isNotEmpty() }
            ?: p.getString("local_device_name", null)?.trim()?.takeIf { it.isNotEmpty() }
            ?: android.os.Build.MODEL
    }

    private fun prefs() = getSharedPreferences(ClipRelayService.PREFS_NAME, MODE_PRIVATE)
    private fun saveBool(key: String, value: Boolean) = prefs().edit().putBoolean(key, value).apply()
    private fun saveStr(key: String, value: String)   = prefs().edit().putString(key, value).apply()
    private fun gap(size: Int = 16) = Space(this).apply { layoutParams = LinearLayout.LayoutParams(1, dp(size)) }
    private fun dp(v: Int) = (v * resources.displayMetrics.density).roundToInt()
    private fun color(id: Int) = ContextCompat.getColor(this, id)
}
