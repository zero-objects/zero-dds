# WP 4H-j — Delegation-Track (Stufenplan)

> **Status:** Ready-to-start, 2026-04-25
> **Architektur-Referenz:** `docs/architecture/09_delegation.md`
> **Voraussetzungen:** WP 4H-a bis 4H-i abgeschlossen (Heterogeneous-
>   Security-Policy + Governance-XML-Peer-Classes + PKI↔Crypto-
>   Integration)
> **Geschätzter Umfang:** ~2100 LOC, 8 Sub-WPs, 10 Arbeitstage

## Einstiegs-Briefing für die frische Session

Lies in dieser Reihenfolge:
1. Dieses Dokument (Stufenplan + Definition-of-Done).
2. `docs/architecture/09_delegation.md` (vollständiges Datenmodell +
   Trust-Policies + Governance-XML-Hybrid-Struktur).
3. `docs/architecture/08_heterogeneous_security.md` §9 (Referenz-
   Eintrag — Delegation ist dort aus der Nicht-Ziel-Liste
   herausgehoben worden).
4. `docs/plans/wp-4H-heterogeneous-security-plan.md` (vorheriger
   Track, dessen Peer-Class-Matching in Stufe j-d erweitert wird).
5. Aktueller Stand:
   * `crates/security-pki/src/plugin.rs` — Handshake + X.509
   * `crates/security-pki/tests/pki_crypto_integration.rs` — WP 4H-i
     E2E-Referenz (`SharedSecretProvider`-Bridge)
   * `crates/security-permissions/src/governance.rs` — PeerClass +
     PeerClassMatch (hier kommen die neuen Attribute `delegated_by`,
     `delegation_profile`, `max_delegation_depth` dazu)

**Kontext in drei Sätzen:**
Vehicle-Mesh im Stern (Wanne-Gateway) oder Doppelstern (Wanne+Turm)
hat Edge-Peers ohne eigenen Cert, die über ein Gateway repräsentiert
werden. Externe Peers (C4I, V2V-via-C4I-Broker) kennen nur das
Gateway-Cert und akzeptieren Edge-Samples über kryptographische
Delegation-Chains. Ein Delegation-Profile in Governance-XML
definiert pro Use-Case (Vehicle-intern, C4I-Relay, Federated-V2V)
Trust-Policy, Chain-Tiefe, Signatur-Algorithmus und erlaubte
Delegators.

## Sub-WP-Übersicht

| WP    | Thema                                                   | LOC-Schätzung | Abhängigkeit |
|-------|---------------------------------------------------------|--------------:|--------------|
| 4H-j-a | `DelegationLink` + `DelegationChain` + Sign/Verify     | ~350          | —            |
| 4H-j-b | Chain-Validation + Scope-Intersection                  | ~200          | j-a          |
| 4H-j-c | SPDP-Propagation (Wire-Format)                         | ~200          | j-a          |
| 4H-j-d | PeerClassMatch-Extensions + Profile-Referenz           | ~200          | j-b, j-c     |
| 4H-j-e | GatewayBridge-Helper + Sub-Gateway-Chaining            | ~300          | j-a..d       |
| 4H-j-f | Static + Ephemeral Edge-Identity-Config                | ~200          | j-e          |
| 4H-j-g | E2E-Test Wanne+Turm-Doppelstern + C4I                  | ~400          | j-a..f       |
| 4H-j-h | Governance-XML Hybrid-Profile-Parser + Interop-Tests   | ~250          | j-b          |

## Stufenplan

### Stufe j-a — `DelegationLink`/`DelegationChain` + Sign/Verify (1–2 Tage)

**Deliverables:**
1. Neues Modul `crates/security-pki/src/delegation.rs`:
   * `DelegationLink { delegator_guid, delegatee_guid,
     allowed_topic_patterns, allowed_partition_patterns,
     not_before, not_after, signature }`
   * `DelegationChain { origin_guid, links: Vec<DelegationLink> }`
   * `SignatureAlgorithm` enum (Ecdsa256, Ecdsa384, RsaPss, Ed25519)
   * `DelegationLink::sign(signing_key, algo) -> Result<(), Error>`
   * `DelegationLink::verify(verifying_cert, algo) -> Result<(), Error>`
2. Serialisierung: deterministische Byte-Reihenfolge für
   Signing-Input. Layout:
   ```
   magic(8) = b"ZERODDSD"
   delegator_guid(16)
   delegatee_guid(16)
   not_before(i64_be)
   not_after(i64_be)
   n_topic_patterns(u32_be)
   [ len(u32_be) + utf8_bytes ] * n_topic_patterns
   n_partition_patterns(u32_be)
   [ len(u32_be) + utf8_bytes ] * n_partition_patterns
   ```
