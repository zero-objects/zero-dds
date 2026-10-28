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
    buffer: Arc<[u8]>,
    range: Range<usize>,
}

impl Payload {
    /// All of `buffer`.
    #[must_use]
    pub fn whole(buffer: Arc<[u8]>) -> Self {
        let range = 0..buffer.len();
        Self { buffer, range }
    }

    /// `range` of `buffer`, sharing it.
    ///
    /// # Panics
    /// When `range` is not within `buffer` — a decoder that computed it
    /// from a length it did not check.
    #[must_use]
    pub fn slice(buffer: &Arc<[u8]>, range: Range<usize>) -> Self {
        assert!(
            range.start <= range.end && range.end <= buffer.len(),
            "payload range {range:?} outside a buffer of {}",
            buffer.len()
        );
        Self {
            buffer: Arc::clone(buffer),
            range,
        }
    }

    /// The buffer the octets are a range of.
    #[must_use]
    pub fn buffer(&self) -> &Arc<[u8]> {
        &self.buffer
    }

    /// Where in [`Self::buffer`] the octets are.
    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// The octets as an `Arc<[u8]>` of their own: the buffer itself when
    /// the payload is all of it, a copy otherwise.
    #[must_use]
    pub fn to_arc(&self) -> Arc<[u8]> {
        if self.range.start == 0 && self.range.end == self.buffer.len() {
            Arc::clone(&self.buffer)
        } else {
            Arc::from(&self.buffer[self.range.clone()])
        }
    }
}

impl Deref for Payload {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buffer[self.range.clone()]
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

impl From<Vec<u8>> for Payload {
    fn from(octets: Vec<u8>) -> Self {
        Self::whole(Arc::from(octets))
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
        assert!(Arc::ptr_eq(middle.buffer(), &buffer));
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
}
