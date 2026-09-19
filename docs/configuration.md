# Configuration and controller behavior

The main static config is `${XDG_CONFIG_HOME:-~/.config}/niri-pip/config.toml`.
Runtime learned state is `${XDG_STATE_HOME:-~/.local/state}/niri-pip/state.json`.
PiP opacity is applied through `${XDG_CONFIG_HOME:-~/.config}/niri/niri-pip-runtime.kdl` when the
marker-scoped include is installed.

## Settings UI

Run `niripip ui` to change the everyday settings without editing TOML by hand. The UI intentionally
does not expose a window dashboard or width/height editor: move and resize the real PiP window in Niri
and learned geometry is updated when `remember_geometry = true`.

The settings app autosaves only the fields it owns and preserves unrelated manual TOML sections.
Language is stored separately in `~/.config/niri-pip/ui-language` as `auto`, `ru` or `en`.

## Static config vs learned state

Static TOML defines defaults and detector policy. Learned state stores user choices that should
survive daemon/browser restarts.

With the defaults:

```toml
[general]
remember_geometry = true

[pip]
position_mode = "remember"
```

manual PiP drag/resize becomes authoritative after the daemon's short self-action suppression
window. Arbitrary sizes are valid; the built-in size profiles are only starting points.

## Size profiles

```text
tiny     320x180
small    384x216
medium   480x270
large    640x360
cinema   960x540
custom   pip.width x pip.height
```

Exact runtime control:

```sh
niripip size 1131 636
niripip scale 10
niripip scale -10
```

For an auto PiP, explicit runtime size is immediately written into the remembered detector geometry
so a restart does not revert it.

## Universal overlay and origin restore

There are two manual-management entry points:

```sh
niripip pin
niripip overlay
```

`pin` preserves the current size. `overlay` turns the selected window into a compact floating overlay using `[overlay]`:

```toml
[overlay]
position = "bottom-right"
width = 520
height = 340
follow_workspace = true
follow_mode = "follow-workspace"
```

Before either manual mode starts, niri-pip captures the source workspace, floating/tiling mode and current size. For an already-floating window it also captures normalized position. `niripip unpin` returns the window to that source workspace and mode and restores the captured size; original floating position is restored when available.

Niri currently has no ID-addressable action for restoring a tiled window to an exact column index. niri-pip therefore restores tiling without focus-juggling. It does not claim exact tiled-column restoration when the compositor cannot guarantee it.

## Peek

Peek is temporary geometry for any already-tracked PiP, pin or overlay:

```toml
[peek]
position = "center"
width = 960
height = 540
```

```sh
niripip peek
niripip peek on
niripip peek off
```

Entering peek captures the current base geometry, applies the peek geometry and marks the window as temporarily peeking. Leaving peek restores the captured geometry. Peek layout events are excluded from learned PiP state.

While peek is active, commands that mutate base geometry (`size`, `scale`, `position`, `nudge`, `lock`, `unlock`, `reset`, `preset`) are rejected. Follow policy remains usable.

## Minimize / scratchpad

```toml
[minimize]
enabled = true
scratchpad_name = "niri-pip:scratchpad"
restore_focus = true
```

`niripip minimize` parks the focused ordinary window on a dynamically named empty workspace with `focus=false`. The scratchpad is created through Niri IPC only when needed; no permanent workspace entry is required in `config.kdl`.

```sh
niripip minimize
niripip restore-minimized
niripip restore-minimized --window-id 123
niripip restore-all
niripip minimized
```

The minimized stack is persisted in runtime state and survives a daemon restart within the same compositor session. The state records a Niri session key derived from the Linux boot ID and `NIRI_SOCKET`; a changed compositor/boot session discards old live window IDs before the engine starts. Stale entries within the same session are also pruned against Niri's authoritative window snapshot. Restore returns the original workspace and floating/tiling mode. Floating layout is left to Niri's exact per-workspace memory; tiled dimensions are replayed because Niri can lose tiled height across a workspace round-trip.

