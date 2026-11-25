//! Copying slides between decks: the slides, their images (renamed into
//! the destination's `assets/`), and the layouts they use when the
//! destination lacks them. Saving a layout on its own as a
//! `<name>.deck_layout/` folder (`layout.json` + `assets/`) and loading
//! one into a deck.

use std::collections::HashMap;
use std::path::Path;

use crate::model::{Deck, Element, Layout, Slide};
use crate::native::{self, NativeError, ASSETS_DIR};

/// Copies `indices` (0-based) of `src` into `dst` before position `at`
/// (clamped to the end). Returns the number of slides inserted.
///
/// Images are copied from `src_dir` into `dst_dir/assets/` under names
/// that do not collide with existing files. A layout named by a copied
/// slide is copied too, with its images, unless `dst` already has a
/// layout of that name, which is then used as is. The math preamble and
/// `typst/` files are not copied: merge them by hand when a formula of
/// the copied slide needs them.
pub fn copy_slides(
    src: &Deck,
    src_dir: &Path,
    indices: &[usize],
    dst: &mut Deck,
    dst_dir: &Path,
    at: usize,
) -> Result<usize, NativeError> {
    let mut assets = AssetCopier::new(src_dir, dst_dir);
    let mut at = at.min(dst.slides.len());
    let mut copied = 0;
    for &i in indices {
        let Some(slide) = src.slides.get(i) else { continue };
        let mut slide: Slide = slide.clone();
        for e in &mut slide.elements {
            assets.remap(e)?;
        }
        if let Some(name) = &slide.layout {
            if dst.layout(name).is_none() {
                if let Some(layout) = src.layout(name) {
                    let mut layout: Layout = layout.clone();
                    for e in &mut layout.elements {
                        assets.remap(e)?;
                    }
                    dst.layouts.push(layout);
                }
            }
        }
        dst.slides.insert(at, slide);
        at += 1;
        copied += 1;
    }
    Ok(copied)
}

/// The file of a saved layout inside its `.deck_layout` folder.
pub const LAYOUT_FILE: &str = "layout.json";

/// Writes `layout` of `deck` as `out_dir` (a `.deck_layout` folder:
/// `layout.json` and its images under `assets/`).
pub fn save_layout(deck: &Deck, deck_dir: &Path, name: &str, out_dir: &Path) -> Result<(), NativeError> {
    let layout = deck.layout(name).ok_or_else(|| NativeError::Io {
        path: out_dir.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, format!("no layout named {name}")),
    })?;
    let mut layout: Layout = layout.clone();
    std::fs::create_dir_all(out_dir.join(ASSETS_DIR))
        .map_err(|source| NativeError::Io { path: out_dir.to_path_buf(), source })?;
    let mut assets = AssetCopier::new(deck_dir, out_dir);
    for e in &mut layout.elements {
        assets.remap(e)?;
    }
    let path = out_dir.join(LAYOUT_FILE);
    let text =
        serde_json::to_string_pretty(&layout).map_err(|source| NativeError::Json { path: path.clone(), source })?;
    std::fs::write(&path, text + "\n").map_err(|source| NativeError::Io { path, source })
}

/// Loads the layout of a `.deck_layout` folder into `dst`, copying its
/// images, under a name that does not clash. Returns the name it got.
pub fn load_layout(src_dir: &Path, dst: &mut Deck, dst_dir: &Path) -> Result<String, NativeError> {
    let path = src_dir.join(LAYOUT_FILE);
    let text = std::fs::read_to_string(&path).map_err(|source| NativeError::Io { path: path.clone(), source })?;
    let mut layout: Layout = serde_json::from_str(&text).map_err(|source| NativeError::Json { path, source })?;
    let mut assets = AssetCopier::new(src_dir, dst_dir);
    for e in &mut layout.elements {
        assets.remap(e)?;
    }
    let base = layout.name.clone();
    let mut n = 1;
    while dst.layout(&layout.name).is_some() {
        n += 1;
        layout.name = format!("{base} {n}");
    }
    let name = layout.name.clone();
    dst.layouts.push(layout);
    Ok(name)
}

struct AssetCopier<'a> {
    src_dir: &'a Path,
    dst_dir: &'a Path,
    done: HashMap<String, String>,
}

impl<'a> AssetCopier<'a> {
    fn new(src_dir: &'a Path, dst_dir: &'a Path) -> Self {
        Self { src_dir, dst_dir, done: HashMap::new() }
    }

