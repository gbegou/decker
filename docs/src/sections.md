# Sections and PDF export

A slide can start a **section** and a **subsection**: two names on the slide. The slide stack
on the left shows them as headers above the slide that starts them: hover a thumbnail and
click **+ section** or **+ subsection** to start one there, click a header to rename it, clear
the name to remove it. The slide panel has the same two fields, and `deck.json` stores them
(`"section": "Method"`, `"subsection": "Model"`). The outline follows the slide order, so
moving slides keeps it consistent.

## The plan slide

**+ Plan** adds an outline element to the current slide: the list of sections, numbered, the
section the slide belongs to in bold and the others in gray, with the subsections of the
current section under it (the current subsection in bold). The highlight follows the slide's
position in the deck, so build the plan slide once, **Duplicate** it and drag a copy to the
start of each section: every copy marks its own section, and reordering slides keeps them
right. Its size and colors are in `deck.json` (`size`, `color`, `highlight`, `subsections`).
In PPTX export each copy becomes a plain text box with the text it shows.

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

## PDF export options

**Export PDF…** asks how the pages are made:

- **One page per slide**, as each slide looks at its last build step: for reading and
  printing.
- **One page per build step**: a slide with three clicks becomes four pages, so the PDF plays
  the builds in [decker-viewer](viewer.md) or in any PDF reader. Bookmarks point at the first
  page of each slide.

- **Handout**: one A4 portrait page per slide, the slide on top and its speaker notes below,
  with the section and the slide number in the header (`deck-convert pdf talk.deck --notes`).

With the first two, the speaker notes are written next to the PDF as `name.notes.md`, one `# page`
section per page (a slide's notes repeat on each of its step pages), which is the file the
viewer's presenter view reads. The converter does the same: `deck-convert pdf talk.deck
--steps`.

Text in the PDF can be selected, searched and read by a screen reader: each slide is drawn as
vector artwork, and its texts are typeset once more on top, invisibly, at the same place.
Formulas are typeset once more over the picture, transparent, and figures under it, so
their text (axis labels, legends) is in the PDF as well.
