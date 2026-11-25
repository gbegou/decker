//! Document model, native format and SVG renderer of decker.
//!
//! The native format is a folder `<name>.deck/` holding `deck.json` (the
//! [`Deck`] model, serialized, with a `version` field) and `assets/` for
//! binary files (images). A single-file zip of that folder comes later.
//!
//! Coordinates are in points (1/72 inch), origin at the top-left corner of
//! the slide. A 16:9 slide is 960 x 540 points.

pub mod model;
pub mod native;
pub mod svg;
pub mod text;
pub mod themes;
pub mod transfer;
pub mod vars;

pub use model::*;
pub use svg::{MathImage, MathRenderer, NoMath};
pub use themes::{ThemeSet, PLACEHOLDER_PROPS};
