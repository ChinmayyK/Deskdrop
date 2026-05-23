package com.deskdrop.ui.theme

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.cos
import kotlin.math.sin

object CRTheme {
    // Brand
    val brandElectric = Color(0xFF0066FF)
    val brandViolet = Color(0xFF8B5CF6)
    val brandCyan = Color(0xFF06B6D4)
    val brandPink = Color(0xFFEC4899)

    // System accents
    val accentBlue = Color(0xFF3B82F6)
    val accentGreen = Color(0xFF10B981)
    val accentRed = Color(0xFFEF4444)
    val accentAmber = Color(0xFFF59E0B)

    // Semantic Surfaces
    val surfaceStrongDark = Color(0xFF09090B)

    fun canvasTop(isDark: Boolean) = if (isDark) Color(0xFF000000) else Color(0xFFF8FAFC)
    fun canvasBottom(isDark: Boolean) = if (isDark) Color(0xFF070708) else Color(0xFFF1F5F9)
    
    fun ink(isDark: Boolean) = if (isDark) Color(0xFFFFFFFF) else Color(0xFF0F172A)
    fun inkSoft(isDark: Boolean) = if (isDark) Color(0xFFA1A1AA) else Color(0xFF475569)
    fun inkSubtle(isDark: Boolean) = if (isDark) Color(0xFF71717A) else Color(0xFF64748B)

    fun stroke(isDark: Boolean) = if (isDark) Color(0xFFFFFFFF).copy(alpha = 0.12f) else Color(0xFF000000).copy(alpha = 0.08f)

    fun surface(isDark: Boolean) = if (isDark) Color.White.copy(alpha = 0.05f) else Color.White
    fun surfaceElevated(isDark: Boolean) = if (isDark) Color.White.copy(alpha = 0.08f) else Color.White
    fun divider(isDark: Boolean) = if (isDark) Color.White.copy(alpha = 0.1f) else Color.Black.copy(alpha = 0.05f)

    val brandGradient = Brush.horizontalGradient(listOf(brandElectric, brandViolet))
    val successGradient = Brush.horizontalGradient(listOf(accentGreen, brandCyan))

    fun cardGradient(isDark: Boolean): Brush {
        return Brush.linearGradient(
            colors = if (isDark) listOf(
                Color(0xFFFFFFFF).copy(alpha = 0.07f),
                Color(0xFFFFFFFF).copy(alpha = 0.02f)
            ) else listOf(
                Color(0xFFFFFFFF).copy(alpha = 0.85f),
                Color(0xFFFFFFFF).copy(alpha = 0.50f)
            )
        )
    }

    fun canvasGradient(isDark: Boolean): Brush {
        return Brush.linearGradient(
            colors = listOf(canvasTop(isDark), canvasBottom(isDark))
        )
    }
}

object CRMotion {
    val bouncy = spring<Float>(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessLow)
    val smooth = spring<Float>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium)
    val snappy = spring<Float>(dampingRatio = Spring.DampingRatioLowBouncy, stiffness = Spring.StiffnessHigh)
}

@Composable
fun CyberMeshOverlay(isDark: Boolean) {
    val dotColor = if (isDark) Color.White.copy(alpha = 0.04f) else Color.Black.copy(alpha = 0.02f)
    androidx.compose.foundation.Canvas(modifier = Modifier.fillMaxSize()) {
        val dotRadius = 1.dp.toPx()
        val spacing = 24.dp.toPx()
        var x = 0f
        while (x < size.width) {
            var y = 0f
            while (y < size.height) {
                drawCircle(
                    color = dotColor,
                    radius = dotRadius,
                    center = Offset(x, y)
                )
                y += spacing
            }
            x += spacing
        }
    }
}

@Composable
fun RadarRipple(color: Color, modifier: Modifier = Modifier) {
    val infiniteTransition = rememberInfiniteTransition(label = "radar")
    val progress1 by infiniteTransition.animateFloat(
        initialValue = 0f, targetValue = 1f,
        animationSpec = infiniteRepeatable(animation = tween(2400, easing = LinearEasing)),
        label = "ripple1"
    )
    val progress2 by infiniteTransition.animateFloat(
        initialValue = 0f, targetValue = 1f,
        animationSpec = infiniteRepeatable(animation = tween(2400, easing = LinearEasing), initialStartOffset = StartOffset(800)),
        label = "ripple2"
    )
    val progress3 by infiniteTransition.animateFloat(
        initialValue = 0f, targetValue = 1f,
        animationSpec = infiniteRepeatable(animation = tween(2400, easing = LinearEasing), initialStartOffset = StartOffset(1600)),
        label = "ripple3"
    )

    androidx.compose.foundation.Canvas(modifier = modifier.fillMaxSize()) {
        val center = center
        val maxRadius = size.minDimension / 2f

        listOf(progress1, progress2, progress3).forEach { p ->
            val radius = maxRadius * p
            val alpha = (1f - p) * 0.45f
            if (radius > 0f) {
                drawCircle(
                    color = color,
                    radius = radius,
                    center = center,
                    alpha = alpha,
                    style = Stroke(width = 2.dp.toPx())
                )
            }
        }
    }
}

