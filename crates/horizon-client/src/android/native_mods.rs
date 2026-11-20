//! Native mod (`.hnmod` / `.hmod`) import, listing, building and removal for the Android launcher.
//!
//! A `.hnmod` bundle is source-only Rust that hooks the game's own functions.
//! A combined `.hmod` carries the same native code plus a content part; it is
//! stored in both folders so the existing content installer/list sees it.
//! Native code is compiled into the game and runs unsandboxed — it is never
//! treated like a sandboxed content mod. Compiling cannot happen on the device:
//! the launcher sends the enabled bundles to the personal build
//! service for this fork (`BUILD_BASE`), the service compiles the game with
//! the mod's hooks on a hosted runner and publishes
//! `/v1/job/<job>` and `/v1/job/<job>/lib`. The launcher streams the library
//! back, verifies it against the job's SHA-256 and publishes
//! `files/native/current.json`, which the Java loader
//! (`HorizonActivity`) reads before the next start. Nothing here executes
//! mod code; a mod runs only as part of a build the loader accepts.
//! `files/native/` (app-private) holds the loader state the game process
//! reads at start; bundles live with the game content.
use horizon_mods::index::native::Bundle;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

// TODO(keep-consistency): the desktop launcher's shared native-mod browser reads build records
// written by its local Rust toolchain. Android builds through the hosted service above instead,
// so it has neither a `native-mods.json` nor a GitHub origin for a build; the shared launcher
// modules need these names to compile, and report "no native build" on this platform.
/// The folder the desktop launcher also uses for `.hnmod` bundles.
pub const INSTALLED: &str = "horizon-native-mods";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Client,
    Server,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LeftOut {
    pub id: String,
    pub version: String,
    pub why: String,
}

impl LeftOut {
    pub fn label(&self) -> String {
        String::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct About {
    pub base: String,
    pub horizon: String,
    pub target: Target,
    pub base_horizon: Option<String>,
    pub pinned_by: Vec<String>,
    pub on_base: bool,
    pub left_out: Vec<LeftOut>,
}

impl About {
    pub fn off_its_nightly(&self) -> bool {
        false
    }
}

/// No desktop-style native build record exists on Android.
pub fn about(_version: &str) -> Option<About> {
    None
}

pub fn is_native(version: &str) -> bool {
    version.starts_with("native-")
}

pub fn base_for(_current: Option<&str>, _installed: &[String], _update: bool) -> Option<String> {
    None
}

pub fn revision_of(_version: &str) -> Result<String, String> {
    Err("Android has no desktop native build record".into())
}

pub fn hooks_at(_revision: &str) -> Option<u32> {
    None
}

pub fn digest<'a>(_files: impl Iterator<Item = (&'a str, &'a [u8])>) -> String {
    String::new()
}

pub(crate) const MAX_BUNDLE: u64 = 64 << 20;
/// The build service accepts at most 32 MiB; refuse to start the upload
/// beyond a comfortable margin.
pub(crate) const MAX_BUNDLE_TOTAL: u64 = 30 << 20;
/// Hard cap for the assembled `mods.bin` body.
pub(crate) const MAX_MODS_BIN: usize = 32 << 20;
const BUILD_VERSION: &str = "0.7.7";
const BUILD_BASE: &str =
    "https://horizon-android-mod-builds.steve-horizon-android-hnmod.workers.dev";
const POLL_SECS: u64 = 3;
const BUILD_TIMEOUT_SECS: u64 = 45 * 60;
const MAX_CONSECUTIVE_POLL_ERRORS: u32 = 8;

/// Bundles sit in the active instance next to `horizon-mods/`.
pub(crate) fn bundle_dir(game_home: &Path) -> PathBuf {
    game_home.join("horizon-native-mods")
}

/// Storage extensions: `.hnmod` is native-only, `.hmod` also carries content
/// (the desktop launcher's combined format). Both are accepted everywhere.
const BUNDLE_EXTENSIONS: [&str; 2] = ["hnmod", "hmod"];

/// The path a bundle with `extension` is stored under, keeping its format.
fn bundle_file(game_home: &Path, id: &str, extension: &str) -> PathBuf {
    bundle_dir(game_home).join(format!("{id}.{extension}"))
}

/// Split a stored file name into `(id, extension)` for either bundle format.
fn bundle_stem(name: &str) -> Option<(&str, &str)> {
    BUNDLE_EXTENSIONS.iter().find_map(|&extension| {
        name.strip_suffix(extension)
            .and_then(|stem| stem.strip_suffix('.'))
            .map(|stem| (stem, extension))
    })
}

/// The stored path for `id`, whichever of the two extensions is present.
fn stored_bundle(game_home: &Path, id: &str) -> Option<PathBuf> {
    BUNDLE_EXTENSIONS
        .iter()
        .map(|&extension| bundle_file(game_home, id, extension))
        .find(|path| path.is_file())
}

/// The extension to store a staged bundle under. `.hnmod` and `.hmod` keep
/// their own name; the importers' internal `.part`/`.partial` staging names
/// follow the payload. Anything else is rejected rather than guessed.
fn staged_extension(staged: &Path, has_content: bool) -> Result<&'static str, String> {
    match staged.extension().and_then(|ext| ext.to_str()) {
        Some("hmod") => Ok("hmod"),
        Some("hnmod") => Ok("hnmod"),
        Some("part") | Some("partial") | None => Ok(if has_content { "hmod" } else { "hnmod" }),
        Some(other) => Err(format!("unsupported native bundle extension '.{other}'")),
    }
}

