# OmniScreenVeil

Selective screen-capture privacy for Windows, macOS and Linux.

![OmniScreenVeil with protection armed on X11](docs/images/app-x11-protected.png)

Pick the windows you do not want an audience to see, arm protection, then share
your screen normally. OmniScreenVeil keeps those windows usable on your own
desktop while they are replaced with a veil for everyone else.

OmniScreenVeil is **not** a window hider, a fake-camera filter, or a screenshot
blocker. It is honest about where each technique works:

| Mode | What it does | Where it works |
|---|---|---|
| **Exclusion** | The OS removes the window from every capture while it stays visible and interactive for you. | **Windows only** (display affinity + a small helper loaded into the app you selected). |
| **Veil Surface** | Share a window we render: a live desktop copy with protected windows masked. | **All platforms.** On Wayland it needs a geometry provider and is marked partial. |
| **Strict hide** | Hide the window on your own screen for the session, then restore it. | **All platforms** with the right permission. |

Anything not listed above is not claimed. macOS, X11 and Wayland have **no**
public per-window capture exclusion, and OmniScreenVeil does not use private
APIs, SIP changes, a patched X server or a root kernel module to pretend
otherwise. See [`docs/support-matrix.md`](docs/support-matrix.md) for the full
matrix and [`docs/threat-model.md`](docs/threat-model.md) for what stays out of
scope.

## Status

This is a Phase 0/1 working implementation, not a finished store release.

- The shared core, canary verifier, Veil Surface compositor and CLI are
  implemented and tested on Linux.
- The Linux/X11 backend is validated end to end against a real X server
  (`Xvfb`) with pixel assertions.
- The Tauri desktop app builds and runs; it is verified under Xvfb in this
  repository.
- The Windows and macOS backends compile behind their target gates and have
  Phase 0 probes, but still need runtime validation on real Windows and macOS
  machines. Run the probes in [`docs/phase-0-spikes.md`](docs/phase-0-spikes.md)
  and report what you see.
- Wayland now has a real backend for sway and Hyprland: `grim`/wlr-screencopy
  capture plus compositor IPC geometry, and strict hide through the compositor's
  own IPC. GNOME Shell and KDE Plasma lack a portable public equivalent and are
  reported as partial until a desktop-specific provider lands.

Verification is captured in [`docs/verification.md`](docs/verification.md).

## Layout

```
crates/
  veil-core          domain types, rules, reconciliation, capability matrix
  veil-surface       frame buffer + Veil Surface compositor
  veil-canary        8x8 canary token renderer and detector
  veil-protect       backend trait, engine, simulated backend
  veil-verify        baseline -> arm -> capture verification harness
  veil-protect-linux X11 backend (inventory, composite capture, strict hide)
  veil-protect-wayland Wayland backend for sway/Hyprland (grim + compositor IPC)
  veil-protect-win   Windows backend (injection + display affinity)
  veil-hook-win      helper DLL loaded into a selected process
  veil-protect-macos macOS backend + C ABI to the Swift helpers
apps/
  cli                the `veil` command-line agent
  desktop            Tauri 2 + React desktop application
platform/macos       Swift package: CGWindowList, Accessibility, ScreenCaptureKit
docs/                support matrix, threat model, verification, ADRs
research/            product research and constraints
```

The CLI and the desktop app consume the same crates. No protection policy lives
in the UI.

## Build and run

