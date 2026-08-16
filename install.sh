#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
CONFIG_HOME="${XDG_CONFIG_HOME:-${HOME}/.config}"
CONFIG_DIR="${CONFIG_HOME}/niri-pip"
SYSTEMD_DIR="${CONFIG_HOME}/systemd/user"
SERVICE_PATH="${SYSTEMD_DIR}/niripip.service"
ALIAS_PATH="${SYSTEMD_DIR}/niripipd.service"
DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
APPLICATIONS_DIR="${DATA_HOME}/applications"
DESKTOP_PATH="${APPLICATIONS_DIR}/niri-pip.desktop"
BACKUP_TAG="$(date +%Y%m%d-%H%M%S)"
RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/niri-pip"

log() { printf '==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
backup_if_exists() {
    local path="$1"
    if [[ -e "$path" || -L "$path" ]]; then
        local backup="${path}.bak.${BACKUP_TAG}"
        cp -a -- "$path" "$backup"
        log "Backed up ${path} -> ${backup}"
    fi
}

[[ "${EUID}" -ne 0 ]] || die "do not run install.sh as root"
command -v systemctl >/dev/null 2>&1 || die "systemd user tools are required"
command -v niri >/dev/null 2>&1 || die "niri was not found in PATH"
command -v python >/dev/null 2>&1 || die "python is required (Arch: sudo pacman -S python)"

if [[ -x "${ROOT_DIR}/bin/niripip" && -x "${ROOT_DIR}/bin/niripipd" ]]; then
    CLI_SOURCE="${ROOT_DIR}/bin/niripip"
    DAEMON_SOURCE="${ROOT_DIR}/bin/niripipd"
    log "Using prebuilt niri-pip binaries"
else
    command -v cargo >/dev/null 2>&1 || die "cargo is required for a source install (Arch: sudo pacman -S rust)"
    command -v rustc >/dev/null 2>&1 || die "rustc is required for a source install"
    log "Building niri-pip 0.2.1 in release mode"
    cd "$ROOT_DIR"
    cargo build --release --workspace
    CLI_SOURCE="${ROOT_DIR}/target/release/niripip"
    DAEMON_SOURCE="${ROOT_DIR}/target/release/niripipd"
fi

log "Installing user binaries"
install -d -m 0755 "$BIN_DIR"
for name in niripip niripipd niripip-menu niripip-integrate niripip-unintegrate; do
    backup_if_exists "$BIN_DIR/$name"
done
install -m 0755 "$CLI_SOURCE" "$BIN_DIR/niripip"
install -m 0755 "$DAEMON_SOURCE" "$BIN_DIR/niripipd"
install -m 0755 "$ROOT_DIR/integrations/inir/niripip-menu" "$BIN_DIR/niripip-menu"
install -m 0755 "$ROOT_DIR/scripts/setup-niri-integration.sh" "$BIN_DIR/niripip-integrate"
install -m 0755 "$ROOT_DIR/scripts/remove-niri-integration.sh" "$BIN_DIR/niripip-unintegrate"

log "Installing default config"
install -d -m 0700 "$CONFIG_DIR"
if [[ ! -e "${CONFIG_DIR}/config.toml" ]]; then
    install -m 0600 "$ROOT_DIR/config/config.example.toml" "${CONFIG_DIR}/config.toml"
else
    log "Keeping existing ${CONFIG_DIR}/config.toml"
fi

if command -v fish >/dev/null 2>&1; then
    FISH_CONF_DIR="${CONFIG_HOME}/fish/conf.d"
    install -d -m 0755 "$FISH_CONF_DIR"
    cat > "${FISH_CONF_DIR}/niri-pip.fish" <<'FISH'
fish_add_path -m "$HOME/.local/bin"
FISH
fi

log "Installing Niri/iNiR runtime integration"
"$BIN_DIR/niripip-integrate"

log "Installing application launcher"
install -d -m 0755 "$APPLICATIONS_DIR"
backup_if_exists "$DESKTOP_PATH"
python - "$ROOT_DIR/packaging/desktop/niri-pip.desktop.in" "$DESKTOP_PATH" "$BIN_DIR/niripip" <<'PY'
from pathlib import Path
import sys

template = Path(sys.argv[1]).read_text()
out = Path(sys.argv[2])
exe = sys.argv[3].replace('\\', '\\\\').replace('"', '\\"')
out.write_text(template.replace('@NIRIPIP@', exe))
PY
chmod 0644 "$DESKTOP_PATH"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPLICATIONS_DIR" >/dev/null 2>&1 || true
fi

log "Installing systemd user service"
install -d -m 0755 "$SYSTEMD_DIR"
backup_if_exists "$SERVICE_PATH"
install -m 0644 "$ROOT_DIR/systemd/niripip.service" "$SERVICE_PATH"

if [[ -e "$ALIAS_PATH" || -L "$ALIAS_PATH" ]]; then
    if [[ ! -L "$ALIAS_PATH" || "$(readlink -- "$ALIAS_PATH")" != "niripip.service" ]]; then
        backup_if_exists "$ALIAS_PATH"
        rm -f -- "$ALIAS_PATH"
    fi
fi

systemctl --user daemon-reload
systemctl --user enable niripip.service >/dev/null
if systemctl --user cat niri.service >/dev/null 2>&1; then
    systemctl --user add-wants niri.service niripip.service >/dev/null
fi

if [[ -n "${NIRI_SOCKET:-}" && -S "${NIRI_SOCKET}" ]]; then
    systemctl --user import-environment NIRI_SOCKET WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP 2>/dev/null || true
    log "Starting niri-pip in the current Niri session"
    systemctl --user restart niripip.service

    for _ in {1..40}; do
        [[ -S "${RUNTIME_DIR}/niripip.sock" ]] && break
        sleep 0.1
    done

    printf '\n'
    "$BIN_DIR/niripip" doctor
else
    log "Service enabled; it will start automatically with the next Niri graphical session"
fi

printf '\n'
log "niri-pip 0.2.1 installed"
printf 'Controller: %s\n' "${BIN_DIR}/niripip menu"
printf 'Status:     %s\n' "${BIN_DIR}/niripip status"
printf 'Service:    systemctl --user status niripip.service\n'
