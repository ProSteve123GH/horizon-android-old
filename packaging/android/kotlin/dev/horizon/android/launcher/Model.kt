package dev.horizon.android.launcher

import org.json.JSONArray
import org.json.JSONObject

private fun JSONObject.text(key: String): String? = if (isNull(key)) null else optString(key)
private fun <T> JSONArray?.list(map: (JSONObject) -> T): List<T> =
    if (this == null) emptyList() else List(length()) { map(getJSONObject(it)) }

data class Install(
    val installed: Boolean,
    val installing: Boolean,
    val importing: Boolean,
    val cancelable: Boolean,
    val stage: String,
    val stageIndex: Int,
    val stages: List<String>,
    val overall: Float,
    val detail: String,
    val source: String,
    val sizeBytes: Long,
    val cacheBytes: Long,
    val filesMissing: Long,
    val freeBytes: Long,
)

data class LoginCode(val userCode: String, val uri: String)

data class SavedAccount(
    val key: String,
    val name: String,
    val provider: String,
    val selected: Boolean,
)

data class Account(
    val status: String,
    val signedIn: Boolean,
    val profile: String?,
    val pending: Boolean,
    val message: String,
    val code: LoginCode?,
    val needsReview: Boolean,
    val saved: List<SavedAccount>,
    val offlineName: String,
)

data class WardrobeEntry(
    val index: Int,
    val id: String,
    val title: String,
    val kind: String,
    val selected: Boolean,
    val image: String,
)

data class CloudCape(val index: Int, val title: String, val active: Boolean)
data class CloudSkin(val title: String, val active: Boolean, val slim: Boolean)

data class WardrobeState(
    val provider: String?,
    val capes: Boolean,
    val model: String,
    val hideCape: Boolean,
    val selectedSkin: Boolean,
    val selectedCape: Boolean,
    val entries: List<WardrobeEntry>,
    val cloudCapes: List<CloudCape>,
    val cloudSkins: List<CloudSkin>,
    val preview: String?,
    val previewStamp: Long,
    val busy: Boolean,
    val damaged: Boolean,
    val note: String,
)

data class Config(
    val name: String,
    val server: String,
    val profile: Int,
    val profileName: String,
    val renderer: String,
    val renderScale: Int,
    val input: String,
    val modSource: String,
    val shaderSource: String,
)

data class SettingRow(
    val id: String,
    val title: String,
    val value: String,
    val kind: String,
    val on: Boolean,
)

data class SettingGroup(val name: String, val rows: List<SettingRow>)

data class Project(
    val index: Int,
    val title: String,
    val summary: String,
    val author: String,
    val downloads: Long,
    val state: String?,
    val note: String?,
    val button: String,
)

data class CatalogState(
    val page: String,
    val query: String,
    val results: List<Project>,
    val offset: Int,
    val busy: Boolean,
)

/** Where the player's game files live: `private` (app storage), `shared`
 * (the Horizon-Android folder), `missing` (folder gone) or `moving`. */
data class StorageState(
    val state: String,
    val dir: String?,
    val done: Long,
    val total: Long,
    /** True once the first-run internal-storage prompt was dismissed or answered. */
    val prompted: Boolean,
)

data class LocalItem(
    val index: Int,
    val kind: String,
    val title: String,
    val file: String,
    val enabled: Boolean,
    val toggle: Boolean,
)

/** One stored `.hnmod` bundle in the active instance. */
data class NativeMod(
    val id: String,
    val version: String,
    val name: String,
    val size: Long,
    /** `imported`, `selected` (runs next start), `rejected` (set aside) or `invalid`. */
    val enabledState: String,
    /** Whether the mod takes part in **Compile**. */
    val enabled: Boolean,
)

/** The modded build chosen for the next start, or the one that was set aside. */
data class NativeCurrent(
    val id: String,
    val version: String,
    val versionCode: Long,
    val abi: String,
    val rejected: Boolean,
    val rejectReason: String?,
    /** Ids of the mods that build covers, as recorded in `current.json`. */
    val mods: List<String>,
)

/** One mod inside the built library the build service produced. */
data class NativeModRef(
    val id: String,
    val version: String,
    val sha256: String,
)

