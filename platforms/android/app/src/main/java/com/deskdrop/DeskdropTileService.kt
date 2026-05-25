package com.deskdrop

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.annotation.ColorRes
import androidx.core.content.ContextCompat
import kotlin.math.roundToInt
import android.webkit.MimeTypeMap
import android.provider.OpenableColumns
import android.widget.ProgressBar
import android.content.res.ColorStateList
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.deskdrop.ui.theme.AppTheme
import com.deskdrop.ui.theme.CRTheme

/**
 * Quick Settings tile — lets users toggle clipboard sync from the notification shade.
 *
 * Shows:
 *   - Active state: "Deskdrop · Syncing"
 *   - Inactive state: "Deskdrop · Paused"
 *
 * Long-press opens Deskdrop settings.
 * Does NOT show clipboard content or peer data in the tile.
 */
class DeskdropTileService : TileService() {

    override fun onStartListening() {
        super.onStartListening()
        refreshTile()
    }

    override fun onClick() {
        super.onClick()
        pushClipboard()
    }

    private fun refreshTile() {
        val tile = qsTile ?: return
        val prefs   = getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE)
        val count   = prefs.getInt("connected_count", 0)

        tile.state = if (count > 0) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = "Push Clipboard"

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            tile.subtitle = when {
                count == 0   -> "No devices"
                count == 1   -> "1 device"
                else         -> "$count devices"
            }
        }

        tile.contentDescription = "Push Clipboard to Mac"
        tile.updateTile()
    }

    private fun pushClipboard() {
        val intent = Intent(this, DeskdropService::class.java).apply {
            action = DeskdropService.ACTION_PUSH_CLIPBOARD
        }
        runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                ContextCompat.startForegroundService(this, intent)
            } else {
                startService(intent)
            }
        }
        Toast.makeText(
            applicationContext,
            "Pushing clipboard...",
            Toast.LENGTH_SHORT
        ).show()
    }
}

/**
 * Share target — appears in Android's share sheet, letting users push
 * any shared text directly to Deskdrop peers without opening the app.
 */
class DeskdropShareTarget : ComponentActivity() {

    private fun dp(v: Int): Int = (v * resources.displayMetrics.density).roundToInt()
    private fun c(@ColorRes id: Int): Int = ContextCompat.getColor(this, id)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        androidx.core.view.WindowCompat.setDecorFitsSystemWindows(window, false)
        window.setBackgroundDrawable(ColorDrawable(android.graphics.Color.TRANSPARENT))
        window.decorView.setBackgroundColor(android.graphics.Color.TRANSPARENT)
        window.statusBarColor = android.graphics.Color.TRANSPARENT
        window.navigationBarColor = android.graphics.Color.TRANSPARENT

