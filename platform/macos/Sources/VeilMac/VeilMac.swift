// VeilMac — clean-API macOS helpers for OmniScreenVeil.
//
// Ground rules, enforced by review and by the helpers below:
//
//   * AppKit / ApplicationServices / ScreenCaptureKit public API only.
//   * No private SkyLight/CGS symbols, no SIP changes, no code signing tricks.
//   * Filters only affect captures created by this process, never captures
//     made by another app. System-wide `Exclusion` remains unsupported. This
//     library provides is:
//       - real window inventory (CGWindowList + Accessibility),
//       - strict hide / show for a foreign window (Accessibility),
//       - a ScreenCaptureKit capture path used by the Veil Surface and by the
//         verification harness.
//
// The FFI surface at the bottom is intentionally tiny so the Rust side cannot
// grow accidental dependencies on private behaviour.

import AppKit
import ApplicationServices
import CoreGraphics
import Foundation
import ScreenCaptureKit

public struct VeilWindow: Codable {
    public let id: UInt32
    public let pid: Int32
    public let app: String
    public let bundle: String
    public let title: String
    public let x: Int
    public let y: Int
    public let width: Int
    public let height: Int
    public let onScreen: Bool
    public let layer: Int
}

public enum VeilMacError: LocalizedError {
    case notTrusted
    case noAccessibilityElement(pid: Int32)
    case windowNotFound(UInt32)
    case captureUnavailable(String)
    case axFailure(String)

    public var errorDescription: String? {
        switch self {
        case .notTrusted:
            return "Accessibility permission is required (System Settings > Privacy & Security > Accessibility)."
        case .noAccessibilityElement(let pid):
            return "No Accessibility element for pid \(pid)."
        case .windowNotFound(let id):
            return "Window \(id) was not found."
        case .captureUnavailable(let why):
            return "Screen capture unavailable: \(why)"
        case .axFailure(let what):
            return "Accessibility call failed: \(what)"
        }
    }
}

public enum VeilMac {
    /// Whether this process may use the Accessibility API.
    public static func isAccessibilityTrusted() -> Bool {
        AXIsProcessTrusted()
    }

