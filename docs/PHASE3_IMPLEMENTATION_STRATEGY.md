# Phase-3 Implementation Strategy

**Stand:** 2026-05-01. Re-Skoped 2026-05-01 nach User-Feedback
"keine MVPs, Produkte bauen" (siehe Memory-Eintrag
`feedback_no_mvp_build_product`).

**Original-Entwurf:** 2026-04-30 nach der vollständigen Cluster-
Reklassifikation (Memory-Einträge `project_corba_coexistence`,
`project_ccm_container_strategy`, `project_optional_profiles_strategy`,
`project_bridge_full_stacks`, `project_ros2_architecture_decision`,
`feedback_spec_completeness_over_competition`).

**Execution-Modell:** Sequenzielle Sprint-Abfolge. Jeder Sprint
liefert **ein produktreifes Release** mit voller Spec-Coverage,
vollständigem Test-Track, Performance-Validierung und Dokumentation.
Keine MVP-/Demo-Stufen — ein Sprint endet erst, wenn das WP voll
auslieferbar ist.

**Ziel:** ZeroDDS auf Spec-Vollständigkeits-Niveau bringen, das
**Drop-in-Migration** für CORBA/CCM/SOAP-Bestand und **no_std-
Embedded-Bridges** ermöglicht. Mission: nicht "so gut wie RTI/Cyclone",
sondern **strikt darüber hinaus** — voll spec-konform, produktiv-
tauglich, ohne MVP-Schulden.

---

## 1. Kritischer Pfad

```
Phase-2-Restbacklog (✅ Sprint 0 done)
    ↓
Sprint 1: CORBA-Coexistence Produkt-Release          ── voller CORBA-3.3-Stack
    ↓
Sprint 2: CCM-Container Produkt-Release              ── voller CCM-4.0-Stack
    ↓
Sprint 3: DLRL Produkt-Release                       ── DDS-DLRL-Profil voll
    ↓
Sprint 4: XML-Wire + SOAP-PSM Produkt-Release        ── Web-Profile voll
    ↓
Sprint 5: gRPC Produkt-Release                       ── no_std-HTTP/2-Stack
    ↓
Sprint 6: CoAP Produkt-Release                       ── Embedded-IoT
    ↓
Sprint 7: MQTT Produkt-Release                       ── IoT-Backend-Bridge
    ↓
Sprint 8: WebSocket Produkt-Release                  ── Browser-Bridge
```

Sequenzieller Pfad. Jeder Sprint endet mit einem fertigen,
auslieferbaren Produkt — kein "Demo-Milestone" mit nachgelagertem
Hardening.

**Total Phase-3:** ~125-160 PW menschen-äquivalent.

---

## 2. Sprint-Phasing

### Sprint 0 — Restbacklog (✅ done 2026-05-01)

Phase-2-K-Track-Restbacklog vollständig geschlossen:
DDS-OPCUA §8.3/§8.4/§9.2/§9.3.4, DDS-TSN §7.2/§7.3,
AMI4CCM §7.5, CCM 4.0 §6.7.2/§6.7.3, RTC 1.0 §5.4.

### Sprint 1 — CORBA-Coexistence Produkt-Release (Cluster A voll)

**Aufwand:** 30-40 PW menschen-äquivalent.

**Liefert ein voll spec-konformes CORBA-3.3-Coexistence-Produkt.**
Kein "Echo-Demo"-Zwischenschritt. Bei Sprint-Ende sind alle
Conformance-Points aus CORBA-3.3 abgedeckt, die für Drop-in-
Migration aus Finanzindustrie-/Telco-Bestand erforderlich sind.

**Voller Inhalt — alles in dieser Reihenfolge:**

1. **GIOP-Wire-Codec voll** (`crates/corba-giop/`): alle 8 Message-
   Types (Request/Reply/CancelRequest/LocateRequest/LocateReply/
   CloseConnection/MessageError/Fragment) für GIOP 1.0, 1.1, 1.2
   inkl. Bidirectional-GIOP, ServiceContextList, alle Reply-
   Statuses (`NO_EXCEPTION`/`USER_EXCEPTION`/`SYSTEM_EXCEPTION`/
   `LOCATION_FORWARD`/`LOCATION_FORWARD_PERM`/`NEEDS_ADDRESSING_MODE`).
