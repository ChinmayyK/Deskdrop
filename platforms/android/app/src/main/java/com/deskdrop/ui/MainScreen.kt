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
import com.deskdrop.ui.theme.CRTheme
import com.deskdrop.ui.theme.SectionHeader
import com.deskdrop.ui.theme.crCard
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
    onActionStreamCamera: () -> Unit,
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
                        onScanNow = onScanNow,
                        onActionStreamCamera = onActionStreamCamera
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
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .crCard(isDark = isDark, cornerRadius = 0.dp)
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
    val tint = if (isSelected) CRTheme.bg(isDark) else CRTheme.textMedium(isDark)
    val pillBg = if (isSelected) CRTheme.textHigh(isDark) else Color.Transparent
    
    Row(
        modifier = Modifier
            .height(48.dp)
            .background(pillBg)
            .border(if (isSelected) 1.dp else 0.dp, if (isSelected) CRTheme.stroke(isDark) else Color.Transparent)
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
        if (isSelected) {
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                text = label.uppercase(),
                fontSize = 12.sp,
                fontWeight = FontWeight.Medium,
                color = tint,
                letterSpacing = 1.sp
            )
        }
    }
}

@Composable
fun PlatformBadge(name: String, isDark: Boolean) {
    val clean = name.lowercase()
    val label = when {
        clean.contains("mac") || clean.contains("apple") || clean.contains("macbook") || clean.contains("imac") -> "macOS"
        clean.contains("win") || clean.contains("pc") || clean.contains("windows") -> "Windows"
        clean.contains("linux") || clean.contains("ubuntu") -> "Linux"
        else -> "Android"
    }
    
    Text(
        text = label.uppercase(),
        fontSize = 10.sp,
        fontWeight = FontWeight.Medium,
        color = CRTheme.textLow(isDark),
        letterSpacing = 1.sp
    )
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
    onScanNow: () -> Unit,
    onActionStreamCamera: () -> Unit
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
                        fontWeight = FontWeight.Light,
                        color = CRTheme.textHigh(isDark),
                        letterSpacing = (-1).sp
                    )
                    Text(
                        text = ambientStatus,
                        fontSize = 14.sp,
                        fontWeight = FontWeight.Normal,
                        color = CRTheme.textMedium(isDark)
                    )
                }
                Box(
                    modifier = Modifier
                        .size(48.dp)
                        .border(0.5.dp, CRTheme.stroke(isDark))
                        .clickable { onOpenSettings() },
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Settings,
                        contentDescription = "Settings",
                        tint = CRTheme.textHigh(isDark),
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
            Spacer(modifier = Modifier.height(8.dp))
            
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    if (connectedPeers.isNotEmpty() && isSyncEnabled) {
                        ActionCard(
                            isDark = isDark,
                            title = "Send Clipboard",
                            icon = Icons.Default.Send,
                            modifier = Modifier.weight(1f),
                            onClick = onActionPushClipboard
                        )
                    } else {
                        ActionCard(
                            isDark = isDark,
                            title = "Pair Device",
                            icon = Icons.Default.Add,
                            modifier = Modifier.weight(1f),
                            onClick = onActionPairMagicLink
                        )
                    }
                    
                    val syncIcon = if (isSyncEnabled) Icons.Default.Clear else Icons.Default.PlayArrow
                    val syncLabel = if (isSyncEnabled) "Pause Sync" else "Resume Sync"
                    ActionCard(
                        isDark = isDark,
                        title = syncLabel,
                        icon = syncIcon,
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
                                  modifier = Modifier.weight(1f),
                                  onClick = onActionDisconnectAll
                            )
                            ActionCard(
                                isDark = isDark,
                                title = "Stream Camera",
                                icon = Icons.Default.PlayArrow,
                                modifier = Modifier.weight(1f),
                                onClick = onActionStreamCamera
                            )
                        }
                        ActionCard(
                            isDark = isDark,
                            title = "Stop Service",
                            icon = Icons.Default.ExitToApp,
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
            .crCard(isDark, cornerRadius = 0.dp)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(40.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Box(contentAlignment = Alignment.Center, modifier = Modifier.padding(vertical = 24.dp).size(120.dp)) {
                Box(
                    modifier = Modifier
                        .size(64.dp)
                        .border(1.dp, CRTheme.stroke(isDark), CircleShape),
                    contentAlignment = Alignment.Center
                ) {
                    Icon(
                        imageVector = Icons.Default.Home,
                        contentDescription = null,
                        tint = CRTheme.textHigh(isDark),
                        modifier = Modifier.size(24.dp)
                    )
                }
            }
            
            Text(
                text = if (!isServiceRunning) "SYNC STOPPED" else "LOOKING FOR DEVICES",
                fontSize = 12.sp,
                fontWeight = FontWeight.Medium,
                color = CRTheme.textMedium(isDark),
                letterSpacing = 1.sp
            )
            Spacer(modifier = Modifier.height(16.dp))
            Text(
                text = if (!isServiceRunning) "Start the service to discover devices seamlessly on your local network." else "Ensure you are on the same Wi-Fi network and Deskdrop is open on other devices.",
                fontSize = 14.sp,
                color = CRTheme.textLow(isDark),
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
                modifier = Modifier.fillMaxWidth().height(48.dp)
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(CRTheme.textHigh(isDark))
                        .border(1.dp, CRTheme.stroke(isDark)),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = when {
                            !isServiceRunning -> "START SYNCING"
                            !isSyncEnabled -> "RESUME SYNC"
                            else -> "SCAN NEARBY"
                        },
                        color = CRTheme.bg(isDark),
                        fontSize = 12.sp,
                        fontWeight = FontWeight.Medium,
                        letterSpacing = 1.sp
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
            .crCard(isDark, cornerRadius = 0.dp)
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(32.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(64.dp)
                    .border(1.dp, CRTheme.textMedium(isDark), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = Icons.Default.Home,
                    contentDescription = null,
                    tint = CRTheme.textHigh(isDark),
                    modifier = Modifier.size(24.dp)
                )
            }

            Spacer(modifier = Modifier.width(28.dp))

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "MESH ACTIVE",
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Medium,
                    color = CRTheme.textMedium(isDark),
                    letterSpacing = 1.5.sp
                )
                Spacer(modifier = Modifier.height(12.dp))
                
                Row(verticalAlignment = Alignment.Bottom) {
                    Text(
                        text = connectedPeers.size.toString(),
                        fontSize = 42.sp,
                        fontWeight = FontWeight.Light,
                        color = CRTheme.textHigh(isDark),
                        lineHeight = 42.sp
                    )
                    Spacer(modifier = Modifier.width(10.dp))
                    Text(
                        text = if (connectedPeers.size == 1) "Device\nConnected" else "Devices\nConnected",
                        fontSize = 12.sp,
                        fontWeight = FontWeight.Normal,
                        color = CRTheme.textLow(isDark),
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
                cornerRadius = 0.dp, 
                highlighted = peer.isConnected,
                accentColor = CRTheme.textHigh(isDark),
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
                    .background(CRTheme.surface(isDark))
                    .border(0.5.dp, CRTheme.stroke(isDark)),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = peer.name.take(1).uppercase(),
                    color = CRTheme.textHigh(isDark),
                    fontWeight = FontWeight.Light,
                    fontSize = 20.sp
                )
            }
            Spacer(modifier = Modifier.width(16.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(text = peer.name, fontSize = 16.sp, fontWeight = FontWeight.Medium, color = CRTheme.textHigh(isDark))
                    PlatformBadge(name = peer.name, isDark = isDark)
                }
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = peer.status.uppercase(),
                    fontSize = 10.sp,
                    color = CRTheme.textMedium(isDark),
                    fontWeight = FontWeight.Medium,
                    letterSpacing = 1.sp
                )
            }
            if (!peer.trusted && peer.status != "connected") {
                Button(
                    onClick = {
                        haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onTrust()
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = Color.Transparent),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                    modifier = Modifier.border(1.dp, CRTheme.textHigh(isDark))
                ) {
                    Text("TRUST", fontSize = 10.sp, fontWeight = FontWeight.Medium, color = CRTheme.textHigh(isDark), letterSpacing = 1.sp)
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
    modifier: Modifier = Modifier,
    onClick: () -> Unit
) {
    val haptic = LocalHapticFeedback.current
    Box(
        modifier = modifier
            .crCard(isDark, cornerRadius = 0.dp, highlighted = false, onClick = {
                haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                onClick()
            })
            .padding(20.dp)
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
            Box(
                modifier = Modifier
                    .size(40.dp)
                    .border(0.5.dp, CRTheme.stroke(isDark)),
                contentAlignment = Alignment.Center
            ) {
                Icon(imageVector = icon, contentDescription = title, tint = CRTheme.textHigh(isDark), modifier = Modifier.size(18.dp))
            }
            Spacer(modifier = Modifier.height(16.dp))
            Text(
                text = title.uppercase(),
                fontSize = 10.sp,
                fontWeight = FontWeight.Medium,
                color = CRTheme.textHigh(isDark),
                letterSpacing = 1.sp,
                textAlign = TextAlign.Center
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
fun ActivityCardContent(isDark: Boolean, entry: ActivityEntry, onApplyClipboard: () -> Unit) {
    val haptic = LocalHapticFeedback.current
    Column(modifier = Modifier.padding(20.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                modifier = Modifier
                    .size(24.dp)
                    .border(0.5.dp, CRTheme.stroke(isDark)),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = entry.deviceName.take(1).uppercase(),
                    color = CRTheme.textMedium(isDark),
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Medium
                )
            }
            Spacer(modifier = Modifier.width(10.dp))
            Text(text = entry.deviceName, fontSize = 14.sp, fontWeight = FontWeight.Medium, color = CRTheme.textHigh(isDark))
            Spacer(modifier = Modifier.width(8.dp))
            
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
            
            Text(text = tagLabel, fontSize = 9.sp, fontWeight = FontWeight.Medium, color = CRTheme.textMedium(isDark), letterSpacing = 1.sp)

            Spacer(modifier = Modifier.weight(1f))
            
            val timeString = android.text.format.DateFormat.format("hh:mm a", entry.timestamp).toString()
            Text(
                text = timeString, 
                fontSize = 11.sp,
                fontWeight = FontWeight.Normal,
                color = CRTheme.textLow(isDark)
            )
        }
        Spacer(modifier = Modifier.height(14.dp))

        when (entry.kind) {
            ActivityKind.CLIPBOARD_TEXT -> {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(CRTheme.surface(isDark))
                        .border(0.5.dp, CRTheme.stroke(isDark))
                        .padding(16.dp)
                ) {
                    Text(
                        text = entry.preview,
                        fontSize = 14.sp,
                        fontFamily = androidx.compose.ui.text.font.FontFamily.Monospace,
                        color = CRTheme.textHigh(isDark),
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
                    colors = ButtonDefaults.buttonColors(containerColor = Color.Transparent),
                    contentPadding = PaddingValues(horizontal = 20.dp, vertical = 0.dp),
                    modifier = Modifier.height(38.dp).border(1.dp, CRTheme.textHigh(isDark))
                ) {
                    Text("COPY TO CLIPBOARD", fontSize = 10.sp, color = CRTheme.textHigh(isDark), fontWeight = FontWeight.Medium, letterSpacing = 1.sp)
                }
            }
            ActivityKind.CLIPBOARD_IMAGE -> {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(120.dp)
                        .background(CRTheme.surface(isDark))
                        .border(0.5.dp, CRTheme.stroke(isDark)),
                    contentAlignment = Alignment.Center
                ) {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Icon(
                            imageVector = Icons.Default.Home,
                            contentDescription = null,
                            tint = CRTheme.textMedium(isDark),
                            modifier = Modifier.size(36.dp)
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = "IMAGE VIEWPORT ACTIVE",
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Medium,
                            color = CRTheme.textLow(isDark),
                            letterSpacing = 1.sp
                        )
                    }
                }
            }
            ActivityKind.FILE_TRANSFER_INCOMING, ActivityKind.FILE_TRANSFER_PROGRESS -> {
                Column(modifier = Modifier.fillMaxWidth()) {
                    Text(
                        text = entry.preview,
                        fontSize = 14.sp,
                        fontWeight = FontWeight.Medium,
                        color = CRTheme.textHigh(isDark)
                    )
                    Spacer(modifier = Modifier.height(10.dp))
                    
                    Box(modifier = Modifier.fillMaxWidth().height(2.dp).background(CRTheme.stroke(isDark))) {
                        Box(modifier = Modifier.fillMaxWidth(entry.progressPercent / 100f).height(2.dp).background(CRTheme.textHigh(isDark)))
                    }
                    
                    Spacer(modifier = Modifier.height(10.dp))
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "${entry.progressPercent}%",
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Medium,
                            color = CRTheme.textHigh(isDark),
                            letterSpacing = 1.sp
                        )
                        if (entry.transferBytesReceived > 0) {
                            Text(
                                text = "${formatSize(entry.transferBytesReceived)} / ${formatSize(entry.fileTotalBytes)}",
                                fontSize = 11.sp,
                                color = CRTheme.textLow(isDark),
                                fontWeight = FontWeight.Normal
                            )
                        }
                    }
                }
            }
            else -> {
                Text(
                    text = entry.preview, 
                    fontSize = 14.sp, 
                    color = CRTheme.textMedium(isDark), 
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
            fontWeight = FontWeight.Light,
            color = CRTheme.textHigh(isDark),
            letterSpacing = (-1).sp,
            modifier = Modifier.padding(start = 24.dp, top = 24.dp, bottom = 8.dp)
        )

        if (grouped.isEmpty()) {
            Box(modifier = Modifier.fillMaxSize().padding(bottom = 140.dp), contentAlignment = Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Filled.List,
                        contentDescription = null,
                        tint = CRTheme.textLow(isDark),
                        modifier = Modifier.size(48.dp)
                    )
                    Spacer(modifier = Modifier.height(16.dp))
                    Text("No recent activity", fontSize = 16.sp, fontWeight = FontWeight.Normal, color = CRTheme.textMedium(isDark))
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
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Medium,
                            color = CRTheme.textMedium(isDark),
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
                            enter = fadeIn()
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
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .crCard(isDark, cornerRadius = 0.dp)
    ) {
        ActivityCardContent(
            isDark = isDark,
            entry = entry,
            onApplyClipboard = onApplyClipboard
        )
    }
}
