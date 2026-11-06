//! The Compose launcher's bridge (`dev.horizon.android.NativeBridge`).
//!
//! The launcher Activity runs in the app's main process and the game runs in its
//! own `:game` process. This bridge gives the Kotlin UI the Rust launcher's state
//! as one JSON snapshot and takes its actions by name. Every call comes from the
//! Android main thread; slow work stays on the same worker threads the native
//! launcher uses.
use crate::catalog::{Catalog, Page};
use crate::instances;
use crate::launcher::{
    login::Login,
    wardrobe::{CloudAction, Wardrobe},
};
use horizon_client::content_files::Kind;
use jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JObject, JString},
    sys::{jint, jstring},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

struct Controller {
    home: PathBuf,
    catalog: Catalog,
    appearance: crate::appearance::Settings,
    login: Login,
    wardrobe: Wardrobe,
    install: Arc<Mutex<horizon_client::official_install::InstallProgress>>,
    installation: horizon_client::minecraft_import::InstallationInfo,
    worker: Option<mpsc::Receiver<Result<(), String>>>,
    diagnostic_export: Option<mpsc::Receiver<Result<String, String>>>,
    /// Manual server status checks, keyed by the address in `servers.dat`. The UI asks for one
    /// check and reads the result; nothing polls the network by itself.
    server_pings: Arc<Mutex<HashMap<String, crate::servers::Ping>>>,
    error: Option<String>,
}

thread_local! {
    static CONTROLLER: RefCell<Option<Controller>> = const { RefCell::new(None) };
}

/// The file the `:game` process reads to start the game without its own launcher.
pub(crate) const LAUNCH_REQUEST: &str = "launch-request.json";

/// A running game holds the world's `session.lock`; never rewrite or delete it.
fn world_in_use(world: &crate::worlds::World) -> bool {
    horizon_client::save::SessionLock::is_locked(&world.path)
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Mods => "mods",
        Kind::Packs => "packs",
        Kind::Shaders => "shaders",
    }
}
fn parse_kind(name: &str) -> Option<Kind> {
    Kind::ALL.into_iter().find(|k| kind_name(*k) == name)
}
fn page_name(page: Page) -> &'static str {
    match page {
        Page::Play => "play",
        Page::Mods => "mods",
        Page::Packs => "packs",
        Page::Shaders => "shaders",
        Page::Installed => "installed",
        Page::Settings => "settings",
        Page::Skins => "skins",
    }
}

fn profile_dir(home: &std::path::Path, id: u32) -> PathBuf {
    if id == 0 {
        home.into()
    } else {
        home.join("profiles").join(format!("profile-{id}"))
    }
}

fn choice(
    label: impl Into<String>,
    detail: impl Into<String>,
    value: impl ToString,
    selected: bool,
) -> Value {
    json!({"label": label.into(), "detail": detail.into(), "value": value.to_string(), "selected": selected})
}

impl Controller {
    fn new(home: PathBuf) -> Self {
        let cleanup_home = home.clone();
        std::thread::spawn(move || {
            if let Err(error) =
                horizon_client::official_install::cleanup_completed_downloads(&cleanup_home)
            {
                log::warn!("Could not clean completed Minecraft downloads: {error}");
            }
        });
        let mut catalog = Catalog::new(&home);
        if crate::diagnostics::previous_unclean(&home) {
            catalog.status = crate::diagnostics::summary(&home);
        }
        horizon_client::android_runtime::graphics_startup::configure(home.clone());
        if horizon_client::android_runtime::graphics_startup::recover(&home) {
            catalog.config.renderer = crate::catalog::RendererMode::OpenGl;
            if let Err(error) = catalog.config.save(&home) {
                log::warn!("Could not save graphics recovery choice: {error}");
            }
            catalog.status = "Previous Vulkan startup was interrupted. Using OpenGL ES recovery; you can change Rendering mode in Settings.".into();
        }
        let installation = horizon_client::minecraft_import::info(&home);
        let mut controller = Self {
            home: home.clone(),
            catalog,
            appearance: crate::appearance::Settings::load(&home),
            login: Login::new(),
            wardrobe: Wardrobe::new(&home),
            install: Arc::default(),
            installation,
            worker: None,
            diagnostic_export: None,
            server_pings: Arc::default(),
            error: None,
        };
        controller.resume();
        controller
    }

    /// The launcher Activity came back to the front, usually from the game.
    fn resume(&mut self) {
        self.catalog.config = crate::catalog::Config::load(&self.home);
        self.catalog.refresh(&self.home);
        self.installation = horizon_client::minecraft_import::info(&self.home);
        self.login.refresh_status();
        self.wardrobe.open(
            &self.catalog.config.name,
            &self.catalog.config.microsoft_client_id,
        );
        if let Ok(message) = std::fs::read_to_string(self.home.join("android-error.txt")) {
            self.error = Some(message);
        }
    }

    fn poll(&mut self) {
        if let Some(result) = self.worker.as_ref().and_then(|w| w.try_recv().ok()) {
            self.worker = None;
            if result.is_ok() {
                self.installation = horizon_client::minecraft_import::info(&self.home);
                self.catalog.refresh(&self.home);
            }
            if let Err(error) = result {
                self.error = Some(
                    if error.contains("Dns Failed") || error.contains("lookup address") {
                        "Minecraft download interrupted: DNS could not resolve the download host. Check your connection, VPN or Private DNS, then retry Download. Completed files are kept.".into()
                    } else {
                        error
                    },
                );
                crate::diagnostics::record_error(self.error.as_deref().unwrap_or_default());
            }
        }
        if let Some(result) = self
            .diagnostic_export
            .as_ref()
            .and_then(|r| r.try_recv().ok())
        {
            self.diagnostic_export = None;
            self.catalog.status = match result {
                Ok(name) => {
                    let _ = crate::platform::open_documents("reports");
                    format!(
                        "Saved {name} in Reports. Review the ZIP before sharing; nothing was uploaded."
                    )
                }
                Err(error) => format!("Diagnostic export failed: {error}"),
            };
        }
        if let Some(notice) = crate::minecraft_import::poll() {
            self.catalog.status = notice.text;
            if let Some(error) = notice.error {
                self.error = Some(error);
            }
            if notice.changed {
                self.installation = horizon_client::minecraft_import::info(&self.home);
                self.catalog.refresh(&self.home);
            }
        }
        self.catalog.poll(&self.home);
        if let Some(notice) = crate::documents::poll() {
            self.catalog.status = notice.text;
            if notice.changed {
                self.catalog.refresh(&self.home);
            }
        }
        for result in crate::gallery::poll() {
            self.catalog.status = result.unwrap_or_else(|e| format!("File export failed: {e}"));
        }
        let account_changed = self.login.poll();
        self.wardrobe.poll();
        if account_changed {
            self.wardrobe.open(
                &self.catalog.config.name,
                &self.catalog.config.microsoft_client_id,
            );
        }
    }

    fn settings(&self) -> Value {
        let c = &self.catalog.config;
        let row = |id: &str, title: &str, value: String, kind: &str| json!({"id": id, "title": title, "value": value, "kind": kind});
        let switch = |id: &str, title: &str, value: &str, on: bool| json!({"id": id, "title": title, "value": value, "kind": "switch", "on": on});
        json!([
            {"group": "Graphics", "rows": [
                row("renderer", "Rendering", c.renderer.label().into(), "choice"),
                row("render_scale", "Client resolution", format!("{}%", c.render_scale.percent()), "choice"),
                row("display_hz", "Display refresh rate", c.device.display_label(), "choice"),
                switch("performance_hints", "Performance hints", if c.device.performance_hints { "On (ADPF)" } else { "Off" }, c.device.performance_hints),
            ]},
            {"group": "Performance", "rows": [
                row("performance_presets", "Performance preset", crate::performance_presets::selected_label(&c.game_home(&self.home)).unwrap_or_else(|| "No preset selected".into()), "action"),
                row("worker_limit", "Chunk worker limit", if c.worker_limit == 0 { "No additional limit".into() } else { format!("{} worker(s)", c.worker_limit) }, "choice"),
                row("cache_budget_mib", "Reusable RAM cache", if c.cache_budget_mib == 0 { "Engine default (80 MiB)".into() } else { format!("{} MiB", c.cache_budget_mib) }, "choice"),
            ]},
            {"group": "Screen", "rows": [
                row("orientation", "Orientation", c.orientation.label().into(), "choice"),
                row("aspect", "Aspect ratio", c.screen.aspect.label().into(), "choice"),
                switch("immersive", "Immersive fullscreen", if c.screen.immersive { "On" } else { "Off" }, c.screen.immersive),
                switch("cutout", "Avoid camera cutout", if c.screen.avoid_cutout { "On" } else { "Off" }, c.screen.avoid_cutout),
            ]},
            {"group": "Appearance", "rows": [
                row("appearance", "Launcher appearance", self.appearance.summary(), "action"),
            ]},
            {"group": "Game", "rows": [
                row("profile", "Instances", instances::display_name(&profile_dir(&self.home, c.profile), c.profile), "choice"),
                row("input_host", "Input", c.input_host.label().into(), "choice"),
                row("touch_layouts", "Touch layout profiles", crate::touch_layouts::summary(&c.game_home(&self.home)), "action"),
                row("name", "Offline player name", if c.name.is_empty() { "Not set".into() } else { c.name.clone() }, "text"),
                row("server", "Server", if c.server.is_empty() { "Singleplayer".into() } else { c.server.clone() }, "text"),
                row("servers", "Saved servers", "Add, edit and organize".into(), "action"),
            ]},
            {"group": "Storage", "rows": [
                row("screenshots", "Screenshots", "Browse, preview, share or delete".into(), "action"),
                row("game_files", "Game files", "Open in system Files".into(), "action"),
                row("world_files", "World files", "Import / copy / back up in Files".into(), "action"),
            ]},
            {"group": "Diagnostics", "rows": [
                switch("gpu_profile", "GPU profiling", if c.device.gpu_profile { "On / included in report" } else { "Off" }, c.device.gpu_profile),
                switch("record_performance", "Performance recording", if c.record_performance { "On / up to 30 min" } else { "Off" }, c.record_performance),
                row("export_diagnostics", "Diagnostic report", "Export one ZIP to share".into(), "action"),
                row("export_performance", "Raw performance data", "Advanced / JSONL in Reports".into(), "action"),
                row("logs", "Last session summary", "Show what happened last time".into(), "action"),
            ]},
        ])
    }

