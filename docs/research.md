# Upstream research

Research date: 2026-08-16

This document records the upstream interfaces and constraints that niri-pip v0.1.0 is designed around. The implementation deliberately avoids inventing compositor APIs.

## Sources

Primary sources used for the v0.1 design:

- Niri IPC wiki: https://github.com/niri-wm/niri/wiki/IPC
- Niri IPC Rust source: https://github.com/niri-wm/niri/blob/main/niri-ipc/src/lib.rs
- Niri event-stream state helper: https://github.com/niri-wm/niri/blob/main/niri-ipc/src/state.rs
- Niri floating-window implementation: https://github.com/niri-wm/niri/blob/main/src/layout/floating.rs
- Niri floating-window docs: https://github.com/niri-wm/niri/wiki/Floating-Windows
- Niri window-rule docs: https://github.com/niri-wm/niri/wiki/Configuration:-Window-Rules
- Niri fullscreen/maximize docs: https://github.com/niri-wm/niri/wiki/Fullscreen-and-Maximize
- Niri session startup source: https://github.com/niri-wm/niri/blob/main/resources/niri-session
- Niri IPC server/socket source: https://github.com/niri-wm/niri/blob/main/src/ipc/server.rs
- Niri main/session environment import source: https://github.com/niri-wm/niri/blob/main/src/main.rs
- Niri Xwayland docs: https://github.com/niri-wm/niri/wiki/Xwayland
- iNiR repository: https://github.com/snowarch/iNiR
- iNiR architecture: https://github.com/snowarch/iNiR/blob/main/ARCHITECTURE.md
- iNiR IPC reference: https://github.com/snowarch/iNiR/blob/main/docs/IPC.md
- Chromium PiP window manager: https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/picture_in_picture/picture_in_picture_window_manager.cc
- Chromium Ozone/Wayland toplevel: https://chromium.googlesource.com/chromium/src/+/refs/heads/main/ui/ozone/platform/wayland/host/wayland_toplevel_window.cc

At the time of research the latest stable Niri release is v26.04. The repository's main branch can be newer than the release, so niri-pip uses the stable JSON IPC shape conservatively and ignores unknown event variants rather than binding its runtime compatibility to an exhaustive Rust enum.

The compatibility baseline was checked against the **v26.04 tag itself**, not only `main`. The v26.04 tag already contains the ID-addressable `SetWindowWidth`/`SetWindowHeight`, `MoveWindowToFloating`, `MoveWindowToTiling`, `MoveFloatingWindow`, and `MoveWindowToWorkspace { window_id, reference, focus }` actions used by niri-pip. Its `Window` shape also already contains optional `pid`, `workspace_id`, floating/focus state and `WindowLayout`. Current `main` was reviewed separately for forward changes.

## Niri IPC transport

Niri exposes a Unix-domain socket at `$NIRI_SOCKET`. Requests and responses are newline-delimited JSON.

`Request::EventStream` changes a connection into a one-way stream: after the request is accepted, Niri stops reading further requests on that socket and continuously writes events. A client that both watches events and executes actions therefore needs two connections:

1. a long-lived event-stream socket;
2. a request/action socket (or short-lived action connections).

niri-pip uses a long-lived event stream plus independently reconnectable short-lived command connections.

Niri v26.04 creates the IPC pathname as `niri.<wayland-socket>.<pid>.sock`. This matters for recovery: a full compositor restart normally changes `NIRI_SOCKET`. niri-pip retries transient failures while the original socket owner still exists, but exits when the PID-scoped owner is gone so systemd can spawn a fresh daemon with the newly imported session environment.

The event stream sends a complete state snapshot first. The daemon therefore does not need a polling loop or a separate `niri msg windows` bootstrap.

Niri explicitly documents that different state components are not guaranteed to be consistent after every individual event. For example, a workspace update can precede the window update that makes a referenced window visible. The daemon state machine must tolerate temporary missing references and reconcile on subsequent events.

## Current event names

