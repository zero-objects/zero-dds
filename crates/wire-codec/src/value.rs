// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What a probe is asked for.

use zerodds_ast::Extensibility;
use zerodds_ast_layout::{
    AggregateDelimiter, CollectionDelimiterPolicy, DefaultedDiscriminatorMember, Dialect,
    ElementPadding, Encoding, Endianness, EnumHolderWidth, EnumeratorValue, FixedDigitCount,
    KeyHolderAlignment, KeyHolderExtensibility, KeyHolderNesting, KeyHolderOrder, LayoutKey,
    LengthCodePolicy, MapDelimiter, MapEntryOrder, MustUnderstandPolicy, MutableUnionFraming,
    PidFlagPolicy, PlCdr1HeaderPolicy, PlCdr1Reading, Purpose, SequenceElementDelimiter,
    ShortPidRange, UnionLabelMatch, WcharFraming, WcharWidth, WstringCodeUnits,
    WstringLengthPolicy, WstringUnitOrder,
};
use zerodds_wire::{
    PidFlagPolicy as WirePidFlagPolicy, PlCdr1Framing, PlCdr1Reading as WirePlCdr1Reading,
    WcharFraming as WireWcharFraming, WcharWidth as WireWcharWidth,
    WstringUnitOrder as WireWstringUnitOrder,
};

/// Whether a delimited `map` carries an entry count.
///
/// This policy exists because XTypes 1.3 contradicts itself, not
/// because two implementations were measured disagreeing. Nothing in
/// `crates/ast-layout/tests/fixtures/vendor_bytes.md` has a map in it,
/// so neither shape below is verified against a vendor.
///
/// §7.4.1.1.4 defines a map by an equivalence: it "shall be serialized
/// as if" it were `sequence<MapEntry_<key_type>_<value_type>>`. A
/// sequence under XCDR2 is rule (12) of §7.4.3.5.3, `DHEADER` then
/// `O.length` then the elements. So the equivalence yields both words.
///
/// Rule (15) of the same clause, which is the rule written for
/// `MAP_TYPE` itself under XCDR2, writes `DHEADER` and then the pairs —
/// with no `O.length`. Rules (14) and (16), the primitive-map and the
/// XCDR1 cases, both write `O.length` and no delimiter, so the omission
/// is only in (15).
///
/// The two readings differ by four bytes. Both are decodable — with a
/// delimiter a reader can consume entries until the byte extent is
/// used — which is why neither can be ruled out from the stream alone.
///
/// # Both are in the field
///
/// Measured on 2026-08-09. Neither C generator parses `map`, so the
/// implementations that can be asked are eProsima Fast DDS and OpenDDS
/// 3.34.0, and they took one reading each. `@final struct { map<long,
/// Cell> entries; long tail; }` with one entry `5 -> {1, 2}`, XCDR2
/// little endian: Fast DDS writes `10000000 01000000 05000000 01000000
/// 02000000 07000000` and OpenDDS writes `0c000000 05000000 01000000
/// 02000000 07000000`. Under XCDR1, where neither delimits, the two are
/// identical. `crates/ast-layout/tests/fixtures/vendor_bytes.md`,
/// "Where the three cannot answer", has the run and a second shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MapLengthPolicy {
    /// Write the entry count whether or not a delimiter precedes it.
    ///
    /// The default, because it is the reading under which §7.4.1.1.4's
    /// equivalence holds, because it is the only one of the two that
    /// makes a map's bytes the same shape as the sequence the clause
    /// says it is, and because it is what Fast DDS writes.
    #[default]
    SequenceEquivalent,
    /// Omit the entry count when the map is delimited, as rule (15) is
    /// written.
    ///
    /// OpenDDS's reading: its generated `operator<<` calls
    /// `write_delimiter` under XCDR2 and the `<< length` in the `else`
    /// branch under XCDR1, so the count and the delimiter are
    /// alternatives rather than both.
    Rule15Omits,
}

impl MapLengthPolicy {
    /// Both readings.
    pub const ALL: [Self; 2] = [Self::SequenceEquivalent, Self::Rule15Omits];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::SequenceEquivalent)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SequenceEquivalent => "sequence-equivalent",
            Self::Rule15Omits => "rule-15-omits",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than the default: a typo
    /// in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|policy| policy.as_str() == value)
    }
}

