//! Native mods in the launcher (`docs/android-native-mods.md`): the `.hnmod` files an instance has, which
//! of them are on, and the build of this game's library that has them compiled in.
//!
//! A native mod's code is compiled into the game, so playing one means playing another build of the
//! same game. The build is made by a service (`packaging/android/mod_build_server.py`): the
//! launcher sends the mods that are on, the service compiles this commit with them, and the
//! launcher keeps the library under `files/native/compiled-<key>/`. `HorizonActivity` loads the
//! chosen one (`current`) in place of the APK's own library.
//!
//! The protocol is Starling's (`docs/android-native-mods.md`), which this fork keeps so the same service
//! and the same `.hnmod` format work: `POST /v1/build?version=…&commit=…&via=github` with a body
//! of `[u32 big-endian length][zip]…`, `GET /v1/job/<key>` while it compiles, and
//! `GET /v1/job/<key>/lib` (Range resumes) for the library.
use horizon_client::mod_list::{self, ModList};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// The service's default address. Empty until this fork publishes one: a player points the
/// launcher at their own (`Settings > Native mods > Build server`), which is also how a build
/// server on a PC is reached from a phone on the same network.
pub const DEFAULT_SERVER: &str = "";
/// The most the service takes in one request (`mod_build_server.py`'s `MAX_BODY`).
const LARGEST_REQUEST: usize = 32 << 20;
/// Where the builds are, under the launcher's home (`filesDir`): the game process reads it too.
const DIR: &str = "native";
/// The library's name in a build's folder, as `HorizonActivity` loads it.
const LIBRARY: &str = "libhorizon_android.so";
/// The build chosen for the next start, and the install of the APK it was made under.
const CURRENT: &str = "current";
const APK: &str = "apk";
/// A build's folder starts with this; a new build takes the place of the last.
const COMPILED: &str = "compiled-";
/// How often a job is looked at, and how long it may take at all.
const POLL: Duration = Duration::from_secs(2);
const LONGEST: Duration = Duration::from_secs(40 * 60);
const LONGEST_ON_GITHUB: Duration = Duration::from_secs(90 * 60);
/// How long a sleeping gateway may take to wake.
const WAKING: Duration = Duration::from_secs(5 * 60);
/// Tries in a row that brought nothing before a look or a download gives up.
const QUIET_TRIES: u32 = 15;
/// What the player is told before the first native mod is built.
pub const WARNING: &str = "Native mods aren't sandboxed: they run with full access to this device, like any app. Install only mods whose authors you trust. Building needs a build server (Settings > Native mods) or the project's gateway.";

#[derive(Default)]
struct Job {
    running: bool,
    text: String,
    error: Option<String>,
    done: Option<String>,
    cancel: Option<Arc<AtomicBool>>,
}
static JOB: Mutex<Job> = Mutex::new(Job {
    running: false,
    text: String::new(),
    error: None,
    done: None,
    cancel: None,
});

/// The commit this game was built from: the full one when the build named it (`HORIZON_ANDROID_BUILD_SHA`,
/// set by `tools/android_build.py`), else the short one shown in the launcher. A build service fetches
/// the full commit by name; a source export has neither and falls back to `versions.json` there.
pub fn build_commit() -> Option<&'static str> {
    let full = option_env!("HORIZON_ANDROID_BUILD_SHA").filter(|commit| {
        commit.len() == 40
            && commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    });
    full.or(option_env!("HORIZON_ANDROID_BUILD_COMMIT"))
}

fn dir(home: &Path) -> PathBuf {
    home.join(DIR)
}

fn chosen(home: &Path) -> Option<String> {
    let name = std::fs::read_to_string(dir(home).join(CURRENT)).ok()?;
    let name = name.trim();
    (!name.is_empty() && plain_build(name)).then(|| name.to_owned())
}

fn plain_build(name: &str) -> bool {
    name.starts_with(COMPILED)
        && name.len() <= 80
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

fn plain_job(key: &str) -> bool {
    key.len() == 24
        && key
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Whether `text` is a build server's address: HTTP(S) to a host, maybe with a port, no path.
pub fn plain_server(text: &str) -> bool {
    let text = text.trim().trim_end_matches('/');
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"));
    let Some(rest) = rest else {
        return false;
    };
    if rest.is_empty() || rest.contains(['/', '?', '#', '@', ' ']) {
        return false;
    }
    let (host, port) = match rest.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (rest, None),
    };
    let host_ok = !host.is_empty()
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'));
    let port_ok = port.is_none_or(|port| {
        !port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit())
    });
    host_ok && port_ok
}

