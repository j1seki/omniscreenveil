//! OmniScreenVeil desktop app.
//!
//! The UI is a thin view over the same crates the CLI uses: `veil-core` for
//! rules and reconciliation, a platform backend from `veil-protect-*`, the
//! Veil Surface compositor and the canary verifier. No policy lives here.

mod backend;
mod preferences;

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;
use veil_core::capability::{PlatformCapabilities, ProtectionMode};
use veil_core::ids::{AppKey, RuleId, WindowId};
use veil_core::recovery::{
    intended_snapshot, matches_live, snapshot as recovery_snapshot, RecoveryJournal, RecoveryRecord,
};
use veil_core::state::AppliedProtection;
use veil_core::{plan, AutoArm, ConfigStore, FillStyle, Profile, Rect, WindowInfo};
use veil_protect::{Engine, ProtectionBackend, TickReport};
use veil_surface::Frame;

use backend::{BackendInfo, DesktopBackend};

pub struct AppState {
    engine: Engine<DesktopBackend>,
    store: ConfigStore,
    info: BackendInfo,
    /// User intent from "Arm now"/"Release all"; auto-arm can override it.
    armed: bool,
    /// Session override: disarming must not rewrite the saved automation policy.
    paused: bool,
    /// Journal of protections with OS-visible side effects for crash recovery.
    recovery: RecoveryJournal,
}

impl AppState {
    fn save(&self) -> Result<(), String> {
        self.store
            .save(&self.engine.profile)
            .map_err(|err| err.to_string())
    }

    /// Persist the current applied set so a crash can be recovered on next start.
    fn journal(&self) {
        let windows = match self.engine.backend.list_windows() {
            Ok(windows) => windows,
            Err(_) => return,
        };
        let records = recovery_snapshot(&self.engine.applied(), &windows);
        if let Err(err) = self.recovery.save(&records) {
            eprintln!("veil: could not update recovery journal: {err}");
        }
    }
}

/// Release protections left behind by a previous, crashed session.
///
/// Returns the number of records that were actually restored. Records for
/// windows that no longer exist (or whose handle was reused by another PID)
/// are dropped without touching anything.
fn recover_previous_session(backend: &mut DesktopBackend, journal: &RecoveryJournal) -> usize {
    let records: Vec<RecoveryRecord> = match journal.load() {
        Ok(records) => records,
        Err(err) => {
            eprintln!("veil: could not read recovery journal: {err}");
            return 0;
        }
    };
    if records.is_empty() {
        return 0;
    }
    let mut windows = match backend.list_windows() {
        Ok(windows) => windows,
        Err(err) => {
            eprintln!("veil: cannot recover, inventory unavailable: {err}");
            return 0;
        }
    };
    let own_pid = std::process::id();
    windows.retain(|window| window.pid != own_pid);
    let mut restored = 0;
    let mut pending = Vec::new();
    for record in &records {
        if !matches_live(record, &windows) {
            continue;
        }
        match ProtectionBackend::release(backend, record.window) {
            Ok(()) => {
                eprintln!(
                    "veil: recovered window {} left protected by a previous session",
                    record.window
                );
                restored += 1;
            }
            Err(err) => {
                eprintln!("veil: could not recover {}: {err}", record.window);
                pending.push(record.clone());
            }
        }
    }
    if let Err(err) = journal.save(&pending) {
        eprintln!("veil: could not update recovery journal: {err}");
    }
    restored
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    windows: Vec<veil_core::WindowInfo>,
    backend: BackendInfo,
    applied: Vec<AppliedProtection>,
    armed: bool,
    capture_active: bool,
    automation_paused: bool,
    mac_permissions: Option<MacPermissions>,
}

#[derive(Debug, Serialize)]
struct MacPermissions {
    screen_recording: bool,
    accessibility: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum MacPermission {
    ScreenRecording,
    Accessibility,
}

#[tauri::command]
fn mac_permission(permission: MacPermission, settings: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use veil_protect_macos::MacBackend;

        if settings {
            let pane = match permission {
                MacPermission::ScreenRecording => "Privacy_ScreenCapture",
                MacPermission::Accessibility => "Privacy_Accessibility",
            };
            let status = std::process::Command::new("/usr/bin/open")
                .arg(format!(
                    "x-apple.systempreferences:com.apple.preference.security?{pane}"
                ))
                .status()
                .map_err(|err| format!("Could not open System Settings: {err}"))?;
            if !status.success() {
                return Err("Could not open System Settings.".into());
            }
        } else {
            match permission {
                MacPermission::ScreenRecording => MacBackend::request_screen_recording(),
                MacPermission::Accessibility => MacBackend::request_accessibility(),
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (permission, settings);
        Err("These permissions are only available on macOS.".into())
    }
}

#[derive(Debug, Serialize)]
pub struct SurfacePreview {
    png_base64: String,
    width: u32,
    height: u32,
    masked: usize,
    source: String,
    armed: bool,
}

#[derive(Debug, Serialize)]
pub struct ReleaseReport {
    released: usize,
    failed: Vec<String>,
}

fn lock<'a>(
    state: &'a State<'_, Mutex<AppState>>,
) -> Result<std::sync::MutexGuard<'a, AppState>, String> {
    state
        .inner()
        .lock()
        .map_err(|_| "application state is poisoned".to_string())
}

#[tauri::command]
fn capabilities(state: State<'_, Mutex<AppState>>) -> Result<PlatformCapabilities, String> {
    Ok(lock(&state)?.engine.caps.clone())
}

/// Every entry in the honest support matrix, for the comparison table.
#[tauri::command]
fn capability_matrix() -> Vec<PlatformCapabilities> {
    [
        veil_core::Platform::Windows,
        veil_core::Platform::MacOs,
        veil_core::Platform::LinuxX11,
        veil_core::Platform::LinuxWayland,
    ]
    .into_iter()
    .map(PlatformCapabilities::for_platform)
    .collect()
}

