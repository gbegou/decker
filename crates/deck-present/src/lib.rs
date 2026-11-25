//! What presenting needs and editing does not: where the show is (page,
//! build step, blank screen), how keys move it, and the two windows,
//! audience and presenter, with their placement on the monitors. The
//! editor (`deck-app`) and the viewer (`deck-viewer`) both use it, so
//! that a deck and a PDF present the same way.
//!
//! The pages themselves are the application's business: it renders them
//! and tells [`Show`] how many there are and how many steps each has.

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// The position of a show and the state of its screen.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Show {
    /// The page shown (a slide, a PDF page).
    pub page: usize,
    /// The build step reached on that page (0 on a page without steps).
    pub step: u32,
    /// `""`, `"black"` or `"white"`: the audience sees that instead of the page.
    pub blank: String,
    /// Whether the presentation windows are up.
    pub presenting: bool,
}

/// What a show navigates over: pages with build steps.
pub trait Pages {
    fn count(&self) -> usize;
    /// The last build step of page `page` (0 for a page without steps).
    fn last_step(&self, page: usize) -> u32;
}

impl Pages for Vec<u32> {
    fn count(&self) -> usize {
        self.len()
    }
    fn last_step(&self, page: usize) -> u32 {
        self.get(page).copied().unwrap_or(0)
    }
}

impl Show {
    /// Moves by `action`: `next` and `prev` walk steps then pages;
    /// `next_slide` and `prev_slide` skip steps; `first`, `last`; `black`
    /// and `white` toggle the blank screen. Unknown actions do nothing.
    pub fn navigate(&mut self, action: &str, pages: &dyn Pages) {
        let count = pages.count();
        if count == 0 {
            return;
        }
        self.page = self.page.min(count - 1);
        let last_step = pages.last_step(self.page);
        match action {
            "next" => {
                if self.step < last_step {
                    self.step += 1;
                } else if self.page + 1 < count {
                    self.page += 1;
                    self.step = 0;
                }
            }
            "prev" => {
                if self.step > 0 {
                    self.step -= 1;
                } else if self.page > 0 {
                    self.page -= 1;
                    self.step = pages.last_step(self.page);
                }
            }
            "next_slide" | "next_page" => {
                if self.page + 1 < count {
                    self.page += 1;
                    self.step = 0;
                }
            }
            "prev_slide" | "prev_page" => {
                if self.page > 0 {
                    self.page -= 1;
                    self.step = 0;
                }
            }
            "first" => {
                self.page = 0;
                self.step = 0;
            }
            "last" => {
                self.page = count - 1;
                self.step = 0;
            }
            "black" | "blackout" => self.toggle_blank("black"),
            "white" => self.toggle_blank("white"),
            _ => {}
        }
    }

    /// Jumps to `page` at `step` (clamped to the page's steps).
    pub fn goto(&mut self, page: usize, step: u32, pages: &dyn Pages) {
        if page < pages.count() {
            self.page = page;
            self.step = step.min(pages.last_step(page));
        }
    }

    fn toggle_blank(&mut self, color: &str) {
        self.blank = if self.blank == color { String::new() } else { color.to_string() };
    }
}

/// The labels of the two presentation windows.
pub const AUDIENCE: &str = "audience";
pub const PRESENTER: &str = "presenter";

/// `DECKER_PRESENT`: `1` starts the show at launch, `both` also opens
/// the presenter window on a single monitor.
pub fn present_at_launch() -> Option<String> {
    std::env::var("DECKER_PRESENT").ok().filter(|v| !v.is_empty())
}

/// Gives every window of the app the icon `png` (PNG bytes, typically
/// `include_bytes!` of the app's `icons/128x128.png`, which makes the
/// compiler track the file): the taskbar and title bar show it even when
/// an incremental build kept an older embedded icon.
pub fn set_icons(app: &AppHandle, png: &'static [u8]) {
    let Ok(icon) = tauri::image::Image::from_bytes(png) else {
        eprintln!("[decker] the window icon is not a readable PNG");
        return;
    };
    for window in app.webview_windows().values() {
        let _ = window.set_icon(icon.clone());
    }
}

