//! OmniScreenVeil desktop app.
//!
//! The UI is a thin view over the same crates the CLI uses: `veil-core` for
//! rules and reconciliation, a platform backend from `veil-protect-*`, the
//! Veil Surface compositor and the canary verifier. No policy lives here.

mod backend;

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, State};
use veil_core::capability::{PlatformCapabilities, ProtectionMode};
use veil_core::ids::{AppKey, RuleId, WindowId};
use veil_core::state::AppliedProtection;
use veil_core::{AutoArm, ConfigStore, FillStyle, Profile, Size};
use veil_protect::{Engine, TickReport};
use veil_verify::{SyntheticCapture, SyntheticDesktop, VerificationHarness, VerificationRun};

use backend::{BackendInfo, DesktopBackend};

pub struct AppState {
    engine: Engine<DesktopBackend>,
    store: ConfigStore,
    info: BackendInfo,
    /// User intent from "Arm now"/"Release all"; auto-arm can override it.
    armed: bool,
}

impl AppState {
    fn save(&self) -> Result<(), String> {
        self.store
            .save(&self.engine.profile)
            .map_err(|err| err.to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    windows: Vec<veil_core::WindowInfo>,
    backend: BackendInfo,
    applied: Vec<AppliedProtection>,
    armed: bool,
    capture_active: bool,
}

#[derive(Debug, Serialize)]
pub struct SurfacePreview {
    png_base64: String,
    width: u32,
    height: u32,
    masked: usize,
    source: String,
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
fn inventory(state: State<'_, Mutex<AppState>>) -> Result<Inventory, String> {
    let state = lock(&state)?;
    let windows = state.engine.backend.list_windows()?;
    let capture_active = veil_core::capture_app_running(&windows);
    let armed = state.engine.should_arm(&windows, state.armed) || !state.engine.applied().is_empty();
    Ok(Inventory {
        windows,
        backend: state.info.clone(),
        applied: state.engine.applied(),
        armed,
        capture_active,
    })
}

#[tauri::command]
fn get_profile(state: State<'_, Mutex<AppState>>) -> Result<Profile, String> {
    Ok(lock(&state)?.engine.profile.clone())
}

#[tauri::command]
fn add_app_rule(
    state: State<'_, Mutex<AppState>>,
    app: AppKey,
    mode: ProtectionMode,
) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state.engine.profile.add_app_rule(app, mode);
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
    state: State<'_, Mutex<AppState>>,
    mode: ProtectionMode,
) -> Result<Profile, String> {
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
    state.engine.tick(&windows);
    state.engine.drain_events();
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn remove_app_rules(
    state: State<'_, Mutex<AppState>>,
    app: AppKey,
) -> Result<Profile, String> {
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
    state.engine.profile.rules.retain(|rule| {
        !matches!(&rule.target, veil_core::TargetRef::Window(id) if *id == window)
    });
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
fn tick(state: State<'_, Mutex<AppState>>) -> Result<TickReport, String> {
    let mut state = lock(&state)?;
    let windows = state.engine.backend.list_windows()?;
    state.armed = true;
    Ok(state.engine.tick(&windows))
}

#[tauri::command]
fn set_auto_arm(state: State<'_, Mutex<AppState>>, auto_arm: AutoArm) -> Result<Profile, String> {
    let mut state = lock(&state)?;
    state.engine.profile.auto_arm = auto_arm;
    if let Ok(windows) = state.engine.backend.list_windows() {
        let explicit = state.armed;
        state.engine.auto_tick(&windows, explicit);
        state.engine.drain_events();
    }
    state.save()?;
    Ok(state.engine.profile.clone())
}

#[tauri::command]
fn release_all(state: State<'_, Mutex<AppState>>) -> Result<ReleaseReport, String> {
    let mut state = lock(&state)?;
    state.armed = false;
    // A release must remain released instead of auto-arming again two seconds later.
    state.engine.profile.auto_arm = AutoArm::Off;
    state.save()?;
    let results = state.engine.release_all();
    state.engine.drain_events();
    let failed = results
        .iter()
        .filter_map(|(window, err)| err.as_ref().map(|err| format!("{window}: {err}")))
        .collect::<Vec<_>>();
    Ok(ReleaseReport {
        released: results.len() - failed.len(),
        failed,
    })
}

/// Render the shareable Veil Surface.
///
/// Uses a real X11 capture when one is available, otherwise a synthetic
/// desktop, and always labels which one in `source`.
#[tauri::command]
fn preview(state: State<'_, Mutex<AppState>>) -> Result<SurfacePreview, String> {
    let state = lock(&state)?;
    let windows = state.engine.backend.list_windows()?;
    // Mask only what the engine has actually applied. Deriving this from the
    // profile would keep painting the veil after "Release all", so the preview
    // would disagree with the applied set and with strict hide.
    let surface: BTreeSet<WindowId> = state
        .engine
        .applied()
        .into_iter()
        .filter(|protection| {
            protection.status.is_applied() && protection.mode == ProtectionMode::Surface
        })
        .map(|protection| protection.window)
        .collect();

    // X11 and macOS exclude our windows inside the capture; Wayland's
    // screencopy sees the whole output, so its own windows are veiled out.
    let own_windows = if state.engine.backend.capture_includes_own_windows() {
        state.engine.backend.own_windows()
    } else {
        Vec::new()
    };
    let (source_frame, source_label) = match state.engine.backend.capture_frame() {
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

    let mut plan =
        veil_surface::SurfacePlan::new(source_frame.size(), FillStyle::veiled());
    let mut masked = 0usize;
    for window in &windows {
        if surface.contains(&window.id) {
            let before = plan.exclusions.len();
            plan = plan.exclude(window.bounds);
            if plan.exclusions.len() > before {
                masked += 1;
            }
        }
    }
    for own in own_windows {
        plan = plan.exclude(own);
    }
    let rendered = veil_surface::render(&source_frame, &plan);
    let png = encode_png(&rendered)?;
    Ok(SurfacePreview {
        png_base64: base64::engine::general_purpose::STANDARD.encode(png),
        width: rendered.width,
        height: rendered.height,
        masked,
        source: source_label.to_string(),
    })
}

/// Headless verification of the Surface guarantee.
///
/// Always records `CapturePath::Synthetic`, because that is the truth: the
/// canary is composited and re-captured in-process. Live capture paths are
/// verified per platform by the CLI harness.
#[tauri::command]
fn verify() -> Result<VerificationRun, String> {
    let desktop = SyntheticDesktop::new(Size::new(960, 540));
    let plan = desktop.protected_plan();
    let mut harness = VerificationHarness::new(
        SyntheticCapture::new(&desktop, None),
        desktop.token(),
        desktop.cell(),
    )
    .with_region(desktop.canary_rect());
    harness
        .run_with(|source| {
            source.set_plan(Some(plan.clone()));
            Ok(())
        })
        .map_err(|err| err.to_string())
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
            guard.engine.release_all();
            guard.engine.drain_events();
            let _ = guard.save();
        }
    }
    let _ = app.emit("veil://changed", ());
}

fn arm_now(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<Mutex<AppState>>() {
        if let Ok(mut guard) = state.lock() {
            guard.armed = true;
            if let Ok(windows) = guard.engine.backend.list_windows() {
                guard.engine.tick(&windows);
                guard.engine.drain_events();
            }
        }
    }
    let _ = app.emit("veil://changed", ());
}

fn toggle_now(app: &tauri::AppHandle) {
    let armed = app
        .try_state::<Mutex<AppState>>()
        .and_then(|state| {
            state
                .lock()
                .ok()
                .map(|guard| !guard.engine.applied().is_empty())
        })
        .unwrap_or(false);
    if armed {
        if let Some(state) = app.try_state::<Mutex<AppState>>() {
            if let Ok(mut guard) = state.lock() {
                guard.engine.profile.auto_arm = AutoArm::Off;
            }
        }
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
        if !needs_scan {
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
        if report.tick.as_ref().is_some_and(|tick| tick.pruned_rules > 0) {
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
/// This second webview renders only the composited frame, polls `preview`, and
/// carries no chrome. It is created lazily so a user who only needs Exclusion
/// never pays for it.
fn open_surface_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("surface") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        app,
        "surface",
        tauri::WebviewUrl::App("index.html?surface=1".into()),
    )
    .title("OmniScreenVeil Surface")
    .inner_size(960.0, 600.0)
    .min_inner_size(480.0, 320.0)
    .background_color(tauri::window::Color(11, 13, 18, 255))
    .build()
    .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_surface(app: tauri::AppHandle) -> Result<(), String> {
    open_surface_window(&app)
}

pub fn run() {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
    let toggle_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);

    let app = tauri::Builder::default()
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
            verify,
            open_surface,
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
            let (backend, info) = DesktopBackend::detect();
            let mut engine = Engine::new(profile, backend);
            if engine.profile.auto_arm != AutoArm::Off {
                if let Ok(windows) = engine.backend.list_windows() {
                    engine.auto_tick(&windows, false);
                    engine.drain_events();
                }
            }
            app.manage(Mutex::new(AppState {
                engine,
                store: ConfigStore::new(store),
                info,
                armed: false,
            }));

            let open = MenuItem::with_id(app, "open", "Open OmniScreenVeil", true, None::<&str>)?;
            let surface =
                MenuItem::with_id(app, "surface", "Open Surface window", true, None::<&str>)?;
            let arm = MenuItem::with_id(app, "arm", "Arm / disarm (Ctrl+Shift+V)", true, None::<&str>)?;
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
                            let _ = window.set_focus();
                        }
                    }
                    "surface" => {
                        if let Err(err) = open_surface_window(app) {
                            eprintln!("veil: could not open surface window: {err}");
                        }
                    }
                    "arm" => toggle_now(app),
                    "release" => release_all_now(app),
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