/// What the probe writes out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Emit {
    /// The serialised sample, with no encapsulation header.
    ///
    /// The default, because it is the part two implementations have to
    /// agree on byte for byte. The header says which encoding was
    /// used, which the caller already chose.
    #[default]
    Payload,
    /// The sample behind its four-byte encapsulation header.
    PayloadWithEncapsulation,
    /// The sixteen-byte KeyHash. DDSI-RTPS 2.5 §9.6.3.8.
    KeyHash,
}

impl Emit {
    /// Every form.
    pub const ALL: [Self; 3] = [Self::Payload, Self::PayloadWithEncapsulation, Self::KeyHash];

    /// The name the command line uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Payload => "payload",
            Self::PayloadWithEncapsulation => "payload+encapsulation",
            Self::KeyHash => "keyhash",
        }
    }

    /// Parse a command-line value, rejecting anything else.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|form| form.as_str() == value)
    }
}

/// One probe run.
///
/// Every field a foreign probe also takes, so the four command lines
/// are the same shape. A field that only one implementation
/// understands would make the matrix unreadable.
#[derive(Debug, Clone)]
pub struct ProbeRequest {
    /// The type, by scoped name — `common::Robot`.
    pub type_name: String,
    /// XCDR1 or XCDR2.
    pub encoding: Encoding,
    /// Byte order.
    pub endianness: Endianness,
    /// What to write out.
    pub emit: Emit,
    /// How variable-length `@mutable` members are framed.
    pub length_codes: LengthCodePolicy,
    /// Which collections carry a delimiter.
    pub collection_delimiters: CollectionDelimiterPolicy,
    /// Which of a `map`'s halves decides whether the map is delimited.
    pub map_delimiter: MapDelimiter,
    /// How a `wstring`'s length is counted.
    pub wstring_length: WstringLengthPolicy,
    /// How a scalar `wchar` is framed.
    pub wchar_framing: WcharFraming,
    /// How wide a scalar `wchar` is.
    pub wchar_width: WcharWidth,
    /// In what byte order a `wstring`'s code units go out.
    pub wstring_unit_order: WstringUnitOrder,
    /// How a `@mutable` member is framed under XCDR1.
    pub pl_cdr1_headers: PlCdr1HeaderPolicy,
    /// What flags a PL_CDR1 parameter id carries, per position.
    pub pid_flags: PidFlagPolicy,
    /// How tolerant the PL_CDR1 reader is of another framing.
    pub pl_cdr1_reading: PlCdr1Reading,
    /// Which table the identifier of an encapsulation header is from.
    pub encapsulation_ids: EncapsulationIds,
    /// Whether a member's `must_understand` reaches the wire.
    pub must_understand: MustUnderstandPolicy,
    /// How wide an enumeration's holder is.
    pub enum_holder: EnumHolderWidth,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    pub union_label: UnionLabelMatch,
    /// In what order a `map`'s entries go on the wire.
    pub map_entries: MapEntryOrder,
    /// Whether an array's element is flattened before the delimiter
    /// question. See `ArrayElementFlattening`.
    pub array_element_flattening: zerodds_ast_layout::key::ArrayElementFlattening,
    /// Which integer an enumerator serialises as.
    pub enumerator_value: EnumeratorValue,
    /// How a `@mutable` union numbers its branches, and whether its
    /// XCDR1 parameter list is terminated.
    pub union_framing: MutableUnionFraming,
    /// Which member ids fit a PL_CDR1 short parameter header.
    pub short_pid_range: ShortPidRange,
    /// Whether a `fixed<P,S>` carries its declared digit count or the
    /// value's.
    pub fixed_digits: FixedDigitCount,
    /// What an aggregate whose source said nothing is taken to be.
    pub default_extensibility: Extensibility,
    /// Whether a key holder keeps the type's extensibility.
    pub key_holder: KeyHolderExtensibility,
    /// How deep a key holder's member removal reaches.
    pub key_holder_nesting: KeyHolderNesting,
    /// In what order a key holder's members are serialized.
    pub key_holder_order: KeyHolderOrder,
    /// Whether a key holder's members are aligned.
    pub key_holder_alignment: KeyHolderAlignment,
    /// What becomes of a union's member when `@try_construct(use_default)`
    /// defaulted its discriminator.
    pub defaulted_discriminator_member: DefaultedDiscriminatorMember,
    /// Whether a delimited `map` also carries an entry count.
    pub map_length: MapLengthPolicy,
    /// Whether a collection's elements carry alignment padding between
    /// them.
    pub element_padding: ElementPadding,
    /// Whether an aggregate's declared extensibility reaches the wire.
    pub aggregate_delimiter: AggregateDelimiter,
    /// Whether a `sequence` element of a non-`@final` type is preceded
    /// by its own octet length.
    pub sequence_element_delimiter: SequenceElementDelimiter,
    /// Which octets stand for a `wstring`'s characters.
    pub wstring_code_units: WstringCodeUnits,
}