2. **IIOP TCP-Transport voll** (`crates/corba-iiop/`): TCP-Reactor +
   ProfileBody-Codec für IIOP 1.0/1.1/1.2/1.3, Connection-Pooling,
   Bidirectional-Connections, Listen-Endpoint-Konfiguration.
3. **IOR-Format voll** (`crates/corba-ior/`): stringified-IOR +
   alle 32 Standard-TaggedComponents (TAG_ORB_TYPE, TAG_CODE_SETS,
   TAG_POLICIES, TAG_ALTERNATE_IIOP_ADDRESS, TAG_SSL_SEC_TRANS,
   TAG_CSI_SEC_MECH_LIST, TAG_TLS_SEC_TRANS, …) und
   TaggedProfile-Slot.
4. **POA voll** (`crates/corba-poa/`): alle 7 POA-Policies in allen
   Modi (Lifespan TRANSIENT/PERSISTENT, IdAssignment USER/SYSTEM,
   IdUniqueness UNIQUE/MULTIPLE, ImplicitActivation IMPLICIT/NO_IMPLICIT,
   ServantRetention RETAIN/NON_RETAIN, RequestProcessing USE_ACTIVE/
   USE_DEFAULT/USE_SERVANT_MANAGER, Threading SINGLE_THREAD/
   ORB_CTRL/MAIN_THREAD). POAManager-Statemachine + Servant-Manager.
5. **Naming-Service voll** (`crates/corba-cosnaming/`): NamingContext +
   NamingContextExt + alle Bind/Resolve/Rebind/List/Destroy-
   Operations + URL-Form `corbaname:` und `corbaloc:`.
6. **Interface-Repository (IR) voll** (`crates/corba-ir/`): IDL-Type-
   Lookup zur Laufzeit (TypeCode-Hierarchie, Repository-IDs).
7. **CSIv2 Security voll** (`crates/corba-csiv2/`): TLS via
   `crates/security-pki/`, GSSUP-Token, SAS-Protocol (Establish-
   Context/CompleteEstablishContext/MessageInContext), AuthorizationToken,
   AS-Layer + SAS-Layer.
8. **Annex-A.1-Codegen voll** (in `crates/idl-cpp/`, `crates/idl-csharp/`,
   `crates/idl-java/`): IDL-3-zu-Stub/Skeleton-Codegen für alle drei
   PSMs, alle 13 Special-Types (Object/ValueBase/CORBA::any/etc.),
   Marshalling-Glue.
9. **CORBA-Object ↔ DDS-Topic-Bridge voll** (`crates/corba-dds-bridge/`):
   bidirektional, Topic-Annotation `@corba_object`, Type-Mapping,
   Lifecycle-Sync.

**Sprint-Ende-Kriterien (alle hart):**

* Cross-Vendor-Wire-Tests: TAO + omniORB Clients reden bidirektional
  gegen ZeroDDS-Endpoint mit allen GIOP-Versionen + Bidirectional-
  GIOP + LOCATION_FORWARD-Roundtrips.
* CORBA-3.3 §11 (POA) Conformance-Suite: alle 7 Policies in allen
  Kombinationen test-getrieben.
* CSIv2-Conformance: omniORB mit TLS+GSSUP authentifiziert sich
  gegen ZeroDDS, AS+SAS-Layer beidseitig durchexerziert.
* Performance-Validierung gegen TAO Baseline auf llvm-bench-host:
  IIOP-Latenz innerhalb 1.5× von TAO, Throughput innerhalb 0.8×.
* Annex-A.1-Codegen: Generierte Stubs/Skels gegen TAO + omniORB
  IDL-Beispielsuiten test-validiert (DSI/DII Roundtrip).
* DDS-Bridge: Round-Trip CORBA-Client → ZeroDDS-Bridge →
  DDS-Topic → ZeroDDS-Bridge → CORBA-Client byte-identisch.

Cross-Ref: `docs/spec-coverage/corba-3.3.md`,
`project_corba_coexistence.md`.

### Sprint 2 — CCM-Container Produkt-Release (Cluster B voll)

**Aufwand:** 25-33 PW menschen-äquivalent.

**Liefert das voll spec-konforme CCM-4.0-Container-Produkt** —
inkl. CosEventService, AMI4CCM-Connector, EJB-Bridge und D&C-
Deployment. Voraussetzung: Sprint 1 (CORBA-Coexistence) ist
abgeschlossen, weil CCM auf POA + Naming + IR aufbaut.

