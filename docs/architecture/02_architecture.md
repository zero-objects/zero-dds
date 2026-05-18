# System-Architektur und Crate-Workspace

> **Status:** Draft v0.2
> **Abhängigkeiten:** `00_overview.md`, `01_scope_and_specs.md`

## 1 Architektur-Prinzipien

Die folgenden Prinzipien sind für alle Architektur-Entscheidungen verbindlich. Konflikte sind in dieser Reihenfolge aufzulösen:

1. **Korrektheit vor Performance.** RTPS-Interop und QoS-Semantik dürfen nicht für Mikrosekunden geopfert werden.
2. **Safety-Qualifizierbarkeit vor Komfort.** Kern-Module werden so geschrieben, dass sie safety-qualifizierbar bleiben, auch wenn die aktuelle Build-Variante das nicht verlangt.
3. **Spec-Konformität vor Feature-Innovation.** OMG-Abweichungen nur bei dokumentierter Begründung und Interop-Test-Nachweis.
4. **Modulare Trennung vor Monolithen.** Jedes Crate hat klare Verantwortung und Abhängigkeiten. Keine zirkulären Dependencies.
5. **Feature-Flags vor Forks.** Unterschiede zwischen Profilen werden durch Feature-Gates realisiert, nicht durch separate Code-Bäume.
6. **Generics vor Dynamic Dispatch.** In Safe-qualifizierbaren Crates ist `dyn Trait` nur in expliziter Begründung erlaubt. Devirtualisierung soll für den Compiler immer möglich sein.
7. **No-Panic-Kontrakt.** In allen Crates oberhalb `dds-tools` und `zerodds-dashboard` ist `.unwrap()`, `.expect()`, `panic!()`, `unreachable!()` (außerhalb von `#[cfg(debug_assertions)]`) durch CI-Lint verboten.

## 2 Schichten-Architektur

Das System ist in fünf Schichten organisiert. Abhängigkeiten fließen strikt von oben nach unten; Querabhängigkeiten innerhalb einer Schicht sind erlaubt, Aufwärts-Abhängigkeiten verboten.

```
┌─────────────────────────────────────────────────────────────┐
│  Public APIs: zerodds-rs, zerodds-sys, zerodds-cpp, zerodds-cs, zerodds-java,   │
│               zerodds-py                                         │
├─────────────────────────────────────────────────────────────┤
│  Core Services: zerodds-dcps, zerodds-rpc, zerodds-security, zerodds-xml,   │
│                 zerodds-recorder, zerodds-monitor, dds-tools        │
├─────────────────────────────────────────────────────────────┤
│  Protocol: zerodds-rtps, zerodds-discovery, zerodds-types, zerodds-cdr,     │
│            zerodds-qos                                           │
├─────────────────────────────────────────────────────────────┤
│  Transport: zerodds-transport (trait), zerodds-transport-udp,       │
│             zerodds-transport-tcp, zerodds-transport-shm            │
├─────────────────────────────────────────────────────────────┤
│  Foundation: zerodds-foundation                                  │
└─────────────────────────────────────────────────────────────┘
                     ↑
                     │
          Ferrocene qualified toolchain
          + certified core subset (Safe profile)
```

## 3 Crate-Katalog

Der Workspace umfasst folgende Crates. Jede Zeile markiert Safety-Klassifikation, ob no_std-tauglich und die primäre Verantwortung.

### 3.1 Foundation Layer

| Crate | Safety-Klasse | no_std | Verantwortung |
|---|---|---|---|
| `zerodds-foundation` | Safe | Ja | Kern-Typen (InstanceHandle, Time, Duration, SequenceNumber, GUID), Error-Enum-Familie, Result-Aliasse |

### 3.2 Transport Layer

| Crate | Safety-Klasse | no_std | Verantwortung |
|---|---|---|---|
| `zerodds-transport` | Safe | Ja | Transport-Trait (`Transport`, `Listener`, `Locator`), abstrakte Send/Receive |
| `zerodds-transport-udp` | Safe | Optional | UDP/IP PSM, Raw-Socket, Multicast |
| `zerodds-transport-tcp` | Standard | Nein | DDSI TCP/IP PSM, Connection-Pool |
| `zerodds-transport-shm` | Safe | Optional | Shared-Memory-Segment-Management, Zero-Copy-Path |

### 3.3 Protocol Layer

