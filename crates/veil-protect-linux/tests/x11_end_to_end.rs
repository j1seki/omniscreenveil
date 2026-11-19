#![cfg(target_os = "linux")]

//! End-to-end X11 test against a real (virtual) X server.
//!
//! Proves, with pixels, that:
//! 1. a canary window is detectable in a root capture,
//! 2. the Veil Surface compositor removes it,
//! 3. strict hide removes it from the actual desktop,
//! 4. remapping restores it.
//!
//! Requires `Xvfb`; the test reports a skip instead of failing when it is
//! unavailable.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use veil_canary::{detect, render, Token};
use veil_core::ids::WindowId;
use veil_core::{Color, FillStyle, Profile, ProtectionMode, Rect};
use veil_protect::Engine;
use veil_protect_linux::{create_canary_window, X11Backend, X11Platform};
use veil_surface::{render as render_surface, SurfacePlan};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt, CreateWindowAux, MapState, PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

struct Xvfb(Child);

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_xvfb() -> Option<(Xvfb, String)> {
    for n in 90..180 {
        let socket = format!("/tmp/.X11-unix/X{n}");
        if std::path::Path::new(&socket).exists() {
            continue;
        }
        let mut child = Command::new("Xvfb")
            .arg(format!(":{n}"))
            .args(["-screen", "0", "800x600x24", "-nolisten", "tcp"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if std::path::Path::new(&socket).exists() {
                return Some((Xvfb(child), format!(":{n}")));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    None
}

fn wait_for_canary(platform: &X11Platform, token: Token, cell: u32, present: bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Ok(frame) = platform.capture_root() {
            if detect(&frame, token, cell).found == present {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn x11_canary_is_captured_masked_and_hidden() {
    let Some((_guard, display)) = spawn_xvfb() else {
        eprintln!("SKIP: Xvfb is not available");
        return;
    };

    let platform = X11Platform::connect_to(Some(&display)).expect("connect to Xvfb");
    let token = Token::from_seed("x11-end-to-end");
    let cell = 8;
    let canary = render(token, cell);
    let origin = (50i16, 40i16);
    let wid = create_canary_window(&platform, "veil-canary", &canary, origin)
        .expect("create canary window");

    // The window must show up in the inventory with sane geometry.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut found_window = None;
    while Instant::now() < deadline {
        if let Ok(list) = platform.list_windows() {
            if let Some(window) = list.iter().find(|w| w.id == WindowId(wid as u64)) {
                found_window = Some(window.clone());
                if window.visible {
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let window = found_window.expect("canary window is listed");
    assert_eq!(window.app_name, "veil-canary");
    assert_eq!(window.bounds, Rect::new(50, 40, 64, 64));
    assert!(window.visible);

    assert!(
        wait_for_canary(&platform, token, cell, true),
        "canary must be visible in the root capture before protection"
    );

    // 1. The Veil Surface does not capture the root: it composites foreign
    // windows and drops its own process's windows, which is what prevents the
    // Surface window from recursing into itself.
    let composite_all = platform
        .capture_desktop_excluding(0)
        .expect("composite every window");
    assert!(
        detect(&composite_all, token, cell).found,
        "the composited desktop must include a foreign window"
    );

    // Mark the canary as owned by this process. `capture_desktop` must then
    // drop it, exactly as it drops the app's own control and Surface windows.
    let (conn, _) = x11rb::connect(Some(&display)).expect("second X11 connection");
    let pid_atom = conn
        .intern_atom(false, b"_NET_WM_PID")
        .expect("intern _NET_WM_PID")
        .reply()
        .expect("intern reply")
        .atom;
    conn.change_property32(
        PropMode::REPLACE,
        wid,
        pid_atom,
        AtomEnum::CARDINAL,
        &[std::process::id()],
    )
    .unwrap()
    .check()
    .unwrap();
    let composite_own = platform
        .capture_desktop()
        .expect("composite without own windows");
    assert!(
        !detect(&composite_own, token, cell).found,
        "the composited desktop must exclude the app's own windows"
    );

    // Reparent the canary like a window manager would. The inventory should
    // report the frame's title bar and border as part of the client's mask.
    let foreign_pid = std::process::id() + 1;
    conn.change_property32(
        PropMode::REPLACE,
        wid,
        pid_atom,
        AtomEnum::CARDINAL,
        &[foreign_pid],
    )
    .unwrap()
    .check()
    .unwrap();
    let root = platform.root();
    let frame_id = conn.generate_id().unwrap();
    let title_color = Color::rgb(244, 90, 100);
    conn.create_window(
        0,
        frame_id,
        root,
        30,
        20,
        100,
        100,
        2,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new()
            .background_pixel(0xf45a64)
            .border_pixel(0xf45a64),
    )
    .unwrap()
    .check()
    .unwrap();
    conn.reparent_window(wid, frame_id, 8, 24)
        .unwrap()
        .check()
        .unwrap();
    conn.map_window(frame_id).unwrap().check().unwrap();
    veil_protect_linux::draw_frame(&platform, wid, &canary).unwrap();
    let clients = conn
        .intern_atom(false, b"_NET_CLIENT_LIST")
        .unwrap()
        .reply()
        .unwrap()
        .atom;
    conn.change_property32(PropMode::REPLACE, root, clients, AtomEnum::WINDOW, &[wid])
        .unwrap()
        .check()
        .unwrap();

    let window = platform
        .list_windows()
        .unwrap()
        .into_iter()
        .find(|w| w.id == WindowId(u64::from(wid)))
        .unwrap();
    assert_eq!(window.bounds, Rect::new(30, 20, 104, 104));
    let size = platform.root_size().expect("root size");
    let plan = SurfacePlan::new(size, FillStyle::veiled()).exclude(window.bounds);
    let frame = platform.capture_root().expect("capture");
    let masked = render_surface(&frame, &plan);
    assert_eq!(frame.get_pixel(40, 30), Some(title_color));
    assert_ne!(masked.get_pixel(40, 30), Some(title_color));
    assert!(
        !detect(&masked, token, cell).found,
        "surface compositor must mask the canary"
    );
    assert!(
        detect(&frame, token, cell).found,
        "the untouched desktop still contains the canary"
    );

    let backend = X11Backend::connect_to(Some(&display)).unwrap();
    let mut profile = Profile::new();
    profile.add_window_rule(window.id, ProtectionMode::StrictHide);
    let mut engine = Engine::new(profile, backend);
    let report = engine.tick(std::slice::from_ref(&window));
    assert_eq!(report.applied.len(), 1);
    assert!(
        wait_for_canary(&platform, token, cell, false),
        "strict hide must remove the canary from the desktop"
    );

    // Simulate a process crash and a WM that removes the withdrawn client
    // from its managed list. Recovery must find the marker directly.
    drop(engine);
    conn.change_property32(
        PropMode::REPLACE,
        root,
        clients,
        AtomEnum::WINDOW,
        &[frame_id],
    )
    .unwrap()
    .check()
    .unwrap();
    let fresh_backend = X11Backend::connect_to(Some(&display)).unwrap();
    let fresh = fresh_backend.platform();
    assert!(!fresh
        .restore_strict_hide(window.id, foreign_pid + 1)
        .unwrap());
    assert!(fresh.restore_strict_hide(window.id, foreign_pid).unwrap());
    // A remap repaints the background; a real application would repaint on
    // expose, so the test paints its canary again.
    veil_protect_linux::draw_frame(&platform, wid, &canary).expect("repaint canary");
    assert!(
        wait_for_canary(&platform, token, cell, true),
        "release must restore the window"
    );

    // A journal entry alone must not map a previously unmapped window.
    platform.set_mapped(window.id, false).unwrap();
    assert!(!fresh.restore_strict_hide(window.id, foreign_pid).unwrap());
    assert_eq!(
        conn.get_window_attributes(wid)
            .unwrap()
            .reply()
            .unwrap()
            .map_state,
        MapState::UNMAPPED
    );
}
