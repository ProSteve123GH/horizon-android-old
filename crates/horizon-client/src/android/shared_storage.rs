//! Where the user's game files live, and how they move between the two places.
//!
//! Player-facing files (worlds, mods, resource packs, shader packs, options
//! and the shareable reports) live on shared storage in a normal folder
//! (`/storage/emulated/0/Horizon-Android`), so any file manager can open them.
//! All-files access is mandatory; the launcher's permission gate asks for it
//! and blocks Play until it is granted.
//!
//! Legacy installs that were using the app-private home (`getFilesDir()`) are
//! migrated once with [`migrate`], which records the destination in
//! `shared-storage.json` in the private home. The migration is **one way**:
//! there is no "move back" mode. [`resolve`] returns the recorded shared
//! folder unconditionally once the marker exists; a missing or unreadable
//! folder is a permission/storage problem for the launcher to surface, never
//! a reason to silently write game files into a second location.
//!
//! Every reader goes through [`resolve`]/[`profile_dir`] so there is one root;
//! no call site may join the private home with a game file name. The marker is
//! also the only thing the SAF provider and the game process need to agree on,
//! so they can be running at the same time.
//!
//! Migrations journal every file they create in `.storage-migration.json`
//! (private) so a killed or crashed move can roll itself back (copy phase) or
//! finish itself (delete phase) on the next start; see [`recover`].
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

/// One-shot marker for the first-run prompt. Starts with `android-` so the
/// migration keeps it private on both moves.
pub const PROMPT_MARKER: &str = "android-storage-prompt.json";

/// Private journal of an in-flight migration: every destination path this
/// run creates, the matching sources, the phase (`copying` | `deleting`) and
/// both roots. Used by [`recover`] after a crash. Starts with a dot, so the
/// migration keeps it private too.
const JOURNAL: &str = ".storage-migration.json";

/// Top-level entries of the private home that never move to shared storage:
/// the prepared engine build, private diagnostics, account sessions and the
/// launcher's own state. Reports the user asked to see travel separately.
///
/// `native/` holds the Java loader's `current.json` and the compiled modded
/// libraries under `builds/` (`filesDir/native/...` in
/// `NativeMods.prepareAndLoad`). It must stay app-private: moving it leaves the
/// loader with no manifest and silently boots the stock game. Native-mod
/// *bundles* live in the separate `horizon-native-mods/` folder and do travel.
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
    "native",
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

/// The shared folder recorded in the marker, without requiring it to exist.
/// `None` when there is no usable `shared-storage.json` (never moved, or
/// corrupt). Used to re-create a missing folder and to move back to app
/// storage; every other reader must use [`marker_dir`]'s existence check.
pub fn recorded_dir(home: &Path) -> Option<PathBuf> {
    let bytes = fs::read(home.join(MARKER)).ok()?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    let dir = value.get("dir")?.as_str()?;
    let path = PathBuf::from(dir);
    path.is_absolute().then_some(path)
}

/// Whether the one-shot first-run prompt was already dismissed or answered.
pub fn prompt_done(home: &Path) -> bool {
    home.join(PROMPT_MARKER).is_file()
}

/// Records that the first-run prompt was dismissed or answered; the caller
/// never shows it again afterwards.
pub fn mark_prompt_done(home: &Path) -> std::io::Result<()> {
    let partial = home.join(format!("{PROMPT_MARKER}.partial"));
    fs::write(&partial, b"{\"version\":1}\n")?;
    fs::rename(&partial, home.join(PROMPT_MARKER))
}

