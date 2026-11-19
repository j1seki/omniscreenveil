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
| **Exclusion** | Protected window stays visible and interactive locally while compatible capture paths omit it. | Requires OS compositor cooperation and, on Windows, a helper loaded into the selected process. |
| **Veil Surface** | Protected windows are filtered out or masked in the window we render. | Select the Surface window in the share picker, not the whole desktop. |
| **Strict hide** | The protected window is hidden on the local desktop for the session. | You cannot use the window while protected. Restored on release. |

## Platform matrix

| Platform | Exclusion | Veil Surface | Strict hide | Notes |
|---|---|---|---|---|
| **Windows 10 2004+ / 11** | **partial / experimental** | **partial / experimental** | **partial / experimental** | Helper injection and GDI capture require native runtime validation. Test the actual capture application. |
| **macOS 14+** | **unsupported** — no public ScreenCaptureKit exclusion API for foreign windows | **partial / experimental** | **partial / experimental** | ScreenCaptureKit and Accessibility require consent and native runtime validation. |
| **Linux / X11** | **unsupported** | **native** | **native** | XComposite or root-window capture, with window frames included in masks. Strict hide unmaps windows. |
| **Linux / Wayland** | **unsupported** | **partial** | **partial** | sway and Hyprland use `grim` and compositor IPC. GNOME Shell and KDE Plasma show a labelled simulation. |

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
| macOS Surface | ScreenCaptureKit filters selected window IDs out of the main-display capture. Verify a visible canary window disappears from the Surface and reappears after release on a consented Mac. |
| macOS strict hide | Accessibility `AXMinimized` removes the window from the capture and `AXMinimized=false` restores it; requires consented permission. |
| X11 Surface | The Xvfb integration test checks the canary and window-manager frame pixels before and after masking. |
| X11 strict hide | The same test checks unmap, recovery through a new X11 connection and restored pixels. |
| Wayland Surface | Live sway and `grim` checks cover the window body and compositor title bar. Hyprland needs runtime validation. |
| Wayland strict hide | A live sway check verifies a window moves to the hidden workspace and is restored on release. Hyprland needs runtime validation. |
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
| macOS | Screen Recording (capture), Accessibility (strict hide) | when requested from the desktop app’s permission panel | the affected feature is unavailable; the app starts and can still use the other feature if its permission is granted |
| X11 | access to `$DISPLAY` | on connect | simulated backend, labelled in the UI |
| Wayland | Access to the compositor IPC socket and `grim` capture | When inventory or capture runs | Inventory and capture errors are shown in the UI |

There is no silent elevation, no helper installed as a service, and no
background process that outlives the app.
