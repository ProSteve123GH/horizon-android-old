//! Installing Minecraft 26.3 from files the player already owns.
//!
//! A selected folder or archive may be a Java `.minecraft` layout
//! (`versions/26.3/26.3.jar`, `assets/indexes/*.json`, `assets/objects/`), a
//! previous Horizon installer cache (`client.jar`, `asset-index.json`,
//! `objects/`), or any recursive mix of those files. Only the pinned 26.3
//! client, an accepted 26.3 asset index and assets named by that index are used.
use super::*;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_SCAN_FILES: usize = 200_000;
const MAX_SCAN_DEPTH: usize = 32;
const MAX_INDEX_BYTES: u64 = 64 << 20;
const MAX_FILE_BYTES: u64 = 1 << 30;
const MAX_TOTAL_BYTES: u64 = 8 << 30;
const MAX_ARCHIVE_ENTRIES: usize = 200_000;
const MAX_ARCHIVE_PATH: usize = 1024;

struct Pins {
    client: String,
    indexes: Vec<String>,
}

struct Found {
    client: Option<PathBuf>,
    index: Option<Value>,
    objects: HashMap<String, PathBuf>,
}

#[derive(Clone, Copy)]
enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    TarXz,
}

impl ArchiveKind {
    fn from_name(name: &str) -> Result<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".zip") {
            Ok(Self::Zip)
        } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Ok(Self::TarGz)
        } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
            Ok(Self::TarXz)
        } else if lower.ends_with(".tar") {
            Ok(Self::Tar)
        } else {
            Err("Choose a .zip, .tar, .tar.gz or .tar.xz archive".into())
        }
    }
}

fn pins() -> Result<Pins> {
    let value: Value = serde_json::from_str(LOCK)?;
    let minecraft = &value["minecraft"];
    let client = minecraft["client_sha1"]
        .as_str()
        .ok_or("Missing pinned client checksum")?
        .to_owned();
    let indexes = minecraft["accepted_asset_index_sha1"]
        .as_array()
        .ok_or("Missing accepted asset index checksums")?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    if indexes.is_empty() {
        return Err("No accepted Minecraft asset index checksum".into());
    }
    Ok(Pins { client, indexes })
}

fn is_sha1(name: &str) -> bool {
    name.len() == 40 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn numeric_json(name: &str) -> bool {
    name.strip_suffix(".json")
        .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|byte| byte.is_ascii_digit()))
}

fn likely_json(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name == "asset-index.json"
        || numeric_json(name)
        || path
            .components()
            .any(|component| component.as_os_str() == "indexes")
}

fn likely_client(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower == "client.jar"
        || lower == "26.3.jar"
        || path
            .components()
            .any(|component| component.as_os_str() == "versions")
}

fn ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "backups"
            | "cache"
            | "crash-reports"
            | "downloads"
            | "logs"
            | "mods"
            | "resourcepacks"
            | "saves"
            | "screenshots"
            | "shaderpacks"
    )
}

fn scan(root: &Path, pins: &Pins) -> Result<Found> {
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Choose a folder, not a single file".into());
    }
    let mut jars = Vec::new();
    let mut indexes = Vec::new();
    let mut objects = HashMap::new();
    let mut files = 0usize;
    walk(root, 0, &mut files, &mut jars, &mut indexes, &mut objects)?;
    let client = choose_client(jars, pins)?;
    let index = choose_index(indexes, &objects, pins)?;
    if client.is_none() && index.is_none() {
        return Err(
            "No Minecraft 26.3 client or asset index was found. Select a .minecraft folder, \
             a folder containing 26.3.jar/client.jar, or a folder with assets/indexes."
                .into(),
        );
    }
    Ok(Found {
        client,
        index,
        objects,
    })
}

