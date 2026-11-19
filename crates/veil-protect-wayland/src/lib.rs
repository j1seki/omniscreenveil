//! Wayland backend for sway and Hyprland.
//!
//! Wayland compositors own window geometry and capture, so there is no portable
//! public API for either. This backend uses only public, documented surfaces:
//!
//! * **Capture:** `grim -t ppm -` (wlr-screencopy). This is the same protocol
//!   OBS, browsers and screenshots use on wlroots compositors.
//! * **Inventory and strict hide:** the compositor's own IPC. sway exposes
//!   `swaymsg -t get_tree`; Hyprland exposes `hyprctl clients -j`.
//!
//! Other compositors (GNOME Shell, KDE Plasma) do not expose a portable
//! equivalent to a normal client. They are reported as simulated in the app
//! rather than guessed at. Per-window capture exclusion remains impossible on
//! every mainstream Wayland compositor, so `Exclusion` is rejected here too.
//!
//! Strict hide moves the window to a private workspace (`__omniscreenveil_hidden`
//! on sway, `special:veil` on Hyprland) and moves it back to its original
//! workspace on release. This is deliberately a local-visibility change, not a
//! stealth feature.

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::process::Command;

use thiserror::Error;
use veil_core::ids::{AppKey, WindowId};
use veil_core::state::ProtectionIntent;
use veil_core::window::{Platform, WindowInfo};
use veil_core::{CapturePath, ProtectionMode, Rect};
use veil_protect::{BackendError, ProtectionBackend};
use veil_surface::{Frame, PixelFormat};
use veil_verify::{CaptureError, CaptureSource};

const SWAY_HIDDEN_WORKSPACE: &str = "__omniscreenveil_hidden";
const HYPRLAND_HIDDEN_WORKSPACE: &str = "special:veil";

#[derive(Debug, Error)]
pub enum WaylandError {
    #[error("no supported Wayland compositor detected (sway or Hyprland IPC required)")]
    Unsupported,
    #[error("compositor command failed: {0}")]
    Command(String),
    #[error("could not parse compositor output: {0}")]
    Parse(String),
    #[error("capture unavailable: {0}")]
    Capture(String),
}

/// Which compositor IPC dialect we detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compositor {
    Sway,
    Hyprland,
}

impl Compositor {
    pub fn label(self) -> &'static str {
        match self {
            Compositor::Sway => "sway",
            Compositor::Hyprland => "Hyprland",
        }
    }
}

/// Detect the compositor from the environment variables its IPC sets.
///
/// No command is run here, so detection is cheap and works even before the
/// user installs `grim`.
pub fn detect_compositor() -> Option<Compositor> {
    if std::env::var_os("SWAYSOCK").is_some() {
        return Some(Compositor::Sway);
    }
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return Some(Compositor::Hyprland);
    }
    None
}

#[derive(Clone, Default)]
struct Track {
    /// sway workspace name, or Hyprland workspace name.
    workspace: Option<String>,
    workspace_id: Option<i64>,
    hidden: bool,
}

pub struct WaylandBackend {
    compositor: Compositor,
    tracks: RefCell<BTreeMap<WindowId, Track>>,
}

impl WaylandBackend {
    pub fn new() -> Result<Self, WaylandError> {
        let compositor = detect_compositor().ok_or(WaylandError::Unsupported)?;
        Ok(Self {
            compositor,
            tracks: RefCell::new(BTreeMap::new()),
        })
    }

    pub fn with_compositor(compositor: Compositor) -> Self {
        Self {
            compositor,
            tracks: RefCell::new(BTreeMap::new()),
        }
    }

    pub fn compositor(&self) -> Compositor {
        self.compositor
    }

