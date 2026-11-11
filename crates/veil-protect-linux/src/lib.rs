//! X11 backend.
//!
//! X11 is the one Linux platform where a normal client can enumerate foreign
//! windows, read their geometry and capture the root window, so the Veil
//! Surface is fully functional here. True capture exclusion would require an
//! Xorg XACE module and is deliberately not implemented.
//!
//! The whole crate is Unix-only; on Windows it compiles to an empty library so
//! the workspace still builds, and the CLI gates every call site.

#![cfg(unix)]
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;
use veil_core::geometry::{Rect, Size};
use veil_core::ids::{AppKey, WindowId};
use veil_core::state::ProtectionIntent;
use veil_core::window::{Platform, WindowInfo};
use veil_core::ProtectionMode;
use veil_protect::{BackendError, ProtectionBackend};
use veil_surface::{Frame, PixelFormat};
use veil_verify::{CaptureError, CaptureSource};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt, CreateWindowAux, EventMask, ImageFormat, MapState, Window,
    WindowClass,
};
use x11rb::rust_connection::RustConnection;

#[derive(Debug, Error)]
pub enum X11Error {
    #[error("cannot connect to X11: {0}")]
    Connect(#[from] x11rb::errors::ConnectError),
    #[error("X11 request failed: {0}")]
    Request(#[from] x11rb::errors::ReplyOrIdError),
    #[error("X11 reply failed: {0}")]
    Reply(#[from] x11rb::errors::ReplyError),
    #[error("X11 image is {bits} bits per pixel, expected 24 or 32")]
    UnsupportedDepth { bits: u8 },
    #[error("window {0} does not exist")]
    NoSuchWindow(WindowId),
    #[error("X11 connection error: {0}")]
    Connection(#[from] x11rb::errors::ConnectionError),
}

#[derive(Debug, Clone)]
struct Atoms {
    client_list: Atom,
    wm_pid: Atom,
    wm_name: Atom,
    net_wm_name: Atom,
    net_wm_type: Atom,
    utf8: Atom,
    skip_types: Vec<Atom>,
}

impl Atoms {
    fn intern(conn: &RustConnection) -> Result<Self, X11Error> {
        let atom = |name: &[u8]| -> Result<Atom, X11Error> {
            Ok(conn.intern_atom(false, name)?.reply()?.atom)
        };
        Ok(Self {
            client_list: atom(b"_NET_CLIENT_LIST")?,
            wm_pid: atom(b"_NET_WM_PID")?,
            wm_name: atom(b"WM_NAME")?,
            net_wm_name: atom(b"_NET_WM_NAME")?,
            net_wm_type: atom(b"_NET_WM_WINDOW_TYPE")?,
            utf8: atom(b"UTF8_STRING")?,
            skip_types: [
                b"_NET_WM_WINDOW_TYPE_DESKTOP".as_slice(),
                b"_NET_WM_WINDOW_TYPE_DOCK",
                b"_NET_WM_WINDOW_TYPE_TOOLTIP",
                b"_NET_WM_WINDOW_TYPE_MENU",
                b"_NET_WM_WINDOW_TYPE_DROPDOWN_MENU",
                b"_NET_WM_WINDOW_TYPE_POPUP_MENU",
                b"_NET_WM_WINDOW_TYPE_NOTIFICATION",
                b"_NET_WM_WINDOW_TYPE_COMBO",
                b"_NET_WM_WINDOW_TYPE_DND",
            ]
            .iter()
            .filter_map(|n| atom(n).ok())
            .collect(),
        })
    }
}

pub struct X11Platform {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
}

impl X11Platform {
    pub fn connect() -> Result<Self, X11Error> {
        Self::connect_to(None)
    }

    pub fn connect_to(display: Option<&str>) -> Result<Self, X11Error> {
        let (conn, screen_num) = x11rb::connect(display)?;
        let root = conn.setup().roots[screen_num].root;
        let atoms = Atoms::intern(&conn)?;
        Ok(Self { conn, root, atoms })
    }

    pub fn root(&self) -> Window {
        self.root
    }

    pub fn root_size(&self) -> Result<Size, X11Error> {
        let geom = self.conn.get_geometry(self.root)?.reply()?;
        Ok(Size::new(geom.width as u32, geom.height as u32))
    }

    /// Every top-level window the window manager (or, without one, the server)
    /// reports. Falls back to walking the root's children when
    /// `_NET_CLIENT_LIST` is absent, which is the normal case on bare Xvfb.
    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, X11Error> {
        let mut ids = self.client_list()?;
        if ids.is_empty() {
            ids = self.root_children()?;
        }
        let mut out = Vec::new();
        for id in ids {
            if let Some(info) = self.describe_window(id)? {
                out.push(info);
            }
        }
        out.sort_by_key(|w| w.id);
        Ok(out)
    }

    fn client_list(&self) -> Result<Vec<Window>, X11Error> {
        let reply = self.conn.get_property(
            false,
            self.root,
            self.atoms.client_list,
            AtomEnum::WINDOW,
            0,
            u32::MAX,
        )?;
        let reply = reply.reply()?;
        Ok(reply
            .value32()
            .map(|values| values.collect::<Vec<u32>>())
            .unwrap_or_default())
    }

    fn root_children(&self) -> Result<Vec<Window>, X11Error> {
        let tree = self.conn.query_tree(self.root)?.reply()?;
        Ok(tree.children)
    }

    fn describe_window(&self, id: Window) -> Result<Option<WindowInfo>, X11Error> {
        let attrs = match self.conn.get_window_attributes(id) {
            Ok(cookie) => match cookie.reply() {
                Ok(reply) => reply,
                Err(_) => return Ok(None), // destroyed between listing and query
            },
            Err(_) => return Ok(None),
        };
        if attrs.override_redirect {
            return Ok(None);
        }
        let geom = match self.conn.get_geometry(id) {
            Ok(cookie) => match cookie.reply() {
                Ok(reply) => reply,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };
        if geom.width < 32 || geom.height < 32 {
            return Ok(None);
        }
        let translated = self
            .conn
            .translate_coordinates(id, self.root, 0, 0)?
            .reply()?;
        let wm_type = self.window_types(id)?;
        if wm_type
            .iter()
            .any(|t| self.atoms.skip_types.iter().any(|skip| skip == t))
        {
            return Ok(None);
        }

        let pid = self.cardinal(id, self.atoms.wm_pid).unwrap_or(0);
        // Never list our own windows: the app window, the tray helper and the
        // portal dialogs would otherwise appear as protectable targets.
        if pid != 0 && pid == std::process::id() {
            return Ok(None);
        }
        let class = self.wm_class(id).unwrap_or_default();
        let exe = if pid != 0 {
            std::fs::read_link(format!("/proc/{pid}/exe"))
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
        } else {
            None
        };
        let app_id = if class.is_empty() {
            exe.clone().unwrap_or_else(|| format!("window-{id}"))
        } else {
            class.clone()
        };
        let app = AppKey::LinuxAppId(app_id.clone());
        let app_name = if class.is_empty() {
            exe.as_deref()
                .and_then(|p| p.rsplit('/').next())
                .unwrap_or(&app_id)
                .to_string()
        } else {
            class
        };
        let title = self
            .text(id, self.atoms.net_wm_name, self.atoms.utf8)
            .or_else(|| self.text(id, self.atoms.wm_name, AtomEnum::STRING.into()))
            .unwrap_or_default();

        Ok(Some(WindowInfo {
            id: WindowId(id as u64),
            app,
            app_name,
            title,
            pid,
            bounds: Rect::new(
                translated.dst_x as i32,
                translated.dst_y as i32,
                geom.width as u32,
                geom.height as u32,
            ),
            monitor: None,
            visible: attrs.map_state == MapState::VIEWABLE,
            minimized: attrs.map_state != MapState::VIEWABLE,
            top_level: true,
            platform: Platform::LinuxX11,
        }))
    }

    fn window_types(&self, id: Window) -> Result<Vec<Atom>, X11Error> {
        let reply =
            self.conn
                .get_property(false, id, self.atoms.net_wm_type, AtomEnum::ATOM, 0, 64)?;
        let reply = reply.reply()?;
        Ok(reply
            .value32()
            .map(|values| values.collect::<Vec<u32>>())
            .unwrap_or_default())
    }

    fn cardinal(&self, id: Window, atom: Atom) -> Option<u32> {
        let reply = self
            .conn
            .get_property(false, id, atom, AtomEnum::CARDINAL, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        let value = reply.value32()?.next();
        value
    }

    fn wm_class(&self, id: Window) -> Option<String> {
        let reply = self
            .conn
            .get_property(false, id, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 4096)
            .ok()?
            .reply()
            .ok()?;
        let raw = String::from_utf8_lossy(&reply.value);
        raw.rsplit('\0')
            .find(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
    }

    fn text(&self, id: Window, atom: Atom, type_: Atom) -> Option<String> {
        let reply = self
            .conn
            .get_property(false, id, atom, type_, 0, 4096)
            .ok()?
            .reply()
            .ok()?;
        if reply.value.is_empty() {
            return None;
        }
        Some(String::from_utf8_lossy(&reply.value).into_owned())
    }

    /// Show or hide a foreign window. Strict-hide mode only.
    pub fn set_mapped(&self, window: WindowId, mapped: bool) -> Result<(), X11Error> {
        let id = window.0 as Window;
        if mapped {
            self.conn.map_window(id)?;
        } else {
            self.conn.unmap_window(id)?;
        }
        self.conn.flush()?;
        Ok(())
    }

    /// Bounds of this process's largest mapped window.
    ///
    /// The Veil Surface must never contain itself, otherwise the shared frame
    /// recurses into the app UI. The compositor paints the veiled placeholder
    /// over this rectangle instead.
    pub fn own_window_bounds(&self) -> Option<Rect> {
        let tree = self.conn.query_tree(self.root).ok()?.reply().ok()?;
        let pid = std::process::id();
        let mut best: Option<Rect> = None;
        for id in tree.children {
            let Some(attrs) = self
                .conn
                .get_window_attributes(id)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
            else {
                continue;
            };
            if attrs.map_state != MapState::VIEWABLE {
                continue;
            }
            if self.cardinal(id, self.atoms.wm_pid) != Some(pid) {
                continue;
            }
            let Some(geom) = self
                .conn
                .get_geometry(id)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
            else {
                continue;
            };
            if geom.width < 64 || geom.height < 64 {
                continue;
            }
            let Some(translated) = self
                .conn
                .translate_coordinates(id, self.root, 0, 0)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
            else {
                continue;
            };
            let rect = Rect::new(
                translated.dst_x as i32,
                translated.dst_y as i32,
                geom.width as u32,
                geom.height as u32,
            );
            if best.map_or(true, |current| rect.area() > current.area()) {
                best = Some(rect);
            }
        }
        best
    }

    /// Capture the whole virtual desktop as BGRA.
    pub fn capture_root(&self) -> Result<Frame, X11Error> {
        let geom = self.conn.get_geometry(self.root)?.reply()?;
        let image = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                0,
                0,
                geom.width,
                geom.height,
                u32::MAX,
            )?
            .reply()?;
        let bits = self
            .conn
            .setup()
            .pixmap_formats
            .iter()
            .find(|format| format.depth == image.depth)
            .map(|format| format.bits_per_pixel)
            .ok_or(X11Error::UnsupportedDepth { bits: image.depth })?;
        let pixels = match bits {
            32 => {
                let mut data = image.data;
                for pixel in data.chunks_exact_mut(4) {
                    pixel[3] = 255;
                }
                data
            }
            24 => {
                // Expand BGR into BGRA.
                let mut out = Vec::with_capacity(image.data.len() / 3 * 4);
                for pixel in image.data.chunks_exact(3) {
                    out.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
                }
                out
            }
            other => return Err(X11Error::UnsupportedDepth { bits: other }),
        };
        Frame::from_bytes(
            geom.width as u32,
            geom.height as u32,
            PixelFormat::Bgra8,
            pixels,
        )
        .map_err(|_| X11Error::UnsupportedDepth { bits })
    }
}

/// Capture source built on a dedicated X11 connection.
pub struct X11Capture {
    platform: X11Platform,
}

impl X11Capture {
    pub fn connect() -> Result<Self, X11Error> {
        Ok(Self {
            platform: X11Platform::connect()?,
        })
    }

    pub fn platform(&self) -> &X11Platform {
        &self.platform
    }

    pub fn root_size(&self) -> Result<Size, X11Error> {
        self.platform.root_size()
    }
}

impl CaptureSource for X11Capture {
    fn path(&self) -> veil_core::CapturePath {
        veil_core::CapturePath::X11GetImage
    }

    fn capture(&mut self) -> Result<Frame, CaptureError> {
        self.platform
            .capture_root()
            .map_err(|e| CaptureError::Failed(e.to_string()))
    }

    fn label(&self) -> String {
        "X11 root capture".to_string()
    }
}

/// X11 protection backend.
///
/// * `Surface` is a no-op at the OS level: the surface compositor does the work.
/// * `StrictHide` unmaps the window and remaps it on release.
/// * `Exclusion` is refused; the capability matrix already reports it as
///   unsupported, and the engine downgrades before we ever see it.
pub struct X11Backend {
    platform: X11Platform,
    hidden: BTreeSet<WindowId>,
}

impl X11Backend {
    pub fn connect() -> Result<Self, X11Error> {
        Ok(Self {
            platform: X11Platform::connect()?,
            hidden: BTreeSet::new(),
        })
    }

    pub fn platform(&self) -> &X11Platform {
        &self.platform
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, X11Error> {
        self.platform.list_windows()
    }

    pub fn hidden(&self) -> &BTreeSet<WindowId> {
        &self.hidden
    }

    pub fn own_window_bounds(&self) -> Option<Rect> {
        self.platform.own_window_bounds()
    }
}

impl ProtectionBackend for X11Backend {
    fn platform(&self) -> Platform {
        Platform::LinuxX11
    }

    fn apply(&mut self, intent: &ProtectionIntent) -> Result<(), BackendError> {
        match intent.mode {
            ProtectionMode::Surface => Ok(()),
            ProtectionMode::StrictHide => {
                self.platform
                    .set_mapped(intent.window, false)
                    .map_err(|e| BackendError::failed(e.to_string()))?;
                self.hidden.insert(intent.window);
                Ok(())
            }
            ProtectionMode::Exclusion => Err(BackendError::Unsupported {
                platform: Platform::LinuxX11,
                mode: ProtectionMode::Exclusion,
                reason: "requires a privileged Xorg XACE module".into(),
            }),
        }
    }

    fn release(&mut self, window: WindowId) -> Result<(), BackendError> {
        if !self.hidden.remove(&window) {
            return Ok(());
        }
        match self.platform.set_mapped(window, true) {
            Ok(()) => Ok(()),
            // The window may have been destroyed while hidden; that is not an
            // error, there is nothing left to restore.
            Err(X11Error::Request(x11rb::errors::ReplyOrIdError::X11Error(err)))
                if err.error_kind == x11rb::protocol::ErrorKind::Window =>
            {
                Ok(())
            }
            Err(e) => Err(BackendError::failed(e.to_string())),
        }
    }
}

/// Build a surface plan from a window list and the set of windows the engine
/// currently protects with `Surface`.
pub fn surface_plan(
    size: Size,
    windows: &[WindowInfo],
    protected: &BTreeMap<WindowId, ProtectionMode>,
    fill: veil_core::FillStyle,
    own_window: Option<Rect>,
) -> veil_surface::SurfacePlan {
    let mut plan = veil_surface::SurfacePlan::new(size, fill);
    for window in windows {
        if protected.get(&window.id).copied() == Some(ProtectionMode::Surface) {
            plan = plan.exclude(window.bounds);
        }
    }
    if let Some(own) = own_window {
        plan = plan.exclude(own);
    }
    plan
}

/// Convenience used by the CLI: a tiny X11 window that displays a canary, so
/// protection can be demonstrated end to end without touching the desktop.
pub fn create_canary_window(
    platform: &X11Platform,
    title: &str,
    frame: &Frame,
    origin: (i16, i16),
) -> Result<Window, X11Error> {
    let conn = &platform.conn;
    let wid = conn.generate_id()?;
    let root = platform.root;
    let aux = CreateWindowAux::new()
        .background_pixel(0x00202020)
        .event_mask(EventMask::EXPOSURE);
    conn.create_window(
        0,
        wid,
        root,
        origin.0,
        origin.1,
        frame.width as u16,
        frame.height as u16,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &aux,
    )?;
    let name = title.as_bytes();
    conn.change_property(
        x11rb::protocol::xproto::PropMode::REPLACE,
        wid,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        8,
        name.len() as u32,
        name,
    )?;
    conn.change_property(
        x11rb::protocol::xproto::PropMode::REPLACE,
        wid,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        8,
        name.len() as u32,
        name,
    )?;
    conn.map_window(wid)?;
    conn.flush()?;
    // Mapping paints the window background, so draw after the map.
    draw_frame(platform, wid, frame)?;
    Ok(wid)
}

/// Paint a frame into an existing X11 window (used by the canary test app).
pub fn draw_frame(platform: &X11Platform, window: Window, frame: &Frame) -> Result<(), X11Error> {
    let conn = &platform.conn;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &Default::default())?;
    conn.put_image(
        ImageFormat::Z_PIXMAP,
        window,
        gc,
        frame.width as u16,
        frame.height as u16,
        0,
        0,
        0,
        24,
        &frame.pixels,
    )?;
    conn.free_gc(gc)?;
    conn.flush()?;
    Ok(())
}
