package com.deskdrop.ui

import androidx.compose.animation.core.*
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.deskdrop.ui.theme.CRTheme
import com.deskdrop.ui.theme.crPressScale
import com.deskdrop.ui.theme.CRBackground
import com.deskdrop.ui.theme.crGlassCard
import com.deskdrop.ui.theme.CRTypography
import kotlinx.coroutines.delay

@Composable
fun PairingScreen(
    isDark: Boolean,
    deviceName: String,
    pin: String,
    fingerprint: String,
    onApprove: () -> Unit,
    onDeny: () -> Unit
) {
    var remainingMs by remember { mutableLongStateOf(30_000L) }
    val haptic = LocalHapticFeedback.current

    LaunchedEffect(Unit) {
        while (remainingMs > 0) {
            delay(100)
            remainingMs -= 100
        }
        if (remainingMs <= 0) {
            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            onDeny()
        }
    }

    val progress = (remainingMs / 30_000f).coerceIn(0f, 1f)

    CRBackground(isDark = isDark, hasConnectedDevices = true) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .systemBarsPadding(),
            contentAlignment = Alignment.Center
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(24.dp)
                    .crGlassCard(isDark = isDark, cornerRadius = 32.dp)
                    .padding(32.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                // Header
                Text(
                    text = "PAIRING REQUEST",
                    style = CRTypography.label,
                    color = CRTheme.brandElectric,
                    letterSpacing = 2.sp,
                    fontWeight = FontWeight.Bold
                )
                
                Spacer(modifier = Modifier.height(24.dp))
                
                // Device Name
                Text(
                    text = deviceName,
                    style = CRTypography.h1,
                    color = CRTheme.textHigh(isDark),
                    maxLines = 2,
                    textAlign = TextAlign.Center
                )
                
                Spacer(modifier = Modifier.height(8.dp))
                
                Text(
                    text = "wants to connect to your ecosystem.",
                    style = CRTypography.bodyMedium,
                    color = CRTheme.textMedium(isDark),
                    textAlign = TextAlign.Center
                )
                
                Spacer(modifier = Modifier.height(32.dp))
                
                // PIN Area
                Text(
                    text = "MATCH THIS PIN ON THE OTHER DEVICE",
                    style = CRTypography.caption,
                    color = CRTheme.textMedium(isDark),
                    letterSpacing = 1.sp,
                    fontWeight = FontWeight.Bold
                )
                
                Spacer(modifier = Modifier.height(16.dp))
                
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(CRTheme.textHigh(isDark).copy(alpha = 0.05f), RoundedCornerShape(16.dp))
                        .padding(vertical = 16.dp),
                    horizontalArrangement = Arrangement.SpaceEvenly
                ) {
                    pin.forEach { char ->
                        Text(
                            text = char.toString(),
                            fontSize = 40.sp,
                            fontFamily = FontFamily.Monospace,
                            fontWeight = FontWeight.Medium,
                            color = CRTheme.textHigh(isDark)
                        )
                    }
                }
                
                Spacer(modifier = Modifier.height(24.dp))
                
                // Fingerprint
                val formattedFingerprint = if (fingerprint.isBlank()) "N/A" else fingerprint.replace(":", "").chunked(4).joinToString(" ")
                Text(
                    text = "Fingerprint: $formattedFingerprint",
                    style = CRTypography.caption,
                    fontFamily = FontFamily.Monospace,
                    color = CRTheme.textMedium(isDark),
                    textAlign = TextAlign.Center
                )
                
                Spacer(modifier = Modifier.height(48.dp))
                
                // Progress Bar
                LinearProgressIndicator(
                    progress = { progress },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(4.dp)
                        .clip(RoundedCornerShape(2.dp)),
                    color = CRTheme.brandElectric,
                    trackColor = CRTheme.brandElectric.copy(alpha = 0.2f)
                )
                
                Spacer(modifier = Modifier.height(24.dp))
                
                // Actions
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    // Deny
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(56.dp)
                            .crPressScale(0.95f)
                            .background(CRTheme.accentRed.copy(alpha = 0.15f), RoundedCornerShape(16.dp))
                            .border(1.dp, CRTheme.accentRed.copy(alpha = 0.3f), RoundedCornerShape(16.dp))
                            .clickable {
                                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                onDeny()
                            },
                        contentAlignment = Alignment.Center
                    ) {
                        Text("Decline", style = CRTypography.label, color = CRTheme.accentRed, fontWeight = FontWeight.Bold)
                    }
                    
                    // Approve
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(56.dp)
                            .crPressScale(0.95f)
                            .background(CRTheme.brandElectric, RoundedCornerShape(16.dp))
                            .clickable {
                                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                onApprove()
                            },
                        contentAlignment = Alignment.Center
                    ) {
                        Text("Accept", style = CRTypography.label, color = Color.White, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}
