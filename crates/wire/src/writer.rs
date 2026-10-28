// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Putting bytes on the wire.
//!
//! Alignment is relative to the start of the stream, not to the start of
//! the value: XCDR aligns each primitive to its own size counted from
//! the encapsulation, which is why a `long` after a `char` sits at 4 and
//! not at 1. DDS-XTypes 1.3 §7.4.1.1.

use crate::{Endianness, WireError};
use alloc::format;
use alloc::vec::Vec;
use zerodds_float128::Float128;

/// How a PL_CDR1 parameter list is framed.
///
/// The fields below are the byte-level ways the measured
/// implementations differ inside a `@mutable` XCDR1 sample. Which header
/// *form* each member takes is not one of them: the plan chooses it, and
/// [`Writer::finish_pid`] overrides the plan in the one case the plan
/// cannot see — a value that turns out to overrun the short form's
/// sixteen-bit length. Set this from
/// `zerodds_ast_layout::PlCdr1HeaderPolicy`, whose documentation
/// carries the measurement, rather than assembling it by hand.
///
/// A writer that framed one way while its plan assumed another would
/// produce bytes no implementation writes, so the default here is the
/// same dialect as the layout key's: `Dialect::Specification`, the
/// clause-by-clause reading. [`PlCdr1Framing::DEFAULT`] carries the
/// four values and names where each comes from, because this crate has
/// no dependency on `zerodds-ast-layout` and cannot derive them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlCdr1Framing {
    /// What flags a parameter id carries at a position — a member's id,
    /// `PID_LIST_END`, and the `PID_EXTENDED` marker.
    ///
    /// One setting for all three, asked per [`WirePosition`], because no
    /// position's answer can be read off another's: OpenDDS sets both
    /// flags on every member and writes a bare `0x3f02`, while Cyclone
    /// and RTI leave the member bare and write `0x7f02`; and at the
    /// marker Cyclone and RTI agree (`7f01`) across two different
    /// framing forms while Fast DDS and OpenDDS share a framing form and
    /// write `3f01` against `ff01`.
    pub pid_flags: PidFlagPolicy,
    /// Whether a `@mutable` member's declared length covers the padding
    /// that follows its value to the next four-byte boundary.
    pub pad_declared_length: bool,
    /// The same question for an `@optional` member of a type that is not
    /// `@mutable`, which is a separate axis because one implementation
    /// answers the two differently.
    ///
    /// Measured 2026-08-09: Cyclone declares `04000000` for the two-byte
    /// `short c` of `ext_mutable` and `01000000` for the one-byte
    /// `@optional octet b` of `opt_final_present`. RTI pads both, Fast
    /// DDS pads neither. One boolean for both would make
    /// `Dialect::Cyclone` wrong about one of them whichever way it was
    /// set.
    pub pad_declared_optional_length: bool,
    /// Whether a member's own must-understand bit goes out.
    ///
    /// §7.4.1.2.2 makes it unconditional and two of the three
    /// implementations write it; Fast DDS writes the bare id. Set from
    /// `PlCdr1HeaderPolicy::member_must_understand`, which carries the
    /// measurement.
    pub member_must_understand: bool,
    /// The highest member id a short parameter header carries.
    ///
    /// §7.4.1.2.2 stops at `0x3F00` inclusive and reserves the rest of
    /// the low fourteen bits; one implementation carries all fourteen
    /// bits, so a reader that took the clause's threshold for a fact
    /// would report `@id(16383)` as a reserved parameter and drop the
    /// member. The byte-layer half of
    /// `zerodds_ast_layout::ShortPidRange`.
    ///
    /// Both sides. The layout plan chooses the form, because it changes
    /// a member's extent; [`Writer::write_pid`] reads this to refuse an
    /// id the short header cannot carry, which is the floor under a
    /// caller whose plan did not choose.
    pub highest_short_id: u16,
    /// Whether every parameter is expected in the extended form.
    ///
    /// Read-side only. The writer takes the form from the layout plan,
    /// because it changes a member's extent and the plan is where
    /// extents are decided; the reader has no plan for a stream it did
    /// not write, so it needs telling. Both come from the same
    /// `PlCdr1HeaderPolicy`, which is what keeps them agreeing.
    pub require_extended: bool,
    /// Whether the parameter that just ended writes its own trailing
    /// padding to the next four-byte boundary.
    ///
    /// Every header writer aligns itself, so the two readings only
    /// part at the end of the stream: RTI pads after a final
    /// `@optional` parameter, Fast DDS does not. Set from
    /// `PlCdr1HeaderPolicy::pads_after_value`.
    pub pad_after_value: bool,
}

/// Whether a PL_CDR1 reader takes a stream framed another way than it was
/// configured for. The byte-layer half of
/// `zerodds_ast_layout::PlCdr1Reading`, which carries the reasoning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlCdr1Reading {
    /// The configured framing is read; another framing's flags, header
    /// form and length convention are reported as
    /// [`crate::WireError::WrongFraming`].
    #[default]
    Strict,
    /// Any flags on `PID_EXTENDED` and `PID_LIST_END`, either header form
    /// and either length convention are read.
    Tolerant,
}

impl PlCdr1Reading {
    /// Whether this is [`Self::Tolerant`].
    #[must_use]
    pub const fn is_tolerant(self) -> bool {
        matches!(self, Self::Tolerant)
    }
}

impl PlCdr1Framing {
    /// The clause's framing, which is this project's default dialect.
    ///
    /// A `const` as well as a `Default` because both the writer's and
    /// the reader's `const fn` constructors need it, and a `Default`
    /// impl cannot be called from one.
    ///
    /// It was Cyclone's while `Dialect::Cyclone` was the default, and
    /// two of the six moved with the default: `require_extended`, which
    /// is `PlCdr1HeaderPolicy::AlwaysExtended` in Cyclone and the short
    /// header wherever §7.4.1.2.2 allows it in the clause; and
    /// `pad_declared_optional_length`, which Cyclone answers `false` and
    /// the clause `true`.
    /// `zerodds_wire_probe::value`'s
    /// `the_default_framing_is_the_default_dialects` holds all six
    /// against `zerodds_ast_layout::Dialect::default`. It is there and
    /// not here because this crate depends on neither the layout crate
    /// nor the probe, and a constant nothing compares is a constant that
    /// drifts.
    pub const DEFAULT: Self = Self {
        pid_flags: PidFlagPolicy::SentinelMustUnderstand,
        highest_short_id: PID_SHORT_MAX_ID,
        pad_declared_length: false,
        pad_declared_optional_length: false,
        member_must_understand: true,
        require_extended: false,
        pad_after_value: true,
    };

    /// Whether a parameter in this role declares a padded length.
    #[must_use]
    pub const fn pads_declared_length(self, role: ParameterRole) -> bool {
        match role {
            ParameterRole::Member => self.pad_declared_length,
            ParameterRole::Optional => self.pad_declared_optional_length,
        }
    }
}

/// How many bytes one `Char16` occupies on the wire.
///
/// XTypes 1.3 §7.4.1.1.1, Table 31 says two, and two of the three
/// measured implementations write two everywhere. RTI Connext 7.7 writes
/// **four** under XCDR1 — measured 2026-08-09 with
/// `@final struct WCh { wchar c; octet tail; }`, `c = U+0061`: RTI's
/// XCDR1 payload is `61000000 ab` little endian and `00000061 ab` big
/// endian, against `6100 ab` / `0061 ab` from Cyclone, Fast DDS and
/// RTI's own XCDR2. It is the same character width RTI's XCDR1 `wstring`
/// uses, which is why the two are one setting and not two —
/// `zerodds_ast_layout::WstringLengthPolicy::character_width` is where
/// the width is chosen, and this enum is how it reaches the byte layer.
///
/// A width is not a formatting difference. A `wchar` read two bytes wide
/// out of a four-byte field returns the wrong character *and* leaves
/// every member after it two bytes out of place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WcharWidth {
    /// Two bytes, one UTF-16 code unit. Table 31, and what three of the
    /// four measured (vendor, encoding version) pairs write.
    #[default]
    Two,
    /// Four bytes, one UTF-16 code unit zero-extended. RTI Connext under
    /// XCDR1.
    Four,
}

impl WcharWidth {
    /// The width in bytes, which is also the alignment under
    /// [`WcharFraming::Fixed`]: a `Char16` is a primitive and aligns to
    /// its own size, capped by `MAXALIGN`. Under
    /// [`WcharFraming::GiopLengthPrefixed`] it is the count in the
    /// leading octet and the alignment is one.
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

/// How many BCD digits a `fixed<P, S>` puts on the wire.
///
/// CORBA 3.3 part 2 §9.3.2.8 packs a `fixed` as BCD nibbles followed by a
/// sign nibble, and leaves the digit count to the implementation. Two answers
/// exist in the field and both are self-consistent, so this is a dialect axis
/// and not a defect: omniORB writes the declared `P` whatever the value is,
/// while JacORB and ACE/TAO write as many digits as the value has.
///
/// The same two values live in `zerodds_ast_layout::key::FixedDigitCount`,
/// which is where a plan chooses between them. This copy exists because a
/// `Writer` must not depend on the layout planner — the same reason
/// [`WcharFraming`] sits here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FixedDigits {
    /// `P` digits, whatever the value is. omniORB 4.3.3.
    #[default]
    Declared,
    /// As many digits as the value has, never fewer than one. JacORB 3.9.
    ValuePrecision,
    /// The integer digits of the decimal as written, leading zeros
    /// included, plus the `S` fraction digits, padded to an odd digit
    /// count by adding one leading zero if the count is even. ACE/TAO
    /// 2.5.24's `ACE_CDR::Fixed` for a decimal with `S` fraction digits,
    /// through its own CDR operator. Measured in
    /// `unspecified_constructs.md` §3. OpenDDS writes the declared count.
    ValueStringDigits,
}

impl FixedDigits {
    /// How many digit nibbles a value goes out with.
    ///
    /// `declared` is `P`; `significant` is the value's digit count with
    /// its leading zeros removed; `string_digits` is the digit count of
    /// the input string. The byte-layer half of
    /// `zerodds_ast_layout::key::FixedDigitCount::wire_digits`, and
    /// `crates/ast-layout/tests/fixed_digits_agree.rs` holds the two halves
    /// against each other over every argument either can take — a copy that
    /// nothing compares is a copy that drifts.
    ///
    /// Never zero under any value: a field of no digits could not carry
    /// the sign nibble's pair.
    #[must_use]
    pub const fn wire_digits(self, declared: u8, significant: u8, string_digits: u8) -> u8 {
        match self {
            Self::Declared => declared,
            Self::ValuePrecision => {
                if significant == 0 {
                    1
                } else {
                    significant
                }
            }
            Self::ValueStringDigits => {
                // ACE rule: if the string's digit count is even, add one
                // leading zero to make it odd. The sign nibble then makes
                // the total nibble count even (whole octets).
                if string_digits == 0 {
                    1
                } else if string_digits % 2 == 0 {
                    string_digits + 1
                } else {
                    string_digits
                }
            }
        }
    }
}

/// How a `wchar` is framed.
///
/// The byte-layer half of
/// `zerodds_ast_layout::WcharFraming`, which carries the run: all five
/// DDS implementations write XTypes 1.3 Table 31's fixed-width code
/// unit, and omniORB 4.3.3, TAO 2.5.24 and JacORB 3.9 all write CORBA
/// 3.3 part 2 §9.3.1.6's GIOP 1.2 form instead — an octet saying how
/// many octets follow, then that many, aligned to one.
///
/// It is not a width. A `wchar` that carries its own count is one octet
/// longer *and* aligns to one, so every member behind it moves; a reader
/// that had the width right and the framing wrong would take the count
/// octet for half a character and return a different one.
///
/// It is not a property of the type either. TAO writes the count octet
/// for a `wchar` **member** and a bare unit for a `wchar` inside an
/// array or a sequence, so the framing is answered per
/// [`WcharPosition`] and every entry point that writes or reads one
/// takes the position as an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WcharFraming {
    /// A fixed-width code unit, aligned to its own width. Table 31.
    #[default]
    Fixed,
    /// One octet of length, then that many octets, aligned to one, in
    /// every position. CORBA §9.3.1.6's GIOP 1.2 `wchar`, as omniORB
    /// and JacORB write it.
    GiopLengthPrefixed,
    /// The count octet for a member, a bare fixed-width unit for a
    /// collection element. TAO 2.5.24.
    GiopLengthPrefixedMemberOnly,
}

impl WcharFraming {
    /// Whether a `wchar` in this position carries its own octet count.
    #[must_use]
    pub const fn has_length_octet(self, position: WcharPosition) -> bool {
        match self {
            Self::Fixed => false,
            Self::GiopLengthPrefixed => true,
            Self::GiopLengthPrefixedMemberOnly => matches!(position, WcharPosition::Member),
        }
    }
}

/// Where the value being written or read sits.
///
/// The byte-layer half of `zerodds_ast_layout::WirePosition`. It is an
/// argument to [`Writer::write_wchar`],
/// [`crate::reader::Reader::read_wchar`] and to [`PidFlagPolicy`]
/// rather than a field of any of them, for the reason [`ParameterRole`]
/// is an argument: one stream carries several positions at once, so a
/// setting made once at construction could not answer for all of them.
///
/// Two axes read it, and each reads a different pair of variants:
/// [`WcharFraming`] tells [`Self::Member`] from [`Self::Element`], and
/// [`PidFlagPolicy`] tells [`Self::Member`] from [`Self::Sentinel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WirePosition {
    /// A member of a structure, a union branch, or the whole value.
    ///
    /// For [`PidFlagPolicy`] this is a member's own parameter id in a
    /// PL_CDR1 list.
    #[default]
    Member,
    /// An element of an array or a sequence.
    Element,
    /// The `PID_LIST_END` that closes a PL_CDR1 parameter list.
    ///
    /// Read by [`PidFlagPolicy`], which carries the measurement that
    /// makes it a position rather than a property of the list.
    Sentinel,
    /// The `PID_EXTENDED` marker that opens a parameter's extended
    /// header.
    ///
    /// Read by [`PidFlagPolicy`]. It was a field of
    /// `zerodds_ast_layout::PlCdr1HeaderPolicy` on the argument that the
    /// marker exists only under the always-extended form; it does not —
    /// a member id above `PID_SHORT_MAX_ID` reaches it under the
    /// specification's form too, and RTI sets the bit there.
    ExtendedMarker,
}

/// The name the position enumeration shipped under, when it reached only
/// `wchar`.
///
/// The alias stays because the argument for the rename is in the rename,
/// not in breaking every caller in one commit. Its layout-crate half is
/// aliased the same way.
pub type WcharPosition = WirePosition;

