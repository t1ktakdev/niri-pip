#!/usr/bin/env bash
set -euo pipefail

CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
NIRI_DIR="$CONFIG_HOME/niri"
INIR_EXTRA="$NIRI_DIR/config.d/90-user-extra.kdl"
MAIN_CONFIG="$NIRI_DIR/config.kdl"
RUNTIME_FILE="$NIRI_DIR/niri-pip-runtime.kdl"

if [[ -f "$INIR_EXTRA" ]]; then
  TARGET="$INIR_EXTRA"
elif [[ -f "$MAIN_CONFIG" ]]; then
  TARGET="$MAIN_CONFIG"
else
  echo "error: could not find $INIR_EXTRA or $MAIN_CONFIG" >&2
  exit 1
fi

mkdir -p "$NIRI_DIR"
touch "$TARGET"
BACKUP="$TARGET.bak.niripip.$(date +%Y%m%d-%H%M%S)"
cp -a "$TARGET" "$BACKUP"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ -x "$SCRIPT_DIR/niripip" ]]; then
  NIRIPIP_BIN="$SCRIPT_DIR/niripip"
elif command -v niripip >/dev/null 2>&1; then
  NIRIPIP_BIN="$(command -v niripip)"
else
  NIRIPIP_BIN="niripip"
fi

python - "$TARGET" "$RUNTIME_FILE" "$MAIN_CONFIG" "$NIRIPIP_BIN" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
runtime = sys.argv[2]
main_config = Path(sys.argv[3])
niripip_bin = sys.argv[4]
runtime_kdl = runtime.replace("\\", "\\\\").replace('"', '\\"')
niripip_kdl = niripip_bin.replace("\\", "\\\\").replace('"', '\\"')
text = path.read_text()

blocks = [
    ("// >>> niri-pip opacity override >>>", "// <<< niri-pip opacity override <<<"),
    ("// >>> niri-pip runtime include >>>", "// <<< niri-pip runtime include <<<"),
    ("// >>> niri-pip hide shortcuts >>>", "// <<< niri-pip hide shortcuts <<<"),
]
for start, end in blocks:
    while start in text and end in text:
        before, rest = text.split(start, 1)
        _, after = rest.split(end, 1)
        text = before.rstrip() + "\n" + after.lstrip("\n")

def include_target(line: str):
    line = line.strip()
    if line.startswith("//") or line.startswith("/-") or not line.startswith("include "):
        return None
    first = line.find('"')
    if first < 0:
        return None
    second = line.find('"', first + 1)
    if second < 0:
        return None
    return line[first + 1:second]

def active_lines(config: Path):
    visited = set()
    out = []

    def walk(current: Path):
        resolved = current.expanduser().resolve(strict=False)
        if resolved in visited:
            return
        visited.add(resolved)
        if resolved == path.resolve(strict=False):
            data = text
        else:
            try:
                data = current.expanduser().read_text()
            except OSError:
                return
        out.extend(data.splitlines())
        parent = current.expanduser().parent
        for raw in data.splitlines():
            target = include_target(raw)
            if not target:
                continue
            included = Path(target).expanduser()
            if not included.is_absolute():
                included = parent / included
            walk(included)

    walk(config if config.exists() else path)
    return out

def normalize_bind(token: str):
    parts = [part for part in token.split("+") if part]
    if len(parts) < 2:
        return token
    key = parts[-1]
    modifiers = ["Super" if part == "Mod" else part for part in parts[:-1]]
    order = {"Super": 0, "Ctrl": 1, "Alt": 2, "Shift": 3}
    modifiers.sort(key=lambda item: (order.get(item, 99), item))
    return "+".join([*modifiers, key])

def bind_status(bind: str, accepted_commands):
    wanted = normalize_bind(bind)
    ours = False
    conflict = False
    for raw in active_lines(main_config):
        line = raw.strip()
        if not line or line.startswith("//") or line.startswith("/-"):
            continue
        token = line.split(None, 1)[0]
        if normalize_bind(token) != wanted:
            continue
        is_ours = "niripip" in line and any(f'"{command}"' in line for command in accepted_commands)
        ours = ours or is_ours
        conflict = conflict or not is_ours
    return ours, conflict

bindings = [
    ("Mod+Alt+M", "hide", ("hide", "minimize")),
    ("Mod+Alt+Shift+M", "restore-hidden", ("restore-hidden", "restore-minimized")),
]
new_bindings = []
for bind, command, accepted in bindings:
    installed, conflict = bind_status(bind, accepted)
    if conflict:
        print(f"warning: {bind} is already used; niri-pip will not overwrite it", file=sys.stderr)
    elif installed:
        print(f"niri-pip shortcut already present: {bind}")
    else:
        new_bindings.append((bind, command))

runtime_block = f'''
// >>> niri-pip runtime include >>>
// niri-pip runtime rules
include optional=true "{runtime_kdl}"
// <<< niri-pip runtime include <<<
'''

def binds_block_end(source: str):
    import re

    match = re.search(r"(?m)^[ \t]*binds[ \t]*\{", source)
    if not match:
        return None
    opening = source.find("{", match.start())
    depth = 0
    in_string = False
    escaped = False
    in_comment = False
    index = opening
    while index < len(source):
        char = source[index]
        next_char = source[index + 1] if index + 1 < len(source) else ""

        if in_comment:
            if char == "\n":
                in_comment = False
            index += 1
            continue

        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue

        if char == "/" and next_char == "/":
            in_comment = True
            index += 2
            continue
        if char == '"':
            in_string = True
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
        index += 1
    return None

def inject_shortcuts(source: str, bindings_to_add):
    if not bindings_to_add:
        return source

    rows = "\n".join(
        f'    {bind} repeat=false {{ spawn "{niripip_kdl}" "{command}"; }}'
        for bind, command in bindings_to_add
    )
    marker = (
        "    // >>> niri-pip hide shortcuts >>>\n"
        "    // Hide windows without conflicting with common maximize/mute bindings.\n"
        f"{rows}\n"
        "    // <<< niri-pip hide shortcuts <<<\n"
    )

    end = binds_block_end(source)
    if end is None:
        return (
            source.rstrip()
            + "\n\nbinds {\n"
            + marker
            + "}\n"
        )

    prefix = source[:end]
    if not prefix.endswith("\n"):
        prefix += "\n"
    return prefix + marker + source[end:]

text = inject_shortcuts(text, new_bindings)

tmp = path.with_name(path.name + ".niripip.tmp")
tmp.write_text(text.rstrip() + "\n\n" + runtime_block.lstrip())
tmp.replace(path)
PY

if [[ ! -f "$RUNTIME_FILE" ]]; then
  cat > "$RUNTIME_FILE" <<'KDL'
// Generated by niri-pip. Do not edit while niripipd is running.
window-rule {
    match title=r#"(?i)^picture(?:[ -]?in[ -]?)picture$"#
    opacity 1.00
}
KDL
  chmod 600 "$RUNTIME_FILE"
fi

if command -v niri >/dev/null 2>&1 && [[ -n "${NIRI_SOCKET:-}" ]]; then
  if ! niri validate; then
    echo "error: Niri rejected the integration; restoring $BACKUP" >&2
    cp -a "$BACKUP" "$TARGET"
    niri validate || true
    exit 1
  fi
fi

echo "niri-pip Niri integration installed"
echo "  include: $TARGET"
echo "  runtime: $RUNTIME_FILE"
echo "  backup:  $BACKUP"
