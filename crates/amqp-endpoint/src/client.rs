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
    attach_sender_unsettled, attach_to, begin, close, data_payload_from_transfer,
    decode_performative, delivery_state, descriptor, detach, disposition_accept,
    disposition_from_body, end, flow_link_credit, open, sasl_init, sasl_mechanisms_from_body,
    sasl_outcome_code, sasl_plain_response, transfer_message,
};

/// AMQP protocol header (Spec §2.2): protocol-id `0x00` = AMQP, version 1.0.0.
const AMQP_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x00, 0x01, 0x00, 0x00];
/// SASL protocol header (Spec §5.2.1): protocol-id `0x03` = SASL, version 1.0.0.
const SASL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x03, 0x01, 0x00, 0x00];

/// Hard cap on a single AMQP 1.0 frame (16 MiB).
///
/// TCP is a stream transport: the peer-announced frame `size` (a `u32`,
/// up to ~4 GiB, Spec §2.3.1) cannot be checked against "bytes
/// remaining" the way an in-memory buffer decode can — there is no
/// bound until this cap is applied. This client does not negotiate
/// `max-frame-size` via the `open` performative, so the cap is a fixed
/// ceiling rather than a broker-negotiated one. Mirrors the
/// established `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE`
/// DoS-cap pattern: reject before allocating.
const MAX_AMQP_FRAME_SIZE: usize = 16 * 1024 * 1024;

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

        self.detach_link(handle)
    }

    /// Publishes `payload` to `address` and returns only once the broker has
    /// settled the delivery as `accepted` (AMQP 1.0 Part 3 §3.4.2).
    ///
    /// [`Self::send_to`] transfers pre-settled, so AMQP gives its caller
    /// nothing to wait on and the broker may not hold the message yet when
    /// it returns. Here the link is attached with `snd-settle-mode` =
    /// `unsettled` (Part 2 §2.8.2), the transfer goes out unsettled, and the
    /// broker's disposition for that delivery-id decides: `accepted`
    /// returns, any other outcome is an error that names it. A `received`
    /// state (§3.4.1) is not an outcome and is waited past.
    ///
    /// # Errors
    /// I/O, a protocol error, a `rejected`, `released` or `modified`
    /// outcome (`ErrorKind::Other`, the outcome named), a disposition
    /// settled without an outcome, or none within the stream's read
    /// timeout.
    pub fn send_to_accepted(&mut self, address: &str, payload: &[u8]) -> io::Result<()> {
        let handle = self.handle;
        self.handle += 1;
        let link = format!("zd-snd-{handle}");
        self.write_frame(
            FrameType::Amqp,
            0,
            &attach_sender_unsettled(&link, handle, address).map_err(map_codec)?,
        )?;
        self.expect_amqp(descriptor::ATTACH)?;
        self.wait_for(descriptor::FLOW)?;

        let did = self.delivery_id;
        self.delivery_id += 1;
        let tag = did.to_be_bytes();
        let xfer = transfer_message(handle, did, &tag, payload, false).map_err(map_codec)?;
        self.write_frame(FrameType::Amqp, 0, &xfer)?;

        let outcome = self.await_outcome(did);
        // The link goes either way; an outcome error is the one reported.
        let detached = self.detach_link(handle);
        outcome?;
        detached
    }

    /// Detaches `handle` and reads the peer's own detach for it (AMQP 1.0
    /// §2.6.5), so the next attach on this session reads its own answer
    /// and not this one. Without it a second link on one connection takes
    /// the first link's detach for the reply to its attach.
    fn detach_link(&mut self, handle: u32) -> io::Result<()> {
        self.write_frame(
            FrameType::Amqp,
            0,
            &detach(handle, true).map_err(map_codec)?,
        )?;
        self.wait_for(descriptor::DETACH)
    }

    /// Reads frames until the broker's disposition settles `delivery_id`
    /// with an outcome. Bounded by the stream's read timeout per frame and
    /// by a frame count, so a peer that never answers ends the wait.
    fn await_outcome(&mut self, delivery_id: u32) -> io::Result<()> {
        for _ in 0..1024 {
            let body = self.read_frame(FrameType::Amqp)?;
            if body.is_empty() {
                continue;
            }
            let (desc, fields, _) = decode_performative(&body).map_err(map_codec)?;
            if desc != descriptor::DISPOSITION {
                continue;
            }
            let disposition = disposition_from_body(&fields)
                .ok_or_else(|| proto("a disposition without role and first"))?;
            if !disposition.from_receiver || !disposition.covers(delivery_id) {
                continue;
            }
            match disposition.state {
                Some(delivery_state::ACCEPTED) => return Ok(()),
                Some(delivery_state::RECEIVED) | None if !disposition.settled => {}
                Some(delivery_state::REJECTED) => return Err(outcome_error("rejected")),
                Some(delivery_state::RELEASED) => return Err(outcome_error("released")),
                Some(delivery_state::MODIFIED) => return Err(outcome_error("modified")),
                Some(other) => {
                    return Err(proto_owned(format!(
                        "delivery {delivery_id} settled with state {other:#x}, which is no outcome"
                    )));
                }
                None => {
                    return Err(proto_owned(format!(
                        "delivery {delivery_id} settled without an outcome"
                    )));
                }
            }
        }
        Err(proto("no disposition for the delivery"))
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
        self.detach_link(handle)?;
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
        if h.size as usize > MAX_AMQP_FRAME_SIZE {
            return Err(proto_owned(format!(
                "AMQP frame size {} exceeds MAX_AMQP_FRAME_SIZE ({MAX_AMQP_FRAME_SIZE})",
                h.size
            )));
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

    /// Reads one AMQP frame and asserts its performative descriptor. A
    /// `detach` or `close` in its place is the peer refusing, and the
    /// error it carries (AMQP 1.0 §2.8.14: condition and description) is
    /// what the returned error says.
    fn expect_amqp(&mut self, want: u64) -> io::Result<()> {
        let body = self.read_frame(FrameType::Amqp)?;
        let (desc, fields, _) = decode_performative(&body).map_err(map_codec)?;
        if desc != want {
            let refusal = match desc {
                descriptor::DETACH => peer_error(&fields, 2),
                descriptor::CLOSE | descriptor::END => peer_error(&fields, 0),
                _ => None,
            };
            return Err(proto_owned(match refusal {
                Some(error) => format!(
                    "expected performative {want:#x}, got {desc:#x}: the peer refused: {error}"
                ),
                None => format!("expected performative {want:#x}, got {desc:#x}"),
            }));
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
/// The `error` field of a performative at `index` (AMQP 1.0 §2.8.14, a
/// described list of condition, description and info), as
/// `condition: description`; `None` when there is none.
fn peer_error(fields: &zerodds_amqp_bridge::AmqpExtValue, index: usize) -> Option<String> {
    use zerodds_amqp_bridge::AmqpExtValue;
    let AmqpExtValue::List(fields) = fields else {
        return None;
    };
    let AmqpExtValue::Described { value, .. } = fields.get(index)? else {
        return None;
    };
    let AmqpExtValue::List(error) = value.as_ref() else {
        return None;
    };
    let text = |value: Option<&AmqpExtValue>| match value {
        Some(AmqpExtValue::Symbol(text) | AmqpExtValue::Str(text)) => Some(text.clone()),
        _ => None,
    };
    let condition = text(error.first()).unwrap_or_else(|| String::from("(no condition)"));
    Some(match text(error.get(1)) {
        Some(description) => format!("{condition}: {description}"),
        None => condition,
    })
}

/// A delivery the broker settled with an outcome other than `accepted`.
fn outcome_error(outcome: &str) -> io::Error {
    io::Error::other(format!("the broker settled the delivery as {outcome}"))
}
fn map_codec(e: zerodds_amqp_bridge::TypeError) -> io::Error {
    proto_owned(format!("amqp codec: {e:?}"))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use zerodds_amqp_bridge::AmqpExtValue;

    // -------------------------------------------------------------
    // Buffer-cap hardening — a peer-announced frame `size` above
    // MAX_AMQP_FRAME_SIZE must be rejected cleanly (no multi-GB
    // allocation attempt) right after the 8-byte header is read,
    // before the extended-header/body is read/allocated. Mirrors the
    // established `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE`
    // guard. `AmqpClient` is built directly (bypassing the full SASL +
    // `open`/`begin` handshake `connect_plain()` drives), since this
    // crate has no mock-broker test harness — `read_frame` is
    // exercised in isolation instead.
    // -------------------------------------------------------------

    #[test]
    fn read_frame_rejects_oversized_size_cleanly() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let handle = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect");
            // Header only: size far beyond MAX_AMQP_FRAME_SIZE, doff=2
            // (no extended header). No body bytes follow — read_frame
            // must reject before ever attempting to read/allocate it.
            let h = FrameHeader::new_amqp(u32::MAX, 2, 0);
            stream
                .write_all(&encode_frame_header(h))
                .expect("write header");
        });
        let (accepted, _) = listener.accept().expect("accept");
        let mut client = AmqpClient {
            stream: accepted,
            handle: 0,
            delivery_id: 0,
        };
        let res = client.read_frame(FrameType::Amqp);
        assert!(res.is_err(), "expected clean rejection, got {res:?}");
        handle.join().expect("writer thread");
    }

    /// The broker side of one `send_to_accepted`: answers the attach and
    /// grants credit, reads the transfer, then sends `replies` — each a
    /// `(delivery-id, state descriptor, settled)` disposition — and reads
    /// the detach. Returns what the client sent: the attach's
    /// `snd-settle-mode` and the transfer's `settled`.
    fn broker(
        addr: std::net::SocketAddr,
        replies: Vec<(u32, u64, bool)>,
    ) -> std::thread::JoinHandle<(AmqpExtValue, AmqpExtValue)> {
        use zerodds_amqp_bridge::encode_performative;
        std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect");
            let read = |stream: &mut TcpStream| {
                let mut hdr = [0u8; 8];
                stream.read_exact(&mut hdr).expect("header");
                let h = decode_frame_header(&hdr).expect("frame header");
                let mut body = vec![0u8; h.size as usize - 8];
                stream.read_exact(&mut body).expect("body");
                let (desc, fields, _) = decode_performative(&body).expect("performative");
                let AmqpExtValue::List(fields) = fields else {
                    panic!("a performative without a list body");
                };
                (desc, fields)
            };
            let write = |stream: &mut TcpStream, body: &[u8]| {
                let h = FrameHeader::new_amqp(8 + body.len() as u32, 2, 0);
                stream.write_all(&encode_frame_header(h)).expect("header");
                stream.write_all(body).expect("body");
            };
            let (desc, attach) = read(&mut stream);
            assert_eq!(desc, descriptor::ATTACH);
            write(
                &mut stream,
                &attach_to("broker", 0, false, "/queues/q").expect("attach"),
            );
            write(&mut stream, &flow_link_credit(0, 0, 100).expect("flow"));
            let (desc, transfer) = read(&mut stream);
            assert_eq!(desc, descriptor::TRANSFER);
            for (delivery_id, state, settled) in replies {
                let body = AmqpExtValue::List(vec![
                    AmqpExtValue::Boolean(true),
                    AmqpExtValue::Uint(delivery_id),
                    AmqpExtValue::Null,
                    AmqpExtValue::Boolean(settled),
                    AmqpExtValue::Described {
                        descriptor: state,
                        value: Box::new(AmqpExtValue::List(vec![])),
                    },
                ]);
                write(
                    &mut stream,
                    &encode_performative(descriptor::DISPOSITION, &body).expect("disposition"),
                );
            }
            let (desc, _) = read(&mut stream);
            assert_eq!(desc, descriptor::DETACH);
            // A broker answers a detach with its own (AMQP 1.0 §2.6.5).
            write(&mut stream, &detach(0, true).expect("detach"));
            (attach[3].clone(), transfer[4].clone())
        })
    }

    fn client_against(
        replies: Vec<(u32, u64, bool)>,
    ) -> (
        io::Result<()>,
        std::thread::JoinHandle<(AmqpExtValue, AmqpExtValue)>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let peer = broker(listener.local_addr().expect("local_addr"), replies);
        let (accepted, _) = listener.accept().expect("accept");
        accepted
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        let mut client = AmqpClient {
            stream: accepted,
            handle: 0,
            delivery_id: 0,
        };
        (client.send_to_accepted("/queues/q", b"payload"), peer)
    }

    /// AMQP 1.0 §2.8.2, §3.4: the link says `unsettled`, the transfer goes
    /// out unsettled, and the call returns on the disposition that accepts
    /// this delivery — not on one for another delivery, and not on the
    /// non-terminal `received`.
    #[test]
    fn send_to_accepted_returns_on_the_accepting_disposition() {
        let (result, peer) = client_against(vec![
            (7, delivery_state::ACCEPTED, true),
            (0, delivery_state::RECEIVED, false),
            (0, delivery_state::ACCEPTED, true),
        ]);
        result.expect("accepted");
        let (snd_settle_mode, settled) = peer.join().expect("broker");
        assert_eq!(snd_settle_mode, AmqpExtValue::Ubyte(0), "snd-settle-mode");
        assert_eq!(settled, AmqpExtValue::Boolean(false), "transfer settled");
    }

    #[test]
    fn send_to_accepted_reports_an_outcome_other_than_accepted() {
        for (state, name) in [
            (delivery_state::REJECTED, "rejected"),
            (delivery_state::RELEASED, "released"),
            (delivery_state::MODIFIED, "modified"),
        ] {
            let (result, peer) = client_against(vec![(0, state, true)]);
            let error = result.expect_err(name);
            assert!(error.to_string().contains(name), "{error}");
            let _ = peer.join().expect("broker");
        }
    }

    #[test]
    fn read_frame_within_bound_still_round_trips() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let body = [1u8, 2, 3, 4];
        let handle = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect");
            let h = FrameHeader::new_amqp(8 + body.len() as u32, 2, 0);
            stream
                .write_all(&encode_frame_header(h))
                .expect("write header");
            stream.write_all(&body).expect("write body");
        });
        let (accepted, _) = listener.accept().expect("accept");
        let mut client = AmqpClient {
            stream: accepted,
            handle: 0,
            delivery_id: 0,
        };
        let got = client.read_frame(FrameType::Amqp).expect("read_frame");
        assert_eq!(got, body);
        handle.join().expect("writer thread");
    }
}
