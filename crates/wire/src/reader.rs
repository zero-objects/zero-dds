// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Taking bytes off the wire.
//!
//! Every read that cannot be satisfied is an error naming what was
//! expected and where. A reader that returned a zero for a truncated
//! stream would hand the caller a value that was never sent.

use crate::writer::{
    BYTE_ORDER_MARK, E_FLAG_MUST_UNDERSTAND, E_MEMBER_ID_MASK, FixedDigits, ParameterRole,
    PlCdr1Framing, PlCdr1Reading, SWAPPED_BYTE_ORDER_MARK, WcharFraming, WcharPosition, WcharWidth,
    WirePosition, WstringLengthCounts, WstringLengthPolicy, WstringTerminator, WstringUnitOrder,
};
use crate::writer::{
    E_FLAG_IMPL_EXTENSION, PID_EXTENDED, PID_IGNORE, PID_IMPL_EXTENSION, PID_LIST_END,
};
use crate::{Endianness, WireError};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_float128::Float128;

/// The widest `fixed` IDL 4.2 §7.4.1.4.4.3.4 admits, in octets:
/// `fixed<31, S>` is thirty-two nibbles. A value-precision field that presents
/// no sign nibble within it is a stream this reader refuses rather than one it
/// keeps reading.
const MAX_FIXED_OCTETS: usize = 16;

/// The lowest nibble value that is a sign and not a digit.
///
/// CORBA 3.3 part 2 §9.3.2.8: a digit is `0` to `9`, and every nibble at or
/// above this is a sign. It is what makes the value-precision form
/// self-delimiting. `zerodds_ast_layout::key::FixedDigitCount::FIRST_SIGN_NIBBLE`
/// is the planner's half of the same constant, and
/// `crates/wire-codec/tests/fixed_digits_agree.rs` holds the two together.
const FIRST_SIGN_NIBBLE: u8 = 0x0a;

/// Reads one sample's bytes.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    endianness: Endianness,
    /// Where alignment is counted from.
    ///
    /// Zero for a sample, and zero inside a DHEADER as well, because
    /// XCDR counts alignment from the encapsulation and not from a
    /// nested value's own start. A PL_CDR1 parameter is the exception:
    /// §7.4.1.2 makes each one "a CDR-encapsulated block of data", so
    /// its value counts from the block. `Writer` has the same field for
    /// the same clause.
    origin: usize,
    /// The encoding's `MAXALIGN`. The writer's counterpart — a reader
    /// that skipped padding the writer never wrote reads the wrong
    /// bytes from the second member on.
    max_align: usize,
    /// How the peer is expected to have framed a PL_CDR1 parameter list.
    ///
    /// The same type the writer takes, and set from the same policy, so
    /// the two cannot drift: a reader that accepted any framing would
    /// pass every wrong-dialect run silently, and one that carried its
    /// own copy of the rules would disagree with the writer the first
    /// time either changed.
    ///
    /// Deliberately `PlCdr1Framing` and not a dialect. `zerodds-wire`
    /// has no dependencies and is not going to gain one — it executes a
    /// plan and does not know what a dialect is. The settings that *are*
    /// dialect-level reach the read path the other way, through the
    /// layout plan the reader walks, which is computed from the same
    /// `LayoutKey` the writer used.
    pl_cdr1: PlCdr1Framing,
    /// How wide a `Char16` the peer wrote is. The writer's counterpart:
    /// a reader two bytes narrower than the stream returns the right
    /// character and then reads every later member two bytes early.
    wchar_width: WcharWidth,
    /// How the peer framed a scalar `Char16`. See
    /// [`crate::writer::WcharFraming`]: an ORB writes an octet count and
    /// then the octets, and a reader that expected a bare code unit
    /// takes the count for half a character.
    wchar_framing: WcharFraming,
    /// In what byte order the peer put wide code units. See
    /// [`crate::writer::WstringUnitOrder`].
    wstring_unit_order: WstringUnitOrder,
    /// Whether a PL_CDR1 stream in another framing than `pl_cdr1` is
    /// read or reported. See [`PlCdr1Reading`].
    pl_cdr1_reading: PlCdr1Reading,
}

impl<'a> Reader<'a> {
    /// A reader over `bytes` in the given byte order.
    #[must_use]
    pub const fn new(bytes: &'a [u8], endianness: Endianness) -> Self {
        Self {
            bytes,
            at: 0,
            endianness,
            origin: 0,
            max_align: 8,
            pl_cdr1: PlCdr1Framing::DEFAULT,
            wchar_width: WcharWidth::Two,
            wchar_framing: WcharFraming::Fixed,
            wstring_unit_order: WstringUnitOrder::Stream,
            pl_cdr1_reading: PlCdr1Reading::Strict,
        }
    }

    /// A reader whose alignment is capped at `max_align`. Four for
    /// XCDR2, eight for XCDR1.
    #[must_use]
    pub const fn with_max_align(bytes: &'a [u8], endianness: Endianness, max_align: usize) -> Self {
        Self {
            bytes,
            at: 0,
            endianness,
            origin: 0,
            max_align: if max_align < 1 { 1 } else { max_align },
            pl_cdr1: PlCdr1Framing::DEFAULT,
            wchar_width: WcharWidth::Two,
            wchar_framing: WcharFraming::Fixed,
            wstring_unit_order: WstringUnitOrder::Stream,
            pl_cdr1_reading: PlCdr1Reading::Strict,
        }
    }

    /// The same reader expecting a `Char16` at another width.
    ///
    /// Set from the same
    /// `zerodds_ast_layout::WstringLengthPolicy::character_width` the
    /// writer was set from, which is what keeps the two agreeing.
    #[must_use]
    pub const fn with_wchar_width(mut self, width: WcharWidth) -> Self {
        self.wchar_width = width;
        self
    }

    /// The `Char16` width in force.
    #[must_use]
    pub const fn wchar_width(&self) -> WcharWidth {
        self.wchar_width
    }

    /// The same reader expecting another scalar `Char16` framing.
    ///
    /// Set from the same `zerodds_ast_layout::WcharFraming` the writer
    /// and the layout plan were set from.
    #[must_use]
    pub const fn with_wchar_framing(mut self, framing: WcharFraming) -> Self {
        self.wchar_framing = framing;
        self
    }

    /// The `Char16` framing in force.
    #[must_use]
    pub const fn wchar_framing(&self) -> WcharFraming {
        self.wchar_framing
    }

    /// The same reader expecting another wide code-unit order.
    #[must_use]
    pub const fn with_wstring_unit_order(mut self, order: WstringUnitOrder) -> Self {
        self.wstring_unit_order = order;
        self
    }

    /// The wide code-unit order in force.
    #[must_use]
    pub const fn wstring_unit_order(&self) -> WstringUnitOrder {
        self.wstring_unit_order
    }

    /// The same reader expecting another PL_CDR1 framing.
    #[must_use]
    pub const fn with_pl_cdr1_framing(mut self, framing: PlCdr1Framing) -> Self {
        self.pl_cdr1 = framing;
        self
    }

    /// The framing in force.
    #[must_use]
    pub const fn pl_cdr1_framing(&self) -> PlCdr1Framing {
        self.pl_cdr1
    }

    /// The same reader taking, or reporting, a PL_CDR1 stream in another
    /// framing than the one it was given.
    ///
    /// [`PlCdr1Reading::Strict`] is the default. Under
    /// [`PlCdr1Reading::Tolerant`] the reader accepts any flags on
    /// `PID_EXTENDED` and `PID_LIST_END` (XTypes 1.3 §7.4.1.2.1), the
    /// short and the extended header of a parameter whichever form the
    /// framing names (§7.4.1.2.2), and a declared length that is the
    /// value's own length or that length rounded up to four (§7.4.1.2).
    #[must_use]
    pub const fn with_pl_cdr1_reading(mut self, reading: PlCdr1Reading) -> Self {
        self.pl_cdr1_reading = reading;
        self
    }

    /// The PL_CDR1 reading in force.
    #[must_use]
    pub const fn pl_cdr1_reading(&self) -> PlCdr1Reading {
        self.pl_cdr1_reading
    }

    /// The alignment cap in force.
    #[must_use]
    pub const fn max_align(&self) -> usize {
        self.max_align
    }

    /// The byte order this reader was given.
    ///
    /// A caller that has to decide something the stream's order settles
    /// and that this reader does not read for it — a CORBA `wstring`
    /// without a byte-order mark takes the message's order, CORBA 3.3
    /// part 2 §9.3.1.6 — has to be able to ask.
    #[must_use]
    pub const fn endianness(&self) -> Endianness {
        self.endianness
    }

