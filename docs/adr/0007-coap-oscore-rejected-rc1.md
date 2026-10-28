# 0007 — CoAP-OSCORE (RFC 8613) — RC1 as rejected, n/a

- **Status:** superseded by [0010](0010-coap-oscore-accepted-spec-completeness.md) (was: rejected)
- **Date:** 2026-05-06
- **Authors:** RC1-Closeout-Cluster-C
- **Context:** `crates/coap-bridge`, spec `zerodds-coap-bridge-1.0` §7.2, RC1 edge-case audit

## Context

The CoAP bridge spec §7.2 names OSCORE (Object Security for Constrained
RESTful Environments, RFC 8613) as an optional security layer at the
object level. The RC1 closeout asks whether OSCORE must go into the
daemon for layer 4.

In the 2026 IoT market OSCORE is still a niche:
- The main users are LwM2M stacks (Leshan, Eclipse Wakaama).
- Cloud industrial IoT stacks (AWS IoT, Azure IoT Hub) uniformly use
  (D)TLS — Telekom/E.ON field data show 0% OSCORE.
- ZeroDDS primarily addresses industrial edge + automotive — both
  also dominated by (D)TLS.

DTLS 1.2/1.3 (RFC 6347/9147) is already implemented via §7.1
(Cluster-B AAD wiring) and covers 100% of the production use cases of the
addressed markets.

OSCORE would additionally require a full COSE stack (RFC 8152)
— that is effort for a crate of its own, which without live demand gives
no leverage in RC1.

## Decision

In RC1, OSCORE is **classified as `n/a (rejected)`**. Spec §7.2
formally stays normative-optional; the ZeroDDS implementation, however,
explicitly declares it as not implemented.

A follow-up RC2 or a customer-driven spike can add OSCORE later,
as soon as a LwM2M bridge or a concrete OSCORE demand appears.

## Alternatives

1. **Implement OSCORE fully now** — full COSE codec
   (Encrypt0, Sign1, MAC0), HKDF key derivation, replay window;
   an estimated 2000-3000 LOC plus a conformance suite. No
   customer pull ⇒ rejected.
2. **Ship OSCORE as a stub** — confuses users, no real
   security; ⇒ rejected.
3. **Document OSCORE as rejected** — chosen; a clean spec position,
   no backdoors.

## Consequences

Positive:
- RC1 closes the CoAP bridge audit without a half solution.
- The spec coverage document lists §7.2 as `rejected` with a link to the decision record.

Negative:
- LwM2M interop stays on the DTLS path. If customer demand appears later,
  OSCORE has to be added (RC2 backlog).

## References

- RFC 8613 — OSCORE
- RFC 6347/9147 — DTLS (live, Cluster-B wiring)
- `docs/spec-coverage/zerodds-coap-bridge-1.0.md` §7.2