Prerequisites: Rust 1.80+, and for the desktop app Node 18+/pnpm plus the usual
[Tauri 2 system dependencies](https://v2.tauri.app/start/prerequisites/). On
Debian/Ubuntu the webview packages are `libwebkit2gtk-4.1-dev` and
`libayatana-appindicator3-dev`.

```sh
# core tests and self-test (no display required)
cargo test --workspace
cargo run -p veil-cli -- selftest

# honest capability matrix
cargo run -p veil-cli -- capabilities
cargo run -p veil-cli -- capabilities --platform wayland

# live X11 inventory, surface preview and strict hide
cargo run -p veil-cli -- list
cargo run -p veil-cli -- surface --live --protect 0x1a --out veil.ppm
cargo run -p veil-cli -- protect 0x1a --mode strict-hide
cargo run -p veil-cli -- release 0x1a
```

Desktop app:

Prerequisites for the live Linux backends: X11 needs a display; sway/Hyprland
need `grim` plus the compositor's own IPC tool (`swaymsg` or `hyprctl`). Without
them the app falls back to a labelled simulation rather than pretending.

```sh
cd apps/desktop
pnpm install
pnpm tauri dev        # development, live backend
pnpm build            # typecheck + frontend bundle
pnpm tauri build      # native bundle
```

macOS Phase 0 helpers:

```sh
cd platform/macos
swift build -c release
.build/release/veilmac-spike permission
.build/release/veilmac-spike list
swift test
```

## How protection works

### Verification is a capture

Every claim is proven by pixels, not by a return code. The verifier composites
an 8×8 canary token into the region, captures a **baseline**, arms protection,
captures again, and searches for the token. A missing baseline is
`Inconclusive`, never `Protected`. This is why the UI can say "Protected" with a
reason attached, and why a failed permission produces a visible result instead
of silent success.

### Windows

`Exclusion` injects `veil_hook_win.dll` into the process that owns the selected
window. The main application marks windows with the
`OmniScreenVeil.Protected` property; the helper applies
`SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` from inside the owning
process, which is the only way Windows accepts that call for a foreign window.
`StrictHide` uses `ShowWindow`. The hook is loaded only into the process the
user selected, and is unloaded best-effort on release.

### macOS

There is no public API for one application to remove another application's
window from ScreenCaptureKit, so `Exclusion` is reported unsupported. The Veil
Surface uses ScreenCaptureKit to capture a display (excluding the app's own
windows in the `SCContentFilter` so the Surface never recurses into itself) and
masks the protected windows; strict hide uses the Accessibility
`AXMinimized` attribute with the user's consent. `platform/macos` is a small
Swift package
with a C ABI so the Rust backend can call those public APIs.

### Linux

On X11 a normal client can enumerate foreign windows and read geometry, so the
Veil Surface works. With a compositing manager (`xcompmgr`, `picom`, KWin,
Mutter, ...) the backend composites foreign window pixmaps via XComposite
(`NameWindowPixmap`), which captures obscured windows and drops the app's own
control and Surface windows before the audience ever sees them. Without a
compositor it falls back to a root capture and masks its own windows. Strict
hide unmaps the window and restores it. All of these paths are proven in
`crates/veil-protect-linux/tests/x11_end_to_end.rs`.

The desktop app also provides an **Open window** action (and tray entry) that
opens a dedicated, chrome-free `OmniScreenVeil Surface` window — that second
window is what you select in Discord, Zoom, OBS or a browser share dialog.

On Wayland the compositor owns geometry and capture. The sway/Hyprland backend
combines two public surfaces: `grim` (wlr-screencopy) for frames and the
compositor's own IPC (`swaymsg -t get_tree`, `hyprctl clients -j`) for window
rectangles and strict hide. Protected windows are masked in the composited
Surface; strict hide moves them to a private workspace and returns them to the
original workspace on release. GNOME Shell and KDE Plasma do not expose a
portable equivalent, so they fall back to simulation / single-window mirroring
and are marked partial. `Exclusion` remains unsupported because no mainstream
compositor exposes per-window capture exclusion.

## Repository conventions

- GPL-3.0-or-later. See [`LICENSE`](LICENSE) and [`NOTICE.md`](NOTICE.md).
- No telemetry and no network access in the app.
- Format with `cargo fmt`, lint with `cargo clippy -- -D warnings`.
- Every `Native` capability in the matrix must have a capture test that proves
  it. If a test cannot run on a machine, it reports a skip, not a pass.
