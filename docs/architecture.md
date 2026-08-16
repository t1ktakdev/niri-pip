# Architecture

## Goals

`niri-pip` is a Niri-first, event-driven user-session daemon that classifies Picture-in-Picture windows and emulates sticky windows by moving tracked windows to the globally focused workspace. It also exposes generic pin/unpin operations for arbitrary windows.

The design optimizes for:

- no polling loop for window/workspace state;
- no focus stealing for normal management;
- explicit handling of late metadata and event reordering;
- a small, version-tolerant Niri JSON IPC boundary;
- testable policy code independent of a real compositor;
- user-only IPC and state;
- safe installation that does not edit Niri or iNiR configuration.

## Workspace layout

```text
crates/niripip-core     pure policy, configuration, detector, geometry, state machine
crates/niripip-ipc      Niri JSON IPC wire adapter and NiriBackend trait
crates/niripip-daemon   event loop, reconnect, daemon Unix socket, persistence
crates/niripip-cli      human-facing CLI and doctor
```

The dependency direction is:

```text
niripip-core
    ↑
    ├── niripip-ipc
    │      ↑
    │      ├── niripip-daemon
    │      └── niripip-cli
    └────────── niripip-daemon
```

Core never imports Niri's Rust crate. This keeps policy tests independent of upstream enum churn.

## Core domain model

### WindowInfo

A normalized subset of Niri's current `Window`:

- id
- title
- app_id
- optional pid
- optional workspace id
- focused/floating/urgent flags
- logical layout geometry

### WorkspaceInfo

Normalized Niri workspace state. `id` is the identity; `idx` is display metadata only.

### OutputInfo

Logical output geometry retrieved through the `Outputs` request. Output state is refreshed on startup/reconnect and through rate-limited, event-triggered refreshes when workspace or layout changes indicate that output topology/geometry may have changed.

### TrackedWindow

```text
TrackedWindow {
    window_id
    mode: AutoPip | ManualPin
    detector: Option<String>
    score: Option<i32>
    follow_mode
    current_workspace
    desired_workspace
    original_was_floating
    geometry
    placement
    pending_action
    last_external_geometry
}
```

Window IDs are runtime identifiers only. They are never restored from disk after daemon restart or reboot.

## Event processing

The production Niri adapter creates two channels to Niri:

1. an event-stream connection;
2. independent request/action connections.

The event stream's initial full snapshot is the authoritative compositor state. A reconnect creates a fresh stream and reconciles tracked IDs against its `WindowsChanged` snapshot.

Known upstream events are normalized into `CompositorEvent`. Unknown future top-level events are ignored with trace logging.

### Event loop

```text
Niri event
  ↓
wire parser
  ↓
normalized CompositorEvent
  ↓
Engine::handle_event
  ├── update canonical state
  ├── classify untracked windows
  ├── reconcile tracked windows
  └── emit Vec<Effect>
             ↓
      daemon executor
             ↓
      NiriBackend::execute
```

Effects are explicit values. Core never performs I/O.

## Detector engine

Each configured detector has required filters plus a score. Required filters can include:

- title regex
- app-id regex
- minimum/maximum logical size
- floating state

Optional heuristics add score:

- compact-window bonus
- 16:9-ish aspect bonus
- empty app-id bonus
- PID-present bonus
- newly-opened bonus

A detector is eligible only when all of its required filters pass. Eligible score is:

```text
base detector score + enabled heuristic bonuses
```

The highest score above `general.detection_threshold` wins.

This prevents the Chromium empty-app-id case from depending on a browser app-id while still allowing stricter Firefox/Brave/etc. profiles.

Classification is re-run on every `WindowOpenedOrChanged` while a window is untracked. This directly handles clients that set title/app-id after mapping.

## State machine

Tracked windows use these conceptual phases:

```text
Observed
  ↓ detector/manual pin
Managing
  ↓ actions issued
Tracked
  ↔ FollowingWorkspace
  ↔ UserAdjusted
  ↓ WindowClosed
Forgotten
```

There is no separate async task per window. One engine owns canonical state, making deduplication deterministic.

### Initial PiP management

For an auto-detected PiP:

1. mark it tracked before emitting effects;
2. `MoveWindowToFloating { id }` if needed;
3. set width/height by id;
4. move by id to the selected placement;
5. never focus the PiP for management;
6. if the PiP itself stole focus during its short map/classification window, optionally restore the previously focused live window;
7. record short-lived action expectations to distinguish resulting layout events from manual user movement.

