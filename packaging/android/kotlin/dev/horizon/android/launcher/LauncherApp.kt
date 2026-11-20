package dev.horizon.android.launcher

import android.content.Intent
import android.net.Uri
import android.os.SystemClock
import android.provider.OpenableColumns
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.Face
import androidx.compose.material.icons.rounded.Inventory2
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PhotoLibrary
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.animation.Crossfade
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.toShape
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.foundation.Canvas
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.horizon.android.NativeBridge
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject

enum class Destination(val id: String, val label: String, val icon: ImageVector, val inNavigation: Boolean = true) {
    Play("play", "Play", Icons.Rounded.PlayArrow),
    Skins("skins", "Skins", Icons.Rounded.Face),
    Mods("mods", "Mods", Icons.Rounded.Extension),
    Packs("packs", "Packs", Icons.Rounded.Palette),
    Shaders("shaders", "Shaders", Icons.Rounded.AutoAwesome),
    Installed("installed", "Installed", Icons.Rounded.Inventory2),
    // Opened from Play, Settings or the gallery itself, not from the navigation.
    Screenshots("screenshots", "Screenshots", Icons.Rounded.PhotoLibrary, inNavigation = false),
    // Opened from the gear beside the account chip on every page, not from the navigation.
    Settings("settings", "Settings", Icons.Rounded.Settings, inNavigation = false),
}

/** Runs a launcher action; failures also reach the status line through the bridge. */
fun act(method: String, arg: String = ""): Reply = Reply.parse(NativeBridge.call(method, arg))

/** The open dialog or sheet, if any. */
sealed interface Overlay {
    data object Account : Overlay
    data object Profiles : Overlay
    data object Worlds : Overlay
    data object Performance : Overlay
    data object Servers : Overlay
    data object TouchLayouts : Overlay
    data object Appearance : Overlay
    data class Choose(val id: String, val options: Options) : Overlay
    data class Edit(val id: String, val title: String, val value: String, val hint: String) : Overlay
    data class Remove(val item: LocalItem) : Overlay
    data class RemoveNative(val id: String) : Overlay
    data object RemoveInstallation : Overlay
}