#[tauri::command]
async fn inventory(state: State<'_, Mutex<AppState>>) -> Result<Inventory, String> {
    let state = lock(&state)?;
    let windows = state.engine.backend.inventory(true)?;
    let capture_active = veil_core::capture_app_running(&windows);
    let armed = !state.paused && state.engine.should_arm(&windows, state.armed);
    Ok(Inventory {
        windows,
        backend: state.info.clone(),
        applied: state.engine.applied(),
        armed,
        capture_active,
        automation_paused: state.paused,
        mac_permissions: match &state.engine.backend {
            #[cfg(target_os = "macos")]
            DesktopBackend::Mac(backend) => Some(MacPermissions {
                screen_recording: veil_protect_macos::MacBackend::screen_recording_granted(),
                accessibility: backend.accessibility_trusted(),
            }),
            _ => None,
        },
    })
}

#[tauri::command]
async fn get_profile(state: State<'_, Mutex<AppState>>) -> Result<Profile, String> {
    Ok(lock(&state)?.engine.profile.clone())
}

#[tauri::command]
fn add_app_rule(
    state: State<'_, Mutex<AppState>>,
    app: AppKey,
    mode: ProtectionMode,
    enabled: Option<bool>,
) -> Result<Profile, String> {
    if app.as_str().trim().is_empty() {
        return Err("Enter an application identity first.".into());
    }
    #[cfg(target_os = "macos")]
    let app = match app {
        AppKey::MacBundle(path) if path.starts_with('/') && path.ends_with(".app") => {
            let plist = std::path::Path::new(&path).join("Contents/Info.plist");
            let output = std::process::Command::new("/usr/bin/plutil")
                .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-", "--"])
                .arg(plist)
                .output()
                .map_err(|err| err.to_string())?;
            let bundle = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !output.status.success() || bundle.is_empty() {
                return Err(
                    "Could not read that app bundle. Enter its bundle ID or an existing .app path."
                        .into(),
                );
            }
            AppKey::MacBundle(bundle)
        }
        other => other,
    };
    let mut state = lock(&state)?;
    let id = state.engine.profile.add_app_rule(app, mode);
    state
        .engine
        .profile
        .set_enabled(id, enabled.unwrap_or(true));
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn add_window_rule(
    state: State<'_, Mutex<AppState>>,
    window: WindowId,
    mode: ProtectionMode,
) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state.engine.profile.add_window_rule(window, mode);
    state.save()?;
    Ok(state.engine.profile.clone())
}

/// Protect every app that currently owns a protectable window.
#[tauri::command]
fn protect_all(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    mode: ProtectionMode,
) -> Result<TickReport, String> {
    let mut state = lock(&state)?;
    let own_pid = std::process::id();
    let windows: Vec<_> = state
        .engine
        .backend
        .list_windows()?
        .into_iter()
        .filter(|window| window.pid != own_pid)
        .collect();
    state.engine.profile.protect_all(&windows, mode);
    state.armed = true;
    state.paused = false;
    let reconcile = plan(&state.engine.profile, &windows, &state.engine.applied());
    let mut records = state.recovery.load().map_err(|err| err.to_string())?;
    for mode in [ProtectionMode::StrictHide, ProtectionMode::Exclusion] {
        let intended: Vec<_> = reconcile
            .apply
            .iter()
            .filter(|intent| intent.mode == mode)
            .filter_map(|intent| windows.iter().find(|w| w.id == intent.window).cloned())
            .collect();
        records.extend(intended_snapshot(&intended, mode));
    }
    records.retain(|record| {
        windows
            .iter()
            .any(|w| w.id == record.window && w.pid == record.pid)
    });
    records.sort_by_key(|record| record.window);
    records.dedup_by_key(|record| record.window);
    records.sort_by_key(|record| record.window);
    state
        .recovery
        .save(&records)
        .map_err(|err| err.to_string())?;
    let report = state.engine.tick(&windows);
    state.journal();
    state.engine.drain_events();
    state.save()?;
    drop(state);
    let _ = app.emit("veil://changed", ());
    Ok(report)
}

