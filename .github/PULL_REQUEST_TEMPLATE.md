## Summary

Describe the behavior change and why it is needed.

## Verification

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] `./scripts/release-preflight.sh`
- [ ] Niri live smoke test when compositor behavior changed

## Compatibility

Note any Niri IPC, browser, iNiR, multi-monitor or state-schema impact.
