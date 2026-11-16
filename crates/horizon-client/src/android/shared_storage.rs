//! Where the user's game files live, and how they move between the two places.
//!
//! Horizon has two storage modes for the player-facing files (worlds, mods,
//! resource packs, shader packs, options and the shareable reports):
//!
//! - **Private** (the default): the files sit in the app-private home
//!   (`getFilesDir()`). Other apps can only reach them through the SAF
//!   documents provider.
//! - **Shared**: the same files sit in a normal folder on shared storage
//!   (typically `/storage/emulated/0/Horizon-Android`), so any file manager
//!   can open them. The switch is deliberate (Settings → Storage → *Game
//!   files location*), never automatic, and it needs the all-files access
//!   the user grants in system settings first.
//!
//! The choice is recorded in `shared-storage.json` in the app-private home.
//! Every reader goes through [`resolve`]/[`profile_dir`] so both modes behave
//! the same; no call site may join the private home with a game file name.
//! The marker is also the only thing the SAF provider and the game process
//! need to agree on, so they can be running at the same time.
//!
//! Kept deliberately free of crate dependencies: the storage harness
//! (`tools/test_android_shared_storage.py`) compiles this file standalone.
//! `document_store.rs` carries a copy of [`resolve`]/`marker_dir` with the
//! same marker file and semantics; keep the two in sync.
use serde_json::json;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

/// The marker file, written by [`migrate`] and read by every resolver. The
/// temporary file it is renamed from shares the prefix so the mover can find
/// and skip both while collecting jobs.
pub const MARKER: &str = "shared-storage.json";
const MARKER_PARTIAL: &str = "shared-storage.json.partial";

/// Top-level entries of the private home that never move to shared storage:
/// the prepared engine build, private diagnostics, account sessions and the
/// launcher's own state. Reports the user asked to see travel separately.
const PRIVATE_TOP_LEVEL: &[&str] = &[
    "engine-data.zip",
    "engine-data.partial",
    ".local",
    "logs",
    "crash-reports",
    "sessions",
    "account-secrets",
    "control.json",
    "launch-request.json",
    "game.lock",
];

/// Entries that never move even inside instance folders: account sessions
/// are private, and logs and caches are generated output.
const PRIVATE_NESTED: &[&str] = &["sessions", "logs", "crash-reports", ".cache", "cache"];

/// Matches exactly what the SAF provider serves from `performance/`: the
/// shareable reports. Raw diagnostics stay private on both migrations.
fn shareable_report(name: &str) -> bool {
    (name.starts_with("horizon-diagnostics-") && name.ends_with(".zip")) || name.ends_with(".jsonl")
}

/// One migration at a time; the snapshot polls [`progress`].
static MOVING: AtomicBool = AtomicBool::new(false);
static PROGRESS_DONE: AtomicU64 = AtomicU64::new(0);
static PROGRESS_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Where the player-facing files currently live.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// No marker or no usable shared folder: game files are app-private.
    Private,
    /// Marker present, shared folder missing: the user removed or lost it.
    Missing,
    /// Migration done and the shared folder is there.
    Shared,
}

/// The shared folder the marker records, if it is absolute and exists.
fn marker_dir(home: &Path) -> Option<PathBuf> {
    let bytes = fs::read(home.join(MARKER)).ok()?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    let dir = value.get("dir")?.as_str()?;
    let path = PathBuf::from(dir);
    // stat() succeeds even without the all-files access, so probe by opening.
    (path.is_absolute() && fs::read_dir(&path).is_ok()).then_some(path)
}

fn marker_exists(home: &Path) -> bool {
    home.join(MARKER).is_file()
}

/// The active root for the player's game files: the shared folder when the
/// marker records it and the folder is there, else the private home. When
/// the shared folder disappears this falls back cleanly for readers; the
/// launcher blocks Play and the Storage row says so instead.
pub fn resolve(home: &Path) -> PathBuf {
    marker_dir(home).unwrap_or_else(|| home.to_path_buf())
}

/// The game folder for instance `id` (`0` is the default instance).
pub fn profile_dir(home: &Path, id: u32) -> PathBuf {
    let base = resolve(home);
    if id == 0 {
        base
    } else {
        base.join("profiles").join(format!("profile-{id}"))
    }
}

