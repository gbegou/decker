# Roadmap

1. Typst as the text and math engine behind the renderer (LaTeX math through mitex); until
   then formulas are drawn as their source and text is wrapped by an estimate.
2. Embedded Python (PyO3) exposing the document model as a macro API.
3. Editor: styles (font, size, color, alignment), shapes and images, undo, step editing.
4. PDF export from the SVG renderer; single-file `.deck` zip.
5. Master text styles, theme colors and tables in the PPTX import; motion paths as steps.
6. Installers published from CI for Linux and Windows.
