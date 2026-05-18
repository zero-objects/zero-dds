# WP 4H — Heterogeneous-Security-Track (Stufenplan)

> **Status:** Ready-to-start, 2026-04-24
> **Architektur-Referenz:** `docs/architecture/08_heterogeneous_security.md`
> **Vorausetzungen:** WP 4.1–4.6 + 4.4-b.4 abgeschlossen (Security-Gate
> bereits in `zerodds-dcps::runtime`-Hot-Path integriert)
> **Geschaetzter Umfang:** 3–4 Wochen, 8 Sub-WPs

## Einstiegs-Briefing fuer die frische Session

Lies in dieser Reihenfolge:
1. Dieses Dokument (Stufenplan + Definition-of-Done).
2. `docs/architecture/08_heterogeneous_security.md` (Datenmodelle).
3. `docs/plans/release-plan-v1.2-to-v2.0.md` §1.4 (v1.4-Security-Kontext).
4. `crates/security-runtime/src/shared.rs` (aktueller Stand vom Gate).
5. `crates/dcps/src/runtime.rs` — Zeilen um `secure_outbound_bytes` /
   `secure_inbound_bytes` (die 6 Inject-Punkte).

**Kontext in drei Saetzen:**
Der aktuelle `SharedSecurityGate` macht ein-Policy-pro-Participant.
Fuer System-of-Systems (Vehicle, Tactical) brauchen wir feiner
gekoerntes Policy-Management pro `(peer, topic, interface)`. Alle
v1.4-Security-Crates sind da — WP 4H refaktoriert den Gate zum
`PolicyEngine`-Default-Impl und baut das Writer/Reader-Dispatch um.

## Sub-WP-Uebersicht

| WP    | Thema                                    | LOC-Schaetzung | Abhaengigkeit |
|-------|------------------------------------------|----------------|----------------|
| 4H-a  | `PolicyEngine`-Trait + `PeerCapabilities` | ~400          | —              |
| 4H-b  | SPDP-Capability-Advertisement/-Parse     | ~300           | 4H-a           |
| 4H-c  | SEDP-Capability-Propagation pro Endpoint | ~200           | 4H-b           |
| 4H-d  | Writer-Side Per-Reader-Serializer        | ~500           | 4H-c           |
| 4H-e  | Reader-Side Per-Writer-Validator         | ~300           | 4H-c           |
| 4H-f  | Interface-Routing (Multi-Socket-Binding) | ~400           | 4H-d,e         |
| 4H-g  | Receiver-Specific-MACs (SEC_POSTFIX)     | ~250           | 4H-d           |
| 4H-h  | Governance-XML `<peer_classes>`-Parser   | ~350           | 4H-a..f        |

## Stufenplan

### Stufe 0 — Vorbereitung (0,5 Tag)

**Ziel:** Baseline festhalten, Roadmap-Doc updaten.

* `cargo test --workspace -- --test-threads=1` muss gruen sein.
* `cargo test -p zerodds-security-runtime` muss 24+ Tests zeigen.
* Release-Plan um WP 4H-Track ergaenzen (unter §1.4 Security-Track).
* Branch: `feat/wp-4H-heterogeneous-security`.

### Stufe 1 — `PolicyEngine`-Trait + `PeerCapabilities` (WP 4H-a, 1-2 Tage)

**Deliverables:**
1. Neues Modul `crates/security-runtime/src/policy.rs` mit:
   * `trait PolicyEngine` (wie in Architektur-Doc §3.1).
   * `struct PolicyDecision { protection, suite, drop }`.
   * `enum NetInterface { Loopback, LocalHost(_), LocalSubnet(_), Wan, Named(_) }`.
   * `fn classify_interface(locator: &Locator, config: &InterfaceConfig) -> NetInterface`.
2. Neues Modul `crates/security-runtime/src/caps.rs`:
   * `struct PeerCapabilities` (alle Felder aus Architektur-Doc §3.2).
   * `struct PeerCache { inner: BTreeMap<PeerKey, PeerCapabilities> }` mit
     `insert`, `get`, `update_partial`, `forget`.
3. Default-Impl `GovernancePolicyEngine` die das bestehende Governance-
   Verhalten nachbildet (1:1 Kompatibilitaet zu Stand v1.4).
4. Tests:
   * policy engine basic decision paths
   * peer cache insert/get/update/forget
   * interface classifier unit tests (loopback 127.x, 10.0.0.0/24, 0.0.0.0/0)
5. **Bestehender `SharedSecurityGate` bleibt unveraendert** — der wird
   in Stufe 6 umgebogen.

**Definition-of-Done:**
- [ ] `cargo test -p zerodds-security-runtime` alle gruen
- [ ] Branch-Coverage ≥ 95 % auf `policy.rs` + `caps.rs`
- [ ] Doc-comment auf jedem Public-Item
- [ ] `GovernancePolicyEngine` hat End-to-End-Test der zum bestehenden
      `SharedSecurityGate`-Verhalten byte-identisch ist

