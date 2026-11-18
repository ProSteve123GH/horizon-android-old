//! The `:game` process: plays the world the Compose (Material 3) launcher asked for.
//!
//! The Compose launcher writes `launch-request.json` and starts this process to play.
//! Without that file there is nothing to show here, so there is no window and the
//! Activity finishes; the player lands back in the Compose launcher. The classic
//! Rust launcher pages no longer exist.
use horizon_client::Result;
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::WindowId,
};

struct PreparedAccount {
    key: String,
    file: PathBuf,
    appearance: crate::skins::cloud::Cache,
}

pub(super) struct DirectGame {
    home: PathBuf,
    config: crate::catalog::Config,
    wardrobe: crate::wardrobe::Wardrobe,
    game: Option<Box<dyn ApplicationHandler>>,
    /// A world folder in the active profile to open directly.
    world: Option<String>,
    started: bool,
    session_worker: Option<mpsc::Receiver<std::result::Result<PreparedAccount, String>>>,
    session_ready: bool,
    prepared_session: Option<PathBuf>,
}

impl DirectGame {
    pub(super) fn new(home: PathBuf, world: Option<String>) -> Self {
        // Upgrade existing installations that retained the old download cache.
        let cleanup_home = home.clone();
        std::thread::spawn(move || {
            if let Err(error) =
                horizon_client::official_install::cleanup_completed_downloads(&cleanup_home)
            {
                log::warn!("Could not clean completed Minecraft downloads: {error}");
            }
        });
        let mut config = crate::catalog::Config::load(&home);
        horizon_client::android_runtime::graphics_startup::configure(home.clone());
        if horizon_client::android_runtime::graphics_startup::recover(&home) {
            config.renderer = crate::catalog::RendererMode::OpenGl;
            if let Err(error) = config.save(&home) {
                log::warn!("Could not save graphics recovery choice: {error}");
            }
        }
        crate::configure_input_mode(config.input_host);
        crate::device_runtime::configure(config.device);
        Self {
            wardrobe: crate::wardrobe::Wardrobe::new(&home),
            home,
            config,
            game: None,
            world,
            started: false,
            session_worker: None,
            session_ready: false,
            prepared_session: None,
        }
    }

    fn fail(&mut self, error: impl std::fmt::Display) {
        log::error!("{error}");
        crate::diagnostics::record_error(&error.to_string());
        let _ = std::fs::write(self.home.join("android-error.txt"), error.to_string());
    }

    fn configure_screen(&self) {
        crate::native_ime::configure_screen(
            self.config.screen.immersive,
            self.config.screen.avoid_cutout,
        );
        winit::platform::android::set_native_content_aspect(self.config.screen.aspect.ratio());
    }

    fn leave_to_launcher(&mut self, events: &ActiveEventLoop) {
        crate::platform::finish_activity();
        events.exit();
    }

