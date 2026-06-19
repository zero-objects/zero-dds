# Cross-Vendor- / Inter-Distro-Interop

← [Zurück zur Übersicht](index.md)

## Der Schmerz

DDS *verspricht* Interoperabilität — das ist der ganze Sinn eines Wire-Standards. In der ROS-2-Praxis bricht es häufig (**32 Reports**):

- **Mixed-RMW-Flotten reden nicht.** Ein `rmw_fastrtps`-Node und ein `rmw_cyclonedds`-Node auf demselben Topic tauschen womöglich keine Daten aus; Services und Actions (auf Pub/Sub gebaut) sind effektiv vendor-locked, selbst wenn reines Pub/Sub funktioniert.
- **Cross-Vendor-Deserialisierungs-Mismatches** können schlimmer sein als ein No-Match — ein fehlerhafter Cross-RMW-Request hat Out-of-Memory auf dem Server ausgelöst.
- **Inter-Distro wird nicht unterstützt.** Ein Humble-Node und ein Eloquent-/Jazzy-Node auf derselben Domain können oft nicht kommunizieren und stranden inkrementelle Flotten-Upgrades.
- **XTypes-Encoding-Drift.** Selbst compliant aussehende Stacks sind sich über CDR-/XCDR2-Encoding-Details uneinig, sodass Type-Matching still scheitert.

### Jüngstes Beispiel

**[rmw_cyclonedds#577 — „Cross-RMW service interoperability: ListParameters request from rmw_cyclonedds_cpp client can be misdeserialized and trigger OOM on rmw_fastrtps_cpp server"](https://github.com/ros2/rmw_cyclonedds/issues/577)** (2026-04-02). Ein Cross-Vendor-Service-Call ist nicht nur inkompatibel — er wird in eine Allocation *fehl-deserialisiert*, die den Server crasht. Interop-Fehler als Denial-of-Service.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2026-04-02 | [rmw_cyclonedds#577](https://github.com/ros2/rmw_cyclonedds/issues/577) | Cross-RMW-Service-Deserialisierung → OOM-Crash |
| 2025-06-12 | [RTI KB](https://community.rti.com/kb/xtypes-compliance-mismatch) | Connext-Default-CDR nicht konform mit XTypes 1.3 |
| 2025-05-14 | [ROS Discourse](https://discourse.openrobotics.org/t/incompatability-between-distributions/43747) | Inkompatibilität zwischen ROS-2-Distributionen |
| 2024-09-18 | [ROS Discourse](https://discourse.openrobotics.org/t/difference-between-dds-design-and-reality/39669) | „DDS-Design vs Realität": Services/Actions vendor-locked |
| 2024-08-05 | [cyclonedds#2062](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2062) | Cyclone ↔ Micro XRCE-DDS Comms |

## Wie ZeroDDS es löst

**Interop ist das Design-Zentrum, und es wird kontinuierlich gegen vier Vendoren getestet.**

- **Natives RTPS 2.5 auf dem Draht.** ZeroDDS ist verifiziert interoperabel mit Cyclone DDS, Fast DDS, OpenDDS und RTI Connext, gepflegt als Cross-Vendor-Matrix (inkl. Security-Matrix) — dieselben Stellen, an denen Fast DDS ↔ Cyclone im Feld bricht, sind für uns Regressions-Zellen.
- **Live-ROS-2-Interop, beide Richtungen.** `rmw_zerodds` tauscht Daten mit einem echten `rmw_cyclonedds`-Talker/Listener auf `rt/chatter` aus, **20/20 in beide Richtungen**. Der Bug, der dies ursprünglich blockierte (Keyed-vs-Keyless-Entity-Kind), ist durch Konsultieren von `DdsType::HAS_KEY` gefixt.
- **XCDR1 *und* XCDR2.** ZeroDDS modelliert `DataRepresentationQosPolicy` und bietet beide Encodings; `ros_defaults()` bietet XCDR1 für ROS-Writer out of the box, sodass der „compliant-but-doesn't-match"-Encoding-Drift behandelt ist. Das XCDR2-Alignment von ZeroDDS wurde byte-für-byte gegen ein Cross-Vendor-Capture validiert.
- **Volle XTypes 1.3 + DDS-RPC.** TypeObject/TypeLookup und Assignability sind implementiert, und die DDS-RPC-Spec (Services) ist standardkonform implementiert — das Fundament, das Services/Actions brauchen, um nicht mehr vendor-locked zu sein.
- **Memory-safe Parsing.** Ein fehlerhafter Cross-Vendor-Request kann nicht so in ein OOM fehl-deserialisiert werden, wie [rmw_cyclonedds#577](https://github.com/ros2/rmw_cyclonedds/issues/577) es beschreibt: das Decoding läuft in sicherem Rust mit expliziten Bounds und DoS-Caps.

## Warum es kein Schmerz mehr sein muss

Interop bricht, wenn das „compliant" jedes Vendors in den Encoding- und Entity-Kind-Details divergiert, und wenn Parser dem Draht vertrauen. ZeroDDS behandelt Cross-Vendor-Interop als First-Class-Anforderung mit kontinuierlichen Tests (vier Vendoren, beide Richtungen) und parst defensiv — sodass eine heterogene Flotte eine unterstützte Konfiguration ist, kein Glücksspiel.

## Selbst reproduzieren

```bash
# rmw_zerodds ↔ echtes rmw_cyclonedds, bidirektional, auf rt/chatter:
crates/ros2-rmw/interop/run_interop.sh

# Cross-Vendor, multicast-frei:
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh
```

Siehe auch den Cross-Vendor-Validierungs-Record in [`../spec-coverage/cross-vendor-validation.md`](../spec-coverage/cross-vendor-validation.md).

→ [Zurück zur Übersicht](index.md) · Weiter: [Konfigurations-Komplexität](config-complexity.md)
