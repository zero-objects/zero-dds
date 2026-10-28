// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Layout keys. Spec §7.

use zerodds_ast::Extensibility;

/// Which encoding a plan is computed for.
///
/// XCDR1 and XCDR2 differ in their framing: XCDR1 has no delimiter
/// header, and `@mutable` uses PID headers there and EMHEADERs here.
/// Primitive alignment is the same in both. OMG XTypes 1.3 §7.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Encoding {
    /// XCDR1 without a parameter list. XTypes 1.3 §7.4.1.
    PlainCdr1,
    /// XCDR1 with a parameter list, for `@mutable`. XTypes 1.3 §7.4.1.2.
    PlCdr1,
    /// XCDR2 without a header. XTypes 1.3 §7.4.2.
    PlainCdr2,
    /// XCDR2 with a delimiter header, for `@appendable`.
    DelimitedCdr,
    /// XCDR2 with a parameter list, for `@mutable`.
    PlCdr2,
}

impl Encoding {
    /// Every encoding the layout planner covers.
    pub const ALL: [Self; 5] = [
        Self::PlainCdr1,
        Self::PlCdr1,
        Self::PlainCdr2,
        Self::DelimitedCdr,
        Self::PlCdr2,
    ];

    /// Whether this is an XCDR version 1 encoding.
    #[must_use]
    pub const fn is_v1(self) -> bool {
        matches!(self, Self::PlainCdr1 | Self::PlCdr1)
    }

    /// Whether this is an XCDR version 2 encoding.
    #[must_use]
    pub const fn is_v2(self) -> bool {
        !self.is_v1()
    }

    /// The largest alignment any primitive takes.
    ///
    /// XTypes 1.3 §7.4.1, the XCDR stream operations table, defines it
    /// outright:
    ///
    /// > `MAXALIGN( VERSION2) = 4`
    /// > `MAXALIGN( VERSION1) = 8`
    /// > `MAXALIGN( VERSION_NONE) = 8`
    ///
    /// and `ALIGN(N)` aligns to `MIN(N, XCDR.maxalign)`. §7.6.8 repeats
    /// it for the key holder: "the maximum alignment in PLAIN_CDR2
    /// (i.e., 4)".
    ///
    /// # Why this reads as a correction
    ///
    /// It was briefly eight for both versions, on the authority of a
    /// wire vector V-3 in `crates/cdr/tests/xcdr2_wire_vectors.rs` at
    /// `e1f3af895`. That vector is not an OMG artefact — its header
    /// cites §7.4.2 but its byte table was computed by hand, and it pads
    /// the trailing double to align(8) for a total of 48 bytes. Three
    /// independent sources say 44:
    ///
    /// - the spec text above,
    /// - Cyclone DDS, `src/core/cdr/src/dds_cdrstream.c:239` —
    ///   `xcdr_version == DDSI_RTPS_CDR_ENC_VERSION_2 ? MK_ALIGN(4) :
    ///   MK_ALIGN(8)`, and `:2435` `.alignmask = (… ? 3 : 7)`,
    /// - Fast CDR, `src/cpp/Cdr.cpp:238` — `align64_ = 4` on reading an
    ///   XCDRv2 encapsulation, 8 otherwise.
    ///
    /// Measured, not read: `@final struct Point { long x; double y; }`
    /// with `x=1, y=2.0` comes off Cyclone's and RTI Connext 7.7's own
    /// serialisers as twelve payload bytes,
    /// `010000000000000000000040`, with the double at offset 4.
    /// `tests/fixtures/vendor_bytes.md` records the runs.
    #[must_use]
    pub const fn max_alignment(self) -> u8 {
        if self.is_v1() { 8 } else { 4 }
    }
}

/// Byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Endianness {
    /// Little-endian.
    Little,
    /// Big-endian.
    Big,
}

impl Endianness {
    /// Both orders.
    pub const ALL: [Self; 2] = [Self::Little, Self::Big];
}

/// What the plan is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Purpose {
    /// The full sample.
    Data,
    /// The key holder only. DDSI-RTPS 2.5 §9.6.3.8.
    Key,
}

impl Purpose {
    /// Both purposes.
    pub const ALL: [Self; 2] = [Self::Data, Self::Key];
}

/// Which length code a `@mutable` member's EMHEADER gets.
///
/// XTypes 1.3 §7.4.3.4.2 gives eight codes and constrains almost
/// nothing about which one a writer picks. LC0 to LC3 state a length of
/// 1, 2, 4 or 8 in the header itself; LC4 states it in a following
/// NEXTINT; LC5 to LC7 say the member's own leading word *is* the
/// NEXTINT, times one, four or eight. Several are legal for the same
/// member and they are not the same bytes, so which one is a
/// configuration point.
///
/// # The three implementations take three positions
///
/// Measured 2026-08-09 through `wire-oracle/gate`, cases
/// `mut_lengthcodes` and `mut_lc_nested`, XCDR2 little endian. The
/// EMHEADER each member of a `@mutable` struct gets:
///
/// | member | Cyclone | RTI 7.7 | Fast DDS |
/// |---|---|---|---|
/// | `string s = "abc"` | `00000050` LC5 | LC5 | LC5 |
/// | `sequence<octet>` of 3 | `01000050` LC5 | LC5 | LC5 |
/// | `sequence<short>` of 2 | `02000040` LC4 | LC4 | `02000030` **LC3** |
/// | `sequence<long>` of 2 | `03000060` LC6 | LC6 | LC6 |
/// | `sequence<double>` of 1 | `04000070` LC7 | LC7 | LC7 |
/// | `sequence<Cell>` of 1 | `05000050` LC5 | `05000040` LC4 | LC5 |
/// | `Cell arr[2]` | `06000050` LC5 | LC4 | LC5 |
/// | `long ap[2]` | `07000040` LC4 | LC4 | `07000030` **LC3** |
/// | `Cell nested` | `08000040` LC4 | LC4 | `08000030` **LC3** |
/// | `long tail` | `09000020` LC2 | LC2 | LC2 |
/// | `ACell app` (`@appendable`) | `00000040` LC4 | `00000050` **LC5** | **LC5** |
/// | `MCell mnest` (`@mutable`) | `01000040` LC4 | **LC5** | **LC5** |
///
/// The `sequence<Cell>` and `Cell arr[2]` rows are the collection
/// delimiter — [`CollectionDelimiterPolicy`] — seen from the length-code
/// side: RTI writes no collection DHEADER, so there is no leading word
/// to reuse and LC4 is the only code left. What is left over after that
/// is two independent choices, and each implementation makes a different
/// pair of them:
///
/// 1. **Does a nested aggregate's own DHEADER count as a reusable
///    leading word?** An `@appendable` or `@mutable` member begins with
///    a DHEADER, which is exactly the byte length LC5 would state. RTI
///    and Fast DDS reuse it. Cyclone does not, and says why in
///    `dds_cdrstream.c`: `case DDS_OP_VAL_UNI: case DDS_OP_VAL_EXT:
///    return LENGTH_CODE_NEXTINT; /* FIXME: may be optimized for
///    specific cases, e.g. when EXT type is appendable */`.
/// 2. **Does a member with no reusable word take a short code when its
///    serialized size happens to be 1, 2, 4 or 8?** Fast DDS does.
///    Cyclone and RTI take a short code only where the member's *kind*
///    fixes the width — a primitive, an enumeration, a bitmask — and
///    write LC4 for an eight-byte array or an eight-byte nested
///    structure alike. Cyclone's `get_length_code` switches on the op
///    kind and never on a size.
///
/// The two nest: Cyclone makes neither choice, RTI the first, Fast DDS
/// both. So one enum in that order rather than two booleans, which
/// would have four combinations and three witnesses.
///
/// # The runtime half of Fast DDS's rule
///
/// Fast DDS's `sequence<short>` of two elements is eight bytes and takes
/// LC3. Three elements would be ten and take LC4. A layout plan cannot
/// make that choice — the count is in the value, not in the type — so
/// [`Self::ShortWhereItFits`] is answered in two places: the plan
/// answers for every member whose size the *type* fixes, and the writer
/// answers for the rest by patching the header once the value's length
/// is known. `Writer::finish_emheader` does the patching, which is the
/// same thing it already does to the NEXTINT and is safe for the same
/// reason: under XCDR2 `MAXALIGN` is four, the value starts four-aligned
/// with the NEXTINT and four-aligned without it, so removing the
/// placeholder moves the value without changing a byte of it.
///
/// # Every code decodes
///
/// A reader that implements §7.4.3.4.2 handles all eight, because the
/// code is in the header. This is a question about which bytes go out,
/// not about which can be read — unlike [`CollectionDelimiterPolicy`],
/// where nothing in the stream says whether the four bytes in front are
/// a delimiter or the first element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum LengthCodePolicy {
    /// LC5, LC6 and LC7 where a string's or a sequence's own leading
    /// word can be reused, LC0 to LC3 where the member's *kind* fixes
    /// its width, and LC4 everywhere else.
    ///
    /// The default, and what Cyclone DDS writes. A nested aggregate's
    /// DHEADER is not reused and an eight-byte array is not shortened,
    /// which is where Cyclone parts from the other two.
    #[default]
    Compact,
    /// [`Self::Compact`], and a nested aggregate that begins with its
    /// own DHEADER reuses it as the NEXTINT.
    ///
    /// What RTI Connext 7.7 writes. Four bytes shorter than
    /// [`Self::Compact`] for every `@appendable` or `@mutable` member of
    /// a `@mutable` type.
    ReuseNestedDelimiter,
    /// [`Self::ReuseNestedDelimiter`], and any member with no reusable
    /// leading word takes LC0 to LC3 whenever its serialized size turns
    /// out to be 1, 2, 4 or 8 bytes.
    ///
    /// What Fast DDS writes. The reuse is tried first: Fast DDS frames
    /// an eight-byte `string` as LC5 and not LC3, so the leading word
    /// wins where there is one.
    ///
    /// Half of this is a writer-time decision — see the type's
    /// documentation.
    ShortWhereItFits,
    /// LC4 with an explicit NEXTINT throughout, whatever the member's
    /// size or shape. Four bytes more per member than any of the three
    /// above, and the form the pre-rebuild ZeroDDS emitters produced.
    ///
    /// No measured implementation writes it. Reach for it when a peer
    /// expects that form — the OMG wire vector V-10 at `e1f3af895` is
    /// pinned in exactly that shape — or when reproducing a recorded
    /// capture byte for byte.
    UniversalNextInt,
    /// LC0 to LC3 wherever the member's **serialized** length is exactly
    /// 1, 2, 4 or 8 octets, LC4 everywhere else, and never a reused
    /// leading word.
    ///
    /// What OpenDDS writes, and the fourth of the four positions: it
    /// answers the second question of the pair above yes and the first
    /// no, which is the combination none of the three others is.
    /// Measured on `struct-mutable-length-codes`, XCDR2, against this
    /// project's [`Self::Compact`]:
    ///
    /// ```text
    /// member                    ours              OpenDDS
    /// string s = "abc"          LC5               LC3
    /// sequence<octet> of 3      LC5               LC4, NEXTINT 7
    /// sequence<short> of 2      LC4, NEXTINT 8    LC3
    /// sequence<long> of 2       LC6               LC4, NEXTINT 12
    /// sequence<double> of 1     LC7               LC4, NEXTINT 12
    /// sequence<Cell> of 1       LC5               LC4
    /// Cell pair[2]              LC5               LC4
    /// long plain[2]             LC4               LC3
    /// Cell nested               LC4               LC3
    /// ```
    ///
    /// Every code on both sides is legal for the member it is on —
    /// §7.4.3.4.2 permits several for the same member — so this is a
    /// reading of the clause and not a defect, and it is the one of the
    /// four that never spends a code on a word the member already
    /// carries.
    ///
    /// Half of it is a writer-time decision, for the same reason
    /// [`Self::ShortWhereItFits`]'s is: `string s` is eight octets
    /// because the sample says `"abc"`, and no plan knows that. The plan
    /// answers where the *type* fixes the size — `long plain[2]` and
    /// `Cell nested` — and `Writer::finish_emheader` patches the rest.
    SizedOrNextInt,
}

impl LengthCodePolicy {
    /// Every policy.
    pub const ALL: [Self; 5] = [
        Self::Compact,
        Self::ReuseNestedDelimiter,
        Self::ShortWhereItFits,
        Self::UniversalNextInt,
        Self::SizedOrNextInt,
    ];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Compact)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::ReuseNestedDelimiter => "reuse-nested-delimiter",
            Self::ShortWhereItFits => "short-where-it-fits",
            Self::UniversalNextInt => "universal-nextint",
            Self::SizedOrNextInt => "sized-or-nextint",
        }
    }

    /// Whether a member's own leading word — a string's length, a
    /// sequence's count, a nested aggregate's DHEADER — is ever spent as
    /// the NEXTINT.
    ///
    /// Three of the five do it in some form; [`Self::UniversalNextInt`]
    /// and [`Self::SizedOrNextInt`] never do, which is what leaves them
    /// with LC0 to LC4 only.
    #[must_use]
    pub const fn reuses_a_leading_word(self) -> bool {
        matches!(
            self,
            Self::Compact | Self::ReuseNestedDelimiter | Self::ShortWhereItFits
        )
    }

    /// Whether a nested aggregate's own DHEADER is reused as the
    /// NEXTINT, which is LC5 over the member rather than LC4 in front
    /// of it.
    #[must_use]
    pub const fn reuses_nested_delimiter(self) -> bool {
        matches!(self, Self::ReuseNestedDelimiter | Self::ShortWhereItFits)
    }

    /// Whether a member with no reusable leading word takes LC0 to LC3
    /// when its serialized size is 1, 2, 4 or 8.
    ///
    /// The plan applies it where the type fixes the size; the writer
    /// applies it to the rest.
    #[must_use]
    pub const fn short_code_where_it_fits(self) -> bool {
        matches!(self, Self::ShortWhereItFits | Self::SizedOrNextInt)
    }

    /// Parse a configuration value.
    ///
    /// Accepts the names [`LengthCodePolicy::as_str`] produces. An
    /// unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "compact" => Some(Self::Compact),
            "reuse-nested-delimiter" => Some(Self::ReuseNestedDelimiter),
            "short-where-it-fits" => Some(Self::ShortWhereItFits),
            "universal-nextint" => Some(Self::UniversalNextInt),
            "sized-or-nextint" => Some(Self::SizedOrNextInt),
            _ => None,
        }
    }
}

/// Which collections carry a `DHEADER` under XCDR2.
///
/// XTypes 1.3 §7.4.3.5 rules (8) through (13): a collection of a
/// primitive element type serialises identically under both versions,
/// and every other collection is delimited under XCDR2. §7.2.2.2 lists
/// the primitive types; an enumeration is a constructed type under
/// §7.2.2.4.1 and is therefore not one.
///
/// # The three implementations disagree
///
/// Measured on 2026-08-09, XCDR2 little-endian, payload bytes:
///
/// | element type | Cyclone | RTI 7.7 | Fast DDS |
/// |---|---|---|---|
/// | `@final struct Cell`, `Cell items[2]` | 20 | 16 | 20 |
/// | `@final struct Cell`, `sequence<Cell>` | 24 | 20 | 24 |
/// | `@appendable struct`, `sequence<>` | 32 | 28 | 32 |
/// | `enum Colour`, `Colour cs[2]` | 12 | 8 | 8 |
/// | `enum Colour`, `sequence<Colour>` | 16 | 12 | 12 |
///
/// The runs are in `tests/fixtures/vendor_bytes.md`. Only Cyclone
/// matches the specification on every row.
///
/// Changing this changes the bytes on the wire. Unlike the length-code
/// policy, a conforming reader cannot absorb the difference: there is
/// no flag in the stream saying whether the four bytes it is looking at
/// are a delimiter or the first element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum CollectionDelimiterPolicy {
    /// Delimit every collection whose element type is not primitive,
    /// enumerations included.
    ///
    /// The default, because it is what §7.4.3.5 says and what Cyclone
    /// DDS emits.
    #[default]
    Specification,
    /// Treat an enumeration as if it were primitive, and delimit the
    /// rest.
    ///
    /// What Fast DDS emits. Select it to interoperate with a Fast DDS
    /// peer over a topic whose type has an enum collection.
    EnumIsPrimitive,
    /// Never delimit a collection, whatever its element type.
    ///
    /// What RTI Connext 7.7 emits. Element-level delimiters an
    /// `@appendable` element type calls for are still written; this
    /// policy is only about the collection's own.
    Never,
}

impl CollectionDelimiterPolicy {
    /// Every policy.
    pub const ALL: [Self; 3] = [Self::Specification, Self::EnumIsPrimitive, Self::Never];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Specification)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Specification => "specification",
            Self::EnumIsPrimitive => "enum-is-primitive",
            Self::Never => "never",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "specification" => Some(Self::Specification),
            "enum-is-primitive" => Some(Self::EnumIsPrimitive),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

/// Which of a `map`'s two halves decides whether the map carries a
/// delimiter under XCDR2.
///
/// Separate from [`CollectionDelimiterPolicy`] because it is a separate
/// question and one measured implementation answers the two
/// differently. That policy says *which element types* exempt a
/// collection; this says which of a map's two element types is asked.
///
/// # The measurement
///
/// `tests/interop-results/sources/vendor-corpus.tsv`, Fast DDS 3.6.1,
/// encoding version 2, both byte orders. Little endian, the leading
/// word of each payload:
///
/// | case | the map | Fast DDS writes |
/// |---|---|---|
/// | `map-key-long` | `map<long, long>` | `03000000` — the entry count |
/// | `map-key-string` | `map<string, long>` | `03000000` — the entry count |
/// | `map-empty` | `map<string, long>` | `00000000` — the entry count |
/// | `map-value-string` | `map<long, string>` | `22000000` — a DHEADER |
/// | `map-value-struct` | `map<long, Cell>` | `22000000` — a DHEADER |
/// | `map-value-sequence` | `map<long, sequence<long>>` | `1c000000` — a DHEADER |
/// | `map-nested` | `map<long, map<long, long>>` | `1c000000` — a DHEADER |
///
/// A `string` **key** exempts nothing and a `string` **value** does not,
/// so the question Fast DDS asks is the value's alone. Twelve recorded
/// rows separate the two readings — `map-empty`, `map-key-string` and
/// `typedef-of-map` in both orders under both of their halves — and
/// they were the last twelve rows of the Fast DDS column that no single
/// setting reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MapDelimiter {
    /// Either half. Rule (14) of XTypes 1.3 §7.4.3.5.3 exempts a map
    /// only where the key *and* the value are primitive, so a
    /// `map<string, long>` is delimited by rule (15).
    ///
    /// The default, and the clause's reading.
    #[default]
    EitherHalf,
    /// The value alone, whatever the key is. Fast CDR 2.3.5's.
    ValueOnly,
}

impl MapDelimiter {
    /// Both readings.
    pub const ALL: [Self; 2] = [Self::EitherHalf, Self::ValueOnly];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::EitherHalf)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EitherHalf => "either-half",
            Self::ValueOnly => "value-only",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "either-half" => Some(Self::EitherHalf),
            "value-only" => Some(Self::ValueOnly),
            _ => None,
        }
    }
}

/// How a `wstring`'s length is counted, and how wide one character is.
///
/// XTypes 1.3 is not ambiguous here. §7.4.1.1.2: "The serialized length
/// of an object of type `String<Char16>` shall be the number of bytes in
/// the CDR buffer taken by the `String<Char16>` characters. This is twice
/// the number of characters in the string", and "The representation
/// shall also not include any terminating NUL character(s)."
/// §7.4.3.5.3 rule (4) writes `O.ssize` as a `UInt32` and then the
/// `Char16` units, annotated "No NUL". The three implementations write
/// three different things anyway.
///
/// # The three implementations disagree
///
/// Measured on 2026-08-09 from `@final struct WStr { wstring w; octet
/// tail; }`, little endian, the `w` member's bytes only:
///
/// | | `w = "ab"`, XCDR2 | `w = "ab"`, XCDR1 |
/// |---|---|---|
/// | Cyclone | `04000000 61006200` | `04000000 61006200` |
/// | RTI 7.7 | `04000000 61006200` | `03000000 61000000 62000000 00000000` |
/// | Fast DDS | `02000000 61006200` | `02000000 61006200` |
///
/// | | `w = "𝄞"` (`U+1D11E`), XCDR2 | XCDR1 |
/// |---|---|---|
/// | Cyclone | `04000000 34d81edd` | `04000000 34d81edd` |
/// | RTI 7.7 | `04000000 34d81edd` | `03000000 34d80000 1edd0000 00000000` |
/// | Fast DDS | `01000000 1ed1` | `01000000 1ed1` |
///
/// The runs are in `tests/fixtures/vendor_bytes.md`. Fast DDS's shape
/// is in its own source rather than in the harness: Fast CDR 2.3.5
/// `src/cpp/Cdr.cpp:884` computes `wstrlen * 2` for the capacity check
/// and then serialises `wstrlen`, and writes each `wchar_t` as two
/// bytes. `wchar_t` is four bytes wide on Linux, so `U+1D11E` leaves as
/// its low sixteen bits, `d11e`. Past the BMP that stops being a length
/// convention and becomes a lost character.
///
/// `U+D11E` is a Hangul syllable and a perfectly well-formed code
/// point. Nothing downstream can see that it is the wrong one: the
/// truncation happened in the sender and left no trace in the bytes.
/// Only where the low sixteen bits land in `D800..=DFFF` does the
/// result fail to be UTF-16 and get refused, and that is luck rather
/// than a check.
///
/// # The reader takes the configured form and never infers one
///
/// The forms are not separable from the bytes. Fast DDS's encoding of
/// `@final struct { wstring w; long n; }` with `w = "ab", n = 7` is
/// `02000000 61006200 07000000`; the specification's encoding of the
/// same type with `w = "a", n = 7` is `02000000 6100`, two bytes of
/// padding, `07000000`. Twelve bytes each, differing only in the
/// content of padding that no clause fixes and that RTI is measured
/// leaving uninitialised. A reader choosing between those two samples
/// would not be resolving an ambiguity, it would be inventing an
/// answer, and it would be taking it from the length field — the field
/// a hostile peer controls, and the one a reader must never let decide
/// how much it reads. So this is a setting, not a detection.
///
/// What the reader owes in exchange is that every mismatch it *can* see
/// is an error and not a shorter string:
///
/// - a length the configured form cannot have produced is refused, and
///   the error names the form the length does fit and the setting that
///   selects it. RTI's XCDR1 count of 3 and Fast DDS's odd character
///   counts are both odd byte counts under [`WstringLengthPolicy::SPECIFICATION`];
/// - code units that are not well-formed UTF-16 are refused rather
///   than replaced. Substituting U+FFFD would hand the caller a
///   character the sender never had;
/// - whatever is left at the end of the frame has to be padding that
///   alignment accounts for, so a short read that shifts every later
///   member ends as an error rather than as a plausible sample.
///
/// Two cases survive all three. An even character count of BMP text
/// from Fast DDS, where the next member's alignment absorbs the bytes
/// the short read left — the twelve bytes above are that case. And a
/// non-BMP code point Fast DDS truncated into a valid BMP one, which
/// arrives as a well-formed string of the right length holding the
/// wrong character. Both are undecidable from the stream, and they are
/// the reason this is a setting at all.
///
/// # The width reaches a scalar `wchar` as well
///
/// [`WstringLengthPolicy::character_width`] is not a `wstring` detail. A `Char16` is
/// one type and each implementation writes it at one width wherever it
/// appears. Measured 2026-08-09, case `wchar_member`,
/// `@final struct WCh { wchar c; octet tail; }` with `c = U+0061`:
///
/// | | XCDR1 LE | XCDR1 BE | XCDR2 LE | XCDR2 BE |
/// |---|---|---|---|---|
/// | Cyclone | `6100 ab` | `0061 ab` | `6100 ab` | `0061 ab` |
/// | RTI 7.7 | `61000000 ab` | `00000061 ab` | `6100 ab` | `0061 ab` |
/// | Fast DDS | `6100 ab` | `0061 ab` | `6100 ab` | `0061 ab` |
///
/// RTI's four-byte XCDR1 `wchar` is the same four-byte character its
/// XCDR1 `wstring` writes, and its two-byte XCDR2 `wchar` is the same
/// two-byte character its XCDR2 `wstring` writes. So the scalar takes
/// [`WstringLengthPolicy::character_width`] and there is no second setting: a policy
/// that said "four-byte `wstring` characters, two-byte `wchar`" would
/// name no implementation.
///
/// This is why [`LayoutKey::offset_key`] carries the policy. A
/// `wstring` plans as [`crate::Extent::Unbounded`] under every variant
/// and moves no offset, but a `wchar` is a sized primitive: at four
/// bytes it aligns to four and shifts every member after it. A cache
/// that served a two-byte plan to a four-byte request would put them
/// all in the wrong place.
///
/// A code point above the BMP has no `wchar` in any variant
/// (§7.2.2.2.1.2.5) — the four-byte form zero-extends a code *unit*,
/// it does not carry a code point.
/// **C1** — what a `wstring`'s length prefix counts.
///
/// One of the three questions [`WstringLengthPolicy`] used to answer at
/// once. Separated because implementations answer them independently:
/// see that type's measured table, where two implementations agree on
/// C1 and part on C3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum WstringTerminator {
    /// No terminator. Eight of the nine measured implementations.
    #[default]
    Absent,
    /// A NUL code unit at the end, inside the length.
    ///
    /// CORBA §9.3.1.6's GIOP 1.1 rule. RTI under XCDR1 alone among the
    /// nine.
    Present,
}

/// **C3** — how wide one UTF-16 code unit is on the wire.
///
/// The transmission code set's width. No GIOP version fixes it and
/// XTypes Table 31 fixes it at two, so it varies independently of C1
/// and C2 — which is the whole reason these are three types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum WstringUnitWidth {
    /// Two octets. XTypes Table 31, Cyclone, OpenDDS, Fast DDS,
    /// omniORB, JacORB.
    #[default]
    Two,
    /// Four octets, a code unit zero-extended — **not** a code point,
    /// so `U+1D11E` stays the surrogate pair `34d80000 1edd0000`.
    ///
    /// RTI under XCDR1, TAO, and both ROS type supports.
    Four,
}

impl WstringUnitWidth {
    /// The width in octets.
    #[must_use]
    pub const fn octets(self) -> u8 {
        match self {
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

/// How a `wstring`'s length is written and how wide its code units are
/// — **C1 × C2 × C3**, a product space and not a list.
///
/// # Why this is a struct and no longer an enum
///
/// It was an enum of five named forms, and the enum was the defect. The
/// five names mixed three independent questions — what the length
/// counts, whether a terminator is included, how wide a code unit is —
/// and a sixth question, byte order, which had no business here at all
/// and has moved to [`WstringUnitOrder`].
///
/// A list of named forms can only hold the combinations somebody has
/// already written down. Measured 2026-08-12 by reconstruction, over
/// `wstring-unbounded-utf8`, `wstring-bounded-utf8`,
/// `wstring-unbounded-empty` and `array-1d-wstring`: every combination
/// of (C1, C2, C3, C4) was encoded and compared against the recorded
/// octets, and exactly one reproduces each row.
///
/// | implementation | C1 counts | C2 terminator | C3 unit | C4 order |
/// |---|---|---|--:|---|
/// | Cyclone | octets | absent | 2 | host, unswapped |
/// | OpenDDS | octets | absent | 2 | stream |
/// | Fast DDS | characters | absent | 2 | stream |
/// | RTI, XCDR1 | characters | **present** | 4 | stream |
/// | ros-cyclone | characters | absent | **4** | stream |
/// | ros-fastrtps | characters | absent | **4** | host, unswapped |
/// | omniORB | octets, mark counted | absent | 2 | byte-order mark |
/// | TAO | octets | absent | 4 | stream |
/// | JacORB | octets | absent | 2 | big-endian always |
///
/// The row that forced the rebuild is either ROS one: **characters,
/// no terminator, four-octet units**. The old enum could not reach it.
/// `CharacterCount` was that length with two-octet units and
/// `SpecificationWideUnits` was four-octet units under the other
/// length; the cell between them had no name, so two implementations
/// wrote a form this project had no way to express. That is what an
/// enumeration standing in for a product space costs, and
/// [`Self::ROS_TYPE_SUPPORT`] is the cell.
///
/// `tests::every_cell_of_the_product_is_reachable` fails if any of
/// the eight cells stops being constructible or stops round-tripping
/// through [`Self::parse`].
///
/// # Position
///
/// Every method here takes a [`WirePosition`], for the reason
/// [`WcharFraming`]'s methods do: a caller must not be able to reach a
/// member's answer for an element by not asking. On this axis the two
/// positions are **measured to agree** — all nine implementations
/// answer the member and the collection element identically, checked
/// 2026-08-12 and guarded by
/// `wstring_length_policy.rs::the_two_positions_still_agree`. That is a
/// checked non-existence, not an assumption, and the argument is here so
/// that the day an implementation parts them there is a place to put it
/// rather than a signature to change.
///
/// # Width, offsets, and the scalar `wchar`
///
/// RTI's four-octet XCDR1 `wchar` is the same four-octet character its
/// XCDR1 `wstring` writes, and its two-octet XCDR2 `wchar` the same as
/// its XCDR2 `wstring`. So the scalar takes C3 and there is no second
/// setting: a policy saying "four-octet `wstring` characters, two-octet
/// `wchar`" would name no implementation.
///
/// This is why [`LayoutKey::offset_key`] carries it. A `wstring` plans
/// as [`crate::Extent::Unbounded`] under every cell and moves no
/// offset, but a `wchar` is a sized primitive: at four octets it aligns
/// to four and shifts every member behind it. A cache that served a
/// two-octet plan to a four-octet request would put them all in the
/// wrong place.
///
/// A code point above the BMP has no `wchar` in any cell
/// (§7.2.2.2.1.2.5) — the four-octet form zero-extends a code *unit*,
/// it does not carry a code point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
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

    /// Octets, no terminator, two-octet units.
    ///
    /// XTypes 1.3 §7.4.1.1.2 and CORBA's GIOP 1.2 `wstring` over a
    /// two-octet code set. The default. Cyclone and OpenDDS under both
    /// versions, RTI under XCDR2, omniORB and JacORB.
    pub const SPECIFICATION: Self = Self::new(
        WstringLengthCounts::Octets,
        WstringTerminator::Absent,
        WstringUnitWidth::Two,
    );

    /// Characters, no terminator, two-octet units.
    ///
    /// What Fast DDS writes under both versions. The writer refuses a
    /// code point outside the BMP in this cell rather than truncating
    /// it the way Fast CDR does: the truncation is undetectable at the
    /// far end, so emitting it would be this project putting a
    /// different character on the wire and calling it interoperability.
    pub const CHARACTER_COUNT: Self = Self::new(
        WstringLengthCounts::Characters,
        WstringTerminator::Absent,
        WstringUnitWidth::Two,
    );

    /// Characters including the terminator, four-octet units.
    ///
    /// CORBA 3.3 part 2 §9.3.1.6's GIOP **1.1** `wstring` over a
    /// four-octet code set, and what RTI Connext 7.7 writes under
    /// XCDR1 — measured 2026-08-12 at both the member and the
    /// collection-element position, byte for byte, with no
    /// RTI-specific rule anywhere. Its XCDR2 output is
    /// [`Self::SPECIFICATION`].
    pub const CHARACTER_COUNT_TERMINATED: Self = Self::new(
        WstringLengthCounts::Characters,
        WstringTerminator::Present,
        WstringUnitWidth::Four,
    );

    /// Octets, no terminator, four-octet units.
    ///
    /// CORBA §9.3.1.6's GIOP 1.2 `wstring` over a four-octet
    /// transmission code set — TAO 2.5.24.
    pub const SPECIFICATION_WIDE_UNITS: Self = Self::new(
        WstringLengthCounts::Octets,
        WstringTerminator::Absent,
        WstringUnitWidth::Four,
    );

    /// Characters, no terminator, four-octet units.
    ///
    /// **The cell the old enumeration could not reach.** Both ROS 2
    /// type supports write it: `rosidl_typesupport_fastrtps` counts
    /// characters like Fast CDR and hands each one over as a
    /// four-octet `wchar_t`, and `rmw_cyclonedds_cpp` walking the
    /// introspection type support arrives at the same octets.
    ///
    /// Measured 2026-08-12 from the recorded corpus. `wstring-unbounded-utf8`,
    /// `plain-cdr1` little endian, `"äöü ሴ Z"`:
    ///
    /// ```text
    /// ros-fastrtps  07000000 e4000000 f6000000 fc000000 20000000 34120000 20000000 5a000000
    /// ros-cyclone   07000000 e4000000 f6000000 fc000000 20000000 34120000 20000000 5a000000
    /// ```
    ///
    /// Seven, for seven characters, over four-octet units and no
    /// terminator. The two are byte-identical here, and they part only
    /// on C4 in a big-endian stream.
    pub const ROS_TYPE_SUPPORT: Self = Self::new(
        WstringLengthCounts::Characters,
        WstringTerminator::Absent,
        WstringUnitWidth::Four,
    );

    /// The cells an implementation has been measured in, for a caller
    /// that wants to sweep the named forms.
    ///
    /// **Not the set of legal values** — that is the eight-cell product,
    /// and [`Self::ALL_CELLS`] enumerates it. A caller that sweeps this
    /// list is sweeping what has been seen, which is the right set for a
    /// vendor comparison and the wrong one for a completeness check.
    pub const MEASURED: [Self; 5] = [
        Self::SPECIFICATION,
        Self::CHARACTER_COUNT,
        Self::CHARACTER_COUNT_TERMINATED,
        Self::SPECIFICATION_WIDE_UNITS,
        Self::ROS_TYPE_SUPPORT,
    ];

    /// Every cell of C1 × C2 × C3, measured or not.
    ///
    /// Eight, and the three that no implementation writes are as
    /// constructible and as nameable as the five that are. A
    /// completeness check runs over this; a vendor sweep runs over
    /// [`Self::MEASURED`].
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

    /// Whether this is the default.
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

    /// The name a configuration uses.
    ///
    /// Every one of the eight cells has one, and every one round-trips
    /// through [`Self::parse`]. A cell an implementation was measured in
    /// renders under the name it shipped with; the three nobody writes
    /// render as their coordinates, `counts/terminator/width`. That a
    /// cell nobody has named is still writable on a command line and
    /// still readable in a diagnostic is the property the enumeration
    /// did not have, and it is why the ROS form was unreachable for as
    /// long as the names were the values.
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
            // The three cells no measured implementation writes. Named
            // by coordinate rather than left unnamed: a value that
            // cannot be printed cannot be reported in a diagnostic, and
            // a value that cannot be parsed cannot be reached from a
            // command line to be measured against.
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

    /// The coordinate form's grammar, for a help text next to
    /// [`Self::MEASURED`]'s five named cells.
    ///
    /// Kept here, beside [`Self::parse`], because a caller building a
    /// help text out of the named cells alone would leave the three
    /// unnamed ones unreachable from anything it printed — the defect
    /// [`Self::ALL_CELLS`]'s own documentation describes for the old
    /// enum. One constant next to the grammar it describes, rather than
    /// a copy at each call site that could drift from what
    /// [`Self::parse`] actually accepts.
    pub const COORDINATE_FORM_HELP: &'static str =
        "octets|characters / unterminated|terminated / 2|4";

    /// Parse a configuration value.
    ///
    /// A named cell, or the three coordinates separated by `/`. Every
    /// value [`Self::as_str`] produces parses back to the same cell, and
    /// the coordinate form reaches all eight whether or not they carry a
    /// name. An unknown value returns `None` rather than falling back to
    /// the default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "specification" => return Some(Self::SPECIFICATION),
            "character-count" => return Some(Self::CHARACTER_COUNT),
            "character-count-terminated" => return Some(Self::CHARACTER_COUNT_TERMINATED),
            "specification-wide-units" => return Some(Self::SPECIFICATION_WIDE_UNITS),
            "ros-type-support" => return Some(Self::ROS_TYPE_SUPPORT),
            _ => {}
        }
        let mut parts = value.split('/');
        let counts = match parts.next()? {
            "octets" => WstringLengthCounts::Octets,
            "characters" => WstringLengthCounts::Characters,
            _ => return None,
        };
        let terminator = match parts.next()? {
            "unterminated" => WstringTerminator::Absent,
            "terminated" => WstringTerminator::Present,
            _ => return None,
        };
        let unit = match parts.next()? {
            "2" => WstringUnitWidth::Two,
            "4" => WstringUnitWidth::Four,
            _ => return None,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(Self::new(counts, terminator, unit))
    }
    /// **C1** at a position — what the length prefix counts.
    ///
    /// An accessor and not the field, for the reason the type's
    /// documentation gives: a caller that reads `policy.counts`
    /// directly has answered for every position by not asking about
    /// any, which is exactly the shape that let a member's `wchar`
    /// framing stand in for an element's through five emitters. The
    /// nine implementations answer both positions alike, measured
    /// 2026-08-12, so the argument is unused — and it is the argument
    /// that keeps that a measurement rather than an assumption.
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

    /// How many octets one `Char16` takes, in a `wstring` and as a
    /// scalar `wchar` alike.
    ///
    /// Takes the position for the reason the type's documentation
    /// gives. The two positions are measured to agree, so the argument
    /// is unused in the body and named `_position` rather than dropped
    /// from the signature: a caller still cannot reach this without
    /// saying where it is asking from.
    #[must_use]
    pub const fn character_width(self, _position: WirePosition) -> usize {
        self.unit.octets() as usize
    }

    /// The same width as a byte count, for the one caller that needs it
    /// narrow: [`WcharFraming::scalar_layout`], whose alignment answer
    /// has to be a `u8` and must not be produced by a fallible
    /// conversion.
    #[must_use]
    pub const fn char16_octets(self, _position: WirePosition) -> u8 {
        self.unit.octets()
    }

    /// The same width as the planner wants it.
    ///
    /// A separate signature and not a cast at the call site: a plan that
    /// narrowed a `usize` into a `u32` with a fallback would answer 2
    /// for a width it did not recognise, and a wrong width is a wrong
    /// offset for every member behind the member.
    #[must_use]
    pub const fn char16_size(self, _position: WirePosition) -> u32 {
        self.unit.octets() as u32
    }
}

/// How a scalar `wchar` is framed.
///
/// A `wstring` is delimited as a whole and a `wchar` is not, so the two
/// have never had to be one setting — until an ORB was asked. CORBA 3.3
/// part 2 §9.3.1.6 gives the scalar its own framing, and it is the one
/// place where the GIOP edition changes the *shape* of a member and not
/// only its length field.
///
/// # The three forms
///
/// Measured 2026-08-10 on the Linux test host,
/// `@final struct Sample { wchar a; wchar b; }` with `a = U+00E4` and
/// `b = U+1234`, both byte orders:
///
/// ```text
///          little endian                 big endian
/// zerodds  e400 3412                     00e4 1234
/// omniorb  02 00e4  02 1234              02 00e4  02 1234
/// tao      04 e4000000  04 34120000      (host order only)
/// jacorb   (big endian only)             02 00e4  02 1234
/// ```
///
/// - [`Self::Fixed`] is XTypes 1.3 Table 31 and CORBA's GIOP **1.1**
///   `wchar`: a code unit of [`WstringLengthPolicy::character_width`]
///   octets, aligned to its own width, in the stream's byte order.
/// - [`Self::GiopLengthPrefixed`] is CORBA's GIOP **1.2** `wchar`: an
///   octet holding how many octets follow, then that many, aligned to
///   **one**. omniORB and JacORB write it wherever a `wchar` appears.
/// - [`Self::GiopLengthPrefixedMemberOnly`] is TAO's: the length octet
///   for a `wchar` reached as a **member**, and a bare fixed-width unit
///   for one reached as a collection **element**.
///
/// # The framing is per position, not per type
///
/// The third variant exists because `tao_idl`'s generated code takes
/// two different paths to the same IDL type, and the measurement is
/// what says so. Re-measured 2026-08-11 from the recorded corpus,
/// `plain-cdr1` little endian:
///
/// ```text
/// case                     tao                            omniorb
/// primitive-wchar          04 e4000000  04 34120000       02 00e4  02 1234
/// array-1d-primitives      00 e4000000 34120000 5a000000  02 00e4 02 1234 02 005a
/// sequence-primitives-…    03000000 e4000000 34120000 …   (no wchar sequence run)
/// ```
///
/// In `array-1d-primitives` TAO writes one pad octet from offset 15 to
/// the four-octet alignment and then three bare units; omniORB writes
/// no padding and keeps the count octet on every element. So the two
/// ORBs part on the element and agree on the member, and a single
/// per-type answer reproduces at most one of them. [`WcharPosition`] is
/// the axis, and every method here takes it so that a caller cannot
/// reach the wrong one by omission.
///
/// # It is not a width, it is an alignment and a size
///
/// A `wchar` under [`Self::Fixed`] is a primitive of two or four octets
/// that aligns to two or four. Under [`Self::GiopLengthPrefixed`] it is
/// `1 + width` octets that align to **one**, so `@final struct { octet
/// head; wchar c; }` puts `c` at offset 1 and not at offset 2. Every
/// member behind it moves. That is why this policy is in
/// [`LayoutKey::offset_key`] and why it could not be modelled as another
/// value of the width.
///
/// The octet count is fixed per implementation rather than per value:
/// omniORB writes `02` for both U+00E4 and U+1234, TAO `04` for both.
/// It is the transmission code set's width, which is
/// [`WstringLengthPolicy::character_width`] again, so the plan stays a
/// fixed size and the two settings do not have to agree by hand.
///
/// # What it does not reach
///
/// A `wchar` inside a `wstring`. CORBA does not length-prefix the units
/// of a `wstring` — measured above, omniORB's `wstring` is a BOM and
/// then bare units — so this setting is the scalar's alone, and
/// [`WstringLengthPolicy`] stays the `wstring`'s.
///
/// A `wchar` member of a struct that is *itself* a collection element
/// is a member: the position is where the `wchar` is reached from and
/// not how deep the walk got there. See [`WcharPosition::inside`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum WcharFraming {
    /// A fixed-width code unit, aligned to its own width.
    ///
    /// XTypes 1.3 Table 31, and CORBA GIOP 1.1's `wchar`. The default,
    /// and what all five DDS implementations write.
    #[default]
    Fixed,
    /// A one-octet length, then that many octets, aligned to one.
    ///
    /// CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 `wchar`, which XTypes
    /// Table 31 replaced. What omniORB 4.3.3 and JacORB 3.9 write in
    /// every position, and the reason a topic with a scalar `wchar` in
    /// it could not be exchanged with an ORB in either direction before
    /// this policy existed.
    ///
    /// The failure it replaces was loud rather than silent — omniORB
    /// answers a bare two-octet `wchar` with `CORBA::MARSHAL`, because
    /// the first octet reads as a count of `0xE4` octets and runs off
    /// the end. Loud, and still a topic nobody could carry.
    GiopLengthPrefixed,
    /// The length octet for a member, a bare fixed-width unit for a
    /// collection element. TAO 2.5.24.
    ///
    /// Not a softening of [`Self::GiopLengthPrefixed`] and not a
    /// tolerance: it is the second of the two paths `tao_idl` generates,
    /// measured in the corpus rows named on the type. An element under
    /// this framing is byte for byte [`Self::Fixed`] at the same
    /// code-unit width — which is why `--dialect rti`, whose XCDR1
    /// `wchar` is four octets fixed, already reproduced TAO's arrays
    /// while `--dialect tao` did not.
    GiopLengthPrefixedMemberOnly,
}

