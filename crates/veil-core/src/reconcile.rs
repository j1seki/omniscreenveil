use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::capability::{PlatformCapabilities, ProtectionMode};
use crate::ids::WindowId;
use crate::rules::Profile;
use crate::state::{AppliedProtection, ApplyStatus, ProtectionIntent};
use crate::window::WindowInfo;

/// Two rules wanted different modes for the same window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub window: WindowId,
    pub requested: Vec<ProtectionMode>,
    pub chosen: ProtectionMode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcilePlan {
    pub apply: Vec<ProtectionIntent>,
    pub release: Vec<AppliedProtection>,
    pub unchanged: Vec<AppliedProtection>,
    pub conflicts: Vec<Conflict>,
}

impl ReconcilePlan {
    pub fn is_empty(&self) -> bool {
        self.apply.is_empty() && self.release.is_empty()
    }

    /// Replace unsupported modes with the best supported fallback.
    ///
    /// This is where the honest capability matrix meets user intent: a rule
    /// asking for `Exclusion` on macOS becomes `Surface` instead of silently
    /// doing nothing.
    pub fn downgrade(&mut self, caps: &PlatformCapabilities) {
        for intent in &mut self.apply {
            intent.mode = caps.effective_mode(intent.mode);
        }
        for applied in &mut self.release {
            // release is mode-agnostic, nothing to downgrade
            let _ = applied;
        }
    }
}

/// Resolve every enabled rule against the live window list.
///
/// Returns one intent per window, using the highest-precedence mode requested
/// by any matching rule, plus a record for every window with competing modes.
pub fn intents(
    profile: &Profile,
    windows: &[WindowInfo],
) -> (Vec<ProtectionIntent>, Vec<Conflict>) {
    let mut by_window: BTreeMap<WindowId, Vec<&crate::rules::Rule>> = BTreeMap::new();
    for window in windows.iter().filter(|w| w.is_protectable()) {
        for rule in profile
            .rules
            .iter()
            .filter(|r| r.enabled && r.matches(window))
        {
            by_window.entry(window.id).or_default().push(rule);
        }
    }

    let mut out = Vec::with_capacity(by_window.len());
    let mut conflicts = Vec::new();
    for (window_id, rules) in by_window {
        let Some(window) = windows.iter().find(|w| w.id == window_id) else {
            continue;
        };
        let chosen_rule = rules
            .iter()
            .max_by_key(|r| (r.mode.precedence(), std::cmp::Reverse(r.id.0)))
            .expect("group is non-empty");
        let mut requested: Vec<ProtectionMode> = rules.iter().map(|r| r.mode).collect();
        requested.sort_by_key(|m| std::cmp::Reverse(m.precedence()));
        requested.dedup();
        if requested.len() > 1 {
            conflicts.push(Conflict {
                window: window_id,
                requested,
                chosen: chosen_rule.mode,
            });
        }
        out.push(ProtectionIntent {
            window: window_id,
            app: window.app.clone(),
            app_name: window.app_name.clone(),
            pid: window.pid,
            bounds: window.bounds,
            mode: chosen_rule.mode,
            rule: chosen_rule.id,
        });
    }
    (out, conflicts)
}