/// A build's `for` (the version name it was made for) and `apk` (the install it was made under).
fn build_text(folder: &Path, file: &str) -> Option<String> {
    let text = std::fs::read_to_string(folder.join(file)).ok()?;
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// Whether the build `name` loads at the next start: its library is there, made for `version` and
/// under this very install of the APK (the same test `NativeMods.chosen` makes).
fn loads(home: &Path, name: &str, version: &str, stamp: Option<&str>) -> bool {
    let folder = dir(home).join(name);
    folder.join(LIBRARY).is_file()
        && build_text(&folder, "for").as_deref() == Some(version)
        && stamp.is_some_and(|stamp| build_text(&folder, APK).as_deref() == Some(stamp))
}

/// The mods a build was made with, from its `build.json`.
fn build_mods(folder: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(folder.join("build.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    value["mods"]
        .as_array()
        .map(|mods| {
            mods.iter()
                .filter_map(|m| {
                    Some((
                        m["id"].as_str()?.to_owned(),
                        m["version"].as_str().unwrap_or_default().to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The `.hnmod`s in an instance's folder, read. An unreadable one keeps its file name and no
/// version, so it is listed and can be removed.
pub fn installed(game: &Path) -> Vec<(String, String, String, String)> {
    let folder = game.join(mod_list::NATIVE_DIR);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&folder)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.retain(|p| p.extension().is_some_and(|e| e == "hnmod"));
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let file = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            match horizon_mods::index::native::Bundle::read(&path) {
                Ok(bundle) => (
                    file,
                    bundle.manifest.id,
                    bundle.manifest.version,
                    String::new(),
                ),
                Err(error) => (
                    file.clone(),
                    file.trim_end_matches(".hnmod").to_owned(),
                    String::new(),
                    error,
                ),
            }
        })
        .collect()
}

/// The mods that are on in `game`: their id and version, and the digest of their files. The
/// launcher builds exactly these, and compares a build's list with them.
pub fn enabled(game: &Path) -> Vec<(String, String, String)> {
    let off = ModList::load(game).disabled;
    let folder = game.join(mod_list::NATIVE_DIR);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&folder)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.retain(|p| p.extension().is_some_and(|e| e == "hnmod"));
    let mut mods = Vec::new();
    for path in paths {
        let Ok(bundle) = horizon_mods::index::native::Bundle::read(&path) else {
            continue;
        };
        if off.contains(&bundle.manifest.id) {
            continue;
        }
        let digest = horizon_mods::index::native::Bundle::digest(&bundle);
        mods.push((bundle.manifest.id, bundle.manifest.version, digest));
    }
    mods.sort();
    mods
}

/// What the launcher's snapshot says about native mods: the files, the builds, the chosen one and
/// the compile under way.
pub fn snapshot(home: &Path, game: &Path, version: &str, stamp: Option<&str>) -> serde_json::Value {
    let list = ModList::load(game);
    let mods: Vec<serde_json::Value> = installed(game)
        .into_iter()
        .map(|(file, id, version, error)| {
            serde_json::json!({
                "file": file,
                "id": id,
                "version": version,
                "enabled": !list.disabled.contains(&id),
                "error": (!error.is_empty()).then_some(error),
            })
        })
        .collect();
    let current = chosen(home);
    let mut builds = Vec::new();
    for entry in std::fs::read_dir(dir(home)).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !plain_build(&name) || !entry.path().is_dir() {
            continue;
        }
        let folder = entry.path();
        let mods = build_mods(&folder);
        let about: serde_json::Value = std::fs::read_to_string(folder.join("build.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        builds.push(serde_json::json!({
            "name": name,
            "chosen": current.as_deref() == Some(name.as_str()),
            "loads": loads(home, &name, version, stamp),
            "for": build_text(&folder, "for"),
            "commit": about["commit"].as_str(),
            "size": about["size"].as_u64(),
            "mods": mods.into_iter().map(|(id, version)| serde_json::json!({"id": id, "version": version})).collect::<Vec<_>>(),
        }));
    }
    builds.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let job = JOB.lock().unwrap_or_else(|e| e.into_inner());
    let enabled = enabled(game);
    let built = current
        .as_deref()
        .map(|name| build_mods(&dir(home).join(name)))
        .unwrap_or_default();
    let up_to_date = !enabled.is_empty()
        && enabled.len() == built.len()
        && enabled
            .iter()
            .all(|(id, version, _)| built.iter().any(|(a, b)| a == id && b == version));
    let config = crate::catalog::Config::load(home);
    serde_json::json!({
        "warning": WARNING,
        "server": config.native_server,
        "github": config.native_github,
        "version": version,
        "commit": build_commit(),
        "stamp": stamp,
        "mods": mods,
        "enabled": enabled.len(),
        "up_to_date": up_to_date,
        "current": current,
        "builds": builds,
        "job": {
            "running": job.running,
            "text": job.text,
            "error": job.error,
            "done": job.done,
        },
    })
}

/// Turns native mod `id` on or off. A native mod is on until turned off; the next build leaves
/// out the ones that are off.
pub fn toggle(home: &Path, game: &Path, id: &str) -> Result<(), String> {
    if !installed(game)
        .iter()
        .any(|(_, file_id, _, _)| file_id == id)
    {
        return Err("That native mod is not installed".into());
    }
    let mut list = ModList::load(game);
    if list.disabled.remove(id) {
        list.enabled.insert(id.to_owned());
    } else {
        list.enabled.remove(id);
        list.disabled.insert(id.to_owned());
    }
    list.save(game);
    let _ = home;
    Ok(())
}

/// Removes native mod `id`'s file. A build that has it stays as it is until the next build.
pub fn remove(game: &Path, id: &str) -> Result<(), String> {
    let Some((file, _, _, _)) = installed(game)
        .into_iter()
        .find(|(_, file_id, _, _)| file_id == id)
    else {
        return Err("That native mod is not installed".into());
    };
    let path = game.join(mod_list::NATIVE_DIR).join(&file);
    std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut list = ModList::load(game);
    list.enabled.remove(id);
    list.disabled.remove(id);
    list.save(game);
    Ok(())
}

/// Chooses build `name` for the next start, or stops using a build (`None`).
pub fn choose(home: &Path, name: Option<&str>) -> Result<(), String> {
    let dir = dir(home);
    match name {
        None => {
            if let Err(error) = std::fs::remove_file(dir.join(CURRENT)) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    return Err(error.to_string());
                }
            }
        }
        Some(name) if plain_build(name) && dir.join(name).join(LIBRARY).is_file() => {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(dir.join(CURRENT), name).map_err(|e| e.to_string())?;
            // Using a build again is a fresh try: its count of starts that never showed goes, and
            // with it the notice that set it aside (`NativeMods.kt`).
            let _ = std::fs::remove_file(dir.join(name).join("starts"));
            let _ = std::fs::remove_file(dir.join("notice"));
        }
        Some(_) => return Err("That build is not on this device".into()),
    }
    Ok(())
}

/// Deletes build `name`'s library and metadata.
pub fn forget(home: &Path, name: &str) -> Result<(), String> {
    if !plain_build(name) {
        return Err("That is not a build".into());
    }
    let folder = dir(home).join(name);
    if chosen(home).as_deref() == Some(name) {
        let _ = std::fs::remove_file(dir(home).join(CURRENT));
    }
    std::fs::remove_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))
}

