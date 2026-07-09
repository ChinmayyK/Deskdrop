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
    onOpenSettings: () -> Unit,
    onOpenDiagnostics: () -> Unit
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
            item { SettingsActionTile(isDark = isDark, icon = Icons.Default.Info, label = "Diagnostics", color = CRTheme.brandElectric, onClick = onOpenDiagnostics) }
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
            .semantics(mergeDescendants = true) {
                role = androidx.compose.ui.semantics.Role.Button
            }
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