/// `Bundle::read` only exposes a combined bundle's content when the file it
/// opens ends in `.hmod`. Importers stage under `.part`/`.partial`, so read
/// through a same-directory `.hmod` alias to keep validation (including the
/// content part) identical for every path. The bytes themselves are unchanged.
fn read_staged_bundle(staged: &Path) -> Result<Bundle, String> {
    let name = staged
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let alias = staged.with_file_name(format!(".{name}.view.hmod"));
    if let Ok(metadata) = fs::symlink_metadata(&alias) {
        // A leftover from an interrupted import is ours to clean up; a
        // symlink or directory at that name must never be followed.
        if !metadata.is_file() {
            return Err("Refusing a redirected bundle inspection path".into());
        }
        let _ = fs::remove_file(&alias);
    }
    if fs::hard_link(staged, &alias).is_err() {
        // O_EXCL: a symlink planted in between must not redirect the copy.
        let mut input = fs::File::open(staged).map_err(|error| error.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&alias)
            .map_err(|error| error.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|error| error.to_string())?;
    }
    let result = Bundle::read(&alias);
    let _ = fs::remove_file(&alias);
    result
}

/// Copy `source` to `destination` without ever replacing an existing file or
/// following a symlink at the destination.
fn copy_new(source: &Path, destination: &Path) -> Result<(), String> {
    let mut input = fs::File::open(source).map_err(|error| error.to_string())?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| error.to_string())?;
    std::io::copy(&mut input, &mut output).map_err(|error| error.to_string())?;
    Ok(())
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
    crate::documents::note("Checking the picked .hmod / .hnmod bundle…", false);
    let spawned = std::thread::Builder::new()
        .name("Native mod import".into())
        .spawn(move || {
            let result = import_worker(&vm, &activity, &text, &game_home);
            crate::documents::set_working(false);
            match result {
                Ok((id, version)) => finish(format!(
                    "Imported native mod {id} {version}. Native code is compiled into the game and runs unsandboxed; it only runs in a build the mod build service publishes."
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
    activity: &jni::objects::GlobalRef,
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
        install_staged(game_home, &staged)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result
}

/// Validate a staged bundle file and store it as
/// `<game_home>/horizon-native-mods/<id>.hnmod` or `<id>.hmod`, keeping the
/// staged format. A combined `.hmod` is also mirrored to
/// `<game_home>/horizon-mods/<id>.hmod` so the existing content installer and
/// list see it; neither destination is ever overwritten.
/// Shared by the SAF (`import_external`) and HTTPS (`catalog::import_native`) import paths.
/// Returns `(id, version)`; refuses bad bundles and an id that is already installed.
pub(crate) fn install_staged(
    game_home: &Path,
    staged: &Path,
) -> Result<(String, String), String> {
    let bundle = read_staged_bundle(staged)
        .map_err(|error| format!("not a valid .hmod / .hnmod bundle: {error}"))?;
    let id = bundle.manifest.id.clone();
    let version = bundle.manifest.version.clone();
    let has_content = !bundle.content.is_empty();
    drop(bundle);
    let extension = staged_extension(staged, has_content)?;
    let target_dir = bundle_dir(game_home);
    fs::create_dir_all(&target_dir).map_err(|error| error.to_string())?;
    if fs::symlink_metadata(&target_dir)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Refusing a redirected native-mods folder".into());
    }
    if stored_bundle(game_home, &id).is_some() {
        return Err(format!("a native mod with id '{id}' is installed; remove it first"));
    }
    let target = bundle_file(game_home, &id, extension);
    // A combined bundle is content as well. Refuse an id collision before
    // writing anything, and never replace an existing content mod.
    let mirror = if has_content {
        let content_dir = game_home.join("horizon-mods");
        fs::create_dir_all(&content_dir).map_err(|error| error.to_string())?;
        if fs::symlink_metadata(&content_dir)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Refusing a redirected mods folder".into());
        }
        let mirror = content_dir.join(format!("{id}.hmod"));
        if fs::symlink_metadata(&mirror).is_ok() {
            return Err(format!("a content mod with id '{id}' is installed; remove it first"));
        }
        Some(mirror)
    } else {
        None
    };
    // The cache and the instance can be on different filesystems (app
    // storage and shared storage), so a plain rename can fail with EXDEV.
    match fs::rename(staged, &target) {
        Ok(()) => {}
        Err(rename_error) => {
            if fs::copy(staged, &target).is_ok() {
                let _ = fs::remove_file(staged);
            } else {
                let _ = fs::remove_file(&target);
                return Err(format!(
                    "could not store the bundle: rename failed ({rename_error}); copy failed"
                ));
            }
        }
    }
    if let Some(mirror) = mirror {
        if let Err(error) = copy_new(&target, &mirror) {
            // Roll back both files: a half-imported combined mod is worse
            // than no import at all, and would block the next attempt.
            let _ = fs::remove_file(&mirror);
            let _ = fs::remove_file(&target);
            return Err(format!("could not mirror the content part: {error}"));
        }
    }
    Ok((id, version))
}

// ---- removal -----------------------------------------------------------------

/// `native_remove`: delete the stored bundle (either extension) and the
/// mirrored content file of a combined `.hmod`, if present. The installed
/// build, if any, is left alone; it only loads on its own APK version anyway.
pub(crate) fn remove(game_home: &Path, id: &str) -> Result<(), String> {
    if !horizon_mods::index::valid_id(id) {
        return Err("Invalid native mod id".into());
    }
    let mut removed = false;
    for &extension in &BUNDLE_EXTENSIONS {
        let file = bundle_file(game_home, id, extension);
        match fs::symlink_metadata(&file) {
            Ok(metadata) if metadata.is_file() => {
                fs::remove_file(&file).map_err(|error| format!("could not remove {id}: {error}"))?;
                removed = true;
            }
            Ok(_) => return Err(format!("Refusing to remove a redirected native mod with id '{id}'")),
            Err(_) => {}
        }
    }
    if !removed {
        return Err(format!("No native mod with id '{id}' is installed"));
    }
    // Only combined bundles have a mirror, and we never remove content for an
    // id that has no native bundle installed.
    let mirror = game_home.join("horizon-mods").join(format!("{id}.hmod"));
    if let Ok(metadata) = fs::symlink_metadata(&mirror) {
        if metadata.is_file() {
            fs::remove_file(&mirror).map_err(|error| format!("could not remove {id}: {error}"))?;
        }
    }
    Ok(())
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

// ---- enabled flags ---------------------------------------------------

fn enabled_file(game_home: &Path) -> PathBuf {
    bundle_dir(game_home).join("enabled.json")
}

/// Read the enabled list. A missing file means every bundle is enabled
/// (the default). Unknown ids (bundles that were removed) are ignored.
fn enabled_set(game_home: &Path) -> Vec<String> {
    let Some(value) = read_json(&enabled_file(game_home)) else {
        return Vec::new();
    };
    value
        .get("enabled")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .filter(|id| horizon_mods::index::valid_id(id))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Write the enabled list atomically (`.tmp` + rename), keeping only ids
/// that have a bundle on disk.
fn write_enabled(game_home: &Path, ids: &[String]) -> Result<(), String> {
    let dir = bundle_dir(game_home);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let mut ids: Vec<String> = ids
        .iter()
        .filter(|id| stored_bundle(game_home, id).is_some())
        .cloned()
        .collect();
    ids.sort();
    ids.dedup();
    let body = json!({ "enabled": ids }).to_string();
    let target = enabled_file(game_home);
    let temp = target.with_file_name("enabled.json.tmp");
    fs::write(&temp, body).map_err(|error| error.to_string())?;
    fs::rename(&temp, &target).map_err(|error| error.to_string())
}

/// `native_enabled`: flip whether `id` takes part in **Compile**.
pub(crate) fn set_enabled(
    game_home: &Path,
    id: &str,
    enabled: bool,
) -> Result<bool, String> {
    if !horizon_mods::index::valid_id(id) {
        return Err("Invalid native mod id".into());
    }
    if stored_bundle(game_home, id).is_none() {
        return Err(format!("No native mod with id '{id}' is installed"));
    }
    let mut set = enabled_set(game_home);
    if !enabled_file(game_home).is_file() {
        // Materialize the default (all enabled) before applying the change.
        set = list_ids(game_home);
    }
    let present = set.iter().position(|entry| entry == id).is_some();
    match (enabled, present) {
        (true, false) => set.push(id.to_owned()),
        (false, true) => {
            set.retain(|entry| entry != id);
        }
        _ => {}
    }
    write_enabled(game_home, &set)?;
    Ok(enabled)
}

/// The ids of every stored bundle.
fn list_ids(game_home: &Path) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(bundle_dir(game_home))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let (stem, _) = bundle_stem(&name)?;
            if !horizon_mods::index::valid_id(stem) {
                return None;
            }
            entry.metadata().ok().filter(|m| m.is_file()).map(|_| stem.to_owned())
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
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
                bundle_stem(&name)?;
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
    match fs::metadata(enabled_file(game_home)) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            fingerprint.extend(b"enabled.json");
            fingerprint.extend(metadata.len().to_le_bytes());
            fingerprint.extend(modified.to_le_bytes());
        }
        Err(_) => fingerprint.extend(b"enabled.json"),
    }
    fingerprint.push(0);
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
    // The build state changes on its own schedule, so it is never
    // cached: a fresh read at the 200 ms snapshot cadence.
    let mut value = cache
        .value
        .clone()
        .unwrap_or_else(|| json!({ "mods": [], "current": Value::Null }));
    if let Some(object) = value.as_object_mut() {
        object.insert("build".to_string(), build_state_json());
    }
    value
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
            let Some((stem, _)) = bundle_stem(&name) else {
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
    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    // One row per id even if both formats are present for it.
    rows.dedup_by(|a, b| a.0 == b.0);
    let enabled = enabled_set(game_home);
    let all_enabled = !enabled_file(game_home).is_file();
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
                    "enabled": all_enabled || enabled.iter().any(|entry| entry == &id),
                })
            }
            // A bundle that no longer parses stays listed instead of vanishing.
            Err(_) => json!({
                "id": stem,
                "version": "unknown",
                "name": stem,
                "size": size,
                "enabled_state": "invalid",
                "enabled": false,
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
    let mod_ids: Vec<String> = value
        .get("mods")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|entry| entry.get("id").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    out["mods"] = json!(mod_ids);
    out
}

// ---- the mod build service -------------------------------------------

#[derive(Clone, PartialEq, Eq)]
enum BuildPhase {
    Idle,
    Requesting,
    Queued,
    Building,
    Installing,
    /// The built library passed verification and `current.json` is written.
    Ready,
    Failed,
}

#[derive(Clone)]
struct BuildMod {
    id: String,
    version: String,
    sha256: String,
}

struct BuildState {
    phase: BuildPhase,
    job: Option<String>,
    cached: bool,
    ahead: Option<i64>,
    seconds: Option<i64>,
    sha256: Option<String>,
    size: Option<u64>,
    error: Option<String>,
    /// The service says the job settled (it will not change state again).
    settled: bool,
    downloaded: u64,
    install_total: Option<u64>,
    /// The failure happened while fetching/verifying the library:
    /// downloading again is meaningful, building again is not.
    install_failed: bool,
    mods: Vec<BuildMod>,
    version_code: u64,
    abi: String,
    stock: Option<String>,
}

static BUILD: Mutex<BuildState> = Mutex::new(BuildState {
    phase: BuildPhase::Idle,
    job: None,
    cached: false,
    ahead: None,
    seconds: None,
    sha256: None,
    size: None,
    error: None,
    settled: false,
    downloaded: 0,
    install_total: None,
    install_failed: false,
    mods: Vec::new(),
    version_code: 0,
    abi: String::new(),
    stock: None,
});

static RESUMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn build_lock() -> std::sync::MutexGuard<'static, BuildState> {
    BUILD.lock().unwrap_or_else(|error| error.into_inner())
}