/// What flags a PL_CDR1 parameter id carries, at a position.
///
/// The byte-layer half of `zerodds_ast_layout::PidFlagPolicy`, which
/// carries the measurement. XTypes 1.3 §7.4.1.2 splits the sixteen-bit
/// field: the low fourteen bits are the id, bit 14 is
/// `PID_MUST_UNDERSTAND` and bit 15 is [`PID_IMPL_EXTENSION`].
///
/// A member's *own* `must_understand` is not on this axis — it is
/// [`PlCdr1Framing::member_must_understand`], and the writer ORs the two
/// together. This one answers the part that does not depend on what the
/// member declared, which is the whole of the sentinel's answer:
/// `PID_LIST_END` is not a member and has nothing to declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PidFlagPolicy {
    /// A bare member id, and `PID_MUST_UNDERSTAND` on the sentinel.
    /// Cyclone DDS and RTI Connext 7.7.
    #[default]
    SentinelMustUnderstand,
    /// Bare everywhere. Fast DDS.
    Bare,
    /// `PID_IMPL_EXTENSION | PID_MUST_UNDERSTAND` on every member id
    /// whatever the member declared, and a bare sentinel. OpenDDS.
    ///
    /// Measured on the **short** header only: OpenDDS writes no
    /// extended header anywhere in the recorded corpus, so
    /// [`Writer::write_pid_extended`] does not claim an answer for
    /// `FLAG_1` of an `eMemberHeader`.
    ImplExtensionOnMembers,
}

impl PidFlagPolicy {
    /// The flag bits a parameter id carries at `position`.
    ///
    /// Only [`WirePosition::Member`], [`WirePosition::Sentinel`] and
    /// [`WirePosition::ExtendedMarker`] have a parameter id at all;
    /// [`WirePosition::Element`] answers zero because there is nothing
    /// there to carry a flag.
    ///
    /// The marker's answers are Cyclone and RTI `7f01`, Fast DDS `3f01`,
    /// OpenDDS `ff01` — the same three values, at a third position.
    #[must_use]
    pub const fn flags_at(self, position: WirePosition) -> u16 {
        match (self, position) {
            (
                Self::SentinelMustUnderstand,
                WirePosition::Sentinel | WirePosition::ExtendedMarker,
            ) => PID_MUST_UNDERSTAND,
            (Self::ImplExtensionOnMembers, WirePosition::Member | WirePosition::ExtendedMarker) => {
                PID_IMPL_EXTENSION | PID_MUST_UNDERSTAND
            }
            _ => 0,
        }
    }
}

/// In what byte order a `wstring`'s code units go out — **C4**.
///
/// The byte-layer half of `zerodds_ast_layout::WstringUnitOrder`, which
/// carries the runs. Read by [`Writer::write_wstring`] and
/// [`Reader::read_wstring`](crate::Reader::read_wstring) for a `wstring`'s units, and by
/// [`Writer::write_wchar`] for a scalar, where CORBA gives the mark no
/// place to ride and [`Self::ByteOrderMark`] therefore degrades to
/// big-endian.
///
/// A fourth axis and not a fourth coordinate of
/// [`WstringLengthPolicy`]: it crosses the length convention rather
/// than extending it. omniORB's mark rides on an octet count and
/// nothing stops a GIOP 1.1 character count from carrying one too, so
/// folding the order in would give eight variants for two questions —
/// which is how `UnswappedUnits`, a byte order wearing a length
/// policy's name, came to exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum WstringUnitOrder {
    /// The stream's own byte order. XTypes §7.4.1.1.2 through `ESWAP`.
    #[default]
    Stream,
    /// UTF-16 big-endian whatever the stream says. What omniORB and
    /// JacORB decode in the absence of a byte-order mark.
    BigEndianAlways,
    /// A byte-order mark, then the units, both little-endian. omniORB.
    ByteOrderMark,
    /// The units always little-endian, no mark. Cyclone's big-endian
    /// output, and `rosidl_typesupport_fastrtps`'s over four-octet
    /// units. Arrived as a value of the length policy — a byte order
    /// wearing a length's name — and belongs on this axis.
    HostUnswapped,
}

impl WstringUnitOrder {
    /// Whether a byte-order mark is written, and expected on read.
    ///
    /// The mark is a code unit like any other and its octets are
    /// counted in the length: omniORB writes `0a000000 fffe 6100 34d8
    /// 1edd 6200` for `"a𝄞b"`, ten octets over five units.
    /// §7.4.1.1.2 forbids it outright — "shall not include a Byte Order
    /// Mark" — so no DDS dialect selects it.
    #[must_use]
    pub const fn has_mark(self) -> bool {
        matches!(self, Self::ByteOrderMark)
    }

    /// Whether one code unit goes out little-endian whatever the
    /// stream's order is.
    ///
    /// Takes the position for the reason [`WstringLengthPolicy`]'s
    /// accessors do. Measured to agree at the member and the collection
    /// element in all nine implementations.
    #[must_use]
    pub const fn writes_little_endian_units(self, _position: WirePosition) -> bool {
        matches!(self, Self::ByteOrderMark | Self::HostUnswapped)
    }
}

/// **C1** — what a `wstring`'s length prefix counts.
///
/// The byte-layer half of `zerodds_ast_layout::WstringLengthCounts`,
/// which carries the runs and the argument. Declared again here rather
/// than imported for the reason [`PlCdr1Framing`] is declared again:
/// this crate depends on nothing and is not going to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum WstringLengthCounts {
    /// The number of octets the code units occupy.
    ///
    /// XTypes 1.3 §7.4.1.1.2, and CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2
    /// `wstring`. Cyclone, OpenDDS, omniORB, TAO and JacORB.
    #[default]
    Octets,
    /// The number of code units, whatever each one weighs.
    ///
    /// CORBA §9.3.1.6's GIOP 1.1 `wstring`, and Fast CDR's `wcslen`.
    /// RTI under XCDR1, Fast DDS, and both ROS type supports.
    Characters,
}

/// **C2** — whether a terminating NUL code unit is written.
///
/// It is written *and* counted or neither: no measured implementation
/// writes a terminator the length does not cover, which is why this is
/// one question and not two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum WstringTerminator {
    /// No terminator. Eight of the nine measured implementations.
    #[default]
    Absent,
    /// A NUL code unit at the end, inside the length. CORBA §9.3.1.6's
    /// GIOP 1.1 rule, and RTI under XCDR1 alone among the nine.
    Present,
}

/// **C3** — how wide one UTF-16 code unit is on the wire.
///
/// The transmission code set's width. No GIOP version fixes it and
/// XTypes Table 31 fixes it at two, so it varies independently of C1
/// and C2 — which is why these are three types and not one enumeration
/// of named forms.
///
/// Not [`WcharWidth`], and the difference is measured:
/// `rmw_cyclonedds_cpp` 1.3.4 writes a two-octet scalar `wchar` and
/// four-octet `wstring` code units, so the scalar's width is its own
/// axis. See `zerodds_ast_layout::WcharWidth`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum WstringUnitWidth {
    /// Two octets. XTypes Table 31, Cyclone, OpenDDS, Fast DDS,
    /// omniORB, JacORB.
    #[default]
    Two,
    /// Four octets, a code unit zero-extended — **not** a code point,
    /// so `U+1D11E` stays the surrogate pair `34d80000 1edd0000`. RTI
    /// under XCDR1, TAO, and both ROS type supports.
    Four,
}

impl WstringUnitWidth {
    /// The width in octets.
    #[must_use]
    pub const fn octets(self) -> usize {
        match self {
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

/// How a `wstring`'s length is written and how wide its code units are
/// — **C1 × C2 × C3**, a product space and not a list.
///
/// The byte-layer half of `zerodds_ast_layout::WstringLengthPolicy`,
/// which carries the measurements. Eight cells, of which five have a
/// name because an implementation was measured writing them:
///
/// | implementation | C1 counts | C2 terminator | C3 unit |
/// |---|---|---|--:|
/// | Cyclone, OpenDDS, RTI XCDR2, omniORB, JacORB | octets | absent | 2 |
/// | Fast DDS | characters | absent | 2 |
/// | RTI, XCDR1 | characters | **present** | 4 |
/// | TAO | octets | absent | 4 |
/// | ros-cyclone, ros-fastrtps | characters | absent | **4** |
///
/// The other three cells are as constructible and as nameable as the
/// five: an enumeration standing in for a product space is what left
/// the ROS cell — characters, no terminator, four-octet units — with no
/// name for as long as the names were the values, and two
/// implementations writing a form this project could not express.
///
/// The **order** the units go out in is not here. It is
/// [`WstringUnitOrder`], a fourth axis, because it crosses the length
/// convention rather than extending it: omniORB's mark rides on an
/// octet count and nothing stops a GIOP 1.1 count from carrying one
/// too. Two settings that both decide one byte order, with a precedence
/// rule between them, is the shape that produces a wrong character and
/// no error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WstringLengthPolicy {
    /// C1 — what the length prefix counts.
    pub counts: WstringLengthCounts,
    /// C2 — whether a terminating NUL is written and counted.
    pub terminator: WstringTerminator,
    /// C3 — how wide one code unit is.
    pub unit: WstringUnitWidth,
}

impl WstringLengthPolicy {
    /// Build a cell of the product directly.
    ///
    /// Public and `const` so that a cell nobody has named is as easy to
    /// reach as one that is named. The named constants below are
    /// conveniences for the cells an implementation was measured in;
    /// they are not the set of legal values.
    #[must_use]
    pub const fn new(
        counts: WstringLengthCounts,
        terminator: WstringTerminator,
        unit: WstringUnitWidth,
    ) -> Self {
        Self {
            counts,
            terminator,
            unit,
        }
    }

    /// Octets, no terminator, two-octet units. XTypes 1.3 §7.4.1.1.2
    /// and CORBA's GIOP 1.2 `wstring` over a two-octet code set. The
    /// default.
    pub const SPECIFICATION: Self = Self::new(
        WstringLengthCounts::Octets,
        WstringTerminator::Absent,
        WstringUnitWidth::Two,
    );

    /// Characters, no terminator, two-octet units. Fast DDS under both
    /// versions.
    ///
    /// [`Writer::write_wstring`] refuses a code point outside the BMP
    /// in this cell rather than truncating it the way Fast CDR does.
    pub const CHARACTER_COUNT: Self = Self::new(
        WstringLengthCounts::Characters,
        WstringTerminator::Absent,
        WstringUnitWidth::Two,
    );

    /// Characters including the terminator, four-octet units. CORBA 3.3
    /// part 2 §9.3.1.6's GIOP **1.1** `wstring`, and RTI Connext 7.7
    /// under XCDR1.
    pub const CHARACTER_COUNT_TERMINATED: Self = Self::new(
        WstringLengthCounts::Characters,
        WstringTerminator::Present,
        WstringUnitWidth::Four,
    );

    /// Octets, no terminator, four-octet units. CORBA §9.3.1.6's GIOP
    /// 1.2 `wstring` over a four-octet transmission code set — TAO
    /// 2.5.24.
    pub const SPECIFICATION_WIDE_UNITS: Self = Self::new(
        WstringLengthCounts::Octets,
        WstringTerminator::Absent,
        WstringUnitWidth::Four,
    );

    /// Characters, no terminator, four-octet units. **The cell the old
    /// enumeration could not reach.** Both ROS 2 type supports write
    /// it.
    pub const ROS_TYPE_SUPPORT: Self = Self::new(
        WstringLengthCounts::Characters,
        WstringTerminator::Absent,
        WstringUnitWidth::Four,
    );

    /// Every cell of C1 × C2 × C3, measured or not.
    ///
    /// The reader sweeps this and not the measured five when it has to
    /// name the cell a refused length *does* fit: a cell left out of the
    /// sweep is a cell the diagnostic can never name.
    pub const ALL_CELLS: [Self; 8] = [
        Self::new(
            WstringLengthCounts::Octets,
            WstringTerminator::Absent,
            WstringUnitWidth::Two,
        ),
        Self::new(
            WstringLengthCounts::Octets,
            WstringTerminator::Absent,
            WstringUnitWidth::Four,
        ),
        Self::new(
            WstringLengthCounts::Octets,
            WstringTerminator::Present,
            WstringUnitWidth::Two,
        ),
        Self::new(
            WstringLengthCounts::Octets,
            WstringTerminator::Present,
            WstringUnitWidth::Four,
        ),
        Self::new(
            WstringLengthCounts::Characters,
            WstringTerminator::Absent,
            WstringUnitWidth::Two,
        ),
        Self::new(
            WstringLengthCounts::Characters,
            WstringTerminator::Absent,
            WstringUnitWidth::Four,
        ),
        Self::new(
            WstringLengthCounts::Characters,
            WstringTerminator::Present,
            WstringUnitWidth::Two,
        ),
        Self::new(
            WstringLengthCounts::Characters,
            WstringTerminator::Present,
            WstringUnitWidth::Four,
        ),
    ];

    /// Whether this is the default cell, [`Self::SPECIFICATION`].
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(
            (self.counts, self.terminator, self.unit),
            (
                WstringLengthCounts::Octets,
                WstringTerminator::Absent,
                WstringUnitWidth::Two
            )
        )
    }

    /// The name a configuration uses, the same spelling
    /// `zerodds_ast_layout::WstringLengthPolicy::as_str` produces.
    ///
    /// A cell an implementation was measured in renders under the name
    /// it shipped with; the three nobody writes render as their
    /// coordinates, `counts/terminator/width`. Every one of the eight
    /// has a name, because a value that cannot be printed cannot be
    /// reported in a diagnostic.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match (self.counts, self.terminator, self.unit) {
            (WstringLengthCounts::Octets, WstringTerminator::Absent, WstringUnitWidth::Two) => {
                "specification"
            }
            (WstringLengthCounts::Octets, WstringTerminator::Absent, WstringUnitWidth::Four) => {
                "specification-wide-units"
            }
            (WstringLengthCounts::Characters, WstringTerminator::Absent, WstringUnitWidth::Two) => {
                "character-count"
            }
            (
                WstringLengthCounts::Characters,
                WstringTerminator::Absent,
                WstringUnitWidth::Four,
            ) => "ros-type-support",
            (
                WstringLengthCounts::Characters,
                WstringTerminator::Present,
                WstringUnitWidth::Four,
            ) => "character-count-terminated",
            (WstringLengthCounts::Octets, WstringTerminator::Present, WstringUnitWidth::Two) => {
                "octets/terminated/2"
            }
            (WstringLengthCounts::Octets, WstringTerminator::Present, WstringUnitWidth::Four) => {
                "octets/terminated/4"
            }
            (
                WstringLengthCounts::Characters,
                WstringTerminator::Present,
                WstringUnitWidth::Two,
            ) => "characters/terminated/2",
        }
    }

    /// The command-line setting that selects this cell, whole.
    ///
    /// A `&'static str` and not a formatted one because
    /// [`WireError::WrongFraming`]'s `fits` is a `&'static str`: the
    /// point of that field is that a peer misconfiguration reports the
    /// flag that fixes it instead of "an odd octet count", and a field
    /// that had to allocate would not have been there on the no-`std`
    /// path.
    #[must_use]
    pub const fn flag(self) -> &'static str {
        match (self.counts, self.terminator, self.unit) {
            (WstringLengthCounts::Octets, WstringTerminator::Absent, WstringUnitWidth::Two) => {
                "--wstring-length specification"
            }
            (WstringLengthCounts::Octets, WstringTerminator::Absent, WstringUnitWidth::Four) => {
                "--wstring-length specification-wide-units"
            }
            (WstringLengthCounts::Characters, WstringTerminator::Absent, WstringUnitWidth::Two) => {
                "--wstring-length character-count"
            }
            (
                WstringLengthCounts::Characters,
                WstringTerminator::Absent,
                WstringUnitWidth::Four,
            ) => "--wstring-length ros-type-support",
            (
                WstringLengthCounts::Characters,
                WstringTerminator::Present,
                WstringUnitWidth::Four,
            ) => "--wstring-length character-count-terminated",
            (WstringLengthCounts::Octets, WstringTerminator::Present, WstringUnitWidth::Two) => {
                "--wstring-length octets/terminated/2"
            }
            (WstringLengthCounts::Octets, WstringTerminator::Present, WstringUnitWidth::Four) => {
                "--wstring-length octets/terminated/4"
            }
            (
                WstringLengthCounts::Characters,
                WstringTerminator::Present,
                WstringUnitWidth::Two,
            ) => "--wstring-length characters/terminated/2",
        }
    }

    /// **C1** at a position — what the length prefix counts.
    ///
    /// An accessor and not the field: a caller that read `policy.counts`
    /// directly would have answered for every position by not asking
    /// about any, which is exactly the shape that let a member's `wchar`
    /// framing stand in for an element's through five emitters. The nine
    /// implementations answer both positions alike, measured 2026-08-12,
    /// so the argument is unused — and it is the argument that keeps
    /// that a measurement rather than an assumption.
    #[must_use]
    pub const fn counts_at(self, _position: WirePosition) -> WstringLengthCounts {
        self.counts
    }

    /// **C2** at a position — whether a terminating NUL is written.
    #[must_use]
    pub const fn terminator_at(self, _position: WirePosition) -> WstringTerminator {
        self.terminator
    }

    /// **C3** at a position — how wide one code unit is.
    #[must_use]
    pub const fn unit_at(self, _position: WirePosition) -> WstringUnitWidth {
        self.unit
    }

    /// How many octets one code unit takes at a position.
    #[must_use]
    pub const fn character_width(self, position: WirePosition) -> usize {
        self.unit_at(position).octets()
    }

    /// The length field this cell declares for a run of `units` code
    /// units, or `None` when it does not fit the `UInt32` the field is.
    ///
    /// The run is the whole run: the text's units, plus C2's terminator
    /// and C4's mark where those are written. One expression rather than
    /// an arm per named cell, so a cell nobody has measured computes its
    /// length the same way as the five that are.
    ///
    /// `None` and not a wrapped value. A length that names a prefix of
    /// its own content is the silent class: the peer reads a well-formed
    /// shorter `wstring` and then every following member from inside
    /// this one, and nothing in the frame says so. A separate function
    /// from [`Writer::write_wstring`] so that the refusal can be tested
    /// at `usize::MAX` without a caller having to hold two gigabytes of
    /// text to reach it.
    #[must_use]
    pub const fn declared_length(self, units: usize, position: WirePosition) -> Option<u32> {
        let counted = match self.counts_at(position) {
            WstringLengthCounts::Octets => {
                match units.checked_mul(self.character_width(position)) {
                    Some(counted) => counted,
                    None => return None,
                }
            }
            WstringLengthCounts::Characters => units,
        };
        if counted > u32::MAX as usize {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        Some(counted as u32)
    }

    /// Whether `declared` is a length this cell could have written.
    ///
    /// Only the shape of the number, which is all a refused read has to
    /// go on: an octet count has to divide by the unit width, and a
    /// count that includes a terminator cannot be zero. A cell nobody
    /// has measured is checked by the same two rules.
    ///
    /// A byte count and one written under
    /// [`WstringUnitOrder::HostUnswapped`] are the same number — that
    /// point of the order axis differs in the *content*'s byte order,
    /// which no length can show — so a diagnostic that names one names
    /// both.
    #[must_use]
    pub const fn admits_length(self, declared: usize) -> bool {
        if matches!(self.counts, WstringLengthCounts::Octets) && declared % self.unit.octets() != 0
        {
            return false;
        }
        if matches!(self.terminator, WstringTerminator::Present) && declared < 1 {
            return false;
        }
        true
    }
}

