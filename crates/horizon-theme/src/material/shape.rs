//! Material's shape scale and per-corner radii, in dp.
//!
//! Component shapes are named tokens; [`Corners::lerp`] drives the expressive shape morph.

/// A per-corner radius, in dp. The renderer's rounded-rect primitive consumes this directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub const fn pill() -> Self {
        Self::all(FULL)
    }

    /// Linear interpolation for shape morphs. `t` is clamped to 0..1.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0., 1.);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self {
            top_left: mix(self.top_left, other.top_left),
            top_right: mix(self.top_right, other.top_right),
            bottom_right: mix(self.bottom_right, other.bottom_right),
            bottom_left: mix(self.bottom_left, other.bottom_left),
        }
    }
}

pub const NONE: f32 = 0.;
pub const EXTRA_SMALL: f32 = 4.;
pub const SMALL: f32 = 8.;
pub const MEDIUM: f32 = 12.;
pub const LARGE: f32 = 16.;
pub const EXTRA_LARGE: f32 = 28.;
pub const EXTRA_EXTRA_LARGE: f32 = 32.;
pub const EXTRA_EXTRA_EXTRA_LARGE: f32 = 48.;
pub const FULL: f32 = 999.;

/// Filled and tonal buttons: pills that relax to [`LARGE`] while pressed.
pub const BUTTON: Corners = Corners::pill();
pub const BUTTON_PRESSED: Corners = Corners::all(LARGE);
/// Cards and filled tonal containers.
pub const CARD: Corners = Corners::all(MEDIUM);
pub const CARD_LARGE: Corners = Corners::all(LARGE);
/// The Play hero, large cards and the FAB's expressive cousin.
pub const HERO: Corners = Corners::all(EXTRA_LARGE);
/// Sheets are square at the bottom and rounded at the top.
pub const SHEET: Corners = Corners::new(EXTRA_LARGE, EXTRA_LARGE, NONE, NONE);
pub const DIALOG: Corners = Corners::all(EXTRA_LARGE);
pub const SEARCH: Corners = Corners::pill();
pub const CHIP: Corners = Corners::all(SMALL);
pub const NAV_INDICATOR: Corners = Corners::pill();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_morph_hits_both_ends() {
        let a = Corners::all(SMALL);
        let b = Corners::all(FULL);
        assert!((a.lerp(b, 0.).top_left - SMALL).abs() < 1e-6);
        assert!((a.lerp(b, 1.).top_left - FULL).abs() < 1e-6);
        assert!((a.lerp(b, 0.5).top_left - (SMALL + FULL) / 2.).abs() < 1e-6);
    }
}