fn phase_name(phase: &BuildPhase) -> &'static str {
    match phase {
        BuildPhase::Idle => "idle",
        BuildPhase::Requesting => "requesting",
        BuildPhase::Queued => "queued",
        BuildPhase::Building => "building",
        BuildPhase::Installing => "installing",
        BuildPhase::Ready => "ready",
        BuildPhase::Failed => "failed",
    }
}

fn phase_busy(phase: &BuildPhase) -> bool {
    matches!(
        phase,
        BuildPhase::Requesting | BuildPhase::Queued | BuildPhase::Building | BuildPhase::Installing
    )
}

/// The snapshot's `build` section. Never blocks: a fresh read of the shared
/// state at the 200 ms snapshot cadence.
pub(crate) fn build_state_json() -> Value {
    let state = build_lock();
    json!({
        "state": phase_name(&state.phase),
        "job": state.job,
        "cached": state.cached,
        "ahead": state.ahead,
        "seconds": state.seconds,
        "sha256": state.sha256,
        "size": state.size,
        "error": state.error,
        "settled": state.settled,
        "downloaded": state.downloaded,
        "installTotal": state.install_total,
        "installFailed": state.install_failed,
        "mods": state.mods.iter().map(|m| json!({
            "id": m.id,
            "version": m.version,
            "sha256": m.sha256,
        })).collect::<Vec<_>>(),
        "versionCode": state.version_code,
        "abi": state.abi,
    })
}

