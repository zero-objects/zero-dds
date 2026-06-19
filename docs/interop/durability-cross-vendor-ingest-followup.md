# Durability-Service Cross-Vendor (Ingest + Replay) — Followup

**Status**: **ERLEDIGT — beide Richtungen Wire-bewiesen gegen echtes CycloneDDS**
(2026-06-11, codepit Domain 140). Zwei echte Interop-Bugs gefunden + gefixt:
- **Ingest**: realer CycloneDDS-`TRANSIENT_LOCAL`-Writer → Service (`matched=1`).
- **Replay (representation-treu)**: ein alignment-sensitiver FINAL-Typ
  `al::AlignS { octet tag; long long val; }` (XCDR1≠XCDR2-Layout) durch den
  Service zu einem CycloneDDS-**Late-Joiner** (Original-Pub bereits tot →
  Sample kam zwingend vom ZeroDDS-Replay): `SUB GOT tag=7 val=1122334455667788`
  — Wert byte-genau, also Encap **und** Discovery-Match korrekt.

**Was erledigt ist:**
- `DurabilityService::serve_typed(topic, type_name, keyed, contract)` registriert
  Ingest-Reader + Replay-Writer über das **Runtime-Level-User-Entity-API** mit
  einem **expliziten** `type_name` (event-driven `mpsc::Receiver<UserSample>`-Pump,
  kein Busy-Poll). Rein additiv — `serve()` (nativer RawBytes-Pfad) unverändert.
- `enable_auto_discovery` routet jetzt über `serve_typed` mit dem aus der
  `DCPSPublication` **entdeckten** `type_name` → matcht Fremd-Vendor-Writer.
- **Interop-Fix (data_representation)**: der Ingest-Reader bot anfangs nur XCDR2
  (rep 2). Ein CycloneDDS-Writer eines **FINAL**-Extensibility-Typs bietet aber
  nur XCDR1 (rep 0) → RxO-inkompatibel → **kein Match** (Cyclone-Discovery-Trace
  belegt: Writer `data_representation=1(0)`, Reader `1(2)`). Fix: der Ingest-Reader
  bietet **beide** (`data_representation_offer = Some(vec![0, 2])`) — ein
  Durability-Service muss jede Quell-Repräsentation ingestieren. Nach dem Fix:
  `matched=1`, Sample ingestiert.
- **Interop-Fix (Replay-Representation-Treue)**: der ingestierte `payload` ist
  der CDR-Body in der QUELL-Repräsentation (Cyclone-FINAL = XCDR1); der
  Replay-Writer deklarierte fix den Default-Encap (XCDR2) → ein strikter
  Fremd-Reader misparst einen alignment-sensitiven Typ (XCDR2 deckelt 8-Byte-
  Alignment auf 4). Fix: der Pump liest `UserSample::Alive.representation` und
  setzt vor dem Replay den Writer-Encap passend via neuem additivem dcps-API
  `DcpsRuntime::set_user_writer_data_rep_override` (offer-id 0/2). Topic ist
  repräsentations-konsistent → effektiv einmaliges Setzen.
- **Tests**: `tests/cross_vendor_ingest.rs` (in-process, Type-Name-Flexibilität,
  Late-Joiner-Replay) + `tests/external_cyclone.rs` (`#[ignore]`, gegen echten
  externen Writer, `ZD_HOLD_SECS` für die Replay-Validierung; Harnesse
  `tests/cyclone-harness/` (octet-Ingest) + `/root/durtest/align` auf codepit
  (alignment-sensitiver Replay)). Volle Suite grün.

## Wurzel

Der Daemon erzeugt seinen Ingest-Reader getypt über die High-Level-DCPS-API:

```rust
// crates/durability-service/src/lib.rs::serve()
let rtopic = self.ingest.create_topic::<RawBytes>(topic_name, ...)?;
let reader = subscriber.create_datareader::<RawBytes>(&rtopic, ...)?;
```

`create_topic::<RawBytes>` registriert den Type-Name **fix** als
`RawBytes::TYPE_NAME = "zerodds::RawBytes"` (`crates/dcps/src/dds_type.rs:384`).
Beim `use_xtypes=no`-Cross-Vendor-Matching wird über **(topic_name, type_name)**
gematcht — ein Cyclone-Writer mit z.B. `type_name = "SensorData"` matcht den
Reader mit `type_name = "zerodds::RawBytes"` **nicht** → kein Ingest.

