# Changelog — `@zerodds/node`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `@zerodds/cdr` helper modules under `src/cdr/` (mandatory conformance
  to zerodds-xcdr2-ts-1.0 §8):
  - `types.ts` — `DdsTopicType<T>`, `ExtensibilityKind`, `EndianMode`.
  - `writer.ts` — `Xcdr2Writer` with primitive writes,
    string/wstring, alignment-origin stack, `beginAppendable`/
    `beginMutable`/`endAppendable`/`endMutable`, `writeEmHeader`,
    `patchUint32`.
  - `reader.ts` — `Xcdr2Reader` as the inverse of the writer plus
    `readEmHeader` and `lcInlineSize`.
  - `md5.ts` — RFC 1321 MD5 in pure TypeScript (synchronous, no
    Web Crypto API), only for the DDS key hash.
  - `errors.ts` — `XcdrError extends Error`.
  - `index.ts` — re-exports.
- Mandatory wire-conformance tests `test/xcdr2-wire-vectors.test.ts`
  for V-1..V-12 from zerodds-xcdr2-bindings-conformance-1.0 §6
  (15 tests, all byte-exact + roundtrip + MD5 self-check).
- New `npm run test:wire` script — runs the wire-vector tests
  in isolation without the native library.

### Fixed
- `Xcdr2Writer.writeEmHeader` now writes EMHEADER1 as a
  big-endian 4-byte word independent of the stream endian, matching
  V-10/V-11A of the conformance spec. NEXTINT (if present) stays
  stream-endian.
- `Xcdr2Reader.readEmHeader` reads EMHEADER mirror-image as BE.

## [0.0.0]

Pre-release.