    fn options(&self, id: &str) -> Value {
        let c = &self.catalog.config;
        let (title, help, choices): (&str, &str, Vec<Value>) = match id {
            "renderer" => (
                "Rendering mode",
                "Uses the system GPU driver. Applies on the next game launch.",
                vec![
                    choice(
                        "OpenGL ES / experimental",
                        "For older GPUs. Requires ES 3.0+ and sufficient driver limits.",
                        "opengl",
                        c.renderer == crate::catalog::RendererMode::OpenGl,
                    ),
                    choice(
                        "Vulkan / GPU batching",
                        "Groups terrain draws and enables supported GPU culling.",
                        "vulkan",
                        c.renderer == crate::catalog::RendererMode::Vulkan,
                    ),
                    choice(
                        "Vulkan / compatibility",
                        "Direct terrain draws for drivers with missing or transparent blocks.",
                        "vulkan_compat",
                        c.renderer == crate::catalog::RendererMode::VulkanCompatibility,
                    ),
                ],
            ),
            "render_scale" => (
                "Client resolution",
                "Lower values render fewer pixels for the whole picture.",
                crate::display::RenderScale::PRESETS
                    .into_iter()
                    .map(|p| {
                        choice(
                            format!("{p}%"),
                            if p == 100 {
                                "Native resolution"
                            } else {
                                "Lower resolution for the whole picture"
                            },
                            p,
                            c.render_scale.percent() == p,
                        )
                    })
                    .collect(),
            ),
            "display_hz" => (
                "Display refresh rate",
                "An Android display preference, not a game FPS cap. Unsupported rates may be ignored.",
                crate::device_preferences::DISPLAY_RATES
                    .into_iter()
                    .map(|hz| {
                        choice(
                            if hz == 0 {
                                "System default".into()
                            } else {
                                format!("{hz} Hz")
                            },
                            "",
                            hz,
                            c.device.display_hz == hz,
                        )
                    })
                    .collect(),
            ),
            "worker_limit" => (
                "Chunk worker limit",
                "Ceiling for generation and meshing workers, not all app threads. Lower values can reduce CPU contention but slow loading. Applies on the next Play.",
                (0..=64u32)
                    .map(|limit| {
                        choice(
                            if limit == 0 {
                                "No additional limit".into()
                            } else {
                                format!("{limit} worker(s)")
                            },
                            "",
                            limit,
                            c.worker_limit == limit,
                        )
                    })
                    .collect(),
            ),
            "cache_budget_mib" => (
                "Reusable RAM cache",
                "Decoded sounds (80%) and empty mesh buffers (20%). Recreated at the same quality. Smaller budgets can increase CPU work. Not total RAM/VRAM. Applies on the next Play.",
                [0, 16, 32, 64, 128, 256]
                    .into_iter()
                    .map(|mib| {
                        choice(
                            if mib == 0 {
                                "Engine default (80 MiB)".into()
                            } else {
                                format!("{mib} MiB")
                            },
                            "",
                            mib,
                            c.cache_budget_mib == mib,
                        )
                    })
                    .collect(),
            ),
            "orientation" => (
                "Screen orientation",
                "Applies to the game. Does not change touch bindings.",
                crate::catalog::Orientation::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(i, o)| choice(o.label(), "", i, c.orientation == o))
                    .collect(),
            ),
            "aspect" => (
                "Aspect ratio",
                "Fits the entire picture without stretching. Portrait swaps width and height.",
                ASPECTS
                    .into_iter()
                    .enumerate()
                    .map(|(i, a)| choice(a.label(), "", i, c.screen.aspect == a))
                    .collect(),
            ),
            "input_host" => (
                "Input mode",
                "Mouse mode uses a physical keyboard. Touch mode keeps the Android keyboard.",
                crate::catalog::InputHost::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| choice(m.label(), "", i, c.input_host == m))
                    .collect(),
            ),
            "profile" => {
                let mut ids = vec![0, c.profile];
                ids.extend(
                    std::fs::read_dir(self.home.join("profiles"))
                        .into_iter()
                        .flatten()
                        .flatten()
                        .filter(|e| e.path().is_dir())
                        .filter_map(|e| {
                            e.file_name()
                                .to_str()?
                                .strip_prefix("profile-")?
                                .parse::<u32>()
                                .ok()
                        }),
                );
                ids.sort_unstable();
                ids.dedup();
                let next = ids.last().copied().unwrap_or(0).checked_add(1);
                let mut choices: Vec<_> = ids
                    .into_iter()
                    .map(|id| {
                        choice(
                            format!("Profile {id}"),
                            if id == 0 {
                                "Default profile"
                            } else {
                                "Separate saves, mods and packs"
                            },
                            id,
                            id == c.profile,
                        )
                    })
                    .collect();
                if let Some(id) = next {
                    choices.push(choice(
                        "Create profile",
                        format!("Create Profile {id}"),
                        id,
                        false,
                    ));
                }
                (
                    "Choose profile",
                    "Each profile keeps its own game settings and saves.",
                    choices,
                )
            }
            _ => ("", "", vec![]),
        };
        json!({"title": title, "help": help, "choices": choices})
    }

    fn snapshot(&mut self) -> Value {
        use horizon_client::official_install::InstallStage;
        self.poll();
        let imported = crate::minecraft_import::snapshot();
        let installed = self.installation.installed;
        let installing = self.worker.is_some() || imported.is_some();
        let progress = *self.install.lock().unwrap_or_else(|e| e.into_inner());
        let current = InstallStage::ALL
            .iter()
            .position(|s| *s == progress.stage)
            .unwrap_or(0);
        let fraction = if progress.total > 0 {
            progress.done as f32 / progress.total as f32
        } else {
            0.0
        };
        let (stage, stage_index, stages, overall, detail, cancelable, importing) = if let Some(
            import,
        ) = imported
        {
            let stages = import["stages"]
                .as_array()
                .map(|stages| {
                    stages
                        .iter()
                        .filter_map(|stage| stage.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (
                import["stage"].as_str().unwrap_or("Importing").to_owned(),
                import["stage_index"].as_u64().unwrap_or(0) as usize,
                stages,
                import["overall"].as_f64().unwrap_or(0.0) as f32,
                import["detail"].as_str().unwrap_or("").to_owned(),
                import["cancelable"].as_bool().unwrap_or(false),
                import["importing"].as_bool().unwrap_or(false),
            )
        } else {
            let detail = match progress.stage {
                InstallStage::Download => {
                    if progress.downloaded == 0 && progress.cached > 0 {
                        format!(
                            "{:.1} / {:.1} MiB verified from existing files · nothing downloaded",
                            progress.done as f64 / 1048576.0,
                            progress.total as f64 / 1048576.0
                        )
                    } else if progress.cached > 0 {
                        format!(
                            "{:.1} / {:.1} MiB checked · {:.1} MiB downloaded, {:.1} MiB reused",
                            progress.done as f64 / 1048576.0,
                            progress.total as f64 / 1048576.0,
                            progress.downloaded as f64 / 1048576.0,
                            progress.cached as f64 / 1048576.0
                        )
                    } else {
                        format!(
                            "{:.1} / {:.1} MiB downloaded from Mojang",
                            progress.done as f64 / 1048576.0,
                            progress.total as f64 / 1048576.0
                        )
                    }
                }
                InstallStage::Controls => {
                    format!(
                        "{} / {} control files verified",
                        progress.done, progress.total
                    )
                }
                _ => "Please keep Horizon open. Downloaded files are kept for retry.".into(),
            };
            (
                progress.stage.label().to_owned(),
                current,
                InstallStage::ALL
                    .iter()
                    .map(|stage| stage.label().to_owned())
                    .collect(),
                (current as f32 + fraction.clamp(0.0, 1.0)) / InstallStage::ALL.len() as f32,
                detail,
                false,
                false,
            )
        };
        let free_bytes = horizon_client::official_install::available_space(&self.home);
        let wardrobe = if self.catalog.page == Page::Skins {
            self.wardrobe.state()
        } else {
            Value::Null
        };
        let accounts = crate::account::Accounts::load();
        let selected_account = accounts.selected.clone();
        let saved_accounts: Vec<Value> = self
            .login
            .saved_accounts
            .iter()
            .map(|account| {
                json!({
                    "key": account.key(),
                    "name": account.name,
                    "provider": match account.provider {
                        crate::account::Provider::Microsoft => "microsoft",
                        crate::account::Provider::Ely => "ely",
                    },
                    "selected": selected_account.as_deref() == Some(account.key().as_str()),
                })
            })
            .collect();
        let c = &self.catalog.config;
        let results: Vec<Value> = self
            .catalog
            .results
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let state = self.catalog.download_label(p);
                let java_mod = p.kind == crate::browser::Kind::Mod
                    && p.source == crate::browser::SourceId::Modrinth;
                let note = if java_mod {
                    Some("Java mod")
                } else if crate::catalog::source_only_mod(p) {
                    Some(".hnmod: later")
                } else if p.kind == crate::browser::Kind::Mod && !cfg!(target_pointer_width = "64")
                {
                    Some("64-bit required")
                } else {
                    None
                };
                json!({
                    "index": i,
                    "title": p.title,
                    "summary": p.summary,
                    "author": p.author,
                    "downloads": p.downloads,
                    "state": state,
                    "note": note,
                    "button": self.catalog.install_button(p),
                })
            })
            .collect();
        let local: Vec<Value> = self
            .catalog
            .local
            .iter()
            .enumerate()
            .map(|(i, item)| {
                json!({
                    "index": i,
                    "kind": kind_name(item.kind),
                    "title": item.title,
                    "file": item.file,
                    "enabled": item.enabled,
                    "toggle": item.kind != Kind::Mods || cfg!(target_pointer_width = "64"),
                })
            })
            .collect();
        let code = match self.login.state() {
            Some(crate::account::SignInState::Code(code)) => {
                Some(json!({"user_code": code.user_code, "uri": code.verification_uri}))
            }
            _ => None,
        };
        json!({
            "version": crate::backend::VERSION,
            "status": self.catalog.status,
            "error": self.error,
            "install": {
                "installed": installed,
                "installing": installing,
                "importing": importing,
                "cancelable": cancelable,
                "stage": stage,
                "stage_index": stage_index,
                "stages": stages,
                "overall": overall,
                "detail": detail,
                "source": self.installation.source,
                "size_bytes": self.installation.size_bytes,
                "cache_bytes": self.installation.cache_bytes,
                "downloaded_bytes": progress.downloaded,
                "cached_bytes": progress.cached,
                "files_missing": self.installation.files_missing,
                "free_bytes": free_bytes,
            },
            "account": {
                "status": self.login.status(&c.name),
                "signed_in": self.login.signed_in,
                "profile": self.login.profile,
                "pending": self.login.pending(),
                "message": self.login.message(),
                "code": code,
                "needs_review": self.login.message().contains("not approved by Mojang"),
                "saved": saved_accounts,
                "offline_name": c.name,
            },
            "config": {
                "name": c.name,
                "server": c.server,
                "profile": c.profile,
                "profile_name": instances::display_name(&profile_dir(&self.home, c.profile), c.profile),
                "renderer": c.renderer.label(),
                "render_scale": c.render_scale.percent(),
                "input": c.input_host.label(),
                "mod_source": if c.mod_source == crate::shader_catalog::Origin::Modrinth { "modrinth" } else { "horizon" },
                "shader_source": if c.shader_source == crate::shader_catalog::Origin::Modrinth { "modrinth" } else { "horizon" },
            },
            "appearance": self.appearance.state(&self.home),
            "settings": self.settings(),
            "wardrobe": wardrobe,
            "catalog": {
                "page": page_name(self.catalog.page),
                "query": self.catalog.query,
                "results": results,
                "offset": self.catalog.offset,
                "busy": self.catalog.busy(),
            },
            "local": local,
            "documents": crate::documents::overlay(),
            "is_64_bit": cfg!(target_pointer_width = "64"),
        })
    }

    fn save(&mut self) -> Result<(), String> {
        self.catalog.config.save(&self.home)
    }

    /// Profile 0 plus every `profiles/profile-N` folder, sorted.
    fn profile_ids(&self) -> Vec<u32> {
        let mut ids = vec![0, self.catalog.config.profile];
        ids.extend(
            std::fs::read_dir(self.home.join("profiles"))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    e.file_name()
                        .to_str()?
                        .strip_prefix("profile-")?
                        .parse::<u32>()
                        .ok()
                }),
        );
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// What the instance manager shows: name, identity, use and storage.
    fn profiles(&self) -> Value {
        let count = |dir: PathBuf| {
            std::fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .count()
        };
        let active = self.catalog.config.profile;
        Value::Array(
            self.profile_ids()
                .into_iter()
                .map(|id| {
                    let dir = profile_dir(&self.home, id);
                    json!({
                        "id": id,
                        "name": instances::display_name(&dir, id),
                        "active": id == active,
                        "deletable": id != 0 && id != active,
                        "missing": !dir.is_dir(),
                        "minecraft": horizon_core::VANILLA_VERSION,
                        "last_played": instances::last_played(&dir),
                        "size_bytes": instances::dir_size(&dir),
                        "worlds": count(dir.join("saves")),
                        "mods": count(dir.join("horizon-mods")),
                        "packs": count(dir.join("resourcepacks")),
                        "shaders": count(dir.join("shaderpacks")),
                    })
                })
                .collect(),
        )
    }

    /// Every instance id with the name shown to the player.
    fn named_profiles(&self) -> Vec<(u32, String)> {
        self.profile_ids()
            .into_iter()
            .map(|id| {
                let dir = profile_dir(&self.home, id);
                (id, instances::display_name(&dir, id))
            })
            .collect()
    }

    fn call(&mut self, method: &str, arg: &str) -> Result<Value, String> {
        let home = self.home.clone();
        match method {
            "resume" => self.resume(),
            "dismiss_error" => {
                self.error = None;
                let _ = std::fs::remove_file(home.join("android-error.txt"));
            }
            "dismiss_status" => self.catalog.status.clear(),
            "page" => {
                let page = Page::ALL
                    .into_iter()
                    .find(|p| page_name(*p) == arg)
                    .ok_or("Unknown page")?;
                self.catalog.page = page;
                self.catalog.offset = 0;
                self.error = None;
                self.catalog.browse(&home);
                if page == Page::Skins {
                    self.wardrobe.open(
                        &self.catalog.config.name,
                        &self.catalog.config.microsoft_client_id,
                    );
                }
            }
            "search" => {
                self.catalog.query = arg.to_owned();
                self.catalog.offset = 0;
                self.catalog.search(&home);
            }
            "next" => {
                self.catalog.offset += 12;
                self.catalog.search(&home);
            }
            "previous" => {
                self.catalog.offset = self.catalog.offset.saturating_sub(12);
                self.catalog.search(&home);
            }
            "source" => {
                let origin = if arg == "modrinth" {
                    crate::shader_catalog::Origin::Modrinth
                } else {
                    crate::shader_catalog::Origin::Horizon
                };
                if self.catalog.page == Page::Mods {
                    self.catalog.config.mod_source = origin;
                } else {
                    self.catalog.config.shader_source = origin;
                }
                self.save()?;
                self.catalog.offset = 0;
                self.catalog.browse(&home);
            }
            "install" => {
                let index = arg.parse::<usize>().map_err(|e| e.to_string())?;
                self.catalog.install(&home, index);
            }
            "cancel_import" => crate::minecraft_import::cancel(),
            "repair_installation" => {
                if self.worker.is_some() || crate::minecraft_import::busy() {
                    return Err("Finish the current installation first".into());
                }
                crate::minecraft_import::start_repair(self.home.clone())?;
                self.error = None;
                self.catalog.status =
                    "Checking Minecraft resources and downloading anything missing...".into();
            }
            "remove_installation" => {
                if self.worker.is_some() || crate::minecraft_import::busy() {
                    return Err("Finish or cancel the current installation first".into());
                }
                horizon_client::minecraft_import::remove(&home)
                    .map_err(|error| error.to_string())?;
                self.installation = horizon_client::minecraft_import::info(&home);
                self.catalog.status =
                    "Minecraft resources removed. Saves, profiles and downloaded content are kept."
                        .into();
            }
            "toggle_local" => {
                let index = arg.parse::<usize>().map_err(|e| e.to_string())?;
                self.catalog.toggle_local(&home, index);
            }
            "remove_local" => {
                let index = arg.parse::<usize>().map_err(|e| e.to_string())?;
                self.catalog.pending_delete = self.catalog.local.get(index).cloned();
                self.catalog.remove_local(&home);
            }
            "open_folder" => {
                let folder = match parse_kind(arg).ok_or("Unknown content kind")? {
                    Kind::Mods => "mods",
                    Kind::Packs => "packs",
                    Kind::Shaders => "shaders",
                };
                let id = crate::document_store::folder_id(self.catalog.config.profile, folder);
                self.catalog.status = if crate::platform::open_documents(&id) {
                    "Opened in Files. Return here after copying files.".into()
                } else {
                    "Open the system Files app and choose Horizon Android.".into()
                };
            }
            "game_files" => {
                self.catalog.status = if crate::platform::open_documents(
                    crate::document_store::ROOT,
                ) {
                    "Game files opened. Worlds, mods, packs, shaders and reports are available here.".into()
                } else {
                    "Open system Files > Horizon Android.".into()
                };
            }
            "screenshots" => return Ok(self.screenshots("newest")),
            "screenshot_delete" => {
                let result = crate::screenshots::delete(&home, arg)?;
                self.catalog.status = if result.deleted {
                    format!("Deleted {}.", result.name)
                } else if result.missing {
                    format!(
                        "{} was already gone; the gallery was refreshed.",
                        result.name
                    )
                } else {
                    format!("Could not delete {}.", result.name)
                };
                return Ok(json!({
                    "name": result.name,
                    "deleted": result.deleted,
                    "missing": result.missing,
                }));
            }
            "world_files" => {
                let id = crate::document_store::folder_id(self.catalog.config.profile, "worlds");
                self.catalog.status = if crate::platform::open_documents(&id) {
                    "Copy a complete world folder to back up or import. Finish copying before Play; never replace an existing world silently.".into()
                } else {
                    "Open system Files > Horizon Android > Worlds. Copy complete world folders while the game is closed.".into()
                };
            }
            "select_account" => {
                let mut accounts = crate::account::Accounts::load();
                let Some(account) = accounts
                    .accounts
                    .iter()
                    .find(|account| account.key() == arg)
                    .cloned()
                else {
                    return Err("That saved account is no longer available.".into());
                };
                accounts.selected = Some(account.key());
                accounts.save()?;
                self.login.refresh_status();
                self.wardrobe.open(
                    &self.catalog.config.name,
                    &self.catalog.config.microsoft_client_id,
                );
                self.catalog.status = format!("Playing as {}.", account.name);
            }
            "select_offline" => {
                let mut accounts = crate::account::Accounts::load();
                accounts.selected = None;
                accounts.save()?;
                self.login.refresh_status();
                self.wardrobe.open(
                    &self.catalog.config.name,
                    &self.catalog.config.microsoft_client_id,
                );
                self.catalog.status = if self.catalog.config.name.trim().is_empty() {
                    "Using the game's offline name.".into()
                } else {
                    format!("Playing offline as {}.", self.catalog.config.name)
                };
            }
            "wardrobe_kind" => self.wardrobe.capes = arg == "cape",
            "wardrobe_select" => self
                .wardrobe
                .select(arg.parse::<usize>().map_err(|e| e.to_string())?),
            "wardrobe_model" => self.wardrobe.model(),
            "wardrobe_hide_cape" => self.wardrobe.hide_cape(),
            "wardrobe_use_cloud" => self.wardrobe.use_cloud(),
            "wardrobe_cloud" => self
                .wardrobe
                .cloud(&self.catalog.config.microsoft_client_id, None),
            "wardrobe_upload" => self.wardrobe.cloud(
                &self.catalog.config.microsoft_client_id,
                Some(CloudAction::Upload),
            ),
            "wardrobe_cape" => {
                let index = arg.parse::<usize>().map_err(|e| e.to_string())?;
                self.wardrobe.cloud(
                    &self.catalog.config.microsoft_client_id,
                    Some(CloudAction::Cape(index)),
                )
            }
            "wardrobe_rotate" => self
                .wardrobe
                .rotate(arg.parse::<i8>().map_err(|e| e.to_string())?),
            "wardrobe_zoom" => self.wardrobe.zoom(arg == "in"),
            "import_conflict" => crate::documents::resolve(match arg {
                "keep_both" => crate::documents::Choice::KeepBoth,
                "replace" => crate::documents::Choice::Replace,
                _ => crate::documents::Choice::Cancel,
            }),
            "import_cancel" => crate::documents::cancel(),
            "choose" => {
                let (id, value) = arg.split_once('=').ok_or("Expected id=value")?;
                self.choose(id, value)?;
            }
            "toggle" => self.toggle(arg)?,
            "text" => {
                let (id, value) = arg.split_once('=').ok_or("Expected id=value")?;
                let value = value.trim().to_owned();
                match id {
                    "name" => self.catalog.config.name = value.chars().take(16).collect(),
                    "server" => self.catalog.config.server = value,
                    "render_scale" => {
                        let percent = value
                            .trim_end_matches('%')
                            .parse::<u32>()
                            .map_err(|_| "Enter a percentage from 25 to 100")?;
                        self.catalog.config.render_scale =
                            crate::display::RenderScale::new(percent);
                    }
                    "client_id" => self.catalog.config.microsoft_client_id = value,
                    _ => return Err("Unknown field".into()),
                }
                self.save()?;
            }
            "sign_in" => self.login.start(&self.catalog.config.microsoft_client_id),
            "device_sign_in" => self
                .login
                .start_device(&self.catalog.config.microsoft_client_id),
            "ely_sign_in" => {
                let details: Value =
                    serde_json::from_str(arg).map_err(|_| "Invalid Ely.by sign-in details")?;
                self.login.username = details["username"].as_str().unwrap_or_default().to_owned();
                self.login.password = details["password"].as_str().unwrap_or_default().to_owned();
                self.login.otp = details["otp"].as_str().unwrap_or_default().to_owned();
                self.login.start_ely();
            }
            "copy_code" => self.login.copy_code(),
            "open_link" => self.login.open_link(),
            "cancel_login" => self.login.close(),
            "sign_out" => {
                self.login.sign_out();
                self.wardrobe.open(
                    &self.catalog.config.name,
                    &self.catalog.config.microsoft_client_id,
                );
            }
            "review_app" => self.login.review_app(),
            "repository" => {
                self.catalog.status = if crate::platform::open_repository() {
                    "Repository opened in browser."
                } else {
                    "No browser available to open the repository."
                }
                .into();
            }
            "logs" => {
                self.catalog.status = crate::diagnostics::summary(&home);
                if let Ok(error) = std::fs::read_to_string(home.join("android-error.txt")) {
                    self.catalog.status = format!(
                        "Last handled error: {}. {}",
                        crate::diagnostics::redact(&error),
                        self.catalog.status
                    );
                }
            }
            "export_diagnostics" if self.diagnostic_export.is_none() => {
                let (sender, receiver) = mpsc::channel();
                self.diagnostic_export = Some(receiver);
                self.catalog.status =
                    "Preparing local diagnostic ZIP. No upload; review before sharing.".into();
                std::thread::spawn(move || {
                    let result = crate::diagnostics::export(&home).map(|p| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    });
                    let _ = sender.send(result);
                });
            }
            "export_diagnostics" => {}
            "export_performance" => {
                self.catalog.status = match crate::performance::export(&home) {
                    Ok(()) => "Reports opened in Files. Copy or share a report from there.".into(),
                    Err(e) => e,
                };
            }
            "play" => return self.play(None),
            "worlds" => return Ok(self.worlds()),
            "world_launch" => {
                let world = self.world(arg)?;
                if world_in_use(&world) {
                    return Err("That world is open in the game. Close it first.".into());
                }
                return self.play(Some(&world.folder));
            }
            "world_rename" => {
                let request: Value = serde_json::from_str(arg)
                    .map_err(|_| "Expected a world folder and name".to_owned())?;
                let folder = request["folder"].as_str().ok_or("Missing world folder")?;
                let name = request["name"].as_str().ok_or("Missing world name")?;
                let world = self.world(folder)?;
                if world_in_use(&world) {
                    return Err("Close the world in the game before renaming it".into());
                }
                crate::worlds::rename(&world, name)?;
            }
            "world_duplicate" => {
                let world = self.world(arg)?;
                if world_in_use(&world) {
                    return Err("Close the world in the game before duplicating it".into());
                }
                if world.mode == "Unknown" {
                    return Err(
                        "This world has no readable level.dat, so it cannot be duplicated".into(),
                    );
                }
                let copy = crate::worlds::copy_lenient(&world)?;
                return Ok(json!({"folder": copy.folder, "name": copy.name}));
            }
            "world_delete" => {
                let world = self.world(arg)?;
                if world_in_use(&world) {
                    return Err("Close the world in the game before deleting it".into());
                }
                crate::worlds::delete(&world)?;
            }
            "options" => return Ok(self.options(arg)),
            "profiles" => return Ok(self.profiles()),
            "profile_create" => {
                let name = instances::validated_name(&self.named_profiles(), None, arg)?;
                let id = instances::next_id(&self.profile_ids()).ok_or("Too many instances")?;
                let dir = profile_dir(&home, id);
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                instances::write_name(&dir, &name)?;
                self.choose("profile", &id.to_string())?;
            }
            "profile_rename" => {
                let (id, name) = arg.split_once('=').ok_or("Expected id=name")?;
                let id = id.parse::<u32>().map_err(|e| e.to_string())?;
                if !self.profile_ids().contains(&id) {
                    return Err("That instance no longer exists".into());
                }
                let name = instances::validated_name(&self.named_profiles(), Some(id), name)?;
                instances::write_name(&profile_dir(&home, id), &name)?;
            }
            "profile_duplicate" => {
                let id = arg.parse::<u32>().map_err(|e| e.to_string())?;
                let source = profile_dir(&home, id);
                if !self.profile_ids().contains(&id) || !source.is_dir() {
                    return Err("That instance's folder is missing; it cannot be duplicated".into());
                }
                let new_id = instances::next_id(&self.profile_ids()).ok_or("Too many instances")?;
                let destination = profile_dir(&home, new_id);
                instances::copy_profile(&source, &destination)?;
                let name = instances::unique_name(
                    &self.named_profiles(),
                    &format!("{} copy", instances::display_name(&source, id)),
                );
                if let Err(error) = instances::write_name(&destination, &name) {
                    let _ = std::fs::remove_dir_all(&destination);
                    return Err(error);
                }
                return Ok(json!({"id": new_id, "name": name}));
            }
            "profile_delete" => {
                let id = arg.parse::<u32>().map_err(|e| e.to_string())?;
                if id == 0 {
                    return Err("The default instance cannot be deleted".into());
                }
                if id == self.catalog.config.profile {
                    return Err("Switch to another instance before deleting this one".into());
                }
                if !self.profile_ids().contains(&id) {
                    return Err("That instance no longer exists".into());
                }
                std::fs::remove_dir_all(profile_dir(&home, id)).map_err(|e| e.to_string())?;
            }
            "profile_launch" => {
                let id = arg.parse::<u32>().map_err(|e| e.to_string())?;
                if !self.profile_ids().contains(&id) {
                    return Err("That instance no longer exists".into());
                }
                self.choose("profile", &id.to_string())?;
                return self.play(None);
            }
            "performance_state" => {
                let instance = self.catalog.config.game_home(&home);
                let mut state = serde_json::to_value(crate::performance_presets::state(&instance)?)
                    .map_err(|e| e.to_string())?;
                state["instance"] = json!(instances::display_name(
                    &instance,
                    self.catalog.config.profile
                ));
                state["instance_id"] = json!(self.catalog.config.profile);
                return Ok(state);
            }
            "performance_preview" => {
                let instance = self.catalog.config.game_home(&home);
                return serde_json::to_value(crate::performance_presets::preview(&instance, arg)?)
                    .map_err(|e| e.to_string());
            }
            "performance_apply" => {
                let instance = self.catalog.config.game_home(&home);
                let result = crate::performance_presets::apply(&instance, arg)?;
                self.catalog.status = format!(
                    "Applied {}. {} changed{}",
                    result.name,
                    result.changes,
                    if result.restart {
                        "; restart the game to apply worker-thread changes"
                    } else {
                        ""
                    }
                );
                return serde_json::to_value(result).map_err(|e| e.to_string());
            }
            "performance_reset" => {
                let instance = self.catalog.config.game_home(&home);
                let result = crate::performance_presets::reset_defaults(&instance)?;
                self.catalog.status = format!(
                    "Restored engine defaults for the supported performance settings ({} changed){}",
                    result.changes,
                    if result.restart {
                        ". Restart the game to apply worker-thread changes."
                    } else {
                        "."
                    }
                );
                return serde_json::to_value(result).map_err(|e| e.to_string());
            }
            "performance_create" | "performance_update" => {
                let request: Value = serde_json::from_str(arg)
                    .map_err(|_| "Expected a performance preset name and values".to_owned())?;
                let name = request["name"]
                    .as_str()
                    .ok_or("Missing performance preset name")?;
                let values: std::collections::BTreeMap<String, crate::performance_presets::Value> =
                    serde_json::from_value(request["values"].clone())
                        .map_err(|_| "Invalid performance preset values".to_owned())?;
                let instance = self.catalog.config.game_home(&home);
                let preset = if method == "performance_create" {
                    crate::performance_presets::create_custom(&instance, name, &values)?
                } else {
                    let id = request["id"]
                        .as_str()
                        .ok_or("Missing performance preset id")?;
                    crate::performance_presets::update_custom(&instance, id, name, &values)?
                };
                self.catalog.status = format!("Saved performance preset {}.", preset.name);
                return serde_json::to_value(preset).map_err(|e| e.to_string());
            }
            "performance_rename" => {
                let request: Value = serde_json::from_str(arg)
                    .map_err(|_| "Expected a performance preset id and name".to_owned())?;
                let id = request["id"]
                    .as_str()
                    .ok_or("Missing performance preset id")?;
                let name = request["name"]
                    .as_str()
                    .ok_or("Missing performance preset name")?;
                let instance = self.catalog.config.game_home(&home);
                let preset = crate::performance_presets::rename_custom(&instance, id, name)?;
                self.catalog.status = format!("Renamed performance preset to {}.", preset.name);
                return serde_json::to_value(preset).map_err(|e| e.to_string());
            }
            "performance_duplicate" => {
                let instance = self.catalog.config.game_home(&home);
                let preset = crate::performance_presets::duplicate_custom(&instance, arg)?;
                self.catalog.status = format!("Duplicated performance preset as {}.", preset.name);
                return serde_json::to_value(preset).map_err(|e| e.to_string());
            }
            "performance_delete" => {
                let instance = self.catalog.config.game_home(&home);
                crate::performance_presets::delete_custom(&instance, arg)?;
                self.catalog.status = "Performance preset deleted.".into();
            }
            "touch_layouts" => return self.touch_state(),
            "touch_layout_create" => {
                let instance = self.catalog.config.game_home(&home);
                let snapshot = touch_snapshot(&touch_settings(&instance));
                let profile = crate::touch_layouts::create(&instance, arg, &snapshot)?;
                self.catalog.status = format!(
                    "Saved layout profile {} from the current layout.",
                    profile.name
                );
                return serde_json::to_value(profile).map_err(|e| e.to_string());
            }
            "touch_layout_apply" => {
                let instance = self.catalog.config.game_home(&home);
                let snapshot = crate::touch_layouts::snapshot(&instance, arg)?;
                let fields = crate::touch_layouts::apply_fields(
                    &serde_json::to_value(touch_settings(&instance)).map_err(|e| e.to_string())?,
                    &snapshot,
                )?;
                touch_write_fields(&instance, &fields)?;
                crate::touch_layouts::set_selected(&instance, arg)?;
                let name = crate::touch_layouts::selected_name(&instance)
                    .unwrap_or_else(|| "layout profile".into());
                self.catalog.status = format!(
                    "Applied layout profile {name}. Open the game's Options › Controls to move or resize its controls."
                );
                return self.touch_state();
            }
            "touch_layout_save" => {
                let instance = self.catalog.config.game_home(&home);
                let snapshot = touch_snapshot(&touch_settings(&instance));
                let profile = crate::touch_layouts::save_current(&instance, arg, &snapshot)?;
                self.catalog.status = format!(
                    "Updated {} with the layout currently in touch settings.",
                    profile.name
                );
                return serde_json::to_value(profile).map_err(|e| e.to_string());
            }
            "touch_layout_rename" => {
                let request: Value = serde_json::from_str(arg)
                    .map_err(|_| "Expected a layout profile id and name".to_owned())?;
                let id = request["id"].as_str().ok_or("Missing layout profile id")?;
                let name = request["name"]
                    .as_str()
                    .ok_or("Missing layout profile name")?;
                let instance = self.catalog.config.game_home(&home);
                let profile = crate::touch_layouts::rename(&instance, id, name)?;
                self.catalog.status = format!("Renamed layout profile to {}.", profile.name);
                return serde_json::to_value(profile).map_err(|e| e.to_string());
            }
            "touch_layout_duplicate" => {
                let instance = self.catalog.config.game_home(&home);
                let profile = crate::touch_layouts::duplicate(&instance, arg)?;
                self.catalog.status = format!("Duplicated layout profile as {}.", profile.name);
                return serde_json::to_value(profile).map_err(|e| e.to_string());
            }
            "touch_layout_delete" => {
                let instance = self.catalog.config.game_home(&home);
                crate::touch_layouts::delete(&instance, arg)?;
                self.catalog.status =
                    "Layout profile deleted. The touch layout itself stays as it is.".into();
            }
            "touch_layout_reset" => {
                let instance = self.catalog.config.game_home(&home);
                let current =
                    serde_json::to_value(touch_settings(&instance)).map_err(|e| e.to_string())?;
                let defaults = serde_json::to_value(horizon_client::touch::Settings::default())
                    .map_err(|e| e.to_string())?;
                let fields = crate::touch_layouts::reset_fields(&current, &defaults)?;
                touch_write_fields(&instance, &fields)?;
                crate::touch_layouts::clear_selected(&instance)?;
                self.catalog.status =
                    "Default touch layout restored. Custom key actions were kept.".into();
            }
            "appearance_state" => return Ok(self.appearance.state(&home)),
            "appearance_theme" => {
                let theme = crate::appearance::Theme::parse(arg).ok_or("Unknown launcher theme")?;
                self.appearance.set_theme(&home, theme)?;
                self.catalog.status = format!("Launcher theme set to {}.", theme.label());
                return Ok(self.appearance.state(&home));
            }
            "appearance_accent" => {
                let accent =
                    crate::appearance::Accent::parse(arg).ok_or("Unknown launcher accent color")?;
                self.appearance.set_accent(&home, accent)?;
                self.catalog.status = "Launcher accent color updated.".into();
                return Ok(self.appearance.state(&home));
            }
            "appearance_background" => {
                self.appearance.set_background(&home)?;
                self.catalog.status = "Launcher background updated.".into();
                return Ok(self.appearance.state(&home));
            }
            "appearance_background_reset" => {
                self.appearance.reset_background(&home)?;
                self.catalog.status = "Launcher background restored to the default.".into();
                return Ok(self.appearance.state(&home));
            }
            "appearance_reset" => {
                self.appearance.reset(&home)?;
                self.catalog.status = "Launcher appearance reset to the defaults.".into();
                return Ok(self.appearance.state(&home));
            }
            "servers_state" => {
                let game = self.catalog.config.game_home(&home);
                let manager = crate::servers::Manager::load(&game);
                let instance = instances::display_name(&game, self.catalog.config.profile);
                return Ok(manager.state(&instance));
            }
            "server_add" | "server_update" => {
                let request: Value = serde_json::from_str(arg)
                    .map_err(|_| "Expected a server name, address and group".to_owned())?;
                let name = request["name"].as_str().unwrap_or_default();
                let address = request["address"]
                    .as_str()
                    .ok_or("Missing server address")?;
                let group = request["group"].as_str().unwrap_or_default();
                let favorite = request["favorite"].as_bool().unwrap_or(false);
                let mut manager = self.server_manager(&home);
                if method == "server_add" {
                    manager.add(name, address, group, favorite)?;
                    self.catalog.status = "Server added.".into();
                } else {
                    let index = request["index"].as_u64().ok_or("Missing server index")? as usize;
                    manager.update(index, name, address, group, favorite)?;
                    self.catalog.status = "Server updated.".into();
                }
            }
            "server_remove" => {
                let index = arg
                    .parse::<usize>()
                    .map_err(|_| "Missing server index".to_owned())?;
                let server = self.server_manager(&home).remove(index)?;
                self.catalog.status = format!("Removed {}.", server.name);
            }
            "server_duplicate" => {
                let index = arg
                    .parse::<usize>()
                    .map_err(|_| "Missing server index".to_owned())?;
                let copy = self.server_manager(&home).duplicate(index)?;
                self.catalog.status = format!("Duplicated server as {}.", copy.name);
            }
            "server_move" => {
                let request: Value = serde_json::from_str(arg)
                    .map_err(|_| "Expected a server index and direction".to_owned())?;
                let index = request["index"].as_u64().ok_or("Missing server index")? as usize;
                let delta = request["delta"]
                    .as_i64()
                    .ok_or("Missing server direction")?;
                self.server_manager(&home).move_by(index, delta)?;
            }
            "server_favorite" => {
                let index = arg
                    .parse::<usize>()
                    .map_err(|_| "Missing server index".to_owned())?;
                let favorite = self.server_manager(&home).toggle_favorite(index)?;
                return Ok(json!({"favorite": favorite}));
            }
            "server_launch" => {
                let index = arg
                    .parse::<usize>()
                    .map_err(|_| "Missing server index".to_owned())?;
                let address = self.server_manager(&home).address(index)?;
                crate::servers::validate_address(&address)?;
                // The existing multiplayer path reads this setting and passes `--server`.
                self.catalog.config.server = address;
                self.save()?;
                return self.play(None);
            }
            "server_ping" => {
                let manager = self.server_manager(&home);
                let addresses = if arg == "all" {
                    manager.addresses()
                } else {
                    let index = arg
                        .parse::<usize>()
                        .map_err(|_| "Missing server index".to_owned())?;
                    vec![manager.address(index)?]
                };
                let started = addresses
                    .into_iter()
                    .filter(|address| self.start_server_ping(address.clone()))
                    .count();
                return Ok(json!({"started": started}));
            }
            "server_status" => return Ok(self.server_ping_status()),
            _ => return Err(format!("Unknown launcher action {method}")),
        }
        Ok(Value::Null)
    }

    fn choose(&mut self, id: &str, value: &str) -> Result<(), String> {
        let index = |len: usize| -> Result<usize, String> {
            value
                .parse::<usize>()
                .ok()
                .filter(|i| *i < len)
                .ok_or_else(|| "Unknown choice".to_owned())
        };
        let c = &mut self.catalog.config;
        match id {
            "renderer" => {
                c.renderer = match value {
                    "opengl" => crate::catalog::RendererMode::OpenGl,
                    "vulkan_compat" => crate::catalog::RendererMode::VulkanCompatibility,
                    _ => crate::catalog::RendererMode::Vulkan,
                }
            }
            "render_scale" => {
                c.render_scale = crate::display::RenderScale::new(
                    value.parse::<u32>().map_err(|e| e.to_string())?,
                )
            }
            "display_hz" => {
                c.device.display_hz = value.parse::<u16>().map_err(|e| e.to_string())?;
                c.device.sanitize();
            }
            "worker_limit" => {
                c.worker_limit = value.parse::<u32>().map_err(|e| e.to_string())?.min(64);
            }
            "cache_budget_mib" => {
                c.cache_budget_mib = value.parse::<u32>().map_err(|e| e.to_string())?.min(1024);
            }
            "orientation" => {
                c.orientation =
                    crate::catalog::Orientation::ALL[index(crate::catalog::Orientation::ALL.len())?]
            }
            "aspect" => c.screen.aspect = ASPECTS[index(ASPECTS.len())?],
            "input_host" => {
                c.input_host =
                    crate::catalog::InputHost::ALL[index(crate::catalog::InputHost::ALL.len())?]
            }
            "profile" => {
                let profile = value.parse::<u32>().map_err(|e| e.to_string())?;
                std::fs::create_dir_all(if profile == 0 {
                    self.home.clone()
                } else {
                    self.home
                        .join("profiles")
                        .join(format!("profile-{profile}"))
                })
                .map_err(|e| e.to_string())?;
                c.profile = profile;
                self.save()?;
                self.catalog.refresh(&self.home);
                return Ok(());
            }
            _ => return Err("Unknown setting".into()),
        }
        self.save()
    }

    fn toggle(&mut self, id: &str) -> Result<(), String> {
        let c = &mut self.catalog.config;
        match id {
            "immersive" => c.screen.immersive ^= true,
            "cutout" => c.screen.avoid_cutout ^= true,
            "performance_hints" => {
                c.device.performance_hints ^= true;
                self.catalog.status = "ADPF gives Android CPU frame-work hints on supported devices. It does not cap FPS or guarantee a speedup.".into();
            }
            "gpu_profile" => {
                c.device.gpu_profile ^= true;
                self.catalog.status = "Detailed GPU profiling records diagnostics and has overhead. Reports remain on this device.".into();
            }
            "record_performance" => {
                c.record_performance ^= true;
                self.catalog.status = if c.record_performance {
                    "Recording starts on Play. Detailed timings add overhead. Return here to save the report."
                } else {
                    "Performance recording off."
                }
                .into();
            }
            _ => return Err("Unknown setting".into()),
        }
        self.save()
    }

    /// Adds a picked PNG to the local wardrobe gallery.
    fn import_wardrobe(&mut self, kind: &str, title: &str, bytes: &[u8]) -> Result<Value, String> {
        if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
            return Err("Choose a PNG smaller than 8 MiB.".into());
        }
        let kind = match kind {
            "cape" => crate::skins::library::Kind::Cape,
            "skin" => crate::skins::library::Kind::Skin,
            _ => return Err("Unknown wardrobe image kind.".into()),
        };
        let title = title.trim();
        let title = if title.is_empty() {
            "Imported image"
        } else {
            title
        };
        self.wardrobe.import_bytes(title, kind, bytes)?;
        self.catalog.status =
            format!("{title} added to the local gallery. Select it to apply it locally.");
        Ok(json!({"imported": true}))
    }

    /// Imports a folder or ZIP picked in the Compose launcher.
    fn start_import(
        &mut self,
        kind: crate::minecraft_import::Kind,
        uri: String,
    ) -> Result<Value, String> {
        if self.worker.is_some() || crate::minecraft_import::busy() {
            return Err("Finish the current installation first".into());
        }
        crate::minecraft_import::start(self.home.clone(), kind, uri)?;
        self.error = None;
        self.catalog.status = "Reading the selected Minecraft installation...".into();
        Ok(json!({"started": true}))
    }

    /// Every instance's screenshots, in the order the gallery requested.
    fn screenshots(&self, order: &str) -> Value {
        screenshot_list(&self.home, order)
    }

    /// The launcher's layout profiles and the instance's current touch layout.
    fn touch_state(&self) -> Result<Value, String> {
        let instance = self.catalog.config.game_home(&self.home);
        let current = touch_snapshot(&touch_settings(&instance));
        let mut state = serde_json::to_value(crate::touch_layouts::state(&instance)?)
            .map_err(|e| e.to_string())?;
        state["instance_id"] = json!(self.catalog.config.profile);
        state["current_snapshot"] = current.clone();
        state["mode"] = current["mode"].clone();
        state["mode_label"] =
            crate::touch_layouts::mode_label(current["mode"].as_str().unwrap_or_default());
        Ok(state)
    }

    /// Starts the install, or asks the Activity to start the game process.
    /// The active instance's worlds, with the metadata the World Manager shows.
    /// The list is lenient: a folder with no readable level.dat stays visible
    /// (so it can be deleted) and reports `metadata: false`.
    fn worlds(&self) -> Value {
        let game = self.catalog.config.game_home(&self.home);
        let instance = instances::display_name(&game, self.catalog.config.profile);
        let worlds: Vec<Value> = crate::worlds::list_lenient(&game)
            .into_iter()
            .map(|world| {
                json!({
                    "folder": world.folder,
                    "name": world.name,
                    "mode": world.mode,
                    "version": world.version,
                    "last_played": world
                        .last_played
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|since| since.as_secs()),
                    "icon": world.icon.as_ref().map(|path| path.to_string_lossy().into_owned()),
                    "in_use": world_in_use(&world),
                    "metadata": world.mode != "Unknown",
                })
            })
            .collect();
        json!({
            "instance": instance,
            "instance_id": self.catalog.config.profile,
            "worlds": worlds,
        })
    }

    fn world(&self, query: &str) -> Result<crate::worlds::World, String> {
        let game = self.catalog.config.game_home(&self.home);
        crate::worlds::find_lenient(&game, query)
            .map_err(|_| "That world no longer exists".to_owned())
    }

    fn server_manager(&self, home: &std::path::Path) -> crate::servers::Manager {
        crate::servers::Manager::load(&self.catalog.config.game_home(home))
    }

    /// Starts one manual status request. A request already waiting for a reply is not repeated.
    fn start_server_ping(&self, address: String) -> bool {
        {
            let mut pings = self
                .server_pings
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if matches!(pings.get(&address), Some(crate::servers::Ping::Pending)) {
                return false;
            }
            pings.insert(address.clone(), crate::servers::Ping::Pending);
        }
        let pings = Arc::clone(&self.server_pings);
        std::thread::spawn(move || {
            let result = horizon_client::net::ping(&address, Duration::from_secs(4));
            let state = crate::servers::Ping::from_result(result);
            pings
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(address, state);
        });
        true
    }

    fn server_ping_status(&self) -> Value {
        let pings = self
            .server_pings
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let statuses: serde_json::Map<String, Value> = pings
            .iter()
            .map(|(address, ping)| (address.clone(), ping.to_json()))
            .collect();
        json!({"statuses": statuses})
    }

    fn play(&mut self, world: Option<&str>) -> Result<Value, String> {
        if self.worker.is_some() || crate::minecraft_import::busy() || self.login.pending() {
            return Ok(json!({"launch": false}));
        }
        if horizon_client::official_install::ready(&self.home) {
            self.save()?;
            let _ = instances::mark_played(&self.catalog.config.game_home(&self.home));
            self.error = None;
            let _ = std::fs::remove_file(self.home.join("android-error.txt"));
            let request = match world {
                Some(world) => json!({"play": true, "world": world}),
                None => json!({"play": true}),
            };
            std::fs::write(
                self.home.join(LAUNCH_REQUEST),
                serde_json::to_vec(&request).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            crate::diagnostics::heartbeat(
                "play_requested",
                || json!({"renderer": self.catalog.config.renderer.label(), "launcher": "compose"}),
            );
            return Ok(json!({"launch": true}));
        }
        if !self.home.join("engine-data.zip").is_file() {
            return Err("This APK is missing its engine data. Install a complete build.".into());
        }
        self.error = None;
        *self.install.lock().unwrap_or_else(|e| e.into_inner()) = Default::default();
        let home = self.home.clone();
        let install = self.install.clone();
        let (sender, receiver) = mpsc::channel();
        self.worker = Some(receiver);
        std::thread::spawn(move || {
            let result = horizon_client::official_install::install_with_progress(&home, |update| {
                let mut current = install.lock().unwrap_or_else(|e| e.into_inner());
                let done = if current.stage == update.stage {
                    current.done.max(update.done)
                } else {
                    update.done
                };
                *current = horizon_client::official_install::InstallProgress { done, ..update };
            });
            let _ = sender.send(result.map_err(|e| e.to_string()));
        });
        Ok(json!({"launch": false}))
    }
}

