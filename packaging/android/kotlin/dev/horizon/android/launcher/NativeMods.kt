package dev.horizon.android.launcher

import android.app.ActivityManager
import android.content.Context
import android.os.Build
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Build
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * The native-mods section of the Mods page. Native (`.hnmod`) bundles are
 * source-only Rust that must be compiled into the game; the compile happens
 * on a GitHub-hosted runner through the personal build service configured
 * for this fork. Nothing is executed on this device: the runner compiles,
 * downloads, and verifies; the loader decides at the next start.
 */
@Composable
fun NativeModsSection(native: NativeMods, actions: LauncherActions) {
    val context = LocalContext.current
    val versionCode = remember(context) { apkVersionCode(context) }
    val abi = remember {
        Build.SUPPORTED_ABIS.firstOrNull { it == "arm64-v8a" }
            ?: Build.SUPPORTED_ABIS.firstOrNull()
            ?: "arm64-v8a"
    }
    val stock = remember { context.applicationInfo.nativeLibraryDir + File.separator + "libhorizon_android.so" }

    var rejectedDismissed by remember { mutableStateOf(false) }
    var showCompileConfirm by remember { mutableStateOf(false) }
    var detailsMod by remember { mutableStateOf<NativeMod?>(null) }
    var restartNote by remember { mutableStateOf<String?>(null) }

    val current = native.current
    val build = native.build
    val enabledIds = native.mods.filter { it.enabled }.map { it.id }
    val inFlight = build?.state in setOf("requesting", "queued", "building", "installing")
    val nothingEnabled = native.mods.isNotEmpty() && enabledIds.isEmpty()
    val compileDisabled = native.mods.isEmpty() || nothingEnabled || inFlight
    val hasBuild = current != null && !current.rejected
    val compileLabel = if (hasBuild) "Recompile (applies after re-launch)" else "Compile"
    val disabledReason = when {
        native.mods.isEmpty() -> "Import a .hnmod bundle first."
        nothingEnabled -> "Enable at least one native mod to compile."
        inFlight -> "A build is already in progress."
        else -> null
    }

    Card(
        shape = RoundedCornerShape(24.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    "Native mods (.hnmod)",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                FilledTonalButton(onClick = actions.importNative) {
                    Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Import")
                }
            }
            current?.takeIf { it.rejected && !rejectedDismissed }?.let {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(
                        Icons.Rounded.Warning,
                        null,
                        tint = MaterialTheme.colorScheme.error,
                        modifier = Modifier.size(20.dp),
                    )
                    Spacer(Modifier.width(8.dp))
                    val reason = it.rejectReason?.takeIf { text -> text.isNotBlank() }
                    Text(
                        reason ?: "A modded build was set aside after crashing before its first frame. Playing the stock game. See docs/android-native-mods.md.",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = { rejectedDismissed = true }) {
                        Icon(Icons.Rounded.Clear, "Dismiss")
                    }
                }
            }
            if (native.mods.isEmpty()) {
                Text(
                    ".hnmod bundles are imported here. Import a bundle, enable it, then Compile: the mod is rebuilt into the game on a hosted runner and runs on the next start. See docs/android-native-mods.md.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                native.mods.forEach { mod ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(
                            Modifier
                                .weight(1f)
                                .combinedClickable(
                                    onClick = { },
                                    onLongClick = { detailsMod = mod },
                                ),
                        ) {
                            Text(
                                mod.name,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                fontWeight = FontWeight.Medium,
                            )
                            Text(
                                "${mod.version} · ${formatSize(mod.size)} · long-press for details",
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.outline,
                            )
                            Text(
                                stateText(mod),
                                style = MaterialTheme.typography.labelMedium,
                                color = if (mod.enabledState == "selected") {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    MaterialTheme.colorScheme.outline
                                },
                            )
                        }
                        HorizonSwitch(
                            checked = mod.enabled,
                            onCheckedChange = { on ->
                                act("native_enabled", JSONObject().put("id", mod.id).put("enabled", on).toString())
                            },
                        )
                        Spacer(Modifier.width(4.dp))
                        IconButton(onClick = { actions.removeNative(mod.id) }) {
                            Icon(Icons.Rounded.DeleteOutline, "Remove", tint = MaterialTheme.colorScheme.error)
                        }
                    }
                }
            }
            buildStatusRow(build, stock, versionCode, abi)
            if (build?.state == "ready") {
                OutlinedButton(onClick = {
                    val manager = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
                    manager?.killBackgroundProcesses(context.packageName)
                    restartNote = "If the game is running, it was closed. Press Play to start with your mods."
                }, modifier = Modifier.fillMaxWidth()) {
                    Text("Restart")
                }
            }
            if (hasBuild) {
                val chosen = current!!
                val summary = chosen.mods.takeIf { it.isNotEmpty() }?.joinToString(", ") ?: "${chosen.id} ${chosen.version}"
                Text(
                    "Native build active: $summary — applies on the next start after you close the game.",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (current != null) {
                val chosen = current
                Text(
                    if (chosen.rejected) {
                        "Set aside: ${chosen.id} ${chosen.version}"
                    } else {
                        "Runs on the next start: ${chosen.id} ${chosen.version}"
                    },
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                restartNote?.let { note ->
                    Text(
                        note,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
                OutlinedButton(onClick = actions.disableNative, modifier = Modifier.fillMaxWidth()) {
                    Text("Play without native mods")
                }
            }
            FilledTonalButton(
                onClick = { showCompileConfirm = true },
                enabled = !compileDisabled,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Icon(Icons.Rounded.Build, null, Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(compileLabel)
            }
            Text(
                disabledReason ?: "Native mods run with full access to this device. Compile only mods you trust.",
                style = MaterialTheme.typography.bodySmall,
                color = if (disabledReason != null) {
                    MaterialTheme.colorScheme.outline
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
            )
        }
    }

    if (showCompileConfirm) {
        AlertDialog(
            onDismissRequest = { showCompileConfirm = false },
            title = { Text("Compile native mods?") },
            text = {
                Text(
                    "Native mods run with full access to this device. Only compile mods you trust. The mod code is compiled on a GitHub-hosted runner; nothing is executed here. This may take a few minutes.",
                )
            },
            confirmButton = {
                Button(onClick = {
                    showCompileConfirm = false
                    val arg = JSONObject()
                        .put("enabled", JSONArray(enabledIds))
                        .put("versionCode", versionCode)
                        .put("abi", abi)
                        .put("stock", stock)
                    act("native_compile", arg.toString())
                }) { Text("Compile") }
            },
            dismissButton = { TextButton(onClick = { showCompileConfirm = false }) { Text("Cancel") } },
        )
    }
    detailsMod?.let { mod ->
        AlertDialog(
            onDismissRequest = { detailsMod = null },
            title = { Text("Native mod") },
            text = {
                Text(
                    "id: ${mod.id}\nversion: ${mod.version}\nfile: ${mod.id}.hnmod",
                )
            },
            confirmButton = { TextButton(onClick = { detailsMod = null }) { Text("Close") } },
        )
    }
}

private fun apkVersionCode(context: Context): Long = runCatching {
    val info = context.packageManager.getPackageInfo(context.packageName, 0)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        info.longVersionCode
    } else {
        @Suppress("DEPRECATION")
        info.versionCode.toLong()
    }
}.getOrDefault(0L)

/** The one-line inline progress row for the build pipeline. Renders nothing when idle. */
@Composable
private fun buildStatusRow(
    build: NativeBuild?,
    stock: String,
    versionCode: Long,
    abi: String,
) {
    val message: String? = when (build?.state) {
        "requesting" -> "Sending bundles to the build service…"
        "queued" -> build.ahead?.let { "Waiting for a free build slot ($it ahead)" }
            ?: "Waiting for a free build slot…"
        "building" -> build.seconds?.let { "Building… ${it}s (up to ~15 min)" } ?: "Building… (up to ~15 min)"
        "installing" -> {
            val done = build.downloaded / (1024.0 * 1024.0)
            val total = build.installTotal?.div(1024.0 * 1024.0)
            if (total != null) {
                "Downloading… %.1f / %.1f MiB".format(done, total)
            } else {
                "Downloading… %.1f MiB".format(done)
            }
        }
        "ready" -> "Ready — restart Horizon to use it."
        "failed" -> build.error ?: "Build failed"
        else -> null
    }
    if (message == null) return
    val retry = build?.state == "failed" && build.installFailed
    val cachedNote = if (build?.cached == true && build.state == "queued") " A cached build was found." else ""
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            message + cachedNote,
            style = MaterialTheme.typography.bodySmall,
            color = if (build?.state == "failed") MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        if (retry) {
            Spacer(Modifier.width(8.dp))
            OutlinedButton(onClick = {
                val arg = JSONObject()
                    .put("job", build?.job)
                    .put("stock", stock)
                    .put("versionCode", versionCode)
                    .put("abi", abi)
                act("native_install", arg.toString())
            }) { Text("Retry") }
        }
    }
}

private fun formatSize(bytes: Long): String = when {
    bytes >= 1024L * 1024L -> "%.1f MiB".format(bytes / (1024.0 * 1024.0))
    bytes >= 1024L -> "%.1f KiB".format(bytes / 1024.0)
    else -> "$bytes B"
}

private fun stateText(mod: NativeMod): String = when {
    !mod.enabled -> "Disabled — not part of Compile"
    mod.enabledState == "selected" -> "Selected — runs on the next start"
    mod.enabledState == "rejected" -> "Set aside after crashing twice"
    mod.enabledState == "invalid" -> "Invalid bundle"
    else -> "Imported"
}
