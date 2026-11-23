# Math

A formula is stored as source, in Typst or LaTeX syntax, either as a `math` element or as a
run with `"math": "typst"` or `"math": "latex"` inside a text box.

Until a math engine is embedded, the renderer draws the source in a monospace font and tags it
with `data-math`, so a front-end can replace it.

The plan is to embed [Typst](https://typst.app) as the text and math engine behind the
renderer: Typst math renders natively, and LaTeX math is translated to Typst by
[mitex](https://github.com/mitex-rs/mitex), which is also a Rust crate. Formulas then render
identically in the editor, on the projector and in the PDF export.

In PPTX export, formulas go out as their source text, with the syntax in the shape's
description so that a re-import recovers it. PowerPoint's own math markup (OMML) is not a
target.