| Crate | Safety-Klasse | no_std | Verantwortung |
|---|---|---|---|
| `zerodds-cdr` | Safe | Ja | XCDR1/XCDR2 Encoder/Decoder, Endianness, Alignment |
| `zerodds-types` | Safe | Ja | XTypes Type System, TypeObject, TypeIdentifier, Compatibility |
| `zerodds-qos` | Safe | Ja | QoS Policies, Request/Offered-Compatibility-Matrix, Typestate-Kompatibilität |
| `zerodds-idl` | Safe | Nein (std-only) | IDL4-Parser, AST, Semantik-Modell (OMG IDL 4.2). Grammar-driven (Earley-Engine), Build-Zeit-Tool — kein embedded-Use-Case. Konsumiert von `zerodds-idlc`. Siehe `docs/rfcs/0001-idl-parser-architecture.md` |
| `zerodds-rtps` | Safe | Ja | Writer/Reader State Machines, Heartbeat/Acknack/Gap/Data-Submessages, Fragmentation |
| `zerodds-discovery` | Safe | Ja | SPDP, SEDP, TypeLookup Service |

### 3.4 Core Services Layer

| Crate | Safety-Klasse | no_std | Verantwortung |
|---|---|---|---|
| `zerodds-dcps` | Standard | Nein | DomainParticipant, Publisher, Subscriber, Topic, DataReader, DataWriter |
| `zerodds-rpc` | Standard | Nein | Request/Reply Framework, Service-Definition-Runtime |
| `zerodds-security` | Safe (Kern) / Standard (Plugins) | Teilweise | Authentication/AccessControl/Cryptographic Plugin-Trait + Default-Implementierungen |
| `zerodds-xml` | Standard | Nein | DDS-XML-Parser, QoS-Profile-Loader, Schema-Validator |
| `zerodds-xrce-client` | Safe | Ja (no alloc) | XRCE-Client für Micro-Profile, transport-agnostisch |
| `zerodds-xrce-agent` | Standard | Nein | XRCE-Agent, läuft im Full/Standard-Profile |
| `zerodds-recorder` | Comfort | Nein | Deterministic Record/Replay Service |
| `zerodds-monitor` | Comfort | Nein | OpenTelemetry-Instrumentierung, Prometheus-Exporter, Wire-Probe |
| `dds-tools` | Comfort | Nein | Admin-CLI, Config-Validator |

### 3.5 Binding/API Layer

| Crate | Safety-Klasse | no_std | Verantwortung |
|---|---|---|---|
| `zerodds-rs` | Standard | Nein | Idiomatisches Rust-SDK, async/await, Streams |
| `zerodds-sys` | Safe (Kern) / Binding (FFI-Modul) | Ja (Kern) | Stabile C-ABI, Basis für alle Nicht-Rust-Bindings. `lib.rs`-Kern ist Safe/no_std; C-ABI-Exports leben isoliert in `mod ffi` (siehe §4.4.3/§4.4.4) |
| `zerodds-cpp` | Standard | Nein | C++-Wrapper, IDL4-C++-Runtime |
| `zerodds-cs` | Standard | Nein | C# P/Invoke, NativeAOT-kompatibel, IDL4-C#-Runtime |
| `zerodds-java-omgdds` | Standard | Nein | Pure-Java DDS-Java-PSM (`org.omg.dds.*`) + IDL4-Java-Runtime; kein JNI, kein Native-Lib auf der Java-Seite |
| `zerodds-py` | Comfort | Nein | PyO3-Bindings, pandas/numpy-freundlich |

### 3.6 Tooling (Binary-Crates)

| Crate | Typ | Verantwortung |
|---|---|---|
| `zerodds-idlc` | bin | IDL4-Compiler, Backends: C, C++, C#, Java, Python, Rust. Nutzt `zerodds-idl` fuer Parser/AST |
| `zerodds-admin` | bin | Admin-CLI: Domain-Inspector, QoS-Validator, Discovery-Snapshot |
| `zerodds-xmlc` | bin | DDS-XML-Validator, Schema-Checker, Deployment-Renderer |
| `zerodds-dashboard` | bin | Tauri-App für Live-Monitoring, Discovery-Graph, Replay-Browser |
| `zerodds-perf` | bin | Load-Generator, Latency-Profiler, Benchmark-Suite |
| `zerodds-traceability` | bin | Requirements-zu-Code-Matrix-Generator |

### 3.7 Meta-Tooling (Lint-Plugin)

