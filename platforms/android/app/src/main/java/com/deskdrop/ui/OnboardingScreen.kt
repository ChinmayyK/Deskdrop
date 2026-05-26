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
    onTrustPeer: (PeerSnapshot) -> Unit,
    onSendSampleText: (PeerSnapshot) -> Unit,
    onComplete: () -> Unit
) {
    var currentStep by remember { mutableStateOf(0) }
    var selectedPeer by remember { mutableStateOf<PeerSnapshot?>(null) }

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
                        0 -> StepOneFindDevice(isDark, peers, selectedPeer, onPeerSelect = { selectedPeer = it; currentStep = 1 })
                        1 -> StepTwoVerify(isDark, selectedPeer, onTrust = { 
                            if (it != null) onTrustPeer(it)
                            currentStep = 2 
                        })
                        2 -> StepThreeSendSample(isDark, selectedPeer, onSend = {
                            if (it != null) onSendSampleText(it)
                            currentStep = 3
                        })
                        3 -> StepFourCompletion(isDark, onComplete = onComplete)
                    }
                }
            }

            // Footer
            Row(
                modifier = Modifier.fillMaxWidth().padding(bottom = 16.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                if (currentStep > 0) {
                    TextButton(onClick = { currentStep -= 1 }) {
                        Text("BACK", color = CRTheme.textMedium(isDark), fontWeight = FontWeight.Bold)
                    }
                } else {
                    Spacer(modifier = Modifier.width(64.dp))
                }

                if (currentStep < 3) {
                    Button(
                        onClick = { currentStep += 1 },
                        colors = ButtonDefaults.buttonColors(containerColor = CRTheme.blueSoft),
                        enabled = (currentStep == 0 && selectedPeer != null) || currentStep > 0
                    ) {
                        Text("NEXT", color = CRTheme.bg(isDark), fontWeight = FontWeight.Bold)
                    }
                } else {
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
private fun StepTwoVerify(isDark: Boolean, selectedPeer: PeerSnapshot?, onTrust: (PeerSnapshot?) -> Unit) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
        Text("Step 2: Verify & Trust", style = CRTypography.h1, color = CRTheme.textHigh(isDark))
        Spacer(modifier = Modifier.height(16.dp))
        Text("Ensure this matches the code on ${selectedPeer?.name ?: "the device"}:", style = CRTypography.bodyMedium, color = CRTheme.textMedium(isDark), textAlign = TextAlign.Center)
        
        Spacer(modifier = Modifier.height(32.dp))
        
        Box(
            modifier = Modifier
                .clip(RoundedCornerShape(16.dp))
                .background(CRTheme.surface(isDark))
                .padding(24.dp)
        ) {
            Text(
                (selectedPeer?.id?.take(6) ?: "------").uppercase(),
                fontSize = 32.sp,
                fontWeight = FontWeight.Black,
                letterSpacing = 8.sp,
                color = CRTheme.textHigh(isDark)
            )
        }
        
        Spacer(modifier = Modifier.height(32.dp))
        
        Button(
            onClick = { onTrust(selectedPeer) },
            colors = ButtonDefaults.buttonColors(containerColor = CRTheme.statusGreen)
        ) {
            Text("TRUST DEVICE", color = CRTheme.bg(isDark), fontWeight = FontWeight.Bold)
        }
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
private fun StepFourCompletion(isDark: Boolean, onComplete: () -> Unit) {
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
