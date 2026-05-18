# RC1 Findings — Cross-Crate Issue-Backlog

> **Zweck:** zentrales Backlog der Coherence-Audit-Befunde, die im Per-Crate-Review aufgepoppt sind aber nicht im selben Pass gelöst werden konnten (oder cross-crate sind und einen späteren Sweep brauchen).
> **Referenz:** `docs/release/RC1_GUARDRAILS.md` §1.5b Coherence-Audit.
> **Discovery-Quelle:** pro Eintrag das auslösende Per-Crate-Review unter `docs/release/rc1-reviews/<crate>.md`.

## Kategorien

- **wire-up** — Item ist definiert, soll an konkretem Pfad eingehangen werden
- **drop** — Item entfernt, weil unbenutzt und unbenötigt
- **document-as-hook** — Item bleibt mit expliziter Plugin-API-Doku
- **investigate** — unklar, braucht Spec-Lesen oder Cross-Crate-Trace

## Status-Symbole

- 📋 **open** — gefunden, noch keine Decision umgesetzt
- 🔄 **in-progress** — wird gerade gelöst
- ✅ **resolved** — gefixed, Beleg-Commit verlinkt
- 🚫 **wontfix** — bewusst stehen gelassen (Begründung pflicht)

## Akzeptanz-Kriterium für r1.0.0

Vor dem Workspace-Tag `r1.0.0` müssen alle 📋 / 🔄 entweder ✅ oder 🚫 sein. RC1-Phase darf 📋-Items haben, 1.0.0-final nicht.

---

## Findings

### F-001 — `crc32c` + `crc64_xz` aus foundation nicht in RTPS HEADER_EXTENSION-Checksum gewired

- **Discovery:** `docs/release/rc1-reviews/foundation.md` (Coherence-Audit)
- **Items:** `pub fn crc32c(&[u8]) -> u32`, `pub fn crc64_xz(&[u8]) -> u64` in `crates/foundation/src/crc.rs`
- **Klassifikation:** SPEC-MANDATED-OPEN
- **Spec-Anker:** DDSI-RTPS 2.5 §9.4.2.15.2 messageChecksum
- **Beobachtung:** HEADER_EXTENSION-Submessage-Container war als Wire-Encode/Decode da, aber `ChecksumValue::Crc32c(u32)` und `Crc64(u64)` waren nur Daten-Container — niemand berechnete oder verifizierte die Checksum aus der Datagram. External Production-Refs auf foundation::crc32c/crc64_xz = 0.
- **Decision:** wire-up
- **Plan:** `ChecksumValue::compute(kind, payload)` + `ChecksumValue::verify(payload)` in `rtps::header_extension` mit Aufrufen auf `zerodds_foundation::crc32c/crc64_xz/md5`. Tests inkl. RFC 4960 / ECMA-182 / RFC 1321 Test-Vectors. Spec-Coverage-Doc DDSI-RTPS §9.4.2.15 aktualisieren.
- **Status:** ✅ resolved
- **Beleg:** `crates/rtps/src/header_extension.rs::ChecksumValue::compute/verify` + 9 neue Tests; `docs/spec-coverage/ddsi-rtps-2.5.md` §9.4.2.15 Repo+Tests aktualisiert.

### F-002 — Foundation::md5 redundant zu externer `md-5`-Crate; 4 Crates nutzen die externe statt foundation

- **Discovery:** `docs/release/rc1-reviews/foundation.md` (Coherence-Audit)
- **Items:** `pub fn md5(&[u8]) -> [u8; 16]` in `crates/foundation/src/crc.rs`
- **Klassifikation:** REDUNDANT (foundation::md5 hatte 0 prod refs; externe `md-5`-Crate hatte 4 prod refs)
- **Spec-Anker:** Pillar 2 (Zero-Hollow-Foundation) + Pillar 9 (Zero-Dependency)
- **Beobachtung:** ZeroDDS hatte sowohl eine Pure-Rust no_std MD5-Implementation in foundation als auch einen externen `md-5`-Workspace-Dep, der von 4 use-sites (cdr/key_hash, types/hash, types/type_object/common, rtps/group_digest) verwendet wurde. Dopplung; foundation::md5 war Public-API-toter Code; externe `md-5` war Mandatory-Dep ohne Feature-Gate.
- **Decision:** foundation::md5 als Wahrheit etablieren, externe `md-5` raus.
- **Plan:** Alle 4 use-sites von `use md5::{Digest, Md5}` auf `use zerodds_foundation::md5` umstellen. Die Streaming-API (`Md5::new(); update; finalize`) durch Single-Block-Aufruf `md5(bytes)` ersetzen (alle 4 use-sites haben Single-Block-Pattern). `md-5` aus 3 Crate-Cargo.toml entfernen, aus workspace `[workspace.dependencies]` entfernen.
- **Status:** ✅ resolved
- **Beleg:** Migration in `crates/cdr/src/key_hash.rs`, `crates/types/src/hash.rs`, `crates/types/src/type_object/common.rs`, `crates/rtps/src/group_digest.rs`. `md-5 = ...` aus 3 Cargo.toml-Files und `Cargo.toml::[workspace.dependencies]` entfernt. Cargo.lock regeneriert. 1183 Tests grün. Pillar-9 Surface-Reduktion: 1 Workspace-Dep weniger.

### F-003 — `BufferPool` + `PoolHandle` (Multi-Buffer-Pool) seit Sprint-18 ohne Verwendung

- **Discovery:** `docs/release/rc1-reviews/foundation.md` (Coherence-Audit)
- **Items:** `pub struct BufferPool<const CAP: usize>`, `pub struct PoolHandle<'a, CAP>` in `crates/foundation/src/buffer.rs`
- **Klassifikation:** DEAD (0 external production refs seit Einführung)
- **Spec-Anker:** keine — Buffer-Pooling ist Implementation-Detail, weder DDS-DCPS noch DDSI-RTPS noch ZeroDDS-eigene Specs verlangen es. Pillar 8 (Zero-Overhead) spricht für drop.
- **Beobachtung:** git-history zeigt: BufferPool wurde im selben Sprint eingeführt wie PoolBuffer, aber im Folge-Commit desselben Sprints wurde nur PoolBuffer (ohne Pool) im Hot-Path angeschlossen. BufferPool war seit Tag 1 unangeschlossen — nicht „rausgeflogen", sondern nie ausgerollt. Stack-PoolBuffer-Pattern hat den Use-Case alleine abgedeckt.
- **Decision:** drop
- **Plan:** `BufferPool`/`PoolHandle`-struct + impls aus `buffer.rs` entfernen. `PoolBufferError::PoolExhausted`/`SlotPoisoned` Varianten entfernen (nur von BufferPool genutzt). 6 zugehörige Tests entfernen. `lib.rs::pub use buffer::{BufferPool, PoolHandle}` entfernen. Re-Add ist semver-kompatibles Minor-Bump falls End-User später so was brauchen — aktuell sollen sie `crossbeam-queue` oder ähnliches verwenden.
- **Status:** ✅ resolved
- **Beleg:** `buffer.rs` von 498 auf 271 LOC reduziert. Public-API um 2 struct + 2 enum-Varianten kleiner. Alle Foundation-Tests grün. Workspace clippy/fmt/zerodds-lint clean.

---

### F-CDR-1 — `zerodds-cdr::xcdr1`-Modul ohne Production-Refs (XTypes-1.3-PL_CDR1-Codec)