/// `native_compile_status`: the last known state of the build. The poller
/// thread keeps it current; this call does no network I/O.
pub(crate) fn compile_status() -> Value {
    build_state_json()
}

// ---- `mods.bin` packing -----------------------------------------------

/// One bundle plus what `mods.json` names for it.
pub(crate) struct ModBundle {
    pub id: String,
    pub version: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}

/// Pack the selected bundles into the `mods.bin` body the build service
/// expects: a `mods.json` manifest plus one `<id>.hnmod` entry per bundle.
pub(crate) fn pack_mods_bin(files: &[ModBundle]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut body);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        let entries: Vec<Value> = files
            .iter()
            .map(|file| {
                json!({
                    "id": file.id,
                    "version": file.version,
                    "file": format!("{}.hnmod", file.id),
                    "sha256": file.sha256,
                })
            })
            .collect();
        zip.start_file("mods.json", options)
            .map_err(|error| error.to_string())?;
        zip.write_all(json!({ "files": entries }).to_string().as_bytes())
            .map_err(|error| error.to_string())?;
        for file in files {
            zip.start_file(format!("{}.hnmod", file.id), options)
                .map_err(|error| error.to_string())?;
            zip.write_all(&file.bytes)
                .map_err(|error| error.to_string())?;
        }
        zip.finish().map_err(|error| error.to_string())?;
    }
    if body.len() > MAX_MODS_BIN {
        return Err(format!(
            "the build archive is {} MiB; the build service accepts at most 32 MiB",
            body.len() / (1024 * 1024)
        ));
    }
    Ok(body)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// ---- build state persistence -------------------------------------------

fn build_file(home: &Path) -> PathBuf {
    priv_native(home).join("build.json")
}

fn persist_locked(home: &Path, state: &BuildState) {
    if matches!(state.phase, BuildPhase::Idle | BuildPhase::Ready) {
        let _ = fs::remove_file(build_file(home));
        return;
    }
    let body = json!({
        "phase": phase_name(&state.phase),
        "job": state.job,
        "cached": state.cached,
        "sha256": state.sha256,
        "size": state.size,
        "error": state.error,
        "installFailed": state.install_failed,
        "mods": state.mods.iter().map(|m| json!({
            "id": m.id,
            "version": m.version,
            "sha256": m.sha256,
        })).collect::<Vec<_>>(),
        "versionCode": state.version_code,
        "abi": state.abi,
        "stock": state.stock,
    });
    let dir = priv_native(home);
    if fs::create_dir_all(&dir).is_ok() {
        let temp = dir.join("build.json.tmp");
        if fs::write(&temp, body.to_string()).is_ok() {
            let _ = fs::rename(&temp, build_file(home));
        }
    }
}

fn persist(home: &Path) {
    let state = build_lock();
    persist_locked(home, &state);
}

/// Forget the in-flight build after a successful install or when the caller
/// wants a clean slate.
fn forget_build(home: &Path) {
    let _ = fs::remove_file(build_file(home));
}

/// Raise a terminal failure; `install_failed` marks that only the download
/// part failed and `native_install` can retry it.
fn set_failed(home: &Path, message: String, install_failed: bool) {
    let mut state = build_lock();
    state.phase = BuildPhase::Failed;
    state.error = Some(message);
    state.install_failed = install_failed;
    state.ahead = None;
    state.seconds = None;
    persist_locked(home, &state);
}

