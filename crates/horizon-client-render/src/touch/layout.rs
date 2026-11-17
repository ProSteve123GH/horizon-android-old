// Adapted from Starling (https://github.com/starling-client/starling, `android` branch at
// 150a414b09a1, `crates/horizon-client-render/src/touch/layout.rs`), written by Vasysik and the
// Starling contributors. Copyright (c) 2026 Ludovic Marie and the Horizon contributors, under the
// same MIT license as Horizon (LICENSE at the repository root).

//! The touch layout: which on-screen controls exist, where each one sits for
//! each control scheme, and the player's saved choices. Coordinates are
//! normalized to the surface; sizes are fractions of its height, so a button
//! stays square on wide phones and on tablets. Nothing here reads input or
//! draws: this is the file the touch controls and their editor share.
use serde::{Deserialize, Serialize};
use std::path::Path;

/// How the player moves and aims.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// A joystick moves; attack and use act on the crosshair.
    #[default]
    Crosshair,
    /// A joystick moves; tapping the world attacks or uses what was tapped.
    Tap,
    /// A D-pad moves; tapping the world acts on it (or the crosshair, split).
    Dpad,
}

impl Mode {
    pub const ALL: [Self; 3] = [Self::Crosshair, Self::Tap, Self::Dpad];
    /// The four schemes a settings screen offers: mode, `split_controls`, label.
    pub const SCHEMES: [(Self, bool, &'static str); 4] = [
        (Self::Crosshair, false, "Joystick & aim crosshair"),
        (Self::Tap, false, "Joystick & tap to interact"),
        (Self::Dpad, false, "D-Pad & tap to interact"),
        (Self::Dpad, true, "D-Pad & aim crosshair"),
    ];

    /// This mode's row in [`Settings::layouts`].
    pub fn index(self) -> usize {
        self as usize
    }
}

/// The built-in controls. The order is the index into a layout row and into
/// `button_visibility`, so new controls go at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Move,
    Sprint,
    Jump,
    Sneak,
    Attack,
    Use,
    Chat,
    Pause,
    Settings,
    Inventory,
    Debug,
    Perspective,
    Offhand,
    Shield,
}

/// How many built-in controls a layout row holds.
pub const CONTROLS: usize = Control::ALL.len();

impl Control {
    pub const ALL: [Self; 14] = [
        Self::Move,
        Self::Sprint,
        Self::Jump,
        Self::Sneak,
        Self::Attack,
        Self::Use,
        Self::Chat,
        Self::Pause,
        Self::Settings,
        Self::Inventory,
        Self::Debug,
        Self::Perspective,
        Self::Offhand,
        Self::Shield,
    ];

    /// The name the layout editor shows.
    pub fn name(self) -> &'static str {
        match self {
            Self::Move => "Movement",
            Self::Sprint => "Sprint",
            Self::Jump => "Jump / fly up",
            Self::Sneak => "Sneak / fly down",
            Self::Attack => "Attack / mine",
            Self::Use => "Use / place",
            Self::Chat => "Chat",
            Self::Pause => "Pause",
            Self::Settings => "Touch settings",
            Self::Inventory => "Inventory",
            Self::Debug => "Debug overlay (F3)",
            Self::Perspective => "Camera perspective (F5)",
            Self::Offhand => "Swap hands",
            Self::Shield => "Raise shield",
        }
    }
}

/// Where one control sits: its centre (0..1 of the surface), its size (a
/// fraction of the surface height) and its opacity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub center: [f32; 2],
    pub size: f32,
    pub opacity: f32,
}

impl Placement {
    pub const fn new(x: f32, y: f32, size: f32) -> Self {
        Self {
            center: [x, y],
            size,
            opacity: 0.65,
        }
    }

    /// The square control as `[x, y, w, h]` in normalized surface coordinates,
    /// for a surface `aspect` (width / height) wide.
    pub fn rect(self, aspect: f32) -> [f32; 4] {
        let w = self.size / aspect.max(0.1);
        [
            self.center[0] - w / 2.0,
            self.center[1] - self.size / 2.0,
            w,
            self.size,
        ]
    }

