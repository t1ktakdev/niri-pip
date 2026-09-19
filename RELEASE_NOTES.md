# niri-pip v0.3.1

v0.3.1 is a focused correctness and UX hotfix for the scratchpad window-hiding feature introduced in v0.3.0.

Highlights:

- user-facing terminology is now **Hide / Hidden windows** instead of pretending scratchpad parking is native Wayland minimize;
- new primary CLI commands: `niripip hide`, `restore-hidden`, `restore-all-hidden` and `hidden`;
- v0.3.0 command names such as `minimize` and `restore-minimized` remain available as compatibility aliases;
- recommended bindings moved to `Mod+Alt+M` and `Mod+Alt+Shift+M`, avoiding common maximize and mute shortcuts;
- `niripip-integrate` scans the active Niri config and never overwrites an occupied Hide binding;
- the hidden scratchpad is guarded: focusing the service workspace immediately returns focus to the previous normal workspace;
- Settings scans active Niri includes and labels shortcut suggestions as installed, free or conflicting;
- Settings lists currently hidden windows and can restore a specific one, the latest one, or all of them;
- transactional restore, original workspace/layout recovery, daemon-restart persistence and Niri-session scoping from v0.3.0 remain unchanged.

This is still an emulation built on a guarded service workspace because Niri 26.04 does not expose a native hidden/minimized-window IPC action. Applications stay running while hidden, but niri-pip does not claim to intercept application-native Wayland minimize requests.

See `VERIFICATION.md` for the tested matrix and `README.md` / `README.ru.md` for usage.
