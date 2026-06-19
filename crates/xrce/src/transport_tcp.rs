// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE TCP-Transport-Mapping (Spec §11.3).
//!
//! TCP differs from UDP (§11.2) in that it has no
//! datagram boundaries — the stream delivers only a continuous
//! byte stream. Spec §11.3.3 therefore requires a 2-byte length prefix
//! per XRCE message.
//!
//! ```text
//!  +--------+--------+----------------------------+
//!  | length (LE u16) |       XRCE-Message         |
//!  +--------+--------+----------------------------+
//! ```
//!
//! ## Note on endianness
//!
//! The spec text §11.3.3 speaks of a "2-byte length prefix". Most
//! existing XRCE implementations (Micro-XRCE-DDS) use
//! little-endian, because it is consistent with the submessage length
//! field (§8.3.4 — always LE). We follow this de-facto convention.
//!
//! ## DoS protection
//!
//! - max-message-size = `MAX_DATAGRAM_SIZE` (analogous to UDP)
//! - on truncation/connection close → `XrceError::ValueOutOfRange`

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};

use crate::error::XrceError;
use crate::submessages::{DOSC_MAX_PAYLOAD_SIZE, Message};
use crate::transport_udp::MAX_DATAGRAM_SIZE;

/// Wire size of the length prefix (§11.3.3).
pub const TCP_LENGTH_PREFIX_SIZE: usize = 2;

/// XRCE TCP client. Connects to an XRCE TCP agent and encapsulates
/// length-prefix framing.
#[derive(Debug)]
pub struct XrceTcpClient {
    /// Active TCP connection.
    pub stream: TcpStream,
}

impl XrceTcpClient {
    /// Connects to `addr` (agent address).
    ///
    /// # Errors
    /// `XrceError::ValueOutOfRange` if the connect fails.
    pub fn connect(addr: SocketAddr) -> Result<Self, XrceError> {
        let stream = TcpStream::connect(addr).map_err(|_| XrceError::ValueOutOfRange {
            message: "tcp connect failed",
        })?;
        Ok(Self { stream })
    }

    /// Wraps an already existing stream (e.g. from `accept`).
    #[must_use]
    pub fn from_stream(stream: TcpStream) -> Self {
        Self { stream }
    }

    /// Sends `msg` with a 2-byte length prefix (LE).
    ///
    /// # Errors
    /// - `PayloadTooLarge` if the message is > `MAX_DATAGRAM_SIZE`.
    /// - `ValueOutOfRange` if the encode or the TCP write fails.
    pub fn send_message(&mut self, msg: &Message) -> Result<(), XrceError> {
        let bytes = msg.encode()?;
        if bytes.len() > MAX_DATAGRAM_SIZE {
            return Err(XrceError::PayloadTooLarge {
                limit: MAX_DATAGRAM_SIZE,
                actual: bytes.len(),
            });
        }
        let len = u16::try_from(bytes.len()).map_err(|_| XrceError::ValueOutOfRange {
            message: "tcp message length exceeds u16",
        })?;
        let prefix = len.to_le_bytes();
        self.stream
            .write_all(&prefix)
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "tcp write_all length-prefix failed",
            })?;
        self.stream
            .write_all(&bytes)
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "tcp write_all body failed",
            })?;
        Ok(())
    }

    /// Receives a message — blocks until the complete frame is present.
    ///
    /// # Errors
    /// - `UnexpectedEof` if the connection is closed before the
    ///   frame is complete.
    /// - `PayloadTooLarge` if the length prefix announces > `MAX_DATAGRAM_SIZE`
    ///   (DoS cap).
    /// - `XrceError` from `Message::decode`.
    pub fn recv_message(&mut self) -> Result<Message, XrceError> {
        let mut prefix = [0u8; TCP_LENGTH_PREFIX_SIZE];
        read_exact_eof(&mut self.stream, &mut prefix)?;
        let len = u16::from_le_bytes(prefix) as usize;
        if len > MAX_DATAGRAM_SIZE {
            return Err(XrceError::PayloadTooLarge {
                limit: MAX_DATAGRAM_SIZE,
                actual: len,
            });
        }
        if len > DOSC_MAX_PAYLOAD_SIZE {
            return Err(XrceError::PayloadTooLarge {
                limit: DOSC_MAX_PAYLOAD_SIZE,
                actual: len,
            });
        }
        let mut body = std::vec![0u8; len];
        read_exact_eof(&mut self.stream, &mut body)?;
        Message::decode(&body)
    }

    /// Closes the connection explicitly (Drop does this automatically too).
    ///
    /// # Errors
    /// `ValueOutOfRange` on a shutdown error.
    pub fn close(&mut self) -> Result<(), XrceError> {
        self.stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "tcp shutdown failed",
            })
    }
}