| Crate | Typ | Verantwortung |
|---|---|---|
| `zerodds-lint` | lib | Custom Clippy-Lints (Projekt-Regeln gemaess `04_safety_by_architecture.md §3.4`). Kein Runtime-Code, nicht Safety-klassifiziert. Wird von CI als Clippy-Plugin geladen |

## 4 Abhängigkeits-Regeln

### 4.1 Erlaubte Abhängigkeitsrichtungen

- Jede Schicht darf von Schichten **unter** sich abhängen.
- Innerhalb einer Schicht dürfen Crates von anderen Crates derselben Schicht abhängen, solange keine Zyklen entstehen.
- `zerodds-sys` darf nur von `zerodds-rs`-Re-Export-Crates und `zerodds-dcps` direkt verwendet werden, um die C-ABI-Oberfläche sauber zu halten.

### 4.2 Verbotene Muster

- Binding-Crates (`zerodds-cpp`, `zerodds-cs`, `zerodds-java`, `zerodds-py`) dürfen **nicht** direkt auf Protocol- oder Transport-Crates zugreifen. Nur über `zerodds-sys` oder `zerodds-rs`.
- Safety-Crates dürfen **keine** Dependencies auf Standard- oder Comfort-Crates haben.
- Keine Crate darf direkt `tokio` als mandatory dep haben; stattdessen Executor-agnostisch via `futures::Stream`-Traits. Tokio ist nur in Comfort- und optionalen Standard-Builds verlinkt.

### 4.3 Third-Party-Dependency-Politik

- **Safe-Crates:** Whitelist-basiert. Erlaubte Crates: `heapless`, `bytes` (Safe-Subset), `zerocopy`, `byteorder`. Jede neue Dep erfordert explizite Begründung und Security-Review.
- **Standard-Crates:** Kuratierte Liste. Erlaubt: `serde`, `tokio` (optional feature), `tracing`, `thiserror`, `hex`, `sha2`, `ring` oder `rustls` je nach Security-Plugin. Neue Deps per Pull-Request-Review.
- **Comfort-Crates:** Offener, aber jede Dep in CI mit `cargo-audit`, `cargo-deny` und License-Check durchlaufen.

**`deny.toml`-Konventionen** (Quelle: Projekt-Root; Tweaks begruendet mit Inline-Kommentaren):

- `[licenses] allow = [...]` ist eine **Vorrats-Allowlist** (Apache-2.0, MIT, BSD-2/3, ISC, Unicode-3.0, Unicode-DFS-2016, CC0-1.0, Zlib, Apache-2.0-WITH-LLVM-exception). Eintraege bleiben, auch wenn aktuell kein Crate sie nutzt; `unused-allowed-license = "allow"` unterdrueckt die sonst ausgeloesten Warnings. GPL/AGPL sind implizit verboten — siehe `07_risks_and_strategy.md` §2.3.
- `[bans] wildcards = "deny"` bleibt aktiv, aber `allow-wildcard-paths = true` nimmt **workspace-interne `path = "../..."`-Deps** aus. Das ist die uebliche Cargo-Praxis fuer unveroeffentlichte Sub-Crates: sie haben keine `version = "..."` und wuerden sonst als Wildcard abgelehnt. Registry-Wildcards (`foo = "*"`) bleiben geblockt.
- `[advisories] yanked = "deny"`, `[sources] unknown-registry = "deny"`, `unknown-git = "deny"` bleiben hart — keine Software aus nicht-verifizierten Quellen, keine zurueckgezogenen Crates.

### 4.4 Unsafe-Code-Politik

Jede Safety-Klasse setzt ihren eigenen crate-weiten Default, der durch das
entsprechende Inner-Attribute in `src/lib.rs` durchgesetzt wird. Ausnahmen
sind nur in klar benannten, isolierten Modulen zulaessig — typischerweise
`mod ffi;` — und fordern dort einen lokalen Lint-Override **plus**
SAFETY-Kommentar pro `unsafe`-Block.

#### 4.4.1 Crate-weite Defaults nach Safety-Klasse

| Safety-Klasse | `lib.rs` Default | Ausnahmen erlaubt? |
|---|---|---|
| **Safe** | `#![forbid(unsafe_code)]` | Nein auf Crate-Ebene. Nur ueber strukturell separierte FFI-/Plugin-Module (siehe §4.4.3). |
| **Standard** | `#![deny(unsafe_code)]` | Ja, in `#[allow(unsafe_code)]`-markierten Modulen mit SAFETY-Kommentar-Pflicht. |
| **Comfort** | `#![warn(unsafe_code)]` | Ja, jeder `unsafe`-Block benoetigt SAFETY-Kommentar, CI-Lint `dds_require_safety_comment` (siehe `04_safety_by_architecture.md §3.4`). |

