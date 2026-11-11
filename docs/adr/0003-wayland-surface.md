# ADR 0003 — Wayland: compositing plus a geometry provider, never a private compositor patch

- Status: accepted
- Date: 2026-11

## Context

On Wayland the compositor owns window geometry, stacking and capture. A normal
client cannot enumerate foreign windows, cannot capture the root window, and
cannot ask the compositor to exclude a surface from a screencast. The product
still needs an honest answer for Wayland users.

The tempting options — a patched compositor, a privileged helper, an LD_PRELOAD
hook against the compositor — are not shippable and would break the clean-API
rule.

## Decision

Build the Veil Surface on two public inputs:

1. **Capture:** the xdg-desktop-portal ScreenCast API over PipeWire (what OBS,
   browsers and Teams use). Frames arrive as a DMA-BUF or shared-memory stream.
2. **Geometry:** a desktop-specific provider that reports window rectangles:
   - KWin: a KWin script exposing window geometry,
   - GNOME Shell: a Shell extension over D-Bus,
   - sway/Hyprland: their IPC sockets.

The compositor masks protected rectangles in the captured frame before it is
presented in the shareable Surface window. If no provider is available, the
fallback is **mirroring one chosen window** from the portal stream, which is
honest and useful but less flexible.

`Exclusion` remains unsupported: no mainstream compositor exposes per-window
capture exclusion to clients.

## Consequences

- The Wayland Surface is marked `partial`, with the missing provider named in
  the UI.
- Geometry can race with the compositor (window moved between frame and
  provider sample). The design samples both as close as possible and accepts a
  small lag; the verifier detects a missed mask as `Visible`.
- The Surface needs continuous capture and compositing, which costs CPU/GPU. The
  UI must say so.
- No root modules, no patched compositors, no private protocols.
- Future compositor protocols for capture exclusion can replace this design
  without changing the user-facing modes.
