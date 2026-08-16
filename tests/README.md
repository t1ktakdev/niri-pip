# Test strategy

The executable tests live with the Rust crates so Cargo discovers them naturally.

- `niripip-core` unit tests cover config parsing/validation, detector scoring (including Chromium
  PiP with empty or missing `app_id`), false-positive rejection, geometry, workspace-follow state,
  duplicate metadata, late metadata, focus restoration, close/reopen, disable behavior, manual
  geometry retention, transient disconnects, and persistent-state schema handling.
- `niripip-ipc` unit tests cover tolerant Niri event parsing and exact JSON encoding of the
  v26.04 action shapes used by the daemon.
- `crates/niripip-ipc/tests/mock_backend.rs` checks the `NiriBackend` mock seam.
- `crates/niripip-ipc/tests/acceptance.rs` is a compositor-free MVP flow: empty-app-id Chromium
  PiP -> float/size -> workspace follow with `focus=false` -> close cleanup -> Kitty pin/unpin.

A real Chrome/Firefox compositor smoke test is intentionally separate; see
`docs/smoke-test.md`. It must be run inside an actual Niri session before claiming browser support
as release-verified.
