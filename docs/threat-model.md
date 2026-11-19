# Threat model

OmniScreenVeil protects one specific boundary: **what a capture sees**. It does
not protect a machine from its user, its user from the machine, or a window from
someone with code execution on the same desktop.

## Assets

1. The pixels of a protected window (passwords, messages, customer data).
2. The fact that a protected window exists, in the Veil Surface mode.
3. The user's ability to keep using the window locally (Exclusion / Surface) or
   knowingly give that up (strict hide).

## Adversary and channel

**In scope:** a screen-sharing or recording audience receiving a capture
through a supported path (Discord, Zoom, OBS, browser getDisplayMedia, Meet,
Teams, QuickTime, `ffmpeg`-style capture, platform screenshot APIs).

**Out of scope:** an adversary with local code execution, a root/administrator
compromise, a modified capture client that bypasses the OS, or a user who
willingly captures the window outside OmniScreenVeil.

## Assumptions per mode

### Exclusion (Windows only)

- The capture client asks the Desktop Window Manager / graphics capture stack
  for the window contents. `WDA_EXCLUDEFROMCAPTURE` is honoured by DWM and by
  current Windows capture APIs.
- The selected application is not malicious and does not remove the property or
  call `SetWindowDisplayAffinity(WDA_NONE)` itself. OmniScreenVeil re-applies it,
  but a hostile process inside the target can always win a race.
- Games and some hardware-accelerated overlays may render through paths that
  ignore display affinity. The verifier will report what actually happened; if
  the canary remains, the UI says **Visible**, not **Protected**.

### Veil Surface (all platforms)

- The audience shares the Surface window, not the whole desktop. If they share
  the entire desktop, the original windows are visible; the product cannot stop
  that and the UI says so.
- The Surface captures the desktop itself; the audience captures only the
  rendered Surface. On macOS, selected windows are filtered out of the main
  display capture by ScreenCaptureKit. Other backends replace protected
  rectangles after capture, before the frame is presented.
- Protection is for inventoried windows, not arbitrary application pixels.
  Menus, transient overlays, or other content rendered outside those windows
  may still appear. Inventory and capture are separate operations; test with
  harmless content in your sharing app before showing sensitive material.
- The Surface must never capture itself. On X11 with a compositor and on macOS
  (ScreenCaptureKit `SCContentFilter`), the app's own control and Surface
  windows are excluded before compositing; on bare X11 and on Wayland
  (`grim`/wlr-screencopy), their rectangles are masked to prevent recursion.
  On a single-monitor Wayland setup the Surface window's own rectangle will
  therefore appear checker-filled if it sits on the captured output; placing the
  Surface window on a second monitor avoids that overlap.
- On Wayland, live capture requires `grim` and compositor geometry from sway
  or Hyprland. Other compositors show a labelled simulation.

### Strict hide (all platforms)

- Hiding is local and visible: the window disappears for the user too. This is
  a fallback, not a stealth feature.
- On restart, X11 restores windows marked by this app. On sway, windows still
  on the private workspace return to the current workspace. Restoration after
  a crash on Windows or macOS can require restarting the affected app.
- macOS Accessibility can fail for hardened apps or after a permission reset.
  The failure is surfaced, and the window is not reported as protected.

## Non-goals

- Preventing the local user from taking a screenshot with another tool.
- Protecting audio, clipboard, notifications, thumbnails, task switchers, or
  window titles. Some OS surfaces (Alt-Tab previews, Mission Control) may still
  show a thumbnail; the support matrix does not claim otherwise.
- Protecting against a compromised capture client, kernel malware, a virtual
  machine hypervisor, or a camera pointed at the screen.
- DRM circumvention, anti-cheat evasion, forensic hiding, or parental control.
- Hiding the existence of the application from the OS or from administrators.

## Failure behaviour

Protection is **fail-closed** with respect to claims:

- No baseline canary → `Inconclusive`.
- Unsupported mode → the requested mode is downgraded by the capability matrix,
  and the UI shows the effective mode.
- Backend error → the window stays in the applied set as `Failed`, the visible
  status is red, and the next tick retries.
- Release error → the window is not removed from the applied set, so a later
  release can retry.

The one place the product fails *open* is intentional: if no real backend can
be reached, the app runs a labelled simulation with demo windows. It never
touches pixels and it never reports those windows as protected.

## Residual risks

| Risk | Mitigation / statement |
|---|---|
| Capture path ignores the OS flag (some games, overlays) | Pixel verification; the UI reports Visible. |
| Audience shares the full desktop in Surface mode | Prominent UI copy: share the Surface window. Not technically preventable. |
| Target process is hostile and undoes the flag | Re-applied on each tick; documented as out of scope against a local attacker. |
| Wayland compositor has no supported provider | The UI shows a labelled simulation. |
| App crash while strict-hidden | The journal records pending side effects before activation. Recovery depends on platform and runs when the app restarts. |
| Window titles / thumbnails leak | Not claimed; OS-level surfaces are outside the pixel guarantee. |
