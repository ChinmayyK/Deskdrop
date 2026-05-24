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
    val timerColor = CRTheme.textHigh(isDark)

    CRBackground(isDark = isDark) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(CRTheme.bg(isDark).copy(alpha = 0.8f))
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
                initialValue = 0.98f, targetValue = 1.02f,
                animationSpec = infiniteRepeatable(animation = tween(2000, easing = FastOutSlowInEasing), repeatMode = RepeatMode.Reverse),
                label = "pulse"
            )

            // Timer with sharp aesthetic
            Box(
                modifier = Modifier
                    .size(130.dp)
                    .scale(pulseScale),
                contentAlignment = Alignment.Center
            ) {
                CircularProgressIndicator(
                    progress = { progress },
                    modifier = Modifier.fillMaxSize(),
                    color = timerColor,
                    trackColor = CRTheme.stroke(isDark),
                    strokeWidth = 1.dp
                )
                Box(
                    modifier = Modifier
                        .size(98.dp)
                        .background(CRTheme.surface(isDark))
                        .border(0.5.dp, CRTheme.stroke(isDark)),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = deviceName.take(1).uppercase(),
                        color = CRTheme.textHigh(isDark),
                        fontSize = 32.sp,
                        fontWeight = FontWeight.Light
                    )
                }
            }

            Spacer(modifier = Modifier.height(32.dp))
            Text(
                text = "SECURE PAIRING REQUEST",
                fontSize = 10.sp,
                fontWeight = FontWeight.Medium,
                color = CRTheme.textMedium(isDark),
                letterSpacing = 1.5.sp
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = deviceName,
                fontSize = 24.sp,
                fontWeight = FontWeight.Light,
                color = CRTheme.textHigh(isDark),
                letterSpacing = (-1).sp,
                textAlign = TextAlign.Center
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "WANTS TO JOIN YOUR SECURE NETWORK",
                fontSize = 10.sp,
                color = CRTheme.textLow(isDark),
                fontWeight = FontWeight.Medium,
                letterSpacing = 1.sp
            )

            Spacer(modifier = Modifier.height(40.dp))

            // PIN Digit Blocks
            Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 0.dp)) {
                Column(modifier = Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Box(modifier = Modifier.size(24.dp).border(0.5.dp, CRTheme.stroke(isDark)), contentAlignment = Alignment.Center) {
                                Box(modifier = Modifier.size(6.dp).background(CRTheme.textMedium(isDark)))
                            }
                            Spacer(modifier = Modifier.width(12.dp))
                            Text(
                                text = "VERBAL CONFIRM",
                                fontSize = 10.sp,
                                fontWeight = FontWeight.Medium,
                                color = CRTheme.textMedium(isDark),
                                letterSpacing = 1.sp
                            )
                        }
                    }
                    Spacer(modifier = Modifier.height(24.dp))
                    
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        pin.forEach { char ->
                            Box(
                                modifier = Modifier
                                    .size(width = 44.dp, height = 56.dp)
                                    .background(CRTheme.surface(isDark))
                                    .border(0.5.dp, CRTheme.stroke(isDark)),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    text = char.toString(),
                                    fontSize = 24.sp,
                                    fontFamily = FontFamily.Monospace,
                                    fontWeight = FontWeight.Normal,
                                    color = CRTheme.textHigh(isDark)
                                )
                            }
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(20.dp))

            // Fingerprint Block
            Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 0.dp)) {
                Column(modifier = Modifier.padding(24.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(modifier = Modifier.size(6.dp).background(CRTheme.textMedium(isDark)))
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = "DEVICE FINGERPRINT",
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Medium,
                            color = CRTheme.textMedium(isDark),
                            letterSpacing = 1.sp
                        )
                    }
                    Spacer(modifier = Modifier.height(16.dp))
                    
                    val pairs = if (fingerprint.isBlank()) emptyList() else fingerprint.replace(":", "").chunked(2)
                    val formatted = if (pairs.isEmpty()) "Not available" else pairs.chunked(8).joinToString("\n") { it.joinToString(":") }
                    
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(CRTheme.surface(isDark))
                            .border(0.5.dp, CRTheme.stroke(isDark))
                            .padding(16.dp)
                    ) {
                        Text(
                            text = formatted,
                            fontSize = 11.sp,
                            fontFamily = FontFamily.Monospace,
                            fontWeight = FontWeight.Normal,
                            color = CRTheme.textHigh(isDark),
                            lineHeight = 24.sp,
                            letterSpacing = 1.sp
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(32.dp))

            Text(
                text = "${(remainingMs / 1000).coerceAtLeast(0)}S REMAINING",
                fontSize = 10.sp,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Medium,
                color = CRTheme.textMedium(isDark),
                letterSpacing = 1.5.sp
            )

            Spacer(modifier = Modifier.weight(1f))

            val haptic = LocalHapticFeedback.current
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onDeny()
                    },
                    modifier = Modifier.weight(1f).height(56.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.surface(isDark)),
                    shape = RoundedCornerShape(0.dp)
                ) {
                    Text("DENY", color = CRTheme.textMedium(isDark), fontSize = 12.sp, fontWeight = FontWeight.Medium, letterSpacing = 1.sp)
                }
                Spacer(modifier = Modifier.width(16.dp))
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onApprove()
                    },
                    modifier = Modifier.weight(1f).height(56.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Color.Transparent),
                    contentPadding = PaddingValues(0.dp),
                    shape = RoundedCornerShape(0.dp)
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(CRTheme.textHigh(isDark))
                            .border(0.5.dp, CRTheme.stroke(isDark)),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("TRUST DEVICE", color = CRTheme.bg(isDark), fontSize = 12.sp, fontWeight = FontWeight.Medium, letterSpacing = 1.sp)
                    }
                }
            }
        }
    }
}