Minimize/restore requests are transactional: runtime state is committed only after every planned Niri IPC action succeeds. If an IPC action fails after earlier actions already ran, niri-pip issues best-effort compositor compensation and leaves the persisted minimized stack unchanged.

This feature is separate from application-native Wayland minimize. niri-pip cannot intercept a client's `set_minimized` request before Niri receives it.

## Named overlay profiles

Optional profiles reuse the same fields as `[overlay]`:

```toml
[profiles.study]
position = "top-right"
width = 700
height = 420
follow_workspace = true
follow_mode = "follow-workspace"
```

Apply one to the selected window with:

```sh
niripip overlay --profile study
```

Unknown profile names fail without changing the window.

## Position

Static default:

```toml
[pip]
position = "bottom-right"
```

Runtime presets:

```sh
niripip position top-left
niripip position top-right
niripip position bottom-left
niripip position bottom-right
niripip position center
```

`niripip nudge DX DY` sends ID-addressed `MoveFloatingWindow` adjustments in logical pixels.

## Follow

Static defaults:

```toml
[general]
follow_workspace = true
follow_mode = "follow-workspace"
```

Runtime per-window override:

```sh
niripip follow off
niripip follow on
niripip follow-mode follow-workspace
niripip follow-mode follow-focused-output
niripip follow-mode stay-on-output
```

Runtime follow settings are remembered per auto-PiP detector. Manual pins keep runtime settings only
for the lifetime of that tracked window.

`follow-workspace` is the primary behavior: the window follows the globally focused Niri workspace.
`follow-focused-output` currently follows the focused workspace/output pair. `stay-on-output` moves
only when the target workspace is on the same output as the tracked window.

Every workspace move uses `focus=false`.

## Geometry lock

```sh
niripip lock
niripip unlock
```

Lock captures the current width, height and normalized position. While locked, external drag/resize
changes are reconciled back to the frozen geometry without focusing the window. Controller size,
position and nudge commands update the locked target intentionally.

## Opacity

Default persistent override: 100%.

```sh
niripip opacity 100
niripip opacity 90
niripip opacity 80
niripip opacity auto
```

`auto` means no niri-pip opacity window rule; normal Niri/iNiR rules are inherited. This is useful if
you want iNiR's global `match is-active=false; opacity 0.9` behavior.

Opacity is intentionally scoped to PiP-title window rules because Niri does not expose an
ID-addressable arbitrary opacity action. It is not applied to generic manual pins.

## Presets

Presets are optional shortcuts, not hard policy:

```text
tiny    320x180, bottom-right, opacity 100
small   384x216, bottom-right, opacity 100
medium  480x270, bottom-right, opacity 100
large   640x360, bottom-right, opacity 100
cinema  960x540, bottom-right, opacity 100
movie   1120x630, bottom-right, opacity 100
study   560x315, top-right, opacity 95
```

All presets enable workspace follow. You can immediately resize/drag afterward; learned geometry
then becomes the next starting point.

## Reset

`niripip reset` removes the selected auto-PiP detector's remembered geometry/control overrides and
reapplies the static config defaults. It does not delete the entire state file or unrelated detector
profiles.

## Detector scoring

Detector constraints are hard requirements when present. After constraints pass, the detector's
base score and bonuses are summed. The highest eligible score at or above
`general.detection_threshold` wins.

The default empty-app-id detector is deliberately stronger than generic title-only matching.

## Focus policy

`pip.steal_focus=true` is rejected. The daemon never focuses a PiP just to resize, move, follow,
lock or control it. A short focus-recovery path exists only for the browser mapping race where the
new PiP itself stole focus before enough metadata arrived to classify it.

### Large identified PiP windows

The built-in browser-identified detectors (Chromium empty-app-id, Chromium-family and Firefox) do
not reject a window merely because it is large. This keeps arbitrary learned/manual PiP dimensions
usable on high-resolution displays. The generic title-only fallback retains conservative dimension
guards because it has weaker identity evidence.
