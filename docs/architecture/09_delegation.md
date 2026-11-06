# Delegation — gateway/bridge identity for vehicle mesh + loosely coupled systems

> **Dependencies:** `08_heterogeneous_security.md` (peer classes, interface routing),
>   `02_architecture.md §3` (transport layers), WP 4H-i (PKI↔crypto integration)
>
> **Code:** `crates/security-pki/src/delegation.rs` (`DelegationLink`,
> `DelegationChain`, sign, verify, wire form), `crates/security-permissions/src/delegation_check.rs`
> (`validate_chain`, `DelegationProfile`, `TrustAnchor`),
> `crates/security-permissions/src/governance.rs` (the XML of §7.1),
> `crates/security-runtime/src/gateway_bridge.rs` (`GatewayBridge`),
> `crates/security-runtime/src/peer_class.rs` (`peer_matches_class_with_delegation`),
> `crates/security-runtime/src/caps_wire.rs` (the SPDP property). This document
> describes what that code does. The DCPS runtime calls none of it yet: no
> participant announces a chain and none is validated during discovery
> (`internal/release/OPEN-ITEMS.md`).

## 1 Motivation

In the heterogeneous track (WP 4H) we built peer classes for a flat
participant space. In real vehicle networks the space is, however,
**hierarchical**: a central gateway (sometimes two — hull+turret)
bundles dozens of sensors, ECUs and subsystems that themselves
have no own security stack. Toward the outside world the
vehicle appears as **one participant**, internally it is a star or
double star with a security mix.

```
                    (outside world)
                       ▲
            C4I ──────►│◄──── other vehicles (V2V via C4I broker)
                       │
    ═══════════════════╪═══════════════════  vehicle boundary
                       │
                 hull gateway  ◄──► turret gateway      ← 1 or 2 hubs
                  ┌────┴────┐         ┌────┴────┐
                  │         │         │         │
             sensor ECU  driver HMI   weapon station  turret sensor
             (legacy)    (secure)     (secure)        (mix)
```

Delegation is the mechanism with which gateways represent the **identity** of
edge peers toward the outside, without those edge peers themselves
having to own PKI material.

## 2 Distinction: security layer vs. bridge layer

Delegation addresses the **security attribution** (who wrote it,
who may read it), not the routing:

| Layer | Task | Knows about delegation? |
|-------|---------|-------------------|
| **Bridge layer** | Transports samples between domains/hosts/vehicles; reacts to QoS, topic routes, partitions | No — agnostic, forwards everything |
| **Security layer** | Decides whether a peer is authentic, whether its sample is accepted, with which protection class | Yes — checks the delegation chain, scope, revocation |

**Rule:** The bridge layer must transport every message that
the security layer let through — no matter how many delegation hops
are in it. **Exception:** only explicitly spec-forbidden combinations
(e.g. OMG forbids secure envelopes over unencrypted
transport hops when the domain requires `rtps_protection_kind=ENCRYPT`).

## 3 Use cases

### 3.1 Vehicle-internal (star, 1 hop)

```
sensor ECU  ──► hull gateway   ──►  other vehicle subsystems
  (legacy,       (delegates for         (read "from lidar-A
   no cert)      lidar-A, radar-B…)      via hull-gateway")
```

### 3.2 Vehicle-internal (double star, 2 hops)

```
turret sensor ──► turret gateway ──► hull gateway ──► driver HMI
  (legacy)         (delegates)        (re-delegates    (reads with
                                       for turret       a 2-hop chain)
                                       sensor via
                                       turret GW)
```

The hull-gateway delegation is a **re-delegation**: it
confirms the turret-gateway delegation and appends itself as the
next hop.

### 3.3 Vehicle → C4I (2–3 hops)

```
turret sensor ──► turret GW ──► hull GW ──► C4I node
                                        (verifies the 3-hop chain
                                         against the fleet trust anchor)
```

### 3.4 V2V via a C4I broker (transport special case)

