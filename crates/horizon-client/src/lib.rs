//! Gameplay and startup, above the reusable client library.
// No `#![recursion_limit]` or other inner attribute horizon-mixins doesn't know here: the
// launcher's horizon-mixins then rejects every native mod targeting this crate (#8169).
#![cfg_attr(not(target_os = "android"), forbid(unsafe_code))]
extern crate self as horizon_client;
pub use horizon_client_lib::*;
pub mod entry;
// On Android the cdylib drives gameplay; this library copy of game is compiled
// for shared paths but has no entry point of its own here. Desktop is unaffected.
#[cfg_attr(target_os = "android", allow(dead_code))]
pub mod game;
mod horizon_look;
pub mod mod_api;

#[cfg(test)]
mod brewing_speed_load_tests;
#[cfg(test)]
mod living_death_tests;


// Android's host glue. game.rs is compiled both into this library and into the
// horizon-android cdylib (which declares the same modules in android_main.rs);
// the library copy needs the same crate-root paths to resolve on Android. The
// cdylib is the one that drives the app, so the library copy of these hosts is
// never called at runtime.
#[cfg(target_os = "android")]
thread_local! {
    /// The Android activity, set by the cdylib's entry point; the library copy
    /// of the host glue reads it the same way.
    pub(crate) static APP: std::cell::RefCell<
        Option<winit::platform::android::activity::AndroidApp>,
    > = const { std::cell::RefCell::new(None) };
}

// Android-only JNI/entry boundary: these host modules give the shared gameplay
// source its crate-root paths in this copy. The cdylib owns the real entry
// point, so nothing here is called at runtime and dead-code lint is noise.
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/platform.rs"]
mod platform;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/display.rs"]
mod display;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/device_runtime.rs"]
mod device_runtime;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/document_bridge.rs"]
mod document_bridge;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/documents.rs"]
mod documents;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/gallery.rs"]
mod gallery;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/native_ime.rs"]
mod native_ime;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/performance.rs"]
mod performance;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/diagnostics.rs"]
mod diagnostics;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/shared_storage.rs"]
mod shared_storage;
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
#[path = "android/device_preferences.rs"]
mod device_preferences;

// Android-only stub backend, used only to resolve paths from the cdylib copy.
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
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
}

// Only the pieces of the launcher's catalog the host glue names. The full
// catalog lives in the cdylib, which is where the Android launcher runs.
#[cfg(target_os = "android")]
#[cfg_attr(target_os = "android", allow(dead_code))]
mod catalog {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Orientation {
        #[default]
        Landscape,
        Portrait,
        ReverseLandscape,
        ReversePortrait,
        Auto,
    }
    impl Orientation {
        pub fn android_value(self) -> i32 {
            match self {
                Self::Landscape => 0,
                Self::Portrait => 1,
                Self::ReverseLandscape => 8,
                Self::ReversePortrait => 9,
                Self::Auto => -1,
            }
        }
    }
}

#[cfg(target_os = "android")]
pub(crate) const PUBLIC_BUILD: bool = match option_env!("HORIZON_ANDROID_PUBLIC") {
    Some(value) => value.len() == 1 && value.as_bytes()[0] == b'1',
    None => false,
};

#[cfg(target_os = "android")]
mod android_host {
    // The cdylib owns the AndroidApp and the launcher config; this library copy
    // of the game never drives the app, so only the input state it reads at
    // runtime needs to be honest.
    pub(crate) fn android_event_loop(
    ) -> Result<winit::event_loop::EventLoop<()>, winit::error::EventLoopError> {
        winit::event_loop::EventLoop::new()
    }
    pub(crate) fn keyboard_mouse_input() -> bool {
        horizon_client::android_runtime::active_input()
            == horizon_client::android_gamepad::InputMode::KeyboardMouse
    }
    pub(crate) fn touch_visible() -> bool {
        horizon_client::android_runtime::active_input()
            == horizon_client::android_gamepad::InputMode::Touch
    }
    pub(crate) fn touch_input(touch: bool) {
        horizon_client::android_runtime::note_input(if touch {
            horizon_client::android_gamepad::InputMode::Touch
        } else {
            horizon_client::android_gamepad::InputMode::KeyboardMouse
        });
    }
    pub(crate) fn save_input_mode() {}
}
#[cfg(target_os = "android")]
pub(crate) use android_host::{
    android_event_loop, keyboard_mouse_input, save_input_mode, touch_input, touch_visible,
};
