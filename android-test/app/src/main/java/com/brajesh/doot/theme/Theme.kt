package com.brajesh.doot.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val DarkColorScheme = darkColorScheme(
    primary = DootBluePrimaryDark,
    onPrimary = DootBlueOnPrimaryContainer,
    primaryContainer = Color(0xFF0842A0),
    onPrimaryContainer = DootBluePrimaryContainer,
    secondary = DootSecondaryDark,
    onSecondary = DootOnSecondaryContainer,
    secondaryContainer = Color(0xFF004D73),
    onSecondaryContainer = DootSecondaryContainer,
    tertiary = DootTertiaryDark,
    onTertiary = Color(0xFF003919),
    tertiaryContainer = Color(0xFF0F5223),
    onTertiaryContainer = Color(0xFFC4EED0),
    background = DootBackgroundDark,
    surface = DootSurfaceDark,
    surfaceContainerLowest = Color(0xFF0C0E12),
    surfaceContainerLow = Color(0xFF14161C),
    surfaceContainer = DootSurfaceContainerDark,
    surfaceContainerHigh = DootSurfaceContainerHighDark,
    surfaceContainerHighest = Color(0xFF33353A)
)

private val LightColorScheme = lightColorScheme(
    primary = DootBluePrimary,
    onPrimary = DootBlueOnPrimary,
    primaryContainer = DootBluePrimaryContainer,
    onPrimaryContainer = DootBlueOnPrimaryContainer,
    secondary = DootSecondary,
    onSecondary = Color(0xFFFFFFFF),
    secondaryContainer = DootSecondaryContainer,
    onSecondaryContainer = DootOnSecondaryContainer,
    tertiary = DootTertiary,
    onTertiary = Color(0xFFFFFFFF),
    tertiaryContainer = DootTertiaryContainer,
    onTertiaryContainer = Color(0xFF07210E),
    background = DootBackgroundLight,
    surface = DootSurfaceLight,
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFF7F9FC),
    surfaceContainer = DootSurfaceContainerLight,
    surfaceContainerHigh = DootSurfaceContainerHighLight,
    surfaceContainerHighest = Color(0xFFE1E6EE)
)

@OptIn(androidx.compose.material3.ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun DootTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    dynamicColor: Boolean = true,
    content: @Composable () -> Unit
) {
    val colorScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            val context = LocalContext.current
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        }
        darkTheme -> DarkColorScheme
        else -> LightColorScheme
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        motionScheme = androidx.compose.material3.MotionScheme.expressive(),
        content = content
    )
}

