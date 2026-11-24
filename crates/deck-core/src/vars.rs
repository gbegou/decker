//! Variables in layout text: `{{section}}`, `{{subsection}}`, `{{slide}}`,
//! `{{slides}}`, `{{title}}`, `{{deck}}` and `{{date}}`, substituted when a
//! slide is rendered.

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
                    let name = after[..end].trim();
                    match self.value(name) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

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
