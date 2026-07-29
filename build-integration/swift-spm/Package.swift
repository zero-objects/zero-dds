// swift-tools-version:5.9
// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// A real SPM `BuildToolPlugin` (Plugins/ZeroddsIdlcPlugin) — mirrors the
// Rust `zerodds-build` build.rs helper (crates/zerodds-build) and the
// CMake `zerodds_idlc_generate()` function for the Swift/SPM ecosystem.
// Package name kept distinct from the consumer so the plugin can be
// referenced as a local package dependency the same way it would be
// referenced once published (`.package(url: "...", from: "1.0.0")`).
import PackageDescription

let package = Package(
    name: "zerodds-idlc-plugin",
    products: [
        .plugin(name: "ZeroddsIdlcPlugin", targets: ["ZeroddsIdlcPlugin"])
    ],
    targets: [
        .plugin(
            name: "ZeroddsIdlcPlugin",
            capability: .buildTool()
        )
    ]
)
