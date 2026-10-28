// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The KeyHash of an instance. DDSI-RTPS 2.5 §9.6.4.8, XTypes 1.3 §7.6.8.
//!
//! `PID_KEY_HASH` (`0x0070`, sixteen octets) in the inline QoS of a
//! `DATA` or `DATA_FRAG` addresses the **instance** a sample belongs to,
//! so a reader or a persistence service can correlate samples of one
//! instance without decoding the payload.
//!
//! # The computation, §7.6.8 steps 1 to 5
//!
//! 1. Build a `FooKeyHolder`: the `@key` members of `Foo` and nothing
//!    else, in ascending `member_id` order.
//! 2. Fill it with the instance's key values.
//! 3. Expand a nested aggregate member into its own key members.
//! 4. Serialise as PLAIN_CDR2, **big-endian**, `MAXALIGN` four, with no
//!    encapsulation header and no member headers.
//! 5. If the holder's **maximum** serialised size is at most sixteen
//!    octets, the KeyHash is those octets zero-padded to sixteen;
//!    otherwise it is their MD5.
//!
//! Step 5 asks about the *type* and not about the instance: a topic
//! keyed on `string<8>` has a maximum of twelve octets — four for the
//! length and eight for the content — and zero-pads even when a given
//! instance is shorter. A topic keyed on an unbounded `string` has no
//! maximum and always hashes. That is why [`compute_key_hash`] takes the
//! maximum as an argument rather than reading the length of what it was
//! given.
//!
//! # Where this used to live
//!
//! `crates/cdr`'s `key_hash.rs`, which the rebuild replaced with
//! `crates/wire` — a crate that executes a layout plan and does not know
//! what a DDS instance is. The plan-driven half of the same clause is
//! `zerodds_ast_layout::keyhash`, which decides
//! [`zerodds_ast_layout::KeyHashForm`] from a planned extent and serves
//! `crates/wire-codec`. This module serves the other half: a hand-written
//! or generated [`crate::dds_type::DdsType`], which states its own
//! maximum through `KEY_HOLDER_MAX_SIZE` and writes its own holder
//! through `encode_key_holder_be`. Both answer §7.6.8 step 5 the same
//! way; neither can be expressed in the other's terms, because one has a
//! plan and the other has an `impl`.
//!
//! The alignment and the byte layout are **not** re-implemented here:
//! [`PlainCdr2BeKeyHolder`] is a big-endian [`zerodds_wire::Writer`]
//! capped at `MAXALIGN` four, so a holder is written by the same code
//! that writes every other byte this tree puts on a socket.

extern crate alloc;

use alloc::vec::Vec;
use zerodds_wire::{Endianness, Writer};

/// The wire length of a KeyHash. §9.6.4.8: sixteen octets, always.
pub const KEY_HASH_LEN: usize = 16;

/// The KeyHash of an instance, from its serialised key holder.
///
/// `plain_cdr2_be_bytes` is the holder as [`PlainCdr2BeKeyHolder`]
/// writes it. `key_holder_max_size` is the holder's **statically known**
/// maximum length, which is what decides between the two branches of
/// §7.6.8 step 5 — see the module docs for why it is not the length of
/// the argument.
#[must_use]
pub fn compute_key_hash(
    plain_cdr2_be_bytes: &[u8],
    key_holder_max_size: usize,
) -> [u8; KEY_HASH_LEN] {
    if key_holder_max_size <= KEY_HASH_LEN {
        // Step 5.1 — the octets themselves, zero-padded on the right.
        let mut out = [0u8; KEY_HASH_LEN];
        let n = core::cmp::min(plain_cdr2_be_bytes.len(), KEY_HASH_LEN);
        out[..n].copy_from_slice(&plain_cdr2_be_bytes[..n]);
        out
    } else {
        // Step 5.2 — MD5 of the holder.
        zerodds_wire::md5(plain_cdr2_be_bytes)
    }
}

/// The KeyHash from an **unordered** list of `(member_id, value_bytes)`.
///
/// §7.6.8.3.1.b fixes the order the members go into the holder in:
/// ascending `member_id`. Two implementations that enumerate their
/// `@key` members in different source order must still compute the same
/// KeyHash, so the sort is done here rather than left to each caller's
/// discipline. `key_holder_max_size` is as in [`compute_key_hash`].
#[must_use]
pub fn keyhash_cdr2_be(
    members: &[(u32, Vec<u8>)],
    key_holder_max_size: usize,
) -> [u8; KEY_HASH_LEN] {
    let mut sorted: Vec<&(u32, Vec<u8>)> = members.iter().collect();
    sorted.sort_by_key(|(id, _)| *id);
    let mut concat: Vec<u8> = Vec::new();
    for (_, bytes) in &sorted {
        concat.extend_from_slice(bytes);
    }
    compute_key_hash(&concat, key_holder_max_size)
}

