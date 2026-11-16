//! Platform backend selection for the desktop app.
//!
//! One enum hides the per-OS differences from the Tauri commands. When no real
//! backend is available (headless CI, a Wayland compositor without the IPC this
//! build speaks, missing permissions) we fall back to the in-memory backend and
//! say so in the UI instead of pretending protection is live.

use veil_core::window::Platform;
use veil_core::{WindowInfo, WindowId};
use veil_protect::{BackendError, MemoryBackend, ProtectionBackend};
use veil_surface::Frame;

#[cfg(target_os = "linux")]
use veil_protect_linux::X11Backend;
#[cfg(target_os = "linux")]
use veil_protect_wayland::WaylandBackend;

#[cfg(windows)]
use veil_protect_win::WinBackend;

#[cfg(target_os = "macos")]
use veil_protect_macos::MacBackend;

/// What the UI should say about the backend it got.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackendInfo {
    pub platform: Platform,
    pub name: String,
    pub live: bool,
    pub note: String,
}

// One backend instance is created at startup; keeping the variants inline is
// clearer than boxing them just to satisfy a size lint.
#[allow(clippy::large_enum_variant)]
pub enum DesktopBackend {
    #[cfg(target_os = "linux")]
    X11(X11Backend),
    #[cfg(target_os = "linux")]
    Wayland(WaylandBackend),
    #[cfg(windows)]
    Windows(WinBackend),
    #[cfg(target_os = "macos")]
    Mac(MacBackend),
    /// Honest simulation: the engine runs, the UI is real, no pixels change.
    Simulated(MemoryBackend),
}

impl DesktopBackend {
    /// Detect the best backend for this machine, never failing.
    pub fn detect() -> (Self, BackendInfo) {
        let detected = Platform::detect();
        #[cfg(target_os = "linux")]
        {
            if detected == Platform::LinuxX11 {
                match X11Backend::connect() {
                    Ok(backend) => {
                        return (
                            Self::X11(backend),
                            BackendInfo {
                                platform: Platform::LinuxX11,
                                name: "X11".into(),
                                live: true,
                                note: "Live X11 session. Surface masks Windows; strict hide unmaps them.".into(),
                            },
                        );
                    }
                    Err(err) => {
                        return (
                            Self::Simulated(MemoryBackend::new(Platform::LinuxX11)),
                            BackendInfo {
                                platform: Platform::LinuxX11,
                                name: "Simulated".into(),
                                live: false,
                                note: format!(
                                    "Could not connect to the X server ({err}); showing demo windows."
                                ),
                            },
                        );
                    }
                }
            }
            match WaylandBackend::new() {
                Ok(backend) => {
                    let compositor = backend.compositor().label();
                    (
                        Self::Wayland(backend),
                        BackendInfo {
                            platform: Platform::LinuxWayland,
                            name: format!("Wayland ({compositor})"),
                            live: true,
                            note: format!(
                                "Live {compositor} session. The Surface captures through grim/wlr-screencopy and masks protected windows; strict hide moves them to a private workspace via compositor IPC."
                            ),
                        },
                    )
                }
                Err(err) => (
                    Self::Simulated(MemoryBackend::new(Platform::LinuxWayland)),
                    BackendInfo {
                        platform: Platform::LinuxWayland,
                        name: "Simulated (Wayland)".into(),
                        live: false,
                        note: format!(
                            "Wayland backend unavailable ({err}). Only sway and Hyprland expose the public IPC this build uses; GNOME/KDE need a compositor extension. Protected windows are simulated here."
                        ),
                    },
                ),
            }
        }

        #[cfg(windows)]
        {
            match WinBackend::new() {
                Ok(backend) => {
                    return (
                        Self::Windows(backend),
                        BackendInfo {
                            platform: Platform::Windows,
                            name: "Windows (WDA + GDI)".into(),
                            live: true,
                            note: "Live Windows session. Exclusion uses a helper DLL; Surface uses GDI capture. Test your actual capture application before sharing."
                                .into(),
                        },
                    );
                }
                Err(err) => {
                    return (
                        Self::Simulated(MemoryBackend::new(Platform::Windows)),
                        BackendInfo {
                            platform: Platform::Windows,
                            name: "Simulated".into(),
                            live: false,
                            note: format!("Windows backend unavailable ({err})."),
                        },
                    );
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            match MacBackend::new() {
                Ok(backend) => {
                    let note = if backend.accessibility_trusted() {
                        "Live macOS session. Surface captures via ScreenCaptureKit; strict hide uses Accessibility."
                            .to_string()
                    } else {
                        "Live macOS session, but Accessibility permission is not granted. Strict hide will fail until you allow it in System Settings → Privacy & Security → Accessibility."
                            .to_string()
                    };
                    return (
                        Self::Mac(backend),
                        BackendInfo {
                            platform: Platform::MacOs,
                            name: "macOS".into(),
                            live: true,
                            note,
                        },
                    );
                }
                Err(err) => {
                    return (
                        Self::Simulated(MemoryBackend::new(Platform::MacOs)),
                        BackendInfo {
                            platform: Platform::MacOs,
                            name: "Simulated".into(),
                            live: false,
                            note: format!("macOS helper unavailable ({err})."),
                        },
                    );
                }
            }
        }

        #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
        {
            (
                Self::Simulated(MemoryBackend::new(detected)),
                BackendInfo {
                    platform: detected,
                    name: "Simulated".into(),
                    live: false,
                    note: "Unsupported host; simulation only.".into(),
                },
            )
        }
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, String> {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => backend
                .list_windows()
                .map_err(|err| format!("X11 inventory failed: {err}")),
            #[cfg(target_os = "linux")]
            Self::Wayland(backend) => backend
                .list_windows()
                .map_err(|err| format!("Wayland inventory failed: {err}")),
            #[cfg(windows)]
            Self::Windows(backend) => backend
                .list_windows()
                .map_err(|err| format!("Windows inventory failed: {err}")),
            #[cfg(target_os = "macos")]
            Self::Mac(backend) => backend
                .list_windows()
                .map_err(|err| format!("macOS inventory failed: {err}")),
            Self::Simulated(_) => Ok(demo_windows(Platform::detect())),
        }
    }

    /// Capture the desktop for the Veil Surface preview, when the backend can.
    pub fn capture_frame(&self) -> Option<Frame> {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => backend.platform().capture_surface().ok(),
            #[cfg(target_os = "linux")]
            Self::Wayland(backend) => backend.capture_root().ok(),
            #[cfg(windows)]
            Self::Windows(backend) => backend.capture_root().ok(),
            #[cfg(target_os = "macos")]
            Self::Mac(backend) => backend.capture_root().ok(),
            _ => None,
        }
    }

    /// Whether the backend's capture includes its own windows. X11 composites
    /// foreign windows only, and macOS excludes them in ScreenCaptureKit, so
    /// the Windows GDI and Wayland screencopy paths need the extra mask.
    pub fn capture_includes_own_windows(&self) -> bool {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => !backend.platform().has_compositor(),
            #[cfg(target_os = "linux")]
            Self::Wayland(_) => true,
            #[cfg(windows)]
            Self::Windows(_) => true,
            _ => false,
        }
    }

