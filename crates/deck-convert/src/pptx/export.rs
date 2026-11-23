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
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpg" ContentType="image/jpeg"/><Default Extension="jpeg" ContentType="image/jpeg"/><Default Extension="gif" ContentType="image/gif"/><Default Extension="bmp" ContentType="image/bmp"/><Default Extension="svg" ContentType="image/svg+xml"/><Default Extension="webp" ContentType="image/webp"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#,
    );
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
    let _ = write!(
        pres,
        r#"</p:sldIdLst><p:sldSz cx="{cx}" cy="{cy}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#
    );
    pres_rels.push_str("</Relationships>");
    add("ppt/presentation.xml", pres.as_bytes())?;
    add("ppt/_rels/presentation.xml.rels", pres_rels.as_bytes())?;

    add("ppt/slideMasters/slideMaster1.xml", master_xml().as_bytes())?;
    add("ppt/slideMasters/_rels/slideMaster1.xml.rels", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="{REL_LAYOUT}" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#).as_bytes())?;
    add("ppt/slideLayouts/slideLayout1.xml", layout_xml().as_bytes())?;
    add("ppt/slideLayouts/_rels/slideLayout1.xml.rels", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="{REL_MASTER}" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#).as_bytes())?;
    add("ppt/theme/theme1.xml", theme_xml().as_bytes())?;
    if has_notes {
        add("ppt/notesMasters/notesMaster1.xml", notes_master_xml().as_bytes())?;
        add("ppt/notesMasters/_rels/notesMaster1.xml.rels", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#).as_bytes())?;
    }

    let mut media_count = 0u32;
    for (i, slide) in deck.slides.iter().enumerate() {
        let i = i + 1;
        let mut rels = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="{REL_LAYOUT}" Target="../slideLayouts/slideLayout1.xml"/>"#
        );
        let mut xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld{}>"#,
            if slide.name.is_empty() { String::new() } else { format!(r#" name="{}""#, esc(&slide.name)) }
        );
        if let Some(bg) = slide.background {
            let _ = write!(
                xml,
                r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#,
                hex(bg)
            );
        }
        xml.push_str(r#"<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#);
        let mut rel_id = 1;
        for (k, element) in slide.elements.iter().enumerate() {
            let id = k + 2;
            match element {
                Element::Text(t) => {
                    let _ = write!(
                        xml,
                        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="TextBox {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{}{}</p:spPr>{}</p:sp>"#,
                        xfrm(&t.rect),
                        fill(t.fill),
                        line(t.stroke),
                        tx_body(&t.paragraphs)
                    );
                }
                Element::Shape(s) => {
                    let prst = match s.kind {
                        ShapeKind::Rectangle => "rect",
                        ShapeKind::Ellipse => "ellipse",
                        ShapeKind::Line => "line",
                    };
                    let _ = write!(
                        xml,
                        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Shape {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom>{}{}</p:spPr>{}</p:sp>"#,
                        xfrm(&s.rect),
                        fill(s.fill),
                        line(s.stroke),
                        tx_body(&s.paragraphs)
                    );
                }
                Element::Math(m) => {
                    warnings.push(format!("slide {i}: formula exported as its source text"));
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
                Element::Image(img) => {
                    let src = deck_dir.join(&img.src);
                    let bytes = std::fs::read(&src).with_context(|| format!("reading asset {}", src.display()))?;
                    media_count += 1;
                    let ext = Path::new(&img.src)
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_else(|| "png".into());
                    let media = format!("ppt/media/image{media_count}.{ext}");
                    add(&media, &bytes)?;
                    rel_id += 1;
                    let _ = write!(
                        rels,
                        r#"<Relationship Id="rId{rel_id}" Type="{REL_IMAGE}" Target="../media/image{media_count}.{ext}"/>"#
                    );
                    let _ = write!(
                        xml,
                        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Picture {id}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId{rel_id}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
                        xfrm(&img.rect)
                    );
                }
            }
        }
        xml.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>");
        xml.push_str(&timing_xml(slide, i, &mut warnings));
        xml.push_str("</p:sld>");
        if !slide.notes.is_empty() {
            rel_id += 1;
            let _ = write!(
                rels,
                r#"<Relationship Id="rId{rel_id}" Type="{REL_NOTES}" Target="../notesSlides/notesSlide{i}.xml"/>"#
            );
            add(&format!("ppt/notesSlides/notesSlide{i}.xml"), notes_xml(&slide.notes).as_bytes())?;
            add(&format!("ppt/notesSlides/_rels/notesSlide{i}.xml.rels"), format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{NS_REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/><Relationship Id="rId2" Type="{REL_SLIDE}" Target="../slides/slide{i}.xml"/></Relationships>"#).as_bytes())?;
        }
        rels.push_str("</Relationships>");
        add(&format!("ppt/slides/slide{i}.xml"), xml.as_bytes())?;
        add(&format!("ppt/slides/_rels/slide{i}.xml.rels"), rels.as_bytes())?;
    }
    zip.finish()?;
    Ok(warnings)
}

/// Build steps as a PowerPoint timing tree: one click per step, a fade
/// entrance for opacity going to 1, a fade exit for opacity going to 0.
fn timing_xml(slide: &Slide, slide_no: usize, warnings: &mut Vec<String>) -> String {
    let last = slide.last_step();
    if last == 0 {
        return String::new();
    }
    let mut id = 1u32;
    let mut next = || {
        id += 1;
        id
    };
    let mut clicks = String::new();
    for step in 1..=last {
        let mut effects = String::new();
        for (k, element) in slide.elements.iter().enumerate() {
            let shape_id = k + 2;
            for s in element.steps().iter().filter(|s| s.at == step) {
                let o = &s.set;
                if o.x.is_some()
                    || o.y.is_some()
                    || o.w.is_some()
                    || o.h.is_some()
                    || o.rotation.is_some()
                    || o.fill.is_some()
                {
                    warnings.push(format!(
                        "slide {slide_no}: step {step} of element {k} changes geometry or color, not exported"
                    ));
                }
                let (class, preset, to) = match o.opacity {
                    Some(a) if a >= 1.0 => ("entr", "entr", "visible"),
                    Some(a) if a <= 0.0 => ("exit", "exit", "hidden"),
                    _ => continue,
                };
                let dur = ((if s.duration > 0.0 { s.duration } else { 0.5 }) * 1000.0) as u64;
                let delay = (s.delay * 1000.0) as u64;
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
    if clicks.is_empty() {
        return String::new();
    }
    format!(
        r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="{}" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{clicks}</p:childTnLst></p:cTn><p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#,
        next()
    )
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
            out.push_str(r#"<a:r><a:rPr lang="en-US""#);
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
            let _ = write!(out, r#"</a:rPr><a:t>{}</a:t></a:r>"#, esc(&r.text));
        }
        out.push_str("</a:p>");
    }
    out.push_str("</p:txBody>");
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

fn master_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>{CLR_MAP}<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz="4400"/></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#
    )
}

fn layout_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
    )
}

fn notes_master_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:notesMaster xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="1143000" y="685800"/><a:ext cx="4572000" cy="3429000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="4343400"/><a:ext cx="5486400" cy="4114800"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr/></a:p></p:txBody></p:sp></p:spTree></p:cSld>{CLR_MAP}<p:notesStyle><a:lvl1pPr><a:defRPr sz="1200"/></a:lvl1pPr></p:notesStyle></p:notesMaster>"#
    )
}

fn theme_xml() -> String {
    let font = |tag: &str| {
        format!(r#"<a:{tag}><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:{tag}>"#)
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="{NS_A}" name="Decker"><a:themeElements><a:clrScheme name="Decker"><a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Decker">{}{}</a:fontScheme><a:fmtScheme name="Decker"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#,
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
                Element::Image(Image { rect: Rect::new(600.0, 300.0, 100.0, 80.0), src, steps: vec![] }),
                Element::Shape(Shape {
                    rect: Rect::new(10.0, 400.0, 100.0, 50.0),
                    kind: ShapeKind::Ellipse,
                    fill: Some(Color::rgb(1, 2, 3)),
                    stroke: Some(Stroke { color: Color::BLACK, width: 2.0 }),
                    paragraphs: vec![Paragraph::plain("in")],
                    steps: vec![Step::disappear(2)],
                }),
                Element::Math(MathBlock {
                    rect: Rect::new(300.0, 300.0, 200.0, 60.0),
                    source: "x^2".into(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        });
        native::save(&deck, &dir).unwrap();
        let pptx = tmp.join("out.pptx");
        let warnings = export(&deck, &dir, &pptx).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");

        let back_dir = tmp.join("back.deck");
        let (back, report) = super::super::import(&pptx, &back_dir).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(back.slides.len(), 1);
        let s = &back.slides[0];
        assert_eq!(s.name, "first");
        assert_eq!(s.background, Some(Color::rgb(0x11, 0x22, 0x33)));
        assert_eq!(s.notes, "note line");
        assert_eq!(s.elements.len(), 4);
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
}
