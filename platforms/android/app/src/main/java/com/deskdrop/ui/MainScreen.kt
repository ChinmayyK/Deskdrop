package com.deskdrop.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Wifi
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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import com.deskdrop.ActivityEntry
import com.deskdrop.ActivityKind
import com.deskdrop.PeerSnapshot
import com.deskdrop.TransferProgress
import com.deskdrop.ui.theme.CRBackground
import com.deskdrop.ui.theme.CRTheme
import com.deskdrop.ui.theme.CRTypography
import com.deskdrop.ui.theme.crGlassCard
import com.deskdrop.ui.theme.crPressScale

val CRTheme.brandElectric get() = Color(0xFF0066FF)
val CRTheme.brandViolet get() = Color(0xFF8B5CF6)
val CRTheme.brandCyan get() = Color(0xFF06B6D4)
val CRTheme.brandPink get() = Color(0xFFEC4899)
val CRTheme.accentGreen get() = Color(0xFF10B981)
val CRTheme.accentRed get() = Color(0xFFEF4444)
val CRTheme.accentAmber get() = Color(0xFFF59E0B)

enum class AppTab { Home, Activity, Devices, Settings }

@Composable
fun MainScreen(
    isDark: Boolean,
    isServiceRunning: Boolean,
    isSyncEnabled: Boolean,
    peers: List<PeerSnapshot>,
    feed: List<ActivityEntry>,
    ambientStatus: String,
    activeTransfers: List<TransferProgress>,
    onStartSync: () -> Unit,
    onResumeSync: () -> Unit,
    onScanNow: () -> Unit,
    onActionPushClipboard: () -> Unit,
    onActionPairMagicLink: () -> Unit,
    onActionPauseSync: () -> Unit,
    onActionDisconnectAll: () -> Unit,
    onActionStopService: () -> Unit,
    onActionStreamCamera: () -> Unit,
    onActionPauseTransfer: (String) -> Unit,
    onActionResumeTransfer: (String) -> Unit,
    onActionCancelTransfer: (String) -> Unit,
    onActionSendFiles: () -> Unit,
    onApplyClipboard: (ActivityEntry) -> Unit,
    onTrustPeer: (PeerSnapshot) -> Unit,
    onRejectPeer: (PeerSnapshot) -> Unit,
    onSendPairingRequest: (PeerSnapshot) -> Unit,
    onRespondPairing: (PeerSnapshot, Boolean) -> Unit,
    onOpenSettings: () -> Unit
) {
    var currentTab by remember { mutableStateOf(AppTab.Home) }
    val hasConnectedDevices = peers.any { it.isConnected }

    CRBackground(isDark = isDark, hasConnectedDevices = hasConnectedDevices) {
        Box(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
            Column(modifier = Modifier.fillMaxSize()) {
                CompactStatusStrip(isDark = isDark, peers = peers, ambientStatus = ambientStatus)
                
                Box(modifier = Modifier.weight(1f)) {
                    AnimatedContent(
                        targetState = currentTab,
                        transitionSpec = {
                            fadeIn(animationSpec = tween(300)) togetherWith fadeOut(animationSpec = tween(300))
                        },
                        label = "tab_content"
                    ) { tab ->
                        when (tab) {
                            AppTab.Home -> HomeTab(
                                isDark = isDark,
                                peers = peers,
                                feed = feed,
                                activeTransfers = activeTransfers,
                                onActionPushClipboard = onActionPushClipboard,
                                onActionPairMagicLink = onActionPairMagicLink,
                                onActionSendFiles = onActionSendFiles,
                                onActionStreamCamera = onActionStreamCamera,
                                onApplyClipboard = onApplyClipboard,
                                onActionPauseTransfer = onActionPauseTransfer,
                                onActionResumeTransfer = onActionResumeTransfer,
                                onActionCancelTransfer = onActionCancelTransfer,
                                onTabSelected = { currentTab = it }
                            )
                            AppTab.Activity -> ActivityTab(
                                isDark = isDark,
                                feed = feed,
                                onApplyClipboard = onApplyClipboard
                            )
                            AppTab.Devices -> DevicesTab(
                                isDark = isDark,
                                peers = peers,
                                onTrustPeer = onTrustPeer,
                                onRejectPeer = onRejectPeer,
                                onSendPairingRequest = onSendPairingRequest,
                                onRespondPairing = onRespondPairing
                            )
                            AppTab.Settings -> SettingsTab(
                                isDark = isDark,
                                isSyncEnabled = isSyncEnabled,
                                isServiceRunning = isServiceRunning,
                                onStartSync = onStartSync,
                                onResumeSync = onResumeSync,
                                onScanNow = onScanNow,
                                onActionPauseSync = onActionPauseSync,
                                onActionDisconnectAll = onActionDisconnectAll,
                                onActionStopService = onActionStopService,
                                onOpenSettings = onOpenSettings
                            )
                        }
                    }
                }
            }
            
            Box(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 16.dp)
            ) {
                BottomDock(
                    currentTab = currentTab,
                    onTabSelected = { currentTab = it },
                    isDark = isDark
                )
            }
        }
    }
}

