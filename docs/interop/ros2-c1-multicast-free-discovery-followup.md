# C1 — Multicast-freie Discovery (Initial-Peers / Discovery-Server)

- **Status:** ✅ **implementiert + e2e-verifiziert**, ZeroDDS↔ZeroDDS **und**
  Cross-Vendor ZeroDDS↔CycloneDDS, beide multicast-frei. Offen nur noch:
  >100-Participant-Skalierungs-Messung.
- **Datum:** 2026-06-08
- **Kontext:** ROS-2-Pain-Cluster C1 (Discovery — N²-Skalierung,
  Multicast-Abhängigkeit; der **#1-ROS-Schmerz** und der strukturelle
  Treiber hinter `rmw_zenoh`). `docs/ros2-dds-painpoints-strategy.md` §C1.

## Ziel

Discovery **ohne Multicast**: konfigurierte Unicast-Initial-Peers (wie
CycloneDDS `Peers` / FastDDS `initialPeersList`), damit ZeroDDS in
Netzen funktioniert, die Multicast droppen (WiFi, Cloud-VPC, geswitchte
Subnetze) — und ohne SPDP-Multicast-Storm bei >100 Participants.

## Analyse des Ist-Zustands (verifiziert, runtime.rs)

1. **SPDP-Bootstrap ist multicast-only.** Der periodische Beacon
   (`tick_loop`, runtime.rs:5308) und `announce_spdp_now` senden
   ausschließlich an `mc_target` (SPDP-Multicast-Gruppe). Ohne Multicast
   findet kein Participant einen anderen.
2. **Der Unicast-SPDP-Socket bindet einen *ephemeral* Port**
   (`spdp_uc = UdpTransport::bind_v4(UNSPECIFIED, 0)`, runtime.rs:1835).
   Sein Locator (`metatraffic_unicast_locator`) wird zwar im Beacon
   annonciert — aber **erst nachdem** der Peer ZeroDDS via Multicast
   gefunden hat. Für den *Bootstrap* ist er nutzlos: ein Standard-Peer
   (Cyclone/FastDDS) sendet Initial-SPDP an den **well-known** RTPS-Port
   `7400 + 250·domain + 10 + 2·participant_id` (RTPS §9.6.1.1, PB+DG·d+d1+PG·pid),
   nicht an einen ephemeral Port, den er nicht kennen kann.
3. **Der rmw-Shim modelliert das Konzept schon**
   (`unicast_initial_peers: Vec<String>`, `zerodds-ros2-shim.rs:433`,
   geparst + im Banner gezeigt) — aber es ist **nicht** in den
   DCPS-Runtime-Discovery-Pfad verdrahtet. Reine Karteileiche bis C1.
4. **Ripple ist niedrig:** `RuntimeConfig` hat nur **2** Voll-Literal-
   Konstruktionen (beide in runtime.rs); ein neues Feld mit Default
   bricht sonst nichts.

## Design (Implementierungs-Pfad)

1. **Well-known-Port-Bind für den Unicast-SPDP-Socket** statt ephemeral:
   `participant_id`-indexiert `7400 + 250·domain + 10 + 2·pid`, mit
   pid-Allokation (pid 0,1,2… bis Bind gelingt — Standard-RTPS-Verhalten
   bei mehreren Participants pro Host). **Das ist das Herzstück** — ohne
   well-known Port ist standard-interoperable Initial-Peer-Discovery
   unmöglich. Der announcte `metatraffic_unicast_locator` nutzt dann
   diesen Port.
2. **`initial_peers: Vec<Locator>` in `RuntimeConfig`** (Default leer →
   kein Verhaltens-Change) + Env `ZERODDS_PEERS` (Komma-Liste `ip` oder
   `ip:port`; ohne Port → well-known SPDP-Unicast-Ports für pid 0..N
   scannen, wie Cyclone `MaxAutoParticipantIndex`). Nur die 2 runtime.rs-
   Literale anpassen.
3. **Beacon-Send an Initial-Peers:** im periodischen Send (runtime.rs:5308)
   und in `announce_spdp_now` den Beacon **zusätzlich** an jeden
   Initial-Peer-Unicast-Locator senden (`spdp_mc_tx.send` kann auch
   unicast). Optional Multicast komplett abschaltbar
   (`spdp_multicast_enabled: bool`).
4. **Shim-Verdrahtung:** `unicast_initial_peers` (Strings) →
   `RuntimeConfig.initial_peers` (Locator) parsen.

## Cross-Vendor-Relevanz

Mit well-known-Port + Peer-Send interoperiert ZeroDDS im **reinen
Unicast-Modus** mit CycloneDDS (`<Discovery><Peers><Peer address=…>`,
Multicast via `<General><AllowMulticast>false`) und FastDDS
(`initialPeersList` + `use_builtin_transports` ohne Multicast) — der
direkte „DDS ohne Packet-Storm"-Beleg.

## Test-Plan

- **ZeroDDS↔ZeroDDS multicast-frei:** zwei Runtimes mit
  *unterschiedlichen* Multicast-Gruppen (MC-Discovery unmöglich) +
  `initial_peers` gegenseitig auf die well-known Unicast-Ports → müssen
  sich trotzdem discovern. Negativ-Kontrolle: ohne `initial_peers` →
  keine Discovery.
