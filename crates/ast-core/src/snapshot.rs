// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Deterministic snapshot primitives. Spec §9.
//!
//! Hand-written rather than delegated to a serialisation framework: byte
//! determinism must be a property of this code, so that a snapshot hash
//! can key a build cache and a diff between two runs reflects model
//! differences rather than a library's iteration order.
//!
//! All multi-byte values are big-endian, so a snapshot written on one
//! host reads identically on another.

use crate::error::AstCoreError;
use crate::id::NodeId;
use alloc::string::String;
use alloc::vec::Vec;

/// File magic identifying a common-AST snapshot.
pub const SNAPSHOT_MAGIC: [u8; 8] = *b"ZDDSAST\0";

/// Snapshot format version. Bumped when the encoding changes shape.
///
/// 2 adds the `XTypesFacet::discriminator_is_key` bit — `@key` on a
/// union discriminator — behind the try-construct quartet. A version 1
/// snapshot has no such bit and every reader of one would take the byte
/// that follows for it, so the bump is what makes an old file a refusal
/// rather than a misread model.
///
/// 3 writes a constant's type, which no version before carried, and a
/// pending or broken reference with its lookup key and cause instead of
/// one "unresolved" byte.
///
/// 4 writes, for a type, a constant and an exception, the interface or
/// value type it is declared in, when it is not declared directly in a
/// module.
pub const SNAPSHOT_VERSION: u16 = 4;

/// Appends primitives to a snapshot buffer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotWriter {
    out: Vec<u8>,
}

impl SnapshotWriter {
    /// Empty writer.
    #[must_use]
    pub const fn new() -> Self {
        Self { out: Vec::new() }
    }

    /// Writer that has already emitted the magic and version header.
    #[must_use]
    pub fn with_header() -> Self {
        let mut writer = Self::new();
        writer.out.extend_from_slice(&SNAPSHOT_MAGIC);
        writer.u16(SNAPSHOT_VERSION);
        writer
    }

    /// Bytes written so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.out.len()
    }

    /// Whether nothing has been written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    /// Write a boolean as one byte.
    pub fn bool(&mut self, value: bool) {
        self.out.push(u8::from(value));
    }

    /// Write one byte.
    pub fn u8(&mut self, value: u8) {
        self.out.push(value);
    }

    /// Write a big-endian `u16`.
    pub fn u16(&mut self, value: u16) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Write a big-endian `u32`.
    pub fn u32(&mut self, value: u32) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Write a big-endian `u64`.
    pub fn u64(&mut self, value: u64) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Write a big-endian `i64`.
    pub fn i64(&mut self, value: i64) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Write a big-endian `i128`.
    pub fn i128(&mut self, value: i128) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }

    /// Write a length-prefixed byte string.
    ///
    /// # Errors
    /// [`AstCoreError::Exhausted`] when the slice is longer than `u32`
    /// can express. Truncating silently would corrupt the snapshot.
    pub fn bytes(&mut self, value: &[u8]) -> Result<(), AstCoreError> {
        let len = u32::try_from(value.len()).map_err(|_| AstCoreError::Exhausted)?;
        self.u32(len);
        self.out.extend_from_slice(value);
        Ok(())
    }

    /// Write a length-prefixed UTF-8 string.
    ///
    /// # Errors
    /// As [`SnapshotWriter::bytes`].
    pub fn str(&mut self, value: &str) -> Result<(), AstCoreError> {
        self.bytes(value.as_bytes())
    }

    /// Write a node identity in its big-endian byte form.
    pub fn node_id(&mut self, value: NodeId) {
        self.out.extend_from_slice(&value.to_bytes());
    }

    /// Take the encoded buffer.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.out
    }
}

