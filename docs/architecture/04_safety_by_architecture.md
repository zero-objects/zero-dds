# Safety-by-Architecture

> **Status:** Draft v0.2
> **Abhängigkeiten:** `02_architecture.md`, `03_profiles_and_platforms.md`
> **Eigentümer:** Safety Engineering Lead

Dieses Dokument ist der verbindliche Vertrag für alle Safety-relevanten architektonischen Entscheidungen. Jede Code-Änderung in Safe-klassifizierten Crates muss diesem Dokument genügen. Verstöße werden durch CI blockiert.

## 1 Philosophie

Safety-by-Architecture bedeutet: die Codebase ist so strukturiert, dass Safety-Zertifizierung (ISO 26262 ASIL D, DO-178C DAL B+, IEC 61508 SIL 3+) **möglich** ist — ohne dass jede Änderung kontinuierliche Safety-Reviews erfordert. Die architektonischen Regeln werden einmalig fest verankert und durchgehend automatisch durchgesetzt. Safety-Audit am Ende ist dann ein Dokumentations- und Validierungs-Schritt, kein Refactoring-Schritt.

Drei Grundsätze:

1. **Statische Durchsetzung vor Runtime-Checks.** Regelverletzungen werden zur Compile-Zeit oder in CI erkannt, nicht zur Laufzeit.
2. **Separation of Concerns zwischen Safe und Comfort.** Sicherheitskritische Crates haben andere Regeln als Comfort-Crates. Die Grenze ist klar und physisch im Workspace manifestiert.
3. **Traceability als Nebenprodukt, nicht als Nachlauf.** Commits, Tests und Code-Annotationen erzeugen die Artefakte, die ein Auditor braucht, im laufenden Betrieb — nicht retrospektiv.

## 2 Safe-Subset-Vertrag

Die folgenden Crates sind als **Safe-Subset** klassifiziert:

```
zerodds-foundation
zerodds-cdr
zerodds-types
zerodds-qos
zerodds-rtps
zerodds-discovery
zerodds-transport (trait-only)
zerodds-transport-udp (ohne tokio-feature)
zerodds-transport-shm
zerodds-security (Kern-Plugin-API, ohne Plugin-Implementierungen)
zerodds-xrce-client
zerodds-sys (stabile C-ABI-Oberfläche)
```

Diese Crates müssen den folgenden Vertrag einhalten.

### 2.1 Sprach-Einschränkungen

| Regel | Durchsetzung |
|---|---|
| Keine `panic!()`, `unreachable!()`, `todo!()`, `unimplemented!()` außerhalb `#[cfg(debug_assertions)]` oder Tests | `clippy::panic = "deny"`, `clippy::unreachable = "deny"`, `clippy::todo = "deny"`, `clippy::unimplemented = "deny"` |
| Keine `.unwrap()`, `.expect()` außerhalb Tests | `clippy::unwrap_used = "deny"`, `clippy::expect_used = "deny"` |
| Keine `.unwrap_or_default()` wenn Default unerwünscht ist | manuelles Review |
| Kein `unsafe`-Code ohne SAFETY-Kommentar | Custom lint `dds_require_safety_comment` |
| Keine `dyn Trait` außer in explizit markierten Plugin-Boundaries | Custom lint `dds_no_dyn_in_safe` |
| Keine `std`-Dependencies (nur `core` + `alloc`) | `#![no_std]` + `extern crate alloc` |
| Keine `tokio` oder andere Async-Runtimes (stattdessen Executor-agnostic über `core::future::Future` und `futures-core`) | Dependency-Check in CI |
| Keine `HashMap` (verwendet Randomness für DoS-Resistenz, nicht deterministisch) | Clippy-Lint: `disallowed-types` |
| `Vec` nur mit bounded Capacity; bevorzugt `heapless::Vec` | Review + Custom lint |
| Keine Rekursion ohne dokumentierte obere Tiefenschranke | Review-Regel, kein automatischer Lint |

### 2.2 Speicher-Disziplin

Alle Safe-Crates halten sich an strenge Speicher-Regeln:

- **Keine dynamische Allocation in Hot Paths.** "Hot Path" ist definiert als jeder Code, der in einem Sample-Verarbeitungspfad (Receive, Deserialize, Deliver) läuft.
- **Static-Allocation first.** Interne Datenstrukturen nutzen `heapless::Vec`, `heapless::FnvIndexMap`, vorab allozierte Pools aus `zerodds-foundation::pool`.
- **Bounded Queues.** Jede Queue hat eine explizite Obergrenze. Overflow-Verhalten ist policy-gesteuert (Drop, Block, Reject), nie undefined.
- **Kein unbeschränktes `Vec::push` auf User-Input.** Länge-Limits werden vor jedem Wachstum geprüft.
- **Owned vs. Borrowed klar.** Hot-Path-APIs akzeptieren `&[u8]` oder `Bytes`, nie `Vec<u8>`.

