# Troubleshooting

## Starting

**Linux: a blank or gray window.** Some GPU drivers break WebKitGTK's compositing. Start
with `WEBKIT_DISABLE_COMPOSITING_MODE=1 decker`, or `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

**Windows: "WebView2 is missing".** decker draws its interface with Microsoft Edge
WebView2, part of Windows 11 and of current Windows 10. Install the *Evergreen Runtime* from
Microsoft; its per-user installer needs no administrator rights.

**The old icon in the taskbar after an update.** Windows caches icons; it refreshes after
signing out, or after unpinning and pinning decker again.

## Editing

**Typed text stays invisible until another slide is shown.** Fixed in November 2026: update
decker. (After each key, the caret briefly pointed past the end of the text; drawing it
failed, which stopped the canvas from updating. It showed most when typing into an empty
box.)

**A figure shows as a dashed red box.** Typst could not compile it; the box shows the
message. **A formula shows its source** instead of the result: the same, for formulas. In text, LaTeX's `\alpha` works but `\frac{a}{b}` does not: write `frac(a, b)` or
`a/b`. See [Formulas and figures](formulas.md).

**A price shows as a formula.** Two `$` in one paragraph make a formula; write `\$` for a
literal dollar.

**A font is missing.** decker sees the system fonts plus those in the deck's `fonts/` folder.
Put the `.ttf` or `.otf` there so the deck carries it to other machines.

**Text editing in place says it needs the Typst feature.** That build was made without
Typst (`--no-default-features`); install a release build.

## Importing and exporting

**An imported PowerPoint looks different.** Read the import warnings in the side panel;
[Importing PowerPoint](import.md) lists what is kept and what is not (gradients, charts,
SmartArt).

**The PDF export fails on a formula.** The export retries without selectable formulas and
figures rather than failing; if it still fails, look for a slide whose figure shows as an error
box.

**A video does not play in the presentation.** Use MP4 (H.264) or WebM; on Linux, playback
needs the GStreamer plugins (`gst-plugins-good`, `gst-libav`).

## Reporting a problem

Open an issue on the project's GitHub page with the decker version, the system, and if you can,
the deck (or a `.deckz` of it, see [Reusing work](reuse.md#sending-a-deck-as-one-file)).
