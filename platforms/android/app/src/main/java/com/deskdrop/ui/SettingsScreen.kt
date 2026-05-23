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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.deskdrop.ui.theme.CRBackground
import com.deskdrop.ui.theme.CRMotion
import com.deskdrop.ui.theme.CRTheme
import com.deskdrop.ui.theme.SectionHeader
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

    // Parallax title scale effect
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
                        .background(CRTheme.surface(isDarkMode))
                        .clickable {
                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                            onBack()
                        },
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = "Back",
                        tint = CRTheme.ink(isDarkMode),
                        modifier = Modifier.size(24.dp)
                    )
                }
                Spacer(modifier = Modifier.width(20.dp))
                Text(
                    text = "Settings",
                    fontSize = 32.sp,
                    fontWeight = FontWeight.Black,
                    color = CRTheme.ink(isDarkMode),
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
                // Profile & Local Network Metadata Hero Card
                item {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .crCard(isDarkMode, cornerRadius = 32.dp, highlighted = true, accentColor = CRTheme.brandViolet)
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
                                    .size(92.dp)
                                    .clip(CircleShape)
                                    .background(CRTheme.brandGradient)
                                    .border(2.dp, Color.White.copy(alpha = 0.8f), CircleShape)
                                    .shadow(24.dp, CircleShape, spotColor = CRTheme.brandElectric),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    text = deviceName.take(1).uppercase(),
                                    fontSize = 42.sp,
                                    fontWeight = FontWeight.Black,
                                    color = Color.White
                                )
                            }
                            Spacer(modifier = Modifier.height(20.dp))
                            Text(text = deviceName, fontSize = 24.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDarkMode))
                            Spacer(modifier = Modifier.height(6.dp))
                            
                            // Dynamic Local Network IP and metadata
                            Row(
                                modifier = Modifier
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(Color.White.copy(alpha = if (isDarkMode) 0.05f else 0.4f))
                                    .border(0.5.dp, CRTheme.stroke(isDarkMode), RoundedCornerShape(12.dp))
                                    .padding(horizontal = 14.dp, vertical = 6.dp),
                                horizontalArrangement = Arrangement.Center,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = "IP: ${getLocalIpAddress()}",
                                    fontSize = 13.sp,
                                    fontFamily = FontFamily.Monospace,
                                    color = CRTheme.brandElectric,
                                    fontWeight = FontWeight.Bold
                                )
                                Spacer(modifier = Modifier.width(12.dp))
                                Box(modifier = Modifier.size(4.dp).clip(CircleShape).background(CRTheme.inkSubtle(isDarkMode)))
                                Spacer(modifier = Modifier.width(12.dp))
                                Text(
                                    text = "CORE: ACTIVE",
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.ExtraBold,
                                    color = CRTheme.accentGreen
                                )
                            }
                            
                            Spacer(modifier = Modifier.height(16.dp))
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(Icons.Default.Edit, contentDescription = null, tint = CRTheme.brandElectric, modifier = Modifier.size(14.dp))
                                Spacer(modifier = Modifier.width(6.dp))
                                Text("Tap to edit name", fontSize = 13.sp, fontWeight = FontWeight.Bold, color = CRTheme.brandElectric)
                            }
                        }
                    }
                }

                // Appearance Collapsible Section
                item {
                    SettingsCategory(
                        isDark = isDarkMode,
                        title = "Appearance",
                        icon = Icons.Default.Info,
                        accentColor = CRTheme.brandViolet
                    ) {
                        SettingsSwitchRow(
                            isDark = isDarkMode,
                            icon = Icons.Default.Info,
                            iconColor = CRTheme.brandViolet,
                            title = "Dark mode",
                            subtitle = "Deep black high-contrast theme",
                            checked = isDarkMode,
                            onCheckedChange = onDarkModeChange
                        )
                    }
                }

                // Clipboard Sync Collapsible Section
                item {
                    SettingsCategory(
                        isDark = isDarkMode,
                        title = "Clipboard Sync",
                        icon = Icons.Default.Refresh,
                        accentColor = CRTheme.brandElectric
                    ) {
                        Column {
                            SettingsSwitchRow(
                                isDark = isDarkMode,
                                icon = Icons.Default.Refresh,
                                iconColor = CRTheme.brandElectric,
                                title = "Enable sync",
                                subtitle = "Master switch for all features",
                                checked = syncEnabled,
                                onCheckedChange = onSyncEnabledChange
                            )
                            
                            if (syncEnabled) {
                                HorizontalDivider(color = CRTheme.divider(isDarkMode), thickness = 0.5.dp, modifier = Modifier.padding(start = 72.dp))
                                SettingsSwitchRow(
                                    isDark = isDarkMode,
                                    icon = Icons.Default.Edit,
                                    iconColor = CRTheme.inkSubtle(isDarkMode),
                                    title = "Sync text",
                                    subtitle = null,
                                    checked = syncText,
                                    onCheckedChange = onSyncTextChange
                                )
                                HorizontalDivider(color = CRTheme.divider(isDarkMode), thickness = 0.5.dp, modifier = Modifier.padding(start = 72.dp))
                                SettingsSwitchRow(
                                    isDark = isDarkMode,
                                    icon = Icons.Default.Face,
                                    iconColor = CRTheme.inkSubtle(isDarkMode),
                                    title = "Sync images",
                                    subtitle = null,
                                    checked = syncImages,
                                    onCheckedChange = onSyncImagesChange
                                )
                                HorizontalDivider(color = CRTheme.divider(isDarkMode), thickness = 0.5.dp, modifier = Modifier.padding(start = 72.dp))
                                SettingsSwitchRow(
                                    isDark = isDarkMode,
                                    icon = Icons.Default.List,
                                    iconColor = CRTheme.inkSubtle(isDarkMode),
                                    title = "Sync files",
                                    subtitle = "Saved directly to Downloads",
                                    checked = syncFiles,
                                    onCheckedChange = onSyncFilesChange
                                )
                            }
                        }
                    }
                }

                // Background Execution Category Section
                item {
                    SettingsCategory(
                        isDark = isDarkMode,
                        title = "Background Execution",
                        icon = Icons.Default.Warning,
                        accentColor = CRTheme.accentAmber
                    ) {
                        Column(modifier = Modifier.padding(24.dp)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Box(
                                    modifier = Modifier.size(40.dp).clip(CircleShape).background(CRTheme.accentAmber.copy(alpha = 0.15f)),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Icon(Icons.Default.Warning, contentDescription = null, tint = CRTheme.accentAmber)
                                }
                                Spacer(modifier = Modifier.width(16.dp))
                                Text(
                                    text = "To ensure Deskdrop stays alive in the background, disable battery optimization.",
                                    fontSize = 14.sp,
                                    color = CRTheme.inkSoft(isDarkMode),
                                    lineHeight = 20.sp
                                )
                            }
                            Spacer(modifier = Modifier.height(24.dp))
                            Button(
                                onClick = {
                                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                    onBatterySettingsClicked()
                                },
                                modifier = Modifier.fillMaxWidth().height(52.dp),
                                colors = ButtonDefaults.buttonColors(containerColor = Color.Transparent),
                                contentPadding = PaddingValues(0.dp),
                                shape = RoundedCornerShape(16.dp)
                            ) {
                                Box(
                                    modifier = Modifier
                                        .fillMaxSize()
                                        .background(CRTheme.surfaceElevated(isDarkMode))
                                        .border(1.dp, CRTheme.divider(isDarkMode), RoundedCornerShape(16.dp)),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Text("Open battery settings", fontSize = 15.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDarkMode))
                                }
                            }
                        }
                    }
                }

                // About section
                item {
                    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth().padding(top = 16.dp)) {
                        Box(
                            modifier = Modifier
                                .size(64.dp)
                                .clip(RoundedCornerShape(16.dp))
                                .background(CRTheme.brandGradient)
                                .shadow(8.dp, RoundedCornerShape(16.dp), spotColor = CRTheme.brandElectric),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(Icons.Default.Home, contentDescription = null, tint = Color.White, modifier = Modifier.size(32.dp))
                        }
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(text = "Deskdrop", fontSize = 20.sp, fontWeight = FontWeight.Black, color = CRTheme.ink(isDarkMode))
                        Text(text = "Version 1.0.0", fontSize = 13.sp, fontWeight = FontWeight.Medium, color = CRTheme.inkSubtle(isDarkMode))
                        Spacer(modifier = Modifier.height(24.dp))
                        Text(
                            text = "No cloud. No account. No telemetry.",
                            fontSize = 14.sp,
                            fontWeight = FontWeight.Bold,
                            color = CRTheme.brandElectric,
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
    accentColor: Color,
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
                modifier = Modifier.size(32.dp).clip(CircleShape).background(accentColor.copy(alpha = 0.12f)),
                contentAlignment = Alignment.Center
            ) {
                Icon(imageVector = icon, contentDescription = null, tint = accentColor, modifier = Modifier.size(16.dp))
            }
            Spacer(modifier = Modifier.width(12.dp))
            Text(
                text = title.uppercase(),
                fontSize = 13.sp,
                fontWeight = FontWeight.ExtraBold,
                color = CRTheme.inkSubtle(isDark),
                letterSpacing = 1.sp,
                modifier = Modifier.weight(1f)
            )
            Text(
                text = if (expanded) "▲" else "▼",
                fontSize = 11.sp,
                color = CRTheme.inkSubtle(isDark)
            )
        }
        
        AnimatedVisibility(
            visible = expanded,
            enter = expandVertically(animationSpec = spring(stiffness = Spring.StiffnessMedium)) + fadeIn(),
            exit = shrinkVertically(animationSpec = spring(stiffness = Spring.StiffnessMedium)) + fadeOut()
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .crCard(isDark, cornerRadius = 24.dp)
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
    iconColor: Color,
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
            modifier = Modifier.size(36.dp).clip(CircleShape).background(iconColor.copy(alpha = 0.15f)),
            contentAlignment = Alignment.Center
        ) {
            Icon(imageVector = icon, contentDescription = null, tint = iconColor, modifier = Modifier.size(20.dp))
        }
        Spacer(modifier = Modifier.width(16.dp))
        
        Column(modifier = Modifier.weight(1f)) {
            Text(text = title, fontSize = 16.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDark))
            if (subtitle != null) {
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = subtitle,
                    fontSize = 13.sp,
                    color = CRTheme.inkSoft(isDark),
                    fontWeight = FontWeight.Medium
                )
            }
        }
        CRSwitch(checked = checked, isDark = isDark)
    }
}
