package com.cliprelay.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.cliprelay.ActivityEntry
import com.cliprelay.ActivityKind
import com.cliprelay.PeerSnapshot
import com.cliprelay.ui.theme.CRTheme
import com.cliprelay.ui.theme.crCard
import com.cliprelay.ui.theme.CRBackground

@Composable
fun MainScreen(
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
    val isDark = false // Force light mode as default

    CRBackground(isDark = isDark) {
        Column(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
            Box(modifier = Modifier.weight(1f)) {
                if (selectedTab == 0) {
                    DashboardTab(
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
                } else {
                    ActivityFeedTab(
                        isDark = isDark,
                        feed = feed,
                        onApplyClipboard = onApplyClipboard
                    )
                }
            }

            // Custom Navigation Bar
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(CRTheme.canvasTop(isDark).copy(alpha = 0.8f))
                    .padding(vertical = 12.dp, horizontal = 24.dp),
                horizontalArrangement = Arrangement.SpaceEvenly,
                verticalAlignment = Alignment.CenterVertically
            ) {
                CRNavButton(
                    icon = Icons.Default.Home,
                    label = "Dashboard",
                    isSelected = selectedTab == 0,
                    isDark = isDark,
                    onClick = { selectedTab = 0 }
                )
                CRNavButton(
                    icon = Icons.AutoMirrored.Filled.List,
                    label = "Activity",
                    isSelected = selectedTab == 1,
                    isDark = isDark,
                    onClick = { selectedTab = 1 }
                )
            }
        }
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
    val tint = if (isSelected) CRTheme.brandElectric else CRTheme.inkSoft(isDark)
    val bg = if (isSelected) (if (isDark) Color(0xFFFFFFFF).copy(alpha = 0.12f) else Color(0xFFFFFFFF).copy(alpha = 0.95f)) else Color.Transparent
    val stroke = if (isSelected) (if (isDark) Color(0xFFFFFFFF).copy(alpha = 0.15f) else Color(0xFFE2E8F0).copy(alpha = 0.9f)) else Color.Transparent

    Row(
        modifier = Modifier
            .clip(RoundedCornerShape(8.dp))
            .background(bg)
            .border(0.5.dp, stroke, RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.Center
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = tint,
            modifier = Modifier.size(18.dp)
        )
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = label,
            fontSize = 14.sp,
            fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Medium,
            color = if (isSelected) CRTheme.ink(isDark) else CRTheme.inkSoft(isDark)
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
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(top = 48.dp, start = 20.dp, end = 20.dp, bottom = 120.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = ambientStatus,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Medium,
                    color = CRTheme.inkSubtle(isDark),
                    letterSpacing = 0.5.sp
                )
                TextButton(onClick = onOpenSettings) {
                    Text("Settings", color = CRTheme.inkSoft(isDark), fontSize = 14.sp)
                }
            }
            Spacer(modifier = Modifier.height(16.dp))
        }

        if (peers.isEmpty()) {
            item {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .crCard(isDark, cornerRadius = 16.dp)
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(32.dp),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Box(
                            modifier = Modifier
                                .size(56.dp)
                                .clip(CircleShape)
                                .background(CRTheme.brandElectric.copy(alpha = 0.1f))
                                .border(0.5.dp, CRTheme.brandElectric.copy(alpha = 0.15f), CircleShape),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(
                                imageVector = Icons.Default.Home, // Replace with appropriate icon if needed, or use a custom mark
                                contentDescription = null,
                                tint = CRTheme.brandElectric,
                                modifier = Modifier.size(28.dp)
                            )
                        }
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = if (!isServiceRunning) "Sync is stopped" else "Looking for devices...",
                            fontSize = 20.sp,
                            fontWeight = FontWeight.Bold,
                            color = CRTheme.ink(isDark),
                            letterSpacing = (-0.5).sp
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = if (!isServiceRunning) "Start the service to discover devices." else "Stay on the same Wi-Fi network.",
                            fontSize = 14.5f.sp,
                            color = CRTheme.inkSoft(isDark),
                            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                            lineHeight = 20.sp
                        )
                        Spacer(modifier = Modifier.height(28.dp))
                        Button(
                            onClick = {
                                when {
                                    !isServiceRunning -> onStartSync()
                                    !isSyncEnabled -> onResumeSync()
                                    else -> onScanNow()
                                }
                            },
                            colors = ButtonDefaults.buttonColors(containerColor = CRTheme.brandElectric),
                            shape = RoundedCornerShape(10.dp),
                            contentPadding = PaddingValues(horizontal = 24.dp, vertical = 14.dp)
                        ) {
                            Text(
                                text = when {
                                    !isServiceRunning -> "Start Sync"
                                    !isSyncEnabled -> "Resume Sync"
                                    else -> "Scan Nearby"
                                },
                                color = Color.White,
                                fontSize = 15.sp,
                                fontWeight = FontWeight.SemiBold
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
            Spacer(modifier = Modifier.height(32.dp))
            CRSectionHeader(isDark, "ACTIONS", "Quick Shortcuts")
        }

        item {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .crCard(isDark, cornerRadius = 16.dp)
            ) {
                Column {
                    if (peers.any { it.isConnected } && isSyncEnabled) {
                        ActionRow(isDark, "Send clipboard to Mac", CRTheme.brandElectric, onActionPushClipboard)
                        HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)
                    }
                    ActionRow(isDark, "Pair via Magic Link", CRTheme.brandElectric, onActionPairMagicLink)
                    HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)
                    ActionRow(isDark, if (isSyncEnabled) "Pause sync" else "Resume sync", CRTheme.ink(isDark), onActionPauseSync)
                    
                    if (peers.any { it.isConnected }) {
                        HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)
                        ActionRow(isDark, "Disconnect all", CRTheme.accentRed, onActionDisconnectAll)
                    }
                    if (isServiceRunning) {
                        HorizontalDivider(color = CRTheme.stroke(isDark), thickness = 0.5.dp)
                        ActionRow(isDark, "Stop service", CRTheme.accentRed, onActionStopService)
                    }
                }
            }
        }
    }
}

