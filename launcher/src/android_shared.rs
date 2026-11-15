//! The desktop launcher's backend modules the Android JNI bridge shares.
//!
//! The Android binary (`crates/horizon-client/src/android_main.rs`, the
//! `horizon-android` cdylib) cannot depend on this crate: the launcher's
//! dependencies include GPUI (`gpui-fast`), which is desktop-only. It
//! therefore includes the launcher's backend files with `#[path]`, and this
//! list is the crate root they resolve against: because it lives in
//! `launcher/src/`, every `mod` in it finds the same file it would in the
//! launcher, and their own submodules (`browser/…`, `skins/…`) resolve as
//! regular modules of this one.
//!
//! Nothing in `launcher/` declares this file, so it is not part of the
//! desktop launcher; keep it in step with the modules the shared files use
//! (`cargo check -p horizon-android` names any missing one). `backend` is
//! here because the shared files call it; its `data_dir`/`game_home` answer
//! with the app home on Android (the game's library backend).
//!
//! Not listed: `catalog`, `secure_store` and `proxy`, which the Android crate
//! provides itself (`crates/horizon-client/src/android/`, and `proxy.rs` is
//! the same launcher file included directly).
#![allow(dead_code, unused_imports)]

pub mod account;
pub mod backend;
pub mod browser;
pub mod build_import;
pub mod data_location;
pub mod datapacks;
pub mod desktop;
pub mod desktop_seams;
pub mod game_app;
pub mod github;
pub mod github_mods;
pub mod i18n;
#[cfg(windows)]
pub mod installer;
pub mod instances;
pub mod java;
pub mod java_launch;
pub mod launch;
pub mod loaders;
#[cfg(target_os = "macos")]
pub mod mac_update;
pub mod minecraft_versions;
pub mod modpacks;
pub mod mojang;
pub mod native_cache;
pub mod native_fit;
pub mod native_mods;
pub mod native_status;
pub mod nightly;
pub mod other_launchers;
pub mod packs;
pub mod process;
pub mod rust_toolchain;
pub mod server_list;
pub mod setup_args;
pub mod shortcut;
pub mod signed;
pub mod skins;
pub mod sources;
pub mod themes;
pub mod uninstall_aside;
pub mod update_recovery;
#[cfg(windows)]
pub mod windows_runtime;
pub mod worlds;
