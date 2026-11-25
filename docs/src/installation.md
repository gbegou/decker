# Installation

Installers are published with each release for Windows (per-user, no admin rights needed) and
Linux (`.deb` and AppImage), for decker and for [decker-viewer](viewer.md). Until the first
release, build from source.

## Building from source

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

They land in `target/release/bundle/` at the workspace root. The viewer builds the same way
from `apps/deck-viewer`.

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

Then, in a new terminal with the environment active, `npm install` and `npm run tauri dev` in
`apps/deck-app`. The first build compiles about 400 crates and takes 10 to 15 minutes.

The repository's `.cargo/config.toml` makes Rust link with the MinGW runtime it ships and
use conda's `x86_64-w64-mingw32-dlltool.exe` for import libraries, since conda's gcc only
compiles C files and the `dlltool.exe` rustup ships does not start on every machine. Tauri's
icon embedding looks for a resource compiler named `windres`; the app's build script provides
that name from conda's `x86_64-w64-mingw32-windres.exe`. Do not add rustup's
`bin\self-contained` folder to the PATH: its gcc stub shadows conda's compiler and cannot
compile C.

## Starting options

Environment variables help scripts and tests:

| Variable | Effect |
| --- | --- |
| `DECKER_OPEN=<file>` | Opens a `.pptx`, `.deckz` or `.deck` folder at startup (a PDF in the viewer) |
| `DECKER_PRESENT=1` | Starts the presentation at once; `both` also opens the presenter window on a single monitor |
| `DECKER_WINDOWED=1` | The audience window as a normal window instead of fullscreen, for debugging |
| `DECKER_PYTHON` | The Python interpreter for [macros](python.md) |