/// The UTF-16 byte-order mark, `U+FEFF`.
///
/// Named because it appears on both sides of the wire and a bare
/// `0xFEFF` in a reader would read as a character of the string.
pub(crate) const BYTE_ORDER_MARK: u16 = 0xFEFF;

/// `U+FFFE`, which is not a character: a mark read this way says the
/// units behind it are the other way round.
pub(crate) const SWAPPED_BYTE_ORDER_MARK: u16 = 0xFFFE;

/// Which of §7.4.1.2's two roles a parameter header stands in.
///
/// The header is the same header — the id, the length, the short and
/// extended forms are all one clause. The role decides what the length
/// field counts, and whether a member that took no value writes a header
/// at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParameterRole {
    /// A member of a `@mutable` type. Rules (24) and (25). Absence is
    /// written by leaving the parameter out.
    #[default]
    Member,
    /// An `@optional` member of a type that is not `@mutable`. Rule (19).
    /// Absence is written as a header with a length of zero.
    Optional,
}

impl Default for PlCdr1Framing {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The must-understand bit of a parameter id. DDSI-RTPS 2.5 §9.4.2.11.2.
const PID_MUST_UNDERSTAND: u16 = 0x4000;

/// `FLAG_2` of the extended `eMemberHeader`, which is the **member's**
/// must-understand. XTypes 1.3 §7.4.1.2.1.
pub const E_FLAG_MUST_UNDERSTAND: u32 = 0x4000_0000;

/// `FLAG_1` of the extended `eMemberHeader`, which is
/// `FLAG_IMPL_EXTENSION`. XTypes 1.3 §7.4.1.2.1, and the same property
/// as [`PID_IMPL_EXTENSION`] in the short form.
pub const E_FLAG_IMPL_EXTENSION: u32 = 0x8000_0000;

/// `FLAG_IMPL_EXTENSION`, bit 15 of a short parameter id.
///
/// XTypes 1.3 §7.4.1.2.2: a parameter that sets it is the writing
/// implementation's own and user-defined members "will never set the
/// FLAG_IMPL_EXTENSION bit". A reader that masks it off takes such a
/// parameter for the user member with the same low fourteen bits and
/// reads a vendor's private payload into it.
pub const PID_IMPL_EXTENSION: u16 = 0x8000;

/// `PID_IGNORE`. XTypes 1.3 §7.4.1.2.2: "All consumers of this Data
/// Representation shall ignore parameters with this ID" — including a
/// consumer that would otherwise discard the sample over the
/// must-understand bit.
pub const PID_IGNORE: u16 = 0x3F03;

/// The low twenty-eight bits of an extended `eMemberHeader`, which are
/// the member id. The top four are `FLAG_1` to `FLAG_4`.
pub const E_MEMBER_ID_MASK: u32 = 0x0FFF_FFFF;

/// A `fixed<P, S>` value split into its parts, with every rule about what is
/// a legal value for that declaration already applied.
///
/// The digits come back trimmed of the leading zeros the declaration would
/// pad back on, because that is what both callers need and neither can
/// recover once they are gone: [`Writer::write_fixed`] counts them to answer
/// [`FixedDigits::ValuePrecision`], and `zerodds-wire-codec`'s canonical form
/// writes them out as the one spelling a sample has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedParts<'a> {
    /// Whether the value carries a `-`. A negative zero is still `true` here:
    /// [`Writer::write_fixed`] gives it the positive sign nibble because
    /// §9.3.2.8 has one zero, but that is the packing's rule and not the
    /// parse's, and the canonical form has to make the same decision itself.
    pub negative: bool,
    /// The integer digits, leading zeros removed. Empty for a value below one.
    pub whole: &'a str,
    /// The fraction digits as written, never more than `S` and possibly fewer.
    pub fraction: &'a str,
}

/// Split a `fixed<P, S>` value, refusing everything the declaration cannot
/// carry.
///
/// The one place the rule for a legal `fixed` value lives. Three call sites
/// had a copy of it before — this writer, `zerodds-wire-codec`'s encoder and
/// its canonical form — and a copy that disagreed with the others would
/// accept a value one path writes and another rejects, on the same sample.
///
/// What it refuses, and why each is a refusal and not a silent repair:
///
/// * A declaration outside IDL 4.2 rule (42): `P` of zero carries no digit
///   to pair the sign nibble with, `P` above 31 has no packing defined, and
///   an `S` wider than `P` describes a fraction with no type to hold it.
/// * A value that is not a decimal, or has no digits at all.
/// * An integer part wider than `P - S`, or a fraction wider than `S`. A
///   truncated decimal is the wrong number with nothing to notice it by,
///   which is the failure `fixed` exists to prevent.
pub fn parse_fixed(
    text: &str,
    digits: u8,
    scale: u8,
    at: usize,
) -> Result<FixedParts<'_>, WireError> {
    if digits == 0 {
        return Err(WireError::refused(at, "a fixed with at least one digit"));
    }
    // OMG IDL 4.2 rule (42) caps the BCD form at 31 digits, and `fixed<31, S>`
    // is the widest declaration IDL admits. A wider one has no packing defined,
    // so it is refused rather than packed into octets whose width no reader can
    // be told.
    if digits > 31 {
        return Err(WireError::refused(at, "a fixed of at most 31 digits"));
    }
    if scale > digits {
        return Err(WireError::refused(at, "a scale no larger than the digits"));
    }

    let (negative, magnitude) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (whole, fraction) = match magnitude.split_once('.') {
        Some((w, f)) => (w, f),
        None => (magnitude, ""),
    };
    if whole.is_empty() && fraction.is_empty() {
        return Err(WireError::refused(at, "a decimal with at least one digit"));
    }
    if !whole
        .bytes()
        .chain(fraction.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return Err(WireError::refused(at, "a decimal of digits and one point"));
    }

    // The integer part has P - S digits and the fraction has S. Neither is
    // padded silently in the direction that would lose one: a value too long
    // for its half is a value this type cannot represent.
    let whole = whole.trim_start_matches('0');
    if whole.len() > usize::from(digits - scale) {
        return Err(WireError::refused(
            at,
            "a decimal whose integer part fits the declared digits",
        ));
    }
    if fraction.len() > usize::from(scale) {
        return Err(WireError::refused(
            at,
            "a decimal whose fraction fits the declared scale",
        ));
    }
    Ok(FixedParts {
        negative,
        whole,
        fraction,
    })
}

/// The BCD nibbles of `text` under `fixed<digits, scale>`.
///
/// Split from [`Writer::write_fixed`] for the reason the same split exists in
/// `zerodds-wire-codec`: the byte packing is mechanical, and every rule about
/// what is a legal value for `fixed<P, S>` is in [`parse_fixed`].
fn fixed_nibbles(
    text: &str,
    digits: u8,
    scale: u8,
    policy: FixedDigits,
    at: usize,
) -> Result<Vec<u8>, WireError> {
    let parts = parse_fixed(text, digits, scale, at)?;
    let integral_width = usize::from(digits - scale);

    let mut all = Vec::with_capacity(usize::from(digits));
    all.extend(core::iter::repeat_n(
        0u8,
        integral_width - parts.whole.len(),
    ));
    all.extend(parts.whole.bytes().map(|byte| byte - b'0'));
    all.extend(parts.fraction.bytes().map(|byte| byte - b'0'));
    all.extend(core::iter::repeat_n(
        0u8,
        usize::from(scale) - parts.fraction.len(),
    ));
    let significant =
        u8::try_from(all.len() - all.iter().take_while(|n| **n == 0).count()).unwrap_or(u8::MAX);
    // What `ValueStringDigits` counts: the integer digits as written,
    // leading zeros included, plus `S` fraction digits. The fraction as
    // written would cut `7` under `fixed<5,2>` to its last nibble, `0c`.
    // Past 31 on the wire no reader can be told the field's width, and a
    // count that does not fit a `u8` would wrap into a different field.
    let magnitude = text.trim_start_matches(['+', '-']);
    let whole_as_written = magnitude.split('.').next().map_or(0, str::len);
    let written = whole_as_written + usize::from(scale);
    if policy == FixedDigits::ValueStringDigits && written + (1 - written % 2) > 31 {
        return Err(WireError::refused(
            at,
            "a fixed of at most 31 digits on the wire, as written",
        ));
    }
    // Only `ValueStringDigits` reads the count, and the check above holds
    // it to 31 there.
    let string_digits = if policy == FixedDigits::ValueStringDigits {
        u8::try_from(written).map_err(|_| {
            WireError::refused(at, "a fixed of at most 31 digits on the wire, as written")
        })?
    } else {
        0
    };
    let wire = usize::from(policy.wire_digits(digits, significant, string_digits));
    let kept = &all[all.len().saturating_sub(wire)..];

    let mut nibbles = Vec::with_capacity(wire + 2);
    if (wire + 1) % 2 == 1 {
        nibbles.push(0);
    }
    // A count wider than the value's `P` digits — ACE's reading of
    // `0.12345` under `fixed<5,5>` is seven — is filled with leading zeros;
    // without them the sign nibble fell off the end of the packing.
    nibbles.extend(core::iter::repeat_n(0u8, wire.saturating_sub(kept.len())));
    nibbles.extend_from_slice(kept);
    // §9.3.2.8: `0xd` marks a negative value and `0xc` everything else, zero
    // included. A negative zero is written positive: there is one zero.
    nibbles.push(if parts.negative && nibbles.iter().any(|n| *n != 0) {
        0x0d
    } else {
        0x0c
    });
    Ok(nibbles)
}

/// Accumulates the bytes of one sample.
#[derive(Debug, Clone)]
pub struct Writer {
    out: Vec<u8>,
    endianness: Endianness,
    /// How this writer frames a PL_CDR1 parameter list.
    pl_cdr1: PlCdr1Framing,
    /// Where alignment is counted from.
    ///
    /// Zero for a sample; for a nested value inside a DHEADER it stays
    /// zero as well, because XCDR counts alignment from the
    /// encapsulation and not from the nested value's own start.
    origin: usize,
    /// The encoding's `MAXALIGN`, which caps every alignment request.
    ///
    /// Eight under XCDR1, four under XCDR2. XTypes 1.3 §7.4.1:
    /// `ALIGN(N)` aligns to `MIN(N, XCDR.maxalign)`. A writer that did
    /// not know this padded an eight-byte primitive to eight under
    /// XCDR2 and put four bytes on the wire that no peer expects.
    max_align: usize,
    /// How wide a `Char16` is. See [`WcharWidth`].
    wchar_width: WcharWidth,
    /// How a scalar `Char16` is framed. See [`WcharFraming`].
    wchar_framing: WcharFraming,
    /// In what byte order wide code units go out. See
    /// [`WstringUnitOrder`], and [`Writer::write_wchar`] for the one
    /// place a *scalar* consults it.
    wstring_unit_order: WstringUnitOrder,
    /// Whether an LC4 member whose length turns out to be 1, 2, 4 or 8
    /// is rewritten with a short length code.
    short_length_codes: bool,
    /// Every span of alignment padding this writer emitted, when a
    /// caller asked for the record.
    ///
    /// `None` by default: nothing on the data path pays for it. The one
    /// caller is the interop comparison, which needs the answer to
    /// "which offsets of this byte string carry no value" computed from
    /// the *layout* rather than read off a byte-for-byte difference
    /// with a peer — see [`Writer::recording_padding`].
    /// Whether each `MinimalStructMember`/`MinimalUnionMember` carries a
    /// trailing octet of padding inside its own `DHEADER`.
    ///
    /// The XTypes 1.3 §7.3.4.5 minimal `EquivalenceHash` is taken over
    /// the serialized minimal `TypeObject`, and two vendor camps write
    /// an aggregated member differently: Cyclone/Fast DDS/HDDS write the
    /// member's `DHEADER` body and stop; RTI Connext and CoreDX append
    /// one octet inside that body. `docs/omg-xtypes-issues.md` §C2
    /// records the two byte strings. `false` is the Cyclone camp and
    /// the default; a peer talking to RTI/CoreDX sets it from
    /// `zerodds_ast_layout::Dialect`.
    minimal_member_trailing_octet: bool,
    padding: Option<Vec<(usize, usize)>>,
    /// The first error an infallible framing call ([`Writer::finish_dheader`])
    /// met. The bytes are not a valid stream once it is set; see
    /// [`Writer::deferred_error`].
    deferred: Option<WireError>,
}

