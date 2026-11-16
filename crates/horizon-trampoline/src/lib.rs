//! Horizon's loader shim: the library `NativeActivity` loads (`android.app.lib_name`).
//!
//! Java (`HorizonActivity`) validates `files/native/current.json` on every start and
//! writes `files/native/active.json` naming the library for this process — an
//! installed modded build, or the stock game. This shim reads `active.json`, sets
//! the environment the crash guard watches for, calls the named library with
//! `dlopen`, and runs its `android_main`. It holds no game code and no policy of
//! its own: choosing the library is Java's job, running the game is the game's.

use android_activity::AndroidApp;
use std::ffi::CString;

/// Read the `active.json` text and return the library path it names.
fn parse_active(text: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("native/active.json is not JSON: {error}"))?;
    let lib = value
        .get("lib")
        .and_then(serde_json::Value::as_str)
        .filter(|lib| !lib.is_empty())
        .ok_or_else(|| "native/active.json has no lib".to_string())?;
    Ok(lib.to_owned())
}

/// `NativeActivity` entry point (`ANativeActivity_onCreate`) comes from
/// `android-activity`, which then calls this with the same signature the game
/// library exports.
#[unsafe(no_mangle)]
pub extern "C" fn android_main(app: AndroidApp) {
    let Some(files_dir) = app.internal_data_path() else {
        fatal("Android supplied no application data directory");
    };
    let active = files_dir.join("native").join("active.json");
    let lib = match std::fs::read_to_string(&active) {
        Ok(text) => match parse_active(&text) {
            Ok(lib) => lib,
            Err(error) => fatal(&error),
        },
        Err(_) => fatal("native/active.json is missing or unreadable"),
    };
    // The game clears `pending_crash` on its first `about_to_wait` when these
    // are present. Set them at the very top of this thread; nothing else reads
    // or writes them concurrently.
    // SAFETY: no other thread reads or writes these variables at this point.
    unsafe {
        std::env::set_var("HORIZON_GAME_LIB", &lib);
        std::env::set_var(
            "HORIZON_NATIVE_PENDING",
            files_dir.join("native").join("pending_crash"),
        );
    }
    let Ok(c_lib) = CString::new(lib) else {
        fatal("native/active.json lib contains an interior NUL");
    };
    // The game library is already loaded by `HorizonActivity` via `System.load`
    // before `super.onCreate`, so this resolves to the same image (refcount
    // only); the initializers are never run twice.
    let handle = unsafe { libc::dlopen(c_lib.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        fatal("dlopen failed for the chosen game library");
    }
    let entry = unsafe { libc::dlsym(handle, c"android_main".as_ptr()) };
    if entry.is_null() {
        fatal("the chosen game library does not export android_main");
    }
    // SAME signature the game library exports (crates/horizon-client/src/android_main.rs).
    let game: extern "C" fn(AndroidApp) = unsafe { std::mem::transmute(entry) };
    game(app);
}

fn fatal(message: &str) -> ! {
    // No logging framework is initialized at this point; the message lands in
    // the process's stderr and the abort keeps a broken install from hanging
    // silently on a black window.
    eprintln!("horizon-trampoline: {message}");
    std::process::abort();
}

#[cfg(test)]
mod tests {
    use super::parse_active;

    #[test]
    fn parses_the_lib_path() {
        let lib = parse_active(r#"{"lib": "/data/user/0/dev.horizon.android/files/native/builds/a/1.0.0/libhorizon_android.so"}"#)
            .expect("valid active.json parses");
        assert_eq!(
            lib,
            "/data/user/0/dev.horizon.android/files/native/builds/a/1.0.0/libhorizon_android.so"
        );
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_active("{not json").is_err());
    }

    #[test]
    fn rejects_a_missing_lib() {
        assert!(parse_active(r#"{"lib": ""}"#).is_err());
        assert!(parse_active(r#"{"other": "x"}"#).is_err());
        assert!(parse_active(r#"{"lib": 42}"#).is_err());
    }

    #[test]
    fn tolerates_extra_fields() {
        let lib = parse_active(r#"{"lib": "/stock/libhorizon_android.so", "id": "x"}"#).expect("extra fields are fine");
        assert_eq!(lib, "/stock/libhorizon_android.so");
    }
}
