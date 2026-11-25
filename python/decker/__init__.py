"""decker: the document model of decker decks, for scripts and macros.

A deck is a folder holding ``deck.json`` and ``assets/``. This package
wraps that JSON with helpers so that a script can build or change a deck
in a few lines, standalone or as a macro run from the editor.

Standalone::

    import decker
    deck = decker.load("talk.deck")        # or decker.new("Title")
    s = deck.add_slide(layout="Corp")
    s.add_text("Hello", x=40, y=30, w=880, h=80, size=40, bold=True)
    deck.save("talk.deck")

As a macro (``Run script…`` in the editor, or a file of the deck's
``scripts/`` folder)::

    import decker
    deck = decker.current()               # the open deck
    for e in deck.slide.selected:         # the selected elements
        e.rect.w *= 1.1
    deck.commit()                         # applied as one undo step

Without ``commit()``, the deck is applied when the script ends, unless
``deck.discard()`` was called. Anything printed shows up in the editor.
"""

from __future__ import annotations

import atexit
import copy
import json
import os
import shutil
from pathlib import Path
from typing import Any, Iterator

__all__ = ["Deck", "Slide", "Element", "Rect", "Series", "load", "new", "current", "VERSION"]

VERSION = 1


class Rect:
    """An element's box in points, as a view on the element's dict."""

    __slots__ = ("_d",)

    def __init__(self, d: dict):
        self._d = d

    def _get(self, k: str) -> float:
        return float(self._d.get(k, 0.0))

    def _set(self, k: str, v: float) -> None:
        self._d[k] = float(v)

    x = property(lambda s: s._get("x"), lambda s, v: s._set("x", v))
    y = property(lambda s: s._get("y"), lambda s, v: s._set("y", v))
    w = property(lambda s: s._get("w"), lambda s, v: s._set("w", v))
    h = property(lambda s: s._get("h"), lambda s, v: s._set("h", v))

    def move(self, dx: float = 0.0, dy: float = 0.0) -> "Rect":
        self.x += dx
        self.y += dy
        return self

    def __repr__(self) -> str:
        return f"Rect(x={self.x:g}, y={self.y:g}, w={self.w:g}, h={self.h:g})"


class Element:
    """One element of a slide or layout: a thin view on its JSON dict.

    ``kind`` is ``text``, ``image``, ``media``, ``shape``, ``math``,
    ``outline`` or ``group``; the raw dict is ``data`` for anything the
    helpers do not cover (see the format chapter of the manual).
    """

    def __init__(self, data: dict, owner: "Slide | None" = None):
        self.data = data
        self._owner = owner

    @property
    def kind(self) -> str:
        return self.data.get("type", "")

    @property
    def rect(self) -> Rect:
        return Rect(self.data.setdefault("rect", {"x": 0, "y": 0, "w": 100, "h": 50}))

    @property
    def steps(self) -> list:
        return self.data.setdefault("steps", [])

    # ----- text
    @property
    def text(self) -> str:
        """The plain text of a text box or shape (paragraphs joined by newlines), a formula's or figure's source."""
        if self.kind in ("math", "figure"):
            return self.data.get("source", "")
        return "\n".join("".join(r.get("text", "") for r in p.get("runs", [])) for p in self.data.get("paragraphs", []))

    @text.setter
    def text(self, value: str) -> None:
        if self.kind in ("math", "figure"):
            self.data["source"] = value
            return
        if self.kind not in ("text", "shape"):
            raise TypeError(f"a {self.kind} has no text")
        old = self.data.get("paragraphs", [])
        style = {k: v for k, v in (old[0]["runs"][0] if old and old[0].get("runs") else {}).items() if k != "text"}
        align = old[0].get("align") if old else None
        paragraphs = []
        for line in value.split("\n"):
            p: dict[str, Any] = {"runs": [dict(style, text=line)]}
            if align:
                p["align"] = align
            paragraphs.append(p)
        self.data["paragraphs"] = paragraphs

    def style(self, **run_style: Any) -> "Element":
        """Sets run properties on every run: ``bold``, ``italic``, ``size``, ``color`` (``"#RRGGBB"``), ``font``."""
        for p in self.data.get("paragraphs", []):
            for r in p.get("runs", []):
                for k, v in run_style.items():
                    if v is None:
                        r.pop(k, None)
                    else:
                        r[k] = v
        return self

    # ----- build steps
    def appear(self, at: int = 1) -> "Element":
        """Hidden until click ``at``."""
        self.steps[:] = [s for s in self.steps if s.get("at") not in (0, at)]
        self.steps.extend([{"at": 0, "set": {"opacity": 0.0}}, {"at": at, "set": {"opacity": 1.0}}])
        self.steps.sort(key=lambda s: s.get("at", 0))
        return self

    def disappear(self, at: int = 1) -> "Element":
        """Shown until click ``at``."""
        self.steps[:] = [s for s in self.steps if s.get("at") != at]
        self.steps.append({"at": at, "set": {"opacity": 0.0}})
        self.steps.sort(key=lambda s: s.get("at", 0))
        return self

    def move_to(self, at: int, x: float | None = None, y: float | None = None, w: float | None = None, h: float | None = None) -> "Element":
        """A move step at click ``at`` to the given coordinates."""
        over = {k: float(v) for k, v in (("x", x), ("y", y), ("w", w), ("h", h)) if v is not None}
        self.steps[:] = [s for s in self.steps if s.get("at") != at]
        self.steps.append({"at": at, "set": over})
        self.steps.sort(key=lambda s: s.get("at", 0))
        return self

    def remove(self) -> None:
        """Removes the element from its slide."""
        if self._owner is None:
            raise ValueError("the element is not on a slide")
        self._owner._elements().remove(self.data)

    def __repr__(self) -> str:
        return f"Element({self.kind}, {self.rect!r})"