Relevant current upstream window/workspace events reviewed for v0.1:

- `WorkspacesChanged { workspaces }`
- `WorkspaceActivated { id, focused }`
- `WorkspaceActiveWindowChanged { workspace_id, active_window_id }`
- `WindowsChanged { windows }`
- `WindowOpenedOrChanged { window }`
- `WindowClosed { id }`
- `WindowFocusChanged { id }`
- `WindowFocusTimestampChanged { id, focus_timestamp }`
- `WindowUrgencyChanged { id, urgent }`
- `WindowLayoutsChanged { changes }`

v0.1 consumes the state-changing events it needs (`WorkspacesChanged`, workspace activation/active-window changes, `WindowsChanged`, window open/change/close/focus, and `WindowLayoutsChanged`). Focus-timestamp and urgency events are valid upstream variants but are not needed for sticky/PiP policy, so the tolerant parser treats them like other currently irrelevant events.

There is no separate "window metadata changed" event. Title/app-id/workspace/floating changes arrive in `WindowOpenedOrChanged`; layout-only changes arrive in `WindowLayoutsChanged`.

## Current Window shape

Current upstream `niri_ipc::Window` exposes:

- `id: u64`
- `title: Option<String>`
- `app_id: Option<String>`
- `pid: Option<i32>`
- `workspace_id: Option<u64>`
- `is_focused: bool`
- `is_floating: bool`
- `is_urgent: bool`
- `layout: WindowLayout`
- `focus_timestamp: Option<Timestamp>`

Important constraints:

- PID exists in current upstream, but is optional and must never be required for PiP detection.
- There is no IPC field that identifies a window as native Wayland versus XWayland.
- There is no `is_fullscreen` field in `Window`.
- `title` and `app_id` can be missing. An empty string is also a valid observed value and differs from `None` at the wire level; matching treats both deliberately.

`WindowLayout` provides logical-pixel geometry including `tile_size`, `window_size`, and optional `tile_pos_in_workspace_view`. For floating windows, Niri reports a tile position; this is enough to observe geometry changes, including probable user moves. It is not an explicit "user moved" signal, so niri-pip uses an action-origin suppression window to distinguish its own recent moves from external geometry changes.

## Current Workspace shape

Current upstream `Workspace` exposes:

- stable-lifetime `id: u64`
- per-output `idx: u8`
- optional `name`
- optional output name
- `is_urgent`
- `is_active`
- `is_focused`
- `active_window_id`

Every output has one active workspace; only one workspace is globally focused. v0.1 `follow-workspace` follows the globally focused workspace (`WorkspaceActivated` with `focused=true`), not every output's active workspace.

## Current Output shape

`Output` exposes the output name, modes and `logical: Option<LogicalOutput>`. `LogicalOutput` includes logical x/y, width/height, scale and transform.

The IPC does not expose the compositor's calculated usable/working-area rectangle directly. There is also no dedicated output-change event in the current `Event` enum. niri-pip therefore reads `Outputs` on connect and performs a rate-limited refresh when workspace topology or window-layout events provide an event-driven hint that output geometry may have changed. It does not poll on a timer.

## Actions used by niri-pip

Current upstream actions used by v0.1:

- `MoveWindowToFloating { id: Option<u64> }`
- `MoveWindowToTiling { id: Option<u64> }`
- `SetWindowWidth { id: Option<u64>, change: SizeChange }`
- `SetWindowHeight { id: Option<u64>, change: SizeChange }`
- `MoveFloatingWindow { id: Option<u64>, x: PositionChange, y: PositionChange }`
- `MoveWindowToWorkspace { window_id: Option<u64>, reference: WorkspaceReferenceArg, focus: bool }`
- `FocusWindow { id: u64 }` is available but is intentionally not part of normal PiP management.

`SizeChange::SetFixed` uses logical pixels. `PositionChange::SetFixed` also uses logical pixels.

`MoveWindowToWorkspace` accepts both a concrete window id and `focus=false`, which is crucial: a pinned PiP can follow the focused workspace without focusing the PiP or moving keyboard focus to the destination as a side effect.

