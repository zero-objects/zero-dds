# Layer-6-RC1 Spec-Coverage-Gaps + Vendor-Spec-Mapping

**Stand:** 2026-05-06
**Scope:** Layer-6-Bindings (zerodds-c-api, zerodds-cpp, zerodds-cs,
zerodds-py, zerodds-java/-jni/-omgdds, zerodds-ts-node, zerodds-ts-wasm,
zerodds-rs, zerodds-sys).

## Zweck

Dieses Dokument identifiziert die **Lücken zwischen Spec-Audit-Files
und der RC1-Code-Realität** und verweist pro Lücke auf die jeweilige
Vendor-Spec, die das Verhalten normativ definiert. Damit ist die
ZeroDDS-Decision-Position pro Item dokumentiert.

PROCESS.md verlangt nach großen Code-Wellen ein Item-für-Item
Re-Sync der betroffenen Audit-Files. Diese Datei ist der
Übergangs-Marker bis das Re-Sync durchgeführt ist.

## Vendor-Spec-Inventar (Layer-6-relevant)

| Vendor-Spec | Pfad | Status | Scope |
|---|---|---|---|
| **C-FFI Mini-Spec** | `docs/specs/zerodds-c-api-1.0.md` | Live RC1 | Volle 130+ FFI-Funktionen, kein OMG-Pendant |
| **Listener-Callbacks** | `docs/specs/zerodds-listener-callbacks-1.0.md` | Live Phase-1 | C-FFI-Listener-Pattern (vtable+user_data), kein OMG-C-Standard |
| **Async-API** | `docs/specs/zerodds-async-1.0.md` | Live | Async-DDS-API für Rust, kein OMG-Pendant |
| **IDL-Rust** | `docs/specs/zerodds-idl-rust-1.0.md` | Live | IDL→Rust-Codegen, ergänzt OMG-Lücke (kein OMG-IDL-Rust-PSM) |
| **CORBA-Rust** | `docs/specs/zerodds-corba-rust-1.1.md` | Live | CORBA-Mappings für Rust, kein OMG-CORBA-Rust-PSM |
| **Flatdata** | `docs/specs/zerodds-flatdata-1.0.md` | Live | Zero-Copy-Daten-Layout, kein OMG-Pendant |
| **Monitor** | `docs/specs/zerodds-monitor-1.0.md` | Live | Built-in DCPS-Monitor, ergänzt §2.2.5 |
| **Recorder** | `docs/specs/zddsrec-1.0.md` | Live | Sample-Recording, kein OMG-Pendant |
| **OTLP-Bridge** | `docs/specs/zerodds-observability-otlp-1.0.md` | Live | Observability-Bridge, kein OMG-Pendant |

## Spec-Coverage Gap-Tabelle

### A. `dds-psm-cxx-1.0.md` (Audit: 122 Items, 104 done + 18 n/a)

#### Phase-1 RC1: Surface-Live, Active-Wireup Phase-2

Items deren API-Signatur in `crates/cpp/include/dds/*.hpp` live ist, aber
deren *aktive Runtime-Verbindung* erst in Phase-2 kommt:

| §-Ref | Item | RC1-Status | Phase-2-Plan | Stützung |
|---|---|---|---|---|
| §7.5.9.5 | DataWriterListener::on_* | API-Surface live | Active-Wireup an Runtime-Status-Counter | `zerodds-listener-callbacks-1.0.md` §6 |
| §7.5.9.6 | DataReaderListener::on_* | API-Surface live | Active-Wireup | `zerodds-listener-callbacks-1.0.md` §6 |
| §7.5.13.7 | ContentFilteredTopic | Klasse + create-FFI | DataReader-Bind via cft_handle_ | `zerodds-c-api-1.0.md` §2.2 |
| §7.5.10.6 | ReadCondition<T> | Klasse + FFI | Active-Trigger via DataReader-Status | `zerodds-c-api-1.0.md` §6 |
| §7.5.10.7 | QueryCondition<T> | Klasse + FFI | SQL-Filter-Engine wired | `zerodds-c-api-1.0.md` §6 |

#### Phase-1 RC1: Defaults-only

Items die spec-konform deklariert sind aber im RC1 nur Defaults verwenden:

| §-Ref | Item | RC1-Status | Phase-2-Plan |
|---|---|---|---|
| §7.5.6 | QoS-Konstruktor-Argumente | Akzeptiert, aber Default-Pfad | Volle Field-Conversion via `qos_ffi`-Tabellen |
| §7.5.13.5 | Topic<T>(dp, name, qos) | Default-QoS aus Participant | QoS-Argument propagieren |

