package dev.horizon.android.launcher.settings

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
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
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.OpenInBrowser
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dev.horizon.android.R
import dev.horizon.android.launcher.Avatar
import dev.horizon.android.launcher.LauncherActions
import dev.horizon.android.launcher.SettingRow
import dev.horizon.android.launcher.Snapshot
import dev.horizon.android.launcher.act
import dev.horizon.android.launcher.settings.sections.AccountsSection
import dev.horizon.android.launcher.settings.sections.AppearanceSection
import dev.horizon.android.launcher.settings.sections.DiagnosticsSection
import dev.horizon.android.launcher.settings.sections.DisplaySection
import dev.horizon.android.launcher.settings.sections.GeneralSection
import dev.horizon.android.launcher.settings.sections.GraphicsSection
import dev.horizon.android.launcher.settings.sections.ModsSection
import dev.horizon.android.launcher.settings.sections.SectionScope
import dev.horizon.android.launcher.settings.sections.SettingsRowList
import dev.horizon.android.launcher.settings.sections.StorageSection

/**
 * CloudStream-style settings: an instance header, a search box, category cards,
 * detail pages with a breadcrumb back, and a build-info footer. All data still
 * comes from [Snapshot.settings] and every action uses the existing bridge.
 */
@Composable
fun SettingsRoot(
    state: Snapshot,
    wide: Boolean,
    extraWide: Boolean,
    actions: LauncherActions,
    onGameFiles: () -> Unit,
    onStorage: () -> Unit,
) {
    var query by rememberSaveable { mutableStateOf("") }
    var selectedKey by rememberSaveable { mutableStateOf("") }
    var accessTick by remember { mutableIntStateOf(0) }
    // Reading the tick forces StorageAccessRow to re-check the permission.
    remember(accessTick) { Unit }

    val allRows = remember(state.settings) { state.settings.flatMap { it.rows } }
    val scope = SectionScope(
        state = state,
        actions = actions,
        extraWide = extraWide,
        onStorage = onStorage,
        onGameFiles = onGameFiles,
        onAccessChanged = { accessTick++ },
    )
    val category = SettingsCategory.fromKey(selectedKey)

    Column(Modifier.fillMaxSize()) {
        SettingsHeader(state)
        Spacer(Modifier.height(12.dp))
        SearchField(query, onQuery = { query = it; selectedKey = "" })
        Spacer(Modifier.height(12.dp))
        if (category == null) {
            LandingPage(state, allRows, wide, extraWide, query, scope) { selectedKey = it }
        } else {
            DetailPage(category, allRows, query, scope) { selectedKey = "" }
        }
        SettingsFooter(state)
    }
}

@Composable
private fun SettingsHeader(state: Snapshot) {
    Surface(
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.primaryContainer,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Avatar(state.account, 56.dp)
            Column(Modifier.weight(1f)) {
                Text(
                    stringResource(R.string.settings_title),
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                )
                Text(
                    stringResource(
                        R.string.settings_header_subtitle,
                        state.config.profileName.ifBlank { state.config.name }
                            .ifBlank { state.account.profile.orEmpty() },
                    ),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun SearchField(query: String, onQuery: (String) -> Unit) {
    OutlinedTextField(
        value = query,
        onValueChange = onQuery,
        placeholder = { Text(stringResource(R.string.settings_search)) },
        leadingIcon = { Icon(Icons.Rounded.Search, null) },
        trailingIcon = {
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQuery("") }) {
                    Icon(Icons.Rounded.Close, stringResource(R.string.settings_search_clear))
                }
            }
        },
        singleLine = true,
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier.fillMaxWidth(),
    )
}

@Composable
private fun ColumnScope.LandingPage(
    state: Snapshot,
    allRows: List<SettingRow>,
    wide: Boolean,
    extraWide: Boolean,
    query: String,
    scope: SectionScope,
    onOpen: (String) -> Unit,
) {
    val q = query.trim()
    val titleOf = SettingsCategory.entries.associateWith { stringResource(it.titleRes) }
    val subtitleOf = SettingsCategory.entries.associateWith { stringResource(it.subtitleRes) }
    val gallery = stringResource(R.string.settings_screenshots_title)
    val columns = if (extraWide) 4 else if (wide) 2 else 1

    if (q.isEmpty()) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(columns),
            modifier = Modifier.fillMaxWidth().weight(1f),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(SettingsCategory.entries, key = { it.key }) { category ->
                CategoryCard(
                    category = category,
                    title = titleOf.getValue(category),
                    subtitle = subtitleOf.getValue(category),
                    extraWide = extraWide,
                    onClick = { onOpen(category.key) },
                )
            }
        }
        return
    }

    val matchedCategories = SettingsCategory.entries.filter { category ->
        titleOf.getValue(category).contains(q, ignoreCase = true) ||
            subtitleOf.getValue(category).contains(q, ignoreCase = true)
    }
    val matchedRows = SettingsCategory.ordered(allRows).filter { (_, row) ->
        val title = if (row.id == "screenshots") gallery else row.title
        title.contains(q, ignoreCase = true) || row.value.contains(q, ignoreCase = true)
    }
    LazyColumn(
        modifier = Modifier.fillMaxWidth().weight(1f),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        matchedCategories.forEach { category ->
            item(key = "result-${category.key}") {
                CategoryCard(
                    category = category,
                    title = titleOf.getValue(category),
                    subtitle = subtitleOf.getValue(category),
                    extraWide = extraWide,
                    onClick = { onOpen(category.key) },
                )
            }
        }
        item {
            Text(
                stringResource(R.string.settings_results, q),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(top = 4.dp),
            )
        }
        if (matchedRows.isEmpty()) {
            item { Text(stringResource(R.string.settings_no_results), style = MaterialTheme.typography.bodyMedium) }
        } else {
            item {
                SettingsRowList(
                    rows = matchedRows.map { it.second },
                    state = state,
                    actions = scope.actions,
                    extraWide = scope.extraWide,
                    onStorage = scope.onStorage,
                    onGameFiles = scope.onGameFiles,
                )
            }
        }
        item { Spacer(Modifier.height(8.dp)) }
    }
}

