# iNiR integration

[English](../../README.md) · [Русский](../../README.ru.md)

niri-pip integrates with iNiR through stable external boundaries and does not replace iNiR QML files.

Installed pieces:

- `niripip-ui` — local settings application opened by `niripip ui`;
- `niripip-menu` — compact fuzzel controller with rofi/gum fallbacks;
- `niripip-integrate` / `niripip-unintegrate` — safe runtime KDL include management;
- `~/.config/fish/conf.d/niri-pip.fish` — adds `~/.local/bin` to fish PATH;
- `~/.local/share/applications/niri-pip.desktop` — application-launcher entry;
- a marker-scoped include in `90-user-extra.kdl` when available.

The include points to `~/.config/niri/niri-pip-runtime.kdl`. Only niri-pip owns that runtime file. This keeps PiP opacity independent from iNiR's global inactive-window rule.

Open the settings application:

```sh
niripip ui
```

The settings app supports **Automatic / Русский / English** and stores the preference in `~/.config/niri-pip/ui-language`.

The compact power-user controller is still available:

```sh
niripip menu
```

Suggested shortcuts:

```kdl
Mod+Alt+M repeat=false { spawn "niripip" "hide"; }
Mod+Alt+Shift+M repeat=false { spawn "niripip" "restore-hidden"; }
Mod+Alt+P { spawn "niripip" "ui"; }
```