/// A builder for the PLAIN_CDR2 big-endian key-holder stream.
///
/// A [`zerodds_wire::Writer`] in big-endian with `MAXALIGN` four — the
/// two things §7.6.8 step 4 fixes and the two a caller most easily gets
/// wrong. Under XCDR2 an eight-octet primitive is aligned to **four**
/// (XTypes 1.3 §7.4.1), so a holder written by an XCDR1 writer diverges
/// from every peer's at the first `long long` behind an odd member.
///
/// Members are written in ascending `member_id` order; the caller states
/// that order by the order it calls the methods in, or hands the pairs
/// to [`keyhash_cdr2_be`] and lets it sort.
#[derive(Debug)]
pub struct PlainCdr2BeKeyHolder {
    writer: Writer,
}

impl Default for PlainCdr2BeKeyHolder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlainCdr2BeKeyHolder {
    /// An empty holder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            writer: Writer::with_max_align(Endianness::Big, 4),
        }
    }

    /// An empty holder that writes into `buffer`, keeping its capacity:
    /// what [`Self::into_bytes`] gave back can be handed in again, so an
    /// endpoint that keys every sample does not allocate a holder per
    /// sample.
    #[must_use]
    pub fn reusing(mut buffer: Vec<u8>) -> Self {
        buffer.clear();
        Self {
            writer: Writer::appending(Endianness::Big, 4, buffer),
        }
    }

    /// How many octets have been written.
    #[must_use]
    pub fn len(&self) -> usize {
        self.writer.position()
    }

    /// Whether nothing has been written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.writer.position() == 0
    }

    /// The finished octets.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.writer.finish()
    }

    /// The octets so far, without consuming the holder.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.writer.as_bytes()
    }

    /// `boolean`.
    pub fn write_bool(&mut self, v: bool) {
        self.writer.write_bool(v);
    }

    /// `octet` / `uint8`.
    pub fn write_u8(&mut self, v: u8) {
        self.writer.write_u8(v);
    }

    /// `int8`.
    pub fn write_i8(&mut self, v: i8) {
        self.writer.write_i8(v);
    }

    /// `uint16`, aligned to two.
    pub fn write_u16(&mut self, v: u16) {
        self.writer.write_u16(v);
    }

    /// `int16`, aligned to two.
    pub fn write_i16(&mut self, v: i16) {
        self.writer.write_i16(v);
    }

    /// `uint32`, aligned to four.
    pub fn write_u32(&mut self, v: u32) {
        self.writer.write_u32(v);
    }

    /// `int32`, aligned to four.
    pub fn write_i32(&mut self, v: i32) {
        self.writer.write_i32(v);
    }

    /// `uint64`, aligned to **four** and not eight — PLAIN_CDR2.
    pub fn write_u64(&mut self, v: u64) {
        self.writer.write_u64(v);
    }

    /// `int64`, aligned to four.
    pub fn write_i64(&mut self, v: i64) {
        self.writer.write_i64(v);
    }

    /// `float`, aligned to four.
    pub fn write_f32(&mut self, v: f32) {
        self.writer.write_f32(v);
    }

    /// `double`, aligned to four.
    pub fn write_f64(&mut self, v: f64) {
        self.writer.write_f64(v);
    }

    /// A `string`: a four-octet length counting the terminator, the
    /// octets, then the NUL.
    ///
    /// The caller keeps the value inside the bound its
    /// `KEY_HOLDER_MAX_SIZE` was computed from; a longer one makes the
    /// stated maximum a lie and moves every later member.
    pub fn write_string(&mut self, s: &str) {
        self.writer.write_string(s);
    }

    /// Octets as they are, with no alignment and no length in front —
    /// an `octet[N]` member, or a nested holder already written.
    pub fn write_bytes(&mut self, b: &[u8]) {
        self.writer.write_bytes(b);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::vec;

    // -- step 5.1, the zero-padded branch --------------------------------

    #[test]
    fn a_holder_of_four_octets_is_padded_to_sixteen() {
        let bytes = 0x1234_5678u32.to_be_bytes();
        let h = compute_key_hash(&bytes, 4);
        assert_eq!(h[0..4], [0x12, 0x34, 0x56, 0x78]);
        assert_eq!(h[4..16], [0u8; 12]);
    }

    #[test]
    fn a_holder_of_exactly_sixteen_is_the_hash() {
        let mut bytes = [0u8; 16];
        for (i, b) in bytes.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            {
                *b = i as u8;
            }
        }
        assert_eq!(compute_key_hash(&bytes, 16), bytes);
    }

    #[test]
    fn a_short_instance_of_a_bounded_type_still_pads() {
        // The maximum decides, not this instance's five octets.
        let h = compute_key_hash(&[1, 2, 3, 4, 5], 16);
        assert_eq!(&h[..5], &[1, 2, 3, 4, 5]);
        assert_eq!(&h[5..], &[0u8; 11]);
    }

    // -- step 5.2, the MD5 branch ----------------------------------------

    #[test]
    fn a_holder_over_sixteen_is_hashed() {
        // MD5 of twenty zero octets.
        assert_eq!(
            compute_key_hash(&[0u8; 20], 20),
            [
                0x44, 0x10, 0x18, 0x52, 0x52, 0x08, 0x45, 0x77, 0x05, 0xbf, 0x09, 0xa8, 0xee, 0x3c,
                0x10, 0x93
            ]
        );
    }

    #[test]
    fn md5_of_the_empty_holder() {
        assert_eq!(
            compute_key_hash(&[], 17),
            [
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                0x42, 0x7e
            ]
        );
    }

    #[test]
    fn md5_of_abc() {
        assert_eq!(
            compute_key_hash(b"abc", 17),
            [
                0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
                0x7f, 0x72
            ]
        );
    }

    // -- the holder ------------------------------------------------------

    #[test]
    fn a_u32_goes_out_big_endian() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u32(0x1234_5678);
        assert_eq!(h.into_bytes(), vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn a_u32_behind_an_octet_is_aligned_to_four() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(0xAA);
        h.write_u32(0x1234_5678);
        assert_eq!(h.into_bytes(), vec![0xAA, 0, 0, 0, 0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn a_u64_is_aligned_to_four_and_not_to_eight() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(1);
        h.write_u64(0x1122_3344_5566_7788);
        assert_eq!(h.len(), 1 + 3 + 8);
        assert_eq!(
            h.into_bytes(),
            vec![1, 0, 0, 0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
        );
    }

    #[test]
    fn a_f64_is_aligned_to_four_as_well() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u8(1);
        h.write_f64(1.0);
        assert_eq!(h.len(), 1 + 3 + 8);
    }

    #[test]
    fn a_string_carries_its_terminator_in_its_length() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_string("hi");
        assert_eq!(h.into_bytes(), vec![0x00, 0x00, 0x00, 0x03, b'h', b'i', 0]);
    }

    #[test]
    fn two_members_in_declaration_order() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u32(42);
        h.write_string("foo");
        let bytes = h.into_bytes();
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..4], &[0, 0, 0, 42]);
        assert_eq!(&bytes[4..8], &[0, 0, 0, 4]);
        assert_eq!(&bytes[8..12], &[b'f', b'o', b'o', 0]);
    }

    #[test]
    fn holder_to_hash_over_the_padded_branch() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_u32(0x1122_3344);
        let key = compute_key_hash(&h.into_bytes(), 4);
        assert_eq!(&key[..4], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(&key[4..], &[0u8; 12]);
    }

    #[test]
    fn holder_to_hash_over_the_md5_branch() {
        let mut h = PlainCdr2BeKeyHolder::new();
        h.write_string("hello-world-this-is-a-long-key-name-exceeding-16");
        let key = compute_key_hash(&h.into_bytes(), usize::MAX);
        assert_ne!(key, [0u8; 16]);
    }

    // -- §7.6.8.3.1.b, the member order ----------------------------------

    #[test]
    fn the_member_order_does_not_reach_the_hash() {
        let m_3 = (3u32, vec![0xAAu8, 0xBB, 0xCC, 0xDD]);
        let m_5 = (5u32, vec![0x11u8, 0x22, 0x33, 0x44]);
        let m_7 = (7u32, vec![0xDEu8, 0xAD, 0xBE, 0xEF]);

        let a = keyhash_cdr2_be(&[m_3.clone(), m_5.clone(), m_7.clone()], 12);
        let b = keyhash_cdr2_be(&[m_7.clone(), m_3.clone(), m_5.clone()], 12);
        let c = keyhash_cdr2_be(&[m_5, m_7, m_3], 12);
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(&a[..4], &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    /// The holder writes what §7.6.8 step 4 asks for and not what the
    /// sample's own encoding would: big-endian, `MAXALIGN` four. An
    /// XCDR1 writer over the same members produces a different stream
    /// from the first eight-octet primitive on, and therefore a
    /// different KeyHash from every peer's.
    #[test]
    fn an_xcdr1_writer_over_the_same_members_disagrees() {
        let mut holder = PlainCdr2BeKeyHolder::new();
        holder.write_u8(1);
        holder.write_u64(0x1122_3344_5566_7788);

        let mut xcdr1 = Writer::new(Endianness::Big); // MAXALIGN eight
        xcdr1.write_u8(1);
        xcdr1.write_u64(0x1122_3344_5566_7788);

        assert_eq!(holder.len(), 12);
        assert_eq!(xcdr1.position(), 16);
        assert_ne!(holder.into_bytes(), xcdr1.finish());
    }
}
