package dev.horizon.android.launcher

import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.util.LruCache
import android.widget.Toast
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.BrokenImage
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.PhotoLibrary
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material.icons.rounded.Workspaces
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.text.DateFormat
import java.util.Date
import java.util.Locale

/** One image in the launcher gallery, identified by an opaque native gallery ID. */
data class ScreenshotItem(
    val id: String,
    val name: String,
    val profile: Int,
    val instance: String,
    val size: Long,
    val modified: Long,
    val width: Int?,
    val height: Int?,
    val mime: String,
)

data class ScreenshotGalleryState(val items: List<ScreenshotItem>, val error: String?)

enum class ScreenshotOrder(val value: String, val label: String) {
    Newest("newest", "Newest"),
    Oldest("oldest", "Oldest"),
    Name("name", "Name"),
}

object Screenshots {
    /**
     * Reads the list on the calling (background) thread. The native function
     * only scans directories and image headers, so no full screenshot is held
     * in memory while the grid loads.
     */
    fun load(order: ScreenshotOrder): ScreenshotGalleryState {
        val text = runCatching { NativeBridge.screenshots(order.value) }.getOrNull()
            ?: return ScreenshotGalleryState(emptyList(), "The screenshot gallery is unavailable.")
        val root = runCatching { JSONObject(text) }.getOrNull()
            ?: return ScreenshotGalleryState(emptyList(), "The screenshot gallery returned an invalid reply.")
        val items = root.optJSONArray("items")?.let { array ->
            List(array.length()) { index ->
                val item = array.getJSONObject(index)
                ScreenshotItem(
                    id = item.optString("id"),
                    name = item.optString("name"),
                    profile = item.optInt("profile"),
                    instance = item.optString("instance"),
                    size = item.optLong("size"),
                    modified = item.optLong("modified"),
                    width = if (item.isNull("width")) null else item.optInt("width"),
                    height = if (item.isNull("height")) null else item.optInt("height"),
                    mime = item.optString("mime", "image/*"),
                )
            }
        }.orEmpty()
        return ScreenshotGalleryState(
            items = items,
            error = root.optString("error").takeIf { it.isNotBlank() },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ScreenshotGallery(activeInstance: String) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var order by rememberSaveable { mutableStateOf(ScreenshotOrder.Newest) }
    var grouped by rememberSaveable { mutableStateOf(true) }
    var refresh by remember { mutableIntStateOf(0) }
    var loading by remember { mutableStateOf(true) }
    var state by remember { mutableStateOf<ScreenshotGalleryState?>(null) }
    var preview by remember { mutableStateOf<ScreenshotItem?>(null) }
    var deleting by remember { mutableStateOf<ScreenshotItem?>(null) }

    LaunchedEffect(order, refresh) {
        loading = true
        state = withContext(Dispatchers.IO) { Screenshots.load(order) }
        loading = false
    }

    val items = state?.items.orEmpty()
    val profiles = items.map { it.profile }.distinct()
    Column(Modifier.fillMaxSize()) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    when (items.size) {
                        0 -> "No screenshots"
                        1 -> "1 screenshot"
                        else -> "${items.size} screenshots"
                    },
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    if (profiles.size > 1) "${profiles.size} instances" else activeInstance,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (loading) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
            if (profiles.size > 1) {
                FilterChip(
                    selected = grouped,
                    onClick = { grouped = !grouped },
                    label = { Text("Group") },
                    leadingIcon = { Icon(Icons.Rounded.Workspaces, null, Modifier.size(18.dp)) },
                )
            }
            IconButton(onClick = { refresh++ }) {
                Icon(Icons.Rounded.Refresh, "Refresh screenshots")
            }
        }
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(top = 4.dp)) {
            ScreenshotOrder.entries.forEachIndexed { index, value ->
                SegmentedButton(
                    selected = order == value,
                    onClick = { order = value },
                    shape = SegmentedButtonDefaults.itemShape(index, ScreenshotOrder.entries.size),
                    label = { Text(value.label, maxLines = 1) },
                )
            }
        }
        Spacer(Modifier.height(8.dp))
        when {
            state?.error != null -> GalleryError(state!!.error.orEmpty()) { refresh++ }
            items.isEmpty() && loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            items.isEmpty() -> EmptyGallery(activeInstance)
            else -> ScreenshotGrid(
                screenshots = items,
                grouped = grouped && profiles.size > 1,
                onOpen = { preview = it },
                modifier = Modifier.fillMaxSize(),
            )
        }
    }

    preview?.let { item ->
        ScreenshotPreview(
            item = item,
            onDismiss = { preview = null },
            onShare = { shareScreenshot(context, item) },
            onDelete = { deleting = item },
        )
    }
    deleting?.let { item ->
        DeleteScreenshotDialog(
            item = item,
            onDismiss = { deleting = null },
            onConfirm = {
                deleting = null
                scope.launch {
                    val reply = act("screenshot_delete", item.id)
                    if (reply.error != null) {
                        refresh++
                    } else {
                        preview = null
                        refresh++
                    }
                }
            },
        )
    }
}

