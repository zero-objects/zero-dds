// swift-tools-version:5.9
// SPDX-License-Identifier: Apache-2.0
import PackageDescription

let package = Package(
    name: "Zerodds",
    platforms: [.macOS(.v12)],
    targets: [
        .target(name: "Zerodds"),
        .executableTarget(name: "ZeroddsExample", dependencies: ["Zerodds"]),
        .executableTarget(name: "ZeroddsExampleSync", dependencies: ["Zerodds"]),
        .executableTarget(name: "ZeroddsExampleAsync", dependencies: ["Zerodds"]),
        .testTarget(name: "ZeroddsTests", dependencies: ["Zerodds"]),
    ]
)