### 2.3 Fehler-Behandlung

- Jeder fehlbare Call gibt ein `Result<T, E>` zurück. Fehler-Enums sind per `thiserror` definiert, exhaustive, und stabil (SemVer-Pflicht).
- Keine `io::Error` in Public-APIs von Safe-Crates (Abhängigkeit auf `std::io`).
- Panics sind nur akzeptabel in Invariant-Verletzungen, die logisch unmöglich sind — und auch dann werden sie zu `Result<_, InvariantViolation>` umgewandelt, wo ein Fehlerpfad existiert.

### 2.4 Concurrency

- **Keine `std::sync::Mutex`** (verwendet Futex auf Linux, nicht analysierbar in Safety-Kontext).
- Stattdessen: `spin::Mutex` (mit dokumentierten Wait-Verhalten-Garantien) oder OS-spezifische Primitive, die von der Ziel-RTOS qualifiziert sind.
- **Kein `std::thread::spawn`** in Safe-Crates; Concurrency wird extern injiziert (Executor-agnostisches Future-API).
- **Atomic-Operationen** sind erlaubt, aber jede Memory-Ordering-Annotation (`Ordering::Acquire` etc.) muss kommentiert und gerechtfertigt sein.

### 2.5 Generik und Monomorphisierung

- Generics werden bevorzugt über `dyn Trait` in Performance-kritischen Pfaden, um Devirtualisierung zu garantieren.
- Explizite `Box<dyn Trait>` ist in Plugin-Boundaries (Security-Plugins, Transport-Plugins) erlaubt und erforderlich, aber muss in Trait-Objects mit `'static` Bound sein und als `#[allow(dds_no_dyn_in_safe)]` markiert werden.
- Monomorphisierungs-Explosion wird vermieden durch Facette-Traits und sekundäre Object-Safe-Traits für die API-Surface.

## 3 CI-Durchsetzung

Die folgende CI-Pipeline läuft bei jedem PR und muss grün sein für Merge. Safety-relevante Jobs sind nicht überspringbar.

### 3.1 Build-Jobs

| Job | Zweck | Blocking? |
|---|---|---|
| `cargo build --all --all-features` | Baut Full-Profile | ✓ |
| `cargo build -p safe-crates-only --no-default-features --features safety` | Baut Safe-Profile ohne std | ✓ |
| `cargo build --target aarch64-unknown-none -p zerodds-rtps -p zerodds-xrce-client` | Cross-compile für bare-metal | ✓ |
| `cargo build --target thumbv7em-none-eabihf -p zerodds-xrce-client` | Micro-Profile Cortex-M7 | ✓ |
| Ferrocene build für Safe-Crates | Qualified toolchain | ✓ |

### 3.2 Lint-Jobs

| Job | Inhalt | Blocking? |
|---|---|---|
| `cargo clippy -- -D warnings` | Workspace-weite Clippy-Lints | ✓ |
| `cargo clippy -p <safe-crate> --features safety -- -D clippy::unwrap_used -D clippy::panic -D clippy::unreachable` | Safe-spezifische Lints | ✓ |
| Custom `zerodds-lint` (eigener Clippy-Plugin) | Projekt-spezifische Regeln (siehe unten) | ✓ |
| `cargo fmt -- --check` | Formatierung | ✓ |
| `cargo deny check` | License + Security Audit | ✓ |

### 3.3 Test-Jobs

| Job | Inhalt | Blocking? |
|---|---|---|
| `cargo test --workspace` | Unit + Integration Tests | ✓ |
| `cargo miri test -p zerodds-cdr -p zerodds-rtps` | Undefined-Behavior-Detection | ✓ |
| `cargo kani -p zerodds-foundation -p zerodds-cdr -p zerodds-qos` | Model-Checking für formalisierbare Properties | Nightly |
| `cargo fuzz run rtps_parser` | Fuzz-Testing für Wire-Parser | Nightly, mindestens 1h pro Run |
| OMG-Conformance-Test-Suite | Spec-Compliance-Tests | ✓ |
| Interop gegen CycloneDDS, Fast DDS | Docker-compose-Harness | ✓ |
| Performance-Regression (Criterion) | Keine >5% Regression in Hot-Path-Benchmarks | ✓ |

