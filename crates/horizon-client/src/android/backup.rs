//! Selective player-data backups for the Android launcher.
//!
//! A backup is a standard `.tar.gz` the user can open with any file manager.
//! Only the listed player-data trees are archived; account sessions, logs and
//! prepared engine files are never included. Restore is deliberately strict:
//! entries must stay under the allowed roots, links and traversal are refused,
//! and existing files are only replaced when the caller says so explicitly.
//!
//! Kept dependency-light so `tools/test_android_backup.py` can compile it
//! standalone: `flate2` and `serde_json` only.
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// The backup file prefix; a UTC millisecond timestamp is appended.
pub const PREFIX: &str = "horizon-backup-";
const SUFFIX: &str = ".tar.gz";
const BLOCK: usize = 512;
const MAGIC: &[u8; 6] = b"ustar\0";

/// Selectable groups, in the order the launcher shows them.
pub const CATEGORIES: [(&str, &str, &str); 5] = [
    ("instances", "Instances", "Worlds, per-instance settings, mods, packs and shaders"),
    ("controls", "Controls", "Saved touch-layout profiles"),
    ("launcher", "Launcher settings", "Language, theme and preferences"),
    ("screenshots", "Screenshots", "Original PNGs from every instance"),
    ("reports", "Reports", "Shareable performance and diagnostic reports"),
];

pub fn categories() -> Value {
    Value::Array(
        CATEGORIES
            .iter()
            .map(|(id, label, detail)| json!({"id": id, "label": label, "detail": detail}))
            .collect(),
    )
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

fn selected(set: &Value, key: &str) -> bool {
    set.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn private_entry(name: &str) -> bool {
    name.starts_with('.')
        || matches!(name, "sessions" | "logs" | "crash-reports" | "cache" | ".cache")
}

/// Walks a source tree into `(on-disk path, archive name)` pairs.
fn collect_tree(source: &Path, prefix: &str, jobs: &mut Vec<(PathBuf, String)>) -> Result<(), String> {
    if !source.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(source)
        .map_err(|e| format!("Cannot read {}: {e}", source.display()))?
        .flatten()
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let kind = entry
            .file_type()
            .map_err(|e| format!("Cannot inspect {}: {e}", entry.path().display()))?;
        if kind.is_symlink() || private_entry(&name) {
            continue;
        }
        let path = entry.path();
        let archived = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if kind.is_dir() {
            jobs.push((path.clone(), archived.clone()));
            collect_tree(&path, &archived, jobs)?;
        } else if kind.is_file() {
            jobs.push((path, archived));
        }
    }
    Ok(())
}

fn collect_jobs(home: &Path, set: &Value) -> Result<Vec<(PathBuf, String)>, String> {
    let mut jobs = Vec::new();
    if selected(set, "instances") {
        collect_tree(&home.join("Instances"), "Instances", &mut jobs)?;
    }
    if selected(set, "controls") {
        collect_tree(&home.join("Controls"), "Controls", &mut jobs)?;
    }
    if selected(set, "launcher") {
        let file = home.join("android-launcher.json");
        if file.is_file() {
            jobs.push((file, "android-launcher.json".into()));
        }
    }
    if selected(set, "reports") {
        collect_tree(&home.join("performance"), "performance", &mut jobs)?;
    }
    if selected(set, "screenshots") {
        // Screenshots live inside instances; include only that folder so a
        // screenshots-only backup can be restored without overwriting worlds.
        let instances = home.join("Instances");
        let Ok(entries) = fs::read_dir(&instances) else {
            return Ok(jobs);
        };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let shots = entry.path().join("screenshots");
            collect_tree(&shots, &format!("Instances/{name}/screenshots"), &mut jobs)?;
        }
    }
    Ok(jobs)
}

fn put_octal(field: &mut [u8], value: u64) {
    let text = format!("{value:o}");
    let bytes = text.as_bytes();
    let len = field.len();
    let keep = bytes.len().min(len.saturating_sub(1));
    field[..len - 1].fill(b'0');
    field[len - 1] = 0;
    field[len - 1 - keep..len - 1].copy_from_slice(&bytes[bytes.len() - keep..]);
}