impl ProbeRequest {
    /// A request for `type_name` in the given encoding and byte order,
    /// in the default dialect.
    #[must_use]
    pub fn new(type_name: &str, encoding: Encoding, endianness: Endianness) -> Self {
        Self::in_dialect(type_name, encoding, endianness, Dialect::default())
    }

    /// A request with every wire policy taken from one dialect.
    #[must_use]
    pub fn in_dialect(
        type_name: &str,
        encoding: Encoding,
        endianness: Endianness,
        dialect: Dialect,
    ) -> Self {
        let key = LayoutKey::in_dialect(dialect, encoding, endianness, Purpose::Data);
        let map_length = if dialect.map_count_omitted_when_delimited() {
            MapLengthPolicy::Rule15Omits
        } else {
            MapLengthPolicy::SequenceEquivalent
        };
        Self::from_key(type_name, key, map_length)
    }

    /// A request for `type_name` planned under `key`, with `map_length`
    /// standing in for the one framing question `key` does not answer
    /// (see [`MapLengthPolicy`]'s own documentation for why the entry
    /// count is a codec question and not a layout one).
    ///
    /// The field-by-field mapping from a `LayoutKey` to a `ProbeRequest`
    /// lives here once. [`Self::in_dialect`] is this plus the one
    /// `LayoutKey::in_dialect` call a dialect alone needs; a caller that
    /// has already built a key — a dialect with axis flags overriding
    /// some of its answers, as `zerodds_ast_layout::axes` produces —
    /// calls this directly instead of re-deriving the same fields by
    /// hand.
    #[must_use]
    pub fn from_key(type_name: &str, key: LayoutKey, map_length: MapLengthPolicy) -> Self {
        Self {
            type_name: type_name.to_string(),
            encoding: key.encoding,
            endianness: key.endianness,
            emit: Emit::Payload,
            length_codes: key.length_codes,
            collection_delimiters: key.collection_delimiters,
            map_delimiter: key.map_delimiter,
            wstring_length: key.wstring_length,
            wchar_framing: key.wchar_framing,
            wchar_width: key.wchar_width,
            wstring_unit_order: key.wstring_unit_order,
            pl_cdr1_headers: key.pl_cdr1_headers,
            pid_flags: key.pid_flags,
            pl_cdr1_reading: key.pl_cdr1_reading,
            encapsulation_ids: EncapsulationIds::default(),
            must_understand: key.must_understand,
            enum_holder: key.enum_holder,
            union_label: key.union_label,
            map_entries: key.map_entries,
            array_element_flattening: key.array_element_flattening,
            enumerator_value: key.enumerator_value,
            union_framing: key.union_framing,
            short_pid_range: key.short_pid_range,
            fixed_digits: key.fixed_digits,
            default_extensibility: key.default_extensibility,
            key_holder: key.key_holder,
            key_holder_nesting: key.key_holder_nesting,
            key_holder_order: key.key_holder_order,
            key_holder_alignment: key.key_holder_alignment,
            defaulted_discriminator_member: key.defaulted_discriminator_member,
            map_length,
            element_padding: key.element_padding,
            aggregate_delimiter: key.aggregate_delimiter,
            sequence_element_delimiter: key.sequence_element_delimiter,
            wstring_code_units: key.wstring_code_units,
        }
    }

    /// The same request writing something else out.
    #[must_use]
    pub fn emitting(mut self, emit: Emit) -> Self {
        self.emit = emit;
        self
    }

    /// The same request with a PL_CDR1 reader that takes, or reports, a
    /// stream in another framing.
    #[must_use]
    pub fn with_pl_cdr1_reading(mut self, reading: PlCdr1Reading) -> Self {
        self.pl_cdr1_reading = reading;
        self
    }