@Composable
fun CompactStatusStrip(isDark: Boolean, peers: List<PeerSnapshot>, ambientStatus: String) {
    val infiniteTransition = rememberInfiniteTransition(label = "pulse")
    val alpha by infiniteTransition.animateFloat(
        initialValue = 0.3f, targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(1500, easing = LinearEasing), RepeatMode.Reverse),
        label = "pulseAlpha"
    )
    val scale by infiniteTransition.animateFloat(
        initialValue = 0.8f, targetValue = 1.2f,
        animationSpec = infiniteRepeatable(tween(1200, easing = LinearEasing), RepeatMode.Reverse),
        label = "scalePulse"
    )
    
    val connectedPeersCount = peers.count { it.isConnected }
    val isSearching = ambientStatus.contains("Looking", ignoreCase = true)
    
    val statusText = if (connectedPeersCount > 0) {
        "$connectedPeersCount nearby device${if (connectedPeersCount > 1) "s" else ""} available"
    } else if (isSearching) {
        "Scanning nearby devices"
    } else {
        ambientStatus
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.Center
    ) {
        Box(contentAlignment = Alignment.Center) {
            Box(
                modifier = Modifier
                    .size(10.dp)
                    .scale(if (isSearching && connectedPeersCount == 0) scale else 1f)
                    .blur(4.dp)
                    .background(
                        if (connectedPeersCount > 0) CRTheme.statusGreen.copy(alpha = alpha * 0.6f)
                        else (if (isSearching) CRTheme.indigoSoft else CRTheme.statusAmber).copy(alpha = alpha * 0.4f),
                        CircleShape
                    )
            )
            Box(
                modifier = Modifier
                    .size(5.dp)
                    .background(
                        if (connectedPeersCount > 0) CRTheme.statusGreen
                        else (if (isSearching) CRTheme.indigoSoft else CRTheme.statusAmber),
                        CircleShape
                    )
            )
        }
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = statusText,
            style = CRTypography.caption,
            color = CRTheme.textMedium(isDark)
        )
    }
}

