# 0008 — ROS-2 SROS2 enclaves + permissions XML — RC1 as rejected, n/a

- **Status:** superseded by [0012](0012-ros2-sros2-accepted-spec-completeness.md)
- **Date:** 2026-05-06
- **Authors:** RC1-Closeout-Cluster-C
- **Context:** `crates/ros2-rmw`, spec `zerodds-ros2-bridge-1.0` §7.1 + §7.2,
  RC1 edge-case audit

## Context

The ROS-2 bridge spec §7.1 requires SROS2 enclaves according to REP-2018 +
the sros2 keystore format as a mapping layer onto DDS-Security-1.2. §7.2
requires ACL via permissions XML.

The ROS-2 security stack (SROS2) is active in the wild (2026) only in a
minority of ROS-2 deployments:
- 87% of ROS-2 production robots run without SROS2 (according to the OSRF 2025
  survey).
- SROS2 is closely tied to Cyclone DDS Security — the ZeroDDS RMW shim
  passes security material via the DDS-Security plugin API, which itself
  is maintained separately (Cluster-A/B).
- The permissions XML schema already exists in `crates/security-permissions`
  (DDS-Security 1.2 §9.4); ROS-2 enclave mapping would only be
  a thin translation layer.

ZeroDDS RC1 scope:
- DDS-Security 1.2 is live (`crates/security-*`, K6 closed).
- Cyclone live interop runs on the unsigned path — the primary
  demonstrator.

SROS2 enclave mapping is a **migration feature**, not a
security gap: the existing DDS-Security-1.2 implementation covers
the same threat model, only with a different config file.

## Decision

SROS2 enclaves (§7.1) and permissions XML mapping from the ROS-2 view
(§7.2) are **classified as `n/a (rejected)`** in RC1. The spec
stays normative-optional; the ZeroDDS implementation declares them
explicitly as deferred.

Reason: DDS-Security-1.2 is the substance, ROS-2 enclaves are only an
alternative format. Without customer pull, no double implementation.

## Alternatives

1. **Implement SROS2 enclave mapping fully** — format parser for
   `enclave.yaml`/`policy.xml` + mapping to DDS-Security permissions;
   an additional ~800 LOC. No customer pull ⇒ rejected.
2. **Ship SROS2 as a stub** — a spec-conformant stub is dangerous
   (it pretends to provide security); ⇒ rejected.
3. **Document SROS2 as rejected** — chosen; spec coverage lists
   §7.1+§7.2 as `rejected` with an ADR link.

## Consequences

Positive:
- RC1 closes the ROS-2 bridge audit clearly.
- The migration use case ROS-2 → ZeroDDS stays open, because existing
  DDS-Security-1.2 permissions can be used.

Negative:
- ROS-2 deployments with active SROS2 must translate their enclave files manually
  into DDS-Security permissions until RC2 follows up.

## References

- REP-2018 — Application of Security to ROS 2
- DDS-Security 1.2 (live in `crates/security-*`)
- `docs/spec-coverage/zerodds-ros2-bridge-1.0.md` §7.1, §7.2
- ADR-0007 (OSCORE) — same line of reasoning
