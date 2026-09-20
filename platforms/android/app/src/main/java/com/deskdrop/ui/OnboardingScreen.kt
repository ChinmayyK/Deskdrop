package com.deskdrop.ui

import com.deskdrop.ui.theme.*

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.graphics.Color
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
import kotlinx.coroutines.delay
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
        forceCompletion -> 1
        selectedPeer == null -> 0
        !selectedPeer.trusted -> 1
        else -> 1 // Handled by LaunchedEffect
    }

    LaunchedEffect(selectedPeer?.trusted) {
        if (selectedPeer?.trusted == true) {
            onComplete()
        }
    }

    CRBackground(isDark = isDark) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .systemBarsPadding()
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Spacer(modifier = Modifier.height(16.dp))

            Box(modifier = Modifier.weight(1f).widthIn(max = 520.dp)) {
                AnimatedContent(targetState = currentStep, label = "step") { step ->
                    when (step) {
                        0 -> StepOneFindDevice(isDark, peers, selectedPeer, onScanQr = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); onScanQr() }, onManualIp = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); onManualIp() }, onPeerSelect = {
                            haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                            selectedPeerId = it.id
                            onConnectPeer(it)
                        })
                        1 -> StepTwoPairing(isDark, selectedPeer, onCancel = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); selectedPeerId = null })
                    }
                }
            }

            // Footer (Simplified)
            if (currentStep == 1) {
                TextButton(
                    onClick = { haptic.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove); selectedPeerId = null },
                    modifier = Modifier.padding(bottom = 8.dp)
                ) {
                    Text("Cancel", color = CRTheme.textMedium(isDark), style = CRTypography.label)
                }
            }
        }
    }
}

@Composable
private fun StepOneFindDevice(isDark: Boolean, peers: List<PeerSnapshot>, selected: PeerSnapshot?, onScanQr: () -> Unit, onManualIp: () -> Unit, onPeerSelect: (PeerSnapshot) -> Unit) {
    LazyColumn(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
        modifier = Modifier.fillMaxSize()
    ) {
        item {
            Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
                androidx.compose.foundation.Image(
                    painter = androidx.compose.ui.res.painterResource(
                        if (isDark) com.deskdrop.R.drawable.dark_logo else com.deskdrop.R.drawable.light_logo
                    ),
                    contentDescription = null,
                    modifier = Modifier.size(88.dp)
                )
                Spacer(modifier = Modifier.height(20.dp))
                Text(
                    "Welcome to Deskdrop",
                    style = CRTypography.h1,
                    color = CRTheme.textHigh(isDark),
                    textAlign = TextAlign.Center
                )
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    "Your phone and your computer, one clipboard. No cloud in between.",
                    style = CRTypography.bodyMedium,
                    color = CRTheme.textMedium(isDark),
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(horizontal = 12.dp)
                )
                Spacer(modifier = Modifier.height(28.dp))

                Button(
                    onClick = onScanQr,
                    shape = RoundedCornerShape(16.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.blueSoft),
                    modifier = Modifier.fillMaxWidth().height(56.dp)
                ) {
                    Icon(Icons.Rounded.QrCodeScanner, contentDescription = null, modifier = Modifier.size(20.dp), tint = Color.White)
                    Spacer(modifier = Modifier.width(10.dp))
                    Text("Scan QR code", color = Color.White, style = CRTypography.label, fontWeight = FontWeight.SemiBold)
                }
                Spacer(modifier = Modifier.height(10.dp))
                OutlinedButton(
                    onClick = onManualIp,
                    shape = RoundedCornerShape(16.dp),
                    border = androidx.compose.foundation.BorderStroke(1.dp, CRTheme.stroke(isDark)),
                    modifier = Modifier.fillMaxWidth().height(52.dp)
                ) {
                    Icon(Icons.Rounded.Lan, contentDescription = null, modifier = Modifier.size(18.dp), tint = CRTheme.textHigh(isDark))
                    Spacer(modifier = Modifier.width(10.dp))
                    Text("Enter IP address", color = CRTheme.textHigh(isDark), style = CRTypography.label)
                }

                Spacer(modifier = Modifier.height(32.dp))
                Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text("Nearby devices", style = CRTypography.h2, color = CRTheme.textHigh(isDark))
                    Spacer(modifier = Modifier.weight(1f))
                    SearchingPill(isDark)
                }
                Spacer(modifier = Modifier.height(4.dp))
            }
        }

        if (peers.isEmpty()) {
            item {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    modifier = Modifier
                        .fillMaxWidth()
                        .crGlassCard(isDark, cornerRadius = 24.dp)
                        .padding(vertical = 28.dp, horizontal = 20.dp)
                ) {
                    RadarAnimation(isDark)
                    Spacer(modifier = Modifier.height(12.dp))
                    Text("Looking around your network…", style = CRTypography.label, color = CRTheme.textHigh(isDark))
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        "Open Deskdrop on your computer and keep both devices on the same Wi-Fi.",
                        style = CRTypography.caption,
                        color = CRTheme.textMedium(isDark),
                        textAlign = TextAlign.Center
                    )
                }
            }
        } else {
            items(peers.size, key = { peers[it].id }) { idx ->
                val peer = peers[idx]
                NearbyDeviceRow(isDark, peer, isSelected = selected?.id == peer.id, onClick = { onPeerSelect(peer) })
            }
        }
        item { Spacer(modifier = Modifier.height(8.dp)) }
    }
}

