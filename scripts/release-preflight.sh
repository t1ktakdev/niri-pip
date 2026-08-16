#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
SKIP_RUST=false
[[ "${1:-}" == "--skip-rust" ]] && SKIP_RUST=true

ok() { printf '[OK] %s\n' "$*"; }
fail() { printf '[FAIL] %s\n' "$*" >&2; exit 1; }

for file in install.sh uninstall.sh integrations/inir/niripip-menu scripts/*.sh packaging/arch/PKGBUILD packaging/arch/niri-pip.install; do
    bash -n "$file"
done
ok "shell syntax"

python - <<'PY'
from pathlib import Path
import tomllib

for path in Path('.').rglob('*.toml'):
    if 'target' in path.parts:
        continue
    tomllib.loads(path.read_text())
print('toml ok')
PY
ok "TOML parse"

python - <<'PY'
from pathlib import Path

p = Path('packaging/desktop/niri-pip.desktop.in')
text = p.read_text()
required = ['[Desktop Entry]', 'Type=Application', 'Name=niri-pip Controller', 'Exec=', 'Terminal=false']
missing = [x for x in required if x not in text]
if missing:
    raise SystemExit('desktop entry missing: ' + ', '.join(missing))
PY
ok "desktop entry structure"

python - <<'PYSCAN'
from pathlib import Path
import re

root = Path('.')
checks = [
    re.compile(r'\bOWNER\b'),
    re.compile(r'\bTODO\b'),
    re.compile(r'\bFIXME\b'),
    re.compile(r'\bTBD\b'),
    re.compile(r'Replace with the checksum', re.I),
]
ignore = {Path('scripts/release-preflight.sh')}
for path in root.rglob('*'):
    if not path.is_file() or '.git' in path.parts or 'target' in path.parts or path in ignore:
        continue
    try:
        text = path.read_text()
    except UnicodeDecodeError:
        continue
    for pattern in checks:
        m = pattern.search(text)
        if m:
            raise SystemExit(f'{path}: forbidden marker: {m.group(0)}')
PYSCAN
ok "placeholder/internal-marker scan"

grep -q '\[Русский\](README.ru.md)' README.md || fail "README.md has no Russian language switch"
grep -q '\[English\](README.md)' README.ru.md || fail "README.ru.md has no English language switch"
ok "language navigation"

python - <<'PYLINKS'
from pathlib import Path
import re

link_re = re.compile(r'\[[^\]]*\]\(([^)]+)\)')
errors = []
for path in Path('.').rglob('*.md'):
    if '.git' in path.parts or 'target' in path.parts:
        continue
    text = path.read_text()
    for target in link_re.findall(text):
        target = target.strip().split('#', 1)[0]
        if not target or target.startswith(('http://', 'https://', 'mailto:')):
            continue
        target = target.split('?', 1)[0]
        resolved = (path.parent / target).resolve()
        if not resolved.exists():
            errors.append(f'{path}: {target}')
if errors:
    raise SystemExit('broken local markdown links:\n' + '\n'.join(errors))
PYLINKS
ok "local documentation links"

VERSION="$(python - <<'PY'
import tomllib
print(tomllib.load(open('Cargo.toml','rb'))['workspace']['package']['version'])
PY
)"
grep -q "pkgver=${VERSION}" packaging/arch/PKGBUILD || fail "PKGBUILD version mismatch"
grep -q "pkgver = ${VERSION}" packaging/arch/.SRCINFO || fail ".SRCINFO version mismatch"
grep -q "## \[${VERSION}\]" CHANGELOG.md || fail "CHANGELOG version mismatch"
ok "version consistency (${VERSION})"

if command -v systemd-analyze >/dev/null 2>&1; then
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    cp systemd/niripip.service "$tmp/niripip.service"
    sed -i \
      -e 's#ExecStart=.*#ExecStart=/bin/true#' \
      -e 's#ExecReload=.*#ExecReload=/bin/true#' \
      -e '/^PartOf=/d' \
      -e '/^After=/d' \
      -e '/^Requisite=/d' \
      -e '/^ConditionEnvironment=/d' \
      "$tmp/niripip.service"
    systemd-analyze verify "$tmp/niripip.service" >/dev/null
    rm -rf "$tmp"
    trap - EXIT
    ok "systemd unit verification"
fi

if ! $SKIP_RUST; then
    command -v cargo >/dev/null 2>&1 || fail "cargo is required unless --skip-rust is used"
    [[ -f Cargo.lock ]] || cargo generate-lockfile
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --all-targets --locked
    cargo build --release --workspace --locked
    ok "Rust format/clippy/test/release build"
fi

ok "release preflight complete"
