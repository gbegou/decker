# Your first deck

Ten minutes from an empty window to a presented, exported talk. It assumes decker is
[installed](installation.md).

## 1. A new deck

Start decker and click **New**: one empty slide. Click **Save as…** and choose a folder name
such as `talk.deck`: a deck is a folder, with `deck.json` and an `assets/` folder for
pictures (see [the deck format](format.md)). From now on, **Save** writes it.

## 2. A layout

Layouts give slides their placeholders, background and footer, like PowerPoint's masters. In
the side panel, open **+ New layout** and pick **Title and content**: the layout editor
opens. Click **+ Text** and type `{{section}} — {{slide}} / {{slides}}` in the new box; drag it
to the bottom of the slide. Click **Done**. Back on the slide, choose the new layout in the
side panel's **Layout** selector. See [Layouts and themes](layouts.md) and
[Variables](variables.md).

## 3. Bullets and a formula

Right-click the slide's thumbnail and choose **New slide after**: the new slide starts from the layout, with a title and a body placeholder.
Double-click the title, type `Waves in a tank`, press Escape. Double-click the body and type:

```text
Waves travel at speed $c = sqrt(g h)$ in shallow water
Deep water: $omega^2 = g k$
```

Each line is a bullet. Typed between dollars, a formula is typeset as you leave it; Alt+=
types the dollars for you. To edit a formula again, move the caret into it: it shows as its
source. See [Text](text.md#formulas-in-text).

## 4. A picture and a figure

Drag an image file from your file manager onto the slide; it lands where you drop it. Drag
its corner to resize it; edges snap to the slide and to the other elements.

Click **+ Figure** for a plot drawn by Typst. The side panel shows its source; change the
function, and the figure follows. See [Formulas and figures](formulas.md).

## 5. Sections

Right-click the space below the thumbnails and choose **New section**; name it `Theory`.
The section starts with a new slide. Right-click the first thumbnail and choose **New plan
slide after**: a slide listing the sections, the current one highlighted, kept up to date. See
[Slides and sections](slides.md).

## 6. Build steps

Select the figure, and in the side panel's **Build steps** choose to make it appear on
click 1. The **◀ ▶** under the canvas show the slide at each click. See
[Build steps](steps.md).

## 7. Notes

With nothing selected, the side panel ends with the slide's **Notes**: what to say, shown in
the presenter view and, if you want, in the PDF handout.

## 8. Present

Click **Present**. With a projector plugged in, the slides go fullscreen there and the
presenter view (current and next slide, notes, timer) stays on your screen; with one screen,
two windows open. Space or a click goes forward, Backspace back, B blacks out, Esc ends. See
[Presenting with decker](presenting.md).

## 9. Share it

**Export PDF…** writes a PDF with bookmarks per section, selectable text and, if you want,
one page per click or a handout with notes. **Export PPTX…** writes a PowerPoint file for
colleagues who need one. See [Exporting](export.md).