Effects are idempotent enough to be retried after a transient IPC reconnect, but the engine avoids repeatedly issuing them for every metadata event.

### Generic pin

`niripip pin` defaults to the currently focused window in cached compositor state. `--window-id` selects explicitly.

Manual pin:

- records whether the window was already floating;
- moves it to floating if necessary;
- does not resize it to a PiP preset;
- begins workspace following.

Unpin stops following. By default, a manual pin that niri-pip itself moved from tiling to floating is restored to tiling; a window that was already floating remains floating. This can be disabled in config.

## Workspace follow algorithm

Default mode: `follow-workspace`.

Trigger: `WorkspaceActivated { id, focused: true }`.

For each tracked window:

1. ignore if already on target workspace;
2. ignore if the same `(window,target)` move is already pending and fresh;
3. emit `MoveWindowToWorkspace` with the concrete window id, workspace id reference, and `focus=false`;
4. store expected target and a bounded suppression deadline;
5. accept the subsequent window update as acknowledgement;
6. re-apply floating/placement only if Niri reports that the invariant was lost.

Events generated by our own move therefore do not recursively trigger another move.

A short debounce coalesces rapid workspace activations; the final globally focused workspace wins.

## Focus policy

Normal PiP management uses only actions that address a concrete window id. Workspace transfer uses `focus=false`.

The daemon does not focus the PiP to resize/move it. A bounded compensation path exists only for a newly mapped PiP that is reported focused: while the original map candidate is younger than `focus_restore_window_ms`, the engine may focus the previously focused live window. The candidate is then discarded, so ordinary later focus changes are not fought. The default window is 500 ms and can be set to zero.

If a future Niri action loses id targeting, that feature must be disabled or guarded; the daemon must not silently fall back to focus juggling.

## Positioning

Niri's move action operates in working-area coordinates but IPC does not publish the working-area rectangle.

v0.1 represents placement as normalized percentages derived from logical output size, desired window size, gap and configured safe margins. The adapter sends `PositionChange::SetProportion` so Niri applies the percentage to the actual working area. This is intentionally better behaved across bar/dock reservations than assuming raw absolute coordinates, while still acknowledging that exact arbitrary exclusive-zone geometry is unavailable.

Supported placements:

- top-left
- top-right
- bottom-left
- bottom-right
- center

`position_mode = "remember"` records normalized observed floating position after an external layout change. A short action-origin suppression interval prevents the daemon from mistaking its own moves/resizes for user movement.

`position_mode = "fixed"` keeps the configured corner and does not learn manual movement.

## Multi-monitor

The core model distinguishes:

- `follow-workspace` — follow globally focused workspace (default);
- `follow-focused-output` — currently implemented as following the focused workspace/output pair; experimental in v0.1;
- `stay-on-output` — do not follow a focused workspace on another output; experimental.

Niri workspace IDs remain the move target, avoiding ambiguous per-output workspace indices.

Because the current event stream has no dedicated output-change event, output geometry is fetched on connection and refreshed at most once per 2 seconds when `WorkspacesChanged` or `WindowLayoutsChanged` supplies an event-driven hint. There is no timer polling loop.

## Fullscreen

True fullscreen can cover floating windows in Niri. v0.1 does not attempt to defeat compositor stacking rules.

Roadmap research can evaluate a separate layer-shell overlay renderer. Such a renderer would be a distinct component and security/input model; it will not be smuggled into the normal window-management daemon.

## Manual geometry tracking

`WindowLayoutsChanged` is the only compositor signal required for geometry observation. There is no upstream "user moved window" event.

For a tracked floating window:

- layout changes inside the recent-action suppression interval update current geometry but are not learned as manual position;
- later layout changes that materially change position/size are treated as external/user adjustments;
- remember mode writes normalized geometry to runtime state with a one-second daemon flush cadence; CLI mutations flush immediately.

State writes are atomic (`state.json.tmp` + rename).

## Daemon ↔ CLI protocol

Socket path:

```text
$XDG_RUNTIME_DIR/niri-pip/niripip.sock
```

The directory is mode `0700`; the socket is mode `0600` after bind.

