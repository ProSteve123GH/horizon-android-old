package dev.horizon.android

import android.content.Context
import android.util.Log
import java.io.File

/**
 * The native-mod build the game loads in place of the APK's own library.
 *
 * `crates/horizon-client/src/android/native_builds.rs` writes `files/native/compiled-<key>/`
 * (the library, `for`, `apk`, `build.json`) and `files/native/current`. This object is the
 * game's side: it picks that library when it was made for this version name and this install
 * of the APK, and counts the starts that never reached a first frame, so a build that crashes
 * the game is set aside instead of locking the player out.
 */
object NativeMods {
    private const val DIR = "native"
    private const val CURRENT = "current"
    private const val APK = "apk"
    private const val FOR = "for"
    private const val STARTS = "starts"
    private const val NOTICE = "notice"
    private const val LIBRARY = "libhorizon_android.so"
    private const val COMPILED = "compiled-"
    private const val MOST_STARTS = 2

    /** This install of the APK (`PackageManager.lastUpdateTime`). */
    fun stamp(context: Context): String? = runCatching {
        context.packageManager.getPackageInfo(context.packageName, 0).lastUpdateTime.toString()
    }.getOrNull()

    /**
     * The library to load for this start, or null for the APK's own. A build made for another
     * version name or install, or one that failed to start [MOST_STARTS] times, is not used.
     * Each start counts, and `started` ends the count when the game shows its first frame: a
     * build that closes before then is one that does not work.
     */
    fun chosen(context: Context, version: String?): File? {
        val dir = File(context.filesDir, DIR)
        val name = text(File(dir, CURRENT)).orEmpty()
        if (name.isEmpty() || !plain(name)) return null
        val build = File(dir, name)
        val library = File(build, LIBRARY)
        if (!library.isFile) return null
        if (text(File(build, FOR)) != version) return null
        val stamp = stamp(context) ?: return null
        if (text(File(build, APK)) != stamp) return null
        val starts = text(File(build, STARTS))?.toIntOrNull() ?: 0
        if (starts >= MOST_STARTS) {
            setAside(context, "The native mod build closed $starts times before it showed; the APK's own game plays.")
            return null
        }
        runCatching { File(build, STARTS).writeText((starts + 1).toString()) }
        return library
    }

    /** The game reached its first frame: the build it runs works. */
    fun started(context: Context) {
        val dir = File(context.filesDir, DIR)
        val name = text(File(dir, CURRENT)).orEmpty()
        if (name.isEmpty()) return
        File(File(dir, name), STARTS).delete()
        File(dir, NOTICE).delete()
    }

    /** The build could not be loaded: it cannot work, so the APK's own game plays from now on. */
    fun failed(context: Context, reason: String?) {
        setAside(
            context,
            reason?.takeIf { it.isNotBlank() }?.let { "The native mod build could not be loaded ($it); the APK's own game plays." }
                ?: "The native mod build could not be loaded; the APK's own game plays.",
        )
    }

    /** The launcher's notice about a build, read once. */
    fun takeNotice(context: Context): String? {
        val file = File(context.filesDir, DIR).resolve(NOTICE)
        val text = text(file) ?: return null
        file.delete()
        return text
    }

    private fun setAside(context: Context, reason: String) {
        val dir = File(context.filesDir, DIR)
        runCatching { File(dir, NOTICE).writeText(reason) }
        File(dir, CURRENT).delete()
        Log.w("Horizon", reason)
    }

    private fun text(file: File): String? =
        runCatching { file.readText().trim() }.getOrNull()?.takeIf { it.isNotEmpty() }

    /** As `native_builds::plain_build`: a build's own folder name, never a path. */
    private fun plain(name: String): Boolean =
        name.length <= 80 && name.startsWith(COMPILED) &&
            name.all { it in 'a'..'z' || it in '0'..'9' || it == '-' || it == '_' }
}