@Composable
fun LauncherApp(
    onPlay: () -> Unit,
    onImport: (String, String?) -> Unit,
    onImportNative: (String?) -> Unit,
    crashNotice: String? = null,
) {
    var appearance by remember { mutableStateOf(LauncherAppearance.load()) }
    val reducedMotion = rememberReducedMotion()

    // ---- Snapshot loading state machine --------------------------------
    // Poll the native launcher off the UI loop only while it is on screen. Both the
    // bridge call and the parser are guarded: a missing key used to kill the coroutine
    // and leave a permanent spinner. After a short run of failures, or three seconds
    // without a snapshot, we surface a retryable failure and keep navigation mounted.
    var loadState by remember { mutableStateOf<SnapshotState>(SnapshotState.Loading) }
    var retryTick by remember { mutableStateOf(0) }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(lifecycle, retryTick) {
        var ready = false
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            var failures = 0
            val deadline = SystemClock.elapsedRealtime() + 3000L
            while (true) {
                val parsed = runCatching {
                    NativeBridge.snapshot()?.let { Snapshot.parse(it) }
                }.getOrNull()
                if (parsed != null) {
                    ready = true
                    failures = 0
                    loadState = SnapshotState.Ready(parsed)
                } else {
                    failures++
                    if (!ready && (failures >= 10 || SystemClock.elapsedRealtime() >= deadline)) {
                        loadState = SnapshotState.Failed(
                            "The launcher could not load its data. Check the installation and try again.",
                        )
                    }
                }
                delay(200)
            }
        }
    }
    val retry = {
        loadState = SnapshotState.Loading
        retryTick++
        Unit
    }

    // Fade and grow in over the splash window so opening the app is a transition, not a cut.
    val appear = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        appear.animateTo(1f, tween(durationMillis = if (reducedMotion) 0 else 450, easing = FastOutSlowInEasing))
    }
    HorizonTheme(appearance) {
        Box(Modifier.fillMaxSize()) {
            Box(
                Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        alpha = appear.value
                        scaleX = 0.97f + 0.03f * appear.value
                        scaleY = 0.97f + 0.03f * appear.value
                    },
            ) {
                LauncherContent(
                    loadState = loadState,
                    onRetry = retry,
                    crashNotice = crashNotice,
                    appearance = appearance,
                    onAppearance = { appearance = it },
                    onPlay = onPlay,
                    onImport = onImport,
                    onImportNative = onImportNative,
                )
            }
            LaunchSplash(visible = loadState is SnapshotState.Loading)
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun LauncherContent(
    loadState: SnapshotState,
    onRetry: () -> Unit,
    crashNotice: String?,
    appearance: LauncherAppearance,
    onAppearance: (LauncherAppearance) -> Unit,
    onPlay: () -> Unit,
    onImport: (String, String?) -> Unit,
    onImportNative: (String?) -> Unit,
) {
    val state = (loadState as? SnapshotState.Ready)?.snapshot
    val failure = (loadState as? SnapshotState.Failed)?.message
    LaunchedEffect(state?.appearance) {
        state?.appearance?.let { if (it != appearance) onAppearance(it) }
    }
    var destination by rememberSaveable { mutableStateOf(Destination.Play) }
    // Settings is a layer over the current page, not a destination: closing it reveals that page as it was.
    var settingsOpen by rememberSaveable { mutableStateOf(false) }
    var storagePromptDismissed by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(destination, settingsOpen) {
        // The gallery is a launcher view, not one of the catalog pages.
        if (!settingsOpen && destination == Destination.Screenshots) return@LaunchedEffect
        act("page", if (settingsOpen) Destination.Settings.id else destination.id)
    }
    var overlay by remember { mutableStateOf<Overlay?>(null) }
    var importKind by rememberSaveable { mutableStateOf("mods") }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var backgroundError by remember { mutableStateOf<String?>(null) }
    var importingBackground by remember { mutableStateOf(false) }
    val backgroundPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            importingBackground = true
            scope.launch {
                backgroundError = null
                try {
                    if (importLauncherBackground(context, uri).isFailure) {
                        backgroundError = "That file could not be used. Choose a PNG, JPEG or WebP image."
                    } else {
                        val reply = act("appearance_background")
                        val value = reply.ok as? JSONObject
                        if (reply.error != null || value == null) {
                            backgroundError = reply.error ?: "The launcher did not reply."
                        } else {
                            onAppearance(LauncherAppearance.parse(value))
                        }
                    }
                } finally {
                    importingBackground = false
                }
            }
        }
    }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        onImport(importKind, uri?.toString())
    }
    val nativePicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        onImportNative(uri?.toString())
    }
    var wardrobeKind by rememberSaveable { mutableStateOf("skin") }
    val wardrobePicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            val bytes = runCatching {
                context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            }.getOrNull()
            if (bytes != null) {
                val title = runCatching {
                    context.contentResolver.query(
                        uri,
                        arrayOf(OpenableColumns.DISPLAY_NAME),
                        null,
                        null,
                        null,
                    )?.use { cursor ->
                        if (cursor.moveToFirst()) cursor.getString(0) else null
                    }
                }.getOrNull() ?: "Imported ${if (wardrobeKind == "cape") "cape" else "skin"}"
                NativeBridge.importWardrobe(wardrobeKind, title, bytes)
            }
        }
    }
    val persist = { uri: Uri ->
        runCatching {
            context.contentResolver.takePersistableUriPermission(
                uri,
                Intent.FLAG_GRANT_READ_URI_PERMISSION,
            )
        }
        Unit
    }
    val folderPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        if (uri != null) {
            persist(uri)
            NativeBridge.importMinecraftFolder(uri.toString())
        }
    }
    val zipPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            persist(uri)
            NativeBridge.importMinecraftZip(uri.toString())
        }
    }
    val snackbars = remember { SnackbarHostState() }
    LaunchedEffect(state?.status) {
        val status = state?.status.orEmpty()
        if (status.isNotBlank()) {
            snackbars.showSnackbar(status, withDismissAction = true)
            act("dismiss_status")
        }
    }
    // Show a crash notice from the previous run once, through the existing snackbar host,
    // then consume the marker. [remember] keeps it to one show per composition lifetime.
    val crashShown = rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(crashNotice) {
        if (!crashNotice.isNullOrBlank() && !crashShown.value) {
            crashShown.value = true
            snackbars.showSnackbar(crashNotice, withDismissAction = true)
            CrashHandler.clear(context)
        }
    }
    BackHandler(enabled = overlay != null || settingsOpen || destination != Destination.Play) {
        when {
            overlay != null -> overlay = null
            settingsOpen -> settingsOpen = false
            else -> destination = Destination.Play
        }
    }

    val actions = LauncherActions(
        play = {
            val reply = act("play")
            if ((reply.ok as? JSONObject)?.optBoolean("launch") == true) onPlay()
        },
        openAccount = { overlay = Overlay.Account },
        openWorlds = { overlay = Overlay.Worlds },
        openPerformance = { overlay = Overlay.Performance },
        openTouchLayouts = { overlay = Overlay.TouchLayouts },
        openServers = { overlay = Overlay.Servers },
        openScreenshots = {
            settingsOpen = false
            destination = Destination.Screenshots
        },
        openAppearance = {
            backgroundError = null
            overlay = Overlay.Appearance
        },
        choose = { id ->
            if (id == "profile") {
                overlay = Overlay.Profiles
            } else {
                act("options", id).options()?.let { overlay = Overlay.Choose(id, it) }
            }
        },
        edit = { id, title, value, hint -> overlay = Overlay.Edit(id, title, value, hint) },
        import = { kind ->
            importKind = kind
            picker.launch(arrayOf("*/*"))
        },
        importNative = { nativePicker.launch(arrayOf("application/octet-stream", "application/zip", "application/x-zip-compressed", "*/*")) },
        removeNative = { id -> overlay = Overlay.RemoveNative(id) },
        disableNative = { act("native_disable") },
        importWardrobe = { kind ->
            wardrobeKind = kind
            wardrobePicker.launch(arrayOf("image/png"))
        },
        remove = { overlay = Overlay.Remove(it) },
        importFolder = { folderPicker.launch(null) },
        importZip = {
            zipPicker.launch(
                arrayOf("application/zip", "application/x-zip-compressed", "application/octet-stream"),
            )
        },
        cancelImport = { act("cancel_import") },
        repairInstallation = { act("repair_installation") },
        removeInstallation = { overlay = Overlay.RemoveInstallation },
        toggleSettings = { settingsOpen = !settingsOpen },
    )

    LauncherScaffold(
        state = state,
        destination = destination,
        onDestination = { destination = it },
        appearance = appearance,
        actions = actions,
        snackbars = snackbars,
        settingsOpen = settingsOpen,
        failure = failure,
        onRetry = onRetry,
    )

    if (state != null) {
        when (val o = overlay) {
            Overlay.Account -> AccountSheet(state, actions, onDismiss = { overlay = null })
            is Overlay.Choose -> ChooseDialog(o, state.config.renderScale, onDismiss = { overlay = null })
            Overlay.Profiles -> InstancesSheet(onDismiss = { overlay = null }, onPlay = onPlay)
            Overlay.Worlds -> WorldsSheet(onDismiss = { overlay = null }, onPlay = onPlay)
            Overlay.Performance -> PerformanceSheet(onDismiss = { overlay = null })
            Overlay.TouchLayouts -> TouchLayoutsSheet(onDismiss = { overlay = null })
            Overlay.Servers -> ServersSheet(onDismiss = { overlay = null }, onPlay = onPlay)
            Overlay.Appearance -> AppearanceSheet(
                appearance = appearance,
                pickerError = backgroundError,
                importing = importingBackground,
                onDismiss = {
                    overlay = null
                    backgroundError = null
                },
                onChooseBackground = {
                    backgroundError = null
                    backgroundPicker.launch(arrayOf("image/*"))
                },
                onChanged = {
                    backgroundError = null
                    onAppearance(it)
                },
            )
            is Overlay.Edit -> EditDialog(o, onDismiss = { overlay = null })
            is Overlay.Remove -> RemoveDialog(o.item, onDismiss = { overlay = null })
            is Overlay.RemoveNative -> RemoveNativeDialog(o.id, onDismiss = { overlay = null })
            Overlay.RemoveInstallation -> RemoveInstallationDialog(state.install, onDismiss = { overlay = null })
            null -> {}
        }
        ImportDialogs(state.documents)
        // First run, or still on SAF: offer internal storage once. "Not now"
        // keeps the SAF mode fully working and never shows again.
        if (state.storage.state == "private" && !state.storage.prompted && !storagePromptDismissed && !settingsOpen) {
            StoragePromptDialog(onDismiss = { storagePromptDismissed = true })
        }
    }
}