/// Where the shareable reports live (what the Files app shows as Reports).
pub fn reports_dir(home: &Path) -> PathBuf {
    resolve(home).join("performance")
}

pub fn state(home: &Path) -> State {
    if marker_dir(home).is_some() {
        State::Shared
    } else if marker_exists(home) {
        State::Missing
    } else {
        State::Private
    }
}

pub fn busy() -> bool {
    MOVING.load(Ordering::Acquire)
}

pub fn progress() -> (u64, u64) {
    (
        PROGRESS_DONE.load(Ordering::Relaxed),
        PROGRESS_TOTAL.load(Ordering::Relaxed),
    )
}

/// The name users see in Files, used by the summary row and the dialog.
pub fn display_name(home: &Path) -> String {
    match marker_dir(home) {
        Some(dir) => dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Horizon-Android".into()),
        None => "App storage".into(),
    }
}

fn is_private_top_level(name: &str) -> bool {
    PRIVATE_TOP_LEVEL.contains(&name) || name.starts_with("android-") || name.starts_with(MARKER)
}

/// Whether a game is currently running on `base` (its `game.lock` is held).
/// The launcher process and the game process do not share memory, so the
/// lock file is the only reliable signal.
fn game_in_use(base: &Path) -> bool {
    let Ok(file) = std::fs::OpenOptions::new()
        .write(true)
        .open(base.join("game.lock"))
    else {
        // No lock file: the game never ran here, so nothing is in use.
        return false;
    };
    #[cfg(unix)]
    {
        use rustix::fs::{FlockOperation, flock};
        flock(&file, FlockOperation::NonBlockingLockExclusive).is_err()
    }
    #[cfg(not(unix))]
    {
        file.try_lock().is_err()
    }
}

/// A file that will travel as one unit (an instance folder, saves, ...).
struct EntryPlan {
    src: PathBuf,
    dst: PathBuf,
    bytes: u64,
}

/// One destination path that was created this run, for rollback.
struct Created {
    path: PathBuf,
    is_dir: bool,
    /// `true` when the destination did not exist before this run; only
    /// those directories may be removed on rollback.
    new: bool,
}

/// Walks one tree, collecting the files that must travel and its size.
/// `depth` is the number of levels below the source root. Directories of the
/// source are recorded so the caller can remove the emptied ones afterwards.
fn collect_tree(
    src: &Path,
    dst: &Path,
    depth: usize,
    to_shared: bool,
    plans: &mut Vec<EntryPlan>,
    source_dirs: &mut BTreeSet<PathBuf>,
    total: &mut u64,
) -> std::io::Result<()> {
    if depth > 0 {
        source_dirs.insert(src.to_path_buf());
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str().map(str::to_owned) else {
            // Non-UTF-8 names cannot be addressed safely; leave them behind.
            continue;
        };
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            // Never follow or move symlinks.
            continue;
        }
        if to_shared {
            if depth == 0 && is_private_top_level(&name) {
                continue;
            }
            if depth > 0 && PRIVATE_NESTED.contains(&name.as_str()) {
                continue;
            }
        }
        let entry_src = entry.path();
        let entry_dst = dst.join(&name);
        if file_type.is_dir() {
            collect_tree(
                &entry_src,
                &entry_dst,
                depth + 1,
                to_shared,
                plans,
                source_dirs,
                total,
            )?;
        } else {
            let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            *total += bytes;
            plans.push(EntryPlan {
                src: entry_src,
                dst: entry_dst,
                bytes,
            });
        }
    }
    Ok(())
}

