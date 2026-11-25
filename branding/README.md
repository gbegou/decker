# Branding

The decker logo: a terrier (the Decker terrier) on a slide, with a D tile. `final_icon.py`
draws it; `install_icons.py` puts it in place.

| File | Use |
| --- | --- |
| `decker_icon.svg` | the detailed icon, 64 px and up |
| `decker_icon_small.svg` | the simplified icon, 16 to 32 px |
| `decker_lockup.svg`, `decker_lockup_dark.svg` | icon and wordmark, for light and dark backgrounds |

To regenerate and install (needs `assemble.py` next to `final_icon.py`, `rsvg-convert`,
numpy and Pillow, and the apps' `npm install` done):

```bash
python3 branding/final_icon.py
python3 branding/install_icons.py
```

The second step writes both apps' icon sets (window, taskbar, installers) and the
documentation's images and favicon.

`assemble.py` (the terrier drawing, the palette and the lettering) is not in the repository yet:
add it here to regenerate the logos.
