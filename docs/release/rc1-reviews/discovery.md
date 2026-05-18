# RC1 Review — `zerodds-discovery`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 2.1 (Wire — Discovery)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

DDSI-RTPS-Discovery für ZeroDDS — SPDP (§8.5.3) + SEDP (§8.5.4) +
TypeLookup-Service (XTypes 1.3 §7.6.3.3.4) + DDS-Security 1.2 §7.4.2
Builtin-Endpoint-Slots.

## 2 Public-Strategy

🌐 public — Discovery-Primitives für End-User-Custom-DCPS-Builds.

## 3 Content-Inventur

### 3.1 Module

```
src/lib.rs
src/spdp.rs            # SPDP Beacon + Reader + Cache
src/sedp/              # SEDP Stack (cache, reader, writer)
src/security/          # DDS-Security Stateless + Volatile-Secure
src/type_lookup/       # TypeLookup-Service (Server, Client, Endpoints)
src/endpoint_match.rs  # Topic+Type+QoS-Match-Logic
src/capabilities.rs    # PeerCapabilities (BuiltinEndpointSet)
```

51 Public-Items insgesamt.

### 3.2 Tests

- `cargo test -p zerodds-discovery`: ✅ 144+ passed.
- `live-interop` Feature für Cross-Vendor-Cyclone-Tests.

### 3.3 Coherence-Audit (§1.5b) — gruppiert nach Public-API-Family

| Family | Items | Spec-Anker | External Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| **SPDP Stack** | `SpdpBeacon`, `SpdpReader`, `DiscoveredParticipant` | DDSI-RTPS 2.5 §8.5.3 | `SpdpBeacon`/`SpdpReader`/`DiscoveredParticipant` CONNECTED in dcps::runtime | CONNECTED | — |
| **SPDP Cache** | `DiscoveredParticipantsCache` | DDSI-RTPS §8.5.3 (Lease-Tracking) | 0 ext (DCPS nutzt raw BTreeMap statt Cache) | VENDOR-EXTENSION (Library-API für End-User-Custom-Discovery-Loops) | doc-as-hook + F-DISC-spdp-cache-consolidation für Layer-3-DCPS-Review |
| **SEDP Aggregator** | `SedpStack`, `SedpEvents` | DDSI-RTPS §8.5.4 | CONNECTED in dcps::runtime | CONNECTED | — |
| **SEDP Components** | `DiscoveredEndpointsCache`, `CacheCaps`, `DiscoveredPublication`, `DiscoveredSubscription`, `SedpPublicationsReader`, `SedpSubscriptionsReader`, `SedpPublicationsWriter`, `SedpSubscriptionsWriter`, `SedpReaderError` | DDSI-RTPS §8.5.4 | 0 ext direkt; via SedpStack-Aggregator CONNECTED | VENDOR-EXTENSION (Sub-Components der Public-API für End-User-Custom-SEDP-Wires) | doc-as-hook |
| **SEDP Defaults** | `SEDP_READER_MAX_SAMPLES`, `SEDP_DEFAULT_DEPTH`, `SEDP_HEARTBEAT_PERIOD`, `DEFAULT_MAX_PUBLICATIONS_PER_PARTICIPANT`, `DEFAULT_MAX_SUBSCRIPTIONS_PER_PARTICIPANT` | Library-Defaults | 0 ext | VENDOR-EXTENSION (Public Default-Konstanten) | doc-as-hook |
| **DDS-Security Stack** | `SecurityBuiltinStack` | DDS-Security 1.2 §7.4.2 | CONNECTED in dcps::runtime | CONNECTED | — |
| **Security Sub-Components** | `StatelessMessageReader`, `StatelessMessageWriter`, `VolatileSecureMessageReader`, `VolatileSecureMessageWriter`, `VOLATILE_SECURE_DEFAULT_DEPTH`, `VOLATILE_SECURE_HEARTBEAT_PERIOD`, `VOLATILE_SECURE_READER_CAPACITY`, codec helpers (`encode_generic_message`, `decode_generic_message`, `ENCAPSULATION_CDR_LE`, `ENCAPSULATION_HEADER_LEN`) | DDS-Security 1.2 §7.4.4+§7.4.5 | 0 ext direkt; via SecurityBuiltinStack CONNECTED | VENDOR-EXTENSION (Sub-Components) | doc-as-hook |
| **TypeLookup Service** | `TypeLookupServer`, `TypeLookupClient`, `TypeLookupEndpoints`, `TypeLookupReply`, `RequestId`, `ClientCallback`, `request_types_payload`, `request_dependencies_payload`, `hashes_to_minimal_ids`, `format_service_instance_name`, `format_service_instance_name_short`, `TYPELOOKUP_TOPIC_PREFIX`, `TypeLookupStack` | XTypes 1.3 §7.6.3.3.4 | CONNECTED in dcps::runtime (F-DCPS-typelookup-wiring) | CONNECTED | — |
| **Endpoint-Match** | `endpoint_match::*` (`MatchInputs`, `EndpointMatchResult`) | DDS 1.4 §2.2.3 (Compatibility-Match) | 0 ext direkt; DCPS hat eigenen Match-Pfad in subscriber.rs | VENDOR-EXTENSION (Library-API für End-User-Match-Inspect) | doc-as-hook + F-DISC-endpoint-match-consolidation für Layer-3-DCPS-Review |
| **Capabilities** | `PeerCapabilities` | DDSI-RTPS §9.3.2.12 (BuiltinEndpointSet-Bits) | CONNECTED in dcps::runtime | CONNECTED | — |