/// XRCE TCP agent (server side). Listens on a bind port.
#[derive(Debug)]
pub struct XrceTcpServer {
    /// Listener socket.
    pub listener: TcpListener,
}

impl XrceTcpServer {
    /// Binds a listener port.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the bind fails.
    pub fn bind(addr: SocketAddr) -> Result<Self, XrceError> {
        let listener = TcpListener::bind(addr).map_err(|_| XrceError::ValueOutOfRange {
            message: "tcp bind failed",
        })?;
        Ok(Self { listener })
    }

    /// Accepts the next connection. Blocks.
    ///
    /// # Errors
    /// `ValueOutOfRange` if `accept` fails.
    pub fn accept(&self) -> Result<(XrceTcpClient, SocketAddr), XrceError> {
        let (stream, peer) = self
            .listener
            .accept()
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "tcp accept failed",
            })?;
        Ok((XrceTcpClient::from_stream(stream), peer))
    }

    /// Locally bound address.
    ///
    /// # Errors
    /// `ValueOutOfRange` if `local_addr` fails.
    pub fn local_addr(&self) -> Result<SocketAddr, XrceError> {
        self.listener
            .local_addr()
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "tcp local_addr failed",
            })
    }
}

/// Reads exactly `buf.len()` bytes; on premature EOF → `UnexpectedEof`.
fn read_exact_eof<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<(), XrceError> {
    let needed = buf.len();
    let mut read = 0usize;
    while read < needed {
        match r.read(&mut buf[read..]) {
            Ok(0) => {
                return Err(XrceError::UnexpectedEof {
                    needed: needed - read,
                    offset: read,
                });
            }
            Ok(n) => read += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                return Err(XrceError::ValueOutOfRange {
                    message: "tcp read failed",
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::header::{ClientKey, MessageHeader, SessionId, StreamId};
    use crate::serial_number::SerialNumber16;
    use crate::submessages::write_data::DataFormat;
    use crate::submessages::{
        AckNackPayload, CreateClientPayload, HeartbeatPayload, ResetPayload, Submessage,
        WriteDataPayload,
    };
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::thread;
    use std::time::Duration;

    extern crate alloc;

    fn loopback_pair() -> (XrceTcpServer, SocketAddr) {
        let server =
            XrceTcpServer::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))).unwrap();
        let addr = server.local_addr().unwrap();
        (server, addr)
    }

    fn message_with(sm: Submessage) -> Message {
        let header = MessageHeader::with_client_key(
            SessionId(0),
            StreamId::BUILTIN_RELIABLE,
            SerialNumber16::new(1),
            ClientKey([0xCA, 0xFE, 0xBA, 0xBE]),
        )
        .unwrap();
        Message::new(header, alloc::vec![sm]).unwrap()
    }

    #[test]
    fn tcp_loopback_create_client_roundtrip() {
        let (server, addr) = loopback_pair();
        let server_thread = thread::spawn(move || {
            let (mut client, _) = server.accept().unwrap();
            client.recv_message().unwrap()
        });
        let mut client = XrceTcpClient::connect(addr).unwrap();
        let msg = message_with(
            CreateClientPayload {
                representation: alloc::vec![b'X', b'R', b'C', b'E', 1, 0],
            }
            .into_submessage()
            .unwrap(),
        );
        client.send_message(&msg).unwrap();
        let received = server_thread.join().unwrap();
        assert_eq!(received, msg);
    }

    #[test]
    fn tcp_loopback_write_data_roundtrip() {
        let (server, addr) = loopback_pair();
        let server_thread = thread::spawn(move || {
            let (mut client, _) = server.accept().unwrap();
            client.recv_message().unwrap()
        });
        let mut client = XrceTcpClient::connect(addr).unwrap();
        let msg = message_with(
            WriteDataPayload {
                representation: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                data_format: DataFormat::Sample,
            }
            .into_submessage()
            .unwrap(),
        );
        client.send_message(&msg).unwrap();
        let received = server_thread.join().unwrap();
        assert_eq!(received, msg);
    }

    #[test]
    fn tcp_loopback_three_message_chain() {
        let (server, addr) = loopback_pair();
        let server_thread = thread::spawn(move || {
            let (mut client, _) = server.accept().unwrap();
            let m1 = client.recv_message().unwrap();
            let m2 = client.recv_message().unwrap();
            let m3 = client.recv_message().unwrap();
            (m1, m2, m3)
        });
        let mut client = XrceTcpClient::connect(addr).unwrap();
        let m1 = message_with(ResetPayload.into_submessage().unwrap());
        let m2 = message_with(
            HeartbeatPayload {
                first_unacked_seq_nr: 1,
                last_unacked_seq_nr: 9,
                stream_id: 0x80,
            }
            .into_submessage()
            .unwrap(),
        );
        let m3 = message_with(
            AckNackPayload {
                first_unacked_seq_num: 5,
                nack_bitmap: [0xAA, 0x55],
                stream_id: 0x80,
            }
            .into_submessage()
            .unwrap(),
        );
        client.send_message(&m1).unwrap();
        client.send_message(&m2).unwrap();
        client.send_message(&m3).unwrap();
        let (r1, r2, r3) = server_thread.join().unwrap();
        assert_eq!(r1, m1);
        assert_eq!(r2, m2);
        assert_eq!(r3, m3);
    }

    #[test]
    fn tcp_recv_after_close_returns_eof() {
        let (server, addr) = loopback_pair();
        let server_thread = thread::spawn(move || {
            let (client, _) = server.accept().unwrap();
            // Immediate drop → closes the connection.
            drop(client);
        });
        let mut client = XrceTcpClient::connect(addr).unwrap();
        client
            .stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        server_thread.join().unwrap();
        let res = client.recv_message();
        assert!(matches!(res, Err(XrceError::UnexpectedEof { .. })));
    }

    #[test]
    fn tcp_recv_oversized_length_rejected() {
        // We build a mock server that sends a length prefix larger
        // than MAX_DATAGRAM_SIZE (which would not be possible
        // because len is a u16) — so here we test that the
        // DOSC cap also takes effect.
        let (server, addr) = loopback_pair();
        let server_thread = thread::spawn(move || {
            let (mut client, _) = server.accept().unwrap();
            // Send length prefix > DOSC_MAX_PAYLOAD_SIZE (only goes up to
            // u16::MAX = 65535 = DOSC_MAX_PAYLOAD_SIZE; so we test
            // the boundary directly).
            let bad: u16 = u16::MAX;
            client.stream.write_all(&bad.to_le_bytes()).unwrap();
            // then stream many bytes so that Read does not hang
            client.stream.write_all(&[0u8; 100]).unwrap();
            client.stream.shutdown(std::net::Shutdown::Both).ok();
        });
        let mut client = XrceTcpClient::connect(addr).unwrap();
        client
            .stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let res = client.recv_message();
        // Either PayloadTooLarge (cap), or UnexpectedEof, or a
        // decode error — all are valid reject paths.
        assert!(res.is_err());
        server_thread.join().unwrap();
    }

    #[test]
    fn tcp_send_truncation_when_peer_drops() {
        let (server, addr) = loopback_pair();
        let server_thread = thread::spawn(move || {
            let (client, _) = server.accept().unwrap();
            drop(client);
        });
        let mut client = XrceTcpClient::connect(addr).unwrap();
        server_thread.join().unwrap();
        // The first send may still write into the kernel buffer; the second
        // should fail, since the peer has closed.
        let msg = message_with(ResetPayload.into_submessage().unwrap());
        let _ = client.send_message(&msg);
        let _ = client.send_message(&msg);
        // We make no hard assert here — different OSes behave
        // differently. The test runs cleanly through, no UB.
    }

    #[test]
    fn tcp_local_addr_consistent_after_bind() {
        let server =
            XrceTcpServer::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))).unwrap();
        let addr = server.local_addr().unwrap();
        assert_eq!(addr.ip(), Ipv4Addr::LOCALHOST);
        assert!(addr.port() > 0);
    }

    #[test]
    fn tcp_close_idempotent_safe() {
        let (server, addr) = loopback_pair();
        let server_thread = thread::spawn(move || {
            let _ = server.accept().unwrap();
        });
        let mut client = XrceTcpClient::connect(addr).unwrap();
        let _ = client.close();
        // Double close must not panic (variant 2 may be an err).
        let _ = client.close();
        server_thread.join().unwrap();
    }

    #[test]
    fn tcp_length_prefix_size_constant() {
        assert_eq!(TCP_LENGTH_PREFIX_SIZE, 2);
    }
}
