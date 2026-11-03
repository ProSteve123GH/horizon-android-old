//! Android glue for importing an existing Minecraft 26.3 installation.
//!
//! The platform-independent importer (`horizon_client::minecraft_import`) fills
//! the official installer's download cache. This module supplies the two Android
//! sources (an SAF folder tree and a picked ZIP), reports progress to the Compose
//! launcher, checks free space, and then runs the same preparation step as the
//! Download button so a source with missing files still finishes normally.
use horizon_client::{
    minecraft_import::{self as core, InstallationSource, Progress},
    official_install::{self, InstallProgress, InstallStage},
};
use jni::{
    JNIEnv,
    objects::{JObject, JValue},
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    os::fd::FromRawFd,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_JSON: u64 = 32 * 1024 * 1024;
const MAX_FILES: usize = 50_000;
const MAX_DEPTH: usize = 24;
/// Player data and Java-only folders are never imported. Skipping them keeps
/// large `.minecraft` folders (saves, packs, libraries) out of the SAF index.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "backups",
    "cache",
    "config",
    "crash-reports",
    "debug",
    "downloads",
    "libraries",
    "local",
    "logs",
    "mods",
    "replay_recordings",
    "resourcepacks",
    "saves",
    "schematics",
    "screenshots",
    "shaderpacks",
];
/// Preparation needs room beside the download cache. The cache is removed after
/// a successful install, so this is peak, not permanent, usage.
const PREPARE_RESERVE: u64 = 128 * 1024 * 1024;
/// A conservative lower bound for the prepared Minecraft resources. A source
/// that contains only a JAR still causes the installer to download the assets.
const MIN_INSTALL_BYTES: u64 = 512 * 1024 * 1024;
const MIME_DIRECTORY: &str = "vnd.android.document/directory";

static STATE: Mutex<State> = Mutex::new(State {
    busy: false,
    phase: Phase::Idle,
    source: String::new(),
    stages: Vec::new(),
    stage: String::new(),
    stage_index: 0,
    overall: 0.0,
    detail: String::new(),
    notice: None,
    cancelable: false,
    install_base: 0.35,
    stage_offset: 1,
});
static CANCEL: AtomicBool = AtomicBool::new(false);
static INSUFFICIENT: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Reading,
    Importing,
    Preparing,
}

struct State {
    busy: bool,
    phase: Phase,
    source: String,
    stages: Vec<String>,
    stage: String,
    stage_index: usize,
    overall: f32,
    detail: String,
    notice: Option<Notice>,
    cancelable: bool,
    /// Where installer stages start on the overall bar (0.35 for an import,
    /// 0 for a repair, which has no copy phase of its own).
    install_base: f32,
    /// 1 when the stage list starts with the import copy phase, 0 for a repair.
    stage_offset: usize,
}

/// One finished status line for the bridge to show once.
pub(crate) struct Notice {
    pub text: String,
    pub error: Option<String>,
    pub changed: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Folder,
    Zip,
}

pub(crate) fn busy() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).busy
}

/// Starts copying the selected source and then prepares it. Returns immediately;
/// the worker updates [`snapshot`] and leaves a [`Notice`] for [`poll`].
pub(crate) fn start(home: PathBuf, kind: Kind, uri: String) -> Result<(), String> {
    if !uri.starts_with("content://") || uri.len() > 8192 {
        return Err("The selected document has an unsupported URI".into());
    }
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if state.busy {
            return Err("An installation import is already running".into());
        }
        CANCEL.store(false, Ordering::Relaxed);
        INSUFFICIENT.store(false, Ordering::Relaxed);
        state.busy = true;
        state.phase = Phase::Reading;
        state.source = match kind {
            Kind::Folder => "Minecraft folder".into(),
            Kind::Zip => "Minecraft ZIP".into(),
        };
        state.stages = import_stages();
        state.stage = "Reading the selected source".into();
        state.stage_index = 0;
        state.overall = 0.0;
        state.detail = "Reading the folder or archive. Nothing is uploaded.".into();
        state.notice = None;
        state.cancelable = true;
        state.install_base = 0.35;
        state.stage_offset = 1;
    }
    let spawned = std::thread::Builder::new()
        .name("Minecraft import".into())
        .spawn(move || run(home, kind, uri));
    if let Err(error) = spawned {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.busy = false;
        state.phase = Phase::Idle;
        state.cancelable = false;
        return Err(format!("Could not start the import: {error}"));
    }
    Ok(())
}

