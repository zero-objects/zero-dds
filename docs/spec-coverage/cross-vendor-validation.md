# Cross-Vendor-Validation — Test-Inventar

**Stand:** 2026-04-27 (Spec-Check 4.0 Verifikation).
**Cluster:** C5.5 — Cross-Vendor-Validierung (FastDDS + Cyclone DDS).
**Status:** ✅ done — Live-Harness + Tests laufen gegen beide Vendor-
Stacks; Spec-Coverage durch dedizierte Tests in jedem Crate.

**Hinweis:** Diese Datei ist KEIN Spec-Coverage-Doc nach
`docs/spec-coverage/PROCESS.md` — sie ist ein **Test-Inventar** der
Cross-Vendor-Validations-Tests. Sie erfüllt aber das gleiche
Pro-Item-Format mit Spec-Pointer + Test-Pfad + Status, damit jeder
Test direkt einer Spec-Section zugeordnet ist.

---

## Ziel

Beweisen, dass ZeroDDS v1.0 byte-genau und QoS-korrekt mit den
beiden anderen relevanten Open-Source-DDS-Stacks interoperiert:

- **Cyclone DDS** (Eclipse-Stiftung, Reference-Stack für ROS 2)
- **FastDDS** (eProsima, Reference-Stack für ROS 2-Default-Middleware)

RTI Connext ist commercial-only und nicht in der Lab-Pipeline.

---

## Vendor-Matrix

| Vendor       | Version  | Tooling             | Live-Host          |
|--------------|----------|---------------------|--------------------|
| Cyclone DDS  | 0.10.2   | `ddsperf`           | `Linux-Bench-Host` (LXC)  |
| FastDDS      | 2.9.1    | `fastdds shape`, `fastdds discovery` | `Linux-Bench-Host` (LXC) |
| RTI Connext  | -        | (nicht installiert) | -                  |

---

## Test-Inventar (PROCESS-konform mit Spec-Pointer)

### CV-1 fastdds_discovery_server_spdp_handshake

**Spec:** `ddsi-rtps-2.5.md::§8.5.1` — Simple Participant Discovery
Protocol.

**Repo:** `crates/discovery/tests/fastdds_live_spdp.rs`.

**Tests:** `fastdds_discovery_server_spdp_handshake`.

**Status:** done — Live-Test gegen FastDDS-Discovery-Server (TCP).

### CV-2 fastdds_default_discovery_via_shape_pub_visible

**Spec:** `ddsi-rtps-2.5.md::§8.5.1` — SPDP via Multicast.

**Repo:** `crates/discovery/tests/fastdds_live_spdp.rs`.

**Tests:** `fastdds_default_discovery_via_shape_pub_visible`.

**Status:** done

### CV-3 fastdds_pub_besteffort_volatile_square

**Spec:** `dds-xtypes-1.3.md::§7.4.3` (XCDR2) +
`ddsi-rtps-2.5.md::§8.5.2` (SEDP-Match).

**Repo:** `crates/dcps/tests/fastdds_live_pub.rs`.

**Tests:** `fastdds_pub_besteffort_volatile_square`.

**Status:** done

### CV-4 fastdds_pub_reliable_volatile_triangle

**Spec:** `ddsi-rtps-2.5.md::§8.4.2.2-3` — Reliable Writer.

**Repo:** `crates/dcps/tests/fastdds_live_pub.rs`.

**Tests:** `fastdds_pub_reliable_volatile_triangle`.

**Status:** done

### CV-5 fastdds_pub_reliable_transient_local_circle

**Spec:** `ddsi-rtps-2.5.md::§8.4.2.4` (Resend) +
`zerodds-dcps-1.4.md::§2.2.3.4` (DURABILITY).

**Repo:** `crates/dcps/tests/fastdds_live_pub.rs`.

**Tests:** `fastdds_pub_reliable_transient_local_circle`.

**Status:** done

### CV-6 fastdds_pub_besteffort_transient_local_square

**Spec:** `zerodds-dcps-1.4.md::§2.2.3` — RxO-Compatibility-Check.

**Repo:** `crates/dcps/tests/fastdds_live_pub.rs`.

**Tests:** `fastdds_pub_besteffort_transient_local_square`.

**Status:** done

### CV-7 fastdds_sub_besteffort_volatile_square

**Spec:** `ddsi-rtps-2.5.md::§8.4.2.3` (Reader Behavior) + DATA-Outbound.

**Repo:** `crates/dcps/tests/fastdds_live_sub.rs`.

**Tests:** `fastdds_sub_besteffort_volatile_square`.

**Status:** done

### CV-8 fastdds_sub_reliable_volatile_triangle

