//! Reuse an existing Minecraft 26.3 installation instead of downloading it again.
//!
//! A source is either a normal launcher layout (`versions/26.3/26.3.json`,
//! `assets/indexes/34.json`, `assets/objects/xx/<hash>`) or a ZIP of one. The
//! importer verifies the same pins the official installer uses and fills the
//! installer's existing download cache. It never writes the prepared
//! installation directory; the normal installer validates the cache and
//! prepares the resources, downloading only what the source did not contain.
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

const LOCK: &str = include_str!("../../../research-lock.json");
const MAX_JSON: u64 = 16 * 1024 * 1024;
const MAX_OBJECT: u64 = 2 * 1024 * 1024 * 1024;

/// A normal folder, a ZIP, or Android's SAF tree, seen as files by logical path.
pub trait InstallationSource {
    /// Reads a small metadata file. `None` means the path is absent.
    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>>;
    /// Reports a file's size without reading it. `None` means the path is absent.
    fn size(&mut self, path: &str) -> Result<Option<u64>>;
    /// Streams a file into `sink`. `Ok(None)` means the path is absent.
    fn copy_to(&mut self, path: &str, sink: &mut dyn Write) -> Result<Option<u64>>;
    /// Duplicate archive entries that were ignored, for the import report.
    fn duplicate_entries(&self) -> u64 {
        0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub files_done: u64,
    pub files_total: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Report {
    pub files_copied: u64,
    pub files_reused: u64,
    pub files_missing: u64,
    pub duplicate_entries: u64,
    pub bytes_copied: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportRecord {
    pub schema: u32,
    pub source: String,
    pub imported_unix: u64,
    pub files_copied: u64,
    pub files_reused: u64,
    pub files_missing: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct InstallationInfo {
    pub version: String,
    pub installed: bool,
    pub source: String,
    pub imported_unix: Option<u64>,
    pub size_bytes: u64,
    pub cache_bytes: u64,
    /// Source files the importer did not find and had to download.
    pub files_missing: u64,
    pub missing: Vec<String>,
}

/// Reads an already prepared installation without touching player data.
pub fn info(home: &Path) -> InstallationInfo {
    let root = prepared_root(home);
    let installed = crate::official_install::ready(home);
    let record = record(home);
    let mut missing = Vec::new();
    if installed {
        for name in [
            "horizon-resources.json",
            "asset-index.json",
            "android-downloads-v2.json",
        ] {
            if !root.join(name).is_file() {
                missing.push(name.to_owned());
            }
        }
        if cfg!(feature = "audio") && !root.join("audio-complete.json").is_file() {
            missing.push("audio-complete.json".into());
        }
    }
    InstallationInfo {
        version: horizon_core::VANILLA_VERSION.to_owned(),
        installed,
        source: record
            .as_ref()
            .map(|r| r.source.clone())
            .unwrap_or_else(|| "downloaded".into()),
        imported_unix: record.as_ref().map(|r| r.imported_unix),
        size_bytes: directory_size(&root),
        cache_bytes: directory_size(&crate::official_install::download_directory(home)),
        files_missing: record.map(|r| r.files_missing).unwrap_or(0),
        missing,
    }
}

/// Deletes the prepared 26.3 resources and its reusable download cache.
/// Saves, profiles, packs, mods and the APK's engine tables are untouched.
pub fn remove(home: &Path) -> Result<()> {
    let versions = home.join(".local/horizon-client");
    for name in ["26.3", "26.3.previous", "26.3.partial", "official.partial"] {
        remove_owned(&versions.join(name))?;
    }
    remove_owned(&crate::official_install::download_directory(home))?;
    Ok(())
}

fn remove_owned(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!("Refusing to remove redirected path {}", path.display()).into());
        }
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// The prepared installation directory for the pinned Minecraft version.
pub fn prepared_root(home: &Path) -> PathBuf {
    home.join(".local/horizon-client")
        .join(horizon_core::VANILLA_VERSION)
}

/// Reads the import record, if this installation came from an import.
pub fn record(home: &Path) -> Option<ImportRecord> {
    read_record(&prepared_root(home))
}

fn read_record(root: &Path) -> Option<ImportRecord> {
    serde_json::from_slice(&fs::read(root.join("minecraft-import.json")).ok()?).ok()
}

/// Records where an installation came from after preparation succeeded.
pub fn write_record(home: &Path, source: &str, report: &Report) -> Result<()> {
    let record = ImportRecord {
        schema: 1,
        source: source.to_owned(),
        imported_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        files_copied: report.files_copied,
        files_reused: report.files_reused,
        files_missing: report.files_missing,
    };
    store_record(&prepared_root(home), &record)
}

/// Re-writes an existing import record after a repair replaced the prepared
/// directory atomically. The original source and import counts are preserved.
pub fn restore_record(home: &Path, record: &ImportRecord) -> Result<()> {
    store_record(&prepared_root(home), record)
}

fn store_record(root: &Path, record: &ImportRecord) -> Result<()> {
    fs::create_dir_all(root)?;
    let partial = root.join("minecraft-import.partial");
    fs::write(&partial, serde_json::to_vec_pretty(record)?)?;
    fs::rename(partial, root.join("minecraft-import.json"))?;
    Ok(())
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => directory_size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// A normal folder from a desktop launcher (or a folder picked through SAF).
pub struct DirectorySource {
    root: PathBuf,
}

impl DirectorySource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, path: &str) -> Result<PathBuf> {
        let relative = safe_relative(path)?;
        let full = self.root.join(relative);
        if !full.starts_with(&self.root) {
            return Err("Installation path escapes the selected folder".into());
        }
        Ok(full)
    }
}

impl InstallationSource for DirectorySource {
    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>> {
        let full = self.path(path)?;
        let metadata = match fs::metadata(&full) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() || metadata.len() > MAX_JSON {
            return Err(format!("Installation metadata is too large: {path}").into());
        }
        Ok(Some(fs::read(full)?))
    }

    fn size(&mut self, path: &str) -> Result<Option<u64>> {
        let full = self.path(path)?;
        match fs::metadata(&full) {
            Ok(metadata) if metadata.is_file() => Ok(Some(metadata.len())),
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn copy_to(&mut self, path: &str, sink: &mut dyn Write) -> Result<Option<u64>> {
        let full = self.path(path)?;
        let mut file = match fs::File::open(&full) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        Ok(Some(std::io::copy(&mut file, sink)?))
    }
}

/// Normalises ZIP entry separators and rejects traversal components. The
/// importer only reads indexed paths and never extracts an archive, but a
/// canonical map also makes `./` and Windows-created `\` layouts work.
fn normalize_zip_name(name: &str) -> Option<String> {
    let name = name.replace('\\', "/");
    let mut parts = Vec::new();
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            part if part.contains('\0') => return None,
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// A ZIP archive containing a launcher layout, possibly under `.minecraft/`.
pub struct ZipSource {
    archive: zip::ZipArchive<fs::File>,
    files: HashMap<String, (usize, u64)>,
    duplicates: u64,
}

impl ZipSource {
    pub fn new(file: fs::File) -> Result<Self> {
        let mut archive = zip::ZipArchive::new(file)?;
        let mut files = HashMap::new();
        let mut duplicates = 0;
        for index in 0..archive.len() {
            let entry = archive.by_index_raw(index)?;
            if entry.is_dir() {
                continue;
            }
            let Some(name) = normalize_zip_name(&entry.name()?) else {
                // Unsafe names are never indexed, so they cannot be extracted.
                duplicates += 1;
                continue;
            };
            if files.insert(name, (index, entry.size())).is_some() {
                duplicates += 1;
            }
        }
        Ok(Self {
            archive,
            files,
            duplicates,
        })
    }
}

impl InstallationSource for ZipSource {
    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>> {
        let Some((index, size)) = self.files.get(path).copied() else {
            return Ok(None);
        };
        if size > MAX_JSON {
            return Err(format!("Installation metadata is too large: {path}").into());
        }
        let mut entry = self.archive.by_index(index)?;
        let mut bytes = Vec::with_capacity(size as usize);
        entry.by_ref().take(MAX_JSON + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != size {
            return Err(format!("ZIP entry changed while reading: {path}").into());
        }
        Ok(Some(bytes))
    }

    fn size(&mut self, path: &str) -> Result<Option<u64>> {
        Ok(self.files.get(path).map(|(_, size)| *size))
    }

    fn copy_to(&mut self, path: &str, sink: &mut dyn Write) -> Result<Option<u64>> {
        let Some((index, size)) = self.files.get(path).copied() else {
            return Ok(None);
        };
        let mut entry = self.archive.by_index(index)?;
        let mut buffer = [0u8; 65536];
        let mut written = 0u64;
        loop {
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            written += count as u64;
            if written > size {
                return Err(format!("ZIP entry grew while reading: {path}").into());
            }
            sink.write_all(&buffer[..count])?;
        }
        if written != size {
            return Err(format!("ZIP entry is truncated: {path}").into());
        }
        Ok(Some(written))
    }

    fn duplicate_entries(&self) -> u64 {
        self.duplicates
    }
}

/// The verify-once pins from `research-lock.json`; tests use their own values.
#[derive(Clone, Debug)]
struct Pins {
    version: String,
    manifests: HashSet<String>,
    client: String,
    indexes: HashSet<String>,
}

impl Pins {
    fn from_lock() -> Result<Self> {
        Self::from_json(&serde_json::from_str(LOCK)?)
    }

    fn from_json(lock: &Value) -> Result<Self> {
        let minecraft = &lock["minecraft"];
        let strings = |value: &Value| -> Result<Vec<String>> {
            value
                .as_array()
                .ok_or("Missing accepted Minecraft hashes")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "Invalid Minecraft hash".into())
                })
                .collect()
        };
        let mut manifests: HashSet<String> = strings(&minecraft["accepted_manifest_sha1"])?
            .into_iter()
            .collect();
        let mut indexes: HashSet<String> = strings(&minecraft["accepted_asset_index_sha1"])?
            .into_iter()
            .collect();
        manifests.insert(
            minecraft["manifest_sha1"]
                .as_str()
                .ok_or("Missing version metadata hash")?
                .to_owned(),
        );
        if let Some(current) = minecraft["asset_index_sha1"].as_str() {
            indexes.insert(current.to_owned());
        }
        Ok(Self {
            version: minecraft["version"]
                .as_str()
                .ok_or("Missing Minecraft version")?
                .to_owned(),
            manifests,
            client: minecraft["client_sha1"]
                .as_str()
                .ok_or("Missing client jar hash")?
                .to_owned(),
            indexes,
        })
    }
}

struct Layout {
    root: String,
    jar: String,
    jar_size: u64,
    index: Option<(String, Vec<u8>)>,
    objects: Vec<(String, u64)>,
}

/// Imports a launcher folder or ZIP into the official installer's cache.
///
/// `cache` is the version directory of that cache (normally
/// `official_install::download_directory(home)`). Existing verified files are
/// reused. Missing source files are reported, not fatal: the normal installer
/// downloads them immediately afterwards.
pub fn import(
    source: &mut dyn InstallationSource,
    cache: &Path,
    cancel: &AtomicBool,
    progress: impl FnMut(Progress),
) -> Result<Report> {
    import_inner(source, cache, cancel, &Pins::from_lock()?, progress)
}

fn import_inner(
    source: &mut dyn InstallationSource,
    cache: &Path,
    cancel: &AtomicBool,
    pins: &Pins,
    mut progress: impl FnMut(Progress),
) -> Result<Report> {
    let layout = discover(source, pins)?;
    let mut work = Vec::new();
    work.push(Work {
        source: layout.jar.clone(),
        destination: cache.join("client.jar"),
        hash: pins.client.clone(),
        size: layout.jar_size,
    });
    if let Some((path, bytes)) = &layout.index {
        work.push(Work {
            source: path.clone(),
            destination: cache.join("asset-index.json"),
            hash: hex(&Sha1::digest(bytes)),
            size: bytes.len() as u64,
        });
    }
    for (hash, size) in &layout.objects {
        work.push(Work {
            source: format!("{}assets/objects/{}/{}", layout.root, &hash[..2], hash),
            destination: cache.join("objects").join(hash),
            hash: hash.clone(),
            size: *size,
        });
    }
    fs::create_dir_all(cache.join("objects"))?;
    // A killed process can leave `.partial` files behind. They are never valid
    // inputs; the official installer ignores them and recreates them on demand.
    cleanup_partials(cache);
    let mut state = Run {
        report: Report {
            duplicate_entries: source.duplicate_entries(),
            ..Default::default()
        },
        progress: Progress {
            total: work.iter().map(|w| w.size).sum(),
            files_total: work.len() as u64,
            ..Default::default()
        },
    };
    progress(state.progress.clone());
    for item in work {
        if cancel.load(Ordering::Relaxed) {
            return Err("Import cancelled".into());
        }
        let copied = copy_verified(source, &item, cancel, &mut state)?;
        if !copied {
            state.report.files_missing += 1;
        }
        state.progress.files_done += 1;
        state.progress.done = state
            .progress
            .done
            .saturating_add(item.size)
            .min(state.progress.total);
        progress(state.progress.clone());
    }
    Ok(state.report)
}

struct Work {
    source: String,
    destination: PathBuf,
    hash: String,
    size: u64,
}

struct Run {
    report: Report,
    progress: Progress,
}

/// Removes incomplete files left by an interrupted importer or installer.
/// Only files with a `.partial` extension are touched; verified cache entries
/// are never removed here.
fn cleanup_partials(cache: &Path) {
    let mut directories = vec![cache.to_owned()];
    let objects = cache.join("objects");
    if objects.is_dir() {
        directories.push(objects);
    }
    for directory in directories {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("partial") {
                continue;
            }
            match entry.file_type() {
                Ok(kind) if kind.is_file() => {
                    let _ = fs::remove_file(path);
                }
                _ => {}
            }
        }
    }
}

/// Returns false when the source did not contain the file.
fn copy_verified(
    source: &mut dyn InstallationSource,
    item: &Work,
    cancel: &AtomicBool,
    state: &mut Run,
) -> Result<bool> {
    // A verified cache entry is reusable even when the source has since been
    // deleted. Check it before asking the source, so retries after a cancelled
    // import report those files as reused rather than missing.
    if verified(&item.destination, &item.hash, item.size)? {
        state.report.files_reused += 1;
        return Ok(true);
    }
    let Some(source_size) = source.size(&item.source)? else {
        return Ok(false);
    };
    if source_size != item.size {
        return Err(format!("Installation file is corrupt: {}", item.source).into());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Import cancelled".into());
    }
    let partial = item.destination.with_extension("partial");
    fs::create_dir_all(item.destination.parent().ok_or("Missing cache parent")?)?;
    let result = (|| -> Result<()> {
        let mut file = fs::File::create(&partial)?;
        let copied = source
            .copy_to(&item.source, &mut file)?
            .ok_or_else(|| format!("Installation file disappeared: {}", item.source))?;
        file.sync_all()?;
        drop(file);
        if copied != item.size || counting_digest(&partial)? != item.hash {
            return Err(format!("Installation file is corrupt: {}", item.source).into());
        }
        fs::rename(&partial, &item.destination)?;
        state.report.files_copied += 1;
        state.report.bytes_copied += copied;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result?;
    Ok(true)
}

fn verified(path: &Path, expected: &str, size: u64) -> Result<bool> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() != size {
        return Ok(false);
    }
    Ok(counting_digest(path)? == expected)
}

fn counting_digest(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha1::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex(&digest.finalize()))
}

fn discover(source: &mut dyn InstallationSource, pins: &Pins) -> Result<Layout> {
    let roots = [
        "",
        ".minecraft/",
        "minecraft/",
        "Minecraft/",
        ".minecraft/.minecraft/",
    ];
    let mut version = None;
    let mut root = "";
    let mut last_error = None;
    'outer: for candidate in roots {
        for path in [
            format!("{candidate}versions/{}/{}.json", pins.version, pins.version),
            format!("{candidate}versions/{}.json", pins.version),
        ] {
            let Some(bytes) = source.read(&path)? else {
                continue;
            };
            let digest = hex(&Sha1::digest(&bytes));
            if !pins.manifests.contains(&digest) {
                last_error =
                    Some("The selected version JSON was not published for this engine".to_owned());
                continue;
            }
            let Ok(json) = serde_json::from_slice::<Value>(&bytes) else {
                last_error = Some("The selected version JSON is invalid".to_owned());
                continue;
            };
            if json["id"].as_str() != Some(&pins.version) {
                last_error = Some(format!(
                    "This installation is not Minecraft {}",
                    pins.version
                ));
                continue;
            }
            if json["downloads"]["client"]["sha1"].as_str() != Some(&pins.client) {
                return Err("Minecraft game files differ from this engine's pinned version".into());
            }
            version = Some(json);
            root = candidate;
            break 'outer;
        }
    }
    let version_json = version.ok_or_else(|| {
        last_error
            .unwrap_or_else(|| format!("No Minecraft {} installation was found", pins.version))
    })?;
    let primary = format!("{root}versions/{}/{}.jar", pins.version, pins.version);
    let alternate = format!("{root}versions/{}/client.jar", pins.version);
    let expected_jar_size = version_json["downloads"]["client"]["size"]
        .as_u64()
        .unwrap_or(0);
    let (jar, source_jar_size) = match source.size(&primary)? {
        Some(size) => (primary, size),
        None => match source.size(&alternate)? {
            Some(size) => (alternate, size),
            None => (primary, 0),
        },
    };
    // The version JSON's size is authoritative. A launcher can leave a truncated
    // jar behind; the cache's pinned hash and size still decide whether it is usable.
    let jar_size = expected_jar_size.max(source_jar_size);
    let index_id = version_json["assetIndex"]["id"]
        .as_str()
        .ok_or("Version metadata has no asset index")?;
    if !safe_component(index_id) {
        return Err("Unsafe asset index name".into());
    }
    let index_path = format!("{root}assets/indexes/{index_id}.json");
    let index = source.read(&index_path)?;
    let mut objects = Vec::new();
    if let Some(bytes) = index {
        if bytes.len() as u64 > MAX_JSON {
            return Err("Asset index is too large".into());
        }
        let digest = hex(&Sha1::digest(&bytes));
        if !pins.indexes.contains(&digest) {
            return Err("Asset index does not match this engine's pinned Minecraft version".into());
        }
        let json: Value = serde_json::from_slice(&bytes)?;
        let entries = json["objects"]
            .as_object()
            .ok_or("Asset index has no objects")?;
        let mut seen = HashSet::new();
        for (name, object) in entries {
            if !cfg!(feature = "audio")
                && (name.starts_with("minecraft/sounds/") || name == "minecraft/sounds.json")
            {
                continue;
            }
            let hash = object["hash"].as_str().ok_or("Missing asset hash")?;
            let size = object["size"].as_u64().ok_or("Missing asset size")?;
            if hash.len() != 40
                || !hash
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                || size > MAX_OBJECT
            {
                return Err("Asset index contains an invalid object".into());
            }
            if seen.insert(hash.to_owned()) {
                objects.push((hash.to_owned(), size));
            }
        }
        Ok(Layout {
            root: root.to_owned(),
            jar,
            jar_size,
            index: Some((index_path, bytes)),
            objects,
        })
    } else {
        Ok(Layout {
            root: root.to_owned(),
            jar,
            jar_size,
            index: None,
            objects,
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains(['/', '\\'])
        && !value.contains('\0')
}

fn safe_relative(path: &str) -> Result<PathBuf> {
    let mut output = PathBuf::new();
    for part in path.split('/') {
        if part.is_empty() {
            continue;
        }
        if part == "." || part == ".." || part.contains('\\') || part.contains('\0') {
            return Err("Installation path escapes the selected folder".into());
        }
        output.push(part);
    }
    let check = Path::new(&output);
    if check
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Installation path escapes the selected folder".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(bytes: &[u8]) -> String {
        hex(&Sha1::digest(bytes))
    }

    fn scratch(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "horizon-import-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    struct Fixture {
        root: PathBuf,
        jar: Vec<u8>,
        index: Vec<u8>,
        pins: Pins,
    }

    fn fixture(name: &str) -> Fixture {
        let root = scratch(name);
        let jar = b"client jar bytes".to_vec();
        let object = b"asset object bytes";
        let hash = digest(object);
        let index = serde_json::to_vec(&serde_json::json!({
            "objects": {
                "minecraft/textures/block/stone.png": {"hash": hash, "size": object.len()}
            }
        }))
        .unwrap();
        let version = serde_json::to_vec(&serde_json::json!({
            "id": "26.3",
            "downloads": {"client": {"sha1": digest(&jar), "size": jar.len()}},
            "assetIndex": {"id": "34", "sha1": digest(&index), "size": index.len()}
        }))
        .unwrap();
        let pins = Pins {
            version: "26.3".into(),
            manifests: HashSet::from([digest(&version)]),
            client: digest(&jar),
            indexes: HashSet::from([digest(&index)]),
        };
        let write = |path: &str, bytes: &[u8]| {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        };
        write("versions/26.3/26.3.json", &version);
        write("versions/26.3/26.3.jar", &jar);
        write("assets/indexes/34.json", &index);
        write(&format!("assets/objects/{}/{}", &hash[..2], hash), object);
        Fixture {
            root,
            jar,
            index,
            pins,
        }
    }

    fn object_path(root: &Path) -> PathBuf {
        fs::read_dir(root.join("assets/objects"))
            .unwrap()
            .flatten()
            .flat_map(|dir| fs::read_dir(dir.path()).unwrap().flatten())
            .next()
            .unwrap()
            .path()
    }

    #[test]
    fn imports_the_launcher_layout_into_the_installer_cache() {
        let fixture = fixture("folder");
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(false);
        let mut source = DirectorySource::new(&fixture.root);
        let mut last = Progress::default();
        let report =
            import_inner(&mut source, &cache, &cancel, &fixture.pins, |p| last = p).unwrap();
        assert_eq!(report.files_copied, 3);
        assert_eq!(fs::read(cache.join("client.jar")).unwrap(), fixture.jar);
        assert_eq!(
            fs::read(cache.join("asset-index.json")).unwrap(),
            fixture.index
        );
        assert_eq!(last.files_done, 3);
        assert_eq!(last.done, last.total);
    }

    #[test]
    fn a_repeated_import_reuses_every_verified_cache_file() {
        let fixture = fixture("repeat");
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(false);
        let mut source = DirectorySource::new(&fixture.root);
        import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).unwrap();
        let report = import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).unwrap();
        assert_eq!(report.files_copied, 0);
        assert_eq!(report.files_reused, 3);
    }

    #[test]
    fn a_retry_reuses_cache_when_the_source_files_are_gone() {
        let fixture = fixture("source-gone");
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(false);
        let mut source = DirectorySource::new(&fixture.root);
        import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).unwrap();
        fs::remove_file(fixture.root.join("versions/26.3/26.3.jar")).unwrap();
        fs::remove_file(object_path(&fixture.root)).unwrap();
        let report = import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).unwrap();
        assert_eq!(report.files_missing, 0);
        assert_eq!(report.files_reused, 3);
    }

    #[test]
    fn a_corrupt_source_cannot_replace_a_verified_cache_file() {
        let fixture = fixture("corrupt-cache");
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(false);
        let mut source = DirectorySource::new(&fixture.root);
        import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).unwrap();
        fs::write(fixture.root.join("versions/26.3/26.3.jar"), b"corrupt jar!").unwrap();
        let report = import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).unwrap();
        assert_eq!(report.files_reused, 3);
        assert_eq!(fs::read(cache.join("client.jar")).unwrap(), fixture.jar);
    }

    #[test]
    fn a_corrupt_source_leaves_no_partial_file_in_an_empty_cache() {
        let fixture = fixture("corrupt-source");
        fs::write(fixture.root.join("versions/26.3/26.3.jar"), b"corrupt jar!").unwrap();
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(false);
        let mut source = DirectorySource::new(&fixture.root);
        let error = import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {})
            .unwrap_err()
            .to_string();
        assert!(error.contains("corrupt"), "{error}");
        assert!(!cache.join("client.jar").exists());
        assert!(!cache.join("client.partial").exists());
    }

    #[test]
    fn missing_objects_are_reported_and_left_for_the_installer() {
        let fixture = fixture("missing");
        fs::remove_file(object_path(&fixture.root)).unwrap();
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(false);
        let mut source = DirectorySource::new(&fixture.root);
        let report = import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).unwrap();
        assert_eq!(report.files_missing, 1);
        assert!(!cache.join("objects").read_dir().unwrap().any(|_| true));
    }

    #[test]
    fn zip_import_accepts_a_minecraft_folder_prefix() {
        let fixture = fixture("zip");
        let archive = fixture.root.join("install.zip");
        let file = fs::File::create(&archive).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in [
            (
                ".minecraft/versions/26.3/26.3.json",
                fs::read(fixture.root.join("versions/26.3/26.3.json")).unwrap(),
            ),
            (".minecraft/versions/26.3/26.3.jar", fixture.jar.clone()),
            (".minecraft/assets/indexes/34.json", fixture.index.clone()),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(&bytes).unwrap();
        }
        let object = object_path(&fixture.root);
        let name = object.file_name().unwrap().to_string_lossy();
        zip.start_file(
            format!(".minecraft/assets/objects/{}/{}", &name[..2], name),
            options,
        )
        .unwrap();
        zip.write_all(&fs::read(&object).unwrap()).unwrap();
        zip.finish().unwrap();
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(false);
        let mut source = ZipSource::new(fs::File::open(archive).unwrap()).unwrap();
        let report = import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).unwrap();
        assert_eq!(report.files_copied, 3);
    }

    #[test]
    fn cancelling_keeps_verified_files_and_removes_partials() {
        let fixture = fixture("cancel");
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(true);
        let mut source = DirectorySource::new(&fixture.root);
        let error = import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {})
            .unwrap_err()
            .to_string();
        assert_eq!(error, "Import cancelled");
        assert!(!cache.join("client.partial").exists());
    }

    #[test]
    fn remove_deletes_only_minecraft_resources() {
        let fixture = fixture("remove");
        let home = fixture.root.join("home");
        let prepared = home.join(".local/horizon-client/26.3");
        fs::create_dir_all(&prepared).unwrap();
        fs::write(prepared.join("horizon-resources.json"), b"{}").unwrap();
        fs::create_dir_all(home.join("saves/world")).unwrap();
        fs::write(home.join("saves/world/level.dat"), b"world").unwrap();
        fs::write(home.join("engine-data.zip"), b"engine").unwrap();
        remove(&home).unwrap();
        assert!(!prepared.exists());
        assert!(home.join("saves/world/level.dat").is_file());
        assert!(home.join("engine-data.zip").is_file());
    }

    #[test]
    fn unsafe_index_names_are_rejected() {
        assert!(!safe_component("../evil"));
        assert!(!safe_component("a/b"));
        assert!(!safe_component(""));
        assert!(safe_component("34"));
        assert!(safe_relative("assets/objects/aa/bb").is_ok());
        assert!(safe_relative("assets/../../escape").is_err());
    }

    #[test]
    fn zip_names_are_normalised_without_allowing_traversal() {
        assert_eq!(
            normalize_zip_name("./.minecraft/versions/26.3/26.3.json").as_deref(),
            Some(".minecraft/versions/26.3/26.3.json")
        );
        assert_eq!(
            normalize_zip_name(".minecraft\\assets\\objects\\aa\\bb").as_deref(),
            Some(".minecraft/assets/objects/aa/bb")
        );
        assert_eq!(
            normalize_zip_name("assets/./objects/../indexes/34.json"),
            None
        );
        assert_eq!(normalize_zip_name("../outside.txt"), None);
        assert_eq!(normalize_zip_name("assets/../../outside.txt"), None);
        assert_eq!(normalize_zip_name("./"), None);
    }

    #[test]
    fn an_invalid_zip_is_rejected_without_touching_the_cache() {
        let root = scratch("invalid-zip");
        let archive = root.join("bad.zip");
        fs::write(&archive, b"this is not a zip").unwrap();
        let error = ZipSource::new(fs::File::open(archive).unwrap())
            .err()
            .expect("invalid ZIP must be rejected")
            .to_string();
        assert!(!error.is_empty());
        assert!(!root.join("cache").exists());
    }

    #[test]
    fn a_failed_import_leaves_an_existing_installation_untouched() {
        let fixture = fixture("existing");
        let home = fixture.root.join("home");
        let sentinel = home.join(".local/horizon-client/26.3/sentinel");
        fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
        fs::write(&sentinel, b"keep me").unwrap();
        fs::remove_file(fixture.root.join("versions/26.3/26.3.json")).unwrap();
        let cache = fixture.root.join("cache");
        let cancel = AtomicBool::new(false);
        let mut source = DirectorySource::new(&fixture.root);
        assert!(import_inner(&mut source, &cache, &cancel, &fixture.pins, |_| {}).is_err());
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep me");
    }

    #[test]
    fn import_record_can_be_restored_after_repair() {
        let root = scratch("record");
        let saved = ImportRecord {
            schema: 1,
            source: "Minecraft folder".into(),
            imported_unix: 42,
            files_copied: 7,
            files_reused: 3,
            files_missing: 1,
        };
        restore_record(&root, &saved).unwrap();
        assert_eq!(record(&root), Some(saved));
    }
}
