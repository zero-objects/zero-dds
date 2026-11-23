# zerodds-transport-tcp

[![docs.rs](https://img.shields.io/docsrs/zerodds-transport-tcp)](https://docs.rs/zerodds-transport-tcp)
[![crates.io](https://img.shields.io/crates/v/zerodds-transport-tcp)](https://crates.io/crates/zerodds-transport-tcp)

ZeroDDS TCP transport: RTPS messages over TCP. Layer 2 (wire implementation).

`std`-only, `forbid(unsafe_code)`, safety class **STANDARD**.

## Spec status

DDSI-RTPS 2.5 defines no TCP mapping: §9.3.2.1 names the locator kinds
UDPv4 and UDPv6 only, §9.5 maps RTPS messages to UDP/IP, and §8.3.3 says
that a stream transport like TCP would need the message length in the RTPS
HeaderExtension. §9.3.2.4 reserves the locator kinds 3 to `0x01ffffff` for
vendors.

This transport is specified by **ZeroDDS TCP Transport 1.0**,
[`docs/spec-coverage/zerodds-tcp-transport-1.0.md`](../../docs/spec-coverage/zerodds-tcp-transport-1.0.md):
a 4-byte length in front of every RTPS message (§2), a 16-byte handshake
before the first frame (§3) and a connection pool (§5). The RTPS message
itself is the one of the UDP mapping. The locator kinds are `0x01F0_0004`
(TCPv4) and `0x01F0_0005` (TCPv6); `TcpTransport::with_locator_kinds`
selects 4 and 8, the values of Cyclone DDS and Fast DDS (§1.1).

## What this crate provides

- `TcpTransport`: `Transport` trait implementation with a connection pool,
  `serve` (one reading thread per accepted connection) and the counters
  `dropped_frames`, `truncated_frames`, `connection_errors`,
  `refused_connections`, `refused_unanswered`, `connect_rejections`,
  `peer_closed_connections`; the settings `with_locator_kinds`,
  `with_accepted_vendors`, `with_accept_limit`, `with_logical_port_claim`
- `TcpTransportError`: typed errors
- `framing`: length-prefix frame encoder and decoder (§2)
- `rtps_stream` and `TcpDialect::Rtps`: a stream of RTPS messages with no
  handshake, each delimited by the messageLength of its HeaderExtension
  (DDSI-RTPS 2.5 §8.3.3; §2.5)
- `cyclone_stream` and `TcpDialect::Cyclone`: the stream of Cyclone DDS
  (`ddsi_tcp`), RTPS messages with no handshake, each with the length
  submessage `0x81` behind its RTPS header; the transport puts it in and
  takes it out (§4.1)
- Detection of the dialect of an accepted connection at its first octets
  (`with_accepted_dialects`, §4.2), and connections used in both
  directions: a send to the address an accepted connection came from is
  written on that connection, and what a peer writes on an outgoing
  connection is read (§4.1)
- `handshake`: BindConnection request and response (§3); the accepting
  side decides a request against a `ServerPolicy` (version, reserved
  flags, vendor id, peer limit, logical-port claim) and answers Accept or
  a Reject with its reason code

## Cross-vendor interop

| Peer | Status |
|---|---|
| ZeroDDS ↔ ZeroDDS | handshake and RTPS frames, both roles |
| ZeroDDS ↔ Cyclone DDS | the framing of `ddsi_tcp` on both roles, tested against octets Cyclone DDS 0.10.5 wrote (§4.1) |
| ZeroDDS ↔ Fast DDS | open |
| ZeroDDS ↔ RTI Connext | open |

The open items are listed in
[`docs/spec-coverage/zerodds-tcp-transport-1.0.open.md`](../../docs/spec-coverage/zerodds-tcp-transport-1.0.open.md).

## Tests

```bash
cargo nextest run --profile merge -p zerodds-transport-tcp --all-targets
```

The tests of each spec section are named in
[`zerodds-tcp-transport-1.0.md`](../../docs/spec-coverage/zerodds-tcp-transport-1.0.md).

## License

Apache-2.0 OR MIT — see the workspace root.
