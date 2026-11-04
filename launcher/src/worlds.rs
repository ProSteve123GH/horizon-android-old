//! An instance's single-player worlds (`saves/` in its game folder): what each is, and backing
//! up, copying and deleting them. Backups are made as vanilla's Make Backup makes them
//! (`LevelStorageAccess.makeWorldBackup`): `backups/<yyyy-MM-dd_HH-mm-ss>_<folder>.zip` in the
//! game folder, every file under `<folder>/` but the world's `session.lock`.
use horizon_world::nbt::{self, Limits, Tag};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The lock a running game holds on a world; vanilla's backups leave it out.
const LOCK: &str = "session.lock";

#[derive(Clone, Debug, PartialEq)]
pub struct World {
    /// Its folder's name under `saves/`, the id commands take.
    pub folder: String,
    pub path: PathBuf,
    /// `LevelName`, else the folder's name.
    pub name: String,
    /// Survival, Creative, Adventure, Spectator or Hardcore.
    pub mode: &'static str,
    /// `LastPlayed`, else when level.dat changed.
    pub last_played: SystemTime,
    /// The version that last saved it (`Version.Name`).
    pub version: Option<String>,
    /// Bytes on disk, every file in its folder.
    pub size: u64,
    /// `icon.png`, the picture the game took of it.
    pub icon: Option<PathBuf>,
}

/// The worlds in a game folder's `saves/`, the last played first.
pub fn list(home: &Path) -> Vec<World> {
    collect(home, true, false)
}

/// `list` without walking every file: sizes stay 0, and a folder whose level.dat
/// is missing or unreadable is still shown (named after the folder). The Android
/// world manager uses this: it must not scan a large world on every open, and a
/// partially copied world must stay visible so it can be deleted.
pub fn list_lenient(home: &Path) -> Vec<World> {
    collect(home, false, true)
}

fn collect(home: &Path, sized: bool, lenient: bool) -> Vec<World> {
    let mut worlds: Vec<World> = fs::read_dir(home.join("saves"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| read_with(&e.path(), sized, lenient))
        .collect();
    worlds.sort_by_key(|w| std::cmp::Reverse(w.last_played));
    worlds
}

/// level.dat: gzip, as vanilla writes it, or plain.
fn level_dat(path: &Path) -> Option<Tag> {
    let bytes = fs::read(path).ok()?;
    let mut raw = Vec::new();
    let raw = match flate2::read::GzDecoder::new(&bytes[..])
        .take(64 << 20)
        .read_to_end(&mut raw)
    {
        Ok(_) => raw,
        Err(_) => bytes,
    };
    Some(nbt::read_named(&raw, Limits::default()).ok()?.0.tag)
}

fn read(path: &Path) -> Option<World> {
    read_with(path, true, false)
}

fn read_with(path: &Path, sized: bool, lenient: bool) -> Option<World> {
    // A lenient listing shows recoverable world folders, not stray files or
    // links: deleting through one must never leave the saves tree.
    if lenient
        && (!path.is_dir()
            || path
                .symlink_metadata()
                .map(|meta| meta.file_type().is_symlink())
                .unwrap_or(true))
    {
        return None;
    }
    let dat = path.join("level.dat");
    let changed = match fs::metadata(&dat).and_then(|m| m.modified()) {
        Ok(time) => time,
        Err(_) if lenient => fs::metadata(path).and_then(|m| m.modified()).ok()?,
        Err(_) => return None,
    };
    let folder = path.file_name()?.to_string_lossy().into_owned();
    let root = level_dat(&dat);
    let data = root.as_ref().and_then(|r| r.get("Data"));
    let field = |key: &str| data.and_then(|d| d.get(key));
    let number = |tag: Option<&Tag>| match tag {
        Some(Tag::Byte(v)) => i64::from(*v),
        Some(Tag::Int(v)) => i64::from(*v),
        Some(Tag::Long(v)) => *v,
        _ => 0,
    };
    let text = |tag: Option<&Tag>| match tag {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    // 26.3 keeps hardcore with the difficulty; older saves at the top of Data.
    let hardcore = number(field("difficulty_settings").and_then(|d| d.get("hardcore"))) != 0
        || number(field("hardcore")) != 0;
    let mode = if root.is_none() {
        "Unknown"
    } else {
        match (hardcore, number(field("GameType"))) {
            (true, _) => "Hardcore",
            (_, 1) => "Creative",
            (_, 2) => "Adventure",
            (_, 3) => "Spectator",
            _ => "Survival",
        }
    };
    let last_played = match number(field("LastPlayed")) {
        millis if millis > 0 => UNIX_EPOCH + Duration::from_millis(millis as u64),
        _ => changed,
    };
    let icon = path.join("icon.png");
    Some(World {
        name: text(field("LevelName"))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| folder.clone()),
        folder,
        path: path.to_path_buf(),
        mode,
        last_played,
        version: text(field("Version").and_then(|v| v.get("Name"))),
        size: if sized { size(path) } else { 0 },
        icon: icon.is_file().then_some(icon),
    })
}

fn size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => size(&e.path()),
            Ok(_) => e.metadata().map_or(0, |m| m.len()),
            Err(_) => 0,
        })
        .sum()
}

