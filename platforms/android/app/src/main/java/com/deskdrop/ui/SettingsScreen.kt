package com.deskdrop.ui

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
fun SettingsScreen(
    deviceName: String,
    deviceId: String,
    syncEnabled: Boolean,
    syncText: Boolean,
    syncImages: Boolean,
    syncFiles: Boolean,
    isDarkMode: Boolean,
    peers: List<com.deskdrop.PeerSnapshot>,
    onSyncEnabledChange: (Boolean) -> Unit,
    onSyncTextChange: (Boolean) -> Unit,
    onSyncImagesChange: (Boolean) -> Unit,
    onSyncFilesChange: (Boolean) -> Unit,
    onDarkModeChange: (Boolean) -> Unit,
    onRenameClicked: () -> Unit,
    onBatterySettingsClicked: () -> Unit,
    onForgetDevice: (String) -> Unit,
    onBack: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    val listState = rememberLazyListState()

    val titleScale by remember {
        derivedStateOf {
            val offset = listState.firstVisibleItemScrollOffset
            if (listState.firstVisibleItemIndex == 0) {
                1f - (offset / 800f).coerceIn(0f, 0.2f)
            } else {
                0.8f
            }
        }
    }

    CRBackground(isDark = isDarkMode) {
        Column(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
            
            // Premium Top Bar
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 24.dp, vertical = 16.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box(
                    modifier = Modifier
                        .size(48.dp)
                        .clip(CircleShape)
                        .background(CRTheme.glass(isDarkMode))
                        .border(1.dp, CRTheme.stroke(isDarkMode), CircleShape)
                        .clickable {
                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                            onBack()
                        },
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = "Back",
                        tint = CRTheme.textHigh(isDarkMode),
                        modifier = Modifier.size(24.dp)
                    )
                }
                Spacer(modifier = Modifier.width(20.dp))
                Text(
                    text = "Settings",
                    style = CRTypography.h1,
                    color = CRTheme.textHigh(isDarkMode),
                    modifier = Modifier.scale(titleScale)
                )
            }

            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(top = 8.dp, start = 24.dp, end = 24.dp, bottom = 64.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp)
            ) {
                item {
                    // Profile Card Hero
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .crGlassCard(isDark = isDarkMode, cornerRadius = 24.dp)
                            .clickable {
                                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                onRenameClicked()
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
                            Text(text = deviceName, style = CRTypography.h2, color = CRTheme.textHigh(isDarkMode))
                            Spacer(modifier = Modifier.height(12.dp))
                            
                            Row(
                                modifier = Modifier
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(CRTheme.surface(isDarkMode).copy(alpha = 0.5f))
                                    .border(1.dp, CRTheme.stroke(isDarkMode), RoundedCornerShape(12.dp))
                                    .padding(horizontal = 14.dp, vertical = 6.dp),
                                horizontalArrangement = Arrangement.Center,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = "IP: ${getLocalIpAddress()}",
                                    fontSize = 12.sp,
                                    fontFamily = FontFamily.Monospace,
                                    color = CRTheme.textMedium(isDarkMode),
                                    fontWeight = FontWeight.Medium
                                )
                                Spacer(modifier = Modifier.width(12.dp))
                                Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(CRTheme.statusGreen))
                                Spacer(modifier = Modifier.width(8.dp))
                                Text(
                                    text = "ACTIVE",
                                    style = CRTypography.caption,
                                    color = CRTheme.textHigh(isDarkMode)
                                )
                            }
                            
                            Spacer(modifier = Modifier.height(16.dp))
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(Icons.Rounded.Edit, contentDescription = null, tint = CRTheme.blueSoft, modifier = Modifier.size(14.dp))
                                Spacer(modifier = Modifier.width(6.dp))
                                Text("TAP TO EDIT NAME", style = CRTypography.caption, color = CRTheme.blueSoft)
                            }
                        }
                    }
                }

                item {
                    SettingsSection(
                        isDark = isDarkMode,
                        title = "Appearance",
                        accentColor = CRTheme.blueSoft,
                        icon = Icons.Rounded.Brush
                    ) {
                        SettingsSwitchRow(
                            isDark = isDarkMode,
                            icon = Icons.Rounded.DarkMode,
                            title = "Dark Mode",
                            subtitle = "Pure black theme for OLED displays",
                            checked = isDarkMode,
                            onCheckedChange = onDarkModeChange
                        )
                    }
                }

                item {
                    SettingsSection(
                        isDark = isDarkMode,
                        title = "Clipboard Sync",
                        accentColor = CRTheme.statusGreen,
                        icon = Icons.Rounded.Sync
                    ) {
                        Column {
                            SettingsSwitchRow(
                                isDark = isDarkMode,
                                icon = Icons.Rounded.Link,
                                title = "Enable Sync",
                                subtitle = "Master switch to pause all transfers",
                                checked = syncEnabled,
                                onCheckedChange = onSyncEnabledChange
                            )
                            
                            AnimatedVisibility(
                                visible = syncEnabled,
                                enter = expandVertically() + fadeIn(),
                                exit = shrinkVertically() + fadeOut()
                            ) {
                                Column {
                                    HorizontalDivider(color = CRTheme.stroke(isDarkMode), modifier = Modifier.padding(start = 72.dp))
                                    SettingsSwitchRow(
                                        isDark = isDarkMode,
                                        icon = Icons.Rounded.TextFields,
                                        title = "Sync Text",
                                        subtitle = null,
                                        checked = syncText,
                                        onCheckedChange = onSyncTextChange
                                    )
                                    HorizontalDivider(color = CRTheme.stroke(isDarkMode), modifier = Modifier.padding(start = 72.dp))
                                    SettingsSwitchRow(
                                        isDark = isDarkMode,
                                        icon = Icons.Rounded.Image,
                                        title = "Sync Images",
                                        subtitle = null,
                                        checked = syncImages,
                                        onCheckedChange = onSyncImagesChange
                                    )
                                    HorizontalDivider(color = CRTheme.stroke(isDarkMode), modifier = Modifier.padding(start = 72.dp))
                                    SettingsSwitchRow(
                                        isDark = isDarkMode,
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
                        isDark = isDarkMode,
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
                                        color = CRTheme.textMedium(isDarkMode)
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
                                            modifier = Modifier.size(40.dp).clip(CircleShape).background(CRTheme.surface(isDarkMode)),
                                            contentAlignment = Alignment.Center
                                        ) {
                                            Text(peer.name.take(1).uppercase(), style = CRTypography.h2, color = CRTheme.textHigh(isDarkMode))
                                        }
                                        Spacer(modifier = Modifier.width(16.dp))
                                        Column(modifier = Modifier.weight(1f)) {
                                            Text(text = peer.name, style = CRTypography.bodyMedium, color = CRTheme.textHigh(isDarkMode))
                                            Spacer(modifier = Modifier.height(4.dp))
                                            Row(verticalAlignment = Alignment.CenterVertically) {
                                                Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(if (peer.isConnected) CRTheme.statusGreen else CRTheme.textMedium(isDarkMode)))
                                                Spacer(modifier = Modifier.width(6.dp))
                                                Text(text = if (peer.isConnected) "Connected" else "Offline", fontSize = 12.sp, color = CRTheme.textMedium(isDarkMode))
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
                                        HorizontalDivider(color = CRTheme.stroke(isDarkMode), modifier = Modifier.padding(start = 80.dp))
                                    }
                                }
                            }
                        }
                    }
                }

                item {
                    SettingsSection(
                        isDark = isDarkMode,
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
                                    Icon(Icons.Rounded.Warning, contentDescription = null, tint = CRTheme.statusAmber, modifier = Modifier.size(20.dp))
                                }
                                Spacer(modifier = Modifier.width(16.dp))
                                Text(
                                    text = "To ensure Deskdrop stays alive in the background and receives clips instantly, disable battery optimization for this app.",
                                    style = CRTypography.bodyMedium,
                                    color = CRTheme.textMedium(isDarkMode),
                                    lineHeight = 22.sp
                                )
                            }
                            Spacer(modifier = Modifier.height(24.dp))
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(48.dp)
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(CRTheme.textHigh(isDarkMode))
                                    .clickable {
                                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                        onBatterySettingsClicked()
                                    },
                                contentAlignment = Alignment.Center
                            ) {
                                Text("OPEN BATTERY SETTINGS", style = CRTypography.label, color = CRTheme.bg(isDarkMode))
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
                                .background(CRTheme.glass(isDarkMode))
                                .border(1.dp, CRTheme.stroke(isDarkMode), RoundedCornerShape(16.dp)),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(Icons.Rounded.EnergySavingsLeaf, contentDescription = null, tint = CRTheme.statusGreen, modifier = Modifier.size(32.dp))
                        }
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(text = "Deskdrop", style = CRTypography.h2, color = CRTheme.textHigh(isDarkMode))
                        Spacer(modifier = Modifier.height(4.dp))
                        Text(text = "VERSION 1.0.0", style = CRTypography.caption, color = CRTheme.textMedium(isDarkMode))
                        Spacer(modifier = Modifier.height(24.dp))
                        Text(
                            text = "NO CLOUD. NO ACCOUNT. NO TELEMETRY.",
                            style = CRTypography.caption,
                            color = CRTheme.textHigh(isDarkMode),
                            textAlign = TextAlign.Center
                        )
                    }
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
            Icon(imageVector = icon, contentDescription = null, tint = CRTheme.textHigh(isDark), modifier = Modifier.size(20.dp))
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
