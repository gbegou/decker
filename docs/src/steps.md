# Build steps and animations

Animations are not effects attached to elements; they are **states per step**. An element has
a base state, and for any step it may carry overrides: position, size, rotation, opacity,
fill. Rendering step N applies the overrides in force at step N and draws a static picture.
Playing step N interpolates from the state at N − 1 over the step's duration.

![Build steps](images/build-steps.svg)

| Effect | Override at step N |
| --- | --- |
| Appear, fade in | `opacity` 1 (with `opacity` 0 at step 0) |
| Disappear | `opacity` 0 |
| Fly in from the left | `x` from off-slide to its place |
| Grow, shrink | `w`, `h` |
| Spin | `rotation` |
| Color change | `fill` |
| Move | `x`, `y` |

A group's steps apply to the group as a whole: its children fade, move, grow or spin
together, while keeping steps of their own.

## In the editor

Select an element: the **Build steps** panel lists its steps with everything they change, and
offers **Appear on click** and **Disappear on click**. The ◀ ▶ controls under the canvas show
the slide at a given click; viewing click 1 or later, dragging or resizing an element sets
where it is at that click, creating a move step if there is none (**+ Move step** does the
same from the panel). Each step's visibility, position, size, rotation, fill, duration,
delay and easing are editable in place; **view** shows that click.

Each step has `at` (the click that triggers it), `set` (the overrides), `duration`, `delay`
and `easing`. A slide consumes as many clicks as its highest step before moving on.

In the audience window, the renderer tags every element with a stable id, and a new step
patches the existing elements rather than replacing the document, so the browser engine
transitions opacity and rotation in place. Positions snap for now.

Not covered, by design: curved motion paths (use intermediate steps, or a future `path`
override), per-character text effects, looping animations.
