//! Entry for the `:game` process (`android.app.func_name=horizon_on_create`).
//!
//! Reads `files/native/active.json` — written by `HorizonActivity` on every
//! start, naming the chosen library (an installed modded build, else the stock
//! game) — and forwards Android's own entry-point call to that library's
//! `ANativeActivity_onCreate`. The chosen game library then runs through the
//! same entry glue every stock build uses: its `android-activity` glue
//! initializes its jni-rs singleton, spawns its Rust thread and attaches it,
//! and only then `android_main` runs. This shim carries no JNI machinery and
//! no game code; the only difference from a stock device is which library
//! answers the call.
//!
//! Why not `android_main` directly, like before: the game library only
//! initializes its own jni-rs singleton inside its own `ANativeActivity_onCreate`
//! path. Calling its `android_main` skipped that, `vm_as_ptr().unwrap()` then
//! panicked through an unwinding `Global`-ref drop and the process aborted
//! before the first frame ("panic in a destructor during cleanup"). Forwarding
//! the framework's raw call instead keeps every game path identical to stock.

use std::ffi::{CStr, CString, OsString};
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use ndk_sys::ANativeActivity;

type OnCreate = unsafe extern "C" fn(*mut ANativeActivity, *mut std::ffi::c_void, usize);

#[unsafe(no_mangle)]
pub extern "C" fn horizon_on_create(
    activity: *mut ANativeActivity,
    saved_state: *mut std::ffi::c_void,
    saved_state_size: usize,
) {
    if let Err(error) = unsafe { dispatch(activity, saved_state, saved_state_size) } {
        eprintln!("horizon-trampoline: {error}");
        unsafe { libc::abort() }
    }
}

unsafe fn dispatch(
    activity: *mut ANativeActivity,
    saved_state: *mut std::ffi::c_void,
    saved_state_size: usize,
) -> Result<(), String> {
    let root = unsafe { files_dir(activity)? };
    let active = std::fs::read_to_string(root.join("native").join("active.json"))
        .map_err(|error| format!("native/active.json unreadable: {error}"))?;
    let (lib, fallback) = parse_active(&active)?;
    // The game clears the crash-guard markers on its first frame when these
    // are present. The layout of the process is unchanged from a stock start.
    unsafe {
        std::env::set_var("HORIZON_GAME_LIB", &lib);
        std::env::set_var(
            "HORIZON_NATIVE_PENDING",
            root.join("native").join("pending_crash"),
        );
    }
    match unsafe { call_game(&lib, activity, saved_state, saved_state_size) } {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("horizon-trampoline: {error}; falling back to {fallback}");
            unsafe { call_game(&fallback, activity, saved_state, saved_state_size) }
        }
    }
}

unsafe fn call_game(
    lib: &str,
    activity: *mut ANativeActivity,
    saved_state: *mut std::ffi::c_void,
    saved_state_size: usize,
) -> Result<(), String> {
    let path = CString::new(lib).map_err(|_| format!("nul in path {lib:?}"))?;
    let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        return Err(format!("dlopen failed for {lib}"));
    }
    let entry = unsafe { libc::dlsym(handle, c"ANativeActivity_onCreate".as_ptr()) };
    if entry.is_null() {
        return Err(format!("{lib} exports no ANativeActivity_onCreate"));
    }
    let game: OnCreate = unsafe { std::mem::transmute(entry) };
    unsafe { game(activity, saved_state, saved_state_size) };
    Ok(())
}

unsafe fn files_dir(activity: *mut ANativeActivity) -> Result<PathBuf, String> {
    let path = unsafe { (*activity).internalDataPath };
    if path.is_null() {
        return Err("ANativeActivity has no internalDataPath".to_string());
    }
    let bytes = unsafe { CStr::from_ptr(path) }.to_bytes().to_vec();
    Ok(PathBuf::from(OsString::from_vec(bytes)))
}

/// `active.json` — Java (`HorizonActivity`) always writes `lib`; `stock` as
/// fallback when both are known. Missing `stock` means the game is loaded
/// once, no fallback (the same APK's library is both).
fn parse_active(text: &str) -> Result<(String, String), String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("native/active.json is not JSON: {error}"))?;
    let lib = value
        .get("lib")
        .and_then(|lib| lib.as_str())
        .filter(|lib| !lib.is_empty())
        .ok_or_else(|| "native/active.json has no lib".to_string())?;
    let fallback = value
        .get("stock")
        .and_then(|stock| stock.as_str())
        .unwrap_or(lib)
        .to_string();
    Ok((lib.to_string(), fallback))
}

#[cfg(test)]
mod tests {
    use super::parse_active;

    #[test]
    fn parses_lib_and_stock() {
        let (lib, stock) = parse_active(r#"{"lib": "/lib/chosen.so", "stock": "/lib/stock.so"}"#).unwrap();
        assert_eq!(lib, "/lib/chosen.so");
        assert_eq!(stock, "/lib/stock.so");
    }

    #[test]
    fn falls_back_to_lib_when_stock_absent() {
        let (lib, stock) = parse_active(r#"{"lib": "/lib/stock.so"}"#).unwrap();
        assert_eq!(lib, stock);
    }

    #[test]
    fn rejects_bad_shapes() {
        assert!(parse_active(r#"{"other": 1}"#).is_err());
        assert!(parse_active(r#"{"lib": ""}"#).is_err());
        assert!(parse_active("{nope").is_err());
    }
}
