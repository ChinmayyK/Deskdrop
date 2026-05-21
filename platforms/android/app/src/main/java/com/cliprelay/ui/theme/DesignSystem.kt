package com.cliprelay.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
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

    // Semantic Surfaces
    val surfaceLight = Color(0xFFFFFFFF)
    val surfaceDark = Color(0xFF000000)
    
    val surfaceStrongLight = Color(0xFFF8FAFC)
    val surfaceStrongDark = Color(0xFF09090B)
    
    val surfaceElevatedLight = Color(0xFFFFFFFF).copy(alpha = 0.95f)
    val surfaceElevatedDark = Color(0xFF000000).copy(alpha = 0.95f)

    // System accents
    val accentBlue = Color(0xFF3B82F6)
    val accentGreen = Color(0xFF10B981)
    val accentRed = Color(0xFFEF4444)
    val accentAmber = Color(0xFFF59E0B)

    // Strokes
    val strokeLight = Color(0xFFE2E8F0)
    val strokeDark = Color(0xFFFFFFFF).copy(alpha = 0.15f)

    fun canvasTop(isDark: Boolean) = if (isDark) Color(0xFF000000) else Color(0xFFFFFFFF)
    fun canvasBottom(isDark: Boolean) = if (isDark) Color(0xFF000000) else Color(0xFFF8FAFC)
    
    fun ink(isDark: Boolean) = if (isDark) Color(0xFFFFFFFF) else Color(0xFF0F172A)
    fun inkSoft(isDark: Boolean) = if (isDark) Color(0xFFA1A1AA) else Color(0xFF475569)
    fun inkSubtle(isDark: Boolean) = if (isDark) Color(0xFF71717A) else Color(0xFF64748B)

    fun stroke(isDark: Boolean) = if (isDark) strokeDark else strokeLight

    fun cardGradient(isDark: Boolean): Brush {
        return Brush.linearGradient(
            colors = if (isDark) listOf(
                Color(0xFF18181B).copy(alpha = 0.6f),
                Color(0xFF09090B).copy(alpha = 0.4f)
            ) else listOf(
                Color(0xFFFFFFFF).copy(alpha = 0.88f),
                Color(0xFFF8FAFC).copy(alpha = 0.50f)
            )
        )
    }

    fun canvasGradient(isDark: Boolean): Brush {
        return Brush.linearGradient(
            colors = listOf(canvasTop(isDark), canvasBottom(isDark))
        )
    }
}

fun Modifier.crCard(
    isDark: Boolean,
    cornerRadius: Dp = 12.dp,
    highlighted: Boolean = false,
    accentColor: Color = CRTheme.accentBlue
): Modifier = composed {
    val shape = RoundedCornerShape(cornerRadius)
    val strokeColor = if (highlighted) accentColor.copy(alpha = 0.5f) else CRTheme.stroke(isDark)
    val shadowColor = if (highlighted) accentColor.copy(alpha = 0.15f) else Color(0xFF0F172A).copy(alpha = 0.04f)

    this
        .shadow(
            elevation = if (highlighted) 6.dp else 4.dp,
            shape = shape,
            ambientColor = shadowColor,
            spotColor = shadowColor
        )
        .clip(shape)
        .background(CRTheme.cardGradient(isDark))
        .border(if (highlighted) 1.dp else 0.5.dp, strokeColor, shape)
}
