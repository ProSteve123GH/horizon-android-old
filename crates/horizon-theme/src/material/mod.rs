//! Material 3 Expressive tokens for Horizon's Android launcher.
//!
//! The launcher follows Material's colour roles; the typefaces and the fallback palette stay
//! Horizon's. The launcher is the only consumer: vanilla game menus keep the existing tokens in
//! [`crate`].
pub mod color;
pub use color::{Scheme, Seeds, TonalPalette};