    /// Brings a value read from disk back on screen and into range.
    pub fn sanitize(&mut self) {
        self.size = finite_or(self.size, 0.15).clamp(0.045, 0.45);
        self.opacity = finite_or(self.opacity, 0.65).clamp(0.0, 1.0);
        for c in &mut self.center {
            *c = finite_or(*c, 0.5).clamp(0.025, 0.975);
        }
    }
}

/// `button_visibility` values: follow the scheme, always shown, always hidden.
pub const FOLLOW_SCHEME: u8 = 0;
pub const SHOWN: u8 = 1;
pub const HIDDEN: u8 = 2;

/// The player's touch layout, saved as JSON. Missing fields take their
/// defaults, so later options can be added without resetting a saved file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    /// With [`Mode::Dpad`]: attack and use act on the crosshair.
    pub split_controls: bool,
    /// Mirrors every control left to right.
    pub left_handed: bool,
    /// With [`Mode::Dpad`]: the jump and sneak buttons trade places.
    pub swap_jump_sneak: bool,
    pub sprint_button: bool,
    /// Sprint by pushing the joystick past its rim instead of with a button.
    pub sprint_with_joystick: bool,
    pub offhand_button: bool,
    pub shield_button: bool,
    pub circle_size: f32,
    /// Per control: [`FOLLOW_SCHEME`], [`SHOWN`] or [`HIDDEN`].
    #[serde(deserialize_with = "deserialize_visibility")]
    pub button_visibility: [u8; CONTROLS],
    /// One row of placements per [`Mode`], indexed by [`Control`].
    #[serde(deserialize_with = "deserialize_layouts")]
    pub layouts: [[Placement; CONTROLS]; 3],
    /// Camera turn speed for look drags; scales both axes.
    pub sensitivity: f32,
    /// Extra scale on the yaw (left and right) turn.
    pub sensitivity_horizontal: f32,
    /// Extra scale on the pitch (up and down) turn.
    pub sensitivity_vertical: f32,
    /// The sprint button toggles sprinting instead of holding it.
    pub sprint_toggle: bool,
    /// In the crosshair schemes, taps and holds on the world also attack and
    /// use (at the crosshair), as in the tap schemes.
    pub crosshair_gestures: bool,
    /// In the crosshair schemes, dragging the attack button turns the camera.
    pub attack_button_look: bool,
    /// In the tap schemes, the use button shows while it would act on what
    /// the crosshair points at.
    pub contextual_interact: bool,
    /// The joystick appears where the finger lands, within the dynamic area.
    pub dynamic_joystick: bool,
    /// The dynamic area's width: a fraction of the surface, from the
    /// joystick's edge inwards.
    pub dynamic_area_x: f32,
    /// The dynamic area's height, centred on the joystick (1 is the whole height).
    pub dynamic_area_y: f32,
    /// Scales every control without changing the saved sizes (0.5-2).
    pub button_scale: f32,
    pub custom_buttons: Vec<super::CustomButton>,
    pub analog_movement: bool,
    pub inventory_controls: bool,
    pub virtual_mouse_sensitivity: f32,
    pub gyroscope: bool,
    pub gyro_sensitivity: f32,
    pub attack_indicator_left: bool,
    pub horizontal_margin: f32,
    pub vertical_margin: f32,
}


