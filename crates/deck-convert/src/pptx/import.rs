//! PPTX → deck.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use roxmltree::{Document, Node};

use deck_core::*;

use super::package::Package;
use super::*;

/// What the import skipped, for the user.
#[derive(Debug, Default)]
pub struct ImportReport {
    pub warnings: Vec<String>,
}

/// Converts `pptx` into a deck folder at `out` (created). Images are copied
/// into `out/assets/`.
pub fn import(pptx: &Path, out: &Path) -> Result<(Deck, ImportReport)> {
    let pkg = Package::open(pptx)?;
    let mut report = ImportReport::default();

    let pres_part = pkg
        .rel_by_type("[Content_Types].xml", REL_OFFICE_DOC)
        .ok()
        .flatten()
        .or_else(|| pkg.rels("").ok()?.into_iter().find(|r| r.kind == REL_OFFICE_DOC).map(|r| r.target))
        .unwrap_or_else(|| "ppt/presentation.xml".to_string());
    let pres_xml = pkg.xml(&pres_part)?;
    let pres = Document::parse(&pres_xml).context("parsing presentation.xml")?;
    let root = pres.root_element();

    let mut deck = Deck::new(pptx.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    if let Some(sz) = child(root, NS_P, "sldSz") {
        deck.width = emu_attr(sz, "cx").unwrap_or(deck.width);
        deck.height = emu_attr(sz, "cy").unwrap_or(deck.height);
    }

    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let mut images = ImageStore { pkg: &pkg, out, copied: HashMap::new(), next: 1 };

    let ids: Vec<String> = child(root, NS_P, "sldIdLst")
        .map(|l| {
            l.children()
                .filter(|n| n.has_tag_name((NS_P, "sldId")))
                .filter_map(|n| n.attribute((NS_R, "id")).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    for id in ids {
        let part = pkg.rel_by_id(&pres_part, &id)?;
        let slide = import_slide(&pkg, &part, &mut images, &mut report).with_context(|| format!("importing {part}"))?;
        deck.slides.push(slide);
    }

    native::save(&deck, out)?;
    Ok((deck, report))
}

struct ImageStore<'a> {
    pkg: &'a Package,
    out: &'a Path,
    copied: HashMap<String, String>,
    next: u32,
}

impl ImageStore<'_> {
    fn copy(&mut self, part: &str) -> Result<String> {
        if let Some(src) = self.copied.get(part) {
            return Ok(src.clone());
        }
        let ext =
            Path::new(part).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "bin".into());
        let name = format!("image{}.{ext}", self.next);
        self.next += 1;
        let src = native::add_asset(self.out, &name, self.pkg.part(part)?)?;
        self.copied.insert(part.to_string(), src.clone());
        Ok(src)
    }
}

fn import_slide(pkg: &Package, part: &str, images: &mut ImageStore, report: &mut ImportReport) -> Result<Slide> {
    let xml = pkg.xml(part)?;
    let doc = Document::parse(&xml).with_context(|| format!("parsing {part}"))?;
    let layout_part = pkg.rel_by_type(part, REL_LAYOUT)?;
    let layout_xml = layout_part.as_deref().map(|p| pkg.xml(p)).transpose()?;
    let layout = layout_xml.as_deref().map(Document::parse).transpose()?;
    let master_part = layout_part.as_deref().map(|p| pkg.rel_by_type(p, REL_MASTER)).transpose()?.flatten();
    let master_xml = master_part.as_deref().map(|p| pkg.xml(p)).transpose()?;
    let master = master_xml.as_deref().map(Document::parse).transpose()?;

    let mut slide = Slide::default();
    let csld = child(doc.root_element(), NS_P, "cSld").ok_or_else(|| anyhow!("no cSld"))?;
    slide.name = csld.attribute("name").unwrap_or_default().to_string();
    if let Some(bg) = child(csld, NS_P, "bg").and_then(|b| child(b, NS_P, "bgPr")).and_then(solid_fill) {
        slide.background = Some(bg);
    }
    let tree = child(csld, NS_P, "spTree").ok_or_else(|| anyhow!("no spTree"))?;
    let inherit = |node: Node| -> Option<Rect> {
        let ph = placeholder(node)?;
        [layout.as_ref(), master.as_ref()].into_iter().flatten().find_map(|d| placeholder_rect(d.root_element(), &ph))
    };
    import_tree(tree, part, pkg, images, report, &inherit, &mut slide.elements, Rect::default())?;

    if let Some(notes_part) = pkg.rel_by_type(part, REL_NOTES)? {
        let notes_xml = pkg.xml(&notes_part)?;
        let notes = Document::parse(&notes_xml).with_context(|| format!("parsing {notes_part}"))?;
        slide.notes = notes
            .descendants()
            .filter(|n| n.has_tag_name((NS_P, "sp")))
            .filter(|sp| placeholder(*sp).map(|p| p.kind == "body").unwrap_or(false))
            .filter_map(|sp| child(sp, NS_P, "txBody"))
            .flat_map(|tb| tb.children().filter(|n| n.has_tag_name((NS_A, "p"))).map(|p| paragraph_text(p)))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
    }
    Ok(slide)
}

#[allow(clippy::too_many_arguments)]
fn import_tree(
    tree: Node,
    part: &str,
    pkg: &Package,
    images: &mut ImageStore,
    report: &mut ImportReport,
    inherit: &dyn Fn(Node) -> Option<Rect>,
    elements: &mut Vec<Element>,
    offset: Rect,
) -> Result<()> {
    for node in tree.children().filter(Node::is_element) {
        let tag = node.tag_name().name();
        match tag {
            "sp" => {
                if let Some(e) = import_shape(node, inherit, offset, report) {
                    elements.push(e);
                }
            }
            "pic" => {
                let Some(rect) = xfrm(node).map(|r| shift(r, offset)) else {
                    report.warnings.push(format!("{part}: picture without position skipped"));
                    continue;
                };
                let embed = node
                    .descendants()
                    .find(|n| n.has_tag_name((NS_A, "blip")))
                    .and_then(|b| b.attribute((NS_R, "embed")));
                match embed.and_then(|id| pkg.rel_by_id(part, id).ok()) {
                    Some(media)
                        if media.ends_with(".png")
                            || media.ends_with(".jpg")
                            || media.ends_with(".jpeg")
                            || media.ends_with(".gif")
                            || media.ends_with(".svg")
                            || media.ends_with(".bmp")
                            || media.ends_with(".webp") =>
                    {
                        elements.push(Element::Image(Image { rect, src: images.copy(&media)? }));
                    }
                    Some(media) => report.warnings.push(format!("{part}: unsupported media {media} skipped")),
                    None => report.warnings.push(format!("{part}: picture without image skipped")),
                }
            }
            "grpSp" => {
                // Child coordinates are in the group's own space; map them
                // onto the slide with the group's transform.
                let group_offset = group_offset(node, offset);
                import_tree(node, part, pkg, images, report, inherit, elements, group_offset)?;
            }
            "nvGrpSpPr" | "grpSpPr" => {}
            "graphicFrame" => report.warnings.push(format!("{part}: table, chart or diagram skipped")),
            "cxnSp" => {
                if let Some(rect) = xfrm(node).map(|r| shift(r, offset)) {
                    let sp_pr = child(node, NS_P, "spPr");
                    elements.push(Element::Shape(Shape {
                        rect,
                        kind: ShapeKind::Line,
                        stroke: sp_pr.and_then(stroke).or(Some(Stroke { color: Color::BLACK, width: 1.0 })),
                        ..Default::default()
                    }));
                }
            }
            other => report.warnings.push(format!("{part}: element {other} skipped")),
        }
    }
    Ok(())
}

/// Group transform: children at `chOff`/`chExt` map onto `off`/`ext`.
/// Returned as a Rect used by [`shift`]: x,y = translation, w,h = scale.
fn group_offset(group: Node, parent: Rect) -> Rect {
    let Some(x) = child(group, NS_P, "grpSpPr").and_then(|p| child(p, NS_A, "xfrm")) else { return parent };
    let off = child(x, NS_A, "off");
    let ext = child(x, NS_A, "ext");
    let ch_off = child(x, NS_A, "chOff");
    let ch_ext = child(x, NS_A, "chExt");
    let get = |n: Option<Node>, a: &str| n.and_then(|n| emu_attr(n, a));
    let (ox, oy) = (get(off, "x").unwrap_or(0.0), get(off, "y").unwrap_or(0.0));
    let (ew, eh) = (get(ext, "cx").unwrap_or(0.0), get(ext, "cy").unwrap_or(0.0));
    let (cx, cy) = (get(ch_off, "x").unwrap_or(0.0), get(ch_off, "y").unwrap_or(0.0));
    let (cw, ch) = (get(ch_ext, "cx").unwrap_or(ew), get(ch_ext, "cy").unwrap_or(eh));
    let sx = if cw > 0.0 { ew / cw } else { 1.0 };
    let sy = if ch > 0.0 { eh / ch } else { 1.0 };
    let (psx, psy) = scale(parent);
    Rect { x: parent.x + (ox - cx * sx) * psx, y: parent.y + (oy - cy * sy) * psy, w: sx * psx, h: sy * psy }
}

fn scale(o: Rect) -> (f64, f64) {
    (if o.w == 0.0 { 1.0 } else { o.w }, if o.h == 0.0 { 1.0 } else { o.h })
}

fn shift(r: Rect, o: Rect) -> Rect {
    let (sx, sy) = scale(o);
    Rect { x: o.x + r.x * sx, y: o.y + r.y * sy, w: r.w * sx, h: r.h * sy }
}

fn import_shape(
    node: Node,
    inherit: &dyn Fn(Node) -> Option<Rect>,
    offset: Rect,
    report: &mut ImportReport,
) -> Option<Element> {
    let rect = match xfrm(node).or_else(|| inherit(node)) {
        Some(r) => shift(r, offset),
        None => {
            report.warnings.push(format!("shape {:?} without position skipped", shape_name(node)));
            return None;
        }
    };
    let sp_pr = child(node, NS_P, "spPr");
    let fill = sp_pr.and_then(solid_fill);
    let stroke = sp_pr.and_then(stroke);
    let geom = sp_pr.and_then(|p| child(p, NS_A, "prstGeom")).and_then(|g| g.attribute("prst")).unwrap_or("rect");
    let ph = placeholder(node);
    let is_body = ph.as_ref().map(|p| p.kind == "body" || p.kind == "obj").unwrap_or(false);
    let mut paragraphs = child(node, NS_P, "txBody")
        .map(|tb| if is_body { body_paragraphs(tb) } else { paragraphs(tb) })
        .unwrap_or_default();
    // Placeholder sizes come from the master's text styles, which are not
    // read yet: give titles and subtitles PowerPoint's usual defaults.
    if let Some(ph) = &ph {
        let default = match ph.kind.as_str() {
            "title" | "ctrTitle" => Some(44.0),
            "subTitle" => Some(24.0),
            _ => None,
        };
        if let Some(size) = default {
            for run in paragraphs.iter_mut().flat_map(|p| p.runs.iter_mut()) {
                run.size.get_or_insert(size);
            }
        }
    }
    let is_text_box = child(node, NS_P, "nvSpPr")
        .and_then(|n| child(n, NS_P, "cNvSpPr"))
        .and_then(|n| n.attribute("txBox"))
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
        || ph.is_some();
    if is_text_box && geom == "rect" {
        return Some(Element::Text(TextBox { rect, paragraphs, fill, stroke }));
    }
    let kind = match geom {
        "ellipse" => ShapeKind::Ellipse,
        "line" | "straightConnector1" => ShapeKind::Line,
        "rect" | "roundRect" | "snip1Rect" | "snip2SameRect" | "snip2DiagRect" | "snipRoundRect" | "round1Rect"
        | "round2SameRect" | "round2DiagRect" | "flowChartProcess" => ShapeKind::Rectangle,
        other => {
            report.warnings.push(format!("shape {:?}: geometry {other} drawn as a rectangle", shape_name(node)));
            ShapeKind::Rectangle
        }
    };
    if fill.is_none() && stroke.is_none() && paragraphs.iter().all(|p| p.text().is_empty()) {
        return None;
    }
    Some(Element::Shape(Shape { rect, kind, fill, stroke, paragraphs }))
}

fn shape_name(node: Node) -> String {
    node.descendants()
        .find(|n| n.has_tag_name((NS_P, "cNvPr")))
        .and_then(|n| n.attribute("name"))
        .unwrap_or("")
        .to_string()
}

#[derive(Debug, Clone, PartialEq)]
struct Placeholder {
    kind: String,
    idx: Option<String>,
}

fn placeholder(sp: Node) -> Option<Placeholder> {
    let ph = child(sp, NS_P, "nvSpPr").and_then(|n| child(n, NS_P, "nvPr")).and_then(|n| child(n, NS_P, "ph"))?;
    Some(Placeholder {
        kind: ph.attribute("type").unwrap_or("body").to_string(),
        idx: ph.attribute("idx").map(str::to_string),
    })
}

/// Finds the layout/master shape matching a placeholder: same idx when the
/// slide gives one, else same type. Titles match `title` or `ctrTitle`.
fn placeholder_rect(root: Node, ph: &Placeholder) -> Option<Rect> {
    let shapes: Vec<(Placeholder, Node)> = root
        .descendants()
        .filter(|n| n.has_tag_name((NS_P, "sp")))
        .filter_map(|n| placeholder(n).map(|p| (p, n)))
        .collect();
    let is_title = |k: &str| k == "title" || k == "ctrTitle";
    let found = shapes
        .iter()
        .find(|(p, _)| ph.idx.is_some() && p.idx == ph.idx)
        .or_else(|| shapes.iter().find(|(p, _)| p.kind == ph.kind))
        .or_else(|| if is_title(&ph.kind) { shapes.iter().find(|(p, _)| is_title(&p.kind)) } else { None })
        .or_else(|| {
            if ph.kind == "body" || ph.kind == "obj" {
                shapes.iter().find(|(p, _)| p.kind == "body" || p.kind == "obj")
            } else {
                None
            }
        });
    found.and_then(|(_, n)| xfrm(*n))
}

fn xfrm(node: Node) -> Option<Rect> {
    let x = node
        .children()
        .find(|n| n.has_tag_name((NS_P, "spPr")) || n.has_tag_name((NS_P, "grpSpPr")))
        .and_then(|p| child(p, NS_A, "xfrm"))?;
    let off = child(x, NS_A, "off")?;
    let ext = child(x, NS_A, "ext")?;
    Some(Rect { x: emu_attr(off, "x")?, y: emu_attr(off, "y")?, w: emu_attr(ext, "cx")?, h: emu_attr(ext, "cy")? })
}

fn solid_fill(node: Node) -> Option<Color> {
    let fill = child(node, NS_A, "solidFill")?;
    srgb(fill)
}

fn srgb(node: Node) -> Option<Color> {
    child(node, NS_A, "srgbClr").and_then(|c| c.attribute("val")).and_then(|v| Color::from_hex(v).ok())
}

fn stroke(sp_pr: Node) -> Option<Stroke> {
    let ln = child(sp_pr, NS_A, "ln")?;
    if child(ln, NS_A, "noFill").is_some() {
        return None;
    }
    let color = solid_fill(ln)?;
    let width = ln.attribute("w").and_then(|w| w.parse::<f64>().ok()).map(|w| w / EMU_PER_PT).unwrap_or(0.75);
    Some(Stroke { color, width })
}

fn paragraphs(tx_body: Node) -> Vec<Paragraph> {
    tx_body.children().filter(|n| n.has_tag_name((NS_A, "p"))).map(|p| paragraph(p).0).collect()
}

/// Body placeholders are bulleted at every level by the master's text
/// styles: give them a bullet unless the paragraph opts out with `buNone`.
fn body_paragraphs(tx_body: Node) -> Vec<Paragraph> {
    tx_body
        .children()
        .filter(|n| n.has_tag_name((NS_A, "p")))
        .map(|p| {
            let (mut para, no_bullet) = paragraph(p);
            if !no_bullet && para.bullet.is_none() && !para.text().is_empty() {
                para.bullet = Some(0);
            }
            para
        })
        .collect()
}

/// A paragraph and whether it explicitly refuses a bullet (`buNone`).
fn paragraph(p: Node) -> (Paragraph, bool) {
    let p_pr = child(p, NS_A, "pPr");
    let align = match p_pr.and_then(|n| n.attribute("algn")) {
        Some("ctr") => Align::Center,
        Some("r") => Align::Right,
        Some("just") => Align::Justify,
        _ => Align::Left,
    };
    let level = p_pr.and_then(|n| n.attribute("lvl")).and_then(|l| l.parse::<u8>().ok()).unwrap_or(0);
    let no_bullet = p_pr.map(|n| child(n, NS_A, "buNone").is_some()).unwrap_or(false);
    let explicit_bullet =
        p_pr.map(|n| child(n, NS_A, "buChar").is_some() || child(n, NS_A, "buAutoNum").is_some()).unwrap_or(false);
    // Body placeholders are bulleted by the master's text styles; a level
    // above 0 or an explicit bullet char is the signal we keep.
    let bullet = if no_bullet {
        None
    } else if explicit_bullet || level > 0 {
        Some(level)
    } else {
        None
    };
    let mut runs = vec![];
    for n in p.children() {
        if n.has_tag_name((NS_A, "r")) {
            let text = child(n, NS_A, "t").and_then(|t| t.text()).unwrap_or("").to_string();
            let r_pr = child(n, NS_A, "rPr");
            let flag = |a: &str| r_pr.and_then(|r| r.attribute(a)).map(|v| v == "1" || v == "true").unwrap_or(false);
            runs.push(Run {
                text,
                bold: flag("b"),
                italic: flag("i"),
                size: r_pr.and_then(|r| r.attribute("sz")).and_then(|s| s.parse::<f64>().ok()).map(|s| s / 100.0),
                color: r_pr.and_then(solid_fill),
                font: r_pr
                    .and_then(|r| child(r, NS_A, "latin"))
                    .and_then(|l| l.attribute("typeface"))
                    .map(str::to_string),
            });
        } else if n.has_tag_name((NS_A, "br")) {
            runs.push(Run::plain("\n"));
        } else if n.has_tag_name((NS_A, "fld")) {
            let text = child(n, NS_A, "t").and_then(|t| t.text()).unwrap_or("").to_string();
            runs.push(Run::plain(text));
        }
    }
    (Paragraph { runs, align, bullet }, no_bullet)
}

fn paragraph_text(p: Node) -> String {
    paragraph(p).0.text()
}

fn child<'a>(node: Node<'a, 'a>, ns: &str, name: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|n| n.has_tag_name((ns, name)))
}

