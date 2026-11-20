use std::time::{Duration, Instant};

use tauri::{
    webview::PageLoadEvent, AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use veil_canary::{detect_in, Token};
use veil_core::{state::ProtectionIntent, ProtectionMode, Rect, RuleId, WindowId, WindowInfo};
use veil_protect::ProtectionBackend;
use veil_protect_win::{stream::DesktopStream, WinBackend};

use super::{blocked, result, verify, Check};

const PROBES: [(&str, &str); 6] = [
    ("exclusion", "Windows Exclusion and restore"),
    ("strict-hide", "Windows Strict Hide and restore"),
    ("dxgi", "Windows Native stream"),
    ("dashboard-feedback", "Dashboard feedback region"),
    ("stream-inventory", "Separate Stream window"),
    ("stream-minimize", "Stream stays available after minimize"),
];

pub async fn run(app: &AppHandle) -> Vec<Check> {
    let token = Token::fresh();
    let colors: Vec<_> = veil_canary::cells(token)
        .iter()
        .map(|color| format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b))
        .collect();
    let script = format!(
        "paintProbe({})",
        serde_json::to_string(&colors).unwrap_or_default()
    );
    let window = match WebviewWindowBuilder::new(
        app,
        "verification-probe",
        WebviewUrl::App("verification-probe.html".into()),
    )
    .title("OmniScreenVeil verification target")
    .inner_size(216.0, 204.0)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .center()
    .on_page_load(move |webview, payload| {
        if payload.event() == PageLoadEvent::Finished {
            if let Err(err) = webview.eval(&script) {
                log::warn!("Could not paint the verification target: {err}");
            }
        }
    })
    .build()
    {
        Ok(window) => window,
        Err(error) => return blocked_checks(error.to_string()),
    };
    let probe = window.clone();
    let handle = app.clone();
    let checks = tauri::async_runtime::spawn_blocking(move || checks(&handle, &probe, token)).await;
    let checks = match checks {
        Ok(Ok(mut checks)) => {
            checks.push(result(
                "stream-minimize",
                "Windows",
                "Stream stays available after minimize",
                super::super::sharing::windows::verify_minimize(&window).await,
            ));
            checks
        }
        Ok(Err(error)) => blocked_checks(error),
        Err(error) => blocked_checks(error.to_string()),
    };
    let _ = window.close();
    checks
}

fn blocked_checks(reason: String) -> Vec<Check> {
    PROBES
        .iter()
        .map(|(id, label)| blocked(id, "Windows", label, reason.clone()))
        .collect()
}

fn await_marker(
    backend: &WinBackend,
    bounds: Rect,
    token: Token,
    cell: u32,
    expected: bool,
) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let frame = backend.capture_root().map_err(|err| err.to_string())?;
        if detect_in(&frame, token, cell, bounds).found == expected {
            return Ok(());
        }
        if Instant::now() >= until {
            let expectation = if expected { "visible" } else { "absent" };
            return Err(format!(
                "The pixel marker did not become {expectation} in the Windows desktop capture."
            ));
        }
        std::thread::sleep(Duration::from_millis(55));
    }
}

fn protection_check(
    backend: &mut WinBackend,
    window: &WindowInfo,
    token: Token,
    cell: u32,
    mode: ProtectionMode,
) -> Result<String, String> {
    let intent = ProtectionIntent {
        window: window.id,
        app: window.app.clone(),
        app_name: window.app_name.clone(),
        pid: window.pid,
        bounds: window.bounds,
        mode,
        rule: RuleId(1),
    };
    let applied = backend
        .apply(&intent)
        .map_err(|err| err.to_string())
        .and_then(|()| {
            match mode {
                ProtectionMode::Exclusion if !backend.exclusion_active(window.id) => {
                    return Err("Windows did not report capture-exclusion display affinity.".into());
                }
                ProtectionMode::StrictHide
                    if backend
                        .owned_window_info(window.id)
                        .is_some_and(|target| target.visible) =>
                {
                    return Err("Windows still reported the test window as visible.".into());
                }
                _ => {}
            }
            await_marker(backend, window.bounds, token, cell, false)
        });
    let released = backend
        .release(window.id)
        .map_err(|err| err.to_string())
        .and_then(|()| await_marker(backend, window.bounds, token, cell, true));
    released.map_err(|err| format!("Could not verify restoration of the test window: {err}"))?;
    applied?;
    Ok("Read back the native protection state. The pixel marker disappeared and returned after release.".into())
}

