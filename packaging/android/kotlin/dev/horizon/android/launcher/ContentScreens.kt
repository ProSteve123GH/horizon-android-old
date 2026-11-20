package dev.horizon.android.launcher

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.NavigateBefore
import androidx.compose.material.icons.automirrored.rounded.NavigateNext
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.ExpandLess
import androidx.compose.material.icons.rounded.ExpandMore
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Badge
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

private fun kindOf(page: Destination) = when (page) {
    Destination.Mods -> "mods"
    Destination.Shaders -> "shaders"
    else -> "packs"
}

/** Reuse the navigation icons so a row reads at a glance without another icon dependency. */
private fun iconOf(kind: String) = when (kind) {
    "mods" -> Destination.Mods.icon
    "shaders" -> Destination.Shaders.icon
    else -> Destination.Packs.icon
}

@Composable
private fun FolderButton(onOpen: () -> Unit, modifier: Modifier = Modifier) {
    // The Documents provider was removed, so the old "open_folder" bridge call
    // silently failed. Take the player to the in-app browser instead.
    FilledTonalButton(onClick = onOpen, modifier = modifier) {
        Icon(Icons.Rounded.FolderOpen, null, Modifier.size(18.dp))
        Spacer(Modifier.width(8.dp))
        Text("Folder", maxLines = 1)
    }
}

