// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A broker side of AMQP 1.0 that a test drives frame by frame: it accepts
//! the bridge daemon's connection, answers the SASL layer (ANONYMOUS or
//! PLAIN), the protocol header and the open, begins and attaches what the
//! daemon begins and attaches, and hands the test every frame the daemon
//! sends. The test decides what the broker answers, so it can check the
//! fields of each performative (AMQP 1.0 Part 2 §2.7) and provoke what a
//! real broker does only now and then: a dropped connection, an ended
//! session, a settle mode `second`.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::frame::{
    FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};
use zerodds_amqp_bridge::performatives::{decode_performative, descriptor, encode_performative};

/// The AMQP protocol header (Part 2 §2.2) and the SASL one (Part 5 §5.3.1).
pub const AMQP_HEADER: [u8; 8] = *b"AMQP\x00\x01\x00\x00";
pub const SASL_HEADER: [u8; 8] = *b"AMQP\x03\x01\x00\x00";

/// How long the broker waits for the daemon's next frame.
pub const STEP: Duration = Duration::from_secs(10);

/// One frame of the daemon: channel, performative code, fields, payload.
/// An empty frame (a heartbeat) has code 0.
#[derive(Debug, Clone)]
pub struct Frame {
    pub channel: u16,
    pub code: u64,
    pub fields: Vec<AmqpExtValue>,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn field(&self, i: usize) -> &AmqpExtValue {
        self.fields.get(i).unwrap_or(&AmqpExtValue::Null)
    }

    pub fn uint(&self, i: usize) -> Option<u32> {
        match self.field(i) {
            AmqpExtValue::Uint(v) => Some(*v),
            AmqpExtValue::Ushort(v) => Some(u32::from(*v)),
            AmqpExtValue::Ubyte(v) => Some(u32::from(*v)),
            _ => None,
        }
    }

    pub fn boolean(&self, i: usize) -> Option<bool> {
        match self.field(i) {
            AmqpExtValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn text(&self, i: usize) -> Option<String> {
        match self.field(i) {
            AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) => Some(s.clone()),
            _ => None,
        }
    }
}

/// A link the daemon attached, as the broker sees it.
#[derive(Debug, Clone)]
pub struct Link {
    pub channel: u16,
    /// The daemon's handle, which the broker uses for its own end too.
    pub handle: u32,
    pub name: String,
    /// `true`: the daemon sends on it.
    pub daemon_sends: bool,
    /// The address of the node (target of a sender, source of a receiver).
    pub address: String,
    /// The daemon's attach.
    pub attach: Frame,
}

/// The listening broker.
pub struct Broker {
    listener: TcpListener,
    pub port: u16,
}

impl Broker {
    pub fn bind() -> Self {
        Self::bind_port(0)
    }

    /// A broker on `port` of 127.0.0.1 (0: one the system picks).
    pub fn bind_port(port: u16) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        Self { listener, port }
    }

    /// A port of 127.0.0.1 nobody listens on (until [`Broker::bind_port`]).
    pub fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    pub fn url(&self) -> String {
        format!("amqp://127.0.0.1:{}", self.port)
    }

    /// The next connection of the daemon, within `within`.
    pub fn accept(&self, within: Duration) -> Conn {
        self.try_accept(within)
            .unwrap_or_else(|| panic!("the daemon did not connect within {within:?}"))
    }