@Composable
fun CRBackground(isDark: Boolean, content: @Composable () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(CRTheme.canvasGradient(isDark))
    ) {
        val infiniteTransition = rememberInfiniteTransition(label = "bg")
        val time by infiniteTransition.animateFloat(
            initialValue = 0f, targetValue = 2f * Math.PI.toFloat(),
            animationSpec = infiniteRepeatable(animation = tween(40000, easing = LinearEasing), repeatMode = RepeatMode.Restart),
            label = "time"
        )

        val opacity = if (isDark) 0.12f else 0.16f

        // Orb 1: Cyan
        Box(
            modifier = Modifier
                .offset(
                    x = (sin(time) * 140).dp,
                    y = (cos(time * 0.7f) * 90).dp
                )
                .fillMaxSize(0.7f)
                .blur(160.dp)
                .background(Brush.radialGradient(listOf(CRTheme.brandCyan.copy(alpha = opacity), Color.Transparent)), shape = CircleShape)
        )

        // Orb 2: Violet
        Box(
            modifier = Modifier
                .offset(
                    x = (cos(time * 1.1f) * 190).dp,
                    y = (sin(time * 0.8f) * 130 + 180).dp
                )
                .fillMaxSize(0.75f)
                .blur(180.dp)
                .background(Brush.radialGradient(listOf(CRTheme.brandViolet.copy(alpha = opacity), Color.Transparent)), shape = CircleShape)
        )

        // Orb 3: Pink
        Box(
            modifier = Modifier
                .offset(
                    x = (sin(time * 0.6f) * 230).dp,
                    y = 480.dp + (cos(time * 1.2f) * 60).dp
                )
                .fillMaxSize(0.85f)
                .blur(170.dp)
                .background(Brush.radialGradient(listOf(CRTheme.brandPink.copy(alpha = opacity * 0.7f), Color.Transparent)), shape = CircleShape)
        )

        CyberMeshOverlay(isDark = isDark)

        content()
    }
}

@Composable
fun GlowingOrb(color: Color, size: Dp, blurRadius: Dp, opacity: Float, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .size(size)
            .blur(blurRadius)
            .background(Brush.radialGradient(listOf(color.copy(alpha = opacity), Color.Transparent)), shape = CircleShape)
    )
}

@Composable
fun StatusPulse(color: Color, modifier: Modifier = Modifier) {
    val infiniteTransition = rememberInfiniteTransition(label = "pulse")
    val scale by infiniteTransition.animateFloat(
        initialValue = 0.8f, targetValue = 1.4f,
        animationSpec = infiniteRepeatable(animation = tween(1500), repeatMode = RepeatMode.Reverse),
        label = "scale"
    )
    val alpha by infiniteTransition.animateFloat(
        initialValue = 0.8f, targetValue = 0.2f,
        animationSpec = infiniteRepeatable(animation = tween(1500), repeatMode = RepeatMode.Reverse),
        label = "alpha"
    )

    Box(modifier = modifier.size(12.dp), contentAlignment = Alignment.Center) {
        Box(modifier = Modifier.matchParentSize().scale(scale).clip(CircleShape).background(color.copy(alpha = alpha)))
        Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(color))
    }
}

@Composable
fun GradientIcon(imageVector: ImageVector, brush: Brush, contentDescription: String?, modifier: Modifier = Modifier) {
    Icon(
        imageVector = imageVector,
        contentDescription = contentDescription,
        tint = Color.White,
        modifier = modifier.graphicsLayer(alpha = 0.99f).drawWithCache {
            onDrawWithContent {
                drawContent()
                drawRect(brush, blendMode = BlendMode.SrcAtop)
            }
        }
    )
}