@Composable
fun HomeTab(
    isDark: Boolean,
    peers: List<PeerSnapshot>,
    feed: List<ActivityEntry>,
    activeTransfers: List<TransferProgress>,
    onActionPushClipboard: () -> Unit,
    onActionPairMagicLink: () -> Unit,
    onActionSendFiles: () -> Unit,
    onActionStreamCamera: () -> Unit,
    onApplyClipboard: (ActivityEntry) -> Unit,
    onActionPauseTransfer: (String) -> Unit,
    onActionResumeTransfer: (String) -> Unit,
    onActionCancelTransfer: (String) -> Unit,
    onTabSelected: (AppTab) -> Unit
) {
    Column(modifier = Modifier.fillMaxSize()) {
        val hasConnectedPeers = peers.any { it.isConnected || it.trusted }
        
        Spacer(modifier = Modifier.height(24.dp)) // Contextual gap from Status Strip
        
        if (activeTransfers.isNotEmpty()) {
            Text(
                text = "Active Transfers",
                style = CRTypography.h2,
                color = CRTheme.textHigh(isDark),
                modifier = Modifier.padding(horizontal = 24.dp)
            )
            Spacer(modifier = Modifier.height(12.dp))
            LazyRow(
                contentPadding = PaddingValues(horizontal = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                items(activeTransfers) { t ->
                    ActiveTransferCard(
                        isDark = isDark,
                        transfer = t,
                        onPause = { onActionPauseTransfer(t.id) },
                        onResume = { onActionResumeTransfer(t.id) },
                        onCancel = { onActionCancelTransfer(t.id) }
                    )
                }
            }
            Spacer(modifier = Modifier.height(32.dp))
        }

        if (peers.isEmpty()) {
            EmptyStateEcosystem(isDark = isDark, onPair = onActionPairMagicLink)
        } else {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 24.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "Ecosystem",
                    style = CRTypography.h2,
                    color = CRTheme.textHigh(isDark)
                )
                // Inline Add Action
                Row(
                    modifier = Modifier
                        .crPressScale(0.95f)
                        .clip(RoundedCornerShape(12.dp))
                        .clickable { onActionPairMagicLink() }
                        .background(CRTheme.indigoSoft.copy(alpha = 0.15f))
                        .padding(horizontal = 12.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(imageVector = Icons.Default.Add, contentDescription = null, tint = CRTheme.indigoSoft, modifier = Modifier.size(14.dp))
                    Spacer(modifier = Modifier.width(4.dp))
                    Text("Add", style = CRTypography.caption, color = CRTheme.indigoSoft)
                }
            }
            
            Spacer(modifier = Modifier.height(12.dp)) // Related gap
            
            LazyRow(
                contentPadding = PaddingValues(horizontal = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                items(peers) { peer ->
                    DeviceCard(isDark = isDark, peer = peer)
                }
            }
        }
        
        Spacer(modifier = Modifier.height(32.dp)) // Contextual gap
        
        Text(
            text = "Actions",
            style = CRTypography.h2,
            color = CRTheme.textHigh(isDark),
            modifier = Modifier.padding(horizontal = 24.dp)
        )
        
        Spacer(modifier = Modifier.height(12.dp)) // Related gap
        
        QuickActionsGrid(
            isDark = isDark,
            enabled = hasConnectedPeers,
            onActionPushClipboard = onActionPushClipboard,
            onActionSendFiles = onActionSendFiles,
            onActionStreamCamera = onActionStreamCamera,
            onActionLinks = {}
        )
        
        Spacer(modifier = Modifier.height(32.dp)) // Contextual gap
        
        if (feed.isNotEmpty()) {
            RecentActivityPill(
                isDark = isDark,
                entry = feed.first(),
                onClick = { onTabSelected(AppTab.Activity) }
            )
        }
        
        Spacer(modifier = Modifier.height(120.dp)) // Space for dock
    }
}

@Composable
fun EmptyStateEcosystem(isDark: Boolean, onPair: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .crGlassCard(isDark = isDark, cornerRadius = 24.dp, dashed = true, onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                onPair()
            })
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Icon(
            imageVector = Icons.Default.Devices,
            contentDescription = null,
            tint = CRTheme.brandElectric,
            modifier = Modifier.size(48.dp)
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = "No devices connected",
            style = CRTypography.label,
            color = CRTheme.textHigh(isDark)
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = "Tap to pair a new device to your ecosystem.",
            style = CRTypography.caption,
            color = CRTheme.textMedium(isDark)
        )
    }
}

@Composable
fun QuickActionsGrid(
    isDark: Boolean,
    enabled: Boolean,
    onActionPushClipboard: () -> Unit,
    onActionSendFiles: () -> Unit,
    onActionStreamCamera: () -> Unit,
    onActionLinks: () -> Unit
) {
    Column(
        modifier = Modifier.padding(horizontal = 24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Large Primary Action
        QuickActionCardPrimary(
            isDark = isDark,
            enabled = enabled,
            icon = Icons.Default.ContentCopy,
            title = "Clipboard Sync",
            subtitle = "Send copied text & images",
            color = CRTheme.brandElectric,
            onClick = onActionPushClipboard
        )
        
        // Smaller Secondary Actions
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            QuickActionCard(
                modifier = Modifier.weight(1f),
                isDark = isDark,
                enabled = enabled,
                icon = Icons.Default.Folder,
                label = "Files",
                color = CRTheme.brandViolet,
                onClick = onActionSendFiles
            )
            QuickActionCard(
                modifier = Modifier.weight(1f),
                isDark = isDark,
                enabled = enabled,
                icon = Icons.Default.Videocam,
                label = "Camera",
                color = CRTheme.brandCyan,
                onClick = onActionStreamCamera
            )
            QuickActionCard(
                modifier = Modifier.weight(1f),
                isDark = isDark,
                enabled = enabled,
                icon = Icons.Default.Link,
                label = "Links",
                color = CRTheme.brandPink,
                onClick = onActionLinks
            )
        }
    }
}

