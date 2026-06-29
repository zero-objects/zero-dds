# Heterogeneous security — system-of-systems policy

> **Status:** Draft v0.1 (2026-04-24)
> **Dependencies:** `02_architecture.md`, `04_safety_by_architecture.md`,
> `internal/plans/release-plan-v1.2-to-v2.0.md` §1.4 (v1.4 security track)

## 1 Motivation

The OMG DDS-Security 1.1 spec assumes a **homogeneous** security model:
one domain, one governance, one suite per policy class. That fits
monolithic deployments (one cluster, one CA, one suite).

ZeroDDS targets **systems-of-systems**: vehicle networks, tactical mesh,
industrial edge gateways. There, the following typically share **one interface**:

| Peer class             | Security level               | Reason                          |
|-------------------------|-----------------------------|---------------------------------|
| Legacy ECU without cert | Plain (no security plugin)   | The supplier cannot do X.509    |
| Fast subsystem          | HMAC-SHA256 (SIGN only)      | The latency budget forbids encrypt |
| Regular peer            | AES-GCM-128                  | Default suite v1.4              |
| High-assurance peer     | AES-GCM-256 + OCSP           | Compliance / safety cert        |
| Intra-host loopback     | Plain (never leaves the host) | Performance                    |

No single vendor offers this pattern out of the box today.
`SharedSecurityGate` (v1.4 state) is **participant-global** — one level
for everything. The **heterogeneous track** WP 4H is meant to break that open.

## 2 Requirements

### 2.1 Functional

1. **Policy granularity** down to the `(peer, topic, interface)` triple.
2. **Peer-capability awareness**: a peer without an `auth_plugin_class` in
   SPDP is classified as legacy, not dropped.
3. **Per-reader encoding at the writer**: the same `DataWriter` sends to
   reader A plain, to reader B encrypted — simultaneously, without duplicate
   topic bindings.
4. **Interface routing**: an outgoing datagram is placed per destination locator on
   the "right" interface socket; the interface class is a
   policy input.
5. **Dynamic upgrade**: a peer can, after a successful handshake,
   subsequently move to a higher class (SIGN → ENCRYPT).
6. **Fail-safe defaults**: unknown peers → the most restrictive policy from
   `<domain_rule>` (not the most permissive).

### 2.2 Non-functional

* **OMG compatibility as a subset**: when the governance XML uses only OMG elements,
  ZeroDDS behaves identically to Fast-DDS/Connext.
  The heterogeneous extension is opt-in via its own namespace.
* **Zero overhead without security**: without the `security` feature or with
  an all-plain policy, the hot path must not become more expensive than today.
* **Branch coverage ≥ 99 %** in the `security-runtime` crate
  (safety-crate class).

## 3 Core data structures

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
    pub source_caps: Option<&'a PeerCapabilities>, // None for a foreign vendor
    pub is_sec_prefixed: bool,
}

pub struct PolicyDecision {
    pub protection: ProtectionLevel,   // None | Sign | Encrypt
    pub suite:      Option<SuiteHint>, // Aes128 | Aes256 | HmacOnly
    pub drop:       bool,              // hard drop (not accepted)
}
```

The default impl reads governance XML (compatible with OMG). The user can
plug in their own impl (e.g. from an external policy database, LDAP, or
from a vehicle-network certification manager).

### 3.2 `PeerCapabilities`

```rust
pub struct PeerCapabilities {
    pub auth_plugin_class:    Option<String>,  // "DDS:Auth:PKI-DH:1.2", etc.
    pub crypto_plugin_class:  Option<String>,
    pub access_plugin_class:  Option<String>,
    pub supported_suites:     Vec<SuiteHint>,
    pub offered_protection:   ProtectionLevel,
    pub has_valid_cert:       bool,            // from OCSP + chain
    pub validity_window:      Option<Validity>,
    pub vendor_hint:          Option<String>,  // for vendor quirks
}
```

Filled from SPDP (auth-plugin class, crypto-plugin class, etc.) and
from the SEDP permissions token.

### 3.3 `NetInterface`

```rust
pub enum NetInterface {
    Loopback,                      // 127.0.0.0/8, ::1
    LocalHost(SharedMem | UnixDomainSocket),
    LocalSubnet(IpRange),          // e.g. 10.0.0.0/24
    Wan,                           // everything else
    Named(String),                 // "eth0", "can0"-bridge, "tun0"
}
```

The classification happens at the send point from the locator IP + runtime
configuration (the user provides `local_subnets: Vec<IpRange>`).

## 4 Data flow

### 4.1 Outbound (one writer, N readers)

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
    └─> (N wire packets, one per reader, different protection levels)
```

Consequence: **1 sample → up to N wire packets**. Instead of multicast (one
packet), heterogeneous policy uses a unicast fan-out per reader. That is
the **cost side** of system-of-systems flexibility.

The OMG optimization `ReceiverSpecificMACs` (WP 4H-g) reduces this to
"one ciphertext + N MACs" when **all** readers use the same suite
but expect different auth tokens — it saves encryption cost,
not socket traffic.

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

### 4.3 Peer-capability update (SPDP)

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

From this moment, every subsequent policy decision hits the fresh
caps. **Upgrade path**: the peer was initially classified as `auth_plugin=None`,
sends an extended SPDP after the handshake → the cache is updated,
the next send to this peer is encrypted.

## 5 Governance-XML extensions

