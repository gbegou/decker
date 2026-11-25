//! Application state and the view sent to the windows.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

use deck_core::{
    native, svg, text, transfer, Color, Crop, Deck, Element, Layout, Mask, MathRenderer, Media, Paragraph, Placeholder,
    Play, Rect, Slide, Step, Stroke, TextBox, TextDefaults, Theme, ThemeSet,
};

#[derive(Default)]
pub struct AppState {
    pub deck: Option<Deck>,
    /// The formula renderer: Typst when the `typst` feature is on.
    pub math: Math,
    /// The `.deck` folder, empty for an unsaved new deck.
    pub dir: PathBuf,
    pub slide: usize,
    pub step: u32,
    pub presenting: bool,
    /// `""`, `"black"` or `"white"`: the audience window shows a blank
    /// screen of that color instead of the slide (B and W keys).
    pub blank: String,
    /// The layout being edited instead of a slide, by name.
    pub editing_layout: Option<String>,
    /// The theme being edited instead of a slide, by name (its background,
    /// shared elements and placeholder styles).
    pub editing_theme: Option<String>,
    pub warnings: Vec<String>,
    /// Rendered thumbnails, one per slide; `None` when stale.
    thumbs: Vec<Option<String>>,
    /// Images as data URIs, by deck-relative path.
    pub assets: HashMap<String, String>,
    /// The table cell being edited in place: `(element, row, column)`.
    /// Text edits and styles aimed at that table go to this cell.
    pub cell: Option<(usize, usize, usize)>,
    /// The text element being edited and its caret, `(element, caret)`:
    /// a formula typed as `$...$` around the caret shows as its source.
    pub raw: Option<(usize, usize)>,
    /// The `.deckz` file the deck came from or was last saved to: saving
    /// packs the working folder back into it.
    pub archive: Option<PathBuf>,
    /// Snapshots before each edit, oldest first, and the ones undone.
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// What the last snapshot was taken for: consecutive typing in one
    /// box merges into a single undo step.
    last_edit: Option<String>,
}

/// A placeholder's default text style (see [`AppState::style_placeholder`]);
/// an empty string clears a color or font, a size of 0 the size.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PlaceholderStyle {
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub align: Option<deck_core::Align>,
    #[serde(default)]
    pub bullets: Option<bool>,
}

/// Element-level style change (see [`AppState::style_element`]).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ElementStyle {
    #[serde(default)]
    pub fill: Option<String>,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub stroke_width: Option<f64>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub highlight: Option<String>,
    #[serde(default)]
    pub subsections: Option<bool>,
    /// Media: `appear` or `button`.
    #[serde(default)]
    pub play: Option<String>,
    #[serde(default)]
    pub looping: Option<bool>,
    #[serde(default)]
    pub sound: Option<bool>,
    /// Lines and arrows: flip the direction.
    #[serde(default)]
    pub flip: Option<bool>,
}

/// A theme edit (see [`AppState::set_theme`]): `Some("")` clears a field.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ThemeChange {
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub palette: Option<Vec<String>>,
}

/// The theme shown in the UI: `own` is what the edited target (deck or
/// layout) sets itself, `effective` what the slide actually gets.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ThemeInfo {
    /// The theme a slide's fonts and colors come from (on a slide).
    pub of_theme: Option<String>,
    pub own: Theme,
    pub effective: Theme,
    /// Font families the typesetter knows, bundled and from `fonts/`.
    pub fonts: Vec<String>,
}

