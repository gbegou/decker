//! The decker application: one editor window, and during a presentation an
//! audience window (fullscreen on the projector) and a presenter window.
//!
//! The Rust side is the single source of truth: it holds the deck, the
//! current slide and build step, renders SVG, and emits `view-changed`
//! after every change. Every window then calls `get_view`.

mod scripts;
mod state;

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};

use deck_core::text::{ParagraphChange, StyleChange, StyleInfo};
use deck_core::{Crop, Deck, Mask, Rect};
use state::{AppState, ElementStyle, PlaceholderStyle, TableEdit, TextLayout, ThemeChange, View};

type Shared<'a> = State<'a, Mutex<AppState>>;

/// The system clipboard, kept alive for the whole run: on X11 the copied
/// contents are served by the owning process and vanish when the handle
/// is dropped.
struct SystemClipboard(Mutex<Option<arboard::Clipboard>>);

impl SystemClipboard {
    fn with<T>(&self, f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.0.lock().map_err(err)?;
        if guard.is_none() {
            *guard = Some(arboard::Clipboard::new().map_err(err)?);
        }
        f(guard.as_mut().expect("clipboard opened"))
    }
}

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
        s.archive = None;
        if deck_convert::archive::is_archive(&path) {
            let (deck, dir) = open_archive(&path)?;
            s.load(deck, dir);
            s.archive = Some(path);
        } else if path.extension().map(|e| e.eq_ignore_ascii_case("pptx")).unwrap_or(false) {
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
/// Unpacks a `.deckz` file into a fresh working folder (in the temporary
/// directory: the file is the deck, saving packs it again).
fn open_archive(file: &std::path::Path) -> Result<(Deck, PathBuf), String> {
    let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "deck".into());
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("decker-{}-{nanos}", std::process::id())).join(format!("{stem}.deck"));
    deck_convert::archive::unpack(file, &dir).map_err(err)?;
    Ok((deck_core::native::load(&dir).map_err(err)?, dir))
}

#[tauri::command]
fn new_deck(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        let mut deck = Deck::new("Untitled");
        deck.slides.push(deck_core::Slide::default());
        s.load(deck, PathBuf::new());
        s.archive = None;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn save(state: Shared, path: Option<String>) -> Result<String, String> {
    let mut s = state.lock().map_err(err)?;
    // Saving to a `.deckz` name: the working folder is saved, then packed
    // into that file; a plain Save of a deck opened from one does the same.
    let chosen = path.map(PathBuf::from);
    let archive = match &chosen {
        Some(p) if deck_convert::archive::is_archive(p) => Some(p.clone()),
        Some(_) => None,
        None => s.archive.clone(),
    };
    let dst = match (&chosen, &archive) {
        (Some(p), None) => p.clone(),
        // An archive needs a folder to pack: the current one, else a temporary one.
        _ if !s.dir.as_os_str().is_empty() => s.dir.clone(),
        (_, Some(a)) => {
            let stem = a.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "deck".into());
            std::env::temp_dir().join(format!("decker-{}-save", std::process::id())).join(format!("{stem}.deck"))
        }
        _ => PathBuf::new(),
    };
    if dst.as_os_str().is_empty() {
        return Err("no file name: use Save as".into());
    }
    let deck = s.deck.take().ok_or("no deck open")?;
    // Saving elsewhere copies the images, fonts and Typst files along.
    let saved = deck_core::transfer::save_as(&deck, &s.dir, &dst);
    match saved {
        Ok(d) => {
            // Files no element uses any more (deleted pictures) go with the save.
            if let Ok(removed) = deck_core::transfer::prune_assets(&d, &dst) {
                for name in &removed {
                    s.assets.remove(&format!("assets/{name}"));
                }
            }
            s.deck = Some(d);
            s.dir = dst;
            s.archive = archive;
            match &s.archive {
                Some(file) => {
                    deck_convert::archive::pack(&s.dir, file).map_err(err)?;
                    Ok(file.display().to_string())
                }
                None => Ok(s.dir.display().to_string()),
            }
        }
        Err(e) => {
            s.deck = Some(deck);
            Err(err(e))
        }
    }
}

/// Adds an image from pasted bytes (base64) to the current slide.
#[tauri::command]
fn add_image(
    app: AppHandle,
    state: Shared,
    name: String,
    data: String,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<usize, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(err)?;
    let index = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("add_image", false);
        s.add_image(&name, &bytes, x.zip(y))?
    };
    edited(&app);
    Ok(index)
}

/// Copies elements of the current slide to the system clipboard.
#[tauri::command]
fn copy_elements(state: Shared, clipboard: State<SystemClipboard>, indices: Vec<usize>) -> Result<(), String> {
    let text = state.lock().map_err(err)?.copy_elements(&indices)?;
    clipboard.with(|c| c.set_text(text).map_err(err))
}

