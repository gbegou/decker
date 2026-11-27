"""Builds the demo deck the manual's screenshots are taken of: a short talk
with layouts, sections, a plan, text with formulas, a plot, a table, shapes
and build steps. Usage: python3 make_demo.py OUT.deck"""
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
import decker  # noqa: E402

INK, ACCENT, PALE = "#1F2430", "#C0532F", "#F3EEE6"


def footer():
    return {"type": "text", "rect": {"x": 60, "y": 505, "w": 760, "h": 24},
            "paragraphs": [{"runs": [{"text": "{{section, \" · \", subsection}}", "size": 12, "color": "#6B7080"}]}]}


def number():
    return {"type": "text", "rect": {"x": 820, "y": 505, "w": 100, "h": 24},
            "paragraphs": [{"align": "right", "runs": [{"text": "{{slide, \" / \", slides}}", "size": 12, "color": "#6B7080"}]}]}


def band():
    return {"type": "shape", "kind": "rectangle", "rect": {"x": 0, "y": 0, "w": 960, "h": 8}, "fill": ACCENT}


# One theme: the band, footer and slide number on every slide, the
# placeholder styles; the layouts keep only what differs.
THEMES = [
    {"name": "Waves", "background": "#FFFFFF", "elements": [band(), footer(), number()],
     "placeholders": [
         # The theme is a layout too: its title and body are placed.
         {"name": "title", "rect": {"x": 60, "y": 30, "w": 860, "h": 60}, "size": 32, "bold": True, "color": INK},
         {"name": "subtitle", "rect": {"x": 0, "y": 0, "w": 0, "h": 0}, "size": 24, "color": ACCENT},
         {"name": "body", "rect": {"x": 60, "y": 110, "w": 860, "h": 380}, "size": 22, "bullets": True}]},
]

LAYOUTS = [
    # The title slide changes the theme's background, title size and
    # alignment, and has no footer.
    {"name": "Title slide", "parent": "Waves", "background": PALE, "hide_theme_elements": True, "elements": [band()],
     "placeholders": [
         {"name": "title", "rect": {"x": 80, "y": 170, "w": 800, "h": 90}, "size": 48, "align": "center", "own": ["size", "align"]},
         {"name": "subtitle", "rect": {"x": 80, "y": 280, "w": 800, "h": 50}, "align": "center", "own": ["align"]}]},
    {"name": "Title and content", "parent": "Waves",
     "placeholders": [
         {"name": "title", "rect": {"x": 60, "y": 30, "w": 860, "h": 60}},
         {"name": "body1", "rect": {"x": 60, "y": 110, "w": 860, "h": 380}}]},
    {"name": "Title only", "parent": "Waves",
     "placeholders": [{"name": "title", "rect": {"x": 60, "y": 30, "w": 860, "h": 60}}]},
]


def fill(slide, **texts):
    """Fills the placeholders of a new slide, by name, in order."""
    boxes = [e for e in slide.elements if e.kind == "text"]
    for box, text in zip(boxes, texts.values()):
        box.text = text


def main(out: str):
    deck = decker.new("Waves in a tank")
    deck.data["themes"] = THEMES
    deck.data["layouts"] = LAYOUTS
    deck.theme.update({"accent": ACCENT})

    s = deck.add_slide(layout="Title slide")
    fill(s, title="Waves in a tank", subtitle="A short talk made with decker")

    s = deck.add_slide(layout="Title only")
    fill(s, title="Plan")
    s.add_outline(60, 120, 600, 340, size=24)

    s = deck.add_slide(layout="Title and content", section="Theory", subsection="Shallow water")
    fill(s, title="How fast do waves travel?",
         body="In shallow water, speed depends on depth only: $c = sqrt(g h)$\n"
              "In deep water, on wavelength: $omega^2 = g k$\nLong waves feel the bottom first")
    s.data["notes"] = "Start from the tank experiment; ask who has seen a tsunami map."

    s = deck.add_slide(layout="Title only", subsection="Dispersion")
    fill(s, title="Dispersion relation")
    ks = [i / 10 for i in range(1, 61)]
    s.add_plot(decker.Series([(k, math.sqrt(9.81 * k * math.tanh(k * 0.5))) for k in ks], label="h = 0.5 m"),
               decker.Series([(k, math.sqrt(9.81 * k * math.tanh(k * 2))) for k in ks], label="h = 2 m"),
               x=60, y=110, w=520, h=360, xlabel="$k$ (1/m)", ylabel="$omega$ (1/s)")
    s.add_math(r"omega^2 = g k tanh(k h)", 620, 200, 300, 80, size=28)
    s.add_text("deep water: $tanh -> 1$", 620, 300, 300, 40, size=18).appear(1)

    s = deck.add_slide(layout="Title only", section="Experiment")
    fill(s, title="Measured speeds")
    s.add_table([["Depth (cm)", "Theory (m/s)", "Measured (m/s)"],
                 ["2", "0.44", "0.45"], ["5", "0.70", "0.68"], ["10", "0.99", "0.97"]],
                60, 120, 520, 200, size=18)
    arrow = s.add_shape("arrow", 600, 220, 120, 0, stroke=ACCENT)
    arrow.appear(1)
    s.add_text("within 3 %", 730, 200, 190, 40, size=22).style(color=ACCENT, bold=True).appear(1)

    deck.save(out)


if __name__ == "__main__":
    main(sys.argv[1])
