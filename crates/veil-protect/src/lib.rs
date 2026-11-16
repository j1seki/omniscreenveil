//! Platform backends implement [`ProtectionBackend`]; the [`Engine`] keeps the
//! world in sync with the user's rules. The engine never trusts a single apply
//! call: every tick re-derives intent from the live window list and re-applies
//! anything that is missing, changed or failed.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use veil_core::capability::{PlatformCapabilities, ProtectionMode};
use veil_core::capture_apps::capture_app_running;
use veil_core::reconcile::{plan_with_caps, Conflict, ReconcilePlan};
use veil_core::rules::AutoArm;
use veil_core::state::{AppliedProtection, ApplyStatus, ProtectionIntent};
use veil_core::window::{Platform, WindowInfo};
use veil_core::{Profile, WindowId};

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("{mode:?} is not supported on {platform:?}: {reason}")]
    Unsupported {
        platform: Platform,
        mode: ProtectionMode,
        reason: String,
    },
    #[error("window {0} no longer exists")]
    NotFound(WindowId),
    #[error("{0}")]
    Failed(String),
}

impl BackendError {
    pub fn failed(reason: impl Into<String>) -> Self {
        BackendError::Failed(reason.into())
    }
}

/// Contract every platform implementation fulfils.
///
/// Backends are intentionally dumb: they apply one intent to one window and
/// report failure. All policy (rules, precedence, downgrades, retries) lives in
/// the engine so it behaves identically on every OS.
pub trait ProtectionBackend {
    fn platform(&self) -> Platform;

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities::for_platform(self.platform())
    }

    fn apply(&mut self, intent: &ProtectionIntent) -> Result<(), BackendError>;

    fn release(&mut self, window: WindowId) -> Result<(), BackendError>;

    /// Short human-readable description for diagnostics.
    fn describe(&self) -> String {
        format!("{} backend", self.platform().label())
    }
}

/// Event stream suitable for the tray UI and the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Applied {
        window: WindowId,
        mode: ProtectionMode,
    },
    Released {
        window: WindowId,
    },
    Failed {
        window: WindowId,
        reason: String,
    },
    Downgraded {
        window: WindowId,
        requested: ProtectionMode,
        applied: ProtectionMode,
    },
    Conflict {
        window: WindowId,
        chosen: ProtectionMode,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickReport {
    pub applied: Vec<AppliedProtection>,
    pub released: Vec<WindowId>,
    pub failed: Vec<(WindowId, String)>,
    pub unchanged: usize,
    pub conflicts: Vec<Conflict>,
    pub upgraded_to_surface: Vec<WindowId>,
    pub pruned_rules: usize,
}

impl TickReport {
    pub fn did_work(&self) -> bool {
        !self.applied.is_empty() || !self.released.is_empty() || !self.failed.is_empty()
    }
}

/// Outcome of one automatic-monitor pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoTickReport {
    /// Whether the auto-arm policy wanted protection active.
    pub armed: bool,
    /// Reconciliation result when armed.
    pub tick: Option<TickReport>,
    /// Releases performed because the policy disarmed.
    pub released: Vec<(WindowId, Option<String>)>,
}

impl AutoTickReport {
    pub fn did_work(&self) -> bool {
        self.tick.as_ref().is_some_and(TickReport::did_work) || !self.released.is_empty()
    }
}

#[derive(Debug)]
pub struct Engine<B: ProtectionBackend> {
    pub profile: Profile,
    pub backend: B,
    pub caps: PlatformCapabilities,
    applied: BTreeMap<WindowId, AppliedProtection>,
    events: Vec<Event>,
}

