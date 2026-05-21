package com.cliprelay.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cliprelay.ui.theme.CRTheme
import com.cliprelay.ui.theme.crCard

@Composable
fun SettingsScreen(
    deviceName: String,
    deviceId: String,
    syncEnabled: Boolean,
    syncText: Boolean,
    syncImages: Boolean,
    syncFiles: Boolean,
    onSyncEnabledChange: (Boolean) -> Unit,
    onSyncTextChange: (Boolean) -> Unit,
    onSyncImagesChange: (Boolean) -> Unit,
    onSyncFilesChange: (Boolean) -> Unit,
    onRenameClicked: () -> Unit,
    onBatterySettingsClicked: () -> Unit,
    onBack: () -> Unit
) {
    val isDark = false // Force light mode as default

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(CRTheme.canvasGradient(isDark))
    ) {
        Column(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
            
            // Custom Top Bar
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(CRTheme.canvasTop(isDark).copy(alpha = 0.8f))
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                IconButton(onClick = onBack) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = "Back",
                        tint = CRTheme.ink(isDark)
                    )
                }
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = "Settings",
                    fontSize = 18.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = CRTheme.ink(isDark)
                )
            }
            
            HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)

            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(top = 24.dp, start = 20.dp, end = 20.dp, bottom = 48.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp)
            ) {
                item {
                    SettingsSectionHeader(isDark, "THIS DEVICE")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 16.dp)) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(20.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Column {
                                Text(text = deviceName, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = CRTheme.ink(isDark))
                                Spacer(modifier = Modifier.height(2.dp))
                                Text(
                                    text = "ID: ${deviceId.take(8)}",
                                    fontSize = 13.sp,
                                    color = CRTheme.inkSoft(isDark)
                                )
                            }
                            TextButton(onClick = onRenameClicked) {
                                Text("Edit", color = CRTheme.brandElectric, fontSize = 14.sp, fontWeight = FontWeight.Medium)
                            }
                        }
                    }
                }

                item {
                    SettingsSectionHeader(isDark, "CLIPBOARD SYNC")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 16.dp)) {
                        Column {
                            SettingsSwitchRow(isDark, "Enable sync", null, syncEnabled, onSyncEnabledChange)
                            HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp, modifier = Modifier.padding(start = 20.dp))
                            SettingsSwitchRow(isDark, "Sync text", null, syncText, onSyncTextChange)
                            HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp, modifier = Modifier.padding(start = 20.dp))
                            SettingsSwitchRow(isDark, "Sync images", null, syncImages, onSyncImagesChange)
                            HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp, modifier = Modifier.padding(start = 20.dp))
                            SettingsSwitchRow(isDark, "Sync files", "Saved to Downloads", syncFiles, onSyncFilesChange)
                        }
                    }
                }

                item {
                    SettingsSectionHeader(isDark, "BATTERY")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 16.dp)) {
                        Column(modifier = Modifier.padding(20.dp)) {
                            Text(
                                text = "To ensure Deskdrop stays alive in the background, disable battery optimization.",
                                fontSize = 14.sp,
                                color = CRTheme.inkSoft(isDark),
                                lineHeight = 20.sp
                            )
                            Spacer(modifier = Modifier.height(16.dp))
                            Button(
                                onClick = onBatterySettingsClicked,
                                modifier = Modifier.fillMaxWidth().height(44.dp),
                                colors = ButtonDefaults.buttonColors(containerColor = CRTheme.brandElectric),
                                shape = androidx.compose.foundation.shape.RoundedCornerShape(8.dp)
                            ) {
                                Text("Open battery settings", fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                            }
                        }
                    }
                }

                item {
                    SettingsSectionHeader(isDark, "ABOUT")
                    Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 16.dp)) {
                        Column(modifier = Modifier.padding(20.dp)) {
                            Text(text = "Deskdrop", fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = CRTheme.ink(isDark))
                            Spacer(modifier = Modifier.height(6.dp))
                            Text(
                                text = "Private clipboard and file relay for your local network. No cloud. No account. No telemetry.",
                                fontSize = 14.sp,
                                color = CRTheme.inkSoft(isDark),
                                lineHeight = 20.sp
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
        fontSize = 11.sp,
        fontWeight = FontWeight.Bold,
        color = CRTheme.brandElectric,
        letterSpacing = 1.sp,
        modifier = Modifier.padding(bottom = 8.dp, start = 4.dp)
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
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onCheckedChange(!checked) }
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(text = title, fontSize = 15.sp, fontWeight = FontWeight.Medium, color = CRTheme.ink(isDark))
            if (subtitle != null) {
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = subtitle,
                    fontSize = 13.sp,
                    color = CRTheme.inkSoft(isDark)
                )
            }
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = Color.White,
                checkedTrackColor = CRTheme.brandElectric
            )
        )
    }
}