class Slide:
    """A slide: elements, layout, section, notes."""

    def __init__(self, data: dict, deck: "Deck"):
        self.data = data
        self.deck = deck

    def _elements(self) -> list:
        return self.data.setdefault("elements", [])

    @property
    def index(self) -> int:
        return next(i for i, s in enumerate(self.deck.data["slides"]) if s is self.data)

    @property
    def elements(self) -> list[Element]:
        return [Element(e, self) for e in self._elements()]

    @property
    def selected(self) -> list[Element]:
        """The elements selected in the editor, when run as a macro on this slide."""
        if self.deck._selection is None or self.deck._slide != self.index:
            return []
        els = self._elements()
        return [Element(els[i], self) for i in self.deck._selection if i < len(els)]

    layout = property(lambda s: s.data.get("layout"), lambda s, v: s._set_opt("layout", v))
    section = property(lambda s: s.data.get("section"), lambda s, v: s._set_opt("section", v))
    subsection = property(lambda s: s.data.get("subsection"), lambda s, v: s._set_opt("subsection", v))
    notes = property(lambda s: s.data.get("notes", ""), lambda s, v: s._set_opt("notes", v or None))
    background = property(lambda s: s.data.get("background"), lambda s, v: s._set_opt("background", v))

    def _set_opt(self, key: str, value: Any) -> None:
        if value is None:
            self.data.pop(key, None)
        else:
            self.data[key] = value

    def add(self, element: dict) -> Element:
        """Appends a raw element dict (see the format chapter) and returns it."""
        self._elements().append(element)
        return Element(element, self)

    def add_text(self, text: str, x: float, y: float, w: float, h: float, *, size: float | None = None,
                 bold: bool = False, italic: bool = False, color: str | None = None, font: str | None = None,
                 align: str | None = None, bullets: bool = False, placeholder: str | None = None) -> Element:
        """A text box; one paragraph per line of ``text``."""
        run_style: dict[str, Any] = {}
        if size is not None:
            run_style["size"] = float(size)
        if bold:
            run_style["bold"] = True
        if italic:
            run_style["italic"] = True
        if color:
            run_style["color"] = color
        if font:
            run_style["font"] = font
        paragraphs = []
        for line in text.split("\n"):
            p: dict[str, Any] = {"runs": [dict(run_style, text=line)]}
            if align:
                p["align"] = align
            if bullets:
                p["bullet"] = 0
            paragraphs.append(p)
        e: dict[str, Any] = {"type": "text", "rect": _rect(x, y, w, h), "paragraphs": paragraphs}
        if placeholder:
            e["placeholder"] = placeholder
        return self.add(e)

    def add_math(self, source: str, x: float, y: float, w: float, h: float, *, size: float | None = None, color: str | None = None) -> Element:
        """A display formula in Typst math syntax."""
        e: dict[str, Any] = {"type": "math", "rect": _rect(x, y, w, h), "source": source}
        if size is not None:
            e["size"] = float(size)
        if color:
            e["color"] = color
        return self.add(e)

    def add_shape(self, kind: str, x: float, y: float, w: float, h: float, *, fill: str | None = None,
                  stroke: str | None = None, stroke_width: float = 1.0, text: str = "", flip: bool = False) -> Element:
        """``rectangle``, ``ellipse``, ``line`` or ``arrow``; ``flip=True`` runs a line or arrow from the bottom-left to the top-right."""
        e: dict[str, Any] = {"type": "shape", "kind": kind, "rect": _rect(x, y, w, h)}
        if flip:
            e["flip"] = True
        if fill:
            e["fill"] = fill
        if stroke:
            e["stroke"] = {"color": stroke, "width": float(stroke_width)}
        el = self.add(e)
        if text:
            el.text = text
        return el

    def add_image(self, path: str, x: float, y: float, w: float | None = None, h: float | None = None) -> Element:
        """A picture: a file copied into ``assets/`` (or an ``assets/…`` path already there).

        Without ``w`` and ``h`` the box is the picture's pixel size (PNG, JPEG, GIF, WebP)
        or 300×200.
        """
        src = self.deck.add_asset(path)
        if w is None or h is None:
            pw, ph = _image_size(self.deck.dir / src) if self.deck.dir else (None, None)
            w, h = (float(pw), float(ph)) if pw else (300.0, 200.0)
        return self.add({"type": "image", "rect": _rect(x, y, w, h), "src": src})

    def add_video(self, path: str, x: float, y: float, w: float, h: float, *, play: str = "on_appear",
                  loop: bool = False, sound: bool = False) -> Element:
        """A video or animated GIF; ``play`` is ``on_appear`` or ``button``."""
        e: dict[str, Any] = {"type": "media", "rect": _rect(x, y, w, h), "src": self.deck.add_asset(path)}
        if play != "on_appear":
            e["play"] = play
        if loop:
            e["loop"] = True
        if sound:
            e["sound"] = True
        return self.add(e)

    def add_figure(self, source: str, x: float, y: float, w: float, h: float) -> Element:
        """A Typst figure: any markup, with ``@preview/cetz:0.5.2`` and ``@preview/cetz-plot:0.1.4`` bundled."""
        return self.add({"type": "figure", "rect": _rect(x, y, w, h), "source": source})

    def add_plot(self, *series: "Series | list | tuple", x: float, y: float, w: float, h: float,
                 xlabel: str = "", ylabel: str = "", title: str = "", legend: str | None = "inner-north-east",
                 xmin: float | None = None, xmax: float | None = None, ymin: float | None = None, ymax: float | None = None,
                 size: tuple[float, float] = (12, 6), grid: bool = False) -> Element:
        """A line or scatter plot of one or more series, as a figure.

        A series is ``Series(ys, xs=None, label=, mark=, style=)`` or just a list of
        ``y`` values (``x`` = 0, 1, 2…) or of ``(x, y)`` pairs. ``mark`` is ``"o"``,
        ``"x"``, ``"+"``, ``"*"`` or ``None``; ``style`` is ``line``, ``scatter`` or ``bars``.
        Labels take Typst markup (``$x$`` for math).
        """
        body = "".join(_series_code(s) for s in series)
        return self.add_figure(_plot_source(body, xlabel, ylabel, title, legend, xmin, xmax, ymin, ymax, size, grid), x, y, w, h)

    def add_bars(self, labels: list[str], values: "list[float] | dict[str, list[float]]", *, x: float, y: float,
                 w: float, h: float, ylabel: str = "", title: str = "", size: tuple[float, float] = (12, 6)) -> Element:
        """A bar chart: one value per label, or a dict of named series (grouped bars)."""
        groups = values if isinstance(values, dict) else {"": list(values)}
        names = list(groups)
        rows = []
        for i in range(len(labels)):
            ys = ", ".join(f"{float(groups[nm][i]):g}" if i < len(groups[nm]) else "0" for nm in names)
            rows.append(f"({i}, {ys})")
        keys = ", ".join(str(k + 1) for k in range(len(names)))
        if len(names) > 1:
            series_labels = ", ".join(f"[{_typst_text(nm)}]" for nm in names)
            body = f"    plot.add-bar(({', '.join(rows)},), y-key: ({keys},), mode: \"clustered\", labels: ({series_labels},), bar-width: 0.8)\n"
        else:
            body = f"    plot.add-bar(({', '.join(rows)},), bar-width: 0.6)\n"
        ticks = ", ".join(f"({i}, [{_typst_text(lab)}])" for i, lab in enumerate(labels))
        extra = f"x-tick-step: none, x-ticks: ({ticks},), x-min: -0.6, x-max: {len(labels) - 0.4:g}, y-min: 0, "
        return self.add_figure(_plot_source(body, "", ylabel, title, "inner-north-east" if len(names) > 1 else None,
                                            None, None, None, None, size, False, extra), x, y, w, h)

    def add_outline(self, x: float, y: float, w: float, h: float, *, size: float | None = None) -> Element:
        """The plan: the deck's sections with this slide's own highlighted."""
        e: dict[str, Any] = {"type": "outline", "rect": _rect(x, y, w, h)}
        if size is not None:
            e["size"] = float(size)
        return self.add(e)

    def remove(self) -> None:
        self.deck.data["slides"].remove(self.data)

    def duplicate(self) -> "Slide":
        """A copy inserted right after this slide."""
        copy_ = copy.deepcopy(self.data)
        slides = self.deck.data["slides"]
        slides.insert(self.index + 1, copy_)
        return Slide(copy_, self.deck)

    def __repr__(self) -> str:
        return f"Slide({self.index}, {len(self._elements())} elements)"