/** The mod build pipeline's live state (`idle`, `requesting`, `queued`, `building`, `installing`, `ready`, `failed`). */
data class NativeBuild(
    val state: String,
    val job: String?,
    val cached: Boolean,
    val ahead: Int?,
    val seconds: Int?,
    val sha256: String?,
    val size: Long?,
    val error: String?,
    val settled: Boolean,
    val downloaded: Long,
    val installTotal: Long?,
    val installFailed: Boolean,
    val mods: List<NativeModRef>,
    val versionCode: Long,
    val abi: String,
)

data class NativeMods(
    val mods: List<NativeMod>,
    val current: NativeCurrent?,
    val build: NativeBuild?,
)

data class Conflict(val name: String, val keepBoth: Boolean)

data class Documents(val working: Boolean, val text: String?, val conflict: Conflict?)

data class Snapshot(
    val version: String,
    val status: String,
    val error: String?,
    val install: Install,
    val account: Account,
    val config: Config,
    val appearance: LauncherAppearance,
    val settings: List<SettingGroup>,
    val wardrobe: WardrobeState?,
    val storage: StorageState,
    val catalog: CatalogState,
    val local: List<LocalItem>,
    val documents: Documents,
    val nativeMods: NativeMods,
) {
    companion object {
        fun parse(text: String): Snapshot? {
            val root = runCatching { JSONObject(text) }.getOrNull() ?: return null
            val install = root.getJSONObject("install")
            val account = root.getJSONObject("account")
            val config = root.getJSONObject("config")
            val catalog = root.getJSONObject("catalog")
            val documents = root.getJSONObject("documents")
            return Snapshot(
                version = root.optString("version"),
                status = root.optString("status"),
                error = root.text("error"),
                install = Install(
                    installed = install.optBoolean("installed"),
                    installing = install.optBoolean("installing"),
                    importing = install.optBoolean("importing"),
                    cancelable = install.optBoolean("cancelable"),
                    stage = install.optString("stage"),
                    stageIndex = install.optInt("stage_index"),
                    stages = install.optJSONArray("stages").let { a ->
                        if (a == null) emptyList() else List(a.length()) { a.getString(it) }
                    },
                    overall = install.optDouble("overall", 0.0).toFloat(),
                    detail = install.optString("detail"),
                    source = install.optString("source"),
                    sizeBytes = install.optLong("size_bytes"),
                    cacheBytes = install.optLong("cache_bytes"),
                    filesMissing = install.optLong("files_missing"),
                    freeBytes = install.optLong("free_bytes"),
                ),
                account = Account(
                    status = account.optString("status"),
                    signedIn = account.optBoolean("signed_in"),
                    profile = account.text("profile"),
                    pending = account.optBoolean("pending"),
                    message = account.optString("message"),
                    code = account.optJSONObject("code")?.let {
                        LoginCode(it.optString("user_code"), it.optString("uri"))
                    },
                    needsReview = account.optBoolean("needs_review"),
                    saved = account.optJSONArray("saved").list {
                        SavedAccount(
                            it.optString("key"),
                            it.optString("name"),
                            it.optString("provider"),
                            it.optBoolean("selected"),
                        )
                    },
                    offlineName = account.optString("offline_name"),
                ),
                config = Config(
                    name = config.optString("name"),
                    server = config.optString("server"),
                    profile = config.optInt("profile"),
                    profileName = config.optString("profile_name"),
                    renderer = config.optString("renderer"),
                    renderScale = config.optInt("render_scale"),
                    input = config.optString("input"),
                    modSource = config.optString("mod_source"),
                    shaderSource = config.optString("shader_source"),
                ),
                appearance = root.optJSONObject("appearance")?.let { LauncherAppearance.parse(it) }
                    ?: LauncherAppearance(),
                settings = root.optJSONArray("settings").list { group ->
                    SettingGroup(
                        group.optString("group"),
                        group.optJSONArray("rows").list {
                            SettingRow(
                                it.optString("id"),
                                it.optString("title"),
                                it.optString("value"),
                                it.optString("kind"),
                                it.optBoolean("on"),
                            )
                        },
                    )
                },
                storage = root.optJSONObject("storage")?.let { s2 ->
                    StorageState(
                        state = s2.optString("state", "private"),
                        dir = s2.text("dir"),
                        done = s2.optLong("done"),
                        total = s2.optLong("total"),
                        prompted = s2.optBoolean("prompted", false),
                    )
                } ?: StorageState("private", null, 0, 0, prompted = true),
                wardrobe = root.optJSONObject("wardrobe")?.let { w ->
                    WardrobeState(
                        provider = w.text("provider"),
                        capes = w.optBoolean("capes"),
                        model = w.optString("model"),
                        hideCape = w.optBoolean("hide_cape"),
                        selectedSkin = w.optBoolean("selected_skin"),
                        selectedCape = w.optBoolean("selected_cape"),
                        entries = w.optJSONArray("entries").list {
                            WardrobeEntry(
                                it.optInt("index"),
                                it.optString("id"),
                                it.optString("title"),
                                it.optString("kind"),
                                it.optBoolean("selected"),
                                it.optString("image"),
                            )
                        },
                        cloudCapes = w.optJSONArray("cloud_capes").list {
                            CloudCape(it.optInt("index"), it.optString("title"), it.optBoolean("active"))
                        },
                        cloudSkins = w.optJSONArray("cloud_skins").list {
                            CloudSkin(it.optString("title"), it.optBoolean("active"), it.optBoolean("slim"))
                        },
                        preview = w.text("preview"),
                        previewStamp = w.optLong("preview_stamp"),
                        busy = w.optBoolean("busy"),
                        damaged = w.optBoolean("damaged"),
                        note = w.optString("note"),
                    )
                },
                catalog = CatalogState(
                    page = catalog.optString("page"),
                    query = catalog.optString("query"),
                    results = catalog.optJSONArray("results").list {
                        Project(
                            index = it.optInt("index"),
                            title = it.optString("title"),
                            summary = it.optString("summary"),
                            author = it.optString("author"),
                            downloads = it.optLong("downloads"),
                            state = it.text("state"),
                            note = it.text("note"),
                            button = it.optString("button"),
                        )
                    },
                    offset = catalog.optInt("offset"),
                    busy = catalog.optBoolean("busy"),
                ),
                local = root.optJSONArray("local").list {
                    LocalItem(
                        index = it.optInt("index"),
                        kind = it.optString("kind"),
                        title = it.optString("title"),
                        file = it.optString("file"),
                        enabled = it.optBoolean("enabled"),
                        toggle = it.optBoolean("toggle"),
                    )
                },
                documents = Documents(
                    working = documents.optBoolean("working"),
                    text = documents.text("text"),
                    conflict = documents.optJSONObject("conflict")?.let {
                        Conflict(it.optString("name"), it.optBoolean("keep_both"))
                    },
                ),
                nativeMods = root.optJSONObject("native")?.let { n ->
                    NativeMods(
                        mods = n.optJSONArray("mods").list {
                            NativeMod(
                                id = it.optString("id"),
                                version = it.optString("version"),
                                name = it.optString("name"),
                                size = it.optLong("size"),
                                enabledState = it.optString("enabled_state"),
                                enabled = it.optBoolean("enabled", true),
                            )
                        },
                        current = n.optJSONObject("current")?.let { c2 ->
                            NativeCurrent(
                                id = c2.optString("id"),
                                version = c2.optString("version"),
                                versionCode = c2.optLong("versionCode"),
                                abi = c2.optString("abi"),
                                rejected = c2.optBoolean("rejected"),
                                rejectReason = c2.text("reject_reason"),
                                mods = c2.optJSONArray("mods")?.let { list ->
                                    List(list.length()) { index -> list.optString(index) }
                                }.orEmpty(),
                            )
                        },
                        build = n.optJSONObject("build")?.let { b ->
                            NativeBuild(
                                state = b.optString("state", "idle"),
                                job = b.text("job"),
                                cached = b.optBoolean("cached"),
                                ahead = b.optInt("ahead", -1).takeIf { it >= 0 },
                                seconds = b.optInt("seconds", -1).takeIf { it >= 0 },
                                sha256 = b.text("sha256"),
                                size = b.optLong("size", -1).takeIf { it >= 0 },
                                error = b.text("error"),
                                settled = b.optBoolean("settled"),
                                downloaded = b.optLong("downloaded"),
                                installTotal = b.optLong("installTotal", -1).takeIf { it >= 0 },
                                installFailed = b.optBoolean("installFailed"),
                                mods = b.optJSONArray("mods")?.let { list ->
                                    List(list.length()) { index ->
                                        val entry = list.optJSONObject(index)
                                        NativeModRef(
                                            id = entry?.optString("id").orEmpty(),
                                            version = entry?.optString("version").orEmpty(),
                                            sha256 = entry?.optString("sha256").orEmpty(),
                                        )
                                    }
                                }.orEmpty(),
                                versionCode = b.optLong("versionCode"),
                                abi = b.optString("abi"),
                            )
                        },
                    )
                } ?: NativeMods(emptyList(), null, null),
            )
        }
    }
}

