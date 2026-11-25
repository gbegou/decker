# Python macros and API

decker scripts are plain Python. They use the `decker` package, a small pure-Python
library that reads and writes the [native format](format.md) with helpers for the common
operations. Nothing is linked into the application: the editor runs scripts with the
Python it finds on the machine, and without Python everything else works.

## Standalone: generating a deck

```python
import decker

deck = decker.new("Quarterly review")
deck.theme["font"] = "Libertinus Serif"
deck.layouts.append({"name": "Corp", "placeholders": [
    {"name": "title", "rect": {"x": 40, "y": 30, "w": 880, "h": 80}, "size": 40, "bold": True}]})

for region, figure in [("North", "n.png"), ("South", "s.png")]:
    s = deck.add_slide(layout="Corp", section=region)   # placeholders become text boxes
    s.elements[0].text = f"{region}: results"
    s.add_text("Revenue up\nCosts flat", 40, 130, 400, 300, size=24, bullets=True).appear(1)
    s.add_math("Delta = sum_(i) x_i", 40, 450, 400, 60, size=24)
deck.save("review.deck")                                  # creates the folder
img = deck.slides[0].add_image(figure, 500, 130)          # copied into assets/
deck.save()
```

Install the package with `pip install -e python` from the repository (it is published as
`decker-slides`, imported as `decker`), or point `PYTHONPATH` at the repository's `python/`
folder, or at the `python/` folder of an installed decker's resources.

A macro written for the editor also runs on a deck folder from the command line, the
result saved to its `deck.json`:

```bash
python -m decker run macro.py talk.deck --slide 3    # --dry-run to try without saving
python -m decker info talk.deck
```

The model is thin: `deck.data`, `slide.data` and `element.data` are the JSON dicts, so
anything the helpers do not cover can be set directly, following the format chapter.
`decker.load("talk.deck")` reads a deck, `deck.save()` writes it back.

## Charts from data

`add_plot` and `add_bars` write a [CeTZ figure](math.md#figures-drawings-and-plots-with-cetz)
from Python values; the result stays an editable Typst source on the slide.

```python
s = deck.add_slide()
s.add_plot([1, 3, 2, 4],                                   # y values, x = 0, 1, 2…
           decker.Series([4, 5, 6], xs=[0, 1.5, 3], label="measured", mark="x", style="scatter"),
           x=60, y=100, w=600, h=350, xlabel="time (s)", ylabel="$v$", title="Speed", ymin=0, grid=True)
s.add_bars(["Q1", "Q2", "Q3"], {"2024": [3, 4, 5], "2025": [4, 5, 6]},
           x=60, y=100, w=600, h=350, ylabel="revenue")
s.add_figure("#import \"@preview/cetz:0.5.2\": canvas, draw\n#canvas({ draw.circle((0, 0)) })", 700, 100, 200, 200)
```

A `Series` takes `ys` with optional `xs` (or a list of `(x, y)` pairs), a legend `label`, a
`mark` (`o`, `x`, `+`, `*`, `None`) and a `style` (`line`, `scatter`, `bars`). Labels and
titles accept Typst markup. For anything else, write the Typst source yourself with
`add_figure`, or produce an SVG with matplotlib and `add_image` it.

## Macros: scripts run from the editor

The **Macros** section of the right panel lists the `.py` files of the deck's `scripts/`
folder, each with a run button, and **Run script…** runs any file. The script gets the
open deck:

```python
import decker

deck = decker.current()            # the deck open in the editor
slide = deck.slide                 # the slide shown
for e in slide.selected:           # the selected elements
    e.rect.move(0, 20)
    e.style(color="#AA0000")
print(f"moved {len(slide.selected)} elements")   # shown under the buttons
```

What the script leaves in `deck` replaces the editor's deck when the script ends, as a
single undo step (`deck.commit()` does it earlier, `deck.discard()` cancels). A script
that raises leaves the deck untouched and its traceback is shown. Files the script adds
with `add_image` or `add_video` go into the deck's `assets/`, so save the deck once before
writing macros that add pictures.

The editor finds Python as `python3` or `python` on the path, or in `DECKER_PYTHON`. From
a conda prompt, the environment's interpreter is used. The package travels with the
application; `DECKER_PYTHONPATH` overrides where it is looked for.

## Reference

| Object | Members |
|---|---|
| `decker` | `new(title, width, height)`, `load(path)`, `current()` |
| `Deck` | `title`, `width`, `height`, `slides`, `slide`, `layouts`, `theme`, `add_slide(layout=, at=, section=, subsection=)`, `layout(name)`, `add_asset(path)`, `save(path=)`, `commit()`, `discard()` |
| `Slide` | `index`, `elements`, `selected`, `layout`, `section`, `subsection`, `notes`, `background`, `add_text(...)`, `add_math(...)`, `add_shape(kind, ...)`, `add_image(path, x, y, w=, h=)`, `add_video(path, ..., play=, loop=, sound=)`, `add_figure(source, ...)`, `add_plot(*series, ...)`, `add_bars(labels, values, ...)`, `add_outline(...)`, `add(dict)`, `duplicate()`, `remove()` |
| `Series` | `Series(ys, xs=None, label=, mark=, style=)` |
| `Element` | `kind`, `rect` (`x`, `y`, `w`, `h`, `move(dx, dy)`), `text`, `style(bold=, italic=, size=, color=, font=)`, `steps`, `appear(at)`, `disappear(at)`, `move_to(at, x=, y=, w=, h=)`, `remove()`, `data` |

Sizes are in points on a 960 × 540 slide; colors are `"#RRGGBB"`; `align` is `left`,
`center`, `right` or `justify`.
