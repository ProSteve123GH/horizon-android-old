package dev.horizon.android.launcher

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Build
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Warning

/**
 * The native-mods section of the Mods page. Native (`.hnmod`) bundles are
 * source-only Rust that must be compiled into the game; this build cannot
 * compile one, so only import, listing, removal and "play stock" work.
 */
@Composable
fun NativeModsSection(native: NativeMods, actions: LauncherActions) {
    var rejectedDismissed by remember { mutableStateOf(false) }
    val current = native.current
    Card(
        shape = RoundedCornerShape(24.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    "Native mods (.hnmod)",
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
                    Text(
                        "A modded build was set aside after crashing before its first frame. Playing the stock game. See docs/android-native-mods.md.",
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
                    ".hnmod bundles are imported here. They only run once a mod build service is added to this fork; see docs/android-native-mods.md.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                native.mods.forEach { mod ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(
                                mod.name,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                fontWeight = FontWeight.Medium,
                            )
                            Text(
                                "${mod.version} · ${formatSize(mod.size)}",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.outline,
                            )
                            Text(
                                stateText(mod.enabledState),
                                style = MaterialTheme.typography.labelMedium,
                                color = if (mod.enabledState == "selected") {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    MaterialTheme.colorScheme.outline
                                },
                            )
                        }
                        IconButton(onClick = { actions.removeNative(mod.id) }) {
                            Icon(Icons.Rounded.DeleteOutline, "Remove", tint = MaterialTheme.colorScheme.error)
                        }
                    }
                }
            }
            if (current != null) {
                Text(
                    if (current.rejected) {
                        "Set aside: ${current.id} ${current.version}"
                    } else {
                        "Runs on the next start: ${current.id} ${current.version}"
                    },
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                OutlinedButton(onClick = actions.disableNative, modifier = Modifier.fillMaxWidth()) {
                    Text("Play without native mods")
                }
            }
            OutlinedButton(onClick = {}, enabled = false, modifier = Modifier.fillMaxWidth()) {
                Icon(Icons.Rounded.Build, null, Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Compile (requires build service)")
            }
            Text(
                "This build of Horizon has no Horizon mod build service. Native mods need a service that recompiles the game; nothing can build them on the phone yet.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

private fun formatSize(bytes: Long): String = when {
    bytes >= 1024L * 1024L -> "%.1f MiB".format(bytes / (1024.0 * 1024.0))
    bytes >= 1024L -> "%.1f KiB".format(bytes / 1024.0)
    else -> "$bytes B"
}

private fun stateText(state: String): String = when (state) {
    "selected" -> "Selected — runs on the next start"
    "rejected" -> "Set aside after crashing twice"
    "invalid" -> "Invalid bundle"
    else -> "Imported"
}
