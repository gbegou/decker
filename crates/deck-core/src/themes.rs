//! Themes: what a family of layouts shares.
//!
//! A [`ThemeSet`] holds a text style (fonts, colors, palette), a
//! background, shared elements (a logo, a footer) and the default style of
//! each kind of placeholder (title, subtitle, body...). A [`Layout`] that
//! names it as its `parent` stores only what it changes: its placeholders'
//! positions and the style properties it overrides (listed in each
//! placeholder's `own`), a background, a style, elements of its own. So a
//! change to the theme reaches every layout that did not override it, and
//! an override is undone by dropping it ([`Layout::reset_to_theme`],
//! [`Placeholder::reset`]).
//!
//! Readers (the renderer, new slides) use [`Deck::resolve`], which merges a
//! layout with its theme; editors work on the layout itself.

use serde::{Deserialize, Serialize};

use crate::model::*;

/// A theme: see the [module](self) documentation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ThemeSet {
    pub name: String,
    /// Default text font, size and colors, and the palette.
    #[serde(default, skip_serializing_if = "is_default")]
    pub style: Theme,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    /// Drawn on every slide of the theme's layouts (unless a layout hides
    /// them), under the layout's own elements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Element>,
    /// The default style of each kind of placeholder, by name (`title`,
    /// `subtitle`, `body`...); their positions are the layouts' business.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placeholders: Vec<Placeholder>,
}

/// The style properties a placeholder can override.
pub const PLACEHOLDER_PROPS: [&str; 6] = ["size", "bold", "color", "font", "align", "bullets"];

/// The kind of a placeholder name: `body2` is a `body`.
pub fn kind_of(name: &str) -> &str {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
}

impl ThemeSet {
    /// The theme's placeholder for a layout placeholder named `name`: of
    /// the same name, else of the same kind.
    pub fn placeholder_for(&self, name: &str) -> Option<&Placeholder> {
        self.placeholders
            .iter()
            .find(|p| p.name == name)
            .or_else(|| self.placeholders.iter().find(|p| kind_of(&p.name) == kind_of(name)))
    }
}

impl Placeholder {
    /// Whether this placeholder sets property `prop` itself.
    pub fn owns(&self, prop: &str) -> bool {
        self.own.iter().any(|p| p == prop)
    }

    /// Copies property `prop` from `from`.
    fn take(&mut self, prop: &str, from: &Placeholder) {
        match prop {
            "size" => self.size = from.size,
            "bold" => self.bold = from.bold,
            "color" => self.color = from.color,
            "font" => self.font.clone_from(&from.font),
            "align" => self.align = from.align,
            "bullets" => self.bullets = from.bullets,
            _ => {}
        }
    }

    fn same(&self, prop: &str, other: &Placeholder) -> bool {
        match prop {
            "size" => self.size == other.size,
            "bold" => self.bold == other.bold,
            "color" => self.color == other.color,
            "font" => self.font == other.font,
            "align" => self.align == other.align,
            "bullets" => self.bullets == other.bullets,
            _ => true,
        }
    }

    /// This placeholder with the properties it does not own taken from
    /// `theme`'s placeholder of its kind (`own` emptied: it is resolved).
    pub fn resolved(&self, theme: Option<&ThemeSet>) -> Placeholder {
        let mut out = self.clone();
        if let Some(base) = theme.and_then(|t| t.placeholder_for(&self.name)) {
            for prop in PLACEHOLDER_PROPS {
                if !self.owns(prop) {
                    out.take(prop, base);
                }
            }
        }
        out.own.clear();
        out
    }

    /// Marks `prop` as set by this placeholder (after an edit of it).
    pub fn set_own(&mut self, prop: &str) {
        if PLACEHOLDER_PROPS.contains(&prop) && !self.owns(prop) {
            self.own.push(prop.to_string());
        }
    }

    /// Drops the override of `prop`: the theme's value applies again.
    pub fn reset(&mut self, prop: &str, theme: Option<&ThemeSet>) {
        self.own.retain(|p| p != prop);
        if let Some(base) = theme.and_then(|t| t.placeholder_for(&self.name)) {
            self.take(prop, base);
        }
    }
}

impl Layout {
    /// Drops every override: style, background, placeholder styles, hidden
    /// theme elements. The positions and the layout's own elements stay.
    pub fn reset_to_theme(&mut self, theme: Option<&ThemeSet>) {
        self.theme = Theme::default();
        self.background = None;
        self.hide_theme_elements = false;
        for p in &mut self.placeholders {
            for prop in PLACEHOLDER_PROPS {
                p.reset(prop, theme);
            }
        }
    }

    /// What this layout overrides of its theme, in words, for the panel.
    pub fn overrides(&self) -> Vec<String> {
        let mut out = vec![];
        if self.background.is_some() {
            out.push("background".into());
        }
        if self.theme != Theme::default() {
            out.push("text style".into());
        }
        if self.hide_theme_elements {
            out.push("hides the theme's elements".into());
        }
        for p in &self.placeholders {
            if !p.own.is_empty() {
                out.push(format!("{}: {}", p.name, p.own.join(", ")));
            }
        }
        out
    }
}

