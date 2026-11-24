#!/usr/bin/env python3
"""Generates the architecture diagrams of the manual as SVG files.

Run from the repository root: ``python3 docs/diagrams/generate.py``.
Output goes to ``docs/src/images/``. Only the standard library is used, so
the diagrams stay reproducible on any machine and in CI.
"""

from __future__ import annotations

import html
from dataclasses import dataclass, field
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "src" / "images"

FONT = "system-ui, -apple-system, 'Segoe UI', Helvetica, Arial, sans-serif"
MONO = "ui-monospace, 'Cascadia Mono', Consolas, monospace"

# A small palette; a fill and its border.
RUST = ("#fde8d7", "#c9742f")
WEB = ("#dceeff", "#2f72c9")
FILE = ("#eeeeee", "#777777")
PLAN = ("#f4f4f4", "#aaaaaa")
NOTE = ("#fff8d6", "#c2a200")


@dataclass
class Box:
    x: float
    y: float
    w: float
    h: float
    title: str
    lines: list[str] = field(default_factory=list)
    style: tuple[str, str] = RUST
    dashed: bool = False
    mono: bool = False

    @property
    def cx(self) -> float:
        return self.x + self.w / 2

    @property
    def cy(self) -> float:
        return self.y + self.h / 2

    def svg(self) -> str:
        fill, stroke = self.style
        dash = ' stroke-dasharray="6 4"' if self.dashed else ""
        out = [
            f'<rect x="{self.x}" y="{self.y}" width="{self.w}" height="{self.h}" rx="8" '
            f'fill="{fill}" stroke="{stroke}" stroke-width="1.5"{dash}/>'
        ]
        n = 1 + len(self.lines)
        line_h = 17
        top = self.cy - (n - 1) * line_h / 2
        out.append(
            f'<text x="{self.cx}" y="{top}" text-anchor="middle" dominant-baseline="middle" '
            f'font-weight="600" font-size="14">{html.escape(self.title)}</text>'
        )
        for i, line in enumerate(self.lines, start=1):
            family = f' font-family="{MONO}"' if self.mono else ""
            out.append(
                f'<text x="{self.cx}" y="{top + i * line_h}" text-anchor="middle" dominant-baseline="middle" '
                f'font-size="12" fill="#333"{family}>{html.escape(line)}</text>'
            )
        return "\n".join(out)


def arrow(x1, y1, x2, y2, label="", both=False, dashed=False, color="#444", label_dy=-6, mono=False) -> str:
    dash = ' stroke-dasharray="5 4"' if dashed else ""
    start = ' marker-start="url(#dot)"' if both else ""
    out = [
        f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="1.5" '
        f'marker-end="url(#arrow)"{start}{dash}/>'
    ]
    if label:
        mx, my = (x1 + x2) / 2, (y1 + y2) / 2
        family = f' font-family="{MONO}"' if mono else ""
        out.append(
            f'<text x="{mx}" y="{my + label_dy}" text-anchor="middle" font-size="11" fill="{color}"{family} '
            f'paint-order="stroke" stroke="white" stroke-width="4">{html.escape(label)}</text>'
        )
    return "\n".join(out)


def group(x, y, w, h, title, color="#999") -> str:
    return (
        f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" fill="none" stroke="{color}" '
        f'stroke-width="1" stroke-dasharray="8 5"/>'
        f'<text x="{x + 12}" y="{y + 18}" font-size="12" font-weight="600" fill="{color}" '
        f'letter-spacing="0.5">{html.escape(title.upper())}</text>'
    )


def document(width, height, body: str) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" font-family="{FONT}">
<defs>
  <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">
    <path d="M 0 0 L 10 5 L 0 10 z" fill="#444"/>
  </marker>
  <marker id="dot" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="6" markerHeight="6">
    <circle cx="5" cy="5" r="4" fill="#444"/>
  </marker>