impl WcharFraming {
    /// Every framing.
    pub const ALL: [Self; 3] = [
        Self::Fixed,
        Self::GiopLengthPrefixed,
        Self::GiopLengthPrefixedMemberOnly,
    ];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Fixed)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::GiopLengthPrefixed => "giop-length-prefixed",
            Self::GiopLengthPrefixedMemberOnly => "giop-length-prefixed-member-only",
        }
    }

    /// Whether a `wchar` reached at `position` carries an octet count of
    /// its own.
    ///
    /// The position is an argument and not a default because the two
    /// answers differ for one measured implementation, and a caller that
    /// could omit it would silently take the member's answer for an
    /// element — which is exactly the defect this argument closed.
    #[must_use]
    pub const fn has_length_octet(self, position: WcharPosition) -> bool {
        match self {
            Self::Fixed => false,
            Self::GiopLengthPrefixed => true,
            Self::GiopLengthPrefixedMemberOnly => position.is_member(),
        }
    }

    /// The size and alignment of a `wchar` at a given code-unit width,
    /// in a given position.
    ///
    /// Returned as one pair rather than as two methods because the two
    /// answers are not independent: the length-prefixed form is both one
    /// octet longer *and* aligned to one, and a caller that took the
    /// size from here and the alignment from the width would put every
    /// following member in the wrong place.
    ///
    /// The alignment is a `u8` and the width a `u8` for the same reason
    /// [`WstringLengthPolicy::char16_size`] has its own signature: a
    /// caller that narrowed a `u32` with a fallback would answer some
    /// plausible alignment for a width it did not recognise, and a wrong
    /// alignment is a wrong offset for every member behind the member.
    /// Both matches here are exhaustive, so a variant added without an
    /// answer does not compile.
    #[must_use]
    pub const fn scalar_layout(self, position: WcharPosition, character_width: u8) -> (u8, u32) {
        if self.has_length_octet(position) {
            (1, 1 + character_width as u32)
        } else {
            (character_width, character_width as u32)
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fixed" => Some(Self::Fixed),
            "giop-length-prefixed" => Some(Self::GiopLengthPrefixed),
            "giop-length-prefixed-member-only" => Some(Self::GiopLengthPrefixedMemberOnly),
            _ => None,
        }
    }
}

/// How wide a scalar `wchar` is on the wire.
///
/// # Why this is not [`WstringLengthPolicy`]'s C3
///
/// It was, and the coupling was wrong. A scalar `wchar` and a
/// `wstring`'s code units both take a width, the two agree in eight of
/// the nine measured implementations, and the ninth is why this axis
/// exists: **`rmw_cyclonedds_cpp` 1.3.4 writes a two-octet scalar
/// `wchar` and four-octet `wstring` code units.** Measured against
/// `tests/interop-results/sources/ros-corpus.tsv`, encoding version 1,
/// little endian, which is the only order that type support writes:
///
/// ```text
/// primitive-wchar          e400 3412
/// wstring-unbounded-utf8   07000000 e4000000 f6000000 fc000000 20000000
///                          34120000 20000000 5a000000
/// ```
///
/// `ä` and `ሴ` take two octets each as members; the same `ä` takes four
/// inside the `wstring`, whose length `07000000` is a count of the
/// seven characters. `rosidl_typesupport_fastrtps_c` 2.2.2 writes four
/// in both positions — `e400000034120000` for the same pair — so the two
/// ROS 2 type supports part here and nowhere else in the wide-character
/// family except the unit order.
///
/// Before this axis existed the two questions were one, so the
/// `ros-cyclone` column's `wstring` rows and its `wchar` rows were
/// reproduced by two **different** settings and by no single one. That
/// is what a dialect for it could not be built out of.
///
/// # What it does not decide
///
/// The width of a `wstring`'s code units, which is
/// [`WstringUnitWidth`]; whether a count octet precedes the value,
/// which is [`WcharFraming`]; and the byte order, which is
/// [`WstringUnitOrder`] for a `wstring` and the stream's for a scalar
/// under every DDS implementation measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum WcharWidth {
    /// Two octets. XTypes 1.3 Table 31, and seven of the nine measured
    /// implementations: Cyclone, OpenDDS, Fast DDS, RTI under XCDR2,
    /// omniORB, JacORB and `rmw_cyclonedds_cpp`.
    #[default]
    Two,
    /// Four octets, a code unit zero-extended — **not** a code point,
    /// so a value above the BMP is still refused.
    ///
    /// RTI under XCDR1, TAO, and `rosidl_typesupport_fastrtps_c`.
    Four,
}

impl WcharWidth {
    /// Both widths.
    pub const ALL: [Self; 2] = [Self::Two, Self::Four];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Two)
    }

    /// The width in octets.
    #[must_use]
    pub const fn octets(self) -> u8 {
        match self {
            Self::Two => 2,
            Self::Four => 4,
        }
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Two => "2",
            Self::Four => "4",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "2" => Some(Self::Two),
            "4" => Some(Self::Four),
            _ => None,
        }
    }
}

/// Where a `wchar` is reached from.
///
/// The one axis [`WcharFraming`] varies along, and the reason it exists
/// is a measurement rather than a symmetry: `tao_idl` writes GIOP 1.2's
/// length octet for a `wchar` member and no length octet for a `wchar`
/// inside an array or a sequence. Nothing else in this crate's layout
/// varies with position, so this enum is deliberately small and
/// deliberately not a general traversal context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum WirePosition {
    /// Reached as a member of a structure, a union branch, or as the
    /// whole of a top-level value. The default, because it is the
    /// position every value was in before collections were measured.
    ///
    /// **Measured** on [`WcharFraming`] — `tao_idl` writes GIOP 1.2's
    /// length octet here and not at [`Self::Element`] — and on
    /// [`WstringLengthPolicy`], where it is measured to *agree* with
    /// [`Self::Element`] in all nine implementations.
    #[default]
    Member,
    /// Reached as an element of an array or a sequence.
    ///
    /// A `wstring`'s code units are **not** this: they are not `wchar`s
    /// on the wire at all and are laid out by [`WstringLengthPolicy`].
    ///
    /// **Measured** on [`WcharFraming`] and on
    /// [`WstringLengthPolicy`]; see [`Self::Member`].
    Element,
    /// The discriminator of a union.
    ///
    /// **Prospective.** No axis has been measured to answer this
    /// differently from [`Self::Member`]. It is here because a
    /// discriminator is reached by a path of its own in every emitter,
    /// so an axis that starts varying with it has somewhere to say so
    /// without a signature change.
    Discriminator,
    /// A key holder's copy of a member.
    ///
    /// **Prospective.** [`KeyHolderExtensibility`] and
    /// [`KeyHolderOrder`] are today whole-holder settings rather than
    /// per-position ones; if a per-member difference is ever measured
    /// this is where it lands.
    KeyHolderMember,
    /// The sentinel that closes an XCDR1 parameter list.
    ///
    /// **Measured** on [`PidFlagPolicy`], which is the axis that reads
    /// this variant: [`PidFlagPolicy::must_understand_at`] and
    /// [`PidFlagPolicy::impl_extension_at`] both take a position, the
    /// writer asks them at [`Self::Member`] for a member id and here for
    /// `PID_LIST_END`, and the four implementations' answers differ
    /// between the two positions. The measurement is on
    /// [`PidFlagPolicy`].
    Sentinel,
    /// The `PID_EXTENDED` marker that opens an XCDR1 parameter's
    /// extended header.
    ///
    /// **Measured** on [`PidFlagPolicy`]. The flag was
    /// `PlCdr1HeaderPolicy::marker_must_understand`, on the argument
    /// that the marker exists only under
    /// [`PlCdr1HeaderPolicy::AlwaysExtended`] and its flag is therefore
    /// a property of the form. Eight recorded rows contradict that:
    /// `xcdr.rs::needs_extended_pid` writes the marker under
    /// [`PlCdr1HeaderPolicy::Specification`] too — for a member id above
    /// `PID_SHORT_MAX_ID` or a value the short length cannot hold — and
    /// the four implementations answer it as follows, on
    /// `annotation-hashid-implicit` and `annotation-id-sparse`:
    ///
    /// ```text
    /// implementation  framing form       the marker
    /// Cyclone         AlwaysExtended     7f01
    /// RTI             Specification      7f01
    /// Fast DDS        ShortTrueLength    3f01
    /// OpenDDS         ShortTrueLength    ff01  — with FLAG_IMPL_EXTENSION
    /// ```
    ///
    /// Two forms answer alike and two forms answer differently, which is
    /// judgement 3 of `docs/wire-axes-followup.md`: a field two forms
    /// answer differently and two forms answer alike is not a field of
    /// the form. The three [`PidFlagPolicy`] values answer all four
    /// implementations here with no fourth value.
    ExtendedMarker,
}

impl WirePosition {
    /// Every position.
    pub const ALL: [Self; 6] = [
        Self::Member,
        Self::Element,
        Self::Discriminator,
        Self::KeyHolderMember,
        Self::Sentinel,
        Self::ExtendedMarker,
    ];

    /// The positions on which some axis has a **measured** difference or
    /// a **measured** agreement, as against those carried prospectively.
    ///
    /// Kept apart because measured and provided-for must not look alike:
    /// a reader who cannot tell them apart reads the next measurement as
    /// a confirmation of something that was never checked.
    /// [`Self::Member`] and [`Self::Element`] are measured on
    /// [`WcharFraming`] (they differ) and on [`WstringLengthPolicy`]
    /// (they agree, in nine implementations); [`Self::Sentinel`] and
    /// [`Self::ExtendedMarker`] are measured on [`PidFlagPolicy`], which
    /// reads both. The other two are prospective.
    pub const MEASURED: [Self; 4] = [
        Self::Member,
        Self::Element,
        Self::Sentinel,
        Self::ExtendedMarker,
    ];

    /// Whether some axis has been measured at this position.
    #[must_use]
    pub const fn is_measured(self) -> bool {
        matches!(
            self,
            Self::Member | Self::Element | Self::Sentinel | Self::ExtendedMarker
        )
    }

    /// Whether this is the member position.
    #[must_use]
    pub const fn is_member(self) -> bool {
        matches!(self, Self::Member)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Element => "element",
            Self::Discriminator => "discriminator",
            Self::KeyHolderMember => "key-holder-member",
            Self::Sentinel => "sentinel",
            Self::ExtendedMarker => "extended-marker",
        }
    }

    /// The position an element of a collection is reached in.
    ///
    /// A named constructor rather than a literal at the call sites so
    /// that "descending into a collection" is one decision written once:
    /// a sequence of arrays of `wchar` reaches the same answer as an
    /// array of `wchar`, and neither has to say so itself.
    #[must_use]
    pub const fn inside_collection() -> Self {
        Self::Element
    }

    /// The position the members of an aggregate are reached in.
    ///
    /// An aggregate resets the position: a `wchar` member of a struct
    /// that is itself an array element takes the member's framing,
    /// because `tao_idl` reaches it through the struct's own marshaller.
    /// Unmeasured against TAO — the corpus has no such case — and it is
    /// the reading the generated code's shape gives, recorded here so
    /// that the day a case is added there is something to contradict.
    #[must_use]
    pub const fn inside(self) -> Self {
        Self::Member
    }
}

/// The name the position axis shipped under, when it reached only
/// `wchar`.
///
/// Judgement 1 of `docs/wire-axes-followup.md` makes the position first
/// class on every axis, so the type is [`WirePosition`] now. The alias
/// stays because the argument for the rename is in the rename, not in
/// breaking every caller in one commit.
pub type WcharPosition = WirePosition;

/// In what byte order a `wstring`'s code units go out, and whether a
/// byte-order mark travels with them.
///
/// One axis and one decision, because the two halves are one: an
/// implementation that writes a mark reads the units by it, and one that
/// writes no mark has to have a rule for a stream that carries none.
///
/// # What was measured
///
/// The write direction, 2026-08-10 on the Linux test host,
/// `@final struct Sample { wstring w; octet tail; }` with `w = "a𝄞b"`:
///
/// ```text
/// little-endian stream
///   zerodds  08000000      6100 34d8 1edd 6200  ab
///   omniorb  0a000000 fffe 6100 34d8 1edd 6200  ab
/// big-endian stream
///   zerodds  00000008      0061 d834 dd1e 0062  ab
///   omniorb  00000010 fffe e400 …                     ("äöü ሴ Z", the long case)
/// ```
///
/// omniORB swaps the **length** word into the stream's order and leaves
/// the mark and the content alone, always little-endian on this x86-64
/// host. In a big-endian stream its mark therefore reads as `U+FFFE`,
/// the byte-swapped mark, which is precisely the signal that the units
/// that follow are opposite to the stream — so omniORB's output is
/// self-describing where Cyclone's identically-ordered output
/// (`WstringLengthPolicy::UnswappedUnits`) is not.
///
/// The read direction, from `docs/corba-column.md`'s `array-1d-wstring`
/// run over a little-endian stream carrying no mark:
///
/// ```text
/// zerodds  {"items":["","äZ","ሴ"]}
/// omniorb  {"items":["","娀","㐒"]}
/// jacorb   {"items":["","娀","㐒"]}
/// ```
///
/// Both ORBs decode the units as UTF-16 **big-endian** whatever the
/// stream says. That is not a defect in either: XTypes §7.4.1.1.2 makes
/// the units the stream's through `ESWAP` (row A10 of the inventory),
/// and the OMG code-set rule for a GIOP 1.2 `wstring` makes them
/// UTF-16's own — big-endian in the absence of a mark. Each side is
/// self-consistent and they cannot talk, and the failure is the silent
/// kind: a `wstring` comes back well formed, of the right length, and
/// holding different characters.
///
/// # Why this is a second setting and not a fifth `WstringLengthPolicy`
///
/// Because it crosses the length convention rather than extending it.
/// omniORB's mark rides on the octet count of
/// [`WstringLengthPolicy::SPECIFICATION`]; nothing stops a GIOP 1.1
/// count from carrying one too. Folding the order into the length enum
/// is what produced `WstringLengthPolicy::UnswappedUnits`, which is
/// this axis's little-endian point wearing a length policy's name, and
/// doing it again would give eight variants for two questions.
///
/// `WstringLengthPolicy::UnswappedUnits` therefore stays where it
/// shipped, and the pair `(UnswappedUnits, anything but
/// [`Self::Stream`])` is **refused** rather than resolved — see
/// `WstringUnitOrder::conflicts_with_length`, which is retired. Two settings that both
/// decide one byte order, with a precedence rule between them, is the
/// shape that produces a wrong character and no error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum WstringUnitOrder {
    /// The stream's own byte order, no mark.
    ///
    /// XTypes 1.3 §7.4.1.1.2 through `ESWAP`, and what Cyclone, RTI and
    /// Fast DDS write in little endian and RTI and Fast DDS write in
    /// big. The default.
    #[default]
    Stream,
    /// UTF-16 big-endian whatever the stream says, no mark.
    ///
    /// What omniORB and JacORB **decode**, and the CORBA rule for a
    /// GIOP 1.2 `wstring` that carries no mark. Reachable as
    /// `--wstring-units big-endian-always` and selected by
    /// [`Dialect::JacOrb`].
    ///
    /// JacORB's write direction cannot tell this apart from
    /// [`Self::Stream`]: `org.jacorb.orb.CDROutputStream` has no
    /// `setLittleEndian`, so every JacORB byte string measured here is
    /// big-endian and the two forms coincide in it. What is measured is
    /// JacORB's *reader*, which takes the units big-endian out of a
    /// little-endian stream — so this is the form a JacORB peer reads
    /// correctly, which is the half that decides what a writer should
    /// do. Recorded as an extrapolation from the read direction, in the
    /// place a reader of [`Dialect::JacOrb`] will look for it.
    BigEndianAlways,
    /// A byte-order mark, then the units, both little-endian.
    ///
    /// What omniORB 4.3.3 writes. The mark is `U+FEFF`, its two octets
    /// are counted in the length, and it is not a character of the
    /// string.
    ///
    /// On **read** the mark decides: `FEFF` in the stream's order means
    /// the units follow in the stream's order, `FFFE` means they are
    /// opposite, and no mark at all means big-endian — the CORBA rule,
    /// and the one that makes this variant read a [`Self::BigEndianAlways`]
    /// peer as well as an omniORB one.
    ///
    /// On **write** the mark and the units are little-endian, because
    /// that is what was measured, on x86-64, in both stream orders.
    /// "Little-endian" and not "the host's order" for the reason
    /// `WstringLengthPolicy::UnswappedUnits` gives at more length: a
    /// big-endian host running omniORB would presumably write big-endian
    /// units and a plain `FEFF`, and that is not something a peer on a
    /// little-endian host can produce or this project can measure.
    ///
    /// XTypes §7.4.1.1.2 forbids the mark outright — "The representation
    /// shall also not include any … byte-order mark" — so no dialect of a
    /// DDS implementation selects this and the default never will.
    ByteOrderMark,
    /// The units always little-endian, whatever the stream says, and no
    /// mark.
    ///
    /// **This point reproduces a peer misreading its own output**, and
    /// it is on this axis because it is a byte order. It arrived as
    /// `WstringLengthPolicy::UnswappedUnits` — a byte order wearing a
    /// length policy's name — and judgement 2 of
    /// `docs/wire-axes-followup.md` moved it here, which also retired
    /// the `(UnswappedUnits, non-Stream)` conflict rule: two settings
    /// that both decide one byte order, with a precedence between them,
    /// is the shape that produces a wrong character and no error. One
    /// axis cannot conflict with itself.
    ///
    /// Cyclone swaps the length word for a big-endian stream and does
    /// not swap the content, so its big-endian `"ab"` leaves as
    /// `00000004 61006200` where RTI and Fast DDS write
    /// `00000004 00610062`. Read as the big-endian stream the
    /// encapsulation identifier says it is, Cyclone's `"ab"` is U+6100
    /// U+6200 — two different characters, well-formed UTF-16, the right
    /// length, and nothing in the frame saying so. Cyclone reads its own
    /// bytes back correctly because it unswaps them the same way; every
    /// conforming peer gets the other string.
    ///
    /// `rosidl_typesupport_fastrtps` does the same, measured
    /// 2026-08-12: `wstring-unbounded-utf8` big endian is
    /// `00000007 e4000000 …`, a big-endian length over little-endian
    /// four-octet units. Two implementations, and the second one is
    /// what makes this a point on the axis rather than one product's
    /// defect.
    ///
    /// In little endian it is byte for byte [`Self::Stream`], which is
    /// why the divergence took a big-endian run to find.
    ///
    /// "Little-endian" and not "the host's order" because that is what
    /// was measured: Cyclone `memcpy`s its native UTF-16 buffer, and
    /// every run is on x86-64. A big-endian host would presumably write
    /// big-endian units and be byte-identical to [`Self::Stream`] there
    /// — unmeasured, and not something a peer on a little-endian host
    /// can produce.
    ///
    /// No [`Dialect`] takes it as a default, including
    /// [`Dialect::Cyclone`]: a default has to be the form a conforming
    /// reader gets right. It exists because refusing to model it means
    /// nobody can exchange a `wstring` with Cyclone over big endian at
    /// all, and "we decline to talk to that peer" is not a wire question
    /// the project gets to leave unanswered.
    HostUnswapped,
}

impl WstringUnitOrder {
    /// Every order.
    pub const ALL: [Self; 4] = [
        Self::Stream,
        Self::BigEndianAlways,
        Self::ByteOrderMark,
        Self::HostUnswapped,
    ];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Stream)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stream => "stream",
            Self::BigEndianAlways => "big-endian-always",
            Self::ByteOrderMark => "byte-order-mark",
            Self::HostUnswapped => "host-unswapped",
        }
    }

    /// Whether a mark is written, and expected on read.
    #[must_use]
    pub const fn has_mark(self) -> bool {
        matches!(self, Self::ByteOrderMark)
    }

    /// In what byte order one code unit goes out.
    ///
    /// Takes the position for the reason [`WstringLengthPolicy`]'s
    /// methods do. Measured to agree at the member and the collection
    /// element in all nine implementations.
    #[must_use]
    pub const fn writes_little_endian_units(self, _position: WirePosition) -> bool {
        matches!(self, Self::ByteOrderMark | Self::HostUnswapped)
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "stream" => Some(Self::Stream),
            "big-endian-always" => Some(Self::BigEndianAlways),
            "byte-order-mark" => Some(Self::ByteOrderMark),
            "host-unswapped" => Some(Self::HostUnswapped),
            _ => None,
        }
    }
}

/// How a PL_CDR1 parameter is framed, and how the list ends.
///
/// XCDR1 frames a `@mutable` member as `{ PID, length }` — the short
/// header of XTypes 1.3 §7.4.1.2.2 — or as `PID_EXTENDED` followed by a
/// 32-bit id and a 32-bit length, §7.4.1.2.3, which the short form's
/// sixteen bits cannot express. The clause says when the extended form
/// is *required*; it does not forbid using it when it is not, and one
/// implementation uses nothing else.
///
/// # The three implementations disagree
///
/// Measured on 2026-08-09 from `@mutable struct Ext { long a; double b;
/// short c; }` with `a=7, b=1.5, c=-3`, XCDR1 little endian:
///
/// ```text
/// cyclone  017f0800 00000000 04000000 07000000
///          017f0800 01000000 08000000 0000000000 00f83f
///          017f0800 02000000 04000000 fdff0000
///          027f0000                                        (56)
///
/// rti      00000400 07000000
///          01000800 0000000000 00f83f
///          02000400 fdff0000
///          027f0000                                        (32)
///
/// fastdds  00000400 07000000
///          01000800 0000000000 00f83f
///          02000200 fdff 0000
///          023f0000                                        (32)
/// ```
///
/// The run is in `tests/fixtures/vendor_bytes.md`. Three things vary,
/// and no two of them vary independently, which is why this is one
/// policy and not three:
///
/// - **which header form.** Cyclone writes `PID_EXTENDED` for every
///   member and never the short one. The other two use the short form
///   wherever it fits — including for a `string` or a `sequence`, whose
///   length the plan cannot bound. `needs_extended_pid` therefore plans
///   the short form for an unbounded member and `Writer::finish_pid`
///   rewrites the header on the sample that actually overruns sixteen
///   bits, which is the only way to write RTI's 24-byte `mut_string`
///   and still be correct on a 70 KiB one.
/// - **what the length field counts.** `short c` is two bytes followed
///   by two of padding. Cyclone and RTI both declare `4`; Fast DDS
///   declares `2` and leaves the padding outside. XTypes 1.3 §7.4.1.2
///   gives the exact length, and that is the default
///   ([`PlCdr1HeaderPolicy::Specification`]). DDSI-RTPS 2.5 §9.4.2.11
///   says the length is always a multiple of four — this project's own
///   RTPS decoder refuses a `parameterLength` that is not one
///   (`crates/rtps/src/parameter_list.rs`, "ParameterList length not
///   4-byte aligned") — and that reading is a named setting,
///   [`PlCdr1HeaderPolicy::ShortRoundedLength`]. A reader realigns after
///   the value either way; whether it accepts the other convention's
///   length is [`PlCdr1Reading`].
///
/// So this policy answers two things and not three. The **sentinel's**
/// flag was a third field of it and the **`PID_EXTENDED` marker's** was
/// a fourth; both are [`PidFlagPolicy`] now, asked at
/// [`WirePosition::Sentinel`] and [`WirePosition::ExtendedMarker`].
///
/// The sentinel moved because it is not a framing question: OpenDDS
/// writes Fast DDS's framing — the short header, the true length — with
/// a member id no framing here produces, and the sentinel has to be
/// answerable separately from the form to say so.
///
/// The marker moved for the same reason, and against a claim this
/// comment used to make: "the marker exists only under
/// [`Self::AlwaysExtended`], which is why its flag is a property of the
/// form". It does not. `needs_extended_pid` writes the marker under
/// [`Self::Specification`] as well, for a member id above
/// `PID_SHORT_MAX_ID` or a value the short length cannot hold, and RTI
/// — which is [`Self::Specification`] — sets the bit there. Eight
/// recorded rows differ in exactly that octet. Cyclone and RTI write
/// different forms and the same marker; RTI and Fast DDS both produce
/// the marker without being [`Self::AlwaysExtended`] and write
/// different markers. [`WirePosition::ExtendedMarker`] carries the
/// table.
///
/// A *member's* own must-understand bit is not one of these three, and
/// it used to be. It is not a PL_CDR1 question: Fast DDS omits the flag
/// under both encoding versions and Cyclone and RTI write it under
/// both, so [`MustUnderstandPolicy`] carries it and this policy does
/// not.
///
/// Under XCDR2 the framing disagreement disappears — all three produce
/// the same thirty bytes for `ext_mutable`, because the EMHEADER has no
/// short form to choose. The must-understand disagreement does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum PlCdr1HeaderPolicy {
    /// The short header wherever §7.4.1.2.2 allows it and the declared
    /// length exactly the length of the serialized member.
    ///
    /// The default, as XTypes 1.3 §7.4.1.2 has it: "the value of the
    /// parameter length is the exact length of the serialized member. It
    /// does not account for any padding bytes that may follow the
    /// serialized member". The next parameter starts on a four-byte
    /// boundary. DDSI-RTPS 2.5 §9.4.2.11 says the length is always a
    /// multiple of four; that reading is [`Self::ShortRoundedLength`].
    #[default]
    Specification,
    /// The short header wherever §7.4.1.2.2 allows it and the declared
    /// length rounded up to four.
    ///
    /// DDSI-RTPS 2.5 §9.4.2.11 ("the length is always a multiple of
    /// four"), and what RTI Connext 7.7 writes for a sample whose type
    /// stays within the short form's length — see
    /// [`Self::ExtendedForConstructedInLargeSamples`]. A ParameterList
    /// reader that checks the multiple-of-four rule accepts it.
    ShortRoundedLength,
    /// `PID_EXTENDED` for every member, with the must-understand bit on
    /// the marker, and the same rounded length.
    ///
    /// What Cyclone DDS writes. Twelve bytes of header per member
    /// instead of four, and legal under §7.4.1.2.3 — the clause makes
    /// the extended form mandatory above the short form's limits, not
    /// exclusive to them.
    AlwaysExtended,
    /// The short header and the value's own length, with the padding
    /// outside it.
    ///
    /// What Fast DDS writes, and what a ParameterList reader that
    /// checks the multiple-of-four rule refuses. Select it to
    /// reproduce a Fast DDS peer's XCDR1 bytes.
    ShortTrueLength,
    /// `PID_EXTENDED` for every member whose type is not a primitive or
    /// an enumeration, the short header for those; lengths as
    /// [`Self::ShortRoundedLength`].
    ///
    /// What RTI Connext 7.7 writes for a sample past the step
    /// [`Self::ExtendedForConstructedInLargeSamples`] describes. A plan
    /// is given it by [`LayoutKey::for_sample`], not by a dialect.
    ExtendedForConstructed,
    /// [`Self::ShortRoundedLength`] for a sample whose type stays within the
    /// short form's length, [`Self::ExtendedForConstructed`] for one that
    /// can exceed it; [`LayoutKey::for_sample`] makes the choice for the
    /// sample's type, and every type nested in it is framed by that
    /// choice.
    ///
    /// What RTI Connext 7.7 writes. Measured on rtiddsgen's type support
    /// through `PRESTypePlugin_interpretedSerialize`, XCDR1 little
    /// endian: `@mutable struct T { long id; sequence<long, 100> small;
    /// sequence<octet, N> mid; }` frames every member short up to
    /// N = 65068 and `small` and `mid` with `PID_EXTENDED` from
    /// N = 65069, `id` short throughout; without `small` the step is at
    /// 65485. A `@mutable struct Inner { long a; sequence<long, 100> s;
    /// }`, small on its own, writes `s` extended as a member of a sample
    /// past the step. `boolean`, `octet`, `char`, `wchar`, `int8`,
    /// `uint8`, the integers, `float`, `double`, an `enum` and a typedef
    /// of `long` keep the short header there; a `string`, a sequence, a
    /// `@final` struct of two `long`, `long[2]` and a union arm take the
    /// extended one. The same in a `@mutable` union and for the
    /// `@optional` members of an `@appendable` struct. Where the step
    /// lies is `crate::large_sample`'s count.
    ExtendedForConstructedInLargeSamples,
}

