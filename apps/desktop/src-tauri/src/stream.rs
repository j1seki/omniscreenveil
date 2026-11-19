//! Native streaming uses a persistent DXGI session, with one frame in flight
//! per webview. Only the reduced, masked image is sent over IPC.
#[derive(Default)]
pub struct Sessions {
    #[cfg(windows)]
    inner: std::sync::Mutex<std::collections::HashMap<String, Session>>,
    #[cfg(windows)]
    sequence: std::sync::atomic::AtomicU32,
}

#[cfg(windows)]
struct Session {
    id: u32,
    display: super::Display,
    capture: veil_protect_win::stream::DesktopStream,
    snapshot: Option<(i64, Masks)>,
}

#[cfg(windows)]
#[derive(PartialEq, Eq)]
struct Masks {
    armed: bool,
    targets: Vec<(veil_core::WindowId, veil_core::Rect)>,
    own: Vec<(veil_core::WindowId, veil_core::Rect)>,
}

#[cfg(windows)]
fn masks(
    app: &tauri::AppHandle,
    dashboard: Option<veil_core::WindowId>,
    display: &super::Display,
) -> Result<Masks, String> {
    use tauri::Manager;
    let state = app.state::<std::sync::Mutex<super::AppState>>();
    let state = super::lock(&state)?;
    if let Some(error) = &state.profile_error {
        return Err(error.clone());
    }
    if !super::displays(&state.engine.backend)?.contains(display) {
        return Err("The streaming display changed. Restart the stream.".into());
    }
    let windows = state.engine.backend.list_windows()?;
    let armed = !state.paused && state.engine.should_arm(&windows, state.armed);
    let targets = super::surface_targets(&state.engine, state.armed, state.paused, &windows);
    let targets = windows
        .iter()
        .filter(|w| targets.contains(&w.id) && w.bounds.intersects(&display.bounds))
        .map(|w| (w.id, w.bounds))
        .collect();
    let own = super::own_capture_masks(&state.engine.backend, dashboard)?
        .into_iter()
        .filter(|(_, r)| r.intersects(&display.bounds))
        .collect();
    Ok(Masks {
        armed,
        targets,
        own,
    })
}

#[tauri::command]
pub async fn open_stream(
    window: tauri::WebviewWindow,
    monitor: Option<String>,
) -> Result<u32, String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || {
        use std::sync::atomic::Ordering;
        use tauri::Manager;
        let app = window.app_handle();
        let dashboard = super::dashboard_id(app);
        let state = app.state::<std::sync::Mutex<super::AppState>>();
        let outputs = {
            let state = super::lock(&state)?;
            super::displays(&state.engine.backend)?
        };
        let display = match monitor {
            Some(id) => outputs.into_iter().find(|d| d.id == id),
            None => None,
        }
        .ok_or("Choose an available display for native streaming.")?;
        let sessions = app.state::<Sessions>();
        let mut sessions_map = sessions.inner.lock().map_err(|e| e.to_string())?;
        sessions_map.remove(window.label());
        let since = veil_protect_win::stream::DesktopStream::clock()?;
        let before = masks(app, dashboard, &display)?;
        let capture = veil_protect_win::stream::DesktopStream::open(&display.id)?;
        let id = sessions
            .sequence
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        sessions_map.insert(
            window.label().to_owned(),
            Session {
                id,
                display,
                capture,
                snapshot: Some((since, before)),
            },
        );
        Ok(id)
    })
    .await
    .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    {
        let _ = (window, monitor);
        Err("Native streaming is currently available on Windows only.".into())
    }
}

#[tauri::command]
pub async fn next_stream_frame(
    window: tauri::WebviewWindow,
    id: u32,
) -> Result<tauri::ipc::Response, String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        use veil_protect_win::stream::DesktopStream;
        // Empty = unchanged. One byte = invalidate the displayed frame.
        let invalidate = || Ok(tauri::ipc::Response::new(vec![0u8]));
        let unchanged = || Ok(tauri::ipc::Response::new(Vec::<u8>::new()));
        if window.is_minimized().unwrap_or(true) || !window.is_visible().unwrap_or(false) {
            remove(&window, Some(id));
            return Err("Capture paused while this window is hidden.".into());
        }
        let app = window.app_handle();
        let dashboard = super::dashboard_id(app);
        let sessions = app.state::<Sessions>();
        let mut sessions = sessions.inner.lock().map_err(|e| e.to_string())?;
        let session = sessions
            .get_mut(window.label())
            .filter(|session| session.id == id)
            .ok_or("Stream closed.")?;
        let started = DesktopStream::clock()?;
        let before = masks(app, dashboard, &session.display)?;
        let changed = session
            .snapshot
            .as_ref()
            .map_or(true, |(_, previous)| *previous != before);
        if changed {
            session.snapshot = Some((started, before));
        }
        let (since, before) = session.snapshot.as_ref().unwrap();
        let bounds = session.display.bounds;
        let Some(frame) = session.capture.next(*since, (bounds.w, bounds.h))? else {
            return if changed { invalidate() } else { unchanged() };
        };
        if masks(app, dashboard, &session.display)? != *before {
            session.snapshot = None;
            return invalidate();
        }
        let scale = (960.0 / f64::from(bounds.w.max(bounds.h))).min(1.0);
        let width = (f64::from(bounds.w) * scale).max(1.0) as u32;
        let height = (f64::from(bounds.h) * scale).max(1.0) as u32;
        let mut frame = super::sample_frame(&frame, frame.bounds(), width, height);
        for (_, r) in before.targets.iter().chain(before.own.iter()) {
            let Some(local) = r
                .intersection(&bounds)
                .map(|r| r.translated(-bounds.x, -bounds.y))
            else {
                continue;
            };
            // Round outward: downsampling must never uncover edge pixels.
            let x = (f64::from(local.x) * f64::from(width) / f64::from(bounds.w)).floor() as i32;
            let y = (f64::from(local.y) * f64::from(height) / f64::from(bounds.h)).floor() as i32;
            let right =
                (local.right() as f64 * f64::from(width) / f64::from(bounds.w)).ceil() as i32;
            let bottom =
                (local.bottom() as f64 * f64::from(height) / f64::from(bounds.h)).ceil() as i32;
            frame.fill_rect(
                veil_core::Rect::new(x, y, (right - x) as u32, (bottom - y) as u32),
                veil_core::FillStyle::veiled(),
            );
        }
        drop(sessions);
        super::encode_png(&frame).map(tauri::ipc::Response::new)
    })
    .await
    .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    {
        let _ = (window, id);
        Err("Native streaming is currently available on Windows only.".into())
    }
}

#[cfg(windows)]
fn remove(window: &tauri::WebviewWindow, id: Option<u32>) {
    use tauri::Manager;
    remove_label(window.app_handle(), window.label(), id);
}

#[cfg(windows)]
pub fn remove_label(app: &tauri::AppHandle, label: &str, id: Option<u32>) {
    use tauri::Manager;
    if let Some(state) = app.try_state::<Sessions>() {
        if let Ok(mut sessions) = state.inner.lock() {
            if sessions
                .get(label)
                .is_some_and(|session| id.is_none() || id == Some(session.id))
            {
                sessions.remove(label);
            }
        }
    }
}

#[tauri::command]
pub async fn close_stream(window: tauri::WebviewWindow, id: u32) -> Result<(), String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || remove(&window, Some(id)))
        .await
        .map_err(|e| e.to_string());
    #[cfg(not(windows))]
    {
        let _ = (window, id);
        Ok(())
    }
}
