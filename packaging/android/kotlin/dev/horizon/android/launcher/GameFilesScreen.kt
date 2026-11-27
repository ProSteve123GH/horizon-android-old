package dev.horizon.android.launcher

import android.content.Context
import android.content.Intent
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.ContentCut
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.InsertDriveFile
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import java.io.File

private val TEXT_EXTENSIONS = setOf("txt", "json", "toml", "properties", "cfg", "yml", "yaml", "md", "xml", "ini", "log")
private val IMAGE_EXTENSIONS = setOf("png", "jpg", "jpeg", "webp", "gif", "bmp")
private val MEDIA_EXTENSIONS = setOf("mp4", "mkv", "webm", "mp3", "ogg", "wav", "m4a", "flac")

private fun extension(file: File): String = file.name.substringAfterLast('.', "").lowercase()

private fun copyOrMove(source: File, target: File, move: Boolean): String? {
    if (target.startsWith(source)) return "Cannot paste a folder into itself"
    if (target.exists()) return "A file with that name already exists here"
    return try {
        if (source.isDirectory) {
            target.mkdirs()
            source.listFiles()?.forEach { child ->
                val error = copyOrMove(child, File(target, child.name), move)
                if (error != null) return error
            }
            if (move) source.deleteRecursively()
        } else {
            source.copyTo(target, overwrite = false)
            if (move) source.delete()
        }
        null
    } catch (error: Exception) {
        error.message ?: "Could not paste that file"
    }
}

private fun textEditor(context: Context, file: File, onDone: (String?) -> Unit) {
    if (file.length() > 2_000_000L) {
        onDone("That text file is too large to edit here")
        return
    }
    onDone(null)
}

private fun openFile(context: Context, file: File): String? {
    val mime = when (extension(file)) {
        in IMAGE_EXTENSIONS -> "image/*"
        in MEDIA_EXTENSIONS -> "video/*"
        else -> return "This file type cannot be opened here"
    }
    return try {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, mime)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        context.startActivity(intent)
        null
    } catch (error: Exception) {
        error.message ?: "No app can open that file"
    }
}

