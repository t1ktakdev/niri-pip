# iNiR integration

[English](../../README.md) · [Русский](../../README.ru.md)

niri-pip integrates with iNiR through stable external boundaries and does not replace iNiR QML files.

Installed pieces:

- `niripip-menu` — fuzzel controller with rofi/gum fallbacks;
- `niripip-integrate` / `niripip-unintegrate` — safe runtime KDL include management;
- `~/.config/fish/conf.d/niri-pip.fish` — adds `~/.local/bin` to fish PATH;
- `~/.local/share/applications/niri-pip.desktop` — application-launcher entry;
- a marker-scoped include in `90-user-extra.kdl` when available.

The include points to `~/.config/niri/niri-pip-runtime.kdl`. Only niri-pip owns that runtime file. This keeps PiP opacity independent from iNiR's global inactive-window rule.

Open the controller:

```sh
niripip menu
```

The controller auto-detects Russian/English locale and has a persistent **Language / Язык** selector.

Suggested shortcut:

```kdl
Mod+Alt+P { spawn "niripip" "menu"; }
```
