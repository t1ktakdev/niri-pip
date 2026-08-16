# Browser support

## Chromium / Google Chrome / Brave

The important v0.1 case is a PiP window with a PiP title and an empty or missing app-id. This is a
normal possibility in the target Niri + xwayland-satellite setup and is why niri-pip does not require
`google-chrome` as an app-id.

Default matching recognizes the English `Picture in picture` / `Picture-in-Picture` title forms,
constrains the window to a PiP-like maximum size, then scores empty app-id and aspect/compactness as
additional evidence.

Niri IPC currently has no native-Wayland-vs-XWayland marker, so niri-pip does not claim to recover a browser parent relationship that the compositor does not expose. On native Ozone/Wayland Chromium normally sends a toplevel app-id derived from its WM/application class; on Ozone/Wayland Chromium also cannot assume global screen-coordinate control for PiP placement. niri-pip therefore keeps detection metadata-driven and leaves placement to Niri IPC.

## Firefox

Niri's own window-rule documentation uses `app-id="firefox$"` and title
`Picture-in-Picture` as a PiP example. niri-pip ships a high-confidence equivalent detector.

## Vivaldi / Edge / Zen

Chromium-family titles/app-ids can work through the generic/configurable detector engine. They are
not claimed as exhaustively tested in v0.1. Add a custom detector when your build/localization uses
different metadata.

## Localization

Browser PiP titles are not guaranteed to remain English across all builds/locales. Add a detector
with your observed exact localized title rather than making the built-in matcher dangerously broad.
Use `niri msg --json windows` to inspect metadata.
