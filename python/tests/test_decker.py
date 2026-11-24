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


if __name__ == "__main__":
    unittest.main()