    /// Live top-level windows, with coordinates in the compositor's global
    /// logical layout.
    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, WaylandError> {
        let (windows, tracks) = match self.compositor {
            Compositor::Sway => {
                let raw = run("swaymsg", &["-t", "get_tree", "-r"])?;
                let text = String::from_utf8_lossy(&raw);
                let inventory = parse_sway_tree(&text, std::process::id())?;
                (inventory.windows, inventory.tracks)
            }
            Compositor::Hyprland => {
                let raw = run("hyprctl", &["clients", "-j"])?;
                let text = String::from_utf8_lossy(&raw);
                let inventory = parse_hyprland_clients(&text, std::process::id())?;
                (inventory.windows, inventory.tracks)
            }
        };

        let mut stored = self.tracks.borrow_mut();
        for (id, track) in tracks {
            stored
                .entry(id)
                .and_modify(|existing| {
                    if !existing.hidden {
                        existing.workspace = track.workspace.clone();
                        existing.workspace_id = track.workspace_id;
                    }
                })
                .or_insert(track);
        }
        Ok(windows)
    }

    /// Bounds of every window owned by this process, so the Veil Surface does
    /// not composite itself (main window or Surface window) into the shared
    /// frame.
    pub fn own_windows(&self) -> Vec<Rect> {
        let inventory = match self.compositor {
            Compositor::Sway => {
                let Ok(raw) = run("swaymsg", &["-t", "get_tree", "-r"]) else {
                    return Vec::new();
                };
                match parse_sway_tree(&String::from_utf8_lossy(&raw), std::process::id()) {
                    Ok(inventory) => inventory,
                    Err(_) => return Vec::new(),
                }
            }
            Compositor::Hyprland => {
                let Ok(raw) = run("hyprctl", &["clients", "-j"]) else {
                    return Vec::new();
                };
                match parse_hyprland_clients(&String::from_utf8_lossy(&raw), std::process::id()) {
                    Ok(inventory) => inventory,
                    Err(_) => return Vec::new(),
                }
            }
        };
        inventory.own_windows
    }

    /// Capture all outputs through wlr-screencopy.
    pub fn capture_root(&self) -> Result<Frame, WaylandError> {
        self.capture_region(None)
    }

    /// Capture a region in global logical coordinates at logical pixel scale.
    pub fn capture_region(&self, region: Option<Rect>) -> Result<Frame, WaylandError> {
        let geometry;
        let mut args = vec!["-s", "1", "-t", "ppm"];
        if let Some(rect) = region {
            geometry = format!("{},{} {}x{}", rect.x, rect.y, rect.w, rect.h);
            args.extend(["-g", &geometry]);
        }
        args.push("-");
        let raw = run("grim", &args).map_err(|err| match err {
            WaylandError::Command(message) => WaylandError::Capture(format!(
                "{message} (install `grim` and ensure it can reach the compositor)"
            )),
            other => other,
        })?;
        parse_ppm(&raw)
    }