impl Writer {
    /// A writer in the given byte order.
    #[must_use]
    pub fn new(endianness: Endianness) -> Self {
        Self {
            out: Vec::new(),
            endianness,
            pl_cdr1: PlCdr1Framing::default(),
            origin: 0,
            max_align: 8,
            wchar_width: WcharWidth::Two,
            wchar_framing: WcharFraming::Fixed,
            wstring_unit_order: WstringUnitOrder::Stream,
            short_length_codes: false,
            minimal_member_trailing_octet: false,
            padding: None,
            deferred: None,
        }
    }

    /// The same writer, keeping a record of every span of alignment
    /// padding it emits.
    ///
    /// The record is the answer §7.4.1's alignment rules give for this
    /// type under this key: [`Writer::align_to`] is the one place the
    /// walk emits a byte that carries no value, so the spans it notes
    /// are the fill positions of the plan and not an observation about
    /// the octets that came out. Two byte-shifting rewrites happen
    /// after the fact — `Writer::promote_pid` inserts eight octets
    /// and `shorten_emheader` removes four — and both move the spans
    /// above them, so the record stays in the finished stream's
    /// coordinates.
    ///
    /// Off by default and allocating only when asked for.
    #[must_use]
    pub fn recording_padding(mut self) -> Self {
        self.padding = Some(Vec::new());
        self
    }

    /// The recorded padding spans as `(offset, length)`, in the
    /// finished stream's coordinates and in ascending order.
    ///
    /// Empty when [`Writer::recording_padding`] was not asked for,
    /// which is indistinguishable from a stream with no padding. A
    /// caller that needs to tell the two apart asks for the record.
    #[must_use]
    pub fn padding_spans(&self) -> &[(usize, usize)] {
        self.padding.as_deref().unwrap_or(&[])
    }

    /// Whether this writer is keeping the record.
    #[must_use]
    pub const fn is_recording_padding(&self) -> bool {
        self.padding.is_some()
    }

    /// Move every recorded padding span at or above `from` by `delta`.
    ///
    /// The two rewrites that change a byte's offset after it is written
    /// call this. Neither can move a span *partially*: the eight octets
    /// [`Writer::promote_pid`] inserts go between a parameter header
    /// and its value, and the four `shorten_emheader` removes are the
    /// NEXTINT it wrote itself — no padding span begins inside either
    /// range, so shifting whole spans is the whole correction.
    ///
    /// A no-op when nothing is being recorded.
    fn shift_padding(&mut self, from: usize, delta: isize) {
        let Some(spans) = self.padding.as_mut() else {
            return;
        };
        for span in spans.iter_mut() {
            if span.0 >= from {
                span.0 = span.0.saturating_add_signed(delta);
            }
        }
    }

    /// The same writer framing a scalar `Char16` another way.
    ///
    /// A preset and not a patch, like every other `with_*` here: the
    /// framing and the width are separate because CORBA makes them
    /// separate — omniORB and TAO write the same framing over two
    /// octets and four.
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

    /// The same writer ordering wide code units another way.
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

    /// The same writer putting a `Char16` on the wire at another width.
    ///
    /// Set it from
    /// `zerodds_ast_layout::WstringLengthPolicy::character_width`, which
    /// is the same source the layout plan sized the member from. A
    /// writer wider than its plan writes a member the plan did not
    /// reserve room for and every offset after it is wrong.
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

    /// The same writer shortening an LC4 header whose member turned out
    /// to be 1, 2, 4 or 8 bytes long.
    ///
    /// Set it from
    /// `zerodds_ast_layout::LengthCodePolicy::short_code_where_it_fits`.
    /// The plan already picks a short code for every member whose *type*
    /// fixes the size; this is the other half, for a sequence or a map
    /// whose length is in the value. Fast DDS makes the same choice the
    /// same way — its `sequence<short>` of two elements is eight bytes
    /// and goes out as `02000030`, where three elements would be ten and
    /// go out as LC4.
    #[must_use]
    pub const fn with_short_length_codes(mut self, shorten: bool) -> Self {
        self.short_length_codes = shorten;
        self
    }

    /// Whether an LC4 header is shortened when its member fits a code.
    #[must_use]
    pub const fn short_length_codes(&self) -> bool {
        self.short_length_codes
    }

    /// The same writer framing a PL_CDR1 parameter list another way.
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

    /// The writer, set to append (or not) the RTI/CoreDX minimal-member
    /// trailing octet. See [`Writer::minimal_member_trailing_octet`].
    #[must_use]
    pub const fn with_minimal_member_trailing_octet(mut self, on: bool) -> Self {
        self.minimal_member_trailing_octet = on;
        self
    }

    /// Whether this writer appends the RTI/CoreDX minimal-member trailing
    /// octet (XTypes 1.3 §7.3.4.5; `docs/omg-xtypes-issues.md` §C2).
    #[must_use]
    pub const fn minimal_member_trailing_octet(&self) -> bool {
        self.minimal_member_trailing_octet
    }

    /// A writer whose alignment is capped at `max_align`.
    ///
    /// Four for XCDR2, eight for XCDR1 — take it from
    /// `zerodds_ast_layout::Encoding::max_alignment` rather than
    /// writing a number here.
    #[must_use]
    pub fn with_max_align(endianness: Endianness, max_align: usize) -> Self {
        Self {
            max_align: max_align.max(1),
            ..Self::new(endianness)
        }
    }

    /// A writer that appends to `out`, with its alignment cap at
    /// `max_align` and alignment counted from where `out` ends.
    ///
    /// For an encoder handed a buffer to fill: a writer of its own grows
    /// from empty and its bytes are then copied over, and for a 1 kB sample
    /// that was four reallocations and a copy per write (measured on the
    /// Linux test host). Counting from `out`'s end makes the bytes the same
    /// as a fresh writer's whose bytes were appended to `out`.
    /// [`Self::finish`] hands `out` back with the sample behind what it
    /// held.
    #[must_use]
    pub fn appending(endianness: Endianness, max_align: usize, out: Vec<u8>) -> Self {
        Self {
            origin: out.len(),
            out,
            ..Self::with_max_align(endianness, max_align)
        }
    }

    /// A writer that continues a stream whose first octets are `prefix`:
    /// it appends behind them and counts alignment from the start of
    /// `prefix`, not from where `prefix` ends (which is what
    /// [`Self::appending`] does).
    ///
    /// For a value that is a member of a larger object whose earlier
    /// members were written by someone else — DDS-RPC 1.0 §7.5.1.1.6 puts
    /// the `Call` union behind a `RequestHeader` of variable length, and
    /// under XCDR1 the union's 8-octet members align to the sample, not to
    /// the union. [`Self::finish`] hands back `prefix` with the value
    /// behind it.
    #[must_use]
    pub fn continuing(endianness: Endianness, max_align: usize, prefix: Vec<u8>) -> Self {
        Self {
            out: prefix,
            ..Self::with_max_align(endianness, max_align)
        }
    }

    /// The alignment cap in force.
    #[must_use]
    pub const fn max_align(&self) -> usize {
        self.max_align
    }

    /// The byte order this writer was given.
    ///
    /// The counterpart of [`crate::reader::Reader::endianness`], for the
    /// callers that have to put the order they are writing in *into* the
    /// stream — a CDR encapsulation opens with a byte-order octet that
    /// says which order its body is in.
    #[must_use]
    pub const fn endianness(&self) -> Endianness {
        self.endianness
    }

    /// A big-endian writer, which is what a KeyHash holder is.
    #[must_use]
    pub fn big_endian() -> Self {
        Self::new(Endianness::Big)
    }

    /// A little-endian writer.
    #[must_use]
    pub fn little_endian() -> Self {
        Self::new(Endianness::Little)
    }

