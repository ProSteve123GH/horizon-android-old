//! Native mod (`.hnmod` / `.hmod`) import, listing, toggling and removal for the Android launcher.
//!
//! A `.hnmod` bundle is a native mod; a combined `.hmod` carries native code
//! plus a content part and is stored in both folders so the existing content
//! installer/list sees it. Native mod **building is not supported on Android**:
//! there is no upstream Android SDK slice to rebuild the game against, so this
//! module only manages bundles that are already installed and the loader state
//! the game process reads at start. Native mods use the `mod_api` SDK and
//! `.hmod` packages on supported hosts. Nothing here compiles or executes mod
//! code. `files/native/` (app-private) holds the loader state; bundles live
//! with the game content.
use horizon_mods::index::native::Bundle;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

// TODO(keep-consistency): the desktop launcher's shared native-mod browser reads build records
// written by its local Rust toolchain. Android has no native mod builder, so it has neither a
// `native-mods.json` nor a GitHub origin for a build; the shared launcher modules need these
// names to compile, and report "no native build" on this platform.
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

/// Reserve a sibling temp file next to `destination` by copying `source` into
/// it with `create_new`, so no existing file or symlink is ever followed or
/// replaced. The dot-prefixed name keeps a half-written copy invisible to the
/// content and native mod installers, which only scan for `.hmod` / `.hnmod`.
fn copy_to_temp(source: &Path, destination: &Path) -> Result<PathBuf, String> {
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".to_owned());
    for attempt in 0..32 {
        let temp_name = if attempt == 0 {
            format!(".{name}.tmp")
        } else {
            format!(".{name}.tmp.{attempt}")
        };
        let temp = destination.with_file_name(temp_name);
        if fs::symlink_metadata(&temp).is_ok() {
            continue; // a leftover (or planted) name: try the next one
        }
        match copy_new(source, &temp) {
            Ok(()) => return Ok(temp),
            Err(error) => {
                // `create_new` refused a path that appeared in between, or a
                // copy failed part way; never leave a partial temp behind.
                let _ = fs::remove_file(&temp);
                return Err(error);
            }
        }
    }
    Err(format!(
        "could not reserve a temporary file next to {}",
        destination.display()
    ))
}

/// Publish a prepared temp into its final path without replacing an existing
/// file or following a symlink. The temp is consumed on success and left in
/// place on failure so the caller can roll it back.
fn publish_temp(temp: &Path, destination: &Path) -> Result<(), String> {
    if fs::hard_link(temp, destination).is_ok() {
        let _ = fs::remove_file(temp);
        return Ok(());
    }
    // FUSE-backed shared storage may not support hard links; `copy_new` keeps
    // the same create_new / no-follow guarantees within the directory.
    copy_new(temp, destination)?;
    let _ = fs::remove_file(temp);
    Ok(())
}

/// The id in a file's own `horizon-mod.toml`. A file name with the right id is
/// not proof of ownership: the content manifest is what identifies a mod.
fn content_manifest_id(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let manifest = zip.by_name(horizon_mods::manifest::MANIFEST).ok()?;
    let mut text = String::new();
    manifest.take(1 << 20).read_to_string(&mut text).ok()?;
    horizon_mods::Manifest::parse(&text).ok().map(|parsed| parsed.id)
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
                    "Imported native mod {id} {version}. Native code in a bundle runs unsandboxed; building native mods is not supported on Android."
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
    if fs::symlink_metadata(&target).is_ok() {
        return Err(format!("a native mod with id '{id}' is installed; remove it first"));
    }
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
    // Prepare both destinations as sibling temps before publishing either.
    // The staged source is never consumed until both files are in place, so a
    // failure anywhere below leaves the user's picked file untouched and a
    // retry does not ask them to pick it again. The cache and the instance may
    // be on different filesystems, so copying (rather than renaming) avoids
    // EXDEV surprises too.
    let native_temp = copy_to_temp(staged, &target)?;
    let mirror_temp = match &mirror {
        Some(mirror) => match copy_to_temp(staged, mirror) {
            Ok(temp) => Some(temp),
            Err(error) => {
                let _ = fs::remove_file(&native_temp);
                return Err(error);
            }
        },
        None => None,
    };
    let result = (|| -> Result<(), String> {
        publish_temp(&native_temp, &target)?;
        if let (Some(temp), Some(mirror)) = (&mirror_temp, &mirror) {
            publish_temp(temp, mirror)?;
        }
        Ok(())
    })();
    if result.is_err() {
        // Neither destination existed when we started (collisions are refused
        // above), so removing them here cannot delete someone else's file: it
        // only clears a partial or half-published install. The temps and the
        // staged source stay untouched so a retry needs no new file pick.
        if let Some(mirror) = &mirror {
            let _ = fs::remove_file(mirror);
        }
        let _ = fs::remove_file(&target);
        let _ = fs::remove_file(&native_temp);
        if let Some(temp) = &mirror_temp {
            let _ = fs::remove_file(temp);
        }
        return Err(result.unwrap_err());
    }
    // Both files are safely in place; the staged copy is now ours to consume.
    let _ = fs::remove_file(staged);
    Ok((id, version))
}

