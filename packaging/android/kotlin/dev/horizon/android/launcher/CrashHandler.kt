package dev.horizon.android.launcher

import android.app.ActivityManager
import android.app.Application
import android.content.Context
import android.os.Build
import android.os.Process
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Last-resort crash reporting for the Kotlin launcher process.
 *
 * The Rust game process already has its own panic hook and native-tombstone
 * collection, so this handler is installed only in the launcher (default)
 * process; the two never write a report for the same failure. Reports are
 * bounded, redacted and written synchronously to private app storage. This
 * code never calls back into Rust/NativeBridge, never uses coroutines and
 * guards against recursive handling.
 */
object CrashHandler {
    private const val REPORT_PREFIX = "kotlin-crash-"
    private const val MAX_REPORTS = 4
    private const val MAX_REPORT_BYTES = 64 * 1024
    private const val MAX_STACK_LINES = 64
    private const val MAX_STACK_BYTES = 8 * 1024
    private const val MAX_FIELD_CHARS = 2048
    private const val MARKER = "kotlin-crash-pending.marker"

    // Short, fixed notice. Kotlin has no i18n layer today, matching the rest
    // of the launcher. It never exposes stack details in the UI.
    private const val NOTICE =
        "A previous launch crashed. Diagnostics are available in Settings."

    private val SECRET = Regex("token|password|authorization|session")
    private val ABSOLUTE_PATH = Regex("""/(?:data|storage|sdcard)/[^\s)]*""")
    private val URL = Regex("""[a-z][a-z0-9+.-]*://[^\s)]*""")
    private val IP = Regex("""\b\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?\b""")

    private val installed = AtomicBoolean(false)
    private val handling = AtomicBoolean(false)
    private var previous: Thread.UncaughtExceptionHandler? = null

    /** Install once per process; never allowed to break startup. */
    fun install(app: Application) {
        if (!installed.compareAndSet(false, true)) return
        try {
            if (!isLauncherProcess(app)) return
            previous = Thread.getDefaultUncaughtExceptionHandler()
            Thread.setDefaultUncaughtExceptionHandler { thread, error -> handle(thread, error, app) }
        } catch (t: Throwable) {
            // Crash reporting must never prevent the app from starting.
        }
    }

    /**
     * Only the default (launcher) process owns Kotlin reports. An unknown
     * process name is treated as the launcher; any named child process such as
     * the game's ":game" is left to the Rust hooks.
     */
    private fun isLauncherProcess(app: Application): Boolean = try {
        val pid = Process.myPid()
        val manager = app.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        val name = manager?.runningAppProcesses?.firstOrNull { it.pid == pid }?.processName
        name == null || name == app.packageName
    } catch (t: Throwable) {
        true
    }

    private fun handle(thread: Thread, error: Throwable, app: Application) {
        if (!handling.compareAndSet(false, true)) {
            Runtime.getRuntime().halt(1)
            return
        }
        try {
            writeReport(thread, error, app)
        } catch (t: Throwable) {
            // Writing is best effort; never mask the original failure.
        } finally {
            val prev = previous
            if (prev != null) {
                try {
                    prev.uncaughtException(thread, error)
                } catch (t: Throwable) {
                    // Fall through to the hard stop below.
                }
                // A system handler should not return; stop hard if it does.
                Runtime.getRuntime().halt(1)
            } else {
                Runtime.getRuntime().halt(1)
            }
        }
    }

