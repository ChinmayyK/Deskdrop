package com.cliprelay.ui.theme

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.sin
import kotlin.math.cos

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

    fun canvasTop(isDark: Boolean) = if (isDark) Color(0xFF050505) else Color(0xFFF8FAFC)
    fun canvasBottom(isDark: Boolean) = if (isDark) Color(0xFF000000) else Color(0xFFF1F5F9)
    
    fun ink(isDark: Boolean) = if (isDark) Color(0xFFFFFFFF) else Color(0xFF0F172A)
    fun inkSoft(isDark: Boolean) = if (isDark) Color(0xFFA1A1AA) else Color(0xFF475569)
    fun inkSubtle(isDark: Boolean) = if (isDark) Color(0xFF71717A) else Color(0xFF64748B)

    fun stroke(isDark: Boolean) = if (isDark) Color(0xFFFFFFFF).copy(alpha = 0.12f) else Color(0xFFFFFFFF).copy(alpha = 0.6f)

    fun cardGradient(isDark: Boolean): Brush {
        return Brush.linearGradient(
            colors = if (isDark) listOf(
                Color(0xFFFFFFFF).copy(alpha = 0.08f),
                Color(0xFFFFFFFF).copy(alpha = 0.03f)
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

@Composable
fun CRBackground(isDark: Boolean, content: @Composable () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(CRTheme.canvasGradient(isDark))
    ) {
        // Fluid Mesh Gradient Simulation
        val infiniteTransition = rememberInfiniteTransition()
        val time by infiniteTransition.animateFloat(
            initialValue = 0f, targetValue = 2f * Math.PI.toFloat(),
            animationSpec = infiniteRepeatable(animation = tween(20000, easing = LinearEasing), repeatMode = RepeatMode.Restart)
        )

        val opacity = if (isDark) 0.12f else 0.18f

        // Orb 1: Cyan, drifting in figure-8
        Box(
            modifier = Modifier
                .offset(
                    x = (sin(time) * 150).dp,
                    y = (cos(time * 0.8f) * 100).dp
                )
                .fillMaxSize(0.65f)
                .blur(140.dp)
                .background(Brush.radialGradient(listOf(CRTheme.brandCyan.copy(alpha = opacity), Color.Transparent)), shape = CircleShape)
        )

        // Orb 2: Violet, drifting in reverse figure-8
        Box(
            modifier = Modifier
                .offset(
                    x = (cos(time * 1.2f) * 200).dp,
                    y = (sin(time * 0.9f) * 150 + 200).dp
                )
                .fillMaxSize(0.7f)
                .blur(160.dp)
                .background(Brush.radialGradient(listOf(CRTheme.brandViolet.copy(alpha = opacity), Color.Transparent)), shape = CircleShape)
        )

        // Orb 3: Pink, sweeping bottom
        Box(
            modifier = Modifier
                .offset(
                    x = (sin(time * 0.5f) * 300).dp,
                    y = 500.dp + (cos(time * 1.1f) * 50).dp
                )
                .fillMaxSize(0.8f)
                .blur(150.dp)
                .background(Brush.radialGradient(listOf(CRTheme.brandPink.copy(alpha = opacity * 0.8f), Color.Transparent)), shape = CircleShape)
        )

        content()
    }
}

fun Modifier.crCard(
    isDark: Boolean,
    cornerRadius: Dp = 24.dp, // Premium rounded corners
    highlighted: Boolean = false,
    accentColor: Color = CRTheme.brandElectric,
    onClick: (() -> Unit)? = null
): Modifier = composed {
    var isPressed by remember { mutableStateOf(false) }
    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.96f else 1f,
        animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessLow),
        label = "card_bounce"
    )

    val shape = RoundedCornerShape(cornerRadius)
    val strokeColor = if (highlighted) accentColor.copy(alpha = 0.5f) else CRTheme.stroke(isDark)
    val shadowColor = if (isDark) Color(0xFF000000).copy(alpha = 0.6f) else Color(0xFF0066FF).copy(alpha = 0.08f)

    var modifier = this
        .scale(scale)
        .shadow(
            elevation = if (highlighted) 24.dp else 16.dp,
            shape = shape,
            ambientColor = shadowColor,
            spotColor = shadowColor
        )
        .clip(shape)
        .background(CRTheme.cardGradient(isDark))
        .border(if (highlighted) 1.5.dp else 0.5.dp, strokeColor, shape)
        
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