impl Default for Settings {
    fn default() -> Self {
        let row = [
            Placement::new(0.13, 0.76, 0.32),   // Move
            Placement::new(0.72, 0.78, 0.105),  // Sprint
            Placement::new(0.92, 0.78, 0.16),   // Jump
            Placement::new(0.81, 0.86, 0.115),  // Sneak
            Placement::new(0.80, 0.64, 0.16),   // Attack
            Placement::new(0.92, 0.52, 0.16),   // Use
            Placement::new(0.46, 0.055, 0.075), // Chat
            Placement::new(0.50, 0.055, 0.075), // Pause
            Placement::new(0.54, 0.055, 0.075), // Settings
            Placement::new(0.73, 0.95, 0.075),  // Inventory
            Placement::new(0.42, 0.055, 0.075), // Debug
            Placement::new(0.58, 0.055, 0.075), // Perspective
            Placement::new(0.80, 0.96, 0.075),  // Offhand
            Placement::new(0.92, 0.36, 0.12),   // Shield
        ];
        let mut layouts = [row; 3];
        // The D-pad sits where the joystick does; sneak becomes its centre.
        layouts[Mode::Dpad.index()][Control::Sneak as usize] = Placement::new(0.13, 0.76, 0.10);
        layouts[Mode::Dpad.index()][Control::Jump as usize] = Placement::new(0.90, 0.78, 0.14);
        Self {
            mode: Mode::Crosshair,
            split_controls: false,
            left_handed: false,
            swap_jump_sneak: false,
            sprint_button: true,
            sprint_with_joystick: false,
            offhand_button: false,
            shield_button: true,
            button_visibility: [FOLLOW_SCHEME; CONTROLS],
            layouts,
            sensitivity: 1.0,
            sensitivity_horizontal: 1.0,
            sensitivity_vertical: 1.0,
            sprint_toggle: false,
            crosshair_gestures: false,
            attack_button_look: true,
            contextual_interact: false,
            dynamic_joystick: false,
            dynamic_area_x: 0.25,
            dynamic_area_y: 1.0,
            circle_size: 32.0,
            custom_buttons: Vec::new(),
            analog_movement: true,
            inventory_controls: false,
            virtual_mouse_sensitivity: 1.0,
            gyroscope: false,
            gyro_sensitivity: 1.0,
            attack_indicator_left: true,
            horizontal_margin: 0.0,
            vertical_margin: 0.0,
            button_scale: 1.0,
        }
    }
}

impl Settings {
    /// Reads `path`; a missing or unreadable file gives the defaults.
    pub fn load(path: &Path) -> Self {
        let mut settings: Self = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        settings.sanitize();
        // The camera button is a factory utility, not a saved layout choice.
        // Add it to every install (including upgrades from files with an empty
        // custom list) without touching any button the player added.
        if !settings
            .custom_buttons
            .iter()
            .any(|button| button.key == "action:key.screenshot")
        {
            let mut camera = super::CustomButton::new("action:key.screenshot".into(), 0);
            camera.layouts = [Placement::new(0.80, 0.30, 0.075); 3];
            settings.custom_buttons.push(camera);
        }
        // Legacy migrations: the top row and the factory swap button moved.
        for row in &mut settings.layouts {
            for (control, old, new) in [
                (Control::Chat, 0.45, 0.46),
                (Control::Settings, 0.55, 0.54),
                (Control::Debug, 0.40, 0.42),
                (Control::Perspective, 0.60, 0.58),
            ] {
                let p = &mut row[control as usize];
                if p.center == [old, 0.055] && p.size == 0.075 {
                    p.center[0] = new;
                }
            }
        }
        for row in &mut settings.layouts {
            let swap = &mut row[Control::Offhand as usize];
            if matches!(swap.center, [0.68, 0.80] | [0.66, 0.38])
                && (swap.size - 0.09).abs() < 0.0001
            {
                swap.center = [0.80, 0.96];
                swap.size = 0.075;
            }
        }
        settings
    }

    /// Reads `path`, falling back to the file in the working directory (the
    /// fork's `horizon-touch.json`).
    pub fn load_from(path: &Path) -> Self {
        Self::load(path)
    }

