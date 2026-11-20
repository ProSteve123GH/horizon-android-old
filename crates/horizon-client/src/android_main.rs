//! Android's native entry point. The simulation and renderer are shared with desktop.
#![cfg(target_os = "android")]

// The shared gameplay source is compiled twice: into `horizon-client` and into this cdylib.
// Its generated host code spells the API types as `horizon_client::mod_api::*`, so inside this
// crate that path must reach *this* copy, not the library's. Everything else the game uses is
// re-exported from the real dependency below; explicitly declared modules (game, mod_api) shadow
// the glob's copies.
extern crate self as horizon_client;
// The real library is reached through a second alias; `::horizon_client` would hit the alias above.
extern crate horizon_client as horizon_client_dep;

// Shared vanilla match structure and desktop harnesses. It intentionally shadows
// the library's `game` in the glob re-export below: this is the Android JNI entry
// crate, so its own copy wins.
#[allow(dead_code, clippy::collapsible_match, hidden_glob_reexports)]
mod game;
#[path = "android/game_launch.rs"]
mod game_launch;
#[path = "android/direct.rs"]
mod direct;
// The shared account and wardrobe state machines the Compose launcher's JNI
// bridge drives (`crates/horizon-client/src/android/login.rs`, `wardrobe.rs`).
#[path = "android/login.rs"]
pub(crate) mod login;
#[path = "android/wardrobe.rs"]
pub(crate) mod wardrobe;
// Share the desktop launcher's actual search, compatibility checks, downloads,
// dependency resolution and Java content porting; GPUI itself is desktop-only.
// These files are formatted in the launcher's separate Rust 2021 workspace.
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/account.rs"]
mod account;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/browser.rs"]
mod browser;
#[path = "android/dns.rs"]
mod android_dns;
#[path = "android/appearance.rs"]
mod appearance;
#[path = "android/bridge.rs"]
mod bridge;
// Android-only JNI glue: unused catalog helpers are kept for the bridge's paths.
#[allow(dead_code)]
#[path = "android/catalog.rs"]
mod catalog;
#[path = "android/device_preferences.rs"]
mod device_preferences;
#[path = "android/device_runtime.rs"]
mod device_runtime;
#[path = "android/diagnostics.rs"]
mod diagnostics;
// Android-only JNI glue: display helpers are driven from the entry boundary.
#[allow(dead_code)]
#[path = "android/display.rs"]
mod display;
#[path = "android/document_bridge.rs"]
mod document_bridge;
#[path = "android/document_provider.rs"]
mod document_provider;
#[path = "android/document_store.rs"]
mod document_store;
// Android-only SAF/JNI glue: several document helpers are reached from other glue.
#[allow(dead_code)]
#[path = "android/documents.rs"]
mod documents;
#[path = "android/exit_reports.rs"]
mod exit_reports;
#[path = "android/gallery.rs"]
mod gallery;
#[path = "android/instances.rs"]
mod instances;
// The desktop launcher's world model: discovery, metadata, copying, renaming
// and deletion. The Android world manager adds only its UI and launch request.
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/worlds.rs"]
mod worlds;
// Named `android_minecraft_import` so the library's `minecraft_import` (re-exported below) is
// what `horizon_client::minecraft_import` reaches from the shared gameplay source.
#[path = "android/minecraft_import.rs"]
mod android_minecraft_import;
// Android-only JNI glue for the system keyboard; some entry points are unused here.
#[allow(dead_code)]
#[path = "android/native_ime.rs"]
mod native_ime;
// Android-only JNI glue for native mods; the cdylib uses a subset of its entry points.
#[allow(dead_code)]
#[path = "android/native_mods.rs"]
mod native_mods;
#[path = "android/performance.rs"]
mod performance;
#[path = "android/performance_presets.rs"]
mod performance_presets;
// Android-only JNI platform bridge; unused helpers stay for the entry boundary.
#[allow(dead_code)]
#[path = "android/platform.rs"]
mod platform;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/proxy.rs"]
mod proxy;
// The desktop launcher's servers.dat model: vanilla NBT, pictures and the fields the launcher
// does not use. The Android Server Manager adds its UI, favorites and groups.
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/server_list.rs"]
mod server_list;
#[path = "android/screenshots.rs"]
mod screenshots;
#[path = "android/search_queue.rs"]
mod search_queue;
#[path = "android/secure_store.rs"]
mod secure_store;
#[path = "android/shared_storage.rs"]
pub(crate) mod shared_storage;
#[path = "android/servers.rs"]
mod servers;
#[path = "android/shader_catalog.rs"]
mod shader_catalog;
// Android-only JNI glue for touch layouts; some constants are not wired up on this target.
#[allow(dead_code)]
#[path = "android/touch_layouts.rs"]
mod touch_layouts;
// Android-only JNI glue; this module also shadows the library's `skins` glob export.
#[rustfmt::skip]
#[allow(dead_code, hidden_glob_reexports)]
#[path = "../../../launcher/src/skins.rs"]
mod skins;
// Upstream split the gameplay library into horizon-client-lib; the shared
// gameplay source compiled here still uses the old crate-root paths.
#[path = "mod_api.rs"]
pub mod mod_api;
pub use horizon_client_dep::*;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/i18n.rs"]
mod i18n;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/native_fit.rs"]
mod native_fit;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/native_status.rs"]
mod native_status;
// TODO(keep-consistency): the shared native-mod browser reads Nightly build records and GitHub
// origins when it says whether a native mod fits. Android builds through its own service and has
// neither, so report an unknown origin and use the launcher's date parsing for labels.
#[allow(dead_code)]
mod nightly {
    /// The date of a nightly's version (`nightly-2026-10-28-0c02067`).
    pub fn date_of(version: &str) -> Option<&str> {
        let date = version.strip_prefix("nightly-")?.get(..10)?;
        let digits = date
            .bytes()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
        (digits && date.as_bytes()[4] == b'-' && date.as_bytes()[7] == b'-').then_some(date)
    }
}

