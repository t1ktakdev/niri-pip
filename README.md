# niri-pip

[![CI](https://github.com/t1ktakdev/niri-pip/actions/workflows/ci.yml/badge.svg)](https://github.com/t1ktakdev/niri-pip/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/t1ktakdev/niri-pip)](https://github.com/t1ktakdev/niri-pip/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**English** · [Русский](README.ru.md)

Sticky Picture-in-Picture and floating-window control for the Niri Wayland compositor.

`niri-pip` watches Niri's event stream, detects browser PiP windows, keeps them floating, follows the active workspace without stealing focus, remembers free-form geometry, and exposes a small controller for size, position, opacity, follow mode, locking and media keys.

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
- Optional MPRIS controls through `playerctl`.
- Compact controller using fuzzel, with rofi/gum fallbacks.
- English/Russian controller UI with a remembered language choice.
- systemd user service that starts with the graphical Niri session and restarts automatically.
- Safe iNiR integration through a separate runtime KDL file and marker-scoped include.

## Verified environment

The v0.2.1 core passed the full build and live-controller acceptance flow on Arch Linux with Niri 26.04 and Rust 1.97.1. The tested flow includes empty-app-id Chromium PiP detection, free-form geometry across daemon restart, workspace follow on/off, opacity changes, geometry lock/unlock, nudge, generic pin/unpin, daemon socket recovery and `niripip doctor` with zero problems.

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

## Controller

Open it from your application launcher as **niri-pip Controller**, or run:

```sh
niripip menu
```

The menu follows your locale automatically. Use **Language / Язык** inside the menu to force English or Russian; the choice is stored in `~/.config/niri-pip/ui-language`.

Suggested Niri shortcut:

```kdl
binds {
    Mod+Alt+P { spawn "niripip" "menu"; }
}
```

If iNiR already owns your `binds` layout, merge only the binding itself instead of creating a second conflicting block.

## CLI

```sh
niripip status
niripip list
niripip doctor

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
niripip unpin
niripip toggle
```

Manual PiP resize remains authoritative. Presets are shortcuts, not restrictions.

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
