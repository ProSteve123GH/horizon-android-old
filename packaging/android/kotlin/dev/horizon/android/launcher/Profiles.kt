package dev.horizon.android.launcher

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.toShape
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
import org.json.JSONObject
import java.text.DateFormat
import java.util.Date
import java.util.Locale

/** An instance action waiting for its dialog: create, rename or delete. */
private sealed interface InstanceEdit {
    data object Create : InstanceEdit
    data class Rename(val instance: Profile) : InstanceEdit
    data class Delete(val instance: Profile) : InstanceEdit
}

/** What an instance holds, for telling instances apart at a glance. */
private fun Profile.contents(): String {
    fun part(n: Int, one: String, many: String) = if (n == 0) null else "$n ${if (n == 1) one else many}"
    return listOfNotNull(
        part(worlds, "world", "worlds"),
        part(mods, "mod", "mods"),
        part(packs, "pack", "packs"),
        part(shaders, "shader", "shaders"),
    ).joinToString("  ·  ").ifEmpty { "Empty" }
}

private fun sizeLabel(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val units = listOf("KB", "MB", "GB", "TB")
    var value = bytes.toDouble() / 1024
    var unit = 0
    while (value >= 1024 && unit < units.lastIndex) {
        value /= 1024
        unit++
    }
    return if (value >= 100) {
        "${value.toInt()} ${units[unit]}"
    } else {
        String.format(Locale.US, "%.1f %s", value, units[unit])
    }
}