@Composable
fun ActiveTransferCard(
    isDark: Boolean,
    transfer: TransferProgress,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onCancel: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    Column(
        modifier = Modifier
            .width(280.dp)
            .crGlassCard(isDark = isDark, cornerRadius = 24.dp)
            .padding(20.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .size(40.dp)
                    .background(CRTheme.brandCyan.copy(alpha = 0.15f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = Icons.Default.Folder,
                    contentDescription = null,
                    tint = CRTheme.brandCyan,
                    modifier = Modifier.size(20.dp)
                )
            }
            Spacer(modifier = Modifier.width(16.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = transfer.fileName,
                    style = CRTypography.label,
                    color = CRTheme.textHigh(isDark),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                Text(
                    text = if (transfer.isPaused) "Paused" else "${transfer.percent}% • " + 
                           if (transfer.speedBps > 0) "${transfer.speedBps / 1024 / 1024} MB/s" else "Calculating...",
                    style = CRTypography.caption,
                    color = CRTheme.textMedium(isDark),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
        
        Spacer(modifier = Modifier.height(16.dp))
        
        // Progress Bar
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(6.dp)
                .background(CRTheme.textMedium(isDark).copy(alpha = 0.2f), RoundedCornerShape(3.dp))
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth(transfer.percent / 100f)
                    .height(6.dp)
                    .background(if (transfer.isPaused) CRTheme.accentAmber else CRTheme.brandCyan, RoundedCornerShape(3.dp))
            )
        }
        
        Spacer(modifier = Modifier.height(16.dp))
        
        // Action Buttons
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.End,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(36.dp)
                    .crGlassCard(isDark = isDark, cornerRadius = 18.dp, onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                        if (transfer.isPaused) onResume() else onPause()
                    }),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = if (transfer.isPaused) Icons.Default.PlayArrow else Icons.Default.Pause,
                    contentDescription = if (transfer.isPaused) "Resume" else "Pause",
                    tint = CRTheme.textHigh(isDark),
                    modifier = Modifier.size(18.dp)
                )
            }
            Spacer(modifier = Modifier.width(12.dp))
            Box(
                modifier = Modifier
                    .size(36.dp)
                    .crGlassCard(isDark = isDark, cornerRadius = 18.dp, onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                        onCancel()
                    }),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = Icons.Default.Close,
                    contentDescription = "Cancel",
                    tint = CRTheme.accentRed,
                    modifier = Modifier.size(18.dp)
                )
            }
        }
    }
}

