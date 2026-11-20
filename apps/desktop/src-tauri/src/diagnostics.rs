use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Log, Metadata, Record};
use serde::{Deserialize, Serialize};
use tauri_plugin_clipboard_manager::ClipboardExt;

const CAPACITY: usize = 1_000;
static LOGGER: Diagnostics = Diagnostics(Mutex::new(VecDeque::new()));

struct Diagnostics(Mutex<VecDeque<LogEntry>>);

#[derive(Clone, Serialize)]
pub struct LogEntry {
    timestamp: u64,
    level: String,
    source: String,
    message: String,
}

impl Log for Diagnostics {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target().starts_with("omni_desktop_lib") || metadata.level() <= Level::Warn
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let source = record.target().rsplit("::").next().unwrap_or("desktop");
            let source = if source == "omni_desktop_lib" {
                "desktop"
            } else {
                source
            };
            let message = record.args().to_string();
            if record.level() <= Level::Warn && record.target().starts_with("omni_desktop_lib") {
                eprintln!("veil: {message}");
            }
            push(record.level(), source, &message);
        }
    }

    fn flush(&self) {}
}

fn push(level: Level, source: &str, message: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let entry = LogEntry {
        timestamp,
        level: level.as_str().to_ascii_lowercase(),
        source: source.into(),
        message: message.chars().take(4_000).collect(),
    };
    let mut entries = LOGGER.0.lock().unwrap_or_else(|err| err.into_inner());
    if entries.len() == CAPACITY {
        entries.pop_front();
    }
    entries.push_back(entry);
}

pub fn initialize() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Trace);
    }
}

#[tauri::command]
pub fn get_logs() -> Vec<LogEntry> {
    LOGGER
        .0
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .iter()
        .cloned()
        .collect()
}

#[tauri::command]
pub fn clear_logs() {
    LOGGER
        .0
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clear();
}

