package dev.horizon.android.launcher

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/**
 * The native-mods section of the Mods page. A `.hmod` may carry content only,
 * or content plus native source-only Rust (legacy `.hnmod` carried native only).
 * Native mod building is not supported on Android: there is no upstream Android
 * SDK slice to rebuild the game. Native mods use the `mod_api` SDK and `.hmod`
 * packages on supported hosts. Bundles imported here are source-only on Android:
 * nothing is compiled or executed on this device.
 */
@Composable
fun NativeModsSection(native: NativeMods, actions: LauncherActions) {
    var rejectedDismissed by remember { mutableStateOf(false) }
    var detailsMod by remember { mutableStateOf<NativeMod?>(null) }

    val current = native.current

    Card(
        shape = RoundedCornerShape(24.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    "Native source bundles",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                FilledTonalButton(onClick = actions.importNative) {
                    Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Import")
                }
            }
            current?.takeIf { it.rejected && !rejectedDismissed }?.let {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(
                        Icons.Rounded.Warning,
                        null,
                        tint = MaterialTheme.colorScheme.error,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(Modifier.width(8.dp))
                    val reason = it.rejectReason?.takeIf { text -> text.isNotBlank() }
                    Text(
                        reason ?: "A modded build was set aside after crashing before its first frame. Playing the stock game. See docs/android-native-mods.md.",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = { rejectedDismissed = true }) {
                        Icon(Icons.Rounded.Clear, "Dismiss")
                    }
                }
            }
            if (native.mods.isEmpty()) {
                Text(
                    "Import .hmod or legacy .hnmod source bundles here. Android validates and stores native source, but does not compile or run it. See docs/android-native-mods.md.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                native.mods.forEach { mod ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(
                            Modifier
                                .weight(1f)
                                .combinedClickable(
                                    onClick = { },
                                    onLongClick = { detailsMod = mod },
                                ),
                        ) {
                            Text(
                                mod.name,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                fontWeight = FontWeight.Medium,
                            )
                            Text(
                                "${mod.version} · ${formatSize(mod.size)} · long-press for details",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.outline,
                            )
                            Text(
                                stateText(mod),
                                style = MaterialTheme.typography.labelMedium,
                                color = if (mod.state == "rejected") {
                                    MaterialTheme.colorScheme.error
                                } else if (mod.state == "selected") {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    MaterialTheme.colorScheme.outline
                                },
                            )
                        }
                        Spacer(Modifier.width(4.dp))
                        IconButton(onClick = { actions.removeNative(mod.id) }) {
                            Icon(Icons.Rounded.DeleteOutline, "Remove", tint = MaterialTheme.colorScheme.error)
                        }
                    }
                }
            }
        }
    }
    detailsMod?.let { mod ->
        AlertDialog(
            onDismissRequest = { detailsMod = null },
            title = { Text("Native source bundle") },
            text = {
                Text(
                    "id: ${mod.id}\nversion: ${mod.version}\n\nThis source bundle is not compiled or run on Android.",
                )
            },
            confirmButton = { TextButton(onClick = { detailsMod = null }) { Text("Close") } },
        )
    }
}

private fun formatSize(bytes: Long): String = when {
    bytes >= 1024L * 1024L -> "%.1f MiB".format(bytes / (1024.0 * 1024.0))
    bytes >= 1024L -> "%.1f KiB".format(bytes / 1024.0)
    else -> "$bytes B"
}

private fun stateText(mod: NativeMod): String = when {
    mod.state == "rejected" -> "Source bundle · related build set aside"
    mod.state == "invalid" -> "Invalid source bundle"
    mod.state == "selected" -> "Source bundle · separate library selected"
    else -> "Imported source · not runnable on Android"
}
