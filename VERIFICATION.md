# Verification

## v0.2.1 live verification

Date: 2026-08-16

Environment:

- Arch Linux
- Niri 26.04 (`8ed0da4`)
- Rust 1.97.1
- Cargo 1.97.1
- user systemd session
- Chromium-family browser PiP with `title="Picture in picture"` and empty Niri `app-id`

Build gates:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo build --release --workspace`

All gates passed.

Live checks:

- daemon starts under `systemd --user`;
- daemon Unix socket responds;
- `niripip doctor` reports zero problems;
- Chromium PiP with empty `app-id` is detected by `chromium-empty-app-id`;
- PiP follows workspace changes without a focus request;
- follow can be disabled and enabled at runtime;
- free-form manually chosen PiP geometry survives daemon restart;
- a stale remembered profile does not overwrite the geometry of an already-open PiP;
- geometry lock still intentionally enforces its frozen geometry;
- pixel nudge works;
- opacity runtime rule remains valid under `niri validate`;
- opacity can be changed and restored;
- generic Kitty pin/unpin works without applying PiP resize presets;
- state schema migration to v2 succeeds;
- controller smoke test restores original PiP size and toggles after completion.

The live acceptance run ended with:

```text
NIRI-PIP v0.2.1: ALL BUILD + LIVE CONTROLLER + RESTART-GEOMETRY TESTS PASSED
```

## Scope

This verification proves the tested Niri 26.04 single-session behavior above. It does not claim that every browser exposes MPRIS, that every multi-monitor topology behaves identically, or that an ordinary Niri floating window can render above focused true fullscreen.
