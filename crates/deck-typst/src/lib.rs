//! Typst embedded as a library: a hermetic world (no file system access, no
//! package registry, bundled fonts) that renders formulas to SVG.
//!
//! A deck brings its own notations: its `math_preamble` is prepended to
//! every formula, and the `.typ` files of its `typst/` folder are served to
//! `#import`, so a deck renders identically on every machine.
//!
//! Text layout through Typst is the next step; see the roadmap.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use deck_core::{Color, MathImage, MathRenderer};

/// The folder of a deck whose `.typ` files are importable by formulas.
pub const TYPST_DIR: &str = "typst";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Typst: {0}")]
    Typst(String),
    #[error("the formula produced no page")]
    Empty,
}

/// Fonts and deck files, loaded once and shared by every render.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Shared>,
}

struct Shared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    files: HashMap<FileId, Source>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Loads the bundled fonts (a few milliseconds), with no deck files.
    pub fn new() -> Self {
        Self::with_files(Vec::new())
    }

    /// An engine serving `files` as `(path, text)` pairs, importable from
    /// formulas by that path (`#import "notation.typ": *`).
    pub fn with_files(files: Vec<(String, String)>) -> Self {
        let mut fonts = vec![];
        let mut book = FontBook::new();
        for data in typst_assets::fonts() {
            for font in Font::iter(Bytes::new(data)) {
                book.push(font.info().clone());
                fonts.push(font);
            }
        }
        let files = files
            .into_iter()
            .map(|(path, text)| {
                let id = FileId::new(None, VirtualPath::new(path));
                (id, Source::new(id, text))
            })
            .collect();
        Self {
            inner: Arc::new(Shared {
                library: LazyHash::new(Library::default()),
                book: LazyHash::new(book),
                fonts,
                files,
            }),
        }
    }

    /// An engine serving the `.typ` files of `<deck_dir>/typst/`.
    pub fn for_deck(deck_dir: &Path) -> Self {
        Self::with_files(deck_files(deck_dir))
    }

    /// Renders a display formula at `size` points in `color`, with
    /// `preamble` (Typst code, e.g. `#let` definitions) in scope. The
    /// result's `width` and `height` are the formula's natural size in
    /// points.
    pub fn render_math(&self, preamble: &str, source: &str, size: f64, color: Color) -> Result<MathImage, Error> {
        let text = format!(
            "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n#set text(size: {size}pt, fill: rgb(\"{}\"))\n{preamble}\n$ {source} $",
            color.to_hex()
        );
        self.render_document(&text)
    }

    fn render_document(&self, text: &str) -> Result<MathImage, Error> {
        let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
        let world = Sandbox { shared: &self.inner, main: Source::new(main_id, text.to_string()) };
        let document: PagedDocument =
            typst::compile(&world).output.map_err(|errors| Error::Typst(describe(&errors)))?;
        let page = document.pages.first().ok_or(Error::Empty)?;
        let svg = typst_svg::svg(page);
        let width = page.frame.width().to_pt();
        let height = page.frame.height().to_pt();
        Ok(MathImage { content: svg_children(&svg), width, height })
    }
}

impl MathRenderer for Engine {
    fn render_math(&self, preamble: &str, source: &str, size: f64, color: Color) -> Option<MathImage> {
        Engine::render_math(self, preamble, source, size, color).ok()
    }
}

/// The `.typ` files of `<deck_dir>/typst/`, by file name.
pub fn deck_files(deck_dir: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(deck_dir.join(TYPST_DIR)) else { return vec![] };
    let mut files: Vec<(String, String)> = entries
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "typ").unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            std::fs::read_to_string(e.path()).ok().map(|text| (format!("/{name}"), text))
        })
        .collect();
    files.sort();
    files
}

struct Sandbox<'a> {
    shared: &'a Shared,
    main: Source,
}

impl World for Sandbox<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.shared.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.shared.book
    }
    fn main(&self) -> FileId {
        self.main.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        self.shared.files.get(&id).cloned().ok_or_else(|| FileError::NotFound(id.vpath().as_rootless_path().into()))
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.shared.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

fn describe(errors: &[SourceDiagnostic]) -> String {
    errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; ")
}

/// The children of the root `<svg>` element of a typst-svg document, so
/// that they can be nested in another SVG with our own viewport.
fn svg_children(svg: &str) -> String {
    let start = svg.find('>').map(|i| i + 1).unwrap_or(0);
    let end = svg.rfind("</svg>").unwrap_or(svg.len());
    svg[start..end].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_math() {
        let e = Engine::new();
        let img = e.render_math("", "(-b plus.minus sqrt(b^2 - 4 a c)) / (2 a)", 24.0, Color::BLACK).unwrap();
        assert!(img.width > 50.0 && img.height > 20.0, "{} x {}", img.width, img.height);
        assert!(img.content.contains("<symbol"), "glyph outlines expected");
        assert!(!img.content.starts_with("<svg"));
        assert!(img.content.contains("#000000"));
        let red = e.render_math("", "x", 24.0, Color::rgb(0xCC, 0, 0)).unwrap();
        assert!(red.content.to_lowercase().contains("#cc0000"));
    }

    #[test]
    fn preamble_defines_notations() {
        let e = Engine::new();
        assert!(e.render_math("", "vb(x)", 20.0, Color::BLACK).is_err(), "undefined without the preamble");
        let img = e.render_math("#let vb(x) = math.bold(x)", "vb(x) + vb(y)", 20.0, Color::BLACK).unwrap();
        assert!(img.width > 10.0);
    }

    #[test]
    fn deck_files_are_importable() {
        let dir = std::env::temp_dir().join(format!("decker-typst-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(TYPST_DIR)).unwrap();
        std::fs::write(dir.join(TYPST_DIR).join("notation.typ"), "#let dd(f, x) = $(dif f) / (dif x)$\n").unwrap();
        std::fs::write(dir.join(TYPST_DIR).join("README.md"), "ignored").unwrap();
        let e = Engine::for_deck(&dir);
        assert_eq!(deck_files(&dir).len(), 1);
        let img = e.render_math("#import \"notation.typ\": *", "dd(f, x)", 20.0, Color::BLACK).unwrap();
        assert!(img.width > 10.0);
        assert!(e.render_math("#import \"missing.typ\": *", "x", 20.0, Color::BLACK).is_err());
        assert!(
            e.render_math("#import \"@preview/physica:0.9.3\": *", "x", 20.0, Color::BLACK).is_err(),
            "no registry"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reports_errors() {
        let e = Engine::new();
        assert!(matches!(e.render_math("", "#undefined-thing", 20.0, Color::BLACK), Err(Error::Typst(_))));
    }

    #[test]
    fn size_scales_the_glyphs() {
        let e = Engine::new();
        let a = e.render_math("", "x", 20.0, Color::BLACK).unwrap();
        let b = e.render_math("", "x", 40.0, Color::BLACK).unwrap();
        assert!((b.height / a.height - 2.0).abs() < 0.05);
    }
}
