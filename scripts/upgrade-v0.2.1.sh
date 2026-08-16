#!/usr/bin/env bash
set -Eeuo pipefail

TS="$(date +%Y%m%d-%H%M%S)"
LOG="$HOME/niripip-v0.2.1-upgrade-$TS.log"
exec > >(tee -a "$LOG") 2>&1

ok()   { printf '\033[1;32m[ OK ]\033[0m %s\n' "$*"; }
info() { printf '\033[1;34m[....]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[WARN]\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31m[FAIL]\033[0m %s\n' "$*" >&2; exit 1; }

printf '\n=== niri-pip v0.2.1 full upgrade / acceptance ===\n'
printf 'Report: %s\n\n' "$LOG"

[[ "$EUID" -ne 0 ]] || fail "Run this as your normal user, not root."
command -v niri >/dev/null 2>&1 || fail "niri is not in PATH"
[[ -n "${NIRI_SOCKET:-}" ]] || fail "NIRI_SOCKET is unset. Run this terminal inside your active Niri session."
[[ -S "$NIRI_SOCKET" ]] || fail "NIRI_SOCKET is not a live Unix socket: $NIRI_SOCKET"

info "Niri session"
niri --version
printf 'NIRI_SOCKET=%s\n' "$NIRI_SOCKET"
printf 'SHELL=%s\n' "${SHELL:-<unset>}"
ok "active Niri IPC session detected"

# Keep the live Niri socket visible to the user systemd manager. No compositor config is changed here.
systemctl --user import-environment NIRI_SOCKET >/dev/null
for var in WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE; do
    [[ -n "${!var:-}" ]] && systemctl --user import-environment "$var" >/dev/null || true
done
ok "Niri environment imported into systemd --user"

info "Ensuring Arch build/test dependencies"
missing=0
for cmd in cargo rustc rustfmt cargo-clippy unzip jq python; do
    command -v "$cmd" >/dev/null 2>&1 || missing=1
done
if (( missing )); then
    command -v pacman >/dev/null 2>&1 || fail "Missing build tools and pacman is unavailable"
    echo "Some build tools are missing; sudo may ask for your password once."
    sudo -v
    pkgs=(base-devel unzip jq python)
    if pacman -Q rustup >/dev/null 2>&1; then
        sudo pacman -S --needed --noconfirm "${pkgs[@]}"
        rustup toolchain install 1.97.1 --profile minimal --component rustfmt clippy
    else
        pkgs+=(rust)
        sudo pacman -S --needed --noconfirm "${pkgs[@]}"
    fi
fi
for cmd in cargo rustc rustfmt cargo-clippy unzip jq python; do
    command -v "$cmd" >/dev/null 2>&1 || fail "$cmd is still unavailable"
done
printf 'cargo: '; cargo --version
printf 'rustc: '; rustc --version
ok "build/test dependencies ready"

# Capture the currently working v0.1 state before touching anything.
OLD_BIN="$HOME/.local/bin/niripip"
PRE_STATUS=""
PRE_PIP_ROW=""
if [[ -x "$OLD_BIN" ]]; then
    printf 'Installed before upgrade: '
    "$OLD_BIN" --version || true
    PRE_STATUS="$($OLD_BIN --json status 2>/dev/null || true)"
    if [[ -n "$PRE_STATUS" ]]; then
        PRE_PIP_ROW="$(jq -r '.data.windows[]? | select(.mode == "auto-pip") | [.id,.width,.height,.workspace_id] | @tsv' <<<"$PRE_STATUS" | head -n1)"
        if [[ -n "$PRE_PIP_ROW" ]]; then
            IFS=$'\t' read -r PRE_ID PRE_W PRE_H PRE_WS <<<"$PRE_PIP_ROW"
            printf 'Existing live PiP before upgrade: id=%s size=%sx%s workspace=%s\n' "$PRE_ID" "$PRE_W" "$PRE_H" "$PRE_WS"
        fi
    fi
fi

# Recover the user's pre-regression live size from the immediately previous failed v0.2.0
# acceptance log, but only if the window still has the exact erroneous post-failure size.
# If the user resized it since then, the current live geometry remains authoritative.
RECOVER_W=""
RECOVER_H=""
LATEST_V020_LOG="$(ls -t "$HOME"/niripip-v0.2-upgrade-*.log 2>/dev/null | head -n1 || true)"
if [[ -n "$PRE_PIP_ROW" && -n "$LATEST_V020_LOG" ]]; then
    FAILURE_LINE="$(grep -E 'manual PiP size was not preserved: before [0-9]+x[0-9]+, after [0-9]+x[0-9]+' "$LATEST_V020_LOG" | tail -n1 || true)"
    if [[ "$FAILURE_LINE" =~ before[[:space:]]+([0-9]+)x([0-9]+),[[:space:]]+after[[:space:]]+([0-9]+)x([0-9]+) ]]; then
        OLD_W="${BASH_REMATCH[1]}"; OLD_H="${BASH_REMATCH[2]}"
        BAD_W="${BASH_REMATCH[3]}"; BAD_H="${BASH_REMATCH[4]}"
        if [[ "$PRE_W" == "$BAD_W" && "$PRE_H" == "$BAD_H" ]]; then
            RECOVER_W="$OLD_W"; RECOVER_H="$OLD_H"
            printf 'Detected previous v0.2.0 geometry regression; will restore %sx%s after proving the core fix.\n' "$RECOVER_W" "$RECOVER_H"
        fi
    fi