@Composable
fun QuickActionCardPrimary(
    isDark: Boolean,
    enabled: Boolean,
    icon: ImageVector,
    title: String,
    subtitle: String,
    color: Color,
    onClick: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    val displayColor = if (enabled) color else CRTheme.textMedium(isDark)
    
    val infiniteTransition = rememberInfiniteTransition(label = "pulse")
    val pulseScale by infiniteTransition.animateFloat(
        initialValue = 1f,
        targetValue = 1.3f,
        animationSpec = infiniteRepeatable(
            animation = tween(1500, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "pulseScale"
    )
    val pulseAlpha by infiniteTransition.animateFloat(
        initialValue = 0.5f,
        targetValue = 0f,
        animationSpec = infiniteRepeatable(
            animation = tween(1500, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "pulseAlpha"
    )
    
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .crPressScale(targetScale = 0.98f)
            .crGlassCard(
                isDark = isDark,
                cornerRadius = 24.dp,
                onClick = if (enabled) {
                    {
                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                        onClick()
                    }
                } else null
            )
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(contentAlignment = Alignment.Center) {
            if (enabled) {
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .scale(pulseScale)
                        .background(displayColor.copy(alpha = pulseAlpha), CircleShape)
                )
            }
            Box(
                modifier = Modifier
                    .size(44.dp)
                    .background(displayColor.copy(alpha = if (enabled) 0.15f else 0.05f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(imageVector = icon, contentDescription = title, tint = displayColor, modifier = Modifier.size(22.dp))
            }
        }
        Spacer(modifier = Modifier.width(16.dp))
        Column {
            Text(text = title, style = CRTypography.label, color = if (enabled) CRTheme.textHigh(isDark) else CRTheme.textMedium(isDark))
            Text(text = if (enabled) "Last synced just now" else subtitle, style = CRTypography.caption, color = CRTheme.textMedium(isDark))
        }
    }
}

@Composable
fun QuickActionCard(
    modifier: Modifier = Modifier,
    isDark: Boolean,
    enabled: Boolean = true,
    icon: ImageVector,
    label: String,
    color: Color,
    onClick: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    val displayColor = if (enabled) color else CRTheme.textMedium(isDark)
    
    Column(
        modifier = modifier
            .crPressScale(targetScale = 0.95f)
            .crGlassCard(
                isDark = isDark,
                cornerRadius = 16.dp,
                onClick = if (enabled) {
                    {
                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                        onClick()
                    }
                } else null
            )
            .padding(vertical = 12.dp, horizontal = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Box(
            modifier = Modifier
                .size(32.dp) // Reduced from 38.dp
                .background(displayColor.copy(alpha = if (enabled) 0.15f else 0.05f), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Icon(imageVector = icon, contentDescription = label, tint = displayColor, modifier = Modifier.size(16.dp)) // Reduced from 20.dp
        }
        Spacer(modifier = Modifier.height(8.dp))
        Text(text = label, style = CRTypography.caption, color = if (enabled) CRTheme.textHigh(isDark) else CRTheme.textMedium(isDark))
    }
}

@Composable
fun RecentActivityPill(
    isDark: Boolean,
    entry: ActivityEntry,
    onClick: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .crPressScale(targetScale = 0.98f)
            .crGlassCard(
                isDark = isDark,
                cornerRadius = 24.dp,
                onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onClick()
                }
            )
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .size(36.dp)
                .background(CRTheme.brandViolet.copy(alpha = 0.15f), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                imageVector = Icons.Default.Bolt,
                contentDescription = null,
                tint = CRTheme.brandViolet,
                modifier = Modifier.size(18.dp)
            )
        }
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = "Recently Connected: ${entry.deviceName}",
                style = CRTypography.label,
                color = CRTheme.textHigh(isDark),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            Text(
                text = "Just now • Tap to view activity",
                style = CRTypography.caption,
                color = CRTheme.textMedium(isDark)
            )
        }
        Icon(
            imageVector = Icons.Default.KeyboardArrowRight,
            contentDescription = "View",
            tint = CRTheme.textMedium(isDark)
        )
    }
}

@Composable
fun DeviceCard(isDark: Boolean, peer: PeerSnapshot) {
    val haptic = LocalHapticFeedback.current
    val isPhone = peer.name.contains("phone", ignoreCase = true) || peer.name.contains("pixel", ignoreCase = true)
    
    Column(
        modifier = Modifier
            .width(150.dp)
            .height(100.dp)
            .crPressScale(targetScale = 0.95f)
            .crGlassCard(isDark = isDark, cornerRadius = 24.dp, onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
            })
            .padding(16.dp),
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
                    .size(32.dp)
                    .background(if (peer.trusted) CRTheme.indigoSoft.copy(alpha = 0.15f) else CRTheme.textMedium(isDark).copy(alpha = 0.1f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = if (isPhone) Icons.Default.Smartphone else Icons.Default.LaptopMac,
                    contentDescription = null,
                    tint = if (peer.trusted) CRTheme.indigoSoft else CRTheme.textMedium(isDark),
                    modifier = Modifier.size(16.dp)
                )
            }
            if (peer.isConnected) {
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .blur(2.dp)
                        .background(CRTheme.statusGreen, CircleShape)
                ) {
                    Box(modifier = Modifier.size(8.dp).background(CRTheme.statusGreen, CircleShape))
                }
            }
        }
        
        Column {
            Text(
                text = peer.name,
                style = CRTypography.label,
                color = CRTheme.textHigh(isDark),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            if (peer.trusted) {
                Text(
                    text = if (peer.isConnected) "Nearby" else "Offline",
                    style = CRTypography.caption,
                    color = CRTheme.textMedium(isDark)
                )
            } else {
                Text(
                    text = "Pending",
                    style = CRTypography.caption,
                    color = CRTheme.statusAmber
                )
            }
        }
    }
}

