# ADR 0014 — CoAP DTLS 1.2 on OpenSSL, with raw public keys

- **Status:** accepted
- **Date:** 2026-11-13
- **Supersedes:** ADR 0011 (CoAP DTLS 1.2 via webrtc-dtls)
- **Context:** `crates/coap-bridge` (feature `dtls`), new crate `crates/dtls-openssl`; `coap-rfc-7252` §9, §9a

## Context

RFC 7252 §9 names four security modes and makes NoSec and RawPublicKey
mandatory to implement; RawPublicKey uses RFC 7250 and identifies a key by
its RFC 6920 name (§9.1.3.2.1). The DTLS transport of ADR 0011,
webrtc-dtls 0.12, has neither the client_certificate_type nor the
server_certificate_type extension of RFC 7250; rustls has raw public keys
for TLS 1.3 only, not for DTLS. OpenSSL has RFC 7250 since 3.2
(`SSL_CTX_set1_client_cert_type`, `SSL_CTX_set1_server_cert_type`,
`SSL_get0_peer_rpk`, `X509_STORE_CTX_get0_rpk`).

Measured on the Linux test host (OpenSSL 3.5.6): OpenSSL 3's `DEFAULT`
cipher list holds no CCM suite, so an OpenSSL peer with defaults does not
offer the suites RFC 7252 makes mandatory, and OpenSSL rates the CCM-8
suites below its default security level.

## Decision

The maintainer decided on 2026-11-13 (`internal/release/rc8-plan.md`,
arc 4, decision 6): the DTLS transport of `zerodds-coap-bridge`, server and
proxy client, moves to OpenSSL 3.2 or later through the `openssl` crate.

- A new crate `zerodds-dtls-openssl` runs DTLS 1.2 over UDP: a server on one
  socket that hands each datagram to the session of its source, the cookie
  exchange of RFC 6347 §4.2.1, at most 64 handshakes at a time; a client;
  blocking, a thread per session. It configures pre-shared keys, X.509
  certificates checked against trust anchors and the peer's name, and raw
  public keys checked against RFC 6920 names (`ni:`, `nih:`, binary).
- `src/ffi.rs` of that crate declares what openssl-sys 0.9.118 does not
  bind: the RFC 7250 functions and the security callback. The callback lets
  TLS_PSK_WITH_AES_128_CCM_8 and TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8 pass
  and leaves every other decision to OpenSSL's callback at the context's
  level, instead of lowering the level for the whole context.
- The suites RFC 7252 makes mandatory are offered first, CCM and GCM after
  them for peers whose OpenSSL leaves CCM out.
- `zerodds-coap-bridge` drops webrtc-dtls, webrtc-util, tokio, pem and
  rustls from its `dtls` feature. The daemon serves each DTLS session on a
  thread; notifications and the proxy's separate responses are queued for
  the session and wake its receive.

## Alternatives

1. **Extend webrtc-dtls in a fork of our own** — stays pure Rust; the fork
   and its handshake code would be ours to maintain.
2. **A DTLS 1.2 stack of our own** — full control, the largest effort.
3. **OpenSSL** (chosen) — RFC 7250, the widest suite support, a stack many
   CoAP implementations use (libcoap's OpenSSL backend).

## Consequences

- The `dtls` feature links libssl (3.2 or later) at build and run time;
  `dtls-vendored-openssl` builds OpenSSL from source (`openssl-src`). The
  CI image (Debian trixie) and the Linux test host have OpenSSL 3.5.
- The transport is no longer pure Rust; FIPS goes through the OpenSSL FIPS
  provider.
- The subject of a raw public key in the bridge's ACL is `rpk:` and the
  key's `ni` URI (sha-256).
- `ffi.rs` holds the crate's only `unsafe` code; the coap-bridge stays
  `forbid(unsafe_code)`.
