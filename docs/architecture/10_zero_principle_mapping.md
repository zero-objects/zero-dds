# Zero-Principle Mapping

> **Abhängigkeiten:** Zero-Principle Manifest (extern, gepflegt im Zero-Concept-Repo), `02_architecture.md`, `04_safety_by_architecture.md`.
>
> Track-Materialisierung via git-commits dieser Datei.

Dieses Dokument legt offen, wie ZeroDDS die Zero-Principle-Werte umsetzt — Substrat-Mapping, Pillar-by-Pillar-Befund, Strenge-Verortung. Es ist die Brücke zwischen dem Zero-Manifest (Werte, kein Spec) und der DDS-Domäne (Spec, kein Manifest).

## 1 Verortung

ZeroDDS ist die **Realtime-Pub/Sub-Substrat-Implementierung** unter dem Zero-Label. Die Domäne ist messaging-zentrisch (DDSI-RTPS, DDS 1.4), nicht graph-zentrisch — die Zero-Principle-Substrat-Begriffe (Fragment, Trail, Trait, Strain, Track) lassen sich auf DDS-Begriffe natürlich abbilden, ohne dass DDS als Graph-Datenbank umgedeutet werden muss.

Foundation-Statement 1 (Selbstähnlichkeit) gilt: jede DDS-Schicht (Domain → Topic → Sample) ist auf ihrer Ebene selbst Substrat. Die Strenge ist graduierbar — Zero-vague-Variante ist eine offline-Participant-CLI, Zero-strict-Variante ist ein DDS-Security-1.2-gebondeter Domain mit BuiltinDataTagging und Audit-Sink.

ZeroDDS verortet sich auf der Strenge-Skala explizit am **Zero-strict-Pol**: formal modelliert (32 Spec-Coverage-Docs strict-audited), kryptographisch verankert (DDS-Security 1.2 mit IdentityToken/Permissions/AccessControl), vollständig auditiert (Observability + OTLP + DDS-Security-Audit-Hooks).

## 2 Substrat-Mapping

| Zero-Concept | DDS-Counterpart | Crate / Quelle |
|--------------|-----------------|----------------|
| **Fragment** | DDS Sample (typed, key-addressed, lifecycle-aware) | `zerodds-dcps::Sample`, `zerodds-cdr` für Inhalt |
| **Trail** | Topic + ContentFilter (Filter-Vokabular) | `zerodds-dcps::Topic`, `zerodds-dcps::ContentFilteredTopic` |
| **Trait** | TypeObject + Partition + DataTag (Klassifikations-Position) | `zerodds-types`, `zerodds-qos::PartitionQosPolicy`, `zerodds-security::data_tagging` |
| **Strain** | DDS-Security Permissions + AccessControl (Sichtbarkeit/Berechtigung) | `zerodds-security`, `zerodds-security-permissions` |
| **Track** | DurabilityCache (TransientLocal-Snapshot Tx), Recorder `.zddsrec` | `zerodds-rtps::history_cache`, `zerodds-recorder` |
| **Substrat** | Domain-Participant-Cluster | `zerodds-dcps::DomainParticipant` |
| **Cluster of Ground Truth** | Domain mit Discovery-Cache als föderierte Authority | `zerodds-discovery::sedp::SedpStack` |
| **Genesis** | DomainParticipantFactory.create_participant | `zerodds-dcps::DomainParticipantFactory` |

### 2.1 Strong-Edges in DDS

Foundation §3 fordert Strong-Kanten-Garantien (referentiell zwingend, content-adressiert). DDS realisiert sie über:

- **Sample-Key-Hash + SequenceNumber:** content-adressierte Identität pro Sample (RTPS 2.5 §9.6.3.4).
- **Type-Hash SHA-256:** content-adressierte Identität pro Schema (XTypes 1.3 §7.3.1, ZeroDDS-`flatdata-1.0` §6.1).
- **GUID (GuidPrefix + EntityId):** content-adressierte Identität pro Entity (RTPS 2.5 §8.2.4.3).

Diese drei Achsen liefern die Merkle-DAG-Eigenschaft: jede Strong-Kante ist überprüfbar, Bruch ist detektierbar.

### 2.2 Vier-Schritt-Kette in DDS-Wire-Begriffen

Foundation §4 (Fragment → Trail → Trait → Strain) ist in DDS naturalisiert:

```
Sample (Fragment)
   ↓ via Topic-Match
Topic + Filter (Trail)
   ↓ via TypeInfo + Partition + DataTag
Klassifikation (Trait)
   ↓ via Permissions + AccessControl
Sichtbarkeit (Strain)
```

Berechtigungen leben **nur** in der Strain-Schicht (DDS-Security-Permissions). Das ist Foundation-konform: keine Berechtigungs-Logik in `zerodds-rtps` oder `zerodds-cdr`, sondern ausschließlich in `zerodds-security*`.