Lateral communication between vehicles runs, in this specific
scope, **not as a DDS direct peer link**, but as store-and-forward
over C4I: the sample is packed into SMTP datagrams, sent to C4I,
C4I forwards to the target vehicle, where it is re-assembled. The
security layer sees only the local C4I link, not the other
vehicle directly.

For **general loosely coupled systems** outside this SMTP
scenario (later): cross-anchor federation — a vehicle trusts
the other when its gateway cert is signed by a common fleet CA.
See §9 "Future extensions".

## 4 Trust model

### 4.1 Vertical trust chain

```
fleet root CA
    │
    ├── vehicle CA chassis-42
    │       │
    │       ├── hull gateway cert   ← in the trust anchor of every external peer
    │       └── turret gateway cert ← likewise
    │
    └── C4I CA
            └── C4I node cert         ← a different trust-anchor level
```

* **External peers** (C4I, other vehicles) have only the **gateway
  certs** in their trust store — not the edge sensors.
* **The gateway** signs delegation tokens for its edge peers with
  its own gateway key. Every delegation is cryptographically
  traceable back to the gateway cert.
* **Edge peers** have no own key material — their identity
  exists only **derived** from the gateway delegation.

### 4.2 Horizontal peer-class classification

Edge peers are additionally sorted into peer classes (WP 4H-h):

| Edge type | Peer class | Delegation? |
|----------|-----------|-------------|
| Legacy ECU without cert | `legacy` | Mandatory (reachable only as a gateway delegate) |
| Fast subsystem with cert | `fast` | Optional (the gateway can represent them anyway) |
| Highly secure C4I | `highassurance` | No delegation — a direct cert |

A peer class requires delegation by naming a delegation profile
(`PeerClassMatch::delegation_profile`, §7.1).

### 4.3 Trust-policy modes

The trust-anchor semantics are **configurable per delegation profile**
(see §7.1). Four modes are available:

| Mode | Semantics |
|-------|----------|
| `gateway-only` | Only gateway certs in the trust anchor; edge peers accepted **only** via delegation. Default for vehicle → C4I. |
| `direct-or-delegated` | An edge with its own cert is accepted **directly** (if present in the trust store); otherwise the delegation path. Mixed networks with some strong edges. |
| `federation` | **Cross-anchor**: the trust store knows several root CAs, peers of all known roots accepted. For V2V-general / loosely coupled systems. |
| `strict-delegated` | Even gateway certs are accepted **only** as delegators, never as direct user peers. For cases where the gateway itself must not write samples. |

Combinable: a profile `c4i-relay-only` can be `strict-delegated`
while another profile `internal-bridge` runs on
`direct-or-delegated` — within the same governance.

## 5 Data model

### 5.1 `DelegationLink`

```rust
pub struct DelegationLink {
    /// Who delegates: the owner of the key that signed the link.
    pub delegator_guid: [u8; 16],
    /// To whom: the peer that holds the delegated identity.
    pub delegatee_guid: [u8; 16],
    /// Topic patterns the delegatee may serve (wildcards as `topic_match`);
    /// empty = all, only in the first link.
    pub allowed_topic_patterns: Vec<String>,
    /// Partition patterns; empty = the default partition.
    pub allowed_partition_patterns: Vec<String>,
    /// Validity window (Unix seconds).
    pub not_before: i64,
    pub not_after: i64,
    /// The algorithm the link is signed with (§7.2).
    pub algorithm: SignatureAlgorithm,
    /// Signature by the delegator over the fields above.
    pub signature: Vec<u8>,
}
```

### 5.2 `DelegationChain`

```rust
pub struct DelegationChain {
    /// The participant the chain starts at: the delegator of `links[0]`,
    /// whose key is a trust anchor of the receiver's profile.
    pub origin_guid: [u8; 16],
    /// Links in delegation order: `links[0]` from the origin to the first
    /// delegatee, each next one from the previous delegatee onward; the
    /// delegatee of the last link is the edge, the peer whose GUID is in
    /// the samples (`DelegationChain::edge_guid`).
    pub links: Vec<DelegationLink>,
}
```

