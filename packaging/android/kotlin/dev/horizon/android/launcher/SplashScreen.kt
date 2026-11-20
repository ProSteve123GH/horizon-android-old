// Owned by the Compose launcher redesign (Android fork). This file holds the
// cold-start splash and the small loading-state types shared with LauncherApp.
package dev.horizon.android.launcher

import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * The lifecycle of the very first native snapshot. Kept here, next to the splash that
 * consumes it, so [LauncherApp] can drive one state machine for both the overlay and
 * the page underneath. [Failed] is terminal only until a retry or the poll recovers.
 */
internal sealed interface SnapshotState {
    data object Loading : SnapshotState
    data class Ready(val snapshot: Snapshot) : SnapshotState
    data class Failed(val message: String) : SnapshotState
}

/** True when the user turned animations off in Android's accessibility settings. */
@Composable
internal fun rememberReducedMotion(): Boolean {
    val context = LocalContext.current
    return remember(context) {
        runCatching {
            Settings.Global.getFloat(
                context.contentResolver,
                Settings.Global.ANIMATOR_DURATION_SCALE,
                1f,
            ) == 0f
        }.getOrDefault(false)
    }
}

/**
 * Full-screen splash shown while the first snapshot loads. It overlays [LauncherContent]
 * and animates out (fade) once the load reaches either [SnapshotState.Ready] or
 * [SnapshotState.Failed]; on failure the retry panel lives in the mounted scaffold beneath.
 * When the OS animator scale is 0, every duration collapses to zero and nothing animates.
 */
@Composable
internal fun LaunchSplash(visible: Boolean, modifier: Modifier = Modifier) {
    val reduced = rememberReducedMotion()
    val appear: Float by animateFloatAsState(
        targetValue = if (visible) 1f else 0.85f,
        animationSpec = tween<Float>(durationMillis = if (reduced) 0 else 500),
        label = "splash-appear",
    )
    val mark: Dp by animateDpAsState(
        targetValue = if (visible) 96.dp else 80.dp,
        animationSpec = tween<Dp>(durationMillis = if (reduced) 0 else 700),
        label = "splash-mark",
    )
    AnimatedVisibility(
        visible = visible,
        enter = fadeIn(tween<Float>(durationMillis = if (reduced) 0 else 250)),
        exit = fadeOut(tween<Float>(durationMillis = if (reduced) 0 else 350)),
        modifier = modifier,
    ) {
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
            Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    HorizonMark(
                        Modifier
                            .size(mark)
                            .graphicsLayer {
                                alpha = appear
                                scaleX = 0.9f + 0.1f * appear
                                scaleY = 0.9f + 0.1f * appear
                            },
                    )
                    Spacer(Modifier.height(24.dp))
                    LoadingIndicator()
                    Spacer(Modifier.height(12.dp))
                    Text(
                        "Starting Horizon…",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}
