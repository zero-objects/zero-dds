// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The body of a Request or Reply at its place in the message.
//!
//! CDR aligns the body of a GIOP message from the first octet of the
//! message header (CORBA 3.3 Part 2 §9.4, §9.3.1). In GIOP 1.2 and later
//! the body starts on an 8-octet boundary, so its alignment is the same
//! whether it is counted from the message or from the body. In GIOP 1.0
//! and 1.1 the body follows the header without padding (§9.4.2.1): its
//! offset in the message is any multiple of one, and an 8-octet argument
//! inside it is aligned relative to that offset. A body read or written
//! as a buffer of its own must be read or written at its offset, which
//! [`crate::Request::body_offset`] and [`crate::Reply::body_offset`]
//! give; [`AlignedBody`] reads it there and [`body_writer`] with
//! [`finish_body`] writes it there.

use alloc::vec;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, Writer};

/// The alignment CDR needs at most (`long long`, `double`).
const MAX_ALIGN: usize = 8;

/// A body with its offset in the message, to read it with the
/// alignment the sender wrote it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignedBody {
    bytes: Vec<u8>,
    start: usize,
    offset: usize,
}

impl AlignedBody {
    /// The body `body` found at `offset` octets from the start of its
    /// message.
    #[must_use]
    pub fn new(body: &[u8], offset: usize) -> Self {
        let start = offset % MAX_ALIGN;
        let mut bytes = vec![0; start];
        bytes.extend_from_slice(body);
        Self {
            bytes,
            start,
            offset,
        }
    }

    /// The octets of the body, without what stood before it.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[self.start..]
    }

    /// The offset of the body in its message.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Whether the body has no octets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes().is_empty()
    }

    /// A reader at the first octet of the body whose alignment counts
    /// from the message start.
    #[must_use]
    pub fn reader(&self, endianness: Endianness) -> Reader<'_> {
        let mut r = Reader::new(&self.bytes, endianness);
        // `start` is at most the length of `bytes`: the skip succeeds.
        let _ = r.skip_to(self.start);
        r
    }
}

/// A writer for a body that goes at `offset` octets from the start of
/// its message; [`finish_body`] with the same `offset` returns the body.
#[must_use]
pub fn body_writer(offset: usize, endianness: Endianness) -> Writer {
    let mut w = Writer::new(endianness);
    for _ in 0..offset % MAX_ALIGN {
        w.write_u8(0);
    }
    w
}

/// The body [`body_writer`] for `offset` collected.
#[must_use]
pub fn finish_body(w: Writer, offset: usize) -> Vec<u8> {
    let mut bytes = w.finish();
    bytes.drain(..(offset % MAX_ALIGN).min(bytes.len()));
    bytes
}