3. Verwendung `rustls-webpki` + `ring::signature` für Sign/Verify
   aller 4 SignatureAlgorithm-Werte.
4. Tests:
   * Roundtrip Sign → Verify für alle 4 Algorithmen
   * Tampered Byte im Middle → Verify-Fail
   * Mismatched Cert (wrong issuer) → Verify-Fail
   * Serialisierung ist deterministisch (zweimal gleich)

**Definition-of-Done:**
- [ ] `cargo test -p zerodds-security-pki delegation::` grün
- [ ] Branch-Coverage ≥ 95 % auf `delegation.rs`
- [ ] Doc-comments auf jedem Public-Item
- [ ] `PkiAuthenticationPlugin` bleibt unverändert (Isolation)

### Stufe j-b — Chain-Validation + Scope-Intersection (1 Tag)

**Deliverables:**
1. Neues Modul `crates/security-permissions/src/delegation_check.rs`:
   * `validate_chain(chain: &DelegationChain, trust_anchor:
     &TrustAnchor, profile: &DelegationProfile, now: i64) ->
     Result<ValidatedChain, DelegationError>`
   * Führt alle 7 Checks aus §6 des Arch-Docs durch:
     1. Chain-Kontinuität
     2. Origin-Match
     3. Trust-Anchor (`trust_policy`-abhängig)
     4. Signatur-Kette (jeder Link gegen vorigen Delegator)
     5. Zeitfenster pro Link
     6. max_chain_depth gegen Profile
     7. Scope-Intersection aller Topic-/Partition-Patterns
2. `ValidatedChain { effective_topic_patterns, effective_partition_patterns,
   origin_guid, chain_depth }` — Ausgabe für den Aufrufer.
3. `scope_intersect(a: &[String], b: &[String]) -> Vec<String>` —
   Wildcard-Pattern-Schnitt via `topic_match`.
4. Tests:
   * 1-Hop-Chain valid → ValidatedChain OK
   * 2-Hop-Chain mit mittlerem Scope-Enger → Effektiver Scope engste
   * Tiefe > max_depth → `ChainTooDeep`
   * Expired Link → `LinkExpired`
   * Broken Chain (delegatee != next.delegator) → `ChainBroken`
   * Unknown Trust-Anchor → `UntrustedDelegator`
   * Alle 4 `trust_policy`-Modi in eigenem Test

**Definition-of-Done:**
- [ ] 15+ Unit-Tests grün
- [ ] Coverage ≥ 95 % auf `delegation_check.rs`
- [ ] Alle 4 Trust-Policy-Modi explizit getestet
- [ ] Integration in `GovernancePolicyEngine::accept_peer` ist
      **noch nicht** drin — kommt in j-d

### Stufe j-c — SPDP-Propagation (1–2 Tage)

**Deliverables:**
1. `crates/security-runtime/src/caps.rs` — `PeerCapabilities`
   erweitern um `delegation_chain: Option<DelegationChain>`.
2. `crates/security-runtime/src/caps_wire.rs` — neue
   Property-Keys:
   * `zerodds.sec.delegation_chain` (base64-encoded binary blob)
   * Format: `u8 version=1 | u16 n_links | [DelegationLink]*`
   * DoS-Cap: maximale Blob-Größe = 8 KiB
3. Encode/Decode mit `DelegationLink::serialize/deserialize`.
4. Tests:
   * Roundtrip mit 0/1/2/3 Links
   * Malformed Blob → parse_peer_caps liefert `delegation_chain = None`
   * Blob > Cap → reject

**Definition-of-Done:**
- [ ] Bestehender `spdp_caps_e2e.rs`-Test bleibt grün
- [ ] Neuer Test `delegation_chain_roundtrip_via_spdp` grün
- [ ] DoS-Cap unit-getestet

### Stufe j-d — PeerClassMatch-Extensions + Profile-Referenz (1 Tag)

**Deliverables:**
1. `crates/security-permissions/src/governance.rs`:
   * `PeerClassMatch` um `delegation_profile: Option<String>` erweitern
   * `PeerClass`-Resolve: wenn `delegation_profile.is_some()`,
     lookup in `Governance::delegation_profiles`, sonst
     direkt-auth-Pfad.
2. `peer_matches_class` in `security-runtime/src/peer_class.rs`
   erweitern:
   * Wenn `delegation_profile` gesetzt:
     a) Peer MUSS `caps.delegation_chain.is_some()`
     b) Chain MUSS gegen Profile validieren (`validate_chain`)
     c) Effektive Scope-Patterns im `ValidatedChain` MÜSSEN Topic
        des Writers/Readers enthalten
