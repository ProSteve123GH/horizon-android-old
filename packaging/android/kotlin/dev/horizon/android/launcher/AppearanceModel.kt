package dev.horizon.android.launcher

import org.json.JSONObject

/** One theme mode offered by the Rust appearance store. */
data class ThemeChoice(val key: String, val label: String)

/** The four Material primary roles for one accent in one light/dark scheme. */
data class AccentColors(
    val primary: String,
    val onPrimary: String,
    val primaryContainer: String,
    val onPrimaryContainer: String,
)

/** A built-in accent and its two contrast-checked colour pairs. */
data class AccentChoice(
    val key: String,
    val label: String,
    val light: AccentColors?,
    val dark: AccentColors?,
)

/**
 * Launcher-wide appearance state. The Rust store owns persistence and the
 * colour/contrast data; Compose only renders it and sends named actions.
 */
data class LauncherAppearance(
    val theme: String = "system",
    val themeLabel: String = "System default",
    val accent: String = "system",
    val accentLabel: String = "System default",
    val themes: List<ThemeChoice> = emptyList(),
    val accents: List<AccentChoice> = emptyList(),
    val backgroundPath: String? = null,
    val backgroundRevision: Long = 0,
    val backgroundMissing: Boolean = false,
) {
    val hasCustomBackground: Boolean get() = backgroundPath != null

    companion object {
        fun load(): LauncherAppearance {
            val reply = act("appearance_state")
            return (reply.ok as? JSONObject)?.let { parse(it) } ?: LauncherAppearance()
        }

        fun parse(root: JSONObject): LauncherAppearance {
            val themes = root.optJSONArray("themes")?.let { array ->
                (0 until array.length()).mapNotNull { index ->
                    array.optJSONObject(index)?.let {
                        val key = it.optString("key")
                        if (key.isBlank()) null else ThemeChoice(key, it.optString("label"))
                    }
                }
            }.orEmpty()
            val accents = root.optJSONArray("accents")?.let { array ->
                (0 until array.length()).mapNotNull { index ->
                    array.optJSONObject(index)?.let {
                        val key = it.optString("key")
                        if (key.isBlank()) {
                            null
                        } else {
                            AccentChoice(
                                key = key,
                                label = it.optString("label"),
                                light = parseColors(it.optJSONObject("light")),
                                dark = parseColors(it.optJSONObject("dark")),
                            )
                        }
                    }
                }
            }.orEmpty()
            val backgroundPath = if (root.isNull("background_path")) {
                null
            } else {
                root.optString("background_path").takeIf { it.isNotBlank() }
            }
            return LauncherAppearance(
                theme = root.optString("theme", "system"),
                themeLabel = root.optString("theme_label", "System default"),
                accent = root.optString("accent", "system"),
                accentLabel = root.optString("accent_label", "System default"),
                themes = themes,
                accents = accents,
                backgroundPath = backgroundPath,
                backgroundRevision = root.optLong("background_revision"),
                backgroundMissing = root.optBoolean("background_missing"),
            )
        }

        private fun parseColors(value: JSONObject?): AccentColors? {
            value ?: return null
            return AccentColors(
                primary = value.optString("primary"),
                onPrimary = value.optString("on_primary"),
                primaryContainer = value.optString("primary_container"),
                onPrimaryContainer = value.optString("on_primary_container"),
            )
        }
    }
}
