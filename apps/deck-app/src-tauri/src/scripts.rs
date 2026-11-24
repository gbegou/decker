//! Python macros: a script run by the Python found on the machine, with
//! the bundled `decker` package on its path, against the open deck. The
//! deck goes out as a JSON file; what the script hands back (through
//! `decker.current().commit()`, or at exit) replaces it as one undo step.
//! No interpreter is linked: without Python, everything else works.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

/// What running a script produced.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ScriptOutput {
    pub ok: bool,
    /// Whether the deck was changed by the script.
    pub changed: bool,
    pub stdout: String,
    pub stderr: String,
    /// The interpreter used.
    pub python: String,
}

/// The Python interpreter: `DECKER_PYTHON`, else the first of `python3`,
/// `python` on the path that runs.
pub fn find_python() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("DECKER_PYTHON") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    for name in ["python3", "python"] {
        if let Ok(out) = Command::new(name).arg("--version").output() {
            if out.status.success() {
                return Some(PathBuf::from(name));
            }
        }
    }
    None
}

/// The folder holding the `decker` Python package: the first that
/// exists among `DECKER_PYTHONPATH`, the app's bundled resources and,
/// for a development build, the repository's `python/` folder.
pub fn package_dir(resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    let mut candidates = vec![];
    if let Ok(p) = std::env::var("DECKER_PYTHONPATH") {
        candidates.push(PathBuf::from(p));
    }
    if let Some(r) = resource_dir {
        candidates.push(r.join("python"));
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../python"));
    candidates.into_iter().find(|p| p.join("decker").join("__init__.py").is_file())
}

/// Runs `script` with the deck JSON `deck_json`; `slide` and `selection`
/// tell the script where the user is. Returns the output and, when the
/// script committed, the JSON it handed back.
pub fn run(
    python: &Path,
    package_dir: &Path,
    script: &Path,
    deck_json: &str,
    deck_dir: &Path,
    slide: usize,
    selection: &[usize],
) -> Result<(ScriptOutput, Option<String>), String> {
    let work = std::env::temp_dir().join(format!("decker-script-{}-{}", std::process::id(), nanos()));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let input = work.join("deck.json");
    let result = work.join("result.json");
    std::fs::write(&input, deck_json).map_err(|e| e.to_string())?;
    let mut path = std::ffi::OsString::from(package_dir);
    if let Some(existing) = std::env::var_os("PYTHONPATH") {
        path.push(if cfg!(windows) { ";" } else { ":" });
        path.push(existing);
    }
    let output = Command::new(python)
        .arg("-u")
        .arg(script)
        .env("PYTHONPATH", path)
        .env("PYTHONIOENCODING", "utf-8")
        .env("DECKER_DECK", &input)
        .env("DECKER_RESULT", &result)
        .env("DECKER_DIR", deck_dir)
        .env("DECKER_SLIDE", slide.to_string())
        .env("DECKER_SELECTION", selection.iter().map(usize::to_string).collect::<Vec<_>>().join(","))
        .current_dir(script.parent().unwrap_or(Path::new(".")))
        .output()
        .map_err(|e| format!("cannot run {}: {e}", python.display()))?;
    let returned = std::fs::read_to_string(&result).ok();
    let _ = std::fs::remove_dir_all(&work);
    let out = ScriptOutput {
        ok: output.status.success(),
        changed: false,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        python: python.display().to_string(),
    };
    Ok((out, if output.status.success() { returned } else { None }))
}

fn nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

/// The `.py` files of `<deck_dir>/scripts/`, sorted: the deck's macros.
pub fn deck_scripts(deck_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(deck_dir.join("scripts")) else { return vec![] };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "py").unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_script_against_the_bundled_package() {
        let Some(python) = find_python() else {
            eprintln!("no python: skipped");
            return;
        };
        let pkg = package_dir(None).expect("the repository's python/ folder");
        let dir = std::env::temp_dir().join(format!("decker-scripts-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("scripts")).unwrap();
        std::fs::write(
            dir.join("scripts/upper.py"),
            "import decker\nd = decker.current()\nprint('hi', d.slide.index)\nd.title = d.title.upper()\n",
        )
        .unwrap();
        std::fs::write(dir.join("scripts/notes.txt"), "x").unwrap();
        assert_eq!(deck_scripts(&dir), ["upper.py"]);
        let deck = r#"{"version":1,"title":"t","width":960,"height":540,"slides":[{"elements":[]}]}"#;
        let (out, returned) = run(&python, &pkg, &dir.join("scripts/upper.py"), deck, &dir, 0, &[]).unwrap();
        assert!(out.ok, "{}", out.stderr);
        assert_eq!(out.stdout.trim(), "hi 0");
        let returned: serde_json::Value = serde_json::from_str(&returned.expect("committed at exit")).unwrap();
        assert_eq!(returned["deck"]["title"], "T");
        let (out, returned) = run(&python, &pkg, &dir.join("scripts/missing.py"), deck, &dir, 0, &[]).unwrap();
        assert!(!out.ok && returned.is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
