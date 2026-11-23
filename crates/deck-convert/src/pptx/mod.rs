//! PPTX (PresentationML) import and export.
//!
//! Only a subset of the format is handled: slide size, text with runs
//! (bold, italic, size, color, font), bullets and alignment, pictures,
//! rectangles, ellipses and lines, and speaker notes. Placeholders inherit
//! their position from the slide layout, then the master. Anything else is
//! skipped and reported as a warning.

pub mod export;
pub mod import;
mod package;

pub use export::export;
pub use import::import;

/// English Metric Units per point.
pub const EMU_PER_PT: f64 = 12700.0;

pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

pub const REL_SLIDE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
pub const REL_LAYOUT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
pub const REL_MASTER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
pub const REL_NOTES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
pub const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const REL_OFFICE_DOC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
