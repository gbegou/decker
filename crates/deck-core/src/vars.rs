//! Variables in layout text: `{{section}}`, `{{subsection}}`, `{{slide}}`,
//! `{{slides}}`, `{{title}}`, `{{deck}}` and `{{date}}`, substituted when a
//! slide is rendered.
//!
//! A list joins values and quoted text: `{{section, " | ", subsection}}`,
//! `{{slide, " / ", slides}}`. Text between two values shows only when
//! both are non-empty, text before the first value only when it is, and
//! text after the last value only when it is: a missing subsection leaves
//! no dangling separator.

use crate::model::{Deck, Element, Paragraph};

/// The values for a given slide of a deck.
pub struct Vars {
    pub section: String,
    pub subsection: String,
    pub slide: usize,
    pub slides: usize,
    pub title: String,
    pub deck: String,
    pub date: String,
}

impl Vars {
    /// The variables of slide `index`; the date is today's, ISO formatted.
    pub fn for_slide(deck: &Deck, index: usize) -> Self {
        let (section, subsection) = deck.position(index);
        Self {
            section: section.unwrap_or("").to_string(),
            subsection: subsection.unwrap_or("").to_string(),
            slide: index + 1,
            slides: deck.slides.len(),
            title: deck.slides.get(index).map(|s| s.title()).unwrap_or_default(),
            deck: deck.title.clone(),
            date: today(),
        }
    }