    /// The next connection of the daemon, or `None` when none comes within
    /// `within`.
    pub fn try_accept(&self, within: Duration) -> Option<Conn> {
        let deadline = Instant::now() + within;
        loop {
            match self.listener.accept() {
                Ok((sock, _)) => {
                    sock.set_nonblocking(false).unwrap();
                    sock.set_read_timeout(Some(Duration::from_millis(50)))
                        .unwrap();
                    sock.set_nodelay(true).unwrap();
                    return Some(Conn {
                        sock,
                        buf: Vec::new(),
                        backlog: VecDeque::new(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("accept: {e}"),
            }
        }
    }
}

/// One connection of the daemon.
pub struct Conn {
    sock: TcpStream,
    buf: Vec<u8>,
    /// Frames read while waiting for another kind.
    backlog: VecDeque<Frame>,
}

impl Conn {
    /// Reads until `n` octets are buffered; `false` when `deadline` passes
    /// or the daemon closed its end first.
    fn fill(&mut self, n: usize, deadline: Instant) -> bool {
        let mut chunk = [0u8; 65_536];
        while self.buf.len() < n {
            if Instant::now() >= deadline {
                return false;
            }
            match self.sock.read(&mut chunk) {
                Ok(0) => return false,
                Ok(k) => self.buf.extend_from_slice(&chunk[..k]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(e) => panic!("read: {e}"),
            }
        }
        true
    }

    /// The end of the stream within `within`: the daemon closed its end.
    pub fn expect_eof(&mut self, within: Duration) {
        let deadline = Instant::now() + within;
        let mut chunk = [0u8; 4096];
        loop {
            assert!(Instant::now() < deadline, "the daemon kept the connection");
            match self.sock.read(&mut chunk) {
                Ok(0) => return,
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => return,
            }
        }
    }

    fn take(&mut self, n: usize) -> Vec<u8> {
        self.buf.drain(..n).collect()
    }

    fn read_header(&mut self) -> [u8; 8] {
        assert!(
            self.fill(8, Instant::now() + STEP),
            "no protocol header within {STEP:?}"
        );
        self.take(8).try_into().unwrap()
    }

    /// The next raw frame (type, channel, body) within `within`.
    fn raw(&mut self, within: Duration) -> Option<(FrameType, u16, Vec<u8>)> {
        let deadline = Instant::now() + within;
        if !self.fill(8, deadline) {
            return None;
        }
        let h = decode_frame_header(&self.buf[..8]).unwrap();
        let size = h.size as usize;
        let doff = usize::from(h.doff) * 4;
        assert!(
            self.fill(size, Instant::now() + STEP),
            "a frame cut short or the connection closed"
        );
        let frame = self.take(size);
        Some((h.frame_type, h.channel, frame[doff..].to_vec()))
    }

    fn write_raw(&mut self, t: FrameType, channel: u16, body: &[u8]) {
        let h = encode_frame_header(FrameHeader {
            size: 8 + body.len() as u32,
            doff: 2,
            frame_type: t,
            channel,
        });
        self.sock.write_all(&h).unwrap();
        self.sock.write_all(body).unwrap();
        self.sock.flush().unwrap();
    }

    /// Sends a performative with `fields` and `payload` on `channel`.
    pub fn send(&mut self, channel: u16, code: u64, fields: Vec<AmqpExtValue>, payload: &[u8]) {
        let mut body = encode_performative(code, &AmqpExtValue::List(fields)).unwrap();
        body.extend_from_slice(payload);
        self.write_raw(FrameType::Amqp, channel, &body);
    }

    /// The daemon's next frame that is not a heartbeat, within `within`;
    /// `None` also when the daemon closed the connection.
    pub fn try_next(&mut self, within: Duration) -> Option<Frame> {
        if let Some(f) = self.backlog.pop_front() {
            return Some(f);
        }
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let (_, channel, body) = self.raw(left)?;
            if body.is_empty() {
                continue;
            }
            let (code, value, used) = decode_performative(&body).unwrap();
            let AmqpExtValue::List(fields) = value else {
                panic!("a performative is a list")
            };
            return Some(Frame {
                channel,
                code,
                fields,
                payload: body[used..].to_vec(),
            });
        }
    }

    /// The daemon's next frame.
    pub fn next(&mut self) -> Frame {
        self.try_next(STEP)
            .unwrap_or_else(|| panic!("no frame from the daemon within {STEP:?}"))
    }

    /// The daemon's next frame with performative `code`; frames of other
    /// kinds read on the way stay for [`Conn::next`].
    pub fn next_of(&mut self, code: u64) -> Frame {
        let mut skipped = Vec::new();
        let found = loop {
            let f = self.next();
            if f.code == code {
                break f;
            }
            skipped.push(f);
        };
        for f in skipped.into_iter().rev() {
            self.backlog.push_front(f);
        }
        found
    }

    /// The SASL layer (Part 5 §5.3): the SASL header both ways, `offer` as
    /// the mechanisms, the daemon's init, the outcome `code` (0 ok).
    /// Returns the mechanism the daemon chose.
    pub fn sasl(&mut self, offer: &[&str], code: u8) -> String {
        self.sasl_by(offer, |_| code)
    }

    /// [`Conn::sasl`] with the outcome code `decide` gives the mechanism
    /// the daemon chose.
    pub fn sasl_by(&mut self, offer: &[&str], decide: impl Fn(&str) -> u8) -> String {
        let hdr = self.read_header();
        assert_eq!(hdr, SASL_HEADER, "the SASL header first");
        self.sock.write_all(&SASL_HEADER).unwrap();
        let mechanisms = offer
            .iter()
            .map(|m| AmqpExtValue::Symbol((*m).into()))
            .collect();
        let body = encode_performative(
            descriptor::SASL_MECHANISMS,
            &AmqpExtValue::List(vec![AmqpExtValue::Array(mechanisms)]),
        )
        .unwrap();
        self.write_raw(FrameType::Sasl, 0, &body);
        let Some((FrameType::Sasl, _, init)) = self.raw(STEP) else {
            return String::new();
        };
        let (_, value, _) = decode_performative(&init).unwrap();
        let chosen = match &value {
            AmqpExtValue::List(f) => match f.first() {
                Some(AmqpExtValue::Symbol(s)) => s.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        };
        let code = decide(&chosen);
        let body = encode_performative(
            descriptor::SASL_OUTCOME,
            &AmqpExtValue::List(vec![AmqpExtValue::Ubyte(code)]),
        )
        .unwrap();
        self.write_raw(FrameType::Sasl, 0, &body);
        chosen
    }

    /// The AMQP header both ways and the daemon's open; returns its fields.
    pub fn read_open(&mut self) -> Frame {
        let hdr = self.read_header();
        assert_eq!(hdr, AMQP_HEADER);
        self.sock.write_all(&AMQP_HEADER).unwrap();
        let open = self.next();
        assert_eq!(open.code, descriptor::OPEN, "{open:?}");
        open
    }

    /// The times at which the daemon's empty frames (heartbeats, Part 2
    /// §2.4.5) arrive within `within`; other frames stay for
    /// [`Conn::next`].
    pub fn heartbeats(&mut self, within: Duration) -> Vec<Instant> {
        let deadline = Instant::now() + within;
        let mut beats = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return beats;
            }
            let Some((_, channel, body)) = self.raw(left) else {
                return beats;
            };
            if body.is_empty() {
                beats.push(Instant::now());
                continue;
            }
            let (code, value, used) = decode_performative(&body).unwrap();
            let AmqpExtValue::List(fields) = value else {
                panic!("a performative is a list")
            };
            self.backlog.push_back(Frame {
                channel,
                code,
                fields,
                payload: body[used..].to_vec(),
            });
        }
    }

    /// The broker's open with `idle-time-out` `idle_ms` (Part 2 §2.4.5).
    pub fn send_open_with_idle(&mut self, channel_max: u16, idle_ms: u32) {
        self.send(
            0,
            descriptor::OPEN,
            vec![
                AmqpExtValue::Str("scripted-broker".into()),
                AmqpExtValue::Null,
                AmqpExtValue::Uint(65_536),
                AmqpExtValue::Ushort(channel_max),
                AmqpExtValue::Uint(idle_ms),
            ],
            &[],
        );
    }

    /// The broker's open: container `broker`, `channel_max`.
    pub fn send_open(&mut self, channel_max: u16) {
        self.send(
            0,
            descriptor::OPEN,
            vec![
                AmqpExtValue::Str("scripted-broker".into()),
                AmqpExtValue::Null,
                AmqpExtValue::Uint(65_536),
                AmqpExtValue::Ushort(channel_max),
            ],
            &[],
        );
    }

    /// SASL ANONYMOUS, header, both opens; returns the daemon's open.
    pub fn handshake(&mut self) -> Frame {
        assert_eq!(self.sasl(&["ANONYMOUS", "PLAIN"], 0), "ANONYMOUS");
        let open = self.read_open();
        self.send_open(255);
        open
    }

    /// Answers a begin of the daemon on its channel (Part 2 §2.7.2).
    pub fn answer_begin(&mut self, begin: &Frame) {
        assert_eq!(begin.code, descriptor::BEGIN);
        self.send(
            begin.channel,
            descriptor::BEGIN,
            vec![
                AmqpExtValue::Ushort(begin.channel),
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(10_000),
                AmqpExtValue::Uint(10_000),
            ],
            &[],
        );
    }

    /// Answers an attach of the daemon (Part 2 §2.7.3): the other role,
    /// the same name, handle and termini, the broker's `rcv_settle_mode`,
    /// an initial delivery count of 0 where the broker sends.
    pub fn answer_attach(&mut self, attach: &Frame, rcv_settle_mode: u8) -> Link {
        assert_eq!(attach.code, descriptor::ATTACH);
        let daemon_receives = attach.boolean(2) == Some(true);
        let node = if daemon_receives {
            attach.field(5)
        } else {
            attach.field(6)
        };
        let address = match node {
            AmqpExtValue::Described { value, .. } => match &**value {
                AmqpExtValue::List(f) => match f.first() {
                    Some(AmqpExtValue::Str(s)) => s.clone(),
                    _ => String::new(),
                },
                _ => String::new(),
            },
            _ => String::new(),
        };
        let handle = attach.uint(1).unwrap();
        let name = attach.text(0).unwrap();
        self.send(
            attach.channel,
            descriptor::ATTACH,
            vec![
                AmqpExtValue::Str(name.clone()),
                AmqpExtValue::Uint(handle),
                AmqpExtValue::Boolean(!daemon_receives),
                attach.field(3).clone(),
                AmqpExtValue::Ubyte(rcv_settle_mode),
                attach.field(5).clone(),
                attach.field(6).clone(),
                AmqpExtValue::Null,
                AmqpExtValue::Boolean(false),
                if daemon_receives {
                    AmqpExtValue::Uint(0)
                } else {
                    AmqpExtValue::Null
                },
            ],
            &[],
        );
        Link {
            channel: attach.channel,
            handle,
            name,
            daemon_sends: !daemon_receives,
            address,
            attach: attach.clone(),
        }
    }

    /// Answers the begins and attaches of `n` links, in the order the
    /// daemon attaches them; other frames stay for [`Conn::next`].
    pub fn setup_links(&mut self, n: usize, rcv_settle_mode: u8) -> Vec<Link> {
        let mut links = Vec::new();
        let mut skipped = Vec::new();
        while links.len() < n {
            let f = self.next();
            match f.code {
                descriptor::BEGIN => self.answer_begin(&f),
                descriptor::ATTACH => links.push(self.answer_attach(&f, rcv_settle_mode)),
                _ => skipped.push(f),
            }
        }
        for f in skipped.into_iter().rev() {
            self.backlog.push_front(f);
        }
        links
    }

    /// Link credit for a link the daemon sends on (Part 2 §2.7.4).
    pub fn flow(&mut self, link: &Link, delivery_count: u32, credit: u32) {
        self.send(
            link.channel,
            descriptor::FLOW,
            vec![
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(10_000),
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(10_000),
                AmqpExtValue::Uint(link.handle),
                AmqpExtValue::Uint(delivery_count),
                AmqpExtValue::Uint(credit),
            ],
            &[],
        );
    }

    /// The flow state of `link` with `drain` and `echo` (Part 2 §2.7.4):
    /// as the receiver of a link the peer sends on, the credit and the
    /// drain mode it asks for; as the sender of one the peer receives on,
    /// its own delivery-count and credit.
    pub fn flow_with(
        &mut self,
        link: &Link,
        delivery_count: u32,
        credit: u32,
        drain: bool,
        echo: bool,
    ) {
        self.send(
            link.channel,
            descriptor::FLOW,
            vec![
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(10_000),
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(10_000),
                AmqpExtValue::Uint(link.handle),
                AmqpExtValue::Uint(delivery_count),
                AmqpExtValue::Uint(credit),
                AmqpExtValue::Null,
                AmqpExtValue::Boolean(drain),
                AmqpExtValue::Boolean(echo),
            ],
            &[],
        );
    }

    /// A flow of session `channel` alone, without a link, that asks for
    /// the peer's session state (Part 2 §2.7.4 `echo`).
    pub fn session_echo(&mut self, channel: u16) {
        self.send(
            channel,
            descriptor::FLOW,
            vec![
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(10_000),
                AmqpExtValue::Uint(0),
                AmqpExtValue::Uint(10_000),
                AmqpExtValue::Null,
                AmqpExtValue::Null,
                AmqpExtValue::Null,
                AmqpExtValue::Null,
                AmqpExtValue::Null,
                AmqpExtValue::Boolean(true),
            ],
            &[],
        );
    }

    /// A message on a link the daemon receives on.
    pub fn transfer(&mut self, link: &Link, delivery_id: u32, settled: bool, message: &[u8]) {
        self.send(
            link.channel,
            descriptor::TRANSFER,
            vec![
                AmqpExtValue::Uint(link.handle),
                AmqpExtValue::Uint(delivery_id),
                AmqpExtValue::Binary(delivery_id.to_be_bytes().to_vec()),
                AmqpExtValue::Uint(0),
                AmqpExtValue::Boolean(settled),
                AmqpExtValue::Boolean(false),
            ],
            message,
        );
    }

    /// A disposition of the broker: `role_receiver` for the daemon's
    /// deliveries, `state` an encoded delivery state or null.
    pub fn disposition(
        &mut self,
        channel: u16,
        role_receiver: bool,
        first: u32,
        last: u32,
        settled: bool,
        state: AmqpExtValue,
    ) {
        self.send(
            channel,
            descriptor::DISPOSITION,
            vec![
                AmqpExtValue::Boolean(role_receiver),
                AmqpExtValue::Uint(first),
                AmqpExtValue::Uint(last),
                AmqpExtValue::Boolean(settled),
                state,
            ],
            &[],
        );
    }

    /// Ends session `channel` with an error.
    pub fn end(&mut self, channel: u16, condition: &str) {
        self.send(channel, descriptor::END, vec![error(condition)], &[]);
    }

    /// Closes the connection with an error.
    pub fn close(&mut self, condition: &str) {
        self.send(0, descriptor::CLOSE, vec![error(condition)], &[]);
    }

    /// Drops the TCP connection without a close.
    pub fn drop_connection(self) {
        let _ = self.sock.shutdown(std::net::Shutdown::Both);
    }
}

/// An AMQP error (Part 2 §2.8.14) with `condition`.
pub fn error(condition: &str) -> AmqpExtValue {
    AmqpExtValue::Described {
        descriptor: 0x1d,
        value: Box::new(AmqpExtValue::List(vec![AmqpExtValue::Symbol(
            condition.into(),
        )])),
    }
}

/// The `accepted` outcome (Part 3 §3.4.2).
pub fn accepted() -> AmqpExtValue {
    AmqpExtValue::Described {
        descriptor: 0x24,
        value: Box::new(AmqpExtValue::List(vec![])),
    }
}

/// The descriptor of a described value, or `None`.
pub fn descriptor_of(v: &AmqpExtValue) -> Option<u64> {
    match v {
        AmqpExtValue::Described { descriptor, .. } => Some(*descriptor),
        _ => None,
    }
}
