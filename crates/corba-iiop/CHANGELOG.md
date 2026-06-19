# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), and versioning follows [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the `zerodds-corba-iiop` crate.

### Spec references

- **OMG CORBA 3.3 Part 2**: §14 (IIOP overview), §15.7 (IIOP profile +
  ProfileBody), §15.9 (bidirectional GIOP).

### Public API

- `bidir::{BiDirIiopListenPoint, BiDirIiopServiceContext,
  IIOP_BI_DIR_TAG}` — bidirectional-GIOP negotiation (§15.9).
- `profile_body::{IiopProfileBody, IiopVersion, TaggedComponent}` —
  ProfileBody for all 4 IIOP versions (1.0/1.1/1.2/1.3) including the
  components sequence.
- `acceptor::{Acceptor, AcceptorConfig}` (feature `std`) — TCP
  listener loop with a per-connection worker thread.
- `connection::Connection` — TCP stream wrapper with a frame reader
  (12-byte GIOP header → `message_size` → body read).
- `connector::{Connector, ConnectorConfig}` — client connect with
  connection reuse + thread-safe pool + connect timeout +
  reconnect logic on `CloseConnection`.
- `error::IiopError` — error surface.
- `framing::{read_giop_message, write_giop_message}` — frame codec
  over `corba-giop::Message`.

### Implementation

`#![cfg_attr(not(feature = "std"), no_std)]` with `extern crate alloc`;
`#![forbid(unsafe_code)]`.

`IiopVersion` constants `V1_0`/`V1_1`/`V1_2`/`V1_3`.
The `IiopProfileBody` encoder/decoder respects the version quirks:
1.0 without a components sequence, 1.1 and up with `sequence<TaggedComponent>`.

`Connection` reads GIOP frames byte-exactly and delegates to
`corba-giop::decode_message`. `Acceptor` and `Connector` work on
standard `std::net::TcpStream`.

`BiDirIiopServiceContext` (§15.9) lets a server send requests to a
client endpoint after the client has announced a `ListenPoint` set via
a ServiceContext.

### Architecture

- **Layer:** 8 (CORBA stack, Tier-B).
- **Dependencies (in):** `zerodds-cdr`, `zerodds-corba-giop`.
- **Dependents (out):** `zerodds-corba-ior` (TaggedProfile content),
  `zerodds-corba-dds-bridge` (forwarder).
- **Feature flags:** `std` (default), `alloc` (via std).

### Stability

- Public API: RC1-stable.
- Wire format fixed by OMG.
