package com.deskdrop.ui

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.deskdrop.PeerSnapshot
import com.deskdrop.ui.theme.CRBackground
import com.deskdrop.ui.theme.CRTheme
import com.deskdrop.ui.theme.CRTypography
import com.deskdrop.ui.theme.crGlassCard

@Composable
fun OnboardingScreen(
    isDark: Boolean,
    peers: List<PeerSnapshot>,
    onConnectPeer: (PeerSnapshot) -> Unit,
    onSendSampleText: (PeerSnapshot) -> Unit,
    onScanQr: () -> Unit,
    onManualIp: () -> Unit,
    onComplete: () -> Unit
) {
    var selectedPeerId by remember { mutableStateOf<String?>(null) }
    var forceCompletion by remember { mutableStateOf(false) }
    val sessionStartTimeSecs = remember { System.currentTimeMillis() / 1000 }
    val selectedPeer = peers.find { it.id == selectedPeerId }
    val haptic = androidx.compose.ui.platform.LocalHapticFeedback.current

    val currentStep = when {
        forceCompletion -> 3
        selectedPeer == null -> 0
        !selectedPeer.trusted -> 1
        selectedPeer.lastSyncSecs != null && selectedPeer.lastSyncSecs > sessionStartTimeSecs -> 3
        else -> 2
    }

    CRBackground(isDark = isDark) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .systemBarsPadding()
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            // Pagination
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.padding(top = 16.dp)) {
                repeat(4) { step ->
                    Box(
                        modifier = Modifier
                            .size(if (step == currentStep) 10.dp else 8.dp)
                            .clip(CircleShape)
                            .background(if (step == currentStep) CRTheme.blueSoft else CRTheme.stroke(isDark))
                    )
                }
            }

            Spacer(modifier = Modifier.height(32.dp))

            Box(modifier = Modifier.weight(1f)) {
                AnimatedContent(targetState = currentStep, label = "step") { step ->
                    when (step) {
                        0 -> StepOneFindDevice(isDark, peers, selectedPeer, onScanQr = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); onScanQr() }, onManualIp = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); onManualIp() }, onPeerSelect = {
                            haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                            selectedPeerId = it.id
                            onConnectPeer(it)
                        })
                        1 -> StepTwoPairing(isDark, selectedPeer, onCancel = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); selectedPeerId = null })
                        2 -> StepThreeSendSample(isDark, selectedPeer, onSend = {
                            haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove)
                            if (it != null) onSendSampleText(it)
                            forceCompletion = true
                        })
                        3 -> StepFourCompletion(isDark)
                    }
                }
            }

            // Footer
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = 16.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                if (currentStep == 0) {
                    TextButton(onClick = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); onComplete() }) {
                        Text("SKIP FOR NOW", color = CRTheme.textMedium(isDark), fontWeight = FontWeight.Bold)
                    }
                } else if (currentStep > 0 && currentStep < 3) {
                    TextButton(onClick = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); selectedPeerId = null }) {
                        Text("CANCEL", color = CRTheme.textMedium(isDark), fontWeight = FontWeight.Bold)
                    }
                } else {
                    Spacer(modifier = Modifier.width(64.dp))
                }

                if (currentStep == 3) {
                    Button(
                        onClick = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress); onComplete() },
                        colors = ButtonDefaults.buttonColors(containerColor = CRTheme.blueSoft)
                    ) {
                        Text("GET STARTED", color = CRTheme.bg(isDark), fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

@Composable
private fun StepOneFindDevice(isDark: Boolean, peers: List<PeerSnapshot>, selected: PeerSnapshot?, onScanQr: () -> Unit, onManualIp: () -> Unit, onPeerSelect: (PeerSnapshot) -> Unit) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Text("Welcome to Deskdrop", style = CRTypography.h1, color = CRTheme.textHigh(isDark))
        Spacer(modifier = Modifier.height(16.dp))
        Text("Let's link your computer to get started.", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
        
        Spacer(modifier = Modifier.height(24.dp))
        
        Button(
            onClick = onScanQr,
            colors = ButtonDefaults.buttonColors(containerColor = CRTheme.blueSoft),
            modifier = Modifier.fillMaxWidth(0.7f).height(56.dp)
        ) {
            Icon(Icons.Rounded.QrCodeScanner, contentDescription = null, modifier = Modifier.size(20.dp), tint = CRTheme.bg(isDark))
            Spacer(modifier = Modifier.width(8.dp))
            Text("SCAN QR CODE", color = CRTheme.bg(isDark), fontWeight = FontWeight.Bold)
        }

        Spacer(modifier = Modifier.height(8.dp))

        TextButton(onClick = onManualIp) {
            Text("Enter IP Manually", color = CRTheme.textMedium(isDark))
        }

        Spacer(modifier = Modifier.height(16.dp))
        
        LazyColumn(verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
            if (peers.isEmpty()) {
                item { 
                    Spacer(modifier = Modifier.height(24.dp))
                    RadarAnimation(isDark)
                    Spacer(modifier = Modifier.height(24.dp))
                    Text("Searching for nearby devices...", color = CRTheme.textMedium(isDark)) 
                }
            } else {
                items(peers.size) { idx ->
                    val peer = peers[idx]
                    val isSelected = selected?.id == peer.id
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .crGlassCard(isDark)
                            .border(1.dp, if (isSelected) CRTheme.blueSoft else CRTheme.stroke(isDark), RoundedCornerShape(24.dp))
                            .clickable { onPeerSelect(peer) }
                            .padding(16.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Box(
                            modifier = Modifier
                                .size(48.dp)
                                .clip(CircleShape)
                                .background(CRTheme.bg(isDark).copy(alpha = 0.5f)),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(
                                if (peer.name.lowercase().contains("mac")) Icons.Rounded.LaptopMac else Icons.Rounded.Computer,
                                contentDescription = null,
                                tint = CRTheme.textHigh(isDark),
                                modifier = Modifier.size(24.dp)
                            )
                        }
                        Spacer(modifier = Modifier.width(16.dp))
                        Text(peer.name, style = CRTypography.bodyMedium, color = CRTheme.textHigh(isDark), fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

@Composable
private fun StepTwoPairing(isDark: Boolean, selectedPeer: PeerSnapshot?, onCancel: () -> Unit) {
    var hasTimedOut by remember { mutableStateOf(false) }

    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Text("Connect & Pair", style = CRTypography.h1, color = CRTheme.textHigh(isDark))
        Spacer(modifier = Modifier.height(16.dp))
        
        if (hasTimedOut) {
            Text("Connection failed or timed out.", style = CRTypography.bodyMedium, color = CRTheme.accentRed, textAlign = TextAlign.Center)
            Spacer(modifier = Modifier.height(32.dp))
            Button(
                onClick = onCancel,
                colors = ButtonDefaults.buttonColors(containerColor = CRTheme.blueSoft)
            ) {
                Text("TRY AGAIN", color = CRTheme.bg(isDark), fontWeight = FontWeight.Bold)
            }
        } else {
            Text("Connecting to ${selectedPeer?.name ?: "the device"}...", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
            Spacer(modifier = Modifier.height(48.dp))
            LinkingAnimation(isDark)
            Spacer(modifier = Modifier.height(48.dp))
            Text("A pairing prompt with a secure PIN will appear shortly on your computer.", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
        }
    }
}

@Composable
private fun StepThreeSendSample(isDark: Boolean, selectedPeer: PeerSnapshot?, onSend: (PeerSnapshot?) -> Unit) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Text("Test Connection", style = CRTypography.h1, color = CRTheme.textHigh(isDark))
        Spacer(modifier = Modifier.height(16.dp))
        Text("Let's make sure it works. Click below to send a sample message.", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
        
        Spacer(modifier = Modifier.height(32.dp))
        
        Button(
            onClick = { onSend(selectedPeer) },
            colors = ButtonDefaults.buttonColors(containerColor = CRTheme.blueSoft),
            modifier = Modifier.fillMaxWidth(0.8f).height(56.dp)
        ) {
            Icon(Icons.Rounded.Send, contentDescription = null, tint = CRTheme.bg(isDark))
            Spacer(modifier = Modifier.width(8.dp))
            Text("SEND TEST MESSAGE", color = CRTheme.bg(isDark), fontWeight = FontWeight.Bold)
        }
    }
}

@Composable
private fun StepFourCompletion(isDark: Boolean) {
    val scale = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        scale.animateTo(
            targetValue = 1f,
            animationSpec = spring(
                dampingRatio = Spring.DampingRatioMediumBouncy,
                stiffness = Spring.StiffnessLow
            )
        )
    }

    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Box(
            modifier = Modifier
                .size(100.dp)
                .scale(scale.value)
                .clip(CircleShape)
                .background(CRTheme.statusGreen.copy(alpha = 0.1f)),
            contentAlignment = Alignment.Center
        ) {
            Icon(Icons.Rounded.CheckCircle, contentDescription = null, tint = CRTheme.statusGreen, modifier = Modifier.size(48.dp))
        }
        Spacer(modifier = Modifier.height(24.dp))
        Text("You're all set!", style = CRTypography.h1, color = CRTheme.textHigh(isDark))
        Spacer(modifier = Modifier.height(16.dp))
        Text("Received files will automatically appear here.\nClipboard text will be instantly available to paste.", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
    }
}

@Composable
fun RadarAnimation(isDark: Boolean) {
    val infiniteTransition = rememberInfiniteTransition(label = "radar")
    val scale by infiniteTransition.animateFloat(
        initialValue = 1f,
        targetValue = 2.5f,
        animationSpec = infiniteRepeatable(
            animation = tween(2000, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "radarScale"
    )
    val alpha by infiniteTransition.animateFloat(
        initialValue = 1f,
        targetValue = 0f,
        animationSpec = infiniteRepeatable(
            animation = tween(2000, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "radarAlpha"
    )
    
    Box(contentAlignment = Alignment.Center, modifier = Modifier.size(120.dp)) {
        Box(
            modifier = Modifier
                .size(40.dp)
                .scale(scale)
                .clip(CircleShape)
                .background(CRTheme.brandCyan.copy(alpha = alpha * 0.4f))
        )
        Box(
            modifier = Modifier
                .size(48.dp)
                .clip(CircleShape)
                .background(CRTheme.bg(isDark)),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                Icons.Rounded.Search, 
                contentDescription = null, 
                tint = CRTheme.brandCyan, 
                modifier = Modifier.size(24.dp)
            )
        }
    }
}

@Composable
fun LinkingAnimation(isDark: Boolean) {
    val infiniteTransition = rememberInfiniteTransition(label = "linking")
    val dotAlpha by infiniteTransition.animateFloat(
        initialValue = 0.2f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(800, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "dotAlpha"
    )

    Row(verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Rounded.Smartphone, contentDescription = null, tint = CRTheme.textHigh(isDark), modifier = Modifier.size(40.dp))
        Spacer(modifier = Modifier.width(16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            repeat(3) { i ->
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(CRTheme.brandCyan.copy(alpha = if (i % 2 == 0) dotAlpha else 1f - dotAlpha))
                )
            }
        }
        Spacer(modifier = Modifier.width(16.dp))
        Icon(Icons.Rounded.LaptopMac, contentDescription = null, tint = CRTheme.textHigh(isDark), modifier = Modifier.size(48.dp))
    }
}
