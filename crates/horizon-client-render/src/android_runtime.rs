//! Android renderer and platform policy, and the input runtime: which device is in charge, the
//! controllers and the keypad profile. The Android event backend, the options screen and the
//! game reach the same state from whichever thread delivers the event.
//!
//! The input half of this file is pure logic over [`android_gamepad`](crate::android_gamepad)
//! and [`keypad`](crate::keypad) and builds and is tested on every platform; the Android
//! build feeds it. The platform policy is Android-only.
#[path = "graphics_startup.rs"]
pub mod graphics_startup;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "android")]
static CACHE_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
#[cfg(target_os = "android")]
pub fn configure_cache_dir(path: std::path::PathBuf) {
    let _ = CACHE_DIR.set(path);
}
#[cfg(target_os = "android")]
pub(crate) fn cache_dir() -> Option<&'static std::path::Path> {
    CACHE_DIR.get().map(std::path::PathBuf::as_path)
}

// The input half of this file is a port of Starling's Android input runtime
// (https://github.com/starling-client/starling, `android` branch at
// 150a414b09a1, the input part of `crates/horizon-client-render/src/android_runtime.rs`), made by
// Vasysik and the Starling contributors. Copyright (c) 2026 Ludovic Marie and the
// Horizon contributors, under the same MIT license as Horizon (LICENSE at the
// repository root).
use crate::android_gamepad::{Devices, InputMode, Selection};
use crate::gamepad::PadState;
use crate::keypad::{Keypad, Settings};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use winit::event::{DeviceEvent, ElementState};
use winit::keyboard::KeyCode;

static SELECTION: LazyLock<Mutex<Selection>> = LazyLock::new(Mutex::default);
static GAMEPADS: LazyLock<Mutex<Devices>> = LazyLock::new(Mutex::default);
static KEYPAD: LazyLock<Mutex<Keypad>> = LazyLock::new(Mutex::default);
static KEYPAD_ENABLED: AtomicBool = AtomicBool::new(false);

/// A lock that a panic elsewhere never makes unusable: the state stays
/// consistent between calls, so input keeps working.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The input mode picked in the options.
pub fn input_mode() -> InputMode {
    lock(&SELECTION).mode()
}

/// The device in charge now (never `Auto`).
pub fn active_input() -> InputMode {
    lock(&SELECTION).active()
}

/// Pick an input mode. A change lets go of every held controller and keypad
/// input, so nothing stays pressed across the switch.
pub fn configure_input(mode: InputMode) {
    let changed = lock(&SELECTION).configure(mode);
    if changed {
        clear_gamepads();
        clear_keypad();
    }
}

/// A device of `kind` was used: in `Auto` it takes over. Returns whether it
/// may act (false when the player picked another mode). Leaving the
/// controller lets go of its held buttons.
pub fn note_input(kind: InputMode) -> bool {
    let (accepted, left_controller) = {
        let mut selection = lock(&SELECTION);
        let was = selection.active();
        let accepted = selection.note(kind);
        (
            accepted,
            accepted && was == InputMode::Controller && kind != InputMode::Controller,
        )
    };
    if left_controller {
        clear_gamepads();
    }
    accepted
}

/// A raw event of the controller with Android device id `id`: motion axes
/// and buttons by Android code. Returns whether the event was a controller's
/// (the caller then keeps it from other handlers); a meaningful one also
/// notes the controller as used. A removed device forgets every controller,
/// the platform then lists those still there ([`retain_gamepads`]).
pub fn gamepad_event(id: i32, event: &DeviceEvent) -> bool {
    let activity = {
        let mut devices = lock(&GAMEPADS);
        match *event {
            DeviceEvent::Motion { axis, value } => devices.axis(id, axis, value),
            DeviceEvent::Button { button, state } => {
                devices.key(id, button, state == ElementState::Pressed)
            }
            DeviceEvent::Removed => {
                devices.clear();
                false
            }
            _ => return false,
        }
    };
    if activity {
        note_input(InputMode::Controller);
    }
    true
}

/// Forget the controllers that are no longer connected.
pub fn retain_gamepads(ids: &[i32]) {
    lock(&GAMEPADS).retain(ids);
}

/// Let go of every controller's held input.
pub fn clear_gamepads() {
    lock(&GAMEPADS).clear();
}

/// The Android device id of the controller in charge (for its rumble), if a
/// controller is the active input.
pub fn active_gamepad_id() -> Option<i32> {
    if active_input() != InputMode::Controller {
        return None;
    }
    lock(&GAMEPADS).active_id()
}

/// The controller in charge as a pad, once per frame; nothing while another
/// device is the active input.
pub fn gamepad_state() -> PadState {
    if active_input() != InputMode::Controller {
        return PadState::default();
    }
    lock(&GAMEPADS).poll()
}

/// Use this keypad profile, on or off as it says.
pub fn configure_keypad_settings(settings: &Settings) {
    lock(&KEYPAD).configure(settings);
    KEYPAD_ENABLED.store(settings.enabled, Ordering::Relaxed);
}

