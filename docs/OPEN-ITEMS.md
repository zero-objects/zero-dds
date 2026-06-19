# Open Items — Persistente Backlog-Liste

Diese Datei trackt **offene Engineering-Items**, die in einem
abgeschlossenen Sprint identifiziert aber bewusst zurückgestellt
wurden. Pro Item: Was, Warum-deferred, Implikationen, Wann-pickup,
und ein Pfad zur Pick-up-Spec.

Live-Issue-Tracker (gitlab.sandra-kessler.eu) ist die Authoritative-
Quelle für aktive Arbeit. Diese Datei ist die **Engineering-
Diary-Version** — was wissen wir, was haben wir bewusst *nicht*
gemacht, und warum.

## Konvention

* Pro Item ein eigenes `*-followup.md` im thematischen `docs/`-Verzeichnis (`docs/perf/`, `docs/interop/`, `docs/architecture/` …)
* Filename-Pattern: `<sprint-id>-<topic>-followup.md` ODER `<topic>-followup.md`
* Inhalt-Template:
  - **Status** (deferred / partial / blocking)
  - **Datum** + **Sprint-Kontext**
  - **Was ist offen** (technisch konkret)
  - **Warum offen** (Trade-off der bewusst ging)
  - **Implikationen** wenn nicht implementiert (funktional / perf / spec / UX)
  - **Wann pick-up sinnvoll** (Trigger-Events)
  - **Implementations-Pfad** (geschätzte Dauer + Phasen)

## Currently Open

* ~~`docs/safety-flag-drift.md` — safety-Feature-Flag halb-verdrahtet~~ ✅ **DONE 2026-06-13**: `safety = []`-Marker auf alle 19 fehlenden SAFE-Crates; `crates/safe-crates-only`-Meta-Crate angelegt → `cargo build -p safe-crates-only --no-default-features --features safety` baut real no_std grün (Doc-✓ eingelöst); latenter no_std-cfg-Bug in `discovery/security/stack.rs` gefixt

### Performance

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| ~~**D.5e Phase 3 — Deadline-Heap-Worker**~~ ✅ | [`docs/perf/d5e-phase3-deadline-heap-followup.md`](perf/d5e-phase3-deadline-heap-followup.md) | **DONE 2026-06-14 (df042009)** | Phase A (Scheduler-Skelett) + B (Integration) + C (make-default: `scheduler_tick=true`, `tick_loop` als `ZERODDS_SCHEDULER_TICK=0`-Escape-Hatch) + **B-2** (Tick/Housekeep-Event-Split, Deadline/Lifespan/Liveliness exakt-terminiert via `NextDue`). Idle-CPU 150/s→10/s. **Gate verifiziert (codepit, scheduler_tick=1):** zerodds↔Cyclone/FastDDS 13/13, ↔OpenDDS 9/13 (Spec-Max), full-AC live joinbar, keine Regression |

### Interop

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| ~~**QoS-Profile XML-Handling**~~ ✅ | [`docs/interop/qos-profile-xml-followup.md`](interop/qos-profile-xml-followup.md) | **DONE 2026-06-13** | `zerodds_xml::QosProfileRegistry` (load + `Lib::Profile` + base_name-Inheritance → `WriterQos`/`ReaderQos`, 5 Tests) + `From<WriterQos> for DataWriterQos`/`ReaderQos` in dcps → `create_datawriter(topic, reg.writer_qos("Lib::Prof")?.into())`. Phase-D-Live-Rig optional |
| ~~**ShapeExtended-Type Support**~~ ✅ | [`docs/interop/shape-extended-followup.md`](interop/shape-extended-followup.md) | **DONE 2026-06-13** | `ShapeExtendedType` + `ShapeFillKind` in `crates/dcps/src/interop.rs` (keyed, CDR-LE, 4 Unit-Tests byte-exakt) + `ZERODDS_SHAPE_EXTENDED=1`-Toggle in den shapes-demo-Examples → RTI-7.x-ShapesDemo ohne `-dataType Shape`-Flag |
| ~~**ROS-2 Reader-XCDR1-Offer**~~ ✅ | [`docs/interop/ros2-reader-xcdr1-offer-followup.md`](interop/ros2-reader-xcdr1-offer-followup.md) | **DONE** | ros_defaults() (ROS out-of-the-box, e2e 20/20) + per-Endpoint DataReader/WriterQos.data_representation (B5) |