const ASPECTS: [crate::display::Aspect; 4] = [
    crate::display::Aspect::Native,
    crate::display::Aspect::Wide,
    crate::display::Aspect::Classic,
    crate::display::Aspect::Square,
];

fn screenshot_list(home: &std::path::Path, order: &str) -> Value {
    let order = match crate::screenshots::Order::parse(order) {
        Ok(order) => order,
        Err(error) => return json!({"items": [], "error": error}),
    };
    let items: Vec<Value> = crate::screenshots::list(home, order)
        .into_iter()
        .map(|item| {
            json!({
                "id": item.id,
                "name": item.name,
                "profile": item.profile,
                "instance": item.instance,
                "size": item.size,
                "modified": item.modified,
                "width": item.width,
                "height": item.height,
                "mime": item.mime,
            })
        })
        .collect();
    json!({"items": items})
}

/// The engine's touch settings in an instance, with all validations applied.
fn touch_settings(instance: &std::path::Path) -> horizon_client::touch::Settings {
    horizon_client::touch::Settings::load_from(&instance.join(crate::touch_layouts::TOUCH_FILE))
}

/// The layout-only snapshot a touch layout profile stores: the control mode,
/// the three placements rows, button visibility and the custom buttons.
fn touch_snapshot(settings: &horizon_client::touch::Settings) -> Value {
    let value = serde_json::to_value(settings).unwrap_or_default();
    json!({
        "mode": value["mode"],
        "layouts": value["layouts"],
        "button_visibility": value["button_visibility"],
        "custom_buttons": value["custom_buttons"],
    })
}

