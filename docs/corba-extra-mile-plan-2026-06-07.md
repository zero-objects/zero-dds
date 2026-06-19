# CORBA „Extra-Meile"-Masterplan — alle 9 Lücken (2026-06-07)

Ziel: jede der 9 identifizierten Lücken gegenüber omniORB/TAO/JacORB **sauber
analysiert, spec-treu umgesetzt, tief unit-getestet und cross-ORB-e2e-belegt**
schließen. Kein MVP, kein Phasen-Deferral als done-Workaround. Wo OMG-Spec-Text
nötig ist, wird er beschafft und in den `docs/specs/`-Prozess gehoben; wo es um
ZeroDDS-Vendor-Erweiterungen geht (Nicht-OMG-Transporte), wird eine formale
Vendor-Spec autoren-seitig erstellt (Muster `docs/adr/0001-vendor-spec-strategie.md`).

Grundlage: 4 code-belegte Tiefen-Analysen (2026-06-07). Jede Lücke unten mit
IST (Datei:Zeile), Spec-Bezug, Umsetzungs-Skizze, Test-Strategie, Aufwand.

Tracking: pro Punkt ein `docs/interop/corba-<topic>-followup.md` + Index in
`docs/OPEN-ITEMS.md`. Cross-ORB-Beleg läuft über
`crates/corba-interop/competitors/` (codepit).

---

## Spec-Beschaffung (Wave 0, vor jeder Umsetzung)

| Punkt | OMG-Spec / Quelle | Aktion |
|---|---|---|
| Valuetype-Wire | CORBA 3.4 Part 1 §5 (Semantik), §15.3.4 (Value-Wire), §7.6 (Repo-IDs) | PDF beschaffen → Konformitäts-Items nach `docs/specs/omg-corba-valuetype-conformance.md` extrahieren |
| CSIv2 | CORBA 3.4 Part 2 §10 (Secure Interoperability), §10.6 (GSSUP), InitialContextToken (RFC 2743 §3.1) | PDF §10 → `docs/specs/omg-csiv2-conformance.md` |
| TypeCode-Indirection | CORBA 3.4 Part 1 §15.3.5 (TypeCode-Wire), §15.3.4.3 (Indirection-Mechanik) | aus o.g. PDF mit abdecken |
| DII/DSI/DynAny | CORBA 3.4 Part 1 §7 (DII), §8 (DSI), §9 (DynAny) | PDF §7-9 |
| AMI | CORBA 3.4 Part 2 „CORBA Messaging" (Callback + Polling), IDL 4.2 §8.3.6.3 `@ami` | PDF Messaging-Kapitel |
| GIOP-Versionen | CORBA 3.4 Part 2 §15.4.1 (Version-Regel), §15.4.2 (Layouts), §15.4.9 (Fragments) | PDF §15 |
| Bidir-GIOP | CORBA 3.4 Part 2 §15.8 (Bidirectional GIOP) | PDF §15.8 |
| UIOP / SHMIOP | **Nicht-OMG** (vendor) | ZeroDDS-Vendor-Spec `docs/specs/zerodds-uiop-transport-1.0.md` autoren |
| OTS / Trading / RT-CORBA | OMG Transaction Service, Trading Object Service, Real-Time CORBA (eigene Specs) | erst Scope-Entscheid (s. Punkt 9), dann PDF je Service |

---

## Wave-Plan (Reihenfolge nach ROI + Abhängigkeit)

```
Wave 1  Foundation + Quick-Wins   #3 SSLIOP-Pool · #5 UIOP · #8 GIOP-Versionen · DII-live
Wave 2  Security-Interop          #2 CSIv2 cross-ORB           (braucht Wave-1-SSLIOP-Pool)
Wave 3  Recursive/Shared-Wire     #4 TypeCode-Indirection → #1 Valuetype-Wire (geteilte Infra)
Wave 4  Advanced-Invocation       #6 Client-AMI · #7 Bidir-GIOP
Wave 5  Service-Spec-Scope        #9 OTS / Trading / RT-CORBA (Go/No-Go je Service)
```

Begründung der Kopplungen:
* **#3+#5** teilen sich denselben `Connector`-Refit (`enum PoolKey { Tcp(addr) | Tls{addr,sni,cfg} | Uds(path) }`).
* **#2** koppelt GSSUP an TLS (alle 3 Fremd-ORBs verlangen das) → braucht stabiles, gepooltes SSLIOP aus Wave 1.
* **#4+#1** teilen die Positions-Tracking-/Indirection-Infrastruktur (`0xffffffff` + neg. Offset, Zyklen-Schutz); `tk_value` muss zuerst ins TypeCode-Enum (#4), bevor rekursive Valuetypes (#1) wire-fähig sind.
* **#7** ist am invasivsten (Connection-Sharing) → zuletzt.