/** Derived once from [BoxWithConstraints] and threaded through the pages. */
private data class LayoutConfig(val wide: Boolean, val short: Boolean)

/** Navigation labels stay visible in every layout; short landscape just makes them smaller. */
@Composable
private fun NavLabel(text: String, short: Boolean) {
    Text(
        text,
        style = if (short) MaterialTheme.typography.labelSmall else MaterialTheme.typography.labelMedium,
        maxLines = 1,
        softWrap = false,
    )
}

/** What the content area shows before the first snapshot: a transient spinner or a retryable failure. */
@Composable
private fun LoadPlaceholder(failure: String?, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        if (failure == null) {
            LoadingIndicator()
        } else {
            Column(
                modifier = Modifier.padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text("Launcher unavailable", style = MaterialTheme.typography.titleMedium)
                Text(
                    failure,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                FilledTonalButton(onClick = onRetry) { Text("Retry") }
            }
        }
    }
}

/** Navigation and the current page; [LauncherApp] adds the bridge, dialogs and pickers. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LauncherScaffold(
    state: Snapshot?,
    destination: Destination,
    onDestination: (Destination) -> Unit,
    appearance: LauncherAppearance,
    actions: LauncherActions,
    snackbars: SnackbarHostState,
    settingsOpen: Boolean = false,
    failure: String? = null,
    onRetry: () -> Unit = {},
) {
    // Status snackbars sit over the page, never over the navigation bar or rail.
    val snackbarHost = @Composable { SnackbarHost(snackbars, Modifier.padding(bottom = 4.dp)) }
    Box(Modifier.fillMaxSize()) {
        LauncherBackground(appearance)
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            containerColor = Color.Transparent,
            contentColor = MaterialTheme.colorScheme.onBackground,
            contentWindowInsets = WindowInsets(0),
        ) { padding ->
            BoxWithConstraints(Modifier.fillMaxSize().padding(padding)) {
                val layout = LayoutConfig(
                    wide = maxWidth >= 600.dp,
                    // Phones held sideways are ~400 dp tall: tighter header, no rail logo.
                    short = maxHeight < 520.dp,
                )
                val wide = layout.wide
                val short = layout.short
                val page: @Composable () -> Unit = {
                    if (state != null) {
                        Page(destination, state, wide, short, actions, Modifier.fillMaxSize())
                    } else {
                        LoadPlaceholder(failure, onRetry, Modifier.fillMaxSize())
                    }
                }
                if (wide) {
                    Row(Modifier.fillMaxSize()) {
                        NavigationRail(
                            modifier = Modifier.windowInsetsPadding(WindowInsets.safeDrawing),
                        ) {
                            Spacer(Modifier.weight(1f))
                            Destination.entries.filter { it.inNavigation }.forEach { d ->
                                NavigationRailItem(
                                    selected = destination == d,
                                    onClick = { onDestination(d) },
                                    icon = { Icon(d.icon, contentDescription = null) },
                                    label = { NavLabel(d.label, short) },
                                )
                            }
                            Spacer(Modifier.weight(1f))
                        }
                        Box(Modifier.weight(1f).windowInsetsPadding(WindowInsets.safeDrawing)) {
                            page()
                            if (!settingsOpen) Box(Modifier.align(Alignment.BottomCenter)) { snackbarHost() }
                        }
                    }
                } else {
                    Column(Modifier.fillMaxSize()) {
                        Box(
                            Modifier.weight(1f).windowInsetsPadding(
                                WindowInsets.safeDrawing.only(WindowInsetsSides.Top + WindowInsetsSides.Horizontal),
                            ),
                        ) {
                            page()
                            if (!settingsOpen) Box(Modifier.align(Alignment.BottomCenter)) { snackbarHost() }
                        }
                        NavigationBar {
                            Destination.entries.filter { it.inNavigation }.forEach { d ->
                                NavigationBarItem(
                                    selected = destination == d,
                                    onClick = { onDestination(d) },
                                    icon = { Icon(d.icon, contentDescription = null) },
                                    label = { NavLabel(d.label, short) },
                                )
                            }
                        }
                    }
                }
                // Settings covers everything, navigation included, and simply fades in and out.
                // It needs a snapshot, so it stays out of the loading/failed states.
                if (state != null) {
                    AnimatedVisibility(settingsOpen, enter = fadeIn(), exit = fadeOut()) {
                        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
                            Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
                                Column(Modifier.fillMaxSize().padding(horizontal = if (wide) 24.dp else 16.dp)) {
                                    Header(Destination.Settings, state, short, actions)
                                    Box(Modifier.weight(1f).fillMaxWidth()) { SettingsScreen(state, wide, actions) }
                                }
                                Box(Modifier.align(Alignment.BottomCenter)) { snackbarHost() }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** What the pages ask the launcher shell to do. */
