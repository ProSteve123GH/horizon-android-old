package dev.horizon.android.launcher

import android.app.Application

/**
 * Created in every process. CrashHandler gates itself to the launcher process,
 * leaving the Rust game process to its own panic/tombstone hooks.
 */
class HorizonApplication : Application() {
    override fun onCreate() {
        // Install first so crashes during the rest of startup are covered.
        CrashHandler.install(this)
        super.onCreate()
    }
}
