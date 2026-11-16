//! Injected helper for the Windows backend.
//!
//! Loaded into a process the user explicitly selected. It watches that
//! process's own top-level windows and applies `WDA_EXCLUDEFROMCAPTURE` to the
//! ones marked with the `OmniScreenVeil.Protected` window property. The property
//! is set and cleared by the main application, so per-window control works
//! without any IPC channel.
//!
//! Carefully avoids two classic Win32 pitfalls:
//! 1. Uses raw Win32 `CreateThread` + `DisableThreadLibraryCalls` in `DllMain`
//!    instead of `std::thread::spawn`, avoiding loader-lock deadlocks.
//! 2. Queries `GetWindowDisplayAffinity` before calling `SetWindowDisplayAffinity`
//!    so DWM redirection surfaces are not recreated on every poll tick.

#![cfg(windows)]
#![allow(unsafe_code)]

use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HANDLE, HMODULE, HWND, LPARAM, TRUE};
use windows_sys::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows_sys::Win32::System::Threading::{CreateThread, GetCurrentProcessId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetPropW, GetWindowDisplayAffinity, GetWindowThreadProcessId, IsWindow,
    RemovePropW, SetPropW, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

const DLL_PROCESS_ATTACH: u32 = 1;
const POLL_INTERVAL: Duration = Duration::from_millis(50);

const PROTECTED_PROP_UTF16: &[u16] = &[
    b'O' as u16,
    b'm' as u16,
    b'n' as u16,
    b'i' as u16,
    b'S' as u16,
    b'c' as u16,
    b'r' as u16,
    b'e' as u16,
    b'e' as u16,
    b'n' as u16,
    b'V' as u16,
    b'e' as u16,
    b'i' as u16,
    b'l' as u16,
    b'.' as u16,
    b'P' as u16,
    b'r' as u16,
    b'o' as u16,
    b't' as u16,
    b'e' as u16,
    b'c' as u16,
    b't' as u16,
    b'e' as u16,
    b'd' as u16,
    0,
];

const APPLIED_PROP_UTF16: &[u16] = &[
    b'O' as u16,
    b'm' as u16,
    b'n' as u16,
    b'i' as u16,
    b'S' as u16,
    b'c' as u16,
    b'r' as u16,
    b'e' as u16,
    b'e' as u16,
    b'n' as u16,
    b'V' as u16,
    b'e' as u16,
    b'i' as u16,
    b'l' as u16,
    b'.' as u16,
    b'A' as u16,
    b'p' as u16,
    b'p' as u16,
    b'l' as u16,
    b'i' as u16,
    b'e' as u16,
    b'd' as u16,
    0,
];

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

unsafe fn ensure_affinity(hwnd: HWND, desired: u32) -> bool {
    let mut current = 0u32;
    if GetWindowDisplayAffinity(hwnd, &mut current) != 0 && current == desired {
        return true;
    }
    SetWindowDisplayAffinity(hwnd, desired) != 0
}

unsafe extern "system" fn worker_thread(_param: *mut core::ffi::c_void) -> u32 {
    let mut tracked: Vec<usize> = Vec::new();
    loop {
        let mut found: Vec<usize> = Vec::new();
        let mut ctx = Context {
            prop: PROTECTED_PROP_UTF16.as_ptr(),
            out: &mut found,
        };
        EnumWindows(Some(collect_cb), &mut ctx as *mut _ as LPARAM);

        for &hwnd_val in &found {
            let hwnd = hwnd_val as HWND;
            if ensure_affinity(hwnd, WDA_EXCLUDEFROMCAPTURE) {
                let _ = SetPropW(hwnd, APPLIED_PROP_UTF16.as_ptr(), 1 as HANDLE);
            }
        }
        for &stale_val in tracked.iter().filter(|h| !found.contains(h)) {
            let stale = stale_val as HWND;
            if IsWindow(stale) != 0 {
                let _ = ensure_affinity(stale, WDA_NONE);
                let _ = RemovePropW(stale, APPLIED_PROP_UTF16.as_ptr());
            }
        }
        tracked = found;
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Win32 DLL entry point, called by the OS loader with a valid module handle.
///
/// # Safety
/// The `instance` handle is provided by the loader and must be a valid module
/// handle for the duration of the call. This function must not be called
/// directly by Rust code.
#[no_mangle]
pub unsafe extern "system" fn DllMain(
    instance: HMODULE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            let _ = DisableThreadLibraryCalls(instance);
            let thread = CreateThread(
                std::ptr::null(),
                0,
                Some(worker_thread),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
            );
            if !thread.is_null() {
                CloseHandle(thread);
            }
        }
    }
    TRUE
}
