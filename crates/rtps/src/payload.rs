// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! [`Payload`] — the serializedPayload of a DATA as a range of a shared
//! buffer.
//!
//! On the receive path the buffer is the received datagram, and every
//! DATA of it is a range of the one allocation: a datagram of thirteen
//! 1 kB samples used to be copied into thirteen allocations of their
//! own, each released later on the thread that took the sample. On the
//! write path the buffer is the writer's history payload, whole.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::{Deref, Range};

/// The octets of a serializedPayload: `range` of `buffer`.
///
/// Cloning shares the buffer. Equality and `Debug` look at the octets
/// only, not at the buffer they are a range of.
#[derive(Clone)]
pub struct Payload {
    buffer: Buffer,
    range: Range<usize>,
}

/// What a [`Payload`] is a range of.
#[derive(Clone)]
enum Buffer {
    /// Octets copied once into an allocation of their own.
    Slice(Arc<[u8]>),
    /// A vector handed over without a copy: a sample serialized where it
    /// goes, or a receive buffer that several datagrams are ranges of.
    /// `Arc<[u8]>` cannot take over a vector's allocation; building one
    /// from it copies the octets.
    Vec(Arc<Vec<u8>>),
}

impl Buffer {
    fn octets(&self) -> &[u8] {
        match self {
            Self::Slice(octets) => octets,
            Self::Vec(octets) => octets,
        }
    }

    fn ptr_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Slice(a), Self::Slice(b)) => Arc::ptr_eq(a, b),
            (Self::Vec(a), Self::Vec(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Payload {
    /// All of `buffer`.
    #[must_use]
    pub fn whole(buffer: Arc<[u8]>) -> Self {
        let range = 0..buffer.len();
        Self {
            buffer: Buffer::Slice(buffer),
            range,
        }
    }

    /// `range` of `buffer`, sharing it.
    ///
    /// # Panics
    /// When `range` is not within `buffer` — a decoder that computed it
    /// from a length it did not check.
    #[must_use]
    pub fn slice(buffer: &Arc<[u8]>, range: Range<usize>) -> Self {
        Self::whole(Arc::clone(buffer)).sub(range)
    }

    /// `range` of the shared vector `buffer`, without a copy.
    ///
    /// # Panics
    /// When `range` is not within `buffer`.
    #[must_use]
    pub fn of_shared_vec(buffer: Arc<Vec<u8>>, range: Range<usize>) -> Self {
        let len = buffer.len();
        Self {
            buffer: Buffer::Vec(buffer),
            range: 0..len,
        }
        .sub(range)
    }

    /// `range` of these octets, sharing the buffer; `range` counts from
    /// the first octet of this payload.
    ///
    /// # Panics
    /// When `range` is not within this payload — a decoder that computed
    /// it from a length it did not check.
    #[must_use]
    pub fn sub(&self, range: Range<usize>) -> Self {
        assert!(
            range.start <= range.end && range.end <= self.len(),
            "payload range {range:?} outside a buffer of {}",
            self.len()
        );
        Self {
            buffer: self.buffer.clone(),
            range: self.range.start + range.start..self.range.start + range.end,
        }
    }

    /// Whether `self` and `other` are ranges of one buffer.
    #[must_use]
    pub fn shares_buffer_with(&self, other: &Self) -> bool {
        self.buffer.ptr_eq(&other.buffer)
    }

    /// Whether these octets are a range of `buffer`.
    #[must_use]
    pub fn is_range_of(&self, buffer: &Arc<[u8]>) -> bool {
        matches!(&self.buffer, Buffer::Slice(own) if Arc::ptr_eq(own, buffer))
    }

    /// The octets as an `Arc<[u8]>` of their own: the buffer itself when
    /// the payload is all of an `Arc<[u8]>`, a copy otherwise.
    #[must_use]
    pub fn to_arc(&self) -> Arc<[u8]> {
        match &self.buffer {
            Buffer::Slice(buffer) if self.range.start == 0 && self.range.end == buffer.len() => {
                Arc::clone(buffer)
            }
            _ => Arc::from(&**self),
        }
    }
}

impl Deref for Payload {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buffer.octets()[self.range.clone()]
    }
}

impl AsRef<[u8]> for Payload {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for Payload {}

impl core::fmt::Debug for Payload {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl From<Arc<[u8]>> for Payload {
    fn from(buffer: Arc<[u8]>) -> Self {
        Self::whole(buffer)
    }
}

impl From<&[u8]> for Payload {
    fn from(octets: &[u8]) -> Self {
        Self::whole(Arc::from(octets))
    }
}

/// Takes the vector over, without copying its octets.
impl From<Vec<u8>> for Payload {
    fn from(octets: Vec<u8>) -> Self {
        let len = octets.len();
        Self::of_shared_vec(Arc::new(octets), 0..len)
    }
}

impl From<alloc::boxed::Box<[u8]>> for Payload {
    fn from(octets: alloc::boxed::Box<[u8]>) -> Self {
        Self::whole(Arc::from(octets))
    }
}

impl<const N: usize> From<[u8; N]> for Payload {
    fn from(octets: [u8; N]) -> Self {
        Self::whole(Arc::from(octets.as_slice()))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_slice_shares_the_buffer_and_compares_by_its_octets() {
        let buffer: Arc<[u8]> = Arc::from([1u8, 2, 3, 4, 5].as_slice());
        let middle = Payload::slice(&buffer, 1..4);
        assert_eq!(&*middle, &[2, 3, 4]);
        assert!(middle.is_range_of(&buffer));
        assert_eq!(middle, Payload::from([2u8, 3, 4]));
        assert_eq!(&*middle.to_arc(), &[2, 3, 4]);
        assert!(!Arc::ptr_eq(&middle.to_arc(), &buffer));
        let whole = Payload::whole(Arc::clone(&buffer));
        assert!(Arc::ptr_eq(&whole.to_arc(), &buffer));
    }

    #[test]
    #[should_panic(expected = "outside a buffer")]
    fn a_range_past_the_buffer_is_refused() {
        let buffer: Arc<[u8]> = Arc::from([1u8, 2].as_slice());
        let _ = Payload::slice(&buffer, 1..3);
    }

    /// A vector is taken over, not copied, and a range of it shares it.
    #[test]
    fn a_vector_is_taken_over_and_its_ranges_share_it() {
        let octets = alloc::vec![1u8, 2, 3, 4, 5];
        let at = octets.as_ptr();
        let whole = Payload::from(octets);
        assert_eq!(whole.as_ptr(), at);
        let middle = whole.sub(1..4);
        assert_eq!(&*middle, &[2, 3, 4]);
        assert!(middle.shares_buffer_with(&whole));
        assert_eq!(&*middle.sub(1..2), &[3]);
        assert_eq!(&*middle.to_arc(), &[2, 3, 4]);
    }
}
