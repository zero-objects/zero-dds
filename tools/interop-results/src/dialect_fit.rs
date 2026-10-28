// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Which axis values a measured system's own octets require.
//!
//! A dialect name is a promise: `--dialect opendds` and nothing else
//! has to reproduce every OpenDDS row the record holds. This module is
//! how that claim is derived and how it is checked, and both halves
//! read the same recorded octets.
//!
//! ## Derived, not asserted
//!
//! [`fit`] takes a system's recorded rows and searches the axis product
//! for an assignment that reproduces all of them. It is a search and
//! not a table: every axis of [`Assignment`] is varied against the
//! octets, so a value that is in a dialect because somebody believed it
//! belongs there and not because a row needs it shows up as
//! **unpinned** — [`Fit::pinned`] holds, per axis, whether moving that
//! axis alone loses a row.
//!
//! An unpinned axis is not a defect. It is a question this system's
//! recorded rows do not ask: no ORB row reaches a length code, no
//! payload row reaches a key holder. What it must not be is invisible,
//! because a value nothing pins is a value nothing would catch if it
//! were wrong.
//!
//! ## What a row is allowed not to reproduce
//!
//! One thing: padding content. `docs/wire-axes-followup.md` group G is
//! the classification and it is three criteria, all of which
//! [`Verdict::PaddingOnly`] requires — same length, every differing
//! octet one this project's plan puts fill at *and* one this project
//! wrote as `00`, and the peer's own octets reading back as the case's
//! own sample. Anything else is a miss.

use std::collections::BTreeMap;

use zerodds_ast::Ast;
use zerodds_ast::Extensibility;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{
    AggregateDelimiter, ArrayElementFlattening, CollectionDelimiterPolicy,
    DefaultedDiscriminatorMember, Dialect, ElementPadding, Encoding, Endianness, EnumHolderWidth,
    EnumeratorValue, FixedDigitCount, KeyHolderAlignment, KeyHolderExtensibility, KeyHolderNesting,
    KeyHolderOrder, LayoutKey, LengthCodePolicy, MapDelimiter, MapEntryOrder, MustUnderstandPolicy,
    MutableUnionFraming, PidFlagPolicy, PlCdr1HeaderPolicy, PlCdr1Reading, Purpose,
    SequenceElementDelimiter, ShortPidRange, UnionLabelMatch, WcharFraming, WcharWidth,
    WstringCodeUnits, WstringLengthCounts, WstringLengthPolicy, WstringTerminator,
    WstringUnitOrder, WstringUnitWidth,
};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, encode_with_padding, hex, model_from_idl_with};
use zerodds_wire_probe::value::{Emit, EncapsulationIds, MapLengthPolicy, ProbeRequest};

use crate::sources::{Answer, Measured};
use crate::systems::{Kind, System};
use crate::{Case, canonical_encoding};

/// Every wire question a dialect answers, as one point of the product.
///
/// The field list is [`LayoutKey`]'s, plus the two settings that reach
/// the octets without going through a layout plan: `map_length`, which
/// the probe carries, and `hashed_id_advances`, which the **front end**
/// answers before a plan exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assignment {
    /// Which length code a `@mutable` member's EMHEADER gets.
    pub length_codes: LengthCodePolicy,
    /// Which collections carry a delimiter under XCDR2.
    pub collection_delimiters: CollectionDelimiterPolicy,
    /// Which of a `map`'s halves decides whether the map is delimited.
    pub map_delimiter: MapDelimiter,
    /// C1 — what a `wstring`'s length prefix counts.
    pub wstring_counts: WstringLengthCounts,
    /// C2 — whether a terminating NUL is written and counted.
    pub wstring_terminator: WstringTerminator,
    /// C3 — how wide one `wstring` code unit is.
    pub wstring_unit: WstringUnitWidth,
    /// C4 — in what byte order those units go out.
    pub wstring_unit_order: WstringUnitOrder,
    /// How a `wchar` is framed.
    pub wchar_framing: WcharFraming,
    /// How wide a scalar `wchar` is.
    pub wchar_width: WcharWidth,
    /// How a `@mutable` member is framed under XCDR1.
    pub pl_cdr1_headers: PlCdr1HeaderPolicy,
    /// What flags a PL_CDR1 parameter id carries, per position.
    pub pid_flags: PidFlagPolicy,
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
    /// question.
    pub array_element_flattening: ArrayElementFlattening,
    /// Which integer an enumerator serialises as.
    pub enumerator_value: EnumeratorValue,
    /// How a `@mutable` union numbers its branches.
    pub union_framing: MutableUnionFraming,
    /// Which member ids fit a PL_CDR1 short parameter header.
    pub short_pid_range: ShortPidRange,
    /// Whether a `fixed<P,S>` carries its declared digit count.
    pub fixed_digits: FixedDigitCount,
    /// What an aggregate whose source said nothing is taken to be.
    pub default_extensibility: Extensibility,
    /// Whether a delimited `map` also carries an entry count.
    pub map_length: MapLengthPolicy,
    /// Whether a `@hashid` member advances the sequential counter.
    pub hashed_id_advances: bool,
    /// Whether a declaration may carry the name of its immediate scope.
    ///
    /// The second front-end axis, and here for the reason the first one
    /// is: it decides whether the file lowers at all, and a file that
    /// does not lower produces no octets to fit. Cyclone DDS and
    /// OpenDDS refuse it, Fast DDS and RTI Connext take it, and both
    /// sides are recorded in `ledger.tsv` under
    /// `resolve-module-named-as-type`.
    pub admits_the_name_of_the_enclosing_scope: bool,
    /// Whether a member-id hash reads an escaped name as written.
    ///
    /// The third front-end axis. IDL 4.2 §7.2.3.2 makes the escape
    /// underscore a lexical convention, so the clause hashes `long` for
    /// `_long`; RTI Connext hashes `_long`. It moves the member id, so
    /// it moves the octets — `identifier-escaped-keyword` is the case
    /// that shows it.
    pub hashid_reads_the_escaped_spelling: bool,
    /// Whether a key holder keeps the type's extensibility.
    pub key_holder: KeyHolderExtensibility,
    /// How deep a key holder's member removal reaches.
    pub key_holder_nesting: KeyHolderNesting,
    /// In what order a key holder's members are serialized.
    pub key_holder_order: KeyHolderOrder,
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

