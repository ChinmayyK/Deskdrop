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

@Composable
fun HomeTab(
    isDark: Boolean,
    peers: List<PeerSnapshot>,
    feed: List<ActivityEntry>,
    activeTransfers: List<TransferProgress>,
    onActionPushClipboard: () -> Unit,
    onActionSendQuickContext: () -> Unit,
    quickContextText: String?,
    onActionPairMagicLink: () -> Unit,
    onManualIp: () -> Unit,
    onActionSendFiles: (String?) -> Unit,
    onDropFiles: (String, List<android.net.Uri>) -> Unit,
    onActionStreamCamera: () -> Unit,
    onApplyClipboard: (ActivityEntry) -> Unit,
    onActionPauseTransfer: (String) -> Unit,
    onActionResumeTransfer: (String) -> Unit,
    onActionCancelTransfer: (String) -> Unit,
    onForgetPeer: (PeerSnapshot) -> Unit,
    onRejectPeer: (PeerSnapshot) -> Unit,
    onTrustPeer: (PeerSnapshot) -> Unit,
    onConnectPeer: (PeerSnapshot) -> Unit,
    onSendPairingRequest: (PeerSnapshot) -> Unit,
    onDeleteActivity: (ActivityEntry) -> Unit,
    onResendActivity: (ActivityEntry) -> Unit,
    onReplayOnboarding: () -> Unit,
    onTabSelected: (AppTab) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        val hasConnectedPeers = peers.any { it.isConnected || it.trusted }
        
        Spacer(modifier = Modifier.height(24.dp)) // Contextual gap from Status Strip

        if (peers.isEmpty()) {
            EmptyStateEcosystem(isDark = isDark, onReplayOnboarding = onReplayOnboarding)
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

                Row(verticalAlignment = Alignment.CenterVertically) {
                    var showQrDialog by remember { mutableStateOf(false) }

                    // Show QR Code Action
                    Box(
                        modifier = Modifier
                            .crPressScale(0.95f)
                            .clip(CircleShape)
                            .clickable { showQrDialog = true }
                            .background(CRTheme.textHigh(isDark).copy(alpha = 0.05f))
                            .padding(8.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(imageVector = Icons.Default.QrCode, contentDescription = "Show QR", tint = CRTheme.textHigh(isDark), modifier = Modifier.size(18.dp))
                    }

                    if (showQrDialog) {
                        androidx.compose.material3.AlertDialog(
                            onDismissRequest = { showQrDialog = false },
                            title = { Text("Scan to Pair") },
                            text = {
                                Box(modifier = Modifier.fillMaxWidth().aspectRatio(1f), contentAlignment = Alignment.Center) {
                                    val ip = getLocalIpAddress()
                                    val uri = "deskdrop://$ip:${DeskdropService.DEFAULT_DESKDROP_PORT}"
                                    val bitmap = remember(uri) {
                                        try {
                                            val writer = com.google.zxing.qrcode.QRCodeWriter()
                                            val bitMatrix = writer.encode(uri, com.google.zxing.BarcodeFormat.QR_CODE, 512, 512)
                                            val width = bitMatrix.width
                                            val height = bitMatrix.height
                                            val bmp = android.graphics.Bitmap.createBitmap(width, height, android.graphics.Bitmap.Config.RGB_565)
                                            for (x in 0 until width) {
                                                for (y in 0 until height) {
                                                    bmp.setPixel(x, y, if (bitMatrix.get(x, y)) android.graphics.Color.BLACK else android.graphics.Color.WHITE)
                                                }
                                            }
                                            bmp
                                        } catch (e: Exception) { null }
                                    }
                                    if (bitmap != null) {
                                        androidx.compose.foundation.Image(
                                            bitmap = bitmap.asImageBitmap(),
                                            contentDescription = "QR Code",
                                            modifier = Modifier.fillMaxSize()
                                        )
                                    } else {
                                        Text("Failed to generate QR Code")
                                    }
                                }
                            },
                            confirmButton = {
                                TextButton(onClick = { showQrDialog = false }) { Text("Close") }
                            }
                        )
                    }
                    
                    Spacer(modifier = Modifier.width(8.dp))

                    // Inline Add Action (Scan QR)
                    Box(
                        modifier = Modifier
                            .crPressScale(0.95f)
                            .clip(CircleShape)
                            .clickable { onActionPairMagicLink() }
                            .background(CRTheme.textHigh(isDark).copy(alpha = 0.05f))
                            .padding(8.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(imageVector = Icons.Default.QrCodeScanner, contentDescription = "Scan QR", tint = CRTheme.textHigh(isDark), modifier = Modifier.size(18.dp))
                    }

                    Spacer(modifier = Modifier.width(8.dp))

                    // Manual IP Action
                    Box(
                        modifier = Modifier
                            .crPressScale(0.95f)
                            .clip(CircleShape)
                            .clickable { onManualIp() }
                            .background(CRTheme.textHigh(isDark).copy(alpha = 0.05f))
                            .padding(8.dp),
                        contentAlignment = Alignment.Center
                    ) {
                        Icon(imageVector = Icons.Default.Language, contentDescription = "Manual IP", tint = CRTheme.textHigh(isDark), modifier = Modifier.size(18.dp))
                    }
                }
            }

            Spacer(modifier = Modifier.height(12.dp)) // Related gap

            if (!quickContextText.isNullOrBlank()) {
                androidx.compose.material3.Card(
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp),
                    shape = RoundedCornerShape(16.dp),
                    colors = androidx.compose.material3.CardDefaults.cardColors(containerColor = CRTheme.surfaceElevated(isDark))
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text("JUST COPIED", style = CRTypography.caption, color = CRTheme.blueSoft)
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(quickContextText, maxLines = 2, overflow = TextOverflow.Ellipsis, color = CRTheme.textHigh(isDark))
                        Spacer(modifier = Modifier.height(12.dp))
                        androidx.compose.material3.Button(onClick = onActionSendQuickContext, modifier = Modifier.fillMaxWidth()) {
                            Text("Send to Ecosystem")
                        }
                    }
                }
            }

            LazyRow(
                contentPadding = PaddingValues(horizontal = 24.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                items(peers) { peer ->
                    DeviceCard(
                        isDark = isDark, 
                        peer = peer,
                        onSendFiles = { onActionSendFiles(peer.id) },
                        onDropFiles = { uris -> onDropFiles(peer.id, uris) },
                        onPair = { onSendPairingRequest(peer) },
                        onConnect = { onConnectPeer(peer) },
                        onForget = { onForgetPeer(peer) },
                        onReject = { onRejectPeer(peer) },
                        modifier = if (peers.size == 1) Modifier.fillParentMaxWidth(0.95f) else Modifier.width(170.dp)
                    )
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
        
        val maxSyncSecs = peers.mapNotNull { it.lastSyncSecs }.maxOrNull()
        QuickActionsGrid(
            isDark = isDark,
            enabled = hasConnectedPeers,
            lastSyncSecs = maxSyncSecs,
            onActionPushClipboard = onActionPushClipboard,
            onActionSendFiles = { onActionSendFiles(null) },
            onActionStreamCamera = onActionStreamCamera,
            onActionLinks = {}
        )
        
        Spacer(modifier = Modifier.height(32.dp)) // Contextual gap
        
        ActivityTimelineSection(
            isDark = isDark,
            feed = feed,
            onApply = onApplyClipboard,
            onDelete = onDeleteActivity,
            onResend = onResendActivity,
            onViewAll = { onTabSelected(AppTab.Activity) }
        )
        
        Spacer(modifier = Modifier.height(160.dp)) // Space for dock
    }
}

@Composable
fun EmptyStateEcosystem(isDark: Boolean, onReplayOnboarding: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp)
            .crGlassCard(isDark = isDark, cornerRadius = 24.dp, dashed = true, onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                onReplayOnboarding()
            })
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Icon(
            imageVector = Icons.Default.Devices,
            contentDescription = "Devices",
            tint = CRTheme.brandElectric,
            modifier = Modifier.size(48.dp)
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = "Finish Onboarding",
            style = CRTypography.label,
            color = CRTheme.textHigh(isDark)
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = "Tap to pair your first device and complete setup.",
            style = CRTypography.caption,
            color = CRTheme.textMedium(isDark)
        )
    }
}

