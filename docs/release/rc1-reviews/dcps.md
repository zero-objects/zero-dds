# RC1 Review — `zerodds-dcps`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Layer:** 4 (Core Services)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public (Dev-Repo `publish = false` wegen Embargo-Pfad-Dep `zerodds-inspect-endpoint`; Public-Mirror `publish = true`).
>
> Track-Materialisierung via git: `git log docs/release/rc1-reviews/dcps.md`.

---

## 1 Purpose

DCPS Public API (OMG DDS 1.4 §2.2.2) mit Live-Runtime: `DomainParticipantFactory`, `DomainParticipant`, `Publisher`/`DataWriter<T>`, `Subscriber`/`DataReader<T>`, `Topic<T>`, Built-in-Topics, Conditions/WaitSet, alle 22 QoS-Policies. Spawnt SPDP-/SEDP-/WLP-/TypeLookup-Endpoints, fuehrt Cross-Vendor-Discovery und liefert User-Daten.

## 2 Public-Strategy

- **Marker:** 🌐 public
- **Begründung:** Standard-Konsumenten-Crate fuer DDS-Anwender. Vom Public-Mirror-`publish = true` ist abzusehen, dass die Crate via `crates.io` die Default-Hochlevel-API darstellt.
- **Embargo-Pfad-Dep:** `zerodds-inspect-endpoint` ist Embargo (PDE-Reality-Inspector). Im Dev-Repo bleibt `publish = false`; das Public-Mirror unter `github/crates/dcps/` strippt das `inspect`-Feature.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs                    # Crate-Entry, pub-use Aggregator + Doctest
├── builtin_subscriber.rs     # BuiltinSubscriber + 4 Builtin-Topic-Reader
├── builtin_topics.rs         # ParticipantBuiltinTopicData u.a. (DCPS-API-Sicht)
├── coherent_set.rs           # CoherentScope/Marker + GroupAccessScope
├── condition.rs              # Condition/ReadCondition/QueryCondition/WaitSet
├── dds_type.rs               # DdsType-Trait + RawBytes + DdsTypeRow
├── durability_service.rs     # InMemory- + OnDisk-DurabilityBackend
├── entity.rs                 # Entity-Trait + StatusCondition + StatusMask
├── error.rs                  # DdsError + Result
├── factory.rs                # DomainParticipantFactory (Singleton)
├── flatdata_integration.rs   # opt-in: FlatWriterExt/FlatReaderExt
├── instance_handle.rs        # InstanceHandle + Allocator
├── instance_tracker.rs       # KeyHash → Instance-Lifecycle-State
├── interop.rs                # Cross-Vendor-Test-Typen
├── listener.rs               # Listener-Traits (6 Stueck)
├── listener_dispatch.rs      # Bubble-Up-Resolution
├── participant.rs            # DomainParticipant + IgnoreFilter
├── psm_constants.rs          # status_bits + qos_policy_id
├── publisher.rs              # Publisher + DataWriter<T>
├── qos.rs                    # 6 QoS-Familien (Participant/Publisher/...)
├── runtime.rs                # DcpsRuntime: Event-Loop, UDP, SPDP/SEDP/WLP/TypeLookup
├── sample.rs                 # Sample<T>
├── sample_info.rs            # SampleInfo + State-Kinds + Masks
├── status.rs                 # 13 Communication-Statuses
├── subscriber.rs             # Subscriber + DataReader<T>
├── time.rs                   # Time/Duration + get_current_time
├── topic.rs                  # Topic/ContentFilteredTopic/MultiTopic
└── wlp.rs                    # WLP-Endpoint (Writer-Liveliness-Protocol)
```

### 3.2 Public-API-Surface

Die Top-Level-Re-Exports aus `lib.rs`:

```rust
// Factory + Participant + Topic
pub use factory::DomainParticipantFactory;
pub use participant::{DomainId, DomainParticipant, IgnoreFilter};
pub use topic::{
    ContentFilteredTopic, JoinedRow, MultiTopic, Topic,
    TopicDescription, TopicDescriptionHandle, hash_join_two,
};

