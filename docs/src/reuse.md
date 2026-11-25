# Reusing work

## Copy and paste

Ctrl+C and Ctrl+V copy elements between slides, between layouts and slides, and between two
running copies of decker. Pictures and videos come along: shared within a deck, copied once
into the other deck's `assets/` across decks.

## Inserting slides from another deck

**Insert from deck…** asks for the `deck.json` of the other deck, shows its slides as
thumbnails, and inserts the ticked ones after the current slide. The current deck must have
been saved once, so that images have a folder to be copied into.

What is copied:

- the slides, with their notes and build steps;
- their images, into the destination's `assets/` under names that do not collide
  (`logo.png` becomes `logo-2.png` when taken);
- the layouts they use, with the layouts' images, unless the destination already has a
  layout of the same name, which is then used as is.

Not copied: the math preamble and the `typst/` files. When a copied formula needs a notation
from the other deck, merge those by hand.

From the command line:

```bash
deck-convert copy other.deck 3 talk.deck            # slide 3, at the end
deck-convert copy other.deck 2,5-7 talk.deck --at 1 # slides 2 and 5 to 7, before slide 1
deck-convert copy other.deck all talk.deck
```

## Reusing a layout

A layout saves to a `.deck_layout` folder and loads into any deck; see
[Layouts and themes](layouts.md#reusing-a-layout-deck_layout).

## Sending a deck as one file

A deck is a folder; a `.deckz` file is that folder zipped, to send it as one file. The editor
opens it (**Open file…**), works in a temporary folder and packs it again on every save;
**Save as…** with a `.deckz` name turns a folder deck into one, and with a folder name turns
it back.

```bash
deck-convert pack talk.deck          # writes talk.deckz
deck-convert unpack talk.deckz       # writes talk.deck/
```

The folder remains the format to keep under version control: `deck.json` diffs line by line,
an archive does not.
