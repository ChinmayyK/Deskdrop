package com.deskdrop.ui

import com.deskdrop.ui.theme.*

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.rounded.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.deskdrop.ui.theme.CRBackground
import com.deskdrop.ui.theme.CRTheme
import com.deskdrop.ui.theme.CRTypography
import com.deskdrop.ui.theme.crGlassCard
import com.deskdrop.ui.theme.CRSwitch

fun getLocalIpAddress(): String {
    try {
        val en = java.net.NetworkInterface.getNetworkInterfaces()
        while (en.hasMoreElements()) {
            val intf = en.nextElement()
            val enumIpAddr = intf.inetAddresses
            while (enumIpAddr.hasMoreElements()) {
                val inetAddress = enumIpAddr.nextElement()
                if (!inetAddress.isLoopbackAddress && inetAddress is java.net.Inet4Address) {
                    return inetAddress.hostAddress ?: ""
                }
            }
        }
    } catch (ex: Exception) {
        // Ignore
    }
    return "Unknown IP"
}

@Composable
fun SettingsTab(
    isDark: Boolean,
    isServiceRunning: Boolean,
    isSyncEnabled: Boolean,
    syncText: Boolean,
    syncImages: Boolean,
    syncFiles: Boolean,
    callContinuityEnabled: Boolean,
    notificationMirroringEnabled: Boolean,
    autoForwardSms: Boolean,
    autoForwardScreenshots: Boolean,
    deviceName: String,
    deviceId: String,
    peers: List<com.deskdrop.PeerSnapshot>,
    onSyncEnabledChange: (Boolean) -> Unit,
    onSyncTextChange: (Boolean) -> Unit,
    onSyncImagesChange: (Boolean) -> Unit,
    onSyncFilesChange: (Boolean) -> Unit,
    onCallContinuityChange: (Boolean) -> Unit,
    onNotificationMirroringChange: (Boolean) -> Unit,
    onAutoForwardSmsChange: (Boolean) -> Unit,
    onAutoForwardScreenshotsChange: (Boolean) -> Unit,
    onDarkModeChange: (Boolean) -> Unit,
    onForgetDevice: (String) -> Unit,
    onStartSync: () -> Unit,
    onResumeSync: () -> Unit,
    onScanNow: () -> Unit,
    onActionPauseSync: () -> Unit,
    onActionDisconnectAll: () -> Unit,
    onActionStopService: () -> Unit,
    onOpenDiagnostics: () -> Unit,
    onBatterySettingsClicked: () -> Unit = {},
    onStorageSettingsClicked: () -> Unit = {},
    onNotificationSettingsClicked: () -> Unit = {}
) {
    val haptic = LocalHapticFeedback.current
    val listState = rememberLazyListState()

    Column(modifier = Modifier.fillMaxSize()) {
            
        Text(
            text = "Settings",
            style = CRTypography.h2,
            color = CRTheme.textHigh(isDark),
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
        )

        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, bottom = 120.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp)
            ) {
                item {
                    SettingsSection(
                        isDark = isDark,
                        title = "Service Controls",
                        accentColor = CRTheme.brandElectric,
                        icon = Icons.Rounded.Settings
                    ) {
                        Column {
                            if (isSyncEnabled) {
                                SettingsActionTile(isDark = isDark, icon = Icons.Rounded.Pause, label = "Pause Sync", color = CRTheme.accentAmber, onClick = onActionPauseSync)
                            } else {
                                SettingsActionTile(isDark = isDark, icon = Icons.Rounded.PlayArrow, label = "Resume Sync", color = CRTheme.accentGreen, onClick = onResumeSync)
                            }
                            HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                            if (!isServiceRunning) {
                                SettingsActionTile(isDark = isDark, icon = Icons.Rounded.PlayCircle, label = "Start Service", color = CRTheme.accentGreen, onClick = onStartSync)
                                HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                            }
                            SettingsActionTile(isDark = isDark, icon = Icons.Rounded.Search, label = "Scan Now", color = CRTheme.brandCyan, onClick = onScanNow)
                            HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                            SettingsActionTile(isDark = isDark, icon = Icons.Rounded.LinkOff, label = "Disconnect All", color = CRTheme.brandPink, onClick = onActionDisconnectAll)
                            HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                            SettingsActionTile(isDark = isDark, icon = Icons.Rounded.Stop, label = "Stop Service", color = CRTheme.accentRed, onClick = onActionStopService)
                            HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                            SettingsActionTile(isDark = isDark, icon = Icons.Rounded.Info, label = "Diagnostics", color = CRTheme.brandElectric, onClick = onOpenDiagnostics)
                        }
                    }
                }

                item {
                    // Aboutfile Card Hero
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .crGlassCard(isDark = isDark, cornerRadius = 24.dp)
                            .clickable {
                                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)

                            }
                    ) {
                        Column(
                            modifier = Modifier.fillMaxWidth().padding(32.dp),
                            horizontalAlignment = Alignment.CenterHorizontally
                        ) {
                            Box(
                                modifier = Modifier
                                    .size(80.dp)
                                    .clip(CircleShape)
                                    .background(CRTheme.blueSoft.copy(alpha = 0.1f))
                                    .border(2.dp, CRTheme.blueSoft.copy(alpha = 0.5f), CircleShape),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    text = deviceName.take(1).uppercase(),
                                    fontSize = 32.sp,
                                    fontWeight = FontWeight.Bold,
                                    color = CRTheme.blueSoft
                                )
                            }
                            Spacer(modifier = Modifier.height(20.dp))
                            Text(text = deviceName, style = CRTypography.h2, color = CRTheme.textHigh(isDark))
                            Spacer(modifier = Modifier.height(12.dp))
                            
                            Row(
                                modifier = Modifier
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(CRTheme.surface(isDark).copy(alpha = 0.5f))
                                    .border(1.dp, CRTheme.stroke(isDark), RoundedCornerShape(12.dp))
                                    .padding(horizontal = 14.dp, vertical = 6.dp),
                                horizontalArrangement = Arrangement.Center,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = "IP: ${getLocalIpAddress()}",
                                    fontSize = 12.sp,
                                    fontFamily = FontFamily.Monospace,
                                    color = CRTheme.textMedium(isDark),
                                    fontWeight = FontWeight.Medium
                                )
                                Spacer(modifier = Modifier.width(12.dp))
                                Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(CRTheme.statusGreen))
                                Spacer(modifier = Modifier.width(8.dp))
                                Text(
                                    text = "ACTIVE",
                                    style = CRTypography.caption,
                                    color = CRTheme.textHigh(isDark)
                                )
                            }
                            
                            Spacer(modifier = Modifier.height(16.dp))
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(Icons.Rounded.Edit, contentDescription = "Edit", tint = CRTheme.blueSoft, modifier = Modifier.size(14.dp))
                                Spacer(modifier = Modifier.width(6.dp))
                                Text("TAP TO EDIT NAME", style = CRTypography.caption, color = CRTheme.blueSoft)
                            }
                        }
                    }
                }

                item {
                    SettingsSection(
                        isDark = isDark,
                        title = "Appearance",
                        accentColor = CRTheme.blueSoft,
                        icon = Icons.Rounded.Brush
                    ) {
                        SettingsSwitchRow(
                            isDark = isDark,
                            icon = Icons.Rounded.DarkMode,
                            title = "Dark Mode",
                            subtitle = "Pure black theme for OLED displays",
                            checked = isDark,
                            onCheckedChange = onDarkModeChange
                        )
                    }
                }

                item {
                    SettingsSection(
                        isDark = isDark,
                        title = "Clipboard Sync",
                        accentColor = CRTheme.statusGreen,
                        icon = Icons.Rounded.Sync
                    ) {
                        Column {
                            SettingsSwitchRow(
                                isDark = isDark,
                                icon = Icons.Rounded.Link,
                                title = "Enable Sync",
                                subtitle = "Master switch to pause all transfers",
                                checked = isSyncEnabled,
                                onCheckedChange = onSyncEnabledChange
                            )
                            
                            AnimatedVisibility(
                                visible = isSyncEnabled,
                                enter = expandVertically() + fadeIn(),
                                exit = shrinkVertically() + fadeOut()
                            ) {
                                Column {
                                    HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                                    SettingsSwitchRow(
                                        isDark = isDark,
                                        icon = Icons.Rounded.TextFields,
                                        title = "Sync Text",
                                        subtitle = null,
                                        checked = syncText,
                                        onCheckedChange = onSyncTextChange
                                    )
                                    HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                                    SettingsSwitchRow(
                                        isDark = isDark,
                                        icon = Icons.Rounded.Image,
                                        title = "Sync Images",
                                        subtitle = null,
                                        checked = syncImages,
                                        onCheckedChange = onSyncImagesChange
                                    )
                                    HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                                    SettingsSwitchRow(
                                        isDark = isDark,
                                        icon = Icons.Rounded.FilePresent,
                                        title = "Sync Files",
                                        subtitle = "Saved directly to Downloads folder",
                                        checked = syncFiles,
                                        onCheckedChange = onSyncFilesChange
                                    )
                                }
                            }
                        }
                    }
                }

                item {
                    SettingsSection(
                        isDark = isDark,
                        title = "Ambient Continuity",
                        accentColor = CRTheme.statusAmber,
                        icon = Icons.Rounded.Star
                    ) {
                        Column {
                            SettingsSwitchRow(
                                isDark = isDark,
                                icon = Icons.Rounded.Message,
                                title = "Auto-forward SMS 2FA",
                                subtitle = "Automatically copies 2FA codes to Mac clipboard",
                                checked = autoForwardSms,
                                onCheckedChange = onAutoForwardSmsChange
                            )
                            HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                            SettingsSwitchRow(
                                isDark = isDark,
                                icon = Icons.Rounded.CameraAlt,
                                title = "Screenshot Sync",
                                subtitle = "Instantly sends Android screenshots to your Mac",
                                checked = autoForwardScreenshots,
                                onCheckedChange = onAutoForwardScreenshotsChange
                            )
                            HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                            SettingsSwitchRow(
                                isDark = isDark,
                                icon = Icons.Rounded.Phone,
                                title = "Call Continuity",
                                subtitle = "Requires Phone, Contacts, and Call Log permissions",
                                checked = callContinuityEnabled,
                                onCheckedChange = onCallContinuityChange
                            )
                            HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 72.dp))
                            SettingsSwitchRow(
                                isDark = isDark,
                                icon = Icons.Rounded.Notifications,
                                title = "Notification Mirroring",
                                subtitle = "Mirror Android notifications to your Mac",
                                checked = notificationMirroringEnabled,
                                onCheckedChange = onNotificationMirroringChange
                            )
                        }
                    }
                }

                item {
                    SettingsSection(
                        isDark = isDark,
                        title = "Saved Devices",
                        accentColor = CRTheme.cyanSoft,
                        icon = Icons.Rounded.Devices
                    ) {
                        Column {
                            val savedPeers = peers.filter { it.remembered || it.trusted }
                            if (savedPeers.isEmpty()) {
                                Box(modifier = Modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) {
                                    Text(
                                        text = "No saved devices.",
                                        style = CRTypography.bodyMedium,
                                        color = CRTheme.textMedium(isDark)
                                    )
                                }
                            } else {
                                savedPeers.forEachIndexed { index, peer ->
                                    Row(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .padding(horizontal = 24.dp, vertical = 20.dp),
                                        verticalAlignment = Alignment.CenterVertically
                                    ) {
                                        Box(
                                            modifier = Modifier.size(40.dp).clip(CircleShape).background(CRTheme.surface(isDark)),
                                            contentAlignment = Alignment.Center
                                        ) {
                                            Text(peer.name.take(1).uppercase(), style = CRTypography.h2, color = CRTheme.textHigh(isDark))
                                        }
                                        Spacer(modifier = Modifier.width(16.dp))
                                        Column(modifier = Modifier.weight(1f)) {
                                            Text(text = peer.name, style = CRTypography.bodyMedium, color = CRTheme.textHigh(isDark))
                                            Spacer(modifier = Modifier.height(4.dp))
                                            Row(verticalAlignment = Alignment.CenterVertically) {
                                                Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(if (peer.isConnected) CRTheme.statusGreen else CRTheme.textMedium(isDark)))
                                                Spacer(modifier = Modifier.width(6.dp))
                                                Text(text = if (peer.isConnected) "Connected" else "Offline", fontSize = 12.sp, color = CRTheme.textMedium(isDark))
                                            }
                                        }
                                        Box(
                                            modifier = Modifier
                                                .clip(RoundedCornerShape(8.dp))
                                                .background(CRTheme.statusRed.copy(alpha = 0.1f))
                                                .clickable { 
                                                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                                    onForgetDevice(peer.id) 
                                                }
                                                .padding(horizontal = 12.dp, vertical = 8.dp)
                                        ) {
                                            Text(
                                                text = "FORGET",
                                                style = CRTypography.caption,
                                                color = CRTheme.statusRed
                                            )
                                        }
                                    }
                                    if (index < savedPeers.size - 1) {
                                        HorizontalDivider(color = CRTheme.stroke(isDark), modifier = Modifier.padding(start = 80.dp))
                                    }
                                }
                            }
                        }
                    }
                }

                item {
                    SettingsSection(
                        isDark = isDark,
                        title = "Background Execution",
                        accentColor = CRTheme.statusAmber,
                        icon = Icons.Rounded.BatteryAlert
                    ) {
                        Column(modifier = Modifier.padding(24.dp)) {
                            Row(verticalAlignment = Alignment.Top) {
                                Box(
                                    modifier = Modifier.size(40.dp).clip(CircleShape).background(CRTheme.statusAmber.copy(alpha = 0.1f)),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Icon(Icons.Rounded.Warning, contentDescription = "Warning", tint = CRTheme.statusAmber, modifier = Modifier.size(20.dp))
                                }
                                Spacer(modifier = Modifier.width(16.dp))
                                Text(
                                    text = "To ensure Deskdrop stays alive in the background and receives clips instantly, disable battery optimization for this app.",
                                    style = CRTypography.bodyMedium,
                                    color = CRTheme.textMedium(isDark),
                                    lineHeight = 22.sp
                                )
                            }
                            Spacer(modifier = Modifier.height(24.dp))
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(48.dp)
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(CRTheme.textHigh(isDark))
                                    .clickable {
                                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                        onBatterySettingsClicked()
                                    },
                                contentAlignment = Alignment.Center
                            ) {
                                Text("OPEN BATTERY SETTINGS", style = CRTypography.label, color = CRTheme.bg(isDark))
                            }
                        }
                    }
                }

                // Remote Explorer Storage Permissions Section
                item {
                    SettingsSection(
                        isDark = isDark,
                        title = "Remote File Explorer Access",
                        accentColor = CRTheme.statusGreen,
                        icon = Icons.Rounded.Folder
                    ) {
                        Column(modifier = Modifier.padding(24.dp)) {
                            Row(verticalAlignment = Alignment.Top) {
                                Box(
                                    modifier = Modifier.size(40.dp).clip(CircleShape).background(CRTheme.statusGreen.copy(alpha = 0.1f)),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Icon(Icons.Rounded.Folder, contentDescription = "Folder", tint = CRTheme.statusGreen, modifier = Modifier.size(20.dp))
                                }
                                Spacer(modifier = Modifier.width(16.dp))
                                Text(
                                    text = "To browse and pull all files across your phone (Photos, Documents, APKs, Downloads) directly from your Mac, grant All Files Access.",
                                    style = CRTypography.bodyMedium,
                                    color = CRTheme.textMedium(isDark),
                                    lineHeight = 22.sp
                                )
                            }
                            Spacer(modifier = Modifier.height(24.dp))
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(48.dp)
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(CRTheme.textHigh(isDark))
                                    .clickable {
                                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                        onStorageSettingsClicked()
                                    },
                                contentAlignment = Alignment.Center
                            ) {
                                Text("GRANT ALL FILES ACCESS", style = CRTypography.label, color = CRTheme.bg(isDark))
                            }
                        }
                    }
                }

                // Status Bar Notification Hiding Section
                item {
                    SettingsSection(
                        isDark = isDark,
                        title = "Status Bar Notification",
                        accentColor = CRTheme.blueSoft,
                        icon = Icons.Rounded.NotificationsOff
                    ) {
                        Column(modifier = Modifier.padding(24.dp)) {
                            Row(verticalAlignment = Alignment.Top) {
                                Box(
                                    modifier = Modifier.size(40.dp).clip(CircleShape).background(CRTheme.blueSoft.copy(alpha = 0.1f)),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Icon(Icons.Rounded.NotificationsOff, contentDescription = null, tint = CRTheme.blueSoft, modifier = Modifier.size(20.dp))
                                }
                                Spacer(modifier = Modifier.width(16.dp))
                                Text(
                                    text = "Deskdrop runs an ultra-efficient background service so clips sync instantly. You can minimize or hide the top status bar icon in system notification settings without affecting sync.",
                                    style = CRTypography.bodyMedium,
                                    color = CRTheme.textMedium(isDark),
                                    lineHeight = 22.sp
                                )
                            }
                            Spacer(modifier = Modifier.height(24.dp))
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(48.dp)
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(CRTheme.textHigh(isDark))
                                    .clickable {
                                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                        onNotificationSettingsClicked()
                                    },
                                contentAlignment = Alignment.Center
                            ) {
                                Text("HIDE STATUS BAR ICON", style = CRTypography.label, color = CRTheme.bg(isDark))
                            }
                        }
                    }
                }

                item {
                    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth().padding(vertical = 32.dp)) {
                        Box(
                            modifier = Modifier
                                .size(64.dp)
                                .clip(RoundedCornerShape(16.dp))
                                .background(CRTheme.glass(isDark))
                                .border(1.dp, CRTheme.stroke(isDark), RoundedCornerShape(16.dp)),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(Icons.Rounded.EnergySavingsLeaf, contentDescription = "Deskdrop", tint = CRTheme.statusGreen, modifier = Modifier.size(32.dp))
                        }
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(text = "Deskdrop", style = CRTypography.h2, color = CRTheme.textHigh(isDark))
                        Spacer(modifier = Modifier.height(4.dp))
                        Text(text = "VERSION 1.0.0", style = CRTypography.caption, color = CRTheme.textMedium(isDark))
                        Spacer(modifier = Modifier.height(24.dp))
                        Text(
                            text = "NO CLOUD. NO ACCOUNT. NO TELEMETRY.",
                            style = CRTypography.caption,
                            color = CRTheme.textHigh(isDark),
                            textAlign = TextAlign.Center
                        )
                    }
                }
        }
    }
}