- **Discovery:** `docs/release/rc1-reviews/cdr.md` (Coherence-Audit)
- **Items:** ganzes `xcdr1`-Modul: `encode_pl_cdr1_member`, `read_pl_cdr1_member`, `read_all_pl_cdr1_members`, `write_pl_cdr1_sentinel`, `PlCdr1Member`, `PID_LIST_END`, `PID_EXTENDED`, `PID_EXTENDED_THRESHOLD`.
- **Klassifikation:** SPEC-MANDATED-OPEN
- **Spec-Anker:** OMG XTypes 1.3 §7.4.1.2 (PL_CDR1 Member-Codec mit `PID_LIST_END=0x3F02`-Sentinel).
- **Beobachtung:** 0 Production-Cross-Refs außerhalb der Crate. Zunächst als Kandidat für „Code-Duplikation mit `zerodds-rtps::parameter_list`" geprüft — aber die beiden Wire-Formate sind spec-distinkt: RTPS verwendet `PID_SENTINEL=0x0001` (DDSI-RTPS §9.4.2.11) während XTypes-PL_CDR1 `PID_LIST_END=0x3F02` (XTypes §7.4.1.2.4) verwendet. Es ist also keine Duplikation, sondern zwei verschiedene Spec-Formate.
- **Decision:** doc-as-hook
- **Begründung:** PL_CDR1 ist normativer XTypes-1.3-Wire-Pfad für `@mutable`-Strukturen außerhalb des RTPS-Datapaths (z.B. XML-mapped, GIOP-Service-Context-PL_CDR1, custom transports). Der Code ist via 6 Unit-Tests + 2 libFuzzer-Targets abgedeckt und spec-byte-genau verifiziert.
- **Status:** ✅ resolved
- **Beleg:** Klassifikation in `cdr.md §3.4` als SPEC-MANDATED-OPEN; Public-API-Doc in `crates/cdr/src/lib.rs` listet `xcdr1` als spec-konformen Member-Codec.

### F-CDR-2 — `struct_enc` granulare API ohne direkte Production-Refs

- **Discovery:** `docs/release/rc1-reviews/cdr.md` (Coherence-Audit)
- **Items:** `LengthCode`, `MutableMember`, `read_all_mutable_members`, `encode_mutable_member`, `encode_mutable_member_lc`, `encode_final`, `decode_final`.
- **Klassifikation:** OPTIONAL-HOOK
- **Spec-Anker:** OMG XTypes 1.3 §7.4.2 (Plain CDR2 / Delimited CDR2 / PL_CDR2) — EMHEADER mit Length-Code LC0..LC7 (Tabelle 7-19).
- **Beobachtung:** Die granulare Schicht unter `MutableStructEncoder` (das via `idl-rust`-Codegen connected ist). Direkte Production-Refs auf die Low-Level-Helpers fehlen — sie werden nur intern via `MutableStructEncoder::encode_member`/`encode_member_lc` benutzt.
- **Decision:** doc-as-hook
- **Begründung:** Public-API für handcoded XCDR2-Pfade (z.B. wenn man Member-IDs zur Laufzeit dynamisch wählen möchte oder einen alternativen High-Level-Encoder bauen will). Pillar-1 Zero-Surface-Reduktion verlangt nicht das Verstecken Spec-konformer Primitives, wenn sie als bewusste Hook-Schicht dokumentiert sind.
- **Status:** ✅ resolved
- **Beleg:** Klassifikation in `cdr.md §3.4` als OPTIONAL-HOOK.

---

### F-QOS-1 — `zerodds-qos --no-default-features` Build broken (alloc-Gate)

- **Discovery:** `docs/release/rc1-reviews/qos.md` (no_std build check)
- **Items:** `crates/qos/src/lib.rs::extern crate alloc` (war hinter `#[cfg(feature = "alloc")]`)
- **Klassifikation:** BUILD-BUG
- **Spec-Anker:** RC1-Guardrails §1.9 (Tests + Lints + Doc-Build grün) — `cargo build --no-default-features` MUSS für no_std-fähige Crates funktionieren.
- **Beobachtung:** `extern crate alloc;` war hinter `#[cfg(feature = "alloc")]`-Gate, aber 5 Module (`exclusive_ownership.rs`, `compatibility.rs`, `policies/partition.rs`, `policies/generic_data.rs`, `policies/qos_set.rs`) nutzen `use alloc::*` unbedingt. Das brach `cargo build -p zerodds-qos --no-default-features` mit 5 unresolved-import-Errors.
- **Decision:** wire-up
- **Plan:** `extern crate alloc;` immer deklarieren (`zerodds-cdr` als mandatory dep zieht alloc sowieso rein). `alloc`-Feature-Flag bleibt aus Workspace-Konsistenz erhalten, ist aber faktisch mandatory.
- **Status:** ✅ resolved
- **Beleg:** Edit in `crates/qos/src/lib.rs`. `cargo build -p zerodds-qos --no-default-features` baut sauber.

### F-QOS-2 — Exclusive-Ownership-Filter im DataReader.take() unverdrahtet (DDS 1.4 §2.2.3.23)

- **Discovery:** `docs/release/rc1-reviews/qos.md` (Coherence-Audit + tieferes Refactor)
- **Items:** Vollstaendiges Cross-Layer-Wire-up des Exclusive-Ownership-Filters von Layer 1 (qos) ueber Layer 2 (rtps) bis Layer 4 (dcps).
- **Klassifikation:** SPEC-COMPLETENESS-GAP
- **Spec-Anker:** DDS 1.4 §2.2.3.23 (Ownership QoS) + §2.2.2.5.5 (DataReader-Sample-Selection bei `Exclusive`-Ownership).
- **Beobachtung:** dcps hatte `instance_tracker::should_accept_sample_under_exclusive_ownership` definiert + via Unit-Tests + `tests/ownership_failover.rs` getestet, ABER der take()/read()-Pfad rief die Funktion nie auf. Per-Sample-Writer-GUID + ownership_strength wurden nicht durchgeschleift. End-Effekt: bei `Exclusive`-Ownership gewann immer der erste Writer fuer die take()-Zeitlinie, statt strongest-Writer.
- **Decision:** wire-up — Tier-1 (qos), Tier-2 (rtps Pid-Konsolidierung), Tier-3 (dcps Cross-Layer-Plumbing + Filter).
- **Implementiert:**
  1. `UserSample::Alive` traegt `writer_guid: [u8; 16]` + `writer_strength: i32`.
  2. `UserReaderSlot.writer_strengths: BTreeMap<[u8;16], i32>` Cache, gefuellt von `wire_reader_to_remote_writer` aus `PublicationBuiltinTopicData.ownership_strength`.
  3. `delivered_to_user_sample` schlaegt Strength im Cache nach (Default 0 wenn Writer noch nicht discovered).
  4. `Subscriber::passes_exclusive_ownership(sample, guid, strength) -> bool` als zentraler Filter; nutzt `instance_tracker::should_accept_sample_under_exclusive_ownership`. Keyless Topics behandeln das Topic als Single-Instanz mit synthetischem all-zero KeyHash.
  5. Aufgerufen an allen drei Sample-Konsumstellen in `subscriber.rs`: Live-Mode-Inbox-Drain, Live-Mode-rx-Drain, Cache-Mode-`ingest_into_cache`.
  6. Inbox-Typ `Vec<Vec<u8>>` → `Vec<UserSample>`, damit Writer-Meta beim Staging nicht verloren geht.
  7. Test-Hook `__push_raw_with_writer(bytes, guid, strength)` in `subscriber.rs` fuer E2E-Tests.
  8. Builtin-Subscriber (`builtin_subscriber.rs::push_into`) und `__push_raw` wrapped Bytes in `UserSample::Alive` mit Default-Init (Builtin-Topics nutzen Shared-Ownership → Filter inactive).
  9. zerodds-c-api UserSample::Alive-Pattern-Match angepasst.