    fn remap(&mut self, element: &mut Element) -> Result<(), NativeError> {
        if let Element::Group(g) = element {
            for c in &mut g.children {
                self.remap(c)?;
            }
            return Ok(());
        }
        match element {
            Element::Image(img) => img.src = self.copy(&img.src)?,
            Element::Media(m) => {
                m.src = self.copy(&m.src)?;
                if let Some(poster) = &m.poster {
                    m.poster = Some(self.copy(poster)?);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The destination path of one asset, copied on first use.
    fn copy(&mut self, src: &str) -> Result<String, NativeError> {
        if let Some(dst) = self.done.get(src) {
            return Ok(dst.clone());
        }
        let path = self.src_dir.join(src);
        let bytes = std::fs::read(&path).map_err(|source| NativeError::Io { path: path.clone(), source })?;
        let name = free_name(self.dst_dir, Path::new(src));
        let dst = native::add_asset(self.dst_dir, &name, &bytes)?;
        self.done.insert(src.to_string(), dst.clone());
        Ok(dst)
    }
}

/// Elements copied to the clipboard: their deck folder lets a paste into
/// another deck bring the images along, while a paste into the same
/// deck reuses them.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Clip {
    pub decker: u32,
    pub dir: String,
    pub elements: Vec<Element>,
}

impl Clip {
    /// The marker the clipboard text starts with.
    pub const PREFIX: &'static str = "{\"decker\":";

    pub fn new(dir: &Path, elements: Vec<Element>) -> Self {
        Self { decker: 1, dir: dir.display().to_string(), elements }
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Parses clipboard text, `None` when it is not a clip.
    pub fn parse(text: &str) -> Option<Self> {
        text.starts_with(Self::PREFIX).then(|| serde_json::from_str(text).ok()).flatten()
    }

    /// The elements for the deck in `dst_dir`, images copied into it
    /// when they come from another folder, each moved by `offset`.
    pub fn take(self, dst_dir: &Path, offset: f64) -> Result<Vec<Element>, NativeError> {
        let src_dir = Path::new(&self.dir);
        let mut elements = self.elements;
        if !self.dir.is_empty() && src_dir != dst_dir {
            let mut assets = AssetCopier::new(src_dir, dst_dir);
            for e in &mut elements {
                assets.remap(e)?;
            }
        }
        for e in &mut elements {
            let mut r = *e.rect();
            r.x += offset;
            r.y += offset;
            e.set_rect(r);
        }
        Ok(elements)
    }
}

/// Saves `deck` under `dst_dir`, bringing its images, `typst/` and `fonts/`
/// files along when it was loaded from another folder (`src_dir`, empty
/// for a deck that never had one). Returns the deck as saved (image
/// paths may change).
pub fn save_as(deck: &Deck, src_dir: &Path, dst_dir: &Path) -> Result<Deck, NativeError> {
    if src_dir == dst_dir {
        native::save(deck, dst_dir)?;
        return Ok(deck.clone());
    }
    let mut copy = deck.clone();
    std::fs::create_dir_all(dst_dir).map_err(|source| NativeError::Io { path: dst_dir.to_path_buf(), source })?;
    if !src_dir.as_os_str().is_empty() {
        let mut assets = AssetCopier::new(src_dir, dst_dir);
        for e in copy.slides.iter_mut().flat_map(|s| s.elements.iter_mut()) {
            assets.remap(e)?;
        }
        for e in copy.layouts.iter_mut().flat_map(|l| l.elements.iter_mut()) {
            assets.remap(e)?;
        }
        for sub in ["typst", "fonts"] {
            let from = src_dir.join(sub);
            let Ok(entries) = std::fs::read_dir(&from) else { continue };
            let to = dst_dir.join(sub);
            std::fs::create_dir_all(&to).map_err(|source| NativeError::Io { path: to.clone(), source })?;
            for entry in entries.flatten().filter(|e| e.path().is_file()) {
                let target = to.join(entry.file_name());
                std::fs::copy(entry.path(), &target).map_err(|source| NativeError::Io { path: target, source })?;
            }
        }
    }
    native::save(&copy, dst_dir)?;
    Ok(copy)
}

/// The pages of an exported document: `(slide, step)` in order, one page
/// per slide at its last step, or one per build step when `steps` is set.
pub fn pages(deck: &Deck, steps: bool) -> Vec<(usize, u32)> {
    deck.slides
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            let last = s.last_step();
            (if steps { 0 } else { last }..=last).map(move |step| (i, step))
        })
        .collect()
}

/// The speaker notes as the Markdown file the viewer reads next to a PDF
/// (`<name>.notes.md`): a `# N` section per page that has notes, a
/// slide's notes repeated on each of its step pages. Empty without notes.
pub fn notes_markdown(deck: &Deck, steps: bool) -> String {
    let mut out = String::new();
    for (n, (slide, _)) in pages(deck, steps).into_iter().enumerate() {
        let notes = deck.slides[slide].notes.trim();
        if !notes.is_empty() {
            // A line starting with `#` and a number would read as a page heading: indent it.
            let body: Vec<String> = notes
                .lines()
                .map(|l| {
                    let t = l.trim_start_matches('#').trim();
                    if l.starts_with('#') && t.parse::<usize>().is_ok() {
                        format!(" {l}")
                    } else {
                        l.to_string()
                    }
                })
                .collect();
            out.push_str(&format!("# {}\n{}\n\n", n + 1, body.join("\n")));
        }
    }
    out
}

/// Removes the files of `<dir>/assets/` that no element of `deck` (slides,
/// layouts, groups included) references; returns their names.
pub fn prune_assets(deck: &Deck, dir: &Path) -> Result<Vec<String>, NativeError> {
    let mut used = std::collections::HashSet::new();
    fn collect(e: &Element, used: &mut std::collections::HashSet<String>) {
        match e {
            Element::Image(i) => {
                used.insert(i.src.clone());
            }
            Element::Media(m) => {
                used.insert(m.src.clone());
                used.extend(m.poster.clone());
            }
            Element::Group(g) => g.children.iter().for_each(|c| collect(c, used)),
            _ => {}
        }
    }
    deck.slides.iter().flat_map(|s| &s.elements).for_each(|e| collect(e, &mut used));
    deck.layouts.iter().flat_map(|l| &l.elements).for_each(|e| collect(e, &mut used));
    let assets = dir.join(ASSETS_DIR);
    let Ok(entries) = std::fs::read_dir(&assets) else { return Ok(vec![]) };
    let mut removed = vec![];
    for entry in entries.flatten().filter(|e| e.path().is_file()) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !used.contains(&format!("{ASSETS_DIR}/{name}")) {
            std::fs::remove_file(entry.path()).map_err(|source| NativeError::Io { path: entry.path(), source })?;
            removed.push(name);
        }
    }
    removed.sort();
    Ok(removed)
}

/// Adds `bytes` to the deck's assets as `name` (made unique) and returns
/// the deck-relative path.
pub fn add_image(dst_dir: &Path, name: &str, bytes: &[u8]) -> Result<String, NativeError> {
    let name = free_name(dst_dir, Path::new(name));
    native::add_asset(dst_dir, &name, bytes)
}

/// `stem.ext`, or `stem-2.ext`, `stem-3.ext`… if taken in the assets folder.
fn free_name(dst_dir: &Path, src: &Path) -> String {
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "asset".into());
    let ext = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let assets = dst_dir.join(ASSETS_DIR);
    let mut name = format!("{stem}{ext}");
    let mut n = 1;
    while assets.join(&name).exists() {
        n += 1;
        name = format!("{stem}-{n}{ext}");
    }
    name
}

