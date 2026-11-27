//! The document model.

use serde::{Deserialize, Serialize};

pub use crate::themes::ThemeSet;

/// Version of the `deck.json` schema. Bump it with a migration when the
/// model changes incompatibly.
pub const FORMAT_VERSION: u32 = 4;

/// The first version with themes holding layouts (see [`crate::themes`]).
pub const THEMES_VERSION: u32 = 2;

/// The first version with fonts and colors in the themes only (the deck's
/// own `theme` is merged into them, see [`Deck::migrate`]).
pub const FONTS_IN_THEMES_VERSION: u32 = 3;

/// The first version where a theme is a layout too: its title and body
/// placeholders have positions, and a slide can use the theme itself.
pub const THEME_LAYOUT_VERSION: u32 = 4;

/// Width of a 16:9 slide, in points.
pub const DEFAULT_WIDTH: f64 = 960.0;
/// Height of a 16:9 slide, in points.
pub const DEFAULT_HEIGHT: f64 = 540.0;

/// A whole presentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    /// Schema version, see [`FORMAT_VERSION`].
    pub version: u32,
    pub title: String,
    /// Slide width in points.
    pub width: f64,
    /// Slide height in points.
    pub height: f64,
    /// Typst code prepended to every formula: `#let` definitions of the
    /// deck's notations, `#import "file.typ": *` of files in the deck's
    /// `typst/` folder.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub math_preamble: String,
    /// The themes: what a family of layouts shares (PowerPoint's slide
    /// masters), see [`crate::themes`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<ThemeSet>,
    /// The layouts slides can use; each may belong to a theme and then
    /// holds only what it changes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layouts: Vec<Layout>,
    /// Fonts and colors of decks saved before version 3: merged into the
    /// themes when the deck is opened, then empty. Fonts and colors now
    /// belong to the themes (and a layout's overrides).
    #[serde(default, skip_serializing_if = "is_default")]
    pub theme: Theme,
    pub slides: Vec<Slide>,
}

/// Default text font, size and colors, and a palette of swatches.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Theme {
    /// Font family of text that sets none (bundled, or in the deck's `fonts/`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Size in points of text that sets none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// Color of text that sets none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// The accent: default highlight of the plan, first swatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<Color>,
    /// Swatches offered by the color pickers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<Color>,
}

impl Theme {
    /// `self` with every unset field taken from `fallback`.
    pub fn over(&self, fallback: &Theme) -> Theme {
        Theme {
            font: self.font.clone().or_else(|| fallback.font.clone()),
            size: self.size.or(fallback.size),
            color: self.color.or(fallback.color),
            accent: self.accent.or(fallback.accent),
            palette: if self.palette.is_empty() { fallback.palette.clone() } else { self.palette.clone() },
        }
    }

    /// The text defaults the renderer uses.
    pub fn defaults(&self) -> TextDefaults {
        TextDefaults { font: self.font.clone(), size: self.size, color: self.color }
    }
}

/// What text without a font, size or color of its own gets.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextDefaults {
    pub font: Option<String>,
    pub size: Option<f64>,
    pub color: Option<Color>,
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            title: String::new(),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            math_preamble: String::new(),
            themes: Vec::new(),
            layouts: Vec::new(),
            theme: Theme::default(),
            slides: Vec::new(),
        }
    }
}

impl Deck {
    /// The prefix of placeholder `name` on slide `index`, its variables
    /// filled in; `None` without one.
    pub fn placeholder_prefix(&self, index: usize, name: &str) -> Option<String> {
        let slide = self.slides.get(index)?;
        let layout = self.layout_of(slide)?;
        let prefix = &layout.placeholder(name)?.prefix;
        (!prefix.is_empty()).then(|| crate::vars::Vars::for_slide(self, index).substitute(prefix))
    }

    /// A copy with every placeholder prefix written into the slides' text,
    /// as drawn: for the exports.
    pub fn with_prefixes(&self) -> Deck {
        let mut out = self.clone();
        for i in 0..self.slides.len() {
            for e in &mut out.slides[i].elements {
                if let Element::Text(t) = e {
                    if let Some(prefix) = t.placeholder.as_deref().and_then(|n| self.placeholder_prefix(i, n)) {
                        t.paragraphs = crate::text::with_prefix(&t.paragraphs, &prefix);
                    }
                }
            }
        }
        out
    }

    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), ..Default::default() }
    }

    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.layouts.iter().find(|l| l.name == name)
    }

    /// The layout of a slide, when it names one that exists, resolved
    /// against its theme (see [`Deck::resolve`]).
    pub fn layout_of(&self, slide: &Slide) -> Option<Layout> {
        slide.layout.as_deref().and_then(|n| self.resolved_layout(n))
    }

    /// The layout named `name`, resolved against its theme; a theme's name
    /// gives the theme used as a layout (see [`ThemeSet::as_layout`]).
    pub fn resolved_layout(&self, name: &str) -> Option<Layout> {
        match self.layout(name) {
            Some(l) => Some(self.resolve(l)),
            None => self.theme_set(name).map(|t| self.resolve(&t.as_layout())),
        }
    }

    /// Whether a slide can use `name`: a layout, or a theme.
    pub fn has_layout(&self, name: &str) -> bool {
        self.layout(name).is_some() || self.theme_set(name).is_some()
    }

    /// `base`, or `base 2`, `base 3`… : a name no layout or theme has (a
    /// slide names either).
    pub fn free_name(&self, base: &str) -> String {
        let mut name = base.to_string();
        let mut n = 1;
        while self.has_layout(&name) {
            n += 1;
            name = format!("{base} {n}");
        }
        name
    }

    /// The theme named `name`.
    pub fn theme_set(&self, name: &str) -> Option<&ThemeSet> {
        self.themes.iter().find(|t| t.name == name)
    }

    /// The text style in force on a slide: its layout's (with its theme's)
    /// over the deck's.
    pub fn theme_of(&self, slide: &Slide) -> Theme {
        match self.layout_of(slide) {
            Some(l) if l.parent.is_some() => l.theme.over(&self.theme),
            // A layout without a theme, or none: over the deck's first theme.
            Some(l) => l.theme.over(&self.first_style()).over(&self.theme),
            None => self.first_style().over(&self.theme),
        }
    }

    /// The fonts and colors of the deck's first theme (slides without a
    /// layout use them).
    pub fn first_style(&self) -> Theme {
        self.themes.first().map(|t| t.style.clone()).unwrap_or_default()
    }

    /// The theme whose fonts and colors a slide uses: its layout's, else
    /// the first one.
    pub fn style_theme_of(&self, slide: &Slide) -> Option<&str> {
        let parent = slide.layout.as_deref().and_then(|n| match self.layout(n) {
            Some(l) => l.parent.as_deref(),
            None => self.theme_set(n).map(|t| t.name.as_str()),
        });
        parent.filter(|p| self.theme_set(p).is_some()).or_else(|| self.themes.first().map(|t| t.name.as_str()))
    }

    /// The text style in force on a layout (its own, its theme's, the deck's).
    pub fn theme_of_layout(&self, layout: &Layout) -> Theme {
        let style = self.resolve(layout).theme;
        let style = if layout.parent.is_some() { style } else { style.over(&self.first_style()) };
        style.over(&self.theme)
    }

    /// Sections and subsections in slide order, each with the index of
    /// the slide that starts it.
    pub fn outline(&self) -> Vec<OutlineEntry> {
        let mut out = vec![];
        for (i, s) in self.slides.iter().enumerate() {
            if let Some(t) = &s.section {
                out.push(OutlineEntry { level: 1, title: t.clone(), slide: i });
            }
            if let Some(t) = &s.subsection {
                out.push(OutlineEntry { level: 2, title: t.clone(), slide: i });
            }
        }
        out
    }

    /// The section and subsection a slide belongs to.
    pub fn position(&self, slide: usize) -> (Option<&str>, Option<&str>) {
        let mut section = None;
        let mut subsection = None;
        for s in self.slides.iter().take(slide + 1) {
            if s.section.is_some() {
                section = s.section.as_deref();
                subsection = None;
            }
            if s.subsection.is_some() {
                subsection = s.subsection.as_deref();
            }
        }
        (section, subsection)
    }
}

/// A section (level 1) or subsection (level 2) heading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub slide: usize,
}