/// The active root for the player's game files: once the all-files permission
/// has been granted and the migration wrote `shared-storage.json`, the shared
/// folder recorded there is used unconditionally — even if a transient mount
/// or a revoked permission makes a probe fail. There is no silent fallback to
/// app storage: the permission gate in the launcher blocks Play and the
/// Storage row explains how to restore access. Before any migration the marker
/// is absent and the private home remains the root (legacy installs).
pub fn resolve(home: &Path) -> PathBuf {
    recorded_dir(home).unwrap_or_else(|| home.to_path_buf())
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
    PRIVATE_TOP_LEVEL.contains(&name)
        || name.starts_with("android-")
        || name.starts_with(MARKER)
        || name.starts_with('.')
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
        // Only a genuine contention report means a game is running. On
        // FUSE-backed /sdcard flock reports other errors, which must not be
        // mistaken for a live game and block file moves.
        matches!(
            flock(&file, FlockOperation::NonBlockingLockExclusive),
            Err(rustix::io::Errno::WOULDBLOCK)
        )
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
            if depth > 0 && (PRIVATE_NESTED.contains(&name.as_str()) || name.starts_with('.')) {
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

/// Byte-for-byte comparison of two files. Used both to verify a copy and, on
/// recovery, to confirm that a journal source still holds the migrated
/// original — no hashing dependency, and the verified destination is a
/// stronger reference than a recorded hash would be.
fn files_equal(a: &Path, b: &Path) -> std::io::Result<bool> {
    use std::io::Read;

    let mut left = fs::File::open(a)?;
    let mut right = fs::File::open(b)?;
    let mut left_buf = [0u8; 64 * 1024];
    let mut right_buf = [0u8; 64 * 1024];
    loop {
        let read_left = left.read(&mut left_buf)?;
        let read_right = right.read(&mut right_buf)?;
        if read_left != read_right || left_buf[..read_left] != right_buf[..read_right] {
            return Ok(false);
        }
        if read_left == 0 {
            return Ok(true);
        }
    }
}

/// Re-reads a copied file end to end and compares it with the source, byte
/// for byte. A truncated or torn copy would otherwise become the only copy
/// of a world; the size check alone cannot catch a same-length corruption.
fn verify_file(plan: &EntryPlan) -> std::io::Result<()> {
    let source = fs::metadata(&plan.src)?;
    let copied = fs::metadata(&plan.dst)?;
    if source.len() != plan.bytes || copied.len() != plan.bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "Size mismatch for {}: source {} bytes, copy {} bytes",
                plan.dst.display(),
                source.len(),
                copied.len()
            ),
        ));
    }
    if !files_equal(&plan.src, &plan.dst)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Contents differ between {} and {}", plan.src.display(), plan.dst.display()),
        ));
    }
    Ok(())
}

/// Every directory the destination side of `plans` needs to exist, from the
/// target root down to the files. Recovery removes these only when empty.
fn destination_dirs(target: &Path, plans: &[EntryPlan]) -> Vec<PathBuf> {
    let mut dirs = BTreeSet::new();
    for plan in plans {
        let mut current = plan.dst.parent();
        while let Some(dir) = current {
            if !dir.starts_with(target) {
                break;
            }
            let insert = dirs.insert(dir.to_path_buf());
            if !insert || dir == target {
                break;
            }
            current = dir.parent();
        }
    }
    dirs.into_iter().collect()
}

fn rollback(created: &[Created]) {
    for entry in created.iter().rev() {
        if entry.is_dir {
            // Only remove directories this run created, and only if empty;
            // a non-empty one holds files we must not touch.
            if entry.new {
                let _ = fs::remove_dir(&entry.path);
            }
        } else {
            let _ = fs::remove_file(&entry.path);
        }
    }
}

/// One file of an in-flight migration. `bytes` is the source length at the
/// moment the journal was written; recovery in the `deleting` phase uses it
/// (plus a byte-for-byte comparison with the verified `dst`) to make sure a
/// source path still holds the original before removing it. Without that check
/// a stale journal could delete an unrelated file placed at the same path
/// after the crash. `None` means an older journal that did not record a length.
struct JournalFile {
    src: PathBuf,
    dst: PathBuf,
    bytes: Option<u64>,
}

/// A snapshot of an in-flight migration, written to the private home before
/// anything is copied. The journal is the only state a killed or crashed
/// migration leaves behind; [`recover`] finishes or undoes exactly what it
/// records and never anything else.
struct MigrationJournal {
    /// `copying` until every byte is in place and verified; `deleting` while
    /// the sources are being removed.
    phase: &'static str,
    to_shared: bool,
    from: PathBuf,
    target: PathBuf,
    /// Every file that travels, in copy order.
    files: Vec<JournalFile>,
    /// Every directory the destination side needs, including `target`
    /// itself. Recovery removes these deepest-first and only when empty.
    created_dirs: Vec<PathBuf>,
    /// Source directories to empty after the delete, deepest-first handled
    /// by the caller.
    source_dirs: Vec<PathBuf>,
}