    /// Bounds of every app-owned window, so the surface never composites itself.
    pub fn own_windows(&self) -> Vec<veil_core::Rect> {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => backend.own_windows(),
            #[cfg(target_os = "linux")]
            Self::Wayland(backend) => backend.own_windows(),
            #[cfg(windows)]
            Self::Windows(backend) => backend.own_windows(),
            _ => Vec::new(),
        }
    }
}

impl ProtectionBackend for DesktopBackend {
    fn platform(&self) -> Platform {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => ProtectionBackend::platform(backend),
            #[cfg(target_os = "linux")]
            Self::Wayland(backend) => ProtectionBackend::platform(backend),
            #[cfg(windows)]
            Self::Windows(backend) => backend.platform(),
            #[cfg(target_os = "macos")]
            Self::Mac(backend) => backend.platform(),
            Self::Simulated(backend) => backend.platform(),
        }
    }

    fn apply(&mut self, intent: &veil_core::state::ProtectionIntent) -> Result<(), BackendError> {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => backend.apply(intent),
            #[cfg(target_os = "linux")]
            Self::Wayland(backend) => backend.apply(intent),
            #[cfg(windows)]
            Self::Windows(backend) => backend.apply(intent),
            #[cfg(target_os = "macos")]
            Self::Mac(backend) => backend.apply(intent),
            Self::Simulated(backend) => backend.apply(intent),
        }
    }

    fn release(&mut self, window: WindowId) -> Result<(), BackendError> {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => backend.release(window),
            #[cfg(target_os = "linux")]
            Self::Wayland(backend) => backend.release(window),
            #[cfg(windows)]
            Self::Windows(backend) => backend.release(window),
            #[cfg(target_os = "macos")]
            Self::Mac(backend) => backend.release(window),
            Self::Simulated(backend) => backend.release(window),
        }
    }

    fn describe(&self) -> String {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(_) => "X11 backend".into(),
            #[cfg(target_os = "linux")]
            Self::Wayland(backend) => backend.describe(),
            #[cfg(windows)]
            Self::Windows(_) => "Windows backend".into(),
            #[cfg(target_os = "macos")]
            Self::Mac(_) => "macOS backend".into(),
            Self::Simulated(_) => "Simulated backend (no pixels change)".into(),
        }
    }
}

/// Demo desktop used when no live backend exists. Same fixture the CLI and the
/// UI preview use, retagged for the host platform so the rule matcher behaves
/// consistently.
fn demo_windows(platform: Platform) -> Vec<WindowInfo> {
    let mut windows = veil_core::testutil::demo_windows();
    for window in &mut windows {
        window.platform = platform;
        window.app = match platform {
            Platform::Windows => {
                veil_core::AppKey::WindowsExecutable(format!("C:/Apps/{}.exe", window.app_name))
            }
            Platform::MacOs => veil_core::AppKey::MacBundle(format!("app.{}", window.app_name)),
            _ => veil_core::AppKey::LinuxAppId(window.app_name.clone()),
        };
    }
    windows
}
