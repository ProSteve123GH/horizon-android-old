package dev.horizon.android.launcher

import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.lazy.staggeredgrid.LazyVerticalStaggeredGrid
import androidx.compose.foundation.lazy.staggeredgrid.StaggeredGridCells
import androidx.compose.foundation.lazy.staggeredgrid.StaggeredGridItemSpan
import androidx.compose.foundation.lazy.staggeredgrid.items
import androidx.compose.material3.VerticalDivider
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.material3.Slider
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import kotlin.math.roundToInt
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ButtonDefaults
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.rounded.Login
import androidx.compose.material.icons.automirrored.rounded.Logout
import androidx.compose.material.icons.rounded.Archive
import androidx.compose.material.icons.rounded.AspectRatio
import androidx.compose.material.icons.rounded.Badge
import androidx.compose.material.icons.rounded.BugReport
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.Dns
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Fullscreen
import androidx.compose.material.icons.rounded.Gamepad
import androidx.compose.material.icons.rounded.HighQuality
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Insights
import androidx.compose.material.icons.rounded.Memory
import androidx.compose.material.icons.rounded.Monitor
import androidx.compose.material.icons.rounded.OpenInBrowser
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PhotoLibrary
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.Storage
import androidx.compose.material.icons.rounded.ScreenRotation
import androidx.compose.material.icons.rounded.Smartphone
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.Timer
import androidx.compose.material.icons.rounded.TouchApp
import androidx.compose.material.icons.rounded.UnfoldMore
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearWavyProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dev.horizon.android.R
import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import java.io.File
import org.json.JSONArray

/**
 * Entry point kept for the rest of the launcher. The landing page, category
 * cards, detail pages, search and footer live in [SettingsRoot] and its
 * `settings/sections` helpers; this wrapper only owns the storage dialog.
 */
@Composable
fun SettingsScreen(
    state: Snapshot,
    wide: Boolean,
    extraWide: Boolean,
    actions: LauncherActions,
    onGameFiles: () -> Unit,
) {
    var storageOpen by remember { mutableStateOf(false) }
    dev.horizon.android.launcher.settings.SettingsRoot(
        state = state,
        wide = wide,
        extraWide = extraWide,
        actions = actions,
        onGameFiles = onGameFiles,
        onStorage = { storageOpen = true },
    )
    if (storageOpen) {
        StorageDialog(state.storage, onDismiss = { storageOpen = false })
    }
}

/** Numeric settings: a slider reads better than a list of presets. */
private val SLIDER_ROWS = setOf("render_scale", "display_hz", "worker_limit", "cache_budget_mib")

/**
 * Numeric settings open a slider instead of a preset list. Render scale is continuous (5% steps);
 * refresh rates stop at the launcher's supported values.
 */