## 3 Pillar-by-Pillar-Befund

### Pillar 1 — Zero-Lock-In ✅
- OMG DDS 1.4 als offene Spec, byte-identische Wire-Compat zu Cyclone/FastDDS (`crates/discovery/tests/cyclone_*`).
- Fünf PSMs: cpp/csharp/java/python/typescript (Welle 1–4).
- Sieben Bridges: AMQP, MQTT, CoAP, gRPC, WebSocket, Zenoh, ROS2-RMW.
- `.zddsrec` Recording-Format (open, dokumentiert in `crates/recorder`).

### Pillar 2 — Zero-Hollow-Foundation ✅
- `zerodds-foundation` ist tatsächlich Foundation: keine ZeroDDS-Logik versteckt sich daneben.
- Reference-Implementation IS das Projekt; keine kommerzielle Schale.
- Lizenz: Apache-2.0 für Code (Rust-Ecosystem-Default, kompatibel zum DDS-Peer-Stack Cyclone/FastDDS, expliziter Patent-Grant). Pillar-2-Foundation-Schutz wird über Trademark des Zero-Labels und Repo-Governance realisiert, nicht über Copyleft. Spec-Coverage-Docs sind Apache-2.0-kompatibel; das Zero-Principle-Manifest selbst (extern gepflegt) bleibt CC-BY-SA 4.0.

### Pillar 3 — Zero-Notation-Lock-In ✅
- IDL als Schema-Sprache (industry-standard OMG IDL 4.2, nicht erfunden).
- XCDR1/2 + PL-CDR als Wire-Encoding (alle drei spec).
- Keine ZeroDDS-eigene DSL erzwungen; `zerodds-xml` liest QoS-XML standard-konform.

### Pillar 4 — Zero-Imposed-Topology ✅
- DDS by-design broker-frei P2P (DDSI-RTPS).
- Domain-Participants autonom, SPDP-Multicast-Discovery.
- Bridges erlauben Mix mit broker-orientierten Welten (AMQP-Broker ↔ DDS-Bus) — keine Topologie wird auferlegt.
- Cluster of Ground Truth = Domain-Cluster ist erlaubte lokale Authority, nicht globale Plattform.

### Pillar 5 — Zero-Implicit-Sharing ✅
- DDS-Security 1.2 voll: IdentityToken, PermissionsToken, AccessControl, BuiltinDataTagging.
- Topic + Partition + ContentFilter sind explizite Sichtbarkeits-Aussagen.
- Built-in Topics (PARTICIPANT/PUBLICATION/SUBSCRIPTION) sind introspectable und damit selbst Aussage, nicht versteckt.

### Pillar 6 — Zero-Context-Loss ✅
- XTypes 1.3 TypeInformation propagiert via Discovery (Type-Object + Type-Identifier-Hashes).
- TYPE_HASH-Cross-Validation in `flatdata`-Read-Path — Schema-Drift schlägt sofort als `PreconditionNotMet`, nicht als Datenkorruption.
- BuiltinDataTagging propagiert Klassifikations-Tags pro Sample.
- PID_RELATED_ENTITY_GUID erhält RPC-Endpoint-Pairings durch Migration.

### Pillar 7 — Zero-Out-of-Band ✅ (mit Begründung)
- Production-State liegt vollständig im DDS-Substrat (Discovery-Cache, History-Cache, Built-in Topics).
- **Inspect-Endpoint / Ghost-Interface**: per Zero-Principle ein *Track* mit Scope `inspect` und eigener *Strain* (Cert-Layer). Nicht Out-of-Band, weil:
  1. **Compile-Default OFF** (`#[cfg(feature = "inspect")]`) — Substrat im Release-Build hat den Track nicht.
  2. **Config-Default OFF** — auch mit Feature-Build aktiviert sich der Track nicht ohne explizite Config.
  3. **Cert-Layer mandatory** (`cert.d`-Loader, X.509-PEM, R-100..R-104) — Strain kontrolliert Sichtbarkeit auf der Permission-Ebene.

  Drei explizite Opt-ins sind die stärkste Form von „Sichtbarkeit ist Aussage, kein Default" (Pillar 5). Der Inspect-Track ist *im* Substrat erklärt (dokumentiert in `crates/inspect-endpoint/src/lib.rs`), nicht *neben* dem Substrat.

  Ghost-Inject (R-110) bypasst Production-Taps absichtlich — das ist der definierte Trail dieses Tracks: Ghost-Inject ist eine Transformation auf einem Sub-Substrat, dessen Sichtbarkeit per Strain (Cert-Auth) reguliert ist. Das ist Zero-konform genau weil es definiert und dokumentiert ist.

