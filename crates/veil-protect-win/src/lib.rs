//! Windows backend (Phase 0).
//!
//! Two mechanisms, matching the capability matrix:
//!
//! * `Exclusion` — inject `veil_hook_win.dll` into the target process and mark
//!   the chosen windows with a window property. The helper applies
//!   `WDA_EXCLUDEFROMCAPTURE` from *inside* the owning process, which Windows
//!   requires. Injection is deliberately scoped: only processes the user
//!   selected, no persistence, no service.
//! * `StrictHide` — `ShowWindow(SW_HIDE)` / `SW_SHOW`, tracked so the window is
//!   restored even after a crash of the UI (the helper keeps no state).
//!
//! `Surface` is a no-op at the OS level: the compositor masks the window in the
//! shared surface.
//!
//! Everything in this file is compile-gated to Windows. The pure helpers at the
//! bottom are tested on every host.

#![cfg_attr(windows, allow(unsafe_code))]

use veil_core::ids::WindowId;
use veil_core::state::ProtectionIntent;
use veil_core::window::Platform;
use veil_protect::{BackendError, ProtectionBackend};

#[derive(Debug, thiserror::Error)]
pub enum WinError {
    #[error("Windows backend is only available on Windows")]
    NotWindows,
    #[error("win32 {call} failed (error {code})")]
    Win32 { call: &'static str, code: u32 },
    #[error("{0}")]
    Other(String),
}

pub struct WinBackend {
    #[cfg(windows)]
    inner: win::Inner,
    #[cfg(not(windows))]
    _private: (),
}

impl WinBackend {
    pub fn new() -> Result<Self, WinError> {
        #[cfg(windows)]
        {
            Ok(Self {
                inner: win::Inner::new()?,
            })
        }
        #[cfg(not(windows))]
        {
            Err(WinError::NotWindows)
        }
    }

    /// Live inventory, used by the UI and the reconcile loop.
    pub fn list_windows(&self) -> Result<Vec<veil_core::WindowInfo>, WinError> {
        #[cfg(windows)]
        {
            self.inner.list_windows()
        }
        #[cfg(not(windows))]
        {
            Err(WinError::NotWindows)
        }
    }
}

impl ProtectionBackend for WinBackend {
    fn platform(&self) -> Platform {
        Platform::Windows
    }

    fn apply(&mut self, intent: &ProtectionIntent) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            self.inner
                .apply(intent)
                .map_err(|e| BackendError::failed(e.to_string()))
        }
        #[cfg(not(windows))]
        {
            Err(BackendError::Unsupported {
                platform: Platform::Windows,
                mode: intent.mode,
                reason: "not running on Windows".into(),
            })
        }
    }

    fn release(&mut self, window: WindowId) -> Result<(), BackendError> {
        #[cfg(windows)]
        {
            self.inner
                .release(window)
                .map_err(|e| BackendError::failed(e.to_string()))
        }
        #[cfg(not(windows))]
        {
            let _ = window;
            Ok(())
        }
    }

    fn describe(&self) -> String {
        "Windows backend (display affinity + strict hide)".to_string()
    }
}

/// Window property used to tell the injected helper which windows to protect.
pub const PROTECTED_PROP: &str = "OmniScreenVeil.Protected";

/// Name of the injected library next to the main executable.
pub const HOOK_DLL_NAME: &str = "veil_hook_win.dll";

