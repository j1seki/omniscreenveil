use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use veil_core::{Profile, ProtectionMode};

#[cfg(windows)]
pub(super) mod windows;

pub const LABEL: &str = "surface";
pub const STATUS_EVENT: &str = "veil://stream-status";
pub const DASHBOARD_TITLE: &str = "OmniScreenVeil Dashboard";
pub const STREAM_TITLE: &str = "OmniScreenVeil Stream";

pub fn window_title(title: &str) -> String {
    match option_env!("OMNISCREENVEIL_TEST_BUILD") {
        Some(label) => format!("{title} [{label}]"),
        None => title.into(),
    }
}

#[derive(Default)]
pub struct Sharing {
    enabled: AtomicBool,
    policy: Mutex<StreamPolicy>,
    transition: tauri::async_runtime::Mutex<()>,
}

#[derive(Clone, Copy, Default)]
enum StreamIntent {
    #[default]
    Automatic,
    ManualOn,
    HoldOff,
}

#[derive(Default)]
struct StreamPolicy {
    eligible: bool,
    intent: StreamIntent,
}

impl StreamPolicy {
    fn observe(&mut self, eligible: bool) -> bool {
        let changed = eligible != self.eligible;
        self.eligible = eligible;
        if !eligible && matches!(self.intent, StreamIntent::HoldOff) {
            self.intent = StreamIntent::Automatic;
        }
        changed
    }

    fn desired(&self) -> bool {
        match self.intent {
            StreamIntent::Automatic => self.eligible,
            StreamIntent::ManualOn => true,
            StreamIntent::HoldOff => false,
        }
    }

    fn set_manual(&mut self, enabled: bool) {
        self.intent = if enabled {
            StreamIntent::ManualOn
        } else if self.eligible {
            StreamIntent::HoldOff
        } else {
            StreamIntent::Automatic
        };
    }
}

impl Sharing {
    fn policy(&self) -> std::sync::MutexGuard<'_, StreamPolicy> {
        self.policy.lock().unwrap_or_else(|err| err.into_inner())
    }

    pub fn validate_capture(&self, armed: bool, profile: &Profile) -> Result<(), String> {
        if !self.enabled.load(Ordering::Acquire) {
            return Err("Stream is off.".into());
        }
        let policy = self.policy();
        match policy.intent {
            StreamIntent::ManualOn => Ok(()),
            StreamIntent::Automatic
                if policy.eligible && armed && surface_rule_count(profile) > 0 =>
            {
                Ok(())
            }
            _ => Err(
                "Automatic Stream requires armed protection and an enabled Surface rule.".into(),
            ),
        }
    }
}

fn surface_rule_count(profile: &Profile) -> usize {
    profile
        .rules
        .iter()
        .filter(|rule| rule.enabled && rule.mode == ProtectionMode::Surface)
        .count()
}

/// Called alongside protection reconciliation, including when a capture app's
/// arrival or departure changes arming without changing any applied windows.
pub fn observe_protection(app: &AppHandle, state: &super::AppState, armed: bool) {
    if let Some(sharing) = app.try_state::<Sharing>() {
        let eligible = armed
            && state.info.live
            && state.profile_error.is_none()
            && surface_rule_count(&state.engine.profile) > 0;
        let mut policy = sharing.policy();
        if policy.observe(eligible) {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let sharing = app.state::<Sharing>();
                let _transition = sharing.transition.lock().await;
                let desired = sharing.policy().desired();
                if let Err(err) = apply_output(&app, desired).await {
                    log::warn!("Could not update automatic Stream output: {err}");
                }
            });
        }
    }
}

pub fn active(app: &AppHandle) -> bool {
    app.state::<Sharing>().enabled.load(Ordering::Acquire)
}

#[tauri::command]
pub fn stream_enabled(app: AppHandle) -> bool {
    active(&app) && app.get_webview_window(LABEL).is_some()
}

#[tauri::command]
pub async fn set_stream_enabled(app: AppHandle, enabled: bool) -> Result<StreamStatus, String> {
    let sharing = app.state::<Sharing>();
    let _transition = sharing.transition.lock().await;
    let protection = if enabled {
        let handle = app.clone();
        Some(
            tauri::async_runtime::spawn_blocking(move || {
                let backend = handle.state::<Mutex<super::AppState>>();
                let state = super::lock(&backend)?;
                let armed = if !state.paused
                    && !state.armed
                    && state.engine.profile.auto_arm == veil_core::AutoArm::CaptureAppRunning
                {
                    let windows = state.engine.backend.list_windows()?;
                    state.engine.should_arm(&windows, false)
                } else {
                    !state.paused && state.engine.should_arm(&[], state.armed)
                };
                observe_protection(&handle, &state, armed);
                Ok::<_, String>(StreamProtection {
                    armed,
                    surface_rules: surface_rule_count(&state.engine.profile),
                })
            })
            .await
            .map_err(|err| err.to_string())??,
        )
    } else {
        None
    };
    let previous = {
        let mut policy = sharing.policy();
        let previous = policy.intent;
        policy.set_manual(enabled);
        previous
    };
    if let Err(err) = apply_output(&app, enabled).await {
        sharing.policy().intent = previous;
        return Err(err);
    }
    Ok(StreamStatus {
        enabled,
        protection,
    })
}