// Pub/Sub Hierarchy
pub use publisher::{DataWriter, Publisher};
pub use subscriber::{DataReader, Subscriber};

// QoS-Familien
pub use qos::{
    DataReaderQos, DataWriterQos, DomainParticipantQos,
    PublisherQos, SubscriberQos, TopicQos,
};

// Builtin-Topics
pub use builtin_subscriber::{BuiltinSinks, BuiltinSubscriber, BuiltinTopic, builtin_reader_qos};
pub use builtin_topics::{
    ParticipantBuiltinTopicData as DcpsParticipantBuiltinTopicData,
    PublicationBuiltinTopicData as DcpsPublicationBuiltinTopicData,
    SubscriptionBuiltinTopicData as DcpsSubscriptionBuiltinTopicData,
    TopicBuiltinTopicData as DcpsTopicBuiltinTopicData,
    TOPIC_NAME_DCPS_PARTICIPANT, TOPIC_NAME_DCPS_PUBLICATION,
    TOPIC_NAME_DCPS_SUBSCRIPTION, TOPIC_NAME_DCPS_TOPIC,
};

// Conditions/WaitSet
pub use condition::{Condition, GuardCondition, QueryCondition, ReadCondition, WaitSet};

// Sample/Status/Lifecycle
pub use sample::Sample;
pub use sample_info::{
    InstanceStateKind, SampleInfo, SampleStateKind, ViewStateKind,
    instance_state_mask, sample_state_mask, view_state_mask,
};
pub use entity::{Entity, EntityState, StatusCondition, StatusMask, immutable_if_enabled};
pub use coherent_set::{CoherentScope, CoherentSetMarker, GroupAccessScope};
pub use instance_handle::{HANDLE_NIL, InstanceHandle, InstanceHandleAllocator};
pub use instance_tracker::{InstanceState, InstanceTracker, KeyHash};

// Type/Time/Error
pub use dds_type::{DdsType, DdsTypeRow, DecodeError, EncodeError, RawBytes};
pub use time::{Duration, Time, get_current_time};
pub use error::{DdsError, Result};
```

### 3.3 Tests

- `cargo test -p zerodds-dcps`: ✅ **583 passed**, 0 failed, 8 ignored (Lab-Live-Tests benötigen `live-interop`-Feature).
- `cargo test -p zerodds-dcps --features flatdata-integration --test flatdata_integration`: ✅ **11 passed** (5 Builder-API + 6 Spec-konforme Direct-Method-Tests).
- Cross-Vendor-Live-Tests: `tests/cyclone_live_*.rs` (Cyclone-DDS), `tests/common/cross_vendor.rs` — `#[ignore]` bis `live-interop`-Feature aktiv ist.
- E2E-Tests inklusiv: `xtypes_aware_match.rs`, `exclusive_ownership_take.rs`, `type_lookup_e2e.rs`, `writer_data_lifecycle_qos.rs`, `cyclone_live_*` (TopicAnnounce / Discovery / Reliable / WLP).

### 3.4 Coherence-Audit (Public-API × Cross-Crate × Spec)

