package com.deskdrop.ui.theme

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.graphics.SolidColor

object CRTheme {
    // Earthy / Neutral Palette
    val espresso = Color(0xFF1A1816)
    val charcoal = Color(0xFF282522)
    val parchment = Color(0xFFF6F4F0)
    val alabaster = Color(0xFFEBE8E2)
    
    // Accents
    val terracotta = Color(0xFFC05C49)
    val sage = Color(0xFF708272)
    val mutedGold = Color(0xFFC29B62)

    fun bg(isDark: Boolean) = if (isDark) espresso else parchment
    fun surface(isDark: Boolean) = if (isDark) charcoal else alabaster
    
    fun textHigh(isDark: Boolean) = if (isDark) parchment else espresso
    fun textMedium(isDark: Boolean) = if (isDark) alabaster.copy(alpha = 0.7f) else charcoal.copy(alpha = 0.7f)
    fun textLow(isDark: Boolean) = if (isDark) alabaster.copy(alpha = 0.4f) else charcoal.copy(alpha = 0.4f)

    fun stroke(isDark: Boolean) = if (isDark) Color.White.copy(alpha = 0.15f) else Color.Black.copy(alpha = 0.15f)
}

object CRMotion {
    val elegant = spring<Float>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessLow)
    val snappy = spring<Float>(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium)
}

@Composable
fun PaperGrainOverlay(isDark: Boolean) {
    val color = if (isDark) Color.White.copy(alpha = 0.02f) else Color.Black.copy(alpha = 0.02f)
    androidx.compose.foundation.Canvas(modifier = Modifier.fillMaxSize()) {
        val spacing = 3.dp.toPx()
        var y = 0f
        while (y < size.height) {
            var x = 0f
            while (x < size.width) {
                // A very subtle dotted grid to simulate fine paper texture
                drawRect(
                    color = color,
                    topLeft = Offset(x, y),
                    size = androidx.compose.ui.geometry.Size(1f, 1f)
                )
                x += spacing
            }
            y += spacing
        }
    }
}

@Composable
fun CRBackground(isDark: Boolean, content: @Composable () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(CRTheme.bg(isDark))
    ) {
        PaperGrainOverlay(isDark = isDark)
        content()
    }
}

@Composable
fun SectionHeader(isDark: Boolean, title: String, modifier: Modifier = Modifier) {
    Column(modifier = modifier.padding(vertical = 12.dp)) {
        Text(
            text = title.uppercase(),
            fontSize = 11.sp,
            fontWeight = FontWeight.Medium,
            color = CRTheme.textMedium(isDark),
            letterSpacing = 2.sp
        )
        Box(
            modifier = Modifier
                .padding(top = 4.dp)
                .fillMaxWidth()
                .height(0.5.dp)
                .background(CRTheme.stroke(isDark))
        )
    }
}

fun Modifier.crCard(
    isDark: Boolean,
    cornerRadius: Dp = 2.dp,
    highlighted: Boolean = false,
    accentColor: Color = CRTheme.terracotta,
    onClick: (() -> Unit)? = null
): Modifier = composed {
    val shape = RoundedCornerShape(cornerRadius)
    val borderColor = if (highlighted) accentColor else CRTheme.stroke(isDark)
    val shadowColor = if (isDark) Color.Black.copy(alpha = 0.3f) else Color.Black.copy(alpha = 0.05f)

    var modifier = this
        .shadow(
            elevation = if (highlighted) 12.dp else 4.dp,
            shape = shape,
            ambientColor = shadowColor,
            spotColor = shadowColor
        )
        .clip(shape)
        .background(CRTheme.surface(isDark))
        .border(if (highlighted) 1.5.dp else 0.5.dp, borderColor, shape)
        
    if (onClick != null) {
        modifier = modifier.clickable { onClick() }
    }
    
    modifier
}

fun Modifier.crButton(
    isDark: Boolean,
    color: Color = CRTheme.terracotta,
    cornerRadius: Dp = 2.dp,
    onClick: () -> Unit
): Modifier = composed {
    this
        .shadow(8.dp, RoundedCornerShape(cornerRadius), spotColor = Color.Black.copy(alpha = 0.1f))
        .clip(RoundedCornerShape(cornerRadius))
        .background(color)
        .clickable { onClick() }
}

@Composable
fun CRSwitch(checked: Boolean, isDark: Boolean) {
    val thumbColor = if (checked) CRTheme.bg(isDark) else CRTheme.textMedium(isDark)
    val trackColor = if (checked) CRTheme.textHigh(isDark) else Color.Transparent

    Box(
        modifier = Modifier
            .width(44.dp)
            .height(24.dp)
            .border(1.dp, if (checked) Color.Transparent else CRTheme.stroke(isDark), RoundedCornerShape(2.dp))
            .background(trackColor, RoundedCornerShape(2.dp))
            .padding(2.dp)
    ) {
        Box(
            modifier = Modifier
                .offset(x = if (checked) 20.dp else 0.dp)
                .size(20.dp)
                .background(thumbColor, RoundedCornerShape(1.dp))
        )
    }
}
