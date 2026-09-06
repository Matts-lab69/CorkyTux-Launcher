# Emulator manager plugin

CorkyTux communicates with the installed `emulator-manager` executable using JSON
on stdout. The plugin is normally installed at:

```text
~/.local/share/CorkyTux/plugins/emulator-manager/emulator-manager
```

The launcher uses these commands:

```bash
emulator-manager corky-list
emulator-manager corky-install <name>
emulator-manager corky-remove <name>
emulator-manager corky-unlink <name>
emulator-manager corky-register-game <emulator> <game_dir>
```

## Link native emulators

A native emulator can be linked by the plugin's own discovery/link workflow. After
linking, `corky-list` reports `linked: true` and the executable path points outside
the plugin directory, for example `/usr/bin/melonDS`.

```bash
emulator-manager corky-list
```

CorkyTux displays linked emulators as `(native)`. The Remove button is disabled for
them and `corky-remove` is never called, so a system emulator is not deleted.

## Install an AppImage

Use the plugin command through Settings > Plugins > Emulator Manager, or run:

```bash
emulator-manager corky-install melonDS
```

The plugin returns an entry with `installed: true`, `linked: false`, and a path inside
its emulator directory. CorkyTux allows Remove for this case and calls:

```bash
emulator-manager corky-remove melonDS
```

## Register a game directory

To add a game directory to an emulator's own game list:

```bash
emulator-manager corky-register-game Dolphin /path/to/game-directory
```

## JSON contract

`corky-list` returns an object like:

```json
{
  "ok": true,
  "emulators": [
    {
      "name": "Dolphin",
      "description": "GameCube / Wii",
      "extensions": ["iso", "rvz"],
      "installed": true,
      "linked": true,
      "path": "/usr/bin/dolphin-emu",
      "launch_args": "-e {rom}",
      "settings": []
    }
  ]
}
```

Game Settings stores emulator-specific settings as `emuSettings` in the game's
profile. The launcher converts those values into emulator arguments before launch.
