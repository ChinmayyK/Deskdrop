package com.deskdrop.ui

import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.foundation.ScrollState
import androidx.compose.ui.graphics.graphicsLayer

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.core.updateTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.animateDp
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.Spring
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.input.pointer.pointerInput
import kotlinx.coroutines.launch
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Wifi
import androidx.compose.material.icons.filled.SettingsInputAntenna
import androidx.compose.material.icons.filled.LinkOff
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.graphics.asImageBitmap
import com.deskdrop.*
import com.deskdrop.ui.getLocalIpAddress
import androidx.compose.material3.TextButton
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import com.deskdrop.ActivityEntry
import com.deskdrop.ActivityKind
import com.deskdrop.PeerSnapshot
import com.deskdrop.TransferProgress
import com.deskdrop.ui.theme.CRBackground
import com.deskdrop.ui.theme.CRTheme
import com.deskdrop.ui.theme.CRTypography
import com.deskdrop.ui.theme.crGlassCard
import com.deskdrop.ui.theme.crPressScale

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
fun DeviceCard(
    isDark: Boolean,
    peer: PeerSnapshot,
    onSendFiles: () -> Unit,
    onDropFiles: (List<android.net.Uri>) -> Unit = {},
    onPair: () -> Unit,
    onForget: () -> Unit,
    onReject: () -> Unit,
    modifier: Modifier = Modifier.width(170.dp)
) {
    val haptic = LocalHapticFeedback.current
    val isPhone = peer.name.contains("phone", ignoreCase = true) || peer.name.contains("pixel", ignoreCase = true)
    var showMenu by remember { mutableStateOf(false) }
    
    val infiniteTransition = rememberInfiniteTransition(label = "glow")
    val idleGlowAlpha by infiniteTransition.animateFloat(
        initialValue = 0.1f,
        targetValue = 0.4f,
        animationSpec = infiniteRepeatable(
            animation = tween(1500, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "glow_alpha"
    )

    var isHovered by remember { mutableStateOf(false) }
    val glowAlpha = if (isHovered) 1.0f else (if (peer.isConnected) idleGlowAlpha else 0.0f)
    
    Box(modifier = modifier
        .height(116.dp)
        .dragAndDropTarget(
            shouldStartDragAndDrop = { event -> 
                val androidEvent = event.toAndroidDragEvent()
                androidEvent.clipData != null && androidEvent.clipData.itemCount > 0
            },
            target = object : DragAndDropTarget {
                override fun onDrop(event: DragAndDropEvent): Boolean {
                    isHovered = false
                    val androidEvent = event.toAndroidDragEvent()
                    val clipData = androidEvent.clipData ?: return false
                    val uris = mutableListOf<android.net.Uri>()
                    for (i in 0 until clipData.itemCount) {
                        clipData.getItemAt(i).uri?.let { uris.add(it) }
                    }
                    if (uris.isNotEmpty()) {
                        onDropFiles(uris)
                        return true
                    }
                    return false
                }
                override fun onEntered(event: DragAndDropEvent) { isHovered = true }
                override fun onExited(event: DragAndDropEvent) { isHovered = false }
                override fun onEnded(event: DragAndDropEvent) { isHovered = false }
            }
        )
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .crPressScale(targetScale = 0.95f)
                .then(
                    if (glowAlpha > 0f) Modifier.border(if (isHovered) 2.dp else 1.dp, CRTheme.statusGreen.copy(alpha = glowAlpha), RoundedCornerShape(24.dp))
                    else Modifier
                )
                .crGlassCard(isDark = isDark, cornerRadius = 24.dp, onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    if (!peer.trusted) {
                        onPair()
                    } else {
                        showMenu = true
                    }
                })
                .padding(20.dp),
            horizontalAlignment = Alignment.Start,
            verticalArrangement = Arrangement.SpaceBetween
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Box(
                    modifier = Modifier
                        .size(38.dp)
                        .background(if (peer.trusted) CRTheme.blueSoft.copy(alpha = 0.15f) else CRTheme.textMedium(isDark).copy(alpha = 0.1f), CircleShape),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = if (isPhone) Icons.Default.Smartphone else Icons.Default.LaptopMac,
                        contentDescription = "Device type",
                        tint = if (peer.trusted) CRTheme.blueSoft else CRTheme.textMedium(isDark),
                        modifier = Modifier.size(20.dp)
                    )
                }
            }
            
            Column {
                Text(
                    text = peer.name,
                    style = CRTypography.label,
                    color = CRTheme.textHigh(isDark),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
                if (peer.trusted) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        if (peer.isConnected) {
                            Box(
                                modifier = Modifier
                                    .padding(end = 6.dp)
                                    .size(6.dp)
                                    .blur(1.dp)
                                    .background(CRTheme.statusGreen, CircleShape)
                            ) {
                                Box(modifier = Modifier.size(6.dp).background(CRTheme.statusGreen, CircleShape))
                            }
                        }
                        Text(
                            text = if (peer.isConnected) "Nearby" else "Offline",
                            style = CRTypography.caption,
                            color = CRTheme.textMedium(isDark)
                        )
                    }
                } else {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Box(modifier = Modifier.padding(end = 6.dp).size(6.dp).background(CRTheme.statusAmber, CircleShape))
                        Text(
                            text = "Pending",
                            style = CRTypography.caption,
                            color = CRTheme.statusAmber
                        )
                    }
                }
            }
        }
        
        androidx.compose.material3.DropdownMenu(
            expanded = showMenu,
            onDismissRequest = { showMenu = false },
            modifier = Modifier.background(if (isDark) Color(0xFF1E1E1E) else Color.White)
        ) {
            if (peer.isConnected) {
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text("Send Files", color = CRTheme.textHigh(isDark)) },
                    onClick = { showMenu = false; onSendFiles() }
                )
            }
            androidx.compose.material3.DropdownMenuItem(
                text = { Text("Forget Device", color = CRTheme.accentRed) },
                onClick = {
                    showMenu = false
                    onForget()
                },
                leadingIcon = {
                    Icon(
                        Icons.Default.Delete,
                        contentDescription = "Remove",
                        tint = CRTheme.accentRed
                    )
                }
            )
            androidx.compose.material3.DropdownMenuItem(
                text = { Text("Revoke Trust", color = CRTheme.accentRed) },
                onClick = {
                    showMenu = false
                    onReject()
                },
                leadingIcon = {
                    Icon(
                        Icons.Default.Block,
                        contentDescription = "Revoke trust",
                        tint = CRTheme.accentRed
                    )
                }
            )
        }
    }
}