    /// Where it is.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.at
    }

    /// How many bytes are left.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }

    /// Move to `target`, which is a position this reader has already
    /// been told about.
    ///
    /// The one way a header-driven decoder steps over a member it does
    /// not declare: XTypes 1.3 §7.4.3.4.2 gives every `MMEMBER` an
    /// extent, and §7.4.1.2 gives every PL_CDR1 parameter a length, so
    /// where the next header starts is always known even when what sits
    /// between is not.
    ///
    /// # Errors
    /// [`WireError::Truncated`] for a target past the end of the stream
    /// and [`WireError::OutOfRange`] for one behind the current
    /// position. Neither is silently clamped: a target behind the cursor
    /// means the extent and the value disagree, and a reader that
    /// stayed put would read the *value's* bytes as the next header.
    pub fn skip_to(&mut self, target: usize) -> Result<(), WireError> {
        if target < self.at {
            return Err(WireError::OutOfRange {
                at: self.at,
                expected: alloc::format!(
                    "a member extent ending at or after {}, and this one ends at {target}",
                    self.at
                ),
            });
        }
        if target > self.bytes.len() {
            return Err(WireError::Truncated {
                at: self.at,
                needed: target - self.at,
                available: self.remaining(),
            });
        }
        self.at = target;
        Ok(())
    }

    /// A string this reader has just taken off the wire, against the
    /// bound of the member it is going into.
    ///
    /// A method rather than a free call so the offset in a
    /// [`WireError::Discarded`] is this reader's own. The narrowing
    /// itself is [`crate::try_construct::fit_string`], which is where
    /// the clause quotations are.
    ///
    /// # Errors
    /// As [`crate::try_construct::fit_string`].
    pub fn fit_string(
        &self,
        value: String,
        bound: usize,
        behaviour: crate::TryConstruct,
    ) -> Result<crate::Narrowed<String>, WireError> {
        crate::try_construct::fit_string(self.at, value, bound, behaviour)
    }

    /// A `wstring` this reader has just taken off the wire, against the
    /// bound of the member it is going into.
    ///
    /// The bound counts **UTF-16 code units** and not octets, which is
    /// why this is its own method and not [`Reader::fit_string`] with
    /// another argument. The narrowing itself is
    /// [`crate::try_construct::fit_wstring`].
    ///
    /// # Errors
    /// As [`crate::try_construct::fit_wstring`].
    pub fn fit_wstring(
        &self,
        value: String,
        bound: usize,
        behaviour: crate::TryConstruct,
    ) -> Result<crate::Narrowed<String>, WireError> {
        crate::try_construct::fit_wstring(self.at, value, bound, behaviour)
    }

    /// A sequence this reader has just taken off the wire, against the
    /// bound of the member it is going into.
    ///
    /// # Errors
    /// As [`crate::try_construct::fit_sequence`].
    pub fn fit_sequence<T>(
        &self,
        value: Vec<T>,
        bound: usize,
        behaviour: crate::TryConstruct,
    ) -> Result<crate::Narrowed<Vec<T>>, WireError> {
        crate::try_construct::fit_sequence(self.at, value, bound, behaviour)
    }

    /// A map this reader has just taken off the wire, against the bound
    /// of the member it is going into.
    ///
    /// The bound counts **entries**, which is the unit Table 17's
    /// `O2.length <= T1.length` is in for a `map<K, V, N>`.
    ///
    /// # Errors
    /// As [`crate::try_construct::fit_map`].
    pub fn fit_map<K: Ord, V>(
        &self,
        value: BTreeMap<K, V>,
        bound: usize,
        behaviour: crate::TryConstruct,
    ) -> Result<crate::Narrowed<BTreeMap<K, V>>, WireError> {
        crate::try_construct::fit_map(self.at, value, bound, behaviour)
    }

    /// A map held as its entries (XTypes 1.3 §7.5.1.3.6), against the bound of the member it
    /// is going into.
    ///
    /// # Errors
    /// As [`crate::try_construct::fit_map_entries`].
    pub fn fit_map_entries<K, V>(
        &self,
        value: Vec<crate::MapEntry<K, V>>,
        bound: usize,
        behaviour: crate::TryConstruct,
    ) -> Result<crate::Narrowed<Vec<crate::MapEntry<K, V>>>, WireError> {
        crate::try_construct::fit_map_entries(self.at, value, bound, behaviour)
    }

    /// Skip padding to `align`.
    ///
    /// # Errors
    /// [`WireError::Truncated`] when the padding the writer must have
    /// written is not there. The offset reported is where the padding
    /// began, which is where the stream stopped agreeing with itself —
    /// letting the position run past the buffer instead put every later
    /// error at an offset the stream does not have.
    pub fn align_to(&mut self, align: usize) -> Result<(), WireError> {
        let align = if align < self.max_align {
            align
        } else {
            self.max_align
        };
        if align <= 1 {
            return Ok(());
        }
        let offset = self.at - self.origin;
        let padding = (align - (offset % align)) % align;
        if padding > self.remaining() {
            return Err(WireError::Truncated {
                at: self.at,
                needed: padding,
                available: self.remaining(),
            });
        }
        self.at += padding;
        Ok(())
    }

    /// Begin a collection element whose alignment is counted from its
    /// own first octet, returning the origin it replaced.
    ///
    /// The read-side pair of [`Writer::begin_packed_element`](crate::Writer::begin_packed_element), and it
    /// has to move with it: a reader that aligned the departure from the
    /// encapsulation would consume the padding its writer did not emit
    /// and misread every element after the first.
    pub fn begin_packed_element(&mut self) -> usize {
        let saved = self.origin;
        self.origin = self.at;
        saved
    }

    /// Restore the stream origin a packed element displaced. The pair of
    /// [`Self::begin_packed_element`].
    pub fn finish_packed_element(&mut self, saved: usize) {
        self.origin = saved;
    }

    // -- primitives ------------------------------------------------------

    /// `boolean`.
    ///
    /// # Errors
    /// [`WireError::Truncated`], or [`WireError::OutOfRange`] for a byte
    /// that is neither 0 nor 1 — a boolean that is 2 is a stream that
    /// disagrees with its own type.
    pub fn read_bool(&mut self) -> Result<bool, WireError> {
        let at = self.at;
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(WireError::OutOfRange {
                at,
                expected: String::from("a boolean of 0 or 1"),
            }),
        }
    }

    /// `octet` and `uint8`.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_u8(&mut self) -> Result<u8, WireError> {
        let byte = *self.bytes.get(self.at).ok_or(WireError::Truncated {
            at: self.at,
            needed: 1,
            available: self.remaining(),
        })?;
        self.at += 1;
        Ok(byte)
    }

    /// `int8`.
    ///
    /// # Errors
    /// As [`Reader::read_u8`].
    pub fn read_i8(&mut self) -> Result<i8, WireError> {
        #[allow(clippy::cast_possible_wrap)]
        Ok(self.read_u8()? as i8)
    }

    /// `char`.
    ///
    /// # Errors
    /// As [`Reader::read_u8`].
    pub fn read_char(&mut self) -> Result<char, WireError> {
        Ok(char::from(self.read_u8()?))
    }

    /// `wchar`, at the width [`Reader::wchar_width`] names.
    ///
    /// The four-byte form is RTI's XCDR1 one, where the high sixteen
    /// bits are zero padding around a code unit. A value that uses them
    /// is refused rather than truncated: narrowing it here would return
    /// a character the peer did not send.
    ///
    /// `position` selects the framing exactly as it does on the writing
    /// side, and for the same reason: under
    /// [`WcharFraming::GiopLengthPrefixedMemberOnly`] a member carries a
    /// count octet and an element does not, so a reader given the wrong
    /// position takes the first half of a code unit for a count and
    /// refuses — or, at the one width where the count is a plausible
    /// unit, returns a character nobody sent.
    ///
    /// # Errors
    /// [`WireError::Truncated`], or [`WireError::OutOfRange`] for a code
    /// point that is not a character, or for a four-byte character
    /// holding more than one UTF-16 code unit.
    pub fn read_wchar(&mut self, position: WcharPosition) -> Result<char, WireError> {
        let width = self.wchar_width;
        let at = self.at;
        let value = if self.wchar_framing.has_length_octet(position) {
            self.read_giop_wchar()?
        } else {
            self.align_to(width.bytes())?;
            match width {
                WcharWidth::Two => u32::from(self.read_u16()?),
                WcharWidth::Four => self.read_u32()?,
            }
        };
        if value > 0xFFFF {
            return Err(WireError::OutOfRange {
                at,
                expected: String::from("a four-byte wchar holding one UTF-16 code unit"),
            });
        }
        char::from_u32(value).ok_or(WireError::OutOfRange {
            at,
            expected: String::from("a Unicode code point"),
        })
    }

    /// [`Reader::read_wchar`] at the member position.
    ///
    /// The reading counterpart of `Writer::write_wchar_member`, and
    /// there for the same reason: a collection combinator takes a path
    /// of one argument.
    ///
    /// # Errors
    /// As [`Reader::read_wchar`].
    pub fn read_wchar_member(&mut self) -> Result<char, WireError> {
        self.read_wchar(WcharPosition::Member)
    }

    /// [`Reader::read_wchar`] at the collection-element position.
    ///
    /// # Errors
    /// As [`Reader::read_wchar`].
    pub fn read_wchar_element(&mut self) -> Result<char, WireError> {
        self.read_wchar(WcharPosition::Element)
    }

    /// CORBA §9.3.1.6's GIOP 1.2 `wchar`: an octet count, then that many
    /// octets, aligned to one.
    ///
    /// The count is checked against the configured code-set width rather
    /// than believed. A stream whose first octet says `0xE4` — which is
    /// what a bare two-octet `wchar` looks like read this way — would
    /// otherwise consume 228 octets of whatever followed and return a
    /// character assembled out of the next four members. The count is
    /// the one field a peer controls and the one a reader must never let
    /// decide how much it reads.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a count that is not the configured
    /// width, [`WireError::Truncated`] for a stream that ends inside the
    /// octets.
    fn read_giop_wchar(&mut self) -> Result<u32, WireError> {
        let at = self.at;
        let order = self.wstring_unit_order;
        let expected = self.wchar_width.bytes();
        let declared = usize::from(self.read_u8()?);
        if declared != expected {
            return Err(WireError::OutOfRange {
                at,
                expected: format!(
                    "a GIOP 1.2 wchar of {expected} octets, the configured transmission code set, \
                     and not {declared}"
                ),
            });
        }
        let octets = self.take(expected)?;
        let mut value: u32 = 0;
        // A scalar cannot carry a byte-order mark, so `ByteOrderMark`
        // means CORBA's default here — big-endian — exactly as
        // `BigEndianAlways` does. `Writer::write_wchar` has the run.
        let little = matches!(order, WstringUnitOrder::Stream) && self.endianness.is_little();
        if little {
            for (shift, octet) in octets.iter().enumerate() {
                value |= u32::from(*octet) << (8 * shift);
            }
        } else {
            for octet in octets {
                value = (value << 8) | u32::from(*octet);
            }
        }
        Ok(value)
    }

    /// A bitmask, read from the holder its `@bit_bound` names, reduced to
    /// the bits the type declares.
    ///
    /// `declared` is the set of positions the bitmask names, one bit per
    /// position, and it is an argument rather than a runtime table
    /// because the type is the generated code's and the wire is this
    /// crate's.
    ///
    /// DDS-XTypes 1.3 §7.4.1.1.3.2: "the states of the remaining
    /// serialized bits are not specified, and those bits are not
    /// considered to be part of the bitmask", and Table 18: "Preserve bit
    /// values by index for all bits identified in both T1 and T2." A bit
    /// the stream sets and the type does not name is therefore not part
    /// of the value this type reads, and it is dropped; it is not a
    /// reason to refuse the sample, which would make a reader refuse a
    /// writer whose type Table 18 declares assignable.
    ///
    /// # Errors
    /// [`WireError::Truncated`] for a short stream, and
    /// [`WireError::OutOfRange`] for a bound outside 1..=64.
    pub fn read_bitmask(&mut self, bit_bound: u16, declared: u64) -> Result<u64, WireError> {
        if bit_bound == 0 || bit_bound > 64 {
            return Err(WireError::OutOfRange {
                at: self.at,
                expected: alloc::format!(
                    "a bitmask bound between 1 and 64, and this one is {bit_bound}"
                ),
            });
        }
        let bits = match bit_bound {
            0..=8 => u64::from(self.read_u8()?),
            9..=16 => u64::from(self.read_u16()?),
            17..=32 => u64::from(self.read_u32()?),
            _ => self.read_u64()?,
        };
        Ok(bits & declared)
    }

    /// A bitset's anchor integer, of the width the plan sized it at.
    ///
    /// The counterpart of `Writer::write_bitset`; the unpacking of the
    /// fields out of the anchor is the generated code's, because the
    /// positions belong to the type and not to the wire.
    ///
    /// # Errors
    /// [`WireError::Truncated`] for a short stream, and
    /// [`WireError::OutOfRange`] for an anchor that is not one, two,
    /// four or eight octets.
    pub fn read_bitset(&mut self, anchor_bytes: u32) -> Result<u64, WireError> {
        match anchor_bytes {
            1 => Ok(u64::from(self.read_u8()?)),
            2 => Ok(u64::from(self.read_u16()?)),
            4 => Ok(u64::from(self.read_u32()?)),
            8 => self.read_u64(),
            other => Err(WireError::OutOfRange {
                at: self.at,
                expected: alloc::format!(
                    "a bitset anchor of 1, 2, 4 or 8 octets, and this one is {other}"
                ),
            }),
        }
    }

    /// `unsigned short`.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_u16(&mut self) -> Result<u16, WireError> {
        self.align_to(2)?;
        let bytes: [u8; 2] = self.take(2)?.try_into().map_err(|_| WireError::Truncated {
            at: self.at,
            needed: 2,
            available: self.remaining(),
        })?;
        Ok(if self.endianness.is_little() {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    }

    /// A 16-bit value in **little-endian** order whatever the stream's
    /// order is. The counterpart of `Writer::write_u16_little`, and see
    /// there for the one caller it has.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_u16_little(&mut self) -> Result<u16, WireError> {
        self.align_to(2)?;
        let bytes: [u8; 2] = self.take(2)?.try_into().map_err(|_| WireError::Truncated {
            at: self.at,
            needed: 2,
            available: self.remaining(),
        })?;
        Ok(u16::from_le_bytes(bytes))
    }

    /// A 16-bit value in **big-endian** order whatever the stream's
    /// order is. The counterpart of `Writer::write_u16_big`.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_u16_big(&mut self) -> Result<u16, WireError> {
        self.align_to(2)?;
        let bytes: [u8; 2] = self.take(2)?.try_into().map_err(|_| WireError::Truncated {
            at: self.at,
            needed: 2,
            available: self.remaining(),
        })?;
        Ok(u16::from_be_bytes(bytes))
    }

    /// A 32-bit value in **little-endian** order whatever the stream's
    /// order is. The counterpart of `Writer::write_u32_little`.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_u32_little(&mut self) -> Result<u32, WireError> {
        self.align_to(4)?;
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| WireError::Truncated {
            at: self.at,
            needed: 4,
            available: self.remaining(),
        })?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// A 32-bit value in **big-endian** order whatever the stream's
    /// order is. The counterpart of `Writer::write_u32_big`.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_u32_big(&mut self) -> Result<u32, WireError> {
        self.align_to(4)?;
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| WireError::Truncated {
            at: self.at,
            needed: 4,
            available: self.remaining(),
        })?;
        Ok(u32::from_be_bytes(bytes))
    }

    /// `short`.
    ///
    /// # Errors
    /// As [`Reader::read_u16`].
    pub fn read_i16(&mut self) -> Result<i16, WireError> {
        #[allow(clippy::cast_possible_wrap)]
        Ok(self.read_u16()? as i16)
    }

    /// `unsigned long`.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_u32(&mut self) -> Result<u32, WireError> {
        self.align_to(4)?;
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| WireError::Truncated {
            at: self.at,
            needed: 4,
            available: self.remaining(),
        })?;
        Ok(if self.endianness.is_little() {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    }

    /// `long`.
    ///
    /// # Errors
    /// As [`Reader::read_u32`].
    pub fn read_i32(&mut self) -> Result<i32, WireError> {
        #[allow(clippy::cast_possible_wrap)]
        Ok(self.read_u32()? as i32)
    }

    /// `unsigned long long`.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_u64(&mut self) -> Result<u64, WireError> {
        self.align_to(8)?;
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| WireError::Truncated {
            at: self.at,
            needed: 8,
            available: self.remaining(),
        })?;
        Ok(if self.endianness.is_little() {
            u64::from_le_bytes(bytes)
        } else {
            u64::from_be_bytes(bytes)
        })
    }

    /// `long long`.
    ///
    /// # Errors
    /// As [`Reader::read_u64`].
    pub fn read_i64(&mut self) -> Result<i64, WireError> {
        #[allow(clippy::cast_possible_wrap)]
        Ok(self.read_u64()? as i64)
    }

    /// `float`.
    ///
    /// # Errors
    /// As [`Reader::read_u32`].
    pub fn read_f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_bits(self.read_u32()?))
    }

    /// `double`.
    ///
    /// # Errors
    /// As [`Reader::read_u64`].
    pub fn read_f64(&mut self) -> Result<f64, WireError> {
        Ok(f64::from_bits(self.read_u64()?))
    }

    /// `long double`: all sixteen bytes, as IEEE 754 binary128.
    ///
    /// The counterpart of [`Writer::write_f128`](crate::Writer::write_f128). This used to read the
    /// first eight bytes as a `double` and skip the rest, which decoded
    /// every value a `double` cannot hold as a different number and said
    /// nothing.
    ///
    /// The bytes are taken whole and reversed as a whole for a
    /// big-endian stream, per CORBA 3.3 part 2 Figure 9.2. Alignment is
    /// eight, capped to four under XCDR2 by
    /// [`Reader::align_to`]'s `MAXALIGN`.
    ///
    /// # Errors
    /// [`WireError::Truncated`].
    pub fn read_f128(&mut self) -> Result<Float128, WireError> {
        self.align_to(8)?;
        let bytes = self.take(16)?;
        let mut sixteen = [0u8; 16];
        sixteen.copy_from_slice(bytes);
        Ok(if self.endianness.is_little() {
            Float128::from_le_bytes(sixteen)
        } else {
            Float128::from_be_bytes(sixteen)
        })
    }

    // -- strings ---------------------------------------------------------

    /// `fixed<P, S>`: packed BCD, CORBA 3.3 part 2 §9.3.2.8, as a decimal
    /// string.
    ///
    /// A string and not a number, for the reason the twenty other language
    /// columns of this project carry it as text too: `fixed<31, 2>` fits no
    /// integer Rust has, and a binary float would be the wrong answer rather
    /// than an approximate one, because `fixed` exists to be exact.
    ///
    /// The text that comes back is the one spelling the declaration admits —
    /// no leading zeros past a single one before the point, exactly `S`
    /// after it, and a `-` only on a value that is neither zero nor
    /// positive. `1.5` and `0001.500` are the same sample under
    /// `fixed<7, 3>` and only one string can come out of it.
    ///
    /// # Errors
    /// [`WireError::Truncated`] for a stream that ends inside the field.
    /// [`WireError::OutOfRange`] for a declaration outside IDL 4.2 rule
    /// (42), a nibble that is neither a digit nor a sign, or — under
    /// [`FixedDigits::ValuePrecision`] or [`FixedDigits::ValueStringDigits`] —
    /// a field that presents no sign nibble within the widest `fixed` IDL
    /// admits.
    pub fn read_fixed(
        &mut self,
        digits: u8,
        scale: u8,
        policy: FixedDigits,
    ) -> Result<String, WireError> {
        // The same bound as the writer's, on this side too: rule (42) caps
        // the BCD form at 31 digits, and a reader asked for a wider field
        // would take octets no declaration can name.
        if digits == 0 || digits > 31 || scale > digits {
            return Err(WireError::refused(
                self.position(),
                "a fixed whose digit count and scale can describe a value",
            ));
        }
        let mut read = Vec::with_capacity(usize::from(digits) + 2);
        let nibbles = match policy {
            FixedDigits::Declared => {
                // The declaration decides how wide the field is, so the
                // octet count is known before a byte is read.
                let nibbles = usize::from(digits) + 1;
                for _ in 0..nibbles.div_ceil(2) {
                    let byte = self.read_u8()?;
                    read.push(byte >> 4);
                    read.push(byte & 0x0f);
                }
                nibbles
            }
            FixedDigits::ValuePrecision | FixedDigits::ValueStringDigits => {
                // The value decides, so the field's end is read out of the
                // octets: §9.3.2.8 makes a digit `0` to `9` and a sign `0xa`
                // to `0xf`, and the nibble count is even by construction, so
                // the field ends at the first octet whose **low** nibble is
                // a sign. That is what makes this form decodable at all. A
                // field wider than `fixed<31, S>`'s sixteen octets is a
                // stream that never presents a sign nibble, and is refused
                // rather than read on.
                let mut octets = 0usize;
                loop {
                    if octets >= MAX_FIXED_OCTETS {
                        return Err(WireError::refused(
                            self.position(),
                            "a sign nibble within the widest fixed IDL admits",
                        ));
                    }
                    let byte = self.read_u8()?;
                    read.push(byte >> 4);
                    read.push(byte & 0x0f);
                    octets += 1;
                    if byte & 0x0f >= FIRST_SIGN_NIBBLE {
                        break;
                    }
                }
                read.len()
            }
        };
        // A leading zero nibble pads the pair when the nibble count is odd;
        // it is not a digit and the writer put it there for the packing
        // alone. Under the value's count the pad is already inside what was
        // read, and stripping a leading zero changes no value.
        let body = read.get(read.len() - nibbles..).unwrap_or(&[]);
        let Some((sign, value)) = body.split_last() else {
            return Err(WireError::refused(
                self.position(),
                "at least a sign nibble",
            ));
        };
        let negative = match sign {
            0x0d | 0x0b => true,
            0x0c | 0x0a | 0x0f | 0x0e => false,
            _ => {
                return Err(WireError::refused(
                    self.position(),
                    "a sign nibble, 0xc or 0xd",
                ));
            }
        };
        // The point sits `scale` digits from the **right** and not `P - S`
        // from the left. Under the declared count the two are the same
        // arithmetic, because the field carries exactly `P` digits; under
        // the value's count it carries fewer, and a value with fewer digits
        // than the scale — `1c` under `fixed<5, 2>` — needs its integer part
        // written as a single zero. Enough leading zeros are prepended for
        // the integer part to have at least one digit, which makes the split
        // below unconditional.
        let mut spelled = String::with_capacity(value.len() + usize::from(scale) + 1);
        for _ in value.len()..usize::from(scale) + 1 {
            spelled.push('0');
        }
        for digit in value {
            if *digit > 9 {
                return Err(WireError::refused(
                    self.position(),
                    "a decimal digit nibble",
                ));
            }
            spelled.push(char::from(b'0' + digit));
        }
        // The one spelling the declaration admits: leading zeros beyond a
        // single one go, and a negative zero comes back positive because
        // §9.3.2.8 has one zero and gives it the positive sign nibble.
        let split = spelled.len() - usize::from(scale);
        let whole = spelled[..split].trim_start_matches('0');
        let fraction = &spelled[split..];
        let mut text = String::with_capacity(spelled.len() + 2);
        if negative && !(whole.is_empty() && fraction.bytes().all(|byte| byte == b'0')) {
            text.push('-');
        }
        text.push_str(if whole.is_empty() { "0" } else { whole });
        if scale > 0 {
            text.push('.');
            text.push_str(fraction);
        }
        // A terminated BCD field can still exceed this declaration's bound
        // under ValuePrecision. Apply the writer's shared rule before
        // returning a value that the same type could never write back.
        crate::writer::parse_fixed(&text, digits, scale, self.position())?;
        Ok(text)
    }

    /// A string: a length including the terminator, the bytes, the NUL.
    ///
    /// # Errors
    /// [`WireError::ImpossibleLength`] for a length the stream cannot
    /// hold — allocating for it is how a reader turns a bad message into
    /// an out-of-memory. [`WireError::NotUtf8`] for bytes that are not.
    pub fn read_string(&mut self) -> Result<String, WireError> {
        // Aligned before the offset is taken, so `at` names the length
        // field and not the padding in front of it. An error whose
        // offset points at padding cannot be matched to a length field
        // by anyone reading a hex dump.
        self.align_to(4)?;
        let at = self.at;
        let length = self.read_u32()? as usize;
        if length == 0 || length > self.remaining() + 1 {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: length,
                available: self.remaining(),
            });
        }
        // The length counts the terminator; the text is one byte less.
        let text_at = self.at;
        let text = self.take(length - 1)?;
        let out = core::str::from_utf8(text)
            .map_err(|_| WireError::NotUtf8 { at })?
            .to_string();
        // §7.4.1.1.2 puts a NUL at the end and the length counts it. A
        // last byte that is not one is a frame that disagrees with
        // itself, and reading past it takes the next member's first
        // byte for a terminator.
        let terminator_at = self.at;
        if self.read_u8()? != 0 {
            return Err(WireError::OutOfRange {
                at: terminator_at,
                expected: String::from("a NUL terminating the string, as its length counts"),
            });
        }
        // A NUL inside the text is worse than a wrong terminator: every
        // C-based peer stops at it, so the value this reader returns and
        // the value the peer returns differ for bytes both accepted.
        if let Some(offset) = out.bytes().position(|byte| byte == 0) {
            return Err(WireError::OutOfRange {
                at: text_at + offset,
                expected: String::from("no NUL inside a string a C peer would stop at"),
            });
        }
        Ok(out)
    }

    /// A `wstring`, in the cell `policy` and `order` name.
    ///
    /// The default is XTypes 1.3 §7.4.1.1.2 — an octet count, "twice
    /// the number of characters in the string", no terminator and no
    /// byte-order mark to strip. Eight of the nine measured
    /// implementations write something else in at least one axis, and
    /// [`WstringLengthPolicy`] carries the octets and the argument.
    ///
    /// # Nothing here looks at the length to decide which cell it is
    ///
    /// The length is the field a hostile peer controls, and the cells
    /// are not separable anyway: Fast DDS's `@final struct { wstring w;
    /// long n; }` with `w = "ab", n = 7` is `02000000 61006200
    /// 07000000`, and the specification's encoding of the same type with
    /// `w = "a", n = 7` is `02000000 6100`, two octets of padding,
    /// `07000000`. Twelve octets each, differing only in the content of
    /// padding that no clause fixes. A reader choosing between those two
    /// samples would not be resolving an ambiguity, it would be taking
    /// its answer from the length field.
    ///
    /// So the configured cell is read, and a length that cell cannot
    /// have produced is refused with the cell it *does* fit named in
    /// the error — a diagnosis after the refusal, not a parse decided
    /// by the stream.
    ///
    /// # Errors
    ///
    /// [`WireError::WrongFraming`] for a length the configured cell
    /// cannot have produced: an octet count the unit width does not
    /// divide, or a count of zero where C2 says the count includes a
    /// terminator. Rounding an odd octet count down would leave the
    /// remainder in the stream and put every later member at the wrong
    /// offset.
    ///
    /// [`WireError::ImpossibleLength`] for a length past the end of the
    /// stream — allocating for it is how a reader turns a bad message
    /// into an out-of-memory.
    ///
    /// [`WireError::OutOfRange`] for a four-octet character above
    /// `0xFFFF` (the wide form zero-extends a code *unit*, so a value
    /// that does not fit one is not a form this reader knows how to
    /// narrow), for a terminator that is not a NUL where one is
    /// expected, for a NUL inside the text where a terminator is
    /// expected — every C peer stops at it, so this reader's value and
    /// the peer's would differ for octets both accepted — and for units
    /// that are not well-formed UTF-16. An unpaired surrogate is
    /// refused rather than replaced: U+FFFD in its place would be a
    /// character the sender never had.
    ///
    /// That last refusal catches less than it looks. Fast CDR's
    /// truncation of `U+1D11E` is `U+D11E`, a Hangul syllable and
    /// well-formed UTF-16, so the wrong character arrives with nothing
    /// to object to. Only a code point whose low sixteen bits land in
    /// `D800..=DFFF` fails here — which is why
    /// [`Writer::write_wstring`](crate::Writer::write_wstring) refuses to produce the truncation in
    /// the first place.
    pub fn read_wstring(
        &mut self,
        policy: WstringLengthPolicy,
        order: WstringUnitOrder,
        position: WirePosition,
    ) -> Result<String, WireError> {
        // Aligned before the offset is taken, so `at` names the length
        // field and not the padding in front of it.
        self.align_to(4)?;
        let at = self.at;
        let declared = self.read_u32()? as usize;
        let width = policy.character_width(position);
        // The three coordinates, not a list of named forms. C1 says
        // whether the count is of units or of the octets they occupy,
        // C3 says what an octet count divides by, and C2 is handled
        // after the run because a terminator occupies a unit on the
        // wire whichever way it is counted.
        let count = match policy.counts_at(position) {
            WstringLengthCounts::Characters => declared,
            WstringLengthCounts::Octets => {
                if declared % width != 0 {
                    return Err(wstring_form_mismatch(policy, declared, at));
                }
                declared / width
            }
        };
        let mut units = self.read_wstring_units(count, width, order, at)?;
        if matches!(policy.terminator_at(position), WstringTerminator::Present) {
            // The count includes the NUL, so a count of zero is a frame
            // that does not even hold its own terminator.
            if declared < 1 {
                return Err(wstring_form_mismatch(policy, declared, at));
            }
            let terminator = units.pop();
            if terminator != Some(0) {
                return Err(WireError::OutOfRange {
                    at,
                    expected: String::from(
                        "a NUL character ending the wstring, as its count includes one",
                    ),
                });
            }
            // A NUL inside the text is the narrow string's problem in
            // wide form. Only where a terminator is expected — a cell
            // without one is entitled to carry U+0000 as a character.
            if let Some(offset) = units.iter().position(|unit| *unit == 0) {
                return Err(WireError::OutOfRange {
                    at,
                    expected: format!(
                        "no NUL inside a wstring a C peer would stop at, and this one has \
                         a NUL at character {offset} of {}",
                        units.len()
                    ),
                });
            }
        }
        // The mark is not a character of the string, and under
        // `ByteOrderMark` it also decides the order of everything behind
        // it — which `read_wstring_units` could not know until it had
        // read it.
        if order.has_mark() {
            strip_byte_order_mark(&mut units);
        }
        String::from_utf16(&units).map_err(|_| WireError::OutOfRange {
            at,
            expected: String::from("well-formed UTF-16, not an unpaired surrogate"),
        })
    }

    /// A `wstring` whose characters arrive as their own UTF-8 octets.
    ///
    /// The read side of `Writer::write_wstring_utf8` and of
    /// `WstringCodeUnits::Utf8`: the declared length is the UTF-8 octet
    /// count, and the terminator, when the cell carries one, is the last
    /// of those octets. A count the stream cannot hold is refused before
    /// a byte is read, for the reason [`Reader::read_wstring`] gives.
    ///
    /// # Errors
    /// [`WireError::ImpossibleLength`] for a length larger than the
    /// stream can hold, [`WireError::OutOfRange`] for a terminator cell
    /// whose last octet is not a NUL or whose run carries one earlier,
    /// and [`WireError::NotUtf8`] for octets that are not well-formed
    /// UTF-8.
    pub fn read_wstring_utf8(
        &mut self,
        policy: WstringLengthPolicy,
        position: WirePosition,
    ) -> Result<String, WireError> {
        // `read_u32` aligns to four, so `at` names the length field and
        // not the padding in front of it.
        let at = self.at;
        let declared = self.read_u32()? as usize;
        if declared > self.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: declared,
                available: self.remaining(),
            });
        }
        let mut bytes = Vec::with_capacity(declared);
        for _ in 0..declared {
            bytes.push(self.read_u8()?);
        }
        if matches!(policy.terminator_at(position), WstringTerminator::Present) {
            if bytes.pop() != Some(0) {
                return Err(WireError::OutOfRange {
                    at,
                    expected: String::from(
                        "a NUL octet ending the wstring, as its count includes one",
                    ),
                });
            }
            if bytes.contains(&0) {
                return Err(WireError::OutOfRange {
                    at,
                    expected: String::from(
                        "no NUL inside a wstring a C peer would stop at, and this one has one",
                    ),
                });
            }
        }
        String::from_utf8(bytes).map_err(|_| WireError::NotUtf8 { at })
    }

    /// `count` characters of `width` octets each, as UTF-16 code units.
    ///
    /// The four-octet form is RTI's XCDR1 one and TAO's, where a
    /// character is a code unit zero-extended; a value above `0xFFFF`
    /// there is not a form this reader knows how to narrow, so it is
    /// refused rather than truncated — truncating is the defect the
    /// whole policy exists because of.
    fn read_wstring_units(
        &mut self,
        count: usize,
        width: usize,
        order: WstringUnitOrder,
        at: usize,
    ) -> Result<Vec<u16>, WireError> {
        let octets = count.checked_mul(width).unwrap_or(usize::MAX);
        if octets > self.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: octets,
                available: self.remaining(),
            });
        }
        let mut units = Vec::with_capacity(count);
        for _ in 0..count {
            if width == 4 {
                // `ByteOrderMark` reads little-endian here and the mark
                // is re-examined once the whole run is in: a mark that
                // says `FFFE` means every unit has to be swapped back,
                // which is `strip_byte_order_mark`'s second job.
                let wide = match order {
                    WstringUnitOrder::Stream => self.read_u32()?,
                    WstringUnitOrder::BigEndianAlways => self.read_u32_big()?,
                    WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped => {
                        self.read_u32_little()?
                    }
                };
                units.push(u16::try_from(wide).map_err(|_| WireError::OutOfRange {
                    at,
                    expected: format!(
                        "a four-octet wstring character holding one UTF-16 code unit, \
                         not {wide:08x}"
                    ),
                })?);
            } else {
                units.push(match order {
                    WstringUnitOrder::Stream => self.read_u16()?,
                    WstringUnitOrder::BigEndianAlways => self.read_u16_big()?,
                    WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped => {
                        self.read_u16_little()?
                    }
                });
            }
        }
        Ok(units)
    }

    // -- framing ---------------------------------------------------------

    /// A DHEADER: the length the members that follow occupy.
    ///
    /// # Errors
    /// [`WireError::ImpossibleLength`] when it claims more than is left.
    pub fn read_dheader(&mut self) -> Result<usize, WireError> {
        self.align_to(4)?;
        let at = self.at;
        let length = self.read_u32()? as usize;
        if length > self.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: length,
                available: self.remaining(),
            });
        }
        Ok(length)
    }

    /// An EMHEADER, unpacked into its parts, its NEXTINT and the extent
    /// of the member it introduces.
    ///
    /// # The NEXTINT of length codes 5 to 7 is not consumed
    ///
    /// XTypes 1.3 §7.4.3.4.2 gives the NEXTINT of codes 5, 6 and 7 two
    /// jobs: it "shall be reused also as part of the serialized member
    /// ... because the serialization of certain members also starts
    /// with an integer length, which would take exactly the same value
    /// as NEXTINT. Therefore the use of length codes 5 to 7 saves 4
    /// bytes in the serialization."
    ///
    /// `Writer::write_emheader` reserves a placeholder for code 4 only,
    /// for that reason. This read it for every code from 4 up and left
    /// the position four bytes into the member — so the sequence that
    /// followed lost its own length word and every byte after it was
    /// read at the wrong offset. It is peeked now, and the member is
    /// read from where it starts.
    ///
    /// # The extent
    ///
    /// `extent` is how many bytes the member occupies counted from the
    /// position this call leaves the reader at, which is the number a
    /// caller needs whichever code it turned out to be:
    ///
    /// | code | extent |
    /// |---|---|
    /// | 0, 1, 2, 3 | 1, 2, 4, 8 |
    /// | 4 | the NEXTINT, which this call consumed |
    /// | 5 | 4 + NEXTINT |
    /// | 6 | 4 + 4 × NEXTINT |
    /// | 7 | 4 + 8 × NEXTINT |
    ///
    /// # Errors
    /// [`WireError::Truncated`], or [`WireError::ImpossibleLength`] for
    /// a NEXTINT claiming more than the stream holds. Allocating on an
    /// unchecked NEXTINT is how a reader turns a malformed message into
    /// an out-of-memory; this returned `Ok` for `ffffffff`.
    pub fn read_emheader(&mut self) -> Result<EmHeaderRead, WireError> {
        let word = self.read_u32()?;
        #[allow(clippy::cast_possible_truncation)]
        let length_code = ((word >> 28) & 0x7) as u8;
        let member_id = word & 0x0FFF_FFFF;
        let must_understand = (word >> 31) != 0;

        let at = self.at;
        let (next_int, extent) = match length_code {
            0 => (None, 1),
            1 => (None, 2),
            2 => (None, 4),
            3 => (None, 8),
            4 => {
                let next = self.read_u32()? as usize;
                (Some(next), next)
            }
            // Peeked, not consumed: the member's own serialization
            // begins at this word.
            code => {
                let next = self.peek_u32()? as usize;
                let scale = match code {
                    5 => 1,
                    6 => 4,
                    _ => 8,
                };
                let body = next.checked_mul(scale).ok_or(WireError::ImpossibleLength {
                    at,
                    claimed: next,
                    available: self.remaining(),
                })?;
                let extent = body.checked_add(4).ok_or(WireError::ImpossibleLength {
                    at,
                    claimed: next,
                    available: self.remaining(),
                })?;
                (Some(next), extent)
            }
        };
        if extent > self.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: extent,
                available: self.remaining(),
            });
        }
        Ok(EmHeaderRead {
            member_id,
            length_code,
            must_understand,
            next_int,
            extent,
        })
    }

    /// A PL_CDR1 parameter header, in whichever form it was written.
    ///
    /// # The flag bits are not part of the id
    ///
    /// XTypes 1.3 §7.4.1.2 splits the sixteen-bit field: the member id
    /// is the low fourteen bits, bit 14 is must-understand and bit 15 is
    /// implementation-specific. Cyclone DDS sets must-understand on
    /// every parameter it writes, so its `PID_EXTENDED` goes out as
    /// `0x7f01` and its sentinel as `0x7f02` —
    /// `crates/ast-layout/tests/fixtures/vendor_bytes.md` §2 records
    /// both, and RTI Connext writes the same sentinel.
    ///
    /// Comparing the whole word against `0x3f01` and `0x3f02` therefore
    /// recognised neither. A Cyclone parameter list read that way takes
    /// the extended header's id and length for eight bytes of value, and
    /// runs on past the sentinel into whatever follows it, with no error
    /// anywhere. Two of the three implementations this project is
    /// measured against write those bytes.
    ///
    /// # Errors
    /// [`WireError::Truncated`]; [`WireError::ImpossibleLength`] for a
    /// declared length longer than the stream holds;
    /// [`WireError::OutOfRange`] for an extended header whose short
    /// length is not the 8 §7.4.1.2 fixes it at.
    pub fn read_pid(&mut self) -> Result<PidRead, WireError> {
        self.read_pid_as(ParameterRole::Member)
    }

    /// A PL_CDR1 parameter header in a named role.
    ///
    /// The bytes are the same bytes; the role decides which length
    /// convention [`Reader::finish_pid`] checks the declared length
    /// against, because one implementation pads a `@mutable` member's
    /// and not an `@optional` member's. See [`ParameterRole`].
    ///
    /// # Errors
    /// As [`Reader::read_pid`].
    pub fn read_pid_as(&mut self, role: ParameterRole) -> Result<PidRead, WireError> {
        self.align_to(4)?;
        let pid = self.read_u16()?;
        let at = self.at;
        let length = self.read_u16()?;
        let must_understand = pid & PID_MUST_UNDERSTAND != 0;
        let id = pid & PID_ID_MASK;

        if id == PID_EXTENDED & PID_ID_MASK {
            // The marker's flags are asked at
            // `WirePosition::ExtendedMarker` and nowhere else. They are
            // not a property of the framing form: Cyclone and RTI write
            // different forms and the same `7f01`, while Fast DDS and
            // OpenDDS write the same form and `3f01` against `ff01`.
            let expected = self
                .pl_cdr1
                .pid_flags
                .flags_at(WirePosition::ExtendedMarker);
            let found = pid & !PID_ID_MASK;
            if found != expected && !self.pl_cdr1_reading.is_tolerant() {
                return Err(self.wrong_framing(at, "PID_EXTENDED", expected, found));
            }
            if length != 8 {
                return Err(WireError::OutOfRange {
                    at,
                    expected: String::from("a PID_EXTENDED header of 8 bytes, per §7.4.1.2"),
                });
            }
            let member_header = self.read_u32()?;
            let at = self.at;
            let length = self.read_u32()? as usize;
            if length > self.remaining() {
                return Err(WireError::ImpossibleLength {
                    at,
                    claimed: length,
                    available: self.remaining(),
                });
            }
            // The four flag bits are not part of the id. XTypes 1.3
            // §7.4.1.2.1 splits the `eMemberHeader` into `FLAG_1` to
            // `FLAG_4` and a 28-bit member id, and `FLAG_2` is the
            // member's must-understand.
            //
            // Taking the whole word for the id gave a Cyclone member
            // declared `@must_understand` an id of `0x40000001`, which
            // matches nothing the model declares, and the member was
            // **silently dropped**. Cyclone writes `01000040` for
            // `@id(1) @must_understand` and `02000000` for the plain
            // member beside it — `vendor_bytes.md` §2.
            //
            // The marker's own must-understand bit is a different bit
            // in a different word and is checked above; Cyclone sets it
            // on every parameter, so reading the member's property off
            // it would make the whole list must-understand.
            return Ok(PidRead {
                pid: member_header & E_MEMBER_ID_MASK,
                length,
                end: false,
                must_understand: member_header & E_FLAG_MUST_UNDERSTAND != 0,
                // `FLAG_1` is `FLAG_IMPL_EXTENSION` in the extended
                // header exactly as bit 15 is in the short one.
                impl_extension: member_header & E_FLAG_IMPL_EXTENSION != 0,
                // The extended header carries a 28-bit id and has no
                // reserved range: 0x3F01 upwards are ordinary member ids
                // there, which is why the short form needs the extended
                // one for them at all.
                reserved: false,
                ignore: false,
                role,
                saved_origin: self.begin_parameter_value(),
            });
        }

        let length = length as usize;
        let end = id == PID_LIST_END & PID_ID_MASK;
        if !end && self.pl_cdr1.require_extended && !self.pl_cdr1_reading.is_tolerant() {
            return Err(WireError::WrongFraming {
                at,
                expected: alloc::string::String::from(
                    "PID_EXTENDED for every parameter, as Cyclone DDS writes",
                ),
                found: alloc::format!("the short header for id {id}"),
                fits: Some("--pl-cdr1-headers specification or short-true-length"),
            });
        }
        // The sentinel's flags are asked at `WirePosition::Sentinel` and
        // nowhere else. They are not the member's: OpenDDS sets both
        // flags on every member id and closes with a bare `3f02`, while
        // Cyclone and RTI leave the member bare and close with `7f02`.
        let sentinel_flags = self.pl_cdr1.pid_flags.flags_at(WirePosition::Sentinel);
        if end && pid & !PID_ID_MASK != sentinel_flags && !self.pl_cdr1_reading.is_tolerant() {
            return Err(self.wrong_framing(at, "PID_LIST_END", sentinel_flags, pid & !PID_ID_MASK));
        }
        if !end && length > self.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: length,
                available: self.remaining(),
            });
        }
        let saved_origin = if end {
            self.origin
        } else {
            self.begin_parameter_value()
        };
        Ok(PidRead {
            pid: u32::from(id),
            length,
            end,
            must_understand,
            // Under a framing that puts `FLAG_IMPL_EXTENSION` on every
            // member id the bit no longer tells a vendor-private
            // parameter from a user member — OpenDDS writes `c0010004`
            // for `@id(1) long a`. It is reported absent there because
            // under that framing it carries no information, and
            // reporting it would send every member down the
            // unknown-parameter path. The loss is the writing
            // implementation's, not this reader's: a stream in which
            // every id sets the bit has no way left to say "vendor
            // private".
            impl_extension: pid & PID_IMPL_EXTENSION != 0
                && self.pl_cdr1.pid_flags.flags_at(WirePosition::Member) & PID_IMPL_EXTENSION == 0,
            // 0x3F01 and 0x3F02 never reach here — both are handled
            // above — so what this catches is `PID_IGNORE` and the rest
            // of §7.4.1.2.2's reserved range.
            // The threshold is the framing's and not a constant: one
            // implementation carries the whole low fourteen bits, and
            // reading `@id(16383)` as a reserved parameter there drops
            // the member without a word.
            reserved: !end && id > self.pl_cdr1.highest_short_id,
            ignore: id == PID_IGNORE,
            role,
            saved_origin,
        })
    }

    /// Start a parameter's value, returning the origin it replaces.
    ///
    /// The mirror of `Writer::begin_parameter_value`, and it has to
    /// exist for the same clause. XTypes 1.3 §7.4.1.2: "Each element, or
    /// parameter, within a parameter list data structure is simply a
    /// CDR-encapsulated block of data." A block counts alignment from
    /// its own start, so an eight-byte member at the head of one is
    /// already aligned.
    ///
    /// The writer was given this and the reader was not. Cyclone,
    /// RTI and Fast DDS all put `Ext`'s `double` directly behind its
    /// header — `crates/ast-layout/tests/fixtures/vendor_bytes.md` §2
    /// records the header as `01000800`, an eight-byte value with no
    /// padding in front of it. Counting from the stream instead put four
    /// bytes of padding there that no vendor wrote, so the double came
    /// out of the following bytes and every member after it was read at
    /// the wrong offset. No error anywhere: eight bytes were available,
    /// and eight bytes are a double.
    fn begin_parameter_value(&mut self) -> usize {
        let saved = self.origin;
        self.origin = self.at;
        saved
    }

    /// End a parameter's value and step to the next header.
    ///
    /// The padding to four follows the value and is outside the
    /// declared length, so the next parameter is found by alignment and
    /// not by adding the length — which is what lets one reader take
    /// RTI's rounded-up length and Fast DDS's exact one.
    ///
    /// The padding is consumed here only under a framing that writes it
    /// (`PlCdr1Framing::pad_after_value`), mirroring
    /// `Writer::finish_pid`: under Fast DDS's and OpenDDS's framing a
    /// parameter writes no padding of its own — the next header aligns
    /// itself in `read_pid_as` — and a *final* `@optional` parameter of
    /// a non-mutable type ends the stream where its value does, so an
    /// unconditional alignment here read two bytes past the end of a
    /// stream the writer had closed correctly (measured:
    /// `zerodds-rust/opendds-xcdr1-*/annotation-must-understand-unannotated`).
    ///
    /// # Errors
    /// [`WireError::Truncated`] where the framing's padding is not
    /// there.
    pub fn finish_pid(&mut self, header: PidRead) -> Result<(), WireError> {
        if !header.end {
            self.check_declared_length(&header)?;
        }
        self.origin = header.saved_origin;
        if self.pl_cdr1.pad_after_value {
            return self.align_to(4);
        }
        Ok(())
    }

    /// End a parameter whose member this reader does not declare.
    ///
    /// [`Reader::finish_pid`] finds the next header by alignment from
    /// wherever the value's read left the cursor, which is the right
    /// answer when the value *was* read; it also checks the declared
    /// length against how far the cursor moved, which for a member
    /// nobody read has moved nowhere. This one steps over the declared
    /// length instead. §7.4.1.2 gives every parameter one, which is what
    /// makes an unknown member skippable at all.
    ///
    /// # Errors
    /// [`WireError::Truncated`] where the declared length runs past the
    /// stream, and whatever the alignment to the next header returns.
    pub fn skip_parameter(&mut self, header: PidRead) -> Result<(), WireError> {
        if !header.end {
            // `origin` is the value's own start until it is put back,
            // so the value ends a declared length after it.
            let end = self.origin.saturating_add(header.length);
            self.skip_to(end)?;
        }
        self.origin = header.saved_origin;
        // The same mirror as `finish_pid`: only a framing that wrote
        // the padding has it to skip, and the next header aligns
        // itself.
        if self.pl_cdr1.pad_after_value {
            return self.align_to(4);
        }
        Ok(())
    }

    /// The declared length against what the configured framing would
    /// have declared for the value that was just read.
    ///
    /// `overran` in the probe already refuses a value longer than the
    /// header allows, which is the malformed case. This is the other
    /// one: a length that is well formed and belongs to a different
    /// implementation. `short c` of `vendor_bytes.md`'s `ext_mutable`
    /// is the whole difference — two bytes of value, declared as four
    /// by Cyclone and RTI and as two by Fast DDS — so a member whose
    /// value already fills its four-byte slot reveals nothing and is
    /// silently accepted either way. That is a property of the bytes
    /// and not a gap here.
    fn check_declared_length(&self, header: &PidRead) -> Result<(), WireError> {
        // `origin` is still the parameter value's own start: the caller
        // restores it after this returns.
        let read = self.at.saturating_sub(self.origin);
        let padded = read.next_multiple_of(4);
        let pads = self.pl_cdr1.pads_declared_length(header.role);
        let expected = if pads { padded } else { read };
        if header.length == expected {
            return Ok(());
        }
        // §7.4.1.2: the length is the serialized member's own; DDSI-RTPS
        // 2.5 §9.4.2.11 rounds it up to four. A tolerant reader takes
        // either, and only those two.
        if self.pl_cdr1_reading.is_tolerant() && (header.length == read || header.length == padded)
        {
            return Ok(());
        }
        // The other convention, and only that one: a length matching
        // neither is a malformed stream and gets the plain error.
        let other = if pads { read } else { padded };
        if header.length != other {
            return Err(WireError::OutOfRange {
                at: self.at,
                expected: alloc::format!(
                    "a parameter length of {expected} for the {read} byte(s) that followed it"
                ),
            });
        }
        Err(WireError::WrongFraming {
            at: self.at,
            expected: alloc::string::String::from(if pads {
                "a parameter length rounded up to four (DDSI-RTPS 2.5 §9.4.2.11)"
            } else {
                "a parameter length that stops at the value (XTypes 1.3 §7.4.1.2)"
            }),
            found: alloc::format!("{} for {read} byte(s) of value", header.length),
            fits: Some(if pads {
                "--pl-cdr1-headers specification or short-true-length"
            } else {
                "--pl-cdr1-headers short-rounded-length or always-extended"
            }),
        })
    }

    /// A must-understand bit that is not the one the framing calls for.
    fn wrong_framing(&self, at: usize, what: &str, expected: u16, found: u16) -> WireError {
        // Named rather than printed as a number: `0x4000` and `0xc000`
        // are the two flags of §7.4.1.2, and a reader who has to look
        // them up has been told a hexadecimal literal rather than what
        // is wrong.
        let names = |flags: u16| match (
            flags & PID_MUST_UNDERSTAND != 0,
            flags & PID_IMPL_EXTENSION != 0,
        ) {
            (false, false) => "no flags",
            (true, false) => "the must-understand bit",
            (false, true) => "the implementation-extension bit",
            (true, true) => "the must-understand and implementation-extension bits",
        };
        WireError::WrongFraming {
            at,
            expected: alloc::format!("{what} with {}", names(expected)),
            found: alloc::format!("it carries {}", names(found)),
            // Both positions are `PidFlagPolicy`'s now, so the flag that
            // fits is `--pid-flags` and not the framing form's.
            fits: Some(
                match (
                    found & PID_MUST_UNDERSTAND != 0,
                    found & PID_IMPL_EXTENSION != 0,
                    what,
                ) {
                    (_, true, _) => "--pid-flags impl-extension-members",
                    (true, false, "PID_EXTENDED") => "--pid-flags sentinel-must-understand",
                    (false, false, "PID_EXTENDED") => "--pid-flags bare",
                    (true, false, _) => "--pid-flags sentinel-must-understand",
                    (false, false, _) => "--pid-flags bare or impl-extension-members",
                },
            ),
        }
    }

    // -- collections -----------------------------------------------------

    /// A sequence: a count, then each element.
    ///
    /// # Errors
    /// [`WireError::ImpossibleLength`] for a count larger than the
    /// stream can hold — a count is one element or more, so a stream
    /// with fewer bytes than elements cannot be honest about it.
    pub fn read_sequence<T, F: Fn(&mut Self) -> Result<T, WireError>>(
        &mut self,
        element: F,
    ) -> Result<Vec<T>, WireError> {
        self.align_to(4)?;
        let at = self.at;
        let count = self.read_u32()? as usize;
        if count > self.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: count,
                available: self.remaining(),
            });
        }
        // No `with_capacity` on the claimed count. The count is checked
        // against the bytes left, which bounds it — but an element is
        // not a byte, and `Vec::<String>::with_capacity(n)` on a count
        // that is honest about bytes reserves twenty-four times the
        // message. Growing as elements arrive never exceeds twice what
        // was actually read.
        let mut out = Vec::new();
        for _ in 0..count {
            out.push(element(self)?);
        }
        Ok(out)
    }

    /// An array of `count` elements, with no count on the wire.
    ///
    /// # Errors
    /// Whatever the element reader returns.
    pub fn read_array<T, F: Fn(&mut Self) -> Result<T, WireError>>(
        &mut self,
        element: F,
        count: usize,
    ) -> Result<Vec<T>, WireError> {
        let mut out = Vec::with_capacity(count.min(self.remaining()));
        for _ in 0..count {
            out.push(element(self)?);
        }
        Ok(out)
    }

    /// A sequence behind a delimiter. XTypes 1.3 §7.4.3.5 rule (12).
    ///
    /// The delimiter is read and discarded: it exists so a reader can
    /// skip a collection it cannot decode, and this reader decodes it.
    ///
    /// # Errors
    /// As [`Reader::read_sequence`], plus a short buffer at the
    /// delimiter.
    pub fn read_delimited_sequence<T, F: Fn(&mut Self) -> Result<T, WireError>>(
        &mut self,
        element: F,
    ) -> Result<Vec<T>, WireError> {
        self.within_delimiter(|inner| inner.read_sequence(element))
    }

    /// An array behind a delimiter. Rule (9), the array counterpart.
    ///
    /// # Errors
    /// As [`Reader::read_array`], plus a short buffer at the delimiter.
    pub fn read_delimited_array<T, F: Fn(&mut Self) -> Result<T, WireError>>(
        &mut self,
        element: F,
        count: usize,
    ) -> Result<Vec<T>, WireError> {
        self.within_delimiter(|inner| inner.read_array(element, count))
    }

    /// A map: an entry count, then each entry's key followed by its
    /// value.
    ///
    /// The count is entries and not halves — see
    /// [`crate::Writer::write_map`]
    /// for the clause and the recorded foreign measurement it is read
    /// against.
    ///
    /// # A repeated key is refused
    ///
    /// [`BTreeMap::insert`] would answer a second entry under a key it
    /// already holds by returning the first and keeping the second: the
    /// sample would decode, report success, and hold fewer entries than
    /// the count on the wire said. That is the silent half of a bad
    /// decode, so it is an error here. A `map` is not supposed to carry
    /// one, which is exactly why a stream that does is a statement about
    /// the stream.
    ///
    /// # Errors
    /// [`WireError::ImpossibleLength`] for a count larger than the
    /// stream can hold, [`WireError::OutOfRange`] for a repeated key,
    /// and whatever either half's function returns.
    pub fn read_map<K, V, FK, FV>(
        &mut self,
        key: FK,
        value: FV,
    ) -> Result<BTreeMap<K, V>, WireError>
    where
        K: Ord,
        FK: Fn(&mut Self) -> Result<K, WireError>,
        FV: Fn(&mut Self) -> Result<V, WireError>,
    {
        self.align_to(4)?;
        let at = self.at;
        let count = self.read_u32()? as usize;
        // An entry is two values and neither is empty, so a stream with
        // fewer bytes left than entries cannot be honest about its
        // count. The same check `read_sequence` makes, and for the same
        // reason: it bounds the loop before anything is allocated.
        if count > self.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: count,
                available: self.remaining(),
            });
        }
        let mut out = BTreeMap::new();
        for _ in 0..count {
            let at = self.at;
            let k = key(self)?;
            let v = value(self)?;
            if out.insert(k, v).is_some() {
                return Err(WireError::OutOfRange {
                    at,
                    expected: format!(
                        "{count} entries under distinct keys, and this one repeats a key already \
                         in the map"
                    ),
                });
            }
        }
        Ok(out)
    }

    /// A map as the sequence of its entries (XTypes 1.3 §7.5.1.3.6, §7.4.1.1.4): the entries
    /// in the order the stream carries them. Unlike [`Reader::read_map`] this does not refuse
    /// a key that repeats: the wire carries a sequence, and what a repeated key means is the
    /// application's.
    ///
    /// # Errors
    /// [`WireError::ImpossibleLength`] for a count the stream cannot hold, and whatever either
    /// half's function returns.
    pub fn read_map_entries<K, V, FK, FV>(
        &mut self,
        key: FK,
        value: FV,
    ) -> Result<Vec<crate::MapEntry<K, V>>, WireError>
    where
        FK: Fn(&mut Self) -> Result<K, WireError>,
        FV: Fn(&mut Self) -> Result<V, WireError>,
    {
        self.align_to(4)?;
        let at = self.at;
        let count = self.read_u32()? as usize;
        if count > self.remaining() {
            return Err(WireError::ImpossibleLength {
                at,
                claimed: count,
                available: self.remaining(),
            });
        }
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let k = key(self)?;
            let v = value(self)?;
            out.push(crate::MapEntry::new(k, v));
        }
        Ok(out)
    }

    /// The entries of a map behind a delimiter. Rule (15).
    ///
    /// # Errors
    /// As [`Reader::read_map_entries`], plus a short buffer at the delimiter.
    pub fn read_delimited_map_entries<K, V, FK, FV>(
        &mut self,
        key: FK,
        value: FV,
    ) -> Result<Vec<crate::MapEntry<K, V>>, WireError>
    where
        FK: Fn(&mut Self) -> Result<K, WireError>,
        FV: Fn(&mut Self) -> Result<V, WireError>,
    {
        self.within_delimiter(|inner| inner.read_map_entries(key, value))
    }

    /// A map behind a delimiter. XTypes 1.3 §7.4.3.5 rule (15).
    ///
    /// The delimiter is read and discarded, as
    /// [`Reader::read_delimited_sequence`] does.
    ///
    /// # Errors
    /// As [`Reader::read_map`], plus a short buffer at the delimiter.
    pub fn read_delimited_map<K, V, FK, FV>(
        &mut self,
        key: FK,
        value: FV,
    ) -> Result<BTreeMap<K, V>, WireError>
    where
        K: Ord,
        FK: Fn(&mut Self) -> Result<K, WireError>,
        FV: Fn(&mut Self) -> Result<V, WireError>,
    {
        self.within_delimiter(|inner| inner.read_map(key, value))
    }

    /// An array of a length the *type* fixes, as `[T; N]`.
    ///
    /// The count is in the type — XTypes 1.3 §7.4.3.5 — so a generated
    /// field for `long grid[2][3]` is `[[i32; 3]; 2]` and not a `Vec`.
    /// [`Reader::read_array`] hands back a `Vec`, which is the right
    /// answer for a caller that wants one and does not fit that field:
    /// a `Vec` of the wrong length assigns to nothing, and one of the
    /// right length still has to be converted. `N` is inferred from the
    /// destination, so the generated call carries no turbofish.
    ///
    /// # Errors
    /// As [`Reader::read_array`], plus [`WireError::OutOfRange`] if the
    /// elements read do not number `N` — which cannot happen given the
    /// loop below, and is returned rather than unwrapped because a
    /// panic in a decoder is a denial of service a peer can send.
    pub fn read_array_n<T, const N: usize, F: Fn(&mut Self) -> Result<T, WireError>>(
        &mut self,
        element: F,
    ) -> Result<[T; N], WireError> {
        let at = self.at;
        let mut out = Vec::with_capacity(N.min(self.remaining().saturating_add(1)));
        for _ in 0..N {
            out.push(element(self)?);
        }
        out.try_into().map_err(|_| WireError::OutOfRange {
            at,
            expected: alloc::format!("an array of {N} elements"),
        })
    }

    /// A fixed-length array behind a delimiter. Rule (9).
    ///
    /// # Errors
    /// As [`Reader::read_array_n`], plus a short buffer at the
    /// delimiter.
    pub fn read_delimited_array_n<T, const N: usize, F: Fn(&mut Self) -> Result<T, WireError>>(
        &mut self,
        element: F,
    ) -> Result<[T; N], WireError> {
        self.within_delimiter(|inner| inner.read_array_n(element))
    }

    /// Read a delimited value, held to the extent the DHEADER declared.
    ///
    /// The delimiter was read and thrown away. A collection whose
    /// elements ran past it went on consuming the *next* member's bytes
    /// and returned them as its own — a decode that succeeds and is
    /// wrong, which is the harder half of the silent-drop class. One
    /// that stopped short left bytes inside its own extent unread, and
    /// said nothing about those either.
    ///
    /// The inner reader sees the stream truncated at the extent, so it
    /// cannot reach past it, and the position has to land exactly on the
    /// end. Alignment is counted from the stream's origin and not from
    /// the delimiter, so the inner reader keeps this one's offset:
    /// XTypes 1.3 §7.4.1 aligns from the encapsulation.
    ///
    /// # Errors
    /// [`WireError::ImpossibleLength`] for a delimiter longer than the
    /// stream, [`WireError::OutOfRange`] where the value did not fill
    /// its extent, plus whatever the value's own read returned.
    fn within_delimiter<T, F: FnOnce(&mut Self) -> Result<T, WireError>>(
        &mut self,
        read: F,
    ) -> Result<T, WireError> {
        let length = self.read_dheader()?;
        let end = self
            .at
            .checked_add(length)
            .ok_or(WireError::ImpossibleLength {
                at: self.at,
                claimed: length,
                available: self.remaining(),
            })?;
        let mut inner = Self {
            bytes: self.bytes.get(..end).ok_or(WireError::ImpossibleLength {
                at: self.at,
                claimed: length,
                available: self.remaining(),
            })?,
            at: self.at,
            endianness: self.endianness,
            origin: self.origin,
            max_align: self.max_align,
            pl_cdr1: self.pl_cdr1,
            wchar_width: self.wchar_width,
            wchar_framing: self.wchar_framing,
            wstring_unit_order: self.wstring_unit_order,
            pl_cdr1_reading: self.pl_cdr1_reading,
        };
        let value = read(&mut inner)?;
        if inner.at != end {
            return Err(WireError::OutOfRange {
                at: inner.at,
                expected: alloc::format!("a value filling the {length} bytes its DHEADER declared"),
            });
        }
        self.at = end;
        Ok(value)
    }

    /// The next four bytes as a word, without moving.
    ///
    /// The one place a header has to be inspected and left where it is.
    fn peek_u32(&self) -> Result<u32, WireError> {
        let end = self.at.checked_add(4).ok_or(WireError::Truncated {
            at: self.at,
            needed: 4,
            available: self.remaining(),
        })?;
        let bytes: [u8; 4] = self
            .bytes
            .get(self.at..end)
            .and_then(|slice| slice.try_into().ok())
            .ok_or(WireError::Truncated {
                at: self.at,
                needed: 4,
                available: self.remaining(),
            })?;
        Ok(if self.endianness.is_little() {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    }

    /// `count` bytes as they lie, with no alignment and no
    /// interpretation.
    ///
    /// The opaque payloads have no other reader: a GIOP request body is
    /// a byte string whose type the receiving skeleton knows and this
    /// reader does not, a CDR encapsulation carries its own byte-order
    /// octet and has to be read by a second reader, and an octet
    /// sequence is its length followed by exactly this.
    ///
    /// # Errors
    /// [`WireError::Truncated`] when fewer than `count` bytes remain.
    /// The offset reported is where the read began.
    pub fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], WireError> {
        self.take(count)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], WireError> {
        let end = self.at.checked_add(count).ok_or(WireError::Truncated {
            at: self.at,
            needed: count,
            available: self.remaining(),
        })?;
        let slice = self.bytes.get(self.at..end).ok_or(WireError::Truncated {
            at: self.at,
            needed: count,
            available: self.remaining(),
        })?;
        self.at = end;
        Ok(slice)
    }
}

