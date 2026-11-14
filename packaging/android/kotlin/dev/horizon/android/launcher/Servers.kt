package dev.horizon.android.launcher

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.foundation.Image
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Dns
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material.icons.rounded.StarBorder
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.json.JSONObject

/** One saved server as the Rust Server Manager sees it. */
data class SavedServer(
    val index: Int,
    val name: String,
    val address: String,
    val displayAddress: String?,
    val icon: String?,
    val favorite: Boolean,
    val group: String,
)

data class ServerListState(
    val instance: String,
    val servers: List<SavedServer>,
    val groups: List<String>,
    val maxName: Int = 64,
    val maxGroup: Int = 32,
    val error: String? = null,
) {
    companion object {
        fun load(): ServerListState {
            val reply = act("servers_state")
            val root = reply.ok as? JSONObject
                ?: return ServerListState("", emptyList(), emptyList(), error = reply.error ?: "Could not read the server list.")
            val servers = root.optJSONArray("servers")?.let { array ->
                (0 until array.length()).mapNotNull { i ->
                    array.optJSONObject(i)?.let { item ->
                        SavedServer(
                            index = item.optInt("index"),
                            name = item.optString("name").ifBlank { "Minecraft Server" },
                            address = item.optString("address"),
                            displayAddress = item.stringOrNull("display_address"),
                            icon = item.stringOrNull("icon"),
                            favorite = item.optBoolean("favorite"),
                            group = item.optString("group"),
                        )
                    }
                }
            }.orEmpty()
            val groups = root.optJSONArray("groups")?.let { array ->
                (0 until array.length()).mapNotNull { i -> array.optString(i).takeIf { it.isNotBlank() } }
            }.orEmpty()
            return ServerListState(
                instance = root.optString("instance"),
                servers = servers,
                groups = groups,
                maxName = root.optInt("max_name", 64),
                maxGroup = root.optInt("max_group", 32),
                error = reply.error,
            )
        }
    }
}

private fun JSONObject.stringOrNull(key: String): String? =
    if (isNull(key)) null else optString(key).takeIf { it.isNotBlank() }

private sealed interface ServerFilter {
    data object All : ServerFilter
    data object Favorites : ServerFilter
    data class Group(val name: String) : ServerFilter
}

private sealed interface ServerStatus {
    data object Checking : ServerStatus
    data class Online(val players: Pair<Int, Int>?, val latencyMs: Long) : ServerStatus
    data class Offline(val error: String) : ServerStatus
}

private fun parseStatuses(reply: Reply): Map<String, ServerStatus> {
    val root = reply.ok as? JSONObject ?: return emptyMap()
    val statuses = root.optJSONObject("statuses") ?: return emptyMap()
    val parsed = mutableMapOf<String, ServerStatus>()
    val keys = statuses.keys()
    while (keys.hasNext()) {
        val address = keys.next()
        val item = statuses.optJSONObject(address) ?: continue
        parsed[address] = when (item.optString("state")) {
            "pending" -> ServerStatus.Checking
            "online" -> {
                val players = item.optJSONObject("players")?.let {
                    it.optInt("online") to it.optInt("max")
                }
                ServerStatus.Online(players, item.optLong("latency_ms"))
            }
            else -> ServerStatus.Offline(
                item.stringOrNull("error") ?: "No status reply.",
            )
        }
    }
    return parsed
}

private fun serverAddress(server: SavedServer): String =
    server.displayAddress?.takeIf { it.isNotBlank() } ?: server.address

