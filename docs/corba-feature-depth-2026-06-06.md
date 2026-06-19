# CORBA-Stack Feature-Tiefe — Kartierung 2026-06-06

Ehrlicher Audit der **gesamten** CORBA-Crate-Familie (~24 Crates, >100k LOC) auf
branch `corba`. Ziel: trennen, **was wir wirklich können** (live, über den Draht,
interop-bewiesen) von **wo nur Spec-Konformitäts-Modelle** liegen (in-memory,
nicht an die laufende GIOP/IIOP/DDS-Runtime gewired).

Methodik: 6 parallele Code-Audits + empirische Codegen-/Interop-Proben. Jede
Aussage `file:line`-belegt. Klassifikation:

- 🟢 **LIVE + interop-bewiesen** — über den Draht getestet gegen Fremd-ORBs.
- 🔵 **IMPL (ungetestet cross-ORB)** — vollständiger Wire-Code + Unit-Tests, aber
  kein Fremd-ORB-Nachweis.
- 🟡 **SPEC-MODELL** — korrektes In-Memory-Datenmodell / AST-Transformation, **keine**
  Live-Wire-Anbindung. Nützlich als Migrations-/Codegen-Tooling, **kein** Runtime-Dienst.
- 🔴 **STUB / LÜCKE / BUG** — unfertig, ungenutzt oder fehlerhaft.

---

## 1. Das Gesamtbild in einem Satz

Der **Wire-Kern** (GIOP/IIOP/IOR/POA-dispatch + IDL-Codegen) ist echt und
**bidirektional gegen omniORB/TAO/JacORB bewiesen**. Fast alles **darüber** —
die CCM-Container-Familie, COS-Event, D&C, EJB-Bridge, Interface-Repository und
die CORBA↔DDS-Bridge — ist **Spec-Konformitäts-Modell**: sauber, getestet, aber
in-memory und **nicht an die laufende Runtime gewired**. Es gibt keinen einzigen
Remote-Call, der je eine CCM-Component, einen EventChannel oder die DDS-Bridge
über den Draht erreicht.

---

## 2. Crate-Familie — Überblick

| Crate | LOC | Schicht | Tier |
|---|---|---|---|
| `corba-giop` | 2697 | GIOP-Wire-Codec | 🟢/🔵 |
| `corba-iiop` | 1763 | IIOP-TCP-Transport | 🟢/🔵 |
| `corba-ior` | 1707 | IOR + stringified + corbaloc/name | 🟢/🔵 |
| `corba-poa` | 1766 | Object Adapter | 🟢/🔵 |
| `corba-rust` | 1515 | IDL→Rust Service-Codegen | 🟢/🔴 |
| `idl-rust` | 2278 | IDL→Rust DataType-Codegen | 🟢/🔴 |
| `idl` | 36954 | IDL-4.2-Parser + AST | 🟢/🔴 |
| `corba-interop` | 1026 | Cross-ORB-Harness + Runtime-Glue | 🟢 |
| `corba-cosnaming` | 896 | NamingService | 🟡 |
| `corba-csiv2` | 908 | CSIv2-Security-Codecs | 🟡 |
| `corba-ir` | 996 | Interface Repository | 🟡 |
| `corba-cos-event` | 1200 | CosEventService | 🟡 |
| `corba-dds-bridge` | 2632 | CORBA↔DDS-Bridge | 🔵 (CORBA) / 🔴 (DDS) |
| `corba-ccm` | 6394 | CCM-Container + TimerEventService | 🟡 (+ Timer 🔵) |
| `ccm` | 2855 | CCM-4.0-Modell + Equiv-IDL | 🟡 |
| `corba-ccm-lib` | 838 | DDS/Persistence/Telemetry-Components | 🟡 |
| `corba-ccm-ejb` | 908 | CCM↔EJB-Bridge | 🟡 |
| `ami4ccm` | 2431 | AMI4CCM Async-Invocation | 🟡 |
| `corba-dnc` | 1468 | Deployment & Configuration | 🟡 |
| `corba-codegen` | 598 | Annex-A.1 Codegen-Helfer | 🟡 (1 Fn 🟢) |
| `corba-ccm` … `idl-cpp/java/cs/py/ts` | — | Sprach-PSM-Codegen (eigener Audit) | — |

