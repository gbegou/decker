# Math

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
