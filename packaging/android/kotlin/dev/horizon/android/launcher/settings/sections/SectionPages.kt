package dev.horizon.android.launcher.settings.sections

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dev.horizon.android.R
import dev.horizon.android.launcher.LauncherActions
import dev.horizon.android.launcher.SettingRow
import dev.horizon.android.launcher.Snapshot

/** Shared parameter set for every category page. */
internal data class SectionScope(
    val state: Snapshot,
    val actions: LauncherActions,
    val extraWide: Boolean,
    val onStorage: () -> Unit,
    val onGameFiles: () -> Unit,
    val onAccessChanged: () -> Unit,
)

@Composable
fun GeneralSection(rows: List<SettingRow>, scope: SectionScope) {
    SettingsRowList(rows, scope.state, scope.actions, scope.extraWide, scope.onStorage, scope.onGameFiles)
}

@Composable
fun GraphicsSection(rows: List<SettingRow>, scope: SectionScope) {
    SettingsRowList(rows, scope.state, scope.actions, scope.extraWide, scope.onStorage, scope.onGameFiles)
}

@Composable
fun DisplaySection(rows: List<SettingRow>, scope: SectionScope) {
    SettingsRowList(rows, scope.state, scope.actions, scope.extraWide, scope.onStorage, scope.onGameFiles)
}

@Composable
fun AppearanceSection(rows: List<SettingRow>, scope: SectionScope) {
    SettingsRowList(rows, scope.state, scope.actions, scope.extraWide, scope.onStorage, scope.onGameFiles)
}

@Composable
fun DiagnosticsSection(rows: List<SettingRow>, scope: SectionScope) {
    SettingsRowList(rows, scope.state, scope.actions, scope.extraWide, scope.onStorage, scope.onGameFiles)
}

@Composable
fun StorageSection(rows: List<SettingRow>, scope: SectionScope) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        StorageAccessRow(scope.extraWide, scope.onAccessChanged)
        SettingsRowList(rows, scope.state, scope.actions, scope.extraWide, scope.onStorage, scope.onGameFiles)
        ShortcutRow(
            icon = Icons.Rounded.Folder,
            label = stringResource(R.string.settings_game_files_browse),
            detail = stringResource(R.string.category_storage_subtitle),
            extraWide = scope.extraWide,
            onClick = scope.onGameFiles,
        )
    }
}

@Composable
fun ModsSection(rows: List<SettingRow>, scope: SectionScope) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (rows.isNotEmpty()) {
            SettingsRowList(rows, scope.state, scope.actions, scope.extraWide, scope.onStorage, scope.onGameFiles)
        }
        Text(
            stringResource(R.string.settings_mods_hint),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
    }
}

@Composable
fun AccountsSection(rows: List<SettingRow>, scope: SectionScope) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (rows.isNotEmpty()) {
            SettingsRowList(rows, scope.state, scope.actions, scope.extraWide, scope.onStorage, scope.onGameFiles)
        }
        ShortcutRow(
            icon = Icons.Rounded.Person,
            label = stringResource(R.string.settings_account_shortcut),
            detail = scope.state.account.status.ifBlank { stringResource(R.string.category_accounts_subtitle) },
            extraWide = scope.extraWide,
            onClick = scope.actions.openAccount,
        )
    }
}