    /// The bytes written so far.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.out
    }

    /// The bytes, with the writer consumed.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.out
    }

    /// How many bytes have been written.
    #[must_use]
    pub fn position(&self) -> usize {
        self.out.len()
    }

    /// Pad to `align`, counted from the stream's origin.
    pub fn align_to(&mut self, align: usize) {
        let align = align.min(self.max_align);
        if align <= 1 {
            return;
        }
        let offset = self.out.len().saturating_sub(self.origin);
        let padding = (align - (offset % align)) % align;
        if padding == 0 {
            return;
        }
        if let Some(spans) = self.padding.as_mut() {
            spans.push((self.out.len(), padding));
        }
        for _ in 0..padding {
            self.out.push(0);
        }
    }

    /// Begin a collection element whose alignment is counted from its
    /// own first octet, returning the origin it replaced.
    ///
    /// XTypes 1.3 §7.4.1 counts a value's alignment from the
    /// encapsulation, so an element of an array or a sequence begins at
    /// the next multiple of its alignment and the collection pads
    /// *between* elements. `zerodds_ast_layout::ElementPadding::Packed`
    /// is the measured departure: the element begins where the previous
    /// one ended and its own members align from there. Resetting the
    /// origin here is the whole of it; [`Self::finish_packed_element`]
    /// puts the stream's origin back so the collection's tail aligns
    /// from the encapsulation again.
    pub fn begin_packed_element(&mut self) -> usize {
        let saved = self.origin;
        self.origin = self.out.len();
        saved
    }

    /// Restore the stream origin a packed element displaced. The pair of
    /// [`Self::begin_packed_element`].
    pub fn finish_packed_element(&mut self, saved: usize) {
        self.origin = saved;
    }

    // -- primitives ------------------------------------------------------

    /// `boolean`, one byte, `0` or `1`.
    pub fn write_bool(&mut self, value: bool) {
        self.out.push(u8::from(value));
    }

    /// `octet` and `uint8`.
    pub fn write_u8(&mut self, value: u8) {
        self.out.push(value);
    }

    /// `fixed<digits, scale>`, as BCD — CORBA 3.3 part 2 §9.3.2.8.
    ///
    /// The value arrives as a decimal string and not as a number, because a
    /// `fixed<31, 2>` is exactly what a `f64` cannot carry: the type exists to
    /// hold a decimal exactly, and a binary float on the way in would defeat
    /// it before the first nibble.
    ///
    /// Two nibbles to the byte, high first, sign last: `0xd` negative, `0xc`
    /// otherwise, zero included. A negative zero goes out positive, because
    /// there is one zero to write.
    ///
    /// No DDS vendor generates code for `fixed` — Cyclone, Fast DDS and RTI
    /// emit nothing for it and OpenDDS does not compile it, measured across
    /// the corpus — so there is no vendor oracle here, only the clause and
    /// the twenty other columns of this project that already write it.
    ///
    /// # Errors
    /// [`WireError`] when the declaration is outside IDL rule (42)'s 31
    /// digits, when `scale` exceeds `digits`, or when the text is not a
    /// decimal that fits `fixed<digits, scale>`. A value that does not fit is
    /// refused rather than truncated: a silently shortened decimal is the
    /// wrong number with no way to notice.
    pub fn write_fixed(
        &mut self,
        text: &str,
        digits: u8,
        scale: u8,
        policy: FixedDigits,
    ) -> Result<(), WireError> {
        let nibbles = fixed_nibbles(text, digits, scale, policy, self.position())?;
        // The count is even by construction, so the last byte carries a digit
        // and the sign and never half a byte of nothing.
        for pair in nibbles.chunks_exact(2) {
            self.write_u8((pair[0] << 4) | pair[1]);
        }
        Ok(())
    }

    /// `int8`.
    pub fn write_i8(&mut self, value: i8) {
        #[allow(clippy::cast_sign_loss)]
        self.out.push(value as u8);
    }

    /// `char`, one byte. DDS-XTypes 1.3 §7.4.1.1.1, Table 31.
    ///
    /// §7.2.2.2.1.2.2 gives the whole rule: "This specification does not
    /// define an encoding for the `CHAR_8_TYPE`. The only constraint is
    /// that it shall be representable using 8 bits." A code point above
    /// U+00FF is not, and no byte stands for it.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] above U+00FF. This used to be
    /// `value as u32 as u8`, which drops the high bits and puts a
    /// different character on the wire with nothing said.
    pub fn write_char(&mut self, value: char) -> Result<(), WireError> {
        let point = u32::from(value);
        let byte = u8::try_from(point).map_err(|_| WireError::OutOfRange {
            at: self.out.len(),
            expected: format!("a character below U+0100, not U+{point:04X}"),
        })?;
        self.out.push(byte);
        Ok(())
    }

    /// `wchar`: one UTF-16 code unit, at the width
    /// [`Writer::wchar_width`] names. DDS-XTypes 1.3 §7.4.1.1.1,
    /// Table 31 gives two bytes; [`WcharWidth`] carries the one measured
    /// implementation that writes four and where it does so.
    ///
    /// §7.2.2.2.1.2.5: "The `CHAR_16_TYPE` shall be restricted to
    /// representing Unicode codepoints in the Basic Multilingual Plane.
    /// That is Unicode codepoints from 0x0000 to U+FFFF." A code point
    /// above the BMP takes two UTF-16 code units and has no `wchar`.
    /// That is a property of the type and not of the width, so the
    /// four-byte form refuses it too: RTI zero-extends a code *unit*,
    /// it does not carry a code point.
    ///
    /// `position` says whether this `wchar` is a member or a collection
    /// element. Under [`WcharFraming::GiopLengthPrefixedMemberOnly`] the
    /// two take different octets, and there is no default: a caller that
    /// could leave it out would take the member's framing for an
    /// element, which is the shape of the defect that put the argument
    /// here. `tao_idl` writes `04 e4000000` for a `wchar` member of
    /// `primitive-wchar` and a bare `e4000000` for the first element of
    /// `array-1d-primitives`.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] above U+FFFF, for the reason
    /// [`Writer::write_char`] gives at its own width.
    pub fn write_wchar(&mut self, value: char, position: WcharPosition) -> Result<(), WireError> {
        let point = u32::from(value);
        let unit = u16::try_from(point).map_err(|_| WireError::OutOfRange {
            at: self.out.len(),
            expected: format!("a BMP character, not U+{point:04X}"),
        })?;
        if self.wchar_framing.has_length_octet(position) {
            // CORBA §9.3.1.6's GIOP 1.2 form: the octet count first,
            // then the code unit's octets in the stream's order, all
            // aligned to one. omniORB writes `02 00e4` for U+00E4 in
            // either stream order and TAO `04 e4000000` on this
            // little-endian host; the count is the transmission code
            // set's width and not the value's, so it is the same octet
            // for every character.
            let width = self.wchar_width.bytes();
            #[allow(clippy::cast_possible_truncation)]
            self.out.push(width as u8);
            // The octets follow the same axis a `wstring`'s code
            // units do, with one difference the clause makes: a
            // scalar cannot carry a byte-order mark, so CORBA's
            // default applies and `ByteOrderMark` degrades to
            // big-endian here. Measured 2026-08-10 in a
            // *little-endian* stream: omniORB writes `02 00e4` and
            // JacORB `02 00e4`, both big-endian; TAO writes
            // `04 e4000000`, the stream's order.
            let big = !matches!(self.wstring_unit_order, WstringUnitOrder::Stream);
            match (self.wchar_width, big) {
                (WcharWidth::Two, false) => self.extend(&self.order2(unit)),
                (WcharWidth::Two, true) => self.extend(&unit.to_be_bytes()),
                (WcharWidth::Four, false) => self.extend(&self.order4(u32::from(unit))),
                (WcharWidth::Four, true) => self.extend(&u32::from(unit).to_be_bytes()),
            }
        } else {
            // Table 31's fixed-width unit, aligned to its own width.
            // Reached by every DDS implementation in every position and
            // by TAO inside a collection, where its generated code
            // marshals the element array directly and never passes the
            // per-character path that writes the count.
            match self.wchar_width {
                WcharWidth::Two => self.write_u16(unit),
                WcharWidth::Four => self.write_u32(u32::from(unit)),
            }
        }
        Ok(())
    }

    /// [`Writer::write_wchar`] at the member position.
    ///
    /// The generated codecs call this and
    /// [`Writer::write_wchar_element`] rather than passing a position
    /// along: a collection combinator takes `Fn(&mut Writer, &char)`,
    /// and a two-argument method cannot be handed to one as a path
    /// without a closure that would hide which position it chose.
    ///
    /// # Errors
    /// As [`Writer::write_wchar`].
    pub fn write_wchar_member(&mut self, value: char) -> Result<(), WireError> {
        self.write_wchar(value, WcharPosition::Member)
    }

    /// [`Writer::write_wchar`] at the collection-element position.
    ///
    /// # Errors
    /// As [`Writer::write_wchar`].
    pub fn write_wchar_element(&mut self, value: &char) -> Result<(), WireError> {
        self.write_wchar(*value, WcharPosition::Element)
    }

    /// `short`.
    pub fn write_i16(&mut self, value: i16) {
        #[allow(clippy::cast_sign_loss)]
        self.write_u16(value as u16);
    }

    /// A bitmask, in the holder Table 33 of XTypes 1.3 §7.4.1.1.3.2
    /// gives for its `@bit_bound`.
    ///
    /// The bound and not the language's holder: `@bit_bound(12)` is a
    /// `uint16_t` in C and a `u16` here, and the two octets it writes
    /// carry twelve bits that mean something and four that do not. The
    /// argument arrives from `zerodds_ast_layout::plan::scalar_write`,
    /// which is the same crossing every other column asks — the C
    /// runtime's `zdw_write_bitmask` takes the same pair in the same
    /// order.
    ///
    /// # Refused, not masked
    ///
    /// A value with a bit set above the bound is refused rather than
    /// truncated to the bound. Masking would put a value on the wire
    /// nobody wrote and report success, and the peer would read a set
    /// the sender never held. `runtime/c/src/writer.c` refuses the same
    /// value with `ZDW_RANGE`.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a bound of zero or above 64, and
    /// for a value that does not fit the bound.
    pub fn write_bitmask(&mut self, bit_bound: u16, bits: u64) -> Result<(), WireError> {
        if bit_bound == 0 || bit_bound > 64 {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: alloc::format!(
                    "a bitmask bound between 1 and 64, and this one is {bit_bound}"
                ),
            });
        }
        if bit_bound < 64 && (bits >> bit_bound) != 0 {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: alloc::format!(
                    "a bitmask value inside its bound of {bit_bound} bits, and this one is {bits}"
                ),
            });
        }
        // The check above is what makes each narrowing exact: a value
        // that did not fit its bound never reaches this match.
        #[allow(clippy::cast_possible_truncation)]
        match bit_bound {
            0..=8 => self.write_u8(bits as u8),
            9..=16 => self.write_u16(bits as u16),
            17..=32 => self.write_u32(bits as u32),
            _ => self.write_u64(bits),
        }
        Ok(())
    }

    /// A bitset, as the anchor integer its bitfields pack into.
    ///
    /// The anchor width is the layout plan's answer and arrives as a
    /// byte count, which is what `zerodds_ast_layout::xcdr::plan` gives
    /// as the type's extent and what `zdw_write_bitset` takes.
    ///
    /// XTypes 1.3 has no serialisation rule for a bitset — §7.4.3.5.3
    /// runs from (1) to (30) and none matches one. What is implemented
    /// is the only reading under which an IDL bitset round-trips
    /// through the TypeObject of §7.3.4.6: positions run cumulatively
    /// from zero in declaration order, anonymous padding included. The
    /// C runtime carries the same paragraph.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for an anchor that is not one, two,
    /// four or eight octets.
    pub fn write_bitset(&mut self, anchor_bytes: u32, bits: u64) -> Result<(), WireError> {
        // Exact for the same reason `write_bitmask`'s narrowings are:
        // `bitfield` refused every value that does not fit its width,
        // and the plan sized the anchor over the widths.
        #[allow(clippy::cast_possible_truncation)]
        match anchor_bytes {
            1 => self.write_u8(bits as u8),
            2 => self.write_u16(bits as u16),
            4 => self.write_u32(bits as u32),
            8 => self.write_u64(bits),
            other => {
                return Err(WireError::OutOfRange {
                    at: self.out.len(),
                    expected: alloc::format!(
                        "a bitset anchor of 1, 2, 4 or 8 octets, and this one is {other}"
                    ),
                });
            }
        }
        Ok(())
    }

    /// One bitfield's contribution to its bitset's anchor: the value
    /// shifted to its position.
    ///
    /// A value wider than the field is **refused** rather than masked.
    /// Masking would put a different value on the wire than the caller
    /// held and report success — the generated field is as wide as its
    /// holder and not as wide as its `bitfield<N>`, so `bitfield<3> a`
    /// is a `u8` that can hold 200 and a bitset that can not.
    /// `zdw_bitfield` refuses the same value with `ZDW_RANGE`.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a width outside 1..=64, a field
    /// that runs past the sixty-fourth bit, or a value the width cannot
    /// hold.
    pub fn bitfield(&mut self, value: u64, width: u32, position: u32) -> Result<u64, WireError> {
        if width == 0 || width > 64 || position.saturating_add(width) > 64 {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: alloc::format!(
                    "a bitfield inside the anchor's 64 bits, and this one is {width} bits at {position}"
                ),
            });
        }
        let limit = if width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        if value > limit {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: alloc::format!(
                    "a bitfield value inside its {width} bits, and this one is {value}"
                ),
            });
        }
        Ok(value << position)
    }

    /// `unsigned short`.
    pub fn write_u16(&mut self, value: u16) {
        self.align_to(2);
        self.extend(&self.order2(value));
    }

    /// A 16-bit value in **little-endian** order whatever the stream's
    /// order is, aligned to two like any other.
    ///
    /// One caller and one reason: Cyclone DDS swaps a `wstring`'s length
    /// word for a big-endian stream and does not swap its code units.
    /// `zerodds_ast_layout::WstringLengthPolicy::UnswappedUnits` carries
    /// the measurement and the argument for reproducing it on request.
    /// Nothing else on the wire has this shape, and a member that used
    /// it without that reason would be a member no peer reads.
    pub fn write_u16_little(&mut self, value: u16) {
        self.align_to(2);
        self.extend(&value.to_le_bytes());
    }

    /// A 16-bit value in **big-endian** order whatever the stream's
    /// order is, aligned to two like any other.
    ///
    /// The counterpart of [`Writer::write_u16_little`] on the other
    /// side of the same axis. Both omniORB and JacORB decode a
    /// `wstring`'s code units as UTF-16 big-endian whatever the stream
    /// says — the OMG code-set rule for a GIOP 1.2 `wstring` that
    /// carries no byte-order mark — so this is the form a `wstring`
    /// written for an ORB takes in a little-endian stream.
    pub fn write_u16_big(&mut self, value: u16) {
        self.align_to(2);
        self.extend(&value.to_be_bytes());
    }

    /// A 32-bit value in **little-endian** order whatever the stream's
    /// order is.
    ///
    /// A four-octet transmission code set crossed with a fixed unit
    /// order: reachable as `--wstring-length specification-wide-units
    /// --wstring-units byte-order-mark` and by no dialect, because no
    /// measured implementation writes wide units in a fixed order. It
    /// exists so that two orthogonal settings have an answer for every
    /// combination rather than a hole one of them falls into.
    pub fn write_u32_little(&mut self, value: u32) {
        self.align_to(4);
        self.extend(&value.to_le_bytes());
    }

    /// A 32-bit value in **big-endian** order whatever the stream's
    /// order is. The counterpart of [`Writer::write_u32_little`].
    pub fn write_u32_big(&mut self, value: u32) {
        self.align_to(4);
        self.extend(&value.to_be_bytes());
    }

    /// `long`.
    pub fn write_i32(&mut self, value: i32) {
        #[allow(clippy::cast_sign_loss)]
        self.write_u32(value as u32);
    }

    /// `unsigned long`.
    pub fn write_u32(&mut self, value: u32) {
        self.align_to(4);
        self.extend(&self.order4(value));
    }

    /// `long long`.
    pub fn write_i64(&mut self, value: i64) {
        #[allow(clippy::cast_sign_loss)]
        self.write_u64(value as u64);
    }

    /// `unsigned long long`.
    pub fn write_u64(&mut self, value: u64) {
        self.align_to(8);
        self.extend(&self.order8(value));
    }

    /// `float`.
    pub fn write_f32(&mut self, value: f32) {
        self.write_u32(value.to_bits());
    }

    /// `double`.
    pub fn write_f64(&mut self, value: f64) {
        self.write_u64(value.to_bits());
    }

    /// `long double`: sixteen bytes of IEEE 754 binary128.
    ///
    /// CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2 gives the layout — one
    /// sign bit, a fifteen-bit exponent, a 112-bit fraction — and shows
    /// the two byte orders as the whole sixteen bytes reversed, not two
    /// eight-byte halves swapped in place. DDS-XTypes 1.3 Table 31 gives
    /// the alignment: eight, capped to four by `MAXALIGN` under XCDR2,
    /// which is what [`Writer::align_to`] already does.
    ///
    /// This used to take an `f64` and write it followed by eight zero
    /// bytes, because Rust has no stable `f128`. That was the right
    /// length and the wrong value in every column and it reported
    /// success; `zerodds_float128::Float128` carries the value instead.
    /// Nothing here computes with it — a wire runtime never needs to.
    pub fn write_f128(&mut self, value: Float128) {
        self.align_to(8);
        let bytes = if self.endianness.is_little() {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        self.extend(&bytes);
    }

    // -- strings ---------------------------------------------------------

    /// A string: a four-byte length including the terminator, the bytes,
    /// then a NUL. DDS-XTypes 1.3 §7.4.1.1.
    ///
    /// The length counts the terminator, which is the one place XCDR
    /// differs from every other length-prefixed encoding.
    pub fn write_string(&mut self, value: &str) {
        #[allow(clippy::cast_possible_truncation)]
        let length = (value.len() + 1) as u32;
        self.write_u32(length);
        self.extend(value.as_bytes());
        self.out.push(0);
    }

    /// A bounded string.
    ///
    /// The same bytes as an unbounded one: the bound is a promise about
    /// the source, not a difference on the wire. It is checked here
    /// rather than trusted, because a value that overruns its bound is
    /// one no peer expected.
    ///
    /// The bound counts **octets**, which is what `string<N>` means to
    /// every peer that has to hold the value — `char[N + 1]` in the C
    /// mapping of all four DDS implementations, `fixed_string<N>` in
    /// Fast DDS's.
    ///
    /// # Refused, not truncated
    ///
    /// This truncated at a character boundary and reported nothing,
    /// which is `AL-14` of `docs/wire-feature-inventory.md`: acceptable
    /// only while nothing called it. Something does now — the generated
    /// codec emits `write_string_bounded` for every bounded `string`
    /// member — and a writer that truncated would put a value on the
    /// wire nobody wrote. §7.2.2.7's TRIM is a *read*-side behaviour and
    /// belongs to the receiver's bound, not the sender's;
    /// `wire-probe/src/encode.rs::write_string` refuses the same values
    /// (D6), so the two columns now agree.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a value longer than the bound.
    pub fn write_string_bounded(&mut self, value: &str, bound: usize) -> Result<(), WireError> {
        if value.len() > bound {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: alloc::format!(
                    "at most {bound} octets in a bounded string, and this one has {}",
                    value.len()
                ),
            });
        }
        self.write_string(value);
        Ok(())
    }

    /// A `wstring`: a four-octet length, then UTF-16 code units, in the
    /// cell `policy` and `order` name.
    ///
    /// XTypes 1.3 §7.4.3.5.3 rule (4) writes `O.ssize` as a `UInt32`
    /// and then the `Char16` units, annotated "No NUL" against rule
    /// (3)'s "includes NUL" for a narrow string. What `O.ssize` counts
    /// is settled in §7.4.1.1.2: "The serialized length of an object of
    /// type `String<Char16>` shall be the number of bytes in the CDR
    /// buffer taken by the `String<Char16>` characters. This is twice the
    /// number of characters in the string" — code units, not
    /// characters, so a `"𝄞"` is a length of four and not of one or
    /// two. The same clause forbids the two things a UTF-16 encoder
    /// would otherwise emit: "The UTF-16 representation … shall not
    /// include a Byte Order Mark (BOM). The representation shall also
    /// not include any terminating NUL character(s)."
    ///
    /// That is one cell of eight. [`WstringLengthPolicy`] carries the
    /// other seven and the runs that measured five of them, and
    /// [`WstringUnitOrder`] carries the fourth axis. Both arrive as
    /// arguments, resolved from `zerodds_ast_layout::LayoutKey`, for the
    /// reason `MapDelimiter` arrives as the choice between
    /// [`Writer::write_map`] and [`Writer::write_delimited_map`]: a
    /// default in the runtime would be a second place the wire format
    /// lives, and the two places would disagree the first time either
    /// moved.
    ///
    /// `position` says whether this `wstring` is reached as a member or
    /// as a collection element. The nine measured implementations
    /// answer both alike, so it changes nothing today — and it is the
    /// argument that keeps that a measurement rather than an
    /// assumption, the way [`Writer::write_wchar`]'s position is the
    /// argument that caught `tao_idl` writing two different framings.
    ///
    /// # Errors
    ///
    /// [`WireError::OutOfRange`] for a code point outside the Basic
    /// Multilingual Plane under a **character count over two-octet
    /// units** — the [`WstringLengthPolicy::CHARACTER_COUNT`] cell.
    /// Fast CDR counts `wcslen` characters and then writes each
    /// four-octet `wchar_t` as two octets, so `U+1D11E` leaves as
    /// `d11e`, which is `U+D11E`, a Hangul syllable and a well-formed
    /// code point. The peer reads a different character and has nothing
    /// to object to, and the sender left no trace in the bytes.
    /// Reproducing a vendor's length convention is a policy;
    /// reproducing a data loss no reader can see is not.
    ///
    /// The refusal is on the pair (characters, two octets) and not on a
    /// named form: it is the narrow unit that loses the high bits, so
    /// [`WstringLengthPolicy::ROS_TYPE_SUPPORT`] — a character count
    /// over *four*-octet units — carries the surrogate pair intact and
    /// is not refused.
    pub fn write_wstring(
        &mut self,
        value: &str,
        policy: WstringLengthPolicy,
        order: WstringUnitOrder,
        position: WirePosition,
    ) -> Result<(), WireError> {
        let units = self.wstring_run(value, policy, order, position)?;
        let width = policy.character_width(position);
        // C1's answer, over units that already include C2's terminator
        // and C4's mark. `WstringLengthPolicy::declared_length` is the
        // one expression, so a cell nobody has measured computes its
        // length the same way as the five that are.
        //
        // `None` is refused rather than written wrapped. `as u32` stood
        // here: a length that names a prefix of its own content is the
        // silent class, and the peer would have read a well-formed
        // shorter `wstring` and then every following member from inside
        // this one. Unreachable from a `String` on a 32-bit target,
        // where the value cannot be that long in the first place;
        // reachable on a 64-bit one, at 2 GiB of text for the narrow
        // width and 1 GiB for the wide.
        let counts = match policy.counts_at(position) {
            WstringLengthCounts::Octets => "octet count",
            WstringLengthCounts::Characters => "code-unit count",
        };
        let run = units.len();
        let declared =
            policy
                .declared_length(run, position)
                .ok_or_else(|| WireError::OutOfRange {
                    at: self.out.len(),
                    expected: format!(
                        "a wstring whose {counts} fits the UInt32 length field, and this one \
                     has {run} code units at {width} octets each"
                    ),
                })?;
        self.write_u32(declared);
        for unit in units {
            self.write_wstring_unit(unit, width, order);
        }
        Ok(())
    }

    /// A bounded `wstring`.
    ///
    /// The same octets as an unbounded one of the same length: the
    /// bound is a promise about the source, not a difference on the
    /// wire.
    ///
    /// The bound counts **UTF-16 code units**, which is what
    /// `wstring<N>` means to every peer that has to hold the value — an
    /// `N`-unit buffer in all four DDS implementations' mappings, and
    /// what §7.4.1.1.2's own count is in. Counting Unicode scalar
    /// values instead would accept a five-character value of six code
    /// units that no peer can hold; counting octets would make the same
    /// declaration mean two different capacities under C3's two widths.
    ///
    /// The terminator [`WstringTerminator::Present`] adds is **not**
    /// counted against the bound. `wstring<5>` names a capacity of five
    /// characters and RTI's XCDR1 form writes six units for five
    /// characters; counting the terminator would refuse a value the
    /// declaration admits.
    ///
    /// # Errors
    ///
    /// [`WireError::OutOfRange`] for a value longer than the bound,
    /// refused and not truncated for the reason
    /// [`Writer::write_string_bounded`] gives — §7.2.2.7's TRIM is a
    /// *read*-side behaviour and belongs to the receiver's bound. And
    /// whatever [`Writer::write_wstring`] refuses.
    pub fn write_wstring_bounded(
        &mut self,
        value: &str,
        bound: usize,
        policy: WstringLengthPolicy,
        order: WstringUnitOrder,
        position: WirePosition,
    ) -> Result<(), WireError> {
        let units = value.encode_utf16().count();
        if units > bound {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: format!(
                    "at most {bound} UTF-16 code units in a bounded wstring, \
                     and this one has {units}"
                ),
            });
        }
        self.write_wstring(value, policy, order, position)
    }

    /// A `wstring` whose characters go out as their own UTF-8 octets.
    ///
    /// The write side of `WstringCodeUnits::Utf8`, the measured departure
    /// of one product's generated serialiser: XTypes 1.3 Table 31 gives a
    /// `wstring`'s code unit as `Char16` and §7.4.1.1.2 makes the length
    /// "twice the number of characters", so the clause has no UTF-8 form
    /// at all. The length is the UTF-8 octet count plus C2's terminator
    /// when the cell carries one: under this value C1 and C3 have nothing
    /// to say, because there is no fixed-width code unit for a character
    /// count to count and no width for an octet count to multiply by.
    ///
    /// The order axis is not consulted: UTF-8 octets have no byte order
    /// of their own.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for an octet count that does not fit the
    /// UInt32 length field, refused rather than written wrapped — a
    /// length that names a prefix of its own content is the silent class,
    /// and the peer would read every following member from inside this
    /// one.
    pub fn write_wstring_utf8(
        &mut self,
        value: &str,
        policy: WstringLengthPolicy,
        position: WirePosition,
    ) -> Result<(), WireError> {
        let bytes = value.as_bytes();
        let terminator = matches!(policy.terminator_at(position), WstringTerminator::Present);
        let content = bytes.len() + usize::from(terminator);
        let declared = u32::try_from(content).map_err(|_| WireError::OutOfRange {
            at: self.out.len(),
            expected: format!(
                "a wstring whose UTF-8 octet count fits the UInt32 length field, and this one \
                 has {content} octets"
            ),
        })?;
        self.write_u32(declared);
        for byte in bytes {
            self.write_u8(*byte);
        }
        if terminator {
            self.write_u8(0);
        }
        Ok(())
    }

    /// A bounded `wstring` whose characters go out as UTF-8 octets.
    ///
    /// The same octets as the unbounded one of the same length; the bound
    /// is a promise about the source and counts UTF-16 code units, as
    /// [`Writer::write_wstring_bounded`] does, because that is what
    /// `wstring<N>` means to a peer whatever the content's encoding.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a value longer than the bound, and
    /// whatever [`Writer::write_wstring_utf8`] refuses.
    pub fn write_wstring_utf8_bounded(
        &mut self,
        value: &str,
        bound: usize,
        policy: WstringLengthPolicy,
        position: WirePosition,
    ) -> Result<(), WireError> {
        let units = value.encode_utf16().count();
        if units > bound {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: format!(
                    "at most {bound} UTF-16 code units in a bounded wstring, \
                     and this one has {units}"
                ),
            });
        }
        self.write_wstring_utf8(value, policy, position)
    }

    /// The code units one `wstring` puts on the wire: the text's, plus
    /// C2's terminator, plus C4's mark.
    ///
    /// One function for all three because the three compose in one
    /// order and the length counts what comes out of it. A caller that
    /// assembled them itself could count the terminator and forget the
    /// mark, which is a length that disagrees with its own content.
    fn wstring_run(
        &self,
        value: &str,
        policy: WstringLengthPolicy,
        order: WstringUnitOrder,
        position: WirePosition,
    ) -> Result<Vec<u16>, WireError> {
        let narrow_count = matches!(policy.counts_at(position), WstringLengthCounts::Characters)
            && policy.unit_at(position) == WstringUnitWidth::Two;
        let mut units: Vec<u16> = if narrow_count {
            // The BMP refusal. See this method's caller for why it is
            // an error and not a truncation, and why it is on the pair
            // and not on a named form.
            if let Some(point) = value.chars().find(|c| *c as u32 > 0xFFFF) {
                let point = u32::from(point);
                return Err(WireError::OutOfRange {
                    at: self.out.len(),
                    expected: format!(
                        "a code point inside the BMP, not U+{point:04X}: a character count \
                         over two-octet units has no way to carry one outside it, and Fast \
                         CDR truncates it to sixteen bits and sends a different character"
                    ),
                });
            }
            #[allow(clippy::cast_possible_truncation)]
            value.chars().map(|c| c as u32 as u16).collect()
        } else {
            value.encode_utf16().collect()
        };
        if matches!(policy.terminator_at(position), WstringTerminator::Present) {
            // CORBA §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form. The
            // NUL is a unit of the run and is inside whatever the
            // length counts.
            units.push(0);
        }
        // An **empty** `wstring` carries no mark: omniORB writes
        // `00000000` for one, measured 2026-08-10 as
        // `wstring-unbounded-empty`, which is byte-identical to every
        // other implementation. A mark on an empty string would be a
        // two-octet string that decodes to nothing, and no peer writes
        // it.
        if order.has_mark() && !units.is_empty() {
            units.insert(0, BYTE_ORDER_MARK);
        }
        Ok(units)
    }

    /// One `wstring` code unit, at its width and in its order.
    ///
    /// C3 picks the writer and C4 picks its endianness, in one place, so
    /// that a cell cannot be written one way in the length field and
    /// another in the content. There is no arm for a length policy that
    /// decides a byte order: `HostUnswapped` is a point of the order
    /// axis, so Cyclone's form and `rosidl_typesupport_fastrtps`'s are
    /// the same answer at two different widths rather than two
    /// unrelated special cases.
    fn write_wstring_unit(&mut self, unit: u16, width: usize, order: WstringUnitOrder) {
        match (width, order) {
            (2, WstringUnitOrder::Stream) => self.write_u16(unit),
            (2, WstringUnitOrder::BigEndianAlways) => self.write_u16_big(unit),
            (2, WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped) => {
                self.write_u16_little(unit);
            }
            (_, WstringUnitOrder::Stream) => self.write_u32(u32::from(unit)),
            (_, WstringUnitOrder::BigEndianAlways) => self.write_u32_big(u32::from(unit)),
            (_, WstringUnitOrder::ByteOrderMark | WstringUnitOrder::HostUnswapped) => {
                self.write_u32_little(u32::from(unit));
            }
        }
    }

    // -- framing ---------------------------------------------------------

    /// A DHEADER: a four-byte length the members that follow occupy.
    ///
    /// Written as a placeholder and patched when the members are done,
    /// because the length is not known until then.
    #[must_use]
    pub fn write_dheader(&mut self) -> DHeader {
        self.align_to(4);
        let at = self.out.len();
        self.extend(&[0u8; 4]);
        DHeader { at }
    }

    /// Patch a DHEADER with the length of what followed it.
    ///
    /// A DHEADER is a `UInt32` and "O.ssize is constrained to being smaller
    /// than 4 Giga Bytes" (XTypes 1.3 §7.4.3.4.1): an object past that has
    /// no DHEADER. This form has no way to return the error, so it records
    /// it: the placeholder stays unpatched, [`Self::deferred_error`] returns
    /// the error and [`Self::finish_checked`] and [`Self::check_deferred`]
    /// refuse the stream. A caller that can return an error uses
    /// [`Self::try_finish_dheader`] and gets it at the call.
    pub fn finish_dheader(&mut self, header: DHeader) {
        let span = self.out.len() - header.at - 4;
        self.finish_dheader_spanning(header, span);
    }

    /// [`Self::finish_dheader`] for members that occupy `span` octets.
    fn finish_dheader_spanning(&mut self, header: DHeader, span: usize) {
        if let Err(error) = self.patch_dheader(header, span) {
            self.defer(error);
        }
    }

    /// Patch `header` with `span`, or refuse a `span` a `UInt32` does not
    /// hold.
    fn patch_dheader(&mut self, header: DHeader, span: usize) -> Result<(), WireError> {
        let length = checked_dheader_length(span, header.at)?;
        let bytes = self.order4(length);
        self.out[header.at..header.at + 4].copy_from_slice(&bytes);
        Ok(())
    }

    /// Remember the first error of a call that cannot return one.
    fn defer(&mut self, error: WireError) {
        if self.deferred.is_none() {
            self.deferred = Some(error);
        }
    }

    /// The first error an infallible framing call met, if one did.
    ///
    /// Set by [`Self::finish_dheader`] for an object of 4 GiB or more.
    #[must_use]
    pub fn deferred_error(&self) -> Option<&WireError> {
        self.deferred.as_ref()
    }

    /// [`Self::deferred_error`] as a result.
    ///
    /// # Errors
    /// The first error an infallible framing call met.
    pub fn check_deferred(&self) -> Result<(), WireError> {
        match &self.deferred {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    /// The bytes, refused when an infallible framing call met an error.
    ///
    /// # Errors
    /// [`Self::deferred_error`]'s error.
    pub fn finish_checked(self) -> Result<Vec<u8>, WireError> {
        match self.deferred {
            Some(error) => Err(error),
            None => Ok(self.out),
        }
    }

    /// [`Self::finish_dheader`], refusing a length a `UInt32` does not
    /// hold.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] when the members that followed the
    /// DHEADER occupy 4 GiB or more (XTypes 1.3 §7.4.3.4.1). The buffer is
    /// left as it was, with the placeholder unpatched, for the caller to
    /// discard.
    pub fn try_finish_dheader(&mut self, header: DHeader) -> Result<(), WireError> {
        let span = self.out.len() - header.at - 4;
        self.patch_dheader(header, span)
    }

    /// An XCDR2 EMHEADER: the member id, its length code and the
    /// must-understand flag in one 32-bit word.
    ///
    /// The length code is the layout plan's decision and arrives here as
    /// a number; this crate does not choose one. XTypes 1.3 §7.4.3.4.2.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a member id above
    /// [`E_MEMBER_ID_MASK`]. §7.2.2.4.4.4.4 runs the ids from 0 to
    /// 0x0FFFFFFF and §7.4.3.4.2 leaves them the low 28 bits of the word.
    /// This used to be `member_id & 0x0FFF_FFFF`, which writes a
    /// well-formed header for a member the caller did not name — the peer
    /// reads the wrong member and nothing at either end says so.
    pub fn write_emheader(
        &mut self,
        member_id: u32,
        length_code: u8,
        must_understand: bool,
    ) -> Result<EmHeader, WireError> {
        if member_id > E_MEMBER_ID_MASK {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: format!("a member id inside the 28 bits of an EMHEADER, not {member_id}"),
            });
        }
        self.align_to(4);
        let word = (u32::from(must_understand) << 31)
            | (u32::from(length_code & 0x7) << 28)
            | (member_id & 0x0FFF_FFFF);
        self.write_u32(word);

        // LC0 through LC3 carry the length in the code itself.
        //
        // LC4 is followed by a NEXTINT holding the member's length.
        //
        // LC5 through LC7 also have a NEXTINT, but §7.4.3.4.2 says it
        // "shall be reused also as part of the serialized member ...
        // because the serialization of certain members also starts with
        // an integer length, which would take exactly the same value as
        // NEXTINT. Therefore the use of length codes 5 to 7 saves 4
        // bytes in the serialization." So no placeholder is reserved:
        // the member's own leading word lands in that position and *is*
        // the NEXTINT. Reserving one here would write the length twice.
        let word_at = self.out.len() - 4;
        if length_code == 4 {
            let at = self.out.len();
            self.extend(&[0u8; 4]);
            return Ok(EmHeader {
                word: word_at,
                next_int: Some(at),
                length_code,
            });
        }
        Ok(EmHeader {
            word: word_at,
            next_int: None,
            length_code,
        })
    }

    /// Patch an EMHEADER's NEXTINT with the length of what followed.
    ///
    /// Under [`Writer::short_length_codes`] it may instead delete the
    /// NEXTINT and put the length in the code, which is what Fast DDS
    /// does for a member whose serialized size turns out to be 1, 2, 4
    /// or 8 bytes. See [`Writer::with_short_length_codes`].
    /// # Errors
    /// [`WireError::OutOfRange`] when the member written under a code of
    /// 0 to 3 is not the length that code declares. §7.4.3.4.2 makes the
    /// code *be* the length there — 1, 2, 4 or 8 octets — so a value of
    /// another size misdeclares it and the reader's skip lands inside
    /// data. Codes 4 to 7 carry a NEXTINT and declare nothing of their
    /// own, so they cannot be wrong that way.
    pub fn finish_emheader(&mut self, header: EmHeader) -> Result<(), WireError> {
        let Some(at) = header.next_int else {
            if header.length_code <= 3 && header.word + 4 <= self.out.len() {
                let declared = 1usize << header.length_code;
                let body = self.out.len() - header.word - 4;
                if body != declared {
                    let code = header.length_code;
                    return Err(WireError::OutOfRange {
                        at: self.out.len(),
                        expected: format!(
                            "a member of {declared} octets under length code {code}, not {body}"
                        ),
                    });
                }
            }
            return Ok(());
        };
        #[allow(clippy::cast_possible_truncation)]
        let length = (self.out.len() - at - 4) as u32;
        if self.short_length_codes && matches!(length, 1 | 2 | 4 | 8) {
            self.shorten_emheader(header.word, at, length);
            return Ok(());
        }
        let bytes = self.order4(length);
        self.out[at..at + 4].copy_from_slice(&bytes);
        Ok(())
    }

    /// Replace an LC4 header's NEXTINT with a short length code.
    ///
    /// The four placeholder bytes are removed and the code in the header
    /// word is raised from 4 to the one that spells `length`. Every byte
    /// of the member's value is unchanged and only its position moves,
    /// which is safe for one reason and only under XCDR2: `MAXALIGN` is
    /// four there, the value began at a four-aligned offset with the
    /// NEXTINT in front of it and begins at a four-aligned offset
    /// without it, so no padding inside it was computed against an
    /// offset that has moved.
    ///
    /// Nothing outside the member has been written yet — `finish_emheader`
    /// runs immediately after the value — so the only handles pointing
    /// past `at` are this one's, and both are consumed here. An enclosing
    /// DHEADER or parameter header sits at a lower offset and is patched
    /// afterwards from `self.out.len()`, which the removal has already
    /// corrected.
    fn shorten_emheader(&mut self, word_at: usize, next_int_at: usize, length: u32) {
        let code: u32 = match length {
            1 => 0,
            2 => 1,
            4 => 2,
            _ => 3,
        };
        self.out.drain(next_int_at..next_int_at + 4);
        self.shift_padding(next_int_at, -4_isize);
        // `word_at` is four bytes below `next_int_at` by construction —
        // `write_emheader` set both from the same `out.len()` — and the
        // drain removed bytes above it, so the slice below is in range
        // for the same reason `finish_dheader`'s is. Indexed rather than
        // fetched with a fallback: a fallback here would leave the code
        // at 4 over a stream whose NEXTINT had already been removed,
        // which is four bytes of the member read as its own length.
        let previous = if self.endianness.is_little() {
            u32::from_le_bytes([
                self.out[word_at],
                self.out[word_at + 1],
                self.out[word_at + 2],
                self.out[word_at + 3],
            ])
        } else {
            u32::from_be_bytes([
                self.out[word_at],
                self.out[word_at + 1],
                self.out[word_at + 2],
                self.out[word_at + 3],
            ])
        };
        let patched = (previous & !(0x7 << 28)) | (code << 28);
        let out = self.order4(patched);
        self.out[word_at..word_at + 4].copy_from_slice(&out);
    }

    /// A PL_CDR1 parameter header, short form: `[id:u16][length:u16]`.
    ///
    /// `role` says what the length field will count when
    /// [`Writer::finish_pid`] patches it; see [`ParameterRole`].
    ///
    /// `must_understand` is bit 14 of the id. §7.4.1.2.2: the flag
    /// "shall be set if and only if the must_understand property of the
    /// member is set to true". A member declared `@must_understand`
    /// whose header does not say so goes out looking skippable, and a
    /// peer that does not know it skips it.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for an id above
    /// [`PlCdr1Framing::highest_short_id`]. §7.4.1.2.2 gives the short
    /// form the ids 0 to that value inclusive and reserves what is above
    /// it for the protocol's own parameters — [`PID_EXTENDED`] is 0x3F01
    /// itself — so such an id takes [`Writer::write_pid_extended`] or it
    /// does not go out. The field was read only by the reader until this
    /// check; the plan chooses the form and this is the floor under a
    /// caller that did not.
    pub fn write_pid(
        &mut self,
        pid: u16,
        role: ParameterRole,
        must_understand: bool,
    ) -> Result<Pid, WireError> {
        if pid > self.pl_cdr1.highest_short_id {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: format!("a parameter id the short header carries, not {pid}")
                    + " — the extended header takes it",
            });
        }
        self.align_to(4);
        // Two claimants on the same word, deliberately: the member's own
        // declaration, and whatever the framing puts on every id
        // regardless. OpenDDS's `c0010004` is the second alone, and its
        // `@must_understand` member is the same octets, which is why the
        // two are OR-ed rather than chosen between.
        let flags = self.pl_cdr1.pid_flags.flags_at(WirePosition::Member);
        let pid = if must_understand && self.pl_cdr1.member_must_understand {
            pid | PID_MUST_UNDERSTAND | flags
        } else {
            pid | flags
        };
        self.write_u16(pid);
        let at = self.out.len();
        self.extend(&[0u8; 2]);
        Ok(Pid {
            at,
            extended: false,
            // The id without either flag. Masking only must-understand
            // left `PID_IMPL_EXTENSION` in the id a promotion to the
            // extended form would carry into the `eMemberHeader`, where
            // it is a different bit and would read as an id of
            // 0x8000-something.
            pid: u32::from(pid & !(PID_MUST_UNDERSTAND | PID_IMPL_EXTENSION)),
            must_understand,
            role,
            saved_origin: self.begin_parameter_value(),
        })
    }

    /// A PL_CDR1 parameter header, extended form.
    ///
    /// `[PID_EXTENDED][8][id:u32][length:u32]`, required when the id
    /// reaches the reserved range or the value is longer than `0xFFFF`,
    /// and used for every member under
    /// `zerodds_ast_layout::PlCdr1HeaderPolicy::AlwaysExtended`.
    /// XTypes 1.3 §7.4.1.2.
    ///
    /// The marker carries the must-understand bit when the framing says
    /// so — Cyclone DDS writes `0x7f01`, and its reader would not be
    /// misled by `0x3f01` either, but the bytes differ.
    ///
    /// The **member's** must-understand is a different bit in a
    /// different word: `FLAG_2` of the 32-bit `eMemberHeader`,
    /// §7.4.1.2.1. Cyclone sets the marker's bit on every parameter it
    /// writes and `FLAG_2` only on the members that declare it —
    /// `017f0800 01000040` against `017f0800 02000000` in the same
    /// sample — so reading the member's property off the marker would
    /// make every Cyclone parameter must-understand.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for a member id above
    /// [`E_MEMBER_ID_MASK`], for the reason
    /// [`Writer::write_emheader`] gives: the `eMemberHeader` of
    /// §7.4.1.2.1 has the same 28 bits, and masking aliases the parameter
    /// onto another member.
    pub fn write_pid_extended(
        &mut self,
        pid: u32,
        role: ParameterRole,
        must_understand: bool,
    ) -> Result<Pid, WireError> {
        if pid > E_MEMBER_ID_MASK {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: format!("a member id inside the 28 bits of an eMemberHeader, not {pid}"),
            });
        }
        self.align_to(4);
        let marker = PID_EXTENDED
            | self
                .pl_cdr1
                .pid_flags
                .flags_at(WirePosition::ExtendedMarker);
        self.write_u16(marker);
        self.write_u16(8);
        let member_header = if must_understand && self.pl_cdr1.member_must_understand {
            (pid & E_MEMBER_ID_MASK) | E_FLAG_MUST_UNDERSTAND
        } else {
            pid & E_MEMBER_ID_MASK
        };
        self.write_u32(member_header);
        let at = self.out.len();
        self.extend(&[0u8; 4]);
        Ok(Pid {
            at,
            extended: true,
            pid,
            must_understand,
            role,
            saved_origin: self.begin_parameter_value(),
        })
    }

    /// Start a parameter's value, returning the origin it replaces.
    ///
    /// "Each element, or parameter, within a parameter list data
    /// structure is simply a CDR-encapsulated block of data" —
    /// XTypes 1.3 §7.4.1.2. A block counts alignment from its own start,
    /// so an eight-byte member at the head of one is already aligned.
    ///
    /// Without this a `double` took four bytes of padding *inside* its
    /// block, and `finish_pid` counted them into the declared length:
    /// `01000c00` where Cyclone, RTI and Fast DDS all write `01000800`.
    /// A reader that trusts the length reads four bytes too many and is
    /// wrong from there on.
    fn begin_parameter_value(&mut self) -> usize {
        let saved = self.origin;
        self.origin = self.out.len();
        saved
    }

    /// Patch a parameter header with its value's length.
    ///
    /// The length field never counts the alignment padding *inside* the
    /// block: a parameter's value is its own CDR-encapsulated block and
    /// counts alignment from its own start, XTypes 1.3 §7.4.1.2, which
    /// is why an eight-byte member at the head of one declares eight and
    /// not twelve. All three measured implementations agree on that.
    ///
    /// Whether it counts the padding that *follows* the value is
    /// `PlCdr1Framing::pads_declared_length`, asked per
    /// [`ParameterRole`]: Cyclone and RTI round a `@mutable` member's
    /// declared length up to four and Fast DDS stops at the value, and
    /// for an `@optional` member of a non-mutable type only RTI rounds.
    ///
    /// An absent `@optional` member finishes here with nothing written
    /// between the header and this call, which declares the length of
    /// zero §7.4.1.2 asks for.
    pub fn finish_pid(&mut self, header: Pid) {
        let width = if header.extended { 4 } else { 2 };
        let mut length = self.out.len() - header.at - width;
        if self.pl_cdr1.pads_declared_length(header.role) {
            // Counted from the origin the padding will be measured
            // from, which is the enclosing stream's and not the
            // parameter block's — `align_to` runs below, after the
            // restore. Computing it against the block's own origin
            // would agree today, because a parameter value starts
            // four-aligned, and would stop agreeing the moment that
            // changed.
            let past = (self.out.len() - header.saved_origin) % 4;
            if past != 0 {
                length += 4 - past;
            }
        }
        if header.extended {
            #[allow(clippy::cast_possible_truncation)]
            let bytes = self.order4(length as u32);
            self.out[header.at..header.at + 4].copy_from_slice(&bytes);
        } else if length > PID_SHORT_MAX_LEN {
            self.promote_pid(&header, length);
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let bytes = self.order2(length as u16);
            self.out[header.at..header.at + 2].copy_from_slice(&bytes);
        }
        // The padding follows the value and is outside the length. It
        // starts the next parameter ID at a four-byte offset relative to
        // the previous one, so it is measured from the enclosing stream
        // and not from the block that just ended.
        self.origin = header.saved_origin;
        // Under Fast DDS's framing the parameter writes no padding of
        // its own — the next header aligns itself, and a final
        // parameter ends the stream where its value does.
        if self.pl_cdr1.pad_after_value {
            self.align_to(4);
        }
    }

    /// Rewrite a short parameter header as the extended form.
    ///
    /// §7.4.1.2.2's `ssize` is sixteen bits, so a value longer than
    /// `PID_SHORT_MAX_LEN` cannot state its length there and §7.4.1.2.3
    /// requires the extended form. Which values those are is a property
    /// of the *sample* and not of the type: a `string` member is short
    /// in almost every sample and over 64 KiB in some.
    ///
    /// The plan used to answer for it by giving every unbounded member
    /// the extended header, on the reasoning that a plan cannot promise
    /// the value will fit. That is true of a plan and not of a writer:
    /// the length is patched here, after the value, and so is the form.
    /// Consequence, measured: `mut_string` is 24 bytes from RTI and from
    /// Fast DDS and was 32 from this project under either dialect —
    /// twelve bytes of header where the peer writes four, on every
    /// `string` and every `sequence` of every `@mutable` XCDR1 type.
    ///
    /// Eight bytes are inserted between the header and the value and the
    /// four fields are written over them. The value's own bytes do not
    /// change: XCDR1's `MAXALIGN` is eight, the insertion is eight, so
    /// every offset inside the value keeps its remainder and no padding
    /// it already contains was computed against an offset that moved.
    /// The trailing `align_to(4)` below is likewise unaffected, eight
    /// being a multiple of four.
    fn promote_pid(&mut self, header: &Pid, length: usize) {
        let start = header.at - 2;
        self.out.splice(header.at + 2..header.at + 2, [0u8; 8]);
        self.shift_padding(header.at + 2, 8_isize);

        let marker = PID_EXTENDED
            | self
                .pl_cdr1
                .pid_flags
                .flags_at(WirePosition::ExtendedMarker);
        let marker = self.order2(marker);
        self.out[start..start + 2].copy_from_slice(&marker);
        let slength = self.order2(8);
        self.out[start + 2..start + 4].copy_from_slice(&slength);

        let member_header = if header.must_understand && self.pl_cdr1.member_must_understand {
            (header.pid & E_MEMBER_ID_MASK) | E_FLAG_MUST_UNDERSTAND
        } else {
            header.pid & E_MEMBER_ID_MASK
        };
        let member_header = self.order4(member_header);
        self.out[start + 4..start + 8].copy_from_slice(&member_header);

        #[allow(clippy::cast_possible_truncation)]
        let llength = self.order4(length as u32);
        self.out[start + 8..start + 12].copy_from_slice(&llength);
    }

    /// The `PID_LIST_END` sentinel a PL_CDR1 parameter list ends with.
    ///
    /// A reader that stops at the last member it recognises instead of
    /// at the sentinel misreads whatever follows.
    /// The flags come from [`PlCdr1Framing::pid_flags`] asked at
    /// [`WirePosition::Sentinel`], which is the only thing that decides
    /// them: `PID_LIST_END` is not a member and has no declaration of
    /// its own to carry.
    pub fn write_sentinel(&mut self) {
        self.align_to(4);
        let sentinel = PID_LIST_END | self.pl_cdr1.pid_flags.flags_at(WirePosition::Sentinel);
        self.write_u16(sentinel);
        self.write_u16(0);
    }

    // -- collections -----------------------------------------------------

    /// A sequence: a four-byte count, then each element.
    ///
    /// The element function is fallible, and every element function is,
    /// whether or not the element itself can refuse. Three of the four
    /// element kinds a collection can hold *can* — a `char` outside
    /// Latin-1 (§7.2.2.2.1.2.2), a `wchar` outside the BMP, and a
    /// nested aggregate whose own `encode` reaches one — and a second
    /// infallible overload beside this one would be a shape for the
    /// generator to pick between, one construct at a time. `Ok(…)`
    /// around an infallible write costs nothing and reads as what it
    /// is.
    ///
    /// # Errors
    /// Whatever the element function returns.
    pub fn write_sequence<T, F: Fn(&mut Self, &T) -> Result<(), WireError>>(
        &mut self,
        values: &[T],
        element: F,
    ) -> Result<(), WireError> {
        #[allow(clippy::cast_possible_truncation)]
        self.write_u32(values.len() as u32);
        for value in values {
            element(self, value)?;
        }
        Ok(())
    }

    /// An array: each element, with no count.
    ///
    /// The element count is in the type, not on the wire.
    /// XTypes 1.3 §7.4.3.5.
    ///
    /// # Errors
    /// As [`Writer::write_sequence`].
    pub fn write_array<T, F: Fn(&mut Self, &T) -> Result<(), WireError>>(
        &mut self,
        values: &[T],
        element: F,
    ) -> Result<(), WireError> {
        for value in values {
            element(self, value)?;
        }
        Ok(())
    }

    /// A sequence behind a delimiter.
    ///
    /// XTypes 1.3 §7.4.3.5 rule (12): under XCDR2 a sequence whose
    /// element type is not primitive carries a `DHEADER` ahead of the
    /// length. Which sequences those are is
    /// `zerodds_ast_layout::collection_is_delimited`, not a decision
    /// taken here.
    ///
    /// # Errors
    /// As [`Writer::write_sequence`]. The delimiter is patched before
    /// the error is returned only when there is no error: a refused
    /// element leaves a partial buffer, which the caller discards.
    pub fn write_delimited_sequence<T, F: Fn(&mut Self, &T) -> Result<(), WireError>>(
        &mut self,
        values: &[T],
        element: F,
    ) -> Result<(), WireError> {
        let dheader = self.write_dheader();
        self.write_sequence(values, element)?;
        self.try_finish_dheader(dheader)
    }

    /// An array behind a delimiter. Rule (9), the array counterpart.
    ///
    /// # Errors
    /// As [`Writer::write_delimited_sequence`].
    pub fn write_delimited_array<T, F: Fn(&mut Self, &T) -> Result<(), WireError>>(
        &mut self,
        values: &[T],
        element: F,
    ) -> Result<(), WireError> {
        let dheader = self.write_dheader();
        self.write_array(values, element)?;
        self.try_finish_dheader(dheader)
    }

    /// A `sequence<T, N>`, whose count is checked against `N`.
    ///
    /// The bytes are a plain sequence's: Table 17's
    /// `O2.length <= T1.length` is a property of the *value*, and the
    /// wire carries no bound to compare it with. Refused rather than
    /// written short, for the reason [`Writer::write_string_bounded`]
    /// gives — the receiver's own `sequence<T, N>` has room for `N`, and
    /// a sender that wrote more has written a value of a type nobody
    /// declared.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for more than `bound` elements, and
    /// whatever the element function returns.
    pub fn write_sequence_bounded<T, F: Fn(&mut Self, &T) -> Result<(), WireError>>(
        &mut self,
        values: &[T],
        bound: usize,
        element: F,
    ) -> Result<(), WireError> {
        self.check_sequence_bound(values.len(), bound)?;
        self.write_sequence(values, element)
    }

    /// A bounded sequence behind a delimiter. Rule (12).
    ///
    /// # Errors
    /// As [`Writer::write_sequence_bounded`].
    pub fn write_delimited_sequence_bounded<T, F: Fn(&mut Self, &T) -> Result<(), WireError>>(
        &mut self,
        values: &[T],
        bound: usize,
        element: F,
    ) -> Result<(), WireError> {
        self.check_sequence_bound(values.len(), bound)?;
        self.write_delimited_sequence(values, element)
    }

    /// The bound check both bounded-sequence writers make, before any
    /// byte of the value is written.
    fn check_sequence_bound(&self, length: usize, bound: usize) -> Result<(), WireError> {
        if length > bound {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: alloc::format!(
                    "at most {bound} elements in a bounded sequence, and this one has {length}"
                ),
            });
        }
        Ok(())
    }

    /// A map: a four-byte **entry** count, then each entry's key
    /// followed by its value.
    ///
    /// # The count is entries and not halves
    ///
    /// XTypes 1.3 §7.4.1.1.4 defines a `map<K, V>` by equivalence to a
    /// `sequence<Entry>` whose element is the pair, so the `O.length`
    /// that precedes it counts pairs. Two independent readings say the
    /// same:
    ///
    /// - the layout plan. `zerodds_ast_layout::xcdr`'s `TypeKind::Map`
    ///   arm gives the map `max(key, value, 4)` alignment and one
    ///   `SEQUENCE_LENGTH_ALIGNMENT` length word, not one per half;
    /// - a recorded foreign measurement.
    ///   `tests/interop-results/sources/vendor-corpus.tsv`, case
    ///   `map-key-string` under `fastdds`, XCDR1 big-endian, is
    ///   `00000003` followed by three `(string, long)` pairs and then
    ///   the `ffffffff` of the trailing `long tail`. Three entries, six
    ///   halves, and the word on the wire is `3`.
    ///
    /// # The entries go out in the order the container holds them
    ///
    /// This runtime takes a `BTreeMap` and walks it, so the order is
    /// ascending by key — which is what
    /// `zerodds_ast_layout::MapEntryOrder::AscendingKey` is defined as:
    /// what a binding that holds a `map` in an ordered container writes.
    /// The order is a property of the container the caller already
    /// chose, not a second decision taken here.
    ///
    /// Whether a delimited map *also* carries this count is the one
    /// thing the specification says twice and differently — §7.4.1.1.4
    /// against rule (15) of §7.4.3.5.3 — and it is the caller's, through
    /// which of these four functions it calls. Fast DDS writes both,
    /// which is `zerodds_wire_codec::value::MapLengthPolicy`'s default
    /// and what [`Writer::write_delimited_map`] does.
    ///
    /// # Errors
    /// Whatever either half's function returns.
    pub fn write_map<M, K, V, FK, FV>(
        &mut self,
        entries: &M,
        key: FK,
        value: FV,
    ) -> Result<(), WireError>
    where
        M: crate::WireMap<K, V> + ?Sized,
        FK: Fn(&mut Self, &K) -> Result<(), WireError>,
        FV: Fn(&mut Self, &V) -> Result<(), WireError>,
    {
        #[allow(clippy::cast_possible_truncation)]
        self.write_u32(entries.entry_count() as u32);
        entries.try_visit(|k, v| {
            key(self, k)?;
            value(self, v)
        })
    }

    /// A map behind a delimiter.
    ///
    /// XTypes 1.3 §7.4.3.5 rule (14) exempts a map from the `DHEADER`
    /// only when **both** halves are primitive, and rule (15) delimits
    /// every other map under XCDR2. Which maps those are is
    /// `zerodds_ast_layout`'s `TypeLayout::needs_dheader`, read through
    /// `MapDelimiter`, and not a decision taken here.
    ///
    /// # Errors
    /// As [`Writer::write_map`].
    pub fn write_delimited_map<M, K, V, FK, FV>(
        &mut self,
        entries: &M,
        key: FK,
        value: FV,
    ) -> Result<(), WireError>
    where
        M: crate::WireMap<K, V> + ?Sized,
        FK: Fn(&mut Self, &K) -> Result<(), WireError>,
        FV: Fn(&mut Self, &V) -> Result<(), WireError>,
    {
        let dheader = self.write_dheader();
        self.write_map(entries, key, value)?;
        self.try_finish_dheader(dheader)
    }

    /// A `map<K, V, N>`, whose entry count is checked against `N`.
    ///
    /// The bytes are a plain map's, for the reason
    /// [`Writer::write_sequence_bounded`] gives: the wire carries no
    /// bound to compare with, so an over-long value is refused rather
    /// than written short.
    ///
    /// # Errors
    /// [`WireError::OutOfRange`] for more than `bound` entries, and
    /// whatever either half's function returns.
    pub fn write_map_bounded<M, K, V, FK, FV>(
        &mut self,
        entries: &M,
        bound: usize,
        key: FK,
        value: FV,
    ) -> Result<(), WireError>
    where
        M: crate::WireMap<K, V> + ?Sized,
        FK: Fn(&mut Self, &K) -> Result<(), WireError>,
        FV: Fn(&mut Self, &V) -> Result<(), WireError>,
    {
        self.check_map_bound(entries.entry_count(), bound)?;
        self.write_map(entries, key, value)
    }

    /// A bounded map behind a delimiter. Rule (15).
    ///
    /// # Errors
    /// As [`Writer::write_map_bounded`].
    pub fn write_delimited_map_bounded<M, K, V, FK, FV>(
        &mut self,
        entries: &M,
        bound: usize,
        key: FK,
        value: FV,
    ) -> Result<(), WireError>
    where
        M: crate::WireMap<K, V> + ?Sized,
        FK: Fn(&mut Self, &K) -> Result<(), WireError>,
        FV: Fn(&mut Self, &V) -> Result<(), WireError>,
    {
        self.check_map_bound(entries.entry_count(), bound)?;
        self.write_delimited_map(entries, key, value)
    }

    /// The bound check both bounded-map writers make, before any byte of
    /// the value is written.
    ///
    /// Its own message and not `check_sequence_bound`'s: the
    /// unit is an entry and not an element, and a diagnostic that
    /// counted a `map<string, long>` in elements would name a number
    /// twice the one the declaration bounds.
    fn check_map_bound(&self, entries: usize, bound: usize) -> Result<(), WireError> {
        if entries > bound {
            return Err(WireError::OutOfRange {
                at: self.out.len(),
                expected: alloc::format!(
                    "at most {bound} entries in a bounded map, and this one has {entries}"
                ),
            });
        }
        Ok(())
    }

    // -- the KeyHash -----------------------------------------------------

    /// The holder as a KeyHash, zero-padded to sixteen bytes.
    ///
    /// For a holder of sixteen bytes or fewer. DDSI-RTPS 2.5 §9.6.3.8.
    #[must_use]
    pub fn zero_padded(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        let take = self.out.len().min(16);
        out[..take].copy_from_slice(&self.out[..take]);
        out
    }

    /// The MD5 of the holder, for one longer than sixteen bytes.
    #[must_use]
    pub fn md5(&self) -> [u8; 16] {
        crate::md5(&self.out)
    }

    /// `bytes` as they are, with no alignment and no length in front.
    ///
    /// The counterpart of [`crate::reader::Reader::read_bytes`], and the
    /// one way an already-encoded byte string goes into a stream: a
    /// CDR encapsulation built by a second writer, a GIOP request body
    /// whose type only the calling stub knows, the octets of a
    /// `sequence<octet>` behind their own length.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }

    // -- internals -------------------------------------------------------

    fn extend(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }

    fn order2(&self, value: u16) -> [u8; 2] {
        if self.endianness.is_little() {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        }
    }

    fn order4(&self, value: u32) -> [u8; 4] {
        if self.endianness.is_little() {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        }
    }

    fn order8(&self, value: u64) -> [u8; 8] {
        if self.endianness.is_little() {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        }
    }
}

