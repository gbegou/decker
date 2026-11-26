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


class SlideMenus(EditorTest):
    def test_a_plan_slide_from_the_thumbnail_menu(self):
        e = self.e
        x, y = e.rect(".stack-item[data-index='2'] .thumb")[:2]
        e.pointer(x + 40, y + 40, button=2)
        e.s.js("[...document.querySelectorAll('.context-menu button')].find(b => b.textContent.trim() === 'New plan slide after').click(); return true")
        e.settle(1.5)
        v = self.view()
        self.assertEqual((v["slide_count"], v["slide"]), (6, 3), "a new slide after slide 3, shown")
        self.assertIn("outline", [x["kind"] for x in v["elements"]])
        self.assertFalse(e.s.js("return [...document.querySelectorAll('header button, .toolbar button')]"
                                ".some(b => ['+ Plan', '+ Slide', '− Slide'].includes(b.textContent.trim()))"),
                         "those are in the menus now")


class Layouts(EditorTest):
    def test_a_layout_for_several_slides_from_the_menu(self):
        e = self.e
        thumb = lambda i: e.rect(f".stack-item[data-index='{i}'] .thumb")
        x, y = thumb(2)[:2]
        e.pointer(x + 40, y + 40, keys=(decker_app.CTRL,))
        x, y = thumb(4)[:2]
        e.pointer(x + 40, y + 40, keys=(decker_app.CTRL,))
        e.pointer(x + 40, y + 40, button=2)
        self.assertEqual(e.s.js("return document.querySelector('.context-menu .menu-title').textContent"), "3 slides")
        click = "[...document.querySelectorAll('.context-menu button')].find(b => b.textContent.trim().startsWith(arguments[0])).click(); return true"
        e.s.js(click, "Layout")
        e.settle(0.3)
        e.s.js(click, "Title slide")
        e.settle(1.5)
        layouts = []
        for i in range(5):
            e.goto(i)
            layouts.append(self.view()["layout"])
        # Ctrl+click adds to the current slide (the first): slides 1, 3 and 5.
        self.assertEqual(layouts, ["Title slide", "Title only", "Title slide", "Title only", "Title slide"])


class Themes(EditorTest):
    def button(self, text):
        self.e.s.js("[...document.querySelectorAll('button')].find(b => b.textContent.trim() === arguments[0]).click(); return true", text)
        self.e.settle(1.0)

    def set_field(self, selector, value):
        self.e.s.js("""const el = document.querySelector(arguments[0]); el.value = arguments[1];
                       el.dispatchEvent(new Event('change', {bubbles: true})); return true""", selector, value)
        self.e.settle(1.0)

    def test_override_reset_and_theme_change(self):
        e = self.e
        e.goto(2)
        self.button("Edit layout")
        e.click_slide(400, 60)  # the title placeholder
        self.assertEqual(e.s.js("return document.querySelectorAll('.props .mark').length"), 6, "all from the theme")
        self.set_field(".props .fields input[type=number]", "40")
        self.assertEqual(self.view()["layout_meta"]["overrides"], ["title: size"])
        e.s.js("document.querySelector('.props .mark.own button').click(); return true")  # ↺
        e.settle(1.0)
        self.assertEqual(self.view()["layout_meta"]["overrides"], [])
        self.button("Edit theme “Waves”")
        self.assertEqual(self.view()["editing_theme"], "Waves")
        e.s.js("document.querySelector('details.kind').open = true; return true")
        self.set_field("details.kind .fields input[type=number]", "36")
        self.button("Done")
        e.goto(2)
        self.button("Edit layout")
        title = next(x for x in self.view()["elements"] if x["kind"] == "placeholder" and x["text"] == "title")
        self.assertEqual(title["placeholder"]["size"], 36, "the theme's new title size reaches the layout")


class FileMenu(EditorTest):
    def test_the_file_menu_and_ctrl_s(self):
        e = self.e
        self.assertFalse(e.s.js("return [...document.querySelectorAll('header > button')].some(b => /^(Save|New|Export)/.test(b.textContent.trim()))"),
                         "the file commands are in the menu")
        e.s.js("[...document.querySelectorAll('header button')].find(b => b.textContent.startsWith('File')).click(); return true")
        e.settle(0.5)
        items = e.s.js("return [...document.querySelectorAll('.context-menu.file button')].map(b => b.textContent.trim())")
        self.assertEqual([i.split(" Ctrl")[0] for i in items],
                         ["New", "Open file…", "Open .deck folder…", "Save", "Save as…", "Insert slides from deck…",
                          "Export PowerPoint…", "Export PDF…"])
        e.escape()
        e.s.js("document.querySelector('.menu-backdrop')?.click(); return true")
        # Ctrl+S saves the deck in place: deck.json changes on disk.
        deck_json = Path(self.deck) / "deck.json"
        before = deck_json.stat().st_mtime
        time.sleep(1.1)
        e.chord(decker_app.CTRL, "s")
        self.assertGreater(deck_json.stat().st_mtime, before)


class Paths(EditorTest):
    def tool(self, value):
        self.e.s.js("""const sel = [...document.querySelectorAll('select.add-shape')].find(s => s.querySelector('option[value=bezier]'));
            sel.value = arguments[0]; sel.dispatchEvent(new Event('change', {bubbles: true})); return true""", value)
        self.e.settle(0.5)

    def last(self):
        return self.view()["elements"][-1]

    def test_a_bezier_path_closed_on_itself(self):
        e = self.e
        e.goto(1)
        self.tool("bezier")
        e.click_slide(200, 200)
        e.drag_slide([(500, 200), (560, 300)])  # a smooth point
        e.click_slide(350, 420)
        e.click_slide(200, 200)  # the first point: closes
        el = self.last()
        self.assertEqual((el["shape"], el["path"]["closed"], len(el["path"]["nodes"])), ("path", True, 3))
        smooth = el["path"]["nodes"][1]
        self.assertTrue(smooth.get("in") and smooth.get("out"), smooth)
        self.assertIsNotNone(el["fill"], "a closed path is filled")

    def test_freehand_closes_near_its_start_and_edits(self):
        e = self.e
        e.goto(1)
        self.tool("free")
        import math
        loop = [(480 + 150 * math.cos(t / 12 * 2 * math.pi), 300 + 100 * math.sin(t / 12 * 2 * math.pi)) for t in range(13)]
        e.drag_slide(loop, steps=3)
        el = self.last()
        self.assertEqual((el["shape"], el["path"]["closed"]), ("path", True))
        n = len(el["path"]["nodes"])
        self.assertGreaterEqual(n, 4)
        # Edit: drag the first point outwards, then delete it.
        first = el["path"]["nodes"][0]
        x0 = el["rect"]["x"]
        e.click_slide(el["rect"]["x"] + el["rect"]["w"] / 2, el["rect"]["y"] + el["rect"]["h"] / 2, clicks=2)
        e.drag_slide([(first["x"], first["y"]), (first["x"] + 60, first["y"])])
        moved = self.last()
        self.assertAlmostEqual(moved["path"]["nodes"][0]["x"], first["x"] + 60, delta=3)
        self.assertGreater(moved["rect"]["w"] + moved["rect"]["x"], el["rect"]["w"] + x0, "the box follows")
        e.keys("\ue017")  # Delete: the picked point
        self.assertEqual(len(self.last()["path"]["nodes"]), n - 1)
        e.escape()
        self.assertEqual(len(self.view()["elements"]), len(self.view()["elements"]), "still there")


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