#[tauri::command]
fn remove_app_rules(state: State<'_, Mutex<AppState>>, app: AppKey) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    let target = app.normalized();
    state.engine.profile.rules.retain(|rule| {
        !matches!(&rule.target, veil_core::TargetRef::App(key) if key.normalized() == target)
    });
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn remove_window_rules(
    state: State<'_, Mutex<AppState>>,
    window: WindowId,
) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state
        .engine
        .profile
        .rules
        .retain(|rule| !matches!(&rule.target, veil_core::TargetRef::Window(id) if *id == window));
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn remove_rule(state: State<'_, Mutex<AppState>>, id: RuleId) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state.engine.profile.remove_rule(id);
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn set_rule_enabled(
    state: State<'_, Mutex<AppState>>,
    id: RuleId,
    enabled: bool,
) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state.engine.profile.set_enabled(id, enabled);
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn set_rule_mode(
    state: State<'_, Mutex<AppState>>,
    id: RuleId,
    mode: ProtectionMode,
) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state.engine.profile.set_mode(id, mode);
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn reset_profile(state: State<'_, Mutex<AppState>>) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state.engine.profile = Profile::new();
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn tick(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    arm: Option<bool>,
) -> Result<TickReport, String> {
    let mut state = lock(&state)?;
    let windows = state.engine.backend.list_windows()?;
    if arm.unwrap_or(true) {
        state.armed = true;
        state.paused = false;
    }
    if state.paused || !state.engine.should_arm(&windows, state.armed) {
        return Ok(TickReport::default());
    }
    let reconcile = plan(&state.engine.profile, &windows, &state.engine.applied());
    let mut journal_records: Vec<_> = state
        .recovery
        .load()
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(|record| {
            windows
                .iter()
                .any(|w| w.id == record.window && w.pid == record.pid)
        })
        .collect();
    for mode in [ProtectionMode::StrictHide, ProtectionMode::Exclusion] {
        let intended: Vec<_> = reconcile
            .apply
            .iter()
            .filter(|intent| intent.mode == mode)
            .filter_map(|intent| windows.iter().find(|w| w.id == intent.window).cloned())
            .collect();
        if !intended.is_empty() {
            journal_records.extend(intended_snapshot(&intended, mode));
        }
    }
    journal_records.sort_by_key(|record| record.window);
    journal_records.dedup_by_key(|record| record.window);
    journal_records.sort_by_key(|record| record.window);
    state
        .recovery
        .save(&journal_records)
        .map_err(|err| err.to_string())?;
    let report = state.engine.tick(&windows);
    state.journal();
    state.engine.drain_events();
    drop(state);
    let _ = app.emit("veil://changed", ());
    Ok(report)
}

#[tauri::command]
fn set_auto_arm(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    auto_arm: AutoArm,
) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state.engine.profile.auto_arm = auto_arm;
    if let Some(windows) = (!state.paused)
        .then(|| state.engine.backend.list_windows().ok())
        .flatten()
    {
        let explicit = state.armed;
        let reconcile = plan(&state.engine.profile, &windows, &state.engine.applied());
        let mut intended = Vec::new();
        for mode in [ProtectionMode::StrictHide, ProtectionMode::Exclusion] {
            intended.extend(
                reconcile
                    .apply
                    .iter()
                    .filter(|intent| intent.mode == mode)
                    .filter_map(|intent| windows.iter().find(|w| w.id == intent.window))
                    .map(|w| veil_core::RecoveryRecord {
                        window: w.id,
                        pid: w.pid,
                        mode,
                        saved_at_unix: 0,
                    }),
            );
        }
        let mut records: Vec<_> = state
            .recovery
            .load()
            .map_err(|err| err.to_string())?
            .into_iter()
            .filter(|record| {
                windows
                    .iter()
                    .any(|w| w.id == record.window && w.pid == record.pid)
            })
            .collect();
        records.retain(|record| {
            !reconcile
                .apply
                .iter()
                .any(|intent| intent.window == record.window)
        });
        records.extend(intended);
        records.sort_by_key(|record| record.window);
        state
            .recovery
            .save(&records)
            .map_err(|err| err.to_string())?;
        state.engine.auto_tick(&windows, explicit);
        state.engine.drain_events();
        state.journal();
    }
    state.save()?;
    let profile = state.engine.profile.clone();
    drop(state);
    let _ = app.emit("veil://changed", ());
    Ok(profile)
}

#[tauri::command]
fn release_all(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<ReleaseReport, String> {
    let mut state = lock(&state)?;
    state.armed = false;
    state.paused = true;
    let results = state.engine.release_all();
    let mut pending = Vec::new();
    for (window, error) in &results {
        if error.is_some() {
            let applied = state.engine.applied();
            if let Some(record) = applied.iter().find(|record| record.window == *window) {
                if let Ok(windows) = state.engine.backend.list_windows() {
                    if let Some(w) = windows.iter().find(|w| w.id == *window) {
                        pending.push(RecoveryRecord {
                            window: *window,
                            pid: w.pid,
                            mode: record.mode,
                            saved_at_unix: 0,
                        });
                    }
                }
            }
        }
    }
    state
        .recovery
        .save(&pending)
        .map_err(|err| err.to_string())?;
    state.engine.drain_events();
    let failed = results
        .iter()
        .filter_map(|(window, err)| err.as_ref().map(|err| format!("{window}: {err}")))
        .collect::<Vec<_>>();
    drop(state);
    let _ = app.emit("veil://changed", ());
    Ok(ReleaseReport {
        released: results.len() - failed.len(),
        failed,
    })
}

/// Render the shareable Veil Surface.
///
/// Uses a real X11 capture when one is available, otherwise a synthetic
/// desktop, and always labels which one in `source`.
// Capture can take seconds on macOS; never run it on the webview event loop.
#[tauri::command]
async fn preview(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    monitor: Option<String>,
    motion: Option<bool>,
    live_only: Option<bool>,
) -> Result<SurfacePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if window.is_minimized().unwrap_or(false) || !window.is_visible().unwrap_or(false) {
            return Err("Capture paused while this window is hidden.".into());
        }
        let state = app.state::<Mutex<AppState>>();
        render_preview(
            state.inner(),
            monitor.as_deref(),
            motion.unwrap_or(false),
            live_only.unwrap_or(false),
        )
    })
    .await
    .map_err(|err| err.to_string())?
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct Display {
    id: String,
    name: String,
    bounds: Rect,
}

fn displays(backend: &DesktopBackend) -> Result<Vec<Display>, String> {
    let named = |(name, bounds): (String, Rect)| Display {
        id: name.clone(),
        name,
        bounds,
    };
    let outputs = match backend {
        #[cfg(target_os = "linux")]
        DesktopBackend::X11(x11) => x11.platform().outputs().into_iter().map(named).collect(),
        #[cfg(target_os = "linux")]
        DesktopBackend::Wayland(wayland) => wayland
            .outputs()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(named)
            .collect(),
        #[cfg(windows)]
        DesktopBackend::Windows(_) => veil_protect_win::WinBackend::outputs()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(named)
            .collect(),
        #[cfg(target_os = "macos")]
        DesktopBackend::Mac(mac) => mac
            .displays()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|d| Display {
                id: d.id.to_string(),
                name: d.name,
                bounds: d.bounds,
            })
            .collect(),
        _ => Vec::new(),
    };
    let _ = named;
    let mut outputs: Vec<Display> = outputs;
    outputs.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(outputs)
}

