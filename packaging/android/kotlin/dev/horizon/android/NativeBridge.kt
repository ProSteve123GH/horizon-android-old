package dev.horizon.android

import android.app.Activity

/** The Rust launcher's state and actions (`crates/horizon-client/src/android/bridge.rs`). */
object NativeBridge {
    init {
        System.loadLibrary("horizon_android")
    }

    /** Registers [activity] for Android services and prepares the launcher once. */
    @JvmStatic external fun start(activity: Activity, home: String)

    /** The whole launcher state as JSON; call from the main thread. */
    @JvmStatic external fun snapshot(): String?

    /** Runs a launcher action: `{"ok": …}` or `{"error": "…"}`. */
    @JvmStatic external fun call(method: String, arg: String): String?

    /** Adds a picked PNG to the local skin or cape gallery. */
    @JvmStatic external fun importWardrobe(kind: String, title: String, bytes: ByteArray)

    /** Hands a picked document to the content importer. */
    @JvmStatic external fun importDocument(activity: Activity, kind: String, uri: String?)

    /** Reuses Minecraft 26.3 from a SAF folder tree. The Activity persists the URI grant. */
    @JvmStatic external fun importMinecraftFolder(uri: String?)

    /** Reuses Minecraft 26.3 from a picked ZIP without writing a second copy. */
    @JvmStatic external fun importMinecraftZip(uri: String?)
}
