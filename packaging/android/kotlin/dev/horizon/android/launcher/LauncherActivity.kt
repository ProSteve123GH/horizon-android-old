package dev.horizon.android.launcher

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import dev.horizon.android.HorizonActivity
import dev.horizon.android.NativeBridge
import dev.horizon.android.NativeMods
import java.io.File

/**
 * The launcher players see first: Jetpack Compose over the Rust launcher logic.
 * The game itself runs in [HorizonActivity] in the separate `:game` process.
 */
class LauncherActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        copyEngineData()
        NativeBridge.start(
            this,
            filesDir.absolutePath,
            NativeMods.stamp(this),
            runCatching { packageManager.getPackageInfo(packageName, 0).versionName }.getOrNull(),
        )
        setContent {
            LauncherApp(
                onPlay = ::startGame,
                onImport = { kind, uri -> NativeBridge.importDocument(this, kind, uri) },
                onClassicLauncher = ::startClassicLauncher,
            )
        }
    }

    override fun onResume() {
        super.onResume()
        // Coming back from the game: reload settings, content and the last error.
        NativeBridge.call("resume", "")
        // A native-mod build that was set aside after failing to start.
        NativeMods.takeNotice(this)?.let { NativeBridge.call("notice", it) }
    }

    private fun startGame() {
        startActivity(Intent(this, HorizonActivity::class.java))
    }

    /**
     * Opens the Rust launcher as a fallback for any page the Compose launcher
     * cannot reach. Start it without a pending play request so its pages remain
     * reachable.
     */
    private fun startClassicLauncher() {
        File(filesDir, "launch-request.json").delete()
        startActivity(Intent(this, HorizonActivity::class.java))
    }

    /** The engine tables ship inside the APK; the installer reads them from app storage. */
    private fun copyEngineData() {
        val target = File(filesDir, "engine-data.zip")
        val updated = packageManager.getPackageInfo(packageName, 0).lastUpdateTime
        if (target.isFile && target.lastModified() >= updated) return
        runCatching {
            val partial = File(filesDir, "engine-data.partial")
            assets.open("engine-data.zip").use { input ->
                partial.outputStream().use { input.copyTo(it) }
            }
            if (!partial.renameTo(target)) partial.delete()
        }
    }
}
