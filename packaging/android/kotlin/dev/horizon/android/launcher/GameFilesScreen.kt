package dev.horizon.android.launcher

import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.InsertDriveFile
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.horizon.android.R
import java.io.File

/** Raw folder/file names mapped to friendly labels; disk names never change. */
@Composable
private fun friendlyLabel(name: String): String = when (name) {
    "saves" -> stringResource(R.string.label_worlds)
    "resourcepacks" -> stringResource(R.string.label_resource_packs)
    "shaderpacks" -> stringResource(R.string.label_shader_packs)
    "horizon-mods", "mods" -> stringResource(R.string.label_mods)
    "performance" -> stringResource(R.string.label_reports)
    "config", "options.txt", "options.json" -> stringResource(R.string.label_configuration)
    "wardrobe" -> stringResource(R.string.label_skins_capes)
    else -> if (isConfigFile(name)) stringResource(R.string.label_configuration) else name
}

private fun isConfigFile(name: String): Boolean =
    listOf(".txt", ".json", ".toml", ".properties", ".dat", ".cfg", ".yml", ".yaml")
        .any { name.endsWith(it, ignoreCase = true) }

/**
 * Browses the game folder directly with [File]. Friendly labels are presentation
 * only. Everything happens inside the app: handing a `file://` URI to another app
 * throws `FileUriExposedException` on modern Android, so there is no external
 * "open raw folder" escape hatch here.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun GameFilesScreen(root: File, onBack: () -> Unit) {
    var stack by remember { mutableStateOf(listOf(root)) }
    var advanced by rememberSaveable { mutableStateOf(false) }
    val current = stack.last()

    fun up() {
        if (stack.size > 1) stack = stack.dropLast(1) else onBack()
    }
    BackHandler { up() }

    fun openFolder(folder: File) {
        if (folder.isDirectory) stack = stack + folder
    }

    Column(Modifier.fillMaxSize()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { up() }) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.game_files_up))
            }
            Text(
                stringResource(R.string.game_files_title),
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
                modifier = Modifier.weight(1f),
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.game_files_advanced), style = MaterialTheme.typography.bodySmall)
                Spacer(Modifier.width(6.dp))
                Switch(checked = advanced, onCheckedChange = { advanced = it })
            }
        }
        Text(
            stringResource(R.string.game_files_advanced_hide),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 56.dp),
        )
        Spacer(Modifier.height(8.dp))
        Breadcrumbs(stack) { index -> stack = stack.take(index + 1) }
        Spacer(Modifier.height(8.dp))
        HorizontalDivider()

        when {
            !root.exists() -> Column(
                Modifier.fillMaxSize().padding(24.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(stringResource(R.string.game_files_root_missing), textAlign = androidx.compose.ui.text.style.TextAlign.Center)
            }
            else -> {
                val children = remember(current, advanced) {
                    current.listFiles()
                        ?.filter { advanced || !it.name.startsWith(".horizon-cache") }
                        ?.sortedWith(compareBy({ !it.isDirectory }, { it.name.lowercase() }))
                        .orEmpty()
                }
                if (children.isEmpty()) {
                    Box(Modifier.fillMaxWidth().weight(1f), contentAlignment = Alignment.Center) {
                        Text(stringResource(R.string.game_files_empty), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                } else {
                    LazyColumn(
                        Modifier.fillMaxWidth().weight(1f),
                        verticalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        items(children, key = { it.absolutePath }) { entry ->
                            FileRow(
                                file = entry,
                                // Browsing stays in-app; individual files have no safe external
                                // open path without a FileProvider, so only folders navigate.
                                onOpen = { if (entry.isDirectory) openFolder(entry) },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Breadcrumbs(stack: List<File>, onSelect: (Int) -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        stack.forEachIndexed { index, dir ->
            if (index > 0) Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, Modifier.size(16.dp))
            val label = if (index == 0) stringResource(R.string.game_files_title) else friendlyLabel(dir.name)
            TextButtonLike(label, index == stack.lastIndex) { onSelect(index) }
        }
    }
}

@Composable
private fun TextButtonLike(label: String, current: Boolean, onClick: () -> Unit) {
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(8.dp),
        color = if (current) MaterialTheme.colorScheme.secondaryContainer else androidx.compose.ui.graphics.Color.Transparent,
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = if (current) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.primary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        )
    }
}

@Composable
private fun FileRow(file: File, onOpen: () -> Unit) {
    val friendly = friendlyLabel(file.name)
    Surface(
        onClick = onOpen,
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                if (file.isDirectory) Icons.Rounded.Folder else Icons.Rounded.InsertDriveFile,
                null,
                tint = MaterialTheme.colorScheme.primary,
            )
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(friendly, fontWeight = androidx.compose.ui.text.font.FontWeight.Medium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (friendly != file.name) {
                    Text(
                        file.name,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}
