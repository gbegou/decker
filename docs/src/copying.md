# Copying slides between decks

Slides move between decks with their images and their layouts.

In the editor, **Insert from deck…** asks for the `deck.json` of the other deck, shows its
slides as thumbnails, and inserts the ticked ones after the current slide. The current deck
must have been saved once, so that images have a folder to be copied into.

On the command line:

```bash
deck-convert copy other.deck 3 talk.deck            # slide 3, at the end
```

```bash
deck-convert copy other.deck 2,5-7 talk.deck --at 1 # slides 2 and 5 to 7, before slide 1
```

```bash
deck-convert copy other.deck all talk.deck
```

What is copied:

- the slides, with their notes and build steps;
- their images, into the destination's `assets/` under names that do not collide
  (`logo.png` becomes `logo-2.png` when taken);
- the layouts they use, with the layouts' images, unless the destination already has a
  layout of the same name, which is then used as is.

Not copied: the math preamble and the `typst/` files. When a copied formula needs a notation
from the other deck, merge those by hand.