fn dxgi_check(
    app: &AppHandle,
    probe: &WebviewWindow,
    bounds: Rect,
    token: Token,
    cell: u32,
) -> Result<String, String> {
    let state = app.state::<std::sync::Mutex<super::super::AppState>>();
    let display = super::super::displays(&super::super::lock(&state)?.engine.backend)?
        .into_iter()
        .find(|display| bounds.intersects(&display.bounds))
        .ok_or("Could not find the test window's display.")?;
    let mut stream = DesktopStream::open(&display.id)?;
    let since = DesktopStream::clock()?;
    let until = Instant::now() + Duration::from_secs(2);
    let local = bounds.translated(-display.bounds.x, -display.bounds.y);
    let mut pulse = false;
    while Instant::now() < until {
        pulse = !pulse;
        probe
            .eval(format!("pulseProbe({pulse})"))
            .map_err(|err| err.to_string())?;
        if let Some(frame) = stream.next(since, (display.bounds.w, display.bounds.h))? {
            if detect_in(&frame, token, cell, local).found {
                return verify(
                    frame.width == display.bounds.w && frame.height == display.bounds.h,
                    format!(
                        "DXGI provided a fresh {}×{} frame containing the baseline marker from {}.",
                        frame.width, frame.height, display.name
                    ),
                    "The DXGI frame did not match the selected display dimensions.",
                );
            }
        }
    }
    Err("DXGI did not return a fresh frame containing the baseline marker.".into())
}

fn feedback_check(
    backend: &mut WinBackend,
    window: &WindowInfo,
    token: Token,
    cell: u32,
) -> Result<String, String> {
    backend.register_dashboard(window.id, "Verification dashboard".into());
    let areas = [Rect::new(0, 0, 8, 8)];
    let masks: Vec<_> = backend
        .capture_masks(Some(&areas))
        .into_iter()
        .filter(|(id, _)| *id == window.id)
        .map(|(_, rect)| rect)
        .collect();
    let [area] = masks.as_slice() else {
        return Err("The dashboard did not produce one feedback region.".into());
    };
    let mut frame = backend.capture_root().map_err(|err| err.to_string())?;
    frame.fill_rect(*area, super::super::FEEDBACK_FILL);
    verify(area.w == 8 && area.h == 8
        && area.intersection(&window.bounds).as_ref() == Some(area)
        && backend.capture_masks(Some(&[])).iter().all(|(id, _)| *id != window.id)
        && detect_in(&frame, token, cell, window.bounds).found,
        "Only the reported client area was covered. The rest of the test dashboard stayed visible, and an empty report left it uncovered.",
        "The feedback mask covered the whole dashboard or landed outside its client area.")
}

fn stream_inventory_check(
    app: &AppHandle,
    backend: &mut WinBackend,
    id: WindowId,
) -> Result<String, String> {
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    };

    let dashboard =
        super::super::dashboard_id(app).ok_or("The dashboard window is unavailable.")?;
    let output = Arc::new(AtomicU64::new(id.0));
    backend.register_dashboard(dashboard, "Verification dashboard".into());
    backend.track_stream_window(output.clone(), "Verification Stream".into());
    let windows = backend.inventory(false).map_err(|err| err.to_string())?;
    let has_separate_entries = dashboard != id
        && windows
            .iter()
            .filter(|window| window.id == dashboard && window.title == "Verification dashboard")
            .count()
            == 1
        && windows
            .iter()
            .filter(|window| window.id == id && window.title == "Verification Stream")
            .count()
            == 1;
    let no_implicit_mask = backend
        .capture_masks(Some(&[]))
        .iter()
        .all(|(window, _)| *window != id);
    output.store(0, Ordering::Release);
    let removed = backend
        .inventory(false)
        .map_err(|err| err.to_string())?
        .iter()
        .all(|window| window.id != id);
    verify(has_separate_entries && no_implicit_mask && removed,
        "The dashboard and Stream had separate handles and titles. Stream had no implicit mask and left the inventory when unregistered.",
        "The Stream inventory entry or mask did not match its registered window.")
}

fn checks(app: &AppHandle, probe: &WebviewWindow, token: Token) -> Result<Vec<Check>, String> {
    let mut backend = WinBackend::new().map_err(|err| err.to_string())?;
    let id = WindowId(probe.hwnd().map_err(|err| err.to_string())?.0 as usize as u64);
    let window = backend
        .owned_window_info(id)
        .ok_or("The test window is unavailable.")?;
    let cell = (16.0 * probe.scale_factor().map_err(|err| err.to_string())?).round() as u32;
    await_marker(&backend, window.bounds, token, cell, true)
        .map_err(|err| format!("Baseline unavailable. Keep the test window visible. {err}"))?;
    let dxgi = result(
        "dxgi",
        "Windows",
        "Windows Native stream",
        dxgi_check(app, probe, window.bounds, token, cell),
    );
    let exclude = result(
        "exclusion",
        "Windows",
        "Windows Exclusion and restore",
        protection_check(
            &mut backend,
            &window,
            token,
            cell,
            ProtectionMode::Exclusion,
        ),
    );
    let hide = result(
        "strict-hide",
        "Windows",
        "Windows Strict Hide and restore",
        protection_check(
            &mut backend,
            &window,
            token,
            cell,
            ProtectionMode::StrictHide,
        ),
    );
    let feedback = result(
        "dashboard-feedback",
        "Windows",
        "Dashboard feedback region",
        feedback_check(&mut backend, &window, token, cell),
    );
    let inventory = result(
        "stream-inventory",
        "Windows",
        "Separate Stream window",
        stream_inventory_check(app, &mut backend, id),
    );
    Ok(vec![exclude, hide, dxgi, feedback, inventory])
}