fn walk(
    directory: &Path,
    depth: usize,
    files: &mut usize,
    jars: &mut Vec<PathBuf>,
    indexes: &mut Vec<PathBuf>,
    objects: &mut HashMap<String, PathBuf>,
) -> Result<()> {
    if depth > MAX_SCAN_DEPTH {
        return Err("Minecraft file folders are nested too deeply".into());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with('.') && name != ".minecraft" {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if !ignored_dir(name) {
                walk(&path, depth + 1, files, jars, indexes, objects)?;
            }
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        *files += 1;
        if *files > MAX_SCAN_FILES {
            return Err("Selected files contain too many entries".into());
        }
        if metadata.len() > MAX_FILE_BYTES {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".jar") && likely_client(&path) {
            jars.push(path.clone());
        }
        if lower.ends_with(".json") && metadata.len() <= MAX_INDEX_BYTES && likely_json(&path) {
            indexes.push(path.clone());
        }
        if is_sha1(name) {
            objects.entry(lower).or_insert(path);
        }
    }
    Ok(())
}

fn choose_client(mut jars: Vec<PathBuf>, pins: &Pins) -> Result<Option<PathBuf>> {
    jars.sort_by_key(|path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if name == "client.jar" || name == "26.3.jar" {
            0
        } else if path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            == Some(horizon_core::VANILLA_VERSION)
        {
            1
        } else {
            2
        }
    });
    for path in jars.into_iter().take(128) {
        if hash(fs::File::open(&path)?)? == pins.client {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn choose_index(
    mut paths: Vec<PathBuf>,
    objects: &HashMap<String, PathBuf>,
    pins: &Pins,
) -> Result<Option<Value>> {
    paths.sort_by_key(|path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name == "asset-index.json" {
            0
        } else if numeric_json(name) {
            1
        } else {
            2
        }
    });
    let mut accepted = Vec::new();
    for path in paths.into_iter().take(256) {
        let bytes = match fs::read(&path) {
            Ok(bytes) if bytes.len() as u64 <= MAX_INDEX_BYTES => bytes,
            _ => continue,
        };
        if !pins
            .indexes
            .iter()
            .any(|expected| format!("{:x}", Sha1::digest(&bytes)) == *expected)
        {
            continue;
        }
        if let Ok(index) = serde_json::from_slice::<Value>(&bytes) {
            accepted.push(index);
            if accepted.len() == pins.indexes.len() {
                break;
            }
        }
    }
    if accepted.is_empty() {
        return Ok(None);
    }
    let mut best = None;
    for index in accepted {
        let Some(entries) = index["objects"].as_object() else {
            continue;
        };
        let missing = entries
            .iter()
            .filter(|(name, _)| needed_asset(name))
            .filter(|(_, object)| {
                object["hash"]
                    .as_str()
                    .is_none_or(|hash| !objects.contains_key(&hash.to_ascii_lowercase()))
            })
            .count();
        if best.as_ref().is_none_or(|(_, count)| missing < *count) {
            best = Some((index, missing));
        }
    }
    Ok(best.map(|(index, _)| index))
}

fn install_found(
    home: &Path,
    found: Found,
    pins: &Pins,
    progress: impl Fn(InstallProgress) + Sync,
) -> Result<()> {
    let mut client = found.client;
    let mut index = found.index;
    if client.is_none() || index.is_none() {
        let (remote_client, remote_index) = remote_downloads(home)?;
        if client.is_none() {
            progress(InstallProgress {
                stage: InstallStage::Download,
                done: 0,
                total: remote_client.size,
            });
            remote_client.get()?;
            progress(InstallProgress {
                stage: InstallStage::Download,
                done: remote_client.size,
                total: remote_client.size,
            });
            client = Some(remote_client.path);
        }
        if index.is_none() {
            progress(InstallProgress {
                stage: InstallStage::Download,
                done: 0,
                total: remote_index.size,
            });
            remote_index.get()?;
            progress(InstallProgress {
                stage: InstallStage::Download,
                done: remote_index.size,
                total: remote_index.size,
            });
            index = Some(serde_json::from_reader(remote_index.open()?)?);
        }
    }
    let client = client.ok_or("Minecraft client JAR is unavailable")?;
    let index = index.ok_or("Minecraft asset index is unavailable")?;
    let mut objects = found.objects;
    ensure_objects(home, &index, &mut objects, &progress)?;
    install_inputs_locked(
        home,
        Inputs {
            client,
            client_sha1: pins.client.clone(),
            index,
            objects,
        },
        progress,
    )
}

fn remote_downloads(home: &Path) -> Result<(Download, Download)> {
    let lock: Value = serde_json::from_str(LOCK)?;
    let pins = &lock["minecraft"];
    let version = horizon_core::VANILLA_VERSION;
    let catalog = json(CATALOG)?;
    let entry = catalog["versions"]
        .as_array()
        .and_then(|versions| versions.iter().find(|entry| entry["id"] == version))
        .ok_or("Pinned Minecraft version is not listed by Mojang")?;
    let checksum = entry["sha1"]
        .as_str()
        .ok_or("Missing version metadata hash")?;
    let manifest_url = entry["url"]
        .as_str()
        .ok_or("Missing version metadata URL")?;
    let manifest = validate_manifest(&metadata_bytes(manifest_url)?, checksum, version, pins)?;
    let cache = download_cache(home);
    fs::create_dir_all(&cache)?;
    let client = Download::from(&manifest["downloads"]["client"], cache.join("client.jar"))?;
    let index = Download::from(&manifest["assetIndex"], cache.join("asset-index.json"))?;
    if !pins["accepted_asset_index_sha1"]
        .as_array()
        .is_some_and(|accepted| {
            accepted
                .iter()
                .any(|value| value.as_str() == Some(index.sha1.as_str()))
        })
    {
        return Err("Mojang's 26.3 asset index is not an accepted build".into());
    }
    Ok((client, index))
}

/// Validate what the selected files provide and download only the 26.3 assets
/// that are absent or fail their checksum. A deleted object costs one object,
/// not another full Minecraft download.
fn missing_objects(
    index: &Value,
    objects: &HashMap<String, PathBuf>,
) -> Result<Vec<(String, u64)>> {
    let entries = index["objects"]
        .as_object()
        .ok_or("Minecraft asset index has no objects")?;
    let mut missing = HashMap::new();
    for (name, object) in entries {
        if !needed_asset(name) {
            continue;
        }
        let checksum = object["hash"]
            .as_str()
            .ok_or("Missing asset hash")?
            .to_ascii_lowercase();
        let size = object["size"]
            .as_u64()
            .filter(|size| *size <= MAX_FILE_BYTES)
            .ok_or("Invalid asset size")?;
        if checksum.len() != 40 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Invalid asset hash".into());
        }
        if objects
            .get(&checksum)
            .is_some_and(|path| valid_object(path, &checksum, size))
        {
            continue;
        }
        missing.insert(checksum, size);
    }
    let mut missing = missing.into_iter().collect::<Vec<_>>();
    missing.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(missing)
}

fn ensure_objects(
    home: &Path,
    index: &Value,
    objects: &mut HashMap<String, PathBuf>,
    progress: &(impl Fn(InstallProgress) + Sync),
) -> Result<()> {
    let missing = missing_objects(index, objects)?;
    if missing.is_empty() {
        return Ok(());
    }
    let total = missing
        .iter()
        .try_fold(0u64, |total, (_, size)| total.checked_add(*size))
        .filter(|total| *total <= MAX_TOTAL_BYTES)
        .ok_or("Missing Minecraft assets exceed 8 GiB")?;
    progress(InstallProgress {
        stage: InstallStage::Download,
        done: 0,
        total,
    });
    let cache = download_cache(home);
    fs::create_dir_all(cache.join("objects"))?;
    let downloads = missing
        .into_iter()
        .map(|(checksum, size)| {
            Download::from(
                &serde_json::json!({
                    "url": format!(
                        "https://resources.download.minecraft.net/{}/{checksum}",
                        &checksum[..2]
                    ),
                    "sha1": checksum.as_str(),
                    "size": size,
                }),
                cache.join("objects").join(checksum.as_str()),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let next = AtomicUsize::new(0);
    let done = AtomicU64::new(0);
    let error = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                loop {
                    if error.lock().unwrap().is_some() {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(download) = downloads.get(index) else {
                        break;
                    };
                    if let Err(failure) = download.get() {
                        *error.lock().unwrap() = Some(failure.to_string());
                        break;
                    }
                    progress(InstallProgress {
                        stage: InstallStage::Download,
                        done: done.fetch_add(download.size, Ordering::Relaxed) + download.size,
                        total,
                    });
                }
            });
        }
    });
    if let Some(error) = error.into_inner().unwrap() {
        return Err(error.into());
    }
    for download in downloads {
        objects.insert(download.sha1, download.path);
    }
    Ok(())
}