@Composable
fun SectionHeader(isDark: Boolean, title: String, modifier: Modifier = Modifier) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = modifier.padding(vertical = 8.dp)) {
        Box(modifier = Modifier.size(6.dp).clip(CircleShape).background(CRTheme.brandElectric))
        Spacer(modifier = Modifier.width(8.dp))
        Text(text = title.uppercase(), fontSize = 13.sp, fontWeight = FontWeight.ExtraBold, color = CRTheme.inkSubtle(isDark), letterSpacing = 1.2.sp)
    }
}

fun Modifier.crCard(
    isDark: Boolean,
    cornerRadius: Dp = 24.dp,
    highlighted: Boolean = false,
    accentColor: Color = CRTheme.brandElectric,
    onClick: (() -> Unit)? = null
): Modifier = composed {
    var isPressed by remember { mutableStateOf(false) }
    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.96f else 1f,
        animationSpec = CRMotion.bouncy,
        label = "card_bounce"
    )

    val shape = RoundedCornerShape(cornerRadius)
    val strokeColor = if (highlighted) accentColor.copy(alpha = 0.7f) else CRTheme.stroke(isDark)
    val shadowColor = if (isDark) Color.Black.copy(alpha = 0.7f) else Color.Black.copy(alpha = 0.06f)
    val innerHalo = if (isDark) Color.White.copy(alpha = 0.04f) else Color.White.copy(alpha = 0.5f)

    var modifier = this
        .scale(scale)
        .shadow(
            elevation = if (highlighted) 20.dp else 8.dp,
            shape = shape,
            ambientColor = shadowColor,
            spotColor = if (highlighted) accentColor.copy(alpha = 0.35f) else shadowColor
        )
        .clip(shape)
        .background(CRTheme.cardGradient(isDark))
        .border(if (highlighted) 1.5.dp else 1.dp, strokeColor, shape)
        .border(2.dp, innerHalo, shape)
        
    if (onClick != null) {
        modifier = modifier.pointerInput(Unit) {
            detectTapGestures(
                onPress = {
                    isPressed = true
                    tryAwaitRelease()
                    isPressed = false
                },
                onTap = { onClick() }
            )
        }
    }
    
    modifier
}

fun Modifier.crCardElevated(isDark: Boolean, cornerRadius: Dp = 24.dp, onClick: (() -> Unit)? = null): Modifier = composed {
    this.crCard(isDark, cornerRadius, highlighted = true, onClick = onClick)
}

fun Modifier.crButton(
    brush: Brush = CRTheme.brandGradient,
    cornerRadius: Dp = 16.dp,
    onClick: () -> Unit
): Modifier = composed {
    var isPressed by remember { mutableStateOf(false) }
    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.95f else 1f,
        animationSpec = CRMotion.bouncy,
        label = "btn_bounce"
    )

    this
        .scale(scale)
        .shadow(12.dp, RoundedCornerShape(cornerRadius), spotColor = CRTheme.brandElectric.copy(alpha = 0.5f))
        .clip(RoundedCornerShape(cornerRadius))
        .background(brush)
        .pointerInput(Unit) {
            detectTapGestures(
                onPress = {
                    isPressed = true
                    tryAwaitRelease()
                    isPressed = false
                },
                onTap = { onClick() }
            )
        }
}

@Composable
fun CRSwitch(checked: Boolean, isDark: Boolean) {
    val thumbOffset by animateFloatAsState(
        targetValue = if (checked) 24f else 4f,
        animationSpec = CRMotion.bouncy,
        label = "switchOffset"
    )
    val thumbColor by animateColorAsState(
        targetValue = if (checked) Color.White else CRTheme.inkSubtle(isDark),
        animationSpec = tween(200),
        label = "switchThumb"
    )
    val trackColor by animateColorAsState(
        targetValue = if (checked) CRTheme.brandElectric else CRTheme.surfaceElevated(isDark),
        animationSpec = tween(200),
        label = "switchTrack"
    )

    Box(
        modifier = Modifier
            .width(52.dp)
            .height(28.dp)
            .clip(CircleShape)
            .background(trackColor)
    ) {
        Box(
            modifier = Modifier
                .offset(x = thumbOffset.dp, y = 4.dp)
                .size(20.dp)
                .shadow(6.dp, CircleShape)
                .clip(CircleShape)
                .background(thumbColor)
                .border(0.5.dp, CRTheme.stroke(isDark), CircleShape),
            contentAlignment = Alignment.Center
        ) {
            Box(
                modifier = Modifier
                    .size(6.dp)
                    .clip(CircleShape)
                    .background(if (checked) CRTheme.brandElectric else Color.Transparent)
            )
        }
    }
}

