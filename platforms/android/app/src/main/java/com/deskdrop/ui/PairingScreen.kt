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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
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
import com.deskdrop.ui.theme.crCard
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

    LaunchedEffect(Unit) {
        while (remainingMs > 0) {
            delay(100)
            remainingMs -= 100
        }
        if (remainingMs <= 0) {
            onDeny()
        }
    }

    val progress = (remainingMs / 30_000f).coerceIn(0f, 1f)
    val timerColor = if (progress > 0.3f) CRTheme.brandElectric else CRTheme.accentRed

    CRBackground(isDark = isDark) {
        // Blur / frosted backdrop scrim overlay
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Black.copy(alpha = if (isDark) 0.5f else 0.25f))
        )

        Column(
            modifier = Modifier
                .fillMaxSize()
                .systemBarsPadding()
                .padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            val infiniteTransition = rememberInfiniteTransition(label = "pulse")
            val pulseScale by infiniteTransition.animateFloat(
                initialValue = 0.98f, targetValue = 1.05f,
                animationSpec = infiniteRepeatable(animation = tween(1200, easing = FastOutSlowInEasing), repeatMode = RepeatMode.Reverse),
                label = "pulse"
            )

            // Glowing Circular Timer with Frosted Avatar
            Box(
                modifier = Modifier
                    .size(130.dp)
                    .scale(pulseScale),
                contentAlignment = Alignment.Center
            ) {
                CircularProgressIndicator(
                    progress = { progress },
                    modifier = Modifier
                        .fillMaxSize()
                        .shadow(
                            elevation = if (remainingMs < 10000) 24.dp else 12.dp,
                            shape = CircleShape,
                            ambientColor = timerColor,
                            spotColor = timerColor
                        ),
                    color = timerColor,
                    trackColor = CRTheme.stroke(isDark),
                    strokeWidth = 6.dp
                )
                Box(
                    modifier = Modifier
                        .size(98.dp)
                        .clip(CircleShape)
                        .background(CRTheme.brandGradient)
                        .border(1.5.dp, Color.White.copy(alpha = 0.8f), CircleShape),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = deviceName.take(1).uppercase(),
                        color = Color.White,
                        fontSize = 42.sp,
                        fontWeight = FontWeight.Black
                    )
                }
            }

            Spacer(modifier = Modifier.height(32.dp))
            Text(
                text = "SECURE PAIRING REQUEST",
                fontSize = 12.sp,
                fontWeight = FontWeight.ExtraBold,
                color = CRTheme.inkSubtle(isDark),
                letterSpacing = 1.5.sp
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = deviceName,
                fontSize = 32.sp,
                fontWeight = FontWeight.Black,
                color = CRTheme.ink(isDark),
                letterSpacing = (-1).sp,
                textAlign = TextAlign.Center
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "wants to join your secure network",
                fontSize = 16.sp,
                color = CRTheme.inkSoft(isDark),
                fontWeight = FontWeight.Medium
            )

            Spacer(modifier = Modifier.height(40.dp))

            // PIN Digit Blocks
            Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                Column(modifier = Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(CRTheme.brandElectric))
                            Spacer(modifier = Modifier.width(8.dp))
                            Text(
                                text = "VERBAL CONFIRM",
                                fontSize = 11.sp,
                                fontWeight = FontWeight.ExtraBold,
                                color = CRTheme.inkSubtle(isDark),
                                letterSpacing = 1.sp
                            )
                        }
                        Text(text = "Verify on other screen", fontSize = 12.sp, fontWeight = FontWeight.Medium, color = CRTheme.inkSoft(isDark))
                    }
                    Spacer(modifier = Modifier.height(24.dp))
                    
                    // Display PIN digits as highly premium separate floating blocks
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        pin.forEach { char ->
                            Box(
                                modifier = Modifier
                                    .size(width = 44.dp, height = 56.dp)
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(CRTheme.surfaceElevated(isDark))
                                    .border(1.dp, CRTheme.stroke(isDark), RoundedCornerShape(12.dp))
                                    .shadow(4.dp, RoundedCornerShape(12.dp)),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    text = char.toString(),
                                    fontSize = 28.sp,
                                    fontFamily = FontFamily.Monospace,
                                    fontWeight = FontWeight.Black,
                                    color = CRTheme.brandElectric
                                )
                            }
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(20.dp))

            // Fingerprint Block
            Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                Column(modifier = Modifier.padding(24.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(CRTheme.inkSubtle(isDark)))
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = "DEVICE FINGERPRINT",
                            fontSize = 11.sp,
                            fontWeight = FontWeight.ExtraBold,
                            color = CRTheme.inkSubtle(isDark),
                            letterSpacing = 1.sp
                        )
                    }
                    Spacer(modifier = Modifier.height(16.dp))
                    
                    val pairs = if (fingerprint.isBlank()) emptyList() else fingerprint.replace(":", "").chunked(2)
                    val formatted = if (pairs.isEmpty()) "Not available" else pairs.chunked(8).joinToString("\n") { it.joinToString(":") }
                    
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(CRTheme.surfaceElevated(isDark))
                            .border(1.dp, CRTheme.divider(isDark), RoundedCornerShape(12.dp))
                            .padding(16.dp)
                    ) {
                        Text(
                            text = formatted,
                            fontSize = 13.sp,
                            fontFamily = FontFamily.Monospace,
                            fontWeight = FontWeight.SemiBold,
                            color = CRTheme.ink(isDark),
                            lineHeight = 24.sp,
                            letterSpacing = 1.sp
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(32.dp))

            // Timer countdown info
            Text(
                text = "${(remainingMs / 1000).coerceAtLeast(0)}s remaining",
                fontSize = 15.sp,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
                color = if (remainingMs < 10000) CRTheme.accentRed else CRTheme.inkSoft(isDark)
            )

            Spacer(modifier = Modifier.weight(1f))

            // Dual tactile action buttons
            val haptic = LocalHapticFeedback.current
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onDeny()
                    },
                    modifier = Modifier.weight(1f).height(60.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.surfaceElevated(isDark)),
                    shape = RoundedCornerShape(16.dp)
                ) {
                    Text("Deny", color = CRTheme.ink(isDark), fontSize = 17.sp, fontWeight = FontWeight.Bold)
                }
                Spacer(modifier = Modifier.width(16.dp))
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onApprove()
                    },
                    modifier = Modifier.weight(1f).height(60.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Color.Transparent),
                    contentPadding = PaddingValues(0.dp),
                    shape = RoundedCornerShape(16.dp)
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(CRTheme.brandGradient)
                            .clip(RoundedCornerShape(16.dp)),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("Trust Device", color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}