### Stufe 2 — SPDP-Capability-Ads (WP 4H-b, 1-2 Tage)

**Deliverables:**
1. SPDP-Property-Builder in `crates/discovery/src/spdp.rs`:
   * `fn advertise_security_caps(props: &mut PropertyList, caps: &LocalCaps)`.
   * Properties:
     * `dds.sec.auth.plugin_class`
     * `dds.sec.access.plugin_class`
     * `dds.sec.crypto.plugin_class`
     * `zerodds.sec.supported_suites` (CSV: "AES_128_GCM,AES_256_GCM,HMAC_SHA256")
     * `zerodds.sec.offered_protection` (NONE|SIGN|ENCRYPT)
2. SPDP-Property-Parser:
   * `fn parse_peer_caps(props: &PropertyList) -> PeerCapabilities`
3. Integration ins `SpdpBeacon`: beim Start/Restart werden die
   lokalen Caps ins SPDP-Token gepackt. Beim Empfang werden sie in
   den `PeerCache` geschrieben.

**Definition-of-Done:**
- [ ] SPDP-Roundtrip-Test: Participant A annonciert, Participant B
      sieht die Caps im PeerCache.
- [ ] Legacy-Peer (ohne Security-Properties) landet als
      `auth_plugin_class: None` im Cache — kein Drop.
- [ ] Cyclone-Interop-Test: unser SPDP wird von Cyclone still akzeptiert
      (extra Properties ignoriert).

### Stufe 3 — SEDP-Endpoint-Caps (WP 4H-c, 1 Tag)

**Deliverables:**
1. Analog zu SPDP: `crates/rtps/src/publication_data.rs` +
   `subscription_data.rs` bekommen optionale Security-PIDs
   (`PID_ENDPOINT_SECURITY_INFO`, Spec §9.4.2.4).
2. `DiscoveredEndpoint`-Typen im `discovery`-Crate tragen
   `security_info: Option<EndpointSecurityInfo>`.
3. Matching-Logic: beim `run_matching_pass` werden die Caps an die
   `PolicyEngine::accept_peer`-Check uebergeben.

**Definition-of-Done:**
- [ ] Writer mit `protection=ENCRYPT`, Reader ohne Security-Plugin:
      Match wird abgelehnt.
- [ ] Writer `protection=SIGN`, Reader `protection=ENCRYPT`: Match
      passiert mit `ENCRYPT` (staerkerer Wert gewinnt).

### Stufe 4 — Writer-Side Per-Reader-Serializer (WP 4H-d, 3-4 Tage)

**Der groesste Schritt.** Statt `user_unicast.send(bytes, all_targets)`
wird pro matched Reader einzeln serialisiert.

**Deliverables:**
1. `crates/dcps/src/runtime.rs`:
   * `UserWriterSlot` bekommt `matched_readers: BTreeMap<PeerKey, ReaderInfo>`.
   * Writer-Tick-Loop iteriert ueber matched_readers, ruft
     `policy.outbound_decision` pro Reader, serialisiert entsprechend.
2. `SharedSecurityGate` bekommt neue API `transform_for_peer(
   peer_key, bytes, decision) -> Vec<u8>`.
3. Performance-Guard: Bench vorher/nachher — nur heterogene Policy
   verursacht Fan-Out, homogener Fall bleibt gleich.

**Definition-of-Done:**
- [ ] Integrations-Test: 1 Writer, 3 Reader (Legacy/Fast/Secure),
      alle bekommen unterschiedlich geschuetzte Wire-Pakete.
- [ ] Homogener Fall (alle Reader secure): ein Paket, kein Fan-Out.
- [ ] `cargo llvm-cov` zeigt Coverage-Delta < 3 % (Regression-OK).

### Stufe 5 — Reader-Side Per-Writer-Validator (WP 4H-e, 1-2 Tage)

**Deliverables:**
1. Reader-Tick-Inbound-Pfad: pro eingehendes Paket wird
   `policy.inbound_decision` gerufen mit der extrahierten
   Source-GUID + Interface-Klasse.
2. Bei Policy-Violation: Paket droppen + `LoggingPlugin`-Event
   (Level `Warning`).
3. Bei missing-caps (unauth Peer auf protected Topic): drop mit
   `Error`.

**Definition-of-Done:**
- [ ] Tampering-Test: Writer schickt plain, Policy erwartet ENCRYPT
      → Reader droppt, Event im Log.
- [ ] Legacy-Peer kann weiter mit Reader reden, wenn Governance
      `allow_unauthenticated_participants=true` setzt.

### Stufe 6 — Interface-Routing (WP 4H-f, 2-3 Tage)

**Deliverables:**
1. `RuntimeConfig::interfaces: Vec<InterfaceBinding>` — jedes Binding
   haelt einen eigenen UDP-Socket + eine `NetInterface`-Klasse.