fn valid_object(path: &Path, checksum: &str, size: u64) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != size {
        return false;
    }
    fs::File::open(path)
        .ok()
        .and_then(|file| hash(file).ok())
        .is_some_and(|actual| actual == checksum)
}

pub(super) fn install_directory(
    home: &Path,
    source: &Path,
    progress: impl Fn(InstallProgress) + Sync,
) -> Result<()> {
    let _guard = INSTALLATION.lock().unwrap_or_else(|e| e.into_inner());
    progress(InstallProgress {
        stage: InstallStage::Metadata,
        done: 0,
        total: 0,
    });
    let pins = pins()?;
    let found = scan(source, &pins)?;
    install_found(home, found, &pins, progress)
}

pub(super) fn install_archive(
    home: &Path,
    source: fs::File,
    name: &str,
    progress: impl Fn(InstallProgress) + Sync,
) -> Result<()> {
    let _guard = INSTALLATION.lock().unwrap_or_else(|e| e.into_inner());
    let kind = ArchiveKind::from_name(name)?;
    progress(InstallProgress {
        stage: InstallStage::Metadata,
        done: 0,
        total: 0,
    });
    let temporary = temporary_directory(home, "archive")?;
    let result = (|| -> Result<()> {
        extract_archive(source, kind, &temporary)?;
        let pins = pins()?;
        let found = scan(&temporary, &pins)?;
        install_found(home, found, &pins, progress)
    })();
    let _ = fs::remove_dir_all(&temporary);
    result
}

