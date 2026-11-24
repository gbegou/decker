//! Renders a slide to SVG. The same SVG serves the editor canvas, the
//! presentation window and the PDF export.
//!
//! Text layout is left to the SVG viewer: each paragraph becomes one
//! `<text>` line, wrapped by a naive word-wrap on an estimated glyph width.
//! A real layout engine comes with the editor.

use std::fmt::Write;

use crate::model::*;

/// Default font size in points when a run sets none.
pub const DEFAULT_FONT_SIZE: f64 = 18.0;
pub const DEFAULT_FONT: &str = "sans-serif";
/// Average glyph width as a fraction of the font size, for word wrapping.
const GLYPH_WIDTH: f64 = 0.55;
const LINE_HEIGHT: f64 = 1.2;
/// Inset of text inside its box, in points.
pub const PADDING: f64 = 4.0;

/// A rendered formula: the children of an SVG root (glyph outlines), and
/// its natural size in points.
#[derive(Debug, Clone, PartialEq)]
pub struct MathImage {
    pub content: String,
    pub width: f64,
    pub height: f64,
}

/// Typesets formulas and text. `deck-typst` implements it; without one,
/// formulas are drawn as their source and text is wrapped by an estimate.
pub trait MathRenderer {
    /// Renders Typst math `source` with the deck's `preamble` in scope.
    fn render_math(&self, preamble: &str, source: &str, size: f64, color: Color) -> Option<MathImage>;

    /// Typesets paragraphs in a column `width` points wide, `defaults`
    /// giving the font, size and color of runs that set none. The result's
    /// height is the text's natural height; `None` falls back to the
    /// estimate.
    fn render_text(
        &self,
        _preamble: &str,
        _paragraphs: &[Paragraph],
        _width: f64,
        _defaults: &TextDefaults,
    ) -> Option<MathImage> {
        None
    }
}

/// The renderer that draws formulas as their source text.
pub struct NoMath;

impl MathRenderer for NoMath {
    fn render_math(&self, _: &str, _: &str, _: f64, _: Color) -> Option<MathImage> {
        None
    }
}

/// Renders one slide at its final build step, as a standalone SVG document.
pub fn render_slide(deck: &Deck, slide: &Slide) -> String {
    render_step(deck, slide, slide.last_step())
}

/// Renders one slide at build step `step`, formulas as their source.
pub fn render_step(deck: &Deck, slide: &Slide, step: u32) -> String {
    render_step_with(deck, slide, step, &NoMath)
}

/// Renders one slide at build step `step`. Every element is wrapped in a
/// `<g id="e{index}">` carrying its opacity and rotation, so that a
/// presentation window can transition each element in place between steps.
pub fn render_step_with(deck: &Deck, slide: &Slide, step: u32, math: &dyn MathRenderer) -> String {
    let slide_index = deck.slides.iter().position(|s| std::ptr::eq(s, slide));
    let ctx = Ctx { deck, math, slide_index, defaults: deck.theme_of(slide).defaults() };
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="{font}">"#,
        w = fmt(deck.width),
        h = fmt(deck.height),
        font = DEFAULT_FONT
    );
    let layout = deck.layout_of(slide);
    let bg = slide.background.or(layout.and_then(|l| l.background)).unwrap_or(Color::WHITE);
    let _ = write!(out, r#"<rect width="100%" height="100%" fill="{}"/>"#, bg.to_hex());
    if let Some(layout) = layout {
        // The layout's static elements, under the slide, at their final
        // step, with the variables of this slide substituted.
        let vars = slide_index.map(|i| crate::vars::Vars::for_slide(deck, i));
        out.push_str(r#"<g id="layout">"#);
        for element in &layout.elements {
            let substituted = vars.as_ref().map(|v| v.element(element));
            let element = substituted.as_ref().unwrap_or(element);
            element_body(&mut out, &ctx, element, element.last_step(), None);
        }
        out.push_str("</g>");
    }
    for (index, element) in slide.elements.iter().enumerate() {
        element_body(&mut out, &ctx, element, step, Some(index));
    }
    out.push_str("</svg>\n");
    out
}

/// Renders a layout for its editor: background, static elements (with
/// `id="e{index}"` like slide elements), then its placeholders as dashed
/// boxes showing their names, with ids continuing the numbering.
pub fn render_layout(deck: &Deck, layout: &Layout, math: &dyn MathRenderer) -> String {
    let ctx = Ctx { deck, math, slide_index: None, defaults: deck.theme_of_layout(layout).defaults() };
    let defaults = &ctx.defaults;
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="{font}">"#,
        w = fmt(deck.width),
        h = fmt(deck.height),
        font = DEFAULT_FONT
    );
    let bg = layout.background.unwrap_or(Color::WHITE);
    let _ = write!(out, r#"<rect width="100%" height="100%" fill="{}"/>"#, bg.to_hex());
    for (index, element) in layout.elements.iter().enumerate() {
        element_body(&mut out, &ctx, element, element.last_step(), Some(index));
    }
    for (k, ph) in layout.placeholders.iter().enumerate() {
        let index = layout.elements.len() + k;
        let r = &ph.rect;
        let _ = write!(
            out,
            r##"<g id="e{index}"><rect x="{}" y="{}" width="{}" height="{}" fill="#F3F6FF" stroke="#8AA4D8" stroke-width="1.5" stroke-dasharray="6 4"/>"##,
            fmt(r.x),
            fmt(r.y),
            fmt(r.w),
            fmt(r.h)
        );
        let sample = ph.text_box();
        text_body(&mut out, r, &sample.paragraphs, &deck.math_preamble, math, defaults);
        out.push_str("</g>");
    }
    out.push_str("</svg>\n");
    out
}

/// What every element of one render shares.
struct Ctx<'a> {
    deck: &'a Deck,
    math: &'a dyn MathRenderer,
    /// The slide's index in the deck, for the outline's highlight.
    slide_index: Option<usize>,
    defaults: TextDefaults,
}

