package dev.horizon.android.launcher

import android.graphics.Paint
import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.DashboardCustomize
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.RestartAlt
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
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

private fun JSONObject.nullableString(key: String): String? {
    val value = opt(key)
    return (value as? String)?.takeIf { it.isNotBlank() }
}

/** The fixed controls, in the engine's `Control::ALL` order (bridge.rs CONTROL_LABELS). */
private val TOUCH_CONTROL_LABELS = listOf(
    "Move", "Sprint", "Jump", "Sneak", "Attack", "Use", "Chat", "Pause", "Settings", "Inventory",
    "Debug", "Perspective", "Offhand", "Shield",
)

/** Which placements row a snapshot's control mode draws, the engine's `Mode::index`. */
private val TOUCH_MODE_ROWS = mapOf("crosshair" to 0, "tap" to 1, "dpad" to 2)

internal data class TouchProfile(
    val id: String,
    val name: String,
    val selected: Boolean,
    val valid: Boolean,
    val snapshot: JSONObject,
) {
    companion object {
        fun parse(item: JSONObject): TouchProfile = TouchProfile(
            id = item.optString("id"),
            name = item.optString("name"),
            selected = item.optBoolean("selected"),
            valid = item.optBoolean("valid", true),
            snapshot = item.optJSONObject("snapshot") ?: JSONObject(),
        )
    }
}