#[tauri::command]
async fn surface_displays(app: tauri::AppHandle) -> Result<Vec<Display>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Mutex<AppState>>();
        let guard = lock(&state)?;
        displays(&guard.engine.backend)
    })
    .await
    .map_err(|err| err.to_string())?
}

fn surface_targets<B: ProtectionBackend>(
    engine: &Engine<B>,
    armed: bool,
    paused: bool,
    windows: &[WindowInfo],
) -> BTreeSet<WindowId> {
    let mut surface: BTreeSet<WindowId> = engine
        .applied()
        .into_iter()
        .filter(|protection| {
            protection.status.is_applied() && protection.mode == ProtectionMode::Surface
        })
        .map(|protection| protection.window)
        .collect();

    // The monitor is periodic. Don't expose newly opened windows that match
    // armed Surface rules while waiting for the next reconciliation pass.
    if !paused && engine.should_arm(windows, armed) {
        surface.extend(
            engine
                .plan(windows)
                .apply
                .into_iter()
                .filter(|intent| intent.mode == ProtectionMode::Surface)
                .map(|intent| intent.window),
        );
    }
    surface
}

#[cfg(test)]
mod surface_tests {
    use super::*;
    use veil_core::{Platform, Rect};
    use veil_protect::MemoryBackend;

    #[test]
    fn geometry_masks_reject_changed_targets_but_ignore_unprotected_motion() {
        let before = veil_core::testutil::demo_windows();
        let targets = BTreeSet::from([before[0].id]);
        let mut after = before.clone();
        after[1].bounds.x += 100;
        assert!(mask_geometry_stable(&before, &targets, &after, &targets));
        let expanded = BTreeSet::from([before[0].id, before[1].id]);
        assert!(!mask_geometry_stable(&before, &targets, &after, &expanded));
        after[0].bounds.x += 1;
        assert!(!mask_geometry_stable(&before, &targets, &after, &targets));
    }

    #[test]
    fn reduced_preview_reads_only_pixels_within_the_selected_display() {
        let red = veil_core::Color::rgb(255, 0, 0);
        let blue = veil_core::Color::rgb(0, 0, 255);
        let mut desktop = Frame::filled(8, 4, veil_surface::PixelFormat::Bgra8, red);
        desktop.fill_rect(Rect::new(4, 0, 4, 4), FillStyle::Solid { color: blue });
        let preview = sample_frame(&desktop, Rect::new(4, 0, 4, 4), 2, 2);
        assert_eq!(preview.size(), veil_core::Size::new(2, 2));
        assert!(preview
            .pixels
            .chunks_exact(4)
            .all(|p| p == [255, 0, 0, 255]));
    }

    #[test]
    fn surface_covers_new_matches_before_monitor_ticks_but_not_when_disarmed() {
        let app = AppKey::MacBundle("example.demo".into());
        let mut profile = Profile::new();
        profile.add_app_rule(app.clone(), ProtectionMode::Exclusion);
        let engine = Engine::new(profile, MemoryBackend::new(Platform::MacOs));
        let windows = [WindowInfo {
            id: WindowId(42),
            app,
            app_name: "Demo".into(),
            title: "New window".into(),
            pid: 123,
            bounds: Rect::new(0, 0, 320, 240),
            monitor: None,
            visible: true,
            minimized: false,
            top_level: true,
            platform: Platform::MacOs,
            icon: String::new(),
        }];
        assert!(engine.applied().is_empty());
        assert_eq!(
            surface_targets(&engine, true, false, &windows),
            BTreeSet::from([WindowId(42)])
        );
        assert!(surface_targets(&engine, false, false, &windows).is_empty());
        assert!(surface_targets(&engine, true, true, &windows).is_empty());
    }
}

