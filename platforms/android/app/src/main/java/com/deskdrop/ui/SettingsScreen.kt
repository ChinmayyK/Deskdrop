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
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Face
import androidx.compose.material.icons.filled.List
import androidx.compose.material.icons.filled.Home
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
import com.deskdrop.ui.theme.crCard
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
    onSyncEnabledChange: (Boolean) -> Unit,
    onSyncTextChange: (Boolean) -> Unit,
    onSyncImagesChange: (Boolean) -> Unit,
    onSyncFilesChange: (Boolean) -> Unit,
    onDarkModeChange: (Boolean) -> Unit,
    onRenameClicked: () -> Unit,
    onBatterySettingsClicked: () -> Unit,
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
                        .border(0.5.dp, CRTheme.stroke(isDarkMode))
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
                    fontSize = 32.sp,
                    fontWeight = FontWeight.Light,
                    color = CRTheme.textHigh(isDarkMode),
                    letterSpacing = (-1).sp,
                    modifier = Modifier.scale(titleScale)
                )
            }

            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(top = 8.dp, start = 24.dp, end = 24.dp, bottom = 64.dp),
                verticalArrangement = Arrangement.spacedBy(28.dp)
            ) {
                item {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .crCard(isDarkMode, cornerRadius = 0.dp, highlighted = false)
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
                                    .border(1.dp, CRTheme.textHigh(isDarkMode)),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    text = deviceName.take(1).uppercase(),
                                    fontSize = 42.sp,
                                    fontWeight = FontWeight.Light,
                                    color = CRTheme.textHigh(isDarkMode)
                                )
                            }
                            Spacer(modifier = Modifier.height(20.dp))
                            Text(text = deviceName, fontSize = 20.sp, fontWeight = FontWeight.Medium, color = CRTheme.textHigh(isDarkMode))
                            Spacer(modifier = Modifier.height(12.dp))
                            
                            Row(
                                modifier = Modifier
                                    .border(0.5.dp, CRTheme.stroke(isDarkMode))
                                    .padding(horizontal = 14.dp, vertical = 6.dp),
                                horizontalArrangement = Arrangement.Center,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = "IP: ${getLocalIpAddress()}",
                                    fontSize = 11.sp,
                                    fontFamily = FontFamily.Monospace,
                                    color = CRTheme.textMedium(isDarkMode),
                                    fontWeight = FontWeight.Medium
                                )
                                Spacer(modifier = Modifier.width(12.dp))
                                Box(modifier = Modifier.size(4.dp).background(CRTheme.stroke(isDarkMode)))
                                Spacer(modifier = Modifier.width(12.dp))
                                Text(
                                    text = "CORE ACTIVE",
                                    fontSize = 10.sp,
                                    fontWeight = FontWeight.Medium,
                                    color = CRTheme.textHigh(isDarkMode),
                                    letterSpacing = 1.sp
                                )
                            }
                            
                            Spacer(modifier = Modifier.height(16.dp))
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(Icons.Default.Edit, contentDescription = null, tint = CRTheme.textMedium(isDarkMode), modifier = Modifier.size(12.dp))
                                Spacer(modifier = Modifier.width(6.dp))
                                Text("TAP TO EDIT NAME", fontSize = 10.sp, fontWeight = FontWeight.Medium, color = CRTheme.textMedium(isDarkMode), letterSpacing = 1.sp)
                            }
                        }
                    }
                }

                item {
                    SettingsCategory(
                        isDark = isDarkMode,
                        title = "Appearance",
                        icon = Icons.Default.Info
                    ) {
                        SettingsSwitchRow(
                            isDark = isDarkMode,
                            icon = Icons.Default.Info,
                            title = "Dark mode",
                            subtitle = "Deep black high-contrast theme",
                            checked = isDarkMode,
                            onCheckedChange = onDarkModeChange
                        )
                    }
                }

                item {
                    SettingsCategory(
                        isDark = isDarkMode,
                        title = "Clipboard Sync",
                        icon = Icons.Default.Refresh
                    ) {
                        Column {
                            SettingsSwitchRow(
                                isDark = isDarkMode,
                                icon = Icons.Default.Refresh,
                                title = "Enable sync",
                                subtitle = "Master switch for all features",
                                checked = syncEnabled,
                                onCheckedChange = onSyncEnabledChange
                            )
                            
                            if (syncEnabled) {
                                HorizontalDivider(color = CRTheme.stroke(isDarkMode), thickness = 0.5.dp, modifier = Modifier.padding(start = 72.dp))
                                SettingsSwitchRow(
                                    isDark = isDarkMode,
                                    icon = Icons.Default.Edit,
                                    title = "Sync text",
                                    subtitle = null,
                                    checked = syncText,
                                    onCheckedChange = onSyncTextChange
                                )
                                HorizontalDivider(color = CRTheme.stroke(isDarkMode), thickness = 0.5.dp, modifier = Modifier.padding(start = 72.dp))
                                SettingsSwitchRow(
                                    isDark = isDarkMode,
                                    icon = Icons.Default.Face,
                                    title = "Sync images",
                                    subtitle = null,
                                    checked = syncImages,
                                    onCheckedChange = onSyncImagesChange
                                )
                                HorizontalDivider(color = CRTheme.stroke(isDarkMode), thickness = 0.5.dp, modifier = Modifier.padding(start = 72.dp))
                                SettingsSwitchRow(
                                    isDark = isDarkMode,
                                    icon = Icons.Default.List,
                                    title = "Sync files",
                                    subtitle = "Saved directly to Downloads",
                                    checked = syncFiles,
                                    onCheckedChange = onSyncFilesChange
                                )
                            }
                        }
                    }
                }

                item {
                    SettingsCategory(
                        isDark = isDarkMode,
                        title = "Background Execution",
                        icon = Icons.Default.Warning
                    ) {
                        Column(modifier = Modifier.padding(24.dp)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Box(
                                    modifier = Modifier.size(40.dp).border(0.5.dp, CRTheme.stroke(isDarkMode)),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Icon(Icons.Default.Warning, contentDescription = null, tint = CRTheme.textHigh(isDarkMode), modifier = Modifier.size(18.dp))
                                }
                                Spacer(modifier = Modifier.width(16.dp))
                                Text(
                                    text = "To ensure Deskdrop stays alive in the background, disable battery optimization.",
                                    fontSize = 12.sp,
                                    color = CRTheme.textMedium(isDarkMode),
                                    lineHeight = 18.sp
                                )
                            }
                            Spacer(modifier = Modifier.height(24.dp))
                            Button(
                                onClick = {
                                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                    onBatterySettingsClicked()
                                },
                                modifier = Modifier.fillMaxWidth().height(48.dp),
                                colors = ButtonDefaults.buttonColors(containerColor = Color.Transparent),
                                contentPadding = PaddingValues(0.dp)
                            ) {
                                Box(
                                    modifier = Modifier
                                        .fillMaxSize()
                                        .border(1.dp, CRTheme.textHigh(isDarkMode)),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Text("OPEN BATTERY SETTINGS", fontSize = 11.sp, fontWeight = FontWeight.Medium, color = CRTheme.textHigh(isDarkMode), letterSpacing = 1.sp)
                                }
                            }
                        }
                    }
                }

                item {
                    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth().padding(top = 16.dp)) {
                        Box(
                            modifier = Modifier
                                .size(56.dp)
                                .border(1.dp, CRTheme.textHigh(isDarkMode)),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(Icons.Default.Home, contentDescription = null, tint = CRTheme.textHigh(isDarkMode), modifier = Modifier.size(24.dp))
                        }
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(text = "Deskdrop", fontSize = 18.sp, fontWeight = FontWeight.Medium, color = CRTheme.textHigh(isDarkMode))
                        Text(text = "VERSION 1.0.0", fontSize = 10.sp, fontWeight = FontWeight.Medium, color = CRTheme.textMedium(isDarkMode), letterSpacing = 1.sp)
                        Spacer(modifier = Modifier.height(24.dp))
                        Text(
                            text = "NO CLOUD. NO ACCOUNT. NO TELEMETRY.",
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Medium,
                            color = CRTheme.textHigh(isDarkMode),
                            letterSpacing = 1.sp,
                            textAlign = TextAlign.Center
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun SettingsCategory(
    isDark: Boolean,
    title: String,
    icon: ImageVector,
    content: @Composable ColumnScope.() -> Unit
) {
    var expanded by remember { mutableStateOf(true) }
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { expanded = !expanded }
                .padding(vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier.size(28.dp).border(0.5.dp, CRTheme.stroke(isDark)),
                contentAlignment = Alignment.Center
            ) {
                Icon(imageVector = icon, contentDescription = null, tint = CRTheme.textMedium(isDark), modifier = Modifier.size(14.dp))
            }
            Spacer(modifier = Modifier.width(12.dp))
            Text(
                text = title.uppercase(),
                fontSize = 11.sp,
                fontWeight = FontWeight.Medium,
                color = CRTheme.textMedium(isDark),
                letterSpacing = 1.sp,
                modifier = Modifier.weight(1f)
            )
            Text(
                text = if (expanded) "▲" else "▼",
                fontSize = 10.sp,
                color = CRTheme.textMedium(isDark)
            )
        }
        
        AnimatedVisibility(
            visible = expanded,
            enter = expandVertically(animationSpec = spring(stiffness = Spring.StiffnessLow)) + fadeIn(),
            exit = shrinkVertically(animationSpec = spring(stiffness = Spring.StiffnessLow)) + fadeOut()
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .crCard(isDark, cornerRadius = 0.dp)
            ) {
                Column(modifier = Modifier.fillMaxWidth()) {
                    content()
                }
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
            modifier = Modifier.size(32.dp).border(0.5.dp, CRTheme.stroke(isDark)),
            contentAlignment = Alignment.Center
        ) {
            Icon(imageVector = icon, contentDescription = null, tint = CRTheme.textMedium(isDark), modifier = Modifier.size(16.dp))
        }
        Spacer(modifier = Modifier.width(16.dp))
        
        Column(modifier = Modifier.weight(1f)) {
            Text(text = title, fontSize = 14.sp, fontWeight = FontWeight.Medium, color = CRTheme.textHigh(isDark))
            if (subtitle != null) {
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = subtitle,
                    fontSize = 12.sp,
                    color = CRTheme.textLow(isDark),
                    fontWeight = FontWeight.Normal
                )
            }
        }
        CRSwitch(checked = checked, isDark = isDark)
    }
}