pub(crate) fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.busy && state.cancelable {
        state.detail = "Cancelling after the current file...".into();
    }
}

/// Verifies the prepared installation and re-downloads missing or damaged
/// Minecraft files through the official installer. Existing prepared assets are
/// hard-linked into the installer cache, so only missing/corrupt objects and the
/// client JAR are fetched from Mojang.
pub(crate) fn start_repair(home: PathBuf) -> Result<(), String> {
    if !official_install::ready(&home) && !core::prepared_root(&home).is_dir() {
        return Err("Minecraft is not installed yet. Import or download it first.".into());
    }
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if state.busy {
            return Err("An installation task is already running".into());
        }
        CANCEL.store(false, Ordering::Relaxed);
        INSUFFICIENT.store(false, Ordering::Relaxed);
        state.busy = true;
        state.phase = Phase::Preparing;
        state.source = "Minecraft resources".into();
        state.stages = installer_stages();
        state.stage = "Checking existing resources".into();
        state.stage_index = 0;
        state.overall = 0.0;
        state.detail = "Checking the current installation before downloading anything.".into();
        state.notice = None;
        state.cancelable = false;
        state.install_base = 0.0;
        state.stage_offset = 0;
    }
    let spawned = std::thread::Builder::new()
        .name("Minecraft repair".into())
        .spawn(move || run_repair(home));
    if let Err(error) = spawned {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.busy = false;
        state.phase = Phase::Idle;
        return Err(format!("Could not start the repair: {error}"));
    }
    Ok(())
}

/// The bridge merges these fields over its normal installation progress.
pub(crate) fn snapshot() -> Option<Value> {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !state.busy {
        return None;
    }
    Some(json!({
        "source": state.source,
        "importing": matches!(state.phase, Phase::Reading | Phase::Importing),
        "stage": state.stage,
        "stage_index": state.stage_index,
        "stages": state.stages,
        "overall": state.overall,
        "detail": state.detail,
        "cancelable": state.cancelable,
    }))
}

pub(crate) fn poll() -> Option<Notice> {
    STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .notice
        .take()
}

fn import_stages() -> Vec<String> {
    vec![
        "Importing existing Minecraft files".into(),
        "Downloading missing files".into(),
        "Preparing game data".into(),
    ]
}

fn install_stages() -> Vec<String> {
    let mut stages = vec!["Importing existing Minecraft files".to_owned()];
    stages.extend(installer_stages());
    stages
}

fn installer_stages() -> Vec<String> {
    InstallStage::ALL
        .iter()
        .map(|stage| stage.label().to_owned())
        .collect()
}

fn run(home: PathBuf, kind: Kind, uri: String) {
    let (mut source, label) = match open_source(kind, &uri) {
        Ok(source) => source,
        Err(error) => {
            fail(format!("Could not read the selected installation: {error}"));
            return;
        }
    };
    if CANCEL.load(Ordering::Relaxed) {
        cancelled();
        return;
    }
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.source = label.clone();
        state.phase = Phase::Importing;
        state.stage = "Importing existing Minecraft files".into();
        state.detail = "Checking files against this engine's pinned Minecraft version.".into();
    }
    let cache = official_install::download_directory(&home);
    let available = official_install::available_space(&home).unwrap_or(u64::MAX);
    let mut checked_space = false;
    let mut needed = 0;
    let result = core::import(source.as_mut(), &cache, &CANCEL, |progress| {
        if !checked_space && progress.total > 0 {
            checked_space = true;
            // Copying into the download cache and then preparing the stage
            // briefly needs room for both copies. The cache is removed after a
            // successful preparation.
            needed = progress
                .total
                .max(MIN_INSTALL_BYTES)
                .saturating_mul(2)
                .saturating_add(PREPARE_RESERVE);
            if available < needed {
                INSUFFICIENT.store(true, Ordering::Relaxed);
                CANCEL.store(true, Ordering::Relaxed);
            }
        }
        update_import(progress);
    });
    if INSUFFICIENT.load(Ordering::Relaxed) {
        fail(format!(
            "Not enough free storage: importing and preparing Minecraft needs about {} MiB. Free space in Files, then retry.",
            needed.div_ceil(1_048_576)
        ));
        return;
    }
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            if CANCEL.load(Ordering::Relaxed) {
                cancelled();
            } else {
                fail(format!("Import failed: {error}"));
            }
            return;
        }
    };
    if CANCEL.load(Ordering::Relaxed) {
        cancelled();
        return;
    }
    prepare(home, label, report);
}