---

## 3. Wire-Kern — was wir WIRKLICH können

### 3.1 GIOP / IIOP / IOR — 🟢 interop-bewiesen (GIOP 1.2)

Bidirektional grün gegen **omniORB 4.3.3 / TAO 2.5.24 / JacORB 3.9** (Little- +
Big-Endian), Feature-Matrix string/long/double/long long/sequence/out/inout —
siehe [`perf/corba-cross-orb-interop-2026-06-06.md`](perf/corba-cross-orb-interop-2026-06-06.md).

| Feature | Tier | Beleg |
|---|---|---|
| GIOP 1.2 Request/Reply | 🟢 | `corba-giop/src/codec.rs` |
| LocateRequest/LocateReply | 🟢 | `corba-interop/src/runtime.rs:255-275` |
| Reply NoException/User/System | 🟢 | `runtime.rs:301-323` |
| stringified-IOR Austausch | 🟢 | `corba-ior/src/stringified.rs` |
| TargetAddress KeyAddr | 🟢 | `target_address.rs` |
| IIOP-TCP + Connection-Pool | 🟢 | `corba-iiop/src/connector.rs` |
| GIOP 1.0/1.1 Layout-Switching | 🔵 | `reply.rs:123` — impl, nur 1.2 cross-getestet |
| TargetAddress Profile/ReferenceAddr | 🔵 | `target_address.rs:54-147` — opak, ungetestet |
| Reply LocationForward(Perm)/NeedsAddr | 🔵 | `reply.rs:43-58` — **nur decode, Server treibt es nie** |
| CancelRequest / CloseConnection | 🔵 | Codec da, **Server-Loop ignoriert sie** |
| Fragment-Message | 🔴 | `fragment.rs` — Single-Frame-Codec, **keine Reassembly** |
| GIOP 1.3 | 🔴 | `header.rs:126` — aktiv abgelehnt |
| Service-Context-Payloads (Codeset/CSI) | 🔴 | nur opak durchgereicht, kein typisierter Parser |
| IIOP-over-TLS / SSLIOP | 🔴 | `corba-iiop/README.md:35` — bewusst deferred |

### 3.2 POA — 🟢 dispatch bewiesen, 🔴 Hierarchie fehlt

| Feature | Tier | Beleg |
|---|---|---|
| 7 Policies + dispatch | 🟢 | `corba-poa/src/poa.rs:183-340` (via interop serve) |
| POAManager-State-Machine | 🔵 | `poa_manager.rs:30-112` |
| ServantActivator/Locator, Default-Servant | 🔵 | `servant_manager.rs` |
| **POA-Hierarchie** (create_POA/find_POA) | 🔴 | **fehlt** — flacher Single-Adapter |
| **AdapterActivator** | 🔴 | **fehlt** |

### 3.3 IDL-Codegen — 🟢 Kern-Matrix, 🔴 mehrere Wire-Bugs

🟢 **Voll generiert + interop-bewiesen:** interface (Stub-Marshalling +
Skeleton-dispatch), in/out/inout/oneway, attribute get/set, struct
(final-CDR), enum, union, sequence, array, string, exception-als-Datentyp.

🔴 **Echte Bugs / Lücken** (siehe §5):
- `char` schreibt 4 Bytes statt 1 (Wire-inkompatibel)
- `wstring` = `string` (UTF-8, nicht UTF-16)
- typed `raises(E)` nicht gewired (generisches `CorbaException`)
- `valuetype`/`component`/`home` nur Typ-Deklaration
- Object-Referenz-Typen strukturell fehlend

---

## 4. Add-ons — was Spec-Modell ist (nicht Live-Runtime)

> **Kernbefund:** Keine der Add-on-Crates macht echte CORBA-Wire-I/O. Es gibt
> keine POA-Bindung von CCM-Components, keinen GIOP-exponierten EventChannel,
> keinen remote erreichbaren NameService/IR. Alle Tests sind Unit-Tests gegen
> In-Memory-Modelle; **null** Cross-ORB-/Integration-Tests in dieser Schicht.

### 4.1 TimerEventService (corba-ccm) — 🔵 echter Scheduler, aber lokal

