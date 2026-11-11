# Phase 0 spikes

Phase 0 exists to answer one question per platform: **can the promise be kept
with clean, shippable APIs?** These are the probes that answer it before the
product claims anything.

## Windows

**Question:** can a normal desktop app remove a foreign window from capture
while it stays visible and interactive?

**Probe:** `crates/veil-protect-win` + `crates/veil-hook-win`.

1. Build `veil_hook_win.dll` (MSVC, x64).
2. Start a test window owned by another process.
3. Inject the DLL into that process (`VeilHook.dll` path).
4. Set `OmniScreenVeil.Protected` on the window.
5. The hook calls `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` from
   inside the owning process.
6. Capture with Windows Graphics Capture (OBS or the verifier) and assert the
   canary is gone while `IsWindowVisible` is still true.

**Result to record:** pass/fail per app (Win32, Electron, UWP, hardware
accelerated). Some capture paths may ignore display affinity; that must show as
`Visible` in the verifier.

**Known risk:** Defender/EDR may flag process injection. The hook is loaded only
into the process the user selects, and the release path unloads it, but code
signing and enterprise policy need testing before release.

## macOS

**Question:** which public APIs can we build on, and where is exclusion
impossible?

**Probe:** `platform/macos` (`veilmac-spike`). Public API only.

```sh
cd platform/macos
swift build -c release
.build/release/veilmac-spike permission   # Accessibility prompt
.build/release/veilmac-spike list         # CGWindowListCopyWindowInfo
.build/release/veilmac-spike capture 1    # ScreenCaptureKit -> PPM
swift test                                # invariants + no private symbols
```

The tests assert that the inventory excludes layer != 0 (menu bar, Dock,
overlays) and our own process, and that the legacy private symbols
`CGSSetWindowCaptureExcluded` / `SLSSetWindowCaptureState` are *not* used.

**Conclusion already encoded:** `Exclusion` is unsupported on macOS. The Surface
(ScreenCaptureKit) and strict hide (Accessibility `AXHidden`) are the product.

## Linux / X11

**Question:** can X11 provide inventory, capture, Surface masking and strict
hide without a privileged module?

**Probe:** `crates/veil-protect-linux/tests/x11_end_to_end.rs`.

Starts `Xvfb`, creates a canary window, and asserts with pixels:

1. canary visible in a root capture,
2. canary removed by the Surface plan,
3. canary removed by `XUnmapWindow`,
4. canary restored by `XMapWindow`.

This test runs in CI on Ubuntu (`xvfb` is installed) and locally without a
desktop session.

**Conclusion:** yes for Surface and strict hide. No for Exclusion (would need
XACE and a modified server).

## Linux / Wayland

**Question:** is there any clean path to per-window exclusion? If not, what is
the best honest fallback?

**Probe (Phase 1):**

1. Request a ScreenCast session through the portal (GNOME/KDE), receive a
   PipeWire stream.
2. Implement one geometry provider first (pick KWin or sway; they are easiest to
   script).
3. Mask protected rectangles in the captured frame and verify with the same
   canary harness.
4. If no provider: mirror one window from the stream.

**Conclusion so far:** Exclusion is unsupported; Surface is `partial` until a
provider ships. This is recorded in the capability matrix and shown in the UI.

## What Phase 0 deliberately did not do

- No private macOS symbols, no SIP changes.
- No patched X server, no XACE module, no root kernel module.
- No injection into a process the user did not select.
- No claim of protection without a baseline capture.
