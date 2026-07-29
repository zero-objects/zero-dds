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

## Validated

Against a locally-built `zerodds-idlc` 1.0.0-rc.6 (dotnet SDK 9.0.106,
`net8.0` target):

- **Generation + incrementality: validated, works.** `dotnet build
  /t:ZeroddsIdlcGenerate` runs `zerodds-idlc generate idl/Robot.idl
  --csharp -o zerodds-idlc/`. Confirmed by `stat` on the output file's
  mtime across three runs: touching `idl/Robot.idl` changes the output
  mtime (regeneration happened); rebuilding again unchanged leaves the
  mtime untouched (MSBuild's own `Inputs`/`Outputs` staleness check
  skipped the `Exec`, not a lucky no-op `zerodds-idlc` run — confirmed
  via `-v:normal`, no `zerodds-idlc generate` line on the second run).
- **Full `dotnet build` (compile): blocked by a discovered, pre-existing
  runtime bug — not a build-integration bug, not silently worked around.**

### The blocking bug (out of scope: runtime libraries, not build tooling)

The generated `Robot.cs` needs two runtime pieces that, before this pass,
**no `.csproj` in the repo referenced together** (confirmed via a
repo-wide grep for both file names across `.csproj`):

1. `crates/cs/csharp/ZeroDDS.Cdr/ZeroDDS.Cdr.csproj` — `Xcdr2Writer`,
   `Xcdr2Reader`, `EndianMode`, and its own `IDdsTopicType<T>` +
   `ExtensibilityKind` (`ZeroDDS.Cdr/src/IDdsTopicType.cs`,
   `ZeroDDS.Cdr/src/ExtensibilityKind.cs`).
2. `crates/idl-csharp/runtime/Omg.Types.cs` — a bare `.cs` file (no
   `.csproj` of its own), "shipped alongside generated sources" per its
   own header comment. Defines `ITopicType<T>`, `[Extensibility(...)]`,
   `[Key]`, and **its own separate** `ExtensibilityKind`.

The generated `Robot.cs` itself does `using Omg.Types; using
ZeroDDS.Cdr;` and needs symbols from *both* — the `Pose` record
implements `Omg.Types.ITopicType<Pose>` (line 12), while
`PoseTypeSupport` implements `ZeroDDS.Cdr.IDdsTopicType<Pose>` (line 21).
Compiling both runtime pieces together — which the generated code
requires — fails:

```
error CS0104: "ExtensibilityKind" ist ein mehrdeutiger Verweis zwischen
"Omg.Types.ExtensibilityKind" und "ZeroDDS.Cdr.ExtensibilityKind".
error CS0738: "PoseTypeSupport" implementiert den Schnittstellenmember
"IDdsTopicType<Pose>.Extensibility" nicht. "PoseTypeSupport.Extensibility"
hat nicht den entsprechenden Rückgabetyp "ExtensibilityKind" ...
```

Two independently-evolved runtime type systems (`Omg.Types.ITopicType<T>`
+ its `ExtensibilityKind`, vs `ZeroDDS.Cdr.IDdsTopicType<T>` + its own
different `ExtensibilityKind`) collide the moment both are compiled
together — which every consumer of `idl-csharp`-generated code that
needs both wire-format types (`Xcdr2Writer`/`Reader`) and the
topic-marker/attribute types must do. This means **`idl-csharp`'s
generated output was never actually build-validated end to end against
its own runtime before this pass** — `crates/cs/Examples/TopicTypedSmoke.csproj`
only references `ZeroDDS.Cdr` (not `Omg.Types.cs`), so it never hits
this collision.

This is a runtime-library coherence bug (`crates/cs/csharp/ZeroDDS.Cdr`
+ `crates/idl-csharp/runtime/Omg.Types.cs`), not a code-generation-emitter
bug (`crates/idl-csharp/src`) and not a build-tool-integration bug — the
`.targets` file, `Inputs`/`Outputs`, and `ProjectReference`/`Compile`
wiring in `sample-consumer/ZeroddsIdlcSample.csproj` are all correct;
the two runtime pieces they wire in are the ones that don't coexist.
Flagged here rather than fixed (touching either runtime piece is outside
this task's build-tool-integration scope) or silently worked around
(e.g. dropping one runtime reference would make the sample compile but
would stop proving the *actual* generated output builds).

## What a fuller pass would add

- A packaged MSBuild SDK/task (`Microsoft.Build.Utilities.Task`) instead
  of a plain `.targets` file — would give typed `[Required]` properties
  instead of raw MSBuild property/item syntax, and could shell out via
  the `ToolTask` base class (built-in `stdout`/`stderr` capture and
  cross-platform executable resolution) instead of a raw `<Exec>`.
- `#include` dependency tracking (same gap as the other integrations).
- NuGet packaging (`Microsoft.Build.Framework`
  `Sdk="ZeroDDS.Idlc.Sdk/1.0.0"` import) instead of a relative
  `<Import Project="...">` path.
