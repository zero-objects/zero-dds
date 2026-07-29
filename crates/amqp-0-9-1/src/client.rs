// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A synchronous AMQP 0.9.1 broker client (RabbitMQ-compatible).
//!
//! Drives the full connection handshake (protocol header → connection.start /
//! start-ok → tune / tune-ok → open / open-ok → channel.open / open-ok) and
//! then publish (basic.publish + content header + body) and consume
//! (basic.get → get-ok + header + body → basic.ack). `std`-only (TCP).

use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::frame::{Frame, FrameType, PROTOCOL_HEADER};
use crate::method::{self, class, id};
use crate::types::WireError;

const CHANNEL: u16 = 1;

/// Hard ceiling on a single AMQP frame (16 MiB), used until
/// `connection.tune` negotiates the broker's actual `frame-max`, and as
/// an upper clamp on whatever `frame-max` the broker proposes afterward.
///
/// TCP is a stream transport: the broker-announced frame `size` (a
/// `u32`, up to ~4 GiB) cannot be checked against "bytes remaining" the
/// way an in-memory buffer decode can. AMQP 0-9-1 itself defines
/// `frame-max` (negotiated in `connection.tune`) for exactly this
/// purpose, but a malicious/compromised broker could still propose an
/// unreasonably large value — so this ceiling is enforced on our side
/// regardless of what the broker claims. Mirrors the established
/// `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE` DoS-cap
/// pattern: reject before allocating.
const MAX_AMQP_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// An established AMQP 0.9.1 connection + open channel.
pub struct Amqp091Client {
    stream: TcpStream,
    /// Negotiated (and hard-capped) per-frame size ceiling — see
    /// [`MAX_AMQP_FRAME_SIZE`]. Defaults to that ceiling until
    /// `connection.tune` narrows it to the broker's actual `frame-max`.
    frame_max: usize,
}

impl Amqp091Client {
    /// Connects to `addr` and performs the AMQP 0.9.1 handshake with PLAIN
    /// auth on `vhost`, leaving channel 1 open.
    ///
    /// # Errors
    /// I/O or protocol error (incl. auth failure → the broker closes the
    /// connection during the handshake).
    pub fn connect<A: ToSocketAddrs>(
        addr: A,
        user: &str,
        pass: &str,
        vhost: &str,
    ) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        stream.set_nodelay(true)?;
        let mut c = Self {
            stream,
            frame_max: MAX_AMQP_FRAME_SIZE,
        };

        // Protocol header.
        c.stream.write_all(&PROTOCOL_HEADER)?;

        // connection.start (server) → connection.start-ok (client).
        c.expect_method(id::CONNECTION_START)?;
        c.send_method(0, &method::connection_start_ok(user, pass).map_err(wire)?)?;

        // connection.tune (server) → connection.tune-ok (client).
        let tune = c.expect_method(id::CONNECTION_TUNE)?;
        let (chan_max, frame_max, _hb) = method::connection_tune_params(&tune).map_err(wire)?;
        // Adopt the broker's negotiated frame-max, but never above our
        // own hard ceiling — a malicious/compromised broker proposing
        // an oversized frame-max must not widen what we're willing to
        // allocate for a single frame.
        c.frame_max = (frame_max as usize).min(MAX_AMQP_FRAME_SIZE);
        // Disable heartbeats (0) for the simple synchronous client.
        c.send_method(0, &method::connection_tune_ok(chan_max, frame_max, 0))?;

        // connection.open → open-ok.
        c.send_method(0, &method::connection_open(vhost).map_err(wire)?)?;
        c.expect_method(id::CONNECTION_OPEN_OK)?;

        // channel.open → open-ok.
        c.send_method(CHANNEL, &method::channel_open().map_err(wire)?)?;
        c.expect_method(id::CHANNEL_OPEN_OK)?;