#### Vendor-Extension (kein OMG-C++-Pendant)

| Item | Vendor-Spec | Begründung |
|---|---|---|
| `topic_type_support<T>`-Trait-Pflicht | `zerodds-c-api-1.0.md` §2.5 | DDS-PSM-Cxx fordert nur `T` mit IDL-Codegen-Support; ZeroDDS akzeptiert auch raw-Bytes via `topic_type_support<core::ByteSeq>` |
| `_DataWriterListenerBridge<T>` | `zerodds-listener-callbacks-1.0.md` §7.1 | Brücke zwischen Spec-Klassen-Listener und C-FFI-vtable |

### B. `dds-java-psm-1.0.md` (171 Items, 156 done + 15 n/a)

#### Pure-Java ohne JNI (Vendor-Pfad)

Die Audit-Items die "Pure-Java-Implementation" als Implementations-Pfad annehmen
sind in ZeroDDS via `crates/java-omgdds/java/src/main/java/org/omg/dds/*`
*pure-Java* implementiert. Rust-side ist nur die Codegen-Pipeline
involviert (`crates/idl-java/`).

| Item-Kategorie | Spec-Annahme | RC1-Pfad | Stützung |
|---|---|---|---|
| Builtin-Typen | JNI/native-Lib | InProcessBus + Xcdr2Codec | `zerodds-java-omgdds`-Vendor-Spec |
| Discovery | JNI/native-Lib | InProcessBus für lokale-only Tests | Phase-2: gRPC-Bridge zu libzerodds-Server |
| RTPS-Wire | JNI/native-Lib | nicht in pure-Java | Vendor: Pure-Java verzichtet auf RTPS-Live; gRPC-Bridge-Pfad |

**Empfehlung:** Eigene Vendor-Spec `zerodds-java-omgdds-1.0.md`
schreiben, die den Pure-Java + gRPC-Bridge-Pfad als alternative
Implementations-Variante zur OMG-DDS-Java-PSM 1.0 dokumentiert.

### C. `idl4-cpp-1.0.md` (77 Items, 57 done + 20 n/a)

Audit ist solide. Die 20 n/a-Items sind alle "Spec-eigene non-binding
Aussagen" — kein Implementierungs-Soll. Kein RC1-Gap.

### D. `idl4-csharp-1.0.md` (81 Items, 66 done + 15 n/a)

Audit ist solide. Kein RC1-Gap.

### E. `idl4-java-1.0.md` (87 Items, 72 done + 15 n/a)

Audit ist solide. Kein RC1-Gap.

## C-FFI-Surface — Vendor-Spec-Coverage

`docs/specs/zerodds-c-api-1.0.md` deckt die **vollständige** ZeroDDS-
C-FFI ab. Die Tabelle unten mappt OMG-DDS-Spec-Sections auf Vendor-Spec-
Sections der C-FFI:

| OMG-DDS-Spec | Vendor-Spec-Section | RC1-Status |
|---|---|---|
| §2.2.2.2.1 DomainParticipant | C-API §2.2 | ✅ alle 16 Operationen |
| §2.2.2.3 Topic | C-API §2.5 | ✅ alle 6 Operationen |
| §2.2.2.4 Publisher+DataWriter | C-API §2.3+§2.6 | ✅ Pub: 8/16 Phase-1 + 8/16 Phase-2 |
| §2.2.2.5 Subscriber+DataReader | C-API §2.4+§2.7 | ✅ Sub: 8/14 Phase-1 + 6/14 Phase-2 |
| §2.2.4 Listeners | listener-callbacks Vendor-Spec | API-Surface Phase-1, Active Phase-2 |
| §2.2.4 Conditions+WaitSet | C-API §6 | ✅ GuardCondition + StatusCondition + ReadCondition + QueryCondition + WaitSet |
| §2.2.5 Built-in Topics | C-API §7 | ✅ alle 4 BuiltinTopicData |

## C-FFI-Funktions-Inventar (RC1)

Stand 2026-05-06 nach Phase-1-Welle:

```
factory_ffi.rs        :   7 Funktionen
participant_ffi.rs    :  16 Funktionen
topic_ffi.rs          :   5 Funktionen
publisher_ffi.rs      :  16 Funktionen (Pub + DW)
subscriber_ffi.rs     :  18 Funktionen (Sub + DR)
qos_ffi.rs            : 22 #[repr(C)] Strukturen + 7 Konvertierungen
condition_ffi.rs      :  12 Funktionen
builtin_ffi.rs        :   5 Funktionen
extra_ffi.rs          :  30 Funktionen (QoS get/set, Instance-Ops, R/T-Variants)
listener_ffi.rs       :  12 Funktionen + 6 Listener-Strukturen
─────────────────────────────────────────
Total                 : ~130 Funktionen + 28 Strukturen
```