/// Copies one profile's fields into the engine's touch settings. Everything the
/// profiles are not about (sensitivity, handedness, the dynamic joystick, the
/// gyroscope, ...) stays exactly as it is, and the engine validates the result.
fn touch_write_fields(instance: &std::path::Path, fields: &Value) -> Result<(), String> {
    let path = instance.join(crate::touch_layouts::TOUCH_FILE);
    let mut settings = touch_settings(instance);
    let mut value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "The touch settings are not an object".to_owned())?;
    for key in ["mode", "layouts", "button_visibility", "custom_buttons"] {
        object.insert(key.to_owned(), fields.get(key).cloned().unwrap_or_default());
    }
    let merged: horizon_client::touch::Settings =
        serde_json::from_value(value).map_err(|error| {
            format!("This layout snapshot is not compatible with this build: {error}")
        })?;
    merged.sanitize();
    merged
        .save_to(&path)
        .map_err(|error| format!("Could not save the touch layout: {error}"))
}

fn string(env: &mut JNIEnv<'_>, value: &JString<'_>) -> String {
    if value.is_null() {
        return String::new();
    }
    env.get_string(value).map(String::from).unwrap_or_else(|_| {
        let _ = env.exception_clear();
        String::new()
    })
}
fn reply(env: &mut JNIEnv<'_>, value: &Value) -> jstring {
    env.new_string(value.to_string())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `NativeBridge.start(Activity, String home)`: once per Activity instance.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_NativeBridge_start(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    activity: JObject<'_>,
    home: JString<'_>,
) {
    let home = PathBuf::from(string(&mut env, &home));
    let (Ok(vm), Ok(global)) = (env.get_java_vm(), env.new_global_ref(&activity)) else {
        let _ = env.exception_clear();
        return;
    };
    crate::platform::set_bridge_context(vm, global);
    if CONTROLLER.with(|c| c.borrow().is_some()) {
        return;
    }
    crate::init_logging();
    crate::backend::initialize(home.clone());
    if let Err(error) = crate::diagnostics::initialize(&home, json!({"launcher": "compose"})) {
        log::error!("Cannot initialize local diagnostics: {error}");
    }
    if let Ok(vm) = env.get_java_vm() {
        crate::secure_store::initialize(vm);
    }
    if let Some(cache) = crate::platform::cache_dir() {
        horizon_client::android_runtime::configure_cache_dir(cache.clone());
        horizon_client::official_install::configure_download_cache_dir(cache);
    }
    if let Err(error) =
        std::fs::create_dir_all(&home).and_then(|()| std::env::set_current_dir(&home))
    {
        log::error!("Cannot open {}: {error}", home.display());
    }
    horizon_client::crash::install(home.clone(), "launcher");
    let controller = Controller::new(home);
    CONTROLLER.with(|c| *c.borrow_mut() = Some(controller));
}

/// `NativeBridge.snapshot()`: the whole launcher state as JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_NativeBridge_snapshot(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let value = CONTROLLER
        .with(|c| c.borrow_mut().as_mut().map(Controller::snapshot))
        .unwrap_or(Value::Null);
    reply(&mut env, &value)
}

