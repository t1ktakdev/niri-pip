# Arch packaging

The `PKGBUILD` uses the immutable release tag through a Git VCS source. `SKIP` is used only because makepkg does not checksum VCS sources; the source is pinned to `v0.2.1`.

Build locally:

```sh
cd packaging/arch
makepkg -si
```

After package installation, each Niri user runs:

```sh
niripip-integrate
systemctl --user enable --now niripip.service
niripip doctor
```