### PSM / Bindings

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| ~~**F-PSM-CXX-readcond-segv**~~ ✅ | [`docs/cpp/psm-cxx-readcond-segv-followup.md`](cpp/psm-cxx-readcond-segv-followup.md) | **DONE 2026-06-13 (332c0d5d)** | Root-Cause: Condition-Structs waren `repr(Rust)` → `header`-Diskriminante nicht @0 → Garbage-Cast → Linux-SIGSEGV. Fix: `#[repr(C)]` + offset-Regressionstest (`condition_header_at_offset_zero`) + cbindgen-opaque-Forward-Decls; `#[ignore]` raus, codepit-Smoke grün |

### DCPS / Discovery

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| ~~**F-DCPS-latency-self-match-timeout**~~ ✅ | [`docs/dcps/latency-self-match-timeout-followup.md`](dcps/latency-self-match-timeout-followup.md) | **DONE 2026-06-13 (346314bf)** | Echter Bug: intra-runtime-Self-Match lieferte Samples, meldete aber keinen matched-Status (Count=reader_proxy_count, kein Wire-Proxy). Fix: matched-Count/-Handles = distinct Union Wire-Proxies + Same-Participant intra-runtime-Peers; `#[ignore]` raus, codepit grün |
| **C1 Multicast-freie Discovery** | [`docs/interop/ros2-c1-multicast-free-discovery-followup.md`](interop/ros2-c1-multicast-free-discovery-followup.md) | ✅ done | Unicast-Peers + `ZERODDS_NO_MULTICAST`; e2e ZeroDDS↔ZeroDDS **und** ZeroDDS↔Cyclone mcast-frei; Scaling **50→2,9s / 100→19,9s** all-to-all; `ZERODDS_MAX_PEER_PARTICIPANTS` |
| **C3 Große Daten — Rest** (Latenz-Zahlen + variable-Zero-Copy) | [`docs/interop/ros2-c3-large-data-wifi-followup.md`](interop/ros2-c3-large-data-wifi-followup.md) | ~1 PT | **1-MiB-Cap gefixt + 2/4/8 MB DCPS-e2e + Real-WiFi 2/4 MB cross-machine done**; Latenz Loopback p50=40µs/p99=83µs; variable-Zero-Copy via SHM done (B8). **WiFi-Discovery-`participants=0` (B7): ZeroDDS-Robustheits-Fix GELANDET (f7ba0b92)** — Initial-Announcement-Burst (Default 10×200ms bis matched, analog FastDDS) statt 1×+5s; behebt verlorene Erst-Beacons im Cold-Start-/802.11-Power-Save-Fenster. Cadence deterministisch bewiesen (`initial_announce_burst.rs`). *Frühere „kein ZeroDDS-Bug/Netz-Infra-Limit"-Einordnung war Excuse.* Rest: Power-Save bleibt OS-ergänzend, aber der Stack trägt jetzt seinen Teil |
| ~~**C6/C7/C8 ROS-Surfacing**~~ ✅ | [`docs/interop/ros2-c6c7c8-surfacing-followup.md`](interop/ros2-c6c7c8-surfacing-followup.md) | **DONE 2026-06-12** | **C6** `RuntimeConfig::multi_robot()` WAN-Profil (9d14829e); **C7** `SecurityProfile::from_enclave_dir/from_env` + `zerodds_runtime_create_secure_from_env` + rmw-Shim `--features security` SROS2-Enclave-Load (3361c94b); **C8** `zerodds-ros2-shim doctor`+`graph` discovery-unabhängige Diagnose (1821f249). Alle mit Tests + e2e |