@Composable
fun AddDeviceCard(isDark: Boolean, onClick: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    Column(
        modifier = Modifier
            .width(170.dp)
            .height(116.dp)
            .crPressScale(targetScale = 0.95f)
            .crGlassCard(
                isDark = isDark,
                cornerRadius = 24.dp,
                dashed = true,
                onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onClick()
                }
            ),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Icon(
            imageVector = Icons.Default.Add,
            contentDescription = "Add Device",
            tint = CRTheme.textMedium(isDark),
            modifier = Modifier.size(32.dp)
        )
        Spacer(modifier = Modifier.height(12.dp))
        Text(
            text = "Add Device",
            style = CRTypography.label,
            color = CRTheme.textMedium(isDark)
        )
    }
}

@Composable
fun DevicesTab(
    isDark: Boolean,
    peers: List<PeerSnapshot>,
    onTrustPeer: (PeerSnapshot) -> Unit,
    onRejectPeer: (PeerSnapshot) -> Unit,
    onConnectPeer: (PeerSnapshot) -> Unit,
    onDisconnectPeer: (PeerSnapshot) -> Unit,
    onSendPairingRequest: (PeerSnapshot) -> Unit,
    onRespondPairing: (PeerSnapshot, Boolean) -> Unit
) {
    Column(modifier = Modifier.fillMaxSize()) {
        Text(
            text = "All Devices",
            style = CRTypography.h2,
            color = CRTheme.textHigh(isDark),
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
        )
        LazyColumn(
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            item {
                HotspotTipCard(isDark = isDark)
            }
            if (peers.isEmpty()) {
                item {
                    EmptyStateGlassCard(
                        isDark = isDark,
                        icon = Icons.Default.Devices,
                        title = "No Devices Found",
                        description = "Make sure Deskdrop is running on your other devices on the same Wi-Fi network."
                    )
                }
            } else {
                items(peers) { peer ->
                    PeerListCard(
                        isDark = isDark,
                        peer = peer,
                        onTrust = { onTrustPeer(peer) },
                        onReject = { onRejectPeer(peer) },
                        onPair = { onSendPairingRequest(peer) },
                        onConnect = { onConnectPeer(peer) },
                        onDisconnect = { onDisconnectPeer(peer) },
                        onRespond = { accepted -> onRespondPairing(peer, accepted) }
                    )
                }
            }
        }
    }
}