| Public-Item | Spec-Anker | External Production-Refs | Test-Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| `DomainParticipantFactory` + `*Qos`-Familie | DDS 1.4 §2.2.2.2.1 + §2.2.3 | Konsumiert von allen DCPS-Anwendungen + den Bridges (mqtt/coap/grpc/...) + PSMs (cpp/cs/java/py/rs/ts-*) | dcps-tests + Cross-Vendor-Tests | CONNECTED | — |
| `DomainParticipant` + `IgnoreFilter` | DDS 1.4 §2.2.2.2.2 + §2.2.2.2.1.14-17 | dcps-async + Bridges + PSMs | E2E-Tests | CONNECTED | — |
| `Publisher` / `DataWriter<T>` | DDS 1.4 §2.2.2.4 | dcps-async + Bridges | E2E-Tests | CONNECTED | — |
| `Subscriber` / `DataReader<T>` | DDS 1.4 §2.2.2.5 | dcps-async + Bridges | E2E-Tests | CONNECTED | — |
| `Topic<T>` / `ContentFilteredTopic` / `MultiTopic` | DDS 1.4 §2.2.2.3 + Annex B (SQL-Filter) | dcps-async, opcua-gateway | E2E-Tests | CONNECTED | — |
| `BuiltinSubscriber` + 4 Builtin-Topic-Datentypen | DDS 1.4 §2.2.5 | Konsumenten via `DataReader<DcpsPublicationBuiltinTopicData>` | builtin_topics-Tests | CONNECTED | — |
| `Condition` / `ReadCondition` / `QueryCondition` / `GuardCondition` / `WaitSet` | DDS 1.4 §2.2.2.7 | dcps-async (Wakers) | E2E-Tests | CONNECTED | — |
| `DdsType`-Trait + `DdsTypeRow` + `RawBytes` | XTypes 1.3 §7.6 + DDS 1.4 §2.2.2.4 (User-Type) | idl-rust-Codegen, Bridges, PSMs, alle End-User-Stubs | encode/decode-Tests | CONNECTED | — |
| `InstanceTracker` / `InstanceHandle` / `InstanceState` / `KeyHash` | DDS 1.4 §2.2.2.4.2 + §2.2.2.5.1 | dcps-async, Bridges (Mqtt-Last-Will-Translator), Recorder | E2E-Tests | CONNECTED | — |
| `CoherentScope` / `CoherentSetMarker` / `GroupAccessScope` | DDS 1.4 §2.2.2.4.1.6 + §2.2.2.5.1.6 + §2.2.3.7 | dcps-async (Coherent-Set-Aware Async-Drain) | coherent_set-Tests | CONNECTED | — |
| `DurabilityBackend`-Trait + `InMemoryDurabilityBackend` + `OnDiskDurabilityBackend` | DDS 1.4 §2.2.3.5 (Durability-Service) | dcps-Self-Use im Writer-Pfad; konsumiert via `DataWriter::durability_backend()`-Hook | durability_service-Tests + writer_data_lifecycle_qos | CONNECTED | — |
| Listener-Traits (`DataWriterListener`, `DataReaderListener`, `PublisherListener`, `SubscriberListener`, `TopicListener`, `DomainParticipantListener`) | DDS 1.4 §2.2.4.2 + §2.2.2.*.3 | dcps-async (async-Wrapper), End-User-Listener-Impls | listener_dispatch-Tests | CONNECTED | — |
| Status-Strukturen (13 Stueck) | DDS 1.4 §2.2.4.1 Tab. 2.10 | Listener-Callbacks + `get_*_status`-Methoden | E2E-Tests | CONNECTED | — |
| `DataWriter::write_flat` / `DataWriter::set_flat_backend` (spec-konforme Methoden) + `DataReader::read_flat` / `DataReader::set_flat_backend` | ZeroDDS-flatdata 1.0 §8.1 + §9.1 (ADR-0005) | Bench-suite + opt-in End-User-Builds | flatdata_integration-Tests (6 spec-konforme Tests) | CONNECTED | — (F-DCPS-flatdata-backend-noop wire-up) |
| `FlatWriterExt<T>` / `FlatReaderExt<T>` (alternative Builder-API) | ZeroDDS-flatdata 1.0 §8/§9 | flatdata_integration-Tests (5 Builder-Tests) | TEST-ONLY (auf Production-Side hat aber externe Library-API-Konsumenten als Hook) | document-as-hook (alternative API) |
| `WlpEndpoint` (in `wlp.rs`) | DDSI-RTPS 2.5 §8.4.13 + §9.6.3.1 | runtime.rs (Self-Use) | wlp-Tests | CONNECTED | — |
| `interop`-Module (Cross-Vendor-Test-Typen) | Vendor-Extension fuer Live-Tests | tests/cyclone_live_* | TEST-ONLY | document-as-hook (Cross-Vendor-Test-Helper) |

