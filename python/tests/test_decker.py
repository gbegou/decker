import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import decker  # noqa: E402


class DeckTests(unittest.TestCase):
    def test_placeholders_take_their_theme_style(self):
        deck = decker.new("t")
        deck.themes.append({"name": "T", "placeholders": [{"name": "body", "rect": {"x": 0, "y": 0, "w": 0, "h": 0}, "size": 22.0, "bullets": True}]})
        deck.layouts.append({"name": "L", "parent": "T", "placeholders": [
            {"name": "body1", "rect": {"x": 10, "y": 10, "w": 100, "h": 50}, "size": 10.0},
            {"name": "body2", "rect": {"x": 10, "y": 70, "w": 100, "h": 50}, "size": 10.0, "own": ["size"]},
        ]})
        sizes = [p.get("size") for p in deck.placeholders("L")]
        self.assertEqual(sizes, [22.0, 10.0])
        slide = deck.add_slide(layout="L")
        self.assertEqual(slide.data["elements"][0]["paragraphs"][0].get("bullet"), 0, "bullets from the theme")

    def test_build_and_save(self):
        deck = decker.new("Demo")
        deck.theme["font"] = "Libertinus Serif"
        deck.layouts.append({"name": "Corp", "placeholders": [
            {"name": "title", "rect": {"x": 40, "y": 30, "w": 880, "h": 80}, "size": 40, "bold": True, "align": "center"}]})
        s = deck.add_slide(layout="Corp", section="Intro")
        self.assertEqual(s.elements[0].text, "title")
        s.elements[0].text = "Hello\nworld"
        self.assertEqual(s.elements[0].data["paragraphs"][1]["runs"][0], {"size": 40, "bold": True, "text": "world"})
        s.add_text("Body", 40, 130, 880, 300, bullets=True, size=20).appear(1)
        s.add_math("x^2 + y^2 = z^2", 40, 400, 400, 60, size=24)
        s.add_shape("ellipse", 600, 400, 100, 60, fill="#FF0000", text="go").move_to(2, x=700)
        s.add_outline(500, 100, 400, 200)
        arrow = s.add_shape("arrow", 0, 0, 100, 50, stroke="#000000", stroke_width=2, flip=True)
        self.assertEqual((arrow.data["kind"], arrow.data["flip"]), ("arrow", True))
        arrow.remove()
        self.assertEqual(len(deck), 1)
        with tempfile.TemporaryDirectory() as tmp:
            png = Path(tmp) / "pic.png"
            png.write_bytes(b"\x89PNG\r\n\x1a\n" + b"\0" * 8 + (300).to_bytes(4, "big") + (150).to_bytes(4, "big"))
            with self.assertRaises(ValueError):
                s.add_image(png, 0, 0)
            out = deck.save(Path(tmp) / "demo.deck")
            img = s.add_image(png, 10, 10)
            self.assertEqual((img.rect.w, img.rect.h), (300.0, 150.0))
            self.assertEqual(img.data["src"], "assets/pic.png")
            self.assertEqual(s.add_image(png, 0, 0).data["src"], "assets/pic.png", "the same bytes are not copied twice")
            s.add_video(png, 0, 0, 100, 100, play="button", loop=True)
            deck.save()
            back = decker.load(out)
            self.assertEqual(back.title, "Demo")
            self.assertEqual(back.slides[0].section, "Intro")
            self.assertEqual([e.kind for e in back.slide.elements], ["text", "text", "math", "shape", "outline", "image", "image", "media"])
            self.assertEqual(back.slide.elements[1].steps, [{"at": 0, "set": {"opacity": 0.0}}, {"at": 1, "set": {"opacity": 1.0}}])
            self.assertEqual(back.slide.elements[7].data["play"], "button")
            dup = back.slide.duplicate()
            self.assertEqual(dup.index, 1)
            dup.elements[0].remove()
            self.assertEqual(len(dup.elements), len(back.slides[0].elements) - 1)
            json.dumps(back.data)

    def test_plots_generate_cetz_source(self):
        deck = decker.new("p")
        s = deck.add_slide()
        f = s.add_plot([1, 3, 2], decker.Series([4, 5], xs=[0, 2], label="b", mark="x", style="scatter"),
                       x=0, y=0, w=400, h=300, xlabel="t", ylabel="$v$", title="Speed", ymin=0, grid=True)
        self.assertEqual(f.kind, "figure")
        src = f.text
        self.assertIn('#import "@preview/cetz-plot:0.1.4": plot', src)
        self.assertIn("plot.add(((0, 1), (1, 3), (2, 2),), mark: \"o\")", src)
        self.assertIn('plot.add(((0, 4), (2, 5),), mark: "x", style: (stroke: none), label: [b])', src)
        self.assertIn("x-label: [t], y-label: [$v$], legend: \"inner-north-east\", y-min: 0, x-grid: true", src)
        self.assertTrue(src.startswith("#import") and "*Speed*" in src)
        b = s.add_bars(["a", "b#"], {"2024": [1, 2], "2025": [2, 3]}, x=0, y=0, w=10, h=10, ylabel="n")
        self.assertIn('plot.add-bar(((0, 1, 2), (1, 2, 3),), y-key: (1, 2,), mode: "clustered", labels: ([2024], [2025],), bar-width: 0.8)', b.text)
        self.assertIn("x-ticks: ((0, [a]), (1, [b\\#]),)", b.text)
        t = s.add_table([["Name", "Value"], ["pi", 3.14], ["none"]], 0, 0, 400, 90, columns=[2, 1])
        self.assertEqual(t.kind, "table")
        self.assertEqual(t.data["rows"][1][1], {"paragraphs": [{"runs": [{"text": "3.14"}]}]})
        self.assertEqual(t.data["rows"][2], [{"paragraphs": [{"runs": [{"text": "none"}]}]}, {}])
        self.assertEqual((t.data["header"], t.data["columns"], t.data["stroke"]["width"]), (True, [2.0, 1.0], 0.75))
        g = s.add_figure("#draw.circle((0,0))", 0, 0, 10, 10)
        g.text = "x"
        self.assertEqual(g.data["source"], "x")

    def test_macro_protocol(self):
        with tempfile.TemporaryDirectory() as tmp:
            deck = decker.new("m")
            deck.add_slide().add_text("a", 0, 0, 10, 10)
            deck.add_slide().add_text("b", 0, 0, 10, 10)
            src, result = Path(tmp) / "in.json", Path(tmp) / "out.json"
            src.write_text(json.dumps(deck.data))
            script = Path(tmp) / "macro.py"
            script.write_text(
                "import decker\n"
                "deck = decker.current()\n"
                "print('slide', deck.slide.index, [e.text for e in deck.slide.selected])\n"
                "for e in deck.slide.selected:\n"
                "    e.rect.move(5, 5); e.text = e.text.upper()\n"
            )
            env = dict(os.environ, DECKER_DECK=str(src), DECKER_RESULT=str(result), DECKER_SLIDE="1", DECKER_SELECTION="0",
                       PYTHONPATH=str(Path(__file__).resolve().parents[1]))
            run = subprocess.run([sys.executable, str(script)], env=env, capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(run.stdout.strip(), "slide 1 ['b']")
            out = json.loads(result.read_text())
            self.assertEqual(out["slide"], 1)
            self.assertEqual(out["deck"]["slides"][1]["elements"][0]["rect"]["x"], 5.0)
            self.assertEqual(out["deck"]["slides"][1]["elements"][0]["paragraphs"][0]["runs"][0]["text"], "B")
            # discard() leaves no result behind.
            result.unlink()
            script.write_text("import decker\ndecker.current().discard()\n")
            subprocess.run([sys.executable, str(script)], env=env, check=True)
            self.assertFalse(result.exists())


class CommandLineTests(unittest.TestCase):
    def test_run_a_macro_on_a_folder(self):
        with tempfile.TemporaryDirectory() as tmp:
            deck = decker.new("cli")
            deck.add_slide().add_text("a", 0, 0, 10, 10)
            deck.add_slide()
            folder = deck.save(Path(tmp) / "t.deck")
            script = Path(tmp) / "m.py"
            script.write_text("import decker\nd = decker.current()\nd.slide.add_text('added on %d' % d.slide.index, 0, 0, 1, 1)\n")
            env = dict(os.environ, PYTHONPATH=str(Path(__file__).resolve().parents[1]))
            cmd = [sys.executable, "-m", "decker", "run", str(script), str(folder), "--slide", "2"]
            dry = subprocess.run(cmd + ["--dry-run"], env=env, capture_output=True, text=True)
            self.assertEqual((dry.returncode, dry.stdout.strip()), (0, "deck would change (dry run)"), dry.stderr)
            self.assertEqual(len(decker.load(folder).slides[1].elements), 0)
            run = subprocess.run(cmd, env=env, capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(decker.load(folder).slides[1].elements[0].text, "added on 1")
            script.write_text("import decker\ndecker.current()\n")
            same = subprocess.run(cmd, env=env, capture_output=True, text=True)
            self.assertEqual(same.stdout.strip(), "deck unchanged")
            info = subprocess.run([sys.executable, "-m", "decker", "info", str(folder)], env=env, capture_output=True, text=True)
            self.assertIn("cli (960 x 540 pt, 2 slides)", info.stdout)


if __name__ == "__main__":
    unittest.main()
