# `ZeroddsIdlcPlugin` — SPM `BuildToolPlugin`

`Plugins/ZeroddsIdlcPlugin/plugin.swift` declares one build command per
`.idl` file of a target: `zerodds-idlc generate --lang swift --out <plugin
work dir> <file.idl>`, with the IDL file as input and `<stem>.swift` as
output, so SPM runs it only when the IDL file changed.

## Usage

```swift
// Package.swift
let package = Package(
    name: "my-app",
    dependencies: [
        .package(path: "path/to/build-integration/swift-spm"),
        // the wire runtime the generated code imports (`import ZdWire`)
        .package(path: "path/to/runtime/swift")
    ],
    targets: [
        .executableTarget(
            name: "MyApp",
            dependencies: [.product(name: "ZdWire", package: "swift")],
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

## Checked

`sample-consumer/` on macOS (Swift 6.3.3):

```bash
cd sample-consumer
ZERODDS_IDLC=<repo>/target/debug/zerodds-idlc swift build
swift run
# OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (40 wire bytes), robot_id=r2d2
```

A second `swift build` does not run `zerodds-idlc`; after
`touch Sources/RobotDemo/idl/Robot.idl` it does.

## Not covered

- An `#include`d IDL file is not an input of the build command.
- The language is fixed to Swift.
- Consumed by `path:`; not published.
