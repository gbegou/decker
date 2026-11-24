# The native format

A deck is a folder:

```text
talk.deck/
  deck.json     # the model, with a "version" field for migrations
  assets/       # images, referenced by deck-relative path
```

Coordinates are in points (1/72 inch), origin at the top-left of the slide; a 16:9 slide is
960 × 540 points. A single-file (zipped) variant is planned: the same layout, archived.

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

Element types are `text`, `image`, `shape` (`rectangle`, `ellipse`, `line`) and `math`
(Typst math source). A run may carry `"math": true` to be rendered as an inline formula.
Every element may carry `steps`; see [Build steps](steps.md). The deck may carry a
`math_preamble` and a `typst/` folder of importable files; see [Math](math.md), and
`layouts` that slides name; see [Layouts](layouts.md).

The authoritative definition is the `Deck` type in `deck-core`, documented in the
[API reference](api.md).