    fn action(&mut self, events: &ActiveEventLoop) -> Result<()> {
        crate::diagnostics::heartbeat(
            "play_requested",
            || serde_json::json!({"renderer":self.config.renderer.label(),"resolution_percent":self.config.render_scale.percent()}),
        );
        horizon_client::android_runtime::configure_worker_limit(self.config.worker_limit);
        horizon_client::android_runtime::configure_cache_budget(self.config.cache_budget_mib);
        crate::configure_input_mode(self.config.input_host);
        crate::device_runtime::configure(self.config.device);
        if !horizon_client::official_install::ready(&self.home) {
            return Err(
                "Minecraft is not installed yet. Download it from the launcher first.".into(),
            );
        }
        horizon_client::android_resources::repair_structure_transforms(
            &self.home.join("engine-data.zip"),
            &self
                .home
                .join(".local/horizon-client")
                .join(horizon_core::VANILLA_VERSION),
        )?;
        crate::platform::hide();
        self.config.save(&self.home)?;
        // Migrate the earlier launcher preset once; subsequent edits belong to Key Binds.
        let keypad_path = self.config.game_home(&self.home).join("horizon-keypad.json");
        if self.config.keypad_controls && !keypad_path.exists() {
            if let Some(dir) = keypad_path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            horizon_client::keypad::Settings {
                enabled: true,
                ..Default::default()
            }
            .save(&keypad_path)?;
        }
        self.config.keypad_controls = false;
        let game_home = self.config.game_home(&self.home);
        if !self.session_ready && crate::account::Accounts::load().selected().is_some() {
            let id = self.config.microsoft_client_id.clone();
            let wardrobe_home = self.home.join("wardrobe");
            let session_dir = self.home.join("sessions");
            let (sender, receiver) = mpsc::channel();
            self.session_worker = Some(receiver);
            std::thread::spawn(move || {
                let result = (|| {
                    let (account, secrets) = crate::login::Login::session(&id)?
                        .ok_or("The selected account changed. Choose Play again.")?;
                    let appearance =
                        crate::skins::cloud::fetch(&account, &secrets, &wardrobe_home)
                            .unwrap_or_default();
                    let file = crate::account::write_session_file(
                        &session_dir,
                        &account,
                        &secrets,
                    )?;
                    Ok(PreparedAccount {
                        key: account.key(),
                        file,
                        appearance,
                    })
                })();
                if let Err(unsent) = sender.send(result) {
                    if let Ok(prepared) = unsent.0 {
                        let _ = std::fs::remove_file(prepared.file);
                    }
                }
            });
            return Ok(());
        }
        self.session_ready = false;
        std::fs::create_dir_all(&game_home)?;
        if crate::account::Accounts::load().selected().is_none() {
            self.wardrobe
                .prepare_game(&self.config.name, &game_home)?;
        }
        let extra_arguments = if crate::PUBLIC_BUILD {
            Vec::new()
        } else {
            match std::fs::read(self.home.join("android-launch.json")) {
                Ok(bytes) => serde_json::from_slice::<Vec<String>>(&bytes)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(e) => return Err(e.into()),
            }
        };

        if self.config.record_performance || self.config.device.gpu_profile {
            crate::performance::start(
                &self.home,
                serde_json::json!({
                    "renderer": self.config.renderer.label(),
                    "resolution_percent": self.config.render_scale.percent(),
                    "source_commit": option_env!("HORIZON_ANDROID_BUILD_COMMIT"),
                    "source_dirty": option_env!("HORIZON_ANDROID_BUILD_DIRTY"),
                    "performance_recording": self.config.record_performance,
                    "gpu_profiling": self.config.device.gpu_profile,
                }),
            )?;
        }
        // The A press that launches the game must not accept its first menu too.
        horizon_client::android_runtime::clear_gamepads();
        // A selected world is single-player: the configured server must not
        // win over `--world` in the game's option parsing.
        let world = self.world.take();
        let server = if world.is_some() {
            String::new()
        } else {
            self.config.server.clone()
        };
        let request = crate::game_launch::LaunchRequest {
            resources: self
                .home
                .join(".local/horizon-client")
                .join(horizon_core::VANILLA_VERSION),
            game_home,
            home_dir: self.home.clone(),
            session_file: self.prepared_session.take(),
            offline_name: crate::catalog::truncate_player_name(&self.config.name),
            server,
            world,
            control_socket: (!crate::PUBLIC_BUILD).then(|| self.home.join("control.json")),
            extra_arguments,
            open_gl: self.config.renderer == crate::catalog::RendererMode::OpenGl,
            gpu_batching: self.config.renderer == crate::catalog::RendererMode::Vulkan,
        };
        crate::document_store::set_game_active(&self.home, true);
        let game = match crate::game_launch::start(request) {
            Ok(game) => game,
            Err(error) => {
                crate::document_store::set_game_active(&self.home, false);
                crate::performance::stop();
                return Err(error);
            }
        };
        horizon_client::ui_theme::reset_atlas();
        self.game = Some(game);
        self.game.as_mut().unwrap().resumed(events);
        Ok(())
    }