impl Deck {
    /// `layout` merged with its theme: the theme's background, style,
    /// shared elements and placeholder styles where the layout sets none.
    /// A layout without a theme is returned as is.
    pub fn resolve(&self, layout: &Layout) -> Layout {
        let theme = layout.parent.as_deref().and_then(|n| self.theme_set(n));
        let Some(t) = theme else {
            let mut out = layout.clone();
            out.placeholders = layout.placeholders.iter().map(|p| p.resolved(None)).collect();
            return out;
        };
        let mut elements = if layout.hide_theme_elements { vec![] } else { t.elements.clone() };
        elements.extend(layout.elements.iter().cloned());
        Layout {
            name: layout.name.clone(),
            background: layout.background.or(t.background),
            theme: layout.theme.over(&t.style),
            elements,
            placeholders: layout.placeholders.iter().map(|p| p.resolved(Some(t))).collect(),
            parent: layout.parent.clone(),
            hide_theme_elements: false,
        }
    }

    /// The layouts of theme `name`, in deck order.
    pub fn layouts_of_theme(&self, name: &str) -> Vec<&Layout> {
        self.layouts.iter().filter(|l| l.parent.as_deref() == Some(name)).collect()
    }

    /// Makes a theme named `name` of the layouts named `members` (layouts
    /// without a theme): what they all share moves into the theme, and
    /// each keeps only its differences. Shared: the text style, the
    /// background, the elements every one of them has, and per kind of
    /// placeholder the most common style. Nothing drawn changes.
    pub fn factor_theme(&mut self, name: &str, members: &[String]) {
        let idx: Vec<usize> = (0..self.layouts.len())
            .filter(|&i| members.contains(&self.layouts[i].name) && self.layouts[i].parent.is_none())
            .collect();
        if idx.is_empty() {
            return;
        }
        let first = self.layouts[idx[0]].clone();
        let all = |f: &dyn Fn(&Layout) -> bool| idx.iter().all(|&i| f(&self.layouts[i]));
        let mut theme = ThemeSet { name: name.to_string(), ..Default::default() };
        if all(&|l| l.theme == first.theme) {
            theme.style = first.theme.clone();
        }
        if first.background.is_some() && all(&|l| l.background == first.background) {
            theme.background = first.background;
        }
        theme.elements = first.elements.iter().filter(|e| all(&|l| l.elements.contains(e))).cloned().collect();
        // Per kind: the most common style among the layouts' placeholders.
        let mut kinds: Vec<String> = vec![];
        for &i in &idx {
            for p in &self.layouts[i].placeholders {
                let k = kind_of(&p.name).to_string();
                if !kinds.contains(&k) {
                    kinds.push(k);
                }
            }
        }
        for kind in kinds {
            let of_kind: Vec<&Placeholder> = idx
                .iter()
                .flat_map(|&i| self.layouts[i].placeholders.iter())
                .filter(|p| kind_of(&p.name) == kind)
                .collect();
            let mut best: Option<(&Placeholder, usize)> = None;
            for p in &of_kind {
                let n = of_kind.iter().filter(|q| PLACEHOLDER_PROPS.iter().all(|prop| p.same(prop, q))).count();
                if best.is_none_or(|(_, m)| n > m) {
                    best = Some((p, n));
                }
            }
            if let Some((p, _)) = best {
                theme.placeholders.push(Placeholder { name: kind, rect: Rect::default(), own: vec![], ..(*p).clone() });
            }
        }
        for &i in &idx {
            let l = &mut self.layouts[i];
            if theme.style == l.theme {
                l.theme = Theme::default();
            }
            if theme.background.is_some() && theme.background == l.background {
                l.background = None;
            }
            l.elements.retain(|e| !theme.elements.contains(e));
            for p in &mut l.placeholders {
                p.own = match theme.placeholder_for(&p.name) {
                    Some(base) => {
                        PLACEHOLDER_PROPS.iter().filter(|prop| !p.same(prop, base)).map(|s| s.to_string()).collect()
                    }
                    None => PLACEHOLDER_PROPS.iter().map(|s| s.to_string()).collect(),
                };
            }
            l.parent = Some(name.to_string());
        }
        self.themes.push(theme);
    }

    /// Brings a deck of an older format up to date: before themes, all its
    /// layouts become one theme, factored (see [`Deck::factor_theme`]).
    pub fn migrate(&mut self) {
        if self.version < THEMES_VERSION && self.themes.is_empty() && !self.layouts.is_empty() {
            let names: Vec<String> = self.layouts.iter().map(|l| l.name.clone()).collect();
            self.factor_theme("Theme", &names);
        }
        self.version = self.version.max(FORMAT_VERSION);
    }

