//! The decker application: one editor window, and during a presentation an
//! audience window (fullscreen on the projector) and a presenter window.
//!
//! The Rust side is the single source of truth: it holds the deck, the
//! current slide and build step, renders SVG, and emits `view-changed`
//! after every change. Every window then calls `get_view`.

mod state;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use deck_core::{Crop, Deck, Mask, Rect};
use state::{AppState, TextLayout, View};

type Shared<'a> = State<'a, Mutex<AppState>>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Navigation or a blank screen: windows re-fetch the view.
fn changed(app: &AppHandle) {
    let _ = app.emit("view-changed", ());
}

/// Content changed: windows also re-fetch the thumbnails.
fn edited(app: &AppHandle) {
    let _ = app.emit("view-changed", ());
    let _ = app.emit("deck-changed", ());
}

#[tauri::command]
fn get_thumbnails(state: Shared) -> Result<Vec<String>, String> {
    Ok(state.lock().map_err(err)?.thumbnails())
}

/// Opens a `.deck` folder or imports a `.pptx` next to it.
#[tauri::command]
fn open(app: AppHandle, state: Shared, path: String) -> Result<View, String> {
    let path = PathBuf::from(path);
    let view = {
        let mut s = state.lock().map_err(err)?;
        if path.extension().map(|e| e.eq_ignore_ascii_case("pptx")).unwrap_or(false) {
            let dir = path.with_extension("deck");
            if dir.exists() {
                s.load(deck_core::native::load(&dir).map_err(err)?, dir);
            } else {
                let (deck, report) = deck_convert::pptx::import(&path, &dir).map_err(err)?;
                s.load(deck, dir);
                s.warnings = report.warnings;
            }
        } else {
            s.load(deck_core::native::load(&path).map_err(err)?, path);
        }
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// A new empty deck with one slide, not saved until `save_as`.
#[tauri::command]
fn new_deck(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        let mut deck = Deck::new("Untitled");
        deck.slides.push(deck_core::Slide::default());
        s.load(deck, PathBuf::new());
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn save(state: Shared, path: Option<String>) -> Result<String, String> {
    let mut s = state.lock().map_err(err)?;
    if let Some(p) = path {
        s.dir = PathBuf::from(p);
    }
    if s.dir.as_os_str().is_empty() {
        return Err("no file name: use Save as".into());
    }
    let deck = s.deck.as_ref().ok_or("no deck open")?;
    deck_core::native::save(deck, &s.dir).map_err(err)?;
    Ok(s.dir.display().to_string())
}

#[tauri::command]
fn export_pptx(state: Shared, path: String) -> Result<Vec<String>, String> {
    let s = state.lock().map_err(err)?;
    let deck = s.deck.as_ref().ok_or("no deck open")?;
    deck_convert::pptx::export(deck, &s.dir, &PathBuf::from(path)).map_err(err)
}

#[tauri::command]
fn export_pdf(state: Shared, path: String) -> Result<(), String> {
    let mut s = state.lock().map_err(err)?;
    s.export_pdf(&PathBuf::from(path))
}

#[tauri::command]
fn set_section(
    app: AppHandle,
    state: Shared,
    section: Option<String>,
    subsection: Option<String>,
) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.set_section(section, subsection)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Sets the section and subsection a given slide starts.
#[tauri::command]
fn set_section_of(
    app: AppHandle,
    state: Shared,
    slide: usize,
    section: Option<String>,
    subsection: Option<String>,
) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.set_section_of(slide, section, subsection)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn get_view(state: Shared) -> Result<View, String> {
    Ok(state.lock().map_err(err)?.view())
}

/// `next`, `prev`, `first`, `last`, `next_slide`, `prev_slide`, `black`, `white`.
#[tauri::command]
fn navigate(app: AppHandle, state: Shared, action: String) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.navigate(&action);
        s.view()
    };
    changed(&app);
    Ok(view)
}

#[tauri::command]
fn goto(app: AppHandle, state: Shared, slide: usize, step: Option<u32>) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.goto(slide, step.unwrap_or(0));
        s.view()
    };
    changed(&app);
    Ok(view)
}

