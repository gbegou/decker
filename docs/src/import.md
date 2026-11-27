# Importing PowerPoint

**File ▸ Open file…** (Ctrl+O) imports a `.pptx` (or a `.pptm`: its macros are left out,
its slides import the same) into a `.deck` folder next to it, and opens it; a second
opening of the same file opens that folder. The command line does the same with
`deck-convert import talk.pptx`. After an import, the side panel lists what was skipped.

## What comes across

- Slide size and order, slide names, solid background colors, speaker notes, sections.
- Each slide master as a decker [theme](layouts.md) holding its layouts: what the layouts
  share (background, shapes, fonts and colors, placeholder styles) in the theme, their
  differences as overrides. Placeholders keep the master's text styles (size, bold, color, alignment, bullets) and their vertical alignment (anchor); slide text takes what its runs
  leave unset from its placeholder, one size step down per bullet level. Placeholders without
  an explicit position inherit it from the layout, then the master. The master's theme fonts
  and colors become the theme's fonts and colors.
- Text: paragraphs with alignment and bullet level, runs with bold, italic, size, color and
  font; the vertical alignment (anchor) of text boxes and shapes, a placeholder's coming from
  its layout or master when the slide does not set it.
- Pictures (PNG, JPEG, GIF, BMP, SVG, WebP), copied into `assets/`, with their crop, their
  crop-to-shape (ellipse, rounded rectangle) and their mirroring.
- Shapes: rectangles and rounded variants, ellipses, lines, connectors and arrows, with solid
  fill, outline and text; rotation. Other preset geometries are drawn as rectangles.
- Tables, with their grid, texts, cell fills, merged cells and header row. Their table style
  (banded rows and columns, header, total row, first and last column, inner borders) is
  baked into the cells' fills and text colors: styles defined in the file, and PowerPoint's
  default *Medium Style 2* family; other built-in styles are drawn as Medium Style 2, with a
  warning.
- Colors given as theme references ("Accent 1, darker 25%"), resolved in the slide's theme.
- Text that PowerPoint shrinks to fit its box ("shrink text on overflow"), at the size
  PowerPoint shows it.
- Embedded videos, with their poster; one started automatically plays when it appears.
- Groups, with their child-to-slide transform.
- Animations: entrance and exit effects of the main sequence become build steps, with
  their start (on click, with previous, after previous), delay and order; effects that
  PowerPoint starts with the slide play by themselves when the slide is shown; motion
  paths become moves (to the path's end point), Spin a turn, Grow/Shrink a size change.
- Formulas and figures exported by decker come back editable.

## What does not

Skipped, with a warning: charts, SmartArt, embedded objects, gradients, other emphasis effects (color, pulse...),
transitions. The old binary `.ppt` format is not read; convert it first:

```bash
soffice --headless --convert-to pptx old.ppt
```