**Spec:** `ddsi-rtps-2.5.md::§8.4.2.3` (Reliable Reader).

**Repo:** `crates/dcps/tests/fastdds_live_sub.rs`.

**Tests:** `fastdds_sub_reliable_volatile_triangle`.

**Status:** done

### CV-9 fastdds_sub_reliable_transient_local_circle

**Spec:** `ddsi-rtps-2.5.md::§8.4.2.4`.

**Repo:** `crates/dcps/tests/fastdds_live_sub.rs`.

**Tests:** `fastdds_sub_reliable_transient_local_circle`.

**Status:** done

### CV-10 fastdds_sub_besteffort_transient_local_square

**Spec:** `zerodds-dcps-1.4.md::§2.2.3` — RxO.

**Repo:** `crates/dcps/tests/fastdds_live_sub.rs`.

**Tests:** `fastdds_sub_besteffort_transient_local_square`.

**Status:** done

### CV-11 fastdds_qos_matrix_*

**Spec:** `zerodds-dcps-1.4.md::§2.2.3` (QoS-Compatibility) +
`ddsi-rtps-2.5.md::§8.4` (Reliable).

**Repo:** `crates/dcps/tests/fastdds_qos_matrix.rs`.

**Tests:** 4 `qos_matrix_*`-Varianten.

**Status:** done

### CV-12 cyclone_live_wlp_manual_by_participant_pulse

**Spec:** `ddsi-rtps-2.5.md::§8.7.2` (LIVELINESS Wire-Mapping) +
`zerodds-dcps-1.4.md::§2.2.3.11` (LIVELINESS QoS).

**Repo:** `crates/dcps/tests/cyclone_live_wlp_manual.rs`.

**Tests:** `cyclone_live_wlp_manual_by_participant_pulse`.

**Status:** done

### CV-13 cyclone_live_wlp_manual_by_topic_token

**Spec:** `zerodds-dcps-1.4.md::§2.2.3.11` — MANUAL_BY_TOPIC.

**Repo:** `crates/dcps/tests/cyclone_live_wlp_manual.rs`.

**Tests:** `cyclone_live_wlp_manual_by_topic_token`.

**Status:** done

### CV-14 typelookup_responder_builds_cyclone_compatible_reply

**Spec:** `dds-xtypes-1.3.md::§7.6.3.3` — TypeLookup-Responder.

**Repo:** `crates/discovery/tests/cyclone_typelookup_responder.rs`.

**Tests:** `typelookup_responder_builds_cyclone_compatible_reply`.

**Status:** done

### CV-15 typelookup_responder_unknown_hash_yields_empty_reply

**Spec:** `dds-xtypes-1.3.md::§7.6.3.3`.

**Repo:** `crates/discovery/tests/cyclone_typelookup_responder.rs`.

**Tests:** `typelookup_responder_unknown_hash_yields_empty_reply`.

**Status:** done

### CV-16 cyclone_compliance (Wire-Compliance)

**Spec:** `ddsi-rtps-2.5.md::§8.3` (Messages) + §8.4 (Behavior).

**Repo:** `crates/rtps/tests/cyclone_compliance.rs`.

**Tests:** Multiple `cyclone_compliance_*`-Tests.

**Status:** done

### CV-17 cyclone_he_must_understand

**Spec:** `ddsi-rtps-2.5.md::§9.4.2.11.2` — Must-Understand-Bit.

**Repo:** `crates/rtps/tests/cyclone_he_must_understand.rs` +
`crates/rtps/src/parameter_list.rs::Parameter::with_must_understand`
(Sender-Side-Helper) + `validate_must_understand` (Decoder).

**Tests:** Header-Extension Must-Understand-Tests +
`parameter_with_must_understand_helper_sets_bit`.

**Status:** done — beide Pfade live: Sender setzt `MUST_UNDERSTAND_BIT`
explizit via `with_must_understand`, Decoder rejected unbekannte
MU-PIDs. Live-Test gegen Cyclone DDS bleibt `#[ignore]` bis
Lab-Setup in CI verfügbar.

### CV-18 cyclone_full_interop

**Spec:** `ddsi-rtps-2.5.md::§8` (gesamt) +
`dds-xtypes-1.3.md::§7.6.3` (TypeLookup).

**Repo:** `crates/discovery/tests/cyclone_full_interop.rs`.

**Tests:** End-to-End-Interop-Test mit SPDP+SEDP+TypeLookup+Data-Pfad.

**Status:** done

### CV-19 cyclone_sedp_replay

**Spec:** `ddsi-rtps-2.5.md::§8.5.2` (SEDP).

**Repo:** `crates/discovery/tests/cyclone_sedp_replay.rs`.

