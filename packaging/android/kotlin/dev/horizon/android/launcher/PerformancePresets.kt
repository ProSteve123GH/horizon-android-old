package dev.horizon.android.launcher

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONObject
import kotlin.math.roundToInt

private fun JSONObject.nullableString(key: String): String? {
    val value = opt(key)
    return (value as? String)?.takeIf { it.isNotBlank() }
}

internal data class PerformanceChoice(val value: String, val label: String)

internal data class PerformanceSetting(
    val id: String,
    val label: String,
    val group: String,
    val kind: String,
    val current: Any?,
    val defaultValue: Any?,
    val restart: Boolean,
    val help: String,
    val unit: String,
    val min: Int,
    val max: Int,
    val step: Int,
    val options: List<PerformanceChoice>,
) {
    fun display(value: Any?): String = when (kind) {
        "bool" -> if (value == true) "On" else "Off"
        "choice" -> options.firstOrNull { it.value == value?.toString() }?.label
            ?: value?.toString().orEmpty()
        "int" -> {
            val number = value?.toString().orEmpty()
            when (unit) {
                "%" -> "$number%"
                "" -> number
                else -> "$number $unit"
            }
        }
        else -> value?.toString().orEmpty()
    }
}

internal data class PerformancePreset(
    val id: String,
    val name: String,
    val summary: String,
    val builtin: Boolean,
    val values: JSONObject,
)

internal data class PerformanceState(
    val instance: String,
    val instanceId: Int,
    val selected: String?,
    val selectedName: String?,
    val presets: List<PerformancePreset>,
    val settings: List<PerformanceSetting>,
    val warning: String?,
) {
    companion object {
        fun parse(root: JSONObject): PerformanceState = PerformanceState(
            instance = root.optString("instance"),
            instanceId = root.optInt("instance_id"),
            selected = root.nullableString("selected"),
            selectedName = root.nullableString("selected_name"),
            presets = root.optJSONArray("presets")?.let { list ->
                List(list.length()) { index ->
                    val item = list.getJSONObject(index)
                    PerformancePreset(
                        id = item.optString("id"),
                        name = item.optString("name"),
                        summary = item.optString("summary"),
                        builtin = item.optBoolean("builtin"),
                        values = item.optJSONObject("values") ?: JSONObject(),
                    )
                }
            }.orEmpty(),
            settings = root.optJSONArray("settings")?.let { list ->
                List(list.length()) { index -> parseSetting(list.getJSONObject(index)) }
            }.orEmpty(),
            warning = root.nullableString("warning"),
        )

        private fun parseSetting(item: JSONObject): PerformanceSetting {
            val kind = item.optJSONObject("kind")
            val options = kind?.optJSONArray("options")?.let { list ->
                List(list.length()) { index ->
                    val option = list.getJSONObject(index)
                    PerformanceChoice(option.optString("value"), option.optString("label"))
                }
            }.orEmpty()
            return PerformanceSetting(
                id = item.optString("id"),
                label = item.optString("label"),
                group = item.optString("group"),
                kind = kind?.optString("type").orEmpty(),
                current = item.opt("current"),
                defaultValue = item.opt("default"),
                restart = item.optBoolean("restart"),
                help = item.optString("help"),
                unit = item.optString("unit"),
                min = kind?.optInt("min") ?: 0,
                max = kind?.optInt("max") ?: 0,
                step = kind?.optInt("step")?.coerceAtLeast(1) ?: 1,
                options = options,
            )
        }
    }
}

internal data class PerformanceChange(
    val id: String,
    val label: String,
    val current: Any?,
    val target: Any?,
    val restart: Boolean,
    val unit: String,
)

internal data class PerformancePreview(
    val id: String,
    val name: String,
    val changes: List<PerformanceChange>,
    val restart: Boolean,
    val noChanges: Boolean,
) {
    companion object {
        fun parse(root: JSONObject, id: String, name: String): PerformancePreview =
            PerformancePreview(
                id = id,
                name = name,
                changes = root.optJSONArray("changes")?.let { list ->
                    List(list.length()) { index ->
                        val item = list.getJSONObject(index)
                        PerformanceChange(
                            id = item.optString("id"),
                            label = item.optString("label"),
                            current = item.opt("current"),
                            target = item.opt("target"),
                            restart = item.optBoolean("restart"),
                            unit = item.optString("unit"),
                        )
                    }
                }.orEmpty(),
                restart = root.optBoolean("restart"),
                noChanges = root.optBoolean("no_changes"),
            )
    }
}

