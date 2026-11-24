//! Runtime support for native mods that are already installed.
//!
//! The launcher no longer builds native mods and no longer packs `.hnmod`
//! files: upstream retired that format, and the private game build that
//! compiled a mod's Rust into Horizon is gone with it. What stays here is the
//! small amount of code the rest of the launcher still needs to *list*, toggle
//! and describe mods installed by an older launcher: their folder name, the
//! `native-mods.json` a build left in its version, and their file digests. The
//! native host API used by bundled features and voice chat is untouched.
use crate::backend;
use crate::i18n::{tr, trf};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const PREFIX: &str = "native-";
/// Where an instance keeps native mods installed by older launchers, `<id>.hnmod`,
/// each on or off in `horizon-mods.txt` like any mod.
pub const INSTALLED: &str = "horizon-native-mods";

/// The build profile kept in settings. It no longer selects a compilation profile,
/// but older settings files still carry it, so parsing stays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    #[default]
    Performance,
    Dev,
    Release,
}

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Performance => "performance",
            Self::Dev => "dev",
            Self::Release => "release",
        }
    }

    /// The profile named `text`; `fast`, its name before `Dev`, still names it.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "performance" | "fast" => Some(Self::Performance),
            "dev" => Some(Self::Dev),
            "release" => Some(Self::Release),
            _ => None,
        }
    }

    pub fn chosen(settings: &crate::backend::Settings) -> Self {
        settings
            .native_profile
            .unwrap_or(if settings.native_release {
                Self::Release
            } else {
                Self::Performance
            })
    }
}

/// Whether an installed version was a native build, rather than a release or
/// Nightly. Such a version's game binary and `native-mods.json` stay readable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    Client,
    Server,
}

/// Why a mod was left out of an old build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    OtherVersion,
    Clash(String),
    Needs(String),
}

/// A mod an old build was made without, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOut {
    pub id: String,
    pub version: String,
    pub why: Why,
}

impl LeftOut {
    /// What the player reads.
    pub fn label(&self) -> String {
        match &self.why {
            Why::OtherVersion => trf(
                "{name} was left out: made for another Horizon version",
                &[("name", &self.id)],
            ),
            Why::Clash(other) => trf(
                "{name} was left out: clashes with {other}",
                &[("name", &self.id), ("other", other)],
            ),
            Why::Needs(other) => trf(
                "{name} was left out: it needs {other}, which was left out",
                &[("name", &self.id), ("other", other)],
            ),
        }
    }

    fn from_json(value: &serde_json::Value) -> Option<Self> {
        let with = || value["with"].as_str().map(str::to_owned);
        Some(Self {
            id: value["id"].as_str()?.to_owned(),
            version: value["version"].as_str()?.to_owned(),
            why: match value["why"].as_str()? {
                "clash" => Why::Clash(with()?),
                "needs" => Why::Needs(with()?),
                _ => Why::OtherVersion,
            },
        })
    }
}

/// What an installed native build was made from: its `native-mods.json`.
#[derive(Clone, Debug, Default, PartialEq)]
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
    /// A build from before `base_horizon` was recorded that compiled another
    /// commit than its Nightly's: mods pinned it, but which ones is unknown.
    pub fn off_its_nightly(&self) -> bool {
        let nightly = self.base.strip_prefix("nightly-").and_then(|r| r.get(11..));
        self.base_horizon.is_none()
            && nightly.is_some_and(|commit| !commit.is_empty() && !self.horizon.starts_with(commit))
    }
}

/// Whether an installed version is a native build.
pub fn is_native(version: &str) -> bool {
    version.starts_with(PREFIX)
}

pub(crate) fn root() -> PathBuf {
    backend::build_dir().join("native")
}

#[derive(serde::Serialize)]
pub struct CacheInfo {
    pub path: PathBuf,
    pub bytes: u64,
    pub cleared_bytes: u64,
    pub limit_gib: u64,
}

pub const DEFAULT_CACHE_GIB: u64 = 30;

fn cache_error(error: crate::native_cache::Error) -> String {
    match error {
        crate::native_cache::Error::Busy => {
            tr("A native build or cache cleanup is running. Try again when it finishes.").into()
        }
        crate::native_cache::Error::Unsafe(path) => trf(
            "Refusing native cache cleanup at an unexpected file or link: {path}",
            &[("path", &path.display().to_string())],
        ),
        crate::native_cache::Error::Io(error) => trf(
            "Could not access the native build cache: {error}",
            &[("error", &error.to_string())],
        ),
    }
}

/// Inspect or clear the old native compiler cache. Nothing is built here.
pub fn cache_info(clear: bool) -> Result<CacheInfo, String> {
    let cache = crate::native_cache::Cache::acquire(&backend::build_dir()).map_err(cache_error)?;
    let bytes = cache.bytes().map_err(cache_error)?;
    let cleared_bytes = if clear {
        cache.clear().map_err(cache_error)?
    } else {
        0
    };
    Ok(CacheInfo {
        path: root().join("target"),
        bytes,
        cleared_bytes,
        limit_gib: backend::Settings::load()
            .native_cache_gib
            .unwrap_or(DEFAULT_CACHE_GIB),
    })
}

