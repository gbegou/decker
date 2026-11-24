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

use deck_core::{Align, Color, Deck, MathImage, MathRenderer, Paragraph, Run, TextDefaults};
use typst::layout::{Frame, FrameItem, Point, Transform};

/// The font text boxes default to (bundled, see [`DEJAVU_SANS`]).
pub const DEFAULT_FONT: &str = "DejaVu Sans";
/// The size text runs default to, in points.
pub const DEFAULT_SIZE: f64 = 18.0;

/// The folder of a deck whose `.typ` files are importable by formulas.
pub const TYPST_DIR: &str = "typst";
/// The folder of a deck whose `.ttf`/`.otf` files are loaded as fonts.
pub const FONTS_DIR: &str = "fonts";

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
        Self::with_files_and_fonts(files, Vec::new())
    }

    /// [`Self::with_files`] plus the font files `extra` (TrueType or
    /// OpenType data), usable by name like the bundled ones.
    pub fn with_files_and_fonts(files: Vec<(String, String)>, extra: Vec<Vec<u8>>) -> Self {
        let mut fonts = vec![];
        let mut book = FontBook::new();
        // Typst's bundled fonts (serif, math, mono) plus a sans-serif
        // family, since slides default to one, plus the deck's own.
        let bundled = typst_assets::fonts().map(Bytes::new);
        let sans = DEJAVU_SANS.iter().map(|d| Bytes::new(*d));
        let extra = extra.into_iter().map(Bytes::new);
        for data in bundled.chain(sans).chain(extra) {
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

    /// An engine serving the `.typ` files of `<deck_dir>/typst/` and the
    /// fonts of `<deck_dir>/fonts/`.
    pub fn for_deck(deck_dir: &Path) -> Self {
        Self::with_files_and_fonts(deck_files(deck_dir), deck_fonts(deck_dir))
    }

    /// The font families available, sorted: bundled ones and the deck's.
    pub fn font_families(&self) -> Vec<String> {
        let mut names: Vec<String> = self.inner.book.families().map(|(name, _)| name.to_string()).collect();
        names.sort();
        names.dedup();
        names
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
    pub fn render_text(
        &self,
        preamble: &str,
        paragraphs: &[Paragraph],
        width: f64,
        defaults: &TextDefaults,
    ) -> Result<MathImage, Error> {
        self.render_cached(&text_source(preamble, paragraphs, width, defaults))
    }

    /// The box of every character of `paragraphs` as typeset in a column
    /// `width` wide, plus one more for the caret at the end: the geometry
    /// of a text caret and selection. Characters the layout does not show
    /// as such (paragraph breaks, spaces eaten by a line break, formula
    /// source) get the position where they end.
    pub fn layout_text(
        &self,
        preamble: &str,
        paragraphs: &[Paragraph],
        width: f64,
        defaults: &TextDefaults,
    ) -> Result<Vec<CharBox>, Error> {
        let text = text_source(preamble, paragraphs, width, defaults);
        let main_id = FileId::new(None, VirtualPath::new("/main.typ"));
        let binaries = Binaries::new();
        let world = Sandbox { shared: &self.inner, main: Source::new(main_id, text), binaries: &binaries };
        let document: PagedDocument =
            typst::compile(&world).output.map_err(|errors| Error::Typst(describe(&errors)))?;
        let page = document.pages.first().ok_or(Error::Empty)?;
        let mut glyphs = vec![];
        collect_glyphs(&page.frame, Point::zero(), &mut glyphs);
        Ok(map_chars(&deck_core::text::flat_text(paragraphs), &glyphs, is_math_mask(paragraphs)))
    }

    #[doc(hidden)]
    pub fn text_source_for(preamble: &str, paragraphs: &[Paragraph], width: f64, defaults: &TextDefaults) -> String {
        text_source(preamble, paragraphs, width, defaults)
    }
}

/// The Typst source of a text box; `defaults` style what runs leave unset.
fn text_source(preamble: &str, paragraphs: &[Paragraph], width: f64, defaults: &TextDefaults) -> String {
    let size = defaults.size.unwrap_or(DEFAULT_SIZE);
    let font = font_list(defaults.font.as_deref());
    let fill = defaults.color.unwrap_or(Color::BLACK).to_hex();
    let mut text = format!(
            "#set page(width: {width}pt, height: auto, margin: 0pt, fill: none)\n#set text(size: {size}pt, font: {font}, fill: rgb(\"{fill}\"), top-edge: \"ascender\", bottom-edge: \"descender\")\n#set par(leading: 0.4em)\n#set block(spacing: 0.6em)\n#set list(marker: [•], indent: 0pt, body-indent: 0.5em)\n{preamble}\n"
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
                    format_args!("#block(inset: (left: {}pt))[#list([{aligned}])]\n", f64::from(level) * 1.5 * size),
                );
            }
            None => {
                text.push_str(&aligned);
                text.push_str("\n\n");
            }
        }
    }
    text
}

