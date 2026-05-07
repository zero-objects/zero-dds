# Changelog — `@zerodds/node`

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
SemVer per [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `@zerodds/cdr` Helper-Module unter `src/cdr/` (Pflicht-Konformanz
  zu zerodds-xcdr2-ts-1.0 §8):
  - `types.ts` — `DdsTopicType<T>`, `ExtensibilityKind`, `EndianMode`.
  - `writer.ts` — `Xcdr2Writer` mit Primitive-Writes,
    String/WString, Alignment-Origin-Stack, `beginAppendable`/
    `beginMutable`/`endAppendable`/`endMutable`, `writeEmHeader`,
    `patchUint32`.
  - `reader.ts` — `Xcdr2Reader` als Inverse zum Writer plus
    `readEmHeader` und `lcInlineSize`.
  - `md5.ts` — RFC 1321 MD5 in pure TypeScript (synchron, kein
    Web-Crypto-API), nur fuer DDS-Key-Hash.
  - `errors.ts` — `XcdrError extends Error`.
  - `index.ts` — Re-Exports.
- Pflicht-Wire-Konformanz-Tests `test/xcdr2-wire-vectors.test.ts`
  fuer V-1..V-12 aus zerodds-xcdr2-bindings-conformance-1.0 §6
  (15 Tests, alle byte-exakt + Roundtrip + MD5-Self-Check).
- Neuer `npm run test:wire`-Script — laeuft die Wire-Vector-Tests
  in Isolation ohne native Library.

### Fixed
- `Xcdr2Writer.writeEmHeader` schreibt EMHEADER1 jetzt als
  Big-Endian 4-Byte-Wort unabhaengig vom Stream-Endian, passend zu
  V-10/V-11A der Conformance-Spec. NEXTINT (falls vorhanden) bleibt
  Stream-Endian.
- `Xcdr2Reader.readEmHeader` liest EMHEADER spiegelbildlich als BE.

## [0.0.0]

Pre-release.
