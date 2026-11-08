//! Material 3 Expressive colour tokens for Horizon's Android launcher.
//!
//! Dynamic colour reads the system tonal palettes on Android 12+ and calls [`Scheme::from_seeds`].
//! Older devices, and any failure to read the system palette, use [`Scheme::horizon`]. Colours are
//! `0xRRGGBB`; the launcher is the only consumer, and vanilla game menus keep the existing tokens.

/// The four palette seeds: the wallpaper's three accents and its neutral.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seeds {
    pub primary: u32,
    pub secondary: u32,
    pub tertiary: u32,
    pub neutral: u32,
}

impl Seeds {
    /// Horizon's violet fallback, used when the system palette is unavailable.
    pub const HORIZON: Self = Self {
        primary: 0x7656fa,
        secondary: 0xa08cff,
        tertiary: 0xe58bb5,
        neutral: 0x6b6480,
    };
}

/// A Material colour scheme. The names are Material's roles; `surface_container` is the default
/// container role and the low/high variants sit either side of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scheme {
    pub primary: u32,
    pub on_primary: u32,
    pub primary_container: u32,
    pub on_primary_container: u32,
    pub secondary: u32,
    pub on_secondary: u32,
    pub secondary_container: u32,
    pub on_secondary_container: u32,
    pub tertiary: u32,
    pub on_tertiary: u32,
    pub tertiary_container: u32,
    pub on_tertiary_container: u32,
    pub error: u32,
    pub on_error: u32,
    pub error_container: u32,
    pub on_error_container: u32,
    pub background: u32,
    pub on_background: u32,
    pub surface: u32,
    pub on_surface: u32,
    pub surface_variant: u32,
    pub on_surface_variant: u32,
    pub surface_dim: u32,
    pub surface_bright: u32,
    pub surface_container_lowest: u32,
    pub surface_container_low: u32,
    pub surface_container: u32,
    pub surface_container_high: u32,
    pub surface_container_highest: u32,
    pub outline: u32,
    pub outline_variant: u32,
    pub inverse_surface: u32,
    pub inverse_on_surface: u32,
    pub inverse_primary: u32,
    pub scrim: u32,
    pub shadow: u32,
    pub surface_tint: u32,
    pub is_dark: bool,
}

impl Scheme {
    /// The violet fallback, in the requested appearance.
    pub fn horizon(dark: bool) -> Self {
        Self::from_seeds(Seeds::HORIZON, dark)
    }

    /// A scheme from one seed: the primary hue supplies the secondary, tertiary and neutral
    /// hues. This is the fallback path and the basis for the dynamic path.
    pub fn from_seed(seed: u32, dark: bool) -> Self {
        let seed = TonalPalette::from_rgb(seed);
        if seed.chroma < 0.02 {
            return Self::horizon(dark);
        }
        let primary = seed;
        let secondary = TonalPalette {
            hue: primary.hue,
            chroma: (primary.chroma * 0.55).clamp(0.04, 0.10),
        };
        let tertiary = TonalPalette {
            hue: (primary.hue + 60.) % 360.,
            chroma: (primary.chroma * 0.8).clamp(0.08, 0.14),
        };
        let neutral = TonalPalette {
            hue: primary.hue,
            chroma: 0.006,
        };
        let neutral_variant = TonalPalette {
            hue: primary.hue,
            chroma: 0.012,
        };
        Self::from_palettes(
            with_min_chroma(primary, 0.12),
            secondary,
            tertiary,
            neutral,
            neutral_variant,
            TonalPalette::from_rgb(0xb3261e),
            dark,
        )
    }

    /// A scheme from the system's tonal seeds. Android 12+ supplies the four accent/neutral
    /// colours; the error palette stays Material's fixed red.
    pub fn from_seeds(seeds: Seeds, dark: bool) -> Self {
        let seed_primary = TonalPalette::from_rgb(seeds.primary);
        if seed_primary.chroma < 0.02 {
            return Self::horizon(dark);
        }
        let primary = with_min_chroma(seed_primary, 0.12);
        let secondary = with_chroma_range(TonalPalette::from_rgb(seeds.secondary), 0.04, 0.10);
        let tertiary = with_chroma_range(TonalPalette::from_rgb(seeds.tertiary), 0.08, 0.14);
        let neutral_hue = TonalPalette::from_rgb(seeds.neutral).hue;
        let neutral = TonalPalette {
            hue: neutral_hue,
            chroma: 0.006,
        };
        let neutral_variant = TonalPalette {
            hue: neutral_hue,
            chroma: 0.012,
        };
        Self::from_palettes(
            primary,
            secondary,
            tertiary,
            neutral,
            neutral_variant,
            TonalPalette::from_rgb(0xb3261e),
            dark,
        )
    }

