//! Launcher-facing touch layout profiles, stored per instance.
//!
//! The in-game touch editor (`horizon-client-render`'s touch module) owns moving,
//! resizing, hiding and custom keys. This module only stores named snapshots of
//! a touch layout and which profile is active, so a player can switch between
//! saved arrangements without opening the game. Profiles are pure JSON snapshots
//! of the engine's `horizon-touch.json` fields; the bridge serializes the
//! engine's `Settings` and hands the snapshot over, which keeps this module
//! testable without the engine.
//!
//! A snapshot captures only layout-relevant data: `mode`, `layouts`,
//! `button_visibility` and `custom_buttons`. Sensitivity, handedness, the
//! dynamic joystick, gyroscope and every other touch preference stay untouched
//! when a profile is applied, so no touch setting is lost or overwritten that
//! the profiles are not about.
//!
//! Applying a profile keeps every custom key the touch file has: a button whose
//! key matches a stored one takes the stored placement, a key added after the
//! profile was saved keeps its own, and keys the player removed stay removed.
//! Keys are gameplay actions, the same reason the in-game editor's reset
//! preserves them while resetting positions.
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::{fs, io, path::Path};

/// Launcher metadata, kept beside the instance's saves and engine settings.
pub const STORE_FILE: &str = "android-touch-layouts.json";
/// The engine's touch settings file in an instance.
pub const TOUCH_FILE: &str = "horizon-touch.json";
/// The longest profile name the launcher stores.
pub const MAX_NAME: usize = 32;
/// The most profiles an instance keeps; snapshots are a few kilobytes each.
pub const MAX_PROFILES: usize = 32;
/// The longest custom-button key token a snapshot may store.
pub const MAX_KEY: usize = 128;
/// The fixed controls the engine lays out (`Control::ALL`).
pub const CONTROLS: usize = 14;

/// `(mode, label)` in the engine's `Mode::ALL` order.
pub const MODES: [(&str, &str); 3] = [
    ("crosshair", "Joystick & aim crosshair"),
    ("tap", "Joystick & tap to interact"),
    ("dpad", "D-Pad & tap to interact"),
];

/// Short labels of the fixed controls, in the engine's `Control::ALL` order.
/// The launcher draws them beside a profile's preview.
pub const CONTROL_LABELS: [&str; CONTROLS] = [
    "Move",
    "Sprint",
    "Jump",
    "Sneak",
    "Attack",
    "Use",
    "Chat",
    "Pause",
    "Settings",
    "Inventory",
    "Debug",
    "Perspective",
    "Offhand",
    "Shield",
];

pub fn mode_label(mode: &str) -> &'static str {
    MODES
        .iter()
        .find(|(value, _)| *value == mode)
        .map(|(_, label)| *label)
        .unwrap_or("Unknown mode")
}

fn number(value: &Json) -> Option<f64> {
    value.as_f64().filter(|value| value.is_finite())
}

fn fraction(value: &Json) -> Option<f64> {
    number(value).filter(|value| (0.0..=1.0).contains(value))
}

fn placement_ok(placement: &Json) -> Result<(), String> {
    let object = placement
        .as_object()
        .ok_or_else(|| "a button placement must be an object".to_owned())?;
    let center = object
        .get("center")
        .ok_or_else(|| "a button placement needs a center".to_owned())?;
    let center = center
        .as_array()
        .filter(|center| center.len() == 2)
        .ok_or_else(|| "a button center must be two numbers from 0 to 1".to_owned())?;
    for coordinate in center {
        fraction(coordinate)
            .ok_or_else(|| "a button center must be two numbers from 0 to 1".to_owned())?;
    }
    for key in ["size", "opacity"] {
        fraction(object.get(key).unwrap_or(&Json::Null))
            .ok_or_else(|| format!("a button {key} must be a number from 0 to 1"))?;
    }
    Ok(())
}

