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
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from webdriver import Session  # noqa: E402

DRIVER = "http://127.0.0.1:4444"
DECK = ""
W, H = 1440, 900


class Driver:
    """A tauri-driver of its own: it serves one session, one app start."""

    def __init__(self, **env: str):
        self.p = subprocess.Popen(["tauri-driver"], env={**os.environ, "DECKER_OPEN": DECK, **env},
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(2)

    def close(self):
        self.p.terminate()
        self.p.wait()
        time.sleep(0.5)


class Editor:
    def __init__(self, app: str, out: Path):
        self.s = Session(DRIVER, app)
        self.out = out
        self.s.cmd("POST", "/window/rect", {"width": W, "height": H, "x": 0, "y": 0})
        self.s.wait("return document.querySelectorAll('.thumb').length >= 5")
        self.settle()

    def settle(self, t: float = 1.0):
        time.sleep(t)

    def invoke(self, command: str, args: dict):
        self.s.js("window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]); return true", command, args)
        self.settle()

    def goto(self, slide: int, step: int = 0):
        self.invoke("goto", {"slide": slide, "step": step})

    def point(self, x: float, y: float):
        """Viewport coordinates of slide point (x, y)."""
        return self.s.js("""const r = document.querySelector('svg.overlay').getBoundingClientRect();
            const vb = document.querySelector('svg.overlay').viewBox.baseVal;
            return [r.left + arguments[0] / vb.width * r.width, r.top + arguments[1] / vb.height * r.height];""", x, y)

    def pointer(self, x: float, y: float, clicks: int = 1, button: int = 0, keys=()):
        acts = [{"type": "pointerMove", "x": int(x), "y": int(y), "origin": "viewport"}]
        for _ in range(clicks):
            acts += [{"type": "pointerDown", "button": button}, {"type": "pointerUp", "button": button}]
        chain = [{"type": "pointer", "id": "mouse", "parameters": {"pointerType": "mouse"}, "actions": acts}]
        if keys:
            down = [{"type": "keyDown", "value": k} for k in keys]
            up = [{"type": "keyUp", "value": k} for k in keys]
            chain.append({"type": "key", "id": "kb", "actions": down + [{"type": "pause"}] * (len(acts) - len(down) - len(up)) + up})
        self.s.cmd("POST", "/actions", {"actions": chain})
        self.s.cmd("DELETE", "/actions")
        self.settle(0.6)

    def click_slide(self, x, y, clicks=1, keys=()):
        self.pointer(*self.point(x, y), clicks=clicks, keys=keys)

    def keys(self, *values: str):
        acts = []
        for v in values:
            acts += [{"type": "keyDown", "value": v}, {"type": "keyUp", "value": v}]
        self.s.cmd("POST", "/actions", {"actions": [{"type": "key", "id": "kb", "actions": acts}]})
        self.s.cmd("DELETE", "/actions")
        self.settle(0.6)

    def rect(self, selector: str):
        return self.s.js("const r = document.querySelector(arguments[0]).getBoundingClientRect();"
                         "return [r.left, r.top, r.width, r.height];", selector)

    def shot(self, name: str, crop=None, pad: int = 0):
        """Saves the window, or the box `crop` = [x, y, w, h] grown by `pad`."""
        path = self.out / f"{name}.png"
        self.s.screenshot(str(path))
        if crop:
            x, y, w, h = (int(v) for v in crop)
            x, y = max(0, x - pad), max(0, y - pad)
            subprocess.run(["convert", str(path), "-crop", f"{w + 2 * pad}x{h + 2 * pad}+{x}+{y}", "+repage", str(path)], check=True)
        print("wrote", path)

    def escape(self):
        self.keys("")


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
    s = Session(DRIVER, app)
    s.wait("return document.readyState === 'complete'")
    time.sleep(1)
    s.js("window.__TAURI_INTERNALS__.invoke('goto', {slide: 2, step: 0}); return true")
    time.sleep(1.5)
    for handle in s.cmd("GET", "/window/handles"):
        s.cmd("POST", "/window", {"handle": handle})
        if s.js("return !!document.querySelector('.presenter, .notes')"):
            s.cmd("POST", "/window/rect", {"width": W, "height": H, "x": 0, "y": 0})
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
