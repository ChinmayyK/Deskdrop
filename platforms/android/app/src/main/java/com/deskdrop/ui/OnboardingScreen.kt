package com.deskdrop.ui

import androidx.compose.animation.*
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
    onComplete: () -> Unit
) {
    var selectedPeerId by remember { mutableStateOf<String?>(null) }
    var forceCompletion by remember { mutableStateOf(false) }
    val sessionStartTimeSecs = remember { System.currentTimeMillis() / 1000 }
    val selectedPeer = peers.find { it.id == selectedPeerId }

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
                        0 -> StepOneFindDevice(isDark, peers, selectedPeer, onPeerSelect = {
                            selectedPeerId = it.id
                            onConnectPeer(it)
                        })
                        1 -> StepTwoPairing(isDark, selectedPeer)
                        2 -> StepThreeSendSample(isDark, selectedPeer, onSend = {
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
                    TextButton(onClick = onComplete) {
                        Text("SKIP FOR NOW", color = CRTheme.textMedium(isDark), fontWeight = FontWeight.Bold)
                    }
                } else if (currentStep > 0 && currentStep < 3) {
                    TextButton(onClick = { selectedPeerId = null }) {
                        Text("CANCEL", color = CRTheme.textMedium(isDark), fontWeight = FontWeight.Bold)
                    }
                } else {
                    Spacer(modifier = Modifier.width(64.dp))
                }

                if (currentStep == 3) {
                    Button(
                        onClick = onComplete,
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
private fun StepOneFindDevice(isDark: Boolean, peers: List<PeerSnapshot>, selected: PeerSnapshot?, onPeerSelect: (PeerSnapshot) -> Unit) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Text("Step 1: Find a device", style = CRTypography.h1, color = CRTheme.textHigh(isDark))
        Spacer(modifier = Modifier.height(16.dp))
        Text("Make sure Deskdrop is running on your Mac or PC.", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
        
        Spacer(modifier = Modifier.height(32.dp))
        
        LazyColumn(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            if (peers.isEmpty()) {
                item { Text("Searching for nearby devices...", color = CRTheme.textMedium(isDark)) }
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
                        Icon(
                            if (peer.name.lowercase().contains("mac")) Icons.Rounded.LaptopMac else Icons.Rounded.Computer,
                            contentDescription = null,
                            tint = CRTheme.textHigh(isDark),
                            modifier = Modifier.size(32.dp)
                        )
                        Spacer(modifier = Modifier.width(16.dp))
                        Text(peer.name, style = CRTypography.bodyMedium, color = CRTheme.textHigh(isDark), fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

@Composable
private fun StepTwoPairing(isDark: Boolean, selectedPeer: PeerSnapshot?) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Text("Step 2: Connect & Pair", style = CRTypography.h1, color = CRTheme.textHigh(isDark))
        Spacer(modifier = Modifier.height(16.dp))
        Text("Connecting to ${selectedPeer?.name ?: "the device"}...", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
        
        Spacer(modifier = Modifier.height(32.dp))
        
        CircularProgressIndicator(color = CRTheme.blueSoft)
        
        Spacer(modifier = Modifier.height(32.dp))
        
        Text("A pairing prompt with a secure PIN will appear shortly.", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
    }
}

@Composable
private fun StepThreeSendSample(isDark: Boolean, selectedPeer: PeerSnapshot?, onSend: (PeerSnapshot?) -> Unit) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Text("Step 3: Send Sample Text", style = CRTypography.h1, color = CRTheme.textHigh(isDark))
        Spacer(modifier = Modifier.height(16.dp))
        Text("Let's make sure it works. Click below to send a sample message.", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
        
        Spacer(modifier = Modifier.height(32.dp))
        
        Button(
            onClick = { onSend(selectedPeer) },
            colors = ButtonDefaults.buttonColors(containerColor = CRTheme.blueSoft)
        ) {
            Text("SEND 'HELLO FROM ANDROID'", color = CRTheme.bg(isDark), fontWeight = FontWeight.Bold)
        }
    }
}

@Composable
private fun StepFourCompletion(isDark: Boolean) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Box(
            modifier = Modifier
                .size(100.dp)
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
