# Delegation — gateway/bridge identity for vehicle mesh + loosely coupled systems

> **Status:** Draft v0.1 (2026-04-25)
> **Dependencies:** `08_heterogeneous_security.md` (peer classes, interface routing),
>   `02_architecture.md §3` (transport layers), WP 4H-i (PKI↔crypto integration)

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

The `PeerClassMatch` criteria are extended in stage j-d by
`delegated_by` / `max_delegation_depth`.

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
    /// Who delegates (known in the receiver's trust anchor, or
    /// itself delegated by the next link).
    pub delegator_guid: [u8; 16],
    /// To whom it is delegated.
    pub delegatee_guid: [u8; 16],
    /// Topic patterns the delegatee may serve on behalf of the
    /// delegator. Wildcard semantics like `topic_match` (WP 4.2-c).
    pub allowed_topic_patterns: Vec<String>,
    /// Partition patterns (wildcard).
    pub allowed_partition_patterns: Vec<String>,
    /// Validity window (Unix seconds).
    pub not_before: i64,
    pub not_after: i64,
    /// Signature by the delegator cert over all of the above fields.
    pub signature: Vec<u8>,
}
```

### 5.2 `DelegationChain`

```rust
pub struct DelegationChain {
    /// Original claim bearer — the peer whose identity is in the
    /// sample header. Must === links[0].delegatee_guid.
    pub origin_guid: [u8; 16],
    /// Chain in delegation order. First entry: the gateway
    /// that attests the identity of origin_guid. Subsequent
    /// entries are re-delegations.
    pub links: Vec<DelegationLink>,
}
```

### 5.3 Ephemeral vs. static edge identity

Default: **static GuidPrefix** from the gateway config.

```xml
<!-- gateway config (example) -->
<edge_identities>
  <edge name="lidar-A" guid_prefix="01020304050607080900000a" />
  <edge name="radar-B" guid_prefix="01020304050607080900000b" />
</edge_identities>
```

Optional: **ephemeral identity** per boot cycle or sample batch,
when replay robustness is required. The gateway then generates a
random GuidPrefix and signs it short-lived.

```xml
<edge_identities default_mode="static">
  <edge name="lidar-A" guid_prefix="0102030405060708090...a" />
  <edge name="radar-B" mode="ephemeral" lifetime_seconds="300" />
</edge_identities>
```

* **Static** — simple configuration, stable identity across boots,
  susceptible to replay when message counters are not protected
  along with it.
* **Ephemeral** — a new GuidPrefix + new delegation per
  `lifetime_seconds`. Replay windows are thus extremely narrow. More
  SPDP traffic as the price.

## 6 Chain validation

The receiver validates an incoming sample delegation as follows:

1. **Chain continuity**: for all `i`: `links[i].delegatee_guid == links[i+1].delegator_guid`.
2. **Origin match**: `chain.origin_guid == links[0].delegatee_guid`.
3. **Trust anchor**: the cert with which `links[0].signature` was created must lie in the receiver trust anchor (i.e. the first delegator is known-trusted).
4. **Signature chain**: for all `i > 0`: the signature of `links[i]` must be created by the cert that belongs to `links[i-1].delegatee_guid` (i.e. every next delegator is authorized by the previous one).
5. **Time window**: each `links[i]` with `now ∈ [not_before, not_after]`.
6. **Chain depth**: `chain.links.len() <= max_delegation_depth` from the governance (default **3**, configurable).
7. **Scope cascading**: the current topic/partition must match in **every** `allowed_*_pattern` set of all links — this corresponds to the intersection of all scopes (the narrowest hop wins).

## 7 Configuration

### 7.1 Governance XML — hybrid with named profiles

Delegation rules are defined as **named profiles** at the top level.
Peer classes reference profiles via a `delegation_profile`
attribute. This is DRY (one profile, several peer classes), allows
differentiated policy per class and stays audit-friendly.

```xml
<!-- top level: named delegation profiles (named types) -->
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

<!-- peer classes reference profiles -->
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
    <!-- No delegation_profile → direct auth, no chain expected -->
    <match auth_plugin_class="DDS:Auth:PKI-DH:1.2" />
  </peer_class>
