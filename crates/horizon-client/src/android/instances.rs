//! Instance (profile) folders shared by the Android launcher bridge.
//!
//! An instance is the launcher's existing profile folder: the app home for
//! profile 0 and `home/profiles/profile-N` for the others. This module only
//! names, measures and copies those folders; the launcher keeps owning the
//! active choice, the configuration and the game launch.

use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The longest instance name the launcher stores.
pub const MAX_NAME: usize = 32;
/// The player-chosen name, stored beside the instance's saves.
pub const NAME_FILE: &str = "android-profile-name.txt";
/// When the instance last started the game, as Unix seconds.
const PLAYED_FILE: &str = "android-profile-played.txt";
/// Generated output that a duplicate does not need: logs, crash reports and
/// caches. `sessions` holds account session files and is never copied.
const TRANSIENT: &[&str] = &["logs", "crash-reports", ".cache", "cache", "sessions"];

/// Trims a typed name and drops control characters, capped like the dialog.
pub fn clean_name(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect()
}

/// Whether another instance already uses this name (case-insensitive).
pub fn name_in_use(existing: &[(u32, String)], skip: Option<u32>, name: &str) -> bool {
    let wanted = name.trim().to_lowercase();
    existing
        .iter()
        .any(|(id, current)| Some(*id) != skip && current.trim().to_lowercase() == wanted)
}

/// The next free `profile-N` number.
pub fn next_id(ids: &[u32]) -> Option<u32> {
    ids.iter().copied().max().unwrap_or(0).checked_add(1)
}

/// A name no instance uses, derived from `base` (`Survival copy`, `… copy 2`).
pub fn unique_name(existing: &[(u32, String)], base: &str) -> String {
    let base = clean_name(base);
    let base = if base.is_empty() {
        "Instance".to_owned()
    } else {
        base
    };
    if !name_in_use(existing, None, &base) {
        return base;
    }
    for n in 2..=999 {
        let suffix = format!(" {n}");
        let keep = MAX_NAME.saturating_sub(suffix.chars().count());
        let stem: String = base.chars().take(keep).collect();
        let candidate = format!("{}{suffix}", stem.trim_end());
        if !name_in_use(existing, None, &candidate) {
            return candidate;
        }
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    format!("Instance {now}")
}

/// Checks a create/rename name against the names already in use.
pub fn validated_name(
    existing: &[(u32, String)],
    skip: Option<u32>,
    raw: &str,
) -> Result<String, String> {
    let name = clean_name(raw);
    if name.is_empty() {
        return Err("Give the instance a name".into());
    }
    if name_in_use(existing, skip, &name) {
        return Err(format!(
            "Another instance is already called \"{name}\". Choose a different name."
        ));
    }
    Ok(name)
}

/// The stored name, or a stable fallback for folders that predate the name file.
pub fn display_name(dir: &Path, id: u32) -> String {
    fs::read_to_string(dir.join(NAME_FILE))
        .ok()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            if id == 0 {
                "Default".into()
            } else {
                format!("Instance {id}")
            }
        })
}

/// Stores a name; an empty name restores the default.
pub fn write_name(dir: &Path, name: &str) -> Result<(), String> {
    let name = clean_name(name);
    if name.is_empty() {
        let _ = fs::remove_file(dir.join(NAME_FILE));
        return Ok(());
    }
    fs::write(dir.join(NAME_FILE), name).map_err(|e| e.to_string())
}

/// The instance's storage usage. Unreadable entries are skipped, not counted.
pub fn dir_size(root: &Path) -> u64 {
    fn walk(dir: &Path, total: &mut u64) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                walk(&entry.path(), total);
            } else if let Ok(metadata) = entry.metadata() {
                *total += metadata.len();
            }
        }
    }
    let mut total = 0;
    walk(root, &mut total);
    total
}

