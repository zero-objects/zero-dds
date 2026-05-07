// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE UDP-Transport-Mapping (Spec §11.2).
//!
//! Default-Port-Schema (Task-Spezifikation C6.2.A):
//! - Agent: `7400 + 4 * domain_id + 0`
//! - Client: `7400 + 4 * domain_id + 1`
//! - Multicast: keiner per Default (XRCE ist Unicast zwischen Client+Agent;
//!   Multicast-Discovery bei `239.255.0.2:7400` gemaess §11.2.4 ist
//!   out-of-scope fuer C6.2.A).
//!
//! UDP/IP-Payload = exakt eine XRCE-Message (Spec §11.2.3, keine
//! Envelopes).

use std::net::{SocketAddr, UdpSocket};

use crate::error::XrceError;
use crate::submessages::{DOSC_MAX_PAYLOAD_SIZE, Message};

/// Maximales UDP-Datagram. Groesser als `DOSC_MAX_PAYLOAD_SIZE` waere
/// per UDP nicht moeglich, weil das Wire-Limit 65 507 Byte ist.
pub const MAX_DATAGRAM_SIZE: usize = 65_507;

/// Default-Agent-Port `7400 + 4*domain_id`.
#[must_use]
pub fn agent_default_port(domain_id: u16) -> u16 {
    7400u16.saturating_add(domain_id.saturating_mul(4))
}

/// Default-Client-Port `7400 + 4*domain_id + 1`.
#[must_use]
pub fn client_default_port(domain_id: u16) -> u16 {
    7400u16
        .saturating_add(domain_id.saturating_mul(4))
        .saturating_add(1)
}

/// XRCE-UDP-Sender, gebunden an einen lokalen Socket mit fix gesetzter
/// Agent-Adresse als Default-Ziel.
#[derive(Debug)]
pub struct XrceUdpSender {
    /// Lokaler Socket.
    pub socket: UdpSocket,
    /// Default-Ziel (Agent-Adresse).
    pub agent_addr: SocketAddr,
}

impl XrceUdpSender {
    /// Konstruiert mit explizitem lokalen Bind und Agent-Ziel.
    ///
    /// # Errors
    /// `std::io::Error`, wenn Bind fehlschlaegt.
    pub fn bind(local: SocketAddr, agent_addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(local)?;
        Ok(Self { socket, agent_addr })
    }
}

/// Sendet die Message an `sender.agent_addr`.
///
/// # Errors
/// `XrceError::PayloadTooLarge`, wenn das Datagram > `MAX_DATAGRAM_SIZE`
/// ist; ansonsten `XrceError` aus dem Encoder oder `std::io::Error`
/// (gewrappt in `XrceError::ValueOutOfRange`, weil das Crate keinen
/// IO-Variant hat — Caller bekommt nur strukturierte Fehler).
pub fn send_message(sender: &XrceUdpSender, msg: &Message) -> Result<(), XrceError> {
    let bytes = msg.encode()?;
    if bytes.len() > MAX_DATAGRAM_SIZE {
        return Err(XrceError::PayloadTooLarge {
            limit: MAX_DATAGRAM_SIZE,
            actual: bytes.len(),
        });
    }
    sender
        .socket
        .send_to(&bytes, sender.agent_addr)
        .map_err(|_| XrceError::ValueOutOfRange {
            message: "udp send_to failed",
        })?;
    Ok(())
}

