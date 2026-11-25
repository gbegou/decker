# Variables

In a [layout](layouts.md)'s texts, variables are replaced on each slide that uses the layout:

| Variable | Value |
| --- | --- |
| `{{section}}`, `{{subsection}}` | The section and subsection the slide belongs to |
| `{{slide}}`, `{{slides}}` | The slide number and the number of slides |
| `{{title}}` | The slide's title (its name, else the first line of its first text box) |
| `{{deck}}` | The deck's title |
| `{{date}}` | Today's date, `YYYY-MM-DD` |

Unknown names are left as written.

## Joining

A variable can be a list of variables and quoted texts, joined:

```text
{{section, " | ", subsection}}      Intro | Motivation, or just Intro without a subsection
{{slide, " / ", slides}}            3 / 12
{{"Part ", section}}                Part Intro, or nothing outside a section
```

Text between two variables shows only when both have a value, text before the first only
when it has one, and text after the last likewise, so a missing subsection leaves no dangling
separator. Inside quotes, `\"` is a quote and commas are kept.

## In PowerPoint

`{{slide}}` alone becomes PowerPoint's slide-number field and comes back as `{{slide}}` on
import; the other variables have no PowerPoint equivalent and are exported as the text they
show on each slide.
