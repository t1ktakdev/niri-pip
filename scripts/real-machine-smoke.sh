#!/usr/bin/env bash
set -Eeuo pipefail

WORKSPACE_FOLLOW=false
if [[ "${1:-}" == "--workspace-follow" ]]; then
    WORKSPACE_FOLLOW=true
elif [[ $# -gt 0 ]]; then
    echo "usage: $0 [--workspace-follow]" >&2
    exit 2
fi

ok() { printf '[ OK ] %s\n' "$*"; }
warn() { printf '[WARN] %s\n' "$*"; }
fail() { printf '[FAIL] %s\n' "$*" >&2; exit 1; }

NIRIPIP="${NIRIPIP_BIN:-$HOME/.local/bin/niripip}"
[[ -x "$NIRIPIP" ]] || NIRIPIP="$(command -v niripip || true)"
[[ -n "$NIRIPIP" ]] || fail "niripip is not installed"
command -v niri >/dev/null 2>&1 || fail "niri is unavailable"
command -v jq >/dev/null 2>&1 || fail "jq is required for the real-machine smoke test"
[[ -n "${NIRI_SOCKET:-}" && -S "$NIRI_SOCKET" ]] || fail "run this inside the active Niri session"

TEST_ID=""
ORIG_OPACITY=""
ORIG_FOLLOW=""
ORIG_LOCK=""
ORIG_WS=""
MOVED_WS=false
CONTROLLER_TOUCHED=false

restore() {
    set +e
    if $MOVED_WS; then
        niri msg action focus-workspace-up >/dev/null 2>&1 || true
        MOVED_WS=false
        sleep 0.2
    fi
    if $CONTROLLER_TOUCHED && [[ -n "$TEST_ID" ]]; then
        if [[ "$ORIG_FOLLOW" == "true" ]]; then
            "$NIRIPIP" follow on --window-id "$TEST_ID" >/dev/null 2>&1 || true
        elif [[ "$ORIG_FOLLOW" == "false" ]]; then
            "$NIRIPIP" follow off --window-id "$TEST_ID" >/dev/null 2>&1 || true
        fi
        if [[ "$ORIG_LOCK" == "true" ]]; then
            "$NIRIPIP" lock --window-id "$TEST_ID" >/dev/null 2>&1 || true
        elif [[ "$ORIG_LOCK" == "false" ]]; then
            "$NIRIPIP" unlock --window-id "$TEST_ID" >/dev/null 2>&1 || true
        fi
    fi
    if $CONTROLLER_TOUCHED; then
        if [[ "$ORIG_OPACITY" == "null" ]]; then
            "$NIRIPIP" opacity auto >/dev/null 2>&1 || true
        elif [[ "$ORIG_OPACITY" =~ ^[0-9]+$ ]]; then
            "$NIRIPIP" opacity "$ORIG_OPACITY" >/dev/null 2>&1 || true
        fi
    fi
}
trap restore EXIT INT TERM

"$NIRIPIP" doctor
ok "doctor passed"

[[ "$("$NIRIPIP" --version)" == *"0.2.1"* ]] || fail "expected niri-pip 0.2.1"
ok "running v0.2.1"

RUNTIME="${XDG_CONFIG_HOME:-$HOME/.config}/niri/niri-pip-runtime.kdl"
[[ -f "$RUNTIME" ]] || fail "runtime KDL is missing: $RUNTIME"
grep -Rqs 'niri-pip-runtime.kdl' "${XDG_CONFIG_HOME:-$HOME/.config}/niri/config.d/90-user-extra.kdl" "${XDG_CONFIG_HOME:-$HOME/.config}/niri/config.kdl" 2>/dev/null \
    || fail "runtime KDL include is not installed"
niri validate >/dev/null
ok "runtime include + Niri validation passed"

STATE="${XDG_STATE_HOME:-$HOME/.local/state}/niri-pip/state.json"
[[ -f "$STATE" ]] || fail "state file is missing: $STATE"
SCHEMA="$(jq -r '.schema_version // 0' "$STATE")"
[[ "$SCHEMA" == "2" ]] || fail "expected state schema 2, got $SCHEMA"
ok "state schema 2 is durable"

[[ -x "$HOME/.local/bin/niripip-menu" || -n "$(command -v niripip-menu || true)" ]] \
    || fail "niripip-menu is not installed"
ok "controller menu installed"

STATUS="$($NIRIPIP --json status)"
ROW="$(jq -r '.data.windows[]? | select(.mode == "auto-pip") | [.id,.width,.height,.workspace_id,.follow_enabled,.geometry_locked] | @tsv' <<<"$STATUS" | head -n1)"
if [[ -z "$ROW" ]]; then
    warn "No auto PiP is open. Build/doctor/integration passed; live controller checks skipped."
    trap - EXIT INT TERM
    exit 0
fi

IFS=$'\t' read -r TEST_ID ORIG_W ORIG_H PIP_WS ORIG_FOLLOW ORIG_LOCK <<<"$ROW"
ORIG_OPACITY="$(jq -r '.data.opacity_override_percent' <<<"$STATUS")"
ORIG_WS="$(niri msg -j workspaces | jq -r '.[] | select(.is_focused == true) | .id' | head -n1)"
printf 'Live PiP: id=%s size=%sx%s workspace=%s follow=%s lock=%s opacity=%s\n' \
    "$TEST_ID" "$ORIG_W" "$ORIG_H" "$PIP_WS" "$ORIG_FOLLOW" "$ORIG_LOCK" "$ORIG_OPACITY"
CONTROLLER_TOUCHED=true

# Exercise exact-size protocol without changing the user's chosen dimensions.
"$NIRIPIP" size "$ORIG_W" "$ORIG_H" --window-id "$TEST_ID" >/dev/null
sleep 0.2
NOW="$($NIRIPIP --json status | jq -r --argjson id "$TEST_ID" '.data.windows[] | select(.id == $id) | [.width,.height] | @tsv')"
IFS=$'\t' read -r NOW_W NOW_H <<<"$NOW"
[[ "$NOW_W" == "$ORIG_W" && "$NOW_H" == "$ORIG_H" ]] \
    || fail "exact-size controller changed the saved size unexpectedly: ${NOW_W}x${NOW_H}"
ok "free-form exact size path works and preserved ${ORIG_W}x${ORIG_H}"

# Restart the daemon while the PiP is already open. The live compositor geometry must be
# adopted as authoritative; a stale remembered profile must never resize the existing window.
systemctl --user restart niripip.service
for _ in $(seq 1 60); do
    if "$NIRIPIP" --json status >/tmp/niripip-smoke-status.$$ 2>/dev/null; then
        RESTARTED_ROW="$(jq -r --argjson id "$TEST_ID" '.data.windows[]? | select(.id == $id) | [.width,.height] | @tsv' /tmp/niripip-smoke-status.$$)"
        [[ -n "$RESTARTED_ROW" ]] && break
    fi
    sleep 0.1
done
rm -f /tmp/niripip-smoke-status.$$
[[ -n "${RESTARTED_ROW:-}" ]] || fail "PiP was not re-adopted after daemon restart"
IFS=$'\t' read -r RESTART_W RESTART_H <<<"$RESTARTED_ROW"
[[ "$RESTART_W" == "$ORIG_W" && "$RESTART_H" == "$ORIG_H" ]] \
    || fail "daemon restart overwrote live geometry: expected ${ORIG_W}x${ORIG_H}, got ${RESTART_W}x${RESTART_H}"
ok "daemon restart adopts the already-open PiP geometry (${RESTART_W}x${RESTART_H})"

# Zero nudge exercises the command/selection path with no visual change.
"$NIRIPIP" nudge 0 0 --window-id "$TEST_ID" >/dev/null
ok "nudge controller path works"

# Verify opacity runtime rewrite, then restore the user's policy.
"$NIRIPIP" opacity 95 >/dev/null
sleep 0.15
grep -q 'opacity 0.95' "$RUNTIME" || fail "opacity 95 did not update runtime KDL"
niri validate >/dev/null
if [[ "$ORIG_OPACITY" == "null" ]]; then
    "$NIRIPIP" opacity auto >/dev/null
else
    "$NIRIPIP" opacity "$ORIG_OPACITY" >/dev/null
fi
ok "opacity controller rewrites a valid runtime rule and restores the previous value"

"$NIRIPIP" lock --window-id "$TEST_ID" >/dev/null
[[ "$($NIRIPIP --json status | jq -r --argjson id "$TEST_ID" '.data.windows[] | select(.id == $id) | .geometry_locked')" == "true" ]] \
    || fail "lock did not become active"
"$NIRIPIP" unlock --window-id "$TEST_ID" >/dev/null
[[ "$($NIRIPIP --json status | jq -r --argjson id "$TEST_ID" '.data.windows[] | select(.id == $id) | .geometry_locked')" == "false" ]] \
    || fail "unlock did not become active"
if [[ "$ORIG_LOCK" == "true" ]]; then "$NIRIPIP" lock --window-id "$TEST_ID" >/dev/null; fi
ok "geometry lock/unlock controller works"

if $WORKSPACE_FOLLOW; then
    "$NIRIPIP" follow off --window-id "$TEST_ID" >/dev/null
    [[ "$($NIRIPIP --json status | jq -r --argjson id "$TEST_ID" '.data.windows[] | select(.id == $id) | .follow_enabled')" == "false" ]] \
        || fail "follow off did not become active"

    niri msg action focus-workspace-down >/dev/null
    MOVED_WS=true
    sleep 0.45
    NEW_WS="$(niri msg -j workspaces | jq -r '.[] | select(.is_focused == true) | .id' | head -n1)"
    [[ -n "$NEW_WS" && "$NEW_WS" != "$ORIG_WS" ]] || fail "could not switch to a different workspace"
    STILL_WS="$($NIRIPIP --json status | jq -r --argjson id "$TEST_ID" '.data.windows[] | select(.id == $id) | .workspace_id')"
    [[ "$STILL_WS" == "$ORIG_WS" ]] || fail "PiP moved while follow was disabled (expected $ORIG_WS, got $STILL_WS)"
    ok "follow off keeps PiP on its workspace"

    "$NIRIPIP" follow on --window-id "$TEST_ID" >/dev/null
    for _ in $(seq 1 40); do
        CUR="$($NIRIPIP --json status | jq -r --argjson id "$TEST_ID" '.data.windows[] | select(.id == $id) | .workspace_id')"
        [[ "$CUR" == "$NEW_WS" ]] && break
        sleep 0.05
    done
    [[ "${CUR:-}" == "$NEW_WS" ]] || fail "follow on did not move PiP to workspace $NEW_WS"
    ok "follow on moves PiP to the focused workspace without a focus request"

    niri msg action focus-workspace-up >/dev/null
    MOVED_WS=false
    for _ in $(seq 1 40); do
        CUR="$($NIRIPIP --json status | jq -r --argjson id "$TEST_ID" '.data.windows[] | select(.id == $id) | .workspace_id')"
        [[ "$CUR" == "$ORIG_WS" ]] && break
        sleep 0.05
    done
    [[ "${CUR:-}" == "$ORIG_WS" ]] || fail "PiP did not follow back to original workspace $ORIG_WS"
    ok "workspace follow round-trip passed"
fi

# Restore original toggles explicitly before declaring success.
if [[ "$ORIG_FOLLOW" == "true" ]]; then
    "$NIRIPIP" follow on --window-id "$TEST_ID" >/dev/null
else
    "$NIRIPIP" follow off --window-id "$TEST_ID" >/dev/null
fi
if [[ "$ORIG_LOCK" == "true" ]]; then
    "$NIRIPIP" lock --window-id "$TEST_ID" >/dev/null
else
    "$NIRIPIP" unlock --window-id "$TEST_ID" >/dev/null
fi
CONTROLLER_TOUCHED=false
trap - EXIT INT TERM
ok "live v0.2 controller smoke passed; original PiP size/settings restored"
