package dev.horizon.android.launcher

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.lazy.staggeredgrid.LazyVerticalStaggeredGrid
import androidx.compose.foundation.lazy.staggeredgrid.StaggeredGridCells
import androidx.compose.foundation.lazy.staggeredgrid.StaggeredGridItemSpan
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AddPhotoAlternate
import androidx.compose.material.icons.rounded.CloudDownload
import androidx.compose.material.icons.rounded.CloudUpload
import androidx.compose.material.icons.rounded.Face
import androidx.compose.material.icons.rounded.HideImage
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.RotateLeft
import androidx.compose.material.icons.rounded.RotateRight
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material.icons.rounded.ZoomIn
import androidx.compose.material.icons.rounded.ZoomOut
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

@Composable
fun SkinsScreen(
    state: Snapshot,
    wide: Boolean,
    short: Boolean,
    actions: LauncherActions,
) {
    val wardrobe = state.wardrobe
    if (wardrobe == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("Loading the local wardrobe…", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        return
    }
    var tab by rememberSaveable { mutableStateOf(if (wardrobe.capes) "cape" else "skin") }
    LaunchedEffect(tab) { act("wardrobe_kind", tab) }
    LazyVerticalStaggeredGrid(
        columns = StaggeredGridCells.Fixed(if (wide) 2 else 1),
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalItemSpacing = 12.dp,
    ) {
        item(span = StaggeredGridItemSpan.FullLine) {
            PreviewCard(wardrobe, short)
        }
        item {
            LocalGallery(wardrobe, tab, wide, actions) { tab = it }
        }
        item {
            CloudSection(wardrobe, tab)
        }
        item(span = StaggeredGridItemSpan.FullLine) {
            NoteCard(wardrobe)
        }
    }
}

@Composable
private fun PreviewCard(wardrobe: WardrobeState, short: Boolean) {
    val bitmap = remember(wardrobe.preview, wardrobe.previewStamp) {
        wardrobe.preview?.let { path ->
            runCatching { BitmapFactory.decodeFile(path) }.getOrNull()
        }
    }
    Surface(
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(
            Modifier.padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (bitmap != null) {
                Image(
                    bitmap.asImageBitmap(),
                    contentDescription = "Skin and cape preview",
                    contentScale = ContentScale.Fit,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(if (short) 180.dp else 260.dp)
                        .clip(RoundedCornerShape(18.dp)),
                )
            } else {
                Box(
                    Modifier.fillMaxWidth().height(180.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        Icons.Rounded.Face,
                        contentDescription = null,
                        modifier = Modifier.size(72.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                SmallIconButton(Icons.Rounded.RotateLeft, "Rotate left") {
                    act("wardrobe_rotate", "-1")
                }
                SmallIconButton(Icons.Rounded.RotateRight, "Rotate right") {
                    act("wardrobe_rotate", "1")
                }
                SmallIconButton(Icons.Rounded.ZoomOut, "Zoom out") {
                    act("wardrobe_zoom", "out")
                }
                SmallIconButton(Icons.Rounded.ZoomIn, "Zoom in") {
                    act("wardrobe_zoom", "in")
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilledTonalButton(onClick = { act("wardrobe_model") }) {
                    Text(if (wardrobe.model == "slim") "Alex / Slim" else "Steve / Classic")
                }
                OutlinedButton(onClick = { act("wardrobe_hide_cape") }) {
                    Icon(
                        if (wardrobe.hideCape) Icons.Rounded.VisibilityOff else Icons.Rounded.Visibility,
                        contentDescription = null,
                        modifier = Modifier.size(18.dp),
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(if (wardrobe.hideCape) "Show cape" else "Hide cape")
                }
            }
        }
    }
}

@Composable
private fun SmallIconButton(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    description: String,
    onClick: () -> Unit,
) {
    FilledTonalButton(
        onClick = onClick,
        contentPadding = PaddingValues(0.dp),
        modifier = Modifier.size(40.dp),
    ) {
        Icon(icon, contentDescription = description, modifier = Modifier.size(18.dp))
    }
}

@Composable
private fun LocalGallery(
    wardrobe: WardrobeState,
    tab: String,
    wide: Boolean,
    actions: LauncherActions,
    onTab: (String) -> Unit,
) {
    val entries = wardrobe.entries.filter { it.kind == tab }
    Surface(
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterButton("Skins", tab == "skin") { onTab("skin") }
                FilterButton("Capes", tab == "cape") { onTab("cape") }
            }
            Text(
                if (entries.isEmpty()) {
                    "No local ${if (tab == "cape") "capes" else "skins"} yet."
                } else {
                    "Tap one to apply it locally. Cloud appearance stays unchanged."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            entries.chunked(if (wide) 3 else 2).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
                    row.forEach { entry ->
                        EntryCard(entry, Modifier.weight(1f))
                    }
                    repeat((if (wide) 3 else 2) - row.size) {
                        Spacer(Modifier.weight(1f))
                    }
                }
            }
            FilledTonalButton(
                onClick = { actions.importWardrobe(tab) },
                enabled = !wardrobe.damaged,
                modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
            ) {
                Icon(Icons.Rounded.AddPhotoAlternate, null, Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(if (tab == "cape") "Import cape PNG" else "Import skin PNG")
            }
        }
    }
}

@Composable
private fun FilterButton(label: String, selected: Boolean, onClick: () -> Unit) {
    if (selected) {
        Button(onClick = onClick) { Text(label) }
    } else {
        OutlinedButton(onClick = onClick) { Text(label) }
    }
}

@Composable
private fun EntryCard(entry: WardrobeEntry, modifier: Modifier = Modifier) {
    val bitmap = remember(entry.image) {
        runCatching { BitmapFactory.decodeFile(entry.image) }.getOrNull()
    }
    val border = if (entry.selected) {
        Modifier.border(2.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(18.dp))
    } else {
        Modifier
    }
    Surface(
        onClick = { act("wardrobe_select", entry.index.toString()) },
        shape = RoundedCornerShape(18.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = modifier.then(border),
    ) {
        Column(
            Modifier.padding(8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            if (bitmap != null) {
                Image(
                    bitmap.asImageBitmap(),
                    contentDescription = entry.title,
                    contentScale = ContentScale.Fit,
                    modifier = Modifier.size(64.dp).clip(RoundedCornerShape(10.dp)),
                )
            } else {
                Icon(
                    Icons.Rounded.Face,
                    contentDescription = null,
                    modifier = Modifier.size(64.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Text(
                entry.title,
                style = MaterialTheme.typography.labelMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                fontWeight = if (entry.selected) FontWeight.Bold else FontWeight.Normal,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun CloudSection(wardrobe: WardrobeState, tab: String) {
    Surface(
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Cloud appearance", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
            Text(
                when (wardrobe.provider) {
                    "microsoft" -> "Signed in with Microsoft. Cloud changes affect your Minecraft profile."
                    "ely" -> "Signed in with Ely.by. Skins are managed on ely.by; capes can be applied locally."
                    else -> "Offline appearance. Everything here stays on this device."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (wardrobe.busy) {
                LinearWavyProgressIndicator(Modifier.fillMaxWidth())
            }
            if (wardrobe.provider == null) {
                OutlinedButton(
                    onClick = { act("wardrobe_use_cloud") },
                    enabled = !wardrobe.damaged,
                    modifier = Modifier.fillMaxWidth(),
                ) { Text("Reset to the default appearance") }
            }
            if (wardrobe.provider != null) {
                FilledTonalButton(onClick = { act("wardrobe_use_cloud") }, modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Rounded.CloudDownload, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Use current cloud appearance")
                }
                OutlinedButton(onClick = { act("wardrobe_cloud") }, modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Rounded.Refresh, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Refresh cloud profile")
                }
                if (tab == "skin") {
                    OutlinedButton(onClick = { act("wardrobe_upload") }, modifier = Modifier.fillMaxWidth()) {
                        Icon(Icons.Rounded.CloudUpload, null, Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text(
                            if (wardrobe.provider == "microsoft") "Upload selected skin to Microsoft"
                            else "Manage Ely.by skin on the website",
                        )
                    }
                }
            }
            if (tab == "cape" && wardrobe.cloudCapes.isNotEmpty()) {
                Text("Cloud capes", style = MaterialTheme.typography.labelLarge)
                wardrobe.cloudCapes.forEach { cape ->
                    Surface(
                        onClick = { act("wardrobe_cape", cape.index.toString()) },
                        shape = RoundedCornerShape(14.dp),
                        color = if (cape.active) MaterialTheme.colorScheme.primaryContainer
                        else MaterialTheme.colorScheme.surfaceContainerHigh,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(
                            cape.title + (if (cape.active) " · active" else " · activate"),
                            modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            if (tab == "skin" && wardrobe.cloudSkins.isNotEmpty()) {
                Text("Cloud skins", style = MaterialTheme.typography.labelLarge)
                wardrobe.cloudSkins.forEach { skin ->
                    Text(
                        skin.title + (if (skin.active) " · active" else ""),
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (skin.active) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun NoteCard(wardrobe: WardrobeState) {
    val note = when {
        wardrobe.damaged -> "The wardrobe index is damaged. Remove or repair it before adding images."
        wardrobe.note.isNotBlank() -> wardrobe.note
        else -> "Local choices change your view. Cloud buttons update your account."
    }
    Text(
        note,
        style = MaterialTheme.typography.bodySmall,
        color = if (wardrobe.damaged) MaterialTheme.colorScheme.error
        else MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 8.dp),
    )
}