data class Choice(val label: String, val detail: String, val value: String, val selected: Boolean)

data class Options(val title: String, val help: String, val choices: List<Choice>)

/** The result of [dev.horizon.android.NativeBridge.call]. */
data class Reply(val ok: Any?, val error: String?) {
    companion object {
        fun parse(text: String?): Reply {
            val root = text?.let { runCatching { JSONObject(it) }.getOrNull() }
                ?: return Reply(null, "No reply from the launcher")
            return Reply(root.opt("ok"), root.text("error"))
        }
    }

    fun options(): Options? {
        val o = ok as? JSONObject ?: return null
        return Options(
            o.optString("title"),
            o.optString("help"),
            o.optJSONArray("choices").list {
                Choice(it.optString("label"), it.optString("detail"), it.optString("value"), it.optBoolean("selected"))
            },
        )
    }
}

/** One single-player world in the active instance's `saves` folder. */
data class World(
    val folder: String,
    val name: String,
    val mode: String,
    val version: String?,
    val lastPlayed: Long?,
    val icon: String?,
    val inUse: Boolean,
    val metadata: Boolean,
)

/** The active instance's worlds, plus the instance name the sheet shows. */
data class Worlds(
    val instance: String,
    val worlds: List<World>,
    val error: String?,
) {
    companion object {
        fun load(): Worlds {
            val reply = act("worlds")
            val root = reply.ok as? JSONObject
            return Worlds(
                instance = root?.optString("instance").orEmpty(),
                worlds = root?.optJSONArray("worlds").list {
                    World(
                        folder = it.optString("folder"),
                        name = it.optString("name"),
                        mode = it.optString("mode"),
                        version = it.text("version"),
                        lastPlayed = if (it.isNull("last_played")) null else it.optLong("last_played"),
                        icon = it.text("icon"),
                        inUse = it.optBoolean("in_use"),
                        metadata = it.optBoolean("metadata"),
                    )
                }.orEmpty(),
                error = reply.error,
            )
        }
    }
}

/** One instance: a folder with its own worlds, settings and add-ons. */
data class Profile(
    val id: Int,
    val name: String,
    val active: Boolean,
    val deletable: Boolean,
    val missing: Boolean,
    val minecraft: String,
    val lastPlayed: Long?,
    val sizeBytes: Long,
    val worlds: Int,
    val mods: Int,
    val packs: Int,
    val shaders: Int,
) {
    companion object {
        fun load(): List<Profile> = (act("profiles").ok as? JSONArray).list {
            Profile(
                id = it.optInt("id"),
                name = it.optString("name"),
                active = it.optBoolean("active"),
                deletable = it.optBoolean("deletable"),
                missing = it.optBoolean("missing"),
                minecraft = it.optString("minecraft"),
                lastPlayed = if (it.isNull("last_played")) null else it.optLong("last_played"),
                sizeBytes = it.optLong("size_bytes"),
                worlds = it.optInt("worlds"),
                mods = it.optInt("mods"),
                packs = it.optInt("packs"),
                shaders = it.optInt("shaders"),
            )
        }
    }
}
