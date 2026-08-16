# Troubleshooting

## `NIRI_SOCKET is not set`

Start Niri as a session (`niri-session` / `niri --session`). Current Niri imports `NIRI_SOCKET` and
other display variables into the systemd user manager during session startup. Then restart the user
service:

```sh
systemctl --user restart niripip.service
niripip doctor
```

Do not hard-code a socket filename into the unit; Niri socket paths are runtime state.

## Daemon works in terminal but not as a service

Check the manager environment and unit logs:

```sh
systemctl --user show-environment | grep -E 'NIRI_SOCKET|WAYLAND_DISPLAY'
systemctl --user status niripip.service
journalctl --user -u niripip.service -b --no-pager -n 100
```

If Niri itself was not launched as a session, fix the Niri session startup rather than adding a
permanent stale socket path.

## PiP is detected but overlaps my bar/dock

Niri's IPC exposes output dimensions, not the full calculated usable-area rectangle. Increase the
corresponding `[margins]` value. Keep in mind that proper layer-shell exclusive zones are already
part of Niri's working area, so large margins can double-count space.

## PiP is behind true fullscreen

This is a compositor stacking limitation, not a detector bug. Focused true fullscreen can cover
normal floating windows in Niri. v0.1 does not use an overlay-layer hack. Exit true fullscreen, use a
normal/maximized window, or follow the future overlay research roadmap.

## Chrome PiP not detected

Inspect what Niri actually sees:

```sh
niri msg --json windows
journalctl --user -u niripip.service -f
```

Then add a narrow custom detector using the observed title/app-id. Do not rely on a browser PID or
invented XWayland property that Niri does not expose.

## Service restart after Niri restart

Niri v26.04 names its IPC socket with the compositor PID, so a compositor restart normally changes
`NIRI_SOCKET`. The unit is `PartOf=niri.service` and the installer also adds it as a Niri service
want when the Niri systemd unit exists. Independently, niripipd treats a disappeared socket or a
PID-scoped socket whose owner exited as a full compositor restart and exits, allowing systemd to
start a fresh process with the new manager environment. Transient disconnects from the same live
Niri process use bounded exponential backoff instead.
