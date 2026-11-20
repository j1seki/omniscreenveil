//! Native streaming uses a persistent DXGI session, with one frame in flight
//! per webview. Only the reduced, masked image is sent over IPC.
pub fn frame_response(
    armed: bool,
    width: u32,
    height: u32,
    masked: usize,
    excluded: usize,
    mut png: Vec<u8>,
) -> tauri::ipc::Response {
    let mut header = [0; 17];
    header[0] = u8::from(armed);
    for (field, value) in
        header[1..]
            .chunks_exact_mut(4)
            .zip([width, height, masked as u32, excluded as u32])
    {
        field.copy_from_slice(&value.to_le_bytes());
    }
    drop(png.splice(..0, header));
    tauri::ipc::Response::new(png)
}
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
    max_edge: u32,
    capture: veil_protect_win::stream::DesktopStream,
    snapshot: Option<(i64, Masks)>,
}

#[cfg(windows)]
#[derive(PartialEq, Eq)]
struct Masks {
    armed: bool,
    excluded: std::collections::BTreeSet<veil_core::WindowId>,
    targets: Vec<(veil_core::WindowId, veil_core::Rect)>,
    own: Vec<(veil_core::WindowId, veil_core::Rect)>,
}

#[cfg(windows)]
impl Masks {
    fn same_protection(&self, other: &Self) -> bool {
        self.armed == other.armed
            && self.excluded == other.excluded
            && self.targets == other.targets
    }
}

#[cfg(windows)]
fn masks(app: &tauri::AppHandle, display: &super::Display) -> Result<Masks, String> {
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
    app.state::<super::sharing::Sharing>()
        .validate_capture(armed, &state.engine.profile)?;
    let protection =
        super::stream_protection(&state.engine, state.armed, state.paused, &windows, |id| {
            state.engine.backend.native_exclusion_active(id)
        });
    let excluded = windows
        .iter()
        .filter(|w| protection.excluded.contains(&w.id) && w.bounds.intersects(&display.bounds))
        .map(|w| w.id)
        .collect();
    let targets = windows
        .iter()
        .filter(|w| protection.masks.contains(&w.id) && w.bounds.intersects(&display.bounds))
        .map(|w| (w.id, w.bounds))
        .collect();
    let own =
        super::own_capture_masks(&state.engine.backend, &app.state::<super::FeedbackAreas>())?
            .into_iter()
            .filter(|(_, r)| r.intersects(&display.bounds))
            .collect();
    Ok(Masks {
        armed,
        excluded,
        targets,
        own,
    })
}

#[tauri::command]
pub async fn open_stream(
    window: tauri::WebviewWindow,
    monitor: Option<String>,
    max_edge: u32,
) -> Result<u32, String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || {
        use std::sync::atomic::Ordering;
        use tauri::Manager;
        let app = window.app_handle();
        if window.label() != super::sharing::LABEL || !super::sharing::active(app) {
            return Err("Native capture requires an enabled Stream output.".into());
        }
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
        if !super::sharing::active(app) {
            return Err("Stream is off.".into());
        }
        sessions_map.remove(window.label());
        let since = veil_protect_win::stream::DesktopStream::clock()?;
        let before = masks(app, &display)?;
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
                max_edge: super::preferences::clamp_stream_edge(max_edge),
                capture,
                snapshot: Some((since, before)),
            },
        );
        log::trace!(
            "Started DXGI session {id} from the {} window",
            window.label()
        );
        Ok(id)
    })
    .await
    .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    {
        let _ = (window, monitor, max_edge);
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
        // Empty = unchanged. Commands [0] and [1] request a new snapshot.
        // Only [0] first clears the last frame, for protection changes.
        let invalidate = || Ok(tauri::ipc::Response::new(vec![0u8]));
        let refresh = || Ok(tauri::ipc::Response::new(vec![1u8]));
        let unchanged = || Ok(tauri::ipc::Response::new(Vec::<u8>::new()));
        if window.is_minimized().unwrap_or(true) || !window.is_visible().unwrap_or(false) {
            remove(&window, Some(id));
            return Err("Capture paused while this window is hidden.".into());
        }
        let app = window.app_handle();
        if !super::sharing::active(app) {
            return Err("Stream is off.".into());
        }
        let sessions = app.state::<Sessions>();
        let mut sessions = sessions.inner.lock().map_err(|e| e.to_string())?;
        let session = sessions
            .get_mut(window.label())
            .filter(|session| session.id == id)
            .ok_or("Stream closed.")?;
        let started = DesktopStream::clock()?;
        let before = masks(app, &session.display)?;
        let protection_changed = session
            .snapshot
            .as_ref()
            .map_or(true, |(_, previous)| !previous.same_protection(&before));
        let changed = session
            .snapshot
            .as_ref()
            .map_or(true, |(_, previous)| *previous != before);
        if changed {
            session.snapshot = Some((started, before));
        }
        let (since, before) = session.snapshot.as_ref().unwrap();
        let bounds = session.display.bounds;
        let size = super::stream_dimensions(bounds, session.max_edge);
        let Some(mut frame) = session
            .capture
            .next_scaled(*since, (bounds.w, bounds.h), size)?
        else {
            return if protection_changed {
                invalidate()
            } else if changed {
                // The desktop may have presented the window move before
                // the new mask was sampled. A snapshot can read it now.
                refresh()
            } else {
                unchanged()
            };
        };
        let latest = masks(app, &session.display)?;
        if latest != *before {
            let invalidate_frame = protection_changed || !latest.same_protection(before);
            session.snapshot.as_mut().unwrap().0 = DesktopStream::clock()?;
            return if invalidate_frame {
                invalidate()
            } else {
                refresh()
            };
        }
        let (width, height) = size;
        for ((_, r), style) in before
            .own
            .iter()
            .map(|mask| (mask, super::FEEDBACK_FILL))
            .chain(
                before
                    .targets
                    .iter()
                    .map(|mask| (mask, veil_core::FillStyle::veiled())),
            )
        {
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
                style,
            );
        }
        let observed = super::diagnostics::CapturedFrame {
            width,
            height,
            masked: before.targets.len(),
            excluded: before.excluded.len(),
            armed: before.armed,
            path: "Windows DXGI Desktop Duplication".into(),
        };
        drop(sessions);
        let response = frame_response(
            observed.armed,
            width,
            height,
            observed.masked,
            observed.excluded,
            super::encode_png(&frame)?,
        );
        if !super::sharing::active(app) {
            return Err("Stream is off.".into());
        }
        super::diagnostics::observe_frame(window.label(), observed);
        Ok(response)
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
                log::trace!("Closed DXGI session for the {label} window");
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
