# decker

**D**ecker is an **E**fficient, **C**ross-platform **K**eynote **E**ditor and **R**enderer.

A free, fast, modern alternative to PowerPoint, written in Rust, for Linux and Windows.

- **Reads and writes PPTX** through a converter that imports what can be expressed and says
  what it skipped, so decks from colleagues open and go back to them.
- **Presents** on a projector with a presenter view: current slide, what comes next, notes,
  timer, black screen.
- **Layouts**, the equivalent of masters, imported from and exported to PPTX; slides copy
  between decks with their images and layouts.
- **Sections** become PDF bookmarks; **PDF export** keeps text selectable and formulas crisp.
- **Build steps** reveal, move, grow or recolor elements click by click, with one mechanism.
- **Text and formulas** typeset by Typst embedded: exact line breaking, inline math, the
  deck's own notations, identical on screen and in PDF.
- **Python macros** run from the editor against the open deck, and a Python API builds decks from data; see [Python](python.md).

Status: first draft. The document model, the native format, an SVG renderer, the PPTX
converter and the application with its three windows exist. See the [roadmap](roadmap.md).

![The stack](images/stack.svg)
- **A viewer**: the audience and presenter windows alone, for any PDF; see [The viewer](viewer.md).