### 3.4 Custom Lints (`zerodds-lint` Crate)

`zerodds-lint` ist ein eigenes Binary-Crate (`crates/lint`), das die folgenden
Projekt-Regeln **AST-basiert auf stable Rust** durchsetzt — kein
Nightly-Toolchain, kein dylint, keine Type-Info. Aufruf in CI:
`cargo run -p zerodds-lint -- check` (siehe GitLab-CI-Job `zerodds-lint`).

Stand WP 0.7 (Phase 0):

| Lint | Status | Markierung zur Ausnahme |
|---|---|---|
| `dds_require_safety_comment` | implementiert | `// SAFETY: <begruendung>` direkt vor unsafe-Block/fn/impl |
| `dds_no_dyn_in_safe` | implementiert | File-Marker `zerodds-lint: allow no_dyn_in_safe` |
| `dds_safety_classification_present` | implementiert | jede Crate mit `lib.rs` braucht `Safety classification: **<KLASSE>**` im Doc-Header |
| `dds_no_panic_in_safe` | implementiert | tests/examples ausgenommen, File-Marker `zerodds-lint: allow no_panic_in_safe` |
| `dds_no_alloc_in_hot_path` | implementiert | aktiviert per Doc-Marker `/// zerodds-lint: hot-path` an Funktion oder Modul |
| `dds_bounded_recursion` | implementiert (Phase-0-Approximation: intra-File, max. 1-Hop indirekt) | Doc-Marker `/// zerodds-lint: recursion-depth N` |
| `dds_spec_annotated` | nicht aktiv | Bestand braucht Migration; Phase 1 |

**Phase-0-Limitierungen** (alle bewusst):

- Keine Type-Info: `.unwrap()` flaggt unabhaengig vom Receiver-Typ.
- Custom-Attribute (`#[dds_hot_path]`, `#[dds_recursion_depth(N)]`) sind auf
  stable Rust ohne `register_tool` nicht ohne Proc-Macro syntaktisch
  erlaubt — wir ersetzen sie durch Doc-Comment-Marker
  (`/// zerodds-lint: hot-path`, `/// zerodds-lint: recursion-depth N`), die als
  regulaere `#[doc = "..."]`-Attribute geparst werden.
- Rekursions-Erkennung ist intra-File und maximal 1-Hop indirekt; cycles
  laenger als zwei Funktionen oder cross-file Rekursion (Trait-Impls,
  mod-Splits) sind nicht erfasst.
- Tests, Examples und Benches werden flaechendeckend von den Lints
  ausgenommen.

Echte Clippy-Plugin-Variante mit Type-Info (dylint oder rustc-driver) ist
fuer Phase 1 vorgesehen, sobald sich die Anforderungen aus realer
Anwendung herauskristallisieren.

## 4 Traceability-Infrastruktur

### 4.1 Commit-Konvention

Alle Commits folgen Conventional Commits mit zusätzlichem Requirements-Tag:

```
<type>(<scope>): <description> [REQ-<id>]

<body>

<footer>
```

Beispiele:

```
feat(rtps): implement Heartbeat submessage [REQ-RTPS-0047]

Implements OMG DDSI-RTPS 2.5 §8.3.7.3 Heartbeat Submessage per spec.
Serializer validates first/last sequence number invariant.

Tests: tests/heartbeat_roundtrip.rs, tests/heartbeat_spec_vectors.rs
Covers: REQ-RTPS-0047, REQ-RTPS-0048, REQ-RTPS-0049
```

`REQ-<id>` verweist auf Einträge im Requirements-Tracker (Polarion, DOORS, oder projekteigen).

### 4.2 Code-Annotationen

```rust
/// Implements DDSI-RTPS 2.5 §8.3.7.3 Heartbeat Submessage.
///
/// # Safety
/// The `first_sn` and `last_sn` fields must satisfy `first_sn <= last_sn + 1`
/// per spec. This invariant is checked on deserialization.
#[spec(rtps = "2.5", section = "8.3.7.3")]
#[satisfies(req = ["REQ-RTPS-0047", "REQ-RTPS-0048"])]
pub struct Heartbeat {
    pub reader_id: EntityId,
    pub writer_id: EntityId,
    pub first_sn: SequenceNumber,
    pub last_sn: SequenceNumber,
    pub count: Count,
}
```