    /// Writes `path` through a temporary file, so a crash never leaves half a layout.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temp, path)
    }

    /// Compatibility wrapper for the explicit file path.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        self.save(path)
    }

    pub fn sanitize(&mut self) {
        // Old Tap + split files become Crosshair; taps then act at the crosshair.
        if self.mode == Mode::Tap && self.split_controls {
            self.mode = Mode::Crosshair;
            self.split_controls = false;
            self.crosshair_gestures = true;
        }
        // Only the D-pad has a split variant (see `Mode::SCHEMES`).
        if self.mode != Mode::Dpad {
            self.split_controls = false;
        }
        self.virtual_mouse_sensitivity = finite_or(self.virtual_mouse_sensitivity, 1.0).clamp(0.25, 3.0);
        self.gyro_sensitivity = finite_or(self.gyro_sensitivity, 1.0).clamp(0.1, 3.0);
        for value in [&mut self.horizontal_margin, &mut self.vertical_margin] {
            *value = finite_or(*value, 0.0).clamp(0.0, 0.12);
        }
        self.circle_size = finite_or(self.circle_size, 32.0).clamp(16.0, 64.0);
        self.custom_buttons.truncate(32);
        self.custom_buttons
            .retain(|b| super::CustomButton::valid_token(&b.key));
        for button in &mut self.custom_buttons {
            for p in &mut button.layouts {
                p.sanitize();
            }
        }
        for v in &mut self.button_visibility {
            if *v > HIDDEN {
                *v = FOLLOW_SCHEME;
            }
        }
        for p in self.layouts.iter_mut().flatten() {
            p.sanitize();
        }
        for v in [
            &mut self.sensitivity,
            &mut self.sensitivity_horizontal,
            &mut self.sensitivity_vertical,
        ] {
            *v = finite_or(*v, 1.0).clamp(0.1, 3.0);
        }
        self.button_scale = finite_or(self.button_scale, 1.0).clamp(0.5, 2.0);
        self.dynamic_area_x = finite_or(self.dynamic_area_x, 0.25).clamp(0.0, 1.0);
        self.dynamic_area_y = finite_or(self.dynamic_area_y, 1.0).clamp(0.0, 1.0);
    }

    /// Whether attack and use act on the crosshair rather than on a tap.
    pub fn crosshair(&self) -> bool {
        self.mode == Mode::Crosshair || self.split_controls
    }

    pub fn visible(&self, c: Control) -> bool {
        match self.button_visibility[c as usize] {
            SHOWN => return true,
            HIDDEN => return false,
            _ => {}
        }
        match c {
            Control::Offhand => self.offhand_button,
            Control::Shield => self.shield_button && !self.visible(Control::Use),
            Control::Attack | Control::Use => self.crosshair(),
            Control::Sprint => {
                self.sprint_button && self.mode != Mode::Dpad && !self.sprint_with_joystick
            }
            _ => true,
        }
    }

    /// The layout slot a control is drawn in (jump and sneak may trade places).
    pub fn slot(&self, c: Control) -> usize {
        let swap = self.mode == Mode::Dpad && self.swap_jump_sneak;
        match c {
            Control::Jump if swap => Control::Sneak as usize,
            Control::Sneak if swap => Control::Jump as usize,
            _ => c as usize,
        }
    }

    /// Where a control is in the current scheme, mirrored for left hands.
    pub fn placement(&self, c: Control) -> Placement {
        let mut p = self.layouts[self.mode.index()][self.slot(c)];
        if self.left_handed {
            p.center[0] = 1.0 - p.center[0];
        }
        p
    }
}

/// Whether `p` lies in the rectangle `[x, y, w, h]` (edges included).
pub fn contains([x, y, w, h]: [f32; 4], p: [f32; 2]) -> bool {
    (x..=x + w).contains(&p[0]) && (y..=y + h).contains(&p[1])
}

// Append new buttons without resetting a player's existing ten-button layout.
fn deserialize_layouts<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<[[Placement; CONTROLS]; 3], D::Error> {
    let rows = Vec::<Vec<Placement>>::deserialize(deserializer)?;
    if rows.len() != 3 || rows.iter().any(|r| !matches!(r.len(), 10 | 12 | 14)) {
        return Err(serde::de::Error::custom(
            "expected three touch layouts of 10, 12 or 14 controls",
        ));
    }
    let mut layouts = Settings::default().layouts;
    for (output, input) in layouts.iter_mut().zip(rows) {
        output[..input.len()].copy_from_slice(&input);
    }
    Ok(layouts)
}