fn header(name: &str, size: u64, directory: bool) -> Result<[u8; BLOCK], String> {
    let mut out = [0u8; BLOCK];
    let bytes = name.as_bytes();
    if bytes.len() > 255 {
        return Err(format!("Backup path is too long: {name}"));
    }
    if bytes.len() <= 100 {
        out[..bytes.len()].copy_from_slice(bytes);
    } else {
        // USTAR splits long names at a slash into prefix + name.
        let split = bytes
            .iter()
            .enumerate()
            .take(155)
            .rev()
            .find(|(_, byte)| **byte == b'/')
            .map(|(index, _)| index)
            .ok_or_else(|| format!("Backup path has no split point: {name}"))?;
        out[345..345 + split].copy_from_slice(&bytes[..split]);
        out[..bytes.len() - split - 1].copy_from_slice(&bytes[split + 1..]);
    }
    put_octal(&mut out[100..108], if directory { 0o755 } else { 0o644 });
    put_octal(&mut out[108..116], 0);
    put_octal(&mut out[116..124], 0);
    put_octal(&mut out[124..136], size);
    put_octal(&mut out[136..148], 0);
    out[148..156].fill(b' '); // checksum is computed with this field blank
    out[156] = if directory { b'5' } else { b'0' };
    out[257..263].copy_from_slice(MAGIC);
    out[263..265].copy_from_slice(b"00");
    let sum: u64 = out.iter().map(|byte| u64::from(*byte)).sum();
    put_octal(&mut out[148..156], sum);
    Ok(out)
}

fn write_padded<W: Write>(out: &mut W, bytes: &[u8]) -> io::Result<()> {
    out.write_all(bytes)?;
    let pad = (BLOCK - bytes.len() % BLOCK) % BLOCK;
    if pad > 0 {
        out.write_all(&vec![0u8; pad])?;
    }
    Ok(())
}

fn archive_name(name: &str) -> String {
    format!("{PREFIX}{name}{SUFFIX}")
}

