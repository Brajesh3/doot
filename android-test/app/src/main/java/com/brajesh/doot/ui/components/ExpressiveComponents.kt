package com.brajesh.doot.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun Modifier.expressivePressScale(
    interactionSource: MutableInteractionSource,
    pressedScale: Float = 0.95f
): Modifier {
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed) pressedScale else 1f,
        animationSpec = MaterialTheme.motionScheme.fastEffectsSpec(),
        label = "press_scale"
    )
    return this.scale(scale)
}