Transport is one JSON request and one JSON response per Unix socket connection. Protocol messages include a protocol version so a future GUI can reuse the endpoint.

Commands:

- status
- list
- pin
- unpin
- toggle
- reload-config
- set-enabled

No TCP listener and no shell-command execution exists.

## Persistence

Path:

```text
$XDG_STATE_HOME/niri-pip/state.json
```

or `~/.local/state/niri-pip/state.json`.

Persisted:

- learned PiP size;
- normalized remembered position per detector/profile.

The configured placement remains configuration; live window IDs and transient placement state are not persisted.

Not persisted:

- live window IDs;
- pending actions;
- focused workspace;
- Niri socket path.

This prevents stale IDs after logout/reboot.

## Reconnect and recovery

Event-stream failure enters degraded mode and reconnects with capped exponential backoff:

```text
250 ms → 500 ms → 1 s → 2 s → 4 s → 8 s → 15 s cap
```

A successful reachability/version probe resets the backoff before the next stream attempt.

Command failures do not kill the daemon. Failed effects are logged and invariants are re-evaluated when the next compositor event arrives.

Niri session services are tied to the graphical/Niri lifecycle so a full compositor service restart normally restarts the daemon with a fresh `NIRI_SOCKET` environment.

## Race handling

- **open then immediate close:** `WindowClosed` removes both candidate and tracked state; later effects are allowed to fail harmlessly.
- **late title/app-id:** untracked windows re-score on every `WindowOpenedOrChanged`.
- **workspace switch during open:** the current focused workspace is evaluated after classification; follow effect targets the latest state.
- **duplicate events:** tracked classification and pending target checks make processing idempotent.
- **user disables floating:** tracked PiP re-establishes floating; manually pinned windows do the same while pinned.
- **browser crash:** closed IDs are removed; full bootstrap reconciliation also removes stale IDs.
- **daemon restart:** no window IDs restored from disk; bootstrap re-detects existing PiP windows from current metadata.
- **IPC disconnect:** mark the backend disconnected but preserve tracked/manual intent while the same PID-scoped Niri socket owner is alive; reconnect with capped exponential backoff and let the authoritative `WindowsChanged` bootstrap prune windows that disappeared during the gap. If the socket disappears or its Niri PID exits, the daemon exits so systemd can restart it with a freshly imported `NIRI_SOCKET`.

## Testing seam

`niripip-ipc` defines `NiriBackend`.

Production: `RealNiriBackend`.

Tests: `MockNiriBackend`, an in-memory event receiver/action recorder.

Most logic tests call the pure core engine directly. Adapter tests validate exact Niri JSON shapes separately.

## v0.2 controller layer

v0.2 keeps the v0.1 compositor adapter/state-machine boundary and adds controller mutations inside
`Engine` rather than shelling out to `niri msg`.

Controller requests travel over daemon protocol v2 and resolve a target in this order:

1. explicit `--window-id`;
2. focused tracked window;
3. exactly one auto-detected PiP;
4. exactly one tracked window;
5. otherwise an ambiguity error.

This lets a user keep keyboard focus in another application while resizing, moving, following or
locking the PiP.

### Learned geometry

Auto-PiP detector profiles keep arbitrary width/height and normalized x/y. Manual compositor layout
events outside the daemon self-action suppression window update this profile. Exact CLI resize and
position commands update the desired profile immediately so partial width/height events cannot lose
an explicit user request.

State schema 2 adds per-detector controller metadata while migrating schema-1 geometry in place.

### Geometry lock

Lock captures the current normalized geometry. Later `WindowLayoutsChanged` events compare the real
layout with the locked target. Deviations outside small tolerances generate ID-addressed width,
height and floating-position actions, followed by the normal suppression interval to avoid feedback
loops.

### Opacity boundary

Niri opacity is a continuous window-rule property, not an ID-addressable IPC setter. The daemon
therefore does not focus the PiP and call `toggle-window-rule-opacity`. Instead it atomically writes a
tiny generated `niri-pip-runtime.kdl` rule scoped to PiP titles. A marker-scoped late include is
installed with backup + validation. `opacity auto` writes an empty runtime rule file, returning
control to ordinary Niri/iNiR rules.

### Media boundary

MPRIS media commands are intentionally implemented in the CLI through `playerctl`, not in core
window policy. Browser media-session ambiguity or absence cannot break Niri window management.