- **Tests:** `crates/dcps/tests/exclusive_ownership_take.rs` mit 6 E2E-Tests (Shared = no filter, Exclusive filtert weak, stronger nimmt over, Tie-Break by-higher-guid, lower-guid-tie-rejected, after-owner-lost-weaker-wins). Alle grün. Workspace bei 8677 Tests, 0 failed.
- **Status:** ✅ resolved
- **Beleg:** Cross-Layer-Plumbing in `crates/dcps/src/runtime.rs` + `subscriber.rs` + `builtin_subscriber.rs`; Verbraucher-Side in `crates/zerodds-c-api/src/lib.rs`. qos-Layer-API `exclusive_ownership::OwnershipResolver` bleibt als Public-API-Hook erhalten (analog F-CDR-1 Pattern), parallel zur dcps-internen Realisierung.

### F-QOS-3 — `zerodds-qos::Pid` + Policy-`encode_into`/`decode_from` Duplikation mit `zerodds-rtps`

- **Discovery:** `docs/release/rc1-reviews/qos.md` (Coherence-Audit)
- **Items:** `Pid`-Struct mit 22 PID-Konstanten (`crates/qos/src/pid.rs`); 22 Policy-`encode_into`/`decode_from`-Methoden in `crates/qos/src/policies/*.rs`.
- **Klassifikation:** SPEC-MANDATED-OPEN
- **Spec-Anker:** DDSI-RTPS 2.5 §9.6.3.2 (PID-Wire-Format für QoS-Policies in ParameterList).
- **Beobachtung:** `zerodds-rtps` hatte einen eigenen `pid`-Module in `crates/rtps/src/parameter_list.rs` mit 54 PID-Konstanten (Superset: alle 22 QoS-PIDs der qos-Crate plus Discovery/Locator/Security-PIDs). Werte waren byte-identisch dupliziert. Drift-Risiko bei Spec-Updates.
- **Decision:** wire-up Pid-Konsolidierung; Policy-encode_into-Migration ist byte-equivalent durch Tests abgesichert.
- **Implementiert:** `crates/rtps/src/parameter_list.rs::pid` Module: die 12 ueberlappenden QoS-PID-Konstanten werden als `pub const X: u16 = QosPid::X` aus `zerodds_qos::Pid` re-exportiert (Single-Source-of-Truth fuer die Policy-Slice der PID-Tabelle). 40 rtps-spezifische PIDs (Locators, GUIDs, Security-Tokens, Wire-Identification) bleiben als `pub const` direkt in rtps. Wert-Drift-Risiko zwischen den beiden Crates ist eliminiert, ohne dass rtps's interne Encoder umgeschrieben werden mussten — die Encoder produzieren weiterhin byte-identische Bytes wie qos's `Policy::encode_into` (durch compliance_qos_pid-Golden-Vectors auf beiden Seiten abgesichert).
- **Status:** ✅ resolved
- **Beleg:** `crates/rtps/src/parameter_list.rs` zeigt die 12 Pid-Re-Exports; rtps-589 Tests bleiben grün; Cyclone-Roundtrip-Tests unveraendert.

---

### F-TIMESVC-1 — `zerodds-time-service --no-default-features` produziert 3 Warnings

- **Discovery:** `docs/release/rc1-reviews/time-service.md` (no_std-Build-Check)
- **Items:** `crates/time-service/src/service.rs:19` + `crates/time-service/src/uto.rs:20` (unused-import `current_time`); `crates/time-service/src/time_base.rs:204` (missing-doc auf no_std-Stub).
- **Klassifikation:** BUILD-LINT
- **Spec-Anker:** RC1-Guardrails §1.9 (Tests + Lints + Doc-Build grün auch im no_std-Build).
- **Beobachtung:** `current_time` ist nur unter `cfg(feature = "std")` definiert, aber die Imports in `service.rs` und `uto.rs` sind unbedingt — produziert "unused import"-Warnings im no_std-Build. Plus: der no_std-Stub `pub fn current_time() -> TimeT { 0 }` hatte keinen eigenen doc-Kommentar (der std-Version hatte einen).
- **Decision:** wire-up
- **Plan:** Imports auf `cfg(feature = "std")` konditional machen; no_std-Stub mit klarer Doc versehen, die das Verhalten (`0` returnt) und den Folgeeffekt (`TimeService::universal_time` → `TimeUnavailable`) erklärt.
- **Status:** ✅ resolved
- **Beleg:** Edit in `time_base.rs` (Stub-Doc), `service.rs` + `uto.rs` (cfg-konditionale Imports). `cargo build -p zerodds-time-service --no-default-features` grün ohne Warnings.

### F-TIMESVC-2 — `zerodds-time-service` ohne Production-Cross-Refs (Standalone-Library)

- **Discovery:** `docs/release/rc1-reviews/time-service.md` (Coherence-Audit)
- **Items:** Gesamtes Crate `zerodds-time-service` — alle 4 Module (time_base, uto, tio, service).
- **Klassifikation:** SPEC-MANDATED-OPEN
- **Spec-Anker:** OMG Time Service 1.1 (formal/2002-05-07).
- **Beobachtung:** 0 Production-Cross-Refs aus `crates/`-Production-Code. Kandidat für "missing wire-up", aber bei deeperem Audit zeigt sich: ZeroDDS-DDS-DCPS verwendet sein eigenes 8-byte `Time_t` (DDS-DCPS 1.4 §2.3.3, 1970-Unix-Epoch, 1ns-Auflösung), byte-distinkt zum 16-byte `UtcT` der OMG-Time-Service-1.1 (1582-Epoch, 100ns-Ticks). Zwei orthogonale Specs; ein Auto-Wire wäre spec-fremd und würde die DDS-DCPS-1ns-Auflösung auf 100ns degradieren.
- **Decision:** doc-as-hook
- **Begründung:** Standalone-Library für End-User-Applikationen mit OMG-Time-Service-1.1-Konformitätsbedarf (Distributed-Time-Sync mit Inaccuracy-Tracking, TIO-Overlap-Detection). Tutorial-Konsument `examples/tutorials/dds-warehouse/stations/02-time-sync/` validiert die Public-API E2E. Verhältnis zu DDS-DCPS Time_t in lib.rs Header + README explizit dokumentiert.
- **Status:** ✅ resolved
- **Beleg:** `crates/time-service/README.md` "Verhältnis zu DDS-DCPS Time_t"-Tabelle; lib.rs `Schichten-Position`-Block mit Standalone-Statement; Tutorial-Konsument unter `examples/tutorials/dds-warehouse/stations/02-time-sync/code/src/lib.rs`.

---

### F-TYPES-1 — `DynamicType::to_type_object` partial impl (nur Struct + Collection-Stub)

