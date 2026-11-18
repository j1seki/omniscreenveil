# OmniScreenVeil

Selective screen-capture privacy for Windows, macOS and Linux.

![OmniScreenVeil dark-mode walkthrough: overview, window rules, armed protection, Surface preview, and canary audit](docs/images/app-dark-demo.gif)

Pick which windows you want to keep out of a share, then open and share the
dedicated **OmniScreenVeil Surface** window. The original windows stay usable
on your desktop. Sharing the whole desktop instead bypasses Surface rules.

OmniScreenVeil is **not** a window hider, a fake-camera filter, or a screenshot
blocker. It is honest about where each technique works:

| Mode | What it does | Where it works |
|---|---|---|
| **Exclusion** | Asks the OS to omit the window from compatible capture paths while it stays usable locally. Verify your actual capture app. | **Windows only, experimental** (display affinity + a small helper loaded into the app you selected). |
| **Veil Surface** | Share a window we render: a desktop copy with selected windows filtered out or masked. | **All platforms.** On Wayland it needs a geometry provider and is marked partial. |
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
  repository. It ships a signed updater with `stable` and `beta` channels,
  automatic update checks and an explicit install/restart step.
- The Windows and macOS backends compile behind their target gates and have
  Phase 0 probes, but still need runtime validation on real Windows and macOS
  machines. Run the probes in [`docs/phase-0-spikes.md`](docs/phase-0-spikes.md)
  and report what you see.
- Wayland now has a real backend for sway and Hyprland: `grim`/wlr-screencopy
  capture plus compositor IPC geometry, and strict hide through the compositor's
  own IPC. GNOME Shell and KDE Plasma lack a portable public equivalent and are
  reported as partial until a desktop-specific provider lands.

Verification is captured in [`docs/verification.md`](docs/verification.md).

## Install on macOS

The release DMG targets **Apple silicon with macOS 14 or later**. Open the DMG,
drag OmniScreenVeil into Applications, then open the app from Applications.
Apple-signed, notarized releases do not require **Open Anyway**. If macOS says
an older download should be moved to the Trash, replace it with a current
notarized release rather than overriding the warning.

**Temporary workaround for a trusted test build:** If you built the app yourself
or verified where the build came from, copy it into Applications, then run this
from the repository in Terminal:

```sh
bash tools/open-macos-test-build.sh
```

You can also pass the installed `.app` path as an argument. The script checks
the bundle and its code signature, asks before removing quarantine from that
copy, then opens it. It doesn't use `sudo` or change system-wide settings. This
isn't a fix for a damaged build or a malware warning; if verification fails or
macOS still blocks it, download a fresh notarized build.

In the app, use **Request Screen Recording** for Veil Surface and **Request
Accessibility** only if you use strict hide. macOS still requires your consent.
The permission panel can open the exact settings page if the prompt doesn't
appear. If macOS asks you to quit and reopen after granting Screen Recording,
do that before using the Surface.

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
tools/               release helpers (updater manifest generator)
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

For a local Apple silicon test bundle (no notarization or updater artifact),
run this from the repository root on a Mac with Xcode installed:

```sh
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop tauri build --bundles app --config '{"bundle":{"createUpdaterArtifacts":false,"macOS":{"signingIdentity":"-"}}}'
open apps/desktop/src-tauri/target/release/bundle/macos/OmniScreenVeil.app
```

### Updates and release channels

The desktop app uses Tauri's signed updater. Both channels are backed by the
same GitHub Releases:

- **stable** — tags without a pre-release suffix (`v0.2.0`). The workflow
  publishes a normal GitHub release with `latest.json`; the app reads
  `releases/latest/download/latest.json`.
- **beta** — SemVer pre-release tags (`v0.2.0-rc.1`, `v0.2.0-beta.2`). The
  workflow marks them as GitHub pre-releases and refreshes `latest.json` on the
  rolling `updater-beta` release, which is what the beta endpoint reads.

`releases/latest` deliberately excludes pre-releases, so a beta can never be
served to stable users. Switching from beta to stable does not downgrade an
installed beta; the next newer stable release replaces it.

The updater verifies bundles before installing them. The updater public key is
checked into `tauri.conf.json`; the private key and its password must be stored
as the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
repository secrets. A local release build needs the same two variables:

```sh
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.tauri/omniscreenveil.key")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$HOME/.tauri/omniscreenveil.key.password")"
cd apps/desktop/src-tauri
pnpm tauri build
```

Back up the private key and password. Losing them means existing installs can
never receive another update; leaking the private key lets someone sign a
malicious update. The release workflow also refuses to publish when a tag does
not match the version in `tauri.conf.json`, and derives the channel from SemVer,
so a workflow dispatch from a branch cannot mark an RC as stable "latest".

### macOS release signing

The updater signature does not satisfy macOS Gatekeeper. For both stable and
beta releases, configure these GitHub Actions repository secrets before
building a release:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` exported from Keychain, including the Developer ID Application certificate and its private key |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` export |
| `APPLE_SIGNING_IDENTITY` | Full identity, `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID` | Apple Account email for notarization |
| `APPLE_PASSWORD` | App-specific password for that account, not the account password |
| `APPLE_TEAM_ID` | Developer team ID for that account |

Tauri signs and notarizes the app before creating the DMG and updater archive.
[`tools/verify-macos-release.sh`](tools/verify-macos-release.sh) notarizes and
staples the DMG, then checks it with Gatekeeper. It also checks the app extracted
from the updater archive. The workflow fails before upload if these checks fail.
For a release smoke test, download the published DMG through a browser on a
clean Mac, install it into Applications, and launch normally. Deny each feature
permission once to confirm the app still opens; then use the panel to grant it
and check that Surface capture and strict hide work independently.

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
window from another capture client's stream, so system-wide **Exclusion** is
reported unsupported. The Veil Surface uses ScreenCaptureKit to filter out
selected windows and OmniScreenVeil's own windows from its **main-display
capture** before any pixels reach the preview. It does not change what Zoom,
OBS, or a full-desktop share sees when they capture your desktop directly.
Strict hide uses the Accessibility `AXMinimized` attribute with your consent.

To test on a Mac: grant Screen Recording, place a harmless window on the main
display, and select **Surface** for it in Windows. Render a preview: the target
should be gone without moving or minimizing it locally. Share only the Veil
Surface in your actual capture app and check the receiver view. Move and resize
the sample window, open another window of the same app, then disarm and confirm
it comes back in the Surface.

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

On backends that mask by window position, the desktop app compares protected
window bounds before and after capture. If they differ, the Surface drops the
frame and retries. These samples are not atomic: a window that moves away and
back between samples can still be missed. If the Surface keeps retrying,
render a preview in the main window to see the capture error privately.

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
- No telemetry. The only network access is the signed update check against
  GitHub Releases when automatic checks are enabled or you press Check now.
- Format with `cargo fmt`, lint with `cargo clippy -- -D warnings`.
- Regenerate the dark-mode GIF with `bash tools/record-dark-demo.sh` after a
  Linux desktop build. It uses only an isolated Xvfb session and demo windows.
- Every `Native` capability in the matrix must have a capture test that proves
  it. If a test cannot run on a machine, it reports a skip, not a pass.
