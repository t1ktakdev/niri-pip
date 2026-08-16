#!/usr/bin/env bash
set -euo pipefail

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
NIRI_DIR="$CONFIG_HOME/niri"
RUNTIME_FILE="$NIRI_DIR/niri-pip-runtime.kdl"
STAMP="$(date +%Y%m%d-%H%M%S)"

for TARGET in "$NIRI_DIR/config.d/90-user-extra.kdl" "$NIRI_DIR/config.kdl"; do
  [[ -f "$TARGET" ]] || continue
  if grep -qE 'niri-pip (runtime include|opacity override)' "$TARGET"; then
    cp -a "$TARGET" "$TARGET.bak.niripip-remove.$STAMP"
  fi
  python - "$TARGET" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
blocks = [
    ("// >>> niri-pip runtime include >>>", "// <<< niri-pip runtime include <<<"),
    ("// >>> niri-pip opacity override >>>", "// <<< niri-pip opacity override <<<"),
]
changed = False
for start, end in blocks:
    while start in text and end in text:
        before, rest = text.split(start, 1)
        _, after = rest.split(end, 1)
        text = before.rstrip() + "\n" + after.lstrip("\n")
        changed = True
if changed:
    tmp = path.with_name(path.name + ".niripip-remove.tmp")
    tmp.write_text(text.rstrip() + "\n")
    tmp.replace(path)
PY
done

rm -f "$RUNTIME_FILE" "$RUNTIME_FILE.tmp" "${RUNTIME_FILE%.kdl}.kdl.tmp"

if command -v niri >/dev/null 2>&1 && [[ -n "${NIRI_SOCKET:-}" ]]; then
  niri validate || true
fi

echo "niri-pip runtime integration removed (marker-scoped edits only; timestamped backups kept)"
