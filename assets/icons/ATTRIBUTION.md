# Bundled icon attribution

The `CorkyTux/symbolic/*.svg` files in this directory are copies of icons
from the **Adwaita icon theme** (GNOME), used as a bundled fallback so the
launcher does not depend on the user's system icon theme being installed.

- Source: Adwaita (https://gitlab.gnome.org/GNOME/adwaita-icon-theme)
- License: CC BY-SA 3.0 (https://creativecommons.org/licenses/by-sa/3.0/)
- Exception: `emblem-ok-symbolic.svg` is a copy of Adwaita's
  `object-select-symbolic.svg` renamed, because Adwaita ships no
  `emblem-ok-symbolic` (only full-color PNGs exist elsewhere).
- Exception: `corkytux-system-software-install-symbolic.svg` uses the trace
  of the modern `system-software-install-symbolic.svg` glyph from the
  **Mint-Breeze** icon theme: the original Adwaita-legacy copy is a lock
  glyph that mismatched the store button; normalized to the bundle's plain
  `fill #2e3436` ink convention. Mint icon themes are GPL-3.0-or-later;
  this copy is bundled for local display only.
- Exception: `corkytux-warning-symbolic.svg` / `corkytux-error-symbolic.svg`
  are copies of Adwaita's `dialog-warning-symbolic.svg` /
  `dialog-error-symbolic.svg` renamed (same ink glyph, no redraw); used as
  the header Warnings entry points and tinted at runtime with libadwaita's
  `.warning` / `.error` utility classes so the colour always matches the
  active theme.

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
- `corkytux-steamdb.png` / `corkytux-steamdb_dark.png` /
  `corkytux-protondb.png`: SteamDB and ProtonDB logos from **Simple Icons**
  (`simple-icons/simple-icons` @ `icons/steamdb.svg` y `icons/protondb.svg`),
  rasterized to 128px PNG:
    - SteamDB hex `#000000`: the light theme uses the brand black
      (`corkytux-steamdb_dark.png`); the dark theme needs a contrast variant
      (`corkytux-steamdb.png`, white `#FFFFFF`) because the brand black
      disappears on dark backgrounds. No other recoloring — no theme-driven
      tinting beyond this contrast swap.
    - ProtonDB hex `#F50057`: single asset (`corkytux-protondb.png`).
  - Fuente: https://github.com/simple-icons/simple-icons
  - Campo `license` en los datos de Simple Icons (`_data/simple-icons.json`,
    dump jsDelivr del 24-sep-2026): `steamdb` → `null`, `protondb` → `null`.
    `license: null` significa que aplica la licencia por defecto del proyecto,
    CC0 1.0
    (https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md),
    que es la que se declara aquí para el arte vectorial.
    SteamDB and ProtonDB are trademarks of their respective owners; the
    logos are used solely to indicate the integration (same criterion as
    Steam/Lutris/Heroic entries), without implying endorsement.

# Bundled emulator icons (Papirus)

The files in `emulators/` are unmodified copies of the 48 px application
icons from **Papirus icon theme**:

- Source: https://github.com/PapirusDevelopmentTeam/papirus-icon-theme
- Commit: `bf539287ef5dc18529424a02cccee76175920a6f`
- Upstream directory: `Papirus/48x48/apps/`
- License: GPL-3.0-only (the repository `LICENSE`)
  (https://github.com/PapirusDevelopmentTeam/papirus-icon-theme/blob/bf539287ef5dc18529424a02cccee76175920a6f/LICENSE)

Bundled files: `cemu.svg`, `desmume.svg`, `dolphin-emu.svg`, `duckstation.svg`,
`mupen64plus-qt.svg`, `PCSX2.svg`, `ppsspp.svg`, `rpcs3.svg`, `ryujinx.svg`,
`vita3k.svg`, and `net.kuribo64.melonDS.svg`.

They are used only to identify the corresponding emulator.

# Bundled Azahar logo

`emulators/azahar.svg` is an unmodified copy of the official Azahar Emulator
application logo:

- Source: https://github.com/azahar-emu/azahar
- Upstream path: `dist/azahar.svg`
- Commit: `56d99197957f9c89609def36514319d961ce01eb`
- Authors: `angyartanddraw` and `PabloMK7`
- License: CC BY 4.0, declared in the asset's embedded notice
  (https://creativecommons.org/licenses/by/4.0/)
- Format: official 512x512 SVG with `viewBox="0 0 512 512"`

The Azahar repository's `license.txt` is GPL-2.0-or-later for the emulator
source, while `dist/license.md` separately records third-party UI icons. The
logo's own CC BY 4.0 notice is the applicable license for this bundled asset;
the attribution above is retained with the unmodified file. The emulator
resolver remains unchanged in this change, so Azahar still uses the project
monogram `A` until a separate UI decision activates this verified asset.