fn prepare(home: PathBuf, label: String, report: core::Report) {
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.phase = Phase::Preparing;
        state.cancelable = false;
        state.stages = install_stages();
        state.stage_offset = 1;
        state.stage_index = 1;
        state.stage = InstallStage::Metadata.label().into();
        state.detail = "Files that were missing from the source will be downloaded now.".into();
        state.overall = 0.35;
    }
    let result = official_install::install_with_progress(&home, update_install);
    match result {
        Ok(()) => {
            if let Err(error) = core::write_record(&home, &label, &report) {
                log::warn!("Could not record the imported Minecraft source: {error}");
            }
            let text = if report.files_missing == 0 {
                format!(
                    "Minecraft 26.3 imported from {label}. {} files were reused.",
                    report.files_reused
                )
            } else {
                format!(
                    "Minecraft 26.3 imported from {label}. {} files were reused and {} missing files were downloaded.",
                    report.files_reused, report.files_missing
                )
            };
            finish(text, None, true);
        }
        Err(error) => fail(format!("Preparation failed: {error}")),
    }
}

fn fail(message: impl Into<String>) {
    let message = message.into();
    finish(message.clone(), Some(message), false);
}

fn ensure_space(home: &Path, needed: u64) -> Result<(), String> {
    let available = official_install::available_space(home);
    if available.is_some_and(|available| available < needed) {
        return Err(format!(
            "Not enough free storage to repair Minecraft: about {} MiB is needed. Free space in Files, then retry.",
            needed.div_ceil(1_048_576)
        ));
    }
    Ok(())
}

fn run_repair(home: PathBuf) {
    let root = core::prepared_root(&home);
    let record = core::record(&home);
    let assets = match prepared_assets(&root) {
        Ok(assets) => Some(assets),
        Err(error) => {
            log::warn!("Could not inspect the prepared Minecraft assets: {error}");
            None
        }
    };
    let missing = assets
        .as_ref()
        .map(|assets| assets.iter().filter(|asset| !asset.present).count())
        .unwrap_or(0);
    if official_install::ready(&home) && assets.is_some() && missing == 0 {
        finish(
            "Minecraft resources are complete. No files needed redownloading.".into(),
            None,
            false,
        );
        return;
    }
    let mut needed = MIN_INSTALL_BYTES
        .saturating_mul(2)
        .saturating_add(PREPARE_RESERVE);
    if let Some(assets) = &assets {
        let total = assets
            .iter()
            .fold(0u64, |total, asset| total.saturating_add(asset.size));
        let missing_bytes = assets
            .iter()
            .filter(|asset| !asset.present)
            .fold(0u64, |total, asset| total.saturating_add(asset.size));
        // The stage copies every prepared asset; only files that are absent
        // from the prepared directory must also fit in the download cache.
        needed = total
            .saturating_add(missing_bytes)
            .saturating_add(PREPARE_RESERVE);
        if let Err(error) = ensure_space(&home, needed) {
            fail(error);
            return;
        }
        if let Err(error) =
            seed_installer_cache(&root, assets, &official_install::download_directory(&home))
        {
            // Seeding is an optimisation. The installer still downloads anything
            // that could not be linked or copied.
            log::warn!("Could not seed the Minecraft installer cache: {error}");
        }
    } else if let Err(error) = ensure_space(&home, needed) {
        fail(error);
        return;
    }
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.stage = "Verifying and repairing resources".into();
        state.stage_index = 0;
        state.detail = if missing > 0 {
            format!(
                "{missing} prepared resource{} missing. Downloading only what is needed.",
                if missing == 1 { " is" } else { "s are" }
            )
        } else {
            "Verifying files and downloading anything missing or damaged.".into()
        };
        state.overall = 0.01;
    }
    let result = official_install::install_with_progress(&home, update_install);
    match result {
        Ok(()) => {
            if let Some(record) = record {
                match core::restore_record(&home, &record) {
                    Ok(()) => {}
                    Err(error) => {
                        log::warn!(
                            "Could not preserve the imported Minecraft source record: {error}"
                        );
                    }
                }
            }
            let text = if missing > 0 {
                format!(
                    "Minecraft resources repaired. {missing} missing file{} downloaded.",
                    if missing == 1 { " was" } else { "s were" }
                )
            } else {
                "Minecraft resources verified and repaired.".into()
            };
            finish(text, None, true);
        }
        Err(error) => fail(format!("Resource repair failed: {error}")),
    }
}

