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
        self.button("Design")
        e.click_slide(400, 60)  # the title placeholder
        self.assertEqual(e.s.js("return document.querySelectorAll('.props .mark').length"), 6, "all from the theme")
        self.set_field(".props .fields input[type=number]", "40")
        self.assertEqual(self.view()["layout_meta"]["overrides"], ["title: size"])
        e.s.js("document.querySelector('.props .mark.own button').click(); return true")  # ↺
        e.settle(1.0)
        self.assertEqual(self.view()["layout_meta"]["overrides"], [])
        self.e.s.js("[...document.querySelectorAll('.tabs button')].find(b => b.textContent.trim() === 'Layout').click(); return true")
        self.e.settle(0.5)
        self.button("Edit theme “Waves”")
        self.assertEqual(self.view()["editing_theme"], "Waves")
        e.s.js("document.querySelector('details.kind').open = true; return true")
        self.set_field("details.kind .fields input[type=number]", "36")
        self.button("Close design view")
        e.goto(2)
        self.button("Design")
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
        e = self.e
        label = {"bezier": "Bézier path", "free": "Freehand"}[value]
        e.s.js("""[...document.querySelectorAll('header .menu-button > button')].find(b => b.textContent.includes('Shape')).click(); return true""")
        e.settle(0.3)
        e.s.js("""[...document.querySelectorAll('.menu-button .menu button')].find(b => b.textContent.includes(arguments[0])).click(); return true""", label)
        e.settle(0.5)

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


class Animations(EditorTest):
    def tab(self, name):
        self.e.s.js("[...document.querySelectorAll('.tabs button')].find(b => b.textContent.trim() === arguments[0]).click(); return true", name)
        self.e.settle(0.6)

    def test_name_then_order_in_the_timeline(self):
        e = self.e
        e.goto(4)  # the table, the arrow and the note appear at click 1
        e.click_slide(820, 220)  # the note
        e.s.js("""const i = document.querySelector('.props label.name input'); i.value = 'Agreement';
                  i.dispatchEvent(new Event('change', {bubbles: true})); return true""")
        e.settle(1.0)
        self.tab("Animations")
        labels = e.s.js("return [...document.querySelectorAll('.timeline .entry b')].map(b => b.textContent)")
        self.assertIn("Agreement", labels, "the name shows in the timeline")
        # Drag the named entry below the last click: a click of its own.
        entry = e.s.js("""const b = [...document.querySelectorAll('.timeline .entry')].find(x => x.textContent.includes('Agreement'));
                          const r = b.getBoundingClientRect(); return [r.x + 20, r.y + r.height / 2]""")
        end = e.rect(".timeline .gap.end")
        acts = [{"type": "pointerMove", "x": int(entry[0]), "y": int(entry[1]), "origin": "viewport"}, {"type": "pointerDown", "button": 0}]
        for k in range(1, 9):
            acts.append({"type": "pointerMove", "duration": 30, "origin": "viewport", "x": int(entry[0]),
                         "y": int(entry[1] + (end[1] + 4 - entry[1]) * k / 8)})
        acts.append({"type": "pointerUp", "button": 0})
        e.s.cmd("POST", "/actions", {"actions": [{"type": "pointer", "id": "m", "parameters": {"pointerType": "mouse"}, "actions": acts}]})
        e.s.cmd("DELETE", "/actions")
        e.settle(1.2)
        self.assertEqual(self.view()["last_step"], 2, "two clicks now")
        rows = e.s.js("return [...document.querySelectorAll('.timeline .row:not(.start)')].map(r => r.textContent)")
        self.assertIn("Agreement", rows[1])
        self.assertNotIn("Agreement", rows[0])

    def test_the_panel_collapses_and_the_notes_fold(self):
        e = self.e
        wide = e.rect(".canvas")[2]
        e.s.js("document.querySelector('.tabs .collapse').click(); return true")
        e.settle(0.8)
        self.assertGreater(e.rect(".canvas")[2], wide + 150, "the slide takes the room")
        self.assertTrue(e.s.js("return !!document.querySelector('textarea.notes')"), "the notes are under the slide")
        e.s.js("document.querySelector('.steps .fold').click(); return true")
        e.settle(0.5)
        self.assertFalse(e.s.js("return !!document.querySelector('textarea.notes')"))


