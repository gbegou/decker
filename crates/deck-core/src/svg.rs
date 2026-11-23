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
const PADDING: f64 = 4.0;

/// Renders one slide at its final build step, as a standalone SVG document.
pub fn render_slide(deck: &Deck, slide: &Slide) -> String {
    render_step(deck, slide, slide.last_step())
}

/// Renders one slide at build step `step`. Every element is wrapped in a
/// `<g id="e{index}">` carrying its opacity and rotation, so that a
/// presentation window can transition each element in place between steps.
pub fn render_step(deck: &Deck, slide: &Slide, step: u32) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="{font}">"#,
        w = fmt(deck.width),
        h = fmt(deck.height),
        font = DEFAULT_FONT
    );
    let bg = slide.background.unwrap_or(Color::WHITE);
    let _ = write!(out, r#"<rect width="100%" height="100%" fill="{}"/>"#, bg.to_hex());
    for (index, element) in slide.elements.iter().enumerate() {
        let st = element.state_at(step);
        let r = &st.rect;
        let _ = write!(out, r#"<g id="e{index}""#);
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
                shape_body(&mut out, r, ShapeKind::Rectangle, st.fill.or(t.fill), t.stroke);
                text_body(&mut out, r, &t.paragraphs);
            }
            Element::Image(i) => {
                let _ = write!(
                    out,
                    r#"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" xlink:href="{}"/>"#,
                    fmt(r.x),
                    fmt(r.y),
                    fmt(r.w),
                    fmt(r.h),
                    escape(&i.src)
                );
            }
            Element::Shape(s) => {
                shape_body(&mut out, r, s.kind, st.fill.or(s.fill), s.stroke);
                text_body(&mut out, r, &s.paragraphs);
            }
            Element::Math(m) => math_body(&mut out, r, m),
        }
        out.push_str("</g>");
    }
    out.push_str("</svg>\n");
    out
}

/// Until a math engine is embedded, a formula is drawn as its source in a
/// monospace font, centered, tagged with `data-math` so a web front-end
/// can replace it (KaTeX for LaTeX, Typst for Typst).
fn math_body(out: &mut String, r: &Rect, m: &MathBlock) {
    let size = m.size.unwrap_or(DEFAULT_FONT_SIZE);
    let color = m.color.unwrap_or(Color::BLACK).to_hex();
    let syntax = match m.syntax {
        MathSyntax::Typst => "typst",
        MathSyntax::Latex => "latex",
    };
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" font-size="{}" font-family="monospace" fill="{color}" text-anchor="middle" dominant-baseline="middle" data-math="{syntax}">{}</text>"#,
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

fn text_body(out: &mut String, r: &Rect, paragraphs: &[Paragraph]) {
    let mut y = r.y + PADDING;
    let width = (r.w - 2.0 * PADDING).max(1.0);
    for p in paragraphs {
        let size = p.runs.iter().filter_map(|r| r.size).fold(0.0, f64::max);
        let size = if size > 0.0 { size } else { DEFAULT_FONT_SIZE };
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
                if let Some(c) = run.color {
                    let _ = write!(attrs, r#" fill="{}""#, c.to_hex());
                }
                if let Some(f) = &run.font {
                    let _ = write!(attrs, r#" font-family="{}""#, escape(f));
                }
                if let Some(m) = run.math {
                    let syntax = match m {
                        MathSyntax::Typst => "typst",
                        MathSyntax::Latex => "latex",
                    };
                    let _ = write!(attrs, r#" font-family="monospace" data-math="{syntax}""#);
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
            steps: vec![],
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
        let math = MathBlock {
            rect: Rect::new(0.0, 0.0, 100.0, 50.0),
            source: "a < b".into(),
            syntax: MathSyntax::Latex,
            ..Default::default()
        };
        deck.slides.push(Slide { elements: vec![Element::Shape(shape), Element::Math(math)], ..Default::default() });
        let slide = &deck.slides[0];
        let s0 = render_step(&deck, slide, 0);
        assert!(s0.contains(r#"<g id="e0" opacity="0">"#), "{s0}");
        let s1 = render_step(&deck, slide, 1);
        assert!(s1.contains(r#"<g id="e0">"#));
        let s2 = render_slide(&deck, slide);
        assert!(s2.contains(r#"transform="rotate(45 5 5)""#), "{s2}");
        assert!(s2.contains(r#"data-math="latex">a &lt; b</text>"#));
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
