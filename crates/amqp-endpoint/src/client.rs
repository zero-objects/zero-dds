// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Minimal AMQP 1.0 **client connection** (initiator role) — the piece a broker
//! such as RabbitMQ requires: a SASL handshake before the AMQP connection, then
//! open / begin / attach(target) / transfer / detach / close.
//!
//! RabbitMQ 4.0 speaks AMQP 1.0 natively on port 5672 and **mandates SASL**
//! (the bare AMQP protocol header without a preceding SASL layer is rejected).
//! This client does SASL-PLAIN, then publishes a settled message to a RabbitMQ
//! v2 node address (`/queues/<name>` or `/exchanges/<name>/<key>`).
//!
//! Built on the `zerodds-amqp-bridge` codec (performatives + SASL + described
//! types). `std`-only (uses `TcpStream`).

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use zerodds_amqp_bridge::frame::{
    FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};
use zerodds_amqp_bridge::{
    attach_to, begin, close, data_payload_from_transfer, decode_performative, descriptor, detach,
    disposition_accept, end, flow_link_credit, open, sasl_init, sasl_mechanisms_from_body,
    sasl_outcome_code, sasl_plain_response, transfer_message,
};

/// AMQP protocol header (Spec §2.2): protocol-id `0x00` = AMQP, version 1.0.0.
const AMQP_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x00, 0x01, 0x00, 0x00];
/// SASL protocol header (Spec §5.2.1): protocol-id `0x03` = SASL, version 1.0.0.
const SASL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x03, 0x01, 0x00, 0x00];

/// An established AMQP 1.0 client connection to a broker.
pub struct AmqpClient {
    stream: TcpStream,
    handle: u32,
    delivery_id: u32,
}

