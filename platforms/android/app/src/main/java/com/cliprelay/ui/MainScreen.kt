package com.cliprelay.ui

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cliprelay.ActivityEntry
import com.cliprelay.ActivityKind
import com.cliprelay.PeerSnapshot
import com.cliprelay.ui.theme.CRBackground
import com.cliprelay.ui.theme.CRTheme
import com.cliprelay.ui.theme.crCard
import kotlinx.coroutines.delay

@OptIn(ExperimentalAnimationApi::class)
@Composable
fun MainScreen(
    isDark: Boolean,
    isServiceRunning: Boolean,
    isSyncEnabled: Boolean,
    peers: List<PeerSnapshot>,
    feed: List<ActivityEntry>,
    ambientStatus: String,
    onStartSync: () -> Unit,
    onResumeSync: () -> Unit,
    onScanNow: () -> Unit,
    onActionPushClipboard: () -> Unit,
    onActionPairMagicLink: () -> Unit,
    onActionPauseSync: () -> Unit,
    onActionDisconnectAll: () -> Unit,
    onActionStopService: () -> Unit,
    onApplyClipboard: (ActivityEntry) -> Unit,
    onTrustPeer: (PeerSnapshot) -> Unit,
    onRejectPeer: (PeerSnapshot) -> Unit,
    onOpenSettings: () -> Unit
) {
    var selectedTab by remember { mutableIntStateOf(0) }

    CRBackground(isDark = isDark) {
        Box(modifier = Modifier.fillMaxSize()) {
            AnimatedContent(
                targetState = selectedTab,
                transitionSpec = {
                    if (targetState > initialState) {
                        slideInHorizontally(animationSpec = spring(stiffness = Spring.StiffnessLow), initialOffsetX = { fullWidth -> fullWidth }) + fadeIn() togetherWith
                                slideOutHorizontally(animationSpec = spring(stiffness = Spring.StiffnessLow), targetOffsetX = { fullWidth -> -fullWidth }) + fadeOut()
                    } else {
                        slideInHorizontally(animationSpec = spring(stiffness = Spring.StiffnessLow), initialOffsetX = { fullWidth -> -fullWidth }) + fadeIn() togetherWith
                                slideOutHorizontally(animationSpec = spring(stiffness = Spring.StiffnessLow), targetOffsetX = { fullWidth -> fullWidth }) + fadeOut()
                    }
                }, label = "tab_transition",
                modifier = Modifier.fillMaxSize()
            ) { targetTab ->
                when (targetTab) {
                    0 -> DashboardTab(
                        isDark = isDark,
                        isServiceRunning = isServiceRunning,
                        isSyncEnabled = isSyncEnabled,
                        peers = peers,
                        ambientStatus = ambientStatus,
                        onTrustPeer = onTrustPeer,
                        onRejectPeer = onRejectPeer,
                        onActionPushClipboard = onActionPushClipboard,
                        onActionPairMagicLink = onActionPairMagicLink,
                        onActionPauseSync = onActionPauseSync,
                        onActionDisconnectAll = onActionDisconnectAll,
                        onActionStopService = onActionStopService,
                        onOpenSettings = onOpenSettings,
                        onStartSync = onStartSync,
                        onResumeSync = onResumeSync,
                        onScanNow = onScanNow
                    )
                    1 -> ActivityFeedTab(
                        isDark = isDark,
                        feed = feed,
                        onApplyClipboard = onApplyClipboard
                    )
                }
            }

            // Floating Bottom Navigation Bar
            Box(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 32.dp, start = 32.dp, end = 32.dp)
            ) {
                FloatingNavBar(
                    selectedTab = selectedTab,
                    isDark = isDark,
                    onTabSelected = { selectedTab = it }
                )
            }
        }
    }
}

@Composable
fun FloatingNavBar(selectedTab: Int, isDark: Boolean, onTabSelected: (Int) -> Unit) {
    val haptic = LocalHapticFeedback.current
    val navBg = if (isDark) Color(0xFF1E1E1E).copy(alpha = 0.85f) else Color.White.copy(alpha = 0.95f)
    val navBorder = if (isDark) Color.White.copy(alpha = 0.1f) else Color.Black.copy(alpha = 0.05f)
    val shadowColor = if (isDark) Color.Black.copy(alpha = 0.5f) else Color.Black.copy(alpha = 0.15f)

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .shadow(
                elevation = 24.dp,
                shape = RoundedCornerShape(32.dp),
                spotColor = shadowColor,
                ambientColor = shadowColor
            )
            .clip(RoundedCornerShape(32.dp))
            .background(navBg)
            .border(1.dp, navBorder, RoundedCornerShape(32.dp))
            .padding(horizontal = 12.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically
    ) {
        CRNavButton(
            icon = Icons.Default.Home,
            label = "Dashboard",
            isSelected = selectedTab == 0,
            isDark = isDark,
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onTabSelected(0)
            }
        )
        CRNavButton(
            icon = Icons.AutoMirrored.Filled.List,
            label = "Activity",
            isSelected = selectedTab == 1,
            isDark = isDark,
            onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onTabSelected(1)
            }
        )
    }
}