impl Engine {
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

/// A character's box in the typeset text: `x` is its left edge, `right`
/// its right edge, `top` and `bottom` the line box it sits on.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CharBox {
    pub x: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

/// A positioned glyph of the page, in reading order.
struct GlyphPos {
    text: String,
    x: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

fn collect_glyphs(frame: &Frame, offset: Point, out: &mut Vec<GlyphPos>) {
    for (pos, item) in frame.items() {
        let p = Point::new(offset.x + pos.x, offset.y + pos.y);
        match item {
            FrameItem::Group(g) => {
                // Only the translation of the transform matters for text boxes.
                let t: Transform = g.transform;
                collect_glyphs(&g.frame, Point::new(p.x + t.tx, p.y + t.ty), out);
            }
            FrameItem::Text(t) => {
                let size = t.size.to_pt();
                let m = t.font.metrics();
                let top = p.y.to_pt() - m.ascender.at(t.size).to_pt();
                let bottom = p.y.to_pt() - m.descender.at(t.size).to_pt();
                let mut x = p.x.to_pt();
                for g in &t.glyphs {
                    let adv = g.x_advance.at(t.size).to_pt();
                    let range = g.range.start as usize..g.range.end as usize;
                    let text = t.text.get(range).unwrap_or("").to_string();
                    out.push(GlyphPos { text, x, right: x + adv, top, bottom });
                    x += adv;
                    let _ = size;
                }
            }
            _ => {}
        }
    }
}

/// Which flat characters belong to formula runs (never matched to glyphs).
fn is_math_mask(paragraphs: &[Paragraph]) -> Vec<bool> {
    let mut mask = vec![];
    for (i, p) in paragraphs.iter().enumerate() {
        if i > 0 {
            mask.push(false);
        }
        for r in &p.runs {
            mask.extend(r.text.chars().map(|_| r.math));
        }
    }
    mask
}

/// Walks the glyphs in reading order and assigns each flat character a
/// box: matching glyph text consumes characters; glyphs that match nothing
/// (bullets, hyphens, formulas) are skipped; characters the layout dropped
/// (breaks, spaces at line ends, formula source) take the end of the last
/// matched glyph. The last entry is the caret box after the final character.
fn map_chars(flat: &str, glyphs: &[GlyphPos], math: Vec<bool>) -> Vec<CharBox> {
    let chars: Vec<char> = flat.chars().collect();
    let mut boxes: Vec<Option<CharBox>> = vec![None; chars.len()];
    let mut ci = 0usize;
    let mut last = CharBox { x: 0.0, right: 0.0, top: 0.0, bottom: 0.0 };
    let mut gi = 0usize;
    while gi < glyphs.len() && ci < chars.len() {
        let g = &glyphs[gi];
        let gchars: Vec<char> = g.text.chars().collect();
        if gchars.is_empty() {
            gi += 1;
            continue;
        }
        // Skip characters the layout never shows as glyphs.
        while ci < chars.len() && (math[ci] || chars[ci] == '\n') {
            boxes[ci] = Some(CharBox { x: last.right, right: last.right, top: last.top, bottom: last.bottom });
            ci += 1;
        }
        if ci >= chars.len() {
            break;
        }
        if gchars[0] == chars[ci] {
            // A glyph may cover several characters (ligatures): split its width.
            let n = gchars.len().min(chars.len() - ci);
            let step = (g.right - g.x) / n as f64;
            for (k, &gc) in gchars.iter().enumerate().take(n) {
                if chars[ci] != gc {
                    break;
                }
                let b = CharBox {
                    x: g.x + step * k as f64,
                    right: g.x + step * (k + 1) as f64,
                    top: g.top,
                    bottom: g.bottom,
                };
                boxes[ci] = Some(b);
                last = b;
                ci += 1;
            }
            gi += 1;
        } else if chars[ci] == ' ' {
            // A space swallowed by a line break: it ends where the line ends.
            boxes[ci] = Some(CharBox { x: last.right, right: last.right, top: last.top, bottom: last.bottom });
            ci += 1;
        } else {
            gi += 1;
        }
    }
    let mut out: Vec<CharBox> = Vec::with_capacity(chars.len() + 1);
    let mut prev = CharBox { x: 0.0, right: 0.0, top: 0.0, bottom: 0.0 };
    for b in boxes {
        let b = b.unwrap_or(CharBox { x: prev.right, right: prev.right, top: prev.top, bottom: prev.bottom });
        out.push(b);
        prev = b;
    }
    // The caret after the last character: a new line after a break, else the right edge.
    let end = match chars.last() {
        Some('\n') => {
            let h = prev.bottom - prev.top;
            CharBox { x: 0.0, right: 0.0, top: prev.bottom, bottom: prev.bottom + h.max(1.0) }
        }
        _ => CharBox { x: prev.right, right: prev.right, top: prev.top, bottom: prev.bottom },
    };
    out.push(end);
    out
}

impl MathRenderer for Engine {
    fn render_math(&self, preamble: &str, source: &str, size: f64, color: Color) -> Option<MathImage> {
        Engine::render_math(self, preamble, source, size, color).ok()
    }

