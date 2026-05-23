package com.cliprelay.ui

import androidx.compose.animation.core.*
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cliprelay.ui.theme.CRTheme
import com.cliprelay.ui.theme.crCard
import com.cliprelay.ui.theme.CRBackground

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
    val isDark = isDarkMode

    CRBackground(isDark = isDark) {
        Column(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
            
            // Premium Top Bar
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(Color.Transparent)
                    .padding(horizontal = 24.dp, vertical = 16.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                val haptic = LocalHapticFeedback.current
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(androidx.compose.foundation.shape.CircleShape)
                        .background(if (isDark) Color.White.copy(alpha = 0.05f) else Color.Black.copy(alpha = 0.03f))
                        .clickable {
                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                            onBack()
                        },
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = "Back",
                        tint = CRTheme.ink(isDark),
                        modifier = Modifier.size(22.dp)
                    )
                }
                Spacer(modifier = Modifier.width(16.dp))
                Text(
                    text = "Settings",
                    fontSize = 28.sp,
                    fontWeight = FontWeight.Bold,
                    color = CRTheme.ink(isDark),
                    letterSpacing = (-0.5).sp
                )
            }

            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(top = 16.dp, start = 24.dp, end = 24.dp, bottom = 48.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp)
            ) {
                item {
                    SettingsSectionHeader(isDark, "APPEARANCE")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                        SettingsSwitchRow(isDark, "Dark mode", "Use dark theme", isDarkMode, onDarkModeChange)
                    }
                }

                item {
                    SettingsSectionHeader(isDark, "THIS DEVICE")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(24.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Column {
                                Text(text = deviceName, fontSize = 18.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDark))
                                Spacer(modifier = Modifier.height(4.dp))
                                Text(
                                    text = "ID: ${deviceId.take(8)}",
                                    fontSize = 14.sp,
                                    color = CRTheme.inkSoft(isDark),
                                    fontWeight = FontWeight.Medium
                                )
                            }
                            TextButton(onClick = onRenameClicked) {
                                Text("Edit", color = CRTheme.brandElectric, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                            }
                        }
                    }
                }

                item {
                    SettingsSectionHeader(isDark, "CLIPBOARD SYNC")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                        Column {
                            SettingsSwitchRow(isDark, "Enable sync", null, syncEnabled, onSyncEnabledChange)
                            HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp, modifier = Modifier.padding(start = 24.dp))
                            SettingsSwitchRow(isDark, "Sync text", null, syncText, onSyncTextChange)
                            HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp, modifier = Modifier.padding(start = 24.dp))
                            SettingsSwitchRow(isDark, "Sync images", null, syncImages, onSyncImagesChange)
                            HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp, modifier = Modifier.padding(start = 24.dp))
                            SettingsSwitchRow(isDark, "Sync files", "Saved to Downloads", syncFiles, onSyncFilesChange)
                        }
                    }
                }

                item {
                    SettingsSectionHeader(isDark, "BATTERY")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                        Column(modifier = Modifier.padding(24.dp)) {
                            Text(
                                text = "To ensure ClipRelay stays alive in the background, disable battery optimization.",
                                fontSize = 15.sp,
                                color = CRTheme.inkSoft(isDark),
                                lineHeight = 22.sp
                            )
                            Spacer(modifier = Modifier.height(24.dp))
                            val haptic = LocalHapticFeedback.current
                            Button(
                                onClick = {
                                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                    onBatterySettingsClicked()
                                },
                                modifier = Modifier.fillMaxWidth().height(54.dp),
                                colors = ButtonDefaults.buttonColors(containerColor = CRTheme.brandElectric),
                                shape = androidx.compose.foundation.shape.RoundedCornerShape(12.dp)
                            ) {
                                Text("Open battery settings", fontSize = 15.sp, fontWeight = FontWeight.Bold)
                            }
                        }
                    }
                }

                item {
                    SettingsSectionHeader(isDark, "ABOUT")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                        Column(modifier = Modifier.padding(24.dp)) {
                            Text(text = "ClipRelay", fontSize = 18.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDark))
                            Spacer(modifier = Modifier.height(8.dp))
                            Text(
                                text = "Private clipboard and file relay for your local network. No cloud. No account. No telemetry.",
                                fontSize = 15.sp,
                                color = CRTheme.inkSoft(isDark),
                                lineHeight = 22.sp
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun SettingsSectionHeader(isDark: Boolean, title: String) {
    Text(
        text = title,
        fontSize = 12.sp,
        fontWeight = FontWeight.ExtraBold,
        color = CRTheme.brandElectric,
        letterSpacing = 1.5.sp,
        modifier = Modifier.padding(bottom = 12.dp, start = 8.dp)
    )
}

@Composable
fun SettingsSwitchRow(
    isDark: Boolean,
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
        Column(modifier = Modifier.weight(1f)) {
            Text(text = title, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = CRTheme.ink(isDark))
            if (subtitle != null) {
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = subtitle,
                    fontSize = 14.sp,
                    color = CRTheme.inkSoft(isDark)
                )
            }
        }
        CRSwitch(checked = checked, isDark = isDark)
    }
}

@Composable
fun CRSwitch(checked: Boolean, isDark: Boolean) {
    val thumbOffset by animateFloatAsState(
        targetValue = if (checked) 24f else 4f,
        animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessLow),
        label = "switchOffset"
    )
    val thumbColor by animateColorAsState(
        targetValue = if (checked) Color.White else CRTheme.inkSubtle(isDark),
        animationSpec = tween(200),
        label = "switchThumb"
    )
    val trackColor by animateColorAsState(
        targetValue = if (checked) CRTheme.brandElectric else CRTheme.stroke(isDark),
        animationSpec = tween(200),
        label = "switchTrack"
    )

    Box(
        modifier = Modifier
            .width(52.dp)
            .height(28.dp)
            .clip(androidx.compose.foundation.shape.CircleShape)
            .background(trackColor)
    ) {
        Box(
            modifier = Modifier
                .offset(x = thumbOffset.dp, y = 4.dp)
                .size(20.dp)
                .shadow(4.dp, androidx.compose.foundation.shape.CircleShape)
                .clip(androidx.compose.foundation.shape.CircleShape)
                .background(thumbColor)
        )
    }
}