**Voller Inhalt:**

1. **COS-EventService voll** (`crates/corba-cos-event/`): CosEventComm
   (PushConsumer/PushSupplier/PullConsumer/PullSupplier) +
   CosEventChannelAdmin (EventChannel/ConsumerAdmin/SupplierAdmin/
   ProxyPushConsumer/ProxyPullSupplier/ProxyPushSupplier/
   ProxyPullConsumer) + TypedEventChannel (CosTypedEvent).
2. **CCM-Container Core voll** (`crates/corba-ccm/`):
   - §7 CIDL-Parser + Code-Gen (alle CIDL-3-Konstrukte: composition,
     home executor, segment, storagehome, storagetype).
   - §8 Component Implementation Framework (Servant-Base-Classes,
     Context-Iface, EnterpriseComponent, ProxyHomeRegistration).
   - §9 Container-Runtime: voller Lifecycle, Home/Servant-Mgmt,
     CosTransactions-Integration, CosNotification-Integration,
     Security-Integration mit CSIv2.
   - TimerEventService (`omg-time-1.1.md` §2.2-§2.4) voll integriert.
3. **AMI4CCM-Connector voll** (`crates/ami4ccm/` Erweiterung):
   Spec §7.6 Connector-Fragment-Codegen, §7.7 Receptacle-Pragma
   Context-Methode-Generation, §7.7 Multiplex-Receptacle, §7.8
   D&C-Deployment des AMI4CCM-Connectors.
4. **CCM-EJB-Bridge voll** (`crates/corba-ccm-ejb/`): CosTransactions↔JTA
   bidirektional, Java-CCM-Stub-Codegen für `idl-java`,
   ConnectorBean-Wrapper, EJB-Container-Side-Glue.
5. **D&C-Deployment voll** (`crates/corba-dnc/`): Plan-Loader (DPD/CPD/
   IDD/PSD-XML), Node-Manager, ExecutionManager, RepositoryManager,
   ContainerHost. OMG-D&C-3.3-konform.
6. **CCM-Components-Library voll** (`crates/corba-ccm-lib/`):
   Production-ready Beispiel-Komponenten (DDS-Bridge-Component,
   PersistenceStorage-Component, Telemetry-Component) als
   wiederverwendbare Bausteine.

**Sprint-Ende-Kriterien:**

* CCM-3.0 + CCM-4.0 Conformance-Suite voll grün.
* End-to-End: D&C-Plan deployed CCM-Component, Component publiziert
  DDS-Topic via DDS-Bridge-Component, CosTransactions koordiniert
  Multi-Component-Update, EJB-Bridge integriert Java-EE-Container.
* AMI4CCM voll spec-konform: Async-Operations + ReplyHandler +
  Connector-Fragment + D&C-Deployment alle live.
* TimerEventService Conformance-Test (`omg-time-1.1.md` Annex).
* Performance: CCM-Component-Aktivierung < 5 ms, EventChannel-Push
  < 50 µs Latenz, D&C-Plan-Apply < 500 ms für 100-Component-Plan.

Cross-Ref: `docs/spec-coverage/cos-event-service-1.4.md`,
`omg-time-1.1.md` §2.2-§2.4, `omg-ccm-4.0.md` §7-§16,
`omg-ami4ccm-1.1.md` §7.6-§7.8, `project_ccm_container_strategy.md`.

### Sprint 3 — DLRL Produkt-Release

**Aufwand:** 13-17 PW menschen-äquivalent.

**Liefert das voll spec-konforme DDS-DLRL-Profil** (`zerodds-dcps-1.4.md`
§2.1.3, `dds-xtypes-1.3.md` §2.4). Wichtigstes Differenzierungs-
Feature gegenüber RTI/Cyclone — beide Vendoren liefern DLRL nicht.

**Voller Inhalt** (`crates/dlrl/`):

1. **Object-Cache** mit Identity-Tracking + WeakRef-Container.
2. **Relationship-Resolver** (mono-/bi-direktional, kompositional/
   referentiell, kaskadiertes Update/Delete).
3. **Transaktions-Semantik** mit Begin/Commit/Rollback,
   Optimistic-Concurrency, Consistency-Level-Konfiguration.