@Composable
private fun ServerIcon(server: SavedServer, size: Int) {
    val bitmap = remember(server.icon) {
        server.icon?.let { encoded ->
            runCatching {
                val bytes = Base64.decode(encoded, Base64.DEFAULT)
                if (bytes.size > 64 * 1024) null else BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
            }.getOrNull()
        }
    }
    Surface(
        modifier = Modifier.size(size.dp),
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
    ) {
        if (bitmap != null) {
            Image(
                bitmap.asImageBitmap(),
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        } else {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                val letter = server.name.trim().firstOrNull()?.uppercaseChar()
                if (letter != null) {
                    Text(
                        letter.toString(),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                    )
                } else {
                    Icon(Icons.Rounded.Dns, null, tint = MaterialTheme.colorScheme.onSecondaryContainer)
                }
            }
        }
    }
}

@Composable
private fun statusLine(status: ServerStatus?): Pair<String, androidx.compose.ui.graphics.Color>? = when (status) {
    null -> null
    ServerStatus.Checking -> "Checking status…" to MaterialTheme.colorScheme.onSurfaceVariant
    is ServerStatus.Offline ->
        "Unreachable — ${status.error.removePrefix("No status reply: ")}" to MaterialTheme.colorScheme.error
    is ServerStatus.Online -> {
        val players = status.players?.let { (online, max) -> "$online / $max players" }
        listOfNotNull(players, if (status.latencyMs > 0) "${status.latencyMs} ms" else null)
            .joinToString("  ·  ")
            .ifBlank { "Online" } to MaterialTheme.colorScheme.primary
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ServersSheet(onDismiss: () -> Unit, onPlay: () -> Unit) {
    var state by remember { mutableStateOf(ServerListState.load()) }
    var filter by remember { mutableStateOf<ServerFilter>(ServerFilter.All) }
    var statuses by remember { mutableStateOf<Map<String, ServerStatus>>(emptyMap()) }
    var checking by remember { mutableStateOf(false) }
    var pingRound by remember { mutableStateOf(0) }
    var editor by remember { mutableStateOf<ServerEditor?>(null) }
    var deleting by remember { mutableStateOf<SavedServer?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }

    fun reload() {
        val next = ServerListState.load()
        state = next
        if (filter is ServerFilter.Group && (filter as ServerFilter.Group).name !in next.groups) {
            filter = ServerFilter.All
        }
    }

    fun apply(reply: Reply, success: String? = null) {
        if (reply.error != null) {
            error = reply.error
            notice = null
        } else {
            error = null
            notice = success
        }
        reload()
    }

    fun startPings(index: Int? = null) {
        val reply = act("server_ping", index?.toString() ?: "all")
        if (reply.error != null) {
            error = reply.error
            return
        }
        error = null
        notice = null
        checking = true
        pingRound += 1
    }

    fun launch(server: SavedServer) {
        if (server.displayAddress.isNullOrBlank()) {
            error = "That saved address is not valid. Edit or remove it before connecting."
            notice = null
            return
        }
        val reply = act("server_launch", server.index.toString())
        if (reply.error != null) {
            error = reply.error
            notice = null
            return
        }
        val started = (reply.ok as? JSONObject)?.optBoolean("launch") == true
        if (!started) {
            notice = "The game is already starting. Try again in a moment."
            return
        }
        onDismiss()
        onPlay()
    }

    LaunchedEffect(Unit) {
        val reply = act("server_status")
        if (reply.error == null) {
            statuses = parseStatuses(reply)
            // A check started before this sheet was closed may still be waiting for its reply.
            if (statuses.values.any { it is ServerStatus.Checking }) {
                checking = true
                pingRound += 1
            }
        }
    }

    LaunchedEffect(pingRound) {
        if (pingRound == 0) return@LaunchedEffect
        checking = true
        repeat(60) {
            val reply = act("server_status")
            if (reply.error != null) {
                error = reply.error
                checking = false
                return@LaunchedEffect
            }
            statuses = parseStatuses(reply)
            if (statuses.values.none { it is ServerStatus.Checking }) {
                checking = false
                return@LaunchedEffect
            }
            delay(400)
        }
        checking = false
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        val visible = state.servers.filter { server ->
            when (val current = filter) {
                ServerFilter.All -> true
                ServerFilter.Favorites -> server.favorite
                is ServerFilter.Group -> server.group == current.name
            }
        }
        LazyColumn(
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            item(key = "header") {
                Column(Modifier.fillMaxWidth().padding(start = 8.dp, bottom = 8.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(
                                "Servers",
                                style = MaterialTheme.typography.headlineSmall,
                                fontWeight = FontWeight.SemiBold,
                            )
                            Text(
                                if (state.instance.isBlank()) {
                                    "The game's Multiplayer list and this one are the same."
                                } else {
                                    "In ${state.instance}. Shared with the game's Multiplayer list."
                                },
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        IconButton(onClick = { error = null; notice = null; reload() }) {
                            Icon(Icons.Rounded.Refresh, "Refresh servers")
                        }
                        FilledTonalButton(
                            onClick = { editor = ServerEditor(null, "", "", "", false) },
                            contentPadding = PaddingValues(horizontal = 14.dp),
                        ) {
                            Icon(Icons.Rounded.Add, null, Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text("Add")
                        }
                    }
                    Row(
                        Modifier.fillMaxWidth().padding(top = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            "${state.servers.size} saved",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.weight(1f),
                        )
                        TextButton(
                            onClick = { startPings() },
                            enabled = state.servers.isNotEmpty() && !checking,
                        ) {
                            Icon(Icons.Rounded.Refresh, null, Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(if (checking) "Checking status…" else "Check status")
                        }
                    }
                    (error ?: state.error)?.let { message ->
                        Text(
                            message,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.error,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                    notice?.let { message ->
                        Text(
                            message,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                }
            }
            item(key = "filters") {
                Row(
                    Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(start = 8.dp, bottom = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    FilterChip(
                        selected = filter == ServerFilter.All,
                        onClick = { filter = ServerFilter.All },
                        label = { Text("All") },
                    )
                    FilterChip(
                        selected = filter == ServerFilter.Favorites,
                        onClick = { filter = ServerFilter.Favorites },
                        label = { Text("Favorites") },
                        leadingIcon = {
                            Icon(
                                if (filter == ServerFilter.Favorites) Icons.Rounded.Star else Icons.Rounded.StarBorder,
                                null,
                                Modifier.size(16.dp),
                            )
                        },
                    )
                    state.groups.forEach { group ->
                        FilterChip(
                            selected = filter == ServerFilter.Group(group),
                            onClick = { filter = ServerFilter.Group(group) },
                            label = { Text(group) },
                        )
                    }
                }
            }
            if (visible.isEmpty()) {
                item(key = "empty") {
                    Column(
                        Modifier.fillMaxWidth().padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Icon(
                            Icons.Rounded.Dns,
                            null,
                            Modifier.size(48.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            if (state.servers.isEmpty()) "No saved servers yet" else "Nothing in this view",
                            style = MaterialTheme.typography.titleMedium,
                        )
                        Text(
                            if (state.servers.isEmpty()) {
                                "Add a Java server to join it without retyping its address."
                            } else {
                                "Choose All or another group to see the rest."
                            },
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
            items(visible, key = { it.index }) { server ->
                val position = visible.indexOf(server)
                ServerRow(
                    server = server,
                    status = statuses[server.address],
                    shape = if (visible.size == 1) RoundedCornerShape(24.dp) else connectedShape(position, visible.size),
                    canMoveUp = filter == ServerFilter.All && server.index > 0,
                    canMoveDown = filter == ServerFilter.All && server.index < state.servers.lastIndex,
                    onLaunch = { launch(server) },
                    onFavorite = { apply(act("server_favorite", server.index.toString())) },
                    onCheck = { startPings(server.index) },
                    onEdit = { editor = serverEditor(server) },
                    onDuplicate = {
                        val reply = act("server_duplicate", server.index.toString())
                        apply(reply, "Server duplicated.")
                    },
                    onMoveUp = {
                        val arg = JSONObject().put("index", server.index).put("delta", -1).toString()
                        apply(act("server_move", arg))
                    },
                    onMoveDown = {
                        val arg = JSONObject().put("index", server.index).put("delta", 1).toString()
                        apply(act("server_move", arg))
                    },
                    onDelete = { deleting = server },
                )
            }
        }
    }

    editor?.let { target ->
        ServerEditDialog(
            target = target,
            maxName = state.maxName,
            maxGroup = state.maxGroup,
            onDismiss = { editor = null },
            onSaved = { name ->
                editor = null
                notice = if (target.index == null) "Added $name." else "Saved $name."
                error = null
                reload()
            },
        )
    }

    deleting?.let { server ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            icon = { Icon(Icons.Rounded.DeleteOutline, null) },
            title = { Text("Remove ${server.name}?") },
            text = {
                Text(
                    "This removes the entry from this device's server list, including its favorite mark and group. " +
                        "The server itself is not changed.",
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        val reply = act("server_remove", server.index.toString())
                        val message = reply.error
                        deleting = null
                        if (message != null) {
                            error = message
                            notice = null
                        } else {
                            error = null
                            notice = "Removed ${server.name}."
                        }
                        reload()
                    },
                ) {
                    Text("Remove", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text("Cancel") } },
        )
    }
}

private data class ServerEditor(
    val index: Int?,
    val name: String,
    val address: String,
    val group: String,
    val favorite: Boolean,
)

private fun serverEditor(server: SavedServer) = ServerEditor(
    index = server.index,
    name = server.name,
    address = server.address,
    group = server.group,
    favorite = server.favorite,
)

@Composable
private fun ServerRow(
    server: SavedServer,
    status: ServerStatus?,
    shape: RoundedCornerShape,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onLaunch: () -> Unit,
    onFavorite: () -> Unit,
    onCheck: () -> Unit,
    onEdit: () -> Unit,
    onDuplicate: () -> Unit,
    onMoveUp: () -> Unit,
    onMoveDown: () -> Unit,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    Surface(
        onClick = onLaunch,
        shape = shape,
        color = colors.surfaceContainer,
    ) {
        Row(
            Modifier.padding(start = 10.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ServerIcon(server, 48)
            Column(
                Modifier.weight(1f).padding(horizontal = 10.dp),
                verticalArrangement = Arrangement.spacedBy(1.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        server.name,
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                    if (server.favorite) {
                        Spacer(Modifier.width(4.dp))
                        Icon(
                            Icons.Rounded.Star,
                            "Favorite",
                            Modifier.size(14.dp),
                            tint = colors.primary,
                        )
                    }
                }
                Text(
                    serverAddress(server),
                    style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (server.group.isNotBlank()) {
                    Text(
                        server.group,
                        style = MaterialTheme.typography.labelSmall,
                        color = colors.primary,
                        maxLines = 1,
                    )
                }
                if (server.displayAddress.isNullOrBlank()) {
                    Text(
                        "Address is not valid. Edit or remove this entry.",
                        style = MaterialTheme.typography.labelSmall,
                        color = colors.error,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                } else {
                    statusLine(status)?.let { (text, color) ->
                        Text(
                            text,
                            style = MaterialTheme.typography.labelSmall,
                            color = color,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            IconButton(onClick = onFavorite) {
                Icon(
                    if (server.favorite) Icons.Rounded.Star else Icons.Rounded.StarBorder,
                    if (server.favorite) "Remove from favorites" else "Add to favorites",
                    tint = if (server.favorite) colors.primary else colors.onSurfaceVariant,
                )
            }
            Box {
                IconButton(onClick = { menu = true }) {
                    Icon(Icons.Rounded.MoreVert, "Server actions")
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text("Connect") },
                        leadingIcon = { Icon(Icons.Rounded.PlayArrow, null) },
                        onClick = { menu = false; onLaunch() },
                    )
                    DropdownMenuItem(
                        text = { Text("Check status") },
                        leadingIcon = { Icon(Icons.Rounded.Refresh, null) },
                        onClick = { menu = false; onCheck() },
                    )
                    DropdownMenuItem(
                        text = { Text("Edit") },
                        leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                        onClick = { menu = false; onEdit() },
                    )
                    DropdownMenuItem(
                        text = { Text("Duplicate") },
                        leadingIcon = { Icon(Icons.Rounded.ContentCopy, null) },
                        onClick = { menu = false; onDuplicate() },
                    )
                    DropdownMenuItem(
                        text = { Text("Move up") },
                        leadingIcon = { Icon(Icons.Rounded.ArrowUpward, null) },
                        enabled = canMoveUp,
                        onClick = { menu = false; onMoveUp() },
                    )
                    DropdownMenuItem(
                        text = { Text("Move down") },
                        leadingIcon = { Icon(Icons.Rounded.ArrowDownward, null) },
                        enabled = canMoveDown,
                        onClick = { menu = false; onMoveDown() },
                    )
                    DropdownMenuItem(
                        text = { Text("Remove", color = colors.error) },
                        leadingIcon = { Icon(Icons.Rounded.DeleteOutline, null, tint = colors.error) },
                        onClick = { menu = false; onDelete() },
                    )
                }
            }
        }
    }
}

@Composable
private fun ServerEditDialog(
    target: ServerEditor,
    maxName: Int,
    maxGroup: Int,
    onDismiss: () -> Unit,
    onSaved: (String) -> Unit,
) {
    var name by remember(target) { mutableStateOf(target.name) }
    var address by remember(target) { mutableStateOf(target.address) }
    var group by remember(target) { mutableStateOf(target.group) }
    var favorite by remember(target) { mutableStateOf(target.favorite) }
    var failure by remember(target) { mutableStateOf<String?>(null) }

    fun save() {
        val request = JSONObject()
            .put("name", name.trim())
            .put("address", address.trim())
            .put("group", group.trim())
            .put("favorite", favorite)
        if (target.index != null) request.put("index", target.index)
        val reply = act(if (target.index == null) "server_add" else "server_update", request.toString())
        if (reply.error != null) {
            failure = reply.error
        } else {
            onSaved(name.trim().ifBlank { "Minecraft Server" })
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Rounded.Dns, null) },
        title = { Text(if (target.index == null) "Add server" else "Edit server") },
        text = {
            Column(
                Modifier
                    .heightIn(max = 520.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    "This entry is shared with the game's Multiplayer screen. No account or password is stored.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                OutlinedTextField(
                    value = name,
                    onValueChange = {
                        if (it.length <= maxName) {
                            name = it
                            failure = null
                        }
                    },
                    label = { Text("Name") },
                    placeholder = { Text("My server") },
                    supportingText = { Text("${name.length}/$maxName") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = address,
                    onValueChange = {
                        if (it.length <= 255) {
                            address = it
                            failure = null
                        }
                    },
                    label = { Text("Address") },
                    placeholder = { Text("play.example.com or 203.0.113.4:25565") },
                    supportingText = { Text("Hostname, IPv4, or [IPv6], with an optional port. Default: 25565.") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        imeAction = ImeAction.Next,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = group,
                    onValueChange = {
                        if (it.length <= maxGroup) {
                            group = it
                            failure = null
                        }
                    },
                    label = { Text("Group (optional)") },
                    placeholder = { Text("Friends") },
                    supportingText = { Text("Groups and favorites stay on this device.") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Favorite", style = MaterialTheme.typography.bodyLarge)
                        Text(
                            "Show it in the Favorites view.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Switch(checked = favorite, onCheckedChange = { favorite = it })
                }
                failure?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        },
        confirmButton = {
            SaveButton(enabled = address.isNotBlank()) { save() }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
