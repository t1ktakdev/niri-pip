# Участие в разработке

[English](CONTRIBUTING.md)

Перед pull request запусти:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --release --workspace
./scripts/release-preflight.sh
```

Основная policy-логика должна оставаться в `niripip-core`, wire-format Niri — в `niripip-ipc`, а daemon orchestration — в `niripip-daemon`.

Новые Niri events/actions нельзя добавлять по предположению: сначала проверь актуальный upstream Niri IPC, затем добавь wire-shape test.

Для bug report приложи вывод:

```sh
niripip doctor
./scripts/collect-debug.sh
```

Перед публикацией debug-файла просмотри его содержимое.