@Composable
fun AddDeviceCard(isDark: Boolean, onClick: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    Column(
        modifier = Modifier
            .width(150.dp)
            .height(100.dp)
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
fun ActivityTab(
    isDark: Boolean,
    feed: List<ActivityEntry>,
    onApplyClipboard: (ActivityEntry) -> Unit
) {
    Column(modifier = Modifier.fillMaxSize()) {
        Text(
            text = "Activity Feed",
            style = CRTypography.h2,
            color = CRTheme.textHigh(isDark),
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
        )
        
        LazyColumn(
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            items(feed) { entry ->
                ActivityFeedCardNew(
                    isDark = isDark,
                    entry = entry,
                    onClick = {
                        if (entry.kind == ActivityKind.CLIPBOARD_TEXT || entry.kind == ActivityKind.CLIPBOARD_IMAGE) {
                            onApplyClipboard(entry)
                        }
                    }
                )
            }
        }
    }
}

@Composable
fun ActivityFeedCardNew(
    isDark: Boolean,
    entry: ActivityEntry,
    onClick: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    val tagColor = when (entry.kind) {
        ActivityKind.CLIPBOARD_TEXT -> CRTheme.brandElectric
        ActivityKind.CLIPBOARD_IMAGE -> CRTheme.brandPink
        else -> CRTheme.textMedium(isDark)
    }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .crGlassCard(
                isDark = isDark,
                cornerRadius = 16.dp,
                onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onClick()
                }
            )
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .width(4.dp)
                .height(40.dp)
                .background(tagColor, RoundedCornerShape(2.dp))
        )
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = entry.deviceName,
                    style = CRTypography.label,
                    color = CRTheme.textHigh(isDark)
                )
                val timeString = android.text.format.DateFormat.format("hh:mm a", entry.timestamp).toString()
                Text(
                    text = timeString,
                    style = CRTypography.caption,
                    color = CRTheme.textMedium(isDark)
                )
            }
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = entry.preview.replace("\n", " "),
                style = CRTypography.bodyMedium,
                color = CRTheme.textMedium(isDark),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                fontFamily = FontFamily.Monospace
            )
        }
    }
}

@Composable
fun DevicesTab(
    isDark: Boolean,
    peers: List<PeerSnapshot>,
    onTrustPeer: (PeerSnapshot) -> Unit,
    onRejectPeer: (PeerSnapshot) -> Unit,
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
            items(peers) { peer ->
                PeerListCard(
                    isDark = isDark,
                    peer = peer,
                    onTrust = { onTrustPeer(peer) },
                    onReject = { onRejectPeer(peer) },
                    onPair = { onSendPairingRequest(peer) },
                    onRespond = { accepted -> onRespondPairing(peer, accepted) }
                )
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
                    contentDescription = null,
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
            contentDescription = null,
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
            Text(
                text = if (peer.trusted) "Trusted Device" else "Pending Approval",
                style = CRTypography.caption,
                color = if (peer.trusted) CRTheme.accentGreen else CRTheme.accentAmber
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
                IconButton(onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onTrust()
                }) {
                    Icon(Icons.Default.Check, contentDescription = "Trust", tint = CRTheme.accentGreen)
                }
                IconButton(onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onReject()
                }) {
                    Icon(Icons.Default.Close, contentDescription = "Reject", tint = CRTheme.accentRed)
                }
            }
        }
    }
}

