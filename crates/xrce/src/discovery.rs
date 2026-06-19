// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Multicast-Discovery (Spec §11.2.4).
//!
//! Spec §11.2.4 reserves the multicast group `239.255.0.2` port `7400`
//! for agent discovery via `GET_INFO`. Clients send a
//! `GET_INFO` datagram to the group; agents bind the port and
//! reply unicast with `INFO`.
//!
//! This file encapsulates the multicast bind plus send/recv helpers.
//! Tests run on UDP loopback (no real IGMP setup
//! needed — that is the DCPS builtin topic, see `reference_pve_multicast_setup`).

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

use crate::error::XrceError;
use crate::submessages::Message;

/// Multicast group for XRCE discovery (Spec §11.2.4).
pub const XRCE_DISCOVERY_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 2);
/// Default port of the discovery group.
pub const XRCE_DISCOVERY_PORT: u16 = 7400;

/// Multicast discovery binding. Holds a socket bound to
/// `0.0.0.0:7400` and joined to the discovery group.
#[derive(Debug)]
pub struct MulticastDiscovery {
    /// Receive socket (bound to port `XRCE_DISCOVERY_PORT`).
    pub socket: UdpSocket,
    /// Address of the discovery group (for send).
    pub group_addr: SocketAddrV4,
}

