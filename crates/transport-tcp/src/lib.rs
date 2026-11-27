// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-transport-tcp`. Safety classification: **STANDARD**.
//!
//! ZeroDDS TCP transport: RTPS messages over TCP, in one of four
//! dialects ([`TcpDialect`]). The dialect `zerodds`, the default, has a
//! length-prefix framing and the ZeroDDS 16-byte handshake; the dialect
//! `rtps` has neither and delimits each message by the messageLength of
//! its HeaderExtension; the dialect `cyclone` is the stream of Cyclone
//! DDS, with its length submessage behind each RTPS header; the dialect
//! `fastdds` is the TCP protocol of Fast DDS (RTCP), with its header,
//! bind and logical ports. A transport opens connections in one dialect
//! (a destination of Fast DDS in `fastdds`) and finds the dialect of a
//! connection it accepts at its first octets.
//!
//! ## Spec
//!
//! - **ZeroDDS TCP Transport 1.0**,
//!   `docs/spec-coverage/zerodds-tcp-transport-1.0.md`: framing (§2),
//!   handshake (§3), the dialect of Cyclone DDS and the detection of a
//!   connection's dialect (§4), connection pool (§5). In the dialect
//!   `zerodds` the handshake is mandatory on both sides; the dialects
//!   `rtps` (§2.5) and `cyclone` (§4.1) have none.
//! - **DDSI-RTPS 2.5** defines no TCP mapping. §9.3.2.1 names the
//!   locator kinds UDPv4 and UDPv6 only; §9.3.2.4 reserves 3 to
//!   `0x01ffffff` for vendor-specific kinds, where the TCP kinds of this
//!   transport lie (`0x01F0_0004`, `0x01F0_0005`, or 4 and 8 with
//!   [`TcpTransport::with_locator_kinds`]). §9.5 maps RTPS messages to
//!   UDP/IP. §8.3.3 says that a stream transport like TCP would need the
//!   message length in the RTPS HeaderExtension. By default this
//!   transport carries the length in front of the message instead and
//!   leaves the message as it is on UDP; in the dialect
//!   [`TcpDialect::Rtps`] a connection carries the messages back to back
//!   and each is delimited by the messageLength of its HeaderExtension
//!   (§2.5, [`rtps_stream`]).
//!
//! ## Implemented
//!
//! - Dialect `zerodds`: length-prefix frame (4 bytes big-endian, 16 MiB
//!   limit) and the 16-byte ZeroDDS handshake with magic, version, vendor
//!   id, logical port and reason code, under one deadline on each side.
//!   The accepting side refuses a wrong version, a reserved flag, a
//!   vendor that is not on its list, a connection beyond its peer limit
//!   and a logical port that a live connection from the same address
//!   holds or that is above 65535, each with its reason code.
//! - Dialect `rtps`: a stream of RTPS 2.5 messages, each with a
//!   HeaderExtension that states its messageLength ([`rtps_stream`]); no
//!   handshake, no length in front. The vendor list is held against the
//!   vendor id of each RTPS header, and a connection beyond the peer
//!   limit is closed without an answer.
//! - Dialect `cyclone`: the stream of Cyclone DDS ([`cyclone_stream`]).
//!   The transport puts the length submessage `0x81` behind the RTPS
//!   header of each message it writes and takes it out of each it reads;
//!   a message that would be above the 65536 octets Cyclone DDS reads is
//!   refused.
//! - Dialect `fastdds`: the TCP protocol of Fast DDS ([`rtcp`], §6.1). A
//!   14-octet header (`RTCP`, length, checksum, logical port) in front of
//!   each message; the connecting side binds the connection with a
//!   BindConnectionRequest, and either side asks the other with an
//!   OpenLogicalPortRequest before its first message to a logical port.
//!   The control messages of Fast DDS are answered as Fast DDS answers
//!   them. The locators are of the kinds 4 and 8 and hold the logical
//!   port in the upper 16 bits of their port; a destination whose locator
//!   does so is reached in this dialect.
//! - `TcpTransport`: connection pool with reuse, reconnect and backoff,
//!   bounded inbound queue. A listener finds the dialect of a connection
//!   at its first four octets (`ZDDS`, `RTCP`, or `RTPS` and the first
//!   submessage; §4.2). Connections are used in both directions (§4.1):
//!   what a peer writes on an outgoing connection is read, and a send to
//!   the address an accepted connection came from is written on that
//!   connection.
//!
//! ## Cross-vendor interop
//!
//! - ZeroDDS ↔ ZeroDDS: both ends of a connection have the same dialect;
//!   the connecting side sets it per transport, the accepting side
//!   detects it and can be restricted
//!   ([`TcpTransport::with_accepted_dialects`]).
//! - Cyclone DDS: the framing of `ddsi_tcp` on both roles, tested against
//!   octets Cyclone DDS 0.10.5 wrote.
//! - Fast DDS: the dialect `fastdds` in both roles, tested against octets
//!   Fast DDS 3.6.1 wrote and against a running Fast DDS 3.6.1
//!   (`crates/dcps/tests/fastdds_live_tcp.rs`).
//! - RTI Connext: none. Its dialect is an open item.
//!
//! The open items are listed in
//! `docs/spec-coverage/zerodds-tcp-transport-1.0.open.md`.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod cyclone_stream;
pub mod framing;
pub mod handshake;
pub mod rtcp;
pub mod rtps_stream;
pub mod tcp_transport;
// The `WSAPoll` of a read of an accepted connection, the crate's only
// `unsafe` code.
#[cfg(windows)]
mod windows;

pub use tcp_transport::{TcpDialect, TcpTransport, TcpTransportError};
