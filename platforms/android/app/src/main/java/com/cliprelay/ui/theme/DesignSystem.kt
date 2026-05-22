package com.cliprelay.ui.theme

import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

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
        // Animated glowing orbs for rich aesthetics
        val infiniteTransition = rememberInfiniteTransition()
        val orb1X by infiniteTransition.animateFloat(
            initialValue = -100f, targetValue = 300f,
            animationSpec = infiniteRepeatable(animation = tween(15000, easing = LinearEasing), repeatMode = RepeatMode.Reverse)
        )
        val orb1Y by infiniteTransition.animateFloat(
            initialValue = -100f, targetValue = 500f,
            animationSpec = infiniteRepeatable(animation = tween(12000, easing = LinearEasing), repeatMode = RepeatMode.Reverse)
        )
        val orb2X by infiniteTransition.animateFloat(
            initialValue = 400f, targetValue = -200f,
            animationSpec = infiniteRepeatable(animation = tween(18000, easing = LinearEasing), repeatMode = RepeatMode.Reverse)
        )
        val orb2Y by infiniteTransition.animateFloat(
            initialValue = 600f, targetValue = 100f,
            animationSpec = infiniteRepeatable(animation = tween(14000, easing = LinearEasing), repeatMode = RepeatMode.Reverse)
        )

        val opacity = if (isDark) 0.15f else 0.25f

        Box(
            modifier = Modifier
                .offset(x = orb1X.dp, y = orb1Y.dp)
                .fillMaxSize(0.6f)
                .blur(100.dp)
                .background(Brush.radialGradient(listOf(CRTheme.brandCyan.copy(alpha = opacity), Color.Transparent)), shape = CircleShape)
        )
        Box(
            modifier = Modifier
                .offset(x = orb2X.dp, y = orb2Y.dp)
                .fillMaxSize(0.7f)
                .blur(120.dp)
                .background(Brush.radialGradient(listOf(CRTheme.brandViolet.copy(alpha = opacity), Color.Transparent)), shape = CircleShape)
        )

        content()
    }
}

fun Modifier.crCard(
    isDark: Boolean,
    cornerRadius: Dp = 24.dp, // Increased radius for premium feel
    highlighted: Boolean = false,
    accentColor: Color = CRTheme.brandElectric
): Modifier = composed {
    val shape = RoundedCornerShape(cornerRadius)
    val strokeColor = if (highlighted) accentColor.copy(alpha = 0.5f) else CRTheme.stroke(isDark)
    val shadowColor = if (isDark) Color(0xFF000000).copy(alpha = 0.5f) else Color(0xFF0066FF).copy(alpha = 0.06f)

    this
        .shadow(
            elevation = if (highlighted) 16.dp else 12.dp,
            shape = shape,
            ambientColor = shadowColor,
            spotColor = shadowColor
        )
        .clip(shape)
        .background(CRTheme.cardGradient(isDark))
        .border(if (highlighted) 1.dp else 0.5.dp, strokeColor, shape)
}
