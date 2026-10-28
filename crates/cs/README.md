# ZeroDDS for C#

C# binding over the ZeroDDS C API (`libzerodds`) by P/Invoke, NativeAOT
compatible, with the IDL4-C# runtime. A .NET 8 solution, not a Cargo package.
Part of [**ZeroDDS**](../../README.md).

| Project | Content |
|---|---|
| `csharp/ZeroDDS` | the binding (NuGet package `ZeroDDS`) |
| `csharp/ZeroDDS.Cdr` | the `IDdsTopicType<T>` contract a generated `*TypeSupport` companion implements, bridged into `ZeroDDS`'s typed-topic path (`Typed.cs`) |
| `csharp/ZeroDDS.Tests`, `ZeroDDS.Cdr.Tests` | tests |

Types come from `zerodds-idlc generate --lang c#`, which also emits the
`*TypeSupport` companion (`tools/idlc/src/csharp_support.rs`). The actual
XCDR2 codec is `runtime/csharp` (`ZeroDds.Wire.Writer`/`Reader`) — the
companion calls it directly. There is no second, hand-written codec in
this crate to keep in sync with it.

## Tests

```bash
crates/cs/csharp/build_cs_smoke.sh      # needs dotnet 8
```
