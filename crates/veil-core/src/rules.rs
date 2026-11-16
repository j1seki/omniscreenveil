use serde::{Deserialize, Serialize};

use crate::capability::ProtectionMode;
use crate::ids::{AppKey, RuleId, WindowId};
use crate::style::FillStyle;
use crate::window::WindowInfo;
use crate::PROFILE_FORMAT_VERSION;

/// What a rule points at. Application rules follow new windows; window rules
/// are one-shot and disappear when the window does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TargetRef {
    App(AppKey),
    Window(WindowId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    pub target: TargetRef,
    pub mode: ProtectionMode,
    pub enabled: bool,
    #[serde(default)]
    pub note: String,
}

impl Rule {
    pub fn app(id: RuleId, app: AppKey, mode: ProtectionMode) -> Self {
        Self {
            id,
            target: TargetRef::App(app),
            mode,
            enabled: true,
            note: String::new(),
        }
    }

    pub fn window(id: RuleId, window: WindowId, mode: ProtectionMode) -> Self {
        Self {
            id,
            target: TargetRef::Window(window),
            mode,
            enabled: true,
            note: String::new(),
        }
    }

    pub fn matches(&self, window: &crate::window::WindowInfo) -> bool {
        match &self.target {
            TargetRef::App(app) => app.normalized() == window.app.normalized(),
            TargetRef::Window(id) => *id == window.id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSettings {
    pub fill: FillStyle,
    /// Never draw our own surface window inside the surface.
    pub exclude_self: bool,
}

impl Default for SurfaceSettings {
    fn default() -> Self {
        Self {
            fill: FillStyle::veiled(),
            exclude_self: true,
        }
    }
}

/// When protection should arm itself without the user pressing anything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoArm {
    #[default]
    Off,
    /// Arm when a known capture application (Discord, Zoom, OBS, Teams, ...) is
    /// running. Heuristic: running is not the same as sharing, so the UI must
    /// never claim it detected a live share.
    CaptureAppRunning,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub version: u32,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub surface: SurfaceSettings,
    #[serde(default)]
    pub auto_arm: AutoArm,
    #[serde(default)]
    next_rule_id: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: PROFILE_FORMAT_VERSION,
            rules: Vec::new(),
            surface: SurfaceSettings::default(),
            auto_arm: AutoArm::default(),
            next_rule_id: 1,
        }
    }
}

