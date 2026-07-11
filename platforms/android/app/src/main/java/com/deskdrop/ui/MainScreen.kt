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

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class, androidx.compose.animation.ExperimentalAnimationApi::class)
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
    onManualIp: () -> Unit,
    onActionPauseSync: () -> Unit,
    onActionDisconnectAll: () -> Unit,
    onActionStopService: () -> Unit,
    onActionStreamCamera: () -> Unit,
    onActionPauseTransfer: (String) -> Unit,
    onActionResumeTransfer: (String) -> Unit,
    onActionCancelTransfer: (String) -> Unit,
    onActionAcceptTransfer: (String) -> Unit,
    onActionRejectTransfer: (String) -> Unit,
    onActionSendFiles: (String?) -> Unit,
    onDropFiles: (String, List<android.net.Uri>) -> Unit = { _, _ -> },
    onApplyClipboard: (ActivityEntry) -> Unit,
    onTrustPeer: (PeerSnapshot) -> Unit,
    onRejectPeer: (PeerSnapshot) -> Unit,
    onConnectPeer: (PeerSnapshot) -> Unit,
    onDisconnectPeer: (PeerSnapshot) -> Unit,
    onForgetPeer: (PeerSnapshot) -> Unit,
    onSendPairingRequest: (PeerSnapshot) -> Unit,
    onRespondPairing: (PeerSnapshot, Boolean) -> Unit,
    onOpenSettings: () -> Unit,
    onOpenDiagnostics: () -> Unit,
    onDeleteActivity: (ActivityEntry) -> Unit = {},
    onResendActivity: (ActivityEntry) -> Unit = {},
    onReplayOnboarding: () -> Unit = {}
) {
    @OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
    val pagerState = androidx.compose.foundation.pager.rememberPagerState(initialPage = AppTab.Home.ordinal, pageCount = { AppTab.values().size })
    val currentTab = AppTab.values()[pagerState.targetPage]
    val scope = rememberCoroutineScope()
    
    val hasConnectedDevices = peers.any { it.isConnected }

    CRBackground(isDark = isDark, hasConnectedDevices = hasConnectedDevices) {
        Box(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
            Column(modifier = Modifier.fillMaxSize()) {

                
                Box(modifier = Modifier.weight(1f)) {
                    
                    @OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
                    androidx.compose.foundation.pager.HorizontalPager(
                        state = pagerState,
                        modifier = Modifier.fillMaxSize()
                    ) { page ->
                        when (AppTab.values()[page]) {
                            AppTab.Home -> HomeTab(
                                isDark = isDark,
                                peers = peers,
                                feed = feed,
                                activeTransfers = activeTransfers,
                                onActionPushClipboard = onActionPushClipboard,
                                onActionSendQuickContext = {
                                    onActionPushClipboard()
                                },
                                quickContextText = DeskdropService.quickSendContextFlow.collectAsState().value,
                                onActionPairMagicLink = onActionPairMagicLink,
                                onManualIp = onManualIp,
                                onActionSendFiles = onActionSendFiles,
                                onDropFiles = onDropFiles,
                                onActionStreamCamera = onActionStreamCamera,
                                onApplyClipboard = onApplyClipboard,
                                onActionPauseTransfer = onActionPauseTransfer,
                                onActionResumeTransfer = onActionResumeTransfer,
                                onActionCancelTransfer = onActionCancelTransfer,
                                onForgetPeer = onForgetPeer,
                                onRejectPeer = onRejectPeer,
                                onTrustPeer = onTrustPeer,
                                onConnectPeer = onConnectPeer,
                                onSendPairingRequest = onSendPairingRequest,
                                onDeleteActivity = onDeleteActivity,
                                onResendActivity = onResendActivity,
                                onReplayOnboarding = onReplayOnboarding,
                                onTabSelected = { tab -> scope.launch { pagerState.animateScrollToPage(tab.ordinal) } }
                            )
                            AppTab.Activity -> ActivityTab(
                                isDark = isDark,
                                feed = feed,
                                onApplyClipboard = onApplyClipboard,
                                onDeleteActivity = onDeleteActivity
                            )
                            AppTab.Devices -> DevicesTab(
                                isDark = isDark,
                                peers = peers,
                                onTrustPeer = onTrustPeer,
                                onRejectPeer = onRejectPeer,
                                onConnectPeer = onConnectPeer,
                                onDisconnectPeer = onDisconnectPeer,
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
                                onOpenSettings = onOpenSettings,
                                onOpenDiagnostics = onOpenDiagnostics
                            )
                        }
                    }
                }
            }
            
            Box(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .height(120.dp)
                    .background(
                        androidx.compose.ui.graphics.Brush.verticalGradient(
                            colors = listOf(
                                Color.Transparent,
                                CRTheme.bg(isDark).copy(alpha = 0.8f),
                                CRTheme.bg(isDark)
                            )
                        )
                    )
            )

            // Dynamic Island Overlay
            Box(modifier = Modifier.align(Alignment.TopCenter).padding(top = 16.dp)) {
                DynamicIslandOverlay(
                    activeTransfers = activeTransfers,
                    isDark = isDark,
                    onAccept = onActionAcceptTransfer,
                    onReject = onActionRejectTransfer,
                    onCancel = onActionCancelTransfer,
                    onPause = onActionPauseTransfer,
                    onResume = onActionResumeTransfer
                )
            }
            
            Box(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 24.dp)
            ) {
                BottomDock(
                    currentTab = currentTab,
                    onTabSelected = { tab ->
                        scope.launch { pagerState.animateScrollToPage(tab.ordinal) }
                    },
                    isDark = isDark
                )
            }
        }
    }
}

