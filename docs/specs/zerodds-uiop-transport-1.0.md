# `zerodds-uiop-transport` v1.0 — UIOP: GIOP over Unix domain sockets

ZeroDDS vendor spec. Implemented in `crates/corba-iiop` + `crates/corba-interop`.
Authored in the style of the OMG CORBA spec (numbered clauses, RFC 2119 keywords,
conformance profile). Consistent with the DDS-side `zerodds-uds-transport-1.0`
and ADR-0001 (vendor spec strategy).

## Motivation

GIOP/IIOP (OMG CORBA 3.4 §15.7) standardizes only TCP/IP as transport.
For **same-host IPC** (client and server on the same machine) the
loopback TCP path pays unnecessary overhead: IP stack, port allocation, Nagle/ACK logic,
firewall hooks. omniORB and TAO offer proprietary Unix domain socket
transports for this (`giop:unix` or TAO `uiop`) — both vendor-specific and
not interoperable with each other.

ZeroDDS defines **UIOP** as an orthogonal, clearly delimited vendor transport:
GIOP 1.0/1.1/1.2 messages — byte-identical to the IIOP wire — over an
`AF_UNIX` `SOCK_STREAM`. Only the transport substructure changes; the GIOP codec,
CDR, POA dispatch and the codeset negotiation remain unchanged.

## Goals

- **GIOP-wire-identical**: the same Request/Reply/LocateRequest/Fragment bytes
  as over IIOP; only the stream is a `UnixStream` instead of a `TcpStream`.
- **Announcement in the IOR**: a vendor `TaggedComponent` `TAG_ZERODDS_UDS_TRANS`
  carries the socket path; the ProfileBody remains a valid
  `TAG_INTERNET_IOP` profile (host `localhost`, port 0).
- **Graceful ignoring**: foreign ORBs (omniORB/TAO/JacORB) ignore the
  unknown component and use the TCP profile, if present; a pure
  UIOP IOR cannot be called by them (by design, same-host-only).
- **Connection pooling**: UIOP connections are pooled by path like TCP/SSLIOP
  (no connect per call).

## Non-goals

- **Cross-vendor interop over Unix sockets** — the omniORB/TAO `unix` IOR format
  is vendor-proprietary; UIOP targets ZeroDDS↔ZeroDDS same-host. Cross-vendor
  stays IIOP/SSLIOP.
- **Abstract Unix sockets** (Linux `@` namespace) — v1.0 uses filesystem paths.
- **Multi-host** — UIOP is same-host by definition.

## §1 Transport binding

### §1.1 Socket

A UIOP endpoint is an `AF_UNIX`/`SOCK_STREAM` socket, bound to a
filesystem path. The server does `bind()`+`listen()`+`accept()`; the client
`connect()`. The server MUST remove a stale socket file from an earlier run before
`bind()`.

### §1.2 GIOP framing

The GIOP message stream over the `UnixStream` is **byte-identical** to the
IIOP framing (OMG §15.4): 12-byte magic+header (`GIOP`, version, flags,
message type, message size) followed by the CDR body. Fragmentation (§15.4.9),
version honoring (§15.4.1) and codeset negotiation (§13.10) apply unchanged.

### §1.3 Reader/writer split & timeouts

As with the TCP transport, reader/writer halves are formed via `UnixStream::try_clone`.
`SO_RCVTIMEO`/`SO_SNDTIMEO` are set; `TCP_NODELAY` is meaningless on
`AF_UNIX` and MUST be treated as a no-op (no
`setsockopt(IPPROTO_TCP)`).

## §2 IOR announcement

### §2.1 `TAG_ZERODDS_UDS_TRANS`

| Field | Value |
|---|---|
| Tag ID | `0x5A445544` (ASCII `"ZDUD"`) |
| Location | `TaggedComponent` in the `TAG_INTERNET_IOP` ProfileBody (`components`, from IIOP 1.1) |
| `component_data` | CDR encapsulation: byte order octet + `string` (socket path) |

### §2.2 ProfileBody

A UIOP IOR carries a normal `TAG_INTERNET_IOP` ProfileBody with host
`localhost` and port `0` (no usable TCP endpoint), plus optionally
`TAG_CODE_SETS` and the `TAG_ZERODDS_UDS_TRANS`. A ZeroDDS client that finds the
component chooses the UIOP transport; otherwise it falls back to the
TCP profile.

## §3 Conformance

A **UIOP-conformant** ZeroDDS endpoint:

1. publishes UIOP IORs according to §2 (`stringify_object_ref_uds`),
2. accepts incoming GIOP messages over `AF_UNIX` (`CorbaServer::serve_uds` /
   `Acceptor::start_uds`),
3. recognizes `TAG_ZERODDS_UDS_TRANS` in a target IOR and calls via the
   socket path (`IiopCorbaConnection` → `Connector::connect_uds`),
4. treats `TCP_NODELAY` as a no-op and removes stale socket files before `bind`,
5. pools UIOP connections by socket path.

## §4 Implementation mapping

| Spec | Code |
|---|---|
| §1.1/§1.3 Transport | `corba-iiop/src/connection.rs` — `Transport::Uds`, `Connection::from_unix_stream` |
| §1 Server accept | `corba-iiop/src/acceptor.rs` — `Acceptor::start_uds` |
| §3.3/§3.5 Client pool | `corba-iiop/src/connector.rs` — `Connector::connect_uds` (`PoolKey::Uds`) |
| §2 IOR component | `corba-interop/src/runtime.rs` — `TAG_ZERODDS_UDS_TRANS`, `stringify_object_ref_uds`, `serve_uds` |

## §5 Tests

- Unit: `connector::connect_uds_reuses_pooled_connection` (pooling), iiop transport roundtrip.
- E2E: `corba-interop/tests/uiop.rs` — `serve_uds` + IOR component + client detection
  + 5 pooled echo calls over UDS.

## Annex A — Platform notes

UIOP is `#[cfg(unix)]` (Linux/macOS). macOS has a strict `sun_path` limit
(104 bytes); a server SHOULD choose short socket paths. A UDS socket
half-closed by the peer can reject `setsockopt(SO_RCVTIMEO)` with `EINVAL`
(macOS) — a conformant server keeps accepted connections open.