    /// The same request writing the identifiers of an encapsulation header
    /// from another table.
    #[must_use]
    pub fn with_encapsulation_ids(mut self, ids: EncapsulationIds) -> Self {
        self.encapsulation_ids = ids;
        self
    }

    /// How tolerant the PL_CDR1 reader is, as the byte layer names it.
    #[must_use]
    pub const fn wire_pl_cdr1_reading(&self) -> WirePlCdr1Reading {
        match self.pl_cdr1_reading {
            PlCdr1Reading::Strict => WirePlCdr1Reading::Strict,
            PlCdr1Reading::Tolerant => WirePlCdr1Reading::Tolerant,
        }
    }

    /// The layout key this request plans under.
    ///
    /// One place, so that the write direction, the read direction and
    /// the key holder cannot each assemble a slightly different key out
    /// of the same fields.
    #[must_use]
    pub fn layout_key(&self, purpose: Purpose) -> LayoutKey {
        LayoutKey {
            encoding: self.encoding,
            endianness: self.endianness,
            purpose,
            length_codes: self.length_codes,
            collection_delimiters: self.collection_delimiters,
            map_delimiter: self.map_delimiter,
            wstring_length: self.wstring_length,
            wchar_framing: self.wchar_framing,
            wchar_width: self.wchar_width,
            wstring_unit_order: self.wstring_unit_order,
            pl_cdr1_headers: self.pl_cdr1_headers,
            pid_flags: self.pid_flags,
            pl_cdr1_reading: self.pl_cdr1_reading,
            must_understand: self.must_understand,
            enum_holder: self.enum_holder,
            union_label: self.union_label,
            map_entries: self.map_entries,
            array_element_flattening: self.array_element_flattening,
            enumerator_value: self.enumerator_value,
            union_framing: self.union_framing,
            short_pid_range: self.short_pid_range,
            fixed_digits: self.fixed_digits,
            default_extensibility: self.default_extensibility,
            key_holder: self.key_holder,
            key_holder_nesting: self.key_holder_nesting,
            key_holder_order: self.key_holder_order,
            key_holder_alignment: self.key_holder_alignment,
            defaulted_discriminator_member: self.defaulted_discriminator_member,
            element_padding: self.element_padding,
            aggregate_delimiter: self.aggregate_delimiter,
            sequence_element_delimiter: self.sequence_element_delimiter,
            wstring_code_units: self.wstring_code_units,
        }
    }

    /// The key-holder key this request plans under.
    ///
    /// The same dialect as the payload: §7.6.8 fixes the holder's byte
    /// order and encoding, not what an unannotated aggregate inside it
    /// is taken to be, and that decides whether the holder carries a
    /// DHEADER. A probe that wrote one dialect's payload and another's
    /// KeyHash would report a divergence of its own making.
    #[must_use]
    pub fn key_holder_key(&self) -> LayoutKey {
        LayoutKey {
            endianness: Endianness::Big,
            ..self.layout_key(Purpose::Key)
        }
    }

    /// How wide a `Char16` is for this request.
    #[must_use]
    pub const fn wchar_width(&self) -> WireWcharWidth {
        wire_wchar_width_of(self.wchar_width)
    }

    /// How a `Char16` is framed for this request, as the byte layer
    /// names it.
    #[must_use]
    pub const fn wire_wchar_framing(&self) -> WireWcharFraming {
        wire_wchar_framing_of(self.wchar_framing)
    }

    /// In what byte order wide code units go out, as the byte layer
    /// names it.
    #[must_use]
    pub const fn wire_wstring_unit_order(&self) -> WireWstringUnitOrder {
        match self.wstring_unit_order {
            WstringUnitOrder::Stream => WireWstringUnitOrder::Stream,
            WstringUnitOrder::BigEndianAlways => WireWstringUnitOrder::BigEndianAlways,
            WstringUnitOrder::ByteOrderMark => WireWstringUnitOrder::ByteOrderMark,
            WstringUnitOrder::HostUnswapped => WireWstringUnitOrder::HostUnswapped,
        }
    }