@Composable
private fun ImportButton(modifier: Modifier = Modifier, onClick: () -> Unit) {
    FilledTonalButton(onClick = onClick, modifier = modifier) {
        Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
        Spacer(Modifier.width(8.dp))
        Text("Import", maxLines = 1)
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun SourcePicker(page: Destination, state: Snapshot, modifier: Modifier, short: Boolean) {
    val selected = if (page == Destination.Mods) state.config.modSource else state.config.shaderSource
    SingleChoiceSegmentedButtonRow(modifier) {
        listOf("horizon" to "Horizon", "modrinth" to "Modrinth").forEachIndexed { i, (id, label) ->
            SegmentedButton(
                selected = selected == id,
                onClick = { act("source", id) },
                shape = SegmentedButtonDefaults.itemShape(i, 2),
                modifier = Modifier.heightIn(min = if (short) 40.dp else 48.dp),
                label = { Text(label, style = MaterialTheme.typography.labelLarge, maxLines = 1) },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun CatalogScreen(page: Destination, state: Snapshot, wide: Boolean, short: Boolean, actions: LauncherActions) {
    val catalog = state.catalog
    var query by rememberSaveable(page) { mutableStateOf(catalog.query) }
    val keyboard = LocalSoftwareKeyboardController.current
    val search = {
        keyboard?.hide()
        act("search", query)
    }
    val kind = kindOf(page)
    val hasSources = page == Destination.Mods || page == Destination.Shaders
    val spacing = if (short) 8.dp else 12.dp
    val hint = when {
        page == Destination.Mods && state.config.modSource == "modrinth" ->
            "Modrinth Java mods are browse-only: Fabric, Forge and NeoForge cannot run here."
        page == Destination.Mods -> "Horizon .hmod (content, or content + native) and legacy .hnmod mods. Paste an HTTPS link to import."
        page == Destination.Shaders && state.config.shaderSource == "modrinth" ->
            "Modrinth shader ZIPs are experimental; some packs may not work."
        page == Destination.Shaders -> "Horizon native .hshader packs."
        else -> "Verified downloads. Enable them in Installed."
    }
    // One list for controls and results: on a phone held sideways the search field and buttons scroll
    // away instead of squeezing the results to nothing.
    LazyColumn(
        Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(spacing),
        contentPadding = PaddingValues(bottom = 16.dp),
    ) {
        item(key = "controls") {
            HorizonCard(Modifier.fillMaxWidth()) {
                TextField(
                    value = query,
                    onValueChange = { query = it },
                    modifier = Modifier.fillMaxWidth(),
                    placeholder = {
                        Text(if (page == Destination.Mods) "Search mods or paste a .hmod (content + native) / legacy .hnmod link" else "Search")
                    },
                    leadingIcon = { Icon(Icons.Rounded.Search, null) },
                    trailingIcon = {
                        if (query.isNotEmpty()) {
                            IconButton(onClick = { query = ""; act("search", "") }) { Icon(Icons.Rounded.Clear, "Clear") }
                        }
                    },
                    singleLine = true,
                    shape = CircleShape,
                    colors = TextFieldDefaults.colors(
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                        disabledIndicatorColor = Color.Transparent,
                    ),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { search() }),
                )
                Spacer(Modifier.height(spacing))
                if (wide && hasSources) {
                    // Landscape has room for the source picker beside the file actions.
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(spacing),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            FolderButton(actions.openGameFiles, Modifier.weight(1f))
                            ImportButton(Modifier.weight(1f)) { actions.import(kind) }
                        }
                        SourcePicker(page, state, Modifier.weight(2f), short)
                    }
                } else {
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FolderButton(actions.openGameFiles, Modifier.weight(1f))
                        ImportButton(Modifier.weight(1f)) { actions.import(kind) }
                    }
                    if (hasSources) {
                        Spacer(Modifier.height(spacing))
                        // The source picker gets its own full-width row so both labels always fit.
                        SourcePicker(page, state, Modifier.fillMaxWidth(), short)
                    }
                }
                Spacer(Modifier.height(spacing))
                Text(hint, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        if (page == Destination.Mods) {
            item(key = "native-mods") {
                NativeModsSection(state.nativeMods, actions)
            }
        }
        when {
            catalog.busy && catalog.results.isEmpty() -> item(key = "loading") {
                HorizonCard(Modifier.fillMaxWidth()) {
                    Box(Modifier.fillMaxWidth().padding(vertical = 24.dp), contentAlignment = Alignment.Center) {
                        Column(
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            LoadingIndicator()
                            Text(
                                "Searching…",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
            catalog.results.isEmpty() -> item(key = "empty") {
                HorizonCard(Modifier.fillMaxWidth()) {
                    Column(
                        Modifier.fillMaxWidth().padding(vertical = 8.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Icon(
                            Icons.Rounded.Search,
                            null,
                            Modifier.size(40.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            "Nothing here yet. Search, or change the source.",
                            textAlign = TextAlign.Center,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        if (query.isNotBlank()) {
                            FilledTonalButton(onClick = { query = ""; act("search", "") }) {
                                Icon(Icons.Rounded.Clear, null, Modifier.size(18.dp))
                                Spacer(Modifier.width(6.dp))
                                Text("Clear search")
                            }
                        } else {
                            ImportButton { actions.import(kind) }
                        }
                    }
                }
            }
            else -> {
                item(key = "results-header") {
                    SectionHeader(
                        title = "Results",
                        action = {
                            Text(
                                catalog.results.size.toString(),
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        },
                    )
                }
                items(catalog.results, key = { it.index }) { project ->
                    ProjectCard(page, project, wide)
                }
                item(key = "pager") {
                    HorizonCard(Modifier.fillMaxWidth()) {
                        Row(
                            Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            OutlinedButton(
                                onClick = { act("previous") },
                                enabled = catalog.offset > 0,
                                modifier = Modifier.weight(1f),
                            ) {
                                Icon(Icons.AutoMirrored.Rounded.NavigateBefore, null, Modifier.size(18.dp))
                                Spacer(Modifier.width(6.dp))
                                Text("Previous", maxLines = 1)
                            }
                            OutlinedButton(onClick = { act("next") }, modifier = Modifier.weight(1f)) {
                                Text("Next", maxLines = 1)
                                Spacer(Modifier.width(6.dp))
                                Icon(Icons.AutoMirrored.Rounded.NavigateNext, null, Modifier.size(18.dp))
                            }
                            Text(
                                "Page ${catalog.offset / 12 + 1}",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ProjectCard(page: Destination, project: Project, wide: Boolean) {
    HorizonCard(Modifier.fillMaxWidth()) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Surface(
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.primaryContainer,
                modifier = Modifier.size(if (wide) 52.dp else 44.dp),
            ) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Icon(
                        page.icon,
                        null,
                        modifier = Modifier.size(24.dp),
                        tint = MaterialTheme.colorScheme.onPrimaryContainer,
                    )
                }
            }
            Column(
                Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(
                    project.title,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (project.summary.isNotBlank()) {
                    Text(
                        project.summary,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (project.author.isNotBlank()) {
                    Text(
                        "by ${project.author}",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.outline,
                    )
                }
            }
            when {
                project.state != null -> Surface(
                    shape = CircleShape,
                    color = MaterialTheme.colorScheme.primaryContainer,
                    contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                ) {
                    Text(
                        project.state,
                        style = MaterialTheme.typography.labelMedium,
                        maxLines = 1,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                    )
                }
                project.note != null -> Text(
                    project.note,
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                )
                else -> FilledTonalButton(onClick = { act("install", project.index.toString()) }) {
                    Icon(Icons.Rounded.Download, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(project.button, maxLines = 1)
                }
            }
        }
    }
}

/**
 * Kept for callers that do not pass the layout params (the older shell signature). The
 * four-argument version below is used when the page supplies `wide` and `short`.
 */
@Composable
fun InstalledScreen(state: Snapshot, actions: LauncherActions) {
    InstalledScreen(state, actions, isLandscape(), false)
}

@Composable
fun InstalledScreen(state: Snapshot, actions: LauncherActions, wide: Boolean, short: Boolean) {
    val open = remember { mutableStateMapOf("mods" to true, "packs" to true, "shaders" to true) }
    val sections = listOf("mods" to "Mods", "packs" to "Resource packs", "shaders" to "Shaders")
    val spacing = if (short) 8.dp else 12.dp
    val iconSize = if (wide) 52.dp else 44.dp
    LazyColumn(
        Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(spacing),
        contentPadding = PaddingValues(bottom = 16.dp),
    ) {
        sections.forEach { (kind, label) ->
            val items = state.local.filter { it.kind == kind }
            val expanded = open[kind] == true
            item(key = "header-$kind") {
                HorizonCard(Modifier.fillMaxWidth(), tonal = true) {
                    SectionHeader(
                        title = label,
                        action = {
                            Badge(containerColor = MaterialTheme.colorScheme.primaryContainer) {
                                Text(items.size.toString(), color = MaterialTheme.colorScheme.onPrimaryContainer)
                            }
                            IconButton(onClick = { open[kind] = !expanded }) {
                                Icon(
                                    if (expanded) Icons.Rounded.ExpandLess else Icons.Rounded.ExpandMore,
                                    "Expand or collapse",
                                )
                            }
                            ImportButton { actions.import(kind) }
                        },
                    )
                }
            }
            if (expanded) {
                if (items.isEmpty()) {
                    item(key = "empty-$kind") {
                        HorizonCard(Modifier.fillMaxWidth()) {
                            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                Text("Nothing installed yet.", style = MaterialTheme.typography.bodyLarge)
                                Text(
                                    "Import a $label file to make it available in the game.",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                                ImportButton { actions.import(kind) }
                            }
                        }
                    }
                }
                items(items, key = { "item-${it.index}-${it.file}" }) { local ->
                    HorizonCard(Modifier.fillMaxWidth()) {
                        Row(
                            Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Box {
                                Surface(
                                    shape = RoundedCornerShape(14.dp),
                                    color = MaterialTheme.colorScheme.secondaryContainer,
                                    modifier = Modifier.size(iconSize),
                                ) {
                                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                                        Icon(
                                            iconOf(kind),
                                            null,
                                            tint = MaterialTheme.colorScheme.onSecondaryContainer,
                                        )
                                    }
                                }
                                if (local.toggle && local.enabled) {
                                    Badge(
                                        Modifier.align(Alignment.TopEnd),
                                        containerColor = MaterialTheme.colorScheme.primary,
                                    ) { }
                                }
                            }
                            Column(
                                Modifier.weight(1f),
                                verticalArrangement = Arrangement.spacedBy(2.dp),
                            ) {
                                Text(
                                    local.title,
                                    style = MaterialTheme.typography.titleMedium,
                                    fontWeight = FontWeight.Medium,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    local.file,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                if (!local.toggle) {
                                    Text(
                                        "64-bit required",
                                        style = MaterialTheme.typography.labelMedium,
                                        color = MaterialTheme.colorScheme.error,
                                    )
                                }
                            }
                            Row(
                                horizontalArrangement = Arrangement.spacedBy(4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                if (local.toggle) {
                                    HorizonSwitch(
                                        checked = local.enabled,
                                        onCheckedChange = { act("toggle_local", local.index.toString()) },
                                    )
                                }
                                IconButton(onClick = { actions.remove(local) }) {
                                    Icon(
                                        Icons.Rounded.DeleteOutline,
                                        "Remove",
                                        tint = MaterialTheme.colorScheme.error,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
