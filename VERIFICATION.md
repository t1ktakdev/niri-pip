# Verification

## v0.3.1 verification

Date: 2026-09-20

Environment:

- Arch Linux
- Niri 26.04 (`8ed0da4`)
- Rust 1.97.1
- user systemd session

### Build and release gates

The full release preflight completed successfully after the Hide hotfix work, including shell/TOML/desktop validation, documentation checks, package/version consistency, systemd verification, `cargo fmt --all --check`, clippy with warnings denied, the complete workspace test suite and a locked release build.

Final test totals:

- CLI: 5 passed;
- core: 50 passed;
- daemon: 5 passed;
- IPC unit tests: 6 passed;
- compositor-free acceptance: 3 passed;
- mock backend: 1 passed;
- settings UI backend: 7 passed.

That is **77 passing tests** across the workspace. New regression coverage includes Hide CLI aliases, guarded service-workspace focus, exact Niri `FocusWorkspace` IPC encoding, shortcut conflict normalization and active-include parsing.

The release preflight also contains a temporary-config regression for the Niri integration installer. It verifies that an existing `binds {}` block is reused rather than duplicated, installation is idempotent, existing user bindings survive install/remove, and Hide shortcut markers are removed cleanly.

### Hide / restore live acceptance

The installed v0.3.1 build was exercised against a real disposable tiled Alacritty window.

Verified:

- the window started on workspace 1 at exactly 922x1030;
- `niripip hide` moved it to the dynamically named `niri-pip:scratchpad` workspace with no focus transfer;
- the Alacritty process remained alive while hidden;
- the focused normal workspace remained workspace 1;
- deliberately focusing `niri-pip:scratchpad` caused the daemon guard to immediately return to workspace 1;
- `niripip restore-hidden --window-id ...` returned the window to workspace 1 as tiled at exactly 922x1030;
- the hidden stack returned to zero and the scratchpad workspace name disappeared;
- the disposable window was closed and the user's previous focus was restored.

This confirms the v0.3.1 behavior is a guarded Hide emulation: the application remains running while the window leaves normal workspace use. It is not represented as native Wayland minimize.

### Settings and shortcut acceptance

The real installed `niripip-ui --no-open` server was exercised while a disposable window was hidden.

Verified:

- Settings reported `Mod+Alt+M` / `niripip hide` as installed with no conflict;
- Settings reported `Mod+Alt+Shift+M` / `niripip restore-hidden` as installed with no conflict;
- the hidden window appeared in the Settings bootstrap list;
- the per-window restore endpoint returned that specific window and reduced the hidden list to zero.

The real user's Niri configuration was also checked after installation:

- existing `Super+M` maximize remained unchanged;
- existing `Mod+Shift+M` mute remained unchanged;
- Hide and Restore were added under marker comments inside the existing user `binds {}` block;
- `niri validate` passed;
- `niripip doctor` reported 0 problems.

### Installer rollback finding

During live acceptance, the first v0.3.1 integration attempt exposed a duplicate-`binds` bug in the installer. Niri rejected the generated config and the installer correctly restored the timestamped backup automatically, leaving the user's config intact. The installer was then fixed to inject shortcut rows inside an existing `binds {}` block, and the corrected path passed temporary-config validation, idempotence/removal checks and the real Niri install described above.

## v0.3.0 verification

Date: 2026-09-19

Environment:

- Arch Linux
- Niri 26.04 (`8ed0da4`)
- Rust 1.97.1
- user systemd session
- Google Chrome Flatpak for the settings-app browser acceptance checks

### Build and repository gates

The release preflight completed successfully with:

- shell syntax validation;
- TOML parsing;
- desktop-entry structure validation;
- placeholder/internal-marker scan;
- local Markdown link validation;
- package/version consistency;
- systemd unit verification;
- `cargo fmt --all --check`;
- `cargo clippy --workspace --all-targets --locked -- -D warnings`;
- `cargo test --workspace --all-targets --locked`;
- `cargo build --release --workspace --locked`.

Test totals in the final preflight:

- CLI: 4 passed;
- core: 49 passed;
- daemon: 5 passed;
- IPC unit tests: 5 passed;
- compositor-free acceptance: 3 passed;
- mock backend: 1 passed;
- settings UI backend: 5 passed.

