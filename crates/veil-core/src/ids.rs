use serde::{Deserialize, Serialize};

/// Stable identifier for a top-level window.
///
/// Window identifiers are reused by every desktop platform we target, so they
/// are only meaningful together with the owning pid (see
/// [`crate::window::WindowInfo`]). The reconcile loop never remembers an id
/// across ticks; it re-derives intent from the live window list every time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuleId(pub u64);

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rule-{}", self.0)
    }
}

/// Stable, cross-session identity of an application.
///
/// User rules must survive restarts, so they are anchored to the application
/// identity rather than a pid or a window handle:
///
/// * Windows: normalized path of the executable.
/// * macOS: bundle identifier, falling back to the bundle path.
/// * Linux: desktop app id (`WM_CLASS` / `app_id`) or the executable path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AppKey {
    WindowsExecutable(String),
    MacBundle(String),
    LinuxAppId(String),
}

impl AppKey {
    pub fn as_str(&self) -> &str {
        match self {
            AppKey::WindowsExecutable(s) | AppKey::MacBundle(s) | AppKey::LinuxAppId(s) => s,
        }
    }

    /// Normalize case and separators so rules match regardless of how the
    /// platform formatted the identity.
    pub fn normalized(&self) -> String {
        match self {
            AppKey::WindowsExecutable(s) => s.replace('\\', "/").to_ascii_lowercase(),
            AppKey::MacBundle(s) => s.to_ascii_lowercase(),
            AppKey::LinuxAppId(s) => s.to_ascii_lowercase(),
        }
    }
}

impl std::fmt::Display for AppKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self {
            AppKey::WindowsExecutable(_) => "exe",
            AppKey::MacBundle(_) => "bundle",
            AppKey::LinuxAppId(_) => "app",
        };
        write!(f, "{prefix}:{}", self.as_str())
    }
}
