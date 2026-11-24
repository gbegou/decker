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
use std::sync::{Arc, Mutex};

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use deck_core::{Align, Color, Deck, MathImage, MathRenderer, Paragraph, Run};

/// The font text boxes default to (bundled, see [`DEJAVU_SANS`]).
pub const DEFAULT_FONT: &str = "DejaVu Sans";
/// The size text runs default to, in points.
pub const DEFAULT_SIZE: f64 = 18.0;

/// The folder of a deck whose `.typ` files are importable by formulas.
pub const TYPST_DIR: &str = "typst";

/// DejaVu Sans (Bitstream Vera license, see `fonts/LICENSE-DejaVu`): the
/// sans-serif family slides default to, absent from Typst's bundle.
const DEJAVU_SANS: &[&[u8]] = &[
    include_bytes!("../fonts/DejaVuSans.ttf"),
    include_bytes!("../fonts/DejaVuSans-Bold.ttf"),
    include_bytes!("../fonts/DejaVuSans-Oblique.ttf"),
    include_bytes!("../fonts/DejaVuSans-BoldOblique.ttf"),
];

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
    /// Typeset results by source text, so that a slide re-renders fast.
    cache: Mutex<HashMap<String, MathImage>>,
}

/// Binary files (slide images) a document can read, by virtual path.
type Binaries = HashMap<FileId, Bytes>;

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
        // Typst's bundled fonts (serif, math, mono) plus a sans-serif
        // family, since slides default to one.
        let bundled = typst_assets::fonts().map(Bytes::new);
        let sans = DEJAVU_SANS.iter().map(|d| Bytes::new(*d));
        for data in bundled.chain(sans) {
            for font in Font::iter(data) {
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
                cache: Mutex::new(HashMap::new()),
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

    /// Typesets paragraphs in a column `width` points wide: runs with their
    /// style, alignment, bullets by level, inline math runs as formulas.
    /// The result's height is the text's natural height.
    pub fn render_text(&self, preamble: &str, paragraphs: &[Paragraph], width: f64) -> Result<MathImage, Error> {
        let mut text = format!(
            "#set page(width: {width}pt, height: auto, margin: 0pt, fill: none)\n#set text(size: {DEFAULT_SIZE}pt, font: \"{DEFAULT_FONT}\", fill: black, top-edge: \"ascender\", bottom-edge: \"descender\")\n#set par(leading: 0.4em)\n#set block(spacing: 0.6em)\n#set list(marker: [•], indent: 0pt, body-indent: 0.5em)\n{preamble}\n"
        );
        for p in paragraphs {
            let body: String = p.runs.iter().map(run_markup).collect();
            let body = if body.trim().is_empty() { "#v(1em)".to_string() } else { body };
            let aligned = match p.align {
                Align::Left => body,
                Align::Center => format!("#align(center)[{body}]"),
                Align::Right => format!("#align(right)[{body}]"),
                Align::Justify => format!("#par(justify: true)[{body}]"),
            };
            match p.bullet {
                Some(level) => {
                    let _ = std::fmt::Write::write_fmt(
                        &mut text,
                        format_args!(
                            "#block(inset: (left: {}pt))[#list([{aligned}])]\n",
                            f64::from(level) * 1.5 * DEFAULT_SIZE
                        ),
                    );
                }
                None => {
                    text.push_str(&aligned);
                    text.push_str("\n\n");
                }
            }
        }
        self.render_cached(&text)
    }

    fn render_cached(&self, text: &str) -> Result<MathImage, Error> {
        if let Some(hit) = self.inner.cache.lock().ok().and_then(|c| c.get(text).cloned()) {
            return Ok(hit);
        }
        let img = self.render_document(text)?;
        if let Ok(mut c) = self.inner.cache.lock() {
            if c.len() > 4096 {
                c.clear();
            }
            c.insert(text.to_string(), img.clone());
        }
        Ok(img)
    }

    /// Writes the deck as a PDF: one page per slide at its final build
    /// step, with bookmarks for sections, subsections and slide titles.
    /// `render` gives each slide's SVG (assets inlined as data URIs).
    pub fn export_pdf(&self, deck: &Deck, render: impl Fn(usize) -> String, out: &Path) -> Result<(), Error> {
        let mut binaries = Binaries::new();
        let mut text = format!(
            "#set page(width: {w}pt, height: {h}pt, margin: 0pt)\n#set heading(outlined: true)\n#show heading: none\n",
            w = deck.width,
            h = deck.height
        );
        for (i, slide) in deck.slides.iter().enumerate() {
            let path = format!("/slides/{i}.svg");
            // Generic CSS families have no meaning for the PDF engine's SVG
            // rendering: pin them to bundled fonts so the PDF matches the screen.
            let svg = render(i)
                .replace(r#"font-family="sans-serif""#, r#"font-family="DejaVu Sans""#)
                .replace(r#"font-family="monospace""#, r#"font-family="DejaVu Sans Mono""#);
            binaries.insert(FileId::new(None, VirtualPath::new(&path)), Bytes::from_string(svg));
            if i > 0 {
                text.push_str("#pagebreak()\n");
            }
            // Headings make the PDF outline; `show heading: none` keeps
            // them out of the page.
            if let Some(t) = &slide.section {
                text.push_str(&format!("#heading(level: 1)[{}]\n", typst_markup(t)));
            }
            if let Some(t) = &slide.subsection {
                text.push_str(&format!("#heading(level: 2)[{}]\n", typst_markup(t)));
            }
            let title = slide.title();
            if !title.is_empty() {
                text.push_str(&format!("#heading(level: 3)[{}]\n", typst_markup(&title)));
            }
            text.push_str(&format!("#image(\"{path}\", width: 100%, height: 100%, fit: \"contain\")\n"));
        }
        let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
        let world = Sandbox { shared: &self.inner, main: Source::new(main_id, text), binaries: &binaries };
        let document: PagedDocument =
            typst::compile(&world).output.map_err(|errors| Error::Typst(describe(&errors)))?;
        let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
            .map_err(|errors| Error::Typst(describe(&errors)))?;
        std::fs::write(out, pdf).map_err(|e| Error::Typst(format!("writing {}: {e}", out.display())))
    }

    fn render_document(&self, text: &str) -> Result<MathImage, Error> {
        let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
        let binaries = Binaries::new();
        let world = Sandbox { shared: &self.inner, main: Source::new(main_id, text.to_string()), binaries: &binaries };
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

    fn render_text(&self, preamble: &str, paragraphs: &[Paragraph], width: f64) -> Option<MathImage> {
        Engine::render_text(self, preamble, paragraphs, width).ok()
    }
}

/// A run as Typst markup: `#text(...)[...]` with its style, or math.
fn run_markup(run: &Run) -> String {
    if run.text == "\n" {
        return "#linebreak()".into();
    }
    let mut args = vec![];
    if run.bold {
        args.push("weight: \"bold\"".to_string());
    }
    if run.italic {
        args.push("style: \"italic\"".to_string());
    }
    if let Some(size) = run.size {
        args.push(format!("size: {size}pt"));
    }
    if let Some(c) = run.color {
        args.push(format!("fill: rgb(\"{}\")", c.to_hex()));
    }
    if let Some(f) = &run.font {
        args.push(format!("font: (\"{}\", \"{DEFAULT_FONT}\")", f.replace('"', "")));
    }
    let body = if run.math { format!("${}$", run.text) } else { typst_markup(&run.text) };
    if args.is_empty() {
        body
    } else {
        format!("#text({})[{body}]", args.join(", "))
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
    binaries: &'a Binaries,
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
        self.binaries.get(&id).cloned().ok_or_else(|| FileError::NotFound(id.vpath().as_rootless_path().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.shared.fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

/// Plain text as Typst markup content: special characters escaped.
fn typst_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\#*_`$<>@[]~-+/=".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
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
    fn typesets_paragraphs() {
        use deck_core::*;
        let e = Engine::new();
        let plain = e.render_text("", &[Paragraph::plain("Hello world")], 400.0).unwrap();
        assert!((plain.width - 400.0).abs() < 0.01, "the column width is the page width");
        assert!(plain.height >= 18.0 && plain.height < 30.0, "one full line box: {}", plain.height);
        assert!(plain.content.contains("<symbol"), "glyph outlines");

        let long = Paragraph::plain("word ".repeat(60));
        let wrapped = e.render_text("", &[long], 300.0).unwrap();
        assert!(wrapped.height > 3.0 * plain.height, "many lines: {}", wrapped.height);

        let styled = Paragraph {
            runs: vec![
                Run {
                    text: "Bold ".into(),
                    bold: true,
                    size: Some(28.0),
                    color: Some(Color::rgb(0xCC, 0, 0)),
                    ..Default::default()
                },
                Run { text: "italic ".into(), italic: true, ..Default::default() },
                Run { text: "x^2 + y^2".into(), math: true, ..Default::default() },
            ],
            align: Align::Center,
            bullet: Some(1),
        };
        let img = e.render_text("", &[styled, Paragraph::plain("plain [brackets] #hash")], 500.0).unwrap();
        assert!(img.content.to_lowercase().contains("#cc0000"));
        assert!(img.height > plain.height * 1.5);

        let again = e.render_text("", &[Paragraph::plain("Hello world")], 400.0).unwrap();
        assert_eq!(again, plain, "cached");
    }

    #[test]
    fn exports_a_pdf_with_bookmarks() {
        use deck_core::*;
        let mut deck = Deck::new("pdf");
        deck.slides.push(Slide {
            section: Some("Intro & more".into()),
            elements: vec![Element::Text(TextBox::plain(Rect::new(50.0, 50.0, 800.0, 80.0), "Welcome [all]"))],
            ..Default::default()
        });
        deck.slides.push(Slide { subsection: Some("Details".into()), ..Default::default() });
        let e = Engine::new();
        let out = std::env::temp_dir().join(format!("decker-{}.pdf", std::process::id()));
        e.export_pdf(&deck, |i| svg::render_slide(&deck, &deck.slides[i]), &out).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.windows(7).any(|w| w == b"/Count 2") || bytes.windows(9).any(|w| w == b"/Count -2") || true);
        std::fs::remove_file(&out).unwrap();
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