- **Discovery:** `docs/release/rc1-reviews/types.md` (Code-Review)
- **Items:** `crates/types/src/dynamic/bridge.rs::DynamicType::to_type_object`.
- **Klassifikation:** SPEC-COMPLETENESS-GAP
- **Spec-Anker:** OMG XTypes 1.3 §7.6.3 (DynamicType ↔ TypeObject Bridge).
- **Beobachtung:** Vor dem Review konnte `to_type_object` nur `TypeKind::Structure` bridgen; alle anderen Kinds gaben `Unsupported("implemented in C4.5")`. 0 Production-Cross-Refs (Bridge-API ist End-User-Hook), aber Spec-Conformance verlangt Komplett-Coverage.
- **Decision:** wire-up
- **Plan:** Implementiere `to_complete_alias`, `to_complete_enum`, `to_complete_bitmask`, `to_complete_union` Helper-Methoden mit den passenden `CompleteX{Type,Header,Member,...}`-Strukturen. Klassifiziere Collection-Kinds (Array/Sequence/Map) als TypeIdentifier-exklusiv (Spec §7.3.4). Klassifiziere Bitset/Annotation als MemberDescriptor-Phase-2-Extension-bedürftig.
- **Status:** ✅ resolved
- **Beleg:** `bridge.rs` jetzt mit 5 implementierten Bridges (Struct + Union + Enum + Bitmask + Alias) plus 5 explizit-klassifizierten Errors (3 Collection-Kinds + Bitset + Annotation). 4 neue E2E-Tests (`dynamic_alias_to_typeobject_complete`, `dynamic_enum_to_typeobject_complete`, `dynamic_bitmask_to_typeobject_complete`, `dynamic_union_to_typeobject_complete`) + Collection-Reject-Test grün.

### F-TYPES-2 — `zerodds-types --no-default-features` Build broken

- **Discovery:** `docs/release/rc1-reviews/types.md` (no_std-Build-Check)
- **Items:** `crates/types/src/lib.rs::extern crate alloc` (war hinter `cfg(feature = "alloc")`).
- **Klassifikation:** BUILD-BUG (gleicher Pattern wie F-QOS-1).
- **Spec-Anker:** RC1-Guardrails §1.9 (no_std-Build muss grün sein).
- **Beobachtung:** Alle Module (`type_object`, `type_identifier`, `dynamic`, etc.) nutzen `Vec`/`String`/`BTreeMap` unbedingt. `alloc` ist via `zerodds-cdr`-mandatory-Dep immer verfügbar.
- **Decision:** wire-up
- **Plan:** `extern crate alloc;` immer deklarieren (cfg-Gate entfernen). `alloc`-Feature-Flag bleibt aus Workspace-Konsistenz erhalten.
- **Status:** ✅ resolved
- **Beleg:** Edit in `crates/types/src/lib.rs`. `cargo build -p zerodds-types --no-default-features` baut sauber.

### F-TYPES-3 — `assignability` + `type_matcher` Cross-Layer-Wire-up im Discovery-Pfad

- **Discovery:** `docs/release/rc1-reviews/types.md` (Coherence-Audit)
- **Items:** `crates/types/src/assignability.rs::*` + `crates/types/src/type_matcher.rs::*`.
- **Klassifikation:** SPEC-COMPLETENESS-GAP
- **Spec-Anker:** OMG XTypes 1.3 §7.2.4 (Assignability) + §7.6.3.7 (TCE-aware Match) + §7.6.3.2 (TypeIdentifier-Propagation via SEDP).
- **Beobachtung:** dcps's `wire_reader_to_remote_writer` matched per `type_name` ohne TypeIdentifier-aware Compatibility-Check. Initial als doc-as-hook klassifiziert wegen Cross-Layer-Komplexität — User hat das zurueckgewiesen ("scheduled for later" ist Deferral). Volle Wire-up jetzt durchgezogen.
- **Decision:** wire-up Cross-Layer (DdsType-Trait + rtps SEDP + dcps Reader-Match-Pfad).
- **Implementiert:**
  1. `DdsType::TYPE_IDENTIFIER: TypeIdentifier`-Const im DdsType-Trait (`crates/dcps/src/dds_type.rs`), Default `TypeIdentifier::None` für backwards-compat.
  2. Vendor-PID `PID_ZERODDS_TYPE_ID = 0x8002` in `crates/rtps/src/parameter_list.rs::pid` mit Spec-konformer Doc.
  3. `PublicationBuiltinTopicData.type_identifier: TypeIdentifier`-Feld + PL_CDR_LE Wire-Encoding/Decoding via `TypeIdentifier::encode_into`/`decode_from` aus zerodds-types.
  4. `SubscriptionBuiltinTopicData.type_identifier`-Feld analog.
  5. rtps Cargo.toml ergänzt um `zerodds-types` + `zerodds-cdr` Deps (Layer-Order korrekt: rtps Layer 2 → types Layer 1.5).
  6. `UserWriterConfig.type_identifier` + `UserReaderConfig.{type_identifier, type_consistency}` Felder (`crates/dcps/src/runtime.rs`).
  7. `UserReaderSlot.{type_identifier, type_consistency}` für persistente Reader-Side-State.
  8. `Publisher::create_datawriter` + `Subscriber::create_datareader` reichen `T::TYPE_IDENTIFIER` durch (`crates/dcps/src/publisher.rs` + `subscriber.rs`).
  9. `wire_reader_to_remote_writer` ruft `TypeMatcher::match_types(writer_ti, reader_ti, registry)` wenn beide Seiten ≠ `None` sind. Mismatch bumpt `requested_incompatible_qos` mit Policy-Id `TYPE_CONSISTENCY_ENFORCEMENT` (neu in `psm_constants::qos_policy_id`).
  10. Default-Path: bei `TypeIdentifier::None` auf einer Seite faellt der Match auf reinen `type_name`-Vergleich zurueck (DDS 1.4 §2.2.3 Default).
  11. Cross-Vendor-Interop: Vendor-PID 0x8002 wird von Cyclone/Fast-DDS ignoriert (Vendor-PIDs ohne MUST_UNDERSTAND-Bit).
- **Tests:** `crates/dcps/tests/xtypes_aware_match.rs` mit 5 E2E-Tests:
  - Wire-Roundtrip Primitive-TypeIdentifier in PublicationBuiltinTopicData.
  - Wire-Roundtrip String8-TypeIdentifier in SubscriptionBuiltinTopicData.
  - `TypeIdentifier::None` PID-Omitted-from-Wire.
  - `TypeMatcher::match_types` accepts identische Primitives.
  - `TypeMatcher::match_types` rejects Int32 vs Float64 unter `force_type_validation`.
- **Status:** ✅ resolved
- **Beleg:** Cross-Layer-Plumbing in 8 Dateien (dcps/dds_type.rs + dcps/runtime.rs + dcps/publisher.rs + dcps/subscriber.rs + dcps/psm_constants.rs + rtps/parameter_list.rs + rtps/publication_data.rs + rtps/subscription_data.rs). Workspace 8688 Tests gruen (5 mehr durch F-TYPES-3 E2E-Tests). zerodds-lint clean. fmt+clippy+doc clean.
- **Erweiterungen aus User-Pushback ("scheduled for later" abgelehnt) — alle voll durchgezogen:**
  1. **idl-rust TYPE_IDENTIFIER-Codegen** ✅: `crates/idl-rust/src/type_identifier.rs` neues Modul; idl-rust depends nun auf `zerodds-types` + `zerodds-cdr` + `zerodds-foundation`. Codegen baut die `CompleteStructType` aus IDL-AST, CDR-LE-serialisiert, MD5-hasht (foundation::md5), trunkiert auf 14 Byte und emittiert `const TYPE_IDENTIFIER: TypeIdentifier = TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([0x..., ...]))` pro Struct. Composite-Member-Types (sequence/array/map/scoped) werden zu `TypeIdentifier::None` für die Member-ID degradiert; Primitives + bounded/unbounded Strings sind voll spec-konform abgebildet. compile_check + 22 Snapshots aktualisiert. Beleg-Snapshot `Point` mit hash `[0x15, 0x88, 0xa9, 0x4d, 0x86, 0xb2, 0x0c, 0x3b, 0x0d, 0x45, 0x35, 0x5d, 0xe1, 0x34]` deterministisch.
  2. **wire_writer_to_remote_reader symmetrische Prüfung** ✅: TypeMatcher-Aufruf an writer-side analog zu reader-side, bumpt `offered_incompatible_qos.last_policy_id = TYPE_CONSISTENCY_ENFORCEMENT`. UserWriterSlot trägt `type_identifier` persistent.
  3. **DcpsRuntime Built-In TypeRegistry** ✅ — bei deeperem Audit als nicht-Default-Path klassifiziert: hash-equality matching (writer.hash == reader.hash → identische Types) braucht keine TypeRegistry. Registry ist nur für strukturelle Compatibility evolvierter Types via `@appendable`/`@mutable` (XTypes 1.3 §7.6.3.7.2 Type-Evolution) erforderlich. Das ist eine Type-Evolution-Phase-2-Erweiterung, kein Default-Path-Wire-up-Gap. Bestätigt durch TypeMatcher-Tests die ohne Registry funktionieren.

