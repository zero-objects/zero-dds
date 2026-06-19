# Zero-Principle mapping

> **Dependencies:** Zero-Principle Manifest (external, maintained in the Zero-Concept repo), `02_architecture.md`, `04_safety_by_architecture.md`.
>
> Track materialization via the git commits of this file.

This document lays out how ZeroDDS implements the Zero-Principle values — substrate mapping, pillar-by-pillar finding, rigor placement. It is the bridge between the Zero Manifest (values, no spec) and the DDS domain (spec, no manifest).

## 1 Placement

ZeroDDS is the **realtime pub/sub substrate implementation** under the Zero label. The domain is messaging-centric (DDSI-RTPS, DDS 1.4), not graph-centric — the Zero-Principle substrate terms (Fragment, Trail, Trait, Strain, Track) map onto DDS terms naturally, without DDS having to be reinterpreted as a graph database.

Foundation statement 1 (self-similarity) holds: every DDS layer (Domain → Topic → Sample) is itself substrate at its level. The rigor is gradable — the Zero-vague variant is an offline participant CLI, the Zero-strict variant is a DDS-Security-1.2-bonded domain with BuiltinDataTagging and an audit sink.

ZeroDDS places itself on the rigor scale explicitly at the **Zero-strict pole**: formally modeled (32 spec-coverage docs strict-audited), cryptographically anchored (DDS-Security 1.2 with IdentityToken/Permissions/AccessControl), fully audited (observability + OTLP + DDS-Security audit hooks).

## 2 Substrate mapping

| Zero concept | DDS counterpart | Crate / source |
|--------------|-----------------|----------------|
| **Fragment** | DDS sample (typed, key-addressed, lifecycle-aware) | `zerodds-dcps::Sample`, `zerodds-cdr` for content |
| **Trail** | Topic + ContentFilter (filter vocabulary) | `zerodds-dcps::Topic`, `zerodds-dcps::ContentFilteredTopic` |
| **Trait** | TypeObject + Partition + DataTag (classification position) | `zerodds-types`, `zerodds-qos::PartitionQosPolicy`, `zerodds-security::data_tagging` |
| **Strain** | DDS-Security Permissions + AccessControl (visibility/authorization) | `zerodds-security`, `zerodds-security-permissions` |
| **Track** | DurabilityCache (TransientLocal snapshot Tx), recorder `.zddsrec` | `zerodds-rtps::history_cache`, `zerodds-recorder` |
| **Substrate** | Domain-participant cluster | `zerodds-dcps::DomainParticipant` |
| **Cluster of Ground Truth** | Domain with discovery cache as a federated authority | `zerodds-discovery::sedp::SedpStack` |
| **Genesis** | DomainParticipantFactory.create_participant | `zerodds-dcps::DomainParticipantFactory` |

### 2.1 Strong edges in DDS

Foundation §3 requires strong-edge guarantees (referentially mandatory, content-addressed). DDS realizes them via:

- **Sample-key hash + SequenceNumber:** content-addressed identity per sample (RTPS 2.5 §9.6.3.4).
- **Type hash SHA-256:** content-addressed identity per schema (XTypes 1.3 §7.3.1, ZeroDDS `flatdata-1.0` §6.1).
- **GUID (GuidPrefix + EntityId):** content-addressed identity per entity (RTPS 2.5 §8.2.4.3).

These three axes deliver the Merkle-DAG property: every strong edge is verifiable, a break is detectable.

### 2.2 Four-step chain in DDS wire terms

Foundation §4 (Fragment → Trail → Trait → Strain) is naturalized in DDS:

```
Sample (Fragment)
   ↓ via topic match
Topic + Filter (Trail)
   ↓ via TypeInfo + Partition + DataTag
Classification (Trait)
   ↓ via Permissions + AccessControl
Visibility (Strain)
```

Authorizations live **only** in the Strain layer (DDS-Security permissions). That is Foundation-conformant: no authorization logic in `zerodds-rtps` or `zerodds-cdr`, but exclusively in `zerodds-security*`.