/// The value of a DHEADER for members that occupy `span` octets.
///
/// XTypes 1.3 §7.4.3.4.1: "O.ssize is constrained to being smaller than
/// 4 Giga Bytes", because a DHEADER is a `UInt32`.
///
/// # Errors
/// [`WireError::OutOfRange`] naming the offset of the DHEADER, for a span
/// of 4 GiB or more.
pub fn checked_dheader_length(span: usize, at: usize) -> Result<u32, WireError> {
    u32::try_from(span).map_err(|_| WireError::OutOfRange {
        at,
        expected: alloc::string::String::from(
            "an object smaller than 4 GiB, which a DHEADER can hold (XTypes 1.3 7.4.3.4.1)",
        ),
    })
}

/// A DHEADER waiting for its length.
#[derive(Debug, Clone, Copy)]
pub struct DHeader {
    at: usize,
}

/// An EMHEADER waiting for its NEXTINT, when it has one.
#[derive(Debug, Clone, Copy)]
pub struct EmHeader {
    /// Where the EMHEADER word itself sits, so a short code can be
    /// patched into it once the value's length is known.
    word: usize,
    next_int: Option<usize>,
    /// The code the header was written with, so that
    /// [`Writer::finish_emheader`] can hold the member's value to the
    /// length the code declares. Codes 0 to 3 are lengths; anything else
    /// declares nothing of its own.
    length_code: u8,
}

