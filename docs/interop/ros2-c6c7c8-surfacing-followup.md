# C6/C7/C8 — Querschnitt-Cluster: Capability-Mapping + ROS-Surfacing

- **Status:** **ERLEDIGT 2026-06-12** — alle drei Surfacing-Schritte
  implementiert + getestet (vorher: analysiert/Capability-vorhanden).
- **Datum:** 2026-06-08 (analysiert), 2026-06-12 (umgesetzt)

## Umsetzung (2026-06-12)

- **C6** — `RuntimeConfig::multi_robot()` (`crates/dcps/src/runtime.rs`):
  benanntes WAN/Cross-Subnet-Profil analog `ros_defaults()` — multicast-freie
  Unicast-Discovery (`spdp_multicast_send=false`) + ROS-XCDR1/2-Reprs +
  WAN-tolerante 300s-Lease. Tests: Config-Assertion + e2e (zwei Runtimes in
  verschiedenen Multicast-Buckets discovern NUR via Unicast, vom Profil
  getrieben) + Doc-Test. Commit 9d14829e.
- **C7** — `SecurityProfile::from_enclave_dir()` + `from_env()`
  (`crates/security-runtime/src/profile.rs`): SROS2-Enclave-Verzeichnis →
  Profil in einem Call; `ZERODDS_SECURITY_DIR`+`ROS_DOMAIN_ID`-Env-Pfad.
  c-api `zerodds_runtime_create_secure_from_env`; rmw-Shim lädt das Enclave
  opt-in (`--features security`) in `rmw_zerodds_init` (hard error bei
  set-but-invalid — kein Silent-Downgrade). Commit 3361c94b.
- **C8** — `zerodds-ros2-shim doctor` + `graph`
  (`crates/rmw-zerodds-shim/src/bin/`): discovery-unabhängige Diagnose
  (RMW/Distro/Domain-Konsistenz, multicast-frei-ohne-Peers = hard fail,
  Enclave-Validität, Mangling-Selftest; ok/warn/fail + exit 5). `graph`
  zeigt lokalen Participant + Topic-Graph. 5 e2e-Tests. Commit 1821f249.

### Historischer Analyse-Kontext (2026-06-08)
- **Kontext:** ROS-2-Pain-Cluster C6 (WAN/Multi-Robot), C7 (Security-UX),
  C8 (Tooling). Querschnitt — ZeroDDS hat das Fundament, die Lücke ist
  ROS-facing Surfacing, nicht Kern-Implementierung.

## C6 — WAN / Multi-Robot / Cross-Subnet

**Vorhanden (code-belegt):**
- **C1-Unicast-Initial-Peers** (`ZERODDS_PEERS` + `ZERODDS_NO_MULTICAST`,
  diese Session) = der multicast-freie Cross-Subnet-Discovery-Pfad. WAN/
  Cloud/WiFi droppen Multicast → Unicast-Peers sind die Lösung. **e2e
  belegt** (ZeroDDS↔ZeroDDS + ZeroDDS↔Cyclone multicast-frei).
- **Multi-Interface-Routing** (`DcpsRuntime::route`, runtime.rs) +
  `interface_bindings` / „wan-default"-Binding.
- **Bridge-Crates** `zenoh-bridge`, `opcua-gateway`, `grpc-bridge`,
  `mqtt-bridge`, `coap-bridge`, `websocket-bridge`, `amqp-bridge` —
  Protokoll-Übergänge für heterogene/WAN-Topologien.

**Surfacing-Schritte (offen):**
1. Ein benanntes **`RuntimeConfig::multi_robot()`**-Profil (Domain-
   Isolation + Unicast-Peers + Interface-Scope) analog `ros_defaults()`.
2. Routed-WAN-Topologie-Doku (Peer-Listen über Subnetze; Bridge als
   WAN-Relay). 3. Optionaler eingebauter Discovery-Server (statt
   per-Host-Peer-Liste) für >N-Roboter-Flotten.

## C7 — Security-Setup-Bürde

**Vorhanden (code-belegt):**
- **`SecurityProfile`/`SecurityProfileConfig`** (`security-runtime`)
  bündelt das XML/Cert-Zeremoniell in EIN Objekt: `identity_ca_pem`,
  `permissions_ca_pem`, `governance.p7s`, `permissions.p7s` werden
  **CMS-verifiziert + geparst** (statt manuellem Multi-File-Wiring).
- **FFI `runtime_create_secure`** + `SecurityProfile` (live 2026-05-27,
  codepit 4×4 secured-Matrix) — secured Participant in einem Call.
- Volle DDS-Security 1.2 + cross-vendor secured-Interop.

**Surfacing-Schritte (offen):**
1. **„secure by default"-Launch-Integration**: ein ROS-Launch-/Env-Pfad
   (`ZERODDS_SECURITY_DIR=<enclave>`), der `SecurityProfile` aus einem
   SROS2-Enclave-Verzeichnis lädt — eine Zeile statt per-Participant-XML.
2. Enclave-Auto-Discovery (governance/permissions aus Standard-Pfaden).
3. Doku „weniger Cert-Zeremonie" mit Enclave→SecurityProfile-Mapping.

## C8 — Tooling / Monitoring / Introspektion

**Vorhanden (code-belegt):**
- **`zerodds-monitor`**: Metric-Registry + **Prometheus-Text-Exporter** +
  W3C-Trace-Context + Span-Schema. **Discovery-unabhängig** — die Metriken
  sind lokal (Endpoint-Counts, Match-Status, QoS-Reject-Counter), nicht
  vom DDS-Graph abhängig → funktionieren AUCH mit Multicast-aus/
  Discovery-Server (genau der C8-Schmerz „fix one thing, break another").
- **`inspect-endpoint`** (feature-gated Tap) für Wire-Level-Debug.
- **C2-`qos_check`-CLI** (diese Session) = statische QoS-Mismatch-
  Diagnose ahead-of-time (der C8-Punkt „QoS-Mismatch nur Laufzeit").
- Das C2-laute-Reject-Logging (`qos.incompatible.*`) macht stille
  Mismatches sichtbar.

**Surfacing-Schritte (offen):**
1. Ein **`zerodds graph`/`zerodds doctor`**-CLI, das die monitor-Metriken
   + den lokalen Endpoint-/Match-Zustand dumpt — funktioniert ohne
   DDS-Multicast-Introspektion (liest den lokalen Participant-Zustand).
2. RMW-Konsistenz-Check (`RMW_IMPLEMENTATION`-Validierung).
3. Prometheus-Endpoint im rmw-Layer exponieren.

## Fazit

Für C6/C7/C8 ist **kein Kern-Feature offen** — Discovery-Server-Fundament
(C1-Unicast), Security-Bündelung (`SecurityProfile`) und discovery-
unabhängige Observability (`zerodds-monitor`) existieren. Die offenen
Punkte sind **ROS-facing Surfacing** (benannte Profile, Launch-Integration,
CLIs) — tractable, aber kein struktureller Schmerzpunkt mehr.
