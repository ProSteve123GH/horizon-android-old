//! Material 3 Expressive tokens for Horizon's Android launcher.
//!
//! The launcher follows Material's colour roles, shape scale, type scale and springs while
//! keeping Horizon's typefaces and fallback palette. The launcher is the only consumer: vanilla
//! game menus keep the existing tokens in [`crate`].
pub mod color;
pub mod motion;
pub mod shape;
pub mod type_scale;

pub use color::{Scheme, Seeds, TonalPalette};
pub use motion::{MotionScheme, Spring, SpringState};
pub use shape::Corners;
pub use type_scale::{TypeFace, TypeStyle};