/// A slide layout: what every slide using it shares (background, static
/// elements such as a logo or a footer) and the placeholders a new slide
/// starts from.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    /// Default font, size and colors for the slides using this layout.
    #[serde(default, skip_serializing_if = "is_default")]
    pub theme: Theme,
    /// Drawn under the slide's own elements, not editable from a slide.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Element>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placeholders: Vec<Placeholder>,
    /// The theme this layout belongs to: what it leaves unset comes from
    /// there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The theme's shared elements (logo, footer) are not drawn on this
    /// layout's slides.
    #[serde(default, skip_serializing_if = "is_default")]
    pub hide_theme_elements: bool,
}

impl Layout {
    pub fn placeholder(&self, name: &str) -> Option<&Placeholder> {
        self.placeholders.iter().find(|p| p.name == name)
    }

    /// A new slide: one text box per placeholder, carrying the
    /// placeholder's name, position and default style.
    /// The preset dispositions offered when creating a layout, as
    /// `(id, label)`: PowerPoint's usual ones.
    pub const PRESETS: &'static [(&'static str, &'static str)] = &[
        ("title", "Title slide"),
        ("content", "Title and content"),
        ("section", "Section header"),
        ("two", "Two contents"),
        ("compare", "Comparison"),
        ("title-only", "Title only"),
        ("blank", "Blank"),
    ];

    /// A layout named `name` with the placeholders of preset `kind` (see
    /// [`Layout::PRESETS`]) on a `w` by `h` slide; unknown kinds are blank.
    pub fn preset(name: &str, kind: &str, w: f64, h: f64) -> Layout {
        let ph = |name: &str, x: f64, y: f64, pw: f64, ph: f64| Placeholder {
            name: name.into(),
            rect: Rect::new(x * w, y * h, pw * w, ph * h),
            ..Default::default()
        };
        let title = |y: f64, hh: f64| Placeholder { size: Some(40.0), bold: true, ..ph("title", 0.06, y, 0.88, hh) };
        let body = |name: &str, x: f64, y: f64, pw: f64, hh: f64| Placeholder {
            size: Some(24.0),
            bullets: true,
            ..ph(name, x, y, pw, hh)
        };
        let placeholders = match kind {
            "title" => vec![
                Placeholder { size: Some(48.0), bold: true, align: Align::Center, ..ph("title", 0.1, 0.28, 0.8, 0.2) },
                Placeholder { size: Some(26.0), align: Align::Center, ..ph("subtitle", 0.15, 0.52, 0.7, 0.14) },
            ],
            "content" => vec![title(0.05, 0.15), body("body1", 0.06, 0.24, 0.88, 0.68)],
            "section" => vec![
                Placeholder { size: Some(44.0), bold: true, ..ph("title", 0.08, 0.38, 0.84, 0.18) },
                Placeholder { size: Some(22.0), ..ph("subtitle", 0.08, 0.58, 0.84, 0.12) },
            ],
            "two" => {
                vec![title(0.05, 0.15), body("body1", 0.06, 0.24, 0.42, 0.68), body("body2", 0.52, 0.24, 0.42, 0.68)]
            }
            "compare" => vec![
                title(0.05, 0.15),
                Placeholder { size: Some(26.0), bold: true, ..ph("heading1", 0.06, 0.24, 0.42, 0.09) },
                body("body1", 0.06, 0.34, 0.42, 0.58),
                Placeholder { size: Some(26.0), bold: true, ..ph("heading2", 0.52, 0.24, 0.42, 0.09) },
                body("body2", 0.52, 0.34, 0.42, 0.58),
            ],
            "title-only" => vec![title(0.05, 0.15)],
            _ => vec![],
        };
        Layout { name: name.into(), placeholders, ..Default::default() }
    }

    pub fn new_slide(&self) -> Slide {
        Slide {
            layout: Some(self.name.clone()),
            elements: self.placeholders.iter().map(|p| Element::Text(p.text_box())).collect(),
            ..Default::default()
        }
    }
}

/// A named slot of a layout, e.g. `title` or `body`, with the style its
/// text starts with.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Placeholder {
    pub name: String,
    pub rect: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: Align,
    /// Paragraphs start bulleted (a body placeholder).
    #[serde(default, skip_serializing_if = "is_default")]
    pub bullets: bool,
    /// Where the text sits in the box, top to bottom.
    #[serde(default, skip_serializing_if = "is_default")]
    pub valign: VAlign,
    /// Text shown before the slide's own text in this placeholder, on
    /// every slide (variables such as `{{section}}` filled in); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// In a layout with a theme: the style properties (`size`, `bold`,
    /// `color`, `font`, `align`, `bullets`, `valign`) this placeholder sets itself;
    /// the others come from the theme's placeholder of the same kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub own: Vec<String>,
}

impl Placeholder {
    /// The empty text box a slide fills this placeholder with: the
    /// placeholder's style, no text (the editor shows a prompt in it).
    pub fn text_box(&self) -> TextBox {
        self.text_box_with(String::new())
    }

    /// The box with the placeholder's name as its text: how a layout shows it.
    pub fn sample_box(&self) -> TextBox {
        self.text_box_with(self.name.clone())
    }

    fn text_box_with(&self, text: String) -> TextBox {
        let run = Run {
            text,
            bold: self.bold,
            size: self.size,
            color: self.color,
            font: self.font.clone(),
            ..Default::default()
        };
        TextBox {
            rect: self.rect,
            valign: self.valign,
            paragraphs: vec![Paragraph {
                runs: vec![run],
                align: self.align,
                bullet: if self.bullets { Some(0) } else { None },
            }],
            placeholder: Some(self.name.clone()),
            ..Default::default()
        }
    }
}

/// One slide.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Slide {
    /// Optional name, shown in the editor and usable from macros.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// The [`Layout`] this slide uses, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    /// This slide starts a section of that name (PDF bookmarks, PowerPoint
    /// sections, the presenter view).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// This slide starts a subsection of that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subsection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    #[serde(default)]
    pub elements: Vec<Element>,
    /// Speaker notes, plain text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub transition: Transition,
    /// Transition duration in seconds.
    #[serde(default, skip_serializing_if = "is_default")]
    pub transition_duration: f64,
}

impl Slide {
    /// The number of "next" presses the slide consumes before moving on.
    pub fn last_step(&self) -> u32 {
        self.elements.iter().map(Element::last_step).max().unwrap_or(0)
    }

    /// The slide's title: its name, else the first line of its first
    /// non-empty text box.
    pub fn title(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        self.elements
            .iter()
            .find_map(|e| match e {
                Element::Text(t) => {
                    let text = t.text();
                    let line = text.lines().find(|l| !l.trim().is_empty())?.trim().to_string();
                    Some(line)
                }
                _ => None,
            })
            .unwrap_or_default()
    }
}

/// Anything placed on a slide, in drawing order (first is at the back).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Text(TextBox),
    Image(Image),
    /// A video or an animated GIF.
    Media(Media),
    /// A figure: Typst markup (CeTZ drawings and plots included).
    Figure(Figure),
    /// A table of text cells.
    Table(Table),
    Shape(Shape),
    Math(MathBlock),
    /// The deck's sections, with the slide's own section highlighted: a
    /// "plan" slide that repeats at each section start.
    Outline(OutlineBox),
    /// Elements moved, resized and animated together.
    Group(Group),
}

impl Element {
    pub fn rect(&self) -> &Rect {
        match self {
            Element::Text(t) => &t.rect,
            Element::Image(i) => &i.rect,
            Element::Media(m) => &m.rect,
            Element::Figure(f) => &f.rect,
            Element::Table(t) => &t.rect,
            Element::Shape(s) => &s.rect,
            Element::Math(m) => &m.rect,
            Element::Outline(o) => &o.rect,
            Element::Group(g) => &g.rect,
        }
    }

    /// The element's own name (may be empty).
    pub fn name(&self) -> &str {
        match self {
            Element::Text(t) => &t.name,
            Element::Image(i) => &i.name,
            Element::Media(m) => &m.name,
            Element::Figure(f) => &f.name,
            Element::Table(t) => &t.name,
            Element::Shape(s) => &s.name,
            Element::Math(m) => &m.name,
            Element::Outline(o) => &o.name,
            Element::Group(g) => &g.name,
        }
    }

