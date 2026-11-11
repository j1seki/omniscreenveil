//! Platform backend selection for the desktop app.
//!
//! One enum hides the per-OS differences from the Tauri commands. When no real
//! backend is available (headless CI, Wayland with nothing implemented yet,
//! missing permissions) we fall back to the in-memory backend and say so in the
//! UI instead of pretending protection is live.

use veil_core::window::Platform;
use veil_core::{WindowInfo, WindowId};
use veil_protect::{BackendError, MemoryBackend, ProtectionBackend};
use veil_surface::Frame;

#[cfg(target_os = "linux")]
use veil_protect_linux::X11Backend;

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

pub enum DesktopBackend {
    #[cfg(target_os = "linux")]
    X11(X11Backend),
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
            return (
                Self::Simulated(MemoryBackend::new(Platform::LinuxWayland)),
                BackendInfo {
                    platform: Platform::LinuxWayland,
                    name: "Simulated (Wayland)".into(),
                    live: false,
                    note: "Wayland session: the portal/PipeWire Veil Surface pipeline is not wired into this build. Use strict hide or the CLI; protected windows are simulated here."
                        .into(),
                },
            );
        }

        #[cfg(windows)]
        {
            match WinBackend::new() {
                Ok(backend) => {
                    return (
                        Self::Windows(backend),
                        BackendInfo {
                            platform: Platform::Windows,
                            name: "Windows".into(),
                            live: true,
                            note: "Live Windows session. Exclusion injects veil_hook_win.dll; strict hide uses ShowWindow."
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
                    return (
                        Self::Mac(backend),
                        BackendInfo {
                            platform: Platform::MacOs,
                            name: "macOS".into(),
                            live: true,
                            note: "Live macOS session. Surface captures via ScreenCaptureKit; strict hide uses Accessibility."
                                .into(),
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
            Self::X11(backend) => backend.platform().capture_root().ok(),
            _ => None,
        }
    }

    /// Bounds of the app's own window, so the surface never composites itself.
    pub fn own_window(&self) -> Option<veil_core::Rect> {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => backend.own_window_bounds(),
            _ => None,
        }
    }
}

impl ProtectionBackend for DesktopBackend {
    fn platform(&self) -> Platform {
        match self {
            #[cfg(target_os = "linux")]
            Self::X11(backend) => ProtectionBackend::platform(backend),
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