class Deck:
    """A deck: ``slides``, ``layouts`` (raw dicts), ``theme``, title and size."""

    def __init__(self, data: dict, dir: "str | os.PathLike | None" = None):
        self.data = data
        self.dir = Path(dir) if dir else None
        self._slide: int | None = None
        self._selection: list[int] | None = None
        self._result: str | None = None
        self._done = False
        data.setdefault("version", VERSION)
        data.setdefault("slides", [])

    # ----- document
    title = property(lambda s: s.data.get("title", ""), lambda s, v: s.data.__setitem__("title", v))
    width = property(lambda s: float(s.data.get("width", 960)), lambda s, v: s.data.__setitem__("width", float(v)))
    height = property(lambda s: float(s.data.get("height", 540)), lambda s, v: s.data.__setitem__("height", float(v)))

    @property
    def slides(self) -> list[Slide]:
        return [Slide(s, self) for s in self.data["slides"]]

    @property
    def layouts(self) -> list[dict]:
        return self.data.setdefault("layouts", [])

    @property
    def theme(self) -> dict:
        return self.data.setdefault("theme", {})

    @property
    def slide(self) -> Slide:
        """The slide shown in the editor (a macro), else the first one."""
        slides = self.data["slides"]
        if not slides:
            raise IndexError("the deck has no slide")
        return Slide(slides[min(self._slide or 0, len(slides) - 1)], self)

    def __iter__(self) -> Iterator[Slide]:
        return iter(self.slides)

    def __len__(self) -> int:
        return len(self.data["slides"])

    def add_slide(self, *, layout: str | None = None, at: int | None = None, section: str | None = None,
                  subsection: str | None = None) -> Slide:
        """A new slide, from the layout's placeholders when ``layout`` names one."""
        s: dict[str, Any] = {"elements": []}
        if layout:
            s["layout"] = layout
            lay = next((l for l in self.layouts if l.get("name") == layout), None)
            if lay is None:
                raise KeyError(f"no layout named {layout!r}")
            for ph in lay.get("placeholders", []):
                run = {k: ph[k] for k in ("size", "bold", "color", "font") if ph.get(k)}
                p: dict[str, Any] = {"runs": [dict(run, text=ph.get("name", ""))]}
                if ph.get("align"):
                    p["align"] = ph["align"]
                if ph.get("bullets"):
                    p["bullet"] = 0
                s["elements"].append({"type": "text", "rect": dict(ph["rect"]), "placeholder": ph.get("name"), "paragraphs": [p]})
        if section:
            s["section"] = section
        if subsection:
            s["subsection"] = subsection
        slides = self.data["slides"]
        slides.insert(len(slides) if at is None else at, s)
        return Slide(s, self)

    def layout(self, name: str) -> dict | None:
        return next((l for l in self.layouts if l.get("name") == name), None)

    def add_asset(self, path: "str | os.PathLike") -> str:
        """Copies a file into ``assets/`` (unless already an ``assets/…`` path) and returns its deck-relative path."""
        p = str(path).replace("\\", "/")
        if p.startswith("assets/"):
            return p
        if self.dir is None:
            raise ValueError("the deck has no folder yet: save it first, or pass an assets/ path")
        src = Path(path)
        assets = self.dir / "assets"
        assets.mkdir(parents=True, exist_ok=True)
        name, n = src.name, 1
        while (assets / name).exists() and (assets / name).read_bytes() != src.read_bytes():
            n += 1
            name = f"{src.stem}-{n}{src.suffix}"
        if not (assets / name).exists():
            shutil.copyfile(src, assets / name)
        return f"assets/{name}"

    # ----- files
    def save(self, path: "str | os.PathLike | None" = None) -> Path:
        """Writes ``deck.json`` into the deck folder (created if needed)."""
        if path is not None:
            self.dir = Path(path)
        if self.dir is None:
            raise ValueError("no folder: deck.save('talk.deck')")
        self.dir.mkdir(parents=True, exist_ok=True)
        (self.dir / "deck.json").write_text(json.dumps(self.data, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        return self.dir

    # ----- macros
    def commit(self) -> None:
        """Hands the deck back to the editor now (a macro); a no-op standalone."""
        if self._result and not self._done:
            Path(self._result).write_text(json.dumps({"deck": self.data, "slide": self._slide}), encoding="utf-8")
            self._done = True

    def discard(self) -> None:
        """Leaves the editor's deck as it was."""
        self._done = True

    def __repr__(self) -> str:
        return f"Deck({self.title!r}, {len(self)} slides)"


class Series:
    """One series of a plot: ``ys`` (with ``xs``, or ``(x, y)`` pairs), a legend label, a mark and a style."""

    def __init__(self, ys: "list", xs: "list | None" = None, *, label: str = "", mark: str | None = "o", style: str = "line"):
        if xs is not None:
            self.points = [(float(a), float(b)) for a, b in zip(xs, ys)]
        elif ys and isinstance(ys[0], (tuple, list)):
            self.points = [(float(a), float(b)) for a, b in ys]
        else:
            self.points = [(float(i), float(v)) for i, v in enumerate(ys)]
        self.label, self.mark, self.style = label, mark, style


def _series_code(s: "Series | list | tuple") -> str:
    if not isinstance(s, Series):
        s = Series(list(s))
    pts = "(" + ", ".join(f"({a:g}, {b:g})" for a, b in s.points) + ",)"
    label = f", label: [{s.label}]" if s.label else ""
    if s.style == "bars":
        labels = f", labels: ([{s.label}],)" if s.label else ""
        return f"    plot.add-bar({pts}, bar-width: 0.6{labels})\n"
    mark = f', mark: "{s.mark}"' if s.mark else ""
    line = ", style: (stroke: none)" if s.style == "scatter" else ""
    return f"    plot.add({pts}{mark}{line}{label})\n"


def _plot_source(body: str, xlabel: str, ylabel: str, title: str, legend: str | None, xmin, xmax, ymin, ymax,
                 size: tuple[float, float], grid: bool, extra: str = "") -> str:
    opts = f"size: ({size[0]:g}, {size[1]:g}), "
    if xlabel:
        opts += f"x-label: [{xlabel}], "
    if ylabel:
        opts += f"y-label: [{ylabel}], "
    if legend:
        opts += f'legend: "{legend}", '
    for name, v in (("x-min", xmin), ("x-max", xmax), ("y-min", ymin), ("y-max", ymax)):
        if v is not None:
            opts += f"{name}: {float(v):g}, "
    if grid:
        opts += "x-grid: true, y-grid: true, "
    opts += extra
    head = f"#align(center)[*{title}*]\n" if title else ""
    return (
        '#import "@preview/cetz:0.5.2": canvas, draw\n'
        '#import "@preview/cetz-plot:0.1.4": plot\n'
        f"{head}#canvas({{\n  plot.plot({opts}{{\n{body}  }})\n}})\n"
    )


def _typst_text(s: str) -> str:
    out = ""
    for c in str(s):
        if c in "\\#*_`$<>@[]~":
            out += "\\"
        out += c
    return out


def _rect(x: float, y: float, w: float, h: float) -> dict:
    return {"x": float(x), "y": float(y), "w": float(w), "h": float(h)}


def _image_size(path: Path) -> tuple[int, int] | tuple[None, None]:
    """Pixel size from a PNG, GIF, JPEG or WebP header."""
    try:
        b = path.read_bytes()
    except OSError:
        return None, None
    if b[:8] == b"\x89PNG\r\n\x1a\n":
        return int.from_bytes(b[16:20], "big"), int.from_bytes(b[20:24], "big")
    if b[:4] == b"GIF8":
        return int.from_bytes(b[6:8], "little"), int.from_bytes(b[8:10], "little")
    if b[:2] == b"\xff\xd8":
        i = 2
        while i + 9 < len(b):
            if b[i] != 0xFF:
                break
            marker = b[i + 1]
            if marker == 0xFF:
                i += 1
                continue
            length = int.from_bytes(b[i + 2:i + 4], "big")
            if 0xC0 <= marker <= 0xCF and marker not in (0xC4, 0xC8, 0xCC):
                return int.from_bytes(b[i + 7:i + 9], "big"), int.from_bytes(b[i + 5:i + 7], "big")
            i += 2 + length
    if b[:4] == b"RIFF" and b[8:12] == b"WEBP" and b[12:16] == b"VP8 ":
        return int.from_bytes(b[26:28], "little") & 0x3FFF, int.from_bytes(b[28:30], "little") & 0x3FFF
    return None, None


def new(title: str = "Untitled", width: float = 960, height: float = 540) -> Deck:
    """An empty deck (16:9 at 960×540 points by default)."""
    return Deck({"version": VERSION, "title": title, "width": float(width), "height": float(height), "slides": []})


def load(path: "str | os.PathLike") -> Deck:
    """A deck from its folder (or its ``deck.json``)."""
    p = Path(path)
    if p.is_file():
        p = p.parent
    data = json.loads((p / "deck.json").read_text(encoding="utf-8"))
    return Deck(data, p)


_current: Deck | None = None


def current() -> Deck:
    """The deck open in the editor, when the script is run from it.

    The editor passes the deck through the environment: ``DECKER_DECK``
    (the JSON file to read), ``DECKER_RESULT`` (where to write it back),
    ``DECKER_DIR``, ``DECKER_SLIDE`` and ``DECKER_SELECTION``. Outside the
    editor this raises ``RuntimeError``.
    """
    global _current
    if _current is not None:
        return _current
    src = os.environ.get("DECKER_DECK")
    if not src:
        raise RuntimeError("not run from the decker editor: use decker.load(path)")
    data = json.loads(Path(src).read_text(encoding="utf-8"))
    deck = Deck(data, os.environ.get("DECKER_DIR") or None)
    deck._result = os.environ.get("DECKER_RESULT")
    slide = os.environ.get("DECKER_SLIDE")
    deck._slide = int(slide) if slide else None
    sel = os.environ.get("DECKER_SELECTION", "")
    deck._selection = [int(i) for i in sel.split(",") if i.strip()] if sel else None
    _current = deck
    atexit.register(deck.commit)
    return deck