/// Restore a persisted build after a launcher restart. Resumes the poller
/// or the installer; never re-sends a build request.
pub(crate) fn resume_build(home: &Path) {
    if RESUMED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Some(saved) = read_json(&build_file(home)) else {
        return;
    };
    let phase = saved["phase"].as_str().unwrap_or("idle").to_owned();
    if phase == "idle" || phase == "ready" {
        let _ = fs::remove_file(build_file(home));
        return;
    }
    let job = saved["job"].as_str().map(str::to_owned);
    let mods: Vec<BuildMod> = saved["mods"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| {
                    Some(BuildMod {
                        id: entry["id"].as_str()?.to_owned(),
                        version: entry["version"].as_str()?.to_owned(),
                        sha256: entry["sha256"].as_str()?.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let version_code = saved["versionCode"].as_u64().unwrap_or(0);
    let abi = saved["abi"].as_str().unwrap_or("").to_owned();
    let stock = saved["stock"].as_str().map(str::to_owned);
    let sha256 = saved["sha256"].as_str().map(str::to_owned);
    let size = saved["size"].as_u64();
    let error = saved["error"].as_str().map(str::to_owned);
    let install_failed = saved["installFailed"].as_bool().unwrap_or(false);
    {
        let mut state = build_lock();
        if state.phase != BuildPhase::Idle {
            // The user already acted; the persisted file is stale.
            return;
        }
        state.job = job.clone();
        state.sha256 = sha256;
        state.size = size;
        state.mods = mods;
        state.version_code = version_code;
        state.abi = abi;
        state.stock = stock;
        state.error = error;
        state.install_failed = install_failed;
        state.phase = match phase.as_str() {
            "requesting" => BuildPhase::Failed, // the request never reached the service
            "queued" | "building" => BuildPhase::Queued,
            "installing" => BuildPhase::Installing,
            "done" => BuildPhase::Installing, // service said done; the install never finished
            "failed" => BuildPhase::Failed,
            _ => BuildPhase::Idle,
        };
        if phase == "requesting" {
            state.error = Some("The build request was interrupted; compile again.".to_owned());
        }
    }
    persist(home);
    let resume_install = build_lock().phase == BuildPhase::Installing && build_lock().job.is_some();
    let resume_poll = build_lock().phase == BuildPhase::Queued && build_lock().job.is_some();
    if resume_install {
        std::thread::Builder::new()
            .name("Native mod install".into())
            .spawn({
                let home = home.to_path_buf();
                move || install_from_state(&home)
            })
            .ok();
    } else if resume_poll {
        std::thread::Builder::new()
            .name("Native mod build".into())
            .spawn({
                let home = home.to_path_buf();
                move || poll_build(&home)
            })
            .ok();
    }
}

// ---- network -----------------------------------------------------------

fn agent(timeout_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .https_only(true)
        .timeout_connect(std::time::Duration::from_secs(20))
        .timeout_read(std::time::Duration::from_secs(timeout_secs))
        .build()
}

/// Parse a 4xx/5xx body into a message; only 4xx/5xx reaches this path
/// because `ureq` turns them into `Error::Status`.
fn error_message(code: u16, body: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    if let Some(message) = parsed
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(Value::as_str)
        .filter(|message| !message.is_empty())
    {
        return message.to_owned();
    }
    format!("the build service answered HTTP {code}")
}

struct JobStatus {
    state: String,
    ahead: Option<i64>,
    seconds: Option<i64>,
    sha256: Option<String>,
    size: Option<u64>,
    error: Option<String>,
    settled: bool,
}

/// `GET /v1/job/<job>` → the service's status document, or `None` when the
/// answer is definitively not parseable (a 4xx is terminal, a transport
/// error is returned as `Err` so the poller can retry).
fn poll_job(job: &str) -> Result<JobStatus, String> {
    let url = format!("{BUILD_BASE}/v1/job/{job}");
    let result = agent(15).get(&url).call();
    let body = match result {
        Ok(response) => response.into_string().map_err(|error| error.to_string())?,
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            // A 4xx is terminal: the job is unknown or malformed. Report it
            // as a status so the poller stops instead of counting strikes.
            return Ok(JobStatus {
                state: "failed".to_owned(),
                ahead: None,
                seconds: None,
                sha256: None,
                size: None,
                error: Some(error_message(code, &body)),
                settled: true,
            });
        }
        Err(ureq::Error::Transport(transport)) => {
            return Err(format!("cannot reach the build service: {transport}"));
        }
    };
    let value: Value = serde_json::from_str(&body).map_err(|_| "the build service answered with an unreadable status".to_string())?;
    let state = value
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    Ok(JobStatus {
        ahead: value.get("ahead").and_then(Value::as_i64),
        seconds: value.get("seconds").and_then(Value::as_i64),
        sha256: value
            .get("sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        size: value.get("size").and_then(Value::as_u64),
        error: value.get("error").and_then(Value::as_str).map(str::to_owned),
        state,
        settled: value.get("settled").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn poll_build(home: &Path) {
    let job = {
        let state = build_lock();
        state.job.clone()
    };
    let Some(job) = job else {
        set_failed(home, "the build job is missing from the state".to_owned(), false);
        return;
    };
    let start = Instant::now();
    let mut consecutive_errors = 0u32;
    loop {
        if start.elapsed().as_secs() > BUILD_TIMEOUT_SECS {
            set_failed(
                home,
                "the build service did not finish within 45 minutes; try again".to_owned(),
                false,
            );
            return;
        }
        std::thread::sleep(Duration::from_secs(POLL_SECS));
        let status = match poll_job(&job) {
            Ok(status) => status,
            Err(message) => {
                consecutive_errors += 1;
                if consecutive_errors >= MAX_CONSECUTIVE_POLL_ERRORS {
                    set_failed(home, format!("lost contact with the build service: {message}"), false);
                    return;
                }
                continue;
            }
        };
        consecutive_errors = 0;
        let done = status.state == "done";
        let failed = status.state == "failed";
        {
            let mut state = build_lock();
            state.ahead = status.ahead;
            state.seconds = status.seconds;
            state.sha256 = status.sha256.clone();
            state.size = status.size;
            state.error = status.error.clone();
            state.settled = status.settled;
            state.phase = if done {
                BuildPhase::Installing
            } else if failed {
                BuildPhase::Failed
            } else if status.state == "building" {
                BuildPhase::Building
            } else {
                BuildPhase::Queued
            };
            persist_locked(home, &state);
        }
        if done {
            install_from_state(home);
            return;
        }
        if failed {
            return;
        }
    }
}

// ---- `native_compile` ---------------------------------------------------

/// The ids worth building from a Compile request: valid ids only, sorted and
/// deduplicated. `None` when the request has no `enabled` array or when every
/// submitted id is invalid — the caller reports the existing "no mods
/// selected" error instead of starting a build with nothing in it.
pub(crate) fn selected_ids(args: &Value) -> Option<Vec<String>> {
    let enabled = args.get("enabled").and_then(Value::as_array)?;
    let mut ids: Vec<String> = enabled
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| horizon_mods::index::valid_id(id))
        .map(str::to_owned)
        .collect();
    ids.sort();
    ids.dedup();
    (!ids.is_empty()).then_some(ids)
}

/// Gather the enabled bundles, pack them and POST the archive to the build
/// service. The POST happens on a worker thread: the launcher snapshot loop
/// is the progress channel.
pub(crate) fn compile(
    home: &Path,
    game_home: &Path,
    arg: &str,
) -> Result<Value, String> {
    if phase_busy(&build_lock().phase) {
        return Err("A build is already in progress. Wait for it to finish, or retry after it fails.".into());
    }
    let args: Value = serde_json::from_str(arg).map_err(|_| {
        "Expected a JSON object with the enabled mods, the APK version code and the ABI".to_string()
    })?;
    let ids = selected_ids(&args)
        .ok_or("No mods selected: enable at least one native mod to compile")?;
    let version_code = args
        .get("versionCode")
        .and_then(Value::as_u64)
        .filter(|code| *code >= 1)
        .ok_or("Missing the APK version code; the build must target this exact APK")?;
    let abi = args
        .get("abi")
        .and_then(Value::as_str)
        .filter(|abi| !abi.is_empty() && abi.len() <= 32)
        .unwrap_or("arm64-v8a")
        .to_owned();
    let stock = args
        .get("stock")
        .and_then(Value::as_str)
        .filter(|stock| !stock.is_empty() && Path::new(stock).is_absolute())
        .map(str::to_owned);
    // The bundles on disk are the input the service sees; enabled ids must exist.
    let mut bundles: Vec<ModBundle> = Vec::with_capacity(ids.len());
    let mut total: u64 = 0;
    for id in &ids {
        let path = stored_bundle(game_home, id)
            .ok_or_else(|| format!("{id}: the bundle is missing from the game files folder"))?;
        if fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(format!("{id}: the stored bundle is redirected"));
        }
        let metadata = fs::metadata(&path)
            .map_err(|_| format!("{id}: the bundle is missing from the game files folder"))?;
        if !metadata.is_file() {
            return Err(format!("{id}: the stored bundle is not a file"));
        }
        total += metadata.len();
        if total > MAX_BUNDLE_TOTAL {
            return Err(format!(
                "the selected mods add up to more than {} MiB; the build service accepts at most 32 MiB",
                MAX_BUNDLE_TOTAL >> 20
            ));
        }
        let bundle = Bundle::read(&path)
            .map_err(|error| format!("{id}: {error}"))?;
        let bytes = fs::read(&path)
            .map_err(|error| format!("{id}: {error}"))?;
        bundles.push(ModBundle {
            id: id.clone(),
            version: bundle.manifest.version,
            sha256: sha256_bytes(&bytes),
            bytes,
        });
    }
    let body = pack_mods_bin(&bundles)?;
    {
        let mut state = build_lock();
        *state = BuildState {
            phase: BuildPhase::Requesting,
            job: None,
            cached: false,
            ahead: None,
            seconds: None,
            sha256: None,
            size: None,
            error: None,
            settled: false,
            downloaded: 0,
            install_total: None,
            install_failed: false,
            mods: bundles
                .iter()
                .map(|bundle| BuildMod {
                    id: bundle.id.clone(),
                    version: bundle.version.clone(),
                    sha256: bundle.sha256.clone(),
                })
                .collect(),
            version_code,
            abi,
            stock,
        };
        persist_locked(home, &state);
    }
    std::thread::Builder::new()
        .name("Native mod build".into())
        .spawn({
            let home = home.to_path_buf();
            move || submit_build(&home, body)
        })
        .map_err(|error| format!("cannot start the build worker: {error}"))?;
    Ok(json!({ "started": true }))
}

fn submit_build(home: &Path, body: Vec<u8>) {
    let url = format!("{BUILD_BASE}/v1/build?version={BUILD_VERSION}");
    let result = agent(300)
        .post(&url)
        .set("content-type", "application/octet-stream")
        .send_bytes(&body);
    let job = match result {
        Ok(response) => {
            let body = response.into_string().unwrap_or_default();
            match serde_json::from_str::<Value>(&body) {
                Ok(value) => match (
                    value.get("job").and_then(Value::as_str),
                    value.get("cached").and_then(Value::as_bool).unwrap_or(false),
                ) {
                    (Some(job), cached) if !job.is_empty() => {
                        let mut state = build_lock();
                        state.phase = BuildPhase::Queued;
                        state.job = Some(job.to_owned());
                        state.cached = cached;
                        persist_locked(home, &state);
                        Some(job.to_owned())
                    }
                    _ => {
                        set_failed(home, "the build service did not return a job id".to_owned(), false);
                        None
                    }
                },
                Err(_) => {
                    set_failed(home, "the build service answered with an unreadable response".to_owned(), false);
                    None
                }
            }
        }
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            set_failed(home, error_message(code, &body), false);
            None
        }
        Err(ureq::Error::Transport(transport)) => {
            set_failed(home, format!("cannot reach the build service: {transport}"), false);
            None
        }
    };
    if job.is_some() {
        poll_build(home);
    }
}

// ---- `native_install` ---------------------------------------------------

/// The download URL for a finished job.
fn library_url(job: &str) -> String {
    format!("{BUILD_BASE}/v1/job/{job}/lib")
}

/// The stock library path Java expects in `current.json`: an explicit value
/// (from the launcher, derived from JNI), the persisted one, or the one Java
/// wrote to `active.json` on the last game start.
fn stock_path(home: &Path, explicit: Option<String>) -> Option<String> {
    if let Some(stock) = explicit.filter(|stock| Path::new(stock).is_absolute()) {
        return Some(stock);
    }
    if let Some(stock) = build_lock().stock.clone() {
        if Path::new(&stock).is_absolute() {
            return Some(stock);
        }
    }
    read_json(&priv_native(home).join("active.json"))
        .and_then(|value| value.get("stock").and_then(Value::as_str).map(str::to_owned))
        .filter(|stock| Path::new(stock).is_absolute())
}

/// Download `/v1/job/<job>/lib` into `native/builds/<job>/libhorizon_android.so`.
///
/// Resumes an interrupted download when the service honors `Range: bytes=N-`
/// (HTTP 206 with a matching `Content-Range`); otherwise starts the file over.
/// Verifies SHA-256 at the end against the value the service published for the
/// job; on a mismatch the partial file is deleted and the download is retried
/// from scratch exactly once.
fn download_library(home: &Path, job: &str, expected_sha: &str) -> Result<(PathBuf, String, u64), String> {
    let target = priv_native(home).join(format!("builds/{job}/libhorizon_android.so"));
    let partial = target.with_file_name("libhorizon_android.so.partial");
    let parent = target
        .parent()
        .ok_or_else(|| "builds folder is unreachable".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    if fs::symlink_metadata(parent)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err("Refusing a redirected builds folder".into());
    }
    const MAX_ATTEMPTS: u32 = 2; // one from scratch after a SHA-256 mismatch
    let mut attempt = 0;
    loop {
        attempt += 1;
        let offset = fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
        let url = library_url(job);
        let request = agent(300).get(&url);
        let request = if offset > 0 {
            request.set("Range", &format!("bytes={offset}-"))
        } else {
            request
        };
        let result = request.call();
        let response = match result {
            Ok(response) => response,
            Err(ureq::Error::Status(416, _)) => {
                // A resume offset the service cannot serve: restart cleanly.
                let _ = fs::remove_file(&partial);
                if attempt >= MAX_ATTEMPTS {
                    return Err("the build service cannot serve the library download (HTTP 416)".into());
                }
                continue;
            }
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                return Err(error_message(code, &body));
            }
            Err(ureq::Error::Transport(transport)) => {
                return Err(format!("cannot reach the build service: {transport}"));
            }
        };
        let status = response.status();
        let resume = offset > 0 && status == 206 && {
            let prefix = format!("bytes {offset}-");
            response
                .header("content-range")
                .map(|value| value.starts_with(prefix.as_str()))
                .unwrap_or(false)
        };
        let total = response
            .header("content-length")
            .and_then(|value| value.parse::<u64>().ok())
            .map(|length| if resume { offset + length } else { length })
            .or_else(|| if resume { None } else { build_lock().size });
        {
            let mut state = build_lock();
            state.install_total = total;
            state.downloaded = if resume { offset } else { 0 };
        }
        let mut file = if resume {
            fs::OpenOptions::new()
                .append(true)
                .open(&partial)
                .map_err(|error| format!("cannot append to {}: {error}", partial.display()))?
        } else {
            fs::File::create(&partial)
                .map_err(|error| format!("cannot create {}: {error}", partial.display()))?
        };
        let mut reader = response.into_reader();
        let mut buffer = [0u8; 65536];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| format!("the download was interrupted: {error}"))?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])
                .map_err(|error| format!("cannot write {}: {error}", partial.display()))?;
            let mut state = build_lock();
            state.downloaded = state.downloaded.saturating_add(read as u64);
        }
        drop(file);
        drop(reader);
        let sha256 = sha256_file(&partial)?;
        if sha256 == expected_sha.to_ascii_lowercase() {
            let size = fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
            fs::rename(&partial, &target)
                .map_err(|error| format!("cannot move the library into place: {error}"))?;
            return Ok((target, sha256, size));
        }
        // Hash mismatch: keep nothing. One from-scratch attempt is allowed.
        let _ = fs::remove_file(&partial);
        if attempt >= MAX_ATTEMPTS {
            return Err(
                "downloaded library failed sha256 — retried from scratch once and it still did not match"
                    .into(),
            );
        }
        {
            let mut state = build_lock();
            state.downloaded = 0;
            persist_locked(home, &state);
        }
    }
}

