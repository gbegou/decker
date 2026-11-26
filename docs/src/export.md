# Exporting

## PDF

**File ▸ Export PDF…** asks how the pages are made:

- **One page per slide**, as each slide looks at its last build step: for reading and
  printing.
- **One page per build step**: a slide with three clicks becomes four pages, so the PDF plays
  the builds in [decker-viewer](viewer.md) or any PDF reader.
- **Handout**: one A4 portrait page per slide, the slide on top and its speaker notes below,
  with the section and the slide number in the header.

The PDF has a bookmark tree: sections, subsections under them, and every slide by its title
(its name, else the first line of its first text box), pointing at the slide's first page.
With the first two modes, the speaker notes are written next to the PDF as `name.notes.md`,
one `# page` section per page, which is the file the viewer's presenter view reads.

Everything is vector. Text can be selected, searched and read by a screen reader: each slide
is drawn as artwork, and its texts are typeset once more on top, invisibly, at the same place;
formulas and figure labels too. Fonts are embedded, so the output does not depend on the
machine. Videos appear as their poster.

From the command line (built with the `typst` feature):

```bash
deck-convert pdf talk.deck              # talk.pdf, one page per slide
deck-convert pdf talk.deck --steps      # one page per build step
deck-convert pdf talk.deck --notes      # the handout
```

## PowerPoint

**File ▸ Export PowerPoint…** writes a package with one master, one PowerPoint layout per decker
[layout](layouts.md) plus a blank one, the fonts and colors of the deck's first theme (fonts, text color,
accents) and
the speaker notes. On the slides:

- text boxes, bound to their layout placeholders when they fill one, with their vertical
  alignment (PowerPoint's anchor), as for the text of shapes;
- pictures with their crop, crop-to-shape and mirroring; shapes and arrows; tables as real
  tables; groups; rotation;
- formulas and figures as pictures of decker's rendering (PNG at three pixels per point,
  fitted and centered in the element's box), with the Typst source kept in the picture's
  description so that importing the file brings them back editable;
- videos embedded the way PowerPoint writes them, with their poster, looping and muted as
  set; a video that plays when it appears starts by itself in PowerPoint ("Automatically"),
  the others on click. Animated GIFs as pictures, which PowerPoint
  animates;
- build steps: appear and disappear as fade entrances and exits, moves as motion paths,
  size changes as Grow/Shrink, turns as Spin; sections.

What cannot be expressed is listed as warnings after the export: color changes in build
steps, plan slides (exported as their current text). Without the Typst engine, formulas and figures go out as their source text.