fn temporary_directory(home: &Path, label: &str) -> Result<PathBuf> {
    let base = home.join(".local/imports");
    fs::create_dir_all(&base)?;
    let metadata = fs::symlink_metadata(&base)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Refusing a redirected Minecraft import folder".into());
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = base.join(format!("{label}-{}-{stamp}", std::process::id()));
    fs::create_dir(&path)?;
    Ok(path)
}

fn wanted_archive_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    (lower.ends_with(".jar") && likely_client(path))
        || (lower.ends_with(".json")
            && (name == "asset-index.json"
                || numeric_json(name)
                || path
                    .components()
                    .any(|component| component.as_os_str() == "indexes")))
        || is_sha1(name)
}

fn safe_path(root: &Path, path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path.as_os_str().len() > MAX_ARCHIVE_PATH
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err("Archive contains an unsafe path".into());
    }
    let mut result = root.to_path_buf();
    for component in path.components() {
        if let Component::Normal(name) = component {
            result.push(name);
        }
    }
    if !result.starts_with(root) {
        return Err("Archive path escapes the import folder".into());
    }
    Ok(result)
}

fn copy_entry(
    mut input: impl Read,
    output: &mut fs::File,
    declared: u64,
    total: &mut u64,
) -> Result<()> {
    let mut buffer = [0; 65536];
    let mut copied = 0u64;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        copied += count as u64;
        if copied > declared {
            return Err("Archive entry exceeds its declared size".into());
        }
        *total = (*total)
            .checked_add(count as u64)
            .filter(|total| *total <= MAX_TOTAL_BYTES)
            .ok_or("Archive expands beyond 8 GiB")?;
        output.write_all(&buffer[..count])?;
    }
    if copied != declared {
        return Err("Archive entry is incomplete".into());
    }
    Ok(())
}

