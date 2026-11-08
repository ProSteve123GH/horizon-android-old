//! Material's type scale, in dp, on Horizon's display, UI and mono faces.
//!
//! The sizes, line heights and weights follow Material's 15 roles. Tracking is in em, matching
//! the launcher's existing [`crate::Step`] tokens.

/// A typeface family. The UI maps these to the bundled Bricolage and Geist faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeFace {
    Display,
    Ui,
    Mono,
}

/// One step of the Material type scale, in dp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStyle {
    pub size: f32,
    pub line: f32,
    pub weight: u16,
    /// Letter spacing in em.
    pub tracking: f32,
    pub face: TypeFace,
}

const fn style(face: TypeFace, size: f32, line: f32, weight: u16, tracking: f32) -> TypeStyle {
    TypeStyle {
        size,
        line,
        weight,
        tracking,
        face,
    }
}

pub const DISPLAY_LARGE: TypeStyle = style(TypeFace::Display, 57., 64., 700, -0.0044);
pub const DISPLAY_MEDIUM: TypeStyle = style(TypeFace::Display, 45., 52., 700, 0.);
pub const DISPLAY_SMALL: TypeStyle = style(TypeFace::Display, 36., 44., 700, 0.);
pub const HEADLINE_LARGE: TypeStyle = style(TypeFace::Display, 32., 40., 600, 0.);
pub const HEADLINE_MEDIUM: TypeStyle = style(TypeFace::Display, 28., 36., 600, 0.);
pub const HEADLINE_SMALL: TypeStyle = style(TypeFace::Display, 24., 32., 600, 0.);
pub const TITLE_LARGE: TypeStyle = style(TypeFace::Ui, 22., 28., 600, 0.);
pub const TITLE_MEDIUM: TypeStyle = style(TypeFace::Ui, 16., 24., 600, 0.0094);
pub const TITLE_SMALL: TypeStyle = style(TypeFace::Ui, 14., 20., 600, 0.0071);
pub const BODY_LARGE: TypeStyle = style(TypeFace::Ui, 16., 24., 400, 0.031);
pub const BODY_MEDIUM: TypeStyle = style(TypeFace::Ui, 14., 20., 400, 0.018);
pub const BODY_SMALL: TypeStyle = style(TypeFace::Ui, 12., 16., 400, 0.033);
pub const LABEL_LARGE: TypeStyle = style(TypeFace::Ui, 14., 20., 600, 0.0071);
pub const LABEL_MEDIUM: TypeStyle = style(TypeFace::Ui, 12., 16., 600, 0.042);
pub const LABEL_SMALL: TypeStyle = style(TypeFace::Ui, 11., 16., 600, 0.045);

/// Every type role, in Material's order.
pub const TYPE_SCALE: [(&str, TypeStyle); 15] = [
    ("display-large", DISPLAY_LARGE),
    ("display-medium", DISPLAY_MEDIUM),
    ("display-small", DISPLAY_SMALL),
    ("headline-large", HEADLINE_LARGE),
    ("headline-medium", HEADLINE_MEDIUM),
    ("headline-small", HEADLINE_SMALL),
    ("title-large", TITLE_LARGE),
    ("title-medium", TITLE_MEDIUM),
    ("title-small", TITLE_SMALL),
    ("body-large", BODY_LARGE),
    ("body-medium", BODY_MEDIUM),
    ("body-small", BODY_SMALL),
    ("label-large", LABEL_LARGE),
    ("label-medium", LABEL_MEDIUM),
    ("label-small", LABEL_SMALL),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_lines_are_at_least_as_tall_as_their_text() {
        for (name, style) in TYPE_SCALE {
            assert!(style.line >= style.size, "{name}");
            assert!(style.size > 0., "{name}");
        }
    }
}
