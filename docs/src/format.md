# The deck format

A deck is a folder:

```text
talk.deck/
  deck.json     # the model, with a "version" field for migrations
  assets/       # images, referenced by deck-relative path (unused ones removed on save)
  typst/        # optional: .typ files formulas can import
  fonts/        # optional: .ttf/.otf files usable by name
```

Coordinates are in points (1/72 inch), origin at the top-left of the slide; a 16:9 slide is
960 × 540 points.

A `.deckz` file is the same folder zipped; see [Reusing work](reuse.md#sending-a-deck-as-one-file).

A minimal deck:

```json
{
  "version": 1,
  "title": "Example",
  "width": 960,
  "height": 540,
  "slides": [
    {
      "notes": "Say hello.",
      "elements": [
        {
          "type": "text",
          "rect": { "x": 50, "y": 40, "w": 860, "h": 80 },
          "paragraphs": [
            { "runs": [{ "text": "Hello", "bold": true, "size": 44 }], "align": "center" }
          ]
        },
        {
          "type": "image",
          "rect": { "x": 600, "y": 300, "w": 200, "h": 150 },
          "src": "assets/image1.png",
          "crop": { "left": 0.1, "bottom": 0.25 },
          "mask": { "kind": "ellipse" }
        },
        {
          "type": "shape",
          "rect": { "x": 50, "y": 300, "w": 200, "h": 100 },
          "kind": "ellipse",
          "fill": "#0080FF",
          "steps": [{ "at": 0, "set": { "opacity": 0 } }, { "at": 1, "set": { "opacity": 1 }, "duration": 0.3 }]
        },
        {
          "type": "math",
          "rect": { "x": 300, "y": 420, "w": 360, "h": 80 },
          "source": "E = m c^2"
        }
      ]
    }
  ]
}
```

An image's optional `crop` gives the fraction of the source removed from each edge (the kept
part fills the rect), and its `mask` the shape it is clipped to: `rectangle`, `ellipse` or
`rounded_rectangle` with a `radius` as a fraction of the shorter side. Both round-trip to
PowerPoint's crop and "crop to shape".

Element types are `text`, `image`, `media` (a video or animated GIF: `src`, optional
`poster`, `play` = `on_appear` or `button`, `loop`, `sound`, `duration` in seconds for a
GIF), `figure` (Typst markup with the bundled CeTZ packages, `source`), `table` (`rows` of
cells with `paragraphs` and an optional `fill`, relative `columns` widths, `header`, `stroke`
for the rules, `size`), `shape`
(`rectangle`, `ellipse`, `line`, `arrow`; `flip` turns a line or arrow to run from the
bottom-left to the top-right, `reverse` puts an arrow's head at its start), `math` (Typst
math source), `outline` (the plan: sections with the slide's own highlighted) and `group`
(`children` in slide coordinates, `rect` their bounding box; steps on the group animate all of
them). A run may carry `"math": true` to be rendered as an inline formula.
Every element may carry `steps`; see [Build steps](steps.md). A rotation from the start is
a step at click 0 that sets `rotation` (degrees clockwise); PowerPoint's frame rotation maps
to it both ways. The deck may carry a
`math_preamble` and a `typst/` folder of importable files; see [Formulas and figures](formulas.md), a `theme`
(the deck's text style: default font, size, colors and palette), `themes` (what a family of
layouts shares: background, elements, text style, placeholder styles) and `layouts` that
slides name, each keeping only what it changes of its theme; see
[Themes and layouts](layouts.md#in-deckjson). The format version is 2; decks of version 1
are converted when opened. Variables in theme and layout texts are listed in
[Variables](variables.md).

The authoritative definition is the `Deck` type in `deck-core`, documented in the
[API reference](api.md).
