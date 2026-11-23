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

They land in `apps/deck-app/src-tauri/target/release/bundle/`.

## Opening a presentation

In the editor, **Open PPTX…** imports a PowerPoint file into a `.deck` folder next to it, and
**Open .deck folder…** opens a deck saved by decker. **Export PPTX…** writes a PowerPoint file.

Two environment variables help scripts and tests:

| Variable | Effect |
| --- | --- |
| `DECKER_OPEN=<file.pptx or folder.deck>` | Opens that deck at startup |
| `DECKER_PRESENT=1` | Starts the presentation at once; `both` also opens the presenter window on a single monitor |

## The editor

- The left column lists the slides; click one to edit it.
- On the canvas, click an element to select it, drag it to move it, drag the corner handle to
  resize it, press Delete to remove it.
- The right panel edits the selected element's position, size and text (one paragraph per
  line), and the slide's speaker notes.
- **+ Text**, **+ Slide** and **− Slide** do what they say. Page Up and Page Down change slide.
