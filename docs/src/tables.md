# Tables

**+ Table** adds a three by three table with a header row on the theme's accent.

- **Typing**: double-click a cell to type in it, with the caret, selection and styles of any
  text. Tab and Shift+Tab move to the next and previous cell, wrapping over rows.
- **The panel** shows the cells as a grid of fields (one line per paragraph), adds or removes
  rows and columns around the cell that has the focus, toggles the header row, sets the
  focused cell's fill, and takes the relative column widths (`2 1 1` for a first column twice
  as wide).
- **Merged cells**: in the panel, **Merge →** joins the focused cell with the next one to
  its right, **Merge ↓** with the one below (repeat to span more), and **Split** undoes it.
  The texts of the joined cells are gathered in the merged one. Adding or removing a row or
  column through a merged cell splits it.
- **Sizes**: rows are as tall as their line count asks; the table's box sets the overall
  size, the style bar its text size, the [arrange bar](arranging.md) its rules (color and
  width).

Tables go to PowerPoint and back as real tables (grid, texts, fills, header row, merged cells), their text
is selectable in the PDF, and Python builds them from rows of values (see [Python](python.md)).