#[allow(dead_code)]
mod github_mods {
    /// Where a native mod installed from a GitHub link came from. Android has no such sidecars.
    #[derive(Clone, Debug)]
    pub struct Origin;

    pub fn origin(_home: &std::path::Path, _id: &str) -> Option<Origin> {
        None
    }
}

// The shared account protocol needs only the launcher's version and data path.
mod backend {
    pub const VERSION: &str = if crate::PUBLIC_BUILD {
        "0.7.7"
    } else {
        "0.7.7-dev"
    };
    static HOME: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    pub fn initialize(home: std::path::PathBuf) {
        let _ = HOME.set(home);
    }
    pub fn game_home() -> std::path::PathBuf {
        data_dir()
    }
    pub fn game_mod_api_level() -> Option<u32> {
        Some(horizon_mods::api_level::LEVEL)
    }
    pub fn data_dir() -> std::path::PathBuf {
        HOME.get().expect("android_main sets account home").clone()
    }
    pub fn try_data_dir() -> Option<std::path::PathBuf> {
        HOME.get().cloned()
    }
    /// Installed game versions, newest tag first (the launcher's `backend::installed`).
    pub fn installed() -> Vec<String> {
        let dir = data_dir().join("versions");
        let mut tags: Vec<String> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().join(".installed").exists())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        tags.sort_by(|a, b| b.cmp(a));
        tags
    }
    pub fn version_dir(tag: &str) -> std::path::PathBuf {
        data_dir().join("versions").join(tag)
    }
}

