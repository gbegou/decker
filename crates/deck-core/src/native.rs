//! The native format: a folder `<name>.deck/` with `deck.json` and `assets/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::model::{Deck, FORMAT_VERSION};

pub const DECK_FILE: &str = "deck.json";
pub const ASSETS_DIR: &str = "assets";

#[derive(Debug, thiserror::Error)]
pub enum NativeError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{path}: invalid deck.json: {source}")]
    Json { path: PathBuf, source: serde_json::Error },
    #[error("{path}: format version {found} is newer than the supported {supported}")]
    Version { path: PathBuf, found: u32, supported: u32 },
}

/// Reads `<dir>/deck.json`.
pub fn load(dir: &Path) -> Result<Deck, NativeError> {
    let path = dir.join(DECK_FILE);
    let text = fs::read_to_string(&path).map_err(|source| NativeError::Io { path: path.clone(), source })?;
    let mut deck: Deck =
        serde_json::from_str(&text).map_err(|source| NativeError::Json { path: path.clone(), source })?;
    if deck.version > FORMAT_VERSION {
        return Err(NativeError::Version { path, found: deck.version, supported: FORMAT_VERSION });
    }
    deck.migrate();
    Ok(deck)
}

/// Writes `<dir>/deck.json`, creating `<dir>` and `<dir>/assets/` if needed.
/// Existing assets are kept.
pub fn save(deck: &Deck, dir: &Path) -> Result<(), NativeError> {
    fn io(path: &Path) -> impl FnOnce(io::Error) -> NativeError + '_ {
        move |source| NativeError::Io { path: path.to_path_buf(), source }
    }
    let assets = dir.join(ASSETS_DIR);
    fs::create_dir_all(&assets).map_err(io(&assets))?;
    let path = dir.join(DECK_FILE);
    let text = serde_json::to_string_pretty(deck).map_err(|source| NativeError::Json { path: path.clone(), source })?;
    fs::write(&path, text + "\n").map_err(io(&path))
}

/// Copies bytes into `<dir>/assets/<name>` and returns the deck-relative
/// path to store in an [`crate::Image`].
pub fn add_asset(dir: &Path, name: &str, bytes: &[u8]) -> Result<String, NativeError> {
    let assets = dir.join(ASSETS_DIR);
    fs::create_dir_all(&assets).map_err(|source| NativeError::Io { path: assets.clone(), source })?;
    let path = assets.join(name);
    fs::write(&path, bytes).map_err(|source| NativeError::Io { path: path.clone(), source })?;
    Ok(format!("{ASSETS_DIR}/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn save_then_load() {
        let dir = std::env::temp_dir().join(format!("decker-native-{}", std::process::id()));
        let mut deck = Deck::new("Round trip");
        deck.slides.push(Slide::default());
        save(&deck, &dir).unwrap();
        let src = add_asset(&dir, "a.png", b"png").unwrap();
        assert_eq!(src, "assets/a.png");
        assert!(dir.join("assets/a.png").is_file());
        assert_eq!(load(&dir).unwrap(), deck);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn newer_version_is_refused() {
        let dir = std::env::temp_dir().join(format!("decker-version-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(DECK_FILE), r#"{"version":999,"title":"","width":1,"height":1,"slides":[]}"#).unwrap();
        assert!(matches!(load(&dir), Err(NativeError::Version { found: 999, .. })));
        fs::remove_dir_all(&dir).unwrap();
    }
}