/// The world in `home` a command names: its folder, or its name (any case).
pub fn find(home: &Path, query: &str) -> Result<World, String> {
    find_in(home, &list(home), query)
}

/// `find` over `list_lenient`, for the Android world manager: a folder with no
/// readable level.dat can still be deleted or inspected.
pub fn find_lenient(home: &Path, query: &str) -> Result<World, String> {
    find_in(home, &list_lenient(home), query)
}

fn find_in(home: &Path, all: &[World], query: &str) -> Result<World, String> {
    all.iter()
        .find(|w| w.folder == query)
        .or_else(|| all.iter().find(|w| w.name.eq_ignore_ascii_case(query)))
        .cloned()
        .ok_or_else(|| format!("No world {query} in {}", home.join("saves").display()))
}

/// `base`, else `base (1)`, `base (2)`... from the `first` count: the first name free in `dir`
/// (vanilla's `FileUtil.findAvailableName`, after making the name safe for every file system).
fn free_name(dir: &Path, base: &str, suffix: &str, first: u32) -> String {
    let safe: String = base
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let safe = safe.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let safe = if safe.is_empty() { "World" } else { safe };
    (first..)
        .map(|n| match n {
            0 => format!("{safe}{suffix}"),
            n => format!("{safe} ({n}){suffix}"),
        })
        .find(|name| !dir.join(name).exists())
        .unwrap()
}

/// Zips the world into the game folder's `backups/`, as vanilla's Make Backup does. Returns the
/// zip's path.
pub fn backup(home: &Path, world: &World) -> Result<PathBuf, String> {
    backup_at(home, world, chrono::Local::now())
}

fn backup_at(
    home: &Path,
    world: &World,
    now: chrono::DateTime<chrono::Local>,
) -> Result<PathBuf, String> {
    let dir = home.join("backups");
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let prefix = format!("{}_{}", now.format("%Y-%m-%d_%H-%M-%S"), world.folder);
    let zip_path = dir.join(free_name(&dir, &prefix, ".zip", 0));
    let made = (|| -> std::io::Result<()> {
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(fs::File::create(&zip_path)?));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        let mut stack = vec![world.path.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries: Vec<_> = fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let path = e.path();
                if e.file_type()?.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.file_name() == Some(LOCK.as_ref()) {
                    continue;
                }
                let relative = path.strip_prefix(&world.path).unwrap_or(&path);
                let mut entry = world.folder.clone();
                for part in relative.components() {
                    entry.push('/');
                    entry.push_str(&part.as_os_str().to_string_lossy());
                }
                zip.start_file(entry, options)?;
                std::io::copy(&mut fs::File::open(&path)?, &mut zip)?;
            }
        }
        zip.finish()?.flush()
    })();
    made.map(|()| zip_path.clone()).map_err(|e| {
        let _ = fs::remove_file(&zip_path);
        format!("Could not back up {}: {e}", world.name)
    })
}

