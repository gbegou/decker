"""Writes docs/src/keys.md from the editor's help dialog (Help.svelte), so the
manual and the F1 list never disagree. `--check` fails if keys.md is stale."""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HELP = ROOT / "apps/deck-app/src/lib/Help.svelte"
OUT = ROOT / "docs/src/keys.md"


def render() -> str:
    src = HELP.read_text(encoding="utf-8")
    out = ["# Commands and keys", "",
           "The same list as the editor's help (**F1** or **?**). Generated from the editor's source by",
           "`docs/gen_keys.py`; do not edit by hand.", ""]
    for title, body in re.findall(r'\["(\w[^"]*)", \[\n(.*?)\n    \]\]', src, re.S):
        out += [f"## {title}", "", "| Key or control | Does |", "| --- | --- |"]
        for key, what in re.findall(r'\["((?:[^"\\]|\\.)*)", "((?:[^"\\]|\\.)*)"\]', body):
            cell = lambda s: s.replace("|", "\\|").replace("{{", "`{{").replace("}}", "}}`")
            out.append(f"| {cell(key)} | {cell(what)} |")
        out.append("")
    return "\n".join(out)


if __name__ == "__main__":
    text = render()
    if "--check" in sys.argv:
        if OUT.read_text(encoding="utf-8") != text:
            sys.exit("docs/src/keys.md is stale: run python3 docs/gen_keys.py")
    else:
        OUT.write_text(text, encoding="utf-8")