/// What a paste did: elements or an image added (their indices), or the
/// clipboard's text for the box being edited.
#[derive(Debug, Default, serde::Serialize)]
struct Pasted {
    added: Vec<usize>,
    text: Option<String>,
}

/// Pastes the system clipboard: copied elements, else an image, else
/// text, which is returned for the caller to insert. The clipboard is
/// read on the Rust side because WebKitGTK hands the page no image.
#[tauri::command]
fn paste(app: AppHandle, state: Shared, clipboard: State<SystemClipboard>) -> Result<Pasted, String> {
    let (text, png) = clipboard.with(|c| {
        let text = c.get_text().ok();
        let png = if text.is_some() { None } else { clipboard_png(c)? };
        Ok((text, png))
    })?;
    if let Some(clip) = text.as_deref().and_then(deck_core::transfer::Clip::parse) {
        let added = {
            let mut s = state.lock().map_err(err)?;
            s.checkpoint("paste", false);
            s.paste_elements(clip)?
        };
        edited(&app);
        return Ok(Pasted { added, text: None });
    }
    if let Some(png) = png {
        let index = {
            let mut s = state.lock().map_err(err)?;
            s.checkpoint("paste", false);
            s.add_image("pasted.png", &png, None)?
        };
        edited(&app);
        return Ok(Pasted { added: vec![index], text: None });
    }
    Ok(Pasted { added: vec![], text: text.filter(|t| !t.is_empty()) })
}

/// The clipboard's image as PNG bytes, `None` when it holds none.
fn clipboard_png(clipboard: &mut arboard::Clipboard) -> Result<Option<Vec<u8>>, String> {
    let image = match clipboard.get_image() {
        Ok(i) => i,
        Err(arboard::Error::ContentNotAvailable) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let (w, h) = (u32::try_from(image.width).map_err(err)?, u32::try_from(image.height).map_err(err)?);
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(err)?;
        writer.write_image_data(&image.bytes).map_err(err)?;
    }
    Ok(Some(out))
}

/// Adds an image from a file dropped on the editor.
#[tauri::command]
fn add_image_file(
    app: AppHandle,
    state: Shared,
    path: String,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<usize, String> {
    let p = PathBuf::from(&path);
    let bytes = std::fs::read(&p).map_err(err)?;
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("image.png");
    let index = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("add_image_file", false);
        s.add_image(name, &bytes, x.zip(y))?
    };
    edited(&app);
    Ok(index)
}

#[tauri::command]
fn export_pptx(state: Shared, path: String) -> Result<Vec<String>, String> {
    let s = state.lock().map_err(err)?;
    let deck = s.deck.as_ref().ok_or("no deck open")?;
    deck_convert::pptx::export(deck, &s.dir, &PathBuf::from(path)).map_err(err)
}

#[tauri::command]
fn export_pdf(state: Shared, path: String, steps: Option<bool>) -> Result<(), String> {
    let mut s = state.lock().map_err(err)?;
    s.export_pdf(&PathBuf::from(path), steps.unwrap_or(false))
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
        s.checkpoint("set_section", false);
        s.set_section(section, subsection)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// The deck's macros (`scripts/*.py`) and whether a Python was found.
#[derive(Debug, Serialize)]
struct Scripts {
    scripts: Vec<String>,
    python: Option<String>,
}

#[tauri::command]
fn list_scripts(state: Shared) -> Result<Scripts, String> {
    let dir = state.lock().map_err(err)?.dir.clone();
    Ok(Scripts {
        scripts: scripts::deck_scripts(&dir),
        python: scripts::find_python().map(|p| p.display().to_string()),
    })
}

/// Runs a Python script against the open deck: `name` is a file of the
/// deck's `scripts/` folder or an absolute path. The deck the script
/// hands back replaces the open one as one undo step.
#[tauri::command]
fn run_script(
    app: AppHandle,
    state: Shared,
    name: String,
    selection: Vec<usize>,
) -> Result<scripts::ScriptOutput, String> {
    let python = scripts::find_python().ok_or("no Python found: install one (or set DECKER_PYTHON to it)")?;
    let package = scripts::package_dir(app.path().resource_dir().ok()).ok_or("the decker Python package is missing")?;
    let (script, deck_json, dir, slide) = {
        let s = state.lock().map_err(err)?;
        let deck = s.deck.as_ref().ok_or("no deck open")?;
        let p = PathBuf::from(&name);
        let script = if p.is_absolute() { p } else { s.dir.join("scripts").join(&name) };
        (script, serde_json::to_string(deck).map_err(err)?, s.dir.clone(), s.slide)
    };
    let (mut out, returned) = scripts::run(&python, &package, &script, &deck_json, &dir, slide, &selection)?;
    if let Some(json) = returned {
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("the script returned bad JSON: {e}"))?;
        let deck: Deck = serde_json::from_value(value["deck"].clone())
            .map_err(|e| format!("the script returned a bad deck: {e}"))?;
        let mut s = state.lock().map_err(err)?;
        s.checkpoint(&format!("script:{name}"), false);
        s.replace_deck(deck, value["slide"].as_u64().map(|n| n as usize));
        out.changed = true;
        drop(s);
        edited(&app);
    }
    Ok(out)
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
        s.checkpoint("set_section_of", false);
        s.set_section_of(slide, section, subsection)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Moves a slide to another position in the deck.
#[tauri::command]
fn move_slide(app: AppHandle, state: Shared, from: usize, to: usize) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("move_slide", false);
        s.move_slide(from, to)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn edit_layout(app: AppHandle, state: Shared, name: Option<String>) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.edit_layout(name.filter(|n| !n.is_empty()))?;
        s.view()
    };
    changed(&app);
    Ok(view)
}

