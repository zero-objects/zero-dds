// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
import Foundation
import PackagePlugin

/// Runs `zerodds-idlc generate <idl> --swift -o <dir>` as a Swift Package
/// Manager pre-build command, once per `.idl` file found under the
/// consuming target's `idl/` directory. Emits one `BuildCommand` per
/// `.idl` with explicit `inputFiles`/`outputFiles`, giving SPM's own
/// incremental build graph the same skip-if-unchanged guarantee
/// `zerodds-build`'s `cargo:rerun-if-changed` gives Cargo — SPM only
/// re-runs the command when the declared input is newer than the
/// declared output.
@main
struct ZeroddsIdlcPlugin: BuildToolPlugin {
    func createBuildCommands(context: PluginContext, target: Target) async throws -> [Command] {
        guard let target = target as? SourceModuleTarget else {
            return []
        }

        let idlDir = target.directory.appending("idl")
        guard FileManager.default.fileExists(atPath: idlDir.string) else {
            return []
        }

        let idlFiles = try FileManager.default
            .contentsOfDirectory(atPath: idlDir.string)
            .filter { $0.hasSuffix(".idl") }
            .sorted()

        let outputDir = context.pluginWorkDirectory.appending("generated")
        try? FileManager.default.createDirectory(atPath: outputDir.string, withIntermediateDirectories: true)

        let executable = try zeroddsIdlcExecutable()

        return idlFiles.map { fileName in
            let idlPath = idlDir.appending(fileName)
            let stem = String(fileName.dropLast(".idl".count))
            let outputPath = outputDir.appending("\(stem).swift")

            return .buildCommand(
                displayName: "zerodds-idlc generate \(fileName) --swift",
                executable: executable,
                arguments: [
                    "generate", idlPath.string,
                    "--swift",
                    "-o", outputDir.string,
                ],
                inputFiles: [idlPath],
                outputFiles: [outputPath]
            )
        }
    }

    /// Resolves the `zerodds-idlc` executable. SPM plugins cannot declare
    /// a dependency on an externally-built (non-SPM, non-Xcode) binary via
    /// `context.tool(named:)` unless it is vendored as a `.binaryTarget` in
    /// this same package graph — `zerodds-idlc` is a plain Cargo binary,
    /// so this plugin instead does its own `PATH` search (the same
    /// fallback SwiftGen's and swift-protobuf's community build plugins
    /// use for non-SPM tools), overridable via `ZERODDS_IDLC` for CI
    /// runners that install it to a non-`PATH` location.
    private func zeroddsIdlcExecutable() throws -> Path {
        if let override = ProcessInfo.processInfo.environment["ZERODDS_IDLC"] {
            return Path(override)
        }
        let pathEnv = ProcessInfo.processInfo.environment["PATH"] ?? ""
        for dir in pathEnv.split(separator: ":") {
            let candidate = "\(dir)/zerodds-idlc"
            if FileManager.default.isExecutableFile(atPath: candidate) {
                return Path(candidate)
            }
        }
        throw ZeroddsIdlcPluginError.executableNotFound
    }
}

enum ZeroddsIdlcPluginError: Error, CustomStringConvertible {
    case executableNotFound

    var description: String {
        "zerodds-idlc not found on PATH and ZERODDS_IDLC is not set"
    }
}