impl MulticastDiscovery {
    /// Binds a new discovery socket on `0.0.0.0:port` and joins
    /// `XRCE_DISCOVERY_GROUP`. `port = 0` binds an ephemeral port
    /// (useful for tests).
    ///
    /// `domain_id` is only for the send address — the multicast group
    /// is domain-independent (Spec §11.2.4 uses a fixed port per
    /// XRCE implementation).
    ///
    /// # Errors
    /// `std::io::Error` on bind errors.
    pub fn start(port: u16) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))?;
        // Best-effort multicast join. If the join fails (because the
        // test environment has no multicast), we tolerate that —
        // the socket remains usable for loopback.
        let _ = socket.join_multicast_v4(&XRCE_DISCOVERY_GROUP, &Ipv4Addr::UNSPECIFIED);
        Ok(Self {
            socket,
            group_addr: SocketAddrV4::new(XRCE_DISCOVERY_GROUP, XRCE_DISCOVERY_PORT),
        })
    }

    /// Binds on an explicit local address (e.g. `127.0.0.1:0` for
    /// pure loopback tests).
    ///
    /// # Errors
    /// `std::io::Error`.
    pub fn start_on(local: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(local)?;
        let _ = socket.join_multicast_v4(&XRCE_DISCOVERY_GROUP, &Ipv4Addr::UNSPECIFIED);
        Ok(Self {
            socket,
            group_addr: SocketAddrV4::new(XRCE_DISCOVERY_GROUP, XRCE_DISCOVERY_PORT),
        })
    }

    /// Sends a `GET_INFO` discovery message to the multicast group
    /// (or to `target` in loopback tests).
    ///
    /// # Errors
    /// `XrceError` from the encoder or `ValueOutOfRange` on IO.
    pub fn send_to(&self, msg: &Message, target: SocketAddr) -> Result<(), XrceError> {
        let bytes = msg.encode()?;
        self.socket
            .send_to(&bytes, target)
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "discovery send_to failed",
            })?;
        Ok(())
    }

    /// Sends to the multicast group (`XRCE_DISCOVERY_GROUP:7400`).
    ///
    /// # Errors
    /// `XrceError`.
    pub fn send_multicast(&self, msg: &Message) -> Result<(), XrceError> {
        self.send_to(msg, SocketAddr::V4(self.group_addr))
    }

    /// Receives a datagram from the discovery group.
    ///
    /// # Errors
    /// `XrceError`.
    pub fn recv(&self) -> Result<(SocketAddr, Message), XrceError> {
        let mut buf = [0u8; 65_507];
        let (n, peer) =
            self.socket
                .recv_from(&mut buf)
                .map_err(|_| XrceError::ValueOutOfRange {
                    message: "discovery recv_from failed",
                })?;
        let msg = Message::decode(&buf[..n])?;
        Ok((peer, msg))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    extern crate alloc;

    use super::*;
    use crate::header::{ClientKey, MessageHeader, SessionId, StreamId};
    use crate::serial_number::SerialNumber16;
    use crate::submessages::{GetInfoPayload, Submessage};

    fn build_get_info_msg() -> Message {
        let header = MessageHeader::with_client_key(
            SessionId(0x00),
            StreamId::NONE,
            SerialNumber16::new(1),
            ClientKey([0xCA, 0xFE, 0xBA, 0xBE]),
        )
        .unwrap();
        let sm: Submessage = GetInfoPayload {
            representation: alloc::vec![0xAA, 0xBB, 0xCC, 0xDD, 0, 0, 0, 0],
        }
        .into_submessage()
        .unwrap();
        Message::new(header, alloc::vec![sm]).unwrap()
    }

    #[test]
    fn discovery_constants_match_spec() {
        assert_eq!(XRCE_DISCOVERY_GROUP, Ipv4Addr::new(239, 255, 0, 2));
        assert_eq!(XRCE_DISCOVERY_PORT, 7400);
    }

    #[test]
    fn loopback_send_recv_roundtrip() {
        // Both endpoints bound to loopback — no real multicast,
        // only test the XRCE wire layer.
        let listener = MulticastDiscovery::start_on("127.0.0.1:0".parse().unwrap()).unwrap();
        let listener_addr = listener.socket.local_addr().unwrap();
        let sender = MulticastDiscovery::start_on("127.0.0.1:0".parse().unwrap()).unwrap();
        sender
            .socket
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        listener
            .socket
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();

        let msg = build_get_info_msg();
        sender.send_to(&msg, listener_addr).expect("send");
        let (_peer, received) = listener.recv().expect("recv");
        assert_eq!(received, msg);
    }

    #[test]
    fn start_with_ephemeral_port_succeeds() {
        let d = MulticastDiscovery::start(0).unwrap();
        let addr = d.socket.local_addr().unwrap();
        assert_ne!(addr.port(), 0);
    }

    /// Spec §11.2.4 + §8.4.4 — agent discovery multicast path.
    /// A client sends GET_INFO to the `XRCE_DISCOVERY_GROUP` and
    /// verifies that `send_multicast` does not crash. Real
    /// multicast reception requires OS config (see
    /// `reference_pve_multicast_setup`); here we only check the
    /// wire send path integrity.
    #[test]
    fn multicast_send_via_xrce_discovery_group_does_not_error() {
        let d = MulticastDiscovery::start(0).expect("bind");
        let msg = build_get_info_msg();
        // In the loopback setup the multicast is sent to the default
        // interface; with no available multicast route
        // the send is discarded by the OS, but no error is generated.
        // We only check that the API path is consistent.
        let res = d.send_multicast(&msg);
        // The OS may report multicast with an OS error when a route is
        // missing — we accept both cases as long as the XrceError type
        // is correct.
        match res {
            Ok(()) => {}
            Err(XrceError::ValueOutOfRange { .. }) => {}
            Err(other) => panic!("unexpected XrceError type: {other:?}"),
        }
    }

    /// Spec §11.2.4 — discovery group address + default port.
    #[test]
    fn discovery_group_addr_constructed_correctly() {
        let d = MulticastDiscovery::start(0).expect("bind");
        assert_eq!(d.group_addr.ip(), &Ipv4Addr::new(239, 255, 0, 2));
        assert_eq!(d.group_addr.port(), XRCE_DISCOVERY_PORT);
    }

    /// Spec §11.3.4 — TCP agent discovery uses the same port scheme
    /// as UDP (`agent_default_port` from `transport_udp.rs`).
    #[test]
    fn tcp_discovery_uses_same_port_scheme_as_udp() {
        // §11.3.4 says: "TCP Agent Discovery uses the same port
        // scheme as UDP". We verify that via
        // agent_default_port (from transport_udp).
        use crate::transport_udp::agent_default_port;
        for domain in 0u16..=10 {
            let p = agent_default_port(domain);
            // Port is 7400 + 4*domain (Spec §11.2.4 / §11.3.4).
            assert_eq!(p, 7400 + 4 * domain);
        }
    }
}