impl Assignment {
    /// The point one dialect holds, for one encoding version.
    ///
    /// The encoding is an argument because four of the axes depend on
    /// it: RTI counts a `wstring` differently under the two versions
    /// and keeps a `@mutable` type's parameter list in the key holder
    /// under version 1 only.
    #[must_use]
    pub fn of(dialect: Dialect, encoding: Encoding) -> Self {
        let key = LayoutKey::in_dialect(dialect, encoding, Endianness::Little, Purpose::Data);
        Self {
            length_codes: key.length_codes,
            collection_delimiters: key.collection_delimiters,
            map_delimiter: key.map_delimiter,
            wstring_counts: key.wstring_length.counts,
            wstring_terminator: key.wstring_length.terminator,
            wstring_unit: key.wstring_length.unit,
            wstring_unit_order: key.wstring_unit_order,
            wchar_framing: key.wchar_framing,
            wchar_width: key.wchar_width,
            pl_cdr1_headers: key.pl_cdr1_headers,
            pid_flags: key.pid_flags,
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
            // No dialect has an opinion about a map's entry count —
            // `ProbeRequest::in_dialect` leaves this at its own
            // default for the same reason, and the fit is what asks
            // whether a peer's octets have one.
            map_length: MapLengthPolicy::default(),
            hashed_id_advances: dialect.hashed_id_advances_the_counter(),
            admits_the_name_of_the_enclosing_scope: dialect
                .admits_the_name_of_the_enclosing_scope(),
            hashid_reads_the_escaped_spelling: dialect.hashid_reads_the_escaped_spelling(),
            key_holder: key.key_holder,
            key_holder_nesting: key.key_holder_nesting,
            key_holder_order: key.key_holder_order,
            element_padding: key.element_padding,
            aggregate_delimiter: key.aggregate_delimiter,
            sequence_element_delimiter: key.sequence_element_delimiter,
            wstring_code_units: key.wstring_code_units,
        }
    }

    /// A probe request for one case in this assignment.
    #[must_use]
    pub fn request(
        &self,
        type_name: &str,
        encoding: Encoding,
        endianness: Endianness,
    ) -> ProbeRequest {
        ProbeRequest {
            type_name: type_name.to_string(),
            encoding,
            endianness,
            emit: Emit::Payload,
            length_codes: self.length_codes,
            collection_delimiters: self.collection_delimiters,
            map_delimiter: self.map_delimiter,
            wchar_width: self.wchar_width,
            wstring_length: WstringLengthPolicy::new(
                self.wstring_counts,
                self.wstring_terminator,
                self.wstring_unit,
            ),
            wchar_framing: self.wchar_framing,
            wstring_unit_order: self.wstring_unit_order,
            pl_cdr1_headers: self.pl_cdr1_headers,
            pid_flags: self.pid_flags,
            // Not a fit axis: a stream's octets do not depend on how
            // tolerant the reader of it is.
            pl_cdr1_reading: PlCdr1Reading::Strict,
            encapsulation_ids: EncapsulationIds::Table60,
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
            // Not an assignment field and not fitted: a request here writes
            // payloads, which the ledger records, and never a key holder,
            // which it does not. `tools/keyhash-probe` measures that axis.
            key_holder_alignment: KeyHolderAlignment::default(),
            map_length: self.map_length,
            // Not a fit axis. Every other setting here is decided by
            // octets a vendor wrote, and this one is decided by what a
            // reader hands its application for a union member whose
            // discriminator was defaulted — no recorded byte string
            // separates the two values. The fit takes the default and
            // says so.
            defaulted_discriminator_member: DefaultedDiscriminatorMember::TypeDefault,
            element_padding: self.element_padding,
            aggregate_delimiter: self.aggregate_delimiter,
            sequence_element_delimiter: self.sequence_element_delimiter,
            wstring_code_units: self.wstring_code_units,
        }
    }

    /// This assignment's value on one axis, as the axis spells it.
    ///
    /// The printer belongs to the axis rather than to a lookup here, so
    /// that there is no name a caller can ask for and no arm to fall
    /// off the end of. An axis is a value of [`AXES`] or it does not
    /// exist.
    #[must_use]
    pub fn value_on(&self, axis: &Axis) -> String {
        (axis.show)(self)
    }
}

/// One axis: its name, and every value it can take.
///
/// The list is the search space. An axis missing from it is an axis the
/// fit cannot move, which is how a setting stays wrong through a search
/// that reports success — so
/// `crate::dialect_fit::tests::the_axes_are_the_assignment` counts
/// them against [`Assignment`]'s own fields.
pub struct Axis {
    /// The name the reports and the flags use.
    pub name: &'static str,
    /// Every value, applied to an assignment.
    pub values: &'static [fn(&mut Assignment)],
    /// How this axis spells the value an assignment holds, in the
    /// spelling the matching flag parses.
    pub show: fn(&Assignment) -> String,
}

