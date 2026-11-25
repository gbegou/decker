# Installing and running

Installers are not published yet: build from source.

Requirements: Rust (stable), Node 22, and on Linux the WebKitGTK development packages:

```bash
sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
```

```bash
cd apps/deck-app && npm install
```

Run in development, with live reload of the interface:

```bash
npm run tauri dev
```

Build the installers (`.deb`, `.rpm` and AppImage on Linux, `.msi` and `.exe` on Windows):

```bash
npm run tauri build
```

They land in `target/release/bundle/` at the workspace root. The Installers workflow builds them
on every push and attaches them to the run as artifacts; the Windows one is built without
Visual Studio, the way a user without admin rights would (see `conda/environment-windows.yml`).

## Windows without admin rights

Everything installs into the user profile: Miniconda, then from the repository root

```powershell
conda env create -f conda/environment-windows.yml
```

```powershell
conda activate decker
```

Rust comes from rustup, with the GNU toolchain (the MSVC one needs Visual Studio, an
admin install). From the folder where `rustup-init.exe` was downloaded:

```powershell
.\rustup-init.exe --default-host x86_64-pc-windows-gnu --default-toolchain stable --profile minimal -y
```

The repository's `.cargo/config.toml` makes Rust link with the MinGW runtime it ships and
use conda's `x86_64-w64-mingw32-dlltool.exe` for import libraries, since conda's gcc only
compiles C files and the `dlltool.exe` rustup ships does not start on every machine. Tauri's
icon embedding looks for a resource compiler named `windres`; the app's build script provides
that name from conda's `x86_64-w64-mingw32-windres.exe`. Do not add rustup's `bin\self-contained` folder to
the PATH: its gcc stub shadows conda's compiler and cannot compile C.

Then, in a new terminal with the environment active, `npm install` and `npm run tauri dev` in
`apps/deck-app`. The first build compiles about 400 crates and takes 10 to 15 minutes.

## Opening a presentation

In the editor, **Open file…** opens a single-file deck (`.deckz`) or imports a PowerPoint file into a `.deck` folder next to it, and
**Open .deck folder…** opens a deck saved by decker. **Export PPTX…** writes a PowerPoint file.

Two environment variables help scripts and tests:

| Variable | Effect |
| --- | --- |
| `DECKER_OPEN=<file.pptx or folder.deck>` | Opens that deck at startup |
| `DECKER_PRESENT=1` | Starts the presentation at once; `both` also opens the presenter window on a single monitor |
| `DECKER_WINDOWED=1` | The audience window as a normal window instead of fullscreen, for debugging |

## The editor

- The left column lists the slides; click one to edit it, drag a thumbnail to reorder, or
  press Ctrl+Up and Ctrl+Down to move the current slide. Sections move with the slide that
  starts them.
- On the canvas, click an element to select it, drag it to move it, drag the corner handle to
  resize it, press Delete to remove it. Double-click a text box to edit its text in place, with a
  caret inside the typeset text: type, Backspace and Delete, arrows, Home and End, Shift with
  any of those for a selection, Ctrl+A, Enter for a new paragraph, a click to place the caret,
  Escape or a click outside to finish. Typing continues the style of the character before the
  caret. Formulas inside a text are skipped by the caret; edit them in the side panel.
- Shift+click selects several elements; **Group** (Ctrl+G) makes them one element that
  moves, resizes and animates together, **Ungroup** (Ctrl+Shift+G) takes it apart. PowerPoint
  groups import as groups, with their animations on the group.
- With an element selected, the style bar above the canvas sets bold, italic, size, color,
  font, alignment and bullet level (the selection while editing text, else the whole box;
  Ctrl+B and Ctrl+I too), the fill and outline of boxes and shapes, and the size and colors
  of formulas and of the plan.
- The right panel edits the selected element's position, size and text (one paragraph per
  line), and the slide's speaker notes.
- **?** in the toolbar, or F1, lists every command and key. For an image it offers the crop: a shape to clip to
  (rectangle, ellipse, rounded rectangle with its corner radius) and the percentage cut off
  each edge; the kept part fills the box.
- Images: paste one from the clipboard (Ctrl+V, a screenshot for instance) or drop image
  files from the file manager onto the canvas, where they land centered under the cursor
  at their pixel size, scaled down to fit half the slide (tiny icons are scaled up). The file is copied into the
  deck's `assets/` folder. An unsaved deck keeps it in a temporary folder until **Save as**
  moves everything to the chosen one. Pasting text while editing a box inserts it.
- Videos (`mp4`, `webm`, `mov`) and animated GIFs are added like images: dropped, pasted,
  or picked with **+ Video…**. The panel sets when they play (as soon as they appear, that is
  on entering the slide or at their appear step, or on the play button drawn over them) and
  whether they loop or play once and stop on the last frame; videos can also have their
  sound on. They play in the audience window; the editor and the presenter's previews show
  the first frame. Thumbnails, PDF and PowerPoint exports show a poster with a play glyph: a
  GIF's first frame, or for a video the frame chosen with **Use as poster** in the panel
  (pick the time in seconds).
- Ctrl+Z undoes any edit of the deck and Ctrl+Y (or Ctrl+Shift+Z) redoes it, also from the
  ↶ ↷ toolbar buttons; a run of typing in one box counts as one step. Opening a deck
  clears the history; saving does not.
- Ctrl+C and Ctrl+X copy or cut the selected elements, Ctrl+V pastes them, slightly
  offset, on the current slide or layout, in this deck or in another running copy of decker.
  Within a deck the pasted image elements share the file in `assets/`; across decks the
  file is copied once. Resizing, cropping or masking never changes the file: those are
  properties of the element, so two copies of one picture can be cropped differently.
  Saving removes from `assets/` the files no element uses any more.
- **+ Shape** adds a rectangle, an ellipse, a line or an arrow in the theme's accent color;
  the style bar sets its fill and outline.
- **+ Table** adds a table with a header row. Double-click a cell to type in it, with the
  caret, selection and styles of any text; Tab and Shift+Tab move between cells. Its panel
  also shows the cells as a grid of fields
  (one line per paragraph), adds or removes rows and columns around the cell that has the
  focus, toggles the header, sets a cell's fill and the relative column widths. Rows are as
  tall as their line count asks; the table's box sets the overall size, the style bar its
  text size, the arrange bar its rules. Tables go to PowerPoint and back as real tables, and
  their text is selectable in the PDF.
- Dragging on the empty slide draws a rubber band that selects every element it touches.
  The round handle above a selected element rotates it (Shift for steps of 15°; it catches
  at right angles; double-click to reset). Viewing a later click, the rotation becomes a
  build step, like a move.
- Dragging moves the whole selection. Its edges and center snap to the slide's edges and
  center and to those of the other elements, with a dashed guide where it snapped; hold Alt
  to move freely.
- The **arrange bar**, under the style bar, acts on the selection: align left, center, right,
  top, middle or bottom (to the selection's bounds, or to the slide when one element is
  selected), distribute three or more with equal gaps, mirror left to right or top to bottom
  (lines and arrows turn, pictures are drawn mirrored, groups mirror their content, text
  stays readable), bring forward or send back in the drawing order, and set the stroke color
  and width of shapes and text boxes.
- While editing a text, the mouse works as in any editor: click places the caret, dragging
  selects, a double click selects the word and a triple click the paragraph.
- **+ Text**, **+ Slide**, **Duplicate** and **− Slide** do what they say; a new slide starts
  from the current slide's [layout](layouts.md). **Insert from deck…** copies slides from
  another deck. Page Up and Page Down change slide.