Ergebnis: **0 ❌-Klassen offen**. Alle Public-Items sind entweder CONNECTED, OPTIONAL-HOOK oder explizite Test-Helper.

## 4 Wiring

### 4.1 Dependencies (uses)

```toml
[dependencies]
zerodds-cdr            = { path = "../cdr" }
zerodds-foundation     = { path = "../foundation" }
zerodds-qos            = { path = "../qos" }
zerodds-rtps           = { path = "../rtps" }
zerodds-types          = { path = "../types" }
zerodds-transport      = { path = "../transport" }
zerodds-transport-udp  = { path = "../transport-udp" }
zerodds-discovery      = { path = "../discovery" }
zerodds-sql-filter     = { path = "../sql-filter" }
# Optional
zerodds-security-runtime = { path = "../security-runtime", optional = true }
zerodds-flatdata         = { path = "../flatdata",         optional = true, features = ["std"] }
zerodds-inspect-endpoint = { path = "../inspect-endpoint", optional = true } # Embargo
```

### 4.2 Dependents (used-by)

```bash
$ rg -l 'zerodds-dcps' --type-add 'cargo:Cargo.toml' -t cargo crates/ tools/ examples/
```

DCPS ist die zentrale High-Level-API. Direkte Konsumenten in dieser Workspace-Phase: `dcps-async`, `monitor`, `recorder`, `rpc`, alle Bridges (`amqp-bridge`, `coap-bridge`, `grpc-bridge`, `mqtt-bridge`, `websocket-bridge`, `zenoh-bridge`), alle PSM-Bindings (`cpp`, `cs`, `java`, `java-omgdds`, `py`, `rs`, `ts-node`, `ts-wasm`, `zerodds-c-api`, `java-omgdds`), alle Profile-Crates (`conformance`, `dlrl`, `xrce`/`-agent`/`-client`, `web`, `ros2-rmw`, `rmw-zerodds-shim`, `opcua-gateway`, `zerodds-soap`).

### 4.3 Feature-Flags

Siehe `README.md` und `Cargo.toml`. Default = `["std"]`.

## 5 Spec-Relevanz

- **Spec(s):** OMG DDS 1.4 §2.2 + DDSI-RTPS 2.5 §8.5 + XTypes 1.3 §7.6.3 + DDS-Security 1.2 (opt-in) + ZeroDDS-flatdata 1.0 (opt-in).
- **Coverage-Doc(s):** `docs/spec-coverage/dds-dcps-1.4.md`, `docs/spec-coverage/ddsi-rtps-2.5.md`.
- **Abgedeckte Sektionen:** komplette §2.2 (DCPS-Module). RTPS §8.5 (Discovery) + §8.4.13 (WLP) + §9.6.4.8 (Inline-QoS) sind via diese Crate produktiv. XTypes §7.6.3.3 (TypeLookup-Service) und §7.6.3.7 (TypeIdentifier-aware Match) sind cross-layer wired.

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

```bash
rg -i -e 'llvm@llvm' -e 'sandra-kessler' -e 'fishermen21' \
  -e '/Users/sandrakessler' -e 'PDE-Spec' -e 'zero_concept' \
  -e 'zero-principle' -e 'Ghost-Inject' -e 'R-09[7-9]' \
  -e 'R-10[0-4]' -e 'R-110' -e '\bseesaw\b' -e 'IfynaNeu' \
  -e 'paperless' -e '\bglr1\b' -e '\bglr2\b' -e '/tmp/cyc\.xml' \
  crates/dcps/src/ crates/dcps/Cargo.toml crates/dcps/README.md crates/dcps/CHANGELOG.md
```