Die Annotationen werden vom `zerodds-traceability`-Tool aggregiert in eine Matrix:
- Requirements → Code (welcher Code implementiert welches Req)
- Code → Tests (welche Tests decken welchen Code)
- Requirements → Tests (welche Tests verifizieren welches Req)

### 4.3 Test-Annotationen

```rust
/// Verifies DDSI-RTPS 2.5 §8.3.7.3.1 Heartbeat validity invariant.
#[test]
#[verifies(req = "REQ-RTPS-0047")]
#[spec_vector(source = "OMG DDSI-RTPS 2.5 Annex B, Vector 23")]
fn heartbeat_validates_sn_ordering() {
    // ...
}
```

## 5 Ferrocene-Integration (Expansion-Era)

Ferrocene ist der qualifizierte Rust-Compiler, der für formale Safety-Zertifizierung des Safe-Subsets erforderlich ist. Ferrocene ist TÜV-Süd-qualifiziert nach ISO 26262 ASIL D, IEC 61508 SIL 3, IEC 62304 Class C, und unterstützt Qualifizierungs-Bemühungen bis SIL 4 und DO-178C DAL C.

**Aktueller Plan-Status:** Ferrocene-Integration ist ein **Expansion-Era-Thema** (Track B in `06_roadmap.md` §8.1). In Bootstrap- und Proof-Era wird der Safe-Subset mit stable Rust gebaut. Die Architektur-Disziplin (no_panic, no_alloc-in-hot-path, strukturelle Trennung) ist ab Tag 1 in Kraft und von stable Rust durchsetzbar — Ferrocene fügt die formale Qualifikation hinzu, ändert aber nicht den Code-Stil.

Damit der Umstieg auf Ferrocene später ohne Refactoring-Kosten möglich ist, gelten folgende Regeln bereits in Bootstrap-Era:

- Safe-Crates verwenden nur APIs, die im Ferrocene Certified Core Subset enthalten sind (ISO 26262 ASIL B / IEC 61508 SIL 2), soweit bekannt. Clippy-`disallowed-methods`-Konfiguration wird entsprechend gepflegt.
- Toolchain-Pinning ist vorbereitet, nur aktuell noch auf stable Rust (`rust-toolchain.toml`). Der Switch-over auf Ferrocene-Channel erfordert nur eine Konfigurations-Änderung.
- Target-Triples werden in CI bereits so gewählt, dass sie mit dem Ferrocene-Target-Portfolio kompatibel sind.

### 5.1 Ferrocene-Release-Pinning (wird in Track B aktiviert)

Bei Expansion-Era-Switch wird ein spezifisches Ferrocene-Release gepinnt:

```toml
[toolchain]
channel = "ferrocene-XX.YY.Z"
components = ["rust-src"]
targets = ["aarch64-unknown-nto-qnx710", "..."]
```

Release-Upgrades werden formell durch Safety-Review gezogen.

### 5.2 Certified Core Subset (ab Expansion-Era relevant)

Das Ferrocene Certified Core Subset (aktuell ISO 26262 ASIL B, IEC 61508 SIL 2) wird in Safe-Crates als Design-Maßstab bereits in Bootstrap-Era verwendet. Verfügbare APIs umfassen `Option`, `Result`, `Clone`, `str`, Pointer-Types, die meisten Primitives, `core::slice`, `core::iter`, `core::ffi`. Nicht-zertifizierte APIs sind in Safe-Crates verboten via `disallowed-methods` Clippy-Konfiguration.

### 5.3 Target-Platforms für Safe-Builds (Expansion-Era)