impl MigrationJournal {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "version": 1,
            "phase": self.phase,
            "to_shared": self.to_shared,
            "from": self.from.to_string_lossy(),
            "target": self.target.to_string_lossy(),
            "files": self.files.iter().map(|file| json!({
                "src": file.src.to_string_lossy(),
                "dst": file.dst.to_string_lossy(),
                "bytes": file.bytes,
            })).collect::<Vec<_>>(),
            "created_dirs": self.created_dirs.iter()
                .map(|dir| dir.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            "source_dirs": self.source_dirs.iter()
                .map(|dir| dir.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
        })
    }

    fn from_json(value: &serde_json::Value) -> Option<Self> {
        let phase = match value.get("phase")?.as_str()? {
            "copying" => "copying",
            "deleting" => "deleting",
            _ => return None,
        };
        let files = value
            .get("files")?
            .as_array()?
            .iter()
            .map(|entry| {
                Some(JournalFile {
                    src: PathBuf::from(entry.get("src")?.as_str()?),
                    dst: PathBuf::from(entry.get("dst")?.as_str()?),
                    // Older journals did not record the source length; those
                    // rely on the byte-for-byte match with `dst` alone.
                    bytes: entry.get("bytes").and_then(|v| v.as_u64()),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let dirs = |key: &str| -> Option<Vec<PathBuf>> {
            Some(
                value
                    .get(key)?
                    .as_array()?
                    .iter()
                    .map(|entry| entry.as_str().map(PathBuf::from))
                    .collect::<Option<Vec<_>>>()?,
            )
        };
        Some(Self {
            phase,
            to_shared: value.get("to_shared")?.as_bool()?,
            from: PathBuf::from(value.get("from")?.as_str()?),
            target: PathBuf::from(value.get("target")?.as_str()?),
            files,
            created_dirs: dirs("created_dirs")?,
            source_dirs: dirs("source_dirs")?,
        })
    }
}

fn journal_path(home: &Path) -> PathBuf {
    home.join(JOURNAL)
}

/// Atomically records `journal` in the private home.
fn write_journal(home: &Path, journal: &MigrationJournal) -> std::io::Result<()> {
    let partial = home.join(format!("{JOURNAL}.partial"));
    fs::write(&partial, journal.to_json().to_string())?;
    fs::rename(&partial, journal_path(home))
}

fn read_journal(home: &Path) -> Option<MigrationJournal> {
    let bytes = fs::read(journal_path(home)).ok()?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    MigrationJournal::from_json(&value)
}

/// Refuses a journal that does not describe a self-contained move: every
/// recorded path must be absolute and under the roots the migration
/// controlled, so a corrupt or stale file can never delete unrelated data.
fn journal_paths_are_safe(journal: &MigrationJournal) -> bool {
    if !journal.from.is_absolute() || !journal.target.is_absolute() {
        return false;
    }
    journal.files.iter().all(|file| {
        file.src.is_absolute()
            && file.dst.is_absolute()
            && file.src.starts_with(&journal.from)
            && file.dst.starts_with(&journal.target)
    }) && journal.created_dirs.iter().all(|dir| dir.starts_with(&journal.target))
        && journal
            .source_dirs
            .iter()
            .all(|dir| dir.starts_with(&journal.from))
}

/// Deletes every destination file the journal lists, deepest first, and the
/// directories this run created (only while they are empty). Sources are
/// left untouched. Used after a crash in the `copying` phase.
fn recover_copying(home: &Path, journal: &MigrationJournal) {
    for file in journal.files.iter().rev() {
        // The destination was created by the interrupted run; only files
        // (never directories) are removed by path.
        match fs::symlink_metadata(&file.dst) {
            Ok(meta) if !meta.is_dir() => {
                let _ = fs::remove_file(&file.dst);
            }
            _ => {}
        }
    }
    let mut dirs = journal.created_dirs.clone();
    dirs.sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
    for dir in dirs {
        let _ = fs::remove_dir(dir);
    }
    let _ = fs::remove_file(journal_path(home));
}

/// True when a journal source still holds the very bytes we copied to `dst`.
/// The recorded length rejects a replacement of a different size; comparing
/// against the verified destination also rejects a same-length replacement
/// without pulling a hashing crate into this standalone-compiled file.
fn source_still_original(file: &JournalFile) -> bool {
    let Ok(meta) = fs::symlink_metadata(&file.src) else {
        return false; // already removed, or gone: nothing to delete
    };
    // A directory or symlink now at this path is not the file we migrated.
    if !meta.is_file() {
        return false;
    }
    if file.bytes.is_some_and(|bytes| meta.len() != bytes) {
        return false;
    }
    files_equal(&file.src, &file.dst).unwrap_or(false)
}

/// Finishes the delete phase the interrupted run started: the destination
/// was fully copied and verified, so the recorded sources may be removed,
/// the empty source directories pruned, and the marker recorded. Used after
/// a crash in the `deleting` phase. A source that no longer matches the copy
/// is left in place — deleting it would destroy unrelated data placed there
/// after the crash.
fn recover_deleting(home: &Path, journal: &MigrationJournal) {
    for file in &journal.files {
        if source_still_original(file) {
            let _ = fs::remove_file(&file.src);
        }
    }
    let mut dirs = journal.source_dirs.clone();
    dirs.sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
    for dir in dirs {
        let _ = fs::remove_dir(dir);
    }
    if journal.to_shared {
        let _ = write_marker(home, &journal.target);
    } else {
        let _ = fs::remove_file(home.join(MARKER));
        let _ = fs::remove_dir(&journal.from);
    }
    let _ = fs::remove_file(journal_path(home));
}

/// Recovers an interrupted migration, if any. Called at launcher start and
/// at the top of every migration; safe when no journal exists.
///
/// Returns `Ok(Some(note))` when a journal was found and handled, `Ok(None)`
/// when there was nothing to do, and `Err` when the journal is unreadable or
/// unsafe — in that case nothing is deleted and the caller must surface it.
pub fn recover(home: &Path) -> Result<Option<String>, String> {
    if !journal_path(home).is_file() {
        return Ok(None);
    }
    let journal = read_journal(home).ok_or_else(|| {
        format!(
            "An interrupted storage move left an unreadable {} in app storage. Nothing was deleted.",
            JOURNAL
        )
    })?;
    if !journal_paths_are_safe(&journal) {
        return Err(format!(
            "An interrupted storage move left an inconsistent {JOURNAL} in app storage. Nothing was deleted."
        ));
    }
    match journal.phase {
        "copying" => {
            recover_copying(home, &journal);
            Ok(Some(
                "Rolled back an interrupted storage move: the partial copy was removed and your game files were left in place.".into(),
            ))
        }
        "deleting" => {
            recover_deleting(home, &journal);
            Ok(Some(
                "Completed an interrupted storage move that had already copied and verified every file.".into(),
            ))
        }
        _ => Err(format!(
            "An interrupted storage move left an unknown phase in {JOURNAL}. Nothing was deleted."
        )),
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
    if let Err(error) = recover(home) {
        return Err(error);
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

/// Moves the player's game files from the private home to `shared` — one way.
///
/// `shared` must be empty (or absent); the caller is expected to have created
/// it or let this function create it. Nothing is deleted from the source until
/// every file is copied and size-checked; on failure the destination is rolled
/// back and the source is untouched. Moving *back* to app storage is no longer
/// offered: once all-files access is granted the launcher uses shared storage
/// unconditionally. The flag stays in the signature so older callers get a
/// clear error instead of a link failure; it must be `true`.
pub fn migrate(home: &Path, shared: &Path, to_shared: bool) -> Result<String, String> {
    if !to_shared {
        return Err("Moving game files back to app storage is no longer supported".into());
    }
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
    // A killed migration from an earlier run is finished or undone first,
    // so a half-moved tree is never copied on top of.
    if let Err(error) = recover(home) {
        return Err(error);
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

    // Journal the copy before anything is written, so a crash at any point
    // can be undone (copying) or completed (deleting) on the next start.
    let journal_files: Vec<JournalFile> = plans
        .iter()
        .map(|plan| JournalFile {
            src: plan.src.clone(),
            dst: plan.dst.clone(),
            bytes: Some(plan.bytes),
        })
        .collect();
    let journal_dirs = destination_dirs(to, &plans);
    let journal = MigrationJournal {
        phase: "copying",
        to_shared,
        from: from.to_path_buf(),
        target: to.to_path_buf(),
        files: journal_files,
        created_dirs: journal_dirs,
        source_dirs: source_dirs.iter().cloned().collect(),
    };
    if let Err(error) = write_journal(home, &journal) {
        return Err(format!("Could not record the move in progress: {error}"));
    }

    let mut created: Vec<Created> = Vec::new();
    let mut done = 0u64;
    let copy_result = (|| -> std::io::Result<()> {
        for plan in &plans {
            done += copy_one(plan, &mut created)?;
            PROGRESS_DONE.store(done, Ordering::Relaxed);
        }
        // Every byte is claimed in place; prove it before anything is
        // deleted from the source.
        for plan in &plans {
            verify_file(plan)?;
        }
        Ok(())
    })();
    if let Err(error) = copy_result {
        rollback(&created);
        let _ = fs::remove_file(journal_path(home));
        return Err(format!("Could not copy the game files: {error}"));
    }

    // The copies are in place and verified; only now may the sources go.
    // Record the phase before the first delete so a crash here completes
    // rather than rolls back.
    let deleting = MigrationJournal {
        phase: "deleting",
        ..journal
    };
    if let Err(error) = write_journal(home, &deleting) {
        rollback(&created);
        let _ = fs::remove_file(journal_path(home));
        return Err(format!("Could not record the move in progress: {error}"));
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

    // The marker is the commit point; everything before it rolls back. If
    // writing it fails here the journal stays behind with phase `deleting`,
    // and the next start finishes the marker through `recover`.
    if to_shared {
        if let Err(error) = write_marker(home, shared) {
            return Err(format!("Could not record the storage location: {error}"));
        }
    } else {
        let _ = fs::remove_file(home.join(MARKER));
        // Leave nothing behind when everything moved.
        let _ = fs::remove_dir(shared);
    }
    let _ = fs::remove_file(journal_path(home));
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
        // The migration is one way: a "move back" request is refused and the
        // shared root stays authoritative.
        assert!(migrate(&home, &shared, false).is_err());
        assert_eq!(state(&home), State::Shared);
        assert_eq!(resolve(&home), shared);
        assert_eq!(
            fs::read(shared.join("saves/World One/level.dat")).unwrap(),
            b"level"
        );
        fs::remove_dir_all(&home).unwrap();
        let _ = fs::remove_dir_all(&shared);
    }

    #[test]
    fn native_builds_stay_private_while_native_mod_bundles_move() {
        let home = scratch();
        // What the Java loader reads from filesDir/native on every start.
        fs::create_dir_all(home.join("native/builds")).unwrap();
        fs::write(home.join("native/current.json"), b"current").unwrap();
        fs::write(home.join("native/builds/lib.so"), b"lib").unwrap();
        // Native-mod bundles are player files and must still travel.
        fs::create_dir_all(home.join("horizon-native-mods")).unwrap();
        fs::write(home.join("horizon-native-mods/mod.hmod"), b"bundle").unwrap();
        let shared = home.with_file_name(format!(
            "{}-shared",
            home.file_name().unwrap().to_string_lossy()
        ));
        migrate(&home, &shared, true).unwrap();
        assert_eq!(
            fs::read(home.join("native/current.json")).unwrap(),
            b"current",
            "the loader manifest must stay app-private"
        );
        assert_eq!(fs::read(home.join("native/builds/lib.so")).unwrap(), b"lib");
        assert!(!shared.join("native").exists(), "native builds must not move");
        assert_eq!(
            fs::read(shared.join("horizon-native-mods/mod.hmod")).unwrap(),
            b"bundle"
        );
        assert!(!home.join("horizon-native-mods").exists());
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&shared).unwrap();
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
    fn missing_shared_folder_keeps_the_recorded_root() {
        let home = scratch();
        let recorded = home.join("gone");
        fs::write(
            home.join(MARKER),
            json!({"version": 1, "dir": recorded.to_string_lossy()}).to_string(),
        )
        .unwrap();
        assert_eq!(state(&home), State::Missing);
        // A transient mount or revoked permission must not split the save data
        // across two roots; the launcher handles the failure instead.
        assert_eq!(resolve(&home), recorded);
        assert_eq!(profile_dir(&home, 0), recorded);
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

    #[test]
    fn denylist_keeps_private_paths_out_of_the_plan() {
        let home = scratch();
        seed(&home);
        fs::write(home.join("control.json"), b"{}").unwrap();
        fs::write(home.join("launch-request.json"), b"{}").unwrap();
        fs::create_dir_all(home.join("crash-reports")).unwrap();
        fs::write(home.join("crash-reports/crash.txt"), b"boom").unwrap();
        fs::create_dir_all(home.join("profiles/profile-2/crash-reports")).unwrap();
        fs::write(
            home.join("profiles/profile-2/crash-reports/crash.txt"),
            b"boom",
        )
        .unwrap();
        fs::write(home.join(JOURNAL), b"{}").unwrap();
        let target = scratch();
        let (plans, _dirs, _total) = collect_jobs(&home, &target, true).unwrap();
        assert!(!plans.is_empty(), "game files must be planned");
        for plan in &plans {
            let rel = plan
                .dst
                .strip_prefix(&target)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            assert!(!rel.starts_with("android-"), "android-* planned: {rel}");
            assert!(!rel.starts_with('.'), "dotfile planned: {rel}");
            for component in rel.split('/') {
                assert!(
                    !component.starts_with("android-"),
                    "android-* component in {rel}"
                );
                assert!(
                    !matches!(
                        component,
                        "sessions" | "logs" | "crash-reports" | ".cache" | "cache" | "game.lock"
                    ),
                    "private component in {rel}"
                );
            }
            assert_ne!(rel, "engine-data.zip", "engine build planned: {rel}");
            assert_ne!(
                rel, "shared-storage.json",
                "marker planned: {rel}"
            );
            assert_ne!(
                rel, ".storage-migration.json",
                "journal planned: {rel}"
            );
        }
        // The plan does contain the player-facing files.
        let names: Vec<_> = plans
            .iter()
            .map(|plan| {
                plan.dst
                    .strip_prefix(&target)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert!(names.contains(&"options.txt".to_string()));
        assert!(names.contains(&"saves/World One/level.dat".to_string()));
        assert!(names.iter().any(|n| n == "horizon-mods/test.hmod"));
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&target).unwrap();
    }

    #[test]
    fn is_private_top_level_covers_the_journal_marker_and_lock() {
        for name in [
            "game.lock",
            "engine-data.zip",
            "engine-data.partial",
            "control.json",
            "launch-request.json",
            ".storage-migration.json",
            "shared-storage.json",
            "shared-storage.json.partial",
            "android-launcher.json",
            ".local",
            ".git",
            "native",
        ] {
            assert!(is_private_top_level(name), "{name} must stay private");
        }
        for name in ["saves", "options.txt", "servers.dat", "profiles", "performance"] {
            assert!(!is_private_top_level(name), "{name} may move");
        }
        assert!(is_private_top_level(PROMPT_MARKER));
    }

    fn journal_body(
        phase: &str,
        to_shared: bool,
        from: &Path,
        target: &Path,
        files: &[(&Path, &Path)],
        created_dirs: &[&Path],
        source_dirs: &[&Path],
    ) -> String {
        json!({
            "version": 1,
            "phase": phase,
            "to_shared": to_shared,
            "from": from.to_string_lossy(),
            "target": target.to_string_lossy(),
            "files": files.iter().map(|(src, dst)| json!({
                "src": src.to_string_lossy(),
                "dst": dst.to_string_lossy(),
                "bytes": fs::metadata(src).ok().map(|m| m.len()),
            })).collect::<Vec<_>>(),
            "created_dirs": created_dirs.iter()
                .map(|dir| dir.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            "source_dirs": source_dirs.iter()
                .map(|dir| dir.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
        })
        .to_string()
    }

    #[test]
    fn journal_copying_phase_removes_only_what_this_run_created() {
        let home = scratch();
        fs::create_dir_all(home.join("saves/World One")).unwrap();
        fs::write(home.join("saves/World One/level.dat"), b"world").unwrap();
        fs::write(home.join("options.txt"), b"options").unwrap();
        let target = scratch();
        // Half-copied destination: two files from this run, one dir created
        // here, and a pre-existing empty dir that must survive.
        fs::create_dir_all(target.join("saves/World One")).unwrap();
        fs::write(target.join("options.txt"), b"partial").unwrap();
        fs::write(target.join("saves/World One/level.dat"), b"half").unwrap();
        let target_saves = target.join("saves");
        let target_world = target.join("saves/World One");
        let journal = journal_body(
            "copying",
            true,
            &home,
            &target,
            &[
                (&home.join("options.txt"), &target.join("options.txt")),
                (
                    &home.join("saves/World One/level.dat"),
                    &target.join("saves/World One/level.dat"),
                ),
            ],
            &[&target, &target_saves, &target_world],
            &[],
        );
        fs::write(home.join(JOURNAL), journal).unwrap();
        let note = recover(&home).unwrap();
        assert!(note.is_some());
        assert!(!target.join("options.txt").exists());
        assert!(!target.join("saves").exists(), "created dirs must be pruned");
        assert!(!journal_path(&home).exists());
        // The source is untouched.
        assert_eq!(fs::read(home.join("options.txt")).unwrap(), b"options");
        assert_eq!(
            fs::read(home.join("saves/World One/level.dat")).unwrap(),
            b"world"
        );
        fs::remove_dir_all(&home).unwrap();
        // Recovery may have pruned the target root itself.
        let _ = fs::remove_dir_all(&target);
    }

    #[test]
    fn journal_deleting_phase_completes_the_move() {
        let home = scratch();
        let target = scratch();
        fs::write(home.join("options.txt"), b"options").unwrap();
        fs::create_dir_all(home.join("saves/World One")).unwrap();
        fs::write(home.join("saves/World One/level.dat"), b"world").unwrap();
        fs::write(target.join("options.txt"), b"options").unwrap();
        fs::create_dir_all(target.join("saves/World One")).unwrap();
        fs::write(target.join("saves/World One/level.dat"), b"world").unwrap();
        let journal = journal_body(
            "deleting",
            true,
            &home,
            &target,
            &[
                (&home.join("options.txt"), &target.join("options.txt")),
                (
                    &home.join("saves/World One/level.dat"),
                    &target.join("saves/World One/level.dat"),
                ),
            ],
            &[&target, &target.join("saves"), &target.join("saves/World One")],
            &[&home.join("saves"), &home.join("saves/World One")],
        );
        fs::write(home.join(JOURNAL), journal).unwrap();
        let note = recover(&home).unwrap();
        assert!(note.is_some());
        assert!(!home.join("options.txt").exists(), "sources must be removed");
        assert!(!home.join("saves").exists());
        assert!(target.join("options.txt").is_file(), "copy must stay in place");
        assert_eq!(state(&home), State::Shared);
        assert_eq!(resolve(&home), target);
        assert!(!journal_path(&home).exists());
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&target).unwrap();
    }

    #[test]
    fn journal_deleting_phase_leaves_replaced_sources_alone() {
        let home = scratch();
        let target = scratch();
        // Same length, different content: only the byte match with dst can
        // tell this is no longer the file we migrated.
        fs::write(home.join("options.txt"), b"CHANGED").unwrap();
        fs::write(target.join("options.txt"), b"options").unwrap();
        // Different length: the recorded size rejects it before any read.
        fs::write(home.join("servers.dat"), b"new").unwrap();
        fs::write(target.join("servers.dat"), b"old servers").unwrap();
        // Still the original: this one must be removed.
        fs::write(home.join("options-old.txt"), b"same").unwrap();
        fs::write(target.join("options-old.txt"), b"same").unwrap();
        let journal = journal_body(
            "deleting",
            true,
            &home,
            &target,
            &[
                (&home.join("options.txt"), &target.join("options.txt")),
                (&home.join("servers.dat"), &target.join("servers.dat")),
                (
                    &home.join("options-old.txt"),
                    &target.join("options-old.txt"),
                ),
            ],
            &[],
            &[],
        );
        fs::write(home.join(JOURNAL), journal).unwrap();
        assert!(recover(&home).unwrap().is_some());
        assert_eq!(fs::read(home.join("options.txt")).unwrap(), b"CHANGED");
        assert_eq!(fs::read(home.join("servers.dat")).unwrap(), b"new");
        assert!(!home.join("options-old.txt").exists());
        // The verified copies were never touched.
        assert_eq!(fs::read(target.join("options.txt")).unwrap(), b"options");
        assert_eq!(
            fs::read(target.join("servers.dat")).unwrap(),
            b"old servers"
        );
        assert!(!journal_path(&home).exists());
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&target).unwrap();
    }

    #[test]
    fn journal_with_paths_outside_the_move_is_refused() {
        let home = scratch();
        let outside = scratch();
        let victim = outside.join("keep.txt");
        fs::write(&victim, b"precious").unwrap();
        let journal = journal_body(
            "copying",
            true,
            &home,
            &home.join("target"),
            &[(&home.join("options.txt"), &victim)],
            &[],
            &[],
        );
        fs::write(home.join(JOURNAL), journal).unwrap();
        assert!(recover(&home).is_err());
        assert_eq!(fs::read(&victim).unwrap(), b"precious");
        assert!(journal_path(&home).is_file(), "unsafe journal must stay for inspection");
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn restore_to_app_storage_is_refused_even_with_new_private_data() {
        let home = scratch();
        seed(&home);
        let shared = home.with_file_name(format!(
            "{}-shared",
            home.file_name().unwrap().to_string_lossy()
        ));
        migrate(&home, &shared, true).unwrap();
        // App storage gets a new file at a path the shared folder holds.
        fs::write(home.join("options.txt"), b"new user data").unwrap();
        let result = migrate(&home, &shared, false);
        assert!(result.is_err(), "the one-way migration refuses to move back");
        assert_eq!(fs::read(home.join("options.txt")).unwrap(), b"new user data");
        assert_eq!(
            fs::read(shared.join("options.txt")).unwrap(),
            b"options",
            "the shared copy must stay in place"
        );
        assert_eq!(state(&home), State::Shared);
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&shared).unwrap();
    }

    #[test]
    fn adopt_refuses_when_both_locations_hold_game_files() {
        let home = scratch();
        seed(&home);
        let shared = scratch();
        fs::write(shared.join("someone-else.txt"), b"x").unwrap();
        let result = adopt(&home, &shared);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Both"));
        assert!(home.join("options.txt").is_file());
        assert!(shared.join("someone-else.txt").is_file());
        fs::remove_dir_all(&home).unwrap();
        fs::remove_dir_all(&shared).unwrap();
    }

    #[test]
    fn restore_to_app_storage_is_refused_with_a_clear_reason() {
        let home = scratch();
        seed(&home);
        let gone = home.join("gone");
        fs::write(
            home.join(MARKER),
            json!({"version": 1, "dir": gone.to_string_lossy()}).to_string(),
        )
        .unwrap();
        let result = migrate(&home, &gone, false);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Moving game files back to app storage is no longer supported"));
        assert_eq!(state(&home), State::Missing);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn prompt_marker_roundtrip_stays_private() {
        let home = scratch();
        assert!(!prompt_done(&home));
        mark_prompt_done(&home).unwrap();
        assert!(prompt_done(&home));
        fs::remove_dir_all(&home).unwrap();
    }
}