fn is_revision(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// What installed native build `version` was made from; None for any other version.
pub fn about(version: &str) -> Option<About> {
    if !is_native(version) {
        return None;
    }
    about_in(&backend::version_dir(version))
}

fn about_in(dir: &Path) -> Option<About> {
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("native-mods.json")).ok()?).ok()?;
    let field = |name: &str| json[name].as_str().map(str::to_owned);
    Some(About {
        base: field("base")?,
        horizon: field("horizon").filter(|r| is_revision(r))?,
        target: match field("target").as_deref() {
            Some("server") => Target::Server,
            _ => Target::Client,
        },
        base_horizon: field("base_horizon").filter(|r| is_revision(r)),
        pinned_by: json["pinned_by"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m.as_str().map(str::to_owned))
            .collect(),
        on_base: json["on_base"].as_bool().unwrap_or(false),
        left_out: json["left_out"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(LeftOut::from_json)
            .collect(),
    })
}

/// A digest of a mod's files (name, length, bytes, sorted by name), read from an
/// old build's `native-mods.json` by the Mods page.
pub fn digest<'a>(files: impl Iterator<Item = (&'a str, &'a [u8])>) -> String {
    let mut hash = Sha256::new();
    for (name, bytes) in files {
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The newest installed release or Nightly in `installed` (newest first).
pub(crate) fn newest_installed(installed: &[String]) -> Option<&String> {
    installed
        .iter()
        .find(|t| backend::is_game_tag(t) || t.starts_with("nightly-"))
}

/// The installed version a native build for an instance that plays `current`
/// started from. Kept so the Mods page can describe an existing build.
pub fn base_for(current: Option<&str>, installed: &[String], update: bool) -> Option<String> {
    let imported = current.and_then(crate::build_import::about).map(|a| a.base);
    match imported.as_deref().or(current) {
        Some(v) if !update && installed.iter().any(|t| t == v) => Some(v.to_owned()),
        _ => newest_installed(installed).cloned(),
    }
}

/// The Horizon commit `version` was built from, from GitHub.
pub fn revision_of(version: &str) -> Result<String, String> {
    let reference = match version.strip_prefix("nightly-") {
        Some(rest) => rest.rsplit('-').next().unwrap_or(rest),
        None => version,
    };
    let url = format!(
        "https://api.github.com/repos/{}/commits/{reference}",
        backend::REPO
    );
    let commit = crate::github::json(&url)?;
    commit["sha"]
        .as_str()
        .filter(|s| is_revision(s))
        .map(str::to_owned)
        .ok_or_else(|| format!("GitHub doesn't know the commit of {version}."))
}

/// Where Horizon declares its native hook points' version, and the line that does.
const HOOKS_FILE: &str = "crates/horizon-client/src/game/native_hooks.rs";
const HOOKS_LINE: &str = "pub(crate) const NATIVE_HOOKS_API: u32 = ";

fn hooks_in(source: &str) -> Option<u32> {
    let line = source.lines().find_map(|l| l.strip_prefix(HOOKS_LINE))?;
    line.trim_end().strip_suffix(';')?.parse().ok()
}

/// The native hook points' version of Horizon at `revision`, from GitHub (None:
/// it has none). Used by the compatibility display for installed mods.
pub(crate) fn hooks_at(revision: &str) -> Option<u32> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{revision}/{HOOKS_FILE}",
        backend::REPO
    );
    let text = backend::get(&url).ok()?.into_string().ok()?;
    hooks_in(&text)
}

/// Copies version folder `from` into `to` but for what a build made its own.
/// Also an imported build's start (`build_import`).
pub(crate) fn copy_base(from: &Path, to: &Path, target: Target) -> Result<(), String> {
    for entry in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name();
        if !carried_over(&name, target) {
            continue;
        }
        let to = to.join(&name);
        let copied = if entry.path().is_dir() {
            backend::copy_dir(&entry.path(), &to)
        } else {
            horizon_platform::prepare::clone_or_copy(&entry.path(), &to)
                .and_then(|()| fs::set_permissions(&to, entry.metadata()?.permissions()))
        };
        copied.map_err(|e| format!("Could not copy {}: {e}", entry.path().display()))?;
    }
    Ok(())
}

fn carried_over(name: &std::ffi::OsStr, target: Target) -> bool {
    let mut own: Vec<String> = [
        "Horizon.app",
        "native-mods.json",
        "native-data",
        "native-content",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    own.push(backend::CLIENT.to_string());
    own.push(".installed".to_string());
    if target == Target::Server {
        own.push("horizon-engine".to_string());
    }
    let named = |n: &str| name == std::ffi::OsStr::new(n);
    !own.iter()
        .any(|n| named(n) || named(&format!("{n}{}", std::env::consts::EXE_SUFFIX)))
}

/// Reads the COFF machine from a Windows PE executable after validating its two
/// signatures. Kept for the toolchain checks that still use it.
pub(crate) fn pe_machine(path: &Path) -> Result<u16, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut dos = [0; 64];
    file.read_exact(&mut dos)
        .map_err(|error| error.to_string())?;
    if &dos[..2] != b"MZ" {
        return Err("missing DOS MZ signature".into());
    }
    let pe_offset = u32::from_le_bytes(dos[0x3c..0x40].try_into().unwrap()) as u64;
    file.seek(SeekFrom::Start(pe_offset))
        .map_err(|error| error.to_string())?;
    let mut header = [0; 6];
    file.read_exact(&mut header)
        .map_err(|error| error.to_string())?;
    if &header[..4] != b"PE\0\0" {
        return Err("missing PE signature".into());
    }
    Ok(u16::from_le_bytes([header[4], header[5]]))
}