        Ok(c)
    }

    /// Declares a (durable) queue. Returns the queue name the broker echoes.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn queue_declare(&mut self, queue: &str, durable: bool) -> io::Result<String> {
        self.send_method(
            CHANNEL,
            &method::queue_declare(queue, durable).map_err(wire)?,
        )?;
        let ok = self.expect_method(id::QUEUE_DECLARE_OK)?;
        method::queue_declare_ok_name(&ok).map_err(wire)
    }

    /// Publishes `payload` to `exchange` with `routing_key` (the default
    /// exchange `""` routes by queue name). Sends basic.publish + a content
    /// header + a body frame.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn publish(&mut self, exchange: &str, routing_key: &str, payload: &[u8]) -> io::Result<()> {
        self.publish_with_props(
            exchange,
            routing_key,
            payload,
            &method::ContentProperties::default(),
        )
    }

    /// Like [`Self::publish`], but attaches content properties (content-type,
    /// delivery-mode, headers, …).
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn publish_with_props(
        &mut self,
        exchange: &str,
        routing_key: &str,
        payload: &[u8],
        props: &method::ContentProperties,
    ) -> io::Result<()> {
        self.send_method(
            CHANNEL,
            &method::basic_publish(exchange, routing_key).map_err(wire)?,
        )?;
        self.send_frame(
            FrameType::Header,
            &method::content_header_with_props(class::BASIC, payload.len() as u64, props)
                .map_err(wire)?,
        )?;
        if !payload.is_empty() {
            self.send_frame(FrameType::Body, payload)?;
        }
        Ok(())
    }

    /// Synchronously fetches one message from `queue` (basic.get). Returns the
    /// body, or `None` on basic.get-empty. The message is acked.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn get(&mut self, queue: &str) -> io::Result<Option<Vec<u8>>> {
        self.send_method(CHANNEL, &method::basic_get(queue, false).map_err(wire)?)?;
        let frame = self.read_frame()?;
        if frame.frame_type != FrameType::Method {
            return Err(proto("expected a method frame after basic.get"));
        }
        let mid = method::method_id(&frame.payload).map_err(wire)?;
        if mid == id::BASIC_GET_EMPTY {
            return Ok(None);
        }
        if mid != id::BASIC_GET_OK {
            return Err(proto_owned(format!("expected basic.get-ok, got {mid:?}")));
        }
        let delivery_tag = method::basic_get_ok_delivery_tag(&frame.payload).map_err(wire)?;
        let body = self.read_content()?;

        // Ack.
        self.send_method(CHANNEL, &method::basic_ack(delivery_tag, false))?;
        Ok(Some(body))
    }

    /// Like [`Self::get`], but also returns the message's content properties.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn get_with_props(
        &mut self,
        queue: &str,
    ) -> io::Result<Option<(Vec<u8>, method::ContentProperties)>> {
        self.send_method(CHANNEL, &method::basic_get(queue, false).map_err(wire)?)?;
        let frame = self.read_frame()?;
        if frame.frame_type != FrameType::Method {
            return Err(proto("expected a method frame after basic.get"));
        }
        let mid = method::method_id(&frame.payload).map_err(wire)?;
        if mid == id::BASIC_GET_EMPTY {
            return Ok(None);
        }
        if mid != id::BASIC_GET_OK {
            return Err(proto_owned(format!("expected basic.get-ok, got {mid:?}")));
        }
        let delivery_tag = method::basic_get_ok_delivery_tag(&frame.payload).map_err(wire)?;
        let (body, props) = self.read_content_full()?;
        self.send_method(CHANNEL, &method::basic_ack(delivery_tag, false))?;
        Ok(Some((body, props)))
    }

    /// Like [`Self::get`], but **rejects** the message instead of acking it.
    /// With `requeue = true` the broker re-queues it. Returns the body, or
    /// `None` on basic.get-empty.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn get_reject(&mut self, queue: &str, requeue: bool) -> io::Result<Option<Vec<u8>>> {
        self.send_method(CHANNEL, &method::basic_get(queue, false).map_err(wire)?)?;
        let frame = self.read_frame()?;
        if frame.frame_type != FrameType::Method {
            return Err(proto("expected a method frame after basic.get"));
        }
        let mid = method::method_id(&frame.payload).map_err(wire)?;
        if mid == id::BASIC_GET_EMPTY {
            return Ok(None);
        }
        if mid != id::BASIC_GET_OK {
            return Err(proto_owned(format!("expected basic.get-ok, got {mid:?}")));
        }
        let delivery_tag = method::basic_get_ok_delivery_tag(&frame.payload).map_err(wire)?;
        let body = self.read_content()?;
        self.send_method(CHANNEL, &method::basic_reject(delivery_tag, requeue))?;
        Ok(Some(body))
    }

    // ---- exchange class -----------------------------------------------

    /// Declares an exchange (`direct`/`fanout`/`topic`/`headers`).
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn exchange_declare(&mut self, name: &str, kind: &str, durable: bool) -> io::Result<()> {
        self.send_method(
            CHANNEL,
            &method::exchange_declare(name, kind, durable).map_err(wire)?,
        )?;
        self.expect_method(id::EXCHANGE_DECLARE_OK)?;
        Ok(())
    }

    /// Deletes an exchange.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn exchange_delete(&mut self, name: &str, if_unused: bool) -> io::Result<()> {
        self.send_method(
            CHANNEL,
            &method::exchange_delete(name, if_unused).map_err(wire)?,
        )?;
        self.expect_method(id::EXCHANGE_DELETE_OK)?;
        Ok(())
    }

    // ---- queue bind / unbind / purge / delete -------------------------

    /// Binds `queue` to `exchange` with `routing_key`.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn queue_bind(&mut self, queue: &str, exchange: &str, routing_key: &str) -> io::Result<()> {
        self.send_method(
            CHANNEL,
            &method::queue_bind(queue, exchange, routing_key).map_err(wire)?,
        )?;
        self.expect_method(id::QUEUE_BIND_OK)?;
        Ok(())
    }

    /// Unbinds `queue` from `exchange` for `routing_key`.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn queue_unbind(
        &mut self,
        queue: &str,
        exchange: &str,
        routing_key: &str,
    ) -> io::Result<()> {
        self.send_method(
            CHANNEL,
            &method::queue_unbind(queue, exchange, routing_key).map_err(wire)?,
        )?;
        self.expect_method(id::QUEUE_UNBIND_OK)?;
        Ok(())
    }

    /// Purges all messages from `queue`, returning the purged count.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn queue_purge(&mut self, queue: &str) -> io::Result<u32> {
        self.send_method(CHANNEL, &method::queue_purge(queue).map_err(wire)?)?;
        let ok = self.expect_method(id::QUEUE_PURGE_OK)?;
        method::queue_op_ok_message_count(&ok).map_err(wire)
    }

    /// Deletes `queue`, returning the count of messages it held.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn queue_delete(&mut self, queue: &str) -> io::Result<u32> {
        self.send_method(CHANNEL, &method::queue_delete(queue).map_err(wire)?)?;
        let ok = self.expect_method(id::QUEUE_DELETE_OK)?;
        method::queue_op_ok_message_count(&ok).map_err(wire)
    }

    // ---- channel flow / close -----------------------------------------

    /// Sends channel.flow and awaits the broker's flow-ok echo.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn channel_flow(&mut self, active: bool) -> io::Result<()> {
        self.send_method(CHANNEL, &method::channel_flow(active))?;
        self.expect_method(id::CHANNEL_FLOW_OK)?;
        Ok(())
    }

    /// Gracefully closes the channel (channel.close → close-ok), leaving the
    /// connection open.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn channel_close(&mut self) -> io::Result<()> {
        self.send_method(CHANNEL, &method::channel_close(200, "ok").map_err(wire)?)?;
        self.expect_method(id::CHANNEL_CLOSE_OK)?;
        Ok(())
    }

    // ---- basic.qos / async consume ------------------------------------

    /// Sets the consumer prefetch window (basic.qos).
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn qos(&mut self, prefetch_count: u16) -> io::Result<()> {
        self.send_method(CHANNEL, &method::basic_qos(0, prefetch_count, false))?;
        self.expect_method(id::BASIC_QOS_OK)?;
        Ok(())
    }

    /// Subscribes with basic.consume and blocks for the **first** asynchronous
    /// basic.deliver, acks it, then cancels the subscription. Exercises the
    /// async push path (consume/deliver/ack/cancel) distinct from basic.get.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn consume_one(&mut self, queue: &str) -> io::Result<Vec<u8>> {
        self.send_method(
            CHANNEL,
            &method::basic_consume(queue, "", false).map_err(wire)?,
        )?;
        let ok = self.expect_method(id::BASIC_CONSUME_OK)?;
        let consumer_tag = method::basic_consume_ok_tag(&ok).map_err(wire)?;

        // Wait for the server to push a delivery.
        let deliver = self.expect_method(id::BASIC_DELIVER)?;
        let delivery_tag = method::basic_deliver_delivery_tag(&deliver).map_err(wire)?;
        let body = self.read_content()?;
        self.send_method(CHANNEL, &method::basic_ack(delivery_tag, false))?;

        // Cancel the subscription.
        self.send_method(
            CHANNEL,
            &method::basic_cancel(&consumer_tag, false).map_err(wire)?,
        )?;
        self.expect_method(id::BASIC_CANCEL_OK)?;
        Ok(body)
    }

    // ---- publisher confirms (confirm class) ---------------------------

    /// Puts the channel into publisher-confirm mode (confirm.select).
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn confirm_select(&mut self) -> io::Result<()> {
        self.send_method(CHANNEL, &method::confirm_select(false))?;
        self.expect_method(id::CONFIRM_SELECT_OK)?;
        Ok(())
    }

    /// Publishes and blocks for the broker's publisher-confirm (basic.ack).
    /// Requires [`Self::confirm_select`] to have been called.
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn publish_confirmed(
        &mut self,
        exchange: &str,
        routing_key: &str,
        payload: &[u8],
    ) -> io::Result<u64> {
        self.publish(exchange, routing_key, payload)?;
        let ack = self.expect_method(id::BASIC_ACK)?;
        method::basic_ack_delivery_tag(&ack).map_err(wire)
    }

    // ---- transactions (tx class) --------------------------------------

    /// Starts a transaction (tx.select).
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn tx_select(&mut self) -> io::Result<()> {
        self.send_method(CHANNEL, &method::tx_select())?;
        self.expect_method(id::TX_SELECT_OK)?;
        Ok(())
    }

    /// Commits the current transaction (tx.commit).
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn tx_commit(&mut self) -> io::Result<()> {
        self.send_method(CHANNEL, &method::tx_commit())?;
        self.expect_method(id::TX_COMMIT_OK)?;
        Ok(())
    }

    /// Rolls back the current transaction (tx.rollback).
    ///
    /// # Errors
    /// I/O or protocol error.
    pub fn tx_rollback(&mut self) -> io::Result<()> {
        self.send_method(CHANNEL, &method::tx_rollback())?;
        self.expect_method(id::TX_ROLLBACK_OK)?;
        Ok(())
    }

    /// Reads a content header frame + body frame(s), returning the assembled
    /// body. Shared by basic.get and basic.deliver.
    fn read_content(&mut self) -> io::Result<Vec<u8>> {
        Ok(self.read_content_full()?.0)
    }

    /// Reads a content header + body, returning `(body, properties)`.
    fn read_content_full(&mut self) -> io::Result<(Vec<u8>, method::ContentProperties)> {
        let header = self.read_frame()?;
        if header.frame_type != FrameType::Header {
            return Err(proto("expected a content header frame"));
        }
        let (body_size, props) = method::parse_content_header(&header.payload).map_err(wire)?;
        let body_size = body_size as usize;
        // `body_size` is the *total* content size, legitimately larger
        // than a single frame (that's the point of splitting it across
        // multiple body frames, each individually bounded by
        // `self.frame_max` in `read_frame`). So this must not reject —
        // only avoid pre-allocating the whole broker-announced total
        // upfront; `Vec::with_capacity` is merely a perf hint, and the
        // loop below grows the buffer safely via `extend_from_slice` as
        // bytes are actually, verifiably received.
        let mut body = Vec::with_capacity(body_size.min(self.frame_max));
        while body.len() < body_size {
            let bf = self.read_frame()?;
            if bf.frame_type != FrameType::Body {
                return Err(proto("expected a content body frame"));
            }
            body.extend_from_slice(&bf.payload);
        }
        Ok((body, props))
    }

    /// Closes the connection (best-effort): connection.close + close-ok.
    pub fn close(mut self) {
        let mut w = crate::types::Writer::new();
        w.u16(id::CONNECTION_CLOSE.0)
            .u16(id::CONNECTION_CLOSE.1)
            .u16(200); // reply-code 200 = success
        let _ = w.short_str("bye"); // reply-text
        w.u16(0).u16(0); // class-id, method-id
        let _ = self.send_method(0, &w.into_bytes());
    }

    // ---- framing -------------------------------------------------------

    fn send_method(&mut self, channel: u16, payload: &[u8]) -> io::Result<()> {
        self.send_frame_on(FrameType::Method, channel, payload)
    }
    fn send_frame(&mut self, ft: FrameType, payload: &[u8]) -> io::Result<()> {
        self.send_frame_on(ft, CHANNEL, payload)
    }
    fn send_frame_on(&mut self, ft: FrameType, channel: u16, payload: &[u8]) -> io::Result<()> {
        let f = Frame {
            frame_type: ft,
            channel,
            payload: payload.to_vec(),
        };
        self.stream.write_all(&f.encode())
    }

    fn read_frame(&mut self) -> io::Result<Frame> {
        // Fixed 7-byte header: type + channel + size.
        let mut head = [0u8; 7];
        self.stream.read_exact(&mut head)?;
        let ft = FrameType::from_u8(head[0])
            .ok_or_else(|| proto_owned(format!("bad frame type {}", head[0])))?;
        let channel = u16::from_be_bytes([head[1], head[2]]);
        let size = u32::from_be_bytes([head[3], head[4], head[5], head[6]]) as usize;
        if size > self.frame_max {
            return Err(proto_owned(format!(
                "AMQP frame size {size} exceeds the negotiated frame-max ({})",
                self.frame_max
            )));
        }
        let mut payload = vec![0u8; size];
        if size > 0 {
            self.stream.read_exact(&mut payload)?;
        }
        let mut end = [0u8; 1];
        self.stream.read_exact(&mut end)?;
        if end[0] != crate::frame::FRAME_END {
            return Err(proto("missing frame-end octet"));
        }
        Ok(Frame {
            frame_type: ft,
            channel,
            payload,
        })
    }

    /// Reads frames until a method frame with id `(class, method)` arrives,
    /// returning its payload. A `connection.close` from the broker (e.g. on
    /// auth failure) is surfaced as an error.
    fn expect_method(&mut self, want: (u16, u16)) -> io::Result<Vec<u8>> {
        for _ in 0..16 {
            let f = self.read_frame()?;
            if f.frame_type != FrameType::Method {
                continue;
            }
            let mid = method::method_id(&f.payload).map_err(wire)?;
            if mid == want {
                return Ok(f.payload);
            }
            if mid == id::CONNECTION_CLOSE {
                return Err(proto_owned(format!(
                    "broker closed the connection while awaiting {want:?}"
                )));
            }
            if mid.0 == class::CONNECTION || mid.0 == class::CHANNEL {
                // tolerate interleaved connection/channel methods
                continue;
            }
        }
        Err(proto_owned(format!("did not receive method {want:?}")))
    }
}

