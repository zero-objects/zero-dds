# Heterogeneous Security — System-of-Systems-Policy

> **Status:** Draft v0.1 (2026-04-24)
> **Abhängigkeiten:** `02_architecture.md`, `04_safety_by_architecture.md`,
> `docs/plans/release-plan-v1.2-to-v2.0.md` §1.4 (v1.4-Security-Track)

## 1 Motivation

Die OMG-DDS-Security-1.1-Spec nimmt ein **homogenes** Security-Modell an:
eine Domain, eine Governance, eine Suite pro Policy-Klasse. Das passt
für monolithische Deployments (ein Cluster, eine CA, eine Suite).

ZeroDDS zielt auf **System-of-Systems**: Vehicle Networks, Tactical Mesh,
Industrie-Edge-Gateways. Da teilen sich auf **einem Interface** typischerweise:

| Peer-Klasse             | Sicherheits-Level            | Grund                           |
|-------------------------|-----------------------------|---------------------------------|
| Legacy-ECU ohne Cert    | Plain (kein Security-Plugin) | Zulieferer kann kein X.509      |
| Fast-Subsystem          | HMAC-SHA256 (SIGN only)      | Latenz-Budget verbietet Encrypt |
| Regular-Peer            | AES-GCM-128                  | Default-Suite v1.4              |
| High-Assurance-Peer     | AES-GCM-256 + OCSP           | Compliance / safety-cert        |
| Intra-Host-Loopback     | Plain (kein Netz verlassen)  | Performance                     |

Kein einzelner Vendor bietet heute dieses Muster out-of-the-box.
`SharedSecurityGate` (v1.4 Stand) macht **participant-global** — ein Level
für alles. Das soll der **Heterogeneous-Track** WP 4H aufbrechen.

## 2 Anforderungen

### 2.1 Funktional

1. **Policy-Granularität** bis auf `(peer, topic, interface)`-Tripel.
2. **Peer-Capability-Awareness**: ein Peer ohne `auth_plugin_class` in
   SPDP wird als Legacy klassifiziert, nicht gedroppt.
3. **Per-Reader-Encoding am Writer**: derselbe `DataWriter` sendet an
   Reader A plain, an Reader B encrypted — simultan, ohne doppelte
   Topic-Bindings.
4. **Interface-Routing**: Outgoing-Datagram wird pro Ziel-Locator auf
   das "richtige" Interface-Socket gelegt; die Interface-Klasse ist
   Policy-Input.
5. **Dynamic Upgrade**: Peer kann nach erfolgreichem Handshake
   nachträglich in höhere Klasse wechseln (SIGN → ENCRYPT).
6. **Fail-Safe-Defaults**: unbekannte Peers → restriktivste Policy aus
   `<domain_rule>` (nicht der freizügigste).

### 2.2 Nicht-funktional

* **OMG-Kompatibilität als Subset**: Wenn Governance-XML nur OMG-Elemente
  nutzt, verhält sich ZeroDDS identisch zu Fast-DDS/Connext.
  Heterogene-Erweiterung ist opt-in über eigenen Namespace.
* **Zero-Overhead ohne Security**: Ohne `security`-Feature oder mit
  allen-plain-Policy, darf der Hot-Path nicht teurer werden als heute.
* **Branch-Coverage ≥ 99 %** im `security-runtime`-Crate
  (Safety-Crate-Klasse).

## 3 Kern-Datenstrukturen

### 3.1 `PolicyEngine`

```rust
pub trait PolicyEngine: Send + Sync {
    fn outbound_decision(&self, ctx: OutboundCtx<'_>) -> PolicyDecision;
    fn inbound_decision(&self, ctx: InboundCtx<'_>)   -> PolicyDecision;
    fn accept_peer(&self, caps: &PeerCapabilities)    -> bool;
}

pub struct OutboundCtx<'a> {
    pub domain_id:   u32,
    pub topic:       &'a str,
    pub partition:   &'a [String],
    pub interface:   &'a NetInterface,
    pub remote_peer: &'a PeerKey,
    pub remote_caps: &'a PeerCapabilities,
}

pub struct InboundCtx<'a> {
    pub domain_id:   u32,
    pub source_peer: &'a PeerKey,
    pub source_iface:&'a NetInterface,
    pub source_caps: Option<&'a PeerCapabilities>, // None bei Fremd-Vendor
    pub is_sec_prefixed: bool,
}

pub struct PolicyDecision {
    pub protection: ProtectionLevel,   // None | Sign | Encrypt
    pub suite:      Option<SuiteHint>, // Aes128 | Aes256 | HmacOnly
    pub drop:       bool,              // hard drop (nicht akzeptiert)
}
```

