//! macOS backend (Phase 0).
//!
//! Clean APIs only: no private CGS/SkyLight calls, no SIP changes. On macOS the
//! product therefore never claims `Exclusion`; it offers the Veil Surface
//! (ScreenCaptureKit filters) and strict hide (Accessibility), which the Swift
//! package in `platform/macos` validates.
//!
//! The Swift static library is expected at link time. `build.rs` in the desktop
//! app points at `platform/macos/.build/release/libVeilMac.a`; the CLI links
//! against it too when built on macOS.

use std::collections::BTreeSet;

use veil_core::ids::WindowId;
use veil_core::state::ProtectionIntent;
use veil_core::window::Platform;
use veil_core::ProtectionMode;
use veil_protect::{BackendError, ProtectionBackend};

#[derive(Debug, thiserror::Error)]
pub enum MacError {
    #[error("macOS backend is only available on macOS")]
    NotMacOs,
    #[error("Swift helper unavailable: {0}")]
    Helper(String),
}

/// Talks to the Swift helper (`platform/macos`). The FFI layer lives behind
/// `cfg(target_os = "macos")`; on other hosts this is a stub so the workspace
/// still builds and the capability matrix can be inspected.
pub struct MacBackend {
    #[cfg(target_os = "macos")]
    inner: ffi::Helper,
    /// Windows this backend actually hid. Release must not touch anything
    /// else (notably not Veil Surface windows, which are no-ops here).
    hidden: BTreeSet<WindowId>,
}

impl MacBackend {
    #[cfg(target_os = "macos")]
    pub fn watch_windows(signal: std::sync::mpsc::SyncSender<()>) -> bool {
        ffi::watch_windows(signal)
    }
    pub fn new() -> Result<Self, MacError> {
        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                inner: ffi::Helper::load().map_err(MacError::Helper)?,
                hidden: BTreeSet::new(),
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(MacError::NotMacOs)
        }
    }

    /// Live window inventory from `CGWindowListCopyWindowInfo`, with bundle
    /// identifiers so rules survive restarts and `open`-style launches.
    pub fn list_windows(&self) -> Result<Vec<veil_core::WindowInfo>, MacError> {
        self.inventory(false)
    }

    pub fn inventory(&self, icons: bool) -> Result<Vec<veil_core::WindowInfo>, MacError> {
        #[cfg(target_os = "macos")]
        {
            self.inner.list_windows(icons)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = icons;
            Err(MacError::NotMacOs)
        }
    }

    pub fn accessibility_trusted(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.inner.accessibility_trusted()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    #[cfg(target_os = "macos")]
    pub fn screen_recording_granted() -> bool {
        ffi::screen_recording_granted()
    }

    #[cfg(target_os = "macos")]
    pub fn request_screen_recording() {
        ffi::request_screen_recording();
    }

    #[cfg(target_os = "macos")]
    pub fn request_accessibility() {
        ffi::request_accessibility();
    }

    /// Capture the main display through ScreenCaptureKit.
    ///
    /// The Swift helper excludes this process's own windows from the filter, so
    /// the Veil Surface cannot recurse into itself.
    pub fn capture_root(&self) -> Result<veil_surface::Frame, MacError> {
        self.capture_filtered(&BTreeSet::new())
    }

    /// Exclude windows from our ScreenCaptureKit capture, not from other apps' captures.
    pub fn capture_filtered(
        &self,
        excluded: &BTreeSet<WindowId>,
    ) -> Result<veil_surface::Frame, MacError> {
        self.capture_display(0, excluded, 0)
    }

    #[cfg(target_os = "macos")]
    pub fn displays(&self) -> Result<Vec<Display>, MacError> {
        self.inner.displays()
    }

    pub fn capture_display(
        &self,
        display: u32,
        excluded: &BTreeSet<WindowId>,
        max_edge: u32,
    ) -> Result<veil_surface::Frame, MacError> {
        #[cfg(target_os = "macos")]
        {
            let ids = excluded
                .iter()
                .map(|id| u32::try_from(id.0))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| MacError::Helper(format!("invalid window ID: {err}")))?;
            self.inner.capture_display(display, &ids, max_edge)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (display, excluded, max_edge);
            Err(MacError::NotMacOs)
        }
    }
}