private fun playedLabel(seconds: Long?): String {
    if (seconds == null) return "Never played"
    val age = ((System.currentTimeMillis() / 1000) - seconds).coerceAtLeast(0)
    return when {
        age < 60 -> "Played just now"
        age < 3600 -> "Played ${age / 60} min ago"
        age < 86_400 -> "Played ${age / 3600} h ago"
        age < 86_400 * 30 -> "Played ${age / 86_400} d ago"
        else -> "Played " + DateFormat.getDateInstance(DateFormat.MEDIUM)
            .format(Date(seconds * 1000))
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun InstancesSheet(onDismiss: () -> Unit, onPlay: () -> Unit) {
    var instances by remember { mutableStateOf(Profile.load()) }
    var edit by remember { mutableStateOf<InstanceEdit?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }
    val reload = { instances = Profile.load() }

    /** Runs an action, keeps the backend's error, and reloads the list. */
    fun applyReply(reply: Reply, success: String? = null) {
        error = reply.error
        notice = if (reply.error == null) success else null
        reload()
    }

    fun launch(instance: Profile) {
        val reply = act("profile_launch", instance.id.toString())
        if (reply.error != null) {
            applyReply(reply)
            return
        }
        val started = (reply.ok as? JSONObject)?.optBoolean("launch") == true
        reload()
        onDismiss()
        if (started) onPlay()
    }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        // One scrolling grid, header included. Sideways, instances pair up in two columns.
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
                            Text("Instances", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold)
                            Text(
                                "Each instance keeps its own worlds, settings and add-ons.",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Spacer(Modifier.width(12.dp))
                        Button(
                            onClick = { error = null; notice = null; edit = InstanceEdit.Create },
                            contentPadding = ButtonDefaults.ButtonWithIconContentPadding,
                        ) {
                            Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
                            Spacer(Modifier.width(8.dp))
                            Text("New")
                        }
                    }
                    error?.let { message ->
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
            itemsIndexed(instances, key = { _, p -> p.id }) { i, instance ->
                InstanceRow(
                    instance,
                    if (landscape) RoundedCornerShape(20.dp) else connectedShape(i, instances.size, outer = 24),
                    onSelect = {
                        if (!instance.active) applyReply(act("choose", "profile=${instance.id}"))
                    },
                    onPlay = { launch(instance) },
                    onRename = { error = null; notice = null; edit = InstanceEdit.Rename(instance) },
                    onDuplicate = {
                        applyReply(
                            act("profile_duplicate", instance.id.toString()),
                            success = "Duplicated ${instance.name}.",
                        )
                    },
                    onDelete = { error = null; notice = null; edit = InstanceEdit.Delete(instance) },
                )
            }
        }
    }

    when (val e = edit) {
        InstanceEdit.Create -> NameDialog("New instance", "", "Create", onDismiss = { edit = null }) { name ->
            applyReply(act("profile_create", name), success = "Created $name.")
        }
        is InstanceEdit.Rename -> NameDialog("Rename instance", e.instance.name, "Save", onDismiss = { edit = null }) { name ->
            applyReply(act("profile_rename", "${e.instance.id}=$name"), success = "Renamed to $name.")
        }
        is InstanceEdit.Delete -> AlertDialog(
            onDismissRequest = { edit = null },
            icon = { Icon(Icons.Rounded.DeleteOutline, null) },
            title = { Text("Delete ${e.instance.name}?") },
            text = {
                Text(
                    "Its worlds, settings and add-ons (${e.instance.contents().lowercase()}, " +
                        "${sizeLabel(e.instance.sizeBytes)}) are deleted from this phone. This cannot be undone.",
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        applyReply(act("profile_delete", e.instance.id.toString()), success = "Deleted ${e.instance.name}.")
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
private fun InstanceRow(
    instance: Profile,
    shape: androidx.compose.ui.graphics.Shape,
    onSelect: () -> Unit,
    onPlay: () -> Unit,
    onRename: () -> Unit,
    onDuplicate: () -> Unit,
    onDelete: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Surface(
        onClick = onSelect,
        shape = shape,
        color = if (instance.active) colors.secondaryContainer else colors.surfaceContainer,
    ) {
        Row(Modifier.padding(start = 12.dp, top = 10.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Surface(
                modifier = Modifier.size(40.dp),
                shape = if (instance.active) MaterialShapes.Cookie9Sided.toShape() else MaterialShapes.Circle.toShape(),
                color = if (instance.active) colors.primary else colors.surfaceContainerHighest,
            ) {
                Box(contentAlignment = Alignment.Center) {
                    if (instance.active) {
                        Icon(Icons.Rounded.Check, null, tint = colors.onPrimary)
                    } else {
                        Text(
                            instance.name.firstOrNull()?.uppercase() ?: "?",
                            style = MaterialTheme.typography.titleMedium,
                            color = colors.onSurfaceVariant,
                        )
                    }
                }
            }
            Column(Modifier.padding(start = 14.dp).weight(1f)) {
                Text(
                    instance.name,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    buildString {
                        if (instance.active) append("In use  ·  ")
                        append("Minecraft ${instance.minecraft.ifBlank { "unknown" }}  ·  ${sizeLabel(instance.sizeBytes)}")
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    if (instance.missing) "Folder missing  ·  ${instance.contents()}"
                    else "${instance.contents()}  ·  ${playedLabel(instance.lastPlayed)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = if (instance.missing) colors.error else colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            var menu by remember { mutableStateOf(false) }
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, "More for ${instance.name}") }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text("Play") },
                        leadingIcon = { Icon(Icons.Rounded.PlayArrow, null) },
                        enabled = !instance.missing,
                        onClick = { menu = false; onPlay() },
                    )
                    DropdownMenuItem(
                        text = { Text("Rename") },
                        leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                        onClick = { menu = false; onRename() },
                    )
                    DropdownMenuItem(
                        text = { Text("Duplicate") },
                        leadingIcon = { Icon(Icons.Rounded.ContentCopy, null) },
                        enabled = !instance.missing,
                        onClick = { menu = false; onDuplicate() },
                    )
                    // The default instance is the app's home folder; the active one is in use.
                    DropdownMenuItem(
                        text = {
                            Column {
                                Text("Delete")
                                if (!instance.deletable) {
                                    Text(
                                        if (instance.id == 0) "Default instance" else "Switch first",
                                        style = MaterialTheme.typography.bodySmall,
                                    )
                                }
                            }
                        },
                        leadingIcon = { Icon(Icons.Rounded.DeleteOutline, null) },
                        enabled = instance.deletable,
                        onClick = { menu = false; onDelete() },
                    )
                }
            }
        }
    }
}

@Composable
private fun NameDialog(title: String, initial: String, confirm: String, onDismiss: () -> Unit, onSave: (String) -> Unit) {
    var name by remember { mutableStateOf(initial) }
    val save = {
        onSave(name.trim())
        onDismiss()
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { if (it.length <= 32) name = it },
                label = { Text("Name") },
                placeholder = { Text("Survival, Modded, Creative…") },
                supportingText = { Text("${name.length}/32") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = { Button(onClick = save, enabled = name.isNotBlank()) { Text(confirm) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