class Ergonomics(EditorTest):
    def menu(self, item):
        self.e.s.js("[...document.querySelectorAll('.context-menu button')].find(b => b.textContent.trim().startsWith(arguments[0])).click(); return true", item)
        self.e.settle(1.0)

    def test_canvas_menu_shortcuts_and_zoom(self):
        e = self.e
        e.goto(3)
        n = len(self.view()["elements"])
        x, y = e.point(300, 300)
        e.pointer(x, y, button=2)  # right-click the figure
        self.assertTrue(e.s.js("return !!document.querySelector('.context-menu')"))
        self.menu("Duplicate")
        e.settle(1.0)
        self.assertEqual(len(self.view()["elements"]), n + 1, "Duplicate from the menu")
        e.chord(decker_app.CTRL, "d")
        e.settle(1.0)
        self.assertEqual(len(self.view()["elements"]), n + 2, "Ctrl+D")
        e.pointer(x, y, button=2)
        self.menu("Appear on next click")
        self.assertEqual(self.view()["last_step"], 2, "a new click at the end")
        e.click_slide(30, 525)  # off the elements
        e.keys("t")
        self.assertEqual(self.view()["elements"][-1]["kind"], "text", "T adds a text box")
        fit = e.rect(".canvas")[2]
        e.s.js("[...document.querySelectorAll('footer .zoom button')].find(b => b.textContent === '100%').click(); return true")
        e.settle(0.6)
        self.assertAlmostEqual(e.rect(".canvas")[2], self.view()["width"], delta=2, msg="100%: a point per pixel")
        e.chord(decker_app.CTRL, "0")
        self.assertAlmostEqual(e.rect(".canvas")[2], fit, delta=2, msg="Ctrl+0 fits again")

    def test_the_slides_pane_collapses_and_resizes(self):
        e = self.e
        w = e.rect("aside.slides")[2]
        e.s.js("document.querySelector('aside.slides .pane-toggle').click(); return true")
        e.settle(0.5)
        self.assertLess(e.rect("aside.slides")[2], 50)
        e.s.js("document.querySelector('aside.slides .pane-toggle').click(); return true")
        e.settle(0.5)
        x, y, rw, rh = e.rect(".pane-resizer")
        acts = [{"type": "pointerMove", "x": int(x + rw / 2), "y": int(y + 200), "origin": "viewport"}, {"type": "pointerDown", "button": 0},
                {"type": "pointerMove", "duration": 100, "x": int(x + rw / 2 + 80), "y": int(y + 200), "origin": "viewport"}, {"type": "pointerUp", "button": 0}]
        e.s.cmd("POST", "/actions", {"actions": [{"type": "pointer", "id": "m", "parameters": {"pointerType": "mouse"}, "actions": acts}]})
        e.s.cmd("DELETE", "/actions")
        e.settle(0.5)
        self.assertAlmostEqual(e.rect("aside.slides")[2], w + 80, delta=4)


class DesignView(EditorTest):
    def test_the_design_view_tree(self):
        e = self.e
        e.goto(2)
        e.s.js("[...document.querySelectorAll('.views button')].find(b => b.textContent.trim() === 'Design').click(); return true")
        e.settle(1.5)
        v = self.view()
        self.assertEqual(v["editing_layout"], "Title and content", "opens on the slide's layout")
        names = e.s.js("return [...document.querySelectorAll('.design-item .design-name')].map(n => n.textContent)")
        self.assertEqual(names[0].strip(), "◆ Waves", names)
        self.assertEqual(len(names), 4, "the theme and its three layouts")
        e.s.js("document.querySelector('.design-item.theme').click(); return true")
        e.settle(1.0)
        self.assertEqual(self.view()["editing_theme"], "Waves")
        x, y = e.s.js("const r = document.querySelector('.design-item.theme').getBoundingClientRect(); return [r.x + 30, r.y + 20]")
        e.pointer(x, y, button=2)
        e.s.js("[...document.querySelectorAll('.context-menu button')].find(b => b.textContent.trim() === 'Two contents').click(); return true")
        e.settle(1.5)
        v = self.view()
        self.assertEqual(v["editing_layout"], "Two contents")
        self.assertEqual(v["layout_meta"]["parent"], "Waves", "made in the theme right-clicked")
        e.s.js("[...document.querySelectorAll('button')].find(b => b.textContent.trim() === 'Close design view').click(); return true")
        e.settle(1.5)
        v = self.view()
        self.assertIsNone(v["editing_layout"])
        self.assertIsNone(v["editing_theme"])
        self.assertEqual(v["slide"], 2, "back on the slide")


