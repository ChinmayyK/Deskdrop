package com.deskdrop

import android.app.Application
import androidx.appcompat.app.AppCompatDelegate
import com.google.android.material.color.DynamicColors

class DeskdropApp : Application() {
    override fun onCreate() {
        super.onCreate()
        val prefs = getSharedPreferences(packageName + "_preferences", android.content.Context.MODE_PRIVATE)
        
        // 1. Theme Mode
        val mode = prefs.getInt("theme_mode", AppCompatDelegate.MODE_NIGHT_NO)
        AppCompatDelegate.setDefaultNightMode(mode)

        // 2. Dynamic Colors (Material 3)
        val useDynamic = prefs.getBoolean("use_dynamic_colors", true)
        if (useDynamic) {
            DynamicColors.applyToActivitiesIfAvailable(this)
        }
    }
}
