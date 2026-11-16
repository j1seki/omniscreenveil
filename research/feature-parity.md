# Reference-repo feature parity

Source research: `research/findings.md`. Repositories inspected at their current
default branch (shallow clones, not vendored):

| Project | Platform | Relevant features |
| --- | --- | --- |
| [k4yt3x/Evanesco](https://github.com/k4yt3x/Evanesco) | Windows | DLL injection, tray, **autohider for process lists**, select-all/process grouping |
| [Shaku-Med/hide-window](https://github.com/Shaku-Med/hide-window) | Windows | protection state model, **Unhide ALL (reset)**, keep-active, decoy cursor |
| [Vedant-29/mica](https://github.com/Vedant-29/mica) | macOS | **On/Off/Auto modes**, auto triggers (capture app, sharing, schedule), **crash-safe restore**, hide all/except |
| [D3OXY/Cloak](https://github.com/D3OXY/Cloak) | macOS | global hotkeys, **app exclusion**, placeholder modes, self-hiding surface |
| [CITGuru/clave](https://github.com/CITGuru/clave) | Win/mac | daemon reconciliation, zone policy |
| [Marksonthegamer/X11WithPrivacy](https://github.com/Marksonthegamer/X11WithPrivacy) | X11 | process access policy, daemon + rules file |
| [pingod/wayland-share-bridge](https://github.com/pingod/wayland-share-bridge) | Wayland | filtered-sharing fallback |

## Mutual gaps found

Features present in two or more references but missing/dead in this project:

1. **Automatic arming** (`AutoArm` was persisted and typed but never read).
   Evanesco autohides process lists; mica has Auto triggers; hide-window keeps
   protection state live.
2. **Continuous reconciliation**. All reference tools react to new windows or
   processes. The desktop app only reconciled on user actions.
3. **Bulk protect / protect all**. Evanesco select-all, mica hide-all, and
   hide-window's global hide/reset all provide a one-click way to cover the
   current desktop. This project only had per-window/per-app toggles.
4. Crash recovery (mica; hide-window documents the manual reset). **Not
   implemented** — see "Remaining gaps".
5. Keep-active / decoy cursor (hide-window). **Not implemented.**
6. Placeholder modes: blur/custom image/black (Cloak). Surface currently has
   solid/checker only. **Not implemented.**

## Implemented in this change

### 1. Auto-arm policy + background monitor

- New `veil-core::capture_apps` module: exact, case-normalized heuristic
  detection of capture apps by Windows executable stem, macOS bundle id, Linux
  app id, and display name. Deliberately avoids substring matching so
  `obsidian`/`observer` are not treated as OBS. Covers OBS, Zoom, Teams,
  Discord, Slack, WebEx, Skype, Snipping Tool, ShareX, Camtasia, Loom, Peek,
  Kooha, GNOME Screenshot, Flameshot, Spectacle, wf-recorder, etc.
- `Engine::should_arm(windows, explicit)` and `Engine::auto_tick(...)`:
  - `off`: only an explicit arm protects.
  - `always`: always protects.
  - `capture_app_running`: protects while a known capture app is present, or
    when explicitly armed; releases applied windows when the trigger disappears
    but keeps persistent rules.
- Desktop `AppState` now tracks explicit user intent (`armed`); "Arm now"
  sets it, "Release all" clears it. A 1.5 s background thread reconciles the
  live window list, so rules follow windows opened after startup and auto-arm
  reacts to capture apps appearing/disappearing.
- New `set_auto_arm` Tauri command and a selector in the Persistent rules card.
- The UI states the heuristic explicitly: a running capture app is a hint,
  never proof of a live share.

### 2. Protect all

- `Profile::protect_all(windows, mode)` adds one app rule per protectable app,
  deduplicates multiple windows of the same app, re-enables existing disabled
  app rules while preserving their configured mode, and skips empty/non-top-level
  windows.
- New `protect_all` Tauri command (filters out this process's own windows so the
  control UI cannot hide itself) and a "Protect all" button.
- Browser-preview mock mirrors the same behavior.

## Files changed

- `crates/veil-core/src/capture_apps.rs` (new)
- `crates/veil-core/src/lib.rs`
- `crates/veil-core/src/rules.rs`
- `crates/veil-protect/src/lib.rs`
- `apps/desktop/src-tauri/src/lib.rs`
- `apps/desktop/src/types.ts`
- `apps/desktop/src/api.ts`
- `apps/desktop/src/App.tsx`
- `apps/desktop/src/components/RulesCard.tsx`

## Verification (run on this machine)

```sh
cargo test -p veil-core -p veil-protect
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo check -p veil-core -p veil-protect -p veil-protect-win --target x86_64-pc-windows-msvc
cargo check -p veil-core -p veil-protect -p veil-protect-macos --target aarch64-apple-darwin
cargo run -p veil-cli -- selftest
pnpm --dir apps/desktop typecheck
pnpm --dir apps/desktop build
```

Results: 37 `veil-core` tests pass (including 4 capture-app tests and 3
`protect_all` tests); 14 `veil-protect` tests pass (including 4 auto-arm tests);
full workspace tests pass; clippy is clean; Windows/macOS cross-target checks
pass; CLI selftest reports 12/12; frontend typecheck and production build pass.

## Remaining gaps (not implemented here)

- Crash/restart recovery of windows left hidden by a previous run.
- Windows keep-active and decoy cursor.
- Surface placeholder modes beyond solid/checker (blur, image).
- Schedule/display-mirroring auto triggers (mica); only the running-capture-app
  heuristic and Always are implemented.
- Configurable trigger-app list (mica) instead of the built-in heuristic.