/// Every axis a recorded octet can depend on, and the two it cannot.
///
/// The last three are the key-holder settings. They are here so that
/// the list is the whole of [`Assignment`] rather than the part that
/// happened to matter, and the fit reports them as unpinned: every
/// recorded row of every system is a **payload**, and a key holder is
/// reached only by a KeyHash.
pub const AXES: &[Axis] = &[
    Axis {
        name: "length-codes",
        values: &[
            |a| a.length_codes = LengthCodePolicy::Compact,
            |a| a.length_codes = LengthCodePolicy::ReuseNestedDelimiter,
            |a| a.length_codes = LengthCodePolicy::ShortWhereItFits,
            |a| a.length_codes = LengthCodePolicy::SizedOrNextInt,
            |a| a.length_codes = LengthCodePolicy::UniversalNextInt,
        ],
        show: |a| a.length_codes.as_str().to_string(),
    },
    Axis {
        name: "collection-delimiters",
        values: &[
            |a| a.collection_delimiters = CollectionDelimiterPolicy::Specification,
            |a| a.collection_delimiters = CollectionDelimiterPolicy::EnumIsPrimitive,
            |a| a.collection_delimiters = CollectionDelimiterPolicy::Never,
        ],
        show: |a| a.collection_delimiters.as_str().to_string(),
    },
    Axis {
        name: "map-delimiter",
        values: &[
            |a| a.map_delimiter = MapDelimiter::EitherHalf,
            |a| a.map_delimiter = MapDelimiter::ValueOnly,
        ],
        show: |a| a.map_delimiter.as_str().to_string(),
    },
    Axis {
        name: "wstring-counts",
        values: &[
            |a| a.wstring_counts = WstringLengthCounts::Octets,
            |a| a.wstring_counts = WstringLengthCounts::Characters,
        ],
        show: |a| {
            match a.wstring_counts {
                WstringLengthCounts::Octets => "octets",
                WstringLengthCounts::Characters => "characters",
            }
            .to_string()
        },
    },
    Axis {
        name: "wstring-terminator",
        values: &[
            |a| a.wstring_terminator = WstringTerminator::Absent,
            |a| a.wstring_terminator = WstringTerminator::Present,
        ],
        show: |a| {
            match a.wstring_terminator {
                WstringTerminator::Absent => "unterminated",
                WstringTerminator::Present => "terminated",
            }
            .to_string()
        },
    },
    Axis {
        name: "wstring-unit",
        values: &[
            |a| a.wstring_unit = WstringUnitWidth::Two,
            |a| a.wstring_unit = WstringUnitWidth::Four,
        ],
        show: |a| {
            match a.wstring_unit {
                WstringUnitWidth::Two => "2",
                WstringUnitWidth::Four => "4",
            }
            .to_string()
        },
    },
    Axis {
        name: "wstring-unit-order",
        values: &[
            |a| a.wstring_unit_order = WstringUnitOrder::Stream,
            |a| a.wstring_unit_order = WstringUnitOrder::BigEndianAlways,
            |a| a.wstring_unit_order = WstringUnitOrder::ByteOrderMark,
            |a| a.wstring_unit_order = WstringUnitOrder::HostUnswapped,
        ],
        show: |a| a.wstring_unit_order.as_str().to_string(),
    },
    Axis {
        name: "wchar-framing",
        values: &[
            |a| a.wchar_framing = WcharFraming::Fixed,
            |a| a.wchar_framing = WcharFraming::GiopLengthPrefixed,
            |a| a.wchar_framing = WcharFraming::GiopLengthPrefixedMemberOnly,
        ],
        show: |a| a.wchar_framing.as_str().to_string(),
    },
    Axis {
        name: "wchar-width",
        values: &[
            |a| a.wchar_width = WcharWidth::Two,
            |a| a.wchar_width = WcharWidth::Four,
        ],
        show: |a| a.wchar_width.as_str().to_string(),
    },
    Axis {
        name: "pl-cdr1-headers",
        values: &[
            |a| a.pl_cdr1_headers = PlCdr1HeaderPolicy::Specification,
            |a| a.pl_cdr1_headers = PlCdr1HeaderPolicy::ShortRoundedLength,
            |a| a.pl_cdr1_headers = PlCdr1HeaderPolicy::AlwaysExtended,
            |a| a.pl_cdr1_headers = PlCdr1HeaderPolicy::ShortTrueLength,
        ],
        show: |a| a.pl_cdr1_headers.as_str().to_string(),
    },
    Axis {
        name: "pid-flags",
        values: &[
            |a| a.pid_flags = PidFlagPolicy::SentinelMustUnderstand,
            |a| a.pid_flags = PidFlagPolicy::Bare,
            |a| a.pid_flags = PidFlagPolicy::ImplExtensionOnMembers,
        ],
        show: |a| a.pid_flags.as_str().to_string(),
    },
    Axis {
        name: "must-understand",
        values: &[
            |a| a.must_understand = MustUnderstandPolicy::Declared,
            |a| a.must_understand = MustUnderstandPolicy::Omitted,
            |a| a.must_understand = MustUnderstandPolicy::DeclaredAndKey,
            |a| a.must_understand = MustUnderstandPolicy::DeclaredKeyAndDiscriminator,
        ],
        show: |a| a.must_understand.as_str().to_string(),
    },
    Axis {
        name: "enum-holder",
        values: &[
            |a| a.enum_holder = EnumHolderWidth::Declared,
            |a| a.enum_holder = EnumHolderWidth::BitBoundDroppedByFrontEnd,
        ],
        show: |a| a.enum_holder.as_str().to_string(),
    },
    Axis {
        name: "union-labels",
        values: &[
            |a| a.union_label = UnionLabelMatch::SignExtended,
            |a| a.union_label = UnionLabelMatch::NegativeLabelUnmatchedBelow32,
        ],
        show: |a| a.union_label.as_str().to_string(),
    },
    Axis {
        name: "map-entries",
        values: &[
            |a| a.map_entries = MapEntryOrder::Declaration,
            |a| a.map_entries = MapEntryOrder::AscendingKey,
        ],
        show: |a| a.map_entries.as_str().to_string(),
    },
    Axis {
        name: "array-element-flattening",
        values: &[
            |a| a.array_element_flattening = ArrayElementFlattening::ThroughAliasesAndArrays,
            |a| a.array_element_flattening = ArrayElementFlattening::StopsAtAlias,
        ],
        show: |a| a.array_element_flattening.as_str().to_string(),
    },
    Axis {
        name: "enumerator-value",
        values: &[
            |a| a.enumerator_value = EnumeratorValue::Declared,
            |a| a.enumerator_value = EnumeratorValue::Ordinal,
        ],
        show: |a| a.enumerator_value.as_str().to_string(),
    },
    Axis {
        name: "union-framing",
        values: &[
            |a| a.union_framing = MutableUnionFraming::Specification,
            |a| a.union_framing = MutableUnionFraming::BranchesFromZero,
        ],
        show: |a| a.union_framing.as_str().to_string(),
    },
    Axis {
        name: "short-pid-range",
        values: &[
            |a| a.short_pid_range = ShortPidRange::UpToReserved,
            |a| a.short_pid_range = ShortPidRange::AllFourteenBits,
        ],
        show: |a| a.short_pid_range.as_str().to_string(),
    },
    Axis {
        name: "fixed-digits",
        values: &[
            |a| a.fixed_digits = FixedDigitCount::Declared,
            |a| a.fixed_digits = FixedDigitCount::ValuePrecision,
        ],
        show: |a| a.fixed_digits.as_str().to_string(),
    },
    Axis {
        name: "default-extensibility",
        values: &[
            |a| a.default_extensibility = Extensibility::Final,
            |a| a.default_extensibility = Extensibility::Appendable,
            |a| a.default_extensibility = Extensibility::Mutable,
        ],
        show: |a| a.default_extensibility.as_str().to_string(),
    },
    Axis {
        name: "map-length",
        values: &[
            |a| a.map_length = MapLengthPolicy::SequenceEquivalent,
            |a| a.map_length = MapLengthPolicy::Rule15Omits,
        ],
        show: |a| a.map_length.as_str().to_string(),
    },
    Axis {
        name: "hashid-name-spelling",
        values: &[
            |a| a.hashid_reads_the_escaped_spelling = true,
            |a| a.hashid_reads_the_escaped_spelling = false,
        ],
        show: |a| {
            if a.hashid_reads_the_escaped_spelling {
                "as-written"
            } else {
                "stripped"
            }
            .to_string()
        },
    },
    Axis {
        name: "enclosing-scope-name",
        values: &[
            |a| a.admits_the_name_of_the_enclosing_scope = true,
            |a| a.admits_the_name_of_the_enclosing_scope = false,
        ],
        show: |a| {
            if a.admits_the_name_of_the_enclosing_scope {
                "admitted"
            } else {
                "refused"
            }
            .to_string()
        },
    },
    Axis {
        name: "hashed-id-counter",
        values: &[
            |a| a.hashed_id_advances = true,
            |a| a.hashed_id_advances = false,
        ],
        show: |a| {
            if a.hashed_id_advances {
                "advances"
            } else {
                "skipped"
            }
            .to_string()
        },
    },
    Axis {
        name: "key-holder",
        values: &[
            |a| a.key_holder = KeyHolderExtensibility::ForcedFinal,
            |a| a.key_holder = KeyHolderExtensibility::AsDeclared,
        ],
        show: |a| a.key_holder.as_str().to_string(),
    },
    Axis {
        name: "key-holder-nesting",
        values: &[
            |a| a.key_holder_nesting = KeyHolderNesting::Recursive,
            |a| a.key_holder_nesting = KeyHolderNesting::KeylessNestedWhole,
        ],
        show: |a| a.key_holder_nesting.as_str().to_string(),
    },
    Axis {
        name: "key-holder-order",
        values: &[
            |a| a.key_holder_order = KeyHolderOrder::AscendingMemberId,
            |a| a.key_holder_order = KeyHolderOrder::Declaration,
        ],
        show: |a| a.key_holder_order.as_str().to_string(),
    },
    Axis {
        name: "element-padding",
        values: &[
            |a| a.element_padding = ElementPadding::Aligned,
            |a| a.element_padding = ElementPadding::Packed,
        ],
        show: |a| a.element_padding.as_str().to_string(),
    },
    Axis {
        name: "aggregate-delimiter",
        values: &[
            |a| a.aggregate_delimiter = AggregateDelimiter::Specification,
            |a| a.aggregate_delimiter = AggregateDelimiter::Never,
        ],
        show: |a| a.aggregate_delimiter.as_str().to_string(),
    },
    Axis {
        name: "sequence-element-delimiter",
        values: &[
            |a| a.sequence_element_delimiter = SequenceElementDelimiter::Never,
            |a| {
                a.sequence_element_delimiter = SequenceElementDelimiter::BeforeEachNonFinalElement;
            },
        ],
        show: |a| a.sequence_element_delimiter.as_str().to_string(),
    },
    Axis {
        name: "wstring-code-units",
        values: &[
            |a| a.wstring_code_units = WstringCodeUnits::Utf16,
            |a| a.wstring_code_units = WstringCodeUnits::Utf8,
        ],
        show: |a| a.wstring_code_units.as_str().to_string(),
    },
];