@Composable
fun CRNavButton(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    isSelected: Boolean,
    isDark: Boolean,
    onClick: () -> Unit
) {
    val tint by animateColorAsState(
        targetValue = if (isSelected) Color.White else CRTheme.inkSubtle(isDark), 
        animationSpec = tween(300, easing = FastOutSlowInEasing),
        label = "icon_color"
    )
    val bgAlpha by animateFloatAsState(
        targetValue = if (isSelected) 1f else 0f, 
        animationSpec = tween(300, easing = FastOutSlowInEasing),
        label = "bg_alpha"
    )
    
    // Smooth width animation for the pill effect
    val pillBg = CRTheme.brandElectric.copy(alpha = bgAlpha)
    
    Row(
        modifier = Modifier
            .height(52.dp)
            .clip(RoundedCornerShape(26.dp))
            .background(pillBg)
            .clickable(
                interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() },
                indication = null,
                onClick = onClick
            )
            .padding(horizontal = if (isSelected) 24.dp else 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.Center
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = tint,
            modifier = Modifier.size(20.dp)
        )
        AnimatedVisibility(visible = isSelected) {
            Row {
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = label,
                    fontSize = 14.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = Color.White
                )
            }
        }
    }
}

@Composable
fun DashboardTab(
    isDark: Boolean,
    isServiceRunning: Boolean,
    isSyncEnabled: Boolean,
    peers: List<PeerSnapshot>,
    ambientStatus: String,
    onTrustPeer: (PeerSnapshot) -> Unit,
    onRejectPeer: (PeerSnapshot) -> Unit,
    onActionPushClipboard: () -> Unit,
    onActionPairMagicLink: () -> Unit,
    onActionPauseSync: () -> Unit,
    onActionDisconnectAll: () -> Unit,
    onActionStopService: () -> Unit,
    onOpenSettings: () -> Unit,
    onStartSync: () -> Unit,
    onResumeSync: () -> Unit,
    onScanNow: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    LazyColumn(
        modifier = Modifier.fillMaxSize().systemBarsPadding(),
        contentPadding = PaddingValues(top = 24.dp, start = 24.dp, end = 24.dp, bottom = 140.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp)
    ) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text(
                        text = "ClipRelay",
                        fontSize = 28.sp,
                        fontWeight = FontWeight.Bold,
                        color = CRTheme.ink(isDark),
                        letterSpacing = (-0.5).sp
                    )
                    Text(
                        text = ambientStatus,
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Medium,
                        color = CRTheme.inkSubtle(isDark)
                    )
                }
                Box(
                    modifier = Modifier
                        .size(44.dp)
                        .clip(CircleShape)
                        .background(if (isDark) Color.White.copy(alpha = 0.05f) else Color.Black.copy(alpha = 0.03f))
                        .clickable { onOpenSettings() },
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Settings,
                        contentDescription = "Settings",
                        tint = CRTheme.ink(isDark),
                        modifier = Modifier.size(22.dp)
                    )
                }
            }
            Spacer(modifier = Modifier.height(16.dp))
        }

        if (peers.isEmpty()) {
            item {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .crCard(isDark, cornerRadius = 28.dp)
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(36.dp),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        val infiniteTransition = rememberInfiniteTransition()
                        val pulseScale by infiniteTransition.animateFloat(
                            initialValue = 0.95f, targetValue = 1.1f,
                            animationSpec = infiniteRepeatable(animation = tween(1500, easing = FastOutSlowInEasing), repeatMode = RepeatMode.Reverse),
                            label = "pulse_scale"
                        )
                        val glowAlpha by infiniteTransition.animateFloat(
                            initialValue = 0.1f, targetValue = 0.3f,
                            animationSpec = infiniteRepeatable(animation = tween(1500, easing = FastOutSlowInEasing), repeatMode = RepeatMode.Reverse),
                            label = "glow_alpha"
                        )

                        Box(contentAlignment = Alignment.Center) {
                            // Outer Glow
                            Box(
                                modifier = Modifier
                                    .size(90.dp)
                                    .scale(pulseScale)
                                    .clip(CircleShape)
                                    .background(CRTheme.brandElectric.copy(alpha = glowAlpha))
                            )
                            // Inner Icon
                            Box(
                                modifier = Modifier
                                    .size(64.dp)
                                    .clip(CircleShape)
                                    .background(CRTheme.brandElectric.copy(alpha = 0.2f))
                                    .border(1.dp, CRTheme.brandElectric.copy(alpha = 0.4f), CircleShape),
                                contentAlignment = Alignment.Center
                            ) {
                                Icon(
                                    imageVector = Icons.Default.Home,
                                    contentDescription = null,
                                    tint = CRTheme.brandElectric,
                                    modifier = Modifier.size(32.dp)
                                )
                            }
                        }
                        
                        Spacer(modifier = Modifier.height(24.dp))
                        Text(
                            text = if (!isServiceRunning) "Sync is stopped" else "Looking for devices",
                            fontSize = 22.sp,
                            fontWeight = FontWeight.Bold,
                            color = CRTheme.ink(isDark),
                            letterSpacing = (-0.5).sp
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = if (!isServiceRunning) "Start the service to discover devices seamlessly." else "Ensure you are on the same Wi-Fi network.",
                            fontSize = 15.sp,
                            color = CRTheme.inkSoft(isDark),
                            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                            lineHeight = 22.sp
                        )
                        Spacer(modifier = Modifier.height(32.dp))
                        
                        Button(
                            onClick = {
                                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                                when {
                                    !isServiceRunning -> onStartSync()
                                    !isSyncEnabled -> onResumeSync()
                                    else -> onScanNow()
                                }
                            },
                            colors = ButtonDefaults.buttonColors(containerColor = CRTheme.brandElectric),
                            shape = RoundedCornerShape(16.dp),
                            contentPadding = PaddingValues(horizontal = 32.dp, vertical = 16.dp)
                        ) {
                            Text(
                                text = when {
                                    !isServiceRunning -> "Start Syncing"
                                    !isSyncEnabled -> "Resume Sync"
                                    else -> "Scan Nearby"
                                },
                                color = Color.White,
                                fontSize = 16.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                }
            }
        } else {
            items(peers) { peer ->
                PeerRow(isDark = isDark, peer = peer, onTrust = { onTrustPeer(peer) }, onReject = { onRejectPeer(peer) })
            }
        }

        item {
            Spacer(modifier = Modifier.height(24.dp))
            CRSectionHeader(isDark, "QUICK ACTIONS")
        }

        item {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .crCard(isDark, cornerRadius = 24.dp)
            ) {
                Column(modifier = Modifier.animateContentSize(spring(stiffness = Spring.StiffnessLow))) {
                    if (peers.any { it.isConnected } && isSyncEnabled) {
                        ActionRow(isDark, "Send clipboard to Mac", CRTheme.brandElectric, isTop = true) {
                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                            onActionPushClipboard()
                        }
                        HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)
                    }
                    ActionRow(isDark, "Pair via Magic Link", CRTheme.brandElectric, isTop = !peers.any { it.isConnected } || !isSyncEnabled) {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onActionPairMagicLink()
                    }
                    HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)
                    ActionRow(isDark, if (isSyncEnabled) "Pause sync" else "Resume sync", CRTheme.ink(isDark)) {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onActionPauseSync()
                    }
                    
                    if (peers.any { it.isConnected }) {
                        HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)
                        ActionRow(isDark, "Disconnect all", CRTheme.accentRed) {
                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                            onActionDisconnectAll()
                        }
                    }
                    if (isServiceRunning) {
                        HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)
                        ActionRow(isDark, "Stop service", CRTheme.accentRed, isBottom = true) {
                            haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                            onActionStopService()
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun CRSectionHeader(isDark: Boolean, title: String) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 8.dp)) {
        Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(CRTheme.brandElectric))
        Spacer(modifier = Modifier.width(8.dp))
        Text(text = title, fontSize = 13.sp, fontWeight = FontWeight.ExtraBold, color = CRTheme.inkSubtle(isDark), letterSpacing = 1.sp)
    }
}