### Pillar 8 — Zero-Overhead ✅
- Feature-Flags überall: `security`, `iceoryx2`, `tokio-glue`, `inspect`, `live-interop`, `tcp-transport`, `shm-transport`.
- `zerodds-foundation` no_std-fähig (PoolBuffer + BufferPool ohne Heap).
- Offline-Participant funktioniert ohne UDP, ohne Security, ohne Discovery (`create_participant_offline`).

### Pillar 9 — Zero-Dependency ✅
- iceoryx2 opt-in (Stub-Adapter im Default-Build).
- zenoh opt-in (rustc-1.86-Anforderung gated).
- tokio opt-in (`tokio-glue`-Feature).
- Kein Mandatory-Broker, kein Mandatory-Cloud, keine Mandatory-PKI (DDS-Security ist opt-in).

## 4 Tech-Strategien T1–T9

| | Strategie | ZeroDDS-Realisierung | Status |
|---|---|---|---|
| T1 | Transformations | IDL→PSM-Codegen (cpp/csharp/java/python/ts), Bridge-Mappings | partial — als Codegen-Pipelines da, nicht als Transformations-DSL formalisiert |
| T2 | TGGs | nicht direkt | n/a — TGG ist heavy für DDS-Domäne, bewusste Out-of-Scope |
| T3 | Content-Addressing | ✅ Type-Hash SHA-256, Sample-Key-Hash, GUID | done |
| T4 | Versioning & Lineage | ✅ XTypes Assignability + Evolution-Rules | done |
| T5 | Federation Protocol | ✅ DDSI-RTPS 2.5 (SPDP + SEDP + reliable/best-effort) | done |
| T6 | Identity & Actors | ✅ GuidPrefix + DDS-Security IdentityToken + Permissions | done |
| T7 | Lifecycle | ✅ vollständige DDS Sample-Lifecycle (alive/disposed/unregistered + autodispose) | done |
| T8 | Schema Evolution | ✅ XTypes 1.3 voll | done |
| T9 | Audit & Provenance | ✅ Observability + OTLP + DDS-Security-Audit | done |

T1/T2 sind die einzigen Lücken — bewusst, weil die DDS-Domäne sie nicht braucht und Zero-Principle in §Concepts explizit erlaubt, Strategien projektspezifisch auszulegen.

## 5 Strenge-Verortung

ZeroDDS ist als **Zero-strict-Implementierung** konzipiert:

- **formal modelliert:** 32 Spec-Coverage-Docs (`docs/spec-coverage/`), Strict-Audit-Pass auf allen.
- **kryptographisch verankert:** DDS-Security 1.2 mit RSA-PSS-2048, AES-GCM-Crypto, X.509-Cert-Bind, CRL-Validation.
- **vollständig auditiert:** Foundation-Observability-Sinks (`zerodds-foundation::observability::Component`), OTLP-Adapter (`zerodds-observability-otlp`), Pre-Commit-Lints (`zerodds-lint`), CI mit Bench-Regression-Check und Cross-Vendor-Soak.

Zero-vague-Anwendungen (z.B. ein offline-Participant ohne Security für Lab-Tests) sind im selben Codepfad möglich, weil Zero-Overhead (Pillar 8) Feature-Flags vorsieht. Beide Pole leben im gleichen Workspace.

## 6 Was nicht in ZeroDDS gehört

Klare Abgrenzung — Pillar 4 (Zero-Imposed-Topology) und Pillar 9 (Zero-Dependency) verbieten:

- **Globale Topologie:** kein zentraler Discovery-Server (außer als opt-in `discovery-server` Feature, Bridges-Layer).
- **Mandatory External Service:** keine Cloud-Bindings im Core; Bridges sind opt-in.
- **Hidden State:** kein State neben dem DDS-Substrat. Auch der Inspect-Track ist im Substrat dokumentiert (siehe §3 Pillar 7).
- **Vendor-Vocabulary:** keine ZeroDDS-eigene IDL-Erweiterung außer dokumentierten Vendor-PIDs (`PID_SHM_LOCATOR = 0x8001`, ohne MUST_UNDERSTAND-Bit, fremde Vendoren ignorieren still).

## 7 Compliance-Statement

**ZeroDDS 1.0 ist Zero-Principle-konform am Zero-strict-Pol** — alle 9 Pillars erfüllt, Foundation-Substrat-Modell auf DDS-Domäne abgebildet, Inspect-Track innerhalb des Substrats verortet.

Bei Konflikten zwischen DDS-Spec und Zero-Principle gilt die Reihenfolge aus `02_architecture.md §1`:

1. Korrektheit vor Performance.
2. Safety-Qualifizierbarkeit vor Komfort.
3. Spec-Konformität vor Feature-Innovation.

Zero-Principle-Konformität ist Werte-Ebene und kollidiert mit dieser Reihenfolge nicht, weil die Pillars Werte sind, keine Spec-Anforderungen.
