# 0010 — CoAP-OSCORE (RFC 8613) — accepted, implemented (supersedes 0007)

- **Status:** accepted (supersedes [0007](0007-coap-oscore-rejected-rc1.md))
- **Date:** 2026-06-11
- **Context:** `crates/coap-bridge`, spec `zerodds-coap-bridge-1.0` §7.2,
  spec completeness program (group F2 — "extra mile")

## Context

ADR 0007 classified OSCORE for RC1 as `rejected/n/a` — justified solely
by missing market pull (DTLS covers 100% of the addressed production cases).
That was an **RC1 scoping decision**, not a technical impossibility.

The spec completeness program explicitly overrides this scoping decision:
optional spec profiles (here §7.2 OSCORE) are a
**differentiating feature**, not a reason to reject ("other vendors do not have
it" does not count). So for OSCORE, as for every spec mechanism:
**implement fully, no stub, no hidden TODO** — alternative 1, rejected in ADR 0007,
is now the chosen one.

## Decision

OSCORE is **implemented fully**, starting with the security context
key derivation and building up to the AEAD protect/unprotect layer. Correctness
is anchored byte-exactly against the **RFC 8613 Appendix C test vectors** (the
authoritative ground truth, analogous to the cross-vendor capture methodology in the
DDS Security and XCDR2 stack).

## Architecture

`crates/coap-bridge/src/oscore/` (feature `oscore`, `no_std + alloc`):

| Layer | Status | Verification |
|---|---|---|
| **HKDF-SHA-256** (RFC 5869) | ✅ done | RFC 5869 A.1 (PRK + OKM byte-exact) |
| **Security Context Key Derivation** (§3.2: sender/recipient key + Common IV, CBOR `info`) | ✅ done | RFC 8613 **C.1.1** (empty sender ID, `f0910ed7…`), **C.1.2** (mirror), **C.2.1** (ID context path, `e39a0c7c`/`2ca58fb8`) |
| **AEAD AES-CCM-16-64-128** (COSE alg 10) | ✅ done | `ccm` crate; **RFC 3610 PV#1** byte-exact |
| **Nonce (§5.2) + AAD/Enc_structure (§5.4) + protect/unprotect** | ✅ done | spec-deterministic nonce/AAD vectors + protect→unprotect roundtrip |
| **OSCORE option (9) codec (§6.1)** | ✅ done | RFC 8613 §6.3 option vectors (encode/decode roundtrip) |
| **Message Protect/Unprotect (§8)** + option class split (§4.1.2) | ✅ done | `oscore/message.rs`; protect→unprotect roundtrip (GET + payload + tamper reject) |
| **Replay window** (§3.2.2) | ✅ done | sliding-window unit tests (in-window/out-of-order/replay) |

Crypto building blocks present in the workspace: `hmac`/`sha2` (HKDF, used),
`aes`/`cipher` (AES-CCM substrate), `ciborium` (in case serde CBOR is needed — the
small fixed `info`/`Enc_structure` structures are currently hand-encoded
deterministically). The only new external dependency is `ccm` (AES-CCM mode),
which `ring`/`security-crypto` (AES-GCM) do NOT cover; its inclusion goes through
cargo-deny.

## Alternatives

1. **Stay with ADR 0007 (rejected)** — rejected: contradicts the
   spec completeness mandate (optional profiles are features).
2. **OSCORE as a stub** — rejected (as in 0007): no real security.
3. **Implement fully, anchored in RFC test vectors** — chosen.

## Consequences

Positive:
- §7.2 goes from `rejected` to `implemented` — clean spec status.
- Key derivation is proven conformant right away (RFC C.1/C.2 vectors).
- OSCORE works through proxies (object security survives CoAP proxies, unlike
  hop-by-hop DTLS) — a real differentiating feature for LwM2M/constrained IoT.

**Complete** (2026-06-12): key derivation + AEAD + nonce/AAD + protect/unprotect + option 9 codec + replay window + §8 message pipeline — anchored in RFCs (RFC 5869 A.1, RFC 8613 C.1/C.2, RFC 3610 PV#1). DTLS: separate dossier (ADR 0011).

**Daemon wiring closed**: response protection §8.3/§8.4 in `oscore/message.rs`, `daemon/oscore_gateway.rs` (per-`kid` contexts + replay window + notification PIV sequence), `dispatch` integration in `daemon/server.rs` (unprotected → `4.01`, replay → `4.01`, verify error → `4.00`, responses + Observe notifications protected), `oscore:` config block per spec §3/§7.2, fail-closed without the feature. Correctness: RFC 8613 Appendix C.4-C.8 as full-message vectors byte-exact (`tests/oscore_rfc8613_vectors.rs`) + daemon e2e over UDP (`tests/daemon_oscore_e2e.rs`).

## References

- RFC 8613 — OSCORE (in particular §3.2 Key Derivation, Appendix C test vectors)
- RFC 5869 — HKDF; RFC 8152 — COSE; RFC 8949 — CBOR
- `crates/coap-bridge/src/oscore/mod.rs` — implementation + test vectors
- [0007](0007-coap-oscore-rejected-rc1.md) — previous (superseded) decision
