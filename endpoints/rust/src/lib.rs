// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-endpoint-rust`. Safety classification: **STANDARD**.
//!
//! Native Rust endpoint SDK (ADR 0013): a thin XRCE + XCDR2 client over the
//! reference wire runtime. Sync (poll) and async (a std thread + `mpsc`
//! channel — no async runtime dependency). Byte-identical to the Rust core by
//! construction: the codec is `zerodds-wire`'s own `Writer`/`Reader`.

// The endpoint demo uses `expect` on infallible in-memory paths (poisoned locks,
// well-formed frames); the workspace denies `expect_used` for library code.
#![allow(clippy::expect_used)]

pub mod reliable;
pub mod xrce_client;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zerodds_wire::{Endianness, Reader, Writer};

/// XCDR2's `MAXALIGN`. DDS-XTypes 1.3 §7.4.1.1.1: an eight-byte
/// primitive aligns to four, not to eight.
const XCDR2_MAX_ALIGN: usize = 4;

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
        let mut w = Writer::with_max_align(endian, XCDR2_MAX_ALIGN);
        w.write_u32(self.id);
        w.write_f32(self.value);
        w.write_string(&self.label);
        w.finish()
    }

    /// Decodes an XCDR2 body (little-endian, as delivered by the examples).
    #[must_use]
    pub fn decode(body: &[u8]) -> Self {
        let mut r = Reader::with_max_align(body, Endianness::Little, XCDR2_MAX_ALIGN);
        let id = r.read_u32().expect("id");
        let value = r.read_f32().expect("value");
        let label = r.read_string().expect("label");
        Self { id, value, label }
    }
}

const XRCE_SESSION_NOKEY: u8 = 0x80;
const XRCE_STREAM_BEST_EFFORT: u8 = 0x01;
/// Data-submessage flags: E flag (little-endian body) + `DataFormat`
/// `FORMAT_DATA` (`0b000`) in bits 1..3 — DDS-XRCE 1.0 §8.3.5.8.
const XRCE_WRITE_FLAGS: u8 = 0x01;
/// Wire size of the `BaseObjectRequest` that precedes the sample (§7.7.8).
pub const XRCE_BASE_SIZE: usize = 4;
/// `OBJK_DATAWRITER` — the `ObjectKind` of a DataWriter (§7.2.2).
pub const OBJK_DATAWRITER: u16 = 0x05;

/// Why an XRCE frame could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The body (`BaseObjectRequest` + sample) is larger than the 16-bit
    /// `submessage_length` field (DDS-XRCE 1.0 §8.3.4) can describe.
    BodyTooLarge,
}

/// Builds an XRCE `ObjectId` (§7.2.1) from a 12-bit raw id and a 4-bit kind.
#[must_use]
pub const fn xrce_object_id(raw_id: u16, kind: u16) -> u16 {
    ((raw_id & 0x0FFF) << 4) | (kind & 0x0F)
}

/// Wraps an XCDR sample body in an XRCE WRITE_DATA message: 8-byte header, then
/// the `BaseObjectRequest` (§7.7.8 — `request_id` then `object_id`, each two
/// octets most-significant-first), then the sample.
///
/// `writer_id` is the target DataWriter's `ObjectId`; without it no XRCE agent
/// can route the sample. The `request_id` is the message's own `seq`, which
/// §7.7.8 only requires to be unique per stream.
///
/// # Errors
/// [`FrameError::BodyTooLarge`] when `XRCE_BASE_SIZE + sample.len()` exceeds
/// `0xFFFF`: refuse rather than truncate the length while copying the whole
/// payload.
pub fn xrce_write_frame(seq: u16, writer_id: u16, sample: &[u8]) -> Result<Vec<u8>, FrameError> {
    let n = u16::try_from(XRCE_BASE_SIZE + sample.len()).map_err(|_| FrameError::BodyTooLarge)?;
    let mut out = Vec::with_capacity(8 + usize::from(n));
    out.push(XRCE_SESSION_NOKEY);
    out.push(XRCE_STREAM_BEST_EFFORT);
    out.extend_from_slice(&seq.to_le_bytes());
    out.push(0x07); // WRITE_DATA
    out.push(XRCE_WRITE_FLAGS);
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&seq.to_be_bytes()); // request_id
    out.extend_from_slice(&writer_id.to_be_bytes()); // object_id
    out.extend_from_slice(sample);
    Ok(out)
}

/// Returns the sample body of a data frame, or `None` if it is not one.
///
/// Frame layout: `[session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
/// len_hi]`, then the 4-octet `BaseObjectRequest` (§7.7.8), then the sample.
/// The 16-bit little-endian `submessage_length` (bytes 6..8) bounds the body
/// exactly — the returned slice is `frame[12..8+len]`, never `frame[12..]`.
/// Trailing bytes (padding or an appended submessage) are therefore never
/// folded into the sample.
///
/// Accepts both WRITE_DATA (`0x07`, endpoint->hub / loopback) and DATA (`0x09`,
/// hub->endpoint) — a hub pushes samples to a client with DATA. See DDS-XRCE
/// spec §8.3.5.
///
/// Returns `None` (never panics) when the frame is shorter than the header,
/// carries some other submessage id, declares a length that runs past the
/// datagram (truncation / wrong length), or declares a body too short to hold
/// the `BaseObjectRequest`.
#[must_use]
pub fn xrce_read_frame(frame: &[u8]) -> Option<&[u8]> {
    let (_, sample) = xrce_read_frame_with_base(frame)?;
    Some(sample)
}

