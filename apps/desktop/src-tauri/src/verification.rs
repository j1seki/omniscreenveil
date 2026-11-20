//! On-device checks keep fixture rules and native probe windows separate from
//! the user's profile. Screenshots stay in memory; reports contain observations.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
use veil_core::{
    testutil, AutoArm, ConfigStore, Platform, Profile, ProtectionMode, RecoveryJournal,
    RecoveryRecord, Rect, WindowId,
};
use veil_protect::{Engine, MemoryBackend};

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Passed,
    Failed,
    Blocked,
}

#[derive(Serialize)]
pub struct Check {
    id: &'static str,
    group: &'static str,
    label: &'static str,
    status: Status,
    evidence: String,
}

fn result(
    id: &'static str,
    group: &'static str,
    label: &'static str,
    check: Result<String, String>,
) -> Check {
    let (status, evidence) = match check {
        Ok(evidence) => (Status::Passed, evidence),
        Err(evidence) => (Status::Failed, evidence),
    };
    Check {
        id,
        group,
        label,
        status,
        evidence,
    }
}

fn blocked(
    id: &'static str,
    group: &'static str,
    label: &'static str,
    reason: impl Into<String>,
) -> Check {
    Check {
        id,
        group,
        label,
        status: Status::Blocked,
        evidence: reason.into(),
    }
}

fn verify(
    condition: bool,
    evidence: impl Into<String>,
    failure: impl Into<String>,
) -> Result<String, String> {
    if condition {
        Ok(evidence.into())
    } else {
        Err(failure.into())
    }
}

fn rules_check() -> Result<String, String> {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let store = ConfigStore::new(dir.path().join("profile.json"));
    let first = testutil::window(1, "editor", "First", Rect::new(0, 0, 400, 240));
    let second = testutil::window(2, "editor", "Second", first.bounds);
    let mut profile = Profile::new();
    profile.add_app_rule(first.app.clone(), ProtectionMode::StrictHide);
    let rule = profile.add_app_rule(first.app.clone(), ProtectionMode::Surface);
    store.save(&profile).map_err(|err| err.to_string())?;
    let saved = store.load().map_err(|err| err.to_string())?;
    if saved != profile || saved.rules.len() != 1 || saved.rules[0].mode != ProtectionMode::Surface
    {
        return Err("A replaced rule did not round-trip as one saved rule.".into());
    }
    let mut engine = Engine::new(saved, MemoryBackend::new(Platform::Windows));
    if engine.tick(std::slice::from_ref(&first)).applied.len() != 1 {
        return Err("The saved rule did not apply to the first matching window.".into());
    }
    let windows = [first, second];
    if engine.tick(&windows).applied.len() != 1 || engine.tick(&windows).unchanged != 2 {
        return Err("The app rule did not apply once to each matching window.".into());
    }
    engine.profile.set_enabled(rule, false);
    if engine.tick(&windows).released.len() != 2 || !engine.applied().is_empty() {
        return Err("Disabling the rule did not release both fixture windows.".into());
    }
    engine.profile.remove_rule(rule);
    store.save(&engine.profile).map_err(|err| err.to_string())?;
    verify(store.load().map_err(|err| err.to_string())?.rules.is_empty(),
        "Saved, replaced, applied to a new matching window, disabled, and removed an app rule in a temporary profile.",
        "The deleted fixture rule was present after reopening the profile.")
}

fn automation_check() -> Result<String, String> {
    let capture = testutil::window(
        1,
        "discord",
        "Capture app fixture",
        Rect::new(0, 0, 300, 220),
    );
    let windows = [capture];
    let mut engine = Engine::new(Profile::new(), MemoryBackend::new(Platform::Windows));
    engine
        .profile
        .add_app_rule(windows[0].app.clone(), ProtectionMode::Surface);
    let manual = !engine.should_arm(&windows, false) && engine.should_arm(&windows, true);
    engine.profile.auto_arm = AutoArm::CaptureAppRunning;
    let trigger = engine.should_arm(&windows, false) && !engine.should_arm(&[], false);
    engine.profile.auto_arm = AutoArm::Always;
    let always = engine.should_arm(&[], false);
    let paused = super::stream_protection(&engine, true, true, &windows, |_| false)
        .masks
        .is_empty();
    verify(manual && trigger && always && paused,
        "Manual, capture-app, and Always policies returned the expected state. A paused session produced no planned masks.",
        format!("Unexpected automation readback: manual={manual}, capture trigger={trigger}, always={always}, pause={paused}."))
}

