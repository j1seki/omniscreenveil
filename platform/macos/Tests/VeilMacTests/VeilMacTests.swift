import XCTest
import CoreGraphics
import Darwin
@testable import VeilMac

final class VeilMacTests: XCTestCase {
    func testInventoryHasSaneGeometry() {
        for window in VeilMac.listWindows() {
            XCTAssertGreaterThan(window.width, 0)
            XCTAssertGreaterThan(window.height, 0)
            XCTAssertNotEqual(window.app, "")
        }
    }

    func testInventoryOnlyNormalPlane() {
        for window in VeilMac.listWindows() {
            XCTAssertEqual(window.layer, 0, "overlays and the menu bar must be excluded")
        }
    }

    func testInventoryExcludesOwnProcess() {
        let ownPid = ProcessInfo.processInfo.processIdentifier
        let windows = VeilMac.listWindows()
        XCTAssertFalse(windows.contains { $0.pid == ownPid })
    }

    func testCapabilityMatrixIsHonest() {
        // The Swift layer must never grow a private exclusion API; this test is
        // the reminder. The Rust matrix asserts the same thing independently.
        let symbols = ["CGSSetWindowCaptureExcluded", "SLSSetWindowCaptureState"]
        for symbol in symbols {
            XCTAssertNil(dlsym(UnsafeMutableRawPointer(bitPattern: -2), symbol))
        }
    }
}