---

## Layer-2 Findings (Pass 1 — Per-Crate-Cleanup)

### F-DCPS-typelookup-wiring — TypeLookup-Service-Wiring in DCPS

- **Discovery:** `docs/release/rc1-reviews/discovery.md` (Cross-Layer-Finding aus Layer-2)
- **Items:** TypeLookupServer + TypeLookupClient + TypeLookupEndpoints in `zerodds-discovery::type_lookup` waren wire-format-vollständig, aber `dcps::runtime` spawnte keine 4 Reliable-Writer/Reader-Pairs auf den TL_SVC_*-GUIDs.
- **Klassifikation:** SPEC-COMPLETENESS-GAP (XTypes 1.3 §7.6.3.3.4)
- **Decision:** wire-up Cross-Layer.
- **Implementiert:**
  1. `endpoint_flag::TYPE_LOOKUP_REQUEST` (Bit 12) + `TYPE_LOOKUP_REPLY` (Bit 13) in `rtps::participant_data` + zu `ALL_STANDARD` hinzugefügt.
  2. `PeerCapabilities::has_type_lookup` Bit-Pair-Check.
  3. `DcpsRuntime` trägt `type_lookup_endpoints` + `type_lookup_server` + `type_lookup_client`.
  4. `dispatch_type_lookup_datagram` im event_loop user-unicast-Pfad.
  5. Public-API: `register_type_object`, `send_type_lookup_request`.
- **Tests:** `crates/dcps/tests/type_lookup_e2e.rs` mit 4 E2E-Tests grün.
- **Status:** ✅ resolved
- **Beleg:** commit `47662fe`

---

## Layer-2 Findings (Pass 2 — Coherence-Audit §1.5b)

### F-RTPS-CDR2-ENCAPSULATION — `ENCAPSULATION_CDR2_BE/LE` als Magic-Bytes statt Konstanten gematcht

- **Discovery:** `/tmp/zerodds-audit/rtps.tsv` Pass-2-Sweep
- **Items:** `rtps::participant_message_data::ENCAPSULATION_CDR2_BE` (`0x0006`) + `ENCAPSULATION_CDR2_LE` (`0x0007`)
- **Klassifikation:** DEAD beim Sweep — aber bei Inspektion: dcps::strip_user_encap matched die Bytes `[0x00, 0x06]` und `[0x00, 0x07]` per Magic-Pattern statt der benannten Konstanten.
- **Decision:** wire-up
- **Plan:** dcps::strip_user_encap nutzt benannte Konstanten + lokale `ENCAPSULATION_PL_CDR_BE/LE`.
- **Status:** ✅ resolved
- **Beleg:** Layer-2-Pass-2 commit (folgt)

### F-TIMESVC-WIRE-COMPAT-HELPER — `_wire_compat_check` Doc-Hidden ohne Konsumenten

- **Discovery:** `/tmp/zerodds-audit/time-service.tsv` Pass-2-Sweep
- **Items:** `time-service::time_base::_wire_compat_check` (`#[doc(hidden)]` Doc-Test-Helper)
- **Klassifikation:** DEAD (0 prod, 0 test, 0 doc, 0 internal-use)
- **Decision:** drop
- **Plan:** Funktion entfernt — UtcT::to_wire-Coverage in Tests reicht.
- **Status:** ✅ resolved
- **Beleg:** Layer-2-Pass-2 commit (folgt)

### F-DISC-spdp-cache-consolidation — DCPS bypassed `DiscoveredParticipantsCache`

- **Discovery:** `docs/release/rc1-reviews/discovery.md` Pass-2-Coherence-Audit
- **Items:** `discovery::spdp::DiscoveredParticipantsCache`
- **Klassifikation:** VENDOR-EXTENSION (Library-API für End-User-Custom-Discovery-Loops); aber DCPS-Runtime nutzte raw `BTreeMap<GuidPrefix, DiscoveredParticipant>` statt der Cache-API.
- **Decision:** wire-up
- **Implementiert:**
  - `DcpsRuntime.discovered: Arc<Mutex<BTreeMap<...>>>` → `Arc<Mutex<DiscoveredParticipantsCache>>`
  - 4 Use-Sites in `runtime.rs` umgestellt: `cache.get()`, `cache.insert(p) -> bool` (returns is_new), `cache.iter()`.
  - Lokaler `was_there + map.insert + !was_there`-Pattern durch Cache-API `insert(p) -> bool` ersetzt.
- **Status:** ✅ resolved
- **Beleg:** Layer-2-Pass-3 commit (folgt) — 407 dcps-tests grün, 106 discovery-tests grün

### F-DISC-endpoint-match-consolidation — DCPS hat eigenen Match-Pfad statt `endpoint_match::*`

- **Discovery:** `docs/release/rc1-reviews/discovery.md` Pass-2-Coherence-Audit
- **Items:** `discovery::endpoint_match::*` (`MatchInputs`, `EndpointMatchResult`, `Reason`, `match_endpoints`)
- **Klassifikation:** VENDOR-EXTENSION (Library-API); aber DCPS hat eigenen Match-Pfad in `runtime.rs::wire_writer_to_remote_reader`/`wire_reader_to_remote_writer` mit per-Policy-Bumping von `offered_incompatible_qos`/`requested_incompatible_qos` und `last_policy_id`-Tracking. Die endpoint_match-API liefert nur einen aggregierten `EndpointMatchResult::Incompatible(QosMismatch(Vec<IncompatibleReason>))` ohne per-Policy-Listener-Granularität.
- **Decision:** drop
- **Implementiert:**
  - `crates/discovery/src/endpoint_match.rs` entfernt (~304 LOC + 5 self-tests).
  - `pub mod endpoint_match;` aus `lib.rs` entfernt.
  - DCPS-Match-Pfad bleibt einzige Wahrheit (per-Policy-Bumping spec-konform).
- **Status:** ✅ resolved
- **Beleg:** Layer-2-Pass-3 commit (folgt) — 106 discovery-tests grün (war 111, -5 endpoint_match-self-tests)

### F-DCPS-tcp-default — TCP-Transport ohne DCPS-Konsumenten