fn failed_release_check() -> Result<String, String> {
    let target = testutil::window(1, "editor", "Release fixture", Rect::new(0, 0, 300, 220));
    let mut profile = Profile::new();
    profile.add_app_rule(target.app.clone(), ProtectionMode::StrictHide);
    let mut engine = Engine::new(profile, MemoryBackend::new(Platform::Windows));
    if engine.tick(std::slice::from_ref(&target)).applied.len() != 1 {
        return Err("Could not apply the fixture protection.".into());
    }
    engine.backend.fail_on(target.id);
    let failed = engine.release_all();
    if !failed.iter().any(|(_, error)| error.is_some()) || engine.applied().len() != 1 {
        return Err("A failed release was incorrectly reported as cleared.".into());
    }
    engine.backend.heal(target.id);
    let released = engine.release_all();
    verify(
        released == [(target.id, None)] && engine.applied().is_empty(),
        "A rejected release stayed tracked. Retrying successfully cleared the protection record.",
        "The retry failed to clear the tracked record.",
    )
}

fn recovery_check() -> Result<String, String> {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let journal = RecoveryJournal::new(dir.path());
    let record = RecoveryRecord {
        window: WindowId(41),
        pid: 401,
        mode: ProtectionMode::StrictHide,
        saved_at_unix: 10,
    };
    journal
        .save(std::slice::from_ref(&record))
        .map_err(|err| err.to_string())?;
    let readback = journal.load().map_err(|err| err.to_string())?;
    journal.clear().map_err(|err| err.to_string())?;
    verify(
        readback.first().is_some_and(|saved| {
            saved.window == record.window && saved.pid == record.pid && saved.mode == record.mode
        }) && journal.load().map_err(|err| err.to_string())?.is_empty(),
        "The temporary recovery journal preserved window identity and cleared after restoration.",
        "The saved recovery record did not round-trip or clear correctly.",
    )
}

fn compositor_check() -> Result<String, String> {
    let token = veil_canary::Token::fresh();
    let cell = veil_canary::DEFAULT_CELL;
    let mut frame = veil_canary::render(token, cell);
    let baseline = veil_canary::detect(&frame, token, cell).found;
    let plan = veil_surface::SurfacePlan::new(frame.size(), veil_core::FillStyle::veiled())
        .exclude(frame.bounds());
    veil_surface::apply_exclusions(&mut frame, &plan);
    verify(baseline && !veil_canary::detect(&frame, token, cell).found,
        "Detected a unique pixel marker before applying the Surface mask. It was absent from the resulting frame.",
        "The marker was missing at baseline or remained visible after masking.")
}

fn live_checks(app: &AppHandle) -> Vec<Check> {
    let state = app.state::<Mutex<super::AppState>>();
    let (outputs, live, inventory) = match super::lock(&state) {
        Ok(guard) => (
            super::displays(&guard.engine.backend),
            guard.info.live,
            guard.engine.backend.inventory(true),
        ),
        Err(err) => return vec![result("inventory", "Desktop", "Window detection", Err(err))],
    };
    let mut checks = vec![
        if matches!(&inventory, Ok(windows) if live && windows.is_empty()) {
            blocked(
                "inventory",
                "Desktop",
                "Window detection and app metadata",
                "No target windows were open. Open a sample app and rerun the suite.",
            )
        } else if live {
            result("inventory", "Desktop", "Window detection and app metadata", inventory.and_then(|windows| {
            let unique: std::collections::BTreeSet<_> = windows.iter().map(|window| window.id).collect();
            verify(unique.len() == windows.len() && windows.iter().all(|window| !window.app.as_str().is_empty()),
                format!("Read {} live windows with unique handles and app identities. {} provided icons.", windows.len(), windows.iter().filter(|window| !window.icon.is_empty()).count()),
                "The inventory contained duplicate window handles or missing app identities.")
        }))
        } else {
            blocked(
                "inventory",
                "Desktop",
                "Window detection and app metadata",
                "A live desktop backend is required.",
            )
        },
    ];
    match outputs {
        Ok(outputs) if live && !outputs.is_empty() => {
            checks.push(result(
                "displays",
                "Stream",
                "Display enumeration",
                verify(
                    outputs.iter().all(|d| d.bounds.w > 0 && d.bounds.h > 0),
                    format!("Detected {} displays with nonzero bounds.", outputs.len()),
                    "A display reported empty bounds.",
                ),
            ));
            let display = &outputs[0];
            checks.push(result(
                "snapshot",
                "Stream",
                "Live snapshot and output resolution",
                super::render_preview(
                    state.inner(),
                    &app.state::<super::FeedbackAreas>(),
                    Some(&display.id),
                    640,
                    true,
                    None,
                )
                .and_then(|frame| {
                    verify(
                        frame.png.starts_with(b"\x89PNG\r\n\x1a\n")
                            && frame.width > 0
                            && frame.height > 0
                            && frame.width.max(frame.height) <= 640,
                        format!(
                            "Captured a real {}×{} PNG from {} with a 640 px limit.",
                            frame.width, frame.height, display.name
                        ),
                        "The captured image was invalid or exceeded the requested resolution.",
                    )
                }),
            ));
        }
        Ok(_) => {
            checks.push(blocked(
                "displays",
                "Stream",
                "Display enumeration",
                "The current backend reported no live displays.",
            ));
            checks.push(blocked(
                "snapshot",
                "Stream",
                "Live snapshot and output resolution",
                "A live display is required.",
            ));
        }
        Err(error) => {
            checks.push(result(
                "displays",
                "Stream",
                "Display enumeration",
                Err(error),
            ));
            checks.push(blocked(
                "snapshot",
                "Stream",
                "Live snapshot and output resolution",
                "Display enumeration failed.",
            ));
        }
    }
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);
    checks.push(result(
        "shortcut",
        "Desktop",
        "Global protection shortcut",
        verify(
            app.global_shortcut().is_registered(shortcut),
            "The OS shortcut manager has Ctrl+Shift+V registered.",
            "Ctrl+Shift+V is unavailable in this session.",
        ),
    ));
    checks.push(result(
        "startup",
        "Desktop",
        "Launch-at-login readback",
        app.autolaunch()
            .is_enabled()
            .map(|enabled| {
                format!(
                    "The OS reports launch at login is {}.",
                    if enabled { "enabled" } else { "disabled" }
                )
            })
            .map_err(|err| err.to_string()),
    ));
    checks.push(result(
        "settings",
        "Desktop",
        "Saved playback and window preferences",
        super::preferences::verify_preferences(&app.state::<super::preferences::PreferenceStore>()),
    ));
    checks
}

