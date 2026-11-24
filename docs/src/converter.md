# The converter

`deck-convert` is the command-line converter; the application calls the same code.

```bash
cargo build --release
```

```bash
target/release/deck-convert import talk.pptx          # → talk.deck/
```

```bash
target/release/deck-convert export talk.deck -o out.pptx
```

```bash
target/release/deck-convert render talk.deck          # → talk.deck/svg/slideN.svg
```

```bash
target/release/deck-convert info talk.deck
```

```bash
target/release/deck-convert pdf talk.deck             # with --features typst
```

## What the PPTX import handles

- Slide size and order, slide names, solid background colors, speaker notes.
- Text: paragraphs with alignment and bullet level, runs with bold, italic, size, color and
  font.
- Placeholders without an explicit position inherit it from the slide layout, then the
  master. Body placeholders are bulleted, titles get 44 pt and subtitles 24 pt when the run
  sets no size (the master's text styles are not read yet).
- Pictures (png, jpeg, gif, bmp, svg, webp), copied into `assets/`, with their crop and
  their crop-to-shape (ellipse, rounded rectangle).
- Shapes: rectangles and rounded variants, ellipses, lines and connectors, with solid fill,
  outline and text. Other preset geometries are drawn as rectangles.
- Groups, with their child-to-slide transform.
- Animations: entrance and exit effects of the main sequence become build steps, one click
  per step.
- Masters and layouts, as decker [layouts](layouts.md), with their placeholders.
- PowerPoint sections, as the `section` of the slide that starts each.

Skipped, with a warning: tables, charts, SmartArt, embedded objects, gradients, theme
colors, emphasis and motion-path animations, transitions. The old binary `.ppt` format is not
read; convert it first:

```bash
soffice --headless --convert-to pptx old.ppt
```

## Export

Writes a package with one master, one PowerPoint layout per decker layout (placeholders and
static shapes) plus a blank one, a theme and notes; slide elements as explicit shapes, text
boxes bound to their layout placeholders, build steps as fade entrances and exits, and
formulas as their source text. What cannot be expressed is reported: geometry or color changes in a step, formulas.