        val sharedUris = when (intent?.action) {
            Intent.ACTION_SEND -> {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)?.let { arrayListOf(it) }
                } else {
                    @Suppress("DEPRECATION")
                    intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)?.let { arrayListOf(it) }
                }
            }
            Intent.ACTION_SEND_MULTIPLE -> {
                val items = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
                } else {
                    @Suppress("DEPRECATION")
                    intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
                }
                items
            }
            else -> null
        }
        val sharedText = when {
            sharedUris.isNullOrEmpty() &&
                intent?.action == Intent.ACTION_SEND &&
                intent.hasExtra(Intent.EXTRA_TEXT) ->
                intent.getStringExtra(Intent.EXTRA_TEXT)
            else -> null
        }
        val sharedName = intent?.getStringExtra(Intent.EXTRA_TITLE)?.takeIf { it.isNotBlank() }

        if (!sharedText.isNullOrBlank()) {
            runCatching {
                ContextCompat.startForegroundService(this, Intent(this, DeskdropService::class.java).apply {
                    action = DeskdropService.ACTION_PUSH_TEXT
                    putExtra("text", sharedText)
                })
            }
            Toast.makeText(this, "Pushed to Deskdrop peers", Toast.LENGTH_SHORT).show()
            finish()
        } else if (!sharedUris.isNullOrEmpty()) {
            val peers = getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE)
                .peerSnapshots()
                .filter { it.isConnected }
            val isDark = getSharedPreferences(DeskdropService.PREFS_NAME, MODE_PRIVATE).getBoolean("dark_mode", false)

            setContent {
                AppTheme(useDarkTheme = isDark) {
                    ShareTargetUI(
                        sharedUris = sharedUris,
                        sharedName = sharedName,
                        peers = peers,
                        isDark = isDark,
                        onCancel = { finish() },
                        onSend = { targetId ->
                            sendFiles(sharedUris, sharedName, targetId)
                        }
                    )
                }
            }
        } else {
            Toast.makeText(this, "Nothing to push", Toast.LENGTH_SHORT).show()
            finish()
        }
    }

    private fun sendFiles(sharedUris: List<Uri>, sharedName: String?, targetId: String?) {
        Thread {
            val stagedUris = mutableListOf<String>()
            sharedUris.forEachIndexed { index, uri ->
                val stagedFileUri = stageFileInActivity(uri, index + 1)
                if (stagedFileUri != null) {
                    stagedUris.add(stagedFileUri.toString())
                }
            }

            runOnUiThread {
                if (stagedUris.isNotEmpty()) {
                    val svc = Intent(this@DeskdropShareTarget, DeskdropService::class.java).apply {
                        action = DeskdropService.ACTION_PUSH_SHARED_URI
                        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                        putStringArrayListExtra(
                            DeskdropService.EXTRA_SHARED_URIS,
                            ArrayList(stagedUris)
                        )
                        sharedName?.let { putExtra(DeskdropService.EXTRA_SHARED_NAME, it) }
                        targetId?.let { putExtra(DeskdropService.EXTRA_TARGET_DEVICE_ID, it) }
                    }
                    runCatching { ContextCompat.startForegroundService(this@DeskdropShareTarget, svc) }
                    Toast.makeText(this@DeskdropShareTarget, "Sending to Deskdrop", Toast.LENGTH_SHORT).show()
                } else {
                    Toast.makeText(this@DeskdropShareTarget, "Staging failed", Toast.LENGTH_SHORT).show()
                }
                finish()
            }
        }.start()
    }

    private fun stageFileInActivity(uri: Uri, fallbackIndex: Int): Uri? = runCatching {
        val mime = contentResolver.getType(uri) ?: "application/octet-stream"
        val ext = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime.substringBefore(';')) ?: ""
        
        var displayName = "Shared file $fallbackIndex"
        if (uri.scheme.equals("file", ignoreCase = true)) {
            displayName = uri.path?.let { java.io.File(it).name } ?: displayName
        } else {
            contentResolver.query(uri, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                val col = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                if (col >= 0 && cursor.moveToFirst()) {
                    cursor.getString(col)?.takeIf { it.isNotBlank() }?.let { displayName = it }
                }
            }
        }
        
        val stagedDir = java.io.File(cacheDir, "shared-outgoing").also { it.mkdirs() }
        val sanitizedName = displayName.replace("[\\\\/:*?\"<>|]".toRegex(), "_")
        val finalExt = if (ext.isNotEmpty()) ".$ext" else ""
        val stagedFile = java.io.File(stagedDir, if (sanitizedName.endsWith(finalExt)) sanitizedName else "$sanitizedName$finalExt")
        
        contentResolver.openInputStream(uri)?.use { input ->
            java.io.FileOutputStream(stagedFile).use { output ->
                input.copyTo(output, 256 * 1024)
            }
        } ?: return null
        
        Uri.fromFile(stagedFile)
    }.onFailure { Log.w("DeskdropShareTarget", "Failed to stage shared URI $uri", it) }.getOrNull()
}

