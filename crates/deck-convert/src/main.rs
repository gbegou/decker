//! Converter between decker's native format and PPTX / SVG.
//!
//! ```text
//! deck-convert import talk.pptx -o talk.deck     # PPTX → native folder
//! deck-convert export talk.deck -o talk.pptx     # native folder → PPTX
//! deck-convert render talk.deck -o svg/          # one SVG per slide
//! deck-convert info talk.deck                    # slide titles and counts
//! ```

mod pptx;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use deck_core::{native, svg, Element};

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
    /// Print a summary of a deck folder.
    Info { deck: PathBuf },
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
            for w in pptx::export(&d, &deck, &out)? {
                eprintln!("warning: {w}");
            }
            println!("{}: {} slides written to {}", deck.display(), d.slides.len(), out.display());
        }
        Command::Render { deck, out } => {
            let out = out.unwrap_or_else(|| deck.join("svg"));
            let d = native::load(&deck)?;
            std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
            for (i, slide) in d.slides.iter().enumerate() {
                let path = out.join(format!("slide{}.svg", i + 1));
                let mut text = svg::render_slide(&d, slide);
                // Image hrefs are deck-relative; make them relative to the SVG.
                if let Ok(rel) = pathdiff(&deck, &out) {
                    text = text.replace(r#"xlink:href="assets/"#, &format!(r#"xlink:href="{rel}/assets/"#));
                }
                std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
            }
            println!("{} slides rendered to {}", d.slides.len(), out.display());
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
                let steps = if s.last_step() == 0 { String::new() } else { format!(", {} steps", s.last_step()) };
                println!("{:3}  {:<40} {} elements{steps}{notes}", i + 1, title, s.elements.len());
            }
        }
    }
    Ok(())
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