impl PlCdr1HeaderPolicy {
    /// Every policy.
    pub const ALL: [Self; 6] = [
        Self::Specification,
        Self::ShortRoundedLength,
        Self::AlwaysExtended,
        Self::ShortTrueLength,
        Self::ExtendedForConstructed,
        Self::ExtendedForConstructedInLargeSamples,
    ];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Specification)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Specification => "specification",
            Self::ShortRoundedLength => "short-rounded-length",
            Self::AlwaysExtended => "always-extended",
            Self::ShortTrueLength => "short-true-length",
            Self::ExtendedForConstructed => "extended-for-constructed",
            Self::ExtendedForConstructedInLargeSamples => {
                "extended-for-constructed-in-large-samples"
            }
        }
    }

    /// Whether the parameter that just ended writes its own trailing
    /// padding to the next four-byte boundary.
    ///
    /// Every header writer aligns itself before writing, so *between*
    /// two parameters the two readings produce the same octets. They
    /// part at the end of the stream: RTI writes the trailing pad
    /// after a final `@optional` parameter and Fast DDS does not —
    /// measured 2026-08-24 on `annotation-must-understand-unannotated`
    /// under XCDR1, 24 octets against 22 for the same values.
    /// [`Self::ShortTrueLength`] is Fast DDS's spelling of PL_CDR1 and
    /// carries this answer with the true length it is named for.
    #[must_use]
    pub const fn pads_after_value(self) -> bool {
        !matches!(self, Self::ShortTrueLength)
    }

    /// Whether every member takes the extended header, whatever its id
    /// and length.
    #[must_use]
    pub const fn always_extended(self) -> bool {
        matches!(self, Self::AlwaysExtended)
    }

    /// Whether a member whose type is not a primitive or an enumeration
    /// takes the extended header, whatever its id and length.
    #[must_use]
    pub const fn extends_constructed(self) -> bool {
        matches!(self, Self::ExtendedForConstructed)
    }

    /// Whether the form depends on the sample's type, and
    /// [`LayoutKey::for_sample`] has a choice to make.
    #[must_use]
    pub const fn decided_per_sample(self) -> bool {
        matches!(self, Self::ExtendedForConstructedInLargeSamples)
    }

    /// Whether a `@mutable` member's declared length covers the value's
    /// trailing padding.
    #[must_use]
    pub const fn pads_declared_length(self) -> bool {
        matches!(
            self,
            Self::ShortRoundedLength
                | Self::AlwaysExtended
                | Self::ExtendedForConstructed
                | Self::ExtendedForConstructedInLargeSamples
        )
    }

    /// The same question for an `@optional` member of a type that is not
    /// `@mutable`, which one implementation answers the other way.
    ///
    /// Measured 2026-08-09 with `@final struct Sample { long a;
    /// @optional octet b; long c; };`, `b = 0xAB`, XCDR1 little endian:
    /// Cyclone declares `1` for the one-byte value where the same
    /// Cyclone declares `4` for the two-byte `short c` of `ext_mutable`.
    /// RTI declares `4` in both places and Fast DDS the true length in
    /// both. So the padded reading is not a property of the
    /// implementation but of the role the parameter is in, and
    /// [`PlCdr1HeaderPolicy::pads_declared_length`] alone made
    /// `Dialect::Cyclone` wrong about one of the two whichever way it
    /// was set. `crates/ast-layout/tests/fixtures/vendor_bytes.md` §1.
    #[must_use]
    pub const fn pads_declared_optional_length(self) -> bool {
        matches!(
            self,
            Self::ShortRoundedLength
                | Self::ExtendedForConstructed
                | Self::ExtendedForConstructedInLargeSamples
        )
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "specification" => Some(Self::Specification),
            "short-rounded-length" => Some(Self::ShortRoundedLength),
            "always-extended" => Some(Self::AlwaysExtended),
            "short-true-length" => Some(Self::ShortTrueLength),
            "extended-for-constructed" => Some(Self::ExtendedForConstructed),
            "extended-for-constructed-in-large-samples" => {
                Some(Self::ExtendedForConstructedInLargeSamples)
            }
            _ => None,
        }
    }
}

/// How tolerant a PL_CDR1 reader is of a stream another implementation
/// framed.
///
/// XTypes 1.3 §7.4.1.2.1 asks a reader to be "robust to any setting of the
/// FLAG_MUST_UNDERSTAND bit and accept the parameter nevertheless" and
/// §7.4.1.2.2 calls the short and the extended header of one parameter
/// "totally equivalent". Implementations differ in exactly those places:
/// the flags on `PID_EXTENDED` and on `PID_LIST_END` (Cyclone and RTI
/// `0x7f..`, Fast DDS `0x3f..`, OpenDDS `0xff01`/`0x3f02`), the header form
/// (Cyclone writes the extended header for every member) and whether a
/// parameter's length counts the padding after its value. A reader built
/// for one framing ([`PlCdr1HeaderPolicy`], [`PidFlagPolicy`]) refuses a
/// stream in another with `WireError::WrongFraming`, naming the setting
/// that would read it. This setting is the other answer.
///
/// The default is [`Self::Strict`]: the reader takes the framing it was
/// configured for and reports a stream in any other. A stream that is
/// malformed (a length that fits neither convention, a value that overruns
/// its parameter) is refused under both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum PlCdr1Reading {
    /// The reader accepts the framing it is configured for and reports
    /// the flags, the header form and the length convention of another.
    #[default]
    Strict,
    /// The reader accepts any flags on `PID_EXTENDED` and `PID_LIST_END`,
    /// the short and the extended header of a parameter whichever form it
    /// is configured for, and a declared length that is either the
    /// value's own length or that length rounded up to four.
    Tolerant,
}

impl PlCdr1Reading {
    /// Every reading.
    pub const ALL: [Self; 2] = [Self::Strict, Self::Tolerant];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Strict)
    }

    /// Whether the reader takes a stream in another framing.
    #[must_use]
    pub const fn tolerant(self) -> bool {
        matches!(self, Self::Tolerant)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Tolerant => "tolerant",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "strict" => Some(Self::Strict),
            "tolerant" => Some(Self::Tolerant),
            _ => None,
        }
    }
}

/// What flags a PL_CDR1 parameter id carries, asked per
/// [`WirePosition`].
///
/// XTypes 1.3 §7.4.1.2 splits the sixteen-bit field: the low fourteen
/// bits are the member id, bit 14 is `FLAG_MUST_UNDERSTAND` and bit 15
/// is `FLAG_IMPL_EXTENSION`. Two ids in the reserved range are not
/// members — `PID_EXTENDED` (`0x3F01`) and `PID_LIST_END` (`0x3F02`) —
/// and the clause says nothing about what flags either of them carries.
///
/// # The axis, and why the sentinel is a position on it
///
/// Measured from the recorded corpus, `struct-mutable`
/// (`@mutable struct { long a; double b; short c; }`) and
/// `evolution-mutable-mustunderstand`
/// (`@id(1) long a; @id(2) long b; @id(3) @must_understand long c;`),
/// XCDR1 big endian:
///
/// ```text
/// implementation  a member's PID                      the sentinel
/// Cyclone         extended form; must-understand as declared   7f02
/// RTI             0001, 0002, and 4003 for the declared one    7f02
/// Fast DDS        0001, 0002, 0003 — never the flag            3f02
/// OpenDDS         c001, c002, c003 — both flags, always        3f02
/// ```
///
/// Cyclone and RTI write the same sentinel octets while answering the
/// member as declared, and OpenDDS writes a bare sentinel while setting
/// every member's flags unconditionally. The sentinel's answer therefore
/// cannot be read off the member's: OpenDDS differs from Cyclone at both
/// positions, from Fast DDS at the member only, and agrees with Fast DDS
/// at the sentinel. Two independent implementations agreeing at the
/// sentinel while differing at the member is judgement 3 of
/// `docs/wire-axes-followup.md`, and it is what makes
/// [`WirePosition::Sentinel`] a measured position rather than a
/// property of the list.
///
/// # What is not on this axis
///
/// - **A member's own `must_understand`.** That is
///   [`MustUnderstandPolicy`], because it is answered the same way under
///   both encoding versions and this axis exists only under version 1.
///   The two compose: [`Self::ImplExtensionOnMembers`] sets the bit
///   whatever the member declared, and a member that also declares it
///   gets the same octets.
/// - **The `eMemberHeader`'s own flags in the extended form.**
///   [`Self::ImplExtensionOnMembers`] reaches the sixteen-bit parameter
///   id and not the 28-bit member id behind a `PID_EXTENDED` marker.
///   Measured, and a checked non-existence rather than an omission:
///   OpenDDS's `annotation-hashid-implicit` writes
///   `ff010008 0343172c 00000004` — both flags on the marker, and a
///   **bare** `eMemberHeader` for a member that declares nothing. That
///   is what this policy's writers do.
///
/// # The third position
///
/// [`WirePosition::ExtendedMarker`] is answered here too, and it moved
/// off [`PlCdr1HeaderPolicy`] for the reason the sentinel did. All four
/// implementations, on `annotation-hashid-implicit` and
/// `annotation-id-sparse`:
///
/// ```text
/// implementation  framing form       the marker
/// Cyclone         AlwaysExtended     7f01
/// RTI             Specification      7f01
/// Fast DDS        ShortTrueLength    3f01
/// OpenDDS         ShortTrueLength    ff01
/// ```
///
/// Two framing forms answer alike and two answer differently, so the
/// marker's flags are not a property of the form. The three values here
/// answer all four with no fourth value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum PidFlagPolicy {
    /// Nothing on a member id, `FLAG_MUST_UNDERSTAND` on the sentinel.
    ///
    /// The default, and what Cyclone DDS and RTI Connext 7.7 both write.
    #[default]
    SentinelMustUnderstand,
    /// Nothing anywhere: a bare member id and a bare `0x3f02`.
    ///
    /// What Fast DDS writes.
    Bare,
    /// `FLAG_IMPL_EXTENSION | FLAG_MUST_UNDERSTAND` on every member id,
    /// whatever the member declared, and a bare sentinel.
    ///
    /// What OpenDDS writes — `c0000004` where Fast DDS writes
    /// `00000004`, with the same short header, the same true length and
    /// the same `3f02`. No dialect selects it, because OpenDDS is a
    /// recorded peer rather than a preset here; it is reached by
    /// `--pid-flags impl-extension-members`.
    ImplExtensionOnMembers,
}

impl PidFlagPolicy {
    /// Every policy.
    pub const ALL: [Self; 3] = [
        Self::SentinelMustUnderstand,
        Self::Bare,
        Self::ImplExtensionOnMembers,
    ];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::SentinelMustUnderstand)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SentinelMustUnderstand => "sentinel-must-understand",
            Self::Bare => "bare",
            Self::ImplExtensionOnMembers => "impl-extension-members",
        }
    }

    /// Whether a parameter id at `position` carries
    /// `FLAG_MUST_UNDERSTAND` whatever the member declared.
    ///
    /// The member's own declaration is [`MustUnderstandPolicy`] and is
    /// OR-ed on top of this by the writer; this answers the part that
    /// does not depend on the declaration.
    ///
    /// Only [`WirePosition::Member`], [`WirePosition::Sentinel`] and
    /// [`WirePosition::ExtendedMarker`] have a parameter id at all. The
    /// other two answer `false` because there is nothing there to carry
    /// a flag, not because a policy decided so.
    #[must_use]
    pub const fn must_understand_at(self, position: WirePosition) -> bool {
        matches!(
            (self, position),
            (
                Self::SentinelMustUnderstand,
                WirePosition::Sentinel | WirePosition::ExtendedMarker
            ) | (
                Self::ImplExtensionOnMembers,
                WirePosition::Member | WirePosition::ExtendedMarker
            )
        )
    }

    /// Whether a parameter id at `position` carries
    /// `FLAG_IMPL_EXTENSION`.
    #[must_use]
    pub const fn impl_extension_at(self, position: WirePosition) -> bool {
        matches!(
            (self, position),
            (
                Self::ImplExtensionOnMembers,
                WirePosition::Member | WirePosition::ExtendedMarker
            )
        )
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "sentinel-must-understand" => Some(Self::SentinelMustUnderstand),
            "bare" => Some(Self::Bare),
            "impl-extension-members" => Some(Self::ImplExtensionOnMembers),
            _ => None,
        }
    }
}

/// Whether a member's `must_understand` reaches the wire.
///
/// XTypes 1.3 says it does, in both encoding versions and without a
/// condition. §7.4.1.2.2 for the XCDR1 parameter header:
/// `FLAG_MUST_UNDERSTAND` "shall be set if and only if the
/// must_understand property of the member is set to true"; §7.4.3.4.2
/// for the XCDR2 EMHEADER's `M_FLAG`, which is the same property in the
/// same place in the header's top bit.
///
/// # One implementation never writes it, in either version
///
/// Measured 2026-08-09, case `mut_mustunderstand`,
/// `@mutable struct M { @id(1) @must_understand long a; @id(2) long b; };`
/// with `a = 7, b = -2`, little endian:
///
/// ```text
///           XCDR1                           XCDR2
/// cyclone   017f0800 010000|40| 04000000    10000000 0100 00a0 07000000
/// rti       01|40|0400 07000000             10000000 0100 00a0 07000000
/// fastdds   01000400 07000000               10000000 0100 0020 07000000
/// ```
///
/// The bit is in a different place in each form — bit 14 of the short
/// parameter id, `FLAG_2` of the extended `eMemberHeader`, bit 31 of the
/// EMHEADER — and Fast DDS omits it in all three. So this is one
/// property of one implementation and not a property of a framing, which
/// is why it is its own policy and not a fourth axis of
/// [`PlCdr1HeaderPolicy`]. Carrying it there made `Dialect::FastDds`
/// right about the XCDR1 half and wrong about the XCDR2 half, which is
/// the inventory's I11.
///
/// The default writes the bit. A member declared `@must_understand`
/// that goes out looking skippable is a member a peer will skip, which
/// is the failure this project ranks above all others.
///
/// # The clause implies the bit on a `@key` member, and three
/// implementations do not write it
///
/// §7.2.2.4.4.4.8, verbatim: "Key members shall never be optional, and
/// they shall always have their 'must understand' attribute set to
/// true." That is the type system's clause and not the data
/// representation's, so it reaches every encoding version through
/// whichever bit the framing puts the attribute in.
///
/// Measured on `evolution-mutable-key`
/// (`@mutable struct { @id(1) @key long a; @id(2) long b; @id(3) @key
/// long k; }`), XCDR2 big endian:
///
/// ```text
/// cyclone   00000018 20000001 00000007 20000002 00000009 20000003 0000000b
/// rti       00000018 20000001 …        20000002 …        20000003 …
/// fastdds   00000018 20000001 …        20000002 …        20000003 …
/// opendds   00000018 a0000001 …        20000002 …        a0000003 …
/// ```
///
/// OpenDDS sets `M_FLAG` on the two `@key` members and on neither of the
/// others; Cyclone, RTI and Fast DDS set it on none. Three
/// implementations against a clause with no second reading, and one with
/// it. The clause is not silent here — it is the one place in this
/// project's measured set where the departure is **this tree's** and not
/// a peer's, so it is a value of this axis and
/// [`Dialect::Specification`] takes it.
///
/// Under XCDR1 the question does not arise in the corpus: OpenDDS writes
/// [`PidFlagPolicy::ImplExtensionOnMembers`], which puts
/// `FLAG_MUST_UNDERSTAND` on **every** parameter id whatever the member
/// declared, so its `@key` members are indistinguishable from the rest.
/// The two rows this closes are XCDR2's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MustUnderstandPolicy {
    /// The flag is set wherever the member declares it, in whichever of
    /// the three places the framing puts it.
    ///
    /// The default, §7.4.1.2.2's and §7.4.3.4.2's reading of the
    /// attribute, and what Cyclone DDS, RTI Connext 7.7 and Fast DDS all
    /// write for a `@key` member — that is, without §7.2.2.4.4.4.8's
    /// implication.
    #[default]
    Declared,
    /// The flag is never written, in either encoding version.
    ///
    /// What Fast DDS writes. Select it to reproduce a Fast DDS peer's
    /// bytes; it does not change what this project's *reader* does with
    /// a flag it finds, which is §7.2.2.4.4.4.6's discard either way.
    Omitted,
    /// The flag is set wherever the member declares it **and** wherever
    /// §7.2.2.4.4.4.8 implies it: on every `@key` member.
    ///
    /// The clause read whole, and what OpenDDS writes.
    /// [`Dialect::Specification`] selects it, because the specification
    /// dialect is what the clause-by-clause column means; no vendor
    /// dialect does, because three vendors were measured not writing it.
    DeclaredAndKey,
    /// [`Self::DeclaredAndKey`], and the discriminator of a `@mutable`
    /// union carries the flag as well.
    ///
    /// §7.2.2.4.4.4.6: "In a union type, the discriminator member shall
    /// always have the 'must understand' attribute set to true." The
    /// discriminator has a member header in a `@mutable` union (rule (27)
    /// under XCDR2, rule (28) under XCDR1), and that header's `M_FLAG`
    /// (§7.4.3.4.2) or `FLAG_MUST_UNDERSTAND` (§7.4.1.2.1) is the
    /// attribute. RTI Connext 7.7 and OpenDDS were measured writing
    /// `union-mutable` and `evolution-union-unselected` with the bit
    /// clear on the discriminator (`20000000 00000001` under XCDR2); the
    /// other measured toolchains refuse a `@mutable` union. So this is the
    /// clause read whole, and [`Dialect::Specification`] takes it; the
    /// vendors' reading is [`Self::DeclaredAndKey`].
    DeclaredKeyAndDiscriminator,
}

impl MustUnderstandPolicy {
    /// Every policy.
    pub const ALL: [Self; 4] = [
        Self::Declared,
        Self::Omitted,
        Self::DeclaredAndKey,
        Self::DeclaredKeyAndDiscriminator,
    ];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Declared)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Omitted => "omitted",
            Self::DeclaredAndKey => "declared-and-key",
            Self::DeclaredKeyAndDiscriminator => "declared-key-and-discriminator",
        }
    }

    /// Whether a member declaring `must_understand` writes the flag.
    #[must_use]
    pub const fn writes_flag(self) -> bool {
        !matches!(self, Self::Omitted)
    }

    /// Whether `@key` implies the attribute, per §7.2.2.4.4.4.8.
    #[must_use]
    pub const fn key_implies_the_flag(self) -> bool {
        matches!(
            self,
            Self::DeclaredAndKey | Self::DeclaredKeyAndDiscriminator
        )
    }

    /// Whether the discriminator of a `@mutable` union carries the flag,
    /// per §7.2.2.4.4.4.6.
    #[must_use]
    pub const fn discriminator_is_understood(self) -> bool {
        matches!(self, Self::DeclaredKeyAndDiscriminator)
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "declared" => Some(Self::Declared),
            "omitted" => Some(Self::Omitted),
            "declared-and-key" => Some(Self::DeclaredAndKey),
            "declared-key-and-discriminator" => Some(Self::DeclaredKeyAndDiscriminator),
            _ => None,
        }
    }
}

/// How wide an enumeration's holder is on the wire.
///
/// **This axis has one specified answer and one defect.** XTypes 1.3
/// §7.3.1.2.1.9 gives `@bit_bound` on an enumeration and Table 32 (§7.4.1.1.3) the
/// serialised form: the holder is the width the bound calls for. There
/// is no second reading of the clause, and
/// [`Self::BitBoundDroppedByFrontEnd`] is not one — it reproduces two
/// products whose IDL front ends never carry the annotation as far as a
/// serialiser.
///
/// # The measurement, which is about the front ends and not the octets
///
/// M1 of `docs/wire-axes-followup.md`, taken over
/// `@bit_bound(8) enum Colour` and its 16-bit twin:
///
/// | front end | what it does with the annotation |
/// |---|---|
/// | `rtiddsgen` 4.7.0 | refuses at parse: `Only 32 bit bounds are accepted for enumerations.` No files generated |
/// | `fastddsgen` 4.3.0 | accepts, emits `enum class Colour : int32_t` and `BitBound bit_bound_Colour = 32;` for **both** bounds |
/// | `opendds_idl` | `error in lookup of symbol: bit_bound`; the serialiser is `strm << static_cast<ACE_CDR::Long>(enumval)` |
///
/// Cyclone DDS honours the bound — `enum-bitbound-8-plain` under XCDR1
/// big endian is `0200 ab`, one octet a value, and the 16-bit case is
/// `0002 0000 ab`. So Fast DDS's and OpenDDS's agreement on four octets
/// is the agreement of two front ends that dropped the same annotation,
/// which is the addendum to judgement 3: an implementation's octets are
/// evidence about an axis only where the implementation was in a
/// position to answer differently.
///
/// # Why it is a setting all the same
///
/// A compatibility flag has to be able to reproduce a peer's defect,
/// precisely because it is a defect. A peer whose front end drops
/// `@bit_bound` still puts four octets on the wire; a reader of this
/// project's one octet is three octets out of step from there on, and
/// every member behind the enumeration lands somewhere else. "Not an
/// axis" answers the model question and is not a reason to stop counting
/// the row.
///
/// # It reaches an enumeration and not a bitmask
///
/// M1 measured enumerations. `@bit_bound` on a `bitmask` is a different
/// construct in a different clause (§7.4.1.1.3.2, Table 33) and no
/// measurement here says what those front ends do with it, so the arms
/// that used to read `Enumeration | Bitmask` together are split and this
/// policy is applied to the enumeration half only.
///
/// # What it does not reach
///
/// OpenDDS's `enum-bitbound-{8,16}-value` rows carry a second front-end
/// answer on top of this one: `opendds_idl` numbers enumerators by
/// ordinal and drops `@value`, so `@value(200)` goes out as `00000002`.
/// The holder is the same four octets its `-plain` rows write; the
/// contents are not. That is the inventory's E1 and no width setting
/// reaches it —
/// `crates/wire-probe/tests/enum_holder_width.rs::opendds_closes_on_the_holder_and_not_on_the_numbering`
/// asserts the non-reproduction rather than leaving those rows out.
///
/// # No dialect selects the second value
///
/// Fast DDS and OpenDDS are recorded peers here rather than presets, for
/// the same reason [`PidFlagPolicy::ImplExtensionOnMembers`] is reached
/// only by name. `--enum-holder bit-bound-dropped` reaches it, and so
/// does the configuration of that name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum EnumHolderWidth {
    /// The `@bit_bound` decides: 1, 2, 4 or 8 octets.
    ///
    /// The default, the clause, and what Cyclone DDS writes.
    #[default]
    Declared,
    /// Four octets whatever the annotation says.
    ///
    /// What Fast DDS and OpenDDS write, because neither front end
    /// carries the bound to a serialiser. Named for the cause so that a
    /// reader does not take it for a reading of the specification.
    BitBoundDroppedByFrontEnd,
}

impl EnumHolderWidth {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Declared, Self::BitBoundDroppedByFrontEnd];

    /// The width a front end that drops the annotation is left with —
    /// the default `enum`'s, XTypes 1.3 §7.3.1.2.1.9.
    pub const DROPPED_BOUND: u16 = 32;

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Declared)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::BitBoundDroppedByFrontEnd => "bit-bound-dropped",
        }
    }

    /// The bound an enumeration's holder is built from, given what the
    /// IDL declared.
    ///
    /// The one place the declared bound crosses into a wire width.
    /// [`crate::enumeration_bit_bound`] is what call sites use; this is
    /// the arithmetic behind it, separated so that the crossing can be
    /// tested without an `Ast`.
    #[must_use]
    pub const fn holder_bound(self, declared: u16) -> u16 {
        match self {
            Self::Declared => declared,
            Self::BitBoundDroppedByFrontEnd => Self::DROPPED_BOUND,
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "declared" => Some(Self::Declared),
            "bit-bound-dropped" => Some(Self::BitBoundDroppedByFrontEnd),
            _ => None,
        }
    }
}

/// Whether a case label of a narrow signed discriminator can be
/// negative.
///
/// IDL 4.2 §7.4.1.4.4.2.4 makes a case label a value **of the
/// discriminator's type**, and XTypes 1.3 §7.4.3.5.3 rule (26) writes
/// the discriminator and then the member the labels select. Nothing in
/// either clause is width-dependent: `case -13:` on an `int8` selects
/// its branch for `_d = -13` exactly as `case -1234567:` on a `long`
/// does.
///
/// # What one implementation does instead
///
/// Cyclone DDS 11.0.1 compares the label against the discriminator's
/// octets read **unsigned**, so `f3` is 243 and not `-13`, and `cfc7` is
/// 53191 and not `-12345`. No label matches, and what happens next is
/// whether the union declared a `default:`:
///
/// | the union | what Cyclone writes | measured on |
/// |---|---|---|
/// | no `default:` | the discriminator and **no branch at all** | `union-discriminator-{int8,short}`, four octets shorter |
/// | a `default:` | the discriminator and the union's **storage read through the default branch** | `combined-everything`, four and eight octets longer |
///
/// It is the label's width and its sign together. `union-discriminator-long`
/// carries `case -1234567:` and agrees byte for byte;
/// `union-discriminator-{octet,uint8,ushort}` carry unsigned narrow
/// labels and agree byte for byte. Only the pair diverges.
///
/// # The storage, and why it is not the member zero-padded
///
/// `combined-everything-unannotated`, `case -1: long whole; default:
/// double fraction;`, `whole = 0x7fffffff`, XCDR1:
///
/// ```text
/// le  cyclone   ffff 0000 00000000   ffffff7f 00000000
/// be  cyclone   ffff 0000 00000000   00000000 7fffffff
/// ```
///
/// The discriminator, padding to the `double`'s eight-alignment, and
/// then eight octets. The big-endian row is the same eight octets
/// reversed **whole**: a member zero-padded to the branch's width would
/// give `7fffffff 00000000` there. So what is written is the union's
/// storage — the selected member's octets in the host's order at offset
/// zero, zeros above them — emitted as one value of the *default*
/// branch's width in the stream's byte order.
///
/// The zero octets are guaranteed rather than lucky:
/// `wireprobe-zerodds-c-gen` declares the sample as
/// `static {root_type} zd_sample;`, and §6.7.9 of the C standard
/// zero-initialises a static object.
///
/// This is a **host-storage** dependence, the same class as
/// [`WstringUnitOrder::HostUnswapped`], and it is recorded here for the
/// same reason: the corpus was measured on x86-64 and "little-endian"
/// is what was measured, not "the host's order". The C column reaches
/// it through a real C union and would follow a big-endian host; the
/// Rust and Python columns synthesise the little-endian octets. The two
/// coincide on every host this project measures on and would part on a
/// big-endian one — unmeasured, and stated rather than left to be
/// discovered.
///
/// # No dialect selects the second value
///
/// Not even [`Dialect::Cyclone`], which is this project's default. A
/// default has to be the form a conforming reader gets right, and this
/// one drops the selected branch off the wire: a peer reading
/// `union-discriminator-short` takes the member *after* the union for
/// the branch. That is a silent wrong value, and shipping it to every
/// caller who wrote `--dialect cyclone` would be shipping the defect.
/// `--union-labels negative-unmatched-below-32` reaches it, and so does
/// the configuration of that name — the same construction
/// [`EnumHolderWidth::BitBoundDroppedByFrontEnd`] and
/// [`PidFlagPolicy::ImplExtensionOnMembers`] are reached by.
///
/// # What it reaches, and what it does not
///
/// [`crate::union_negative_matches_nothing`] is the one crossing, and it
/// answers two questions.
///
/// The **discriminator** has to be a signed integer primitive narrower
/// than 32 bits — `int8` and `short`. Not `octet`, `char` or `boolean`,
/// none of which can carry a negative label; not `long` or `long long`,
/// which were measured to agree; and **not an enumeration**
/// discriminator: `union-discriminator-enum` declares no negative
/// `@value`, so nothing measured says what Cyclone does with one and
/// answering would be a claim about octets nobody has.
///
/// Every **branch** has to be a primitive of one, two, four or eight
/// octets, because the storage above is what a `default:` branch is read
/// through and a `string` or `sequence` branch is read as a *pointer*.
/// `union-member-sequence` is
/// `switch (short) { case -1: sequence<long> ids; default: string text; }`,
/// its `-1` is the union's only label, and
/// `tests/interop-results/sources/vendor-corpus.tsv` records Cyclone on
/// it and on its twin — both encoding versions, both byte orders — as
/// `refused — the probe was killed: Segmentation fault`. There are no
/// octets to reproduce, so the setting does not reach the shape and this
/// project writes the clause's frame there: a complete frame a
/// conforming reader can use.
///
/// It is **not** a planner setting. A union's extent is the
/// discriminator plus the widest branch under both values — with a
/// `default:` Cyclone writes the widest branch's storage, and without
/// one the extent still bounds what any value of the discriminator could
/// produce. Nothing here reaches an offset, so it stays out of
/// [`LayoutKey::offset_key`], and that is a claim rather than an
/// omission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum UnionLabelMatch {
    /// A case label is compared against the discriminator's own signed
    /// value.
    ///
    /// The clause, and what RTI, Fast DDS and this project write.
    #[default]
    SignExtended,
    /// A negative label on a signed discriminator narrower than 32 bits
    /// matches nothing.
    ///
    /// What Cyclone DDS writes. Named for the behaviour and not for the
    /// vendor, so that a reader does not take it for a reading of
    /// §7.4.1.4.4.2.4.
    NegativeLabelUnmatchedBelow32,
}

impl UnionLabelMatch {
    /// Both.
    pub const ALL: [Self; 2] = [Self::SignExtended, Self::NegativeLabelUnmatchedBelow32];

    /// The width at or above which a signed label still matches.
    ///
    /// Thirty-two: `union-discriminator-long`'s `case -1234567:` was
    /// measured to match and `union-discriminator-short`'s `case
    /// -12345:` was measured not to.
    pub const MATCHING_WIDTH: u16 = 32;

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::SignExtended)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignExtended => "sign-extended",
            Self::NegativeLabelUnmatchedBelow32 => "negative-unmatched-below-32",
        }
    }

    /// Whether a label of the given value, on a discriminator of the
    /// given signedness and width, selects its branch.
    ///
    /// The arithmetic behind [`crate::union_negative_matches_nothing`],
    /// separated so that the crossing can be tested without an `Ast`.
    #[must_use]
    pub const fn label_matches(self, label: i128, signed: bool, bits: u16) -> bool {
        match self {
            Self::SignExtended => true,
            Self::NegativeLabelUnmatchedBelow32 => {
                !(signed && bits < Self::MATCHING_WIDTH && label < 0)
            }
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "sign-extended" => Some(Self::SignExtended),
            "negative-unmatched-below-32" => Some(Self::NegativeLabelUnmatchedBelow32),
            _ => None,
        }
    }
}

/// In what order a `map`'s entries go on the wire.
///
/// XTypes 1.3 §7.4.3.5.3 rules (14) and (15) give `MAP_TYPE` as a length
/// and a run of key-value pairs and say **nothing** about the order of
/// the pairs. Rule (16) is the same for encoding version 1. The order is
/// therefore not a reading of the clause in either direction; it is
/// whatever the binding's container does.
///
/// # What one implementation does
///
/// `fastddsgen` 4.3.0 maps an IDL `map<K,V>` to `std::map<K,V>`, which
/// is ordered by key, so Fast DDS serialises entries in ascending key
/// order whatever order the sample declared them in. Measured on
/// `map-key-long`, entries `[(-1234567, 1), (2147483647, -2), (1, 3)]`,
/// `plain-cdr1` big endian:
///
/// ```text
/// ours    00000003 ffed2979 00000001 7fffffff fffffffe 00000001 00000003 ffffffff
/// theirs  00000003 ffed2979 00000001 00000001 00000003 7fffffff fffffffe ffffffff
/// ```
///
/// A permutation, and nothing else: the count is the same, the length is
/// the same and every entry is present.
///
/// The evidence is complete rather than illustrative. **Exactly the four
/// `map-key-*` cases whose sample is not already in ascending key order
/// diverge, and every map case whose sample already is agrees.**
/// `map-key-boolean` is `[true, false]`, `map-key-char` is
/// `['a', 'Z', '~']`, `map-key-long` is `[-1234567, 2147483647, 1]` and
/// `map-key-ulonglong` is `[16045690984833335998, 7]`; `map-bounded`,
/// `map-nested`, `map-value-sequence`, `map-value-string`,
/// `map-value-struct`, `sequence-map`, `array-1d-map`,
/// `evolution-mutable-map` and `map-key-string` are all ascending
/// already and all agree byte for byte.
///
/// # The order is the key's own, and the corpus separates three of them
///
/// Not the encoded octets and not the canonical text — the key type's
/// own value order, which is what `std::map`'s default `std::less`
/// gives:
///
/// - `map-key-long` puts `-1234567` first, so a **signed** integer key
///   is compared signed. An unsigned reading would put it last.
/// - `map-key-ulonglong` puts `7` before `16045690984833335998`, so an
///   **unsigned** integer key is compared unsigned. A signed reading of
///   `0xdeadbeefdeadc2be` is negative and would put it first.
/// - `map-key-string`'s `["", "alpha", "üሴ"]` stays as declared, so a
///   `string` key is compared by **unsigned** octets — `std::string`'s
///   `char_traits<char>::compare`, which is `memcmp` semantics. A signed
///   octet comparison reads `üሴ`'s leading `c3` as negative and would
///   put that entry first, which Fast DDS does not.
///
/// # What the corpus does not separate, stated rather than assumed
///
/// `map-key-char`'s keys are `'a'`, `'Z'` and `'~'` — all below `0x80`,
/// so nothing here says whether a `char` key is compared signed or
/// unsigned. An IDL `char` is a C++ `char`, whose signedness is
/// implementation-defined and which is **signed** on x86-64 Linux, where
/// the corpus was recorded. That is what is written, in all three
/// columns and explicitly rather than by following each column's
/// compiler, so that the answer is the same on a host whose `char` is
/// unsigned. The same class as [`WstringUnitOrder::HostUnswapped`] and
/// recorded for the same reason.
///
/// `map-key-enum` says nothing either: `wireprobe-fastdds` records it as
/// `refused — fastddsgen failed` in every slot, because its key is a
/// `@bit_bound(16)` enumeration. An enumeration key is ordered by its
/// **declared value** here — `enum class Colour : int32_t` under
/// `std::less` is the underlying integer — and no recorded octets reach
/// that arm.
///
/// # Two dialects select the second value
///
/// [`Dialect::FastDds`] and [`Dialect::OpenDds`]. Fast DDS was the only
/// implementation in the corpus that produced `map` octets when this
/// axis was built; the Cyclone and RTI probes still refuse a `map`,
/// because the IDL-to-C mapping §7.5.1 has no map type, and OpenDDS's
/// column joined on 2026-08-24. Both write ascending keys —
/// `map-key-long` and `map-key-ulonglong` under OpenDDS reorder the
/// sample's declared entries in all four encodings — so judgement 3's
/// threshold of two implementations is met on the *position*.
///
/// It stays a compatibility setting all the same, and that is what
/// judgement 3's addendum in `docs/wire-axes-followup.md` says: the
/// order is a property of the ordered container each code generator
/// chose and not of a serialisation rule either product could have
/// written differently. §7.4.3.5.3 rules (14) to (16) give a map as a
/// length and a run of pairs and say nothing about their order.
/// `--map-entries ascending-key` reaches it, and so does the
/// configuration of that name.
///
/// # It is not a planner setting, and not a reader setting
///
/// A permutation changes no length and moves no offset, so it stays out
/// of [`LayoutKey::offset_key`] for the same reason [`UnionLabelMatch`]
/// does. And a reader is unaffected: `decode::read_map` and both other
/// columns sort a map's entries into the canonical order before they
/// hand it back, because a map is a mapping and not a list. All 32 rows
/// decode to the case's own sample under either value — they are the one
/// cause in this corpus where a reader sees no difference and the wire
/// decision is real all the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MapEntryOrder {
    /// The order the sample declares its entries in.
    ///
    /// The default, and what this project has always written: the clause
    /// gives no order, so the one the caller handed over is kept.
    #[default]
    Declaration,
    /// Ascending by key, under the key type's own value order.
    ///
    /// What a binding that holds a `map` in an ordered container writes.
    /// Named for the behaviour and not for the vendor, so that a reader
    /// does not take it for a reading of §7.4.3.5.3.
    AscendingKey,
}

/// Whether an array's element is flattened through aliases and inner
/// arrays before the delimiter question ([`crate::array_is_delimited`]).
///
/// XTypes models a multi-dimensional array as one ARRAY_TYPE over the
/// base element, so the specification's reading — and Cyclone DDS's and
/// Fast DDS's, both measured byte-identical on
/// `resolve-array-of-typedef-array` — sees `typedef long Row[3];
/// Row grid[2];` as an array of `long` and writes no DHEADER. OpenDDS
/// stops at the alias: its generated serialiser treats the `Row`
/// element as a constructed type and writes the inner DHEADER —
/// measured 2026-08-24, 32 octets against 28 for the same values under
/// XCDR2. Orthogonal to [`CollectionDelimiterPolicy`]: OpenDDS and
/// Cyclone share `Specification` there and part here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum ArrayElementFlattening {
    /// Through aliases and inner arrays to the base element — the
    /// specification's reading.
    #[default]
    ThroughAliasesAndArrays,
    /// The element is taken as declared: an alias of an array is a
    /// constructed type and the array is delimited. OpenDDS.
    StopsAtAlias,
}