    /// A theme name not yet taken, from `base`.
    pub fn free_theme_name(&self, base: &str) -> String {
        let mut name = base.to_string();
        let mut n = 1;
        while self.theme_set(&name).is_some() {
            n += 1;
            name = format!("{base} {n}");
        }
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(y: f64) -> Element {
        Element::Text(TextBox::plain(Rect::new(0.0, y, 100.0, 20.0), "footer"))
    }

    fn two_layouts() -> Deck {
        // A deck saved before themes: its layouts get factored on load.
        let mut deck = Deck { version: 1, ..Deck::new("t") };
        let logo = text(500.0);
        let title = |size: f64| Placeholder {
            name: "title".into(),
            rect: Rect::new(10.0, 10.0, 500.0, 60.0),
            size: Some(size),
            bold: true,
            ..Default::default()
        };
        deck.layouts.push(Layout {
            name: "Title".into(),
            background: Some(Color::WHITE),
            elements: vec![logo.clone(), text(10.0)],
            placeholders: vec![title(48.0)],
            ..Default::default()
        });
        deck.layouts.push(Layout {
            name: "Content".into(),
            background: Some(Color::WHITE),
            elements: vec![logo],
            placeholders: vec![
                title(32.0),
                Placeholder { name: "body1".into(), size: Some(20.0), bullets: true, ..Default::default() },
            ],
            ..Default::default()
        });
        deck.layouts.push(Layout {
            name: "Other".into(),
            background: Some(Color::WHITE),
            elements: vec![text(500.0)],
            placeholders: vec![title(32.0)],
            ..Default::default()
        });
        deck
    }

    #[test]
    fn factoring_keeps_the_look_and_moves_the_shared_parts() {
        let mut deck = two_layouts();
        let before: Vec<Layout> = deck.layouts.iter().map(|l| deck.resolve(l)).collect();
        deck.migrate();
        assert_eq!(deck.version, FORMAT_VERSION);
        let after: Vec<Layout> = deck.layouts.iter().map(|l| deck.resolve(l)).collect();
        for (b, a) in before.iter().zip(&after) {
            assert_eq!((&b.elements, b.background, &b.placeholders), (&a.elements, a.background, &a.placeholders));
        }
        let t = &deck.themes[0];
        assert_eq!(t.elements.len(), 1, "the logo is on every layout: shared");
        assert_eq!(t.placeholder_for("title").unwrap().size, Some(32.0), "the most common title size");
        let title = &deck.layout("Title").unwrap().placeholders[0];
        assert_eq!(title.own, ["size"], "the title slide overrides only its size");
        assert!(deck.layout("Content").unwrap().placeholders[0].own.is_empty());
    }

    #[test]
    fn a_theme_change_reaches_the_layouts_that_do_not_override_it() {
        let mut deck = two_layouts();
        deck.migrate();
        assert_eq!(deck.themes[0].elements.len(), 1, "the logo is on all: shared");
        assert_eq!(deck.themes[0].background, Some(Color::WHITE));
        deck.themes[0].placeholders.iter_mut().find(|p| p.name == "title").unwrap().size = Some(36.0);
        deck.themes[0].background = Some(Color::BLACK);
        let content = deck.resolved_layout("Content").unwrap();
        let title = deck.resolved_layout("Title").unwrap();
        assert_eq!(content.placeholders[0].size, Some(36.0));
        assert_eq!(title.placeholders[0].size, Some(48.0), "an override stays");
        assert_eq!(content.background, Some(Color::BLACK));
        assert_eq!(content.elements.len(), 1);
        assert_eq!(title.elements.len(), 2, "theme elements under the layout's own");
    }

    #[test]
    fn overrides_reset_to_the_theme() {
        let mut deck = two_layouts();
        deck.migrate();
        let theme = deck.themes[0].clone();
        let layout = deck.layouts.iter_mut().find(|l| l.name == "Title").unwrap();
        layout.background = Some(Color::BLACK);
        layout.hide_theme_elements = true;
        assert_eq!(layout.overrides(), ["background", "hides the theme's elements", "title: size"]);
        layout.placeholders[0].reset("size", Some(&theme));
        assert_eq!(layout.placeholders[0].size, Some(32.0));
        layout.reset_to_theme(Some(&theme));
        assert!(layout.overrides().is_empty());
        let title = deck.resolved_layout("Title").unwrap();
        assert_eq!((title.elements.len(), title.background), (2, Some(Color::WHITE)));
    }

    #[test]
    fn placeholders_of_a_kind_share_the_theme_style() {
        assert_eq!(kind_of("body2"), "body");
        let t = ThemeSet {
            placeholders: vec![Placeholder { name: "body".into(), size: Some(22.0), ..Default::default() }],
            ..Default::default()
        };
        let p = Placeholder { name: "body2".into(), size: Some(10.0), ..Default::default() };
        assert_eq!(p.resolved(Some(&t)).size, Some(22.0));
        let mut q = p.clone();
        q.set_own("size");
        assert_eq!(q.resolved(Some(&t)).size, Some(10.0));
    }
}