Treffer: **0** (nach Cleanup). `R-110` (in einem Inspect-Tap-Comment) war einziger Treffer und wurde durch eine fachliche Umformulierung ersetzt. Lab-Refs in `tests/cyclone_live_*.rs` und `tests/common/cross_vendor.rs` sind per Guardrails §2.1 explizit erlaubt (Public-Mirror-Exclude).

### 6.2 Sprint-/Project-Management-Sprache

Pre-Cleanup-Stand: 212 Sprint-/WP-/Cluster-/Phase-Marker. Post-Cleanup: **0**. Bulk-Pass per `sed` mit nachgelagerter manueller Review. Header-Doku in `lib.rs`/`runtime.rs`/`participant.rs`/`factory.rs` wurde auf neutrale, fachliche Form (Live-Mode vs. Offline-Mode, Spec-Section-Referenzen) umgeschrieben.

### 6.3 Datums-Marker

Keine im Source. CHANGELOG.md hat den Keep-a-Changelog-Konvention-Marker `## [1.0.0-rc.1] — 2026-05-06` (per Guardrails §2.1c erlaubt).

### 6.4 Soft-Review (TODO/FIXME/HACK/XXX)

Keine.

### 6.5 Lab-Refs in src/

Keine. Alle Lab-Refs leben in `tests/cyclone_live_*.rs` und `tests/common/cross_vendor.rs` (Public-Mirror-Exclude).

### 6.6 Public-API-Leaks

Keine. `lib.rs` listet alle `pub use` explizit, kein `pub use crate::module::*` auf interne Module.

### 6.7 Dead-Code

`DataWriter::set_flat_backend(_, _)` war ein leerer Stub (0 LOC effektiv) — entfernt zugunsten der bereits voll implementierten `FlatWriterExt`-Builder-API (siehe F-DCPS-flatdata-backend-noop).

## 7 Cleanup-Actions

Im Detail:

1. **F-DCPS-builtin-userdata** (resolved, see `RC1_FINDINGS.md`): `ParticipantBuiltinTopicData::from_wire` propagiert `user_data` aus dem RTPS-Wire-Typ statt es zu verwerfen. RuntimeConfig erhält ein `user_data: Vec<u8>`-Feld; `DomainParticipantFactory::create_participant` reicht `qos.user_data.value` durch. SPDP-Beacon trägt das Feld korrekt (Spec §2.2.5.1 PID_USER_DATA).
2. **F-DCPS-typelookup-registry** (resolved): `participant::has_type_for_hash` konsultiert jetzt `runtime.type_lookup_server.registry` (Minimal + Complete), statt immer `false` zu liefern. Zusätzlich: `runtime::send_type_lookup_request` registriert einen Reply-Callback, der eingehende `GetTypesReply.types` in den lokalen Registry-Slot einspeist (per `compute_hash` umgeschlüsselt).
3. **F-DCPS-durability-sequence** (resolved): `DataWriter` hat jetzt `durability_seq: AtomicU64` mit `fetch_add(1)` per `write()`. Ersetzt das `sequence: 0` im `DurabilityBackend::store`-Pfad, sodass Late-Joiner-Replay in Insert-Reihenfolge geliefert wird.
4. **F-DCPS-locator-binding** (resolved): SPDP-Beacon-Locators werden via `announce_locator()` materialisiert. Bei `0.0.0.0`-Bind-Adresse löst ein UDP-Connect-Probe (zu RFC-5737 TEST-NET-1 192.0.2.1:7) die outbound-Interface-IP auf und annonciert sie statt `0.0.0.0`. Fallback-Kette: Probe → `multicast_interface`-Hint → Loopback. Cross-Host-Interop-fähig ohne externe Crate-Abhängigkeit.
5. **F-DCPS-sedp-topic-decode** (resolved): SEDP-Topics-Endpoint-Bits 28/29 sind per RTPS 2.5 §8.5.4.4 optional. `endpoint_flag::ALL_STANDARD` wurde auf bit 28/29 gestrippt; DCPSTopic-Samples werden weiterhin synthetisch aus Pub/Sub abgeleitet (`push_sedp_events_to_builtin_readers`). Cross-Layer-Auswirkung: `discovery::PeerCapabilities::has_topics_discovery` reagiert weiterhin korrekt, wenn ein Vendor die Bits explicitly ergänzt; Test-Coverage erweitert.
6. **F-DCPS-keyhash-stub** (resolved): Cross-Layer-Wire-up: `rtps::CacheChange` und `rtps::DeliveredSample` tragen jetzt `key_hash: Option<[u8; 16]>`. `inline_qos::find_key_hash` liest `PID_KEY_HASH` aus dem Inline-QoS (Spec §9.6.4.8). DCPS-`delivered_to_user_sample` nutzt den propagierten Hash statt einer naiven First-16-Bytes-Slice-Heuristik; Fallback auf die alte Heuristik bleibt für nicht-spec-konforme Writer erhalten.
7. **F-DCPS-flatdata-backend-noop** (resolved): User-Pushback gegen den ersten "drop"-Vorschlag — Stub war Wire-up-Artefakt der ursprünglichen Zero-Copy-Einführung, nicht eine bewusste Dead-API. Spec-konformes API jetzt voll implementiert: `DataWriter` und `DataReader` tragen je ein `flat_backend`-Feld; `set_flat_backend(Some|None, ...)`/`write_flat(sample)` und `read_flat() -> Option<T>` sind direkte Methoden gemäß Spec §8.1/§9.1, mit UDP-Fallback ohne Backend, Type-Hash-Cross-Validation gegen `T::TYPE_HASH` (Spec §6.1) und `last_sn`-basierter Re-Read-Suppression. Die `FlatWriterExt`/`FlatReaderExt`-Builder-API bleibt als alternative API für Caller, die mehrere Slot-Backends parallel mit derselben Entity benutzen.
8. **F-SEC-AAD-NOT-WIRED-Test-Followup** (Side-Effekt aus Workspace-Test-Run): `crates/security-pki/tests/pki_crypto_integration.rs` und `crates/security-permissions/tests/psk_handshake.rs` riefen `encrypt_submessage`/`decrypt_submessage`/`encrypt_submessage_multi`/`decrypt_submessage_with_receiver_mac` ohne den neuen `aad_extension: &[u8]`-Parameter auf — pre-existing Compile-Fehler nach commit `33cd0c8`. Behoben durch Anhängen von `&[]` an die 4 Call-Sites; Tests grün.
9. **`endpoint_flag::ALL_STANDARD`-Test-Update** (Layer-2-Side-Effekt): `discovery::capabilities::tests::capabilities_full_standard_bundle` hat die Topics-Bits-Erwartung auf "nicht in ALL_STANDARD" angepasst; ein neuer Test `capabilities_topics_discovery_when_explicitly_added` deckt den Vendor-Add-Path ab.
10. **Doku-Cleanup** lib.rs/runtime.rs/participant.rs/factory.rs: stale "Phase A/B/C"-Sprache durch Live-Mode/Offline-Mode + Spec-Sektion-Referenzen ersetzt. Crate-Header in `lib.rs` neu in RC1-Form (Public-API-Aufzählung + Schichten-Position + Spec-Anker + Doctest).
11. **SPDX-Header pro `*.rs`**: 28 src-Files erhielten `SPDX-License-Identifier: Apache-2.0` + Copyright-Header.
12. **Cargo.toml-Metadata**: `homepage`, `documentation`, `readme`, `keywords`, `categories` ergänzt. `publish = false` mit Begründung dokumentiert (Embargo-Pfad-Dep). Dev-Dependency-Kommentare entjargonifiziert.
13. **Doc-Build-Warnings**: 8 rustdoc-Warnings (`unresolved link`, `unclosed HTML tag`, `could not parse code block`) eliminiert. `cargo doc -p zerodds-dcps --no-deps` ist warning-frei.
14. **README.md** auf RC1-Form (Status-Badges, Spec-Anker, Quickstart, Feature-Flags-Tabelle, Stabilitäts-Statement).
15. **CHANGELOG.md** als initial-Materialisierung erstellt (Spec-Referenzen, vollständige Public-API-Aufzählung, Implementierungs-Notizen, Architektur-Mapping, Stabilitäts-Statement).