4. **Code-Gen-Erweiterung** in `idl-cpp/idl-csharp/idl-java/idl-ts`
   für DLRL-Annotations: `#pragma DCPS_DATA_TYPE`,
   `#pragma DCPS_DATA_KEY`, `#pragma DCPS_DLRL_RELATION`.
5. **Query-Engine** mit DLRL-spezifischen Filter/Order/Limit auf
   dem Object-Cache.
6. **Subscription-Hierarchie** (HomeFactory/HomeListener/
   ObjectListener) voll spec-konform.

**Sprint-Ende-Kriterien:**

* DLRL-Spec-Beispiel-Tabellen aus DDS 1.4 §B.x byte-identisch
  reproduziert.
* Cross-Vendor-Test gegen Vortex OpenSplice DLRL (das einzige
  bekannte historische DLRL-Backend) für Wire-Kompatibilität, wo
  Spec normativ ist.
* Performance: Object-Cache-Update < 10 µs, Relationship-Traversal
  < 100 ns pro Hop.

Cross-Ref: `zerodds-dcps-1.4.md`, `dds-xtypes-1.3.md`,
`project_optional_profiles_strategy.md`.

### Sprint 4 — XML-Wire + SOAP-PSM Produkt-Release

**Aufwand:** 12-17 PW menschen-äquivalent.

**Liefert die zwei voll spec-konformen Web-Profile.**

**Voller Inhalt:**

1. **WP XML-Wire-Profile** (`crates/zerodds-xml-wire/`, ~6-9 PW):
   XML-PSM für DDS-Topic-Daten gemäß DDS-XML 1.0 §6 (Wire-Form,
   nicht nur Config). Bidirektionaler XML↔CDR-Codec, XSD-Schema-
   Generation aus IDL-Types, Streaming-Parser/Emitter, Validator.
2. **WP SOAP-PSM** (`crates/zerodds-soap/`, ~6-8 PW): voller SOAP 1.2-
   Stack inkl. WSDL-1.1+2.0-Generation aus IDL-Service-Defs (für
   Java-2000er-Bestand), MTOM-Attachments, WS-Addressing, optional
   WS-Security-1.1.

**Sprint-Ende-Kriterien:**

* XML-Wire: Cross-Vendor-Validation gegen Cyclone DDS XML-Wire-
  Implementation (wo verfügbar) byte-identisch.
* SOAP: Java-2008-Bestands-Client läuft gegen ZeroDDS-SOAP-Endpoint
  ohne Application-Code-Änderung. WSDL-Generation gegen JAX-WS
  Reference-Implementation validiert.
* WSDL-2.0-Reference-Tests aus W3C-WSDL-Test-Suite grün.

Cross-Ref: `zerodds-web-1.0.md`, `zerodds-dcps-1.4.md`,
`project_optional_profiles_strategy.md`.

### Sprint 5 — gRPC-Bridge Produkt-Release

**Aufwand:** 12-15 PW menschen-äquivalent.

**Liefert den voll spec-konformen gRPC-Bridge-Stack mit no_std-
Embedded-Target.** Alleinstellungsmerkmal: kein anderer DDS-Vendor
liefert gRPC-Bridge mit no_std-HTTP/2.

**Voller Inhalt** (`crates/grpc-bridge/`):

1. **HTTP/2 voll no_std** (RFC 7540): Connection-Preface, Frame-
   Layer (alle 10 Frame-Types), Stream-Statemachine, Flow-Control
   (Connection + Stream), HEADERS-Frame-Compression mit HPACK
   (`httlib-hpack`-Vorlage), Settings-Negotiation, GOAWAY,
   PING, RST_STREAM.
2. **HPACK voll no_std** (RFC 7541): Static-Table, Dynamic-Table
   mit Eviction-Policy, Huffman-Coding-Table, Index-Operations.
3. **gRPC-Wire voll** (Length-Prefixed-Message-Framing): Compressed-
   Flag, Length, Message-Bytes; Streaming-Modes (Unary, Server-
   Streaming, Client-Streaming, Bidirectional).
4. **Service-Bridge** zu DDS-Topics: Proto3-Schema-Generation aus
   IDL-Types, bidirektionale Type-Mapping, ServerReflection-API.
5. **TLS-Integration** über `crates/security-pki/`.

**Sprint-Ende-Kriterien:**