fn deserialize_visibility<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[u8; CONTROLS], D::Error> {
    let old = Vec::<u8>::deserialize(d)?;
    if !matches!(old.len(), 12 | 14) {
        return Err(serde::de::Error::custom(
            "expected 12 or 14 visibility values",
        ));
    }
    let mut values = [0; CONTROLS];
    values[..old.len()].copy_from_slice(&old);
    Ok(values)
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("horizon-touch-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn mode_indices_match_layout_rows_and_schemes() {
        for (i, mode) in Mode::ALL.into_iter().enumerate() {
            assert_eq!(mode.index(), i);
        }
        for (mode, split, _) in Mode::SCHEMES {
            assert!(!split || mode == Mode::Dpad);
        }
        for (i, c) in Control::ALL.into_iter().enumerate() {
            assert_eq!(c as usize, i);
        }
    }

    #[test]
    fn scheme_decides_attack_use_sprint_and_shield() {
        let mut s = Settings::default();
        assert!(s.visible(Control::Attack) && s.visible(Control::Use));
        assert!(s.visible(Control::Sprint));
        // The shield button only stands in when Use is hidden.
        assert!(!s.visible(Control::Shield));
        s.mode = Mode::Tap;
        assert!(!s.visible(Control::Attack) && !s.visible(Control::Use));
        assert!(s.visible(Control::Shield));
        s.mode = Mode::Dpad;
        assert!(!s.visible(Control::Sprint));
        s.split_controls = true;
        assert!(s.crosshair() && s.visible(Control::Attack));
        s.mode = Mode::Crosshair;
        s.sprint_with_joystick = true;
        assert!(!s.visible(Control::Sprint));
        assert!(!s.visible(Control::Offhand));
    }

    #[test]
    fn player_visibility_overrides_the_scheme_and_survives_a_save() {
        let mut s = Settings {
            mode: Mode::Tap,
            ..Settings::default()
        };
        s.button_visibility[Control::Attack as usize] = SHOWN;
        s.button_visibility[Control::Chat as usize] = HIDDEN;
        assert!(s.visible(Control::Attack));
        assert!(!s.visible(Control::Chat));
        let file = temp_file("visibility");
        s.save(&file).unwrap();
        let read = Settings::load(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(read, s);
    }

    #[test]
    fn missing_bad_or_partial_files_fall_back_to_defaults() {
        assert_eq!(Settings::load(&temp_file("absent")), Settings::default());
        let file = temp_file("bad");
        std::fs::write(&file, b"{ not json").unwrap();
        assert_eq!(Settings::load(&file), Settings::default());
        std::fs::write(&file, br#"{"mode":"dpad","left_handed":true}"#).unwrap();
        let read = Settings::load(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(read.mode, Mode::Dpad);
        assert!(read.left_handed);
        assert_eq!(read.layouts, Settings::default().layouts);
    }

    #[test]
    fn sanitize_brings_controls_back_on_screen() {
        let mut s = Settings {
            mode: Mode::Tap,
            split_controls: true,
            ..Settings::default()
        };
        s.button_visibility[0] = 9;
        s.layouts[0][0] = Placement {
            center: [f32::NAN, 7.0],
            size: 0.0,
            opacity: f32::INFINITY,
        };
        s.sanitize();
        assert!(!s.split_controls);
        assert_eq!(s.button_visibility[0], FOLLOW_SCHEME);
        let p = s.layouts[0][0];
        assert_eq!(p.center, [0.5, 0.975]);
        assert_eq!(p.size, 0.045);
        assert_eq!(p.opacity, 0.65);
    }

    #[test]
    fn sensitivity_stays_finite_and_in_range() {
        let mut s = Settings {
            sensitivity: f32::INFINITY,
            sensitivity_horizontal: 9.0,
            sensitivity_vertical: 0.0,
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(
            [
                s.sensitivity,
                s.sensitivity_horizontal,
                s.sensitivity_vertical
            ],
            [1.0, 3.0, 0.1]
        );
        // Files saved before these options existed keep the defaults.
        let read: Settings = serde_json::from_str(r#"{"mode":"tap"}"#).unwrap();
        assert_eq!(read.sensitivity, 1.0);
    }

    #[test]
    fn control_options_default_safely_and_stay_in_range() {
        let read: Settings = serde_json::from_str(r#"{"mode":"tap"}"#).unwrap();
        assert!(read.attack_button_look && !read.sprint_toggle && !read.dynamic_joystick);
        assert_eq!([read.dynamic_area_x, read.dynamic_area_y], [0.25, 1.0]);
        let mut s = Settings {
            button_scale: f32::NAN,
            dynamic_area_x: -1.0,
            dynamic_area_y: f32::INFINITY,
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(
            [s.button_scale, s.dynamic_area_x, s.dynamic_area_y],
            [1.0, 0.0, 1.0]
        );
        s.button_scale = 9.0;
        s.sanitize();
        assert_eq!(s.button_scale, 2.0);
    }

    #[test]
    fn left_hands_mirror_and_dpad_can_swap_jump_and_sneak() {
        let mut s = Settings::default();
        let right = s.placement(Control::Jump).center;
        s.left_handed = true;
        assert_eq!(
            s.placement(Control::Jump).center,
            [1.0 - right[0], right[1]]
        );
        s.left_handed = false;
        s.mode = Mode::Dpad;
        s.swap_jump_sneak = true;
        let row = s.layouts[Mode::Dpad.index()];
        assert_eq!(s.placement(Control::Jump), row[Control::Sneak as usize]);
        assert_eq!(s.placement(Control::Sneak), row[Control::Jump as usize]);
        // Only the D-pad swaps.
        s.mode = Mode::Crosshair;
        assert_eq!(s.slot(Control::Jump), Control::Jump as usize);
    }

    #[test]
    fn controls_stay_square_on_wide_surfaces() {
        let p = Placement::new(0.5, 0.5, 0.2);
        let [x, y, w, h] = p.rect(2.0);
        assert_eq!([w, h], [0.1, 0.2]);
        assert!(contains([x, y, w, h], [0.5, 0.5]));
        assert!(contains([x, y, w, h], [x + w, y + h]));
        assert!(!contains([x, y, w, h], [0.56, 0.5]));
    }

    #[test]
    fn factory_controls_are_on_screen_in_every_mode() {
        let s = Settings::default();
        for row in s.layouts {
            for p in row {
                let mut clamped = p;
                clamped.sanitize();
                assert_eq!(clamped, p);
            }
        }
    }
}

#[cfg(test)]
mod fork_tests {
    use super::*;

    #[test]
    fn gyro_preferences_roundtrip_and_reject_bad_values() {
        let mut s = Settings::default();
        assert!(!s.gyroscope);
        s.gyroscope = true;
        s.gyro_sensitivity = 2.0;
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert!(restored.gyroscope);
        assert_eq!(restored.gyro_sensitivity, 2.0);
        s.gyro_sensitivity = f32::NAN;
        s.sanitize();
        assert_eq!(s.gyro_sensitivity, 1.0);
    }

    #[test]
    fn blank_buttons_and_game_actions_survive_settings_validation() {
        let mut settings = Settings::default();
        settings.custom_buttons.push(super::super::CustomButton::new(
            "key.keyboard.unknown".into(),
            0,
        ));
        settings
            .custom_buttons
            .push(super::super::CustomButton::new("action:key.jump".into(), 1));
        settings.sanitize();
        assert_eq!(settings.custom_buttons.len(), 2);
        let encoded = serde_json::to_string(&settings.custom_buttons[1]).unwrap();
        assert_eq!(
            serde_json::from_str::<super::super::CustomButton>(&encoded)
                .unwrap()
                .key,
            "action:key.jump"
        );
        assert!(!super::super::CustomButton::valid_token(
            "action:no-such-action"
        ));
        assert!(
            super::super::custom::choices(false)
                .iter()
                .any(|(token, _)| token == "action:key.attack")
        );
        assert!(
            super::super::custom::choices(true)
                .iter()
                .any(|(token, _)| token == "key.keyboard.z")
        );
    }

    #[test]
    fn loading_adds_one_camera_button_without_touching_custom_buttons() {
        let mut settings = Settings::default();
        settings
            .custom_buttons
            .push(super::super::CustomButton::new("action:key.jump".into(), 0));
        let path = std::env::temp_dir().join(format!(
            "horizon-touch-camera-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
        let loaded = Settings::load_from(&path);
        let camera = loaded
            .custom_buttons
            .iter()
            .filter(|button| button.key == "action:key.screenshot")
            .count();
        assert_eq!(camera, 1);
        assert!(loaded
            .custom_buttons
            .iter()
            .any(|button| button.key == "action:key.jump"));
        // Loading twice is idempotent: no duplicate button on every start.
        let loaded_twice = Settings::load_from(&path);
        assert_eq!(loaded_twice.custom_buttons.len(), loaded.custom_buttons.len());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn added_buttons_override_scheme_and_removed_buttons_stay_hidden() {
        let mut settings = Settings {
            mode: Mode::Tap,
            ..Settings::default()
        };
        assert!(!settings.visible(Control::Attack));
        settings.button_visibility[Control::Attack as usize] = 1;
        assert!(settings.visible(Control::Attack));
        settings.button_visibility[Control::Chat as usize] = 2;
        assert!(!settings.visible(Control::Chat));
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(restored.visible(Control::Attack));
        assert!(!restored.visible(Control::Chat));
    }

    #[test]
    fn old_layout_preserves_custom_buttons_when_shortcuts_are_added() {
        let mut json = serde_json::to_value(Settings::default()).unwrap();
        json["circle_size"] = 48.into();
        for row in json["layouts"].as_array_mut().unwrap() {
            row.as_array_mut().unwrap().truncate(10);
            row[6]["center"] = serde_json::json!([0.25, 0.2]);
            row[6]["opacity"] = serde_json::json!(0.35);
        }
        let settings: Settings = serde_json::from_value(json).unwrap();
        assert_eq!(settings.circle_size, 48.0);
        let factory = Settings::default();
        for (row, factory) in settings.layouts.iter().zip(factory.layouts.iter()) {
            assert_eq!(row[6].center, [0.25, 0.2]);
            assert_eq!(row[6].opacity, 0.35);
            assert_eq!(
                row[Control::Debug as usize].center,
                factory[Control::Debug as usize].center
            );
            assert_eq!(
                row[Control::Perspective as usize].center,
                factory[Control::Perspective as usize].center
            );
        }
        let roundtrip: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(roundtrip.layouts[0][6].center, [0.25, 0.2]);
    }

    #[test]
    fn swap_factory_migration_keeps_custom_positions() {
        let file = std::env::temp_dir().join(format!("horizon-swap-layout-{}.json", std::process::id()));
        let mut settings = Settings::default();
        settings.layouts[0][Control::Offhand as usize] = Placement::new(0.68, 0.80, 0.09);
        settings.layouts[1][Control::Offhand as usize].center = [0.55, 0.43];
        settings.layouts[2][Control::Offhand as usize] = Placement::new(0.66, 0.38, 0.09);
        settings.save_to(&file).unwrap();
        let read = Settings::load_from(&file);
        assert_eq!(
            read.layouts[0][Control::Offhand as usize].center,
            [0.80, 0.96]
        );
        assert_eq!(
            read.layouts[1][Control::Offhand as usize].center,
            [0.55, 0.43]
        );
        assert_eq!(
            read.layouts[2][Control::Offhand as usize].center,
            [0.80, 0.96]
        );
        std::fs::remove_file(file).unwrap();
    }
}