/// Turn the keypad profile on or off, letting go of its held keys.
pub fn configure_keypad(enabled: bool) {
    KEYPAD_ENABLED.store(enabled, Ordering::Relaxed);
    clear_keypad();
}

pub fn keypad_enabled() -> bool {
    KEYPAD_ENABLED.load(Ordering::Relaxed)
}

/// A key event for the keypad profile. Returns true when the profile is on
/// and the key is one of its own (the caller keeps it from the game's
/// keyboard input).
pub fn keypad_key(key: KeyCode, down: bool) -> bool {
    keypad_enabled() && lock(&KEYPAD).key(key, down)
}

/// Let go of every key the keypad profile holds.
pub fn clear_keypad() {
    lock(&KEYPAD).clear();
}

/// The keypad profile's held actions as a pad, once per frame.
pub fn keypad_state() -> PadState {
    lock(&KEYPAD).poll()
}

/// The keypad profile and the controller in charge together, once per frame:
/// what the game's controller path reads on Android.
pub fn pad_state() -> PadState {
    keypad_state().merge(&gamepad_state())
}

#[cfg(test)]
mod tests;
static ATLAS_LIMIT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(16384);
/// The launcher opens the adapter before the resource worker builds the game atlas.
pub fn configure_atlas_limit(limit: u32) {
    ATLAS_LIMIT.store(limit, Ordering::Relaxed);
}
pub fn atlas_limit() -> u32 {
    ATLAS_LIMIT.load(Ordering::Relaxed)
}
// Zero preserves the engine's automatic/manual worker selection.
static WORKER_LIMIT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub fn configure_worker_limit(limit: u32) {
    WORKER_LIMIT.store(limit.min(64), Ordering::Relaxed);
}
pub fn limit_workers(requested: usize) -> usize {
    worker_limit(requested, WORKER_LIMIT.load(Ordering::Relaxed))
}
fn worker_limit(requested: usize, limit: u32) -> usize {
    if limit == 0 {
        requested.max(1)
    } else {
        requested.max(1).min(limit as usize)
    }
}
#[cfg(test)]
mod worker_limit_tests {
    #[test]
    fn ceiling_never_increases_the_existing_pool() {
        assert_eq!(super::worker_limit(6, 0), 6);
        assert_eq!(super::worker_limit(6, 2), 2);
        assert_eq!(super::worker_limit(2, 6), 2);
        assert_eq!(super::worker_limit(0, 0), 1);
    }
}
static GPU_BATCHING: AtomicBool = AtomicBool::new(false);
static OPEN_GL: AtomicBool = AtomicBool::new(cfg!(target_pointer_width = "32"));
pub fn configure_graphics(open_gl: bool) {
    OPEN_GL.store(open_gl, Ordering::Relaxed);
}
pub fn graphics_api() -> crate::graphics::GraphicsApi {
    if OPEN_GL.load(Ordering::Relaxed) {
        crate::graphics::GraphicsApi::OpenGl
    } else {
        crate::graphics::GraphicsApi::Vulkan
    }
}
pub fn configure_gpu(batching: bool) {
    GPU_BATCHING.store(batching, Ordering::Relaxed);
}
pub fn compatibility() -> bool {
    cfg!(target_os = "android") && !GPU_BATCHING.load(Ordering::Relaxed)
}

pub use crate::clipboard::{ClipboardHooks, clipboard_get, clipboard_set, install_clipboard};

static RETURN_TO_LAUNCHER: AtomicBool = AtomicBool::new(false);
pub fn return_to_launcher() {
    RETURN_TO_LAUNCHER.store(true, Ordering::Release);
}
pub fn take_launcher_return() -> bool {
    RETURN_TO_LAUNCHER.swap(false, Ordering::AcqRel)
}

// Disposable decoded-audio and empty mesh buffers only, not total RAM/VRAM.
static CACHE_BUDGET_MIB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub fn configure_cache_budget(mib: u32) {
    CACHE_BUDGET_MIB.store(mib.min(1024), Ordering::Relaxed);
}
pub fn cache_budget_overridden() -> bool {
    CACHE_BUDGET_MIB.load(Ordering::Relaxed) != 0
}
pub fn mesh_cache_budget() -> usize {
    cache_budgets(CACHE_BUDGET_MIB.load(Ordering::Relaxed)).0
}
pub fn sound_cache_budget() -> usize {
    cache_budgets(CACHE_BUDGET_MIB.load(Ordering::Relaxed)).1
}
fn cache_budgets(mib: u32) -> (usize, usize) {
    let bytes = if mib == 0 { 80 } else { mib.min(1024) } as usize * (1 << 20);
    (bytes / 5, bytes - bytes / 5)
}
#[cfg(test)]
mod cache_budget_tests {
    #[test]
    fn default_preserves_existing_sizes_and_custom_total_is_bounded() {
        assert_eq!(super::cache_budgets(0), (16 << 20, 64 << 20));
        for mib in [1, 16, 32, 64, 128, 1024] {
            let (mesh, sound) = super::cache_budgets(mib);
            assert_eq!(mesh + sound, mib as usize * (1 << 20));
        }
    }
}