/// One recorded octet string, with everything needed to reproduce it.
#[derive(Debug, Clone)]
pub struct Row {
    /// Which corpus case, by index into the slice [`rows`] was given.
    pub case: usize,
    /// The framing this project writes for the peer's version.
    pub encoding: Encoding,
    /// The byte order.
    pub endianness: Endianness,
    /// The encoding name, for a report.
    pub encoding_name: &'static str,
    /// The byte order's name, for a report.
    pub endian_name: &'static str,
    /// What the peer wrote.
    pub octets: String,
}

/// What one assignment made of one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The same octets.
    Same,
    /// Different octets, and the difference is padding content by all
    /// three of group G's criteria.
    PaddingOnly,
    /// Anything else, with what this project wrote.
    Different(String),
    /// This project would not write the case at all.
    Refused(String),
}

/// Every row one system recorded octets for.
///
/// A version the system has no encoding 2 for, a byte order it does not
/// write and a case it refused are all absent rather than present and
/// empty: they are not rows.
#[must_use]
pub fn rows(system: &System, cases: &[Case], foreign: &Measured, orbs: &Measured) -> Vec<Row> {
    let measured = if system.kind == Kind::Orb {
        orbs
    } else {
        foreign
    };
    let mut out = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        for version in ["xcdr1", "xcdr2"] {
            if version == "xcdr2" && !system.has_version_2 {
                continue;
            }
            for endian in crate::ORDERS {
                if !system.writes_orders.contains(&endian) {
                    continue;
                }
                let Some(Answer::Bytes(octets)) =
                    crate::sources::get(measured, &case.name, system.id, version, endian)
                else {
                    continue;
                };
                let encoding_name = canonical_encoding(if version == "xcdr1" {
                    "plain-cdr1"
                } else {
                    "plain-cdr2"
                });
                out.push(Row {
                    case: index,
                    encoding: if version == "xcdr1" {
                        Encoding::PlainCdr1
                    } else {
                        Encoding::PlainCdr2
                    },
                    endianness: if endian == "le" {
                        Endianness::Little
                    } else {
                        Endianness::Big
                    },
                    encoding_name,
                    endian_name: endian,
                    octets: octets.clone(),
                });
            }
        }
    }
    out
}