/// Install the finished build: download, verify, then publish `current.json`
/// in exactly the shape the Java loader reads.
pub(crate) fn install_from_state(home: &Path) {
    let (job, expected_sha, size, mods, version_code, abi) = {
        let state = build_lock();
        (
            state.job.clone(),
            state.sha256.clone(),
            state.size,
            state.mods.clone(),
            state.version_code,
            state.abi.clone(),
        )
    };
    let (Some(job), Some(expected_sha)) = (job, expected_sha) else {
        set_failed(home, "the build has not finished yet".to_owned(), true);
        return;
    };
    if phase_busy(&build_lock().phase) && build_lock().phase != BuildPhase::Installing {
        return; // a compile is racing us; let it finish
    }
    {
        let mut state = build_lock();
        state.phase = BuildPhase::Installing;
        state.error = None;
        state.install_failed = false;
        state.downloaded = 0;
        state.install_total = size;
        persist_locked(home, &state);
    }
    let outcome = download_library(home, &job, &expected_sha);
    match outcome {
        Ok((lib, sha256, _len)) => {
            let stock = stock_path(home, None);
            let Some(stock) = stock else {
                set_failed(
                    home,
                    "no stock library path is known: start the stock game once (or rebuild with a fresh APK) so active.json exists".to_owned(),
                    true,
                );
                return;
            };
            if !Path::new(&stock).is_file() {
                set_failed(
                    home,
                    format!("the stock library is missing at {stock}; reinstall the game or contact support before using native mods"),
                    true,
                );
                return;
            }
            let (id, version) = if mods.len() == 1 {
                (mods[0].id.clone(), mods[0].version.clone())
            } else {
                (job.clone(), job.clone())
            };
            let current = json!({
                "id": id,
                "version": version,
                "lib": lib.to_string_lossy(),
                "stock": stock,
                "abi": abi,
                "versionCode": version_code,
                "sha256": sha256,
                "mods": mods.iter().map(|m| json!({
                    "id": m.id,
                    "version": m.version,
                    "sha256": m.sha256,
                })).collect::<Vec<_>>(),
            });
            let dir = priv_native(home);
            let temp = dir.join("current.json.tmp");
            let target = dir.join("current.json");
            let written = fs::write(&temp, current.to_string())
                .and_then(|_| fs::rename(&temp, &target))
                .map_err(|error| format!("cannot publish current.json: {error}"));
            match written {
                Ok(()) => {
                    forget_build(home);
                    let mut state = build_lock();
                    state.phase = BuildPhase::Ready;
                    state.error = None;
                    state.install_failed = false;
                }
                Err(message) => set_failed(home, message, true),
            }
        }
        Err(message) => set_failed(home, message, true),
    }
}