static RUNNING: AtomicBool = AtomicBool::new(false);

struct RunGuard;
impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn run_verification(app: AppHandle) -> Result<Vec<Check>, String> {
    if RUNNING.swap(true, Ordering::AcqRel) {
        return Err("The test suite is already running.".into());
    }
    let _guard = RunGuard;
    let current = app.clone();
    let mut checks = tauri::async_runtime::spawn_blocking(move || {
        let mut checks = vec![
            result(
                "rules",
                "Protection",
                "Rule persistence and matching",
                rules_check(),
            ),
            result(
                "automation",
                "Protection",
                "Auto-arm and pause policies",
                automation_check(),
            ),
            result(
                "failure",
                "Protection",
                "Failed release and retry",
                failed_release_check(),
            ),
            result(
                "recovery",
                "Protection",
                "Crash recovery journal",
                recovery_check(),
            ),
            result(
                "surface",
                "Stream",
                "Surface mask pixel test",
                compositor_check(),
            ),
        ];
        checks.extend(live_checks(&current));
        checks
    })
    .await
    .map_err(|err| err.to_string())?;
    #[cfg(windows)]
    checks.extend(windows::run(&app).await);
    #[cfg(not(windows))]
    {
        let _ = &app;
        for (id, label) in [
            ("exclusion", "Windows Exclusion and restore"),
            ("strict-hide", "Windows Strict Hide and restore"),
            ("dxgi", "Windows Native stream"),
            ("dashboard-feedback", "Dashboard feedback region"),
            ("stream-inventory", "Separate Stream window"),
            ("stream-minimize", "Stream stays available after minimize"),
        ] {
            checks.push(blocked(
                id,
                "Windows",
                label,
                "Requires an interactive Windows desktop.",
            ));
        }
    }
    for (id, channel) in [
        ("stable-updater", super::UpdateChannel::Stable),
        ("beta-updater", super::UpdateChannel::Beta),
    ] {
        let label = if channel == super::UpdateChannel::Stable {
            "Stable update manifest"
        } else {
            "Beta update manifest"
        };
        let check = match super::check_update_inner(&app, channel).await {
            Ok((update, true)) => result(
                id,
                "Updates",
                label,
                Ok(match update {
                    Some(update) => format!(
                        "The manifest provided version {} for this platform.",
                        update.version
                    ),
                    None => {
                        "The update channel responded. No newer compatible release was advertised."
                            .into()
                    }
                }),
            ),
            Ok((_, false)) => blocked(
                id,
                "Updates",
                label,
                "The channel has no published manifest.",
            ),
            Err(error) => blocked(
                id,
                "Updates",
                label,
                format!("Could not read the update manifest: {error}"),
            ),
        };
        checks.push(check);
    }
    for check in &checks {
        match check.status {
            Status::Passed => log::info!("{} passed: {}", check.label, check.evidence),
            Status::Failed => log::warn!("{} failed: {}", check.label, check.evidence),
            Status::Blocked => log::debug!("{} blocked: {}", check.label, check.evidence),
        }
    }
    Ok(checks)
}

#[cfg(windows)]
mod windows;