/// The lowerings a fit needs, cached over the two front-end axes: a
/// `@hashid` member either advances the sequential counter or does not,
/// a declaration may or may not carry the name of its own scope, and a
/// member-id hash reads an escaped name as written or stripped.
/// All three are settled before any plan exists, so they cannot be
/// varied inside the layout search — the model itself differs.
///
/// Eight entries per case, one per point of [`FrontEnd`].
struct Models {
    by_axes: BTreeMap<(FrontEnd, usize), Lowering>,
}

/// One lowering, or the reason there is none.
type Lowering = Result<(Ast, Option<String>), String>;

/// The three front-end axis values a lowering was made with.
///
/// Named rather than a `(bool, bool, bool)`: three bare booleans in one
/// key are three chances to pass them in the wrong order, and nothing
/// would say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FrontEnd {
    /// Whether a `@hashid` member advances the sequential counter.
    hashid_advances: bool,
    /// Whether a declaration may carry the name of its immediate scope.
    admits_the_scope_name: bool,
    /// Whether a member-id hash reads an escaped name as written.
    escaped_spelling: bool,
}

impl Models {
    fn new(cases: &[Case]) -> Self {
        let mut by_axes = BTreeMap::new();
        for (index, case) in cases.iter().enumerate() {
            for hashid_advances in [true, false] {
                for admits_the_scope_name in [true, false] {
                    for escaped_spelling in [true, false] {
                        let axes = FrontEnd {
                            hashid_advances,
                            admits_the_scope_name,
                            escaped_spelling,
                        };
                        by_axes.insert((axes, index), lower(case, axes));
                    }
                }
            }
        }
        Self { by_axes }
    }

    /// The model for one case, or why there is none.
    ///
    /// `None` for a case index that is not in the corpus, which is a
    /// caller error and is reported as a refusal rather than reached
    /// through a panic: this crate denies `expect` and a search that
    /// stopped on one would take a whole system's fit with it.
    fn get(&self, case: usize, axes: FrontEnd) -> Option<Result<&(Ast, Option<String>), &String>> {
        self.by_axes.get(&(axes, case)).map(|entry| entry.as_ref())
    }
}

fn lower(case: &Case, axes: FrontEnd) -> Lowering {
    let options = zerodds_idl_lower::Options {
        hashed_id_advances_the_counter: if axes.hashid_advances {
            zerodds_idl_lower::HashedIdCounter::Advances
        } else {
            zerodds_idl_lower::HashedIdCounter::Skipped
        },
        name_of_the_enclosing_scope: if axes.admits_the_scope_name {
            zerodds_idl_lower::NameOfTheEnclosingScope::Admitted
        } else {
            zerodds_idl_lower::NameOfTheEnclosingScope::Refused
        },
        hashed_name_spelling: if axes.escaped_spelling {
            zerodds_idl_lower::HashedNameSpelling::AsWritten
        } else {
            zerodds_idl_lower::HashedNameSpelling::Stripped
        },
        ..zerodds_idl_lower::Options::default()
    };
    let ast = model_from_idl_with(&case.idl, &format!("{}/type.idl", case.name), options)
        .map_err(|error| format!("{}: the IDL does not lower: {error}", case.name))?;
    let expected = expected_text(&ast, case);
    Ok((ast, expected))
}

fn expected_text(ast: &Ast, case: &Case) -> Option<String> {
    let type_ = ast
        .find_by_scoped_name(&ScopedName::absolute(
            case.type_name
                .split("::")
                .filter(|segment| !segment.is_empty())
                .map(std::string::ToString::to_string)
                .collect(),
        ))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)?;
    canonicalise(ast, type_, &case.values, "sample")
        .ok()
        .map(|value| text(&value))
}

/// How one assignment answers every row of one system.
pub struct Answers {
    /// One verdict per row, in the order [`rows`] gave them.
    pub verdicts: Vec<Verdict>,
}

impl Answers {
    /// How many rows came out byte for byte the peer's.
    #[must_use]
    pub fn same(&self) -> usize {
        self.verdicts
            .iter()
            .filter(|v| **v == Verdict::Same)
            .count()
    }

    /// How many differ only in octets no reader looks at.
    #[must_use]
    pub fn padding_only(&self) -> usize {
        self.verdicts
            .iter()
            .filter(|v| **v == Verdict::PaddingOnly)
            .count()
    }

    /// How many are neither — the rows a dialect does not reach.
    #[must_use]
    pub fn missed(&self) -> usize {
        self.verdicts.len() - self.same() - self.padding_only()
    }

    /// What a search maximises: an exact row first, a padding-content
    /// row second. The second term matters because a padding-content
    /// row is still a row whose *length* and every value octet this
    /// project has to get right.
    fn score(&self) -> (usize, usize) {
        (self.same(), self.padding_only())
    }
}

/// Every row of one system under one assignment.
fn answer(cases: &[Case], models: &Models, rows: &[Row], point: &Assignment) -> Answers {
    let verdicts = rows
        .iter()
        .map(|row| {
            let (ast, expected) = match models.get(
                row.case,
                FrontEnd {
                    hashid_advances: point.hashed_id_advances,
                    admits_the_scope_name: point.admits_the_name_of_the_enclosing_scope,
                    escaped_spelling: point.hashid_reads_the_escaped_spelling,
                },
            ) {
                Some(Ok(model)) => model,
                Some(Err(error)) => return Verdict::Refused(error.clone()),
                None => return Verdict::Refused(format!("no case at index {}", row.case)),
            };
            let Some(case) = cases.get(row.case) else {
                return Verdict::Refused(format!("no case at index {}", row.case));
            };
            let request = point.request(&case.type_name, row.encoding, row.endianness);
            let Ok((bytes, mask)) = encode_with_padding(ast, &request, &case.values) else {
                return Verdict::Refused("this project writes nothing for the case".to_string());
            };
            let mine = hex(&bytes);
            if mine == row.octets {
                return Verdict::Same;
            }
            if padding_only(&mine, &row.octets, &mask)
                && reads_back(ast, &request, expected.as_deref(), &row.octets)
            {
                return Verdict::PaddingOnly;
            }
            Verdict::Different(mine)
        })
        .collect();
    Answers { verdicts }
}

