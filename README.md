# decker

A free, fast, modern alternative to PowerPoint, written in Rust, for Linux and Windows.

Planned layout:

| Crate | Content |
| --- | --- |
| `crates/deck-core` | Document model, native format (`<name>.deck/` folder: `deck.json` + `assets/`), SVG renderer |
| `crates/deck-convert` | Command-line converter: PPTX import, SVG render, PPTX export |
| `apps/deck-app` | Tauri application: editor, presentation window, presenter mode, embedded Python macros (later) |
