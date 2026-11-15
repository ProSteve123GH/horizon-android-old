//! The Android system services gameplay calls, through a table the JNI crate
//! (`horizon-android`, `src/android_main.rs`) fills once at startup.
//!
//! Everything that needs JNI, the native activity or `dlopen` lives there,
//! where `unsafe` is allowed; this library (`#![forbid(unsafe_code)]`) holds
//! the table and the types the calls exchange, under the paths the gameplay
//! already calls (`crate::platform`, `crate::documents`, ...).
#![allow(clippy::too_many_arguments)]

use crate::hud::{HudAtlas, HudBatch};
use std::path::Path;
use std::sync::OnceLock;

/// What the launcher's JNI side provides, installed before the game starts.
pub struct Host {
    pub platform_active: fn() -> bool,
    pub platform_show: fn(&str),
    pub platform_show_multiline: fn(&str),
    pub platform_hide: fn(),
    pub platform_poll: fn() -> Vec<crate::osk::Output>,
    pub platform_field_at: fn([f32; 2]) -> bool,
    pub platform_cancel_vibration: fn(),
    pub platform_vibrate: fn(crate::gamepad::Pulse),
    pub platform_poll_gamepad_connections: fn(),
    pub platform_capture_mouse: fn(bool) -> bool,
    pub platform_performance_power: fn() -> serde_json::Value,
    pub platform_open_url: fn(&str) -> bool,
    pub platform_open_documents: fn(&str) -> bool,
    pub platform_first_frame: fn(),
    pub platform_display_rotation: fn() -> i32,
    pub documents_poll: fn() -> Option<documents::Notice>,
    pub documents_draw: fn(&mut HudBatch, &HudAtlas),
    pub gallery_save: fn(&Path) -> Result<(), String>,
    pub gallery_poll: fn() -> Vec<Result<String, String>>,
    pub native_ime_viewport_bottom: fn() -> Option<u32>,
    pub native_ime_refresh_bounds: fn(),
    pub device_runtime_frame: fn(bool, u16, bool) -> device_runtime::Frame,
    pub device_runtime_gyro: fn(bool, f32, i32) -> [f32; 2],
    pub device_runtime_diagnostics: fn() -> serde_json::Value,
    pub device_runtime_suspend: fn(),
    pub device_runtime_preferences: fn() -> crate::device_preferences::Settings,
    pub save_input_mode: fn(),
    pub android_event_loop:
        fn() -> Result<winit::event_loop::EventLoop<()>, winit::error::EventLoopError>,
}
static HOST: OnceLock<Host> = OnceLock::new();

/// Fill the table. Called once from `android_main`, before anything loads.
pub fn install(host: Host) {
    let _ = HOST.set(host);
}
fn host() -> &'static Host {
    HOST.get()
        .expect("android_main installs the Android host before the game starts")
}

/// Whether the player is using a keyboard and mouse.
pub fn keyboard_mouse_input() -> bool {
    crate::android_runtime::active_input() == crate::android_gamepad::InputMode::KeyboardMouse
}
/// Whether the touch controls are visible.
pub fn touch_visible() -> bool {
    crate::android_runtime::active_input() == crate::android_gamepad::InputMode::Touch
}
/// Note that the player touched the screen, or used the keyboard.
pub fn touch_input(touch: bool) {
    crate::android_runtime::note_input(if touch {
        crate::android_gamepad::InputMode::Touch
    } else {
        crate::android_gamepad::InputMode::KeyboardMouse
    });
}
/// Keep the picked input mode for the launcher's next start.
pub fn save_input_mode() {
    (host().save_input_mode)();
}
/// The event loop the system activity needs (`AndroidApp`).
pub fn android_event_loop() -> Result<winit::event_loop::EventLoop<()>, winit::error::EventLoopError>
{
    (host().android_event_loop)()
}