/// When the instance last played, from the launch marker or, for instances that
/// predate it, the newest world folder.
pub fn last_played(root: &Path) -> Option<u64> {
    if let Ok(text) = fs::read_to_string(root.join(PLAYED_FILE)) {
        if let Ok(seconds) = text.trim().parse::<u64>() {
            return Some(seconds);
        }
    }
    let saves = fs::read_dir(root.join("saves")).ok()?;
    saves
        .flatten()
        .filter_map(|entry| entry.metadata().ok()?.modified().ok())
        .filter_map(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|age| age.as_secs())
        .max()
}

/// Records a game launch for this instance. Failures are the caller's to log.
pub fn mark_played(root: &Path) -> io::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|age| age.as_secs())
        .unwrap_or_default();
    fs::write(root.join(PLAYED_FILE), now.to_string())
}

/// Copies an instance folder for duplication. Logs, caches and session files are
/// skipped, links are never followed, and an existing destination is refused so
/// a duplicate can never overwrite another instance.
pub fn copy_profile(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err("That instance's folder is missing; it cannot be duplicated".into());
    }
    if destination.exists() {
        return Err("A folder for the duplicated instance already exists".into());
    }
    fs::create_dir_all(destination).map_err(|e| format!("Cannot create the new instance: {e}"))?;
    match copy_dir(source, destination) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_dir_all(destination);
            Err(error)
        }
    }
}