impl ArrayElementFlattening {
    /// Both readings.
    pub const ALL: [Self; 2] = [Self::ThroughAliasesAndArrays, Self::StopsAtAlias];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::ThroughAliasesAndArrays)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ThroughAliasesAndArrays => "through-aliases-and-arrays",
            Self::StopsAtAlias => "stops-at-alias",
        }
    }

    /// The named reading, or `None` for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "through-aliases-and-arrays" => Some(Self::ThroughAliasesAndArrays),
            "stops-at-alias" => Some(Self::StopsAtAlias),
            _ => None,
        }
    }
}

/// Which integer an enumerator serialises as.
///
/// XTypes 1.3 §7.4.3.5.3 rule (5) is
/// `XCDR << {O : ENUM_TYPE} = XCDR << { O.value : O.holder_type }`, and
/// IDL 4.2 §7.4.13 gives `O.value` as the `@value(n)` the enumerator
/// declares, else the previous enumerator's value plus one, starting at
/// zero. So the clause has an answer, and it is [`Self::Declared`].
///
/// # What is measured
///
/// Two front ends drop `@value` and number the enumerators by position,
/// and 40 recorded rows differ by exactly that. `enum-negative-value`,
/// `enum-sparse-values`, `enum-bitbound-{8,16,32}-value` and their
/// unannotated twins, OpenDDS, both encoding versions and both byte
/// orders — the same length in every row, and the first differing octet
/// is octet 0:
///
/// ```text
/// enum-negative-value    @value(-1) MINUS, @value(0) ZERO, @value(2147483647) MAX
///   ours    ffffffff 7fffffff
///   theirs  00000000 00000002
///
/// enum-sparse-values     @value(1) OK, @value(100) WARN, @value(65535) FAIL
///   ours    0000ffff 00000001 00000064
///   theirs  00000002 00000000 00000001
///
/// enum-bitbound-8-value  @value(3) RED, @value(9) GREEN, @value(200) BLUE
///   ours    000000c8 00000009 ab
///   theirs  00000002 00000001 ab
/// ```
///
/// `opendds_idl` answers `error in lookup of symbol: bit_bound` and the
/// same for `value`; `tao_idl` drops the annotation the same way —
/// `docs/wire-feature-inventory.md` row E1, and
/// `warnings.rs::CLASSIFICATION` already carries the warning with
/// `carried_into: "E1"`. Cyclone, RTI and Fast DDS all carry `@value`.
/// Two front ends dropping the annotation and three carrying it is the
/// same shape as [`EnumHolderWidth`], and it takes the same answer: not
/// an axis by judgement 3, and a **setting** all the same, because a
/// peer that writes the ordinal writes the ordinal whatever the reason
/// and a reader of the declared value is reading a different
/// enumerator.
///
/// # It is not the holder's width, and the corpus separates them
///
/// The two questions look like one on `enum-bitbound-8-value`, where
/// OpenDDS's octets differ from this project's in both. They are not:
/// `enum-negative-value` declares no `@bit_bound` at all and still
/// differs, and `enum-bitbound-8-plain` declares `@bit_bound(8)` with no
/// `@value` and is closed by [`EnumHolderWidth`] alone. The 16 rows of
/// `enum-bitbound-{8,16}-value` need **both** — the four-octet holder
/// and the ordinal — which is why `EnumHolderWidth` closed 76 rows and
/// not the 92 an estimate gave, and why those 16 were the difference
/// between the causes named for OpenDDS and the rows measured open.
///
/// # It is not a planner setting
///
/// The holder's width does not change, so no offset moves and no
/// EMHEADER length code changes. It stays out of [`LayoutKey::offset_key`]
/// for the same reason [`MapEntryOrder`] and [`UnionLabelMatch`] do.
///
/// # It is a reader setting
///
/// Unlike [`MapEntryOrder`]. All 40 rows reach a reader: a peer's
/// `00000002` is `BLUE`'s ordinal and this project's `MAX`'s declared
/// value, and a decoder that took the pattern for a declared value would
/// name a different enumerator or none at all.
///
/// # No dialect selects the second value
///
/// For the reason [`EnumHolderWidth::BitBoundDroppedByFrontEnd`] is not
/// selected either: OpenDDS and TAO are recorded peers here rather than
/// presets, and a front-end defect is not a reading of §7.4.3.5.3.
/// `--enumerator-value ordinal` reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum EnumeratorValue {
    /// The value IDL 4.2 §7.4.13 gives: `@value(n)`, else the previous
    /// enumerator's value plus one, from zero.
    ///
    /// The default, and what Cyclone, RTI and Fast DDS write.
    #[default]
    Declared,
    /// The enumerator's position in its declaration, from zero,
    /// whatever `@value` says.
    ///
    /// What a front end that does not know the annotation writes.
    /// Identical to [`Self::Declared`] for an enumeration that declares
    /// no `@value` — which is every enumeration in the corpus but five —
    /// so the setting is invisible except where the annotation is.
    Ordinal,
}

impl EnumeratorValue {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Declared, Self::Ordinal];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Declared)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Ordinal => "ordinal",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "declared" => Some(Self::Declared),
            "ordinal" => Some(Self::Ordinal),
            _ => None,
        }
    }
}

/// How a `@mutable` union numbers its branches, and whether its XCDR1
/// parameter list is terminated.
///
/// # The clause
///
/// §7.3.1.2.1.1 numbers the members of an aggregated type sequentially
/// from zero, and a union's discriminator is one of its members
/// (§7.2.2.4.4.4). So the discriminator takes 0 and the branches run on
/// from 1 — the only numbering under which the discriminator cannot
/// collide with an implicitly numbered branch. Rule (28) of §7.4.3.5.3
/// then closes an XCDR1 `MUNION_TYPE` with `PID_SENTINEL` and a length
/// of zero, in the same line as it does for a struct.
///
/// # What is measured
///
/// One implementation departs from both, and in the same generated
/// code. `union-mutable`
/// (`@mutable union Choice switch (long) { case 1: long whole; default:
/// double fraction; }`, `_d = 1`), XCDR2 big endian:
///
/// ```text
/// rti      00000010 20000000 00000001 20000001 ffed2979
/// opendds  00000010 20000000 00000001 20000000 ffed2979
/// ```
///
/// and the same union under XCDR1, where the sentinel goes too:
///
/// ```text
/// ours     00000004 00000001 00010004 ffed2979 7f020000
/// opendds  c0000004 00000001 c0000004 ffed2979
/// ```
///
/// The branch takes member id **0**, which the discriminator already
/// has, and the list ends without `PID_LIST_END`. In
/// `evolution-union-unselected` the union is nested inside a struct and
/// what follows the missing sentinel is the enclosing `long tail`: a
/// reader walking the parameter list takes `0000002a` for another
/// parameter header. RTI and Fast DDS both number the branch 1 and both
/// write the sentinel, so two implementations are with the clause and
/// one against it.
///
/// # One axis and not two, and the corpus says why
///
/// The two halves never occur apart in anything measured, so a second
/// axis would have a value no recorded row reaches. They are still told
/// apart by the rows: the four XCDR2 rows carry the **numbering** alone,
/// because version 2 has no sentinel to suppress, and the four XCDR1
/// rows carry both. A mutation of either half is red on one of the two
/// groups, which is what a merged axis has to be able to say before it
/// is allowed to merge.
///
/// # It is a planner setting
///
/// Both halves reach `TypeLayout` — the branch's `member_id` and
/// `needs_sentinel` — so it is in [`LayoutKey::offset_key`]. A cache
/// that left it out would serve a plan numbered from one to a caller
/// that asked for zero.
///
/// # No dialect selects the second value
///
/// It reproduces a defect: a parameter list with two parameters sharing
/// member id 0 and no terminator is not a frame a conforming reader can
/// walk. `--union-framing branches-from-zero` reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MutableUnionFraming {
    /// The discriminator takes member id 0, the branches run on from 1,
    /// and the XCDR1 parameter list closes with `PID_SENTINEL`.
    ///
    /// The default, the clause, and what RTI Connext 7.7 and Fast DDS
    /// both write.
    #[default]
    Specification,
    /// The branches are numbered from 0, sharing member id 0 with the
    /// discriminator, and the XCDR1 parameter list closes without a
    /// sentinel.
    ///
    /// What OpenDDS writes. Select it to reproduce an OpenDDS peer's
    /// `@mutable` union; it produces a frame no conforming reader can
    /// walk, which is why no dialect selects it.
    BranchesFromZero,
}

impl MutableUnionFraming {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Specification, Self::BranchesFromZero];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Specification)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Specification => "specification",
            Self::BranchesFromZero => "branches-from-zero",
        }
    }

    /// The member id the first branch of a `@mutable` union takes.
    #[must_use]
    pub const fn first_branch_id(self) -> u32 {
        match self {
            Self::Specification => 1,
            Self::BranchesFromZero => 0,
        }
    }

    /// Whether a `@mutable` union's XCDR1 parameter list is terminated.
    #[must_use]
    pub const fn writes_the_sentinel(self) -> bool {
        matches!(self, Self::Specification)
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "specification" => Some(Self::Specification),
            "branches-from-zero" => Some(Self::BranchesFromZero),
            _ => None,
        }
    }
}

/// Which member ids fit a PL_CDR1 **short** parameter header.
///
/// §7.4.1.2.2, verbatim: "Member IDs from 0 to 16,128 (0x3F00)
/// inclusive shall be represented exactly in the lower 14 bits of the
/// parameter ID. All other member IDs must be expressed using the
/// extended parameter header format." The range stops one below
/// `PID_EXTENDED` (`0x3F01`) because `0x3F01` upwards is the reserved
/// range that makes `PID_EXTENDED` and `PID_LIST_END` unambiguous.
///
/// # What is measured
///
/// `annotation-id-sparse` declares `@id(16383)`, which is `0x3FFF` — in
/// the reserved range and out of the short form's. XCDR1 big endian:
///
/// ```text
/// ours     3f010008 00003fff 00000004 fffffffe   (12 octets of header)
/// opendds  ffff0004 fffffffe                     (4)
/// ```
///
/// `0xFFFF` masked to fourteen bits is `0x3FFF`, so OpenDDS uses the
/// whole low fourteen bits and lets the reserved range be a reserved
/// range only for the two ids that are actually parameters. Cyclone,
/// RTI and Fast DDS all write the extended header there — measured on
/// the same case — so this project has the clause and two other
/// implementations with it.
///
/// # It is a planner setting
///
/// A short header is four octets and an extended one twelve, so the
/// choice moves every member behind it. It is in
/// [`LayoutKey::offset_key`] for that reason.
///
/// # No dialect selects the second value
///
/// It writes an id in a range the clause reserves, which a reader that
/// implements §7.4.1.2.2 will take for `PID_IGNORE` or worse.
/// `--short-pid-range all-fourteen-bits` reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum ShortPidRange {
    /// `0` to `0x3F00` inclusive, and the extended header above it.
    ///
    /// The default, the clause, and what Cyclone, RTI and Fast DDS all
    /// write.
    #[default]
    UpToReserved,
    /// The whole low fourteen bits, `0` to `0x3FFF`, reserved ids
    /// included.
    ///
    /// What OpenDDS writes.
    AllFourteenBits,
}

impl ShortPidRange {
    /// Both.
    pub const ALL: [Self; 2] = [Self::UpToReserved, Self::AllFourteenBits];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::UpToReserved)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UpToReserved => "up-to-reserved",
            Self::AllFourteenBits => "all-fourteen-bits",
        }
    }

    /// The highest member id the short header carries.
    ///
    /// `u16` and not a wider integer, because the short header's id
    /// field is fourteen bits and every value this can answer fits one.
    /// It was `u32`, and the one caller that needs the wire's own width
    /// — `PlCdr1Framing::highest_short_id` — narrowed it with
    /// `u16::try_from(..).unwrap_or(u16::MAX)`. That fallback is a
    /// silent clamp on the threshold a reader uses to decide that a
    /// parameter is reserved: `Reader::pl_cdr1` reports every id above
    /// it as reserved and drops the member, so a variant added here
    /// with a value above `0xffff` would have moved the threshold to
    /// `0xffff` and dropped members without saying so. Answering in the
    /// width the field has deletes the conversion instead of guarding
    /// it.
    #[must_use]
    pub const fn highest_short_id(self) -> u16 {
        match self {
            Self::UpToReserved => 0x3f00,
            Self::AllFourteenBits => 0x3fff,
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "up-to-reserved" => Some(Self::UpToReserved),
            "all-fourteen-bits" => Some(Self::AllFourteenBits),
            _ => None,
        }
    }
}

impl MapEntryOrder {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Declaration, Self::AscendingKey];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Declaration)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::AscendingKey => "ascending-key",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "declaration" => Some(Self::Declaration),
            "ascending-key" => Some(Self::AscendingKey),
            _ => None,
        }
    }
}

/// Whether a `fixed<P,S>`'s digit count comes from the declaration or
/// from the value.
///
/// CORBA 3.3 part 2 §9.3.2.8 defines the wire form and nothing else:
/// packed binary-coded decimal, two digits to the octet, most
/// significant first, the low nibble of the last octet the sign — `0xC`
/// for positive or zero and `0xD` for negative — with a leading zero
/// nibble prepended when the nibble count is odd. IDL 4.2
/// §7.4.1.4.4.3.4 rule (42) gives the syntax and caps `P` at 31. XTypes
/// 1.3 has no fixed-point type at all, which is why the rule is borrowed
/// by name. **What neither clause says is how many digits go on the
/// wire**, and the four implementations that write one split two and
/// two.
///
/// # Two on each side, which is what makes it an axis
///
/// | | the digit count | measured on |
/// |---|---|---|
/// | omniORB 4.3.3 | the **declared** `P` | `fixed-basic`, `fixed-scales`, `fixed-scale-equals-digits`, `array-1d-fixed`, `sequence-fixed`, both orders |
/// | this project | the declared `P` | the same rows, byte for byte with omniORB |
/// | JacORB | the **value's** precision | the same rows; `docs/corba-column.md` §3 traced it to `BigDecimal.precision()` |
/// | ACE/TAO 2.5.24 | the digits of the string the value was parsed from | `ACE_CDR::Fixed`'s own CDR operator; `tao_idl` refuses the corpus's `fixed` IDL so TAO has no recorded row, and its runtime was measured directly in `unspecified_constructs.md` §3 |
/// | OpenDDS 3.34 | the declared `P` | its serializer writes `OpenDDS::FaceTypes::Fixed_T<P,S>`, `(P + 2) / 2` octets, not through `ACE_CDR::Fixed`'s operator: `0.00` under `fixed<5,2>` is `00000c`, `7` is `00700c` (measured 2026-10-07 through the FACE mapping) |
///
/// Judgement 3's threshold, met: two independent implementations on each
/// side, and both were in a position to answer differently — a front end
/// that carried `fixed<9,3>` as far as a serialiser and then wrote the
/// value's digits chose to.
///
/// # The octets
///
/// `fixed-basic`, `fixed<9,3>`, `amount = 123456.789` and
/// `debt = -000000.001`, `plain-cdr1` big endian:
///
/// ```text
/// ours    123456789c 00000000 1d ab
/// theirs  123456789c       1d ab
/// ```
///
/// Only a value with leading zeros differs. `fixed-scales` and
/// `fixed-scale-equals-digits` are byte-identical across omniORB, JacORB
/// and this project, which is what makes the cause exact rather than
/// inferred: `fixed<31,10>` holding thirty significant digits pads to
/// sixteen octets under **both** readings, and `fixed<4,4>` holding four
/// pads to three under both.
///
/// # The value's precision is `BigDecimal.precision()`
///
/// The digit string with its leading zeros removed, and **at least one
/// digit**: `new BigDecimal("0.00").precision()` is 1 in Java, so a zero
/// goes out as the single nibble `0` and the sign, one octet `0c`. That
/// is the same "at least one" the C++ and Rust sides need, and it is why
/// the rule is stated as `max(significant, 1)` rather than as "strip the
/// leading zeros".
///
/// # It is decodable, and the estimate said it was not
///
/// `docs/open-rows-breakdown.md` cause K states that "the declared count
/// is the only decodable half: a reader must know the octet count from
/// the type before it reads the value". That is wrong, and the clause
/// says why: a digit nibble is `0` to `9` and a sign nibble is `0xA` to
/// `0xF`, so the field **ends at the first octet whose low nibble is at
/// least `0xA`**. The count of nibbles is even by construction, so the
/// sign is always a low nibble and never a high one. A reader of this
/// form scans; all three columns do, and
/// `crates/wire-probe/tests/fixed_digit_count.rs::the_value_precision_form_reads_back`
/// is that as a test over the recorded JacORB rows.
///
/// # It **is** a planner setting
///
/// Unlike [`MapEntryOrder`] and [`UnionLabelMatch`]. A `fixed`'s extent
/// is `ceil((P + 1) / 2)` octets under [`Self::Declared`] and depends on
/// the value under [`Self::ValuePrecision`] and [`Self::ValueStringDigits`],
/// so the type stops being fixed-size and every member behind it moves.
/// It is in [`LayoutKey::offset_key`] for that reason, and
/// `the_extent_is_unbounded_under_the_value` is the claim as a test.
///
/// # Three readings exist
///
/// 1. **Declared** — `P` digits, padded to even nibble count. What
///    omniORB 4.3.3 and this project's default write.
/// 2. **ValuePrecision** — the value's precision with leading and
///    trailing zeros removed, minimum one digit. What JacORB 3.9 writes
///    (`BigDecimal.precision()`).
/// 3. **ValueStringDigits** — the integer digits of the decimal as
///    written, leading zeros included, plus the `S` fraction digits of
///    the declaration, padded to an odd digit count by adding one
///    leading zero if even. What ACE/TAO 2.5.24's `ACE_CDR::Fixed`
///    writes for a decimal with `S` fraction digits — it counts the
///    digits of the string it was parsed from — which is what OpenDDS's
///    generated `write_fixed` calls. The fraction is counted at `S`
///    and not as written: `"7"` under `fixed<5,2>` is `700c`, and
///    counting one digit would have kept only its last nibble, `0c`.
///
/// The three differ on values with fewer digits than the type declares:
///
/// | declared | value string | digit count | Declared | ValuePrecision | ValueStringDigits |
/// |---|---|---|---|---|---|
/// | `fixed<5,5>` | `"0.12345"` | 6 (even) | 5 | 5 | **7** (`0012345`) |
/// | `fixed<5,2>` | `"0.00"` | 3 (odd) | 5 | 1 | **3** (`000`) |
/// | `fixed<4,2>` | `"12.34"` | 4 (even) | 4 | 4 | **5** (`01234`) |
/// | `fixed<5,2>` | `"123.45"` | 5 (odd) | 5 | 5 | **5** (`12345`) |
/// | `fixed<1,0>` | `"7"` | 1 (odd) | 1 | 1 | **1** (`7`) |
/// | `fixed<5,2>` | `"7"` | 1 + 2 (odd) | 5 | 3 | **3** (`700`) |
///
/// The `ValueStringDigits` column is pinned by the recorded ACE values
/// in `unspecified_constructs.md` §3:
/// - `fixed<5,5>` `"0.12345"` → `0012345c` (7 digits) pins the even→odd padding.
/// - `fixed<5,2>` `"0.00"` → `000c` (3 digits) pins the odd count without padding.
/// - `fixed<4,2>` `"12.34"` → `01234c` (5 digits) pins the even→odd padding.
/// - `fixed<5,2>` `"123.45"` → `12345c` (5 digits) pins the odd count without padding.
/// - `fixed<1,0>` `"7"` → `7c` (1 digit) pins the minimum one digit.
/// - The `fixed<31,0>` row (31 nines) agrees across all three and does not
///   separate them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum FixedDigitCount {
    /// `P` digits, whatever the value is — the declaration decides.
    ///
    /// The default, and what omniORB 4.3.3 writes.
    #[default]
    Declared,
    /// As many digits as the value has, and never fewer than one.
    ///
    /// What JacORB 3.9 writes. Named for the rule and not for a
    /// vendor, because two implementations reached it independently.
    ValuePrecision,
    /// The integer digits of the decimal as written, leading zeros
    /// included, plus the `S` fraction digits, padded to an odd digit
    /// count by adding one leading zero if the count is even.
    ///
    /// What ACE/TAO 2.5.24's `ACE_CDR::Fixed` writes for a decimal with
    /// `S` fraction digits, through its own CDR operator. Measured in
    /// `unspecified_constructs.md` §3. OpenDDS does not write it: its
    /// serializer writes the declared count (see [`Dialect::fixed_digits`]).
    ValueStringDigits,
}

impl FixedDigitCount {
    /// All three.
    pub const ALL: [Self; 3] = [
        Self::Declared,
        Self::ValuePrecision,
        Self::ValueStringDigits,
    ];

    /// The lowest nibble value that is a sign and not a digit.
    ///
    /// CORBA 3.3 part 2 §9.3.2.8: a digit is `0` to `9`, and every
    /// nibble at or above this is a sign. It is what makes the
    /// value-precision form self-delimiting, and it is a constant here
    /// so the three runtimes cannot each pick their own.
    pub const FIRST_SIGN_NIBBLE: u8 = 0x0a;

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Declared)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::ValuePrecision => "value-precision",
            Self::ValueStringDigits => "value-string-digits",
        }
    }

    /// How many digit nibbles a value goes out with.
    ///
    /// `declared` is `P`; `significant` is the value's digit count with
    /// its leading zeros removed. The one place the two cross into a
    /// wire width. [`crate::fixed_wire_digits`] is what call sites use;
    /// this is the arithmetic behind it, separated so that the crossing
    /// can be tested without a value.
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

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "declared" => Some(Self::Declared),
            "value-precision" => Some(Self::ValuePrecision),
            "value-string-digits" => Some(Self::ValueStringDigits),
            _ => None,
        }
    }
}

/// How a `map` key's values are compared when
/// [`MapEntryOrder::AscendingKey`] puts the entries in order.
///
/// The answer [`crate::map_key_ordering`] gives, and the one thing the
/// three language columns have to agree on beyond "sorted": a signed and
/// an unsigned reading of the same octets give different orders, and the
/// corpus separates them — see [`MapEntryOrder`].
///
/// It is deliberately **not** "compare the encoded octets". A negative
/// `long` key encodes as `ffed2979`, which sorts last among the octets
/// and first among the values, and Fast DDS writes it first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MapKeyOrdering {
    /// By signed integer value — `int8`, `short`, `long`, `long long`,
    /// and an IDL `char`, whose C++ counterpart is signed on the host
    /// the corpus was recorded on.
    SignedInteger,
    /// By unsigned integer value — `octet`, `uint8`, `unsigned short`,
    /// `unsigned long`, `unsigned long long`, `boolean` (`false` before
    /// `true`) and `wchar` (by code point).
    UnsignedInteger,
    /// By the enumerator's declared value, signed.
    ///
    /// `enum class E : int32_t` under `std::less` is the underlying
    /// integer, and `@value` gives it. Unmeasured against recorded
    /// octets — see [`MapEntryOrder`].
    EnumeratorValue,
    /// By the text's code units, compared unsigned, shorter first on a
    /// prefix.
    ///
    /// For a `string` this is `std::string::operator<`, which is
    /// `char_traits<char>::compare` and which C++ defines over
    /// `unsigned char` whatever `char`'s own signedness is.
    /// `map-key-string` measures it: `["", "alpha", "üሴ"]` stays as
    /// declared, and a signed reading of `üሴ`'s leading `c3` would put
    /// that entry first.
    ///
    /// For a `wstring` this is `std::wstring::operator<`, which compares
    /// code points. **One variant and not two**, because the two orders
    /// are the same order: every column here holds a `wstring` as UTF-8
    /// — `zd_wstring` is a `const char *`, the Python column holds a
    /// `str` and the Rust probe a `String` — and UTF-8 is order
    /// preserving, so the unsigned octet order of a UTF-8 encoding is
    /// the code-point order of what it encodes. Splitting the variant
    /// would give a coordinate no value could separate, which is a
    /// distinction a mutation cannot make red. No corpus case declares a
    /// `wstring` map key.
    CodeUnits,
}

impl MapKeyOrdering {
    /// Every ordering.
    pub const ALL: [Self; 4] = [
        Self::SignedInteger,
        Self::UnsignedInteger,
        Self::EnumeratorValue,
        Self::CodeUnits,
    ];

    /// Short name, for diagnostics and for the emitters that name it in
    /// generated code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignedInteger => "signed-integer",
            Self::UnsignedInteger => "unsigned-integer",
            Self::EnumeratorValue => "enumerator-value",
            Self::CodeUnits => "code-units",
        }
    }
}

/// Whether a key holder keeps the type's extensibility or is forced to
/// `@final`.
///
/// DDSI-RTPS 2.5 §9.6.4.8 Step 1 and XTypes 1.3 §7.6.8 Step 1 both
/// build `FooKeyHolder` from `Foo` and then say it "shall have
/// extensibility kind 'FINAL'". A holder that kept the original's kind
/// carries the original's framing: a DHEADER for an appendable one,
/// PID headers and a sentinel for a mutable one under XCDR1 — and that
/// pushes the holder over sixteen bytes, which sends Step 5 down the
/// MD5 branch. Two peers reading the clause differently compute
/// different instance handles for one instance.
///
/// # One implementation departs, in one place
///
/// Measured with `@final struct Foo { @key long id; long x; long y; }`
/// and its `@appendable` and `@mutable` variants, `id = 0x12345678`:
///
/// | | XCDR1 | XCDR2 |
/// |---|---|---|
/// | Cyclone, all three kinds | holder `12345678`, hash `12345678` + zeros | the same |
/// | Fast DDS, all three | the same | the same |
/// | RTI, `@final` / `@appendable` | the same | the same |
/// | RTI, `@mutable` | holder `00000004 12345678 7f020000`, hash `0482916deda7e1d2459197a74346627f` | `12345678` + zeros |
///
/// Cyclone's key buffer still carries the framing and
/// `dds_stream_extract_keyBE_from_key(…, DDS_CDR_KEY_SERIALIZATION_KEYHASH,
/// …)` strips it. Fast DDS hard-codes `PLAIN_CDR2` in the generated
/// holder whatever the type says. RTI keeps the parameter list under
/// XCDR1 only: `maxSizeSerializedKey V1 = 28`, past sixteen, so the MD5
/// branch is taken over the framing rather than over the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum KeyHolderExtensibility {
    /// `@final`, whatever the type declares. The clause, and what four
    /// of the five measured combinations do.
    #[default]
    ForcedFinal,
    /// The type's own kind, framing and all.
    ///
    /// RTI Connext 7.7 under XCDR1. Select it to compute the instance
    /// handle an RTI peer computes for a `@mutable` keyed type there;
    /// under XCDR2 RTI forces final like everyone else, which is why
    /// [`Dialect::key_holder_extensibility`] takes the encoding.
    AsDeclared,
}

impl KeyHolderExtensibility {
    /// Both.
    pub const ALL: [Self; 2] = [Self::ForcedFinal, Self::AsDeclared];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::ForcedFinal)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ForcedFinal => "forced-final",
            Self::AsDeclared => "as-declared",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo here changes every instance handle.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "forced-final" => Some(Self::ForcedFinal),
            "as-declared" => Some(Self::AsDeclared),
            _ => None,
        }
    }
}

/// How far Step 1's member removal reaches into a key holder.
///
/// XTypes 1.3 §7.6.8 Step 1 keeps the key members of the holder —
///
/// > If there are any key members, then remove the non-key members from
/// > FooKeyHolder. Otherwise do not remove any members.
///
/// — and Step 3 says to "apply steps 1 and 2 recursively to the members
/// of FooKeyHolder if they are themselves AggregatedTypes". `FooKeyHolder`
/// at that point is whatever Step 1 left, so for a keyless aggregate it
/// is every member, and Step 3 applies to all of them. The recursion has
/// no depth limit written into it.
///
/// # What the vendors do
///
/// Measured on 2026-08-09 through `keyhash-probe/` — Cyclone DDS
/// 11.0.1, RTI Connext 7.7.0, Fast DDS from source. Three shapes:
///
/// ```idl
/// @final struct NestedNoKey        { long a; long b; };
/// @final struct OuterKeylessNested { @key NestedNoKey inner; long tail; };
///
/// @final struct DeepLeaf  { @key long p; long q; };
/// @final struct DeepMid   { DeepLeaf leaf; long r; };   // no key of its own
/// @final struct DeepOuter { @key DeepMid mid; long tail; };
///
/// @final struct ChainLeaf  { @key long p; long q; };
/// @final struct ChainMid   { @key ChainLeaf inner; long r; };
/// @final struct ChainOuter { @key ChainMid mid; long tail; };
/// ```
///
/// All three vendors, under every encoding, give `OuterKeylessNested`
/// both of `NestedNoKey`'s members and `ChainOuter` only `p` — so the
/// "otherwise do not remove any members" clause holds, and the recursion
/// through *keyed* levels is unlimited everywhere.
///
/// They part over `DeepOuter`, where a keyless level sits above a keyed
/// one:
///
/// | | `DeepOuter` holder |
/// |---|---|
/// | Cyclone, XCDR1 and XCDR2 | `0d0d0d0d 0f0f0f0f` — `p` and `r` |
/// | Fast DDS | the same |
/// | RTI, XCDR2 | the same |
/// | RTI, XCDR1 | `0d0d0d0d 0e0e0e0e 0f0f0f0f` — `p`, `q` and `r` |
///
/// RTI's XCDR1 holder stops filtering at the first keyless aggregate and
/// takes what is under it whole: `maxSizeSerializedKey V1 / V2` reports
/// `12 / 8` for that type, so the two holders are genuinely different
/// types and not one type serialized twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum KeyHolderNesting {
    /// Step 1 applies at every level, however deep. The clause, and what
    /// four of the five measured combinations do.
    #[default]
    Recursive,
    /// The first keyless aggregate is taken whole; nothing below it is
    /// filtered.
    ///
    /// RTI Connext 7.7 under XCDR1. Under XCDR2 RTI recurses like
    /// everyone else, which is why [`Dialect::key_holder_nesting`] takes
    /// the encoding.
    KeylessNestedWhole,
}

impl KeyHolderNesting {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Recursive, Self::KeylessNestedWhole];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Recursive)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recursive => "recursive",
            Self::KeylessNestedWhole => "keyless-nested-whole",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo here changes every instance handle.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "recursive" => Some(Self::Recursive),
            "keyless-nested-whole" => Some(Self::KeylessNestedWhole),
            _ => None,
        }
    }
}

/// In what order the members of a key holder are serialized.
///
/// XTypes 1.3 §7.6.8 orders the holder's members by member id, and
/// DDSI-RTPS 2.5 §9.6.3.8 hashes whatever that produces. Two peers that
/// disagree about the order compute two different KeyHashes for one
/// sample, which is the same instance arriving under two handles — a
/// wrong value with nothing on the wire to say so.
///
/// # What the implementations do
///
/// Measured 2026-08-10 through `n3-keyholder/` on the Linux test
/// host — Cyclone DDS 11.0.1 via `dds_stream_extract_keyBE_from_key(…,
/// DDS_CDR_KEY_SERIALIZATION_KEYHASH, …)`, RTI Connext 7.7.0 via
/// `PRESTypePlugin_interpretedSerializeKeyForKeyhash` under all five
/// encapsulations, Fast DDS via the generated
/// `KeyOrderPubSubType::compute_key`. Two types, each declared in the
/// reverse of its id order, so the KeyHash says which order won:
///
/// ```idl
/// @final struct KeyOrder { @id(5) @key long  b; @id(2) @key long a; };  // b = 2, a = 1
/// @final struct KeyMixed { @id(5) @key octet b; @id(2) @key long a; };  // b = 2, a = 1
/// ```
///
/// | | `KeyOrder` KeyHash | `KeyMixed` KeyHash |
/// |---|---|---|
/// | Cyclone, XCDR1 and XCDR2 | `00000001 00000002` | `00000001 02` |
/// | RTI, XCDR2 (`CDR2_BE`, `D_CDR2_BE`, `PL_CDR2_BE`) | `00000001 00000002` | `00000001 02` |
/// | Fast DDS, both | `00000001 00000002` | `00000001 02` |
/// | RTI, XCDR1 (`CDR_BE`, `PL_CDR_BE`) | `00000002 00000001` | `02000000 00000001` |
///
/// — all sixteen bytes wide, zero-extended. So the clause's order is
/// what three of the four combinations write, and RTI under encoding
/// version 1 is the one that keeps declaration order: the same version
/// split [`KeyHolderExtensibility`] and [`KeyHolderNesting`] already
/// carry for RTI, on a third axis.
///
/// `KeyMixed` is the case that shows it is an ordering and not a
/// coincidence of equal-width members: RTI's XCDR1 holder is eight
/// octets, with the `octet` first and three of padding behind it, and
/// everyone else's is five with no padding at all.
///
/// Cyclone's *serdata* key buffer — `dds_stream_extract_key_from_data`,
/// which is not the KeyHash — is in declaration order in both versions.
/// It is an intermediate: `dds_stream_extract_keyBE_from_key` reorders
/// it on the way to the hash, and reading the intermediate as the answer
/// gives Cyclone a divergence it does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum KeyHolderOrder {
    /// Ascending effective member id. The clause, and what three of the
    /// four measured combinations write.
    #[default]
    AscendingMemberId,
    /// The order the members are declared in, which is the order the
    /// sample carries them in.
    ///
    /// RTI Connext 7.7 under XCDR1. Under XCDR2 RTI orders by member id
    /// like everyone else, which is why [`Dialect::key_holder_order`]
    /// takes the encoding.
    Declaration,
}

impl KeyHolderOrder {
    /// Both.
    pub const ALL: [Self; 2] = [Self::AscendingMemberId, Self::Declaration];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::AscendingMemberId)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AscendingMemberId => "ascending-member-id",
            Self::Declaration => "declaration",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo here changes every instance handle.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ascending-member-id" => Some(Self::AscendingMemberId),
            "declaration" => Some(Self::Declaration),
            _ => None,
        }
    }
}

/// How the members of a key holder are aligned, which decides the KeyHash
/// of every type with keys of different widths.
///
/// DDS-XTypes 1.3 §7.6.8 step 4 serialises the holder as PLAIN_CDR2
/// big-endian, so a member is aligned to its size capped at four, as in any
/// XCDR2 body. Measured on the Linux test host with `tools/keyhash-probe` (types
/// `PadKey`, `WideKey`, `CharKey`, `StrKey`, `PathKey`):
///
/// - Fast DDS 3.6.1, and Cyclone 11.0.1 and RTI Connext 7.7.0 under XCDR2,
///   align as the clause says.
/// - Cyclone and RTI under XCDR1 align to the XCDR1 `MAXALIGN` of eight:
///   `@key short s; @key long long ll` hashes as `0102 000000000000
///   0304…` where the clause gives `0102 0000 0304…`. RTI sends this in
///   `PID_KEY_HASH` (plain and PL_CDR); Cyclone sends no `PID_KEY_HASH`.
/// - OpenSplice 6.9 writes the key fields back to back, measured on the
///   wire: `@key octet a; @key long b` with `a=0x11, b=0x22334455` goes out
///   as `11223344 55000000 …` where the clause gives `11000000 22334455 …`,
///   and a holder over sixteen octets is hashed without the padding too.
///   Its DDSI2 source says so — `KEYHASH_HAS_PADDING 0`, "my current guess"
///   (`src/services/ddsi2e/code/q_osplser.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum KeyHolderAlignment {
    /// Each member aligned to its size, capped at PLAIN_CDR2's four. The
    /// clause.
    #[default]
    Aligned,
    /// Each member aligned to its size, capped at the `MAXALIGN` of the
    /// encoding the data goes out in: eight under XCDR1, four under XCDR2.
    /// Cyclone and RTI under XCDR1.
    DataEncoding,
    /// No padding between members: every member at the next free octet.
    /// OpenSplice 6.9.
    Packed,
}

