package dev.horizon.android.launcher.settings

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.BugReport
import androidx.compose.material.icons.rounded.Extension
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.Monitor
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.ScreenRotation
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.ui.graphics.vector.ImageVector
import dev.horizon.android.R
import dev.horizon.android.launcher.SettingRow

/**
 * The eight CloudStream-style landing categories. The native bridge still emits
 * flat groups and rows (`Snapshot.settings`); this registry is a local view over
 * those rows keyed by row id, so no new bridge method or JSON field is needed.
 * Categories with no native rows (Mods, Accounts) offer shortcuts in their pages.
 */
enum class SettingsCategory(
    val key: String,
    val icon: ImageVector,
    val titleRes: Int,
    val subtitleRes: Int,
    val rowIds: Set<String>,
) {
    General(
        "general",
        Icons.Rounded.Settings,
        R.string.category_general_title,
        R.string.category_general_subtitle,
        setOf("profile", "name", "server", "servers"),
    ),
    Graphics(
        "graphics",
        Icons.Rounded.Monitor,
        R.string.category_graphics_title,
        R.string.category_graphics_subtitle,
        setOf("renderer", "render_scale", "display_hz", "performance_hints", "performance_presets", "worker_limit", "cache_budget_mib"),
    ),
    Display(
        "display",
        Icons.Rounded.ScreenRotation,
        R.string.category_display_title,
        R.string.category_display_subtitle,
        setOf("orientation", "aspect", "immersive", "cutout", "input_host", "touch_layouts"),
    ),
    Appearance(
        "appearance",
        Icons.Rounded.Palette,
        R.string.category_appearance_title,
        R.string.category_appearance_subtitle,
        setOf("appearance"),
    ),
    Storage(
        "storage",
        Icons.Rounded.Folder,
        R.string.category_storage_title,
        R.string.category_storage_subtitle,
        setOf("storage", "screenshots", "game_files", "world_files"),
    ),
    Mods(
        "mods",
        Icons.Rounded.Extension,
        R.string.category_mods_title,
        R.string.category_mods_subtitle,
        emptySet(),
    ),
    Accounts(
        "accounts",
        Icons.Rounded.Person,
        R.string.category_accounts_title,
        R.string.category_accounts_subtitle,
        emptySet(),
    ),
    Diagnostics(
        "diagnostics",
        Icons.Rounded.BugReport,
        R.string.category_diagnostics_title,
        R.string.category_diagnostics_subtitle,
        setOf("gpu_profile", "record_performance", "export_diagnostics", "export_performance", "logs"),
    );

    /** Rows from the flat snapshot that belong to this category, in bridge order. */
    fun rows(all: List<SettingRow>): List<SettingRow> = all.filter { it.id in rowIds }

    companion object {
        fun fromKey(key: String?): SettingsCategory? = entries.firstOrNull { it.key == key }

        /** Fallback bucket for any future native row we have not mapped yet. */
        fun forRow(id: String): SettingsCategory = entries.firstOrNull { id in it.rowIds } ?: General

        /** All native rows, ordered by category then by their original order. */
        fun ordered(all: List<SettingRow>): List<Pair<SettingsCategory, SettingRow>> =
            buildList {
                entries.forEach { category ->
                    all.filter { it.id in category.rowIds }.forEach { add(category to it) }
                }
                // Preserve unknown rows instead of dropping them.
                all.filter { row -> entries.none { row.id in it.rowIds } }
                    .forEach { add(General to it) }
            }
    }
}