* Cross-Vendor-Wire-Tests gegen `tonic` (Rust gRPC), `grpc-go`,
  `grpc-java` byte-identisch.
* HTTP/2 Conformance gegen `h2spec`-Test-Suite (alle 145 Tests grün).
* HPACK Conformance gegen `hpack-test-case` (IETF Reference-
  Vectors).
* no_std-Build-Profil für `thumbv7em-none-eabihf` voll grün, ohne
  alloc-only-Pfad-Brüche.

Cross-Ref: `grpc-protocol.md`, `project_bridge_full_stacks.md`.

### Sprint 6 — CoAP-Bridge Produkt-Release

**Aufwand:** 10-12 PW menschen-äquivalent.

**Voller CoAP-Stack mit DTLS-Sicherung und no_std-Target.**

**Voller Inhalt** (`crates/coap-bridge/`):

1. **CoAP voll** (RFC 7252): UDP-Transport, Message-Layer (CON/
   NON/ACK/RST), Request/Response-Layer, Block-Wise-Transfer
   (RFC 7959), Observe-Pattern (RFC 7641).
2. **CoRE-Link-Format** (RFC 6690): Resource-Discovery.
3. **DTLS** über `crates/security-pki/` mit PSK + Cert-Mode.
4. **DDS-Topic-Bridge**: Topic-zu-Resource-Mapping
   (`/dds/<topic>/<instance>`), Observer-zu-DataReader-Wiring,
   GET/POST/PUT/DELETE als publish/subscribe/unregister/dispose.

**Sprint-Ende-Kriterien:**

* Cross-Vendor-Wire-Tests gegen `aiocoap` (Python Reference) und
  `libcoap`.
* `etsi-plugtest`-Conformance-Suite (CoAP CT4-Plugtest) grün.
* no_std-Build für `thumbv7em-none-eabihf` mit DTLS-PSK-Mode lauffähig.

Cross-Ref: `coap-rfc-7252.md`, `project_bridge_full_stacks.md`.

### Sprint 7 — MQTT-Bridge Produkt-Release

**Aufwand:** 10-12 PW menschen-äquivalent.

**Voller MQTT-5.0-Broker-Stack inkl. Client und Bridge.** Ein
weiterer Marktvorsprung — kein Rust-Crate liefert einen no_std-
MQTT-5.0-Broker.

**Voller Inhalt** (`crates/mqtt-bridge/`):

1. **MQTT-5.0 Broker voll** (OASIS MQTT-5.0): alle 15 Control-
   Packets, Topic-Filter-Matching mit Wildcards, Retained-Messages,
   Will-Messages, Session-Persistence, Shared-Subscriptions,
   Topic-Aliases, User-Properties, Reason-Codes.
2. **MQTT-5.0 Client voll**: Reconnect-Logic, Auto-ACK,
   In-Flight-Window-Mgmt, QoS-0/1/2 voll.
3. **TLS** via `crates/security-pki/`.
4. **DDS-Topic-Bridge**: bidirektional MQTT-Topic ↔ DDS-Topic,
   QoS-Mapping, Property-Forwarding.

**Sprint-Ende-Kriterien:**

* Cross-Vendor-Wire-Tests gegen `mosquitto`, `emqx`, `vernemq`.
* MQTT-5.0-Conformance gegen OASIS Test-Vectors.
* no_std-Build für `thumbv7em-none-eabihf`.

Cross-Ref: `mqtt-5.0.md`, `project_bridge_full_stacks.md`.

### Sprint 8 — WebSocket-Bridge Produkt-Release

**Aufwand:** 6-8 PW menschen-äquivalent.

**Voller WebSocket-Stack mit Browser-Integration.**

**Voller Inhalt** (`crates/websocket-bridge/`):

1. **WebSocket voll** (RFC 6455): Handshake, Frame-Layer (alle 6
   Opcodes), Masking, Continuation-Frames, Close-Codes, Ping/Pong.
2. **permessage-deflate** (RFC 7692): Per-Message-Compression.
3. **DDS-Topic-Bridge**: subscribe/publish via JSON-Frames,
   Schema-Validation aus IDL-Types.
4. **Browser-Reference-Client** in TypeScript (`crates/idl-ts/`-
   Codegen-Output).

**Sprint-Ende-Kriterien:**