---

## Punkt 1 — Valuetype-Wire (§15.3.4)  ·  Aufwand: GROSS  ·  Wave 3

**IST**: Parser vollständig (`idl/src/ast/types.rs:171-231`, Grammar `idl42.rs:2604-2701`,
inkl. truncatable/custom/abstract/value-box/factory). Codegen emittiert nur
**Typ-Skelette ohne Marshalling**: `corba-rust/src/valuetype_emit.rs:13-14,90-95`
(`init`-Fn gibt hartcodiert „not yet wired (Phase-2)"-Exception). Wire-Primitive
existieren **isoliert + ungenutzt** in `corba-rust/src/runtime.rs:297-527`
(value-tags `0x7FFFFF02/06/0A`, chunk-Ansatz) — werden NUR von
`corba-rust/tests/wire_giop.rs` aufgerufen, **nie** vom Codegen oder GIOP-Pfad.

**Fehlt** (interop-kritisch): Value-Sharing/Indirection (`0xffffffff`+neg. Offset),
codebase-URL, vollständige Tag-Flag-Bitmaske (statt 4 Konstanten),
Chunk-Length-Backpatching + nesting-levels, Truncation-Skip, Custom-Marshalling-Hook,
ValueFactory-Registry, GIOP-Wireup, Repo-ID-Scoping-Fix (`build_repository_id(&[],…)`
verliert Modul-Präfix → bricht Cross-ORB-Matching).

**Umsetzung**:
1. `corba-rust/src/value_wire.rs` (NEU): `ValueOutputStream` (Identity-Map ptr→pos,
   chunk-size-Backpatch, nesting-Stack, Indirection-Emit) + `ValueInputStream`
   (pos→Value-Map, Indirection-Auflösung, Truncation-Skip, volle Flag-Bitmaske) +
   Trait `ValueMarshal` + `CustomMarshal`.
2. Codegen `valuetype_emit.rs`: `impl ValueMarshal for V` mit State-Member-Encoding
   in Deklarationsreihenfolge; ValueFactory-Registry wired; Repo-ID-Scoping-Fix;
   abstract/value-box-Sonderfälle.
3. GIOP-Integration: Request/Reply-Body-Encoder nutzen ValueStream bei
   valuetype-Parametern.
4. Andere Sprachen (`idl-cpp/java/csharp`): Marshalling/ValueBase nachgelagert,
   sobald Rust-Referenz steht.

**Tests**: Unit — single/nested/abstract/value-box/null, **Sharing** (2 Refs→1 Indirection),
**Zyklus** A→B→A, **Truncation** (Named:Point als Point lesen), **Chunking**
(forced-chunked + nested + neg. end-tags), Custom-Marshal-Roundtrip, byte-exakte
§15.3.4-Fixtures BE+LE. E2E — neuer `competitors`-Harness, IDL
`valuetype Point{double x,y}; valuetype Segment{Point start,end}; valuetype Named:truncatable Point{string label}; valuetype Label string;`
+ `custom`; `Segment echo(in Segment)` gegen omniORB/TAO/**JacORB** (strikteste
Chunking/Truncation-Validierung); Spezialfälle Sharing/Zyklus/Truncation/null.

**Abnahme**: Sharing+Truncation+Custom cross-ORB grün gegen ≥2 Fremd-ORBs; byte-exakte Fixtures.

---

## Punkt 2 — CSIv2 cross-ORB (GSSUP)  ·  Aufwand: MITTEL-GROSS  ·  Wave 2

**IST**: Datenmodell + IOR-Component vollständig: GSSUP-Token (`corba-csiv2/src/gssup.rs:33-120`),
IdentityToken (`sas.rs:32-57`), SAS-Structs (`sas.rs:60-130`), `TAG_CSI_SEC_MECH_LIST`
+ CompoundSecMechList CDR (`mech_list.rs:32-260`, IOR-Roundtrip getestet). 17 Unit-Tests.
**ABER**: kein `SASContextBody`-CDR-Codec (der ServiceContext-15-Body fehlt komplett),
**nicht** in den Live-GIOP-Pfad verdrahtet (alle `ServiceContextList::default()` —
`corba-iiop/src/connection.rs:364,378,465,500`, `acceptor.rs:239-242,300-303`),
Bridge-Hook `corba-dds-bridge/src/csiv2_wire.rs:36-66` ist **nicht** spec-konform
(rohes GSSUP-Encap statt SASContextBody-Union) **und tot** (nie aufgerufen). Kein cross-ORB-Test.

**Umsetzung**:
1. `corba-csiv2/src/sas_wire.rs` (NEU): SASContextBody-Union-CDR (Diskriminator
   MTEstablishContext=0/Complete=1/Error=4/MessageInContext=2) + die 4 Member-Structs;
   GSSUP-`client_authentication_token` korrekt (ggf. 0x60-GSS-InitialContextToken-Wrap).
2. Client-Inject: `corba-iiop/connection.rs` + `corba-interop/lib.rs` — ServiceContext
   id=15 anhängen, wenn Ziel-IOR `CsiSecMechList` mit `EstablishTrustInClient` in
   `AsContextSec.target_requires` führt.
3. Server-Eval: `corba-iiop/acceptor.rs` — `req.service_context[15]` lesen,
   SASContextBody decoden, GSSUP user/pass prüfen → `CompleteEstablishContext`
   oder `ContextError`/`NO_PERMISSION`-Reply.
4. Bridge-Hook auf echten SASContextBody umstellen + tatsächlich aufrufen.

**Fremd-ORB-Konfig** (GSSUP an TLS gekoppelt — daher auf Wave-1-SSLIOP aufsetzen):
omniORB CSIv2-Policies + SSL-Transport; TAO `TAO_Security`/`TAO_CSI` + `-ORBSvcConf`;
JacORB `jacorb.security.csiv2.*` + SAS-Initializer. (Vor Test gegen Doku/Capture verifizieren.)

**Tests**: Unit — SASContextBody-Roundtrip BE/LE + Capture-Vektor gegen echten
omniORB/TAO-Wire-Dump (byte-genau). E2E — `competitors/{omniorb,tao,jacorb}/csiv2_*`
über SSLIOP: GSSUP-Handshake (CompleteEstablishContext) + 1 authentisierter Echo,
beide Richtungen; Negativtest falsches Passwort → ContextError/NO_PERMISSION.

**Abnahme**: GSSUP-Handshake + auth. Call cross-ORB grün ≥1 Fremd-ORB; Negativtest rot-korrekt.

---

## Punkt 3 — SSLIOP-Connection-Pooling  ·  Aufwand: KLEIN  ·  Wave 1

**IST**: `corba-interop/src/runtime.rs:276-317` `send_tls` baut **pro Call** frische
`connect_tls`-Connection (TCP-Connect + rustls-Handshake je Request) — Doc bestätigt es
(`runtime.rs:273-275`). Plain-TCP poolt dagegen voll (`Connector`, `connector.rs:121-167`,
Key=`SocketAddr`). rustls-`StreamOwned` ist `Send` und als `Connection` poolbar **ohne**
Session-Klonen (Pool *bewegt* die Connection).

**Umsetzung**: `Connector` um `enum PoolKey { Tcp(SocketAddr) | Tls{addr,sni,cfg_id} }`
(cfg_id=`Arc::as_ptr(ClientConfig)`); neue `Connector::connect_tls(host,ssl_port,sni,cfg)
-> PooledConnection`; `send_tls` ruft sie statt direkt `tls::connect_tls`. **Bonus**:
Liveness-Peek beim Reuse (fehlt aktuell auch dem TCP-Pool) → `invalidate()` bei totem Stream.

**Tests**: Unit — `connect_tls`-Reuse (Drop→idle==1, 2. Call→0), getrennte SNI/cfg→kein
Reuse, invalidate nach Break. E2E — N sequentielle `invoke()` über serve_tls = nur **ein**
Handshake (Accept-Count im Server). Cross-ORB — omniORB-SSLIOP-Client mehrere Calls auf einer
Connection bleibt grün. **Perf**: `ssliop_bench` muss danach den Stub-Pfad (mit Pool)
statt invoke_on messen → Steady-State auch im Codegen-Pfad.

**Abnahme**: 1 Handshake für N Calls belegt; SSLIOP-Bench über Stub-Pfad ~= invoke_on-Steady-State.

---

## Punkt 4 — TypeCode-Indirection beim Decode  ·  Aufwand: MITTEL  ·  Wave 3 (vor #1)

**IST**: `cdr/src/type_code.rs:355-358` lehnt Indirection (`0xffffffff`) ab. Architektur-Hürde:
`decode_encap` (`:393-421`) wechselt auf einen **kopierten Sub-Buffer** (`:418`) → absolute
Stream-Positionen verloren, aber der neg. Offset ist relativ zur Position im **Gesamt**-Stream.
Modell-Lücke: `tk_value`/`tk_value_box`/`tk_union`/`tk_array`/`tk_abstract_interface` fehlen im
`TypeCode`-Enum (`:63-143`) — der kanonische Indirection-Auslöser (rekursive Valuetypes)
existiert nicht als Typ.

**Umsetzung**: 1. Enum um `tk_value`/`tk_value_box`/`tk_union`/`tk_array` erweitern.
2. `TcDecoder`-Kontext mit Origin-Offset + `HashMap<abs_offset, Rc<TypeCode>>` statt Sub-Buffer-Kopie;
   Indirection: `target=current_pos+offset`, Cache-Lookup, sonst rekursiv an Ziel decoden.
3. Zyklen-Schutz: Platzhalter (`TypeCode::Recursive(repo_id)` / `Rc<RefCell<Option>>`) **vor**
   Member-Walk registrieren. 4. Encode optional (Decode-Akzeptanz reicht für Interop).

**Tests**: Unit — hand-gebautes Indirection-Frame (2 identische structs, 2. als Offset)→gleich;
rekursiver Valuetype terminiert; Negativ (positiver/nicht-4-aligned Offset)→Fehler. Capture —
`any` mit rekursivem Valuetype von JacORB/TAO/omniORB byte-genau decoden.

**Abnahme**: rekursiver + geteilter TypeCode aus ≥1 Fremd-ORB-Capture korrekt dekodiert.

---

## Punkt 5 — UIOP (Unix-Domain-Socket-Transport)  ·  Aufwand: MITTEL  ·  Wave 1

**IST**: nicht vorhanden; `Transport`-Enum nur `Tcp`/`Tls` (`corba-iiop/connection.rs:31-43`).
`framing.rs` ist bereits `Read`/`Write`-generisch (`?Sized`) → transport-agnostisch. Einzige harte
Kopplung: `Transport::sock()->&TcpStream` (`connection.rs:63-69`, für Timeouts/shutdown).
`transport-uds/` ist DDS-SOCK_DGRAM, **nicht** CORBA-wiederverwendbar. UIOP ist **Nicht-OMG**
(vendor) → ZeroDDS-Vendor-Spec.

**Umsetzung**: 1. `Transport::Uds{reader,writer}` (cfg unix) + `from_unix_stream` (UnixStream::try_clone).
2. `sock()`→kleine `socket_ops`-Abstraktion (set_*_timeout/shutdown; `set_nodelay` für UDS no-op).
3. `Connector::connect_uds(path)` über den `PoolKey::Uds(PathBuf)` aus #3. 4. `Acceptor::start_uds`
   (UnixListener, 1:1-Kopie von `start`). 5. IOR: ZeroDDS-`TAG_ZERODDS_UDS_TRANS`-Component (selbe
   Encapsulation-Mechanik wie `ssl_component`) trägt den Socket-Pfad; Cross-ORB-Unix mit omniORB
   (`giop:unix`) als **separates, nachgelagertes** Ziel (Reverse-Engineering omniORB-Unix-IOR).

**Spec**: Vendor-Spec `docs/specs/zerodds-uiop-transport-1.0.md` (konsistent mit
`zerodds-uds-transport-1.0` DDS-Seite + ADR-0001).

**Tests**: Unit — `from_unix_stream`-Roundtrip über `UnixStream::pair()`, Timeout/shutdown auf UDS
(Linux-gated, auf codepit verifizieren — macOS-unsichtbar!). E2E — `start_uds`+`connect_uds` GIOP
über tempdir-Socket; **Perf**: UDS vs loopback-TCP-p50 (erwartet schneller, eigene Bench-Zeile).
Cross-ORB optional (omniORB-Unix, hoher Aufwand).

**Abnahme**: ZeroDDS↔ZeroDDS UIOP grün + Perf-Vorteil vs loopback-TCP belegt; Vendor-Spec publiziert.

---

## Punkt 6 — Client-AMI (Asynchronous Method Invocation)  ·  Aufwand: MITTEL-GROSS  ·  Wave 4

**IST**: existiert NICHT. `@ami`-Annotation **wird geparst** (`idl/src/semantics/annotations.rs:104,413,472-475`)
aber vom Codegen **nicht konsumiert** (0 Treffer für Ami/is_ami in corba-rust/codegen). Kein
`sendc_`/`ReplyHandler`/`Poller`/`get_response`. Basis vorhanden: `CorbaConnection::invoke`,
`next_request_id: AtomicU32`, oneway-Pfad.

**Umsetzung**: 1. Codegen (corba-rust): pro AMI-Op `sendc_<op>(handler,args)` + `<Iface>ReplyHandler`-
   Skeleton (Repo-IDs `IDL:…/AMI_<Iface>Handler:1.0`). 2. Transport: `CorbaConnection::invoke_async
   -> request_id` + **event-driven** Hintergrund-Reader (kein Busy-Poll — Memory-Vorgabe), der
   `request_id→Handler`-Map auflöst, Exception→`<op>_excep`. 3. Optional Polling-Model
   (`sendp_<op>`+`<op>Poller`).

**Spec**: CORBA Messaging (Callback+Polling), IDL 4.2 §8.3.6.3.

**Tests**: Unit — `sendc_`-Frame korrekt, request_id-Korrelation bei 2 outstanding + out-of-order
Replies. E2E — ZeroDDS-AMI-Client gegen ZeroDDS-Server **und gegen TAO** (TAO hat AMI nativ):
Reply-Callback + Exception-Callback.

**Abnahme**: AMI-Callback + excep cross-ORB gegen TAO grün; out-of-order-Korrelation getestet.

---

## Punkt 7 — Bidirectional GIOP (§15.8)  ·  Aufwand: GROSS (invasivst)  ·  Wave 4

**IST**: nur Codec (`corba-iiop/src/bidir.rs`: `IIOP_BI_DIR_TAG=5`, ListenPoint/ServiceContext
+ encode/decode), **null Wiring** — Server liest `service_context` nie aus, Client injiziert nur
CodeSet. Kein BiDirPolicy, kein Reverse-Connection-Handling.

**Umsetzung**: 1. Client annonciert eigene Listen-Points als Tag-5-ServiceContext in der 1. Request
   (Gate via BiDirPolicy). 2. Server scannt `req.service_context[5]`, trägt Connection in bidi-Registry.
   3. **Kern-Refit**: Connection-Sharing (TCP via `try_clone`-Split; TLS zwingend `Arc<Mutex<Connection>>`)
   + Reply-Demux über request_id (beide Seiten senden jetzt Requests). 4. Even/Odd-request_id-Räume
   (§15.8: Originator gerade, Bidir-Partner ungerade). 5. BiDirPolicy (Security — sonst Hijack).

**Spec**: CORBA 3.4 Part 2 §15.8.

**Tests**: Unit — Tag-5-Context-Inject/Decode aus `req.service_context`. E2E — voller Reverse-Call
(Server→Client-Objekt über dieselbe Connection), gleichzeitige In-Flight beider Seiten, Even/Odd-IDs;
Negativ ohne Policy→abgelehnt. Cross-ORB — JacORB/omniORB als BiDir-Partner (härtester Konformitätsbeleg).

**Abnahme**: Reverse-Call ZeroDDS↔ZeroDDS + ≥1 Fremd-ORB grün; ID-Raum-Trennung + Policy-Negativtest.

---

## Punkt 8 — GIOP 1.0/1.1-Versions-Honorierung  ·  Aufwand: KLEIN  ·  Wave 1

**IST**: Codec ist **voll 1.0/1.1/1.2-fähig** (`corba-giop`: `version.rs` Prädikate,
`request.rs:137-241` + `reply.rs:110-164` branchen korrekt auf Layout, `header.rs` gated Fragments).
**ABER Runtime hardcodet 1.2**: Client schreibt immer `V1_2` (`corba-interop/runtime.rs:301,362`),
Server-Reply ebenso (`:505,528`) — Request-Version wird verworfen, nur endianness gespiegelt.

**Umsetzung**: 1. Server: Version aus Header mitführen, in `build_reply`/`write_message` die
   **Request-Version** (gekappt auf max) zurückgeben (§15.4.1 — nie höher). 2. Client: Request-Version
   aus IIOP-Profil-Version des Ziel-IOR ableiten statt Konstante. 3. Fragment-Reassembly im
   Dispatch-Loop für `Message::Fragment` (version-gegated).

**Spec**: CORBA 3.4 Part 2 §15.4.1/.2/.9.

**Tests**: Unit — Server-Reply-Version==Request-Version (1.0-Req→1.0-Reply). E2E — omniORB/TAO mit
erzwungener GIOP 1.0/1.1 gegen `interop_server` + LocateRequest-Probe; Fragment-Interop große Payload.

**Abnahme**: 1.0- und 1.1-Fremd-Client gegen ZeroDDS-Server grün (Reply in Req-Version).

---

## Punkt 9 — Service-Specs: OTS / Trading / RT-CORBA  ·  Aufwand: je GROSS  ·  Wave 5 (Go/No-Go)

**IST**: keine Crates. Bewusste Scope-Entscheidung statt Reflex-Umsetzung.

**Analyse + Empfehlung**:
* **CosTransactions (OTS, Object Transaction Service)** — relevant für Finanz-Migration (2-Phase-Commit,
  Transaction-Context-Propagation über GIOP-ServiceContext id=0 `TransactionService`). **Empfehlung: GO**
  als eigene Crate `corba-cos-transactions`, Muster wie CosNotification. Hebt den „Drop-in für
  Finanz-Bestand"-Anspruch. Spec: OMG Transaction Service 1.4.
* **CosTrading (Trading Object Service)** — Service-Discovery per Property-Constraints. Nischiger,
  aber abgeschlossene OMG-Spec → passt zur „optionale Profile als Differenzierung"-Strategie.
  **Empfehlung: GO (nachgelagert)** als `corba-cos-trading`. Spec: OMG Trading Object Service.
* **Real-Time CORBA** — TAOs Flaggschiff (Priority-Propagation, Thread-Pools mit Lanes,
  Priority-Banded-Connections). **Empfehlung: DEFER/NO-GO** — größter Aufwand, und ZeroDDS deckt
  Echtzeit-QoS bereits über die **DDS**-Seite ab (Deadline/Latency-Budget/Transport-Priority). Als
  Differenzierer fragwürdig, wenn DDS-RT vorhanden. Re-evaluieren, falls konkreter CORBA-RT-Migrationsfall.

**Umsetzung OTS** (wenn GO): Crate `corba-cos-transactions` — Current/Coordinator/Terminator/Resource/
Control/RecoveryCoordinator-Interfaces (PIDL→Codegen), TransactionService-ServiceContext (id=0,
`CosTransactions::PropagationContext`), 2PC-Protokoll-State-Machine. **Tests**: Unit (PropagationContext-CDR,
2PC-State-Transitions) + e2e (verteilte Transaktion über 2 ZeroDDS-Server; cross-ORB gegen TAO-OTS wenn verfügbar).

**Abnahme je Service**: volle Spec-Coverage-Matrix (Muster der bestehenden K1-K15-Audits) + e2e.

---

## Querschnitt: Test- & Qualitäts-Disziplin (für alle Punkte)

* **Tiefe Unit-Tests** mit byte-exakten Wire-Fixtures (BE+LE) gegen handvalidierte Spec-Bytes; 99%-Branch-Coverage-Messlatte (cargo-llvm-cov nachgelagert).
* **Cross-ORB-E2E** im `competitors/`-Harness, beide Richtungen, je ≥2 Fremd-ORBs wo möglich; Negativtests verpflichtend.
* **Capture-getriebene Validierung** für Wire-kritische Punkte (Valuetype-Chunking, CSIv2-SAS, TypeCode-Indirection): echter Fremd-ORB-Dump als Fixture.
* **Perf-Regression**: jeder transport-/wire-nahe Punkt updatet `docs/perf/corba-perf-baseline-*.md` (SSLIOP-Pool, UIOP, Valuetype-Op-Latenz).
* **Linux-gated** Tests (UIOP, evtl. CSIv2-SSL) auf codepit verifizieren (macOS-unsichtbar).
* Kein `cargo fmt --all` bei Path-Deps; pro-Crate; nur eigene Files committen (Parallelagent-Disziplin).

## Reihenfolge-Empfehlung für den Start

**Wave 1 zuerst, Punkt #3 (SSLIOP-Pool) als erster Schritt** — kleinster, in sich
geschlossen, schließt eine ehrliche Schwäche (Handshake-pro-Call), schafft den
`PoolKey`-Refit, den #5 (UIOP) und #2 (CSIv2) mitnutzen, und ist sofort
perf-/e2e-belegbar.