impl AmqpClient {
    /// Connects to `addr`, performs the SASL-PLAIN handshake with
    /// `user`/`pass`, then opens the AMQP connection + a session.
    ///
    /// # Errors
    /// I/O, SASL rejection (auth failure), or protocol error.
    pub fn connect_plain<A: ToSocketAddrs>(
        addr: A,
        user: &str,
        pass: &str,
        container_id: &str,
    ) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        stream.set_nodelay(true)?;
        let mut c = Self {
            stream,
            handle: 0,
            delivery_id: 0,
        };
        c.sasl_plain(user, pass)?;
        c.open_connection(container_id)?;
        Ok(c)
    }

    /// SASL layer: exchange SASL headers, read the offered mechanisms, send
    /// sasl-init(PLAIN), read the outcome.
    fn sasl_plain(&mut self, user: &str, pass: &str) -> io::Result<()> {
        self.stream.write_all(&SASL_HEADER)?;
        let mut peer = [0u8; 8];
        self.stream.read_exact(&mut peer)?;
        if peer[4] != 0x03 {
            return Err(proto("peer did not offer the SASL layer"));
        }
        // sasl-mechanisms (server → client).
        let body = self.read_frame(FrameType::Sasl)?;
        let (desc, val, _) = decode_performative(&body).map_err(map_codec)?;
        if desc != descriptor::SASL_MECHANISMS {
            return Err(proto("expected sasl-mechanisms"));
        }
        let mechs = sasl_mechanisms_from_body(&val);
        if !mechs.iter().any(|m| m == "PLAIN") {
            return Err(proto("server does not offer SASL PLAIN"));
        }
        // sasl-init (client → server) with the PLAIN response.
        let init = sasl_init("PLAIN", &sasl_plain_response(user, pass)).map_err(map_codec)?;
        self.write_frame(FrameType::Sasl, 0, &init)?;
        // sasl-outcome.
        let body = self.read_frame(FrameType::Sasl)?;
        let (desc, val, _) = decode_performative(&body).map_err(map_codec)?;
        if desc != descriptor::SASL_OUTCOME {
            return Err(proto("expected sasl-outcome"));
        }
        match sasl_outcome_code(&val) {
            Some(0) => Ok(()),
            Some(c) => Err(proto_owned(format!("SASL auth failed, outcome code {c}"))),
            None => Err(proto("sasl-outcome missing code")),
        }
    }

    /// AMQP layer: protocol header exchange, open + begin.
    fn open_connection(&mut self, container_id: &str) -> io::Result<()> {
        self.stream.write_all(&AMQP_HEADER)?;
        let mut peer = [0u8; 8];
        self.stream.read_exact(&mut peer)?;
        if &peer[..4] != b"AMQP" || peer[4] != 0x00 {
            return Err(proto("bad AMQP protocol header from peer"));
        }
        self.write_frame(FrameType::Amqp, 0, &open(container_id).map_err(map_codec)?)?;
        self.expect_amqp(descriptor::OPEN)?;
        self.write_frame(
            FrameType::Amqp,
            0,
            &begin(None, 0, 2048, 2048).map_err(map_codec)?,
        )?;
        self.expect_amqp(descriptor::BEGIN)?;
        Ok(())
    }

    /// Publishes `payload` to `address` (RabbitMQ v2, e.g. `/queues/<name>`):
    /// attach a sender link to the target, wait for the broker's attach +
    /// initial flow, transfer one settled message, then detach the link.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn send_to(&mut self, address: &str, payload: &[u8]) -> io::Result<()> {
        let handle = self.handle;
        self.handle += 1;
        let link = format!("zd-snd-{handle}");
        self.write_frame(
            FrameType::Amqp,
            0,
            &attach_to(&link, handle, true, address).map_err(map_codec)?,
        )?;
        self.expect_amqp(descriptor::ATTACH)?;
        // The broker grants credit via a flow; wait for it so the transfer is
        // accepted. Tolerate it arriving interleaved.
        self.wait_for(descriptor::FLOW)?;

        let did = self.delivery_id;
        self.delivery_id += 1;
        let tag = did.to_be_bytes();
        let xfer = transfer_message(handle, did, &tag, payload, true).map_err(map_codec)?;
        self.write_frame(FrameType::Amqp, 0, &xfer)?;

        self.write_frame(
            FrameType::Amqp,
            0,
            &detach(handle, true).map_err(map_codec)?,
        )?;
        Ok(())
    }

    /// Consumes one message from `address` (RabbitMQ v2, e.g. `/queues/<name>`):
    /// attach a receiver link to the source, grant one credit via a flow, read
    /// the broker's transfer, extract its Data-section payload, settle it
    /// (disposition accepted), then detach. Returns `None` if no message arrives
    /// within the read timeout.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn recv_from(&mut self, address: &str) -> io::Result<Option<Vec<u8>>> {
        let handle = self.handle;
        self.handle += 1;
        let link = format!("zd-rcv-{handle}");
        self.write_frame(
            FrameType::Amqp,
            0,
            &attach_to(&link, handle, false, address).map_err(map_codec)?,
        )?;
        self.expect_amqp(descriptor::ATTACH)?;
        // Grant one credit so the broker will deliver a message.
        self.write_frame(
            FrameType::Amqp,
            0,
            &flow_link_credit(handle, 0, 1).map_err(map_codec)?,
        )?;

        // Read until a transfer arrives (tolerate an interleaved flow).
        let mut payload = None;
        for _ in 0..8 {
            let body = match self.read_frame(FrameType::Amqp) {
                Ok(b) => b,
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(e) => return Err(e),
            };
            if body.is_empty() {
                continue;
            }
            if let Ok((desc, _, _)) = decode_performative(&body) {
                if desc == descriptor::TRANSFER {
                    payload = data_payload_from_transfer(&body);
                    // Settle the delivery (delivery-id 0 for the first one).
                    self.write_frame(
                        FrameType::Amqp,
                        0,
                        &disposition_accept(0, None).map_err(map_codec)?,
                    )?;
                    break;
                }
            }
        }
        let _ = self.write_frame(
            FrameType::Amqp,
            0,
            &detach(handle, true).map_err(map_codec)?,
        );
        Ok(payload)
    }

    /// Ends the session + closes the connection (best-effort).
    pub fn close(mut self) {
        let _ = self
            .end_body()
            .and_then(|b| self.write_frame(FrameType::Amqp, 0, &b))
            .and_then(|()| self.close_body())
            .and_then(|b| self.write_frame(FrameType::Amqp, 0, &b));
    }

    fn end_body(&self) -> io::Result<Vec<u8>> {
        end().map_err(map_codec)
    }
    fn close_body(&self) -> io::Result<Vec<u8>> {
        close().map_err(map_codec)
    }

    // ---- frame I/O -------------------------------------------------------

    fn write_frame(&mut self, ft: FrameType, channel: u16, body: &[u8]) -> io::Result<()> {
        let h = FrameHeader {
            size: 8 + body.len() as u32,
            doff: 2,
            frame_type: ft,
            channel,
        };
        self.stream.write_all(&encode_frame_header(h))?;
        self.stream.write_all(body)?;
        Ok(())
    }

    /// Reads one frame of the expected layer; returns the performative body.
    fn read_frame(&mut self, _expected: FrameType) -> io::Result<Vec<u8>> {
        let mut hdr = [0u8; 8];
        self.stream.read_exact(&mut hdr)?;
        let h =
            decode_frame_header(&hdr).map_err(|e| proto_owned(format!("frame header: {e:?}")))?;
        if h.size < 8 {
            return Err(proto("frame size < 8"));
        }
        // Skip any extended header (doff words beyond the fixed 8 bytes).
        let ext = (h.doff as usize) * 4 - 8;
        if ext > 0 {
            let mut skip = vec![0u8; ext];
            self.stream.read_exact(&mut skip)?;
        }
        let body_len = h.size as usize - (h.doff as usize) * 4;
        let mut body = vec![0u8; body_len];
        if body_len > 0 {
            self.stream.read_exact(&mut body)?;
        }
        Ok(body)
    }

    /// Reads one AMQP frame and asserts its performative descriptor.
    fn expect_amqp(&mut self, want: u64) -> io::Result<()> {
        let body = self.read_frame(FrameType::Amqp)?;
        let (desc, _, _) = decode_performative(&body).map_err(map_codec)?;
        if desc != want {
            return Err(proto_owned(format!(
                "expected performative {want:#x}, got {desc:#x}"
            )));
        }
        Ok(())
    }

    /// Reads AMQP frames until one with descriptor `want` is seen (tolerating
    /// other performatives such as an interleaved flow/disposition).
    fn wait_for(&mut self, want: u64) -> io::Result<()> {
        for _ in 0..8 {
            let body = self.read_frame(FrameType::Amqp)?;
            if body.is_empty() {
                continue;
            }
            if let Ok((desc, _, _)) = decode_performative(&body) {
                if desc == want {
                    return Ok(());
                }
            }
        }
        Err(proto("did not see the awaited performative"))
    }
}

fn proto(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
fn proto_owned(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
fn map_codec(e: zerodds_amqp_bridge::TypeError) -> io::Error {
    proto_owned(format!("amqp codec: {e:?}"))
}