Default-Impl liest Governance-XML (kompatibel zu OMG). Nutzer kann
eigenen Impl einstecken (z.B. aus externer Policy-Datenbank, LDAP, oder
aus einem Vehicle-Network-Certification-Manager).

### 3.2 `PeerCapabilities`

```rust
pub struct PeerCapabilities {
    pub auth_plugin_class:    Option<String>,  // "DDS:Auth:PKI-DH:1.2", etc.
    pub crypto_plugin_class:  Option<String>,
    pub access_plugin_class:  Option<String>,
    pub supported_suites:     Vec<SuiteHint>,
    pub offered_protection:   ProtectionLevel,
    pub has_valid_cert:       bool,            // aus OCSP + Chain
    pub validity_window:      Option<Validity>,
    pub vendor_hint:          Option<String>,  // fuer Vendor-Quirks
}
```

Gefüllt aus SPDP (Auth-Plugin-Class, Crypto-Plugin-Class, etc.) und
aus SEDP-Permissions-Token.

### 3.3 `NetInterface`

```rust
pub enum NetInterface {
    Loopback,                      // 127.0.0.0/8, ::1
    LocalHost(SharedMem | UnixDomainSocket),
    LocalSubnet(IpRange),          // z.B. 10.0.0.0/24
    Wan,                           // alles andere
    Named(String),                 // "eth0", "can0"-Bridge, "tun0"
}
```

Die Klassifikation geschieht am Send-Point aus der Locator-IP + Runtime-
Konfiguration (Nutzer gibt `local_subnets: Vec<IpRange>` an).

## 4 Datenfluss

### 4.1 Outbound (ein Writer, N Reader)

```
DataWriter.write(sample)
    │
    ├─> for reader_guid in matched_readers:
    │       caps     = peer_cache.get(reader_guid)
    │       decision = policy.outbound_decision(ctx)
    │       match decision.protection {
    │           None    => raw_bytes,
    │           Sign    => hmac_wrap(raw_bytes, peer_key),
    │           Encrypt => aead_wrap(raw_bytes, peer_key, decision.suite),
    │       }
    │       socket_for(reader.interface).send(reader.locator, wire)
    │
    └─> (N Wire-Pakete, einer pro Reader, versch. Protection-Level)
```

Konsequenz: **1 Sample → bis zu N Wire-Pakete**. Statt Multicast (ein
Paket) geht bei heterogener Policy Unicast-Fan-Out pro Reader. Das ist
die **Kostenseite** der System-of-Systems-Flexibilität.

OMG-Optimierung `ReceiverSpecificMACs` (WP 4H-g) reduziert das zum
"ein Ciphertext + N MACs" wenn **alle** Reader gleiche Suite nutzen,
aber unterschiedliche Auth-Tokens erwarten — spart Encryption-Kosten,
nicht Socket-Traffic.

### 4.2 Inbound

```
UDP-socket.recv() → (bytes, source_addr)
    │
    ├─> iface  = classify_interface(source_addr)
    │   peer   = extract_guid_prefix(bytes[8..20])
    │   caps   = peer_cache.get(peer)
    │   is_sec = bytes.len() > 20 && bytes[20] == SEC_PREFIX
    │
    ├─> decision = policy.inbound_decision(ctx { peer, iface, caps, is_sec })
    │
    └─> match (decision.drop, is_sec, decision.protection) {
            (true, _, _)                   => drop,
            (false, true, _)               => unwrap_and_dispatch,
            (false, false, None)           => dispatch_plain,
            (false, false, Sign|Encrypt)   => policy_violation_drop,
        }
```

### 4.3 Peer-Capability-Update (SPDP)

