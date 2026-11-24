//! Application state and the view sent to the windows.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

use deck_core::{
    native, svg, text, transfer, Color, Crop, Deck, Element, Layout, Mask, MathRenderer, Paragraph, Placeholder, Rect,
    Slide, Step, Stroke, TextBox, TextDefaults, Theme,
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
    pub warnings: Vec<String>,
    /// Rendered thumbnails, one per slide; `None` when stale.
    thumbs: Vec<Option<String>>,
    /// Images as data URIs, by deck-relative path.
    pub assets: HashMap<String, String>,
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
    pub own: Theme,
    pub effective: Theme,
    /// Font families the typesetter knows, bundled and from `fonts/`.
    pub fonts: Vec<String>,
}

/// The character boxes of a text element, in slide points.
#[derive(Debug, Clone, Serialize)]
pub struct TextLayout {
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
    /// The theme of the layout under edit, else of the current slide.
    pub theme: ThemeInfo,
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
}

fn element_style_info(e: &Element) -> (Option<Color>, Option<Stroke>, Option<Color>, Option<bool>) {
    match e {
        Element::Text(t) => (t.fill, t.stroke, None, None),
        Element::Shape(s) => (s.fill, s.stroke, None, None),
        Element::Math(m) => (m.color, None, None, None),
        Element::Outline(o) => (o.color, None, o.highlight, Some(o.subsections)),
        Element::Image(_) | Element::Group(_) => (None, None, None, None),
    }
}

/// Whether an element (or a group) holds an image.
fn has_image(e: &Element) -> bool {
    match e {
        Element::Image(_) => true,
        Element::Group(g) => g.children.iter().any(has_image),
        _ => false,
    }
}

impl AppState {
    pub fn load(&mut self, deck: Deck, dir: PathBuf) {
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
        inline_assets(&svg::render_step_with(deck, slide, step, &*self.math.0), &self.dir, &self.assets)
    }

