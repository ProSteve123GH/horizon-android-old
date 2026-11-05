//! Launcher-wide appearance preferences for the Android Compose launcher.
//!
//! Theme and accent choices are launcher state, not game settings: they stay
//! outside every instance's game folder and apply to every launcher screen.
//! The custom background is copied into the launcher home by the Activity, so
//! this module only records whether that fixed file is in use and reports
//! whether it still exists. Nothing here reads image pixels or uses the network.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// The launcher's own appearance file, beside `android-launcher.json`.
pub const STORE_FILE: &str = "android-appearance.json";
/// The single copied background image. The Activity writes this exact name.
pub const BACKGROUND_FILE: &str = "launcher-background.jpg";
/// A copied background larger than this is ignored rather than decoded.
pub const MAX_BACKGROUND_BYTES: u64 = 24 * 1024 * 1024;
const BACKGROUND_PARTIAL: &str = "launcher-background.partial";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System default",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.key() == value)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Accent {
    #[default]
    System,
    Violet,
    Ocean,
    Forest,
    Sunset,
    Rose,
    Teal,
    Gold,
}

impl Accent {
    pub const ALL: [Self; 8] = [
        Self::System,
        Self::Violet,
        Self::Ocean,
        Self::Forest,
        Self::Sunset,
        Self::Rose,
        Self::Teal,
        Self::Gold,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Violet => "violet",
            Self::Ocean => "ocean",
            Self::Forest => "forest",
            Self::Sunset => "sunset",
            Self::Rose => "rose",
            Self::Teal => "teal",
            Self::Gold => "gold",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|accent| accent.key() == value)
    }
}

/// One accessible accent pair for a light or dark launcher scheme.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct AccentColors {
    pub primary: &'static str,
    pub on_primary: &'static str,
    pub primary_container: &'static str,
    pub on_primary_container: &'static str,
}

/// A built-in accent and the labels the Appearance sheet shows.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct AccentChoice {
    pub key: &'static str,
    pub label: &'static str,
    pub light: Option<AccentColors>,
    pub dark: Option<AccentColors>,
}

const VIOLET_LIGHT: AccentColors = AccentColors {
    primary: "#65558F",
    on_primary: "#FFFFFF",
    primary_container: "#E9DDFF",
    on_primary_container: "#22005D",
};
const VIOLET_DARK: AccentColors = AccentColors {
    primary: "#CFBCFF",
    on_primary: "#381E72",
    primary_container: "#4F378A",
    on_primary_container: "#E9DDFF",
};
const OCEAN_LIGHT: AccentColors = AccentColors {
    primary: "#2F5F8F",
    on_primary: "#FFFFFF",
    primary_container: "#D3E4FF",
    on_primary_container: "#001C38",
};
const OCEAN_DARK: AccentColors = AccentColors {
    primary: "#A3C9F8",
    on_primary: "#0A3055",
    primary_container: "#23496F",
    on_primary_container: "#D3E4FF",
};
const FOREST_LIGHT: AccentColors = AccentColors {
    primary: "#3D6B3B",
    on_primary: "#FFFFFF",
    primary_container: "#BFEBB8",
    on_primary_container: "#002204",
};
const FOREST_DARK: AccentColors = AccentColors {
    primary: "#A4D39D",
    on_primary: "#0A390B",
    primary_container: "#24512A",
    on_primary_container: "#BFEBB8",
};
const SUNSET_LIGHT: AccentColors = AccentColors {
    primary: "#8A5000",
    on_primary: "#FFFFFF",
    primary_container: "#FFDCBE",
    on_primary_container: "#2C1600",
};
const SUNSET_DARK: AccentColors = AccentColors {
    primary: "#FFB870",
    on_primary: "#4A2800",
    primary_container: "#683C00",
    on_primary_container: "#FFDCBE",
};
const ROSE_LIGHT: AccentColors = AccentColors {
    primary: "#8E3F5A",
    on_primary: "#FFFFFF",
    primary_container: "#FFD9E2",
    on_primary_container: "#3A0B1D",
};
const ROSE_DARK: AccentColors = AccentColors {
    primary: "#FFB0C6",
    on_primary: "#561D32",
    primary_container: "#72334B",
    on_primary_container: "#FFD9E2",
};
const TEAL_LIGHT: AccentColors = AccentColors {
    primary: "#00696A",
    on_primary: "#FFFFFF",
    primary_container: "#9CF1F0",
    on_primary_container: "#002020",
};
const TEAL_DARK: AccentColors = AccentColors {
    primary: "#80D4D4",
    on_primary: "#003737",
    primary_container: "#005050",
    on_primary_container: "#9CF1F0",
};
const GOLD_LIGHT: AccentColors = AccentColors {
    primary: "#765A00",
    on_primary: "#FFFFFF",
    primary_container: "#FFE16B",
    on_primary_container: "#241A00",
};
const GOLD_DARK: AccentColors = AccentColors {
    primary: "#EAC348",
    on_primary: "#3E2E00",
    primary_container: "#584400",
    on_primary_container: "#FFE16B",
};