// ---- removal -----------------------------------------------------------------

/// `native_remove`: delete the stored bundle (either extension) and, only when
/// it provably belongs to this bundle, the mirrored content file.
pub(crate) fn remove(game_home: &Path, id: &str) -> Result<(), String> {
    if !horizon_mods::index::valid_id(id) {
        return Err("Invalid native mod id".into());
    }
    let mut removed = false;
    // Only a combined native bundle (stored as `.hmod`) ever creates a mirror;
    // a native-only `.hnmod` cannot own one.
    let mut stored_combined = false;
    for &extension in &BUNDLE_EXTENSIONS {
        let file = bundle_file(game_home, id, extension);
        match fs::symlink_metadata(&file) {
            Ok(metadata) if metadata.is_file() => {
                fs::remove_file(&file).map_err(|error| format!("could not remove {id}: {error}"))?;
                removed = true;
                stored_combined |= extension == "hmod";
            }
            Ok(_) => return Err(format!("Refusing to remove a redirected native mod with id '{id}'")),
            Err(_) => {}
        }
    }
    if !removed {
        return Err(format!("No native mod with id '{id}' is installed"));
    }
    // A matching file name is not proof of ownership: a separate content mod
    // with the same id may live at that path (e.g. a content `foo.hmod`
    // installed before a native-only `foo.hnmod`). Delete the mirror only when
    // the stored native file was combined *and* the mirror's own content
    // manifest names this id. If either signal is missing, leave it alone.
    if stored_combined {
        let mirror = game_home.join("horizon-mods").join(format!("{id}.hmod"));
        if let Ok(metadata) = fs::symlink_metadata(&mirror) {
            if metadata.is_file() && content_manifest_id(&mirror).as_deref() == Some(id) {
                fs::remove_file(&mirror).map_err(|error| format!("could not remove {id}: {error}"))?;
            }
        }
    }
    Ok(())
}

/// `native_disable`: forget any selected modded build. The next start loads
/// the stock game. Bundles are kept, but the rollback target and crash
/// attribution go too: a disabled selection must not silently come back.
pub(crate) fn disable(home: &Path) -> Result<(), String> {
    let dir = priv_native(home);
    for name in [
        "current.json",
        "current.json.rejected",
        "active.json",
        "pending_crash",
        "crash_streak",
        "last_rejected",
        "last_failed_mod",
        "last_known_good.json",
        "first-frame",
    ] {
        let _ = fs::remove_file(dir.join(name));
    }
    Ok(())
}

// ---- enabled flags ---------------------------------------------------

fn enabled_file(game_home: &Path) -> PathBuf {
    bundle_dir(game_home).join("enabled.json")
}

// ---- safe mode / rollback ------------------------------------------------

/// The rollback target: a copy of `current.json` from a start that reached its
/// first frame. The Java loader restores it after a build is set aside.
pub(crate) const LAST_KNOWN_GOOD: &str = "last_known_good.json";
/// The mod ids that were active when a start failed before its first frame.
pub(crate) const LAST_FAILED_MOD: &str = "last_failed_mod";

/// Write a complete body through a sibling temp file and rename it into place.
/// Readers in this folder (the game process and Java) never see a partial JSON.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut temp_name = path.as_os_str().to_os_string();
    temp_name.push(".tmp");
    let temp = PathBuf::from(temp_name);
    fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        error.to_string()
    })
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

/// `native_enabled`: flip whether `id` is enabled in the launcher's mod list.
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