fn render_preview(
    state: &Mutex<AppState>,
    monitor: Option<&str>,
    motion: bool,
    live_only: bool,
) -> Result<SurfacePreview, String> {
    let state = state.lock().map_err(|err| err.to_string())?;
    if live_only && !state.info.live {
        return Err("A live desktop backend is required for the capture test.".into());
    }
    let max_edge = if motion { 960 } else { 1920 };
    #[cfg(target_os = "macos")]
    if matches!(&state.engine.backend, DesktopBackend::Mac(_))
        && !veil_protect_macos::MacBackend::screen_recording_granted()
    {
        return Err("Allow Screen Recording in OmniScreenVeil's main window to use the Veil Surface. macOS may require quitting and reopening the app.".into());
    }
    let windows = state.engine.backend.list_windows()?;
    let surface = surface_targets(&state.engine, state.armed, state.paused, &windows);
    let armed = !state.paused && state.engine.should_arm(&windows, state.armed);
    let outputs = displays(&state.engine.backend)?;
    let selected = monitor
        .map(|id| {
            outputs
                .iter()
                .find(|display| display.id == id)
                .ok_or_else(|| "Selected display is no longer available.".to_string())
        })
        .transpose()?;

    #[cfg(target_os = "macos")]
    if let DesktopBackend::Mac(backend) = &state.engine.backend {
        let active: Vec<_> = if let Some(display) = selected {
            vec![display]
        } else {
            outputs.iter().collect()
        };
        let desktop = active
            .iter()
            .map(|display| display.bounds)
            .reduce(|a, b| a.union(&b))
            .ok_or("No active displays found.")?;
        let scale = f64::from(max_edge).min(f64::from(desktop.w.max(desktop.h)))
            / f64::from(desktop.w.max(desktop.h));
        let map = |r: Rect| {
            Rect::new(
                (f64::from(r.x - desktop.x) * scale).round() as i32,
                (f64::from(r.y - desktop.y) * scale).round() as i32,
                (f64::from(r.w) * scale).round().max(1.0) as u32,
                (f64::from(r.h) * scale).round().max(1.0) as u32,
            )
        };
        let canvas = map(desktop);
        let mut frame = Frame::new(canvas.w, canvas.h, veil_surface::PixelFormat::Bgra8);
        for display in active {
            let dest = map(display.bounds);
            let capture = backend
                .capture_display(
                    display.id.parse::<u32>().map_err(|err| err.to_string())?,
                    &surface,
                    dest.w.max(dest.h),
                )
                .map_err(|err| err.to_string())?;
            let capture = sample_frame(&capture, capture.bounds(), dest.w, dest.h);
            frame.blit_from(&capture, dest);
        }
        // A new window can appear while ScreenCaptureKit is preparing the
        // frame. If the new inventory requires a different filter, drop it.
        let latest = backend.list_windows().map_err(|err| err.to_string())?;
        if !surface_targets(&state.engine, state.armed, state.paused, &latest).is_subset(&surface)
            || displays(&state.engine.backend)? != outputs
        {
            return Err(
                "Protected windows changed during capture; retrying with a new filter.".into(),
            );
        }
        let count = windows
            .iter()
            .filter(|w| w.visible && surface.contains(&w.id) && w.bounds.intersects(&desktop))
            .count();
        drop(state);
        return surface_preview(&frame, count, "ScreenCaptureKit · filtered capture", armed);
    }

    // Composited X11 excludes our windows inside the capture; Wayland's
    // screencopy sees the whole output, so its own windows are veiled out.
    let own_windows = if state.engine.backend.capture_includes_own_windows() {
        state.engine.backend.own_windows()
    } else {
        Vec::new()
    };
    let region = if state.info.platform == veil_core::Platform::LinuxWayland && state.info.live {
        Some(
            selected
                .map(|display| display.bounds)
                .or_else(|| outputs.iter().map(|d| d.bounds).reduce(|a, b| a.union(&b)))
                .ok_or("Could not locate the Wayland outputs.")?,
        )
    } else {
        None
    };
    let (mut source_frame, source_label) = match state.engine.backend.capture_frame(region) {
        Some(frame) => {
            let label = if own_windows.is_empty() {
                "live capture"
            } else {
                "live capture · own windows masked to prevent recursion"
            };
            (frame, label)
        }
        None if !state.info.live => (synthetic_desktop(&windows), "simulated desktop"),
        None => return Err("Live desktop capture failed. The Surface is unavailable; check capture permissions and platform support.".into()),
    };
    // Geometry-based masks are sampled separately from the pixels. Discard
    // the frame if a protected window appeared, moved or resized in between.
    if state.info.live {
        let latest = state.engine.backend.list_windows()?;
        let latest_targets = surface_targets(&state.engine, state.armed, state.paused, &latest);
        if !mask_geometry_stable(&windows, &surface, &latest, &latest_targets)
            || displays(&state.engine.backend)? != outputs
            || (state.engine.backend.capture_includes_own_windows()
                && state.engine.backend.own_windows() != own_windows)
        {
            return Err("Protected windows changed during capture; retrying.".into());
        }
    }
    drop(state);

    if region.is_some_and(|r| r.w != source_frame.width || r.h != source_frame.height) {
        return Err("Unexpected capture scale; withholding the frame.".into());
    }
    let offset = |r: Rect| region.map_or(r, |capture| r.translated(-capture.x, -capture.y));
    let crop = if region.is_none() {
        selected
            .map(|d| {
                d.bounds
                    .intersection(&source_frame.bounds())
                    .ok_or("Display is outside the capture.")
            })
            .transpose()?
    } else {
        None
    }
    .unwrap_or_else(|| source_frame.bounds());
    let mut plan = veil_surface::SurfacePlan::new(source_frame.size(), FillStyle::veiled());
    let mut masked = 0usize;
    for window in &windows {
        if surface.contains(&window.id) && offset(window.bounds).intersects(&crop) {
            let before = plan.exclusions.len();
            plan = plan.exclude(offset(window.bounds));
            if plan.exclusions.len() > before {
                masked += 1;
            }
        }
    }
    for own in own_windows {
        plan = plan.exclude(offset(own));
    }
    veil_surface::apply_exclusions(&mut source_frame, &plan);
    let scale =
        f64::from(max_edge).min(f64::from(crop.w.max(crop.h))) / f64::from(crop.w.max(crop.h));
    if crop == source_frame.bounds() && scale == 1.0 {
        return surface_preview(&source_frame, masked, source_label, armed);
    }
    let frame = sample_frame(
        &source_frame,
        crop,
        (f64::from(crop.w) * scale).max(1.0) as u32,
        (f64::from(crop.h) * scale).max(1.0) as u32,
    );
    surface_preview(&frame, masked, source_label, armed)
}

fn sample_frame(source: &Frame, crop: Rect, width: u32, height: u32) -> Frame {
    let mut out = Frame::new(width, height, source.format);
    for y in 0..height as usize {
        let from_y = crop.y as usize + y * crop.h as usize / height as usize;
        for x in 0..width as usize {
            let from_x = crop.x as usize + x * crop.w as usize / width as usize;
            let from = 4 * (from_y * source.width as usize + from_x);
            let to = 4 * (y * width as usize + x);
            out.pixels[to..to + 4].copy_from_slice(&source.pixels[from..from + 4]);
        }
    }
    out
}