/// `(mode, layouts, button_visibility, custom_buttons)` inside a snapshot.
///
/// Layout rows may hold 10, 12 or 14 placements and visibility 12 or 14 values,
/// exactly what the engine's touch settings deserialize.
pub fn snapshot_fields(snapshot: &Json) -> Result<(String, &Json, &Json, &Json), String> {
    let object = snapshot
        .as_object()
        .ok_or_else(|| "A layout snapshot must be an object".to_owned())?;
    let mode = object
        .get("mode")
        .and_then(Json::as_str)
        .ok_or_else(|| "A layout snapshot needs a mode".to_owned())?;
    if !MODES.iter().any(|(value, _)| *value == mode) {
        return Err(format!("\"{mode}\" is not a touch control mode"));
    }
    let layouts = object
        .get("layouts")
        .ok_or_else(|| "A layout snapshot needs layouts".to_owned())?;
    let rows = layouts
        .as_array()
        .filter(|rows| rows.len() == 3)
        .ok_or_else(|| "Touch layouts need three rows (one per control mode)".to_owned())?;
    for (index, row) in rows.iter().enumerate() {
        let row = row
            .as_array()
            .ok_or_else(|| format!("Touch layout {index} needs its placements as a list"))?;
        if !matches!(row.len(), 10 | 12 | CONTROLS) {
            return Err(format!(
                "Touch layout {index} holds {} placements; expected 10, 12 or {CONTROLS}",
                row.len()
            ));
        }
        for (control, placement) in row.iter().enumerate() {
            placement_ok(placement)
                .map_err(|error| format!("Touch layout {index}, control {control}: {error}"))?;
        }
    }
    let visibility = object
        .get("button_visibility")
        .ok_or_else(|| "A layout snapshot needs button visibility".to_owned())?;
    let values = visibility
        .as_array()
        .filter(|values| matches!(values.len(), 12 | CONTROLS))
        .ok_or_else(|| "Button visibility needs 12 or 14 values".to_owned())?;
    if let Some(value) = values
        .iter()
        .find(|value| !matches!(value.as_i64(), Some(0..=2)))
    {
        return Err(format!(
            "Button visibility values must be 0 (default), 1 (visible) or 2 (hidden), not {value}"
        ));
    }
    let buttons = object
        .get("custom_buttons")
        .ok_or_else(|| "A layout snapshot needs its custom buttons".to_owned())?;
    let buttons = buttons
        .as_array()
        .ok_or_else(|| "A layout snapshot needs its custom buttons as a list".to_owned())?;
    if buttons.len() > 32 {
        return Err("A touch layout keeps at most 32 custom buttons".into());
    }
    for button in buttons {
        let object = button
            .as_object()
            .ok_or_else(|| "A custom button must be an object".to_owned())?;
        let key = object
            .get("key")
            .and_then(Json::as_str)
            .ok_or_else(|| "A custom button needs a key".to_owned())?;
        if key.is_empty() || key.len() > MAX_KEY {
            return Err("A custom button key is missing or too long".into());
        }
        let rows = button
            .get("layouts")
            .and_then(Json::as_array)
            .filter(|rows| rows.len() == 3)
            .ok_or_else(|| {
                format!("Custom button {key} needs three layout rows, one per control mode")
            })?;
        for (mode, placement) in rows.iter().enumerate() {
            placement_ok(placement)
                .map_err(|error| format!("Custom button {key}, mode {mode}: {error}"))?;
        }
    }
    Ok((
        mode.to_owned(),
        layouts,
        visibility,
        object.get("custom_buttons").expect("read above"),
    ))
}

/// Validates a whole layout snapshot.
pub fn valid_snapshot(snapshot: &Json) -> Result<(), String> {
    snapshot_fields(snapshot).map(|_| ())
}