/// The snapshot's `native` section: stored bundles and the loader's currently
/// selected library (when one is installed by hand). Cached by a fingerprint
/// of the folder and the loader state; the snapshot is rebuilt often.
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
    for name in [
        "current.json",
        "current.json.rejected",
        "last_rejected",
        LAST_KNOWN_GOOD,
        LAST_FAILED_MOD,
    ] {
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
    cache
        .value
        .clone()
        .unwrap_or_else(|| json!({ "mods": [], "current": Value::Null }))
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
    let reason = || {
        fs::read_to_string(dir.join("last_rejected"))
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    let mut value = if let Some(current) = read_json(&dir.join("current.json")) {
        // A soft rejection leaves current.json in place; show its reason too.
        current_value(&current, false, reason())
    } else if dir.join("current.json.rejected").is_file() {
        let rejected = read_json(&dir.join("current.json.rejected")).unwrap_or(Value::Null);
        let why = reason().unwrap_or_else(|| {
            "A modded build was set aside after crashing before its first frame. Playing the stock game."
                .to_owned()
        });
        current_value(&rejected, true, Some(why))
    } else {
        return Value::Null;
    };
    if let Ok(failed) = fs::read_to_string(dir.join(LAST_FAILED_MOD))
        && !failed.trim().is_empty()
    {
        value["lastFailedMod"] = json!(failed.trim());
    }
    value["rollbackAvailable"] = json!(read_json(&dir.join(LAST_KNOWN_GOOD)).is_some());
    value
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
        // Optional compat gate fields; absent on older bundles.
        "engineCommit": text("engine_commit"),
        "apiLevel": text("api_level"),
        "minApkVersion": text("min_apk_version"),
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

// ---- tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The launcher surfaces the compat contract and safe-mode state that the
    /// game and Java write into `native/`: fields, rejection reason, the mod
    /// blamed for a failed start and whether a rollback target exists.
    #[test]
    fn compat_and_safe_mode_state_is_visible_to_the_launcher() {
        let home = std::env::temp_dir().join(format!(
            "horizon-native-safe-mode-{}",
            std::process::id()
        ));
        let dir = priv_native(&home);
        fs::create_dir_all(&dir).unwrap();
        let body = json!({
            "id": "good-mod",
            "version": "1.0",
            "lib": "/builds/good/libhorizon_android.so",
            "abi": "arm64-v8a",
            "versionCode": 42,
            "engine_commit": "abc123",
            "api_level": "1",
            "min_apk_version": "40",
        })
        .to_string();
        atomic_write(&dir.join("current.json"), body.as_bytes()).unwrap();
        atomic_write(&dir.join(LAST_KNOWN_GOOD), body.as_bytes()).unwrap();
        atomic_write(&dir.join(LAST_FAILED_MOD), b"bad-mod").unwrap();
        atomic_write(&dir.join("last_rejected"), b"engine commit mismatch").unwrap();

        let shown = current(&home);
        assert_eq!(shown["engineCommit"], "abc123");
        assert_eq!(shown["apiLevel"], "1");
        assert_eq!(shown["minApkVersion"], "40");
        assert_eq!(shown["reject_reason"], "engine commit mismatch");
        assert_eq!(shown["lastFailedMod"], "bad-mod");
        assert_eq!(shown["rollbackAvailable"], true);
        let _ = fs::remove_dir_all(&home);
    }

    /// Removing a native-only bundle must not delete a content mod that merely
    /// shares its id (the P1 data-loss trigger).
    #[test]
    fn remove_native_only_leaves_a_content_mod_mirror_alone() {
        let home = std::env::temp_dir().join(format!(
            "hz-native-remove-native-only-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(bundle_dir(&home)).unwrap();
        fs::create_dir_all(home.join("horizon-mods")).unwrap();
        fs::write(bundle_file(&home, "foo", "hnmod"), b"native").unwrap();
        let mirror = home.join("horizon-mods/foo.hmod");
        fs::write(&mirror, b"someone else's content").unwrap();

        remove(&home, "foo").unwrap();
        assert_eq!(stored_bundle(&home, "foo"), None);
        assert_eq!(fs::read(&mirror).unwrap(), b"someone else's content");
        let _ = fs::remove_dir_all(&home);
    }

    fn content_hmod(id: &str) -> Vec<u8> {
        let toml = format!(
            "id=\"{id}\"\nname=\"Foo\"\nversion=\"1.0.0\"\napi=\"0.2\"\ncapabilities=[\"registries\"]\n"
        );
        let mut body = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut body));
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file(horizon_mods::manifest::MANIFEST, options)
                .unwrap();
            zip.write_all(toml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        body
    }

    /// A combined native bundle may remove a mirror only when that mirror's own
    /// content manifest names the same id. A mirror naming another id is left
    /// alone even at the expected path.
    #[test]
    fn remove_deletes_only_a_mirror_that_proves_its_identity() {
        let home = std::env::temp_dir().join(format!(
            "hz-native-remove-combined-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(bundle_dir(&home)).unwrap();
        let content_dir = home.join("horizon-mods");
        fs::create_dir_all(&content_dir).unwrap();

        fs::write(bundle_file(&home, "foo", "hmod"), b"combined").unwrap();
        let own = content_dir.join("foo.hmod");
        fs::write(&own, content_hmod("foo")).unwrap();
        remove(&home, "foo").unwrap();
        assert!(!own.exists(), "a proven mirror belongs to the native bundle");

        fs::write(bundle_file(&home, "bar", "hmod"), b"combined").unwrap();
        let foreign = content_dir.join("bar.hmod");
        fs::write(&foreign, content_hmod("other")).unwrap();
        remove(&home, "bar").unwrap();
        assert!(foreign.is_file(), "an unproven mirror must be left alone");
        let _ = fs::remove_dir_all(&home);
    }
}
