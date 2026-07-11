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
fun ActivityTab(
    isDark: Boolean,
    feed: List<ActivityEntry>,
    onApplyClipboard: (ActivityEntry) -> Unit,
    onDeleteActivity: (ActivityEntry) -> Unit = {}
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
                        },
                        onDelete = { onDeleteActivity(entry) }
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
    onClick: () -> Unit,
    onDelete: () -> Unit = {}
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
            val isLink = entry.preview.startsWith("http")
            val previewText = when {
                entry.kind == ActivityKind.CLIPBOARD_TEXT && !isLink ->
                    "Copied text (${entry.preview.length} chars) • Protected"
                entry.kind == ActivityKind.CLIPBOARD_TEXT && isLink ->
                    "Link • ${entry.preview.take(45)}"
                else -> entry.preview.replace("\n", " ")
            }
            Text(
                text = previewText,
                style = CRTypography.bodyMedium,
                color = CRTheme.textMedium(isDark),
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                fontFamily = FontFamily.Monospace
            )
        }
        Spacer(modifier = Modifier.width(8.dp))
        IconButton(
            onClick = onDelete,
            modifier = Modifier.size(32.dp)
        ) {
            Icon(
                imageVector = Icons.Default.Delete,
                contentDescription = "Delete item",
                tint = CRTheme.textMedium(isDark).copy(alpha = 0.7f),
                modifier = Modifier.size(18.dp)
            )
        }
    }
}