/// `NativeBridge.call(String method, String arg)`: `{"ok":…}` or `{"error":…}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_NativeBridge_call(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    method: JString<'_>,
    arg: JString<'_>,
) -> jstring {
    let method = string(&mut env, &method);
    let arg = string(&mut env, &arg);
    let result = CONTROLLER.with(|c| match c.borrow_mut().as_mut() {
        Some(controller) => controller.call(&method, &arg),
        None => Err("Launcher is not initialized".into()),
    });
    let value = match result {
        Ok(value) => json!({"ok": value}),
        Err(error) => {
            CONTROLLER.with(|c| {
                if let Some(controller) = c.borrow_mut().as_mut() {
                    controller.catalog.status = error.clone();
                }
            });
            json!({"error": error})
        }
    };
    reply(&mut env, &value)
}

/// `NativeBridge.importWardrobe(String kind, String title, byte[] png)`: a picked skin or cape.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_NativeBridge_importWardrobe(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    kind: JString<'_>,
    title: JString<'_>,
    bytes: JByteArray<'_>,
) {
    let kind = string(&mut env, &kind);
    let title = string(&mut env, &title);
    let bytes = if bytes.is_null() {
        Vec::new()
    } else {
        match env.convert_byte_array(&bytes) {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = env.exception_clear();
                Vec::new()
            }
        }
    };
    let result = CONTROLLER.with(|c| match c.borrow_mut().as_mut() {
        Some(controller) => controller.import_wardrobe(&kind, &title, &bytes),
        None => Err("Launcher is not initialized".into()),
    });
    if let Err(error) = result {
        CONTROLLER.with(|c| {
            if let Some(controller) = c.borrow_mut().as_mut() {
                controller.catalog.status = error;
            }
        });
    }
}

