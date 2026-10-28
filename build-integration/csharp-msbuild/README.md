# MSBuild `.targets` file

`ZeroDDS.Idlc.targets` (`targets/ZeroDDS.Idlc.targets`) — a real MSBuild
target, `ZeroddsIdlcGenerate`, with `Inputs="@(ZeroddsIdl)"` /
`Outputs="@(ZeroddsIdl->'...')"` for MSBuild's own incremental-build
engine, `BeforeTargets="BeforeCompile;CoreCompile"` so it runs ahead of
compilation, and a second target that splices the generated `.cs` files
into `@(Compile)`. Mirrors `zerodds-build` (Rust, `crates/zerodds-build`)
and CMake's `zerodds_idlc_generate()` for the MSBuild/.NET ecosystem.

## Usage

```xml
<Import Project="path/to/build-integration/csharp-msbuild/targets/ZeroDDS.Idlc.targets" />

<ItemGroup>
  <ZeroddsIdl Include="idl/Robot.idl" />
</ItemGroup>
```

The target runs `zerodds-idlc generate --lang $(ZeroddsIdlcBackend) --out
<dir> <file.idl>` once per `ZeroddsIdl` item. Properties:

| Property | Default |
|---|---|
| `ZeroddsIdlcExecutable` | `zerodds-idlc` from `PATH` |
| `ZeroddsIdlcBackend` | `csharp` |
| `ZeroddsIdlcOutputDir` | `$(IntermediateOutputPath)zerodds-idlc\`; `IntermediateOutputPath` is not yet set where the file is imported, so the files land in `zerodds-idlc/` next to the project |

Generated C# needs two runtime assemblies: `runtime/csharp/ZeroDds.Wire.csproj`
(codec) and `crates/cs/csharp/ZeroDDS.Cdr/ZeroDDS.Cdr.csproj` (`EndianMode`,
`IDdsTopicType<T>`, and `Omg.Types`, which it compiles in). Referencing
`crates/idl-csharp/runtime/Omg.Types.cs` a second time gives two
`Omg.Types.ExtensibilityKind` types and error CS0738.

## Checked

`sample-consumer/` (dotnet SDK 9.0.106, `net8.0`):

```bash
cd build-integration/csharp-msbuild/sample-consumer
dotnet build -p:ZeroddsIdlcExecutable=<repo>/target/debug/zerodds-idlc
DOTNET_ROLL_FORWARD=Major dotnet run --no-build
# OK: Robot.Pose round-tripped through generated PoseTypeSupport (40 wire bytes), Robot_id=r2d2
```

After `touch idl/Robot.idl` the next build runs `zerodds-idlc` again; a
build without a change does not (MSBuild's `Inputs`/`Outputs` check).

## Not covered

- An `#include`d IDL file is not an input of the target: changing only it
  does not regenerate. `zerodds_idlc_generate()` in CMake reads the
  depfile for that.
- No NuGet packaging of the target; it is imported by relative path.
