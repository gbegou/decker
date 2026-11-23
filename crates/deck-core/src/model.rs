//! The document model.

use serde::{Deserialize, Serialize};

/// Version of the `deck.json` schema. Bump it with a migration when the
/// model changes incompatibly.
pub const FORMAT_VERSION: u32 = 1;

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
    pub slides: Vec<Slide>,
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            title: String::new(),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            math_preamble: String::new(),
            slides: Vec::new(),
        }
    }
}

impl Deck {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), ..Default::default() }
    }
}

/// One slide.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Slide {
    /// Optional name, shown in the editor and usable from macros.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
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
}

/// Anything placed on a slide, in drawing order (first is at the back).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Text(TextBox),
    Image(Image),
    Shape(Shape),
    Math(MathBlock),
}

impl Element {
    pub fn rect(&self) -> &Rect {
        match self {
            Element::Text(t) => &t.rect,
            Element::Image(i) => &i.rect,
            Element::Shape(s) => &s.rect,
            Element::Math(m) => &m.rect,
        }
    }

    pub fn rect_mut(&mut self) -> &mut Rect {
        match self {
            Element::Text(t) => &mut t.rect,
            Element::Image(i) => &mut i.rect,
            Element::Shape(s) => &mut s.rect,
            Element::Math(m) => &mut m.rect,
        }
    }

    /// The build steps of the element, empty when it is static.
    pub fn steps(&self) -> &[Step] {
        match self {
            Element::Text(t) => &t.steps,
            Element::Image(i) => &i.steps,
            Element::Shape(s) => &s.steps,
            Element::Math(m) => &m.steps,
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

/// A displayed formula: Typst math source, rendered on display. The deck's
/// [`Deck::math_preamble`] and its `typst/*.typ` files are in scope.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MathBlock {
    pub rect: Rect,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub paragraphs: Vec<Paragraph>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    /// Path relative to the deck folder, e.g. `assets/image1.png`.
    pub src: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    #[default]
    Rectangle,
    Ellipse,
    /// A straight line from the top-left to the bottom-right of the rect.
    Line,
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
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[cfg(test)]
mod tests {
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
        assert!(json.contains("\"version\": 1"));
        assert!(json.contains("\"type\": \"text\""));
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
