package dev.horizon.android.launcher.settings.sections

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Archive
import androidx.compose.material.icons.rounded.AspectRatio
import androidx.compose.material.icons.rounded.Badge
import androidx.compose.material.icons.rounded.BugReport
import androidx.compose.material.icons.rounded.Dns
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Fullscreen
import androidx.compose.material.icons.rounded.Gamepad
import androidx.compose.material.icons.rounded.HighQuality
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Insights
import androidx.compose.material.icons.rounded.Memory
import androidx.compose.material.icons.rounded.Monitor
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PhotoLibrary
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.ScreenRotation
import androidx.compose.material.icons.rounded.Smartphone
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.Storage
import androidx.compose.material.icons.rounded.Timer
import androidx.compose.material.icons.rounded.TouchApp
import androidx.compose.material.icons.rounded.UnfoldMore
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.horizon.android.R
import dev.horizon.android.launcher.HorizonSwitch
import dev.horizon.android.launcher.LauncherActions
import dev.horizon.android.launcher.SettingRow
import dev.horizon.android.launcher.Snapshot
import dev.horizon.android.launcher.act
import dev.horizon.android.launcher.connectedShape
import dev.horizon.android.launcher.hasAllFilesAccess
import dev.horizon.android.launcher.openAllFilesSettings

internal fun rowIcon(id: String): ImageVector = when (id) {
    "renderer" -> Icons.Rounded.Monitor
    "render_scale" -> Icons.Rounded.HighQuality
    "display_hz" -> Icons.Rounded.Speed
    "performance_hints" -> Icons.Rounded.Memory
    "performance_presets" -> Icons.Rounded.Speed
    "appearance" -> Icons.Rounded.Palette
    "screenshots" -> Icons.Rounded.PhotoLibrary
    "worker_limit" -> Icons.Rounded.Memory
    "cache_budget_mib" -> Icons.Rounded.Storage
    "storage" -> Icons.Rounded.FolderOpen
    "game_files" -> Icons.Rounded.Folder
    "backup" -> Icons.Rounded.Storage
    "world_files" -> Icons.Rounded.Public
    "orientation" -> Icons.Rounded.ScreenRotation
    "aspect" -> Icons.Rounded.AspectRatio
    "immersive" -> Icons.Rounded.Fullscreen
    "cutout" -> Icons.Rounded.Smartphone
    "profile" -> Icons.Rounded.Person
    "input_host" -> Icons.Rounded.Gamepad
    "touch_layouts" -> Icons.Rounded.TouchApp
    "name" -> Icons.Rounded.Badge
    "server" -> Icons.Rounded.Dns
    "servers" -> Icons.Rounded.Dns
    "gpu_profile" -> Icons.Rounded.BugReport
    "record_performance" -> Icons.Rounded.Timer
    "export_diagnostics" -> Icons.Rounded.Archive
    "export_performance" -> Icons.Rounded.Insights
    else -> Icons.Rounded.History
}