In addition to the OMG standard elements we define a
ZeroDDS namespace:

```xml
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains>

      <!-- OMG standard -->
      <rtps_protection_kind>SIGN</rtps_protection_kind>

      <!-- ZeroDDS extension: peer classes -->
      <zerodds:peer_classes>
        <peer_class name="legacy" protection="NONE">
          <match auth_plugin_class="" />          <!-- empty = none -->
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

      <!-- ZeroDDS extension: interface bindings -->
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

Vendor interop: other vendors **ignore** the `zerodds:` namespace
silently and fall back to the OMG standard element — so a
ZeroDDS governance with a fallback stays spec-compatible.

## 6 Runtime integration

### 6.1 Multi-socket binding

Currently `DcpsRuntime` binds exactly **one** UDP unicast socket per
port. For interface routing we need:

```
RuntimeConfig {
    interfaces: Vec<InterfaceBinding>,
    ...
}

InterfaceBinding {
    name:      String,           // "eth0", "lo"
    bind_addr: Ipv4Addr,
    kind:      NetInterface,     // logical class
}
```

At startup: one socket per binding. The outbound router looks up the right socket
by destination locator + interface class.

### 6.2 `SharedSecurityGate` → `PolicyDrivenGate`

The existing `SharedSecurityGate` (v1.4-a) becomes the first default
`PolicyEngine` impl. Its `transform_outbound` contract is replaced
by `transform_outbound_for(peer_key, &wire)` — the gate decides
based on the peer key.

Existing tests stay green: when only one peer class is listed in the
governance, the behavior is identical to the current
single-policy gate.

### 6.3 Hot-path-hook changes

The 6 inject points from WP 4.4-b.4 are extended:

| Old hook                                    | New hook                                     |
|---------------------------------------------|----------------------------------------------|
| `secure_outbound_bytes(rt, bytes)`          | `secure_outbound_for(rt, peer, iface, bytes)`|
| `secure_inbound_bytes(rt, bytes)`           | `secure_inbound_from(rt, peer, iface, bytes)`|

The writer tick loop must **iterate over matched readers** instead of
a one-time broadcast.

## 7 Safety and coverage boundaries

The new `PolicyEngine` trait and `peer_cache` live in
`zerodds-security-runtime`. This crate stays **SAFE**-classified
(safety-qualifiable). The `dyn PolicyEngine` polymorphism is the
only point with `zerodds-lint: allow no_dyn_in_safe` — by architecture.

Branch-coverage goal: **99 %** for
* the `PolicyEngine` default impl
* the `PeerCapabilities` parser
* the interface classifier

## 8 Interop risks

1. **Cross-vendor communication** with a ZeroDDS heterogeneous peer:
   * Cyclone does not see `<peer_classes>` → takes `<rtps_protection_kind>`.
   * When ZeroDDS sends `NONE` per peer class where Cyclone expects `SIGN`
     → Cyclone drops. Documented as an **interop constraint**.
2. **Gate-routing bugs** could produce a plaintext leak —
   mitigations:
   * The default policy for an unknown peer is the **most restrictive** class from the XML.
   * Fuzz test with random PeerCaps and interface combinations.
3. **Upgrade path during a running session**: a peer switches from
   legacy→secure. The writer must not retransmit-encrypt old sequence numbers
   (otherwise a replay-attack window).

## 9 Non-goals

* **Ad-hoc peer classes** (JSON/YAML runtime config) — heterogeneous
  config is build-time declarative, not dynamic
  attribute-based access control.
* **Crypto agility** (ciphersuite negotiation on the wire) —
  the OMG spec does not support that, we do not plan it.
* ~~**Delegation** (peer X signs for peer Y) — a v2.0 feature.~~
  **Included in WP 4H-j** — see `09_delegation.md`. Requirement
  from Planning 2026-04: gateway/bridge identity for a vehicle mesh
  (hull+turret) with edge peers without their own cert and inter-vehicle
  communication via a C4I broker.

## 10 Implemented production integration (WP 4H-i)

The original plan named receiver-specific MACs as a
crypto-plugin feature — the end-to-end wiring with PKI-handshake
shared secrets was not explicitly specified there. The addendum
WP 4H-i closes the gap:

* `zerodds-security::authentication::SharedSecretProvider` trait — a
  lean lookup bridge from `AuthenticationPlugin` (delivers the DH-
  derived 32-byte secrets via `secret_bytes(handle)`) to
  `CryptographicPlugin`.
* `PkiAuthenticationPlugin` implements the trait.
* `AesGcmCryptoPlugin::with_secret_provider(suite, provider)` — when
  configured, `register_matched_remote_participant` derives the
  per-peer master key via HKDF-SHA256 from the DH secret instead of generating a
  random key and distributing it via token exchange.
* Mixed operation: when the provider does not know a `SharedSecretHandle`,
  the plugin falls back to the v1.4 random-slot path. Thus
  per-peer MAC keys (DH) can exist in parallel to broadcast cipher keys
  (classic token exchange) in the same plugin object —
  exactly the multi-MAC pattern from §4.1.

**Verified in** `crates/security-pki/tests/pki_crypto_integration.rs`:
* `x25519_handshake_shared_secret_drives_aes_gcm_roundtrip`
* `three_peers_each_own_handshake_yield_distinct_per_peer_keys`
* `multi_mac_group_broadcast_with_real_dh_derived_mac_keys`