impl KeyHolderAlignment {
    /// All three.
    pub const ALL: [Self; 3] = [Self::Aligned, Self::DataEncoding, Self::Packed];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Aligned)
    }

    /// The largest alignment a key holder member takes when the data goes
    /// out in `encoding`.
    #[must_use]
    pub const fn max_alignment(self, encoding: Encoding) -> u8 {
        match self {
            Self::Aligned => Encoding::PlainCdr2.max_alignment(),
            Self::DataEncoding => encoding.max_alignment(),
            Self::Packed => 1,
        }
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::DataEncoding => "data-encoding",
            Self::Packed => "packed",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo here changes every instance handle.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "aligned" => Some(Self::Aligned),
            "data-encoding" => Some(Self::DataEncoding),
            "packed" => Some(Self::Packed),
            _ => None,
        }
    }
}

/// What becomes of a union's member when `@try_construct(use_default)`
/// defaulted its discriminator.
///
/// The question arises on one path only: `union U switch
/// (@try_construct(use_default) K)` — §7.3.1.2.1.12.5's own shape — a
/// stream whose discriminator value `K` cannot represent, and Table 19's
/// `UNION_TYPE` row deciding what the reader hands back for the member.
///
/// # The clause
///
/// §7.2.4.4.8, Table 19, the `UNION_TYPE` row:
///
/// > If the discriminator value selects a member m1 in O1, then m1 is
/// > set to its default value according to its type.
///
/// It does not take the writer's member, and §7.2.4.1 says why it
/// cannot: the whole of type consistency holds
///
/// > without any inspection of a data producer's type definition
///
/// and with no producer type there is nothing that says the octets
/// behind the discriminator belong to a member of the type the reader
/// now selects. So [`Self::TypeDefault`] is the clause read straight,
/// and it is the default here.
///
/// # What the implementations do
///
/// | | writer sends `VAL3`, reader's discriminator defaults to `VAL1` |
/// |---|---|
/// | RTI Connext 7.7.0 | `{"x1":0}` — the member defaulted. Measured. |
/// | OpenDDS 3.34.0 | `d=0 v=0` for §7.3.1.2.1.12.5's own example, not the `42` the writer put in `e3_value`. Measured. |
/// | Toc CoreDX 6.14 | keeps the writer's member — read off its exclusion list, **not** measured here. |
/// | eProsima Fast DDS 3.6.1 | cannot express the case: it refuses annotations on a union discriminator outright. |
///
/// RTI reaches its outcome without the annotation at all — `rtiddsgen`
/// reports `try_construct` as ignored, and the shipped Extensible Types
/// guide lists it among the annotations the middleware ignores — which
/// makes it a witness for §7.2.4.1's default behaviour rather than for a
/// reading of the annotation.
///
/// # Why this is an axis and not a verdict
///
/// The OMG `dds-xtypes` interoperability suite expects the other answer.
/// `tryc_union_enum_disc_2` has a writer send `union_disc_enum_1` with
/// discriminator `VAL3` and `x3 = 3`, a reader declare
/// `union_disc_enum_2_default` whose `E2` carries
/// `defaultLiteral="true"` on `VAL1`, and the sample the reader compares
/// against is `data/json/tryconstruct/union_x1.json` — `{"x1":3}`, the
/// writer's member value under the reader's member name. That is
/// [`Self::StreamValue`], and nothing else produces it.
///
/// A suite that disagrees with the implementations is a disagreement
/// like any other, and a disagreement gets an axis here. What it does
/// not get is the default: a reader that took the streamed octets would
/// hand back `{"x1":3}` where RTI and OpenDDS both hand back
/// `{"x1":0}`, so [`Self::StreamValue`] as the default would buy one
/// suite case at the price of two measured peers.
///
/// # Where it applies
///
/// The reflective codec, `zerodds-wire-codec`, which is the only reader
/// in the tree that implements Table 19's member step at all — the
/// emitted language columns carry `@try_construct` on the discriminator
/// itself and stop there, so they have no second answer to give and no
/// axis value to carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum DefaultedDiscriminatorMember {
    /// The member the defaulted discriminator selects takes its own
    /// default value, and the octets the writer's member occupied are
    /// stepped over. Table 19's `UNION_TYPE` row, and **the default**.
    #[default]
    TypeDefault,
    /// The octets the writer's member occupied are read as the member
    /// the defaulted discriminator selects.
    ///
    /// Reading them at all is the claim about the writer's type that
    /// §7.2.4.1 says a reader cannot make, so it is only ever a
    /// deliberate choice. Two consequences follow from it and are worth
    /// naming, because neither is a defect of the implementation:
    ///
    /// - the octets are read as *this* type's case, so where that case
    ///   is wider than the writer's the read runs into whatever
    ///   follows, and where it is narrower the union's own framing is
    ///   what puts the reader back;
    /// - under a `@mutable` union the member the stream framed carries
    ///   the **writer's** member id, so the id is taken as read rather
    ///   than checked. There is no id to check it against: the reader's
    ///   discriminator no longer names the writer's case.
    ///
    /// Two shapes fall back to [`Self::TypeDefault`] rather than
    /// guessing:
    ///
    /// - a defaulted discriminator that selects no case at all — there
    ///   is no member to read the octets into, so the writer's member is
    ///   stepped over;
    /// - an unbounded case in a union that states no extent of its own.
    ///   An undelimited `@final` or `@appendable` union under XCDR1 has
    ///   neither a delimiter nor a sentinel, so nothing catches a read
    ///   that runs long, and a `string`, `sequence` or `map` case would
    ///   take its length prefix out of octets that were never a length.
    ///   A fixed-extent case has no such prefix — it takes what its own
    ///   type takes — and is read; an unbounded one is stepped over, and
    ///   the step-over is the refusal [`Self::TypeDefault`] makes for
    ///   the same shape.
    StreamValue,
}

/// Whether the elements of a collection carry alignment padding.
///
/// XTypes 1.3 §7.4.1 defines the padding: `ALIGN(N)` is
/// `MIN(N, XCDR.version.maxalign)`, and a structure's members each
/// begin on a multiple of their own alignment counted **from the
/// encapsulation**. An element of an array or a sequence is a value like
/// any other, so §7.4.1 pads in front of one exactly as it does in
/// front of a member — which is what makes a `Cell { long a; short b; }`
/// array take a stride of eight rather than of six.
///
/// # The measured departure
///
/// `tests/interop-results/array-1d-struct-final/hdds/plain-cdr1.le.bin`
/// is `01000000 0200 03000000 0400 77` for
/// `@final struct Cell { long a; short b; }` in `Cell items[2];` with
/// `octet tail`: the second element's `long a` starts at offset six,
/// where §7.4.1's `ALIGN(4)` puts it at eight. Every recorded row of
/// `sequence-struct-final` says the same — `many`'s second `Cell` puts
/// `a` at offset 26 — and so does every array and sequence row of that
/// column.
///
/// **Struct members are still padded**, which is why this is about
/// elements and not about alignment in general.
/// `tests/interop-results/struct-final/hdds/plain-cdr1.le.bin` is
/// `79 29 ed ff 00000000 18 2d 44 54 fb 21 09 40 fe ff`: four octets
/// of fill ahead of the `double`, because XCDR1's `maxalign` is eight.
/// `annotation-must-understand-unannotated` pads the same way after an
/// `@optional` flag. So the axis has two values and not one, and
/// [`Self::Packed`] means "no padding **between the elements**".
///
/// # What `Packed` means for the octets inside an element
///
/// A fresh alignment origin at the element's own first octet. Every
/// recorded row that reaches the question agrees with the plainer
/// reading — "no alignment at all inside an element" — because no
/// recorded row has an element whose own members need padding between
/// them: the corpus's collection elements are `long`/`short` pairs,
/// `string`s and `wstring`s, and the first of those is already packed.
/// The origin form is the one that keeps a member's offset a function of
/// its own type, and it is what [`crate::plan()`] and the writer both
/// implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum ElementPadding {
    /// XTypes 1.3 §7.4.1: an element begins at the next multiple of its
    /// alignment, counted from the encapsulation. The default.
    #[default]
    Aligned,
    /// Every element begins at the octet after the previous element
    /// ends, and alignment inside an element is counted from there.
    Packed,
}

impl ElementPadding {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Aligned, Self::Packed];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Aligned)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::Packed => "packed",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "aligned" => Some(Self::Aligned),
            "packed" => Some(Self::Packed),
            _ => None,
        }
    }

    /// Whether a value of `size` octets is followed by padding up to the
    /// next `align`, under this policy.
    ///
    /// One expression for both the planner's array stride and the
    /// writer's origin, so the two cannot answer differently about the
    /// same element.
    #[must_use]
    pub const fn pads_after(self, _size: u32, _align: u8) -> bool {
        matches!(self, Self::Aligned)
    }
}

/// Whether an aggregate's declared extensibility reaches the wire.
///
/// XTypes 1.3 §7.4.3.5.3 rule (2) puts a `DHEADER` in front of an
/// `@appendable` or `@mutable` aggregate under XCDR2 and nowhere under
/// XCDR1, which §7.4.3.4.1 gives no delimiter at all; rules (21) to
/// (25) frame a `@mutable` type's members with an EMHEADER under XCDR2
/// and a parameter header under XCDR1, and rule (30) terminates the
/// parameter list with `PID_LIST_END`. All of that is
/// [`Self::Specification`].
///
/// # The measured departure
///
/// [`Self::Never`] is one product's generated serialiser writing every
/// aggregate as `@final`, and it is measurable at three of the four
/// positions an aggregate can be reached in:
///
/// - **the sample itself.** `tests/interop-results/struct-appendable/hdds/plain-cdr2.le.bin`
///   is `79 29 ed ff 18 2d 44 54 fb 21 09 40 fe ff`, fourteen octets.
///   §7.4.3.5.3 rule (2) adds `0e000000` in front of them.
/// - **a struct member.**
///   `tests/interop-results/struct-appendable-in-final/hdds/plain-cdr2.le.bin`
///   is `79 29 ed ff fe ff 00 00 ff ff ff ff`; the `@appendable Inner`
///   takes no delimiter.
/// - **a `@mutable` type.** `tests/interop-results/annotation-must-understand-unannotated/hdds/plain-cdr1.le.bin`
///   is `79 29 ed ff ff ff ff 7f 01 00 00 00 06 00 00 00 c3 bc 62 65 72 00`:
///   two `long`s, a one-octet `@optional` flag and a `string`, with no
///   parameter header and no `PID_LIST_END` anywhere, which is
///   rule (18) and rule (20) — the `@final` form.
///
/// The **fourth** position answers differently and has its own axis:
/// an element of a `sequence` does carry one, under both encoding
/// versions. That is [`SequenceElementDelimiter`], and this axis
/// deliberately says nothing about it — a reader who took `Never` to
/// mean "no delimiter anywhere" would be right about three positions
/// and wrong about the fourth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum AggregateDelimiter {
    /// XTypes 1.3 §7.4.3.5.3 rules (2) and (21) to (25): an
    /// `@appendable` or `@mutable` aggregate carries a `DHEADER` under
    /// XCDR2 and a `@mutable` type's members carry their headers under
    /// both versions. The default.
    #[default]
    Specification,
    /// Neither: every aggregate is written as `@final`, wherever it is
    /// reached. An `@appendable` or `@mutable` type takes no
    /// `DHEADER`, a `@mutable` type's members take no EMHEADER, no
    /// parameter header and no sentinel, and its `@optional` member
    /// takes rule (20)'s one-octet flag.
    Never,
}

impl AggregateDelimiter {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Specification, Self::Never];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Specification)
    }

    /// Whether an `@appendable` or `@mutable` aggregate writes a
    /// `DHEADER` at all.
    #[must_use]
    pub const fn writes_dheader(self) -> bool {
        matches!(self, Self::Specification)
    }

    /// Whether a `@mutable` type's members are framed as
    /// `MMEMBER`s — an EMHEADER, a parameter header and a sentinel.
    #[must_use]
    pub const fn frames_mutable_members(self) -> bool {
        matches!(self, Self::Specification)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Specification => "specification",
            Self::Never => "never",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "specification" => Some(Self::Specification),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

/// Whether an element of a `sequence` is preceded by its own octet
/// length.
///
/// A fourth position, and the only one on which
/// [`AggregateDelimiter::Never`] is not the answer: a product whose
/// serialiser frames every aggregate as `@final` writes a four-octet
/// length in front of each element of a `sequence` whose element is not
/// `@final`, and writes it under XCDR1 as well.
///
/// # The measurement
///
/// `tests/interop-results/sequence-struct-appendable/hdds/plain-cdr1.le.bin`
/// is
/// `00000000 01000000 06000000 79 29 ed ff fe ff 0000 03000000 06000000 01000000 0200 …`
/// for `@appendable struct Cell { long a; short b; }` in three
/// `sequence<Cell>` members. The `06 00 00 00` is the element's own six
/// octets, and the file is byte-identical under XCDR2, where
/// §7.4.3.4.1 gives no delimiter at all.
///
/// Three things narrow it, and each is a recorded row:
///
/// - the element type's extensibility. `sequence-struct-final` is the
///   same type declared `@final` and carries no such word: 44 octets
///   with no `06 00 00 00` anywhere.
/// - the collection. `array-1d-struct-appendable` is the same
///   `@appendable Cell` in `Cell items[2]` and writes
///   `01 00 00 00 02 00 03 00 00 00 04 00 77`, thirteen octets and no
///   word in front of either element.
/// - the encoding version. The two files are the same octets, so
///   whatever writes it does not consult `Encoding`.
///
/// The collection delimiter itself is a different question and already
/// has one: the same rows carry none under XCDR2 either, which is
/// [`CollectionDelimiterPolicy::Never`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum SequenceElementDelimiter {
    /// XTypes 1.3 §7.4.3.4.1 and §7.4.3.5.3: no length of its own in
    /// front of an element. The default, and what every measured
    /// implementation except the one named above writes.
    #[default]
    Never,
    /// A four-octet `UInt32` — the element's own octet count — in
    /// front of every element of a `sequence` whose element type is not
    /// `@final`, under both encoding versions.
    BeforeEachNonFinalElement,
}

impl SequenceElementDelimiter {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Never, Self::BeforeEachNonFinalElement];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Never)
    }

    /// Whether an element of a `sequence` is preceded by its own octet
    /// length when its type is not `@final`.
    #[must_use]
    pub const fn writes_element_length(self) -> bool {
        matches!(self, Self::BeforeEachNonFinalElement)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::BeforeEachNonFinalElement => "before-each-non-final-element",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "never" => Some(Self::Never),
            "before-each-non-final-element" => Some(Self::BeforeEachNonFinalElement),
            _ => None,
        }
    }
}

/// Which octets stand for a `wstring`'s characters.
///
/// XTypes 1.3 Table 31 gives `Char16` for a `wstring`'s code unit and
/// §7.4.1.1.2 fixes the two things that follow from it: the length is
/// "twice the number of characters in the string", and the
/// representation "shall not include a terminating NUL character(s)".
/// [`WstringLengthPolicy`] answers how the length is counted and
/// whether there is a terminator; this answers what the characters
/// **are**, and the two are separate because they part.
///
/// # The measurement
///
/// `tests/interop-results/wstring-member/hdds/plain-cdr1.le.bin` is
/// `04 00 00 00 61 62 63 00` for `@appendable struct S { wstring w; }`
/// with `w = "abc"`: three octets of UTF-8 and a NUL, with the NUL
/// inside the four the length declares — which is exactly what
/// §7.4.1.1.1 gives a narrow `string`, and nothing §7.4.1.1.2 admits
/// for a `wstring`.
///
/// `wstring-unbounded-utf8` is the row that pins the encoding rather
/// than the length. `"äöü ሴ Z"` is seven characters; UTF-8 puts them
/// in twelve octets and the recorded file declares
/// `0d 00 00 00` — thirteen, the twelve and the NUL — where
/// §7.4.1.1.2's rule and Table 31 give fourteen. The next four octets
/// are `c3 a4 c3 b6 c3 bc 20 e1 88 b4 20 5a 00`, the twelve in UTF-8
/// and the terminator.
///
/// # What it does not reach
///
/// The scalar `wchar` is [`WcharWidth`]'s and not this: a peer whose
/// `wstring` is UTF-8 can still write a four-octet scalar `wchar`, and
/// one that writes a UTF-8 `wstring` does not thereby say what a
/// `wchar` is. `rmw_cyclonedds_cpp` already parts the two that way.
///
/// And it makes [`WstringLengthPolicy`]'s C1 and C3 vacuous: with no
/// fixed-width code unit there is nothing for a character count to
/// count and no width for an octet count to multiply by, so the writer
/// declares the UTF-8 octet count and only C2 — the terminator — still
/// reaches the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum WstringCodeUnits {
    /// XTypes 1.3 Table 31's `Char16`, two octets per code unit, and
    /// [`WstringLengthPolicy`] decides the rest. The default.
    #[default]
    Utf16,
    /// The characters' own UTF-8 octets, one to four per code point,
    /// with [`WstringTerminator`]'s answer still reached.
    Utf8,
}

impl WstringCodeUnits {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Utf16, Self::Utf8];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Utf16)
    }

    /// Whether the length prefix counts octets rather than code units,
    /// whichever of the two answers [`WstringLengthCounts`] gives.
    ///
    /// Under [`Self::Utf8`] there is no code unit to count and no width
    /// to multiply an octet count by, so the octets are what the length
    /// names whatever C1 says. One predicate, so a reader of the writer
    /// cannot mistake one for the other.
    #[must_use]
    pub const fn counts_octets(self) -> bool {
        matches!(self, Self::Utf8)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Utf16 => "utf16",
            Self::Utf8 => "utf8",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "utf16" => Some(Self::Utf16),
            "utf8" => Some(Self::Utf8),
            _ => None,
        }
    }
}

impl DefaultedDiscriminatorMember {
    /// Both.
    pub const ALL: [Self; 2] = [Self::TypeDefault, Self::StreamValue];

    /// Whether this is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::TypeDefault)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TypeDefault => "type-default",
            Self::StreamValue => "stream-value",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo here changes what a reader hands its application.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "type-default" => Some(Self::TypeDefault),
            "stream-value" => Some(Self::StreamValue),
            _ => None,
        }
    }
}

/// Which implementation's wire behaviour a plan reproduces.
///
/// Every setting below is separately selectable, and a dialect is a
/// name for one whole set of them. It exists because a peer is not a
/// collection of independent quirks: talking to RTI Connext means
/// taking RTI's answer to every question at once, and assembling that
/// from five flags is five chances to get one wrong.
///
/// # A dialect name is a promise
///
/// `--dialect opendds`, **on its own and with no second flag**,
/// reproduces every OpenDDS byte string this project's record holds.
/// The same for the other eight measured systems.
/// `tools/interop-results/tests/dialect_completeness.rs` is that claim
/// as a gate, row by row, and it is the reason several of the settings
/// below moved into a dialect: a name that reaches a product only if
/// the caller also knows which three further switches to set is a name
/// that has moved the question rather than answered it.
///
/// **The consequence, stated once and applying throughout.** Where a
/// product's behaviour departs from the clause, the dialect named for
/// that product takes the product's behaviour, including where that
/// behaviour is a defect. Fast DDS's front end drops `@bit_bound` and
/// writes a four-octet enumeration holder; Cyclone matches no negative
/// case label on a discriminator narrower than 32 bits and writes no
/// branch at all; OpenDDS numbers a `@mutable` union's branches from 0
/// and terminates nothing. Each of those is what the peer puts on the
/// wire, and a preset that named the peer and wrote something else
/// could not talk to it. Interoperability decides here;
/// [`Dialect::Specification`] is where the clause is, and it takes no
/// vendor's departure.
///
/// Each such setting names, at its own method below, the measurement it
/// rests on and the condition under which it leaves the dialect again —
/// which is always the same condition: the product stops doing it, and
/// then the entry goes with a version saying from when.
///
/// # The default is the specification, not a product
///
/// Where the implementations disagree, this project writes what the
/// clause says unless told otherwise. [`Self::Specification`] is
/// [`Dialect::default`] and [`Self::is_default`] answers for it.
/// `docs/specs/zerodds-interop-suite-1.0.md` §7 states it and §6 lists
/// it among what is settled.
///
/// **This reverses an earlier decision, and the reversal is the point.**
/// The default was [`Self::Cyclone`], on the argument that a default has
/// to be *some* implementation's behaviour to interoperate out of the
/// box. What that argument left out is which behaviour it hands to a
/// caller who names nothing: once the products' defects moved into the
/// dialects that name those products — the pass
/// `docs/wire-axes-followup.md` records — the default started writing
/// **Cyclone's defects**. A `wstring`'s code units in the host's order
/// rather than the stream's, and no branch at all for a negative case
/// label on a narrow signed discriminator.
/// [`Self::wstring_unit_order`] and [`Self::union_label`] carry the
/// runs. A caller who named no dialect got those two, and had nothing
/// in hand saying so.
///
/// A dialect is a deliberate choice to talk to one peer, and it may
/// carry that peer's defects — that is what it is for. It must not be
/// what somebody gets who chose nothing. `--dialect cyclone` is a
/// dialect like the other eight now, complete in what it reproduces and
/// preferred in nothing.
///
/// **[`Self::Specification`] is not what any measured implementation
/// produces in full**, and that is the cost of the choice rather than an
/// argument against it. Held against each system's own record it reaches
/// most of what each writes and departs where that system departs from
/// the text; the two result tables carry the per-system count and
/// `tools/interop-results/tests/dialect_completeness.rs` holds the claim
/// that each *named* dialect is complete for its own system.
///
/// **Where it stands alone is measured and counted.**
/// `dialect_completeness.rs::the_default_writes_octets_no_measured_system_wrote`
/// holds the default's octets against every system's in each of the 1332
/// recorded (case, version, order) slots: **84** of them have no witness
/// — no recorded system wrote what the default writes there.
/// `no_single_axis_separates_the_default_from_every_measured_system` says
/// the other half, and it is the one that matters for reading this
/// number: **no axis** is alone. Every value the default holds is held
/// by some product; what nobody was measured writing is the
/// *combination*. The largest family is `map` — Fast DDS is the only
/// system that produces a `map`'s octets and it orders the entries
/// ascending by key, so the clause's declaration order has no witness in
/// 46 slots. The next is an unannotated aggregate taken as
/// `@appendable` (RTI's, Fast DDS's and OpenDDS's answer) beside a
/// holder in the declared `@bit_bound` (Cyclone's and RTI's) — each half
/// somebody's, the pair nobody's.
///
/// The same sweep over [`Self::Cyclone`] gives **147**, and that
/// comparison is asserted rather than recalled: the default this one
/// replaced stood alone on more of the record, not less.
///
/// # What each dialect holds, and where the evidence is
///
/// Every cell is derived from the recorded octets rather than written
/// down: `interop-results --dialects` searches the axis product against
/// each system's own rows and reports, per axis, the value that
/// reproduces them and whether any row **pins** it. A cell no row pins
/// is a question that system's record does not ask, and it is named as
/// such at the method that answers it rather than left to look
/// measured.
///
/// The last column is the default. It is last because the table is read
/// left to right against the products; nothing else follows from the
/// order.
///
/// | setting | Cyclone | RTI 7.7 | Fast DDS | OpenDDS | Specification |
/// |---|---|---|---|---|---|
/// | collection delimiters | specification | never | enum-is-primitive | specification | specification |
/// | a `map`'s delimiter | either half | either half | **value only** | either half | either half |
/// | PL_CDR1 headers | always-extended | extended-for-constructed-in-large-samples | short-true-length | short-true-length | specification |
/// | PL_CDR1 pid flags | sentinel-must-understand | sentinel-must-understand | bare | **impl-extension-members** | sentinel-must-understand |
/// | default extensibility | final | appendable | appendable | appendable | appendable |
/// | `wstring` length, XCDR2 | octets/unterminated/2 | octets/unterminated/2 | characters/unterminated/2 | octets/unterminated/2 | octets/unterminated/2 |
/// | `wstring` length, XCDR1 | octets/unterminated/2 | characters/**terminated**/4 | characters/unterminated/2 | octets/unterminated/2 | octets/unterminated/2 |
/// | `wstring` unit order | **host-unswapped** | stream | stream | stream | stream |
/// | scalar `wchar` width | 2 | 4 under XCDR1, 2 under XCDR2 | 2 | 2 | 2 |
/// | length codes | compact | reuse-nested-delimiter | short-where-it-fits | **sized-or-nextint** | compact |
/// | member must-understand | declared | declared | omitted | declared-and-key | declared-and-key |
/// | enumeration holder | declared | declared | **bit-bound-dropped** | **bit-bound-dropped** | declared |
/// | enumerator value | declared | declared | declared | **ordinal** | declared |
/// | union labels | **negative-unmatched-below-32** | sign-extended | sign-extended | sign-extended | sign-extended |
/// | `@mutable` union framing | specification | specification | specification | **branches-from-zero** | specification |
/// | short pid range | up-to-reserved | up-to-reserved | up-to-reserved | **all-fourteen-bits** | up-to-reserved |
/// | `map` entry order | declaration | declaration | **ascending-key** | **ascending-key** | declaration |
/// | `@hashid` counter | advances | skipped | advances | skipped | advances |
/// | key holder, XCDR2 | forced-final | forced-final | forced-final | forced-final | forced-final |
/// | key holder, XCDR1 | forced-final | as-declared | forced-final | forced-final | forced-final |
///
/// The three CORBA presets and the two ROS presets are in their own
/// sections below, because most of this table is unreachable under
/// them.
///
/// **Fast DDS's default extensibility is measured after all.** It was
/// carried here as an unknown, on the ground that `fastddsgen` refuses
/// the `common::Robot` of GitHub issues #41 and #42. The interop
/// corpus answers it without that case and always has: every
/// `-unannotated` twin is a struct with no annotation, and Fast DDS
/// writes `30000000` in front of `primitive-mixed-unannotated` under
/// XCDR2 where Cyclone writes nothing. That is `@appendable`, from a
/// recorded row, and the footnote that called it a guess was reading
/// past its own corpus.
///
/// # Where a preset does not reproduce its vendor
///
/// **Every recorded row of every measured system is reproduced by that
/// system's own dialect**, and the gate above is what says so. What is
/// left in this section is the two places where that is true of the
/// bytes and not of every value they can carry, plus the questions no
/// dialect can settle.
///
/// ## Closed, and what each one was
///
/// Kept as a list rather than deleted, because "a preset reproduces its
/// vendor" is a claim that has to be re-checkable against the run that
/// settled it.
///
/// | what the preset wrote | what the vendor writes | where |
/// |---|---|---|
/// | `Dialect::Rti`, a two-byte `wchar` under XCDR1 | four bytes, `61000000` | `wchar_member`, [`WcharWidth`] |
/// | `Dialect::FastDds`, `M_FLAG` set in the EMHEADER | never set, `01000020` | `mut_mustunderstand`, [`MustUnderstandPolicy`] |
/// | `Dialect::FastDds`, LC4 for an eight-byte `sequence<short>` | LC3, `02000030` | `mut_lengthcodes`, [`LengthCodePolicy`] |
/// | `Dialect::Cyclone`, LC3 for `long ap[2]` and for an eight-byte nested `Cell` | LC4, `07000040` and `08000040` | `mut_lengthcodes` |
/// | `Dialect::Rti`, the same two | LC4, the same two | `mut_lengthcodes` |
/// | `Dialect::Rti`, LC4 over an `@appendable` nested member | LC5 over its own DHEADER, `00000050` | `mut_lc_nested` |
/// | `Dialect::FastDds`, a DHEADER on `sequence<bitmask>` | none, `01000030` | `mut_lc_enum`, [`CollectionDelimiterPolicy`] |
/// | `Dialect::Cyclone`, a `wstring`'s units in the stream's order | the host's, `61006200` big endian | [`WstringUnitOrder`], 12 rows |
/// | `Dialect::Cyclone`, a negative narrow case label matching | no branch written | [`UnionLabelMatch`], 24 rows |
/// | `Dialect::FastDds`, an enumeration in its declared bound | four octets whatever the bound | [`EnumHolderWidth`], 52 rows |
/// | `Dialect::FastDds`, a `map` in declaration order | ascending by key | [`MapEntryOrder`], 32 rows |
/// | `Dialect::FastDds`, a DHEADER on `map<string, long>` | none | [`MapDelimiter`], 12 rows |
/// | `Dialect::JacOrb`, a `fixed<P,S>`'s declared digit count | the value's precision | [`FixedDigitCount`], 6 rows |
/// | no dialect at all for OpenDDS | six settings between them | [`Self::OpenDds`], 936 rows |
/// | no dialect at all for either ROS type support | two settings and a coupling defect | [`Self::RosFastRtps`], [`Self::RosCyclone`], 369 rows |
///
/// The last three lines are this pass. The ones above them were closed
/// earlier and are kept for the same reason.
///
/// ## `Dialect::FastDds` does not reproduce Fast DDS's lost characters
///
/// One layer under the framing, and it is a **value** question rather
/// than a byte one. Fast CDR counts characters and writes each
/// four-byte `wchar_t` as two bytes, so a code point outside the BMP
/// goes out as its low sixteen bits: `U+1D11E` becomes `d11e`, which is
/// `U+D11E`, a Hangul syllable and a perfectly well-formed code point.
/// The peer reads a different character and has nothing to object to.
///
/// [`WstringLengthPolicy::CHARACTER_COUNT`] reproduces the length
/// convention and **refuses** the value, rather than truncating it.
/// Reproducing a vendor's framing is what a dialect is for; reproducing
/// a data loss that leaves no trace in the bytes is this project putting
/// a different character on the wire and calling it interoperability.
/// So `--dialect fastdds` writes Fast DDS's bytes for every `wstring`
/// Fast DDS does not corrupt, and errors on the ones it does. No
/// recorded row carries such a value, so this costs no row.
///
/// ## `Dialect::OpenDds` takes the declared `fixed<P,S>` count, as OpenDDS writes it
///
/// OpenDDS 3.34's serializer writes a `fixed` as
/// `OpenDDS::FaceTypes::Fixed_T<P,S>`, `(P + 2) / 2` octets: the
/// declared count, measured 2026-10-07 through the FACE mapping
/// (`0.00` under `fixed<5,2>` is `00000c`, `7` is `00700c`). Its
/// `write_fixed(to_ace_fixed())` is the ValueWriter path, and
/// `ACE_CDR::Fixed`'s own CDR operator — the string's digits,
/// [`FixedDigitCount::ValueStringDigits`], `unspecified_constructs.md`
/// §3 — is not on its wire. No recorded row of the OpenDDS column has a
/// `fixed`: the probe builds the classic mapping, where `tao_idl`
/// refuses the type and the struct comes out without the member.
///
/// ## No preset reproduces any vendor's `@hashid` member ids
///
/// The one place where naming a dialect cannot help, because the three
/// implementations disagree with each other as well as with this
/// project. XTypes 1.3 §7.3.1.2.1.1 defines the hash; which *name* goes
/// into it is where they part. `vendor_roundtrip.md`, "`@hashid` member
/// ids are not agreed by anyone", and the inventory's I22. A preset that
/// picked one would be reproducing one vendor and diverging from the
/// clause and from the other two, with nothing to say which.
///
/// ## Two extrapolations inside `Dialect::Rti` and `Dialect::FastDds`
///
/// Both in [`LengthCodePolicy`], both from the same measurement, both
/// written down here rather than left in a code comment:
///
/// - **a union inside a `@mutable` type.** Cyclone's source puts a union
///   and a nested aggregate in one arm and returns LC4 for both. RTI and
///   Fast DDS were measured reusing a nested aggregate's DHEADER, and
///   neither was measured on a union — no vendor row has one inside a
///   `@mutable` type. The two are treated alike, on the reading that one
///   clause and one leading word cannot have two answers.
/// - **a delimited nested aggregate of exactly one, two, four or eight
///   bytes.** [`LengthCodePolicy::ShortWhereItFits`] tries the reuse
///   first, because Fast DDS frames an eight-byte `string` as LC5 and
///   not LC3. Whether it would do the same for an eight-byte delimited
///   aggregate is untested; the shape does not arise in the measured
///   cases, where every delimited member is twelve bytes or more.
///
/// ## `Dialect::Rti` reproduces RTI, including where RTI departs
///
/// Not a gap, and here because it reads like one. RTI keeps a `@mutable`
/// type's parameter list in the key holder under XCDR1, so its KeyHash
/// is the MD5 of the framing and not the specification's key
/// zero-padded. [`KeyHolderExtensibility`] carries the run and
/// `Dialect::Rti` selects it, under XCDR1 only. Two peers that disagree
/// here compute different handles for one instance, which is not a
/// formatting difference — so unlike the byte order above, the departure
/// is modelled rather than recorded.
///
/// # What a dialect deliberately does not set
///
/// A `map`'s entry **count**, which is `MapLengthPolicy` and is not a
/// `Dialect` method at all. XTypes 1.3 §7.4.1.1.4 and rule (15) of
/// §7.4.3.5.3 contradict each other; Fast DDS is the one implementation
/// in the corpus that produces a `map`'s octets, and its rows pin the
/// equivalence's reading — the count is there under a delimiter as well
/// as without one. OpenDDS's generated `operator<<` reads rule (15) the
/// other way, calling `write_delimiter` under version 2 and the
/// `<< length` in the `else` branch under version 1 — and since
/// 2026-08-24 its own rows measure exactly that: `map-key-enum` and
/// `map-value-struct` under XCDR2 carry the DHEADER and no count,
/// `map-key-long` (primitive on both halves, undelimited) carries the
/// count. The `opendds` matrix configuration passes
/// `--map-length rule-15-omits`; the policy stays a probe flag rather
/// than a `Dialect` method because the two readings now have one
/// measured implementation each.
///
/// Which of a map's two halves decides whether it is *delimited* is a
/// different question, is measured, and is [`MapDelimiter`].
///
/// # The three ORB dialects
///
/// XCDR1 *is* CORBA CDR — CORBA 3.3 part 2 §9.3 — so an ORB is a
/// reference for the whole shared base and [`Self::OmniOrb`],
/// [`Self::Tao`] and [`Self::JacOrb`] are presets like the other three.
/// They differ from them in reach rather than in kind: an ORB has no
/// encoding version 2, no `@mutable`, no DHEADER and no EMHEADER, so
/// most of the settings above are unreachable under them and take the
/// clause's value. What they do reach is the wide-character family,
/// where CORBA and XTypes genuinely part.
///
/// | | omniORB 4.3.3 | TAO 2.5.24 | JacORB 3.9 |
/// |---|---|---|---|
/// | `wchar` framing | giop-length-prefixed | giop-length-prefixed-member-only | giop-length-prefixed |
/// | scalar `wchar` width | 2 | 4 | 2 |
/// | `wstring` unit width | 2 | 4 | 2 |
/// | `wstring` length | octets/unterminated | octets/unterminated | octets/unterminated |
/// | `wstring` unit order | byte-order-mark | stream | big-endian-always |
/// | `fixed<P,S>` digits | declared | declared, and unmeasured | **value-precision** |
/// | default extensibility | final | final | final |
///
/// JacORB's digit count is the one CORBA-only setting a dialect took in
/// this pass: [`FixedDigitCount`] carries the six rows.
/// [`Self::Tao`]'s cell is not a measurement — `tao_idl` refuses
/// `fixed` outright, so TAO has no recorded row — and the section above
/// says what ACE's runtime does instead.
///
/// # The two ROS 2 dialects
///
/// Neither is its vendor. `rosidl_typesupport_fastrtps_c` generates its
/// own serialisation rather than calling `fastddsgen`'s output, and
/// `rmw_cyclonedds_cpp` walks the introspection type support rather
/// than using Cyclone's `idlc`; both parse `@final`, `@key`,
/// `@optional` and `@id` and drop them, so every ROS message is a
/// `@final` struct in encoding version 1 whatever its `.idl` says.
/// Almost every setting above is therefore unreachable under them and
/// takes the clause's value.
///
/// What they do reach is the wide-character family, and there they
/// part:
///
/// | | `ros-fastrtps` 2.2.2 | `ros-cyclone` 1.3.4 |
/// |---|---|---|
/// | `wstring` length | characters/unterminated/4 | characters/unterminated/4 |
/// | `wstring` unit order | host-unswapped | stream |
/// | scalar `wchar` width | **4** | **2** |
///
/// The width row is the one that made [`WcharWidth`] an axis: it was
/// read off [`WstringLengthPolicy`] until `rmw_cyclonedds_cpp` was
/// fitted, and one setting cannot answer `e4003412` for a member and
/// four-octet units inside a `wstring` at once. The unit-order row is
/// measured for `ros-fastrtps` — it writes both byte orders and its
/// big-endian rows keep the units little-endian — and is **not**
/// separable for `ros-cyclone`, which writes the host's order and takes
/// no argument for it, so its record has one order and the two answers
/// coincide in it. [`Self::wstring_unit_order`] says which of the two
/// that is and why.
///
/// The extensibility row is not a measurement and does not need to be:
/// CORBA IDL has no extensibility annotation, an ORB writes the members
/// in declaration order with no framing, and under XCDR1 — the only
/// encoding an ORB has — `@appendable` is byte-for-byte `@final`
/// (rule 29, the inventory's H2). `@final` is what an ORB's bytes are;
/// naming it `Appendable` would be true of the bytes and false of the
/// type.
///
/// Two cells are extrapolations and both are named where a reader will
/// look: JacORB's unit order comes from its **read** direction, because
/// `org.jacorb.orb.CDROutputStream` has no `setLittleEndian` and every
/// JacORB byte string is big-endian, where the two candidate orders
/// coincide; and TAO's unit order is unmeasured for the same reason in
/// reverse, because libACE here was built without
/// `ACE_ENABLE_SWAP_ON_WRITE` and TAO writes host order only.
/// [`WstringUnitOrder::BigEndianAlways`] carries the first and
/// [`Self::Tao`] the second.
///
/// # Why the wide-character policies are not named for their GIOP versions
///
/// The measurement makes the case for it and then takes it back. CORBA
/// §9.3.1.6 has exactly two wide-character encodings, one per GIOP
/// version, and the DDS implementations split along that line:
/// [`WstringLengthPolicy::SPECIFICATION`] is GIOP 1.2's `wstring` and
/// [`WstringLengthPolicy::CHARACTER_COUNT_TERMINATED`] is GIOP 1.1's, and
/// TAO reproduces each by changing only the GIOP minor version. Naming
/// them `Giop11` and `Giop12` would put the citation in the name.
///
/// Four things say not to.
///
/// 1. **One of the forms belongs to no edition.**
///    [`WstringLengthPolicy::CHARACTER_COUNT`] — Fast DDS's — is a
///    character count with no terminator, which is neither §9.3.1.6
///    edition. A naming scheme that covers three of five variants and
///    leaves the other two mechanism-named is worse than five mechanism
///    names, because it reads as if the two were unclassified rather
///    than as if they were unclassifiable.
/// 2. **The GIOP version does not fix the whole form.** It fixes the
///    length convention; the code-unit width is the transmission code
///    set, which it does not fix. omniORB and TAO are both GIOP 1.2 and
///    write two octets and four. That is why
///    [`WstringLengthPolicy::SPECIFICATION_WIDE_UNITS`] exists at all, and
///    a variant called `Giop12` sitting beside another called `Giop12`
///    is not a naming scheme.
/// 3. **The default would be named for the wrong specification.** This
///    project implements XTypes; §7.4.1.1.2 is what
///    [`WstringLengthPolicy::SPECIFICATION`] cites. That the clause
///    coincides with GIOP 1.2's `wstring` is a derivation worth
///    recording, not an identity — and it is not an identity across the
///    pair, because XTypes' `Char16` is fixed-width, which is GIOP
///    **1.1**'s `wchar`. XTypes' own combination is GIOP 1.2's `wstring`
///    with GIOP 1.1's `wchar`, a pairing no edition produces. A variant
///    named `Giop12` that selected it would be wrong in its own name.
/// 4. **A name should say what the bytes are.** Somebody holding a
///    length field of 3 where 4 was expected needs "character count,
///    terminated"; "GIOP 1.1" sends them to a document. The citation
///    belongs in the doc comment and in the table above, where it now
///    is, and the equivalence is stated in both directions so it can be
///    followed either way.
///
/// What the GIOP reading *did* change is the shape of the model rather
/// than the names in it: the length convention and the unit width are
/// two axes because §9.3.1.6 makes them two, and the byte order is a
/// third because the code-set rule makes it one.
/// [`WcharFraming`] and [`WstringUnitOrder`] are that reading, and
/// without it the three ORB presets would each have been a fourth
/// special case in one overloaded enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Dialect {
    /// Eclipse Cyclone DDS.
    Cyclone,
    /// RTI Connext DDS 7.7.
    Rti,
    /// eProsima Fast DDS with Fast CDR 2.3.5.
    FastDds,
    /// The clause-by-clause reading of XTypes 1.3, and **the default**.
    ///
    /// No measured implementation produces it in full, which is what
    /// makes it the default rather than what argues against it: the
    /// default is what the specifications say, and a dialect is a
    /// deliberate choice to talk to one peer and to carry that peer's
    /// defects while doing it. [`Self::is_default`] and the
    /// "Why the specification is the default" section of this type's
    /// documentation carry the reasoning.
    #[default]
    Specification,
    /// omniORB 4.3.3, over CORBA CDR — which is XCDR1.
    ///
    /// A two-octet transmission code set, GIOP 1.2's length-prefixed
    /// `wchar`, and a byte-order mark inside every `wstring`.
    OmniOrb,
    /// TAO 2.5.24, over CORBA CDR.
    ///
    /// A **four**-octet transmission code set and GIOP 1.2's
    /// length-prefixed `wchar`, which makes its `wstring` the one form
    /// no DDS implementation writes: an octet count over wide units.
    Tao,
    /// JacORB 3.9, over CORBA CDR.
    ///
    /// A two-octet code set like omniORB's and GIOP 1.2's `wchar`, with
    /// no byte-order mark: it reads the units big-endian whatever the
    /// stream says.
    JacOrb,
    /// OpenDDS 3.34.0 over `OpenDDS::DCPS::Serializer`.
    ///
    /// Fast DDS's XCDR1 `@mutable` framing with a parameter id no other
    /// implementation writes — `PID_IMPL_EXTENSION` and
    /// `PID_MUST_UNDERSTAND` set on every member — an XCDR2 length code
    /// that is short where the serialised length fits and never reuses
    /// a nested delimiter, `@key` implying must-understand as
    /// §7.2.2.4.4.4.8 requires, and a front end that carries neither
    /// `@value` nor `@bit_bound` as far as a serialiser.
    OpenDds,
    /// `rosidl_typesupport_fastrtps_c` 2.2.2, ROS 2 Humble.
    ///
    /// Not Fast DDS: it generates its own serialisation. Encoding
    /// version 1 only, every message `@final`, a `wstring` counted in
    /// characters over four-octet units whose octets stay in the host's
    /// order while the length swaps, and a four-octet scalar `wchar`
    /// that does swap.
    RosFastRtps,
    /// `rmw_cyclonedds_cpp` 1.3.4, ROS 2 Humble.
    ///
    /// Not Cyclone: it walks the introspection type support. The same
    /// `wstring` cell as [`Self::RosFastRtps`] over units in the
    /// stream's order, and a **two**-octet scalar `wchar` beside those
    /// four-octet units — the one implementation of the nine where the
    /// two widths part, and the reason [`WcharWidth`] is an axis.
    RosCyclone,
    /// OpenSplice Community Edition 6.9, over
    /// `DDS::OpenSplice::CdrTypeSupport`.
    ///
    /// `idlpp -S -l cpp` generates the standalone C++ mapping and
    /// `CdrTypeSupport::serialize` writes it: XCDR1 in the host's byte
    /// order, and nothing else — no XTypes, so no encoding version 2
    /// and no `@mutable`. `tools/wireprobe/opensplice-idl.py` rewrites a
    /// corpus type into the IDL 3.5 `idlpp` reads only where every
    /// construct means the same XCDR1 bytes there, and refuses the rest
    /// by name: `@mutable`, `@optional`, `@external`,
    /// `@must_understand`, an enumerator's `@value`, an enum's
    /// `@bit_bound` below 32, `bitmask`, `bitset`, `map`, a keyed union
    /// discriminator and a union as the topic type, and `idlpp` itself
    /// refuses `wchar` and `wstring` outright.
    ///
    /// One axis is measured: OpenSplice's 2 rows of
    /// `union-discriminator-short` and its unannotated twin — the one
    /// byte order and encoding version it writes — put it beside RTI,
    /// Fast DDS and OpenDDS on [`UnionLabelMatch::SignExtended`] — the
    /// clause's answer, and [`Self::Specification`]'s. Every other axis is
    /// unpinned for this dialect: `wchar`/`wstring`, `@mutable` framing,
    /// `map`, `@bit_bound` below 32, `@value` and `fixed` are exactly
    /// the constructs the front end above cannot express, so no recorded
    /// row asks any of them a question, and this preset takes the
    /// clause's answer on every one rather than guess at a form nobody
    /// measured.
    OpenSplice,
    /// HDDS, over the serialiser `hddsgen gen rust` writes into
    /// `encode_xcdr1_le` / `encode_xcdr2_le`.
    ///
    /// **Opt-in, and the column measures its generated code and not its
    /// runtime** — see `docs/adding-a-column.md` step 3 and
    /// `tools/wireprobe/hdds-shim`. `hddsgen` emits no big-endian
    /// encoder at all, so half of every cell does not exist; this preset
    /// carries no endianness of its own and the gate that sweeps it
    /// reaches only the little-endian rows.
    ///
    /// Four departures carry the column, and each has its own axis:
    ///
    /// | departure | axis | rows |
    /// |---|---|---|
    /// | a collection's elements follow one another with no padding between them, and align from their own first octet | [`ElementPadding::Packed`] | `array-1d-struct-final`, `sequence-struct-final` |
    /// | every aggregate is written as `@final`: no `DHEADER`, no EMHEADER, no parameter header | [`AggregateDelimiter::Never`] | `struct-appendable`, `annotation-must-understand-unannotated` |
    /// | …except in front of a `sequence` element of a non-`@final` type, under both versions | [`SequenceElementDelimiter::BeforeEachNonFinalElement`] | `sequence-struct-appendable`, `empty-struct-in-sequence` |
    /// | a `wstring` is UTF-8 with a NUL the length counts | [`WstringCodeUnits::Utf8`] | `wstring-member`, `wstring-unbounded-utf8` |
    ///
    /// Three more answers come off axes that already existed:
    /// [`WcharWidth::Four`] (`char-and-wchar`), [`EnumHolderWidth::BitBoundDroppedByFrontEnd`]
    /// (`combined-keyed-topic`) and [`CollectionDelimiterPolicy::Never`]
    /// (`sequence-struct-final`). What is left is unpinned and takes the
    /// clause's answer, each arm of each method saying which.
    Hdds,
}