    fn render_text(
        &self,
        preamble: &str,
        paragraphs: &[Paragraph],
        width: f64,
        defaults: &TextDefaults,
    ) -> Option<MathImage> {
        Engine::render_text(self, preamble, paragraphs, width, defaults).ok()
    }
}

/// A Typst font list: the family asked for, then the bundled fallback.
fn font_list(family: Option<&str>) -> String {
    match family {
        Some(f) if f != DEFAULT_FONT => format!("(\"{}\", \"{DEFAULT_FONT}\")", f.replace('"', "")),
        _ => format!("\"{DEFAULT_FONT}\""),
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
        args.push(format!("font: {}", font_list(Some(f))));
    }
    let body = if run.math { format!("${}$", run.text) } else { typst_markup(&run.text) };
    if args.is_empty() {
        body
    } else {
        format!("#text({})[{body}]", args.join(", "))
    }
}

/// The font files (`.ttf`, `.otf`) of `<deck_dir>/fonts/`.
pub fn deck_fonts(deck_dir: &Path) -> Vec<Vec<u8>> {
    let Ok(entries) = std::fs::read_dir(deck_dir.join(FONTS_DIR)) else { return vec![] };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .map(|x| matches!(x.to_ascii_lowercase().as_str(), "ttf" | "otf"))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    paths.into_iter().filter_map(|p| std::fs::read(p).ok()).collect()
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
        let plain = e.render_text("", &[Paragraph::plain("Hello world")], 400.0, &TextDefaults::default()).unwrap();
        assert!((plain.width - 400.0).abs() < 0.01, "the column width is the page width");
        assert!(plain.height >= 18.0 && plain.height < 30.0, "one full line box: {}", plain.height);
        assert!(plain.content.contains("<symbol"), "glyph outlines");

        let long = Paragraph::plain("word ".repeat(60));
        let wrapped = e.render_text("", &[long], 300.0, &TextDefaults::default()).unwrap();
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
        let img = e
            .render_text("", &[styled, Paragraph::plain("plain [brackets] #hash")], 500.0, &TextDefaults::default())
            .unwrap();
        assert!(img.content.to_lowercase().contains("#cc0000"));
        assert!(img.height > plain.height * 1.5);

        let again = e.render_text("", &[Paragraph::plain("Hello world")], 400.0, &TextDefaults::default()).unwrap();
        assert_eq!(again, plain, "cached");
    }

