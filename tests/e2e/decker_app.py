"""Drives the decker editor over WebDriver (tauri-driver): one app start per
`Driver`, an `Editor` session with helpers in slide coordinates. Shared by
the end-to-end tests and the manual's screenshots."""
import os
import subprocess
import time
import urllib.request
from pathlib import Path

from webdriver import Session

DRIVER = "http://127.0.0.1:4444"
DECK = ""  # the deck the app opens; set by the caller
W, H = 1440, 900
CTRL, SHIFT, ALT, ESCAPE = "\ue009", "\ue008", "\ue00a", "\ue00c"


# The drivers' and the app's output, for when a test fails: DECKER_E2E_LOG.
LOG = open(os.environ["DECKER_E2E_LOG"], "ab") if os.environ.get("DECKER_E2E_LOG") else subprocess.DEVNULL


class Driver:
    """A tauri-driver of its own: it serves one session, one app start."""

    def __init__(self, deck: str = "", **env: str):
        self.p = subprocess.Popen(["tauri-driver"], env={**os.environ, "DECKER_OPEN": deck or DECK, **env},
                                  stdout=LOG, stderr=subprocess.STDOUT, start_new_session=True)
        # Ready when it answers its status endpoint.
        for _ in range(100):
            try:
                urllib.request.urlopen(DRIVER + "/status", timeout=1).read()
                return
            except OSError:
                time.sleep(0.1)
        raise RuntimeError("tauri-driver did not start")

    def close(self):
        # The driver, its WebKitWebDriver and the app: the whole group.
        import signal
        # tauri-driver may ignore SIGTERM: SIGKILL if it is still there.
        for sig in (signal.SIGTERM, signal.SIGKILL):
            try:
                os.killpg(self.p.pid, sig)
            except ProcessLookupError:
                break
            try:
                self.p.wait(timeout=3)
                break
            except subprocess.TimeoutExpired:
                continue
        self.p.wait()
        # Both ports free again: tauri-driver's and WebKitWebDriver's,
        # or the next driver cannot listen and its session dies midway.
        import socket
        for _ in range(200):
            busy = False
            for port in (4444, 4445):
                with socket.socket() as sock:
                    sock.settimeout(0.2)
                    busy |= sock.connect_ex(("127.0.0.1", port)) == 0
            if not busy:
                break
            time.sleep(0.1)


class Editor:
    def __init__(self, app: str, out: Path = Path("."), thumbs: int = 1):
        try:
            self.s = Session(DRIVER, app)
        except OSError:
            time.sleep(2)  # the previous app still closing: once more
            self.s = Session(DRIVER, app)
        self.out = out
        self.s.cmd("POST", "/window/rect", {"width": W, "height": H, "x": 0, "y": 0})
        self.s.wait(f"return document.querySelectorAll('.thumb').length >= {thumbs}")
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

    def chord(self, *keys: str):
        """Presses keys together (modifiers first), e.g. chord(CTRL, "z")."""
        acts = [{"type": "keyDown", "value": k} for k in keys] + [{"type": "keyUp", "value": k} for k in reversed(keys)]
        self.s.cmd("POST", "/actions", {"actions": [{"type": "key", "id": "kb", "actions": acts}]})
        self.s.cmd("DELETE", "/actions")
        self.settle(1.0)

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


