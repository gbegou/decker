//! Deck → PPTX. Writes a minimal package: one master, one blank layout,
//! and one slide per deck slide with explicit shapes.

use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use zip::write::SimpleFileOptions;

use deck_core::*;

use super::*;

/// Writes `deck` to `pptx`. Returns warnings about what could not be
/// expressed in PPTX (formulas, step overrides other than appear/disappear).
pub fn export(deck: &Deck, deck_dir: &Path, pptx: &Path) -> Result<Vec<String>> {
    export_with(deck, deck_dir, pptx, &deck_core::NoMath)
}

/// [`export`] with a typesetter: formulas and figures are written as
/// pictures of what it renders (and still carry their source, so that
/// importing the file brings them back), instead of their source text.
pub fn export_with(deck: &Deck, deck_dir: &Path, pptx: &Path, math: &dyn MathRenderer) -> Result<Vec<String>> {
    let mut warnings = vec![];
    let file = std::fs::File::create(pptx).with_context(|| format!("creating {}", pptx.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut add = |name: &str, data: &[u8]| -> Result<()> {
        zip.start_file(name, opts)?;
        zip.write_all(data)?;
        Ok(())
    };

    let n = deck.slides.len();
    let cx = emu(deck.width);
    let cy = emu(deck.height);

    let mut ct = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="mp4" ContentType="video/mp4"/><Default Extension="m4v" ContentType="video/x-m4v"/><Default Extension="mov" ContentType="video/quicktime"/><Default Extension="webm" ContentType="video/webm"/><Default Extension="ogv" ContentType="video/ogg"/><Default Extension="jpg" ContentType="image/jpeg"/><Default Extension="jpeg" ContentType="image/jpeg"/><Default Extension="gif" ContentType="image/gif"/><Default Extension="bmp" ContentType="image/bmp"/><Default Extension="svg" ContentType="image/svg+xml"/><Default Extension="webp" ContentType="image/webp"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#,
    );
    // Layout k (0-based) is slideLayout{k+1}; the blank layout, for slides
    // without one, is the last.
    let n_layouts = deck.layouts.len() + 1;
    for l in 1..=n_layouts {
        let _ = write!(
            ct,
            r#"<Override PartName="/ppt/slideLayouts/slideLayout{l}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>"#
        );
    }
    for i in 1..=n {
        let _ = write!(
            ct,
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        );
        if !deck.slides[i - 1].notes.is_empty() {
            let _ = write!(
                ct,
                r#"<Override PartName="/ppt/notesSlides/notesSlide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>"#
            );
        }
    }
    if deck.slides.iter().any(|s| !s.notes.is_empty()) {
        ct.push_str(r#"<Override PartName="/ppt/notesMasters/notesMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"/>"#);
    }
    ct.push_str("</Types>");
    add("[Content_Types].xml", ct.as_bytes())?;

    add("_rels/.rels", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="{REL_OFFICE_DOC}" Target="ppt/presentation.xml"/></Relationships>"#).as_bytes())?;

    let has_notes = deck.slides.iter().any(|s| !s.notes.is_empty());
    let mut pres = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rIdM"/></p:sldMasterIdLst>"#
    );
    if has_notes {
        pres.push_str(r#"<p:notesMasterIdLst><p:notesMasterId r:id="rIdN"/></p:notesMasterIdLst>"#);
    }
    pres.push_str("<p:sldIdLst>");
    let mut pres_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rIdM" Type="{REL_MASTER}" Target="slideMasters/slideMaster1.xml"/><Relationship Id="rIdT" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>"#
    );
    if has_notes {
        let _ = write!(
            pres_rels,
            r#"<Relationship Id="rIdN" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="notesMasters/notesMaster1.xml"/>"#
        );
    }
    for i in 1..=n {
        let _ = write!(pres, r#"<p:sldId id="{}" r:id="rId{i}"/>"#, 255 + i);
        let _ = write!(pres_rels, r#"<Relationship Id="rId{i}" Type="{REL_SLIDE}" Target="slides/slide{i}.xml"/>"#);
    }
    let _ = write!(pres, r#"</p:sldIdLst><p:sldSz cx="{cx}" cy="{cy}"/><p:notesSz cx="6858000" cy="9144000"/>"#);
    pres.push_str(&sections_xml(deck));
    pres.push_str("</p:presentation>");
    pres_rels.push_str("</Relationships>");
    add("ppt/presentation.xml", pres.as_bytes())?;
    add("ppt/_rels/presentation.xml.rels", pres_rels.as_bytes())?;

    add("ppt/slideMasters/slideMaster1.xml", master_xml(n_layouts).as_bytes())?;
    let mut master_rels =
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}">"#);
    for l in 1..=n_layouts {
        let _ = write!(
            master_rels,
            r#"<Relationship Id="rId{l}" Type="{REL_LAYOUT}" Target="../slideLayouts/slideLayout{l}.xml"/>"#
        );
    }
    let _ = write!(
        master_rels,
        r#"<Relationship Id="rIdT" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#
    );
    add("ppt/slideMasters/_rels/slideMaster1.xml.rels", master_rels.as_bytes())?;

    let mut media_count = 0u32;
    for (k, layout) in deck.layouts.iter().enumerate() {
        let l = k + 1;
        let mut part = Part::new(format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="{REL_MASTER}" Target="../slideMasters/slideMaster1.xml"/>"#
        ));
        let _ = write!(
            part.xml,
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}" preserve="1"><p:cSld name="{}">"#,
            esc(&layout.name)
        );
        part.background(layout.background);
        part.open_tree();
        for (e, element) in layout.elements.iter().enumerate() {
            write_element(
                &mut part,
                &mut media_count,
                &mut add,
                deck_dir,
                deck,
                None,
                element,
                e + 2,
                None,
                &format!("layout {}", layout.name),
                &mut warnings,
                math,
            )?;
        }
        let base_id = layout.elements.len() + 2;
        for (pi, ph) in layout.placeholders.iter().enumerate() {
            let _ = write!(
                part.xml,
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{}" name="{}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr>{}</p:nvPr></p:nvSpPr><p:spPr>{}</p:spPr>{}</p:sp>"#,
                base_id + pi,
                esc(&ph.name),
                ph_tag(layout, &ph.name),
                xfrm(&ph.rect),
                placeholder_body(ph)
            );
        }
        part.xml.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>");
        part.rels.push_str("</Relationships>");
        add(&format!("ppt/slideLayouts/slideLayout{l}.xml"), part.xml.as_bytes())?;
        add(&format!("ppt/slideLayouts/_rels/slideLayout{l}.xml.rels"), part.rels.as_bytes())?;
    }
    add(&format!("ppt/slideLayouts/slideLayout{n_layouts}.xml"), blank_layout_xml().as_bytes())?;
    add(&format!("ppt/slideLayouts/_rels/slideLayout{n_layouts}.xml.rels"), format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="{REL_MASTER}" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#).as_bytes())?;
    add("ppt/theme/theme1.xml", theme_xml(&deck.theme).as_bytes())?;
    if has_notes {
        add("ppt/notesMasters/notesMaster1.xml", notes_master_xml().as_bytes())?;
        add("ppt/notesMasters/_rels/notesMaster1.xml.rels", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#).as_bytes())?;
    }

    for (i, slide) in deck.slides.iter().enumerate() {
        let i = i + 1;
        let layout = deck.layout_of(slide);
        let layout_no =
            layout.and_then(|l| deck.layouts.iter().position(|x| x.name == l.name)).map(|k| k + 1).unwrap_or(n_layouts);
        let mut part = Part::new(format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="{REL_LAYOUT}" Target="../slideLayouts/slideLayout{layout_no}.xml"/>"#
        ));
        let _ = write!(
            part.xml,
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld{}>"#,
            if slide.name.is_empty() { String::new() } else { format!(r#" name="{}""#, esc(&slide.name)) }
        );
        part.background(slide.background);
        part.open_tree();
        for (k, element) in slide.elements.iter().enumerate() {
            let ph = match (element, layout) {
                (Element::Text(t), Some(layout)) => {
                    t.placeholder.as_deref().filter(|n| layout.placeholder(n).is_some()).map(|n| ph_tag(layout, n))
                }
                _ => None,
            };
            write_element(
                &mut part,
                &mut media_count,
                &mut add,
                deck_dir,
                deck,
                Some(i - 1),
                element,
                k + 2,
                ph.as_deref(),
                &format!("slide {i}"),
                &mut warnings,
                math,
            )?;
        }
        part.xml.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>");
        part.xml.push_str(&timing_xml(slide, i, (deck.width, deck.height), &mut warnings));
        part.xml.push_str("</p:sld>");
        if !slide.notes.is_empty() {
            let rel = part.next_rel();
            let _ = write!(
                part.rels,
                r#"<Relationship Id="rId{rel}" Type="{REL_NOTES}" Target="../notesSlides/notesSlide{i}.xml"/>"#
            );
            add(&format!("ppt/notesSlides/notesSlide{i}.xml"), notes_xml(&slide.notes).as_bytes())?;
            add(&format!("ppt/notesSlides/_rels/notesSlide{i}.xml.rels"), format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/><Relationship Id="rId2" Type="{REL_SLIDE}" Target="../slides/slide{i}.xml"/></Relationships>"#).as_bytes())?;
        }
        part.rels.push_str("</Relationships>");
        add(&format!("ppt/slides/slide{i}.xml"), part.xml.as_bytes())?;
        add(&format!("ppt/slides/_rels/slide{i}.xml.rels"), part.rels.as_bytes())?;
    }
    zip.finish()?;
    Ok(warnings)
}

/// Build steps as a PowerPoint timing tree: one click per step, a fade
/// entrance for opacity going to 1, a fade exit for opacity going to 0.
fn timing_xml(slide: &Slide, slide_no: usize, size: (f64, f64), warnings: &mut Vec<String>) -> String {
    let last = slide.last_step();
    let mut id = 1u32;
    let mut next = || {
        id += 1;
        id
    };
    // Videos that play when they appear: PowerPoint's "Automatically", a
    // media call after the effect that shows them (or with the slide).
    let autoplay = |k: usize, at: u32, next: &mut dyn FnMut() -> u32| -> Option<String> {
        let Element::Media(m) = &slide.elements[k] else { return None };
        let shown_from = m.steps.iter().filter(|s| s.set.opacity.is_some_and(|o| o >= 1.0)).map(|s| s.at).min();
        let starts_hidden = m.steps.iter().any(|s| s.at == 0 && s.set.opacity == Some(0.0));
        let appears = if starts_hidden { shown_from? } else { 0 };
        if m.is_gif() || m.play != Play::OnAppear || appears != at {
            return None;
        }
        let (outer, inner) = (next(), next());
        Some(format!(
            r#"<p:par><p:cTn id="{outer}" presetID="1" presetClass="mediacall" presetSubtype="0" fill="hold" nodeType="afterEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:cmd type="call" cmd="playFrom(0.0)"><p:cBhvr><p:cTn id="{inner}" dur="1" fill="hold"/><p:tgtEl><p:spTgt spid="{}"/></p:tgtEl></p:cBhvr></p:cmd></p:childTnLst></p:cTn></p:par>"#,
            k + 2
        ))
    };
    let mut clicks = String::new();
    let at_start: String = (0..slide.elements.len()).filter_map(|k| autoplay(k, 0, &mut next)).collect();
    if !at_start.is_empty() {
        let _ = write!(
            clicks,
            r#"<p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>{at_start}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#,
            next(),
            next()
        );
    }
    for step in 1..=last {
        let mut effects = String::new();
        for (k, element) in slide.elements.iter().enumerate() {
            let shape_id = k + 2;
            for s in element.steps().iter().filter(|s| s.at == step) {
                let o = &s.set;
                let dur = ((if s.duration > 0.0 { s.duration } else { 0.5 }) * 1000.0) as u64;
                let delay = (s.delay * 1000.0) as u64;
                // Geometry: a scale about the center, a turn, then a move of
                // the center, from the state before this step.
                let (before, after) = (element.state_at(step - 1), element.state_at(step));
                let (b, a) = (before.rect, after.rect);
                let mut behaviours = vec![];
                let target = format!(r#"<p:tgtEl><p:spTgt spid="{shape_id}"/></p:tgtEl>"#);
                if (o.w.is_some() || o.h.is_some()) && b.w > 0.0 && b.h > 0.0 && (a.w != b.w || a.h != b.h) {
                    behaviours.push((
                        "6",
                        "emph",
                        format!(
                            r#"<p:animScale><p:cBhvr><p:cTn id="{{id}}" dur="{dur}" fill="hold"/>{target}</p:cBhvr><p:by x="{}" y="{}"/></p:animScale>"#,
                            (a.w / b.w * 100_000.0).round(),
                            (a.h / b.h * 100_000.0).round()
                        ),
                    ));
                }
                if o.rotation.is_some() && after.rotation != before.rotation {
                    behaviours.push((
                        "8",
                        "emph",
                        format!(
                            r#"<p:animRot by="{}"><p:cBhvr><p:cTn id="{{id}}" dur="{dur}" fill="hold"/>{target}<p:attrNameLst><p:attrName>r</p:attrName></p:attrNameLst></p:cBhvr></p:animRot>"#,
                            ((after.rotation - before.rotation) * 60000.0).round()
                        ),
                    ));
                }
                let (dx, dy) = ((a.x + a.w / 2.0) - (b.x + b.w / 2.0), (a.y + a.h / 2.0) - (b.y + b.h / 2.0));
                if (o.x.is_some() || o.y.is_some()) && (dx.abs() > 1e-6 || dy.abs() > 1e-6) {
                    behaviours.push((
                        "42",
                        "path",
                        format!(
                            r#"<p:animMotion origin="layout" path="M 0 0 L {} {} E" pathEditMode="relative" ptsTypes=""><p:cBhvr><p:cTn id="{{id}}" dur="{dur}" fill="hold"/>{target}<p:attrNameLst><p:attrName>ppt_x</p:attrName><p:attrName>ppt_y</p:attrName></p:attrNameLst></p:cBhvr></p:animMotion>"#,
                            dx / size.0,
                            dy / size.1
                        ),
                    ));
                }
                for (preset_id, class, body) in behaviours {
                    let node_type = if effects.is_empty() { "clickEffect" } else { "withEffect" };
                    let outer = next();
                    let body = body.replace("{id}", &next().to_string());
                    let _ = write!(
                        effects,
                        r#"<p:par><p:cTn id="{outer}" presetID="{preset_id}" presetClass="{class}" presetSubtype="0" fill="hold" nodeType="{node_type}"><p:stCondLst><p:cond delay="{delay}"/></p:stCondLst><p:childTnLst>{body}</p:childTnLst></p:cTn></p:par>"#
                    );
                }
                if o.fill.is_some() {
                    warnings
                        .push(format!("slide {slide_no}: step {step} of element {k} changes its color, not exported"));
                }
                let (class, preset, to) = match o.opacity {
                    Some(a) if a >= 1.0 => ("entr", "entr", "visible"),
                    Some(a) if a <= 0.0 => ("exit", "exit", "hidden"),
                    _ => continue,
                };
                let node_type = if effects.is_empty() { "clickEffect" } else { "withEffect" };
                let _ = write!(
                    effects,
                    r#"<p:par><p:cTn id="{}" presetID="10" presetClass="{class}" presetSubtype="0" fill="hold" nodeType="{node_type}"><p:stCondLst><p:cond delay="{delay}"/></p:stCondLst><p:childTnLst><p:set><p:cBhvr><p:cTn id="{}" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="{shape_id}"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="{}"/></p:to></p:set><p:animEffect transition="{}" filter="fade"><p:cBhvr><p:cTn id="{}" dur="{dur}"/><p:tgtEl><p:spTgt spid="{shape_id}"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par>"#,
                    next(),
                    next(),
                    to,
                    if preset == "entr" { "in" } else { "out" },
                    next()
                );
            }
        }
        let plays: String = (0..slide.elements.len()).filter_map(|k| autoplay(k, step, &mut next)).collect();
        effects.push_str(&plays);
        if effects.is_empty() {
            continue;
        }
        let _ = write!(
            clicks,
            r#"<p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>{effects}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#,
            next(),
            next()
        );
    }
    // Each embedded video gets the media node that lets a slideshow play it.
    let mut videos = String::new();
    for (k, element) in slide.elements.iter().enumerate() {
        let Element::Media(m) = element else { continue };
        if m.is_gif() {
            continue;
        }
        let _ = write!(
            videos,
            r#"<p:video><p:cMediaNode vol="80000"{}><p:cTn id="{}"{} fill="hold" display="0"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="{}"/></p:tgtEl></p:cMediaNode></p:video>"#,
            if m.sound { "" } else { r#" mute="1""# },
            next(),
            if m.looping { r#" repeatCount="indefinite""# } else { "" },
            k + 2
        );
    }
    if clicks.is_empty() && videos.is_empty() {
        return String::new();
    }
    let sequence = if clicks.is_empty() {
        String::new()
    } else {
        format!(
            r#"<p:seq concurrent="1" nextAc="seek"><p:cTn id="{}" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{clicks}</p:childTnLst></p:cTn><p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq>"#,
            next()
        )
    };
    format!(
        r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst>{sequence}{videos}</p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
    )
}

/// A slide or layout part being written: its XML and its relationships.
struct Part {
    xml: String,
    rels: String,
    rel_id: usize,
}

impl Part {
    fn new(rels: String) -> Self {
        Self { xml: String::new(), rels, rel_id: 1 }
    }

    fn next_rel(&mut self) -> usize {
        self.rel_id += 1;
        self.rel_id
    }

    fn background(&mut self, color: Option<Color>) {
        if let Some(bg) = color {
            let _ = write!(
                self.xml,
                r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#,
                hex(bg)
            );
        }
    }

    fn open_tree(&mut self) {
        self.xml.push_str(r#"<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#);
    }
}

/// The `<p:ph>` tag of a layout placeholder: `title` and `subtitle` map
/// to PowerPoint's types, the others are body placeholders numbered by
/// their position, so that slides and layouts agree.
fn ph_tag(layout: &Layout, name: &str) -> String {
    match name {
        "title" => r#"<p:ph type="title"/>"#.into(),
        "subtitle" => r#"<p:ph type="subTitle" idx="1"/>"#.into(),
        _ => {
            // `body3` keeps idx 3, so that a re-import gives the same name;
            // other names get an index past the usual ones.
            let idx = name
                .strip_prefix("body")
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|n| *n > 0)
                .unwrap_or_else(|| layout.placeholders.iter().position(|p| p.name == name).unwrap_or(0) + 10);
            format!(r#"<p:ph type="body" idx="{idx}"/>"#)
        }
    }
}

/// A layout placeholder's text body: its default style as a list style,
/// and one empty paragraph.
fn placeholder_body(ph: &Placeholder) -> String {
    let mut lvl = String::from("<a:lvl1pPr");
    match ph.align {
        Align::Left => {}
        Align::Center => lvl.push_str(r#" algn="ctr""#),
        Align::Right => lvl.push_str(r#" algn="r""#),
        Align::Justify => lvl.push_str(r#" algn="just""#),
    }
    lvl.push('>');
    if ph.bullets {
        lvl.push_str(r#"<a:buChar char="&#8226;"/>"#);
    } else {
        lvl.push_str("<a:buNone/>");
    }
    lvl.push_str("<a:defRPr");
    if let Some(size) = ph.size {
        let _ = write!(lvl, r#" sz="{}""#, (size * 100.0).round() as i64);
    }
    if ph.bold {
        lvl.push_str(r#" b="1""#);
    }
    lvl.push('>');
    if let Some(c) = ph.color {
        let _ = write!(lvl, r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(c));
    }
    if let Some(f) = &ph.font {
        let _ = write!(lvl, r#"<a:latin typeface="{}"/>"#, esc(f));
    }
    lvl.push_str("</a:defRPr></a:lvl1pPr>");
    format!(r#"<p:txBody><a:bodyPr/><a:lstStyle>{lvl}</a:lstStyle><a:p><a:endParaRPr/></a:p></p:txBody>"#)
}

/// Writes one element into a slide or layout part. `ph` is the `<p:ph>`
/// tag when a text box fills a layout placeholder.
#[allow(clippy::too_many_arguments)]
fn write_element(
    part: &mut Part,
    media_count: &mut u32,
    add: &mut dyn FnMut(&str, &[u8]) -> Result<()>,
    deck_dir: &Path,
    deck: &Deck,
    slide_index: Option<usize>,
    element: &Element,
    id: usize,
    ph: Option<&str>,
    where_: &str,
    warnings: &mut Vec<String>,
    math: &dyn MathRenderer,
) -> Result<()> {
    let start = part.xml.len();
    write_element_plain(part, media_count, add, deck_dir, deck, slide_index, element, id, ph, where_, warnings, math)?;
    // The rotation the element has from the start goes on its frame, in
    // 60000ths of a degree.
    let rotation = element.state_at(0).rotation;
    if rotation != 0.0 {
        let rot = format!(r#" rot="{}""#, (rotation * 60_000.0).round() as i64);
        let written = part.xml.split_off(start);
        let turned = match (written.find("<a:xfrm"), written.find("<p:xfrm")) {
            (Some(a), Some(p)) if p < a => written.replacen("<p:xfrm", &format!("<p:xfrm{rot}"), 1),
            (Some(_), _) => written.replacen("<a:xfrm", &format!("<a:xfrm{rot}"), 1),
            (None, Some(_)) => written.replacen("<p:xfrm", &format!("<p:xfrm{rot}"), 1),
            (None, None) => written,
        };
        part.xml.push_str(&turned);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_element_plain(
    part: &mut Part,
    media_count: &mut u32,
    add: &mut dyn FnMut(&str, &[u8]) -> Result<()>,
    deck_dir: &Path,
    deck: &Deck,
    slide_index: Option<usize>,
    element: &Element,
    id: usize,
    ph: Option<&str>,
    where_: &str,
    warnings: &mut Vec<String>,
    math: &dyn MathRenderer,
) -> Result<()> {
    match element {
        Element::Text(t) => {
            let xml = &mut part.xml;
            let (cnv, nv) = match ph {
                Some(tag) => {
                    (r#"<p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#.to_string(), format!("<p:nvPr>{tag}</p:nvPr>"))
                }
                None => (r#"<p:cNvSpPr txBox="1"/>"#.to_string(), "<p:nvPr/>".to_string()),
            };
            let _ = write!(
                xml,
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="TextBox {id}"/>{cnv}{nv}</p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{}{}</p:spPr>{}</p:sp>"#,
                xfrm(&t.rect),
                fill(t.fill),
                line(t.stroke),
                tx_body(&t.paragraphs)
            );
        }
        Element::Shape(s) => {
            let xml = &mut part.xml;
            let prst = match s.kind {
                ShapeKind::Rectangle => "rect",
                ShapeKind::Ellipse => "ellipse",
                ShapeKind::Line | ShapeKind::Arrow => "line",
            };
            // A flipped line is PowerPoint's flipV; an arrow is a line with a tail end.
            let mut frame = xfrm(&s.rect);
            if s.flip {
                frame = frame.replacen("<a:xfrm>", r#"<a:xfrm flipV="1">"#, 1);
            }
            let mut outline = line(s.stroke);
            if s.kind == ShapeKind::Arrow {
                let end = if s.reverse { "headEnd" } else { "tailEnd" };
                outline = outline.replacen("</a:ln>", &format!(r#"<a:{end} type="triangle"/></a:ln>"#), 1);
            }
            let _ = write!(
                xml,
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Shape {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{frame}<a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom>{}{outline}</p:spPr>{}</p:sp>"#,
                fill(s.fill),
                tx_body(&s.paragraphs)
            );
        }
        Element::Group(g) => {
            // Children keep slide coordinates: the child space equals the group's box.
            let r = &g.rect;
            let _ = write!(
                part.xml,
                r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="{id}" name="Group {id}"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/><a:chOff x="{}" y="{}"/><a:chExt cx="{}" cy="{}"/></a:xfrm></p:grpSpPr>"#,
                emu(r.x),
                emu(r.y),
                emu(r.w),
                emu(r.h),
                emu(r.x),
                emu(r.y),
                emu(r.w),
                emu(r.h)
            );
            for (k, child) in g.children.iter().enumerate() {
                // Child ids must be unique in the slide: derive them from the group's.
                write_element(
                    part,
                    media_count,
                    add,
                    deck_dir,
                    deck,
                    slide_index,
                    child,
                    id * 100 + k + 1,
                    None,
                    where_,
                    warnings,
                    math,
                )?;
            }
            part.xml.push_str("</p:grpSp>");
        }
        Element::Outline(o) => {
            // The outline as it reads on this slide: a plain text box.
            let paragraphs = deck.outline_paragraphs(slide_index, o);
            let _ = write!(
                part.xml,
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Outline {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>{}</p:sp>"#,
                xfrm(&o.rect),
                tx_body(&paragraphs)
            );
        }
        Element::Math(m)
            if typeset_picture(part, media_count, add, id, &m.rect, &format!("typst:{}", m.source), {
                let theme =
                    slide_index.and_then(|i| deck.slides.get(i)).map(|sl| deck.theme_of(sl)).unwrap_or_default();
                math.math_png(
                    &deck.math_preamble,
                    &m.source,
                    m.size.or(theme.size).unwrap_or(18.0),
                    m.color.or(theme.color).unwrap_or(Color::BLACK),
                )
            })? => {}
        Element::Figure(f)
            if typeset_picture(part, media_count, add, id, &f.rect, &format!("typst-figure:{}", f.source), {
                let theme =
                    slide_index.and_then(|i| deck.slides.get(i)).map(|sl| deck.theme_of(sl)).unwrap_or_default();
                math.figure_png(&deck.math_preamble, &f.source, f.rect.w.max(1.0), &theme.defaults())
            })? => {}
        Element::Math(m) => {
            let xml = &mut part.xml;
            warnings.push(format!("{where_}: formula exported as its source text"));
            let p = Paragraph {
                runs: vec![Run {
                    text: m.source.clone(),
                    size: m.size,
                    color: m.color,
                    font: Some("Consolas".into()),
                    ..Default::default()
                }],
                align: Align::Center,
                bullet: None,
            };
            let _ = write!(
                xml,
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Math {id}" descr="typst:{}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>{}</p:sp>"#,
                esc(&m.source),
                xfrm(&m.rect),
                tx_body(&[p])
            );
        }
        Element::Table(t) => {
            let columns = t.column_count().max(1);
            let mut xml = format!(
                r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Table {id}"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>{}<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr{}/><a:tblGrid>"#,
                xfrm(&t.rect).replace("a:xfrm", "p:xfrm"),
                if t.header { r#" firstRow="1""# } else { "" }
            );
            for j in 0..columns {
                let _ = write!(xml, r#"<a:gridCol w="{}"/>"#, emu(t.cell_rect(&t.rect, 0, j).w));
            }
            xml.push_str("</a:tblGrid>");
            for (i, row) in t.rows.iter().enumerate() {
                let _ = write!(xml, r#"<a:tr h="{}">"#, emu(t.cell_rect(&t.rect, i, 0).h));
                for j in 0..columns {
                    let cell = row.get(j).cloned().unwrap_or_default();
                    let body = tx_body(&cell.paragraphs).replace("p:txBody", "a:txBody");
                    let fill = match cell.fill {
                        Some(c) => {
                            format!(r#"<a:tcPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:tcPr>"#, hex(c))
                        }
                        None => "<a:tcPr/>".to_string(),
                    };
                    // A merged cell spans; the cells it covers say so.
                    let mut attrs = String::new();
                    if cell.colspan > 1 {
                        let _ = write!(attrs, r#" gridSpan="{}""#, cell.colspan);
                    }
                    if cell.rowspan > 1 {
                        let _ = write!(attrs, r#" rowSpan="{}""#, cell.rowspan);
                    }
                    let (oi, oj) = t.owner(i, j);
                    if oj < j {
                        attrs.push_str(r#" hMerge="1""#);
                    }
                    if oi < i {
                        attrs.push_str(r#" vMerge="1""#);
                    }
                    let _ = write!(xml, "<a:tc{attrs}>{body}{fill}</a:tc>");
                }
                xml.push_str("</a:tr>");
            }
            xml.push_str("</a:tbl></a:graphicData></a:graphic></p:graphicFrame>");
            part.xml.push_str(&xml);
        }
        Element::Figure(f) => {
            // The figure's source, as a text box; PowerPoint cannot run Typst.
            warnings.push(format!("{where_}: figure exported as its Typst source"));
            let text = deck_core::TextBox::plain(f.rect, &f.source);
            return write_element(
                part,
                media_count,
                add,
                deck_dir,
                deck,
                slide_index,
                &Element::Text(text),
                id,
                ph,
                where_,
                warnings,
                math,
            );
        }
        Element::Media(m) if m.is_gif() => {
            // PowerPoint animates a GIF placed as a picture.
            let gif = Image { rect: m.rect, steps: m.steps.clone(), src: m.src.clone(), ..Default::default() };
            return write_element(
                part,
                media_count,
                add,
                deck_dir,
                deck,
                slide_index,
                &Element::Image(gif),
                id,
                ph,
                where_,
                warnings,
                math,
            );
        }
        Element::Media(m) => {
            // The clip is embedded the way PowerPoint does it: a picture (the
            // poster) that links the media part twice, as a video file and
            // as embedded media.
            let src = deck_dir.join(&m.src);
            let bytes = std::fs::read(&src).with_context(|| format!("reading asset {}", src.display()))?;
            let ext = Path::new(&m.src)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_else(|| "mp4".into());
            *media_count += 1;
            let clip = format!("media{media_count}.{ext}");
            add(&format!("ppt/media/{clip}"), &bytes)?;
            let poster = match &m.poster {
                Some(poster) => {
                    let path = deck_dir.join(poster);
                    std::fs::read(&path).with_context(|| format!("reading asset {}", path.display()))?
                }
                None => VIDEO_POSTER.to_vec(),
            };
            *media_count += 1;
            let still = format!("image{media_count}.png");
            add(&format!("ppt/media/{still}"), &poster)?;
            let (video_rel, media_rel, poster_rel) = (part.next_rel(), part.next_rel(), part.next_rel());
            let _ = write!(
                part.rels,
                r#"<Relationship Id="rId{video_rel}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/video" Target="../media/{clip}"/><Relationship Id="rId{media_rel}" Type="http://schemas.microsoft.com/office/2007/relationships/media" Target="../media/{clip}"/><Relationship Id="rId{poster_rel}" Type="{REL_IMAGE}" Target="../media/{still}"/>"#
            );
            let _ = write!(
                part.xml,
                r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Video {id}"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:videoFile r:link="rId{video_rel}"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="{NS_P14}" r:embed="rId{media_rel}"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="rId{poster_rel}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
                xfrm(&m.rect)
            );
        }
        Element::Image(img) => {
            let src = deck_dir.join(&img.src);
            let bytes = std::fs::read(&src).with_context(|| format!("reading asset {}", src.display()))?;
            *media_count += 1;
            let ext = Path::new(&img.src)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_else(|| "png".into());
            let media = format!("ppt/media/image{media_count}.{ext}");
            add(&media, &bytes)?;
            let rel = part.next_rel();
            let _ = write!(
                part.rels,
                r#"<Relationship Id="rId{rel}" Type="{REL_IMAGE}" Target="../media/image{media_count}.{ext}"/>"#
            );
            let c = img.crop.clamped();
            let src_rect = if c.is_none() {
                String::new()
            } else {
                let k = |v: f64| (v * 100_000.0).round() as i64;
                format!(r#"<a:srcRect l="{}" t="{}" r="{}" b="{}"/>"#, k(c.left), k(c.top), k(c.right), k(c.bottom))
            };
            let geom = match img.mask {
                Mask::Rectangle => r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#.to_string(),
                Mask::Ellipse => r#"<a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>"#.to_string(),
                Mask::RoundedRectangle { radius } => format!(
                    r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val {}"/></a:avLst></a:prstGeom>"#,
                    (radius.clamp(0.0, 0.5) * 100_000.0).round() as i64
                ),
            };
            let _ = write!(
                part.xml,
                r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Picture {id}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId{rel}"/>{src_rect}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}{geom}</p:spPr></p:pic>"#,
                xfrm(&img.rect).replacen(
                    "<a:xfrm>",
                    &format!(
                        "<a:xfrm{}{}>",
                        if img.mirror_h { r#" flipH="1""# } else { "" },
                        if img.mirror_v { r#" flipV="1""# } else { "" }
                    ),
                    1
                )
            );
        }
    }
    Ok(())
}

/// PowerPoint sections (a 2010 extension): one per slide that starts a
/// section, holding the slide ids up to the next one. Slides before the
/// first section go into a default section, as PowerPoint does.
/// The poster of a video that has none: a small dark PNG.
const VIDEO_POSTER: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 16, 0, 0, 0, 9, 8, 2, 0, 0, 0, 180, 72, 59,
    101, 0, 0, 0, 19, 73, 68, 65, 84, 120, 218, 99, 80, 208, 48, 32, 9, 49, 140, 106, 24, 20, 26, 0, 31, 223, 67, 129,
    243, 103, 3, 85, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

/// Writes a typeset formula or figure as a picture fitted in `rect`,
/// its source in the picture's description. `false` when there is no
/// picture (no typesetter, or an error): the caller writes the source.
fn typeset_picture(
    part: &mut Part,
    media_count: &mut u32,
    add: &mut dyn FnMut(&str, &[u8]) -> Result<()>,
    id: usize,
    rect: &Rect,
    description: &str,
    picture: Option<(Vec<u8>, f64, f64)>,
) -> Result<bool> {
    let Some((png, w, h)) = picture.filter(|(_, w, h)| *w > 0.0 && *h > 0.0) else { return Ok(false) };
    *media_count += 1;
    let media = format!("ppt/media/image{media_count}.png");
    add(&media, &png)?;
    let rel = part.next_rel();
    let _ = write!(
        part.rels,
        r#"<Relationship Id="rId{rel}" Type="{REL_IMAGE}" Target="../media/image{media_count}.png"/>"#
    );
    // As on screen: natural size, scaled down to fit, centered in the box.
    let scale = (rect.w / w).min(rect.h / h).min(1.0);
    let fitted =
        Rect::new(rect.x + (rect.w - w * scale) / 2.0, rect.y + (rect.h - h * scale) / 2.0, w * scale, h * scale);
    let _ = write!(
        part.xml,
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Typst {id}" descr="{}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId{rel}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
        esc(description),
        xfrm(&fitted)
    );
    Ok(true)
}

fn sections_xml(deck: &Deck) -> String {
    if deck.slides.iter().all(|s| s.section.is_none()) {
        return String::new();
    }
    let mut sections: Vec<(String, Vec<usize>)> = vec![];
    for (i, slide) in deck.slides.iter().enumerate() {
        match (&slide.section, sections.last_mut()) {
            (Some(name), _) => sections.push((name.clone(), vec![i])),
            (None, Some((_, ids))) => ids.push(i),
            (None, None) => sections.push(("Default Section".into(), vec![i])),
        }
    }
    let mut out = String::from(
        r#"<p:extLst><p:ext uri="{521415D9-36F7-43E2-AB2F-B90AF26B5E84}"><p14:sectionLst xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main">"#,
    );
    for (k, (name, ids)) in sections.iter().enumerate() {
        let _ = write!(
            out,
            r#"<p14:section name="{}" id="{{{:08X}-0000-4000-8000-000000000000}}"><p14:sldIdLst>"#,
            esc(name),
            0x1000_0000u32 + k as u32
        );
        for i in ids {
            let _ = write!(out, r#"<p14:sldId id="{}"/>"#, 256 + i);
        }
        out.push_str("</p14:sldIdLst></p14:section>");
    }
    out.push_str("</p14:sectionLst></p:ext></p:extLst>");
    out
}

fn emu(pt: f64) -> i64 {
    (pt * EMU_PER_PT).round() as i64
}

fn hex(c: Color) -> String {
    format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn xfrm(r: &Rect) -> String {
    format!(
        r#"<a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        emu(r.x),
        emu(r.y),
        emu(r.w),
        emu(r.h)
    )
}

fn fill(c: Option<Color>) -> String {
    match c {
        Some(c) => format!(r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(c)),
        None => "<a:noFill/>".into(),
    }
}

fn line(s: Option<Stroke>) -> String {
    match s {
        Some(s) => format!(
            r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln>"#,
            emu(s.width),
            hex(s.color)
        ),
        None => "<a:ln><a:noFill/></a:ln>".into(),
    }
}

fn tx_body(paragraphs: &[Paragraph]) -> String {
    let mut out =
        String::from(r#"<p:txBody><a:bodyPr wrap="square" rtlCol="0"><a:normAutofit/></a:bodyPr><a:lstStyle/>"#);
    if paragraphs.is_empty() {
        out.push_str("<a:p/>");
    }
    for p in paragraphs {
        out.push_str("<a:p><a:pPr");
        if let Some(level) = p.bullet {
            let _ =
                write!(out, r#" lvl="{level}" marL="{}" indent="-{}""#, emu(18.0 * (level as f64 + 1.0)), emu(18.0));
        }
        match p.align {
            Align::Left => {}
            Align::Center => out.push_str(r#" algn="ctr""#),
            Align::Right => out.push_str(r#" algn="r""#),
            Align::Justify => out.push_str(r#" algn="just""#),
        }
        out.push('>');
        if p.bullet.is_some() {
            out.push_str(r#"<a:buChar char="&#8226;"/>"#);
        } else {
            out.push_str("<a:buNone/>");
        }
        out.push_str("</a:pPr>");
        for r in &p.runs {
            if r.text == "\n" {
                out.push_str("<a:br/>");
                continue;
            }
            // `{{slide}}` is PowerPoint's slide number field; other
            // variables have no PowerPoint equivalent and go out as text.
            let mut pieces = r.text.split("{{slide}}").peekable();
            while let Some(piece) = pieces.next() {
                if !piece.is_empty() {
                    let mut run = r.clone();
                    run.text = piece.to_string();
                    out.push_str(&run_xml(&run, None));
                }
                if pieces.peek().is_some() {
                    let mut run = r.clone();
                    run.text = "1".into();
                    out.push_str(&run_xml(&run, Some("slidenum")));
                }
            }
        }
        out.push_str("</a:p>");
    }
    out.push_str("</p:txBody>");
    out
}

/// A run, or a field of that `kind` with the run's style.
fn run_xml(r: &Run, field: Option<&str>) -> String {
    let mut out = String::new();
    {
        match field {
            Some(kind) => {
                let _ = write!(
                    out,
                    r#"<a:fld id="{{B6F5B7C2-0000-4000-8000-00000000{:04X}}}" type="{kind}"><a:rPr lang="en-US""#,
                    r.text.len()
                );
            }
            None => out.push_str(r#"<a:r><a:rPr lang="en-US""#),
        }
        {
            if r.bold {
                out.push_str(r#" b="1""#);
            }
            if r.italic {
                out.push_str(r#" i="1""#);
            }
            if let Some(s) = r.size {
                let _ = write!(out, r#" sz="{}""#, (s * 100.0).round() as i64);
            }
            out.push('>');
            if let Some(c) = r.color {
                let _ = write!(out, r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(c));
            }
            if let Some(f) = &r.font {
                let _ = write!(out, r#"<a:latin typeface="{}"/>"#, esc(f));
            }
            let close = if field.is_some() { "</a:fld>" } else { "</a:r>" };
            let _ = write!(out, r#"</a:rPr><a:t>{}</a:t>{close}"#, esc(&r.text));
        }
    }
    out
}

fn notes_xml(notes: &str) -> String {
    let mut body = String::new();
    for line in notes.lines() {
        let _ = write!(body, r#"<a:p><a:r><a:rPr lang="en-US"/><a:t>{}</a:t></a:r></a:p>"#, esc(line));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:notes xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{body}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#
    )
}

const CLR_MAP: &str = r#"<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>"#;

fn master_xml(n_layouts: usize) -> String {
    let mut ids = String::new();
    for l in 1..=n_layouts {
        let _ = write!(ids, r#"<p:sldLayoutId id="{}" r:id="rId{l}"/>"#, 2147483648u64 + l as u64);
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>{CLR_MAP}<p:sldLayoutIdLst>{ids}</p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz="4400"/></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#
    )
}

fn blank_layout_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
    )
}

fn notes_master_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:notesMaster xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="1143000" y="685800"/><a:ext cx="4572000" cy="3429000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="4343400"/><a:ext cx="5486400" cy="4114800"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr/></a:p></p:txBody></p:sp></p:spTree></p:cSld>{CLR_MAP}<p:notesStyle><a:lvl1pPr><a:defRPr sz="1200"/></a:lvl1pPr></p:notesStyle></p:notesMaster>"#
    )
}

/// The theme part: the deck theme's font as both theme fonts, its text
/// color as `dk1` and its accent and palette as `accent1`..`accent6`.
fn theme_xml(theme: &deck_core::Theme) -> String {
    let family = theme.font.as_deref().unwrap_or("Calibri");
    let font = |tag: &str| {
        format!(r#"<a:{tag}><a:latin typeface="{}"/><a:ea typeface=""/><a:cs typeface=""/></a:{tag}>"#, esc(family))
    };
    let hex = |c: Color| c.to_hex().trim_start_matches('#').to_string();
    let defaults = ["4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47"];
    let mut swatches: Vec<String> = theme.accent.into_iter().chain(theme.palette.iter().copied()).map(hex).collect();
    swatches.dedup();
    let accents: String = (0..6)
        .map(|i| {
            let v = swatches.get(i).cloned().unwrap_or_else(|| defaults[i].to_string());
            format!(r#"<a:accent{n}><a:srgbClr val="{v}"/></a:accent{n}>"#, n = i + 1)
        })
        .collect();
    let dk1 = theme.color.map(hex).unwrap_or_else(|| "000000".into());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="{NS_A}" name="Decker"><a:themeElements><a:clrScheme name="Decker"><a:dk1><a:srgbClr val="{dk1}"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>{accents}<a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Decker">{}{}</a:fontScheme><a:fmtScheme name="Decker"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#,
        font("majorFont"),
        font("minorFont")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Export, then import again with our own importer.
    #[test]
    fn round_trip_through_pptx() {
        let tmp = std::env::temp_dir().join(format!("decker-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let dir = tmp.join("src.deck");
        let mut deck = Deck::new("rt");
        let src = native::add_asset(&dir, "pic.png", include_bytes!("../../../../tests/fixtures/red.png")).unwrap();
        let mut tb = TextBox::plain(Rect::new(50.0, 40.0, 400.0, 60.0), "Title");
        tb.paragraphs[0].runs[0].bold = true;
        tb.paragraphs[0].runs[0].size = Some(32.0);
        tb.paragraphs[0].align = Align::Center;
        let bullet = Paragraph { runs: vec![Run::plain("item")], bullet: Some(0), ..Default::default() };
        tb.paragraphs.push(bullet);
        tb.steps = vec![Step::disappear(0), Step::appear(1)];
        deck.slides.push(Slide {
            name: "first".into(),
            background: Some(Color::rgb(0x11, 0x22, 0x33)),
            notes: "note line".into(),
            elements: vec![
                Element::Text(tb),
                Element::Image(Image {
                    rect: Rect::new(600.0, 300.0, 100.0, 80.0),
                    src,
                    crop: Crop { left: 0.1, top: 0.2, right: 0.0, bottom: 0.05 },
                    mask: Mask::RoundedRectangle { radius: 0.25 },
                    ..Default::default()
                }),
                Element::Shape(Shape {
                    rect: Rect::new(10.0, 400.0, 100.0, 50.0),
                    kind: ShapeKind::Ellipse,
                    fill: Some(Color::rgb(1, 2, 3)),
                    stroke: Some(Stroke { color: Color::BLACK, width: 2.0 }),
                    paragraphs: vec![Paragraph::plain("in")],
                    steps: vec![Step::disappear(2)],
                    ..Default::default()
                }),
                Element::Math(MathBlock {
                    rect: Rect::new(300.0, 300.0, 200.0, 60.0),
                    source: "x^2".into(),
                    ..Default::default()
                }),
                Element::Group({
                    let mut g = Group::of(vec![
                        Element::Shape(Shape {
                            rect: Rect::new(700.0, 100.0, 50.0, 50.0),
                            fill: Some(Color::BLACK),
                            ..Default::default()
                        }),
                        Element::Text(TextBox::plain(Rect::new(760.0, 100.0, 150.0, 50.0), "grouped")),
                    ]);
                    g.steps = vec![Step::disappear(0), Step::appear(1)];
                    g
                }),
            ],
            ..Default::default()
        });
        deck.theme = deck_core::Theme {
            font: Some("Libertinus Serif".into()),
            color: Some(Color::rgb(0x11, 0x22, 0x33)),
            accent: Some(Color::rgb(0xAA, 0, 0)),
            palette: vec![Color::rgb(0xAA, 0, 0), Color::rgb(0, 0xAA, 0)],
            ..Default::default()
        };
        deck.layouts.push(Layout {
            name: "Corp".into(),
            background: Some(Color::rgb(0xEE, 0xEE, 0xEE)),
            theme: Default::default(),
            elements: vec![Element::Shape(Shape {
                rect: Rect::new(0.0, 500.0, 960.0, 40.0),
                fill: Some(Color::rgb(0x10, 0x20, 0x30)),
                ..Default::default()
            })],
            placeholders: vec![
                Placeholder {
                    name: "title".into(),
                    rect: Rect::new(40.0, 30.0, 880.0, 80.0),
                    size: Some(40.0),
                    bold: true,
                    align: Align::Center,
                    ..Default::default()
                },
                Placeholder {
                    name: "body1".into(),
                    rect: Rect::new(40.0, 130.0, 880.0, 340.0),
                    bullets: true,
                    ..Default::default()
                },
            ],
        });
        let mut from_layout = deck.layouts[0].new_slide();
        from_layout.name = "second".into();
        from_layout.section = Some("Part <2>".into());
        deck.slides.push(from_layout);
        native::save(&deck, &dir).unwrap();
        let pptx = tmp.join("out.pptx");
        let warnings = export(&deck, &dir, &pptx).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");

        let back_dir = tmp.join("back.deck");
        let (back, report) = super::super::import(&pptx, &back_dir).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(back.slides.len(), 2);
        // The layout came back with its background, static shape and placeholders.
        assert_eq!(back.layouts.len(), 1, "{:?}", back.layouts);
        let corp = &back.layouts[0];
        assert_eq!(corp.name, "Corp");
        assert_eq!(corp.background, Some(Color::rgb(0xEE, 0xEE, 0xEE)));
        assert_eq!(corp.theme.font.as_deref(), Some("Libertinus Serif"), "the theme font comes back on the layout");
        assert_eq!(corp.theme.color, Some(Color::rgb(0x11, 0x22, 0x33)));
        assert_eq!(corp.theme.accent, Some(Color::rgb(0xAA, 0, 0)));
        assert_eq!(corp.theme.palette.len(), 6, "{:?}", corp.theme.palette);
        assert_eq!(corp.theme.palette[1], Color::rgb(0, 0xAA, 0));
        assert_eq!(corp.elements.len(), 1);
        assert_eq!(corp.placeholders.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["title", "body1"]);
        let title = corp.placeholder("title").unwrap();
        assert_eq!(title.rect, Rect::new(40.0, 30.0, 880.0, 80.0));
        assert_eq!((title.size, title.bold, title.align), (Some(40.0), true, Align::Center));
        assert!(corp.placeholder("body1").unwrap().bullets);
        let second = &back.slides[1];
        assert_eq!(second.layout.as_deref(), Some("Corp"));
        let names: Vec<Option<&str>> = second
            .elements
            .iter()
            .map(|e| if let Element::Text(t) = e { t.placeholder.as_deref() } else { None })
            .collect();
        assert_eq!(names, [Some("title"), Some("body1")]);
        assert_eq!(back.slides[0].layout, None);
        assert_eq!(back.slides[0].section.as_deref(), Some("Default Section"), "slides before the first section");
        assert_eq!(back.slides[1].section.as_deref(), Some("Part <2>"));
        let s = &back.slides[0];
        assert_eq!(s.name, "first");
        assert_eq!(s.background, Some(Color::rgb(0x11, 0x22, 0x33)));
        assert_eq!(s.notes, "note line");
        assert_eq!(s.elements.len(), 5);
        let Element::Group(g) = &s.elements[4] else { panic!("group") };
        assert_eq!(g.children.len(), 2);
        assert_eq!(*g.children[1].rect(), Rect::new(760.0, 100.0, 150.0, 50.0));
        assert_eq!(g.rect, Rect::new(700.0, 100.0, 210.0, 50.0));
        assert_eq!(g.steps.len(), 2, "the entrance animates the group: {:?}", g.steps);
        let Element::Text(t) = &s.elements[0] else { panic!("text") };
        assert_eq!(t.rect, Rect::new(50.0, 40.0, 400.0, 60.0));
        assert_eq!(t.paragraphs[0].align, Align::Center);
        assert!(t.paragraphs[0].runs[0].bold);
        assert_eq!(t.paragraphs[0].runs[0].size, Some(32.0));
        assert_eq!(t.paragraphs[1].bullet, Some(0));
        assert_eq!(t.steps.len(), 2, "{:?}", t.steps);
        assert_eq!(t.steps[0].set.opacity, Some(0.0));
        assert_eq!((t.steps[1].at, t.steps[1].set.opacity), (1, Some(1.0)));
        let Element::Image(i) = &s.elements[1] else { panic!("image") };
        assert_eq!(i.rect.w, 100.0);
        assert_eq!(i.crop, Crop { left: 0.1, top: 0.2, right: 0.0, bottom: 0.05 });
        assert_eq!(i.mask, Mask::RoundedRectangle { radius: 0.25 });
        assert_eq!(std::fs::read(back_dir.join(&i.src)).unwrap(), include_bytes!("../../../../tests/fixtures/red.png"));
        let Element::Shape(sh) = &s.elements[2] else { panic!("shape") };
        assert_eq!(sh.kind, ShapeKind::Ellipse);
        assert_eq!(sh.stroke, Some(Stroke { color: Color::BLACK, width: 2.0 }));
        assert_eq!(sh.paragraphs[0].text(), "in");
        assert_eq!((sh.steps.len(), sh.steps[0].at, sh.steps[0].set.opacity), (1, 2, Some(0.0)));
        assert_eq!(s.last_step(), 2);
        let Element::Text(m) = &s.elements[3] else { panic!("math as text") };
        assert_eq!(m.text(), "x^2");
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn arrows_and_flipped_lines_round_trip() {
        let dir = std::env::temp_dir().join(format!("decker-arrow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let stroke = Some(Stroke { color: Color::rgb(0xAA, 0, 0), width: 3.0 });
        let mut deck = Deck::new("arrows");
        deck.slides.push(Slide {
            elements: vec![
                Element::Shape(Shape {
                    rect: Rect::new(10.0, 10.0, 200.0, 100.0),
                    kind: ShapeKind::Arrow,
                    stroke,
                    flip: true,
                    ..Default::default()
                }),
                Element::Shape(Shape {
                    rect: Rect::new(10.0, 200.0, 200.0, 50.0),
                    kind: ShapeKind::Line,
                    stroke,
                    ..Default::default()
                }),
                Element::Shape(Shape {
                    rect: Rect::new(300.0, 200.0, 100.0, 50.0),
                    fill: Some(Color::BLACK),
                    steps: vec![
                        Step {
                            at: 0,
                            set: Overrides { rotation: Some(30.0), ..Default::default() },
                            ..Default::default()
                        },
                        Step::disappear(0),
                        Step::appear(1),
                    ],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        });
        let pptx = dir.join("a.pptx");
        export(&deck, &dir, &pptx).unwrap();
        let (back, report) = super::super::import(&pptx, &dir.join("back.deck")).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let Element::Shape(arrow) = &back.slides[0].elements[0] else { panic!("arrow") };
        assert_eq!((arrow.kind, arrow.flip, arrow.stroke), (ShapeKind::Arrow, true, stroke));
        let Element::Shape(line) = &back.slides[0].elements[1] else { panic!("line") };
        assert_eq!((line.kind, line.flip), (ShapeKind::Line, false));
        assert_eq!(back.slides[0].elements[1].state_at(0).rotation, 0.0);
        let turned = &back.slides[0].elements[2];
        assert_eq!(turned.state_at(0).rotation, 30.0, "the rotation from the start survives");
        assert_eq!((turned.state_at(0).opacity, turned.state_at(1).opacity), (0.0, 1.0), "with its appear step");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn moves_turns_and_scales_round_trip() {
        let dir = std::env::temp_dir().join(format!("decker-motion-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut shape =
            Shape { rect: Rect::new(100.0, 100.0, 200.0, 100.0), fill: Some(Color::BLACK), ..Default::default() };
        shape.steps = vec![
            Step {
                at: 1,
                duration: 1.0,
                set: Overrides { x: Some(400.0), y: Some(250.0), ..Default::default() },
                ..Default::default()
            },
            Step { at: 2, set: Overrides { rotation: Some(90.0), ..Default::default() }, ..Default::default() },
            Step {
                at: 3,
                set: Overrides { x: Some(350.0), y: Some(225.0), w: Some(300.0), h: Some(150.0), ..Default::default() },
                ..Default::default()
            },
        ];
        let mut deck = Deck::new("motion");
        deck.slides.push(Slide { elements: vec![Element::Shape(shape)], ..Default::default() });
        let pptx = dir.join("m.pptx");
        let warnings = export(&deck, &dir, &pptx).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let (back, report) = super::super::import(&pptx, &dir.join("back.deck")).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let (a, b) = (&deck.slides[0].elements[0], &back.slides[0].elements[0]);
        for step in 0..=3 {
            let (x, y) = (a.state_at(step), b.state_at(step));
            let close = |p: f64, q: f64| (p - q).abs() < 0.01;
            assert!(
                close(x.rect.x, y.rect.x)
                    && close(x.rect.y, y.rect.y)
                    && close(x.rect.w, y.rect.w)
                    && close(x.rect.h, y.rect.h),
                "step {step}: {:?} came back as {:?}",
                x.rect,
                y.rect
            );
            assert!(close(x.rotation, y.rotation), "step {step}: rotation {} came back as {}", x.rotation, y.rotation);
        }
        assert_eq!(b.steps()[0].duration, 1.0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tables_round_trip() {
        let dir = std::env::temp_dir().join(format!("decker-table-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut t = deck_core::Table::new(Rect::new(40.0, 100.0, 600.0, 120.0), 3, 2);
        t.columns = vec![1.0, 2.0];
        t.rows[0][0] = deck_core::Cell::plain("Name");
        t.rows[0][1] = deck_core::Cell::plain("Value");
        t.rows[1][0] = deck_core::Cell::plain("pi");
        t.rows[1][1] = deck_core::Cell { fill: Some(Color::rgb(0xFF, 0xEE, 0xDD)), ..deck_core::Cell::plain("3.14") };
        t.rows[2][1] = deck_core::Cell::plain("merged");
        t.merge((2, 0), (2, 1));
        t.steps = vec![Step::disappear(0), Step::appear(1)];
        let mut deck = Deck::new("tables");
        deck.slides.push(Slide { elements: vec![Element::Table(t)], ..Default::default() });
        let pptx = dir.join("t.pptx");
        let warnings = export(&deck, &dir, &pptx).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let (back, report) = super::super::import(&pptx, &dir.join("back.deck")).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let Element::Table(b) = &back.slides[0].elements[0] else { panic!("a table") };
        assert_eq!((b.rows.len(), b.column_count(), b.header), (3, 2, true));
        assert_eq!(b.rect, Rect::new(40.0, 100.0, 600.0, 120.0));
        assert!((b.columns[1] / b.columns[0] - 2.0).abs() < 1e-6, "{:?}", b.columns);
        assert_eq!((b.rows[0][1].text(), b.rows[1][1].text()), ("Value".to_string(), "3.14".to_string()));
        assert_eq!(b.rows[1][1].fill, Some(Color::rgb(0xFF, 0xEE, 0xDD)));
        assert_eq!((b.rows[2][0].text(), b.rows[2][0].colspan), ("merged".to_string(), 2), "merged cells survive");
        assert!(b.covered(2, 1));
        assert_eq!(b.steps.len(), 2, "the table's animation comes back");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(feature = "typst")]
    #[test]
    fn formulas_and_figures_go_out_as_pictures_and_come_back() {
        let dir = std::env::temp_dir().join(format!("decker-typeset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut deck = Deck::new("typeset");
        deck.slides.push(Slide {
            elements: vec![
                Element::Math(MathBlock {
                    rect: Rect::new(100.0, 100.0, 400.0, 100.0),
                    source: "x^2 + y^2".into(),
                    size: Some(30.0),
                    ..Default::default()
                }),
                Element::Figure(Figure {
                    rect: Rect::new(100.0, 250.0, 300.0, 200.0),
                    source: "A *figure* <tag>".into(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        });
        let pptx = dir.join("t.pptx");
        let engine = deck_typst::Engine::new();
        let warnings = export_with(&deck, &dir, &pptx, &engine).unwrap();
        assert!(warnings.is_empty(), "no source-text fallback: {warnings:?}");
        let back_dir = dir.join("back.deck");
        let (back, report) = super::super::import(&pptx, &back_dir).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let Element::Math(m) = &back.slides[0].elements[0] else {
            panic!("a formula, got {:?}", back.slides[0].elements[0])
        };
        assert_eq!(m.source, "x^2 + y^2");
        // The picture is fitted and centered in the formula's box.
        assert!(m.rect.w < 400.0 && (m.rect.x + m.rect.w / 2.0 - 300.0).abs() < 0.5, "{:?}", m.rect);
        let Element::Figure(f) = &back.slides[0].elements[1] else { panic!("a figure") };
        assert_eq!(f.source, "A *figure* <tag>");
        assert!(
            !back_dir.join("assets").exists() || std::fs::read_dir(back_dir.join("assets")).unwrap().count() == 0,
            "their pictures are not kept as assets"
        );
        // Without a typesetter: the source as text, with a warning.
        let plain = export(&deck, &dir, &dir.join("plain.pptx")).unwrap();
        assert_eq!(plain.len(), 2, "{plain:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn autoplaying_videos_round_trip() {
        let dir = std::env::temp_dir().join(format!("decker-autoplay-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let clip = native::add_asset(&dir, "clip.mp4", b"not really a video").unwrap();
        let video = |x: f64, steps: Vec<Step>| {
            Element::Media(Media {
                rect: Rect::new(x, 100.0, 320.0, 180.0),
                src: clip.clone(),
                play: Play::OnAppear,
                steps,
                ..Default::default()
            })
        };
        let mut deck = Deck::new("autoplay");
        deck.slides.push(Slide {
            elements: vec![video(0.0, vec![]), video(400.0, vec![Step::disappear(0), Step::appear(1)])],
            ..Default::default()
        });
        let pptx = dir.join("a.pptx");
        let warnings = export(&deck, &dir, &pptx).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let (back, report) = super::super::import(&pptx, &dir.join("back.deck")).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        for (k, e) in back.slides[0].elements.iter().enumerate() {
            let Element::Media(m) = e else { panic!("a video") };
            assert_eq!(m.play, Play::OnAppear, "video {k} plays by itself");
        }
        assert_eq!(back.slides[0].last_step(), 1, "the start-of-slide play is no click");
        assert_eq!(back.slides[0].elements[1].state_at(1).opacity, 1.0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn videos_are_embedded_and_gifs_stay_pictures() {
        let dir = std::env::temp_dir().join(format!("decker-video-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let clip = native::add_asset(&dir, "clip.mp4", b"not really a video").unwrap();
        let gif = native::add_asset(&dir, "anim.gif", include_bytes!("../../../../tests/fixtures/red.png")).unwrap();
        let mut deck = Deck::new("video");
        deck.slides.push(Slide {
            elements: vec![
                Element::Media(Media {
                    rect: Rect::new(100.0, 100.0, 320.0, 180.0),
                    src: clip,
                    play: Play::Button,
                    ..Default::default()
                }),
                Element::Media(Media { rect: Rect::new(500.0, 100.0, 100.0, 100.0), src: gif, ..Default::default() }),
            ],
            ..Default::default()
        });
        let pptx = dir.join("v.pptx");
        let warnings = export(&deck, &dir, &pptx).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let back_dir = dir.join("back.deck");
        let (back, report) = super::super::import(&pptx, &back_dir).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        let Element::Media(m) = &back.slides[0].elements[0] else {
            panic!("a video, got {:?}", back.slides[0].elements[0])
        };
        assert_eq!(m.rect, Rect::new(100.0, 100.0, 320.0, 180.0));
        assert!(m.src.ends_with(".mp4"), "{}", m.src);
        assert_eq!(std::fs::read(back_dir.join(&m.src)).unwrap(), b"not really a video");
        assert!(m.poster.is_some(), "the picture PowerPoint shows is the poster");
        assert!(matches!(&back.slides[0].elements[1], Element::Image(i) if i.src.ends_with(".gif")));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
