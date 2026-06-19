// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `std` TCP framing + a lightweight version-aware MQTT client.
//!
//! Shared by the [`crate::server`] broker and by interop tests. The client
//! speaks both MQTT 5.0 and 3.1.1 (selected via [`ProtocolVersion`]) over plain
//! TCP. It is deliberately small — CONNECT/SUBSCRIBE/PUBLISH/receive — and does
//! not pull in the DDS-bridge `daemon` feature.

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::string::String;
use std::time::Duration;
use std::vec::Vec;

use crate::codec::{PublishPacket, decode_publish_v, encode_publish_v};
use crate::control_packets::{
    ConnackBody, ConnectBody, SubackBody, SubscribeBody, Subscription, connect_flags,
    decode_connack_body_v, decode_suback_body_v, encode_connect_body_v, encode_subscribe_body_v,
};
use crate::packet::ControlPacketType;
use crate::vbi::encode_vbi;
use crate::version::ProtocolVersion;

/// Reads one whole MQTT control packet from `r`, returning `(byte0, body)`
/// where `body` is the variable-header + payload (Remaining Length bytes).
///
/// # Errors
/// I/O error, or a malformed Remaining Length VBI.
pub fn read_packet<R: Read>(r: &mut R) -> io::Result<(u8, Vec<u8>)> {
    let mut b0 = [0u8; 1];
    r.read_exact(&mut b0)?;
    // Remaining Length VBI — up to 4 bytes (§2.1.4).
    let mut mult: u32 = 1;
    let mut value: u32 = 0;
    for _ in 0..4 {
        let mut eb = [0u8; 1];
        r.read_exact(&mut eb)?;
        value += u32::from(eb[0] & 0x7F) * mult;
        if eb[0] & 0x80 == 0 {
            break;
        }
        mult *= 128;
    }
    let mut body = std::vec![0u8; value as usize];
    if value > 0 {
        r.read_exact(&mut body)?;
    }
    Ok((b0[0], body))
}

/// Frames a packet: `byte0`, the VBI Remaining Length of `body`, then `body`.
///
/// # Errors
/// A body too large to encode as a VBI (> 268_435_455 bytes).
pub fn frame_packet(byte0: u8, body: &[u8]) -> io::Result<Vec<u8>> {
    let len = u32::try_from(body.len()).map_err(|_| io::Error::other("body too large"))?;
    let vbi = encode_vbi(len).ok_or_else(|| io::Error::other("remaining length too large"))?;
    let mut out = Vec::with_capacity(1 + vbi.len() + body.len());
    out.push(byte0);
    out.extend_from_slice(&vbi);
    out.extend_from_slice(body);
    Ok(out)
}

/// First-byte helper: type nibble in the high bits, `flags` in the low nibble.
#[must_use]
pub fn byte0(t: ControlPacketType, flags: u8) -> u8 {
    (t.to_bits() << 4) | (flags & 0x0F)
}

/// A lightweight synchronous MQTT client (plain TCP), version-aware.
pub struct MqttClient {
    stream: TcpStream,
    version: ProtocolVersion,
    next_id: u16,
}

impl MqttClient {
    /// Connects to `addr`, sends CONNECT with the chosen `version` and
    /// `client_id` (Clean Start), and waits for a successful CONNACK.
    ///
    /// # Errors
    /// I/O error, or a CONNACK reason code >= 0x80.
    pub fn connect<A: ToSocketAddrs>(
        addr: A,
        client_id: &str,
        version: ProtocolVersion,
    ) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        stream.set_nodelay(true)?;
        let mut c = Self {
            stream,
            version,
            next_id: 1,
        };

        let connect = ConnectBody {
            protocol_name: version.protocol_name().into(),
            protocol_version: version.level(),
            connect_flags: connect_flags::CLEAN_START,
            keep_alive: 0,
            properties: Vec::new(),
            client_id: client_id.into(),
            will_properties: Vec::new(),
            will_topic: None,
            will_payload: Vec::new(),
            user_name: None,
            password: Vec::new(),
        };
        let body = encode_connect_body_v(&connect, version).map_err(codec)?;
        c.write_packet(byte0(ControlPacketType::Connect, 0), &body)?;

