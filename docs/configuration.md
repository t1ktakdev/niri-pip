# Configuration and controller behavior

The main static config is `${XDG_CONFIG_HOME:-~/.config}/niri-pip/config.toml`.
Runtime learned state is `${XDG_STATE_HOME:-~/.local/state}/niri-pip/state.json`.
PiP opacity is applied through `${XDG_CONFIG_HOME:-~/.config}/niri/niri-pip-runtime.kdl` when the
marker-scoped include is installed.

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