/** The common connected list used by every category detail page. */
@Composable
fun SettingsRowList(
    rows: List<SettingRow>,
    state: Snapshot,
    actions: LauncherActions,
    extraWide: Boolean,
    onStorage: () -> Unit,
    onGameFiles: () -> Unit,
    leadingContent: @Composable () -> Unit = {},
) {
    val serverHint = stringResource(R.string.edit_hint_server)
    val nameHint = stringResource(R.string.edit_hint_name)
    Column(verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(2.dp)) {
        leadingContent()
        rows.forEachIndexed { i, row ->
            Surface(
                shape = connectedShape(i, rows.size, outer = 24),
                color = MaterialTheme.colorScheme.surfaceContainer,
                onClick = { onRowClick(row, state, actions, onStorage, onGameFiles, serverHint, nameHint) },
            ) {
                ListItem(
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                    modifier = Modifier.heightIn(min = if (extraWide) 72.dp else 56.dp),
                    leadingContent = {
                        Surface(
                            shape = RoundedCornerShape(14.dp),
                            color = MaterialTheme.colorScheme.secondaryContainer,
                            modifier = Modifier.size(if (extraWide) 48.dp else 40.dp),
                        ) {
                            Box(contentAlignment = Alignment.Center) {
                                Icon(
                                    rowIcon(row.id),
                                    null,
                                    modifier = Modifier.size(if (extraWide) 26.dp else 24.dp),
                                    tint = MaterialTheme.colorScheme.onSecondaryContainer,
                                )
                            }
                        }
                    },
                    headlineContent = {
                        Text(
                            displayTitle(row),
                            fontWeight = androidx.compose.ui.text.font.FontWeight.Medium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    },
                    supportingContent = { Text(row.value, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                    trailingContent = {
                        when (row.kind) {
                            "switch" -> HorizonSwitch(checked = row.on, onCheckedChange = { act("toggle", row.id) })
                            "choice" -> Icon(Icons.Rounded.UnfoldMore, null)
                            else -> Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null)
                        }
                    },
                )
            }
        }
    }
}

/** The gallery row is renamed in the launcher, independent of the native label. */
@Composable
private fun displayTitle(row: SettingRow): String =
    if (row.id == "screenshots") stringResource(R.string.settings_screenshots_title) else row.title

/** Exact click behavior of the previous flat settings screen — no new bridge calls. */
private fun onRowClick(
    row: SettingRow,
    state: Snapshot,
    actions: LauncherActions,
    onStorage: () -> Unit,
    onGameFiles: () -> Unit,
    serverHint: String,
    nameHint: String,
) {
    when (row.kind) {
        "switch" -> act("toggle", row.id)
        "choice" -> actions.choose(row.id)
        "text" -> actions.edit(
            row.id,
            row.title,
            if (row.id == "name") state.config.name else state.config.server,
            if (row.id == "server") serverHint else nameHint,
        )
        else -> when (row.id) {
            "performance_presets" -> actions.openPerformance()
            "touch_layouts" -> actions.openTouchLayouts()
            "servers" -> actions.openServers()
            "storage" -> onStorage()
            "screenshots" -> actions.openScreenshots()
            "appearance" -> actions.openAppearance()
            "game_files" -> onGameFiles()
            "backup" -> actions.openBackup()
            else -> act(row.id)
        }
    }
}

/** Storage access status with a shortcut back to the system all-files page. */
@Composable
fun StorageAccessRow(extraWide: Boolean, onChanged: () -> Unit) {
    val context = LocalContext.current
    val granted = hasAllFilesAccess(context)
    val label = stringResource(
        if (granted) R.string.settings_storage_granted else R.string.settings_storage_revoked,
    )
    Surface(
        shape = RoundedCornerShape(18.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
        modifier = Modifier.fillMaxWidth().heightIn(min = if (extraWide) 72.dp else 56.dp),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                if (granted) Icons.Rounded.FolderOpen else Icons.Rounded.Folder,
                null,
                tint = MaterialTheme.colorScheme.onSecondaryContainer,
            )
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.settings_storage_manage), fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
                Text(
                    label,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                )
            }
            androidx.compose.material3.TextButton(onClick = {
                openAllFilesSettings(context)
                onChanged()
            }) { Text(stringResource(R.string.settings_storage_open)) }
        }
    }
}

/** A shortcut tile that runs an existing launcher action (no new bridge method). */
@Composable
fun ShortcutRow(
    icon: ImageVector,
    label: String,
    detail: String,
    extraWide: Boolean,
    onClick: () -> Unit,
) {
    Surface(
        shape = RoundedCornerShape(18.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
        onClick = onClick,
        modifier = Modifier.fillMaxWidth().heightIn(min = if (extraWide) 72.dp else 56.dp),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(icon, null, tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(label, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
                Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null)
        }
    }
}