/// An EMHEADER as read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmHeaderRead {
    /// The member id.
    pub member_id: u32,
    /// The length code.
    pub length_code: u8,
    /// Whether a reader must fail rather than skip this member.
    pub must_understand: bool,
    /// The NEXTINT, for the codes that carry one. Consumed for code 4
    /// and left in place for codes 5 to 7, where it is also the
    /// member's own first word.
    pub next_int: Option<usize>,
    /// How many bytes the member occupies from here.
    pub extent: usize,
}

/// A parameter header as read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PidRead {
    /// The parameter id, with the flag bits removed.
    pub pid: u32,
    /// The value's unpadded length.
    pub length: usize,
    /// Whether this is the list's end.
    pub end: bool,
    /// Whether the writer set the must-understand flag. Cyclone sets it
    /// on every parameter and Fast DDS on none; both are streams this
    /// project reads.
    pub must_understand: bool,
    /// Whether the writer set `FLAG_IMPL_EXTENSION`.
    ///
    /// XTypes 1.3 §7.4.1.2.2: the parameter is the writing
    /// implementation's own, and a user-defined member "will never set
    /// the FLAG_IMPL_EXTENSION bit". The flag used to be masked off with
    /// the rest, which made a vendor-private parameter indistinguishable
    /// from the user member sharing its low fourteen bits — and the
    /// vendor's payload was then read into that member with no error.
    pub impl_extension: bool,
    /// Whether the id is inside the reserved range, which no member has.
    ///
    /// §7.4.1.2.2 reserves 0x3F01 to 0x3FFF. `PID_EXTENDED` and
    /// `PID_LIST_END` are handled before this field is ever read; what
    /// is left is `PID_IGNORE` and the range above it, and none of them
    /// is a member id. A short parameter carrying one used to be matched
    /// against a member declared with that number through the *extended*
    /// header, where it is a legal id.
    pub reserved: bool,
    /// `PID_IGNORE`, which §7.4.1.2.2 says every consumer shall ignore.
    ///
    /// Kept apart from [`PidRead::reserved`] because it outranks
    /// [`PidRead::must_understand`]: the clause has no exception in it,
    /// so a reader that discarded the sample over the flag would refuse
    /// a stream the clause tells it to accept.
    pub ignore: bool,
    /// Which length convention this parameter is checked against.
    role: ParameterRole,
    /// The stream origin this parameter's block displaced.
    saved_origin: usize,
}

