# Delegation — Gateway/Bridge-Identity für Vehicle-Mesh + Loose-Coupled Systems

> **Status:** Draft v0.1 (2026-04-25)
> **Abhängigkeiten:** `08_heterogeneous_security.md` (Peer-Classes, Interface-Routing),
>   `02_architecture.md §3` (Transport-Schichten), WP 4H-i (PKI↔Crypto-Integration)

## 1 Motivation

Im Heterogeneous-Track (WP 4H) haben wir Peer-Klassen für einen flachen
Participant-Raum gebaut. In realen Vehicle-Netzen ist der Raum aber
**hierarchisch**: ein zentrales Gateway (manchmal zwei — Wanne+Turm)
bündelt Dutzende von Sensoren, ECUs und Subsystemen, die selber
keinen eigenen Security-Stack haben. Gegen die Außenwelt tritt das
Fahrzeug als **ein Participant** auf, intern ist es ein Stern oder
Doppelstern mit Security-Mix.

```
                    (Außenwelt)
                       ▲
            C4I ──────►│◄──── andere Fahrzeuge (V2V via C4I-Broker)
                       │
    ═══════════════════╪═══════════════════  Fahrzeug-Grenze
                       │
                 Wanne-Gateway  ◄──► Turm-Gateway      ← 1 oder 2 Zentren
                  ┌────┴────┐         ┌────┴────┐
                  │         │         │         │
             Sensor-ECU  Fahrer-HMI   Waffenstation  Turm-Sensor
             (legacy)    (secure)     (secure)       (mix)
```

Delegation ist der Mechanismus, mit dem Gateways die **Identity** von
Edge-Peers nach außen vertreten, ohne dass diese Edge-Peers selbst
PKI-Material besitzen müssen.

## 2 Abgrenzung: Security-Layer vs. Bridge-Layer