</defs>
<rect width="100%" height="100%" fill="white"/>
{body}
</svg>
"""


def stack() -> str:
    """The whole stack: files, Rust crates, the webview windows."""
    b = {}
    b["pptx"] = Box(40, 60, 130, 50, ".pptx", style=FILE)
    b["deck"] = Box(40, 150, 130, 60, "talk.deck/", ["deck.json + assets/"], style=FILE)
    b["pdf"] = Box(40, 250, 130, 50, ".pdf", ["planned"], style=PLAN, dashed=True)

    b["convert"] = Box(260, 60, 190, 60, "deck-convert", ["PPTX import / export"])
    b["core"] = Box(260, 150, 190, 70, "deck-core", ["model · native format", "SVG renderer"])
    b["typst"] = Box(260, 250, 190, 50, "deck-typst", ["Typst: text, math, PDF"])
    b["app"] = Box(540, 110, 200, 90, "deck-app (Tauri)", ["state · commands", "windows"])
    b["py"] = Box(540, 250, 200, 50, "Python macros", ["PyO3, planned"], style=PLAN, dashed=True)

    b["editor"] = Box(840, 55, 200, 65, "Editor window", ["slide list · canvas", "properties · notes"], style=WEB)
    b["audience"] = Box(840, 135, 200, 55, "Audience window", ["fullscreen on the projector"], style=WEB)
    b["presenter"] = Box(840, 205, 200, 65, "Presenter window", ["current · next · notes", "timer"], style=WEB)

    parts = [
        group(20, 25, 170, 300, "Files"),
        group(240, 25, 520, 300, "Rust, one binary", RUST[1]),
        group(820, 25, 240, 300, "Webview", WEB[1]),
        '<text x="940" y="300" text-anchor="middle" font-size="11" fill="#555">system engine:</text>'
        '<text x="940" y="314" text-anchor="middle" font-size="11" fill="#555">WebView2 on Windows, WebKitGTK on Linux</text>',
    ]
    parts += [x.svg() for x in b.values()]
    parts += [
        arrow(170, 85, 260, 85, both=True),
        arrow(355, 120, 355, 150, "Deck"),
        arrow(170, 180, 260, 180, both=True, label="serde JSON"),
        arrow(260, 275, 170, 275, dashed=True),
        arrow(355, 250, 355, 220, "typeset SVG"),
        arrow(450, 185, 540, 160, "render_step → SVG", label_dy=-10),
        arrow(640, 250, 640, 200, dashed=True, label="same state functions", label_dy=14),
        arrow(740, 140, 840, 88, "View", label_dy=-8),
        arrow(740, 155, 840, 162, "View", label_dy=-6),
        arrow(740, 170, 840, 237, "View", label_dy=14),
        arrow(840, 110, 740, 185, "commands", color="#2f72c9", label_dy=16),
    ]
    return document(1080, 340, "\n".join(parts))


def pipeline() -> str:
    """From the file on disk to pixels, top to bottom."""
    steps = [
        ("deck.json", ["on disk"], FILE),
        ("Deck", ["Rust struct"], RUST),
        ("SVG string", ["one <g id=\"eN\"> per element"], RUST),
        ("View", ["svg, next_svg, notes, elements…"], RUST),
        ("Webview DOM", ["Svelte component"], WEB),
        ("Pixels", ["fonts, GPU"], WEB),
    ]
    labels = ["serde", "render_step(slide, step)", "inline assets as data URIs", "command result or event", "browser engine"]
    boxes = []
    y = 20
    for title, lines, style in steps:
        boxes.append(Box(40, y, 260, 60, title, lines, style=style))
        y += 100
    parts = [b.svg() for b in boxes]
    for i, label in enumerate(labels):
        a, c = boxes[i], boxes[i + 1]
        parts.append(arrow(a.cx, a.y + a.h, c.cx, c.y))
        family = f' font-family="{MONO}"' if i in (0, 1) else ""
        parts.append(
            f'<text x="{a.cx + 20}" y="{(a.y + a.h + c.y) / 2 + 4}" font-size="12" fill="#444"{family}>{html.escape(label)}</text>'
        )
    note = [
        "render_step is a pure function of the model",
        "and the step: the same SVG serves the editor,",
        "the audience and presenter windows, the",
        "thumbnails, and later the PDF export.",
    ]
    for i, line in enumerate(note):
        parts.append(f'<text x="360" y="{250 + i * 17}" font-size="12" fill="#555">{html.escape(line)}</text>')
    return document(660, y - 30, "\n".join(parts))


def truth() -> str:
    """Sequence: one source of truth."""
    cols = {"P": 120, "R": 420, "A": 720, "E": 1020}
    names = {"P": "Presenter window", "R": "Rust state", "A": "Audience window", "E": "Editor window"}
    sub = {"R": "Mutex<AppState>"}
    styles = {"P": WEB, "R": RUST, "A": WEB, "E": WEB}
    parts = []
    for k, x in cols.items():
        parts.append(Box(x - 80, 20, 160, 50, names[k], [sub.get(k, "")] if k in sub else [], style=styles[k], mono=True).svg())
        parts.append(f'<line x1="{x}" y1="70" x2="{x}" y2="330" stroke="#bbb" stroke-dasharray="4 4"/>')

    def msg(y, a, b, label, dashed=False, mono=False):
        return arrow(cols[a], y, cols[b], y, label, dashed=dashed, mono=mono)

    parts += [
        msg(100, "P", "R", 'navigate("next")', mono=True),
        f'<rect x="{cols["R"] - 6}" y="120" width="12" height="40" fill="{RUST[0]}" stroke="{RUST[1]}"/>',
        f'<text x="{cols["R"] + 14}" y="143" font-size="11" fill="#333">step += 1, or next slide</text>',
        msg(180, "R", "P", "View", dashed=True),
        msg(200, "R", "A", 'event "view-changed"', dashed=True),
        arrow(cols["R"], 220, cols["E"], 220, 'event "view-changed"', dashed=True),
        msg(260, "A", "R", "get_view()", mono=True),
        msg(280, "R", "A", "View at the new step", dashed=True),
        arrow(cols["E"], 300, cols["R"], 300, "get_view()", mono=True),
        arrow(cols["R"], 320, cols["E"], 320, "View", dashed=True),
        '<text x="570" y="360" text-anchor="middle" font-size="12" fill="#555">'
        "Windows never talk to each other and keep no state beyond what they draw. "
        "A Python macro will call the same state functions as the commands.</text>",
    ]
    return document(1140, 380, "\n".join(parts))


def steps() -> str:
    """Build steps as successive overrides."""
    boxes = [
        Box(20, 40, 170, 70, "base", ["rect 50,40 500×60", "opacity 1"], style=RUST),
        Box(250, 40, 170, 70, "step 0", ["opacity: 0"], style=NOTE, mono=True),
        Box(480, 40, 170, 70, "step 1", ["opacity: 1", "duration 0.3 s"], style=NOTE, mono=True),
        Box(710, 40, 170, 70, "step 2", ["x: 600", "fill: #0088FF"], style=NOTE, mono=True),
    ]
    parts = [b.svg() for b in boxes]
    for a, c in zip(boxes, boxes[1:]):
        parts.append(arrow(a.x + a.w, a.cy, c.x, c.cy, "apply"))
    # The resulting states
    y = 160
    states = ["hidden", "visible, fades in", "moved to x=600, blue"]
    for i, s in enumerate(states):
        bx = boxes[i + 1]
        parts.append(
            f'<text x="{bx.cx}" y="{y}" text-anchor="middle" font-size="12" fill="#333">state_at({i}): {html.escape(s)}</text>'
        )
    parts.append(
        '<text x="450" y="200" text-anchor="middle" font-size="12" fill="#555">'
        "Appear, disappear, move, grow, spin and recolor are one mechanism: overrides applied in order up to the step.</text>"
    )
    return document(900, 220, "\n".join(parts))


def windows() -> str:
    """Monitors and windows during a presentation."""
    parts = [
        group(20, 20, 520, 250, "Laptop screen, primary monitor"),
        group(580, 20, 380, 250, "Projector, second monitor"),
        Box(50, 60, 200, 180, "Editor", ["stays open"], style=WEB).svg(),
        Box(280, 60, 230, 180, "Presenter", ["current slide", "next step or slide", "notes · timer", "thumbnails"], style=WEB).svg(),
        Box(610, 60, 320, 180, "Audience", ["fullscreen, no decorations", "black on B", "click or keys: next"], style=WEB).svg(),
        arrow(510, 150, 610, 150, "same Rust state", dashed=True, both=True),
        '<text x="490" y="300" text-anchor="middle" font-size="12" fill="#555">'
        "With a single monitor only the audience window opens, fullscreen. "
        "Keys everywhere: arrows, Page Up/Down, Space, Enter, Home, End, B, Escape.</text>",
    ]
    return document(980, 320, "\n".join(parts))


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fn in {
        "stack": stack,
        "pipeline": pipeline,
        "source-of-truth": truth,
        "build-steps": steps,
        "windows": windows,
    }.items():
        (OUT / f"{name}.svg").write_text(fn(), encoding="utf-8")
        print(f"wrote {OUT / name}.svg")


if __name__ == "__main__":
    main()