At most 8 links (`MAX_CHAIN_DEPTH_HARD_CAP`); a profile sets a lower limit
(§6 point 6). A gateway that is itself a delegatee (§3.2) prepends the chain
it received to its own link (`GatewayBridge::with_upstream`, `chain_for`).

### 5.3 Ephemeral vs. static edge identity

Default: **static GuidPrefix** from the governance.

```xml
<zerodds:edge_identities default_mode="static">
  <zerodds:edge name="lidar-A" guid_prefix="01020304050607080900000a" />
  <zerodds:edge name="radar-B" mode="ephemeral" lifetime_seconds="300" />
</zerodds:edge_identities>
```

* **Static** — simple configuration, stable identity across boots,
  susceptible to replay when message counters are not protected
  along with it.
* **Ephemeral** — a new GuidPrefix and a new delegation per
  `lifetime_seconds` (default 300, `DEFAULT_EPHEMERAL_LIFETIME_SECS`;
  `GatewayBridge::rotate_ephemerals`). Replay windows are thus narrow, at
  the price of more SPDP traffic.

## 6 Chain validation

The receiver validates a chain against a delegation profile
(`validate_chain`):

1. **Chain continuity**: for all `i`: `links[i].delegatee_guid == links[i+1].delegator_guid`.
2. **Origin match**: `chain.origin_guid == links[0].delegator_guid`.
3. **Trust anchor**: `links[0].delegator_guid` is the `subject_guid` of a
   trust anchor of the profile (`gateway-only`: of its one anchor).