const ACCENTS: [AccentChoice; 8] = [
    AccentChoice {
        key: "system",
        label: "System default",
        light: None,
        dark: None,
    },
    AccentChoice {
        key: "violet",
        label: "Horizon violet",
        light: Some(VIOLET_LIGHT),
        dark: Some(VIOLET_DARK),
    },
    AccentChoice {
        key: "ocean",
        label: "Ocean blue",
        light: Some(OCEAN_LIGHT),
        dark: Some(OCEAN_DARK),
    },
    AccentChoice {
        key: "forest",
        label: "Forest green",
        light: Some(FOREST_LIGHT),
        dark: Some(FOREST_DARK),
    },
    AccentChoice {
        key: "sunset",
        label: "Sunset orange",
        light: Some(SUNSET_LIGHT),
        dark: Some(SUNSET_DARK),
    },
    AccentChoice {
        key: "rose",
        label: "Rose",
        light: Some(ROSE_LIGHT),
        dark: Some(ROSE_DARK),
    },
    AccentChoice {
        key: "teal",
        label: "Teal",
        light: Some(TEAL_LIGHT),
        dark: Some(TEAL_DARK),
    },
    AccentChoice {
        key: "gold",
        label: "Gold",
        light: Some(GOLD_LIGHT),
        dark: Some(GOLD_DARK),
    },
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Stored {
    /// Stored as text so one unknown value does not discard the other fields.
    theme: String,
    accent: String,
    /// True after the Activity copied a file; the file itself is not stored here.
    background: bool,
}

/// The launcher's appearance state, stored at `home/android-appearance.json`.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    theme: Theme,
    accent: Accent,
    background: bool,
}

fn read_stored(home: &Path) -> Stored {
    let path = home.join(STORE_FILE);
    let Ok(bytes) = fs::read(&path) else {
        return Stored::default();
    };
    match serde_json::from_slice(&bytes) {
        Ok(stored) => stored,
        Err(_) => {
            // Keep a copy for diagnosis before a later save replaces the file.
            let _ = fs::copy(&path, home.join(format!("{STORE_FILE}.corrupt")));
            Stored::default()
        }
    }
}

impl Settings {
    pub fn load(home: &Path) -> Self {
        let stored = read_stored(home);
        Self {
            theme: Theme::parse(&stored.theme).unwrap_or_default(),
            accent: Accent::parse(&stored.accent).unwrap_or_default(),
            background: stored.background,
        }
    }

