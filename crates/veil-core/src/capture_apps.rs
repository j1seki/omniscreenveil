//! Heuristic detection of screen-capture applications.
//!
//! Several reference tools (Evanesco's autohider, Mica's trigger apps) arm
//! protection when a known capture app is running. Running is not the same as
//! sharing, so callers must present this as a heuristic, never as proof that a
//! capture is live.

use crate::ids::AppKey;
use crate::window::WindowInfo;

/// Bundle ids, Linux app ids and other exact identities.
const CAPTURE_IDS: &[&str] = &[
    "com.apple.quicktimeplayerx",
    "com.apple.screencaptureui",
    "com.apple.screensharing",
    "com.hnc.discord",
    "com.loom.desktop",
    "com.microsoft.teams",
    "com.microsoft.teams2",
    "com.obsproject.obs-studio",
    "com.skype.skype",
    "com.techsmith.camtasia",
    "com.tinyspeck.slackmacgap",
    "com.webex.meeting",
    "com.cisco.webexmeetingsapp",
    "discord",
    "flameshot",
    "gnome-screenshot",
    "kazam",
    "kooha",
    "obs",
    "obs-studio",
    "peek",
    "recordmydesktop",
    "simplescreenrecorder",
    "skype",
    "slack",
    "spectacle",
    "teams",
    "us.zoom.xos",
    "webex",
    "wf-recorder",
    "zoom",
];

/// Windows executable stems and Linux ids without a path.
const CAPTURE_STEMS: &[&str] = &[
    "bandicam",
    "camtasia",
    "camtasiastudio",
    "discord",
    "flameshot",
    "fraps",
    "gnome-screenshot",
    "gotomeeting",
    "greenshot",
    "kazam",
    "kooha",
    "lightshot",
    "loom",
    "ms-teams",
    "obs",
    "obs32",
    "obs64",
    "peek",
    "recordmydesktop",
    "screenclippinghost",
    "sharex",
    "simplescreenrecorder",
    "skype",
    "slack",
    "snippingtool",
    "spectacle",
    "streamlabs",
    "teams",
    "webex",
    "webexmta",
    "wf-recorder",
    "xsplit",
    "zoom",
];

/// Human-readable names some platforms report instead of an identity.
const CAPTURE_NAMES: &[&str] = &[
    "camtasia",
    "discord",
    "flameshot",
    "kooha",
    "loom",
    "microsoft teams",
    "obs",
    "obs studio",
    "peek",
    "quicktime player",
    "recordmydesktop",
    "screen sharing",
    "screenshot",
    "simplescreenrecorder",
    "skype",
    "slack",
    "snipping tool",
    "spectacle",
    "teams",
    "webex",
    "webex meetings",
    "zoom",
    "zoom workplace",
];

fn basename(value: &str) -> &str {
    value.rsplit('/').next().unwrap_or(value)
}

fn without_extension(value: &str) -> &str {
    value
        .strip_suffix(".exe")
        .or_else(|| value.strip_suffix(".app"))
        .unwrap_or(value)
}

/// Whether a window belongs to a known screen-capture application.
pub fn is_capture_app(app: &AppKey, app_name: &str) -> bool {
    let id = app.normalized();
    let base = basename(&id);
    let stem = without_extension(base);
    let name = app_name.trim().to_ascii_lowercase();

    CAPTURE_IDS.contains(&id.as_str())
        || CAPTURE_IDS.contains(&stem)
        || CAPTURE_STEMS.contains(&stem)
        || CAPTURE_NAMES.contains(&name.as_str())
}

/// Whether any window in the inventory belongs to a known capture app.
pub fn capture_app_running(windows: &[WindowInfo]) -> bool {
    windows
        .iter()
        .any(|window| is_capture_app(&window.app, &window.app_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::ids::WindowId;
    use crate::testutil::{app, window};
    use crate::window::Platform;

    #[test]
    fn recognizes_capture_apps_across_platforms() {
        assert!(is_capture_app(
            &AppKey::WindowsExecutable(r"C:\Program Files\obs-studio\bin\64bit\obs64.exe".into()),
            "obs64"
        ));
        assert!(is_capture_app(
            &AppKey::MacBundle("us.zoom.xos".into()),
            "zoom.us"
        ));
        assert!(is_capture_app(
            &AppKey::LinuxAppId("com.obsproject.obs-studio".into()),
            "OBS Studio"
        ));
        assert!(is_capture_app(&app("discord"), "Discord"));
        assert!(is_capture_app(
            &AppKey::WindowsExecutable("C:/Tools/SnippingTool.exe".into()),
            "SnippingTool"
        ));
    }

    #[test]
    fn ignores_lookalikes_and_unrelated_apps() {
        assert!(!is_capture_app(&app("obsidian"), "Obsidian"));
        assert!(!is_capture_app(&app("chrome"), "Google Chrome"));
        assert!(!is_capture_app(&app("1password"), "1Password"));
        assert!(!is_capture_app(&app("firefox"), "Firefox"));
        assert!(!is_capture_app(
            &AppKey::WindowsExecutable("C:/Apps/Observer.exe".into()),
            "Observer"
        ));
    }

    #[test]
    fn detects_a_running_capture_app_in_the_inventory() {
        let idle = vec![
            window(1, "chrome", "Inbox", Rect::new(0, 0, 800, 600)),
            window(2, "1password", "Vault", Rect::new(0, 0, 400, 300)),
        ];
        assert!(!capture_app_running(&idle));

        let sharing = vec![
            window(1, "chrome", "Inbox", Rect::new(0, 0, 800, 600)),
            window(3, "obs", "OBS 31.0", Rect::new(0, 0, 400, 300)),
        ];
        assert!(capture_app_running(&sharing));
    }

    #[test]
    fn detects_a_minimized_capture_app() {
        let mut obs = window(9, "zoom", "Zoom", Rect::new(0, 0, 800, 600));
        obs.minimized = true;
        obs.visible = false;
        obs.platform = Platform::LinuxWayland;
        obs.id = WindowId(9);
        assert!(capture_app_running(&[obs]));
    }
}
