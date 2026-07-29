// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-endpoint-rust`. Safety classification: **STANDARD**.
//!
//! Native Rust endpoint SDK (ADR 0013): a thin XRCE + XCDR2 client over the
//! reference `zerodds-cdr` core. Sync (poll) and async (a std thread + `mpsc`
//! channel — no async runtime dependency). Byte-identical to the Rust core by
//! construction: the codec is `zerodds-cdr`'s own `BufferWriter`/`BufferReader`.

// The endpoint demo uses `expect` on infallible in-memory paths (poisoned locks,
// well-formed frames); the workspace denies `expect_used` for library code.
#![allow(clippy::expect_used)]

pub mod reliable;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};

/// A representative `@final` telemetry type — mirrors the deep examples of the
/// other endpoint SDKs (`Reading { id, value, label }`).
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// `uint32` id.
    pub id: u32,
    /// `float` value.
    pub value: f32,
    /// `string` label.
    pub label: String,
}

impl Reading {
    /// Encodes the XCDR2 body (no XRCE header), byte-identical to the other SDKs.
    #[must_use]
    pub fn marshal(&self, endian: Endianness) -> Vec<u8> {
        let mut w = BufferWriter::new(endian).xcdr2();
        w.write_u32(self.id).expect("id");
        w.write_u32(self.value.to_bits()).expect("value");
        w.write_string(&self.label).expect("label");
        w.into_bytes()
    }

    /// Decodes an XCDR2 body (little-endian, as delivered by the examples).
    #[must_use]
    pub fn decode(body: &[u8]) -> Self {
        let mut r = BufferReader::new(body, Endianness::Little).xcdr2();
        let id = r.read_u32().expect("id");
        let value = f32::from_bits(r.read_u32().expect("value"));
        let label = r.read_string().expect("label");
        Self { id, value, label }
    }
}

const XRCE_SESSION_NOKEY: u8 = 0x80;
const XRCE_STREAM_BEST_EFFORT: u8 = 0x01;

/// Wraps an XCDR sample body in an XRCE WRITE_DATA message (8-byte header).
#[must_use]
pub fn xrce_write_frame(seq: u16, sample: &[u8]) -> Vec<u8> {
    let n = u16::try_from(sample.len()).expect("sample fits u16");
    let mut out = Vec::with_capacity(8 + sample.len());
    out.push(XRCE_SESSION_NOKEY);
    out.push(XRCE_STREAM_BEST_EFFORT);
    out.extend_from_slice(&seq.to_le_bytes());
    out.push(0x07); // WRITE_DATA
    out.push(0x03);
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(sample);
    out
}

/// Returns the sample body of a WRITE_DATA frame, or `None` if it is not one.
#[must_use]
pub fn xrce_read_frame(frame: &[u8]) -> Option<&[u8]> {
    if frame.len() >= 8 && frame[4] == 0x07 {
        Some(&frame[8..])
    } else {
        None
    }
}

/// In-memory FIFO transport for tests + examples. Cheap to clone (shared queue).
#[derive(Clone, Default)]
pub struct MemTransport {
    q: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl MemTransport {
    /// Creates an empty transport.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueues one frame.
    pub fn deliver(&self, frame: Vec<u8>) {
        self.q.lock().expect("transport lock").push_back(frame);
    }

    /// Dequeues one frame, or `None` if empty.
    #[must_use]
    pub fn receive(&self) -> Option<Vec<u8>> {
        self.q.lock().expect("transport lock").pop_front()
    }
}

/// Sync endpoint: frames an XCDR sample and delivers it; polls one back.
pub struct Client {
    transport: MemTransport,
    seq: u16,
}

impl Client {
    /// Creates a client over `transport`.
    #[must_use]
    pub fn new(transport: MemTransport) -> Self {
        Self { transport, seq: 1 }
    }

    /// Frames and delivers one sample body.
    pub fn write(&mut self, sample: &[u8]) {
        let frame = xrce_write_frame(self.seq, sample);
        self.seq = self.seq.wrapping_add(1);
        self.transport.deliver(frame);
    }

    /// One non-blocking receive: the sample body, or `None`.
    #[must_use]
    pub fn poll(&self) -> Option<Vec<u8>> {
        let frame = self.transport.receive()?;
        xrce_read_frame(&frame).map(<[u8]>::to_vec)
    }
}

/// Async endpoint: a background thread polls the transport and forwards decoded
/// sample bodies over an `mpsc` channel — the idiomatic std concurrency model.
pub struct AsyncReader {
    rx: Receiver<Vec<u8>>,
    running: Arc<AtomicBool>,
}

impl AsyncReader {
    /// Spawns the reader thread over `transport`.
    #[must_use]
    pub fn start(transport: MemTransport) -> Self {
        let (tx, rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&running);
        thread::spawn(move || {
            while flag.load(Ordering::Relaxed) {
                if let Some(frame) = transport.receive() {
                    if let Some(body) = xrce_read_frame(&frame) {
                        if tx.send(body.to_vec()).is_err() {
                            break;
                        }
                    }
                } else {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        });
        Self { rx, running }
    }

    /// Blocks for the next decoded sample body.
    #[must_use]
    pub fn recv(&self) -> Vec<u8> {
        self.rx.recv().expect("reader channel closed")
    }

    /// Signals the reader thread to stop.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
