use serde::{Deserialize, Serialize};

use crate::capability::ProtectionMode;
use crate::ids::{AppKey, RuleId, WindowId};
use crate::style::FillStyle;
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
        let id = RuleId(self.next_rule_id);
        self.next_rule_id += 1;
        self.rules.push(Rule {
            id,
            target,
            mode,
            enabled: true,
            note: String::new(),
        });
        id
    }

    pub fn add_app_rule(&mut self, app: AppKey, mode: ProtectionMode) -> RuleId {
        self.add_rule(TargetRef::App(app), mode)
    }

    pub fn add_window_rule(&mut self, window: WindowId, mode: ProtectionMode) -> RuleId {
        self.add_rule(TargetRef::Window(window), mode)
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
        self.next_rule_id = self.next_rule_id.max(max + 1);
    }
}