    pub fn set_name(&mut self, name: &str) {
        let n = match self {
            Element::Text(t) => &mut t.name,
            Element::Image(i) => &mut i.name,
            Element::Media(m) => &mut m.name,
            Element::Figure(f) => &mut f.name,
            Element::Table(t) => &mut t.name,
            Element::Shape(s) => &mut s.name,
            Element::Math(m) => &mut m.name,
            Element::Outline(o) => &mut o.name,
            Element::Group(g) => &mut g.name,
        };
        *n = name.trim().to_string();
    }

    /// A label for lists such as the animation timeline: the name, else the
    /// kind and the start of the text.
    pub fn label(&self) -> String {
        if !self.name().is_empty() {
            return self.name().to_string();
        }
        let text = |t: String| {
            let t = t.split('\n').next().unwrap_or("").trim().to_string();
            if t.chars().count() > 24 {
                format!("{}…", t.chars().take(24).collect::<String>())
            } else {
                t
            }
        };
        match self {
            Element::Text(t) => format!("Text “{}”", text(t.text())),
            Element::Image(_) => "Picture".into(),
            Element::Media(_) => "Video".into(),
            Element::Figure(_) => "Figure".into(),
            Element::Table(_) => "Table".into(),
            Element::Shape(s) => match s.paragraphs.iter().map(Paragraph::text).collect::<String>() {
                t if !t.trim().is_empty() => format!("Shape “{}”", text(t)),
                _ => format!("{:?}", s.kind),
            },
            Element::Math(m) => format!("Formula {}", text(m.source.clone())),
            Element::Outline(_) => "Plan".into(),
            Element::Group(g) => format!("Group of {}", g.children.len()),
        }
    }

    pub fn rect_mut(&mut self) -> &mut Rect {
        match self {
            Element::Text(t) => &mut t.rect,
            Element::Image(i) => &mut i.rect,
            Element::Media(m) => &mut m.rect,
            Element::Figure(f) => &mut f.rect,
            Element::Table(t) => &mut t.rect,
            Element::Shape(s) => &mut s.rect,
            Element::Math(m) => &mut m.rect,
            Element::Outline(o) => &mut o.rect,
            Element::Group(g) => &mut g.rect,
        }
    }

    /// Moves and resizes the element to `rect`; a group carries its
    /// children along, scaled.
    pub fn set_rect(&mut self, rect: Rect) {
        if let Element::Group(g) = self {
            g.set_rect(rect);
        } else {
            *self.rect_mut() = rect;
        }
    }

    /// Mirrors the element in place, about the vertical axis through its
    /// center (`horizontal`: left and right swap) or the horizontal one.
    /// Lines and arrows change direction, images are drawn mirrored, a
    /// group mirrors its children's positions too; text stays readable.
    pub fn mirror(&mut self, horizontal: bool) {
        match self {
            Element::Shape(s) if s.kind == ShapeKind::Path => {
                for n in &mut s.path {
                    if horizontal {
                        n.x = 1.0 - n.x;
                        n.handle_in = n.handle_in.map(|(x, y)| (-x, y));
                        n.handle_out = n.handle_out.map(|(x, y)| (-x, y));
                    } else {
                        n.y = 1.0 - n.y;
                        n.handle_in = n.handle_in.map(|(x, y)| (x, -y));
                        n.handle_out = n.handle_out.map(|(x, y)| (x, -y));
                    }
                }
            }
            Element::Shape(s) if matches!(s.kind, ShapeKind::Line | ShapeKind::Arrow) => {
                s.flip = !s.flip;
                if horizontal {
                    s.reverse = !s.reverse;
                }
            }
            Element::Image(i) => {
                if horizontal {
                    i.mirror_h = !i.mirror_h;
                } else {
                    i.mirror_v = !i.mirror_v;
                }
            }
            Element::Group(g) => {
                let b = g.rect;
                for c in &mut g.children {
                    let mut r = *c.rect();
                    if horizontal {
                        r.x = b.x + (b.x + b.w) - (r.x + r.w);
                    } else {
                        r.y = b.y + (b.y + b.h) - (r.y + r.h);
                    }
                    c.set_rect(r);
                    c.mirror(horizontal);
                }
            }
            _ => {}
        }
    }

    /// The build steps of the element, empty when it is static.
    pub fn steps(&self) -> &[Step] {
        match self {
            Element::Text(t) => &t.steps,
            Element::Image(i) => &i.steps,
            Element::Media(m) => &m.steps,
            Element::Figure(f) => &f.steps,
            Element::Table(t) => &t.steps,
            Element::Shape(s) => &s.steps,
            Element::Math(m) => &m.steps,
            Element::Outline(o) => &o.steps,
            Element::Group(g) => &g.steps,
        }
    }

    pub fn steps_mut(&mut self) -> &mut Vec<Step> {
        match self {
            Element::Text(t) => &mut t.steps,
            Element::Image(i) => &mut i.steps,
            Element::Media(m) => &mut m.steps,
            Element::Figure(f) => &mut f.steps,
            Element::Table(t) => &mut t.steps,
            Element::Shape(s) => &mut s.steps,
            Element::Math(m) => &mut m.steps,
            Element::Outline(o) => &mut o.steps,
            Element::Group(g) => &mut g.steps,
        }
    }

    /// The element as drawn at build step `step`: its base rect with the
    /// overrides of every step up to `step` applied in order.
    pub fn state_at(&self, step: u32) -> State {
        let mut state = State { rect: *self.rect(), ..Default::default() };
        for s in self.steps().iter().filter(|s| s.at <= step) {
            state.apply(&s.set);
        }
        state
    }

    /// The last step that changes this element (0 when static).
    pub fn last_step(&self) -> u32 {
        self.steps().iter().map(|s| s.at).max().unwrap_or(0)
    }
}

/// Elements moved, resized and animated together. Children keep slide
/// coordinates; `rect` is their bounding box.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Group {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    pub children: Vec<Element>,
}

impl Group {
    /// A group of `children`, with their bounding box.
    pub fn of(children: Vec<Element>) -> Self {
        Self { rect: bounding(&children), name: String::new(), steps: vec![], children }
    }

    /// Moves and scales the children so that the group fills `rect`.
    pub fn set_rect(&mut self, rect: Rect) {
        let old = self.rect;
        let sx = if old.w > 0.0 { rect.w / old.w } else { 1.0 };
        let sy = if old.h > 0.0 { rect.h / old.h } else { 1.0 };
        for c in &mut self.children {
            let r = *c.rect();
            c.set_rect(Rect {
                x: rect.x + (r.x - old.x) * sx,
                y: rect.y + (r.y - old.y) * sy,
                w: r.w * sx,
                h: r.h * sy,
            });
        }
        self.rect = rect;
    }
}

/// The bounding box of elements (zero when there are none).
pub fn bounding(elements: &[Element]) -> Rect {
    let mut it = elements.iter().map(|e| *e.rect());
    let Some(first) = it.next() else { return Rect::default() };
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x + first.w, first.y + first.h);
    for r in it {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w);
        y1 = y1.max(r.y + r.h);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Replaces the elements at `indices` (at least two) by one group, placed
/// where the first of them was. Returns the group's index.
pub fn group_elements(elements: &mut Vec<Element>, indices: &[usize]) -> Option<usize> {
    let mut idx: Vec<usize> = indices.iter().copied().filter(|&i| i < elements.len()).collect();
    idx.sort_unstable();
    idx.dedup();
    if idx.len() < 2 {
        return None;
    }
    let at = idx[0];
    let mut children = vec![];
    for &i in idx.iter().rev() {
        children.push(elements.remove(i));
    }
    children.reverse();
    elements.insert(at, Element::Group(Group::of(children)));
    Some(at)
}

/// Replaces the group at `index` by its children, in place. Returns how
/// many elements took its place.
pub fn ungroup_element(elements: &mut Vec<Element>, index: usize) -> Option<usize> {
    match elements.get(index) {
        Some(Element::Group(_)) => {}
        _ => return None,
    }
    let Element::Group(g) = elements.remove(index) else { unreachable!() };
    let n = g.children.len();
    for (k, c) in g.children.into_iter().enumerate() {
        elements.insert(index + k, c);
    }
    Some(n)
}