impl<B: ProtectionBackend> Engine<B> {
    pub fn new(profile: Profile, backend: B) -> Self {
        let caps = backend.capabilities();
        Self {
            profile,
            backend,
            caps,
            applied: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    pub fn applied(&self) -> Vec<AppliedProtection> {
        self.applied.values().cloned().collect()
    }

    pub fn applied_for(&self, window: WindowId) -> Option<&AppliedProtection> {
        self.applied.get(&window)
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Release every window the engine believes is protected.
    ///
    /// Best-effort and fail-closed in the same direction as `tick`: windows
    /// that fail to release stay in the applied set so the next tick retries,
    /// and the caller gets the error text for each one.
    pub fn release_all(&mut self) -> Vec<(WindowId, Option<String>)> {
        let ids: Vec<WindowId> = self.applied.keys().copied().collect();
        let mut out = Vec::with_capacity(ids.len());
        for window in ids {
            match self.backend.release(window) {
                Ok(()) => {
                    self.applied.remove(&window);
                    self.events.push(Event::Released { window });
                    out.push((window, None));
                }
                Err(err) => {
                    let reason = err.to_string();
                    if let Some(record) = self.applied.get_mut(&window) {
                        record.status = ApplyStatus::Failed {
                            reason: reason.clone(),
                        };
                    }
                    self.events.push(Event::Failed {
                        window,
                        reason: reason.clone(),
                    });
                    out.push((window, Some(reason)));
                }
            }
        }
        out
    }

    pub fn plan(&self, windows: &[WindowInfo]) -> ReconcilePlan {
        let applied: Vec<_> = self.applied.values().cloned().collect();
        plan_with_caps(&self.profile, windows, &applied, &self.caps)
    }

    /// Whether the configured auto-arm policy wants protection active.
    ///
    /// `explicit` is the user's arm switch. `Off` requires it, `Always` ignores
    /// it, and `CaptureAppRunning` uses it as a manual override on top of the
    /// running-capture-app heuristic.
    pub fn should_arm(&self, windows: &[WindowInfo], explicit: bool) -> bool {
        match self.profile.auto_arm {
            AutoArm::Off => explicit,
            AutoArm::Always => true,
            AutoArm::CaptureAppRunning => explicit || capture_app_running(windows),
        }
    }

    /// One monitor pass: arm and reconcile when the policy says so, otherwise
    /// release anything currently applied while keeping the rules intact.
    pub fn auto_tick(&mut self, windows: &[WindowInfo], explicit: bool) -> AutoTickReport {
        let armed = self.should_arm(windows, explicit);
        if armed {
            let tick = self.tick(windows);
            AutoTickReport {
                armed,
                tick: Some(tick),
                released: Vec::new(),
            }
        } else if self.applied.is_empty() {
            AutoTickReport {
                armed,
                tick: None,
                released: Vec::new(),
            }
        } else {
            let released = self.release_all();
            AutoTickReport {
                armed,
                tick: None,
                released,
            }
        }
    }

    /// One reconciliation pass. Call on a timer, on window events, or on any
    /// user action; it is idempotent.
    pub fn tick(&mut self, windows: &[WindowInfo]) -> TickReport {
        let pruned_rules = self.profile.prune_dead_window_rules(windows);
        let mut plan = self.plan(windows);

        let mut report = TickReport {
            unchanged: plan.unchanged.len(),
            pruned_rules,
            ..TickReport::default()
        };
        for conflict in &plan.conflicts {
            self.events.push(Event::Conflict {
                window: conflict.window,
                chosen: conflict.chosen,
            });
        }
        report.conflicts = plan.conflicts.clone();

        for intent in plan.apply.drain(..) {
            let requested = self
                .profile
                .rules
                .iter()
                .find(|r| r.id == intent.rule)
                .map(|r| r.mode)
                .unwrap_or(intent.mode);
            if requested != intent.mode {
                self.events.push(Event::Downgraded {
                    window: intent.window,
                    requested,
                    applied: intent.mode,
                });
                report.upgraded_to_surface.push(intent.window);
            }

            // A window can only be in one mode. Release the previous
            // protection before applying a different one, otherwise strict
            // hide would stay in effect underneath a Surface mask, or an
            // exclusion would survive a switch to strict hide.
            if let Some(current) = self.applied.get(&intent.window).cloned() {
                if current.status.is_applied() && current.mode != intent.mode {
                    match self.backend.release(intent.window) {
                        Ok(()) => {
                            self.applied.remove(&intent.window);
                            self.events.push(Event::Released {
                                window: intent.window,
                            });
                        }
                        Err(err) => {
                            let reason = format!(
                                "could not release {} before switching to {}: {err}",
                                current.mode.label(),
                                intent.mode.label()
                            );
                            self.events.push(Event::Failed {
                                window: intent.window,
                                reason: reason.clone(),
                            });
                            report.failed.push((intent.window, reason));
                            continue;
                        }
                    }
                }
            }

            match self.backend.apply(&intent) {
                Ok(()) => {
                    let record = AppliedProtection::applied(intent.window, intent.mode);
                    self.applied.insert(intent.window, record.clone());
                    self.events.push(Event::Applied {
                        window: intent.window,
                        mode: intent.mode,
                    });
                    report.applied.push(record);
                }
                Err(err) => {
                    let reason = err.to_string();
                    let record =
                        AppliedProtection::failed(intent.window, intent.mode, reason.clone());
                    self.applied.insert(intent.window, record);
                    self.events.push(Event::Failed {
                        window: intent.window,
                        reason: reason.clone(),
                    });
                    report.failed.push((intent.window, reason));
                }
            }
        }

        for current in plan.release {
            match self.backend.release(current.window) {
                Ok(()) => {
                    self.applied.remove(&current.window);
                    self.events.push(Event::Released {
                        window: current.window,
                    });
                    report.released.push(current.window);
                }
                Err(err) => {
                    // Keep it in the applied set so the next tick retries the
                    // release instead of leaking a hidden window forever.
                    let reason = format!("release failed: {err}");
                    self.applied.insert(
                        current.window,
                        AppliedProtection::failed(current.window, current.mode, reason.clone()),
                    );
                    self.events.push(Event::Failed {
                        window: current.window,
                        reason: reason.clone(),
                    });
                    report.failed.push((current.window, reason));
                }
            }
        }

        report.applied.sort_by_key(|a| a.window);
        report.released.sort();
        report
    }
}

/// In-memory backend used by unit tests, the CLI self-test and the UI preview.
#[derive(Debug)]
pub struct MemoryBackend {
    platform: Platform,
    applied: BTreeMap<WindowId, ProtectionMode>,
    failing: BTreeSet<WindowId>,
    pub calls: Vec<String>,
}

impl MemoryBackend {
    pub fn new(platform: Platform) -> Self {
        Self {
            platform,
            applied: BTreeMap::new(),
            failing: BTreeSet::new(),
            calls: Vec::new(),
        }
    }

    pub fn fail_on(&mut self, window: WindowId) {
        self.failing.insert(window);
    }

    pub fn heal(&mut self, window: WindowId) {
        self.failing.remove(&window);
    }

    pub fn applied_modes(&self) -> BTreeMap<WindowId, ProtectionMode> {
        self.applied.clone()
    }
}

impl ProtectionBackend for MemoryBackend {
    fn platform(&self) -> Platform {
        self.platform
    }

    fn apply(&mut self, intent: &ProtectionIntent) -> Result<(), BackendError> {
        self.calls
            .push(format!("apply:{:?}:{:?}", intent.window, intent.mode));
        if !self.caps().supports(intent.mode) {
            return Err(BackendError::Unsupported {
                platform: self.platform,
                mode: intent.mode,
                reason: "backend rejected unsupported mode".into(),
            });
        }
        if self.failing.contains(&intent.window) {
            return Err(BackendError::failed("injected failure"));
        }
        self.applied.insert(intent.window, intent.mode);
        Ok(())
    }

    fn release(&mut self, window: WindowId) -> Result<(), BackendError> {
        self.calls.push(format!("release:{window:?}"));
        if self.failing.contains(&window) {
            return Err(BackendError::failed("injected release failure"));
        }
        self.applied.remove(&window);
        Ok(())
    }
}

impl MemoryBackend {
    fn caps(&self) -> PlatformCapabilities {
        PlatformCapabilities::for_platform(self.platform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veil_core::ids::AppKey;
    use veil_core::testutil::{window, windows};

    fn chrome_rule(mode: ProtectionMode) -> Profile {
        let mut p = Profile::new();
        p.add_app_rule(AppKey::LinuxAppId("chrome".into()), mode);
        p
    }

    fn live() -> Vec<WindowInfo> {
        windows(vec![
            window(1, "chrome", "A", veil_core::Rect::new(0, 0, 800, 600)),
            window(2, "chrome", "B", veil_core::Rect::new(0, 0, 800, 600)),
        ])
    }

    #[test]
    fn applies_every_matching_window_once() {
        let mut engine = Engine::new(
            chrome_rule(ProtectionMode::Exclusion),
            MemoryBackend::new(Platform::Windows),
        );
        let report = engine.tick(&live());
        assert_eq!(report.applied.len(), 2);
        assert_eq!(engine.backend.applied_modes().len(), 2);
        assert_eq!(engine.backend.calls.len(), 2);
    }

    #[test]
    fn second_tick_is_a_no_op() {
        let mut engine = Engine::new(
            chrome_rule(ProtectionMode::Exclusion),
            MemoryBackend::new(Platform::Windows),
        );
        engine.tick(&live());
        let calls = engine.backend.calls.len();
        let report = engine.tick(&live());
        assert!(!report.did_work());
        assert_eq!(report.unchanged, 2);
        assert_eq!(engine.backend.calls.len(), calls);
    }

    #[test]
    fn new_window_is_picked_up_on_the_next_tick() {
        let mut engine = Engine::new(
            chrome_rule(ProtectionMode::Exclusion),
            MemoryBackend::new(Platform::Windows),
        );
        engine.tick(&live());
        let mut now = live();
        now.push(window(3, "chrome", "C", veil_core::Rect::new(0, 0, 10, 10)));
        let report = engine.tick(&now);
        assert_eq!(report.applied.len(), 1);
        assert_eq!(report.applied[0].window, WindowId(3));
    }

    #[test]
    fn closed_window_is_released() {
        let mut engine = Engine::new(
            chrome_rule(ProtectionMode::Exclusion),
            MemoryBackend::new(Platform::Windows),
        );
        engine.tick(&live());
        let report = engine.tick(&[window(1, "chrome", "A", veil_core::Rect::new(0, 0, 10, 10))]);
        assert_eq!(report.released, vec![WindowId(2)]);
        assert_eq!(engine.backend.applied_modes().len(), 1);
    }

    #[test]
    fn failure_is_reported_and_retried() {
        let mut engine = Engine::new(
            chrome_rule(ProtectionMode::Exclusion),
            MemoryBackend::new(Platform::Windows),
        );
        engine.backend.fail_on(WindowId(1));
        let report = engine.tick(&live());
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.applied.len(), 1); // window 2 still protected
        let calls = engine.backend.calls.len();
        engine.tick(&live());
        assert!(
            engine.backend.calls.len() > calls,
            "failed window should be retried"
        );
        engine.backend.heal(WindowId(1));
        let report = engine.tick(&live());
        assert_eq!(report.applied.len(), 1);
        assert!(report.failed.is_empty());
    }

    #[test]
    fn unsupported_mode_is_downgraded_not_failed() {
        let mut engine = Engine::new(
            chrome_rule(ProtectionMode::Exclusion),
            MemoryBackend::new(Platform::MacOs),
        );
        let report = engine.tick(&live());
        assert!(report.failed.is_empty());
        assert_eq!(
            engine.backend.applied_modes().get(&WindowId(1)),
            Some(&ProtectionMode::Surface)
        );
        assert_eq!(report.upgraded_to_surface.len(), 2);
        let events = engine.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Downgraded {
                requested: ProtectionMode::Exclusion,
                applied: ProtectionMode::Surface,
                ..
            }
        )));

        let calls = engine.backend.calls.len();
        let second = engine.tick(&live());
        assert!(!second.did_work(), "downgraded windows must stay unchanged");
        assert_eq!(second.unchanged, 2);
        assert_eq!(engine.backend.calls.len(), calls);
    }

    #[test]
    fn switching_modes_releases_the_previous_protection_first() {
        let mut profile = Profile::new();
        profile.add_app_rule(
            AppKey::LinuxAppId("chrome".into()),
            ProtectionMode::StrictHide,
        );
        let mut engine = Engine::new(profile, MemoryBackend::new(Platform::LinuxX11));
        engine.tick(&live());
        assert_eq!(
            engine.backend.applied_modes().get(&WindowId(1)),
            Some(&ProtectionMode::StrictHide)
        );

        engine.profile.rules[0].mode = ProtectionMode::Surface;
        let report = engine.tick(&live());
        assert_eq!(report.applied.len(), 2);
        assert!(
            engine
                .backend
                .calls
                .iter()
                .any(|call| call == "release:WindowId(1)"),
            "previous mode should be released before Surface is applied"
        );
        assert_eq!(
            engine.backend.applied_modes().get(&WindowId(1)),
            Some(&ProtectionMode::Surface)
        );
    }

    #[test]
    fn conflicts_are_exposed() {
        let mut profile = chrome_rule(ProtectionMode::StrictHide);
        profile.add_rule(
            veil_core::TargetRef::App(AppKey::LinuxAppId("chrome".into())),
            ProtectionMode::Surface,
        );
        let mut engine = Engine::new(profile, MemoryBackend::new(Platform::Windows));
        let report = engine.tick(&live());
        assert_eq!(report.conflicts.len(), 2);
    }

    #[test]
    fn disabling_the_rule_releases_everything() {
        let mut profile = chrome_rule(ProtectionMode::Exclusion);
        let mut engine = Engine::new(profile.clone(), MemoryBackend::new(Platform::Windows));
        engine.tick(&live());
        let rule = profile.rules[0].id;
        profile.set_enabled(rule, false);
        engine.profile = profile;
        let report = engine.tick(&live());
        assert_eq!(report.released.len(), 2);
        assert!(engine.backend.applied_modes().is_empty());
    }

    #[test]
    fn events_describe_the_tick() {
        let mut engine = Engine::new(
            chrome_rule(ProtectionMode::Exclusion),
            MemoryBackend::new(Platform::Windows),
        );
        engine.tick(&live());
        let events = engine.drain_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], Event::Applied { .. }));
        assert!(engine.drain_events().is_empty());
    }

    fn with_obs() -> Vec<WindowInfo> {
        windows(vec![
            window(1, "chrome", "A", veil_core::Rect::new(0, 0, 800, 600)),
            window(2, "chrome", "B", veil_core::Rect::new(0, 0, 800, 600)),
            window(9, "obs", "OBS 31.0", veil_core::Rect::new(0, 0, 640, 480)),
        ])
    }

    #[test]
    fn off_policy_waits_for_explicit_arm() {
        let mut engine = Engine::new(
            chrome_rule(ProtectionMode::StrictHide),
            MemoryBackend::new(Platform::Windows),
        );

        let report = engine.auto_tick(&live(), false);
        assert!(!report.armed);
        assert!(!report.did_work());
        assert!(engine.applied().is_empty());

        let report = engine.auto_tick(&live(), true);
        assert!(report.armed);
        assert_eq!(report.tick.as_ref().unwrap().applied.len(), 2);
        assert_eq!(engine.applied().len(), 2);
    }

    #[test]
    fn always_policy_arms_without_user_action() {
        let mut profile = chrome_rule(ProtectionMode::StrictHide);
        profile.auto_arm = veil_core::AutoArm::Always;
        let mut engine = Engine::new(profile, MemoryBackend::new(Platform::Windows));

        let report = engine.auto_tick(&live(), false);
        assert!(report.armed);
        assert!(report.did_work());
        assert_eq!(engine.applied().len(), 2);
    }

    #[test]
    fn capture_policy_arms_and_releases_with_the_capture_app() {
        let mut profile = chrome_rule(ProtectionMode::StrictHide);
        profile.auto_arm = veil_core::AutoArm::CaptureAppRunning;
        let mut engine = Engine::new(profile, MemoryBackend::new(Platform::Windows));

        let report = engine.auto_tick(&with_obs(), false);
        assert!(report.armed);
        assert_eq!(engine.applied().len(), 2);

        let report = engine.auto_tick(&live(), false);
        assert!(!report.armed);
        assert_eq!(report.released.len(), 2);
        assert!(engine.applied().is_empty());
        assert_eq!(
            engine.profile.rules.len(),
            1,
            "auto-disarm must keep persistent rules"
        );
    }

    #[test]
    fn explicit_arm_overrides_a_missing_capture_app() {
        let mut profile = chrome_rule(ProtectionMode::StrictHide);
        profile.auto_arm = veil_core::AutoArm::CaptureAppRunning;
        let mut engine = Engine::new(profile, MemoryBackend::new(Platform::Windows));

        let report = engine.auto_tick(&live(), true);
        assert!(report.armed);
        assert_eq!(engine.applied().len(), 2);
    }
}