Delegation adressiert die **Security-Attribution** (wer hat's
geschrieben, wer darf's lesen), nicht das Routing:

| Layer | Aufgabe | Kennt Delegation? |
|-------|---------|-------------------|
| **Bridge-Layer** | Transportiert Samples zwischen Domains/Hosts/Fahrzeugen; reagiert auf QoS, Topic-Routen, Partitions | Nein — agnostisch, leitet alles weiter |
| **Security-Layer** | Entscheidet ob ein Peer authentisch ist, ob sein Sample akzeptiert wird, mit welcher Protection-Klasse | Ja — prüft Delegation-Chain, Scope, Revocation |

**Regel:** Der Bridge-Layer muss jede Nachricht transportieren, die
der Security-Layer durchgelassen hat — egal wie viele Delegation-Hops
drin sind. **Ausnahme:** nur explizit Spec-verbotene Kombinationen
(z.B. OMG verbietet Secure-Envelopes über unverschlüsselte
Transport-Hops wenn die Domain `rtps_protection_kind=ENCRYPT` verlangt).

## 3 Use-Cases

### 3.1 Fahrzeug-intern (Stern, 1 Hop)

```
Sensor-ECU  ──► Wanne-Gateway  ──►  andere Fahrzeug-Subsysteme
  (legacy,       (delegiert für          (lesen "von lidar-A
   no cert)      lidar-A, radar-B…)       via wanne-gateway")
```

### 3.2 Fahrzeug-intern (Doppelstern, 2 Hops)

```
Turm-Sensor ──► Turm-Gateway ──► Wanne-Gateway ──► Fahrer-HMI
  (legacy)       (delegiert)      (re-delegiert      (liest mit
                                   für Turm-Sensor    2-Hop-Chain)
                                   über Turm-GW)
```

Die Wanne-Gateway-Delegation ist eine **Re-Delegation**: sie
bestätigt die Turm-Gateway-Delegation und fügt sich selbst als
nächster Hop an.

### 3.3 Fahrzeug → C4I (2–3 Hops)

```
Turm-Sensor ──► Turm-GW ──► Wanne-GW ──► C4I-Node
                                        (verifiziert 3-Hop-Chain
                                         gegen Fleet-Trust-Anchor)
```

### 3.4 V2V via C4I-Broker (Transport-Sonderfall)

Lateral-Kommunikation zwischen Fahrzeugen läuft im spezifischen
Scope **nicht als DDS-Direkt-Peer-Link**, sondern als Store-&-Forward
über C4I: Sample wird in SMTP-Datagrams verpackt, an C4I gesendet,
C4I reicht an Ziel-Fahrzeug weiter, dort re-assembliert. Der
Security-Layer sieht nur den lokalen C4I-Link, nicht das andere
Fahrzeug direkt.

Für **generelle lose-gekoppelte Systeme** außerhalb dieses SMTP-
Szenarios (später): Cross-Anchor-Federation — ein Fahrzeug vertraut
dem anderen, wenn dessen Gateway-Cert von einer gemeinsamen Fleet-CA
signiert ist. Siehe §9 "Zukünftige Erweiterungen".

## 4 Trust-Modell

### 4.1 Vertikale Trust-Kette

```
Fleet-Root-CA
    │
    ├── Vehicle-CA-Chassis-42
    │       │
    │       ├── Wanne-Gateway-Cert   ← in Trust-Anchor jedes externen Peers
    │       └── Turm-Gateway-Cert    ← ebenso
    │
    └── C4I-CA
            └── C4I-Node-Cert         ← andere Trust-Anchor-Ebene
```

* **Externe Peers** (C4I, andere Fahrzeuge) haben nur die **Gateway-
  Certs** im Trust-Store — nicht die Edge-Sensoren.
* **Gateway** signiert Delegation-Tokens für seine Edge-Peers mit
  seinem eigenen Gateway-Key. Jede Delegation ist kryptographisch
  auf den Gateway-Cert zurückführbar.
* **Edge-Peers** haben kein eigenes Key-Material — ihre Identity
  existiert nur **abgeleitet** aus der Gateway-Delegation.

### 4.2 Horizontale Peer-Class-Klassifikation

Edge-Peers sind zusätzlich in Peer-Classes (WP 4H-h) einsortiert:

| Edge-Typ | Peer-Class | Delegation? |
|----------|-----------|-------------|
| Legacy-ECU ohne Cert | `legacy` | Pflicht (nur als Gateway-Delegate erreichbar) |
| Fast-Subsystem mit Cert | `fast` | Optional (Gateway kann trotzdem für sie vertreten) |
| Hochsicheres C4I | `highassurance` | Kein Delegation — direkter Cert |

Die `PeerClassMatch`-Kriterien werden in Stufe j-d erweitert um
`delegated_by` / `max_delegation_depth`.

### 4.3 Trust-Policy-Modi

Die Trust-Anchor-Semantik ist **konfigurierbar pro Delegation-Profile**
(siehe §7.1). Vier Modi stehen zur Verfügung:

| Modus | Semantik |
|-------|----------|
| `gateway-only` | Nur Gateway-Certs im Trust-Anchor; Edge-Peers **nur** via Delegation akzeptiert. Standard für Fahrzeug → C4I. |
| `direct-or-delegated` | Edge mit eigenem Cert wird **direkt** akzeptiert (wenn im Trust-Store vorhanden); sonst Delegation-Pfad. Mixed-Netze mit einigen strong-Edges. |
| `federation` | **Cross-Anchor**: Trust-Store kennt mehrere Root-CAs, Peers aller bekannten Roots akzeptiert. Für V2V-generell / lose gekoppelte Systeme. |
| `strict-delegated` | Auch Gateway-Certs werden **nur** als Delegators akzeptiert, nie als direkte User-Peers. Für Fälle wo das Gateway selber keine Samples schreiben darf. |

Kombinierbar: ein Profile `c4i-relay-only` kann `strict-delegated`
sein während ein anderes Profile `internal-bridge` auf
`direct-or-delegated` läuft — innerhalb derselben Governance.

## 5 Datenmodell

### 5.1 `DelegationLink`

```rust
pub struct DelegationLink {
    /// Wer delegiert (in Trust-Anchor des Empfängers bekannt, oder
    /// selbst delegiert durch den nächsten Link).
    pub delegator_guid: [u8; 16],
    /// Wem wird delegiert.
    pub delegatee_guid: [u8; 16],
    /// Topic-Patterns die der Delegatee im Namen des Delegators
    /// bespielen darf. Wildcard-Semantik wie `topic_match` (WP 4.2-c).
    pub allowed_topic_patterns: Vec<String>,
    /// Partition-Patterns (Wildcard).
    pub allowed_partition_patterns: Vec<String>,
    /// Gültigkeitsfenster (Unix-Sekunden).
    pub not_before: i64,
    pub not_after: i64,
    /// Signatur vom Delegator-Cert über alle obigen Felder.
    pub signature: Vec<u8>,
}
```

### 5.2 `DelegationChain`

```rust
pub struct DelegationChain {
    /// Ursprünglicher Claim-Träger — der Peer, dessen Identity im
    /// Sample-Header steht. Muss === links[0].delegatee_guid.
    pub origin_guid: [u8; 16],
    /// Chain in Delegations-Reihenfolge. Erster Eintrag: Gateway,
    /// das die Identity des origin_guid bezeugt. Nachfolgende
    /// Einträge sind Re-Delegations.
    pub links: Vec<DelegationLink>,
}
```

### 5.3 Ephemeral vs. Static Edge-Identity

Default: **statischer GuidPrefix** aus Gateway-Config.

```xml
<!-- Gateway-Config (Beispiel) -->
<edge_identities>
  <edge name="lidar-A" guid_prefix="01020304050607080900000a" />
  <edge name="radar-B" guid_prefix="01020304050607080900000b" />
</edge_identities>
```

Optional: **Ephemeral-Identity** pro Boot-Zyklus oder Sample-Batch,
wenn Replay-Robustheit gefordert ist. Das Gateway erzeugt dann eine
Zufalls-GuidPrefix und signiert sie kurzlebig mit.

```xml
<edge_identities default_mode="static">
  <edge name="lidar-A" guid_prefix="0102030405060708090...a" />
  <edge name="radar-B" mode="ephemeral" lifetime_seconds="300" />
</edge_identities>
```

* **Static** — einfache Konfiguration, stabile Identity über Boot
  hinweg, anfällig für Replay wenn Message-Counter nicht mitgeschützt
  werden.
* **Ephemeral** — pro `lifetime_seconds` neue GuidPrefix + neue
  Delegation. Replay-Windows sind damit extrem eng. Mehr
  SPDP-Traffic als Preis.

## 6 Chain-Validation

Der Empfänger validiert eine ankommende Sample-Delegation so:

1. **Chain-Kontinuität**: Für alle `i`: `links[i].delegatee_guid == links[i+1].delegator_guid`.
2. **Origin-Match**: `chain.origin_guid == links[0].delegatee_guid`.
3. **Trust-Anchor**: Der Cert, mit dem `links[0].signature` erzeugt wurde, muss im Empfänger-Trust-Anchor liegen (d.h. der erste Delegator ist bekannt vertraut).
4. **Signatur-Kette**: Für alle `i > 0`: Signatur von `links[i]` muss vom Cert erzeugt sein, das zu `links[i-1].delegatee_guid` gehört (d.h. jeder nächste Delegator ist vom vorigen autorisiert).
5. **Zeitfenster**: Jedes `links[i]` mit `now ∈ [not_before, not_after]`.
6. **Chain-Tiefe**: `chain.links.len() <= max_delegation_depth` aus Governance (Default **3**, konfigurierbar).
7. **Scope-Kaskadierung**: Das aktuelle Topic/die Partition muss in **jeder** `allowed_*_pattern`-Menge aller Links matchen — das entspricht dem Durchschnitt aller Scopes (der engste Hop gewinnt).

## 7 Konfiguration

### 7.1 Governance-XML — Hybrid mit Named Profiles

Delegation-Regeln werden als **benannte Profiles** auf Top-Level
definiert. Peer-Classes referenzieren Profiles per `delegation_profile`-
Attribut. Das ist DRY (ein Profile, mehrere Peer-Classes), erlaubt
differenzierte Policy pro Class und bleibt audit-freundlich.

```xml
<!-- Top-Level: Named Delegation-Profiles (Named Types) -->
<zerodds:delegation_profiles>
  <profile name="vehicle-internal-gateway">
    <max_chain_depth>2</max_chain_depth>
    <trust_policy>gateway-only</trust_policy>
    <signature_algorithm>ecdsa-p256-sha256</signature_algorithm>
    <allowed_delegators>
      <delegator cert_cn_pattern="*.wanne.fleet-42.vehicle" />
      <delegator cert_cn_pattern="*.turm.fleet-42.vehicle" />
    </allowed_delegators>
    <allowed_topics>
      <topic_pattern>sensor.*</topic_pattern>
      <topic_pattern>telemetry.*</topic_pattern>
    </allowed_topics>
  </profile>

  <profile name="c4i-via-wanne-gateway">
    <max_chain_depth>3</max_chain_depth>
    <trust_policy>strict-delegated</trust_policy>
    <signature_algorithm>ecdsa-p256-sha256</signature_algorithm>
    <allowed_delegators>
      <delegator cert_cn_pattern="*.wanne.fleet-42.vehicle" />
    </allowed_delegators>
    <allowed_topics>
      <topic_pattern>c4i.*</topic_pattern>
    </allowed_topics>
  </profile>

  <profile name="federated-v2v">
    <max_chain_depth>3</max_chain_depth>
    <trust_policy>federation</trust_policy>
    <signature_algorithm>ecdsa-p256-sha256</signature_algorithm>
    <allowed_delegators>
      <delegator cert_cn_pattern="*.gateway.fleet-*.vehicle" />
    </allowed_delegators>
  </profile>
</zerodds:delegation_profiles>

<!-- Peer-Classes referenzieren Profiles -->
<zerodds:peer_classes>
  <peer_class name="legacy-edge" protection="NONE">
    <match auth_plugin_class="" delegation_profile="vehicle-internal-gateway" />
  </peer_class>
  <peer_class name="c4i-relay" protection="ENCRYPT">
    <match cert_cn_pattern="*.c4i.nato.int" delegation_profile="c4i-via-wanne-gateway" />
  </peer_class>
  <peer_class name="v2v-federated" protection="ENCRYPT">
    <match cert_cn_pattern="*.gateway.fleet-*.vehicle"
           delegation_profile="federated-v2v" />
  </peer_class>
  <peer_class name="direct-authed" protection="SIGN">
    <!-- Kein delegation_profile → direkte Auth, keine Chain erwartet -->
    <match auth_plugin_class="DDS:Auth:PKI-DH:1.2" />
  </peer_class>
</zerodds:peer_classes>
```

**Parser-Checks (Stufe j-h):**
- `delegation_profile="..."` ohne existierenden `<profile name="...">` → Warning
  `unreferenced delegation profile "<name>"`.
- `<profile>` ohne Referenz aus Peer-Class → Info
  `delegation profile "<name>" unused`.

### 7.2 Signatur-Algorithmus

Konfiguriert per Profile. Unterstützte Werte (analog zu rustls-webpki):

| Wert | Algorithmus | Token-Größe | Vor/Nach |
|------|-------------|-------------|----------|
| `ecdsa-p256-sha256` (Default) | ECDSA P-256 + SHA-256 | ~72 byte | Hardware-beschleunigt auf ARM-Embedded, kleinste TLS-Standard-Signatur |
| `ecdsa-p384-sha384` | ECDSA P-384 + SHA-384 | ~104 byte | Höhere Sicherheit, teurer |
| `rsa-pss-sha256` | RSA-PSS 2048 + SHA-256 | ~256 byte | OMG-DDS-Security-Mandatory (Spec §9.3.2.1); für Fleet-Interop mit Legacy-Vendors |
| `ed25519` | Ed25519 (pur) | ~64 byte | Schnellste, kleinste — nicht OMG-konform; für Hochperformance-Deployments |

**Rationale Default ECDSA-P-256:**
- ARM-Cortex-M/A haben ECDSA-Hardware-Support (kein AES-NI-Äquivalent für RSA)
- Token-Größe treibt SPDP-Overhead — klein = weniger Beacon-Fragmentation
- OMG-Spec listet es als optional-but-widely-supported

**Wechsel-Semantik:** bei Signatur-Algorithmus-Änderung in einem
Profile müssen alle aktiven Delegations neu erzeugt werden. Das Gateway
erkennt den Profile-Reload und regeneriert — Edge-Peers melden dabei
ein Announce-Refresh-Event.

### 7.3 Runtime-Config

```rust
pub struct DelegationConfig {
    /// Named Profiles geparst aus Governance. In-Memory-Lookup per Name.
    pub profiles: BTreeMap<String, DelegationProfile>,
    /// Peer-Class → Profile-Name Mapping (aus Governance).
    pub class_to_profile: BTreeMap<String, String>,
    /// Pro Edge-Peer: static oder ephemeral Identity.
    pub edge_identities: Vec<EdgeIdentityConfig>,
    /// Wenn true: Peers ohne gültige Delegation-Chain werden gedroppt
    /// (strict mode). Default: true.
    pub require_chain_for_uncerted_peers: bool,
}

pub struct DelegationProfile {
    pub name: String,
    pub max_chain_depth: u8,
    pub trust_policy: TrustPolicy,  // gateway-only / direct-or-delegated / federation / strict-delegated
    pub signature_algorithm: SignatureAlgorithm,
    pub allowed_delegators: Vec<CertCnPattern>,
    pub allowed_topics: Vec<TopicPattern>,
}
```

## 8 Revocation

* **Implicit** via Gateway-Ausfall: Wenn das Gateway keine SPDP-Beacons
  mehr sendet, laufen nach `lease_duration` alle seine Delegationen
  aus (Empfänger verlassen sich nicht länger auf den `not_after`).
* **Explicit** via Gateway-Command: Gateway kann eine Delegation-ID
  aktiv zurückziehen. Das Gateway pusht beim nächsten SPDP-Beacon
  einen `zerodds.sec.revoked_delegations`-Property (Liste von
  `(delegatee_guid, issued_at)`-Hashes). Empfänger fügen sie zu einer
  lokalen Revocation-List hinzu; Samples von diesen Delegatees werden
  gedroppt.
* **Short-lived-First**: Ephemeral-Identities haben inhärent kurze
  Laufzeiten und erfordern keine explizite Revocation.

## 9 Zukünftige Erweiterungen (nicht in WP 4H-j)

* **Cross-Anchor-Federation** — direkte V2V ohne C4I-Broker.
  Mehrere Trust-Anchors, die sich gegenseitig durch Cross-Signing
  oder Federation-Metadata erkennen.
* **Attribute-Based-Delegation** — Scope nicht über Topic-Pattern
  sondern über Key-Value-Attribute (`classification=SECRET`,
  `region=EU`) aus Permissions-XML.
* **Group-Delegation** — eine Delegation gilt für eine ganze
  Peer-Class auf einmal (statt pro Edge einzeln).
* **Transparente Delegation** — der Empfänger sieht nur den Ursprungs-
  GUID, die Chain-Details werden nicht propagiert (Privacy-Argument).

## 10 Interop-Konsequenzen

Delegation ist **nicht in OMG DDS-Security 1.1 spezifiziert**. Der
gesamte Mechanismus läuft im `zerodds:`-Namespace und wird von Fremd-
Vendors ignoriert. Ein Cyclone-Peer würde:

* Die `zerodds.sec.delegation_chain`-SPDP-Property still ignorieren
  und den Gateway-Peer nur mit seiner eigenen Identity wahrnehmen.
* Samples die der Gateway im Namen von Edge-Peers sendet werden als
  vom Gateway selbst geschrieben interpretiert (aus Cyclone-Sicht).
* Das ist kein Sicherheits-Bruch: solange Cyclone die Gateway-
  Identity gültig findet, sieht er eben die aggregierte Sicht —
  gleiche Daten, nur ohne feinere Origin-Auflösung.

## 11 Scope-Matrix WP 4H-j

| Stufe | Thema | LOC (Schätzung) |
|-------|-------|-----------------|
| 4H-j-a | `DelegationLink`/`DelegationChain` + Sign/Verify in `security-pki` | ~350 |
| 4H-j-b | Chain-Validation + Scope-Intersection in `security-permissions` | ~200 |
| 4H-j-c | SPDP-Propagation (Wire-Format für Chain in WireProperty) | ~200 |
| 4H-j-d | `PeerClassMatch`-Extensions (`delegated_by`, `max_chain_depth`) | ~200 |
| 4H-j-e | `GatewayBridge`-Helper (delegate_for / revoke_for / sub-gateway-chaining) | ~300 |
| 4H-j-f | Static + Ephemeral Edge-Identity-Config-Parser | ~200 |
| 4H-j-g | E2E-Test Wanne+Turm-Doppelstern: 2 Gateways, 3 Edges, 1 C4I | ~400 |
| 4H-j-h | Governance-XML `<zerodds:delegation>` Parser + Interop-Test | ~250 |
| **Gesamt** | | **~2100 LOC** |

Erwarteter Zeitaufwand: 10 Arbeitstage.