@Composable
private fun NearbyDeviceRow(isDark: Boolean, peer: PeerSnapshot, isSelected: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .crGlassCard(isDark, cornerRadius = 20.dp, highlighted = isSelected, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .size(44.dp)
                .clip(CircleShape)
                .background(CRTheme.blueSoft.copy(alpha = 0.12f)),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                if (peer.name.lowercase().contains("mac")) Icons.Rounded.LaptopMac else Icons.Rounded.Computer,
                contentDescription = null,
                tint = CRTheme.blueSoft,
                modifier = Modifier.size(22.dp)
            )
        }
        Spacer(modifier = Modifier.width(14.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                peer.name,
                style = CRTypography.label,
                fontWeight = FontWeight.SemiBold,
                color = CRTheme.textHigh(isDark),
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis
            )
            Text(
                peer.ip ?: "On your network",
                style = CRTypography.caption,
                color = CRTheme.textMedium(isDark),
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis
            )
        }
        Spacer(modifier = Modifier.width(8.dp))
        Text("Pair", style = CRTypography.label, fontWeight = FontWeight.SemiBold, color = CRTheme.blueSoft)
        Icon(Icons.Rounded.ChevronRight, contentDescription = null, tint = CRTheme.blueSoft, modifier = Modifier.size(20.dp))
    }
}

@Composable
private fun SearchingPill(isDark: Boolean) {
    val pulse by rememberInfiniteTransition(label = "searching").animateFloat(
        initialValue = 0.3f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(900, easing = LinearEasing), RepeatMode.Reverse),
        label = "searchingAlpha"
    )
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .background(CRTheme.brandCyan.copy(alpha = 0.12f))
            .padding(horizontal = 10.dp, vertical = 4.dp)
    ) {
        Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(CRTheme.brandCyan.copy(alpha = pulse)))
        Spacer(modifier = Modifier.width(6.dp))
        Text("Scanning", style = CRTypography.caption, color = CRTheme.brandCyan)
    }
}