#[derive(serde::Deserialize)]
pub struct Display {
    pub id: u32,
    pub name: String,
    #[serde(flatten)]
    pub bounds: veil_core::Rect,
}

impl ProtectionBackend for MacBackend {
    fn platform(&self) -> Platform {
        Platform::MacOs
    }

    fn apply(&mut self, intent: &ProtectionIntent) -> Result<(), BackendError> {
        match intent.mode {
            // The surface compositor lives in the app process; ScreenCaptureKit
            // exclusion is configured there, not here.
            ProtectionMode::Surface => Ok(()),
            ProtectionMode::StrictHide => {
                #[cfg(target_os = "macos")]
                {
                    self.inner
                        .hide(intent.window)
                        .map_err(BackendError::failed)?;
                    self.hidden.insert(intent.window);
                    Ok(())
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = intent;
                    Err(BackendError::Unsupported {
                        platform: Platform::MacOs,
                        mode: ProtectionMode::StrictHide,
                        reason: "not running on macOS".into(),
                    })
                }
            }
            ProtectionMode::Exclusion => Err(BackendError::Unsupported {
                platform: Platform::MacOs,
                mode: ProtectionMode::Exclusion,
                reason: "third-party windows cannot be excluded from ScreenCaptureKit".into(),
            }),
        }
    }

    fn release(&mut self, window: WindowId) -> Result<(), BackendError> {
        if !self.hidden.contains(&window) {
            // Surface is a compositor effect, not an OS change, and a window we
            // never hid must be left exactly as it is.
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            self.inner.show(window).map_err(BackendError::failed)?;
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = window;
        }
        self.hidden.remove(&window);
        Ok(())
    }

