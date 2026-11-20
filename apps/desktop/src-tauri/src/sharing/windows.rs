use std::sync::atomic::Ordering;

use tauri::{async_runtime::channel, Manager, WebviewWindow};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsIconic, IsWindowVisible, PostMessageW, SetForegroundWindow,
    SetWindowPos, ShowWindow, HWND_BOTTOM, SC_MINIMIZE, SIZE_MINIMIZED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, WM_APP, WM_NCDESTROY, WM_SIZE, WM_SYSCOMMAND,
};

const SUBCLASS_ID: usize = 1;
const RESTORE_IN_BACKGROUND: u32 = WM_APP + 0x317;

fn native_handles(window: &WebviewWindow) -> Result<(usize, usize), String> {
    let hwnd = window.hwnd().map_err(|err| err.to_string())?.0 as usize;
    let dashboard = super::super::dashboard_id(window.app_handle()).map_or(0, |id| id.0 as usize);
    Ok((hwnd, dashboard))
}

fn park_native(hwnd: HWND, dashboard: HWND) -> Result<(), String> {
    // Screen-share pickers expect a shown, unminimized window. Lower the same
    // HWND so a running share keeps its source and receives fresh frames.
    unsafe {
        let was_focused = GetForegroundWindow() == hwnd;
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        if SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        ) == 0
        {
            return Err(format!(
                "Could not put Stream in the background: {}",
                std::io::Error::last_os_error()
            ));
        }
        if IsIconic(hwnd) != 0 {
            return Err("Windows kept the Stream window minimized.".into());
        }
        if was_focused && IsWindowVisible(dashboard) != 0 && IsIconic(dashboard) == 0 {
            SetForegroundWindow(dashboard);
        }
    }
    Ok(())
}

unsafe extern "system" fn stream_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    dashboard: usize,
) -> LRESULT {
    match message {
        WM_SYSCOMMAND if wparam & 0xfff0 == SC_MINIMIZE as usize => {
            if let Err(err) = park_native(hwnd, dashboard as HWND) {
                log::warn!("{err}");
            } else {
                log::debug!("Sent the Stream window to the background on minimize");
                return 0;
            }
        }
        WM_SIZE if wparam == SIZE_MINIMIZED as usize => {
            // Windows can minimize through ShowWindow without a system command.
            // Restore only after that transition completes to avoid reentrancy.
            PostMessageW(hwnd, RESTORE_IN_BACKGROUND, 0, 0);
        }
        RESTORE_IN_BACKGROUND => {
            if IsIconic(hwnd) != 0 {
                if let Err(err) = park_native(hwnd, dashboard as HWND) {
                    log::warn!("{err}");
                }
            }
            return 0;
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(stream_window_proc), id);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

pub async fn show_background(window: &WebviewWindow) -> Result<(), String> {
    let (hwnd, dashboard) = native_handles(window)?;
    window
        .app_handle()
        .state::<super::super::FeedbackAreas>()
        .stream_window
        .store(hwnd as u64, Ordering::Release);
    let (send, mut receive) = channel(1);
    window
        .run_on_main_thread(move || {
            let result = handle_minimize(hwnd as HWND, dashboard)
                .and_then(|()| park_native(hwnd as HWND, dashboard as HWND));
            let _ = send.try_send(result);
        })
        .map_err(|err| err.to_string())?;
    receive
        .recv()
        .await
        .ok_or_else(|| "Could not confirm Stream window visibility.".to_string())?
}

fn handle_minimize(hwnd: HWND, dashboard: usize) -> Result<(), String> {
    let installed =
        unsafe { SetWindowSubclass(hwnd, Some(stream_window_proc), SUBCLASS_ID, dashboard) };
    if installed == 0 {
        return Err(
            "Could not configure the Stream window to stay available when minimized.".into(),
        );
    }
    Ok(())
}

pub fn park(window: &WebviewWindow) -> Result<(), String> {
    let (hwnd, dashboard) = native_handles(window)?;
    window
        .run_on_main_thread(move || {
            if let Err(err) = park_native(hwnd as HWND, dashboard as HWND) {
                log::warn!("{err}");
            }
        })
        .map_err(|err| err.to_string())
}

/// Exercise the same minimize handler on a disposable verification window.
pub async fn verify_minimize(window: &WebviewWindow) -> Result<String, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SendMessageW, GWL_EXSTYLE, WS_EX_TOPMOST,
    };

    let (hwnd, dashboard) = native_handles(window)?;
    let (send, mut receive) = channel(1);
    window
        .run_on_main_thread(move || {
            let hwnd = hwnd as HWND;
            let result = handle_minimize(hwnd, dashboard).and_then(|()| unsafe {
                SendMessageW(hwnd, WM_SYSCOMMAND, SC_MINIMIZE as WPARAM, 0);
                if IsIconic(hwnd) != 0 || IsWindowVisible(hwnd) == 0 {
                    return Err("Windows minimized or hid the verification window.".into());
                }
                if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0 {
                    return Err("The verification window stayed on top after minimize.".into());
                }
                Ok("Minimize lowered the test window and left the same window shown and unminimized.".into())
            });
            let _ = send.try_send(result);
        })
        .map_err(|err| err.to_string())?;
    receive
        .recv()
        .await
        .ok_or_else(|| "Could not read back the minimize result.".to_string())?
}