    fn poll_session(&mut self, events: &ActiveEventLoop) {
        let Some(result) = self
            .session_worker
            .as_ref()
            .and_then(|w| w.try_recv().ok())
        else {
            return;
        };
        self.session_worker = None;
        match result {
            Ok(prepared) => {
                // Only the current selection may publish the game handoff. A canceled
                // worker downloads public cache images but cannot overwrite a new profile.
                if crate::account::Accounts::load()
                    .selected()
                    .is_none_or(|a| a.key() != prepared.key)
                {
                    let _ = std::fs::remove_file(prepared.file);
                    self.fail("The selected account changed. Choose Play again.");
                    events.exit();
                } else {
                    let root = self.home.join("wardrobe");
                    let cloud = prepared.appearance.skin.as_deref().map(|p| {
                        (
                            p,
                            prepared.appearance.slim,
                            prepared.appearance.cape.as_deref(),
                        )
                    });
                    let appearance =
                        crate::skins::library::Library::load(&root).and_then(|library| {
                            library.prepare_game(
                                &root,
                                &prepared.key,
                                &self
                                    .config
                                    .game_home(&self.home),
                                cloud,
                            )
                        });
                    match appearance {
                        Ok(()) => {
                            self.prepared_session = Some(prepared.file);
                            self.session_ready = true;
                            if let Err(e) = self.action(events) {
                                self.fail(e);
                                events.exit();
                            }
                        }
                        Err(error) => {
                            let _ = std::fs::remove_file(prepared.file);
                            self.fail(error);
                            events.exit();
                        }
                    }
                }
            }
            Err(e) => {
                self.fail(e);
                events.exit();
            }
        }
    }

    fn check_return(&mut self, events: &ActiveEventLoop) {
        if !horizon_client::android_runtime::take_launcher_return() {
            return;
        }
        if let Some(mut game) = self.game.take() {
            game.exiting(events);
            drop(game);
        }
        crate::game::android_reset_bundled_mods();
        crate::document_store::set_game_active(&self.home, false);
        crate::performance::stop();
        crate::diagnostics::heartbeat("returned_to_launcher", || serde_json::json!({}));
        self.leave_to_launcher(events);
    }
}

impl ApplicationHandler for DirectGame {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if let Some(game) = &mut self.game {
            return game.resumed(events);
        }
        self.configure_screen();
        crate::platform::set_orientation(self.config.orientation);
        crate::display::set_scale(self.config.render_scale);
        horizon_client::android_runtime::configure_gpu(false);
        if !self.started {
            self.started = true;
            if let Err(error) = self.action(events) {
                self.fail(error);
                events.exit();
            }
        }
    }

    fn suspended(&mut self, events: &ActiveEventLoop) {
        crate::diagnostics::heartbeat("suspended", || serde_json::json!({}));
        if let Some(game) = &mut self.game {
            return game.suspended(events);
        }
    }

    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if crate::documents::handle_event(&event) {
            return;
        }
        if let Some(game) = &mut self.game {
            game.window_event(events, id, event);
            self.check_return(events);
            return;
        }
        // No launcher window exists in this process; nothing to draw.
        if matches!(event, WindowEvent::CloseRequested) {
            self.leave_to_launcher(events);
        }
    }

    fn device_event(&mut self, events: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        if let Some(game) = &mut self.game {
            game.device_event(events, id, event);
        }
    }

    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        if let Some(game) = &mut self.game {
            game.about_to_wait(events);
            self.check_return(events);
            return;
        }
        self.poll_session(events);
        events.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(100),
        ));
    }

    fn exiting(&mut self, events: &ActiveEventLoop) {
        if let Some(game) = &mut self.game {
            game.exiting(events);
        }
        crate::platform::finish_activity();
    }
}