fn mask_geometry_stable(
    before: &[WindowInfo],
    before_targets: &BTreeSet<WindowId>,
    after: &[WindowInfo],
    after_targets: &BTreeSet<WindowId>,
) -> bool {
    let geometry = |windows: &[WindowInfo], targets: &BTreeSet<WindowId>| {
        windows
            .iter()
            .filter(|w| targets.contains(&w.id))
            .map(|w| (w.id, w.bounds))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    geometry(before, before_targets) == geometry(after, after_targets)
}

fn surface_preview(
    frame: &veil_surface::Frame,
    masked: usize,
    source: &str,
    armed: bool,
) -> Result<SurfacePreview, String> {
    let png = encode_png(frame)?;
    Ok(SurfacePreview {
        png_base64: base64::engine::general_purpose::STANDARD.encode(png),
        width: frame.width,
        height: frame.height,
        masked,
        source: source.to_string(),
        armed,
    })
}

fn encode_png(frame: &veil_surface::Frame) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, frame.width, frame.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().map_err(|err| err.to_string())?;
        let mut rgb = Vec::with_capacity(frame.width as usize * frame.height as usize * 3);
        match frame.format {
            veil_surface::PixelFormat::Bgra8 => {
                for px in frame.pixels.chunks_exact(4) {
                    rgb.extend_from_slice(&[px[2], px[1], px[0]]);
                }
            }
            veil_surface::PixelFormat::Rgba8 => {
                for px in frame.pixels.chunks_exact(4) {
                    rgb.extend_from_slice(&[px[0], px[1], px[2]]);
                }
            }
        }
        writer
            .write_image_data(&rgb)
            .map_err(|err| err.to_string())?;
    }
    Ok(out)
}

/// Draw a legible stand-in desktop when no capture is available, so the UI can
/// be exercised and screenshotted on a headless machine.
fn synthetic_desktop(windows: &[veil_core::WindowInfo]) -> veil_surface::Frame {
    let (width, height) = windows.iter().fold((1280u32, 800u32), |(w, h), window| {
        (
            w.max((window.bounds.right().max(0) as u32).saturating_add(40)),
            h.max((window.bounds.bottom().max(0) as u32).saturating_add(40)),
        )
    });
    let mut frame = veil_surface::Frame::filled(
        width,
        height,
        veil_surface::PixelFormat::Bgra8,
        veil_core::Color::rgb(38, 42, 52),
    );
    let colors = [
        veil_core::Color::rgb(70, 90, 130),
        veil_core::Color::rgb(120, 70, 90),
        veil_core::Color::rgb(60, 110, 90),
        veil_core::Color::rgb(110, 100, 60),
    ];
    for (i, window) in windows.iter().enumerate() {
        frame.fill_rect(
            window.bounds,
            FillStyle::Solid {
                color: colors[i % colors.len()],
            },
        );
        // title-bar strip makes the stand-in read as a window, not a block
        frame.fill_rect(
            veil_core::Rect::new(
                window.bounds.x,
                window.bounds.y,
                window.bounds.w,
                window.bounds.h.min(28),
            ),
            FillStyle::Solid {
                color: veil_core::Color::rgb(24, 26, 33),
            },
        );
    }
    frame
}

fn release_all_now(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(mut guard) = state.lock() {
            guard.armed = false;
            guard.paused = true;
            guard.engine.release_all();
            guard.engine.drain_events();
            let pending: Vec<_> = if let Ok(windows) = guard.engine.backend.list_windows() {
                guard
                    .engine
                    .applied()
                    .into_iter()
                    .filter(|applied| RecoveryRecord::needs_restore(applied.mode))
                    .filter_map(|applied| {
                        windows
                            .iter()
                            .find(|w| w.id == applied.window)
                            .map(|w| RecoveryRecord {
                                window: applied.window,
                                pid: w.pid,
                                mode: applied.mode,
                                saved_at_unix: 0,
                            })
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let _ = guard.recovery.save(&pending);
        }
    }
    let _ = app.emit("veil://changed", ());
}

fn arm_now(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(mut guard) = state.lock() {
            guard.armed = true;
            guard.paused = false;
            if let Ok(windows) = guard.engine.backend.list_windows() {
                let reconcile = plan(&guard.engine.profile, &windows, &guard.engine.applied());
                let mut journal_records: Vec<_> = guard
                    .recovery
                    .load()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|record| matches_live(record, &windows))
                    .collect();
                for mode in [ProtectionMode::StrictHide, ProtectionMode::Exclusion] {
                    let intended: Vec<_> = reconcile
                        .apply
                        .iter()
                        .filter(|intent| intent.mode == mode)
                        .filter_map(|intent| {
                            windows.iter().find(|w| w.id == intent.window).cloned()
                        })
                        .collect();
                    if !intended.is_empty() {
                        journal_records.retain(|record| {
                            !intended.iter().any(|window| window.id == record.window)
                        });
                        journal_records.extend(intended_snapshot(&intended, mode));
                    }
                }
                journal_records.sort_by_key(|record| record.window);
                journal_records.dedup_by_key(|record| record.window);
                let _ = guard.recovery.save(&journal_records);
                guard.engine.tick(&windows);
                guard.engine.drain_events();
                guard.journal();
            }
        }
    }
    let _ = app.emit("veil://changed", ());
}

fn toggle_now(app: &tauri::AppHandle) {
    let armed = app
        .try_state::<Mutex<AppState>>()
        .and_then(|state| {
            state.lock().ok().map(|guard| {
                !guard.paused
                    && (guard.armed
                        || !guard.engine.applied().is_empty()
                        || match guard.engine.profile.auto_arm {
                            AutoArm::Always => true,
                            AutoArm::CaptureAppRunning => guard
                                .engine
                                .backend
                                .list_windows()
                                .is_ok_and(|windows| veil_core::capture_app_running(&windows)),
                            AutoArm::Off => false,
                        })
            })
        })
        .unwrap_or(false);
    if armed {
        release_all_now(app);
    } else {
        arm_now(app);
    }
}

