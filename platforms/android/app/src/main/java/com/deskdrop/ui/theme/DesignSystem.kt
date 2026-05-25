package com.deskdrop.ui.theme

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.random.Random

object CRTheme {
    // Structural Neutrals (Proper Contrast)
    val bgLight = Color(0xFFF5F6FA)
    val bgDark = Color(0xFF000000)
    
    val surfaceLight = Color.White.copy(alpha = 0.72f)
    val surfaceDark = Color(0xFF1C1C1E).copy(alpha = 0.72f)
    
    val surfaceElevatedLight = Color.White.copy(alpha = 0.9f)
    val surfaceElevatedDark = Color(0xFF2C2C2E).copy(alpha = 0.9f)
    
    val textHighLight = Color(0xFF111827)
    val textHighDark = Color(0xFFF9FAFB)
    val textMediumLight = Color(0xFF6B7280)
    val textMediumDark = Color(0xFFA1A1AA)
    
    val strokeLight = Color(0xFFE5E7EB)
    val strokeDark = Color(0xFF27272A)
    
    // STRICT ACCENT PALETTE
    val indigoSoft = Color(0xFF6366F1)
    val cyanSoft = Color(0xFF06B6D4)
    val blueGray = Color(0xFF4B5563)

    // Status
    val statusGreen = Color(0xFF10B981)
    val statusAmber = Color(0xFFF59E0B)
    val statusRed = Color(0xFFEF4444)

    fun bg(isDark: Boolean) = if (isDark) bgDark else bgLight
    fun surface(isDark: Boolean) = if (isDark) surfaceDark else surfaceLight
    fun surfaceElevated(isDark: Boolean) = if (isDark) surfaceElevatedDark else surfaceElevatedLight
    fun textHigh(isDark: Boolean) = if (isDark) textHighDark else textHighLight
    fun textMedium(isDark: Boolean) = if (isDark) textMediumDark else textMediumLight
    fun stroke(isDark: Boolean) = if (isDark) strokeDark else strokeLight
    fun glass(isDark: Boolean) = if (isDark) Color.White.copy(alpha = 0.03f) else Color.White.copy(alpha = 0.6f)

    // Legacy Aliases
    val brandElectric = indigoSoft
    val brandViolet = indigoSoft
    val brandCyan = cyanSoft
    val brandPink = statusRed
    val accentGreen = statusGreen
    val accentAmber = statusAmber
    val accentRed = statusRed
    fun textLow(isDark: Boolean) = textMedium(isDark).copy(alpha = 0.5f)
    
    // Ambient Ecosystem Glow Colors
    val ambientLavender = Color(0xFFC4B5FD) // soft violet
    val ambientMint = Color(0xFF6EE7B7) // soft mint
    val ambientSky = Color(0xFF7DD3FC) // soft sky blue
}

object CRTypography {
    val h1 = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 28.sp,
        letterSpacing = (-0.5).sp
    )
    val h2 = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 18.sp,
        letterSpacing = (-0.3).sp
    )
    val bodyMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 15.sp
    )
    val label = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 14.sp,
        letterSpacing = 0.2.sp
    )
    val caption = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 12.sp,
        letterSpacing = 0.3.sp
    )
}

fun Modifier.crPressScale(
    targetScale: Float = 0.96f,
    onClick: (() -> Unit)? = null
): Modifier = composed {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed) targetScale else 1f,
        animationSpec = spring(dampingRatio = 0.6f, stiffness = 400f),
        label = "press_scale"
    )

    var mod = this.scale(scale)
    if (onClick != null) {
        mod = mod.clickable(
            interactionSource = interactionSource,
            indication = null,
            onClick = onClick
        )
    }
    mod
}


object CRMotion {
    val snappy = spring<Float>(dampingRatio = 0.8f, stiffness = 400f)
    val fluid = spring<Float>(dampingRatio = 0.7f, stiffness = 200f)
}

@Composable
fun SubtleNoiseOverlay(isDark: Boolean) {
    val color = if (isDark) Color.White.copy(alpha = 0.02f) else Color.Black.copy(alpha = 0.025f)
    val colorSecondary = if (isDark) Color.White.copy(alpha = 0.01f) else Color.Black.copy(alpha = 0.015f)
    
    androidx.compose.foundation.Canvas(modifier = Modifier.fillMaxSize()) {
        val spacing = 3.dp.toPx()
        var y = 0f
        while (y < size.height) {
            var x = 0f
            while (x < size.width) {
                if (Random.nextBoolean()) {
                    drawRect(
                        color = if (Random.nextBoolean()) color else colorSecondary,
                        topLeft = Offset(x, y),
                        size = androidx.compose.ui.geometry.Size(2f, 2f)
                    )
                }
                x += spacing
            }
            y += spacing
        }
    }
}