@Composable
fun PeerRow(isDark: Boolean, peer: PeerSnapshot, onTrust: () -> Unit, onReject: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .crCard(
                isDark = isDark, 
                cornerRadius = 24.dp, 
                highlighted = peer.isConnected,
                onClick = { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove) }
            )
    ) {
        Row(
            modifier = Modifier.padding(20.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(48.dp)
                    .clip(CircleShape)
                    .background(Brush.linearGradient(listOf(CRTheme.brandElectric.copy(alpha = 0.15f), CRTheme.brandViolet.copy(alpha = 0.15f))))
                    .border(1.dp, CRTheme.brandElectric.copy(alpha = 0.2f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = peer.name.take(1).uppercase(),
                    color = CRTheme.brandElectric,
                    fontWeight = FontWeight.Bold,
                    fontSize = 20.sp
                )
            }
            Spacer(modifier = Modifier.width(16.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(text = peer.name, fontSize = 17.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDark))
                Text(
                    text = peer.status,
                    fontSize = 14.sp,
                    color = if (peer.isConnected) CRTheme.brandElectric else CRTheme.inkSoft(isDark),
                    fontWeight = if (peer.isConnected) FontWeight.Medium else FontWeight.Normal
                )
            }
            if (peer.status == "needs_trust" || peer.status == "rejected" || !peer.trusted) {
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onTrust()
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.brandElectric),
                    shape = RoundedCornerShape(12.dp),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)
                ) {
                    Text("Trust", fontSize = 14.sp, fontWeight = FontWeight.Bold)
                }
            }
        }
    }
}