/// The mode value a snapshot stores.
pub fn snapshot_mode(snapshot: &Json) -> Result<&str, String> {
    snapshot
        .as_object()
        .and_then(|object| object.get("mode"))
        .and_then(Json::as_str)
        .ok_or_else(|| "A layout snapshot needs a mode".to_owned())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Profile {
    id: String,
    name: String,
    snapshot: Json,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Store {
    selected: Option<String>,
    custom: Vec<Profile>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub snapshot: Json,
    pub selected: bool,
    pub valid: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct State {
    pub instance: String,
    pub selected: Option<String>,
    pub selected_name: Option<String>,
    pub profiles: Vec<ProfileView>,
    pub warning: Option<String>,
}

/// The Settings row value: how many profiles an instance keeps and which one matches.
pub fn summary(instance: &Path) -> String {
    let (store, _) = load_store(instance);
    match store.custom.len() {
        0 => "No layout profiles yet".to_owned(),
        count => match store
            .selected
            .as_deref()
            .and_then(|id| store.custom.iter().find(|profile| profile.id == id))
            .map(|profile| profile.name.clone())
        {
            Some(name) => format!("{count} profile(s) · \"{name}\""),
            None => format!("{count} profile(s)"),
        },
    }
}

fn instance_name(instance: &Path) -> String {
    instance
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| instance.display().to_string())
}

fn load_store(instance: &Path) -> (Store, Option<String>) {
    let path = instance.join(STORE_FILE);
    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<Store>(&bytes) {
            Ok(store) => (store, None),
            Err(error) => (
                Store::default(),
                Some(format!(
                    "Layout profiles could not be read ({error}). The file is kept; a backup is made before the next change."
                )),
            ),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => (Store::default(), None),
        Err(error) => (
            Store::default(),
            Some(format!("Layout profiles could not be read: {error}")),
        ),
    }
}

fn atomic_write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
    }
    let partial = path.with_file_name(format!(
        "{}.partial",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&partial, text).map_err(|e| format!("Cannot write {}: {e}", partial.display()))?;
    fs::rename(&partial, path).map_err(|e| {
        let _ = fs::remove_file(&partial);
        format!("Cannot replace {}: {e}", path.display())
    })
}

fn save_store(instance: &Path, store: &Store, damaged: bool) -> Result<(), String> {
    let path = instance.join(STORE_FILE);
    if damaged && path.exists() {
        let backup = instance.join(format!("{STORE_FILE}.corrupt"));
        if !backup.exists()
            && let Err(error) = fs::copy(&path, &backup)
        {
            return Err(format!(
                "Cannot back up the unreadable layout file: {error}"
            ));
        }
    }
    let text = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    atomic_write(&path, &text)
}

fn valid_name(store: &Store, skip: Option<&str>, raw: &str) -> Result<String, String> {
    let name: String = raw.chars().filter(|c| !c.is_control()).collect();
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the layout profile a name".into());
    }
    let name: String = name.chars().collect();
    let name = name.trim();
    if store.custom.iter().any(|profile| {
        Some(profile.id.as_str()) != skip
            && profile.name.trim().to_lowercase() == name.to_lowercase()
    }) {
        return Err(format!(
            "Another layout profile is already called \"{name}\""
        ));
    }
    Ok(name.chars().take(MAX_NAME).collect())
}

fn next_id(store: &Store) -> String {
    let mut next = 1u64;
    for profile in &store.custom {
        if let Some(number) = profile
            .id
            .strip_prefix("layout-")
            .and_then(|n| n.parse::<u64>().ok())
        {
            next = next.max(number + 1);
        }
    }
    format!("layout-{next}")
}

fn view(profile: &Profile, selected: Option<&str>) -> ProfileView {
    ProfileView {
        id: profile.id.clone(),
        name: profile.name.clone(),
        snapshot: profile.snapshot.clone(),
        selected: Some(profile.id.as_str()) == selected,
        valid: valid_snapshot(&profile.snapshot).is_ok(),
    }
}

/// The launcher's profile list for one instance, each with its snapshot.
pub fn state(instance: &Path) -> Result<State, String> {
    let (store, warning) = load_store(instance);
    let profiles: Vec<ProfileView> = store
        .custom
        .iter()
        .map(|profile| view(profile, store.selected.as_deref()))
        .collect();
    let selected_name = store.selected.as_deref().and_then(|id| {
        store
            .custom
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.name.clone())
    });
    let warning = warning.or_else(|| {
        store.selected.as_deref().and_then(|id| {
            (!store.custom.iter().any(|profile| profile.id == id))
                .then(|| "The selected layout profile no longer exists.".to_owned())
        })
    });
    Ok(State {
        instance: instance_name(instance),
        selected: store.selected,
        selected_name,
        profiles,
        warning,
    })
}

/// The selected profile's name, for the Settings row value.
pub fn selected_name(instance: &Path) -> Option<String> {
    let (store, _) = load_store(instance);
    let id = store.selected.as_deref()?;
    store
        .custom
        .iter()
        .find(|profile| profile.id == id)
        .map(|profile| profile.name.clone())
}

/// Saves the current engine layout as a new profile and selects it.
pub fn create(instance: &Path, name: &str, snapshot: &Json) -> Result<ProfileView, String> {
    valid_snapshot(snapshot)?;
    let (mut store, damaged) = load_store(instance);
    let name = valid_name(&store, None, name)?;
    if store.custom.len() >= MAX_PROFILES {
        return Err(format!(
            "This instance keeps at most {MAX_PROFILES} layout profiles. Delete one first."
        ));
    }
    let profile = Profile {
        id: next_id(&store),
        name,
        snapshot: snapshot.clone(),
    };
    store.custom.push(profile.clone());
    store.selected = Some(profile.id.clone());
    save_store(instance, &store, damaged.is_some())?;
    Ok(view(&profile, store.selected.as_deref()))
}