@Composable
fun BottomDock(
    currentTab: AppTab,
    onTabSelected: (AppTab) -> Unit,
    isDark: Boolean
) {
    val haptic = LocalHapticFeedback.current
    val tabs = AppTab.values()
    val selectedIndex = tabs.indexOf(currentTab)
    
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 48.dp, vertical = 16.dp)
    ) {
        BoxWithConstraints(
            modifier = Modifier
                .fillMaxWidth()
                .crGlassCard(isDark = isDark, cornerRadius = 100.dp, elevated = true)
                .padding(6.dp),
            contentAlignment = Alignment.CenterStart
        ) {
            val tabWidth = maxWidth / tabs.size
            
            val transition = updateTransition(targetState = selectedIndex, label = "tabTransition")
            
            val indicatorLeft by transition.animateDp(
                transitionSpec = {
                    if (targetState > initialState) {
                        spring(dampingRatio = 0.7f, stiffness = 150f)
                    } else {
                        spring(dampingRatio = 0.7f, stiffness = 600f)
                    }
                },
                label = "indicatorLeft"
            ) { state: Int -> tabWidth * state.toFloat() }
            
            val indicatorRight by transition.animateDp(
                transitionSpec = {
                    if (targetState > initialState) {
                        spring(dampingRatio = 0.7f, stiffness = 600f)
                    } else {
                        spring(dampingRatio = 0.7f, stiffness = 150f)
                    }
                },
                label = "indicatorRight"
            ) { state: Int -> tabWidth * (state + 1).toFloat() }
            
            Box(
                modifier = Modifier
                    .offset(x = indicatorLeft)
                    .width(indicatorRight - indicatorLeft)
                    .height(48.dp)
                    .padding(horizontal = 2.dp)
                    .background(CRTheme.textHigh(isDark).copy(alpha = 0.08f), CircleShape)
            )
            
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceEvenly,
                verticalAlignment = Alignment.CenterVertically
            ) {
                tabs.forEach { tab ->
                    Box(
                        modifier = Modifier
                            .width(tabWidth)
                            .height(48.dp)
                            .clip(CircleShape)
                            .clickable(
                                interactionSource = remember { MutableInteractionSource() },
                                indication = null,
                                onClick = {
                                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                    onTabSelected(tab)
                                }
                            ),
                        contentAlignment = Alignment.Center
                    ) {
                        val isSelected = currentTab == tab
                        val iconColor = if (isSelected) CRTheme.blueSoft else CRTheme.textHigh(isDark).copy(alpha = 0.4f)
                        
                        val scale by animateFloatAsState(
                            targetValue = if (isSelected) 1.2f else 1f,
                            animationSpec = spring(dampingRatio = 0.5f, stiffness = 300f)
                        )
                        
                        Icon(
                            imageVector = when (tab) {
                                AppTab.Home -> Icons.Default.Home
                                AppTab.Activity -> Icons.Default.List
                                AppTab.Devices -> Icons.Default.Devices
                                AppTab.Settings -> Icons.Default.Settings
                            },
                            contentDescription = tab.name,
                            tint = iconColor,
                            modifier = Modifier.size(24.dp).scale(scale)
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun DynamicIslandOverlay(
    activeTransfers: List<TransferProgress>,
    isDark: Boolean,
    onAccept: (String) -> Unit,
    onReject: (String) -> Unit,
    onCancel: (String) -> Unit,
    onPause: (String) -> Unit,
    onResume: (String) -> Unit
) {
    val activeTransfer = activeTransfers.firstOrNull { 
        it.state == TransferState.INCOMING || it.state == TransferState.PROGRESS || it.state == TransferState.PAUSED 
    }

    var isExpanded by remember { mutableStateOf(false) }

    androidx.compose.animation.AnimatedVisibility(
        visible = activeTransfer != null,
        enter = androidx.compose.animation.slideInVertically(initialOffsetY = { -it - 50 }) + androidx.compose.animation.fadeIn(),
        exit = androidx.compose.animation.slideOutVertically(targetOffsetY = { -it - 50 }) + androidx.compose.animation.fadeOut()
    ) {
        if (activeTransfer != null) {
            val targetProgress = if (activeTransfer.totalBytes > 0) (activeTransfer.bytesReceived.toFloat() / activeTransfer.totalBytes.toFloat()) else 1f
            val animatedProgress by animateFloatAsState(
                targetValue = targetProgress,
                animationSpec = tween(durationMillis = 250, easing = LinearEasing),
                label = "overlay_progress"
            )
            
            Box(
                modifier = Modifier
                    .padding(horizontal = 16.dp)
                    .crGlassCard(isDark = isDark, cornerRadius = if (isExpanded) 24.dp else 32.dp, onClick = {
                        if (activeTransfer.state != TransferState.INCOMING) {
                            isExpanded = !isExpanded
                        }
                    })
                    .background(CRTheme.surfaceElevated(isDark), RoundedCornerShape(if (isExpanded) 24.dp else 32.dp))
                    .border(1.dp, CRTheme.stroke(isDark).copy(alpha = 0.5f), RoundedCornerShape(if (isExpanded) 24.dp else 32.dp))
                    .padding(horizontal = 16.dp, vertical = if (isExpanded) 16.dp else 12.dp)
                    .animateContentSize()
            ) {
                Column {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        // Content left side
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.weight(1f)
                        ) {
                            val icon = if (activeTransfer.state == TransferState.INCOMING) Icons.Default.NotificationsActive else if (activeTransfer.isOutbound) Icons.Default.FileUpload else Icons.Default.FileDownload
                            val color = if (activeTransfer.state == TransferState.INCOMING) CRTheme.accentAmber else CRTheme.blueSoft
                            
                            Box(
                                modifier = Modifier
                                    .size(36.dp)
                                    .background(color.copy(alpha = 0.15f), CircleShape),
                                contentAlignment = Alignment.Center
                            ) {
                                Icon(imageVector = icon, contentDescription = "Transfer status", tint = color, modifier = Modifier.size(18.dp))
                            }
                            
                            Spacer(modifier = Modifier.width(12.dp))
                            
                            Column {
                                val title = if (activeTransfer.state == TransferState.INCOMING) "Incoming from ${activeTransfer.peerName}" else if (activeTransfer.isOutbound) "Sending ${activeTransfer.fileName}" else "Receiving ${activeTransfer.fileName}"
                                Text(title, style = CRTypography.bodyMedium.copy(fontWeight = FontWeight.Medium), color = CRTheme.textHigh(isDark), maxLines = 1)
                                
                                val subtitle = if (activeTransfer.state == TransferState.INCOMING) {
                                    "${android.text.format.Formatter.formatFileSize(androidx.compose.ui.platform.LocalContext.current, activeTransfer.totalBytes)}"
                                } else {
                                    val percentStr = String.format(java.util.Locale.US, "%.2f%%", animatedProgress * 100)
                                    val formattedReceived = android.text.format.Formatter.formatFileSize(androidx.compose.ui.platform.LocalContext.current, activeTransfer.bytesReceived)
                                    val formattedTotal = android.text.format.Formatter.formatFileSize(androidx.compose.ui.platform.LocalContext.current, activeTransfer.totalBytes)
                                    val speedStr = if (activeTransfer.speedBps > 0) "${activeTransfer.speedBps / 1024 / 1024} MB/s" else "Calculating..."
                                    "$percentStr • $formattedReceived / $formattedTotal • $speedStr"
                                }
                                Text(subtitle, style = CRTypography.caption, color = CRTheme.textMedium(isDark), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                        
                        Spacer(modifier = Modifier.width(16.dp))
                        
                        // Buttons right side
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            if (activeTransfer.state == TransferState.INCOMING) {
                                // Reject Button
                                Box(
                                    modifier = Modifier
                                        .clip(CircleShape)
                                        .clickable { onReject(activeTransfer.id) }
                                        .background(CRTheme.accentRed.copy(alpha = 0.15f))
                                        .padding(8.dp),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Icon(Icons.Default.Close, contentDescription = "Reject", tint = CRTheme.accentRed, modifier = Modifier.size(18.dp))
                                }
                                
                                // Accept Button
                                Box(
                                    modifier = Modifier
                                        .clip(CircleShape)
                                        .clickable { onAccept(activeTransfer.id) }
                                        .background(CRTheme.accentGreen)
                                        .padding(horizontal = 16.dp, vertical = 8.dp),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Text("Accept", style = CRTypography.caption.copy(fontWeight = FontWeight.Bold), color = Color.White)
                                }
                            } else if (!isExpanded) {
                                // Cancel progress button (only in collapsed state, in expanded it moves down)
                                Box(
                                    modifier = Modifier
                                        .clip(CircleShape)
                                        .clickable { onCancel(activeTransfer.id) }
                                        .background(CRTheme.textHigh(isDark).copy(alpha = 0.1f))
                                        .padding(8.dp),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Icon(Icons.Default.Close, contentDescription = "Cancel", tint = CRTheme.textHigh(isDark), modifier = Modifier.size(18.dp))
                                }
                            }
                        }
                    }

                    if (isExpanded && activeTransfer.state != TransferState.INCOMING) {
                        Spacer(modifier = Modifier.height(16.dp))
                        
                        // Progress Bar
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp)
                                .background(CRTheme.textMedium(isDark).copy(alpha = 0.2f), RoundedCornerShape(3.dp))
                                .clip(RoundedCornerShape(3.dp))
                        ) {
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth(animatedProgress.coerceIn(0f, 1f))
                                    .height(6.dp)
                                    .background(if (activeTransfer.isPaused) CRTheme.accentAmber else CRTheme.brandCyan)
                            )
                        }
                        
                        Spacer(modifier = Modifier.height(16.dp))
                        
                        // Action Buttons
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.End,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            val haptic = LocalHapticFeedback.current
                            Box(
                                modifier = Modifier
                                    .size(40.dp)
                                    .clip(CircleShape)
                                    .background(CRTheme.textHigh(isDark).copy(alpha = 0.05f))
                                    .clickable {
                                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                        if (activeTransfer.isPaused) onResume(activeTransfer.id) else onPause(activeTransfer.id)
                                    },
                                contentAlignment = Alignment.Center
                            ) {
                                Icon(
                                    imageVector = if (activeTransfer.isPaused) Icons.Default.PlayArrow else Icons.Default.Pause,
                                    contentDescription = if (activeTransfer.isPaused) "Resume" else "Pause",
                                    tint = CRTheme.textHigh(isDark),
                                    modifier = Modifier.size(18.dp)
                                )
                            }
                            Spacer(modifier = Modifier.width(8.dp))
                            Box(
                                modifier = Modifier
                                    .size(40.dp)
                                    .clip(CircleShape)
                                    .background(CRTheme.accentRed.copy(alpha = 0.1f))
                                    .clickable {
                                        haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                                        onCancel(activeTransfer.id)
                                    },
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
            }
        }
    }
}