/// Creates the audience and presenter windows, hidden, at startup: on
/// Windows a webview created later never loaded its page. `app_name`
/// prefixes their titles. Both load the app's page; the window label
/// tells the page what to show.
pub fn create_windows(app: &AppHandle, app_name: &str) {
    for (label, suffix) in [(AUDIENCE, "presentation"), (PRESENTER, "presenter")] {
        let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
            .title(format!("{app_name} — {suffix}"))
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
}

/// Shows the audience window fullscreen on the secondary monitor (the
/// primary one alone), and the presenter window on the primary monitor
/// when there is a secondary one or `with_presenter` asks for it.
/// `main` is the label of the application's own window. With
/// `DECKER_WINDOWED=1` the audience window stays a normal window, to
/// tell a page problem from a fullscreen painting problem.
pub fn start(app: &AppHandle, main: &str, with_presenter: Option<bool>) -> Result<(), String> {
    let main_window = app.get_webview_window(main).ok_or("no main window")?;
    let monitors = main_window.available_monitors().map_err(|e| e.to_string())?;
    let primary = main_window.primary_monitor().map_err(|e| e.to_string())?.or_else(|| monitors.first().cloned());
    let secondary = monitors.iter().find(|m| Some(m.position()) != primary.as_ref().map(|p| p.position())).cloned();
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
    let audience = app.get_webview_window(AUDIENCE).ok_or("no audience window")?;
    let _ = audience.set_decorations(windowed);
    if windowed {
        let _ = audience.set_size(tauri::LogicalSize::new(960.0, 540.0));
    } else if let Some((x, y, w, h)) = audience_monitor.as_ref().map(monitor_rect) {
        let _ = audience.set_position(tauri::LogicalPosition::new(x, y));
        let _ = audience.set_size(tauri::LogicalSize::new(w, h));
    }
    audience.show().map_err(|e| e.to_string())?;
    if !windowed {
        let _ = audience.set_fullscreen(true);
    }
    let _ = audience.set_focus();
    if open_presenter {
        let presenter = app.get_webview_window(PRESENTER).ok_or("no presenter window")?;
        if let Some((x, y, w, h)) = primary.as_ref().map(monitor_rect) {
            let _ = presenter.set_position(tauri::LogicalPosition::new(x + 40.0, y + 40.0));
            let _ = presenter.set_size(tauri::LogicalSize::new((w - 80.0).max(600.0), (h - 120.0).max(400.0)));
        }
        presenter.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Hides both presentation windows.
pub fn stop(app: &AppHandle) {
    for label in [AUDIENCE, PRESENTER] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_fullscreen(false);
            let _ = w.hide();
        }
    }
}

/// The monitors, as `name WxH`, for a diagnostic.
pub fn monitors(app: &AppHandle, main: &str) -> Result<Vec<String>, String> {
    let main_window = app.get_webview_window(main).ok_or("no main window")?;
    Ok(main_window
        .available_monitors()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|m| format!("{} {}x{}", m.name().cloned().unwrap_or_default(), m.size().width, m.size().height))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_walks_steps_then_pages() {
        let pages = &vec![2u32, 0, 1];
        let mut s = Show::default();
        s.navigate("next", pages);
        s.navigate("next", pages);
        assert_eq!((s.page, s.step), (0, 2));
        s.navigate("next", pages);
        assert_eq!((s.page, s.step), (1, 0));
        s.navigate("prev", pages);
        assert_eq!((s.page, s.step), (0, 2), "back lands on the previous page's last step");
        s.navigate("next_slide", pages);
        s.navigate("next_page", pages);
        s.navigate("next", pages);
        s.navigate("next", pages);
        assert_eq!((s.page, s.step), (2, 1), "stays on the last page");
        s.navigate("first", pages);
        assert_eq!((s.page, s.step), (0, 0));
        s.navigate("last", pages);
        assert_eq!((s.page, s.step), (2, 0));
        s.goto(1, 5, pages);
        assert_eq!((s.page, s.step), (1, 0), "steps clamp");
        s.goto(9, 0, pages);
        assert_eq!(s.page, 1, "out of range is ignored");
        s.navigate("black", pages);
        assert_eq!(s.blank, "black");
        s.navigate("white", pages);
        assert_eq!(s.blank, "white");
        s.navigate("white", pages);
        assert_eq!(s.blank, "");
        s.navigate("next", &Vec::<u32>::new());
        assert_eq!(s.page, 1, "an empty show does nothing");
    }
}