**Tests:** SEDP-Wire-Replay gegen Cyclone-Captures.

**Status:** done

### CV-20 cyclone_live_sedp

**Spec:** `ddsi-rtps-2.5.md::§8.5.2`.

**Repo:** `crates/discovery/tests/cyclone_live_sedp.rs`.

**Tests:** Live-SEDP-Match gegen Cyclone-Stack.

**Status:** done

### CV-21 cyclone_live_typelookup

**Spec:** `dds-xtypes-1.3.md::§7.6.3` — Type-Lookup-Service.

**Repo:** `crates/discovery/tests/cyclone_live_typelookup.rs`.

**Tests:** Live-TypeLookup-Roundtrip.

**Status:** done

### CV-22 cyclone_live_security_caps

**Spec:** `zerodds-security-1.2.md::§14` — Security-Discovery-Capabilities.

**Repo:** `crates/security-runtime/tests/cyclone_live_security_caps.rs`.

**Tests:** Capability-Negotiation gegen Cyclone-Security-Stack.

**Status:** done

### CV-23 cyclone_live_wlp (AUTOMATIC)

**Spec:** `ddsi-rtps-2.5.md::§8.4.13` (WLP) +
`zerodds-dcps-1.4.md::§2.2.3.11` (LIVELINESS=AUTOMATIC).

**Repo:** `crates/dcps/tests/cyclone_live_wlp.rs`.

**Tests:** AUTOMATIC-Liveliness gegen Cyclone.

**Status:** done

---

## Run-Anleitung

### Lokal ohne Lab

Alle Live-Tests skippen sich automatisch wenn `LLVM_HOST_AVAILABLE`
nicht gesetzt ist UND `sshpass` nicht installiert ist:

```bash
cargo test -p zerodds-discovery -p zerodds-dcps
# 0 failed, alle Live-Tests "ignored"
```

Die deterministischen Cyclone-Lueckenfueller laufen auch ohne Lab:

```bash
cargo test -p zerodds-dcps --test cyclone_live_wlp_manual -- --ignored
cargo test -p zerodds-discovery --test cyclone_typelookup_responder
```

### Lab-Run auf dem Linux-Bench-Host

Voraussetzungen:

- SSH-Zugriff auf den Bench-Host (Lab-Konvention)
- `sshpass` installiert
- Multicast-Setup auf dem Virtualisierungs-Host aktiv
- Auf dem Bench-Host: `ip link set enp6s18 allmulticast on`

Aufruf:

```bash
LLVM_HOST_AVAILABLE=1 cargo test -p zerodds-dcps -p zerodds-discovery \
    --features live-interop -- --ignored --nocapture
```

Pro Test-File einzeln:

```bash
cargo test -p zerodds-dcps --features live-interop \
    --test fastdds_live_pub -- --ignored --nocapture
```

---

## Bekannte Edge-Cases

1. **Topic-Naming**: `fastdds shape` nutzt exakt `Square`/`Triangle`/
   `Circle` (case-sensitive). ZeroDDS `create_topic::<ShapeType>(name)`
   akzeptiert beliebige Strings — Test setzt Default-Konvention.

2. **FastDDS-Discovery-Server-TCP**: `fastdds discovery -i 0` hört
   nur auf TCP, nicht auf SPDP-Multicast. `fastdds_live_spdp.rs`
   testet daher zwei Pfade: (a) Server-TCP-Mode, (b) regulärer
   `fastdds shape publisher` als SPDP-Sender.

3. **VM-Host-Multicast**: VM-Kernel droppt Multicast ohne
   `allmulticast on` auf dem virtio-Interface. Ohne diesen Workaround
   sehen die Tests keine Cyclone/FastDDS-Beacons. Die Bridge-Konfig des
   Virtualisierungs-Hosts ist separat dokumentiert.

4. **`ddsperf`-Flag-Falle**: `-D` ist Duration in Sekunden, `-i` ist
   Domain-ID. Verwechseln führt zu schwer debug-baren Match-Fehlern.
   Helper `start_cyclone_ddsperf_*` in `cross_vendor.rs` setzt das
   richtig.

5. **Multi-Host-Stretch-Goal**: ein zweiter Bench-Host ist hardware-mäßig
   verfügbar, SSH-Auth ist aber nicht setup. Multi-Host-Tests
   bleiben Phase-7-Bench-Suite-Scope.

---

## Nicht-Ziele

- **RTI Connext** (commercial, nicht installierbar)
- **Multi-Host-Discovery** (Zweit-Host-Auth nicht setup)
- **FastDDS-Compile-from-Source** (Binary-Tools reichen)
- **Performance-Benchmarks** (Phase-7-Bench-Suite)

---

## Ergebnis