struct PreparedAsset {
    hash: String,
    size: u64,
    path: PathBuf,
    present: bool,
}

fn prepared_assets(root: &Path) -> Result<Vec<PreparedAsset>, String> {
    let index_path = root.join("asset-index.json");
    let metadata = fs::metadata(&index_path)
        .map_err(|error| format!("Could not read the prepared asset index: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_JSON {
        return Err("The prepared asset index is missing or too large".into());
    }
    let bytes = fs::read(&index_path)
        .map_err(|error| format!("Could not read the prepared asset index: {error}"))?;
    let json: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("The prepared asset index is invalid: {error}"))?;
    let objects = json["objects"]
        .as_object()
        .ok_or("The prepared asset index has no objects")?;
    let mut assets = Vec::with_capacity(objects.len());
    for (name, object) in objects {
        let hash = object["hash"]
            .as_str()
            .ok_or("A prepared asset has no hash")?;
        if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("A prepared asset has an invalid hash".into());
        }
        let size = object["size"]
            .as_u64()
            .ok_or("A prepared asset has no size")?;
        let path = prepared_asset_path(root, name)
            .ok_or_else(|| format!("A prepared asset has an unsafe path: {name}"))?;
        let present = fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_file() && metadata.len() == size)
            .unwrap_or(false);
        assets.push(PreparedAsset {
            hash: hash.to_owned(),
            size,
            path,
            present,
        });
    }
    Ok(assets)
}

fn prepared_asset_path(root: &Path, name: &str) -> Option<PathBuf> {
    let mut path = root.join("assets");
    for part in name.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains('\\')
            || part.contains('\0')
        {
            return None;
        }
        path.push(part);
    }
    Some(path)
}

/// Links prepared assets into the installer cache. The official installer
/// hashes every cache entry and downloads a fresh copy when it is corrupt, so a
/// bad hard link is repaired without touching the current prepared directory.
fn seed_installer_cache(
    root: &Path,
    assets: &[PreparedAsset],
    cache: &Path,
) -> std::io::Result<()> {
    let objects = cache.join("objects");
    fs::create_dir_all(&objects)?;
    for asset in assets.iter().filter(|asset| asset.present) {
        let destination = objects.join(&asset.hash);
        match fs::symlink_metadata(&destination) {
            Ok(metadata) if metadata.file_type().is_file() => continue,
            Ok(metadata) if metadata.is_dir() => {
                let _ = fs::remove_dir_all(&destination);
            }
            Ok(_) => {
                let _ = fs::remove_file(&destination);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => continue,
        }
        if fs::hard_link(&asset.path, &destination).is_err() {
            // App files and cache normally share a filesystem. If a device or
            // test setup does not, a copy is still better than re-downloading.
            let _ = fs::copy(&asset.path, &destination);
        }
    }
    let cache_index = cache.join("asset-index.json");
    if !cache_index.is_file() {
        let source = root.join("asset-index.json");
        if source.is_file() {
            let _ = fs::copy(source, cache_index);
        }
    }
    Ok(())
}

fn cancelled() {
    finish(
        "Import cancelled. Verified files are kept for retrying.".into(),
        None,
        false,
    );
}

fn finish(text: String, error: Option<String>, changed: bool) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state.busy = false;
    state.phase = Phase::Idle;
    state.cancelable = false;
    state.detail = text.clone();
    state.notice = Some(Notice {
        text,
        error,
        changed,
    });
}

fn update_import(progress: Progress) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !state.busy {
        return;
    }
    let fraction = if progress.total > 0 {
        progress.done.min(progress.total) as f32 / progress.total as f32
    } else {
        0.0
    };
    state.phase = Phase::Importing;
    state.stage_index = 0;
    state.stage = "Importing existing Minecraft files".into();
    state.detail = if progress.total > 0 {
        format!(
            "{} of {} MiB · {} / {} files",
            progress.done / 1_048_576,
            progress.total.div_ceil(1_048_576),
            progress.files_done,
            progress.files_total
        )
    } else {
        "Copying existing files...".into()
    };
    state.overall = 0.35 * fraction.clamp(0.0, 1.0);
}