#### 4.4.2 SAFETY-Kommentar-Konvention

Jeder `unsafe`-Block, `unsafe fn`-Deklaration oder `unsafe impl` erfordert
einen unmittelbar davor stehenden `// SAFETY:`-Kommentar mit mindestens
einem Satz, der die Invarianten begruendet. Diese Regel wird durch
`dds_require_safety_comment` (Custom-Lint, `crates/lint`) erzwungen.

#### 4.4.3 FFI-Modul-Pattern

Crates mit C-ABI-Oberflaeche oder Sprach-Binding (`zerodds-sys`, `zerodds-cpp`,
`zerodds-cs`, `zerodds-java`, `zerodds-py`) trennen **Safe-Kern** von **FFI-Oberflaeche**
physisch:

- `src/lib.rs` behaelt den der Safety-Klasse entsprechenden Default
  (`forbid` fuer `zerodds-sys`, `deny` fuer Standard-Bindings, `warn` fuer
  Comfort-Bindings). Im `lib.rs` lebt nur sicher analysierbarer Rust-Code
  (Typen, Enum-Konstanten, Helper).
- `src/ffi.rs` (oder `src/ffi/` fuer groessere Oberflaechen) traegt auf
  Modul-Ebene `#![allow(unsafe_code)]` und exportiert die tatsaechlichen
  `extern "C"`-Funktionen, `#[no_mangle]`-Symbole, PyO3-Module oder
  P/Invoke-Stubs. Java braucht keine FFI-Schicht: ZeroDDS' Java-PSM
  (`zerodds-java-omgdds`) ist Pure-Java.
- Innerhalb des FFI-Moduls ist die SAFETY-Kommentar-Konvention (§4.4.2)
  weiterhin bindend. Zusaetzlich gilt in Safe-Crates und im Safe-Kern von
  `zerodds-sys`: der Aufruf-Pfad vom Safe-Kern ins FFI-Modul muss
  aufwaerts-frei sein (FFI darf Safe-Kern nutzen, Safe-Kern ruft nicht ins
  FFI-Modul).

#### 4.4.4 Sonderfall `zerodds-sys`

`zerodds-sys` traegt die Klassifikation **Safe (Kern)** trotz eingebauter
C-ABI. Aufloesung: der `lib.rs`-Kern (Typen, Opaque-Handles, Error-Codes)
ist vollstaendig Safe (`#![forbid(unsafe_code)]`, `#![no_std]`-faehig).
Die C-ABI-Exports leben in einem separaten `ffi`-Modul mit
`#![allow(unsafe_code)]` und gelten als **Binding-Oberflaeche** — nicht
als Teil des zertifizierbaren Kerns. Safety-Audits des `zerodds-sys`-Kerns
umfassen damit den `lib.rs`-Anteil; das `ffi`-Modul wird wie andere
Binding-Crates behandelt.

## 5 Workspace-Organisation

Der Root-`Cargo.toml` ist ein Virtual Workspace:

```toml
[workspace]
resolver = "2"
members = [
    "crates/foundation",
    "crates/cdr",
    "crates/types",
    "crates/qos",
    "crates/idl",
    "crates/transport",
    "crates/transport-udp",
    "crates/transport-tcp",
    "crates/transport-shm",
    "crates/rtps",
    "crates/discovery",
    "crates/security",
    "crates/dcps",
    "crates/rpc",
    "crates/xml",
    "crates/xrce-client",
    "crates/xrce-agent",
    "crates/recorder",
    "crates/monitor",
    "crates/rs",
    "crates/sys",
    "crates/cpp",
    "crates/cs",
    "crates/java",
    "crates/py",
    "crates/lint",
    "tools/idlc",
    "tools/admin",
    "tools/xmlc",
    "tools/dashboard",
    "tools/perf",
    "tools/traceability",
]

[workspace.package]
rust-version = "1.85"
edition = "2024"
license = "Apache-2.0"
repository = "https://…"

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
```

Crates haben konsistente `Cargo.toml`-Struktur mit gemeinsamen Package-Metadaten über `workspace = true`.

## 6 Feature-Flag-Grundregime

Globale Feature-Flags werden auf Workspace-Ebene konsistent genutzt:

| Flag | Bedeutung | Gated Crates |
|---|---|---|
| `std` | Nutzung von `std` erlaubt | Alle außer Safe-Kern |
| `alloc` | Nutzung von `alloc` erlaubt | Wie `std`, aber strenger |
| `safety` | Aktiviert alle No-Panic/No-Alloc-Regeln | Safe-Crates |
| `security` | DDS-Security aktiviert | `zerodds-dcps`, `zerodds-rtps` |
| `xtypes` | XTypes-Support | `zerodds-types`, `zerodds-rtps` |
| `tcp` | TCP-Transport aktiviert | `zerodds-transport-tcp` |
| `shm` | Shared-Memory-Transport | `zerodds-transport-shm` |
| `async-tokio` | Tokio-Runtime | Standard-Builds |
| `async-embassy` | Embassy-Runtime | Embedded |
| `otel` | OpenTelemetry-Emission | `zerodds-monitor` |
| `recording` | Wire-Recorder | `zerodds-monitor` |

Details der Profile-zu-Feature-Mapping in `03_profiles_and_platforms.md`.

## 7 API-Stabilitäts-Tiers

Nicht alle Public-APIs haben gleiche Stabilitäts-Garantien. Drei Tiers sind definiert:

| Tier | Crates | SemVer-Policy |
|---|---|---|
| **Tier 1: Stabile Binding-APIs** | `zerodds-sys`, `zerodds-cpp`, `zerodds-cs`, `zerodds-java`, `zerodds-rs` | Strikte SemVer. Breaking-Changes nur in Major-Releases. |
| **Tier 2: Kern-Runtime-APIs** | `zerodds-dcps`, `zerodds-security`, `zerodds-rpc` | SemVer, aber Breaking-Changes in Minor-Releases erlaubt vor 1.0. |
| **Tier 3: Interne APIs** | Alle anderen Protocol-, Transport-, Foundation-Crates | Interne Änderungen jederzeit möglich. User, die direkt auf diese Crates zugreifen, übernehmen Wartungs-Risiko. |

## 8 Test-Architektur

Jedes Crate hat drei Test-Ebenen:

1. **Unit-Tests in `src/`:** private Implementierungs-Tests, `#[cfg(test)]`-Module.
2. **Integration-Tests in `tests/`:** public-API-Tests, auch Compliance-Tests gegen OMG-Spec-Vektoren.
3. **Workspace-Ebene `xtests/`:** Cross-Crate-Integration, Interop-Tests gegen echte DDS-Peers (CycloneDDS, Fast DDS, RTI), End-to-End-Szenarien.

Test-Kategorien:

| Kategorie | Tool | Wann |
|---|---|---|
| Unit | `cargo test` | Bei jedem Commit, CI |
| Integration | `cargo test --test ...` | CI |
| Property-Based | `proptest`, `quickcheck` | CI, speziell für CDR und RTPS |
| Fuzz | `cargo-fuzz`, `AFL` | Nightly CI, speziell für Wire-Parser |
| Model-Checking | `kani` | Für Safe-Crates, Nightly CI |
| Interop | eigenes Harness mit Docker-compose | PR + nightly |
| Performance-Regression | Criterion.rs + Custom-Harness | Nightly, Alerts bei >5% Regression |

## 9 Claude-Teams-Kollaborations-Modell

Die Codebase ist explizit so strukturiert, dass agentische Entwicklung skaliert:

- **Crate-Level-Agents:** Je Crate kann ein dedizierter Claude-Agent arbeiten, ohne Konflikte mit anderen Crates. Crate-interne API-Änderungen bleiben lokal.
- **Spec-Sections als Arbeitspakete:** OMG-Spec-Sektionen mappen auf Code-Module mit `#[spec(...)]`-Annotationen. Ein Agent übernimmt eine Sektion, implementiert, testet gegen Spec-Vektoren.
- **Test-first-Workflow:** Agent liest OMG-Spec-Kapitel, generiert Conformance-Tests (property-based + example-based), implementiert gegen grüne Tests.
- **Review-Layer:** Menschliche Senior-Engineers reviewen architektonische Entscheidungen, Protocol-State-Machines und Safety-kritische Änderungen. Routine-Implementierungen werden Agent-to-Agent reviewt.
- **Dokumentations-Sync:** Claude-Teams halten diese Architektur-Dokumente mit Code synchron. Bei jedem relevanten Commit wird automatisch geprüft, ob Dokumentation aktualisiert werden muss.