/// Empfaengt eine Message vom Socket. Liefert `(peer, msg)`.
///
/// # Errors
/// `XrceError`, wenn die Message-Decode fehlschlaegt; `ValueOutOfRange`
/// fuer IO-Fehler.
pub fn recv_message(sock: &UdpSocket) -> Result<(SocketAddr, Message), XrceError> {
    let mut buf = [0u8; MAX_DATAGRAM_SIZE];
    let (n, peer) = sock
        .recv_from(&mut buf)
        .map_err(|_| XrceError::ValueOutOfRange {
            message: "udp recv_from failed",
        })?;
    if n > DOSC_MAX_PAYLOAD_SIZE {
        return Err(XrceError::PayloadTooLarge {
            limit: DOSC_MAX_PAYLOAD_SIZE,
            actual: n,
        });
    }
    let msg = Message::decode(&buf[..n])?;
    Ok((peer, msg))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::header::{ClientKey, MessageHeader, SessionId, StreamId};
    use crate::serial_number::SerialNumber16;
    use crate::submessages::timestamp::TimePoint;
    use crate::submessages::write_data::DataFormat;
    use crate::submessages::{
        AckNackPayload, CreateClientPayload, FragmentPayload, HeartbeatPayload, ResetPayload,
        Submessage, TimestampPayload, TimestampReplyPayload, WriteDataPayload,
    };
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::time::Duration;

    fn loopback_pair() -> (UdpSocket, UdpSocket) {
        let a = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).expect("bind a");
        let b = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).expect("bind b");
        a.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        b.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        (a, b)
    }

    fn message_with_one_submessage(sm: Submessage) -> Message {
        let header = MessageHeader::with_client_key(
            SessionId(0),
            StreamId::BUILTIN_RELIABLE,
            SerialNumber16::new(1),
            ClientKey([0xCA, 0xFE, 0xBA, 0xBE]),
        )
        .unwrap();
        Message::new(header, alloc::vec![sm]).unwrap()
    }

    fn loopback_roundtrip_one(sm: Submessage) {
        let (sender_sock, receiver_sock) = loopback_pair();
        let agent_addr = receiver_sock.local_addr().unwrap();
        let sender = XrceUdpSender {
            socket: sender_sock,
            agent_addr,
        };
        let msg = message_with_one_submessage(sm);
        send_message(&sender, &msg).expect("send");
        let (_peer, received) = recv_message(&receiver_sock).expect("recv");
        assert_eq!(received, msg);
    }

    extern crate alloc;

    #[test]
    fn agent_port_for_domain_0_is_7400() {
        assert_eq!(agent_default_port(0), 7400);
    }

    #[test]
    fn client_port_for_domain_0_is_7401() {
        assert_eq!(client_default_port(0), 7401);
    }

    #[test]
    fn agent_port_for_domain_5_is_7420() {
        assert_eq!(agent_default_port(5), 7420);
        assert_eq!(client_default_port(5), 7421);
    }

    #[test]
    fn loopback_roundtrip_create_client() {
        let sm = CreateClientPayload {
            representation: alloc::vec![b'X', b'R', b'C', b'E', 1, 0],
        }
        .into_submessage()
        .unwrap();
        loopback_roundtrip_one(sm);
    }

    #[test]
    fn loopback_roundtrip_write_data() {
        let sm = WriteDataPayload {
            representation: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
            data_format: DataFormat::Sample,
        }
        .into_submessage()
        .unwrap();
        loopback_roundtrip_one(sm);
    }

    #[test]
    fn loopback_roundtrip_acknack() {
        let sm = AckNackPayload {
            first_unacked_seq_num: 5,
            nack_bitmap: [0xAA, 0x55],
            stream_id: 0x80,
        }
        .into_submessage()
        .unwrap();
        loopback_roundtrip_one(sm);
    }

    #[test]
    fn loopback_roundtrip_heartbeat() {
        let sm = HeartbeatPayload {
            first_unacked_seq_nr: 1,
            last_unacked_seq_nr: 10,
            stream_id: 0x80,
        }
        .into_submessage()
        .unwrap();
        loopback_roundtrip_one(sm);
    }

    #[test]
    fn loopback_roundtrip_reset_fragment_timestamp_chain() {
        // Bauen wir eine Message mit drei Submessages zusammen, um auch
        // Padding ueber UDP zu validieren.
        let header = MessageHeader::without_client_key(
            SessionId(0xFF),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let sm1 = ResetPayload.into_submessage().unwrap();
        let sm2 = FragmentPayload {
            data: alloc::vec![0xDD; 7],
            last_fragment: false,
        }
        .into_submessage()
        .unwrap();
        let sm3 = TimestampPayload {
            transmit_timestamp: TimePoint {
                seconds: 100,
                nanoseconds: 0,
            },
        }
        .into_submessage()
        .unwrap();
        let msg = Message::new(header, alloc::vec![sm1, sm2, sm3]).unwrap();

        let (sender_sock, receiver_sock) = loopback_pair();
        let agent_addr = receiver_sock.local_addr().unwrap();
        let sender = XrceUdpSender {
            socket: sender_sock,
            agent_addr,
        };
        send_message(&sender, &msg).expect("send");
        let (_peer, received) = recv_message(&receiver_sock).expect("recv");
        assert_eq!(received, msg);
    }

    #[test]
    fn loopback_roundtrip_timestamp_reply() {
        let sm = TimestampReplyPayload::default().into_submessage().unwrap();
        loopback_roundtrip_one(sm);
    }
}