    private fun writeReport(thread: Thread, error: Throwable, app: Application) {
        val dir = File(app.filesDir, "performance")
        if (!dir.isDirectory && !dir.mkdirs()) return

        // If an existing report already grew beyond the budget, stop writing:
        // something is already wrong and we must not fill the device.
        val existing = reports(dir)
        if (existing.any { it.length() > MAX_REPORT_BYTES }) return

        val now = System.currentTimeMillis()
        val lines = ArrayList<String>()
        add(lines, "Horizon Android Kotlin launcher crash report")
        add(lines, "utc_ms: $now")
        add(lines, "thread: ${field(thread.name)}")
        add(lines, "throwable: ${field(error.javaClass.name)}")
        add(lines, "message: ${field(error.message)}")
        add(lines, "version: ${field(versionName(app))}")
        add(lines, "sdk: ${Build.VERSION.SDK_INT}")
        add(lines, "abi: ${field(Build.SUPPORTED_ABIS.firstOrNull() ?: "unknown")}")
        add(lines, "stack:")

        // Bound by characters but keep the line structure intact.
        val rawStack = error.stackTraceToString()
        val stack = if (rawStack.length > MAX_STACK_BYTES) rawStack.substring(0, MAX_STACK_BYTES) else rawStack
        var used = 0
        var count = 0
        for (line in stack.lineSequence()) {
            if (count >= MAX_STACK_LINES) break
            val safe = redact(line) ?: continue
            if (used + safe.length + 1 > MAX_STACK_BYTES) break
            lines.add(safe)
            used += safe.length + 1
            count++
        }

        val bytes = (lines.joinToString("\n") + "\n").toByteArray(Charsets.UTF_8)
        if (bytes.size > MAX_REPORT_BYTES) return
        val file = File(dir, "$REPORT_PREFIX$now.txt")
        val out = FileOutputStream(file)
        try {
            out.write(bytes)
            out.flush()
            try {
                out.fd.sync()
            } catch (t: Throwable) {
                // sync is best effort on some filesystems.
            }
        } finally {
            try {
                out.close()
            } catch (t: Throwable) {
                // ignore
            }
        }

        // Arm the one-time notice for the next launcher launch.
        try {
            val marker = FileOutputStream(File(app.filesDir, MARKER))
            try {
                marker.write(1)
            } finally {
                marker.close()
            }
        } catch (t: Throwable) {
            // ignore
        }
        rotate(dir)
    }

    /** Append one sanitized line, dropping secret-bearing lines entirely. */
    private fun add(lines: MutableList<String>, raw: String) {
        val safe = redact(raw) ?: return
        lines.add(safe)
    }

    /** null means the line must be dropped. */
    private fun redact(line: String): String? {
        if (SECRET.containsMatchIn(line.lowercase())) return null
        var out = ABSOLUTE_PATH.replace(line, "[redacted]")
        out = URL.replace(out, "[redacted]")
        out = IP.replace(out, "[redacted]")
        return out
    }

    /** Bound a single field and keep it on one line. */
    private fun field(value: String?, limit: Int = MAX_FIELD_CHARS): String {
        if (value == null) return ""
        val flat = value.replace('\n', ' ').replace('\r', ' ')
        return if (flat.length > limit) flat.substring(0, limit) + "…(truncated)" else flat
    }

    private fun versionName(app: Application): String = try {
        app.packageManager.getPackageInfo(app.packageName, 0).versionName ?: "unknown"
    } catch (t: Throwable) {
        "unknown"
    }

    private fun reports(dir: File): Array<File> =
        dir.listFiles { f -> f.name.startsWith(REPORT_PREFIX) && f.name.endsWith(".txt") }
            ?: emptyArray()

    /** Newest UTC-ms names sort last; keep at most MAX_REPORTS. */
    private fun rotate(dir: File) {
        val sorted = reports(dir).sortedBy { it.name }
        val excess = sorted.size - MAX_REPORTS
        if (excess <= 0) return
        for (file in sorted.take(excess)) {
            try {
                file.delete()
            } catch (t: Throwable) {
                // ignore
            }
        }
    }

    /** A short notice on the next launch, or null when no crash was recorded. */
    fun pendingReport(context: Context): String? = try {
        if (File(context.filesDir, MARKER).isFile) NOTICE else null
    } catch (t: Throwable) {
        null
    }

    /** Consume the notice. Called by the UI once it has been shown. */
    fun clear(context: Context) {
        try {
            File(context.filesDir, MARKER).delete()
        } catch (t: Throwable) {
            // ignore
        }
    }
}