@Composable
fun ShareTargetUI(
    sharedUris: List<Uri>,
    sharedName: String?,
    peers: List<PeerSnapshot>,
    isDark: Boolean,
    onCancel: () -> Unit,
    onSend: (String?) -> Unit
) {
    var selectedDevice by remember { mutableStateOf<String?>(if (peers.size == 1) peers.first().id else null) }
    var isSending by remember { mutableStateOf(false) }

    val sheetBg = if (isDark) Color(0xFF1C1C1E) else Color(0xFFF2F2F7)
    val cardBg = if (isDark) Color(0xFF2C2C2E) else Color.White
    val sheetShape = RoundedCornerShape(topStart = 32.dp, topEnd = 32.dp)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .clickable(
                indication = null,
                interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() },
                onClick = onCancel
            ),
        contentAlignment = Alignment.BottomCenter
    ) {
        // Apply the clipping and background at this container level to fix the overflow issue
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .clip(sheetShape)
                .background(sheetBg)
                .clickable(
                    indication = null,
                    interactionSource = remember { androidx.compose.foundation.interaction.MutableInteractionSource() },
                    onClick = {} // consume clicks
                )
        ) {
            // Inner subtle border for premium feel
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .border(
                        1.dp,
                        Brush.verticalGradient(
                            colors = listOf(
                                Color.White.copy(alpha = if (isDark) 0.15f else 0.5f),
                                Color.Transparent
                            )
                        ),
                        sheetShape
                    )
            )

            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .navigationBarsPadding()
                    .padding(bottom = 32.dp)
            ) {
                // Drag handle
                Box(
                    modifier = Modifier
                        .padding(top = 16.dp, bottom = 24.dp)
                        .width(48.dp)
                        .height(5.dp)
                        .clip(CircleShape)
                        .background(if (isDark) Color.White.copy(alpha = 0.2f) else Color.Black.copy(alpha = 0.2f))
                        .align(Alignment.CenterHorizontally)
                )

                if (isSending) {
                    Column(
                        modifier = Modifier.fillMaxWidth().padding(vertical = 48.dp),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        CircularProgressIndicator(
                            color = CRTheme.brandElectric,
                            strokeWidth = 3.dp,
                            modifier = Modifier.size(56.dp)
                        )
                        Spacer(modifier = Modifier.height(24.dp))
                        Text(
                            text = "Preparing secure transfer...",
                            color = CRTheme.textHigh(isDark),
                            fontSize = 18.sp,
                            fontWeight = FontWeight.SemiBold
                        )
                        Text(
                            text = "Staging ${sharedUris.size} files locally",
                            color = CRTheme.textMedium(isDark),
                            fontSize = 14.sp,
                            modifier = Modifier.padding(top = 8.dp)
                        )
                    }
                } else {
                    // Header
                    Row(
                        modifier = Modifier.padding(horizontal = 24.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Box(
                            modifier = Modifier
                                .size(48.dp)
                                .background(
                                    Brush.linearGradient(listOf(CRTheme.brandElectric, CRTheme.brandViolet)),
                                    shape = CircleShape
                                ),
                            contentAlignment = Alignment.Center
                        ) {
                            // Fallback to text icon if material icons are missing
                            Text("↑", color = Color.White, fontSize = 24.sp, fontWeight = FontWeight.Bold)
                        }
                        Column(modifier = Modifier.padding(start = 16.dp)) {
                            Text(
                                text = "Send with Deskdrop",
                                color = CRTheme.textHigh(isDark),
                                fontSize = 22.sp,
                                fontWeight = FontWeight.Bold
                            )
                            val noun = if (sharedUris.size == 1) "file" else "files"
                            Text(
                                text = "Sharing ${sharedUris.size} $noun",
                                color = CRTheme.brandElectric,
                                fontSize = 14.sp,
                                fontWeight = FontWeight.Medium
                            )
                        }
                    }

                    Spacer(modifier = Modifier.height(32.dp))

                    Text(
                        text = if (peers.isEmpty()) "NO DEVICES FOUND" else "SELECT DEVICE",
                        color = CRTheme.textMedium(isDark),
                        fontSize = 12.sp,
                        fontWeight = FontWeight.Bold,
                        letterSpacing = 1.sp,
                        modifier = Modifier.padding(horizontal = 24.dp)
                    )

                    Spacer(modifier = Modifier.height(16.dp))

                    // Peer Grid (No scrolling for up to 4 items)
                    if (peers.isEmpty()) {
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 24.dp)
                                .background(cardBg, RoundedCornerShape(20.dp))
                                .padding(32.dp),
                            horizontalAlignment = Alignment.CenterHorizontally
                        ) {
                            Text(
                                text = "No connected devices",
                                color = CRTheme.textHigh(isDark),
                                fontSize = 16.sp,
                                fontWeight = FontWeight.Bold
                            )
                            Spacer(modifier = Modifier.height(8.dp))
                            Text(
                                text = "Open Deskdrop on your Mac, or launch the app to search again.",
                                color = CRTheme.textMedium(isDark),
                                fontSize = 14.sp,
                                textAlign = TextAlign.Center
                            )
                        }
                    } else {
                        // Use a horizontal scrolling row for a more compact "Command Deck" feel
                        androidx.compose.foundation.lazy.LazyRow(
                            contentPadding = PaddingValues(horizontal = 24.dp),
                            horizontalArrangement = Arrangement.spacedBy(16.dp)
                        ) {
                            item {
                                PeerSelectionCard(
                                    title = "All Devices",
                                    subtitle = "Broadcast",
                                    avatarText = "ALL",
                                    gradient = listOf(CRTheme.brandElectric, CRTheme.brandViolet),
                                    isSelected = selectedDevice == null,
                                    isDark = isDark,
                                    cardBg = cardBg,
                                    onClick = { selectedDevice = null }
                                )
                            }
                            items(peers) { peer ->
                                val isMac = peer.name.contains("mac", ignoreCase = true) || peer.name.contains("book", ignoreCase = true)
                                val gradient = if (isMac) listOf(Color(0xFF5E5CE6), Color(0xFF3F3D96)) else listOf(CRTheme.accentGreen, Color(0xFF1E6E3C))
                                PeerSelectionCard(
                                    title = peer.name,
                                    subtitle = if (peer.trusted) "Ready" else "Connected",
                                    avatarText = peer.name.firstOrNull()?.uppercase() ?: "?",
                                    gradient = gradient,
                                    isSelected = selectedDevice == peer.id,
                                    isDark = isDark,
                                    cardBg = cardBg,
                                    onClick = { selectedDevice = peer.id }
                                )
                            }
                        }
                    }

                    Spacer(modifier = Modifier.height(32.dp))

                    // Action Buttons
                    Row(modifier = Modifier.padding(horizontal = 24.dp)) {
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .height(56.dp)
                                .clip(RoundedCornerShape(16.dp))
                                .background(cardBg)
                                .clickable(onClick = onCancel),
                            contentAlignment = Alignment.Center
                        ) {
                            Text("Cancel", color = CRTheme.textHigh(isDark), fontWeight = FontWeight.SemiBold, fontSize = 16.sp)
                        }
                        Spacer(modifier = Modifier.width(16.dp))
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .height(56.dp)
                                .clip(RoundedCornerShape(16.dp))
                                .background(if (peers.isNotEmpty()) Brush.horizontalGradient(listOf(CRTheme.brandElectric, CRTheme.brandViolet)) else Brush.horizontalGradient(listOf(Color.Gray, Color.DarkGray)))
                                .clickable(enabled = peers.isNotEmpty()) {
                                    isSending = true
                                    onSend(selectedDevice)
                                },
                            contentAlignment = Alignment.Center
                        ) {
                            Text("Send", color = Color.White, fontWeight = FontWeight.Bold, fontSize = 16.sp)
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun PeerSelectionCard(
    title: String,
    subtitle: String,
    avatarText: String,
    gradient: List<Color>,
    isSelected: Boolean,
    isDark: Boolean,
    cardBg: Color,
    onClick: () -> Unit
) {
    val borderColor = if (isSelected) CRTheme.brandElectric else if (isDark) Color.White.copy(alpha=0.05f) else Color.Black.copy(alpha=0.05f)
    val bgColor = if (isSelected) CRTheme.brandElectric.copy(alpha = 0.15f) else cardBg
    
    Column(
        modifier = Modifier
            .width(120.dp)
            .height(140.dp)
            .clip(RoundedCornerShape(20.dp))
            .background(bgColor)
            .border(2.dp, borderColor, RoundedCornerShape(20.dp))
            .clickable(onClick = onClick)
            .padding(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Box(
            modifier = Modifier
                .size(48.dp)
                .background(Brush.linearGradient(gradient), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Text(avatarText, color = Color.White, fontSize = if (avatarText.length > 1) 14.sp else 20.sp, fontWeight = FontWeight.Bold)
            if (isSelected) {
                // Checkmark or dot overlay
                Box(modifier = Modifier.align(Alignment.BottomEnd).offset(x=4.dp, y=4.dp).size(16.dp).background(Color.White, CircleShape).padding(2.dp).background(CRTheme.brandElectric, CircleShape))
            }
        }
        Spacer(modifier = Modifier.height(12.dp))
        Text(
            title, 
            color = CRTheme.textHigh(isDark), 
            fontSize = 14.sp, 
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            subtitle, 
            color = CRTheme.textMedium(isDark), 
            fontSize = 12.sp,
            maxLines = 1,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis
        )
    }
}