fn emu_attr(node: Node, attr: &str) -> Option<f64> {
    node.attribute(attr)?.parse::<f64>().ok().map(|v| v / EMU_PER_PT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/fixture.pptx")
    }

    #[test]
    fn imports_the_fixture() {
        let out = std::env::temp_dir().join(format!("decker-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let (deck, report) = import(&fixture(), &out).unwrap();
        assert_eq!(deck.slides.len(), 2);
        assert!((deck.width - 960.0).abs() < 0.1, "width {}", deck.width);
        assert!((deck.height - 540.0).abs() < 0.1);

        // Slide 1: title and subtitle placeholders inherit their position.
        let s1 = &deck.slides[0];
        let texts: Vec<&TextBox> =
            s1.elements.iter().filter_map(|e| if let Element::Text(t) = e { Some(t) } else { None }).collect();
        assert_eq!(texts.len(), 2, "{:?}", s1.elements);
        assert_eq!(texts[0].text(), "Decker fixture");
        assert_eq!(texts[0].paragraphs[0].runs[0].size, Some(44.0));
        assert!(texts[0].rect.w > 100.0 && texts[0].rect.h > 10.0, "inherited rect {:?}", texts[0].rect);
        assert!(texts[1].rect.y > texts[0].rect.y);
        assert_eq!(texts[1].text(), "Subtitle & <xml>");
        assert_eq!(s1.notes, "Speaker notes one");

        // Slide 2: bullets, runs, picture, shape, free text box.
        let s2 = &deck.slides[1];
        let body = s2.elements.iter().find_map(|e| match e {
            Element::Text(t) if t.paragraphs.len() == 3 => Some(t),
            _ => None,
        });
        let body = body.expect("body placeholder");
        assert_eq!(body.paragraphs[0].bullet, Some(0));
        assert_eq!(body.paragraphs[1].bullet, Some(1));
        let runs = &body.paragraphs[2].runs;
        assert_eq!(runs[0].text, "Bold red");
        assert!(runs[0].bold);
        assert_eq!(runs[0].size, Some(28.0));
        assert_eq!(runs[0].color, Some(Color::rgb(0xCC, 0, 0)));
        assert!(runs[1].italic);
        let img =
            s2.elements.iter().find_map(|e| if let Element::Image(i) = e { Some(i) } else { None }).expect("image");
        assert_eq!(img.src, "assets/image1.png");
        assert!((img.rect.x - 648.0).abs() < 0.1);
        assert!(out.join("assets/image1.png").is_file());
        let oval =
            s2.elements.iter().find_map(|e| if let Element::Shape(s) = e { Some(s) } else { None }).expect("shape");
        assert_eq!(oval.kind, ShapeKind::Ellipse);
        assert_eq!(oval.fill, Some(Color::rgb(0, 0x80, 0xFF)));
        assert_eq!(oval.paragraphs[0].text(), "oval");
        let free = s2.elements.iter().find_map(|e| match e {
            Element::Text(t) if t.text() == "Free text box" => Some(t),
            _ => None,
        });
        assert_eq!(free.unwrap().paragraphs[0].align, Align::Center);
        assert!(s2.notes.is_empty());
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(native::load(&out).is_ok());
        std::fs::remove_dir_all(&out).unwrap();
    }
}