    fn save(&self, home: &Path) -> Result<(), String> {
        let stored = Stored {
            theme: self.theme.key().to_owned(),
            accent: self.accent.key().to_owned(),
            background: self.background,
        };
        let part = home.join(format!("{STORE_FILE}.partial"));
        fs::write(
            &part,
            serde_json::to_vec_pretty(&stored).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(part, home.join(STORE_FILE)).map_err(|e| e.to_string())
    }

    #[cfg(test)]
    pub fn theme(&self) -> Theme {
        self.theme
    }

    #[cfg(test)]
    pub fn accent(&self) -> Accent {
        self.accent
    }

    pub fn set_theme(&mut self, home: &Path, theme: Theme) -> Result<(), String> {
        let previous = self.theme;
        self.theme = theme;
        if let Err(error) = self.save(home) {
            self.theme = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn set_accent(&mut self, home: &Path, accent: Accent) -> Result<(), String> {
        let previous = self.accent;
        self.accent = accent;
        if let Err(error) = self.save(home) {
            self.accent = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Record the Activity's copied image. The fixed path is deliberate: this
    /// module never accepts an arbitrary path from the UI.
    pub fn set_background(&mut self, home: &Path) -> Result<(), String> {
        let path = home.join(BACKGROUND_FILE);
        let metadata =
            fs::metadata(&path).map_err(|_| "The selected image could not be saved.".to_owned())?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err("The selected image is empty or unreadable.".into());
        }
        if metadata.len() > MAX_BACKGROUND_BYTES {
            return Err("That background image is too large.".into());
        }
        let previous = self.background;
        self.background = true;
        if let Err(error) = self.save(home) {
            self.background = previous;
            return Err(error);
        }
        let _ = fs::remove_file(home.join(BACKGROUND_PARTIAL));
        Ok(())
    }

    pub fn reset_background(&mut self, home: &Path) -> Result<(), String> {
        let previous = self.background;
        self.background = false;
        if let Err(error) = self.save(home) {
            self.background = previous;
            return Err(error);
        }
        let _ = fs::remove_file(home.join(BACKGROUND_FILE));
        let _ = fs::remove_file(home.join(BACKGROUND_PARTIAL));
        Ok(())
    }

    pub fn reset(&mut self, home: &Path) -> Result<(), String> {
        Self::default().save(home)?;
        *self = Self::default();
        let _ = fs::remove_file(home.join(BACKGROUND_FILE));
        let _ = fs::remove_file(home.join(BACKGROUND_PARTIAL));
        Ok(())
    }

    fn accent_label(&self) -> &'static str {
        ACCENTS
            .iter()
            .find(|accent| accent.key == self.accent.key())
            .map(|accent| accent.label)
            .unwrap_or("System default")
    }

    fn background_state(&self, home: &Path) -> (Option<String>, u64, bool) {
        if !self.background {
            return (None, 0, false);
        }
        let path = home.join(BACKGROUND_FILE);
        let available = fs::metadata(&path)
            .map(|metadata| {
                metadata.is_file() && metadata.len() > 0 && metadata.len() <= MAX_BACKGROUND_BYTES
            })
            .unwrap_or(false);
        if !available {
            return (None, 0, true);
        }
        let revision = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0);
        let text = path.to_string_lossy().into_owned();
        (Some(text), revision, false)
    }

    /// What the Appearance sheet and the settings summary read.
    pub fn state(&self, home: &Path) -> Value {
        let (background_path, background_revision, background_missing) =
            self.background_state(home);
        json!({
            "theme": self.theme.key(),
            "theme_label": self.theme.label(),
            "accent": self.accent.key(),
            "accent_label": self.accent_label(),
            "themes": Theme::ALL.into_iter().map(|theme| json!({
                "key": theme.key(),
                "label": theme.label(),
            })).collect::<Vec<_>>(),
            "accents": ACCENTS,
            "background_path": background_path,
            "background_revision": background_revision,
            "background_missing": background_missing,
        })
    }

    /// A short value for the Settings row, without repeating the background path.
    pub fn summary(&self) -> String {
        let mut summary = if self.theme == Theme::System && self.accent == Accent::System {
            "System defaults".to_owned()
        } else {
            format!("{} · {}", self.theme.label(), self.accent_label())
        };
        if self.background {
            summary.push_str(" · Custom background");
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "horizon-android-appearance-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
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

    fn state(settings: &Settings, home: &Path) -> Value {
        settings.state(home)
    }

    fn literal(channel: f64) -> f64 {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(hex: &str) -> f64 {
        let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
        let red = ((value >> 16) & 0xff) as f64 / 255.0;
        let green = ((value >> 8) & 0xff) as f64 / 255.0;
        let blue = (value & 0xff) as f64 / 255.0;
        0.2126 * literal(red) + 0.7152 * literal(green) + 0.0722 * literal(blue)
    }

    fn contrast(a: &str, b: &str) -> f64 {
        let (light, dark) = {
            let a = luminance(a);
            let b = luminance(b);
            if a > b { (a, b) } else { (b, a) }
        };
        (light + 0.05) / (dark + 0.05)
    }

    #[test]
    fn every_builtin_accent_is_readable_in_both_schemes() {
        for accent in ACCENTS.iter().filter(|accent| accent.light.is_some()) {
            let light = accent.light.unwrap();
            let dark = accent.dark.unwrap();
            assert!(
                contrast(light.primary, light.on_primary) >= 4.5,
                "{} light primary text",
                accent.key
            );
            assert!(
                contrast(light.primary_container, light.on_primary_container) >= 4.5,
                "{} light container text",
                accent.key
            );
            assert!(
                contrast(dark.primary, dark.on_primary) >= 4.5,
                "{} dark primary text",
                accent.key
            );
            assert!(
                contrast(dark.primary_container, dark.on_primary_container) >= 4.5,
                "{} dark container text",
                accent.key
            );
        }
    }

    #[test]
    fn every_accent_has_one_choice_and_system_has_no_colors() {
        assert_eq!(ACCENTS.len(), Accent::ALL.len());
        for accent in Accent::ALL {
            let choices: Vec<_> = ACCENTS
                .iter()
                .filter(|choice| choice.key == accent.key())
                .collect();
            assert_eq!(choices.len(), 1, "{} exactly once", accent.key());
            if accent == Accent::System {
                assert!(choices[0].light.is_none() && choices[0].dark.is_none());
            } else {
                assert!(choices[0].light.is_some() && choices[0].dark.is_some());
            }
        }
    }

    #[test]
    fn defaults_are_system_theme_system_accent_and_no_background() {
        let scratch = Scratch::new();
        let settings = Settings::load(scratch.path());
        let value = state(&settings, scratch.path());
        assert_eq!(value["theme"], "system");
        assert_eq!(value["accent"], "system");
        assert!(value["background_path"].is_null());
        assert_eq!(value["background_missing"], false);
        assert_eq!(settings.summary(), "System defaults");

        // These names are the JSON contract the Compose model reads.
        let themes = value["themes"].as_array().unwrap();
        assert_eq!(
            themes
                .iter()
                .map(|theme| theme["key"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["system", "light", "dark"]
        );
        let accents = value["accents"].as_array().unwrap();
        let system = accents
            .iter()
            .find(|accent| accent["key"] == "system")
            .unwrap();
        assert!(system["light"].is_null() && system["dark"].is_null());
        let ocean = accents
            .iter()
            .find(|accent| accent["key"] == "ocean")
            .unwrap();
        assert_eq!(ocean["light"]["primary"], "#2F5F8F");
        assert_eq!(ocean["light"]["on_primary"], "#FFFFFF");
        assert_eq!(ocean["light"]["primary_container"], "#D3E4FF");
        assert_eq!(ocean["light"]["on_primary_container"], "#001C38");
        assert_eq!(ocean["dark"]["primary"], "#A3C9F8");
    }

    #[test]
    fn theme_accent_and_background_persist_across_loads() {
        let scratch = Scratch::new();
        let mut settings = Settings::load(scratch.path());
        settings.set_theme(scratch.path(), Theme::Dark).unwrap();
        settings.set_accent(scratch.path(), Accent::Ocean).unwrap();
        fs::write(scratch.path().join(BACKGROUND_FILE), b"picture").unwrap();
        settings.set_background(scratch.path()).unwrap();

        let loaded = Settings::load(scratch.path());
        let value = state(&loaded, scratch.path());
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["accent"], "ocean");
        assert_eq!(value["accent_label"], "Ocean blue");
        assert!(
            value["background_path"]
                .as_str()
                .unwrap()
                .ends_with(BACKGROUND_FILE)
        );
        assert!(value["background_revision"].as_u64().unwrap() > 0);
        assert_eq!(value["background_missing"], false);
    }

    #[test]
    fn missing_background_is_reported_instead_of_breaking_state() {
        let scratch = Scratch::new();
        let mut settings = Settings::load(scratch.path());
        fs::write(scratch.path().join(BACKGROUND_FILE), b"picture").unwrap();
        settings.set_background(scratch.path()).unwrap();
        fs::remove_file(scratch.path().join(BACKGROUND_FILE)).unwrap();

        let value = state(&settings, scratch.path());
        assert!(value["background_path"].is_null());
        assert_eq!(value["background_missing"], true);
        assert_eq!(value["theme"], "system");
    }

    #[test]
    fn reset_background_restores_default_and_removes_the_copy() {
        let scratch = Scratch::new();
        let mut settings = Settings::load(scratch.path());
        fs::write(scratch.path().join(BACKGROUND_FILE), b"picture").unwrap();
        settings.set_background(scratch.path()).unwrap();
        settings.reset_background(scratch.path()).unwrap();

        assert!(!scratch.path().join(BACKGROUND_FILE).exists());
        let value = state(&settings, scratch.path());
        assert!(value["background_path"].is_null());
        assert_eq!(value["background_missing"], false);
    }

    #[test]
    fn reset_restores_every_default_and_removes_the_copy() {
        let scratch = Scratch::new();
        let mut settings = Settings::load(scratch.path());
        settings.set_theme(scratch.path(), Theme::Light).unwrap();
        settings.set_accent(scratch.path(), Accent::Rose).unwrap();
        fs::write(scratch.path().join(BACKGROUND_FILE), b"picture").unwrap();
        settings.set_background(scratch.path()).unwrap();
        settings.reset(scratch.path()).unwrap();

        assert!(!scratch.path().join(BACKGROUND_FILE).exists());
        let loaded = Settings::load(scratch.path());
        let value = state(&loaded, scratch.path());
        assert_eq!(value["theme"], "system");
        assert_eq!(value["accent"], "system");
        assert!(value["background_path"].is_null());
    }

    #[test]
    fn a_corrupt_file_is_backed_up_and_defaults_are_used() {
        let scratch = Scratch::new();
        fs::write(scratch.path().join(STORE_FILE), "{not json").unwrap();
        let settings = Settings::load(scratch.path());
        assert_eq!(settings.theme(), Theme::System);
        assert_eq!(settings.accent(), Accent::System);
        assert_eq!(
            fs::read_to_string(scratch.path().join(format!("{STORE_FILE}.corrupt"))).unwrap(),
            "{not json"
        );
    }

    #[test]
    fn unknown_keys_fall_back_to_defaults_per_field() {
        let scratch = Scratch::new();
        fs::write(
            scratch.path().join(STORE_FILE),
            r#"{"theme":"purple","accent":"ocean","background":true}"#,
        )
        .unwrap();
        let settings = Settings::load(scratch.path());
        assert_eq!(settings.theme(), Theme::System);
        assert_eq!(settings.accent(), Accent::Ocean);
    }

    #[test]
    fn oversized_or_empty_backgrounds_are_rejected() {
        let scratch = Scratch::new();
        let mut settings = Settings::load(scratch.path());
        fs::write(scratch.path().join(BACKGROUND_FILE), b"").unwrap();
        assert!(settings.set_background(scratch.path()).is_err());

        let file = fs::OpenOptions::new()
            .write(true)
            .open(scratch.path().join(BACKGROUND_FILE))
            .unwrap();
        file.set_len(MAX_BACKGROUND_BYTES + 1).unwrap();
        assert!(settings.set_background(scratch.path()).is_err());
        assert_eq!(
            state(&settings, scratch.path())["background_path"],
            Value::Null
        );
    }

    #[test]
    fn a_failed_save_does_not_change_the_in_memory_choice() {
        let scratch = Scratch::new();
        // A file where the partial file should be makes the atomic write fail.
        fs::create_dir_all(scratch.path().join(format!("{STORE_FILE}.partial"))).unwrap();
        let mut settings = Settings::load(scratch.path());
        assert!(settings.set_theme(scratch.path(), Theme::Dark).is_err());
        assert_eq!(settings.theme(), Theme::System);
    }
}