That is 72 passing tests across the workspace. The daemon tests include injected partial-failure cases for both minimize and restore, verifying that persistent state is not committed on failure and compositor compensation is attempted. Additional regression coverage verifies Niri-session scoping of minimized live IDs and prevents scratchpad creation on a workspace that already contains a window.

### Settings UI acceptance

The new `niripip ui` implementation was exercised against a temporary XDG config/state and a real Niri IPC session before installation.

Verified:

- the UI renders as a settings application with General, Behavior, Integration, Shortcuts and About pages;
- Russian/English switching works without a reload;
- the language preference survives a complete UI-server restart on a different localhost port;
- changing `remember_geometry` through the browser updates the temporary TOML and is accepted by the daemon;
- changing opacity through the browser updates persistent state and is visible through `niripip status`;
- settings writes preserve unrelated manual TOML sections;
- minimize toggles materialize a missing `[minimize]` table correctly; a live `true -> false -> true` save was verified in the real user config with successful daemon reload after each write;
- failed settings application rolls the file back instead of leaving a partial configuration;
- the local server is bound to loopback and API requests require the per-session token;
- a heartbeat keeps the ephemeral UI server alive while the settings window remains open.

Earlier live Niri acceptance in the same v0.3 worktree also verified the manual control state machine:

- a test window was converted to a 520x340 overlay;
- temporary peek changed it to 960x540;
- leaving peek restored 520x340;
- unpin restored the original workspace, tiled state and 922x1030 size.

### Scratchpad minimize acceptance

The final build was installed and then exercised against two real Alacritty windows through the live Niri 26.04 IPC session.

The first window was tiled at 700x600. The second was floating at 640x420 with compositor position `[1210,342]`.

Verified:

- both windows were minimized into the dynamically named `niri-pip:scratchpad` workspace;
- daemon status and `state.json` reported exactly two minimized windows with state schema 3;
- minimized live IDs are bound to a Niri-session key derived from the Linux boot ID and `NIRI_SOCKET`;
- restarting `niripip.service` preserved both minimized entries and the same session key;
- changing the Niri-session key is regression-tested to clear stale minimized IDs before engine startup;
- `restore-minimized` restored the most recently minimized floating window first;
- the floating window returned to the original workspace with exactly 640x420 size and `[1210,342]` position;
- the tiled window remained parked while the floating window was restored;
- `restore-all` restored the remaining tiled window to the original workspace and exactly 700x600;
- restore focus behavior worked for both restore paths;
- after the final restore the scratchpad workspace name disappeared;
- daemon status and persisted state both returned to zero minimized windows;
- the disposable test windows were closed and the previously focused user window was restored.

Separate live testing also showed why restore is intentionally hybrid: Niri preserves floating size/position exactly across a workspace round-trip, while tiled height can be lost. niri-pip therefore leaves floating geometry to Niri but explicitly restores captured tiled dimensions.

### Installed-user path

The real `./install.sh` path was then run in the active Niri session.

Verified:

- existing `~/.config/niri-pip/config.toml` was preserved;
- previous binaries, desktop entry and user service received timestamped backups;
- `niripip`, `niripipd` and `niripip-ui` 0.3.0 were installed;
- the desktop entry launches `niripip ui`;
- `niripip-ui --help` returns without starting a server;
- `niripip.service` is active;
- `niripip doctor` reports 0 problems;
- Niri IPC reports 26.04 (`8ed0da4`);
- the installed `niripip ui` opened a real `niri-pip — Settings` window on workspace 1 as a separate app-mode browser process;
- the Settings window was arranged through Niri IPC as floating at the preferred ~1020×720 starting size while remaining freely resizable.

The final real-machine smoke reported:

- doctor passed with 0 problems;
- running v0.3.0;
- settings UI binary installed;
- runtime include + `niri validate` passed;
- state schema 3 is durable;
- compact controller installed.

No auto-detected PiP window was open during that final smoke invocation, so the script intentionally skipped its destructive live auto-PiP geometry/follow branch rather than fabricating a result. Those state-machine paths remain covered by the compositor-free acceptance suite and the earlier live manual overlay/peek/restore acceptance described above.

## Scope

This verification is specific to the tested Arch Linux + Niri 26.04 session. Browser PiP metadata can vary by browser build, MPRIS is optional, and Niri does not currently provide every possible exact tiled-layout restoration primitive.
