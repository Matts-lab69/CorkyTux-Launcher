# Credits and third-party components

CorkyTux thanks the projects and people below. This inventory covers the launcher,
its optional plugin packages, and their runtime integrations; not every component
is installed or shipped in every build. External programs are not relicensed by
CorkyTux's [AGPL v3 license](LICENSE).

**Included** means source or artwork is copied into a CorkyTux package.
**Downloaded** means an optional component is obtained separately at runtime.
**Executed** means a separately installed program is invoked.
**Inspiration** means design or workflow influence, not a claim of copied code.

License files were checked on **2026-11-04**. Emulator downloads may select the
latest release, and system programs are resolved through PATH. Pinned runtime
versions and checksum sources are listed in [docs/DOWNLOAD_SECURITY.md](docs/DOWNLOAD_SECURITY.md). The references below
identify upstream source terms, not the exact contents of every AppImage or
installed distribution package. Preserve the notices supplied with each artifact.
Where a GNU license file supplies only its standard text, the table records its
version without inventing an `only` or `or-later` grant. A confirmed project-specific
grant is identified explicitly.

## Included source and artwork

| Project / author | Upstream | License evidence | Relationship |
|---|---|---|---|
| BOX — RPG Maker / box-project, christvh | [Canonical GitLab project](https://gitlab.com/christvh/box-project) | BSD-3-Clause, read in the plugin's `rpgmaker-runtime/vendor/BOX-RPG-LICENSE` | **Included**: adapted box-rpg backend under `rpgmaker-runtime/vendor/box-rpg-src/`; recorded upstream tag `26.9.138`. This is actual source reuse, not merely inspiration. |
| Kenney, Input Prompts 1.5A | [GitHub author profile](https://github.com/KenneyNL), [official asset pack](https://kenney.nl/assets/input-prompts) | CC0, explicitly stated in the supplied `assets/input-prompts/License.txt` | **Included**: selected controller SVGs. Keep the supplied CC0 notice with the assets. The profile is an author link, not a claimed source repository for the pack. |
| GNOME / Adwaita icon theme | [GitHub mirror](https://github.com/GNOME/adwaita-icon-theme) | LGPL v3, selected from the alternatives offered in [COPYING](https://github.com/GNOME/adwaita-icon-theme/blob/master/COPYING) | **Included**: symbolic fallback artwork; retain the existing GNOME attribution and renamed-file details. |
| Papirus Development Team | [papirus-icon-theme](https://github.com/PapirusDevelopmentTeam/papirus-icon-theme) | [GPL v3 LICENSE at the recorded revision](https://github.com/PapirusDevelopmentTeam/papirus-icon-theme/blob/bf539287ef5dc18529424a02cccee76175920a6f/LICENSE) | **Included**: emulator and action icons. Preserve the recorded revision and attribution in `assets/icons/ATTRIBUTION.md`. |
| Simple Icons contributors | [simple-icons](https://github.com/simple-icons/simple-icons) | [CC0 LICENSE.md](https://github.com/simple-icons/simple-icons/blob/develop/LICENSE.md) | **Included**: selected Epic, GOG, SteamDB and ProtonDB artwork. Trademark rights remain with the respective owners. |
| Linux Mint, Mint themes | [mint-themes](https://github.com/linuxmint/mint-themes) | [debian/copyright](https://github.com/linuxmint/mint-themes/blob/master/debian/copyright): explicit GPL-3.0-or-later grant | **Included**: the store-button glyph identified in the existing artwork notice. |
| XApp symbolic icons | [xapp-symbolic-icons](https://github.com/xapp-project/xapp-symbolic-icons) | [LGPL v3 LICENSE](https://github.com/xapp-project/xapp-symbolic-icons/blob/master/LICENSE) | **Included**: the GitHub-link mark. Preserve the existing notice and GitHub's separate trademark conditions. |
| Lutris contributors | [lutris](https://github.com/lutris/lutris) | [GPL v3 LICENSE](https://github.com/lutris/lutris/blob/master/LICENSE) | **Included / executed**: application logo; optional import through the separately installed client. |
| Heroic Games Launcher contributors | [HeroicGamesLauncher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher) | [GPL v3 COPYING](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/COPYING) | **Included / inspiration**: application logo and explicitly credited store workflow influence; the Heroic application itself is not bundled. |
| Azahar logo authors, angyartanddraw and PabloMK7 | [azahar](https://github.com/azahar-emu/azahar) | CC BY 4.0, recorded in the SVG's own notice and `assets/icons/ATTRIBUTION.md` | **Included**: the logo from revision `56d99197957f9c89609def36514319d961ce01eb`. Its artwork license is distinct from the emulator's source license. |
| Valve and other brand owners | [ValveSoftware](https://github.com/ValveSoftware) and the owners named in the artwork notice | Trademark and asset-specific terms; no blanket software license is asserted | **Included**: integration-identification logos. No endorsement is implied. |

The authoritative per-file artwork record remains
[assets/icons/ATTRIBUTION.md](assets/icons/ATTRIBUTION.md). The plugin's
`rpgmaker-runtime/NOTICE`, `vendor/BOX-RPG-LICENSE`, and `vendor/VENDOR.txt` must
remain alongside its included source. This document does not replace them.

## Downloaded and executed backends and runtimes

| Project | GitHub / upstream | License file or vendor terms read | Relationship |
|---|---|---|---|
| Legendary | [legendary-gl/legendary](https://github.com/legendary-gl/legendary) | [GPL v3 LICENSE](https://github.com/legendary-gl/legendary/blob/master/LICENSE) | **Downloaded / executed**: Epic backend for the Heroic Store plugin. |
| Heroic gogdl | [heroic-gogdl](https://github.com/Heroic-Games-Launcher/heroic-gogdl) | [GPL v3 LICENSE](https://github.com/Heroic-Games-Launcher/heroic-gogdl/blob/main/LICENSE) | **Downloaded / executed**: GOG backend. |
| umu-launcher | [umu-launcher](https://github.com/Open-Wine-Components/umu-launcher) | [GPL v3 LICENSE](https://github.com/Open-Wine-Components/umu-launcher/blob/main/LICENSE) | **Downloaded / executed**: optional compatibility runner. |
| GE-Proton | [proton-ge-custom](https://github.com/GloriousEggroll/proton-ge-custom) | [LICENSE](https://github.com/GloriousEggroll/proton-ge-custom/blob/master/LICENSE): multiple component licenses; also retain `LICENSE.proton` and component notices | **Downloaded / executed**: optional Proton distribution; not a single-license bundle. |
| Valve Proton | [Proton](https://github.com/ValveSoftware/Proton) | [LICENSE](https://github.com/ValveSoftware/Proton/blob/master/LICENSE): multiple component licenses | **Executed**: selected compatibility builds, including Steam-managed installations. |
| Wine | [wine-mirror/wine](https://github.com/wine-mirror/wine) | [LICENSE](https://github.com/wine-mirror/wine/blob/master/LICENSE): explicit LGPL-2.1-or-later grant | **Executed**: compatibility components supplied through the chosen runner. |
| Steam Linux Runtime | [steam-runtime](https://github.com/ValveSoftware/steam-runtime) | [COPYING](https://github.com/ValveSoftware/steam-runtime/blob/master/COPYING): Valve scripts/documentation under MIT; individual runtime packages have their own licenses | **Downloaded / executed**: optional runtime/container supplied with runner installations. |
| NW.js | [nwjs/nw.js](https://github.com/nwjs/nw.js) | [MIT LICENSE](https://github.com/nwjs/nw.js/blob/main/LICENSE); binaries additionally contain separately licensed components | **Downloaded / executed**: RPG Maker MV/MZ runtime. |
| EasyRPG Player | [EasyRPG/Player](https://github.com/EasyRPG/Player) | [GPL v3 COPYING](https://github.com/EasyRPG/Player/blob/master/COPYING) | **Downloaded / executed**: RPG Maker 2000/2003 runtime. |
| minecraft-launcher-lib, JakobDev | [minecraft-launcher-lib](https://github.com/JakobDev/minecraft-launcher-lib) | [BSD-2-Clause LICENSE](https://github.com/JakobDev/minecraft-launcher-lib/blob/master/LICENSE) | **Downloaded / executed**: Python install/launch library used by the Minecraft plugin; not an inspiration-only credit. |
| Requests | [psf/requests](https://github.com/psf/requests) | [Apache-2.0 LICENSE](https://github.com/psf/requests/blob/main/LICENSE) | **Downloaded / executed**: HTTP dependency of the Python plugin backend. |
| Python | [python/cpython](https://github.com/python/cpython) | [LICENSE](https://github.com/python/cpython/blob/main/LICENSE): PSF License Version 2 plus incorporated-component notices | **Executed**: plugin interpreter and standard library. |
| Steam client | [steam-for-linux issue tracker](https://github.com/ValveSoftware/steam-for-linux), [Steam Subscriber Agreement](https://store.steampowered.com/subscriber_agreement/) | Valve's client/subscription terms, not the issue tracker's source license | **Executed**: Steam URI handling and optional overlay integration; client code is not included here. |
| Google Chrome for Testing | [chrome-for-testing](https://github.com/GoogleChromeLabs/chrome-for-testing), [Google Chrome terms](https://www.google.com/chrome/terms/) | Google's browser terms; not Chromium's BSD source license | **Downloaded / executed**: optional browser, version `154.0.8037.57` in the source. The availability repository's license does not license the browser binary. |
| Chromium | [GitHub source mirror](https://github.com/chromium/chromium), [canonical LICENSE](https://chromium.googlesource.com/chromium/src/+/main/LICENSE) | BSD-style main license with component-specific notices | **Executed**: an existing system browser can be selected instead of downloading Chrome. |

Game content, Microsoft/Mojang accounts, Ely.by authentication, Modrinth and
CurseForge services remain subject to their respective provider/content terms.
Using an API or downloading a game does not mean its server or game source is
included in CorkyTux or receives CorkyTux's license.

## Optional emulator downloads

The emulator catalog obtains release-latest AppImages. The source project and
the AppImage packaging repository are not interchangeable license authorities.
These are credits, not permission to redistribute every image as an AGPL bundle.

| Emulator / source | License evidence | Relationship and download source |
|---|---|---|
| [Dolphin](https://github.com/dolphin-emu/dolphin) | [COPYING](https://github.com/dolphin-emu/dolphin/blob/master/COPYING): most original source GPL-2.0-or-later; aggregate has component-specific terms | **Downloaded / executed**; catalog packages from [Dolphin-emu-AppImage](https://github.com/pkgforge-dev/Dolphin-emu-AppImage). |
| [PCSX2](https://github.com/PCSX2/pcsx2) | [COPYING.GPLv3](https://github.com/PCSX2/pcsx2/blob/1ccd6a8dc3574771c138849039d71951a3a1c175/COPYING.GPLv3): GPL v3 text | **Downloaded / executed** from the project's releases. |
| [PPSSPP](https://github.com/hrydgard/ppsspp) | [LICENSE.TXT](https://github.com/hrydgard/ppsspp/blob/3bc60270eae3069f13dfbfdb7292a7e1375de6f6/LICENSE.TXT), together with the [project's explicit GPL 2.0-and-later statement](https://github.com/hrydgard/ppsspp/blob/master/README.md): GPL-2.0-or-later plus component notices | **Downloaded / executed** from the project's releases. The PSPSDK notice alone does not license the entire program as BSD. |
| [RPCS3](https://github.com/RPCS3/rpcs3) | [LICENSE](https://github.com/RPCS3/rpcs3/blob/master/LICENSE): GPL v2 text | **Downloaded / executed**; catalog packages from [RPCS3-AppImage-Enhanced](https://github.com/Blackfox-Thane/RPCS3-AppImage-Enhanced). |
| [Vita3K](https://github.com/Vita3K/Vita3K) | [COPYING.txt](https://github.com/Vita3K/Vita3K/blob/f34f11ee44cf04622826d27921a1ff0a02b8ba33/COPYING.txt): GPL v2 text | **Downloaded / executed** from the project's releases. |
| [Azahar](https://github.com/azahar-emu/azahar) | [LICENSE.txt](https://github.com/azahar-emu/azahar/blob/master/LICENSE.txt): GPL v3 text at the checked upstream revision; asset terms are separate | **Downloaded / executed**; catalog packages from [Azahar-AppImage-Enhanced](https://github.com/pkgforge-dev/Azahar-AppImage-Enhanced). |
| [DuckStation](https://github.com/stenzek/duckstation) | [LICENSE](https://github.com/stenzek/duckstation/blob/ce8a79b272f4b8f4572a57ab5859a125d816e482/LICENSE): **CC BY-NC-ND 4.0**, including non-commercial and no-derivatives restrictions | **Downloaded / executed** from the project's releases. Do not describe these terms as an unrestricted open-source license. |
| [Cemu](https://github.com/cemu-project/Cemu) | [MPL-2.0 LICENSE.txt](https://github.com/cemu-project/Cemu/blob/3f8dd913cfd4c8247ad65ab41ba6ef0a55926769/LICENSE.txt) | **Downloaded / executed** from the project's releases. |
| [melonDS](https://github.com/melonDS-emu/melonDS) | [LICENSE](https://github.com/melonDS-emu/melonDS/blob/master/LICENSE): GPL v3 text | **Downloaded / executed** via [melonds-anylinux-appimage](https://github.com/carlos-a-g-h/melonds-anylinux-appimage). Its separate [MIT packaging license](https://github.com/carlos-a-g-h/melonds-anylinux-appimage/blob/6b3b4fa9089faf020c1aff470d72ed065d98f5f8/LICENSE) is not the emulator binary's license. |
| [Mupen64Plus core](https://github.com/mupen64plus/mupen64plus-core) | [doc/gpl-license](https://github.com/mupen64plus/mupen64plus-core/blob/master/doc/gpl-license): GPL v2 text; other modules have their own notices | **Downloaded / executed** via [mupen64plus-appimage](https://github.com/DarkSamus669/mupen64plus-appimage). |
| [DeSmuME](https://github.com/TASEmulators/desmume) | [desmume/COPYING](https://github.com/TASEmulators/desmume/blob/master/desmume/COPYING): GPL v2 text | **Downloaded / executed** via [DeSmuME-AppImage](https://github.com/pkgforge-dev/DeSmuME-AppImage). |

## System programs used by the application

These are runtime integrations, not a requirement to bundle their implementations.
Compatible system replacements may have different terms.
The full system-program inventory is in [docs/THIRD-PARTY.md](docs/THIRD-PARTY.md).

| Project / commands | GitHub / canonical source | License evidence | Relationship |
|---|---|---|---|
| FFmpeg | [FFmpeg](https://github.com/FFmpeg/FFmpeg) | [LICENSE.md](https://github.com/FFmpeg/FFmpeg/blob/master/LICENSE.md): LGPL-2.1-or-later by default; GPL options and other dependencies can change the build's terms | **Executed**: image/media conversion. |
| icoextract | [icoextract](https://github.com/jlu5/icoextract) | [MIT LICENSE](https://github.com/jlu5/icoextract/blob/master/LICENSE) | **Executed**: executable icon extraction. |
| GameMode / gamemoderun | [gamemode](https://github.com/FeralInteractive/gamemode) | [BSD-3-Clause LICENSE.txt](https://github.com/FeralInteractive/gamemode/blob/master/LICENSE.txt) | **Executed**: optional game wrapper. |
| MangoHud | [MangoHud](https://github.com/flightlessmango/MangoHud) | [MIT LICENSE](https://github.com/flightlessmango/MangoHud/blob/master/LICENSE) | **Executed**: optional performance display. |
| Bubblewrap / bwrap | [bubblewrap](https://github.com/containers/bubblewrap) | [COPYING](https://github.com/containers/bubblewrap/blob/main/COPYING): LGPL v2.1 text | **Executed**: launcher and RPG sandbox. |
| Winetricks | [winetricks](https://github.com/Winetricks/winetricks) | [COPYING](https://github.com/Winetricks/winetricks/blob/master/COPYING): LGPL v2.1 text | **Executed** on explicit dependency-install actions. |
| UnRAR | [Source mirror](https://github.com/pmachapman/unrar), [RARLAB](https://www.rarlab.com/) | [license.txt](https://github.com/pmachapman/unrar/blob/master/license.txt): custom restrictive terms; not an OSI open-source license | **Executed** for optional RAR extraction. |

## Inspiration and acknowledgements

| Project / people | GitHub / upstream | License evidence | Relationship |
|---|---|---|---|
| Heroic Games Launcher | [HeroicGamesLauncher](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher) | [GPL v3 COPYING](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/COPYING) | **Inspiration** for the Stores plugin's login, library scanning and runtime workflows, as credited in `heroic-store/README.md`. The Heroic application is not bundled in the plugin. |
| BOX — RPG Maker / box-project | [Canonical GitLab project](https://gitlab.com/christvh/box-project) | Included BSD-3-Clause notice, identified above | **Inspiration** for CorkyTux as a whole, beyond the RPG Maker plugin. **Included source / adaptation**: the plugin also adapts the box-rpg backend, as credited in `rpgmaker-runtime/README.md` and `NOTICE`. |
| PineconeMC / ElyPrismLauncher | [ElyPrismLauncher/Launcher](https://github.com/ElyPrismLauncher/Launcher) | [GPL v3 LICENSE](https://github.com/ElyPrismLauncher/Launcher/blob/develop/LICENSE) | **Inspiration** for the Minecraft launcher plugin; no source reuse is claimed by this acknowledgement. |
| Cristioro | [GitHub profile](https://github.com/Cristioro) | Personal acknowledgement, not a third-party code license | UI ideas and efficiency suggestions, preserving the existing README credit. |
| ZzEdovec | [GitHub profile](https://github.com/ZzEdovec) | Personal acknowledgement, not a third-party code license | Some interfaces and icons used or inspired by this person's repositories, preserving the existing README acknowledgement. |
