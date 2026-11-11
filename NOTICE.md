# NOTICE

OmniScreenVeil is licensed under the GNU General Public License, version 3 or
later (GPL-3.0-or-later). See `LICENSE`.

## Third-party software

The Rust workspace depends on crates published on crates.io, including but not
limited to:

- `serde`, `serde_json` — Apache-2.0 / MIT
- `clap` — Apache-2.0 / MIT
- `thiserror`, `anyhow` — Apache-2.0 / MIT
- `x11rb` — Apache-2.0 / MIT
- `png`, `base64` — Apache-2.0 / MIT
- `windows-sys` — Apache-2.0 / MIT
- Tauri, `tauri-plugin-global-shortcut`, `wry`, `tao` — Apache-2.0 / MIT
- React, React DOM, Vite, TypeScript — MIT / Apache-2.0

The macOS helper uses Apple's system frameworks (AppKit, ApplicationServices,
CoreGraphics, ScreenCaptureKit) under the macOS SDK terms. No Apple source code
is vendored.

Run `cargo tree` and `pnpm licenses list` for the complete dependency set at a
given revision.

## Trademarks

Windows, macOS, Linux, X11, Wayland, KDE, GNOME, sway, Hyprland, Discord, Zoom,
OBS, Microsoft Teams, Google Meet and other names are trademarks of their
respective owners. OmniScreenVeil is not affiliated with, endorsed by, or
sponsored by any of them. Product names are used only to describe
compatibility.

## Research

`research/findings.md` contains product research and links to third-party
material. Those links and quotations remain the property of their authors and
are included for engineering context, not as a license grant.