impl Dialect {
    /// Every dialect.
    pub const ALL: [Self; 12] = [
        Self::Cyclone,
        Self::Rti,
        Self::FastDds,
        Self::Specification,
        Self::OmniOrb,
        Self::Tao,
        Self::JacOrb,
        Self::OpenDds,
        Self::RosFastRtps,
        Self::RosCyclone,
        Self::OpenSplice,
        Self::Hdds,
    ];

    /// Every dialect that is a CORBA ORB rather than a DDS
    /// implementation.
    ///
    /// Named because the difference is not cosmetic: an ORB has no
    /// encoding version 2 and no `@mutable`, so a test that sweeps every
    /// dialect through every encoding has to know which of the two lists
    /// it is in. [`Self::is_corba`] is the same fact per value.
    pub const CORBA: [Self; 3] = [Self::OmniOrb, Self::Tao, Self::JacOrb];

    /// Whether this dialect is a CORBA ORB.
    ///
    /// An ORB implements CORBA CDR, which is XCDR1 and nothing else:
    /// CORBA 3.3 part 2 §9.3 has no delimiter header, no EMHEADER and no
    /// encoding version field.
    ///
    /// Asking one of these presets for an XCDR2 plan is **not** refused,
    /// and the reason is worth stating because a refusal was the first
    /// instinct. Every setting the preset carries has an answer under
    /// both encoding versions — the wide-character three because a
    /// `wchar` and a `wstring` are the same under either, and the rest
    /// because they take the clause's value — so the plan that comes out
    /// is well defined and the three columns are held to it by the
    /// three-column gate, which runs these presets over every encoding.
    /// What such a plan is *not* is a reproduction of any ORB: there is
    /// no XCDR2 byte string from omniORB, TAO or JacORB to be right or
    /// wrong about. Nothing is dropped and nothing is misplaced; the
    /// combination simply has no foreign side, which is what this method
    /// is for saying.
    #[must_use]
    pub const fn is_corba(self) -> bool {
        matches!(self, Self::OmniOrb | Self::Tao | Self::JacOrb)
    }

    /// Whether this is the default.
    ///
    /// [`Self::Specification`], and the one place the answer is written
    /// as a predicate rather than read off `#[default]`. A caller that
    /// wants to know whether a plan is the shipped one asks here; a
    /// caller that wants the plan itself takes [`Dialect::default`].
    #[must_use]
    pub const fn is_default(self) -> bool {
        matches!(self, Self::Specification)
    }