internal data class TouchLayoutsState(
    val instance: String,
    val instanceId: Int,
    val selected: String?,
    val selectedName: String?,
    val profiles: List<TouchProfile>,
    val modeLabel: String,
    val warning: String?,
) {
    companion object {
        fun parse(root: JSONObject): TouchLayoutsState = TouchLayoutsState(
            instance = root.optString("instance"),
            instanceId = root.optInt("instance_id"),
            selected = root.nullableString("selected"),
            selectedName = root.nullableString("selected_name"),
            profiles = root.optJSONArray("profiles")?.let { list ->
                List(list.length()) { index -> TouchProfile.parse(list.getJSONObject(index)) }
            }.orEmpty(),
            modeLabel = root.optString("mode_label"),
            warning = root.nullableString("warning"),
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun TouchLayoutsSheet(onDismiss: () -> Unit) {
    var state by remember { mutableStateOf<TouchLayoutsState?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var applying by remember { mutableStateOf<TouchProfile?>(null) }
    var renaming by remember { mutableStateOf<TouchProfile?>(null) }
    var deleting by remember { mutableStateOf<TouchProfile?>(null) }
    var resetting by remember { mutableStateOf(false) }
    var naming by remember { mutableStateOf(false) }

    val reload: () -> Unit = {
        val reply = act("touch_layouts")
        val root = reply.ok as? JSONObject
        if (root == null) {
            error = reply.error ?: "Could not load touch layout profiles"
        } else {
            state = TouchLayoutsState.parse(root)
            error = null
        }
    }
    LaunchedEffect(Unit) { reload() }

    fun finish(action: () -> Reply, failure: String) {
        val reply = action()
        error = if (reply.error != null) reply.error ?: failure else null
        reload()
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
                                    "Touch layout profiles",
                                    style = MaterialTheme.typography.headlineSmall,
                                    fontWeight = FontWeight.SemiBold,
                                )
                                Text(
                                    "For ${current.instance} · ${current.modeLabel}. Move and resize the " +
                                        "controls themselves inside the game's Options › Controls; a profile " +
                                        "snapshots that arrangement so you can switch between saved ones.",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            Spacer(Modifier.width(12.dp))
                            Button(
                                onClick = {
                                    error = null
                                    naming = true
                                },
                                contentPadding = ButtonDefaults.ButtonWithIconContentPadding,
                            ) {
                                Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
                                Spacer(Modifier.width(8.dp))
                                Text("New")
                            }
                        }
                        current.selectedName?.let { name ->
                            Text(
                                "Applying \"$name\"",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.primary,
                                modifier = Modifier.padding(top = 8.dp),
                            )
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
                items(current.profiles, key = { it.id }) { profile ->
                    ProfileCard(
                        profile = profile,
                        shape = RoundedCornerShape(20.dp),
                        onApply = { applying = profile },
                        onRename = { renaming = profile },
                        onSave = {
                            finish(
                                { act("touch_layout_save", profile.id) },
                                "Could not update ${profile.name}",
                            )
                        },
                        onDuplicate = {
                            finish(
                                { act("touch_layout_duplicate", profile.id) },
                                "Could not duplicate ${profile.name}",
                            )
                        },
                        onDelete = { deleting = profile },
                    )
                }
                if (current.profiles.isEmpty()) {
                    item(key = "empty") {
                        Text(
                            "No layout profiles yet. \"New\" snapshots the controls as they are set up " +
                                "in the game right now.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(start = 8.dp, top = 8.dp, bottom = 8.dp),
                        )
                    }
                }
                item(key = "reset") {
                    OutlinedButton(
                        onClick = { resetting = true },
                        modifier = Modifier.fillMaxWidth().padding(top = 12.dp).heightIn(min = 48.dp),
                    ) {
                        Icon(Icons.Rounded.RestartAlt, null, Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text("Reset to the default layout")
                    }
                }
                item(key = "note") {
                    Text(
                        "A profile keeps the control mode, every placement, button visibility and the " +
                            "positions of your custom keys. Applying one replaces those in the current " +
                            "instance; sensitivity, handedness, the dynamic joystick, the gyroscope and " +
                            "your key actions are never changed, and a key you added later keeps its own " +
                            "position. Changes apply when the touch controls are next reloaded.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 8.dp, top = 8.dp, end = 8.dp),
                    )
                }
            }
        }
    }

    applying?.let { profile ->
        AlertDialog(
            onDismissRequest = { applying = null },
            title = { Text("Apply ${profile.name}?") },
            text = {
                Column(Modifier.heightIn(max = 460.dp)) {
                    if (profile.valid) {
                        TouchPreview(
                            profile.snapshot,
                            Modifier
                                .fillMaxWidth()
                                .height(220.dp)
                                .padding(bottom = 10.dp),
                        )
                    } else {
                        Text(
                            "This snapshot is not readable by this build and cannot be applied.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.error,
                            modifier = Modifier.padding(bottom = 10.dp),
                        )
                    }
                    Text(
                        "The current control mode, placements, button visibility and custom-key " +
                            "positions in this instance are replaced. Save the arrangement you have " +
                            "now as a new profile first if you want to switch back to it.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            },
            confirmButton = {
                Button(
                    enabled = profile.valid,
                    onClick = {
                        finish(
                            { act("touch_layout_apply", profile.id) },
                            "Could not apply ${profile.name}",
                        )
                        applying = null
                    },
                ) { Text("Apply") }
            },
            dismissButton = { TextButton(onClick = { applying = null }) { Text("Cancel") } },
        )
    }

    renaming?.let { profile ->
        NameDialog(
            title = "Rename layout profile",
            initial = profile.name,
            confirm = "Save",
            onDismiss = { renaming = null },
        ) { name ->
            val request = JSONObject().put("id", profile.id).put("name", name)
            finish({ act("touch_layout_rename", request.toString()) }, "Could not rename ${profile.name}")
            renaming = null
        }
    }

    if (naming) {
        NameDialog(
            title = "New layout profile",
            initial = "",
            confirm = "Save",
            onDismiss = { naming = false },
        ) { name ->
            finish({ act("touch_layout_create", name) }, "Could not save the layout profile")
            naming = false
        }
    }

    deleting?.let { profile ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text("Delete ${profile.name}?") },
            text = {
                Text(
                    "Only this snapshot is removed. The touch layout keeps whatever is applied right now.",
                )
            },
            confirmButton = {
                Button(onClick = {
                    finish(
                        { act("touch_layout_delete", profile.id) },
                        "Could not delete ${profile.name}",
                    )
                    deleting = null
                }) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text("Cancel") } },
        )
    }

    if (resetting) {
        AlertDialog(
            onDismissRequest = { resetting = false },
            title = { Text("Reset to the default layout?") },
            text = {
                Text(
                    "The placements and button visibility go back to the factory defaults and the active " +
                        "profile is cleared. Custom keys are kept with their key actions and return to the " +
                        "factory grid; sensitivity, handedness and the gyroscope stay as they are.",
                )
            },
            confirmButton = {
                Button(onClick = {
                    finish({ act("touch_layout_reset") }, "Could not restore the default layout")
                    resetting = false
                }) { Text("Reset") }
            },
            dismissButton = { TextButton(onClick = { resetting = false }) { Text("Cancel") } },
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ProfileCard(
    profile: TouchProfile,
    shape: RoundedCornerShape,
    onApply: () -> Unit,
    onRename: () -> Unit,
    onSave: () -> Unit,
    onDuplicate: () -> Unit,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    Surface(
        shape = shape,
        color = if (profile.selected) colors.secondaryContainer else colors.surfaceContainer,
        onClick = onApply,
    ) {
        Column(Modifier.padding(14.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Surface(
                    modifier = Modifier.size(40.dp),
                    shape = RoundedCornerShape(14.dp),
                    color = if (profile.selected) colors.primary else colors.surfaceContainerHighest,
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        if (profile.selected) {
                            Icon(Icons.Rounded.Check, null, tint = colors.onPrimary)
                        } else {
                            Icon(Icons.Rounded.DashboardCustomize, null, tint = colors.onSurfaceVariant)
                        }
                    }
                }
                Column(Modifier.padding(start = 14.dp).weight(1f)) {
                    Text(
                        profile.name,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        if (profile.selected) "Applied" else "Tap to apply",
                        style = MaterialTheme.typography.bodySmall,
                        color = colors.onSurfaceVariant,
                    )
                }
                Box {
                    IconButton(onClick = { menu = true }) {
                        Icon(Icons.Rounded.MoreVert, "More for ${profile.name}")
                    }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(
                            text = { Text("Save the current layout here") },
                            leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                            onClick = { menu = false; onSave() },
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
            Spacer(Modifier.height(10.dp))
            if (profile.valid) {
                TouchPreview(
                    profile.snapshot,
                    Modifier
                        .fillMaxWidth()
                        .height(180.dp),
                )
            } else {
                Text(
                    "This snapshot is not readable by this build.",
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.error,
                )
            }
        }
    }
}

/**
 * A schematic preview of a snapshot: controls sit where the engine puts them,
 * at normalized coordinates on the given box. A hidden control is an outline.
 */
@Composable
private fun TouchPreview(snapshot: JSONObject, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Canvas(modifier) {
        val mode = snapshot.optString("mode", "crosshair")
        val chosenRow = TOUCH_MODE_ROWS[mode] ?: 0
        val row = snapshot.optJSONArray("layouts")?.optJSONArray(chosenRow)
        val visibility = snapshot.optJSONArray("button_visibility")
        val fill = colors.primary.copy(alpha = 0.5f)
        val dim = colors.onSurfaceVariant.copy(alpha = 0.9f)
        if (row != null) {
            for (index in 0 until row.length()) {
                val placement = row.optJSONObject(index) ?: continue
                val center = placement.optJSONArray("center") ?: continue
                val x = center.optDouble(0)
                val y = center.optDouble(1)
                val controlSize = placement.optDouble("size")
                if (x.isNaN() || y.isNaN() || controlSize.isNaN()) continue
                val point = Offset(x.toFloat() * size.width, y.toFloat() * size.height)
                // The engine sizes a control by surface height, so both sides of
                // the placement rectangle are `size * height` pixels wide.
                val radius = (controlSize.toFloat() * size.height / 2f).coerceAtLeast(1f)
                val hidden = visibility?.optInt(index, 0) == 2
                drawCircle(
                    color = if (hidden) dim else fill,
                    radius = radius,
                    center = point,
                    style = if (hidden) Stroke(1.5.dp.toPx()) else Fill,
                )
                val label = TOUCH_CONTROL_LABELS.getOrNull(index)
                if (label != null && radius >= 11.dp.toPx()) {
                    drawIntoCanvas { canvas ->
                        val paint = Paint()
                        paint.isAntiAlias = true
                        paint.textAlign = Paint.Align.CENTER
                        paint.textSize = 9.dp.toPx()
                        paint.color = if (hidden) dim.toArgb() else colors.onPrimary.toArgb()
                        canvas.nativeCanvas.drawText(
                            label,
                            point.x,
                            point.y + paint.textSize / 3f,
                            paint,
                        )
                    }
                }
            }
        }
        val buttons = snapshot.optJSONArray("custom_buttons")
        if (buttons != null) {
            for (button in 0 until buttons.length()) {
                val custom = buttons.optJSONObject(button) ?: continue
                val key = custom.optString("key")
                val placements = custom.optJSONArray("layouts")?.optJSONArray(chosenRow) ?: continue
                val placement = placements.optJSONObject(0) ?: continue
                val center = placement.optJSONArray("center") ?: continue
                val x = center.optDouble(0)
                val y = center.optDouble(1)
                val controlSize = placement.optDouble("size")
                if (x.isNaN() || y.isNaN() || controlSize.isNaN()) continue
                val point = Offset(x.toFloat() * size.width, y.toFloat() * size.height)
                val radius = (controlSize.toFloat() * size.height / 2f).coerceAtLeast(1f)
                drawRoundRect(
                    color = colors.tertiary.copy(alpha = 0.55f),
                    topLeft = Offset(point.x - radius, point.y - radius),
                    size = androidx.compose.ui.geometry.Size(2f * radius, 2f * radius),
                )
                if (radius >= 11.dp.toPx()) {
                    drawIntoCanvas { canvas ->
                        val paint = Paint()
                        paint.isAntiAlias = true
                        paint.textAlign = Paint.Align.CENTER
                        paint.textSize = 9.dp.toPx()
                        paint.color = colors.onTertiaryContainer.toArgb()
                        canvas.nativeCanvas.drawText(
                            customButtonLabel(key),
                            point.x,
                            point.y,
                            paint,
                        )
                    }
                }
            }
        }
    }
}

/** A short readable label for a custom key token in the preview. */
private fun customButtonLabel(key: String): String = when {
    key.isBlank() || key == "key.keyboard.unknown" -> "Key"
    key.startsWith("action:") -> key.removePrefix("action:").replace('_', ' ')
    key.startsWith("key.keyboard.") -> key.removePrefix("key.keyboard.").replace('_', ' ')
        .replaceFirstChar { it.uppercase() }
    else -> "Key"
}