    /// Ask macOS to show the Accessibility consent prompt. Safe to call
    /// repeatedly; it never blocks.
    @discardableResult
    public static func requestAccessibility() -> Bool {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true]
        return AXIsProcessTrustedWithOptions(options as CFDictionary)
    }

    /// Every top-level window of every process, excluding our own. With
    /// `includeOffscreen` (the FFI default) minimized windows are included too,
    /// which strict hide depends on.
    ///
    /// `CGWindowListCopyWindowInfo` is public API and returns geometry without
    /// any permission. Titles are only populated with Screen Recording consent;
    /// an empty title is normal and must not be treated as an error.
    public static func listWindows(
        includeOffscreen: Bool = false,
        includeTiny: Bool = false
    ) -> [VeilWindow] {
        let options: CGWindowListOption = includeOffscreen
            ? [.optionAll, .excludeDesktopElements]
            : [.optionOnScreenOnly, .excludeDesktopElements]
        guard
            let raw = CGWindowListCopyWindowInfo(options, kCGNullWindowID)
                as? [[String: Any]]
        else {
            return []
        }
        let ownPid = ProcessInfo.processInfo.processIdentifier
        var out: [VeilWindow] = []
        for entry in raw {
            guard
                let number = entry[kCGWindowNumber as String] as? NSNumber,
                let ownerPid = entry[kCGWindowOwnerPID as String] as? NSNumber,
                let boundsDict = entry[kCGWindowBounds as String] as? [String: Any],
                let rect = CGRect(dictionaryRepresentation: boundsDict as CFDictionary)
            else {
                continue
            }
            let pid = ownerPid.int32Value
            if pid == ownPid { continue }
            guard let layer = entry[kCGWindowLayer as String] as? NSNumber else { continue }
            // Layer 0 is the normal application plane. Menus, the Dock, the
            // menu bar and overlays live above it and must never be candidates.
            if layer.intValue != 0 { continue }
            if !includeTiny && (rect.width < 32 || rect.height < 32) { continue }
            let app = entry[kCGWindowOwnerName as String] as? String ?? "pid:\(pid)"
            let bundle = NSRunningApplication(processIdentifier: pid)?.bundleIdentifier ?? ""
            let title = entry[kCGWindowName as String] as? String ?? ""
            let onScreen = (entry[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue ?? false
            out.append(
                VeilWindow(
                    id: number.uint32Value,
                    pid: pid,
                    app: app,
                    bundle: bundle,
                    title: title,
                    x: Int(rect.origin.x.rounded()),
                    y: Int(rect.origin.y.rounded()),
                    width: Int(rect.width.rounded()),
                    height: Int(rect.height.rounded()),
                    onScreen: onScreen,
                    layer: layer.intValue
                )
            )
        }
        return out.sorted { $0.id < $1.id }
    }

    /// Minimal Accessibility tree walk to find the AXUIElement that owns a
    /// CGWindowID. macOS has no direct public mapping, so we match by
    /// window number when the system exposes one, else by frame + title.
    private static func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else {
            return nil
        }
        return value
    }

    private static func frame(of element: AXUIElement) -> CGRect? {
        guard let posRef = attribute(element, kAXPositionAttribute),
            let sizeRef = attribute(element, kAXSizeAttribute),
            CFGetTypeID(posRef) == AXValueGetTypeID(),
            CFGetTypeID(sizeRef) == AXValueGetTypeID()
        else {
            return nil
        }
        var point = CGPoint.zero
        var size = CGSize.zero
        guard AXValueGetValue(posRef as! AXValue, .cgPoint, &point),
            AXValueGetValue(sizeRef as! AXValue, .cgSize, &size)
        else {
            return nil
        }
        return CGRect(origin: point, size: size)
    }

    /// macOS has no public CGWindowID -> AXUIElement mapping, so this matches
    /// by `AXWindowNumber` when exposed, then by a unique title or frame match.
    /// It refuses to guess when the match is ambiguous: minimizing the wrong
    /// window is worse than reporting that the window could not be found.
    private static func axWindow(pid: Int32, windowID: UInt32, expected: VeilWindow?) throws
        -> AXUIElement
    {
        let app = AXUIElementCreateApplication(pid)
        guard let windows = attribute(app, kAXWindowsAttribute) as? [AXUIElement] else {
            throw VeilMacError.noAccessibilityElement(pid: pid)
        }
        var frameMatches: [AXUIElement] = []
        var titleMatches: [AXUIElement] = []
        for window in windows {
            if let number = attribute(window, "AXWindowNumber") as? NSNumber {
                if number.uint32Value == windowID { return window }
                // A known different ID must never be accepted by a heuristic.
                continue
            }
            guard let expected else { continue }
            let title = (attribute(window, kAXTitleAttribute) as? String) ?? ""
            if !expected.title.isEmpty && title == expected.title {
                titleMatches.append(window)
            }
            if let frame = frame(of: window) {
                let matchesFrame = Int(frame.origin.x.rounded()) == expected.x
                    && Int(frame.origin.y.rounded()) == expected.y
                    && Int(frame.size.width.rounded()) == expected.width
                    && Int(frame.size.height.rounded()) == expected.height
                if matchesFrame && (expected.title.isEmpty || title == expected.title) {
                    frameMatches.append(window)
                }
            }
        }
        if frameMatches.count == 1 { return frameMatches[0] }
        if titleMatches.count == 1 { return titleMatches[0] }
        throw VeilMacError.windowNotFound(windowID)
    }

    /// Strict hide: minimize the window's Accessibility element. The process
    /// keeps running and the window is restored by `show`.
    ///
    /// `AXHidden` is an application-level attribute, not a window attribute, so
    /// setting it on a window fails on macOS. `AXMinimized` is the documented
    /// per-window equivalent.
    public static func hideWindow(id: UInt32) throws {
        guard isAccessibilityTrusted() else { throw VeilMacError.notTrusted }
        let windows = listWindows(includeOffscreen: true, includeTiny: true)
        guard let target = windows.first(where: { $0.id == id }) else {
            throw VeilMacError.windowNotFound(id)
        }
        let element = try axWindow(pid: target.pid, windowID: id, expected: target)
        let result = AXUIElementSetAttributeValue(
            element, kAXMinimizedAttribute as CFString, kCFBooleanTrue
        )
        guard result == .success else {
            throw VeilMacError.axFailure("AXMinimized (error \(result.rawValue))")
        }
    }

    public static func showWindow(id: UInt32) throws {
        guard isAccessibilityTrusted() else { throw VeilMacError.notTrusted }
        let windows = listWindows(includeOffscreen: true, includeTiny: true)
        guard let target = windows.first(where: { $0.id == id }) else {
            // Window closed while hidden: nothing to restore.
            return
        }
        let element = try axWindow(pid: target.pid, windowID: id, expected: target)
        let result = AXUIElementSetAttributeValue(
            element, kAXMinimizedAttribute as CFString, kCFBooleanFalse
        )
        guard result == .success else {
            throw VeilMacError.axFailure("AXMinimized=false (error \(result.rawValue))")
        }
    }

    /// Capture one display through ScreenCaptureKit at native resolution.
    ///
    /// This is the path the Veil Surface and the verifier use. It returns BGRA
    /// bytes in `CGImage` row order (top-down) so the Rust compositor can treat
    /// it exactly like a Windows/X11 frame.
    public static func captureDisplay(
        displayID: CGDirectDisplayID,
        excludingWindowIDs: Set<UInt32> = [],
        completion: @escaping (Result<(Data, Int, Int), Error>) -> Void
    ) {
        Task {
            do {
                let content = try await SCShareableContent.excludingDesktopWindows(
                    false, onScreenWindowsOnly: true
                )
                let wanted = displayID == 0 ? CGMainDisplayID() : displayID
                guard let display = content.displays.first(where: { $0.displayID == wanted }) else {
                    throw VeilMacError.captureUnavailable("requested display is unavailable")
                }
                let available = Set(content.windows.map(\.windowID))
                let missing = excludingWindowIDs.subtracting(available)
                if !missing.isEmpty && listWindows(includeTiny: true).contains(where: {
                    missing.contains($0.id)
                }) {
                    throw VeilMacError.captureUnavailable(
                        "could not filter every protected window; withholding this frame"
                    )
                }
                let ownPID = ProcessInfo.processInfo.processIdentifier
                let filter = SCContentFilter(
                    display: display,
                    excludingWindows: content.windows.filter {
                        $0.owningApplication?.processID == ownPID
                            || excludingWindowIDs.contains($0.windowID)
                    }
                )
                let config = SCStreamConfiguration()
                config.width = display.width
                config.height = display.height
                config.pixelFormat = kCVPixelFormatType_32BGRA
                config.showsCursor = true

                if #available(macOS 14.0, *) {
                    let image = try await SCScreenshotManager.captureImage(
                        contentFilter: filter, configuration: config
                    )
                    let (data, w, h) = try bgra(from: image)
                    completion(.success((data, w, h)))
                } else {
                    throw VeilMacError.captureUnavailable(
                        "SCScreenshotManager requires macOS 14; use the SCStream path"
                    )
                }
            } catch {
                completion(.failure(error))
            }
        }
    }

    private static func bgra(from image: CGImage) throws -> (Data, Int, Int) {
        let width = image.width
        let height = image.height
        let bytesPerRow = width * 4
        var data = Data(count: bytesPerRow * height)
        let ok = data.withUnsafeMutableBytes { raw -> Bool in
            guard let base = raw.baseAddress,
                let ctx = CGContext(
                    data: base,
                    width: width,
                    height: height,
                    bitsPerComponent: 8,
                    bytesPerRow: bytesPerRow,
                    space: CGColorSpaceCreateDeviceRGB(),
                    bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
                        | CGBitmapInfo.byteOrder32Little.rawValue
                )
            else { return false }
            ctx.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard ok else {
            throw VeilMacError.captureUnavailable("could not allocate BGRA context")
        }
        return (data, width, height)
    }
}