        let (b0, cbody) = read_packet(&mut c.stream)?;
        if (b0 >> 4) != ControlPacketType::ConnAck.to_bits() {
            return Err(io::Error::other("expected CONNACK"));
        }
        let connack: ConnackBody = decode_connack_body_v(&cbody, version).map_err(codec)?;
        if connack.reason_code >= 0x80 {
            return Err(io::Error::other(std::format!(
                "connack reject 0x{:02x}",
                connack.reason_code
            )));
        }
        Ok(c)
    }

    /// Publishes `payload` to `topic` at `qos` (0/1). QoS 1 waits for PUBACK.
    ///
    /// # Errors
    /// I/O or codec error.
    pub fn publish(
        &mut self,
        topic: &str,
        payload: &[u8],
        qos: u8,
        retain: bool,
    ) -> io::Result<()> {
        let packet_id = if qos > 0 { Some(self.alloc_id()) } else { None };
        let p = PublishPacket {
            dup: false,
            qos,
            retain,
            topic: topic.into(),
            packet_id,
            properties: Vec::new(),
            payload: payload.to_vec(),
        };
        let bytes = encode_publish_v(&p, self.version).map_err(codec)?;
        self.stream.write_all(&bytes)?;
        if qos == 1 {
            // Expect PUBACK.
            let (b0, _) = read_packet(&mut self.stream)?;
            if (b0 >> 4) != ControlPacketType::PubAck.to_bits() {
                return Err(io::Error::other("expected PUBACK"));
            }
        }
        Ok(())
    }

    /// Subscribes to `filter` at `max_qos` and waits for SUBACK.
    ///
    /// # Errors
    /// I/O or codec error, or a SUBACK failure return code (0x80).
    pub fn subscribe(&mut self, filter: &str, max_qos: u8) -> io::Result<()> {
        let sub = SubscribeBody {
            packet_id: self.alloc_id(),
            properties: Vec::new(),
            subscriptions: std::vec![Subscription {
                topic_filter: filter.into(),
                options: max_qos & 0x03,
            }],
        };
        let body = encode_subscribe_body_v(&sub, self.version).map_err(codec)?;
        // SUBSCRIBE has fixed flags 0b0010 (§3.8.1).
        self.write_packet(byte0(ControlPacketType::Subscribe, 0b0010), &body)?;
        let (b0, sbody) = read_packet(&mut self.stream)?;
        if (b0 >> 4) != ControlPacketType::SubAck.to_bits() {
            return Err(io::Error::other("expected SUBACK"));
        }
        let suback: SubackBody = decode_suback_body_v(&sbody, self.version).map_err(codec)?;
        if suback.reason_codes.iter().any(|&rc| rc >= 0x80) {
            return Err(io::Error::other("subscription refused"));
        }
        Ok(())
    }

    /// Blocks for the next PUBLISH and returns `(topic, payload)`. QoS 1
    /// deliveries are acked. Other packets (e.g. PINGRESP) are skipped.
    ///
    /// # Errors
    /// I/O or codec error (a read timeout surfaces as [`io::ErrorKind`]).
    pub fn recv_publish(&mut self, timeout: Duration) -> io::Result<(String, Vec<u8>)> {
        self.stream.set_read_timeout(Some(timeout))?;
        loop {
            let (b0, body) = read_packet(&mut self.stream)?;
            let t = (b0 >> 4) & 0x0F;
            if t == ControlPacketType::Publish.to_bits() {
                // Rebuild the full packet for the decoder.
                let full = frame_packet(b0, &body)?;
                let (_, p) = decode_publish_v(&full, self.version).map_err(codec)?;
                if p.qos == 1 {
                    if let Some(id) = p.packet_id {
                        let ack = crate::control_packets::encode_ack_body_v(
                            &crate::control_packets::AckBody {
                                packet_id: id,
                                reason_code: 0,
                                properties: Vec::new(),
                            },
                            self.version,
                        )
                        .map_err(codec)?;
                        self.write_packet(byte0(ControlPacketType::PubAck, 0), &ack)?;
                    }
                }
                return Ok((p.topic, p.payload));
            }
            // Ignore PINGRESP / others and keep waiting.
        }
    }

    /// Sends DISCONNECT and drops the connection.
    pub fn disconnect(mut self) {
        let body = crate::control_packets::encode_disconnect_body_v(
            &crate::control_packets::DisconnectBody {
                reason_code: 0,
                properties: Vec::new(),
            },
            self.version,
        )
        .unwrap_or_default();
        let _ = self.write_packet(byte0(ControlPacketType::Disconnect, 0), &body);
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }

    fn write_packet(&mut self, b0: u8, body: &[u8]) -> io::Result<()> {
        let pkt = frame_packet(b0, body)?;
        self.stream.write_all(&pkt)
    }

    fn alloc_id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        id
    }
}

fn codec(e: crate::codec::CodecError) -> io::Error {
    io::Error::other(std::format!("mqtt codec: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_and_read_round_trip() {
        let body = b"\x00\x04test";
        let framed = frame_packet(byte0(ControlPacketType::Publish, 0), body).expect("frame");
        let mut cursor = std::io::Cursor::new(framed);
        let (b0, got) = read_packet(&mut cursor).expect("read");
        assert_eq!(b0 >> 4, ControlPacketType::Publish.to_bits());
        assert_eq!(got, body);
    }

    #[test]
    fn read_packet_handles_multibyte_remaining_length() {
        let body = std::vec![0xABu8; 200];
        let framed = frame_packet(byte0(ControlPacketType::Publish, 0), &body).expect("frame");
        // 200 needs a 2-byte VBI.
        let mut cursor = std::io::Cursor::new(framed);
        let (_, got) = read_packet(&mut cursor).expect("read");
        assert_eq!(got.len(), 200);
    }
}
