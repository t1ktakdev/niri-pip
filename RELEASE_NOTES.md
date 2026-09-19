# niri-pip v0.3.0

v0.3.0 keeps the automatic PiP core small while adding a much more usable configuration path and stronger manual-window controls.

Highlights:

- local `niripip ui` settings application with a restrained desktop-oriented interface;
- Automatic / Русский / English language selection persisted across UI launches;
- autosaved settings for PiP detection, remembered geometry, workspace following, restore behavior, focus policy, aspect ratio and opacity;
- real systemd user-session autostart control from the settings app;
- daemon, Niri IPC, iNiR and desktop-entry diagnostics;
- safe global reset for learned PiP geometry;
- copyable recommended Niri keybinds without rewriting the user's Niri config;
- scratchpad-style minimize/restore for ordinary windows with `Mod+M` / `Mod+Shift+M`, restore-last, restore-all and persistent minimized state;
- transactional minimize/restore execution: state is committed only after all Niri IPC actions succeed, with compositor compensation on partial failure;
- minimized live IDs are scoped to the current Niri session, preserving daemon-restart recovery while discarding stale IDs after a compositor restart or reboot;
- universal `overlay` mode for manually selected windows;
- temporary `peek` mode with exact base-geometry restore;
- named overlay profiles;
- origin-aware `unpin` restore for manually managed windows;
- compact `niripip menu` controller retained for power users, now including minimize/restore actions;
- release/install/AUR packaging includes the new `niripip-ui` binary.

The Settings UI intentionally does not expose a window dashboard or manual width/height editor. Users resize and move the real PiP window in Niri; niri-pip learns that geometry when `remember_geometry` is enabled. Its Behavior page also exposes the scratchpad minimize policy, minimized-window count, restore controls and a compatibility notice for application-native minimize on Niri 26.04.

See `VERIFICATION.md` for the tested matrix and `README.md` / `README.ru.md` for usage.