@Composable
private fun SliderDialog(choose: Overlay.Choose, renderScale: Int, onDismiss: () -> Unit) {
    val choices = choose.options.choices
    val continuous = choose.id == "render_scale"
    var value by remember {
        mutableFloatStateOf(
            if (continuous) renderScale.toFloat() else choices.indexOfFirst { it.selected }.coerceAtLeast(0).toFloat()
        )
    }
    val label = if (continuous) "${value.roundToInt()}%" else choices.getOrNull(value.roundToInt())?.label.orEmpty()
    val apply = {
        val arg = if (continuous) value.roundToInt().toString() else choices.getOrNull(value.roundToInt())?.value
        if (arg != null) act("choose", "${choose.id}=$arg")
        onDismiss()
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(choose.options.title, fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 4.dp)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (choose.options.help.isNotBlank()) {
                    Text(
                        choose.options.help,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Text(
                    label,
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.align(Alignment.CenterHorizontally),
                )
                if (continuous) {
                    Slider(value = value, onValueChange = { value = it }, valueRange = 25f..100f, steps = 14)
                } else if (choices.size > 1) {
                    Slider(
                        value = value,
                        onValueChange = { value = it },
                        valueRange = 0f..(choices.size - 1).toFloat(),
                        steps = choices.size - 2,
                    )
                }
                Row(Modifier.fillMaxWidth()) {
                    val ends = if (continuous) "25%" to "100%" else choices.first().label to choices.last().label
                    Text(ends.first, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Spacer(Modifier.weight(1f))
                    Text(ends.second, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        },
        confirmButton = { SaveButton(onClick = apply) },
        dismissButton = { DiscardButton(onDismiss) },
    )
}

@Composable
fun ChooseDialog(choose: Overlay.Choose, renderScale: Int, onDismiss: () -> Unit) {
    if (choose.id in SLIDER_ROWS && choose.options.choices.isNotEmpty()) {
        SliderDialog(choose, renderScale, onDismiss)
        return
    }
    val choices = choose.options.choices
    var picked by remember { mutableStateOf(choices.firstOrNull { it.selected }?.value) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(choose.options.title, fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 4.dp)) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                if (choose.options.help.isNotBlank()) {
                    Text(
                        choose.options.help,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(bottom = 12.dp),
                    )
                }
                if (isLandscape() && choices.size > 2) {
                    // Sideways the dialog is short: tiles pair up so most lists fit without scrolling.
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        choices.chunked(2).forEach { pair ->
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.height(IntrinsicSize.Min)) {
                                pair.forEach { c ->
                                    Box(Modifier.weight(1f).fillMaxHeight()) {
                                        ChoiceTile(c.label, c.detail, picked == c.value, RoundedCornerShape(16.dp)) { picked = c.value }
                                    }
                                }
                                if (pair.size == 1) Spacer(Modifier.weight(1f))
                            }
                        }
                    }
                } else {
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        choices.forEachIndexed { i, c ->
                            ChoiceTile(c.label, c.detail, picked == c.value, connectedShape(i, choices.size)) {
                                picked = c.value
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            SaveButton(enabled = picked != null && picked != choices.firstOrNull { it.selected }?.value) {
                act("choose", "${choose.id}=$picked")
                onDismiss()
            }
        },
        dismissButton = { DiscardButton(onDismiss) },
    )
}

@Composable
fun EditDialog(edit: Overlay.Edit, onDismiss: () -> Unit) {
    var value by remember { mutableStateOf(edit.value) }
    val save = {
        act("text", "${edit.id}=$value")
        onDismiss()
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(edit.title) },
        text = {
            OutlinedTextField(
                value = value,
                onValueChange = { value = if (edit.id == "name") it.take(16) else it },
                singleLine = true,
                supportingText = { Text(edit.hint) },
                keyboardOptions = KeyboardOptions(
                    keyboardType = if (edit.id == "render_scale") KeyboardType.Number else KeyboardType.Text,
                ),
                shape = RoundedCornerShape(16.dp),
            )
        },
        confirmButton = { SaveButton(onClick = save) },
        dismissButton = { DiscardButton(onDismiss) },
    )
}

@Composable
fun RemoveDialog(item: LocalItem, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Remove add-on?") },
        text = { Text("Deletes only ${item.file} from this profile. This cannot be undone.") },
        confirmButton = {
            Button(onClick = {
                act("remove_local", item.index.toString())
                onDismiss()
            }) { Text("Remove") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
fun RemoveNativeDialog(id: String, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Remove native source bundle?") },
        text = { Text("Deletes $id and, when present, its matching content mirror from this profile. This cannot be undone.") },
        confirmButton = {
            Button(onClick = {
                act("native_remove", id)
                onDismiss()
            }) { Text("Remove") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ImportDialogs(documents: Documents) {
    val conflict = documents.conflict
    if (conflict != null) {
        AlertDialog(
            onDismissRequest = {},
            title = { Text("File already exists") },
            text = { Text("${conflict.name} is already installed.") },
            confirmButton = {
                Button(onClick = { act("import_conflict", "replace") }) { Text("Replace") }
            },
            dismissButton = {
                Row {
                    if (conflict.keepBoth) {
                        TextButton(onClick = { act("import_conflict", "keep_both") }) { Text("Keep both") }
                    }
                    TextButton(onClick = { act("import_conflict", "cancel") }) { Text("Cancel") }
                }
            },
        )
    } else if (documents.working) {
        AlertDialog(
            onDismissRequest = {},
            title = { Text("Importing") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    LinearWavyProgressIndicator(Modifier.fillMaxWidth())
                    Text(documents.text ?: "Choose a file")
                }
            },
            confirmButton = { TextButton(onClick = { act("import_cancel") }) { Text("Cancel") } },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AccountSheet(state: Snapshot, actions: LauncherActions, onDismiss: () -> Unit) {
    var elyOpen by remember { mutableStateOf(false) }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        val scroll = Modifier.verticalScroll(rememberScrollState())
        if (isLandscape()) {
            // Sideways: who you are on the left, how to sign in on the right.
            // The whole sheet scrolls; the identity column centres against the sign-in column's height.
            Row(
                Modifier.fillMaxWidth().then(scroll).padding(horizontal = 24.dp).padding(bottom = 24.dp).height(IntrinsicSize.Min),
                horizontalArrangement = Arrangement.spacedBy(20.dp),
            ) {
                Column(
                    Modifier.weight(0.85f).fillMaxHeight(),
                    verticalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterVertically),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    AccountIdentity(state, actions)
                    SavedAccounts(state.account, onDismiss)
                }
                VerticalDivider()
                Column(
                    Modifier.weight(1.15f),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    if (elyOpen) ElyForm(state.account, onBack = { elyOpen = false })
                    else AccountSignIn(state.account, onEly = { elyOpen = true })
                }
            }
        } else {
            Column(
                Modifier.fillMaxWidth().padding(horizontal = 24.dp).padding(bottom = 32.dp).then(scroll),
                verticalArrangement = Arrangement.spacedBy(12.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                AccountIdentity(state, actions)
                SavedAccounts(state.account, onDismiss)
                HorizontalDivider(Modifier.padding(vertical = 4.dp))
                if (elyOpen) ElyForm(state.account, onBack = { elyOpen = false })
                else AccountSignIn(state.account, onEly = { elyOpen = true })
            }
        }
    }
}

@Composable
private fun SavedAccounts(account: Account, onDismiss: () -> Unit) {
    if (account.saved.isEmpty() && account.offlineName.isBlank()) return
    Column(
        Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            "Saved accounts",
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
        )
        val offlineSelected = account.saved.none { it.selected }
        Surface(
            onClick = {
                act("select_offline")
                onDismiss()
            },
            shape = RoundedCornerShape(18.dp),
            color = if (offlineSelected) MaterialTheme.colorScheme.secondaryContainer
            else MaterialTheme.colorScheme.surfaceContainer,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Row(
                Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Icon(Icons.Rounded.Person, null, Modifier.size(20.dp))
                Text(
                    "Offline: ${account.offlineName.ifBlank { "game name" }}",
                    modifier = Modifier.weight(1f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (offlineSelected) Icon(Icons.Rounded.Check, "Selected")
            }
        }
        account.saved.forEach { saved ->
            Surface(
                onClick = {
                    act("select_account", saved.key)
                    onDismiss()
                },
                shape = RoundedCornerShape(18.dp),
                color = if (saved.selected) MaterialTheme.colorScheme.secondaryContainer
                else MaterialTheme.colorScheme.surfaceContainer,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(
                    Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(
                        if (saved.provider == "ely") Icons.Rounded.Public else Icons.Rounded.Person,
                        null,
                        Modifier.size(20.dp),
                    )
                    Column(Modifier.weight(1f)) {
                        Text(saved.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(
                            if (saved.provider == "ely") "Ely.by" else "Microsoft",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    if (saved.selected) Icon(Icons.Rounded.Check, "Selected")
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ElyForm(account: Account, onBack: () -> Unit) {
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var otp by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }
    Column(
        Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("Ely.by account", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        Text(
            "Credentials go only to authserver.ely.by and are not saved on this device.",
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        OutlinedTextField(
            value = username,
            onValueChange = { username = it },
            label = { Text("Username or email") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = password,
            onValueChange = { password = it },
            label = { Text("Password") },
            singleLine = true,
            visualTransformation = if (showPassword) androidx.compose.ui.text.input.VisualTransformation.None
            else PasswordVisualTransformation(),
            trailingIcon = {
                TextButton(onClick = { showPassword = !showPassword }) {
                    Text(if (showPassword) "Hide" else "Show")
                }
            },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = otp,
            onValueChange = { otp = it },
            label = { Text("Two-factor code (optional)") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth(),
        )
        Button(
            onClick = {
                act(
                    "ely_sign_in",
                    org.json.JSONObject()
                        .put("username", username)
                        .put("password", password)
                        .put("otp", otp)
                        .toString(),
                )
                password = ""
            },
            enabled = username.isNotBlank() && password.isNotEmpty() && !account.pending,
            modifier = Modifier.fillMaxWidth().height(48.dp),
        ) { Text("Sign in with Ely.by") }
        if (account.pending) {
            LinearWavyProgressIndicator(Modifier.fillMaxWidth())
            TextButton(onClick = { act("cancel_login") }) { Text("Cancel sign-in") }
        }
        if (account.message.isNotBlank()) {
            Text(
                account.message,
                style = MaterialTheme.typography.bodyMedium,
                textAlign = TextAlign.Center,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        TextButton(onClick = onBack) { Text("Back to Microsoft / offline") }
    }
}

/** Avatar, name and status, with Log out (or Log in offline) right under them. */
@Composable
private fun AccountIdentity(state: Snapshot, actions: LauncherActions) {
    val account = state.account
    Avatar(account, 64.dp)
    Text(
        account.profile ?: state.config.name.ifBlank { "Player" },
        style = MaterialTheme.typography.headlineSmall,
        fontWeight = FontWeight.Bold,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
    Text(
        account.status,
        style = MaterialTheme.typography.bodyLarge,
        textAlign = TextAlign.Center,
        color = if (account.signedIn) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
    )
    // Someone is logged in with a Microsoft account or an offline name; logging out of either
    // turns the button into "Log in offline".
    val microsoft = account.profile != null
    var confirm by remember { mutableStateOf(false) }
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            icon = { Icon(Icons.AutoMirrored.Rounded.Logout, null) },
            title = { Text("Log out?", fontWeight = FontWeight.Bold) },
            text = {
                Text(
                    if (microsoft) "${account.profile} is removed from this phone. You can sign in with Microsoft again at any time."
                    else "The offline name ${state.config.name} is forgotten. Your worlds stay where they are.",
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        if (microsoft) act("sign_out") else act("text", "name=")
                        confirm = false
                    },
                    contentPadding = ButtonDefaults.ButtonWithIconContentPadding,
                ) {
                    Icon(Icons.AutoMirrored.Rounded.Logout, null, Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Log out")
                }
            },
            dismissButton = { DiscardButton { confirm = false } },
        )
    }
    if (microsoft || state.config.name.isNotBlank()) {
        OutlinedButton(
            onClick = { confirm = true },
            contentPadding = ButtonDefaults.ButtonWithIconContentPadding,
        ) {
            Icon(Icons.AutoMirrored.Rounded.Logout, null, Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text("Log out")
        }
    } else {
        FilledTonalButton(
            onClick = { actions.edit("name", "Offline player name", "", "Up to 16 characters") },
            contentPadding = ButtonDefaults.ButtonWithIconContentPadding,
        ) {
            Icon(Icons.AutoMirrored.Rounded.Login, null, Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text("Log in offline")
        }
    }
}

/** Microsoft sign-in: the device code when one is open, the message, and the sign-in buttons. */
@Composable
private fun AccountSignIn(account: Account, onEly: () -> Unit) {
    account.code?.let { code ->
        Surface(
            shape = RoundedCornerShape(24.dp),
            color = MaterialTheme.colorScheme.primaryContainer,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Column(Modifier.padding(16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Text("Your sign-in code", style = MaterialTheme.typography.labelLarge)
                Text(
                    code.userCode,
                    style = MaterialTheme.typography.displaySmall,
                    fontFamily = FontFamily.Monospace,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilledTonalButton(onClick = { act("copy_code") }) {
                        Icon(Icons.Rounded.ContentCopy, null, Modifier.size(18.dp))
                        Text("  Copy")
                    }
                    Button(onClick = { act("open_link") }) {
                        Icon(Icons.Rounded.OpenInBrowser, null, Modifier.size(18.dp))
                        Text("  Open Microsoft")
                    }
                }
            }
        }
    }
    val message = account.message.ifBlank {
        if (account.signedIn) "Account saved securely. Sessions renew automatically."
        else "Your password is entered in Microsoft's browser, never here."
    }
    Text(message, textAlign = TextAlign.Center, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    if (account.needsReview) {
        OutlinedButton(onClick = { act("review_app") }) { Text("Request Minecraft API approval") }
    }
    if (account.pending) {
        LinearWavyProgressIndicatorCompat()
        OutlinedButton(onClick = { act("cancel_login") }, modifier = Modifier.fillMaxWidth().height(48.dp)) {
            Text("Cancel sign-in")
        }
    } else {
        Button(onClick = { act("sign_in") }, modifier = Modifier.fillMaxWidth().height(48.dp)) {
            Text(if (account.signedIn) "Switch Microsoft account" else "Sign in with Microsoft")
        }
        FilledTonalButton(onClick = { act("device_sign_in") }, modifier = Modifier.fillMaxWidth().height(48.dp)) {
            Text("Use a device code instead")
        }
        OutlinedButton(onClick = onEly, modifier = Modifier.fillMaxWidth().height(48.dp)) {
            Icon(Icons.Rounded.Public, null, Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text("Sign in with Ely.by")
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun LinearWavyProgressIndicatorCompat() {
    LinearWavyProgressIndicator(Modifier.fillMaxWidth())
}



/** True when this install can read and write the shared storage folder. */
private fun hasStorageAccess(context: android.content.Context): Boolean =
    if (Build.VERSION.SDK_INT >= 30) {
        Environment.isExternalStorageManager()
    } else {
        ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.WRITE_EXTERNAL_STORAGE,
        ) == PackageManager.PERMISSION_GRANTED
    }

/** The request lambda and whether the last attempt failed to open settings. */
private class StorageGrant(val request: () -> Unit, val error: Boolean)

/**
 * The shared all-files-access flow used by the storage dialog and the first-run
 * prompt: on Android 11+ it sends the user to the system page for this app;
 * coming back with the access granted starts [onGranted]. Android 9 and 10 ask
 * for the legacy storage permission instead.
 */
@Composable
private fun rememberStorageGrant(onGranted: () -> Unit): StorageGrant {
    val context = LocalContext.current
    var awaitingGrant by rememberSaveable { mutableStateOf(false) }
    val requestPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) onGranted()
        awaitingGrant = false
    }
    // Coming back from the system's all-files-access page: if it was granted,
    // start the move without making the user hunt for the button again.
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        if (awaitingGrant && hasStorageAccess(context)) {
            awaitingGrant = false
            onGranted()
        }
    }
    var grantError by remember { mutableStateOf(false) }
    val request = {
        awaitingGrant = true
        grantError = false
        if (Build.VERSION.SDK_INT >= 30) {
            // Some OEM skins do not host the per-app all-files page.
            val intent = Intent(
                Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                Uri.parse("package:${context.packageName}"),
            )
            if (runCatching { context.startActivity(intent) }.isFailure) {
                grantError = true
                awaitingGrant = false
            }
        } else {
            requestPermission.launch(Manifest.permission.WRITE_EXTERNAL_STORAGE)
        }
    }
    return StorageGrant(request, grantError)
}

/**
 * Game files location: shared storage is the only mode. The dialog reports the
 * one folder, offers the one-way private-to-shared move (still needed for old
 * installs) and can re-create a missing folder. There is no SAF chooser.
 */
@Composable
private fun StorageDialog(storage: StorageState, onDismiss: () -> Unit) {
    val sharedDir = remember { sharedStorageDir() }
    var conflict by remember { mutableStateOf(false) }
    val grant = rememberStorageGrant(onGranted = {
        // The normal move reports the "folder already exists" case immediately;
        // only then do we offer the explicit back-up-and-use flow.
        conflict = act("storage_migrate", sharedDir.absolutePath).error?.contains("already exists") == true
    })
    var backups by remember { mutableStateOf<List<String>>(emptyList()) }
    LaunchedEffect(Unit) {
        val reply = act("storage_backups", sharedDir.absolutePath)
        backups = (reply.ok as? JSONArray)?.let { array ->
            (0 until array.length()).mapNotNull { array.optString(it).takeIf { name -> name.isNotBlank() } }
        } ?: emptyList()
    }
    val state = storage.state
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.storage_dialog_title), fontWeight = FontWeight.Bold) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (state == "moving") {
                    Text(stringResource(R.string.storage_dialog_moving))
                    val fraction = if (storage.total > 0) (storage.done.toFloat() / storage.total) else null
                    if (fraction != null) {
                        LinearWavyProgressIndicator(
                            progress = { fraction.coerceIn(0f, 1f) },
                            modifier = Modifier.fillMaxWidth(),
                        )
                    } else {
                        LinearWavyProgressIndicator(Modifier.fillMaxWidth())
                    }
                } else {
                    val recorded = storage.dir ?: sharedDir.absolutePath
                    Text(stringResource(R.string.storage_dialog_shared), fontWeight = FontWeight.SemiBold)
                    Text(
                        stringResource(R.string.storage_dialog_shared_detail, recorded),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    if (state == "missing") {
                        Text(
                            stringResource(R.string.storage_dialog_missing),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                    if (conflict) {
                        Text(
                            stringResource(R.string.storage_conflict_note),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                    Text(
                        stringResource(R.string.storage_dialog_private_note),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    if (grant.error) {
                        Text(
                            stringResource(R.string.storage_dialog_error),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                    if (backups.isNotEmpty()) {
                        HorizontalDivider(Modifier.padding(vertical = 4.dp))
                        Text(
                            stringResource(R.string.storage_backups),
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        Text(
                            stringResource(R.string.storage_restore_note),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        backups.forEach { name ->
                            Row(
                                Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Text(
                                    name,
                                    style = MaterialTheme.typography.bodySmall,
                                    modifier = Modifier.weight(1f),
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                TextButton(onClick = {
                                    act("storage_restore_backup", name)
                                    onDismiss()
                                }) { Text(stringResource(R.string.storage_restore)) }
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            when (state) {
                "private" -> Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    FilledTonalButton(onClick = grant.request) {
                        Text(stringResource(R.string.storage_dialog_move))
                    }
                    if (conflict) {
                        OutlinedButton(onClick = {
                            act("storage_adopt_with_backup", sharedDir.absolutePath)
                            onDismiss()
                        }) {
                            Text(stringResource(R.string.storage_backup_and_use))
                        }
                    }
                }
                "missing" -> OutlinedButton(onClick = { act("storage_recreate") }) {
                    Text(stringResource(R.string.storage_dialog_recreate))
                }
                else -> {}
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.storage_dialog_close)) } },
    )
}

/**
 * One-shot offer shown on launch while the game files are still in app
 * storage: move to the shared folder once the user grants the access Android
 * shows. "Not now" records the choice and keeps the SAF mode fully working;
 * it never shows again after either button.
 */
@Composable
fun StoragePromptDialog(onDismiss: () -> Unit) {
    val sharedDir = remember { sharedStorageDir() }
    val grant = rememberStorageGrant(onGranted = {
        act("storage_prompt_done")
        act("storage_migrate", sharedDir.absolutePath)
    })
    AlertDialog(
        onDismissRequest = {
            act("storage_prompt_done")
            onDismiss()
        },
        icon = { Icon(Icons.Rounded.FolderOpen, null) },
        title = { Text("Game files on internal storage?", fontWeight = FontWeight.Bold) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(
                    "Horizon can keep worlds, mods, packs, shaders, options and shareable reports in /storage/emulated/0/Horizon-Android, so any file manager can open them directly.",
                )
                Text(
                    stringResource(R.string.storage_prompt_move_back),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (grant.error) {
                    Text(
                        "Could not open the settings page. Open Settings → Apps → Horizon Android (dev builds: Horizon Android Dev) → All files access instead.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        },
        confirmButton = { Button(onClick = grant.request) { Text("Grant access and move") } },
        dismissButton = {
            TextButton(onClick = {
                act("storage_prompt_done")
                onDismiss()
            }) { Text("Not now") }
        },
    )
}