/// The deck's outline as a list: every section, the one the slide belongs
/// to highlighted, and the subsections of that section under it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OutlineBox {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    /// Font size in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// Color of the other sections (default gray).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Color of the current section (default black, bold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Color>,
    /// Show the subsections of the current section.
    #[serde(default = "yes", skip_serializing_if = "Clone::clone")]
    pub subsections: bool,
}

fn yes() -> bool {
    true
}

impl Deck {
    /// The paragraphs an outline box shows on slide `index`: one per
    /// section, numbered, the current one highlighted; under it, its
    /// subsections when asked. Without a slide index, nothing is current.
    pub fn outline_paragraphs(&self, index: Option<usize>, o: &OutlineBox) -> Vec<Paragraph> {
        let (current, current_sub) = index.map(|i| self.position(i)).unwrap_or((None, None));
        let theme = index.and_then(|i| self.slides.get(i)).map(|s| self.theme_of(s)).unwrap_or_default();
        let muted = o.color.unwrap_or(Color::rgb(0x88, 0x88, 0x88));
        let strong = o.highlight.or(theme.accent).or(theme.color).unwrap_or(Color::BLACK);
        let mut out = vec![];
        let mut number = 0;
        for (i, s) in self.slides.iter().enumerate() {
            let Some(name) = &s.section else { continue };
            number += 1;
            let is_current = current == Some(name.as_str());
            out.push(Paragraph {
                runs: vec![Run {
                    text: format!("{number}. {name}"),
                    bold: is_current,
                    size: o.size,
                    color: Some(if is_current { strong } else { muted }),
                    ..Default::default()
                }],
                ..Default::default()
            });
            if is_current && o.subsections {
                // The subsections up to the next section.
                for t in self.slides[i..].iter().take_while(|t| std::ptr::eq(*t, s) || t.section.is_none()) {
                    if let Some(sub) = &t.subsection {
                        let is_sub = current_sub == Some(sub.as_str());
                        out.push(Paragraph {
                            runs: vec![Run {
                                text: sub.clone(),
                                bold: is_sub,
                                size: o.size.map(|v| v * 0.85),
                                color: Some(if is_sub { strong } else { muted }),
                                ..Default::default()
                            }],
                            bullet: Some(1),
                            ..Default::default()
                        });
                    }
                }
            }
        }
        out
    }
}

/// A displayed formula: Typst math source, rendered on display. The deck's
/// [`Deck::math_preamble`] and its `typst/*.typ` files are in scope.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MathBlock {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub source: String,
    /// Font size in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
}

/// A change of an element at a build step. Steps are reached by "next";
/// step 0 is the slide as first shown. Playing a step interpolates from the
/// previous state over `duration`, after `delay` (both in seconds).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Step {
    pub at: u32,
    #[serde(default)]
    pub set: Overrides,
    #[serde(default, skip_serializing_if = "is_default")]
    pub duration: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub delay: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub easing: Easing,
    /// When it starts within its click: with the effect before it (the
    /// first of a click starts on the click), or after that one ends.
    #[serde(default, skip_serializing_if = "is_default")]
    pub trigger: Trigger,
    /// Its place among the effects of its click (ties: element order).
    #[serde(default, skip_serializing_if = "is_default")]
    pub seq: u32,
}

/// When a step starts within its click (PowerPoint's "with previous" and
/// "after previous"; "on click" is being the first of a click).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// Together with the effect before it.
    #[default]
    With,
    /// When the effect before it has ended.
    After,
}

/// One effect of a click, in playing order (see [`Slide::click_order`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timed {
    pub element: usize,
    /// Index in the element's steps.
    pub step: usize,
    /// Seconds after the click at which it starts (its delay included).
    pub start: f64,
    pub duration: f64,
}

impl Slide {
    /// The effects of click `at` in playing order (`seq`, then element and
    /// step order), each with its start time: "with" effects start with the
    /// one before, "after" effects when it has ended, plus their own delay.
    /// Whether the slide's first click starts by itself when the slide is
    /// shown: its first effect is "after previous" (PowerPoint's effects
    /// that start with or after the slide).
    pub fn starts_by_itself(&self) -> bool {
        self.click_order(1).first().is_some_and(|t| self.elements[t.element].steps()[t.step].trigger == Trigger::After)
    }

    pub fn click_order(&self, at: u32) -> Vec<Timed> {
        let mut entries: Vec<(u32, usize, usize)> = vec![];
        for (e, element) in self.elements.iter().enumerate() {
            for (k, s) in element.steps().iter().enumerate() {
                if s.at == at {
                    entries.push((s.seq, e, k));
                }
            }
        }
        entries.sort();
        let (mut base, mut end) = (0.0_f64, 0.0_f64);
        let mut out = vec![];
        for (n, (_, e, k)) in entries.into_iter().enumerate() {
            let s = &self.elements[e].steps()[k];
            // The first effect of a click starts on the click (or with the
            // slide when it starts by itself): its trigger sets no wait.
            if n > 0 && s.trigger == Trigger::After {
                base = end;
            }
            let start = base + s.delay.max(0.0);
            end = end.max(start + s.duration.max(0.0));
            out.push(Timed { element: e, step: k, start, duration: s.duration.max(0.0) });
        }
        out
    }
}

impl Step {
    /// A fade-in at `at`.
    pub fn appear(at: u32) -> Self {
        Self { at, set: Overrides { opacity: Some(1.0), ..Default::default() }, duration: 0.3, ..Default::default() }
    }

    /// A fade-out at `at`.
    pub fn disappear(at: u32) -> Self {
        Self { at, set: Overrides { opacity: Some(0.0), ..Default::default() }, duration: 0.3, ..Default::default() }
    }
}

/// What a step changes. Unset fields keep the previous state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Overrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
    /// Rotation around the center, in degrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// 0 is invisible, 1 fully drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    #[default]
    EaseInOut,
    Linear,
    EaseIn,
    EaseOut,
}

/// An element's drawn state at a step.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub rect: Rect,
    pub rotation: f64,
    pub opacity: f64,
    pub fill: Option<Color>,
}

impl Default for State {
    fn default() -> Self {
        Self { rect: Rect::default(), rotation: 0.0, opacity: 1.0, fill: None }
    }
}

impl State {
    pub fn apply(&mut self, o: &Overrides) {
        if let Some(x) = o.x {
            self.rect.x = x;
        }
        if let Some(y) = o.y {
            self.rect.y = y;
        }
        if let Some(w) = o.w {
            self.rect.w = w;
        }
        if let Some(h) = o.h {
            self.rect.h = h;
        }
        if let Some(r) = o.rotation {
            self.rotation = r;
        }
        if let Some(a) = o.opacity {
            self.opacity = a;
        }
        if o.fill.is_some() {
            self.fill = o.fill;
        }
    }
}

/// Transition from the previous slide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    #[default]
    None,
    Fade,
    Push,
}

/// Position and size, in points.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
}

/// An sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses `RRGGBB` or `#RRGGBB`.
    pub fn from_hex(s: &str) -> Result<Self, ColorError> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() != 6 {
            return Err(ColorError(s.to_string()));
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ColorError(s.to_string()));
        Ok(Self { r: byte(0)?, g: byte(2)?, b: byte(4)? })
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid color {0:?}, expected #RRGGBB")]
pub struct ColorError(pub String);

impl TryFrom<String> for Color {
    type Error = ColorError;
    fn try_from(s: String) -> Result<Self, ColorError> {
        Color::from_hex(&s)
    }
}

impl From<Color> for String {
    fn from(c: Color) -> String {
        c.to_hex()
    }
}

/// A box of formatted text.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TextBox {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    /// The layout placeholder this box fills, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub paragraphs: Vec<Paragraph>,
    /// Where the text sits in the box, top to bottom.
    #[serde(default, skip_serializing_if = "is_default")]
    pub valign: VAlign,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
}

impl TextBox {
    /// A text box with a single plain paragraph.
    pub fn plain(rect: Rect, text: impl Into<String>) -> Self {
        Self { rect, paragraphs: vec![Paragraph::plain(text)], ..Default::default() }
    }