@Composable
private fun StepTwoPairing(isDark: Boolean, selectedPeer: PeerSnapshot?, onCancel: () -> Unit) {
    var hasTimedOut by remember { mutableStateOf(false) }

    LaunchedEffect(selectedPeer) {
        hasTimedOut = false
        delay(30000)
        hasTimedOut = true
    }

    val deviceName = selectedPeer?.name ?: "your computer"

    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState())
    ) {
        Text("Connect & Pair", style = CRTypography.h1, color = CRTheme.textHigh(isDark), textAlign = TextAlign.Center)
        Spacer(modifier = Modifier.height(12.dp))

        // Which device this is about, as a chip rather than buried in a sentence.
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .clip(RoundedCornerShape(50))
                .background(CRTheme.textMedium(isDark).copy(alpha = 0.08f))
                .padding(horizontal = 14.dp, vertical = 8.dp)
        ) {
            Icon(Icons.Rounded.Computer, contentDescription = null, tint = CRTheme.textHigh(isDark), modifier = Modifier.size(16.dp))
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                deviceName,
                style = CRTypography.label,
                color = CRTheme.textHigh(isDark),
                maxLines = 1,
                overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis
            )
        }
        Spacer(modifier = Modifier.height(32.dp))

        when {
            hasTimedOut -> {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    modifier = Modifier
                        .fillMaxWidth()
                        .crGlassCard(isDark, cornerRadius = 24.dp)
                        .padding(24.dp)
                ) {
                    Box(
                        modifier = Modifier.size(52.dp).clip(CircleShape).background(CRTheme.accentRed.copy(alpha = 0.12f)),
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(Icons.Rounded.WifiOff, contentDescription = null, tint = CRTheme.accentRed, modifier = Modifier.size(24.dp))
                    }
                    Spacer(modifier = Modifier.height(16.dp))
                    Text("Couldn't reach $deviceName", style = CRTypography.h2, color = CRTheme.textHigh(isDark), textAlign = TextAlign.Center)
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        "Check that Deskdrop is open on it and both devices are on the same Wi-Fi.",
                        style = CRTypography.caption,
                        color = CRTheme.textMedium(isDark),
                        textAlign = TextAlign.Center
                    )
                    Spacer(modifier = Modifier.height(20.dp))
                    Button(
                        onClick = onCancel,
                        shape = RoundedCornerShape(16.dp),
                        colors = ButtonDefaults.buttonColors(containerColor = CRTheme.blueSoft),
                        modifier = Modifier.fillMaxWidth().height(52.dp)
                    ) {
                        Text("Try again", color = Color.White, style = CRTypography.label, fontWeight = FontWeight.SemiBold)
                    }
                }
            }

            selectedPeer?.pairingPin != null -> {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.Rounded.Lock, contentDescription = null, tint = CRTheme.statusGreen, modifier = Modifier.size(18.dp))
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Pairing code", style = CRTypography.label, color = CRTheme.textHigh(isDark), fontWeight = FontWeight.SemiBold)
                }
                Spacer(modifier = Modifier.height(20.dp))

                val digits = selectedPeer.pairingPin.filter { it.isDigit() }.padStart(6, '0').takeLast(6)
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth().widthIn(max = 420.dp)
                ) {
                    digits.forEachIndexed { index, char ->
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .aspectRatio(0.8f)
                                .crGlassCard(isDark, cornerRadius = 14.dp, elevated = true),
                            contentAlignment = Alignment.Center
                        ) {
                            Text(char.toString(), style = CRTypography.h1.copy(fontSize = 30.sp), color = CRTheme.textHigh(isDark))
                        }
                        if (index == 2) {
                            Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(CRTheme.textMedium(isDark).copy(alpha = 0.5f)))
                        }
                    }
                }
                Spacer(modifier = Modifier.height(20.dp))
                Text(
                    "Make sure $deviceName shows the same code, then accept there.",
                    style = CRTypography.bodyMedium,
                    color = CRTheme.textMedium(isDark),
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(horizontal = 16.dp)
                )
            }

            else -> {
                LinkingAnimation(isDark)
                Spacer(modifier = Modifier.height(28.dp))
                Text("Saying hello…", style = CRTypography.h2, color = CRTheme.textHigh(isDark))
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    "A pairing prompt will pop up on $deviceName in a moment.",
                    style = CRTypography.bodyMedium,
                    color = CRTheme.textMedium(isDark),
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(horizontal = 16.dp)
                )
            }
        }
        Spacer(modifier = Modifier.height(24.dp))
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
                contentDescription = "Search", 
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
        Icon(Icons.Rounded.Smartphone, contentDescription = "Phone", tint = CRTheme.textHigh(isDark), modifier = Modifier.size(40.dp))
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
        Icon(Icons.Rounded.LaptopMac, contentDescription = "Computer", tint = CRTheme.textHigh(isDark), modifier = Modifier.size(48.dp))
    }
}