private data class EditorTarget(
    val id: String?,
    val name: String,
    val values: JSONObject,
)

private fun currentValues(state: PerformanceState): JSONObject = JSONObject().apply {
    state.settings.forEach { setting -> put(setting.id, setting.current) }
}

private fun jsonObject(values: Map<String, Any?>): JSONObject = JSONObject().apply {
    values.forEach { (key, value) -> put(key, value) }
}

private fun formatPreviewValue(change: PerformanceChange, state: PerformanceState?): String {
    val setting = state?.settings?.firstOrNull { it.id == change.id }
    if (setting != null) return setting.display(change.target)
    return when (val value = change.target) {
        is Boolean -> if (value) "On" else "Off"
        else -> when (change.unit) {
            "%" -> "${value}%"
            "" -> value?.toString().orEmpty()
            else -> "${value} ${change.unit}"
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PerformanceSheet(onDismiss: () -> Unit) {
    var state by remember { mutableStateOf<PerformanceState?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var preview by remember { mutableStateOf<PerformancePreview?>(null) }
    var editor by remember { mutableStateOf<EditorTarget?>(null) }
    var editorError by remember { mutableStateOf<String?>(null) }
    var renaming by remember { mutableStateOf<PerformancePreset?>(null) }
    var deleting by remember { mutableStateOf<PerformancePreset?>(null) }
    var resetting by remember { mutableStateOf(false) }

    val reload: () -> Unit = {
        val reply = act("performance_state")
        val root = reply.ok as? JSONObject
        if (root == null) {
            error = reply.error ?: "Could not load performance presets"
        } else {
            state = PerformanceState.parse(root)
            error = null
        }
    }
    LaunchedEffect(Unit) { reload() }

    fun openPreview(preset: PerformancePreset) {
        val reply = act("performance_preview", preset.id)
        val root = reply.ok as? JSONObject
        if (root == null) {
            error = reply.error ?: "Could not preview ${preset.name}"
        } else {
            error = null
            preview = PerformancePreview.parse(root, preset.id, preset.name)
        }
    }

    fun saveEditor(target: EditorTarget, name: String, values: Map<String, Any?>) {
        val request = JSONObject()
            .put("name", name)
            .put("values", jsonObject(values))
        target.id?.let { request.put("id", it) }
        val reply = if (target.id == null) {
            act("performance_create", request.toString())
        } else {
            act("performance_update", request.toString())
        }
        if (reply.error != null) {
            editorError = reply.error
        } else {
            editor = null
            editorError = null
            error = null
            reload()
        }
    }

    fun finish(action: () -> Reply, failure: String) {
        val reply = action()
        if (reply.error != null) error = reply.error else error = null
        reload()
        if (reply.error != null) error = reply.error ?: failure
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        val current = state
        if (current == null) {
            Box(Modifier.fillMaxWidth().height(180.dp), contentAlignment = Alignment.Center) {
                if (error == null) LinearWavyProgressIndicator(Modifier.fillMaxWidth()) else Text(error.orEmpty())
            }
        } else {
            LazyColumn(
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                item(key = "header") {
                    Column(Modifier.fillMaxWidth().padding(start = 8.dp, bottom = 12.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(
                                    "Performance presets",
                                    style = MaterialTheme.typography.headlineSmall,
                                    fontWeight = FontWeight.SemiBold,
                                )
                                Text(
                                    "For ${current.instance}. Tap a preset to see every change before applying it; changes take effect when the game next starts.",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            Spacer(Modifier.width(12.dp))
                            Button(
                                onClick = {
                                    error = null
                                    editorError = null
                                    editor = EditorTarget(null, "My preset", currentValues(current))
                                },
                                contentPadding = ButtonDefaults.ButtonWithIconContentPadding,
                            ) {
                                Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
                                Spacer(Modifier.width(8.dp))
                                Text("New")
                            }
                        }
                        current.warning?.let {
                            Text(
                                it,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.error,
                                modifier = Modifier.padding(top = 8.dp),
                            )
                        }
                        error?.let {
                            Text(
                                it,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.error,
                                modifier = Modifier.padding(top = 8.dp),
                            )
                        }
                    }
                }
                items(current.presets, key = { it.id }) { preset ->
                    PresetRow(
                        preset = preset,
                        selected = current.selected == preset.id,
                        shape = RoundedCornerShape(20.dp),
                        onPreview = { openPreview(preset) },
                        onEdit = {
                            editorError = null
                            editor = EditorTarget(preset.id, preset.name, preset.values)
                        },
                        onRename = { renaming = preset },
                        onDuplicate = {
                            finish(
                                { act("performance_duplicate", preset.id) },
                                "Could not duplicate ${preset.name}",
                            )
                        },
                        onDelete = { deleting = preset },
                    )
                }
                item(key = "restore") {
                    OutlinedButton(
                        onClick = { resetting = true },
                        modifier = Modifier.fillMaxWidth().padding(top = 12.dp).heightIn(min = 48.dp),
                    ) {
                        Icon(Icons.Rounded.RestartAlt, null, Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text("Restore engine defaults")
                    }
                }
                item(key = "note") {
                    Text(
                        "Close the game before applying. Only the supported settings listed in a " +
                            "preview are changed; other options, packs, worlds and launcher settings " +
                            "stay as they are. These presets do not guarantee a frame rate or a cooler device.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 8.dp, top = 8.dp, end = 8.dp),
                    )
                }
            }
        }
    }

    preview?.let { shown ->
        PerformancePreviewDialog(
            preview = shown,
            state = state,
            onDismiss = { preview = null },
            onApply = {
                val reply = act("performance_apply", shown.id)
                if (reply.error != null) {
                    error = reply.error
                } else {
                    preview = null
                    error = null
                    reload()
                }
            },
        )
    }

    editor?.let { target ->
        PerformanceEditorDialog(
            title = if (target.id == null) "New performance preset" else "Edit ${target.name}",
            initialName = target.name,
            initialValues = target.values,
            settings = state?.settings.orEmpty(),
            error = editorError,
            onDismiss = { editor = null; editorError = null },
            onSave = { name, values -> saveEditor(target, name, values) },
        )
    }

    renaming?.let { preset ->
        NameDialog(
            title = "Rename performance preset",
            initial = preset.name,
            confirm = "Save",
            onDismiss = { renaming = null },
        ) { name ->
            val request = JSONObject().put("id", preset.id).put("name", name)
            finish({ act("performance_rename", request.toString()) }, "Could not rename ${preset.name}")
            renaming = null
        }
    }

    deleting?.let { preset ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            icon = { Icon(Icons.Rounded.DeleteOutline, null) },
            title = { Text("Delete ${preset.name}?") },
            text = { Text("This deletes only the saved preset. The engine settings it applied stay as they are.") },
            confirmButton = {
                Button(
                    onClick = {
                        finish({ act("performance_delete", preset.id) }, "Could not delete ${preset.name}")
                        deleting = null
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.error,
                        contentColor = MaterialTheme.colorScheme.onError,
                    ),
                ) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text("Cancel") } },
        )
    }

    if (resetting) {
        AlertDialog(
            onDismissRequest = { resetting = false },
            title = { Text("Restore engine defaults?") },
            text = {
                Text(
                    "This rewrites only the supported performance settings to their engine defaults. " +
                        "Other options, add-ons and worlds are not touched.",
                )
            },
            confirmButton = {
                Button(onClick = {
                    finish({ act("performance_reset") }, "Could not restore engine defaults")
                    resetting = false
                }) { Text("Restore") }
            },
            dismissButton = { TextButton(onClick = { resetting = false }) { Text("Cancel") } },
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PresetRow(
    preset: PerformancePreset,
    selected: Boolean,
    shape: RoundedCornerShape,
    onPreview: () -> Unit,
    onEdit: () -> Unit,
    onRename: () -> Unit,
    onDuplicate: () -> Unit,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    Surface(
        shape = shape,
        color = if (selected) colors.secondaryContainer else colors.surfaceContainer,
        onClick = onPreview,
    ) {
        Row(
            Modifier.padding(start = 14.dp, top = 10.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Surface(
                modifier = Modifier.size(40.dp),
                shape = RoundedCornerShape(14.dp),
                color = if (selected) colors.primary else colors.surfaceContainerHighest,
            ) {
                Box(contentAlignment = Alignment.Center) {
                    if (selected) {
                        Icon(Icons.Rounded.Check, null, tint = colors.onPrimary)
                    } else {
                        Icon(Icons.Rounded.Speed, null, tint = colors.onSurfaceVariant)
                    }
                }
            }
            Column(Modifier.padding(start = 14.dp).weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        preset.name,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                    if (preset.builtin) {
                        Text(
                            "  Built-in",
                            style = MaterialTheme.typography.labelSmall,
                            color = colors.onSurfaceVariant,
                        )
                    }
                }
                Text(
                    preset.summary,
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (!preset.builtin) {
                Box {
                    IconButton(onClick = { menu = true }) {
                        Icon(Icons.Rounded.MoreVert, "More for ${preset.name}")
                    }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(
                            text = { Text("Edit values") },
                            leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                            onClick = { menu = false; onEdit() },
                        )
                        DropdownMenuItem(
                            text = { Text("Rename") },
                            leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                            onClick = { menu = false; onRename() },
                        )
                        DropdownMenuItem(
                            text = { Text("Duplicate") },
                            leadingIcon = { Icon(Icons.Rounded.ContentCopy, null) },
                            onClick = { menu = false; onDuplicate() },
                        )
                        DropdownMenuItem(
                            text = { Text("Delete") },
                            leadingIcon = { Icon(Icons.Rounded.DeleteOutline, null) },
                            onClick = { menu = false; onDelete() },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun PerformancePreviewDialog(
    preview: PerformancePreview,
    state: PerformanceState?,
    onDismiss: () -> Unit,
    onApply: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Apply ${preview.name}?") },
        text = {
            Column(Modifier.heightIn(max = 440.dp).verticalScroll(rememberScrollState())) {
                if (preview.noChanges) {
                    Text(
                        "The supported settings already match this preset. Nothing will be written.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                } else {
                    Text(
                        "${preview.changes.size} setting(s) will change",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.padding(bottom = 8.dp),
                    )
                    preview.changes.forEach { change ->
                        val current = state?.settings?.firstOrNull { it.id == change.id }
                        Column(Modifier.padding(vertical = 5.dp)) {
                            Text(
                                change.label,
                                style = MaterialTheme.typography.bodyLarge,
                                fontWeight = FontWeight.Medium,
                            )
                            Text(
                                "${current?.display(change.current) ?: change.current} → ${formatPreviewValue(change, state)}",
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            if (change.restart) {
                                Text(
                                    "Needs a game restart",
                                    style = MaterialTheme.typography.labelMedium,
                                    color = MaterialTheme.colorScheme.primary,
                                )
                            }
                        }
                    }
                }
                if (preview.restart) {
                    Text(
                        "Restart the game after applying to use the new worker-thread count.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 10.dp),
                    )
                }
            }
        },
        confirmButton = {
            Button(onClick = onApply, enabled = !preview.noChanges) { Text("Apply") }
        },
        dismissButton = { DiscardButton(onDismiss) },
    )
}

@Composable
private fun PerformanceEditorDialog(
    title: String,
    initialName: String,
    initialValues: JSONObject,
    settings: List<PerformanceSetting>,
    error: String?,
    onDismiss: () -> Unit,
    onSave: (String, Map<String, Any?>) -> Unit,
) {
    var name by remember { mutableStateOf(initialName) }
    val values = remember {
        mutableStateMapOf<String, Any?>().apply {
            initialValues.keys().forEach { key -> put(key, initialValues.opt(key)) }
        }
    }
    var editing by remember { mutableStateOf<PerformanceSetting?>(null) }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(Modifier.heightIn(max = 480.dp)) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { if (it.length <= 32) name = it },
                    label = { Text("Preset name") },
                    singleLine = true,
                    supportingText = { Text("${name.length}/32") },
                    modifier = Modifier.fillMaxWidth(),
                )
                error?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                Text(
                    "Tap a setting to change it. Only these supported settings are saved.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 10.dp),
                )
                LazyColumn(Modifier.weight(1f, fill = false)) {
                    items(settings, key = { it.id }) { setting ->
                        Surface(
                            onClick = { editing = setting },
                            color = Color.Transparent,
                        ) {
                            ListItem(
                                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                headlineContent = { Text(setting.label) },
                                supportingContent = {
                                    Text(
                                        setting.display(values[setting.id]),
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                },
                                trailingContent = { Icon(Icons.Rounded.Edit, null) },
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {
            SaveButton(enabled = name.isNotBlank() && values.isNotEmpty()) {
                onSave(name.trim(), values.toMap())
            }
        },
        dismissButton = { DiscardButton(onDismiss) },
    )

    editing?.let { setting ->
        PerformanceValueDialog(
            setting = setting,
            initial = values[setting.id],
            onDismiss = { editing = null },
            onSave = { value ->
                values[setting.id] = value
                editing = null
            },
        )
    }
}

@Composable
private fun PerformanceValueDialog(
    setting: PerformanceSetting,
    initial: Any?,
    onDismiss: () -> Unit,
    onSave: (Any?) -> Unit,
) {
    when (setting.kind) {
        "int" -> {
            val steps = ((setting.max - setting.min) / setting.step).coerceAtLeast(1)
            var position by remember(setting.id) {
                val start = (initial as? Number)?.toInt() ?: setting.min
                mutableStateOf((((start - setting.min).coerceIn(0, steps * setting.step)) / setting.step).toFloat())
            }
            val value = (setting.min + position.roundToInt() * setting.step).coerceIn(setting.min, setting.max)
            AlertDialog(
                onDismissRequest = onDismiss,
                title = { Text(setting.label) },
                text = {
                    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(
                            setting.help,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            setting.display(value),
                            style = MaterialTheme.typography.headlineSmall,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.align(Alignment.CenterHorizontally),
                        )
                        Slider(
                            value = position,
                            onValueChange = { position = it },
                            valueRange = 0f..steps.toFloat(),
                            // Hundreds of tick marks are noise; the value still snaps below.
                            steps = if (steps <= 12) (steps - 1).coerceAtLeast(0) else 0,
                        )
                        Row(Modifier.fillMaxWidth()) {
                            Text(
                                setting.display(setting.min),
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Spacer(Modifier.weight(1f))
                            Text(
                                setting.display(setting.max),
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                },
                confirmButton = { SaveButton { onSave(value) } },
                dismissButton = { DiscardButton(onDismiss) },
            )
        }
        "bool" -> {
            var checked by remember(setting.id) { mutableStateOf(initial == true) }
            AlertDialog(
                onDismissRequest = onDismiss,
                title = { Text(setting.label) },
                text = {
                    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text(setting.help, style = MaterialTheme.typography.bodyMedium)
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(if (checked) "On" else "Off", modifier = Modifier.weight(1f))
                            HorizonSwitch(checked = checked, onCheckedChange = { checked = it })
                        }
                    }
                },
                confirmButton = { SaveButton { onSave(checked) } },
                dismissButton = { DiscardButton(onDismiss) },
            )
        }
        else -> {
            var picked by remember(setting.id) { mutableStateOf(initial?.toString()) }
            AlertDialog(
                onDismissRequest = onDismiss,
                title = { Text(setting.label) },
                text = {
                    Column(
                        Modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState()),
                        verticalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        Text(
                            setting.help,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(bottom = 8.dp),
                        )
                        setting.options.forEachIndexed { index, option ->
                            ChoiceTile(
                                label = option.label,
                                detail = "",
                                selected = picked == option.value,
                                shape = connectedShape(index, setting.options.size),
                                onClick = { picked = option.value },
                            )
                        }
                    }
                },
                confirmButton = {
                    SaveButton(enabled = picked != null) { onSave(picked) }
                },
                dismissButton = { DiscardButton(onDismiss) },
            )
        }
    }
}