/// One element at a step: a `<g>` (with `id="e{index}"` for slide
/// elements) carrying its opacity and rotation.
fn element_body(out: &mut String, ctx: &Ctx, element: &Element, step: u32, index: Option<usize>) {
    let (deck, math, slide_index, defaults) = (ctx.deck, ctx.math, ctx.slide_index, &ctx.defaults);
    {
        let st = element.state_at(step);
        let r = &st.rect;
        match index {
            Some(i) => {
                let _ = write!(out, r#"<g id="e{i}""#);
            }
            None => out.push_str("<g"),
        }
        if st.opacity != 1.0 {
            let _ = write!(out, r#" opacity="{}""#, fmt(st.opacity));
        }
        if st.rotation != 0.0 {
            let _ = write!(
                out,
                r#" transform="rotate({} {} {})""#,
                fmt(st.rotation),
                fmt(r.x + r.w / 2.0),
                fmt(r.y + r.h / 2.0)
            );
        }
        out.push('>');
        match element {
            Element::Text(t) => {
                shape_body(out, r, ShapeKind::Rectangle, st.fill.or(t.fill), t.stroke);
                text_body(out, r, &t.paragraphs, &deck.math_preamble, math, defaults);
            }
            Element::Image(i) => image_body(out, r, i, index),
            Element::Shape(s) => {
                shape_body(out, r, s.kind, st.fill.or(s.fill), s.stroke);
                text_body(out, r, &s.paragraphs, &deck.math_preamble, math, defaults);
            }
            Element::Math(m) => math_body(out, r, m, &deck.math_preamble, math, defaults),
            Element::Group(g) => {
                // The group's movement and scaling at this step, as a transform
                // of the children drawn at their base positions.
                let (sx, sy) = (
                    if g.rect.w > 0.0 { r.w / g.rect.w } else { 1.0 },
                    if g.rect.h > 0.0 { r.h / g.rect.h } else { 1.0 },
                );
                let moved = (r.x - g.rect.x).abs() > 1e-9
                    || (r.y - g.rect.y).abs() > 1e-9
                    || (sx - 1.0).abs() > 1e-9
                    || (sy - 1.0).abs() > 1e-9;
                if moved {
                    let _ = write!(
                        out,
                        r#"<g transform="translate({} {}) scale({} {}) translate({} {})">"#,
                        fmt(r.x),
                        fmt(r.y),
                        fmt(sx),
                        fmt(sy),
                        fmt(-g.rect.x),
                        fmt(-g.rect.y)
                    );
                }
                for child in &g.children {
                    element_body(out, ctx, child, step, None);
                }
                if moved {
                    out.push_str("</g>");
                }
            }
            Element::Outline(o) => {
                let paragraphs = deck.outline_paragraphs(slide_index, o);
                if paragraphs.is_empty() {
                    let hint = Paragraph {
                        runs: vec![Run {
                            text: "Outline: no sections yet".into(),
                            color: Some(Color::rgb(0x88, 0x88, 0x88)),
                            size: o.size,
                            ..Default::default()
                        }],
                        ..Default::default()
                    };
                    text_body(out, r, &[hint], &deck.math_preamble, math, defaults);
                } else {
                    text_body(out, r, &paragraphs, &deck.math_preamble, math, defaults);
                }
            }
        }
        out.push_str("</g>");
    }
}

/// An image filling its rect, cropped and clipped to its mask. The crop
/// scales the full image up so that the kept part fills the rect; the
/// mask is a `<clipPath>` with a unique id per element.
fn image_body(out: &mut String, r: &Rect, i: &Image, index: Option<usize>) {
    let c = i.crop.clamped();
    let (kw, kh) = ((1.0 - c.left - c.right).max(0.05), (1.0 - c.top - c.bottom).max(0.05));
    let full_w = r.w / kw;
    let full_h = r.h / kh;
    let x = r.x - c.left * full_w;
    let y = r.y - c.top * full_h;
    let clip_id =
        format!("clip-{}-{}", index.map(|i| i.to_string()).unwrap_or_else(|| "l".into()), (r.x * 7.0 + r.y) as i64);
    let clip = match i.mask {
        Mask::Rectangle if c.is_none() => None,
        Mask::Rectangle => {
            Some(format!(r#"<rect x="{}" y="{}" width="{}" height="{}"/>"#, fmt(r.x), fmt(r.y), fmt(r.w), fmt(r.h)))
        }
        Mask::Ellipse => Some(format!(
            r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}"/>"#,
            fmt(r.x + r.w / 2.0),
            fmt(r.y + r.h / 2.0),
            fmt(r.w / 2.0),
            fmt(r.h / 2.0)
        )),
        Mask::RoundedRectangle { radius } => {
            let rad = radius.clamp(0.0, 0.5) * r.w.min(r.h);
            Some(format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}"/>"#,
                fmt(r.x),
                fmt(r.y),
                fmt(r.w),
                fmt(r.h),
                fmt(rad)
            ))
        }
    };
    if let Some(shape) = &clip {
        let _ = write!(out, r#"<clipPath id="{clip_id}">{shape}</clipPath>"#);
    }
    let _ = write!(
        out,
        r#"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" xlink:href="{}"{}/>"#,
        fmt(x),
        fmt(y),
        fmt(full_w),
        fmt(full_h),
        escape(&i.src),
        if clip.is_some() { format!(r#" clip-path="url(#{clip_id})""#) } else { String::new() }
    );
}

/// A formula, centered in its box: rendered glyphs when a math renderer
/// succeeds, scaled down to fit when larger than the box; otherwise its
/// source in a monospace font, tagged with `data-math`.
fn math_body(
    out: &mut String,
    r: &Rect,
    m: &MathBlock,
    preamble: &str,
    math: &dyn MathRenderer,
    defaults: &TextDefaults,
) {
    let size = m.size.or(defaults.size).unwrap_or(DEFAULT_FONT_SIZE);
    let color_value = m.color.or(defaults.color).unwrap_or(Color::BLACK);
    if let Some(img) = math.render_math(preamble, &m.source, size, color_value) {
        if img.width > 0.0 && img.height > 0.0 {
            let scale = (r.w / img.width).min(r.h / img.height).min(1.0);
            let (w, h) = (img.width * scale, img.height * scale);
            let _ = write!(
                out,
                r#"<svg x="{}" y="{}" width="{}" height="{}" viewBox="0 0 {} {}" data-math="rendered">{}</svg>"#,
                fmt(r.x + (r.w - w) / 2.0),
                fmt(r.y + (r.h - h) / 2.0),
                fmt(w),
                fmt(h),
                fmt(img.width),
                fmt(img.height),
                img.content
            );
            return;
        }
    }
    let color = color_value.to_hex();
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" font-size="{}" font-family="monospace" fill="{color}" text-anchor="middle" dominant-baseline="middle" data-math="source">{}</text>"#,
        fmt(r.x + r.w / 2.0),
        fmt(r.y + r.h / 2.0),
        fmt(size),
        escape(&m.source)
    );
}

fn shape_body(out: &mut String, r: &Rect, kind: ShapeKind, fill: Option<Color>, stroke: Option<Stroke>) {
    if fill.is_none() && stroke.is_none() {
        return;
    }
    let fill = fill.map(Color::to_hex).unwrap_or_else(|| "none".into());
    let (sc, sw) = stroke.map(|s| (s.color.to_hex(), s.width)).unwrap_or_else(|| ("none".into(), 0.0));
    let style = format!(r#"fill="{fill}" stroke="{sc}" stroke-width="{}""#, fmt(sw));
    let _ = match kind {
        ShapeKind::Rectangle => write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" {style}/>"#,
            fmt(r.x),
            fmt(r.y),
            fmt(r.w),
            fmt(r.h)
        ),
        ShapeKind::Ellipse => write!(
            out,
            r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" {style}/>"#,
            fmt(r.x + r.w / 2.0),
            fmt(r.y + r.h / 2.0),
            fmt(r.w / 2.0),
            fmt(r.h / 2.0)
        ),
        ShapeKind::Line => write!(
            out,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" {style}/>"#,
            fmt(r.x),
            fmt(r.y),
            fmt(r.x + r.w),
            fmt(r.y + r.h)
        ),
    };
}

/// Text in a box: typeset by the renderer when it can, nested as an inner
/// `<svg>` at the box's top-left inset; else the estimate below.
fn text_body(
    out: &mut String,
    r: &Rect,
    paragraphs: &[Paragraph],
    preamble: &str,
    math: &dyn MathRenderer,
    defaults: &TextDefaults,
) {
    if paragraphs.iter().all(|p| p.text().is_empty()) {
        return;
    }
    let width = (r.w - 2.0 * PADDING).max(1.0);
    if let Some(img) = math.render_text(preamble, paragraphs, width, defaults) {
        if img.width > 0.0 && img.height > 0.0 {
            let _ = write!(
                out,
                r#"<svg x="{}" y="{}" width="{}" height="{}" viewBox="0 0 {} {}" overflow="visible" data-text="typeset">{}</svg>"#,
                fmt(r.x + PADDING),
                fmt(r.y + PADDING),
                fmt(img.width),
                fmt(img.height),
                fmt(img.width),
                fmt(img.height),
                img.content
            );
            return;
        }
    }
    text_estimate(out, r, paragraphs, defaults);
}

/// One `<text>` per line, wrapped on an estimated glyph width.
fn text_estimate(out: &mut String, r: &Rect, paragraphs: &[Paragraph], defaults: &TextDefaults) {
    let mut y = r.y + PADDING;
    let width = (r.w - 2.0 * PADDING).max(1.0);
    for p in paragraphs {
        let size = p.runs.iter().filter_map(|r| r.size).fold(0.0, f64::max);
        let size = if size > 0.0 { size } else { defaults.size.unwrap_or(DEFAULT_FONT_SIZE) };
        let indent = p.bullet.map(|level| (level as f64 + 1.0) * size).unwrap_or(0.0);
        let (anchor, x) = match p.align {
            Align::Left | Align::Justify => ("start", r.x + PADDING + indent),
            Align::Center => ("middle", r.x + r.w / 2.0),
            Align::Right => ("end", r.x + r.w - PADDING),
        };
        let lines = wrap(&p.runs, (width - indent) / (size * GLYPH_WIDTH));
        for (i, line) in lines.iter().enumerate() {
            y += size;
            let _ =
                write!(out, r#"<text x="{}" y="{}" font-size="{}" text-anchor="{anchor}">"#, fmt(x), fmt(y), fmt(size));
            if i == 0 {
                if let Some(level) = p.bullet {
                    let bx = r.x + PADDING + level as f64 * size;
                    let _ = write!(out, r#"<tspan x="{}">•</tspan><tspan x="{}">"#, fmt(bx), fmt(x));
                } else {
                    out.push_str("<tspan>");
                }
            } else {
                out.push_str("<tspan>");
            }
            for (run, text) in line {
                let mut attrs = String::new();
                if run.bold {
                    attrs.push_str(r#" font-weight="bold""#);
                }
                if run.italic {
                    attrs.push_str(r#" font-style="italic""#);
                }
                if let Some(s) = run.size {
                    let _ = write!(attrs, r#" font-size="{}""#, fmt(s));
                }
                if let Some(c) = run.color.or(defaults.color) {
                    let _ = write!(attrs, r#" fill="{}""#, c.to_hex());
                }
                if let Some(f) = run.font.as_ref().or(defaults.font.as_ref()) {
                    let _ = write!(attrs, r#" font-family="{}""#, escape(f));
                }
                if run.math {
                    attrs.push_str(r#" font-family="monospace" data-math="source""#);
                }
                let _ = write!(out, r#"<tspan xml:space="preserve"{attrs}>{}</tspan>"#, escape(text));
            }
            out.push_str("</tspan></text>");
            y += size * (LINE_HEIGHT - 1.0);
        }
        if lines.is_empty() {
            y += size * LINE_HEIGHT;
        }
    }
}

/// Splits runs into lines of at most `max_chars` characters, breaking at
/// spaces. Each line is a list of (run, text slice).
fn wrap(runs: &[Run], max_chars: f64) -> Vec<Vec<(&Run, String)>> {
    let max = max_chars.floor().max(1.0) as usize;
    let mut lines: Vec<Vec<(&Run, String)>> = vec![];
    let mut line: Vec<(&Run, String)> = vec![];
    let mut len = 0usize;
    for run in runs {
        for (i, word) in run.text.split(' ').enumerate() {
            let sep = if i > 0 && len > 0 { 1 } else { 0 };
            let wlen = word.chars().count();
            if len > 0 && len + sep + wlen > max {
                lines.push(std::mem::take(&mut line));
                len = 0;
            }
            if word.is_empty() && len > 0 && i > 0 {
                push(&mut line, run, " ");
                len += 1;
                continue;
            }
            let piece = if len > 0 && i > 0 { format!(" {word}") } else { word.to_string() };
            len += piece.chars().count();
            push(&mut line, run, &piece);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn push<'a>(line: &mut Vec<(&'a Run, String)>, run: &'a Run, text: &str) {
    if let Some((last, s)) = line.last_mut() {
        if std::ptr::eq(*last, run) {
            s.push_str(text);
            return;
        }
    }
    line.push((run, text.to_string()));
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Formats a number without trailing zeros.
fn fmt(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e12 {
        format!("{}", v as i64)
    } else {
        format!("{:.2}", v).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_text_image_and_shapes() {
        let mut deck = Deck::new("t");
        let mut slide = Slide { background: Some(Color::rgb(0, 0, 0)), ..Default::default() };
        let mut tb = TextBox::plain(Rect::new(10.0, 10.0, 500.0, 100.0), "Hello <world>");
        tb.paragraphs[0].runs[0].bold = true;
        slide.elements.push(Element::Text(tb));
        slide.elements.push(Element::Image(Image {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            src: "assets/a.png".into(),
            ..Default::default()
        }));
        slide.elements.push(Element::Shape(Shape {
            rect: Rect::new(1.0, 2.0, 3.0, 4.0),
            kind: ShapeKind::Ellipse,
            fill: Some(Color::WHITE),
            ..Default::default()
        }));
        deck.slides.push(slide);
        let svg = render_slide(&deck, &deck.slides[0]);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(r##"fill="#000000""##));
        assert!(svg.contains("Hello &lt;world&gt;"));
        assert!(svg.contains(r#"font-weight="bold""#));
        assert!(svg.contains(r#"xlink:href="assets/a.png""#));
        assert!(svg.contains(r#"<ellipse cx="2.5" cy="4" rx="1.5" ry="2""#));
    }

    #[test]
    fn layout_is_drawn_under_the_slide() {
        let mut deck = Deck::new("t");
        deck.layouts.push(Layout {
            name: "L".into(),
            background: Some(Color::rgb(1, 2, 3)),
            elements: vec![Element::Shape(Shape {
                rect: Rect::new(0.0, 500.0, 960.0, 40.0),
                fill: Some(Color::BLACK),
                ..Default::default()
            })],
            ..Default::default()
        });
        deck.slides.push(Slide {
            layout: Some("L".into()),
            elements: vec![Element::Text(TextBox::plain(Rect::new(0.0, 0.0, 10.0, 10.0), "x"))],
            ..Default::default()
        });
        deck.slides.push(Slide { layout: Some("missing".into()), ..Default::default() });
        let svg = render_slide(&deck, &deck.slides[0]);
        let layout_at = svg.find(r#"<g id="layout">"#).expect("layout group");
        let slide_at = svg.find(r#"<g id="e0">"#).expect("slide group");
        assert!(layout_at < slide_at);
        assert!(svg.contains(r##"fill="#010203""##), "layout background");
        let svg = render_slide(&deck, &deck.slides[1]);
        assert!(!svg.contains("layout"), "unknown layout is ignored");
        assert!(svg.contains(r##"fill="#FFFFFF""##));
    }

    #[test]
    fn layout_variables_follow_the_slide() {
        let mut deck = Deck::new("t");
        deck.layouts.push(Layout {
            name: "L".into(),
            elements: vec![Element::Text(TextBox::plain(
                Rect::new(0.0, 500.0, 400.0, 30.0),
                "{{section}} {{slide}}/{{slides}}",
            ))],
            ..Default::default()
        });
        deck.slides.push(Slide { layout: Some("L".into()), section: Some("Intro".into()), ..Default::default() });
        deck.slides.push(Slide { layout: Some("L".into()), ..Default::default() });
        let first = render_slide(&deck, &deck.slides[0]);
        let second = render_slide(&deck, &deck.slides[1]);
        assert!(first.contains("Intro 1/2"), "{first}");
        assert!(second.contains("Intro 2/2"), "{second}");
        let editor = render_layout(&deck, &deck.layouts[0], &NoMath);
        assert!(editor.contains("{{section}}"), "the layout editor shows the variables themselves");
    }

    #[test]
    fn groups_render_their_children_under_one_state() {
        let mut deck = Deck::new("t");
        let mut g = Group::of(vec![
            Element::Shape(Shape {
                rect: Rect::new(10.0, 10.0, 20.0, 20.0),
                fill: Some(Color::BLACK),
                ..Default::default()
            }),
            Element::Shape(Shape {
                rect: Rect::new(50.0, 10.0, 20.0, 20.0),
                fill: Some(Color::WHITE),
                ..Default::default()
            }),
        ]);
        g.steps = vec![
            Step::disappear(0),
            Step::appear(1),
            Step { at: 2, set: Overrides { x: Some(110.0), ..Default::default() }, ..Default::default() },
        ];
        deck.slides.push(Slide { elements: vec![Element::Group(g)], ..Default::default() });
        let s0 = render_step(&deck, &deck.slides[0], 0);
        assert!(s0.contains(r#"<g id="e0" opacity="0">"#), "the whole group fades: {s0}");
        assert_eq!(s0.matches("<rect ").count(), 3, "background + two children");
        let s2 = render_step(&deck, &deck.slides[0], 2);
        assert!(s2.contains(r#"transform="translate(110 10) scale(1 1) translate(-10 -10)""#), "moved as one: {s2}");
    }

    #[test]
    fn renders_steps_and_math() {
        let mut deck = Deck::new("t");
        let mut shape = Shape { rect: Rect::new(0.0, 0.0, 10.0, 10.0), fill: Some(Color::BLACK), ..Default::default() };
        shape.steps.push(Step::disappear(0));
        shape.steps.push(Step::appear(1));
        shape.steps.push(Step {
            at: 2,
            set: Overrides { rotation: Some(45.0), ..Default::default() },
            ..Default::default()
        });
        let math = MathBlock { rect: Rect::new(0.0, 0.0, 100.0, 50.0), source: "a < b".into(), ..Default::default() };
        deck.slides.push(Slide { elements: vec![Element::Shape(shape), Element::Math(math)], ..Default::default() });
        let slide = &deck.slides[0];
        let s0 = render_step(&deck, slide, 0);
        assert!(s0.contains(r#"<g id="e0" opacity="0">"#), "{s0}");
        let s1 = render_step(&deck, slide, 1);
        assert!(s1.contains(r#"<g id="e0">"#));
        let s2 = render_slide(&deck, slide);
        assert!(s2.contains(r#"transform="rotate(45 5 5)""#), "{s2}");
        assert!(s2.contains(r#"data-math="source">a &lt; b</text>"#));
    }

    struct FakeMath;
    impl MathRenderer for FakeMath {
        fn render_math(&self, preamble: &str, source: &str, size: f64, _: Color) -> Option<MathImage> {
            Some(MathImage { content: format!("<path d=\"{preamble}{source}\"/>"), width: size * 10.0, height: size })
        }
        fn render_text(&self, _: &str, paragraphs: &[Paragraph], width: f64, _: &TextDefaults) -> Option<MathImage> {
            Some(MathImage { content: format!("<path d=\"{}\"/>", paragraphs[0].text()), width, height: 30.0 })
        }
    }

    #[test]
    fn typeset_text_is_nested_at_the_box_inset() {
        let mut deck = Deck::new("t");
        deck.slides.push(Slide {
            elements: vec![Element::Text(TextBox::plain(Rect::new(100.0, 50.0, 400.0, 200.0), "Hello"))],
            ..Default::default()
        });
        let svg = render_step_with(&deck, &deck.slides[0], 0, &FakeMath);
        assert!(svg.contains(r#"<svg x="104" y="54" width="392" height="30" viewBox="0 0 392 30" overflow="visible" data-text="typeset"><path d="Hello"/></svg>"#), "{svg}");
        let plain = render_step(&deck, &deck.slides[0], 0);
        assert!(plain.contains("<text "), "fallback without a typesetter");
    }

    #[test]
    fn rendered_math_is_nested_and_fitted() {
        let mut deck = Deck::new("t");
        let math = MathBlock {
            rect: Rect::new(100.0, 100.0, 100.0, 50.0),
            source: "M".into(),
            size: Some(20.0),
            ..Default::default()
        };
        deck.slides.push(Slide { elements: vec![Element::Math(math)], ..Default::default() });
        let svg = render_step_with(&deck, &deck.slides[0], 0, &FakeMath);
        // natural 200 x 20 scaled by 0.5 to fit the 100 wide box, centered vertically
        assert!(svg.contains(r#"<svg x="100" y="120" width="100" height="10" viewBox="0 0 200 20" data-math="rendered"><path d="M"/></svg>"#), "{svg}");
    }

    #[test]
    fn cropped_and_masked_images() {
        let mut deck = Deck::new("t");
        let cropped = Image {
            rect: Rect::new(100.0, 100.0, 200.0, 100.0),
            src: "assets/a.png".into(),
            crop: Crop { left: 0.25, top: 0.0, right: 0.25, bottom: 0.5 },
            ..Default::default()
        };
        let oval = Image {
            rect: Rect::new(0.0, 0.0, 40.0, 20.0),
            src: "assets/b.png".into(),
            mask: Mask::Ellipse,
            ..Default::default()
        };
        deck.slides.push(Slide { elements: vec![Element::Image(cropped), Element::Image(oval)], ..Default::default() });
        let svg = render_slide(&deck, &deck.slides[0]);
        // Keeping the middle half horizontally and the top half vertically: the full image is 400 x 200 at (0, 100).
        assert!(svg.contains(r#"<image x="0" y="100" width="400" height="200" preserveAspectRatio="none" xlink:href="assets/a.png" clip-path="url(#clip-0-"#), "{svg}");
        assert!(
            svg.contains(r#"<clipPath id="clip-0-"#)
                && svg.contains(r#"<rect x="100" y="100" width="200" height="100"/></clipPath>"#)
        );
        assert!(svg.contains(r#"<ellipse cx="20" cy="10" rx="20" ry="10"/></clipPath>"#), "{svg}");
    }

    #[test]
    fn wraps_long_paragraphs() {
        let runs = vec![Run::plain("one two three four five")];
        let lines = wrap(&runs, 9.0);
        let texts: Vec<String> = lines.iter().map(|l| l.iter().map(|(_, s)| s.as_str()).collect()).collect();
        assert_eq!(texts, vec!["one two", "three", "four five"]);
    }

    #[test]
    fn wrap_keeps_run_boundaries() {
        let runs = vec![Run::plain("ab "), Run { text: "cd".into(), bold: true, ..Default::default() }];
        let lines = wrap(&runs, 100.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 2);
        assert_eq!(lines[0][1].1, "cd");
    }
}