#[tauri::command]
fn new_layout(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("new_layout", false);
        s.new_layout()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn rename_layout(app: AppHandle, state: Shared, name: String) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("rename_layout", false);
        s.rename_layout(name)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn delete_layout(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("delete_layout", false);
        s.delete_layout()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Styles runs of a text element over a range (`null` bounds: the whole element).
#[tauri::command]
fn style_text(
    app: AppHandle,
    state: Shared,
    element: usize,
    from: Option<usize>,
    to: Option<usize>,
    change: StyleChange,
) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("style_text", false);
        s.style_text(element, from.unwrap_or(usize::MAX), to.unwrap_or(usize::MAX), &change)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn style_paragraphs(
    app: AppHandle,
    state: Shared,
    element: usize,
    from: Option<usize>,
    to: Option<usize>,
    change: ParagraphChange,
) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("style_paragraphs", false);
        s.style_paragraphs(element, from.unwrap_or(usize::MAX), to.unwrap_or(usize::MAX), &change)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn style_info(state: Shared, element: usize, from: Option<usize>, to: Option<usize>) -> Result<StyleInfo, String> {
    state.lock().map_err(err)?.style_info(element, from.unwrap_or(usize::MAX), to.unwrap_or(usize::MAX))
}

#[tauri::command]
fn style_element(app: AppHandle, state: Shared, element: usize, change: ElementStyle) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("style_element", false);
        s.style_element(element, &change)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn set_theme(app: AppHandle, state: Shared, change: ThemeChange) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("set_theme", false);
        s.set_theme(&change)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn set_steps(app: AppHandle, state: Shared, element: usize, steps: Vec<deck_core::Step>) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("set_steps", false);
        s.set_steps(element, steps)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn group_elements(app: AppHandle, state: Shared, indices: Vec<usize>) -> Result<usize, String> {
    let at = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("group_elements", false);
        s.group(&indices)?
    };
    edited(&app);
    Ok(at)
}

#[tauri::command]
fn ungroup_element(app: AppHandle, state: Shared, index: usize) -> Result<usize, String> {
    let n = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("ungroup_element", false);
        s.ungroup(index)?
    };
    edited(&app);
    Ok(n)
}

/// Turns an element at the viewed click; dragging the handle is one undo step.
#[tauri::command]
fn set_rotation(app: AppHandle, state: Shared, element: usize, degrees: f64) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("set_rotation", false);
        s.set_rotation(element, degrees)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Moves or resizes several elements as one undo step.
#[tauri::command]
fn set_rects(app: AppHandle, state: Shared, rects: Vec<(usize, Rect)>) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("set_rects", false);
        s.set_rects(&rects)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Aligns, distributes, mirrors or reorders the selection; returns where
/// the selected elements are afterwards.
#[tauri::command]
fn arrange(app: AppHandle, state: Shared, indices: Vec<usize>, op: String) -> Result<Vec<usize>, String> {
    let moved = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("arrange", false);
        s.arrange(&indices, &op)?
    };
    edited(&app);
    Ok(moved)
}

#[tauri::command]
fn add_table(app: AppHandle, state: Shared, rows: usize, columns: usize) -> Result<usize, String> {
    let index = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("add_table", false);
        s.add_table(rows, columns)?
    };
    edited(&app);
    Ok(index)
}

#[tauri::command]
fn edit_table(app: AppHandle, state: Shared, element: usize, edit: TableEdit) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        // Typing in one cell is one undo step; structural edits are their own.
        let (kind, merge) = match &edit {
            TableEdit::Cell { row, column, .. } => (format!("table_cell:{element}:{row}:{column}"), true),
            _ => ("edit_table".to_string(), false),
        };
        s.checkpoint(&kind, merge);
        s.edit_table(element, &edit)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn add_shape(app: AppHandle, state: Shared, kind: String) -> Result<usize, String> {
    let index = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("add_shape", false);
        s.add_shape(&kind)?
    };
    edited(&app);
    Ok(index)
}