    fn from_palettes(
        primary: TonalPalette,
        secondary: TonalPalette,
        tertiary: TonalPalette,
        neutral: TonalPalette,
        neutral_variant: TonalPalette,
        error: TonalPalette,
        dark: bool,
    ) -> Self {
        if dark {
            Self {
                primary: primary.tone(80.),
                on_primary: primary.tone(20.),
                primary_container: primary.tone(30.),
                on_primary_container: primary.tone(90.),
                secondary: secondary.tone(80.),
                on_secondary: secondary.tone(20.),
                secondary_container: secondary.tone(30.),
                on_secondary_container: secondary.tone(90.),
                tertiary: tertiary.tone(80.),
                on_tertiary: tertiary.tone(20.),
                tertiary_container: tertiary.tone(30.),
                on_tertiary_container: tertiary.tone(90.),
                error: error.tone(80.),
                on_error: error.tone(20.),
                error_container: error.tone(30.),
                on_error_container: error.tone(90.),
                background: neutral.tone(6.),
                on_background: neutral.tone(90.),
                surface: neutral.tone(6.),
                on_surface: neutral.tone(90.),
                surface_variant: neutral_variant.tone(30.),
                on_surface_variant: neutral_variant.tone(80.),
                surface_dim: neutral.tone(6.),
                surface_bright: neutral.tone(24.),
                surface_container_lowest: neutral.tone(4.),
                surface_container_low: neutral.tone(10.),
                surface_container: neutral.tone(12.),
                surface_container_high: neutral.tone(17.),
                surface_container_highest: neutral.tone(22.),
                outline: neutral_variant.tone(60.),
                outline_variant: neutral_variant.tone(30.),
                inverse_surface: neutral.tone(90.),
                inverse_on_surface: neutral.tone(20.),
                inverse_primary: primary.tone(40.),
                scrim: 0x000000,
                shadow: 0x000000,
                surface_tint: primary.tone(80.),
                is_dark: true,
            }
        } else {
            Self {
                primary: primary.tone(40.),
                on_primary: primary.tone(100.),
                primary_container: primary.tone(90.),
                on_primary_container: primary.tone(10.),
                secondary: secondary.tone(40.),
                on_secondary: secondary.tone(100.),
                secondary_container: secondary.tone(90.),
                on_secondary_container: secondary.tone(10.),
                tertiary: tertiary.tone(40.),
                on_tertiary: tertiary.tone(100.),
                tertiary_container: tertiary.tone(90.),
                on_tertiary_container: tertiary.tone(10.),
                error: error.tone(40.),
                on_error: error.tone(100.),
                error_container: error.tone(90.),
                on_error_container: error.tone(10.),
                background: neutral.tone(98.),
                on_background: neutral.tone(10.),
                surface: neutral.tone(98.),
                on_surface: neutral.tone(10.),
                surface_variant: neutral_variant.tone(90.),
                on_surface_variant: neutral_variant.tone(30.),
                surface_dim: neutral.tone(87.),
                surface_bright: neutral.tone(98.),
                surface_container_lowest: neutral.tone(100.),
                surface_container_low: neutral.tone(96.),
                surface_container: neutral.tone(94.),
                surface_container_high: neutral.tone(92.),
                surface_container_highest: neutral.tone(90.),
                outline: neutral_variant.tone(50.),
                outline_variant: neutral_variant.tone(80.),
                inverse_surface: neutral.tone(20.),
                inverse_on_surface: neutral.tone(95.),
                inverse_primary: primary.tone(80.),
                scrim: 0x000000,
                shadow: 0x000000,
                surface_tint: primary.tone(40.),
                is_dark: false,
            }
        }
    }
}

fn with_min_chroma(palette: TonalPalette, min: f32) -> TonalPalette {
    TonalPalette {
        hue: palette.hue,
        chroma: palette.chroma.max(min),
    }
}

fn with_chroma_range(palette: TonalPalette, min: f32, max: f32) -> TonalPalette {
    TonalPalette {
        hue: palette.hue,
        chroma: palette.chroma.clamp(min, max),
    }
}

/// A tonal palette in OKLCH. Material's tone values are CIELAB L*; [`TonalPalette::tone`] maps
/// them to OKLCH lightness and reduces chroma until the colour is inside sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TonalPalette {
    /// Hue in degrees, 0..360.
    pub hue: f32,
    /// OKLCH chroma. Material's primary palettes are roughly 0.12 to 0.26.
    pub chroma: f32,
}

impl TonalPalette {
    /// The hue and chroma of an sRGB seed. A grey seed keeps an arbitrary hue; its chroma is 0.
    pub fn from_rgb(rgb: u32) -> Self {
        let [_, a, b] = oklab(rgb);
        Self {
            hue: b.atan2(a).to_degrees().rem_euclid(360.),
            chroma: (a * a + b * b).sqrt(),
        }
    }

    /// The sRGB colour at Material tone `t` (0..100).
    pub fn tone(&self, t: f32) -> u32 {
        if t <= 0. {
            return 0x000000;
        }
        if t >= 100. {
            return 0xffffff;
        }
        oklch_to_srgb(tone_lightness(t), self.chroma, self.hue)
    }
}

