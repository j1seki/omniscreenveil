use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::window::Platform;

/// The three ways OmniScreenVeil can protect content.
///
/// They are not interchangeable and they are not equally available on every
/// platform, which is the central honesty constraint of the product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionMode {
    /// The window stays visible and interactive for its owner but is excluded
    /// from capture by the OS compositor. Highest fidelity, Windows-only in
    /// practice.
    Exclusion,
    /// The user shares a window we render: a live copy of the desktop with
    /// protected windows replaced by a placeholder. Works everywhere.
    Surface,
    /// The window is hidden locally as well for the duration of the session
    /// and restored afterwards. Last resort.
    StrictHide,
}

impl ProtectionMode {
    /// Preference order when several rules target the same window. Higher wins.
    pub fn precedence(self) -> u8 {
        match self {
            ProtectionMode::Exclusion => 3,
            ProtectionMode::Surface => 2,
            ProtectionMode::StrictHide => 1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ProtectionMode::Exclusion => "Exclusion",
            ProtectionMode::Surface => "Surface",
            ProtectionMode::StrictHide => "Strict hide",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ProtectionMode::Exclusion => {
                "Window remains usable locally and is removed from capture by the OS."
            }
            ProtectionMode::Surface => {
                "Share the Veil Surface window; protected windows are filtered or masked in it."
            }
            ProtectionMode::StrictHide => {
                "Window is hidden locally while protection is armed and restored afterwards."
            }
        }
    }

    pub const ALL: [ProtectionMode; 3] = [
        ProtectionMode::Exclusion,
        ProtectionMode::Surface,
        ProtectionMode::StrictHide,
    ];
}

/// How well a mode is supported on a platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum CapabilityLevel {
    /// Works for arbitrary third-party windows out of the box.
    Native,
    /// Works with a documented constraint (compositor helper, permission,
    /// geometry provider, ...). The string is user-visible.
    Partial { detail: String },
    /// Cannot be provided on this platform with clean, shippable APIs.
    Unsupported { reason: String },
}

impl CapabilityLevel {
    pub fn is_supported(&self) -> bool {
        !matches!(self, CapabilityLevel::Unsupported { .. })
    }

    pub fn is_native(&self) -> bool {
        matches!(self, CapabilityLevel::Native)
    }

    pub fn detail(&self) -> Option<&str> {
        match self {
            CapabilityLevel::Native => None,
            CapabilityLevel::Partial { detail } => Some(detail),
            CapabilityLevel::Unsupported { reason } => Some(reason),
        }
    }

    /// Short badge for the UI: `native`, `partial` or `unsupported`.
    pub fn badge(&self) -> &'static str {
        match self {
            CapabilityLevel::Native => "native",
            CapabilityLevel::Partial { .. } => "partial",
            CapabilityLevel::Unsupported { .. } => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeCapability {
    pub mode: ProtectionMode,
    pub level: CapabilityLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    pub platform: Platform,
    pub modes: BTreeMap<ProtectionMode, CapabilityLevel>,
}

impl PlatformCapabilities {
    /// The verified support matrix. Keep this in sync with
    /// `docs/support-matrix.md`; every `Native` row must have a capture test
    /// that proves it.
    pub fn for_platform(platform: Platform) -> Self {
        let mut modes = BTreeMap::new();
        match platform {
            Platform::Windows => {
                for mode in ProtectionMode::ALL {
                    modes.insert(mode, CapabilityLevel::Native);
                }
            }
            Platform::MacOs => {
                modes.insert(
                    ProtectionMode::Exclusion,
                    CapabilityLevel::Unsupported {
                        reason: "No public API lets a third-party process exclude another \
                                 application's window from ScreenCaptureKit."
                            .into(),
                    },
                );
                modes.insert(ProtectionMode::Surface, CapabilityLevel::Partial { detail: "Requires macOS 14 and Screen Recording permission. ScreenCaptureKit filters selected windows from this app’s main-display capture only; share the Veil Surface, not your desktop.".into() });
                modes.insert(ProtectionMode::StrictHide, CapabilityLevel::Partial { detail: "Requires Accessibility permission; restoration after a crash is not guaranteed.".into() });
            }
            Platform::LinuxX11 => {
                modes.insert(
                    ProtectionMode::Exclusion,
                    CapabilityLevel::Unsupported {
                        reason: "Requires a privileged Xorg XACE module; not shippable.".into(),
                    },
                );
                modes.insert(ProtectionMode::Surface, CapabilityLevel::Native);
                modes.insert(ProtectionMode::StrictHide, CapabilityLevel::Native);
            }
            Platform::LinuxWayland => {
                modes.insert(
                    ProtectionMode::Exclusion,
                    CapabilityLevel::Unsupported {
                        reason: "Compositors do not expose per-window capture exclusion.".into(),
                    },
                );
                modes.insert(
                    ProtectionMode::Surface,
                    CapabilityLevel::Partial {
                        detail: "Requires grim and sway or Hyprland compositor IPC. Other \
                                 Wayland compositors show a labelled simulation."
                            .into(),
                    },
                );
                modes.insert(
                    ProtectionMode::StrictHide,
                    CapabilityLevel::Partial {
                        detail: "Works on sway and Hyprland through compositor IPC; GNOME Shell \
                                 and KDE Plasma need a desktop-specific extension."
                            .into(),
                    },
                );
            }
        }
        Self { platform, modes }
    }

    pub fn level(&self, mode: ProtectionMode) -> &CapabilityLevel {
        self.modes
            .get(&mode)
            .expect("every mode has a capability entry")
    }

    pub fn supports(&self, mode: ProtectionMode) -> bool {
        self.level(mode).is_supported()
    }

    /// Highest-fidelity mode available on this platform.
    pub fn best_mode(&self) -> Option<ProtectionMode> {
        ProtectionMode::ALL.into_iter().find(|m| self.supports(*m))
    }

    /// Whether a requested mode has to be downgraded before it can be applied.
    pub fn effective_mode(&self, requested: ProtectionMode) -> ProtectionMode {
        if self.supports(requested) {
            return requested;
        }
        // Fall back to the closest supported mode with lower precedence.
        ProtectionMode::ALL
            .into_iter()
            .filter(|m| m.precedence() < requested.precedence())
            .find(|m| self.supports(*m))
            .unwrap_or(ProtectionMode::StrictHide)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_has_every_mode() {
        let caps = PlatformCapabilities::for_platform(Platform::Windows);
        assert_eq!(caps.best_mode(), Some(ProtectionMode::Exclusion));
        for mode in ProtectionMode::ALL {
            assert_eq!(caps.level(mode), &CapabilityLevel::Native);
        }
    }

    #[test]
    fn macos_never_claims_exclusion() {
        let caps = PlatformCapabilities::for_platform(Platform::MacOs);
        assert!(!caps.supports(ProtectionMode::Exclusion));
        assert_eq!(
            caps.effective_mode(ProtectionMode::Exclusion),
            ProtectionMode::Surface
        );
        assert_eq!(caps.best_mode(), Some(ProtectionMode::Surface));
    }

    #[test]
    fn wayland_surface_is_partial() {
        let caps = PlatformCapabilities::for_platform(Platform::LinuxWayland);
        assert_eq!(caps.level(ProtectionMode::Surface).badge(), "partial");
        assert_eq!(caps.level(ProtectionMode::StrictHide).badge(), "partial");
        assert_eq!(caps.best_mode(), Some(ProtectionMode::Surface));
    }
}