/// Overwrites one profile with the current engine layout.
pub fn save_current(instance: &Path, id: &str, snapshot: &Json) -> Result<ProfileView, String> {
    valid_snapshot(snapshot)?;
    let (mut store, damaged) = load_store(instance);
    let index = store
        .custom
        .iter()
        .position(|profile| profile.id == id)
        .ok_or("That layout profile no longer exists")?;
    store.custom[index].snapshot = snapshot.clone();
    let profile = store.custom[index].clone();
    save_store(instance, &store, damaged.is_some())?;
    Ok(view(&profile, store.selected.as_deref()))
}

pub fn rename(instance: &Path, id: &str, name: &str) -> Result<ProfileView, String> {
    let (mut store, damaged) = load_store(instance);
    let index = store
        .custom
        .iter()
        .position(|profile| profile.id == id)
        .ok_or("That layout profile no longer exists")?;
    store.custom[index].name = valid_name(&store, Some(id), name)?;
    let profile = store.custom[index].clone();
    save_store(instance, &store, damaged.is_some())?;
    Ok(view(&profile, store.selected.as_deref()))
}

pub fn duplicate(instance: &Path, id: &str) -> Result<ProfileView, String> {
    let (mut store, damaged) = load_store(instance);
    let index = store
        .custom
        .iter()
        .position(|profile| profile.id == id)
        .ok_or("That layout profile no longer exists")?;
    if store.custom.len() >= MAX_PROFILES {
        return Err(format!(
            "This instance keeps at most {MAX_PROFILES} layout profiles. Delete one first."
        ));
    }
    let source = store.custom[index].clone();
    let name = unique_name(&store, &format!("{} copy", source.name));
    let profile = Profile {
        id: next_id(&store),
        name,
        snapshot: source.snapshot.clone(),
    };
    store.custom.push(profile.clone());
    save_store(instance, &store, damaged.is_some())?;
    Ok(view(&profile, None))
}

fn unique_name(store: &Store, base: &str) -> String {
    if !name_taken(store, base) {
        return base.to_owned();
    }
    for n in 2..=999 {
        let stem = format!("{base} {n}");
        if !name_taken(store, &stem) {
            return stem;
        }
    }
    "Layout".to_owned()
}

fn name_taken(store: &Store, name: &str) -> bool {
    store.custom.iter().any(|profile| profile.name == name)
}

pub fn delete(instance: &Path, id: &str) -> Result<(), String> {
    let (mut store, damaged) = load_store(instance);
    let index = store
        .custom
        .iter()
        .position(|profile| profile.id == id)
        .ok_or("That layout profile no longer exists")?;
    store.custom.remove(index);
    if store.selected.as_deref() == Some(id) {
        store.selected = None;
    }
    save_store(instance, &store, damaged.is_some())
}

/// The engine snapshot of one profile, validated. The bridge merges it into
/// the touch settings; an invalid snapshot never reaches the engine.
pub fn snapshot(instance: &Path, id: &str) -> Result<Json, String> {
    let (store, _) = load_store(instance);
    let profile = store
        .custom
        .iter()
        .find(|profile| profile.id == id)
        .ok_or("That layout profile no longer exists")?;
    valid_snapshot(&profile.snapshot)?;
    Ok(profile.snapshot.clone())
}

/// Marks one profile active without touching the engine settings; the profile's
/// snapshot must have been applied to the touch file first.
pub fn set_selected(instance: &Path, id: &str) -> Result<(), String> {
    let (mut store, damaged) = load_store(instance);
    if !store.custom.iter().any(|profile| profile.id == id) {
        return Err("That layout profile no longer exists".into());
    }
    store.selected = Some(id.to_owned());
    save_store(instance, &store, damaged.is_some())
}

/// Clears the selection, e.g. after a reset to the engine's default layout.
pub fn clear_selected(instance: &Path) -> Result<(), String> {
    let (mut store, damaged) = load_store(instance);
    if store.selected.is_none() {
        return Ok(());
    }
    store.selected = None;
    save_store(instance, &store, damaged.is_some())
}