* Cross-Vendor-Wire-Tests gegen `tungstenite`, `ws-rs`, Chrome/
  Firefox-Browser-Reference-Clients.
* RFC-6455-Conformance gegen Autobahn-Testsuite (alle 517 Tests
  grün, inkl. permessage-deflate).
* Browser-Demo gegen Chrome/Firefox produktiv-lauffähig.

Cross-Ref: `websocket-rfc-6455.md`, `project_bridge_full_stacks.md`.

---

## 3. Aufwands-Bilanz

| Sprint | Cluster | Aufwand (PW) | Status |
|---|---|---|---|
| 0 | Phase-2-Restbacklog | 2-3 | ✅ done |
| 1 | A — CORBA-Coexistence Produkt | 30-40 | pending |
| 2 | B — CCM-Container Produkt | 25-33 | pending |
| 3 | C-1 — DLRL Produkt | 13-17 | pending |
| 4 | C-2 — XML-Wire + SOAP Produkt | 12-17 | pending |
| 5 | D-1 — gRPC-Bridge Produkt | 12-15 | pending |
| 6 | D-2 — CoAP-Bridge Produkt | 10-12 | pending |
| 7 | D-3 — MQTT-Bridge Produkt | 10-12 | pending |
| 8 | D-4 — WebSocket-Bridge Produkt | 6-8 | pending |
| **Total Phase-3** | | **120-157 PW** | |

Wall-Clock bei agentic Claude-Team-Execution mit sequenzieller
Sprint-Abfolge: pro Sprint mehrere Tage bis 1-2 Wochen je nach
Spec-Volumen. Gesamt-Phase-3 ~6-10 Wochen Wall-Clock realistisch.

---

## 4. Risk Register

| Risiko | Sprint | Eskalations-Trigger | Mitigation |
|---|---|---|---|
| POA-Threading-Model-Komplexität | 1 | Aufwand >150% der Schätzung | TAO POA-Conformance-Suite als Goldstandard; alle Threading-Modelle parallel implementieren, nicht subset-first |
| HTTP/2 + HPACK no_std-Greenfield | 5 | Spec-Drift gegen `h2spec` | Wire-Conformance gegen `h2spec`+`tonic` im CI; HPACK-Decoder via `httlib-hpack` als Vorlage |
| DLRL-Semantik ohne Referenz-Impl | 3 | Object-Identity / Relationship-Resolver-Verhalten unklar | Spec-Beispiel-Tabelle aus DDS 1.4 §B.x als Goldstandard; OpenSplice-DLRL-Reverse-Engineering wo notwendig |
| CSIv2 + FIPS-Mode-Anforderung | 1 | Finanz-Pilot verlangt FIPS | `crates/security-*` X.509 hat FIPS-Awareness; CSIv2-GSSUP-Layer voll implementiert mit beiden Auth-Modi |
| Bench-Performance gegen TAO/omniORB | 1 | >1.5x Lücke in IIOP-Latenz | Profiling auf llvm-bench-host; CDR-Marshalling SIMD-tauglich aus WP 0.4; CFI-Disable-Pfad für Hot-Path |
| EJB-Bridge braucht JTA-FFI | 2 | Java-VM-Integration unklar | JTA-Layer voll implementiert; `java-omgdds` als FFI-Trampolin |
| MQTT-5.0-Broker-Performance | 7 | Topic-Filter >10 µs pro Match | Trie-basierter Topic-Matcher; benchmarks gegen `mosquitto` als Baseline |

**Wichtig:** Keine Mitigation der Form "Subset-First / MVP". Wenn
ein WP zu groß für den vorgesehenen Sprint-Aufwand wird, splitten
in eigenständige Sprints — jeder voll auslieferbar.

---

## 5. Test- und Validierungs-Strategie pro Sprint

