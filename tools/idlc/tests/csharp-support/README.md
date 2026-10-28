# Fresh C# support consumer

This is a second-assembly consumer of the current CLI's generated declarations
and TypeSupport companion. Generated code is not checked into this fixture.
Use the compiler, managed runtimes and native library from the same source
candidate.

From the ZeroDDS root, with a built CLI and .NET SDK available:

```sh
generated_support="$(mktemp -d)"
./target/debug/zerodds-idlc generate --lang 'c#' --encoding xcdr2 \
  --out "$generated_support" tools/idlc/tests/csharp-support/support.idl
dotnet run --project tools/idlc/tests/csharp-support/Consumer.csproj \
  -p:GeneratedDirectory="$generated_support"
```

The codec-only invocation checks registry resolution, metadata presence,
independent LE/BE octets, key padding and MD5 rules, optional values, empty
aggregates and malformed-input rejection. The project treats warnings as
errors and references the actual managed wire runtime and DDS binding.

Passing `-- --domain 177` additionally runs native DDS API delivery. Make
the matching built native library available to the platform loader first.
This path requires the current `typed_wire` and keyed-write entry points.
It checks separate keys and stable reader instance identity when non-key
data changes, as well as zero-byte valid samples. A failed native invocation
must not be reported as a successful codec-only invocation.

The native path is not a cross-vendor measurement. Registration, lifecycle
and resource-limit guarantees require their respective checks beyond this
consumer. This document describes the executable contract, not a claim that
every current source revision has already passed.

See [Program.cs](Program.cs), [the CLI contract](../../README.md), and the
[larger C# example](../../../../zerodds-examples/idl-conformance/csharp/README.md).