fn proto(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
fn proto_owned(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
fn wire(e: WireError) -> io::Error {
    proto_owned(format!("amqp-0-9-1 wire: {e:?}"))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    // -------------------------------------------------------------
    // Buffer-cap hardening — a peer-announced frame `size` above the
    // negotiated (and hard-capped) frame-max must be rejected cleanly
    // (no ~4 GB allocation attempt) right after the 7-byte header is
    // read, before the payload is read/allocated. Mirrors the
    // established `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE`
    // guard. `Amqp091Client` is built directly (bypassing the full
    // handshake `connect()` drives) since this crate has no mock-broker
    // test harness — `read_frame` is exercised in isolation instead.
    // -------------------------------------------------------------

    #[test]
    fn read_frame_rejects_oversized_size_cleanly() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let handle = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect");
            let mut head = [0u8; 7];
            head[0] = FrameType::Method as u8;
            head[1..3].copy_from_slice(&CHANNEL.to_be_bytes());
            // Announce a frame far beyond the negotiated frame-max; no
            // payload bytes follow — read_frame must reject before ever
            // attempting to read/allocate the payload.
            head[3..7].copy_from_slice(&u32::MAX.to_be_bytes());
            stream.write_all(&head).expect("write header");
        });
        let (accepted, _) = listener.accept().expect("accept");
        let mut client = Amqp091Client {
            stream: accepted,
            frame_max: MAX_AMQP_FRAME_SIZE,
        };
        let res = client.read_frame();
        assert!(res.is_err(), "expected clean rejection, got {res:?}");
        handle.join().expect("writer thread");
    }

    #[test]
    fn read_frame_within_bound_still_round_trips() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let handle = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("connect");
            let frame = Frame {
                frame_type: FrameType::Method,
                channel: CHANNEL,
                payload: vec![1, 2, 3, 4],
            };
            stream.write_all(&frame.encode()).expect("write frame");
        });
        let (accepted, _) = listener.accept().expect("accept");
        let mut client = Amqp091Client {
            stream: accepted,
            frame_max: MAX_AMQP_FRAME_SIZE,
        };
        let frame = client.read_frame().expect("read_frame");
        assert_eq!(frame.payload, vec![1, 2, 3, 4]);
        handle.join().expect("writer thread");
    }
}