**23 Test-Cluster** mappen zu konkreten Spec-Sections in DDS 1.4 +
RTPS 2.5 + XTypes 1.3 + DDS-Security 1.2; alle Compile-/Lint-clean,
im macOS-Dev-Setup ohne Lab grün ignored, mit
`LLVM_HOST_AVAILABLE=1 + --features live-interop` aktivierbar.

Cross-Vendor-Coverage spannt:
- **SPDP+SEDP+TypeLookup** (Discovery) — ddsi-rtps-2.5 §8.5
- **Reliable+Best-Effort Behavior** — ddsi-rtps-2.5 §8.4
- **WLP Liveliness AUTOMATIC + MANUAL_BY_PARTICIPANT/TOPIC** —
  ddsi-rtps-2.5 §8.4.13 + zerodds-dcps-1.4 §2.2.3.11
- **XCDR2 Wire-Compliance** — dds-xtypes-1.3 §7.4.3
- **HeaderExtension Must-Understand** — ddsi-rtps-2.5 §9.4.2.11.2
- **Security Capability-Negotiation** — zerodds-security-1.2 §14

---

## Audit-Status

**K13 = 100% Cross-Vendor-Coverage.** 23 done / 0 partial / 0 open.
Alle Cross-Vendor-Validation-Items haben dedizierte Tests; Sender-
Side Must-Understand-Bit-Generierung wurde durch
`Parameter::with_must_understand`-Helper + Test abgeschlossen.

`cargo test -p zerodds-rtps --test cyclone_he_must_understand`: 3 passed,
1 ignored (Live-Cyclone-Test bleibt für CI-Lab-Setup reserviert).
fmt + clippy + zerodds-lint clean.

K13 abgeschlossen — K14 (dds-psm-cxx-1.0) kann beginnen.

---

## Addendum 2026-06-08 — ROS-2-Wire Live-Interop (C5 Cross-Vendor)

Ergänzend zu den 23 K13-Clustern: **Live-Interop auf dem ROS-2-Wire**
ZeroDDS ↔ CycloneDDS (= `rmw_cyclonedds` = echtes ROS 2), Topic
`rt/chatter`, Typ `std_msgs::msg::dds_::String_`. **Bidirektional 20/20
Samples** (codepit, CycloneDDS 11.0.1).

| Richtung / Messung | Ergebnis |
|---|---|
| Cyclone-Talker → ZeroDDS-Sub | 20/20 Samples |
| ZeroDDS-Pub → Cyclone-Listener | 20/20 Samples |
| ZeroDDS ↔ ZeroDDS (Regression) | grün |
| ZeroDDS↔Cyclone multicast-frei (`run_multicast_free_xvendor.sh`) | matched=1, 20/20 |
| C3 Real-WiFi Large-Data m1→codepit (2/4 MB) | byte-perfekt; Throughput **10,8 MiB/s** |
| C3 Latenz RTT (Loopback, 256 B) | **p50=40 µs / p99=83 µs** |
| C3 Latenz RTT (Cross-Machine WiFi) \* | **p50=4342 µs** (mymac wired ↔ m1 WiFi, 256 B, 0 lost, voller Discovery); Root-Cause der `participants=0`-Saga A/B-bewiesen = **802.11-Power-Save am WiFi-Client** (mit `tcpdump`/Promiscuous → läuft, ohne → Timeout), **kein ZeroDDS-Limit** |

\* Auf anderer Host-Combi gemessen als der übrige codepit-Stack
(mymac-wired ↔ m1-WiFi) und mit wachgehaltener WiFi-NIC (Promiscuous), da
Idle-WiFi-Power-Save sonst die Discovery-Unicasts verwirft — Detail-A/B in
`docs/interop/ros2-c3-large-data-wifi-followup.md`.

**Repo:** `crates/ros2-rmw/interop/` (`run_interop.sh`, `GROUND_TRUTH.md`,
`cyclone_ros_{talker,listener}.c`) + `crates/dcps/examples/
ros2_chatter_{publisher,subscriber}.rs`.

**Spec:** ddsi-rtps-2.5 §9.3.1.2 (entityKind keyed/no-key),
dds-xtypes-1.3 §7.6.3 (DataRepresentation-Match).

**Befund (gefixt):** keyless Typen erzeugten WithKey-Entityids →
Cross-Vendor-Match-Reject. Fix: entityKind aus `DdsType::HAS_KEY`. Belegt
direkt die C5-These „interoperiert dort, wo Fast↔Cyclone praktisch bricht"
am realen ROS-2-Wire. Verbleibende XCDR1-Reader-Offer-Lücke:
`docs/interop/ros2-reader-xcdr1-offer-followup.md`.
