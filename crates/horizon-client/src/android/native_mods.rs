//! Native mod (`.hnmod`) import, listing and removal for the Android launcher.
//!
//! A `.hnmod` bundle is source-only Rust that hooks the game's own functions.
//! It has to be compiled into the game before it can run, and this build of
//! Horizon cannot compile one on the device: import validates and stores the
//! bundle, and that is all. Nothing here fetches, executes or enables mod code;
//! a mod runs only when an external build service produces a modded library and
//! the user installs it. `files/native/` (app-private) holds the loader state
//! the game process reads at start; bundles live with the game content.
use horizon_mods::index::native::Bundle;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BUNDLE: u64 = 64 << 20;

/// Bundles sit in the active instance next to `horizon-mods/`.
pub(crate) fn bundle_dir(game_home: &Path) -> PathBuf {
    game_home.join("horizon-native-mods")
}

/// The loader state, app-private: the same folder `HorizonActivity` reads.
fn priv_native(home: &Path) -> PathBuf {
    home.join("native")
}

// ---- result channel for the launcher's status snackbar --------------------

static RESULT: Mutex<Option<String>> = Mutex::new(None);

fn finish(message: impl Into<String>) {
    *RESULT.lock().unwrap_or_else(|error| error.into_inner()) = Some(message.into());
}

/// Drain the latest import outcome; `Controller::snapshot` turns it into the
/// status line the Compose launcher shows as a snackbar.
pub(crate) fn take_result() -> Option<String> {
    RESULT
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take()
}

// ---- import ----------------------------------------------------------------

/// `NativeBridge.importNativeDocument(Activity, String uri)`: the picked
/// `.hnmod` document, opened like every other imported file.
pub(crate) fn import_external(
    env: &mut jni::JNIEnv<'_>,
    activity: &jni::objects::JObject<'_>,
    game_home: PathBuf,
    uri: Option<String>,
) {
    if crate::documents::busy() {
        finish("Finish or cancel the current import first");
        return;
    }
    let setup = (|| -> jni::errors::Result<_> {
        let activity = env.new_global_ref(activity)?;
        Ok((env.get_java_vm()?, activity, uri))
    })();
    let Ok((vm, activity, uri)) = setup else {
        let _ = env.exception_clear();
        finish("Could not read the selected document");
        return;
    };
    let Some(text) = uri else {
        // The picker was dismissed.
        return;
    };
    if !text.starts_with("content://") || text.len() > 8192 {
        finish("Unsupported document URI");
        return;
    }
    crate::documents::begin_import();
    crate::documents::note("Checking the picked .hnmod bundle…", false);
    let spawned = std::thread::Builder::new()
        .name("Native mod import".into())
        .spawn(move || {
            let result = import_worker(&vm, &activity, &text, &game_home);
            crate::documents::set_working(false);
            match result {
                Ok((id, version)) => finish(format!(
                    "Imported native mod {id} {version}. It is built and run only once this build of Horizon has a mod build service."
                )),
                Err(error) => finish(format!("Native mod import failed: {error}")),
            }
        });
    if let Err(error) = spawned {
        crate::documents::set_working(false);
        finish(format!("Native mod import failed: {error}"));
    }
}

