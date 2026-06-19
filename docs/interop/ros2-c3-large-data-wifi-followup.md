# C3 — Große Daten / WiFi-robuste Fragmentierung

- **Status:** ✅ Mechanismus vorhanden; **1-MiB-Silent-Drop gefixt** +
  **Real-WiFi-e2e verifiziert** (2/4 MB cross-machine über echten WiFi-Link).
  Offen nur: variable-Zero-Copy (SHM-Track).
- **Datum:** 2026-06-08
- **Kontext:** ROS-2-Pain-Cluster C3 (PointCloud2/Images über WiFi: IP-
  Fragmentierungs-Hang, 30-s-Stalls, Large-Message-Drops — der zweite
  strukturelle Zenoh-Treiber). `docs/ros2-dds-painpoints-strategy.md` §C3.

## Analyse des Ist-Zustands (verifiziert)

ZeroDDS hat den **App-Level-Fragmentierungs-Pfad bereits vollständig**:
- `DEFAULT_FRAGMENT_SIZE = 1344` (`reliable_writer.rs:68`) = 1400-MTU −
  RTPS-Header. Das ist **RTPS-DATA_FRAG auf Anwendungsebene**, NICHT
  IP-Fragmentierung — genau der Punkt, der ROS' WiFi-Stall verursacht
  (IP-Frag verliert ein Fragment → ganzer Datagram-Reassembly-Timeout).
- `set_fragmentation(fragment_size, mtu)` (`reliable_writer.rs:249`):
  Ethernet-sicherer Default für Remote, same-host angehoben.
- **Selektiver Retransmit** via `handle_nackfrag` (`reliable_writer.rs:736`)
  — NACK_FRAG fordert nur die **fehlenden** Fragmente nach, nicht das
  ganze Sample. Tests: `tick_resends_requested_fragments`,
  `acknack_resend_for_fragmented_sn_sends_all_fragments`.
- `FragmentAssembler` mit DoS-Caps (`fragment_assembler.rs`), WP-1.2:
  10 kB @ 30 % Loss byte-identisch reassembliert.

**Fazit:** Der C3-Kernmechanismus (WiFi-sichere App-Frag + selektiver
Retransmit) ist da und unit-bewiesen.

## Gefunden + gefixt (2026-06-08): 1-MiB-Silent-Drop

