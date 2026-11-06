# Third-party system programs

This inventory complements [CREDITS.md](../CREDITS.md). These are runtime
integrations, not a requirement to bundle their implementations. Compatible
system replacements may have different terms. Upstream license references were
checked on 2026-11-04; installed packages retain their own component notices.

## System programs used by the application

| Project / commands | GitHub / canonical source | License evidence | Relationship |
|---|---|---|---|
| curl | [curl](https://github.com/curl/curl) | [COPYING](https://github.com/curl/curl/blob/master/COPYING): curl license | **Executed**: image/network transfers. |
| FFmpeg | [FFmpeg](https://github.com/FFmpeg/FFmpeg) | [LICENSE.md](https://github.com/FFmpeg/FFmpeg/blob/master/LICENSE.md): LGPL-2.1-or-later by default; GPL options and other dependencies can change the build's terms | **Executed**: image/media conversion. |
| icoextract | [icoextract](https://github.com/jlu5/icoextract) | [MIT LICENSE](https://github.com/jlu5/icoextract/blob/master/LICENSE) | **Executed**: executable icon extraction. |
| GameMode / gamemoderun | [gamemode](https://github.com/FeralInteractive/gamemode) | [BSD-3-Clause LICENSE.txt](https://github.com/FeralInteractive/gamemode/blob/master/LICENSE.txt) | **Executed**: optional game wrapper. |
| MangoHud | [MangoHud](https://github.com/flightlessmango/MangoHud) | [MIT LICENSE](https://github.com/flightlessmango/MangoHud/blob/master/LICENSE) | **Executed**: optional performance display. |
| Bubblewrap / bwrap | [bubblewrap](https://github.com/containers/bubblewrap) | [COPYING](https://github.com/containers/bubblewrap/blob/main/COPYING): LGPL v2.1 text | **Executed**: launcher and RPG sandbox. |
| Flatpak | [flatpak](https://github.com/flatpak/flatpak) | [COPYING](https://github.com/flatpak/flatpak/blob/main/COPYING): LGPL v2.1 text | **Executed**: optional Steam/application startup. |
| xdg-dbus-proxy | [xdg-dbus-proxy](https://github.com/flatpak/xdg-dbus-proxy) | [COPYING](https://github.com/flatpak/xdg-dbus-proxy/blob/main/COPYING): LGPL v2.1 text | **Executed**: vendored runtime's GameMode D-Bus proxy. |
| Winetricks | [winetricks](https://github.com/Winetricks/winetricks) | [COPYING](https://github.com/Winetricks/winetricks/blob/master/COPYING): LGPL v2.1 text | **Executed** on explicit dependency-install actions. |
| Zenity | [zenity](https://github.com/GNOME/zenity) | [COPYING](https://github.com/GNOME/zenity/blob/master/COPYING): LGPL v2.1 text at the checked revision | **Executed**: optional dialogs. |
| KDialog | [kdialog](https://github.com/KDE/kdialog) | [LICENSES/GPL-2.0-or-later.txt](https://github.com/KDE/kdialog/blob/master/LICENSES/GPL-2.0-or-later.txt) | **Executed**: dialog fallback. |
| xdg-utils / xdg-open, xdg-user-dir | [Canonical source](https://gitlab.freedesktop.org/xdg/xdg-utils) | [MIT-style LICENSE](https://gitlab.freedesktop.org/xdg/xdg-utils/-/blob/master/LICENSE) | **Executed**: open files/URLs and locate desktop directories. |
| xdotool | [xdotool](https://github.com/jordansissel/xdotool) | [BSD-3-Clause COPYRIGHT](https://github.com/jordansissel/xdotool/blob/master/COPYRIGHT) | **Executed**: browser window/display geometry. |
| SQLite / sqlite3 | [sqlite](https://github.com/sqlite/sqlite) | [LICENSE.md](https://github.com/sqlite/sqlite/blob/master/LICENSE.md): public domain for SQLite; ancillary-file exceptions noted separately | **Executed**: local library imports. |
| GNU Coreutils / cp, date, uname | [coreutils](https://github.com/coreutils/coreutils) | [GPL v3 COPYING](https://github.com/coreutils/coreutils/blob/master/COPYING) | **Executed** when these system commands are supplied by Coreutils. |
| GNU tar | [GitHub source mirror](https://github.com/Distrotech/tar), [canonical GNU project](https://www.gnu.org/software/tar/) | [GPL v3 COPYING](https://github.com/Distrotech/tar/blob/master/COPYING) | **Executed** for archives when GNU tar supplies the system command. |
| UnRAR | [Source mirror](https://github.com/pmachapman/unrar), [RARLAB](https://www.rarlab.com/) | [license.txt](https://github.com/pmachapman/unrar/blob/master/license.txt): custom restrictive terms; not an OSI open-source license | **Executed** for optional RAR extraction. |
| pciutils / lspci | [pciutils](https://github.com/pciutils/pciutils) | [GPL v2 COPYING](https://github.com/pciutils/pciutils/blob/master/COPYING) | **Executed**: GPU information. |
| Mesa demos / glxinfo | [Canonical source](https://gitlab.freedesktop.org/mesa/demos) | [glxinfo.c licensing header](https://gitlab.freedesktop.org/mesa/demos/-/blob/main/src/xdemos/glxinfo.c): MIT | **Executed**: OpenGL information. |
| dpkg / update-alternatives | [GitHub source mirror](https://github.com/guillemj/dpkg) | [GPL v2 COPYING](https://github.com/guillemj/dpkg/blob/main/COPYING) | **Executed when available**: Java discovery. |