class LauncherActions(
    val play: () -> Unit,
    val openAccount: () -> Unit,
    val importNative: () -> Unit,
    val removeNative: (String) -> Unit,
    val disableNative: () -> Unit,
    val openWorlds: () -> Unit,
    val openPerformance: () -> Unit,
    val openTouchLayouts: () -> Unit,
    val openServers: () -> Unit,
    val openScreenshots: () -> Unit,
    val openAppearance: () -> Unit,
    val choose: (String) -> Unit,
    val edit: (id: String, title: String, value: String, hint: String) -> Unit,
    val import: (String) -> Unit,
    val importWardrobe: (String) -> Unit,
    val remove: (LocalItem) -> Unit,
    val importFolder: () -> Unit,
    val importZip: () -> Unit,
    val cancelImport: () -> Unit,
    val repairInstallation: () -> Unit,
    val removeInstallation: () -> Unit,
    val toggleSettings: () -> Unit,
)

/**
 * Horizon's mark (docs/media/brand/mark.svg): an H whose crossbar is the horizon, on a 16-unit grid.
 * Stems take the content colour, their reflection a third of it, the horizon the primary colour.
 */
@Composable
fun HorizonMark(modifier: Modifier = Modifier) {
    val stem = MaterialTheme.colorScheme.onSurface
    val line = MaterialTheme.colorScheme.primary
    Canvas(modifier.semantics { contentDescription = "Horizon" }) {
        val u = size.minDimension / 16f
        fun rect(x: Int, y: Int, w: Int, h: Int, color: Color) =
            drawRect(color, Offset(x * u, y * u), Size(w * u, h * u))
        for (x in intArrayOf(4, 10)) {
            rect(x, 3, 2, 4, stem)
            rect(x, 8, 2, 4, stem.copy(alpha = 0.36f))
        }
        rect(1, 7, 14, 1, line)
    }
}