Der vom User hervorgehobene „Timer". **Echt funktionsfähig** als Rust-API:
Worker-Thread, One-Shot + Periodic, `BTreeMap<Handle,Entry>`-Scheduling, Expiry,
Callback-Trait, cancel/shutdown — `corba-ccm/src/timer.rs:50-175`, 4 Tests grün.
**Aber:** (a) **Poll-basiert** (`thread::sleep(20ms)`-Tick, `timer.rs:150`) statt
event-driven; (b) **nicht über CORBA/GIOP/POA exponiert** — eine in-process
Rust-API, kein CORBA-Timer-Servant mit IOR. Der Name „OMG Time / Timer-Service"
verspricht einen Remote-Dienst; geliefert ist ein lokaler Scheduler.

### 4.2 CCM-Container-Familie — 🟡 In-Memory-Modell

| Crate / Feature | Tier | Beleg |
|---|---|---|
| `corba-ccm` Container-Lifecycle | 🟡 | `container.rs:75-224` — echte State-Machine, aber `ComponentExecutor` ist ein **in-process Rust-Trait**, keine POA/GIOP-Bindung; Components empfangen **keine** Remote-Calls |
| `corba-ccm` CIF/CIDL/PSS/Port | 🟡 | `cidl.rs:16`, `port.rs:60` (IOR als opake Bytes) |
| `corba-ccm` orb_core/orb_extensions | 🔴 | Doc-Header: „Stub-Layer" (`orb_core.rs:4`); MIOP-Transform über abstrakte Sinks, nicht an Sockets |
| `ccm` Equivalent-IDL-Transform | 🟡 | `transform.rs` — reine AST→AST-Compiler-Pass |
| `ccm` dds4ccm | 🔴 | self-deklariert „Connector-**Stub**-Layer" (`dds4ccm.rs:4`) |
| `corba-ccm-lib` DdsBridgeComponent | 🟡 | `dds_bridge.rs:75` — **publiziert NICHT auf DDS** (keine dcps-Dep, `activate` setzt nur `bool`) |
| `corba-ccm-lib` Persistence/Telemetry | 🟡 | In-Memory `BTreeMap`/`Vec`, **kein Disk/Export-I/O** |
| `ami4ccm` Implied-IDL + ReplyHandler | 🟡 | `transform.rs` — AST-Transform; lib.rs: „n/a ohne CCM-Runtime" |

### 4.3 COS-Services + IR — 🟡 In-Memory, nicht remote

| Crate / Feature | Tier | Beleg |
|---|---|---|
| `corba-cos-event` EventChannel Push | 🟡 | `channel.rs:171-189` — echter In-Process-Dispatch, **0 Deps**, kein Fremd-ORB-Supplier kann pushen |
| `corba-cos-event` Pull-Modell | 🔴 | `channel.rs:258-268` — **Busy-Poll** (`yield_now`) |
| `corba-cosnaming` NameService | 🟡 | `context.rs:113-202` — voller In-Memory-NamingContext, **nicht über IIOP exponiert** |
| `corba-ir` TypeCode/Repository | 🟡 | `repository.rs:96-169` — Datenmodell; Doc-Claim „via IIOP/IOR" **ungedeckt** (0 Deps). Real genutzt nur `RepositoryId::parse` für POA `_is_a` |
| `corba-csiv2` GSSUP/SAS-Codecs | 🟡 | reine Wire-Codecs, **keine** Auth-Durchsetzung, kein SAS-Handshake, kein TLS |

### 4.4 CORBA↔DDS-Bridge — 🔵 CORBA-Hälfte echt, 🔴 DDS-Hälfte fehlt

**Der kritischste Befund.** Die Bridge bridged nicht durchgehend:

- 🔵 **CORBA-Seite LIVE:** Der Daemon `zerodds-corba-bridged` ist ein echter
  TCP/IIOP/GIOP-1.2-Server (Decode/Encode, SHA-256-Object-Key, IOR-Gen,
  Prometheus/healthz, optional rustls-SSLIOP). `bridge_e2e.rs:130-181` spawnt ihn
  real und bekommt eine echte GIOP-Reply.