/// Reads primitives back out of a snapshot buffer.
///
/// Every reader returns `Result`; a truncated or malformed stream is an
/// error, never a partially decoded value. This type is a fuzz target.
/// Spec §9, §10.
#[derive(Debug, Clone)]
pub struct SnapshotReader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> SnapshotReader<'a> {
    /// Reader positioned at the start of the buffer.
    #[must_use]
    pub const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    /// Reader positioned past a validated magic and version header.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on a wrong magic, an unknown
    /// version, or a buffer too short to hold the header.
    pub fn with_header(input: &'a [u8]) -> Result<Self, AstCoreError> {
        let mut reader = Self::new(input);
        let magic = reader.take(SNAPSHOT_MAGIC.len(), "snapshot magic")?;
        if magic != SNAPSHOT_MAGIC {
            return Err(AstCoreError::MalformedSnapshot {
                offset: 0,
                expected: "snapshot magic ZDDSAST",
            });
        }
        let version = reader.u16()?;
        if version != SNAPSHOT_VERSION {
            return Err(AstCoreError::MalformedSnapshot {
                offset: SNAPSHOT_MAGIC.len(),
                expected: "snapshot version 1",
            });
        }
        Ok(reader)
    }

    /// Bytes not yet consumed.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.offset)
    }

    /// Current read position.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    fn take(&mut self, count: usize, expected: &'static str) -> Result<&'a [u8], AstCoreError> {
        let start = self.offset;
        let end = start
            .checked_add(count)
            .ok_or(AstCoreError::MalformedSnapshot {
                offset: start,
                expected,
            })?;
        let slice = self
            .input
            .get(start..end)
            .ok_or(AstCoreError::MalformedSnapshot {
                offset: start,
                expected,
            })?;
        self.offset = end;
        Ok(slice)
    }

    fn fixed<const N: usize>(&mut self, expected: &'static str) -> Result<[u8; N], AstCoreError> {
        let start = self.offset;
        let slice = self.take(N, expected)?;
        slice
            .try_into()
            .map_err(|_| AstCoreError::MalformedSnapshot {
                offset: start,
                expected,
            })
    }

    /// Read a boolean. Any byte other than 0 or 1 is malformed.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation or an
    /// out-of-range byte.
    pub fn bool(&mut self) -> Result<bool, AstCoreError> {
        let start = self.offset;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(AstCoreError::MalformedSnapshot {
                offset: start,
                expected: "boolean 0 or 1",
            }),
        }
    }

    /// Read one byte.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation.
    pub fn u8(&mut self) -> Result<u8, AstCoreError> {
        Ok(u8::from_be_bytes(self.fixed::<1>("u8")?))
    }

    /// Read a big-endian `u16`.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation.
    pub fn u16(&mut self) -> Result<u16, AstCoreError> {
        Ok(u16::from_be_bytes(self.fixed::<2>("u16")?))
    }

    /// Read a big-endian `u32`.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation.
    pub fn u32(&mut self) -> Result<u32, AstCoreError> {
        Ok(u32::from_be_bytes(self.fixed::<4>("u32")?))
    }

    /// Read a big-endian `u64`.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation.
    pub fn u64(&mut self) -> Result<u64, AstCoreError> {
        Ok(u64::from_be_bytes(self.fixed::<8>("u64")?))
    }

    /// Read a big-endian `i64`.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation.
    pub fn i64(&mut self) -> Result<i64, AstCoreError> {
        Ok(i64::from_be_bytes(self.fixed::<8>("i64")?))
    }

    /// Read a big-endian `i128`.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation.
    pub fn i128(&mut self) -> Result<i128, AstCoreError> {
        Ok(i128::from_be_bytes(self.fixed::<16>("i128")?))
    }

    /// Read a length-prefixed byte string.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] when the prefix exceeds the
    /// buffer.
    pub fn byte_vec(&mut self) -> Result<Vec<u8>, AstCoreError> {
        let len = self.u32()? as usize;
        Ok(self.take(len, "byte string payload")?.to_vec())
    }

    /// Read a length-prefixed UTF-8 string.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation or invalid
    /// UTF-8.
    pub fn string(&mut self) -> Result<String, AstCoreError> {
        let start = self.offset;
        let bytes = self.byte_vec()?;
        String::from_utf8(bytes).map_err(|_| AstCoreError::MalformedSnapshot {
            offset: start,
            expected: "UTF-8 string payload",
        })
    }

    /// Read a node identity.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] on truncation.
    pub fn node_id(&mut self) -> Result<NodeId, AstCoreError> {
        Ok(NodeId::from_bytes(self.fixed::<16>("node id")?))
    }

    /// Assert the buffer is fully consumed.
    ///
    /// # Errors
    /// [`AstCoreError::MalformedSnapshot`] when bytes remain. Trailing
    /// data means the writer and reader disagree about the format, which
    /// must be loud rather than ignored.
    pub fn expect_end(&mut self) -> Result<(), AstCoreError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(AstCoreError::MalformedSnapshot {
                offset: self.offset,
                expected: "end of snapshot",
            })
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests assert on Results directly; a failed expect IS the test failure.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::id::IdSpace;
    use alloc::string::ToString;
    use alloc::vec;

    #[test]
    fn every_primitive_round_trips() {
        let id = NodeId::from_parts(IdSpace::BUILTIN, 42);
        let mut w = SnapshotWriter::new();
        w.bool(true);
        w.u8(0x7f);
        w.u16(0xbeef);
        w.u32(0xdead_beef);
        w.u64(u64::MAX);
        w.i64(i64::MIN);
        w.i128(-170_141_183_460_469_231_731_687_303_715_884_105_728);
        w.str("Robot::Pose").expect("str");
        w.bytes(&[1, 2, 3]).expect("bytes");
        w.node_id(id);
        let encoded = w.finish();

        let mut r = SnapshotReader::new(&encoded);
        assert_eq!(r.bool(), Ok(true));
        assert_eq!(r.u8(), Ok(0x7f));
        assert_eq!(r.u16(), Ok(0xbeef));
        assert_eq!(r.u32(), Ok(0xdead_beef));
        assert_eq!(r.u64(), Ok(u64::MAX));
        assert_eq!(r.i64(), Ok(i64::MIN));
        assert_eq!(r.i128(), Ok(i128::MIN));
        assert_eq!(r.string(), Ok("Robot::Pose".to_string()));
        assert_eq!(r.byte_vec(), Ok(vec![1, 2, 3]));
        assert_eq!(r.node_id(), Ok(id));
        assert_eq!(r.expect_end(), Ok(()));
    }

    #[test]
    fn encoding_the_same_values_twice_yields_identical_bytes() {
        let build = || {
            let mut w = SnapshotWriter::new();
            w.u32(7);
            w.str("Pose").expect("str");
            w.node_id(NodeId::from_parts(IdSpace::MODEL, 3));
            w.finish()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn a_truncated_stream_is_an_error_not_a_partial_value() {
        let mut w = SnapshotWriter::new();
        w.u64(1);
        let mut encoded = w.finish();
        let _ = encoded.pop();

        let mut r = SnapshotReader::new(&encoded);
        assert!(matches!(
            r.u64(),
            Err(AstCoreError::MalformedSnapshot { .. })
        ));
    }

    #[test]
    fn a_length_prefix_beyond_the_buffer_is_rejected() {
        // A string claiming 0xffff_ffff bytes in a four-byte buffer.
        let encoded = vec![0xff, 0xff, 0xff, 0xff];
        let mut r = SnapshotReader::new(&encoded);
        assert!(matches!(
            r.string(),
            Err(AstCoreError::MalformedSnapshot { .. })
        ));
    }

    #[test]
    fn trailing_bytes_are_rejected_by_expect_end() {
        let mut w = SnapshotWriter::new();
        w.u8(1);
        let mut encoded = w.finish();
        encoded.push(0);

        let mut r = SnapshotReader::new(&encoded);
        assert_eq!(r.u8(), Ok(1));
        assert!(r.expect_end().is_err());
    }

    #[test]
    fn non_utf8_string_payload_is_rejected() {
        let mut w = SnapshotWriter::new();
        w.bytes(&[0xff, 0xfe]).expect("bytes");
        let encoded = w.finish();
        let mut r = SnapshotReader::new(&encoded);
        assert!(r.string().is_err());
    }

    #[test]
    fn a_boolean_byte_outside_zero_and_one_is_rejected() {
        let encoded = vec![2];
        let mut r = SnapshotReader::new(&encoded);
        assert!(matches!(
            r.bool(),
            Err(AstCoreError::MalformedSnapshot { .. })
        ));
    }

    #[test]
    fn the_header_round_trips_and_a_wrong_magic_is_rejected() {
        let w = SnapshotWriter::with_header();
        let encoded = w.finish();
        let mut r = SnapshotReader::with_header(&encoded).expect("header");
        assert_eq!(r.expect_end(), Ok(()));

        let mut wrong = encoded.clone();
        wrong[0] = b'X';
        assert!(SnapshotReader::with_header(&wrong).is_err());
    }

    #[test]
    fn an_unknown_version_is_rejected() {
        let mut w = SnapshotWriter::new();
        w.u8(b'Z');
        w.u8(b'D');
        w.u8(b'D');
        w.u8(b'S');
        w.u8(b'A');
        w.u8(b'S');
        w.u8(b'T');
        w.u8(0);
        w.u16(999);
        let encoded = w.finish();
        assert!(SnapshotReader::with_header(&encoded).is_err());
    }

    #[test]
    fn a_buffer_too_short_for_the_header_is_rejected() {
        assert!(SnapshotReader::with_header(&[0u8; 3]).is_err());
    }
}
