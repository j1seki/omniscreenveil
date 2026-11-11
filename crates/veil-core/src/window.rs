use serde::{Deserialize, Serialize};

use crate::geometry::Rect;
use crate::ids::{AppKey, WindowId};

/// Which host the agent is running on.
///
/// X11 and Wayland are separate platforms because their capabilities differ
/// fundamentally; "Linux" alone is not an actionable answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Windows,
    MacOs,
    LinuxX11,
    LinuxWayland,
}

impl Platform {
    pub fn is_linux(self) -> bool {
        matches!(self, Platform::LinuxX11 | Platform::LinuxWayland)
    }

    pub fn label(self) -> &'static str {
        match self {
            Platform::Windows => "Windows",
            Platform::MacOs => "macOS",
            Platform::LinuxX11 => "Linux (X11)",
            Platform::LinuxWayland => "Linux (Wayland)",
        }
    }

    /// Best-effort detection for the current process.
    pub fn detect() -> Platform {
        if cfg!(target_os = "windows") {
            return Platform::Windows;
        }
        if cfg!(target_os = "macos") {
            return Platform::MacOs;
        }
        match std::env::var("XDG_SESSION_TYPE").as_deref() {
            Ok("wayland") => Platform::LinuxWayland,
            Ok("x11") => Platform::LinuxX11,
            _ => {
                if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                    Platform::LinuxWayland
                } else {
                    Platform::LinuxX11
                }
            }
        }
    }
}

/// A live top-level window as reported by a platform backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: WindowId,
    pub app: AppKey,
    pub app_name: String,
    pub title: String,
    pub pid: u32,
    /// Logical coordinates in the virtual desktop (may be negative on
    /// multi-monitor layouts).
    pub bounds: Rect,
    pub monitor: Option<u32>,
    pub visible: bool,
    pub minimized: bool,
    pub top_level: bool,
    pub platform: Platform,
}

impl WindowInfo {
    /// Windows we are willing to act on: real, top-level surfaces.
    ///
    /// Visibility is deliberately not required: strict-hide unmaps the window,
    /// and we must keep treating it as protected so the engine does not
    /// immediately release (and restore) it on the next tick.
    pub fn is_protectable(&self) -> bool {
        self.top_level && !self.bounds.is_empty()
    }

    pub fn display_name(&self) -> String {
        if self.title.trim().is_empty() {
            self.app_name.clone()
        } else {
            format!("{} — {}", self.app_name, self.title)
        }
    }
}