@Composable
fun QuickActionsGrid(
    isDark: Boolean,
    enabled: Boolean,
    lastSyncSecs: Long?,
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
            subtitle = if (lastSyncSecs != null) {
                val diff = (System.currentTimeMillis() / 1000) - lastSyncSecs
                if (diff < 60) "Last synced just now"
                else if (diff < 3600) "Last synced ${diff / 60}m ago"
                else "Last synced ${diff / 3600}h ago"
            } else "Send copied text & images",
            color = CRTheme.blueSoft,
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
                color = CRTheme.blueSoft,
                onClick = onActionSendFiles
            )
            QuickActionCard(
                modifier = Modifier.weight(1f),
                isDark = isDark,
                enabled = enabled,
                icon = Icons.Default.Videocam,
                label = "Camera",
                color = CRTheme.blueSoft,
                onClick = onActionStreamCamera
            )
            QuickActionCard(
                modifier = Modifier.weight(1f),
                isDark = isDark,
                enabled = enabled,
                icon = Icons.Default.Link,
                label = "Links",
                color = CRTheme.blueSoft,
                onClick = onActionLinks
            )
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
    
    // Removed pulse animation based on user feedback
    
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
            .semantics(mergeDescendants = true) {}
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(contentAlignment = Alignment.Center) {
            Box(
                modifier = Modifier
                    .size(56.dp)
                    .background(displayColor.copy(alpha = if (enabled) 0.15f else 0.05f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(imageVector = icon, contentDescription = title, tint = displayColor, modifier = Modifier.size(28.dp))
            }
        }
        Spacer(modifier = Modifier.width(16.dp))
        Column {
            Text(text = title, style = CRTypography.label, color = if (enabled) CRTheme.textHigh(isDark) else CRTheme.textMedium(isDark))
            Text(text = subtitle, style = CRTypography.caption, color = CRTheme.textMedium(isDark))
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
            .padding(vertical = 16.dp, horizontal = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Box(
            modifier = Modifier
                .size(48.dp) // Scaled up for better touch target
                .background(displayColor.copy(alpha = if (enabled) 0.15f else 0.05f), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Icon(imageVector = icon, contentDescription = label, tint = displayColor, modifier = Modifier.size(24.dp))
        }
        Spacer(modifier = Modifier.height(10.dp))
        Text(text = label, style = CRTypography.caption, color = if (enabled) CRTheme.textHigh(isDark) else CRTheme.textMedium(isDark))
    }
}

@Composable
fun ActivityTimelineSection(
    isDark: Boolean,
    feed: List<ActivityEntry>,
    onApply: (ActivityEntry) -> Unit,
    onDelete: (ActivityEntry) -> Unit,
    onResend: (ActivityEntry) -> Unit,
    onViewAll: () -> Unit
) {
    val filteredFeed = remember(feed) {
        val result = mutableListOf<ActivityEntry>()
        val seenDeviceEvents = mutableSetOf<String>()
        for (entry in feed) {
            if (entry.kind == ActivityKind.PEER_CONNECTED || entry.kind == ActivityKind.PEER_DISCONNECTED) {
                if (!seenDeviceEvents.contains(entry.deviceName)) {
                    result.add(entry)
                    seenDeviceEvents.add(entry.deviceName)
                }
            } else {
                result.add(entry)
            }
        }
        result
    }

    Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = "Activity",
                style = CRTypography.label, // Medium/Semibold
                color = CRTheme.textHigh(isDark)
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                val context = androidx.compose.ui.platform.LocalContext.current
                Row(
                    modifier = Modifier
                        .crPressScale(0.95f)
                        .clip(RoundedCornerShape(12.dp))
                        .clickable { 
                            context.startActivity(android.content.Intent(android.app.DownloadManager.ACTION_VIEW_DOWNLOADS).apply {
                                flags = android.content.Intent.FLAG_ACTIVITY_NEW_TASK
                            }) 
                        }
                        .padding(horizontal = 12.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Downloads", style = CRTypography.caption, color = CRTheme.textHigh(isDark))
                }
                
                if (filteredFeed.size > 4) {
                    Row(
                        modifier = Modifier
                            .crPressScale(0.95f)
                            .clip(RoundedCornerShape(12.dp))
                            .clickable { onViewAll() }
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("View All", style = CRTypography.caption, color = CRTheme.blueSoft)
                    }
                }
            }
        }
        Spacer(modifier = Modifier.height(16.dp))
        
        if (filteredFeed.isEmpty()) {
            Text(
                "Your clipboard, files, and links will appear here.",
                style = CRTypography.caption,
                color = CRTheme.textMedium(isDark)
            )
        } else {
            Column {
                filteredFeed.take(4).forEach { entry ->
                    androidx.compose.animation.AnimatedVisibility(
                        visible = true,
                        enter = androidx.compose.animation.slideInVertically(
                            animationSpec = androidx.compose.animation.core.spring(
                                dampingRatio = androidx.compose.animation.core.Spring.DampingRatioMediumBouncy,
                                stiffness = androidx.compose.animation.core.Spring.StiffnessLow
                            ),
                            initialOffsetY = { fullHeight -> fullHeight / 2 }
                        ) + androidx.compose.animation.fadeIn()
                    ) {
                        TimelineActivityRow(
                            isDark = isDark,
                            entry = entry,
                            onApply = onApply,
                            onDelete = onDelete,
                            onResend = onResend
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun ImagePreviewDialog(
    filePath: String,
    onDismiss: () -> Unit
) {
    androidx.compose.ui.window.Dialog(
        onDismissRequest = onDismiss,
        properties = androidx.compose.ui.window.DialogProperties(
            usePlatformDefaultWidth = false,
            dismissOnBackPress = true,
            dismissOnClickOutside = true
        )
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Black.copy(alpha = 0.9f))
        ) {
            coil.compose.AsyncImage(
                model = "file://$filePath",
                contentDescription = "Image Preview",
                modifier = Modifier
                    .fillMaxSize()
                    .pointerInput(Unit) {
                        detectTapGestures(onTap = { onDismiss() })
                    },
                contentScale = androidx.compose.ui.layout.ContentScale.Fit
            )
            
            // Close Button
            Box(
                modifier = Modifier
                    .align(Alignment.TopEnd)
                    .padding(24.dp)
                    .statusBarsPadding()
                    .size(44.dp)
                    .crGlassCard(isDark = true, cornerRadius = 22.dp, onClick = onDismiss)
                    .background(Color.White.copy(alpha = 0.1f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = Icons.Default.Close,
                    contentDescription = "Close",
                    tint = Color.White,
                    modifier = Modifier.size(24.dp)
                )
            }
        }
    }
}

@Composable
fun TimelineActivityRow(
    isDark: Boolean,
    entry: ActivityEntry,
    onApply: (ActivityEntry) -> Unit,
    onDelete: (ActivityEntry) -> Unit,
    onResend: (ActivityEntry) -> Unit
) {
    val haptic = LocalHapticFeedback.current
    var showMenu by remember { mutableStateOf(false) }
    
    val isLink = entry.preview.startsWith("http")
    
    val title = when (entry.kind) {
        ActivityKind.FILE_SENT -> "Sent to ${entry.deviceName}"
        ActivityKind.FILE_RECEIVED, ActivityKind.FILE_TRANSFER_COMPLETE -> "Received from ${entry.deviceName}"
        ActivityKind.CLIPBOARD_TEXT -> if (isLink) "Link opened" else "Clipboard synced"
        ActivityKind.CLIPBOARD_IMAGE -> "Clipboard image synced"
        ActivityKind.PEER_CONNECTED -> "${entry.deviceName} became available"
        ActivityKind.PEER_DISCONNECTED -> "${entry.deviceName} went offline"
        else -> entry.preview.take(20)
    }
    
    val subtitle = when {
        entry.kind == ActivityKind.CLIPBOARD_TEXT && !isLink ->
            "Copied text (${entry.preview.length} chars) • Protected"
        entry.kind == ActivityKind.CLIPBOARD_TEXT && isLink ->
            "Link • ${entry.preview.take(45)}"
        entry.preview.isNotEmpty() && entry.kind != ActivityKind.WARNING && entry.kind != ActivityKind.PEER_CONNECTED && entry.kind != ActivityKind.PEER_DISCONNECTED ->
            entry.preview
        else -> "Just now"
    }
    
    val icon = when(entry.kind) {
        ActivityKind.PEER_CONNECTED -> Icons.Default.Wifi
        ActivityKind.PEER_DISCONNECTED -> Icons.Default.Close
        ActivityKind.FILE_RECEIVED, ActivityKind.FILE_SENT, ActivityKind.FILE_TRANSFER_COMPLETE -> Icons.Default.Description
        ActivityKind.CLIPBOARD_TEXT -> if (isLink) Icons.Default.Link else Icons.Default.ContentCopy
        ActivityKind.CLIPBOARD_IMAGE -> Icons.Default.Image
        else -> Icons.Default.Sync
    }
    
    val dotColor = when(entry.kind) {
        ActivityKind.PEER_CONNECTED -> CRTheme.accentGreen
        ActivityKind.PEER_DISCONNECTED -> CRTheme.textMedium(isDark)
        ActivityKind.FILE_RECEIVED, ActivityKind.FILE_SENT, ActivityKind.FILE_TRANSFER_COMPLETE -> CRTheme.cyanSoft
        else -> CRTheme.blueSoft
    }

    var showPreview by remember { mutableStateOf(false) }
    
    val isMediaFile = (entry.kind == ActivityKind.FILE_RECEIVED || entry.kind == ActivityKind.FILE_TRANSFER_COMPLETE) && 
            entry.destPath.lowercase().let { it.endsWith(".jpg") || it.endsWith(".jpeg") || it.endsWith(".png") || it.endsWith(".webp") || it.endsWith(".gif") || it.endsWith(".mp4") || it.endsWith(".mkv") }

    val offsetX = remember { androidx.compose.animation.core.Animatable(0f) }
    val coroutineScope = rememberCoroutineScope()
    
    if (showPreview && isMediaFile) {
        ImagePreviewDialog(filePath = entry.destPath, onDismiss = { showPreview = false })
    }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(if (offsetX.value < -20f) CRTheme.statusRed.copy(alpha = 0.15f) else Color.Transparent, RoundedCornerShape(8.dp))
    ) {
        if (offsetX.value < -20f) {
            Box(modifier = Modifier.fillMaxSize().padding(end = 16.dp), contentAlignment = Alignment.CenterEnd) {
                Icon(Icons.Default.Delete, contentDescription = "Delete", tint = CRTheme.statusRed, modifier = Modifier.size(20.dp))
            }
        }
        
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .offset { androidx.compose.ui.unit.IntOffset(offsetX.value.toInt(), 0) }
                .pointerInput(Unit) {
                    detectHorizontalDragGestures(
                        onHorizontalDrag = { change, dragAmount ->
                            if (offsetX.value + dragAmount <= 0) {
                                coroutineScope.launch { offsetX.snapTo(offsetX.value + dragAmount) }
                                change.consume() // Prevent the HorizontalPager from stealing this swipe
                            }
                        },
                        onDragEnd = {
                            if (offsetX.value < -200f) {
                                coroutineScope.launch { 
                                    offsetX.animateTo(-1000f)
                                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                    onDelete(entry)
                                }
                            } else {
                                coroutineScope.launch { offsetX.animateTo(0f, androidx.compose.animation.core.spring()) }
                            }
                        },
                        onDragCancel = {
                            coroutineScope.launch { offsetX.animateTo(0f, androidx.compose.animation.core.spring()) }
                        }
                    )
                }
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null
                ) {
                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    if (isMediaFile) {
                        showPreview = true
                    } else {
                        onApply(entry)
                    }
                }
        ) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Icon Bullet or Thumbnail
            if (isMediaFile) {
                coil.compose.AsyncImage(
                    model = "file://${entry.destPath}",
                    contentDescription = "File preview",
                    modifier = Modifier
                        .size(44.dp)
                        .clip(CircleShape)
                        .border(1.dp, CRTheme.stroke(isDark), CircleShape),
                    contentScale = androidx.compose.ui.layout.ContentScale.Crop
                )
            } else {
                Box(
                    contentAlignment = Alignment.Center,
                    modifier = Modifier
                        .size(44.dp)
                        .background(dotColor.copy(alpha = 0.15f), CircleShape)
                ) {
                    Icon(
                        imageVector = icon,
                        contentDescription = "Activity type",
                        tint = dotColor,
                        modifier = Modifier.size(20.dp)
                    )
                }
            }
            
            Spacer(modifier = Modifier.width(8.dp))
            
            // Content
            Column(modifier = Modifier.weight(1f).padding(vertical = 4.dp)) {
                Text(text = title, style = CRTypography.label, color = CRTheme.textHigh(isDark), maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(text = subtitle, style = CRTypography.caption, color = CRTheme.textMedium(isDark), maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            
            IconButton(
                onClick = { onDelete(entry) },
                modifier = Modifier.size(28.dp)
            ) {
                Icon(
                    imageVector = Icons.Default.Delete,
                    contentDescription = "Delete history item",
                    tint = CRTheme.textMedium(isDark).copy(alpha = 0.7f),
                    modifier = Modifier.size(16.dp)
                )
            }
            IconButton(
                onClick = { showMenu = true },
                modifier = Modifier.size(24.dp)
            ) {
                Icon(imageVector = Icons.Default.MoreVert, contentDescription = "More", tint = CRTheme.textMedium(isDark), modifier = Modifier.size(16.dp))
            }
        }
        
        val primaryActionLabel = when (entry.kind) {
            ActivityKind.CLIPBOARD_TEXT -> if (isLink) "Open Link" else "Copy Again"
            ActivityKind.CLIPBOARD_IMAGE -> "Copy Image"
            ActivityKind.FILE_RECEIVED, ActivityKind.FILE_TRANSFER_COMPLETE -> "Show in Downloads"
            ActivityKind.FILE_SENT -> "Send Again"
            ActivityKind.PEER_CONNECTED -> "Open Device"
            ActivityKind.WARNING -> "Fix Issue"
            else -> "Open / Copy"
        }
        
        androidx.compose.material3.DropdownMenu(
            expanded = showMenu,
            onDismissRequest = { showMenu = false },
            modifier = Modifier.background(if (isDark) Color(0xFF1E1E1E) else Color.White)
        ) {
            androidx.compose.material3.DropdownMenuItem(
                text = { Text(primaryActionLabel, color = CRTheme.textHigh(isDark)) },
                onClick = { showMenu = false; onApply(entry) }
            )
            androidx.compose.material3.DropdownMenuItem(
                text = { Text("Resend", color = CRTheme.textHigh(isDark)) },
                onClick = { showMenu = false; onResend(entry) }
            )
            androidx.compose.material3.DropdownMenuItem(
                text = { Text("Delete history", color = CRTheme.accentRed) },
                onClick = { showMenu = false; onDelete(entry) }
            )
        }
        }
    }
}