    /// `text` with every `{{name}}` replaced; unknown names are kept.
    pub fn substitute(&self, text: &str) -> String {
        if !text.contains("{{") {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            match after.find("}}") {
                Some(end) => {
                    match self.expression(&after[..end]) {
                        Some(v) => out.push_str(&v),
                        None => {
                            out.push_str("{{");
                            out.push_str(&after[..end]);
                            out.push_str("}}");
                        }
                    }
                    rest = &after[end + 2..];
                }
                None => {
                    out.push_str("{{");
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }

    /// The value of what is between the braces: a name, or a list of names
    /// and quoted texts. `None` when a name is unknown or a quote unclosed.
    fn expression(&self, inside: &str) -> Option<String> {
        let mut out = String::new();
        let mut pending = String::new();
        let mut last_value_empty: Option<bool> = None;
        for item in split_items(inside)? {
            match item {
                Item::Text(t) => pending.push_str(&t),
                Item::Name(n) => {
                    let v = self.value(&n)?;
                    if !v.is_empty() {
                        // A separator only between two values that are there.
                        // Text before the first value leads it; later text
                        // separates it from a value already shown.
                        if last_value_empty.is_none() || !out.is_empty() {
                            out.push_str(&pending);
                        }
                        out.push_str(&v);
                    }
                    pending.clear();
                    last_value_empty = Some(v.is_empty());
                }
            }
        }
        if last_value_empty == Some(false) || last_value_empty.is_none() {
            out.push_str(&pending);
        }
        Some(out)
    }

    fn value(&self, name: &str) -> Option<String> {
        Some(match name {
            "section" => self.section.clone(),
            "subsection" => self.subsection.clone(),
            "slide" => self.slide.to_string(),
            "slides" => self.slides.to_string(),
            "title" => self.title.clone(),
            "deck" => self.deck.clone(),
            "date" => self.date.clone(),
            _ => return None,
        })
    }

    /// Paragraphs with their run texts substituted.
    pub fn paragraphs(&self, paragraphs: &[Paragraph]) -> Vec<Paragraph> {
        paragraphs
            .iter()
            .map(|p| {
                let mut p = p.clone();
                for r in &mut p.runs {
                    if !r.math {
                        r.text = self.substitute(&r.text);
                    }
                }
                p
            })
            .collect()
    }

    /// An element with its text substituted (images unchanged).
    pub fn element(&self, element: &Element) -> Element {
        match element {
            Element::Text(t) if has_vars(&t.paragraphs) => {
                let mut t = t.clone();
                t.paragraphs = self.paragraphs(&t.paragraphs);
                Element::Text(t)
            }
            Element::Shape(s) if has_vars(&s.paragraphs) => {
                let mut s = s.clone();
                s.paragraphs = self.paragraphs(&s.paragraphs);
                Element::Shape(s)
            }
            Element::Group(g) => {
                let mut g = g.clone();
                g.children = g.children.iter().map(|c| self.element(c)).collect();
                Element::Group(g)
            }
            other => other.clone(),
        }
    }
}

/// Whether any run mentions a variable.
pub fn has_vars(paragraphs: &[Paragraph]) -> bool {
    paragraphs.iter().any(|p| p.runs.iter().any(|r| r.text.contains("{{")))
}

/// Today as `YYYY-MM-DD`, from the system clock (UTC).
fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (secs / 86_400) as i64;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

enum Item {
    Name(String),
    Text(String),
}

/// The comma-separated items of a variable: names, and texts in double
/// quotes (`\"` and `\\` escape inside). `None` on an unclosed quote.
fn split_items(inside: &str) -> Option<Vec<Item>> {
    let mut items = vec![];
    let mut chars = inside.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        match chars.peek() {
            None => break,
            Some('"') => {
                chars.next();
                let mut text = String::new();
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => text.push(chars.next()?),
                        c => text.push(c),
                    }
                }
                items.push(Item::Text(text));
            }
            Some(_) => {
                let mut name = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ',' {
                        break;
                    }
                    name.push(c);
                    chars.next();
                }
                items.push(Item::Name(name.trim().to_string()));
            }
        }
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        match chars.next() {
            None => break,
            Some(',') => {}
            Some(_) => return None,
        }
    }
    Some(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn lists_join_values_without_dangling_separators() {
        let v = Vars {
            section: "Intro".into(),
            subsection: String::new(),
            slide: 3,
            slides: 12,
            title: "T".into(),
            deck: "D".into(),
            date: "2026-01-01".into(),
        };
        assert_eq!(v.substitute(r#"{{slide, " / ", slides}}"#), "3 / 12");
        assert_eq!(v.substitute(r#"{{section, " | ", subsection}}"#), "Intro", "no subsection, no separator");
        assert_eq!(v.substitute(r#"{{subsection, " | ", section}}"#), "Intro");
        assert_eq!(v.substitute(r#"{{section, " · ", subsection, " · ", slide}}"#), "Intro · 3");
        assert_eq!(v.substitute(r#"{{"Part ", section, "!"}}"#), "Part Intro!");
        assert_eq!(v.substitute(r#"{{"(", subsection, ")"}}"#), "", "around an empty value: nothing");
        assert_eq!(v.substitute(r#"{{"say \"hi\", ", deck}}"#), "say \"hi\", D", "escapes and commas in quotes");
        assert_eq!(
            v.substitute(r#"{{slide, nope}} {{section, "unclosed}}"#),
            r#"{{slide, nope}} {{section, "unclosed}}"#
        );
        assert_eq!(v.substitute("{{ section }}"), "Intro");
    }

    #[test]
    fn substitutes_known_names_and_keeps_others() {
        let mut deck = Deck::new("My talk");
        deck.slides.push(Slide { section: Some("Intro".into()), name: "Welcome".into(), ..Default::default() });
        deck.slides.push(Slide { subsection: Some("Plan".into()), ..Default::default() });
        let v = Vars::for_slide(&deck, 1);
        assert_eq!(
            v.substitute("{{section}} › {{subsection}} — {{slide}}/{{slides}} ({{deck}})"),
            "Intro › Plan — 2/2 (My talk)"
        );
        assert_eq!(v.substitute("{{ title }}|{{unknown}}|{{broken"), "|{{unknown}}|{{broken");
        assert_eq!(Vars::for_slide(&deck, 0).substitute("{{title}}"), "Welcome");
        assert_eq!(v.date.len(), 10);
        let el = Element::Text(TextBox::plain(Rect::default(), "p. {{slide}}"));
        let Element::Text(t) = v.element(&el) else { panic!() };
        assert_eq!(t.text(), "p. 2");
        let math = Paragraph {
            runs: vec![Run { text: "{{x}}".into(), math: true, ..Default::default() }],
            ..Default::default()
        };
        assert_eq!(v.paragraphs(&[math])[0].runs[0].text, "{{x}}", "formulas are left alone");
    }

    #[test]
    fn today_is_a_date() {
        let t = today();
        assert_eq!(t.len(), 10);
        assert!(t.starts_with("20"));
    }
}
