package dev.horizon.android.launcher

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import dev.horizon.android.R
import org.json.JSONObject

private val FIXED_LABEL_KEYS = listOf(
    R.string.touch_control_move, R.string.touch_control_sprint, R.string.touch_control_jump,
    R.string.touch_control_sneak, R.string.touch_control_attack, R.string.touch_control_use,
    R.string.touch_control_chat, R.string.touch_control_pause, R.string.touch_control_settings,
    R.string.touch_control_inventory, R.string.touch_control_debug, R.string.touch_control_perspective,
    R.string.touch_control_offhand, R.string.touch_control_shield,
)
private val MODE_ROWS = mapOf("crosshair" to 0, "tap" to 1, "dpad" to 2)

internal data class EditableControl(
    val slot: Int,
    val label: String,
    val custom: Boolean,
    val x: Float,
    val y: Float,
    val size: Float,
)

private fun modeRow(snapshot: JSONObject): Int = MODE_ROWS[snapshot.optString("mode", "crosshair")] ?: 0

private fun parseControls(snapshot: JSONObject, labels: List<String>, customFallback: String): List<EditableControl> {
    val rowIndex = modeRow(snapshot)
    val result = mutableListOf<EditableControl>()
    val row = snapshot.optJSONArray("layouts")?.optJSONArray(rowIndex)
    if (row != null) {
        for (index in 0 until minOf(row.length(), labels.size)) {
            val placement = row.optJSONObject(index) ?: continue
            val center = placement.optJSONArray("center") ?: continue
            result += EditableControl(
                slot = index,
                label = labels[index],
                custom = false,
                x = center.optDouble(0, 0.5).toFloat().coerceIn(0f, 1f),
                y = center.optDouble(1, 0.5).toFloat().coerceIn(0f, 1f),
                size = placement.optDouble("size", 0.12).toFloat().coerceIn(0.04f, 0.4f),
            )
        }
    }
    val customs = snapshot.optJSONArray("custom_buttons")
    if (customs != null) {
        for (index in 0 until customs.length()) {
            val placement = customs.optJSONObject(index)
                ?.optJSONArray("layouts")?.optJSONArray(rowIndex)?.optJSONObject(0) ?: continue
            val center = placement.optJSONArray("center") ?: continue
            result += EditableControl(
                slot = index,
                label = stringKey(customs.optJSONObject(index)?.optString("key").orEmpty(), customFallback),
                custom = true,
                x = center.optDouble(0, 0.5).toFloat().coerceIn(0f, 1f),
                y = center.optDouble(1, 0.5).toFloat().coerceIn(0f, 1f),
                size = placement.optDouble("size", 0.12).toFloat().coerceIn(0.04f, 0.4f),
            )
        }
    }
    return result
}

private fun stringKey(key: String, fallback: String): String =
    key.removePrefix("key.keyboard.").ifBlank { fallback }

/** Applies the draft's placements to a copy of the snapshot for future native import. */
private fun serializeDraft(snapshot: JSONObject, controls: List<EditableControl>): JSONObject {
    val copy = runCatching { JSONObject(snapshot.toString()) }.getOrElse { return snapshot }
    val rowIndex = modeRow(copy)
    val row = copy.optJSONArray("layouts")?.optJSONArray(rowIndex)
    controls.filter { !it.custom }.forEach { control ->
        val placement = row?.optJSONObject(control.slot) ?: return@forEach
        val center = placement.optJSONArray("center") ?: return@forEach
        runCatching {
            center.put(0, control.x.toDouble())
            center.put(1, control.y.toDouble())
            placement.put("size", control.size.toDouble())
        }
    }
    val customs = copy.optJSONArray("custom_buttons")
    controls.filter { it.custom }.forEach { control ->
        val placement = customs?.optJSONObject(control.slot)
            ?.optJSONArray("layouts")?.optJSONArray(rowIndex)?.optJSONObject(0) ?: return@forEach
        val center = placement.optJSONArray("center") ?: return@forEach
        runCatching {
            center.put(0, control.x.toDouble())
            center.put(1, control.y.toDouble())
            placement.put("size", control.size.toDouble())
        }
    }
    return copy
}

/**
 * Lightweight launcher-side editor. It previews a draft on a phone-shaped
 * surface, lets controls be dragged and resized and custom buttons added or
 * removed, then saves through the existing profile actions. Precise placement
 * is still finished in the in-game editor via [onEditInGame].
 */