@Composable
private fun Page(
    destination: Destination,
    state: Snapshot,
    wide: Boolean,
    short: Boolean,
    actions: LauncherActions,
    modifier: Modifier,
) {
    AnimatedContent(
        targetState = destination,
        transitionSpec = { fadeIn() togetherWith fadeOut() },
        modifier = modifier,
        label = "page",
    ) { page ->
        Column(Modifier.fillMaxSize().padding(horizontal = if (wide) 24.dp else 16.dp)) {
            Header(page, state, short, actions)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                when (page) {
                    Destination.Play -> PlayScreen(state, wide, short, actions)
                    Destination.Skins -> SkinsScreen(state, wide, short, actions)
                    Destination.Mods, Destination.Packs, Destination.Shaders ->
                        CatalogScreen(page, state, wide, short, actions)
                    Destination.Installed -> InstalledScreen(state, actions)
                    Destination.Screenshots -> ScreenshotGallery(state.config.profileName)
                    Destination.Settings -> {} // drawn as a layer by LauncherScaffold
                }
            }
        }
    }
}

@Composable
private fun Header(page: Destination, state: Snapshot, short: Boolean, actions: LauncherActions) {
    Row(
        Modifier.fillMaxWidth().padding(top = if (short) 6.dp else 12.dp, bottom = if (short) 6.dp else 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (page == Destination.Play) HorizonMark(Modifier.size(if (short) 28.dp else 32.dp))
        Text(
            when (page) {
                Destination.Play -> "Horizon Android"
                Destination.Skins -> "Skins & Capes"
                Destination.Mods -> "Horizon Mods"
                Destination.Packs -> "Resource Packs"
                Destination.Shaders -> "Shader Packs"
                Destination.Installed -> "Installed"
                Destination.Screenshots -> "Screenshots"
                Destination.Settings -> "Settings"
            },
            style = if (short) MaterialTheme.typography.titleLarge else MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.weight(1f),
            maxLines = 1,
            overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
        )
        if (page != Destination.Settings) AccountChip(state.account, actions.openAccount)
        val open = page == Destination.Settings
        FilledTonalIconButton(onClick = actions.toggleSettings) {
            Crossfade(open, label = "settings-icon") { closing ->
                Icon(
                    if (closing) Icons.Rounded.Close else Icons.Rounded.Settings,
                    contentDescription = if (closing) "Close settings" else "Settings",
                )
            }
        }
    }
}

/** [inChip]: drawn on the account chip, taking its tonal colour like the settings button beside it. */
@Composable
fun Avatar(account: Account, size: androidx.compose.ui.unit.Dp, inChip: Boolean = false) {
    val letter = account.profile?.firstOrNull()?.uppercaseChar()
    Surface(
        modifier = Modifier.size(size),
        shape = CircleShape,
        color = if (inChip) Color.Transparent else MaterialTheme.colorScheme.primary,
        contentColor = if (inChip) LocalContentColor.current else MaterialTheme.colorScheme.onPrimary,
    ) {
        Box(contentAlignment = Alignment.Center) {
            if (letter != null) {
                Text(
                    letter.toString(),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = LocalContentColor.current,
                )
            } else {
                Icon(
                    Icons.Rounded.Person,
                    contentDescription = null,
                    modifier = Modifier.size(size * 0.6f),
                    tint = LocalContentColor.current,
                )
            }
        }
    }
}

@Composable
private fun AccountChip(account: Account, onClick: () -> Unit) {
    val named = account.profile != null || account.signedIn
    Surface(
        onClick = onClick,
        shape = CircleShape,
        color = MaterialTheme.colorScheme.secondaryContainer,
        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
    ) {
        Row(
            Modifier.padding(start = if (named) 4.dp else 0.dp, end = if (named) 12.dp else 0.dp, top = if (named) 4.dp else 0.dp, bottom = if (named) 4.dp else 0.dp).height(if (named) 32.dp else 40.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Avatar(account, if (named) 32.dp else 40.dp, inChip = true)
            val label = account.profile ?: account.status.takeIf { account.signedIn }
            if (label != null) {
                Text(
                    label,
                    style = MaterialTheme.typography.labelLarge,
                    maxLines = 1,
                    modifier = Modifier.widthIn(max = 140.dp),
                    overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                )
            }
        }
    }
}
