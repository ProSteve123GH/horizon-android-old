package dev.horizon.android.launcher

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.abs

/** The exact game atlas texture names drawn by `horizon-client-render/src/touch.rs`. */
private val EDITOR_TEXTURES = listOf(
    "ui/joystick_frame", // Move
    "ui/sprint",         // Sprint
    "ui/jump",           // Jump
    "ui/sneak",          // Sneak
    "ui/attack",         // Attack
    "ui/interact",       // Use
    "gui",               // Chat
    "ui/pause_icon",     // Pause
    "ui/gear",           // Settings
    "ui/more-dots",      // Inventory
    "gui",               // Debug
    "gui",               // Perspective
    "ui/joystick_knob",  // Offhand
    "ui/joystick_knob",  // Shield
)

private val CONTROL_NAMES = listOf(
    "Move", "Sprint", "Jump", "Sneak", "Attack", "Use", "Chat", "Pause",
    "Settings", "Inventory", "Debug", "Perspective", "Offhand", "Shield",
)

/** A dedicated screen for editing one saved layout with the in-game textures. */
@Composable
fun TouchLayoutEditor(profileId: String, onDismiss: () -> Unit) {
    var snapshot by remember { mutableStateOf<JSONObject?>(null) }
    var profileName by remember { mutableStateOf("") }
    var textures by remember { mutableStateOf<Map<String, ImageBitmap>>(emptyMap()) }
    var selected by remember { mutableStateOf(-1) }
    var error by remember { mutableStateOf<String?>(null) }
    var status by remember { mutableStateOf<String?>(null) }
    BackHandler { onDismiss() }

    LaunchedEffect(profileId) {
        val reply = act("touch_layouts")
        val root = reply.ok as? JSONObject
        val profiles = root?.optJSONArray("profiles")
        var found: JSONObject? = null
        if (profiles != null) {
            for (index in 0 until profiles.length()) {
                val item = profiles.optJSONObject(index) ?: continue
                if (item.optString("id") == profileId) {
                    found = item.optJSONObject("snapshot")
                    profileName = item.optString("name")
                    break
                }
            }
        }
        snapshot = found?.let { JSONObject(it.toString()) }
        if (snapshot == null) error = reply.error ?: "That layout profile is no longer available"
        val loaded = mutableMapOf<String, ImageBitmap>()
        EDITOR_TEXTURES.distinct().forEach { key ->
            val texture = act("touch_texture", key)
            val encoded = texture.ok as? String ?: return@forEach
            val bytes = Base64.decode(encoded, Base64.DEFAULT)
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.let { loaded[key] = it.asImageBitmap() }
        }
        textures = loaded
    }

    val current = snapshot
    val mode = current?.optString("mode", "crosshair") ?: "crosshair"
    val rowIndex = when (mode) {
        "tap" -> 1
        "dpad" -> 2
        else -> 0
    }

    fun row(): JSONArray? = current?.optJSONArray("layouts")?.optJSONArray(rowIndex)

    fun place(index: Int, x: Double, y: Double) {
        val root = current ?: return
        val placement = row()?.optJSONObject(index) ?: return
        placement.put(
            "center",
            JSONArray().put(x.coerceIn(0.0, 1.0)).put(y.coerceIn(0.0, 1.0)),
        )
        snapshot = JSONObject(root.toString())
        status = null
    }

    fun toggleVisibility(index: Int) {
        val root = current ?: return
        val visibility = root.optJSONArray("button_visibility") ?: return
        val value = visibility.optInt(index, 0)
        visibility.put(index, if (value == 2) 0 else 2)
        snapshot = JSONObject(root.toString())
    }

    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(
                    "Edit $profileName",
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    "$mode mode · drag a button to move it",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            OutlinedButton(onClick = onDismiss) { Text("Close") }
        }

        Canvas(
            Modifier
                .fillMaxWidth()
                .weight(1f)
                .pointerInput(current) {
                    detectDragGestures(
                        onDragStart = { offset ->
                            val row = row() ?: return@detectDragGestures
                            selected = (0 until minOf(row.length(), EDITOR_TEXTURES.size))
                                .filter { row.optJSONObject(it) != null }
                                .minByOrNull { index ->
                                    val center = row.optJSONObject(index)?.optJSONArray("center")
                                        ?: return@minByOrNull Float.MAX_VALUE
                                    val x = center.optDouble(0).toFloat() * size.width
                                    val y = center.optDouble(1).toFloat() * size.height
                                    abs(offset.x - x) + abs(offset.y - y)
                                } ?: -1
                        },
                        onDrag = { change, _ ->
                            if (selected >= 0) {
                                place(selected, change.position.x / size.width, change.position.y / size.height)
                            }
                        },
                    )
                },
        ) {
            val row = row() ?: return@Canvas
            val visibility = current?.optJSONArray("button_visibility")
            for (index in 0 until minOf(row.length(), EDITOR_TEXTURES.size)) {
                val placement = row.optJSONObject(index) ?: continue
                val center = placement.optJSONArray("center") ?: continue
                val x = center.optDouble(0).toFloat() * size.width
                val y = center.optDouble(1).toFloat() * size.height
                val side = (placement.optDouble("size").takeIf { !it.isNaN() } ?: 0.15)
                    .toFloat() * size.height
                val hidden = visibility?.optInt(index, 0) == 2
                val key = EDITOR_TEXTURES[index]
                val image = textures[key]
                if (image != null) {
                    drawImage(
                        image,
                        Offset(x - side / 2f, y - side / 2f),
                        alpha = if (hidden) 0.35f else 1f,
                    )
                } else {
                    drawCircle(
                        color = if (hidden) Color.Gray else MaterialTheme.colorScheme.primary,
                        radius = side / 2f,
                        center = Offset(x, y),
                    )
                }
                if (index == selected) {
                    drawCircle(
                        color = MaterialTheme.colorScheme.primary,
                        radius = side / 2f + 4.dp.toPx(),
                        center = Offset(x, y),
                        style = Stroke(2.dp.toPx()),
                    )
                }
            }
        }

        Text(
            if (selected >= 0) {
                "Selected: ${CONTROL_NAMES.getOrNull(selected) ?: "control $selected"}"
            } else {
                "Tap and drag a button to select and move it."
            },
            style = MaterialTheme.typography.bodyMedium,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val root = current ?: return@Button
                    val request = JSONObject()
                        .put("id", profileId)
                        .put("snapshot", JSONObject(root.toString()))
                    val reply = act("touch_snapshot_save", request.toString())
                    error = reply.error
                    status = if (reply.error == null) "Saved $profileName." else null
                },
                modifier = Modifier.weight(1f).heightIn(min = 48.dp),
            ) { Text("Save layout") }
            OutlinedButton(
                onClick = { if (selected >= 0) toggleVisibility(selected) },
                enabled = selected >= 0,
                modifier = Modifier.weight(1f).heightIn(min = 48.dp),
            ) { Text("Toggle visibility") }
        }
        Text(
            "Custom keys keep their own placements; edit those in the game's Controls menu.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        status?.let { Text(it, color = MaterialTheme.colorScheme.primary) }
        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        Spacer(Modifier.height(4.dp))
    }
}
