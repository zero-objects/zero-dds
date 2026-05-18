# Phase-1 Smell/Bug-Audit — Querschnitt-Findings

**Datum:** 2026-04-20. **Scope:** Alle Crates unter `crates/`, Phase-1-
Abschluss-Stand. Fokus auf Findings, die pro-WP-Reviews systembedingt
nicht sehen konnten (Querschnitt ueber Crate-Grenzen).

## Severity-Tabelle

| # | Severity | Kategorie | File:Line |
|---|----------|-----------|-----------|
| 1 | **High** | Silent Failure — ReaderProxy-State-Drift | `rtps/src/reliable_writer.rs:238` |
| 2 | **High** | Comment/Code-Mismatch — recursion-depth Marker | `types/src/resolve.rs:252` + 17× in `idl/src/**` |
| 3 | **High** | Type-Convention-Drift — `Duration` doppelt | `qos/src/duration.rs:11` vs `rtps/src/participant_data.rs:36` |
| 4 | **High** | Type-Convention-Drift — `DurabilityKind` doppelt | `qos/src/policies/durability.rs:14` vs `rtps/src/publication_data.rs:28` |
| 5 | Med | Missing `#[non_exhaustive]` auf Error-Enums | `rtps/src/error.rs:7`, `transport/src/lib.rs:31,67`, `rtps/src/history_cache.rs:82`, `rtps/src/message_builder.rs:54`, `discovery/src/sedp/reader.rs:37`, `discovery/src/spdp.rs:24`, `types/src/error.rs:9`, `types/src/resolve.rs:28` |
| 6 | Med | Silent Lock-Failure — SHM-Registry | `transport-shm/src/registry.rs:78,95` |
| 7 | Med | Silent Lock-Failure — TCP push_inbound | `transport-tcp/src/tcp_transport.rs:297,313` |
| 8 | Med | Doc ueberverspricht — SilentDowngrade nur im Doc | `rtps/src/publication_data.rs:41-46` |
| 9 | Med | Silent Failure — Annotation-Parsing `unwrap_or_default` | `idl/src/semantics/annotations.rs:156,211,249,260,266,272` |
| 10 | Med | Padding-Reader ignoriert Nicht-Null-Bytes | `qos/src/wire_helpers.rs:22-24` |
| 11 | Med | `read_opt_string`/`read_opt_bytes` verwirft Mehrfach-Eintraege | `types/src/type_object/common.rs:444-447, 470-475` |
| 12 | Low | Dead-Code-Attribute ohne Issue-Ref | `types/src/assignability.rs:648`, `discovery/src/sedp/reader.rs:349`, 4× in `idl/src` |
| 13 | Low | Test-Formatter-Hack als Platzhalter | `transport-tcp/src/tcp_transport.rs:871` |
| 14 | Low | Platzhalter-Crates publizieren leere libs | `rpc/dcps/rs/cs/cpp/java/py/xml/sys/foundation/monitor/recorder/security/xrce-*` |
| 15 | Low | `if let Ok(...)` ohne else-Branch | `transport-tcp/src/tcp_transport.rs:165`, `transport-shm/src/registry.rs:78`, mehrere Tests |

## Details

**#1** `reliable_writer.rs:238` ruft `next_unsent_change` als Seiteneffekt, ignoriert den Return. Wenn `cache_max` nicht monoton waechst (Cache-Eviction, Race tick↔write), driftet Proxy↔Cache. Fix: `debug_assert_eq!(..., Some(sn))` oder expliziten Kommentar.

**#2** 17 Stellen in `idl/src/{ast,cst,grammar,lexer}` mit `zerodds-lint: recursion-depth 64`, teils auf Funktionen, deren reale Tiefe ≪ 64 ist. Umgekehrt: `types/assignability.rs:69,91` nutzt 64, aber `cfg.max_depth` ist runtime-konfigurierbar. Marker wird Dekoration statt Contract. Fix: Audit + reale Grenzen.

**#3/#4** `Duration` und `DurabilityKind` existieren doppelt in `qos` + `rtps`. Kein `From`/`TryFrom`. SEDP-Layer entscheidet ad-hoc. `qos::Duration` hat `INFINITE = i32::MAX`, `rtps::Duration` nicht. WP-1.5-Review war pro-Crate-Scope → uebersehen.

**#5** Nur 10 von ~20 public Error-Enums sind `#[non_exhaustive]`. `WireError`, `SendError`, `RecvError`, `CacheError`, `SedpReaderError`, `SpdpError`, `TypeCodecError`, `ResolveError` fehlen. Vor erstem Release fixieren — spaeter Breaking Change.

**#8** Doc warnt "silent-downgrade kann QoS-Violation bedeuten", Code enforced nichts. Aufrufer muss Verantwortung uebernehmen — unklar wer.

**#10** `read_bool_padded` akzeptiert jeden Pad-Byte-Wert. Spec: "MUST be zero". Interop-Risiko gegen strict Validators.

**#11** `read_opt_string`/`read_opt_bytes` unterstuetzen nur 1 Element, ignorieren `len > 1` via `for _ in 1..len { read…?; }`. Bei Mehrfach-Annotations → Datenverlust ohne Diagnostic.

**#14** 15 Platzhalter-Crates publizieren leere libs. Vor Phase-2-Start entweder `publish = false` oder loeschen.

## Positives

- `qos::pid`, `FragmentState`, `endpoint_match::Reason`, `TypeObject`, `TypeIdentifier`, `QosSet` sind `#[non_exhaustive]` — konsequent fuer WP-1.2/1.5-Kern.
- `qos::duration::Duration` implementiert `Ord`+`Hash`, `rtps::participant_data::Duration` nicht — kanonischer Kandidat fuer die Konsolidierung ist `qos`.
- Kein `assert!(true)` oder wirkungsloser Assert gefunden — Test-Disziplin gut.
- `unwrap_or_default` in `subscription_data.rs`/`publication_data.rs` ist spec-konformer QoS-Fallback, kein Error-Swallowing.