fi

STATE="${XDG_STATE_HOME:-$HOME/.local/state}/niri-pip/state.json"
if [[ -f "$STATE" ]]; then
    STATE_BACKUP="$STATE.bak.pre-v0.2.1.$TS"
    cp -a "$STATE" "$STATE_BACKUP"
    printf 'State backup: %s\n' "$STATE_BACKUP"
fi
EXTRA="${XDG_CONFIG_HOME:-$HOME/.config}/niri/config.d/90-user-extra.kdl"
if [[ -f "$EXTRA" ]]; then
    EXTRA_BACKUP="$EXTRA.bak.pre-v0.2.1.$TS"
    cp -a "$EXTRA" "$EXTRA_BACKUP"
    printf 'Niri user-extra backup: %s\n' "$EXTRA_BACKUP"
fi
ok "pre-upgrade state captured"

info "Finding downloaded v0.2.1 source archive"
search_dirs=()
for dir in "$(xdg-user-dir DOWNLOAD 2>/dev/null || true)" "$HOME/Downloads" "$HOME/Загрузки" "$PWD"; do
    [[ -n "$dir" && -d "$dir" ]] && search_dirs+=("$dir")
done
(( ${#search_dirs[@]} > 0 )) || fail "Could not determine a Downloads directory"
ARCHIVE="$({ find "${search_dirs[@]}" -maxdepth 2 -type f \
    \( -iname 'niri-pip-v0.2.1-source*.zip' -o -iname 'niri-pip-v0.2.1-source*.tar.gz' -o -iname 'niri-pip-v0.2.1-source*.tgz' \) \
    -printf '%T@\t%p\n' 2>/dev/null || true; } | sort -nr | head -n1 | cut -f2-)"
[[ -n "$ARCHIVE" && -f "$ARCHIVE" ]] || fail "Download niri-pip-v0.2.1-source.zip first; no v0.2.1 archive was found"
printf 'Using: %s\n' "$ARCHIVE"
ok "v0.2.1 archive found"

WORK="$HOME/.local/src/niri-pip-v0.2-test-$TS"
mkdir -p "$WORK"
case "$ARCHIVE" in
    *.zip) unzip -q "$ARCHIVE" -d "$WORK" ;;
    *.tar.gz|*.tgz) tar -xzf "$ARCHIVE" -C "$WORK" ;;
    *) fail "Unsupported archive: $ARCHIVE" ;;
esac
PROJECT="$(find "$WORK" -maxdepth 3 -type f -name Cargo.toml -printf '%h\n' | head -n1)"
[[ -n "$PROJECT" && -f "$PROJECT/Cargo.toml" ]] || fail "Archive does not contain the Cargo workspace"
cd "$PROJECT"
printf 'Source: %s\n' "$PROJECT"
ok "source extracted"

info "Generating reproducible Cargo.lock"
cargo generate-lockfile
[[ -f Cargo.lock ]] || fail "Cargo.lock was not generated"
ok "Cargo.lock generated"

# The assistant sandbox has no Rust toolchain. Make the real machine the formatting source of truth,
# then enforce the exact gate used by CI.
info "rustfmt"
cargo fmt --all
cargo fmt --all --check
ok "cargo fmt --check passed"

info "clippy -D warnings"
cargo clippy --workspace --all-targets -- -D warnings
ok "cargo clippy passed"

info "unit + mock/controller acceptance tests"
cargo test --workspace --all-targets
ok "all Rust tests passed"

info "release build"
cargo build --release --workspace
ok "release build passed"

info "installing/upgrading v0.2.1"
./install.sh
export PATH="$HOME/.local/bin:$PATH"
[[ -x "$HOME/.local/bin/niripip" && -x "$HOME/.local/bin/niripipd" ]] || fail "binaries were not installed"
[[ "$($HOME/.local/bin/niripip --version)" == *"0.2.1"* ]] || fail "installed CLI is not v0.2.1"
ok "v0.2.1 installed and service restarted"

info "post-install doctor"
"$HOME/.local/bin/niripip" doctor
ok "doctor reports zero problems"

# The v0.1 manual opacity marker must be replaced by the v0.2.1 managed include, not stacked on top.
RUNTIME="${XDG_CONFIG_HOME:-$HOME/.config}/niri/niri-pip-runtime.kdl"
[[ -f "$RUNTIME" ]] || fail "runtime rule file missing"
if [[ -f "$EXTRA" ]]; then
    grep -q 'niri-pip runtime include' "$EXTRA" || fail "v0.2 runtime include marker missing from 90-user-extra.kdl"
    if grep -q 'niri-pip opacity override' "$EXTRA"; then
        fail "old v0.1 manual opacity marker still exists; installer should have migrated it"
    fi
fi
niri validate >/dev/null
ok "opacity integration migrated cleanly and Niri config remains valid"

