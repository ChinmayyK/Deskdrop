package com.deskdrop.ui

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
import androidx.compose.material.icons.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.ExitToApp
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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.deskdrop.ActivityEntry
import com.deskdrop.ActivityKind
import com.deskdrop.PeerSnapshot
import com.deskdrop.ui.theme.CRBackground
import com.deskdrop.ui.theme.CRMotion
import com.deskdrop.ui.theme.CRTheme
import com.deskdrop.ui.theme.GradientIcon
import com.deskdrop.ui.theme.SectionHeader
import com.deskdrop.ui.theme.StatusPulse
import com.deskdrop.ui.theme.RadarRipple
import com.deskdrop.ui.theme.crCard
import com.deskdrop.ui.theme.crCardElevated
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
                        slideInHorizontally(animationSpec = spring(stiffness = Spring.StiffnessMedium), initialOffsetX = { fullWidth -> fullWidth }) + fadeIn() togetherWith
                                slideOutHorizontally(animationSpec = spring(stiffness = Spring.StiffnessMedium), targetOffsetX = { fullWidth -> -fullWidth }) + fadeOut()
                    } else {
                        slideInHorizontally(animationSpec = spring(stiffness = Spring.StiffnessMedium), initialOffsetX = { fullWidth -> -fullWidth }) + fadeIn() togetherWith
                                slideOutHorizontally(animationSpec = spring(stiffness = Spring.StiffnessMedium), targetOffsetX = { fullWidth -> fullWidth }) + fadeOut()
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
    val navBg = if (isDark) Color(0xFF0F0F10).copy(alpha = 0.85f) else Color.White.copy(alpha = 0.95f)
    val navBorder = CRTheme.divider(isDark)
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
    
    val pillBg = CRTheme.brandElectric.copy(alpha = bgAlpha)
    
    Row(
        modifier = Modifier
            .height(52.dp)
            .clip(RoundedCornerShape(26.dp))
            .background(pillBg)
            .clickable(onClick = onClick)
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
fun PlatformBadge(name: String, isDark: Boolean) {
    val clean = name.lowercase()
    val (label, color) = when {
        clean.contains("mac") || clean.contains("apple") || clean.contains("macbook") || clean.contains("imac") -> "macOS" to Color(0xFF8E8E93)
        clean.contains("win") || clean.contains("pc") || clean.contains("windows") -> "Windows" to Color(0xFF00A4EF)
        clean.contains("linux") || clean.contains("ubuntu") -> "Linux" to Color(0xFFFCC624)
        else -> "Android" to CRTheme.accentGreen
    }
    
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(8.dp))
            .background(color.copy(alpha = 0.12f))
            .border(0.5.dp, color.copy(alpha = 0.3f), RoundedCornerShape(8.dp))
            .padding(horizontal = 8.dp, vertical = 2.dp)
    ) {
        Text(
            text = label.uppercase(),
            fontSize = 9.sp,
            fontWeight = FontWeight.ExtraBold,
            color = color,
            letterSpacing = 0.5.sp
        )
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
    val connectedPeers = peers.filter { it.isConnected }
    
    LazyColumn(
        modifier = Modifier.fillMaxSize().systemBarsPadding(),
        contentPadding = PaddingValues(top = 24.dp, start = 24.dp, end = 24.dp, bottom = 140.dp),
        verticalArrangement = Arrangement.spacedBy(24.dp)
    ) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text(
                        text = "Deskdrop",
                        fontSize = 32.sp,
                        fontWeight = FontWeight.Black,
                        color = CRTheme.ink(isDark),
                        letterSpacing = (-1).sp
                    )
                    Text(
                        text = ambientStatus,
                        fontSize = 14.sp,
                        fontWeight = FontWeight.Medium,
                        color = CRTheme.inkSoft(isDark)
                    )
                }
                Box(
                    modifier = Modifier
                        .size(48.dp)
                        .clip(CircleShape)
                        .background(CRTheme.surface(isDark))
                        .clickable { onOpenSettings() },
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Settings,
                        contentDescription = "Settings",
                        tint = CRTheme.ink(isDark),
                        modifier = Modifier.size(24.dp)
                    )
                }
            }
        }

        if (peers.isEmpty()) {
            item {
                HeroEmptyState(
                    isDark = isDark,
                    isServiceRunning = isServiceRunning,
                    isSyncEnabled = isSyncEnabled,
                    onStartSync = onStartSync,
                    onResumeSync = onResumeSync,
                    onScanNow = onScanNow
                )
            }
        } else {
            item {
                HeroConnectedState(isDark = isDark, connectedPeers = connectedPeers)
            }

            items(peers) { peer ->
                PeerRow(isDark = isDark, peer = peer, onTrust = { onTrustPeer(peer) }, onReject = { onRejectPeer(peer) })
            }
        }

        item {
            SectionHeader(isDark, "Quick Actions")
            Spacer(modifier = Modifier.height(16.dp))
            
            // 2x2 Grid of Actions
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    if (connectedPeers.isNotEmpty() && isSyncEnabled) {
                        ActionCard(
                            isDark = isDark,
                            title = "Send Clipboard",
                            icon = Icons.Default.Send,
                            brush = CRTheme.brandGradient,
                            accentColor = CRTheme.brandViolet,
                            modifier = Modifier.weight(1f),
                            onClick = onActionPushClipboard
                        )
                    } else {
                        ActionCard(
                            isDark = isDark,
                            title = "Pair Device",
                            icon = Icons.Default.Add,
                            brush = CRTheme.brandGradient,
                            accentColor = CRTheme.brandElectric,
                            modifier = Modifier.weight(1f),
                            onClick = onActionPairMagicLink
                        )
                    }
                    
                    val syncIcon = if (isSyncEnabled) Icons.Default.Clear else Icons.Default.PlayArrow
                    val syncLabel = if (isSyncEnabled) "Pause Sync" else "Resume Sync"
                    val syncColor = if (isSyncEnabled) CRTheme.accentGreen else CRTheme.inkSubtle(isDark)
                    ActionCard(
                        isDark = isDark,
                        title = syncLabel,
                        icon = syncIcon,
                        brush = Brush.horizontalGradient(listOf(syncColor, syncColor)),
                        accentColor = syncColor,
                        modifier = Modifier.weight(1f),
                        onClick = onActionPauseSync
                    )
                }
                
                if (isServiceRunning) {
                    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                        if (connectedPeers.isNotEmpty()) {
                            ActionCard(
                                  isDark = isDark,
                                  title = "Disconnect All",
                                  icon = Icons.Default.ExitToApp,
                                  brush = Brush.horizontalGradient(listOf(CRTheme.accentRed, CRTheme.brandPink)),
                                  accentColor = CRTheme.accentRed,
                                  modifier = Modifier.weight(1f),
                                  onClick = onActionDisconnectAll
                            )
                        }
                        ActionCard(
                            isDark = isDark,
                            title = "Stop Service",
                            icon = Icons.Default.ExitToApp,
                            brush = Brush.horizontalGradient(listOf(CRTheme.inkSubtle(isDark), CRTheme.inkSubtle(isDark))),
                            accentColor = CRTheme.inkSubtle(isDark),
                            modifier = Modifier.weight(1f),
                            onClick = onActionStopService
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun HeroEmptyState(
    isDark: Boolean,
    isServiceRunning: Boolean,
    isSyncEnabled: Boolean,
    onStartSync: () -> Unit,
    onResumeSync: () -> Unit,
    onScanNow: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .crCardElevated(isDark, cornerRadius = 32.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(40.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Box(contentAlignment = Alignment.Center, modifier = Modifier.padding(vertical = 24.dp).size(200.dp)) {
                // Expanding Radar custom canvas concentric waves
                RadarRipple(color = CRTheme.brandElectric)

                // Central Active Discovery Orb
                Box(
                    modifier = Modifier
                        .size(84.dp)
                        .clip(CircleShape)
                        .background(CRTheme.brandGradient)
                        .shadow(24.dp, CircleShape, spotColor = CRTheme.brandElectric),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Home,
                        contentDescription = null,
                        tint = Color.White,
                        modifier = Modifier.size(36.dp)
                    )
                }
            }
            
            Spacer(modifier = Modifier.height(24.dp))
            Text(
                text = if (!isServiceRunning) "Sync is stopped" else "Looking for devices",
                fontSize = 24.sp,
                fontWeight = FontWeight.Black,
                color = CRTheme.ink(isDark),
                letterSpacing = (-0.5).sp
            )
            Spacer(modifier = Modifier.height(12.dp))
            Text(
                text = if (!isServiceRunning) "Start the service to discover devices seamlessly on your local network." else "Ensure you are on the same Wi-Fi network and Deskdrop is open on other devices.",
                fontSize = 15.sp,
                color = CRTheme.inkSoft(isDark),
                textAlign = TextAlign.Center,
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
                colors = ButtonDefaults.buttonColors(containerColor = Color.Transparent),
                contentPadding = PaddingValues(0.dp),
                shape = RoundedCornerShape(16.dp),
                modifier = Modifier.fillMaxWidth().height(56.dp)
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(CRTheme.brandGradient)
                        .clip(RoundedCornerShape(16.dp)),
                    contentAlignment = Alignment.Center
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
}

@Composable
fun HeroConnectedState(isDark: Boolean, connectedPeers: List<PeerSnapshot>) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .crCardElevated(isDark, cornerRadius = 32.dp)
    ) {
        val infiniteTransition = rememberInfiniteTransition(label = "pulse_radar")
        val scalePulse by infiniteTransition.animateFloat(
            initialValue = 0.95f, targetValue = 1.15f,
            animationSpec = infiniteRepeatable(animation = tween(2200, easing = FastOutSlowInEasing), repeatMode = RepeatMode.Reverse),
            label = "scalePulse"
        )

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(32.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Visual Network Mapping Orb
            Box(
                modifier = Modifier
                    .size(96.dp)
                    .clip(CircleShape)
                    .background(Brush.radialGradient(listOf(CRTheme.brandElectric.copy(alpha = 0.2f), Color.Transparent)))
                    .border(1.dp, CRTheme.brandElectric.copy(alpha = 0.3f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                // Expanding Radar ring
                Box(
                    modifier = Modifier
                        .size(80.dp)
                        .scale(scalePulse)
                        .clip(CircleShape)
                        .border(1.dp, CRTheme.accentGreen.copy(alpha = 0.4f), CircleShape)
                )

                // Central Active Mesh Orb
                Box(
                    modifier = Modifier
                        .size(48.dp)
                        .clip(CircleShape)
                        .background(CRTheme.successGradient)
                        .shadow(16.dp, CircleShape, spotColor = CRTheme.accentGreen),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Home,
                        contentDescription = null,
                        tint = Color.White,
                        modifier = Modifier.size(24.dp)
                    )
                }
            }

            Spacer(modifier = Modifier.width(28.dp))

            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    StatusPulse(color = CRTheme.accentGreen)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = "MESH ACTIVE",
                        fontSize = 11.sp,
                        fontWeight = FontWeight.ExtraBold,
                        color = CRTheme.accentGreen,
                        letterSpacing = 1.5.sp
                    )
                }
                Spacer(modifier = Modifier.height(12.dp))
                
                Row(verticalAlignment = Alignment.Bottom) {
                    Text(
                        text = connectedPeers.size.toString(),
                        fontSize = 54.sp,
                        fontWeight = FontWeight.Black,
                        color = CRTheme.ink(isDark),
                        letterSpacing = (-2).sp,
                        lineHeight = 54.sp
                    )
                    Spacer(modifier = Modifier.width(10.dp))
                    Text(
                        text = if (connectedPeers.size == 1) "Device\nConnected" else "Devices\nConnected",
                        fontSize = 14.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = CRTheme.inkSoft(isDark),
                        modifier = Modifier.padding(bottom = 6.dp)
                    )
                }
            }
        }
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
                accentColor = if (peer.isConnected) CRTheme.accentGreen else CRTheme.brandElectric,
                onClick = { haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove) }
            )
    ) {
        Row(
            modifier = Modifier.padding(20.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(56.dp)
                    .clip(CircleShape)
                    .background(if (peer.isConnected) CRTheme.brandGradient else Brush.linearGradient(listOf(CRTheme.surfaceElevated(isDark), CRTheme.surfaceElevated(isDark))))
                    .border(1.dp, if (peer.isConnected) Color.Transparent else CRTheme.stroke(isDark), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = peer.name.take(1).uppercase(),
                    color = if (peer.isConnected) Color.White else CRTheme.ink(isDark),
                    fontWeight = FontWeight.Black,
                    fontSize = 22.sp
                )
            }
            Spacer(modifier = Modifier.width(16.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(text = peer.name, fontSize = 17.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDark))
                    PlatformBadge(name = peer.name, isDark = isDark)
                }
                Spacer(modifier = Modifier.height(4.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (peer.isConnected) {
                        StatusPulse(color = CRTheme.accentGreen, modifier = Modifier.size(8.dp))
                        Spacer(modifier = Modifier.width(6.dp))
                    }
                    Text(
                        text = peer.status,
                        fontSize = 13.sp,
                        color = if (peer.isConnected) CRTheme.accentGreen else CRTheme.inkSoft(isDark),
                        fontWeight = if (peer.isConnected) FontWeight.SemiBold else FontWeight.Medium
                    )
                }
            }
            if (!peer.trusted && peer.status != "connected") {
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
fun ActionCard(
    isDark: Boolean,
    title: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    brush: Brush,
    accentColor: Color,
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    Box(
        modifier = modifier
            .crCard(isDark, cornerRadius = 24.dp, highlighted = true, accentColor = accentColor, onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onClick()
            })
            .padding(20.dp)
    ) {
        Column {
            Box(
                modifier = Modifier
                    .size(46.dp)
                    .clip(CircleShape)
                    .background(accentColor.copy(alpha = 0.12f))
                    .border(0.5.dp, accentColor.copy(alpha = 0.3f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                GradientIcon(imageVector = icon, brush = brush, contentDescription = title, modifier = Modifier.size(22.dp))
            }
            Spacer(modifier = Modifier.height(20.dp))
            Text(
                text = title,
                fontSize = 15.sp,
                fontWeight = FontWeight.Bold,
                color = CRTheme.ink(isDark)
            )
        }
    }
}

data class GroupedActivity(val title: String, val items: List<ActivityEntry>)

fun groupActivities(feed: List<ActivityEntry>): List<GroupedActivity> {
    val now = System.currentTimeMillis()
    val dayMillis = 24 * 60 * 60 * 1000L
    
    val today = mutableListOf<ActivityEntry>()
    val yesterday = mutableListOf<ActivityEntry>()
    val earlier = mutableListOf<ActivityEntry>()
    
    feed.forEach { entry ->
        val diff = now - entry.timestamp
        when {
            diff < dayMillis -> today.add(entry)
            diff < 2 * dayMillis -> yesterday.add(entry)
            else -> earlier.add(entry)
        }
    }
    
    return buildList {
        if (today.isNotEmpty()) add(GroupedActivity("Today", today))
        if (yesterday.isNotEmpty()) add(GroupedActivity("Yesterday", yesterday))
        if (earlier.isNotEmpty()) add(GroupedActivity("Earlier", earlier))
    }
}

fun formatSize(bytes: Long): String {
    if (bytes <= 0) return "0 B"
    val units = arrayOf("B", "KB", "MB", "GB", "TB")
    val digitGroups = (Math.log10(bytes.toDouble()) / Math.log10(1024.0)).toInt()
    return String.format("%.1f %s", bytes / Math.pow(1024.0, digitGroups.toDouble()), units[digitGroups])
}

@Composable
fun ActivityCardContent(isDark: Boolean, entry: ActivityEntry, accentColor: Color, onApplyClipboard: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    Column(modifier = Modifier.padding(20.dp)) {
        // Top header: device and tag
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .size(24.dp)
                    .clip(CircleShape)
                    .background(accentColor.copy(alpha = 0.15f)),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = entry.deviceName.take(1).uppercase(),
                    color = accentColor,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Bold
                )
            }
            Spacer(modifier = Modifier.width(10.dp))
            Text(text = entry.deviceName, fontSize = 15.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDark))
            Spacer(modifier = Modifier.width(8.dp))
            
            // Premium Tag based on Kind
            val tagLabel = when (entry.kind) {
                ActivityKind.CLIPBOARD_TEXT -> "TEXT"
                ActivityKind.CLIPBOARD_IMAGE -> "IMAGE"
                ActivityKind.FILE_SENT -> "SENT"
                ActivityKind.FILE_RECEIVED -> "RECEIVED"
                ActivityKind.FILE_TRANSFER_INCOMING -> "INCOMING"
                ActivityKind.FILE_TRANSFER_PROGRESS -> "TRANSFERRING"
                ActivityKind.FILE_TRANSFER_COMPLETE -> "SUCCESS"
                ActivityKind.FILE_TRANSFER_FAILED -> "FAILED"
                else -> "SYSTEM"
            }
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .background(accentColor.copy(alpha = 0.1f))
                    .padding(horizontal = 6.dp, vertical = 2.dp)
            ) {
                Text(text = tagLabel, fontSize = 9.sp, fontWeight = FontWeight.ExtraBold, color = accentColor)
            }

            Spacer(modifier = Modifier.weight(1f))
            
            val timeString = android.text.format.DateFormat.format("hh:mm a", entry.timestamp).toString()
            Text(
                text = timeString, 
                fontSize = 12.sp,
                fontWeight = FontWeight.Medium,
                color = CRTheme.inkSubtle(isDark)
            )
        }
        Spacer(modifier = Modifier.height(14.dp))

        // Content Templates
        when (entry.kind) {
            ActivityKind.CLIPBOARD_TEXT -> {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(12.dp))
                        .background(if (isDark) Color.Black.copy(alpha = 0.3f) else Color.Black.copy(alpha = 0.03f))
                        .border(0.5.dp, CRTheme.stroke(isDark), RoundedCornerShape(12.dp))
                        .padding(16.dp)
                ) {
                    Text(
                        text = entry.preview,
                        fontSize = 14.sp,
                        fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                        color = CRTheme.inkSoft(isDark),
                        lineHeight = 20.sp,
                        maxLines = 4,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                Spacer(modifier = Modifier.height(14.dp))
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onApplyClipboard()
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.surfaceElevated(isDark)),
                    shape = RoundedCornerShape(12.dp),
                    contentPadding = PaddingValues(horizontal = 20.dp, vertical = 0.dp),
                    modifier = Modifier.height(38.dp)
                ) {
                    Text("Copy to clipboard", fontSize = 13.sp, color = CRTheme.ink(isDark), fontWeight = FontWeight.Bold)
                }
            }
            ActivityKind.CLIPBOARD_IMAGE -> {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(120.dp)
                        .clip(RoundedCornerShape(16.dp))
                        .background(if (isDark) Color.White.copy(alpha = 0.03f) else Color.Black.copy(alpha = 0.02f))
                        .border(0.5.dp, CRTheme.stroke(isDark), RoundedCornerShape(16.dp)),
                    contentAlignment = Alignment.Center
                ) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Icon(
                            imageVector = Icons.Default.Home,
                            contentDescription = null,
                            tint = accentColor.copy(alpha = 0.5f),
                            modifier = Modifier.size(36.dp)
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = "IMAGE VIEWPORT ACTIVE",
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold,
                            color = CRTheme.inkSoft(isDark)
                        )
                    }
                }
            }
            ActivityKind.FILE_TRANSFER_INCOMING, ActivityKind.FILE_TRANSFER_PROGRESS -> {
                Column(modifier = Modifier.fillMaxWidth()) {
                    Text(
                        text = entry.preview,
                        fontSize = 15.sp,
                        fontWeight = FontWeight.Bold,
                        color = CRTheme.ink(isDark)
                    )
                    Spacer(modifier = Modifier.height(10.dp))
                    
                    LinearProgressIndicator(
                        progress = { entry.progressPercent / 100f },
                        modifier = Modifier.fillMaxWidth().height(6.dp).clip(CircleShape),
                        color = accentColor,
                        trackColor = CRTheme.stroke(isDark)
                    )
                    
                    Spacer(modifier = Modifier.height(10.dp))
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "${entry.progressPercent}% complete",
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Bold,
                            color = accentColor
                        )
                        if (entry.transferBytesReceived > 0) {
                            Text(
                                text = "${formatSize(entry.transferBytesReceived)} / ${formatSize(entry.fileTotalBytes)}",
                                fontSize = 12.sp,
                                color = CRTheme.inkSoft(isDark),
                                fontWeight = FontWeight.Medium
                            )
                        }
                    }
                }
            }
            else -> {
                Text(
                    text = entry.preview, 
                    fontSize = 15.sp, 
                    color = CRTheme.inkSoft(isDark), 
                    lineHeight = 22.sp
                )
            }
        }
    }
}

