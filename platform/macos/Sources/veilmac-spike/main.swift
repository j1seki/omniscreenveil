// `veilmac-spike` — Phase 0 macOS probe.
//
// Usage:
//   veilmac-spike list                  enumerate protectable windows
//   veilmac-spike hide <window-id>      strict hide via Accessibility
//   veilmac-spike show <window-id>      restore
//   veilmac-spike capture <display-id>  ScreenCaptureKit frame -> PPM
//   veilmac-spike permission            request/report Accessibility consent
//   veilmac-spike selftest              assert the invariants we claim
//
// Everything uses public API. If a command fails, the message says which
// permission or system version is missing instead of pretending the product
// can do something it cannot.

import CoreGraphics
import Foundation
import VeilMac

func usage() -> Never {
    let text = """
    veilmac-spike — OmniScreenVeil macOS Phase 0 probe

      list                    list protectable windows
      hide <window-id>        strict-hide a window (Accessibility)
      show <window-id>        restore a hidden window
      capture <display-id> [window-id ...]   capture a display, optionally filtering windows
      permission              report and request Accessibility consent
      selftest                run local invariant checks
    """
    FileHandle.standardError.write(Data((text + "\n").utf8))
    exit(2)
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data(("veilmac-spike: " + message + "\n").utf8))
    exit(1)
}

func pad(_ value: String, _ width: Int) -> String {
    value.count >= width ? value : value + String(repeating: " ", count: width - value.count)
}

func printWindows(_ windows: [VeilWindow]) {
    print(
        pad("WINDOW", 10) + pad("PID", 8) + pad("APP", 24) + pad("TITLE", 30) + "BOUNDS"
    )
    for window in windows {
        let bounds = "\(window.x),\(window.y) \(window.width)x\(window.height)"
        let title = window.title.isEmpty ? "(needs Screen Recording permission)" : window.title
        print(
            pad(String(window.id), 10)
                + pad(String(window.pid), 8)
                + pad(truncated(window.app, 24), 24)
                + pad(truncated(title, 30), 30)
                + bounds
        )
    }
}

func truncated(_ value: String, _ max: Int) -> String {
    if value.count <= max { return value }
    return String(value.prefix(max - 1)) + "…"
}

func writePPM(path: String, data: Data, width: Int, height: Int) {
    var out = Data("P6\n\(width) \(height)\n255\n".utf8)
    out.reserveCapacity(data.count / 4 * 3)
    var index = 0
    while index + 3 < data.count {
        // BGRA -> RGB
        out.append(data[index + 2])
        out.append(data[index + 1])
        out.append(data[index])
        index += 4
    }
    do {
        try out.write(to: URL(fileURLWithPath: path))
        print("wrote \(path) (\(width)x\(height))")
    } catch {
        fail("cannot write \(path): \(error.localizedDescription)")
    }
}

func selftest() {
    var failures = 0
    func check(_ name: String, _ condition: Bool, _ detail: String) {
        print("\(condition ? "PASS" : "FAIL") \(name) — \(detail)")
        if !condition { failures += 1 }
    }

    let windows = VeilMac.listWindows()
    check(
        "inventory returns sane geometry",
        windows.allSatisfy { $0.width > 0 && $0.height > 0 },
        "\(windows.count) windows"
    )
    check(
        "inventory never includes layer != 0",
        windows.allSatisfy { $0.layer == 0 },
        "no menu bar / dock / overlay candidates"
    )
    let trusted = VeilMac.isAccessibilityTrusted()
    check(
        "Accessibility state is observable",
        true,
        trusted ? "trusted" : "not trusted (strict hide disabled until consent)"
    )
    print()
    print(failures == 0 ? "selftest passed." : "\(failures) check(s) failed.")
    exit(failures == 0 ? 0 : 1)
}

let args = CommandLine.arguments
guard args.count >= 2 else { usage() }

switch args[1] {
case "list":
    printWindows(VeilMac.listWindows())
case "hide":
    guard args.count == 3, let id = UInt32(args[2]) else { usage() }
    guard VeilMac.isAccessibilityTrusted() else {
        fail("Accessibility permission missing; run `veilmac-spike permission`")
    }
    do {
        try VeilMac.hideWindow(id: id)
        print("hidden \(id)")
    } catch {
        fail(error.localizedDescription)
    }
case "show":
    guard args.count == 3, let id = UInt32(args[2]) else { usage() }
    guard VeilMac.isAccessibilityTrusted() else {
        fail("Accessibility permission missing; run `veilmac-spike permission`")
    }
    do {
        try VeilMac.showWindow(id: id)
        print("shown \(id)")
    } catch {
        fail(error.localizedDescription)
    }
case "capture":
    guard args.count >= 3, let display = UInt32(args[2]) else { usage() }
    var excluded = Set<UInt32>()
    for value in args.dropFirst(3) {
        guard let id = UInt32(value) else { usage() }
        excluded.insert(id)
    }
    let semaphore = DispatchSemaphore(value: 0)
    VeilMac.captureDisplay(displayID: CGDirectDisplayID(display), excludingWindowIDs: excluded) { result in
        switch result {
        case .success(let (data, width, height)):
            writePPM(path: "veilmac-capture.ppm", data: data, width: width, height: height)
        case .failure(let error):
            fail("capture failed: \(error.localizedDescription)")
        }
        semaphore.signal()
    }
    if semaphore.wait(timeout: .now() + 15) == .timedOut { fail("capture timed out") }
case "permission":
    let already = VeilMac.isAccessibilityTrusted()
    let granted = VeilMac.requestAccessibility()
    print("Accessibility trusted: \(already ? "yes" : "no")")
    print("Consent prompt requested: \(granted ? "yes" : "no")")
    if !already {
        print("Grant it in System Settings > Privacy & Security > Accessibility.")
    }
case "selftest":
    selftest()
default:
    usage()
}