/// Why a refused request says what it does (the service's JSON `error`), or that it can't be
/// reached.
fn service_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(code, response) => response
            .into_string()
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| value["error"].as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("The build server answered {code}.")),
        ureq::Error::Transport(_) => {
            "The build server can't be reached right now. Check the address in Settings > Native mods."
                .into()
        }
    }
}

/// The request's body: the enabled mods, each `for_build`ed (content left out) and sorted, with
/// its length in front. The same mods always make the same bytes, so the service knows it built
/// them before.
pub fn request_body(game: &Path) -> Result<Vec<u8>, String> {
    let off = ModList::load(game).disabled;
    let folder = game.join(mod_list::NATIVE_DIR);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&folder)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.retain(|p| p.extension().is_some_and(|e| e == "hnmod"));
    let mut mods = Vec::new();
    for path in paths {
        let Ok(bundle) = horizon_mods::index::native::Bundle::read(&path) else {
            continue;
        };
        if off.contains(&bundle.manifest.id) {
            continue;
        }
        if let Some(hooks) = bundle.manifest.hooks
            && hooks > crate::game::NATIVE_HOOKS_API
        {
            return Err(format!(
                "{} needs native hooks API {hooks}; this Horizon has {}.",
                bundle.manifest.id,
                crate::game::NATIVE_HOOKS_API
            ));
        }
        mods.push(
            bundle
                .for_build()
                .map_err(|e| format!("{}: {e}", path.display()))?,
        );
    }
    if mods.is_empty() {
        return Err("There is no native mod turned on to build.".into());
    }
    mods.sort();
    let mut body = Vec::new();
    for bytes in mods {
        body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        body.extend_from_slice(&bytes);
    }
    if body.len() > LARGEST_REQUEST {
        return Err(
            "The native mods turned on are too big to build together. Turn some off and try again."
                .into(),
        );
    }
    Ok(body)
}

