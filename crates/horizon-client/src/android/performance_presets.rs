//! Per-instance performance presets for the Android launcher.
//!
//! The launcher owns only the preset names and which preset was last applied.
//! Applying a preset writes the existing engine keys in that instance's
//! `options.txt` and `horizon-graphics.txt`; every other line is left alone.
//! The table below is the complete set the launcher exposes: it deliberately
//! omits settings whose engine support is not present on Android, and it uses
//! the engine's own key names, ranges and defaults.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Launcher metadata, kept beside the instance's saves and engine settings.
pub const STORE_FILE: &str = "android-performance.json";
/// The engine's vanilla options file in an instance.
pub const OPTIONS_FILE: &str = "options.txt";
/// Horizon's own graphics settings file in an instance.
pub const GRAPHICS_FILE: &str = "horizon-graphics.txt";
/// The longest custom preset name the launcher stores.
pub const MAX_NAME: usize = 32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Text(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetFile {
    Options,
    Graphics,
}

#[derive(Clone, Copy, Debug)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Int {
        min: i64,
        max: i64,
        step: i64,
    },
    Bool,
    /// A fixed list. `named` matches vanilla's enum options, which options.txt
    /// stores as quoted strings; numeric ids (like `particles`) are plain numbers.
    Choice {
        options: &'static [Choice],
        named: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Setting {
    pub id: &'static str,
    pub file: TargetFile,
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: Kind,
    /// The engine default, in the file's text form. Parsed on use so the table
    /// stays const and never holds an allocated value.
    pub default: &'static str,
    pub restart: bool,
    pub help: &'static str,
    pub unit: &'static str,
}

const CLOUDS: &[Choice] = &[
    Choice {
        value: "false",
        label: "Off",
    },
    Choice {
        value: "fast",
        label: "Fast",
    },
    Choice {
        value: "true",
        label: "Fancy",
    },
];
const PARTICLES: &[Choice] = &[
    Choice {
        value: "0",
        label: "All",
    },
    Choice {
        value: "1",
        label: "Decreased",
    },
    Choice {
        value: "2",
        label: "Minimal",
    },
];
const LOD: &[Choice] = &[
    Choice {
        value: "0",
        label: "Off",
    },
    Choice {
        value: "64",
        label: "64 chunks",
    },
    Choice {
        value: "128",
        label: "128 chunks",
    },
    Choice {
        value: "256",
        label: "256 chunks",
    },
    Choice {
        value: "512",
        label: "512 chunks",
    },
];

const fn int(
    id: &'static str,
    key: &'static str,
    label: &'static str,
    group: &'static str,
    min: i64,
    max: i64,
    step: i64,
    default: &'static str,
    help: &'static str,
    unit: &'static str,
) -> Setting {
    Setting {
        id,
        file: TargetFile::Options,
        key,
        label,
        group,
        kind: Kind::Int { min, max, step },
        default,
        restart: false,
        help,
        unit,
    }
}

const fn graphics_int(
    id: &'static str,
    key: &'static str,
    label: &'static str,
    group: &'static str,
    min: i64,
    max: i64,
    step: i64,
    default: &'static str,
    restart: bool,
    help: &'static str,
    unit: &'static str,
) -> Setting {
    Setting {
        id,
        file: TargetFile::Graphics,
        key,
        label,
        group,
        kind: Kind::Int { min, max, step },
        default,
        restart,
        help,
        unit,
    }
}

const fn boolean(
    id: &'static str,
    key: &'static str,
    label: &'static str,
    group: &'static str,
    default: bool,
    help: &'static str,
) -> Setting {
    Setting {
        id,
        file: TargetFile::Options,
        key,
        label,
        group,
        kind: Kind::Bool,
        default: if default { "true" } else { "false" },
        restart: false,
        help,
        unit: "",
    }
}

const fn graphics_bool(
    id: &'static str,
    key: &'static str,
    label: &'static str,
    group: &'static str,
    default: bool,
    help: &'static str,
) -> Setting {
    Setting {
        id,
        file: TargetFile::Graphics,
        key,
        label,
        group,
        kind: Kind::Bool,
        default: if default { "true" } else { "false" },
        restart: false,
        help,
        unit: "",
    }
}

const fn choice(
    id: &'static str,
    key: &'static str,
    label: &'static str,
    group: &'static str,
    options: &'static [Choice],
    named: bool,
    default: &'static str,
    help: &'static str,
) -> Setting {
    Setting {
        id,
        file: TargetFile::Options,
        key,
        label,
        group,
        kind: Kind::Choice { options, named },
        default,
        restart: false,
        help,
        unit: "",
    }
}

const SETTINGS: &[Setting] = &[
    int(
        "render_distance",
        "renderDistance",
        "Render distance",
        "Distance",
        2,
        32,
        1,
        "12",
        "Full-detail chunks drawn around the player. Lower values load and draw less terrain.",
        "chunks",
    ),
    int(
        "simulation_distance",
        "simulationDistance",
        "Simulation distance",
        "Distance",
        5,
        32,
        1,
        "12",
        "Chunks around the player that keep simulating. Lower values reduce server and CPU work.",
        "chunks",
    ),
    int(
        "max_fps",
        "maxFps",
        "Frame rate limit",
        "Frame rate",
        10,
        260,
        10,
        "120",
        "The game's frame cap. 260 is the engine's Unlimited value; the display can still limit frames.",
        "FPS",
    ),
    graphics_int(
        "resolution_scale",
        "resolutionScale",
        "Resolution scale",
        "Resolution",
        50,
        200,
        5,
        "100",
        false,
        "World pixels relative to the window. Lower values cost less GPU time and look softer.",
        "%",
    ),
    choice(
        "render_clouds",
        "renderClouds",
        "Clouds",
        "Effects",
        CLOUDS,
        true,
        "true",
        "Cloud rendering. Off is the cheapest; Fast uses simpler clouds.",
    ),
    choice(
        "particles",
        "particles",
        "Particles",
        "Effects",
        PARTICLES,
        false,
        "0",
        "Particle density. Minimal removes most decorative particles.",
    ),
    boolean(
        "entity_shadows",
        "entityShadows",
        "Entity shadows",
        "Effects",
        true,
        "Shadow blobs under entities. Off skips that pass.",
    ),
    boolean(
        "smooth_lighting",
        "ao",
        "Smooth lighting",
        "Effects",
        true,
        "Vanilla's ambient occlusion (ao). Off uses simpler lighting.",
    ),
    int(
        "biome_blend",
        "biomeBlendRadius",
        "Biome blend",
        "Effects",
        0,
        7,
        1,
        "2",
        "Blends colours across biome borders. 0 is off.",
        "",
    ),
    int(
        "mipmap_levels",
        "mipmapLevels",
        "Mipmap levels",
        "Effects",
        0,
        4,
        1,
        "4",
        "Texture mipmaps. Fewer levels use less memory but add distant shimmering.",
        "",
    ),
    Setting {
        id: "lod_distance",
        file: TargetFile::Graphics,
        key: "lodDistance",
        label: "Distant terrain",
        group: "Distance",
        kind: Kind::Choice {
            options: LOD,
            named: false,
        },
        default: "0",
        restart: false,
        help: "Horizon's level-of-detail terrain. Off disables the extra distant surface.",
        unit: "",
    },
    graphics_bool(
        "cave_culling",
        "caveCulling",
        "Cave culling",
        "Culling",
        true,
        "Skip sections hidden behind closed ones.",
    ),
    graphics_bool(
        "entity_culling",
        "entityCulling",
        "Entity culling",
        "Culling",
        false,
        "Skip entities hidden behind opaque terrain.",
    ),
    graphics_bool(
        "fxaa",
        "fxaa",
        "FXAA",
        "Anti-aliasing",
        false,
        "A cheap post-process anti-aliasing pass. TAA takes precedence while it is on.",
    ),
    graphics_bool(
        "taa",
        "taa",
        "TAA",
        "Anti-aliasing",
        false,
        "Temporal anti-aliasing. Turning it on makes the game ignore FXAA's setting.",
    ),
    graphics_int(
        "workers",
        "workers",
        "Worker threads",
        "CPU & power",
        0,
        64,
        1,
        "0",
        true,
        "Chunk generation and meshing threads. 0 lets the engine choose; a restart is required.",
        "",
    ),
    graphics_bool(
        "battery_mode",
        "batteryMode",
        "Battery mode",
        "CPU & power",
        false,
        "Caps frames at 60 in a world and 30 elsewhere, and halves worker threads.",
    ),
];

/// Every setting the launcher may write, in display order.
pub fn setting(id: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|setting| setting.id == id)
}

