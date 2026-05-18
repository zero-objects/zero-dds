# 0005 — Flatdata Dual-Stack: DCPS-Integration als opt-in Feature

- **Status:** accepted
- **Datum:** 2026-05-04
- **Autoren:** @sandra
- **Kontext:** crates/dcps, crates/flatdata
- **Supersedes:** D-9 in zerodds-flatdata-1.0.md (war: "FlatWriter strikt separat")

## Kontext

D-9 in der flatdata-Spec hat ursprünglich gesagt: FlatWriter und
DataWriter bleiben **strikt separate** Types. Begründung: keine
Layout-Constraints aus FlatStruct in den DCPS-Layer leaken lassen.

Bei der Phase-2-Decision-Runde wurde die Frage neu gestellt: ist
das aus Caller-Sicht ergonomisch?

- **Caller-Reality**: ein Robotics-Caller hat 80 % "normale" Topics
  (mit Strings, Vecs) und 20 % High-Frequency-Sensor-Topics (Pose,
  IMU). Er will EINEN Pub/Sub-Stack, nicht zwei.
- **Type-Inference**: `pub.create_datawriter::<Pose>(&topic, qos)` →
  und dann `writer.write_flat(&pose)?` ist intuitiver als
  `flat_pub.create_flat_writer::<Pose>(&topic).write(&pose)?`.
- **Migration**: bestehende Caller, die ihren CDR-DataWriter haben und
  einen Topic auf Zero-Copy migrieren wollen, müssen sonst den
  ganzen Pub-Sub-Code austauschen.

Risiken (siehe Decision-Runde Q3):
- API-Komplexität: Multi-Pattern-DataWriter
- Layout-Constraints leaken in DCPS
- Zwei Wire-Pfade in einer Klasse
- Reader-Side-Type-Inference-Pain

Diese Risiken sind real, aber mit **opt-in via Build-Flag**
mitigierbar: Caller, der nicht migrieren will, sieht die Methoden
gar nicht.

## Entscheidung

**Dual-Stack-Modus: `DataWriter::write_flat` und
`DataReader::read_flat` werden als `T: DdsType + FlatStruct`-bound
Methoden auf den DCPS-Klassen verfügbar — hinter Build-Flag
`--features flatdata-integration`.**

Default-Build (ohne Feature):
- DataWriter/Reader haben **nur** klassische CDR-Pfade (`write`,
  `take`).
- FlatWriter/Reader sind separate Types in `crates/flatdata`.
- Keine Cross-Coupling.

Mit `--features flatdata-integration`:
- DataWriter bekommt `write_flat`/`loan_flat_slot` als Methoden mit
  `where T: FlatStruct`-Bound.
- DataReader bekommt `read_flat`-Method.
- Pro DataWriter ein `Option<Arc<dyn SlotBackend>>`-Feld; bei `None`
  fallback auf CDR.
- Caller-Config bestimmt: SHM für Same-Host, UDP für Cross-Host
  (siehe ADR-0003 Backend-Trait).

derive(FlatStruct) Macro (`crates/dcps-derive`) wird **mit diesem
Feature aktiv** — Caller braucht es, um die `T: FlatStruct`-Bound
ergonomisch zu erfüllen.

## Alternativen

1. **D-9 beibehalten (FlatWriter strikt separat)** — Caller-
   Migration teurer; zwei Stacks. Verworfen.
2. **DCPS-Integration immer aktiv** — Layout-Constraints leaken;
   Caller-API komplex. Verworfen.
3. **Opt-in via Feature-Flag** (gewählt) — Caller wählt explizit.

## Konsequenzen

**Positiv**:
- Caller, der migrate will, behält den gleichen Pub/Sub-Code.
- Layout-Constraints bleiben klar gekapselt (Feature-Gate).
- derive(FlatStruct) hat klare Use-Case-Begründung.

**Negativ**:
- Test-Surface verdoppelt (Default + flatdata-integration).
- Caller-Doku braucht zwei Codepfade-Beispiele.
- DCPS-Compile-Time geht hoch wenn Feature aktiv (Macro-Expansion).

**Folge-Aufgaben**:
- F11: derive(FlatStruct) Macro.
- F-Dual: DataWriter::write_flat/read_flat hinter Feature.
- ADR-0003 SlotBackend-Trait muss auf DataWriter erweiterbar sein.
- CI: zweiter Job mit `--features flatdata-integration`.

## Referenzen

- `docs/specs/zerodds-flatdata-1.0.md` D-9 (superseded)
- ADR-0003 (Backend-Trait)
- ADR-0004 (Iceoryx2 optional)