```
spdp.receive(datagram)
    │
    ├─> parse_participant_proxy(datagram)
    ├─> extract_security_props(participant.properties)
    │      ↓
    ├─> caps = PeerCapabilities {
    │      auth_plugin_class: props["dds.sec.auth.plugin_class"],
    │      supported_suites:  parse_suite_list(props["dds.sec.crypto.suites"]),
    │      ...
    │   }
    │
    └─> peer_cache.insert(guid_prefix, caps)
```

Ab diesem Moment trifft jede Folge-Policy-Entscheidung auf die frischen
Caps. **Uprade-Pfad**: Peer war initial als `auth_plugin=None` klassifiziert,
schickt nach Handshake ein Extended-SPDP → Cache wird aktualisiert,
nächster Send an diesen Peer ist encrypted.

## 5 Governance-XML-Erweiterungen

Zusätzlich zu den OMG-Standard-Elementen definieren wir einen
ZeroDDS-Namespace:

```xml
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains>

      <!-- OMG-Standard -->
      <rtps_protection_kind>SIGN</rtps_protection_kind>

      <!-- ZeroDDS-Erweiterung: Peer-Klassen -->
      <zerodds:peer_classes>
        <peer_class name="legacy" protection="NONE">
          <match auth_plugin_class="" />          <!-- leer = keiner -->
        </peer_class>
        <peer_class name="fast" protection="SIGN">
          <match cert_cn_pattern="*.fast.example" />
        </peer_class>
        <peer_class name="secure" protection="ENCRYPT">
          <match auth_plugin_class="DDS:Auth:PKI-DH:1.2" suite="AES_128_GCM" />
        </peer_class>
        <peer_class name="highassurance" protection="ENCRYPT">
          <match cert_cn_pattern="*.ha.*" suite="AES_256_GCM" require_ocsp="true" />
        </peer_class>
      </zerodds:peer_classes>

      <!-- ZeroDDS-Erweiterung: Interface-Bindings -->
      <zerodds:interface_bindings>
        <interface name="loopback" protection_override="NONE" />
        <interface name="shm"      protection_override="NONE" />
        <interface name="eth0"     peer_class_filter="legacy,fast,secure" />
        <interface name="tun0"     peer_class_filter="secure,highassurance"
                                    protection_min="ENCRYPT" />
      </zerodds:interface_bindings>
    </domain_rule>
  </domain_access_rules>
</dds>
```

Vendor-Interop: andere Vendors **ignorieren** den `zerodds:`-Namespace
still und fallen zurück auf das OMG-Standard-Element — dadurch bleibt
ein ZeroDDS-Governance mit Fallback spec-kompatibel.

## 6 Runtime-Integration

### 6.1 Multi-Socket-Binding

Aktuell bindet `DcpsRuntime` genau **einen** UDP-Unicast-Socket pro
Port. Für Interface-Routing brauchen wir:

```
RuntimeConfig {
    interfaces: Vec<InterfaceBinding>,
    ...
}

InterfaceBinding {
    name:      String,           // "eth0", "lo"
    bind_addr: Ipv4Addr,
    kind:      NetInterface,     // logische Klasse
}
```

Beim Start: pro Binding einen Socket. Outbound-Router schaut per
Ziel-Locator + Interface-Klasse nach dem richtigen Socket.

### 6.2 `SharedSecurityGate` → `PolicyDrivenGate`

Der bestehende `SharedSecurityGate` (v1.4-a) wird zum ersten Default-
`PolicyEngine`-Impl. Sein `transform_outbound`-Contract wird ersetzt
durch `transform_outbound_for(peer_key, &wire)` — das Gate entscheidet
anhand Peer-Key.

Bestands-Tests bleiben grün: wenn nur eine Peer-Klasse in der
Governance gelistet ist, ist das Verhalten identisch zum aktuellen
Ein-Policy-Gate.

### 6.3 Hot-Path-Hook-Änderungen

Die 6 Inject-Punkte aus WP 4.4-b.4 werden erweitert:

| Alter Hook                                  | Neuer Hook                                   |
|---------------------------------------------|----------------------------------------------|
| `secure_outbound_bytes(rt, bytes)`          | `secure_outbound_for(rt, peer, iface, bytes)`|
| `secure_inbound_bytes(rt, bytes)`           | `secure_inbound_from(rt, peer, iface, bytes)`|

Der Writer-Tick-Loop muss **iterieren über matched Readers** statt
einmalig-Broadcast.

