# Bundled icon attribution

The `CorkyTux/symbolic/*.svg` files in this directory are copies of icons
from the **Adwaita icon theme** (GNOME), used as a bundled fallback so the
launcher does not depend on the user's system icon theme being installed.

- Source: Adwaita (https://gitlab.gnome.org/GNOME/adwaita-icon-theme)
- License: CC BY-SA 3.0 (https://creativecommons.org/licenses/by-sa/3.0/)
- Exception: `emblem-ok-symbolic.svg` is a copy of Adwaita's
  `object-select-symbolic.svg` renamed, because Adwaita ships no
  `emblem-ok-symbolic` (only full-color PNGs exist elsewhere).

Lookup order at runtime: bundled `CorkyTux` theme first (prepended search
path, symbolic recoloring preserved), system icon theme second.
