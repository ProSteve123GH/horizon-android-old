//! Screenshot discovery and access for the launcher gallery.
//!
//! F2 writes PNGs into the active instance's `screenshots` folder. This module
//! lists those files without decoding them, resolves opaque gallery IDs back to
//! allowlisted paths, and deletes a file only after the UI asked for it. The
//! Android provider and bridge turn the resolved path into a scoped content URI;
//! no private path is ever handed to a sharing app.

use crate::instances;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const DIR: &str = "screenshots";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Newest,
    Oldest,
    Name,
}

impl Order {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "" | "newest" => Ok(Self::Newest),
            "oldest" => Ok(Self::Oldest),
            "name" => Ok(Self::Name),
            _ => Err("Unknown screenshot order".into()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub profile: u32,
    pub instance: String,
    pub size: u64,
    pub modified: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mime: &'static str,
}

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub modified: u64,
    pub mime: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteResult {
    pub name: String,
    pub deleted: bool,
    pub missing: bool,
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "Invalid screenshot")
}

fn component(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.len() > 240
        || name.chars().any(|c| c.is_control() || "/\\".contains(c))
    {
        return Err(invalid());
    }
    Ok(())
}

/// The MIME type for a supported image extension.
pub fn mime(name: &str) -> Option<&'static str> {
    let extension = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

fn profile_root(home: &Path, profile: u32) -> PathBuf {
    if profile == 0 {
        home.to_owned()
    } else {
        home.join("profiles").join(format!("profile-{profile}"))
    }
}

fn parse_id(id: &str) -> io::Result<(u32, &str)> {
    let (prefix, name) = id.split_once('/').ok_or_else(invalid)?;
    if name.contains('/') {
        return Err(invalid());
    }
    let profile: u32 = prefix
        .strip_prefix('p')
        .ok_or_else(invalid)?
        .parse()
        .map_err(|_| invalid())?;
    if prefix != format!("p{profile}") {
        return Err(invalid());
    }
    component(name)?;
    if mime(name).is_none() {
        return Err(invalid());
    }
    Ok((profile, name))
}

/// The gallery ID for a screenshot in one instance.
pub fn id(profile: u32, name: &str) -> String {
    format!("p{profile}/{name}")
}

/// Resolves an opaque ID to a real file, rejecting traversal and symlinks.
pub fn resolve(home: &Path, id: &str) -> io::Result<PathBuf> {
    let (profile, name) = parse_id(id)?;
    let path = profile_root(home, profile).join(DIR).join(name);
    let relative = path.strip_prefix(home).map_err(|_| invalid())?.to_owned();
    let mut checked = home.to_owned();
    for part in relative.components() {
        checked.push(part);
        match fs::symlink_metadata(&checked) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Err(invalid()),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(error);
            }
            Err(error) => return Err(error),
        }
    }
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(invalid());
    }
    Ok(path)
}

pub fn open(home: &Path, id: &str) -> io::Result<File> {
    File::open(resolve(home, id)?)
}

pub fn inspect(home: &Path, id: &str) -> io::Result<FileInfo> {
    let path = resolve(home, id)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(invalid)?
        .to_owned();
    let metadata = fs::metadata(&path)?;
    let mime = mime(&name).ok_or_else(invalid)?;
    Ok(FileInfo {
        name,
        size: metadata.len(),
        modified: modified_millis(&metadata),
        mime,
    })
}

