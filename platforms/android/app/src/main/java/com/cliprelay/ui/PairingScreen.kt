package com.cliprelay.ui

import androidx.compose.animation.core.animateFloatAsState
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
    deviceName: String,
    pin: String,
    fingerprint: String,
    onApprove: () -> Unit,
    onDeny: () -> Unit
) {
    var remainingMs by remember { mutableLongStateOf(30_000L) }
    val isDark = isSystemInDarkTheme()

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
    val timerColor = if (progress > 0.3f) CRTheme.accentAmber else CRTheme.accentRed

    CRBackground(isDark = isDark) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .systemBarsPadding()
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            // Glowing Circular Timer with Avatar
            Box(
                modifier = Modifier
                    .size(100.dp),
                contentAlignment = Alignment.Center
            ) {
                CircularProgressIndicator(
                    progress = { progress },
                    modifier = Modifier.fillMaxSize().shadow(if (remainingMs < 10000) 24.dp else 12.dp, CircleShape, ambientColor = timerColor, spotColor = timerColor),
                    color = timerColor,
                    trackColor = CRTheme.stroke(isDark),
                    strokeWidth = 6.dp
                )
                Box(
                    modifier = Modifier
                        .size(76.dp)
                        .clip(CircleShape)
                        .background(CRTheme.brandElectric.copy(alpha = 0.12f))
                        .border(0.5.dp, CRTheme.brandElectric.copy(alpha = 0.16f), CircleShape),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = deviceName.take(1).uppercase(),
                        color = CRTheme.brandElectric,
                        fontSize = 32.sp,
                        fontWeight = FontWeight.Bold
                    )
                }
            }

            Spacer(modifier = Modifier.height(24.dp))
            Text(
                text = "PAIRING REQUEST",
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold,
                color = CRTheme.inkSubtle(isDark),
                letterSpacing = 1.sp
            )
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = deviceName,
                fontSize = 26.sp,
                fontWeight = FontWeight.Bold,
                color = CRTheme.ink(isDark),
                letterSpacing = (-0.4).sp
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = "wants to join your clipboard network",
                fontSize = 14.5f.sp,
                color = CRTheme.inkSoft(isDark)
            )

            Spacer(modifier = Modifier.height(32.dp))

            // PIN Block
            Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 16.dp)) {
                Column(modifier = Modifier.padding(20.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "VERBAL CONFIRMATION",
                            fontSize = 10.5f.sp,
                            fontWeight = FontWeight.Bold,
                            color = CRTheme.brandElectric,
                            letterSpacing = 1.sp
                        )
                        Text(text = "Read aloud to verify", fontSize = 11.5f.sp, color = CRTheme.inkSoft(isDark))
                    }
                    Spacer(modifier = Modifier.height(16.dp))
                    Text(
                        text = pin.chunked(3).joinToString(" - "),
                        fontSize = 28.sp,
                        fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                        fontWeight = FontWeight.Bold,
                        color = CRTheme.brandElectric,
                        letterSpacing = 4.sp
                    )
                }
            }

            Spacer(modifier = Modifier.height(16.dp))

            // Fingerprint Block
            Box(modifier = Modifier.fillMaxWidth().crCard(isDark, cornerRadius = 16.dp)) {
                Column(modifier = Modifier.padding(20.dp)) {
                    Text(
                        text = "DEVICE FINGERPRINT",
                        fontSize = 10.5f.sp,
                        fontWeight = FontWeight.Bold,
                        color = CRTheme.inkSubtle(isDark),
                        letterSpacing = 1.sp
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                    
                    val pairs = if (fingerprint.isBlank()) emptyList() else fingerprint.replace(":", "").chunked(2)
                    val formatted = if (pairs.isEmpty()) "Not available" else pairs.chunked(8).joinToString("\n") { it.joinToString(":") }
                    
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(10.dp))
                            .background(if (isDark) Color(0xFFFFFFFF).copy(alpha = 0.05f) else Color(0xFF000000).copy(alpha = 0.03f))
                            .border(0.5.dp, CRTheme.stroke(isDark), RoundedCornerShape(10.dp))
                            .padding(12.dp)
                    ) {
                        Text(
                            text = formatted,
                            fontSize = 12.5f.sp,
                            fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                            color = CRTheme.ink(isDark),
                            lineHeight = 22.sp,
                            letterSpacing = 0.5.sp
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(32.dp))

            // Timer info
            Text(
                text = "Time remaining: ${(remainingMs / 1000).coerceAtLeast(0)}s",
                fontSize = 13.sp,
                fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                fontWeight = FontWeight.SemiBold,
                color = if (remainingMs < 10000) CRTheme.accentRed else CRTheme.inkSoft(isDark)
            )

            Spacer(modifier = Modifier.height(32.dp))

            // Actions
            val haptic = LocalHapticFeedback.current
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onDeny()
                    },
                    modifier = Modifier.weight(1f).height(54.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (isDark) Color(0xFFFFFFFF).copy(alpha = 0.1f) else Color(0xFF000000).copy(alpha = 0.05f)
                    ),
                    shape = RoundedCornerShape(14.dp)
                ) {
                    Text("Deny", color = CRTheme.ink(isDark), fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                }
                Spacer(modifier = Modifier.width(16.dp))
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onApprove()
                    },
                    modifier = Modifier.weight(1f).height(54.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.brandElectric),
                    shape = RoundedCornerShape(14.dp)
                ) {
                    Text("Trust Device", color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                }
            }
        }
    }
}
