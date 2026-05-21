package com.cliprelay

import android.app.Notification
import android.content.Intent
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import android.util.Log

class ClipRelayNotificationListener : NotificationListenerService() {
    companion object {
        private const val TAG = "DeskdropNotifListener"
        private var instance: ClipRelayNotificationListener? = null

        fun getActiveInstance(): ClipRelayNotificationListener? = instance

        fun triggerCallAction(action: String): Boolean {
            val inst = instance ?: return false
            return inst.handleCallAction(action)
        }
    }

    override fun onCreate() {
        super.onCreate()
        instance = this
        Log.i(TAG, "Notification listener service created")
    }

    override fun onDestroy() {
        super.onDestroy()
        if (instance == this) {
            instance = null
        }
        Log.i(TAG, "Notification listener service destroyed")
    }

    override fun onListenerConnected() {
        super.onListenerConnected()
        instance = this
        Log.i(TAG, "Notification listener connected")
    }

    override fun onListenerDisconnected() {
        super.onListenerDisconnected()
        if (instance == this) {
            instance = null
        }
        Log.i(TAG, "Notification listener disconnected")
    }

    fun handleCallAction(actionWanted: String): Boolean {
        val activeNotifs = activeNotifications ?: return false
        Log.i(TAG, "Searching active notifications for call actions... count=${activeNotifs.size}")
        for (sbn in activeNotifs) {
            val notif = sbn.notification ?: continue
            val pkg = sbn.packageName ?: ""
            val category = notif.category ?: ""
            
            // Check if this is a call notification or from dialer/phone app
            val isCall = category == Notification.CATEGORY_CALL || 
                         pkg.contains("dialer") || 
                         pkg.contains("phone") || 
                         pkg.contains("telephony")
                         
            if (isCall) {
                val actions = notif.actions ?: continue
                Log.d(TAG, "Found call notification from package: $pkg with ${actions.size} actions")
                for (action in actions) {
                    val title = action.title?.toString()?.lowercase() ?: continue
                    Log.d(TAG, "Examining action title: $title")
                    
                    if (actionWanted == "accept") {
                        if (title.contains("answer") || title.contains("accept") || title.contains("call")) {
                            try {
                                action.actionIntent.send()
                                Log.i(TAG, "Triggered accept call action successfully via PendingIntent!")
                                return true
                            } catch (e: Exception) {
                                Log.e(TAG, "Failed to send accept call action PendingIntent", e)
                            }
                        }
                    } else if (actionWanted == "decline") {
                        if (title.contains("decline") || title.contains("reject") || title.contains("end") || title.contains("hang") || title.contains("dismiss")) {
                            try {
                                action.actionIntent.send()
                                Log.i(TAG, "Triggered decline call action successfully via PendingIntent!")
                                return true
                            } catch (e: Exception) {
                                Log.e(TAG, "Failed to send decline call action PendingIntent", e)
                            }
                        }
                    }
                }
            }
        }
        Log.w(TAG, "No matching call notification found for action: $actionWanted")
        return false
    }
}