/// Deletes one screenshot. A file that disappeared after the gallery listed it
/// reports `missing` instead of an error, so a stale tile stays harmless.
pub fn delete(home: &Path, id: &str) -> Result<DeleteResult, String> {
    let name = id
        .split_once('/')
        .map(|(_, name)| name.to_owned())
        .unwrap_or_default();
    match resolve(home, id) {
        Ok(path) => match fs::remove_file(path) {
            Ok(()) => Ok(DeleteResult {
                name,
                deleted: true,
                missing: false,
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(DeleteResult {
                name,
                deleted: false,
                missing: true,
            }),
            Err(error) => Err(format!("Could not delete screenshot: {error}")),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(DeleteResult {
            name,
            deleted: false,
            missing: true,
        }),
        Err(error) => Err(format!("Could not delete screenshot: {error}")),
    }
}

fn modified_millis(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|age| u64::try_from(age.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

fn valid_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn screenshots_in(profile: u32, root: &Path, items: &mut Vec<Item>) {
    let dir = root.join(DIR);
    if !valid_dir(&dir) {
        return;
    }
    let instance = instances::display_name(root, profile);
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if component(&name).is_err() {
            continue;
        }
        let Some(mime) = mime(&name) else {
            continue;
        };
        match entry.file_type() {
            Ok(kind) if kind.is_file() && !kind.is_symlink() => {}
            _ => continue,
        }
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let (width, height) = image::image_dimensions(&path)
            .ok()
            .map(|(width, height)| (Some(width), Some(height)))
            .unwrap_or((None, None));
        items.push(Item {
            id: id(profile, &name),
            name,
            profile,
            instance: instance.clone(),
            size: metadata.len(),
            modified: modified_millis(&metadata),
            width,
            height,
            mime,
        });
    }
}

/// Lists every instance's screenshots, newest first unless another order is asked.
pub fn list(home: &Path, order: Order) -> Vec<Item> {
    let mut profiles = vec![(0, home.to_owned())];
    let profiles_dir = home.join("profiles");
    if valid_dir(&profiles_dir)
        && let Ok(entries) = fs::read_dir(&profiles_dir)
    {
        for entry in entries.flatten() {
            let kind = match entry.file_type() {
                Ok(kind) if kind.is_dir() && !kind.is_symlink() => kind,
                _ => continue,
            };
            if !kind.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(number) = name.strip_prefix("profile-") else {
                continue;
            };
            let Ok(profile) = number.parse::<u32>() else {
                continue;
            };
            if profile == 0 || name != format!("profile-{profile}") {
                continue;
            }
            profiles.push((profile, entry.path()));
        }
    }
    profiles.sort_by_key(|(profile, _)| *profile);
    profiles.dedup_by_key(|(profile, _)| *profile);

    let mut items = Vec::new();
    for (profile, root) in profiles {
        screenshots_in(profile, &root, &mut items);
    }
    sort(&mut items, order);
    items
}

fn sort(items: &mut [Item], order: Order) {
    match order {
        Order::Newest => items.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.id.cmp(&b.id))
        }),
        Order::Oldest => items.sort_by(|a, b| {
            a.modified
                .cmp(&b.modified)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.id.cmp(&b.id))
        }),
        Order::Name => items.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage, save_buffer_with_format};
    use std::{
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
        time::Duration,
    };

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "horizon-screenshots-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, AtomicOrdering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
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

    fn image(path: &Path, width: u32, height: u32, format: ImageFormat) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if format == ImageFormat::Jpeg {
            let pixels = image::RgbImage::from_pixel(width, height, image::Rgb([20, 40, 60]));
            save_buffer_with_format(
                path,
                pixels.as_raw(),
                width,
                height,
                image::ColorType::Rgb8,
                format,
            )
            .unwrap();
            return;
        }
        let pixels = RgbaImage::from_pixel(width, height, Rgba([20, 40, 60, 255]));
        save_buffer_with_format(
            path,
            pixels.as_raw(),
            width,
            height,
            image::ColorType::Rgba8,
            format,
        )
        .unwrap();
    }

    fn instance(home: &Path, id: u32, name: &str) -> PathBuf {
        let root = if id == 0 {
            home.to_owned()
        } else {
            home.join("profiles").join(format!("profile-{id}"))
        };
        fs::create_dir_all(root.join(DIR)).unwrap();
        instances::write_name(&root, name).unwrap();
        root
    }

    #[test]
    fn discovers_supported_formats_in_every_instance_with_metadata() {
        let scratch = Scratch::new();
        let home = scratch.path();
        instance(home, 0, "Default");
        let survival = instance(home, 3, "Survival");
        image(
            &home.join(DIR).join("2026-11-05_10.00.00.png"),
            7,
            4,
            ImageFormat::Png,
        );
        image(
            &survival.join(DIR).join("holiday.jpg"),
            8,
            5,
            ImageFormat::Jpeg,
        );
        image(
            &survival.join(DIR).join("animated.webp"),
            3,
            2,
            ImageFormat::WebP,
        );
        image(&survival.join(DIR).join("old.gif"), 2, 1, ImageFormat::Gif);
        fs::write(survival.join(DIR).join("notes.txt"), b"not an image").unwrap();
        fs::write(survival.join(DIR).join(".hidden.png"), b"not an image").unwrap();

        let items = list(home, Order::Name);
        let names: Vec<_> = items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "2026-11-05_10.00.00.png",
                "animated.webp",
                "holiday.jpg",
                "old.gif"
            ]
        );
        let png = &items[0];
        assert_eq!(png.id, "p0/2026-11-05_10.00.00.png");
        assert_eq!(png.instance, "Default");
        assert_eq!(png.profile, 0);
        assert_eq!((png.width, png.height), (Some(7), Some(4)));
        assert_eq!(png.mime, "image/png");
        assert!(png.size > 0);
        let jpeg = &items[2];
        assert_eq!(jpeg.id, "p3/holiday.jpg");
        assert_eq!(jpeg.instance, "Survival");
        assert_eq!((jpeg.width, jpeg.height), (Some(8), Some(5)));
        assert_eq!(jpeg.mime, "image/jpeg");
        assert_eq!(items[1].mime, "image/webp");
        assert_eq!(items[3].mime, "image/gif");
    }

    #[test]
    fn date_and_name_orders_are_stable() {
        let scratch = Scratch::new();
        let home = scratch.path();
        instance(home, 0, "Default");
        for (index, name) in ["b.png", "a.png", "c.png"].into_iter().enumerate() {
            let path = home.join(DIR).join(name);
            image(&path, 1, 1, ImageFormat::Png);
            let file = fs::OpenOptions::new().write(true).open(path).unwrap();
            file.set_modified(UNIX_EPOCH + Duration::from_secs(1_700_000_000 + index as u64))
                .unwrap();
        }
        let newest = list(home, Order::Newest);
        assert_eq!(
            newest
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["c.png", "a.png", "b.png"]
        );
        let oldest = list(home, Order::Oldest);
        assert_eq!(
            oldest
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["b.png", "a.png", "c.png"]
        );
        let named = list(home, Order::Name);
        assert_eq!(
            named
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["a.png", "b.png", "c.png"]
        );
    }

    #[test]
    fn a_large_gallery_lists_every_file_without_skipping() {
        let scratch = Scratch::new();
        let home = scratch.path();
        instance(home, 0, "Default");
        for index in 0..160 {
            image(
                &home.join(DIR).join(format!("shot-{index:03}.png")),
                1,
                1,
                ImageFormat::Png,
            );
        }
        let items = list(home, Order::Name);
        assert_eq!(items.len(), 160);
        assert_eq!(items.first().unwrap().name, "shot-000.png");
        assert_eq!(items.last().unwrap().name, "shot-159.png");
    }

    #[test]
    fn high_resolution_screenshots_report_their_real_size() {
        let scratch = Scratch::new();
        let home = scratch.path();
        instance(home, 0, "Default");
        image(&home.join(DIR).join("4k.png"), 4096, 2160, ImageFormat::Png);
        let items = list(home, Order::Newest);
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].width, items[0].height), (Some(4096), Some(2160)));
        assert!(items[0].size > 0);
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_instance_is_skipped_without_crashing() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new();
        let home = scratch.path();
        instance(home, 0, "Default");
        image(&home.join(DIR).join("private.png"), 3, 3, ImageFormat::Png);
        let dir = home.join(DIR);
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).unwrap();
        let blocked = fs::read_dir(&dir).is_err();
        let items = list(home, Order::Newest);
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        if blocked {
            assert!(items.is_empty());
        } else {
            // A privileged test runner can still read the folder; the important
            // contract is that listing never panics either way.
            assert_eq!(items.len(), 1);
        }
    }

    #[test]
    fn empty_missing_and_broken_directories_are_harmless() {
        let scratch = Scratch::new();
        let home = scratch.path();
        assert!(list(home, Order::Newest).is_empty());
        fs::write(home.join(DIR), b"not a directory").unwrap();
        assert!(list(home, Order::Newest).is_empty());
        fs::remove_file(home.join(DIR)).unwrap();
        fs::create_dir_all(home.join(DIR)).unwrap();
        fs::write(home.join(DIR).join("broken.png"), b"not a png").unwrap();
        let items = list(home, Order::Name);
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].width, items[0].height), (None, None));
        assert!(fs::metadata(home.join(DIR).join("broken.png")).is_ok());
    }

    #[test]
    fn ids_resolve_only_inside_screenshots() {
        let scratch = Scratch::new();
        let home = scratch.path();
        instance(home, 0, "Default");
        instance(home, 4, "Creative");
        image(&home.join(DIR).join("shot.png"), 1, 1, ImageFormat::Png);
        image(
            &home.join("profiles/profile-4").join(DIR).join("shot.png"),
            1,
            1,
            ImageFormat::Png,
        );
        assert_eq!(
            resolve(home, "p4/shot.png").unwrap(),
            home.join("profiles/profile-4/screenshots/shot.png")
        );
        assert_eq!(
            resolve(home, "p0/shot.png").unwrap(),
            home.join("screenshots/shot.png")
        );
        for bad in [
            "",
            "p0",
            "shot.png",
            "p0/../shot.png",
            "p0/../../etc/passwd",
            "p00/shot.png",
            "p-1/shot.png",
            "p4x/shot.png",
            "p0/.hidden.png",
            "p0/notes.txt",
            "p0/shot.png/extra",
            "p0/shot.png\n",
            "p0/a\\b.png",
        ] {
            assert!(resolve(home, bad).is_err(), "{bad:?} should be rejected");
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let outside = scratch.path().join("outside.png");
            image(&outside, 1, 1, ImageFormat::Png);
            symlink(&outside, home.join(DIR).join("linked.png")).unwrap();
            assert!(resolve(home, "p0/linked.png").is_err());
            assert!(
                list(home, Order::Name)
                    .iter()
                    .all(|item| item.name != "linked.png")
            );
            fs::remove_file(home.join(DIR).join("linked.png")).unwrap();
            fs::remove_dir_all(home.join(DIR)).unwrap();
            symlink(scratch.path(), home.join(DIR)).unwrap();
            assert!(
                list(home, Order::Name)
                    .iter()
                    .all(|item| item.name != "linked.png" && item.name != "outside.png")
            );
        }
    }

    #[test]
    fn deletion_removes_the_original_and_tolerates_a_second_request() {
        let scratch = Scratch::new();
        let home = scratch.path();
        instance(home, 0, "Default");
        let file = home.join(DIR).join("shot.png");
        image(&file, 2, 2, ImageFormat::Png);
        let first = delete(home, "p0/shot.png").unwrap();
        assert_eq!(
            first,
            DeleteResult {
                name: "shot.png".into(),
                deleted: true,
                missing: false
            }
        );
        assert!(!file.exists());
        assert!(list(home, Order::Newest).is_empty());
        let second = delete(home, "p0/shot.png").unwrap();
        assert_eq!(
            second,
            DeleteResult {
                name: "shot.png".into(),
                deleted: false,
                missing: true
            }
        );
        assert!(delete(home, "p0/../outside.png").is_err());
    }

    #[test]
    fn inspect_reports_share_metadata_without_a_private_path() {
        let scratch = Scratch::new();
        let home = scratch.path();
        instance(home, 2, "Survival");
        let file = home.join("profiles/profile-2/screenshots/shot.jpg");
        image(&file, 12, 9, ImageFormat::Jpeg);
        let info = inspect(home, "p2/shot.jpg").unwrap();
        assert_eq!(info.name, "shot.jpg");
        assert_eq!(info.mime, "image/jpeg");
        assert_eq!(info.size, fs::metadata(&file).unwrap().len());
        assert!(info.modified > 0);
        assert!(open(home, "p2/shot.jpg").is_ok());
    }
}
