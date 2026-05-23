package com.cliprelay.ui

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.isSystemInDarkTheme
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cliprelay.ui.theme.CRTheme
import com.cliprelay.ui.theme.crCard
import com.cliprelay.ui.theme.CRBackground
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
    val timerColor = if (progress > 0.3f) CRTheme.brandPink else CRTheme.accentRed

    CRBackground(isDark = isDark) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .systemBarsPadding()
                .padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            val infiniteTransition = rememberInfiniteTransition()
            val pulseScale by infiniteTransition.animateFloat(
                initialValue = 0.98f, targetValue = 1.05f,
                animationSpec = infiniteRepeatable(animation = tween(1200, easing = FastOutSlowInEasing), repeatMode = RepeatMode.Reverse),
                label = "pulse"
            )

            // Glowing Circular Timer with Avatar
            Box(
                modifier = Modifier
                    .size(120.dp)
                    .scale(pulseScale),
                contentAlignment = Alignment.Center
            ) {
                CircularProgressIndicator(
                    progress = { progress },
                    modifier = Modifier.fillMaxSize().shadow(if (remainingMs < 10000) 32.dp else 16.dp, CircleShape, ambientColor = timerColor, spotColor = timerColor),
                    color = timerColor,
                    trackColor = CRTheme.stroke(isDark),
                    strokeWidth = 6.dp
                )
                Box(
                    modifier = Modifier
                        .size(90.dp)
                        .clip(CircleShape)
                        .background(Brush.linearGradient(listOf(CRTheme.brandElectric.copy(alpha = 0.2f), CRTheme.brandViolet.copy(alpha = 0.2f))))
                        .border(1.dp, CRTheme.brandElectric.copy(alpha = 0.3f), CircleShape),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = deviceName.take(1).uppercase(),
                        color = CRTheme.brandElectric,
                        fontSize = 40.sp,
                        fontWeight = FontWeight.ExtraBold
                    )
                }
            }

            Spacer(modifier = Modifier.height(32.dp))
            Text(
                text = "PAIRING REQUEST",
                fontSize = 12.sp,
                fontWeight = FontWeight.ExtraBold,
                color = CRTheme.inkSubtle(isDark),
                letterSpacing = 1.5.sp
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = deviceName,
                fontSize = 32.sp,
                fontWeight = FontWeight.ExtraBold,
                color = CRTheme.ink(isDark),
                letterSpacing = (-0.5).sp,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "wants to join your secure network",
                fontSize = 16.sp,
                color = CRTheme.inkSoft(isDark),
                fontWeight = FontWeight.Medium
            )

            Spacer(modifier = Modifier.height(40.dp))

            // PIN Block
            Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                Column(modifier = Modifier.padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "VERBAL CONFIRM",
                            fontSize = 11.sp,
                            fontWeight = FontWeight.ExtraBold,
                            color = CRTheme.brandElectric,
                            letterSpacing = 1.sp
                        )
                        Text(text = "Read aloud", fontSize = 12.sp, fontWeight = FontWeight.Medium, color = CRTheme.inkSoft(isDark))
                    }
                    Spacer(modifier = Modifier.height(20.dp))
                    Text(
                        text = pin.chunked(3).joinToString(" - "),
                        fontSize = 34.sp,
                        fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                        fontWeight = FontWeight.ExtraBold,
                        color = CRTheme.brandElectric,
                        letterSpacing = 6.sp
                    )
                }
            }

            Spacer(modifier = Modifier.height(20.dp))

            // Fingerprint Block
            Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 24.dp)) {
                Column(modifier = Modifier.padding(24.dp)) {
                    Text(
                        text = "DEVICE FINGERPRINT",
                        fontSize = 11.sp,
                        fontWeight = FontWeight.ExtraBold,
                        color = CRTheme.inkSubtle(isDark),
                        letterSpacing = 1.sp
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    
                    val pairs = if (fingerprint.isBlank()) emptyList() else fingerprint.replace(":", "").chunked(2)
                    val formatted = if (pairs.isEmpty()) "Not available" else pairs.chunked(8).joinToString("\n") { it.joinToString(":") }
                    
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(if (isDark) Color(0xFFFFFFFF).copy(alpha = 0.05f) else Color(0xFF000000).copy(alpha = 0.03f))
                            .border(1.dp, CRTheme.stroke(isDark), RoundedCornerShape(12.dp))
                            .padding(16.dp)
                    ) {
                        Text(
                            text = formatted,
                            fontSize = 13.sp,
                            fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                            fontWeight = FontWeight.SemiBold,
                            color = CRTheme.ink(isDark),
                            lineHeight = 24.sp,
                            letterSpacing = 1.sp
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(40.dp))

            // Timer info
            Text(
                text = "${(remainingMs / 1000).coerceAtLeast(0)}s remaining",
                fontSize = 15.sp,
                fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
                color = if (remainingMs < 10000) CRTheme.accentRed else CRTheme.inkSoft(isDark)
            )

            Spacer(modifier = Modifier.weight(1f))

            // Actions
            val haptic = LocalHapticFeedback.current
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onDeny()
                    },
                    modifier = Modifier.weight(1f).height(60.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (isDark) Color(0xFFFFFFFF).copy(alpha = 0.1f) else Color(0xFF000000).copy(alpha = 0.05f)
                    ),
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
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.brandElectric),
                    shape = RoundedCornerShape(16.dp)
                ) {
                    Text("Trust Device", color = Color.White, fontSize = 17.sp, fontWeight = FontWeight.Bold)
                }
            }
        }
    }
}
