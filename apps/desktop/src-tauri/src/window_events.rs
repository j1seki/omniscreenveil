//! Native window notifications feed a single coalescing reconciliation worker.
use std::sync::mpsc::SyncSender;

pub fn start(platform: veil_core::Platform, signal: SyncSender<()>) -> bool {
    match platform {
        #[cfg(windows)]
        veil_core::Platform::Windows => windows::start(signal),
        #[cfg(target_os = "linux")]
        veil_core::Platform::LinuxX11 => x11(signal),
        #[cfg(target_os = "linux")]
        veil_core::Platform::LinuxWayland => wayland(signal),
        #[cfg(target_os = "macos")]
        veil_core::Platform::MacOs => veil_protect_macos::MacBackend::watch_windows(signal),
        _ => {
            let _ = signal;
            false
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::sync::OnceLock;
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::{
            Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
            WindowsAndMessaging::*,
        },
    };

    static SIGNAL: OnceLock<SyncSender<()>> = OnceLock::new();

    unsafe extern "system" fn event(
        _: HWINEVENTHOOK,
        event: u32,
        hwnd: HWND,
        object: i32,
        child: i32,
        _: u32,
        _: u32,
    ) {
        if hwnd.is_null() || object != OBJID_WINDOW || child != 0 {
            return;
        }
        // Destroyed windows can no longer be queried. Coalescing keeps these
        // notifications cheap even when a process closes many child windows.
        if event != EVENT_OBJECT_DESTROY && GetAncestor(hwnd, GA_ROOT) != hwnd {
            return;
        }
        if let Some(tx) = SIGNAL.get() {
            let _ = tx.try_send(());
        }
    }

    pub fn start(signal: SyncSender<()>) -> bool {
        if SIGNAL.set(signal).is_err() {
            return false;
        }
        let (ready, status) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || unsafe {
            let mut hooks = Vec::new();
            for (first, last) in [
                (EVENT_OBJECT_CREATE, EVENT_OBJECT_HIDE),
                (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
                (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
            ] {
                let hook = SetWinEventHook(
                    first,
                    last,
                    std::ptr::null_mut(),
                    Some(event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                if hook.is_null() {
                    for hook in hooks {
                        UnhookWinEvent(hook);
                    }
                    let _ = ready.send(false);
                    return;
                }
                hooks.push(hook);
            }
            let _ = ready.send(true);
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                DispatchMessageW(&message);
            }
            for hook in hooks {
                UnhookWinEvent(hook);
            }
        });
        status.recv().unwrap_or(false)
    }
}

#[cfg(target_os = "linux")]
fn x11(signal: SyncSender<()>) -> bool {
    use x11rb::{
        connection::Connection,
        protocol::{
            xproto::{ChangeWindowAttributesAux, ConnectionExt, EventMask},
            Event,
        },
    };
    let Ok((conn, screen)) = x11rb::connect(None) else {
        return false;
    };
    let root = conn.setup().roots[screen].root;
    let mask = ChangeWindowAttributesAux::new()
        .event_mask(EventMask::SUBSTRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE);
    if !conn
        .change_window_attributes(root, &mask)
        .ok()
        .is_some_and(|request| request.check().is_ok())
    {
        return false;
    }
    let watched_atoms: Vec<_> = [
        b"_NET_CLIENT_LIST".as_slice(),
        b"_NET_CLIENT_LIST_STACKING",
        b"_NET_CURRENT_DESKTOP",
    ]
    .into_iter()
    .filter_map(|name| {
        conn.intern_atom(false, name)
            .ok()?
            .reply()
            .ok()
            .map(|reply| reply.atom)
    })
    .collect();
    std::thread::spawn(move || {
        while let Ok(event) = conn.wait_for_event() {
            let meaningful = match event {
                Event::MapNotify(_)
                | Event::UnmapNotify(_)
                | Event::DestroyNotify(_)
                | Event::ReparentNotify(_) => true,
                Event::PropertyNotify(event) => watched_atoms.contains(&event.atom),
                _ => false,
            };
            if meaningful
                && matches!(
                    signal.try_send(()),
                    Err(std::sync::mpsc::TrySendError::Disconnected(_))
                )
            {
                break;
            }
        }
    });
    true
}

#[cfg(target_os = "linux")]
fn wayland(signal: SyncSender<()>) -> bool {
    use std::{
        io::{BufRead, BufReader},
        os::unix::net::UnixStream,
        path::PathBuf,
        process::{Command, Stdio},
    };
    if std::env::var_os("SWAYSOCK").is_some() {
        let Ok(mut child) = Command::new("swaymsg")
            .args([
                "-m",
                "-t",
                "subscribe",
                "[\"window\",\"workspace\",\"output\"]",
            ])
            .stdout(Stdio::piped())
            .spawn()
        else {
            return false;
        };
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut response = String::new();
        if reader.read_line(&mut response).unwrap_or(0) == 0 || !response.contains("true") {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
        std::thread::spawn(move || {
            for line in reader.lines().map_while(Result::ok) {
                if line.contains("\"change\"")
                    && matches!(
                        signal.try_send(()),
                        Err(std::sync::mpsc::TrySendError::Disconnected(_))
                    )
                {
                    break;
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        });
        return true;
    }
    let Some(signature) = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE") else {
        return false;
    };
    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return false;
    };
    let path = PathBuf::from(runtime)
        .join("hypr")
        .join(signature)
        .join(".socket2.sock");
    let Ok(socket) = UnixStream::connect(path) else {
        return false;
    };
    std::thread::spawn(move || {
        for line in BufReader::new(socket).lines().map_while(Result::ok) {
            let kind = line.split_once(">>").map(|(kind, _)| kind).unwrap_or("");
            if [
                "openwindow",
                "closewindow",
                "movewindow",
                "windowtitle",
                "workspace",
                "monitoradded",
                "monitorremoved",
            ]
            .iter()
            .any(|event| kind.starts_with(event))
                && matches!(
                    signal.try_send(()),
                    Err(std::sync::mpsc::TrySendError::Disconnected(_))
                )
            {
                break;
            }
        }
    });
    true
}