4. **Signature chain**: `links[0]` verifies with the anchor's public key
   (and is signed with the anchor's algorithm); every next link with the
   public key of its delegator, which is the previous link's delegatee.
5. **Algorithm and time window**: every link's algorithm is in the
   profile's allowed set, and `now ∈ [not_before, not_after]`.
6. **Chain depth**: `chain.links.len() <= max_chain_depth` of the profile
   (default 3).
7. **Scope intersection**: the topic and partition patterns in force are
   the intersection of all links' patterns: a later hop only narrows.

The result names the origin, the edge (the last delegatee), the depth and
the patterns in force (`ValidatedChain`).

## 7 Configuration

### 7.1 Governance XML

Delegation profiles are named at the top level, in the namespace
`https://zerodds.org/schema/security/heterogeneous`; a peer class names the
profile its peers' chains are validated against.

```xml
<dds xmlns:zerodds="https://zerodds.org/schema/security/heterogeneous">
  <zerodds:delegation_profiles>
    <zerodds:profile name="vehicle-internal">
      <zerodds:trust_policy>gateway-only</zerodds:trust_policy>
      <zerodds:max_chain_depth>2</zerodds:max_chain_depth>
      <zerodds:require_ocsp>false</zerodds:require_ocsp>
      <zerodds:allowed_algorithms>
        <zerodds:algorithm>ECDSA+P256+SHA256</zerodds:algorithm>
      </zerodds:allowed_algorithms>
      <zerodds:trust_anchors>
        <zerodds:anchor subject_guid="01020304050607080910111213141516"
                        algorithm="ECDSA+P256+SHA256"
                        public_key="(base64 of the public key)" />
      </zerodds:trust_anchors>
    </zerodds:profile>
  </zerodds:delegation_profiles>

  <domain_access_rules><domain_rule>
    <domains><id>0</id></domains>
    <zerodds:peer_classes>
      <zerodds:peer_class name="legacy-edge" protection="NONE">
        <zerodds:match auth_plugin_class="" delegation_profile="vehicle-internal" />
      </zerodds:peer_class>
      <zerodds:peer_class name="direct-authed" protection="SIGN">
        <!-- No delegation_profile: direct authentication, no chain -->
        <zerodds:match auth_plugin_class="DDS:Auth:PKI-DH:1.2" />
      </zerodds:peer_class>
    </zerodds:peer_classes>
  </domain_rule></domain_access_rules>
</dds>
```

`trust_policy`: `gateway-only`, `direct-or-delegated` (default),
`federation`, `strict-delegated` (§4.3). A value the parser does not know is
refused with the value named, as is a profile name declared twice. The
topic and partition scope is in the links (§5.1), not in the profile.

### 7.2 Signature algorithm

Allowed per profile (`allowed_algorithms`) and per trust anchor
(`algorithm`). The names are the CryptoAlgorithmName strings of
DDS-Security 1.2 Table 25 (case does not matter); the short names this
implementation read before are accepted too:

| Name (Table 25) | Also accepted | Algorithm |
|---|---|---|
| `ECDSA+P256+SHA256` | `ECDSA-SHA256`, `EC-prime256v1`, `ecdsa-p256` | ECDSA P-256 + SHA-256 |
| `ECDSA+P384+SHA384` | `ecdsa-p384` | ECDSA P-384 + SHA-384 |
| `RSASSA-PSS-MGF1SHA256+2048+SHA256` | `RSASSA-PSS-SHA256`, `RSA-2048`, `rsa-pss-2048` | RSA-PSS 2048 + SHA-256 |
| — | `ed25519` | Ed25519 (not in Table 25) |

`RSASSA-PKCS1-V1_5+2048+SHA256` of Table 25 has no delegation signature and
is refused. A link names its algorithm on the wire by a number
(`SignatureAlgorithm::wire_id`: 1, 2, 3, 4 in the order of the table).

### 7.3 Runtime config

```rust
pub struct DelegationProfile {
    pub name: String,
    pub trust_policy: TrustPolicy,
    pub trust_anchors: Vec<TrustAnchor>,   // subject_guid, public key, algorithm
    pub max_chain_depth: usize,            // default 3
    pub allowed_algorithms: BTreeSet<u8>,  // SignatureAlgorithm::wire_id
    pub require_ocsp: bool,
}
```

`Governance::delegation_profiles` holds them by name,
`PeerClassMatch::delegation_profile` the reference of a class, and
`Governance::edge_identities` the edges of §5.3.

## 8 Revocation

* **Implicit** via gateway failure: when the gateway no longer sends
  SPDP beacons, all its delegations expire after `lease_duration`.
* **Explicit** via the gateway: `GatewayBridge::revoke_delegation` takes a
  delegation back and keeps the edge for `take_revocations`, which the
  discovery layer is to announce; no SPDP property carries it yet.
* **Short-lived-first**: ephemeral identities have inherently short
  lifetimes and require no explicit revocation.

## 9 Future extensions

* **Cross-anchor federation** — direct V2V without a C4I broker.
  Several trust anchors that recognize each other through cross-signing
  or federation metadata.
* **Attribute-based delegation** — scope not via topic patterns
  but via key-value attributes (`classification=SECRET`,
  `region=EU`) from the permissions XML.
* **Group delegation** — one delegation applies to a whole
  peer class at once (instead of per edge individually).
* **Transparent delegation** — the receiver sees only the origin
  GUID, the chain details are not propagated (privacy argument).

## 10 Interop consequences

Delegation is **not specified in OMG DDS-Security 1.2**. The
entire mechanism runs in the `zerodds:` namespace and is ignored by foreign
vendors. A Cyclone peer would:

* Silently ignore the `org.zerodds.sec.delegation_chain` SPDP property
  and perceive the gateway peer only with its own identity.
* Samples the gateway sends on behalf of edge peers are interpreted as
  written by the gateway itself (from Cyclone's view).
* That is not a security breach: as long as Cyclone finds the gateway
  identity valid, it simply sees the aggregated view —
  the same data, only without finer origin resolution.
