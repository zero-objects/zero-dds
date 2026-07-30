# `ZeroddsIdlcPlugin` — SPM `BuildToolPlugin`

A real `BuildToolPlugin` (`Plugins/ZeroddsIdlcPlugin/plugin.swift`), the
native Swift Package Manager mechanism for pre-build code generation
(the same category as `swift-protobuf`'s and `grpc-swift`'s plugins).
Mirrors the Rust `zerodds-build` build.rs helper and CMake's
`zerodds_idlc_generate()` for the Swift/SPM ecosystem.

## Usage

```swift
// Package.swift
let package = Package(
    name: "my-app",
    dependencies: [
        .package(path: "path/to/build-integration/swift-spm")
        // or, once published: .package(url: "...", from: "1.0.0")
    ],
    targets: [
        .executableTarget(
            name: "MyApp",
            // NB: SwiftPM resolves a local (`path:`) dependency's identity
            // from its *directory name*, not the `name:` field inside its
            // Package.swift — if you copy this directory, `package:` below
            // must match whatever you name the directory (confirmed via
            // `swift package dump-package`; see sample-consumer/Package.swift).
            plugins: [.plugin(name: "ZeroddsIdlcPlugin", package: "swift-spm")]
        )
    ]
)
```

Drop `.idl` files into `Sources/MyApp/idl/`. `swift build` runs the plugin
before compiling `MyApp`, generating one `.swift` file per `.idl` into
the plugin's work directory — SPM tracks the declared `inputFiles`/
`outputFiles` per `BuildCommand` and only re-invokes `zerodds-idlc` when
an `.idl` changed.

`zerodds-idlc` is resolved from `PATH` (override with `ZERODDS_IDLC=/path/to/zerodds-idlc`).

## Known IDL-surface gaps this sample works around (not build-tool gaps)

Both verified by actually building this sample against a locally-built
`zerodds-idlc` (rc.6), not just read from source:

- `crates/idl-swift`'s emitter has no `Definition::Module` arm — a
  `module Foo { struct Bar {...} }` silently drops `Bar` (the same family
  finding as #21 in `internal/github-triage/2026-07-28/SUMMARY.md`, shared
  by 9 other backends: elixir/nim/d/julia/lua/ada/go/ocaml/zig). The
  sample `.idl` here is deliberately a **flat, unwrapped `struct Pose`**
  — the surface `idl-swift` actually supports today.
- `crates/idl-swift`'s `@key` → MD5 keyHash path (triggered once a
  struct's key members exceed 16 bytes, e.g. `@key string<32>`) emits
  `Insecure.MD5.hash(data: Data(b))` and prepends `import CryptoKit` but
  **not** `import Foundation` — `Data` is undefined, a real compile error
  in the generated file (`crates/idl-swift/src/emitter.rs:203-204` adds
  only the CryptoKit import). The sample `.idl` has no `@key` field,
  sidestepping the bug rather than masking it.

Both are codegen-crate bugs, out of this task's scope
(`crates/idl-*/src` is sibling-agent territory). Once fixed, this plugin
needs no changes — only the sample `.idl` could switch back to a
`module`-wrapped, `@key`-bearing form.

## Validated

`swift build` in `sample-consumer/` (Swift 6.3.3, SPM tools-version 5.9,
macOS/arm64, against a locally-built `zerodds-idlc` 1.0.0-rc.6):
generates `Robot.swift`, compiles, and `swift run` round-trips a `Pose`
through the generated `marshalXCDR`/`unmarshalXCDR` (40 wire bytes).
Re-running after `touch idl/Robot.idl` re-invokes `zerodds-idlc`;
running again unchanged skips it — SPM's own build-command
incrementality, confirmed by output diff, not assumed.

## What a fuller pass would add

- `#include` dependency tracking (same gap noted for the Gradle plugin).
- Multi-backend fan-out (today: one plugin invocation always passes
  `--swift`; a config file or target-name convention would be needed to
  make the backend configurable per target).
- Publishing to a Git tag / Swift Package Index — consumed today via a
  local `path:` dependency.
