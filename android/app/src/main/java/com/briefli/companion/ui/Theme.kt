package com.briefli.companion.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.sp

private val BriefliColors = lightColorScheme(
    primary = Color(0xFF176B52),
    onPrimary = Color.White,
    secondary = Color(0xFFB14C2E),
    background = Color(0xFFF4F7F5),
    surface = Color(0xFFFFFFFF),
    surfaceVariant = Color(0xFFE4E9E6),
    onBackground = Color(0xFF1C201D),
    onSurface = Color(0xFF1C201D),
)

private val BriefliTypography = Typography().let { defaults ->
    defaults.copy(
        headlineLarge = defaults.headlineLarge.copy(
            fontFamily = FontFamily.Serif,
            fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
            letterSpacing = 0.sp,
        ),
        displaySmall = defaults.displaySmall.copy(letterSpacing = 0.sp),
        titleLarge = defaults.titleLarge.copy(letterSpacing = 0.sp),
        titleMedium = defaults.titleMedium.copy(letterSpacing = 0.sp),
        bodyLarge = defaults.bodyLarge.copy(letterSpacing = 0.sp),
        bodySmall = defaults.bodySmall.copy(letterSpacing = 0.sp),
        labelLarge = defaults.labelLarge.copy(letterSpacing = 0.sp),
        labelMedium = defaults.labelMedium.copy(letterSpacing = 0.sp),
    )
}

@Composable
fun BriefliTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = BriefliColors,
        typography = BriefliTypography,
        content = content,
    )
}