#[derive(Clone, Debug)]
pub struct Builtin {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    /// Values in file text form; parsed through each setting's type on use.
    pub values: &'static [(&'static str, &'static str)],
}

static BATTERY_VALUES: &[(&str, &str)] = &[
    ("render_distance", "6"),
    ("simulation_distance", "5"),
    ("max_fps", "30"),
    ("resolution_scale", "50"),
    ("render_clouds", "false"),
    ("particles", "2"),
    ("entity_shadows", "false"),
    ("smooth_lighting", "false"),
    ("biome_blend", "0"),
    ("mipmap_levels", "0"),
    ("lod_distance", "0"),
    ("cave_culling", "true"),
    ("entity_culling", "true"),
    ("fxaa", "false"),
    ("taa", "false"),
    ("workers", "0"),
    ("battery_mode", "true"),
];
static BALANCED_VALUES: &[(&str, &str)] = &[
    ("render_distance", "12"),
    ("simulation_distance", "12"),
    ("max_fps", "60"),
    ("resolution_scale", "100"),
    ("render_clouds", "true"),
    ("particles", "0"),
    ("entity_shadows", "true"),
    ("smooth_lighting", "true"),
    ("biome_blend", "2"),
    ("mipmap_levels", "4"),
    ("lod_distance", "0"),
    ("cave_culling", "true"),
    ("entity_culling", "true"),
    ("fxaa", "false"),
    ("taa", "false"),
    ("workers", "0"),
    ("battery_mode", "false"),
];
static PERFORMANCE_VALUES: &[(&str, &str)] = &[
    ("render_distance", "8"),
    ("simulation_distance", "6"),
    ("max_fps", "120"),
    ("resolution_scale", "75"),
    ("render_clouds", "fast"),
    ("particles", "1"),
    ("entity_shadows", "false"),
    ("smooth_lighting", "false"),
    ("biome_blend", "1"),
    ("mipmap_levels", "2"),
    ("lod_distance", "0"),
    ("cave_culling", "true"),
    ("entity_culling", "true"),
    ("fxaa", "false"),
    ("taa", "false"),
    ("workers", "0"),
    ("battery_mode", "false"),
];