/// `NativeBridge.importDocument(Activity, String kind, String uri)`: a picker result.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_NativeBridge_importDocument(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    activity: JObject<'_>,
    kind: JString<'_>,
    uri: JString<'_>,
) {
    let kind = string(&mut env, &kind);
    let uri = (!uri.is_null()).then(|| string(&mut env, &uri));
    let Some(kind) = parse_kind(&kind) else {
        return;
    };
    let Some(home) = CONTROLLER.with(|c| {
        c.borrow()
            .as_ref()
            .map(|c| c.catalog.config.game_home(&c.home))
    }) else {
        return;
    };
    crate::documents::import_external(&mut env, &activity, kind, home, uri);
}

fn import_minecraft(env: &mut JNIEnv<'_>, kind: crate::minecraft_import::Kind, uri: JString<'_>) {
    if uri.is_null() {
        return;
    }
    let uri = string(env, &uri);
    let result = CONTROLLER.with(|c| match c.borrow_mut().as_mut() {
        Some(controller) => controller.start_import(kind, uri),
        None => Err("Launcher is not initialized".into()),
    });
    if let Err(error) = result {
        CONTROLLER.with(|c| {
            if let Some(controller) = c.borrow_mut().as_mut() {
                controller.catalog.status = error;
            }
        });
    }
}