/** A file manager over both the shared game root and the app-private home. */
@Composable
fun GameFilesScreen(root: File, onBack: () -> Unit) {
    val context = LocalContext.current
    val privateRoot = context.filesDir
    var usePrivate by rememberSaveable { mutableStateOf(false) }
    var stack by remember { mutableStateOf(listOf(root)) }
    var advanced by rememberSaveable { mutableStateOf(false) }
    var clipboard by remember { mutableStateOf<Pair<File, Boolean>?>(null) } // (file, cut)
    var menuFor by remember { mutableStateOf<File?>(null) }
    var renameFor by remember { mutableStateOf<File?>(null) }
    var newFolder by remember { mutableStateOf(false) }
    var deleteFor by remember { mutableStateOf<File?>(null) }
    var editFor by remember { mutableStateOf<File?>(null) }
    var editText by remember { mutableStateOf("") }
    var message by remember { mutableStateOf<String?>(null) }

    fun reset(newRoot: File) {
        stack = listOf(newRoot)
        clipboard = null
    }
    val activeRoot = if (usePrivate) privateRoot else root
    val current = stack.lastOrNull() ?: activeRoot

    fun navigateUp() {
        if (stack.size > 1) stack = stack.dropLast(1) else onBack()
    }
    BackHandler { navigateUp() }

    fun switchRoot(toPrivate: Boolean) {
        usePrivate = toPrivate
        reset(if (toPrivate) privateRoot else root)
    }
    fun paste() {
        val (source, cut) = clipboard ?: return
        message = copyOrMove(source, File(current, source.name), cut)
        clipboard = null
    }
    fun report(error: String?) {
        message = error
        if (error != null) Toast.makeText(context, error, Toast.LENGTH_SHORT).show()
    }

    Column(Modifier.fillMaxSize().padding(horizontal = 12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { navigateUp() }) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Up")
            }
            Text(
                "Game files",
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.weight(1f),
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = { switchRoot(false) }, enabled = !usePrivate) { Text("Internal storage") }
            OutlinedButton(onClick = { switchRoot(true) }, enabled = usePrivate) { Text("Private storage") }
        }
        Text(
            if (usePrivate) privateRoot.absolutePath else activeRoot.absolutePath,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { newFolder = true }) { Text("New folder") }
            TextButton(onClick = { paste() }, enabled = clipboard != null) { Text("Paste") }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Show hidden files", Modifier.weight(1f))
            Switch(checked = advanced, onCheckedChange = { advanced = it })
        }
        Text(
            "Breadcrumbs · ${current.absolutePath}",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(8.dp))
        HorizontalDivider()
        val children = remember(current, advanced) {
            current.listFiles()
                ?.filter { advanced || !it.name.startsWith('.') }
                ?.sortedWith(compareBy({ !it.isDirectory }, { it.name.lowercase() }))
                .orEmpty()
        }
        if (children.isEmpty()) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("This folder is empty", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            LazyColumn(Modifier.fillMaxWidth().weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                items(children, key = { it.absolutePath }) { entry ->
                    FileRow(
                        file = entry,
                        onOpen = {
                            when {
                                entry.isDirectory -> stack = stack + entry
                                extension(entry) in TEXT_EXTENSIONS -> {
                                    editFor = entry
                                    editText = entry.readText(Charsets.UTF_8)
                                }
                                else -> report(openFile(context, entry))
                            }
                        },
                        onMenu = { menuFor = entry },
                    )
                }
            }
        }
        message?.let {
            Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
    }

    menuFor?.let { file ->
        DropdownMenu(expanded = true, onDismissRequest = { menuFor = null }) {
            DropdownMenuItem(
                text = { Text("Copy") },
                leadingIcon = { Icon(Icons.Rounded.ContentCopy, null) },
                onClick = { clipboard = file to false; menuFor = null },
            )
            DropdownMenuItem(
                text = { Text("Cut") },
                leadingIcon = { Icon(Icons.Rounded.ContentCut, null) },
                onClick = { clipboard = file to true; menuFor = null },
            )
            if (file.isFile) {
                DropdownMenuItem(
                    text = { Text("Rename") },
                    leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                    onClick = { renameFor = file; menuFor = null },
                )
                if (extension(file) in TEXT_EXTENSIONS) {
                    DropdownMenuItem(
                        text = { Text("Edit text") },
                        onClick = {
                            editFor = file
                            editText = file.readText(Charsets.UTF_8)
                            menuFor = null
                        },
                    )
                }
            }
            DropdownMenuItem(
                text = { Text("Delete") },
                leadingIcon = { Icon(Icons.Rounded.DeleteOutline, null) },
                onClick = { deleteFor = file; menuFor = null },
            )
        }
    }

    renameFor?.let { file ->
        NamePrompt("Rename", file.name) { name ->
            if (!name.isNullOrBlank()) report(runCatching {
                val target = File(file.parentFile, name)
                if (target.exists()) "That name is already in use" else if (!file.renameTo(target)) "Could not rename" else null
            }.getOrElse { it.message })
            renameFor = null
        }
    }
    if (newFolder) {
        NamePrompt("New folder", "") { name ->
            if (!name.isNullOrBlank()) report(runCatching {
                val folder = File(current, name)
                if (!folder.mkdirs()) "Could not create that folder" else null
            }.getOrElse { it.message })
            newFolder = false
        }
    }
    deleteFor?.let { file ->
        AlertDialog(
            onDismissRequest = { deleteFor = null },
            title = { Text("Delete ${file.name}?") },
            text = { Text("This removes the file or folder from this phone. It cannot be undone.") },
            confirmButton = {
                Button(onClick = {
                    report(if (file.deleteRecursively()) null else "Could not delete ${file.name}")
                    deleteFor = null
                }) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { deleteFor = null }) { Text("Cancel") } },
        )
    }
    editFor?.let { file ->
        AlertDialog(
            onDismissRequest = { editFor = null },
            title = { Text("Edit ${file.name}") },
            text = {
                OutlinedTextField(
                    value = editText,
                    onValueChange = { editText = it },
                    modifier = Modifier.fillMaxWidth().heightIn(min = 180.dp),
                )
            },
            confirmButton = {
                Button(onClick = {
                    report(runCatching { file.writeText(editText); null }.getOrElse { it.message })
                    editFor = null
                }) { Text("Save") }
            },
            dismissButton = { TextButton(onClick = { editFor = null }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun NamePrompt(title: String, initial: String, onDone: (String?) -> Unit) {
    var value by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = { onDone(null) },
        title = { Text(title) },
        text = { OutlinedTextField(value = value, onValueChange = { value = it }, singleLine = true) },
        confirmButton = { Button(onClick = { onDone(value.trim()) }) { Text("Save") } },
        dismissButton = { TextButton(onClick = { onDone(null) }) { Text("Cancel") } },
    )
}

@Composable
private fun FileRow(file: File, onOpen: () -> Unit, onMenu: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(vertical = 8.dp, horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            if (file.isDirectory) Icons.Rounded.Folder else Icons.Rounded.InsertDriveFile,
            null,
            Modifier.size(24.dp),
            tint = MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.width(10.dp))
        Text(file.name, Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        IconButton(onClick = onMenu) { Icon(Icons.Rounded.MoreVert, "Actions for ${file.name}") }
    }
}