#[derive(Serialize)]
pub struct StreamStatus {
    enabled: bool,
    protection: Option<StreamProtection>,
}

#[derive(Serialize)]
struct StreamProtection {
    armed: bool,
    surface_rules: usize,
}

async fn apply_output(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let sharing = app.state::<Sharing>();
    if enabled {
        if stream_enabled(app.clone()) {
            return Ok(());
        }
        if app.get_webview_window(LABEL).is_some() {
            return Err("The Stream window is still closing. Try again in a moment.".into());
        }
        {
            let backend = app.state::<Mutex<super::AppState>>();
            let state = super::lock(&backend)?;
            if !state.info.live {
                return Err("Stream requires a live desktop backend.".into());
            }
            if let Some(error) = &state.profile_error {
                return Err(error.clone());
            }
        }
        sharing.enabled.store(true, Ordering::Release);
        let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("surface.html".into()))
            .title(window_title(STREAM_TITLE))
            .inner_size(960.0, 600.0)
            .min_inner_size(480.0, 320.0)
            .center()
            .focused(false)
            .minimizable(false)
            .always_on_bottom(true)
            .background_color(tauri::window::Color(11, 13, 18, 255));
        #[cfg(windows)]
        let builder = builder.visible(false).minimizable(true);
        let window = builder.build().map_err(|err| {
            sharing.enabled.store(false, Ordering::Release);
            err.to_string()
        })?;
        #[cfg(windows)]
        if let Err(err) = windows::show_background(&window).await {
            sharing.enabled.store(false, Ordering::Release);
            app.state::<super::FeedbackAreas>()
                .stream_window
                .store(0, Ordering::Release);
            let _ = window.destroy();
            return Err(err);
        }
        #[cfg(not(windows))]
        let _ = window;
        log::info!("Stream output enabled in the background");
    } else if active(app) || app.get_webview_window(LABEL).is_some() {
        sharing.enabled.store(false, Ordering::Release);
        if let Some(window) = app.get_webview_window(LABEL) {
            if let Err(err) = window.destroy() {
                sharing.enabled.store(true, Ordering::Release);
                return Err(err.to_string());
            }

            // Finish destroying the capture target before another start can
            // reuse its label. Never block the desktop event loop while waiting.
            let handle = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let deadline = Instant::now() + Duration::from_secs(2);
                while handle.get_webview_window(LABEL).is_some() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(15));
                }
                #[cfg(windows)]
                super::stream::remove_label(&handle, LABEL, None);
            })
            .await
            .map_err(|err| err.to_string())?;
        }
        #[cfg(windows)]
        app.state::<super::FeedbackAreas>()
            .stream_window
            .store(0, Ordering::Release);
        super::diagnostics::forget_capture(LABEL);
        log::info!("Stream output disabled");
    } else {
        return Ok(());
    }
    let _ = app.emit(STATUS_EVENT, ());
    let _ = app.emit("veil://windows", ());
    Ok(())
}

#[tauri::command]
pub async fn open_surface(app: AppHandle) -> Result<(), String> {
    let sharing = app.state::<Sharing>();
    let _transition = sharing.transition.lock().await;
    if !active(&app) {
        return Err("Turn on Stream to view the output.".into());
    }
    let window = app
        .get_webview_window(LABEL)
        .ok_or("Stream output is unavailable.")?;
    // From this point Tauri tracks the viewer as shown, so later window
    // style changes preserve its visibility after the Win32 background show.
    window.show().map_err(|err| err.to_string())?;
    window
        .set_always_on_bottom(false)
        .map_err(|err| err.to_string())?;
    window.unminimize().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())
}

pub fn park(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(windows)]
    return windows::park(window);
    #[cfg(not(windows))]
    {
        window
            .set_always_on_bottom(true)
            .map_err(|err| err.to_string())?;
        if let Some(dashboard) = window.app_handle().get_webview_window("main") {
            if dashboard.is_visible().unwrap_or(false) && !dashboard.is_minimized().unwrap_or(true)
            {
                let _ = dashboard.set_focus();
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn close_surface(window: WebviewWindow) -> Result<(), String> {
    if window.label() != LABEL {
        return Err("Open Stream to manage its viewer.".into());
    }
    park(&window)
}

#[cfg(test)]
mod tests {
    use super::StreamPolicy;

    #[test]
    fn manual_stop_holds_until_automatic_eligibility_resets() {
        let mut policy = StreamPolicy::default();
        assert!(!policy.desired());
        assert!(policy.observe(true));
        assert!(policy.desired());

        policy.set_manual(false);
        assert!(!policy.observe(true));
        assert!(!policy.desired());
        assert!(policy.observe(false));
        assert!(!policy.desired());
        assert!(policy.observe(true));
        assert!(policy.desired());
    }

    #[test]
    fn manual_start_stays_on_until_manually_stopped() {
        let mut policy = StreamPolicy::default();
        policy.set_manual(true);
        assert!(policy.desired());
        policy.observe(true);
        policy.observe(false);
        assert!(policy.desired());
        policy.set_manual(false);
        assert!(!policy.desired());
    }
}
