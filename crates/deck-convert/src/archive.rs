//! The single-file deck: a `.deckz` file is a zip of the deck folder
//! (`deck.json`, `assets/`, `typst/`, `fonts/`, `scripts/`), convenient
//! to send. The folder stays the working format: an archive is unpacked
//! to be edited and packed again to be shared.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};

/// The extension of a packed deck.
pub const EXTENSION: &str = "deckz";

/// Whether `path` names a packed deck.
pub fn is_archive(path: &Path) -> bool {
    path.extension().map(|e| e.eq_ignore_ascii_case(EXTENSION)).unwrap_or(false)
}

/// Packs the deck folder `dir` into the file `out`. Returns the number
/// of files written.
pub fn pack(dir: &Path, out: &Path) -> Result<usize> {
    if !dir.join(deck_core::native::DECK_FILE).is_file() {
        bail!("{} is not a deck folder (no deck.json)", dir.display());
    }
    let mut files = vec![];
    collect(dir, dir, &mut files)?;
    files.sort();
    let file = std::fs::File::create(out).with_context(|| format!("creating {}", out.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for rel in &files {
        let name = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
        zip.start_file(name, options)?;
        let bytes = std::fs::read(dir.join(rel)).with_context(|| format!("reading {}", rel.display()))?;
        zip.write_all(&bytes)?;
    }
    zip.finish()?;
    Ok(files.len())
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Hidden files and caches stay out of the archive.
        if name.starts_with('.') || name == "__pycache__" {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_path_buf());
        }
    }
    Ok(())
}

/// Unpacks the archive `file` into the folder `dir` (created; it must
/// not hold a deck already). Entries that would land outside `dir` are
/// refused. Returns the number of files written.
pub fn unpack(file: &Path, dir: &Path) -> Result<usize> {
    if dir.join(deck_core::native::DECK_FILE).exists() {
        bail!("{} already holds a deck", dir.display());
    }
    let reader = std::fs::File::open(file).with_context(|| format!("opening {}", file.display()))?;
    let mut zip = zip::ZipArchive::new(reader).with_context(|| format!("{} is not a deck archive", file.display()))?;
    if zip.by_name(deck_core::native::DECK_FILE).is_err() {
        bail!("{} is not a deck archive (no deck.json inside)", file.display());
    }
    let mut written = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let rel = PathBuf::from(entry.name());
        if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
            bail!("unsafe path in the archive: {}", entry.name());
        }
        let target = dir.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        std::fs::write(&target, bytes).with_context(|| format!("writing {}", target.display()))?;
        written += 1;
    }
    Ok(written)
}

/// The folder an archive is unpacked to for editing: `<name>.deck` next
/// to it, or `<name>-2.deck`… when that one is taken by another deck.
pub fn working_folder(file: &Path) -> PathBuf {
    let base = file.with_extension("deck");
    if !base.exists() {
        return base;
    }
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "deck".into());
    let mut n = 2;
    loop {
        let candidate = file.with_file_name(format!("{stem}-{n}.deck"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deck_core::{native, Deck, Slide};

    #[test]
    fn packs_and_unpacks_a_deck() {
        let base = std::env::temp_dir().join(format!("decker-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("talk.deck");
        let mut deck = Deck::new("zipped");
        deck.slides.push(Slide::default());
        native::save(&deck, &dir).unwrap();
        native::add_asset(&dir, "a.png", b"png").unwrap();
        std::fs::create_dir_all(dir.join("scripts/__pycache__")).unwrap();
        std::fs::write(dir.join("scripts/m.py"), "print(1)").unwrap();
        std::fs::write(dir.join("scripts/__pycache__/m.pyc"), "x").unwrap();
        std::fs::write(dir.join(".hidden"), "x").unwrap();
        let file = base.join("talk.deckz");
        assert!(is_archive(&file) && !is_archive(&dir));
        assert_eq!(pack(&dir, &file).unwrap(), 3, "deck.json, the asset and the script");
        assert_eq!(working_folder(&file), base.join("talk-2.deck"), "talk.deck is taken");
        let out = base.join("out.deck");
        assert_eq!(unpack(&file, &out).unwrap(), 3);
        assert_eq!(native::load(&out).unwrap(), deck);
        assert_eq!(std::fs::read(out.join("assets/a.png")).unwrap(), b"png");
        assert!(unpack(&file, &out).is_err(), "never over an existing deck");
        assert!(pack(&base, &file).is_err(), "not a deck folder");
        std::fs::write(base.join("bad.deckz"), b"not a zip").unwrap();
        assert!(unpack(&base.join("bad.deckz"), &base.join("bad.deck")).is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