/// The `{mode, layouts, button_visibility, custom_buttons}` object to copy into
/// the engine's touch settings when the profile snapshot is applied to a
/// touch file's `current` settings.
///
/// Custom buttons merge by key: matching keys take the snapshot's placement,
/// keys added since the profile was saved keep their own, and keys the player
/// removed stay removed. Gameplay actions are never removed or replaced.
pub fn apply_fields(current: &Json, snapshot: &Json) -> Result<Json, String> {
    let (mode, layouts, visibility, buttons) = {
        let (mode, layouts, visibility, saved) = snapshot_fields(snapshot)?;
        (
            mode.to_owned(),
            layouts.clone(),
            visibility.clone(),
            saved.clone(),
        )
    };
    let stored: Vec<(String, Json)> = buttons
        .as_array()
        .expect("snapshot_fields validated the custom buttons")
        .iter()
        .filter_map(|button| {
            Some((
                button.get("key")?.as_str()?.to_owned(),
                button.get("layouts")?.clone(),
            ))
        })
        .collect();
    let merged: Vec<Json> = current
        .get("custom_buttons")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(32)
        .map(|mut button| {
            let key = button.get("key").and_then(Json::as_str);
            if let Some((_, layouts)) =
                key.and_then(|key| stored.iter().find(|(saved, _)| saved == key))
                && let Some(object) = button.as_object_mut()
            {
                object.insert("layouts".into(), layouts.clone());
            }
            button
        })
        .collect();
    Ok(serde_json::json!({
        "mode": mode,
        "layouts": layouts,
        "button_visibility": visibility,
        "custom_buttons": merged,
    }))
}