/// Like [`xrce_read_frame`] but also returns the frame's `BaseObjectRequest` as
/// `(request_id, object_id)` — the identity an agent routes on (§7.7.8).
#[must_use]
pub fn xrce_read_frame_with_base(frame: &[u8]) -> Option<((u16, u16), &[u8])> {
    if frame.len() < 8 + XRCE_BASE_SIZE || (frame[4] != 0x07 && frame[4] != 0x09) {
        return None;
    }
    let sm_len = usize::from(u16::from_le_bytes([frame[6], frame[7]]));
    if sm_len < XRCE_BASE_SIZE || 8 + sm_len > frame.len() {
        return None;
    }
    let request_id = u16::from_be_bytes([frame[8], frame[9]]);
    let object_id = u16::from_be_bytes([frame[10], frame[11]]);
    Some((
        (request_id, object_id),
        &frame[8 + XRCE_BASE_SIZE..8 + sm_len],
    ))
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
    writer_id: u16,
}

impl Client {
    /// Creates a client over `transport` that writes to the DataWriter
    /// `writer_id` (an XRCE `ObjectId`, §7.2.1).
    #[must_use]
    pub fn new(transport: MemTransport, writer_id: u16) -> Self {
        Self {
            transport,
            seq: 1,
            writer_id,
        }
    }

    /// Frames and delivers one sample body.
    ///
    /// # Errors
    /// [`FrameError::BodyTooLarge`] when the sample does not fit the 16-bit
    /// `submessage_length` — nothing is delivered and the sequence is not
    /// advanced.
    pub fn write(&mut self, sample: &[u8]) -> Result<(), FrameError> {
        let frame = xrce_write_frame(self.seq, self.writer_id, sample)?;
        self.seq = self.seq.wrapping_add(1);
        self.transport.deliver(frame);
        Ok(())
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

#[cfg(test)]
mod frame_tests {
    use super::{
        Client, FrameError, MemTransport, OBJK_DATAWRITER, XRCE_BASE_SIZE, xrce_object_id,
        xrce_read_frame, xrce_write_frame,
    };

    /// The DataWriter every frame in these tests addresses (§7.2.1).
    const WRITER: u16 = xrce_object_id(0x001, OBJK_DATAWRITER);

    #[test]
    fn read_frame_bounds_body_to_declared_length_ignoring_appended_submessage() {
        // A well-formed WRITE_DATA carrying a 3-byte sample, with a second
        // submessage appended after it. The reader must return only the first
        // sample, never the trailing bytes.
        let first = xrce_write_frame(1, WRITER, &[0xAA, 0xBB, 0xCC]).expect("frame");
        let second = xrce_write_frame(2, WRITER, &[0xDD, 0xEE]).expect("frame");
        let mut frame = first.clone();
        frame.extend_from_slice(&second);
        assert_eq!(xrce_read_frame(&frame), Some(&[0xAA, 0xBB, 0xCC][..]));
    }

    #[test]
    fn read_frame_rejects_length_running_past_datagram() {
        // Declared submessage_length = 0xFFFF but only 3 payload bytes present.
        let mut frame = xrce_write_frame(1, WRITER, &[0xAA, 0xBB, 0xCC]).expect("frame");
        frame[6] = 0xFF;
        frame[7] = 0xFF;
        assert_eq!(xrce_read_frame(&frame), None);
    }

    #[test]
    fn read_frame_rejects_truncated_header() {
        assert_eq!(xrce_read_frame(&[0x80, 0x01, 0x00, 0x00, 0x07]), None);
    }

    #[test]
    fn read_frame_rejects_wrong_submessage_id() {
        let mut frame = xrce_write_frame(1, WRITER, &[0xAA]).expect("frame");
        frame[4] = 0x08; // neither WRITE_DATA (0x07) nor DATA (0x09)
        assert_eq!(xrce_read_frame(&frame), None);
    }

    #[test]
    fn write_frame_refuses_body_above_16_bit_length() {
        // The bound is on the body (BaseObjectRequest + sample): exactly 0xFFFF
        // fits, one octet more is refused rather than truncating the field.
        let max_sample = 0xFFFF - XRCE_BASE_SIZE;
        let frame = xrce_write_frame(1, WRITER, &vec![0u8; max_sample]).expect("frame");
        assert_eq!(frame.len(), 8 + 0xFFFF);
        assert_eq!(u16::from_le_bytes([frame[6], frame[7]]), 0xFFFF);

        assert_eq!(
            xrce_write_frame(1, WRITER, &vec![0u8; max_sample + 1]),
            Err(FrameError::BodyTooLarge)
        );
    }

    #[test]
    fn client_write_refuses_and_delivers_nothing() {
        let transport = MemTransport::new();
        let mut client = Client::new(transport.clone(), WRITER);
        let too_big = vec![0u8; 0xFFFF - XRCE_BASE_SIZE + 1];
        assert_eq!(client.write(&too_big), Err(FrameError::BodyTooLarge));
        assert!(
            transport.receive().is_none(),
            "no refused frame reached the transport"
        );
        // The sequence was not advanced: the next (valid) frame is seq 1.
        client.write(&[0xAA]).expect("frame");
        let frame = transport.receive().expect("delivered");
        assert_eq!(&frame[2..4], &1u16.to_le_bytes());
    }
}