/// Copies the world into a new folder of the same `saves/`, named "<name> (1)", "<name> (2)"...
/// and gives the copy that name in its level.dat so the game's list tells them apart.
pub fn copy(world: &World) -> Result<World, String> {
    let saves = world.path.parent().ok_or("A world folder has a parent")?;
    let folder = free_name(saves, &world.name, "", 1);
    let to = saves.join(&folder);
    // Create the destination exclusively. If a folder appeared after the name
    // check, stop without touching it: a duplicate never merges into, or
    // deletes, another world.
    fs::create_dir(&to).map_err(|e| format!("Could not create {}: {e}", to.display()))?;
    let made = copy_contents(&world.path, &to).and_then(|()| rename_level(&to, &folder));
    if let Err(e) = made {
        let _ = fs::remove_dir_all(&to);
        return Err(format!("Could not copy {}: {e}", world.name));
    }
    read(&to).ok_or_else(|| format!("{} has no level.dat", to.display()))
}

/// Copies the contents into an already-created destination. Links are skipped,
/// and a running world's session lock is left behind.
fn copy_contents(from: &Path, to: &Path) -> std::io::Result<()> {
    for e in fs::read_dir(from)? {
        let e = e?;
        let kind = e.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let target = to.join(e.file_name());
        if kind.is_dir() {
            fs::create_dir(&target)?;
            copy_contents(&e.path(), &target)?;
        } else if e.file_name() != LOCK {
            fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// Sets `Data.LevelName`; a level.dat that can't be read is left as it is.
fn rename_level(world: &Path, name: &str) -> std::io::Result<()> {
    let dat = world.join("level.dat");
    let Some(mut root) = level_dat(&dat) else {
        return Ok(());
    };
    let Some(Tag::Compound(data)) = root.get_mut("Data") else {
        return Ok(());
    };
    data.insert("LevelName".into(), Tag::String(name.into()));
    let bytes = nbt::write_named(
        &nbt::NamedTag {
            name: "".into(),
            tag: root,
        },
        Limits::default(),
    )
    .map_err(std::io::Error::other)?;
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&bytes)?;
    fs::write(dat, gz.finish()?)
}

/// Renames a world the way the game's Edit → Rename does: `Data.LevelName`
/// changes, while the folder (the world's id for commands and backups) stays.
pub fn rename(world: &World, new_name: &str) -> Result<World, String> {
    let name = new_name.trim();
    if name.is_empty() {
        return Err("Give the world a name".into());
    }
    if name.chars().count() > 64 {
        return Err("Keep the world name to 64 characters".into());
    }
    if name.chars().any(char::is_control) {
        return Err("The world name has characters the game cannot show".into());
    }
    if level_dat(&world.path.join("level.dat")).is_none() {
        return Err(format!(
            "{} has no readable level.dat, so it cannot be renamed",
            world.name
        ));
    }
    rename_level(&world.path, name).map_err(|e| format!("Could not rename {}: {e}", world.name))?;
    // The world manager only needs the new name; do not walk every region file.
    read_with(&world.path, false, false).ok_or_else(|| format!("{} has no level.dat", world.folder))
}

/// Deletes the world. Android has no system trash, so the launcher removes the
/// folder only after its own confirmation; desktop moves it to the trash.
pub fn delete(world: &World) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        fs::remove_dir_all(&world.path).map_err(|e| format!("Could not delete {}: {e}", world.name))
    }
    #[cfg(not(target_os = "android"))]
    {
        crate::desktop_seams::to_trash(&world.path)
            .map_err(|e| format!("Could not move {} to the trash: {e}", world.name))
    }
}

#[cfg(test)]
#[path = "worlds/tests.rs"]
mod tests;