### Packaging / Distribution

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| **Language-Binding-Registry-Publish (PyPI / npm / Maven / NuGet)** | [`docs/packaging/language-binding-publish-followup.md`](packaging/language-binding-publish-followup.md) | 2-3 Sprints | RC3-Vorbereitung, Anwendungsentwickler-Onboarding (heute "git clone + cargo build" statt `pip install`) |
| **Bounded-Collection-Enforcement (typisierter Pfad)** | [`docs/interop/bounded-sequence-enforcement-followup.md`](interop/bounded-sequence-enforcement-followup.md) | **ERLEDIGT 2026-06-11** | Bounds (XTypes §7.4.3) werden beim Encode in **allen vier Codegens** erzwungen: idl-rust (seq inkl. verschachtelt, narrow+wide string, map, union-arms, Array-Element), idl-java/-csharp (seq, narrow+wide string), idl-cpp (seq, narrow string; `throw std::length_error` + konditionales `<stdexcept>`). Je ein `bounded_collections`/`bounded_sequence`-Test pro Codegen. Rest nur: cpp nested/wstring (im cpp-Encode generell „nicht unterstuetzt") |

### Documentation / Website

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| **Documentation-Trail — inhaltliche Vertiefung** | [`docs/website/trail-bilingual-followup.md`](website/trail-bilingual-followup.md) | rc4 | **Bilingual DE+EN erledigt** (alle 7 Stationen `data-lang`-verdrahtet). Offen: inhaltlicher Ausbau der dünnen Mirror-Stubs zu echten Web-Trail-Kapiteln (heute nur GitHub-Verweis), pro Station EN→DE im bestehenden Muster |
| **rmw-zerodds-shim — Multi-Distro-Live-Smoke in CI** | [`docs/interop/rmw-multidistro-ci-followup.md`](interop/rmw-multidistro-ci-followup.md) | rc4 | RMW-Oberfläche **code-seitig komplett** (Pub/Sub+Services+Wait-Sets+Loaning+REP-2009, kein UNSUPPORTED). Offen: durchgehender Live-`ros2`-Smoke (`RMW_IMPLEMENTATION=rmw_zerodds_cpp`) als grünes CI-Gate über Humble/Iron/Jazzy — reine Verifikations-Infra, kein Funktions-Gap |
| **Tools — Kartierung / Integration / Doku-Ausbau** | [`docs/website/tools-docs-followup.md`](website/tools-docs-followup.md) | rc4 | CLI-Tools vorhanden + man-pages + per-Tool-Seiten, aber als Gesamtbild unterkartiert: Aufgaben→Tool-Matrix, Tool↔Subsystem↔Use-Case-Querverlinkung, Beispiele/Rezepte statt nur Flag-Referenzen, aufgaben-orientierte Handbücher. Content-/IA-Arbeit, kein Korrektur-Fix |

### CORBA — Extra-Mile (alle 9 Lücken vs omniORB/TAO/JacORB)

Master-Plan (Analyse + Spec + Umsetzung + Tests, alle 9 Punkte):
[`docs/corba-extra-mile-plan-2026-06-07.md`](corba-extra-mile-plan-2026-06-07.md).
Identifiziert via 4 code-belegter Tiefen-Analysen 2026-06-07 — *neuer Scope*
nach „CORBA 5-Punkte-Programm + Perf-Baseline abgeschlossen", keine unfertige Baustelle.

| Punkt | Wave | Aufwand | Trigger / Hinweis |
|---|---|---|---|
| ~~#3 SSLIOP-Connection-Pooling~~ ✅ | 1 | klein | **DONE 2026-06-07 (23cda7e7)** — Connector poolt TLS nach (addr,sni,cfg); in_use-Leak gefixt; 16/16 cross-ORB grün |
| ~~#5 UIOP (Unix-Socket-Transport)~~ ✅ | 1 | mittel | **DONE 2026-06-07 (0752dcb1)** — Transport::Uds + connect_uds/serve_uds + TAG_ZERODDS_UDS_TRANS + Vendor-Spec; macOS+Linux grün |
| ~~#8 GIOP 1.0/1.1-Versions-Honorierung~~ ✅ | 1 | klein | **DONE 2026-06-07 (b35e9453)** — Server antwortet in Request-Version, Client aus IOR-Profil; 4 Tests + 16/16 cross-ORB grün |
| ~~DII live-invoke / DSI-Server-Bind~~ ✅ | 1 | klein-mittel | **DONE 2026-06-07 (a3b82468)** — Request::invoke + dispatch_dsi; DII add→5 + DSI reverse e2e. Refinement: out-arg-TypeCode-Splitting |
| #2 CSIv2 — ZeroDDS VOLL DONE ✅ / nur Live-JacORB-Client-Rig offen (Fremd-Gate) | 2 | mittel-groß | **ZeroDDS-Seite VOLL DONE (9a067053 + 23ea7d8e)**: SASContextBody-Wire-Codec + wire-korrekte GSSUP-Encapsulation + GSS-InitialContextToken-Wrap (0x60+OID, byte-exakt getestet) + Client-Inject + Server-Eval + GSSUP-e2e (alice/secret→durch, falsch/keine→NO_PERMISSION); 16/16 cross-ORB ohne Regression. **cross-ORB-WIRE BELEGT (44772417)**: GSSUP byte-identisch zu JacORB 3.9 `InitialContextTokenHelper` — **dabei echten Bug gefunden+gefixt**: username/password waren CDR-String (len+NUL) statt `sequence<octet>` (CSIv2 §16.2.3), hätte JacORB-Auth gebrochen. JacORB ist der GSSUP-Peer (nicht omniORB, das hat kein GSSUP). **VOLLER LIVE-SAS-HANDSHAKE FORWARD ✅ (65782709)**: ZeroDDS-Client-GSSUP von JacORBs echtem SASTargetInterceptor/ListGssUpContext validiert. **REVERSE ✅ in-repo (2026-06-12, f7b7198e)**: expliziter Server-Eval-Decode-Chain-Test (`SasMessage::decode_encapsulation`→`EstablishContext`→`from_gss_token`) auf foreign-format GSS-wrapped GSSUP (byte-identisch JacORB), BE+LE — ZeroDDS-als-SAS-Target dekodiert Fremd-Wire. **Rest = NUR Live-JacORB-Client→ZeroDDS-Server-Rig (codepit, Fremd-Prozess)** — kein ZeroDDS-Code-Gap. |
| ~~#4 TypeCode-Indirection (Decode)~~ ✅ | 3 | mittel | **DONE 2026-06-07 (f6530c43)** — positions-tracking Decoder + Cache; rekursiv→Recursive-Marker, wiederholt→geklont; 4 byte-exakte Tests; 193 cdr-Tests grün. (Encode-seitige Indirection = separates Feature) |
| #1 Valuetype-Wire (§15.3.4) — Core ✅ / chunked+Truncation+Custom offen | 3 | groß | **CORE DONE 2026-06-07 (04cb29af/81e0d391/fb181de8/f8893b18)**: value_wire-Engine (value_tag single repo-id + null + **Value-Sharing-Indirection**, JacORB-Bit-Layout-verifiziert), Codegen emittiert `<Name>Value` + ValueBase/ValueMarshal/Factory mit **Inheritance-State-Flattening**, GIOP-e2e (3 Tests: Roundtrip+Sharing+Flattening über echtes IIOP), **cross-ORB byte-identisch zu JacORB 3.9 bidirektional** (Golden-Vektor + Decode). ~~**OFFEN als Folge-Features**: chunked encoding → Truncation~~ ✅ **DONE 2026-06-07 (e85eefec)**: write_chunked (0x7fffff0e + chunk + end_tag, byte-identisch JacORB) + Truncation-Decode (most-derived unknown → Basis lesen, Rest skippen) + Custom-Marshalling (über chunked-Pfad) + Codegen `<NAME>_BASE_IDS`. ~~Rest-Folge: codebase-URL, multi-chunk + nested chunked Values~~ ✅ **ALLE DONE**: codebase-URL (`codebase_value_tag_and_roundtrip`), nested-chunked + multi-level-end-tags + Truncation-Decode (`nested_chunked_value_in_tail_is_consumed_on_truncation`, `shared_end_tag_closes_nested_and_outer`), und **multi-chunk ENCODE 2026-06-12 (2c462bb1)**: `ValueWriter::write_chunked_tree` + `ChunkedNode` — chunked Value mit nested chunked Children an Chunk-Grenzen; Leaf byte-identisch zu `write_chunked`; nested-Tree byte-identisch zum hand-gebauten Decode-Test-Wire; Roundtrip mit Base-Truncation. **Live-Op cross-ORB (0c0291e9)**: ZeroDDS invokt `ValueEcho::echo(in Point)->Point` auf live JacORB-Server — grün auf codepit. **#1 vollständig geschlossen.** |
| ~~#6 Client-AMI~~ ✅ | 4 | mittel-groß | **DONE 2026-06-07 (994bc563/c0d5ace7/aa1b15d2/157cabe0)**: AmiClient-Runtime (multiplexende Connection, Callback §22.5 + Polling §22.6, request_id-Korreliert, event-driven) + AsyncCorbaChannel-Trait (Layering) + Codegen ami_emit.rs gated auf `@ami` (AmiHandler-Trait + sendc_/sendp_ + typisierter Poller) + 5+2 e2e + compile_check + **cross-ORB live JacORB 3.9** (Callback add + Polling divmod, codepit). Oneway von AMI ausgenommen |
| #7 Bidirectional GIOP (§15.8) — Core ✅ / cross-ORB config-gated | 4 | groß | **CORE DONE 2026-06-07 (818624cc)**: BiDirEndpoint — ein Peer über EINER Connection sendet+bedient Requests (Server-Rückruf über client-geöffnete Connection), request_id-Parität (Originator gerade/Acceptor ungerade §15.8), Listen-Point-Annoncierung via BiDirIIOPServiceContext (Tag 5 Encapsulation) auf dem Wire, reentrantes collect_reply, Out-of-Order-Stash; 3 e2e über echtes TCP-Paar. **OFFEN (extern)**: cross-ORB BiDir braucht Fremd-ORB-Config (omniORB `-ORBofferBiDirectionalGIOP 1` + BiDirectional-POA-Policy + C++-Callback) — wie #2-Fremd-Handshake; neues Wire-Artefakt (BiDir-SC) ist ZeroDDS↔ZeroDDS belegt, Rest = Standard-GIOP (16/16 cross-ORB grün) |
| #9 OTS ✅ / Trading ✅ / RT-CORBA ✅ | 5 | je groß | **DONE 2026-06-07 (904aae15 + f3a58ba2 + a48a9c46)**: **OTS** `corba-cos-transactions` (otid_t + PropagationContext/ServiceContext id=0 + 2PC-Engine + Current/Coordinator/Terminator, 24 Tests inkl. verteilter Bank-Transfer-e2e); **CosTrading** `corba-cos-trading` (Constraint-Sprache + Trader, 17 Tests); **RT-CORBA** `corba-rt` (Priority/PriorityModel/PriorityMapping/Threadpool-Lanes/PriorityBands/RTCorbaPriority-SC-id10/RtCurrent, 12 Tests). **OTS-cross-Check — ZeroDDS-Wire byte-identisch JacORB belegt; Live-Handshake = JacORB-OTS-Limit (2026-06-12 codepit-verifiziert)**: ZeroDDS-Seite vollständig: `otid_t` byte-identisch JacORB (`byte_exact_golden_be` = `otid_tHelper.write`) + `PropagationContext` byte-identisch (`propagation_context_byte_identical_to_jacorb`) + distributed-2PC-e2e commit+rollback (`ots_distributed.rs`). **KORREKTUR früherer Notiz** (war ungenau): (a) **TAO ist vollständig auf codepit** (TAO 2.5.24 OpenDDS-gebündelt, alle Kern-Libs) — nur die optionale `libTAO_CosTransactions`-orbsvcs-Komponente ist nicht mitgebaut + kein ACE/TAO-Source zum Nachbauen. (b) **JacORB HAT eine lauffähige OTS** (`org.jacorb.transaction.TransactionService` + `CoordinatorImpl`/`TransactionCurrentImpl`/`ResourceImpl` + Client/Server-`ContextTransferInterceptor`); über GIOP erreichbar, `create()`+`get_coordinator()` ok nach `start(POA,N)` Pool-Init. JacORBs `CoordinatorImpl.get_txcontext()` + `recreate()` sind zwar `NO_IMPLEMENT` (keine programmatische Context-Ein/Ausfuhr), aber JacORB propagiert OTS-Context über seine `Client/ServerContextTransferInterceptor` bei echten Invocations. **LIVE-CROSS-ORB-OTS-HANDSHAKE ✅ DONE 2026-06-12 (codepit)**: JacORB-`Current.begin()`-Transaktions-Client invokt ZeroDDS-`CorbaServer` über echtes IIOP; JacORBs Interceptor hängt PropagationContext als SC id=0 an; ZeroDDS-Server captured (neuer `CorbaServer::on_request_contexts`-Hook) + dekodiert (timeout=30, live ~300-Byte-Coordinator-IOR). Test `jacorb_live_ots_handshake` (#[ignore], JACORB_OTS-gated) + Harness `competitors/jacorb/ots/`. **Dabei echten Cross-ORB-Befund + Fix**: JacORBs Interceptor sendet SC id=0 NICHT als spec-bare-Struct (OTS §10.4.6), sondern als `Codec.encode(any)` = BO+TypeCode+Value; `from_service_context_data` jetzt liberal (führenden TypeCode skippen). In-repo-Golden `jacorb_live_capture` (3 Tests). **#9 OTS vollständig — ZeroDDS-Wire byte-belegt + Live-End-to-End-Handshake gegen JacORB grün.** |

### CORBA — Zusatz-Vollständigkeit (über die 9 Lücken hinaus, 2026-06-07)

| Punkt | Status | Hinweis |
|---|---|---|
| **AMH** (server-seitiges Async §22.9) ✅ | DONE (c2ef27cc) | AmhEndpoint+AmhResponseHandler, verzögerte out-of-order Replies, 2 e2e |
| **Portable Interceptors Voll-Spec** ✅ | DONE (096c6a18) | RequestInfo (SC add/get, forward_reference) + benannte Points + OTS/CSIv2 durch PI geroutet, 3 Tests |
| **Interface Repository Vollständigkeit** ✅ | DONE (aa173b74) | typisierte *Def-Details (Operation/Attribute/Interface) + Contained (scoped lookup, absolute_name), 4 Tests |
| **Live-Valuetype cross-ORB** ✅ | DONE (0c0291e9) | ZeroDDS↔JacORB Operation-Level (codepit) |
| #7 cross-ORB BiDir ✅✅ | DONE | (a) BiDir-SC byte-identisch omniORB 4.3.3 (4c6c3236); (b) **VOLLER LIVE-CONNECTION-REUSE gegen JacORB**: ZeroDDS-Originator öffnet BiDir-Connection, registriert Callback, ruft callback_hello → JacORB ruft `hello()` ZURÜCK über DIESELBE Connection, ZeroDDS bedient reentrant (codepit grün). Nötiger Production-Fix dabei: handle_request extrahiert Object-Key aus ProfileAddr/ReferenceAddr (§15.4.2). Harness: competitors/jacorb/bidir/run_bidir_server.sh |
| **CSIv2-GSSUP byte-identisch JacORB** ✅ | DONE (44772417) | + echter Bug gefixt (sequence<octet> statt String) |
| **OTS otid_t byte-identisch JacORB** ✅ | DONE (44772417) | `otid_tHelper`-Capture; PropagationContext-Vollform (live coord/term-Refs) = Folge |

### Docs / Website (RC3-html-Konsistenz)

| Item | File | Geschätzt | Trigger |
|---|---|---|---|
| ~~**vendor-feature-matrix.html Voll-Resync**~~ ✅ | [`docs/interop/vendor-feature-matrix.html`](interop/vendor-feature-matrix.html) | **DONE 2026-06-13** | HTML auf `.md`-Source-of-Truth gesynct: Durability-Service-Zeilen (Transient/Persistent ✗→✓¹⁰), neue Fußnote ¹⁰ (`zerodds-durability-svc`), Defizit-Liste (Transient/Persistent durchgestrichen = erledigt, nur Gegenrichtung offen). Restliche ZeroDDS-Spalte verifiziert identisch zur `.md`. |

### Spec-Coverage-Reconcile (Audit-Sweep 2026-06-13)

Aus dem doc-für-doc-Reconcile der Spec-Coverage-Ledger (Neutralisierung
temporaler/Phasen-Bezüge + Code-Re-Audit jedes open/partial/rejected-Items).
Diese fünf sind **echte, actionable not-ready-work-Items** für den Implementer;
alle anderen „Phase/Sprint"-Marker waren Labels auf fertiger Arbeit (reconciled)
oder bereits gelandet (siehe „bei der Gelegenheit geschlossen" unten).

| Item | Status | Ort | Was fehlt | Pfad / Quelle |
|---|---|---|---|---|
| ~~**idl-cpp XCDR2-Encode nested @appendable/@mutable-Struct-Member**~~ ✅ | **DONE 2026-06-13** | dds-xtypes-1.3 §7.2.2.4.4 | Splice-Ansatz: `scoped_struct` (any-ext) + `typespec_supported` für direkten Member erweitert; nested @appendable/@mutable werden via `topic_type_support<Nested>::encode`/`decode` gespliced (deren DHEADER erzwingt 4-Align → byte-korrekt unter XCDR2). 3 clang-Roundtrip-e2e (byte-exakt + 2 roundtrips). seq/array-Elemente bleiben bewusst final-only (separates Folge-Item). | `crates/idl-cpp/tests/xcdr2_wire_vectors.rs` |
| ~~**DDS-XRCE produktives DTLS + TLS Crypto-Backend**~~ ✅ | **DONE 2026-06-13** | dds-xrce-1.0 §3.1.9/§3.1.10/§11.4 | DTLS: `WebrtcDtls`/`WebrtcDtlsServer` (Feature `dtls`, `webrtc-dtls` 0.12 über UDP, analog coap-bridge §7.1) — e2e `dtls_e2e.rs` echter Handshake + verschlüsselter Round-Trip. TLS: `RustlsTlsClient`/`RustlsTlsServer`/`RustlsTlsStream` (Feature `tls`, `rustls` 0.23 über `TcpStream`, self-signed via `rcgen`, u16-LE-Framing) — e2e `tls_e2e.rs` echter Handshake + XRCE-`Message`-Round-Trip. Default-`std`-Build regression-frei; clippy beide Features sauber. Non-normative Profile (§11.4). | [`docs/spec-coverage/dds-xrce-1.0.md`](spec-coverage/dds-xrce-1.0.md) |
| ~~**Java-PSM §8 Reflection-Auto-Marshalling + createType(Class<?>)**~~ ✅ | **DONE 2026-06-13** | dds-java-psm-1.0 §1.2 + §7.8.1.3 | `ReflectionTypeSupport<T>` marshallt Plain-Beans (POJOs+Records) ohne IDL via `java.lang.reflect` nach XCDR2 — **byte-identisch** zum typisierten `Xcdr2Writer`-Pfad (Bean==V-2/V-4/V-5/V-6/V-8/V-9). Tab.8.1-Mapping, `T[]`/`List`→sequence (DHEADER-Regel), `Map`→map, nested rekursiv, `@Extensibility`/`@Key`/`@Id`/`@Optional`/`@MustUnderstand` reflektiv per Simple-Name honoriert (FINAL/APPENDABLE/MUTABLE), Key-Hash §7.6.8.4. `DynamicTypeFactory.createType(Class<?>)` liefert konsistentes `DynamicType`-Modell. 14 Tests + 51/51 Suite + Java-8-Profil grün. | [`docs/spec-coverage/dds-java-psm-1.0.md`](spec-coverage/dds-java-psm-1.0.md) |
| ~~**zerodds-async `spawn_in_tokio` (Tick-Loop)**~~ ✅ | **DONE 2026-06-13** | zerodds-async-1.0 §4 | `AsyncDomainParticipantFactory::spawn_in_tokio` (+`_with_qos`, Feature `tokio-glue`): Live-Participant, dessen Tick-Loop als tokio-Task statt `zdds-tick`-`std::thread` läuft (spart 1 Thread/Participant). Mechanik: `RuntimeConfig::external_tick` unterdrückt den internen Thread; Tick-Body extrahiert in `run_tick_iteration`+`TickState`, getrieben via pub `DcpsRuntime::tick_driver()`→`DcpsTickDriver`; Diagnose `tick_count()`. Tests: `dcps/tests/external_tick.rs` + `dcps-async/tests/spawn_in_tokio.rs` (tokio treibt Tick, voller Writer, sauberer `shutdown`). 454 dcps-Lib-Tests + deadline/liveliness regression-frei; clippy sauber. Recv-Worker unverändert. | [`docs/spec-coverage/zerodds-async-1.0.md`](spec-coverage/zerodds-async-1.0.md) |
| ~~**zerodds-py ROS-2-pytest in reproduzierbarem CI**~~ ✅ | **VERIFIZIERT GRÜN 2026-06-13** | zerodds-py-1.0 §6.4 | `tests/ros2/{conftest,test_rmw_zerodds_interop}.py` **2 passed** auf codepit (ROS 2 Humble/RoboStack): rclpy `init`+`create_node`+`std_msgs/String`-Pub/Sub-Roundtrip über ZeroDDS-RMW. Reproduzierbarer Runner `crates/rmw-zerodds-shim/rmw_c/run_ros2_pytest.sh` (baut C-Layer `librmw_zerodds_cpp.so` + registriert + pytest). Falle dokumentiert: cargo-Build erzeugt nur präfigierte `librmw_zerodds.so`; stock-rclpy braucht den C-Layer. Verbleibend: reiner ROS-2-CI-Job, der den Runner fährt (Infra, kein Code-Gap). | [`docs/spec-coverage/zerodds-py-1.0.md`](spec-coverage/zerodds-py-1.0.md) |
| ~~**flatdata Zero-Copy — Cross-Host + Auto-Binding + Benches**~~ ✅ | **DONE 2026-06-14 (11/11), codepit-verifiziert** | zerodds-flatdata-1.0 | **Alle 9 funktionalen Items done (codepit-verifiziert):** §9.1/§9.3 `read_ref()`+`FlatSampleRef::Drop`, §5.1 `evict_stale()`, §4.2 event-driven Notify (In-Memory Condvar **+ POSIX cross-process Futex** im SHM-Header, `futex_notify_wakes_consumer_across_mappings` grün) + `read_flat_blocking`, §10.5 `write_bp` Backpressure, §7.1 0600 (`segment_is_owner_only_0600` grün) — alle im flatdata-Crate gebaut (Commits 9f42a56f/b8fc3fe4/ea513b61/182b4e7a/b84daae2/c22de49e). §3.2 Auto-Bind im SEDP-Hook, §4.3 `same_host_udp_skip_set` Cross-Host-Split, §10.3 `same_host_e2e` 4/4 grün — bereits im Produktions-`same-host-shm`-Pfad (Wave 4b/ADR-0006). **§11.2** Throughput-Bench `flat_throughput_1kb` = **1,09 GiB/s + ~1,05 Melem/s**; **§11.3** `zero_alloc.rs` dhat-Test = **0 Heap-Blocks**/1000 Writes. Alle Items geschlossen. | [`docs/spec-coverage/zerodds-flatdata-1.0.md`](spec-coverage/zerodds-flatdata-1.0.md) |

Test-Infra (kein Code-Gap): **Java-PSM Multi-Vendor-Class-Loader-Test** —
Class-Identity ist via Spec-API-Disziplin + K13-Wire belegt; ein automatisierter
Multi-Vendor-Class-Loader-Test braucht einen Multi-Vendor-Live-Rig
(dds-java-psm-1.0 §7.2.2.1, RTPS-Workstream).

**Blockiert (nicht actionable):** idl-4.2 `long double` (4 partials —
Type-Tag akzeptiert, Promotion/Float-Range/IEEE-double-extended Stub) —
wartet auf Rust-stable `f128` (~2027). Kein Workaround; bis dahin partial.

**Bei der Gelegenheit geschlossen** (Doc war stale, Code war fertig): CoAP-OSCORE
(dds-coap-bridge §7.2, war `n/a rejected` → done, ADR 0010); CoAP-DTLS (§7.1,
opt-in webrtc-dtls, ADR 0011); ROS-2 SROS2-Enclaves + Permissions-XML
(zerodds-ros2-bridge §7.1/§7.2, war `n/a rejected` → done, **neuer ADR 0012**
supersedet 0008); idl-cpp wstring/nested bound-enforcement (dds-xtypes, war
partial → done); zerodds-async take_stream (nativer Reader-Slot-Waker, war
partial → done).

## Wie ein neues Open-Item hinzugefügt wird

Wenn aus einer Sprint-Retro ein nicht-blocking-deferred Item rausfällt:

1. Pro Item ein neues `<topic>-followup.md` File im thematischen Verzeichnis (`docs/perf/`, `docs/interop/`, `docs/architecture/`)
2. Inhalt nach Template (siehe `docs/perf/d5e-phase3-deadline-heap-followup.md` als Vorlage)
3. Eintrag in dieser Datei (`docs/OPEN-ITEMS.md`) Tabelle hinzufügen
4. Commit-Message: `docs(open-items): add <topic>-followup.md`

## Kompletted-Removed

Wenn ein Item abgeschlossen wurde:
1. `*-followup.md` File **nicht löschen** — wird zu Geschichte / Sprint-Diary
2. Im File-Header `Status: completed` setzen + `Closed-Datum: YYYY-MM-DD` + `Closed-by-commit: <hash>`
3. Aus Tabelle in dieser Datei entfernen (oder in einen "Recently Closed" Abschnitt verschieben falls historisch interessant)

## Nicht hier dokumentiert

Diese Datei trackt **keine**:
* Aktiven In-Progress-Sprints — die leben in `.planning/`
* Bug-Reports — gitlab.sandra-kessler.eu Issues
* Open-Source-Tickets externer Vendoren
* Tutorial- und Onboarding-TODO — `examples/tutorials/dds-chat/ROADMAP.md`
* High-level Strategic Roadmap — `docs/architecture/06_roadmap.md`