    /// How a writer serving this request frames a PL_CDR1 parameter
    /// list.
    ///
    /// The header *form* is in the plan, because it changes a member's
    /// extent; these four do not, so they ride on the writer. Both
    /// come from the same policy, which is what keeps them agreeing.
    #[must_use]
    pub fn pl_cdr1_framing(&self) -> PlCdr1Framing {
        let values = self.layout_key(Purpose::Data).pl_cdr1_framing();
        PlCdr1Framing {
            pid_flags: wire_pid_flags_of(values.pid_flags),
            highest_short_id: values.highest_short_id,
            pad_declared_length: values.pad_declared_length,
            pad_declared_optional_length: values.pad_declared_optional_length,
            member_must_understand: values.member_must_understand,
            require_extended: values.require_extended,
            pad_after_value: values.pad_after_value,
        }
    }
}

/// The byte layer's name for a `Char16` framing.
///
/// A free function as well as a method because the key-holder writer is
/// built from a `LayoutKey` and not from a `ProbeRequest`, and a holder
/// whose framing disagreed with its own plan would put every member
/// after a `wchar` in the wrong place.
///
/// The position axis is not resolved here. Both layers carry all three
/// variants, so the writer answers the position at the call and this
/// function stays a rename.
#[must_use]
pub const fn wire_wchar_framing_of(framing: WcharFraming) -> WireWcharFraming {
    match framing {
        WcharFraming::Fixed => WireWcharFraming::Fixed,
        WcharFraming::GiopLengthPrefixed => WireWcharFraming::GiopLengthPrefixed,
        WcharFraming::GiopLengthPrefixedMemberOnly => {
            WireWcharFraming::GiopLengthPrefixedMemberOnly
        }
    }
}

/// The byte layer's name for a wide code-unit order. As
/// [`wire_wchar_framing_of`].
#[must_use]
pub const fn wire_wstring_unit_order_of(order: WstringUnitOrder) -> WireWstringUnitOrder {
    match order {
        WstringUnitOrder::Stream => WireWstringUnitOrder::Stream,
        WstringUnitOrder::BigEndianAlways => WireWstringUnitOrder::BigEndianAlways,
        WstringUnitOrder::ByteOrderMark => WireWstringUnitOrder::ByteOrderMark,
        WstringUnitOrder::HostUnswapped => WireWstringUnitOrder::HostUnswapped,
    }
}

/// The byte layer's name for a PID-flag policy. As
/// [`wire_wchar_framing_of`].
///
/// The position is not resolved here either: both layers carry all the
/// positions, the writer and the reader ask at the call, and this is a
/// rename.
#[must_use]
pub const fn wire_pid_flags_of(policy: PidFlagPolicy) -> WirePidFlagPolicy {
    match policy {
        PidFlagPolicy::SentinelMustUnderstand => WirePidFlagPolicy::SentinelMustUnderstand,
        PidFlagPolicy::Bare => WirePidFlagPolicy::Bare,
        PidFlagPolicy::ImplExtensionOnMembers => WirePidFlagPolicy::ImplExtensionOnMembers,
    }
}

/// A scalar `wchar`'s width, as the byte layer names it.
///
/// It used to be read off [`WstringLengthPolicy::char16_size`], on the
/// reading that the scalar and the `wstring`'s code units cannot
/// disagree. They can: `rmw_cyclonedds_cpp` writes two octets for a
/// `wchar` member and four for a `wstring`'s units, so the width is its
/// own axis and this only carries it down to `zerodds-wire`, which
/// knows nothing about a policy.
#[must_use]
pub const fn wire_wchar_width_of(width: WcharWidth) -> WireWcharWidth {
    match width {
        WcharWidth::Two => WireWcharWidth::Two,
        WcharWidth::Four => WireWcharWidth::Four,
    }
}

/// How many zero bytes bring `length` up to the next multiple of four.
///
/// XTypes 1.3 §7.4.1.1: the serialized data is padded to a four-byte
/// boundary, and §7.6.3.1.2 puts the count of those bytes in the
/// encapsulation options so a receiver can find where the data ended.
/// Measured on the wire, 2026-08-09: Cyclone, RTI and Fast DDS all pad
/// the payload and all three state the count.
#[must_use]
pub const fn encapsulation_padding(length: usize) -> u8 {
    // `& 3` and not a modulo of the length: the count is what the two
    // bits can hold, and the two bits are the field.
    ((4 - (length & 3)) & 3) as u8
}