/// Group G's first two criteria over one pair of byte strings.
///
/// Same length; every differing octet both one the plan puts fill at
/// and one this project wrote as `00`. The two are independent
/// derivations of the same position and both are required — the mask
/// asks the plan, the `00` asks what was written.
fn padding_only(mine: &str, theirs: &str, mask: &[bool]) -> bool {
    if mine.len() != theirs.len() || mine.len() % 2 != 0 || mask.len() * 2 != mine.len() {
        return false;
    }
    let (ours, them) = (mine.as_bytes(), theirs.as_bytes());
    (0..mask.len()).all(|octet| {
        let at = octet * 2..octet * 2 + 2;
        ours[at.clone()] == them[at] || (mask[octet] && &mine[octet * 2..octet * 2 + 2] == "00")
    })
}

/// Group G's third criterion: the peer's own octets, through this
/// project's decoder under the plan they were compared against, give
/// the case's own sample back.
fn reads_back(ast: &Ast, request: &ProbeRequest, expected: Option<&str>, theirs: &str) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    let Ok(bytes) = unhex(theirs) else {
        return false;
    };
    matches!(decode(ast, request, &bytes), Ok(value) if text(&value) == expected)
}

fn unhex(hex: &str) -> Result<Vec<u8>, ()> {
    if hex.len() % 2 != 0 {
        return Err(());
    }
    (0..hex.len() / 2)
        .map(|at| u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).map_err(|_| ()))
        .collect()
}

/// What a search found for one system.
pub struct Fit {
    /// The assignment that reproduces the most rows.
    pub point: Assignment,
    /// Its verdicts, row for row.
    pub answers: Answers,
    /// How many rows there were.
    pub rows: usize,
    /// Per axis: whether some row pins the value — whether moving that
    /// axis alone, and nothing else, loses a row.
    pub pinned: BTreeMap<&'static str, bool>,
    /// Per axis: the other values that answer every row equally well.
    /// Empty where the axis is pinned.
    pub indifferent: BTreeMap<&'static str, Vec<String>>,
}

/// Search the axis product for the assignment a system's own octets
/// require.
///
/// Coordinate ascent from every dialect in turn, then a pairwise pass
/// where a single move cannot improve: two axes that only pay off
/// together — the enumeration holder and the enumerator numbering are
/// the measured instance — are a local maximum a one-axis search stops
/// at, and a search that stops early reports a system as unreachable
/// when it is not.
///
/// **One encoding version at a time.** Four axes are answered
/// differently under the two versions by one measured peer — RTI counts
/// a `wstring`'s length one way under XCDR1 and another under XCDR2 —
/// and a fit over both versions at once takes whichever answer has more
/// rows behind it and reports the other version's rows as unreachable.
/// It did: RTI came out 48 rows short, all of them XCDR1 `wchar` and
/// `wstring` rows, under an assignment that was XCDR2's.
#[must_use]
pub fn fit(system: &System, cases: &[Case], rows: &[Row], seeds: &[Assignment]) -> Fit {
    let models = Models::new(cases);
    let mut best: Option<(Assignment, Answers)> = None;
    for seed in seeds {
        let (point, answers) = ascend(cases, &models, rows, *seed);
        let better = best
            .as_ref()
            .is_none_or(|(_, current)| answers.score() > current.score());
        if better {
            best = Some((point, answers));
        }
        if best
            .as_ref()
            .is_some_and(|(_, a)| a.same() + a.padding_only() == rows.len())
        {
            break;
        }
    }
    let (point, answers) = best.unwrap_or_else(|| {
        let point = seeds
            .first()
            .copied()
            .unwrap_or(Assignment::of(Dialect::Specification, Encoding::PlainCdr1));
        let answers = answer(cases, &models, rows, &point);
        (point, answers)
    });
    let (pinned, indifferent) = pins(cases, &models, rows, &point, &answers);
    let _ = system;
    Fit {
        rows: rows.len(),
        point,
        answers,
        pinned,
        indifferent,
    }
}

