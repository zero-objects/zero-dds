# ZeroDDS for C#

C# binding over the ZeroDDS C API (`libzerodds`) by P/Invoke, NativeAOT
compatible, with the IDL4-C# runtime. A .NET 8 solution, not a Cargo package.
Part of [**ZeroDDS**](../../README.md).

| Project | Content |
|---|---|
| `csharp/ZeroDDS` | the binding (NuGet package `ZeroDDS`) |
| `csharp/ZeroDDS.Cdr` | the `IDdsTopicType<T>` contract a generated `*TypeSupport` companion implements, bridged into `ZeroDDS`'s typed-topic path (`Typed.cs`) |
| `csharp/ZeroDDS.Tests` | xUnit tests of the binding over `libzerodds` |

Types come from `zerodds-idlc generate --lang c#`, which also emits the
`*TypeSupport` companion (`tools/idlc/src/csharp_support.rs`). The actual
XCDR2 codec is `runtime/csharp` (`ZeroDds.Wire.Writer`/`Reader`) — the
companion calls it directly. There is no second, hand-written codec in
this crate to keep in sync with it.

## Tests

```bash
cargo build -p zerodds-c-api
LD_LIBRARY_PATH=target/debug python3 tests/integration/with_domains.py 1 -- \
    dotnet test crates/cs/csharp/ZeroDDS.Tests   # .NET SDK 9 (global.json)
```

The merge job `merge-cs` (`ci/integration_scenarios.py`) runs them when a file
under `crates/cs` or `crates/zerodds-c-api` changes, each test by name
(`ci/foreign_tests.py`). The C# wire runtime has its self-test in
`runtime/csharp/run-tests.sh` (job `integration-bindings-dotnet`).
