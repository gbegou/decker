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

Editing a layout itself (its static elements and placeholders) is done in `deck.json` for
now; a layout editor is on the roadmap.

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
