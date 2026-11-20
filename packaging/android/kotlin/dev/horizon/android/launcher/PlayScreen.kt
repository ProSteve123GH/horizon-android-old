package dev.horizon.android.launcher

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Archive
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Dns
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PhotoLibrary
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.RadioButtonUnchecked
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.SportsEsports
import androidx.compose.material.icons.rounded.TouchApp
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.toShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.util.Locale

/**
 * The Play page reads as one dashboard: a status card, the primary action, then labelled groups
 * of shortcuts and game-file controls. On wide windows the groups flow into two columns with the
 * Play action pinned; on phones they stack in reading order.
 */
@Composable
fun PlayScreen(state: Snapshot, wide: Boolean, short: Boolean, actions: LauncherActions) {
    if (wide) {
        Row(
            Modifier.fillMaxSize().padding(bottom = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Column(
                Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                SectionHeader("Game status")
                StatusCard(state, actions, short, Modifier.fillMaxWidth())
                if (!state.install.installing) {
                    SectionHeader("Next steps")
                    StepsCard(state.install.installed, Modifier.fillMaxWidth())
                }
                SectionHeader("Your instance")
                VersionCard(state, onClick = { actions.choose("profile") }, Modifier.fillMaxWidth())
            }
            Column(
                Modifier.widthIn(min = 280.dp, max = 380.dp).fillMaxHeight(),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // These cards scroll on short landscape screens; Play stays pinned at the bottom.
                Column(
                    Modifier.weight(1f).verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    SectionHeader("Quick actions")
                    QuickActionsCard(actions, short)
                    if (!state.install.installing) {
                        SectionHeader("Game files")
                        InstallCard(state.install, actions, Modifier.fillMaxWidth())
                    }
                }
                PlayActionCard(state, actions.play, short, Modifier.fillMaxWidth())
            }
        }
    } else {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            SectionHeader("Game status")
            StatusCard(state, actions, short, Modifier.fillMaxWidth())
            PlayActionCard(state, actions.play, short, Modifier.fillMaxWidth())
            if (!state.install.installing) {
                SectionHeader("Next steps")
                StepsCard(state.install.installed, Modifier.fillMaxWidth())
            }
            SectionHeader("Quick actions")
            QuickActionsCard(actions, short)
            SectionHeader("Game files")
            VersionCard(state, onClick = { actions.choose("profile") }, Modifier.fillMaxWidth())
            if (!state.install.installing) {
                InstallCard(state.install, actions, Modifier.fillMaxWidth())
            }
        }
    }
}