[[ -f "$STATE" ]] || fail "state file missing after daemon start"
[[ "$(jq -r '.schema_version // 0' "$STATE")" == "2" ]] || fail "state did not migrate to schema 2"
ok "state schema migrated to v2"

# If the user's real PiP remained open, prove that v0.2.1 adopts the live compositor
# geometry instead of replaying an older state profile over the already-open window.
if [[ -n "$PRE_PIP_ROW" ]]; then
    for _ in $(seq 1 60); do
        POST_ROW="$($HOME/.local/bin/niripip --json status 2>/dev/null | jq -r --argjson id "$PRE_ID" '.data.windows[]? | select(.id == $id) | [.width,.height,.workspace_id] | @tsv' || true)"
        [[ -n "$POST_ROW" ]] && break
        sleep 0.1
    done
    [[ -n "${POST_ROW:-}" ]] || fail "the pre-existing PiP #$PRE_ID was not tracked after upgrade"
    IFS=$'\t' read -r POST_W POST_H POST_WS <<<"$POST_ROW"
    DW=$(( POST_W > PRE_W ? POST_W - PRE_W : PRE_W - POST_W ))
    DH=$(( POST_H > PRE_H ? POST_H - PRE_H : PRE_H - POST_H ))
    (( DW <= 2 && DH <= 2 )) || fail "v0.2.1 still overwrote live PiP geometry: before ${PRE_W}x${PRE_H}, after ${POST_W}x${POST_H}"
    ok "daemon startup adopted existing live PiP size (${POST_W}x${POST_H})"

    if [[ -n "$RECOVER_W" && -n "$RECOVER_H" ]]; then
        info "restoring the manual size captured before the v0.2.0 regression"
        "$HOME/.local/bin/niripip" size "$RECOVER_W" "$RECOVER_H" --window-id "$PRE_ID" >/dev/null
        sleep 0.25
        RESTORED="$($HOME/.local/bin/niripip --json status | jq -r --argjson id "$PRE_ID" '.data.windows[]? | select(.id == $id) | [.width,.height] | @tsv')"
        IFS=$'\t' read -r RESTORE_W RESTORE_H <<<"$RESTORED"
        [[ "$RESTORE_W" == "$RECOVER_W" && "$RESTORE_H" == "$RECOVER_H" ]] \
            || fail "could not restore pre-regression size ${RECOVER_W}x${RECOVER_H}; got ${RESTORE_W}x${RESTORE_H}"
        ok "restored your pre-regression PiP size ${RECOVER_W}x${RECOVER_H}"

        info "restart-persistence regression test"
        systemctl --user restart niripip.service
        for _ in $(seq 1 60); do
            RESTART_ROW="$($HOME/.local/bin/niripip --json status 2>/dev/null | jq -r --argjson id "$PRE_ID" '.data.windows[]? | select(.id == $id) | [.width,.height] | @tsv' || true)"
            [[ -n "$RESTART_ROW" ]] && break
            sleep 0.1
        done
        [[ -n "${RESTART_ROW:-}" ]] || fail "PiP was not tracked after explicit daemon restart"
        IFS=$'\t' read -r RESTART_W RESTART_H <<<"$RESTART_ROW"
        [[ "$RESTART_W" == "$RECOVER_W" && "$RESTART_H" == "$RECOVER_H" ]] \
            || fail "restart changed restored geometry: expected ${RECOVER_W}x${RECOVER_H}, got ${RESTART_W}x${RESTART_H}"
        ok "restored free-form geometry survives daemon restart"
    fi
fi

info "real v0.2.1 controller smoke"
./scripts/real-machine-smoke.sh --workspace-follow
ok "real v0.2.1 controller smoke passed"

info "final status"
"$HOME/.local/bin/niripip" status
systemctl --user --no-pager --full status niripip.service | sed -n '1,25p' || true

# Keep a fully formatted, lockfile-complete source snapshot produced by the machine that passed CI gates.
DOWNLOAD_DIR="$(xdg-user-dir DOWNLOAD 2>/dev/null || true)"
[[ -n "$DOWNLOAD_DIR" && -d "$DOWNLOAD_DIR" ]] || DOWNLOAD_DIR="$HOME/Downloads"
mkdir -p "$DOWNLOAD_DIR"
VERIFIED="$DOWNLOAD_DIR/niri-pip-v0.2.1-verified-$TS.tar.gz"
tar -C "$(dirname "$PROJECT")" -czf "$VERIFIED" "$(basename "$PROJECT")"
sha256sum "$VERIFIED" > "$VERIFIED.sha256"
ok "verified source snapshot created"
printf 'Verified source: %s\n' "$VERIFIED"
printf 'SHA256:         %s.sha256\n' "$VERIFIED"

printf '\n\033[1;32mNIRI-PIP v0.2.1: ALL BUILD + LIVE CONTROLLER + RESTART-GEOMETRY TESTS PASSED\033[0m\n'
printf 'Report: %s\n' "$LOG"
printf 'Test source kept at: %s\n' "$PROJECT"
printf '\nYour existing PiP geometry was not intentionally reset; live controller tests restore toggles/opacity.\n'