#[tauri::command]
pub fn copy_logs(window: tauri::WebviewWindow, text: String) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Copy logs from the dashboard.".into());
    }
    window
        .clipboard()
        .write_text(text)
        .map_err(|err| err.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontendLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[tauri::command]
pub fn record_frontend_log(window: tauri::WebviewWindow, level: FrontendLevel, message: String) {
    let level = match level {
        FrontendLevel::Debug => Level::Debug,
        FrontendLevel::Info => Level::Info,
        FrontendLevel::Warn => Level::Warn,
        FrontendLevel::Error => Level::Error,
    };
    let source = if window.label() == "surface" {
        "stream"
    } else {
        "dashboard"
    };
    push(level, source, &message);
}

#[tauri::command]
pub fn open_devtools(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Open DevTools from the dashboard.".into());
    }
    window.open_devtools();
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProtectionSnapshot {
    armed: bool,
    automation_paused: bool,
    policy: veil_core::AutoArm,
    capture_app_running: bool,
    live: bool,
    configured_rules: usize,
    enabled_rules: usize,
    excluded: usize,
    surface_rules_active: usize,
    hidden: usize,
    failures: Vec<String>,
    awaiting_apply: usize,
    profile_error: Option<String>,
}

impl ProtectionSnapshot {
    pub fn observe(state: &super::AppState, windows: &[veil_core::WindowInfo]) -> Self {
        use veil_core::state::ApplyStatus;
        use veil_core::ProtectionMode;

        let armed = !state.paused && state.engine.should_arm(windows, state.armed);
        let mut snapshot = Self {
            armed,
            automation_paused: state.paused,
            policy: state.engine.profile.auto_arm,
            capture_app_running: veil_core::capture_app_running(windows),
            live: state.info.live,
            configured_rules: state.engine.profile.rules.len(),
            enabled_rules: state
                .engine
                .profile
                .rules
                .iter()
                .filter(|rule| rule.enabled)
                .count(),
            excluded: 0,
            surface_rules_active: 0,
            hidden: 0,
            failures: Vec::new(),
            awaiting_apply: if armed {
                state.engine.plan(windows).apply.len()
            } else {
                0
            },
            profile_error: state.profile_error.clone(),
        };
        for record in state.engine.applied() {
            if let ApplyStatus::Failed { reason } = record.status {
                snapshot.failures.push(format!(
                    "{} · {}: {reason}",
                    record.window,
                    record.mode.label()
                ));
                continue;
            }
            if !matches!(record.status, ApplyStatus::Applied) {
                continue;
            }
            let visible = windows.iter().find(|w| w.id == record.window);
            match record.mode {
                ProtectionMode::Exclusion if snapshot.live && visible.is_some() => {
                    if state.engine.backend.native_exclusion_active(record.window) {
                        snapshot.excluded += 1;
                    } else {
                        snapshot.failures.push(format!(
                            "{} · Windows display affinity is missing or unreadable",
                            record.window
                        ));
                    }
                }
                ProtectionMode::Surface if visible.is_some() => snapshot.surface_rules_active += 1,
                ProtectionMode::StrictHide => {
                    if visible.is_some_and(|window| window.visible && !window.minimized) {
                        snapshot.failures.push(format!(
                            "{} · Strict Hide target is still visible",
                            record.window
                        ));
                    } else {
                        snapshot.hidden += 1;
                    }
                }
                _ => {}
            }
        }
        snapshot
    }

    fn summary(&self) -> String {
        let state = if self.armed {
            "Armed"
        } else if self.failures.is_empty() {
            "Disarmed"
        } else {
            "Disarm incomplete"
        };
        let automation = if self.automation_paused {
            "paused"
        } else {
            "available"
        };
        let backend = if self.live { "live" } else { "simulated" };
        format!("{state} ({backend}) · automation {automation} ({:?}) · {} excluded, {} Surface rules active, {} hidden, {} awaiting apply, {} failures",
            self.policy, self.excluded, self.surface_rules_active, self.hidden, self.awaiting_apply, self.failures.len())
    }
}

static LAST_PROTECTION: Mutex<Option<ProtectionSnapshot>> = Mutex::new(None);

pub fn observe_protection(
    state: &super::AppState,
    windows: &[veil_core::WindowInfo],
) -> ProtectionSnapshot {
    let next = ProtectionSnapshot::observe(state, windows);
    let mut old = LAST_PROTECTION
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    if old.as_ref() != Some(&next) {
        if let Some(error) = &next.profile_error {
            log::error!("Profile verification failed: {error}");
        } else if next.failures.is_empty() {
            log::info!("{}", next.summary());
        } else {
            log::warn!("{}", next.summary());
            for failure in &next.failures {
                log::debug!("{failure}");
            }
        }
        log::debug!(
            "Profile readback: {} enabled of {} saved rules · capture app detected: {}",
            next.enabled_rules,
            next.configured_rules,
            next.capture_app_running
        );
        *old = Some(next.clone());
    }
    next
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub masked: usize,
    pub excluded: usize,
    pub armed: bool,
    pub path: String,
}

static LAST_CAPTURE: Mutex<std::collections::BTreeMap<String, CapturedFrame>> =
    Mutex::new(std::collections::BTreeMap::new());

pub fn forget_capture(label: &str) {
    LAST_CAPTURE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .remove(label);
}

pub fn observe_frame(label: &str, frame: CapturedFrame) {
    let mut captures = LAST_CAPTURE.lock().unwrap_or_else(|err| err.into_inner());
    if captures.get(label) != Some(&frame) {
        log::debug!(
            "Captured {label} frame via {} · {}×{} · armed={} · {} masked, {} excluded",
            frame.path,
            frame.width,
            frame.height,
            frame.armed,
            frame.masked,
            frame.excluded
        );
        captures.insert(label.into(), frame);
    }
}

#[derive(Serialize)]
pub struct DiagnosticState {
    protection: ProtectionSnapshot,
    last_frames: std::collections::BTreeMap<String, CapturedFrame>,
}

#[tauri::command]
pub async fn diagnostic_state(app: tauri::AppHandle) -> Result<DiagnosticState, String> {
    use tauri::Manager;
    tauri::async_runtime::spawn_blocking(move || {
        let handle = app.state::<Mutex<super::AppState>>();
        let state = super::lock(&handle)?;
        let windows = state.engine.backend.list_windows()?;
        let protection = observe_protection(&state, &windows);
        drop(state);
        let last_frames = LAST_CAPTURE.lock().map_err(|err| err.to_string())?.clone();
        Ok(DiagnosticState {
            protection,
            last_frames,
        })
    })
    .await
    .map_err(|err| err.to_string())?
}
