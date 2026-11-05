package dev.horizon.android.launcher

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Photo
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.Wallpaper
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption

/** Draw the chosen launcher background behind every page, with a readable scrim. */
@Composable
fun LauncherBackground(appearance: LauncherAppearance) {
    val path = appearance.backgroundPath
    val background = MaterialTheme.colorScheme.background
    val bitmap by produceState<Bitmap?>(null, path, appearance.backgroundRevision) {
        value = if (path == null) {
            null
        } else {
            withContext(Dispatchers.IO) { decodeBackground(path, 1600) }
        }
    }
    Box(Modifier.fillMaxSize().background(background)) {
        val image = bitmap
        if (image != null) {
            Image(
                bitmap = image.asImageBitmap(),
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
            Box(
                Modifier.fillMaxSize().background(
                    Brush.verticalGradient(
                        listOf(
                            background.copy(alpha = 0.78f),
                            background.copy(alpha = 0.88f),
                            background.copy(alpha = 0.95f),
                        ),
                    ),
                ),
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppearanceSheet(
    appearance: LauncherAppearance,
    pickerError: String?,
    importing: Boolean,
    onDismiss: () -> Unit,
    onChooseBackground: () -> Unit,
    onChanged: (LauncherAppearance) -> Unit,
) {
    var current by remember { mutableStateOf(appearance) }
    var actionError by remember { mutableStateOf<String?>(null) }
    var confirmReset by remember { mutableStateOf(false) }
    LaunchedEffect(appearance) { current = appearance }

    fun applyAction(method: String, arg: String = "") {
        val reply = act(method, arg)
        val value = reply.ok as? JSONObject
        if (reply.error != null || value == null) {
            actionError = reply.error ?: "The launcher did not reply."
            return
        }
        val next = LauncherAppearance.parse(value)
        current = next
        onChanged(next)
        actionError = null
    }

    val themes = current.themes.ifEmpty {
        listOf(
            ThemeChoice("system", "System default"),
            ThemeChoice("light", "Light"),
            ThemeChoice("dark", "Dark"),
        )
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(start = 20.dp, end = 20.dp, bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                "Launcher appearance",
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.SemiBold,
            )
            Text(
                "Changes apply immediately and are saved for the launcher.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Spacer(Modifier.height(8.dp))
            Text("Theme", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            Row(
                Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                themes.forEach { theme ->
                    FilterChip(
                        selected = current.theme == theme.key,
                        onClick = { applyAction("appearance_theme", theme.key) },
                        label = { Text(theme.label) },
                        leadingIcon = {
                            if (current.theme == theme.key) {
                                Icon(Icons.Rounded.Check, null, Modifier.size(16.dp))
                            } else {
                                Spacer(Modifier.size(16.dp))
                            }
                        },
                    )
                }
            }

            Spacer(Modifier.height(8.dp))
            Text("Accent color", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            Text(
                "Text and controls keep a contrast-checked color pair.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Row(
                Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                current.accents.forEach { accent ->
                    FilterChip(
                        selected = current.accent == accent.key,
                        onClick = { applyAction("appearance_accent", accent.key) },
                        label = { Text(accent.label) },
                        leadingIcon = {
                            AccentDot(accent, selected = current.accent == accent.key)
                        },
                    )
                }
            }
            if (current.accents.isEmpty()) {
                Text(
                    "Accent choices are unavailable. Reopen the launcher to retry.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }

            Spacer(Modifier.height(8.dp))
            Text("Background", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            BackgroundPreview(current)
            if (current.backgroundMissing) {
                Text(
                    "The saved image is missing or inaccessible. Horizon is using the default background.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            } else if (current.hasCustomBackground) {
                Text(
                    "A private copy is stored for the launcher, so moving the original does not break it.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                Text(
                    "The default background follows the active theme.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            FilledTonalButton(
                onClick = onChooseBackground,
                enabled = !importing,
                modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
            ) {
                Icon(Icons.Rounded.Wallpaper, null, Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(if (importing) "Importing image…" else "Choose image")
            }
            OutlinedButton(
                onClick = { applyAction("appearance_background_reset") },
                enabled = !importing && (current.hasCustomBackground || current.backgroundMissing),
                modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
            ) {
                Text("Use default background")
            }
            if (importing) {
                Text(
                    "Reading the selected image and storing a private copy.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            Spacer(Modifier.height(12.dp))
            HorizontalDivider()
            Spacer(Modifier.height(8.dp))
            OutlinedButton(
                onClick = { confirmReset = true },
                enabled = !importing,
                modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
            ) {
                Icon(Icons.Rounded.RestartAlt, null, Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text("Reset appearance to defaults")
            }
            (pickerError ?: actionError)?.let { message ->
                Text(
                    message,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }

    if (confirmReset) {
        AlertDialog(
            onDismissRequest = { confirmReset = false },
            title = { Text("Reset launcher appearance?") },
            text = {
                Text("Theme, accent color and the custom background will return to their defaults.")
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmReset = false
                    applyAction("appearance_reset")
                }) {
                    Text("Reset")
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmReset = false }) { Text("Cancel") }
            },
        )
    }
}

@Composable
private fun AccentDot(accent: AccentChoice, selected: Boolean) {
    val primary = accent.light?.primary?.toComposeColor() ?: MaterialTheme.colorScheme.primary
    val onPrimary = accent.light?.onPrimary?.toComposeColor() ?: Color.White
    if (accent.key == "system") {
        Box(
            Modifier
                .size(18.dp)
                .clip(CircleShape)
                .background(
                    Brush.sweepGradient(
                        listOf(
                            Color(0xFF65558F),
                            Color(0xFF00696A),
                            Color(0xFF8A5000),
                            Color(0xFF8E3F5A),
                            Color(0xFF65558F),
                        ),
                    ),
                ),
        )
    } else {
        Box(
            Modifier.size(18.dp).clip(CircleShape).background(primary),
            contentAlignment = Alignment.Center,
        ) {
            if (selected) {
                Icon(
                    Icons.Rounded.Check,
                    null,
                    Modifier.fillMaxSize().padding(2.dp),
                    tint = onPrimary,
                )
            }
        }
    }
}

@Composable
private fun BackgroundPreview(appearance: LauncherAppearance) {
    val path = appearance.backgroundPath
    val bitmap by produceState<Bitmap?>(null, path, appearance.backgroundRevision) {
        value = if (path == null) {
            null
        } else {
            withContext(Dispatchers.IO) { decodeBackground(path, 640) }
        }
    }
    Box(
        Modifier
            .fillMaxWidth()
            .height(148.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceContainer),
    ) {
        val image = bitmap
        if (image != null) {
            Image(
                bitmap = image.asImageBitmap(),
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
            Box(
                Modifier.fillMaxSize().background(
                    Brush.verticalGradient(
                        listOf(Color.Transparent, MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)),
                    ),
                ),
            )
        } else {
            Column(
                Modifier.fillMaxSize(),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Icon(
                    if (appearance.backgroundMissing) Icons.Rounded.Wallpaper else Icons.Rounded.Photo,
                    null,
                    Modifier.size(36.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    if (appearance.backgroundMissing) "Saved image unavailable" else "Horizon default",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Text(
            when {
                appearance.backgroundMissing -> "Missing"
                appearance.hasCustomBackground -> "Custom"
                else -> "Default"
            },
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(12.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.82f))
                .padding(horizontal = 8.dp, vertical = 4.dp),
        )
    }
}

private fun decodeBackground(path: String, maxDimension: Int): Bitmap? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(path, bounds)
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
    var sample = 1
    while (bounds.outWidth / sample > maxDimension || bounds.outHeight / sample > maxDimension) {
        sample *= 2
        if (sample > 1 shl 14) return null
    }
    val options = BitmapFactory.Options().apply {
        inSampleSize = sample
        inPreferredConfig = Bitmap.Config.ARGB_8888
    }
    return BitmapFactory.decodeFile(path, options)
}

/**
 * Copy and downscale a picked image while the picker is still fresh. The app
 * owns this bounded copy, so a later move/delete of the original cannot break
 * the launcher and decoding never allocates the original image at full size.
 */
suspend fun importLauncherBackground(context: Context, uri: Uri): Result<Unit> =
    withContext(Dispatchers.IO) {
        val partial = File(context.filesDir, "launcher-background.partial")
        runCatching {
            val resolver = context.contentResolver
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            resolver.openInputStream(uri)?.use {
                BitmapFactory.decodeStream(it, null, bounds)
            } ?: error("The selected image could not be opened")
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
                error("The selected file is not a supported image")
            }

            var sample = 1
            while (bounds.outWidth / sample > 1600 || bounds.outHeight / sample > 1600) {
                sample *= 2
                if (sample > 1 shl 14) error("The selected image is too large")
            }
            val options = BitmapFactory.Options().apply {
                inSampleSize = sample
                inPreferredConfig = Bitmap.Config.ARGB_8888
            }
            val bitmap = resolver.openInputStream(uri)?.use {
                BitmapFactory.decodeStream(it, null, options)
            } ?: error("The selected image could not be decoded")
            try {
                partial.outputStream().buffered().use { output ->
                    if (!bitmap.compress(Bitmap.CompressFormat.JPEG, 90, output)) {
                        error("The selected image could not be saved")
                    }
                }
                Files.move(
                    partial.toPath(),
                    File(context.filesDir, "launcher-background.jpg").toPath(),
                    StandardCopyOption.REPLACE_EXISTING,
                )
                Unit
            } finally {
                bitmap.recycle()
            }
        }.onFailure {
            partial.delete()
        }
    }
