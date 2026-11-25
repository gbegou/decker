# decker (Python)

The document model of [decker](https://github.com/gbegou/decker) decks, for scripts and
macros: build a deck from data, or change the one open in the editor. Pure Python, no
dependency.

```bash
pip install -e python        # from the repository; the import name is `decker`
```

```python
import decker
deck = decker.new("Hello")
deck.add_slide().add_text("Hello", 40, 30, 880, 80, size=40, bold=True)
deck.save("hello.deck")
```

Run a macro (a script using `decker.current()`) on a deck folder without the editor:

```bash
python -m decker run macro.py talk.deck
```

See the "Python macros and API" chapter of the manual for the full reference.