/// The activity's input methods and vibration (`android/platform.rs`).
pub mod platform {
    use super::host;
    /// Whether the native activity is up (the window has focus to give it).
    pub fn active() -> bool {
        (host().platform_active)()
    }
    /// The system keyboard, over the game's text field.
    pub fn show(text: &str) {
        (host().platform_show)(text);
    }
    pub fn show_multiline(text: &str) {
        (host().platform_show_multiline)(text);
    }
    pub fn hide() {
        (host().platform_hide)();
    }
    /// What the player typed since the last call.
    pub fn poll() -> Vec<crate::osk::Output> {
        (host().platform_poll)()
    }
    /// Whether the system keyboard's field is under `p`.
    pub fn field_at(p: [f32; 2]) -> bool {
        (host().platform_field_at)(p)
    }
    pub fn cancel_vibration() {
        (host().platform_cancel_vibration)();
    }
    pub fn vibrate(pulse: crate::gamepad::Pulse) {
        (host().platform_vibrate)(pulse);
    }
    pub fn poll_gamepad_connections() {
        (host().platform_poll_gamepad_connections)();
    }
    pub fn capture_mouse(capture: bool) -> bool {
        (host().platform_capture_mouse)(capture)
    }
    pub fn performance_power() -> serde_json::Value {
        (host().platform_performance_power)()
    }
    pub fn open_url(url: &str) -> bool {
        (host().platform_open_url)(url)
    }
    pub fn open_documents(id: &str) -> bool {
        (host().platform_open_documents)(id)
    }
    pub fn first_frame() {
        (host().platform_first_frame)();
    }
    pub fn display_rotation() -> i32 {
        (host().platform_display_rotation)()
    }
}

/// A finished SAF import or export (`android/documents.rs`).
pub mod documents {
    use super::host;
    use crate::hud::{HudAtlas, HudBatch};
    /// A line for the player once an import finished.
    #[derive(Clone)]
    pub struct Notice {
        pub text: String,
        /// A pack changed: the lists are scanned again.
        pub changed: bool,
        pub kind: Option<crate::content_files::Kind>,
    }
    /// The import that finished since the last call.
    pub fn poll() -> Option<Notice> {
        (host().documents_poll)()
    }
    /// The import's overlay, over a menu.
    pub fn draw(batch: &mut HudBatch, atlas: &HudAtlas) {
        (host().documents_draw)(batch, atlas);
    }
}

/// Saving a screenshot to the gallery (`android/gallery.rs`).
pub mod gallery {
    use super::host;
    use std::path::Path;
    pub fn save(file: &Path) -> Result<(), String> {
        (host().gallery_save)(file)
    }
    /// Exports that finished since the last call.
    pub fn poll() -> Vec<Result<String, String>> {
        (host().gallery_poll)()
    }
}

/// The system keyboard's bounds (`android/native_ime.rs`).
pub mod native_ime {
    use super::host;
    /// Where the keyboard's top edge is, in window pixels.
    pub fn viewport_bottom() -> Option<u32> {
        (host().native_ime_viewport_bottom)()
    }
    /// The window changed: ask the system for the keyboard's bounds again.
    pub fn refresh_bounds() {
        (host().native_ime_refresh_bounds)();
    }
}

/// Display refresh and motion (`android/device_runtime.rs`).
pub mod device_runtime {
    use super::host;
    /// A frame being timed for the performance report and the platform's
    /// frame pacing; dropping it records the frame (`end_frame` in the JNI
    /// crate, which owns the state).
    pub struct Frame {
        end: fn(),
    }
    impl Frame {
        #[doc(hidden)]
        pub fn new(end: fn()) -> Self {
            Self { end }
        }
    }
    impl Drop for Frame {
        fn drop(&mut self) {
            (self.end)();
        }
    }
    pub fn frame(enabled: bool, hz: u16, active: bool) -> Frame {
        (host().device_runtime_frame)(enabled, hz, active)
    }
    pub fn gyro(enabled: bool, sensitivity: f32, rotation: i32) -> [f32; 2] {
        (host().device_runtime_gyro)(enabled, sensitivity, rotation)
    }
    pub fn diagnostics() -> serde_json::Value {
        (host().device_runtime_diagnostics)()
    }
    pub fn suspend() {
        (host().device_runtime_suspend)();
    }
    pub fn preferences() -> crate::device_preferences::Settings {
        (host().device_runtime_preferences)()
    }
}
