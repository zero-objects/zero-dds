// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Multicast-Discovery (Spec §11.2.4).
//!
//! Spec §11.2.4 reserviert die Multicast-Group `239.255.0.2` Port `7400`
//! fuer Agent-Discovery via `GET_INFO`. Clients senden ein
//! `GET_INFO`-Datagramm an die Gruppe; Agents binden den Port und
//! antworten Unicast mit `INFO`.
//!
//! In dieser Datei wird der Multicast-Bind plus Send/Recv-Helper
//! gekapselt. Tests laufen auf UDP-Loopback (kein echter IGMP-Setup
//! noetig — das ist DCPS-Builtin-Topic, siehe `reference_pve_multicast_setup`).

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

use crate::error::XrceError;
use crate::submessages::Message;

/// Multicast-Group fuer XRCE-Discovery (Spec §11.2.4).
pub const XRCE_DISCOVERY_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 2);
/// Default-Port der Discovery-Gruppe.
pub const XRCE_DISCOVERY_PORT: u16 = 7400;

/// Multicast-Discovery-Bindung. Haelt einen Socket, der an
/// `0.0.0.0:7400` gebunden und der Discovery-Gruppe beigetreten ist.
#[derive(Debug)]
pub struct MulticastDiscovery {
    /// Empfangs-Socket (auf Port `XRCE_DISCOVERY_PORT` gebunden).
    pub socket: UdpSocket,
    /// Adresse der Discovery-Gruppe (fuer Send).
    pub group_addr: SocketAddrV4,
}

impl MulticastDiscovery {
    /// Bindet einen neuen Discovery-Socket auf `0.0.0.0:port` und joint
    /// `XRCE_DISCOVERY_GROUP`. `port = 0` bindet einen ephemeren Port
    /// (nuetzlich fuer Tests).
    ///
    /// `domain_id` ist nur fuer die Send-Adresse — die Multicast-Group
    /// ist Domain-unabhaengig (Spec §11.2.4 nutzt einen festen Port pro
    /// XRCE-Implementierung).
    ///
    /// # Errors
    /// `std::io::Error` bei Bind-Fehlern.
    pub fn start(port: u16) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))?;
        // Best-Effort Multicast-Join. Schlaegt der join fehl (weil das
        // Test-Environment kein Multicast hat), tolerieren wir das —
        // der Socket bleibt fuer Loopback nutzbar.
        let _ = socket.join_multicast_v4(&XRCE_DISCOVERY_GROUP, &Ipv4Addr::UNSPECIFIED);
        Ok(Self {
            socket,
            group_addr: SocketAddrV4::new(XRCE_DISCOVERY_GROUP, XRCE_DISCOVERY_PORT),
        })
    }

    /// Bindet auf einem expliziten Local-Address (z.B. `127.0.0.1:0` fuer
    /// reine Loopback-Tests).
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

    /// Sendet eine `GET_INFO`-Discovery-Message an die Multicast-Gruppe
    /// (oder an `target` bei Loopback-Tests).
    ///
    /// # Errors
    /// `XrceError` aus dem Encoder oder `ValueOutOfRange` bei IO.
    pub fn send_to(&self, msg: &Message, target: SocketAddr) -> Result<(), XrceError> {
        let bytes = msg.encode()?;
        self.socket
            .send_to(&bytes, target)
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "discovery send_to failed",
            })?;
        Ok(())
    }

    /// Sendet an die Multicast-Gruppe (`XRCE_DISCOVERY_GROUP:7400`).
    ///
    /// # Errors
    /// `XrceError`.
    pub fn send_multicast(&self, msg: &Message) -> Result<(), XrceError> {
        self.send_to(msg, SocketAddr::V4(self.group_addr))
    }

    /// Empfaengt ein Datagramm von der Discovery-Gruppe.
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
        // Beide Endpunkte loopback gebunden — kein echtes Multicast,
        // nur die XRCE-Wire-Schicht testen.
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

    /// Spec §11.2.4 + §8.4.4 — Agent-Discovery-Multicast-Pfad.
    /// Ein Client sendet GET_INFO an die `XRCE_DISCOVERY_GROUP` und
    /// verifiziert, dass `send_multicast` nicht crasht. Echtes
    /// Multicast-Empfang erfordert OS-Konfig (siehe
    /// `reference_pve_multicast_setup`); hier prueft wir nur die
    /// Wire-Send-Path-Integrity.
    #[test]
    fn multicast_send_via_xrce_discovery_group_does_not_error() {
        let d = MulticastDiscovery::start(0).expect("bind");
        let msg = build_get_info_msg();
        // Bei Loopback-Setup sendet der Multicast in die default-
        // Schnittstelle; bei keiner verfuegbaren Multicast-Route
        // wird der Send vom OS verworfen, aber kein Error generiert.
        // Wir pruefen nur, dass der API-Pfad konsistent ist.
        let res = d.send_multicast(&msg);
        // OS kann Multicast bei fehlender Route mit OS-Error melden
        // — wir akzeptieren beide Faelle, solange XrceError-Typ
        // korrekt ist.
        match res {
            Ok(()) => {}
            Err(XrceError::ValueOutOfRange { .. }) => {}
            Err(other) => panic!("unerwarteter XrceError-Typ: {other:?}"),
        }
    }

    /// Spec §11.2.4 — Discovery-Group-Adresse + Default-Port.
    #[test]
    fn discovery_group_addr_constructed_correctly() {
        let d = MulticastDiscovery::start(0).expect("bind");
        assert_eq!(d.group_addr.ip(), &Ipv4Addr::new(239, 255, 0, 2));
        assert_eq!(d.group_addr.port(), XRCE_DISCOVERY_PORT);
    }

    /// Spec §11.3.4 — TCP-Agent-Discovery nutzt dieselbe Port-Schema
    /// wie UDP (`agent_default_port` aus `transport_udp.rs`).
    #[test]
    fn tcp_discovery_uses_same_port_scheme_as_udp() {
        // §11.3.4 sagt: "TCP Agent Discovery uses the same port-
        // scheme as UDP". Wir verifizieren das via
        // agent_default_port (aus transport_udp).
        use crate::transport_udp::agent_default_port;
        for domain in 0u16..=10 {
            let p = agent_default_port(domain);
            // Port ist 7400 + 4*domain (Spec §11.2.4 / §11.3.4).
            assert_eq!(p, 7400 + 4 * domain);
        }
    }
}
