# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- A reader of a topic whose `ITopicTraits` declare no key (`IsKeyed`, new, default `true`) is
  created through `zerodds_sub_create_datareader_kind` without a key; before, every untyped reader
  was keyed and matched no writer of an unkeyed topic (DDSI-RTPS 2.5 §9.3.1.2, Table 9.1).
- The QoS records default to DDS 1.4 §2.2.3: DEADLINE, LIFESPAN, the LIVELINESS lease and the two
  READER_DATA_LIFECYCLE delays infinite, RELIABILITY's `max_blocking_time` 100 ms,
  RESOURCE_LIMITS unlimited. A reader that requested a DEADLINE of zero matched no writer with the
  defaults.

### Changed

- `ZeroDDS.Tests` is an xUnit project: the checks of the former smoke program, sorted by B3
  (`rc8-test-suite.md`), as ten tests named after their clause, each in the DDS domain
  `tests/integration/with_domains.py` leases; the merge job `merge-cs` runs them by name against
  `libzerodds`. `build_cs_smoke.sh` and the nightly job `cs-smoke` are gone.

### Removed

- `ZeroDDS.Cdr.Tests`: its wire vectors called `ZeroDds.Wire`, the C# runtime whose self-test
  (`runtime/csharp/run-tests.sh`, job `integration-bindings-dotnet`) checks the same octets.

### Changed

- `QosBridge`: one conversion per policy (`R`, `DS`, `L`, `Pres`, `H`, `RL`, `EF`, `Bytes`) in place
  of the copies inside each `ToNative`; the native structs are filled as before.

### Changed

- The DataWriter and DataReader QoS struct layouts end in the `property` and `data_tags` members of the C ABI; the binding sets them empty.


### Changed
- `ZeroDDS.Cdr` (`csharp/ZeroDDS.Cdr/`) is now only the thin contract the
  typed-topic path bridges through — `IDdsTopicType<T>` (§2/§3),
  `IGeneratedWireTypeSupport`, `EndianMode` and `ExtensibilityKind` — plus
  the `Omg.Types` runtime it re-exports. It no longer carries a codec of
  its own: `zerodds-idlc --lang 'c#'` (`tools/idlc/src/csharp_support.rs`)
  emits a `*TypeSupport : IDdsTopicType<T>` companion per IDL `struct`/
  `union` that calls straight into `ZeroDds.Wire.Writer`/`Reader`
  (`runtime/csharp`) — the one C# XCDR2 codec, exercised by five other
  language columns besides this one. `ZeroDDS`'s `CreateTypedTopic<T>`
  (`Typed.cs`) resolves that companion through `TypeSupportRegistry`
  exactly as before; nothing in the typed-topic public API changed.

### Removed
- The hand-written duplicate codec this library carried alongside the
  generated one: `Xcdr2Writer`, `Xcdr2Reader`, `Md5` and `XcdrException`
  (all public `ZeroDDS.Cdr` types, never released) and the whole
  `ZeroDDS.Cdr.SourceGenerators` project (`[DdsType]`/`[DdsKey]`
  attribute-driven Roslyn generator, limited to flat primitive members and
  final-only extensibility). Both duplicated `runtime/csharp` byte for
  byte for the cases they covered and diverged from it for one they did
  not — see the `Xcdr2WireVectorsTests.V10_Mutable_MatchesWire` comment
  for the one case where the removed writer framed a `@mutable` type's
  `string` member differently: length code 4 with a separate NEXTINT
  (DHEADER 23, 27 bytes). XTypes 1.3 §7.4.3.4.2 permits that framing and
  length code 5, where the string's own length word is the NEXTINT;
  `ZeroDds.Wire.Writer` writes length code 5, the compact framing
  ZeroDDS, Fast DDS and Cyclone DDS emit (`docs/specs/zerodds-ast-1.0.md`
  §7.1), giving DHEADER 19, 23 bytes. The vector pins the compact form.
  A consumer that used `Xcdr2Writer`/`Xcdr2Reader` directly instead of
  through `IDdsTopicType<T>` now uses `ZeroDds.Wire.Writer`/`Reader`
  (`runtime/csharp`) instead; the method names differ (e.g. `WriteInt32`
  -> `WriteI32`, `.ToArray()` -> `.Finish()`) because that runtime is
  shared across bindings and predates this crate's naming.
- `csharp/ZeroDDS.Cdr.Tests/Md5Tests.cs` and `AlignmentTests.cs`: their
  subject (the removed `Md5`/`Xcdr2Reader`) is gone, and their coverage
  (RFC 1321 vectors, alignment/DHEADER/EMHEADER roundtrips) already exists
  against the real runtime in `runtime/csharp/tests/SelfTest.cs`.
  `Xcdr2WireVectorsTests` (V-1..V-12 from
  zerodds-xcdr2-bindings-conformance-1.0 §6) stays as the byte oracle,
  now calling `ZeroDds.Wire.Writer`/`Reader` directly.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the crate **`zerodds-cs`** as a Layer-6 PSM/binding.

### Spec references
- OMG DDS 1.4 §2.2.2 + DDS-PSM-Cxx 1.0 §7.5: PSM API surface
- ZeroDDS vendor spec `zerodds-c-api-1.0` (C-FFI foundation)

### Public API
See `README.md` + `src/lib.rs` doc comments + the associated coverage doc.

### Implementation
C# P/Invoke, NativeAOT-compatible, IDL4-C# runtime

### Architecture
- Layer: 6 (PSMs / Bindings)
- Dependencies (in): `zerodds-c-api` (Foundation) + language-specific helper crates
- Dependents (out): user code

### Stability
All `pub` items are RC1-stable; breaking changes require a major bump.