fn copy_dir(source: &Path, destination: &Path) -> Result<(), String> {
    let entries =
        fs::read_dir(source).map_err(|e| format!("Cannot read {}: {e}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let text = name.to_string_lossy();
        if text.starts_with('.') || TRANSIENT.contains(&text.as_ref()) {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            continue;
        }
        let from = entry.path();
        let to = destination.join(&name);
        if kind.is_dir() {
            fs::create_dir(&to).map_err(|e| format!("Cannot create {}: {e}", to.display()))?;
            copy_dir(&from, &to)?;
        } else if kind.is_file() {
            fs::copy(&from, &to).map_err(|e| format!("Cannot copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A scratch folder that removes itself.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "horizon-instances-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn names_are_trimmed_capped_and_free_of_control_characters() {
        assert_eq!(clean_name("  Survival  "), "Survival");
        assert_eq!(clean_name("a\u{7}b\nc"), "abc");
        assert_eq!(clean_name(&"x".repeat(40)).chars().count(), MAX_NAME);
        assert_eq!(clean_name("   "), "");
    }

    #[test]
    fn duplicate_names_are_rejected_case_insensitively_but_rename_skips_itself() {
        let existing = vec![(0, "Default".to_owned()), (2, "Survival".to_owned())];
        assert!(name_in_use(&existing, None, "survival"));
        assert!(!name_in_use(&existing, Some(2), "Survival"));
        assert_eq!(
            validated_name(&existing, None, " survival ").unwrap_err(),
            "Another instance is already called \"survival\". Choose a different name."
        );
        assert_eq!(
            validated_name(&existing, None, "Creative").unwrap(),
            "Creative"
        );
        assert!(validated_name(&existing, None, "  ").is_err());
    }

    #[test]
    fn new_ids_follow_the_highest_existing_one() {
        assert_eq!(next_id(&[0, 2, 5]), Some(6));
        assert_eq!(next_id(&[]), Some(1));
        assert_eq!(next_id(&[0, u32::MAX]), None);
    }

    #[test]
    fn duplicated_names_stay_unique_and_within_the_limit() {
        let existing = vec![(0, "Default".to_owned()), (2, "Survival".to_owned())];
        assert_eq!(unique_name(&existing, "Creative"), "Creative");
        assert_eq!(unique_name(&existing, "Survival copy"), "Survival copy");
        let taken = vec![(0, "Default".to_owned()), (2, "Survival copy".to_owned())];
        assert_eq!(unique_name(&taken, "Survival copy"), "Survival copy 2");
        let long = unique_name(&taken, &"y".repeat(MAX_NAME));
        assert_eq!(long.chars().count(), MAX_NAME);
    }

    #[test]
    fn a_duplicate_copies_worlds_and_settings_but_not_logs_caches_or_sessions() {
        let scratch = Scratch::new();
        let source = scratch.path().join("source");
        let destination = scratch.path().join("copy");
        fs::create_dir_all(source.join("saves/World/region")).unwrap();
        fs::write(source.join("saves/World/level.dat"), b"level").unwrap();
        fs::write(source.join("saves/World/region/r.0.0.mca"), b"region").unwrap();
        fs::write(source.join("options.txt"), b"renderDistance:8\n").unwrap();
        fs::write(source.join(NAME_FILE), "Survival").unwrap();
        fs::create_dir_all(source.join("logs")).unwrap();
        fs::write(source.join("logs/latest.log"), b"noise").unwrap();
        fs::create_dir_all(source.join("sessions")).unwrap();
        fs::write(source.join("sessions/session.json"), b"secret").unwrap();
        fs::create_dir_all(source.join(".cache")).unwrap();
        fs::write(source.join(".cache/thumb.png"), b"cache").unwrap();

        copy_profile(&source, &destination).unwrap();

        assert_eq!(
            fs::read(destination.join("saves/World/level.dat")).unwrap(),
            b"level"
        );
        assert_eq!(
            fs::read(destination.join("saves/World/region/r.0.0.mca")).unwrap(),
            b"region"
        );
        assert_eq!(
            fs::read(destination.join("options.txt")).unwrap(),
            b"renderDistance:8\n"
        );
        assert_eq!(display_name(&destination, 3), "Survival");
        assert!(!destination.join("logs").exists());
        assert!(!destination.join("sessions").exists());
        assert!(!destination.join(".cache").exists());
    }

    #[test]
    fn duplicating_refuses_missing_sources_and_existing_destinations() {
        let scratch = Scratch::new();
        let source = scratch.path().join("source");
        fs::create_dir_all(&source).unwrap();
        let existing = scratch.path().join("existing");
        fs::create_dir_all(&existing).unwrap();
        assert!(copy_profile(&scratch.path().join("missing"), &scratch.path().join("x")).is_err());
        assert!(copy_profile(&source, &existing).is_err());
        assert!(source.is_dir(), "the source instance is left untouched");
    }

    #[test]
    fn storage_usage_adds_files_and_skips_links() {
        let scratch = Scratch::new();
        let instance = scratch.path().join("instance");
        fs::create_dir_all(instance.join("saves/World")).unwrap();
        fs::write(instance.join("saves/World/level.dat"), b"12345").unwrap();
        fs::write(instance.join("options.txt"), b"abc").unwrap();
        assert_eq!(dir_size(&instance), 8);
        assert_eq!(dir_size(&scratch.path().join("missing")), 0);
    }

    #[test]
    fn play_time_is_recorded_and_read_back() {
        let scratch = Scratch::new();
        let instance = scratch.path().join("instance");
        fs::create_dir_all(&instance).unwrap();
        assert_eq!(last_played(&instance), None);
        mark_played(&instance).unwrap();
        let played = last_played(&instance).expect("the launch marker is read back");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(played <= now && now - played < 60);
    }

    #[test]
    fn names_fall_back_for_missing_or_blank_files() {
        let scratch = Scratch::new();
        let instance = scratch.path().join("instance");
        fs::create_dir_all(&instance).unwrap();
        assert_eq!(display_name(&instance, 4), "Instance 4");
        assert_eq!(display_name(scratch.path(), 0), "Default");
        fs::write(instance.join(NAME_FILE), "  ").unwrap();
        assert_eq!(display_name(&instance, 4), "Instance 4");
        write_name(&instance, "  Creative  ").unwrap();
        assert_eq!(display_name(&instance, 4), "Creative");
        write_name(&instance, "").unwrap();
        assert_eq!(display_name(&instance, 4), "Instance 4");
    }
}
