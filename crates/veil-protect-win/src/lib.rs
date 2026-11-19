//! Windows backend.
//!
//! Three mechanisms, matching the capability matrix:
//!
//! * `Exclusion` — loads `veil_hook_win.dll` into the target process and waits
//!   for `GetWindowDisplayAffinity` to confirm the requested setting. This
//!   setting is not proof that every capture path respects it.
//! * `Surface` — live desktop capture via GDI `BitBlt` with protected windows
//!   and our own windows masked by the `veil-surface` compositor.
//! * `StrictHide` — `ShowWindow(SW_HIDE)` / `SW_SHOW`, tracked in-process and
//!   preserved across inventory scans so reconcile never flickers.
//!
//! Everything Win32-specific is compile-gated to Windows; the pure helpers at
//! the bottom are unit-tested on every host.

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
        self.inventory(true)
    }

    pub fn inventory(&self, icons: bool) -> Result<Vec<veil_core::WindowInfo>, WinError> {
        #[cfg(windows)]
        {
            self.inner.list_windows(icons)
        }
        #[cfg(not(windows))]
        {
            let _ = icons;
            Err(WinError::NotWindows)
        }
    }

    #[cfg(windows)]
    pub fn outputs() -> Result<Vec<(String, veil_core::Rect)>, WinError> {
        win::outputs()
    }

    /// Capture the live Windows desktop as a BGRA frame for the Veil Surface.
    pub fn capture_root(&self) -> Result<veil_surface::Frame, WinError> {
        #[cfg(windows)]
        {
            self.inner.capture_root()
        }
        #[cfg(not(windows))]
        {
            Err(WinError::NotWindows)
        }
    }

    /// Bounds of every window owned by this process so the Veil Surface masks
    /// its own control and preview windows and never recurses.
    pub fn own_windows(&self) -> Vec<veil_core::Rect> {
        #[cfg(windows)]
        {
            self.inner.own_windows()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
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
        "Windows backend (display affinity + GDI surface + strict hide)".to_string()
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

/// Filter predicate for Win32 class names that belong to desktop shell chrome
/// rather than user applications.
pub fn is_shell_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "Windows.UI.Core.CoreWindow"
            | "IME"
            | "MSCTFIME UI"
    )
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, BOOL, HANDLE, HWND, LPARAM, LUID, RECT, TRUE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors,
        GetDC, GetMonitorInfoW, GetObjectW, ReleaseDC, SelectObject, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, DIB_RGB_COLORS, HDC, HMONITOR, MONITORINFOEXW, SRCCOPY,
    };
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows_sys::Win32::System::Memory::{
        VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
    };
    use windows_sys::Win32::System::Threading::{
        CreateRemoteThread, GetCurrentProcess, GetCurrentProcessId, GetExitCodeThread, OpenProcess,
        OpenProcessToken, QueryFullProcessImageNameW, WaitForSingleObject, LPTHREAD_START_ROUTINE,
        PROCESS_CREATE_THREAD, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_OPERATION,
        PROCESS_VM_WRITE,
    };
    use windows_sys::Win32::UI::Shell::ExtractAssociatedIconW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, EnumChildWindows, EnumWindows, GetClassNameW, GetIconInfo, GetSystemMetrics,
        GetWindow, GetWindowDisplayAffinity, GetWindowLongW, GetWindowRect, GetWindowTextW,
        GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, RemovePropW, SetPropW,
        SetWindowDisplayAffinity, ShowWindow, GWL_EXSTYLE, GW_OWNER, HICON, ICONINFO, SM_CXSCREEN,
        SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        SW_HIDE, SW_SHOW, WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
    };

    use veil_core::geometry::Rect;
    use veil_core::ids::AppKey;
    use veil_core::window::WindowInfo;
    use veil_core::ProtectionMode;
    use veil_surface::{Frame, PixelFormat};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error(call: &'static str) -> WinError {
        WinError::Win32 {
            call,
            code: unsafe { GetLastError() },
        }
    }

    /// Best-effort enable `SeDebugPrivilege` on the current process token so
    /// when the user runs OmniScreenVeil as Administrator it can protect
    /// elevated windows as well.
    fn enable_debug_privilege() {
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                &mut token,
            ) == 0
            {
                return;
            }
            let mut luid = LUID {
                LowPart: 0,
                HighPart: 0,
            };
            let priv_name = wide("SeDebugPrivilege");
            if LookupPrivilegeValueW(std::ptr::null(), priv_name.as_ptr(), &mut luid) != 0 {
                let tp = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [windows_sys::Win32::Security::LUID_AND_ATTRIBUTES {
                        Luid: luid,
                        Attributes: SE_PRIVILEGE_ENABLED,
                    }],
                };
                let _ = AdjustTokenPrivileges(
                    token,
                    0,
                    &tp,
                    std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }
            CloseHandle(token);
        }
    }

    unsafe fn is_window_cloaked(hwnd: HWND) -> bool {
        let mut cloaked: u32 = 0;
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            &mut cloaked as *mut _ as *mut _,
            std::mem::size_of::<u32>() as u32,
        );
        hr == 0 && cloaked != 0
    }

    unsafe fn window_class_name(hwnd: HWND) -> String {
        let mut buf = [0u16; 256];
        let len = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }

    unsafe fn window_title(hwnd: HWND) -> String {
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        if len > 0 {
            String::from_utf16_lossy(&title_buf[..len as usize])
        } else {
            String::new()
        }
    }

    unsafe fn window_visible_bounds(hwnd: HWND) -> Option<Rect> {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let is_min = IsIconic(hwnd) != 0;
        if !is_min {
            let hr = DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS as u32,
                &mut rect as *mut _ as *mut _,
                std::mem::size_of::<RECT>() as u32,
            );
            if hr != 0 && GetWindowRect(hwnd, &mut rect) == 0 {
                return None;
            }
        } else if GetWindowRect(hwnd, &mut rect) == 0 {
            return None;
        }
        let w = (rect.right - rect.left).max(0) as u32;
        let h = (rect.bottom - rect.top).max(0) as u32;
        if !is_min && (w < 32 || h < 32) {
            return None;
        }
        Some(Rect::new(rect.left, rect.top, w.max(32), h.max(32)))
    }

    /// For UWP windows hosted inside `ApplicationFrameHost.exe`, find the real
    /// child `CoreWindow` process ID so the app name and icon match the UWP app.
    unsafe fn resolve_uwp_child_pid(frame_hwnd: HWND, frame_pid: u32) -> u32 {
        struct ChildCtx {
            frame_pid: u32,
            resolved_pid: u32,
        }
        unsafe extern "system" fn child_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let ctx = &mut *(lparam as *mut ChildCtx);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != 0 && pid != ctx.frame_pid {
                ctx.resolved_pid = pid;
                return 0;
            }
            TRUE
        }
        let mut ctx = ChildCtx {
            frame_pid,
            resolved_pid: frame_pid,
        };
        EnumChildWindows(frame_hwnd, Some(child_cb), &mut ctx as *mut _ as LPARAM);
        ctx.resolved_pid
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
        if IsWindowVisible(hwnd) == 0 {
            return TRUE;
        }
        if is_window_cloaked(hwnd) {
            return TRUE;
        }
        let exstyle = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let is_app_window = (exstyle & WS_EX_APPWINDOW) != 0;
        if (exstyle & WS_EX_TOOLWINDOW) != 0 && !is_app_window {
            return TRUE;
        }
        if (exstyle & WS_EX_NOACTIVATE) != 0 && !is_app_window {
            return TRUE;
        }
        if !GetWindow(hwnd, GW_OWNER).is_null() && !is_app_window {
            return TRUE;
        }
        let class_name = window_class_name(hwnd);
        if is_shell_class(&class_name) {
            return TRUE;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 || pid == GetCurrentProcessId() {
            return TRUE;
        }
        let title = window_title(hwnd);
        if title.trim().is_empty() {
            return TRUE;
        }
        let Some(bounds) = window_visible_bounds(hwnd) else {
            return TRUE;
        };
        if class_name == "ApplicationFrameWindow" {
            pid = resolve_uwp_child_pid(hwnd, pid);
        }
        out.push(RawWindow {
            hwnd,
            pid,
            title,
            bounds,
            visible: true,
            minimized: IsIconic(hwnd) != 0,
        });
        TRUE
    }

    /// Encode a BGRA pixel buffer as a base64 PNG.
    fn bgra_to_png_base64(pixels: &[u8], w: u32, h: u32) -> Option<String> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().ok()?;
            let mut rgba = Vec::with_capacity(pixels.len());
            for px in pixels.chunks_exact(4) {
                rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
            writer.write_image_data(&rgba).ok()?;
        }
        use base64::Engine;
        Some(base64::engine::general_purpose::STANDARD.encode(buf))
    }

    fn app_icon(path: &str) -> String {
        exe_icon(path)
            .and_then(|(pixels, w, h)| bgra_to_png_base64(&pixels, w, h))
            .unwrap_or_default()
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

    fn exe_icon(path: &str) -> Option<(Vec<u8>, u32, u32)> {
        unsafe {
            let mut wide = wide(path);
            let mut index = 0u16;
            let icon = ExtractAssociatedIconW(std::ptr::null_mut(), wide.as_mut_ptr(), &mut index);
            if icon.is_null() {
                return None;
            }
            let converted = icon_to_bgra(icon);
            DestroyIcon(icon);
            converted
        }
    }

    unsafe fn icon_to_bgra(icon: HICON) -> Option<(Vec<u8>, u32, u32)> {
        let mut info = ICONINFO {
            fIcon: 0,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: std::ptr::null_mut(),
            hbmColor: std::ptr::null_mut(),
        };
        if GetIconInfo(icon, &mut info) == 0 {
            return None;
        }

        let result = (|| {
            let bitmap = info.hbmColor;
            if bitmap.is_null() {
                return None;
            }
            let mut bmp: BITMAP = std::mem::zeroed();
            if GetObjectW(
                bitmap as *mut _,
                std::mem::size_of::<BITMAP>() as i32,
                &mut bmp as *mut _ as *mut _,
            ) == 0
            {
                return None;
            }
            let w = bmp.bmWidth;
            let h = bmp.bmHeight;
            if w <= 0 || h <= 0 {
                return None;
            }

            let screen = GetDC(std::ptr::null_mut());
            if screen.is_null() {
                return None;
            }
            let dc = CreateCompatibleDC(screen);
            let icon_dc = CreateCompatibleDC(screen);
            ReleaseDC(std::ptr::null_mut(), screen);
            if dc.is_null() || icon_dc.is_null() {
                if !dc.is_null() {
                    DeleteDC(dc);
                }
                if !icon_dc.is_null() {
                    DeleteDC(icon_dc);
                }
                return None;
            }

            let header = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };
            let bmi = BITMAPINFO {
                bmiHeader: header,
                bmiColors: [std::mem::zeroed(); 1],
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dib =
                CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if dib.is_null() || bits.is_null() {
                DeleteDC(dc);
                DeleteDC(icon_dc);
                return None;
            }

            let old_icon = SelectObject(icon_dc, bitmap as *mut _);
            let old_dib = SelectObject(dc, dib as *mut _);
            let ok = BitBlt(dc, 0, 0, w, h, icon_dc, 0, 0, SRCCOPY);
            SelectObject(dc, old_dib);
            SelectObject(icon_dc, old_icon);
            DeleteDC(dc);
            DeleteDC(icon_dc);

            if ok == 0 {
                DeleteObject(dib as *mut _);
                return None;
            }

            let row = (w as usize) * 4;
            let mut pixels = vec![0u8; row * h as usize];
            std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), pixels.len());
            DeleteObject(dib as *mut _);
            Some((pixels, w as u32, h as u32))
        })();

        if !info.hbmMask.is_null() {
            DeleteObject(info.hbmMask as *mut _);
        }
        if !info.hbmColor.is_null() {
            DeleteObject(info.hbmColor as *mut _);
        }
        result
    }

    /// Virtual desktop rectangle `(origin_x, origin_y, width, height)`.
    unsafe fn virtual_screen_rect() -> (i32, i32, u32, u32) {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if vw > 0 && vh > 0 {
            (vx, vy, vw as u32, vh as u32)
        } else {
            let sw = GetSystemMetrics(SM_CXSCREEN).max(1280) as u32;
            let sh = GetSystemMetrics(SM_CYSCREEN).max(720) as u32;
            (0, 0, sw, sh)
        }
    }

    pub fn outputs() -> Result<Vec<(String, Rect)>, WinError> {
        unsafe extern "system" fn collect(
            monitor: HMONITOR,
            _: HDC,
            _: *mut RECT,
            data: LPARAM,
        ) -> BOOL {
            let out = &mut *(data as *mut Vec<(String, Rect)>);
            let mut info: MONITORINFOEXW = std::mem::zeroed();
            info.monitorInfo.cbSize = std::mem::size_of_val(&info) as u32;
            if GetMonitorInfoW(monitor, &mut info.monitorInfo) == 0 {
                return 0;
            }
            let end = info
                .szDevice
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(info.szDevice.len());
            let (x, y, _, _) = virtual_screen_rect();
            let r = info.monitorInfo.rcMonitor;
            out.push((
                String::from_utf16_lossy(&info.szDevice[..end]),
                Rect::new(
                    r.left - x,
                    r.top - y,
                    (r.right - r.left) as u32,
                    (r.bottom - r.top) as u32,
                ),
            ));
            TRUE
        }
        let mut out = Vec::new();
        if unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(collect),
                &mut out as *mut _ as LPARAM,
            )
        } == 0
        {
            return Err(last_error("EnumDisplayMonitors"));
        }
        Ok(out)
    }

    pub struct Inner {
        injected: BTreeSet<u32>,
        /// window -> (pid, mode) so release knows what to undo and strict-hidden
        /// windows stay in the inventory.
        protected: BTreeMap<WindowId, (u32, ProtectionMode)>,
    }

    impl Inner {
        pub fn new() -> Result<Self, WinError> {
            enable_debug_privilege();
            Ok(Self {
                injected: BTreeSet::new(),
                protected: BTreeMap::new(),
            })
        }

        pub fn list_windows(&self, icons: bool) -> Result<Vec<WindowInfo>, WinError> {
            let (vx, vy, _, _) = unsafe { virtual_screen_rect() };
            let mut raw: Vec<RawWindow> = Vec::new();
            unsafe {
                EnumWindows(Some(enum_cb), &mut raw as *mut _ as LPARAM);
            }

            // Keep windows we hid via `StrictHide` in the inventory even though
            // `IsWindowVisible` is now false for them; otherwise reconcile
            // would think the window closed and immediately release/re-show it!
            for (&win_id, &(pid, mode)) in &self.protected {
                if mode != ProtectionMode::StrictHide {
                    continue;
                }
                if raw.iter().any(|w| w.hwnd as usize as u64 == win_id.0) {
                    continue;
                }
                let hwnd = win_id.0 as usize as HWND;
                unsafe {
                    if IsWindow(hwnd) == 0 {
                        continue;
                    }
                    let title = window_title(hwnd);
                    let bounds =
                        window_visible_bounds(hwnd).unwrap_or_else(|| Rect::new(0, 0, 400, 300));
                    raw.push(RawWindow {
                        hwnd,
                        pid,
                        title,
                        bounds,
                        visible: false,
                        minimized: IsIconic(hwnd) != 0,
                    });
                }
            }

            let mut exe_cache: BTreeMap<u32, String> = BTreeMap::new();
            let mut icon_cache: BTreeMap<u32, String> = BTreeMap::new();
            let mut out = Vec::with_capacity(raw.len());
            for window in raw {
                let exe = exe_cache
                    .entry(window.pid)
                    .or_insert_with(|| process_image(window.pid).unwrap_or_default())
                    .clone();
                if exe.is_empty() {
                    continue;
                }
                let icon = if icons {
                    icon_cache
                        .entry(window.pid)
                        .or_insert_with(|| app_icon(&exe))
                        .clone()
                } else {
                    String::new()
                };
                let app_name = exe
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&exe)
                    .trim_end_matches(".exe")
                    .trim_end_matches(".EXE")
                    .to_string();
                let bounds = Rect::new(
                    window.bounds.x - vx,
                    window.bounds.y - vy,
                    window.bounds.w,
                    window.bounds.h,
                );
                out.push(WindowInfo {
                    id: WindowId(window.hwnd as usize as u64),
                    app: AppKey::WindowsExecutable(normalize_exe(&exe)),
                    app_name,
                    title: window.title,
                    pid: window.pid,
                    bounds,
                    monitor: None,
                    visible: window.visible,
                    minimized: window.minimized,
                    top_level: true,
                    platform: Platform::Windows,
                    icon,
                });
            }
            out.sort_by_key(|w| w.id);
            Ok(out)
        }

        pub fn own_windows(&self) -> Vec<Rect> {
            unsafe extern "system" fn own_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
                let out = &mut *(lparam as *mut Vec<Rect>);
                if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
                    return TRUE;
                }
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid != GetCurrentProcessId() {
                    return TRUE;
                }
                if let Some(bounds) = window_visible_bounds(hwnd) {
                    out.push(bounds);
                }
                TRUE
            }
            let (vx, vy, _, _) = unsafe { virtual_screen_rect() };
            let mut raw: Vec<Rect> = Vec::new();
            unsafe {
                EnumWindows(Some(own_cb), &mut raw as *mut _ as LPARAM);
            }
            raw.into_iter()
                .map(|r| Rect::new(r.x - vx, r.y - vy, r.w, r.h))
                .collect()
        }

        pub fn capture_root(&self) -> Result<Frame, WinError> {
            unsafe {
                let (vx, vy, width, height) = virtual_screen_rect();
                let screen_dc = GetDC(std::ptr::null_mut());
                if screen_dc.is_null() {
                    return Err(last_error("GetDC(NULL)"));
                }
                let mem_dc = CreateCompatibleDC(screen_dc);
                if mem_dc.is_null() {
                    ReleaseDC(std::ptr::null_mut(), screen_dc);
                    return Err(last_error("CreateCompatibleDC"));
                }

                let header = BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                };
                let bmi = BITMAPINFO {
                    bmiHeader: header,
                    bmiColors: [std::mem::zeroed(); 1],
                };
                let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
                let dib = CreateDIBSection(
                    mem_dc,
                    &bmi,
                    DIB_RGB_COLORS,
                    &mut bits,
                    std::ptr::null_mut(),
                    0,
                );
                if dib.is_null() || bits.is_null() {
                    DeleteDC(mem_dc);
                    ReleaseDC(std::ptr::null_mut(), screen_dc);
                    return Err(last_error("CreateDIBSection"));
                }

                let old = SelectObject(mem_dc, dib as *mut _);
                let ok = BitBlt(
                    mem_dc,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    screen_dc,
                    vx,
                    vy,
                    SRCCOPY,
                );
                SelectObject(mem_dc, old);
                DeleteDC(mem_dc);
                ReleaseDC(std::ptr::null_mut(), screen_dc);

                if ok == 0 {
                    DeleteObject(dib as *mut _);
                    return Err(last_error("BitBlt"));
                }

                let len = width as usize * height as usize * 4;
                let mut pixels = vec![0u8; len];
                std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), len);
                DeleteObject(dib as *mut _);

                for px in pixels.chunks_exact_mut(4) {
                    px[3] = 255;
                }
                Frame::from_bytes(width, height, PixelFormat::Bgra8, pixels)
                    .map_err(|e| WinError::Other(e.to_string()))
            }
        }

        fn hook_dll_path(&self) -> Result<PathBuf, WinError> {
            if let Some(path) = std::env::var_os("OMNISCREENVEIL_HOOK_DLL")
                .or_else(|| std::env::var_os("OMV_HOOK_DLL"))
            {
                let p = PathBuf::from(path);
                if p.is_file() {
                    return Ok(p);
                }
            }
            let exe = std::env::current_exe()
                .map_err(|e| WinError::Other(format!("cannot find own executable: {e}")))?;
            let exe_dir = exe
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));

            let candidates = [
                exe.with_file_name(HOOK_DLL_NAME),
                exe_dir.join("resources").join(HOOK_DLL_NAME),
                exe_dir.join("..").join("release").join(HOOK_DLL_NAME),
                exe_dir.join("..").join("debug").join(HOOK_DLL_NAME),
                exe_dir
                    .join("..")
                    .join("..")
                    .join("release")
                    .join(HOOK_DLL_NAME),
                exe_dir
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("target")
                    .join("release")
                    .join(HOOK_DLL_NAME),
                exe_dir
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("target")
                    .join("x86_64-pc-windows-msvc")
                    .join("release")
                    .join(HOOK_DLL_NAME),
            ];
            for candidate in &candidates {
                if let Ok(meta) = std::fs::metadata(candidate) {
                    if meta.is_file() && meta.len() > 0 {
                        return Ok(candidate.clone());
                    }
                }
            }
            Err(WinError::Other(format!(
                "exclusion helper {HOOK_DLL_NAME} not found next to {}",
                exe.display()
            )))
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
                    let code = GetLastError();
                    if code == 5 {
                        return Err(WinError::Other(format!(
                            "Access denied opening PID {pid} (error 5) — run OmniScreenVeil as Administrator or use Veil Surface mode."
                        )));
                    }
                    return Err(WinError::Win32 {
                        call: "OpenProcess",
                        code,
                    });
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
                let load_library = GetProcAddress(kernel, c"LoadLibraryW".as_ptr().cast());
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
                // A timed-out remote thread may still be reading this buffer.
                // Leave it allocated in that case rather than causing a use-after-free.
                if wait == WAIT_OBJECT_0 {
                    VirtualFreeEx(process, remote, 0, MEM_RELEASE);
                }
                CloseHandle(process);
                if wait != WAIT_OBJECT_0 {
                    return Err(WinError::Other("injection timed out".into()));
                }
                if exit_code == 0 {
                    return Err(WinError::Other(
                        "LoadLibraryW was blocked in target process (ACG / signature policy)"
                            .into(),
                    ));
                }
                self.injected.insert(pid);
                Ok(())
            }
        }

        unsafe fn verify_affinity(hwnd: HWND, expected: u32, timeout: Duration) -> bool {
            let deadline = Instant::now() + timeout;
            loop {
                let mut actual = 0u32;
                if GetWindowDisplayAffinity(hwnd, &mut actual) != 0 && actual == expected {
                    return true;
                }
                if Instant::now() >= deadline {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(15));
            }
        }

        pub fn apply(&mut self, intent: &ProtectionIntent) -> Result<(), WinError> {
            let hwnd = intent.window.0 as usize as HWND;
            unsafe {
                if IsWindow(hwnd) == 0 {
                    return Err(WinError::Other(format!(
                        "window {} no longer exists",
                        intent.window
                    )));
                }
            }
            match intent.mode {
                ProtectionMode::Surface => {
                    self.protected
                        .insert(intent.window, (intent.pid, ProtectionMode::Surface));
                    Ok(())
                }
                ProtectionMode::Exclusion => unsafe {
                    // Always query the actual process ID that owns the top-level
                    // HWND (e.g. ApplicationFrameHost.exe for UWP windows).
                    let mut owner_pid = 0u32;
                    GetWindowThreadProcessId(hwnd, &mut owner_pid);
                    if owner_pid == 0 {
                        owner_pid = intent.pid;
                    }

                    if owner_pid == GetCurrentProcessId() {
                        if SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) == 0 {
                            return Err(last_error("SetWindowDisplayAffinity"));
                        }
                        self.protected
                            .insert(intent.window, (owner_pid, ProtectionMode::Exclusion));
                        return Ok(());
                    }

                    // Mark the window property first so `veil_hook_win.dll` sees
                    // it on its very first poll pass.
                    let prop = wide(PROTECTED_PROP);
                    let _ = SetPropW(hwnd, prop.as_ptr(), 1 as HANDLE);

                    // Loading the helper does not establish protection until the
                    // owning process actually applies display affinity.
                    if let Err(err) = self.ensure_injected(owner_pid) {
                        let _ = RemovePropW(hwnd, prop.as_ptr());
                        return Err(err);
                    }
                    if Self::verify_affinity(hwnd, WDA_EXCLUDEFROMCAPTURE, Duration::from_secs(2)) {
                        self.protected
                            .insert(intent.window, (owner_pid, ProtectionMode::Exclusion));
                        return Ok(());
                    }
                    let _ = RemovePropW(hwnd, prop.as_ptr());
                    Err(WinError::Other(
                        "SetWindowDisplayAffinity was rejected by the target window".into(),
                    ))
                },
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
            let Some(&(pid, mode)) = self.protected.get(&window) else {
                return Ok(());
            };
            let hwnd = window.0 as usize as HWND;
            unsafe {
                if IsWindow(hwnd) == 0 {
                    self.protected.remove(&window);
                    return Ok(());
                }
                match mode {
                    ProtectionMode::StrictHide => {
                        ShowWindow(hwnd, SW_SHOW);
                    }
                    ProtectionMode::Exclusion => {
                        let prop = wide(PROTECTED_PROP);
                        let _ = RemovePropW(hwnd, prop.as_ptr());
                        let mut owner_pid = 0u32;
                        GetWindowThreadProcessId(hwnd, &mut owner_pid);
                        if owner_pid == 0 {
                            owner_pid = pid;
                        }
                        if owner_pid == GetCurrentProcessId() {
                            let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
                        } else if !Self::verify_affinity(hwnd, WDA_NONE, Duration::from_secs(2)) {
                            return Err(WinError::Other(
                                "Helper did not confirm exclusion was cleared".into(),
                            ));
                        }
                    }
                    ProtectionMode::Surface => {}
                }
            }
            self.protected.remove(&window);
            Ok(())
        }
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            let ids: Vec<WindowId> = self.protected.keys().copied().collect();
            for id in ids {
                let _ = self.release(id);
            }
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

    #[test]
    fn shell_classes_are_filtered() {
        assert!(is_shell_class("Progman"));
        assert!(is_shell_class("Shell_TrayWnd"));
        assert!(is_shell_class("MSCTFIME UI"));
        assert!(!is_shell_class("Chrome_WidgetWin_1"));
        assert!(!is_shell_class("Notepad"));
    }

    #[cfg(not(windows))]
    #[test]
    fn stub_reports_not_windows() {
        assert!(matches!(WinBackend::new(), Err(WinError::NotWindows)));
    }
}
