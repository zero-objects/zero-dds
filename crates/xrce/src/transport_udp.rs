// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE UDP-Transport-Mapping (Spec §11.2).
//!
//! Default port scheme (task specification C6.2.A):
//! - Agent: `7400 + 4 * domain_id + 0`
//! - Client: `7400 + 4 * domain_id + 1`
//! - Multicast: none by default (XRCE is unicast between client+agent;
//!   multicast discovery at `239.255.0.2:7400` per §11.2.4 is
//!   out of scope for C6.2.A).
//!
//! UDP/IP payload = exactly one XRCE message (Spec §11.2.3, no
//! envelopes).

use std::net::{SocketAddr, UdpSocket};

use crate::error::XrceError;
use crate::submessages::{DOSC_MAX_PAYLOAD_SIZE, Message};

/// Maximum UDP datagram. Larger than `DOSC_MAX_PAYLOAD_SIZE` would
/// not be possible over UDP, because the wire limit is 65,507 bytes.
pub const MAX_DATAGRAM_SIZE: usize = 65_507;

/// Default agent port `7400 + 4*domain_id`.
#[must_use]
pub fn agent_default_port(domain_id: u16) -> u16 {
    7400u16.saturating_add(domain_id.saturating_mul(4))
}

/// Default client port `7400 + 4*domain_id + 1`.
#[must_use]
pub fn client_default_port(domain_id: u16) -> u16 {
    7400u16
        .saturating_add(domain_id.saturating_mul(4))
        .saturating_add(1)
}

/// XRCE UDP sender, bound to a local socket with a fixed
/// agent address as the default target.
#[derive(Debug)]
pub struct XrceUdpSender {
    /// Local socket.
    pub socket: UdpSocket,
    /// Default target (agent address).
    pub agent_addr: SocketAddr,
}

impl XrceUdpSender {
    /// Constructs with an explicit local bind and agent target.
    ///
    /// # Errors
    /// `std::io::Error` if the bind fails.
    pub fn bind(local: SocketAddr, agent_addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(local)?;
        Ok(Self { socket, agent_addr })
    }
}

/// Sends the message to `sender.agent_addr`.
///
/// # Errors
/// `XrceError::PayloadTooLarge` if the datagram is > `MAX_DATAGRAM_SIZE`;
/// otherwise `XrceError` from the encoder or `std::io::Error`
/// (wrapped in `XrceError::ValueOutOfRange`, because the crate has no
/// IO variant — the caller receives only structured errors).
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

/// Receives a message from the socket. Returns `(peer, msg)`.
///
/// # Errors
/// `XrceError` if the message decode fails; `ValueOutOfRange`
/// for IO errors.
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
    use crate::object_id::ObjectId;
    use crate::object_info::BaseObjectRequest;
    use crate::serial_number::SerialNumber16;
    use crate::submessages::timestamp::TimePoint;
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
            base: BaseObjectRequest {
                request_id: [0x00, 0x01],
                object_id: ObjectId::from_raw(0x0DA5),
            },
            serialized_data: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
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
        // Assemble a message with three submessages to also
        // validate padding over UDP.
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