## Floating position semantics and working area

Niri's floating implementation stores positions relative to the working area. Current source implements `PositionChange::SetFixed(x)` as `x + working_area.loc.x`, and equivalent behavior for y. Proportional positioning uses the working-area width/height.

Niri's public window-rule docs also state that floating positions are logical coordinates relative to the working area and that bars/struts affect that working area.

However, the IPC's `Output` response exposes logical output geometry, not the calculated working-area rectangle. Therefore niri-pip cannot compute a mathematically exact bottom-right top-left coordinate for arbitrary layer-shell reservations from IPC alone.

v0.1 policy:

- use Niri's working-area-relative move action;
- calculate corner offsets from logical output dimensions and configured safe margins;
- treat configured margins as operator-tunable safe insets;
- never claim that arbitrary third-party exclusive zones can be discovered exactly;
- record this limitation in troubleshooting and roadmap docs.

No configuration file is injected into Niri to obtain `default-floating-position`; niri-pip does not edit `~/.config/niri/config.kdl`.

## Floating and fullscreen behavior

Floating windows are a per-workspace layout and render above tiled windows.

True fullscreen is different: when a true fullscreen window is focused and settled, Niri renders it above floating windows and the top layer-shell layer. Overlay layer-shell can render above true fullscreen.

Therefore v0.1 supports tiled/maximized workflows but **does not promise PiP above true fullscreen**. Floating windows are above tiled windows, but niri-pip also does not claim an independent compositor-level keep-above guarantee relative to every other floating window. A future overlay/layer-shell experiment is a roadmap item, not a hidden v0.1 hack.

## Xwayland and Chromium

Current Niri integrates xwayland-satellite and presents X11 applications to Niri as normal Wayland windows. The Niri window IPC does not expose an XWayland/native distinction.

Chromium's current native Ozone/Wayland toplevel path sends its Wayland app-id from the window's Chromium/WM-class-derived identifier and sends the window title separately. Chromium's PiP manager also explicitly notes that Ozone/Wayland platforms may not support client-controlled global screen coordinates, so Chromium itself cannot always place PiP in global desktop coordinates. Those upstream facts support compositor-side placement, but they do **not** provide niri-pip with a stable parent-browser relationship through Niri IPC.

Consequences for detection:

- niri-pip cannot reliably infer "this window belongs to Chrome" from XWayland/native status;
- browser association based on PID is optional/weak because PID can be absent and process topology is not a stable browser identity API;
- title, app-id, geometry, temporal updates, and configurable user matchers are the reliable signals available from Niri IPC.

The target system observation that Chromium PiP can appear as `title = "Picture in picture"` with an empty `app-id` is therefore treated as a first-class default detection case rather than requiring `app-id = "google-chrome"`.

Firefox is easier: Niri's own window-rule documentation uses the example `app-id="firefox$" title="^Picture-in-Picture$"` for Firefox PiP.

## Detection design consequence

v0.1 uses a scoring engine rather than a single browser app-id test. Signals include:

- title regex;
- app-id regex (including explicit empty app-id matching);
- optional PID-present signal (never required by defaults);
- min/max logical width/height;
- aspect-ratio proximity;
- floating state;
- age/new-window signal;
- configurable weights and threshold.

Required regex/geometry constraints reject a detector immediately. Optional signals add score. Default browser detectors are conservative and can be overridden in TOML.

Metadata can arrive after initial mapping, so every `WindowOpenedOrChanged` re-evaluates an unclassified candidate. Once a window is managed, repeated metadata events do not re-run destructive initialization.

## Focus behavior

The actions required for float, resize, position and workspace transfer all support selecting the window by ID. Workspace transfer additionally supports `focus=false`.

niri-pip therefore does not need the common "focus PiP, mutate it, restore old focus" workaround for float/resize/move actions. The daemon never focuses the PiP as part of management.