/// Starts compiling the mods turned on into this game's library. The work is on a thread of its
/// own; [`snapshot`] says how it is going.
pub fn start(
    home: &Path,
    game: &Path,
    version: &str,
    commit: Option<&str>,
    server: &str,
    github: bool,
    stamp: Option<&str>,
) -> Result<(), String> {
    let server = server.trim().trim_end_matches('/');
    if !plain_server(server) {
        return Err(
            "Set the native mod build server first (Settings > Native mods > Build server).".into(),
        );
    }
    let stamp = stamp
        .filter(|s| !s.is_empty())
        .ok_or("The launcher does not know this install of the APK; restart it.")?
        .to_owned();
    let body = request_body(game)?;
    let mut job = JOB.lock().unwrap_or_else(|e| e.into_inner());
    if job.running {
        return Err("A native mod build is already running.".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    *job = Job {
        running: true,
        text: "Sending the mods to the build server...".into(),
        error: None,
        done: None,
        cancel: Some(cancel.clone()),
    };
    drop(job);
    let home = home.to_owned();
    let version = version.to_owned();
    let commit = commit.unwrap_or_default().to_owned();
    let server = server.to_owned();
    std::thread::Builder::new()
        .name("native mod build".into())
        .spawn(move || {
            let result = build_job(
                &home, &version, &commit, &server, github, &stamp, &body, &cancel,
            );
            let mut job = JOB.lock().unwrap_or_else(|e| e.into_inner());
            job.running = false;
            job.cancel = None;
            match result {
                Ok(name) => {
                    job.done = Some(name);
                    job.text = "Built. It plays from the next start.".into();
                }
                Err(error) => {
                    job.error = Some(error.clone());
                    job.text = error;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Stops the compile (the service keeps building; its result is not downloaded).
pub fn cancel() {
    let job = JOB.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(cancel) = &job.cancel {
        cancel.store(true, Ordering::Relaxed);
    }
}

fn stopped(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Build cancelled.".into())
    } else {
        Ok(())
    }
}

fn say(text: impl Into<String>) {
    JOB.lock().unwrap_or_else(|e| e.into_inner()).text = text.into();
}

#[allow(clippy::too_many_arguments)]
fn build_job(
    home: &Path,
    version: &str,
    commit: &str,
    server: &str,
    github: bool,
    stamp: &str,
    body: &[u8],
    cancel: &AtomicBool,
) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(40))
        .build();
    let mut query = format!("?version={}", urlencode(version));
    if !commit.is_empty() && commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        query.push_str("&commit=");
        query.push_str(commit);
    }
    if github {
        query.push_str("&via=github");
    }
    let longest = if github { LONGEST_ON_GITHUB } else { LONGEST };
    let started = Instant::now();
    let key = loop {
        stopped(cancel)?;
        let answer: serde_json::Value = agent
            .post(&format!("{server}/v1/build{query}"))
            .set("Content-Type", "application/octet-stream")
            .set("User-Agent", &format!("Horizon/{version}"))
            .send_bytes(body)
            .map_err(service_error)?
            .into_json()
            .map_err(|e| e.to_string())?;
        if answer["starting"].as_bool() != Some(true) {
            break answer["job"].as_str().unwrap_or_default().to_owned();
        }
        if started.elapsed() > WAKING {
            return Err("The build server did not wake up. Try again later.".into());
        }
        say("Waking the build server up: about a minute...");
        std::thread::sleep(POLL * 3);
    };
    if !plain_job(&key) {
        return Err("The build server gave no job.".into());
    }
    let mut quiet = 0;
    let finished = loop {
        stopped(cancel)?;
        if started.elapsed() > longest {
            return Err("The build took too long. Try again later.".into());
        }
        std::thread::sleep(POLL);
        let looked = agent
            .get(&format!("{server}/v1/job/{key}"))
            .call()
            .map_err(service_error)
            .and_then(|r| {
                r.into_json::<serde_json::Value>()
                    .map_err(|e| e.to_string())
            });
        let status = match looked {
            Ok(status) => status,
            Err(error) => {
                quiet += 1;
                if quiet >= QUIET_TRIES {
                    return Err(error);
                }
                continue;
            }
        };
        quiet = 0;
        match status["state"].as_str() {
            Some("starting") => say("Waking the build server up: about a minute..."),
            Some("queued") => {
                let ahead = status["ahead"].as_u64().unwrap_or(0);
                say(format!("In line: {ahead} before yours"));
            }
            Some("building") => {
                let seconds = status["seconds"].as_u64().unwrap_or(0);
                say(format!("Compiling: {}s", seconds));
            }
            Some("done") => break status,
            Some("failed") => {
                return Err(status["error"]
                    .as_str()
                    .unwrap_or("The build failed.")
                    .to_owned());
            }
            _ => return Err("The build server lost the job. Try again.".into()),
        }
    };
    let want = finished["sha256"].as_str().unwrap_or_default().to_owned();
    let size = finished["size"].as_u64().unwrap_or(0);
    if want.len() != 64 || size == 0 {
        return Err("The build does not say its SHA-256.".into());
    }
    let about = finished["build"].to_string();
    if !finished["build"].is_object() {
        return Err("The build does not say what it is.".into());
    }
    say("Downloading...");
    let name = format!("{COMPILED}{key}");
    let folder = dir(home).join(&name);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    // A build downloaded again after one that never showed starts its count afresh.
    let _ = std::fs::remove_file(folder.join("starts"));
    let _ = std::fs::remove_file(dir(home).join("notice"));
    let part = folder.join(format!("{LIBRARY}.part"));
    let url = format!("{server}/v1/job/{key}/lib");
    fetch_library(&agent, &url, &part, size, &want, cancel)?;
    // A rename: a library this game runs stays mapped as it was.
    std::fs::rename(&part, folder.join(LIBRARY)).map_err(|e| e.to_string())?;
    std::fs::write(folder.join("for"), version).map_err(|e| e.to_string())?;
    std::fs::write(folder.join(APK), stamp).map_err(|e| e.to_string())?;
    std::fs::write(folder.join("build.json"), about.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::write(dir(home).join(CURRENT), &name).map_err(|e| e.to_string())?;
    // The compiled builds before it: 60 MB each.
    for entry in std::fs::read_dir(dir(home)).into_iter().flatten().flatten() {
        let other = entry.file_name().to_string_lossy().into_owned();
        if other.starts_with(COMPILED) && other != name {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    Ok(name)
}

fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// The library into `part`, going on from what is there (`Range`), checked against `want`.
fn fetch_library(
    agent: &ureq::Agent,
    url: &str,
    part: &Path,
    size: u64,
    want: &str,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let on_disk = |path: &Path| std::fs::metadata(path).map_or(0, |m| m.len());
    let mut quiet = 0;
    while on_disk(part) < size {
        stopped(cancel)?;
        let have = on_disk(part);
        let tried = (|| -> Result<(), String> {
            stopped(cancel)?;
            let response = agent
                .get(url)
                .set("Range", &format!("bytes={have}-"))
                .call()
                .map_err(service_error)?;
            let resumed = response.status() == 206;
            let opened = if resumed {
                std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(part)
            } else {
                std::fs::File::create(part)
            };
            let mut file = opened.map_err(|e| format!("{}: {e}", part.display()))?;
            let mut done = if resumed { have } else { 0 };
            let mut body = response.into_reader();
            let mut buffer = vec![0; 256 * 1024];
            loop {
                stopped(cancel)?;
                let n = body.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    return Ok(());
                }
                file.write_all(&buffer[..n])
                    .map_err(|e| format!("{}: {e}", part.display()))?;
                done += n as u64;
                if done > size {
                    return Err("The build server sent more than it said.".into());
                }
                say(format!(
                    "Downloading: {:.0} of {:.0} MB",
                    done as f64 / 1e6,
                    size as f64 / 1e6
                ));
            }
        })();
        let grew = on_disk(part) > have;
        if let Err(error) = tried {
            if cancel.load(Ordering::Relaxed) {
                return Err("Build cancelled.".into());
            }
            if quiet >= QUIET_TRIES {
                return Err(error);
            }
            quiet += 1;
            std::thread::sleep(Duration::from_secs(3));
        } else if grew {
            quiet = 0;
        } else {
            quiet += 1;
            if quiet >= QUIET_TRIES {
                return Err("The build server stopped sending the library. Try again.".into());
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    }
    let mut hash = Sha256::new();
    let mut file = std::fs::File::open(part).map_err(|e| format!("{}: {e}", part.display()))?;
    let mut buffer = vec![0; 256 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if got != want {
        let _ = std::fs::remove_file(part);
        return Err("The download came out damaged. Try again.".into());
    }
    Ok(())
}
