"""Takes the manual's screenshots from the real editor, driven over WebDriver.

Needs a Linux desktop or Xvfb, `tauri-driver` (cargo install tauri-driver),
WebKitWebDriver (package webkit2gtk-driver) and ImageMagick's `convert`.
Run through docs/screenshots/shoot.sh, which builds the app and the demo deck.

    python3 shoot.py APP DECK OUT_DIR
"""
import os
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests" / "e2e"))
import decker_app  # noqa: E402
from webdriver import Session  # noqa: E402

DECK = ""


def Editor(app, out):
    return decker_app.Editor(app, out, thumbs=5)


def Driver(**env):
    return decker_app.Driver(DECK, **env)


def union(*rects):
    x0 = min(r[0] for r in rects); y0 = min(r[1] for r in rects)
    x1 = max(r[0] + r[2] for r in rects); y1 = max(r[1] + r[3] for r in rects)
    return [x0, y0, x1 - x0, y1 - y0]


def window(e):
    # The whole window, a figure selected.
    e.goto(3)
    e.click_slide(300, 300)
    e.shot("editor")


def text_editing(e):
    # Typing in place, the caret in a formula: shown as its source.
    e.goto(2)
    e.click_slide(400, 128, clicks=2)
    e.keys("\ue010")  # End: the end of the first line
    for _ in range(3):
        e.keys("\ue012")  # Left, into the formula
    e.shot("text-editing", crop=e.rect("svg.overlay"))


def slide_menu(e):
    e.goto(2)
    x, y = e.rect(".stack-item[data-index='2'] .thumb")[:2]
    e.pointer(x + 40, y + 40, button=2)
    e.shot("slide-menu", crop=union(e.rect("aside.slides"), e.rect(".context-menu")), pad=4)


def arrange(e):
    # Two elements selected: the arrange bar.
    e.goto(4, 1)
    e.click_slide(300, 220)
    e.click_slide(820, 220, keys=("\ue008",))  # Shift: the note too
    e.shot("arrange", crop=union(e.rect(".bars"), e.rect("svg.overlay")))


def build_steps(e):
    e.goto(3, 1)
    e.click_slide(700, 320)
    top = e.s.js("""const h = [...document.querySelectorAll('aside.props h3')].find(h => /build steps/i.test(h.textContent));
        h.scrollIntoView(); return h.getBoundingClientRect().top;""")
    e.settle(0.5)
    top = e.s.js("return [...document.querySelectorAll('aside.props h3')].find(h => /build steps/i.test(h.textContent)).getBoundingClientRect().top")
    x, y, w, h = e.rect("aside.props")
    e.shot("build-steps", crop=[x, top - 8, w, min(420, y + h - top + 8)])


def layout_editor(e):
    e.goto(2)
    e.s.js("[...document.querySelectorAll('button')].find(b => b.textContent.trim() === 'Edit layout').click(); return true")
    e.settle(1.5)
    e.shot("layout-editor")


def help_dialog(e):
    e.keys("\ue031")  # F1
    e.shot("help", crop=e.rect(".dialog"))


SHOTS = [window, text_editing, slide_menu, arrange, build_steps, layout_editor, help_dialog]


def editor_shots(app: str, out: Path):
    """One app start per shot, so that no state leaks from one to the next."""
    for shot in SHOTS:
        d = Driver()
        try:
            e = Editor(app, out)
            shot(e)
            e.s.quit()
        finally:
            d.close()


def presenter_shot(app: str, out: Path):
    d = Driver(DECKER_PRESENT="both", DECKER_WINDOWED="1")
    s = Session(decker_app.DRIVER, app)
    s.wait("return document.readyState === 'complete'")
    time.sleep(1)
    s.js("window.__TAURI_INTERNALS__.invoke('goto', {slide: 2, step: 0}); return true")
    time.sleep(1.5)
    for handle in s.cmd("GET", "/window/handles"):
        s.cmd("POST", "/window", {"handle": handle})
        if s.js("return !!document.querySelector('.presenter, .notes')"):
            s.cmd("POST", "/window/rect", {"width": decker_app.W, "height": decker_app.H, "x": 0, "y": 0})
            time.sleep(1.5)
            s.screenshot(str(out / "presenter.png"))
            print("wrote", out / "presenter.png")
            break
    s.quit()
    d.close()


if __name__ == "__main__":
    app, DECK, out = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    editor_shots(app, out)
    presenter_shot(app, out)