@Composable
fun CRSectionHeader(isDark: Boolean, eyebrow: String, title: String) {
    VStack(spacing = 4.dp) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(modifier = Modifier.width(3.dp).height(12.dp).clip(CircleShape).background(CRTheme.brandElectric))
            Spacer(modifier = Modifier.width(6.dp))
            Text(text = eyebrow, fontSize = 11.sp, fontWeight = FontWeight.Bold, color = CRTheme.brandElectric, letterSpacing = 1.sp)
        }
        Text(text = title, fontSize = 24.sp, fontWeight = FontWeight.Bold, color = CRTheme.ink(isDark), letterSpacing = (-0.4).sp)
    }
}

@Composable
fun VStack(spacing: androidx.compose.ui.unit.Dp = 0.dp, content: @Composable ColumnScope.() -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(spacing)) {
        content()
    }
}

@Composable
fun PeerRow(isDark: Boolean, peer: PeerSnapshot, onTrust: () -> Unit, onReject: () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .crCard(isDark, cornerRadius = 16.dp)
            .clickable { /* Handle peer click */ }
    ) {
        Row(
            modifier = Modifier.padding(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(42.dp)
                    .clip(CircleShape)
                    .background(CRTheme.brandElectric.copy(alpha = 0.12f))
                    .border(0.5.dp, CRTheme.brandElectric.copy(alpha = 0.16f), CircleShape),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = peer.name.take(1).uppercase(),
                    color = CRTheme.brandElectric,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 18.sp
                )
            }
            Spacer(modifier = Modifier.width(16.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(text = peer.name, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = CRTheme.ink(isDark))
                Text(
                    text = peer.status,
                    fontSize = 13.sp,
                    color = CRTheme.inkSoft(isDark)
                )
            }
            if (peer.status == "needs_trust" || peer.status == "rejected" || !peer.trusted) {
                Button(
                    onClick = onTrust,
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.brandElectric),
                    shape = RoundedCornerShape(8.dp),
                    contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp),
                    modifier = Modifier.height(32.dp)
                ) {
                    Text("Trust", fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                }
            }
        }
    }
}

@Composable
fun ActionRow(isDark: Boolean, label: String, color: Color, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(text = label, color = color, fontSize = 15.sp, fontWeight = FontWeight.Medium)
    }
}

@Composable
fun ActivityFeedTab(isDark: Boolean, feed: List<ActivityEntry>, onApplyClipboard: (ActivityEntry) -> Unit) {
    if (feed.isEmpty()) {
        Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("No activity yet", fontSize = 15.sp, color = CRTheme.inkSoft(isDark))
        }
    } else {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(top = 48.dp, start = 20.dp, end = 20.dp, bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            item {
                CRSectionHeader(isDark, "FEED", "Recent Activity")
                Spacer(modifier = Modifier.height(8.dp))
            }
            items(feed) { entry ->
                ActivityFeedRow(isDark = isDark, entry = entry, onApplyClipboard = { onApplyClipboard(entry) })
            }
        }
    }
}

@Composable
fun ActivityFeedRow(isDark: Boolean, entry: ActivityEntry, onApplyClipboard: () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .crCard(isDark, cornerRadius = 16.dp)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(text = entry.deviceName, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = CRTheme.ink(isDark))
                Spacer(modifier = Modifier.weight(1f))
                Text(
                    text = "Just now", 
                    fontSize = 11.sp,
                    color = CRTheme.inkSubtle(isDark)
                )
            }
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = entry.preview, fontSize = 14.sp, color = CRTheme.inkSoft(isDark), lineHeight = 20.sp)
            
            if (entry.kind == ActivityKind.CLIPBOARD_TEXT) {
                Spacer(modifier = Modifier.height(12.dp))
                Button(
                    onClick = onApplyClipboard,
                    colors = ButtonDefaults.buttonColors(containerColor = CRTheme.surfaceStrongDark.copy(alpha = 0.1f)),
                    shape = RoundedCornerShape(8.dp),
                    contentPadding = PaddingValues(horizontal = 14.dp, vertical = 6.dp),
                    modifier = Modifier.height(34.dp)
                ) {
                    Text("Apply to Clipboard", fontSize = 13.sp, color = CRTheme.brandElectric, fontWeight = FontWeight.SemiBold)
                }
            }
        }
    }
}