**Zusammenfassung:** 51/51 Public-Items klassifiziert.
- 19 CONNECTED
- 32 VENDOR-EXTENSION (mit doc-as-hook Decision)
- 0 DEAD
- 2 Cross-Layer-Findings (F-DISC-spdp-cache-consolidation, F-DISC-endpoint-match-consolidation) → in RC1_FINDINGS.md getrackt

## 4 Wiring

### 4.1 Dependencies

```toml
zerodds-rtps = { path = "../rtps", default-features = false, features = ["alloc"] }
zerodds-types = { path = "../types", default-features = false, features = ["alloc"] }
zerodds-qos = { path = "../qos", default-features = false, features = ["alloc"] }
zerodds-cdr = { path = "../cdr", default-features = false, features = ["alloc"] }
zerodds-security = { path = "../security", default-features = false, features = ["alloc"] }
```

### 4.2 Dependents

`zerodds-dcps` (Hauptkonsument), Examples, Tests.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std + alloc |
| `alloc` | ✅ (via std) | Heap |
| `safety` | ❌ | reserved |
| `live-interop` | ❌ | Cross-Vendor-Cyclone-Tests (SSH zu Lab-Host) |

## 5 Spec-Relevanz

- DDSI-RTPS 2.5 §8.5 (SPDP/SEDP)
- XTypes 1.3 §7.6.3.3.4 (TypeLookup-Service)
- DDS-Security 1.2 §7.4.2 (Builtin-Endpoint-Slots)

## 6 Cleanup-Findings

Bereits abgeschlossen (Layer-2 Pass 1):
- 18 License-Header
- 14 Phase-X-Marker rewriting
- F-DISC-1 (TypeLookup-Wiring) → ✅ resolved (commit 47662fe)

Neue F-Findings für Layer-3 (siehe RC1_FINDINGS.md):
- **F-DISC-spdp-cache-consolidation**: DCPS bypasses `DiscoveredParticipantsCache`, nutzt raw BTreeMap.
- **F-DISC-endpoint-match-consolidation**: DCPS hat eigenen Match-Pfad statt `endpoint_match::*`.

## 7 Cleanup-Actions

Layer-2 Pass 1 + Pass 2 zusammengefasst:
1. License-Header in 18 src-Files.
2. Cargo.toml RC1-Metadata.
3. Phase-X-Marker bereinigt.
4. TypeLookup-Wiring in DCPS (F-DCPS-typelookup-wiring resolved).
5. PeerCapabilities erweitert um `has_type_lookup`-Field.
6. §3.4-Tabelle voll gefüllt (Pass 2).

## 8 Spec-Doc-Updates

`docs/spec-coverage/ddsi-rtps-2.5.md` §8.5 + XTypes 1.3 §7.6.3.3.4 +
DDS-Security 1.2 §7.4.2 — alle done.

## 9 Doc-Artefacts

- [x] Cargo.toml RC1
- [x] lib.rs-Header mit Spec-Verankerung + Layer-Boundary-Statement an DCPS
- [x] README + CHANGELOG

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-discovery                                    # ✅ 144+ passed
cargo clippy -p zerodds-discovery --all-targets -- -D warnings     # ✅
cargo doc -p zerodds-discovery --no-deps                           # ✅
```

## 11 RC1-DoD-Checkliste

- [x] §1.1-§1.13 alle ✅
- F-DISC-spdp-cache-consolidation + F-DISC-endpoint-match-consolidation als Cross-Layer-Findings für DCPS-Review getrackt

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