`crates/zerodds-c-api/include/zerodds.h` (cbindgen-emittiert): 2852 LOC.

## Cross-Language-Test-Inventar (RC1)

| Crate | Tests grün | Pfad |
|---|---|---|
| `zerodds-c-api` | 56 cargo-tests | `cargo test -p zerodds-c-api` |
| `zerodds-cpp` | 1 cargo-test (compile+link+10 sub-asserts) | `cargo test -p zerodds-cpp` |
| `zerodds-cs` | 1 binary-run (8 sub-asserts) | `dotnet ZeroDDS.Tests.dll` |
| `zerodds-py` | 1 cargo-test | `cargo test -p zerodds-py --features extension-module` |
| `zerodds-java-omgdds` | 18 mvn-tests | `mvn test` |
| `java-omgdds` | 37 cargo-tests | `cargo test -p java-omgdds` |
| `zerodds-ts-node` | 4 ts-tests (1 SKIP) | `npm test` |
| `zerodds-rs` | 3 cargo-tests | `cargo test -p zerodds-rs` |
| `zerodds-sys` | 1 cargo-test | `cargo test -p zerodds-sys` |
| `zerodds-ts-wasm` | (lib only) | — |

**Total Layer-6 RC1: 130+ Tests grün, 0 failed.**

## Phase-2-Roadmap

Pro Layer-6-Crate die offenen Items für Phase-2:

### `zerodds-c-api`

- Listener-Active-Wireup an Runtime-Status-Counter (siehe
  `zerodds-listener-callbacks-1.0.md` §6).
- `dr_get_matched_publication_data` und `dw_get_matched_subscription_data`
  voll-wired (RC1: Unsupported-Stub).
- Loan-API (`dw_loan_message`/`commit_loan`/`discard_loan`) aktiv
  schalten — DCPS-Side hat den Mechanismus, FFI-Bridge fehlt.
- `wait_for_historical_data` für Transient-Local-Reader voll wiren.
- `read` vs `take`: aktuell aliased; Read-State-Cache pro Reader
  einbauen.

### `zerodds-cpp`

- Listener-Active-Wireup (sobald C-FFI Phase-2 fertig).
- ContentFilteredTopic-DataReader-Bind via `cft_handle_`.
- AnyDataWriter / AnyDataReader / AnyTopic für type-erased Pfade.
- QoS-Konstruktor-Argumente vollständig propagieren (statt Default).

### `zerodds-cs`

- IDataWriterListener-Active-Wireup (sobald C-FFI Phase-2 fertig).
- QueryCondition mit Filter-Expression aktiv (RC1: nur Surface).
- QoS-Konstruktor-Argumente vollständig propagieren.

### `zerodds-py`

- ContentFilteredTopic-Pyclass.
- QoS-Strukturen als Pyclass exponieren (statt nur Defaults).
- ReadCondition / QueryCondition Pyclass.

### `zerodds-java-omgdds`

- Vendor-Spec `zerodds-java-omgdds-1.0.md` schreiben, der
  Pure-Java + gRPC-Bridge-Pfad dokumentiert.
- gRPC-Bridge-Service-Proto in `crates/grpc-bridge/`.

### `zerodds-ts-node`

- Aktive `take` / `read` Implementation (aktuell nur Wrapper-Surface).
- Listener via koffi-callback.

## PROCESS.md Re-Sync-Pflicht

Nach Phase-2 abgeschlossen: pro betroffener Audit-File ein Item-für-Item
Re-Sync gegen die Spec-PDF, mit Update der `Status:`-Zeilen + dieser
Gap-Tabelle.

| Audit-File | Re-Sync nach Phase | Erwartete Aenderungen |
|---|---|---|
| `dds-psm-cxx-1.0.md` | Phase-2 (Listener active) | 5 Items von "done — API-Surface" → "done" |
| `dds-java-psm-1.0.md` | Phase-2 (gRPC-Bridge) | Vendor-Spec-Cross-Ref aktualisieren |
| `idl4-cpp-1.0.md` | nicht erwartet | — |
| `idl4-csharp-1.0.md` | nicht erwartet | — |
| `idl4-java-1.0.md` | nicht erwartet | — |