/// The encapsulation header for an encoding and byte order.
///
/// DDSI-RTPS 2.5 §10.5 and XTypes 1.3 §7.4.1.1: the identifier, then
/// two option bytes carrying the payload's trailing padding count.
///
/// # The options field is not decoration
///
/// §7.6.3.1.2 makes the low two bits of the second options byte the
/// number of padding bytes at the end of the serialized data, and says
/// they "shall be interpreted by the receiver to determine where the
/// serialized data exactly ended". Leaving them zero over a padded
/// payload tells a receiver that reads them that the padding is data.
///
/// Measured on the loopback wire on 2026-08-09, `@final struct One {
/// octet a; }` with `a = 0xAB`, captured with `tshark`:
///
/// ```text
/// cyclone   CDR_LE  options 0x0003  serializedData ab000000
/// rti       CDR_LE  options 0x0003  serializedData ab000000
/// fastdds   CDR_LE  options 0x0003  serializedData ab000000
/// ```
///
/// and `@final struct TypeB { short a; char b; }` — §7.6.3.1.2's own
/// example — through Cyclone: `options 0x0001`, `serializedData
/// 23006200`. Fast CDR 2.3.5 `Cdr::set_dds_cdr_options` computes the
/// same count for bodies of 1, 2, 3 and 4 bytes (3, 2, 1, 0), and
/// Cyclone's `dds_serdata_default.c` writes `ddsrt_toBE2u(pad)` into
/// `hdr.options` and reads the data back as `d->pos - pad`.
///
/// RTI's static `Plugin_serialize_to_cdr_buffer_ex` leaves the field
/// zero and does not pad; its RTPS layer sets both, which is what the
/// capture above shows. The field belongs to the encapsulated form, so
/// `Emit::Payload` — the unpadded serialized data, which is what every
/// row of `vendor_bytes.md` records — is unaffected.
///
/// The word is big-endian like the identifier: Cyclone writes
/// `ddsrt_toBE2u`, Fast CDR serialises `options_[0]` then `options_[1]`
/// whatever the payload's order, and both put the count in the second
/// byte.
#[must_use]
pub fn encapsulation_header(
    encoding: Encoding,
    endianness: Endianness,
    little: bool,
    padding: u8,
) -> [u8; 4] {
    encapsulation_header_in(
        EncapsulationIds::Table60,
        encoding,
        endianness,
        little,
        padding,
    )
}

/// Which table the identifiers of an encapsulation header are from.
///
/// DDS-XTypes 1.3 gives them twice, and the two tables do not agree for
/// encoding version 2 or for XML: Table 39 (§7.4.3.4, the function
/// `ENC_HEADER`) and DDSI-RTPS 2.5 Table 10.3 name `{0x00, 0x10}` to
/// `{0x00, 0x15}` and `{0x01, 0x00}`; Table 60 (§7.6.3.1.2) names
/// `{0x00, 0x06}` to `{0x00, 0x0b}` and `{0x00, 0x04}`. Cyclone DDS, RTI
/// Connext and Fast DDS were measured writing Table 60's, which is the
/// default; a reader here takes both
/// ([`encapsulation_form`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum EncapsulationIds {
    /// Table 60.
    #[default]
    Table60,
    /// Table 39.
    Table39,
}

impl EncapsulationIds {
    /// Both tables.
    pub const ALL: [Self; 2] = [Self::Table60, Self::Table39];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Table60)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Table60 => "table-60",
            Self::Table39 => "table-39",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than the default: a typo in
    /// a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ids| ids.as_str() == value)
    }

    /// The two identifier octets of an encoding and a byte order.
    #[must_use]
    pub const fn identifier(self, encoding: Encoding, endianness: Endianness) -> [u8; 2] {
        let id: u8 = match (self, encoding, endianness) {
            (_, Encoding::PlainCdr1, Endianness::Big) => 0x00,
            (_, Encoding::PlainCdr1, Endianness::Little) => 0x01,
            (_, Encoding::PlCdr1, Endianness::Big) => 0x02,
            (_, Encoding::PlCdr1, Endianness::Little) => 0x03,
            (Self::Table60, Encoding::PlainCdr2, Endianness::Big) => 0x06,
            (Self::Table60, Encoding::PlainCdr2, Endianness::Little) => 0x07,
            (Self::Table60, Encoding::DelimitedCdr, Endianness::Big) => 0x08,
            (Self::Table60, Encoding::DelimitedCdr, Endianness::Little) => 0x09,
            (Self::Table60, Encoding::PlCdr2, Endianness::Big) => 0x0a,
            (Self::Table60, Encoding::PlCdr2, Endianness::Little) => 0x0b,
            (Self::Table39, Encoding::PlainCdr2, Endianness::Big) => 0x10,
            (Self::Table39, Encoding::PlainCdr2, Endianness::Little) => 0x11,
            (Self::Table39, Encoding::PlCdr2, Endianness::Big) => 0x12,
            (Self::Table39, Encoding::PlCdr2, Endianness::Little) => 0x13,
            (Self::Table39, Encoding::DelimitedCdr, Endianness::Big) => 0x14,
            (Self::Table39, Encoding::DelimitedCdr, Endianness::Little) => 0x15,
        };
        [0x00, id]
    }

    /// The identifier of an XML-encoded sample: `{0x00, 0x04}` in
    /// Table 60, `{0x01, 0x00}` in Table 39.
    #[must_use]
    pub const fn xml_identifier(self) -> [u8; 2] {
        match self {
            Self::Table60 => [0x00, 0x04],
            Self::Table39 => [0x01, 0x00],
        }
    }
}