/// The pixel size of a PNG, JPEG, GIF or WebP image from its header.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let be32 = |i: usize| bytes.get(i..i + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let be16 = |i: usize| bytes.get(i..i + 2).map(|b| u32::from(u16::from_be_bytes([b[0], b[1]])));
    let le16 = |i: usize| bytes.get(i..i + 2).map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])));
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be32(16)?, be32(20)?));
    }
    if bytes.starts_with(b"GIF8") {
        return Some((le16(6)?, le16(8)?));
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return match bytes.get(12..16)? {
            b"VP8 " => Some((le16(26)? & 0x3FFF, le16(28)? & 0x3FFF)),
            b"VP8L" => {
                let b = bytes.get(21..25)?;
                let bits = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
            }
            b"VP8X" => {
                let w = bytes.get(24..27)?;
                let h = bytes.get(27..30)?;
                Some((u32::from_le_bytes([w[0], w[1], w[2], 0]) + 1, u32::from_le_bytes([h[0], h[1], h[2], 0]) + 1))
            }
            _ => None,
        };
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        // JPEG: walk the segments to the first start-of-frame.
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                return None;
            }
            let marker = bytes[i + 1];
            if marker == 0xFF {
                i += 1;
                continue;
            }
            let len = be16(i + 2)? as usize;
            if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                return Some((be16(i + 7)?, be16(i + 5)?));
            }
            i += 2 + len;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn saves_and_loads_a_layout_folder() {
        let tmp = std::env::temp_dir().join(format!("decker-layoutfile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (src_dir, lib_dir, dst_dir) = (tmp.join("src.deck"), tmp.join("Corp.deck_layout"), tmp.join("dst.deck"));
        let mut src = Deck::new("src");
        let logo = native::add_asset(&src_dir, "logo.png", b"logo").unwrap();
        src.layouts.push(Layout {
            name: "Corp".into(),
            elements: vec![Element::Image(Image { rect: Rect::default(), src: logo, ..Default::default() })],
            placeholders: vec![Placeholder { name: "title".into(), ..Default::default() }],
            ..Default::default()
        });
        save_layout(&src, &src_dir, "Corp", &lib_dir).unwrap();
        assert!(lib_dir.join(LAYOUT_FILE).is_file());
        assert!(lib_dir.join("assets/logo.png").is_file());
        assert!(save_layout(&src, &src_dir, "nope", &tmp.join("x.deck_layout")).is_err());

        let mut dst = Deck::new("dst");
        dst.layouts.push(Layout { name: "Corp".into(), ..Default::default() });
        let name = load_layout(&lib_dir, &mut dst, &dst_dir).unwrap();
        assert_eq!(name, "Corp 2", "a clashing name is suffixed");
        assert_eq!(dst.layouts.len(), 2);
        let Element::Image(img) = &dst.layouts[1].elements[0] else { panic!() };
        assert_eq!(std::fs::read(dst_dir.join(&img.src)).unwrap(), b"logo");
        assert_eq!(dst.layouts[1].placeholders[0].name, "title");
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn copies_slides_images_and_layouts() {
        let tmp = std::env::temp_dir().join(format!("decker-transfer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (src_dir, dst_dir) = (tmp.join("src.deck"), tmp.join("dst.deck"));
        let mut src = Deck::new("src");
        let logo = native::add_asset(&src_dir, "image1.png", b"logo").unwrap();
        let pic = native::add_asset(&src_dir, "image2.png", b"pic").unwrap();
        src.layouts.push(Layout {
            name: "Corp".into(),
            elements: vec![Element::Image(Image { rect: Rect::default(), src: logo, ..Default::default() })],
            ..Default::default()
        });
        src.slides.push(Slide { name: "a".into(), ..Default::default() });
        src.slides.push(Slide {
            name: "b".into(),
            layout: Some("Corp".into()),
            elements: vec![Element::Image(Image { rect: Rect::default(), src: pic, ..Default::default() })],
            ..Default::default()
        });
        src.slides.push(Slide { name: "c".into(), layout: Some("Corp".into()), ..Default::default() });

        let mut dst = Deck::new("dst");
        // The destination already has an image1.png: the copy must not clobber it.
        native::add_asset(&dst_dir, "image1.png", b"other").unwrap();
        dst.slides.push(Slide { name: "x".into(), ..Default::default() });

        let n = copy_slides(&src, &src_dir, &[1, 2, 99], &mut dst, &dst_dir, 0).unwrap();
        assert_eq!(n, 2);
        assert_eq!(dst.slides.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["b", "c", "x"]);
        assert_eq!(dst.layouts.len(), 1, "the layout came along once");
        let Element::Image(logo) = &dst.layouts[0].elements[0] else { panic!() };
        assert_eq!(logo.src, "assets/image1-2.png");
        assert_eq!(std::fs::read(dst_dir.join(&logo.src)).unwrap(), b"logo");
        assert_eq!(std::fs::read(dst_dir.join("assets/image1.png")).unwrap(), b"other");
        let Element::Image(pic) = &dst.slides[0].elements[0] else { panic!() };
        assert_eq!(pic.src, "assets/image2.png");
        assert_eq!(std::fs::read(dst_dir.join(&pic.src)).unwrap(), b"pic");
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn save_as_brings_assets_and_sniffs_sizes() {
        let base = std::env::temp_dir().join(format!("decker-saveas-{}", std::process::id()));
        let (a, b) = (base.join("a.deck"), base.join("b.deck"));
        let _ = std::fs::remove_dir_all(&base);
        let mut deck = Deck::new("t");
        let png = [
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 1, 0, 0, 0,
            0, 0x40,
        ];
        assert_eq!(image_size(&png), Some((256, 64)));
        assert_eq!(image_size(b"GIF89a\x10\x00\x20\x00"), Some((16, 32)));
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 2, 0xFF, 0xC0, 0, 11, 8, 0, 100, 0, 200, 3, 0, 0, 0, 0, 0];
        assert_eq!(image_size(&jpeg), Some((200, 100)));
        assert_eq!(image_size(b"nope"), None);
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(a.join("fonts")).unwrap();
        std::fs::write(a.join("fonts/x.ttf"), b"f").unwrap();
        let src = add_image(&a, "shot.png", &png).unwrap();
        assert_eq!(src, "assets/shot.png");
        assert_eq!(add_image(&a, "shot.png", &png).unwrap(), "assets/shot-2.png");
        deck.slides.push(Slide {
            elements: vec![Element::Image(Image { rect: Rect::new(0.0, 0.0, 10.0, 10.0), src, ..Default::default() })],
            ..Default::default()
        });
        let saved = save_as(&deck, &a, &b).unwrap();
        assert!(b.join("assets/shot.png").is_file());
        assert!(b.join("fonts/x.ttf").is_file());
        assert_eq!(native::load(&b).unwrap(), saved);
        // A deck without a folder yet saves without copying anything.
        let c = base.join("c.deck");
        save_as(&Deck::new("n"), Path::new(""), &c).unwrap();
        assert!(c.join("deck.json").is_file());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn clips_round_trip_and_copy_foreign_images() {
        let base = std::env::temp_dir().join(format!("decker-clip-{}", std::process::id()));
        let (a, b) = (base.join("a.deck"), base.join("b.deck"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&a).unwrap();
        let src = add_image(&a, "pic.png", b"png").unwrap();
        let img =
            Element::Image(Image { rect: Rect::new(10.0, 10.0, 50.0, 50.0), src: src.clone(), ..Default::default() });
        let clip = Clip::new(&a, vec![img, Element::Text(TextBox::plain(Rect::new(0.0, 0.0, 10.0, 10.0), "t"))]);
        let text = clip.to_text();
        assert!(text.starts_with(Clip::PREFIX));
        assert_eq!(Clip::parse(&text), Some(clip.clone()));
        assert_eq!(Clip::parse("hello"), None);
        // Same deck: the file is reused, the copies are offset.
        let same = clip.clone().take(&a, 20.0).unwrap();
        assert_eq!(same[0].rect(), &Rect::new(30.0, 30.0, 50.0, 50.0));
        assert!(matches!(&same[0], Element::Image(i) if i.src == src));
        assert_eq!(std::fs::read_dir(a.join("assets")).unwrap().count(), 1);
        // Another deck: the file is copied once.
        std::fs::create_dir_all(&b).unwrap();
        let other = clip.take(&b, 0.0).unwrap();
        assert!(matches!(&other[0], Element::Image(i) if i.src == "assets/pic.png"));
        assert!(b.join("assets/pic.png").is_file());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn prunes_unreferenced_assets() {
        let dir = std::env::temp_dir().join(format!("decker-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let kept = add_image(&dir, "kept.png", b"a").unwrap();
        let grouped = add_image(&dir, "grouped.png", b"b").unwrap();
        add_image(&dir, "orphan.png", b"c").unwrap();
        let mut deck = Deck::new("t");
        deck.slides.push(Slide {
            elements: vec![Element::Group(Group::of(vec![Element::Image(Image {
                src: grouped,
                ..Default::default()
            })]))],
            ..Default::default()
        });
        deck.layouts.push(Layout {
            name: "L".into(),
            elements: vec![Element::Image(Image { src: kept, ..Default::default() })],
            ..Default::default()
        });
        assert_eq!(prune_assets(&deck, &dir).unwrap(), ["orphan.png"]);
        assert!(dir.join("assets/kept.png").is_file() && dir.join("assets/grouped.png").is_file());
        assert_eq!(prune_assets(&Deck::new("x"), Path::new("/nonexistent")).unwrap(), Vec::<String>::new());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pages_and_notes_follow_the_build_steps() {
        let mut deck = Deck::new("t");
        let mut stepped = Shape { rect: Rect::new(0.0, 0.0, 1.0, 1.0), ..Default::default() };
        stepped.steps = vec![Step::disappear(0), Step::appear(2)];
        deck.slides.push(Slide { notes: "first".into(), ..Default::default() });
        deck.slides.push(Slide {
            elements: vec![Element::Shape(stepped)],
            notes: "second\n# 3".into(),
            ..Default::default()
        });
        deck.slides.push(Slide::default());
        assert_eq!(pages(&deck, false), [(0, 0), (1, 2), (2, 0)]);
        assert_eq!(pages(&deck, true), [(0, 0), (1, 0), (1, 1), (1, 2), (2, 0)]);
        assert_eq!(
            notes_markdown(&deck, false),
            "# 1\nfirst\n\n# 2\nsecond\n # 3\n\n",
            "a bare page-number line is indented so it is not read as a heading"
        );
        let stepped = notes_markdown(&deck, true);
        assert!(stepped.contains("# 2\nsecond") && stepped.contains("# 4\nsecond") && !stepped.contains("# 5"));
        assert_eq!(notes_markdown(&Deck::new("x"), true), "");
    }
}
