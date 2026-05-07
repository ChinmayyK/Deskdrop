package com.cliprelay

import android.content.Intent
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.view.Gravity
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Space
import android.widget.Switch
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.AppCompatButton
import androidx.core.content.ContextCompat
import androidx.core.view.setPadding
import kotlin.math.roundToInt

class PairingActivity : AppCompatActivity() {

    companion object {
        const val EXTRA_DEVICE_ID = "device_id"
        const val EXTRA_DEVICE_NAME = "device_name"
        const val EXTRA_FINGERPRINT = "fingerprint"
        const val EXTRA_PIN = "pin"
        const val ACTION_PAIRING_RESULT = "com.cliprelay.PAIRING_RESULT"
        const val EXTRA_APPROVED = "approved"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val deviceId = intent.getStringExtra(EXTRA_DEVICE_ID) ?: return finish()
        val deviceName = intent.getStringExtra(EXTRA_DEVICE_NAME) ?: "Unknown"
        val fingerprint = intent.getStringExtra(EXTRA_FINGERPRINT) ?: ""
        val pin = intent.getStringExtra(EXTRA_PIN) ?: "------"

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(color(R.color.pb_canvas_top), color(R.color.pb_canvas_bottom))
            )
            setPadding(dp(24))
        }

        root.addView(sectionCard().apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL

            addView(TextView(this@PairingActivity).apply {
                text = "PAIRING REQUEST"
                textSize = 11f
                letterSpacing = 0.12f
                setTypeface(Typeface.DEFAULT_BOLD)
                setTextColor(color(R.color.pb_secondary))
            })

            addView(TextView(this@PairingActivity).apply {
                text = "Trust this device?"
                textSize = 30f
                setTypeface(Typeface.create("serif", Typeface.BOLD))
                setTextColor(color(R.color.pb_surface))
                gravity = Gravity.CENTER
                setPadding(0, dp(10), 0, dp(8))
            })

            addView(TextView(this@PairingActivity).apply {
                text = "$deviceName wants to join your local clipboard mesh. Confirm the code below matches the other screen."
                textSize = 15f
                gravity = Gravity.CENTER
                setTextColor(color(R.color.pb_surface_alt))
            })

            addView(Space(this@PairingActivity).apply {
                layoutParams = LinearLayout.LayoutParams(1, dp(18))
            })

            addView(TextView(this@PairingActivity).apply {
                text = pin
                textSize = 42f
                letterSpacing = 0.18f
                gravity = Gravity.CENTER
                setTypeface(Typeface.create("monospace", Typeface.BOLD))
                setTextColor(color(R.color.pb_primary))
                background = roundedFill(color(R.color.pb_outline), color(R.color.pb_primary_dark), dp(24))
                setPadding(dp(20), dp(20), dp(20), dp(20))
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    setMargins(0, 0, 0, dp(16))
                }
            })

            addView(TextView(this@PairingActivity).apply {
                text = "Fingerprint  ${fingerprint.take(23)}"
                textSize = 12f
                setTypeface(Typeface.MONOSPACE)
                setTextColor(color(R.color.pb_surface_alt))
            })

            addView(TextView(this@PairingActivity).apply {
                text = "Only trust devices you control. This request expires in 30 seconds."
                textSize = 13f
                gravity = Gravity.CENTER
                setTextColor(color(R.color.pb_surface_alt))
                setPadding(0, dp(10), 0, 0)
            })
        })

        root.addView(Space(this).apply { layoutParams = LinearLayout.LayoutParams(1, dp(18)) })

        root.addView(LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            addView(actionButton("Deny", primary = false) {
                sendResult(deviceId, approved = false)
            }.apply {
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(Space(this@PairingActivity).apply {
                layoutParams = LinearLayout.LayoutParams(dp(12), 1)
            })
            addView(actionButton("Trust device", primary = true) {
                sendResult(deviceId, approved = true)
            }.apply {
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })
        })

        setContentView(ScrollView(this).apply { addView(root) })
        title = "ClipRelay Pairing"

        window.decorView.postDelayed({
            if (!isFinishing) {
                Toast.makeText(this, "Pairing request timed out", Toast.LENGTH_SHORT).show()
                sendResult(deviceId, approved = false)
            }
        }, 30_000L)
    }

    private fun sendResult(deviceId: String, approved: Boolean) {
        val broadcast = Intent(ACTION_PAIRING_RESULT).apply {
            putExtra(EXTRA_DEVICE_ID, deviceId)
            putExtra(EXTRA_APPROVED, approved)
            setPackage(packageName)
        }
        sendBroadcast(broadcast)
        finish()
    }

    private fun sectionCard(): LinearLayout {
        return LinearLayout(this).apply {
            background = GradientDrawable(
                GradientDrawable.Orientation.TL_BR,
                intArrayOf(color(R.color.pb_outline), color(R.color.pb_canvas_bottom))
            ).apply {
                cornerRadius = dp(28).toFloat()
                setStroke(dp(1), color(R.color.pb_primary_dark))
            }
            setPadding(dp(22))
        }
    }

    private fun actionButton(label: String, primary: Boolean, onClick: () -> Unit): AppCompatButton {
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

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).roundToInt()

    private fun color(resId: Int): Int = ContextCompat.getColor(this, resId)
}

class SettingsActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(color(R.color.pb_canvas_top), color(R.color.pb_canvas_bottom))
            )
            setPadding(dp(24))
        }

        root.addView(settingsHero())
        root.addView(verticalGap())

        root.addView(settingsCard("Sync") {
            addView(switchRow("Enable clipboard sync", true) { saveSetting("sync_enabled", it) })
            addView(switchRow("Sync text", true) { saveSetting("sync_text", it) })
            addView(switchRow("Sync images", true) { saveSetting("sync_images", it) })
            addView(switchRow("Sync files", true) { saveSetting("sync_files", it) })
        })

        root.addView(verticalGap())

        root.addView(settingsCard("Privacy") {
            addView(switchRow("Show notifications on receive", true) { saveSetting("show_receive_notification", it) })
            addView(switchRow("Require PIN confirmation for new devices", true) { saveSetting("require_tofu_confirmation", it) })
        })

        root.addView(verticalGap())

        root.addView(settingsCard("Trusted devices") {
            addView(actionButton("Manage trusted devices", primary = true) { showTrustedDevices() })
        })

        root.addView(verticalGap())

        root.addView(settingsCard("About") {
            addView(TextView(this@SettingsActivity).apply {
                text = "ClipRelay 0.1.0\nPrivate clipboard sync for nearby devices.\nTraffic stays on your local network."
                textSize = 14f
                setTextColor(color(R.color.pb_ink))
            })
        })

        setContentView(ScrollView(this).apply { addView(root) })
        title = "ClipRelay Settings"
    }

    private fun settingsHero(): LinearLayout {
        return settingsCard("Device profile") {
            addView(TextView(this@SettingsActivity).apply {
                text = "Tune the mesh"
                textSize = 30f
                setTypeface(Typeface.create("serif", Typeface.BOLD))
                setTextColor(color(R.color.pb_ink))
            })
            addView(TextView(this@SettingsActivity).apply {
                text = "These controls shape how your clipboard moves, which devices can see it, and how visible the service stays."
                textSize = 15f
                setTextColor(color(R.color.pb_muted))
                setPadding(0, dp(8), 0, 0)
            })
            addView(TextView(this@SettingsActivity).apply {
                text = "This phone shows up as ${resolvedDeviceName()}"
                textSize = 14f
                setTextColor(color(R.color.pb_ink))
                setPadding(0, dp(16), 0, dp(10))
            })
            addView(actionButton("Rename this device", primary = true) { renameDevice() })
        }
    }

    private fun settingsCard(title: String, content: LinearLayout.() -> Unit): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable(
                GradientDrawable.Orientation.TL_BR,
                intArrayOf(color(R.color.pb_surface), color(R.color.pb_surface_alt))
            ).apply {
                cornerRadius = dp(28).toFloat()
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

            addView(Space(this@SettingsActivity).apply {
                layoutParams = LinearLayout.LayoutParams(1, dp(10))
            })

            content()
        }
    }

    private fun switchRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            background = roundedFill(color(R.color.pb_surface_alt), color(R.color.pb_outline), dp(18))
            setPadding(dp(16))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                setMargins(0, 0, 0, dp(10))
            }

            addView(TextView(this@SettingsActivity).apply {
                text = label
                textSize = 15f
                setTextColor(color(R.color.pb_ink))
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            })

            addView(Switch(this@SettingsActivity).apply {
                isChecked = checked
                setOnCheckedChangeListener { _, value -> onChange(value) }
            })
        }
    }

    private fun actionButton(label: String, primary: Boolean, onClick: () -> Unit): AppCompatButton {
        return AppCompatButton(this).apply {
            text = label
            textSize = 15f
            isAllCaps = false
            setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL))
            setTextColor(
                if (primary) ContextCompat.getColor(this@SettingsActivity, android.R.color.white)
                else color(R.color.pb_ink)
            )
            background = if (primary) {
                GradientDrawable(
                    GradientDrawable.Orientation.LEFT_RIGHT,
                    intArrayOf(color(R.color.pb_primary), color(R.color.pb_primary_dark))
                ).apply {
                    cornerRadius = dp(18).toFloat()
                }
            } else {
                roundedFill(color(R.color.pb_surface_alt), color(R.color.pb_outline), dp(18))
            }
            setPadding(dp(18), dp(16), dp(18), dp(16))
            setOnClickListener { onClick() }
        }
    }

    private fun saveSetting(key: String, value: Boolean) {
        getSharedPreferences("cliprelay", MODE_PRIVATE)
            .edit()
            .putBoolean(key, value)
            .apply()
        Toast.makeText(this, "$key updated", Toast.LENGTH_SHORT).show()
    }

    private fun showTrustedDevices() {
        AlertDialog.Builder(this)
            .setTitle("Trusted Devices")
            .setMessage("(Connect devices via LAN to see them here)")
            .setPositiveButton("OK", null)
            .show()
    }

    private fun renameDevice() {
        val input = EditText(this).apply {
            setText(resolvedDeviceName())
            setSelection(text.length)
            hint = "My Phone"
        }
        AlertDialog.Builder(this)
            .setTitle("Rename device")
            .setView(input)
            .setPositiveButton("Save") { _, _ ->
                val updated = input.text?.toString()?.trim().orEmpty()
                getSharedPreferences("cliprelay", MODE_PRIVATE)
                    .edit()
                    .putString("device_name", updated)
                    .apply()
                restartSyncService()
                Toast.makeText(this, "Device name updated", Toast.LENGTH_SHORT).show()
            }
            .setNegativeButton("Cancel", null)
            .show()
    }

    private fun restartSyncService() {
        stopService(Intent(this, ClipRelayService::class.java))
        ContextCompat.startForegroundService(
            this,
            Intent(this, ClipRelayService::class.java).apply {
                action = ClipRelayService.ACTION_START
            }
        )
    }

    private fun resolvedDeviceName(): String {
        val prefs = getSharedPreferences("cliprelay", MODE_PRIVATE)
        return prefs.getString("device_name", null)
            ?.trim()
            ?.takeIf { it.isNotEmpty() }
            ?: prefs.getString("local_device_name", null)
            ?: android.os.Build.MODEL
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

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).roundToInt()

    private fun color(resId: Int): Int = ContextCompat.getColor(this, resId)
}