/// `native_install`: fetch the finished build, verify it and publish it.
/// Only valid when the build is done (or only the download failed).
pub(crate) fn install(home: &Path, arg: &str) -> Result<Value, String> {
    let args: Value = serde_json::from_str(arg).unwrap_or_else(|_| json!({}));
    let requested_job = args.get("job").and_then(Value::as_str).map(str::to_owned);
    {
        let state = build_lock();
        if let (Some(requested), Some(current)) = (requested_job.as_ref(), state.job.as_ref()) {
            if requested != current {
                return Err(format!("a different build ({current}) is the one tracked; refusing to install {requested}"));
            }
        }
        match state.phase {
            BuildPhase::Ready => return Err("the build is installed; restart the game to use it".into()),
            BuildPhase::Failed if state.install_failed => {}
            BuildPhase::Installing => return Err("The download is already in progress.".into()),
            _ => {
                return Err(
                    "the build is not finished yet; wait for the build service to report done".into(),
                )
            }
        }
        // Merge explicit JNI-derived values over the persisted ones.
        let mut state = state;
        if let Some(stock) = args
            .get("stock")
            .and_then(Value::as_str)
            .filter(|stock| !stock.is_empty())
        {
            state.stock = Some(stock.to_owned());
        }
        if let Some(version_code) = args.get("versionCode").and_then(Value::as_u64).filter(|v| *v >= 1) {
            state.version_code = version_code;
        }
        if let Some(abi) = args.get("abi").and_then(Value::as_str).filter(|a| !a.is_empty()) {
            state.abi = abi.to_owned();
        }
    }
    std::thread::Builder::new()
        .name("Native mod install".into())
        .spawn({
            let home = home.to_path_buf();
            move || install_from_state(&home)
        })
        .map_err(|error| format!("cannot start the install worker: {error}"))?;
    Ok(json!({ "started": true }))
}

