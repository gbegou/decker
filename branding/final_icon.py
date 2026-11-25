"""DECKER app icon, final layout: slide background, bullets top-left, D tile bottom-right.

Builds:
  decker_icon.svg        detailed icon (64 px and up)
  decker_icon_small.svg  simplified icon (16-32 px)
  decker_lockup.svg / decker_lockup_dark.svg  icon + wordmark
"""
import os
import re
import subprocess
import numpy as np
from PIL import Image
from assemble import (DOG, INK, CREAM, TAN, RUST, SLATE, text_path, centered_letter, svg)

BLACK_D = re.search(r'id="black"[^>]*d="([^"]+)"', DOG).group(1)

# ---------------------------------------------------------------- dog fixes (local coords)
# 1. eye: almond with sclera, brown iris, black pupil, highlight
ALMOND = "M712 553 Q748 521 799 534 Q819 547 814 570 Q768 592 712 553 Z"
EYE = (f'<path d="M684 556 Q745 508 802 524 Q834 545 824 582 Q752 622 684 556 Z" fill="#222326"/>'
       f'<clipPath id="almond"><path d="{ALMOND}"/></clipPath>'
       f'<path d="{ALMOND}" fill="{CREAM}"/>'
       f'<g clip-path="url(#almond)"><circle cx="777" cy="556" r="29" fill="#5A3114"/>'
       f'<circle cx="779" cy="557" r="16" fill="#111214"/></g>'
       f'<circle cx="766" cy="545" r="6" fill="{CREAM}"/>'
       f'<path d="{ALMOND}" fill="none" stroke="#111214" stroke-width="5"/>')
# 4. neck stripe filled with cream (clipped to the silhouette)
NECK = (f'<clipPath id="silc"><path d="{BLACK_D}"/></clipPath>'
        f'<path d="M545 786 L742 786 L822 812 L822 1240 L545 1240 Z" fill="{CREAM}" clip-path="url(#silc)"/>')


def dog(k, tx, ty, clip, outline_px=7, neck_fix=True, eye=True):
    """Terrier with cream sticker outline (outline_px in final pixels)."""
    sw = 2 * outline_px / k
    ring = (f'<path d="{BLACK_D}" fill="none" stroke="{CREAM}" stroke-width="{sw:.1f}" '
            f'stroke-linejoin="round"/>') if outline_px else ""
    inner = DOG.replace('<g id="terrier">', '<g>')
    seam = (f'<path d="{BLACK_D}" fill="none" stroke="{CREAM}" stroke-width="{2.6 / k:.1f}" '
            f'stroke-linejoin="round"/>') if outline_px else ""
    extra = (NECK if neck_fix else "") + (EYE if eye else "")
    return (f'<g clip-path="url(#{clip})"><g transform="translate({tx} {ty}) scale({k})">'
            f'{ring}{inner}{extra}{seam}</g></g>')


# ---------------------------------------------------------------- slide
def slide(x, y, w, h, cid, bullets=2, deck=2):
    r = h * 0.12
    u = h / 20
    out = []
    cols = [RUST, TAN][2 - deck:]
    for i, col in enumerate(cols):
        d = (len(cols) - i) * 1.1 * u
        out.append(f'<rect x="{x + d:.1f}" y="{y - d:.1f}" width="{w}" height="{h}" rx="{r:.1f}" fill="{col}"/>')
    out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r:.1f}" fill="{SLATE}"/>')
    bx = x + 1.6 * u
    for j, frac in enumerate([0.15, 0.11][:bullets]):
        yy = y + (2.0 + 2.1 * j) * u
        out.append(f'<circle cx="{bx + 0.45 * u:.1f}" cy="{yy + 0.5 * u:.1f}" r="{0.55 * u:.1f}" fill="{TAN}"/>')
        out.append(f'<rect x="{bx + 1.6 * u:.1f}" y="{yy:.1f}" width="{w * frac:.1f}" height="{1.0 * u:.1f}" rx="{0.5 * u:.1f}" fill="{CREAM}" opacity="0.9"/>')
    clip = (f'<clipPath id="{cid}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r:.1f}"/>'
            f'<rect x="{x - 200}" y="{y - 400}" width="{w + 400}" height="{400 + h * 0.3:.1f}"/></clipPath>')
    return "".join(out), clip


def tile(x, y, size, stroke):
    d = centered_letter("D", x + size / 2, y + size / 2, size * 0.58)
    return (f'<rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{size * 0.18:.1f}" '
            f'fill="{CREAM}" stroke="{INK}" stroke-width="{stroke}"/><path d="{d}" fill="{RUST}"/>')


def raw_icon(small=False):
    if small:
        frame, clip = slide(24, 150, 440, 330, "icl", bullets=0, deck=1)
        return (f'<defs>{clip}</defs>' + frame
                + dog(0.40, 8, 18, "icl", outline_px=12, eye=False)
                + tile(286, 300, 196, 14))
    frame, clip = slide(24, 150, 440, 330, "icl")
    return (f'<defs>{clip}</defs>' + frame
            + dog(0.40, 8, 18, "icl", outline_px=7)
            + tile(320, 352, 154, 9))


def fit(body, margin=22, size=512):
    """Measure the drawn bbox and centre it in the square with even margins."""
    tmp = "/tmp/_fit.svg"
    open(tmp, "w").write(svg(size, size, f'<g>{body}</g>').replace(
        f'viewBox="0 0 {size} {size}"', f'viewBox="-200 -200 {size + 400} {size + 400}"').replace(
        f'width="{size}" height="{size}"', f'width="{size + 400}" height="{size + 400}"'))
    subprocess.run(["rsvg-convert", tmp, "-o", "/tmp/_fit.png"], check=True)
    a = np.array(Image.open("/tmp/_fit.png"))[:, :, 3]
    ys, xs = np.where(a > 8)
    x0, x1, y0, y1 = xs.min() - 200, xs.max() - 200, ys.min() - 200, ys.max() - 200
    s = (size - 2 * margin) / max(x1 - x0, y1 - y0)
    tx = (size - (x1 - x0) * s) / 2 - x0 * s
    ty = (size - (y1 - y0) * s) / 2 - y0 * s
    return f'<g transform="translate({tx:.2f} {ty:.2f}) scale({s:.4f})">{body}</g>'


TAG = "Efficient Cross-platform Keynote Editor and Renderer"


def lockup(icon_body, dark=False):
    fg = CREAM if dark else INK
    tag = "#B7C0CC" if dark else "#5B6170"
    d_txt, _ = text_path("DECKER", 0, 0, 104, tracking=0.03)
    t_txt, _ = text_path(TAG, 0, 0, 19.5, tracking=0.01)
    body = (f'<g transform="translate(14 14) scale(0.414)">{icon_body}</g>'
            f'<g transform="translate(264 150)"><path d="{d_txt}" fill="{fg}"/></g>'
            f'<g transform="translate(268 192)"><path d="{t_txt}" fill="{tag}"/></g>')
    return svg(870, 240, body, bg=INK if dark else None)


if __name__ == "__main__":
    # Written next to this script (branding/); install_icons.py takes them from there.
    out = os.path.dirname(os.path.abspath(__file__)) + "/"
    big = fit(raw_icon())
    small = fit(raw_icon(small=True), margin=12)
    files = {
        "decker_icon.svg": svg(512, 512, big),
        "decker_icon_small.svg": svg(512, 512, small),
        "decker_lockup.svg": lockup(big),
        "decker_lockup_dark.svg": lockup(big, dark=True),
    }
    for n, c in files.items():
        open(out + n, "w").write(c)
    print("ok")