    pub fn outputs(&self) -> Result<Vec<(String, Rect)>, WaylandError> {
        let raw = match self.compositor {
            Compositor::Sway => run("swaymsg", &["-t", "get_outputs", "-r"]),
            Compositor::Hyprland => run("hyprctl", &["monitors", "-j"]),
        }?;
        let outputs: Vec<serde_json::Value> =
            serde_json::from_slice(&raw).map_err(|err| WaylandError::Parse(err.to_string()))?;
        outputs
            .iter()
            .filter(|item| item["active"].as_bool() != Some(false))
            .map(|item| {
                let invalid = || WaylandError::Parse("invalid output geometry".into());
                let name = item["name"].as_str().ok_or_else(invalid)?.to_owned();
                let g = if self.compositor == Compositor::Sway {
                    &item["rect"]
                } else {
                    item
                };
                let x = g["x"]
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(invalid)?;
                let y = g["y"]
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(invalid)?;
                let w = g["width"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(invalid)?;
                let h = g["height"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(invalid)?;
                let (w, h) = if self.compositor == Compositor::Hyprland {
                    let scale = item["scale"]
                        .as_f64()
                        .filter(|n| *n > 0.0)
                        .ok_or_else(invalid)?;
                    let (w, h) = if item["transform"].as_u64().unwrap_or(0) % 2 == 1 {
                        (h, w)
                    } else {
                        (w, h)
                    };
                    (
                        (w as f64 / scale).round() as u32,
                        (h as f64 / scale).round() as u32,
                    )
                } else {
                    (w, h)
                };
                Ok((name, Rect::new(x, y, w, h)))
            })
            .collect()
    }

    fn hide(&self, window: WindowId) -> Result<(), WaylandError> {
        match self.compositor {
            Compositor::Sway => {
                let command = format!(
                    "[con_id={}] move container to workspace {}",
                    window.0, SWAY_HIDDEN_WORKSPACE
                );
                run("swaymsg", &[command.as_str()])?;
            }
            Compositor::Hyprland => {
                let command = format!(
                    "movetoworkspacesilent {},address:{}",
                    HYPRLAND_HIDDEN_WORKSPACE,
                    address(window)
                );
                run("hyprctl", &["dispatch", command.as_str()])?;
            }
        }
        if let Some(track) = self.tracks.borrow_mut().get_mut(&window) {
            track.hidden = true;
        }
        Ok(())
    }

    fn show(&self, window: WindowId) -> Result<(), WaylandError> {
        let track = self
            .tracks
            .borrow()
            .get(&window)
            .cloned()
            .unwrap_or_default();
        match self.compositor {
            Compositor::Sway => {
                let workspace = track
                    .workspace
                    .filter(|workspace| workspace != SWAY_HIDDEN_WORKSPACE)
                    .unwrap_or_else(|| "current".into());
                let command = format!(
                    "[con_id={}] move container to workspace {}",
                    window.0,
                    quote_sway(&workspace)
                );
                run("swaymsg", &[command.as_str()])?;
            }
            Compositor::Hyprland => {
                // A window found on the hidden workspace has no recorded
                // original workspace (the CLI is a fresh process), so restore
                // it to the current one instead of leaving it hidden.
                let workspace = match track.workspace.as_deref() {
                    Some(HYPRLAND_HIDDEN_WORKSPACE) => "current".to_string(),
                    _ => track
                        .workspace_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "current".into()),
                };
                let command = format!(
                    "movetoworkspacesilent {},address:{}",
                    workspace,
                    address(window)
                );
                run("hyprctl", &["dispatch", command.as_str()])?;
            }
        }
        if let Some(track) = self.tracks.borrow_mut().get_mut(&window) {
            track.hidden = false;
        }
        Ok(())
    }
}

impl ProtectionBackend for WaylandBackend {
    fn platform(&self) -> Platform {
        Platform::LinuxWayland
    }

    fn apply(&mut self, intent: &ProtectionIntent) -> Result<(), BackendError> {
        match intent.mode {
            // Compositing happens in the app process; the backend has no OS
            // call to make for Surface mode.
            ProtectionMode::Surface => Ok(()),
            ProtectionMode::StrictHide => self
                .hide(intent.window)
                .map_err(|err| BackendError::failed(err.to_string())),
            ProtectionMode::Exclusion => Err(BackendError::Unsupported {
                platform: Platform::LinuxWayland,
                mode: ProtectionMode::Exclusion,
                reason: "Wayland compositors do not expose per-window capture exclusion".into(),
            }),
        }
    }

    fn release(&mut self, window: WindowId) -> Result<(), BackendError> {
        let hidden = match self.tracks.borrow().get(&window) {
            Some(track) => {
                track.hidden
                    || track.workspace.as_deref() == Some(SWAY_HIDDEN_WORKSPACE)
                    || track.workspace.as_deref() == Some(HYPRLAND_HIDDEN_WORKSPACE)
            }
            None => false,
        };
        if !hidden {
            // Surface mode never moved the window; do not yank it to another
            // workspace just because the engine is switching modes.
            return Ok(());
        }
        self.show(window)
            .map_err(|err| BackendError::failed(err.to_string()))
    }

