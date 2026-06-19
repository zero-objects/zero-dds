# zerodds-opcua-uacp

OPC-UA Connection Protocol (**UACP**, OPC Foundation Part 6 §7.1) **and the
OPC UA Secure Conversation** (Part 6 §6.7) — the `opc.tcp://` binary message
framing plus the `OPN`/`MSG`/`CLO` SecureChannel chunks for a native OPC-UA
Client/Server stack.

Pure-Rust `no_std + alloc`, `forbid(unsafe_code)`. Reuses the OPC-UA Part 6 §5
binary codec from [`zerodds-opcua-pubsub`](../opcua-pubsub).

## Contents

- `connection` — the 8-byte message header (`MessageType` + `ChunkType` +
  `MessageSize`) plus the connection-setup messages `Hello` / `Acknowledge` /
  `Error` / `ReverseHello`.
- `securechannel` — the `SecureChannel` chunk framing (`OPN`/`MSG`/`CLO`,
  asymmetric/symmetric security headers, `SequenceHeader`,
  `ChannelSecurityToken`), the common `RequestHeader`/`ResponseHeader`, the
  `OpenSecureChannel` service, and `parse_chunk` / `open_incoming`.
- `crypto` (feature `crypto`) — the secured SecurityPolicies
  (`Basic256Sha256`, `Aes128_Sha256_RsaOaep`, `Aes256_Sha256_RsaPss`):
  P-SHA256 key derivation (§6.7.6), AES-128/256-CBC + HMAC-SHA256 symmetric
  chunk security, RSA-OAEP + RSA-PKCS#1-v1.5/PSS asymmetric `OPN` security, and
  the SHA-1 certificate thumbprint. RustCrypto; the caller supplies the CSPRNG.

SecurityMode `None` works without the `crypto` feature; the secured policies
layer on top via `SecuritySession`.

See `docs/spec-coverage/opcua-client-server-1.05.md` for the full spec coverage.
