//! Android's native entry point. The simulation and renderer are shared with desktop.
#![cfg(target_os = "android")]

#[path = "android/game_launch.rs"]
mod game_launch;
#[path = "android/launcher.rs"]
mod launcher;
// Share the desktop launcher's actual search, compatibility checks, downloads,
// dependency resolution and Java content porting; GPUI itself is desktop-only.
// These files are formatted in the launcher's separate Rust 2021 workspace.
// The launcher's backend modules the JNI bridge shares, in one place: the
// list lives in `launcher/src/` so its modules resolve their own submodules
// (`browser/…`, `skins/…`) as they do in the launcher (that crate needs GPUI,
// which does not build for Android). rustfmt skips the shared files: they are
// formatted in the launcher's own edition-2021 workspace.
#[rustfmt::skip]
#[path = "../../../launcher/src/android_shared.rs"]
mod launcher_shared;
pub(crate) use launcher_shared::*;
#[path = "android/dns.rs"]
mod android_dns;
#[path = "android/instances.rs"]
mod android_instances;
#[path = "android/appearance.rs"]
mod appearance;
#[path = "android/bridge.rs"]
mod bridge;
#[path = "android/catalog.rs"]
mod catalog;
#[path = "android/device_runtime.rs"]
mod device_runtime;
#[path = "android/document_bridge.rs"]
mod document_bridge;
#[path = "android/document_provider.rs"]
mod document_provider;
#[path = "android/documents.rs"]
mod documents;
#[path = "android/exit_reports.rs"]
mod exit_reports;
#[path = "android/gallery.rs"]
mod gallery;
// The desktop launcher's world model: discovery, metadata, copying, renaming
// and deletion. The Android world manager adds only its UI and launch request.
#[path = "android/minecraft_import.rs"]
mod minecraft_import;
#[path = "android/native_builds.rs"]
mod native_builds;
#[path = "android/native_built.rs"]
mod native_built;
#[path = "android/native_ime.rs"]
mod native_ime;
#[path = "android/performance_presets.rs"]
mod performance_presets;
#[path = "android/platform.rs"]
mod platform;
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../launcher/src/proxy.rs"]
mod proxy;
// The desktop launcher's servers.dat model: vanilla NBT, pictures and the fields the launcher
// does not use. The Android Server Manager adds its UI, favorites and groups.
#[path = "android/screenshots.rs"]
mod screenshots;
#[path = "android/search_queue.rs"]
mod search_queue;
#[path = "android/secure_store.rs"]
mod secure_store;
#[path = "android/servers.rs"]
mod servers;
#[path = "android/shader_catalog.rs"]
mod shader_catalog;
#[path = "android/touch_layouts.rs"]
mod touch_layouts;
// The shared account protocol needs only the launcher's version and data path.
// Modules without JNI or the activity moved into the library, so the game it
// runs uses the same state as the launcher around it.
pub(crate) use horizon_client::{
    device_preferences, diagnostics, display, document_store, performance,
};

use std::cell::RefCell;
pub(crate) fn configure_input_mode(mode: catalog::InputHost) {
    horizon_client::android_runtime::configure_input(mode);
}