/// The fields for a full reset to the engine's default layout: the default
/// placements and visibility, the current mode kept, and every custom key kept
/// with its factory placement. `defaults` is the serialized engine
/// `Settings::default()` snapshot, built by the bridge.
pub fn reset_fields(current: &Json, defaults: &Json) -> Result<Json, String> {
    let mode = snapshot_mode(current)?;
    if valid_snapshot(defaults).is_err() {
        return Err("The engine's default touch layout is unreadable".into());
    }
    let default = defaults.as_object().expect("validated above");
    let layouts = default
        .get("layouts")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let visibility = default
        .get("button_visibility")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let factory: Vec<Json> = current
        .get("custom_buttons")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(32)
        .enumerate()
        .map(|(index, button)| {
            // The engine's CustomButton::new(key, index) grid placement, which
            // is the same for every control mode.
            let center_x = 0.68 + (index % 3) as f64 * 0.1;
            let center_y = 0.35 + (index / 3 % 3) as f64 * 0.12;
            let key = button
                .get("key")
                .and_then(Json::as_str)
                .unwrap_or("key.keyboard.unknown");
            serde_json::json!({
                "key": key,
                "layouts": [
                    {"center": [center_x, center_y], "size": 0.105, "opacity": 0.65},
                    {"center": [center_x, center_y], "size": 0.105, "opacity": 0.65},
                    {"center": [center_x, center_y], "size": 0.105, "opacity": 0.65},
                ],
            })
        })
        .collect();
    Ok(serde_json::json!({
        "mode": mode,
        "layouts": layouts,
        "button_visibility": visibility,
        "custom_buttons": factory,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "horizon-touch-layouts-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A sanitized-engine-shaped snapshot: 3 rows of 14 placements, 14 visibility
    /// values, one custom button per mode row.
    fn sample_snapshot(mode: &str, jump_center: [f64; 2], button_center: [f64; 2]) -> Json {
        let row = |offset: f64| -> Json {
            let mut placements = Vec::new();
            for control in 0..CONTROLS {
                let center = if control == 2 {
                    jump_center.clone()
                } else {
                    [0.2, 0.2 + offset]
                };
                placements.push(json!({
                    "center": center,
                    "size": 0.15,
                    "opacity": 0.65,
                }));
            }
            json!(placements)
        };
        json!({
            "mode": mode,
            "layouts": [
                row(0.0),
                row(0.1),
                row(0.2),
            ],
            "button_visibility": Vec::from([0u8; CONTROLS]),
            "custom_buttons": [
                {"key": "key.keyboard.z", "layouts": [
                    {"center": button_center, "size": 0.105, "opacity": 0.65},
                    {"center": button_center, "size": 0.105, "opacity": 0.65},
                    {"center": button_center, "size": 0.105, "opacity": 0.65},
                ]},
            ],
        })
    }

    fn engine_current(mode: &str, button: Option<Json>) -> Json {
        let mut custom = Vec::new();
        if let Some(placement) = button {
            custom.push(json!({
                "key": "key.keyboard.z",
                "layouts": [
                    {"center": placement, "size": 0.105, "opacity": 0.65},
                    {"center": placement, "size": 0.105, "opacity": 0.65},
                    {"center": placement, "size": 0.105, "opacity": 0.65},
                ],
            }));
        }
        json!({
            "mode": mode,
            "sensitivity": 1.0,
            "custom_buttons": custom,
            "layouts": [[
                {"center": [0.13, 0.76], "size": 0.32, "opacity": 0.65}, {"center": [0.72, 0.78], "size": 0.105, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}
            ], [
                {"center": [0.13, 0.76], "size": 0.32, "opacity": 0.65}, {"center": [0.72, 0.78], "size": 0.105, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}
            ], [
                {"center": [0.13, 0.76], "size": 0.32, "opacity": 0.65}, {"center": [0.72, 0.78], "size": 0.105, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65},
                {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}, {"center": [0.2, 0.3], "size": 0.15, "opacity": 0.65}
            ]],
            "button_visibility": Vec::from([0u8; CONTROLS]),
        })
    }

    #[test]
    fn a_valid_snapshot_is_accepted_and_broken_ones_are_rejected() {
        let snapshot = sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]);
        valid_snapshot(&snapshot).unwrap();
        // The legacy short forms the engine accepts.
        let mut legacy = snapshot.clone();
        legacy["layouts"] = json!([
            json!(
                (0..10)
                    .map(|_| json!({"center": [0.5, 0.5], "size": 0.15, "opacity": 0.5}))
                    .collect::<Vec<_>>()
            ),
            json!(
                (0..12)
                    .map(|_| json!({"center": [0.5, 0.5], "size": 0.15, "opacity": 0.5}))
                    .collect::<Vec<_>>()
            ),
            json!(
                (0..14)
                    .map(|_| json!({"center": [0.5, 0.5], "size": 0.15, "opacity": 0.5}))
                    .collect::<Vec<_>>()
            )
        ]);
        legacy["button_visibility"] = json!((0..12).map(|_| 0).collect::<Vec<_>>());
        valid_snapshot(&legacy).unwrap();

        let defaults_vis = json!(vec![0; CONTROLS]);
        for (_name, broken) in [
            ("unknown mode", json!({"mode": "magic"})),
            ("no layouts", json!({"mode": "dpad"})),
            (
                "zero rows",
                json!({"mode": "dpad", "layouts": [], "button_visibility": defaults_vis.clone(), "custom_buttons": []}),
            ),
        ] {
            let mut broken = broken.clone();
            broken["button_visibility"] = defaults_vis.clone();
            broken["custom_buttons"] = json!([]);
            valid_snapshot(&broken).unwrap_err();
        }
        valid_snapshot(&json!({"mode": "crosshair", "layouts": [
            (0..CONTROLS).map(|_| json!({"center": [0.5, 0.5], "size": 0.15, "opacity": 0.65})).collect::<Vec<_>>(),
            (0..CONTROLS).map(|_| json!({"center": [0.5, 0.5], "size": 0.15, "opacity": 0.65})).collect::<Vec<_>>(),
            (0..CONTROLS).map(|_| json!({"center": [0.5, 0.5], "size": 0.15, "opacity": 0.65})).collect::<Vec<_>>(),
        ], "button_visibility": json!(vec![0; CONTROLS]), "custom_buttons": []})).unwrap();
        let out_of_range = sample_snapshot("crosshair", [1.4, 0.7], [0.5, 0.5]);
        valid_snapshot(&out_of_range).unwrap_err();
        let mut hidden = sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]);
        hidden["custom_buttons"][0]["key"] = json!(format!("key.keyboard.{}", "x".repeat(200)));
        valid_snapshot(&hidden).unwrap_err();
    }

    #[test]
    fn engines_short_forms_normalize_and_profiles_roundtrip() {
        let scratch = Scratch::new();
        let view = create(
            scratch.path(),
            "  Travel\u{7}  ",
            &sample_snapshot("dpad", [0.3, 0.6], [0.8, 0.5]),
        )
        .unwrap();
        assert_eq!(view.name, "Travel");
        assert_eq!(view.id, "layout-1");
        assert!(view.selected && view.valid);
        assert_eq!(selected_name(scratch.path()).as_deref(), Some("Travel"));
        // The store survives a process restart: the same file is read again.
        let first_state = state(scratch.path()).unwrap();
        assert_eq!(first_state.profiles.len(), 1);
        assert_eq!(first_state.profiles[0].name, "Travel");
        assert_eq!(first_state.profiles[0].snapshot["mode"], "dpad");
        let renamed = rename(scratch.path(), "layout-1", "Holiday").unwrap();
        assert_eq!(renamed.name, "Holiday");
        let copy = duplicate(scratch.path(), "layout-1").unwrap();
        assert_eq!(copy.name, "Holiday copy");
        assert_eq!(copy.id, "layout-2");
        assert!(!copy.selected);
        assert!(
            state(scratch.path())
                .unwrap()
                .profiles
                .iter()
                .any(|p| p.name == "Holiday copy")
        );
        save_current(
            scratch.path(),
            "layout-1",
            &sample_snapshot("tap", [0.4, 0.6], [0.8, 0.4]),
        )
        .unwrap();
        assert_eq!(snapshot(scratch.path(), "layout-1").unwrap()["mode"], "tap");
        delete(scratch.path(), "layout-2").unwrap();
        let second_state = state(scratch.path()).unwrap();
        assert_eq!(second_state.profiles.len(), 1);
        assert_eq!(second_state.selected.as_deref(), Some("layout-1"));
        delete(scratch.path(), "layout-1").unwrap();
        assert_eq!(state(scratch.path()).unwrap().selected, None);
    }

    #[test]
    fn duplicate_or_long_or_taken_names_are_rejected() {
        let scratch = Scratch::new();
        create(
            scratch.path(),
            "Travel",
            &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]),
        )
        .unwrap();
        for bad in ["", "   ", "\u{1}"] {
            assert!(
                create(
                    scratch.path(),
                    bad,
                    &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5])
                )
                .is_err(),
                "{bad:?}"
            );
        }
        assert!(
            create(
                scratch.path(),
                "travel",
                &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5])
            )
            .is_err()
        );
        let long = "x".repeat(MAX_NAME + 1);
        assert_eq!(
            create(
                scratch.path(),
                &long,
                &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5])
            )
            .unwrap()
            .name
            .chars()
            .count(),
            MAX_NAME
        );
        let copy = duplicate(scratch.path(), "layout-1").unwrap();
        assert_eq!(copy.name, "Travel copy");
        let other = duplicate(scratch.path(), "layout-1").unwrap();
        assert_eq!(other.name, "Travel copy 2");
    }

    #[test]
    fn an_instance_keeps_at_most_max_profiles() {
        let scratch = Scratch::new();
        for index in 0..MAX_PROFILES {
            create(
                scratch.path(),
                &format!("Layout {index:02}"),
                &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]),
            )
            .unwrap();
        }
        let error = create(
            scratch.path(),
            "One over",
            &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]),
        )
        .unwrap_err();
        assert!(error.contains("at most 32"));
    }

    #[test]
    fn deleting_the_selected_or_missing_profile_is_handled() {
        let scratch = Scratch::new();
        create(
            scratch.path(),
            "Travel",
            &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]),
        )
        .unwrap();
        create(
            scratch.path(),
            "Creative",
            &sample_snapshot("dpad", [0.3, 0.7], [0.5, 0.5]),
        )
        .unwrap();
        set_selected(scratch.path(), "layout-2").unwrap();
        assert_eq!(
            state(scratch.path()).unwrap().selected.as_deref(),
            Some("layout-2")
        );
        assert!(
            snapshot(scratch.path(), "nope")
                .unwrap_err()
                .contains("no longer exists")
        );
        delete(scratch.path(), "layout-2").unwrap();
        // The selection went with the deleted profile; Travel stays unselected.
        assert_eq!(state(scratch.path()).unwrap().selected, None);
        assert!(state(scratch.path()).unwrap().selected_name.is_none());
        delete(scratch.path(), "layout-1").unwrap();
        assert_eq!(state(scratch.path()).unwrap().selected, None);
        assert!(
            delete(scratch.path(), "layout-1")
                .unwrap_err()
                .contains("no longer exists")
        );
        clear_selected(scratch.path()).unwrap();
    }

    #[test]
    fn a_missing_selection_is_reported_as_a_warning() {
        let scratch = Scratch::new();
        fs::write(
            scratch.path().join(STORE_FILE),
            r#"{"selected":"layout-9","custom":[{"id":"layout-1","name":"Travel","snapshot":{"mode":"dpad"}}]}"#,
        )
        .unwrap();
        let missing_state = state(scratch.path()).unwrap();
        assert!(missing_state.warning.unwrap().contains("no longer exists"));
    }

    #[test]
    fn a_corrupt_store_is_backed_up_before_the_next_change() {
        let scratch = Scratch::new();
        let path = scratch.path().join(STORE_FILE);
        fs::write(&path, "{not json").unwrap();
        let corrupt_state = state(scratch.path()).unwrap();
        assert!(corrupt_state.warning.is_some());
        create(
            scratch.path(),
            "Travel",
            &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]),
        )
        .unwrap();
        assert!(
            scratch
                .path()
                .join(format!("{STORE_FILE}.corrupt"))
                .exists()
        );
        assert_eq!(state(scratch.path()).unwrap().warning, None);
    }

    #[test]
    fn an_invalid_snapshot_never_becomes_a_profile() {
        let scratch = Scratch::new();
        let mut snapshot = sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]);
        snapshot["layouts"][0][2]["size"] = json!(1.5);
        valid_snapshot(&snapshot).unwrap_err();
        assert!(create(scratch.path(), "Broken", &snapshot).is_err());
        assert!(state(scratch.path()).unwrap().profiles.is_empty());
        assert!(save_current(scratch.path(), "layout-1", &snapshot).is_err());
    }

    #[test]
    fn applying_merges_custom_buttons_by_key_and_replaces_the_rest() {
        let snapshot = sample_snapshot("dpad", [0.35, 0.65], [0.85, 0.45]);
        let mut current = engine_current("tap", Some(serde_json::json!([0.7, 0.9])));
        current["custom_buttons"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "key": "key.keyboard.b",
                "layouts": [
                    {"center": [0.1, 0.9], "size": 0.105, "opacity": 0.65},
                    {"center": [0.1, 0.9], "size": 0.105, "opacity": 0.65},
                    {"center": [0.1, 0.9], "size": 0.105, "opacity": 0.65},
                ],
            }));
        let fields = apply_fields(&current, &snapshot).unwrap();
        assert_eq!(fields["mode"], "dpad");
        // Jump is at the profile's position.
        assert_eq!(fields["layouts"][0][2]["center"], json!([0.35, 0.65]));
        // Visibility comes from the profile.
        assert_eq!(fields["button_visibility"], json!(vec![0; CONTROLS]));
        let buttons = fields["custom_buttons"].as_array().unwrap();
        assert_eq!(buttons.len(), 2);
        assert_eq!(buttons[0]["key"], "key.keyboard.z");
        assert_eq!(buttons[0]["layouts"][0]["center"], json!([0.85, 0.45]));
        // A key added after the profile was saved keeps its placement.
        assert_eq!(buttons[1]["key"], "key.keyboard.b");
        assert_eq!(buttons[1]["layouts"][0]["center"], json!([0.1, 0.9]));
    }

    #[test]
    fn reset_restores_default_layouts_while_keeping_mode_and_custom_keys() {
        let mut current = engine_current("tap", Some(serde_json::json!([0.7, 0.9])));
        current["custom_buttons"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "key": "key.keyboard.b",
                "layouts": [
                    {"center": [0.1, 0.9], "size": 0.105, "opacity": 0.65},
                    {"center": [0.1, 0.9], "size": 0.105, "opacity": 0.65},
                    {"center": [0.1, 0.9], "size": 0.105, "opacity": 0.65},
                ],
            }));
        let defaults = engine_current("crosshair", None);
        let fields = reset_fields(&current, &defaults).unwrap();
        assert_eq!(fields["mode"], "tap");
        // The rows are the engine's default row shape (same 14 layout structure).
        assert_eq!(fields["layouts"].as_array().unwrap().len(), 3);
        for control in 0..3 {
            for row in 0..3 {
                assert_eq!(
                    fields["layouts"][row][control]["center"],
                    defaults["layouts"][0][control]["center"]
                );
            }
        }
        assert_eq!(fields["button_visibility"], json!(vec![0; CONTROLS]));
        // Custom keys survive; every placement back at the factory grid by index.
        let buttons = fields["custom_buttons"].as_array().unwrap();
        assert_eq!(buttons.len(), 2);
        assert_eq!(buttons[0]["key"], "key.keyboard.z");
        assert_eq!(buttons[0]["layouts"][0]["center"], json!([0.68, 0.35]));
        assert_eq!(buttons[1]["key"], "key.keyboard.b");
        assert_eq!(buttons[1]["layouts"][2]["center"], json!([0.78, 0.35]));
    }

    #[test]
    fn mode_labels_match_the_engine_modes() {
        assert_eq!(mode_label("crosshair"), "Joystick & aim crosshair");
        assert_eq!(mode_label("dpad"), "D-Pad & tap to interact");
        assert_eq!(mode_label("other"), "Unknown mode");
    }

    #[test]
    fn summary_reports_the_profile_count_and_the_matching_one() {
        let scratch = Scratch::new();
        assert_eq!(summary(scratch.path()), "No layout profiles yet");
        create(
            scratch.path(),
            "Travel",
            &sample_snapshot("crosshair", [0.3, 0.7], [0.5, 0.5]),
        )
        .unwrap();
        create(
            scratch.path(),
            "Creative",
            &sample_snapshot("dpad", [0.3, 0.7], [0.5, 0.5]),
        )
        .unwrap();
        // The second profile became the selection when it was created.
        assert_eq!(summary(scratch.path()), "2 profile(s) · \"Creative\"");
        delete(scratch.path(), "layout-2").unwrap();
        assert_eq!(summary(scratch.path()), "1 profile(s)");
    }
}