    /// The text of all paragraphs, joined by newlines.
    pub fn text(&self) -> String {
        self.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Where the text sits in its box, top to bottom (PowerPoint's anchor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

impl VAlign {
    /// The share of the free height above the text.
    pub fn fraction(self) -> f64 {
        match self {
            VAlign::Top => 0.0,
            VAlign::Middle => 0.5,
            VAlign::Bottom => 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Paragraph {
    #[serde(default)]
    pub runs: Vec<Run>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: Align,
    /// Bullet list level: `None` for no bullet, `Some(0)` for a first-level bullet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bullet: Option<u8>,
}

impl Paragraph {
    pub fn plain(text: impl Into<String>) -> Self {
        Self { runs: vec![Run::plain(text)], ..Default::default() }
    }

    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// A run of text sharing one style. Unset fields inherit the defaults of
/// the renderer (black, 18 pt, regular).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub italic: bool,
    /// Font size in points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// When set, `text` is Typst math source, rendered inline.
    #[serde(default, skip_serializing_if = "is_default")]
    pub math: bool,
}

impl Run {
    pub fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Default::default() }
    }
}

/// A raster image stored in the deck's `assets/` folder.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Image {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    /// Path relative to the deck folder, e.g. `assets/image1.png`.
    pub src: String,
    /// Fractions of the source cut off each edge before it fills `rect`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub crop: Crop,
    /// The shape the image is clipped to, within `rect`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub mask: Mask,
    /// Drawn mirrored left to right, top to bottom.
    #[serde(default, skip_serializing_if = "is_default")]
    pub mirror_h: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub mirror_v: bool,
}

/// A video (`mp4`, `webm`…) or an animated GIF in the deck's `assets/`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Media {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    /// Path relative to the deck folder, e.g. `assets/clip.mp4`.
    pub src: String,
    /// A still (the first frame) shown before playback, in thumbnails,
    /// PDF and PowerPoint exports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster: Option<String>,
    /// When playback starts.
    #[serde(default, skip_serializing_if = "is_default")]
    pub play: Play,
    /// Loop, or play once and stop on the last frame.
    #[serde(default, skip_serializing_if = "is_default", rename = "loop")]
    pub looping: bool,
    /// Length in seconds, when known (GIFs: the sum of frame delays).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Play the sound; off by default.
    #[serde(default, skip_serializing_if = "is_default")]
    pub sound: bool,
}

impl Media {
    /// Whether the file is an animated GIF (played as an image).
    pub fn is_gif(&self) -> bool {
        self.src.to_ascii_lowercase().ends_with(".gif")
    }
}

/// A figure typeset by Typst: any markup, with the bundled CeTZ packages
/// for drawings and plots (`@preview/cetz`, `@preview/cetz-plot`). The
/// page is as wide as the box; the result is scaled to fit the box.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Figure {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    pub source: String,
}

/// A table: rows of cells holding paragraphs, in a grid that fills the
/// box. Rows share the height equally; columns follow `columns`
/// (relative widths, equal when empty).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Table {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    pub rows: Vec<Vec<Cell>>,
    /// Relative column widths; empty for equal columns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<f64>,
    /// The first row is a header: bold, on the theme's accent.
    #[serde(default, skip_serializing_if = "is_default")]
    pub header: bool,
    /// The grid lines; none for a table without rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Text size of the cells that set none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
}

/// One cell of a table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paragraphs: Vec<Paragraph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    /// Merged cells: this cell spans `colspan` columns and `rowspan` rows
    /// (1 when unmerged). The cells it covers stay in the grid, empty and
    /// not drawn, as in PowerPoint.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub colspan: usize,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub rowspan: usize,
}

fn one() -> usize {
    1
}

fn is_one(n: &usize) -> bool {
    *n <= 1
}

impl Default for Cell {
    fn default() -> Self {
        Self { paragraphs: vec![], fill: None, colspan: 1, rowspan: 1 }
    }
}

impl Cell {
    pub fn plain(text: &str) -> Self {
        Self { paragraphs: text.split('\n').map(Paragraph::plain).collect(), ..Default::default() }
    }

    pub fn text(&self) -> String {
        self.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n")
    }
}

impl Table {
    /// A table of `rows` by `columns` empty cells with a header row and thin rules.
    pub fn new(rect: Rect, rows: usize, columns: usize) -> Self {
        Self {
            rect,
            rows: vec![vec![Cell::default(); columns.max(1)]; rows.max(1)],
            header: true,
            stroke: Some(Stroke { color: Color::rgb(0x99, 0x99, 0x99), width: 0.75 }),
            ..Default::default()
        }
    }

    /// The number of columns: the longest row.
    pub fn column_count(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }

    /// The box of the cell at `row`, `column` when the table fills `rect`.
    pub fn cell_rect(&self, rect: &Rect, row: usize, column: usize) -> Rect {
        let n = self.column_count().max(1);
        let weight = |c: usize| self.columns.get(c).copied().filter(|w| *w > 0.0).unwrap_or(1.0);
        let total: f64 = (0..n).map(weight).sum();
        let before: f64 = (0..column.min(n)).map(weight).sum();
        // A row is as tall as its tallest cell: one share of the height per line.
        let lines = |r: usize| {
            self.rows
                .get(r)
                .map(|cells| cells.iter().map(|c| c.paragraphs.len()).max().unwrap_or(1))
                .unwrap_or(1)
                .max(1) as f64
        };
        let all: f64 = (0..self.rows.len()).map(lines).sum::<f64>().max(1.0);
        let above: f64 = (0..row.min(self.rows.len())).map(lines).sum();
        Rect::new(
            rect.x + rect.w * before / total,
            rect.y + rect.h * above / all,
            rect.w * weight(column) / total,
            rect.h * lines(row) / all,
        )
    }

    /// The paragraphs of a cell as they are drawn: the header row bold,
    /// white unless its runs set a color. Shared by the renderer and by
    /// the caret's layout, so that they agree.
    pub fn display_paragraphs(&self, row: usize, column: usize) -> Vec<Paragraph> {
        let Some(cell) = self.rows.get(row).and_then(|r| r.get(column)) else { return vec![] };
        if !(self.header && row == 0) {
            return cell.paragraphs.clone();
        }
        cell.paragraphs
            .iter()
            .map(|p| Paragraph {
                runs: p
                    .runs
                    .iter()
                    .map(|run| Run { bold: true, color: run.color.or(Some(Color::WHITE)), ..run.clone() })
                    .collect(),
                ..p.clone()
            })
            .collect()
    }

    /// The cell drawn at `row`, `column`: the merged cell covering it, or
    /// itself.
    pub fn owner(&self, row: usize, column: usize) -> (usize, usize) {
        for (i, cells) in self.rows.iter().enumerate().take(row + 1) {
            for (j, c) in cells.iter().enumerate().take(column + 1) {
                if (i, j) != (row, column) && i + c.rowspan.max(1) > row && j + c.colspan.max(1) > column {
                    return (i, j);
                }
            }
        }
        (row, column)
    }

    /// Whether the cell at `row`, `column` is covered by a merged cell.
    pub fn covered(&self, row: usize, column: usize) -> bool {
        self.owner(row, column) != (row, column)
    }

    /// The box of the cell at `row`, `column` with its merged neighbours.
    pub fn span_rect(&self, rect: &Rect, row: usize, column: usize) -> Rect {
        let first = self.cell_rect(rect, row, column);
        let Some(c) = self.rows.get(row).and_then(|r| r.get(column)) else { return first };
        let last_row = (row + c.rowspan.max(1) - 1).min(self.rows.len().saturating_sub(1));
        let last_column = (column + c.colspan.max(1) - 1).min(self.column_count().saturating_sub(1));
        let last = self.cell_rect(rect, last_row, last_column);
        Rect::new(first.x, first.y, last.x + last.w - first.x, last.y + last.h - first.y)
    }

    /// Merges the cells from `(r0, c0)` to `(r1, c1)` (inclusive, any
    /// order) into the top-left one, its text followed by the others';
    /// merged cells inside are split first.
    pub fn merge(&mut self, (r0, c0): (usize, usize), (r1, c1): (usize, usize)) {
        self.square();
        let (top, bottom) = (r0.min(r1), r0.max(r1).min(self.rows.len().saturating_sub(1)));
        let (left, right) = (c0.min(c1), c0.max(c1).min(self.column_count().saturating_sub(1)));
        if self.rows.is_empty() || (top, left) == (bottom, right) {
            return;
        }
        let mut gathered = vec![];
        for i in top..=bottom {
            for j in left..=right {
                let c = &mut self.rows[i][j];
                c.colspan = 1;
                c.rowspan = 1;
                if (i, j) != (top, left) && !c.text().trim().is_empty() {
                    gathered.append(&mut c.paragraphs);
                }
                if (i, j) != (top, left) {
                    c.paragraphs.clear();
                }
            }
        }
        let owner = &mut self.rows[top][left];
        owner.paragraphs.append(&mut gathered);
        owner.colspan = right - left + 1;
        owner.rowspan = bottom - top + 1;
    }