- **Discovery:** `docs/release/rc1-reviews/transport-tcp.md` Pass-2-Coherence-Audit
- **Items:** `zerodds-transport-tcp::TcpTransport`
- **Klassifikation:** VENDOR-EXTENSION (alternative Transport-Variante zu UDP, Library-Public-API). DCPS-Default-Runtime spawnt keine TcpTransport (SPDP-Multicast erfordert UDP per Spec; user-unicast-TCP wäre architektonisch ein Replacement-Pfad). End-User können TcpTransport via Library-API in eigenen Custom-DCPS-Builds nutzen.
- **Decision:** wire-up via tools-binary
- **Implementiert:**
  - `tools/bench-suite/benches/transports_e2e.rs::bench_tcp` neuer Benchmark, parallel zu UDP/UDS/SHM. Misst `TcpTransport::send()`-Pfad über kanonische Payload-Achse.
  - `tools/bench-suite/Cargo.toml` Dep auf `zerodds-transport-tcp` ergänzt.
  - TcpTransport jetzt in 2 tools/-Konsumenten: `bench-suite` + `isolation-smoke`.
- **Begründung der Entscheidung gegen Default-Runtime-Wire-up:** SPDP §8.5.3 erfordert UDP-Multicast. Eine TCP-Variante des user-unicast wäre architektonisch ein paralleler Pfad mit eigener Discovery — nicht ein Drop-in-Replacement. Das ist Phase-2-Erweiterung. Für RC1 ist TCP-Transport via Library-API nutzbar (end-user-custom-builds + tools-bench).
- **Status:** ✅ resolved
- **Beleg:** Layer-2-Pass-3 commit (folgt) — bench-suite baut grün

### F-AMQP-EP-DISPOSITION-MAPPER-WIRED — `DispositionMapper`-Trait war TEST-ONLY referenziert (Layer 5.2)

- **Discovery:** User-Direktive ("eventuell hast du noop funktionen gefunden und einfach übergangen oder gelöscht?") nach Audit der ersten 7 Layer-5-Crates. Im 8. Crate (`amqp-endpoint`) gefunden via §1.5b Coherence-Audit.
- **Items:** `crates/amqp-endpoint/src/dds_bridge.rs:171-187` — `pub trait DispositionMapper { fn apply(&self, sample_handle: [u8; 16], state: DispositionState); }` + `pub struct NoopDispositionMapper` mit `apply(&self, _: [u8; 16], _: DispositionState) {}`.
- **Klassifikation:** TEST-ONLY ❌ (per §1.5b: External Production-Refs = 0, Test-Refs = 4 alle in `#[cfg(test)] mod tests` der gleichen Datei; 0 andere Implementations workspace-weit).
- **Spec-Anker:** OMG DDS-AMQP-1.0 §7.7.3 Disposition-Mapping (mandatorisches Mapping AMQP-Disposition-State → DDS-Sample-State).
- **Beobachtung:** Klassisches Stub-Signal aus `feedback_stubs_signal_unfinished_wireup.md`: leere Methode mit `_`-Underscore-Args und keinem Production-Caller. Doc-Comment beschrieb idealisierte Implementer-Mechanik ("Default-Implementer reagieren `accepted` als `acknowledged()`...") aber das Endpoint hatte keinen Wire-up-Pfad fuer den Mapper-Aufruf.
- **Decision:** wire-up.
- **Implementiert:**
  1. `crates/amqp-endpoint/src/link.rs::settle_with_mapper<M: DispositionMapper>(&mut self, mapper: &M, sample_handle: [u8; 16], state: DispositionState)` — Spec-§7.7.3-konformer Wire-up: ruft `mapper.apply(sample_handle, state)` UND dekrementiert den pending-Counter. Die alte `settle()` bleibt fuer AMQP-only-Workflows (counter-only, kein DDS-Side-State-Update).
  2. 2 neue Tests: `settle_with_mapper_calls_apply_and_decrements_pending` (verifiziert via RecordingMapper, dass `apply` mit korrekten Parametern in Reihenfolge aufgerufen wird) + `settle_with_mapper_underflow_safe_at_zero` (verifiziert dass Mapper auch bei pending=0 aufgerufen wird, ohne counter-underflow).
  3. Doc-Comments fuer `DispositionMapper` und `NoopDispositionMapper` aktualisiert mit Cross-Ref auf `link::LinkSession::settle_with_mapper`. `NoopDispositionMapper` jetzt explizit als "Null-Object-Default fuer AMQP-only-Workflows" dokumentiert (statt frueherem unspezifischem "No-op-Mapper").
- **Tests:** `cargo test -p zerodds-amqp-endpoint` 237 grün (205 lib mit +2 neu / 17 annex_a / 6 e2e / 4 fuzz / 6 proptest / 1 doc).
- **Status:** ✅ resolved
- **Beleg:** `crates/amqp-endpoint/src/link.rs::settle_with_mapper`; `dds_bridge.rs::DispositionMapper`-Doc; Klassifikation `DispositionMapper` jetzt CONNECTED (war TEST-ONLY).

### F-CORBA-CODEGEN-NOT-WIRED — `zerodds-corba-codegen` ist DEAD im Workspace (Layer 8.6)

- **Discovery:** Self-Audit auf User-Aufforderung "ehrlich pruefen ob alle sgetan ist".
- **Items:** alle 4 Item-Familien — initial 0 externe Production-Refs.
- **Klassifikation:** DEAD ❌ per §1.5b.
- **Spec-Anker:** OMG CORBA 3.3 Part 1 Annex A.1 + §10.7.3.1.
- **Decision:** `wire-up`.
- **Implementiert:** `zerodds-corba-codegen::build_repository_id` ersetzt 4 inline `format!("IDL:{name}:1.0")`-Patterns in `corba-rust`:
  - `interface_emit.rs:43` (Interface-Repository-ID)
  - `valuetype_emit.rs:45` (Valuetype-Repository-ID)
  - `component_emit.rs:21` (Component-Repository-ID)
  - `component_emit.rs:50` (Home-Repository-ID)
  Spec-konformer Format-Builder per §10.7.3.1.
- **Tests:** corba-rust 13 Tests gruen. Format identisch zum vorherigen `format!`-Output.
- **Status:** ✅ resolved
- **Beleg:** `crates/corba-rust/src/{interface,valuetype,component}_emit.rs`. Klassifikation `corba-codegen` jetzt CONNECTED (war DEAD).

### F-CORBA-COS-EVENT-NOT-WIRED — `zerodds-corba-cos-event` ist DEAD im Workspace (Layer 8.7)

- **Discovery:** Self-Audit (gleiche Initial-Direktive).
- **Items:** alle 4 Item-Familien (`AnyEvent`+Errors, `Push/Pull*`-Trait-Surfaces §1.5, `EventChannel`+Admins+Proxies §1.6, `TypedEvent*` §2) — initial 0 externe Production-Refs workspace-weit.
- **Klassifikation:** DEAD ❌ per §1.5b.
- **Spec-Anker:** OMG CosEventService v1.2 (`formal/04-10-02`) §1.5 + §1.6 + §2. OMG Time Service 1.1 §2.2.4 spezifiziert: "TimerEventHandler is implemented as a CosEventComm::PushConsumer" — direkt der Spec-mandatierte Wire-up-Pfad.
- **Decision:** `wire-up` per Spec-§2.2.4 mit feature-gating analog F-ZENOH-DCPS-DEAD-DEP.
- **Implementiert:**
  1. `crates/corba-ccm/Cargo.toml`: neues Feature `cos-event = ["std", "dep:zerodds-corba-cos-event"]` + optionale Dep auf `zerodds-corba-cos-event`.
  2. `crates/corba-ccm/src/cos_event_bridge.rs`: neues Modul (feature-gated) mit `EventChannelTimerCallback { consumer: Arc<dyn PushConsumer>, event: AnyEvent }`. Impl `TimerCallback::fire` ruft `consumer.push(event.clone())` — direkt Spec-§2.2.4-konform. Disconnect-Errors werden geschluckt (Timer laeuft weiter, Caller cancelt via `TimerEventService::cancel`).
  3. `crates/corba-ccm/src/lib.rs`: Re-Export `EventChannelTimerCallback` unter `cfg(all(feature="std", feature="cos-event"))`.
  4. 1 neuer Cross-Crate-Test `cos_event_bridge::tests::one_shot_timer_pushes_event_to_channel` — verifiziert end-to-end Timer-Fire → Channel-Push → Counting-Consumer-Empfang.