- **Cross-Vendor (codepit):** ZeroDDS + Cyclone beide Multicast aus +
  Peers gegenseitig konfiguriert → SPDP/SEDP/Daten fließen. pcap-Beleg,
  dass **kein** Multicast-Paket fließt.
- **Skalierung (C1-Teil-2):** >100 Participants, SPDP-Traffic messen
  (Unicast-Point-to-Point vs. N²-Multicast-Storm).

## Effort

~2–3 PT. Delikat ist der well-known-Port + pid-Allokation (Punkt 1);
Config/Env/Send-Pfad (2–4) sind geradlinig.

---

## Umgesetzt (2026-06-08) — `crates/dcps/src/runtime.rs`

1. **well-known-Port-Bind** ✅ — `spdp_uc` bindet jetzt
   `7400+250·domain+10+2·pid` mit Participant-Index-Allokation
   (pid 0..120, Fallback ephemeral). `spdp_unicast_port()` lokal in `dcps`
   (kein `crates/rtps`-Edit). Regression: bestehende Multicast-Discovery +
   Live-Cyclone-Interop unveraendert grün (codepit 20/20).
2. **`RuntimeConfig.initial_peers: Vec<Locator>`** + Env **`ZERODDS_PEERS`**
   (`ip` → well-known Ports pid 0..10; `ip:port` → exakt). Domain-aware in
   `DcpsRuntime::start` gemerged.
3. **Beacon-Send an Peers** (`send_spdp_to_initial_peers`) an beiden
   SPDP-Send-Stellen (periodisch + `announce_spdp_now`).
4. **`ZERODDS_NO_MULTICAST`** (`spdp_multicast_send=false`) — echtes
   Multicast-AUS: kein einziges Multicast-Paket, reine Unicast-Discovery.

### Tests
- Unit (`runtime.rs`): `spdp_unicast_port_follows_rtps_formula`,
  `expand_initial_peer_ip_only_yields_well_known_port_range`,
  `multicast_free_discovery_via_initial_peers` (Multicast **komplett aus**
  → Discovery nur via Unicast-Peers). 441 dcps-Lib-Tests grün.
- **e2e (codepit, `crates/ros2-rmw/interop/run_multicast_free.sh`):** zwei
  ZeroDDS-Prozesse, `ZERODDS_NO_MULTICAST=1`:
  - MIT `ZERODDS_PEERS=127.0.0.1`: **19–20 Samples** (Sub auf well-known
    Port 7410, Pub auf 7412, Daten über reinen Unicast).
  - OHNE Peers: **0 Samples** (Negativ-Kontrolle — kein anderer Kanal).

### Noch offen (C1-Rest)
- **Cross-Vendor multicast-frei:** ✅ **funktioniert** —
  `crates/ros2-rmw/interop/run_multicast_free_xvendor.sh`. ZeroDDS-Sub
  (`ZERODDS_NO_MULTICAST=1` + `ZERODDS_PEERS=<host-ip>`) ↔ Cyclone-Talker
  (`<AllowMulticast>false` + `<Peers>`): **`matched subscriber=1`, 20/20
  Samples**, reine Unicast-Discovery, **kein einziges Multicast-Paket**
  (das WiFi/Cloud-VPC-Szenario). Cyclone-finest-Trace bestätigt: Cyclone
  legt den ZeroDDS-Proxy an (`SPDP ST0 91839248… NEW meta udp/…:7410`) +
  empfängt die SEDP. Discovery braucht ~9 s (5 s-SPDP-Periode bidirektional)
  → Talker muss lange genug publizieren. **Wichtig:** Host-IP (nicht
  `127.0.0.1`) durchgängig nutzen — ZeroDDS annonciert den eth0-Locator,
  also muss Cyclones Interface dieselbe IP sein. `ZERODDS_DATA_REPR_OFFER=
  XCDR1,XCDR2` nötig (separate Reader-Offer-Lücke, nicht C1).
- **Skalierung:** ✅ gemessen. `multicast_free_discovery_scales_to_many_
  participants` (`#[ignore]`, N via `ZERODDS_SCALE_N`): **all-to-all
  multicast-frei** — 12 in ~9 s, **50 in 2,9 s, 100 in 19,9 s** (jeder
  Participant sieht alle N−1 anderen), reine Unicast-Discovery, **kein
  N²-Multicast-Storm**. Die Zeit ist Single-Host-CPU-Contention (100 volle
  DDS-Stacks in einem Prozess), nicht das Protokoll. `INITIAL_PEER_MAX_
  PARTICIPANTS` (Default 10) ist jetzt via `ZERODDS_MAX_PEER_PARTICIPANTS`
  (1..120) erhöhbar für dichte Multi-Robot-/>10-pro-Host-Szenarien.
- **WLP-über-Unicast:** WLP-Heartbeats gehen weiter nur an Multicast
  (separates Liveliness-Thema, kein Discovery-Blocker).
- **Shim-Verdrahtung:** `unicast_initial_peers` (zerodds-ros2-shim.rs) →
  `RuntimeConfig.initial_peers`/`ZERODDS_PEERS` (heute funktional via Env;
  C-ABI-Live-Wiring ist der rmw-Integrations-Schritt).