/// `NativeBridge.importMinecraftFolder(String uri)`: a picked SAF tree.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_NativeBridge_importMinecraftFolder(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    uri: JString<'_>,
) {
    import_minecraft(&mut env, crate::minecraft_import::Kind::Folder, uri);
}

/// `NativeBridge.importMinecraftZip(String uri)`: a picked ZIP document.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_NativeBridge_importMinecraftZip(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    uri: JString<'_>,
) {
    import_minecraft(&mut env, crate::minecraft_import::Kind::Zip, uri);
}

/// `NativeBridge.screenshots(String order)`: the gallery list. The Compose
/// screen calls this off the main thread, so it uses the immutable app home
/// instead of the main-thread launcher controller.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_NativeBridge_screenshots(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    order: JString<'_>,
) -> jstring {
    let order = string(&mut env, &order);
    let value = match crate::backend::try_data_dir() {
        Some(home) => screenshot_list(&home, &order),
        None => json!({"items": [], "error": "Launcher is not initialized"}),
    };
    reply(&mut env, &value)
}

/// `ScreenshotProvider.nativeCall(home, "inspect", id)`: metadata for a scoped
/// share URI. Only IDs accepted by the Rust allowlist resolve to a file.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_ScreenshotProvider_nativeCall(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    home: JString<'_>,
    operation: JString<'_>,
    id: JString<'_>,
) -> jstring {
    let home = PathBuf::from(string(&mut env, &home));
    let operation = string(&mut env, &operation);
    let id = string(&mut env, &id);
    if operation != "inspect" {
        let _ = env.throw_new(
            "java/io/FileNotFoundException",
            "Unknown screenshot operation",
        );
        return std::ptr::null_mut();
    }
    match crate::screenshots::inspect(&home, &id) {
        Ok(info) => reply(
            &mut env,
            &json!({
                "_display_name": info.name,
                "_size": info.size,
                "last_modified": info.modified,
                "mime_type": info.mime,
            }),
        ),
        Err(error) => {
            let _ = env.throw_new("java/io/FileNotFoundException", error.to_string());
            std::ptr::null_mut()
        }
    }
}

/// `ScreenshotProvider.nativeOpen(home, id)`: a read descriptor for the share
/// URI. The Java provider adopts it; the native side owns the allowlist.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizon_android_ScreenshotProvider_nativeOpen(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    home: JString<'_>,
    id: JString<'_>,
) -> jint {
    use std::os::fd::IntoRawFd;
    let home = PathBuf::from(string(&mut env, &home));
    let id = string(&mut env, &id);
    match crate::screenshots::open(&home, &id) {
        Ok(file) => file.into_raw_fd(),
        Err(error) => {
            let _ = env.throw_new("java/io/FileNotFoundException", error.to_string());
            -1
        }
    }
}