fn import_worker(
    vm: &jni::JavaVM,
    activity: &jni::objects::GlobalRef<jni::objects::JObject<'_>>,
    uri: &str,
    game_home: &Path,
) -> Result<(String, String), String> {
    let (input, _name) = (|| -> Result<_, String> {
        let mut env = vm.attach_current_thread().map_err(|error| error.to_string())?;
        let opened = crate::document_bridge::open(&mut env, activity.as_obj(), uri, "import.hnmod");
        if opened.is_err() {
            let _ = env.exception_clear();
        }
        opened.map_err(|error| error.to_string())
    })()?;
    let cache = crate::platform::cache_dir().ok_or("Could not access the Android import cache")?;
    let dir = cache.join("native-imports");
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    if fs::symlink_metadata(&dir)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Refusing a redirected import cache".into());
    }
    // Drop partials from a previous interrupted import.
    for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stale =
            name.starts_with(".import-") && name.ends_with(".part") && entry.file_type().is_ok_and(|t| t.is_file());
        if stale {
            let _ = fs::remove_file(entry.path());
        }
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let staged = dir.join(format!(".import-{stamp}.part"));
    let result = (|| -> Result<(String, String), String> {
        let mut input = input;
        let mut output = fs::File::create(&staged).map_err(|error| error.to_string())?;
        let mut buffer = [0u8; 65536];
        let mut total = 0u64;
        loop {
            if crate::documents::cancel_requested() {
                return Err("Import cancelled".into());
            }
            let read = input.read(&mut buffer).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            total += read as u64;
            if total > MAX_BUNDLE {
                return Err("Native bundle is larger than 64 MiB".into());
            }
            output.write_all(&buffer[..read]).map_err(|error| error.to_string())?;
        }
        drop(output);
        let bundle = Bundle::read(&staged).map_err(|error| format!("not a valid .hnmod bundle: {error}"))?;
        let id = bundle.manifest.id.clone();
        let version = bundle.manifest.version.clone();
        drop(bundle);
        let target_dir = bundle_dir(game_home);
        fs::create_dir_all(&target_dir).map_err(|error| error.to_string())?;
        if fs::symlink_metadata(&target_dir)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Refusing a redirected native-mods folder".into());
        }
        let target = target_dir.join(format!("{id}.hnmod"));
        if target.exists() {
            return Err(format!("a native mod with id '{id}' is installed; remove it first"));
        }
        // The cache and the instance can be on different filesystems (app
        // storage and shared storage), so a plain rename can fail with EXDEV.
        match fs::rename(&staged, &target) {
            Ok(()) => {}
            Err(rename_error) => {
                if fs::copy(&staged, &target).is_ok() {
                    let _ = fs::remove_file(&staged);
                } else {
                    let _ = fs::remove_file(&target);
                    return Err(format!(
                        "could not store the bundle: rename failed ({rename_error}); copy failed"
                    ));
                }
            }
        }
        Ok((id, version))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result
}

// ---- removal -----------------------------------------------------------------

/// `native_remove`: delete the stored bundle. The installed build, if any, is
/// left alone; it only loads on its own APK version anyway.
pub(crate) fn remove(game_home: &Path, id: &str) -> Result<(), String> {
    if !horizon_mods::index::valid_id(id) {
        return Err("Invalid native mod id".into());
    }
    let file = bundle_dir(game_home).join(format!("{id}.hnmod"));
    match fs::symlink_metadata(&file) {
        Ok(metadata) if metadata.is_file() => {
            fs::remove_file(&file).map_err(|error| format!("could not remove {id}: {error}"))
        }
        _ => Err(format!("No native mod with id '{id}' is installed")),
    }
}

/// `native_disable`: forget any selected modded build. The next start loads
/// the stock game. Bundles are kept.
pub(crate) fn disable(home: &Path) -> Result<(), String> {
    let dir = priv_native(home);
    for name in ["current.json", "active.json", "pending_crash", "crash_streak"] {
        let _ = fs::remove_file(dir.join(name));
    }
    Ok(())
}

// ---- listing -----------------------------------------------------------------

