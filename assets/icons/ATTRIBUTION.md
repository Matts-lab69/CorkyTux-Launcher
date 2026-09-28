# Bundled icon attribution

The `CorkyTux/symbolic/*.svg` files in this directory are copies of icons
from the **Adwaita icon theme** (GNOME), used as a bundled fallback so the
launcher does not depend on the user's system icon theme being installed.

- Source: Adwaita (https://gitlab.gnome.org/GNOME/adwaita-icon-theme)
- License: CC BY-SA 3.0 (https://creativecommons.org/licenses/by-sa/3.0/)
- Exception: `emblem-ok-symbolic.svg` is a copy of Adwaita's
  `object-select-symbolic.svg` renamed, because Adwaita ships no
  `emblem-ok-symbolic` (only full-color PNGs exist elsewhere).
- Exception: `corkytux-github-symbolic.svg` (categories) is the GitHub mark
  from `xsi-github-symbolic.svg` of the **xapp-symbolic-icon-theme** set
  (Linux Mint), cleaned of editor metadata and kept at the bundle's plain
  `fill #2e3436` ink so symbolic recoloring is preserved. The XSI project as
  a whole is distributed under LGPLv3; the GitHub mark itself remains
  GitHub's trademark, used here only as the visual for a link to
  github.com (same nominative use as the Epic/GOG brand icons below).
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
  (`simple-icons/simple-icons` @ `icons/epicgames.svg` and `icons/gogdotcom.svg`,
  adapted to the bundle as `epicgames-symbolic.svg` / `gogdotcom-symbolic.svg`
  with the theme's `fill`, a single path, no extra recoloring).
  - Source: https://github.com/simple-icons/simple-icons
  - File license: CC0 1.0
    (https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md).
    Brand logos are copyrighted by their owners; I use them solely to
    indicate integration (same criterion as Steam/Lutris/Heroic), with no
    implied endorsement.
- `steam.png`: official Steam ball icon, Wikimedia Commons
  `File:Steam icon logo.svg` (512px vector, server-rendered to 330px PNG,
  stored at 256px; verified full-color, saturation 0.62). Trademark of
  Valve Corporation — used solely to indicate Steam integration, as
  Lutris/Heroic do; no endorsement implied.
- `corkytux-steamdb.png` / `corkytux-steamdb_dark.png` /
  `corkytux-protondb.png`: SteamDB and ProtonDB logos from **Simple Icons**
  (`simple-icons/simple-icons` @ `icons/steamdb.svg` and `icons/protondb.svg`),
  rasterized to 128px PNG:
    - SteamDB hex `#000000`: the light theme uses the brand black
      (`corkytux-steamdb_dark.png`); the dark theme needs a contrast variant
      (`corkytux-steamdb.png`, white `#FFFFFF`) because the brand black
      disappears on dark backgrounds. No other recoloring — no theme-driven
      tinting beyond this contrast swap.
    - ProtonDB hex `#F50057`: single asset (`corkytux-protondb.png`).
  - Source: https://github.com/simple-icons/simple-icons
  - `license` field in the Simple Icons data (`_data/simple-icons.json`,
    jsDelivr dump of 24-sep-2026): `steamdb` → `null`, `protondb` → `null`.
    `license: null` means the project's default license applies,
    CC0 1.0
    (https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md),
    which is the one declared here for the vector artwork.
    SteamDB and ProtonDB are trademarks of their respective owners; the
    logos are used solely to indicate the integration (same criterion as
    Steam/Lutris/Heroic entries), without implying endorsement.

- `stop_dark.png`: **asset derived in this repository** (2026-09-25), not a
  third-party copy. It is `stop.png` with the ink of the opaque pixels changed
  from `#FFFFFF` to `#241F2E`, keeping dimensions (20×20), alpha channel and
  silhouette byte for byte. `stop` was the only themed icon in the bundle with
  no dark-ink variant; without it, the light theme fell back and
  painted white ink over a light background. The bundle convention is that
  `<name>.png` carries **light** ink (for the dark theme, whose background is
  `#000000`) and `<name>_dark.png` carries the **dark** ink `#241F2E` (for the
  light theme). `_dark` names the ink, not the theme. I did not alter any other
  asset.

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
the attribution above is retained with the unmodified file.

**Enabled in the resolver on 2026-09-25.** `emulator_icon_path`
(`src/ui/settings.rs`) maps `azahar` → `azahar.svg`, so the Azahar row stops
using the `A` monogram and paints the official logo. The file stays unmodified
byte for byte: I applied no center crop and no recolor, because
the logo is centered (measured: 0.00 offset horizontally). I also
verified that the rest of the logo looks like any other icon in
the row: `GtkPicture` scales it to 40×40 with `ContentFit::Contain`
without distortion, because the paintable is square (512×512) and
the logo occupies 449×509 px inside it.

## Chromium (third-party binary, not an icon)

`webdriver_login` (the binary that does the Epic/GOG login) needs a
real browser where the person types their password. I download Chrome for
Testing the first time and I always use it in an ephemeral profile that I delete
when finished.

- Pinned version: **154.0.8037.57** (`src/bin/webdriver_login.rs`, constants
  `CHROME_VERSION` and `CHROME_URL`)
- Origin: <https://googlechromelabs.github.io/chrome-for-testing/>
  (`chrome-linux64.zip`, ~188 MB compressed)
- License: **Google's terms of service**. Chrome for Testing is
  distributed under the Google Chrome terms, not under a project license. I
  redistribute it unmodified.
- Stored in `~/.local/share/corkytux/chrome-<version>/`

I prefer the system Chromium when it is already in the `PATH`, so I do not
download 188 MB when there is no need. The embedded download exists so I do not
depend on root: the Gentoo package would require privileges that a product
installed in `~/.local` cannot ask for.

**Why Chromium and not Firefox.** I implemented it first with Firefox via
geckodriver and it does not work: Epic's hCaptcha rejects the already solved
challenge because `navigator.webdriver` is `true`, and Marionette forces that
value from C++: with no pref changing it. Chromium launched by hand —without
`--enable-automation` nor headless, connected only through CDP— leaves the
value at `false`. I keep the measurements table in the module comment of
`src/bin/webdriver_login.rs`.

This project neither includes nor modifies Chromium code; I only download and
launch it.
