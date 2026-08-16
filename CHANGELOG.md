# Changelog

All notable changes are documented here. The project follows Semantic Versioning.

## [Unreleased]

## [0.2.1] - 2026-08-16

### Fixed

- Already-open PiP windows discovered during Niri's authoritative bootstrap now adopt their live compositor geometry instead of replaying stale remembered dimensions.
- Explicit geometry lock remains authoritative and still restores the frozen size and position.
- Free-form live geometry is persisted back to the detector profile during bootstrap.

### Added

- Exact free-form sizing and relative scaling.
- Position presets and pixel nudging.
- Runtime workspace-follow on/off and follow-mode control.
- Geometry lock/unlock and control reset.
- PiP-specific opacity control through an isolated Niri runtime KDL include.
- Optional MPRIS media commands through `playerctl`.
- Compact fuzzel controller with rofi/gum fallbacks.
- English/Russian controller UI with remembered language selection.
- English and Russian user documentation.
- Desktop application launcher.
- Source/prebuilt dual-mode installer.
- Latest-release installer with SHA-256 verification.
- Persistent systemd user service with automatic restart during the Niri graphical session.
- GitHub CI, release asset generation, Dependabot, issue templates and release preflight tooling.
- Arch packaging files.
- Live verification matrix for Niri 26.04.

### Changed

- Manual PiP resizing is authoritative unless geometry lock is enabled.
- Controller commands prefer the single tracked auto PiP when keyboard focus is elsewhere.
- Default PiP opacity is 100%; `opacity auto` restores normal Niri/iNiR opacity behavior.
- Daemon control protocol is version 2.

## [0.1.0] - 2026-08-16

### Added

- Event-driven Niri IPC backend with full EventStream bootstrap.
- Scoring detector engine including Chromium empty-app-id and Firefox PiP defaults.
- Floating management, aspect-aware initial sizing, corner placement and focus-safe workspace following.
- Generic manual pin/unpin/toggle behavior.
- Unix-socket daemon protocol and human/JSON CLI output.
- XDG config and atomic user-only runtime state.
- Bounded focus compensation for newly mapped PiP windows.
- PID-scoped Niri socket restart handling and reconnect backoff.
- systemd user service, source installer and Arch package template.
- Mock backend and compositor-free acceptance tests.
