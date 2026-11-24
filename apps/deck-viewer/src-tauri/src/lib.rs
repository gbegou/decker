//! decker viewer: presents a PDF (any PDF: decker's export, Beamer,
//! PowerPoint…) with the audience window and the presenter view of
//! decker, without the editor. Pages are rendered by pdf.js in the web
//! view; this side keeps the show's position, the notes and the windows.
//!
//! Notes come from a Markdown file next to the PDF, `<name>.notes.md`,
//! one section per page: `# 3` (or `## 3`) starts the notes of page 3.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct Viewer {
    pdf: Option<PathBuf>,
    pages: usize,
    /// Notes per page, from the sidecar file.
    notes: Vec<String>,
    show: deck_present::Show,
    /// The page's aspect ratio, reported by the page loader.
    width: f64,
    height: f64,
}

/// The view the windows read: the show protocol's fields plus the PDF.
#[derive(Debug, Clone, Serialize)]
struct View {
    dir: String,
    width: f64,
    height: f64,
    slide: usize,
    slide_count: usize,
    step: u32,
    last_step: u32,
    svg: Option<String>,
    next_svg: Option<String>,
    notes: String,
    presenting: bool,
    blank: String,
    /// The PDF's absolute path, for the page renderer; null before one is opened.
    pdf: Option<String>,
}

impl Viewer {
    fn view(&self) -> View {
        View {
            dir: self.pdf.as_ref().and_then(|p| p.parent()).map(|p| p.display().to_string()).unwrap_or_default(),
            width: if self.width > 0.0 { self.width } else { 960.0 },
            height: if self.height > 0.0 { self.height } else { 540.0 },
            slide: self.show.page,
            slide_count: self.pages,
            step: 0,
            last_step: 0,
            svg: None,
            next_svg: None,
            notes: self.notes.get(self.show.page).cloned().unwrap_or_default(),
            presenting: self.show.presenting,
            blank: self.show.blank.clone(),
            pdf: self.pdf.as_ref().map(|p| p.display().to_string()),
        }
    }

    fn steps(&self) -> Vec<u32> {
        vec![0; self.pages]
    }
}

/// The notes of each page from `<pdf>.notes.md`: `# N` headings split
/// the pages; text before the first heading goes to page 1.
fn read_notes(pdf: &Path, pages: usize) -> Vec<String> {
    let mut notes = vec![String::new(); pages];
    let path = pdf.with_extension("notes.md");
    let Ok(text) = std::fs::read_to_string(&path) else { return notes };
    let mut page = 0usize;
    for line in text.lines() {
        let heading = line.trim_start_matches('#');
        if line.starts_with('#') && heading.trim().parse::<usize>().is_ok() {
            page = heading.trim().parse::<usize>().unwrap_or(1).saturating_sub(1);
            continue;
        }
        if let Some(n) = notes.get_mut(page) {
            if !n.is_empty() || !line.trim().is_empty() {
                n.push_str(line);
                n.push('\n');
            }
        }
    }
    notes.iter_mut().for_each(|n| *n = n.trim_end().to_string());
    notes
}

type Shared<'a> = State<'a, Mutex<Viewer>>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn changed(app: &AppHandle) {
    let _ = app.emit("view-changed", ());
}

#[tauri::command]
fn get_view(state: Shared) -> Result<View, String> {
    Ok(state.lock().map_err(err)?.view())
}

/// Opens a PDF: the page loader tells how many pages it has and their
/// size in points once it has parsed it (`set_pages`).
#[tauri::command]
fn open(app: AppHandle, state: Shared, path: String) -> Result<View, String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err(format!("no such file: {path}"));
    }
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.pdf = Some(p);
        s.pages = 0;
        s.notes.clear();
        s.show = deck_present::Show::default();
        s.view()
    };
    changed(&app);
    Ok(view)
}

