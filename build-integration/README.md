# Build-tool integration — cross-language `zerodds-idlc` build steps

D.1 follow-up. D1 designed the "run `zerodds-idlc` as a build step"
pattern and implemented it for five ecosystems (Rust — `crates/zerodds-build`,
C++ — `cmake/zerodds_idlc_generate.cmake`, Go — `//go:generate`, OCaml —
dune `(rule)`, Zig — a build step) plus a documented (but never
build-validated) Maven `exec-maven-plugin` snippet
(`documentation/04-idl/idlc-handbook.md` §4.5). This directory adds the
remaining nine: Java (Gradle + Maven), C#, Elixir, Swift, Nim, D, Julia,
Lua, Ada.

Each subdirectory is `<ecosystem>/` with a `README.md`, the
plugin/hook/target artifact, and a `sample-consumer/` that takes one
`.idl`, runs the build step, and compiles/uses the generated type.

## Ecosystem × form × validation

| Ecosystem | Integration form | Sample builds? | Validated where |
|---|---|---|---|
| Java (Gradle) | native `Plugin<Project>` + `Task` (`@InputFiles`/`@OutputDirectory`) | generation confirmed via CLI; full `./gradlew build` not run | not validated — no `gradle` on either machine; **pending central serial validation** |
| Java (Maven) | thin `exec-maven-plugin` (matches documented pattern) + `build-helper-maven-plugin` | generates + compiles up to a discovered runtime gap | **validated locally** (macOS, Maven 3.9.9) — generation + incrementality work; compile blocked by missing `org.zerodds.types`/`org.omg.dds.topic.TopicType<T>` (see finding below) |
| C# (MSBuild) | native `.targets` file, `Inputs`/`Outputs` | generates; full compile blocked by discovered runtime bug | **validated locally** (macOS, dotnet 9.0.106) — generation + incrementality (confirmed via mtime diff across 3 runs) work; compile blocked by a runtime namespace collision (see finding below) |
| Elixir | real `Mix.Task.Compiler` | generation-only checked | generation confirmed via CLI on this machine; full `mix compile` **pending central serial validation** (only `elixir`/`mix` host was flagged overloaded mid-task) |
| Swift (SPM) | native `BuildToolPlugin` | **yes, full round-trip** | **validated locally** (macOS, Swift 6.3.3) — `swift build` + `swift run`, incrementality confirmed (regenerates on touch, skips unchanged) |
| Nim | native `nimble` `before build` hook | generation-only checked | generation confirmed via CLI; full `nimble run` **pending central serial validation** (nim not installed on either machine) |
| D | native dub `preGenerateCommands` | generation-only checked | generation confirmed via CLI; full `dub run` **pending central serial validation** (no D toolchain on either machine) |
| Julia | native `deps/build.jl` | generation-only checked | generation confirmed via CLI; full run **pending central serial validation** (julia not installed on either machine) |
| Lua | LuaRocks `build.type = "command"` + plain `build.lua` fallback | **yes, full round-trip** | **validated locally** (macOS, Lua 5.5.0) — `lua build.lua && lua app.lua`, incrementality confirmed (skip-if-unchanged message on rerun); LuaRocks form itself not exercised (LuaRocks not installed) |
| Ada | **no native GPR hook exists** — `Makefile` wrapper (loudly flagged) | generation-only checked | generation confirmed via CLI; full `make run` **pending central serial validation** (gprbuild only available on the host that was flagged overloaded) |

"Generation-only checked" = a real `zerodds-idlc generate --<backend>`
was run against the exact sample `.idl` (not read from source) to
confirm the emitted API surface (type/function names, field casing,
endianness constants) matches what the sample's hand-written consumer
code calls — every such check is cited with the actual generated line
in the relevant README. What was **not** independently exercised for
those five is the build-tool hook itself (`nimble before build`, `dub
preGenerateCommands`, `deps/build.jl`, `Mix.Task.Compiler`, the Ada
`Makefile`) end to end through its native invocation (`nimble run`,
`dub run`, etc.) — flagged per-README, not claimed.

## Why a mid-task pivot to "local + generation-checks only"

The shared Linux validation host (`codepit`) was flagged by the
coordinator as overloaded mid-task ("codepit is overloaded and must be
freed up NOW"). All further codepit builds/installs were stopped
immediately (an in-flight `apt-get install ldc dub gprbuild lua5.4` and
an in-flight `cargo build --release -p zerodds-idlc` were killed; no new
codepit work was started after the flag). Before the flag, codepit
usage was: (1) building a release `zerodds-idlc` (killed, unfinished —
superseded by a locally-built macOS debug binary and an existing
pre-built Linux debug binary from another agent's already-completed
checkout, used read-only for generation smoke-tests), (2) a handful of
lightweight `zerodds-idlc generate` invocations (not builds) to verify
the exact generated API surface per backend — cheap, already complete,
not repeated.

## Load-bearing findings from this pass (not build-tool bugs — flagged, not fixed)

1. **`Definition::Module` gap, 7 of the 9 backends in this task's scope**
   (elixir/swift/nim/d/julia/lua/ada — matches finding #21's family in
   `internal/github-triage/2026-07-28/SUMMARY.md`). Confirmed by
   generating from a `module Robot { struct Pose {...} }` IDL and
   finding zero occurrences of `Pose` in the output for all seven,
   versus a flat (unwrapped) `struct Pose` generating correctly. Every
   affected sample's `.idl` is deliberately flat, not module-wrapped —
   proving the *build-step plumbing*, not papering over the codegen
   gap. `crates/idl-*/src` is out of this task's scope (sibling-agent
   territory).
2. **C# runtime collision** (`build-integration/csharp-msbuild/README.md`):
   generated C# needs both `Omg.Types` (`ITopicType<T>`, its own
   `ExtensibilityKind`) and `ZeroDDS.Cdr` (`IDdsTopicType<T>`, a
   *different* `ExtensibilityKind`) — compiling both together (which the
   generated code requires) is a `CS0104` ambiguous-reference error.
   `crates/cs/Examples/TopicTypedSmoke.csproj` never hit this because it
   only references `ZeroDDS.Cdr`, not `Omg.Types.cs`.
3. **Java runtime gap** (`build-integration/java-maven/README.md`):
   generated Java references `org.zerodds.types.{Extensibility,Key}` and
   `org.omg.dds.topic.TopicType<T>`, neither of which exists anywhere in
   the repository (confirmed by repo-wide grep). `org.zerodds.cdr` and
   `org.omg.dds.topic.TopicTypeSupport` do exist (`crates/java-omgdds/java`)
   and resolve correctly once added as a dependency, narrowing the gap
   to exactly those two missing symbols.
4. **Swift `@key` → MD5 missing import** (`build-integration/swift-spm/README.md`):
   `crates/idl-swift/src/emitter.rs:203-204` adds `import CryptoKit` for
   the MD5 keyHash path but not `import Foundation`, so `Data` is
   undefined — a real compile error, confirmed by actually building the
   sample with a `@key string<32>` field before removing it.

All four are runtime-library/codegen-emitter completeness gaps, not
build-tool-integration bugs — the build-step wiring is correct in every
case (confirmed by reaching genuine downstream compile errors, not
build-step failures). Out of this task's scope to fix.

## Files added

See each `<ecosystem>/` subdirectory. Top level: this file,
`java-gradle/`, `java-maven/`, `csharp-msbuild/`, `elixir-mix/`,
`swift-spm/`, `nim-nimble/`, `d-dub/`, `julia-build/`, `lua-build/`,
`ada-gprbuild/`.
