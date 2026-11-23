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

Two details the repository takes care of: `.cargo/config.toml` makes Rust link with the
MinGW runtime it ships, since conda's gcc only compiles C files; and `dlltool.exe`, which a
Windows dependency needs, is in rustup's toolchain, so add its folder to the PATH once (user
PATH, no admin):

```powershell
$env:PATH = "$HOME\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained;$env:PATH"
```

Then, in a new terminal with the environment active, `npm install` and `npm run tauri dev` in
`apps/deck-app`. The first build compiles about 400 crates and takes 10 to 15 minutes.

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
- **+ Text**, **+ Slide**, **Duplicate** and **− Slide** do what they say; a new slide starts
  from the current slide's [layout](layouts.md). **Insert from deck…** copies slides from
  another deck. Page Up and Page Down change slide.