impl PidRead {
    /// Whether this parameter can be one of the reading type's members.
    ///
    /// False for a vendor-private parameter and for one whose id is in
    /// the reserved range. Both are streams a conforming writer may
    /// send, and neither is any member of the type being read, so both
    /// take the unknown-parameter path — skipped, or the sample
    /// discarded if the writer set must-understand. XTypes 1.3
    /// §7.4.1.2.2, §7.2.2.4.4.4.6.
    #[must_use]
    pub const fn is_user_member(&self) -> bool {
        !self.impl_extension && !self.reserved && !self.ignore
    }
}

/// The bit XTypes 1.3 §7.4.1.2 gives to must-understand.
const PID_MUST_UNDERSTAND: u16 = 0x4000;

/// The low fourteen bits, which are the id itself.
const PID_ID_MASK: u16 = 0x3FFF;

/// Take the byte-order mark off the front and apply what it said.
///
/// The units arrived little-endian, because that is what omniORB writes
/// on this host and what [`WstringUnitOrder::ByteOrderMark`] therefore
/// reads. Three cases, and the third is the one that matters:
///
/// - `FEFF` — the mark is the right way up, the units are as read.
/// - `FFFE` — the mark is byte-swapped, so every unit behind it is too,
///   and each one is swapped back. This is what a conforming big-endian
///   omniORB peer produces and what a reader that ignored the mark
///   would return as different characters, well formed and the right
///   length.
/// - no mark — CORBA 3.3 part 2 §9.3.1.6's rule for a GIOP 1.2
///   `wstring` without one is UTF-16 big-endian, so every unit read
///   little-endian is swapped. That rule is what makes this variant read
///   a JacORB peer as well as an omniORB one.
///
/// The mark is never a character of the returned string in any of the
/// three, which is the half §7.4.1.1.2 cares about.
///
/// The stream's own byte order is not an argument and does not need to
/// be: reading the units little-endian and then obeying the mark is
/// correct in all four combinations of writer host and stream order,
/// because the mark is written in the same order as the content it
/// introduces. That is what a byte-order mark is for.
fn strip_byte_order_mark(units: &mut Vec<u16>) {
    match units.first().copied() {
        Some(BYTE_ORDER_MARK) => {
            units.remove(0);
        }
        Some(SWAPPED_BYTE_ORDER_MARK) => {
            units.remove(0);
            for unit in units.iter_mut() {
                *unit = unit.swap_bytes();
            }
        }
        _ => {
            for unit in units.iter_mut() {
                *unit = unit.swap_bytes();
            }
        }
    }
}