// ---- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    fn bundle(id: &str, version: &str, fill: u8) -> ModBundle {
        let bytes = vec![fill; 4096];
        ModBundle {
            id: id.to_string(),
            version: version.to_string(),
            sha256: sha256_bytes(&bytes),
            bytes,
        }
    }

    /// The archive the build service receives must carry a parseable
    /// `mods.json` whose hashes match the entries that follow it.
    #[test]
    fn mods_bin_round_trips() {
        let files = vec![
            bundle("alpha-mod", "1.2.3", 7),
            bundle("beta_mod", "0.1.0", 0xAB),
        ];
        let body = pack_mods_bin(&files).expect("the archive must pack");
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(&body)).expect("the archive must parse");

        let mut manifest = String::new();
        archive
            .by_name("mods.json")
            .expect("mods.json must be an entry")
            .read_to_string(&mut manifest)
            .expect("mods.json must be readable");
        let doc: Value = serde_json::from_str(&manifest).expect("mods.json must be JSON");
        let entries = doc["files"].as_array().expect("files must be a list");
        assert_eq!(entries.len(), 2);

        for (bundle, entry) in files.iter().zip(entries) {
            assert_eq!(entry["id"].as_str(), Some(bundle.id.as_str()));
            assert_eq!(entry["version"].as_str(), Some(bundle.version.as_str()));
            assert_eq!(
                entry["file"].as_str(),
                Some(format!("{}.hnmod", bundle.id).as_str())
            );
            assert_eq!(entry["sha256"].as_str(), Some(bundle.sha256.as_str()));

            let mut bytes = Vec::new();
            archive
                .by_name(&format!("{}.hnmod", bundle.id))
                .expect("each bundle must be an entry")
                .read_to_end(&mut bytes)
                .expect("each bundle must be readable");
            assert_eq!(bytes, bundle.bytes);
            assert_eq!(sha256_bytes(&bytes), entry["sha256"].as_str().unwrap());
        }
    }

    /// A single enabled mod still produces a valid manifest and one entry.
    #[test]
    fn mods_bin_single_mod() {
        let files = vec![bundle("solo", "9.9.9", 1)];
        let body = pack_mods_bin(&files).expect("the archive must pack");
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(&body)).expect("the archive must parse");
        assert_eq!(archive.len(), 2);
        assert!(archive.by_name("mods.json").is_ok());
        assert!(archive.by_name("solo.hnmod").is_ok());
    }

    /// Both storage extensions resolve to a per-id path and are recognised
    /// when scanning the folder; other suffixes are not bundles.
    #[test]
    fn bundle_paths_cover_both_extensions() {
        let home = Path::new("/game");
        assert_eq!(
            bundle_file(home, "vein_miner", "hnmod"),
            PathBuf::from("/game/horizon-native-mods/vein_miner.hnmod")
        );
        assert_eq!(
            bundle_file(home, "vein_miner", "hmod"),
            PathBuf::from("/game/horizon-native-mods/vein_miner.hmod")
        );
        assert_eq!(
            bundle_stem("vein_miner.hnmod"),
            Some(("vein_miner", "hnmod"))
        );
        assert_eq!(bundle_stem("vein_miner.hmod"), Some(("vein_miner", "hmod")));
        assert_eq!(bundle_stem("vein_miner.zip"), None);
        assert_eq!(bundle_stem("vein_miner"), None);
    }

    /// An existing bundle is found under either extension, and the staged
    /// name decides which extension is kept for native-only vs combined.
    #[test]
    fn stored_bundle_and_extension_preservation() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "hz-native-mods-test-{}-{stamp}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(bundle_dir(&dir)).unwrap();

        let native_stage = dir.join("import.part");
        fs::write(&native_stage, b"native").unwrap();
        assert_eq!(staged_extension(&native_stage, false).unwrap(), "hnmod");
        assert_eq!(staged_extension(&native_stage, true).unwrap(), "hmod");

        let combined_stage = dir.join("import.hmod");
        fs::write(&combined_stage, b"combined").unwrap();
        assert_eq!(staged_extension(&combined_stage, true).unwrap(), "hmod");
        assert_eq!(staged_extension(&combined_stage, false).unwrap(), "hmod");

        let unknown = dir.join("import.zip");
        fs::write(&unknown, b"unknown").unwrap();
        assert!(staged_extension(&unknown, false).is_err());

        assert_eq!(stored_bundle(&dir, "alpha"), None);
        fs::write(bundle_file(&dir, "alpha", "hmod"), b"x").unwrap();
        assert_eq!(
            stored_bundle(&dir, "alpha"),
            Some(bundle_file(&dir, "alpha", "hmod"))
        );
        fs::write(bundle_file(&dir, "beta", "hnmod"), b"y").unwrap();
        assert_eq!(
            stored_bundle(&dir, "beta"),
            Some(bundle_file(&dir, "beta", "hnmod"))
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