#[tauri::command]
fn style_placeholder(app: AppHandle, state: Shared, element: usize, change: PlaceholderStyle) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("style_placeholder", false);
        s.style_placeholder(element, &change)?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn add_figure(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("add_figure", false);
        s.add_figure()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn add_outline(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("add_outline", false);
        s.add_outline()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// Saves a layout as a `.deck_layout` folder; returns the layout's name.
#[tauri::command]
fn save_layout(state: Shared, path: String) -> Result<String, String> {
    state.lock().map_err(err)?.save_layout(&layout_dir(&path))
}

/// Loads a `.deck_layout` folder (or its `layout.json`) into the deck.
#[tauri::command]
fn load_layout(app: AppHandle, state: Shared, path: String) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("load_layout", false);
        s.load_layout(&layout_dir(&path))?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// A `.deck_layout` folder, from the folder or its `layout.json`.
fn layout_dir(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.file_name().map(|f| f == "layout.json").unwrap_or(false) {
        p.parent().map(Path::to_path_buf).unwrap_or(p)
    } else {
        p
    }
}

#[tauri::command]
fn add_placeholder(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("add_placeholder", false);
        s.add_placeholder()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn get_view(state: Shared) -> Result<View, String> {
    Ok(state.lock().map_err(err)?.view())
}

#[tauri::command]
fn undo(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.undo()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

#[tauri::command]
fn redo(app: AppHandle, state: Shared) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.redo()?;
        s.view()
    };
    edited(&app);
    Ok(view)
}

/// `next`, `prev`, `first`, `last`, `next_slide`, `prev_slide`, `black`, `white`.
#[tauri::command]
fn navigate(app: AppHandle, state: Shared, action: String) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.break_merge();
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
        s.break_merge();
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
        s.checkpoint("set_rect", false);
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
        s.checkpoint("set_text", false);
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
        s.checkpoint(&format!("text_edit:{element}"), true);
        s.text_edit(element, from, to, &insert)?
    };
    edited(&app);
    Ok(caret)
}

#[tauri::command]
fn set_image(app: AppHandle, state: Shared, element: usize, crop: Crop, mask: Mask) -> Result<View, String> {
    let view = {
        let mut s = state.lock().map_err(err)?;
        s.checkpoint("set_image", false);
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
        s.checkpoint("set_notes", true);
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
        s.checkpoint("add_slide", false);
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
        s.checkpoint("delete_slide", false);
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
        s.checkpoint("duplicate_slide", false);
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
        s.checkpoint("set_layout", false);
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
        s.checkpoint("import_slides", false);
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
        s.checkpoint("add_text", false);
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
        s.checkpoint("delete_element", false);
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
    deck_present::start(&app, "editor", with_presenter)?;
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
    deck_present::stop(&app);
    changed(&app);
    Ok(())
}

#[tauri::command]
fn monitors(app: AppHandle) -> Result<Vec<String>, String> {
    deck_present::monitors(&app, "editor")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `DECKER_OPEN=<path>` opens a deck or pptx at startup (also used by the
    // smoke test under a virtual display).
    let mut initial = AppState::default();
    if let Ok(path) = std::env::var("DECKER_OPEN") {
        let path = PathBuf::from(path);
        if deck_convert::archive::is_archive(&path) {
            if let Ok((deck, dir)) = open_archive(&path) {
                initial.load(deck, dir);
                initial.archive = Some(path);
            }
        } else if path.extension().map(|e| e.eq_ignore_ascii_case("pptx")).unwrap_or(false) {
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
    let present = deck_present::present_at_launch();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(initial))
        .manage(SystemClipboard(Mutex::new(None)))
        .setup(move |app| {
            // The presentation windows, hidden until "Present".
            deck_present::create_windows(app.handle(), "decker");
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
            move_slide,
            edit_layout,
            new_layout,
            rename_layout,
            delete_layout,
            add_placeholder,
            add_outline,
            add_figure,
            add_shape,
            style_placeholder,
            add_table,
            edit_table,
            set_rects,
            arrange,
            set_rotation,
            group_elements,
            set_steps,
            set_theme,
            ungroup_element,
            style_text,
            style_paragraphs,
            style_info,
            style_element,
            save_layout,
            load_layout,
            get_view,
            get_thumbnails,
            undo,
            redo,
            list_scripts,
            run_script,
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
            add_image,
            add_image_file,
            copy_elements,
            paste,
            delete_element,
            start_presentation,
            stop_presentation,
            monitors
        ])
        .run(tauri::generate_context!())
        .expect("error while running decker");
}