/// Background reconciliation so rules follow windows opened after startup and
/// auto-arm reacts when a capture app appears or disappears.
fn start_auto_monitor(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(2000));
        let Some(state) = app.try_state::<Mutex<AppState>>() else {
            continue;
        };
        let mut guard = match state.lock() {
            Ok(guard) => guard,
            Err(_) => continue,
        };
        // Idle default configuration needs no inventory scan at all; the user
        // has not armed anything and no auto-arm policy is active.
        let needs_scan = guard.armed
            || !guard.engine.applied().is_empty()
            || guard.engine.profile.auto_arm != AutoArm::Off;
        if guard.paused || !needs_scan {
            continue;
        }
        let windows = match guard.engine.backend.list_windows() {
            Ok(windows) => windows,
            Err(_) => continue,
        };
        let report = {
            let explicit = guard.armed;
            guard.engine.auto_tick(&windows, explicit)
        };
        let events = guard.engine.drain_events();
        guard.journal();
        if report
            .tick
            .as_ref()
            .is_some_and(|tick| tick.pruned_rules > 0)
        {
            let _ = guard.save();
        }
        drop(guard);
        if report.did_work() || !events.is_empty() {
            let _ = app.emit("veil://changed", ());
        }
    });
}

/// Open (or focus) the clean window the user shares.
///
/// The main window contains controls and is not what an audience should see.
/// This second webview renders the composited frame and offers display selection.
/// It is created lazily so a user who only needs Exclusion
/// never pays for it.
fn open_surface_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("surface") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(app, "surface", tauri::WebviewUrl::App("index.html".into()))
        .title("OmniScreenVeil Surface")
        .inner_size(960.0, 600.0)
        .min_inner_size(480.0, 320.0)
        .background_color(tauri::window::Color(11, 13, 18, 255))
        .build()
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
async fn open_surface(app: tauri::AppHandle) -> Result<(), String> {
    open_surface_window(&app)
}

#[tauri::command]
async fn close_surface(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "surface" {
        return Err("Not the Surface window.".into());
    }
    window.close().map_err(|err| err.to_string())
}

/// Update channel selected by the user. Endpoints are chosen at runtime so
/// stable and beta manifests never leak into each other.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    fn endpoint(self) -> &'static str {
        match self {
            Self::Stable => {
                "https://github.com/j1seki/omniscreenveil/releases/latest/download/latest.json"
            }
            Self::Beta => {
                "https://github.com/j1seki/omniscreenveil/releases/download/updater-beta/latest.json"
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    current_version: String,
    version: String,
    date_unix: Option<i64>,
    notes: Option<String>,
    channel: UpdateChannel,
}

#[derive(Clone, Serialize)]
pub struct UpdateProgress {
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct UpdateCheck {
    update: Option<UpdateInfo>,
    /// False when the channel has no `latest.json` yet (for example a brand
    /// new stable channel). That is different from "no newer version".
    manifest_found: bool,
}

#[derive(Debug, Serialize)]
pub struct InstallOutcome {
    version: String,
}

fn update_builder(
    app: &tauri::AppHandle,
    channel: UpdateChannel,
) -> Result<tauri_plugin_updater::Updater, String> {
    let endpoint: tauri::Url = channel
        .endpoint()
        .parse()
        .map_err(|err| format!("invalid update endpoint: {err}"))?;
    let builder = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|err| err.to_string())?
        .timeout(Duration::from_secs(30));

    // The Windows installer exits the process directly, so the normal
    // RunEvent::Exit cleanup never runs. Release protections before the
    // installer is spawned.
    #[cfg(windows)]
    let builder = {
        let app = app.clone();
        builder.on_before_exit(move || release_all_now(&app))
    };

    builder.build().map_err(|err| err.to_string())
}

async fn check_update_inner(
    app: &tauri::AppHandle,
    channel: UpdateChannel,
) -> Result<(Option<tauri_plugin_updater::Update>, bool), String> {
    let updater = update_builder(app, channel)?;
    match updater.check().await {
        Ok(update) => Ok((update, true)),
        // A 404 on a newly enabled channel only means no manifest is
        // published yet; that is not an error and not "up to date" either.
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => Ok((None, false)),
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
async fn check_update(
    app: tauri::AppHandle,
    channel: UpdateChannel,
) -> Result<UpdateCheck, String> {
    let (update, manifest_found) = check_update_inner(&app, channel).await?;
    Ok(UpdateCheck {
        update: update.map(|update| UpdateInfo {
            current_version: update.current_version.clone(),
            version: update.version.clone(),
            date_unix: update.date.map(|date| date.unix_timestamp()),
            notes: update.body.clone(),
            channel,
        }),
        manifest_found,
    })
}

#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    channel: UpdateChannel,
) -> Result<InstallOutcome, String> {
    let (Some(update), _) = check_update_inner(&app, channel).await? else {
        return Err("No update is available for this channel.".to_string());
    };
    let version = update.version.clone();
    let progress_app = app.clone();
    let mut downloaded = 0u64;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                let _ = progress_app.emit(
                    "veil://update-progress",
                    UpdateProgress {
                        phase: "downloading",
                        downloaded,
                        total,
                    },
                );
            },
            {
                let app = app.clone();
                move || {
                    let _ = app.emit(
                        "veil://update-progress",
                        UpdateProgress {
                            phase: "installing",
                            downloaded: 0,
                            total: None,
                        },
                    );
                }
            },
        )
        .await
        .map_err(|err| err.to_string())?;
    let _ = app.emit(
        "veil://update-progress",
        UpdateProgress {
            phase: "installed",
            downloaded: 0,
            total: None,
        },
    );
    Ok(InstallOutcome { version })
}

/// Restart after a Linux/macOS update. On Windows the installer owns the
/// restart, and the app has already exited.
#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    // `AppHandle::restart` does not always run the RunEvent::Exit handler,
    // depending on which thread calls it. Release first so a protected
    // window is never left hidden or excluded across the restart.
    release_all_now(&app);
    app.restart();
}

