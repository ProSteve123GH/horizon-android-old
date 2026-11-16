package dev.horizon.android

import android.content.Context
import android.util.Log
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.IOException

/**
 * The game's data root. It used to be `getFilesDir()` (app-private internal storage),
 * which the system Files app can only reach through Horizon's DocumentsProvider. The
 * app-specific external folder is a real folder on shared storage, so worlds, packs,
 * mods, configuration and reports can be listed, moved and deleted without the provider
 * having to become a general file manager, and without MANAGE_EXTERNAL_STORAGE.
 *
 * Android may not provide external storage (for example while it is being ejected); the
 * internal folder stays the fallback. A one-time copy moves game data from the old
 * internal folder; the old copy is left in place so a failed migration never loses a
 * world.
 */
object HorizonHome {
    private const val TAG = "HorizonHome"
    private const val MARKER = ".horizon-home"
    private val LOCK = Any()
    private val NOT_GAME_DATA = setOf(
        "engine-data.zip",
        "engine-data.partial",
        "launch-request.json",
    )

    /** The data root for this context, with old internal game data copied there once. */
    @JvmStatic
    fun ensure(context: Context): File {
        synchronized(LOCK) {
            val internal = context.filesDir
            val external = try {
                context.getExternalFilesDir(null)
            } catch (error: Throwable) {
                Log.w(TAG, "External app storage is unavailable", error)
                null
            }
            if (external == null || (!external.isDirectory && !external.mkdirs())) {
                return internal
            }
            val marker = File(external, MARKER)
            if (!marker.isFile) {
                // Copy what the old internal root has and the external root does not.
                // Running it on every start until it finishes also resumes a copy interrupted
                // by a process death; an existing file is never overwritten. A failed or
                // interrupted copy keeps the old internal root as the data root, so the game
                // still starts with its complete data.
                if (!copyMissing(internal, external)) {
                    return internal
                }
                try {
                    if (!marker.createNewFile() && !marker.isFile) {
                        Log.w(TAG, "Cannot mark ${external.absolutePath} as the data root")
                    }
                } catch (error: IOException) {
                    Log.w(TAG, "Cannot mark ${external.absolutePath} as the data root", error)
                }
            }
            return external
        }
    }

    private fun copyMissing(source: File, target: File): Boolean {
        if (!source.isDirectory) return true
        if (!target.isDirectory && !target.mkdirs()) return false
        val children = source.listFiles() ?: return true
        var complete = true
        for (child in children) {
            val name = child.name
            if (name.startsWith(".") || name in NOT_GAME_DATA || child.isHidden) continue
            val destination = File(target, name)
            if (child.isDirectory) {
                if (!copyMissing(child, destination)) complete = false
            } else if (child.isFile && !destination.exists()) {
                if (!copyFile(child, destination)) complete = false
            }
        }
        return complete
    }

    private fun copyFile(source: File, destination: File): Boolean {
        val parent = destination.parentFile
        if (parent != null && !parent.isDirectory && !parent.mkdirs()) return false
        // A process death mid-copy must not leave a truncated world file that the next
        // migration then mistakes for a complete copy. The temporary file is renamed only
        // after its bytes have all been written.
        val partial = File(parent ?: File("."), destination.name + ".partial")
        partial.delete()
        try {
            FileInputStream(source).use { input ->
                FileOutputStream(partial).use { output ->
                    val buffer = ByteArray(64 * 1024)
                    while (true) {
                        val read = input.read(buffer)
                        if (read == -1) break
                        output.write(buffer, 0, read)
                    }
                }
            }
        } catch (error: IOException) {
            partial.delete()
            Log.w(TAG, "Cannot copy ${source.absolutePath}", error)
            return false
        }
        if (partial.renameTo(destination)) return true
        partial.delete()
        Log.w(TAG, "Cannot finish copying ${source.absolutePath}")
        return false
    }
}