/// A length the configured cell cannot have produced.
///
/// [`WireError::WrongFraming`] and not [`WireError::OutOfRange`], for
/// the reason that variant exists: a reader given the wrong dialect and
/// a reader given a corrupt stream both stop, and only one of them is
/// fixed by changing a flag. The error names the cell the length *does*
/// fit and the setting that selects it — telling a user "an odd octet
/// count" and nothing else costs an afternoon to place.
///
/// Sweeps [`WstringLengthPolicy::ALL_CELLS`] and not the measured five:
/// a cell left out of the sweep is a cell this diagnostic can never
/// name, and naming the cell the writer used is the whole point of it.
fn wstring_form_mismatch(policy: WstringLengthPolicy, declared: usize, at: usize) -> WireError {
    let mut names = String::new();
    let mut first = None;
    for other in WstringLengthPolicy::ALL_CELLS {
        if other == policy || !other.admits_length(declared) {
            continue;
        }
        if first.is_none() {
            first = Some(other.flag());
        } else {
            names.push_str(" or ");
        }
        names.push_str(other.as_str());
    }
    let found = if first.is_none() {
        format!("{declared}, which fits no other wstring length policy either")
    } else {
        format!("{declared}, which fits {names}")
    };
    WireError::WrongFraming {
        at,
        expected: format!(
            "a wstring length the {} policy can produce",
            policy.as_str()
        ),
        found,
        fits: first,
    }
}