use std::cell::RefCell;
pub(crate) fn keyboard_mouse_input() -> bool {
    horizon_client::android_runtime::active_input() == catalog::InputHost::KeyboardMouse
}
pub(crate) fn configure_input_mode(mode: catalog::InputHost) {
    horizon_client::android_runtime::configure_input(mode);
}
pub(crate) fn touch_visible() -> bool {
    horizon_client::android_runtime::active_input() == catalog::InputHost::Touch
}
pub(crate) fn touch_input(touch: bool) {
    horizon_client::android_runtime::note_input(if touch {
        catalog::InputHost::Touch
    } else {
        catalog::InputHost::KeyboardMouse
    });
}
pub(crate) fn save_input_mode() {
    APP.with(|app| {
        if let Some(home) = app.borrow().as_ref().and_then(|a| a.internal_data_path()) {
            let mut config = catalog::Config::load(&home);
            let mode = horizon_client::android_runtime::input_mode();
            if config.input_host != mode {
                config.input_host = mode;
                if let Err(e) = config.save(&home) {
                    log::warn!("Saving input mode: {e}");
                }
            }
        }
    });
}
pub(crate) const PUBLIC_BUILD: bool = match option_env!("HORIZON_ANDROID_PUBLIC") {
    Some(value) => value.len() == 1 && value.as_bytes()[0] == b'1',
    None => false,
};

#[unsafe(no_mangle)]
fn android_on_create(state: &winit::platform::android::activity::OnCreateState<'_>) {
    init_logging();
    horizon_protocol::install_system_srv_lookup(android_dns::query_srv);
    if let Err(error) =
        unsafe { native_ime::initialize(state.vm_as_ptr().cast(), state.activity_as_ptr().cast()) }
    {
        log::error!("Android system keyboard initialization: {error}");
    }
}
use winit::{
    event_loop::EventLoop,
    platform::android::{EventLoopBuilderExtAndroid, activity::AndroidApp},
};

thread_local! {
    static APP: RefCell<Option<AndroidApp>> = const { RefCell::new(None) };
}

// `crate::Result` is the engine's one-parameter alias; this host returns winit's own error.
pub(crate) fn android_event_loop(
) -> std::result::Result<EventLoop<()>, winit::error::EventLoopError> {
    APP.with(|app| {
        EventLoop::builder()
            .with_android_app(
                app.borrow()
                    .as_ref()
                    .expect("android_main sets APP")
                    .clone(),
            )
            .build()
    })
}

