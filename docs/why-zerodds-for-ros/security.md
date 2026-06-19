# DDS-Security / SROS2

← [Back to overview](index.md)

## The pain

DDS-Security (SROS2) is powerful but brittle to set up (**22 reports**). The
recurring failure is that **turning security on breaks discovery or
communication**, often with little diagnostic, plus genuine correctness gaps:

- Enabling discovery encryption *and* topic-level protection together stops
  endpoints from matching.
- Security with the Micro XRCE-DDS agent makes discovery fail outright.
- Security enclave overrides don't take effect — `ros2 node list` returns only
  system topics with security on.
- Incomplete privilege inheritance has produced actual security vulnerabilities.

### Most recent example

**[Fast-DDS#5753 — "Discovery Matching fails when discovery_protection_kind=
ENCRYPT and topic-level protection are both enabled"](https://github.com/eProsima/Fast-DDS/issues/5753)**
(2025-04-08). Two standard, supported security settings combined cause endpoints
to stop matching — security configuration as a discovery-breaker.

### Reference list (most recent)

| Date | Source | Problem |
|---|---|---|
| 2025-04-08 | [Fast-DDS#5753](https://github.com/eProsima/Fast-DDS/issues/5753) | Discovery encryption + topic protection → no match |
| 2025-03-13 | [Fast-DDS#5707](https://github.com/eProsima/Fast-DDS/issues/5707) | Security + Micro XRCE agent → discovery fails |
| 2024-08-07 | [ros2#1589](https://github.com/ros2/ros2/issues/1589) | Incomplete privilege inheritance → vulnerability |
| 2024-05-08 | [sros2#306](https://github.com/ros2/sros2/issues/306) | Enclave override ineffective; only system topics visible |
| 2024-04-17 | [sros2#293](https://github.com/ros2/sros2/issues/293) | Node list empty with security enabled |

## How ZeroDDS solves it

**A complete DDS-Security 1.2 implementation, tested as a cross-vendor matrix —
so "security on" is a tested configuration, not a cliff.**

- **Full DDS-Security 1.2.** Authentication, access control, cryptographic,
  logging and data-tagging built-in plugins are all implemented (including CRL
  and a conformance matrix). Security is not an afterthought layer that desyncs
  with discovery — it is part of the audited stack.
- **Secured discovery is a regression cell, not a surprise.** The exact "encrypt
  discovery + protect topics" combinations that break elsewhere are cells in
  ZeroDDS's cross-vendor security matrix, exercised against Cyclone, Fast DDS and
  OpenDDS. The secured handshake (authentication, key exchange, secured SEDP/data)
  is e2e-tested.
- **Profiles, not raw plumbing.** A `SecurityProfile` plus a
  `runtime_create_secure` FFI entry point turns security on through a defined
  surface, rather than hand-assembling enclaves and governance/permissions XML
  whose mistakes fail silently.
- **Memory-safe by construction.** The privilege/parse paths run in safe Rust
  with explicit bounds — the class of memory-safety vulnerabilities behind
  ([ros2#1589](https://github.com/ros2/ros2/issues/1589)) is not expressible in
  the safe core.

> **Honest status:** secured *cross-vendor* interop is broad but not yet 100 %
> green in every cell (e.g. specific OpenDDS secured-SEDP decode paths are still
> being closed). Where a secured cell is verified we say so; the open cells are
> tracked, not hidden.

## Why it no longer has to be a pain

The security cluster is *security configuration that silently breaks discovery*.
ZeroDDS implements the full spec and treats the dangerous combinations as
explicit regression tests across vendors, exposed through a profile API — so
turning security on is a supported, tested step instead of a separate debugging
project.

## Reproduce it yourself

```bash
# Secured runtime via the profile + FFI entry point; cross-vendor secured matrix
# harness exercises encrypt-discovery + topic-protection combinations.
```

→ [Back to overview](index.md) · Next: [Performance](performance.md)
