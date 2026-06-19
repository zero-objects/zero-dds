# ROS-2 / XCDR1-Writer-Interop: Reader-Default-Representation

- **Status:** ✅ **GELÖST.** (1) ROS out-of-the-box via
  `RuntimeConfig::ros_defaults()` (C4) — Reader bietet `[XCDR1, XCDR2]`,
  matcht Cyclones XCDR1-Writer **ohne Env** (e2e codepit: 20/20). (2)
  Allgemeiner **per-Endpoint-Hebel** jetzt da:
  `DataWriterQos`/`DataReaderQos.data_representation: Option<Vec<i16>>`
  (B5) → ein einzelner Reader/Writer kann gezielt `[XCDR1, XCDR2]` (oder
  XCDR1-only) anbieten, ohne den globalen `DEFAULT_OFFER` (XCDR2-only,
  bewusst für FastDDS/OpenDDS) zu ändern. Test
  `data_representation_defaults_none_and_settable`.
- **Datum:** 2026-06-08
- **Kontext:** ROS-2-Live-Interop ZeroDDS ↔ CycloneDDS (= `rmw_cyclonedds`),
  `crates/ros2-rmw/interop/`

## Was ist offen

ZeroDDS-`DataReader` bieten per Default **XCDR2-only**
(`PID_DATA_REPRESENTATION = [XCDR2]`) an. ROS 2 / CycloneDDS schreiben
für `final/simple`-Typen (z.B. `std_msgs/String`) **XCDR1**. Cyclones
`data_representation_match_p` verlangt, dass die Reader-Liste die *erste*
Writer-ID enthält (`ddsi_qosmatch.c`); ein XCDR2-only-Reader matcht daher
**keinen XCDR1-Writer**. Folge: out-of-the-box (ohne Env) fließen keine
Daten.

**Funktionierender Mechanismus (heute):** `ZERODDS_DATA_REPR_OFFER=XCDR1,XCDR2`
lässt den Reader `[XCDR1, XCDR2]` annoncieren → Match + 20/20 Samples
bidirektional (siehe `crates/ros2-rmw/interop/GROUND_TRUTH.md`).
`run_interop.sh` setzt diesen Env als Default.

> Hinweis: Der **entityKind-Mismatch** (keyless vs keyed) war der *eigentliche*
> Match-Blocker und ist **gefixt** (commit „entityKind aus Type-Keyedness
> ableiten"). Dieses Item ist die *zweite*, davon unabhängige Lücke.

## Warum offen (bewusster Trade-off)

Der saubere Fix ist **nicht** ein globaler Default-Wechsel auf
`[XCDR1, XCDR2]` — das würde die bewusste XCDR2-only-Logik
(`crates/rtps/src/publication_data.rs:62-71`) für **echte XCDR2-Body-Typen**
(appendable/mutable mit DHEADER) brechen und FastDDS/OpenDDS-XCDR2-only-
Interop gefährden. Zwei korrekte Wege:

1. **`DataRepresentationQosPolicy` in `DataReaderQos`/`DataWriterQos`
   modellieren** (Spec §2.2.3, DDS-XTypes 1.3 §7.6.2) und ins bereits
   existierende per-Endpoint-Feld `UserReaderConfig.data_representation_offer`
   plumben. Der ROS-Pfad (`ros2-rmw` / Example) setzt dann explizit
   `[XCDR1, XCDR2]` — spec-konformer Hebel statt Env.
2. **Type-Extensibility-getriebene Ableitung** im create-reader-Pfad: für
   `EXTENSIBILITY = Final`-Typen mit XCDR1-kompatiblem Body `[XCDR2, XCDR1]`
   annoncieren, sonst XCDR2-only.

## Implikation / Territorium-Hinweis

Weg 2 berührt das **XCDR1-Decode-Alignment**: ein Reader, der XCDR1 *anbietet*,
muss XCDR1 auch korrekt **dekodieren** — für `Final`-Typen mit 64-Bit-Membern
unterscheidet sich XCDR1 (Align 8) von XCDR2 (Align ≤4, §7.4.1.1.1). Das ist
das **XCDR2-Alignment-Territorium** (siehe Memory `xcdr2_alignment_bug_validation`);
vor einer breiten XCDR1-Offer-Erweiterung muss der XCDR1-Decode-Pfad für
64-Bit-haltige Final-Typen verifiziert sein. Daher: **nicht** im Alleingang
breit umstellen — mit dem XTypes/XCDR2-Implementer abstimmen.

Für **ROS-Typen** (überwiegend simple/final ohne 64-Bit-Align-Falle) ist der
Env-Mechanismus heute funktional vollständig; der saubere QoS-Hebel ist die
Politur.

## Wann pick-up sinnvoll

- Sobald `DataRepresentationQosPolicy` ohnehin modelliert wird (per-Endpoint-
  Representation-Override ist mehrfach als „TBD" markiert:
  `publisher.rs` / `subscriber.rs`).
- Wenn der XCDR1-Decode-Pfad für 64-Bit-Final-Typen verifiziert ist.

## Implementations-Pfad (geschätzt)

1. `DataRepresentationQosPolicy { value: Vec<DataRepresentationId> }` im
   `zerodds_qos`-Crate + Feld in `DataReaderQos`/`DataWriterQos` (+ Default
   `[XCDR2]` → kein Verhaltens-Change) — ~0.5 PT.
2. Plumbing `qos.representation → UserReaderConfig/UserWriterConfig.
   data_representation_offer` in publisher.rs/subscriber.rs — ~0.5 PT.
3. ROS-Example + `ros2-rmw` setzen `[XCDR1, XCDR2]`; Env-Default aus
   `run_interop.sh` entfernen — ~0.25 PT.
4. XCDR1-Decode-Verifikation 64-Bit-Final (mit XCDR2-Implementer) — separat.
