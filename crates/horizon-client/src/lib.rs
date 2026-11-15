//! Gameplay and startup, above the reusable client library.
// No `#![recursion_limit]` or other inner attribute horizon-mixins doesn't know here: the
// launcher's horizon-mixins then rejects every native mod targeting this crate (#8169).
#![forbid(unsafe_code)]
extern crate self as horizon_client;
pub use horizon_client_lib::*;
pub mod entry;
pub mod game;
mod horizon_look;
pub mod mod_api;
// Modules the fork keeps beside the client-library list: content imports the
// Android launcher and the game share, the Android resource extraction the
// official install uses, its installers, the official-mod move-in, and the
// touch UI the desktop windows call.
pub mod android_resources;
pub mod content_files;
pub mod ime_edit;
pub mod minecraft_import;
pub mod moved_extras;
pub mod official_install;
pub mod resource_install;
pub mod touch_ui;

#[cfg(test)]
mod brewing_speed_load_tests;
#[cfg(test)]
mod living_death_tests;
// Android-specific surface for the Compose launcher's JNI bridge.
pub use horizon_client_render::{android_gamepad, android_runtime, keypad, touch};

// Android system services the gameplay inside this library calls. The ones
// that need JNI, the activity or `dlopen` stay in the launcher's JNI crate
// (`horizon-android`, `src/android_main.rs`), which allows `unsafe`; this
// library declares the same paths (`crate::platform`, `crate::documents`, ...)
// over the table it installs at startup (`android_host`).
#[cfg(target_os = "android")]
pub const PUBLIC_BUILD: bool = match option_env!("HORIZON_ANDROID_PUBLIC") {
    Some(value) => value.len() == 1 && value.as_bytes()[0] == b'1',
    None => false,
};
#[cfg(target_os = "android")]
pub mod android_host;
/// The launcher's version and data folder, which the shared account, skin
/// and crash code read (`crates/horizon-client/src/android/backend.rs`).
#[cfg(target_os = "android")]
#[path = "android/backend.rs"]
pub mod backend;
#[cfg(target_os = "android")]
#[path = "android/device_preferences.rs"]
pub mod device_preferences;
#[cfg(target_os = "android")]
#[path = "android/diagnostics.rs"]
pub mod diagnostics;
#[cfg(target_os = "android")]
#[path = "android/display.rs"]
pub mod display;
#[cfg(target_os = "android")]
#[path = "android/document_store.rs"]
pub mod document_store;
#[cfg(target_os = "android")]
#[path = "android/performance.rs"]
pub mod performance;
#[cfg(target_os = "android")]
pub use android_host::{
    android_event_loop, device_runtime, documents, gallery, keyboard_mouse_input, native_ime,
    platform, save_input_mode, touch_input, touch_visible,
};

#[cfg(all(test, not(target_os = "android")))]
#[path = "android/device_preferences.rs"]
mod android_device_preferences;
