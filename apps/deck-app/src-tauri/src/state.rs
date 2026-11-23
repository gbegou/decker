//! Application state and the view sent to the windows.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

use deck_core::{native, svg, transfer, Deck, Element, MathRenderer, Paragraph, Rect, Slide, TextBox};

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
    pub warnings: Vec<String>,
    /// Rendered thumbnails, one per slide; `None` when stale.
    thumbs: Vec<Option<String>>,
    /// Images as data URIs, by deck-relative path.
    assets: HashMap<String, String>,
}

/// The formula renderer, chosen at compile time. With Typst, it serves
/// the `typst/*.typ` files of the open deck.
pub struct Math(Box<dyn MathRenderer + Send + Sync>);

impl Default for Math {
    fn default() -> Self {
        Self::for_deck(Path::new(""))
    }
}

impl Math {
    #[cfg(feature = "typst")]
    pub fn for_deck(dir: &Path) -> Self {
        Math(Box::new(deck_typst::Engine::for_deck(dir)))
    }

    #[cfg(not(feature = "typst"))]
    pub fn for_deck(_dir: &Path) -> Self {
        Math(Box::new(deck_core::NoMath))
    }
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
}

#[derive(Debug, Clone, Serialize)]
pub struct ElementInfo {
    pub index: usize,
    pub kind: &'static str,
    pub rect: Rect,
    pub text: String,
    pub last_step: u32,
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
        let srcs: Vec<String> = slide
            .elements
            .iter()
            .filter_map(|e| if let Element::Image(i) = e { Some(i.src.clone()) } else { None })
            .filter(|src| !self.assets.contains_key(src))
            .collect();
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
            };
        };
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
                    },
                    rect: *e.rect(),
                    text: match e {
                        Element::Text(t) => t.text(),
                        Element::Shape(s) => s.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n"),
                        Element::Math(m) => m.source.clone(),
                        Element::Image(i) => i.src.clone(),
                    },
                    last_step: e.last_step(),
                })
                .collect(),
            presenting: self.presenting,
            blank: self.blank.clone(),
            warnings: self.warnings.clone(),
            layouts: deck.layouts.iter().map(|l| l.name.clone()).collect(),
            layout: slide.layout.clone(),
        }
    }

    pub fn navigate(&mut self, action: &str) {
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
        self.touch(Some(self.slide));
        let slide = self.slide_mut()?;
        let e = slide.elements.get_mut(element).ok_or("no such element")?;
        *e.rect_mut() = Rect { w: rect.w.max(1.0), h: rect.h.max(1.0), ..rect };
        Ok(())
    }

    pub fn set_text(&mut self, element: usize, text: &str) -> Result<(), String> {
        self.touch(Some(self.slide));
        let slide = self.slide_mut()?;
        let e = slide.elements.get_mut(element).ok_or("no such element")?;
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
        }
        Ok(())
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

    pub fn add_text(&mut self) -> Result<(), String> {
        self.touch(Some(self.slide));
        let (w, h) = self.deck.as_ref().map(|d| (d.width, d.height)).ok_or("no deck")?;
        let slide = self.slide_mut()?;
        let rect = Rect::new(w * 0.1, h * 0.4, w * 0.8, h * 0.15);
        slide.elements.push(Element::Text(TextBox::plain(rect, "Text")));
        Ok(())
    }

    pub fn delete_element(&mut self, element: usize) -> Result<(), String> {
        self.touch(Some(self.slide));
        let slide = self.slide_mut()?;
        if element >= slide.elements.len() {
            return Err("no such element".into());
        }
        slide.elements.remove(element);
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
