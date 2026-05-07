# DCPS Interop

Dieser Crate liefert die Public-API fuer DDS-Applikationen und wird
gegen andere DDS-Stacks (Cyclone DDS, eProsima Fast-DDS) auf
Wire-Level getestet.

## Interop-Stufen

| Stufe | Scope | Status |
|-------|-------|--------|
| **Wire-Compliance** | Byte-Identische DATA/HEARTBEAT/ACKNACK-Submessages gegenueber Cyclone-Capture. | ✅ (Cyclone-Replay-Tests) |
| **SPDP-Discovery**  | ZeroDDS discovered Cyclone-/Fast-DDS-Participants ueber Multicast. | ✅ (`tests/interop/matrix.sh`) |
| **SEDP-Discovery**  | ZeroDDS sieht Cyclone-Publications im SEDP-Cache. | ✅ (`cyclone_live_sedp.rs`) |
| **Cross-Process DCPS** | Zwei ZeroDDS-Prozesse tauschen User-Samples ueber die Public-API. | ✅ (`hello_dds_e2e.sh`) |
| **Cross-Vendor DCPS** | ZeroDDS-Publisher ↔ Cyclone-Subscriber (und umgekehrt) auf Applikations-Topic. | ✅ (`tests/interop/xv_pub_sub_roundtrip.sh`) |
| **Cross-Arch DCPS** | macOS-aarch64 ↔ Linux-x86_64, self + Cyclone + Fast-DDS. | ✅ (`tests/interop/matrix.sh` + Wire-Compliance) |

## Cross-Process Smoke

```bash
# Linux-Only (Multicast-Loopback)
./tests/interop/hello_dds_e2e.sh
```

Startet `hello_dds_subscriber` + `hello_dds_publisher` parallel auf
Domain 0, laesst sie 10 s reden, prueft dass ≥ 3 `hello #N`-Samples
im Subscriber-Log stehen. Deckt SPDP + SEDP + Reliable-Data + Topic-
basiertes Matching durch die Public-API ab.

Plus: DCPS-Shape-Tests ueber die Public-API:

```bash
cargo test --test e2e_dcps_api -p zerodds-dcps
```

Diese Tests pruefen Topic-Registry + Entity-Hierarchie via
`create_participant_offline`, ohne UDP-Bind. Der **In-Process
E2E-Datenfluss-Test** ueber die Public-API kommt zurueck, sobald
`DataWriter::wait_for_acknowledgments()` +
`DataReader::wait_for_historical_data()` einfuehrt; ohne diese
Synchronisationsprimitives ist SPDP+SEDP-Discovery im selben
Prozess timing-flaky auf shared CI-Runnern.

Der E2E-Datenfluss ist zwischenzeitlich durch den Runtime-Unit-Test
`two_runtimes_e2e_user_data_match_and_transfer` und das Cross-Process-
Script abgedeckt.

## Cross-Vendor (Cyclone)

Cross-Vendor-Live-Roundtrip ist als CI-Test automatisiert
(`tests/interop/xv_pub_sub_roundtrip.sh`). Manuelles Setup fuer
ad-hoc Cyclone-Interop-Tests:

### ZeroDDS-Publisher ↔ Cyclone-Subscriber

Der Default-Topic von `ddsperf` ist nicht `Chatter`. Man braucht:

1. ZeroDDS-Seite: `hello_dds_publisher` startet Topic `"Chatter"` mit
   Type-Name `"zerodds::RawBytes"`.
2. Cyclone-Seite: Ein C-Subscriber, der denselben Topic-Namen + Type-
   Namen registriert. Skelett:

   ```c
   #include "dds/dds.h"
   // Topic-Type muss gleich heissen; Content ist opaque-Byte-Blob
   // (kein IDL, wir sind hier XCDR2-Plain-Byte).
   dds_entity_t participant = dds_create_participant(0, NULL, NULL);
   dds_entity_t topic = dds_create_topic(
       participant, /*type-desc=*/&ByteArray_desc,
       "Chatter", NULL, NULL);
   ...
   ```

   Als `type-desc` reicht ein `IDL`-generierter `sequence<octet>`-
   Typ — Cyclones `IDLgen` liefert die Descriptor-Struct.

3. Beide auf derselben Multicast-Gruppe (Default 239.255.0.1:7400)
   und Domain-Id.

**Warum das schon heute funktioniert**: Wire-Compliance hat die komplette
Wire-Compliance validiert (`cyclone_sedp_replay.rs`,
`cyclone_compliance.rs`, `cyclone_live_typelookup.rs`). Fehlt nur ein
Test-Client auf Cyclone-Seite mit matchendem Topic-/Type-Namen.

### Cyclone-Publisher ↔ ZeroDDS-Subscriber

Analog, umgekehrte Richtung. `hello_dds_subscriber` registriert sich
auf `Chatter`, und ein Cyclone-Programm mit demselben Topic + Type
schreibt dort hinein.

## Referenzen

- `tests/interop/matrix.sh` — SPDP-Discovery-Matrix (Cyclone + FastDDS).
- `tests/interop/hello_dds_e2e.sh` — Cross-Process-E2E.
- `tests/interop/xv_pub_sub_roundtrip.sh` — Cross-Vendor Pub/Sub-Roundtrip (Cyclone ⇆ ZeroDDS).
- `crates/dcps/examples/hello_dds_publisher.rs` + `hello_dds_subscriber.rs`.
- `crates/dcps/tests/e2e_dcps_api.rs` — In-Process-Integrationtest.
- `crates/discovery/tests/cyclone_live_sedp.rs` — SEDP-Cross-Vendor-Live-Test.
