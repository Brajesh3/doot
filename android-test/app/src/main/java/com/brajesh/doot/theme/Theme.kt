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
    primaryContainer = Color(0xFF004A77),
    onPrimaryContainer = DootBluePrimaryContainer,
    secondary = DootSecondaryDark,
    onSecondary = DootOnSecondaryContainer,
    secondaryContainer = Color(0xFF004D73),
    onSecondaryContainer = DootSecondaryContainer,
    tertiary = DootTertiaryDark,
    background = DootBackgroundDark,
    surface = DootSurfaceDark,
    surfaceContainer = DootSurfaceContainerDark,
    surfaceContainerHigh = DootSurfaceContainerHighDark
)

private val LightColorScheme = lightColorScheme(
    primary = DootBluePrimary,
    onPrimary = DootBlueOnPrimary,
    primaryContainer = DootBluePrimaryContainer,
    onPrimaryContainer = DootBlueOnPrimaryContainer,
    secondary = DootSecondary,
    secondaryContainer = DootSecondaryContainer,
    onSecondaryContainer = DootOnSecondaryContainer,
    tertiary = DootTertiary,
    background = DootBackgroundLight,
    surface = DootSurfaceLight,
    surfaceContainer = DootSurfaceContainerLight,
    surfaceContainerHigh = DootSurfaceContainerHighLight
)

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
        content = content
    )
}