There is one narrowly scoped compensation path: if the newly mapped PiP itself becomes focused within `general.focus_restore_window_ms` (500 ms by default), niri-pip may issue `FocusWindow` for the previously focused live window. This restores focus already stolen during PiP creation; it is not required for management actions. Set the window to `0` to disable this compensation if very fast intentional clicks on a new PiP are more important in a particular setup.

## Systemd user-session environment

Niri's session startup path imports `WAYLAND_DISPLAY`, `DISPLAY`, `XDG_CURRENT_DESKTOP`, `XDG_SESSION_TYPE` and `NIRI_SOCKET` into the system manager and D-Bus activation environment when running as a session.

The recommended Niri systemd model is to run graphical helpers as user services tied to the Niri/graphical session. niri-pip therefore ships a user unit with `PartOf=graphical-session.target`, ordering after the graphical session, and installer logic that also adds it as a want of `niri.service` when that unit exists.

`niripip doctor` checks that the daemon process actually sees a usable Niri socket. It does not silently guess a socket from arbitrary files when `$NIRI_SOCKET` is missing.

## iNiR architecture and integration decision

iNiR is a Quickshell/QML desktop shell with its own QML `IpcHandler` targets and an `inir <target> <function>` CLI routing layer. The current repository documents internal services and IPC targets, but niri-pip does not assume a stable third-party plugin ABI.

v0.1 integration is therefore non-invasive:

- niri-pip remains fully Niri-first and works without iNiR;
- `integrations/inir/` contains optional keybind snippets and notes;
- no iNiR core files are patched;
- a future iNiR UI/status module should be upstreamed or built against a documented extension boundary if/when one exists.

## Compatibility strategy

The `niri-ipc` Rust crate follows Niri's release version and its Rust enum surface can grow. The JSON IPC, in contrast, is intended for programmatic access and evolution by adding fields/variants.

niri-pip therefore uses its own narrow JSON wire model:

- known fields are deserialized with defaults/`Option`;
- unknown object fields are ignored;
- top-level event variants are dispatched by key, so unknown future events are logged at trace level and ignored;
- requests/actions are serialized only from action shapes verified above.

This avoids a needless rebuild lockstep with every Niri patch while keeping the daemon strict about the fields/actions it actually uses.

## v0.1 non-goals / explicit limitations

- No overlay above true fullscreen.
- No guaranteed browser-parent relationship when the compositor does not expose one.
- No guaranteed native-Wayland/XWayland distinction from Niri IPC.
- No exact arbitrary layer-shell usable-area discovery through Niri IPC.
- Multi-monitor `follow-focused-output` is implemented conservatively and marked experimental; `follow-workspace` is the default.
- No modifications to the user's Niri/iNiR configuration during install.

## Output changes: upstream event-stream gap

The current `Event` enum does not contain an `OutputsChanged` event. Output geometry is available through the separate `Outputs` request, while workspace changes carry output names.

v0.1 therefore does **not** poll outputs on a timer. It performs an `Outputs` snapshot at startup, after reconnect, and performs a rate-limited refresh when workspace/layout events (`WorkspacesChanged` or `WindowLayoutsChanged`) indicate that topology or usable geometry may have changed. This is event-triggered reconciliation, not `sleep + niri msg` polling.

## v0.2 controller research update — 2026-08-16

The controller layer continues to use the verified Niri 26.04 IPC actions already documented above.
No invented per-window opacity action was added: current Niri documents opacity as a window-rule
property, while `toggle-window-rule-opacity` applies to the focused window. For focus-safe PiP
control, v0.2 therefore uses a generated late window-rule include for PiP opacity.

Niri 26.04 supports optional top-level includes and watches included files for live reload. Includes
are positional, so a late niri-pip rule can override an earlier generic inactive-window opacity rule.
The installer prefers iNiR's user-extra file and otherwise falls back to the main user Niri config.

Current iNiR documentation describes Quickshell/QML IPC and ships `fuzzel` plus `playerctl`; v0.2
uses these as external integration boundaries rather than patching iNiR core QML.