    /// Splits the merged cell covering `row`, `column` back into cells.
    pub fn split(&mut self, row: usize, column: usize) {
        let (i, j) = self.owner(row, column);
        if let Some(c) = self.rows.get_mut(i).and_then(|r| r.get_mut(j)) {
            c.colspan = 1;
            c.rowspan = 1;
        }
    }

    /// Splits the merged cells spanning row `row` (`Some`) or column
    /// `column`: before a row or column is inserted or removed there.
    pub fn split_through(&mut self, row: Option<usize>, column: Option<usize>) {
        for i in 0..self.rows.len() {
            for j in 0..self.rows[i].len() {
                let c = &self.rows[i][j];
                let through = row.is_some_and(|r| i <= r && r < i + c.rowspan.max(1) && c.rowspan > 1)
                    || column.is_some_and(|k| j <= k && k < j + c.colspan.max(1) && c.colspan > 1);
                if through {
                    self.split(i, j);
                }
            }
        }
    }

    /// Makes every row as long as the longest.
    pub fn square(&mut self) {
        let n = self.column_count();
        for row in &mut self.rows {
            row.resize(n, Cell::default());
        }
    }
}

/// When a media element starts playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Play {
    /// As soon as the element is shown (slide entered, or its appear step).
    #[default]
    OnAppear,
    /// When the presenter clicks the play button drawn over it.
    Button,
}

/// A rectangular crop, as fractions (0 to 1) of the source image removed
/// from each edge, like PowerPoint's `srcRect`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Crop {
    #[serde(default, skip_serializing_if = "is_default")]
    pub left: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub top: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub right: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub bottom: f64,
}

impl Crop {
    pub fn is_none(&self) -> bool {
        *self == Crop::default()
    }

    /// Clamps each edge to 0..0.95 and keeps at least 5 % of each axis.
    pub fn clamped(self) -> Self {
        let c = |v: f64| v.clamp(0.0, 0.95);
        let (mut l, mut r, mut t, mut b) = (c(self.left), c(self.right), c(self.top), c(self.bottom));
        if l + r > 0.95 {
            let f = 0.95 / (l + r);
            l *= f;
            r *= f;
        }
        if t + b > 0.95 {
            let f = 0.95 / (t + b);
            t *= f;
            b *= f;
        }
        Crop { left: l, top: t, right: r, bottom: b }
    }
}

/// The shape an image is clipped to.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Mask {
    #[default]
    Rectangle,
    Ellipse,
    /// Corner radius as a fraction (0 to 0.5) of the shorter side.
    RoundedRectangle {
        #[serde(default = "default_radius")]
        radius: f64,
    },
}

fn default_radius() -> f64 {
    0.15
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    #[default]
    Rectangle,
    Ellipse,
    /// A straight line from the top-left to the bottom-right of the rect
    /// (bottom-left to top-right when the shape is flipped).
    Line,
    /// A line ending in an arrowhead.
    Arrow,
    /// A path of straight and Bézier segments through the shape's
    /// `path` nodes; filled when closed.
    Path,
}

/// A node of a path: a point and the Bézier handles around it (offsets
/// from the point; none for a corner), all in fractions of the shape's
/// box, so that the path follows the box when it moves or resizes.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PathNode {
    pub x: f64,
    pub y: f64,
    /// The handle the curve arrives along, from the previous node.
    #[serde(default, rename = "in", skip_serializing_if = "Option::is_none")]
    pub handle_in: Option<(f64, f64)>,
    /// The handle the curve leaves along, to the next node.
    #[serde(default, rename = "out", skip_serializing_if = "Option::is_none")]
    pub handle_out: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    /// Width in points.
    pub width: f64,
}

/// A geometric shape, optionally with text inside.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Shape {
    pub rect: Rect,
    /// A name for the element (the animation timeline shows it, PowerPoint
    /// keeps it in its selection pane); empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub kind: ShapeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paragraphs: Vec<Paragraph>,
    /// Where the text sits in the shape, top to bottom.
    #[serde(default, skip_serializing_if = "is_default")]
    pub valign: VAlign,
    /// Lines and arrows: run from the bottom-left to the top-right.
    #[serde(default, skip_serializing_if = "is_default")]
    pub flip: bool,
    /// Arrows: the head is at the start (the left end) instead of the end.
    #[serde(default, skip_serializing_if = "is_default")]
    pub reverse: bool,
    /// Paths: the nodes, in fractions of `rect`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<PathNode>,
    /// Paths: the last node joins the first, and the shape is filled.
    #[serde(default, skip_serializing_if = "is_default")]
    pub closed: bool,
}

impl Shape {
    /// A path shape through `nodes` given in slide points (handles as
    /// offsets in points): its box is the bounding box of the nodes and
    /// handles, the nodes stored in fractions of it.
    pub fn path_from_points(nodes: &[PathNode], closed: bool) -> Shape {
        let mut shape = Shape { kind: ShapeKind::Path, closed, ..Default::default() };
        shape.set_path_points(nodes, closed);
        shape
    }

