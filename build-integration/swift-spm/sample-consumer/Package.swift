// swift-tools-version:5.9
// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
import PackageDescription

let package = Package(
    name: "zerodds-idlc-plugin-sample",
    dependencies: [
        .package(path: "../")
    ],
    targets: [
        .executableTarget(
            name: "RobotDemo",
            plugins: [
                // SwiftPM resolves a local (`path:`) dependency's identity
                // from its directory name, not the `name:` in its
                // Package.swift (`zerodds-idlc-plugin`) — the directory
                // here is `swift-spm/`, so that is the identity to
                // reference. Confirmed via `swift package dump-package`.
                .plugin(name: "ZeroddsIdlcPlugin", package: "swift-spm")
            ]
        )
    ]
)
