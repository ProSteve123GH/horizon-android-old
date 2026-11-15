//! The launcher-side globals the shared account, skin, report and mod code
//! read: the version this build names, the data folder the JNI entry set, and
//! the mod API level it serves. `android_main` fills the folder once.
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