3. `GovernancePolicyEngine::accept_peer` ruft diese Erweiterung auf.
4. Tests in `security-runtime/src/engine.rs` (analog zu
   `hetero_dod_*`-Matrix):
   * `hetero_dod_legacy_via_gateway_accepts_with_valid_chain`
   * `hetero_dod_rogue_peer_with_invalid_chain_rejects`
   * `hetero_dod_chain_depth_exceeds_profile_rejects`
   * `hetero_dod_peer_without_profile_reference_direct_auth_works`

**Definition-of-Done:**
- [ ] 8+ neue Tests grün
- [ ] Bestehende `hetero_dod_*`-Tests aus WP 4H-h bleiben grün
- [ ] Coverage ≥ 95 % auf neuen Pfaden

### Stufe j-e — GatewayBridge-Helper (2 Tage)

**Deliverables:**
1. Neues Modul `crates/security-runtime/src/gateway_bridge.rs`:
   * `GatewayBridge { gateway_cert, gateway_key, profile,
     active_delegations: BTreeMap<[u8;16], DelegationLink> }`
   * `GatewayBridge::delegate_for(edge_guid, topic_patterns,
     duration) -> Result<DelegationLink>`
     — erzeugt + signiert neue Delegation.
   * `GatewayBridge::revoke_delegation(edge_guid)` — entfernt aus Map,
     triggert Revocation-Announce beim nächsten SPDP.
   * `GatewayBridge::chain_for(edge_guid) -> Option<DelegationChain>`
     — 1-Hop-Chain für direkten Edge.
2. **Sub-Gateway-Chaining**: wenn das Gateway selbst als Delegatee
   einer höheren Ebene läuft (Turm-GW unter Wanne-GW), nimmt
   `chain_for` den bereits bestehenden Upstream-Link und hängt den
   eigenen dran:
   * `GatewayBridge::with_upstream(upstream_chain: DelegationChain)`
   * `chain_for(edge_guid)` liefert dann n+1-Hop-Chain.
3. Tests:
   * 1-Hop: Wanne-GW delegiert für Lidar-A
   * 2-Hop: Turm-GW delegiert für Turm-Sensor; Wanne-GW nimmt das
     als Upstream + delegiert für den selben Turm-Sensor weiter.
     → 2-Link-Chain mit Wanne-GW als letztem Link.
   * Revocation-Event wird in Announce-Property serialisiert.

**Definition-of-Done:**
- [ ] 10+ Unit-Tests grün
- [ ] Chain-Validation aus j-b nimmt eine Bridge-erzeugte
      2-Hop-Chain als valid an (End-to-End-Verifikation)
- [ ] Coverage ≥ 95 %

### Stufe j-f — Static + Ephemeral Edge-Identity-Config (1 Tag)

**Deliverables:**
1. `crates/security-permissions/src/governance.rs`:
   * `EdgeIdentityConfig { name: String, mode: EdgeIdentityMode,
     guid_prefix: Option<[u8; 12]>, ephemeral_lifetime_seconds:
     Option<u32> }`
   * `EdgeIdentityMode { Static, Ephemeral }`
2. Governance-XML-Parser-Erweiterung:
   ```xml
   <zerodds:edge_identities default_mode="static">
     <edge name="lidar-A" guid_prefix="0102...0a" />
     <edge name="turm-imu" mode="ephemeral" lifetime_seconds="300" />
   </zerodds:edge_identities>
   ```
3. `GatewayBridge::rotate_ephemerals(now)` — für alle
   `Ephemeral`-Edges, deren `last_rotate + lifetime < now`:
   * Neue GuidPrefix ziehen (ChaCha20-RNG)
   * Neue `DelegationLink` signieren
   * Alte wird via Revocation-Liste ausgeschleust
4. Tests:
   * XML-Parse-Tests (Static, Ephemeral, Mixed)
   * `rotate_ephemerals` produziert frische Prefixe
   * Static-Edges bleiben stabil über mehrere Ticks

**Definition-of-Done:**
- [ ] 6+ Tests grün
- [ ] Config-Doc-Beispiel in `09_delegation.md §5.3` kompiliert durch
      den Parser ohne Fehler

### Stufe j-g — E2E-Test Doppelstern + C4I (2 Tage)

**Deliverables:**
1. `crates/dcps/tests/delegation_vehicle_mesh_e2e.rs`:
   * Topologie: Wanne-GW + Turm-GW + 2 Turm-Sensoren + 1 Wanne-ECU
     + 1 C4I-Node
   * Alle DcpsRuntime-Instances starten, SPDP-Beacons fliegen
   * Wanne-GW hat Upstream-Link von Turm-GW (2-Hop für Turm-Sensoren)
   * C4I-Node ist konfiguriert mit `trust_policy=strict-delegated`
     + Profile-Referenz `c4i-via-wanne-gateway`
   * Wanne-ECU sendet Sample → C4I akzeptiert mit 1-Hop-Chain
   * Turm-Sensor sendet Sample → C4I akzeptiert mit 2-Hop-Chain
   * Rogue-Peer (no cert, fake chain) → C4I dropt mit Log-Event
