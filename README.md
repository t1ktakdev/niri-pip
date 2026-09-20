# niri-pip

[![CI](https://github.com/t1ktakdev/niri-pip/actions/workflows/ci.yml/badge.svg)](https://github.com/t1ktakdev/niri-pip/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/t1ktakdev/niri-pip)](https://github.com/t1ktakdev/niri-pip/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**English** · [Русский](README.ru.md)

Smart Picture-in-Picture, sticky windows and compact overlays for the Niri Wayland compositor.

`niri-pip` watches Niri's event stream, detects browser PiP windows, keeps them floating, follows the active workspace without stealing focus, remembers free-form geometry, provides a guarded Hide/restore flow for normal windows, and exposes a small controller for size, position, opacity, follow mode, locking and media keys.

## Highlights

- Event-driven Niri IPC; no `niri msg` polling loop.
- Chromium PiP detection when Niri reports an empty `app-id`.
- Firefox and Chromium-family PiP defaults.
- Workspace following with `focus=false`.
- Arbitrary manual width/height; manual geometry is learned instead of forced back to a preset.
- Geometry lock/unlock.
- Five positions plus pixel nudging.
- PiP-only opacity override without changing global iNiR window opacity rules.
- Generic `pin`, `unpin` and `toggle` for normal windows.
- Universal `overlay` mode for turning any window into a compact sticky overlay.
- Temporary `peek` mode that enlarges a tracked window and restores its exact base geometry.
- Origin-aware restore: manual windows return to their original workspace and floating/tiling mode; original floating geometry is restored when available.
- Named overlay profiles for repeatable call, study, monitoring and other layouts.
- `hide` / `restore-hidden` / `restore-all-hidden` for ordinary windows: the app keeps running while the window is parked on a guarded service workspace, with workspace/layout restoration across daemon restarts.
- Optional MPRIS controls through `playerctl`.
- Compact controller using fuzzel, with rofi/gum fallbacks.
- Human-sized Settings UI with Automatic/Russian/English language selection and autosave.
- systemd user service that starts with the graphical Niri session and restarts automatically.
- Safe iNiR integration through a separate runtime KDL file and marker-scoped include.

## Verified environment

v0.3.1 passed the full release preflight on Arch Linux with Niri 26.04 and Rust 1.97.1: 77 workspace tests, clippy with warnings denied, locked release build, real installer/rollback validation, `niri validate`, `doctor`, live Hide/guard/restore acceptance and live Settings per-window restore. The exact matrix is documented below.

See [VERIFICATION.md](VERIFICATION.md) for the exact verification matrix.

## Install

### Fast install from a GitHub Release

Once a release is published, the shortest path is:

```sh
curl -fsSL https://raw.githubusercontent.com/t1ktakdev/niri-pip/main/scripts/install-release.sh | bash
```

The installer downloads the x86_64 release archive and checksum, verifies SHA-256, installs into your user account, enables the systemd user service, installs the launcher and validates the Niri integration.

### Build from source

```sh
git clone https://github.com/t1ktakdev/niri-pip.git
cd niri-pip
./install.sh
```

Requirements for a source build:

- Niri 26.04+
- Rust/Cargo
- systemd user session
- Python 3
- Bash

On Arch Linux:

```sh
sudo pacman -S --needed rust python
```

Optional controller/media packages:

```sh
sudo pacman -S --needed fuzzel playerctl
```

## Start and autostart

Installation enables `niripip.service` for the graphical user session. No manual daemon command is required after login.

```sh
systemctl --user status niripip.service
niripip doctor
```

Useful service commands:

```sh
systemctl --user restart niripip.service
systemctl --user stop niripip.service
journalctl --user -u niripip.service -f
```

The service uses `Restart=always` while the Niri graphical session is active. It is stopped with the session and can be stopped normally with `systemctl --user stop`.

## Settings UI

Open **niri-pip Settings** from your application launcher, or run:

```sh
niripip ui
```

The settings app is intentionally small: it configures niri-pip behavior instead of acting as a window-control dashboard. Resize and move the real PiP window normally in Niri; when **Remember size and position** is enabled, niri-pip learns those manual changes automatically.

The UI provides:

- Automatic / Russian / English language selection;
- automatic PiP detection;
- systemd user-session autostart;
- remembered PiP geometry;
- workspace following and follow mode;
- original-state restore after unpin;
- focus-stealing prevention and aspect-ratio policy;
- default PiP opacity;
- Hide enable/focus policy, hidden-window list/count and per-window/latest/all restore actions;
- daemon, Niri IPC, iNiR and desktop-entry diagnostics;
- safe copyable Niri keybind suggestions.

Preferences autosave. The language choice is stored in `~/.config/niri-pip/ui-language`, while behavioral settings remain normal `config.toml` fields. The UI writes only the fields it owns and preserves unrelated manual configuration.

Suggested Niri shortcut:

```kdl
binds {
    Mod+Alt+M repeat=false { spawn "niripip" "hide"; }
    Mod+Alt+U repeat=false { spawn "niripip" "restore-hidden"; }
    Mod+Alt+P { spawn "niripip" "ui"; }
}
```

The compact fuzzel/rofi/gum power-user controller is still available with `niripip menu`.

If iNiR already owns your `binds` layout, merge only the binding itself instead of creating a second conflicting block.

## CLI

```sh
niripip status
niripip list
niripip doctor

niripip hide
niripip restore-hidden
niripip restore-hidden --window-id 123
niripip restore-all-hidden
niripip hidden

niripip size 1131 636
niripip scale 10
niripip scale -10

niripip position top-left
niripip position top-right
niripip position bottom-left
niripip position bottom-right
niripip position center

niripip nudge -20 0
niripip nudge 20 0
niripip nudge 0 -20
niripip nudge 0 20

niripip opacity 100
niripip opacity 80
niripip opacity auto

niripip follow on
niripip follow off
niripip follow-mode follow-workspace
niripip follow-mode follow-focused-output
niripip follow-mode stay-on-output

niripip lock
niripip unlock
niripip reset

niripip preset tiny
niripip preset small
niripip preset medium
niripip preset large
niripip preset cinema
niripip preset movie
niripip preset study

niripip pin
niripip overlay
niripip overlay --profile study

niripip peek
niripip peek on
niripip peek off

niripip unpin
niripip toggle
```

Manual PiP resize remains authoritative. Presets are shortcuts, not restrictions.

### Hide / hidden windows

`niripip hide` moves the focused ordinary window to a dynamically named guarded service workspace with `focus=false`. The application stays running, but the window leaves normal workspace use until `restore-hidden`, `restore-all-hidden`, or a Settings restore button returns it. If the service workspace is focused accidentally, niri-pip immediately returns to the previous normal workspace.

This is deliberately described as **Hide**, not native minimize. Niri 26.04 does not expose a hidden/minimized-window IPC action, and niri-pip cannot intercept a client's Wayland `set_minimized` request before Niri receives it. The v0.3.0 names `minimize`, `restore-minimized`, `restore-all` and `minimized` remain compatibility aliases.

Hidden-window metadata survives a daemon restart inside the same Niri session. The stack is session-scoped, so stale live IDs are discarded after a Niri restart or reboot. Floating layout is left to Niri's exact per-workspace memory; tiled dimensions are replayed explicitly. The service workspace name is removed when the last managed hidden window is restored or closed.

### Sticky windows, overlays and peek

`pin` keeps the selected window sticky without changing its size. `overlay` also applies the
configured compact size and position:

```sh
niripip pin
niripip overlay
niripip overlay --profile study
```

For manually managed windows, niri-pip snapshots the origin before it starts moving the window.
`unpin` returns it to the original workspace and floating/tiling mode. Original size is restored when available; if the window was already floating, its normalized position is restored as well. Niri does not currently offer an ID-addressable action for restoring an exact tiled column index, so niri-pip deliberately does not focus-juggle or pretend that part can be restored reliably.

`peek` is temporary:

```sh
niripip peek
niripip peek on
niripip peek off
```

It enlarges and repositions the selected tracked window, then returns to the captured base geometry.
Peek geometry is never learned as normal PiP geometry. Size/position/lock/preset mutations are
rejected while peek is active so temporary state cannot accidentally become permanent.

Named profiles are ordinary `[profiles.NAME]` entries in `config.toml`. They reuse the same
small overlay model: width, height, position and follow policy.

### Media

When the browser exposes an MPRIS player and `playerctl` is installed:

```sh
niripip media play-pause
niripip media back 10
niripip media forward 10
niripip media volume-down 5
niripip media volume-up 5
```

Media control is optional and isolated from window management. A missing or ambiguous MPRIS player does not disable PiP tracking.

## Opacity and iNiR

iNiR commonly has a global rule that makes inactive windows slightly transparent. `niri-pip` does not edit that global rule. It installs one isolated include pointing to:

```text
~/.config/niri/niri-pip-runtime.kdl
```

`niripip opacity 100` keeps PiP fully opaque. `niripip opacity auto` removes the PiP override and lets normal Niri/iNiR rules apply again.

The integration installer backs up the touched user config, uses marker-scoped edits, runs `niri validate`, and rolls back if validation fails.

## Configuration

Main configuration:

```text
~/.config/niri-pip/config.toml
```

Example: [config/config.example.toml](config/config.example.toml)

Detailed reference: [docs/configuration.md](docs/configuration.md) · [Русский](docs/ru/configuration.md)

## Browser detection

The default scoring engine includes:

- Chromium PiP with `app-id=""`;
- Chromium/Chrome/Brave/Vivaldi/Edge identity matches;
- Firefox `Picture-in-Picture`;
- a conservative exact-title fallback.

Detector rules are configurable TOML regex rules. See [docs/browsers.md](docs/browsers.md) · [Русский](docs/ru/browsers.md).

## Architecture

```text
Niri EventStream
      │
      ▼
 niri IPC adapter ──────► typed compositor events
      │
      ▼
  policy engine ────────► detection / follow / geometry / focus policy
      │
      ├───────────────► Niri actions
      │
      ├───────────────► XDG state
      │
      └───────────────► Unix control socket
                               │
                               ▼
                          niripip CLI/menu
```

The daemon keeps compositor wire-format handling outside core policy so most behavior can be tested without launching Niri.

More: [docs/architecture.md](docs/architecture.md).

## Files written to your account

```text
~/.local/bin/niripip
~/.local/bin/niripipd
~/.local/bin/niripip-menu
~/.local/bin/niripip-integrate
~/.local/bin/niripip-unintegrate
~/.config/niri-pip/config.toml
~/.config/niri-pip/ui-language
~/.config/niri/niri-pip-runtime.kdl
~/.config/systemd/user/niripip.service
~/.local/share/applications/niri-pip.desktop
~/.local/state/niri-pip/state.json
```

A small marker-scoped include is also added to iNiR's `90-user-extra.kdl` when available, otherwise to the main Niri config.

## Uninstall

Keep config and remembered state:

```sh
./uninstall.sh
```

Remove everything owned by niri-pip:

```sh
./uninstall.sh --purge
```

The uninstaller removes only niri-pip's marker-scoped Niri integration and keeps timestamped backups.

## Known limitations

- Niri does not provide a normal floating-window API that guarantees staying above a focused true-fullscreen surface. `niri-pip` does not pretend to be a layer-shell overlay.
- Niri IPC does not expose a standalone calculated working-area rectangle, so corner positioning uses Niri work-area-relative moves plus configurable safety margins.
- Niri IPC does not expose a reliable XWayland/native flag for every window.
- `follow-focused-output` and `stay-on-output` are available, but complex multi-monitor layouts deserve testing on the target setup.
- Application-native minimize is a Niri/client interaction and cannot be intercepted by niri-pip; `hide` is a guarded service-workspace emulation, not native minimize.
- MPRIS support depends on the browser/site/player, not only on niri-pip.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --release --workspace
./scripts/release-preflight.sh
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Documentation

- [Configuration](docs/configuration.md)
- [Browsers](docs/browsers.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Architecture](docs/architecture.md)
- [Real-machine smoke test](docs/smoke-test.md)
- [Niri/iNiR integration](integrations/inir/README.md)
- [Русская документация](docs/ru/README.md)

## License

MIT. See [LICENSE](LICENSE).
