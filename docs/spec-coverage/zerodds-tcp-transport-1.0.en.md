# ZeroDDS-TCP-Transport 1.0 — Spec Coverage

A ZeroDDS-vendor-specific TCP transport spec. Analogous to Cyclone's
`ddsi_tcp`, FastDDS TCPv4 and RTI DDS TCP. **Not OMG-normative.**

Implementation:

- `crates/transport-tcp/` — TCP transport (length-prefix framing + 16-byte handshake).

| Spec family | Status |
|---|---|
| **OMG-normative** | DDSI-RTPS 2.5 §9.4 (locator kind) + §9.5 (wire mapping) — fully covered; see `ddsi-rtps-2.5.md` section 9.4/9.5 |
| **ZeroDDS-own spec** | length-prefix framing + 16-byte handshake — [`zerodds-tcp-transport-1.0.md`](https://github.com/zero-objects/zero-dds/blob/main/docs/spec-coverage/zerodds-tcp-transport-1.0.md) |

## §1 Scope and spec status

### §1.1 What OMG standardizes

For TCP, DDSI-RTPS 2.5 standardizes only:

- **§9.4.5 LocatorKind**: `LOCATOR_KIND_TCPv4 = 4`, `LOCATOR_KIND_TCPv6 = 8`.
- **§9.5 wire mapping**: the RTPS message format (RTPS header + submessages)
  is identical to the UDP PSM. TCP adds stream framing, which the spec does
  not further standardize.

ZeroDDS implementation:

- `crates/rtps/src/wire_types.rs` — LocatorKind constants.
- `crates/transport-tcp/src/framing.rs` — RTPS-frame mapping onto the stream.

### §1.2 What OMG does not standardize

- **Handshake / BindConnection**: there is no OMG standard for TCP
  connection bring-up. Vendors have their own formats:
  - **Cyclone DDS**: no handshake; raw RTPS frames with a length prefix.
  - **FastDDS**: BindConnectionRequest/Response as RTPS submessages
    `0x71`/`0x72` (vendor-specific).
  - **RTI DDS**: a TLS-oriented handshake (vendor-specific).
- **Logical-port multiplexing**: vendor-specific.
- **Connection-pool reuse policies**: not spec-relevant.

### §1.3 ZeroDDS choice

The ZeroDDS TCP transport defines its **own spec** (this file) for the
bring-up handshake. The spec is:

- **OMG-conformant** at the locator + wire-frame level (§9.4+§9.5).
- **Vendor-specific** at the handshake level (analogous to
  Cyclone/FastDDS/RTI).
- **Cross-vendor-capable** with Cyclone via a handshake-skip mode.

## §2 Wire format

### §2.1 Length-prefix frame (DDSI-RTPS §9.5-conformant)

Each RTPS datagram is framed over TCP as follows:

```text
+---------+---------+---------+---------+
| length (u32, big-endian)              |   frame length in bytes (excl. the header itself)
+---------+---------+---------+---------+
| RTPS frame body (length bytes)        |   as in the UDP PSM
+---------+---------+---------+---------+
```

- **length field**: 4 bytes big-endian.
- **RTPS frame body**: a complete RTPS message (RTPS header + submessages),
  identical to the UDP PSM (DDSI-RTPS §9.5).

Repo anchors: `crates/transport-tcp/src/framing.rs::encode_frame`,
`decode_frame`. Tests: `crates/transport-tcp/src/framing.rs::tests`.

### §2.2 Maximum frame size

Default: 64 MiB. The receiver MUST reject frames > limit (DoS protection).
Repo anchor: `MAX_FRAME_SIZE` in `framing.rs`.

## §3 Handshake (ZeroDDS-specific)

### §3.1 BindConnectionRequest (16 bytes, big-endian)

```text
+---------+---------+---------+---------+
|  'Z'    |  'D'    |  'D'    |  'S'    |   magic "ZDDS"
+---------+---------+---------+---------+
| v_major | v_minor |  vendor_id (2 B)  |   protocol version + vendor id
+---------+---------+---------+---------+
|               flags (u32)             |   reserved, = 0
+---------+---------+---------+---------+
|            logical_port (u32)         |   DDS endpoint logical port
+---------+---------+---------+---------+
```

- **Magic** `"ZDDS"` — does not collide with the RTPS magic `"RTPS"`, the
  HTTP magic `"HTTP"`, or a TLS ClientHello — an immediate discriminator.
- **Version**: currently `(1,0)`.
- **VendorId**: ZeroDDS = `[0x01, 0x0F]`.
- **Flags**: reserved, MUST be `0`.
- **LogicalPort**: u32; `0` = "no claim".

Repo anchors: `handshake.rs::BindConnectionRequest`,
`encode_request`, `decode_request`.

### §3.2 BindConnectionResponse (16 bytes, big-endian)

```text
+---------+---------+---------+---------+
|  'Z'    |  'D'    |  'A'    | status  |   magic "ZDA" + accept/reject
+---------+---------+---------+---------+
| v_major | v_minor |  vendor_id (2 B)  |   server version + vendor id
+---------+---------+---------+---------+
|               flags (u32)             |   reserved, = 0
+---------+---------+---------+---------+
|            reason_code (u32)          |   0 on accept, otherwise per §3.3
+---------+---------+---------+---------+
```

- **status byte**:
  - `0x2B` (`'+'`) — accept
  - `0x2D` (`'-'`) — reject

Repo anchors: `handshake.rs::BindConnectionResponse`,
`encode_response`, `decode_response`.

### §3.3 Reject reason codes

| Code | Constant | Meaning |
|---:|---|---|
| 0 | `Unknown` | unclassified reject |
| 1 | `VersionMismatch` | peer version outside the accepted window |
| 2 | `ResourceLimit` | server has reached MAX_PEERS |
| 3 | `LogicalPortConflict` | port already bound |
| 4 | `VendorNotAccepted` | vendor policy rejects the vendor id |

Repo anchor: `handshake.rs::RejectReason`.

### §3.4 Sequence

```text
Client                                    Server
  |                                          |
  |-- BindConnectionRequest (16 B) --------->|   §3.1
  |                                          |   verifies version + magic
  |<--- BindConnectionResponse (16 B) -------|   §3.2
  |                                          |
  |-- length-prefix frame [RTPS] ----------->|   §2.1
  |<-- length-prefix frame [RTPS] -----------|
  |                            ...           |
```

On reject both sides drop the connection. The client side should apply
backoff (see the connection pool in `tcp_transport.rs`).

## §4 Cyclone `ddsi_tcp` compatibility

Cyclone DDS speaks raw RTPS frames over TCP **without a handshake**.
ZeroDDS supports this mode via handshake skip:

- Client side: `TcpTransport::without_handshake()` skips §3 and sends §2.1
  length-prefix frames directly.
- Server side: detects Cyclone mode when the first byte of the frame body is
  not `'Z'` (no ZeroDDS handshake magic) but `'R'` (the first byte of the
  RTPS magic), if the auto-detect feature is enabled.

Repo anchor: `tcp_transport.rs::TcpTransport::without_handshake`.

## §5 Connection pool

Not spec-relevant; an implementation detail. The pool provides connection
reuse, backoff and a MAX_PEERS cap.

Repo anchors: `tcp_transport.rs::TcpTransport`,
`tcp_transport.rs::ConnectionPool`.

## §6 Cross-vendor extension points (optional)

Planned as optional feature flags for future releases, **not an RC1
requirement** (no normative spec):

- `feature = "fastdds-compat"`: BindConnectionRequest/Response as RTPS
  submessages `0x71`/`0x72` (FastDDS format).
- `feature = "rti-compat"`: a TLS-oriented handshake (RTI format).

The extension would extend §3, not replace it — the ZeroDDS-own handshake
stays the default.

## §7 Test coverage

| Spec section | Tests |
|---|---|
| §2.1 length-prefix frame | `framing.rs::tests` (8 tests) |
| §2.2 max frame size | `framing.rs::tests::frame_size_limit` |
| §3.1 BindConnectionRequest | `handshake.rs::tests::request_*` |
| §3.2 BindConnectionResponse | `handshake.rs::tests::response_*` |
| §3.3 reject reasons | `handshake.rs::tests::reject_reason_*` |
| §3.4 handshake sequence | `handshake.rs::tests::client_server_*` |
| §4 Cyclone compat | `tests/loopback.rs::cyclone_compat_*` |
| §5 connection pool | `tcp_transport.rs::tests::pool_*` |

Total: 50 tests (lib) + 5 tests (integration). All green as of the audit
date.

## §8 Status

**Fully covered.** The ZeroDDS TCP transport is a complete, internally
coherent spec; all § sections are implemented and tested. The cross-vendor
extension points (§6) are marked optional.