fn update_install(progress: InstallProgress) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !state.busy {
        return;
    }
    state.phase = Phase::Preparing;
    state.cancelable = false;
    let index = InstallStage::ALL
        .iter()
        .position(|stage| *stage == progress.stage)
        .unwrap_or(0);
    state.stage_index = index + state.stage_offset;
    state.stage = progress.stage.label().into();
    state.detail = match progress.stage {
        InstallStage::Download => format!(
            "{} of {} MiB downloaded from Mojang",
            progress.done / 1_048_576,
            progress.total.div_ceil(1_048_576)
        ),
        InstallStage::Controls => format!(
            "{} / {} control files verified",
            progress.done, progress.total
        ),
        _ => "Please keep Horizon open. Prepared files are kept after a failure.".into(),
    };
    let fraction = if progress.total > 0 {
        progress.done.min(progress.total) as f32 / progress.total as f32
    } else {
        0.0
    };
    let count = InstallStage::ALL.len() as f32;
    state.overall = state.install_base
        + (1.0 - state.install_base) * ((index as f32 + fraction.clamp(0.0, 1.0)) / count);
}

fn open_source(kind: Kind, uri: &str) -> Result<(Box<dyn InstallationSource>, String), String> {
    match kind {
        Kind::Folder => Ok((
            Box::new(SafFolderSource::discover(uri)?),
            "Minecraft folder".into(),
        )),
        Kind::Zip => {
            let (file, name) = crate::platform::with_activity(|env, activity| {
                crate::document_bridge::open(env, activity, uri, "minecraft.zip")
                    .map_err(|error| error.to_string())
            })?;
            let source = core::ZipSource::new(file)
                .map_err(|error| format!("Could not read ZIP: {error}"))?;
            Ok((Box::new(source), name))
        }
    }
}

/// A picked SAF tree, indexed once by relative path and opened per file.
struct SafFolderSource {
    files: HashMap<String, SafFile>,
}

struct SafFile {
    uri: String,
    size: Option<u64>,
}

impl SafFolderSource {
    fn discover(tree_uri: &str) -> Result<Self, String> {
        let files = crate::platform::with_activity(|env, activity| {
            enumerate(env, activity, tree_uri).map_err(|error| error.to_string())
        })?;
        if files.is_empty() {
            return Err("The selected folder has no readable files".into());
        }
        Ok(Self { files })
    }
}

impl InstallationSource for SafFolderSource {
    fn read(&mut self, path: &str) -> horizon_client::Result<Option<Vec<u8>>> {
        let Some(file) = self.files.get(path) else {
            return Ok(None);
        };
        if file.size.is_some_and(|size| size > MAX_JSON) {
            return Err(format!("Installation metadata is too large: {path}").into());
        }
        let mut input = open_uri(&file.uri)?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut input)
            .take(MAX_JSON + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_JSON {
            return Err(format!("Installation metadata is too large: {path}").into());
        }
        Ok(Some(bytes))
    }

    fn size(&mut self, path: &str) -> horizon_client::Result<Option<u64>> {
        let Some(file) = self.files.get(path) else {
            return Ok(None);
        };
        if let Some(size) = file.size {
            return Ok(Some(size));
        }
        let input = open_uri(&file.uri)?;
        Ok(Some(input.metadata()?.len()))
    }

    fn copy_to(&mut self, path: &str, sink: &mut dyn Write) -> horizon_client::Result<Option<u64>> {
        let Some(file) = self.files.get(path) else {
            return Ok(None);
        };
        let mut input = open_uri(&file.uri)?;
        Ok(Some(std::io::copy(&mut input, sink)?))
    }
}

fn open_uri(uri: &str) -> Result<File, String> {
    crate::platform::with_activity(|env, activity| {
        let resolver = jni_result(env.call_method(
            activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        ))?
        .l()
        .map_err(jni_error)?;
        let text = JObject::from(env.new_string(uri).map_err(jni_error)?);
        let uri = jni_result(env.call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&text)],
        ))?
        .l()
        .map_err(jni_error)?;
        let mode = JObject::from(env.new_string("r").map_err(jni_error)?);
        let descriptor = jni_result(env.call_method(
            &resolver,
            "openFileDescriptor",
            "(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;",
            &[JValue::Object(&uri), JValue::Object(&mode)],
        ))?
        .l()
        .map_err(jni_error)?;
        if descriptor.is_null() {
            return Err("The provider returned no file for the selected source".into());
        }
        let raw = jni_result(env.call_method(&descriptor, "detachFd", "()I", &[]))
            .and_then(|value| value.i().map_err(jni_error))?;
        if raw < 0 {
            return Err("The provider returned an invalid file descriptor".into());
        }
        // detachFd transferred ownership of the descriptor to Rust.
        let file = unsafe { File::from_raw_fd(raw) };
        if env.call_method(&descriptor, "close", "()V", &[]).is_err() {
            let _ = env.exception_clear();
        }
        Ok(file)
    })
}