/// The snapshot's `native` section: stored bundles and the selected build.
/// Cached by a fingerprint of the folder and the loader state; the snapshot
/// is rebuilt often.
pub(crate) fn section(home: &Path, game_home: &Path) -> Value {
    let dir = bundle_dir(game_home);
    let mut fingerprint = Vec::new();
    if let Ok(read) = fs::read_dir(&dir) {
        let mut rows: Vec<(String, u64, u128)> = read
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".hnmod")?;
                let metadata = entry.metadata().ok()?;
                if !metadata.is_file() {
                    return None;
                }
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                Some((name, metadata.len(), modified))
            })
            .collect();
        rows.sort();
        for (name, len, modified) in rows {
            fingerprint.extend(name.as_bytes());
            fingerprint.extend(len.to_le_bytes());
            fingerprint.extend(modified.to_le_bytes());
            fingerprint.push(0);
        }
    }
    for name in ["current.json", "current.json.rejected", "last_rejected"] {
        match fs::metadata(priv_native(home).join(name)) {
            Ok(metadata) => {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                fingerprint.extend(name.as_bytes());
                fingerprint.extend(metadata.len().to_le_bytes());
                fingerprint.extend(modified.to_le_bytes());
            }
            Err(_) => fingerprint.extend(name.as_bytes()),
        }
        fingerprint.push(0);
    }
    let mut cache = LIST_CACHE.lock().unwrap_or_else(|error| error.into_inner());
    if cache.fingerprint != fingerprint {
        cache.value = Some(section_uncached(home, game_home));
        cache.fingerprint = fingerprint;
    }
    cache.value.clone().unwrap_or_else(|| json!({ "mods": [], "current": Value::Null }))
}

struct ListCache {
    fingerprint: Vec<u8>,
    value: Option<Value>,
}
static LIST_CACHE: Mutex<ListCache> = Mutex::new(ListCache {
    fingerprint: Vec::new(),
    value: None,
});

fn section_uncached(home: &Path, game_home: &Path) -> Value {
    let current = current(home);
    let current_id = current.get("id").and_then(Value::as_str).map(str::to_owned);
    let current_rejected = current.get("rejected").and_then(Value::as_bool).unwrap_or(false);
    let dir = bundle_dir(game_home);
    let mut rows = Vec::new();
    if let Ok(read) = fs::read_dir(&dir) {
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".hnmod") else {
                continue;
            };
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let file_type = metadata.file_type();
            if !file_type.is_file() {
                continue;
            }
            rows.push((stem.to_owned(), entry.path(), metadata.len()));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let mods: Vec<Value> = rows
        .into_iter()
        .map(|(stem, path, size)| match Bundle::read(&path) {
            Ok(bundle) => {
                let id = bundle.manifest.id.clone();
                let state = match &current_id {
                    Some(selected) if selected == &id => {
                        if current_rejected {
                            "rejected"
                        } else {
                            "selected"
                        }
                    }
                    _ => "imported",
                };
                let name = id.clone();
                json!({
                    "id": id,
                    "version": bundle.manifest.version,
                    "name": name,
                    "size": size,
                    "enabled_state": state,
                })
            }
            // A bundle that no longer parses stays listed instead of vanishing.
            Err(_) => json!({
                "id": stem,
                "version": "unknown",
                "name": stem,
                "size": size,
                "enabled_state": "invalid",
            }),
        })
        .collect();
    json!({ "mods": mods, "current": current })
}

fn current(home: &Path) -> Value {
    let dir = priv_native(home);
    if let Some(value) = read_json(&dir.join("current.json")) {
        return current_value(&value, false, None);
    }
    if dir.join("current.json.rejected").is_file() {
        let value = read_json(&dir.join("current.json.rejected")).unwrap_or(Value::Null);
        let reason = fs::read_to_string(dir.join("last_rejected"))
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| {
                "A modded build was set aside after crashing before its first frame. Playing the stock game."
                    .to_owned()
            });
        return current_value(&value, true, Some(reason));
    }
    Value::Null
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn current_value(value: &Value, rejected: bool, reason: Option<String>) -> Value {
    let text = |key| value.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
    let mut out = json!({
        "id": text("id"),
        "version": text("version"),
        "versionCode": value.get("versionCode").and_then(Value::as_i64).unwrap_or_default(),
        "abi": text("abi"),
        "rejected": rejected,
    });
    if let Some(reason) = reason {
        out["reject_reason"] = json!(reason);
    }
    out
}