## 8 Spec-Doc-Updates

- `docs/spec-coverage/dds-dcps-1.4.md` und `docs/spec-coverage/ddsi-rtps-2.5.md`: keine Statusübergänge — die Sektionen waren im K3a-/K3b-Audit bereits auf `done`. Repo-Belege bleiben gültig.

## 9 Doc-Artefacts

- [x] `Cargo.toml`-Metadata vollständig (siehe Guardrails §1.1, mit dokumentiertem `publish = false`-Grund)
- [x] `lib.rs`-Crate-Header mit Safety-Class + Spec-Ref + Layer + API-Aufzählung + Doctest
- [x] `README.md` auf RC1-Form
- [x] `CHANGELOG.md` mit `[1.0.0-rc.1]`-Eintrag (initial-Materialisierung)
- [x] doc-tested Code-Example in `lib.rs`

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-dcps                            # ✅ 583 passed, 0 failed, 8 ignored
cargo clippy -p zerodds-dcps --tests -- -D warnings   # ✅ clean
cargo fmt -p zerodds-dcps -- --check                  # ✅ clean
cargo doc -p zerodds-dcps --no-deps                   # ✅ keine Warnungen
cargo run --bin zerodds-lint -- check                 # ✅ workspace 105 crates / 1013 files / 0 warnings
cargo test --workspace --lib --tests                  # ✅ 8651 passed, 0 failed, 41 ignored
```

## 11 RC1-DoD-Checkliste

- [x] §1.1 Cargo.toml-Metadata
- [x] §1.2 lib.rs Crate-Header
- [x] §1.3 README.md aus Template
- [x] §1.4 CHANGELOG.md mit RC1-Entry (initial-Materialisierung-Format)
- [x] §1.5 Public-API-Audit
- [x] §1.5b Coherence-Audit (Tabelle in §3.4 ausgefüllt, alle ❌ haben Decision)
- [x] §1.6 Spec-Coverage-Update (keine Statusübergänge nötig)
- [x] §1.7 Forbidden-Token-Sweep
- [x] §1.8 License-Header pro File (28 src-Files)
- [x] §1.9 Tests + Lints + Doc-Build grün
- [x] §1.10 Review-Doc ausgefüllt (= dieses Dokument)
- [x] §1.11 Tracker auf ✅
- [x] §1.12 Public-Mirror-Artifacts: `github/crates/dcps/{Cargo.toml,src,README.md,CHANGELOG.md}` + `github/Cargo.toml`-Member-List + `github/CHANGELOG.md`-Eintrag + `website/docs/dcps.md`. **Side-Effekt:** Pre-existing Lücke der Layer-2/3-Mirror-Integration im Workspace-Manifest aufgeholt (Layer-2/3-Member-Liste + Layer-2/3-CHANGELOG-Sektionen ergänzt — die rc1-ready Crates der unteren Layer waren physisch im github/-Tree, aber nicht im Workspace-Manifest referenziert).
- [x] §1.13 Spec-Conformance-Audit (Inline-Deferral-Marker = 0 nach Cleanup; alle 7 F-DCPS-N-Findings ✅ resolved)
- [x] Findings-Tracker `RC1_FINDINGS.md` aktualisiert (F-DCPS-{builtin-userdata, typelookup-registry, durability-sequence, locator-binding, sedp-topic-decode, keyhash-stub, flatdata-backend-noop} ✅ resolved)

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1` (workspace-version wird beim r1.0.0-Tag global hochgezogen)
- **Reviewer-Sign-off:** Claude
- **Tracker-Eintrag aktualisiert:** ✅

(Sign-off-Zeitpunkt = git-commit-Zeitpunkt dieser Datei.)