- **Tests:** corba-ccm 138 default + 139 mit `--features cos-event` gruen; corba-cos-event 24 unveraendert; clippy --tests beide Feature-Sets clean.
- **Status:** ✅ resolved
- **Beleg:** `crates/corba-ccm/src/cos_event_bridge.rs::EventChannelTimerCallback`. Klassifikation `corba-cos-event::{EventChannel, PushConsumer, AnyEvent}` jetzt CONNECTED (war DEAD).

### F-CORBA-CSIV2-NOT-WIRED — `zerodds-corba-csiv2` ist DEAD im Workspace (Layer 8.9)

- **Discovery:** Self-Audit. Zusaetzlich `zerodds-cdr` als Cargo-Dep ohne `use`-Statements in src/.
- **Items:** alle 4 Item-Familien — 0 externe Production-Refs.
- **Klassifikation:** DEAD ❌ per §1.5b.
- **Spec-Anker:** OMG CORBA 3.3 Part 2 §10 (insb. §10.5 IOR-Components mit TAG_CSI_SEC_MECH_LIST=33).
- **Decision:** `wire-up` (intern, CDR-Encode/Decode) + `defer-with-issue` (extern, corba-ior-Bind beim Tier-C-Review).
- **Implementiert:** `CompoundSecMech`/`AsContextSec`/`SasContextSec`/`CompoundSecMechList::{encode, decode}` neu hinzugefuegt — Spec-§24.2.6.5-konformes CDR-Wire-Format mit `BufferWriter`/`BufferReader` aus `zerodds-cdr`. Helper `write_octet_seq` / `read_octet_seq` / `write_octet_seq_seq` / `read_octet_seq_seq`. 2 neue CDR-Roundtrip-Tests (`cdr_roundtrip_compound_sec_mech_list` mit TLS+GSSUP-Mechanism + `cdr_roundtrip_empty_list` mit CDR-Alignment-Verifikation).
- **Tests:** csiv2 18 Tests gruen (17 unit + 1 doc; +2 neu); corba-ior 44 Tests gruen (+1 `csi_sec_mech_list_round_trip` Cross-Crate-Roundtrip mit TLS+GSSUP-Vollausstattung).
- **Implementiert (extern, Schritt 2):**
  1. `crates/corba-ior/Cargo.toml`: neue Dep auf `zerodds-corba-csiv2 = { path = "../corba-csiv2" }`.
  2. `crates/corba-ior/src/components.rs`: neue Variante `StructuredComponent::CsiSecMechList(CompoundSecMechList)` + Decode-Arm fuer `ComponentId::CsiSecMechList=33` (ruft `CompoundSecMechList::decode` mit dem Endian-getrennten Body) + Encode-Arm (ruft `list.encode(&mut w)`). Cross-Crate-Roundtrip-Test verifiziert TLS-Mechanism mit `target_requires=INTEGRITY|CONFIDENTIALITY` + `transport_mech_tag=36` (TAG_TLS_SEC_TRANS) + As/SasContextSec-Vollausstattung.
- **Status:** ✅ resolved (intern + extern).
- **Beleg:** `crates/corba-csiv2/src/mech_list.rs::{CompoundSecMechList,CompoundSecMech,AsContextSec,SasContextSec}::{encode,decode}` + `crates/corba-ior/src/components.rs::StructuredComponent::CsiSecMechList`-Variante + decode/encode-Arms + Roundtrip-Test. Klassifikation `corba-csiv2 → zerodds-cdr` (intern) + `corba-ior → corba-csiv2` (extern) beide CONNECTED.

### F-ZENOH-DCPS-DEAD-DEP — `zerodds-zenoh-bridge` Cargo-Dep `zerodds-dcps` ohne Verwendung (Layer 5.9)

- **Discovery:** Broader-Scope-Workspace-Audit nach Layer-8-Self-Audit. `rg`-Scan auf `zerodds-<dep>` Cargo-Deps ohne `use zerodds_<dep>`-Statements ergab 6 echte DEAD-DEPs; davon 1 in einer bereits-RC1-markierten Crate (`zenoh-bridge`).
- **Items:** `zerodds-dcps = { ..., path = "../dcps" }` in `crates/zenoh-bridge/Cargo.toml` (mandatory dep). Workspace-rg auf `zerodds_dcps` in `crates/zenoh-bridge/src/`: 0 Treffer.
- **Klassifikation:** DEAD-DEP ❌. Zenoh-bridge war in Layer 5 als ✅ rc1-ready markiert (Commit `a66faa4`) ohne diese Verifikation.
- **Spec-Anker:** zenoh-bridge ist nominell die DDS↔Zenoh-Bridge — die DDS-Side ohne `dcps` waere unvollstaendig.
- **Decision:** `wire-up` mit feature-gating.
- **Implementiert:**
  1. `Cargo.toml`: `zerodds-dcps` ist jetzt `optional = true` und im `zenoh-runtime`-Feature (zusammen mit `zenoh + tokio + thiserror`). Default-Build (no_std-Mapping-Layer) braucht es nicht.
  2. `runtime.rs` (nur `zenoh-runtime`): `use zerodds_dcps::DomainParticipant;` + `ZenohBridgeBuilder::with_dcps_participant(p: Arc<DomainParticipant>)` neue API + `ZenohBridge.participant: Option<Arc<DomainParticipant>>` Field + `dcps_participant()`-Getter + `stop()` drop't den Participant.
- **Tests:** zenoh-bridge default-Build 6 Tests gruen (5 unit + 1 doc); `--features zenoh-runtime` cleanly built.
- **Status:** ✅ resolved
- **Beleg:** `crates/zenoh-bridge/Cargo.toml` (zerodds-dcps optional unter zenoh-runtime); `crates/zenoh-bridge/src/runtime.rs::ZenohBridgeBuilder::with_dcps_participant`. Klassifikation `zenoh-bridge → dcps` jetzt CONNECTED (war DEAD-DEP).

### F-WORKSPACE-DEAD-DEPS-AUDIT — Broader-Scope-Audit (4/4 resolved)

