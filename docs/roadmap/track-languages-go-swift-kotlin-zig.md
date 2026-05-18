# Track RC3-A — Sprach-Bindings: Go, Swift, Kotlin, Zig

**Goal:** vier neue First-Class-Sprach-Bindings auf Basis der existing
`zerodds-c-api` (FFI-Foundation, 185 exportierte Symbole).

**Status:** 📋 todo

**Estimate:** 6-10 Personenwochen für alle vier Sprachen.

## Pro Sprache

### Go (`crates/go/` + Go-Module `github.com/zero-objects/zerodds-go`)

**Approach:** cgo-Binding gegen `zerodds-c-api`.

- `cgo` mit `// #cgo LDFLAGS: -lzerodds`
- API-Mapping: idiomatic Go (`Participant.CreateTopic`, `Writer.Write`,
  `<-Reader.Take()` als Channel)
- IDL-Codegen-Backend: `idlc --lang go` ergänzen (in `crates/idl-go`)
- `go test ./...` als CI-Gate
- Veröffentlichung: `go.zerodds.org/dds` als import path, GoProxy
  configured

**Estimate:** 2-3 PW. Go ist groß im Cloud-Native + Industrial-IoT
(Prometheus/etcd/Containerd), klare Adoption-Lücke.

### Swift (`crates/swift/` + SwiftPM-Package)

**Approach:** Swift-System-Module über `module.modulemap` + `zerodds-c-api`.

- SwiftPM-Manifest mit Native-System-Library
- API-Mapping: Swift-async/await für Reader.take(), Codable für IDL-types
- IDL-Codegen-Backend: `idlc --lang swift` (in `crates/idl-swift`)
- Tests: XCTest auf macOS, läuft ggf. auch auf Linux mit Swift 5.9+
- Veröffentlichung: SwiftPackageIndex, github-tag

**Estimate:** 1.5-2 PW. Apple-Ecosystem (iOS/iPadOS/visionOS) ist die
Hauptzielgruppe.

### Kotlin (`crates/kotlin/` + Maven-Coordinate)

**Approach:** Kotlin-Multiplatform in Java (auf JVM) + Native-Image (auf
Android/Native).

- Wrapper um `java-omgdds` (existiert schon ✅), Kotlin-idiomatic-API
  drüberlegt (Coroutines + Flow für Reader.take())
- IDL-Codegen-Backend: `idlc --lang kotlin` ergänzen
- Veröffentlichung: maven-central via Sonatype, AAR für Android

**Estimate:** 1.5-2 PW. Wichtig für Android + Spring-Kotlin-Backend.

### Zig (`crates/zig/` + Zig-Build-Package)

**Approach:** `@cImport`-Direkt-Binding auf `zerodds-c-api`.

- `build.zig.zon`-Manifest
- Idiomatic Zig-API: `Participant.create()` + comptime-Topic-Type-Resolve
- IDL-Codegen-Backend: `idlc --lang zig` ergänzen
- Tests: `zig build test`
- Veröffentlichung: `zigmod` + GitHub-tag

**Estimate:** 1 PW (Zig ist einfach durch C-Interop, comptime-tricks
aber lohnen).

## Cross-Cutting

### IDL-Codegen-Backends

Pro Sprache: `crates/idl-<lang>/` mit Implementation des
`Backend`-Traits aus `zerodds-idl`. Pattern wie idl-cpp/idl-csharp/
idl-java schon etabliert. Pro Sprache:

- Type-Mapping (IDL → Go/Swift/Kotlin/Zig)
- Module-Path-Mapping
- @key / @final / @appendable / @mutable Annotations
- Output-Tree-Layout

### Per-Sprache Vendor-Spec

Pro Sprache eine eigene Spec analog zu `zerodds-xcdr2-rust-1.0`:
- `zerodds-xcdr2-go-1.0.md`
- `zerodds-xcdr2-swift-1.0.md`
- `zerodds-xcdr2-kotlin-1.0.md`
- `zerodds-xcdr2-zig-1.0.md`

Plus Plus den `zerodds-xcdr2-bindings-conformance-1.0.md` um die 4 neuen
Sprachen erweitern (cross-language-roundtrip-test-matrix wächst von
6 auf 10 Sprachen).

### Cross-Language Roundtrip-Test

Erweitern: `tests/cross_lang_live/run_all.sh` testet jetzt 10 Sprachen
gegen einander (Rust-Publisher → 9 Subscriber, plus 9 Publisher → Rust-
Subscriber, plus 1 zentralen Sample-Set der ALLE Type-Construct-Varianten
abdeckt).

## Distribution

| Sprache | Channel | Auto-Publish |
|---|---|---|
| Go | proxy.golang.org via tag-push | ja |
| Swift | SwiftPackageIndex via tag-push | ja |
| Kotlin | maven-central via Sonatype OSSRH | manueller release-Schritt initial, dann auto |
| Zig | zigmod / github-tag | ja |

## Acceptance

1. Pro Sprache: hello-world Pub/Sub-Pair in der Sprache lauffähig
2. Pro Sprache: idiomatic API (nicht nur c-api-thinrap)
3. Pro Sprache: spec-coverage-doc 0/0 partial/open
4. Cross-language roundtrip 10×10 Matrix läuft (mind. 70 % grüne Zellen
   in CI, 100 % auf Linux x86_64)
5. Per-Sprache Distribution-Channel erreichbar mit `go get` / `swift
   build` / `gradle dependencies` / `zig fetch`

## Out-of-Scope

- **Ruby, PHP, Lua, Perl** — kein Mainstream-DDS-Use-Case, kein Demand
- **Crystal, Nim, Vala** — Hobbyist-Sprachen, keine Industrial-Adoption
- **R, Julia** — wenn Datalake-Track Demand erzeugt, separate Track post-
  1.0
- **Dart-native** — Flutter-Binding existiert schon via `dart:ffi` in
  Round-1

## Dependencies

- zerodds-c-api stabile API (✅ seit RC1, ABI-snapshot-test)
- idl-Codegen Trait stabile (✅)
- Multi-platform CI-Runner (Linux + macOS + Windows mind., Linux ARM
  für Cross-Build)

## Risks

- **JNI-Komplexität bei Kotlin Coroutines**: callback-Semantik vom JNI
  ist heikel. Mitigation: Wrapper über `Flow` mit dedicated background-
  Thread.
- **Swift-on-Linux**: Reader-Take async auf Linux-Swift hatte 2024 noch
  Edge-Cases. Mitigation: Swift 6.0+ als minimum.
- **Zig-Sprach-Stabilität**: Zig ist pre-1.0. Mitigation: per-version
  pinning, regen on Zig-major-Bump.
