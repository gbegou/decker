# The editor at a glance

The editor is one window in five areas.

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ ① New  Open file…  Open .deck folder…  Save  Save as…  Export…   ↶ ↷ ?    │
│ ② + Text + Shape + Table + Video… + Figure + Plan  + Slide …   Present     │
├──────────┬──────────────────────────────────────────────┬──────────────────┤
│ ③        │ ④ style bar · arrange bar                    │ ⑤                │
│ thumb-   │ ┌──────────────────────────────────────────┐ │ properties of    │
│ nails,   │ │                                          │ │ the selection,   │
│ sections │ │               the slide                  │ │ build steps,     │
│          │ │                                          │ │ section, layout, │
│          │ └──────────────────────────────────────────┘ │ theme, macros,   │
│          │            ◀  click 0 / 2  ▶                 │ notes            │
└──────────┴──────────────────────────────────────────────┴──────────────────┘
```

1. **The deck bar**: open, save and export the deck; undo and redo; **?** opens the list of
   [commands and keys](keys.md) (also **F1**).
2. **The insert bar**: add elements to the slide, add or remove slides, start the
   presentation. While a layout is being edited, it holds the layout's own commands
   (placeholders, save and load, **Done**).
3. **The thumbnail pane**: one thumbnail per slide under its section headers. Click to go,
   drag to reorder, right-click for the slide menu; see [Slides and sections](slides.md).
4. **The canvas**: the current slide, edited directly. Above it, the **style bar** (font,
   size, color, alignment, bullets, fill and outline) and the **arrange bar** (align,
   distribute, mirror, order, stroke) act on the selection; below it, **◀ ▶** show the slide
   at a given click of its [build steps](steps.md).
5. **The side panel**: without a selection, the slide's section, layout and theme, the
   macros and the speaker notes; with one, its position and size and what belongs to its kind
   (crop for an image, playback for a video, cells for a table, source for a formula) and its
   build steps.

## Selecting and editing

Click an element to select it, Shift+click to add to the selection, drag on an empty spot of
the slide to select everything the rubber band touches. Drag to move (edges and centers snap,
Alt moves freely), drag the corner handle to resize, the round handle above to rotate. See
[Arranging](arranging.md).

Double-click a text box, a shape or a table cell to type in it; Escape ends editing. See
[Text](text.md).

Every change is one undo step (Ctrl+Z, Ctrl+Y); a run of typing in one box counts as one.