fn extract_archive(source: fs::File, kind: ArchiveKind, destination: &Path) -> Result<()> {
    match kind {
        ArchiveKind::Zip => extract_zip(source, destination),
        ArchiveKind::Tar => extract_tar(source, destination),
        ArchiveKind::TarGz => extract_tar(flate2::read::GzDecoder::new(source), destination),
        ArchiveKind::TarXz => {
            let tar = destination.with_extension("tar");
            let file = fs::File::create(&tar)?;
            let mut writer = LimitedWriter {
                inner: file,
                limit: MAX_TOTAL_BYTES,
            };
            let result = (|| -> Result<()> {
                lzma_rs::xz_decompress(&mut BufReader::new(source), &mut writer)
                    .map_err(|error| format!("Could not decompress .tar.xz: {error}"))?;
                extract_tar(fs::File::open(&tar)?, destination)
            })();
            let _ = fs::remove_file(&tar);
            result
        }
    }
}

fn extract_zip(source: fs::File, destination: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(source)?;
    let mut names = HashSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        if index > MAX_ARCHIVE_ENTRIES {
            return Err("Archive contains too many entries".into());
        }
        let mut entry = archive.by_index(index)?;
        let path = entry
            .enclosed_name()
            .ok_or("Archive contains an unsafe path")?;
        if entry.is_dir() {
            continue;
        }
        if entry.is_symlink()
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Archive contains a symbolic link".into());
        }
        if !wanted_archive_path(&path) {
            continue;
        }
        if !names.insert(path.clone()) {
            return Err("Archive contains duplicate paths".into());
        }
        let target = safe_path(destination, &path)?;
        fs::create_dir_all(target.parent().ok_or("Missing archive parent")?)?;
        let declared = entry.size();
        let mut output = fs::File::create(&target)?;
        copy_entry(&mut entry, &mut output, declared, &mut total)?;
    }
    Ok(())
}

fn extract_tar(mut source: impl Read, destination: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(&mut source);
    let mut names = HashSet::new();
    let mut total = 0u64;
    let mut entries = 0usize;
    for entry in archive.entries()? {
        entries += 1;
        if entries > MAX_ARCHIVE_ENTRIES {
            return Err("Archive contains too many entries".into());
        }
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        if !kind.is_file() {
            return Err("Archive contains a link or special file".into());
        }
        if !wanted_archive_path(&path) {
            continue;
        }
        if !names.insert(path.clone()) {
            return Err("Archive contains duplicate paths".into());
        }
        let target = safe_path(destination, &path)?;
        fs::create_dir_all(target.parent().ok_or("Missing archive parent")?)?;
        let declared = entry.size();
        let mut output = fs::File::create(&target)?;
        copy_entry(&mut entry, &mut output, declared, &mut total)?;
    }
    Ok(())
}

struct LimitedWriter<W> {
    inner: W,
    limit: u64,
}

impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() as u64 > self.limit {
            return Err(std::io::Error::other("Decompressed archive is too large"));
        }
        self.limit -= bytes.len() as u64;
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha1::{Digest, Sha1};
    use std::io::Write;

    struct Temporary(PathBuf);
    impl Temporary {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "horizon-import-test-{name}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha1::digest(bytes))
    }

    fn fixture(root: &Path) -> (Pins, Vec<u8>, Vec<u8>) {
        let client = b"not a real jar, but pinned for the scanner".to_vec();
        let object = b"fake asset bytes".to_vec();
        let object_hash = digest(&object);
        let index = serde_json::json!({
            "objects": {
                "minecraft/textures/fake.png": {
                    "hash": object_hash,
                    "size": object.len(),
                }
            }
        });
        let index_bytes = serde_json::to_vec(&index).unwrap();
        let pins = Pins {
            client: digest(&client),
            indexes: vec![digest(&index_bytes)],
        };
        let minecraft = root.join(".minecraft");
        fs::create_dir_all(minecraft.join("versions/26.3")).unwrap();
        fs::create_dir_all(minecraft.join("assets/indexes")).unwrap();
        fs::create_dir_all(minecraft.join(format!("assets/objects/{}", &object_hash[..2])))
            .unwrap();
        fs::write(minecraft.join("versions/26.3/26.3.jar"), &client).unwrap();
        fs::write(minecraft.join("assets/indexes/34.json"), &index_bytes).unwrap();
        fs::write(
            minecraft.join(format!("assets/objects/{object_hash}")),
            &object,
        )
        .unwrap();
        (pins, client, index_bytes)
    }

    #[test]
    fn finds_a_nested_java_layout_and_checks_the_index_hash() {
        let root = Temporary::new("nested");
        let (pins, _, _) = fixture(&root.0);
        let found = scan(&root.0, &pins).unwrap();
        assert!(
            found
                .client
                .as_deref()
                .is_some_and(|path| path.ends_with("26.3.jar"))
        );
        assert_eq!(found.objects.len(), 1);
    }

    #[test]
    fn deleted_assets_are_reported_as_missing_not_as_a_broken_index() {
        let root = Temporary::new("missing");
        let (pins, _, _) = fixture(&root.0);
        let found = scan(&root.0, &pins).unwrap();
        assert!(
            missing_objects(found.index.as_ref().unwrap(), &found.objects)
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root.0.join(".minecraft/assets/objects")).unwrap();
        let found = scan(&root.0, &pins).unwrap();
        assert_eq!(
            missing_objects(found.index.as_ref().unwrap(), &found.objects)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn corrupted_assets_are_reported_as_missing_so_only_they_are_downloaded() {
        let root = Temporary::new("corrupt");
        let (pins, _, _) = fixture(&root.0);
        let object = root.0.join(".minecraft/assets/objects");
        let bucket = fs::read_dir(&object)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let path = fs::read_dir(bucket)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(path, b"short").unwrap();
        let found = scan(&root.0, &pins).unwrap();
        assert_eq!(
            missing_objects(found.index.as_ref().unwrap(), &found.objects)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn accepts_the_supported_archive_names_case_insensitively() {
        for name in [
            "files.zip",
            "files.tar",
            "files.tar.gz",
            "files.TAR.GZ",
            "files.tgz",
            "files.tar.xz",
            "files.txz",
        ] {
            assert!(ArchiveKind::from_name(name).is_ok(), "{name}");
        }
        assert!(ArchiveKind::from_name("files.rar").is_err());
    }

    #[test]
    fn extracts_only_the_minecraft_files_from_a_zip() {
        let root = Temporary::new("zip");
        let (pins, client, index) = fixture(&root.0);
        let source = root.0.join("input.zip");
        let object_hash = pins.client.clone();
        let mut archive = zip::ZipWriter::new(fs::File::create(&source).unwrap());
        archive
            .start_file("backup/26.3.jar", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&client).unwrap();
        archive
            .start_file(
                "backup/assets/indexes/34.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(&index).unwrap();
        archive
            .start_file(
                format!("backup/assets/objects/{object_hash}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(&client).unwrap();
        archive
            .start_file(
                "backup/saves/world/level.dat",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"not imported").unwrap();
        archive.finish().unwrap();
        let output = root.0.join("out");
        fs::create_dir(&output).unwrap();
        extract_archive(fs::File::open(&source).unwrap(), ArchiveKind::Zip, &output).unwrap();
        assert!(output.join("backup/26.3.jar").is_file());
        assert!(!output.join("backup/saves/world/level.dat").exists());
    }

    #[test]
    fn tar_extraction_rejects_links() {
        let root = Temporary::new("tar-link");
        let source = root.0.join("input.tar");
        let mut archive = tar::Builder::new(fs::File::create(&source).unwrap());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_cksum();
        archive
            .append_data(&mut header, "26.3.jar", std::io::empty())
            .unwrap();
        archive.finish().unwrap();
        let output = root.0.join("out");
        fs::create_dir(&output).unwrap();
        let error = extract_archive(fs::File::open(&source).unwrap(), ArchiveKind::Tar, &output)
            .unwrap_err()
            .to_string();
        assert!(error.contains("link or special file"), "{error}");
    }
}