DCPS-Example-Level-Test (`largedata_pub`/`_sub`, codepit) deckte auf, dass
Samples **> 1 MiB still verworfen** wurden — obwohl der Writer sie sendet
und die RTPS-Frag unit-getestet ist. Ursache: der Reassembler-DoS-Cap
`DEFAULT_MAX_SAMPLE_BYTES = 1 MiB` (`fragment_assembler.rs`, Phase-1-
Annahme „grosse Images = kein Use-Case"). Schwelle exakt: 1,0 MB OK,
1,4 MB gedroppt. ROS PointCloud2/Image sind aber oft mehrere MB.

**Fix (dcps, kein rtps-Edit):** `RuntimeConfig.max_reassembly_sample_bytes`
(Default **16 MiB**, Env `ZERODDS_MAX_SAMPLE_BYTES`) wird am Reader-
Konstruktor in `AssemblerCaps.max_sample_bytes` gesetzt statt
`AssemblerCaps::default()`. Bleibt DoS-Guard, nur ROS-realistisch.

**Verifiziert (codepit, `crates/ros2-rmw/interop/run_largedata.sh`):**
2 MB / 4 MB / 8 MB Samples alle **intakt** (Muster-Check) durch den vollen
DCPS-Stack mit DATA_FRAG/Reassembly. Regression-Test
`default_reassembly_cap_is_ros_realistic`.

## Synergie mit C1

WiFi droppt oft Multicast → die **C1-Unicast-Initial-Peers**
(`ZERODDS_NO_MULTICAST` + `ZERODDS_PEERS`) sind die Voraussetzung, damit
sich zwei Knoten über WiFi überhaupt finden. C3-WiFi-Test baut direkt
auf C1 auf.

## Offen

1. **Real-WiFi-e2e:** ✅ **verifiziert** — `run_wifi_largedata.sh`.
   Publisher auf m1-MacBook (WiFi, gemessen ~50 % ICMP-Loss / 226 ms RTT)
   → Subscriber auf codepit, multicast-frei (Discovery via C1-Unicast-
   Peers, da WiFi Multicast droppt). **2 MB und 4 MB Samples
   (~1560 / ~3120 Fragmente) byte-perfekt reassembliert** (intakt=5,
   korrupt=0), selektiver NACK_FRAG-Retransmit übersteht den WiFi-Loss,
   kein 30-s-Stall. Beweist „PointCloud2 über WiFi funktioniert einfach"
   + kombiniert C1 (multicast-frei) + C3 (Frag/Cap-Fix) cross-machine.
   **Throughput gemessen** (`largedata_pub … 0` back-to-back +
   `largedata_sub`): **10.8 MiB/s (~86 Mbit/s)** fragmentierte Large-Data
   über den WiFi-Link (13×1 MiB in 1,20 s, 0 korrupt, trotz ~50 % ICMP-
   Loss).
   **Latenz gemessen** (`latency_ping`/`latency_pong`, Round-Trip
   clock-sync-frei): auf Loopback **256 B RTT min=37 / p50=40 / p90=48 /
   p99=83 / max=137 µs** (200 Samples, 0 lost) — saubere µs-Protokoll-/
   Stack-Latenz.

   **WiFi-Bidir-RTT (B7) — Trigger ist 802.11-Power-Save am WiFi-Client, ABER
   das `participants=0` war zu einem guten Teil eine ZeroDDS-Robustheits-Lücke
   (kein Initial-Announcement-Burst) — jetzt gefixt (commit `f7ba0b92`).**
   *Frühere Einordnung („kein ZeroDDS-Bug, Mitigation nur OS-/AP-Ebene") war ein
   Excuse: ein DDS, das beim Start nur EINEN SPDP-Beacon + 5s-Periode sendet,
   verliert ihn zuverlässig im Cold-Start-/Sleep-Fenster. FastDDS/Cyclone
   überleben genau das mit ihrem Initial-Announcement-Burst — ZeroDDS hatte den
   nicht. Siehe „Fix" unten.*
   Cross-Machine-Latenz gab über Wochen `participants=0`. Der saubere A/B
   (`sudo tcpdump` auf m1 als einzige Variable) hat den Trigger isoliert:

   | Bedingung (identisches Pinning/Befehle/Hosts) | Ergebnis |
   |---|---|
   | **MIT `tcpdump`** (Promiscuous hält m1-WiFi-NIC wach) | `participants=1 pubs=1 subs=1`, **256 B RTT: min=2873 / p50=4342 / p90=7444 / p99=8166 µs**, 15 Samples, 0 lost |
   | **OHNE `tcpdump`** (12 s Idle → WiFi-Power-Save) | `participants=0`, Discovery-Timeout, 0 RTTs |

   Combi: **mymac (wired, `192.168.178.47`) ↔ m1 (WiFi, `.192`)**, beide mit
   `ZERODDS_INTERFACE`-Pinning + `ZERODDS_NO_MULTICAST`/`ZERODDS_PEERS`.
   m1-Capture (43 KB pcap) zeigt im Wach-Fenster vollen bidirektionalen
   Fluss (320 m1→mymac + 58 mymac→m1 Pakete).

   - **Mechanismus:** Der WiFi-Client (m1) geht im Idle-Discovery-Fenster
     in 802.11-Power-Save; die latenzsensitiven Unicast-SPDP/SEDP-Frames
     werden im RX-Sleep verworfen/zu spät zugestellt → Reliable-SEDP
     schließt nie ab. Promiscuous (oder Dauer-Traffic) deaktiviert
     Power-Save → die NIC bleibt wach → Discovery+RTT laufen sofort. Das
     ist das klassische **„funktioniert, sobald Wireshark läuft"**.
   - **Das vereinheitlicht die frühere codepit→m1-„cold/prime"-Beobachtung:**
     schlafende NIC verwirft Cold-Ingress; eigenes Senden (m1→codepit,
     bzw. „Prime") weckt sie → Return-Pfad offen. Die zuvor vermutete
     AP-Ingress-Filterung war eine Fehlattribution — Power-Save erklärt
     alle Beobachtungen ohne Zusatzannahme (ein zusätzlicher AP-Filter ist
     nicht ausgeschlossen, aber zur Erklärung nicht nötig).
   - **Warum forward-Large-Data (m1→codepit) immer lief:** m1 war als
     Dauer-Sender aktiv → NIC durchgehend wach → kein Power-Save.
   - **Fix (ZeroDDS-seitig, commit `f7ba0b92`) — Initial-Announcement-Burst:**
     beim Start (bis ein Peer entdeckt ist, bounded auf
     `initial_announce_count`, Default 10) sendet ZeroDDS SPDP in schneller
     `initial_announce_period`-Kadenz (Default 200 ms) statt nur 1× + 5s. Das
     (a) hält die WiFi-NIC durch häufiges TX wach, (b) hält die
     stateful-Firewall-Pinhole offen und (c) elicitiert gerichtete
     SPDP-Antworten, die in den Wach-Fenstern ankommen — analog FastDDS
     `initial_announcements`. Cadence deterministisch bewiesen
     (`crates/dcps/tests/initial_announce_burst.rs`: peer-loser Participant
     burstet 8× vs. 1× bei `count=0`). Env: `ZERODDS_INITIAL_ANNOUNCE_COUNT` /
     `_PERIOD_MS`.
   - **Restanteil OS-/AP-Ebene:** ein aggressiver Power-Save kann RX trotzdem
     verzögern; der Burst minimiert das Discovery-Fenster, beseitigt PS aber
     nicht restlos — PS-aus / DTIM-Tuning bleiben ergänzend sinnvoll. Aber der
     **DDS-Stack trägt jetzt seinen Teil** (Burst), statt das Problem nur
     wegzudelegieren. Loopback-Bidir-Ping-Pong p50=40 µs unverändert; ~4,3 ms
     p50 cross-WiFi spiegeln den WiFi-Hop wider, keinen Protokoll-Overhead.
   - **Wo Loss überhaupt vorkommt — und wo nicht:** Auf moderner *wired* Infra
     (Threadripper/ECC/2×10GbE/Arosa-Switch) ist Paketverlust effektiv null;
     Discovery ist sofort, Burst hin oder her. Der Burst ist dort ein **No-Op**.
     Loss tritt **ausschließlich am WiFi-Hop** auf: ein ROS-2-Roboter/Laptop mit
     802.11-Power-Save verschläft RX-Fenster und verliert einen einzelnen
     Erst-Beacon → `participants=0`. **Genau und nur dafür** ist der Burst da
     (FastDDS/Cyclone machen es identisch). Eine netem-Loss-Simulation auf der
     sauberen Wired-Kiste ist daher künstlich und aussagelos — sie bestätigte
     bloß, dass ZeroDDS wired sofort discovert (≈50 ms selbst bei hohem
     Random-Loss, da port-lose Peers ~10 Pakete/Announce senden). Die einzige
     aussagekräftige Messung ist das echte WiFi-Szenario (m1-MacBook,
     `run_wifi_largedata.sh` mit der `participants=0`-Wiederholung); Burst-Kadenz
     selbst ist deterministisch bewiesen (`initial_announce_burst.rs`).
2. **DCPS-Example-Level-Large-Data:** ein Beispiel mit konfigurierbarer
   Payload (z.B. `ros2_pointcloud_{pub,sub}`), das DATA_FRAG/Reassembly
   durch den **vollen DCPS-Stack** treibt (heute nur RTPS-Unit-Level).
3. **Variable-size Zero-Copy** (`transport-shm`): ✅ **vom Design erfüllt** —
   der SHM-Ring ist **length-prefixed** (variable Datagramme), KEIN
   Iceoryx-Fixed-Pool/Vorab-Dimensionierung. B8 hob den einzigen Cap
   (`max_datagram` 64 KiB → `ZERODDS_SHM_MAX_DATAGRAM`-konfigurierbar,
   Kapazität zieht mit) für PointCloud2/Image-Größen. Test
   `shm_config_satisfies_capacity_constraint_and_default`. Offen nur:
   e2e-Lauf mit `--features same-host-shm` + großem Sample (Opt-in-Pfad;
   der Default-UDP-Pfad deckt Large-Data bereits via Cap-Fix ab).
4. **MTU-Auto-Discovery:** Path-MTU statt fixem 1344 (optional, Perf).

## Effort

Real-WiFi-e2e + Tooling ~1–2 PT; variable Zero-Copy separater
SHM-Track. Kern-Mechanismus = 0 (vorhanden).