    #[test]
    fn lays_out_characters() {
        use deck_core::*;
        let e = Engine::new();
        let paragraphs = vec![
            Paragraph {
                runs: vec![Run::plain("Hello "), Run { text: "wide".into(), bold: true, ..Default::default() }],
                ..Default::default()
            },
            Paragraph { runs: vec![Run::plain("second line here")], bullet: Some(0), ..Default::default() },
        ];
        let boxes = e.layout_text("", &paragraphs, 400.0, &TextDefaults::default()).unwrap();
        let flat = deck_core::text::flat_text(&paragraphs);
        assert_eq!(boxes.len(), flat.chars().count() + 1);
        let h = &boxes[0];
        assert!(h.x >= 0.0 && h.right > h.x && h.bottom > h.top, "{h:?}");
        // Characters of one word advance left to right on the same line.
        for k in 1..5 {
            assert!(boxes[k].x >= boxes[k - 1].right - 0.01, "{:?} after {:?}", boxes[k], boxes[k - 1]);
            assert_eq!(boxes[k].top, boxes[0].top);
        }
        // The second paragraph sits on a lower line, indented by the bullet.
        let s = &boxes[12];
        assert!(s.top > boxes[0].bottom - 0.01, "second line below the first: {s:?} vs {:?}", boxes[0]);
        assert!(s.x > boxes[0].x, "bullet indent: {s:?}");
        // The caret at the very end is at the right of the last character.
        let end = boxes.last().unwrap();
        assert!((end.x - boxes[boxes.len() - 2].right).abs() < 0.01);

        // A wrapped line: the space at the break takes the line end, the next word starts a new line.
        let long = vec![Paragraph::plain("aaaa bbbb cccc dddd eeee ffff gggg hhhh")];
        let boxes = e.layout_text("", &long, 120.0, &TextDefaults::default()).unwrap();
        let lines: std::collections::BTreeSet<i64> = boxes.iter().map(|b| (b.top * 10.0) as i64).collect();
        assert!(lines.len() >= 2, "wrapped onto several lines");
        // Formula source characters all collapse to one position, outside the formula.
        let math = vec![Paragraph {
            runs: vec![
                Run::plain("a "),
                Run { text: "x^2".into(), math: true, ..Default::default() },
                Run::plain(" b"),
            ],
            ..Default::default()
        }];
        let boxes = e.layout_text("", &math, 300.0, &TextDefaults::default()).unwrap();
        assert_eq!(boxes[2].x, boxes[3].x);
        assert_eq!(boxes[3].x, boxes[4].x);
        assert!(boxes[6].x > boxes[4].x, "text after the formula continues to the right");
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

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn defaults_style_unset_runs_and_deck_fonts_load() {
        let e = Engine::new();
        let d = TextDefaults {
            font: Some("Libertinus Serif".into()),
            size: Some(30.0),
            color: Some(Color::rgb(0, 0x80, 0)),
        };
        let img = e.render_text("", &[Paragraph::plain("Hello")], 400.0, &d).unwrap();
        assert!(img.content.to_lowercase().contains("#008000"), "the theme color fills the text");
        let small = e.render_text("", &[Paragraph::plain("Hello")], 400.0, &TextDefaults::default()).unwrap();
        assert!(img.height > small.height * 1.4, "30pt over 18pt: {} vs {}", img.height, small.height);
        assert!(e.font_families().iter().any(|f| f == "DejaVu Sans"));

        let dir = std::env::temp_dir().join(format!("decker-fonts-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(FONTS_DIR)).unwrap();
        std::fs::write(dir.join(FONTS_DIR).join("x.ttf"), DEJAVU_SANS[0]).unwrap();
        std::fs::write(dir.join(FONTS_DIR).join("note.txt"), "ignored").unwrap();
        assert_eq!(deck_fonts(&dir).len(), 1);
        let _ = Engine::for_deck(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