@Composable
fun TouchDraftEditor(
    snapshot: JSONObject,
    onDismiss: () -> Unit,
    onSave: (JSONObject) -> Unit,
    onEditInGame: () -> Unit,
) {
    val fixedLabels = FIXED_LABEL_KEYS.map { stringResource(it) }
    val customFallback = stringResource(R.string.touch_editor_custom_key)
    val controls = remember(snapshot, fixedLabels, customFallback) {
        mutableStateListOf<EditableControl>().apply { addAll(parseControls(snapshot, fixedLabels, customFallback)) }
    }
    var selected by remember { mutableStateOf(-1) }
    var viewport by remember { mutableStateOf(IntSize.Zero) }
    val density = LocalDensity.current
    val hint = stringResource(R.string.touch_editor_hint)
    val moveHint = stringResource(R.string.touch_editor_move)
    val sizeLabel = stringResource(R.string.touch_editor_size)

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                stringResource(R.string.touch_editor),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
                modifier = Modifier.weight(1f),
            )
            OutlinedButton(onClick = onDismiss) { Text(stringResource(R.string.touch_editor_done)) }
        }
        Text(hint, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(12.dp))

        // Phone-shaped preview; controls sit at normalized coordinates.
        Box(
            Modifier
                .widthIn(max = 280.dp)
                .aspectRatio(9f / 19.5f)
                .clip(RoundedCornerShape(28.dp))
                .background(MaterialTheme.colorScheme.surfaceContainer)
                .align(Alignment.CenterHorizontally)
                .onSizeChanged { viewport = it },
        ) {
            controls.forEachIndexed { index, control ->
                val radiusPx = (control.size * viewport.height / 2f).coerceAtLeast(2f)
                val radiusDp = with(density) { radiusPx.toDp() }
                val selectedHere = selected == index
                Box(
                    Modifier
                        .offset {
                            IntOffset(
                                (control.x * viewport.width - radiusPx).toInt(),
                                (control.y * viewport.height - radiusPx).toInt(),
                            )
                        }
                        .size(radiusDp * 2)
                        .clip(if (control.custom) RoundedCornerShape(6.dp) else androidx.compose.foundation.shape.CircleShape)
                        .background(
                            when {
                                selectedHere -> MaterialTheme.colorScheme.primary
                                control.custom -> MaterialTheme.colorScheme.tertiary
                                else -> MaterialTheme.colorScheme.secondary
                            },
                        )
                        .pointerInput(index) {
                            detectDragGestures(onDragStart = { selected = index }) { change, dragAmount ->
                                change.consume()
                                val width = viewport.width.takeIf { it > 0 }?.toFloat() ?: return@detectDragGestures
                                val height = viewport.height.takeIf { it > 0 }?.toFloat() ?: return@detectDragGestures
                                val current = controls[index]
                                controls[index] = current.copy(
                                    x = (current.x + dragAmount.x / width).coerceIn(0f, 1f),
                                    y = (current.y + dragAmount.y / height).coerceIn(0f, 1f),
                                )
                                selected = index
                            }
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        control.label.take(4),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onPrimary,
                        maxLines = 1,
                    )
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        Text(moveHint, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)

        Spacer(Modifier.height(16.dp))
        if (selected in controls.indices) {
            Text(sizeLabel, style = MaterialTheme.typography.titleSmall)
            Slider(
                value = controls[selected].size,
                onValueChange = { value ->
                    val current = controls[selected]
                    controls[selected] = current.copy(size = value.coerceIn(0.04f, 0.4f))
                },
                valueRange = 0.04f..0.4f,
            )
            if (controls[selected].custom) {
                OutlinedButton(onClick = { controls.removeAt(selected); selected = -1 }) {
                    Icon(Icons.Rounded.DeleteOutline, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.touch_editor_remove))
                }
            }
        }
        Spacer(Modifier.height(8.dp))
        OutlinedButton(
            onClick = {
                controls += EditableControl(
                    slot = controls.count { it.custom },
                    label = stringResource(R.string.touch_editor_custom_key),
                    custom = true,
                    x = 0.85f,
                    y = 0.5f,
                    size = 0.12f,
                )
                selected = controls.lastIndex
            },
            modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
        ) {
            Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.touch_editor_add))
        }
        Spacer(Modifier.height(8.dp))
        Button(
            onClick = { onSave(serializeDraft(snapshot, controls)); onDismiss() },
            modifier = Modifier.fillMaxWidth().height(48.dp),
        ) { Text(stringResource(R.string.touch_editor_save)) }
        Spacer(Modifier.height(8.dp))
        FilledTonalButton(
            onClick = { onEditInGame(); onDismiss() },
            modifier = Modifier.fillMaxWidth().height(48.dp),
        ) { Text(stringResource(R.string.touch_editor_edit_game)) }
        Spacer(Modifier.height(16.dp))
    }
}