pub(crate) fn init_logging() {
    let trace_gles = !PUBLIC_BUILD && std::env::var_os("HORIZON_GLES_TRACE").is_some();
    let mut logger = android_logger::Config::default()
        .with_max_level(if trace_gles {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        })
        .with_tag("Horizon");
    if trace_gles {
        logger = logger.with_filter(
            android_logger::FilterBuilder::new()
                .parse("info,wgpu_hal::gles=trace,wgpu_core::device=debug")
                .build(),
        );
    }
    struct LocalLogger(android_logger::AndroidLogger);
    impl log::Log for LocalLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            log::Log::enabled(&self.0, metadata)
        }
        fn log(&self, record: &log::Record<'_>) {
            if !self.enabled(record.metadata()) {
                return;
            }
            log::Log::log(&self.0, record);
            diagnostics::log_line(record.level(), record.target(), &record.args().to_string());
        }
        fn flush(&self) {
            diagnostics::request_flush();
        }
    }
    static LOGGER: std::sync::OnceLock<LocalLogger> = std::sync::OnceLock::new();
    let logger = LOGGER.get_or_init(|| LocalLogger(android_logger::AndroidLogger::new(logger)));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(if trace_gles {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        });
    }
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    use winit::platform::android::activity::input::Axis;
    for axis in [
        Axis::Vscroll,
        Axis::Hscroll,
        Axis::RelativeX,
        Axis::RelativeY,
    ] {
        app.enable_motion_axis(axis);
    }
    init_logging();
    let Some(home) = app.internal_data_path() else {
        log::error!("Android supplied no application data directory");
        return;
    };
    backend::initialize(home.clone());
    if let Err(error) = diagnostics::initialize(&home, serde_json::json!({})) {
        log::error!("Cannot initialize local diagnostics: {error}");
    }
    if let Ok(vm) = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) } {
        secure_store::initialize(vm);
    }
    let engine_data = app.asset_manager().open(c"engine-data.zip");
    APP.with(|slot| *slot.borrow_mut() = Some(app));
    let Some(cache) = platform::cache_dir() else {
        log::error!("Android supplied no application cache directory");
        return;
    };
    horizon_client::android_runtime::configure_cache_dir(cache.clone());
    horizon_client::official_install::configure_download_cache_dir(cache);
    horizon_client::android_runtime::install_clipboard(
        horizon_client::android_runtime::ClipboardHooks {
            get: platform::clipboard_get,
            set: platform::clipboard_set,
        },
    );
    if let Err(error) =
        std::fs::create_dir_all(&home).and_then(|()| std::env::set_current_dir(&home))
    {
        log::error!("Cannot open {}: {error}", home.display());
        return;
    }
    #[cfg(debug_assertions)]
    if secure_store::development_probe() {
        std::process::exit(0);
    }
    horizon_client::crash::install(home.clone(), "client");
    exit_reports::install(&home);
    let _ = std::fs::remove_file(home.join("android-error.txt"));
    if let Some(mut data) = engine_data {
        let copied = (|| -> std::io::Result<()> {
            let mut file = std::fs::File::create(home.join("engine-data.partial"))?;
            std::io::copy(&mut data, &mut file)?;
            drop(file);
            std::fs::rename(
                home.join("engine-data.partial"),
                home.join("engine-data.zip"),
            )
        })();
        if let Err(error) = copied {
            log::error!("Cannot unpack engine data: {error}");
        }
    }
    let request = std::fs::read(home.join(bridge::LAUNCH_REQUEST))
        .ok()
        .map(|bytes| {
            let _ = std::fs::remove_file(home.join(bridge::LAUNCH_REQUEST));
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .and_then(|request| request.get("world")?.as_str().map(str::to_owned))
        });
    let Some(world) = request else {
        // The Compose launcher is the only UI; without a pending play request
        // this process has nothing to show. Finish and land back in it.
        log::warn!("The game process started without a play request; returning to the launcher");
        platform::finish_activity();
        diagnostics::finish();
        platform::finish();
        std::process::exit(0);
    };
    let mut host = direct::DirectGame::new(home.clone(), world);
    let result = android_event_loop()
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
        .and_then(|events| events.run_app(&mut host).map_err(Into::into));
    if let Err(error) = result {
        log::error!("Horizon: {error}");
        diagnostics::record_error(&error.to_string());
        horizon_client::crash::report_error(&home, "client", &*error);
        let _ = std::fs::write(home.join("android-error.txt"), error.to_string());
    }
    // Winit permits one EventLoop per process. NativeActivity otherwise keeps
    // this process cached after Quit, making the next launch fail. run_app's
    // exiting callback has completed the world save before we get here.
    drop(host);
    diagnostics::finish();
    platform::finish();
    APP.with(|slot| *slot.borrow_mut() = None);
    std::process::exit(0);
}

/// Conventional JNI entry for an SAF result restored before the IME bridge is
/// ready. Only the cdylib exports this symbol; the shared gameplay library
/// contains the implementation without the export name, so a modded build
/// (which links both) does not hit a duplicate-symbol linker error.
#[cfg(target_os = "android")]
#[unsafe(export_name = "Java_dev_horizon_android_HorizonActivity_nativeDocumentResult")]
pub extern "system" fn native_document_result(
    env: jni::JNIEnv<'_>,
    activity: jni::objects::JObject<'_>,
    request: i32,
    result: i32,
    uri: jni::objects::JString<'_>,
) {
    documents::document_result(env, activity, request, result, uri);
}