/// The character boxes of a text element, in slide points.
#[derive(Debug, Clone, Serialize)]
pub struct TextLayout {
    /// The box being edited: the element's, or a table cell's.
    pub rect: Rect,
    pub text: String,
    /// One box per character plus the caret box at the end.
    pub chars: Vec<CharRect>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CharRect {
    pub x: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

/// The formula renderer, chosen at compile time. With Typst, it serves
/// the `typst/*.typ` files of the open deck.
pub struct Math(Box<dyn MathRenderer + Send + Sync>, #[allow(dead_code)] Option<deck_typst_engine::Engine>);

/// The Typst engine type when the feature is on, else a stand-in.
#[cfg(feature = "typst")]
mod deck_typst_engine {
    pub use deck_typst::Engine;
}
#[cfg(not(feature = "typst"))]
mod deck_typst_engine {
    #[derive(Clone)]
    pub struct Engine;
}

impl Default for Math {
    fn default() -> Self {
        Self::for_deck(Path::new(""))
    }
}

impl Math {
    #[cfg(feature = "typst")]
    pub fn for_deck(dir: &Path) -> Self {
        let engine = deck_typst::Engine::for_deck(dir);
        Math(Box::new(engine.clone()), Some(engine))
    }

    #[cfg(not(feature = "typst"))]
    pub fn for_deck(_dir: &Path) -> Self {
        Math(Box::new(deck_core::NoMath), None)
    }

    /// Character boxes in box coordinates, when the typesetter is Typst.
    #[cfg(feature = "typst")]
    fn layout(
        &self,
        preamble: &str,
        paragraphs: &[Paragraph],
        width: f64,
        defaults: &TextDefaults,
    ) -> Option<Vec<deck_typst::CharBox>> {
        self.1.as_ref().and_then(|e| e.layout_text(preamble, paragraphs, width, defaults).ok())
    }

    #[cfg(not(feature = "typst"))]
    fn layout(&self, _: &str, _: &[Paragraph], _: f64, _: &TextDefaults) -> Option<Vec<NoBox>> {
        None
    }

    #[cfg(feature = "typst")]
    fn fonts(&self) -> Vec<String> {
        self.1.as_ref().map(|e| e.font_families()).unwrap_or_default()
    }

    #[cfg(not(feature = "typst"))]
    fn fonts(&self) -> Vec<String> {
        vec![]
    }
}

#[cfg(not(feature = "typst"))]
#[allow(dead_code)]
pub struct NoBox {
    pub x: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

/// Everything a window needs to draw itself.
#[derive(Debug, Clone, Serialize)]
pub struct View {
    pub title: String,
    pub dir: String,
    pub width: f64,
    pub height: f64,
    pub slide: usize,
    pub slide_count: usize,
    pub step: u32,
    pub last_step: u32,
    /// The current slide at the current step, assets inlined.
    pub svg: String,
    /// What "next" shows: the next step, or the next slide's first step.
    pub next_svg: Option<String>,
    pub notes: String,
    /// The current slide's elements, for selection and editing.
    pub elements: Vec<ElementInfo>,
    pub presenting: bool,
    pub blank: String,
    pub warnings: Vec<String>,
    /// The deck's layout names, for the layout selector.
    pub layouts: Vec<String>,
    /// The current slide's layout.
    pub layout: Option<String>,
    /// The section and subsection the current slide starts (editable).
    pub section: Option<String>,
    pub subsection: Option<String>,
    /// The section and subsection the current slide belongs to.
    pub in_section: Option<String>,
    pub in_subsection: Option<String>,
    /// Per slide: the section and subsection it starts, for the outline
    /// shown in the slide stack.
    pub starts: Vec<SlideStart>,
    /// The layout being edited, when the canvas shows a layout.
    pub editing_layout: Option<String>,
    /// The theme being edited, when the canvas shows a theme.
    pub editing_theme: Option<String>,
    /// The layouts by theme, for the layout menus; layouts without a theme
    /// under an empty name, last.
    pub layout_groups: Vec<LayoutGroup>,
    /// The layout under edit: its theme and what it overrides.
    pub layout_meta: Option<LayoutMeta>,
    /// The theme under edit: the default style of each kind of placeholder.
    pub theme_placeholders: Vec<Placeholder>,
    /// The theme of the layout under edit, else of the current slide.
    pub theme: ThemeInfo,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlideStart {
    pub section: Option<String>,
    pub subsection: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElementInfo {
    pub index: usize,
    pub kind: &'static str,
    pub rect: Rect,
    /// Degrees clockwise about the box's center, as shown at the viewed click.
    pub rotation: f64,
    pub text: String,
    pub last_step: u32,
    /// Size of the first run in points, for the in-place editor.
    pub size: f64,
    /// Images: the crop and mask.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<Mask>,
    /// The element's build steps.
    pub steps: Vec<Step>,
    /// Fill color of text boxes and shapes, text color of formulas, muted color of the plan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subsections: Option<bool>,
    /// Media: playback settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaInfo>,
    /// Shapes: `rectangle`, `ellipse`, `line` or `arrow`, and whether a line is flipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<&'static str>,
    pub flip: bool,
    /// Placeholders: their default text style.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<Placeholder>,
    /// Tables: the cells' texts by row, and the settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<TableInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub cells: Vec<Vec<String>>,
    pub fills: Vec<Vec<Option<Color>>>,
    pub header: bool,
    pub columns: Vec<f64>,
    /// The cells' boxes on the slide, to find the cell under a click; a
    /// merged cell's box spans its neighbours, a covered cell's is empty.
    pub rects: Vec<Vec<Rect>>,
    /// `(colspan, rowspan)` per cell; `(0, 0)` for a covered cell.
    pub spans: Vec<Vec<(usize, usize)>>,
}

fn table_info(e: &Element, shown: &Rect) -> Option<TableInfo> {
    let Element::Table(t) = e else { return None };
    Some(TableInfo {
        rects: (0..t.rows.len())
            .map(|i| {
                (0..t.rows[i].len())
                    .map(|j| {
                        if t.covered(i, j) {
                            let r = t.cell_rect(shown, i, j);
                            Rect::new(r.x, r.y, 0.0, 0.0)
                        } else {
                            t.span_rect(shown, i, j)
                        }
                    })
                    .collect()
            })
            .collect(),
        spans: (0..t.rows.len())
            .map(|i| {
                (0..t.rows[i].len())
                    .map(|j| if t.covered(i, j) { (0, 0) } else { (t.rows[i][j].colspan, t.rows[i][j].rowspan) })
                    .collect()
            })
            .collect(),
        cells: t.rows.iter().map(|r| r.iter().map(deck_core::Cell::text).collect()).collect(),
        fills: t.rows.iter().map(|r| r.iter().map(|c| c.fill).collect()).collect(),
        header: t.header,
        columns: t.columns.clone(),
    })
}

/// An edit of a table (see [`AppState::edit_table`]).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TableEdit {
    /// Sets a cell's text (one paragraph per line, keeping the first run's style).
    Cell {
        row: usize,
        column: usize,
        text: String,
    },
    /// A cell's fill: `""` clears it.
    Fill {
        row: usize,
        column: usize,
        color: String,
    },
    /// Inserts an empty row or column before `at` (at the end when past it).
    AddRow {
        at: usize,
    },
    AddColumn {
        at: usize,
    },
    RemoveRow {
        at: usize,
    },
    RemoveColumn {
        at: usize,
    },
    Header {
        on: bool,
    },
    /// Relative column widths; empty for equal columns.
    Columns {
        widths: Vec<f64>,
    },
    /// Merges the cells from one corner to the other (inclusive).
    Merge {
        from: (usize, usize),
        to: (usize, usize),
    },
    /// Splits the merged cell covering this one.
    Split {
        row: usize,
        column: usize,
    },
}

fn shape_info(e: &Element) -> (Option<&'static str>, bool) {
    match e {
        Element::Shape(s) => (
            Some(match s.kind {
                deck_core::ShapeKind::Rectangle => "rectangle",
                deck_core::ShapeKind::Ellipse => "ellipse",
                deck_core::ShapeKind::Line => "line",
                deck_core::ShapeKind::Arrow => "arrow",
            }),
            s.flip,
        ),
        _ => (None, false),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaInfo {
    pub play: Play,
    pub looping: bool,
    pub sound: bool,
    pub duration: Option<f64>,
    pub poster: Option<String>,
}

fn media_info(e: &Element) -> Option<MediaInfo> {
    match e {
        Element::Media(m) => Some(MediaInfo {
            play: m.play,
            looping: m.looping,
            sound: m.sound,
            duration: m.duration,
            poster: m.poster.clone(),
        }),
        _ => None,
    }
}

fn element_style_info(e: &Element) -> (Option<Color>, Option<Stroke>, Option<Color>, Option<bool>) {
    match e {
        Element::Text(t) => (t.fill, t.stroke, None, None),
        Element::Shape(s) => (s.fill, s.stroke, None, None),
        Element::Math(m) => (m.color, None, None, None),
        Element::Outline(o) => (o.color, None, o.highlight, Some(o.subsections)),
        Element::Table(t) => (None, t.stroke, None, None),
        Element::Image(_) | Element::Media(_) | Element::Figure(_) | Element::Group(_) => (None, None, None, None),
    }
}

/// The deck and where the user was, restored by undo.
/// The layouts of one theme.
#[derive(Debug, Clone, Serialize)]
pub struct LayoutGroup {
    pub theme: String,
    pub layouts: Vec<String>,
}

/// A layout under edit, against its theme.
#[derive(Debug, Clone, Serialize)]
pub struct LayoutMeta {
    pub parent: Option<String>,
    pub hide_theme_elements: bool,
    /// What it overrides, in words; empty when it follows its theme.
    pub overrides: Vec<String>,
    pub themes: Vec<String>,
}

#[derive(Debug, Clone)]
struct Snapshot {
    deck: Deck,
    slide: usize,
    editing_layout: Option<String>,
    editing_theme: Option<String>,
}

const UNDO_LIMIT: usize = 200;

/// The source a new figure starts with: a small plot, to edit.
pub const SAMPLE_FIGURE: &str = r#"#import "@preview/cetz:0.5.2": canvas, draw
#import "@preview/cetz-plot:0.1.4": plot
#canvas({
  plot.plot(size: (12, 6), x-label: [x], y-label: [y], {
    plot.add(((0, 1), (1, 3), (2, 2), (3, 4)), mark: "o")
  })
})"#;

/// What the app needs from an animated GIF: its frame count, length in
/// seconds and first frame as PNG (the poster).
struct GifInfo {
    frames: usize,
    duration: f64,
    first_frame_png: Vec<u8>,
}

fn gif_info(bytes: &[u8]) -> Option<GifInfo> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(bytes).ok()?;
    let (w, h) = (u32::from(decoder.width()), u32::from(decoder.height()));
    let mut frames = 0;
    let mut centiseconds = 0u64;
    let mut first = None;
    while let Ok(Some(frame)) = decoder.read_next_frame() {
        if first.is_none() {
            // The first frame may be smaller than the canvas: paste it.
            let mut canvas = vec![0u8; (w * h * 4) as usize];
            for row in 0..u32::from(frame.height) {
                let y = u32::from(frame.top) + row;
                if y >= h {
                    break;
                }
                for col in 0..u32::from(frame.width) {
                    let x = u32::from(frame.left) + col;
                    if x >= w {
                        break;
                    }
                    let src = ((row * u32::from(frame.width) + col) * 4) as usize;
                    let dst = ((y * w + x) * 4) as usize;
                    canvas[dst..dst + 4].copy_from_slice(&frame.buffer[src..src + 4]);
                }
            }
            first = Some(canvas);
        }
        // Browsers treat a 0 or 1 cs delay as 10 cs.
        centiseconds += u64::from(frame.delay.max(2)).max(if frame.delay <= 1 { 10 } else { 0 });
        frames += 1;
    }
    let first = first?;
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&first).ok()?;
    }
    Some(GifInfo { frames, duration: centiseconds as f64 / 100.0, first_frame_png: png_bytes })
}

/// Whether an element (or a group) holds an image.
fn has_image(e: &Element) -> bool {
    match e {
        Element::Image(_) | Element::Media(_) => true,
        Element::Group(g) => g.children.iter().any(has_image),
        _ => false,
    }
}

impl AppState {
    /// Records the state before an edit named `kind`. Edits of the same
    /// kind in a row (typing in one box, dragging) share one snapshot
    /// when `merge` is set.
    pub fn checkpoint(&mut self, kind: &str, merge: bool) {
        let Some(deck) = self.deck.as_ref() else { return };
        if merge && self.last_edit.as_deref() == Some(kind) {
            return;
        }
        self.undo.push(Snapshot {
            deck: deck.clone(),
            slide: self.slide,
            editing_layout: self.editing_layout.clone(),
            editing_theme: self.editing_theme.clone(),
        });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.last_edit = Some(kind.to_string());
    }

    /// Ends a run of mergeable edits (a different edit, a navigation).
    pub fn break_merge(&mut self) {
        self.last_edit = None;
    }

    pub fn undo(&mut self) -> Result<(), String> {
        let snap = self.undo.pop().ok_or("nothing to undo")?;
        let current = self.swap_in(snap);
        self.redo.push(current);
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        let snap = self.redo.pop().ok_or("nothing to redo")?;
        let current = self.swap_in(snap);
        self.undo.push(current);
        Ok(())
    }

    /// Whether undo and redo have something to do.
    pub fn can_undo_redo(&self) -> (bool, bool) {
        (!self.undo.is_empty(), !self.redo.is_empty())
    }

    fn swap_in(&mut self, snap: Snapshot) -> Snapshot {
        let current = Snapshot {
            deck: self.deck.take().unwrap_or_else(|| Deck::new("")),
            slide: self.slide,
            editing_layout: self.editing_layout.clone(),
            editing_theme: self.editing_theme.clone(),
        };
        self.slide = snap.slide.min(snap.deck.slides.len().saturating_sub(1));
        self.step = 0;
        self.editing_layout = snap.editing_layout.filter(|n| snap.deck.layout(n).is_some());
        self.editing_theme = snap.editing_theme.filter(|n| snap.deck.theme_set(n).is_some());
        self.deck = Some(snap.deck);
        self.last_edit = None;
        self.touch(None);
        current
    }

    /// Swaps in a deck a script produced, keeping the folder and history.
    pub fn replace_deck(&mut self, deck: Deck, slide: Option<usize>) {
        self.slide = slide.unwrap_or(self.slide).min(deck.slides.len().saturating_sub(1));
        self.step = 0;
        if self.editing_theme.as_deref().is_some_and(|n| deck.theme_set(n).is_none()) {
            self.editing_theme = None;
        }
        if let Some(n) = &self.editing_layout {
            if deck.layout(n).is_none() {
                self.editing_layout = None;
            }
        }
        self.deck = Some(deck);
        self.touch(None);
        self.assets.clear();
    }

    pub fn load(&mut self, deck: Deck, dir: PathBuf) {
        self.cell = None;
        self.undo.clear();
        self.redo.clear();
        self.last_edit = None;
        self.math = Math::for_deck(&dir);
        self.thumbs = vec![None; deck.slides.len()];
        self.assets.clear();
        self.deck = Some(deck);
        self.dir = dir;
        self.slide = 0;
        self.step = 0;
        self.warnings.clear();
    }

    /// Marks the slide's thumbnail stale after an edit (`None`: all).
    fn touch(&mut self, slide: Option<usize>) {
        let n = self.deck.as_ref().map(|d| d.slides.len()).unwrap_or(0);
        self.thumbs.resize(n, None);
        match slide {
            Some(i) => {
                if let Some(t) = self.thumbs.get_mut(i) {
                    *t = None;
                }
            }
            None => self.thumbs.iter_mut().for_each(|t| *t = None),
        }
    }

    fn render(&self, deck: &Deck, slide: &Slide, step: u32) -> String {
        let svg = match self.raw_slide(slide) {
            Some(copy) => svg::render_copy_with(deck, self.slide, &copy, step, &*self.math.0),
            None => svg::render_step_with(deck, slide, step, &*self.math.0),
        };
        inline_assets(&svg, &self.dir, &self.assets)
    }

    /// The current slide with the formula under the caret shown as source,
    /// when `slide` is the current slide and there is one.
    fn raw_slide(&self, slide: &Slide) -> Option<Slide> {
        let (element, caret) = self.raw?;
        if self.editing_template() || !std::ptr::eq(self.current()?.1, slide) {
            return None;
        }
        let mut slide = slide.clone();
        let paragraphs = match slide.elements.get_mut(element)? {
            Element::Text(t) => &mut t.paragraphs,
            Element::Shape(s) => &mut s.paragraphs,
            _ => return None,
        };
        text::raw_formula(paragraphs, caret).then_some(slide)
    }

    /// Sets the caret of the text being edited (`None` when done); returns
    /// whether the slide's look changed, a formula opening or closing.
    pub fn set_caret(&mut self, raw: Option<(usize, usize)>) -> bool {
        let shown = |s: &Self| {
            s.current().and_then(|(_, slide)| s.raw_slide(slide)).map(|s| serde_json::to_string(&s).unwrap_or_default())
        };
        let before = shown(self);
        self.raw = raw;
        shown(self) != before
    }

    /// The typesetter in use (Typst, or none without the feature).
    pub fn typesetter(&self) -> &dyn MathRenderer {
        &*self.math.0
    }

    /// The slide with videos and GIFs as their poster: thumbnails, PDF.
    fn render_still(&self, deck: &Deck, slide: &Slide, step: u32) -> String {
        inline_assets(&svg::render_step_still(deck, slide, step, &*self.math.0), &self.dir, &self.assets)
    }

    /// Thumbnails of every slide at its final step, rendering only the
    /// stale ones, and caching image data on the way.
    pub fn thumbnails(&mut self) -> Vec<String> {
        let Some(deck) = self.deck.take() else { return vec![] };
        self.thumbs.resize(deck.slides.len(), None);
        for (i, slide) in deck.slides.iter().enumerate() {
            if self.thumbs[i].is_none() {
                self.cache_assets(slide);
                self.thumbs[i] = Some(self.render_still(&deck, slide, slide.last_step()));
            }
        }
        self.deck = Some(deck);
        self.thumbs.iter().map(|t| t.clone().unwrap_or_default()).collect()
    }

    /// Reads and encodes the slide's images not yet in the cache.
    fn cache_assets(&mut self, slide: &Slide) {
        fn collect(e: &Element, out: &mut Vec<String>) {
            match e {
                Element::Image(i) => out.push(i.src.clone()),
                Element::Media(m) => out.extend(m.poster.clone()),
                Element::Group(g) => g.children.iter().for_each(|c| collect(c, out)),
                _ => {}
            }
        }
        let mut all = vec![];
        slide.elements.iter().for_each(|e| collect(e, &mut all));
        let srcs: Vec<String> = all.into_iter().filter(|src| !self.assets.contains_key(src)).collect();
        for src in srcs {
            if let Some(uri) = data_uri(&self.dir.join(&src)) {
                self.assets.insert(src, uri);
            }
        }
    }

    fn current(&self) -> Option<(&Deck, &Slide)> {
        let deck = self.deck.as_ref()?;
        let slide = deck.slides.get(self.slide)?;
        Some((deck, slide))
    }

    pub fn view(&mut self) -> View {
        if let Some(deck) = self.deck.take() {
            if let Some(slide) = deck.slides.get(self.slide) {
                self.cache_assets(slide);
            }
            if let Some(next) = deck.slides.get(self.slide + 1) {
                self.cache_assets(next);
            }
            if let Some(layout) = deck.slides.get(self.slide).and_then(|s| deck.layout_of(s)) {
                let fake = Slide { elements: layout.elements.clone(), ..Default::default() };
                self.cache_assets(&fake);
            }
            self.deck = Some(deck);
        }
        let Some((deck, slide)) = self.current() else {
            return View {
                title: String::new(),
                dir: self.dir.display().to_string(),
                width: deck_core::DEFAULT_WIDTH,
                height: deck_core::DEFAULT_HEIGHT,
                slide: 0,
                slide_count: 0,
                step: 0,
                last_step: 0,
                svg: String::new(),
                next_svg: None,
                notes: String::new(),
                elements: vec![],
                presenting: self.presenting,
                blank: self.blank.clone(),
                warnings: self.warnings.clone(),
                layouts: vec![],
                layout: None,
                section: None,
                subsection: None,
                in_section: None,
                in_subsection: None,
                starts: vec![],
                editing_layout: None,
                editing_theme: None,
                layout_groups: vec![],
                layout_meta: None,
                theme_placeholders: vec![],
                theme: ThemeInfo::default(),
                can_undo: false,
                can_redo: false,
            };
        };
        let (can_undo, can_redo) = self.can_undo_redo();
        if let Some(theme) = self.editing_theme.as_deref().and_then(|n| deck.theme_set(n)) {
            return self.theme_view(deck, theme);
        }
        if let Some(layout) = self.editing_layout.as_deref().and_then(|n| deck.layout(n)) {
            return self.layout_view(deck, layout);
        }
        let render = |s: &Slide, step: u32| self.render(deck, s, step);
        let next_svg = if self.step < slide.last_step() {
            Some(render(slide, self.step + 1))
        } else {
            deck.slides.get(self.slide + 1).map(|s| render(s, 0))
        };
        View {
            title: deck.title.clone(),
            dir: self.dir.display().to_string(),
            width: deck.width,
            height: deck.height,
            slide: self.slide,
            slide_count: deck.slides.len(),
            step: self.step,
            last_step: slide.last_step(),
            svg: render(slide, self.step),
            next_svg,
            notes: slide.notes.clone(),
            elements: slide
                .elements
                .iter()
                .enumerate()
                .map(|(index, e)| ElementInfo {
                    index,
                    kind: match e {
                        Element::Text(_) => "text",
                        Element::Image(_) => "image",
                        Element::Media(_) => "media",
                        Element::Figure(_) => "figure",
                        Element::Table(_) => "table",
                        Element::Shape(_) => "shape",
                        Element::Math(_) => "math",
                        Element::Outline(_) => "outline",
                        Element::Group(_) => "group",
                    },
                    // As shown at the viewed click: a move step changes it.
                    rect: e.state_at(self.step).rect,
                    rotation: e.state_at(self.step).rotation,
                    text: match e {
                        Element::Text(t) => t.text(),
                        Element::Shape(s) => s.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n"),
                        Element::Math(m) => m.source.clone(),
                        Element::Image(i) => i.src.clone(),
                        Element::Media(m) => m.src.clone(),
                        Element::Figure(f) => f.source.clone(),
                        Element::Table(t) => format!("{} x {}", t.rows.len(), t.column_count()),
                        Element::Outline(_) => String::new(),
                        Element::Group(g) => format!("{} elements", g.children.len()),
                    },
                    last_step: e.last_step(),
                    size: match e {
                        Element::Text(t) => t.paragraphs.first().and_then(|p| p.runs.first()).and_then(|r| r.size),
                        Element::Shape(s) => s.paragraphs.first().and_then(|p| p.runs.first()).and_then(|r| r.size),
                        Element::Math(m) => m.size,
                        Element::Outline(o) => o.size,
                        Element::Table(t) => t.size,
                        Element::Image(_) | Element::Media(_) | Element::Figure(_) | Element::Group(_) => None,
                    }
                    .unwrap_or(svg::DEFAULT_FONT_SIZE),
                    crop: if let Element::Image(i) = e { Some(i.crop) } else { None },
                    mask: if let Element::Image(i) = e { Some(i.mask) } else { None },
                    steps: e.steps().to_vec(),
                    fill: element_style_info(e).0,
                    stroke: element_style_info(e).1,
                    highlight: element_style_info(e).2,
                    subsections: element_style_info(e).3,
                    media: media_info(e),
                    shape: shape_info(e).0,
                    flip: shape_info(e).1,
                    placeholder: None,
                    table: table_info(e, &e.state_at(self.step).rect),
                })
                .collect(),
            presenting: self.presenting,
            blank: self.blank.clone(),
            warnings: self.warnings.clone(),
            layouts: deck.layouts.iter().map(|l| l.name.clone()).collect(),
            layout: slide.layout.clone(),
            section: slide.section.clone(),
            subsection: slide.subsection.clone(),
            in_section: deck.position(self.slide).0.map(str::to_string),
            in_subsection: deck.position(self.slide).1.map(str::to_string),
            starts: deck
                .slides
                .iter()
                .map(|s| SlideStart { section: s.section.clone(), subsection: s.subsection.clone() })
                .collect(),
            editing_layout: None,
            editing_theme: None,
            layout_groups: layout_groups(deck),
            layout_meta: None,
            theme_placeholders: vec![],
            theme: ThemeInfo {
                own: deck
                    .style_theme_of(slide)
                    .and_then(|t| deck.theme_set(t))
                    .map(|t| t.style.clone())
                    .unwrap_or_default(),
                effective: deck.theme_of(slide),
                fonts: self.math.fonts(),
                of_theme: deck.style_theme_of(slide).map(str::to_string),
            },
            can_undo,
            can_redo,
        }
    }

    /// The view of a layout under edit: its static elements, then its
    /// placeholders as elements of kind `placeholder` whose text is the name.
    fn layout_view(&self, deck: &Deck, layout: &Layout) -> View {
        let (can_undo, can_redo) = self.can_undo_redo();
        let svg = inline_assets(&svg::render_layout(deck, layout, &*self.math.0), &self.dir, &self.assets);
        let mut elements: Vec<ElementInfo> = layout
            .elements
            .iter()
            .enumerate()
            .map(|(index, e)| ElementInfo {
                index,
                kind: match e {
                    Element::Text(_) => "text",
                    Element::Image(_) => "image",
                    Element::Media(_) => "media",
                    Element::Figure(_) => "figure",
                    Element::Table(_) => "table",
                    Element::Shape(_) => "shape",
                    Element::Math(_) => "math",
                    Element::Outline(_) => "outline",
                    Element::Group(_) => "group",
                },
                rect: *e.rect(),
                rotation: e.state_at(e.last_step()).rotation,
                text: match e {
                    Element::Text(t) => t.text(),
                    Element::Shape(s) => s.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n"),
                    Element::Math(m) => m.source.clone(),
                    Element::Image(i) => i.src.clone(),
                    Element::Media(m) => m.src.clone(),
                    Element::Figure(f) => f.source.clone(),
                    Element::Table(t) => format!("{} x {}", t.rows.len(), t.column_count()),
                    Element::Outline(_) => String::new(),
                    Element::Group(g) => format!("{} elements", g.children.len()),
                },
                last_step: e.last_step(),
                size: svg::DEFAULT_FONT_SIZE,
                crop: if let Element::Image(i) = e { Some(i.crop) } else { None },
                mask: if let Element::Image(i) = e { Some(i.mask) } else { None },
                steps: e.steps().to_vec(),
                fill: element_style_info(e).0,
                stroke: element_style_info(e).1,
                highlight: element_style_info(e).2,
                subsections: element_style_info(e).3,
                media: media_info(e),
                shape: shape_info(e).0,
                flip: shape_info(e).1,
                placeholder: None,
                table: table_info(e, e.rect()),
            })
            .collect();
        let resolved = deck.resolve(layout);
        for (k, (ph, shown)) in layout.placeholders.iter().zip(&resolved.placeholders).enumerate() {
            // The style as drawn (the theme's where the layout sets none),
            // with what the layout overrides in `own`.
            let ph = &Placeholder { own: ph.own.clone(), ..shown.clone() };
            elements.push(ElementInfo {
                index: layout.elements.len() + k,
                kind: "placeholder",
                rect: ph.rect,
                rotation: 0.0,
                text: ph.name.clone(),
                last_step: 0,
                size: ph.size.unwrap_or(svg::DEFAULT_FONT_SIZE),
                crop: None,
                mask: None,
                steps: vec![],
                fill: None,
                stroke: None,
                highlight: None,
                subsections: None,
                media: None,
                shape: None,
                flip: false,
                placeholder: Some(ph.clone()),
                table: None,
            });
        }
        let starts = deck
            .slides
            .iter()
            .map(|s| SlideStart { section: s.section.clone(), subsection: s.subsection.clone() })
            .collect();
        View {
            title: deck.title.clone(),
            dir: self.dir.display().to_string(),
            width: deck.width,
            height: deck.height,
            slide: self.slide,
            slide_count: deck.slides.len(),
            step: 0,
            last_step: 0,
            svg,
            next_svg: None,
            notes: String::new(),
            elements,
            presenting: self.presenting,
            blank: self.blank.clone(),
            warnings: self.warnings.clone(),
            layouts: deck.layouts.iter().map(|l| l.name.clone()).collect(),
            layout: Some(layout.name.clone()),
            section: None,
            subsection: None,
            in_section: None,
            in_subsection: None,
            starts,
            editing_layout: Some(layout.name.clone()),
            editing_theme: None,
            layout_groups: layout_groups(deck),
            layout_meta: Some(LayoutMeta {
                parent: layout.parent.clone(),
                hide_theme_elements: layout.hide_theme_elements,
                overrides: if layout.parent.is_some() { layout.overrides() } else { vec![] },
                themes: deck.themes.iter().map(|t| t.name.clone()).collect(),
            }),
            theme_placeholders: vec![],
            theme: ThemeInfo {
                of_theme: layout.parent.clone(),
                own: layout.theme.clone(),
                effective: deck.theme_of_layout(layout),
                fonts: self.math.fonts(),
            },
            can_undo,
            can_redo,
        }
    }

    /// Edits the theme of the layout under edit, else the deck's.
    pub fn set_theme(&mut self, change: &ThemeChange) -> Result<(), String> {
        let color = |s: &Option<String>| -> Result<Option<Option<Color>>, String> {
            match s {
                None => Ok(None),
                Some(v) if v.is_empty() => Ok(Some(None)),
                Some(v) => Color::from_hex(v).map(|c| Some(Some(c))).map_err(|e| e.to_string()),
            }
        };
        let (c, a) = (color(&change.color)?, color(&change.accent)?);
        let palette = change
            .palette
            .as_ref()
            .map(|p| p.iter().map(|v| Color::from_hex(v).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>())
            .transpose()?;
        let theme = if self.editing_theme.is_some() {
            &mut self.theme_mut()?.style
        } else if self.editing_layout.is_some() {
            &mut self.layout_mut()?.theme
        } else {
            // On a slide: the fonts and colors of the slide's theme (a deck
            // without themes gets one).
            let deck = self.deck.as_mut().ok_or("no deck")?;
            if deck.themes.is_empty() {
                deck.themes.push(ThemeSet { name: "Theme".into(), ..Default::default() });
            }
            let slide = deck.slides.get(self.slide).ok_or("no slide")?;
            let name = deck.style_theme_of(slide).unwrap_or_default().to_string();
            &mut deck.themes.iter_mut().find(|t| t.name == name).ok_or("no theme")?.style
        };
        if let Some(f) = &change.font {
            theme.font = if f.is_empty() { None } else { Some(f.clone()) };
        }
        if let Some(sz) = change.size {
            theme.size = if sz > 0.0 { Some(sz) } else { None };
        }
        if let Some(c) = c {
            theme.color = c;
        }
        if let Some(a) = a {
            theme.accent = a;
        }
        if let Some(p) = palette {
            theme.palette = p;
        }
        self.touch(None);
        Ok(())
    }

    // ----- themes -----

    /// Whether a layout or a theme is under edit rather than a slide.
    pub fn editing_template(&self) -> bool {
        self.editing_layout.is_some() || self.editing_theme.is_some()
    }

    /// The elements under edit: the theme's, the layout's or the slide's.
    fn edited_elements<'a>(&self, deck: &'a Deck, slide: &'a Slide) -> &'a Vec<Element> {
        if let Some(t) = self.editing_theme.as_deref().and_then(|n| deck.theme_set(n)) {
            return &t.elements;
        }
        match self.editing_layout.as_deref().and_then(|n| deck.layout(n)) {
            Some(layout) => &layout.elements,
            None => &slide.elements,
        }
    }

    fn theme_mut(&mut self) -> Result<&mut ThemeSet, String> {
        let name = self.editing_theme.clone().ok_or("no theme under edit")?;
        self.deck
            .as_mut()
            .and_then(|d| d.themes.iter_mut().find(|t| t.name == name))
            .ok_or_else(|| "no such theme".to_string())
    }

    /// The theme editor's view: background and shared elements on the
    /// canvas, the placeholder styles for the panel.
    fn theme_view(&self, deck: &Deck, theme: &ThemeSet) -> View {
        let as_layout = Layout {
            name: theme.name.clone(),
            background: theme.background,
            theme: theme.style.clone(),
            elements: theme.elements.clone(),
            ..Default::default()
        };
        let mut view = self.layout_view(deck, &as_layout);
        view.svg = inline_assets(&svg::render_theme(deck, theme, &*self.math.0), &self.dir, &self.assets);
        view.editing_layout = None;
        view.editing_theme = Some(theme.name.clone());
        view.layout = None;
        view.layout_meta = None;
        view.theme_placeholders = theme.placeholders.clone();
        view.theme.effective = theme.style.over(&deck.theme);
        view
    }

    /// Enters (`Some(name)`) or leaves (`None`) the theme editor.
    pub fn edit_theme(&mut self, name: Option<String>) -> Result<(), String> {
        if let Some(n) = &name {
            if self.deck.as_ref().ok_or("no deck")?.theme_set(n).is_none() {
                return Err(format!("no theme named {n}"));
            }
            self.editing_layout = None;
        }
        self.editing_theme = name;
        Ok(())
    }

    /// A new theme (with the deck's default placeholder styles), entered
    /// for editing. Returns its name.
    pub fn new_theme(&mut self) -> Result<String, String> {
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let name = deck.free_theme_name("Theme");
        let (w, h) = (deck.width, deck.height);
        let sample = Layout::preset("", "compare", w, h);
        let mut placeholders: Vec<Placeholder> = vec![];
        for p in sample.placeholders.iter().chain(Layout::preset("", "title", w, h).placeholders.iter()) {
            let kind = deck_core::themes::kind_of(&p.name).to_string();
            if !placeholders.iter().any(|q| q.name == kind) {
                placeholders.push(Placeholder { name: kind, rect: Rect::default(), ..p.clone() });
            }
        }
        deck.themes.push(ThemeSet { name: name.clone(), placeholders, ..Default::default() });
        self.editing_layout = None;
        self.editing_theme = Some(name.clone());
        Ok(name)
    }

    /// Renames the theme under edit, in its layouts too.
    pub fn rename_theme(&mut self, new_name: String) -> Result<(), String> {
        let new_name = new_name.trim().to_string();
        if new_name.is_empty() {
            return Err("a theme needs a name".into());
        }
        let old = self.editing_theme.clone().ok_or("no theme under edit")?;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        if old != new_name && deck.theme_set(&new_name).is_some() {
            return Err(format!("a theme named {new_name} exists"));
        }
        for t in deck.themes.iter_mut().filter(|t| t.name == old) {
            t.name = new_name.clone();
        }
        for l in deck.layouts.iter_mut().filter(|l| l.parent.as_deref() == Some(old.as_str())) {
            l.parent = Some(new_name.clone());
        }
        self.editing_theme = Some(new_name);
        Ok(())
    }

    /// Deletes the theme under edit; its layouts keep their look (they are
    /// resolved first) and lose the theme.
    pub fn delete_theme(&mut self) -> Result<(), String> {
        let name = self.editing_theme.take().ok_or("no theme under edit")?;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let resolved: Vec<(usize, Layout)> = deck
            .layouts
            .iter()
            .enumerate()
            .filter(|(_, l)| l.parent.as_deref() == Some(name.as_str()))
            .map(|(i, l)| (i, Layout { parent: None, ..deck.resolve(l) }))
            .collect();
        for (i, l) in resolved {
            deck.layouts[i] = l;
        }
        deck.themes.retain(|t| t.name != name);
        self.touch(None);
        Ok(())
    }

    /// Sets the default style of placeholder kind `kind` in the theme under edit.
    pub fn style_theme_placeholder(&mut self, kind: &str, change: &PlaceholderStyle) -> Result<(), String> {
        self.touch(None);
        let theme = self.theme_mut()?;
        if !theme.placeholders.iter().any(|p| p.name == kind) {
            theme.placeholders.push(Placeholder { name: kind.to_string(), ..Default::default() });
        }
        let ph = theme.placeholders.iter_mut().find(|p| p.name == kind).ok_or("no such placeholder")?;
        apply_placeholder_style(ph, change)
    }

    /// The theme the layout under edit belongs to (`None`: none). Its look
    /// is kept: what differs from the new theme becomes overrides.
    pub fn set_layout_theme(&mut self, theme: Option<String>) -> Result<(), String> {
        self.touch(None);
        let deck = self.deck.as_ref().ok_or("no deck")?;
        if let Some(t) = &theme {
            if deck.theme_set(t).is_none() {
                return Err(format!("no theme named {t}"));
            }
        }
        let name = self.editing_layout.clone().ok_or("no layout under edit")?;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let i = deck.layouts.iter().position(|l| l.name == name).ok_or("no such layout")?;
        let mut layout = Layout { parent: None, ..deck.resolve(&deck.layouts[i]) };
        if let Some(t) = theme.as_deref().and_then(|t| deck.theme_set(t)) {
            layout.parent = Some(t.name.clone());
            // What the theme shares is no longer the layout's own.
            layout.elements.retain(|e| !t.elements.contains(e));
            if layout.background == t.background {
                layout.background = None;
            }
            if layout.theme == t.style {
                layout.theme = Theme::default();
            }
            for p in &mut layout.placeholders {
                p.own = match t.placeholder_for(&p.name) {
                    Some(base) => deck_core::PLACEHOLDER_PROPS
                        .iter()
                        .filter(|prop| !same_prop(p, base, prop))
                        .map(|s| s.to_string())
                        .collect(),
                    None => deck_core::PLACEHOLDER_PROPS.iter().map(|s| s.to_string()).collect(),
                };
            }
        }
        deck.layouts[i] = layout;
        Ok(())
    }

    /// Whether the layout under edit draws its theme's shared elements.
    pub fn hide_theme_elements(&mut self, hide: bool) -> Result<(), String> {
        self.touch(None);
        self.layout_mut()?.hide_theme_elements = hide;
        Ok(())
    }

    /// Drops overrides of the layout under edit: of one placeholder
    /// property (`element` and `prop`), or all of them.
    pub fn reset_to_theme(&mut self, element: Option<usize>, prop: Option<String>) -> Result<(), String> {
        self.touch(None);
        let name = self.editing_layout.clone().ok_or("no layout under edit")?;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let i = deck.layouts.iter().position(|l| l.name == name).ok_or("no such layout")?;
        let theme = deck.layouts[i].parent.as_deref().and_then(|t| deck.theme_set(t)).cloned();
        let layout = &mut deck.layouts[i];
        match (element, prop) {
            (Some(e), Some(prop)) => {
                let k = e.checked_sub(layout.elements.len()).ok_or("not a placeholder")?;
                layout.placeholders.get_mut(k).ok_or("no such placeholder")?.reset(&prop, theme.as_ref());
            }
            _ => layout.reset_to_theme(theme.as_ref()),
        }
        Ok(())
    }

    /// Makes an override of the layout under edit the theme's default: the
    /// placeholder's property moves to the theme, for all its layouts.
    pub fn push_to_theme(&mut self, element: usize, prop: &str) -> Result<(), String> {
        self.touch(None);
        let name = self.editing_layout.clone().ok_or("no layout under edit")?;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let i = deck.layouts.iter().position(|l| l.name == name).ok_or("no such layout")?;
        let parent = deck.layouts[i].parent.clone().ok_or("the layout has no theme")?;
        let k = element.checked_sub(deck.layouts[i].elements.len()).ok_or("not a placeholder")?;
        let ph = deck.layouts[i].placeholders.get(k).ok_or("no such placeholder")?.clone();
        let kind = deck_core::themes::kind_of(&ph.name).to_string();
        let theme = deck.themes.iter_mut().find(|t| t.name == parent).ok_or("no such theme")?;
        if !theme.placeholders.iter().any(|p| deck_core::themes::kind_of(&p.name) == kind) {
            theme.placeholders.push(Placeholder { name: kind.clone(), ..Default::default() });
        }
        let base = theme.placeholders.iter_mut().find(|p| deck_core::themes::kind_of(&p.name) == kind).unwrap();
        copy_prop(base, &ph, prop);
        deck.layouts[i].placeholders[k].own.retain(|p| p != prop);
        Ok(())
    }

    // ----- layouts -----

    /// Enters (`Some(name)`) or leaves (`None`) the layout editor.
    pub fn edit_layout(&mut self, name: Option<String>) -> Result<(), String> {
        if let Some(n) = &name {
            let deck = self.deck.as_ref().ok_or("no deck")?;
            if deck.layout(n).is_none() {
                return Err(format!("no layout named {n}"));
            }
            self.editing_theme = None;
        }
        self.editing_layout = name;
        Ok(())
    }

    /// A new empty layout, entered for editing. Returns its name.
    /// A new layout from a preset disposition (see [`Layout::PRESETS`]),
    /// named after it, entered for editing. Returns its name.
    pub fn new_layout(&mut self, kind: &str) -> Result<String, String> {
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let base = Layout::PRESETS
            .iter()
            .find(|(id, _)| *id == kind)
            .map(|(_, label)| label.to_string())
            .unwrap_or_else(|| "Layout".into());
        let mut name = base.clone();
        let mut n = 1;
        while deck.layout(&name).is_some() {
            n += 1;
            name = format!("{base} {n}");
        }
        let mut layout = Layout::preset(&name, kind, deck.width, deck.height);
        // In the theme of the current slide's layout (else the first one):
        // its placeholders take the theme's styles.
        let current = deck.slides.get(self.slide).and_then(|s| s.layout.as_deref()).and_then(|n| deck.layout(n));
        let theme = current.and_then(|l| l.parent.clone()).or_else(|| deck.themes.first().map(|t| t.name.clone()));
        if let Some(t) = theme.as_deref().and_then(|t| deck.theme_set(t)) {
            for p in &mut layout.placeholders {
                if t.placeholder_for(&p.name).is_none() {
                    p.own = deck_core::PLACEHOLDER_PROPS.iter().map(|s| s.to_string()).collect();
                }
            }
            layout.parent = Some(t.name.clone());
        }
        deck.layouts.push(layout);
        self.editing_theme = None;
        self.editing_layout = Some(name.clone());
        Ok(name)
    }

    /// Renames the layout under edit, in the slides that use it too.
    pub fn rename_layout(&mut self, new_name: String) -> Result<(), String> {
        let new_name = new_name.trim().to_string();
        if new_name.is_empty() {
            return Err("a layout needs a name".into());
        }
        let old = self.editing_layout.clone().ok_or("no layout under edit")?;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        if old != new_name && deck.layout(&new_name).is_some() {
            return Err(format!("a layout named {new_name} exists"));
        }
        for l in deck.layouts.iter_mut().filter(|l| l.name == old) {
            l.name = new_name.clone();
        }
        for s in deck.slides.iter_mut().filter(|s| s.layout.as_deref() == Some(old.as_str())) {
            s.layout = Some(new_name.clone());
        }
        self.editing_layout = Some(new_name);
        self.touch(None);
        Ok(())
    }

    /// Deletes the layout under edit; its slides keep their elements and
    /// lose the layout. Leaves the layout editor.
    pub fn delete_layout(&mut self) -> Result<(), String> {
        let name = self.editing_layout.take().ok_or("no layout under edit")?;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        deck.layouts.retain(|l| l.name != name);
        for s in deck.slides.iter_mut().filter(|s| s.layout.as_deref() == Some(name.as_str())) {
            s.layout = None;
        }
        self.touch(None);
        Ok(())
    }

    /// Adds a placeholder to the layout under edit.
    /// Adds a placeholder of `kind`: `title`, `subtitle` or `body` (a
    /// later one of a kind is numbered: `subtitle2`, `body2`…).
    pub fn add_placeholder(&mut self, kind: &str) -> Result<(), String> {
        let (w, h) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let layout = self.layout_mut()?;
        let base = match kind {
            "title" | "subtitle" => kind,
            _ => "body",
        };
        let mut n = 1;
        let mut name = if base == "body" { "body1".to_string() } else { base.to_string() };
        while layout.placeholder(&name).is_some() {
            n += 1;
            name = format!("{base}{n}");
        }
        let placeholder = match base {
            "title" => Placeholder {
                name,
                rect: Rect::new(w * 0.06, h * 0.05, w * 0.88, h * 0.15),
                size: Some(40.0),
                bold: true,
                ..Default::default()
            },
            "subtitle" => Placeholder {
                name,
                rect: Rect::new(w * 0.06, h * 0.22, w * 0.88, h * 0.1),
                size: Some(26.0),
                ..Default::default()
            },
            _ => Placeholder {
                name,
                rect: Rect::new(w * 0.1, h * 0.3, w * 0.8, h * 0.4),
                size: Some(24.0),
                bullets: true,
                ..Default::default()
            },
        };
        layout.placeholders.push(placeholder);
        self.touch(None);
        Ok(())
    }

    fn layout_mut(&mut self) -> Result<&mut Layout, String> {
        let name = self.editing_layout.clone().ok_or("no layout under edit")?;
        self.deck
            .as_mut()
            .and_then(|d| d.layouts.iter_mut().find(|l| l.name == name))
            .ok_or_else(|| "no such layout".to_string())
    }

    /// The elements being edited: the layout's when a layout is under
    /// edit, else the current slide's.
    fn elements_mut(&mut self) -> Result<&mut Vec<Element>, String> {
        if self.editing_theme.is_some() {
            Ok(&mut self.theme_mut()?.elements)
        } else if self.editing_layout.is_some() {
            Ok(&mut self.layout_mut()?.elements)
        } else {
            Ok(&mut self.slide_mut()?.elements)
        }
    }

    /// Marks what the current edit changes: the slide, or every slide of
    /// the layout under edit.
    fn touch_target(&mut self) {
        if self.editing_template() {
            self.touch(None);
        } else {
            self.touch(Some(self.slide));
        }
    }

    /// Moves slide `from` so that it sits at index `to` afterwards; the
    /// current slide follows it.
    pub fn move_slide(&mut self, from: usize, to: usize) -> Result<(), String> {
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let n = deck.slides.len();
        if from >= n {
            return Err("no such slide".into());
        }
        let to = to.min(n - 1);
        if from == to {
            return Ok(());
        }
        let slide = deck.slides.remove(from);
        deck.slides.insert(to, slide);
        let thumb = self.thumbs.remove(from);
        self.thumbs.insert(to, thumb);
        self.slide = if self.slide == from {
            to
        } else if from < self.slide && self.slide <= to {
            self.slide - 1
        } else if to <= self.slide && self.slide < from {
            self.slide + 1
        } else {
            self.slide
        };
        self.step = 0;
        Ok(())
    }

    /// Sets what a given slide starts (`None` for nothing).
    pub fn set_section_of(
        &mut self,
        slide: usize,
        section: Option<String>,
        subsection: Option<String>,
    ) -> Result<(), String> {
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let s = deck.slides.get_mut(slide).ok_or("no such slide")?;
        s.section = section.filter(|t| !t.trim().is_empty());
        s.subsection = subsection.filter(|t| !t.trim().is_empty());
        Ok(())
    }

    pub fn set_section(&mut self, section: Option<String>, subsection: Option<String>) -> Result<(), String> {
        let slide = self.slide_mut()?;
        slide.section = section.filter(|s| !s.trim().is_empty());
        slide.subsection = subsection.filter(|s| !s.trim().is_empty());
        Ok(())
    }

    /// Writes the deck as a PDF with bookmarks (Typst feature), else an error.
    /// `mode`: `slides` (one page per slide), `steps` (one per build step;
    /// both write the notes next to the PDF as `<name>.notes.md` for
    /// decker-viewer) or `notes` (the handout: slide and notes per page).
    pub fn export_pdf(&mut self, out: &Path, mode: &str) -> Result<(), String> {
        let Some(deck) = self.deck.take() else { return Err("no deck open".into()) };
        for slide in &deck.slides {
            self.cache_assets(slide);
        }
        let result = self.pdf(&deck, out, mode);
        self.deck = Some(deck);
        result
    }

    #[cfg(feature = "typst")]
    fn pdf(&self, deck: &Deck, out: &Path, mode: &str) -> Result<(), String> {
        let engine = deck_typst::Engine::for_deck(&self.dir);
        if mode == "notes" {
            return engine
                .export_notes_pdf(deck, |i| self.render_still(deck, &deck.slides[i], deck.slides[i].last_step()), out)
                .map_err(|e| e.to_string());
        }
        let pages = transfer::pages(deck, mode == "steps");
        engine
            .export_pdf_pages(deck, &pages, |i, step| self.render_still(deck, &deck.slides[i], step), out)
            .map_err(|e| e.to_string())
    }

    #[cfg(not(feature = "typst"))]
    fn pdf(&self, _deck: &Deck, _out: &Path, _mode: &str) -> Result<(), String> {
        Err("PDF export needs the typst feature".into())
    }

    /// The steps of every slide, for the show's navigation.
    fn pages(&self) -> Vec<u32> {
        self.deck.as_ref().map(|d| d.slides.iter().map(Slide::last_step).collect()).unwrap_or_default()
    }

    /// Moves the show (see [`deck_present::Show::navigate`]); leaves the layout editor.
    pub fn navigate(&mut self, action: &str) {
        self.editing_layout = None;
        let pages = self.pages();
        let mut show =
            deck_present::Show { page: self.slide, step: self.step, blank: self.blank.clone(), ..Default::default() };
        show.navigate(action, &pages);
        (self.slide, self.step, self.blank) = (show.page, show.step, show.blank);
    }

    pub fn goto(&mut self, slide: usize, step: u32) {
        self.editing_layout = None;
        let pages = self.pages();
        let mut show = deck_present::Show { page: self.slide, step: self.step, ..Default::default() };
        show.goto(slide, step, &pages);
        (self.slide, self.step) = (show.page, show.step);
    }

    fn slide_mut(&mut self) -> Result<&mut Slide, String> {
        let index = self.slide;
        self.deck.as_mut().and_then(|d| d.slides.get_mut(index)).ok_or_else(|| "no slide".to_string())
    }

    pub fn set_rect(&mut self, element: usize, rect: Rect) -> Result<(), String> {
        self.touch_target();
        let rect = Rect { w: rect.w.max(1.0), h: rect.h.max(1.0), ..rect };
        // Viewing a build step the element changes at: the drag edits that
        // step's target box rather than the base position.
        let step = self.step;
        if step > 0 && !self.editing_template() {
            if let Some(e) = self.elements_mut()?.get_mut(element) {
                let steps = e.steps_mut();
                let target = match steps.iter_mut().position(|st| st.at == step) {
                    Some(i) => &mut steps[i],
                    None => {
                        // Dragging at a click the element has no step at: a move step there.
                        steps.push(Step { at: step, duration: 0.5, ..Default::default() });
                        steps.sort_by_key(|st| st.at);
                        steps.iter_mut().find(|st| st.at == step).expect("just inserted")
                    }
                };
                target.set.x = Some(rect.x);
                target.set.y = Some(rect.y);
                target.set.w = Some(rect.w);
                target.set.h = Some(rect.h);
                return Ok(());
            }
        }
        if self.editing_layout.is_some() {
            let layout = self.layout_mut()?;
            if element >= layout.elements.len() {
                let ph = layout.placeholders.get_mut(element - layout.elements.len()).ok_or("no such element")?;
                ph.rect = rect;
                return Ok(());
            }
        }
        let e = self.elements_mut()?.get_mut(element).ok_or("no such element")?;
        e.set_rect(rect);
        Ok(())
    }

    pub fn set_text(&mut self, element: usize, text: &str) -> Result<(), String> {
        self.touch_target();
        if self.editing_layout.is_some() {
            let layout_name = self.editing_layout.clone().unwrap_or_default();
            let layout = self.layout_mut()?;
            if element >= layout.elements.len() {
                // Renaming a placeholder: the slides that fill it follow.
                let k = element - layout.elements.len();
                let new_name = text.trim().to_string();
                if new_name.is_empty() {
                    return Err("a placeholder needs a name".into());
                }
                let old = layout.placeholders.get(k).map(|p| p.name.clone()).ok_or("no such element")?;
                if old != new_name && layout.placeholder(&new_name).is_some() {
                    return Err(format!("a placeholder named {new_name} exists"));
                }
                layout.placeholders[k].name = new_name.clone();
                let deck = self.deck.as_mut().ok_or("no deck")?;
                for s in deck.slides.iter_mut().filter(|s| s.layout.as_deref() == Some(layout_name.as_str())) {
                    for e in &mut s.elements {
                        if let Element::Text(t) = e {
                            if t.placeholder.as_deref() == Some(old.as_str()) {
                                t.placeholder = Some(new_name.clone());
                            }
                        }
                    }
                }
                return Ok(());
            }
        }
        let e = self.elements_mut()?.get_mut(element).ok_or("no such element")?;
        let paragraphs = |old: &[Paragraph]| -> Vec<Paragraph> {
            text.lines()
                .enumerate()
                .map(|(i, line)| {
                    // Keep the style of the paragraph being replaced, when there is one.
                    let mut p = old.get(i).cloned().unwrap_or_default();
                    let mut run = p.runs.first().cloned().unwrap_or_default();
                    run.text = line.to_string();
                    p.runs = vec![run];
                    p
                })
                .collect()
        };
        match e {
            Element::Text(t) => t.paragraphs = paragraphs(&t.paragraphs),
            Element::Shape(s) => s.paragraphs = paragraphs(&s.paragraphs),
            Element::Math(m) => m.source = text.to_string(),
            Element::Figure(f) => f.source = text.to_string(),
            Element::Table(_) => return Err("edit a table's cells in the panel".into()),
            Element::Image(_) | Element::Media(_) => return Err("an image has no text".into()),
            Element::Outline(_) => return Err("the outline is computed from the sections".into()),
            Element::Group(_) => return Err("ungroup to edit the texts of a group".into()),
        }
        Ok(())
    }

    /// The paragraphs of a text box or shape, for the caret editor.
    fn paragraphs_mut(&mut self, element: usize) -> Result<&mut Vec<Paragraph>, String> {
        let cell = self.cell;
        match self.elements_mut()?.get_mut(element) {
            Some(Element::Text(t)) => Ok(&mut t.paragraphs),
            Some(Element::Shape(s)) => Ok(&mut s.paragraphs),
            Some(Element::Table(t)) => match cell {
                Some((e, row, column)) if e == element => {
                    let c = t.rows.get_mut(row).and_then(|r| r.get_mut(column)).ok_or("no such cell")?;
                    // An empty cell has no paragraph to type into yet.
                    if c.paragraphs.is_empty() {
                        c.paragraphs.push(Paragraph::default());
                    }
                    Ok(&mut c.paragraphs)
                }
                _ => Err("pick a cell of the table first".into()),
            },
            Some(_) => Err("not a text element".into()),
            None => Err("no such element".into()),
        }
    }

    /// Character boxes of a text element, in slide coordinates, plus the
    /// caret box after the last character. `None` without the typesetter.
    pub fn text_layout(&self, element: usize) -> Result<Option<TextLayout>, String> {
        let (deck, slide) = self.current().ok_or("no slide")?;
        let elements = self.edited_elements(deck, slide);
        let shown = |e: &Element| if self.editing_template() { *e.rect() } else { e.state_at(self.step).rect };
        let mut size = None;
        let cell_paragraphs;
        let (rect, paragraphs): (Rect, &[Paragraph]) = match elements.get(element) {
            Some(e @ Element::Text(t)) => (shown(e), &t.paragraphs),
            Some(e @ Element::Shape(s)) => (shown(e), &s.paragraphs),
            Some(e @ Element::Table(t)) => match self.cell {
                Some((el, row, column)) if el == element && row < t.rows.len() && column < t.column_count() => {
                    size = t.size;
                    cell_paragraphs = t.display_paragraphs(row, column);
                    (t.span_rect(&shown(e), row, column), &cell_paragraphs)
                }
                _ => return Err("pick a cell of the table first".into()),
            },
            Some(_) => return Err("not a text element".into()),
            None => return Err("no such element".into()),
        };
        let mut defaults = match self.editing_layout.as_deref().and_then(|n| deck.layout(n)) {
            Some(layout) => deck.theme_of_layout(layout).defaults(),
            None if self.editing_theme.is_some() => self
                .editing_theme
                .as_deref()
                .and_then(|n| deck.theme_set(n))
                .map(|t| t.style.over(&deck.theme).defaults())
                .unwrap_or_default(),
            None => deck.theme_of(slide).defaults(),
        };
        defaults.size = size.or(defaults.size);
        let width = (rect.w - 2.0 * svg::PADDING).max(1.0);
        let mut raw_paragraphs = paragraphs.to_vec();
        let paragraphs = match self.raw {
            Some((e, caret))
                if e == element && self.cell.is_none() && text::raw_formula(&mut raw_paragraphs, caret) =>
            {
                &raw_paragraphs
            }
            _ => paragraphs,
        };
        let Some(boxes) = self.math.layout(&deck.math_preamble, paragraphs, width, &defaults) else {
            return Ok(None);
        };
        let (ox, oy) = (rect.x + svg::PADDING, rect.y + svg::PADDING);
        Ok(Some(TextLayout {
            rect,
            text: text::flat_text(paragraphs).replace(text::RAW_DOLLAR, "$"),
            chars: boxes
                .into_iter()
                .map(|b| CharRect { x: ox + b.x, right: ox + b.right, top: oy + b.top, bottom: oy + b.bottom })
                .collect(),
        }))
    }

    /// Replaces `from..to` of a text element by `insert`; returns the caret
    /// index after the edit.
    pub fn text_edit(&mut self, element: usize, from: usize, to: usize, insert: &str) -> Result<usize, String> {
        self.touch_target();
        let paragraphs = self.paragraphs_mut(element)?;
        let (from, to) = (from.min(to), from.max(to));
        let at = text::delete(paragraphs, from, to);
        Ok(text::insert(paragraphs, at, insert))
    }

    /// Sets an image's crop and mask.
    pub fn set_image(&mut self, element: usize, crop: Crop, mask: Mask) -> Result<(), String> {
        self.touch(Some(self.slide));
        let slide = self.slide_mut()?;
        match slide.elements.get_mut(element) {
            Some(Element::Image(img)) => {
                img.crop = crop.clamped();
                img.mask = mask;
                Ok(())
            }
            Some(_) => Err("not an image".into()),
            None => Err("no such element".into()),
        }
    }

    pub fn set_notes(&mut self, notes: String) -> Result<(), String> {
        self.slide_mut()?.notes = notes;
        Ok(())
    }

    /// A new slide after the current one, from the current slide's layout
    /// (its placeholders) when it has one.
    pub fn add_slide(&mut self) -> Result<(), String> {
        self.touch(None);
        let at = self.slide + 1;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let at = at.min(deck.slides.len());
        let slide =
            deck.slides.get(self.slide).and_then(|s| deck.layout_of(s)).map(|l| l.new_slide()).unwrap_or_default();
        deck.slides.insert(at, slide);
        self.slide = at;
        self.step = 0;
        Ok(())
    }

    pub fn duplicate_slide(&mut self) -> Result<(), String> {
        self.touch(None);
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let mut copy = deck.slides.get(self.slide).cloned().ok_or("no slide")?;
        // The copy belongs to the original's section; it starts none of its own.
        copy.section = None;
        copy.subsection = None;
        deck.slides.insert(self.slide + 1, copy);
        self.slide += 1;
        self.step = 0;
        Ok(())
    }

    /// Sets the current slide's layout (`None` for none). Existing
    /// elements are kept; placeholders are not re-created.
    pub fn set_layout(&mut self, name: Option<String>) -> Result<(), String> {
        self.touch(Some(self.slide));
        let deck = self.deck.as_ref().ok_or("no deck")?;
        if let Some(n) = &name {
            if deck.layout(n).is_none() {
                return Err(format!("no layout named {n}"));
            }
        }
        self.slide_mut()?.layout = name;
        Ok(())
    }

    /// Sets the layout of several slides (`None`: no layout).
    pub fn set_layout_of(&mut self, slides: &[usize], name: Option<String>) -> Result<(), String> {
        let deck = self.deck.as_ref().ok_or("no deck")?;
        if let Some(n) = &name {
            if deck.layout(n).is_none() {
                return Err(format!("no layout named {n}"));
            }
        }
        if let Some(&bad) = slides.iter().find(|&&i| i >= deck.slides.len()) {
            return Err(format!("no slide {}", bad + 1));
        }
        for &i in slides {
            self.touch(Some(i));
            if let Some(slide) = self.deck.as_mut().and_then(|d| d.slides.get_mut(i)) {
                slide.layout.clone_from(&name);
            }
        }
        Ok(())
    }

    /// Thumbnails of another deck, for the slide picker.
    pub fn peek(&self, dir: &Path) -> Result<Vec<String>, String> {
        let other = native::load(dir).map_err(|e| e.to_string())?;
        Ok(other
            .slides
            .iter()
            .map(|s| {
                inline_assets(&svg::render_step_with(&other, s, s.last_step(), &*self.math.0), dir, &HashMap::new())
            })
            .collect())
    }

    /// Copies slides of another deck after the current slide.
    pub fn import_slides(&mut self, dir: &Path, indices: &[usize]) -> Result<usize, String> {
        self.touch(None);
        let other = native::load(dir).map_err(|e| e.to_string())?;
        if self.dir.as_os_str().is_empty() {
            return Err("save this deck first, so that images have a folder to be copied into".into());
        }
        let at = self.slide + 1;
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let n = transfer::copy_slides(&other, dir, indices, deck, &self.dir, at).map_err(|e| e.to_string())?;
        if n > 0 {
            self.slide = at;
            self.step = 0;
        }
        Ok(n)
    }

    pub fn delete_slide(&mut self) -> Result<(), String> {
        self.touch(None);
        let deck = self.deck.as_mut().ok_or("no deck")?;
        if deck.slides.len() <= 1 {
            return Err("a deck keeps at least one slide".into());
        }
        deck.slides.remove(self.slide);
        self.slide = self.slide.min(deck.slides.len() - 1);
        self.step = 0;
        Ok(())
    }

    /// Adds `bytes` as an image of the current slide or layout, named
    /// after `name` (its extension gives the format), centered at `at`
    /// in slide points, else at the slide's center. The box is the
    /// image's pixel size, scaled down to fit half the slide. Returns the
    /// new element's index. A deck that has no folder yet gets a
    /// temporary one, which "Save as" copies.
    pub fn add_image(&mut self, name: &str, bytes: &[u8], at: Option<(f64, f64)>) -> Result<usize, String> {
        if self.deck.is_none() {
            return Err("no deck".into());
        }
        if self.dir.as_os_str().is_empty() {
            self.temp_dir()?;
        }
        let ext = Path::new(name).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("image").to_string();
        let name = match ext.as_deref() {
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "mp4" | "webm" | "mov" | "m4v" | "ogv") => {
                name.to_string()
            }
            _ => format!("{stem}.png"),
        };
        let is_video = matches!(ext.as_deref(), Some("mp4" | "webm" | "mov" | "m4v" | "ogv"));
        let animated = ext.as_deref() == Some("gif") && gif_info(bytes).map(|g| g.frames > 1).unwrap_or(false);
        let src = transfer::add_image(&self.dir, &name, bytes).map_err(|e| e.to_string())?;
        self.touch_target();
        let (sw, sh) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        // A video: 16:9 at 60% of the slide, no poster (the browser shows
        // its first frame). An animated GIF: its first frame as poster.
        let mut media = None;
        if is_video {
            media = Some(Media { src: src.clone(), ..Default::default() });
        } else if animated {
            let info = gif_info(bytes).ok_or("unreadable GIF")?;
            let poster = transfer::add_image(&self.dir, &format!("{stem}.poster.png"), &info.first_frame_png)
                .map_err(|e| e.to_string())?;
            media = Some(Media {
                src: src.clone(),
                poster: Some(poster),
                duration: Some(info.duration),
                ..Default::default()
            });
        }
        let (pw, ph) = if is_video {
            (sw * 0.6, sw * 0.6 * 9.0 / 16.0)
        } else {
            transfer::image_size(bytes).map(|(w, h)| (f64::from(w), f64::from(h))).unwrap_or((sw * 0.4, sh * 0.4))
        };
        // Pixel size on the slide, scaled down to fit half of it, or up
        // so that a tiny image (an icon) is still visible.
        let scale = if pw < sw * 0.1 && ph < sh * 0.1 {
            (sw * 0.2 / pw).min(sh * 0.2 / ph)
        } else {
            (sw * 0.5 / pw).min(sh * 0.5 / ph).min(1.0)
        };
        let (w, h) = (pw * scale, ph * scale);
        let (cx, cy) = at.unwrap_or((sw / 2.0, sh / 2.0));
        let rect =
            Rect::new((cx - w / 2.0).clamp(0.0, (sw - w).max(0.0)), (cy - h / 2.0).clamp(0.0, (sh - h).max(0.0)), w, h);
        let elements = self.elements_mut()?;
        elements.push(match media {
            Some(mut m) => {
                m.rect = rect;
                Element::Media(m)
            }
            None => Element::Image(deck_core::Image { rect, src, ..Default::default() }),
        });
        Ok(elements.len() - 1)
    }

    /// The clipboard text for `indices` of the current slide or layout.
    pub fn copy_elements(&self, indices: &[usize]) -> Result<String, String> {
        let (deck, slide) = self.current().ok_or("no slide")?;
        let elements = self.edited_elements(deck, slide);
        let mut sorted: Vec<usize> = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        let picked: Vec<Element> = sorted.iter().filter_map(|&i| elements.get(i).cloned()).collect();
        if picked.is_empty() {
            return Err("nothing to copy".into());
        }
        Ok(transfer::Clip::new(&self.dir, picked).to_text())
    }

    /// Pastes copied elements onto the current slide or layout, images
    /// copied into this deck when they come from another. Returns the
    /// indices of the new elements.
    pub fn paste_elements(&mut self, clip: transfer::Clip) -> Result<Vec<usize>, String> {
        if self.deck.is_none() {
            return Err("no deck".into());
        }
        if self.dir.as_os_str().is_empty() && clip.elements.iter().any(has_image) {
            self.temp_dir()?;
        }
        let pasted = clip.take(&self.dir, 20.0).map_err(|e| e.to_string())?;
        self.touch_target();
        let elements = self.elements_mut()?;
        let first = elements.len();
        elements.extend(pasted);
        Ok((first..elements.len()).collect())
    }

    /// Gives a deck that has no folder yet a temporary one for its assets.
    fn temp_dir(&mut self) -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("decker-untitled-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        self.dir = dir;
        Ok(())
    }

    /// Adds a shape: `rectangle`, `ellipse`, `line` or `arrow`, in the
    /// theme's accent color.
    pub fn add_shape(&mut self, kind: &str) -> Result<usize, String> {
        self.touch_target();
        let (deck, slide) = self.current().ok_or("no slide")?;
        let (w, h) = (deck.width, deck.height);
        let accent = deck.theme_of(slide).accent.unwrap_or(Color::rgb(0x3F, 0x74, 0xE0));
        let shape = match kind {
            "ellipse" => deck_core::Shape {
                rect: Rect::new(w * 0.4, h * 0.35, w * 0.2, h * 0.3),
                kind: deck_core::ShapeKind::Ellipse,
                fill: Some(accent),
                ..Default::default()
            },
            "line" | "arrow" => deck_core::Shape {
                rect: Rect::new(w * 0.35, h * 0.5, w * 0.3, 0.0),
                kind: if kind == "arrow" { deck_core::ShapeKind::Arrow } else { deck_core::ShapeKind::Line },
                stroke: Some(Stroke { color: accent, width: 3.0 }),
                ..Default::default()
            },
            _ => deck_core::Shape {
                rect: Rect::new(w * 0.35, h * 0.35, w * 0.3, h * 0.3),
                fill: Some(accent),
                ..Default::default()
            },
        };
        let elements = self.elements_mut()?;
        elements.push(Element::Shape(shape));
        Ok(elements.len() - 1)
    }

    /// Styles a placeholder of the layout under edit (`element` counts
    /// after the layout's static elements, as in the view).
    pub fn style_placeholder(&mut self, element: usize, change: &PlaceholderStyle) -> Result<(), String> {
        self.touch(None);
        let layout = self.layout_mut()?;
        let k = element.checked_sub(layout.elements.len()).ok_or("not a placeholder")?;
        let ph = layout.placeholders.get_mut(k).ok_or("no such placeholder")?;
        apply_placeholder_style(ph, change)?;
        // In a layout with a theme, what is set here overrides the theme.
        for (prop, set) in [
            ("size", change.size.is_some()),
            ("bold", change.bold.is_some()),
            ("color", change.color.is_some()),
            ("font", change.font.is_some()),
            ("align", change.align.is_some()),
            ("bullets", change.bullets.is_some()),
        ] {
            if set {
                ph.set_own(prop);
            }
        }
        Ok(())
    }

    pub fn add_text(&mut self) -> Result<(), String> {
        self.touch_target();
        let (w, h) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let rect = Rect::new(w * 0.1, h * 0.4, w * 0.8, h * 0.15);
        self.elements_mut()?.push(Element::Text(TextBox::plain(rect, "Text")));
        Ok(())
    }

    /// Styles the runs of a text element over `from..to` (`usize::MAX`
    /// for the whole element).
    pub fn style_text(
        &mut self,
        element: usize,
        from: usize,
        to: usize,
        change: &text::StyleChange,
    ) -> Result<(), String> {
        self.touch_target();
        text::apply_style(self.paragraphs_mut(element)?, from, to, change);
        Ok(())
    }

    /// Styles the paragraphs of a text element touched by `from..to`.
    pub fn style_paragraphs(
        &mut self,
        element: usize,
        from: usize,
        to: usize,
        change: &text::ParagraphChange,
    ) -> Result<(), String> {
        self.touch_target();
        text::apply_paragraph(self.paragraphs_mut(element)?, from, to, change);
        Ok(())
    }

    pub fn style_info(&mut self, element: usize, from: usize, to: usize) -> Result<text::StyleInfo, String> {
        Ok(text::style_info(self.paragraphs_mut(element)?, from, to))
    }

    /// Element-level colors and sizes: fill and stroke of shapes and text
    /// boxes, size and colors of formulas and of the plan. Empty strings
    /// clear a color; a size of 0 resets it.
    pub fn style_element(&mut self, element: usize, change: &ElementStyle) -> Result<(), String> {
        self.touch_target();
        let color = |s: &Option<String>| -> Option<Option<Color>> {
            s.as_ref().map(|v| if v.is_empty() { None } else { Color::from_hex(v).ok() })
        };
        let (fill, stroke, highlight) = (color(&change.fill), color(&change.stroke), color(&change.highlight));
        let stroke_width = change.stroke_width;
        let e = self.elements_mut()?.get_mut(element).ok_or("no such element")?;
        let apply_stroke = |current: &mut Option<Stroke>| {
            if let Some(c) = stroke {
                *current = c.map(|color| Stroke { color, width: current.map(|s| s.width).unwrap_or(1.0) });
            }
            if let (Some(w), Some(s)) = (stroke_width, current.as_mut()) {
                s.width = w.max(0.25);
            }
        };
        match e {
            Element::Text(t) => {
                if let Some(f) = fill {
                    t.fill = f;
                }
                apply_stroke(&mut t.stroke);
            }
            Element::Shape(sh) => {
                if let Some(f) = fill {
                    sh.fill = f;
                }
                apply_stroke(&mut sh.stroke);
                if let Some(f) = change.flip {
                    sh.flip = f;
                }
            }
            Element::Math(m) => {
                if let Some(size) = change.size {
                    m.size = if size > 0.0 { Some(size) } else { None };
                }
                if let Some(c) = fill {
                    m.color = c;
                }
            }
            Element::Outline(o) => {
                if let Some(size) = change.size {
                    o.size = if size > 0.0 { Some(size) } else { None };
                }
                if let Some(c) = fill {
                    o.color = c;
                }
                if let Some(c) = highlight {
                    o.highlight = c;
                }
                if let Some(sub) = change.subsections {
                    o.subsections = sub;
                }
            }
            Element::Media(m) => {
                if let Some(p) = &change.play {
                    m.play = if p == "button" { Play::Button } else { Play::OnAppear };
                }
                if let Some(l) = change.looping {
                    m.looping = l;
                }
                if let Some(snd) = change.sound {
                    m.sound = snd;
                }
            }
            Element::Table(t) => {
                apply_stroke(&mut t.stroke);
                if let Some(size) = change.size {
                    t.size = if size > 0.0 { Some(size) } else { None };
                }
            }
            Element::Image(_) | Element::Figure(_) | Element::Group(_) => {
                return Err("select one element to style it".into())
            }
        }
        Ok(())
    }

    /// Replaces an element's build steps (sorted by click).
    pub fn set_steps(&mut self, element: usize, mut steps: Vec<Step>) -> Result<(), String> {
        self.touch_target();
        steps.sort_by_key(|s| s.at);
        let e = self.elements_mut()?.get_mut(element).ok_or("no such element")?;
        *e.steps_mut() = steps;
        Ok(())
    }

    /// Turns an element to `degrees` clockwise at the viewed click: a
    /// rotation with no step of its own is the element's rotation from
    /// the start (kept as an instant step at click 0).
    pub fn set_rotation(&mut self, element: usize, degrees: f64) -> Result<(), String> {
        self.touch_target();
        let step = if self.editing_template() { 0 } else { self.step };
        let degrees = ((degrees % 360.0) + 360.0) % 360.0;
        let e = self.elements_mut()?.get_mut(element).ok_or("no such element")?;
        let steps = e.steps_mut();
        match steps.iter_mut().find(|st| st.at == step) {
            Some(st) => st.set.rotation = Some(degrees),
            None => {
                let mut st = Step { at: step, duration: if step == 0 { 0.0 } else { 0.5 }, ..Default::default() };
                st.set.rotation = Some(degrees);
                steps.push(st);
                steps.sort_by_key(|st| st.at);
            }
        }
        // A step at click 0 that sets nothing any more is dropped.
        steps
            .retain(|st| !(st.at == 0 && st.set == deck_core::Overrides { rotation: Some(0.0), ..Default::default() }));
        Ok(())
    }

    /// Moves several elements in one edit (a multi-selection dragged).
    pub fn set_rects(&mut self, rects: &[(usize, Rect)]) -> Result<(), String> {
        for (index, rect) in rects {
            self.set_rect(*index, *rect)?;
        }
        Ok(())
    }

    /// Arranges the elements at `indices`: `align_left`, `align_center`,
    /// `align_right`, `align_top`, `align_middle`, `align_bottom` (to the
    /// selection's bounds, or to the slide for one element);
    /// `distribute_h`, `distribute_v` (equal gaps, three or more);
    /// `mirror_h`, `mirror_v` (each element in place, and their positions
    /// within the selection's bounds); `front`, `back`, `forward`,
    /// `backward` (drawing order). Returns the indices after the
    /// operation, for the selection to follow a reordering.
    pub fn arrange(&mut self, indices: &[usize], op: &str) -> Result<Vec<usize>, String> {
        self.touch_target();
        let (sw, sh) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let elements = self.elements_mut()?;
        let mut picked: Vec<usize> = indices.iter().copied().filter(|&i| i < elements.len()).collect();
        picked.sort_unstable();
        picked.dedup();
        if picked.is_empty() {
            return Err("nothing selected".into());
        }
        let rects: Vec<Rect> = picked.iter().map(|&i| *elements[i].rect()).collect();
        let bounds = if picked.len() == 1 && !op.starts_with("mirror") {
            Rect::new(0.0, 0.0, sw, sh)
        } else {
            let x0 = rects.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
            let y0 = rects.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
            let x1 = rects.iter().map(|r| r.x + r.w).fold(f64::NEG_INFINITY, f64::max);
            let y1 = rects.iter().map(|r| r.y + r.h).fold(f64::NEG_INFINITY, f64::max);
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        };
        match op {
            "align_left" | "align_center" | "align_right" | "align_top" | "align_middle" | "align_bottom" => {
                for (&i, r) in picked.iter().zip(&rects) {
                    let mut r = *r;
                    match op {
                        "align_left" => r.x = bounds.x,
                        "align_center" => r.x = bounds.x + (bounds.w - r.w) / 2.0,
                        "align_right" => r.x = bounds.x + bounds.w - r.w,
                        "align_top" => r.y = bounds.y,
                        "align_middle" => r.y = bounds.y + (bounds.h - r.h) / 2.0,
                        _ => r.y = bounds.y + bounds.h - r.h,
                    }
                    elements[i].set_rect(r);
                }
            }
            "distribute_h" | "distribute_v" => {
                if picked.len() < 3 {
                    return Err("select three or more elements to distribute them".into());
                }
                let horizontal = op == "distribute_h";
                let mut order: Vec<usize> = (0..picked.len()).collect();
                order.sort_by(|&a, &b| {
                    let (ra, rb) = (&rects[a], &rects[b]);
                    let (ka, kb) = if horizontal { (ra.x, rb.x) } else { (ra.y, rb.y) };
                    ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
                });
                let total: f64 = rects.iter().map(|r| if horizontal { r.w } else { r.h }).sum();
                let span = if horizontal { bounds.w } else { bounds.h };
                let gap = (span - total) / (picked.len() - 1) as f64;
                let mut at = if horizontal { bounds.x } else { bounds.y };
                for k in order {
                    let mut r = rects[k];
                    if horizontal {
                        r.x = at;
                        at += r.w + gap;
                    } else {
                        r.y = at;
                        at += r.h + gap;
                    }
                    elements[picked[k]].set_rect(r);
                }
            }
            "mirror_h" | "mirror_v" => {
                let horizontal = op == "mirror_h";
                for (&i, r) in picked.iter().zip(&rects) {
                    let mut r = *r;
                    if horizontal {
                        r.x = bounds.x + (bounds.x + bounds.w) - (r.x + r.w);
                    } else {
                        r.y = bounds.y + (bounds.y + bounds.h) - (r.y + r.h);
                    }
                    elements[i].set_rect(r);
                    elements[i].mirror(horizontal);
                }
            }
            "front" | "back" | "forward" | "backward" => {
                let n = elements.len();
                let mut order: Vec<usize> = (0..n).collect();
                let is_picked = |i: usize| picked.binary_search(&i).is_ok();
                match op {
                    "front" => order.sort_by_key(|&i| is_picked(i)),
                    "back" => order.sort_by_key(|&i| !is_picked(i)),
                    "forward" => {
                        for k in (0..n.saturating_sub(1)).rev() {
                            if is_picked(order[k]) && !is_picked(order[k + 1]) {
                                order.swap(k, k + 1);
                            }
                        }
                    }
                    _ => {
                        for k in 1..n {
                            if is_picked(order[k]) && !is_picked(order[k - 1]) {
                                order.swap(k, k - 1);
                            }
                        }
                    }
                }
                let old = std::mem::take(elements);
                let mut slots: Vec<Option<Element>> = old.into_iter().map(Some).collect();
                *elements = order.iter().map(|&i| slots[i].take().expect("each index once")).collect();
                return Ok(order.iter().enumerate().filter(|(_, &i)| is_picked(i)).map(|(k, _)| k).collect());
            }
            other => return Err(format!("unknown arrangement {other}")),
        }
        Ok(picked)
    }

    /// Groups the elements at `indices` of the current slide or layout.
    pub fn group(&mut self, indices: &[usize]) -> Result<usize, String> {
        self.touch_target();
        deck_core::group_elements(self.elements_mut()?, indices)
            .ok_or_else(|| "select at least two elements".to_string())
    }

    /// Ungroups the group at `index`; returns how many elements it held.
    pub fn ungroup(&mut self, index: usize) -> Result<usize, String> {
        self.touch_target();
        deck_core::ungroup_element(self.elements_mut()?, index).ok_or_else(|| "not a group".to_string())
    }

    /// Adds an outline ("plan") element to the current slide.
    /// Sets the poster of the video at `element` from PNG bytes (a frame
    /// captured in the editor), or removes it with `None`.
    pub fn set_poster(&mut self, element: usize, png: Option<&[u8]>) -> Result<(), String> {
        let dir = self.dir.clone();
        self.touch_target();
        let Some(Element::Media(m)) = self.elements_mut()?.get_mut(element) else { return Err("not a video".into()) };
        m.poster = match png {
            Some(bytes) => {
                let stem = Path::new(&m.src).file_stem().and_then(|s| s.to_str()).unwrap_or("video").to_string();
                Some(transfer::add_image(&dir, &format!("{stem}.poster.png"), bytes).map_err(|e| e.to_string())?)
            }
            None => None,
        };
        Ok(())
    }

    /// Adds a table of `rows` by `columns` with a header row.
    pub fn add_table(&mut self, rows: usize, columns: usize) -> Result<usize, String> {
        self.touch_target();
        let (w, h) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let rows = rows.clamp(1, 30);
        let height = (rows as f64 * 36.0).min(h * 0.7);
        let mut table =
            deck_core::Table::new(Rect::new(w * 0.15, (h - height) / 2.0, w * 0.7, height), rows, columns.clamp(1, 12));
        for (j, cell) in table.rows[0].iter_mut().enumerate() {
            *cell = deck_core::Cell::plain(&format!("Column {}", j + 1));
        }
        let elements = self.elements_mut()?;
        elements.push(Element::Table(table));
        Ok(elements.len() - 1)
    }

    /// Makes a cell of the table at `element` the target of in-place text
    /// editing (`text_layout`, `text_edit`, `style_text`).
    pub fn focus_cell(&mut self, element: usize, row: usize, column: usize) -> Result<(), String> {
        let Some(Element::Table(t)) = self.elements_mut()?.get_mut(element) else { return Err("not a table".into()) };
        t.square();
        if row >= t.rows.len() || column >= t.column_count() {
            return Err("no such cell".into());
        }
        // A cell under a merge is typed into through the merged cell.
        let (row, column) = t.owner(row, column);
        self.cell = Some((element, row, column));
        Ok(())
    }

    /// Edits the table at `element`.
    pub fn edit_table(&mut self, element: usize, edit: &TableEdit) -> Result<(), String> {
        self.touch_target();
        let Some(Element::Table(t)) = self.elements_mut()?.get_mut(element) else { return Err("not a table".into()) };
        t.square();
        let columns = t.column_count();
        match edit {
            TableEdit::Cell { row, column, text } => {
                let cell = t.rows.get_mut(*row).and_then(|r| r.get_mut(*column)).ok_or("no such cell")?;
                // Keep the style of the cell's first run for the new text.
                let style = cell.paragraphs.first().and_then(|p| p.runs.first()).cloned().unwrap_or_default();
                let align = cell.paragraphs.first().map(|p| p.align).unwrap_or_default();
                cell.paragraphs = if text.is_empty() {
                    vec![]
                } else {
                    text.split('\n')
                        .map(|line| Paragraph {
                            runs: vec![deck_core::Run { text: line.to_string(), math: false, ..style.clone() }],
                            align,
                            bullet: None,
                        })
                        .collect()
                };
            }
            TableEdit::Fill { row, column, color } => {
                let cell = t.rows.get_mut(*row).and_then(|r| r.get_mut(*column)).ok_or("no such cell")?;
                cell.fill =
                    if color.is_empty() { None } else { Some(Color::from_hex(color).map_err(|e| e.to_string())?) };
            }
            TableEdit::AddRow { at } => {
                let at = (*at).min(t.rows.len());
                t.split_through(Some(at), None);
                t.rows.insert(at, vec![deck_core::Cell::default(); columns]);
                // Rows keep their height: the table grows.
                t.rect.h *= t.rows.len() as f64 / (t.rows.len() - 1) as f64;
            }
            TableEdit::RemoveRow { at } => {
                if t.rows.len() <= 1 || *at >= t.rows.len() {
                    return Err("a table keeps at least one row".into());
                }
                t.split_through(Some(*at), None);
                t.rows.remove(*at);
                t.rect.h *= t.rows.len() as f64 / (t.rows.len() + 1) as f64;
            }
            TableEdit::AddColumn { at } => {
                let at = (*at).min(columns);
                t.split_through(None, Some(at));
                for row in &mut t.rows {
                    row.insert(at, deck_core::Cell::default());
                }
                if !t.columns.is_empty() {
                    t.columns.resize(columns, 1.0);
                    t.columns.insert(at, 1.0);
                }
            }
            TableEdit::RemoveColumn { at } => {
                if columns <= 1 || *at >= columns {
                    return Err("a table keeps at least one column".into());
                }
                t.split_through(None, Some(*at));
                for row in &mut t.rows {
                    row.remove(*at);
                }
                if *at < t.columns.len() {
                    t.columns.remove(*at);
                }
            }
            TableEdit::Header { on } => t.header = *on,
            TableEdit::Merge { from, to } => t.merge(*from, *to),
            TableEdit::Split { row, column } => t.split(*row, *column),
            TableEdit::Columns { widths } => {
                t.columns =
                    if widths.iter().all(|w| *w > 0.0) && widths.len() == columns { widths.clone() } else { vec![] }
            }
        }
        Ok(())
    }

    /// Adds a figure with a sample CeTZ plot, to edit.
    pub fn add_figure(&mut self) -> Result<(), String> {
        self.touch_target();
        let (w, h) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let rect = Rect::new(w * 0.2, h * 0.2, w * 0.6, h * 0.6);
        self.elements_mut()?.push(Element::Figure(deck_core::Figure {
            rect,
            source: SAMPLE_FIGURE.to_string(),
            ..Default::default()
        }));
        Ok(())
    }

    pub fn add_outline(&mut self) -> Result<(), String> {
        self.touch_target();
        let (w, h) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let rect = Rect::new(w * 0.15, h * 0.2, w * 0.7, h * 0.65);
        self.elements_mut()?.push(Element::Outline(deck_core::OutlineBox {
            rect,
            size: Some(28.0),
            subsections: true,
            ..Default::default()
        }));
        Ok(())
    }

    /// Saves the layout under edit (or the current slide's) as a `.deck_layout` folder.
    pub fn save_layout(&self, out_dir: &Path) -> Result<String, String> {
        let deck = self.deck.as_ref().ok_or("no deck")?;
        let name = self
            .editing_layout
            .clone()
            .or_else(|| deck.slides.get(self.slide).and_then(|s| s.layout.clone()))
            .ok_or("no layout to save: edit one or use a slide that has one")?;
        if self.dir.as_os_str().is_empty() {
            return Err("save the deck first, so that images have a folder".into());
        }
        transfer::save_layout(deck, &self.dir, &name, out_dir).map_err(|e| e.to_string())?;
        Ok(name)
    }

    /// Loads a `.deck_layout` folder into the deck and opens it for editing.
    pub fn load_layout(&mut self, src_dir: &Path) -> Result<String, String> {
        if self.dir.as_os_str().is_empty() {
            return Err("save the deck first, so that images have a folder".into());
        }
        let dir = self.dir.clone();
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let name = transfer::load_layout(src_dir, deck, &dir).map_err(|e| e.to_string())?;
        self.editing_layout = Some(name.clone());
        Ok(name)
    }

    pub fn delete_element(&mut self, element: usize) -> Result<(), String> {
        self.touch_target();
        if self.editing_layout.is_some() {
            let layout = self.layout_mut()?;
            if element >= layout.elements.len() {
                let k = element - layout.elements.len();
                if k >= layout.placeholders.len() {
                    return Err("no such element".into());
                }
                layout.placeholders.remove(k);
                return Ok(());
            }
        }
        let elements = self.elements_mut()?;
        if element >= elements.len() {
            return Err("no such element".into());
        }
        elements.remove(element);
        Ok(())
    }
}

/// Replaces `xlink:href="assets/..."` by data URIs: the webview cannot
/// read the deck folder. `cache` holds already encoded images; others are
/// read from `dir`.
fn inline_assets(svg: &str, dir: &Path, cache: &HashMap<String, String>) -> String {
    let marker = r#"xlink:href="assets/"#;
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(i) = rest.find(marker) {
        out.push_str(&rest[..i]);
        let after = &rest[i + r#"xlink:href=""#.len()..];
        let Some(end) = after.find('"') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let src = &after[..end];
        let uri = cache.get(src).cloned().or_else(|| data_uri(&dir.join(src)));
        match uri {
            Some(uri) => {
                out.push_str(r#"xlink:href=""#);
                out.push_str(&uri);
                out.push('"');
            }
            None => out.push_str(&rest[i..i + r#"xlink:href=""#.len() + end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// A file as a `data:` URI, by its extension.
fn data_uri(path: &Path) -> Option<String> {
    let mime = match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    };
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// Applies a placeholder style change.
fn apply_placeholder_style(ph: &mut Placeholder, change: &PlaceholderStyle) -> Result<(), String> {
    if let Some(size) = change.size {
        ph.size = if size > 0.0 { Some(size) } else { None };
    }
    if let Some(b) = change.bold {
        ph.bold = b;
    }
    if let Some(c) = &change.color {
        ph.color = if c.is_empty() { None } else { Some(Color::from_hex(c).map_err(|e| e.to_string())?) };
    }
    if let Some(f) = &change.font {
        ph.font = if f.is_empty() { None } else { Some(f.clone()) };
    }
    if let Some(a) = change.align {
        ph.align = a;
    }
    if let Some(b) = change.bullets {
        ph.bullets = b;
    }
    Ok(())
}

/// Whether two placeholders agree on style property `prop`.
fn same_prop(a: &Placeholder, b: &Placeholder, prop: &str) -> bool {
    let mut c = b.clone();
    copy_prop(&mut c, a, prop);
    c == *b
}

/// Copies style property `prop` of `from` into `to`.
fn copy_prop(to: &mut Placeholder, from: &Placeholder, prop: &str) {
    match prop {
        "size" => to.size = from.size,
        "bold" => to.bold = from.bold,
        "color" => to.color = from.color,
        "font" => to.font.clone_from(&from.font),
        "align" => to.align = from.align,
        "bullets" => to.bullets = from.bullets,
        _ => {}
    }
}

/// The layouts grouped by theme, themes in deck order, then the layouts
/// without a theme (under an empty name).
fn layout_groups(deck: &Deck) -> Vec<LayoutGroup> {
    let mut groups: Vec<LayoutGroup> = deck
        .themes
        .iter()
        .map(|t| LayoutGroup {
            theme: t.name.clone(),
            layouts: deck.layouts_of_theme(&t.name).iter().map(|l| l.name.clone()).collect(),
        })
        .collect();
    let loose: Vec<String> = deck
        .layouts
        .iter()
        .filter(|l| l.parent.as_deref().is_none_or(|p| deck.theme_set(p).is_none()))
        .map(|l| l.name.clone())
        .collect();
    if !loose.is_empty() {
        groups.push(LayoutGroup { theme: String::new(), layouts: loose });
    }
    groups
}

#[cfg(test)]
mod tests {
    #[test]
    fn fonts_and_colors_on_a_slide_are_its_themes() {
        let mut deck = Deck::new("t");
        deck.themes = vec![
            ThemeSet { name: "A".into(), ..Default::default() },
            ThemeSet { name: "B".into(), ..Default::default() },
        ];
        deck.layouts.push(Layout { name: "L".into(), parent: Some("B".into()), ..Default::default() });
        deck.slides = vec![Slide { layout: Some("L".into()), ..Default::default() }, Slide::default()];
        let mut s = AppState::default();
        s.load(deck, std::path::PathBuf::new());
        s.set_theme(&ThemeChange { font: Some("Libertinus Serif".into()), ..Default::default() }).unwrap();
        let d = s.deck.as_ref().unwrap();
        assert_eq!(
            (d.themes[0].style.font.as_deref(), d.themes[1].style.font.as_deref()),
            (None, Some("Libertinus Serif"))
        );
        assert_eq!(s.view().theme.of_theme.as_deref(), Some("B"));
        s.goto(1, 0);
        s.set_theme(&ThemeChange { size: Some(30.0), ..Default::default() }).unwrap();
        assert_eq!(s.deck.as_ref().unwrap().themes[0].style.size, Some(30.0), "no layout: the first theme");
    }

    #[test]
    fn layouts_override_their_theme_and_come_back_to_it() {
        let mut deck = Deck { version: 1, ..Deck::new("t") };
        let title = |size: f64| Placeholder { name: "title".into(), size: Some(size), ..Default::default() };
        let logo = Element::Text(TextBox::plain(Rect::new(0.0, 500.0, 50.0, 20.0), "logo"));
        for name in ["A", "B"] {
            deck.layouts.push(Layout {
                name: name.into(),
                elements: vec![logo.clone()],
                placeholders: vec![title(30.0)],
                ..Default::default()
            });
        }
        deck.slides = vec![Slide { layout: Some("A".into()), ..Default::default() }];
        deck.migrate();
        let mut s = AppState::default();
        s.load(deck, std::path::PathBuf::new());
        s.edit_layout(Some("A".into())).unwrap();
        let size = |s: &AppState, l: &str| s.deck.as_ref().unwrap().resolved_layout(l).unwrap().placeholders[0].size;
        // The title is element 0: the logo belongs to the theme.
        s.style_placeholder(0, &PlaceholderStyle { size: Some(50.0), ..Default::default() }).unwrap();
        assert_eq!((size(&s, "A"), size(&s, "B")), (Some(50.0), Some(30.0)));
        assert_eq!(s.view().layout_meta.unwrap().overrides, ["title: size"]);
        s.reset_to_theme(Some(0), Some("size".into())).unwrap();
        assert_eq!(size(&s, "A"), Some(30.0), "back to the theme");
        s.style_placeholder(0, &PlaceholderStyle { size: Some(44.0), ..Default::default() }).unwrap();
        s.push_to_theme(0, "size").unwrap();
        assert_eq!((size(&s, "A"), size(&s, "B")), (Some(44.0), Some(44.0)), "now the theme's, for both");
        s.hide_theme_elements(true).unwrap();
        assert!(s.deck.as_ref().unwrap().resolved_layout("A").unwrap().elements.is_empty());
        s.reset_to_theme(None, None).unwrap();
        assert_eq!(s.deck.as_ref().unwrap().resolved_layout("A").unwrap().elements.len(), 1);
        let before = s.deck.as_ref().unwrap().resolved_layout("A").unwrap();
        s.set_layout_theme(None).unwrap();
        let after = s.deck.as_ref().unwrap().resolved_layout("A").unwrap();
        assert_eq!(
            (before.elements, before.placeholders[0].size),
            (after.elements, after.placeholders[0].size),
            "leaving the theme keeps the look"
        );
        s.set_layout_theme(Some("Theme".into())).unwrap();
        assert!(s.deck.as_ref().unwrap().layout("A").unwrap().overrides().is_empty(), "back in: nothing differs");
        s.edit_theme(Some("Theme".into())).unwrap();
        assert!(s.view().editing_theme.is_some() && s.view().editing_layout.is_none());
        s.style_theme_placeholder("title", &PlaceholderStyle { bold: Some(true), ..Default::default() }).unwrap();
        assert!(s.deck.as_ref().unwrap().resolved_layout("B").unwrap().placeholders[0].bold);
    }

    #[test]
    fn a_layout_for_several_slides() {
        let mut s = AppState::default();
        let mut deck = Deck::new("t");
        deck.layouts.push(deck_core::Layout { name: "L".into(), ..Default::default() });
        deck.slides = vec![Slide::default(), Slide::default(), Slide::default()];
        s.load(deck, std::path::PathBuf::new());
        s.set_layout_of(&[0, 2], Some("L".into())).unwrap();
        let layouts: Vec<_> = s.deck.as_ref().unwrap().slides.iter().map(|x| x.layout.clone()).collect();
        assert_eq!(layouts, [Some("L".into()), None, Some("L".into())]);
        assert!(s.set_layout_of(&[1], Some("nope".into())).is_err());
        assert!(s.set_layout_of(&[9], None).is_err());
    }

    use super::*;
    use deck_core::{Overrides, Shape, Step};

    fn deck() -> Deck {
        let mut d = Deck::new("t");
        let mut shape = Shape { rect: Rect::new(0.0, 0.0, 10.0, 10.0), ..Default::default() };
        shape.steps = vec![
            Step::disappear(0),
            Step::appear(1),
            Step { at: 2, set: Overrides { x: Some(5.0), ..Default::default() }, ..Default::default() },
        ];
        d.slides.push(Slide { elements: vec![Element::Shape(shape)], ..Default::default() });
        d.slides.push(Slide::default());
        d
    }

    #[test]
    fn next_walks_steps_then_slides() {
        let mut s = AppState { deck: Some(deck()), ..Default::default() };
        let pos = |s: &AppState| (s.slide, s.step);
        assert_eq!(pos(&s), (0, 0));
        s.navigate("next");
        assert_eq!(pos(&s), (0, 1));
        s.navigate("next");
        assert_eq!(pos(&s), (0, 2));
        s.navigate("next");
        assert_eq!(pos(&s), (1, 0));
        s.navigate("next");
        assert_eq!(pos(&s), (1, 0));
        s.navigate("prev");
        assert_eq!(pos(&s), (0, 2), "prev lands on the last step");
        s.navigate("black");
        assert_eq!(s.blank, "black");
        s.navigate("white");
        assert_eq!(s.blank, "white", "white replaces black");
        s.navigate("white");
        assert_eq!(s.blank, "", "a second press restores the slide");
        s.navigate("first");
        assert_eq!(pos(&s), (0, 0));
        let v = s.view();
        assert_eq!((v.last_step, v.slide_count), (2, 2));
        assert!(v.next_svg.is_some());
        let thumbs = s.thumbnails();
        assert_eq!(thumbs.len(), 2);
        assert!(s.thumbs.iter().all(Option::is_some));
        s.touch(Some(1));
        assert!(s.thumbs[0].is_some() && s.thumbs[1].is_none(), "only the edited slide is stale");
    }

    #[test]
    fn move_slide_keeps_the_current_one() {
        let mut s = AppState { deck: Some(deck()), ..Default::default() };
        let d = s.deck.as_mut().unwrap();
        d.slides.push(Slide { name: "c".into(), ..Default::default() });
        d.slides[0].name = "a".into();
        d.slides[1].name = "b".into();
        s.thumbs = vec![Some("A".into()), Some("B".into()), Some("C".into())];
        s.slide = 2;
        s.move_slide(0, 2).unwrap();
        let names: Vec<&str> = s.deck.as_ref().unwrap().slides.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["b", "c", "a"]);
        assert_eq!(s.thumbs, vec![Some("B".into()), Some("C".into()), Some("A".into())], "thumbnails move along");
        assert_eq!(s.slide, 1, "the current slide c moved up by one");
        s.move_slide(2, 0).unwrap();
        assert_eq!(s.slide, 2);
        assert!(s.move_slide(7, 0).is_err());
    }

    #[test]
    fn undo_and_redo_restore_snapshots_and_merge_typing() {
        let mut s = AppState::default();
        s.load(deck(), PathBuf::new());
        assert_eq!(s.can_undo_redo(), (false, false));
        s.checkpoint("add_text", false);
        s.add_text().unwrap();
        let idx = s.deck.as_ref().unwrap().slides[0].elements.len() - 1;
        // Typing: three edits, one undo step.
        for (i, ch) in ["a", "b", "c"].iter().enumerate() {
            s.checkpoint(&format!("text_edit:{idx}"), true);
            s.text_edit(idx, 4 + i, 4 + i, ch).unwrap();
        }
        assert_eq!(s.undo.len(), 2);
        let text_of = |s: &AppState| match &s.deck.as_ref().unwrap().slides[0].elements[idx] {
            Element::Text(t) => t.text(),
            _ => unreachable!(),
        };
        s.undo().unwrap();
        assert_eq!(text_of(&s), "Text");
        assert_eq!(s.can_undo_redo(), (true, true));
        s.undo().unwrap();
        assert_eq!(s.deck.as_ref().unwrap().slides[0].elements.len(), 1);
        assert!(s.undo().is_err());
        s.redo().unwrap();
        s.redo().unwrap();
        assert_eq!(text_of(&s), "Textabc");
        // A new edit after undo drops the redo stack; navigation ends a typing run.
        s.undo().unwrap();
        s.break_merge();
        s.checkpoint(&format!("text_edit:{idx}"), true);
        s.text_edit(idx, 0, 0, "x").unwrap();
        assert_eq!(s.can_undo_redo(), (true, false));
        assert_eq!(s.undo.len(), 2);
    }

    #[test]
    fn gif_info_reads_frames_duration_and_poster() {
        let mut bytes = Vec::new();
        {
            let mut enc = gif::Encoder::new(&mut bytes, 4, 3, &[]).unwrap();
            for (i, delay) in [(0u8, 50u16), (255, 0)] {
                let mut frame = gif::Frame::from_rgba(4, 3, &mut [i; 4 * 3 * 4].to_vec());
                frame.delay = delay;
                enc.write_frame(&frame).unwrap();
            }
        }
        let info = gif_info(&bytes).unwrap();
        assert_eq!(info.frames, 2);
        assert!((info.duration - 0.6).abs() < 1e-9, "50 cs + a 0 delay read as 10 cs: {}", info.duration);
        assert_eq!(deck_core::transfer::image_size(&info.first_frame_png), Some((4, 3)));
        let mut s = AppState::default();
        s.load(deck(), std::env::temp_dir().join(format!("decker-gif-{}", std::process::id())));
        let idx = s.add_image("anim.gif", &bytes, None).unwrap();
        let Element::Media(m) = &s.deck.as_ref().unwrap().slides[0].elements[idx] else { panic!("a media element") };
        assert_eq!(m.poster.as_deref(), Some("assets/anim.poster.png"));
        assert_eq!(m.duration, Some(0.6));
        assert!(m.is_gif() && !m.looping && m.play == Play::OnAppear);
        let idx = s.add_image("clip.mp4", b"not really a video", None).unwrap();
        assert!(matches!(&s.deck.as_ref().unwrap().slides[0].elements[idx], Element::Media(m) if m.poster.is_none()));
        let _ = std::fs::remove_dir_all(&s.dir);
    }

    #[test]
    fn arrange_aligns_distributes_mirrors_and_reorders() {
        let mut s = AppState::default();
        let mut d = Deck::new("t");
        let boxes = [(0.0, 0.0, 10.0), (50.0, 20.0, 20.0), (170.0, 60.0, 30.0)];
        d.slides.push(Slide {
            elements: boxes
                .iter()
                .map(|&(x, y, w)| Element::Shape(Shape { rect: Rect::new(x, y, w, 10.0), ..Default::default() }))
                .collect(),
            ..Default::default()
        });
        s.load(d, PathBuf::new());
        let xs = |s: &AppState| -> Vec<f64> {
            s.deck.as_ref().unwrap().slides[0].elements.iter().map(|e| e.rect().x).collect()
        };
        let ys = |s: &AppState| -> Vec<f64> {
            s.deck.as_ref().unwrap().slides[0].elements.iter().map(|e| e.rect().y).collect()
        };
        s.arrange(&[0, 1, 2], "distribute_h").unwrap();
        assert_eq!(xs(&s), [0.0, 80.0, 170.0], "gaps of 70 between widths 10, 20, 30 over 200");
        s.arrange(&[0, 1, 2], "align_top").unwrap();
        assert_eq!(ys(&s), [0.0, 0.0, 0.0]);
        s.arrange(&[0, 2], "align_right").unwrap();
        assert_eq!(xs(&s), [190.0, 80.0, 170.0]);
        s.arrange(&[1], "align_center").unwrap();
        assert_eq!(xs(&s)[1], 470.0, "one element aligns to the slide");
        s.arrange(&[0, 2], "mirror_h").unwrap();
        assert_eq!(xs(&s), [170.0, 470.0, 170.0], "positions mirror within the selection's bounds (170..200)");
        assert!(s.arrange(&[0, 1], "distribute_v").is_err());
        s.arrange(&[1], "align_left").unwrap();
        assert_eq!(s.arrange(&[0], "front").unwrap(), [2]);
        assert_eq!(xs(&s), [0.0, 170.0, 170.0]);
        assert_eq!(s.arrange(&[2], "backward").unwrap(), [1]);
        assert_eq!(s.arrange(&[1, 2], "back").unwrap(), [0, 1]);
        assert_eq!(xs(&s), [170.0, 170.0, 0.0]);
        s.set_rects(&[(0, Rect::new(1.0, 2.0, 3.0, 4.0)), (1, Rect::new(5.0, 6.0, 7.0, 8.0))]).unwrap();
        assert_eq!(xs(&s)[..2], [1.0, 5.0]);
    }

    #[cfg(feature = "typst")]
    #[test]
    fn table_cells_are_edited_in_place() {
        let mut s = AppState::default();
        s.load(deck(), PathBuf::new());
        let i = s.add_table(2, 2).unwrap();
        assert!(s.text_layout(i).is_err(), "no cell picked yet");
        s.focus_cell(i, 1, 1).unwrap();
        let empty = s.text_layout(i).unwrap().unwrap();
        let cell = s.view().elements[i].table.as_ref().unwrap().rects[1][1];
        assert_eq!(empty.rect, cell);
        assert_eq!(empty.chars.len(), 1, "just the caret");
        assert!(empty.chars[0].x >= cell.x && empty.chars[0].top >= cell.y);
        assert_eq!(s.text_edit(i, 0, 0, "42").unwrap(), 2);
        s.style_text(i, 0, 2, &text::StyleChange { toggle_bold: true, ..Default::default() }).unwrap();
        let Element::Table(t) = &s.deck.as_ref().unwrap().slides[0].elements[i] else { unreachable!() };
        assert_eq!(t.rows[1][1].text(), "42");
        assert!(t.rows[1][1].paragraphs[0].runs[0].bold);
        assert!(t.rows[0][0].text().starts_with("Column"), "other cells untouched");
        // The header row is laid out as drawn: bold, so wider than plain.
        s.focus_cell(i, 0, 0).unwrap();
        let head = s.text_layout(i).unwrap().unwrap();
        s.edit_table(i, &TableEdit::Header { on: false }).unwrap();
        let plain = s.text_layout(i).unwrap().unwrap();
        assert!(head.chars.last().unwrap().x > plain.chars.last().unwrap().x);
        assert!(s.focus_cell(i, 5, 0).is_err());
    }

    #[test]
    fn tables_are_added_and_edited() {
        let mut s = AppState::default();
        s.load(deck(), PathBuf::new());
        let i = s.add_table(3, 2).unwrap();
        let table = |s: &AppState| match &s.deck.as_ref().unwrap().slides[0].elements[i] {
            Element::Table(t) => t.clone(),
            _ => unreachable!(),
        };
        assert_eq!(table(&s).rows[0][1].text(), "Column 2");
        s.edit_table(i, &TableEdit::Cell { row: 1, column: 0, text: "a\nb".into() }).unwrap();
        assert_eq!(table(&s).rows[1][0].paragraphs.len(), 2);
        let h = table(&s).rect.h;
        s.edit_table(i, &TableEdit::AddRow { at: 1 }).unwrap();
        s.edit_table(i, &TableEdit::AddColumn { at: 99 }).unwrap();
        let t = table(&s);
        assert_eq!((t.rows.len(), t.column_count()), (4, 3));
        assert!((t.rect.h - h * 4.0 / 3.0).abs() < 1e-9, "rows keep their height");
        assert_eq!(t.rows[2][0].text(), "a\nb", "the edited row moved down");
        s.edit_table(i, &TableEdit::Fill { row: 2, column: 0, color: "#102030".into() }).unwrap();
        s.edit_table(i, &TableEdit::RemoveColumn { at: 1 }).unwrap();
        s.edit_table(i, &TableEdit::RemoveRow { at: 0 }).unwrap();
        s.edit_table(i, &TableEdit::Header { on: false }).unwrap();
        let t = table(&s);
        assert_eq!((t.rows.len(), t.column_count(), t.header), (3, 2, false));
        assert_eq!(t.rows[1][0].fill, Some(Color::rgb(0x10, 0x20, 0x30)));
        let view = s.view();
        let info = view.elements[i].table.as_ref().unwrap();
        assert_eq!(info.cells[1][0], "a\nb");
        assert_eq!(view.elements[i].kind, "table");
        assert!(s.edit_table(0, &TableEdit::Header { on: true }).is_err());
    }

    #[test]
    fn rotation_is_a_step_at_the_viewed_click() {
        let mut s = AppState::default();
        s.load(deck(), PathBuf::new());
        s.goto(1, 0);
        s.add_text().unwrap();
        s.set_rotation(0, 370.0).unwrap();
        let e = |s: &AppState| s.deck.as_ref().unwrap().slides[1].elements[0].clone();
        assert_eq!(e(&s).state_at(0).rotation, 10.0);
        assert_eq!(s.view().elements[0].rotation, 10.0);
        s.set_rotation(0, 0.0).unwrap();
        assert!(e(&s).steps().is_empty(), "back to no rotation leaves no step");
        // At a later click of a slide with steps: a rotation step there.
        s.goto(0, 1);
        s.set_rotation(0, 45.0).unwrap();
        let shape = s.deck.as_ref().unwrap().slides[0].elements[0].clone();
        assert_eq!((shape.state_at(0).rotation, shape.state_at(1).rotation), (0.0, 45.0));
    }

    #[test]
    fn a_video_takes_a_poster() {
        let mut s = AppState::default();
        s.load(deck(), std::env::temp_dir().join(format!("decker-poster-{}", std::process::id())));
        let i = s.add_image("clip.mp4", b"video bytes", None).unwrap();
        s.set_poster(i, Some(b"png")).unwrap();
        let poster = |s: &AppState| match &s.deck.as_ref().unwrap().slides[0].elements[i] {
            Element::Media(m) => m.poster.clone(),
            _ => unreachable!(),
        };
        assert_eq!(poster(&s).as_deref(), Some("assets/clip.poster.png"));
        assert!(s.dir.join("assets/clip.poster.png").is_file());
        s.set_poster(i, None).unwrap();
        assert_eq!(poster(&s), None);
        assert!(s.set_poster(0, None).is_err());
        let _ = std::fs::remove_dir_all(&s.dir);
    }

    #[test]
    fn a_duplicate_starts_no_section() {
        let mut d = deck();
        d.slides[0].section = Some("Intro".into());
        d.slides[0].subsection = Some("Start".into());
        let mut s = AppState::default();
        s.load(d, PathBuf::new());
        s.duplicate_slide().unwrap();
        let slides = &s.deck.as_ref().unwrap().slides;
        assert_eq!((slides[1].section.as_deref(), slides[1].subsection.as_deref()), (None, None));
        assert_eq!(slides[0].section.as_deref(), Some("Intro"));
        assert_eq!(s.deck.as_ref().unwrap().position(1).0, Some("Intro"));
    }

    #[test]
    fn inline_assets_embeds_files() {
        let dir = std::env::temp_dir().join(format!("decker-inline-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("assets/a.png"), b"abc").unwrap();
        let svg = r#"<image xlink:href="assets/a.png"/><image xlink:href="assets/missing.png"/>"#;
        let out = inline_assets(svg, &dir, &HashMap::new());
        assert_eq!(out, r#"<image xlink:href="data:image/png;base64,YWJj"/><image xlink:href="assets/missing.png"/>"#);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