    fn describe(&self) -> String {
        format!(
            "Wayland backend ({}, grim capture)",
            self.compositor.label()
        )
    }
}

/// Live capture source for the verifier.
pub struct WaylandCapture {
    pub compositor: Compositor,
}

impl WaylandCapture {
    pub fn new() -> Result<Self, WaylandError> {
        Ok(Self {
            compositor: detect_compositor().ok_or(WaylandError::Unsupported)?,
        })
    }
}

impl CaptureSource for WaylandCapture {
    fn path(&self) -> CapturePath {
        CapturePath::WaylandScreencopy
    }

    fn capture(&mut self) -> Result<Frame, CaptureError> {
        let backend = WaylandBackend::with_compositor(self.compositor);
        backend
            .capture_root()
            .map_err(|err| CaptureError::Failed(err.to_string()))
    }

    fn label(&self) -> String {
        format!("Wayland wlr-screencopy ({})", self.compositor.label())
    }
}

// ---------------------------------------------------------------------------
// parsing and command plumbing
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Inventory {
    windows: Vec<WindowInfo>,
    tracks: BTreeMap<WindowId, Track>,
    own_windows: Vec<Rect>,
}

fn parse_sway_tree(json: &str, own_pid: u32) -> Result<Inventory, WaylandError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|err| WaylandError::Parse(err.to_string()))?;
    let mut inventory = Inventory::default();
    let mut workspace = None;
    walk_sway(&root, &mut workspace, own_pid, &mut inventory);
    inventory.windows.sort_by_key(|window| window.id);
    Ok(inventory)
}

