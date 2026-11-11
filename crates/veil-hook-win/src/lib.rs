//! Injected helper for the Windows backend.
//!
//! Loaded into a process the user explicitly selected. It watches that
//! process's own top-level windows and applies `WDA_EXCLUDEFROMCAPTURE` to the
//! ones marked with the `OmniScreenVeil.Protected` window property. The property
//! is set and cleared by the main application, so per-window control works
//! without any IPC channel.
//!
//! This is a compile-gated Phase 0 spike: it must be built with the MSVC
//! toolchain and tested against Defender/EDR before it ships.

#![cfg(windows)]
#![allow(unsafe_code)]

use std::time::Duration;

use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetPropW, GetWindowThreadProcessId, IsWindow, SetWindowDisplayAffinity,
    WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

const DLL_PROCESS_ATTACH: u32 = 1;
const POLL_INTERVAL: Duration = Duration::from_millis(200);

fn protected_prop() -> Vec<u16> {
    "OmniScreenVeil.Protected\0".encode_utf16().collect()
}

struct Context {
    prop: *const u16,
    out: *mut Vec<usize>,
}

unsafe extern "system" fn collect_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam as *mut Context);
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == GetCurrentProcessId() && IsWindow(hwnd) != 0 && !GetPropW(hwnd, ctx.prop).is_null() {
        (*ctx.out).push(hwnd as usize);
    }
    TRUE
}

fn poll_loop() {
    let prop = protected_prop();
    let mut tracked: Vec<usize> = Vec::new();
    loop {
        let mut found: Vec<usize> = Vec::new();
        let mut ctx = Context {
            prop: prop.as_ptr(),
            out: &mut found,
        };
        unsafe {
            EnumWindows(Some(collect_cb), &mut ctx as *mut _ as LPARAM);
        }
        for hwnd in &found {
            unsafe {
                SetWindowDisplayAffinity(*hwnd as HWND, WDA_EXCLUDEFROMCAPTURE);
            }
        }
        for stale in tracked.iter().filter(|h| !found.contains(h)) {
            unsafe {
                SetWindowDisplayAffinity(*stale as HWND, WDA_NONE);
            }
        }
        tracked = found;
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[no_mangle]
pub extern "system" fn DllMain(
    _instance: *mut core::ffi::c_void,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        // Keep the thread name stable so it is obvious in Process Explorer.
        let _ = std::thread::Builder::new()
            .name("omniscreenveil-hook".into())
            .spawn(poll_loop);
    }
    TRUE
}