/// Normalize a Windows executable path into a stable rule key.
pub fn normalize_exe(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;

    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, BOOL, HANDLE, HWND, LPARAM, RECT, TRUE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows_sys::Win32::System::Memory::{
        VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
    };
    use windows_sys::Win32::System::Threading::{
        CreateRemoteThread, GetCurrentProcessId, GetExitCodeThread, OpenProcess,
        QueryFullProcessImageNameW, WaitForSingleObject, LPTHREAD_START_ROUTINE,
        PROCESS_CREATE_THREAD, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_OPERATION,
        PROCESS_VM_WRITE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowLongW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
        IsIconic, IsWindowVisible, RemovePropW, SetPropW, ShowWindow, GWL_EXSTYLE, SW_HIDE,
        SW_SHOW, WS_EX_TOOLWINDOW,
    };

    use veil_core::geometry::Rect;
    use veil_core::ids::AppKey;
    use veil_core::window::WindowInfo;
    use veil_core::ProtectionMode;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error(call: &'static str) -> WinError {
        WinError::Win32 {
            call,
            code: unsafe { GetLastError() },
        }
    }

    #[derive(Debug, Clone)]
    struct RawWindow {
        hwnd: HWND,
        pid: u32,
        title: String,
        bounds: Rect,
        visible: bool,
        minimized: bool,
    }

    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam as *mut Vec<RawWindow>);
        let exstyle = GetWindowLongW(hwnd, GWL_EXSTYLE as i32) as u32;
        if exstyle & WS_EX_TOOLWINDOW != 0 {
            return TRUE;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 || pid == GetCurrentProcessId() {
            return TRUE;
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return TRUE;
        }
        let w = (rect.right - rect.left).max(0) as u32;
        let h = (rect.bottom - rect.top).max(0) as u32;
        if w == 0 || h == 0 {
            return TRUE;
        }
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = if len > 0 {
            String::from_utf16_lossy(&title_buf[..len as usize])
        } else {
            String::new()
        };
        out.push(RawWindow {
            hwnd,
            pid,
            title,
            bounds: Rect::new(rect.left, rect.top, w, h),
            visible: IsWindowVisible(hwnd) != 0,
            minimized: IsIconic(hwnd) != 0,
        });
        TRUE
    }

    fn process_image(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut buf = [0u16; 32768];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(handle);
            if ok == 0 {
                return None;
            }
            let os = OsString::from_wide(&buf[..len as usize]);
            Some(os.to_string_lossy().into_owned())
        }
    }

    pub struct Inner {
        injected: BTreeSet<u32>,
        /// window -> (pid, mode) so release knows what to undo.
        protected: BTreeMap<WindowId, (u32, ProtectionMode)>,
    }

    impl Inner {
        pub fn new() -> Result<Self, WinError> {
            Ok(Self {
                injected: BTreeSet::new(),
                protected: BTreeMap::new(),
            })
        }

        pub fn list_windows(&self) -> Result<Vec<WindowInfo>, WinError> {
            let mut raw: Vec<RawWindow> = Vec::new();
            unsafe {
                EnumWindows(Some(enum_cb), &mut raw as *mut _ as LPARAM);
            }
            let mut exe_cache: BTreeMap<u32, String> = BTreeMap::new();
            let mut out = Vec::with_capacity(raw.len());
            for window in raw {
                let exe = exe_cache
                    .entry(window.pid)
                    .or_insert_with(|| process_image(window.pid).unwrap_or_default())
                    .clone();
                if exe.is_empty() {
                    continue;
                }
                let app_name = exe
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&exe)
                    .trim_end_matches(".exe")
                    .to_string();
                out.push(WindowInfo {
                    id: WindowId(window.hwnd as usize as u64),
                    app: AppKey::WindowsExecutable(normalize_exe(&exe)),
                    app_name,
                    title: window.title,
                    pid: window.pid,
                    bounds: window.bounds,
                    monitor: None,
                    visible: window.visible,
                    minimized: window.minimized,
                    top_level: true,
                    platform: Platform::Windows,
                });
            }
            Ok(out)
        }

        fn hook_dll_path(&self) -> Result<PathBuf, WinError> {
            if let Some(path) = std::env::var_os("OMV_HOOK_DLL") {
                return Ok(PathBuf::from(path));
            }
            let exe = std::env::current_exe()
                .map_err(|e| WinError::Other(format!("cannot find own executable: {e}")))?;
            Ok(exe.with_file_name(HOOK_DLL_NAME))
        }

        fn ensure_injected(&mut self, pid: u32) -> Result<(), WinError> {
            if self.injected.contains(&pid) {
                return Ok(());
            }
            let dll = self.hook_dll_path()?;
            let dll_w = wide(&dll.to_string_lossy());
            unsafe {
                let process = OpenProcess(
                    PROCESS_CREATE_THREAD
                        | PROCESS_QUERY_LIMITED_INFORMATION
                        | PROCESS_VM_OPERATION
                        | PROCESS_VM_WRITE,
                    0,
                    pid,
                );
                if process.is_null() {
                    return Err(last_error("OpenProcess"));
                }
                let bytes = dll_w.len() * 2;
                let remote = VirtualAllocEx(
                    process,
                    std::ptr::null(),
                    bytes,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                );
                if remote.is_null() {
                    CloseHandle(process);
                    return Err(last_error("VirtualAllocEx"));
                }
                let mut written = 0usize;
                let ok = WriteProcessMemory(
                    process,
                    remote,
                    dll_w.as_ptr() as *const _,
                    bytes,
                    &mut written,
                );
                if ok == 0 || written != bytes {
                    let err = last_error("WriteProcessMemory");
                    VirtualFreeEx(process, remote, 0, MEM_RELEASE);
                    CloseHandle(process);
                    return Err(err);
                }
                let kernel = GetModuleHandleW(wide("kernel32.dll").as_ptr());
                let load_library = GetProcAddress(kernel, b"LoadLibraryW\0".as_ptr());
                let Some(load_library) = load_library else {
                    VirtualFreeEx(process, remote, 0, MEM_RELEASE);
                    CloseHandle(process);
                    return Err(WinError::Other("LoadLibraryW not found".into()));
                };
                let start: LPTHREAD_START_ROUTINE =
                    Some(std::mem::transmute::<
                        *const (),
                        unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
                    >(load_library as *const ()));
                let thread = CreateRemoteThread(
                    process,
                    std::ptr::null(),
                    0,
                    start,
                    remote,
                    0,
                    std::ptr::null_mut(),
                );
                if thread.is_null() {
                    let err = last_error("CreateRemoteThread");
                    VirtualFreeEx(process, remote, 0, MEM_RELEASE);
                    CloseHandle(process);
                    return Err(err);
                }
                let wait = WaitForSingleObject(thread, 5000);
                let mut exit_code = 0u32;
                let _ = GetExitCodeThread(thread, &mut exit_code);
                CloseHandle(thread);
                VirtualFreeEx(process, remote, 0, MEM_RELEASE);
                CloseHandle(process);
                if wait != WAIT_OBJECT_0 {
                    return Err(WinError::Other("injection timed out".into()));
                }
                if exit_code == 0 {
                    return Err(WinError::Other(
                        "LoadLibraryW failed in the target process (architecture mismatch?)".into(),
                    ));
                }
                self.injected.insert(pid);
                Ok(())
            }
        }

        pub fn apply(&mut self, intent: &ProtectionIntent) -> Result<(), WinError> {
            let hwnd = intent.window.0 as usize as HWND;
            match intent.mode {
                ProtectionMode::Surface => {
                    self.protected
                        .insert(intent.window, (intent.pid, ProtectionMode::Surface));
                    Ok(())
                }
                ProtectionMode::Exclusion => {
                    self.ensure_injected(intent.pid)?;
                    let prop = wide(PROTECTED_PROP);
                    let ok = unsafe { SetPropW(hwnd, prop.as_ptr(), 1 as HANDLE) };
                    if ok == 0 {
                        return Err(last_error("SetPropW"));
                    }
                    self.protected
                        .insert(intent.window, (intent.pid, ProtectionMode::Exclusion));
                    Ok(())
                }
                ProtectionMode::StrictHide => {
                    unsafe {
                        ShowWindow(hwnd, SW_HIDE);
                    }
                    self.protected
                        .insert(intent.window, (intent.pid, ProtectionMode::StrictHide));
                    Ok(())
                }
            }
        }

        pub fn release(&mut self, window: WindowId) -> Result<(), WinError> {
            let Some((_pid, mode)) = self.protected.remove(&window) else {
                return Ok(());
            };
            let hwnd = window.0 as usize as HWND;
            unsafe {
                match mode {
                    ProtectionMode::StrictHide => {
                        ShowWindow(hwnd, SW_SHOW);
                    }
                    ProtectionMode::Exclusion => {
                        let prop = wide(PROTECTED_PROP);
                        RemovePropW(hwnd, prop.as_ptr());
                    }
                    ProtectionMode::Surface => {}
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_windows_paths() {
        assert_eq!(
            normalize_exe("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"),
            "c:/program files/google/chrome/application/chrome.exe"
        );
    }

    #[test]
    fn prop_and_dll_names_are_stable() {
        assert_eq!(PROTECTED_PROP, "OmniScreenVeil.Protected");
        assert_eq!(HOOK_DLL_NAME, "veil_hook_win.dll");
    }

    #[cfg(not(windows))]
    #[test]
    fn stub_reports_not_windows() {
        assert!(matches!(WinBackend::new(), Err(WinError::NotWindows)));
    }
}
