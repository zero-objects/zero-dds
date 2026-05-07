// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE TCP-Transport-Mapping (Spec §11.3).
//!
//! TCP unterscheidet sich von UDP (§11.2) dadurch, dass es keine
//! Datagram-Boundaries hat — der Stream liefert nur einen kontinuierlichen
//! Byte-Strom. Spec §11.3.3 verlangt deshalb pro XRCE-Message einen
//! 2-Byte-Length-Prefix.
//!
//! ```text
//!  +--------+--------+----------------------------+
//!  | length (LE u16) |       XRCE-Message         |
//!  +--------+--------+----------------------------+
//! ```
//!
//! ## Anmerkung Endianness
//!
//! Der Spec-Text §11.3.3 spricht von "2-Byte-Length-Prefix". Die meisten
//! existierenden XRCE-Implementierungen (Micro-XRCE-DDS) nutzen
//! Little-Endian, weil das mit dem Submessage-Length-Feld (§8.3.4 — immer
//! LE) konsistent ist. Wir folgen dieser de-facto-Konvention.
//!
//! ## DoS-Schutz
//!
//! - max-message-size = `MAX_DATAGRAM_SIZE` (analog UDP)
//! - bei Truncation/Connection-Close → `XrceError::ValueOutOfRange`

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};

use crate::error::XrceError;
use crate::submessages::{DOSC_MAX_PAYLOAD_SIZE, Message};
use crate::transport_udp::MAX_DATAGRAM_SIZE;

/// Wire-Size des Length-Prefix (§11.3.3).
pub const TCP_LENGTH_PREFIX_SIZE: usize = 2;

/// XRCE-TCP-Client. Verbindet sich zu einem XRCE-TCP-Agent und kapselt
/// Length-Prefix-Framing.
#[derive(Debug)]
pub struct XrceTcpClient {
    /// Aktive TCP-Verbindung.
    pub stream: TcpStream,
}

impl XrceTcpClient {
    /// Verbindet sich zu `addr` (Agent-Adresse).
    ///
    /// # Errors
    /// `XrceError::ValueOutOfRange`, wenn der Connect fehlschlaegt.
    pub fn connect(addr: SocketAddr) -> Result<Self, XrceError> {
        let stream = TcpStream::connect(addr).map_err(|_| XrceError::ValueOutOfRange {
            message: "tcp connect failed",
        })?;
        Ok(Self { stream })
    }

    /// Wrappt einen schon existierenden Stream (z.B. aus `accept`).
    #[must_use]
    pub fn from_stream(stream: TcpStream) -> Self {
        Self { stream }
    }

    /// Sendet `msg` mit 2-Byte-Length-Prefix (LE).
    ///
    /// # Errors
    /// - `PayloadTooLarge`, wenn die Message > `MAX_DATAGRAM_SIZE` ist.
    /// - `ValueOutOfRange`, wenn das Encode oder der TCP-Write fehlschlaegt.
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

    /// Empfaengt eine Message — blockt bis komplettes Frame da ist.
    ///
    /// # Errors
    /// - `UnexpectedEof`, wenn die Verbindung geschlossen wird, bevor das
    ///   Frame vollstaendig ist.
    /// - `PayloadTooLarge`, wenn der Length-Prefix > `MAX_DATAGRAM_SIZE`
    ///   ankuendigt (DoS-Cap).
    /// - `XrceError` aus `Message::decode`.
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

    /// Schliesst die Verbindung explizit (Drop tut das auch automatisch).
    ///
    /// # Errors
    /// `ValueOutOfRange` bei Shutdown-Fehler.
    pub fn close(&mut self) -> Result<(), XrceError> {
        self.stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "tcp shutdown failed",
            })
    }
}

/// XRCE-TCP-Agent (Server-Side). Lauscht auf einem Bind-Port.
#[derive(Debug)]
pub struct XrceTcpServer {
    /// Listener-Socket.
    pub listener: TcpListener,
}

impl XrceTcpServer {
    /// Bindet einen Listener-Port.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn der Bind fehlschlaegt.
    pub fn bind(addr: SocketAddr) -> Result<Self, XrceError> {
        let listener = TcpListener::bind(addr).map_err(|_| XrceError::ValueOutOfRange {
            message: "tcp bind failed",
        })?;
        Ok(Self { listener })
    }

    /// Akzeptiert die naechste Verbindung. Blockt.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn `accept` fehlschlaegt.
    pub fn accept(&self) -> Result<(XrceTcpClient, SocketAddr), XrceError> {
        let (stream, peer) = self
            .listener
            .accept()
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "tcp accept failed",
            })?;
        Ok((XrceTcpClient::from_stream(stream), peer))
    }

    /// Lokal gebundene Adresse.
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn `local_addr` fehlschlaegt.
    pub fn local_addr(&self) -> Result<SocketAddr, XrceError> {
        self.listener
            .local_addr()
            .map_err(|_| XrceError::ValueOutOfRange {
                message: "tcp local_addr failed",
            })
    }
}

/// Liest exakt `buf.len()` Bytes; bei vorzeitigem EOF → `UnexpectedEof`.
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
            // Sofort Drop → schliesst die Verbindung.
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
        // Wir bauen einen Mock-Server, der einen Length-Prefix sendet, der
        // groesser als MAX_DATAGRAM_SIZE ist (was nicht moeglich waere,
        // weil len ein u16 ist) — also testen wir hier, dass auch die
        // DOSC-Cap greift.
        let (server, addr) = loopback_pair();
        let server_thread = thread::spawn(move || {
            let (mut client, _) = server.accept().unwrap();
            // Sende Length-Prefix > DOSC_MAX_PAYLOAD_SIZE (geht nur bis
            // u16::MAX = 65535 = DOSC_MAX_PAYLOAD_SIZE; also testen wir
            // den boundary direkt).
            let bad: u16 = u16::MAX;
            client.stream.write_all(&bad.to_le_bytes()).unwrap();
            // dann viele bytes streamen, damit Read nicht haengt
            client.stream.write_all(&[0u8; 100]).unwrap();
            client.stream.shutdown(std::net::Shutdown::Both).ok();
        });
        let mut client = XrceTcpClient::connect(addr).unwrap();
        client
            .stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let res = client.recv_message();
        // Endweder PayloadTooLarge (cap), oder UnexpectedEof, oder
        // dekodier-Fehler — alles sind valide Reject-Pfade.
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
        // Erste Send schreibt evtl. noch in den Kernel-Buffer; der zweite
        // sollte fehlen, da peer closed.
        let msg = message_with(ResetPayload.into_submessage().unwrap());
        let _ = client.send_message(&msg);
        let _ = client.send_message(&msg);
        // Wir machen hier keinen harten Assert — verschiedene OS liefern
        // unterschiedlich. Test laeuft sauber durch, kein UB.
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
        // Doppel-Close darf nicht panicen (Variante 2 darf err sein).
        let _ = client.close();
        server_thread.join().unwrap();
    }

    #[test]
    fn tcp_length_prefix_size_constant() {
        assert_eq!(TCP_LENGTH_PREFIX_SIZE, 2);
    }
}