@Composable
fun ActivityFeedTab(isDark: Boolean, feed: List<ActivityEntry>, onApplyClipboard: (ActivityEntry) -> Unit) {
    val grouped = groupActivities(feed)
    Column(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
        Text(
            text = "Activity",
            fontSize = 32.sp,
            fontWeight = FontWeight.Black,
            color = CRTheme.ink(isDark),
            letterSpacing = (-1).sp,
            modifier = Modifier.padding(start = 24.dp, top = 24.dp, bottom = 8.dp)
        )

        if (grouped.isEmpty()) {
            Box(modifier = Modifier.fillMaxSize().padding(bottom = 140.dp), contentAlignment = Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.List,
                        contentDescription = null,
                        tint = CRTheme.inkSubtle(isDark),
                        modifier = Modifier.size(48.dp)
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text("No recent activity", fontSize = 18.sp, fontWeight = FontWeight.SemiBold, color = CRTheme.inkSoft(isDark))
                }
            }
        } else {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(top = 8.dp, start = 24.dp, end = 24.dp, bottom = 140.dp),
                verticalArrangement = Arrangement.spacedBy(20.dp)
            ) {
                grouped.forEach { group ->
                    item {
                        Text(
                            text = group.title.uppercase(),
                            fontSize = 12.sp,
                            fontWeight = FontWeight.ExtraBold,
                            color = CRTheme.brandElectric,
                            letterSpacing = 1.sp,
                            modifier = Modifier.padding(vertical = 8.dp)
                        )
                    }
                    
                    itemsIndexed(group.items) { index, entry ->
                        var isVisible by remember { mutableStateOf(false) }
                        LaunchedEffect(Unit) {
                            delay(index * 40L)
                            isVisible = true
                        }
                        AnimatedVisibility(
                            visible = isVisible,
                            enter = slideInVertically(initialOffsetY = { 50 }, animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessLow)) + fadeIn()
                        ) {
                            ActivityFeedRow(isDark = isDark, entry = entry, onApplyClipboard = { onApplyClipboard(entry) })
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun ActivityFeedRow(isDark: Boolean, entry: ActivityEntry, onApplyClipboard: () -> Unit) {
    val accentColor = when (entry.kind) {
        ActivityKind.CLIPBOARD_TEXT -> CRTheme.brandViolet
        ActivityKind.CLIPBOARD_IMAGE -> CRTheme.brandPink
        ActivityKind.FILE_RECEIVED -> CRTheme.brandCyan
        else -> CRTheme.inkSubtle(isDark)
    }

    Row(modifier = Modifier.fillMaxWidth().height(IntrinsicSize.Min)) {
        // Vertical connecting path line
        Column(
            modifier = Modifier
                .fillMaxHeight()
                .width(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Box(
                modifier = Modifier
                    .size(10.dp)
                    .clip(CircleShape)
                    .background(accentColor)
                    .border(2.dp, Color.White.copy(alpha = 0.8f), CircleShape)
            )
            Box(
                modifier = Modifier
                    .width(2.dp)
                    .weight(1f)
                    .background(
                        Brush.verticalGradient(
                            listOf(accentColor.copy(alpha = 0.4f), Color.Transparent)
                        )
                    )
            )
        }
        
        Spacer(modifier = Modifier.width(8.dp))
        
        Box(
            modifier = Modifier
                .weight(1f)
                .crCard(isDark, cornerRadius = 24.dp)
        ) {
            ActivityCardContent(
                isDark = isDark,
                entry = entry,
                accentColor = accentColor,
                onApplyClipboard = onApplyClipboard
            )
        }
    }
}