| Sprint | Test-Track |
|---|---|
| 1 | TAO + omniORB Cross-Vendor-Wire-Tests; CORBA-3.3 §11 Conformance-Suite voll; CSIv2 omniORB-TLS-Roundtrip; Performance-Baseline gegen TAO; Annex-A.1-Codegen gegen IDL-Beispielsuiten |
| 2 | CCM-3.0/4.0 Conformance-Suite voll; D&C-Plan-Roundtrip; AMI4CCM Connector-Conformance; TimerEventService-Conformance; EJB-Bridge-Roundtrip |
| 3 | DLRL-Spec-§B-Beispieltabellen byte-identisch; OpenSplice-DLRL-Wire-Compat; Object-Cache-Performance |
| 4 | Cyclone-DDS-XML-Wire-Compat; Java-JAX-WS Reference; W3C-WSDL-Test-Suite |
| 5 | `tonic`/`grpc-go`/`grpc-java` Wire-Tests; `h2spec` 145 Tests; HPACK IETF-Vectors; no_std-thumbv7em-Build |
| 6 | `aiocoap`/`libcoap` Wire-Tests; ETSI CT4-Plugtest; no_std-DTLS-PSK |
| 7 | `mosquitto`/`emqx`/`vernemq` Wire-Tests; OASIS MQTT-5.0-Test-Vectors; no_std-Build |
| 8 | `tungstenite` Wire-Tests; Autobahn-Testsuite 517 Tests inkl. permessage-deflate; Browser-Reference-Client |

Jeder Sprint hat einen **harten Sprint-Ende-Kriterien-Block** (s.o.) —
Sprint endet erst, wenn alle Punkte grün sind.

---

## 6. Konkrete erste Tasks (Sprint 1 Setup)

1. **CORBA-Spec-PDFs einpflegen**:
   - `docs/standards/cache/omg/corba-3.3-part1.pdf`,
     `corba-3.3-part2.pdf`, `corba-3.3-part3.pdf` (Security/CSIv2),
     `event-service-1.4.pdf`, `naming-1.3.pdf`, `dnc-3.3.pdf`.
2. **Crate-Skeletons im Workspace anlegen** — alle gleichzeitig,
   damit Inter-Crate-Dependencies früh definiert sind:
   `crates/corba-giop/`, `crates/corba-iiop/`, `crates/corba-ior/`,
   `crates/corba-poa/`, `crates/corba-cosnaming/`, `crates/corba-ir/`,
   `crates/corba-csiv2/`, `crates/corba-dds-bridge/`. Jeder mit
   `Cargo.toml`-Skeleton, `lib.rs`-Doc-Header, no_std+alloc-Default,
   Workspace-Lints aktiv.
3. **CI-Job-Set für Sprint-1**:
   - `corba-wire-conformance` gegen TAO + omniORB (Docker-Sidecar
     im CI-Image).
   - `corba-bench` gegen TAO als Baseline auf llvm-bench-host
     (siehe Memory `reference_bench_hosts`).
4. **GIOP-Wire-Codec** als erste konkrete Implementation —
   `crates/corba-giop/src/header.rs`, `request.rs`, etc. komplett
   (alle 8 Message-Types in einem WP, nicht inkrementell).

---

## 7. Cross-Refs

- **Spec-Coverage-Files:**
  - Phase-3-WP-Files: `corba-3.3.md`, `cos-event-service-1.4.md`
  - Reklassifizierte: `zerodds-dcps-1.4.md`, `zerodds-rpc-1.0.md`,
    `idl4-cpp-1.0.md`, `idl4-csharp-1.0.md`, `idl4-java-1.0.md`,
    `omg-ccm-4.0.md`, `omg-ami4ccm-1.1.md`, `omg-rtc-1.0.md`,
    `omg-time-1.1.md`, `dds-xtypes-1.3.md`, `zerodds-web-1.0.md`,
    `coap-rfc-7252.md`, `websocket-rfc-6455.md`, `mqtt-5.0.md`,
    `grpc-protocol.md`
  - Bleibt rejected: `ros2-rmw.md` (Cluster E)
- **Memory:**
  - Strategie: `feedback_spec_completeness_over_competition`,
    `feedback_no_mvp_build_product`,
    `project_corba_coexistence`, `project_ccm_container_strategy`,
    `project_optional_profiles_strategy`,
    `project_bridge_full_stacks`,
    `project_ros2_architecture_decision`
- **Bestehende WP-Foundation:**
  - WP 0.4 CDR-Prototyp (`crates/cdr/`) — Foundation für GIOP
  - WP 1.x RTPS-Stack (`crates/rtps/`) — Cross-Vendor-Pattern
    übernehmbar
  - `crates/security-pki/` — Foundation für CSIv2 + DTLS + TLS
- **Bench-Hosts:** `reference_bench_hosts.md` — llvm (bare-metal),
  pivot (LXC).
