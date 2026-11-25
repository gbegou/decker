# Formulas and figures

Formulas and figures are typeset by [Typst](https://typst.app), embedded in the application,
so they look the same in the editor, on the projector and in the PDF, with nothing to
install. For a formula inside a sentence, see [Text](text.md#formulas-in-text).

## Display formulas

A formula is a `math` element whose source is [Typst math](https://typst.app/docs/reference/math/),
without the surrounding `$ $`, edited in the side panel. The style bar sets its size and
color.

```json
{ "type": "math", "rect": { "x": 100, "y": 130, "w": 760, "h": 110 },
  "source": "integral_0^oo e^(-x^2) dif x = sqrt(pi) / 2", "size": 36, "color": "#1F4FA0" }
```

A formula larger than its box is scaled down to fit. When the engine reports an error, the
source is shown instead, in monospace.

## Your own notations

A deck carries its notations, so it renders identically on every machine:

- `math_preamble` in `deck.json` is Typst code prepended to every formula: `#let`
  definitions, `#import` statements.
- The `.typ` files in the deck folder's `typst/` directory are importable by name.

```text
talk.deck/
  deck.json          "math_preamble": "#import \"notation.typ\": *"
  typst/notation.typ #let vb(x) = math.bold(x)
```

The engine is hermetic: no file outside the deck folder and no package from the Typst
registry can be imported, except the bundled CeTZ packages below. To use another registry
package, copy its files into `typst/`.

## Figures: drawings and plots with CeTZ

A **figure** element (toolbar **+ Figure**) is a box of Typst markup: anything Typst can lay
out, including the [CeTZ](https://cetz-package.github.io/) drawing package and its plotting
library, bundled with decker as `@preview/cetz:0.5.2` and `@preview/cetz-plot:0.1.4` (with
`@preview/oxifmt:1.0.0`). A deck's own `typst/` files are importable as in formulas.

The page is as wide as the box and as tall as the content, scaled down to fit the box, so
`size: (12, 6)` in a plot sets its proportions in centimeters before scaling. Text takes the
theme's font and color. The source is edited in the side panel; a failing source shows the
first line of Typst's error in a dashed box.

```typst
#import "@preview/cetz:0.5.2": canvas, draw
#import "@preview/cetz-plot:0.1.4": plot
#canvas({
  plot.plot(size: (12, 6), x-label: [time (s)], y-label: $v$, legend: "inner-north-west", {
    plot.add(((0, 1), (1, 3), (2, 2), (3, 4)), mark: "o", label: [run 1])
    plot.add-bar(((0, 2), (1, 1)), bar-width: 0.5)
  })
})
```

Data can also sit in a CSV of the deck's `typst/` folder: `csv("data.csv")` reads it. From
Python, `slide.add_plot(...)` and `slide.add_bars(...)` write this source from lists or dicts
(see [Python](python.md)). A chart made elsewhere (matplotlib, for instance) is best added as
an SVG picture.

## In exports

- **PDF**: vector, and their text (symbols, axis labels, legends) is selectable.
- **PowerPoint**: a picture of the rendering, with the source kept so that importing the file
  brings the editable formula or figure back (see [Exporting](export.md)).

## Without Typst

Embedding Typst adds about 30 MiB to the application. A build without it wraps text by an
estimate and shows formula sources instead of formulas:

```bash
cargo build -p deck-app --no-default-features
```