@Composable
private fun GalleryError(message: String, retry: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.errorContainer,
        contentColor = MaterialTheme.colorScheme.onErrorContainer,
        shape = RoundedCornerShape(20.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(message, style = MaterialTheme.typography.bodyMedium)
            OutlinedButton(onClick = retry) { Text("Try again") }
        }
    }
}

@Composable
private fun EmptyGallery(activeInstance: String) {
    Column(
        Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            Icons.Rounded.PhotoLibrary,
            null,
            Modifier.size(56.dp),
            tint = MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.height(16.dp))
        Text(
            "No screenshots yet",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(8.dp))
        Text(
            "Press F2 while playing to save a screenshot. It stays in the active instance and appears here automatically.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(8.dp))
        Text(
            activeInstance,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.primary,
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ScreenshotGrid(
    screenshots: List<ScreenshotItem>,
    grouped: Boolean,
    onOpen: (ScreenshotItem) -> Unit,
    modifier: Modifier = Modifier,
) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(132.dp),
        modifier = modifier,
        contentPadding = PaddingValues(bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        if (grouped) {
            screenshots.groupBy { it.profile }.values.forEach { group ->
                val instance = group.first().instance
                item(
                    key = "group-${group.first().profile}",
                    span = { GridItemSpan(maxLineSpan) },
                ) {
                    Text(
                        "$instance (${group.size})",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 4.dp),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                items(group, key = { it.id }) { item ->
                    ScreenshotTile(item, showInstance = false, onClick = { onOpen(item) })
                }
            }
        } else {
            items(screenshots, key = { it.id }) { item ->
                ScreenshotTile(item, showInstance = true, onClick = { onOpen(item) })
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ScreenshotTile(
    item: ScreenshotItem,
    showInstance: Boolean,
    onClick: () -> Unit,
) {
    Card(
        onClick = onClick,
        shape = RoundedCornerShape(18.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Box(Modifier.fillMaxWidth().aspectRatio(1.5f)) {
            ScreenshotThumbnail(item, Modifier.fillMaxSize())
            Surface(
                color = Color.Black.copy(alpha = 0.58f),
                contentColor = Color.White,
                modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth(),
            ) {
                Column(Modifier.padding(horizontal = 8.dp, vertical = 6.dp)) {
                    Text(
                        item.name,
                        style = MaterialTheme.typography.labelMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    val detail = listOfNotNull(
                        if (showInstance) item.instance else null,
                        item.width?.let { width -> item.height?.let { height -> "$width × $height" } },
                    ).joinToString(" · ")
                    if (detail.isNotBlank()) {
                        Text(
                            detail,
                            style = MaterialTheme.typography.labelSmall,
                            color = Color.White.copy(alpha = 0.8f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ScreenshotThumbnail(item: ScreenshotItem, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val uri = remember(item.id) { screenshotUri(context, item.id) }
    val key = remember(item.id, item.modified, item.size) {
        "${item.id}:${item.modified}:${item.size}"
    }
    var failed by remember(key) { mutableStateOf(false) }
    val image by produceState<ImageBitmap?>(null, key) {
        val cached = ScreenshotThumbs.get(key)
        if (cached != null) {
            value = cached.asImageBitmap()
            return@produceState
        }
        val bitmap = withContext(Dispatchers.IO) {
            decodeSampledBitmap(context, uri, THUMBNAIL_PX)?.also { ScreenshotThumbs.put(key, it) }
        }
        if (bitmap == null) failed = true else value = bitmap.asImageBitmap()
    }
    Box(modifier.background(MaterialTheme.colorScheme.surfaceContainerHighest), contentAlignment = Alignment.Center) {
        if (image != null) {
            Image(
                bitmap = image!!,
                contentDescription = item.name,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize(),
            )
        } else if (failed) {
            Icon(
                Icons.Rounded.BrokenImage,
                "Preview unavailable",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp)
        }
    }
}

@Composable
private fun ScreenshotPreview(
    item: ScreenshotItem,
    onDismiss: () -> Unit,
    onShare: () -> Unit,
    onDelete: () -> Unit,
) {
    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Surface(Modifier.fillMaxSize(), color = Color.Black) {
            Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
                PreviewImage(item)
                Row(
                    Modifier.fillMaxWidth().align(Alignment.TopCenter).padding(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    IconButton(
                        onClick = onDismiss,
                        colors = IconButtonDefaults.iconButtonColors(contentColor = Color.White),
                    ) {
                        Icon(Icons.Rounded.Close, "Close preview")
                    }
                    Text(
                        item.name,
                        color = Color.White,
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.weight(1f).padding(horizontal = 4.dp),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    IconButton(
                        onClick = onShare,
                        colors = IconButtonDefaults.iconButtonColors(contentColor = Color.White),
                    ) {
                        Icon(Icons.Rounded.Share, "Share screenshot")
                    }
                    IconButton(
                        onClick = onDelete,
                        colors = IconButtonDefaults.iconButtonColors(contentColor = Color.White),
                    ) {
                        Icon(Icons.Rounded.DeleteOutline, "Delete screenshot")
                    }
                }
                Surface(
                    color = Color.Black.copy(alpha = 0.62f),
                    contentColor = Color.White,
                    modifier = Modifier.fillMaxWidth().align(Alignment.BottomCenter),
                ) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Text(
                            item.name,
                            style = MaterialTheme.typography.titleSmall,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            listOfNotNull(
                                item.width?.let { width -> item.height?.let { height -> "$width × $height" } },
                                formatScreenshotSize(item.size),
                                formatScreenshotDate(item.modified),
                                item.instance,
                            ).joinToString(" · "),
                            style = MaterialTheme.typography.bodySmall,
                            color = Color.White.copy(alpha = 0.85f),
                        )
                        Text(
                            "Pinch to zoom · drag to pan",
                            style = MaterialTheme.typography.labelSmall,
                            color = Color.White.copy(alpha = 0.65f),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun PreviewImage(item: ScreenshotItem) {
    val context = LocalContext.current
    val uri = remember(item.id) { screenshotUri(context, item.id) }
    var bitmap by remember(item.id) { mutableStateOf<ImageBitmap?>(null) }
    var failed by remember(item.id) { mutableStateOf(false) }
    var scale by remember(item.id) { mutableFloatStateOf(1f) }
    var offset by remember(item.id) { mutableStateOf(Offset.Zero) }
    var viewport by remember(item.id) { mutableStateOf(IntSize.Zero) }

    LaunchedEffect(item.id) {
        val decoded = withContext(Dispatchers.IO) { decodeSampledBitmap(context, uri, PREVIEW_PX) }
        if (decoded == null) failed = true else bitmap = decoded.asImageBitmap()
    }

    Box(
        Modifier.fillMaxSize()
            .onSizeChanged { viewport = it }
            .pointerInput(item.id) {
                detectTransformGestures { centroid, pan, zoom, _ ->
                    val previous = scale
                    val next = (previous * zoom).coerceIn(1f, 5f)
                    if (next == 1f) {
                        offset = Offset.Zero
                    } else {
                        val center = Offset(viewport.width / 2f, viewport.height / 2f)
                        val focus = centroid - center
                        val ratio = next / previous
                        val moved = offset * ratio + focus * (1f - ratio) + pan
                        val maxX = viewport.width * (next - 1f) / 2f
                        val maxY = viewport.height * (next - 1f) / 2f
                        offset = Offset(
                            moved.x.coerceIn(-maxX, maxX),
                            moved.y.coerceIn(-maxY, maxY),
                        )
                    }
                    scale = next
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        when {
            bitmap != null -> Image(
                bitmap = bitmap!!,
                contentDescription = item.name,
                contentScale = ContentScale.Fit,
                modifier = Modifier.fillMaxSize().graphicsLayer {
                    scaleX = scale
                    scaleY = scale
                    translationX = offset.x
                    translationY = offset.y
                },
            )
            failed -> Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Icon(
                    Icons.Rounded.BrokenImage,
                    null,
                    Modifier.size(48.dp),
                    tint = Color.White.copy(alpha = 0.7f),
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    "This screenshot is no longer available.",
                    color = Color.White.copy(alpha = 0.8f),
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
            else -> CircularProgressIndicator(color = Color.White)
        }
    }
}

@Composable
private fun DeleteScreenshotDialog(
    item: ScreenshotItem,
    onDismiss: () -> Unit,
    onConfirm: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Rounded.DeleteOutline, null) },
        title = { Text("Delete screenshot?") },
        text = {
            Text(
                "\"${item.name}\" will be removed from this device. This cannot be undone. " +
                    "A copy already shared or exported to Pictures is not affected.",
            )
        },
        confirmButton = {
            TextButton(
                onClick = onConfirm,
                colors = ButtonDefaults.textButtonColors(
                    contentColor = MaterialTheme.colorScheme.error,
                ),
            ) { Text("Delete") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

private fun screenshotUri(context: Context, id: String): Uri {
    val separator = id.indexOf('/')
    val profile = if (separator < 0) id else id.substring(0, separator)
    val name = if (separator < 0) "" else id.substring(separator + 1)
    return Uri.Builder()
        .scheme("content")
        .authority("${context.packageName}.screenshots")
        .appendPath(profile)
        .appendPath(name)
        .build()
}

/** The Android share sheet receives a scoped content URI and a temporary read grant. */
private fun shareScreenshot(context: Context, item: ScreenshotItem) {
    val uri = screenshotUri(context, item.id)
    val send = Intent(Intent.ACTION_SEND).apply {
        type = item.mime
        putExtra(Intent.EXTRA_STREAM, uri)
        clipData = ClipData.newRawUri(item.name, uri)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    runCatching { context.startActivity(Intent.createChooser(send, "Share screenshot")) }
        .onFailure {
            Toast.makeText(context, "No app can share this screenshot.", Toast.LENGTH_SHORT).show()
        }
}

private const val THUMBNAIL_PX = 512
private const val PREVIEW_PX = 2048

private object ScreenshotThumbs {
    private val cache = LruCache<String, Bitmap>(
        ((Runtime.getRuntime().maxMemory() / 8) / 1024).coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
    ) {
        override fun sizeOf(key: String, value: Bitmap): Int = value.byteCount / 1024
    }

    fun get(key: String): Bitmap? = cache.get(key)

    fun put(key: String, bitmap: Bitmap) {
        cache.put(key, bitmap)
    }
}

private fun decodeSampledBitmap(context: Context, uri: Uri, maxDimension: Int): Bitmap? = runCatching {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    context.contentResolver.openInputStream(uri)?.use {
        BitmapFactory.decodeStream(it, null, bounds)
    }
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return@runCatching null
    var sample = 1
    while (maxOf(bounds.outWidth / sample, bounds.outHeight / sample) > maxDimension && sample < 32) {
        sample *= 2
    }
    val options = BitmapFactory.Options().apply {
        inSampleSize = sample
        inPreferredConfig = Bitmap.Config.ARGB_8888
    }
    context.contentResolver.openInputStream(uri)?.use {
        BitmapFactory.decodeStream(it, null, options)
    }
}.getOrNull()

private fun formatScreenshotSize(bytes: Long): String = when {
    bytes >= 1L shl 30 -> String.format(Locale.US, "%.1f GiB", bytes.toDouble() / (1L shl 30))
    bytes >= 1L shl 20 -> String.format(Locale.US, "%.1f MiB", bytes.toDouble() / (1L shl 20))
    bytes >= 1024 -> String.format(Locale.US, "%.0f KiB", bytes.toDouble() / 1024.0)
    else -> "$bytes B"
}

private fun formatScreenshotDate(millis: Long): String =
    if (millis <= 0) "Unknown date"
    else DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT)
        .format(Date(millis))
