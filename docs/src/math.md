# Text and math

Text boxes are typeset by Typst, embedded in the application: line breaking at the box width,
bold, italic, size, color and font per run, alignment, bullets by level, and inline formulas
(`"math": true` on a run) in the flow of the sentence. The editor, the projector, the
thumbnails and the PDF therefore show the same lines. Without the `typst` feature, text is
wrapped by an estimate. Text defaults to the bundled DejaVu Sans at 18 pt.


### Formulas inside text

Typing `$...$` in any text box makes an inline formula: `the angle $alpha$ is small`.
Typst names symbols without a backslash, but LaTeX's habit works too: `$\alpha + 2\pi$`
reads as `$alpha + 2 pi$`. A lone `$` (a price) and `\$` stay text. Other LaTeX commands such
as `\frac{a}{b}` are not translated: write `a/b` or `frac(a, b)`.

## Formulas

Formulas are written in [Typst math](https://typst.app/docs/reference/math/) and rendered by
Typst itself, embedded in the application as a library (the `deck-typst` crate, behind the
`typst` Cargo feature, on by default). A formula is a `math` element whose `source` is the
math content, without the surrounding `$ $`:

```json
{ "type": "math", "rect": { "x": 100, "y": 130, "w": 760, "h": 110 },
  "source": "integral_0^oo e^(-x^2) dif x = sqrt(pi) / 2", "size": 36, "color": "#1F4FA0" }
```

The result is glyph outlines in SVG, so it looks the same in the editor, on the projector and
in the PDF export, with no font installed. A formula larger than its box is scaled down to
fit. When the engine reports an error, the source is shown instead, in monospace.

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
registry (`@preview/...`) can be imported. To use a registry package, copy its files into
`typst/`.

## Export

In PPTX export, formulas go out as their source text, with `typst:` and the source in the
shape's description so that a re-import recovers it. PowerPoint's own math markup (OMML)
is not a target.

## Cost

Embedding Typst adds about 30 MiB to the application binary (its fonts included) and a few
minutes to a release build. A formula renders in a few milliseconds. A build without it:

```bash
cargo build -p deck-app --no-default-features
```

## Figures: drawings and plots with CeTZ

A **figure** element (toolbar **+ Figure**) is a box of Typst markup typeset by the same
engine: anything Typst can lay out, including the [CeTZ](https://cetz-package.github.io/)
drawing package and its plotting library, which are bundled with decker as
`@preview/cetz:0.5.2` and `@preview/cetz-plot:0.1.4` (with `@preview/oxifmt:1.0.0`). No
network access is needed and no other package can be fetched; a deck's own `typst/` files
are importable as in formulas.

The page is as wide as the box and as tall as the content, scaled down to fit the box, so
`size: (12, 6)` in a plot sets its proportions in centimeters before scaling. Text takes the
deck or layout theme's font and color. The source is edited in the right panel; a failing
source shows the first line of Typst's error in a dashed box.

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
Python, `slide.add_plot(...)` and `slide.add_bars(...)` write this source from lists or
dicts; see [Python](python.md). A chart made elsewhere (matplotlib, for instance) is still
best added as an SVG picture.

CeTZ and cetz-plot are LGPL-3.0-or-later; they ship unmodified as Typst source and can be
replaced by newer copies in `crates/deck-typst/packages/preview/`, which keeps decker's own
MIT OR Apache-2.0 license unaffected (see `THIRD-PARTY.md`).