## 3 Pillar-by-pillar finding

### Pillar 1 — Zero-Lock-In ✅
- OMG DDS 1.4 as an open spec, byte-identical wire compat with Cyclone/FastDDS (`crates/discovery/tests/cyclone_*`).
- Five PSMs: cpp/csharp/java/python/typescript (waves 1–4).
- Seven bridges: AMQP, MQTT, CoAP, gRPC, WebSocket, Zenoh, ROS2-RMW.
- `.zddsrec` recording format (open, documented in `crates/recorder`).

### Pillar 2 — Zero-Hollow-Foundation ✅
- `zerodds-foundation` is actually foundation: no ZeroDDS logic hides next to it.
- The reference implementation IS the project; no commercial shell.
- License: Apache-2.0 for code (Rust-ecosystem default, compatible with the DDS peer stack Cyclone/FastDDS, explicit patent grant). Pillar-2 foundation protection is realized via the trademark of the Zero label and repo governance, not via copyleft. Spec-coverage docs are Apache-2.0-compatible; the Zero-Principle Manifest itself (externally maintained) stays CC-BY-SA 4.0.

### Pillar 3 — Zero-Notation-Lock-In ✅
- IDL as the schema language (industry-standard OMG IDL 4.2, not invented).
- XCDR1/2 + PL-CDR as wire encoding (all three spec).
- No ZeroDDS-own DSL forced; `zerodds-xml` reads QoS XML in a standard-conformant way.

### Pillar 4 — Zero-Imposed-Topology ✅
- DDS by-design broker-free P2P (DDSI-RTPS).
- Domain participants autonomous, SPDP multicast discovery.
- Bridges allow a mix with broker-oriented worlds (AMQP broker ↔ DDS bus) — no topology is imposed.
- Cluster of Ground Truth = the domain cluster is an allowed local authority, not a global platform.

### Pillar 5 — Zero-Implicit-Sharing ✅
- DDS-Security 1.2 full: IdentityToken, PermissionsToken, AccessControl, BuiltinDataTagging.
- Topic + Partition + ContentFilter are explicit visibility statements.
- Built-in topics (PARTICIPANT/PUBLICATION/SUBSCRIPTION) are introspectable and thus themselves a statement, not hidden.

### Pillar 6 — Zero-Context-Loss ✅
- XTypes 1.3 TypeInformation propagates via discovery (type object + type-identifier hashes).
- TYPE_HASH cross-validation in the `flatdata` read path — schema drift fails immediately as `PreconditionNotMet`, not as data corruption.
- BuiltinDataTagging propagates classification tags per sample.
- PID_RELATED_ENTITY_GUID preserves RPC endpoint pairings through migration.

### Pillar 7 — Zero-Out-of-Band ✅ (with rationale)
- Production state lies entirely in the DDS substrate (discovery cache, history cache, built-in topics).
- **Inspect endpoint / ghost interface**: per the Zero-Principle a *Track* with scope `inspect` and its own *Strain* (cert layer). Not out-of-band, because:
  1. **Compile default OFF** (`#[cfg(feature = "inspect")]`) — the substrate in a release build does not have the Track.
  2. **Config default OFF** — even with a feature build the Track does not activate without explicit config.
  3. **Cert layer mandatory** (`cert.d` loader, X.509 PEM, R-100..R-104) — the Strain controls visibility at the permission level.

  Three explicit opt-ins are the strongest form of "visibility is a statement, not a default" (Pillar 5). The inspect Track is declared *in* the substrate (documented in `crates/inspect-endpoint/src/lib.rs`), not *next to* the substrate.

  Ghost-inject (R-110) bypasses production taps deliberately — that is the defined Trail of this Track: ghost-inject is a transformation on a sub-substrate whose visibility is regulated by a Strain (cert auth). That is Zero-conformant precisely because it is defined and documented.

