package dev.horizon.android.launcher

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.platform.LocalContext
import dev.horizon.android.HorizonActivity
import dev.horizon.android.NativeBridge
import java.io.File

/**
 * The launcher players see first: Jetpack Compose over the Rust launcher logic.
 * The game itself runs in [HorizonActivity] in the separate `:game` process.
 *
 * All-files access is mandatory: the shared game folder is the only storage
 * mode. Until it is granted we show [StorageAccessGate] and do not start the
 * native bridge, so no native call ever sees a half-configured launcher.
 */
class LauncherActivity : ComponentActivity() {
    private val granted = mutableStateOf(false)
    // State, not a local val: the gate and the launcher are two different branches
    // of the same composition, and the notice must be consumed exactly once.
    private val crashNotice = mutableStateOf<String?>(null)
    private var bridgeStarted = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        copyEngineData()
        // Read the notice unconditionally, before any grant check: a crash from the
        // previous run must be shown as soon as the launcher composes, whether the
        // storage gate is up or the native bridge is already available.
        crashNotice.value = CrashHandler.pendingReport(this)
        granted.value = hasAllFilesAccess(this)
        if (granted.value) startBridge()
        setContent {
            if (granted.value) {
                LauncherApp(
                    onPlay = ::startGame,
                    onImport = { kind, uri -> NativeBridge.importDocument(this, kind, uri) },
                    onImportNative = { uri -> NativeBridge.importNativeDocument(this, uri) },
                    crashNotice = crashNotice.value,
                )
            } else {
                val context = LocalContext.current
                LaunchedEffect(Unit) {
                    crashNotice.value?.let { notice ->
                        Toast.makeText(context, notice, Toast.LENGTH_LONG).show()
                        CrashHandler.clear(context)
                        crashNotice.value = null
                    }
                }
                StorageAccessGate(onRecheck = ::recheck)
            }
        }
    }

    override fun onResume() {
        super.onResume()
        recheck()
    }

    /** Re-check the grant after returning from the system settings page. */
    private fun recheck() {
        val now = hasAllFilesAccess(this)
        if (now && !bridgeStarted) startBridge()
        granted.value = now
        // The launcher's own "resume" reload only makes sense once the bridge is up.
        if (now) NativeBridge.call("resume", "")
    }

    /** NativeBridge.start registers the Activity and prepares the launcher once. */
    private fun startBridge() {
        if (bridgeStarted) return
        NativeBridge.start(this, filesDir.absolutePath)
        bridgeStarted = true
    }

    private fun startGame() {
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