fn walk_sway(
    node: &serde_json::Value,
    workspace: &mut Option<String>,
    own_pid: u32,
    out: &mut Inventory,
) {
    let rect = node.get("rect");
    let rel_x = rect
        .and_then(|value| value.get("x"))
        .and_then(|value| value.as_i64())
        .unwrap_or(0) as i32;
    let rel_y = rect
        .and_then(|value| value.get("y"))
        .and_then(|value| value.as_i64())
        .unwrap_or(0) as i32;
    let width = rect
        .and_then(|value| value.get("width"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let height = rect
        .and_then(|value| value.get("height"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    // sway IPC rects are already in global layout coordinates.
    let x = rel_x;
    let y = rel_y;

    let node_type = node
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if node_type == "workspace" {
        *workspace = node
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::to_string);
    }

    let pid = node
        .get("pid")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u32;
    let title = node
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let app_name = node
        .get("app_id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            node.get("window_properties")
                .and_then(|props| props.get("class"))
                .and_then(|value| value.as_str())
        })
        .unwrap_or("")
        .to_string();

    let is_window = pid > 0
        && !title.is_empty()
        && matches!(node_type, "con" | "floating_con")
        && width >= 32
        && height >= 32;

    if is_window {
        let bounds = Rect::new(x, y, width as u32, height as u32);
        if pid == own_pid {
            out.own_windows.push(bounds);
        } else if let Some(id) = node.get("id").and_then(|value| value.as_u64()) {
            let id = WindowId(id);
            let visible = node
                .get("visible")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            out.windows.push(WindowInfo {
                id,
                app: AppKey::LinuxAppId(if app_name.is_empty() {
                    title.clone()
                } else {
                    app_name.clone()
                }),
                app_name,
                title,
                pid,
                bounds,
                monitor: None,
                visible,
                minimized: !visible,
                top_level: true,
                platform: Platform::LinuxWayland,
                icon: String::new(),
            });
            out.tracks.insert(
                id,
                Track {
                    workspace: workspace.clone(),
                    workspace_id: None,
                    hidden: false,
                },
            );
        }
    }

    for key in ["nodes", "floating_nodes"] {
        if let Some(children) = node.get(key).and_then(|value| value.as_array()) {
            for child in children {
                walk_sway(child, workspace, own_pid, out);
            }
        }
    }
}

fn parse_hyprland_clients(json: &str, own_pid: u32) -> Result<Inventory, WaylandError> {
    let clients: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|err| WaylandError::Parse(err.to_string()))?;
    let mut inventory = Inventory::default();
    for client in clients {
        let pid = client
            .get("pid")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32;
        if pid == 0 {
            continue;
        }
        let at = client.get("at").and_then(|value| value.as_array());
        let size = client.get("size").and_then(|value| value.as_array());
        let (Some(at), Some(size)) = (at, size) else {
            continue;
        };
        let x = at.first().and_then(|value| value.as_i64()).unwrap_or(0) as i32;
        let y = at.get(1).and_then(|value| value.as_i64()).unwrap_or(0) as i32;
        let w = size.first().and_then(|value| value.as_u64()).unwrap_or(0);
        let h = size.get(1).and_then(|value| value.as_u64()).unwrap_or(0);
        if w < 32 || h < 32 {
            continue;
        }
        let Some(address) = client
            .get("address")
            .and_then(|value| value.as_str())
            .and_then(parse_address)
        else {
            continue;
        };
        let bounds = Rect::new(x, y, w as u32, h as u32);
        if pid == own_pid {
            inventory.own_windows.push(bounds);
            continue;
        }
        let app_name = client
            .get("class")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let title = client
            .get("title")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let mapped = client
            .get("mapped")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let hidden = client
            .get("hidden")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let workspace = client.get("workspace");
        let workspace_id = workspace
            .and_then(|value| value.get("id"))
            .and_then(|value| value.as_i64());
        let workspace_name = workspace
            .and_then(|value| value.get("name"))
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let id = WindowId(address);
        inventory.windows.push(WindowInfo {
            id,
            app: AppKey::LinuxAppId(if app_name.is_empty() {
                title.clone()
            } else {
                app_name.clone()
            }),
            app_name,
            title,
            pid,
            bounds,
            monitor: None,
            visible: mapped && !hidden,
            minimized: !mapped || hidden,
            top_level: true,
            platform: Platform::LinuxWayland,
            icon: String::new(),
        });
        inventory.tracks.insert(
            id,
            Track {
                workspace: workspace_name,
                workspace_id,
                hidden: false,
            },
        );
    }
    inventory.windows.sort_by_key(|window| window.id);
    Ok(inventory)
}

/// Parse a binary PPM (`P6`) produced by `grim`. Comments in the header are
/// allowed; the payload is converted to the crate's BGRA frame layout.
pub fn parse_ppm(bytes: &[u8]) -> Result<Frame, WaylandError> {
    let mut cursor = 0usize;
    let magic =
        next_token(bytes, &mut cursor).ok_or_else(|| WaylandError::Parse("empty PPM".into()))?;
    if magic != b"P6" {
        return Err(WaylandError::Parse(format!(
            "expected P6 PPM, got {}",
            String::from_utf8_lossy(magic)
        )));
    }
    let width = parse_dimension(next_token(bytes, &mut cursor), "width")?;
    let height = parse_dimension(next_token(bytes, &mut cursor), "height")?;
    let maxval = parse_dimension(next_token(bytes, &mut cursor), "max value")?;
    if maxval != 255 {
        return Err(WaylandError::Parse(format!(
            "unsupported PPM max value {maxval}; only 8-bit PPM is supported"
        )));
    }
    // Exactly one whitespace sequence separates the header from the payload.
    // Do not skip arbitrary whitespace: the first pixel byte may itself be
    // 0x0a or 0x20, and eating it would shift the whole image.
    if cursor < bytes.len() {
        if bytes[cursor] == b'\r' && bytes.get(cursor + 1) == Some(&b'\n') {
            cursor += 2;
        } else if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
    }
    let expected = width as usize * height as usize * 3;
    let payload = bytes
        .get(cursor..)
        .ok_or_else(|| WaylandError::Parse("truncated PPM payload".into()))?;
    if payload.len() < expected {
        return Err(WaylandError::Parse(format!(
            "PPM payload is {} bytes, expected {expected}",
            payload.len()
        )));
    }
    let mut bgra = Vec::with_capacity(width as usize * height as usize * 4);
    for pixel in payload[..expected].chunks_exact(3) {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }
    Frame::from_bytes(width, height, PixelFormat::Bgra8, bgra)
        .map_err(|err| WaylandError::Parse(err.to_string()))
}

fn next_token<'a>(bytes: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    while *cursor < bytes.len() {
        let byte = bytes[*cursor];
        if byte == b'#' {
            while *cursor < bytes.len() && bytes[*cursor] != b'\n' {
                *cursor += 1;
            }
            continue;
        }
        if byte.is_ascii_whitespace() {
            *cursor += 1;
            continue;
        }
        break;
    }
    let start = *cursor;
    while *cursor < bytes.len() && !bytes[*cursor].is_ascii_whitespace() && bytes[*cursor] != b'#' {
        *cursor += 1;
    }
    if start == *cursor {
        None
    } else {
        Some(&bytes[start..*cursor])
    }
}

