// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "VeilMac",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        // Static library consumed by `veil-protect-macos` through the C ABI in
        // `Sources/VeilMac/Exports.swift`.
        .library(name: "VeilMac", type: .static, targets: ["VeilMac"]),
        // Phase-0 command line spike. This is the executable that proves the
        // clean-API behaviour on a real Mac before the Rust backend claims it.
        .executable(name: "veilmac-spike", targets: ["veilmac-spike"]),
    ],
    targets: [
        .target(
            name: "VeilMac",
            path: "Sources/VeilMac"
        ),
        .executableTarget(
            name: "veilmac-spike",
            dependencies: ["VeilMac"],
            path: "Sources/veilmac-spike"
        ),
        .testTarget(
            name: "VeilMacTests",
            dependencies: ["VeilMac"],
            path: "Tests/VeilMacTests"
        ),
    ]
)