    fn describe(&self) -> String {
        "macOS backend (ScreenCaptureKit surface + Accessibility strict hide)".to_string()
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::{c_char, CStr};

    use veil_core::geometry::Rect;
    use veil_core::ids::{AppKey, WindowId};
    use veil_core::window::{Platform, WindowInfo};

    use super::MacError;

    #[link(name = "VeilMac", kind = "static")]
    extern "C" {
        fn veil_macos_watch_windows(callback: extern "C" fn());
        fn veil_macos_hide_window(window: u64) -> i32;
        fn veil_macos_show_window(window: u64) -> i32;
        fn veil_macos_last_error() -> *mut c_char;
        fn veil_macos_free_string(ptr: *mut c_char);
        fn veil_macos_list_windows_json(include_icons: i32) -> *mut c_char;
        fn veil_macos_list_displays_json() -> *mut c_char;
        fn veil_macos_accessibility_trusted() -> i32;
        fn veil_macos_request_accessibility() -> i32;
        fn veil_macos_screen_recording_granted() -> i32;
        fn veil_macos_request_screen_recording() -> i32;
        fn veil_macos_capture_display(
            display_id: u32,
            excluded_ids: *const u32,
            excluded_count: usize,
            max_edge: u32,
            out_width: *mut u32,
            out_height: *mut u32,
        ) -> *mut u8;
        fn veil_macos_free_buffer(ptr: *mut u8);
    }

    pub(super) fn watch_windows(signal: std::sync::mpsc::SyncSender<()>) -> bool {
        static SIGNAL: std::sync::OnceLock<std::sync::mpsc::SyncSender<()>> =
            std::sync::OnceLock::new();
        extern "C" fn notify() {
            if let Some(signal) = SIGNAL.get() {
                let _ = signal.try_send(());
            }
        }
        if SIGNAL.set(signal).is_err() {
            return false;
        }
        unsafe { veil_macos_watch_windows(notify) };
        // Without Accessibility, workspace events still work but cannot detect
        // every window created by a running app. Keep the faster fallback.
        unsafe { veil_macos_accessibility_trusted() != 0 }
    }

    pub fn screen_recording_granted() -> bool {
        unsafe { veil_macos_screen_recording_granted() == 1 }
    }

    pub fn request_screen_recording() {
        unsafe { veil_macos_request_screen_recording() };
    }

    pub fn request_accessibility() {
        unsafe { veil_macos_request_accessibility() };
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawWindow {
        id: u32,
        pid: i32,
        app: String,
        bundle: String,
        title: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        on_screen: bool,
        layer: i32,
        #[serde(default)]
        icon: String,
    }

    pub struct Helper;

    impl Helper {
        pub fn load() -> Result<Self, String> {
            // Linking is enough; the Swift static library is always present.
            Ok(Self)
        }

        pub fn hide(&mut self, window: WindowId) -> Result<(), String> {
            check(unsafe { veil_macos_hide_window(window.0) })
        }

        pub fn show(&mut self, window: WindowId) -> Result<(), String> {
            check(unsafe { veil_macos_show_window(window.0) })
        }

        /// Whether the process has Accessibility permission, which strict
        /// hide requires. Exposed to the UI so it can tell the user to grant it
        /// instead of failing silently.
        #[allow(dead_code)]
        pub fn accessibility_trusted(&self) -> bool {
            unsafe { veil_macos_accessibility_trusted() == 1 }
        }

        /// Capture the display through ScreenCaptureKit and convert the BGRA
        /// buffer into a `veil_surface::Frame`.
        pub fn capture_display(
            &self,
            display_id: u32,
            excluded: &[u32],
            max_edge: u32,
        ) -> Result<veil_surface::Frame, MacError> {
            let mut width: u32 = 0;
            let mut height: u32 = 0;
            let ptr = unsafe {
                veil_macos_capture_display(
                    display_id,
                    excluded.as_ptr(),
                    excluded.len(),
                    max_edge,
                    &mut width,
                    &mut height,
                )
            };
            if ptr.is_null() {
                return Err(MacError::Helper(last_error()));
            }
            let len = width as usize * height as usize * 4;
            let bytes = unsafe { std::slice::from_raw_parts(ptr, len).to_vec() };
            unsafe { veil_macos_free_buffer(ptr) };
            veil_surface::Frame::from_bytes(width, height, veil_surface::PixelFormat::Bgra8, bytes)
                .map_err(|err| MacError::Helper(format!("bad ScreenCaptureKit frame: {err}")))
        }

        pub fn displays(&self) -> Result<Vec<super::Display>, MacError> {
            let text = json_string(unsafe { veil_macos_list_displays_json() })?;
            serde_json::from_str(&text).map_err(|err| MacError::Helper(err.to_string()))
        }

        pub fn list_windows(&self, icons: bool) -> Result<Vec<WindowInfo>, MacError> {
            let ptr = unsafe { veil_macos_list_windows_json(i32::from(icons)) };
            let text = json_string(ptr)?;
            let raw: Vec<RawWindow> = serde_json::from_str(&text)
                .map_err(|err| MacError::Helper(format!("bad inventory JSON: {err}")))?;
            Ok(raw
                .into_iter()
                .filter(|window| window.layer == 0 && window.width > 0 && window.height > 0)
                .map(|window| WindowInfo {
                    id: WindowId(window.id as u64),
                    app: if window.bundle.is_empty() {
                        AppKey::MacBundle(window.app.clone())
                    } else {
                        AppKey::MacBundle(window.bundle)
                    },
                    app_name: window.app,
                    title: window.title,
                    pid: window.pid.max(0) as u32,
                    bounds: Rect::new(window.x, window.y, window.width, window.height),
                    monitor: None,
                    visible: window.on_screen,
                    minimized: !window.on_screen,
                    top_level: true,
                    platform: Platform::MacOs,
                    icon: window.icon,
                })
                .collect())
        }
    }

    fn json_string(ptr: *mut c_char) -> Result<String, MacError> {
        if ptr.is_null() {
            return Err(MacError::Helper(last_error()));
        }
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { veil_macos_free_string(ptr) };
        Ok(text)
    }

    fn last_error() -> String {
        unsafe {
            let ptr = veil_macos_last_error();
            if ptr.is_null() {
                return "Swift helper returned NULL".to_string();
            }
            let text = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            veil_macos_free_string(ptr);
            text
        }
    }

    fn check(code: i32) -> Result<(), String> {
        if code == 0 {
            return Ok(());
        }
        Err(last_error())
    }
}