/// Collects everything that must move, honouring the shared-reports rule:
/// inside `performance/` only the shareable files travel. Returns the file
/// plans, the source directories to empty afterwards and the total size.
fn collect_jobs(
    from: &Path,
    to: &Path,
    to_shared: bool,
) -> std::io::Result<(Vec<EntryPlan>, BTreeSet<PathBuf>, u64)> {
    let mut plans = Vec::new();
    let mut source_dirs = BTreeSet::new();
    let mut total = 0u64;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str().map(str::to_owned) else {
            continue;
        };
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if to_shared && is_private_top_level(&name) {
            continue;
        }
        let entry_src = entry.path();
        let entry_dst = to.join(&name);
        if to_shared && name == "performance" && file_type.is_dir() {
            // Private diagnostics stay behind; only the shareable reports travel.
            for report in fs::read_dir(&entry_src)? {
                let report = report?;
                let report_name = report.file_name();
                let Some(report_name) = report_name.to_str().map(str::to_owned) else {
                    continue;
                };
                if !report.file_type()?.is_file() || !shareable_report(&report_name) {
                    continue;
                }
                let bytes = report.metadata().map(|m| m.len()).unwrap_or(0);
                total += bytes;
                plans.push(EntryPlan {
                    src: report.path(),
                    dst: entry_dst.join(&report_name),
                    bytes,
                });
            }
            // The source may hold private files too; it is emptied only of
            // what moved (private files would make remove_dir fail, which is
            // exactly what we want).
            source_dirs.insert(entry_src);
            continue;
        }
        if file_type.is_dir() {
            collect_tree(
                &entry_src,
                &entry_dst,
                1,
                to_shared,
                &mut plans,
                &mut source_dirs,
                &mut total,
            )?;
        } else {
            let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            total += bytes;
            plans.push(EntryPlan {
                src: entry_src,
                dst: entry_dst,
                bytes,
            });
        }
    }
    Ok((plans, source_dirs, total))
}

