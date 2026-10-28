# Build-tool integration — `zerodds-idlc` as a build step

Each subdirectory runs `zerodds-idlc generate --lang <lang> --out <dir>
<file.idl>` from one ecosystem's build tool, with a `sample-consumer/`
that generates `Robot::Pose` (`module Robot { @appendable struct Pose {
@key string<32> robot_id; double x, y, theta; }; };`), compiles it
against the language's wire runtime in `runtime/<lang>/` and round-trips
one sample.

Rust (`crates/zerodds-build`) and CMake (`cmake/zerodds_idlc_generate.cmake`)
are outside this directory.

| Ecosystem | Form | Runtime the sample adds | Checked |
|---|---|---|---|
| Java, Gradle | `Plugin<Project>` + task (`@InputFiles`/`@OutputDirectory`) | `runtime/java` sources, `org.zerodds:omgdds` | not built: no Gradle on either host |
| Java, Maven | `exec-maven-plugin` + `build-helper-maven-plugin` | `runtime/java` sources, `org.zerodds:omgdds` | macOS, Maven 3.9.9: round-trip OK |
| C#, MSBuild | `.targets` file, `Inputs`/`Outputs` | `runtime/csharp` (`ZeroDds.Wire`), `ZeroDDS.Cdr` | macOS, dotnet 9.0.106: round-trip OK, skips unchanged |
| Elixir | `Mix.Task.Compiler` with manifest | `runtime/elixir` (`:zerodds_wire`) | macOS, Elixir 1.20.2: round-trip OK, skips unchanged |
| Swift | SPM `BuildToolPlugin` | `runtime/swift` (`ZdWire`) | macOS, Swift 6.3.3: round-trip OK, skips unchanged |
| Nim | Nimble `before build` | `runtime/nim` via `config.nims` | macOS, Nim 2.2.10: round-trip OK |
| D | dub `preGenerateCommands` | `runtime/d/zdwire.d` | the Linux test host, LDC 1.40.0: round-trip OK |
| Julia | `deps/build.jl` | `runtime/julia/zerodds_wire.jl` | macOS, Julia 1.12.6: `Pkg.build()` + round-trip OK |
| Lua | LuaRocks `build.type = "command"`, `build.lua` | `runtime/lua` | macOS, Lua 5.5.0, LuaRocks 3.13.0: round-trip OK |
| Ada | `Makefile` around `gprbuild` (a `.gpr` has no pre-build step) | `runtime/ada/src` | the Linux test host, GPRBUILD 18.0w: round-trip OK |

Every round-trip above wrote 40 bytes. Java and Ada write fixed file names
for every input (`ZdTypes.java` …, `zd_types.ads`/`.adb`); a second IDL
file into the same `--out` in one run is refused.

None of the integrations tracks an `#include`d IDL file; the CMake
function does, through `--depfile`.
