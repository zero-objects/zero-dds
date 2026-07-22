<!-- SPDX-License-Identifier: Apache-2.0 -->
# Per-topic security granularity

ZeroDDS applies DDS-Security (OMG DDS-SECURITY 1.1) protection **per topic
class**, not only per participant. A single secured participant can carry some
topics in the clear, some signed, and some encrypted at the same time, each
with independently chosen metadata and payload protection. This page describes
the granularity the governance model exposes and how each setting maps onto the
wire.

## Where the granularity lives

Governance is parsed into `crates/security-permissions/src/governance.rs`. A
`Governance` document holds one or more `DomainRule`s; each `DomainRule` holds an
ordered list of `TopicRule`s (the governance `<topic_access_rules>` →
`<topic_rule>` elements). Access decisions run through
`crates/security/src/access_control.rs`, which decides *per topic / per
operation* whether an authenticated participant may read or write.

### `TopicRule` — the per-topic knobs

| Field | Governance element | Effect |
|---|---|---|
| `topic_expression` | `<topics>` | Topic-name pattern; wildcards `*` and `?` (same matcher as permissions, `topic_match.rs`). |
| `enable_discovery_protection` | `<enable_discovery_protection>` | SEDP endpoint discovery for this topic is encrypted. |
| `enable_liveliness_protection` | `<enable_liveliness_protection>` | `PARTICIPANT_MESSAGE` (liveliness) is signed. |
| `enable_read_access_control` | `<enable_read_access_control>` | A reader must hold a matching permissions grant. |
| `enable_write_access_control` | `<enable_write_access_control>` | A writer must hold a matching permissions grant. |
| `metadata_protection_kind` | `<metadata_protection_kind>` | Protection of the RTPS submessage metadata — the `SEC_PREFIX`/`SEC_POSTFIX` wrapping. |
| `data_protection_kind` | `<data_protection_kind>` | Protection of the serialized payload — the `SEC_BODY` transform. |

Matching is by topic name against `topic_expression`; the first matching
`TopicRule` in the domain decides. A topic with no explicit rule falls under the
wildcard (`topic_expression: "*"`) default, which is fully permissive/unprotected
unless the governance says otherwise.

### `ProtectionKind` — what protection each knob can request

`metadata_protection_kind` and `data_protection_kind` each take one of five
kinds (`governance.rs`):

| Kind | Cryptographic effect |
|---|---|
| `None` | No transform. |
| `Sign` | Integrity only — a shared-key HMAC/MAC over the protected bytes. |
| `Encrypt` | Integrity + confidentiality — AEAD (the transform's cipher suite). |
| `SignWithOriginAuthentication` | Like `Sign`, plus an additional MAC computed with a per-reader key, so a receiver can prove *which* writer produced the message (defeats a co-tenant replaying another writer's MAC). |
| `EncryptWithOriginAuthentication` | Like `Encrypt`, plus the per-reader origin MAC. |

## How a topic setting reaches the wire

For a secured writer on a topic, the two protection kinds act at two different
layers of the RTPS message:

- **`data_protection_kind` → `SEC_BODY`.** The serialized payload (the CDR of
  the sample) is passed through the crypto transform per sample. `Encrypt`
  replaces the payload with AEAD ciphertext plus a MAC tag; `Sign` appends a MAC
  over the cleartext payload. This is the per-sample cost that scales with
  payload size.
- **`metadata_protection_kind` → `SEC_PREFIX`/`SEC_POSTFIX`.** The RTPS
  submessage (its header and, for `Encrypt`, the whole submessage body) is
  wrapped so an observer cannot read or forge the writer's sequence numbers,
  key hashes, and inline QoS. This cost is (mostly) per submessage, not per
  payload byte.
- **Origin-authentication variants** add one MAC *per matched remote reader* on
  top of the common transform, so their marginal cost grows with the reader
  fan-out of the topic.

Because each topic carries its own `TopicRule`, these costs are paid only on the
topics that ask for them. An unprotected control topic on a secured participant
pays nothing; a high-value command topic can run `Encrypt`/`Encrypt` with
origin authentication — in the same domain, over the same participant.

## Overhead

The qualitative cost model is fixed by the design above:

- `None`: zero.
- `Sign`: one MAC per sample (data) and/or per submessage (metadata); the wire
  grows by the tag length; no confidentiality work.
- `Encrypt`: AEAD over the payload (data) and/or submessage (metadata); the wire
  grows by the AEAD tag; cost scales with the protected byte count.
- `*WithOriginAuthentication`: the above plus one extra MAC per remote reader.

### Measured — payload protection (`data_protection_kind` → SEC_BODY)

`Encrypt` measured against the `None` floor on the Linux bench host (AMD Ryzen
Threadripper PRO 3955WX, `ring` backend), median of 100 Criterion samples, via
`cargo bench -p zerodds-security-crypto --bench crypto_overhead`:

| Payload | None (copy) | Encrypt AES-128-GCM | Encrypt AES-256-GCM |
|---|---|---|---|
| 64 B | 10.3 ns | 560 ns | 564 ns |
| 256 B | 11.2 ns | 606 ns | 621 ns |
| 1 KiB | 18.3 ns | 828 ns | 874 ns |
| 4 KiB | 52.1 ns | 1.48 µs | 1.57 µs |
| 16 KiB | 194 ns | 4.44 µs (3.43 GiB/s) | 4.77 µs (3.20 GiB/s) |

`Encrypt` carries a ~0.55 µs per-sample floor (AEAD key setup + tag + framing)
that dominates small samples, then settles to ~3.2–3.4 GiB/s of AEAD throughput
for large ones; AES-256 costs ~5–8 % more than AES-128 (more rounds). So
per-topic `Encrypt` is cheap in absolute terms but not free per sample — a
high-rate topic of tiny samples pays the fixed floor on every write, which is
exactly why choosing protection **per topic** rather than per participant
matters: the control topics that do not need it pay nothing.

`Sign` (integrity-only) protection runs through the submessage-MAC path, not the
SEC_BODY payload transform, so it is not in this table; its per-sample cost is
below `Encrypt` (a single MAC, no cipher). Numbers for the `aws-lc-rs` /
wolfCrypt backends come from the same bench compiled with `--features aws-lc` /
`wolfcrypt`.

## Related

- Cross-vendor secured interop matrix: `docs/security/` (Cyclone / Fast-DDS /
  OpenDDS).
- Crypto backends (ring / aws-lc-rs / wolfCrypt, `fips` umbrella):
  `crates/security-crypto`.