`TopicInner.type_name` ist `&'static str` und kann daher **keinen** zur Laufzeit
entdeckten Fremd-Type-Name halten. Ein `create_topic`-Override scheidet damit
aus — der korrekte Pfad ist das **Runtime-Level-User-Entity-API** (genau das,
was die byte-orientierte C-FFI für Cross-Vendor schon nutzt).

## Fix-Pfad (exakt)

Der Daemon kommt über `DomainParticipant::runtime() -> Option<&Arc<DcpsRuntime>>`
(`crates/dcps/src/participant.rs:431`) an das Runtime-Handle. Für ein
**entdecktes Fremd-Topic** Reader+Writer über das Runtime-Level-API mit dem
**entdeckten Type-Name** + passendem **NoKey/WithKey**-Flag registrieren:

- **Ingest-Reader**:
  `ingest.runtime().register_user_reader_kind(UserReaderConfig { topic_name, type_name: <entdeckt>, durability: Volatile, reliable, .. }, is_keyed) -> (EntityId, mpsc::Receiver<UserSample>)`
  (`crates/dcps/src/runtime.rs:4099`). Der `Receiver` ist **event-driven**
  (kein Busy-Poll) — Pump = `rx.recv_timeout(200ms)` mit stop-flag-recheck,
  Match auf `UserSample::Alive { payload, .. }` (`runtime.rs:1796`),
  `Lifecycle` skippen.
- **Replay-Writer**:
  `replay.runtime().register_user_writer_kind(UserWriterConfig { topic_name, type_name: <entdeckt>, durability: TransientLocal, .. }, is_keyed) -> EntityId`
  (`runtime.rs:3847`), schreiben via
  `replay.runtime().write_user_sample_borrowed(eid, &payload)` (`runtime.rs:4926`).

Echo-Skip bleibt strukturell: `ingest`/`replay` sind zwei gegenseitig
ignorierende Participants (`ignore_participant` vor jeder Entity).

### Konkrete Schritte

1. ✅ **erledigt** — `serve_typed(topic, type_name, keyed, contract)` neben
   `serve()` mit Runtime-Level Reader/Writer + event-driven `rx.recv_timeout`-Pump.
   `serve()` (nativer RawBytes-Pfad) bleibt unverändert.
2. ✅ **erledigt** — `enable_auto_discovery` liest `type_name` aus der
   `DCPSPublication` und ruft `serve_typed` damit (Keyed-Hint: Increment-1 unkeyed).
3. **offen (codepit)** — **XCDR-Treue beim Replay**: `UserSample::Alive` trägt `xcdr_version` — beim
   Replay denselben Encapsulation-Header schreiben (XCDR1 vs XCDR2 nicht
   verwechseln), sonst lehnt ein strikter Fremd-Reader ab. Prüfen, ob
   `write_user_sample_borrowed` die Original-XCDR-Version erhält oder ob ein
   encap-erhaltender Schreibpfad nötig ist.

## Validierung (Pflicht für „done")

- **In-Process-Proof (lokal)**: ein Runtime-Level-Writer mit einem **Nicht**-
  `zerodds::RawBytes`-Type-Name (z.B. `"Foreign::Sensor"`) → Daemon ingest →
  Late-Joiner-Replay. Beweist die Type-Name-Flexibilität ohne Fremd-Binary.
- **Cross-Vendor (codepit)**: Cyclone/OpenDDS-`TRANSIENT`-Writer → Daemon →
  ZeroDDS-Late-Joiner-Reader. Läuft wie die übrigen Durability-e2e auf **codepit**
  (Prozess-Isolation + Multicast, vgl. `project_durability_service` Memory).
  Cyclone: `ddsperf`/eigener TRANSIENT-Pub; OpenDDS: `-DCPSConfigFile` +
  `DURABILITY TRANSIENT`. Type-Name + Keyed müssen zum Fremd-IDL passen.

## Referenzen

- `crates/durability-service/src/lib.rs` (`serve`, `pump`, `enable_auto_discovery`)
- `crates/dcps/src/runtime.rs` (`register_user_reader_kind` 4099,
  `register_user_writer_kind` 3847, `write_user_sample_borrowed` 4926,
  `UserSample` 1791, `UserReaderConfig` 1968)
- `crates/zerodds-c-api/src/lib.rs` (byte-orientierter Cross-Vendor-Pfad als Vorlage)
- `docs/interop/vendor-feature-matrix.md` (Transient/Persistent-Service-Zeile)