/// Editor: move or resize an element of the current slide.
#[tauri::command]
fn set_rect(app: AppHandle, state: Shared, element: usize, rect: Rect) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.set_rect(element, rect)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Editor: replace the text of a text box or shape (one paragraph per line).
#[tauri::command]
fn set_text(app: AppHandle, state: Shared, element: usize, text: String) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.set_text(element, &text)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Character boxes of a text element of the current slide (`None` without Typst).
#[tauri::command]
fn text_layout(state: Shared, element: usize) -> Result<Option<TextLayout>, String> {
    state.lock().map_err(err)?.text_layout(element)
}

/// Replaces `from..to` by `insert` in a text element; returns the new caret.
#[tauri::command]
fn text_edit(
    app: AppHandle,
    state: Shared,
    element: usize,
    from: usize,
    to: usize,
    insert: String,
) -> Result<usize, String> {
    let caret = {
        let mut s = state.lock().map_err(err)?;
        s.text_edit(element, from, to, &insert)?
    };
    edited(&app);
    Ok(caret)
}

#[tauri::command]
fn set_image(app: AppHandle, state: Shared, element: usize, crop: Crop, mask: Mask) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.set_image(element, crop, mask)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn set_notes(app: AppHandle, state: Shared, notes: String) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.set_notes(notes)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn add_slide(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.add_slide()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn delete_slide(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.delete_slide()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn duplicate_slide(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.duplicate_slide()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn set_layout(app: AppHandle, state: Shared, name: Option<String>) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.set_layout(name.filter(|n| !n.is_empty()))?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Thumbnails of the slides of another deck folder.
#[tauri::command]
fn peek_deck(state: Shared, path: String) -> Result<Vec<String>, String> {
    let s = state.lock().map_err(err)?;
    s.peek(&deck_dir(&path))
}

/// Copies slides (0-based indices) of another deck folder after the current slide.
#[tauri::command]
fn import_slides(app: AppHandle, state: Shared, path: String, indices: Vec<usize>) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.import_slides(&deck_dir(&path), &indices)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// A deck folder, from the folder or its `deck.json`.
fn deck_dir(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.file_name().map(|f| f == "deck.json").unwrap_or(false) {
        p.parent().map(Path::to_path_buf).unwrap_or(p)
    } else {
        p
    }
}

#[tauri::command]
fn add_text(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.add_text()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn delete_element(app: AppHandle, state: Shared, element: usize) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.delete_element(element)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Opens the audience window fullscreen on the second monitor (or the only
/// one) and the presenter window on the primary monitor when there are two.
#[tauri::command]
fn start_presentation(app: AppHandle, state: Shared, with_presenter: Option<bool>) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(err)?;
        if s.deck.is_none() {
            return Err("no deck open".into());
        }
        s.presenting = true;
        s.step = 0;
    }
    let editor = app.get_webview_window("editor").ok_or("no editor window")?;
    let monitors = editor.available_monitors().map_err(err)?;
    let primary = editor.primary_monitor().map_err(err)?.or_else(|| monitors.first().cloned());
    let secondary = monitors.iter().find(|m| Some(m.position()) != primary.as_ref().map(|p| p.position())).cloned();

    // `DECKER_WINDOWED=1`: a normal, decorated, non-fullscreen audience
    // window, to tell a page problem from a fullscreen painting problem.
    let windowed = std::env::var("DECKER_WINDOWED").map(|v| v == "1").unwrap_or(false);
    let audience_monitor = secondary.clone().or(primary.clone());
    let open_presenter = secondary.is_some() || with_presenter == Some(true);
    eprintln!(
        "[decker] presenting: {} monitor(s), windowed={windowed}, audience on {:?}",
        monitors.len(),
        audience_monitor.as_ref().and_then(|m| m.name().cloned())
    );
    let monitor_rect = |m: &tauri::Monitor| {
        let (p, sz, scale) = (m.position(), m.size(), m.scale_factor());
        (p.x as f64 / scale, p.y as f64 / scale, sz.width as f64 / scale, sz.height as f64 / scale)
    };

    // The windows exist since startup, hidden (see `run`): showing them is
    // all that is left, which also makes presenting instant.
    let audience = app.get_webview_window("audience").ok_or("no audience window")?;
    let _ = audience.set_decorations(windowed);
    if windowed {
        let _ = audience.set_size(tauri::LogicalSize::new(960.0, 540.0));
    } else if let Some((x, y, w, h)) = audience_monitor.as_ref().map(monitor_rect) {
        let _ = audience.set_position(tauri::LogicalPosition::new(x, y));
        let _ = audience.set_size(tauri::LogicalSize::new(w, h));
    }
    audience.show().map_err(err)?;
    if !windowed {
        let _ = audience.set_fullscreen(true);
    }
    let _ = audience.set_focus();
    if open_presenter {
        let presenter = app.get_webview_window("presenter").ok_or("no presenter window")?;
        if let Some((x, y, w, h)) = primary.as_ref().map(monitor_rect) {
            let _ = presenter.set_position(tauri::LogicalPosition::new(x + 40.0, y + 40.0));
            let _ = presenter.set_size(tauri::LogicalSize::new((w - 80.0).max(600.0), (h - 120.0).max(400.0)));
        }
        presenter.show().map_err(err)?;
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
fn stop_presentation(app: AppHandle, state: Shared) -> Result<(), String> {
    {
        let mut s = state.lock().map_err(err)?;
        s.presenting = false;
        s.blank.clear();
    }
    for label in ["audience", "presenter"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_fullscreen(false);
            let _ = w.hide();
        }
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
fn monitors(app: AppHandle) -> Result<Vec<String>, String> {
    let editor = app.get_webview_window("editor").ok_or("no editor window")?;
    Ok(editor
        .available_monitors()
        .map_err(err)?
        .iter()
        .map(|m| format!("{} {}x{}", m.name().cloned().unwrap_or_default(), m.size().width, m.size().height))
        .collect())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `DECKER_OPEN=<path>` opens a deck or pptx at startup (also used by the
    // smoke test under a virtual display).
    let mut initial = AppState::default();
    if let Ok(path) = std::env::var("DECKER_OPEN") {
        let path = PathBuf::from(path);
        if path.extension().map(|e| e.eq_ignore_ascii_case("pptx")).unwrap_or(false) {
            let dir = path.with_extension("deck");
            if let Ok((deck, report)) = deck_convert::pptx::import(&path, &dir) {
                initial.load(deck, dir);
                initial.warnings = report.warnings;
            }
        } else if let Ok(deck) = deck_core::native::load(&path) {
            initial.load(deck, path);
        }
    }
    // `DECKER_PRESENT=1` starts the show at once (`=both` also opens the
    // presenter window on a single monitor).
    let present = std::env::var("DECKER_PRESENT").ok();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(initial))
        .setup(move |app| {
            // The presentation windows, hidden until "Present". Created here,
            // at startup, like the editor window: on Windows a webview created
            // later never loaded its page.
            for (label, title) in [("audience", "decker — presentation"), ("presenter", "decker — presenter")] {
                let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
                    .title(title)
                    .visible(false)
                    .inner_size(960.0, 540.0)
                    .on_page_load(|w, payload| {
                        eprintln!("[decker] window {}: page load {:?} {}", w.label(), payload.event(), payload.url());
                    })
                    .build();
                if let Err(e) = built {
                    eprintln!("[decker] cannot create the {label} window: {e}");
                }
            }
            if let Some(mode) = present {
                let handle = app.handle().clone();
                let state = handle.state::<Mutex<AppState>>();
                if let Err(e) = start_presentation(handle.clone(), state, Some(mode == "both")) {
                    eprintln!("cannot start the presentation: {e}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open,
            new_deck,
            save,
            export_pptx,
            export_pdf,
            set_section,
            set_section_of,
            get_view,
            get_thumbnails,
            navigate,
            goto,
            set_rect,
            set_text,
            set_notes,
            set_image,
            text_layout,
            text_edit,
            add_slide,
            delete_slide,
            duplicate_slide,
            set_layout,
            peek_deck,
            import_slides,
            add_text,
            delete_element,
            start_presentation,
            stop_presentation,
            monitors
        ])
        .run(tauri::generate_context!())
        .expect("error while running decker");
}
