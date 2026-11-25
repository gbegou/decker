# The viewer: presenting any PDF

`decker-viewer` is a second, small application: the audience window and the presenter view
of decker, for a PDF instead of a deck. Any PDF works: decker's own export, a Beamer talk, a
PowerPoint or Keynote export. It needs no Typst and no converter, so it installs in a few
megabytes, and it is the thing to put on a machine that only has to show slides.

## Using it

1. **Open PDF…** (or launch with `DECKER_OPEN=talk.pdf`). The pages appear in a strip on
   the left, the current page in the middle.
2. **Present** opens the audience window fullscreen on the second monitor and the presenter
   view on this one; with a single monitor, **Present + presenter** opens both as windows.
3. The keys are decker's ([Presenting](presenting.md)): arrows, space, Page Up and Down,
   Home and End, B and W for a black or white screen, Escape to end; P in the presenter view
   toggles the pointer, mirrored as a red dot on the audience window.

Pages are rendered by pdf.js inside the window, sharp at any size, so vector slides stay
vector. A page has no build steps: a Beamer overlay is a page of its own, as Beamer exports
it, and decker can export one page per build step.

## Notes

The presenter view shows the notes of the current page from a Markdown file next to the PDF,
`talk.notes.md` for `talk.pdf`, one section per page:

```markdown
# 1
Welcome, say who we are.
# 3
Skip the details of the proof if short on time.
```

The notes panel of the main window edits them and writes the file back. decker's PDF export
writes this file from the deck's notes, and its "one page per build step" option makes a PDF
that plays the builds here.

## Building it

```bash
cd apps/deck-viewer && npm install
npm run tauri dev        # or: npm run tauri build
```

The same toolchain as decker applies, including the [Windows without admin
rights](running.md#windows-without-admin-rights) setup. The Rust side (`apps/deck-viewer/src-tauri`)
is a few hundred lines over the shared `deck-present` crate; the windows are the shared
front-end of `apps/shared` ([Architecture](architecture.md)).
