//! Test and demo fixtures shared by unit tests, the CLI self-test and the UI
//! preview. Kept public so integration tests and the desktop backend can build
//! realistic window lists without a live desktop.

use crate::geometry::Rect;
use crate::ids::{AppKey, WindowId};
use crate::window::{Platform, WindowInfo};

pub fn app(key: &str) -> AppKey {
    AppKey::LinuxAppId(key.to_string())
}

pub fn windows(list: Vec<WindowInfo>) -> Vec<WindowInfo> {
    list
}

pub fn window(id: u64, app_key: &str, title: &str, bounds: Rect) -> WindowInfo {
    WindowInfo {
        id: WindowId(id),
        app: AppKey::LinuxAppId(app_key.to_string()),
        app_name: app_key.to_string(),
        title: title.to_string(),
        pid: 1000 + id as u32,
        bounds,
        monitor: Some(0),
        visible: true,
        minimized: false,
        top_level: true,
        platform: Platform::LinuxX11,
        icon: String::new(),
    }
}

/// A believable desktop used by the CLI demo and the UI preview.
pub fn demo_windows() -> Vec<WindowInfo> {
    vec![
        window(0x1a, "chrome", "Inbox — work", Rect::new(40, 40, 820, 520)),
        window(0x2b, "1password", "Vault", Rect::new(420, 240, 560, 420)),
        window(0x3c, "slack", "#general", Rect::new(880, 60, 360, 300)),
        window(0x4d, "obs", "OBS 31.0", Rect::new(880, 400, 360, 260)),
    ]
}
