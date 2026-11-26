# The editor at a glance

Before a deck is open, the window shows the logo with **New deck**, **Open file…** and
**Open .deck folder…**. With a deck, it has these areas.

![The editor, a figure selected](images/screens/editor.png)

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ ① File ▾  ② + Text  + Shape ▾  + Table  + Video…  + Figure    ↶ ↷ ?  Present │
├──────────┬──────────────────────────────────────────────┬──────────────────┤
│ ③      « │ ④ Text · … │ Arrange · …   (one row)        │ ⑤ Element Slide  │
│ thumb-   │ ┌──────────────────────────────────────────┐ │   Animations  ⟩  │
│ nails,   │ │                                          │ │                  │
│ sections │ │               the slide                  │ │ the selection,   │
│          │ │                                          │ │ the slide, or    │
│          │ └──────────────────────────────────────────┘ │ the timeline     │
│          │  All ◀ click 1 / 2 ▶                Notes ▾  │                  │
│          │ ⑥ speaker notes                              │                  │
├──────────┴──────────────────────────────────────────────┴──────────────────┤
│ status                                          ⑦ −  Fit  100%  +    97%   │
└────────────────────────────────────────────────────────────────────────────┘
```

1. **File** menu: new, open, save, insert slides from another deck, export (Ctrl+N,
   Ctrl+O, Ctrl+S, Ctrl+Shift+S). On the right: undo, redo, **?** for the list of
   [commands and keys](keys.md) (also **F1**), and **Present ▾** (from this slide, Shift+F5; from the first, F5).
   A dot after **File** marks unsaved changes; New, Open and closing the window then ask
   whether to save.
2. **Insert**: a text box (key T), a shape (key R for a rectangle; the menu also draws
   freehand and Bézier paths), a table, a video, a figure. **Slides | Design** switches to the
   [design view](layouts.md#the-design-view) of the themes and layouts, where the bar holds
   their commands (placeholders, delete, **Close design view**) and the left pane their tree.
3. **The thumbnail pane**: one thumbnail per slide under its section headers. Click to go,
   drag to reorder; right-click a thumbnail to add, duplicate or delete slides or give them a
   layout, or between thumbnails to add a slide, a plan slide or a section; see
   [Slides and sections](slides.md). Drag its edge to resize it; **«** collapses it.
4. **The canvas**: the current slide, edited directly. Above it, one row of tools for the
   selection: **Text** (font, size, color, horizontal and vertical alignment, bullets, fill and outline) and
   **Arrange** (align, distribute, mirror, order, stroke); for a text box or a shape, whose
   text tools take the row, the arrange tools open from their **Arrange ▾** button, so the row always fits. Right-click an element for cut,
   copy, paste, duplicate (Ctrl+D), delete, order, group, rename and a quick appear or
   disappear on the next click. Below it, **All ◀ ▶** switch between every element shown (the default,
   for editing) and the slide at each click of its [build steps](steps.md).
5. **The side panel**, in three tabs. **Element**: the selection's name, position and size
   and what belongs to its kind (crop for an image, playback for a video, cells for a table,
   source for a formula, points for a path). **Slide** (**Layout** or **Theme** while editing
   one): the slide's section, layout, fonts and colors and the macros. **Animations**: the
   slide's [timeline](steps.md#the-timeline) and the selected element's steps. Selecting an
   element shows its tab, deselecting the slide's. **⟩** collapses the panel to a strip of
   its tabs; it collapses by itself when the window is narrower than 1250 pixels.
6. **The notes**: the slide's speaker notes, under the slide; **Notes ▾** folds them away.
7. **Zoom**, in the status bar: **Fit** (the slide fits the window), **100%** (a point per
   pixel), **−** and **+** (also Ctrl+0, Ctrl+-, Ctrl++).

decker remembers the panel, the thumbnail pane and the notes as you left them.

## Selecting and editing

Click an element to select it, Shift+click to add to the selection, drag on an empty spot of
the slide to select everything the rubber band touches. Drag to move (edges and centers snap,
Alt moves freely), drag the corner handle to resize, the round handle above to rotate. See
[Arranging](arranging.md).

Double-click a text box, a shape or a table cell to type in it; Escape, or a click outside
it, ends editing. See
[Text](text.md).

The arrow keys move the selection by a point (ten with Shift), Ctrl+A selects everything,
Tab goes to the next element and Escape deselects.

Every change is one undo step (Ctrl+Z, Ctrl+Y); a run of typing in one box counts as one.