/// The encoding and byte order an encapsulation identifier names, from
/// either table; `None` for an identifier of neither (XML included, which
/// has no encoding of the kind [`Encoding`] names).
///
/// DDS-XTypes 1.3 Table 39 and Table 60 name the same six forms of
/// version 2 by different identifiers; a reader that takes only one of the
/// tables drops a sample the other table's writers send.
#[must_use]
pub const fn encapsulation_form(identifier: [u8; 2]) -> Option<(Encoding, Endianness)> {
    if identifier[0] != 0x00 {
        return None;
    }
    Some(match identifier[1] {
        0x00 => (Encoding::PlainCdr1, Endianness::Big),
        0x01 => (Encoding::PlainCdr1, Endianness::Little),
        0x02 => (Encoding::PlCdr1, Endianness::Big),
        0x03 => (Encoding::PlCdr1, Endianness::Little),
        0x06 | 0x10 => (Encoding::PlainCdr2, Endianness::Big),
        0x07 | 0x11 => (Encoding::PlainCdr2, Endianness::Little),
        0x08 | 0x14 => (Encoding::DelimitedCdr, Endianness::Big),
        0x09 | 0x15 => (Encoding::DelimitedCdr, Endianness::Little),
        0x0a | 0x12 => (Encoding::PlCdr2, Endianness::Big),
        0x0b | 0x13 => (Encoding::PlCdr2, Endianness::Little),
        _ => return None,
    })
}