</zerodds:peer_classes>
```

**Parser checks (stage j-h):**
- `delegation_profile="..."` without an existing `<profile name="...">` → warning
  `unreferenced delegation profile "<name>"`.
- `<profile>` without a reference from a peer class → info
  `delegation profile "<name>" unused`.

### 7.2 Signature algorithm

Configured per profile. Supported values (analogous to rustls-webpki):

| Value | Algorithm | Token size | Pros/cons |
|------|-------------|-------------|----------|
| `ecdsa-p256-sha256` (default) | ECDSA P-256 + SHA-256 | ~72 byte | Hardware-accelerated on ARM embedded, smallest TLS-standard signature |
| `ecdsa-p384-sha384` | ECDSA P-384 + SHA-384 | ~104 byte | Higher security, more expensive |
| `rsa-pss-sha256` | RSA-PSS 2048 + SHA-256 | ~256 byte | OMG-DDS-Security mandatory (spec §9.3.2.1); for fleet interop with legacy vendors |
| `ed25519` | Ed25519 (pure) | ~64 byte | Fastest, smallest — not OMG-conformant; for high-performance deployments |

**Rationale for the ECDSA-P-256 default:**
- ARM Cortex-M/A have ECDSA hardware support (no AES-NI equivalent for RSA)
- Token size drives SPDP overhead — small = less beacon fragmentation
- The OMG spec lists it as optional-but-widely-supported

**Switch semantics:** on a signature-algorithm change in a
profile, all active delegations must be regenerated. The gateway
detects the profile reload and regenerates — edge peers report an
announce-refresh event in the process.

### 7.3 Runtime config

```rust
pub struct DelegationConfig {
    /// Named profiles parsed from governance. In-memory lookup by name.
    pub profiles: BTreeMap<String, DelegationProfile>,
    /// Peer class → profile name mapping (from governance).
    pub class_to_profile: BTreeMap<String, String>,
    /// Per edge peer: static or ephemeral identity.
    pub edge_identities: Vec<EdgeIdentityConfig>,
    /// When true: peers without a valid delegation chain are dropped
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

* **Implicit** via gateway failure: when the gateway no longer sends
  SPDP beacons, all its delegations expire after `lease_duration`
  (receivers no longer rely on `not_after`).
* **Explicit** via a gateway command: the gateway can actively withdraw
  a delegation ID. On the next SPDP beacon the gateway pushes
  a `zerodds.sec.revoked_delegations` property (a list of
  `(delegatee_guid, issued_at)` hashes). Receivers add them to a
  local revocation list; samples from these delegatees are
  dropped.
* **Short-lived-first**: ephemeral identities have inherently short
  lifetimes and require no explicit revocation.

## 9 Future extensions (not in WP 4H-j)

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

Delegation is **not specified in OMG DDS-Security 1.1**. The
entire mechanism runs in the `zerodds:` namespace and is ignored by foreign
vendors. A Cyclone peer would:

* Silently ignore the `zerodds.sec.delegation_chain` SPDP property
  and perceive the gateway peer only with its own identity.
* Samples the gateway sends on behalf of edge peers are interpreted as
  written by the gateway itself (from Cyclone's view).
* That is not a security breach: as long as Cyclone finds the gateway
  identity valid, it simply sees the aggregated view —
  the same data, only without finer origin resolution.

## 11 Scope matrix WP 4H-j

| Stage | Topic | LOC (estimate) |
|-------|-------|-----------------|
| 4H-j-a | `DelegationLink`/`DelegationChain` + sign/verify in `security-pki` | ~350 |
| 4H-j-b | Chain validation + scope intersection in `security-permissions` | ~200 |
| 4H-j-c | SPDP propagation (wire format for the chain in WireProperty) | ~200 |
| 4H-j-d | `PeerClassMatch` extensions (`delegated_by`, `max_chain_depth`) | ~200 |
| 4H-j-e | `GatewayBridge` helper (delegate_for / revoke_for / sub-gateway chaining) | ~300 |
| 4H-j-f | Static + ephemeral edge-identity config parser | ~200 |
| 4H-j-g | E2E test hull+turret double star: 2 gateways, 3 edges, 1 C4I | ~400 |
| 4H-j-h | Governance-XML `<zerodds:delegation>` parser + interop test | ~250 |
| **Total** | | **~2100 LOC** |

Expected time effort: 10 working days.