/// A parameter header waiting for its length.
#[derive(Debug, Clone, Copy)]
pub struct Pid {
    at: usize,
    extended: bool,
    /// The member id, kept so that a short header can be rewritten as an
    /// extended one when the value turns out not to fit.
    pid: u32,
    /// Whether the member declared `must_understand`, for the same
    /// reason: the bit is in a different word in the two forms.
    must_understand: bool,
    /// Which length convention the patch will follow.
    role: ParameterRole,
    /// The stream origin this parameter's block displaced.
    saved_origin: usize,
}

/// The parameter id that introduces the extended form.
pub const PID_EXTENDED: u16 = 0x3F01;

/// The parameter id that ends a parameter list.
pub const PID_LIST_END: u16 = 0x3F02;

/// The highest member id a short parameter header can carry.
///
/// §7.4.1.2.2 gives the short form's range as "from 0 to 16,128
/// (0x3F00) inclusive", and `PID_EXTENDED` is 0x3F01 — so 0x3F00 is the
/// last usable id and not the first reserved one. It was named for an
/// exclusive threshold and read as one, which gave a member with that
/// id the twelve-octet header where RTI Connext and Fast DDS both write
/// the four-octet one (case `pid_3f00`, 2026-08-10).
pub const PID_SHORT_MAX_ID: u16 = 0x3F00;