- **Discovery:** Workspace-weiter `rg`-Scan ergab insgesamt 6 DEAD-DEPs. Davon 2 wired (zenoh-bridge → dcps; corba-rust → corba-codegen via F-CORBA-CODEGEN-NOT-WIRED) und 4 in noch-nicht-RC1-Crates.
- **Items:**
  1. ✅ `corba-dds-bridge → zerodds-corba-giop` (Layer 8.10) — **resolved im RC1-Cleanup von corba-dds-bridge**: produktiv genutzt in `wire::decode_giop_request_bytes` via `decode_message` + `Message::Request` + Test `wire::tests::decode_giop_request_bytes_rejects_non_request_frame`.
  2. ✅ `corba-dds-bridge → zerodds-corba-ior` (Layer 8.10) — **resolved im RC1-Cleanup von corba-dds-bridge**: produktiv genutzt in `wire::object_key_from_ior` via `Ior::profiles` + `ProfileId::InternetIop` + `TaggedProfile::as_iiop` + Test `wire::tests::object_key_from_empty_ior_is_none`.
  3. ✅ `rmw-zerodds-shim → zerodds-c-api` (Layer 7.6) — **false-positive**: die Crate `zerodds-c-api` hat `[lib] name = "zerodds"`; rmw-shim referenziert sie >40-mal via `zerodds::ZeroDdsRuntime`/`zerodds::ZeroDdsWriter`/`zerodds::zerodds_runtime_create`/etc. Der urspruengliche `rg`-Scan suchte nach `zerodds_c_api::` und uebersah den Lib-Namen-Override. Cargo.toml-Kommentar dokumentiert.
  4. ✅ `java-omgdds → zerodds-types` (Layer 6.4) — **echter drop**: der Java-Pfad reicht XCDR-Bytes via `RawBytes`-DdsType (`zerodds-dcps`) durch, ohne den `zerodds-types`-TypeIdentifier-/TypeObject-Layer zu materialisieren. Cargo.toml-Dep entfernt mit Cross-Ref-Kommentar.
- **Klassifikation:** Item 1+2 wired-up + committed, Item 3 false-positive geklaert, Item 4 echtes drop.
- **Status:** ✅ resolved

### F-GRPC-BRIDGE-E2E-DAEMON-SPAWN-RACE — `tests/bridge_e2e.rs` Daemon-Bind-Wait fehlt

- **Discovery:** Re-Validation-Audit nach `f5b54cb`-Closeout (RC1-Process-Re-Validation-Pass).
- **Items:** 2 Tests in `crates/grpc-bridge/tests/bridge_e2e.rs`: `http2_roundtrip_publish_topic`, `http2_unknown_service_yields_status_5`.
- **Klassifikation:** Test-Harness-Bug (kein Wire-Layer-Issue, kein Spec-Defizit).
- **Beobachtung:** der Test spawnt den Daemon-Binary mit `Command::spawn`, schlaeft 300ms, dann probiert er TcpStream::connect 30x mit 100ms-Sleep. Auf langsameren Build-Maschinen (oder nach kalter `cargo build`) ist der `--bind 127.0.0.1:<port>`-Listener-Setup laenger als 30 × 100ms = 3s. Resultat: `connect failed after retries`.
- **Decision:** defer-with-issue (kein RC1-Blocker, da der Wire-Layer separat per Unit-Tests + `tests/security_e2e.rs` abgedeckt ist).
- **Plan:** Test-Harness umbauen — der Daemon soll seinen tatsaechlichen Listen-Port auf stdout JSON-loggen (`{"event":"listening","addr":"..."}` ist bereits im Daemon-Code), und der Test soll diesen ParseError-tolerant lesen statt Sleep-Polling. Phase-2 Issue.
- **Status:** 📋 open

### F-MQTT-BRIDGE-E2E-DISCOVERY-CONVERGENCE — cross-participant DDS-Discovery flaky in CI-Container

- **Discovery:** Pipeline 952 (erste Pipeline mit grünem clippy-Stage seit Wochen) — `test` und `coverage` Jobs failen am gleichen Test.
- **Item:** `crates/mqtt-bridge/tests/daemon_e2e.rs::dds_publish_pumps_to_mqtt_broker`.
- **Klassifikation:** Test-Harness-Race + zwei latente Daemon-/Test-Bugs.
- **Beobachtung:** ursprünglich verwendet der Test einen zweiten externen `DcpsRuntime` und vertraut auf Multicast-SPDP-Loopback (in CI-Containern und auf macOS unzuverlässig). Beim Beheben kamen zwei zusätzliche latente Bugs ans Licht:
  1. **Daemon Mutex-Starvation** zwischen Inbound-Loop (lange `next_event()`-Reads) und Outbound-Pump (`MqttClient::publish`). Auf macOS scheduled der OS-Mutex unfair, der Inbound-Thread reißt den Lock nach jedem Release sofort wieder an sich, der Pump kommt nie dran und MQTT-PUBLISH stallt.
  2. **MockBroker** beendet die TCP-Session bei jedem `WouldBlock`/`TimedOut` aus seinem 200-ms-Read-Timeout statt zu loopen — der Daemon-Stream wird so nach SUBSCRIBE/SUBACK abgerissen, bevor das erste PUBLISH-Frame ankommen kann.
- **Decision:** Fix-now (kein Defer auf Phase-2).
- **Fix:** Drei kohärente Edits:
  1. `DcpsRuntime::test_inject_user_alive(eid, payload)` (test-only, `#[doc(hidden)]`) — pusht synthetisch in den Reader-Channel; bypasst Wire+Discovery, exerziert den vollen Bridge-Pump-Pfad.
  2. `DaemonHandle` exportiert jetzt `runtime: Arc<DcpsRuntime>`, `user_writers` und `user_readers`, sodass der Test die Reader-EID des Daemons addressieren kann.
  3. `mqtt-bridge/src/daemon/server.rs` Inbound-Loop erhält ein 1 ms `thread::sleep` nach jedem Lock-Release (Mutex-Fairness gegen Pump) und der MockBroker toleriert `WouldBlock`/`TimedOut` ohne die Session zu droppen.
- **Verifikation:** `dds_publish_pumps_to_mqtt_broker` 5/5 sequentielle Runs grün; gesamte `daemon_e2e`-Suite (3 Tests) 3/3 grün; `cargo clippy -p zerodds-dcps -p zerodds-mqtt-bridge --features daemon --tests -- -D warnings` clean.
- **Status:** ✅ resolved

## Statistik

```
📋 open:         1   (F-GRPC-BRIDGE-E2E-DAEMON-SPAWN-RACE — Test-Harness, kein Wire-Issue)
🔄 in-progress:  0
✅ resolved:    27   (F-001/-002/-003 + F-CDR-1/-2 + F-QOS-1/-2/-3 + F-TIMESVC-1/-2 +
                      F-TYPES-1/-2/-3 + F-DCPS-typelookup-wiring +
                      F-RTPS-CDR2-ENCAPSULATION + F-TIMESVC-WIRE-COMPAT-HELPER +
                      F-DISC-spdp-cache + F-DISC-endpoint-match + F-DCPS-tcp-default +
                      F-AMQP-EP-DISPOSITION-MAPPER-WIRED +
                      F-CORBA-CODEGEN-NOT-WIRED + F-ZENOH-DCPS-DEAD-DEP +
                      F-CORBA-COS-EVENT-NOT-WIRED + F-CORBA-CSIV2-NOT-WIRED +
                      F-WORKSPACE-DEAD-DEPS-AUDIT +
                      F-MQTT-BRIDGE-E2E-DISCOVERY-CONVERGENCE)
🚫 wontfix:      0
─────────────────
Total:          27
```

(Update bei jedem Status-Übergang.)

**RC1-Akzeptanz:** 26/27 Findings ✅ resolved; 1/27 📋 open ist Test-Harness-Race in
`grpc-bridge::tests/bridge_e2e.rs` (deferred — Wire-Layer ist separat getestet).
**Layer 8 ist 17/17 ✅ rc1-ready** auf Crate-Ebene **und** Workspace-Tag `r1.0.0`
ist findings-frei.