@Composable
fun ActionRow(isDark: Boolean, label: String, color: Color, isTop: Boolean = false, isBottom: Boolean = false, onClick: () -> Unit) {
    val shape = RoundedCornerShape(
        topStart = if (isTop) 24.dp else 0.dp, topEnd = if (isTop) 24.dp else 0.dp,
        bottomStart = if (isBottom) 24.dp else 0.dp, bottomEnd = if (isBottom) 24.dp else 0.dp
    )
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(text = label, color = color, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
fun ActivityFeedTab(isDark: Boolean, feed: List<ActivityEntry>, onApplyClipboard: (ActivityEntry) -> Unit) {
    Column(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
        Text(
            text = "Activity",
            fontSize = 28.sp,
            fontWeight = FontWeight.Bold,
            color = CRTheme.ink(isDark),
            letterSpacing = (-0.5).sp,
            modifier = Modifier.padding(start = 24.dp, top = 24.dp, bottom = 8.dp)
        )

        if (feed.isEmpty()) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("No recent activity", fontSize = 16.sp, fontWeight = FontWeight.Medium, color = CRTheme.inkSoft(isDark))
            }
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(top = 16.dp, start = 24.dp, end = 24.dp, bottom = 140.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp)
            ) {
                itemsIndexed(feed) { index, entry ->
                    // Staggered entrance animation
                    var isVisible by remember { mutableStateOf(false) }
                    LaunchedEffect(Unit) {
                        delay(index * 50L)
                        isVisible = true
                    }
                    AnimatedVisibility(
                        visible = isVisible,
                        enter = slideInVertically(initialOffsetY = { 50 }, animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy)) + fadeIn()
                    ) {
                        ActivityFeedRow(isDark = isDark, entry = entry, onApplyClipboard = { onApplyClipboard(entry) })
                    }
                }
            }
        }
    }
}

@Composable
fun ActivityFeedRow(isDark: Boolean, entry: ActivityEntry, onApplyClipboard: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .crCard(isDark, cornerRadius = 20.dp)
    ) {
        Column(modifier = Modifier.padding(20.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(modifier = Modifier.size(8.dp).clip(CircleShape).background(if (entry.kind == ActivityKind.CLIPBOARD_TEXT) CRTheme.brandViolet else CRTheme.brandCyan))
                Spacer(modifier = Modifier.width(8.dp))
                Text(text = entry.deviceName, fontSize = 14.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDark))
                Spacer(modifier = Modifier.weight(1f))
                Text(
                    text = "Just now", 
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Medium,
                    color = CRTheme.inkSubtle(isDark)
                )
            }
            Spacer(modifier = Modifier.height(12.dp))
            Text(text = entry.preview, fontSize = 15.sp, color = CRTheme.inkSoft(isDark), lineHeight = 22.sp)
            
            if (entry.kind == ActivityKind.CLIPBOARD_TEXT) {
                Spacer(modifier = Modifier.height(16.dp))
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onApplyClipboard()
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = if (isDark) Color.White.copy(alpha = 0.1f) else Color.Black.copy(alpha = 0.05f)),
                    shape = RoundedCornerShape(12.dp),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                    modifier = Modifier.height(36.dp)
                ) {
                    Text("Copy", fontSize = 14.sp, color = CRTheme.brandElectric, fontWeight = FontWeight.Bold)
                }
            }
        }
    }
}