/// The sRGB colour for an OKLCH tuple. Out-of-gamut colours lose chroma, not lightness.
fn oklch_to_srgb(lightness: f32, chroma: f32, hue: f32) -> u32 {
    let radians = hue.to_radians();
    let (sin, cos) = (radians.sin(), radians.cos());
    let mut chroma = chroma.max(0.);
    for _ in 0..32 {
        let linear = oklab_to_linear([lightness, chroma * cos, chroma * sin]);
        if linear.iter().all(|v| (-0.0001..=1.0001).contains(v)) {
            return linear_to_hex(linear);
        }
        chroma *= 0.9;
    }
    linear_to_hex(oklab_to_linear([lightness, 0., 0.]))
}

/// Material tone (CIELAB L*) to OKLab lightness. For a neutral, OKLab L is `cbrt(Y)`.
fn tone_lightness(tone: f32) -> f32 {
    let lstar = tone.clamp(0., 100.);
    let y = if lstar > 8. {
        ((lstar + 16.) / 116.).powi(3)
    } else {
        lstar / 903.3
    };
    y.cbrt()
}

fn linear_to_hex(linear: [f32; 3]) -> u32 {
    let channel = |v: f32| (linear_to_srgb(v).clamp(0., 1.) * 255.).round() as u32;
    (channel(linear[0]) << 16) | (channel(linear[1]) << 8) | channel(linear[2])
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    }
}

fn rgb(c: u32) -> [f32; 3] {
    [
        ((c >> 16) & 255) as f32 / 255.,
        ((c >> 8) & 255) as f32 / 255.,
        (c & 255) as f32 / 255.,
    ]
}

fn oklab(c: u32) -> [f32; 3] {
    let [r, g, b] = rgb(c).map(srgb_to_linear);
    let l = 0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn oklab_to_linear([l, a, b]: [f32; 3]) -> [f32; 3] {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l, m, s) = (l_.powi(3), m_.powi(3), s_.powi(3));
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_4 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrast;

    fn check(fg: u32, bg: u32, name: &str) {
        let ratio = contrast(fg, bg);
        assert!(ratio >= 4.5, "{name}: {ratio:.2}");
    }

    #[test]
    fn horizon_roles_keep_text_contrast() {
        for dark in [false, true] {
            let s = Scheme::horizon(dark);
            for (name, fg, bg) in [
                ("on_primary", s.on_primary, s.primary),
                (
                    "on_primary_container",
                    s.on_primary_container,
                    s.primary_container,
                ),
                ("on_secondary", s.on_secondary, s.secondary),
                (
                    "on_secondary_container",
                    s.on_secondary_container,
                    s.secondary_container,
                ),
                ("on_tertiary", s.on_tertiary, s.tertiary),
                (
                    "on_tertiary_container",
                    s.on_tertiary_container,
                    s.tertiary_container,
                ),
                ("on_error", s.on_error, s.error),
                (
                    "on_error_container",
                    s.on_error_container,
                    s.error_container,
                ),
                ("on_background", s.on_background, s.background),
                ("on_surface", s.on_surface, s.surface),
                (
                    "on_surface_variant",
                    s.on_surface_variant,
                    s.surface_variant,
                ),
                (
                    "inverse_on_surface",
                    s.inverse_on_surface,
                    s.inverse_surface,
                ),
                (
                    "on_surface_lowest",
                    s.on_surface,
                    s.surface_container_lowest,
                ),
                ("on_surface_low", s.on_surface, s.surface_container_low),
                ("on_surface_container", s.on_surface, s.surface_container),
                ("on_surface_high", s.on_surface, s.surface_container_high),
                (
                    "on_surface_highest",
                    s.on_surface,
                    s.surface_container_highest,
                ),
            ] {
                check(fg, bg, &format!("{name} (dark={dark})"));
            }
            assert!(
                contrast(s.outline, s.surface) >= 3.,
                "outline on surface (dark={dark})"
            );
        }
    }

    #[test]
    fn a_gray_wallpaper_falls_back_to_horizon() {
        let gray = Seeds {
            primary: 0x808080,
            secondary: 0x808080,
            tertiary: 0x808080,
            neutral: 0x808080,
        };
        assert_eq!(Scheme::from_seeds(gray, false), Scheme::horizon(false));
        assert_eq!(Scheme::from_seeds(gray, true), Scheme::horizon(true));
        assert_eq!(Scheme::from_seed(0x808080, false), Scheme::horizon(false));
    }

    #[test]
    fn tonal_palette_is_monotonic_and_keeps_the_endpoints() {
        let palette = TonalPalette::from_rgb(Seeds::HORIZON.primary);
        assert_eq!(palette.tone(0.), 0x000000);
        assert_eq!(palette.tone(100.), 0xffffff);
        let mut previous = palette.tone(0.);
        for step in 1..=10 {
            let current = palette.tone(step as f32 * 10.);
            assert!(
                contrast(current, 0x000000) > contrast(previous, 0x000000),
                "tone {} is not lighter than the previous tone",
                step * 10
            );
            previous = current;
        }
    }
}