- 🔴 **DDS-Seite FEHLT:** **Keine `zerodds-dcps`/`-rtps`-Dependency im Crate**
  (0 Treffer). Der Daemon antwortet hartkodiert `NoException` + leerer Body
  (`bin/...:513-524`), die DDS-Runtime wird nie gestartet (`// FUTURE (L2):
  DcpsRuntime::start`, `:300`), der Sample-Counter inkrementiert **ohne** je zu
  publizieren (Fake-Metrik, `:466`). `BridgeServant`→`DdsPublishSink` ist ein
  Trait mit nur einem `TestSink`-Mock und **nicht im Daemon gewired**.
- **Fazit:** produktionsreifer IIOP-Front-End-Daemon + vollständige
  Mapping-Datenschicht, aber der End-to-End-CORBA↔DDS-Datenfluss ist `FUTURE`/Mock.

### 4.5 EJB-Bridge + D&C — 🟡 Modell, nichts deployt

| Crate / Feature | Tier | Beleg |
|---|---|---|
| `corba-dnc` Plan-Modell + validate | 🟡 | `plan.rs:113` — In-Memory-Referenzintegrität |
| `corba-dnc` XML-Loader | 🔴 | `xml.rs:69` — **hand-gerollter Substring-Parser**, kein echter XML/`Deployment.xsd`-Reader |
| `corba-dnc` ExecutionManager | 🟡 | `execution.rs:69` — `start_launch` kopiert nur Namen in eine BTreeMap, **deployt nichts** |
| `corba-dnc` ContainerHost→corba-ccm | 🔵 | `container_host.rs:128` — ruft echte Container-Lifecycle, **aber vom Plan-Flow nie aufgerufen** (isoliert) |
| `corba-ccm-ejb` JTA/JNDI/Bean | 🟡 | `tx.rs`, `naming_glue.rs` — Enum-Mapping + In-Memory-Tabellen, **keine Java/JVM/JNI-Anbindung** |
| `corba-codegen` build_repository_id | 🟢 | `repository_id.rs:19` — **einzige real genutzte Fn** (corba-rust, 4 Call-Sites) |
| `corba-codegen` special_types/stub/skeleton | 🔴 | ungenutzte Template-Helfer (0 externe Caller) |

---

## 5. Echte Bugs (im „bewiesenen" Pfad, fixbar)

Diese fielen im Cross-ORB-Test nicht auf, weil die Echo/Bench-Matrix die
betroffenen Typen nicht nutzt:

1. ~~**`char` Wire-Bug**~~ — ✅ **GEFIXT 2026-06-06** (`commit 614f2d15`). IDL `char`
   → Rust `u8` (1 Byte), `wchar` → `u16` (2 Byte LE UTF-16), konsistent mit dem
   TypeIdentifier (Char8/Char16), union-switch, idl-cpp/-java/-csharp/-c und dem
   kanonischen `zerodds-xcdr2-rust-1.0`. **Cross-ORB wire-bewiesen** gegen
   omniORB/TAO/JacORB (`next_char`-Op, alle 6 Kombinationen grün).
2. ~~**`wstring` Wire-Bug**~~ — ✅ **GEFIXT**: `zerodds_cdr::WString` (distinkt von
   `string`), GIOP-1.2-UTF-16-Wire (Länge-in-Oktetts, kein Terminator), e2e
   ZeroDDS↔ZeroDDS + JacORB. Voll-omniORB/TAO-Interop braucht Codeset-Negotiation
   (BOM-Konvention) — eigenes Feature.
3. ~~**typed `raises(E)` nicht gewired**~~ — ✅ **GEFIXT**: Operationen mit raises
   geben `Result<T, {iface}Error>`; Stub decodet die UserException typisiert per
   repo_id, Skeleton encodet sie IOR-konform (kontinuierlich + Endianness).
   **Cross-ORB wire-bewiesen** (`checked raises RangeError`, 6/6).
4. ~~**Parser: `native` AST-Build bricht**~~ — ✅ **GEFIXT** (`commit ba252939`):
   `TypeDecl::Native` + Builder + Resolver + Sprach-Emitter + Regressionstest.
5. ~~**Parser: `context(...)`-Clause**~~ — ✅ **GEFIXT**: `OpDecl.context` +
   op_with_context-Branch in build_export + Print-Roundtrip + Regressionstest.
