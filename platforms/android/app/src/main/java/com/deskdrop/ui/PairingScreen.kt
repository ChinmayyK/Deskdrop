package com.deskdrop.ui

import androidx.compose.animation.core.*
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.deskdrop.ui.theme.CRTheme
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

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(CRTheme.bg(isDark)) // Pure void
            .systemBarsPadding()
    ) {
        // Hair-thin progress bar at the very top
        LinearProgressIndicator(
            progress = { progress },
            modifier = Modifier.fillMaxWidth().height(2.dp),
            color = CRTheme.textHigh(isDark),
            trackColor = Color.Transparent
        )

        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(32.dp),
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            
            // Top Section: Request Info
            Column(modifier = Modifier.padding(top = 24.dp)) {
                Text(
                    text = "PAIRING REQUEST",
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = 4.sp,
                    color = CRTheme.brandElectric
                )
                Spacer(modifier = Modifier.height(32.dp))
                
                // Massive Device Name
                Text(
                    text = deviceName,
                    fontSize = 56.sp,
                    fontWeight = FontWeight.Black,
                    lineHeight = 60.sp,
                    letterSpacing = (-2).sp,
                    color = CRTheme.textHigh(isDark)
                )
                Spacer(modifier = Modifier.height(16.dp))
                Text(
                    text = "wants to connect.",
                    fontSize = 28.sp,
                    fontWeight = FontWeight.Light,
                    color = CRTheme.textMedium(isDark)
                )
            }
            
            // Middle Section: Massive PIN
            Column {
                Text(
                    text = "CONFIRM PIN",
                    fontSize = 10.sp,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = 2.sp,
                    color = CRTheme.textMedium(isDark)
                )
                Spacer(modifier = Modifier.height(16.dp))
                
                // Pure typographic PIN
                Row(
                    horizontalArrangement = Arrangement.spacedBy(24.dp)
                ) {
                    pin.forEach { char ->
                        Text(
                            text = char.toString(),
                            fontSize = 84.sp,
                            fontFamily = FontFamily.Monospace,
                            fontWeight = FontWeight.Light,
                            color = CRTheme.textHigh(isDark)
                        )
                    }
                }
            }
            
            // Bottom Section: Actions & Fingerprint
            Column {
                // Microscopic Fingerprint
                val formattedFingerprint = if (fingerprint.isBlank()) "N/A" else fingerprint.replace(":", "").chunked(4).joinToString(" ")
                Text(
                    text = "FINGERPRINT: $formattedFingerprint",
                    fontSize = 9.sp,
                    fontFamily = FontFamily.Monospace,
                    color = CRTheme.textLow(isDark),
                    lineHeight = 14.sp
                )
                
                Spacer(modifier = Modifier.height(48.dp))
                
                // Brutalist Buttons
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    // Stark outlined deny
                    OutlinedButton(
                        onClick = {
                            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                            onDeny()
                        },
                        modifier = Modifier.weight(1f).height(64.dp).crPressScale(0.95f),
                        shape = RoundedCornerShape(0.dp), // Sharp corners
                        border = BorderStroke(1.dp, CRTheme.stroke(isDark))
                    ) {
                        Text("DENY", color = CRTheme.textHigh(isDark), letterSpacing = 2.sp, fontWeight = FontWeight.Bold)
                    }
                    
                    // Stark solid trust
                    Button(
                        onClick = {
                            haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                            onApprove()
                        },
                        modifier = Modifier.weight(1f).height(64.dp).crPressScale(0.95f),
                        shape = RoundedCornerShape(0.dp), // Sharp corners
                        colors = ButtonDefaults.buttonColors(containerColor = CRTheme.textHigh(isDark))
                    ) {
                        Text("TRUST", color = CRTheme.bg(isDark), letterSpacing = 2.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}
