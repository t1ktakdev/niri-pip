#!/usr/bin/env bash
set -euo pipefail

REPO="t1ktakdev/niri-pip"
BASE="https://github.com/${REPO}/releases/latest/download"

[[ "${EUID}" -ne 0 ]] || { echo "error: run as your normal user, not root" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "error: curl is required" >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { echo "error: tar is required" >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { echo "error: sha256sum is required" >&2; exit 1; }

case "$(uname -m)" in
  x86_64|amd64) ARCH="x86_64" ;;
  *) echo "error: prebuilt release is currently available for x86_64 only; use the source installer" >&2; exit 1 ;;
esac

ASSET="niri-pip-linux-${ARCH}.tar.gz"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

printf 'Downloading %s...\n' "$ASSET"
curl -fL --retry 3 --connect-timeout 15 -o "$TMP/$ASSET" "$BASE/$ASSET"
curl -fL --retry 3 --connect-timeout 15 -o "$TMP/$ASSET.sha256" "$BASE/$ASSET.sha256"

(
  cd "$TMP"
  sha256sum -c "$ASSET.sha256"
)

tar -xzf "$TMP/$ASSET" -C "$TMP"
DIR="$TMP/niri-pip-linux-${ARCH}"
[[ -x "$DIR/install.sh" ]] || { echo "error: invalid release bundle" >&2; exit 1; }

exec "$DIR/install.sh"
