# Real-machine smoke test

Run this only in a disposable/normal user Niri session. It does not require root and does not edit
Niri configuration.

## Preflight

```sh
niri --version
niripip --version
systemctl --user restart niripip.service
niripip doctor
```

## Chrome/Chromium PiP

1. Open Chrome/Chromium and a YouTube video.
2. Record the focused window ID from `niri msg --json windows` if desired.
3. Enter browser Picture-in-Picture.
4. Verify `niripip status` lists one `auto-pip` window.
5. Verify it is floating and uses the configured or previously remembered manual geometry.
6. Verify keyboard focus remains in the pre-existing app/window.
7. Switch to another Niri workspace.
8. Verify the PiP follows after the workspace debounce.
9. Switch back; verify it follows again.
10. Close PiP and verify `niripip status` has no stale entry.

If Niri reports the Chromium PiP as `app_id=""`, verify the selected detector is
`chromium-empty-app-id`.

## Generic pin

1. Focus Kitty.
2. Run `niripip pin`.
3. Verify Kitty becomes floating without being resized to 480×270.
4. Switch workspaces and verify it follows.
5. Run `niripip unpin`.
6. If Kitty was tiled before pinning, verify it returns to tiling.

## Fullscreen limitation

Do not mark niri-pip as failed because a normal floating PiP is covered by focused true fullscreen. That
is a documented compositor stacking limitation. Maximized-to-working-area / normal windows are the
supported normal-window scenario.

## v0.3 controller smoke

With a real PiP open and tracked:

```sh
niripip status
niripip opacity 100
niripip follow off
niripip follow on
niripip lock
niripip unlock
```

Record the current size, then verify exact resize and restore it:

```sh
niripip --json status | jq '.data.windows'
niripip size 640 360
sleep 1
niripip status
# Restore the original width/height you recorded.
```

Verify a position preset and a nudge without focusing the PiP:

```sh
niripip position bottom-right
niripip nudge -20 -20
```

Verify Hide on an ordinary disposable test window:

```sh
niripip hide
niripip hidden
niripip restore-hidden
```

For a two-window test, hide both and verify `niripip restore-all-hidden` returns both windows and the
temporary `niri-pip:scratchpad` workspace name disappears afterward. Restart `niripip.service`
while at least one window is hidden and verify the stack is still reported. Also try focusing the
service workspace and verify the daemon immediately returns to the previous normal workspace.

Verify the menu:

```sh
niripip menu
```

Verify runtime opacity integration:

```sh
grep -Rni 'niri-pip-runtime.kdl' ~/.config/niri
cat ~/.config/niri/niri-pip-runtime.kdl
niri validate
```

`niripip opacity auto` should remove the generated opacity rule while keeping the include valid;
`niripip opacity 100` should restore `opacity 1.00`.