2. Outbound-Routing:
   * Pro Ziel-Locator → Interface-Lookup → passender Socket.
   * Falls kein Match → Default-Interface.
3. Inbound-Routing:
   * Jeder Socket hat einen eigenen Reader-Thread (bzw. `epoll`/
     `kqueue`-Integration, je nach bestehender Transport-Infra).
   * `NetInterface` wird mit dem Datagramm ins Policy-Lookup gegeben.

**Definition-of-Done:**
- [ ] Config-Beispiel: `eth0` (WAN) secure + `lo` loopback plain.
- [ ] UDP-Sniffer-Test: Bytes auf `lo` sind plaintext (keine
      SEC_PREFIX-Submessage), Bytes auf `eth0` sind SRTPS-wrapped.
- [ ] Default-Interface-Fallback greift bei unbekannter IP.

### Stufe 7 — Receiver-Specific-MACs (WP 4H-g, 2 Tage)

**Optimierung fuer den homogenen-Suite-Fall.**

**Deliverables:**
1. `crates/security-rtps/src/codec.rs` erweitern um
   Multi-MAC-`SEC_POSTFIX`-Encoding (Spec §7.3.6.3).
2. Crypto-Plugin-Trait: neue Methode
   `encrypt_submessage_multi(local, remote_list, plain)` — liefert
   ein Ciphertext + N MACs.
3. Writer-Side: wenn alle matched Reader die gleiche Suite nutzen
   aber unterschiedliche Keys → Multi-MAC statt Multi-Ciphertext.

**Definition-of-Done:**
- [ ] Test: 3 Reader mit gleicher Suite, unterschiedlichen Tokens —
      Writer produziert ein Ciphertext + 3 MACs.
- [ ] Reader validiert seinen spezifischen MAC korrekt.

### Stufe 8 — Governance-XML `<peer_classes>` (WP 4H-h, 2 Tage)

**Letzte Stufe — macht's Nutzer-konfigurierbar.**

**Deliverables:**
1. `crates/security-permissions/src/governance.rs` erweitern um
   `zerodds:peer_classes` + `zerodds:interface_bindings` (Namespace-
   scoped).
2. `PeerClass`-Matching-Engine: CN-Pattern, auth_plugin_class-Check,
   suite-Require.
3. Integration in `GovernancePolicyEngine`: bei Outbound-Decision
   wird der Peer-Class-Resolve aus den `PeerCapabilities` gemacht.

**Definition-of-Done:**
- [ ] Governance-Beispiel mit 4 Peer-Classes (Legacy/Fast/Secure/HA)
      wird korrekt geparst.
- [ ] `GovernancePolicyEngine` liefert pro-Peer-Class-Protection-
      Level.
- [ ] Vendor-Interop: Cyclone sieht `zerodds:`-Namespace, ignoriert
      ihn still — faellt auf `<rtps_protection_kind>` zurueck.

## Gesamt-DoD (Alle 8 Stufen)

- [ ] 8/8 Sub-WPs implementiert und committed auf `main`.
- [ ] `cargo test --workspace -- --test-threads=1` gruen.
- [ ] Neue Test-Count: mindestens +80 (Schaetzung 15 pro Stufe ausser
      Stufe 0).
- [ ] Coverage: neue `zerodds-security-runtime`-Module ≥ 95 % Branch.
- [ ] Architektur-Doc `08_heterogeneous_security.md` bleibt aktuell.
- [ ] Release-Plan §1.4 nennt WP 4H-a bis -h als **implementiert**.
- [ ] Ein End-to-End-Demo-Test: 4 Runtimes (Legacy, Fast, Secure, HA)
      auf gleicher Domain, Writer sendet, jeder Reader empfaengt
      korrekt — Bytes-on-Wire beweisen Heterogenitaet.

## Risiken und Mitigationen

| Risiko                                              | Mitigation                              |
|-----------------------------------------------------|-----------------------------------------|
| Performance-Regression durch Per-Reader-Serialize   | Bench-Harness in Stufe 4; homogener Fall wird erkannt und fuehrt zu single-send |
| Vendor-Interop-Brueche                              | Cyclone-/Fast-Roundtrip-Tests in jeder Stufe |
| Plaintext-Leak durch Policy-Fehler                  | Default-Policy ist **restriktivste** Class; Fuzz-Test |
| Coverage-Einbruch                                   | Pro Stufe Coverage-Check im Pre-Commit |
| Code-Bloat im runtime.rs                            | Extract in `crates/dcps/src/security_dispatch.rs` ab Stufe 4 |

## Erste Session-Aktion

1. Branch anlegen: `git checkout -b feat/wp-4H-heterogeneous-security`
2. Stufe 0 abhaken (Baseline-Check).
3. Mit Stufe 1 beginnen: `crates/security-runtime/src/policy.rs` anlegen,
   Trait definieren, erste Tests schreiben.
4. **Pro Stufe**: eigener Commit, Pre-Commit-Check, Push. Nicht
   acht-Stufen-Monster-Commit.