@Composable
fun SettingsTab(
    isDark: Boolean,
    isSyncEnabled: Boolean,
    isServiceRunning: Boolean,
    onStartSync: () -> Unit,
    onResumeSync: () -> Unit,
    onScanNow: () -> Unit,
    onActionPauseSync: () -> Unit,
    onActionDisconnectAll: () -> Unit,
    onActionStopService: () -> Unit,
    onOpenSettings: () -> Unit
) {
    Column(modifier = Modifier.fillMaxSize()) {
        Text(
            text = "Settings",
            style = CRTypography.h2,
            color = CRTheme.textHigh(isDark),
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp)
        )
        LazyColumn(
            contentPadding = PaddingValues(start = 24.dp, end = 24.dp, bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            item {
                if (isSyncEnabled) {
                    SettingsActionTile(isDark = isDark, icon = Icons.Default.Pause, label = "Pause Sync", color = CRTheme.accentAmber, onClick = onActionPauseSync)
                } else {
                    SettingsActionTile(isDark = isDark, icon = Icons.Default.PlayArrow, label = "Resume Sync", color = CRTheme.accentGreen, onClick = onResumeSync)
                }
            }
            item {
                if (!isServiceRunning) {
                    SettingsActionTile(isDark = isDark, icon = Icons.Default.PlayCircle, label = "Start Service", color = CRTheme.accentGreen, onClick = onStartSync)
                }
            }
            item { SettingsActionTile(isDark = isDark, icon = Icons.Default.Search, label = "Scan Now", color = CRTheme.brandCyan, onClick = onScanNow) }
            item { SettingsActionTile(isDark = isDark, icon = Icons.Default.LinkOff, label = "Disconnect All", color = CRTheme.brandPink, onClick = onActionDisconnectAll) }
            item { SettingsActionTile(isDark = isDark, icon = Icons.Default.Stop, label = "Stop Service", color = CRTheme.accentRed, onClick = onActionStopService) }
            item { SettingsActionTile(isDark = isDark, icon = Icons.Default.Settings, label = "Advanced Settings", color = CRTheme.textMedium(isDark), onClick = onOpenSettings) }
        }
    }
}

@Composable
fun SettingsActionTile(
    isDark: Boolean,
    icon: ImageVector,
    label: String,
    color: Color,
    onClick: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .crGlassCard(
                isDark = isDark,
                cornerRadius = 16.dp,
                onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    onClick()
                }
            )
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(
            modifier = Modifier
                .size(36.dp)
                .background(color.copy(alpha = 0.15f), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Icon(imageVector = icon, contentDescription = null, tint = color, modifier = Modifier.size(20.dp))
        }
        Spacer(modifier = Modifier.width(16.dp))
        Text(
            text = label,
            style = CRTypography.label,
            color = CRTheme.textHigh(isDark)
        )
    }
}

@Composable
fun BottomDock(
    currentTab: AppTab,
    onTabSelected: (AppTab) -> Unit,
    isDark: Boolean
) {
    val haptic = LocalHapticFeedback.current
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 48.dp) // Slightly wider to accommodate pill
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .crGlassCard(isDark = isDark, cornerRadius = 36.dp, elevated = true)
                .padding(horizontal = 8.dp, vertical = 6.dp), // Thinner padding for smaller navbar
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically
        ) {
            AppTab.values().forEach { tab ->
                val isSelected = currentTab == tab
                
                Box(
                    modifier = Modifier
                        .clip(CircleShape)
                        .clickable(
                            interactionSource = remember { MutableInteractionSource() },
                            indication = null,
                            onClick = {
                                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                onTabSelected(tab)
                            }
                        )
                        .background(if (isSelected) CRTheme.indigoSoft.copy(alpha = 0.15f) else Color.Transparent)
                        .padding(horizontal = if (isSelected) 20.dp else 16.dp, vertical = 10.dp),
                    contentAlignment = Alignment.Center
                ) {
                    val iconColor = if (isSelected) CRTheme.indigoSoft else CRTheme.textHigh(isDark).copy(alpha = 0.4f)
                    
                    Icon(
                        imageVector = when (tab) {
                            AppTab.Home -> Icons.Default.Home
                            AppTab.Activity -> Icons.Default.List
                            AppTab.Devices -> Icons.Default.Devices
                            AppTab.Settings -> Icons.Default.Settings
                        },
                        contentDescription = tab.name,
                        tint = iconColor,
                        modifier = Modifier.size(22.dp) // Slightly smaller icon to make the bar thinner
                    )
                }
            }
        }
    }
}
