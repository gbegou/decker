//! Converter between decker's native format and PPTX / SVG.
//!
//! ```text
//! deck-convert import talk.pptx -o talk.deck     # PPTX → native folder
//! deck-convert export talk.deck -o talk.pptx     # native folder → PPTX
//! deck-convert render talk.deck -o svg/          # one SVG per slide
//! deck-convert info talk.deck                    # slide titles and counts
//! ```

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use deck_convert::pptx;
use deck_core::{native, svg, transfer, Element};

#[derive(Parser)]
#[command(version, about = "Convert between decker decks and PPTX / SVG")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import a .pptx into a deck folder.
    Import {
        pptx: PathBuf,
        /// Output deck folder (default: the pptx name with a .deck suffix).
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Export a deck folder to .pptx.
    Export {
        deck: PathBuf,
        /// Output file (default: the deck name with a .pptx suffix).
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Render each slide of a deck folder to an SVG file.
    Render {
        deck: PathBuf,
        /// Output directory (default: `<deck>/svg`).
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Export a deck folder to PDF, with bookmarks for sections and slides
    /// (needs the `typst` feature).
    Pdf {
        deck: PathBuf,
        /// Output file (default: the deck name with a .pdf suffix).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// One page per build step instead of one per slide.
        #[arg(long)]
        steps: bool,
        /// The handout instead: one A4 page per slide with its notes below.
        #[arg(long, conflicts_with = "steps")]
        notes: bool,
    },
    /// Pack a deck folder into a single `.deckz` file, to send it.
    Pack {
        deck: PathBuf,
        /// Output file (default: the deck name with a .deckz suffix).
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Unpack a `.deckz` file into a deck folder.
    Unpack {
        archive: PathBuf,
        /// Output folder (default: the archive name with a .deck suffix).
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Print a summary of a deck folder.
    Info { deck: PathBuf },
    /// List the layouts of a deck folder, with their placeholders.
    Layouts { deck: PathBuf },
    /// Copy slides from one deck folder into another, with their images
    /// and layouts.
    Copy {
        /// Source deck folder.
        from: PathBuf,
        /// Slides to copy, 1-based: `3`, `2,5`, `4-7`, or `all`.
        slides: String,
        /// Destination deck folder.
        into: PathBuf,
        /// Insert before this 1-based position (default: at the end).
        #[arg(long)]
        at: Option<usize>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Import { pptx, out } => {
            let out = out.unwrap_or_else(|| pptx.with_extension("deck"));
            if out.exists() {
                bail!("{} already exists", out.display());
            }
            let (deck, report) = pptx::import(&pptx, &out)?;
            for w in &report.warnings {
                eprintln!("warning: {w}");
            }
            println!("{}: {} slides written to {}", pptx.display(), deck.slides.len(), out.display());
        }
        Command::Export { deck, out } => {
            let out = out.unwrap_or_else(|| deck.with_extension("pptx"));
            let d = native::load(&deck)?;
            #[cfg(feature = "typst")]
            let warnings = pptx::export_with(&d, &deck, &out, &deck_typst::Engine::for_deck(&deck))?;
            #[cfg(not(feature = "typst"))]
            let warnings = pptx::export(&d, &deck, &out)?;
            for w in warnings {
                eprintln!("warning: {w}");
            }
            println!("{}: {} slides written to {}", deck.display(), d.slides.len(), out.display());
        }
        Command::Render { deck, out } => {
            let out = out.unwrap_or_else(|| deck.join("svg"));
            let d = native::load(&deck)?;
            std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
            #[cfg(feature = "typst")]
            let math: Box<dyn deck_core::MathRenderer> = Box::new(deck_typst::Engine::for_deck(&deck));
            #[cfg(not(feature = "typst"))]
            let math: Box<dyn deck_core::MathRenderer> = Box::new(deck_core::NoMath);
            for (i, slide) in d.slides.iter().enumerate() {
                let path = out.join(format!("slide{}.svg", i + 1));
                let mut text = svg::render_step_still(&d, slide, slide.last_step(), &*math);
                // Image hrefs are deck-relative; make them relative to the SVG.
                if let Ok(rel) = pathdiff(&deck, &out) {
                    text = text.replace(r#"xlink:href="assets/"#, &format!(r#"xlink:href="{rel}/assets/"#));
                }
                std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
            }
            println!("{} slides rendered to {}", d.slides.len(), out.display());
        }
        Command::Pdf { deck, out, steps, notes } => {
            #[cfg(not(feature = "typst"))]
            {
                let _ = (deck, out, steps, notes);
                bail!("PDF export needs deck-convert built with the typst feature: cargo build -p deck-convert --features typst");
            }
            #[cfg(feature = "typst")]
            {
                let out = out.unwrap_or_else(|| deck.with_extension("pdf"));
                let d = native::load(&deck)?;
                let engine = deck_typst::Engine::for_deck(&deck);
                let assets: std::collections::HashMap<String, String> = std::collections::HashMap::new();
                let _ = &assets;
                if notes {
                    engine.export_notes_pdf(
                        &d,
                        |i| {
                            inline_for_pdf(
                                &svg::render_step_still(&d, &d.slides[i], d.slides[i].last_step(), &engine),
                                &deck,
                            )
                        },
                        &out,
                    )?;
                    println!("{}: {} handout pages written to {}", deck.display(), d.slides.len(), out.display());
                    return Ok(());
                }
                let pages = deck_core::transfer::pages(&d, steps);
                engine.export_pdf_pages(
                    &d,
                    &pages,
                    |i, step| inline_for_pdf(&svg::render_step_still(&d, &d.slides[i], step, &engine), &deck),
                    &out,
                )?;
                println!("{}: {} pages written to {}", deck.display(), pages.len(), out.display());
            }
        }
        Command::Pack { deck, out } => {
            let out = out.unwrap_or_else(|| deck.with_extension(deck_convert::archive::EXTENSION));
            let n = deck_convert::archive::pack(&deck, &out)?;
            println!("{}: {n} files packed into {}", deck.display(), out.display());
        }
        Command::Unpack { archive, out } => {
            let out = out.unwrap_or_else(|| archive.with_extension("deck"));
            let n = deck_convert::archive::unpack(&archive, &out)?;
            println!("{}: {n} files unpacked into {}", archive.display(), out.display());
        }
        Command::Info { deck } => {
            let d = native::load(&deck)?;
            println!("{} ({} x {} pt, {} slides)", d.title, d.width, d.height, d.slides.len());
            for (i, s) in d.slides.iter().enumerate() {
                let title = s
                    .elements
                    .iter()
                    .find_map(|e| match e {
                        Element::Text(t) if !t.text().trim().is_empty() => {
                            Some(t.text().lines().next().unwrap_or("").to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();
                let notes = if s.notes.is_empty() { "" } else { " [notes]" };
                if let Some(sec) = &s.section {
                    println!("     ── {sec}");
                }
                if let Some(sub) = &s.subsection {
                    println!("        ── {sub}");
                }
                let steps = if s.last_step() == 0 { String::new() } else { format!(", {} steps", s.last_step()) };
                let layout = s.layout.as_deref().map(|l| format!(" [{l}]")).unwrap_or_default();
                println!("{:3}  {:<40} {} elements{steps}{notes}{layout}", i + 1, title, s.elements.len());
            }
        }
        Command::Layouts { deck } => {
            let d = native::load(&deck)?;
            if d.layouts.is_empty() {
                println!("no layouts");
            }
            for l in &d.layouts {
                let used = d.slides.iter().filter(|s| s.layout.as_deref() == Some(l.name.as_str())).count();
                println!("{} ({} static elements, used by {used} slides)", l.name, l.elements.len());
                for p in &l.placeholders {
                    println!(
                        "    {:<12} at {},{} {}x{}",
                        p.name,
                        p.rect.x.round(),
                        p.rect.y.round(),
                        p.rect.w.round(),
                        p.rect.h.round()
                    );
                }
            }
        }
        Command::Copy { from, slides, into, at } => {
            let src = native::load(&from)?;
            let mut dst = native::load(&into)?;
            let indices = parse_slides(&slides, src.slides.len())?;
            let at = at.map(|a| a.saturating_sub(1)).unwrap_or(dst.slides.len());
            let n = transfer::copy_slides(&src, &from, &indices, &mut dst, &into, at)?;
            native::save(&dst, &into)?;
            println!("{n} slides copied into {} ({} slides now)", into.display(), dst.slides.len());
        }
    }
    Ok(())
}

/// `3`, `2,5`, `4-7`, `all` → 0-based indices, in the order given.
fn parse_slides(spec: &str, count: usize) -> Result<Vec<usize>> {
    if spec == "all" {
        return Ok((0..count).collect());
    }
    let mut out = vec![];
    for part in spec.split(',') {
        let part = part.trim();
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim().parse::<usize>()?, b.trim().parse::<usize>()?),
            None => {
                let n = part.parse::<usize>()?;
                (n, n)
            }
        };
        if a == 0 || b < a || b > count {
            bail!("slide range {part} is outside 1..={count}");
        }
        out.extend((a - 1)..b);
    }
    Ok(out)
}

/// Images as data URIs, so that the PDF engine does not read the disk.
#[cfg(feature = "typst")]
fn inline_for_pdf(svg: &str, dir: &std::path::Path) -> String {
    let marker = r#"xlink:href="assets/"#;
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(i) = rest.find(marker) {
        out.push_str(&rest[..i]);
        let after = &rest[i + r#"xlink:href=""#.len()..];
        let Some(end) = after.find('"') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let src = &after[..end];
        let path = dir.join(src);
        let mime = match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("svg") => "image/svg+xml",
            Some("webp") => "image/webp",
            _ => "application/octet-stream",
        };
        match std::fs::read(&path) {
            Ok(bytes) => {
                use base64::Engine as _;
                out.push_str(&format!(
                    r#"xlink:href="data:{mime};base64,{}""#,
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ));
            }
            Err(_) => out.push_str(&rest[i..i + r#"xlink:href=""#.len() + end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// A relative path from `to` up to `from`'s directory, with `..` segments.
fn pathdiff(from: &std::path::Path, to: &std::path::Path) -> Result<String> {
    let from = from.canonicalize()?;
    let to = to.canonicalize()?;
    let mut ups = 0;
    let mut base = to.as_path();
    while !from.starts_with(base) {
        base = base.parent().context("no common ancestor")?;
        ups += 1;
    }
    let rest = from.strip_prefix(base)?;
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), ups).collect();
    parts.extend(rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    Ok(if parts.is_empty() { ".".into() } else { parts.join("/") })
}