    /// Thumbnails of every slide at its final step, rendering only the
    /// stale ones, and caching image data on the way.
    pub fn thumbnails(&mut self) -> Vec<String> {
        let Some(deck) = self.deck.take() else { return vec![] };
        self.thumbs.resize(deck.slides.len(), None);
        for (i, slide) in deck.slides.iter().enumerate() {
            if self.thumbs[i].is_none() {
                self.cache_assets(slide);
                self.thumbs[i] = Some(self.render(&deck, slide, slide.last_step()));
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
                theme: ThemeInfo::default(),
            };
        };
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
                        Element::Shape(_) => "shape",
                        Element::Math(_) => "math",
                        Element::Outline(_) => "outline",
                        Element::Group(_) => "group",
                    },
                    // As shown at the viewed click: a move step changes it.
                    rect: e.state_at(self.step).rect,
                    text: match e {
                        Element::Text(t) => t.text(),
                        Element::Shape(s) => s.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n"),
                        Element::Math(m) => m.source.clone(),
                        Element::Image(i) => i.src.clone(),
                        Element::Outline(_) => String::new(),
                        Element::Group(g) => format!("{} elements", g.children.len()),
                    },
                    last_step: e.last_step(),
                    size: match e {
                        Element::Text(t) => t.paragraphs.first().and_then(|p| p.runs.first()).and_then(|r| r.size),
                        Element::Shape(s) => s.paragraphs.first().and_then(|p| p.runs.first()).and_then(|r| r.size),
                        Element::Math(m) => m.size,
                        Element::Outline(o) => o.size,
                        Element::Image(_) | Element::Group(_) => None,
                    }
                    .unwrap_or(svg::DEFAULT_FONT_SIZE),
                    crop: if let Element::Image(i) = e { Some(i.crop) } else { None },
                    mask: if let Element::Image(i) = e { Some(i.mask) } else { None },
                    steps: e.steps().to_vec(),
                    fill: element_style_info(e).0,
                    stroke: element_style_info(e).1,
                    highlight: element_style_info(e).2,
                    subsections: element_style_info(e).3,
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
            theme: ThemeInfo { own: deck.theme.clone(), effective: deck.theme_of(slide), fonts: self.math.fonts() },
        }
    }

    /// The view of a layout under edit: its static elements, then its
    /// placeholders as elements of kind `placeholder` whose text is the name.
    fn layout_view(&self, deck: &Deck, layout: &Layout) -> View {
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
                    Element::Shape(_) => "shape",
                    Element::Math(_) => "math",
                    Element::Outline(_) => "outline",
                    Element::Group(_) => "group",
                },
                rect: *e.rect(),
                text: match e {
                    Element::Text(t) => t.text(),
                    Element::Shape(s) => s.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n"),
                    Element::Math(m) => m.source.clone(),
                    Element::Image(i) => i.src.clone(),
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
            })
            .collect();
        for (k, ph) in layout.placeholders.iter().enumerate() {
            elements.push(ElementInfo {
                index: layout.elements.len() + k,
                kind: "placeholder",
                rect: ph.rect,
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
            theme: ThemeInfo {
                own: layout.theme.clone(),
                effective: deck.theme_of_layout(layout),
                fonts: self.math.fonts(),
            },
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
        let theme = if self.editing_layout.is_some() {
            &mut self.layout_mut()?.theme
        } else {
            &mut self.deck.as_mut().ok_or("no deck")?.theme
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

    // ----- layouts -----

    /// Enters (`Some(name)`) or leaves (`None`) the layout editor.
    pub fn edit_layout(&mut self, name: Option<String>) -> Result<(), String> {
        if let Some(n) = &name {
            let deck = self.deck.as_ref().ok_or("no deck")?;
            if deck.layout(n).is_none() {
                return Err(format!("no layout named {n}"));
            }
        }
        self.editing_layout = name;
        Ok(())
    }

    /// A new empty layout, entered for editing. Returns its name.
    pub fn new_layout(&mut self) -> Result<String, String> {
        let deck = self.deck.as_mut().ok_or("no deck")?;
        let mut n = deck.layouts.len() + 1;
        let mut name = format!("Layout {n}");
        while deck.layout(&name).is_some() {
            n += 1;
            name = format!("Layout {n}");
        }
        deck.layouts.push(Layout { name: name.clone(), ..Default::default() });
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
    pub fn add_placeholder(&mut self) -> Result<(), String> {
        let (w, h) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let layout = self.layout_mut()?;
        let mut n = layout.placeholders.len() + 1;
        let mut name = format!("body{n}");
        while layout.placeholder(&name).is_some() {
            n += 1;
            name = format!("body{n}");
        }
        layout.placeholders.push(Placeholder {
            name,
            rect: Rect::new(w * 0.1, h * 0.3, w * 0.8, h * 0.4),
            bullets: true,
            ..Default::default()
        });
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
        if self.editing_layout.is_some() {
            Ok(&mut self.layout_mut()?.elements)
        } else {
            Ok(&mut self.slide_mut()?.elements)
        }
    }

    /// Marks what the current edit changes: the slide, or every slide of
    /// the layout under edit.
    fn touch_target(&mut self) {
        if self.editing_layout.is_some() {
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
    pub fn export_pdf(&mut self, out: &Path) -> Result<(), String> {
        let Some(deck) = self.deck.take() else { return Err("no deck open".into()) };
        for slide in &deck.slides {
            self.cache_assets(slide);
        }
        let result = self.pdf(&deck, out);
        self.deck = Some(deck);
        result
    }

    #[cfg(feature = "typst")]
    fn pdf(&self, deck: &Deck, out: &Path) -> Result<(), String> {
        let engine = deck_typst::Engine::for_deck(&self.dir);
        engine
            .export_pdf(deck, |i| self.render(deck, &deck.slides[i], deck.slides[i].last_step()), out)
            .map_err(|e| e.to_string())
    }

    #[cfg(not(feature = "typst"))]
    fn pdf(&self, _deck: &Deck, _out: &Path) -> Result<(), String> {
        Err("PDF export needs the typst feature".into())
    }

    pub fn navigate(&mut self, action: &str) {
        self.editing_layout = None;
        let Some((deck, slide)) = self.current() else { return };
        let count = deck.slides.len();
        let last_step = slide.last_step();
        let last_steps: Vec<u32> = deck.slides.iter().map(Slide::last_step).collect();
        match action {
            "next" => {
                if self.step < last_step {
                    self.step += 1;
                } else if self.slide + 1 < count {
                    self.slide += 1;
                    self.step = 0;
                }
            }
            "prev" => {
                if self.step > 0 {
                    self.step -= 1;
                } else if self.slide > 0 {
                    self.slide -= 1;
                    self.step = last_steps[self.slide];
                }
            }
            "next_slide" => {
                if self.slide + 1 < count {
                    self.slide += 1;
                    self.step = 0;
                }
            }
            "prev_slide" => {
                if self.slide > 0 {
                    self.slide -= 1;
                    self.step = 0;
                }
            }
            "first" => {
                self.slide = 0;
                self.step = 0;
            }
            "last" => {
                self.slide = count.saturating_sub(1);
                self.step = 0;
            }
            "black" | "blackout" => self.blank = if self.blank == "black" { String::new() } else { "black".into() },
            "white" => self.blank = if self.blank == "white" { String::new() } else { "white".into() },
            _ => {}
        }
    }

    pub fn goto(&mut self, slide: usize, step: u32) {
        self.editing_layout = None;
        if let Some(deck) = &self.deck {
            if slide < deck.slides.len() {
                self.slide = slide;
                self.step = step.min(deck.slides[slide].last_step());
            }
        }
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
        if step > 0 && self.editing_layout.is_none() {
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
            Element::Image(_) => return Err("an image has no text".into()),
            Element::Outline(_) => return Err("the outline is computed from the sections".into()),
            Element::Group(_) => return Err("ungroup to edit the texts of a group".into()),
        }
        Ok(())
    }

    /// The paragraphs of a text box or shape, for the caret editor.
    fn paragraphs_mut(&mut self, element: usize) -> Result<&mut Vec<Paragraph>, String> {
        match self.elements_mut()?.get_mut(element) {
            Some(Element::Text(t)) => Ok(&mut t.paragraphs),
            Some(Element::Shape(s)) => Ok(&mut s.paragraphs),
            Some(_) => Err("not a text element".into()),
            None => Err("no such element".into()),
        }
    }

    /// Character boxes of a text element, in slide coordinates, plus the
    /// caret box after the last character. `None` without the typesetter.
    pub fn text_layout(&self, element: usize) -> Result<Option<TextLayout>, String> {
        let (deck, slide) = self.current().ok_or("no slide")?;
        let elements = match self.editing_layout.as_deref().and_then(|n| deck.layout(n)) {
            Some(layout) => &layout.elements,
            None => &slide.elements,
        };
        let shown = |e: &Element| if self.editing_layout.is_some() { *e.rect() } else { e.state_at(self.step).rect };
        let (rect, paragraphs) = match elements.get(element) {
            Some(e @ Element::Text(t)) => (shown(e), &t.paragraphs),
            Some(e @ Element::Shape(s)) => (shown(e), &s.paragraphs),
            Some(_) => return Err("not a text element".into()),
            None => return Err("no such element".into()),
        };
        let defaults = match self.editing_layout.as_deref().and_then(|n| deck.layout(n)) {
            Some(layout) => deck.theme_of_layout(layout).defaults(),
            None => deck.theme_of(slide).defaults(),
        };
        let width = (rect.w - 2.0 * svg::PADDING).max(1.0);
        let Some(boxes) = self.math.layout(&deck.math_preamble, paragraphs, width, &defaults) else {
            return Ok(None);
        };
        let (ox, oy) = (rect.x + svg::PADDING, rect.y + svg::PADDING);
        Ok(Some(TextLayout {
            text: text::flat_text(paragraphs),
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
        let copy = deck.slides.get(self.slide).cloned().ok_or("no slide")?;
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
        let name = match ext.as_deref() {
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg") => name.to_string(),
            _ => format!("{}.png", Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("image")),
        };
        let src = transfer::add_image(&self.dir, &name, bytes).map_err(|e| e.to_string())?;
        self.touch_target();
        let (sw, sh) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let (pw, ph) =
            transfer::image_size(bytes).map(|(w, h)| (f64::from(w), f64::from(h))).unwrap_or((sw * 0.4, sh * 0.4));
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
        elements.push(Element::Image(deck_core::Image { rect, src, ..Default::default() }));
        Ok(elements.len() - 1)
    }

    /// The clipboard text for `indices` of the current slide or layout.
    pub fn copy_elements(&self, indices: &[usize]) -> Result<String, String> {
        let (deck, slide) = self.current().ok_or("no slide")?;
        let elements = match self.editing_layout.as_deref().and_then(|n| deck.layout(n)) {
            Some(layout) => &layout.elements,
            None => &slide.elements,
        };
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
            Element::Image(_) | Element::Group(_) => return Err("select one element to style it".into()),
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

#[cfg(test)]
mod tests {
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