class Editing(EditorTest):
    def test_all_elements_by_default_and_the_animation_mark(self):
        e = self.e
        e.s.js("document.querySelector(\".stack-item[data-index='4'] .thumb\").click(); return true")
        e.settle(1.0)
        v = self.view()
        self.assertTrue(v["all"], "a slide opens with every element shown")
        self.assertNotIn('opacity="0"', v["svg"], "the elements hidden at the start are visible")
        marks = e.s.js("return [...document.querySelectorAll('.stack-item')].map(i => !!i.querySelector('.anim'))")
        self.assertEqual(marks, [False, False, False, True, True], "a mark on the slides with animation")
        e.s.js("[...document.querySelectorAll('.clicks button')].find(b => b.textContent === '▶').click(); return true")
        e.settle(0.8)
        v = self.view()
        self.assertEqual((v["all"], v["step"]), (False, 0))
        self.assertIn('opacity="0"', v["svg"], "the slide as it opens")

    def test_keys_unsaved_mark_and_right_click(self):
        e = self.e
        e.goto(4, 1)
        self.assertFalse(e.s.js("return !!document.querySelector('.dirty')"))
        e.click_slide(820, 220)
        x0 = next(x for x in self.view()["elements"] if x["text"] == "within 3 %")["rect"]["x"]
        e.keys("\ue014")  # Right
        e.chord(decker_app.SHIFT, "\ue014")
        self.assertEqual(next(x for x in self.view()["elements"] if x["text"] == "within 3 %")["rect"]["x"], x0 + 11)
        self.assertTrue(e.s.js("return !!document.querySelector('.dirty')"), "unsaved changes are marked")
        e.chord(decker_app.CTRL, "s")
        self.assertFalse(e.s.js("return !!document.querySelector('.dirty')"), "saved")
        e.chord(decker_app.CTRL, "a")
        self.assertEqual(e.s.js("return document.querySelectorAll('.hit.selected').length"), len(self.view()["elements"]))
        e.escape()
        self.assertEqual(e.s.js("return document.querySelectorAll('.hit.selected').length"), 0)
        x, y = e.point(820, 220)
        e.pointer(x, y, button=2)
        self.assertEqual(e.s.js("return document.querySelectorAll('line.guide').length"), 0, "a right-click drags nothing")
        self.assertTrue(e.s.js("return !!document.querySelector('.context-menu')"))

    def test_on_click_with_and_after_previous(self):
        e = self.e
        e.goto(4, 1)
        e.s.js("[...document.querySelectorAll('.tabs button')].find(b => b.textContent.trim() === 'Animations').click(); return true")
        e.settle(0.6)
        pick = """const sel = [...document.querySelectorAll('.timeline .row:not(.start) .entry')][arguments[0]].querySelector('select.when');
                  sel.value = arguments[1]; sel.dispatchEvent(new Event('change', {bubbles: true})); return true"""
        self.assertEqual(self.view()["last_step"], 1)
        e.s.js(pick, 1, "click")   # the second effect of click 1: a click of its own
        e.settle(1.0)
        self.assertEqual(self.view()["last_step"], 2)
        e.s.js(pick, 1, "after")   # back into click 1, after the first
        e.settle(1.0)
        v = self.view()
        self.assertEqual(v["last_step"], 1)
        steps = [s for x in v["elements"] for s in x["steps"] if s["at"] == 1]
        self.assertEqual(sorted(s.get("trigger", "with") for s in steps), ["after", "with"])


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