@Composable
fun HotspotTipCard(isDark: Boolean) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .crGlassCard(isDark = isDark, cornerRadius = 16.dp)
            .padding(horizontal = 20.dp, vertical = 20.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .size(36.dp)
                    .background(CRTheme.brandCyan.copy(alpha = 0.15f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = Icons.Rounded.Wifi,
                    contentDescription = "Wi-Fi",
                    tint = CRTheme.brandCyan,
                    modifier = Modifier.size(18.dp)
                )
            }
            Spacer(modifier = Modifier.width(16.dp))
            Text(
                text = "Choose a connection method",
                style = CRTypography.bodyMedium,
                color = CRTheme.textHigh(isDark)
            )
        }
        
        Spacer(modifier = Modifier.height(12.dp))
        
        Column(
            modifier = Modifier.padding(start = 52.dp, end = 16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Row(verticalAlignment = Alignment.Top) {
                Box(
                    modifier = Modifier
                        .padding(top = 7.dp)
                        .size(4.dp)
                        .background(CRTheme.textMedium(isDark).copy(alpha = 0.5f), CircleShape)
                )
                Spacer(modifier = Modifier.width(12.dp))
                Text(
                    text = "Mobile Hotspot (for travel)",
                    style = CRTypography.caption,
                    color = CRTheme.textMedium(isDark)
                )
            }
            Row(verticalAlignment = Alignment.Top) {
                Box(
                    modifier = Modifier
                        .padding(top = 7.dp)
                        .size(4.dp)
                        .background(CRTheme.textMedium(isDark).copy(alpha = 0.5f), CircleShape)
                )
                Spacer(modifier = Modifier.width(12.dp))
                Text(
                    text = "Same Wi-Fi Network (for home/office)",
                    style = CRTypography.caption,
                    color = CRTheme.textMedium(isDark)
                )
            }
        }
    }
}

@Composable
fun PeerListCard(
    isDark: Boolean,
    peer: PeerSnapshot,
    onTrust: () -> Unit,
    onReject: () -> Unit,
    onPair: () -> Unit,
    onConnect: () -> Unit,
    onDisconnect: () -> Unit,
    onRespond: (Boolean) -> Unit
) {
    val haptic = LocalHapticFeedback.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .crGlassCard(isDark = isDark, cornerRadius = 16.dp)
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            imageVector = Icons.Default.Computer,
            contentDescription = "Peer device",
            tint = if (peer.trusted) CRTheme.brandElectric else CRTheme.textMedium(isDark),
            modifier = Modifier.size(32.dp)
        )
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = peer.name,
                style = CRTypography.label,
                color = CRTheme.textHigh(isDark)
            )
            val stateText = when (peer.lifecycleState) {
                "discovered" -> "Nearby Discovered"
                "pending_approval" -> "Pending Approval"
                "paired" -> "Paired Offline"
                "connected" -> "Connected"
                "auto_connected" -> "Auto Connected"
                else -> if (peer.trusted) "Trusted Device" else "Pending Approval"
            }
            val stateColor = when (peer.lifecycleState) {
                "discovered" -> CRTheme.brandElectric
                "pending_approval" -> CRTheme.accentAmber
                "paired" -> CRTheme.textMedium(isDark)
                "connected", "auto_connected" -> CRTheme.accentGreen
                else -> if (peer.trusted) CRTheme.accentGreen else CRTheme.accentAmber
            }
            Text(
                text = stateText,
                style = CRTypography.caption,
                color = stateColor
            )
        }
        if (!peer.trusted) {
            if (peer.pairingRequested) {
                IconButton(onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onRespond(true)
                }) {
                    Icon(Icons.Default.Check, contentDescription = "Accept", tint = CRTheme.accentGreen)
                }
                IconButton(onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onRespond(false)
                }) {
                    Icon(Icons.Default.Close, contentDescription = "Decline", tint = CRTheme.accentRed)
                }
            } else {
                IconButton(onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onPair()
                }) {
                    Icon(Icons.Default.Link, contentDescription = "Pair", tint = CRTheme.brandElectric)
                }
            }
        } else {
            if (peer.isConnected) {
                IconButton(onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onDisconnect()
                }) {
                    Icon(Icons.Default.LinkOff, contentDescription = "Disconnect", tint = CRTheme.accentRed)
                }
            } else {
                IconButton(onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onConnect()
                }) {
                    Icon(Icons.Default.SettingsInputAntenna, contentDescription = "Connect", tint = CRTheme.brandElectric)
                }
            }
        }
    }
}

