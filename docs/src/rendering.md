# Rendering and Typst

How a deck becomes pixels, PDF pages and PowerPoint pictures.

## Slides as SVG

`deck_core::svg` renders a slide at a build step to one SVG document: the layout's static
elements (variables filled), then the slide's, each in a `<g id="eN">` carrying its opacity
and rotation. The editor's canvas, the thumbnails, the presentation windows and the PDF all
start from that SVG; videos are a `<foreignObject>` in the live render and their poster in the
still one (thumbnails, PDF).

In the audience window a new step patches the existing `<g>` elements rather than replacing
the document, so the browser engine transitions opacity and rotation in place, and a playing
video is left alone. Positions snap for now.

## Typst, embedded

The `deck-typst` crate embeds Typst as a library behind the `typst` Cargo feature (on by
default in the application). It typesets text boxes (line breaking, styles, bullets, inline
math), display formulas and figures, with a hermetic world: the bundled fonts (DejaVu Sans,
Libertinus, New Computer Modern), the deck's `fonts/` and `typst/` folders, and the CeTZ
packages vendored in `crates/deck-typst/packages/` (served to `@preview/...` imports by a
build script). Nothing is fetched from the network. Results are cached by source, so a slide
re-renders fast; a formula takes a few milliseconds, a cold CeTZ plot a few hundred.

Typst's output goes into the slide SVG as glyph outlines, so the screen and the PDF match
exactly. Embedding it costs about 30 MiB of binary and a few minutes of release build.

## Text editing

Text is edited where it is drawn. Double-clicking a text box asks Rust for the layout of every
character (`text_layout`: the box of each character as Typst placed it, read from the typeset
frame, plus the caret box after the end); the editor draws the caret and the selection from
those boxes and sends each key as an edit (`text_edit`: replace a range of the flat text by a
string). `deck_core::text` applies edits to paragraphs and runs by flat index, so styles
survive typing. Formulas, typed `$...$` included, map onto their whole box. Table cells work
the same way, the active cell chosen by `focus_cell`. Key actions are queued in the front-end
so that fast typing keeps its order, and view responses that arrive out of order are dropped.

## PDF pages

The PDF is a Typst document: per page, the slide's still SVG as an image, then layers that
make it searchable. Texts and tables are typeset again on top in transparent ink; formulas
too, scaled with Typst's own `measure` as the renderer does; figures go under the picture,
which hides their lines while their labels stay text. If a formula or figure cannot be typeset
in the layer, the export retries without those layers rather than failing. Headings with
`show heading: none` make the bookmark tree.

## PowerPoint pictures

Formulas and figures are rasterized by `typst-render` at three pixels per point for the PPTX
export, with their source in the picture's description (`typst:` or `typst-figure:`), which
the import reads back.