2. Wire-Validation: Output von `tcpdump`-artigem Capture zeigt im
   SEDP-Announce den Delegation-Blob.
3. Sanity-Checks: Fremd-Vendor-Cyclone-Peer sieht Wanne-GW als
   normalen Participant (zerodds-NS ignoriert).

**Definition-of-Done:**
- [ ] 4+ E2E-Tests grün
- [ ] Capture-File mit Delegation-Blob im Test-Output
- [ ] Cyclone-Interop-Smoke: Wanne-GW sichtbar als Participant
      trotz zerodds:-Delegation-Property

### Stufe j-h — Governance-XML Hybrid-Profile-Parser + Interop (1 Tag)

**Deliverables:**
1. `crates/security-permissions/src/governance.rs`:
   * Neuer Typ `DelegationProfile`
   * Parser für `<zerodds:delegation_profiles>` (alle Felder aus
     Arch-Doc §7.1)
   * Parser-Warning bei unreferenced `<profile>` oder
     unknown `delegation_profile="..."`
2. `Governance`-Struct um `delegation_profiles: BTreeMap<String,
   DelegationProfile>` erweitern.
3. Alle 4 `trust_policy`-Modi + alle 4 `signature_algorithm`-Werte
   werden geparst.
4. Tests:
   * Parse-Roundtrip für vollständiges XML-Beispiel aus Arch-Doc §7.1
   * Unknown trust_policy → Default + Warning
   * Missing profile reference aus Peer-Class → Error
   * Circular profile-Referenz → abgelehnt (Path-Check)

**Definition-of-Done:**
- [ ] 8+ Tests grün
- [ ] `cargo clippy --workspace --all-targets --all-features
      -- -D warnings` clean
- [ ] `zerodds-lint check` 0/0
- [ ] Architektur-Doc §7.1 bleibt aktuell

## Gesamt-DoD (alle 8 Stufen)

- [ ] 8/8 Sub-WPs implementiert und committed auf `main`.
- [ ] `cargo test --workspace --all-features` grün.
- [ ] Neue Test-Count: mindestens +60 (Schätzung 8 pro Stufe außer
      j-f mit 6).
- [ ] Coverage: neue Delegation-Module ≥ 95 % Branch.
- [ ] Architektur-Doc `09_delegation.md` bleibt aktuell (Draft-Status
      bleibt v0.1 bis Review).
- [ ] Release-Plan §1.4 nennt WP 4H-j-a bis -h als **implementiert**.
- [ ] E2E-Demo: Doppelstern-Fahrzeug mit Wanne+Turm + 5 Edge-Peers
      + 1 C4I-Node, Bytes-on-Wire beweisen Multi-Hop-Delegation +
      Scope-Kaskadierung.

## Risiken und Mitigationen

| Risiko                                                       | Mitigation                                                      |
|--------------------------------------------------------------|-----------------------------------------------------------------|
| Kaskadierende Revocation-Stürme bei Gateway-Ausfall          | Implicit-Revocation via lease_duration-Timeout; explizite Revocation rate-limited (max 10/sec per Gateway) |
| Replay-Attacks auf Static-Edge-Identities                    | Ephemeral-Mode als Opt-in; Nonce-Counter in jedem Sample (bestehender AES-GCM-Mechanismus) |
| Chain-Blob-Explosion in SPDP (Dutzende Edges × Chain-Links)  | DoS-Cap 8 KiB pro Blob; Chain-Compression via SEDP-Endpoint-Announce (j-g Stretch-Goal) |
| Signatur-Algorithmus-Drift zwischen Gateways                 | Profile-Reference bricht bei Mismatch → harter Fail + Log-Event |
| Code-Bloat im `security-permissions`-Crate                   | `delegation_check.rs` + Parser in eigene Modul-Datei; kein Re-Export außer top-level Traits |

## Erste Session-Aktion

1. Branch anlegen: `git checkout -b feat/wp-4H-j-delegation`
2. Stufe j-a beginnen: `crates/security-pki/src/delegation.rs` anlegen,
   `DelegationLink`-Typ definieren, Sign/Verify mit 4 Algorithmen.
3. **Pro Stufe**: eigener Commit, Pre-Commit-Check, lokaler Test-Run.
   Kein 8-Stufen-Monster-Commit.
4. Am Ende von Stufe j-g: Live-Interop-Smoke gegen Cyclone auf
   `ssh llvm@llvm` (muss Wanne-GW als Participant sehen, selbst wenn
   Delegation-Property ignoriert wird).
