#!/usr/bin/env bash
set -euo pipefail

PURGE=false
if [[ "${1:-}" == "--purge" ]]; then
    PURGE=true
elif [[ $# -gt 0 ]]; then
    printf 'usage: %s [--purge]\n' "$0" >&2
    exit 2
fi

[[ "${EUID}" -ne 0 ]] || { echo "error: do not run uninstall.sh as root" >&2; exit 1; }

BIN_DIR="${HOME}/.local/bin"
CONFIG_HOME="${XDG_CONFIG_HOME:-${HOME}/.config}"
STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
SYSTEMD_DIR="${CONFIG_HOME}/systemd/user"
DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
APPLICATIONS_DIR="${DATA_HOME}/applications"

if command -v systemctl >/dev/null 2>&1; then
    systemctl --user disable --now niripip.service >/dev/null 2>&1 || true
    systemctl --user disable --now niripipd.service >/dev/null 2>&1 || true
fi

if [[ -x "${BIN_DIR}/niripip-unintegrate" ]]; then
    "${BIN_DIR}/niripip-unintegrate" || true
elif [[ -x "$(dirname "$0")/scripts/remove-niri-integration.sh" ]]; then
    "$(dirname "$0")/scripts/remove-niri-integration.sh" || true
fi

rm -f "${SYSTEMD_DIR}/niripip.service" "${SYSTEMD_DIR}/niripipd.service"
rm -f "${SYSTEMD_DIR}/niri.service.wants/niripip.service"
rm -f "${SYSTEMD_DIR}/graphical-session.target.wants/niripip.service"
rm -f \
    "${BIN_DIR}/niripip" \
    "${BIN_DIR}/niripipd" \
    "${BIN_DIR}/niripip-menu" \
    "${BIN_DIR}/niripip-integrate" \
    "${BIN_DIR}/niripip-unintegrate"
rm -f "${CONFIG_HOME}/fish/conf.d/niri-pip.fish"
rm -f "${APPLICATIONS_DIR}/niri-pip.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPLICATIONS_DIR" >/dev/null 2>&1 || true
fi

if command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload
fi

if $PURGE; then
    rm -rf "${CONFIG_HOME}/niri-pip" "${STATE_HOME}/niri-pip"
    echo "Removed niri-pip binaries, service, runtime integration, config and state."
else
    echo "Removed binaries, service and runtime integration. Kept config/state; use --purge to remove them."
fi
