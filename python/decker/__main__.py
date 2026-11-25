"""Command line of the decker Python package.

``python -m decker run macro.py talk.deck [--slide N] [--dry-run]`` runs a
macro, a script written for the editor with ``decker.current()``, on a
deck folder: the deck it hands back is saved to ``deck.json``.

``python -m decker info talk.deck`` prints a summary.
"""

from __future__ import annotations

import argparse
import json
import os
import runpy
import sys
import tempfile
from pathlib import Path

import decker


def run_macro(script: Path, deck_dir: Path, slide: int = 0, dry_run: bool = False) -> bool:
    """Runs ``script`` against the deck in ``deck_dir``; returns whether the deck changed."""
    deck = decker.load(deck_dir)
    before = json.dumps(deck.data, sort_keys=True)
    with tempfile.TemporaryDirectory() as tmp:
        src, result = Path(tmp) / "deck.json", Path(tmp) / "result.json"
        src.write_text(json.dumps(deck.data), encoding="utf-8")
        os.environ.update(
            DECKER_DECK=str(src), DECKER_RESULT=str(result), DECKER_DIR=str(deck.dir), DECKER_SLIDE=str(slide), DECKER_SELECTION=""
        )
        decker._current = None
        sys.path.insert(0, str(script.parent))
        try:
            runpy.run_path(str(script), run_name="__main__")
        finally:
            sys.path.pop(0)
        current = decker._current
        if current is not None:
            current.commit()
        if not result.exists():
            return False
        data = json.loads(result.read_text(encoding="utf-8"))["deck"]
    if json.dumps(data, sort_keys=True) == before:
        return False
    if not dry_run:
        decker.Deck(data, deck.dir).save()
    return True


def main(argv: "list[str] | None" = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m decker", description="Script decker decks.")
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="run a macro on a deck folder and save the result")
    run.add_argument("script", type=Path)
    run.add_argument("deck", type=Path)
    run.add_argument("--slide", type=int, default=1, help="the slide the macro sees as current (1-based)")
    run.add_argument("--dry-run", action="store_true", help="do not write deck.json")
    info = sub.add_parser("info", help="print a summary of a deck folder")
    info.add_argument("deck", type=Path)
    args = parser.parse_args(argv)
    if args.command == "info":
        deck = decker.load(args.deck)
        print(f"{deck.title} ({deck.width:g} x {deck.height:g} pt, {len(deck)} slides)")
        for s in deck:
            kinds = ", ".join(e.kind for e in s.elements) or "empty"
            print(f"  {s.index + 1}. {kinds}" + (f"  [{s.section}]" if s.section else ""))
        return 0
    changed = run_macro(args.script, args.deck, max(args.slide - 1, 0), args.dry_run)
    print("deck unchanged" if not changed else ("deck would change (dry run)" if args.dry_run else f"saved {args.deck}"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
