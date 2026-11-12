// C ABI consumed by the Rust crate `veil-protect-macos`.
//
// Keep this surface deliberately small and boring: integers in, integers out,
// one heap string for the last error. No Swift types, no callbacks, no
// Objective-C runtime poking from Rust.

import CoreGraphics
import Foundation

private let errorLock = NSLock()
private var lastErrorStorage: String = ""

private func setLastError(_ message: String) {
    errorLock.lock()
    lastErrorStorage = message
    errorLock.unlock()
}

private func clearLastError() {
    setLastError("")
}

/// `0` on success, non-zero on failure. Retrieve the message with
/// `veil_macos_last_error`.
@_cdecl("veil_macos_hide_window")
public func veilMacosHideWindow(_ window: UInt64) -> Int32 {
    clearLastError()
    do {
        try VeilMac.hideWindow(id: UInt32(truncatingIfNeeded: window))
        return 0
    } catch {
        setLastError(error.localizedDescription)
        return 1
    }
}

@_cdecl("veil_macos_show_window")
public func veilMacosShowWindow(_ window: UInt64) -> Int32 {
    clearLastError()
    do {
        try VeilMac.showWindow(id: UInt32(truncatingIfNeeded: window))
        return 0
    } catch {
        setLastError(error.localizedDescription)
        return 1
    }
}

@_cdecl("veil_macos_last_error")
public func veilMacosLastError() -> UnsafeMutablePointer<CChar>? {
    errorLock.lock()
    let message = lastErrorStorage
    errorLock.unlock()
    return strdup(message)
}

@_cdecl("veil_macos_free_string")
public func veilMacosFreeString(_ pointer: UnsafeMutablePointer<CChar>?) {
    free(pointer)
}

@_cdecl("veil_macos_accessibility_trusted")
public func veilMacosAccessibilityTrusted() -> Int32 {
    VeilMac.isAccessibilityTrusted() ? 1 : 0
}

@_cdecl("veil_macos_request_accessibility")
public func veilMacosRequestAccessibility() -> Int32 {
    VeilMac.requestAccessibility() ? 1 : 0
}

/// Capture the display synchronously for the Rust verifier.
///
/// On success returns a malloc'd BGRA buffer (caller frees with
/// `veil_macos_free_buffer`) and writes dimensions to `outWidth`/`outHeight`.
/// On failure returns NULL.
@_cdecl("veil_macos_capture_display")
public func veilMacosCaptureDisplay(
    _ displayID: UInt32,
    _ outWidth: UnsafeMutablePointer<UInt32>?,
    _ outHeight: UnsafeMutablePointer<UInt32>?
) -> UnsafeMutablePointer<UInt8>? {
    clearLastError()
    let semaphore = DispatchSemaphore(value: 0)
    var result: Result<(Data, Int, Int), Error>?
    VeilMac.captureDisplay(displayID: CGDirectDisplayID(displayID)) { outcome in
        result = outcome
        semaphore.signal()
    }
    // ScreenCaptureKit often needs more than a second on a cold start.
    if semaphore.wait(timeout: .now() + 15) == .timedOut {
        setLastError("ScreenCaptureKit capture timed out")
        return nil
    }
    switch result {
    case .success(let (data, width, height)):
        let count = data.count
        let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: count)
        data.copyBytes(to: buffer, count: count)
        outWidth?.pointee = UInt32(width)
        outHeight?.pointee = UInt32(height)
        return buffer
    case .failure(let error):
        setLastError(error.localizedDescription)
        return nil
    case .none:
        setLastError("ScreenCaptureKit returned no result")
        return nil
    }
}

@_cdecl("veil_macos_free_buffer")
public func veilMacosFreeBuffer(_ pointer: UnsafeMutablePointer<UInt8>?) {
    pointer?.deallocate()
}

/// Inventory as JSON, consumed by `veil-protect-macos`. Returns a malloc'd
/// UTF-8 string owned by the caller (`veil_macos_free_string`).
@_cdecl("veil_macos_list_windows_json")
public func veilMacosListWindowsJson() -> UnsafeMutablePointer<CChar>? {
    let windows = VeilMac.listWindows(includeOffscreen: false)
    let encoder = JSONEncoder()
    guard let data = try? encoder.encode(windows),
        let text = String(data: data, encoding: .utf8)
    else {
        return strdup("[]")
    }
    return strdup(text)
}
