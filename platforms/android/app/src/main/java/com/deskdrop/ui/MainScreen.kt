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
import androidx.compose.animation.core.updateTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.animateDp
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.Spring
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.ui.input.pointer.pointerInput
import kotlinx.coroutines.launch
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.Brush
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
import com.deskdrop.DeskdropService
import com.deskdrop.ui.getLocalIpAddress
import androidx.compose.material3.TextButton
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import com.deskdrop.ActivityEntry
import com.deskdrop.ActivityKind
import com.deskdrop.PeerSnapshot
import com.deskdrop.TransferProgress
import com.deskdrop.SpeedTestProgress
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

@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@Composable
fun MainScreen(
    isDark: Boolean,
    isServiceRunning: Boolean,
    isSyncEnabled: Boolean,
    syncText: Boolean,
    syncImages: Boolean,
    syncFiles: Boolean,
    callContinuityEnabled: Boolean,
    notificationMirroringEnabled: Boolean,
    autoForwardSms: Boolean,
    autoForwardScreenshots: Boolean,
    deviceName: String,
    deviceId: String,
    peers: List<PeerSnapshot>,
    feed: List<ActivityEntry>,
    ambientStatus: String,
    activeTransfers: List<TransferProgress>,
    activeSpeedTests: List<SpeedTestProgress> = emptyList(),
    onActionStartSpeedTest: (String) -> Unit = {},
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
    onActionAcceptTransfer: (String) -> Unit = {},
    onActionRejectTransfer: (String) -> Unit = {},
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
    onSyncEnabledChange: (Boolean) -> Unit,
    onSyncTextChange: (Boolean) -> Unit,
    onSyncImagesChange: (Boolean) -> Unit,
    onSyncFilesChange: (Boolean) -> Unit,
    onCallContinuityChange: (Boolean) -> Unit,
    onNotificationMirroringChange: (Boolean) -> Unit,
    onAutoForwardSmsChange: (Boolean) -> Unit,
    onAutoForwardScreenshotsChange: (Boolean) -> Unit,
    onDarkModeChange: (Boolean) -> Unit,
    onForgetDevice: (String) -> Unit,
    onOpenDiagnostics: () -> Unit,
    onBatterySettingsClicked: () -> Unit = {},
    onStorageSettingsClicked: () -> Unit = {},
    onNotificationSettingsClicked: () -> Unit = {},
    onDeleteActivity: (ActivityEntry) -> Unit = {},
    onResendActivity: (ActivityEntry) -> Unit = {},
    onReplayOnboarding: () -> Unit = {}
) {
    var currentTab by rememberSaveable { mutableStateOf(AppTab.Home) }
    val tabs = AppTab.values()
    val pagerState = androidx.compose.foundation.pager.rememberPagerState(
        initialPage = tabs.indexOf(currentTab),
        pageCount = { tabs.size }
    )

    LaunchedEffect(currentTab) {
        val targetPage = tabs.indexOf(currentTab)
        if (pagerState.currentPage != targetPage) {
            pagerState.animateScrollToPage(targetPage, animationSpec = tween(400, easing = androidx.compose.animation.core.FastOutSlowInEasing))
        }
    }

    LaunchedEffect(pagerState.currentPage, pagerState.isScrollInProgress) {
        if (!pagerState.isScrollInProgress) {
            currentTab = tabs[pagerState.currentPage]
        }
    }

    val hasConnectedDevices = peers.any { it.isConnected }

    CRBackground(isDark = isDark, hasConnectedDevices = hasConnectedDevices) {
        Box(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
            Column(modifier = Modifier.fillMaxSize()) {

                
                Box(modifier = Modifier.weight(1f)) {
                    androidx.compose.foundation.pager.HorizontalPager(
                        state = pagerState,
                        modifier = Modifier.fillMaxSize()
                    ) { page ->
                        when (tabs[page]) {
                            AppTab.Home -> HomeTab(
                                isDark = isDark,
                                peers = peers,
                                feed = feed,
                                activeTransfers = activeTransfers,
                                activeSpeedTests = activeSpeedTests,
                                onActionStartSpeedTest = onActionStartSpeedTest,
                                onActionPushClipboard = onActionPushClipboard,
                                onActionSendQuickContext = {
                                    onActionPushClipboard()
                                },
                                quickContextText = DeskdropService.quickSendContextFlow.collectAsState().value,
                                onActionPairMagicLink = onActionPairMagicLink,
                                onManualIp = onManualIp,
                                onActionSendFiles = onActionSendFiles,
                                onActionStreamCamera = onActionStreamCamera,
                                onApplyClipboard = onApplyClipboard,
                                onActionPauseTransfer = onActionPauseTransfer,
                                onActionResumeTransfer = onActionResumeTransfer,
                                onActionCancelTransfer = onActionCancelTransfer,
                                onForgetPeer = onForgetPeer,
                                onDeleteActivity = onDeleteActivity,
                                onResendActivity = onResendActivity,
                                onReplayOnboarding = onReplayOnboarding,
                                onTabSelected = { currentTab = it },
                                onRespondPairing = onRespondPairing
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
                                onConnectPeer = onConnectPeer,
                                onDisconnectPeer = onDisconnectPeer,
                                onSendPairingRequest = onSendPairingRequest,
                                onRespondPairing = onRespondPairing
                            )
                            AppTab.Settings -> SettingsTab(
                                isDark = isDark,
                                isServiceRunning = isServiceRunning,
                                isSyncEnabled = isSyncEnabled,
                                syncText = syncText,
                                syncImages = syncImages,
                                syncFiles = syncFiles,
                                callContinuityEnabled = callContinuityEnabled,
                                notificationMirroringEnabled = notificationMirroringEnabled,
                                autoForwardSms = autoForwardSms,
                                autoForwardScreenshots = autoForwardScreenshots,
                                deviceName = deviceName,
                                deviceId = deviceId,
                                peers = peers,
                                onSyncEnabledChange = onSyncEnabledChange,
                                onSyncTextChange = onSyncTextChange,
                                onSyncImagesChange = onSyncImagesChange,
                                onSyncFilesChange = onSyncFilesChange,
                                onCallContinuityChange = onCallContinuityChange,
                                onNotificationMirroringChange = onNotificationMirroringChange,
                                onAutoForwardSmsChange = onAutoForwardSmsChange,
                                onAutoForwardScreenshotsChange = onAutoForwardScreenshotsChange,
                                onDarkModeChange = onDarkModeChange,
                                onForgetDevice = onForgetDevice,
                                onStartSync = onStartSync,
                                onResumeSync = onResumeSync,
                                onScanNow = onScanNow,
                                onActionPauseSync = onActionPauseSync,
                                onActionDisconnectAll = onActionDisconnectAll,
                                onActionStopService = onActionStopService,
                                onOpenDiagnostics = onOpenDiagnostics,
                                onBatterySettingsClicked = onBatterySettingsClicked,
                                onStorageSettingsClicked = onStorageSettingsClicked,
                                onNotificationSettingsClicked = onNotificationSettingsClicked
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
            
            Box(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 24.dp)
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
fun EmptyStateGlassCard(
    isDark: Boolean,
    icon: ImageVector,
    title: String,
    description: String
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .crGlassCard(isDark = isDark, cornerRadius = 24.dp, dashed = true)
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Box(
            modifier = Modifier
                .size(64.dp)
                .background(CRTheme.surfaceElevated(isDark), CircleShape)
                .padding(16.dp),
            contentAlignment = Alignment.Center
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = CRTheme.textMedium(isDark).copy(alpha = 0.8f),
                modifier = Modifier.fillMaxSize()
            )
        }
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = title,
            style = CRTypography.h2,
            color = CRTheme.textHigh(isDark),
            fontWeight = FontWeight.SemiBold,
            textAlign = TextAlign.Center
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = description,
            style = CRTypography.bodyMedium,
            color = CRTheme.textMedium(isDark),
            textAlign = TextAlign.Center
        )
    }
}



@Composable
fun HomeTab(
    isDark: Boolean,
    peers: List<PeerSnapshot>,
    feed: List<ActivityEntry>,
    activeTransfers: List<TransferProgress>,
    activeSpeedTests: List<SpeedTestProgress>,
    onActionStartSpeedTest: (String) -> Unit,
    onActionPushClipboard: () -> Unit,
    onActionSendQuickContext: () -> Unit,
    quickContextText: String?,
    onActionPairMagicLink: () -> Unit,
    onManualIp: () -> Unit,
    onActionSendFiles: (String?) -> Unit,
    onActionStreamCamera: () -> Unit,
    onApplyClipboard: (ActivityEntry) -> Unit,
    onActionPauseTransfer: (String) -> Unit,
    onActionResumeTransfer: (String) -> Unit,
    onActionCancelTransfer: (String) -> Unit,
    onForgetPeer: (PeerSnapshot) -> Unit,
    onDeleteActivity: (ActivityEntry) -> Unit,
    onResendActivity: (ActivityEntry) -> Unit,
    onReplayOnboarding: () -> Unit,
    onTabSelected: (AppTab) -> Unit,
    onRespondPairing: (PeerSnapshot, Boolean) -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        val hasConnectedPeers = peers.any { it.isConnected || it.trusted }
        
        Spacer(modifier = Modifier.height(12.dp)) // Tighter gap from Status Strip
        
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
                    Row(
                        modifier = Modifier
                            .crPressScale(0.95f)
                            .clip(RoundedCornerShape(12.dp))
                            .clickable { showQrDialog = true }
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(imageVector = Icons.Default.QrCode, contentDescription = null, tint = CRTheme.blueSoft, modifier = Modifier.size(14.dp))
                        Spacer(modifier = Modifier.width(4.dp))
                        Text("Show QR", style = CRTypography.caption, color = CRTheme.blueSoft)
                    }

                    if (showQrDialog) {
                        androidx.compose.material3.AlertDialog(
                            onDismissRequest = { showQrDialog = false },
                            title = { Text("Scan to Pair") },
                            text = {
                                Box(modifier = Modifier.fillMaxWidth().aspectRatio(1f), contentAlignment = Alignment.Center) {
                                    val ip = getLocalIpAddress()
                                    val uri = "deskdrop://$ip:47823"
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
                    Row(
                        modifier = Modifier
                            .crPressScale(0.95f)
                            .clip(RoundedCornerShape(12.dp))
                            .clickable { onActionPairMagicLink() }
                            .background(CRTheme.brandElectric.copy(alpha = 0.15f))
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(imageVector = Icons.Default.QrCodeScanner, contentDescription = null, tint = CRTheme.brandElectric, modifier = Modifier.size(14.dp))
                        Spacer(modifier = Modifier.width(4.dp))
                        Text("Scan QR", style = CRTypography.caption, color = CRTheme.brandElectric)
                    }

                    Spacer(modifier = Modifier.width(8.dp))

                    // Manual IP Action
                    Row(
                        modifier = Modifier
                            .crPressScale(0.95f)
                            .clip(RoundedCornerShape(12.dp))
                            .clickable { onManualIp() }
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(imageVector = Icons.Default.Language, contentDescription = null, tint = CRTheme.blueSoft, modifier = Modifier.size(14.dp))
                        Spacer(modifier = Modifier.width(4.dp))
                        Text("IP", style = CRTypography.caption, color = CRTheme.blueSoft)
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
                        onForget = { onForgetPeer(peer) },
                        onStartSpeedTest = { onActionStartSpeedTest(peer.id) },
                        onRespond = { onRespondPairing(peer, it) },
                        speedTestProgress = activeSpeedTests.find { it.peerId == peer.id },
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
            feed = feed,
            onActionPushClipboard = onActionPushClipboard,
            onActionSendFiles = { onActionSendFiles(null) },
            onActionStreamCamera = onActionStreamCamera,
            onApplyClipboard = onApplyClipboard
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
            contentDescription = null,
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
    feed: List<ActivityEntry>,
    onActionPushClipboard: () -> Unit,
    onActionSendFiles: () -> Unit,
    onActionStreamCamera: () -> Unit,
    onApplyClipboard: (ActivityEntry) -> Unit
) {
    Column(
        modifier = Modifier.padding(horizontal = 24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        val recentClipboards = feed.filter { it.kind == ActivityKind.CLIPBOARD_TEXT || it.kind == ActivityKind.CLIPBOARD_IMAGE }.take(5)
        
        if (recentClipboards.isNotEmpty()) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(24.dp))
                    .background(if (isDark) Color(0xFF1E1E1E) else Color(0xFFF5F5F5))
                    .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Clipboard History", style = CRTypography.h2, color = CRTheme.textHigh(isDark))
                    Box(modifier = Modifier.clickable { onActionPushClipboard() }) {
                        Icon(Icons.Default.ContentCopy, contentDescription = "Push", tint = CRTheme.brandElectric)
                    }
                }
                
                recentClipboards.forEachIndexed { index, entry ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(if (index == 0) CRTheme.brandElectric.copy(alpha=0.1f) else Color.Transparent)
                            .clickable { onApplyClipboard(entry) }
                            .padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = entry.preview,
                            style = CRTypography.bodyMedium,
                            color = if (index == 0) CRTheme.brandElectric else CRTheme.textMedium(isDark),
                            maxLines = 1,
                            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f)
                        )
                        Icon(Icons.Default.ContentCopy, contentDescription = null, tint = CRTheme.textMedium(isDark).copy(alpha=0.5f), modifier = Modifier.size(14.dp))
                    }
                }
            }
        } else {
            // Large Primary Action fallback
            QuickActionCardPrimary(
                isDark = isDark,
                enabled = enabled,
                icon = Icons.Default.ContentCopy,
                title = "Clipboard Sync",
                subtitle = "Send copied text & images",
                color = CRTheme.brandElectric,
                onClick = onActionPushClipboard
            )
        }
        
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
                onClick = {}
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
            .width(320.dp)
            .crGlassCard(isDark = isDark, cornerRadius = 24.dp)
            .padding(24.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
            val infiniteTransition = rememberInfiniteTransition(label = "pulse")
            val scale by infiniteTransition.animateFloat(
                initialValue = 1f,
                targetValue = if (transfer.isPaused) 1f else 1.15f,
                animationSpec = infiniteRepeatable(
                    animation = tween(1000, easing = LinearEasing),
                    repeatMode = RepeatMode.Reverse
                ),
                label = "iconScale"
            )
            val alpha by infiniteTransition.animateFloat(
                initialValue = 0.15f,
                targetValue = if (transfer.isPaused) 0.15f else 0.35f,
                animationSpec = infiniteRepeatable(
                    animation = tween(1000, easing = LinearEasing),
                    repeatMode = RepeatMode.Reverse
                ),
                label = "iconAlpha"
            )
            Box(
                modifier = Modifier
                    .size(44.dp)
                    .background(CRTheme.brandCyan.copy(alpha = alpha), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = Icons.Default.CloudDownload,
                    contentDescription = null,
                    tint = CRTheme.brandCyan,
                    modifier = Modifier.size(24.dp).graphicsLayer {
                        scaleX = scale
                        scaleY = scale
                    }
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
                AnimatedContent(targetState = if (transfer.isPaused) "Paused" else if (transfer.speedBps > 0) "${transfer.speedBps / 1024 / 1024} MB/s" else "Calculating...", label = "speed_anim") { speedText ->
                    Text(
                        text = speedText,
                        style = CRTypography.caption,
                        color = CRTheme.textMedium(isDark),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }
            
            Row(
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
                        modifier = Modifier.size(16.dp)
                    )
                }
                Spacer(modifier = Modifier.width(8.dp))
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
                        modifier = Modifier.size(16.dp)
                    )
                }
            }
        }
        
        Spacer(modifier = Modifier.height(20.dp))
        
        val exactRatio = if (transfer.totalBytes > 0L) {
            (transfer.bytesReceived.toDouble() / transfer.totalBytes.toDouble()).coerceIn(0.0, 1.0)
        } else {
            transfer.percent / 100.0
        }
        val exactPercentText = if (transfer.totalBytes > 0L) {
            String.format("%.1f%%", exactRatio * 100.0)
        } else {
            "${transfer.percent}%"
        }
        
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            val downloadedMB = transfer.bytesReceived / 1024 / 1024
            val totalMB = transfer.totalBytes / 1024 / 1024
            val sizeText = if (transfer.totalBytes > 0) "${downloadedMB} MB / ${totalMB} MB" else "${downloadedMB} MB"
            
            Text(
                text = exactPercentText,
                style = CRTypography.caption,
                color = CRTheme.textHigh(isDark)
            )
            Text(
                text = sizeText,
                style = CRTypography.caption,
                color = CRTheme.textMedium(isDark)
            )
        }
        
        Spacer(modifier = Modifier.height(8.dp))
        
        val animatedRatio by animateFloatAsState(
            targetValue = exactRatio.toFloat(),
            animationSpec = tween(durationMillis = 150, easing = LinearEasing),
            label = "progressBar"
        )
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(10.dp)
                .background(CRTheme.textMedium(isDark).copy(alpha = 0.2f), RoundedCornerShape(5.dp))
        ) {
            val gradient = Brush.horizontalGradient(
                colors = listOf(CRTheme.brandCyan, Color(0xFF32ADE6))
            )
            Box(
                modifier = Modifier
                    .fillMaxWidth(animatedRatio)
                    .height(10.dp)
                    .background(if (transfer.isPaused) Brush.horizontalGradient(listOf(CRTheme.accentAmber, CRTheme.accentAmber)) else gradient, RoundedCornerShape(5.dp))
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
                        Text("View All", style = CRTypography.caption, color = CRTheme.brandElectric)
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
    
    val subtitle = if (entry.preview.isNotEmpty() && entry.kind != ActivityKind.WARNING && entry.kind != ActivityKind.PEER_CONNECTED && entry.kind != ActivityKind.PEER_DISCONNECTED) {
        entry.preview
    } else {
        "Just now"
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
        ActivityKind.FILE_RECEIVED, ActivityKind.FILE_SENT, ActivityKind.FILE_TRANSFER_COMPLETE -> CRTheme.brandCyan
        else -> CRTheme.brandElectric
    }

    val offsetX = remember { androidx.compose.animation.core.Animatable(0f) }
    val coroutineScope = rememberCoroutineScope()
    
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
                        }
                    )
                }
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null
                ) {
                    haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    onApply(entry)
                }
        ) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Icon Bullet
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .size(44.dp)
                    .background(dotColor.copy(alpha = 0.15f), CircleShape)
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = dotColor,
                    modifier = Modifier.size(20.dp)
                )
            }
            
            Spacer(modifier = Modifier.width(8.dp))
            
            // Content
            Column(modifier = Modifier.weight(1f).padding(vertical = 4.dp)) {
                Text(text = title, style = CRTypography.label, color = CRTheme.textHigh(isDark), maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(text = subtitle, style = CRTypography.caption, color = CRTheme.textMedium(isDark), maxLines = 1, overflow = TextOverflow.Ellipsis)
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

@Composable
fun DeviceCard(
    isDark: Boolean,
    peer: PeerSnapshot,
    onSendFiles: () -> Unit,
    onForget: () -> Unit,
    onStartSpeedTest: () -> Unit = {},
    onRespond: (Boolean) -> Unit = {},
    speedTestProgress: SpeedTestProgress? = null,
    modifier: Modifier = Modifier.width(170.dp)
) {
    val haptic = LocalHapticFeedback.current
    val isPhone = peer.name.contains("phone", ignoreCase = true) || peer.name.contains("pixel", ignoreCase = true)
    var showMenu by remember { mutableStateOf(false) }
    
    val infiniteTransition = rememberInfiniteTransition(label = "glow")
    val glowAlpha by infiniteTransition.animateFloat(
        initialValue = 0.1f,
        targetValue = 0.4f,
        animationSpec = infiniteRepeatable(
            animation = tween(1500, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "glow_alpha"
    )
    
    Box(modifier = modifier.height(116.dp)) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .crPressScale(targetScale = 0.95f)
                .then(
                    if (peer.isConnected) Modifier.border(1.dp, CRTheme.statusGreen.copy(alpha = glowAlpha), RoundedCornerShape(24.dp))
                    else Modifier
                )
                .crGlassCard(isDark = isDark, cornerRadius = 24.dp, onClick = {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    showMenu = true
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
                        contentDescription = null,
                        tint = if (peer.trusted) CRTheme.blueSoft else CRTheme.textMedium(isDark),
                        modifier = Modifier.size(20.dp)
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
                        text = if (peer.lifecycleState == "pairing_in_progress") "Waiting for ${peer.name}" else "Pending",
                        style = CRTypography.caption,
                        color = CRTheme.statusAmber
                    )
                }
            }
        }
        if (speedTestProgress != null) {
            Spacer(modifier = Modifier.height(12.dp))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween, modifier = Modifier.fillMaxWidth()) {
                Text(
                    text = "${speedTestProgress.phase}...",
                    style = CRTypography.caption,
                    color = CRTheme.blueSoft
                )
                Text(
                    text = speedTestProgress.speedMbpsString,
                    style = CRTypography.caption,
                    color = CRTheme.textHigh(isDark)
                )
            }
            Spacer(modifier = Modifier.height(4.dp))
            androidx.compose.material3.LinearProgressIndicator(
                modifier = Modifier.fillMaxWidth().height(2.dp).clip(RoundedCornerShape(1.dp)),
                color = CRTheme.blueSoft,
                trackColor = if(isDark) Color.White.copy(alpha=0.1f) else Color.Black.copy(alpha=0.1f)
            )
        }
        
        androidx.compose.material3.DropdownMenu(
            expanded = showMenu,
            onDismissRequest = { showMenu = false },
            modifier = Modifier.background(if (isDark) Color(0xFF1E1E1E) else Color.White)
        ) {
            if (peer.lifecycleState == "pending_approval" && !peer.trusted) {
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text("Accept Pairing", color = CRTheme.accentGreen) },
                    onClick = { showMenu = false; onRespond(true) }
                )
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text("Reject Pairing", color = CRTheme.accentRed) },
                    onClick = { showMenu = false; onRespond(false) }
                )
            } else if (peer.isConnected) {
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text("Send Files", color = CRTheme.textHigh(isDark)) },
                    onClick = { showMenu = false; onSendFiles() }
                )
                androidx.compose.material3.DropdownMenuItem(
                    text = { Text("Speed Test", color = CRTheme.textHigh(isDark)) },
                    onClick = { showMenu = false; onStartSpeedTest() }
                )
            }
            androidx.compose.material3.DropdownMenuItem(
                text = { Text("Forget Device", color = CRTheme.accentRed) },
                onClick = { showMenu = false; onForget() }
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
            if (feed.isEmpty()) {
                item {
                    EmptyStateGlassCard(
                        isDark = isDark,
                        icon = Icons.Default.List,
                        title = "No Activity Yet",
                        description = "Incoming files, text, and clipboard syncs will appear here safely sandboxed."
                    )
                }
            } else {
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
            val stateText = when {
                peer.explicitDisconnect || peer.lifecycleState == "disconnected" -> "Disconnected"
                peer.lifecycleState == "discovered" -> "Nearby Discovered"
                peer.lifecycleState == "pending_approval" -> "Pending Approval"
                peer.lifecycleState == "paired" -> "Paired Offline"
                peer.lifecycleState == "connected" -> "Connected"
                peer.lifecycleState == "auto_connected" -> "Auto Connected"
                else -> if (peer.trusted) "Trusted Device" else "Pending Approval"
            }
            val stateColor = when {
                peer.explicitDisconnect || peer.lifecycleState == "disconnected" -> CRTheme.textMedium(isDark)
                peer.lifecycleState == "discovered" -> CRTheme.brandElectric
                peer.lifecycleState == "pending_approval" -> CRTheme.accentAmber
                peer.lifecycleState == "paired" -> CRTheme.textMedium(isDark)
                peer.lifecycleState == "connected" || peer.lifecycleState == "auto_connected" -> CRTheme.accentGreen
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
                    .background(CRTheme.blueSoft.copy(alpha = 0.2f), CircleShape)
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