/// What the page loader found: the page count and the first page's size.
#[tauri::command]
fn set_pages(app: AppHandle, state: Shared, pages: usize, width: f64, height: f64) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.pages = pages;
        s.width = width;
        s.height = height;
        s.notes = s.pdf.as_deref().map(|p| read_notes(p, pages)).unwrap_or_default();
        let (page, steps) = (s.show.page, s.steps());
        s.show.goto(page, 0, &steps);
        s.view()
    };
    changed(&app);
    Ok(view)
}

#[tauri::command]
fn navigate(app: AppHandle, state: Shared, action: String) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        let steps = s.steps();
        s.show.navigate(&action, &steps);
        s.view()
    };
    changed(&app);
    Ok(view)
}

#[tauri::command]
fn goto(app: AppHandle, state: Shared, slide: usize, step: Option<u32>) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        let steps = s.steps();
        s.show.goto(slide, step.unwrap_or(0), &steps);
        s.view()
    };
    changed(&app);
    Ok(view)
}

#[tauri::command]
fn start_presentation(app: AppHandle, state: Shared, with_presenter: Option<bool>) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(err)?;
        if s.pdf.is_none() {
            return Err("open a PDF first".into());
        }
        s.show.presenting = true;
    }
    deck_present::start(&app, "viewer", with_presenter)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn stop_presentation(app: AppHandle, state: Shared) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(err)?;
        s.show.presenting = false;
        s.show.blank.clear();
    }
    deck_present::stop(&app);
    changed(&app);
    Ok(())
}

#[tauri::command]
fn monitors(app: AppHandle) -> Result<Vec<String>, String> {
    deck_present::monitors(&app, "viewer")
}

/// Rewrites the notes file of the open PDF.
#[tauri::command]
fn set_notes(app: AppHandle, state: Shared, page: usize, text: String) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        let pdf = s.pdf.clone().ok_or("no PDF open")?;
        if page < s.notes.len() {
            s.notes[page] = text;
        }
        let mut out = String::new();
        for (i, n) in s.notes.iter().enumerate() {
            if !n.trim().is_empty() {
                out.push_str(&format!("# {}\n{}\n\n", i + 1, n.trim_end()));
            }
        }
        std::fs::write(pdf.with_extension("notes.md"), out).map_err(err)?;
        s.view()
    };
    changed(&app);
    Ok(view)
}

pub fn run() {
    let mut initial = Viewer::default();
    // `DECKER_OPEN=<file.pdf>` opens a PDF at startup.
    if let Ok(path) = std::env::var("DECKER_OPEN") {
        let p = PathBuf::from(path);
        if p.is_file() {
            initial.pdf = Some(p);
        }
    }
    let present = deck_present::present_at_launch();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(initial))
        .setup(move |app| {
            deck_present::create_windows(app.handle(), "decker viewer");
            if let Some(mode) = present {
                // The page loader has not reported the pages yet; the windows
                // show them as soon as it does.
                let handle = app.handle().clone();
                let state = handle.state::<Mutex<Viewer>>();
                if let Ok(mut s) = state.lock() {
                    s.show.presenting = true;
                }
                if let Err(e) = deck_present::start(&handle, "viewer", Some(mode == "both")) {
                    eprintln!("cannot start the presentation: {e}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            open,
            set_pages,
            navigate,
            goto,
            start_presentation,
            stop_presentation,
            monitors,
            set_notes
        ])
        .run(tauri::generate_context!())
        .expect("error while running decker viewer");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_split_by_page_headings() {
        let dir = std::env::temp_dir().join(format!("decker-viewer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("talk.pdf");
        std::fs::write(&pdf, b"%PDF").unwrap();
        assert_eq!(read_notes(&pdf, 2), ["", ""]);
        std::fs::write(dir.join("talk.notes.md"), "intro line\n# 2\nsecond\npage\n\n## 4\nfourth\n").unwrap();
        assert_eq!(read_notes(&pdf, 4), ["intro line", "second\npage", "", "fourth"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