## 7 Safety- und Coverage-Grenzen

Der neue `PolicyEngine`-Trait und `peer_cache` liegen in
`zerodds-security-runtime`. Dieser Crate bleibt **SAFE**-klassifiziert
(Safety-qualifizierbar). Die `dyn PolicyEngine`-Polymorphie ist der
einzige Punkt mit `zerodds-lint: allow no_dyn_in_safe` — architekturbedingt.

Branch-Coverage-Ziel: **99 %** für
* `PolicyEngine`-Default-Impl
* `PeerCapabilities`-Parser
* Interface-Classifier

## 8 Interop-Risiken

1. **Cross-Vendor-Communication** mit ZeroDDS-Heterogeneous-Peer:
   * Cyclone sieht `<peer_classes>` nicht → nimmt `<rtps_protection_kind>`.
   * Wenn ZeroDDS per Peer-Class `NONE` schickt wo Cyclone `SIGN`
     erwartet → Cyclone droppt. Dokumentiert als **Interop-Constraint**.
2. **Gate-Routing-Bugs** könnten plaintext-Leak produzieren —
   Mitigations:
   * Default-Policy beim Unknown-Peer ist **restriktivste** Class aus XML.
   * Fuzz-Test mit zufälligen PeerCaps und Interface-Combis.
3. **Upgrade-Pfad im laufenden Session**: Peer wechselt von
   Legacy→Secure. Writer muss alte Sequence-Numbers nicht
   retransmit-verschlüsseln (sonst Replay-Attack-Window).

## 9 Nicht-Ziele

* **Ad-hoc-Peer-Classes** (JSON/YAML Runtime-Config) — Heterogeneous-
  Config ist Build-time-deklarativ, kein dynamisches
  Attribute-Based-Access-Control.
* **Crypto-Agility** (Ciphersuite-Negotiation auf dem Wire) —
  OMG-Spec unterstützt das nicht, wir planen es nicht.
* ~~**Delegation** (Peer-X-signiert-für-Peer-Y) — v2.0-Feature.~~
  **Aufgenommen in WP 4H-j** — siehe `09_delegation.md`. Anforderung
  aus Planning 2026-04: Gateway/Bridge-Identity für Vehicle-Mesh
  (Wanne+Turm) mit Edge-Peers ohne eigenen Cert und Inter-Fahrzeug-
  Kommunikation via C4I-Broker.

## 10 Umgesetzte Produktions-Integration (WP 4H-i)

Der ursprüngliche Plan nannte Receiver-Specific-MACs als
Crypto-Plugin-Feature — die End-to-End-Verdrahtung mit PKI-Handshake-
Shared-Secrets war dort nicht explizit spezifiziert. Der Nachtrag
WP 4H-i schließt die Lücke:

* `zerodds-security::authentication::SharedSecretProvider`-Trait — eine
  schlanke Lookup-Brücke von `AuthenticationPlugin` (liefert die DH-
  abgeleiteten 32-byte-Secrets via `secret_bytes(handle)`) zu
  `CryptographicPlugin`.
* `PkiAuthenticationPlugin` implementiert den Trait.
* `AesGcmCryptoPlugin::with_secret_provider(suite, provider)` — wenn
  konfiguriert, leitet `register_matched_remote_participant` den
  Per-Peer-Master-Key via HKDF-SHA256 aus dem DH-Secret ab statt einen
  Random-Key zu generieren und per Token-Exchange zu verteilen.
* Gemischter Betrieb: wenn der Provider ein `SharedSecretHandle` nicht
  kennt, fällt der Plugin auf den v1.4-Random-Slot-Pfad zurück. Damit
  können per-Peer-MAC-Keys (DH) parallel zu Broadcast-Cipher-Keys
  (klassischer Token-Exchange) im gleichen Plugin-Objekt existieren —
  genau das Muster von Multi-MAC aus §4.1.

**Verifiziert in** `crates/security-pki/tests/pki_crypto_integration.rs`:
* `x25519_handshake_shared_secret_drives_aes_gcm_roundtrip`
* `three_peers_each_own_handshake_yield_distinct_per_peer_keys`
* `multi_mac_group_broadcast_with_real_dh_derived_mac_keys`