pub static BUILTINS: &[Builtin] = &[
    Builtin {
        id: "battery",
        name: "Battery Saver",
        summary: "Lowest detail, 30 FPS cap and battery mode for long sessions.",
        values: BATTERY_VALUES,
    },
    Builtin {
        id: "balanced",
        name: "Balanced",
        summary: "A default-like balance of distance, detail and a 60 FPS cap.",
        values: BALANCED_VALUES,
    },
    Builtin {
        id: "performance",
        name: "Performance",
        summary: "Lower distance, effects and resolution with a higher frame cap.",
        values: PERFORMANCE_VALUES,
    },
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomPreset {
    pub id: String,
    pub name: String,
    pub values: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Store {
    selected: Option<String>,
    custom: Vec<CustomPreset>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum KindView {
    Int { min: i64, max: i64, step: i64 },
    Bool,
    Choice { options: Vec<ChoiceView> },
}

#[derive(Clone, Debug, Serialize)]
pub struct ChoiceView {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SettingView {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: KindView,
    pub default: Value,
    pub current: Value,
    pub restart: bool,
    pub help: &'static str,
    pub unit: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct PresetView {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub builtin: bool,
    pub values: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct State {
    pub instance: String,
    pub selected: Option<String>,
    pub selected_name: Option<String>,
    pub presets: Vec<PresetView>,
    pub settings: Vec<SettingView>,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Change {
    pub id: &'static str,
    pub label: &'static str,
    pub current: Value,
    pub target: Value,
    pub restart: bool,
    pub unit: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct Preview {
    pub changes: Vec<Change>,
    pub restart: bool,
    pub no_changes: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApplyResult {
    pub name: String,
    pub changes: usize,
    pub restart: bool,
    pub selected: Option<String>,
}

fn instance_name(instance: &Path) -> String {
    instance
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| instance.display().to_string())
}

fn read_optional(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("Cannot read {}: {error}", path.display())),
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

fn load_store(instance: &Path) -> (Store, Option<String>) {
    let path = instance.join(STORE_FILE);
    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<Store>(&bytes) {
            Ok(store) => (store, None),
            Err(error) => (
                Store::default(),
                Some(format!(
                    "Custom performance presets could not be read ({error}). The file is kept; a backup is made before the next change."
                )),
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Store::default(), None),
        Err(error) => (
            Store::default(),
            Some(format!(
                "Custom performance presets could not be read: {error}"
            )),
        ),
    }
}

fn save_store(instance: &Path, store: &Store, damaged: bool) -> Result<(), String> {
    let path = instance.join(STORE_FILE);
    if damaged && path.exists() {
        let backup = instance.join(format!("{STORE_FILE}.corrupt"));
        if !backup.exists() {
            fs::copy(&path, &backup)
                .map_err(|e| format!("Cannot back up the unreadable preset file: {e}"))?;
        }
    }
    let text = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    atomic_write(&path, &text)
}

fn value_from_text(setting: &Setting, text: &str) -> Value {
    let text = text.trim();
    match &setting.kind {
        Kind::Int { .. } => text
            .parse::<i64>()
            .map(Value::Int)
            .unwrap_or_else(|_| default_value(setting)),
        Kind::Bool => match text {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => default_value(setting),
        },
        Kind::Choice { options, .. } => {
            let text = text
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(text);
            options
                .iter()
                .find(|choice| choice.value == text)
                .map(|choice| Value::Text(choice.value.to_owned()))
                .unwrap_or_else(|| default_value(setting))
        }
    }
}

/// The engine default as a typed value.
fn default_value(setting: &Setting) -> Value {
    value_from_text(setting, setting.default)
}

fn value_text(setting: &Setting, value: &Value) -> Result<String, String> {
    match (&setting.kind, value) {
        (Kind::Int { .. }, Value::Int(number)) => Ok(number.to_string()),
        (Kind::Bool, Value::Bool(on)) => Ok(on.to_string()),
        (Kind::Choice { named: true, .. }, Value::Text(text)) => {
            // options.txt stores named options as JSON strings.
            Ok(format!("\"{text}\""))
        }
        (Kind::Choice { named: false, .. }, Value::Text(text)) => Ok(text.clone()),
        (Kind::Int { .. }, _) => Err(format!("{} needs a number", setting.label)),
        (Kind::Bool, _) => Err(format!("{} needs On or Off", setting.label)),
        (Kind::Choice { .. }, _) => Err(format!("{} needs one of its choices", setting.label)),
    }
}

fn graphics_text(setting: &Setting, value: &Value) -> Result<String, String> {
    match (&setting.kind, value) {
        (Kind::Int { .. }, Value::Int(number)) => Ok(number.to_string()),
        (Kind::Bool, Value::Bool(on)) => Ok(on.to_string()),
        (Kind::Choice { .. }, Value::Text(text)) => Ok(text.clone()),
        (Kind::Int { .. }, _) => Err(format!("{} needs a number", setting.label)),
        (Kind::Bool, _) => Err(format!("{} needs On or Off", setting.label)),
        (Kind::Choice { .. }, _) => Err(format!("{} needs one of its choices", setting.label)),
    }
}

fn current_values(instance: &Path) -> Result<BTreeMap<String, Value>, String> {
    let options = read_optional(&instance.join(OPTIONS_FILE))?;
    let graphics = read_optional(&instance.join(GRAPHICS_FILE))?;
    let options = last_values(&options);
    let graphics = last_values(&graphics);
    let mut current = BTreeMap::new();
    for setting in SETTINGS {
        let raw = match setting.file {
            TargetFile::Options => options.get(setting.key),
            TargetFile::Graphics => graphics.get(setting.key),
        };
        current.insert(
            setting.id.to_owned(),
            raw.map_or_else(
                || default_value(setting),
                |text| value_from_text(setting, text),
            ),
        );
    }
    Ok(current)
}

fn last_values(text: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        values.insert(key.trim().to_owned(), value.trim().to_owned());
    }
    values
}

fn normalize(setting: &Setting, value: &Value) -> Result<Value, String> {
    match (&setting.kind, value) {
        (Kind::Int { min, max, step }, Value::Int(number)) => {
            if number < min || number > max {
                return Err(format!("{} must be between {min} and {max}", setting.label));
            }
            if (number - min) % step != 0 {
                return Err(format!("{} must use steps of {step}", setting.label));
            }
            Ok(Value::Int(*number))
        }
        (Kind::Bool, Value::Bool(on)) => Ok(Value::Bool(*on)),
        (Kind::Choice { options, .. }, Value::Text(text)) => {
            let text = text.trim();
            let text = text
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(text);
            let choice = options
                .iter()
                .find(|choice| choice.value == text)
                .ok_or_else(|| format!("{} is not a supported choice", setting.label))?;
            Ok(Value::Text(choice.value.to_owned()))
        }
        (Kind::Int { .. }, _) => Err(format!("{} needs a number", setting.label)),
        (Kind::Bool, _) => Err(format!("{} needs On or Off", setting.label)),
        (Kind::Choice { .. }, _) => Err(format!("{} needs one of its choices", setting.label)),
    }
}

fn builtin_values(builtin: &Builtin) -> BTreeMap<String, Value> {
    let mut values = BTreeMap::new();
    for (id, text) in builtin.values {
        if let Some(setting) = setting(id) {
            values.insert((*id).to_owned(), value_from_text(setting, text));
        }
    }
    values
}

fn custom_values(preset: &CustomPreset) -> Result<BTreeMap<String, Value>, String> {
    let mut values = BTreeMap::new();
    for (id, value) in &preset.values {
        let setting = setting(id).ok_or_else(|| {
            format!(
                "\"{}\" uses a setting this build does not support ({id}); edit it and save again",
                preset.name
            )
        })?;
        values.insert(id.clone(), normalize(setting, value)?);
    }
    if values.is_empty() {
        return Err(format!("\"{}\" does not set any values", preset.name));
    }
    Ok(values)
}

fn preset_values(store: &Store, id: &str) -> Result<(String, BTreeMap<String, Value>), String> {
    if let Some(builtin) = BUILTINS.iter().find(|builtin| builtin.id == id) {
        return Ok((builtin.name.to_owned(), builtin_values(builtin)));
    }
    let preset = store
        .custom
        .iter()
        .find(|preset| preset.id == id)
        .ok_or_else(|| "That performance preset no longer exists".to_owned())?;
    Ok((preset.name.clone(), custom_values(preset)?))
}

fn changes(current: &BTreeMap<String, Value>, targets: &BTreeMap<String, Value>) -> Vec<Change> {
    SETTINGS
        .iter()
        .filter_map(|setting| {
            let target = targets.get(setting.id)?;
            let before = current
                .get(setting.id)
                .cloned()
                .unwrap_or_else(|| default_value(setting));
            (before != *target).then(|| Change {
                id: setting.id,
                label: setting.label,
                current: before,
                target: target.clone(),
                restart: setting.restart,
                unit: setting.unit,
            })
        })
        .collect()
}

fn preview_for(
    instance: &Path,
    store: &Store,
    id: &str,
) -> Result<(String, BTreeMap<String, Value>, Preview), String> {
    let (name, targets) = preset_values(store, id)?;
    let current = current_values(instance)?;
    let mut changes = changes(&current, &targets);
    // These presets are not one of vanilla's fast/fancy/fabulous combinations,
    // so the game's own preset label must not keep claiming a preset it no
    // longer describes. The change is shown with the others, not hidden.
    let engine = engine_preset(instance)?;
    if engine != "custom" {
        changes.push(Change {
            id: "engine_graphics_preset",
            label: "Game graphics preset label",
            current: Value::Text(engine),
            target: Value::Text("custom".into()),
            restart: false,
            unit: "",
        });
    }
    let restart = changes.iter().any(|change| change.restart);
    Ok((
        name,
        targets,
        Preview {
            no_changes: changes.is_empty(),
            changes,
            restart,
        },
    ))
}

fn engine_preset(instance: &Path) -> Result<String, String> {
    let options = read_optional(&instance.join(OPTIONS_FILE))?;
    let values = last_values(&options);
    let raw = values.get("graphicsPreset").map_or("fancy", String::as_str);
    Ok(raw
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(raw)
        .to_owned())
}

fn patch_settings(instance: &Path, targets: &BTreeMap<String, Value>) -> Result<(), String> {
    let mut options = read_optional(&instance.join(OPTIONS_FILE))?;
    let mut graphics = read_optional(&instance.join(GRAPHICS_FILE))?;
    let mut options_changed = false;
    let mut graphics_changed = false;
    for (id, value) in targets {
        let setting = setting(id).ok_or_else(|| format!("Unknown performance setting {id}"))?;
        let value = normalize(setting, value)?;
        match setting.file {
            TargetFile::Options => {
                let rendered = value_text(setting, &value)?;
                options = patch_line(&options, setting.key, &rendered);
                options_changed = true;
            }
            TargetFile::Graphics => {
                let rendered = graphics_text(setting, &value)?;
                graphics = patch_line(&graphics, setting.key, &rendered);
                graphics_changed = true;
            }
        }
    }
    if !targets.is_empty() {
        options = patch_line(&options, "graphicsPreset", "\"custom\"");
        options_changed = true;
    }
    if options_changed {
        atomic_write(&instance.join(OPTIONS_FILE), &options)?;
    }
    if graphics_changed {
        atomic_write(&instance.join(GRAPHICS_FILE), &graphics)?;
    }
    Ok(())
}

fn patch_line(text: &str, key: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut found = false;
    let mut out = String::with_capacity(text.len() + key.len() + value.len() + 2);
    for line in text.split_inclusive('\n') {
        let (body, line_end) = if let Some(body) = line.strip_suffix("\r\n") {
            (body, "\r\n")
        } else if let Some(body) = line.strip_suffix('\n') {
            (body, "\n")
        } else {
            (line, "")
        };
        if body
            .split_once(':')
            .is_some_and(|(line_key, _)| line_key.trim() == key)
        {
            out.push_str(key);
            out.push(':');
            out.push_str(value);
            out.push_str(line_end);
            found = true;
        } else {
            out.push_str(line);
        }
    }
    if !found {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(newline);
        }
        out.push_str(key);
        out.push(':');
        out.push_str(value);
        out.push_str(newline);
    }
    out
}

pub fn preview(instance: &Path, id: &str) -> Result<Preview, String> {
    let (store, _) = load_store(instance);
    preview_for(instance, &store, id).map(|(_, _, preview)| preview)
}

pub fn apply(instance: &Path, id: &str) -> Result<ApplyResult, String> {
    let (mut store, damaged) = load_store(instance);
    let (name, targets, preview) = preview_for(instance, &store, id)?;
    patch_settings(instance, &targets)?;
    store.selected = Some(id.to_owned());
    save_store(instance, &store, damaged.is_some())?;
    Ok(ApplyResult {
        name,
        changes: preview.changes.len(),
        restart: preview.restart,
        selected: store.selected,
    })
}

pub fn reset_defaults(instance: &Path) -> Result<ApplyResult, String> {
    let (mut store, damaged) = load_store(instance);
    let current = current_values(instance)?;
    let targets: BTreeMap<String, Value> = SETTINGS
        .iter()
        .map(|setting| (setting.id.to_owned(), default_value(setting)))
        .collect();
    let changes = changes(&current, &targets);
    patch_settings(instance, &targets)?;
    store.selected = None;
    save_store(instance, &store, damaged.is_some())?;
    Ok(ApplyResult {
        name: "Engine defaults".into(),
        changes: changes.len(),
        restart: changes.iter().any(|change| change.restart),
        selected: None,
    })
}

fn clean_name(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect()
}

fn name_in_use(store: &Store, skip: Option<&str>, name: &str) -> bool {
    let wanted = name.trim().to_lowercase();
    BUILTINS
        .iter()
        .any(|builtin| builtin.name.to_lowercase() == wanted)
        || store.custom.iter().any(|preset| {
            Some(preset.id.as_str()) != skip && preset.name.trim().to_lowercase() == wanted
        })
}

fn validated_name(store: &Store, skip: Option<&str>, raw: &str) -> Result<String, String> {
    let name = clean_name(raw);
    if name.is_empty() {
        return Err("Give the preset a name".into());
    }
    if name_in_use(store, skip, &name) {
        return Err(format!(
            "Another preset is already called \"{name}\". Choose a different name."
        ));
    }
    Ok(name)
}

fn unique_name(store: &Store, base: &str) -> String {
    let base = clean_name(base);
    let base = if base.is_empty() { "Preset" } else { &base };
    if !name_in_use(store, None, base) {
        return base.to_owned();
    }
    for n in 2..=999 {
        let suffix = format!(" {n}");
        let keep = MAX_NAME.saturating_sub(suffix.chars().count());
        let stem: String = base.chars().take(keep).collect();
        let candidate = format!("{}{suffix}", stem.trim_end());
        if !name_in_use(store, None, &candidate) {
            return candidate;
        }
    }
    "Preset".to_owned()
}

fn next_id(store: &Store) -> String {
    let mut next = 1u64;
    for preset in &store.custom {
        if let Some(number) = preset
            .id
            .strip_prefix("custom-")
            .and_then(|n| n.parse::<u64>().ok())
        {
            next = next.max(number + 1);
        }
    }
    format!("custom-{next}")
}

fn validated_values(values: &BTreeMap<String, Value>) -> Result<BTreeMap<String, Value>, String> {
    if values.is_empty() {
        return Err("Choose at least one setting".into());
    }
    let mut clean = BTreeMap::new();
    for (id, value) in values {
        let setting = setting(id).ok_or_else(|| format!("Unknown performance setting {id}"))?;
        clean.insert(id.clone(), normalize(setting, value)?);
    }
    Ok(clean)
}

fn preset_view(preset: &CustomPreset) -> PresetView {
    PresetView {
        id: preset.id.clone(),
        name: preset.name.clone(),
        summary: "Custom preset".into(),
        builtin: false,
        values: preset.values.clone(),
    }
}

pub fn state(instance: &Path) -> Result<State, String> {
    let (store, warning) = load_store(instance);
    let current = current_values(instance)?;
    let mut presets = Vec::new();
    for builtin in BUILTINS {
        presets.push(PresetView {
            id: builtin.id.to_owned(),
            name: builtin.name.to_owned(),
            summary: builtin.summary.to_owned(),
            builtin: true,
            values: builtin_values(builtin),
        });
    }
    presets.extend(store.custom.iter().map(preset_view));
    let selected_name = store.selected.as_deref().and_then(|id| {
        presets
            .iter()
            .find(|preset| preset.id == id)
            .map(|preset| preset.name.clone())
    });
    let invalid_custom = store
        .custom
        .iter()
        .find_map(|preset| validated_values(&preset.values).err());
    let settings = SETTINGS
        .iter()
        .map(|setting| SettingView {
            id: setting.id,
            label: setting.label,
            group: setting.group,
            kind: match &setting.kind {
                Kind::Int { min, max, step } => KindView::Int {
                    min: *min,
                    max: *max,
                    step: *step,
                },
                Kind::Bool => KindView::Bool,
                Kind::Choice { options, .. } => KindView::Choice {
                    options: options
                        .iter()
                        .map(|choice| ChoiceView {
                            value: choice.value.to_owned(),
                            label: choice.label.to_owned(),
                        })
                        .collect(),
                },
            },
            default: default_value(setting),
            current: current
                .get(setting.id)
                .cloned()
                .unwrap_or_else(|| default_value(setting)),
            restart: setting.restart,
            help: setting.help,
            unit: setting.unit,
        })
        .collect();
    let warning = warning
        .or_else(|| {
            invalid_custom.map(|error| format!("A custom performance preset is invalid: {error}"))
        })
        .or_else(|| {
            store.selected.as_deref().and_then(|id| {
                (!presets.iter().any(|preset| preset.id == id))
                    .then(|| "The selected performance preset no longer exists.".to_owned())
            })
        });
    Ok(State {
        instance: instance_name(instance),
        selected: store.selected,
        selected_name,
        presets,
        settings,
        warning,
    })
}

pub fn selected_label(instance: &Path) -> Option<String> {
    let (store, _) = load_store(instance);
    let id = store.selected.as_deref()?;
    BUILTINS
        .iter()
        .find(|builtin| builtin.id == id)
        .map(|builtin| builtin.name.to_owned())
        .or_else(|| {
            store
                .custom
                .iter()
                .find(|preset| preset.id == id)
                .map(|preset| preset.name.clone())
        })
}

pub fn create_custom(
    instance: &Path,
    name: &str,
    values: &BTreeMap<String, Value>,
) -> Result<PresetView, String> {
    let (mut store, damaged) = load_store(instance);
    let name = validated_name(&store, None, name)?;
    let values = validated_values(values)?;
    let preset = CustomPreset {
        id: next_id(&store),
        name,
        values,
    };
    store.custom.push(preset.clone());
    save_store(instance, &store, damaged.is_some())?;
    Ok(preset_view(&preset))
}

pub fn update_custom(
    instance: &Path,
    id: &str,
    name: &str,
    values: &BTreeMap<String, Value>,
) -> Result<PresetView, String> {
    let (mut store, damaged) = load_store(instance);
    let index = store
        .custom
        .iter()
        .position(|preset| preset.id == id)
        .ok_or("Only custom presets can be edited")?;
    let name = validated_name(&store, Some(id), name)?;
    let values = validated_values(values)?;
    store.custom[index].name = name;
    store.custom[index].values = values;
    let preset = store.custom[index].clone();
    save_store(instance, &store, damaged.is_some())?;
    Ok(preset_view(&preset))
}

pub fn rename_custom(instance: &Path, id: &str, name: &str) -> Result<PresetView, String> {
    let (mut store, damaged) = load_store(instance);
    let index = store
        .custom
        .iter()
        .position(|preset| preset.id == id)
        .ok_or("Only custom presets can be renamed")?;
    let name = validated_name(&store, Some(id), name)?;
    store.custom[index].name = name;
    let preset = store.custom[index].clone();
    save_store(instance, &store, damaged.is_some())?;
    Ok(preset_view(&preset))
}

pub fn duplicate_custom(instance: &Path, id: &str) -> Result<PresetView, String> {
    let (mut store, damaged) = load_store(instance);
    let source = store
        .custom
        .iter()
        .find(|preset| preset.id == id)
        .ok_or("Only custom presets can be duplicated")?;
    let name = unique_name(&store, &format!("{} copy", source.name));
    let preset = CustomPreset {
        id: next_id(&store),
        name,
        values: source.values.clone(),
    };
    store.custom.push(preset.clone());
    save_store(instance, &store, damaged.is_some())?;
    Ok(preset_view(&preset))
}

pub fn delete_custom(instance: &Path, id: &str) -> Result<(), String> {
    let (mut store, damaged) = load_store(instance);
    let index = store
        .custom
        .iter()
        .position(|preset| preset.id == id)
        .ok_or("Only custom presets can be deleted")?;
    store.custom.remove(index);
    if store.selected.as_deref() == Some(id) {
        store.selected = None;
    }
    save_store(instance, &store, damaged.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "horizon-performance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
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

    fn values(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(id, value)| ((*id).to_owned(), value.clone()))
            .collect()
    }

    fn option(path: &Path, key: &str) -> String {
        last_values(&fs::read_to_string(path).unwrap())
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn every_builtin_has_every_setting_once() {
        for builtin in BUILTINS {
            assert_eq!(builtin.values.len(), SETTINGS.len(), "{}", builtin.name);
            for setting in SETTINGS {
                let text = builtin
                    .values
                    .iter()
                    .find(|(id, _)| *id == setting.id)
                    .map(|(_, value)| *value)
                    .unwrap_or_else(|| panic!("{} lacks {}", builtin.name, setting.id));
                let value = value_from_text(setting, text);
                assert!(normalize(setting, &value).is_ok(), "{}", setting.id);
            }
        }
    }

    #[test]
    fn applying_battery_changes_owned_keys_and_keeps_everything_else() {
        let scratch = Scratch::new();
        fs::write(
            scratch.path().join(OPTIONS_FILE),
            "version:4189\nrenderDistance:20\nmaxFps:240\nsomeFutureOption:42\nlang:fr_fr\n",
        )
        .unwrap();
        fs::write(
            scratch.path().join(GRAPHICS_FILE),
            "resolutionScale:125\nworkers:4\nshaderPack:keep\n",
        )
        .unwrap();
        let result = apply(scratch.path(), "battery").unwrap();
        assert!(result.restart);
        assert_eq!(result.selected.as_deref(), Some("battery"));
        let options = fs::read_to_string(scratch.path().join(OPTIONS_FILE)).unwrap();
        assert!(options.contains("version:4189\n"));
        assert!(options.contains("renderDistance:6\n"));
        assert!(options.contains("maxFps:30\n"));
        assert!(options.contains("someFutureOption:42\n"));
        assert!(options.contains("lang:fr_fr\n"));
        assert!(options.contains("renderClouds:\"false\"\n"));
        assert!(options.contains("particles:2\n"));
        assert!(options.contains("graphicsPreset:\"custom\"\n"));
        let graphics = fs::read_to_string(scratch.path().join(GRAPHICS_FILE)).unwrap();
        assert!(graphics.contains("resolutionScale:50\n"));
        assert!(graphics.contains("workers:0\n"));
        assert!(graphics.contains("shaderPack:keep\n"));
        assert!(graphics.contains("batteryMode:true\n"));
    }

    #[test]
    fn instances_keep_separate_presets_and_engine_values() {
        let a = Scratch::new();
        let b = Scratch::new();
        apply(a.path(), "battery").unwrap();
        apply(b.path(), "performance").unwrap();
        assert_eq!(option(&a.path().join(OPTIONS_FILE), "renderDistance"), "6");
        assert_eq!(option(&b.path().join(OPTIONS_FILE), "renderDistance"), "8");
        assert_eq!(option(&a.path().join(OPTIONS_FILE), "maxFps"), "30");
        assert_eq!(option(&b.path().join(OPTIONS_FILE), "maxFps"), "120");
        assert_eq!(selected_label(a.path()).as_deref(), Some("Battery Saver"));
        assert_eq!(selected_label(b.path()).as_deref(), Some("Performance"));
    }

    #[test]
    fn custom_presets_persist_and_support_crud() {
        let scratch = Scratch::new();
        let created = create_custom(
            scratch.path(),
            "Travel",
            &values(&[
                ("render_distance", Value::Int(10)),
                ("max_fps", Value::Int(90)),
            ]),
        )
        .unwrap();
        assert_eq!(created.name, "Travel");
        let state_value = state(scratch.path()).unwrap();
        assert!(
            state_value
                .presets
                .iter()
                .any(|preset| preset.name == "Travel")
        );
        apply(scratch.path(), &created.id).unwrap();
        assert_eq!(
            option(&scratch.path().join(OPTIONS_FILE), "renderDistance"),
            "10"
        );
        assert_eq!(option(&scratch.path().join(OPTIONS_FILE), "maxFps"), "90");
        let renamed = rename_custom(scratch.path(), &created.id, "Travel light").unwrap();
        assert_eq!(renamed.name, "Travel light");
        let duplicate = duplicate_custom(scratch.path(), &created.id).unwrap();
        assert_eq!(duplicate.name, "Travel light copy");
        delete_custom(scratch.path(), &created.id).unwrap();
        assert!(
            state(scratch.path())
                .unwrap()
                .presets
                .iter()
                .all(|preset| preset.id != created.id)
        );
        assert_eq!(selected_label(scratch.path()), None);
    }

    #[test]
    fn restoring_defaults_only_touches_preset_owned_keys() {
        let scratch = Scratch::new();
        fs::write(
            scratch.path().join(OPTIONS_FILE),
            "renderDistance:6\nlang:fr_fr\nsomeFutureOption:42\n",
        )
        .unwrap();
        fs::write(
            scratch.path().join(GRAPHICS_FILE),
            "workers:4\nshaderPack:keep\n",
        )
        .unwrap();
        reset_defaults(scratch.path()).unwrap();
        assert_eq!(
            option(&scratch.path().join(OPTIONS_FILE), "renderDistance"),
            "12"
        );
        assert_eq!(option(&scratch.path().join(OPTIONS_FILE), "lang"), "fr_fr");
        assert_eq!(
            option(&scratch.path().join(OPTIONS_FILE), "someFutureOption"),
            "42"
        );
        assert_eq!(
            option(&scratch.path().join(OPTIONS_FILE), "graphicsPreset"),
            "\"custom\""
        );
        assert_eq!(option(&scratch.path().join(GRAPHICS_FILE), "workers"), "0");
        assert_eq!(
            option(&scratch.path().join(GRAPHICS_FILE), "shaderPack"),
            "keep"
        );
        assert_eq!(selected_label(scratch.path()), None);
    }

    #[test]
    fn preview_reports_changes_and_the_restart_flag() {
        let scratch = Scratch::new();
        fs::write(scratch.path().join(GRAPHICS_FILE), "workers:4\n").unwrap();
        let preview = preview(scratch.path(), "balanced").unwrap();
        assert!(preview.restart);
        assert!(preview.changes.iter().any(|change| change.id == "workers"));
    }

    #[test]
    fn unsupported_and_out_of_range_values_are_rejected() {
        let scratch = Scratch::new();
        let unknown = values(&[("not_a_setting", Value::Int(1))]);
        assert!(create_custom(scratch.path(), "Bad", &unknown).is_err());
        let range = values(&[("max_fps", Value::Int(999))]);
        assert!(create_custom(scratch.path(), "Bad", &range).is_err());
        let step = values(&[("max_fps", Value::Int(15))]);
        assert!(create_custom(scratch.path(), "Bad", &step).is_err());
        let no_values = BTreeMap::new();
        assert!(create_custom(scratch.path(), "Empty", &no_values).is_err());
    }

    #[test]
    fn a_stored_preset_with_an_unknown_key_is_marked_unavailable() {
        let scratch = Scratch::new();
        fs::write(
            scratch.path().join(STORE_FILE),
            r#"{"selected":"custom-1","custom":[{"id":"custom-1","name":"Old","values":{"not_a_setting":1}}]}"#,
        )
        .unwrap();
        let state = state(scratch.path()).unwrap();
        let warning = state.warning.unwrap();
        assert!(warning.contains("invalid"), "{warning}");
        assert!(preview(scratch.path(), "custom-1").is_err());
    }

    #[test]
    fn malformed_or_missing_engine_values_fall_back_without_touching_other_lines() {
        let scratch = Scratch::new();
        fs::write(
            scratch.path().join(OPTIONS_FILE),
            "renderDistance:not-a-number\nsomeFutureOption:42\n",
        )
        .unwrap();
        let state = state(scratch.path()).unwrap();
        let distance = state
            .settings
            .iter()
            .find(|setting| setting.id == "render_distance")
            .unwrap();
        assert_eq!(distance.current, Value::Int(12));
        apply(scratch.path(), "balanced").unwrap();
        assert_eq!(
            option(&scratch.path().join(OPTIONS_FILE), "renderDistance"),
            "12"
        );
        assert_eq!(
            option(&scratch.path().join(OPTIONS_FILE), "someFutureOption"),
            "42"
        );
    }

    #[test]
    fn crlf_and_duplicate_keys_are_replaced_without_losing_line_endings() {
        let scratch = Scratch::new();
        fs::write(
            scratch.path().join(OPTIONS_FILE),
            "renderDistance:4\r\nrenderDistance:6\r\nlang:fr_fr\r\n",
        )
        .unwrap();
        apply(scratch.path(), "balanced").unwrap();
        let text = fs::read_to_string(scratch.path().join(OPTIONS_FILE)).unwrap();
        assert!(text.contains("renderDistance:12\r\n"));
        assert!(!text.contains("renderDistance:4\r\n"));
        assert!(text.contains("lang:fr_fr\r\n"));
        assert!(
            text.split('\n')
                .filter(|line| !line.is_empty())
                .all(|line| line.ends_with('\r'))
        );
    }

    #[test]
    fn a_corrupt_store_is_backed_up_before_the_next_change() {
        let scratch = Scratch::new();
        let path = scratch.path().join(STORE_FILE);
        fs::write(&path, "{not json").unwrap();
        let state = state(scratch.path()).unwrap();
        assert!(state.warning.is_some());
        apply(scratch.path(), "battery").unwrap();
        assert_eq!(
            fs::read_to_string(scratch.path().join(format!("{STORE_FILE}.corrupt"))).unwrap(),
            "{not json"
        );
    }

    #[test]
    fn custom_names_are_unique_case_insensitively() {
        let scratch = Scratch::new();
        let created = create_custom(
            scratch.path(),
            "My preset",
            &values(&[("render_distance", Value::Int(10))]),
        )
        .unwrap();
        assert!(
            create_custom(
                scratch.path(),
                "my preset",
                &values(&[("render_distance", Value::Int(9))])
            )
            .is_err()
        );
        let duplicate = duplicate_custom(scratch.path(), &created.id).unwrap();
        assert_eq!(duplicate.name, "My preset copy");
    }

    #[test]
    fn partial_custom_presets_only_change_their_own_values() {
        let scratch = Scratch::new();
        fs::write(
            scratch.path().join(OPTIONS_FILE),
            "renderDistance:20\nmaxFps:240\n",
        )
        .unwrap();
        let created = create_custom(
            scratch.path(),
            "Only distance",
            &values(&[("render_distance", Value::Int(9))]),
        )
        .unwrap();
        apply(scratch.path(), &created.id).unwrap();
        assert_eq!(
            option(&scratch.path().join(OPTIONS_FILE), "renderDistance"),
            "9"
        );
        assert_eq!(option(&scratch.path().join(OPTIONS_FILE), "maxFps"), "240");
    }

    #[test]
    fn builtins_cannot_be_renamed_or_deleted() {
        let scratch = Scratch::new();
        assert!(rename_custom(scratch.path(), "battery", "Mine").is_err());
        assert!(delete_custom(scratch.path(), "balanced").is_err());
        assert!(
            update_custom(
                scratch.path(),
                "performance",
                "Mine",
                &values(&[("render_distance", Value::Int(4))])
            )
            .is_err()
        );
    }
}