    /// Replaces the path with `nodes` in slide points; the box becomes
    /// their bounding box.
    pub fn set_path_points(&mut self, nodes: &[PathNode], closed: bool) {
        let mut pts: Vec<(f64, f64)> = vec![];
        for n in nodes {
            pts.push((n.x, n.y));
            for h in [n.handle_in, n.handle_out].into_iter().flatten() {
                pts.push((n.x + h.0, n.y + h.1));
            }
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (x, y) in &pts {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        if pts.is_empty() {
            (x0, y0, x1, y1) = (0.0, 0.0, 0.0, 0.0);
        }
        let (w, h) = ((x1 - x0).max(1e-6), (y1 - y0).max(1e-6));
        self.rect = Rect::new(x0, y0, x1 - x0, y1 - y0);
        self.path = nodes
            .iter()
            .map(|n| PathNode {
                x: (n.x - x0) / w,
                y: (n.y - y0) / h,
                handle_in: n.handle_in.map(|(dx, dy)| (dx / w, dy / h)),
                handle_out: n.handle_out.map(|(dx, dy)| (dx / w, dy / h)),
            })
            .collect();
        self.closed = closed;
    }

    /// The path's nodes in slide points, for a box `r` (handles as offsets).
    pub fn path_points(&self, r: &Rect) -> Vec<PathNode> {
        self.path
            .iter()
            .map(|n| PathNode {
                x: r.x + n.x * r.w,
                y: r.y + n.y * r.h,
                handle_in: n.handle_in.map(|(dx, dy)| (dx * r.w, dy * r.h)),
                handle_out: n.handle_out.map(|(dx, dy)| (dx * r.w, dy * r.h)),
            })
            .collect()
    }
}

pub(crate) fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn merged_cells() {
        let mut t = Table::new(Rect::new(0.0, 0.0, 300.0, 90.0), 3, 3);
        t.rows[0][1] = Cell::plain("b");
        t.rows[1][0] = Cell::plain("d");
        t.merge((1, 1), (0, 0));
        assert_eq!((t.rows[0][0].colspan, t.rows[0][0].rowspan), (2, 2));
        assert_eq!(t.rows[0][0].text(), "b\nd", "the texts are gathered in the top-left cell");
        assert_eq!(t.owner(1, 1), (0, 0));
        assert!(t.covered(0, 1) && !t.covered(0, 2) && !t.covered(2, 0));
        let (a, b) = (t.cell_rect(&t.rect, 0, 0), t.cell_rect(&t.rect, 1, 1));
        assert_eq!(t.span_rect(&t.rect, 0, 0), Rect::new(0.0, 0.0, 200.0, b.y + b.h - a.y));
        t.split_through(None, Some(2));
        assert_eq!(t.rows[0][0].colspan, 2, "a column outside the merge leaves it");
        t.split_through(Some(1), None);
        assert!(!t.covered(1, 1), "a row through the merge splits it");
        t.merge((2, 1), (2, 2));
        t.split(2, 2);
        assert_eq!(t.rows[2][1].colspan, 1, "splitting a covered cell splits its owner");
    }

    use super::*;

    #[test]
    fn color_hex_round_trip() {
        let c = Color::from_hex("#1A2b3C").unwrap();
        assert_eq!(c, Color::rgb(0x1A, 0x2B, 0x3C));
        assert_eq!(c.to_hex(), "#1A2B3C");
        assert!(Color::from_hex("12345").is_err());
        assert!(Color::from_hex("#GGGGGG").is_err());
    }

    #[test]
    fn json_round_trip_keeps_the_deck() {
        let mut deck = Deck::new("Test");
        deck.slides.push(Slide {
            name: "intro".into(),
            background: Some(Color::WHITE),
            elements: vec![
                Element::Text(TextBox::plain(Rect::new(10.0, 20.0, 300.0, 50.0), "Hello")),
                Element::Shape(Shape {
                    rect: Rect::new(0.0, 0.0, 10.0, 10.0),
                    kind: ShapeKind::Ellipse,
                    fill: Some(Color::rgb(1, 2, 3)),
                    ..Default::default()
                }),
            ],
            notes: "say hi".into(),
            ..Default::default()
        });
        let json = serde_json::to_string_pretty(&deck).unwrap();
        let back: Deck = serde_json::from_str(&json).unwrap();
        assert_eq!(back, deck);
        assert!(json.contains("\"version\": 4"));
        assert!(json.contains("\"type\": \"text\""));
    }

    #[test]
    fn crop_and_mask_round_trip() {
        let img = Image {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            src: "assets/a.png".into(),
            crop: Crop { left: 0.1, top: 0.0, right: 0.2, bottom: 0.05 },
            mask: Mask::RoundedRectangle { radius: 0.2 },
            ..Default::default()
        };
        let json = serde_json::to_string(&Element::Image(img.clone())).unwrap();
        assert!(json.contains("\"crop\":{\"left\":0.1,\"right\":0.2,\"bottom\":0.05}"), "{json}");
        assert!(json.contains("\"mask\":{\"kind\":\"rounded_rectangle\",\"radius\":0.2}"), "{json}");
        assert_eq!(serde_json::from_str::<Element>(&json).unwrap(), Element::Image(img));
        let plain = Image { rect: Rect::default(), src: "x".into(), ..Default::default() };
        let json = serde_json::to_string(&plain).unwrap();
        assert!(!json.contains("crop") && !json.contains("mask"), "defaults are omitted: {json}");
        let c = Crop { left: 0.8, top: 2.0, right: 0.8, bottom: -1.0 }.clamped();
        assert!((c.left + c.right - 0.95).abs() < 1e-9 && c.top <= 0.95 && c.bottom == 0.0);
    }

    #[test]
    fn layouts_make_slides() {
        let mut deck = Deck::new("t");
        deck.layouts.push(Layout {
            name: "Title".into(),
            background: Some(Color::WHITE),
            theme: Theme::default(),
            elements: vec![Element::Shape(Shape {
                rect: Rect::new(0.0, 500.0, 960.0, 40.0),
                fill: Some(Color::BLACK),
                ..Default::default()
            })],
            placeholders: vec![
                Placeholder {
                    name: "title".into(),
                    rect: Rect::new(50.0, 100.0, 860.0, 120.0),
                    size: Some(44.0),
                    bold: true,
                    align: Align::Center,
                    ..Default::default()
                },
                Placeholder {
                    name: "body".into(),
                    rect: Rect::new(50.0, 250.0, 860.0, 200.0),
                    bullets: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        let slide = deck.layouts[0].new_slide();
        assert_eq!(slide.layout.as_deref(), Some("Title"));
        assert_eq!(slide.elements.len(), 2);
        let Element::Text(t) = &slide.elements[0] else { panic!() };
        assert_eq!(t.placeholder.as_deref(), Some("title"));
        assert_eq!(t.rect, Rect::new(50.0, 100.0, 860.0, 120.0));
        assert!(t.paragraphs[0].runs[0].bold);
        assert_eq!(t.paragraphs[0].align, Align::Center);
        let Element::Text(b) = &slide.elements[1] else { panic!() };
        assert_eq!(b.paragraphs[0].bullet, Some(0));
        deck.slides.push(slide);
        assert_eq!(deck.layout_of(&deck.slides[0]).map(|l| l.name), Some("Title".to_string()));
        let json = serde_json::to_string(&deck).unwrap();
        assert!(json.contains("\"layouts\""));
        assert_eq!(serde_json::from_str::<Deck>(&json).unwrap(), deck);
    }

    #[test]
    fn themes_fall_back_from_layout_to_deck() {
        let mut deck = Deck::new("t");
        deck.theme = Theme {
            font: Some("Libertinus Serif".into()),
            size: Some(20.0),
            color: Some(Color::BLACK),
            ..Default::default()
        };
        deck.layouts.push(Layout {
            name: "L".into(),
            theme: Theme { color: Some(Color::rgb(1, 2, 3)), accent: Some(Color::WHITE), ..Default::default() },
            ..Default::default()
        });
        deck.slides.push(Slide { layout: Some("L".into()), ..Default::default() });
        deck.slides.push(Slide::default());
        let t = deck.theme_of(&deck.slides[0]);
        assert_eq!(
            (t.font.as_deref(), t.size, t.color, t.accent),
            (Some("Libertinus Serif"), Some(20.0), Some(Color::rgb(1, 2, 3)), Some(Color::WHITE))
        );
        assert_eq!(deck.theme_of(&deck.slides[1]).color, Some(Color::BLACK));
        let json = serde_json::to_string(&deck).unwrap();
        assert!(json.contains("\"theme\""));
        assert_eq!(serde_json::from_str::<Deck>(&json).unwrap(), deck);
        assert!(!serde_json::to_string(&Deck::new("x")).unwrap().contains("theme"), "the default theme is omitted");
    }

    #[test]
    fn media_round_trips_with_defaults_omitted() {
        let m = Media { rect: Rect::new(1.0, 2.0, 3.0, 4.0), src: "assets/clip.mp4".into(), ..Default::default() };
        let json = serde_json::to_string(&Element::Media(m.clone())).unwrap();
        assert!(!json.contains("play") && !json.contains("loop"), "{json}");
        assert_eq!(serde_json::from_str::<Element>(&json).unwrap(), Element::Media(m));
        let gif: Media = serde_json::from_str(
            r#"{"rect":{"x":0,"y":0,"w":1,"h":1},"src":"assets/a.GIF","play":"button","loop":true,"duration":1.5}"#,
        )
        .unwrap();
        assert!(gif.is_gif() && gif.looping && gif.play == Play::Button && gif.duration == Some(1.5));
    }

    #[test]
    fn mirroring_turns_lines_images_and_groups() {
        let arrow = Shape { rect: Rect::new(0.0, 0.0, 10.0, 10.0), kind: ShapeKind::Arrow, ..Default::default() };
        let mut e = Element::Shape(arrow);
        e.mirror(true);
        assert!(matches!(&e, Element::Shape(s) if s.flip && s.reverse), "left and right swap: the head changes end");
        e.mirror(false);
        assert!(matches!(&e, Element::Shape(s) if !s.flip && s.reverse));
        let mut img = Element::Image(Image::default());
        img.mirror(true);
        img.mirror(false);
        assert!(matches!(&img, Element::Image(i) if i.mirror_h && i.mirror_v));
        let mut g = Element::Group(Group::of(vec![
            Element::Text(TextBox::plain(Rect::new(0.0, 0.0, 10.0, 10.0), "a")),
            Element::Text(TextBox::plain(Rect::new(90.0, 40.0, 10.0, 10.0), "b")),
        ]));
        g.mirror(true);
        let Element::Group(g) = &g else { unreachable!() };
        assert_eq!(g.children[0].rect().x, 90.0);
        assert_eq!(g.children[1].rect().x, 0.0);
        assert_eq!(g.children[1].rect().y, 40.0);
    }

    #[test]
    fn tables_lay_cells_on_a_grid() {
        let mut t = Table::new(Rect::new(100.0, 50.0, 400.0, 90.0), 3, 2);
        t.columns = vec![1.0, 3.0];
        assert_eq!(t.cell_rect(&t.rect, 0, 0), Rect::new(100.0, 50.0, 100.0, 30.0));
        assert_eq!(t.cell_rect(&t.rect, 2, 1), Rect::new(200.0, 110.0, 300.0, 30.0));
        t.rows[0][1] = Cell::plain("head\ntwo lines");
        assert_eq!(t.rows[0][1].text(), "head\ntwo lines");
        assert_eq!(t.cell_rect(&t.rect, 0, 0).h, 45.0, "a two-line row takes two shares of four");
        assert_eq!(t.cell_rect(&t.rect, 2, 0), Rect::new(100.0, 117.5, 100.0, 22.5));
        t.rows[1].push(Cell::default());
        t.square();
        assert!(t.rows.iter().all(|r| r.len() == 3));
        let json = serde_json::to_string(&Element::Table(t.clone())).unwrap();
        assert!(json.contains("\"type\":\"table\"") && json.contains("\"header\":true"));
        assert_eq!(serde_json::from_str::<Element>(&json).unwrap(), Element::Table(t));
    }

    #[test]
    fn presets_name_their_placeholders() {
        let names = |kind: &str| -> Vec<String> {
            Layout::preset("L", kind, 960.0, 540.0).placeholders.iter().map(|p| p.name.clone()).collect()
        };
        assert_eq!(names("title"), ["title", "subtitle"]);
        assert_eq!(names("compare"), ["title", "heading1", "body1", "heading2", "body2"]);
        assert!(names("blank").is_empty() && names("nonsense").is_empty());
        for (kind, _) in Layout::PRESETS {
            for p in &Layout::preset("L", kind, 960.0, 540.0).placeholders {
                assert!(
                    p.rect.x >= 0.0 && p.rect.x + p.rect.w <= 960.0 && p.rect.y + p.rect.h <= 540.0,
                    "{kind}: {p:?}"
                );
            }
        }
        let slide = Layout::preset("L", "two", 960.0, 540.0).new_slide();
        assert_eq!(slide.elements.len(), 3);
    }

    #[test]
    fn groups_move_and_scale_their_children() {
        let mut elements = vec![
            Element::Shape(Shape { rect: Rect::new(10.0, 10.0, 20.0, 20.0), ..Default::default() }),
            Element::Text(TextBox::plain(Rect::new(100.0, 50.0, 50.0, 10.0), "t")),
            Element::Shape(Shape { rect: Rect::new(0.0, 0.0, 5.0, 5.0), ..Default::default() }),
        ];
        assert_eq!(group_elements(&mut elements, &[2]), None, "one element is not a group");
        let at = group_elements(&mut elements, &[1, 0]).unwrap();
        assert_eq!((at, elements.len()), (0, 2));
        let Element::Group(g) = &elements[0] else { panic!() };
        assert_eq!(g.rect, Rect::new(10.0, 10.0, 140.0, 50.0));
        assert_eq!(g.children.len(), 2);
        // Move by (+10, +20) and double the width: children follow.
        elements[0].set_rect(Rect::new(20.0, 30.0, 280.0, 50.0));
        let Element::Group(g) = &elements[0] else { panic!() };
        assert_eq!(*g.children[0].rect(), Rect::new(20.0, 30.0, 40.0, 20.0));
        assert_eq!(*g.children[1].rect(), Rect::new(200.0, 70.0, 100.0, 10.0));
        let json = serde_json::to_string(&elements[0]).unwrap();
        assert!(json.contains("\"type\":\"group\"") && json.contains("\"children\""));
        assert_eq!(serde_json::from_str::<Element>(&json).unwrap(), elements[0]);
        assert_eq!(ungroup_element(&mut elements, 0), Some(2));
        assert_eq!(elements.len(), 3);
        assert_eq!(*elements[1].rect(), Rect::new(200.0, 70.0, 100.0, 10.0));
        assert_eq!(ungroup_element(&mut elements, 2), None, "not a group");
    }

    #[test]
    fn outline_box_highlights_the_current_section() {
        let mut deck = Deck::new("t");
        deck.slides.push(Slide { section: Some("Intro".into()), ..Default::default() });
        deck.slides.push(Slide {
            section: Some("Method".into()),
            subsection: Some("Model".into()),
            ..Default::default()
        });
        deck.slides.push(Slide { subsection: Some("Results".into()), ..Default::default() });
        deck.slides.push(Slide { section: Some("End".into()), ..Default::default() });
        let o = OutlineBox { subsections: true, ..Default::default() };
        let ps = deck.outline_paragraphs(Some(2), &o);
        let texts: Vec<(String, bool, Option<u8>)> = ps.iter().map(|p| (p.text(), p.runs[0].bold, p.bullet)).collect();
        assert_eq!(
            texts,
            [
                ("1. Intro".to_string(), false, None),
                ("2. Method".to_string(), true, None),
                ("Model".to_string(), false, Some(1)),
                ("Results".to_string(), true, Some(1)),
                ("3. End".to_string(), false, None),
            ]
        );
        assert!(
            deck.outline_paragraphs(None, &o).iter().all(|p| !p.runs[0].bold),
            "no current section without a slide"
        );
        let json = serde_json::to_string(&Element::Outline(o.clone())).unwrap();
        assert!(json.contains("\"type\":\"outline\"") && !json.contains("subsections"), "{json}");
        assert_eq!(serde_json::from_str::<Element>(&json).unwrap(), Element::Outline(o));
    }

    #[test]
    fn outline_follows_slide_order() {
        let mut deck = Deck::new("t");
        deck.slides.push(Slide { section: Some("Intro".into()), ..Default::default() });
        deck.slides.push(Slide::default());
        deck.slides.push(Slide {
            section: Some("Method".into()),
            subsection: Some("Model".into()),
            ..Default::default()
        });
        deck.slides.push(Slide { subsection: Some("Results".into()), ..Default::default() });
        let o = deck.outline();
        let flat: Vec<(u8, &str, usize)> = o.iter().map(|e| (e.level, e.title.as_str(), e.slide)).collect();
        assert_eq!(flat, [(1, "Intro", 0), (1, "Method", 2), (2, "Model", 2), (2, "Results", 3)]);
        assert_eq!(deck.position(1), (Some("Intro"), None));
        assert_eq!(deck.position(3), (Some("Method"), Some("Results")));
        let titled = Slide {
            elements: vec![Element::Text(TextBox::plain(Rect::default(), "  Hello\nworld"))],
            ..Default::default()
        };
        assert_eq!(titled.title(), "Hello");
    }

    #[test]
    fn steps_build_the_state() {
        let mut shape = Shape { rect: Rect::new(0.0, 0.0, 10.0, 10.0), ..Default::default() };
        shape.steps.push(Step::disappear(0)); // hidden from the start
        shape.steps.push(Step::appear(1));
        shape.steps.push(Step {
            at: 2,
            set: Overrides { x: Some(50.0), fill: Some(Color::WHITE), ..Default::default() },
            ..Default::default()
        });
        let e = Element::Shape(shape);
        assert_eq!(e.last_step(), 2);
        assert_eq!(e.state_at(0).opacity, 0.0);
        let s1 = e.state_at(1);
        assert_eq!((s1.opacity, s1.rect.x), (1.0, 0.0));
        let s2 = e.state_at(2);
        assert_eq!((s2.opacity, s2.rect.x, s2.fill), (1.0, 50.0, Some(Color::WHITE)));
        assert_eq!(e.state_at(9), s2);
        let slide = Slide { elements: vec![e], ..Default::default() };
        assert_eq!(slide.last_step(), 2);
    }

    #[test]
    fn math_round_trips() {
        let m = Element::Math(MathBlock {
            rect: Rect::new(1.0, 2.0, 3.0, 4.0),
            source: "x^2".into(),
            ..Default::default()
        });
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"type\":\"math\""));
        assert_eq!(serde_json::from_str::<Element>(&json).unwrap(), m);
        let r = Run { text: "a/b".into(), math: true, ..Default::default() };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"math\":true"));
        let d = Deck { math_preamble: "#let vb(x) = bold(x)".into(), ..Default::default() };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("math_preamble"));
        assert_eq!(serde_json::from_str::<Deck>(&json).unwrap(), d);
    }
}
