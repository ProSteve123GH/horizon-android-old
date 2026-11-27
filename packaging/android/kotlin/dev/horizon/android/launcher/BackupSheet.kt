package dev.horizon.android.launcher

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** Copies a locally created archive into a folder the user picked through SAF. */
private fun copyArchiveToTree(context: Context, tree: Uri, source: File): String? = try {
    val document = DocumentsContract.createDocument(
        context.contentResolver,
        tree,
        "application/gzip",
        source.name,
    ) ?: return "Could not create the archive in that folder"
    context.contentResolver.openOutputStream(document)?.use { output ->
        source.inputStream().use { input -> input.copyTo(output) }
    } ?: return "Could not write the archive to that folder"
    null
} catch (error: Exception) {
    error.message ?: "Could not save the archive"
}

/** Selective backup and restore of player files, driven by the Rust backup store. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BackupSheet(onDismiss: () -> Unit) {
    val context = LocalContext.current
    var selected by remember {
        mutableStateOf(setOf("instances", "controls", "launcher", "screenshots", "reports"))
    }
    var labels by remember { mutableStateOf<List<Pair<String, String>>>(emptyList()) }
    var archives by remember { mutableStateOf<List<String>>(emptyList()) }
    var pending by remember { mutableStateOf<File?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var status by remember { mutableStateOf<String?>(null) }
    var replace by remember { mutableStateOf(false) }
    var confirming by remember { mutableStateOf<String?>(null) }

    fun reload() {
        val reply = act("backup_list")
        archives = (reply.ok as? JSONArray)?.let { array ->
            List(array.length()) { index -> array.getString(index) }
        }.orEmpty()
    }
    LaunchedEffect(Unit) {
        val reply = act("backup_categories")
        labels = (reply.ok as? JSONArray)?.let { array ->
            List(array.length()) { index ->
                val item = array.getJSONObject(index)
                item.optString("id") to item.optString("label")
            }
        }.orEmpty()
        reload()
    }

    val saveDirectory = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        val archive = pending
        pending = null
        if (uri != null && archive != null) {
            error = copyArchiveToTree(context, uri, archive)
            if (error == null) status = "Backup copied to the chosen folder."
            reload()
        }
    }
    val restoreFile = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        try {
            val staged = File(context.cacheDir, "restore-${System.currentTimeMillis()}.tar.gz")
            context.contentResolver.openInputStream(uri)?.use { input ->
                staged.outputStream().use { input.copyTo(it) }
            } ?: throw IllegalStateException("Could not read the selected backup")
            val request = JSONObject()
                .put("path", staged.absolutePath)
                .put("replace", replace)
            val reply = act("backup_restore_file", request.toString())
            error = reply.error
            status = (reply.ok as? String) ?: "Restore finished."
            staged.delete()
            reload()
        } catch (caught: Exception) {
            error = caught.message ?: "Could not restore that backup"
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        LazyColumn(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            item(key = "header") {
                Column(Modifier.fillMaxWidth().padding(bottom = 8.dp)) {
                    Text(
                        "Backup & restore",
                        style = MaterialTheme.typography.headlineSmall,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        "Choose the player files to include. Archives are standard .tar.gz files.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            items(labels, key = { it.first }) { (id, label) ->
                Row(
                    Modifier.fillMaxWidth().clickable {
                        selected = if (id in selected) selected - id else selected + id
                    },
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Checkbox(checked = id in selected, onCheckedChange = { _ -> })
                    Text(label, style = MaterialTheme.typography.bodyLarge)
                }
            }
            item(key = "create") {
                Button(
                    onClick = {
                        error = null
                        status = null
                        val reply = act("backup_create", JSONObject(selected.associateWith { true }).toString())
                        error = reply.error
                        val path = reply.ok as? String
                        if (error == null && path != null) {
                            pending = File(path)
                            saveDirectory.launch(null)
                        }
                    },
                    modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
                ) {
                    Text("Create backup and choose a folder")
                }
            }
            item(key = "restore-controls") {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text("Overwrite existing files on restore", Modifier.weight(1f))
                    Switch(checked = replace, onCheckedChange = { replace = it })
                }
                OutlinedButton(
                    onClick = { restoreFile.launch(arrayOf("*/*")) },
                    modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
                ) {
                    Text("Restore from a .tar.gz file")
                }
            }
            items(archives, key = { it }) { name ->
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        name,
                        Modifier.weight(1f).padding(end = 8.dp),
                        style = MaterialTheme.typography.bodySmall,
                    )
                    TextButton(onClick = { confirming = name }) { Text("Restore") }
                }
            }
            item(key = "notices") {
                status?.let { Text(it, color = MaterialTheme.colorScheme.primary) }
                error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                Spacer(Modifier.padding(bottom = 16.dp))
            }
        }
    }

    confirming?.let { name ->
        AlertDialog(
            onDismissRequest = { confirming = null },
            title = { Text("Restore $name?") },
            text = {
                Text(
                    if (replace) {
                        "Existing files with the same names will be overwritten. This cannot be undone."
                    } else {
                        "Files that already exist are left untouched. Use the overwrite switch if you need to replace them."
                    },
                )
            },
            confirmButton = {
                Button(onClick = {
                    val reply = act("backup_restore", JSONObject().put("name", name).put("replace", replace).toString())
                    error = reply.error
                    status = (reply.ok as? String) ?: "Restore finished."
                    confirming = null
                    reload()
                }) { Text("Restore") }
            },
            dismissButton = { TextButton(onClick = { confirming = null }) { Text("Cancel") } },
        )
    }
}
