# Importing PowerPoint

**Open file…** imports a `.pptx` into a `.deck` folder next to it, and opens it; a second
opening of the same file opens that folder. The command line does the same with
`deck-convert import talk.pptx`. After an import, the side panel lists what was skipped.

## What comes across

- Slide size and order, slide names, solid background colors, speaker notes, sections.
- Masters and layouts, as decker [layouts](layouts.md) with their placeholders and the
  master's text styles (size, bold, color, alignment, bullets); slide text takes what its runs
  leave unset from its placeholder, one size step down per bullet level. Placeholders without
  an explicit position inherit it from the layout, then the master. The master's theme fonts
  and colors become the layout's theme.
- Text: paragraphs with alignment and bullet level, runs with bold, italic, size, color and
  font.
- Pictures (PNG, JPEG, GIF, BMP, SVG, WebP), copied into `assets/`, with their crop, their
  crop-to-shape (ellipse, rounded rectangle) and their mirroring.
- Shapes: rectangles and rounded variants, ellipses, lines, connectors and arrows, with solid
  fill, outline and text; rotation. Other preset geometries are drawn as rectangles.
- Tables, with their grid, texts, cell fills and header row.
- Embedded videos, with their poster.
- Groups, with their child-to-slide transform.
- Animations: entrance and exit effects of the main sequence become build steps.
- Formulas and figures exported by decker come back editable.

## What does not

Skipped, with a warning: charts, SmartArt, embedded objects, gradients, colors given as theme
references, merged table cells and table styles, emphasis and motion-path animations,
transitions. The old binary `.ppt` format is not read; convert it first:

```bash
soffice --headless --convert-to pptx old.ppt
```