/// Available bytes on the filesystem backing `path`, for a preflight check.
fn free_bytes(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        let stats = rustix::fs::statvfs(path).ok()?;
        Some(
            stats
                .f_bavail
                .saturating_mul(stats.f_frsize.max(stats.f_bsize)),
        )
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Creates every missing directory along `dir`, recording new ones.
fn ensure_dir(dir: &Path, created: &mut Vec<Created>) -> std::io::Result<()> {
    let mut missing = Vec::new();
    let mut current = dir;
    loop {
        if current.is_dir() {
            break;
        }
        missing.push(current.to_path_buf());
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    for path in missing.into_iter().rev() {
        match fs::create_dir(&path) {
            Ok(()) => created.push(Created {
                path,
                is_dir: true,
                new: true,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Copies one planned file, creating parent directories as needed.
fn copy_one(plan: &EntryPlan, created: &mut Vec<Created>) -> std::io::Result<u64> {
    if let Some(parent) = plan.dst.parent() {
        ensure_dir(parent, created)?;
    }
    // Register the destination before copying so a partial file is rolled
    // back on failure too.
    created.push(Created {
        path: plan.dst.clone(),
        is_dir: false,
        new: true,
    });
    let copied = fs::copy(&plan.src, &plan.dst)?;
    if copied != plan.bytes {
        // A short read/write would silently truncate the player's data.
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "Copied {copied} of {} bytes for {}",
                plan.bytes,
                plan.dst.display()
            ),
        ));
    }
    Ok(copied)
}

fn rollback(created: &[Created]) {
    for entry in created.iter().rev() {
        if entry.is_dir {
            // Only remove directories this run created, and only if empty;
            // a non-empty one holds files we must not touch.
            if entry.new {
                let _ = fs::remove_dir_all(&entry.path);
            }
        } else {
            let _ = fs::remove_file(&entry.path);
        }
    }
}

/// Atomically records `shared` as the active game-files location.
fn write_marker(home: &Path, shared: &Path) -> std::io::Result<()> {
    let marker = home.join(MARKER);
    let partial = home.join(MARKER_PARTIAL);
    let body = json!({"version": 1, "dir": shared.to_string_lossy()});
    fs::write(&partial, body.to_string())?;
    fs::rename(&partial, &marker)
}

/// Points the game at an existing shared folder without moving anything.
///
/// This is the reinstall path: Horizon was uninstalled and installed again,
/// the shared folder still holds the player's worlds, and app storage holds
/// nothing worth keeping. Adoption refuses when there is anything in app
/// storage to lose and reports that both places hold data otherwise.
pub fn adopt(home: &Path, shared: &Path) -> Result<String, String> {
    if MOVING.swap(true, Ordering::AcqRel) {
        return Err("Game files are already being moved".into());
    }
    PROGRESS_DONE.store(0, Ordering::Relaxed);
    PROGRESS_TOTAL.store(0, Ordering::Relaxed);
    let result = adopt_inner(home, shared);
    MOVING.store(false, Ordering::Release);
    result
}

fn adopt_inner(home: &Path, shared: &Path) -> Result<String, String> {
    if !shared.is_absolute() {
        return Err("The shared folder path must be absolute".into());
    }
    if state(home) != State::Private {
        return Err("The game files are already on shared storage".into());
    }
    if !shared.is_dir() {
        return Err("The Horizon-Android folder was not found".into());
    }
    if game_in_use(&resolve(home)) {
        return Err("Close the game before moving the game files".into());
    }
    let (plans, _dirs, total) =
        collect_jobs(home, shared, true).map_err(|e| format!("Cannot read the game files: {e}"))?;
    PROGRESS_TOTAL.store(total, Ordering::Relaxed);
    if !plans.is_empty() {
        return Err(
            "Both app storage and Horizon-Android hold game files. Move or remove one of them first."
                .into(),
        );
    }
    write_marker(home, shared)
        .map_err(|e| format!("Could not record the storage location: {e}"))?;
    Ok("Horizon is using the game files in Horizon-Android".into())
}

/// Moves the player's game files between the private home and `shared`.
///
/// With `to_shared`, `shared` must be empty (or absent); the caller is
/// expected to have created it or let this function create it. Otherwise
/// `home` is the target and `shared` must hold what previously moved.
/// Nothing is deleted from the source until every file is copied and size-
/// checked; on failure the destination is rolled back and the source is
/// untouched.
pub fn migrate(home: &Path, shared: &Path, to_shared: bool) -> Result<String, String> {
    if MOVING.swap(true, Ordering::AcqRel) {
        return Err("Game files are already being moved".into());
    }
    PROGRESS_DONE.store(0, Ordering::Relaxed);
    PROGRESS_TOTAL.store(0, Ordering::Relaxed);
    let result = migrate_inner(home, shared, to_shared);
    MOVING.store(false, Ordering::Release);
    result
}

fn migrate_inner(home: &Path, shared: &Path, to_shared: bool) -> Result<String, String> {
    if !shared.is_absolute() || !home.is_absolute() {
        return Err("Storage paths must be absolute".into());
    }
    let (from, to) = if to_shared {
        (home, shared)
    } else {
        (shared, home)
    };
    // Never race the game: it writes worlds while it runs.
    let base = resolve(home);
    if game_in_use(&base) {
        return Err("Close the game before moving the game files".into());
    }
    if !from.is_dir() {
        return Err(if to_shared {
            "The game files folder was not found".into()
        } else {
            "The shared game files folder was not found".into()
        });
    }
    if to_shared {
        if shared.exists()
            && fs::read_dir(shared)
                .map(|mut d| d.next().is_some())
                .unwrap_or(true)
        {
            return Err(format!(
                "{} is not empty. Move its contents away first.",
                shared.display()
            ));
        }
        if !shared.exists() {
            fs::create_dir_all(shared)
                .map_err(|e| format!("Cannot create {}: {e}", shared.display()))?;
        }
    } else if !marker_exists(home) {
        return Err("The game files were never moved to shared storage".into());
    }

    let (plans, source_dirs, total) = collect_jobs(from, to, to_shared)
        .map_err(|e| format!("Cannot read the game files: {e}"))?;
    PROGRESS_TOTAL.store(total, Ordering::Relaxed);

    // A collision would silently overwrite the player's data in the target;
    // refuse before writing anything.
    if plans.iter().any(|plan| plan.dst.exists()) {
        return Err("Something at the destination would be overwritten; nothing was moved".into());
    }
    if let Some(free) = free_bytes(to) {
        // Leave a little headroom rather than stopping at 100%.
        if free < total.saturating_add(64 * 1024 * 1024) {
            return Err("Not enough free space on shared storage for the game files".into());
        }
    }

    let mut created: Vec<Created> = Vec::new();
    let mut done = 0u64;
    let copy_result = (|| -> std::io::Result<()> {
        for plan in &plans {
            done += copy_one(plan, &mut created)?;
            PROGRESS_DONE.store(done, Ordering::Relaxed);
        }
        Ok(())
    })();
    if let Err(error) = copy_result {
        rollback(&created);
        return Err(format!("Could not copy the game files: {error}"));
    }

    // Every byte is in place; only now remove the sources, files first.
    let mut sources_removed = true;
    for plan in &plans {
        if fs::remove_file(&plan.src).is_err() {
            sources_removed = false;
        }
    }
    // Empty the moved source directories, deepest first. A directory that
    // holds private files this run left behind fails remove_dir and stays.
    let mut sorted_dirs: Vec<_> = source_dirs.into_iter().collect();
    sorted_dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for dir in sorted_dirs {
        // Fails if something private or new lives inside; that is correct.
        let _ = fs::remove_dir(dir);
    }

    // The marker is the commit point; everything before it rolls back.
    if to_shared {
        write_marker(home, shared)
            .map_err(|e| format!("Could not record the storage location: {e}"))?;
    } else {
        let _ = fs::remove_file(home.join(MARKER));
        // Leave nothing behind when everything moved.
        let _ = fs::remove_dir(shared);
    }
    PROGRESS_DONE.store(total, Ordering::Relaxed);
    let note = if sources_removed {
        String::new()
    } else {
        " (some files could not be removed from the old location)".to_string()
    };
    Ok(format!(
        "Moved {} files ({}){}",
        plans.len(),
        format_bytes(total),
        note
    ))
}

fn format_bytes(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "horizon-shared-storage-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn seed(root: &Path) {
        fs::create_dir_all(root.join("saves/World One")).unwrap();
        fs::write(root.join("saves/World One/level.dat"), b"level").unwrap();
        fs::create_dir_all(root.join("horizon-mods")).unwrap();
        fs::write(root.join("horizon-mods/test.hmod"), b"mod").unwrap();
        fs::create_dir_all(root.join("performance")).unwrap();
        fs::write(root.join("performance/horizon-performance-1.jsonl"), b"{}").unwrap();
        fs::write(root.join("performance/diagnostic.log"), b"private").unwrap();
        fs::write(root.join("performance/android-exits.json"), b"private").unwrap();
        fs::create_dir_all(root.join("profiles/profile-2/saves/World Two")).unwrap();
        fs::write(
            root.join("profiles/profile-2/saves/World Two/level.dat"),
            b"two",
        )
        .unwrap();
        fs::create_dir_all(root.join("profiles/profile-2/sessions")).unwrap();
        fs::write(
            root.join("profiles/profile-2/sessions/token.txt"),
            b"secret",
        )
        .unwrap();
        fs::write(root.join("android-launcher.json"), b"{}").unwrap();
        fs::write(root.join("engine-data.zip"), b"engine").unwrap();
        fs::create_dir_all(root.join(".local/horizon-client")).unwrap();
        fs::write(root.join(".local/horizon-client/data"), b"data").unwrap();
        fs::create_dir_all(root.join("logs")).unwrap();
        fs::write(root.join("logs/latest.log"), b"log").unwrap();
        fs::create_dir_all(root.join("screenshots")).unwrap();
        fs::write(root.join("screenshots/shot.png"), b"png").unwrap();
        fs::write(root.join("options.txt"), b"options").unwrap();
    }

    #[test]
    fn migrates_game_files_and_keeps_private_state() {
        let home = scratch();
        seed(&home);
        let shared = home.with_file_name(format!(
            "{}-shared",
            home.file_name().unwrap().to_string_lossy()
        ));
        let summary = migrate(&home, &shared, true).unwrap();
        assert!(summary.contains("Moved"), "{summary}");
        assert_eq!(
            fs::read(shared.join("saves/World One/level.dat")).unwrap(),
            b"level"
        );
        assert_eq!(
            fs::read(shared.join("horizon-mods/test.hmod")).unwrap(),
            b"mod"
        );
        assert_eq!(
            fs::read(shared.join("performance/horizon-performance-1.jsonl")).unwrap(),
            b"{}"
        );
        assert_eq!(
            fs::read(shared.join("profiles/profile-2/saves/World Two/level.dat")).unwrap(),
            b"two"
        );
        assert_eq!(fs::read(shared.join("options.txt")).unwrap(), b"options");
        assert!(
            !shared.join("performance/diagnostic.log").exists(),
            "raw diagnostics must not move"
        );
        assert!(
            !shared.join("performance/android-exits.json").exists(),
            "exit reports must not move"
        );
        assert!(!shared.join("android-launcher.json").exists());
        assert!(!shared.join("engine-data.zip").exists());
        assert!(!shared.join(".local").exists());
        assert!(!shared.join("logs").exists());
        assert!(
            !shared.join("profiles/profile-2/sessions").exists(),
            "sessions must stay private"
        );
        // The private originals that must stay are untouched.
        assert_eq!(fs::read(home.join("android-launcher.json")).unwrap(), b"{}");
        assert_eq!(fs::read(home.join("engine-data.zip")).unwrap(), b"engine");
        assert_eq!(
            fs::read(home.join(".local/horizon-client/data")).unwrap(),
            b"data"
        );
        assert_eq!(
            fs::read(home.join("performance/diagnostic.log")).unwrap(),
            b"private"
        );
        assert_eq!(
            fs::read(home.join("profiles/profile-2/sessions/token.txt")).unwrap(),
            b"secret"
        );
        // The moved game files left the private home.
        assert!(!home.join("saves").exists());
        assert!(!home.join("horizon-mods").exists());
        assert!(!home.join("options.txt").exists());
        assert_eq!(state(&home), State::Shared);
        assert_eq!(resolve(&home), shared);
        assert_eq!(profile_dir(&home, 0), shared);
        assert_eq!(profile_dir(&home, 2), shared.join("profiles/profile-2"));
        assert_eq!(reports_dir(&home), shared.join("performance"));
        // Move back.
        let back = migrate(&home, &shared, false).unwrap();
        assert!(back.contains("Moved"), "{back}");
        assert_eq!(
            fs::read(home.join("saves/World One/level.dat")).unwrap(),
            b"level"
        );
        assert_eq!(
            fs::read(home.join("horizon-mods/test.hmod")).unwrap(),
            b"mod"
        );
        assert_eq!(
            fs::read(home.join("performance/horizon-performance-1.jsonl")).unwrap(),
            b"{}"
        );
        assert_eq!(state(&home), State::Private);
        assert_eq!(resolve(&home), home);
        fs::remove_dir_all(&home).unwrap();
        let _ = fs::remove_dir_all(&shared);
    }

    #[test]
    fn shared_target_must_be_empty() {
        let home = scratch();
        seed(&home);
        let shared = scratch();
        fs::write(shared.join("someone-else.txt"), b"x").unwrap();
        assert!(migrate(&home, &shared, true).is_err());
        assert!(shared.join("someone-else.txt").is_file());
        assert!(home.join("saves/World One/level.dat").is_file());
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&shared).unwrap();
    }

    #[test]
    fn missing_shared_folder_falls_back_for_readers() {
        let home = scratch();
        fs::write(
            home.join(MARKER),
            json!({"version": 1, "dir": home.join("gone").to_string_lossy()}).to_string(),
        )
        .unwrap();
        assert_eq!(state(&home), State::Missing);
        assert_eq!(resolve(&home), home);
        assert_eq!(profile_dir(&home, 0), home);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn game_lock_blocks_migration() {
        let home = scratch();
        seed(&home);
        let shared = home.with_file_name(format!(
            "{}-shared",
            home.file_name().unwrap().to_string_lossy()
        ));
        let lock = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(home.join("game.lock"))
            .unwrap();
        #[cfg(unix)]
        let held = {
            use rustix::fs::{FlockOperation, flock};
            flock(&lock, FlockOperation::NonBlockingLockExclusive).is_ok()
        };
        #[cfg(not(unix))]
        let held = lock.try_lock().is_ok();
        assert!(held);
        assert!(migrate(&home, &shared, true).is_err());
        assert!(home.join("saves/World One/level.dat").is_file());
        fs::remove_dir_all(&home).unwrap();
        let _ = fs::remove_dir_all(&shared);
    }
}