@Composable
fun SettingsSection(
    isDark: Boolean,
    title: String,
    accentColor: Color,
    icon: ImageVector,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(modifier = Modifier.size(8.dp).clip(CircleShape).background(accentColor))
            Spacer(modifier = Modifier.width(12.dp))
            Text(
                text = title.uppercase(),
                style = CRTypography.label,
                color = CRTheme.textMedium(isDark)
            )
        }
        
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .crGlassCard(isDark = isDark, cornerRadius = 24.dp)
        ) {
            Column(modifier = Modifier.fillMaxWidth()) {
                content()
            }
        }
    }
}

@Composable
fun SettingsSwitchRow(
    isDark: Boolean,
    icon: ImageVector,
    title: String,
    subtitle: String?,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    val haptic = LocalHapticFeedback.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null
            ) {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onCheckedChange(!checked)
            }
            .padding(horizontal = 24.dp, vertical = 20.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier.size(40.dp).clip(CircleShape).background(CRTheme.surface(isDark)),
            contentAlignment = Alignment.Center
        ) {
            Icon(imageVector = icon, contentDescription = title, tint = CRTheme.textHigh(isDark), modifier = Modifier.size(20.dp))
        }
        Spacer(modifier = Modifier.width(16.dp))
        
        Column(modifier = Modifier.weight(1f)) {
            Text(text = title, style = CRTypography.bodyMedium, color = CRTheme.textHigh(isDark))
            if (subtitle != null) {
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = subtitle,
                    fontSize = 13.sp,
                    color = CRTheme.textMedium(isDark),
                    lineHeight = 18.sp
                )
            }
        }
        Spacer(modifier = Modifier.width(16.dp))
        CRSwitch(checked = checked, isDark = isDark)
    }
}

@Composable
fun SettingsActionTile(
    isDark: Boolean,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    color: androidx.compose.ui.graphics.Color,
    onClick: () -> Unit
) {
    val haptic = androidx.compose.ui.platform.LocalHapticFeedback.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable {
                haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                onClick()
            }
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .size(36.dp)
                .background(color.copy(alpha = 0.15f), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Icon(imageVector = icon, contentDescription = null, tint = color, modifier = Modifier.size(20.dp))
        }
        Spacer(modifier = Modifier.width(16.dp))
        Text(
            text = label,
            style = CRTypography.label,
            color = CRTheme.textHigh(isDark)
        )
    }
}