6. ~~**Object-Referenzen**~~ — ✅ **GEFIXT** (`commit 34eb2bae`): `Object` parst als
   reservierter Scoped-Name; `ObjectReference` marshallt als echte IOR (§15.3.3);
   **cross-ORB wire-bewiesen** (`echo_ref`-Op, 6/6, Rückgabe-Ref live aufrufbar).
7. ~~**`any` cdr_only-Dependency-Leak**~~ — ✅ **GEFIXT**: `zerodds_cdr::CorbaAny`
   (TypeCode+Value); `any` mappt pfadabhängig (DDS→DdsAny, CORBA→CorbaAny), kein
   dcps-Leak mehr; e2e ZeroDDS↔ZeroDDS.

**Alle 7 §5-Bugs gefixt.** Add-on-Schicht: CosNaming live über GIOP verdrahtet
(bind/resolve, aufgelöste Ref live aufrufbar, typed NameError) — demonstriert das
"Spec-Modell → Live-Runtime"-Pattern; übrige Add-ons (CCM→POA, DDS-Bridge-DDS-
Hälfte, IR-remote) sind je eigene WPs gleicher Bauart.

---

## 6. Priorisierte Lücken-Liste

**Für GIOP-Interop-Vollständigkeit (Wire-Kern härten):**
1. ~~`char` Wire-Encoding~~ ✅ gefixt + cross-ORB-bewiesen. `wstring` (UTF-16) offen.
2. ~~Parser `native`~~ ✅ + ~~`Object`-als-Typ~~ ✅ gefixt. `context`-Clause offen (selten).
3. ~~Object-Referenz-Typen~~ ✅ gefixt: IOR-Marshalling + cross-ORB `echo_ref` 6/6.
4. **Typed-Exception-Marshalling** wiren (raises → echtes Exception-Enum auf dem Draht) —
   nächster Wurzel-Kandidat (Codegen-Schicht).
5. Fragment-Reassembly (große Payloads von omniORB/TAO).
6. `any` cdr_only-dcps-Leak (kleiner Root).

**Für produktive CORBA-Deployments (Runtime-Schicht):**
5. POA-Hierarchie + AdapterActivator (blockt nicht-triviale Server).
6. IIOP-over-TLS / SSLIOP (Pflicht für Finanz-Migrationsziel; CSIv2 ohne TLS wertlos).
7. Reply-LocationForward serverseitig treiben (Load-Balancing/Failover).

**Add-ons von Spec-Modell zu Live-Runtime heben (je ein größeres WP):**
8. CCM-Container an POA/GIOP binden (Components empfangen Remote-Calls).
9. CORBA↔DDS-Bridge: DDS-Hälfte real wiren (dcps-Dep + publish/subscribe/correlate).
10. CosNaming + CosEvent + IR als echte CORBA-Remote-Objekte über IIOP exponieren.
11. D&C-ExecutionManager an ContainerHost anschließen (Plan deployt wirklich).

**Hygiene:**
12. Event-driven statt Busy-Poll im Timer + Cos-Event-Pull (Condvar/Waker).
13. Irreführende Doc-Claims korrigieren („production-ready CCM-Component",
    „via IIOP/IOR", „live DDS-Bridge") — siehe Memory-Notiz zu ehrlichem Wording.

---

## 7. Fazit für die Positionierung

**Substanz, ehrlich:** ein **interop-bewiesener GIOP-1.2-CORBA-Client+Server**
(schnellster ORB im Vergleich) mit IDL→Rust-Codegen für den Kern-Konstruktraum,
plus die **breiteste Spec-Konformitäts-Modell-Bibliothek** für CCM/AMI4CCM/D&C/
COS/IR/CSIv2, die als Migrations- und Codegen-Tooling taugt.

**Nicht behaupten:** „CCM-Container-Runtime", „live DDS-Bridge", „remote
NameService/IR", „voller CORBA-Timer-Service". Das sind heute In-Memory-Modelle
bzw. lokale APIs, keine über den Draht erreichbaren Dienste.

Der Abstand zwischen „bewiesen" und „vollständig" ist klar benannt und in §6
priorisiert abarbeitbar.