fn ascend(
    cases: &[Case],
    models: &Models,
    rows: &[Row],
    seed: Assignment,
) -> (Assignment, Answers) {
    let mut point = seed;
    let mut answers = answer(cases, models, rows, &point);
    loop {
        let mut moved = false;
        for axis in AXES {
            for value in axis.values {
                let mut candidate = point;
                value(&mut candidate);
                if candidate == point {
                    continue;
                }
                let scored = answer(cases, models, rows, &candidate);
                if scored.score() > answers.score() {
                    point = candidate;
                    answers = scored;
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
    if answers.same() + answers.padding_only() == rows.len() {
        return (point, answers);
    }
    // The pairwise pass. Only where a single move is stuck, because it
    // is quadratic in the axes and the single pass settles most of
    // them.
    loop {
        let mut moved = false;
        for (first, second) in pairs() {
            for one in AXES[first].values {
                for two in AXES[second].values {
                    let mut candidate = point;
                    one(&mut candidate);
                    two(&mut candidate);
                    if candidate == point {
                        continue;
                    }
                    let scored = answer(cases, models, rows, &candidate);
                    if scored.score() > answers.score() {
                        point = candidate;
                        answers = scored;
                        moved = true;
                    }
                }
            }
        }
        if !moved {
            break;
        }
    }
    (point, answers)
}

fn pairs() -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for first in 0..AXES.len() {
        for second in first + 1..AXES.len() {
            out.push((first, second));
        }
    }
    out
}

/// Which axes the octets pin, and which they are indifferent to.
fn pins(
    cases: &[Case],
    models: &Models,
    rows: &[Row],
    point: &Assignment,
    answers: &Answers,
) -> (
    BTreeMap<&'static str, bool>,
    BTreeMap<&'static str, Vec<String>>,
) {
    let mut pinned = BTreeMap::new();
    let mut indifferent: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for axis in AXES {
        let mut others = Vec::new();
        for value in axis.values {
            let mut candidate = *point;
            value(&mut candidate);
            if candidate == *point {
                continue;
            }
            let scored = answer(cases, models, rows, &candidate);
            if scored.score() >= answers.score() {
                others.push(candidate.value_on(axis));
            }
        }
        pinned.insert(axis.name, others.is_empty());
        if !others.is_empty() {
            indifferent.insert(axis.name, others);
        }
    }
    (pinned, indifferent)
}

/// The dialect that names one measured system.
///
/// Nine systems, nine dialects. The map is by name and not by order:
/// `Dialect::as_str` and `System::id` are the same string for all nine,
/// and `tests::every_measured_system_has_a_dialect` fails the day one
/// of them stops being.
#[must_use]
pub fn dialect_of(system: &System) -> Option<Dialect> {
    Dialect::parse(system.id)
}

/// Whether the dialect gates hold this system.
///
/// Every measured foreign column, `hdds` included. HDDS was the
/// exemption while it had no dialect — its serialiser framed
/// `@appendable` and `@mutable` types as `@final`, outside every
/// configuration the encoder produced. The dialect pass gave those
/// departures axes and `Dialect::Hdds` holds them, so the exemption is
/// gone and the column is gated like every other.
/// `tests::every_measured_system_has_a_dialect` fails the day a name
/// has to join an exemption again.
#[must_use]
pub fn dialect_gated(system: &System) -> bool {
    system.kind != Kind::Own
}

/// What one system's own dialect, **and nothing else**, makes of every
/// row that system recorded.
///
/// This is the promise a dialect name is: `--dialect opendds` with no
/// second flag reproduces OpenDDS. The one verdict other than
/// [`Verdict::Same`] a caller may accept is [`Verdict::PaddingOnly`],
/// and the three criteria behind it are checked here rather than
/// assumed — see this module's header.
///
/// One encoding version at a time, because four axes are answered per
/// version; the rows carry their own encoding, so the caller does not
/// have to split them.
#[must_use]
pub fn under_dialect(cases: &[Case], rows: &[Row], dialect: Dialect) -> Answers {
    let models = Models::new(cases);
    let verdicts = rows
        .iter()
        .map(|row| {
            let point = Assignment::of(dialect, row.encoding);
            let single = [row.clone()];
            let mut answers = answer(cases, &models, &single, &point);
            answers.verdicts.remove(0)
        })
        .collect();
    Answers { verdicts }
}

/// What one dialect's reader made of one recorded row.
///
/// The read direction's counterpart to [`Verdict`], and it needs one
/// case that direction does not: a row whose *writer* left nothing to
/// read back, which is a property of the peer and not of this reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadVerdict {
    /// The peer's octets came back as the case's own sample.
    Same,
    /// They decoded, and to something else. Carries what came back.
    Different(String),
    /// The decoder refused them, with what it said.
    Refused(String),
    /// The row cannot be read back and the reason is the writer's.
    ///
    /// One shape reaches this: a dialect that suppresses a negative
    /// narrow case label writes no branch at all, so the member the
    /// sample names is not on the wire and a reader that returned it
    /// would be inventing it. Named per row rather than allowed by
    /// class — the string is what the caller reports.
    NothingToRead(String),
    /// The corpus has no canonical text for the case, so there is
    /// nothing to compare against. A gap in the corpus and not a
    /// verdict about the reader.
    NoSample(String),
}

/// Every recorded row of one system, through this project's decoder
/// under one dialect, against the case's own sample.
///
/// The mirror of [`under_dialect`]. That one asks whether naming a
/// dialect **writes** what the peer wrote; this asks whether naming it
/// **reads** what the peer wrote. A project that could do the first and
/// not the second would talk to a peer in one direction, and the tables
/// would say so nowhere: every figure in them is a write.
///
/// The comparison is against the case's own sample and never against
/// another decoder's reading of the same bytes — two decoders wrong the
/// same way agree with each other.
#[must_use]
pub fn read_under_dialect(cases: &[Case], rows: &[Row], dialect: Dialect) -> Vec<ReadVerdict> {
    Sweep::new(cases).read(cases, rows, &|encoding| Assignment::of(dialect, encoding))
}

/// The two lowerings, built once and swept many times.
///
/// [`read_under_dialect`] and [`under_dialect`] each build them, which
/// is right for one sweep and wrong for the axis sweep: that one moves
/// every coordinate of every dialect and would lower 347 cases twice per
/// move. The cache is the whole difference between a gate that runs in
/// seconds and one nobody runs.
pub struct Sweep {
    models: Models,
}

impl Sweep {
    /// Lower every case, both ways round the `@hashid` counter.
    #[must_use]
    pub fn new(cases: &[Case]) -> Self {
        Self {
            models: Models::new(cases),
        }
    }

    /// The read direction under an assignment built per encoding
    /// version.
    #[must_use]
    pub fn read(
        &self,
        cases: &[Case],
        rows: &[Row],
        point_for: &dyn Fn(Encoding) -> Assignment,
    ) -> Vec<ReadVerdict> {
        read_with(&self.models, cases, rows, point_for)
    }

    /// The write direction under the same.
    #[must_use]
    pub fn write(
        &self,
        cases: &[Case],
        rows: &[Row],
        point_for: &dyn Fn(Encoding) -> Assignment,
    ) -> Answers {
        let verdicts = rows
            .iter()
            .map(|row| {
                let point = point_for(row.encoding);
                let single = [row.clone()];
                let mut answers = answer(cases, &self.models, &single, &point);
                answers.verdicts.remove(0)
            })
            .collect();
        Answers { verdicts }
    }
}

/// The same sweep under an assignment the caller builds per encoding
/// version.
///
/// Two callers need this rather than [`read_under_dialect`]: the axis
/// sweep, which moves one coordinate of a dialect's assignment and asks
/// whether any recorded row reads differently, and a caller asking what
/// a *fixed* configuration makes of a peer's rows — which is what
/// reading a peer's octets without naming its dialect is.
#[must_use]
pub fn read_under(
    cases: &[Case],
    rows: &[Row],
    point_for: &dyn Fn(Encoding) -> Assignment,
) -> Vec<ReadVerdict> {
    read_with(&Models::new(cases), cases, rows, point_for)
}

fn read_with(
    models: &Models,
    cases: &[Case],
    rows: &[Row],
    point_for: &dyn Fn(Encoding) -> Assignment,
) -> Vec<ReadVerdict> {
    rows.iter()
        .map(|row| {
            let point = point_for(row.encoding);
            let (ast, expected) = match models.get(
                row.case,
                FrontEnd {
                    hashid_advances: point.hashed_id_advances,
                    admits_the_scope_name: point.admits_the_name_of_the_enclosing_scope,
                    escaped_spelling: point.hashid_reads_the_escaped_spelling,
                },
            ) {
                Some(Ok(model)) => model,
                Some(Err(error)) => return ReadVerdict::Refused(error.clone()),
                None => {
                    return ReadVerdict::Refused(format!("no case at index {}", row.case));
                }
            };
            let Some(case) = cases.get(row.case) else {
                return ReadVerdict::Refused(format!("no case at index {}", row.case));
            };
            let Some(expected) = expected.as_deref() else {
                return ReadVerdict::NoSample(format!(
                    "{}: the corpus has no canonical sample",
                    case.name
                ));
            };
            let request = point.request(&case.type_name, row.encoding, row.endianness);
            // Asked of the plan and not of a list of case names: a
            // suppressed negative narrow label is a dialect setting, and
            // a peer that writes no branch for it has written nothing
            // the reader could hand back.
            if let Some(why) = nothing_to_read(ast, &request, &case.values, expected) {
                return ReadVerdict::NothingToRead(why);
            }
            let Ok(bytes) = unhex(&row.octets) else {
                return ReadVerdict::Refused("the recorded octets are not hex".to_string());
            };
            match decode(ast, &request, &bytes) {
                Ok(value) => {
                    let mine = text(&value);
                    if mine == expected {
                        ReadVerdict::Same
                    } else {
                        ReadVerdict::Different(mine)
                    }
                }
                Err(error) => ReadVerdict::Refused(format!("{error}")),
            }
        })
        .collect()
}

/// Whether the plan leaves the sample's own member off the wire, so
/// that no reader could hand it back.
///
/// Two conditions, both required, and both measured rather than listed.
///
/// 1. **The dialect suppresses a negative narrow case label.** Under
///    `UnionLabelMatch::SignExtended` — the clause, and every dialect
///    but Cyclone's — nothing is suppressed and this returns `None`
///    without looking further. That is what keeps the allowance from
///    covering a decoder defect on a case that has no union in it.
/// 2. **This project's own writer and reader, under the same request,
///    do not return the sample either.** The frame the peer wrote is
///    the frame this project writes; if its own round trip cannot
///    return the member, the member is not on the wire.
///
/// The second condition alone would excuse any decoder bug the encoder
/// shares, which is the shape of a false green this tree has recorded
/// six times. The first alone would excuse every row of a dialect that
/// happens to suppress. Together they name a frame with no branch in
/// it and nothing else.
fn nothing_to_read(
    ast: &Ast,
    request: &ProbeRequest,
    values: &serde_json::Value,
    expected: &str,
) -> Option<String> {
    if request.union_label == UnionLabelMatch::SignExtended {
        return None;
    }
    let mine = encode(ast, request, values).ok()?;
    let back = decode(ast, request, &mine).ok().map(|value| text(&value));
    if back.as_deref() == Some(expected) {
        return None;
    }
    Some(
        "the dialect suppresses a negative narrow case label, so the frame carries no branch \
         for the member the sample names — this project's own writer and reader do not return \
         it either"
            .to_string(),
    )
}

/// Every dialect's point, as search seeds.
#[must_use]
pub fn dialect_seeds(encoding: Encoding) -> Vec<Assignment> {
    Dialect::ALL
        .iter()
        .map(|dialect| Assignment::of(*dialect, encoding))
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::{AXES, Assignment};
    use zerodds_ast_layout::{Dialect, Encoding};

    /// Every axis in the list answers, and no two share a name.
    #[test]
    fn the_axes_are_named_once_each() {
        let mut names: Vec<&str> = AXES.iter().map(|axis| axis.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two axes share a name");
    }

    /// Every axis moves the assignment, and `value_on` answers for it.
    ///
    /// An axis whose values all leave the point where it was is an axis
    /// the search cannot move, which is a search that reports success
    /// over a question it never asked.
    #[test]
    fn every_axis_moves_the_assignment() {
        let base = Assignment::of(Dialect::Cyclone, Encoding::PlainCdr1);
        for axis in AXES {
            let mut seen: Vec<String> = Vec::new();
            for value in axis.values {
                let mut point = base;
                value(&mut point);
                seen.push(point.value_on(axis));
            }
            seen.sort();
            let count = seen.len();
            seen.dedup();
            assert_eq!(
                seen.len(),
                count,
                "`{}` has two values that give the same answer",
                axis.name
            );
            assert!(count > 1, "`{}` has one value", axis.name);
        }
    }

    /// Every measured system is named by a dialect.
    ///
    /// `hdds` joined when `Dialect::Hdds` arrived with the four axes
    /// that carry its departures; there is no exemption left.
    #[test]
    fn every_measured_system_has_a_dialect() {
        use crate::systems::{Kind, SYSTEMS};
        for system in SYSTEMS.iter().filter(|system| system.kind != Kind::Own) {
            assert!(
                super::dialect_of(system).is_some(),
                "`{}` is a measured system with no dialect of that name; either give it one \
                 or name it in `dialect_gated` with its reason",
                system.id
            );
            assert!(
                super::dialect_gated(system),
                "`{}` has a dialect and is hidden from the gates",
                system.id
            );
        }
    }

    /// The assignment's field count is the axis count.
    ///
    /// A field added to [`Assignment`] and not to [`AXES`] is a wire
    /// question the search silently holds fixed. The count is taken
    /// from this file's own source rather than from a list written
    /// twice.
    #[test]
    fn the_axes_are_the_assignment() {
        let source = std::fs::read_to_string(file!())
            .or_else(|_| {
                std::fs::read_to_string(
                    crate::repository_root()
                        .join("tools")
                        .join("interop-results")
                        .join("src")
                        .join("dialect_fit.rs"),
                )
            })
            .expect("this file");
        let body = source
            .split_once("pub struct Assignment {")
            .expect("the assignment")
            .1
            .split_once("\n}")
            .expect("the assignment ends")
            .0;
        let fields = body
            .lines()
            .filter(|line| line.trim_start().starts_with("pub ") && line.trim_end().ends_with(','))
            .count();
        assert_eq!(
            fields,
            AXES.len(),
            "{fields} fields on the assignment and {} axes",
            AXES.len()
        );
    }
}