@Composable
private fun CategoryCard(
    category: SettingsCategory,
    title: String,
    subtitle: String,
    extraWide: Boolean,
    onClick: () -> Unit,
) {
    Surface(
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
        onClick = onClick,
        modifier = Modifier.fillMaxWidth().heightIn(min = if (extraWide) 156.dp else 128.dp),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.secondaryContainer,
                modifier = Modifier.size(if (extraWide) 48.dp else 42.dp),
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(
                        category.icon,
                        null,
                        tint = MaterialTheme.colorScheme.onSecondaryContainer,
                        modifier = Modifier.size(if (extraWide) 28.dp else 24.dp),
                    )
                }
            }
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                maxLines = 2,
            )
            Text(
                subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun ColumnScope.DetailPage(
    category: SettingsCategory,
    allRows: List<SettingRow>,
    query: String,
    scope: SectionScope,
    onBack: () -> Unit,
) {
    val title = stringResource(category.titleRes)
    val q = query.trim()
    val gallery = stringResource(R.string.settings_screenshots_title)
    val rows = category.rows(allRows).filter { row ->
        if (q.isEmpty()) true else {
            val rowTitle = if (row.id == "screenshots") gallery else row.title
            rowTitle.contains(q, ignoreCase = true) || row.value.contains(q, ignoreCase = true)
        }
    }
    Column(Modifier.fillMaxSize()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.settings_back))
            }
            Text(
                stringResource(R.string.settings_breadcrumb, title),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Text(title, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
        Text(
            stringResource(category.subtitleRes),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(12.dp))
        LazyColumn(
            modifier = Modifier.fillMaxWidth().weight(1f),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            item {
                when (category) {
                    SettingsCategory.General -> GeneralSection(rows, scope)
                    SettingsCategory.Graphics -> GraphicsSection(rows, scope)
                    SettingsCategory.Display -> DisplaySection(rows, scope)
                    SettingsCategory.Appearance -> AppearanceSection(rows, scope)
                    SettingsCategory.Storage -> StorageSection(rows, scope)
                    SettingsCategory.Mods -> ModsSection(rows, scope)
                    SettingsCategory.Accounts -> AccountsSection(rows, scope)
                    SettingsCategory.Diagnostics -> DiagnosticsSection(rows, scope)
                }
            }
        }
    }
}

@Composable
private fun SettingsFooter(state: Snapshot) {
    val unknown = stringResource(R.string.settings_unknown)
    val abi = state.buildAbi.ifBlank { Build.SUPPORTED_ABIS.firstOrNull() ?: unknown }
    Column(Modifier.fillMaxWidth().padding(top = 12.dp)) {
        androidx.compose.material3.HorizontalDivider()
        Spacer(Modifier.height(8.dp))
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                FooterLine(stringResource(R.string.settings_footer_version), state.version.ifBlank { unknown })
                FooterLine(
                    stringResource(R.string.settings_footer_engine),
                    state.engineCommit.ifBlank { unknown },
                )
                FooterLine(stringResource(R.string.settings_footer_abi), abi)
                FooterLine(
                    stringResource(R.string.settings_footer_build),
                    state.buildDate.ifBlank { unknown },
                )
            }
            FilledTonalButton(onClick = { act("repository") }) {
                Icon(Icons.Rounded.OpenInBrowser, null, Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.settings_footer_source))
            }
        }
    }
}

@Composable
private fun FooterLine(label: String, value: String) {
    Text(
        "$label: $value",
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
}