fn jni_result<T>(result: jni::errors::Result<T>) -> Result<T, String> {
    result.map_err(jni_error)
}

fn jni_error(error: jni::errors::Error) -> String {
    error.to_string()
}

#[derive(Debug)]
enum SafError {
    Jni(jni::errors::Error),
    Bad(String),
}

impl From<jni::errors::Error> for SafError {
    fn from(error: jni::errors::Error) -> Self {
        Self::Jni(error)
    }
}

impl std::fmt::Display for SafError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Jni(error) => write!(f, "{error}"),
            Self::Bad(error) => f.write_str(error),
        }
    }
}

fn enumerate(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    tree_uri: &str,
) -> Result<HashMap<String, SafFile>, SafError> {
    let resolver = env
        .call_method(
            activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )?
        .l()?;
    let text = JObject::from(env.new_string(tree_uri)?);
    let tree = env
        .call_static_method(
            "android/net/Uri",
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(&text)],
        )?
        .l()?;
    if tree.is_null() {
        return Err(SafError::Bad("The selected folder URI is invalid".into()));
    }
    let root = env
        .call_static_method(
            "android/provider/DocumentsContract",
            "getTreeDocumentId",
            "(Landroid/net/Uri;)Ljava/lang/String;",
            &[JValue::Object(&tree)],
        )?
        .l()?;
    if root.is_null() {
        return Err(SafError::Bad(
            "The selected folder is not a document tree".into(),
        ));
    }
    let root: String = env.get_string((&root).into())?.into();
    let mut files = HashMap::new();
    let mut seen = HashSet::from([root.clone()]);
    let mut count = 0;
    walk(
        env, &resolver, &tree, &root, "", &mut files, &mut seen, &mut count, 0,
    )?;
    Ok(files)
}