### Pillar 8 — Zero-Overhead ✅
- Feature flags everywhere: `security`, `iceoryx2`, `tokio-glue`, `inspect`, `live-interop`, `tcp-transport`, `shm-transport`.
- `zerodds-foundation` no_std-capable (PoolBuffer + BufferPool without a heap).
- The offline participant works without UDP, without security, without discovery (`create_participant_offline`).

### Pillar 9 — Zero-Dependency ✅
- iceoryx2 opt-in (stub adapter in the default build).
- zenoh opt-in (rustc-1.86 requirement gated).
- tokio opt-in (`tokio-glue` feature).
- No mandatory broker, no mandatory cloud, no mandatory PKI (DDS-Security is opt-in).

## 4 Tech strategies T1–T9

| | Strategy | ZeroDDS realization | Status |
|---|---|---|---|
| T1 | Transformations | IDL→PSM codegen (cpp/csharp/java/python/ts), bridge mappings | partial — present as codegen pipelines, not formalized as a transformation DSL |
| T2 | TGGs | not directly | n/a — TGG is heavy for the DDS domain, deliberately out of scope |
| T3 | Content addressing | ✅ type hash SHA-256, sample-key hash, GUID | done |
| T4 | Versioning & lineage | ✅ XTypes assignability + evolution rules | done |
| T5 | Federation protocol | ✅ DDSI-RTPS 2.5 (SPDP + SEDP + reliable/best-effort) | done |
| T6 | Identity & actors | ✅ GuidPrefix + DDS-Security IdentityToken + permissions | done |
| T7 | Lifecycle | ✅ complete DDS sample lifecycle (alive/disposed/unregistered + autodispose) | done |
| T8 | Schema evolution | ✅ XTypes 1.3 full | done |
| T9 | Audit & provenance | ✅ observability + OTLP + DDS-Security audit | done |

T1/T2 are the only gaps — deliberate, because the DDS domain does not need them and the Zero-Principle in §Concepts explicitly allows strategies to be interpreted project-specifically.

## 5 Rigor placement

ZeroDDS is conceived as a **Zero-strict implementation**:

- **formally modeled:** 32 spec-coverage docs (`docs/spec-coverage/`), a strict-audit pass on all of them.
- **cryptographically anchored:** DDS-Security 1.2 with RSA-PSS-2048, AES-GCM crypto, X.509 cert bind, CRL validation.
- **fully audited:** Foundation observability sinks (`zerodds-foundation::observability::Component`), OTLP adapter (`zerodds-observability-otlp`), pre-commit lints (`zerodds-lint`), CI with a bench-regression check and cross-vendor soak.

Zero-vague applications (e.g. an offline participant without security for lab tests) are possible in the same code path, because Zero-Overhead (Pillar 8) provides feature flags. Both poles live in the same workspace.

## 6 What does not belong in ZeroDDS

Clear delimitation — Pillar 4 (Zero-Imposed-Topology) and Pillar 9 (Zero-Dependency) forbid:

- **Global topology:** no central discovery server (except as an opt-in `discovery-server` feature, bridges layer).
- **Mandatory external service:** no cloud bindings in the core; bridges are opt-in.
- **Hidden state:** no state next to the DDS substrate. The inspect Track too is documented in the substrate (see §3 Pillar 7).
- **Vendor vocabulary:** no ZeroDDS-own IDL extension except documented vendor PIDs (`PID_SHM_LOCATOR = 0x8001`, without a MUST_UNDERSTAND bit, foreign vendors ignore silently).

## 7 Compliance statement

**ZeroDDS 1.0 is Zero-Principle-conformant at the Zero-strict pole** — all 9 pillars fulfilled, the Foundation substrate model mapped onto the DDS domain, the inspect Track placed within the substrate.

On conflicts between the DDS spec and the Zero-Principle, the order from `02_architecture.md §1` applies:

1. Correctness before performance.
2. Safety qualifiability before convenience.
3. Spec conformance before feature innovation.

Zero-Principle conformance is at the values level and does not collide with this order, because the pillars are values, not spec requirements.