/// The longest value a short parameter header can state the length of.
///
/// §7.4.1.2.2 gives `ssize` sixteen bits. A value longer than this
/// cannot be described by the short form whatever its member id is, and
/// `Writer::finish_pid` rewrites the header rather than truncating the
/// length into two bytes. `zerodds_ast_layout` has the same constant for
/// the plan's half of the same rule.
pub const PID_SHORT_MAX_LEN: usize = 0xFFFF;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    /// XTypes 1.3 §7.4.3.4.1: "O.ssize is constrained to being smaller than
    /// 4 Giga Bytes". `finish_dheader` cannot return the error, so it
    /// records it, and the placeholder is not patched with a length modulo
    /// 2^32.
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn an_infallible_finish_dheader_over_4_gib_is_deferred_and_not_truncated() {
        let mut w = Writer::little_endian();
        let header = w.write_dheader();
        w.write_u32(7);
        w.finish_dheader_spanning(header, u32::MAX as usize + 1);
        assert!(
            matches!(
                w.deferred_error(),
                Some(WireError::OutOfRange { at: 0, .. })
            ),
            "{:?}",
            w.deferred_error()
        );
        assert!(w.check_deferred().is_err());
        assert_eq!(
            &w.as_bytes()[..4],
            &[0, 0, 0, 0],
            "a length modulo 2^32 was written"
        );
        assert!(matches!(
            w.finish_checked(),
            Err(WireError::OutOfRange { .. })
        ));
    }

    #[test]
    fn a_dheader_below_4_gib_is_patched_and_nothing_is_deferred() {
        let mut w = Writer::little_endian();
        let header = w.write_dheader();
        w.write_u32(7);
        w.finish_dheader(header);
        assert!(w.deferred_error().is_none());
        assert_eq!(
            w.finish_checked().expect("no error"),
            [4, 0, 0, 0, 7, 0, 0, 0]
        );
    }
}
