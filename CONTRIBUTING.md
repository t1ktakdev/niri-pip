# Contributing

Thanks for helping improve niri-pip.

## Development

Use stable Rust and Niri 26.04+ for compositor smoke tests.

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

Core policy must remain testable without launching a compositor. Put window/workspace state-machine
logic in `niripip-core`; put wire-format details in `niripip-ipc`.

## IPC changes

Do not guess action/event names. When changing the Niri adapter:

1. verify current Niri wiki documentation;
2. verify `niri-ipc/src/lib.rs` / `state.rs` upstream;
3. update `docs/research.md` with the research date and compatibility impact;
4. add a wire-shape test for any new request/action;
5. keep unknown future event variants non-fatal where safe.

## Pull requests

Keep PRs focused. Include tests for behavior changes and run the full command set above.

For PiP bugs, include:

```sh
niripip doctor
./scripts/collect-debug.sh
```

Review the collected file before attaching it. The collector intentionally redacts window titles.

## Code style

- no `unwrap()` in runtime I/O paths unless an invariant is locally proven;
- prefer typed errors with actionable context;
- avoid shelling out to `niri msg` in the daemon;
- never add background polling when an event/reconciliation trigger exists;
- do not edit a user's Niri/iNiR configuration from the daemon.