/// [`encapsulation_header`] with the identifiers of the table named.
#[must_use]
pub fn encapsulation_header_in(
    ids: EncapsulationIds,
    encoding: Encoding,
    endianness: Endianness,
    little: bool,
    padding: u8,
) -> [u8; 4] {
    // The identifier itself is always big-endian on the wire, whatever
    // the payload behind it is — it is what tells a reader which order
    // to switch to. `little` is the payload's order, not the header's.
    let _ = little;
    let [high, low] = ids.identifier(encoding, endianness);
    [high, low, 0, padding & 0x3]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn the_emit_names_round_trip_and_a_typo_is_rejected() {
        for emit in Emit::ALL {
            assert_eq!(Emit::parse(emit.as_str()), Some(emit));
        }
        assert_eq!(Emit::parse("Payload"), None);
        assert_eq!(Emit::parse(""), None);
    }

    #[test]
    fn the_map_length_names_round_trip_and_a_typo_is_rejected() {
        for policy in MapLengthPolicy::ALL {
            assert_eq!(MapLengthPolicy::parse(policy.as_str()), Some(policy));
        }
        assert_eq!(MapLengthPolicy::parse("rule15-omits"), None);
        assert!(MapLengthPolicy::default().is_default());
    }

    /// `zerodds_ast_layout::axes::MAP_LENGTH_VALUES` cannot name this
    /// type — this crate depends on `zerodds-ast-layout`, so that
    /// module naming this type back would be a cycle — and writes the
    /// same two spellings out by hand instead. This is the other half
    /// of that duplication: it fails the day `Self::as_str` changes a
    /// spelling or `Self::ALL` grows a third reading and the axis
    /// table's two literals were not told.
    #[test]
    fn map_length_spellings_match_the_axis_table() {
        assert_eq!(MapLengthPolicy::ALL.len(), 2);
        assert_eq!(
            MapLengthPolicy::SequenceEquivalent.as_str(),
            "sequence-equivalent"
        );
        assert_eq!(MapLengthPolicy::Rule15Omits.as_str(), "rule-15-omits");
    }

    /// `PlCdr1Framing::DEFAULT` is the default dialect's framing, in
    /// every one of its six fields.
    ///
    /// The constant lives in `zerodds-wire`, which depends on neither
    /// `zerodds-ast-layout` nor this crate, so it is written out by hand
    /// there and can only be compared here. It was Cyclone's while
    /// `Dialect::Cyclone` was the default; two of the six moved when the
    /// default did, and a hand-written constant nobody compares is a
    /// constant that goes stale in silence.
    ///
    /// Asserted over both byte orders because the framing must not
    /// depend on one, and under encoding version 1 because PL_CDR1 is
    /// the only place these six reach the wire.
    #[test]
    fn the_default_framing_is_the_default_dialects() {
        for endianness in [Endianness::Little, Endianness::Big] {
            let request = ProbeRequest::new("Sample", Encoding::PlCdr1, endianness);
            assert_eq!(
                request.pl_cdr1_framing(),
                PlCdr1Framing::DEFAULT,
                "`PlCdr1Framing::DEFAULT` is not `Dialect::default`'s framing"
            );
        }
        // The two that moved, named so a failure says which. Through
        // the policy and not against the constant directly: a literal
        // `assert!` over a `const` field folds at compile time, which
        // clippy refuses and which would be a check the compiler had
        // already made rather than one this test makes.
        let default = Dialect::default().pl_cdr1_headers();
        assert_eq!(
            default.always_extended(),
            PlCdr1Framing::DEFAULT.require_extended,
            "the clause takes the short parameter header wherever §7.4.1.2.2 allows it"
        );
        assert_eq!(
            default.pads_declared_optional_length(),
            PlCdr1Framing::DEFAULT.pad_declared_optional_length,
            "the clause declares an `@optional` member's exact length"
        );
        assert_eq!(
            default,
            PlCdr1HeaderPolicy::Specification,
            "the default dialect frames a PL_CDR1 parameter list some other way now"
        );
        // And the dialect it used to be, which must now differ in
        // exactly those two.
        let cyclone = ProbeRequest::in_dialect(
            "Sample",
            Encoding::PlCdr1,
            Endianness::Little,
            Dialect::Cyclone,
        )
        .pl_cdr1_framing();
        assert_ne!(cyclone, PlCdr1Framing::DEFAULT);
        assert!(cyclone.require_extended);
        assert!(!cyclone.pad_declared_optional_length);
        assert_eq!(cyclone.pid_flags, PlCdr1Framing::DEFAULT.pid_flags);
        assert_eq!(
            cyclone.highest_short_id,
            PlCdr1Framing::DEFAULT.highest_short_id
        );
        assert_eq!(
            default.pads_declared_length(),
            PlCdr1Framing::DEFAULT.pad_declared_length,
            "XTypes 1.3 §7.4.1.2 declares a member's exact length"
        );
        assert!(
            cyclone.pad_declared_length,
            "Cyclone declares the length rounded up to four"
        );
        assert_eq!(
            cyclone.member_must_understand,
            PlCdr1Framing::DEFAULT.member_must_understand
        );
    }

    #[test]
    fn the_encapsulation_identifiers_are_the_ones_the_vendors_wrote() {
        // Read off the `header=` column of
        // `crates/ast-layout/tests/fixtures/vendor_bytes.md`, which is
        // what RTI Connext put in front of each of those samples.
        assert_eq!(
            encapsulation_header(Encoding::PlainCdr1, Endianness::Little, true, 0)[..2],
            [0x00, 0x01]
        );
        assert_eq!(
            encapsulation_header(Encoding::PlainCdr2, Endianness::Little, true, 0)[..2],
            [0x00, 0x07]
        );
        assert_eq!(
            encapsulation_header(Encoding::DelimitedCdr, Endianness::Little, true, 0)[..2],
            [0x00, 0x09]
        );
        assert_eq!(
            encapsulation_header(Encoding::PlCdr2, Endianness::Little, true, 0)[..2],
            [0x00, 0x0b]
        );
        assert_eq!(
            encapsulation_header(Encoding::PlCdr1, Endianness::Little, true, 0)[..2],
            [0x00, 0x03]
        );
    }
}