    /// Short name, for configuration and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cyclone => "cyclone",
            Self::Rti => "rti",
            Self::FastDds => "fastdds",
            Self::Specification => "specification",
            Self::OmniOrb => "omniorb",
            Self::Tao => "tao",
            Self::JacOrb => "jacorb",
            Self::OpenDds => "opendds",
            Self::RosFastRtps => "ros-fastrtps",
            Self::RosCyclone => "ros-cyclone",
            Self::OpenSplice => "opensplice",
            Self::Hdds => "hdds",
        }
    }

    /// Parse a configuration value.
    ///
    /// An unknown value returns `None` rather than falling back to the
    /// default: a typo in a wire-affecting setting must be loud.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cyclone" => Some(Self::Cyclone),
            "rti" => Some(Self::Rti),
            "fastdds" => Some(Self::FastDds),
            "specification" => Some(Self::Specification),
            "omniorb" => Some(Self::OmniOrb),
            "tao" => Some(Self::Tao),
            "jacorb" => Some(Self::JacOrb),
            "opendds" => Some(Self::OpenDds),
            "ros-fastrtps" => Some(Self::RosFastRtps),
            "ros-cyclone" => Some(Self::RosCyclone),
            "opensplice" => Some(Self::OpenSplice),
            "hdds" => Some(Self::Hdds),
            _ => None,
        }
    }

    /// Which collections carry a delimiter under XCDR2.
    #[must_use]
    pub const fn collection_delimiters(self) -> CollectionDelimiterPolicy {
        match self {
            // The three ORBs take the clause's value because they
            // cannot reach the question: a collection delimiter is an
            // XCDR2 header (rules 9 and 12) and CORBA CDR has no
            // encoding version 2. Written out rather than folded into
            // the `Specification` arm so that adding an eighth dialect
            // does not silently inherit an answer nobody measured.
            Self::Cyclone | Self::Specification | Self::OmniOrb | Self::Tao | Self::JacOrb => {
                CollectionDelimiterPolicy::Specification
            }
            Self::Rti => CollectionDelimiterPolicy::Never,
            Self::FastDds => CollectionDelimiterPolicy::EnumIsPrimitive,
            // Pinned by OpenDDS's own XCDR2 rows: it delimits exactly
            // the collections rules 9 and 12 name.
            Self::OpenDds => CollectionDelimiterPolicy::Specification,
            // Unreachable in either ROS column: both write encoding
            // version 1 only, where no collection carries a delimiter
            // at all. The clause's value, and nothing measured.
            Self::RosFastRtps | Self::RosCyclone => CollectionDelimiterPolicy::Specification,
            // Unreachable: a collection delimiter is an XCDR2 header
            // and OpenSplice 6.9 has no encoding version 2. The
            // clause's value, and nothing measured.
            Self::OpenSplice => CollectionDelimiterPolicy::Specification,
            // Measured: `sequence-struct-final` and `array-1d-string`
            // are byte-identical under the two versions, so the XCDR2
            // delimiter rules 9 and 12 name is not written.
            Self::Hdds => CollectionDelimiterPolicy::Never,
        }
    }

    /// Which length code a `@mutable` member's EMHEADER gets.
    ///
    /// The three take three positions and [`LengthCodePolicy`] carries
    /// the run they were measured in. This used to answer `Compact` for
    /// all four, which made `Dialect::Rti` four bytes long on every
    /// delimited nested member and `Dialect::FastDds` four bytes long on
    /// every eight-byte one.
    #[must_use]
    pub const fn length_codes(self) -> LengthCodePolicy {
        match self {
            // §7.4.3.4.2 permits every one of them, so the
            // clause-by-clause dialect takes the majority form rather
            // than a form of its own.
            Self::Cyclone | Self::Specification => LengthCodePolicy::Compact,
            Self::Rti => LengthCodePolicy::ReuseNestedDelimiter,
            Self::FastDds => LengthCodePolicy::ShortWhereItFits,
            // Unreachable under an ORB: a length code lives in an
            // EMHEADER, which is XCDR2's `@mutable` framing.
            Self::OmniOrb | Self::Tao | Self::JacOrb => LengthCodePolicy::Compact,
            // Pinned by OpenDDS's XCDR2 `@mutable` rows: short where
            // the serialised length fits a code, LC4 otherwise, and
            // never a reused leading word.
            Self::OpenDds => LengthCodePolicy::SizedOrNextInt,
            // Unreachable in either ROS column, as the delimiter is.
            Self::RosFastRtps | Self::RosCyclone => LengthCodePolicy::Compact,
            // Unreachable: a length code lives in an EMHEADER, which is
            // XCDR2's `@mutable` framing, and OpenSplice has neither.
            Self::OpenSplice => LengthCodePolicy::Compact,
            // Unpinned: `hddsgen` generates no parameter-list encoder
            // at all, so no recorded row of that column writes an
            // EMHEADER. The clause's answer.
            Self::Hdds => LengthCodePolicy::Compact,
        }
    }

    /// How a `@mutable` member is framed under XCDR1.
    #[must_use]
    pub const fn pl_cdr1_headers(self) -> PlCdr1HeaderPolicy {
        match self {
            Self::Cyclone => PlCdr1HeaderPolicy::AlwaysExtended,
            Self::Specification => PlCdr1HeaderPolicy::Specification,
            Self::Rti => PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples,
            Self::FastDds => PlCdr1HeaderPolicy::ShortTrueLength,
            // Unreachable under an ORB: PL_CDR1 frames a `@mutable`
            // type and CORBA IDL has no extensibility annotation, so
            // `wireprobe-corba-idl` refuses such a type by name before
            // any ORB sees it.
            Self::OmniOrb | Self::Tao | Self::JacOrb => PlCdr1HeaderPolicy::Specification,
            // Fast DDS's framing exactly — the short header and the
            // member's true length — with a different parameter id.
            // `struct-mutable` under XCDR1: `c0000004` where Fast DDS
            // writes `00000004`, and `c0020002` where RTI rounds the
            // `short`'s length to `0004`.
            Self::OpenDds => PlCdr1HeaderPolicy::ShortTrueLength,
            // Unreachable: every ROS message is a `@final` XCDR1
            // struct whatever its `.idl` says, so no parameter list is
            // ever written.
            Self::RosFastRtps | Self::RosCyclone => PlCdr1HeaderPolicy::Specification,
            // Unreachable: PL_CDR1 frames a `@mutable` type and
            // `tools/wireprobe/opensplice-idl.py` refuses `@mutable` by
            // name before OpenSplice ever sees the file.
            Self::OpenSplice => PlCdr1HeaderPolicy::Specification,
            // Unreachable the same way: `wireprobe-hdds` refuses
            // `--encoding pl-cdr1` because `hddsgen` emits no
            // parameter-list encoder, so no recorded row of that column
            // has one. The clause's answer.
            Self::Hdds => PlCdr1HeaderPolicy::Specification,
        }
    }

    /// What flags a PL_CDR1 parameter id carries, at a member and at the
    /// sentinel.
    ///
    /// [`PidFlagPolicy::ImplExtensionOnMembers`] is **`Dialect::OpenDds`'s**,
    /// and it is in that dialect because it is what OpenDDS puts on the
    /// wire. Measured on `struct-mutable` and
    /// `evolution-mutable-mustunderstand` under XCDR1, both byte
    /// orders: `c0010004 c0020004 c0030004`, `PID_IMPL_EXTENSION` and
    /// `PID_MUST_UNDERSTAND` set on the declared member and on the two
    /// undeclared ones alike, and a bare `3f02` sentinel after them.
    /// It is reached by `--pid-flags impl-extension-members` as well,
    /// for a peer that wants the point without the rest of the preset.
    ///
    /// It leaves this dialect the day OpenDDS stops setting the two
    /// bits unconditionally, with the version it stopped in.
    #[must_use]
    pub const fn pid_flags(self) -> PidFlagPolicy {
        match self {
            Self::Cyclone | Self::Rti | Self::Specification => {
                PidFlagPolicy::SentinelMustUnderstand
            }
            Self::FastDds => PidFlagPolicy::Bare,
            Self::OpenDds => PidFlagPolicy::ImplExtensionOnMembers,
            // Unreachable: a ROS message is `@final` and writes no
            // parameter list.
            Self::RosFastRtps | Self::RosCyclone => PidFlagPolicy::SentinelMustUnderstand,
            // Unreachable under an ORB: a parameter list frames a
            // `@mutable` type and CORBA IDL has no extensibility
            // annotation, so `wireprobe-corba-idl` refuses such a type
            // by name before any ORB sees it. Written out rather than
            // folded into the arm above so that an eighth dialect does
            // not silently inherit an answer nobody measured.
            Self::OmniOrb | Self::Tao | Self::JacOrb => PidFlagPolicy::SentinelMustUnderstand,
            // Unreachable: a PL_CDR1 parameter id frames a `@mutable`
            // member and OpenSplice has none.
            Self::OpenSplice => PidFlagPolicy::SentinelMustUnderstand,
            // Unreachable for the same reason as `length_codes`: no
            // recorded row of that column carries a parameter id.
            Self::Hdds => PidFlagPolicy::SentinelMustUnderstand,
        }
    }

    /// How wide an enumeration's holder is.
    ///
    /// [`EnumHolderWidth::BitBoundDroppedByFrontEnd`] is **Fast DDS's
    /// and OpenDDS's**, and it is in both of their dialects. It is a
    /// front-end defect and not a reading of §7.4.3.5.3 — M1 of
    /// `docs/wire-axes-followup.md` has `fastddsgen` 4.3.0 emitting
    /// `enum class Colour : int32_t` and `BitBound = 32` for an
    /// `@bit_bound(8)` enumeration, and `opendds_idl` not knowing the
    /// symbol at all — and that is exactly why a dialect has to carry
    /// it: a peer that writes four octets writes them whatever the
    /// reason, and a reader of one octet is three octets out of step
    /// from there on. Naming the product is asking to talk to the
    /// product.
    ///
    /// Pinned by 76 recorded rows, both encoding versions and both byte
    /// orders: `enum-bitbound-{8,16}-{plain,value}` and their
    /// unannotated twins, `array-3d-enum`, `combined-everything` and
    /// `combined-keyed-topic`. It leaves those two dialects the day
    /// either front end carries the annotation as far as a serialiser,
    /// with the version it started in.
    ///
    /// `--enum-holder bit-bound-dropped` reaches it as a single point.
    ///
    /// Written out for all ten rather than a catch-all arm, so that an
    /// eleventh dialect does not silently inherit an answer nobody
    /// measured for it.
    #[must_use]
    pub const fn enum_holder(self) -> EnumHolderWidth {
        match self {
            Self::FastDds | Self::OpenDds => EnumHolderWidth::BitBoundDroppedByFrontEnd,
            // Measured: `combined-keyed-topic` puts `@bit_bound(8) enum
            // Mode { IDLE, ACTIVE, FAULT }`'s `FAULT` out as
            // `02 00 00 00`, four octets, where the clause's bound says
            // one.
            Self::Hdds => EnumHolderWidth::BitBoundDroppedByFrontEnd,
            // Cyclone honours the bound: `enum-bitbound-8-plain` under
            // XCDR1 big endian is `0200ab`, one octet a value.
            Self::Cyclone
            | Self::Rti
            | Self::Specification
            // Unreachable in either ROS column: no recorded row of
            // either carries a narrow-bound enumeration.
            | Self::RosFastRtps
            | Self::RosCyclone
            // CORBA has no `@bit_bound` at all: an enumeration is an
            // unsigned long, CORBA 3.3 part 2 §9.3.2.5, and
            // `wireprobe-corba-idl` refuses a type that carries the
            // annotation before any ORB sees it.
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // `opensplice-idl.py` refuses `@bit_bound` below 32 by name
            // and drops it at 32, the width every OpenSplice enum has;
            // no recorded row separates a declared holder from one the
            // front end dropped.
            | Self::OpenSplice => EnumHolderWidth::Declared,
        }
    }

    /// Whether a case label of a narrow signed discriminator can be
    /// negative.
    ///
    /// [`UnionLabelMatch::NegativeLabelUnmatchedBelow32`] is
    /// **[`Self::Cyclone`]'s**, and it is in that dialect because it is
    /// what Cyclone puts on the wire. It drops the selected branch, so
    /// a conforming reader of the frame takes the member *after* the
    /// union for the branch — which is precisely what a peer talking to
    /// Cyclone has to expect, and what this dialect is a name for.
    ///
    /// Pinned by 24 recorded rows, both encoding versions and both byte
    /// orders. `union-discriminator-short`, XCDR1: Cyclone writes
    /// `c7cf0000 ffffffff` where RTI, Fast DDS and OpenDDS write
    /// `c7cf0000 7929edff ffffffff`. `f3` read as 243 is not `-13` and
    /// `cfc7` read as 53191 is not `-12345`; a negative label on a
    /// 32-bit discriminator matches and an unsigned narrow one matches,
    /// only the pair does not.
    /// `tests/interop-corpus/union-discriminator-short/README.md` and
    /// `combined-everything/README.md` carry the runs.
    ///
    /// **[`Self::Cyclone`] is not [`Dialect::default`]**, so naming the
    /// dialect is what it takes to get this. It was the default once,
    /// and this setting is one of the two that made the reversal:
    /// a caller who named nothing got a frame with no branch in it. The
    /// default is [`Self::Specification`], which is
    /// [`UnionLabelMatch::SignExtended`], the clause. This setting
    /// leaves this dialect the day Cyclone matches the label, with the
    /// version it started in.
    ///
    /// `--union-labels negative-unmatched-below-32` reaches it as a
    /// single point.
    ///
    /// Written out for all ten rather than a catch-all arm, so that an
    /// eleventh dialect does not silently inherit an answer nobody
    /// measured for it.
    #[must_use]
    pub const fn union_label(self) -> UnionLabelMatch {
        match self {
            Self::Cyclone => UnionLabelMatch::NegativeLabelUnmatchedBelow32,
            Self::Rti
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            // Unreachable in either ROS column: no recorded row of
            // either carries a union at all.
            | Self::RosFastRtps
            | Self::RosCyclone
            // CORBA 3.3 part 2 §9.3.2.11 writes the discriminant and
            // then the selected member by the same rule, and no ORB
            // measured here departs from it.
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // Measured, not carried over: OpenSplice's own 2 rows of
            // `union-discriminator-short` and its unannotated twin —
            // the one byte order and encoding version it writes —
            // carry the branch, `c7cf 7929edff ffffffff`, the same
            // shape RTI, Fast DDS and OpenDDS write.
            | Self::OpenSplice => UnionLabelMatch::SignExtended,
            // Unpinned: `hddsgen` refuses a union outright, so no
            // recorded row of that column carries a case label. The
            // clause's answer.
            | Self::Hdds => UnionLabelMatch::SignExtended,
        }
    }

    /// Whether a delimited `map` omits its entry count — rule (15) of
    /// §7.4.3.5.3 as written, against §7.4.1.1.4's sequence
    /// equivalence.
    ///
    /// Measured on 2026-08-24, one implementation on each side:
    /// OpenDDS's rows carry the DHEADER and no count (`map-key-enum`,
    /// `map-value-struct` under XCDR2) and the count where the map is
    /// undelimited (`map-key-long`, primitive on both halves); Fast
    /// DDS's rows carry both. A `bool` for the reason
    /// `hashed_id_advances_the_counter` is one: `MapLengthPolicy`
    /// lives with the codec, not here, and `ProbeRequest::in_dialect`
    /// maps the fact onto it.
    #[must_use]
    pub const fn map_count_omitted_when_delimited(self) -> bool {
        match self {
            Self::OpenDds => true,
            Self::FastDds
            | Self::Cyclone
            | Self::Rti
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // `opensplice-idl.py` refuses `map<K, V>` by name; no
            // recorded row of this dialect reaches a `map` at all.
            | Self::OpenSplice => false,
            // `hddsgen` refuses a `map` too, and for the same reason
            // there is nothing to measure here.
            | Self::Hdds => false,
        }
    }

    /// Whether an array's element is flattened before the delimiter
    /// question. See [`ArrayElementFlattening`]; only OpenDDS stops at
    /// the alias, measured on `resolve-array-of-typedef-array`.
    #[must_use]
    pub const fn array_element_flattening(self) -> ArrayElementFlattening {
        match self {
            Self::OpenDds => ArrayElementFlattening::StopsAtAlias,
            Self::Cyclone
            | Self::FastDds
            | Self::Rti
            | Self::Specification
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OpenSplice
            // Unpinned: the one recorded row of that column that
            // reaches an alias at all, `recursive-typedef-sequence`,
            // is a `sequence`, and it separates the two values on the
            // row `resolve-array-of-typedef-array` names — which
            // `hddsgen` does not compile. The clause's answer.
            | Self::Hdds => ArrayElementFlattening::ThroughAliasesAndArrays,
        }
    }

    /// In what order a `map`'s entries go on the wire.
    ///
    /// [`MapEntryOrder::AscendingKey`] is **[`Self::FastDds`]'s**, and
    /// it is in that dialect because it is what Fast DDS puts on the
    /// wire. Judgement 3's threshold — two independent implementations
    /// make a *position on an axis* — is about whether the order is a
    /// reading of the clause, and it is not: §7.4.3.5.3 rules (14) to
    /// (16) give a map as a length and a run of pairs and say nothing
    /// about their order, so this is the property of the `std::map`
    /// `fastddsgen` chose. That makes it a compatibility setting, which
    /// is exactly what belongs in the dialect of the product that
    /// writes it.
    ///
    /// Pinned by 32 recorded rows: `map-key-long`, `map-key-char`,
    /// `map-key-ulonglong`, `map-key-boolean` and `map-key-string`,
    /// their unannotated twins, both byte orders, both versions.
    ///
    /// **[`Self::OpenDds`] joined on 2026-08-24**: its column now
    /// records `map` octets, and they are ascending by key under the
    /// key type's own value order — `map-key-long` puts −1234567 ahead
    /// of 1 ahead of 2147483647 (signed), `map-key-ulonglong` puts 7
    /// ahead of 16045690984833335998 (unsigned; a signed reading would
    /// reverse them). The property of the ordered container both code
    /// generators chose, two products independently — still a
    /// compatibility setting and not a reading of the clause, which
    /// says nothing about order. The remaining dialects take the
    /// declaration order and nothing measured says otherwise.
    ///
    /// `--map-entries ascending-key` reaches it as a single point.
    ///
    /// Written out for all ten rather than a catch-all arm, so that an
    /// eleventh dialect does not silently inherit an answer nobody
    /// measured for it.
    #[must_use]
    pub const fn map_entries(self) -> MapEntryOrder {
        match self {
            Self::FastDds | Self::OpenDds => MapEntryOrder::AscendingKey,
            Self::Cyclone
            | Self::Rti
            | Self::Specification
            // Unreachable in either ROS column: no ROS message has a
            // `map`.
            | Self::RosFastRtps
            | Self::RosCyclone
            // CORBA has no `map` at all: the IDL-to-C and IDL-to-Java
            // mappings this project's CORBA probes go through have no
            // such type, and `wireprobe-corba-idl` refuses one before
            // any ORB sees it.
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // `opensplice-idl.py` refuses `map<K, V>` by name.
            | Self::OpenSplice
            // `hddsgen` refuses a `map` as well, and no recorded row of
            // that column reaches one.
            | Self::Hdds => MapEntryOrder::Declaration,
        }
    }

    /// Which integer an enumerator serialises as.
    ///
    /// [`EnumeratorValue::Ordinal`] is **OpenDDS's**, and it is in
    /// [`Self::OpenDds`] for the reason
    /// [`EnumHolderWidth::BitBoundDroppedByFrontEnd`] is in it: a front
    /// end that drops `@value` still puts the ordinal on the wire, and
    /// naming the product is asking to talk to the product.
    ///
    /// Pinned by 40 recorded rows. `enum-bitbound-8-value` under
    /// OpenDDS, XCDR1 big endian, once the holder width is out of the
    /// way: `00000002 00000001 ab` where `@value(200)` and `@value(9)`
    /// were declared — the ordinals 2 and 1.
    ///
    /// **`tao_idl` drops `@value` too and [`Self::Tao`] does not take
    /// this**, which is a difference between a measurement and a
    /// reading: all three ORB columns record every `@value` case as
    /// `NOFORM` — `an enumerator with an explicit @value, where CORBA's
    /// is its ordinal position` —
    /// so no ORB row separates the two values. A dialect that took it
    /// there would be reproducing a front end nobody measured through
    /// this corpus.
    ///
    /// `--enumerator-value ordinal` reaches it as a single point.
    ///
    /// Written out for all ten rather than a catch-all arm, so that an
    /// eleventh dialect does not silently inherit an answer nobody
    /// measured for it.
    #[must_use]
    pub const fn enumerator_value(self) -> EnumeratorValue {
        match self {
            Self::OpenDds => EnumeratorValue::Ordinal,
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::Specification
            // Unreachable in either ROS column: `rosidl` has no
            // enumeration with an explicit value in the corpus.
            | Self::RosFastRtps
            | Self::RosCyclone
            // CORBA 3.3 part 2 §9.3.2.5 serialises an enumeration as an
            // `unsigned long` of the enumerator's own value, and
            // omniORB and JacORB both carry `@value` — `tao_idl` does
            // not, and TAO is reached by the setting rather than by its
            // dialect for the same reason OpenDDS is.
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // `opensplice-idl.py` refuses an enumerator's `@value` by
            // name; every OpenSplice enum is its ordinal in four
            // octets, which is `Declared`'s own answer where no
            // `@value` was given.
            | Self::OpenSplice
            // Unpinned for that column: `hddsgen` emits
            // `enum-value-gaps` as an error rather than as octets, so
            // no recorded row separates the two answers.
            | Self::Hdds => EnumeratorValue::Declared,
        }
    }

    /// How a `@mutable` union numbers its branches, and whether its
    /// XCDR1 parameter list is terminated.
    ///
    /// [`MutableUnionFraming::BranchesFromZero`] is **OpenDDS's**, and
    /// it is in [`Self::OpenDds`] although it reproduces a frame in
    /// which two parameters share member id 0 and nothing terminates
    /// the list — a frame no conforming reader can walk. That was the
    /// argument for keeping it out of every dialect, and it is the
    /// argument for putting it in this one: the frame is what an
    /// OpenDDS peer sends, and a preset that named the product and
    /// wrote something else would be a preset that cannot talk to it.
    ///
    /// Pinned by 8 recorded rows, `union-mutable` and its unannotated
    /// twin in both versions and both orders.
    ///
    /// `--union-framing branches-from-zero` reaches it as a single
    /// point.
    ///
    /// Written out for all ten rather than a catch-all arm, so that an
    /// eleventh dialect does not silently inherit an answer nobody
    /// measured for it.
    #[must_use]
    pub const fn union_framing(self) -> MutableUnionFraming {
        match self {
            Self::OpenDds => MutableUnionFraming::BranchesFromZero,
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::Specification
            // Unreachable: a ROS message is `@final`.
            | Self::RosFastRtps
            | Self::RosCyclone
            // CORBA 3.3 part 2 §9.3.2.11 has one union framing and no
            // member ids at all: an ORB writes the discriminant and the
            // selected member and nothing else.
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // Unreachable: `opensplice-idl.py` refuses `@mutable` by
            // name, so no OpenSplice union is ever framed as one.
            | Self::OpenSplice
            // Unreachable for that column too: every recorded row of
            // it writes a union as `@final`.
            | Self::Hdds => MutableUnionFraming::Specification,
        }
    }

    /// Which member ids fit a PL_CDR1 short parameter header.
    ///
    /// [`ShortPidRange::AllFourteenBits`] is **OpenDDS's**: it writes
    /// an id in the range §7.4.1.2.2 reserves in the short header,
    /// where Cyclone, RTI and Fast DDS were all measured writing the
    /// extended one. Pinned by 2 recorded rows under XCDR1.
    ///
    /// `--short-pid-range all-fourteen-bits` reaches it as a single
    /// point.
    ///
    /// Written out for all ten rather than a catch-all arm, so that an
    /// eleventh dialect does not silently inherit an answer nobody
    /// measured for it.
    #[must_use]
    pub const fn short_pid_range(self) -> ShortPidRange {
        match self {
            Self::OpenDds => ShortPidRange::AllFourteenBits,
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::Specification
            // Unreachable: a ROS message writes no parameter list.
            | Self::RosFastRtps
            | Self::RosCyclone
            // A CORBA ORB writes no parameter list at all.
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // Unreachable: OpenSplice writes no parameter list either,
            // for the same reason `pl_cdr1_headers` gives.
            | Self::OpenSplice
            // Unreachable for the same reason: no recorded row of that
            // column carries a parameter id.
            | Self::Hdds => ShortPidRange::UpToReserved,
        }
    }

    /// Whether a `fixed<P,S>`'s digit count comes from the declaration
    /// or from the value.
    ///
    /// [`FixedDigitCount::ValuePrecision`] is **[`Self::JacOrb`]'s**,
    /// and it is in that dialect because it is what JacORB puts on the
    /// wire: `BigDecimal.precision()` digits and never fewer than one.
    /// Pinned by 6 recorded rows — `fixed-basic`, `sequence-fixed`,
    /// `array-1d-fixed` and their twins, where JacORB writes
    /// `123456789c1dab` against omniORB's and this project's
    /// `123456789c000000001dab` — and they were the whole of JacORB's
    /// unmatched column.
    ///
    /// [`FixedDigitCount::ValueStringDigits`] is no dialect's. It is what
    /// ACE/TAO 2.5.24's `ACE_CDR::Fixed` writes through its own CDR
    /// operator (the 7 rows of `unspecified_constructs.md` §3), and no
    /// DDS product's serializer goes through that operator: OpenDDS 3.34
    /// writes `OpenDDS::FaceTypes::Fixed_T<P,S>`, `(P + 2) / 2` octets,
    /// the declared count — measured 2026-10-07 through the FACE mapping,
    /// `0.00` under `fixed<5,2>` as `00000c`, `7` as `00700c`, `0.12345`
    /// under `fixed<6,5>` as `0012345c`. Its `write_fixed(to_ace_fixed())`
    /// is the ValueWriter path (JSON and dynamic data), not the wire.
    ///
    /// `--fixed-digits value-precision` reaches JacORB's as a single
    /// point; `--fixed-digits value-string-digits` reaches ACE's.
    ///
    /// Written out for all ten rather than a catch-all arm, so that an
    /// eleventh dialect does not silently inherit an answer nobody
    /// measured for it.
    #[must_use]
    pub const fn fixed_digits(self) -> FixedDigitCount {
        match self {
            Self::JacOrb => FixedDigitCount::ValuePrecision,
            // `Fixed_T<P,S>::operator<<`, `(P + 2) / 2` octets; see the doc.
            Self::OpenDds => FixedDigitCount::Declared,
            // A `fixed` is CORBA's and no DDS implementation writes one:
            // XTypes 1.3 has no fixed-point type, and the DDS probes
            // refuse the construct. `tao_idl` refuses it too.
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            // `idlpp` itself refuses `fixed` outright.
            | Self::OpenSplice
            // XTypes 1.3 has no fixed-point type, and `hddsgen` refuses
            // the construct as well, so no recorded row of that column
            // asks the question.
            | Self::Hdds => FixedDigitCount::Declared,
        }
    }

    /// Whether a member's `must_understand` reaches the wire, in either
    /// encoding version.
    ///
    /// [`Dialect::Specification`] takes
    /// [`MustUnderstandPolicy::DeclaredAndKey`] and the three vendor
    /// dialects do not, which is the one place in this table where the
    /// specification's answer and every measured vendor's part company:
    /// §7.2.2.4.4.4.8 says a `@key` member always has the attribute, and
    /// Cyclone, RTI and Fast DDS were all measured writing
    /// `evolution-mutable-key`'s two key members with `M_FLAG` clear.
    /// OpenDDS writes them with it set.
    #[must_use]
    pub const fn must_understand(self) -> MustUnderstandPolicy {
        match self {
            Self::FastDds => MustUnderstandPolicy::Omitted,
            // OpenDDS is the one measured implementation that agrees
            // with the clause here, and it is pinned by its own XCDR2
            // EMHEADERs: `evolution-mutable-key`'s two key members come
            // out with `M_FLAG` set.
            Self::OpenDds => MustUnderstandPolicy::DeclaredAndKey,
            // The clause read whole: a `@key` member and the
            // discriminator of a `@mutable` union (§7.2.2.4.4.4.6 and
            // §7.2.2.4.4.4.8).
            Self::Specification => MustUnderstandPolicy::DeclaredKeyAndDiscriminator,
            Self::Cyclone | Self::Rti => MustUnderstandPolicy::Declared,
            // Unreachable under an ORB: the flag is a bit of an
            // EMHEADER or of a parameter id, both `@mutable` framing.
            // Unreachable in a ROS column for the same reason: every
            // message is `@final`.
            Self::OmniOrb | Self::Tao | Self::JacOrb | Self::RosFastRtps | Self::RosCyclone => {
                MustUnderstandPolicy::Declared
            }
            // Unreachable for the same reason: the flag lives in an
            // EMHEADER or a PL_CDR1 parameter id, and OpenSplice writes
            // neither — no encoding version 2, `@mutable` refused by
            // name. Takes the clause's value, [`Self::Specification`]'s,
            // rather than the ORBs' and the ROS columns' — none of
            // those measured this system, so nothing here argues for
            // one unreached answer over the other.
            Self::OpenSplice => MustUnderstandPolicy::DeclaredAndKey,
            // Unreachable and inert: `hddsgen` writes every aggregate as
            // `@final`, so no member header of any kind reaches the wire
            // and the flag has nowhere to be written. Takes the clause's
            // value.
            Self::Hdds => MustUnderstandPolicy::DeclaredAndKey,
        }
    }

    /// What an aggregate whose source said nothing is taken to be.
    #[must_use]
    pub const fn default_extensibility(self) -> Extensibility {
        match self {
            Self::Cyclone => Extensibility::Final,
            Self::Rti | Self::FastDds | Self::OpenDds | Self::Specification => {
                Extensibility::Appendable
            }
            // `opensplice-idl.py` drops `@final` and `@appendable`
            // alike — neither changes OpenSplice's XCDR1 bytes, which
            // carry no DHEADER whatever the extensibility says (rule
            // 29) — so no recorded row distinguishes this from `Final`.
            // Takes the clause's value.
            Self::OpenSplice => Extensibility::Appendable,
            // Every ROS message is a `@final` XCDR1 struct whatever its
            // `.idl` says: `rosidl` parses the annotation and drops it.
            // Under XCDR1 `@appendable` is byte-for-byte `@final`, so
            // no recorded row separates the two; `Final` is what the
            // type *is* to that front end, which is the reason the
            // three ORB presets take it too.
            Self::RosFastRtps | Self::RosCyclone => Extensibility::Final,
            // CORBA IDL has no extensibility annotation at all, so
            // every CORBA structure is an aggregate whose source said
            // nothing, and what an ORB writes for it is the members in
            // declaration order with no framing — the `@final` form.
            // Under XCDR1, which is the only encoding an ORB has,
            // `@appendable` is byte-for-byte `@final` (rule 29), so the
            // two would give the same bytes; `Final` is chosen because
            // it is what the type *is* and not only what the bytes are,
            // and because a plan that called it appendable would grow a
            // DHEADER the moment somebody asked one of these presets
            // for an XCDR2 plan it has no business answering.
            Self::OmniOrb | Self::Tao | Self::JacOrb => Extensibility::Final,
            // Measured, and it is the one place this answer is a
            // measurement rather than a reading: `hddsgen` emits
            // `struct_flags: IS_FINAL` for an unannotated structure,
            // where §7.3.1.2.1.8 makes it `@appendable`. The rows that
            // need it are the `-unannotated` twins of every
            // `@appendable` case — `struct-unannotated`,
            // `sequence-struct-unannotated`, `string-adjacent-widths-unannotated`.
            Self::Hdds => Extensibility::Final,
        }
    }

    /// Whether a `@hashid` member advances the sequential counter the
    /// next unannotated member is numbered from.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.1 says it twice and not the same way.
    /// One sentence numbers a member from "the most-recently specified
    /// ID (using the @id annotation …)", which leaves a hashed id out of
    /// the count; the next numbers it from "the member ID of the
    /// previous member of the same Aggregated Type plus one", which does
    /// not.
    ///
    /// Measured 2026-08-09,
    /// `crates/ast-layout/tests/fixtures/vendor_bytes.md` §7. For
    /// `@mutable struct Sample { @hashid long alpha; long b; }` Cyclone
    /// and Fast DDS give `b` the id `0343172D`, the hash plus one; RTI
    /// gives it `00000000`. With an `@id(10)` before the hashed member
    /// RTI gives `0000000B`, so its counter is not merely reset — a
    /// hashed id never advances it at all.
    ///
    /// Three of the four take the second reading, which is why it is the
    /// default. It changes the member id on the wire, and a peer that
    /// numbers differently reads the wrong member or none, so RTI's
    /// answer is a dialect and not a footnote.
    ///
    /// # Why this one is a bool and not a policy enum
    ///
    /// It is consumed by the **lowering** pass, in
    /// `zerodds_idl_lower::complete::member_ids`, not by the planner:
    /// member ids are fixed before a `LayoutKey` is ever built. A shared
    /// policy type would have to live in a crate both `zerodds-ast-layout`
    /// and `zerodds-idl-lower` can see, and neither depends on the other.
    /// The *fact* still lives here, once, beside the other seven, and
    /// `zerodds_idl_lower::Options` carries the boolean.
    #[must_use]
    pub const fn hashed_id_advances_the_counter(self) -> bool {
        match self {
            // OpenDDS numbers the next member from the last *declared*
            // id, the way RTI does. Pinned by its own `@hashid` rows.
            Self::Rti | Self::OpenDds => false,
            Self::Cyclone | Self::FastDds | Self::Specification => true,
            // Unreachable under an ORB: `@hashid` is an XTypes
            // annotation and CORBA IDL has no member ids at all.
            // Unreachable in a ROS column: `rosidl` drops the
            // annotation with the rest of them.
            Self::OmniOrb | Self::Tao | Self::JacOrb | Self::RosFastRtps | Self::RosCyclone => true,
            // Unreachable: `idlpp` never sees `@hashid` at all —
            // `opensplice-idl.py` strips it, along with `@id` and
            // `@autoid`, before the file reaches the front end, because
            // none of the three changes OpenSplice's XCDR1 bytes (its
            // members are positional).
            Self::OpenSplice => true,
            // Unpinned: every recorded row of that column writes its
            // members in declaration order, which both readings give.
            Self::Hdds => true,
        }
    }

    /// Whether a declaration may carry the name of the scope it is
    /// written in — `module common { struct common { … }; }`.
    ///
    /// IDL 4.2 §7.5.2 does not admit it: "The name of a module,
    /// structure, union, map, interface, value type, event type,
    /// exception or home may not be redefined within the immediate
    /// scope of" that same construct. The products split on it, and the
    /// split is measured by the corpus case
    /// `resolve-module-named-as-type` in
    /// `tests/interop-results/ledger.tsv`, each with its own words:
    ///
    /// * Cyclone DDS — `Declaration 'common' collides with an earlier
    ///   declaration of 'common'`
    /// * OpenDDS — `Error - tao_idl: illegal redefinition "common" and
    ///   "common"`, and TAO, omniORB and JacORB refuse it likewise
    /// * Fast DDS and RTI Connext take it and write octets
    /// * `rosidl` takes it too, under either middleware, because it
    ///   runs its own front end rather than the vendor's
    ///
    /// A `bool` for the reason the two above are: the lowering pass
    /// consumes it, before any `LayoutKey` exists.
    /// `zerodds_idl_lower::NameOfTheEnclosingScope` carries the policy.
    ///
    /// A reader that refuses the file can never reproduce the octets the
    /// other four wrote — which is what
    /// `tools/interop-results/tests/dialect_completeness.rs` measures,
    /// and what this axis answers.
    #[must_use]
    pub const fn admits_the_name_of_the_enclosing_scope(self) -> bool {
        match self {
            // Measured: `resolve-module-named-as-type` has an HDDS row
            // in both encoding versions — `01000000` — so `hddsgen`
            // accepts the file where §7.5.2 refuses it.
            Self::Rti | Self::FastDds | Self::RosFastRtps | Self::RosCyclone | Self::Hdds => true,
            Self::Cyclone
            | Self::OpenDds
            | Self::Specification
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb => false,
            // Takes the clause's value: this project's own front end
            // refuses `resolve-module-named-as-type` for OpenSplice
            // before `idlpp` ever sees the file, so the corpus row is
            // this axis's answer and not an independent reading of
            // `idlpp`'s grammar.
            Self::OpenSplice => false,
        }
    }

    /// Whether a member-id hash reads an escaped name as written —
    /// `_long` with its underscore — instead of the identifier the
    /// escape spells.
    ///
    /// IDL 4.2 §7.2.3.2 makes the underscore "a purely lexical
    /// convention" that is not part of the name, so the specification
    /// hashes `long`. RTI hashes the spelling, keyword or not: measured
    /// with rtiddsgen 7.7.0 on `@hashid long _long;` and
    /// `@hashid long _foo;`, whose TypeCode member ids are
    /// 79659193 = hash("_long") and 230782087 = hash("_foo") where the
    /// clause gives 56906255 and 186170796 — rtiddsgen's generated C
    /// even names the fields `_long` and `_foo`. It changes the member
    /// id on the wire, so it is a dialect and not a footnote.
    ///
    /// A `bool` for the reason `hashed_id_advances_the_counter` is one:
    /// consumed by the lowering pass, before any `LayoutKey` exists.
    /// `zerodds_idl_lower::HashedNameSpelling` carries the policy.
    #[must_use]
    pub const fn hashid_reads_the_escaped_spelling(self) -> bool {
        match self {
            Self::Rti => true,
            // Measured stripped for Cyclone and Fast DDS by the
            // `identifier-escaped-keyword` cells; OpenDDS numbers as
            // the clause reads. Unreachable under an ORB or a ROS
            // column, which drop the annotation.
            Self::Cyclone
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            | Self::RosFastRtps
            | Self::RosCyclone
            // Unreachable: member ids do not exist on an OpenSplice
            // wire at all.
            | Self::OpenSplice
            // Unpinned: `identifier-escaped-keyword` is not one of that
            // column's rows, so nothing separates the two readings.
            | Self::Hdds => false,
        }
    }

    /// Whether a key holder is forced to `@final`, which for RTI
    /// depends on the encoding version.
    #[must_use]
    pub const fn key_holder_extensibility(self, encoding: Encoding) -> KeyHolderExtensibility {
        match self {
            Self::Rti if encoding.is_v1() => KeyHolderExtensibility::AsDeclared,
            // Unreachable under an ORB: a KeyHash is DDSI-RTPS §9.6.3.8
            // and CORBA has no instances, no keys and no discovery.
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // Not measured — OpenSplice is a real DDS system with keys
            // and discovery, unlike the three ORBs, but no recorded row
            // reaches a key holder. Takes the clause's value.
            | Self::OpenSplice
            // Not measured either: every recorded row of that column is
            // a payload, and a key holder is reached only by a
            // KeyHash.
            | Self::Hdds => KeyHolderExtensibility::ForcedFinal,
        }
    }

    /// How deep Step 1's member removal reaches, which for RTI depends
    /// on the encoding version.
    #[must_use]
    pub const fn key_holder_nesting(self, encoding: Encoding) -> KeyHolderNesting {
        match self {
            Self::Rti if encoding.is_v1() => KeyHolderNesting::KeylessNestedWhole,
            // Unreachable under an ORB, as `key_holder_extensibility`.
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // Not measured, as `key_holder_extensibility`.
            | Self::OpenSplice
            // Not measured, as `key_holder_extensibility`.
            | Self::Hdds => KeyHolderNesting::Recursive,
        }
    }

    /// In what order a key holder's members are serialized, which for
    /// RTI depends on the encoding version.
    #[must_use]
    pub const fn key_holder_order(self, encoding: Encoding) -> KeyHolderOrder {
        match self {
            Self::Rti if encoding.is_v1() => KeyHolderOrder::Declaration,
            // Unreachable under an ORB, as `key_holder_extensibility`.
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // Not measured, as `key_holder_extensibility`.
            | Self::OpenSplice
            // Not measured, as `key_holder_extensibility`.
            | Self::Hdds => KeyHolderOrder::AscendingMemberId,
        }
    }

    /// How a key holder's members are aligned, which for Cyclone and RTI
    /// depends on the encoding version. See [`KeyHolderAlignment`] for the
    /// measurement.
    #[must_use]
    pub const fn key_holder_alignment(self, encoding: Encoding) -> KeyHolderAlignment {
        match self {
            Self::OpenSplice => KeyHolderAlignment::Packed,
            Self::Cyclone | Self::Rti if encoding.is_v1() => KeyHolderAlignment::DataEncoding,
            // Fast DDS measured as the clause; OpenDDS not measured and
            // takes the clause's value. The ORBs and the ROS type supports
            // never reach a key holder.
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // Not measured, as `key_holder_extensibility`.
            | Self::Hdds => KeyHolderAlignment::Aligned,
        }
    }

    /// What becomes of a union's member when `@try_construct(use_default)`
    /// defaulted its discriminator.
    ///
    /// Every dialect answers [`DefaultedDiscriminatorMember::TypeDefault`],
    /// and the twelve arms are written out rather than folded together
    /// so that a thirteenth does not inherit an answer nobody looked
    /// for.
    ///
    /// - `Rti` and `OpenDds` are measurements: each hands back the
    ///   member's type default, recorded in
    ///   [`DefaultedDiscriminatorMember`]'s table.
    /// - `FastDds` cannot reach the question — it refuses an annotation
    ///   on a union discriminator — so it takes the clause's value and
    ///   claims no measurement.
    /// - `Cyclone` and the two ROS type supports have no recorded run of
    ///   this shape either; the clause's value, and nothing measured.
    /// - The three ORBs have no `@try_construct` at all: CORBA CDR
    ///   knows no such annotation, and a union whose discriminator
    ///   value has no case is an `INV_OBJREF` there, not an evolution.
    /// - `OpenSplice` has neither: `opensplice-idl.py` refuses
    ///   `@try_construct` by name. The clause's value.
    /// - `Hdds` has not been asked: `annotation-try-construct` is one of
    ///   the cases whose generated code does not compile. The clause's
    ///   value.
    #[must_use]
    pub const fn defaulted_discriminator_member(self) -> DefaultedDiscriminatorMember {
        match self {
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            | Self::OpenSplice
            | Self::Hdds => DefaultedDiscriminatorMember::TypeDefault,
        }
    }

    /// Whether this dialect writes the RTI Connext/CoreDX minimal
    /// `TypeObject` camp — one trailing octet inside each aggregated
    /// member's `DHEADER`, counted into the byte stream the minimal
    /// `EquivalenceHash` of XTypes 1.3 §7.3.4.5 is taken over.
    ///
    /// `docs/omg-xtypes-issues.md` §C2 measures the split. Two camps
    /// ship and neither stream is malformed; the clause does not decide
    /// which one the hash covers, so this is a dialect choice and not a
    /// defect in either sender.
    ///
    /// - `Rti` and `CoreDx` (CoreDX rides the `Rti` arm; the interop
    ///   matrix records the two computing the same minimal hash) write
    ///   the octet — measured `Cyclone\u{2194}RTI` and `ZeroDDS\u{2194}RTI`
    ///   on `struct_key_enum_1`/`struct_key_struct_1`.
    /// - `Cyclone`, `FastDds` and `Specification` do not: Cyclone and
    ///   Fast DDS agree with each other and are the default here, and
    ///   `crates/types/tests/typeobject_vendor_dheader.rs` records their
    ///   octets.
    /// - The ORBs and ROS type supports have no minimal `TypeObject`
    ///   (CORBA CDR and the ROS introspection paths carry none), so
    ///   they take the default `false` and claim no measurement.
    #[must_use]
    pub const fn minimal_member_trailing_octet(self) -> bool {
        match self {
            Self::Rti => true,
            Self::Cyclone
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // OpenSplice has no minimal `TypeObject` either — it has no
            // XTypes at all. The default `false`, and no measurement.
            | Self::OpenSplice
            // `tools/wireprobe-hdds` compiles `hddsgen`'s own output
            // verbatim and never reaches a `TypeObject`, so nothing in
            // that column measures this.
            | Self::Hdds => false,
        }
    }

    /// How a `wstring`'s length is counted, which for RTI depends on
    /// the encoding version.
    #[must_use]
    pub const fn wstring_length(self, encoding: Encoding) -> WstringLengthPolicy {
        match self {
            Self::Cyclone | Self::Specification => WstringLengthPolicy::SPECIFICATION,
            Self::FastDds => WstringLengthPolicy::CHARACTER_COUNT,
            // Measured on every `wstring` row OpenDDS wrote: octets, no
            // terminator, two-octet units, which is §7.4.1.1.2's own
            // form.
            Self::OpenDds => WstringLengthPolicy::SPECIFICATION,
            // The cell no name in the old five-value enumeration could
            // reach: a count of **characters** over **four**-octet
            // units. `wstring-unbounded-utf8` under either type
            // support is `07000000` and then seven four-octet units.
            Self::RosFastRtps | Self::RosCyclone => WstringLengthPolicy::ROS_TYPE_SUPPORT,
            // Measured 2026-08-12 at the member and the collection
            // element alike: RTI's XCDR1 `wstring` is CORBA §9.3.1.6's
            // GIOP 1.1 form byte for byte, and needs no RTI-specific
            // rule anywhere.
            Self::Rti if encoding.is_v1() => WstringLengthPolicy::CHARACTER_COUNT_TERMINATED,
            Self::Rti => WstringLengthPolicy::SPECIFICATION,
            // GIOP 1.2's octet count in all three, over a two-octet
            // transmission code set in omniORB and JacORB and a
            // four-octet one in TAO. The width rides on this setting
            // because it rides on it for the scalar `wchar` too.
            Self::OmniOrb | Self::JacOrb => WstringLengthPolicy::SPECIFICATION,
            Self::Tao => WstringLengthPolicy::SPECIFICATION_WIDE_UNITS,
            // Unreachable: `idlpp` itself refuses `wstring` outright.
            Self::OpenSplice => WstringLengthPolicy::SPECIFICATION,
            // Measured, and only the cell's third coordinate is live.
            // `wstring-member` declares `04 00 00 00` for `w = "abc"`
            // and then three UTF-8 octets and a NUL, so the count is of
            // octets and the terminator is inside it — the cell
            // §7.4.3.5.3 rule (3) gives a narrow `string` and rule (4)
            // forbids for a `wstring`. The unit width is not read at
            // all under [`WstringCodeUnits::Utf8`], which is the axis
            // that has the octets.
            Self::Hdds => WstringLengthPolicy::new(
                WstringLengthCounts::Octets,
                WstringTerminator::Present,
                WstringUnitWidth::Two,
            ),
        }
    }

    /// How a `wchar` is framed.
    ///
    /// The setting that splits the five DDS implementations from the
    /// three ORBs: every DDS implementation writes XTypes Table 31's
    /// fixed-width code unit and every ORB writes CORBA §9.3.1.6's
    /// GIOP 1.2 length-prefixed one for a *member*. The three ORBs then
    /// part on the collection element — omniORB and JacORB keep the
    /// count octet, TAO drops it — which is why TAO's answer is
    /// [`WcharFraming::GiopLengthPrefixedMemberOnly`] and not the same
    /// value as the other two. [`WcharFraming`] has the runs.
    #[must_use]
    pub const fn wchar_framing(self) -> WcharFraming {
        match self {
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::OpenDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            // Unreachable: `idlpp` itself refuses `wchar` outright.
            | Self::OpenSplice
            // Unpinned: no recorded row of that column has a scalar
            // `wchar` of its own — `char-and-wchar` is the one, and
            // `hddsgen` maps IDL `char` and `wchar` alike to a Rust
            // `char`, four octets either way. Table 31's answer.
            | Self::Hdds => WcharFraming::Fixed,
            Self::OmniOrb | Self::JacOrb => WcharFraming::GiopLengthPrefixed,
            Self::Tao => WcharFraming::GiopLengthPrefixedMemberOnly,
        }
    }

    /// How wide a scalar `wchar` is.
    ///
    /// Eight of the nine give the same answer here as they give for a
    /// `wstring`'s code units, which is why this was one setting until
    /// the ninth was fitted: `rmw_cyclonedds_cpp` writes `e400 3412`
    /// for `primitive-wchar` and four-octet units inside a `wstring`.
    /// [`WcharWidth`] carries the octets.
    ///
    /// RTI is the reason the encoding is an argument: `61000000` under
    /// XCDR1 and `6100` under XCDR2, the same split its `wstring` takes.
    #[must_use]
    pub const fn wchar_width(self, encoding: Encoding) -> WcharWidth {
        match self {
            Self::Rti if encoding.is_v1() => WcharWidth::Four,
            // Table 31's two octets, and the value every DDS
            // implementation but RTI-under-XCDR1 was measured writing.
            // Unreachable: `idlpp` itself refuses `wchar` outright.
            Self::Cyclone | Self::Rti | Self::FastDds | Self::Specification | Self::OpenSplice => {
                WcharWidth::Two
            }
            // Measured, and the pair the axis exists for.
            // `primitive-wchar` under XCDR1 little endian:
            // `rosidl_typesupport_fastrtps_c` writes `e400000034120000`
            // and `rmw_cyclonedds_cpp` writes `e4003412`, while both
            // put four-octet units inside a `wstring`.
            Self::RosFastRtps => WcharWidth::Four,
            // Measured: `char-and-wchar` is
            // `61 00 00 00 7a 00 00 00` in **both** encoding versions —
            // `char c = 'a'` and then `wchar w = 'z'` in four octets
            // each. `hddsgen` renders both IDL types as a Rust `char`,
            // which is a four-octet scalar; see
            // `docs/edge-case-map.md`, "What the first comparison
            // showed".
            Self::Hdds => WcharWidth::Four,
            Self::OpenDds | Self::RosCyclone => WcharWidth::Two,
            // The transmission code set's width, which the ORBs also
            // put in the GIOP 1.2 count octet: omniORB and JacORB write
            // `02 00e4`, TAO `04 e4000000`.
            Self::OmniOrb | Self::JacOrb => WcharWidth::Two,
            Self::Tao => WcharWidth::Four,
        }
    }

    /// Which of a `map`'s halves decides whether the map is delimited.
    ///
    /// [`MapDelimiter`] carries the twelve Fast DDS rows that separate
    /// the two readings. Every other measured implementation refuses a
    /// `map` before it reaches a wire, so the clause's reading is what
    /// they take.
    #[must_use]
    pub const fn map_delimiter(self) -> MapDelimiter {
        match self {
            Self::FastDds => MapDelimiter::ValueOnly,
            // Rule (14) read as written. None of these produces a
            // `map`'s octets at all — `docs/refused-constructs.md`,
            // "`map` in the C emitters" — so this is the clause's
            // answer and not a measurement, and it is written out for
            // each rather than folded into one arm so that a dialect
            // added later does not inherit an answer nobody took.
            Self::Cyclone
            | Self::Rti
            | Self::OpenDds
            | Self::Specification
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            // `opensplice-idl.py` refuses `map<K, V>` by name.
            | Self::OpenSplice
            // `hddsgen` refuses a `map` as well.
            | Self::Hdds => MapDelimiter::EitherHalf,
        }
    }

    /// In what byte order a `wstring`'s code units go out.
    ///
    /// [`WstringUnitOrder`] has the runs, including which of the three
    /// ORB answers is measured in the write direction and which is
    /// carried over from the read direction.
    #[must_use]
    pub const fn wstring_unit_order(self) -> WstringUnitOrder {
        match self {
            // Cyclone's big-endian units are unswapped, and this
            // preset reproduces that because it is what Cyclone puts on
            // the wire. `"ab"` big endian leaves Cyclone as
            // `00000004 61006200` where RTI and Fast DDS write
            // `00000004 00610062`: the length word swaps and the
            // content does not, so read as the big-endian stream it is
            // labelled, Cyclone's `"ab"` is U+6100 U+6200.
            // `tests/fixtures/vendor_bytes.md` §4 has the run, and 12
            // recorded rows pin it.
            //
            // This preset used to write the stream's order here, on the
            // reading that a default must not be a form a conforming
            // reader mis-decodes — which was true of the reading and
            // false of the place it was applied: it made
            // `--dialect cyclone` unable to exchange a big-endian
            // `wstring` with Cyclone at all. The preset takes the
            // product's order; the *default* is what carries the
            // reading, and `Self::Specification` is `Dialect::default`
            // now, which is `WstringUnitOrder::Stream`. Naming the
            // product is asking to talk to the product. It leaves this
            // dialect the day Cyclone swaps the units, with the version
            // it started in.
            Self::RosFastRtps => WstringUnitOrder::HostUnswapped,
            Self::Cyclone => WstringUnitOrder::HostUnswapped,
            // Unreachable: `idlpp` itself refuses `wstring` outright.
            // `Hdds` is unreachable here for the same axis rather than
            // for the front end: its `wstring` is UTF-8 octets, which
            // `WstringCodeUnits` carries and this order does not reach.
            Self::Rti | Self::FastDds | Self::OpenDds | Self::Specification | Self::OpenSplice => {
                WstringUnitOrder::Stream
            }
            // Not separable from `HostUnswapped` by any recorded row:
            // `rmw_cyclonedds_cpp` writes the host's byte order and
            // takes no argument for it, so every row it has is little
            // endian on a little-endian host and the two answers
            // coincide. `Stream` is the reading of the two that matches
            // what the type support does — on a big-endian host it
            // would produce a big-endian stream with big-endian units —
            // and it is written down here rather than left to look
            // measured.
            Self::RosCyclone => WstringUnitOrder::Stream,
            Self::OmniOrb => WstringUnitOrder::ByteOrderMark,
            // TAO's write direction is host-order only here, so `Stream`
            // and a fixed order cannot be told apart in it. `Stream` is
            // the safe reading of the two: it is what the clause says,
            // it is byte-identical to what TAO was measured writing on
            // this little-endian host, and it is the one that does not
            // claim a big-endian measurement nobody has.
            Self::Tao => WstringUnitOrder::Stream,
            Self::JacOrb => WstringUnitOrder::BigEndianAlways,
            // Unpinned, and inert: under [`WstringCodeUnits::Utf8`] the
            // content is UTF-8, which has no byte order of its own, so
            // the writer and the reader never consult it.
            Self::Hdds => WstringUnitOrder::Stream,
        }
    }

    /// Whether the elements of a collection carry alignment padding
    /// between them.
    ///
    /// [`ElementPadding::Packed`] is **[`Self::Hdds`]'s** and is
    /// measured on `array-1d-struct-final` and `sequence-struct-final`,
    /// which name the departure in their own octets. Every other dialect
    /// answers the clause, and nothing else in this table has been
    /// measured to differ.
    #[must_use]
    pub const fn element_padding(self) -> ElementPadding {
        match self {
            Self::Hdds => ElementPadding::Packed,
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::Specification
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            | Self::OpenDds
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OpenSplice => ElementPadding::Aligned,
        }
    }

    /// Whether an aggregate's declared extensibility reaches the wire.
    ///
    /// [`AggregateDelimiter::Never`] is **[`Self::Hdds`]'s** and is
    /// measured at three of the four positions an aggregate can be
    /// reached in — the sample itself, a struct member, and a `@mutable`
    /// type's member framing. The fourth, a `sequence` element, is
    /// [`Self::sequence_element_delimiter`]'s and answers the opposite,
    /// which is why it is a separate axis and not a third value here.
    ///
    /// **Under the plain and delimited encodings only.** Every recorded
    /// HDDS row is `plain-cdr1` or `plain-cdr2`; no HDDS bytes exist
    /// under PL_CDR1 or PL_CDR2, where the encapsulation identifier states
    /// a parameter list and a reader takes the framing from it. There the
    /// dialect writes and reads the specification's framing: measured on
    /// the Linux test host, `Never` under PL_CDR1 refused Cyclone's `@mutable` stream
    /// in `dialect_roundtrip.rs` and wrote a parameter list with no
    /// parameter headers in `silent_drop.rs`.
    #[must_use]
    pub const fn aggregate_delimiter(self, encoding: Encoding) -> AggregateDelimiter {
        match self {
            Self::Hdds if matches!(encoding, Encoding::PlCdr1 | Encoding::PlCdr2) => {
                AggregateDelimiter::Specification
            }
            Self::Hdds => AggregateDelimiter::Never,
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::Specification
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            | Self::OpenDds
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OpenSplice => AggregateDelimiter::Specification,
        }
    }

    /// Whether a `sequence` element of a non-`@final` type is preceded
    /// by its own octet length.
    ///
    /// [`SequenceElementDelimiter::BeforeEachNonFinalElement`] is
    /// **[`Self::Hdds`]'s**, measured on `sequence-struct-appendable`,
    /// `empty-struct-in-sequence`, `resolve-forward-declared`,
    /// `recursive-mutual` and `recursive-self-sequence`.
    #[must_use]
    pub const fn sequence_element_delimiter(self) -> SequenceElementDelimiter {
        match self {
            Self::Hdds => SequenceElementDelimiter::BeforeEachNonFinalElement,
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::Specification
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            | Self::OpenDds
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OpenSplice => SequenceElementDelimiter::Never,
        }
    }

    /// Which octets stand for a `wstring`'s characters.
    ///
    /// [`WstringCodeUnits::Utf8`] is **[`Self::Hdds`]'s** and is
    /// measured on `wstring-member` and `wstring-unbounded-utf8`. It is
    /// not the same question as [`Self::wstring_length`]: that one says
    /// what the length counts, this one says what the content is made
    /// of, and the two part because a run of UTF-8 octets has no
    /// fixed-width code unit for the first to count.
    #[must_use]
    pub const fn wstring_code_units(self) -> WstringCodeUnits {
        match self {
            Self::Hdds => WstringCodeUnits::Utf8,
            Self::Cyclone
            | Self::Rti
            | Self::FastDds
            | Self::Specification
            | Self::OmniOrb
            | Self::Tao
            | Self::JacOrb
            | Self::OpenDds
            | Self::RosFastRtps
            | Self::RosCyclone
            | Self::OpenSplice => WstringCodeUnits::Utf16,
        }
    }
}

/// What a layout is computed for.
///
/// Member offsets are invariant under endianness — byte order changes
/// the bytes, not where they sit — so the cache shares offset
/// computation between the two and consults the endianness only where it
/// genuinely matters: the big-endian key holder and per-primitive byte
/// order. Spec §7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayoutKey {
    /// Which encoding.
    pub encoding: Encoding,
    /// Which byte order.
    pub endianness: Endianness,
    /// Data or key holder.
    pub purpose: Purpose,
    /// How variable-length mutable members are framed.
    pub length_codes: LengthCodePolicy,
    /// Which collections carry a delimiter under XCDR2.
    pub collection_delimiters: CollectionDelimiterPolicy,
    /// Which of a `map`'s two halves decides whether the map is
    /// delimited.
    pub map_delimiter: MapDelimiter,
    /// How a `wstring`'s length is counted.
    pub wstring_length: WstringLengthPolicy,
    /// How a scalar `wchar` is framed.
    pub wchar_framing: WcharFraming,
    /// How wide a scalar `wchar` is, which is not the width of a
    /// `wstring`'s code units: `rmw_cyclonedds_cpp` writes two here and
    /// four there.
    pub wchar_width: WcharWidth,
    /// In what byte order a `wstring`'s code units go out.
    pub wstring_unit_order: WstringUnitOrder,
    /// How a `@mutable` member is framed under XCDR1.
    pub pl_cdr1_headers: PlCdr1HeaderPolicy,
    /// What flags a PL_CDR1 parameter id carries, per position.
    pub pid_flags: PidFlagPolicy,
    /// How tolerant a PL_CDR1 reader is of another framing.
    pub pl_cdr1_reading: PlCdr1Reading,
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
    /// question. See [`ArrayElementFlattening`].
    pub array_element_flattening: ArrayElementFlattening,
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
    /// Whether the elements of a collection carry alignment padding.
    pub element_padding: ElementPadding,
    /// Whether an aggregate's declared extensibility reaches the wire.
    pub aggregate_delimiter: AggregateDelimiter,
    /// Whether a `sequence` element of a non-`@final` type is preceded
    /// by its own octet length.
    pub sequence_element_delimiter: SequenceElementDelimiter,
    /// Which octets stand for a `wstring`'s characters.
    pub wstring_code_units: WstringCodeUnits,
}

