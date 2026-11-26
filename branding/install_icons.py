"""Installs the decker logos from branding/*.svg into the project.

Run after decker_logo.py has written the SVGs:

    python3 branding/decker_logo.py branding
    python3 branding/install_icons.py

It rasterizes with the Chromium of Playwright (npm), then:
  - generates the icon sets of both apps with `tauri icon` (window,
    taskbar, installers: .ico, .png sizes), with the simplified icon
    for the 16 and 32 px sizes;
  - copies the SVGs to docs/src/images/ and the favicon to docs/theme/;
  - puts the lockup at the top of the README and of the manual.
"""
import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
BRAND = ROOT / "branding"
APPS = [ROOT / "apps" / "deck-app", ROOT / "apps" / "deck-viewer"]
NEEDED = ["decker_icon.svg", "decker_icon_small.svg", "decker_lockup.svg", "decker_lockup_dark.svg"]


def rasterize(svg: pathlib.Path, png: pathlib.Path, size: int) -> None:
    """An SVG drawn at size x size on a transparent background."""
    script = f"""
const {{ chromium }} = require('playwright');
(async () => {{
  const b = await chromium.launch();
  const p = await b.newPage({{ viewport: {{ width: {size}, height: {size} }}, deviceScaleFactor: 1 }});
  await p.setContent(`<html><body style="margin:0;background:transparent">
    <img src="data:image/svg+xml;base64,{__import__('base64').b64encode(svg.read_bytes()).decode()}" width="{size}" height="{size}"></body></html>`);
  await p.waitForTimeout(200);
  await p.screenshot({{ path: {str(png)!r}, omitBackground: true, clip: {{ x: 0, y: 0, width: {size}, height: {size} }} }});
  await b.close();
}})();
"""
    subprocess.run(["node", "-e", script], check=True, cwd=APPS[0])


def main() -> int:
    missing = [n for n in NEEDED if not (BRAND / n).is_file()]
    if missing:
        print("missing in branding/: " + ", ".join(missing) + " (run decker_logo.py first)", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        big, small = tmp / "icon-1024.png", tmp / "icon-32.png"
        rasterize(BRAND / "decker_icon.svg", big, 1024)
        rasterize(BRAND / "decker_icon_small.svg", small, 32)
        for app in APPS:
            icons = app / "src-tauri" / "icons"
            subprocess.run(["npx", "tauri", "icon", str(big), "-o", str(icons)], check=True, cwd=app)
            # Tauri draws every size from the detailed icon: the 32 px one is the simplified drawing.
            shutil.copy(small, icons / "32x32.png")
            # Only the desktop sizes are used: drop the mobile and Store sets tauri icon adds.
            for extra in ["android", "ios"]:
                shutil.rmtree(icons / extra, ignore_errors=True)
            for f in icons.glob("Square*Logo.png"):
                f.unlink()
            f = icons / "StoreLogo.png"
            if f.exists():
                f.unlink()
            print(f"icons written to {icons.relative_to(ROOT)}")
    images = ROOT / "docs" / "src" / "images"
    for n in NEEDED:
        shutil.copy(BRAND / n, images / n)
    theme = ROOT / "docs" / "theme"
    theme.mkdir(exist_ok=True)
    shutil.copy(BRAND / "decker_icon_small.svg", theme / "favicon.svg")
    print("docs images and favicon written")
    # The lockup at the top of the README and of the manual, once.
    for page, src in [(ROOT / "README.md", "docs/src/images/decker_lockup.svg"), (ROOT / "docs" / "src" / "introduction.md", "images/decker_lockup.svg")]:
        text = page.read_text(encoding="utf-8")
        if "decker_lockup.svg" not in text:
            first, _, rest = text.partition("\n")
            page.write_text(f'{first}\n\n<img src="{src}" alt="decker" width="420">\n{rest}', encoding="utf-8")
            print(f"logo added to {page.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

# The editor's start screen: the dark lockup without its background, on the app's own.
_start = (ROOT / "branding" / "decker_lockup_dark.svg").read_text(encoding="utf-8").replace(
    '<rect width="870" height="240" fill="#222326"/>', "", 1)
(ROOT / "apps" / "deck-app" / "src" / "assets").mkdir(parents=True, exist_ok=True)
(ROOT / "apps" / "deck-app" / "src" / "assets" / "decker_lockup.svg").write_text(_start, encoding="utf-8")