pub fn run() {
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };
    let toggle_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_now(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            capabilities,
            capability_matrix,
            inventory,
            mac_permission,
            get_profile,
            add_app_rule,
            add_window_rule,
            protect_all,
            remove_app_rules,
            remove_window_rules,
            remove_rule,
            set_rule_enabled,
            set_rule_mode,
            reset_profile,
            tick,
            set_auto_arm,
            release_all,
            preview,
            surface_displays,
            open_surface,
            close_surface,
            preferences::get_preferences,
            preferences::save_preferences,
            preferences::login_startup,
            check_update,
            install_update,
            restart_app,
        ])
        .setup(move |app| {
            // Resolve the profile location through Tauri so the portable
            // config (`appDirectoriesOverride`) actually keeps data next to
            // the executable. Falling back to the legacy per-user path keeps
            // development builds usable if the resolver is unavailable.
            let store = app
                .path()
                .app_config_dir()
                .map(|dir| dir.join("profile.json"))
                .unwrap_or_else(|err| {
                    eprintln!("veil: could not resolve app config dir: {err}");
                    ConfigStore::default_path()
                });
            let profile = match ConfigStore::new(&store).load() {
                Ok(profile) => profile,
                Err(err) => {
                    eprintln!("veil: could not load profile, starting fresh: {err}");
                    Profile::new()
                }
            };
            app.manage(preferences::PreferenceStore::new(
                store.with_file_name("desktop-settings.json"),
            ));
            let (mut backend, info) = DesktopBackend::detect();
            let recovery =
                RecoveryJournal::new(store.parent().unwrap_or_else(|| std::path::Path::new(".")));
            let recovered = recover_previous_session(&mut backend, &recovery);
            if recovered > 0 {
                eprintln!("veil: restored {recovered} window(s) from previous session");
            }
            let mut recovery_records = recovery.load().unwrap_or_default();
            if let Ok(windows) = backend.list_windows() {
                recovery_records.retain(|record| matches_live(record, &windows));
            } else {
                recovery_records.clear();
            }
            let mut engine = Engine::new(profile, backend);
            if engine.profile.auto_arm != AutoArm::Off {
                if let Ok(windows) = engine.backend.list_windows() {
                    let reconcile = plan(&engine.profile, &windows, &engine.applied());
                    let mut intended: Vec<_> = recovery_records
                        .clone()
                        .into_iter()
                        .filter(|record| matches_live(record, &windows))
                        .collect();
                    for mode in [ProtectionMode::StrictHide, ProtectionMode::Exclusion] {
                        intended.extend(
                            reconcile
                                .apply
                                .iter()
                                .filter(|intent| intent.mode == mode)
                                .filter_map(|intent| windows.iter().find(|w| w.id == intent.window))
                                .map(|w| veil_core::RecoveryRecord {
                                    window: w.id,
                                    pid: w.pid,
                                    mode,
                                    saved_at_unix: 0,
                                }),
                        );
                    }
                    intended.sort_by_key(|record| record.window);
                    intended.dedup_by_key(|record| record.window);
                    let _ = recovery.save(&intended);
                    engine.auto_tick(&windows, false);
                    engine.drain_events();
                    let applied = recovery_snapshot(&engine.applied(), &windows);
                    let applied_ids: std::collections::BTreeSet<_> =
                        applied.iter().map(|record| record.window).collect();
                    let mut records = applied;
                    records.extend(intended.into_iter().filter(|record| {
                        !applied_ids.contains(&record.window)
                            && !engine.profile.rules.iter().any(|rule| {
                                rule.enabled
                                    && RecoveryRecord::needs_restore(rule.mode)
                                    && windows
                                        .iter()
                                        .any(|w| w.id == record.window && rule.matches(w))
                            })
                    }));
                    let armed_ids: std::collections::BTreeSet<_> = records
                        .iter()
                        .filter(|record| {
                            engine.profile.rules.iter().any(|rule| {
                                rule.enabled
                                    && RecoveryRecord::needs_restore(rule.mode)
                                    && windows
                                        .iter()
                                        .any(|w| w.id == record.window && rule.matches(w))
                            })
                        })
                        .map(|record| record.window)
                        .collect();
                    records.retain(|record| armed_ids.contains(&record.window));
                    records.sort_by_key(|record| record.window);
                    records.dedup_by_key(|record| record.window);
                    let _ = recovery.save(&records);
                } else {
                    let _ = recovery.save(&recovery_records);
                }
            } else {
                let _ = recovery.save(&recovery_records);
            }
            app.manage(Mutex::new(AppState {
                engine,
                store: ConfigStore::new(store),
                info,
                armed: false,
                paused: false,
                recovery,
            }));

            let open = MenuItem::with_id(app, "open", "Open OmniScreenVeil", true, None::<&str>)?;
            let surface =
                MenuItem::with_id(app, "surface", "Open Surface window", true, None::<&str>)?;
            let arm = MenuItem::with_id(
                app,
                "arm",
                "Arm / disarm (Ctrl+Shift+V)",
                true,
                None::<&str>,
            )?;
            let release =
                MenuItem::with_id(app, "release", "Release all protection", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &surface, &arm, &release, &quit])?;

            TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .ok_or("application icon is missing")?,
                )
                .tooltip("OmniScreenVeil")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "surface" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(err) = open_surface_window(&app) {
                                eprintln!("veil: could not open surface window: {err}");
                            }
                        });
                    }
                    "arm" => toggle_now(app),
                    "release" => {
                        release_all_now(app);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // A second instance (or another app) may already own the shortcut.
            // That must not abort startup: the tray menu and the UI still work.
            if let Err(err) = app.global_shortcut().register(toggle_shortcut) {
                eprintln!("veil: Ctrl+Shift+V is unavailable ({err}); use the tray or the UI");
            }

            start_auto_monitor(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    if preferences::minimize_on_close(window.app_handle()) {
                        let _ = window.minimize();
                    } else {
                        window.app_handle().exit(0);
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building OmniScreenVeil");

    // Never leave a hidden window, an exclusion property or a moved workspace
    // behind when the app exits (tray quit, Cmd+Q, last window closed).
    app.run(|app_handle, event| {
        if matches!(
            event,
            tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
        ) {
            release_all_now(app_handle);
        }
    });
}
