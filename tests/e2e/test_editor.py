"""End-to-end tests: the real editor, driven over WebDriver.

Linux only; needs tauri-driver, WebKitWebDriver and a display (Xvfb). Run
with tests/e2e/run.sh, which builds the app and starts Xvfb when needed:

    DECKER_APP=target/debug/deck-app python3 -m unittest discover -s tests/e2e
"""
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(ROOT / "docs" / "screenshots"))
sys.path.insert(0, str(ROOT / "python"))
import decker_app  # noqa: E402
import make_demo  # noqa: E402

APP = os.environ.get("DECKER_APP", str(ROOT / "target" / "debug" / "deck-app"))


class EditorTest(unittest.TestCase):
    """Each test starts the app on a fresh copy of the demo deck (5 slides)."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.deck = str(Path(self.tmp.name) / "demo.deck")
        make_demo.main(self.deck)
        self.driver = decker_app.Driver(self.deck, **self.env())
        self.addCleanup(self.driver.close)
        self.addCleanup(self.tmp.cleanup)
        self.e = decker_app.Editor(APP, thumbs=5)
        self.addCleanup(self.e.s.quit)

    def env(self):
        return {}

    def view(self):
        return self.e.s.call("get_view")

    def canvas_svg(self):
        return self.e.s.js("return document.querySelector('.canvas .slide-svg, .stage .slide-svg')?.innerHTML "
                           "?? [...document.querySelectorAll('.slide-svg')].pop().innerHTML")


class Typing(EditorTest):
    def test_typed_text_reaches_the_slide_and_its_glyphs_resolve(self):
        e = self.e
        e.goto(0)
        e.click_slide(480, 215, clicks=2)  # the title
        e.keys("")  # End
        for ch in " now":
            e.keys(ch)
        e.escape()
        self.assertEqual(self.view()["elements"][0]["text"], "Waves in a tank now")
        # Every glyph reference of the canvas points into the canvas itself:
        # WebView2 resolved them to a thumbnail's glyphs, and typed text
        # stayed invisible (fixed by scoping the ids per view).
        dangling = e.s.js("""
            const views = [...document.querySelectorAll('.slide-svg')];
            return views.flatMap(v => [...v.querySelectorAll('use')].map(u => u.getAttribute('href') || u.getAttribute('xlink:href'))
                .filter(h => h && !v.querySelector(`[id="${h.slice(1)}"]`)));""")
        self.assertEqual(dangling, [])

    def test_a_formula_under_the_caret_shows_its_source_and_the_footer_stays(self):
        e = self.e
        e.goto(2)
        footer = "return document.querySelector('svg.overlay').parentElement.querySelector('.slide-svg g[id$=\"layout\"]').innerHTML"
        before = e.s.js(footer)
        e.click_slide(400, 128, clicks=2)
        e.keys("")
        for _ in range(3):
            e.keys("")  # Left, into $c = sqrt(g h)$
        layout = e.s.call("text_layout", {"element": 1})
        self.assertIn("sqrt(g h)$", layout["text"])
        self.assertEqual(e.s.js(footer), before, "the footer keeps its variables filled in")
        e.escape()

    def test_alt_equals_types_an_inline_formula(self):
        e = self.e
        e.goto(2)
        e.click_slide(400, 128, clicks=2)
        e.keys("")
        e.chord(decker_app.ALT, "=")
        e.keys("x")
        e.escape()
        self.assertTrue(self.view()["elements"][1]["text"].split("\n")[0].endswith("$x$"))


class Tables(EditorTest):
    def test_merge_split_and_undo(self):
        e = self.e
        e.goto(4)
        e.click_slide(300, 250)
        e.s.js("document.querySelectorAll('aside.props textarea')[3].focus(); return true")
        e.settle(0.4)
        e.s.js("[...document.querySelectorAll('button')].find(b => b.textContent.trim() === 'Merge →').click(); return true")
        e.settle(1.0)
        table = lambda: next(x for x in self.view()["elements"] if x["kind"] == "table")["table"]
        self.assertEqual(table()["spans"][1][:2], [[2, 1], [0, 0]])
        # Ctrl+Z in a panel field undoes the field's typing: leave the field first.
        e.s.js("document.activeElement.blur(); return true")
        e.chord(decker_app.CTRL, "z")
        self.assertEqual(table()["spans"][1][:2], [[1, 1], [1, 1]], "Ctrl+Z undoes the merge")


class Thumbnails(EditorTest):
    def test_dragging_a_thumbnail_reorders_the_slides(self):
        e = self.e
        x, y, w, h = e.rect(".stack-item[data-index='4'] .thumb")
        tx, ty, _, _ = e.rect(".stack-item[data-index='0'] .thumb")
        moves = [{"type": "pointerMove", "x": int(x + w / 2), "y": int(y + h / 2), "origin": "viewport"},
                 {"type": "pointerDown", "button": 0}]
        for k in range(1, 11):
            moves.append({"type": "pointerMove", "duration": 30, "origin": "viewport",
                          "x": int(x + w / 2), "y": int(y + h / 2 + (ty + 10 - y - h / 2) * k / 10)})
        moves.append({"type": "pointerUp", "button": 0})
        e.s.cmd("POST", "/actions", {"actions": [{"type": "pointer", "id": "m", "parameters": {"pointerType": "mouse"}, "actions": moves}]})
        e.s.cmd("DELETE", "/actions")
        e.settle(1.0)
        e.goto(0)
        self.assertEqual(self.view()["elements"][0]["text"], "Measured speeds")


class Presenting(EditorTest):
    def env(self):
        return {"DECKER_PRESENT": "both", "DECKER_WINDOWED": "1"}

    def test_slides_change_background_in_the_presentation_windows(self):
        s = self.e.s
        s.call("goto", {"slide": 0, "step": 0})
        time.sleep(1)
        s.call("goto", {"slide": 2, "step": 0})
        time.sleep(1.5)
        fills = []
        for handle in s.cmd("GET", "/window/handles"):
            s.cmd("POST", "/window", {"handle": handle})
            if s.js("return !!document.querySelector('aside.slides')"):
                continue  # the editor
            fills += s.js("return [...document.querySelectorAll('.slide-svg > svg')].slice(0, 1)"
                          ".map(v => v.querySelector(':scope > rect')?.getAttribute('fill'))")
        # Title slide beige, then a white slide: no window keeps the beige.
        self.assertEqual(len(fills), 2, "the audience and presenter windows")
        self.assertNotIn("#F3EEE6", fills)


class LongDeck(EditorTest):
    """A deck of 80 slides, as imported decks often are."""

    def setUp(self):
        import decker
        real = make_demo.main

        def long(out):
            real(out)
            deck = decker.load(out)
            for _ in range(75):
                deck.slides[2].duplicate()
            deck.save(out)

        make_demo.main = long
        try:
            super().setUp()
        finally:
            make_demo.main = real

    def env(self):
        return {"DECKER_PRESENT": "both", "DECKER_WINDOWED": "1"}

    def test_the_presenter_keeps_the_current_slide_large(self):
        s = self.e.s
        for handle in s.cmd("GET", "/window/handles"):
            s.cmd("POST", "/window", {"handle": handle})
            if s.js("return !!document.querySelector('.presenter')"):
                break
        else:
            self.fail("no presenter window")
        s.cmd("POST", "/window/rect", {"width": 1400, "height": 800, "x": 0, "y": 0})
        time.sleep(1)
        widths = s.js("return [document.querySelector('.presenter > .current').getBoundingClientRect().width, window.innerWidth]")
        # The thumbnail strip used to widen its column and squeeze the slide.
        self.assertGreater(widths[0], widths[1] * 0.45, widths)


if __name__ == "__main__":
    unittest.main()