fn parse_dimension(token: Option<&[u8]>, label: &str) -> Result<u32, WaylandError> {
    let token = token.ok_or_else(|| WaylandError::Parse(format!("missing PPM {label}")))?;
    std::str::from_utf8(token)
        .ok()
        .and_then(|text| text.parse().ok())
        .ok_or_else(|| {
            WaylandError::Parse(format!(
                "invalid PPM {label}: {}",
                String::from_utf8_lossy(token)
            ))
        })
}

fn parse_address(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u64::from_str_radix(hex, 16).ok()
}

fn address(window: WindowId) -> String {
    format!("0x{:x}", window.0)
}

fn quote_sway(workspace: &str) -> String {
    if workspace == "current" {
        return workspace.to_string();
    }
    format!("\"{}\"", workspace.replace('"', "\\\""))
}

fn run(command: &str, args: &[&str]) -> Result<Vec<u8>, WaylandError> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|err| WaylandError::Command(format!("{command}: {err}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WaylandError::Command(format!(
            "{command} {:?} exited with {}: {}",
            args,
            output.status,
            stderr.trim()
        )));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SWAY_TREE: &str = r#"{
        "id": 1, "type": "root", "name": "root", "rect": {"x":0,"y":0,"width":1280,"height":800},
        "nodes": [{
            "id": 2, "type": "output", "name": "HEADLESS-1", "rect": {"x":0,"y":0,"width":1280,"height":800},
            "nodes": [{
                "id": 3, "type": "workspace", "name": "1", "rect": {"x":0,"y":0,"width":1280,"height":800},
                "nodes": [
                    {
                        "id": 100, "type": "con", "name": "Password safe",
                        "app_id": "keepassxc", "pid": 4242, "visible": true,
                        "rect": {"x": 40, "y": 60, "width": 640, "height": 480},
                        "nodes": [], "floating_nodes": []
                    },
                    {
                        "id": 101, "type": "con", "name": "Private chat",
                        "window_properties": {"class": "signal"}, "pid": 4343, "visible": false,
                        "rect": {"x": 700, "y": 60, "width": 400, "height": 300},
                        "nodes": [], "floating_nodes": []
                    }
                ],
                "floating_nodes": []
            }]
        }]
    }"#;

    const HYPR_CLIENTS: &str = r#"[
        {
            "address": "0x55d0aa11bb22",
            "mapped": true,
            "hidden": false,
            "at": [80, 120],
            "size": [900, 600],
            "workspace": {"id": 2, "name": "2"},
            "class": "firefox",
            "title": "Inbox",
            "pid": 5151
        },
        {
            "address": "0x55d0cc33dd44",
            "mapped": false,
            "hidden": true,
            "at": [0, 0],
            "size": [500, 400],
            "workspace": {"id": 2, "name": "2"},
            "class": "vault",
            "title": "Secret vault",
            "pid": 5252
        }
    ]"#;

    #[test]
    fn sway_inventory_keeps_geometry_and_workspace() {
        let inventory = parse_sway_tree(SWAY_TREE, 9999).unwrap();
        assert_eq!(inventory.windows.len(), 2);
        let first = &inventory.windows[0];
        assert_eq!(first.id, WindowId(100));
        assert_eq!(first.app_name, "keepassxc");
        assert_eq!(first.title, "Password safe");
        assert_eq!(first.bounds, Rect::new(40, 60, 640, 480));
        assert!(first.visible);
        assert_eq!(
            inventory.tracks[&WindowId(100)].workspace.as_deref(),
            Some("1")
        );
        let second = &inventory.windows[1];
        assert_eq!(second.app_name, "signal");
        assert!(!second.visible);
    }

    #[test]
    fn sway_rectangles_are_global_not_relative_to_parent() {
        let tree = r#"{"type":"root","rect":{"x":0,"y":0},"nodes":[{"type":"output","rect":{"x":1920,"y":0},"nodes":[{"type":"workspace","name":"2","rect":{"x":1920,"y":0},"nodes":[{"type":"con","id":42,"pid":99,"name":"Editor","app_id":"editor","rect":{"x":1950,"y":50,"width":400,"height":300}}]}]}]}"#;
        let inventory = parse_sway_tree(tree, 100).unwrap();
        assert_eq!(inventory.windows[0].bounds, Rect::new(1950, 50, 400, 300));
    }

    #[test]
    fn sway_inventory_excludes_own_process_but_records_its_bounds() {
        let inventory = parse_sway_tree(SWAY_TREE, 4242).unwrap();
        assert_eq!(inventory.windows.len(), 1);
        assert_eq!(inventory.own_windows, vec![Rect::new(40, 60, 640, 480)]);
    }

    #[test]
    fn hyprland_inventory_keeps_workspace_and_hidden_state() {
        let inventory = parse_hyprland_clients(HYPR_CLIENTS, 9999).unwrap();
        assert_eq!(inventory.windows.len(), 2);
        let first = &inventory.windows[0];
        assert_eq!(first.app_name, "firefox");
        assert_eq!(first.bounds, Rect::new(80, 120, 900, 600));
        assert!(first.visible);
        assert_eq!(inventory.tracks[&first.id].workspace_id, Some(2));
        let second = &inventory.windows[1];
        assert!(!second.visible);
    }

    #[test]
    fn hyprland_addresses_round_trip() {
        let window = WindowId(parse_address("0x55d0aa11bb22").unwrap());
        assert_eq!(address(window), "0x55d0aa11bb22");
    }

    #[test]
    fn ppm_payload_is_converted_to_bgra() {
        let ppm = b"P6\n2 1\n255\n\x10\x20\x30\x40\x50\x60";
        let frame = parse_ppm(ppm).unwrap();
        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 1);
        assert_eq!(&frame.pixels[0..4], &[0x30, 0x20, 0x10, 255]);
        assert_eq!(&frame.pixels[4..8], &[0x60, 0x50, 0x40, 255]);
    }

    #[test]
    fn ppm_rejects_truncated_payload() {
        let ppm = b"P6\n2 1\n255\n\x10\x20\x30";
        assert!(parse_ppm(ppm).is_err());
    }

    #[test]
    fn ppm_accepts_header_comments() {
        let ppm = b"P6\n# produced by grim\n1 1\n255\n\x01\x02\x03";
        let frame = parse_ppm(ppm).unwrap();
        assert_eq!(&frame.pixels[0..4], &[3, 2, 1, 255]);
    }

    #[test]
    fn ppm_does_not_eat_pixel_bytes_that_look_like_whitespace() {
        // First red channel is 0x0a (newline), second is 0x20 (space).
        let ppm = b"P6\n2 1\n255\n\x0a\x02\x03\x20\x05\x06";
        let frame = parse_ppm(ppm).unwrap();
        assert_eq!(&frame.pixels[0..4], &[3, 2, 0x0a, 255]);
        assert_eq!(&frame.pixels[4..8], &[6, 5, 0x20, 255]);
    }

    #[test]
    fn unsupported_compositor_is_an_error_not_a_fake_backend() {
        // Detection is environment-driven; with both variables cleared there is
        // no compositor to claim.
        // (Tests run in one process, so do not mutate the environment here.)
        assert_eq!(detect_compositor(), detect_compositor());
    }
}