/** Status, version and account at a glance, plus the live install progress when one is running. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun StatusCard(
    state: Snapshot,
    actions: LauncherActions,
    short: Boolean,
    modifier: Modifier,
) {
    val install = state.install
    HorizonCard(modifier = modifier) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    when {
                        install.installing -> "Getting your game ready"
                        install.installed -> "Ready to play"
                        else -> "Not installed yet"
                    },
                    style = if (short) MaterialTheme.typography.titleMedium else MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    "${state.version}  ·  Minecraft 26.3  ·  ${state.config.renderer}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Surface(
                modifier = Modifier.padding(start = 12.dp).size(if (short) 48.dp else 56.dp),
                shape = MaterialShapes.Cookie12Sided.toShape(),
                color = MaterialTheme.colorScheme.primaryContainer,
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(
                        when {
                            install.installing -> Icons.Rounded.Download
                            install.installed -> Icons.Rounded.SportsEsports
                            else -> Icons.Rounded.Download
                        },
                        contentDescription = null,
                        modifier = Modifier.size(if (short) 24.dp else 28.dp),
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                    )
                }
            }
        }
        state.error?.let { ErrorCard(it) }
        AccountRow(state, actions.openAccount)
        if (install.installing) {
            Spacer(Modifier.height(12.dp))
            Text(
                "Installation progress",
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(Modifier.height(4.dp))
            install.stages.forEachIndexed { i, stage ->
                StageRow(stage, done = i < install.stageIndex, current = i == install.stageIndex)
            }
            Spacer(Modifier.height(8.dp))
            LinearWavyProgressIndicator(
                progress = { install.overall.coerceIn(0f, 1f) },
                modifier = Modifier.fillMaxWidth(),
            )
            Text(
                install.detail,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (install.cancelable) {
                TextButton(onClick = actions.cancelImport) { Text("Cancel import") }
            }
        }
    }
}

/** The account strip lives in the status card so identity and install state read together. */
@Composable
private fun AccountRow(state: Snapshot, onClick: () -> Unit) {
    val account = state.account
    val profile = account.profile
    val label = when {
        profile != null && profile.isNotBlank() -> profile
        account.signedIn && account.status.isNotBlank() -> account.status
        account.signedIn -> "Signed in"
        else -> "Not signed in"
    }
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        contentColor = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
    ) {
        Row(
            Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Icon(Icons.Rounded.Person, contentDescription = null, modifier = Modifier.size(18.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    "Account",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(label, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** The three orientation steps, kept verbatim for installed and first-run states. */
@Composable
private fun StepsCard(installed: Boolean, modifier: Modifier) {
    HorizonCard(modifier = modifier) {
        val steps = if (installed) {
            listOf(
                "Open a world or join a Java server",
                "Set up touch in Options › Controls",
                "Find packs, shaders and .hmod in the tabs",
            )
        } else {
            listOf(
                "Download resources from Mojang",
                "Install and verify on this phone",
                "Play offline or sign in for servers",
            )
        }
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            steps.forEachIndexed { i, step -> NumberedStep(i + 1, step) }
        }
    }
}

/** Shortcuts as a 2×2 grid of tiles that collapses to two rows on a phone. */
@Composable
private fun QuickActionsCard(actions: LauncherActions, short: Boolean) {
    HorizonCard(Modifier.fillMaxWidth()) {
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                WorldsButton(actions.openWorlds, Modifier.weight(1f), short)
                ServersButton(actions.openServers, Modifier.weight(1f), short)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                ScreenshotsButton(actions.openScreenshots, Modifier.weight(1f), short)
                TouchLayoutsButton(actions.openTouchLayouts, Modifier.weight(1f), short)
            }
        }
    }
}

/** Import, repair and removal controls, shown when no install is running. */
@Composable
private fun InstallCard(install: Install, actions: LauncherActions, modifier: Modifier) {
    HorizonCard(modifier = modifier) {
        if (!install.installed) {
            ImportOptions(install, actions)
        } else {
            InstalledDetails(install, actions)
        }
    }
}

@Composable
private fun ImportOptions(install: Install, actions: LauncherActions) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            "Already have Minecraft 26.3? Reuse its files from this phone or a ZIP; only missing files download. " +
                "Java libraries and LWJGL are not needed by Horizon.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = actions.importFolder,
                modifier = Modifier.weight(1f),
                contentPadding = PaddingValues(horizontal = 8.dp),
            ) {
                Icon(Icons.Rounded.FolderOpen, null, Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Import folder", maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            OutlinedButton(
                onClick = actions.importZip,
                modifier = Modifier.weight(1f),
                contentPadding = PaddingValues(horizontal = 8.dp),
            ) {
                Icon(Icons.Rounded.Archive, null, Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Import ZIP", maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (install.freeBytes > 0) {
            Text(
                "${formatBytes(install.freeBytes)} free on this device. The Download button remains the official Mojang path.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun InstalledDetails(install: Install, actions: LauncherActions) {
    val source = if (install.source == "downloaded" || install.source.isBlank()) {
        "Downloaded from Mojang"
    } else {
        "Imported from ${install.source}"
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            "$source · ${formatBytes(install.sizeBytes)}",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (install.filesMissing > 0) {
            Text(
                "${install.filesMissing} source files were missing and downloaded automatically.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            TextButton(onClick = actions.repairInstallation) {
                Icon(Icons.Rounded.Refresh, null, Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Repair missing files", maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            TextButton(onClick = actions.removeInstallation) {
                Icon(Icons.Rounded.DeleteOutline, null, Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Remove resources", maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

private fun formatBytes(bytes: Long): String = when {
    bytes >= 1L shl 30 -> String.format(Locale.US, "%.1f GiB", bytes.toDouble() / (1L shl 30))
    bytes >= 1L shl 20 -> String.format(Locale.US, "%.0f MiB", bytes.toDouble() / (1L shl 20))
    else -> "$bytes B"
}

@Composable
fun RemoveInstallationDialog(install: Install, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Rounded.DeleteOutline, null) },
        title = { Text("Remove Minecraft resources?") },
        text = {
            Text(
                "This deletes the prepared Minecraft 26.3 files (${formatBytes(install.sizeBytes)}) " +
                    "so you can import or download them again. Saves, profiles, packs and mods stay.",
            )
        },
        confirmButton = {
            TextButton(
                onClick = {
                    act("remove_installation")
                    onDismiss()
                },
            ) { Text("Remove") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
private fun ErrorCard(message: String) {
    Surface(
        shape = RoundedCornerShape(20.dp),
        color = MaterialTheme.colorScheme.errorContainer,
        modifier = Modifier.fillMaxWidth().padding(top = 12.dp),
    ) {
        Row(Modifier.padding(start = 16.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Rounded.ErrorOutline, null, tint = MaterialTheme.colorScheme.onErrorContainer)
            Text(
                message,
                modifier = Modifier.weight(1f).padding(horizontal = 12.dp),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onErrorContainer,
                maxLines = 4,
                overflow = TextOverflow.Ellipsis,
            )
            IconButton(onClick = { act("dismiss_error") }) {
                Icon(Icons.Rounded.Close, "Dismiss", tint = MaterialTheme.colorScheme.onErrorContainer)
            }
        }
    }
}

@Composable
private fun StageRow(label: String, done: Boolean, current: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Icon(
            if (done) Icons.Rounded.CheckCircle else Icons.Rounded.RadioButtonUnchecked,
            contentDescription = null,
            tint = if (done || current) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline,
            modifier = Modifier.size(18.dp),
        )
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = if (current) FontWeight.SemiBold else FontWeight.Normal,
            color = if (current) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun NumberedStep(number: Int, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Surface(shape = CircleShape, color = MaterialTheme.colorScheme.secondaryContainer, modifier = Modifier.size(26.dp)) {
            Box(contentAlignment = Alignment.Center) {
                Text(
                    number.toString(),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                )
            }
        }
        Text(text, style = MaterialTheme.typography.bodyMedium)
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun VersionCard(state: Snapshot, onClick: () -> Unit, modifier: Modifier) {
    HorizonCard(modifier = modifier, onClick = onClick) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Surface(
                modifier = Modifier.size(40.dp),
                shape = MaterialShapes.Pill.toShape(),
                color = MaterialTheme.colorScheme.tertiaryContainer,
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(Icons.Rounded.Layers, null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
                }
            }
            Column(Modifier.padding(start = 12.dp).weight(1f)) {
                Text("Minecraft 26.3", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                Text(
                    "${state.config.profileName}  ·  ${state.config.renderScale}%  ·  ${state.config.input}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun WorldsButton(onOpen: () -> Unit, modifier: Modifier, short: Boolean) {
    OutlinedButton(
        onClick = onOpen,
        modifier = modifier.height(if (short) 48.dp else 56.dp),
        shape = RoundedCornerShape(16.dp),
        contentPadding = PaddingValues(horizontal = 8.dp),
    ) {
        Icon(Icons.Rounded.Public, null, Modifier.size(20.dp))
        Spacer(Modifier.width(8.dp))
        Text("Worlds", style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun ServersButton(onOpen: () -> Unit, modifier: Modifier, short: Boolean) {
    OutlinedButton(
        onClick = onOpen,
        modifier = modifier.height(if (short) 48.dp else 56.dp),
        shape = RoundedCornerShape(16.dp),
        contentPadding = PaddingValues(horizontal = 8.dp),
    ) {
        Icon(Icons.Rounded.Dns, null, Modifier.size(20.dp))
        Spacer(Modifier.width(8.dp))
        Text("Servers", style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun ScreenshotsButton(onOpen: () -> Unit, modifier: Modifier, short: Boolean) {
    OutlinedButton(
        onClick = onOpen,
        modifier = modifier.height(if (short) 48.dp else 56.dp),
        shape = RoundedCornerShape(16.dp),
        contentPadding = PaddingValues(horizontal = 8.dp),
    ) {
        Icon(Icons.Rounded.PhotoLibrary, null, Modifier.size(20.dp))
        Spacer(Modifier.width(8.dp))
        Text("Screenshots", style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun TouchLayoutsButton(onOpen: () -> Unit, modifier: Modifier, short: Boolean) {
    OutlinedButton(
        onClick = onOpen,
        modifier = modifier.height(if (short) 48.dp else 56.dp),
        shape = RoundedCornerShape(16.dp),
        contentPadding = PaddingValues(horizontal = 8.dp),
    ) {
        Icon(Icons.Rounded.TouchApp, null, Modifier.size(20.dp))
        Spacer(Modifier.width(8.dp))
        Text("Touch layouts", style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/** Primary action: a large filled button, or the live progress state while installing. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PlayActionCard(
    state: Snapshot,
    onPlay: () -> Unit,
    short: Boolean,
    modifier: Modifier,
) {
    val install = state.install
    HorizonCard(modifier = modifier, tonal = true) {
        if (install.installing) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                LoadingIndicator(Modifier.size(if (short) 36.dp else 44.dp))
                Column(Modifier.padding(start = 12.dp).weight(1f)) {
                    Text(install.stage, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(
                        "${(install.overall * 100).toInt()}%",
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.primary,
                        fontWeight = FontWeight.Bold,
                    )
                }
            }
        } else {
            Button(
                onClick = onPlay,
                modifier = Modifier.fillMaxWidth().height(if (short) 60.dp else 68.dp),
                shapes = ButtonDefaults.shapes(),
                contentPadding = PaddingValues(horizontal = 24.dp),
            ) {
                Icon(
                    when {
                        install.installed -> Icons.Rounded.PlayArrow
                        state.error != null -> Icons.Rounded.Refresh
                        else -> Icons.Rounded.Download
                    },
                    contentDescription = null,
                    modifier = Modifier.size(28.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text(
                    when {
                        install.installed -> "Play"
                        state.error != null -> "Retry installation"
                        else -> "Download Minecraft 26.3"
                    },
                    style = if (install.installed) MaterialTheme.typography.headlineSmall else MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
