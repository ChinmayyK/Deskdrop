package com.deskdrop.ui

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
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
import com.deskdrop.ui.theme.crPressScale
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

    CRBackground(isDark = isDark) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .systemBarsPadding()
                .padding(horizontal = 24.dp, vertical = 32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            val infiniteTransition = rememberInfiniteTransition(label = "radar")
            val radarScale by infiniteTransition.animateFloat(
                initialValue = 0.8f, targetValue = 1.4f,
                animationSpec = infiniteRepeatable(animation = tween(2000, easing = LinearOutSlowInEasing), repeatMode = RepeatMode.Restart),
                label = "radar_scale"
            )
            val radarAlpha by infiniteTransition.animateFloat(
                initialValue = 0.8f, targetValue = 0f,
                animationSpec = infiniteRepeatable(animation = tween(2000, easing = LinearOutSlowInEasing), repeatMode = RepeatMode.Restart),
                label = "radar_alpha"
            )
            val pulseScale by infiniteTransition.animateFloat(
                initialValue = 0.98f, targetValue = 1.02f,
                animationSpec = infiniteRepeatable(animation = tween(2000, easing = FastOutSlowInEasing), repeatMode = RepeatMode.Reverse),
                label = "pulse"
            )

            // Animated Hero Avatar
            Box(
                modifier = Modifier
                    .size(140.dp)
                    .scale(pulseScale),
                contentAlignment = Alignment.Center
            ) {
                // Radar ripples
                Box(
                    modifier = Modifier
                        .size(100.dp)
                        .scale(radarScale)
                        .background(CRTheme.indigoSoft.copy(alpha = radarAlpha), CircleShape)
                )
                
                // Avatar background
                Box(
                    modifier = Modifier
                        .size(100.dp)
                        .background(CRTheme.bg(isDark).copy(alpha = 0.8f), CircleShape)
                        .border(1.dp, CRTheme.indigoSoft.copy(alpha = 0.5f), CircleShape),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = deviceName.take(1).uppercase(),
                        color = CRTheme.textHigh(isDark),
                        fontSize = 36.sp,
                        fontWeight = FontWeight.Medium
                    )
                }
                
                // Progress ring
                CircularProgressIndicator(
                    progress = { progress },
                    modifier = Modifier.fillMaxSize(),
                    color = CRTheme.indigoSoft,
                    trackColor = CRTheme.indigoSoft.copy(alpha = 0.1f),
                    strokeWidth = 2.dp,
                    strokeCap = androidx.compose.ui.graphics.StrokeCap.Round
                )
            }

            Spacer(modifier = Modifier.height(32.dp))
            
            Text(
                text = "Secure Pairing Request",
                style = CRTypography.caption,
                color = CRTheme.indigoSoft,
                letterSpacing = 1.5.sp,
                modifier = Modifier.background(CRTheme.indigoSoft.copy(alpha = 0.1f), RoundedCornerShape(100.dp)).padding(horizontal = 12.dp, vertical = 6.dp)
            )
            
            Spacer(modifier = Modifier.height(16.dp))
            Text(
                text = deviceName,
                style = CRTypography.h1,
                color = CRTheme.textHigh(isDark),
                textAlign = TextAlign.Center
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "wants to join your secure network",
                style = CRTypography.bodyMedium,
                color = CRTheme.textMedium(isDark),
                textAlign = TextAlign.Center
            )

            Spacer(modifier = Modifier.height(48.dp))

            // PIN Digit Blocks
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .crGlassCard(isDark = isDark, cornerRadius = 24.dp)
                    .padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text(
                    text = "VERBAL CONFIRMATION PIN",
                    style = CRTypography.caption,
                    color = CRTheme.textMedium(isDark),
                    letterSpacing = 1.sp
                )
                Spacer(modifier = Modifier.height(20.dp))
                
                Row(
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    pin.forEach { char ->
                        Box(
                            modifier = Modifier
                                .size(width = 52.dp, height = 64.dp)
                                .background(CRTheme.surface(isDark).copy(alpha = 0.5f), RoundedCornerShape(12.dp))
                                .border(1.dp, CRTheme.stroke(isDark), RoundedCornerShape(12.dp)),
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = char.toString(),
                                fontSize = 28.sp,
                                fontFamily = FontFamily.Monospace,
                                fontWeight = FontWeight.SemiBold,
                                color = CRTheme.textHigh(isDark)
                            )
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(24.dp))

            // Fingerprint Block (Collapsible or just sleek)
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .crGlassCard(isDark = isDark, cornerRadius = 16.dp)
                    .padding(16.dp)
            ) {
                val pairs = if (fingerprint.isBlank()) emptyList() else fingerprint.replace(":", "").chunked(2)
                val formatted = if (pairs.isEmpty()) "Not available" else pairs.chunked(8).joinToString("\n") { it.joinToString(":") }
                
                Column {
                    Text(
                        text = "DEVICE FINGERPRINT",
                        style = CRTypography.caption,
                        color = CRTheme.textLow(isDark),
                        letterSpacing = 1.sp
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = formatted,
                        fontSize = 11.sp,
                        fontFamily = FontFamily.Monospace,
                        color = CRTheme.textMedium(isDark),
                        lineHeight = 20.sp
                    )
                }
            }

            Spacer(modifier = Modifier.weight(1f))
            
            Text(
                text = "${(remainingMs / 1000).coerceAtLeast(0)}s remaining",
                style = CRTypography.bodyMedium,
                color = CRTheme.textMedium(isDark)
            )
            
            Spacer(modifier = Modifier.height(16.dp))

            // Action Buttons
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                        onDeny()
                    },
                    modifier = Modifier.weight(1f).height(56.dp).crPressScale(0.95f),
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.surface(isDark)),
                    shape = RoundedCornerShape(16.dp)
                ) {
                    Text("DENY", color = CRTheme.textHigh(isDark), style = CRTypography.label)
                }
                
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                        onApprove()
                    },
                    modifier = Modifier.weight(1f).height(56.dp).crPressScale(0.95f),
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.indigoSoft),
                    shape = RoundedCornerShape(16.dp)
                ) {
                    Text("TRUST", color = Color.White, style = CRTypography.label)
                }
            }
        }
    }
}