/// The library's `crate::save_input_mode`, kept for the launcher's next start. It uses the
/// same home the rest of the game does (the app-specific external folder when available),
/// never the old internal folder the activity started with.
fn save_input_mode() {
    let Some(home) = horizon_client::backend::try_data_dir() else {
        return;
    };
    let mut config = catalog::Config::load(&home);
    let mode = horizon_client::android_runtime::input_mode();
    if config.input_host != mode {
        config.input_host = mode;
        if let Err(e) = config.save(&home) {
            log::warn!("Saving input mode: {e}");
        }
    }
}
pub(crate) use horizon_client::PUBLIC_BUILD;

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
/// Winit allows one event loop per process. Android can recreate the NativeActivity in a
/// process whose old activity was destroyed (for example after a render-thread panic) without
/// the process having exited yet. Building a second loop only reports "EventLoop can't be
/// recreated" and hides the first failure; leave the process instead and let the next launch
/// start clean.
static EVENT_LOOP_TAKEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn android_event_loop() -> Result<EventLoop<()>, winit::error::EventLoopError> {
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

/// Fill the table the gameplay library calls through `crate::platform`,
/// `crate::documents`, `crate::gallery`, `crate::native_ime` and
/// `crate::device_runtime` (`horizon_client::android_host`).
fn install_host() {
    horizon_client::android_host::install(horizon_client::android_host::Host {
        platform_active: platform::active,
        platform_show: platform::show,
        platform_show_multiline: platform::show_multiline,
        platform_hide: platform::hide,
        platform_poll: platform::poll,
        platform_field_at: platform::field_at,
        platform_cancel_vibration: platform::cancel_vibration,
        platform_vibrate: platform::vibrate,
        platform_poll_gamepad_connections: platform::poll_gamepad_connections,
        platform_capture_mouse: platform::capture_mouse,
        platform_performance_power: platform::performance_power,
        platform_open_url: platform::open_url,
        platform_open_documents: platform::open_documents,
        platform_first_frame: platform::first_frame,
        platform_display_rotation: platform::display_rotation,
        documents_poll: documents::poll,
        documents_draw: documents::draw,
        gallery_save: gallery::save,
        gallery_poll: gallery::poll,
        native_ime_viewport_bottom: native_ime::viewport_bottom,
        native_ime_refresh_bounds: native_ime::refresh_bounds,
        device_runtime_frame: device_runtime::frame,
        device_runtime_gyro: device_runtime::gyro,
        device_runtime_diagnostics: device_runtime::diagnostics,
        device_runtime_suspend: device_runtime::suspend,
        device_runtime_preferences: device_runtime::preferences,
        save_input_mode,
        android_event_loop,
    });
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
    // `HorizonActivity` selects the app-specific external folder (shared storage, no SAF
    // needed to read and write it) before the native entry point runs; an older or unusual
    // start without it keeps the internal folder.
    let home = std::env::var_os("HORIZON_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| app.internal_data_path());
    let Some(home) = home else {
        log::error!("Android supplied no application data directory");
        return;
    };
    horizon_client::backend::initialize(home.clone());
    if let Err(error) = diagnostics::initialize(&home, serde_json::json!({})) {
        log::error!("Cannot initialize local diagnostics: {error}");
    }
    if let Ok(vm) = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) } {
        secure_store::initialize(vm);
    }
    let engine_data = app.asset_manager().open(c"engine-data.zip");
    APP.with(|slot| *slot.borrow_mut() = Some(app));
    install_host();
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
    // The provider exposes `performance/`, not `crash-reports/`. Keep a copy of every crash
    // there too, so the report is reachable through Files (and in the diagnostic ZIP) even
    // when the crash happened before the launcher could export anything. This runs before the
    // crash report is saved: saving the copy must never depend on a later summary, upload or
    // provider callback succeeding.
    horizon_client::crash::on_report(horizon_client::diagnostics::copy_crash_report);
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
    let mut launcher = launcher::Launcher::new(home.clone());
    // The Compose launcher (main process) starts this `:game` process to play,
    // optionally into the world it selected (`{"play":true,"world":"<folder>"}`).
    if let Ok(bytes) = std::fs::read(home.join(bridge::LAUNCH_REQUEST)) {
        let _ = std::fs::remove_file(home.join(bridge::LAUNCH_REQUEST));
        let world = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|request| request.get("world")?.as_str().map(str::to_owned));
        launcher.play_directly(world);
    }
    if EVENT_LOOP_TAKEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        log::error!(
            "Horizon: Android recreated the game activity in a process that already ran its \
             event loop; exiting instead of reporting a second event-loop failure"
        );
        std::process::exit(0);
    }
    let result = android_event_loop()
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
        .and_then(|events| events.run_app(&mut launcher).map_err(Into::into));
    if let Err(error) = result {
        log::error!("Horizon: {error}");
        diagnostics::record_error(&error.to_string());
        horizon_client::crash::report_error(&home, "client", &*error);
        let _ = std::fs::write(home.join("android-error.txt"), error.to_string());
    }
    // Winit permits one EventLoop per process. NativeActivity otherwise keeps
    // this process cached after Quit, making the next launch fail. run_app's
    // exiting callback has completed the world save before we get here.
    drop(launcher);
    diagnostics::finish();
    platform::finish();
    APP.with(|slot| *slot.borrow_mut() = None);
    std::process::exit(0);
}

unsafe extern "C" {
    fn dlopen(filename: *const std::ffi::c_char, flags: std::ffi::c_int) -> *mut std::ffi::c_void;
    fn dlsym(
        handle: *mut std::ffi::c_void,
        symbol: *const std::ffi::c_char,
    ) -> *mut std::ffi::c_void;
}

/// NativeActivity's entry point (the manifest's `android.app.func_name`): the game of this
/// library, or of the build with native mods `HorizonActivity` loaded in its place
/// (`HORIZON_GAME_LIBRARY`, `NativeMods.kt`). The activity loads that build first, for its
/// Java native methods; `dlopen` finds it already loaded, so the activity and the game share
/// one library.
///
/// # Safety
///
/// NativeActivity's call: an `ANativeActivity` and its saved state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn horizon_on_create(
    activity: *mut std::ffi::c_void,
    saved: *mut std::ffi::c_void,
    saved_size: usize,
) {
    unsafe extern "C" {
        /// android-activity's entry point, which runs `android_main`.
        fn ANativeActivity_onCreate(
            activity: *mut std::ffi::c_void,
            saved: *mut std::ffi::c_void,
            saved_size: usize,
        );
    }
    type OnCreate = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, usize);
    let own: OnCreate = ANativeActivity_onCreate;
    if let Some(path) = std::env::var_os("HORIZON_GAME_LIBRARY") {
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(path.as_bytes()).unwrap_or_default();
        // SAFETY: NUL-terminated names; RTLD_NOW (2). The symbol is android-activity's entry
        // point in a build of this same source, so it has the signature declared above.
        let entry = unsafe {
            let build = dlopen(path.as_ptr(), 2);
            if build.is_null() {
                std::ptr::null_mut()
            } else {
                dlsym(build, c"ANativeActivity_onCreate".as_ptr())
            }
        };
        if !entry.is_null() && entry != own as *mut std::ffi::c_void {
            // SAFETY: see above.
            let entry = unsafe { std::mem::transmute::<*mut std::ffi::c_void, OnCreate>(entry) };
            return unsafe { entry(activity, saved, saved_size) };
        }
        log::error!("The native mod build has no entry point: playing the APK's own game");
    }
    // SAFETY: NativeActivity's own arguments, passed on.
    unsafe { own(activity, saved, saved_size) }
}
