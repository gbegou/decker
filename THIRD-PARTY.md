# Third-party notices

decker is licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`).
The application bundles the following works under their own licenses.

## Fonts

| Font | License | Where |
| --- | --- | --- |
| DejaVu Sans (Regular, Bold, Oblique, Bold Oblique) | Bitstream Vera Fonts license, with the DejaVu additions in the public domain | `crates/deck-typst/fonts/`, license in `crates/deck-typst/fonts/LICENSE-DejaVu` |
| Libertinus Serif | SIL Open Font License 1.1 | bundled by the `typst-assets` crate |
| New Computer Modern, New Computer Modern Math | GNU General Public License v3 with the font exception | bundled by the `typst-assets` crate |
| DejaVu Sans Mono | Bitstream Vera Fonts license | bundled by the `typst-assets` crate |

The font exception of the GPL and the OFL allow embedding these fonts in documents and
distributing them with the application without affecting the license of decker itself.

## Typst packages

| Package | License | Where |
| --- | --- | --- |
| CeTZ 0.5.2 (with its `cetz_core.wasm` plugin) | LGPL-3.0-or-later | `crates/deck-typst/packages/preview/cetz/` |
| cetz-plot 0.1.4 | LGPL-3.0-or-later | `crates/deck-typst/packages/preview/cetz-plot/` |
| oxifmt 1.0.0 | MIT OR Apache-2.0 | `crates/deck-typst/packages/preview/oxifmt/` |

The packages are embedded unmodified, as the Typst source files the registry publishes, with
their license files, and are interpreted at run time by the embedded Typst engine. They can
be replaced by other versions of the same files; nothing of decker is derived from them, so
the LGPL applies to the packages alone.

## Code

Rust dependencies are listed in `Cargo.lock` and JavaScript dependencies in
`apps/deck-app/package-lock.json`; `cargo deny check licenses` (configuration in `deny.toml`)
verifies that every Rust dependency is under a license in the allowed list.