@Composable
fun CRBackground(isDark: Boolean, hasConnectedDevices: Boolean = false, content: @Composable () -> Unit) {
    val infiniteTransition = rememberInfiniteTransition(label = "mesh")
    val breatheShift by infiniteTransition.animateFloat(
        initialValue = 0.95f, targetValue = 1.05f,
        animationSpec = infiniteRepeatable(tween(8000, easing = LinearEasing), RepeatMode.Reverse),
        label = "mesh_breathe"
    )

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(CRTheme.bg(isDark))
    ) {
        // Single controlled soft light source using radial gradient (100x faster than blur)
        androidx.compose.foundation.Canvas(modifier = Modifier.fillMaxSize()) {
            val centerOffset = Offset(size.width * 0.5f, size.height * 0.25f)
            
            if (hasConnectedDevices) {
                // Signature ambient glow when devices are connected
                val radius = size.width * 0.85f * breatheShift
                val color = CRTheme.indigoSoft.copy(alpha = if (isDark) 0.12f else 0.08f)
                drawCircle(
                    brush = androidx.compose.ui.graphics.Brush.radialGradient(
                        colors = listOf(color, Color.Transparent),
                        center = centerOffset,
                        radius = radius
                    ),
                    radius = radius,
                    center = centerOffset
                )
            } else {
                val radius = size.width * 0.7f
                val color = (if (isDark) Color.White else Color.Black).copy(alpha = 0.03f)
                drawCircle(
                    brush = androidx.compose.ui.graphics.Brush.radialGradient(
                        colors = listOf(color, Color.Transparent),
                        center = centerOffset,
                        radius = radius
                    ),
                    radius = radius,
                    center = centerOffset
                )
            }
        }
        SubtleNoiseOverlay(isDark = isDark)
        content()
    }
}

fun Modifier.crGlassCard(
    isDark: Boolean,
    cornerRadius: Dp = 16.dp,
    highlighted: Boolean = false,
    dashed: Boolean = false,
    onClick: (() -> Unit)? = null,
    elevated: Boolean = false
): Modifier = composed {
    val shape = RoundedCornerShape(cornerRadius)
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    
    val scale by animateFloatAsState(
        targetValue = if (isPressed && onClick != null) 0.96f else 1f,
        animationSpec = CRMotion.snappy,
        label = "press_scale"
    )

    val shadowColor = if (isDark) Color.Black.copy(alpha = 0.6f) else Color.Black.copy(alpha = 0.06f)
    val borderColor = if (highlighted) CRTheme.indigoSoft.copy(alpha = 0.4f) else CRTheme.stroke(isDark).copy(alpha = if (isDark) 0.8f else 0.5f)
    val bgColor = if (elevated) CRTheme.surfaceElevated(isDark) else CRTheme.surface(isDark)

    var modifier = this
        .scale(scale)
        .shadow(
            elevation = if (onClick != null) (if (isPressed) 1.dp else 6.dp) else (if (elevated) 8.dp else 4.dp),
            shape = shape,
            ambientColor = shadowColor,
            spotColor = shadowColor
        )
        .clip(shape)
        .background(bgColor)
        .background(CRTheme.glass(isDark))

    if (onClick != null) {
        modifier = modifier.clickable(
            interactionSource = interactionSource,
            indication = null,
            onClick = onClick
        )
    }

    if (dashed) {
        modifier.border(1.dp, borderColor.copy(alpha = 0.5f), shape)
    } else {
        modifier.border(1.dp, borderColor, shape)
    }
}

// Legacy Alias
fun Modifier.crCard(
    isDark: Boolean,
    cornerRadius: Dp = 24.dp,
    highlighted: Boolean = false,
    accentColor: Color = CRTheme.indigoSoft,
    onClick: (() -> Unit)? = null
): Modifier = crGlassCard(
    isDark = isDark,
    cornerRadius = cornerRadius,
    highlighted = highlighted,
    dashed = false,
    onClick = onClick
)

@Composable
fun CRSwitch(checked: Boolean, isDark: Boolean) {
    val thumbColor = if (checked) CRTheme.bg(isDark) else CRTheme.textMedium(isDark)
    val trackColor = if (checked) CRTheme.textHigh(isDark) else Color.Transparent

    Box(
        modifier = Modifier
            .width(44.dp)
            .height(24.dp)
            .border(1.dp, if (checked) Color.Transparent else CRTheme.stroke(isDark), RoundedCornerShape(12.dp))
            .background(trackColor, RoundedCornerShape(12.dp))
            .padding(2.dp)
    ) {
        Box(
            modifier = Modifier
                .offset(x = if (checked) 20.dp else 0.dp)
                .size(20.dp)
                .background(thumbColor, RoundedCornerShape(10.dp))
        )
    }
}