#[allow(clippy::too_many_arguments)]
fn walk(
    env: &mut JNIEnv<'_>,
    resolver: &JObject<'_>,
    tree: &JObject<'_>,
    parent: &str,
    prefix: &str,
    files: &mut HashMap<String, SafFile>,
    seen: &mut HashSet<String>,
    count: &mut usize,
    depth: usize,
) -> Result<(), SafError> {
    if depth > MAX_DEPTH {
        return Err(SafError::Bad(
            "The selected folder is nested too deeply".into(),
        ));
    }
    let parent = JObject::from(env.new_string(parent)?);
    let children = env
        .call_static_method(
            "android/provider/DocumentsContract",
            "buildChildDocumentsUriUsingTree",
            "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(tree), JValue::Object(&parent)],
        )?
        .l()?;
    let rows_result = env.with_local_frame(16, |env| -> jni::errors::Result<Vec<Row>> {
        let projection = env.new_object_array(4, "java/lang/String", JObject::null())?;
        for (index, column) in ["document_id", "display_name", "mime_type", "size"]
            .iter()
            .enumerate()
        {
            env.set_object_array_element(&projection, index as i32, env.new_string(*column)?)?;
        }
        let null = JObject::null();
        let cursor = env
            .call_method(
                resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[
                    JValue::Object(&children),
                    JValue::Object(projection.as_ref()),
                    JValue::Object(&null),
                    JValue::Object(&null),
                    JValue::Object(&null),
                ],
            )?
            .l()?;
        if cursor.is_null() {
            return Err(jni::errors::Error::NullPtr("document provider cursor"));
        }
        let result = (|| -> jni::errors::Result<Vec<Row>> {
            let mut rows = Vec::new();
            let count = env.call_method(&cursor, "getCount", "()I", &[])?.i()?;
            for index in 0..count {
                // A large objects directory can have thousands of children.
                // Read each row in its own local frame so JNI references do not
                // accumulate up to Android's per-frame limit.
                let row = env.with_local_frame(16, |env| -> jni::errors::Result<Option<Row>> {
                    if !env.call_method(
                        &cursor,
                        "moveToPosition",
                        "(I)Z",
                        &[JValue::Int(index)],
                    )?
                    .z()?
                    {
                        return Ok(None);
                    }
                    let id = env
                        .call_method(
                            &cursor,
                            "getString",
                            "(I)Ljava/lang/String;",
                            &[JValue::Int(0)],
                        )?
                        .l()?;
                    let name = env
                        .call_method(
                            &cursor,
                            "getString",
                            "(I)Ljava/lang/String;",
                            &[JValue::Int(1)],
                        )?
                        .l()?;
                    let mime = env
                        .call_method(
                            &cursor,
                            "getString",
                            "(I)Ljava/lang/String;",
                            &[JValue::Int(2)],
                        )?
                        .l()?;
                    if id.is_null() || name.is_null() {
                        return Ok(None);
                    }
                    let id: String = env.get_string((&id).into())?.into();
                    let name: String = env.get_string((&name).into())?.into();
                    let directory = if mime.is_null() {
                        false
                    } else {
                        let mime: String = env.get_string((&mime).into())?.into();
                        mime == MIME_DIRECTORY
                    };
                    let size = if env
                        .call_method(&cursor, "isNull", "(I)Z", &[JValue::Int(3)])?
                        .z()?
                    {
                        None
                    } else {
                        u64::try_from(
                            env.call_method(&cursor, "getLong", "(I)J", &[JValue::Int(3)])?
                                .j()?,
                        )
                        .ok()
                    };
                    let document = env
                        .call_static_method(
                            "android/provider/DocumentsContract",
                            "buildDocumentUriUsingTree",
                            "(Landroid/net/Uri;Ljava/lang/String;)Landroid/net/Uri;",
                            &[
                                JValue::Object(tree),
                                JValue::Object(&JObject::from(env.new_string(&id)?)),
                            ],
                        )?
                        .l()?;
                    let uri = env
                        .call_method(&document, "toString", "()Ljava/lang/String;", &[])?
                        .l()?;
                    if uri.is_null() {
                        return Ok(None);
                    }
                    let uri: String = env.get_string((&uri).into())?.into();
                    Ok(Some(Row {
                        id,
                        name,
                        directory,
                        size,
                        uri,
                    }))
                })?;
                if let Some(row) = row {
                    rows.push(row);
                }
            }
            Ok(rows)
        })();
        if result.is_err() {
            let _ = env.exception_clear();
        }
        let _ = env.call_method(&cursor, "close", "()V", &[]);
        result
    });
    let rows: Vec<Row> = rows_result?;
    // Release the parent and children references before descending; a deep
    // tree otherwise accumulates local references in the enclosing frame.
    let _ = env.delete_local_ref(children);
    let _ = env.delete_local_ref(parent);
    for row in rows {
        if !plain_name(&row.name) {
            return Err(SafError::Bad(
                "The selected folder contains an unsafe file name".into(),
            ));
        }
        let path = format!("{prefix}{}", row.name);
        if row.directory {
            if SKIP_DIRS.contains(&row.name.as_str()) {
                continue;
            }
            if seen.insert(row.id.clone()) {
                walk(
                    env,
                    resolver,
                    tree,
                    &row.id,
                    &format!("{path}/"),
                    files,
                    seen,
                    count,
                    depth + 1,
                )?;
            }
        } else {
            *count += 1;
            if *count > MAX_FILES {
                return Err(SafError::Bad(
                    "The selected folder contains too many files".into(),
                ));
            }
            files.insert(
                path,
                SafFile {
                    uri: row.uri,
                    size: row.size,
                },
            );
        }
    }
    Ok(())
}

struct Row {
    id: String,
    name: String,
    directory: bool,
    size: Option<u64>,
    uri: String,
}

fn plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_names_are_rejected() {
        assert!(!plain_name(".."));
        assert!(!plain_name("assets/objects"));
        assert!(!plain_name("nul\0byte"));
        assert!(plain_name("26.3.jar"));
        assert!(plain_name("objects"));
    }

    #[test]
    fn install_stages_keep_the_import_step_first() {
        let stages = install_stages();
        assert_eq!(stages[0], "Importing existing Minecraft files");
        assert_eq!(stages.len(), InstallStage::ALL.len() + 1);
    }
}
