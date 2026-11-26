# Build steps

A slide can reveal itself click by click. An element has a base state, and at any click it
may change: appear, disappear, move, grow or shrink, rotate, change color. During the show,
each press of a key plays the next click; a slide consumes as many clicks as its highest step
before the next slide comes.

![Build steps](images/build-steps.svg)

| Effect | What the step sets at click N |
| --- | --- |
| Appear, fade in | opacity 1 (with opacity 0 at click 0) |
| Disappear | opacity 0 |
| Fly in | position, from off the slide to its place |
| Grow, shrink | size |
| Spin | rotation |
| Color change | fill |
| Move | position |

A group's steps apply to the group as a whole: its children fade, move, grow or spin
together, while keeping steps of their own.

## In the editor

![The Animations tab: the timeline, then the selected element's steps](images/screens/build-steps.png)

The **Animations** tab of the side panel holds the slide's timeline and the steps of the
selected element.

### The timeline

One row per click, after a **Start** row (what is hidden when the slide opens). Each row lists
what happens at that click, element by element, by [name](arranging.md#names): "Speed arrow
appears", "Tank moves". To reorder:

- drag an entry onto another click: it happens at that click, with the others there;
- drag it onto the thin line between two clicks, or under the last one: it gets a click of its
  own there, and the later clicks shift.

Clicks that end up empty are removed, so the clicks stay numbered 1, 2, 3… An entry's click
selects its element; a row's ▶ shows the slide at that click.

### Steps of an element

Select an element: the **Animations** tab lists its steps with everything they change, and
offers **Appear on click** and **Disappear on click**. The ◀ ▶ controls under the canvas show
the slide at a given click; viewing click 1 or later, dragging, resizing or rotating an
element sets where it is at that click, creating a step if there is none (**+ Move step** does
the same from the panel). Each step's visibility, position, size, rotation, fill, duration,
delay and easing are editable in place; **view** shows that click.

In `deck.json` each step has `at` (the click that triggers it), `set` (what changes),
`duration`, `delay` and `easing`.

## In exports

- **PDF** can carry one page per click (see [Exporting](export.md)).
- **PowerPoint** gets appear and disappear steps as fade entrances and exits, moves as motion
  paths, size changes as Grow/Shrink and turns as Spin; color changes are reported as
  warnings. Importing reads these effects back as steps.

Not covered, by design: curved motion paths (use intermediate steps), per-character text
effects, looping animations.
