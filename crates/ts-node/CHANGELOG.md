# Changelog — `@zerodds/node`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The DataWriter and DataReader QoS struct layouts end in the `property` and `data_tags` members of the C ABI; the binding sets them empty.
- Tests: the lossless RELIABLE test writes fifty samples of one instance with KEEP_ALL on both
  sides; under the default KEEP_LAST(1) the writer and the reader keep the instance's most recent
  sample (DDS 1.4 §2.2.3.18), and one sample arrived. A second test holds what KEEP_LAST(1)
  promises: the stream in order, ending with the last sample.
- Tests: the instance-handle test of the generated TypeSupport writes two samples of key 5 with
  KEEP_ALL on both sides; under KEEP_LAST(1) two of its three samples arrived.
- Tests: the suites lease their domain from the band of `crates/test-domains`
  (`test/support/test-domains.ts`, held to the Rust constants by
  `crates/test-domains/tests/ts_lease_agrees.rs`) instead of the fixed domains 0, 1, 7, 8, 9 and
  55, which the Rust tests keep, and `100 + pid % 50`, whose ports reach into the ephemeral range.


### Removed
- The QoS struct layouts (`ZeroDdsTopicQos`, `ZeroDdsDataWriterQos`, `ZeroDdsDataReaderQos`) end in the
  DataRepresentation and TypeConsistencyEnforcement policies of the C ABI.
- `src/cdr/` (`types.ts`'s `DdsTopicType<T>`, `writer.ts`'s
  `Xcdr2Writer`, `reader.ts`'s `Xcdr2Reader`, `md5.ts`, `errors.ts`,
  `index.ts`) — the hand-written XCDR2 codec this binding carried
  alongside `runtime/typescript`. Nothing in `src/` (`dds.ts`,
  `index.ts`, `native.ts`, `qos.ts`) ever depended on it; only
  `test/xcdr2-wire-vectors.test.ts` and `examples/topic-typed-smoke.ts`
  did, and both are ported onto `@zerodds/wire` (below).

### Added
- `@zerodds/wire` (`runtime/typescript`) as a dependency, linked via
  `file:../../runtime/typescript` (this repository has no npm
  workspaces). Building `crates/ts-node` therefore requires
  `runtime/typescript` to have been built first (`npm run build`
  there) — npm does not build a `file:` dependency's TypeScript source
  the way MSBuild/Maven build a sibling project reference for the C#/
  Java columns.
- `examples/topic-typed-smoke.ts` now implements `dds.ts`'s own
  `TypeSupport<T>` bridge interface directly (the shape
  `DomainParticipant.createTypedTopic` actually accepts), calling
  `@zerodds/wire`'s `Writer`/`Reader` instead of the removed
  `Xcdr2Writer`/`Xcdr2Reader`.

### Fixed
- `test/xcdr2-wire-vectors.test.ts` — V-1..V-12 from
  zerodds-xcdr2-bindings-conformance-1.0 §6, now run directly against
  `@zerodds/wire`. All vectors but one came out byte-identical; the
  removed `Xcdr2Writer`'s V-10 vector wrote the `@mutable` type's
  `string` member under a length code of 3 with a separately-patched
  NEXTINT (DHEADER 23, total 27 bytes). XTypes 1.3 §7.4.3.4.2's length
  code 3 is 8 inline octets with **no** NEXTINT at all — any
  spec-compliant peer reading that header would consume exactly 8
  octets and desync from that member on, not merely read a
  non-optimal-but-valid wire like the C#/Java columns' equivalent
  finding. Length code 5 (NEXTINT reused from the string's own leading
  length word, per §7.4.3.4.2) is what `@zerodds/wire` and its own
  `zerodds-idlc --lang typescript` emission use here — DHEADER 19,
  total 23 bytes — confirmed against a fresh `zerodds-idlc generate
  --lang typescript` run for the equivalent IDL type.
- The MD5 self-check tests (RFC 1321 vectors) are removed along with
  them: `@zerodds/wire` has no MD5 (unlike `runtime/csharp`'s
  `Writer.Md5Digest()` / `runtime/java`'s `Writer.md5()`) — a real gap
  for a `@key` whose holder exceeds 16 octets (XTypes 1.3 §7.6.8.4),
  left open rather than reintroduced as a local copy. V-8's key holder
  is 4 octets, so its test only exercises the zero-pad half. **Closed**:
  `runtime/typescript/src/writer.ts` now has `Writer.zeroPadded()` /
  `Writer.md5()` and a module-level `md5()`, hand-rolled (RFC 1321)
  rather than a call into a platform digest — this runtime has to run
  in a browser as well as Node, and the Web Crypto API a browser
  exposes has no MD5 at all, unlike Java's `MessageDigest`. RFC 1321
  §A.5's own test vectors: `runtime/typescript/tests/keyhash.ts`, run
  from `run-tests.sh`.

### Added (closing the gap above)
- `tools/idlc/src/typescript_support.rs`: `zerodds-idlc --lang
  typescript` now appends an `export const <Type>TypeSupport` companion
  per topic-eligible struct/union to the same generated file — the
  `{typeName, encode, decode, isKeyed?, keyHash?}` shape `dds.ts`'s
  `TypeSupport<T>` already accepted, and `DomainParticipant.
  createTypedTopic` already wired to it (`TypeSupport<T>` was
  `dds.ts`'s own documented target shape for this before the generator
  existed). A type nested in an IDL module gets its companion nested
  the same way, by reopening that module's `namespace` (`bridge.
  PointTypeSupport` beside `bridge.Point`); every type's key-holder
  helpers in one file are declared once, in a shared `namespace
  ZdKeyCodec`, not once per type and not at the file's outer scope,
  where a bare top-level `function zd_enc_p_long` declared twice would
  be `Duplicate function implementation` (TypeScript tolerates no
  redeclaration the way Python's same-named `def` does).
- `tools/idlc/tests/typescript-support/`: the TypeScript counterpart of
  `python-support`/`csharp-support` — `support.idl` (Point/Named/
  ShortName/Extended/Empty, the same fixture `csharp-support` uses),
  `consumer.ts` (descriptor identity, LE/BE octets, maximum-size MD5
  selection against `node:crypto`'s independent digest, optional
  presence/absence, four explicit refusals), `against-recorded-
  octets.ts` (byte-identical to the Rust column's own recorded octets
  in `tests/interop-results/struct-appendable/zerodds-rust/`, both
  XCDR1 and XCDR2, both byte orders, encode and decode), and
  `typesupport-interface-check.ts` (type-check only: a generated
  companion is assignable to `dds.ts`'s real `TypeSupport<T>`, checked
  via a type-only import against that interface itself rather than a
  copy of it — no `koffi`, no domain participant). See its README.md.

## [0.0.0]

Pre-release.
