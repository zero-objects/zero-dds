// swift-tools-version:5.9
// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Swift wire runtime as a SwiftPM package. The probe driver does
// not build through SwiftPM — it hands `Sources/ZdWire/*.swift` to
// `swiftc` beside the generated `main.swift`, so the two compile as one
// module and the generated code names `ZdWriter` unqualified. The
// package exists so a consumer that wants the runtime as a library can
// take it the ordinary way.
import PackageDescription

let package = Package(
    name: "ZdWire",
    products: [
        .library(name: "ZdWire", targets: ["ZdWire"])
    ],
    targets: [
        .target(name: "ZdWire", path: "Sources/ZdWire"),
        .testTarget(name: "ZdWireTests", dependencies: ["ZdWire"], path: "Tests/ZdWireTests"),
    ],
    swiftLanguageVersions: [.v5]
)