/// Diff desired state against what the backend has already applied.
pub fn plan(
    profile: &Profile,
    windows: &[WindowInfo],
    applied: &[AppliedProtection],
) -> ReconcilePlan {
    let (mut desired, conflicts) = intents(profile, windows);
    desired.sort_by_key(|i| i.window);

    let applied_map: BTreeMap<WindowId, &AppliedProtection> =
        applied.iter().map(|a| (a.window, a)).collect();

    let mut out = ReconcilePlan {
        conflicts,
        ..ReconcilePlan::default()
    };

    for intent in desired {
        match applied_map.get(&intent.window) {
            None => out.apply.push(intent),
            Some(current) => {
                let needs_apply = current.mode != intent.mode
                    || matches!(current.status, ApplyStatus::Failed { .. });
                if needs_apply {
                    out.apply.push(intent);
                } else {
                    out.unchanged.push((*current).clone());
                }
            }
        }
    }

    let desired_ids: std::collections::BTreeSet<WindowId> = out
        .apply
        .iter()
        .map(|i| i.window)
        .chain(out.unchanged.iter().map(|a| a.window))
        .collect();
    for current in applied {
        if !desired_ids.contains(&current.window) {
            out.release.push(current.clone());
        }
    }
    out.release.sort_by_key(|r| r.window);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::ids::{AppKey, RuleId};
    use crate::testutil::{app, window, windows};
    use crate::window::Platform;

    fn profile_with_app_rule(app_key: &str, mode: ProtectionMode) -> Profile {
        let mut profile = Profile::new();
        profile.add_app_rule(AppKey::LinuxAppId(app_key.into()), mode);
        profile
    }

    #[test]
    fn first_tick_plans_apply_for_every_matching_window() {
        let profile = profile_with_app_rule("chrome", ProtectionMode::Exclusion);
        let live = windows(vec![
            window(1, "chrome", "Inbox", Rect::new(0, 0, 800, 600)),
            window(2, "chrome", "Docs", Rect::new(800, 0, 800, 600)),
            window(3, "slack", "General", Rect::new(0, 600, 800, 600)),
        ]);
        let plan = plan(&profile, &live, &[]);
        assert_eq!(plan.apply.len(), 2);
        assert_eq!(plan.apply[0].window, WindowId(1));
        assert_eq!(plan.apply[1].window, WindowId(2));
        assert!(plan.release.is_empty());
    }

    #[test]
    fn second_tick_is_idempotent() {
        let profile = profile_with_app_rule("chrome", ProtectionMode::Exclusion);
        let live = windows(vec![window(1, "chrome", "Inbox", Rect::new(0, 0, 10, 10))]);
        let first = plan(&profile, &live, &[]);
        let applied: Vec<_> = first
            .apply
            .iter()
            .map(|i| AppliedProtection::applied(i.window, i.mode))
            .collect();
        let second = plan(&profile, &live, &applied);
        assert!(second.apply.is_empty());
        assert!(second.release.is_empty());
        assert_eq!(second.unchanged.len(), 1);
    }

    #[test]
    fn closing_a_window_releases_it() {
        let profile = profile_with_app_rule("chrome", ProtectionMode::Exclusion);
        let before = windows(vec![
            window(1, "chrome", "A", Rect::new(0, 0, 10, 10)),
            window(2, "chrome", "B", Rect::new(0, 0, 10, 10)),
        ]);
        let applied: Vec<_> = intents(&profile, &before)
            .0
            .into_iter()
            .map(|i| AppliedProtection::applied(i.window, i.mode))
            .collect();
        let after = windows(vec![window(1, "chrome", "A", Rect::new(0, 0, 10, 10))]);
        let p = plan(&profile, &after, &applied);
        assert_eq!(p.release.len(), 1);
        assert_eq!(p.release[0].window, WindowId(2));
    }

    #[test]
    fn disabling_a_rule_releases_windows() {
        let mut profile = profile_with_app_rule("chrome", ProtectionMode::Exclusion);
        let live = windows(vec![window(1, "chrome", "A", Rect::new(0, 0, 10, 10))]);
        let applied = vec![AppliedProtection::applied(
            WindowId(1),
            ProtectionMode::Exclusion,
        )];
        let rule_id = profile.rules[0].id;
        assert!(profile.set_enabled(rule_id, false));
        let p = plan(&profile, &live, &applied);
        assert!(p.apply.is_empty());
        assert_eq!(p.release.len(), 1);
    }

    #[test]
    fn changing_mode_reapplies() {
        let mut profile = profile_with_app_rule("chrome", ProtectionMode::Exclusion);
        let live = windows(vec![window(1, "chrome", "A", Rect::new(0, 0, 10, 10))]);
        let applied = vec![AppliedProtection::applied(
            WindowId(1),
            ProtectionMode::Exclusion,
        )];
        profile.set_mode(RuleId(1), ProtectionMode::Surface);
        let p = plan(&profile, &live, &applied);
        assert_eq!(p.apply.len(), 1);
        assert_eq!(p.apply[0].mode, ProtectionMode::Surface);
        assert!(p.release.is_empty());
    }

    #[test]
    fn failed_apply_is_retried() {
        let profile = profile_with_app_rule("chrome", ProtectionMode::Exclusion);
        let live = windows(vec![window(1, "chrome", "A", Rect::new(0, 0, 10, 10))]);
        let applied = vec![AppliedProtection::failed(
            WindowId(1),
            ProtectionMode::Exclusion,
            "injection blocked",
        )];
        let p = plan(&profile, &live, &applied);
        assert_eq!(p.apply.len(), 1);
        assert!(p.release.is_empty());
    }

    #[test]
    fn mode_precedence_wins_and_is_reported() {
        let mut profile = Profile::new();
        let app = AppKey::LinuxAppId("chrome".into());
        profile.add_app_rule(app.clone(), ProtectionMode::StrictHide);
        profile.add_app_rule(app, ProtectionMode::Exclusion);
        let live = windows(vec![window(1, "chrome", "A", Rect::new(0, 0, 10, 10))]);
        let (desired, conflicts) = intents(&profile, &live);
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[0].mode, ProtectionMode::Exclusion);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            conflicts[0].requested,
            vec![ProtectionMode::Exclusion, ProtectionMode::StrictHide]
        );
    }

    #[test]
    fn window_rule_only_matches_that_instance() {
        let mut profile = Profile::new();
        profile.add_window_rule(WindowId(7), ProtectionMode::Surface);
        let live = windows(vec![
            window(7, "chrome", "A", Rect::new(0, 0, 10, 10)),
            window(8, "chrome", "B", Rect::new(0, 0, 10, 10)),
        ]);
        let (desired, _) = intents(&profile, &live);
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[0].window, WindowId(7));
    }

    #[test]
    fn hidden_windows_are_still_tracked_but_children_are_not() {
        let profile = profile_with_app_rule("chrome", ProtectionMode::Exclusion);
        let mut hidden = window(1, "chrome", "A", Rect::new(0, 0, 10, 10));
        hidden.visible = false;
        let mut child = window(2, "chrome", "B", Rect::new(0, 0, 10, 10));
        child.top_level = false;
        let live = windows(vec![hidden, child]);
        let (intents, _) = intents(&profile, &live);
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].window, WindowId(1));
    }

    #[test]
    fn downgrade_uses_platform_capabilities() {
        let profile = profile_with_app_rule("chrome", ProtectionMode::Exclusion);
        let live = windows(vec![window(1, "chrome", "A", Rect::new(0, 0, 10, 10))]);
        let mut p = plan(&profile, &live, &[]);
        let caps = PlatformCapabilities::for_platform(Platform::MacOs);
        p.downgrade(&caps);
        assert_eq!(p.apply[0].mode, ProtectionMode::Surface);
    }

    #[test]
    fn app_identity_is_case_insensitive_and_path_insensitive() {
        let mut profile = Profile::new();
        profile.add_app_rule(
            AppKey::WindowsExecutable("C:\\Program Files\\Chrome\\chrome.exe".into()),
            ProtectionMode::Exclusion,
        );
        let mut w = window(1, "chrome", "A", Rect::new(0, 0, 10, 10));
        w.app = AppKey::WindowsExecutable("c:/program files/chrome/CHROME.EXE".into());
        assert_eq!(intents(&profile, &[w]).0.len(), 1);
    }

    #[test]
    fn empty_app_rule_does_not_match_everything() {
        let profile = profile_with_app_rule("", ProtectionMode::Exclusion);
        let live = windows(vec![window(1, "chrome", "A", Rect::new(0, 0, 10, 10))]);
        assert!(intents(&profile, &live).0.is_empty());
    }

    #[test]
    fn helper_app_is_unused() {
        // keep testutil::app exercised
        assert_eq!(app("x").normalized(), "x");
    }
}
