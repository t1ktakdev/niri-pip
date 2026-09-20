# niri-pip v0.3.2

v0.3.2 is a small hotfix for the Hidden-window restore shortcut.

Highlights:

- `Mod+Alt+M` still hides the focused ordinary window;
- restore is now `Mod+Alt+U` (`U` for unhide);
- the old `Mod+Alt+Shift+M` recommendation is removed because `Alt+Shift` is commonly used as an XKB layout switch and can collide with iNiR's `Mod+Shift+M` audio-mute shortcut;
- `niripip-integrate` removes its generated Hide shortcut block and recreates it idempotently, so rerunning integration upgrades the generated binding safely;
- Settings, English/Russian documentation, Arch install hints and release verification checks now report the same shortcut.

No daemon protocol, persisted-state schema or Hide/restore behavior changed in this release.
