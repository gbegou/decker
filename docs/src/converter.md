# The converter command line

`deck-convert` does without the editor what the application does: convert, render, export,
copy. The application calls the same code.

```bash
cargo build --release -p deck-convert --features typst
```

| Command | Does |
| --- | --- |
| `deck-convert import talk.pptx [-o talk.deck]` | PowerPoint to a deck folder (see [Importing](import.md)) |
| `deck-convert export talk.deck [-o talk.pptx]` | A deck to PowerPoint (see [Exporting](export.md)) |
| `deck-convert pdf talk.deck [-o talk.pdf] [--steps \| --notes]` | A deck to PDF, per slide, per build step or as a handout |
| `deck-convert render talk.deck [-o dir]` | Each slide to an SVG file (`talk.deck/svg/` by default) |
| `deck-convert info talk.deck` | A summary: size, slides, sections |
| `deck-convert layouts talk.deck` | The layouts and their placeholders |
| `deck-convert copy other.deck 2,5-7 talk.deck [--at N]` | Slides from one deck into another (see [Reusing work](reuse.md)) |
| `deck-convert pack talk.deck` / `unpack talk.deckz` | Between a deck folder and a single `.deckz` file |

Built without the `typst` feature, `pdf` is not available and exports write formula sources
instead of pictures. Python macros run on a deck folder with `python -m decker run`; see
[Python](python.md).
