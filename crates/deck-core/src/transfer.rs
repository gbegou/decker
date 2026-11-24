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
        let Element::Image(img) = element else { return Ok(()) };
        if let Some(dst) = self.done.get(&img.src) {
            img.src = dst.clone();
            return Ok(());
        }
        let path = self.src_dir.join(&img.src);
        let bytes = std::fs::read(&path).map_err(|source| NativeError::Io { path: path.clone(), source })?;
        let name = free_name(self.dst_dir, Path::new(&img.src));
        let dst = native::add_asset(self.dst_dir, &name, &bytes)?;
        self.done.insert(img.src.clone(), dst.clone());
        img.src = dst;
        Ok(())
    }
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
}
