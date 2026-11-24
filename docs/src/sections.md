# Sections and PDF export

A slide can start a **section** and a **subsection**: two names on the slide, set in the
editor's slide panel or in `deck.json` (`"section": "Method"`, `"subsection": "Model"`).
The outline follows the slide order, so moving slides keeps it consistent.

Sections are used in three places:

- **PDF bookmarks.** **Export PDF…** writes one page per slide at its final build step, and
  a bookmark tree: sections, subsections under them, and every slide by its title (its name,
  else the first line of its first text box). Text and formulas are glyph outlines (selectable text is on the roadmap).
- **PowerPoint sections.** Import maps PowerPoint's sections onto the slide that starts each;
  export writes them back. PowerPoint has no subsections, so those stay decker-only.
- **The presenter view**, which shows the current section and subsection in its header.

The PDF is produced by the embedded Typst engine, so it needs the `typst` feature (on by
default in the application). On the command line:

```bash
cargo build -p deck-convert --features typst
```

```bash
target/debug/deck-convert pdf talk.deck            # → talk.pdf
```

Slide text uses the DejaVu Sans family bundled with the engine, so the output does not
depend on the fonts installed on the machine.