impl Profile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_rule(&mut self, target: TargetRef, mode: ProtectionMode) -> RuleId {
        let next_free = self
            .rules
            .iter()
            .map(|rule| rule.id.0.saturating_add(1))
            .max()
            .unwrap_or(1);
        let id = RuleId(self.next_rule_id.max(next_free).max(1));
        self.next_rule_id = id.0.saturating_add(1);
        self.rules.push(Rule {
            id,
            target,
            mode,
            enabled: true,
            note: String::new(),
        });
        id
    }

    /// Add a rule for an application, replacing any existing rule for the same
    /// application. Two modes for one app would otherwise compete forever and
    /// the older rule would silently win.
    pub fn add_app_rule(&mut self, app: AppKey, mode: ProtectionMode) -> RuleId {
        let key = app.normalized();
        self.rules.retain(|rule| {
            !matches!(&rule.target, TargetRef::App(existing) if existing.normalized() == key)
        });
        self.add_rule(TargetRef::App(app), mode)
    }

    /// Add a rule for one window, replacing any existing rule for it.
    pub fn add_window_rule(&mut self, window: WindowId, mode: ProtectionMode) -> RuleId {
        self.rules.retain(
            |rule| !matches!(&rule.target, TargetRef::Window(existing) if *existing == window),
        );
        self.add_rule(TargetRef::Window(window), mode)
    }

    /// Protect every app with a protectable window in the inventory.
    ///
    /// Existing app rules keep their configured mode but are re-enabled.
    /// Returns how many apps were newly covered or re-enabled.
    pub fn protect_all(&mut self, windows: &[WindowInfo], mode: ProtectionMode) -> usize {
        let mut apps: Vec<AppKey> = Vec::new();
        for window in windows.iter().filter(|window| window.is_protectable()) {
            let key = window.app.normalized();
            if !apps.iter().any(|app| app.normalized() == key) {
                apps.push(window.app.clone());
            }
        }

        let mut changed = 0;
        for app in apps {
            let key = app.normalized();
            let existing = self.rules.iter().position(|rule| {
                matches!(&rule.target, TargetRef::App(existing) if existing.normalized() == key)
            });
            match existing {
                Some(index) if !self.rules[index].enabled => {
                    self.rules[index].enabled = true;
                    changed += 1;
                }
                Some(_) => {}
                None => {
                    self.add_rule(TargetRef::App(app), mode);
                    changed += 1;
                }
            }
        }
        changed
    }

    pub fn remove_rule(&mut self, id: RuleId) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != id);
        before != self.rules.len()
    }

    pub fn set_enabled(&mut self, id: RuleId, enabled: bool) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn set_mode(&mut self, id: RuleId, mode: ProtectionMode) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) {
            rule.mode = mode;
            true
        } else {
            false
        }
    }

    /// Drop window rules whose window is gone, so the profile does not grow
    /// unbounded as windows are opened and closed.
    pub fn prune_dead_window_rules(&mut self, live: &[crate::window::WindowInfo]) -> usize {
        let before = self.rules.len();
        self.rules.retain(|rule| match &rule.target {
            TargetRef::Window(id) => live.iter().any(|w| w.id == *id),
            TargetRef::App(_) => true,
        });
        before - self.rules.len()
    }

    pub fn normalize(&mut self) {
        self.version = PROFILE_FORMAT_VERSION;
        if self.next_rule_id == 0 {
            self.next_rule_id = 1;
        }
        let max = self.rules.iter().map(|r| r.id.0).max().unwrap_or(0);
        self.next_rule_id = self.next_rule_id.max(max.saturating_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::ProtectionMode;
    use crate::ids::{AppKey, WindowId};

    #[test]
    fn adding_an_app_rule_replaces_the_previous_mode() {
        let mut profile = Profile::new();
        profile.add_app_rule(
            AppKey::WindowsExecutable("c:/apps/chat.exe".into()),
            ProtectionMode::StrictHide,
        );
        profile.add_app_rule(
            AppKey::WindowsExecutable("C:\\Apps\\Chat.exe".into()),
            ProtectionMode::Surface,
        );
        assert_eq!(profile.rules.len(), 1);
        assert_eq!(profile.rules[0].mode, ProtectionMode::Surface);
    }

    #[test]
    fn adding_a_window_rule_does_not_duplicate_it() {
        let mut profile = Profile::new();
        profile.add_window_rule(WindowId(7), ProtectionMode::StrictHide);
        profile.add_window_rule(WindowId(7), ProtectionMode::Surface);
        assert_eq!(profile.rules.len(), 1);
        assert_eq!(profile.rules[0].mode, ProtectionMode::Surface);
    }

    #[test]
    fn add_rule_skips_ids_already_in_use() {
        let mut profile = Profile::new();
        profile.rules.push(Rule {
            id: RuleId(9),
            target: TargetRef::Window(WindowId(1)),
            mode: ProtectionMode::Surface,
            enabled: true,
            note: String::new(),
        });
        let id = profile.add_window_rule(WindowId(2), ProtectionMode::Surface);
        assert_eq!(id, RuleId(10));
    }

    #[test]
    fn protect_all_covers_each_app_once_and_enables_existing_rules() {
        use crate::geometry::Rect;
        use crate::testutil::window;

        let mut profile = Profile::new();
        let slack = AppKey::LinuxAppId("slack".into());
        profile.add_app_rule(slack.clone(), ProtectionMode::Surface);
        let slack_rule = profile.rules[0].id;
        profile.set_enabled(slack_rule, false);

        let windows = vec![
            window(1, "chrome", "A", Rect::new(0, 0, 800, 600)),
            window(2, "chrome", "B", Rect::new(0, 0, 800, 600)),
            window(3, "slack", "General", Rect::new(0, 0, 400, 300)),
        ];
        assert_eq!(profile.protect_all(&windows, ProtectionMode::StrictHide), 2);
        assert_eq!(profile.rules.len(), 2);

        let chrome = profile
            .rules
            .iter()
            .find(
                |rule| matches!(&rule.target, TargetRef::App(app) if app.normalized() == "chrome"),
            )
            .expect("chrome rule");
        assert_eq!(chrome.mode, ProtectionMode::StrictHide);
        assert!(chrome.enabled);

        let slack = profile
            .rules
            .iter()
            .find(|rule| rule.id == slack_rule)
            .expect("slack rule");
        assert_eq!(slack.mode, ProtectionMode::Surface, "existing mode is kept");
        assert!(slack.enabled, "disabled rule is re-enabled");
    }

    #[test]
    fn protect_all_skips_empty_and_non_top_level_windows() {
        use crate::geometry::Rect;
        use crate::testutil::window;

        let mut profile = Profile::new();
        let empty = window(1, "chrome", "A", Rect::new(0, 0, 0, 0));
        let mut popup = window(2, "discord", "Call", Rect::new(0, 0, 200, 200));
        popup.top_level = false;

        assert_eq!(
            profile.protect_all(&[empty, popup], ProtectionMode::Surface),
            0
        );
        assert!(profile.rules.is_empty());
    }

    #[test]
    fn protect_all_is_idempotent() {
        use crate::geometry::Rect;
        use crate::testutil::window;

        let mut profile = Profile::new();
        let windows = vec![window(1, "chrome", "A", Rect::new(0, 0, 800, 600))];
        assert_eq!(profile.protect_all(&windows, ProtectionMode::Surface), 1);
        assert_eq!(profile.protect_all(&windows, ProtectionMode::Surface), 0);
        assert_eq!(profile.rules.len(), 1);
    }
}
