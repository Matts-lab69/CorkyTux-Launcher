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

# Bundled brand logos (assets/*.png, full-color, theme-independent)

- `lutris.png`: Lutris desktop client icon,
  `lutris/lutris` @ `share/icons/hicolor/128x128/apps/net.lutris.Lutris.png`,
  GPL-3.0 (https://github.com/lutris/lutris/blob/master/LICENSE).
- `heroic.png`: Heroic Games Launcher icon,
  `Heroic-Games-Launcher/HeroicGamesLauncher` @ `public/icon.png` (downscaled
  1024 → 128px, no visual change at 24px display size),
  GPL-3.0 (https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/COPYING).
- umu-launcher: NO bundled logo. Its repo
  (`Open-Wine-Components/umu-launcher`, GPL-3.0) is a CLI tool with no
  artwork directory and no recognizable brand mark, so the row keeps the
  generic `system-run-symbolic` rather than risking an unofficial logo.
- Epic Games / GOG: monochrome SVGs from **Simple Icons**
  (`simple-icons/simple-icons` @ `icons/epicgames.svg` y `icons/gogdotcom.svg`,
  adaptadas al bundle como `epicgames-symbolic.svg` / `gogdotcom-symbolic.svg`
  con `fill` del theme, un solo path, sin recolor extra).
  - Fuente: https://github.com/simple-icons/simple-icons
  - Licencia del archivo: CC0 1.0
    (https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md).
    Logos de marca con copyright de sus dueños; se usan únicamente para
    indicar integración (mismo criterio que Steam/Lutris/Heroic), sin
    endorsement implícito.
- `steam.png`: official Steam ball icon, Wikimedia Commons
  `File:Steam icon logo.svg` (512px vector, server-rendered to 330px PNG,
  stored at 256px; verified full-color, saturation 0.62). Trademark of
  Valve Corporation — used solely to indicate Steam integration, as
  Lutris/Heroic do; no endorsement implied.