Aktuell von Ferrocene qualifizierte Ziel-Plattformen (Stand Prüfung bei Track-B-Start):
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu` (aktuelle Release-Coverage prüfen)
- `aarch64-unknown-none` (bare-metal)
- `x86_64-pc-nto-qnx710`, `aarch64-unknown-nto-qnx710`
- `armv8r-none-eabihf` (Cortex-R)

Weitere Targets werden je nach Projekt-Bedarf mit Ferrous Systems als Engineering-Partnerschaft verhandelt.

## 6 Audit-Pfad (Expansion-Era)

Der formale Safety-Audit ist ein **Expansion-Era-Thema** (Track C in `06_roadmap.md` §8.1). Die Vorbereitung dafür läuft aber bereits in Bootstrap- und Proof-Era mit.

### 6.1 Bootstrap- und Proof-Era-Vorbereitung

- Safety-by-Architecture-Disziplin wird ab Tag 1 etabliert und automatisch durchgesetzt (Lints, CI).
- Traceability-Annotationen und Commit-Konventionen werden ab Tag 1 gelebt (siehe §4).
- Am Ende der Proof-Era (Phase 4) werden die Audit-Artefakte grundsätzlich erstellbar sein — ohne dass jemand sie aktiv als Artefakte verpackt hat.

Das spart erhebliche Retrofit-Kosten, falls und wenn Track C aktiviert wird.

### 6.2 Expansion-Era-Arbeitspakete

Bei Track-C-Aktivierung sind folgende Schritte erforderlich:
- Safety-Engineer ins Team aufnehmen (dedizierte Rolle)
- Requirements-Extraktion und formale Traceability-Matrix-Konsolidierung
- MC/DC-Coverage-Push (für DAL B+)
- Safety-Case-Dokumentation
- Externer Audit durch TÜV Süd oder Alternative

### 6.3 Benötigte Artefakte für Audit

Die folgende Liste definiert, welche Artefakte bei Track-C-Aktivierung vorhanden sein müssen. Die meisten sind durch die architektonische Disziplin bereits ansatzweise vorhanden; Safety-Engineer konsolidiert und ergänzt.

1. **Safety Plan** — Dokument das beschreibt, wie Safety im Projekt umgesetzt wird (dieses Dokument + Ergänzungen).
2. **Requirements Specification** — formale Anforderungen an den Safe-Subset, jeweils mit eindeutiger ID.
3. **Architecture Specification** — `02_architecture.md` + Zusatz für Safe-Subset-Interna.
4. **Module Specification** — pro Crate im Safe-Subset eine detaillierte Modul-Spec.
5. **Test Specification** — welche Tests welche Requirements verifizieren.
6. **Verification Report** — Ergebnisse aller Tests, Coverage-Reports (inkl. MC/DC für DAL B+), Static-Analysis-Reports.
7. **Validation Report** — Nachweis, dass das Produkt in Zielumgebungen korrekt funktioniert (Interop-Tests, Target-Hardware-Tests).
8. **Safety Manual** — Anleitung für Integratoren, wie das Produkt in einem zertifizierten System korrekt eingesetzt wird.
9. **Change Management Log** — vollständige Git-History mit `[REQ-...]`-Tags, aggregiert als Change-Log.
10. **Tool Qualification Report** — Ferrocene-Qualifikations-Artefakte, verlinkt von Ferrous Systems.
11. **SBOM** — CycloneDX Software Bill of Materials pro Release.
12. **Vulnerability Analysis** — `cargo-audit`-Reports, threat-modeling-Dokumente, CVE-Tracking.

### 6.4 Ziel-Standards

Bei Track-C-Aktivierung werden diese Standards angestrebt (in Priorität):

1. **ISO 26262 ASIL D** (Automotive) — primär für Automotive SDV-Kunden.
2. **IEC 61508 SIL 3** (Industrielle Basis) — Grundlage für weitere industrielle Standards.
3. **DO-178C DAL B** initial, DAL A perspektivisch (Avionik).
4. **IEC 62304 Class C** (Medizintechnik) — sekundär, je nach Kunden-Nachfrage.
5. **EN 50128/50716** (Bahn) — sekundär.

## 7 Violations-Protokoll

Wenn ein Commit die Safety-Regeln verletzt:

1. **CI blockiert den Merge.** Lint- oder Test-Failures erscheinen im PR.
2. **Auto-Fix wo möglich.** Claude-Teams-Agenten können viele Verletzungen automatisch beheben (z.B. `.unwrap()` durch explizites Error-Handling ersetzen).
3. **Eskalation bei echten Konflikten.** Wenn eine Safety-Regel aus technischer Notwendigkeit gebrochen werden muss (z.B. Performance-kritischer `unsafe`-Block), ist ein formeller Safety-Waiver-Request erforderlich. Dieser erfordert:
   - Technische Begründung
   - Alternativ-Analyse (warum andere Wege nicht funktionieren)
   - Risiko-Bewertung
   - Kompensatorische Maßnahmen (z.B. zusätzliche Tests, Fuzz-Coverage)
   - Zustimmung vom Safety-Engineering-Lead und einem Senior-Engineer aus einem anderen Team
4. **Dokumentation im Safety-Waiver-Register.** Alle erteilten Waiver werden im Repo unter `docs/safety-waivers/` dokumentiert und in Audit einbezogen.

## 8 Rückblick und Review

Diese Safety-Architektur wird mindestens einmal pro Projektphase formell reviewt. Änderungen erfordern Safety-Engineering-Lead-Zustimmung und werden in den Release-Notes prominent vermerkt.