/// How a PL_CDR1 parameter list is framed beyond the header form the plan
/// carries: the fields of `zerodds_wire::PlCdr1Framing`, which this crate
/// does not depend on and so cannot name.
///
/// [`LayoutKey::pl_cdr1_framing`] is the one place they are computed from a
/// key. `zerodds-wire-codec` sets its reader and writer from it, and the
/// `DdsType` glue `zerodds-build` generates spells it into the reader and
/// writer it constructs. That glue used to construct both with their
/// defaults whatever dialect the plan was built in, so a codec built for
/// Fast DDS still refused the bare `0x3f02` sentinel Fast DDS writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlCdr1FramingValues {
    /// The flags on member ids, the sentinel and the extended marker.
    pub pid_flags: PidFlagPolicy,
    /// The highest member id the short header carries.
    pub highest_short_id: u16,
    /// Whether a parameter's declared length is rounded up to four.
    pub pad_declared_length: bool,
    /// The same for an `@optional` member's parameter.
    pub pad_declared_optional_length: bool,
    /// Whether a member that declares must-understand sets the flag.
    pub member_must_understand: bool,
    /// Whether every parameter uses the extended header.
    pub require_extended: bool,
    /// Whether padding follows a value inside its parameter.
    pub pad_after_value: bool,
}

impl LayoutKey {
    /// The PL_CDR1 framing this key's writer and reader use.
    ///
    /// The header *form* is in the plan, because it changes a member's
    /// extent; these values do not, so they ride on the writer and the
    /// reader, and both take them from here.
    #[must_use]
    pub const fn pl_cdr1_framing(&self) -> PlCdr1FramingValues {
        PlCdr1FramingValues {
            pid_flags: self.pid_flags,
            highest_short_id: self.short_pid_range.highest_short_id(),
            pad_declared_length: self.pl_cdr1_headers.pads_declared_length(),
            pad_declared_optional_length: self.pl_cdr1_headers.pads_declared_optional_length(),
            member_must_understand: self.must_understand.writes_flag()
                && !matches!(self.pid_flags, PidFlagPolicy::ImplExtensionOnMembers),
            require_extended: self.pl_cdr1_headers.always_extended(),
            pad_after_value: self.pl_cdr1_headers.pads_after_value(),
        }
    }

    /// A data plan in the given encoding and byte order, in the default
    /// dialect.
    ///
    /// The dialect is [`Dialect::Specification`], so every wire question
    /// the implementations answer differently is answered the way
    /// XTypes 1.3 answers it. [`LayoutKey::with_dialect`] selects
    /// another; each `with_*` method below overrides one setting of
    /// whichever dialect is in force, and overrides it whether or not a
    /// dialect was named.
    ///
    /// The variant is named rather than taken from [`Dialect::default`]
    /// because this is a `const fn` and `Default::default` is not one.
    /// `zerodds_ast_layout::key::the_default_dialect_is_one_fact` holds
    /// the three spellings — `#[default]`, [`Dialect::is_default`] and
    /// this call — to one value, so a change to one of them that misses
    /// the others fails rather than diverging quietly.
    #[must_use]
    pub const fn data(encoding: Encoding, endianness: Endianness) -> Self {
        Self::in_dialect(Dialect::Specification, encoding, endianness, Purpose::Data)
    }

    /// Every policy of one dialect, for one encoding, byte order and
    /// purpose.
    ///
    /// The encoding is an argument because one setting depends on it:
    /// RTI counts a `wstring`'s length differently under the two
    /// versions.
    #[must_use]
    pub const fn in_dialect(
        dialect: Dialect,
        encoding: Encoding,
        endianness: Endianness,
        purpose: Purpose,
    ) -> Self {
        Self {
            encoding,
            endianness,
            purpose,
            length_codes: dialect.length_codes(),
            collection_delimiters: dialect.collection_delimiters(),
            wstring_length: dialect.wstring_length(encoding),
            wchar_framing: dialect.wchar_framing(),
            wchar_width: dialect.wchar_width(encoding),
            map_delimiter: dialect.map_delimiter(),
            wstring_unit_order: dialect.wstring_unit_order(),
            pl_cdr1_headers: dialect.pl_cdr1_headers(),
            pid_flags: dialect.pid_flags(),
            pl_cdr1_reading: PlCdr1Reading::Strict,
            must_understand: dialect.must_understand(),
            enum_holder: dialect.enum_holder(),
            union_label: dialect.union_label(),
            map_entries: dialect.map_entries(),
            array_element_flattening: dialect.array_element_flattening(),
            enumerator_value: dialect.enumerator_value(),
            union_framing: dialect.union_framing(),
            short_pid_range: dialect.short_pid_range(),
            fixed_digits: dialect.fixed_digits(),
            default_extensibility: dialect.default_extensibility(),
            key_holder: dialect.key_holder_extensibility(encoding),
            key_holder_nesting: dialect.key_holder_nesting(encoding),
            key_holder_order: dialect.key_holder_order(encoding),
            key_holder_alignment: dialect.key_holder_alignment(encoding),
            defaulted_discriminator_member: dialect.defaulted_discriminator_member(),
            element_padding: dialect.element_padding(),
            aggregate_delimiter: dialect.aggregate_delimiter(encoding),
            sequence_element_delimiter: dialect.sequence_element_delimiter(),
            wstring_code_units: dialect.wstring_code_units(),
        }
    }

    /// The same plan with every setting taken from another dialect.
    ///
    /// This resets every one of them: it is a preset and not a patch,
    /// so applying it after a `with_*` call discards that call. The
    /// command-line probes apply the dialect first for that reason.
    #[must_use]
    pub const fn with_dialect(self, dialect: Dialect) -> Self {
        Self::in_dialect(dialect, self.encoding, self.endianness, self.purpose)
    }

    /// The same plan with a different length-code policy.
    #[must_use]
    pub const fn with_length_codes(mut self, policy: LengthCodePolicy) -> Self {
        self.length_codes = policy;
        self
    }

    /// The same plan with a different collection-delimiter policy.
    #[must_use]
    pub const fn with_collection_delimiters(mut self, policy: CollectionDelimiterPolicy) -> Self {
        self.collection_delimiters = policy;
        self
    }

    /// The same plan with the elements of a collection packed against
    /// one another or aligned as §7.4.1 says.
    #[must_use]
    pub const fn with_element_padding(mut self, padding: ElementPadding) -> Self {
        self.element_padding = padding;
        self
    }

    /// The same plan with an aggregate's declared extensibility reaching
    /// the wire or not.
    #[must_use]
    pub const fn with_aggregate_delimiter(mut self, delimiter: AggregateDelimiter) -> Self {
        self.aggregate_delimiter = delimiter;
        self
    }

    /// The same plan with a per-element octet length in front of a
    /// `sequence` element, or without one.
    #[must_use]
    pub const fn with_sequence_element_delimiter(
        mut self,
        delimiter: SequenceElementDelimiter,
    ) -> Self {
        self.sequence_element_delimiter = delimiter;
        self
    }

    /// The same plan with a `wstring`'s characters in UTF-16 code units
    /// or in UTF-8 octets.
    #[must_use]
    pub const fn with_wstring_code_units(mut self, units: WstringCodeUnits) -> Self {
        self.wstring_code_units = units;
        self
    }

    /// The same plan with a different `wstring` length policy.
    #[must_use]
    pub const fn with_wstring_length(mut self, policy: WstringLengthPolicy) -> Self {
        self.wstring_length = policy;
        self
    }

    /// The same plan framing a scalar `wchar` another way.
    #[must_use]
    pub const fn with_wchar_framing(mut self, framing: WcharFraming) -> Self {
        self.wchar_framing = framing;
        self
    }

    /// The same plan at a different scalar `wchar` width.
    #[must_use]
    pub const fn with_wchar_width(mut self, width: WcharWidth) -> Self {
        self.wchar_width = width;
        self
    }

    /// The same plan asking a different half of a `map` whether the map
    /// is delimited.
    #[must_use]
    pub const fn with_map_delimiter(mut self, policy: MapDelimiter) -> Self {
        self.map_delimiter = policy;
        self
    }

    /// The same plan ordering a `wstring`'s code units another way.
    #[must_use]
    pub const fn with_wstring_unit_order(mut self, order: WstringUnitOrder) -> Self {
        self.wstring_unit_order = order;
        self
    }

    /// Whether two settings of this plan both decide the byte order of a
    /// `wstring`'s code units.
    ///
    /// **Always false now, and kept as a signature rather than deleted.**
    /// It answered the pair `(WstringLengthPolicy::UnswappedUnits,
    /// non-Stream)`, which existed only because a byte order was living
    /// in the length enumeration under a length's name. Judgement 2 of
    /// `docs/wire-axes-followup.md` moved it to
    /// [`WstringUnitOrder::HostUnswapped`], and one axis cannot conflict
    /// with itself: the four orders are four points of one setting and
    /// any of them can be combined with any cell of C1 × C2 × C3.
    ///
    /// The method stays because a caller refusing an impossible pair is
    /// cheap and a caller that stops refusing a possible one is not. If
    /// a second setting ever claims the same byte order, this is where
    /// it is caught.
    #[must_use]
    pub const fn wstring_order_conflicts(self) -> bool {
        false
    }

    /// The same plan with a different PL_CDR1 header policy.
    #[must_use]
    pub const fn with_pl_cdr1_headers(mut self, policy: PlCdr1HeaderPolicy) -> Self {
        self.pl_cdr1_headers = policy;
        self
    }

    /// The plan for a sample of `root`: a PL_CDR1 header policy that is
    /// [decided per sample](PlCdr1HeaderPolicy::decided_per_sample)
    /// becomes [`PlCdr1HeaderPolicy::ExtendedForConstructed`] when the
    /// sample's type can exceed the short form's length and
    /// [`PlCdr1HeaderPolicy::ShortRoundedLength`] when it cannot. Any other
    /// key is returned as it is.
    ///
    /// A writer and a reader of a whole sample call this once, with the
    /// sample's type, and frame every type nested in it with the key it
    /// returns: RTI decides the form for the sample, and a nested type
    /// that is small on its own is framed by its sample's choice.
    ///
    /// # Errors
    /// [`crate::plan::LayoutError`] when the sample's type cannot be sized: an
    /// unresolved reference, an unevaluable bound, a nesting past the
    /// planner's bound.
    pub fn for_sample(
        self,
        ast: &zerodds_ast::Ast,
        root: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    ) -> Result<Self, crate::plan::LayoutError> {
        if !self.pl_cdr1_headers.decided_per_sample() || !self.encoding.is_v1() {
            return Ok(self);
        }
        Ok(self.with_pl_cdr1_headers(
            if crate::large_sample::exceeds_short_length(ast, root, self)? {
                PlCdr1HeaderPolicy::ExtendedForConstructed
            } else {
                PlCdr1HeaderPolicy::ShortRoundedLength
            },
        ))
    }

    /// The same plan with a reader that takes another framing's PL_CDR1
    /// streams, or does not.
    #[must_use]
    pub const fn with_pl_cdr1_reading(mut self, reading: PlCdr1Reading) -> Self {
        self.pl_cdr1_reading = reading;
        self
    }

    /// The same plan putting different flags on a parameter id.
    #[must_use]
    pub const fn with_pid_flags(mut self, policy: PidFlagPolicy) -> Self {
        self.pid_flags = policy;
        self
    }

    /// The same plan writing or omitting a member's must-understand
    /// flag.
    #[must_use]
    pub const fn with_must_understand(mut self, policy: MustUnderstandPolicy) -> Self {
        self.must_understand = policy;
        self
    }

    /// The same plan giving an enumeration a holder of another width.
    #[must_use]
    pub const fn with_enum_holder(mut self, policy: EnumHolderWidth) -> Self {
        self.enum_holder = policy;
        self
    }

    /// The same plan matching a negative case label another way.
    #[must_use]
    pub const fn with_union_label(mut self, policy: UnionLabelMatch) -> Self {
        self.union_label = policy;
        self
    }

    /// The same key with another array-element flattening.
    #[must_use]
    pub const fn with_array_element_flattening(mut self, policy: ArrayElementFlattening) -> Self {
        self.array_element_flattening = policy;
        self
    }

    /// The same plan ordering a `map`'s entries another way.
    #[must_use]
    pub const fn with_map_entries(mut self, policy: MapEntryOrder) -> Self {
        self.map_entries = policy;
        self
    }

    /// The same plan with a different enumerator-value policy.
    #[must_use]
    pub const fn with_enumerator_value(mut self, policy: EnumeratorValue) -> Self {
        self.enumerator_value = policy;
        self
    }

    /// The same plan with a different `@mutable` union framing.
    #[must_use]
    pub const fn with_union_framing(mut self, policy: MutableUnionFraming) -> Self {
        self.union_framing = policy;
        self
    }

    /// The same plan with a different short-PID range.
    #[must_use]
    pub const fn with_short_pid_range(mut self, policy: ShortPidRange) -> Self {
        self.short_pid_range = policy;
        self
    }

    /// The same plan counting a `fixed`'s digits another way.
    #[must_use]
    pub const fn with_fixed_digits(mut self, policy: FixedDigitCount) -> Self {
        self.fixed_digits = policy;
        self
    }

    /// The same plan with a different key-holder rule.
    #[must_use]
    pub const fn with_key_holder(mut self, policy: KeyHolderExtensibility) -> Self {
        self.key_holder = policy;
        self
    }

    /// The same plan with a different key-holder nesting rule.
    #[must_use]
    pub const fn with_key_holder_nesting(mut self, policy: KeyHolderNesting) -> Self {
        self.key_holder_nesting = policy;
        self
    }

    /// The same plan ordering a key holder's members differently.
    #[must_use]
    pub const fn with_key_holder_order(mut self, policy: KeyHolderOrder) -> Self {
        self.key_holder_order = policy;
        self
    }

    /// The same plan aligning a key holder's members differently.
    #[must_use]
    pub const fn with_key_holder_alignment(mut self, policy: KeyHolderAlignment) -> Self {
        self.key_holder_alignment = policy;
        self
    }

    /// The same plan giving a union's member another answer when
    /// `@try_construct(use_default)` defaulted its discriminator.
    #[must_use]
    pub const fn with_defaulted_discriminator_member(
        mut self,
        policy: DefaultedDiscriminatorMember,
    ) -> Self {
        self.defaulted_discriminator_member = policy;
        self
    }

    /// The same plan reading an unannotated aggregate as something else.
    ///
    /// XTypes 1.3 §7.3.1.2.1.8 sanctions the knob outright:
    ///
    /// > In the event that the representation of a given type does not
    /// > indicate the type's extensibility kind, the type shall be
    /// > considered appendable. Implementations may provide a mechanism
    /// > to override this default behavior; for example, IDL compilers
    /// > may provide configuration options to allow users to specify
    /// > whether types of unspecified extensibility are to be considered
    /// > final, appendable, or mutable.
    #[must_use]
    pub const fn with_default_extensibility(mut self, extensibility: Extensibility) -> Self {
        self.default_extensibility = extensibility;
        self
    }

    /// The key-holder plan, which is always big-endian.
    /// DDSI-RTPS 2.5 §9.6.3.8.
    ///
    /// In [`Dialect::Specification`], the default, for the reason
    /// [`Self::data`] is: the dialect reaches the holder — whether an
    /// unannotated keyed aggregate carries a DHEADER depends on what
    /// unannotated means — and [`crate::keyhash::key_holder_plan_with`]
    /// is the way to plan a holder under some other dialect.
    #[must_use]
    pub const fn key(encoding: Encoding) -> Self {
        Self::in_dialect(
            Dialect::Specification,
            encoding,
            Endianness::Big,
            Purpose::Key,
        )
    }

    /// Whether a member's own parameter id carries its
    /// must-understand bit — the `M` in a short PID's 16-bit flags and
    /// `FLAG_2` in `PID_EXTENDED`'s eMemberHeader (§7.4.1.2.3).
    ///
    /// Two policies cross here. [`MustUnderstandPolicy::writes_flag`]
    /// says whether the member's declaration reaches any flag at all.
    /// [`PidFlagPolicy::ImplExtensionOnMembers`] — OpenDDS — sets
    /// `FLAG_IMPL_EXTENSION | FLAG_MUST_UNDERSTAND` on every 16-bit
    /// member id *as fixed framing*, and writes nothing
    /// declaration-dependent anywhere in a PID: measured 2026-08-24 on
    /// `identifier-keyword-member-ids` under XCDR1, where the `@key`
    /// member's eMemberHeader is `0x0FEDF2A2` — the bare hash, no
    /// `FLAG_2` — while the same member's XCDR2 EMHEADER carries
    /// `M_FLAG` (which is what put `DeclaredAndKey` in the OpenDDS
    /// dialect). §7.4.1.2.3 as written wants the bit in the
    /// eMemberHeader, so the specification dialect keeps it.
    #[must_use]
    pub const fn member_pid_carries_must_understand(self) -> bool {
        self.must_understand.writes_flag()
            && !matches!(self.pid_flags, PidFlagPolicy::ImplExtensionOnMembers)
    }

    /// The part of the key that actually affects offsets.
    ///
    /// Two keys sharing this share their offset computation, which is
    /// what lets one cache entry serve both byte orders.
    ///
    /// [`Endianness`] is not in it: byte order changes the bytes, not
    /// where they sit.
    ///
    /// Everything else is. [`CollectionDelimiterPolicy`] was left out
    /// until this cache served an RTI-dialect plan, with no delimiter,
    /// to a Cyclone-dialect request that needed one: two keys that
    /// differ in a four-byte header do not share an offset computation.
    /// [`WstringLengthPolicy`] was left out for the reason that a
    /// `wstring` plans as `Extent::Unbounded` under every variant, which
    /// is true and was not the whole question: the policy also fixes the
    /// width of a scalar `wchar`, and a four-byte one aligns to four and
    /// moves every member behind it. [`WcharFraming`] is in for the
    /// second half of the same reason — GIOP 1.2's `wchar` is one octet
    /// longer than its code unit *and* aligns to one, so a plan built
    /// for it puts every following member somewhere a fixed-width plan
    /// does not.
    ///
    /// [`WstringUnitOrder`] is **not** in it, and that is a claim rather
    /// than an omission: a `wstring` plans as `Extent::Unbounded` under
    /// all three orders, and the byte-order mark makes it two octets
    /// longer without making it any less unbounded. Nothing an order
    /// changes reaches an offset.
    ///
    /// [`EnumHolderWidth`] **is** in it, and it is the plainest case of
    /// all: an `@bit_bound(8)` enumeration is one octet under one value
    /// and four under the other, so it changes the type's own extent, its
    /// alignment and the offset of every member behind it. A cache that
    /// left it out would hand a one-octet plan to a caller that asked for
    /// the four-octet holder.
    ///
    /// [`UnionLabelMatch`] is **not** in it, and that is a claim rather
    /// than an omission. A union's extent is the discriminator plus the
    /// widest branch under both values: with no `default:` the
    /// suppressed match writes less than the extent, which the extent
    /// already bounds, and with one it writes the *default* branch,
    /// which the extent already covers. Nothing it changes reaches an
    /// offset —
    /// `crates/wire-probe/tests/union_label_match.rs::the_extent_is_the_same_under_both_values`
    /// is that claim as a test rather than as this sentence.
    ///
    /// [`MapEntryOrder`] is **not** in it, and that is a claim rather
    /// than an omission for the plainest reason of the three: a
    /// permutation of a map's entries writes the same entries, so the
    /// count, the byte extent and every offset behind it are the same
    /// under both values.
    /// `crates/wire-probe/tests/map_entry_order.rs::the_extent_is_the_same_under_both_values`
    /// is that claim as a test.
    ///
    /// [`FixedDigitCount`] **is** in it, and it is the second plainest
    /// case after [`EnumHolderWidth`]: a `fixed<9,3>` is five octets
    /// under one value and as many as the value needs under the other,
    /// so the type stops being fixed-size and every member behind it
    /// moves. A cache that left it out would hand a five-octet plan to a
    /// caller that asked for the value's digits.
    ///
    /// [`EnumeratorValue`] is **not** in it, and it is the sharpest
    /// contrast with [`EnumHolderWidth`] beside it: the two axes reach
    /// the same construct and only one of them reaches an offset. The
    /// holder's *width* is a length; *which integer* goes into it is a
    /// value, and `@value(200)` and the ordinal `2` occupy the same
    /// holder.
    /// `crates/wire-probe/tests/enumerator_value.rs::the_extent_is_the_same_under_both_values`
    /// is that claim as a test.
    #[must_use]
    pub const fn offset_key(self) -> OffsetKey {
        OffsetKey {
            encoding: self.encoding,
            purpose: self.purpose,
            length_codes: self.length_codes,
            collection_delimiters: self.collection_delimiters,
            map_delimiter: self.map_delimiter,
            wstring_length: self.wstring_length,
            wchar_framing: self.wchar_framing,
            wchar_width: self.wchar_width,
            pl_cdr1_headers: self.pl_cdr1_headers,
            default_extensibility: self.default_extensibility,
            key_holder: self.key_holder,
            key_holder_nesting: self.key_holder_nesting,
            key_holder_alignment: self.key_holder_alignment,
            enum_holder: self.enum_holder,
            fixed_digits: self.fixed_digits,
            must_understand: self.must_understand,
            union_framing: self.union_framing,
            short_pid_range: self.short_pid_range,
            element_padding: self.element_padding,
            aggregate_delimiter: self.aggregate_delimiter,
        }
    }
}

/// The part of a [`LayoutKey`] the plan cache keys on.
///
/// A struct and not a tuple, and that is a correction rather than a
/// style: the tuple stood at twelve, which is where Rust's derived
/// `Ord`, `Debug` and `Clone` impls stop, so the next field that had to
/// go in would not have compiled. A cache key that cannot grow is a
/// cache key that gets things left out of it, which is the defect
/// [`Self::must_understand`] below records.
///
/// **A field that reaches `TypeLayout` and is not here is a silent
/// wrong plan**: the cache would serve one caller's entry to another
/// whose answer differs. That is the whole of the rule, and every field
/// is here because it was checked against it rather than because it
/// looked relevant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OffsetKey {
    /// XCDR1 or XCDR2, and which framing the extensibility picks.
    pub encoding: Encoding,
    /// Data or key holder.
    pub purpose: Purpose,
    /// Which length code a `@mutable` member's EMHEADER gets, which is
    /// four octets of header either way it goes.
    pub length_codes: LengthCodePolicy,
    /// Which collections carry a delimiter, which is four octets in
    /// front of the elements.
    pub collection_delimiters: CollectionDelimiterPolicy,
    /// Which of a `map`'s halves decides whether the map is delimited,
    /// which is the same four octets in front of the entries.
    pub map_delimiter: MapDelimiter,
    /// How a `wstring`'s length is counted, which decides the extent of
    /// every `wstring` and so the offset of every member behind one.
    pub wstring_length: WstringLengthPolicy,
    /// How a scalar `wchar` is framed: GIOP 1.2's count octet is one
    /// octet longer than the unit *and* aligns to one.
    pub wchar_framing: WcharFraming,
    /// How wide a scalar `wchar` is, which is its alignment as well as
    /// its extent.
    pub wchar_width: WcharWidth,
    /// Which header form a `@mutable` member takes under XCDR1: four
    /// octets against twelve.
    pub pl_cdr1_headers: PlCdr1HeaderPolicy,
    /// What an aggregate whose source said nothing is taken to be,
    /// which decides whether it carries a delimiter at all.
    pub default_extensibility: Extensibility,
    /// Whether a key holder keeps the type's extensibility.
    pub key_holder: KeyHolderExtensibility,
    /// How deep a key holder's member removal reaches.
    pub key_holder_nesting: KeyHolderNesting,
    /// Whether a key holder's members are aligned, which moves every
    /// offset after the first narrower key.
    pub key_holder_alignment: KeyHolderAlignment,
    /// How wide an enumeration's holder is: one octet against four.
    pub enum_holder: EnumHolderWidth,
    /// Whether a `fixed<P,S>` is `P` digits wide or as wide as the
    /// value, which is the difference between a sized type and an
    /// unbounded one.
    pub fixed_digits: FixedDigitCount,
    /// Whether a member's `must_understand` attribute is set, which the
    /// plan carries in every `MemberHeader` it builds.
    ///
    /// **This was missing, and it was a silent wrong plan.** The
    /// attribute is not a length — no offset moves with it — but the
    /// plan is what carries it to all three columns, so a cache that
    /// left it out served a plan built under `Declared` to a caller that
    /// asked for `Omitted` and the flag went out anyway. Nothing at run
    /// time would have said so: the bytes would simply have been the
    /// other setting's. The field was added when
    /// [`MustUnderstandPolicy::DeclaredAndKey`] made the gap a third
    /// value wide instead of a second.
    pub must_understand: MustUnderstandPolicy,
    /// How a `@mutable` union numbers its branches and whether its XCDR1
    /// parameter list is terminated — both of which the plan carries,
    /// the first in every `MemberPlan::member_id` and the second in
    /// `TypeLayout::needs_sentinel`.
    pub union_framing: MutableUnionFraming,
    /// Which member ids fit a short parameter header, which is four
    /// octets of header against twelve.
    pub short_pid_range: ShortPidRange,
    /// Whether a collection's elements carry padding between them,
    /// which is the array's stride and so every element's offset.
    pub element_padding: ElementPadding,
    /// Whether an aggregate writes a `DHEADER` and a `@mutable` type's
    /// members carry their headers — both of which the plan carries, the
    /// first in `TypeLayout::needs_dheader` and `Extent::Sized`, the
    /// second in every `MemberHeader` and in `TypeLayout::needs_sentinel`.
    pub aggregate_delimiter: AggregateDelimiter,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default dialect is one fact spelled in three places, and
    /// this holds the three to one value.
    ///
    /// `#[default]` on the variant, [`Dialect::is_default`] as a
    /// predicate, and [`LayoutKey::data`]'s `const` call which cannot go
    /// through `Default::default`. Each of the three is read by
    /// different code — the CLI parsers take the first, the result
    /// tables and `wire-probe`'s argument test take the second, the
    /// planner takes the third — so a change that moves one and misses
    /// another would leave two defaults in one tree, and neither would
    /// look wrong on its own.
    ///
    /// The value is named here as well as compared, so that moving the
    /// default is a change to this line and not something a reader has
    /// to derive.
    #[test]
    fn the_default_dialect_is_one_fact() {
        assert_eq!(Dialect::default(), Dialect::Specification);
        assert!(Dialect::default().is_default());
        for dialect in Dialect::ALL {
            assert_eq!(
                dialect.is_default(),
                dialect == Dialect::default(),
                "`{}` disagrees with `#[default]` about being the default",
                dialect.as_str()
            );
        }
        let named = LayoutKey::in_dialect(
            Dialect::default(),
            Encoding::PlainCdr2,
            Endianness::Little,
            Purpose::Data,
        );
        assert_eq!(
            LayoutKey::data(Encoding::PlainCdr2, Endianness::Little),
            named,
            "`LayoutKey::data` names a dialect that is not `Dialect::default`"
        );
        assert_eq!(
            LayoutKey::data(Encoding::PlainCdr1, Endianness::Big),
            LayoutKey::in_dialect(
                Dialect::default(),
                Encoding::PlainCdr1,
                Endianness::Big,
                Purpose::Data
            ),
            "the two spellings part under encoding version 1"
        );
        for encoding in Encoding::ALL {
            assert_eq!(
                LayoutKey::key(encoding),
                LayoutKey::in_dialect(Dialect::default(), encoding, Endianness::Big, Purpose::Key),
                "the key holder's plan names a dialect that is not `Dialect::default`"
            );
        }
    }

    /// The two settings that made the default move are the clause's
    /// under the default, and the product's under the product's name.
    ///
    /// Named rather than implied: these are the two axes on which
    /// `Dialect::Cyclone` departs from XTypes 1.3, and a caller who
    /// names no dialect used to get both. A regression that put the
    /// default back on a product would leave this red with the two axes
    /// in the message rather than leaving a table to be re-read.
    #[test]
    fn the_default_takes_no_product_departure_on_the_two_axes() {
        assert_eq!(
            Dialect::default().union_label(),
            UnionLabelMatch::SignExtended,
            "the default drops a branch a conforming reader expects"
        );
        assert_eq!(
            Dialect::default().wstring_unit_order(),
            WstringUnitOrder::Stream,
            "the default writes a `wstring`'s code units in the host's order"
        );
        assert_eq!(
            Dialect::Cyclone.union_label(),
            UnionLabelMatch::NegativeLabelUnmatchedBelow32,
            "`--dialect cyclone` stopped reproducing Cyclone"
        );
        assert_eq!(
            Dialect::Cyclone.wstring_unit_order(),
            WstringUnitOrder::HostUnswapped,
            "`--dialect cyclone` stopped reproducing Cyclone"
        );
    }

    /// Every cell of C1 × C2 × C3 is constructible, nameable and
    /// parseable — and there are eight of them.
    ///
    /// This is the test the rebuild exists for. The axis was an
    /// enumeration of five named forms, which meant the reachable
    /// values were the ones somebody had already written down; two
    /// implementations were writing a sixth and the project had no way
    /// to say it. A product cannot have that failure, and this fails if
    /// anything puts the names back in charge of the values.
    #[test]
    fn every_cell_of_the_product_is_reachable() {
        assert_eq!(WstringLengthPolicy::ALL_CELLS.len(), 8);
        let mut seen = alloc::collections::BTreeSet::new();
        for cell in WstringLengthPolicy::ALL_CELLS {
            assert!(seen.insert(cell), "{} appears twice", cell.as_str());
            assert_eq!(
                WstringLengthPolicy::parse(cell.as_str()),
                Some(cell),
                "{} does not parse back to itself",
                cell.as_str()
            );
            assert_eq!(
                WstringLengthPolicy::parse(&coordinates(cell)),
                Some(cell),
                "{} is not reachable by coordinate",
                cell.as_str()
            );
        }
        assert_eq!(seen.len(), 8);
    }

    /// The cell both ROS type supports write is one of the eight, and
    /// it is not any of the four the enumeration had.
    ///
    /// Characters, no terminator, four-octet units. Measured 2026-08-12
    /// from the recorded corpus: `wstring-unbounded-utf8` little endian
    /// is `07000000 e4000000 f6000000 fc000000 20000000 34120000
    /// 20000000 5a000000` from `ros-fastrtps` and from `ros-cyclone`
    /// alike. `CHARACTER_COUNT` is that length over two-octet units and
    /// `SPECIFICATION_WIDE_UNITS` is four-octet units under the other
    /// length, so neither reaches it and the cell between them had no
    /// name.
    #[test]
    fn the_ros_cell_is_reachable_and_is_none_of_the_four() {
        let ros = WstringLengthPolicy::ROS_TYPE_SUPPORT;
        assert_eq!(ros.counts, WstringLengthCounts::Characters);
        assert_eq!(ros.terminator, WstringTerminator::Absent);
        assert_eq!(ros.unit, WstringUnitWidth::Four);
        for other in [
            WstringLengthPolicy::SPECIFICATION,
            WstringLengthPolicy::CHARACTER_COUNT,
            WstringLengthPolicy::CHARACTER_COUNT_TERMINATED,
            WstringLengthPolicy::SPECIFICATION_WIDE_UNITS,
        ] {
            assert_ne!(
                ros,
                other,
                "the ROS cell collided with {}, so the split did not separate the questions",
                other.as_str()
            );
        }
        assert!(WstringLengthPolicy::ALL_CELLS.contains(&ros));
        assert_eq!(
            WstringLengthPolicy::parse("characters/unterminated/4"),
            Some(ros)
        );
    }

    /// **Counter-check.** RTI under XCDR1 is CORBA §9.3.1.6's GIOP 1.1
    /// `wstring` and needs no RTI-specific line.
    ///
    /// Judgement 2 of `docs/wire-axes-followup.md` required this to be
    /// checked explicitly after the split: if RTI's XCDR1 form were not
    /// exactly a cell of the product, an axis would still be missing.
    /// It is the cell — characters, terminated, four-octet units — and
    /// `Dialect::Rti` reaches it by naming the cell rather than by
    /// naming itself.
    #[test]
    fn rti_under_xcdr1_is_giop_1_1_with_no_rti_specific_line() {
        let giop_1_1 = WstringLengthPolicy::new(
            WstringLengthCounts::Characters,
            WstringTerminator::Present,
            WstringUnitWidth::Four,
        );
        assert_eq!(
            Dialect::Rti.wstring_length(Encoding::PlainCdr1),
            giop_1_1,
            "RTI's XCDR1 wstring is no longer GIOP 1.1's, so an axis is missing"
        );
        assert_eq!(Dialect::Rti.wstring_length(Encoding::PlCdr1), giop_1_1);
        // And under version 2 it is the specification's, which is what
        // makes the XCDR1 answer a version question rather than a
        // vendor one.
        assert_eq!(
            Dialect::Rti.wstring_length(Encoding::PlainCdr2),
            WstringLengthPolicy::SPECIFICATION
        );
    }

    /// **Counter-check.** Fast DDS's answer is a cell of the product,
    /// not a special rule.
    ///
    /// Characters, no terminator, two-octet units — the crossing of
    /// three coordinates each of which some other implementation also
    /// takes. Nothing about it is Fast DDS's alone, which is the test:
    /// a form that could only be described by naming its vendor would
    /// mean the coordinates were not the right ones.
    #[test]
    fn fast_dds_is_a_cell_and_not_a_rule() {
        let fastdds = Dialect::FastDds.wstring_length(Encoding::PlainCdr1);
        assert_eq!(fastdds, WstringLengthPolicy::CHARACTER_COUNT);
        assert!(WstringLengthPolicy::ALL_CELLS.contains(&fastdds));
        // Each coordinate is shared with an implementation that is not
        // Fast DDS: C1 with RTI and both ROS type supports, C2 with
        // seven of the nine, C3 with Cyclone, OpenDDS, omniORB, JacORB.
        assert_eq!(
            fastdds.counts,
            Dialect::Rti.wstring_length(Encoding::PlainCdr1).counts
        );
        assert_eq!(
            fastdds.terminator,
            Dialect::Cyclone
                .wstring_length(Encoding::PlainCdr1)
                .terminator
        );
        assert_eq!(
            fastdds.unit,
            Dialect::OmniOrb.wstring_length(Encoding::PlainCdr1).unit
        );
    }

    /// Every position renders and every position says whether it is
    /// measured.
    ///
    /// Measured and prospective must not look alike: a reader who
    /// cannot tell them apart reads the next measurement as a
    /// confirmation of something that was never checked.
    #[test]
    fn the_measured_positions_are_told_apart_from_the_prospective_ones() {
        for position in WirePosition::ALL {
            assert!(!position.as_str().is_empty());
        }
        assert_eq!(WirePosition::MEASURED.len(), 4);
        for position in WirePosition::MEASURED {
            assert!(position.is_measured(), "{}", position.as_str());
        }
        for position in WirePosition::ALL {
            assert_eq!(
                position.is_measured(),
                WirePosition::MEASURED.contains(&position),
                "{} disagrees with the measured list",
                position.as_str()
            );
        }
        // The two that are carried and not measured, named so that a
        // future measurement moves a name rather than adds one.
        assert!(!WirePosition::Discriminator.is_measured());
        assert!(!WirePosition::KeyHolderMember.is_measured());
    }

    /// The coordinate spelling of a cell, for the round-trip above.
    fn coordinates(cell: WstringLengthPolicy) -> alloc::string::String {
        let counts = match cell.counts {
            WstringLengthCounts::Octets => "octets",
            WstringLengthCounts::Characters => "characters",
        };
        let terminator = match cell.terminator {
            WstringTerminator::Absent => "unterminated",
            WstringTerminator::Present => "terminated",
        };
        alloc::format!("{counts}/{terminator}/{}", cell.unit.octets())
    }
}
