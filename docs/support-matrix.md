# Support matrix

This is the honest matrix OmniScreenVeil ships. It is the same data the app and
CLI expose (`veil capabilities`, `PlatformCapabilities::for_platform`) and it
is enforced by tests. A capability marked **native** must have a capture test
that proves it; **partial** always carries the constraint in the product UI;
**unsupported** means no shippable, clean API exists and the app refuses to
pretend otherwise.

## Modes

| Mode | Promise | Costs |
|---|---|---|
| **Exclusion** | Protected window stays visible and interactive locally, but is absent from every capture. | Requires OS compositor cooperation and, on Windows, a helper loaded into the selected process. |
| **Veil Surface** | The audience sees a window we render; protected windows are checker-filled there. | The audience must share the Surface window, not the whole desktop. Capture runs continuously. |
| **Strict hide** | The protected window is hidden on the local desktop for the session. | You cannot use the window while protected. Restored on release. |

## Platform matrix

| Platform | Exclusion | Veil Surface | Strict hide | Notes |
|---|---|---|---|---|
| **Windows 10 2004+ / 11** | **partial / experimental** | **partial / experimental** | **partial / experimental** | Helper injection and GDI capture require native runtime validation. Test the actual capture application. |
| **macOS 14+** | **unsupported** — no public ScreenCaptureKit exclusion API for foreign windows | **partial / experimental** | **partial / experimental** | ScreenCaptureKit and Accessibility require consent and native runtime validation. |
| **Linux / X11** | **unsupported** — needs a privileged Xorg XACE module | **native** | **native** | Root-window capture + `XGetGeometry` inventory; `XUnmapWindow` strict hide. |
| **Linux / Wayland** | **unsupported** — compositors do not expose per-window capture exclusion | **partial** — sway/Hyprland: wlr-screencopy capture + IPC geometry; GNOME/KDE: portal mirror fallback | **partial** — sway/Hyprland IPC moves the window off-screen; GNOME Shell and KDE Plasma need a desktop extension | Per-window exclusion remains impossible. Capture uses `grim` (wlr-screencopy); no private compositor patches. |

### Why Exclusion is not universal

- **Windows** is the only OS with a public, per-window capture-exclusion flag.
  Even there, `SetWindowDisplayAffinity` only works for windows owned by the
  calling process, so the helper DLL is not optional.
- **macOS** deliberately gives the capture client the choice of what to include
  (`SCContentFilter`), not the captured app the ability to opt out. There is no
  public equivalent of display affinity.
- **X11** compositing is cooperative; excluding a window from a screencast
  requires changing the X server or its capture extension. XACE would need a
  privileged module and a modified server. That is out of scope.
- **Wayland** compositors own both geometry and capture. There is no protocol
  for "exclude this surface from screencopy/portal streams" that a normal client
  can invoke.

## Verification per platform

| Claim | How it is proven |
|---|---|
| Windows Exclusion | Pixel test: canary visible before arming, absent after `WDA_EXCLUDEFROMCAPTURE` is applied, restored on release, while the window remains `IsWindowVisible`. |
| Windows strict hide | `ShowWindow(SW_HIDE)` removes the canary from a desktop capture; `SW_SHOW` restores it. |
| macOS Surface | ScreenCaptureKit capture with the window's rectangle masked by the compositor; canary absent. |
| macOS strict hide | Accessibility `AXMinimized` removes the window from the capture and `AXMinimized=false` restores it; requires consented permission. |
| X11 Surface | `crates/veil-protect-linux/tests/x11_end_to_end.rs`: canary present in root capture, removed by `SurfacePlan`, strict hide removes it, remap restores it. |
| X11 strict hide | Same test, using `XUnmapWindow` / `XMapWindow`. |
| Wayland Surface | Phase 1 on sway/Hyprland: `grim` capture + `swaymsg`/`hyprctl` geometry, masked by the Surface, same canary harness. GNOME/KDE report partial until a provider ships. |
| Wayland strict hide | `crates/veil-protect-wayland` unit tests cover sway/Hyprland inventory parsing and command construction; runtime validation requires a live compositor. |
| Any platform, missing permission | The verifier returns `Inconclusive`, never `Protected`. |

## What is deliberately out of scope

- Private macOS SkyLight/CGS symbols and SIP changes.
- Modified or privileged X servers, XACE modules, root kernel modules.
- Injecting into processes the user did not select.
- Blocking screenshots taken by the local user with other tools. This product
  protects a shared capture, not the user's own screen from themselves.
- DRM, anti-cheat, forensic or parental-control use.

## Permission model

| Platform | Permission | When asked | If denied |
|---|---|---|---|
| Windows | none for display affinity; the hook is loaded only into the selected app | on first arm | Exclusion fails visibly; Surface / strict hide still available |
| macOS | Screen Recording (capture), Accessibility (strict hide) | when the feature is first used | Surface or strict hide reports `Inconclusive` / fails; the app keeps running in the remaining modes |
| X11 | access to `$DISPLAY` | on connect | simulated backend, labelled in the UI |
| Wayland | portal screen-cast consent | when the Surface starts | partial/unsupported, labelled in the UI |

There is no silent elevation, no helper installed as a service, and no
background process that outlives the app.