/// Creates a `.tar.gz` in `destination` from the selected groups.
pub fn create(home: &Path, selection: &Value, destination: &Path) -> Result<String, String> {
    let set = selection;
    if CATEGORIES.iter().all(|(id, _, _)| !selected(set, id)) {
        return Err("Select at least one group to back up".into());
    }
    if !destination.is_dir() {
        return Err("Choose an existing folder to save the backup".into());
    }
    let jobs = collect_jobs(home, set)?;
    let path = destination.join(archive_name(&now_millis().to_string()));
    let file = File::create(&path).map_err(|e| format!("Cannot create the backup: {e}"))?;
    let mut out = GzEncoder::new(file, Compression::default());
    let mut bytes = 0u64;
    for (source, name) in jobs {
        let directory = source.is_dir();
        let size = if directory {
            0
        } else {
            fs::metadata(&source)
                .map_err(|e| format!("Cannot read {}: {e}", source.display()))?
                .len()
        };
        let head = header(&name, size, directory)?;
        out.write_all(&head)
            .map_err(|e| format!("Cannot write the backup header: {e}"))?;
        bytes += size;
        if !directory {
            let mut input =
                File::open(&source).map_err(|e| format!("Cannot open {}: {e}", source.display()))?;
            io::copy(&mut input, &mut out)
                .map_err(|e| format!("Cannot back up {}: {e}", source.display()))?;
            let copied = fs::metadata(&source).map_err(|e| e.to_string())?.len();
            if copied != size {
                return Err(format!("{} changed while it was being backed up", source.display()));
            }
            let pad = (BLOCK - (size as usize % BLOCK)) % BLOCK;
            if pad > 0 {
                out.write_all(&vec![0u8; pad])
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    out.write_all(&[0u8; BLOCK * 2])
        .map_err(|e| e.to_string())?;
    out.finish()
        .map_err(|e| format!("Cannot finish the backup: {e}"))?;
    Ok(path.to_string_lossy().into_owned())
}

fn read_octal(field: &[u8]) -> Option<u64> {
    let text = field
        .iter()
        .take_while(|byte| **byte != 0 && **byte != b' ')
        .map(|byte| *byte as char)
        .collect::<String>();
    u64::from_str_radix(text.trim(), 8).ok()
}

fn archive_path(name: &str) -> Option<PathBuf> {
    let text = name.trim_end_matches('\0');
    if text.is_empty() || text.starts_with('/') || text.contains('\\') || text.contains('\0') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in text.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.starts_with('.') {
            return None;
        }
        out.push(part);
    }
    // Only the player-data roots a backup can contain.
    let allowed = ["Instances", "Controls", "performance", "android-launcher.json"];
    out.components().next().and_then(|first| match first {
        Component::Normal(name) => allowed.contains(&name.to_str()?).then_some(()),
        _ => None,
    })?;
    Some(out)
}

fn checksum_ok(head: &[u8; BLOCK]) -> bool {
    let stored = match read_octal(&head[148..156]) {
        Some(value) => value,
        None => return false,
    };
    let mut sum = 0u64;
    for (index, byte) in head.iter().enumerate() {
        sum += if (148..156).contains(&index) {
            u64::from(b' ')
        } else {
            u64::from(*byte)
        };
    }
    sum == stored
}

/// Restores a backup. Existing destinations are refused unless `replace` is set.
pub fn restore(archive: &Path, home: &Path, replace: bool) -> Result<String, String> {
    if !archive.is_file() {
        return Err("The backup archive was not found".into());
    }
    let file = File::open(archive).map_err(|e| format!("Cannot open the backup: {e}"))?;
    let mut input = GzDecoder::new(file);
    let mut rest = Vec::new();
    input
        .read_to_end(&mut rest)
        .map_err(|e| format!("Cannot read the backup: {e}"))?;
    if rest.len() < BLOCK * 2 || rest.len() % BLOCK != 0 {
        return Err("That file is not a complete backup archive".into());
    }
    let mut files = 0usize;
    let mut offset = 0usize;
    while offset + BLOCK <= rest.len() {
        let head: &[u8; BLOCK] = rest[offset..offset + BLOCK].try_into().expect("block size");
        offset += BLOCK;
        if head.iter().all(|byte| *byte == 0) {
            break; // the two end-of-archive blocks
        }
        if !checksum_ok(head) {
            return Err("The backup has a damaged header; nothing was restored".into());
        }
        let name = String::from_utf8_lossy(&head[..100])
            .trim_end_matches('\0')
            .to_owned();
        let mut name = name;
        let prefix = String::from_utf8_lossy(&head[345..500])
            .trim_end_matches('\0')
            .to_owned();
        if !prefix.is_empty() {
            name = format!("{prefix}/{name}");
        }
        let path = archive_path(&name).ok_or_else(|| {
            format!("The backup contains an unsafe path ({name}); nothing was restored")
        })?;
        let size = read_octal(&head[124..136])
            .ok_or("The backup has an invalid size; nothing was restored")?;
        let kind = head[156];
        if !matches!(kind, b'0' | b'5') {
            return Err(format!("The backup contains an unsupported entry ({name})"));
        }
        let needed = size as usize;
        let padded = (needed + BLOCK - 1) / BLOCK * BLOCK;
        if offset + padded > rest.len() {
            return Err("The backup ends in the middle of a file; nothing was restored".into());
        }
        let destination = home.join(path);
        if destination.exists() {
            if !replace {
                return Err(format!(
                    "{} already exists; choose Replace to overwrite it",
                    destination.display()
                ));
            }
            let meta = fs::symlink_metadata(&destination).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err(format!("Refusing to replace a shortcut: {}", destination.display()));
            }
        }
        if kind == b'5' {
            fs::create_dir_all(&destination)
                .map_err(|e| format!("Cannot restore {}: {e}", destination.display()))?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&destination, &rest[offset..offset + needed])
                .map_err(|e| format!("Cannot restore {}: {e}", destination.display()))?;
            files += 1;
        }
        offset += padded;
    }
    Ok(format!(
        "Restored {files} file(s) from {}.",
        archive.file_name().unwrap_or_default().to_string_lossy()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "horizon-backup-{}-{}",
                std::process::id(),
                now_millis()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn archive_with(name: &str, bytes: &[u8], output: &Path) {
        let file = File::create(output).unwrap();
        let mut out = GzEncoder::new(file, Compression::default());
        out.write_all(&header(name, bytes.len() as u64, false).unwrap())
            .unwrap();
        write_padded(&mut out, bytes).unwrap();
        out.write_all(&[0u8; BLOCK * 2]).unwrap();
        out.finish().unwrap();
    }

    #[test]
    fn selected_groups_roundtrip_without_private_files() {
        let scratch = Scratch::new();
        let home = scratch.0.join("home");
        let target = scratch.0.join("out");
        fs::create_dir_all(home.join("Instances/Default/saves/World")).unwrap();
        fs::create_dir_all(home.join("Instances/Default/sessions")).unwrap();
        fs::create_dir_all(home.join("Controls/p0")).unwrap();
        fs::write(home.join("Instances/Default/saves/World/level.dat"), b"world").unwrap();
        fs::write(home.join("Instances/Default/sessions/token"), b"secret").unwrap();
        fs::write(home.join("Controls/p0/android-touch-layouts.json"), b"layouts").unwrap();
        fs::write(home.join("android-launcher.json"), b"{}").unwrap();
        fs::create_dir_all(&target).unwrap();

        let selected = json!({"instances": true, "controls": true, "launcher": true});
        let archive = PathBuf::from(create(&home, &selected, &target).unwrap());
        assert_eq!(archive.extension().and_then(|e| e.to_str()), Some("gz"));

        let restored = scratch.0.join("restored");
        fs::create_dir_all(&restored).unwrap();
        let note = restore(&archive, &restored, false).unwrap();
        assert!(note.contains("Restored 3 file(s)"), "{note}");
        assert_eq!(
            fs::read(restored.join("Instances/Default/saves/World/level.dat")).unwrap(),
            b"world"
        );
        assert_eq!(
            fs::read(restored.join("Controls/p0/android-touch-layouts.json")).unwrap(),
            b"layouts"
        );
        assert_eq!(fs::read(restored.join("android-launcher.json")).unwrap(), b"{}");
        assert!(!restored.join("Instances/Default/sessions").exists());
    }

    #[test]
    fn restore_refuses_overwrite_unless_replaced() {
        let scratch = Scratch::new();
        let archive = scratch.0.join("one.tar.gz");
        archive_with("Controls/p0/store.json", b"new", &archive);
        let home = scratch.0.join("home");
        fs::create_dir_all(home.join("Controls/p0")).unwrap();
        fs::write(home.join("Controls/p0/store.json"), b"old").unwrap();

        assert!(restore(&archive, &home, false).unwrap_err().contains("already exists"));
        assert_eq!(fs::read(home.join("Controls/p0/store.json")).unwrap(), b"old");
        restore(&archive, &home, true).unwrap();
        assert_eq!(fs::read(home.join("Controls/p0/store.json")).unwrap(), b"new");
    }

    #[test]
    fn unsafe_archive_paths_are_rejected() {
        let scratch = Scratch::new();
        let home = scratch.0.join("home");
        fs::create_dir_all(&home).unwrap();
        for name in ["../escape", "/absolute", "sessions/token", "Instances/../escape"] {
            let archive = scratch.0.join(format!("{}.tar.gz", name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>()));
            archive_with(name, b"bad", &archive);
            let error = restore(&archive, &home, true).unwrap_err();
            assert!(error.contains("unsafe path") || error.contains("damaged"), "{name}: {error}");
        }
        assert!(!home.join("escape").exists());
    }

    #[test]
    fn an_empty_selection_is_refused() {
        let scratch = Scratch::new();
        let home = scratch.0.join("home");
        let target = scratch.0.join("out");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&target).unwrap();
        assert!(create(&home, &json!({}), &target).unwrap_err().contains("at least one"));
    }
}
