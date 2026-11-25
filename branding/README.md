# Branding

The decker logo: a terrier (the Decker terrier) in front of a slide, with a D tile.
`decker_logo.py` draws it, with no dependency: the traced dog and the Montserrat ExtraBold
outlines of the lettering are embedded (Montserrat is under the SIL Open Font License, which
allows this). `install_icons.py` puts it in place.

| File | Use |
| --- | --- |
| `decker_icon.svg` | the detailed icon, 64 px and up |
| `decker_icon_small.svg` | the simplified icon, 16 to 32 px |
| `decker_lockup.svg`, `decker_lockup_dark.svg` | icon and wordmark, for light and dark backgrounds |

To regenerate and install (the apps' `npm install` done):

```bash
python3 branding/decker_logo.py branding
python3 branding/install_icons.py
```

The second step writes both apps' icon sets (window, taskbar, installers), the
documentation's images and favicon, and the lockup at the top of the README and the manual.
