package dev.horizon.android.launcher

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/** A world action waiting for its dialog. */
private sealed interface WorldEdit {
    data class Rename(val world: World) : WorldEdit
    data class Delete(val world: World) : WorldEdit
}

/** The metadata line under a world's name; never invents what the folder lacks. */
private fun worldDetail(world: World): String {
    if (!world.metadata) return "Metadata unavailable  ·  Folder can still be deleted"
    val parts = mutableListOf(world.mode)
    world.version?.takeIf { it.isNotBlank() }?.let(parts::add)
    parts += playedLabel(world.lastPlayed)
    if (world.inUse) parts += "In use"
    return parts.joinToString("  ·  ")
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun WorldsSheet(onDismiss: () -> Unit, onPlay: () -> Unit) {
    var state by remember { mutableStateOf(Worlds.load()) }
    var edit by remember { mutableStateOf<WorldEdit?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }
    val reload = { state = Worlds.load() }

    /** Runs an action, keeps the backend's error, and reloads the list. */
    fun applyReply(reply: Reply, success: String? = null) {
        error = reply.error
        notice = if (reply.error == null) success else null
        reload()
    }

    fun launch(world: World) {
        val reply = act("world_launch", world.folder)
        if (reply.error != null) {
            applyReply(reply)
            return
        }
        val started = (reply.ok as? JSONObject)?.optBoolean("launch") == true
        reload()
        if (!started) {
            notice = "The game is already starting. Try again in a moment."
            return
        }
        onDismiss()
        onPlay()
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        // One scrolling grid, header included. Sideways, worlds pair up in two columns.
        val landscape = isLandscape()
        LazyVerticalGrid(
            columns = GridCells.Fixed(if (landscape) 2 else 1),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(if (landscape) 8.dp else 2.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            item(key = "header", span = { GridItemSpan(maxLineSpan) }) {
                Column(Modifier.fillMaxWidth().padding(start = 8.dp, bottom = 12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(
                                "Worlds",
                                style = MaterialTheme.typography.headlineSmall,
                                fontWeight = FontWeight.SemiBold,
                            )
                            Text(
                                if (state.instance.isBlank()) {
                                    "Single-player saves in the active instance."
                                } else {
                                    "In ${state.instance}. Each instance keeps its own saves."
                                },
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Spacer(Modifier.width(12.dp))
                        IconButton(onClick = { error = null; notice = null; reload() }) {
                            Icon(Icons.Rounded.Refresh, "Refresh worlds")
                        }
                    }
                    (error ?: state.error)?.let { message ->
                        Text(
                            message,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.error,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                    }
                    notice?.let { message ->
                        Text(
                            message,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                    }
                }
            }
            if (state.worlds.isEmpty()) {
                item(key = "empty", span = { GridItemSpan(maxLineSpan) }) {
                    Column(
                        Modifier.fillMaxWidth().padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Icon(
                            Icons.Rounded.Public,
                            null,
                            Modifier.size(48.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text("No worlds yet", style = MaterialTheme.typography.titleMedium)
                        Text(
                            "Create a world in Minecraft, or copy a complete world folder in Settings → World files.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
            itemsIndexed(state.worlds, key = { _, world -> world.folder }) { i, world ->
                WorldRow(
                    world,
                    if (landscape) RoundedCornerShape(20.dp) else connectedShape(i, state.worlds.size, outer = 24),
                    onLaunch = { launch(world) },
                    onRename = { error = null; notice = null; edit = WorldEdit.Rename(world) },
                    onDuplicate = {
                        val reply = act("world_duplicate", world.folder)
                        val copy = (reply.ok as? JSONObject)?.optString("name")
                        applyReply(reply, success = copy?.takeIf { it.isNotBlank() }?.let { "Duplicated as $it." })
                    },
                    onDelete = { error = null; notice = null; edit = WorldEdit.Delete(world) },
                )
            }
        }
    }

    when (val e = edit) {
        is WorldEdit.Rename -> NameDialog(
            "Rename world",
            e.world.name,
            "Save",
            maxLength = 64,
            onDismiss = { edit = null },
        ) { name ->
            val arg = JSONObject().put("folder", e.world.folder).put("name", name).toString()
            applyReply(act("world_rename", arg), success = "Renamed to $name.")
        }
        is WorldEdit.Delete -> AlertDialog(
            onDismissRequest = { edit = null },
            icon = { Icon(Icons.Rounded.DeleteOutline, null) },
            title = { Text("Delete ${e.world.name}?") },
            text = {
                Text(
                    buildString {
                        append("The world folder \"")
                        append(e.world.folder)
                        append("\" and everything in it (regions, player data and datapacks) is deleted from this phone.")
                        if (!e.world.metadata) append(" It has no readable level.dat.")
                        append(" This cannot be undone.")
                    },
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        applyReply(
                            act("world_delete", e.world.folder),
                            success = "Deleted ${e.world.name}.",
                        )
                        edit = null
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.error,
                        contentColor = MaterialTheme.colorScheme.onError,
                    ),
                ) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { edit = null }) { Text("Cancel") } },
        )
        null -> {}
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun WorldRow(
    world: World,
    shape: Shape,
    onLaunch: () -> Unit,
    onRename: () -> Unit,
    onDuplicate: () -> Unit,
    onDelete: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val canChange = world.metadata && !world.inUse
    Surface(
        onClick = onLaunch,
        enabled = canChange,
        shape = shape,
        color = if (world.inUse) colors.surfaceContainerHighest else colors.surfaceContainer,
    ) {
        Row(
            Modifier.padding(start = 12.dp, top = 10.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            WorldIcon(world)
            Column(Modifier.padding(start = 14.dp).weight(1f)) {
                Text(
                    world.name,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    worldDetail(world),
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (world.inUse) colors.primary else colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    "Folder: ${world.folder}",
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            var menu by remember { mutableStateOf(false) }
            Box {
                IconButton(onClick = { menu = true }) {
                    Icon(Icons.Rounded.MoreVert, "More for ${world.name}")
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text("Play") },
                        leadingIcon = { Icon(Icons.Rounded.PlayArrow, null) },
                        enabled = canChange,
                        onClick = { menu = false; onLaunch() },
                    )
                    DropdownMenuItem(
                        text = { Text("Rename") },
                        leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                        enabled = canChange,
                        onClick = { menu = false; onRename() },
                    )
                    DropdownMenuItem(
                        text = { Text("Duplicate") },
                        leadingIcon = { Icon(Icons.Rounded.ContentCopy, null) },
                        enabled = canChange,
                        onClick = { menu = false; onDuplicate() },
                    )
                    DropdownMenuItem(
                        text = { Text("Delete") },
                        leadingIcon = { Icon(Icons.Rounded.DeleteOutline, null) },
                        enabled = !world.inUse,
                        onClick = { menu = false; onDelete() },
                    )
                }
            }
        }
    }
}

@Composable
private fun WorldIcon(world: World) {
    val bitmap = remember(world.icon) {
        world.icon?.let { path -> runCatching { BitmapFactory.decodeFile(path) }.getOrNull() }
    }
    Surface(
        modifier = Modifier.size(56.dp),
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHighest,
    ) {
        if (bitmap != null) {
            Image(
                bitmap.asImageBitmap(),
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        } else {
            Box(contentAlignment = Alignment.Center) {
                Icon(
                    Icons.Rounded.Public,
                    null,
                    Modifier.size(26.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
