# Layouts

A layout is decker's equivalent of PowerPoint's masters and layouts: what a family of slides
shares, and the slots a new slide starts from.

A layout has:

- a name, chosen by the user or taken from PowerPoint on import;
- a background color;
- static elements, drawn under every slide that uses the layout and not editable from a
  slide: a logo, a footer bar, rules;
- placeholders: named rectangles (`title`, `subtitle`, `body1`…) with a default text style
  (size, bold, color, font, alignment, bullets).

A slide names its layout. A text box may name the placeholder it fills; that is how the PPTX
export produces real placeholders and how a slide keeps its link to the layout.

```json
{
  "layouts": [
    {
      "name": "Corp",
      "background": "#FFFFFF",
      "elements": [
        { "type": "shape", "rect": { "x": 0, "y": 500, "w": 960, "h": 40 }, "fill": "#102030" }
      ],
      "placeholders": [
        { "name": "title", "rect": { "x": 40, "y": 30, "w": 880, "h": 80 }, "size": 40, "bold": true, "align": "center" },
        { "name": "body1", "rect": { "x": 40, "y": 130, "w": 880, "h": 340 }, "bullets": true }
      ]
    }
  ],
  "slides": [
    {
      "layout": "Corp",
      "elements": [
        { "type": "text", "placeholder": "title", "rect": { "x": 40, "y": 30, "w": 880, "h": 80 },
          "paragraphs": [{ "runs": [{ "text": "Hello", "bold": true, "size": 40 }], "align": "center" }] }
      ]
    }
  ]
}
```

## In the editor

- The right panel shows the slide's layout, with a selector to change it. Changing it keeps
  the slide's elements; placeholders are not re-created.
- **+ Slide** creates the next slide from the current slide's layout: one text box per
  placeholder, at the placeholder's position and style, showing the placeholder's name.
- **Duplicate** copies the current slide.

## Editing a layout

**Edit layout**, under the layout selector, shows the layout on the canvas instead of a
slide: its background, its static elements, editable like slide elements (drag, resize,
double-click to type), and its placeholders as dashed boxes with their name, which move and
resize the same way and are renamed in the panel (slides that fill the placeholder follow the
new name). **+ Text** and **+ Placeholder** add elements, **Delete layout** removes the layout
(its slides keep their elements), the name field renames it, **Done** returns to the slides. Selecting
a placeholder shows its default text style in the panel (size, bold, color, font, alignment,
bullets): new slides made from the layout start with it.

## Theme: color scheme and default fonts

A theme is the default font, text size, text color, an accent and a palette of swatches.
The deck has one, and every layout may have one: a layout's theme overrides the deck's field
by field for the slides that use the layout, so blank fields inherit.

```json
{
  "theme": { "font": "Libertinus Serif", "size": 20, "color": "#223344",
             "accent": "#AA0000", "palette": ["#AA0000", "#00AA00", "#0000AA"] },
  "layouts": [
    { "name": "Dark", "background": "#202020",
      "theme": { "color": "#FFFFFF", "accent": "#FFCC00" } }
  ]
}
```

- Text, formulas and placeholders that set no font, size or color of their own take the
  theme's. Changing a layout's font restyles every slide using it. Runs keep an explicit
  style wherever one was set; the style bar's ⌀ buttons return to the theme.
- The accent is the default highlight of the plan slide and the first swatch; the swatches
  appear as round buttons in the style bar.
- The **Theme** section of the right panel edits the deck's theme on a slide and the
  layout's theme in the layout editor, where blank fields show the deck's value.
- Fonts: the bundled families are DejaVu Sans (default), Libertinus Serif, New Computer
  Modern and DejaVu Sans Mono. Any `.ttf` or `.otf` file in the deck's `fonts/` folder is
  loaded with the deck and listed by its family name, so a corporate font travels with the
  deck and the PDF export embeds it. A missing family falls back to DejaVu Sans.
- PPTX: the deck theme is written to the PowerPoint theme (minor and major font, `dk1` text
  color, `accent1`–`accent6`); importing a PPTX gives each layout the theme of its master.
**+ New layout** creates an empty one and opens it.

## Reusing a layout: `.deck_layout`

**Save layout…** in the layout editor writes the layout as a folder:

```text
Corp.deck_layout/
  layout.json   # the layout: background, static elements, placeholders
  assets/       # its images
```

**Load layout…** (under the layout selector) reads such a folder into the current deck,
copying its images, under its name or a numbered one if the name is taken, and opens it for
editing. A folder of `.deck_layout` folders is a layout library. The deck must have been
saved once, so that images have a folder to be copied into.

## Variables

In a layout's texts, these are replaced on each slide that uses the layout:

| Variable | Value |
| --- | --- |
| `{{section}}`, `{{subsection}}` | The section and subsection the slide belongs to |
| `{{slide}}`, `{{slides}}` | The slide number and the number of slides |
| `{{title}}` | The slide's title (its name, else the first line of its first text box) |
| `{{deck}}` | The deck's title |
| `{{date}}` | Today's date, `YYYY-MM-DD` |

A footer such as `{{section}} › {{subsection}}` on the left and `{{slide}} / {{slides}}` on the
right therefore maintains itself. Unknown names are left as written. In PPTX export,
`{{slide}}` becomes PowerPoint's slide-number field and comes back as `{{slide}}` on import;
the other variables have no PowerPoint equivalent and are exported as their text.

## PPTX

**Import** turns each PowerPoint layout used by a slide into a decker layout: the master's
static shapes and background, then the layout's own static shapes and placeholders. Date,
footer and slide-number placeholders are skipped. Placeholder styles come from the layout's
list style, with PowerPoint's usual sizes (44 pt titles, 24 pt subtitles) when it sets none.
A blank layout is not kept. Slides keep their layout and their placeholder names.

**Export** writes one PowerPoint layout per decker layout, with its placeholders and static
shapes, plus a blank layout for slides without one. Text boxes that fill a placeholder are
exported as placeholders of that layout, so PowerPoint sees real masters: the deck opens
with its layouts in the layout gallery, and new slides made in PowerPoint can use them.

```bash
deck-convert layouts talk.deck
```

lists a deck's layouts and placeholders.
