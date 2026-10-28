// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `DynamicData` — a value of a type known only at run time. §7.5.2.
//!
//! It is the **second implementation** of
//! [`zerodds_wire_codec::Source`] and [`zerodds_wire_codec::Sink`], and
//! that is the whole of what it is. `serde_json::Value` is the first, in
//! `zerodds_wire_codec::json`; between them there is one walk over the
//! layout plan, in each direction, and no second answer to a framing
//! question.
//!
//! ## What that buys and what it costs
//!
//! `encode`, `decode`, `canonicalise` and `default::of_type` are already
//! generic over both traits, so a `DynamicData` reaches the wire through
//! the same code a JSON value does — the same DHEADER decisions, the same
//! EMHEADER decisions, the same length codes, the same dialect answers.
//! The measurement is in `tests/dynamic_corpus.rs`: over all 347 cases of
//! `tests/interop-corpus/` in all four `{XCDR1, XCDR2} × {LE, BE}`
//! combinations, a `DynamicData` decoded from the bytes a
//! `serde_json::Value` produced re-encodes to those same bytes, octet for
//! octet.
//!
//! What it costs is that this type carries no rule of its own. A bound is
//! not checked here, a range is not checked here, an enumerator's name is
//! not looked up here — all of that is `zerodds_wire_codec::contract`,
//! which is asked once for every representation. A rule implemented twice
//! is a rule two representations can disagree about.
//!
//! ## What it refuses
//!
//! Nothing silently. `DynamicData` can hold every value the two traits
//! construct — an `i128` holds every IDL integer exactly, a
//! [`Float128`] holds a `long double` exactly, a `String` holds a
//! `fixed<P,S>`'s decimal exactly — so there is no value the sink has to
//! turn away. The refusals are all on the read side: asking a text for an
//! integer, asking an aggregate for a character, asking an integer that
//! no `f64` holds exactly for a float. Each is a
//! [`ProbeError::Mismatch`] naming the path.
//!
//! It is a **superset** of what JSON can hold, which is the one place the
//! two representations differ on purpose: JSON has no NaN and no
//! infinity, and refuses them ([`zerodds_wire_codec::json`]'s fourth
//! rule); this holds them. [`Source::render`] writes them as bare tokens
//! that no value file admits, so a comparison shows them rather than
//! letting one compare equal to a number.
//!
//! ## The canonical text
//!
//! [`Source::render`] writes the form `tests/interop-corpus/`'s value
//! files are written in: keys sorted, no whitespace, a 64-bit integer and
//! a `long double` and a `fixed<P,S>` as strings, a character as a
//! one-character string, an enumerator as its name, a bitmask as an array
//! of bit names, a bitset as an object of numbers, a map entry as a
//! two-element array. That is a decision about **this** representation
//! and is stated here for the reason `json.rs` states JSON's: what "one
//! form" means depends on what the values are written in.
//!
//! One spelling is not JSON's and is named rather than left to be found:
//! a floating-point number is written with Rust's shortest round-trip
//! decimal, where `serde_json` writes `ryu`'s. The two agree on the value
//! and not always on the text — `1.0` against `1`. Nothing compares the
//! two texts: the corpus proof compares **octets**, which is the thing
//! two implementations have to agree on.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::PrimitiveKind;
use zerodds_float128::Float128;
use zerodds_wire_codec::{Aggregate, ProbeError, Sink, Source};

/// A value of a type known only at run time.
///
/// One variant per [`Sink`] constructor, so that what came out of the
/// read direction is what goes back into the write direction. The
/// variants that carry a [`PrimitiveKind`] carry the one the sink was
/// told; nothing here infers a width from a magnitude.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DynamicData {
    /// `true` or `false`.
    Boolean(bool),
    /// An integer.
    ///
    /// `kind` is `None` where the sink was not told a width. That happens
    /// in exactly one place and it is the trait's shape, not an omission
    /// here: [`Sink::bitfields`] hands over `(name, value)` pairs with no
    /// width, because a bitset's holder type is the *type's* business and
    /// §7.4.1.1.3.1 packs the fields into one holder rather than giving
    /// each its own.
    Integer {
        /// The width the sink was told, where it was told one.
        kind: Option<PrimitiveKind>,
        /// The value, exactly. An `i128` holds every IDL integer.
        value: i128,
    },
    /// A floating-point value.
    ///
    /// `kind` is `None` for [`Sink::number`] — "a number no type governs",
    /// which is a `@default` float literal before the member's own width
    /// narrows it.
    Float {
        /// The width the sink was told, where it was told one.
        kind: Option<PrimitiveKind>,
        /// The value.
        value: f64,
    },
    /// A `long double`, exactly. §7.2.2.2.1.2.
    LongDouble(Float128),
    /// One character.
    ///
    /// Held as a `String` of exactly one `char`, which is the invariant
    /// [`Sink::character`] establishes and the only way to build one. The
    /// text form is what [`Source::as_text`] has to return, and a `char`
    /// cannot be borrowed as a `&str`.
    Character {
        /// `Char` or `WChar`, as the sink was told.
        kind: PrimitiveKind,
        /// The character, as a one-character string.
        value: String,
    },
    /// A `string` or a `wstring`, and the loose form of a `@default`
    /// literal that has not met its type yet.
    Text(String),
    /// A `fixed<P, S>`, in the canonical decimal
    /// `zerodds_wire_codec::canonical` gives it.
    Fixed(String),
    /// The enumerator this value names.
    ///
    /// A name and never the number behind it, for the reason
    /// [`Sink::enumerator`] states: the number depends on `@value` and on
    /// the dialect, and two peers that number their enumerators
    /// differently still mean the same enumerator.
    Enumerator(String),
    /// A bitmask, as the bits it sets in declaration order.
    ///
    /// The elements are [`Self::Text`], because that is what
    /// `zerodds_wire_codec::contract::bit_names` reads them back as. The
    /// container is what carries the fact that they are bit names.
    Bits(Vec<DynamicData>),
    /// A bitset, as its named bitfields in declaration order.
    ///
    /// Anonymous bitfields are padding and are not among them.
    Bitfields(Vec<(String, DynamicData)>),
    /// A sequence, an array, a map, or one entry of a map.
    ///
    /// A map entry is a two-element collection, `[key, value]`, and not a
    /// variant of its own — the same shape `serde_json::Value` gives it.
    /// A separate variant would compare unequal to the collection
    /// `zerodds_wire_codec::canonical` rebuilds a map's entries as, and
    /// two forms of one value is what the canonical form exists to
    /// prevent.
    Collection(Vec<DynamicData>),
    /// A structure, a valuetype or a union.
    ///
    /// In insertion order, which is the order the reader met the members
    /// in — for a `@mutable` type that is the order the *writer* chose,
    /// and it is preserved rather than sorted so that a caller can see
    /// it. [`Source::render`] sorts, which is where an order that must
    /// not reach a comparison stops.
    Aggregate(Vec<(String, DynamicData)>),
}

impl DynamicData {
    /// A boolean.
    #[must_use]
    pub const fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    /// An integer of a known width.
    #[must_use]
    pub const fn integer(kind: PrimitiveKind, value: i128) -> Self {
        Self::Integer {
            kind: Some(kind),
            value,
        }
    }

    /// A floating-point value of a known width.
    #[must_use]
    pub const fn float(kind: PrimitiveKind, value: f64) -> Self {
        Self::Float {
            kind: Some(kind),
            value,
        }
    }

    /// One character of a known width.
    #[must_use]
    pub fn character(kind: PrimitiveKind, value: char) -> Self {
        Self::Character {
            kind,
            value: value.to_string(),
        }
    }

    /// A string.
    #[must_use]
    pub const fn text(value: String) -> Self {
        Self::Text(value)
    }

    /// The character this value carries, where it carries exactly one.
    ///
    /// `None` for anything else — including a [`Self::Text`] of one
    /// character, which is a string that happens to be short and not a
    /// character. Which of the two a value is, is the sink's answer and
    /// is not re-derived from the length.
    #[must_use]
    pub fn as_char(&self) -> Option<char> {
        match self {
            Self::Character { value, .. } => value.chars().next(),
            _ => None,
        }
    }

    /// The width this value was built at, where it was built at one.
    #[must_use]
    pub const fn primitive_kind(&self) -> Option<PrimitiveKind> {
        match self {
            Self::Integer { kind, .. } | Self::Float { kind, .. } => *kind,
            Self::Character { kind, .. } => Some(*kind),
            Self::LongDouble(_) => Some(PrimitiveKind::Float128),
            Self::Boolean(_) => Some(PrimitiveKind::Boolean),
            _ => None,
        }
    }

    /// The named members of an aggregate or a bitset, in the order they
    /// went in.
    #[must_use]
    pub fn members(&self) -> Option<&[(String, Self)]> {
        match self {
            Self::Aggregate(entries) | Self::Bitfields(entries) => Some(entries),
            _ => None,
        }
    }

    /// The elements of a collection or a bitmask.
    #[must_use]
    pub fn items(&self) -> Option<&[Self]> {
        match self {
            Self::Collection(items) | Self::Bits(items) => Some(items),
            _ => None,
        }
    }

    /// The value of one named member.
    #[must_use]
    pub fn member(&self, name: &str) -> Option<&Self> {
        self.members()?
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, value)| value)
    }
}

impl Source for DynamicData {
    fn kind(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "a boolean",
            Self::Integer { .. } => "an integer",
            Self::Float { .. } => "a number",
            Self::LongDouble(_) => "a long double",
            Self::Character { .. } => "a character",
            Self::Text(_) => "a string",
            Self::Fixed(_) => "a fixed-point decimal",
            Self::Enumerator(_) => "an enumerator",
            Self::Bits(_) => "a bitmask",
            Self::Bitfields(_) => "a bitset",
            Self::Collection(_) => "a collection",
            Self::Aggregate(_) => "an aggregate",
        }
    }

    /// Always `false`.
    ///
    /// This representation has no explicit nothing: an absent
    /// `@optional` member is a member that is not in the aggregate at
    /// all, which is what `contract::member_value` reads. A representation
    /// with no such thing answers `false` and the rule costs it nothing —
    /// see [`Source::is_null`].
    fn is_null(&self) -> bool {
        false
    }

    fn is_object(&self) -> bool {
        matches!(self, Self::Aggregate(_) | Self::Bitfields(_))
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text)
            | Self::Fixed(text)
            | Self::Enumerator(text)
            | Self::Character { value: text, .. } => Some(text),
            _ => None,
        }
    }

    fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float { value, .. } => Some(*value),
            // An integer where a float is wanted: only where the value
            // survives the conversion. A `long long` past 2^53 that
            // silently became the nearest double would put a number
            // nobody wrote on the wire; `None` here becomes a
            // `ProbeError::Mismatch` naming the path, in
            // `contract::float`.
            Self::Integer { value, .. } => {
                #[allow(clippy::cast_precision_loss)]
                let widened = *value as f64;
                (widened as i128 == *value).then_some(widened)
            }
            _ => None,
        }
    }

    fn as_items(&self) -> Option<&[Self]> {
        self.items()
    }

    fn field(&self, name: &str) -> Option<&Self> {
        self.member(name)
    }

    fn entries(&self) -> Vec<(&str, &Self)> {
        self.members().map_or_else(Vec::new, |entries| {
            entries
                .iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect()
        })
    }

    fn as_integer(&self, path: &str) -> Result<i128, ProbeError> {
        match self {
            Self::Integer { value, .. } => Ok(*value),
            // The loose form of a `@default` integer literal, which
            // `zerodds_wire_codec::default::literal_value` writes as text
            // so that a 64-bit member keeps its low bits.
            // `canonicalise` then reads it back here, at the member's own
            // type.
            Self::Text(text) => text.parse::<i128>().map_err(|_| ProbeError::Mismatch {
                path: path.to_string(),
                expected: String::from("an integer in a string"),
                found: alloc::format!("`{text}`"),
            }),
            // A number where an integer is wanted: only where it is
            // exactly integral. `3.5` asked for as an `octet` is a value
            // file that says something the type cannot hold, and
            // truncating it would put `3` on the wire.
            Self::Float { value, .. } => {
                #[allow(clippy::cast_possible_truncation)]
                let truncated = *value as i128;
                #[allow(clippy::cast_precision_loss)]
                let exact = value.is_finite() && (truncated as f64) == *value;
                if exact {
                    Ok(truncated)
                } else {
                    Err(ProbeError::Mismatch {
                        path: path.to_string(),
                        expected: String::from("an integer"),
                        found: alloc::format!("the number {value}"),
                    })
                }
            }
            other => Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: String::from("an integer"),
                found: Source::kind(other).to_string(),
            }),
        }
    }

    fn as_float128(&self, path: &str) -> Result<Float128, ProbeError> {
        match self {
            Self::LongDouble(value) => Ok(*value),
            // A double widens into a binary128 exactly, so a `@default`
            // float literal on a `long double` member is accepted — the
            // same rule `json.rs` states for a JSON number.
            Self::Float { value, .. } => Ok(Float128::from_f64(*value)),
            // And the decimal spelling, which is what a value file uses
            // and what `Sink::float128` writes back out.
            Self::Text(text) | Self::Fixed(text) => match float128_from_hex_bits(text) {
                Some(Ok(value)) => Ok(value),
                Some(Err(expected)) => Err(ProbeError::Mismatch {
                    path: path.to_string(),
                    expected: String::from(expected),
                    found: alloc::format!("`{text}`"),
                }),
                None => text
                    .parse::<Float128>()
                    .map_err(|error| ProbeError::Mismatch {
                        path: path.to_string(),
                        expected: String::from(
                            "a decimal a long double can hold, or 0x and 32 hex digits",
                        ),
                        found: alloc::format!("`{text}` — {error}"),
                    }),
            },
            other => Err(ProbeError::Mismatch {
                path: path.to_string(),
                expected: String::from("a long double, as a decimal string or a number"),
                found: Source::kind(other).to_string(),
            }),
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        write_text(self, &mut out);
        out
    }
}

/// The canonical text of one value. See the module note.
///
/// zerodds-lint: recursion-depth 64 (collection and aggregate nesting
/// of the value). [`DynamicData`] is an owned tree — no variant holds a
/// shared or back reference — so the walk cannot cycle, and every
/// producer of a value caps its nesting: the decoder at
/// [`zerodds_wire_codec::MAX_VALUE_DEPTH`], the type walk at this
/// module's sibling `type_::MAX_DEPTH`, both 64.
fn write_text(value: &DynamicData, out: &mut String) {
    use core::fmt::Write as _;
    match value {
        DynamicData::Boolean(true) => out.push_str("true"),
        DynamicData::Boolean(false) => out.push_str("false"),
        // The two 64-bit widths are decimal strings and every narrower
        // one is a number — the rule the corpus is written to, decided on
        // the **width** and not on the magnitude, so that a form does not
        // change shape at 2^53. A bitfield, which was told no width, is a
        // number: a bitset's fields are at most 64 bits and its holder is
        // the type's, not the field's.
        DynamicData::Integer { kind, value } => {
            if matches!(kind, Some(PrimitiveKind::Int64 | PrimitiveKind::UInt64)) {
                let _ = write!(out, "\"{value}\"");
            } else {
                let _ = write!(out, "{value}");
            }
        }
        DynamicData::Float { value, .. } => write_number(*value, out),
        // The exact expansion and not the shortest one, as
        // `Sink::float128` writes it: every binary value has finitely
        // many decimal digits, and four columns that each implement a
        // shortest-form search will disagree about which of two equally
        // short spellings to emit long before they disagree about the
        // value.
        DynamicData::LongDouble(value) => write_quoted(&value.to_exact_decimal(), out),
        DynamicData::Character { value, .. }
        | DynamicData::Text(value)
        | DynamicData::Fixed(value)
        | DynamicData::Enumerator(value) => write_quoted(value, out),
        DynamicData::Bits(items) | DynamicData::Collection(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_text(item, out);
            }
            out.push(']');
        }
        DynamicData::Bitfields(entries) | DynamicData::Aggregate(entries) => {
            // Sorted, and not in insertion order: a `@mutable` writer
            // chooses the order it writes its members in, and a canonical
            // form that kept it would report the writer's choice as a
            // difference in the value.
            let mut keys: Vec<&String> = entries.iter().map(|(name, _)| name).collect();
            keys.sort_unstable();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_quoted(key, out);
                out.push(':');
                match entries.iter().find(|(name, _)| name == key) {
                    Some((_, entry)) => write_text(entry, out),
                    // Unreachable through the key list this was built
                    // from. Named rather than skipped: a renderer that
                    // printed nothing here would emit a text that parses
                    // as something else, and the reader downstream would
                    // report only that.
                    None => out.push_str("\"<missing>\""),
                }
            }
            out.push('}');
        }
    }
}

/// A floating-point value, in the one spelling this representation
/// writes.
///
/// NaN and the two infinities are bare tokens no value file admits. They
/// are written rather than refused because [`Source::render`] has no way
/// to refuse — and written as tokens rather than as `null` or `0`, which
/// would compare equal to a value somebody meant.
fn write_number(value: f64, out: &mut String) {
    use core::fmt::Write as _;
    if value.is_nan() {
        out.push_str("nan");
    } else if value.is_infinite() {
        out.push_str(if value.is_sign_negative() {
            "-infinity"
        } else {
            "infinity"
        });
    } else {
        let _ = write!(out, "{value}");
    }
}

/// One string, escaped as a value file escapes it.
fn write_quoted(text: &str, out: &mut String) {
    use core::fmt::Write as _;
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

impl Sink for DynamicData {
    fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    fn integer(kind: PrimitiveKind, value: i128, path: &str) -> Result<Self, ProbeError> {
        // Infallible: an `i128` holds every IDL integer, at every width,
        // exactly. `path` is in the signature because a representation
        // that *cannot* hold one has to say where — see `json.rs`, whose
        // JSON number stops carrying a 64-bit integer past 2^53.
        let _ = path;
        Ok(Self::Integer {
            kind: Some(kind),
            value,
        })
    }

    fn float(kind: PrimitiveKind, value: f64, path: &str) -> Result<Self, ProbeError> {
        // Infallible, and NaN and infinity among them: this
        // representation has a spelling for both. That is the one place
        // it is a superset of JSON, which has neither — see the module
        // note.
        let _ = path;
        Ok(Self::Float {
            kind: Some(kind),
            value,
        })
    }

    fn float128(value: Float128) -> Self {
        Self::LongDouble(value)
    }

    // Built out of the variant and not through the inherent
    // constructor of the same name. Inherent resolution is what keeps
    // the two apart, and `Self::character(..)` here would turn silent
    // precedence into silent recursion the day the inherent one is
    // renamed — the trap `json.rs` names on `is_null`.
    fn character(kind: PrimitiveKind, value: char) -> Self {
        Self::Character {
            kind,
            value: value.to_string(),
        }
    }

    fn text(value: String) -> Self {
        Self::Text(value)
    }

    fn fixed(decimal: String) -> Self {
        Self::Fixed(decimal)
    }

    fn enumerator(name: String) -> Self {
        Self::Enumerator(name)
    }

    fn bit_names(names: Vec<String>) -> Self {
        Self::Bits(names.into_iter().map(Self::Text).collect())
    }

    fn bitfields(fields: Vec<(String, i128)>, path: &str) -> Result<Self, ProbeError> {
        // Infallible for the reason `integer` is. The width is not among
        // the arguments, so the values carry none — see
        // `DynamicData::Integer`.
        let _ = path;
        Ok(Self::Bitfields(
            fields
                .into_iter()
                .map(|(name, value)| (name, Self::Integer { kind: None, value }))
                .collect(),
        ))
    }

    fn collection(items: Vec<Self>) -> Self {
        Self::Collection(items)
    }

    fn map_entry(key: Self, value: Self) -> Self {
        Self::Collection(alloc::vec![key, value])
    }

    fn aggregate(members: Aggregate<Self>) -> Self {
        Self::Aggregate(members.into_entries())
    }

    fn number(value: f64, path: &str) -> Result<Self, ProbeError> {
        let _ = path;
        Ok(Self::Float { kind: None, value })
    }
}

/// A `long double` written as its raw binary128 bits: `0x` and exactly
/// 32 hexadecimal digits, least significant octet first.
///
/// The dds-xtypes test suite spells every `float128` this way — its
/// `struct_float128_x1.json` carries `0x0000000000000000000000000000fe3f`,
/// which is 0.5, and `struct_primitives.json` carries
/// `0x9A999999999999999999999999890240`, which is 12.3. A decimal parse
/// cannot read either: it stops at the `x` in byte 1. Both are the
/// octets a little-endian host would write, so they are read back the
/// same way.
///
/// Returns `None` when the text is not this form at all, so the caller
/// can fall through to the decimal spelling. A text that *is* this form
/// but malformed — wrong digit count, a non-hex digit — is an error at
/// the caller, never a silently substituted value.
fn float128_from_hex_bits(text: &str) -> Option<Result<Float128, &'static str>> {
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?;
    if digits.len() != 32 {
        return Some(Err(
            "exactly 32 hexadecimal digits, one per binary128 nibble",
        ));
    }
    // Decoded rather than validated-then-parsed: a second pass would need a
    // fallback for the digit the first pass already accepted, and a fallback
    // on an unreachable branch is exactly the shape that hides a real one
    // later. `nibble` answers `None`, so there is no value to substitute.
    let nibble = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    };
    let mut octets = [0u8; 16];
    for (slot, pair) in octets.iter_mut().zip(digits.as_bytes().chunks_exact(2)) {
        match (nibble(pair[0]), nibble(pair[1])) {
            (Some(high), Some(low)) => *slot = (high << 4) | low,
            _ => return Some(Err("a hexadecimal digit")),
        }
    }
    Some(Ok(Float128::from_le_bytes(octets)))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;

    /// The width decides the spelling, not the magnitude.
    ///
    /// The rule `tests/interop-corpus/README.md` states, and the reason
    /// it is the width: a form that changed shape at 2^53 would compare
    /// unequal across the boundary, so a `long long` holding 3 is `"3"`
    /// and a `long` holding 3 is `3`.
    #[test]
    fn a_sixty_four_bit_width_renders_as_a_string_at_every_magnitude() {
        let wide = DynamicData::integer(PrimitiveKind::Int64, 3);
        let narrow = DynamicData::integer(PrimitiveKind::Int32, 3);
        assert_eq!(Source::render(&wide), "\"3\"");
        assert_eq!(Source::render(&narrow), "3");
        // And the value is the same on the way back either way.
        assert_eq!(wide.as_integer("p"), Ok(3));
        assert_eq!(narrow.as_integer("p"), Ok(3));
    }

    /// An integer no `f64` carries exactly is refused rather than
    /// widened.
    #[test]
    fn an_integer_a_double_cannot_hold_is_not_a_float() {
        let exact = DynamicData::integer(PrimitiveKind::Int64, 1 << 53);
        assert_eq!(exact.as_float(), Some(9_007_199_254_740_992.0));
        let inexact = DynamicData::integer(PrimitiveKind::Int64, (1 << 53) + 1);
        assert_eq!(inexact.as_float(), None);
    }

    /// A number that is not integral is refused where an integer is
    /// wanted, rather than truncated.
    #[test]
    fn a_fractional_number_is_not_an_integer() {
        let whole = DynamicData::float(PrimitiveKind::Float64, 3.0);
        assert_eq!(whole.as_integer("p"), Ok(3));
        let fractional = DynamicData::float(PrimitiveKind::Float64, 3.5);
        assert!(matches!(
            fractional.as_integer("p"),
            Err(ProbeError::Mismatch { ref path, .. }) if path == "p"
        ));
    }

    /// The loose form of a `@default` integer literal, which
    /// `zerodds_wire_codec::default` writes as text.
    #[test]
    fn an_integer_in_a_string_is_an_integer() {
        assert_eq!(
            DynamicData::text("9007199254740993".to_string()).as_integer("p"),
            Ok(9_007_199_254_740_993)
        );
        assert!(
            DynamicData::text("zerodds".to_string())
                .as_integer("p")
                .is_err()
        );
    }

    /// A character is one character and keeps the width it was built at.
    #[test]
    fn a_character_keeps_its_width_and_reads_back_as_text() {
        let narrow = DynamicData::character(PrimitiveKind::Char, 'a');
        assert_eq!(narrow.as_char(), Some('a'));
        assert_eq!(narrow.as_text(), Some("a"));
        assert_eq!(narrow.primitive_kind(), Some(PrimitiveKind::Char));
        // A one-character string is a string and not a character: which
        // of the two a value is, is the sink's answer.
        assert_eq!(DynamicData::text("a".to_string()).as_char(), None);
    }

    /// This representation has no explicit nothing.
    #[test]
    fn nothing_is_ever_null() {
        for value in [
            DynamicData::Boolean(false),
            DynamicData::text(String::new()),
            DynamicData::Collection(Vec::new()),
            DynamicData::Aggregate(Vec::new()),
        ] {
            assert!(!value.is_null());
        }
    }

    /// NaN and the two infinities have a spelling here and none in a
    /// value file, and the spelling is one no value file admits.
    #[test]
    fn a_non_finite_number_is_written_as_a_token_and_not_as_a_number() {
        assert_eq!(
            Source::render(&DynamicData::float(PrimitiveKind::Float64, f64::NAN)),
            "nan"
        );
        assert_eq!(
            Source::render(&DynamicData::float(PrimitiveKind::Float64, f64::INFINITY)),
            "infinity"
        );
        assert_eq!(
            Source::render(&DynamicData::float(
                PrimitiveKind::Float64,
                f64::NEG_INFINITY
            )),
            "-infinity"
        );
    }

    /// A `@mutable` writer's member order does not reach the canonical
    /// text.
    #[test]
    fn an_aggregate_renders_with_its_keys_sorted() {
        let one = DynamicData::Aggregate(vec![
            ("b".to_string(), DynamicData::Boolean(true)),
            ("a".to_string(), DynamicData::Boolean(false)),
        ]);
        let other = DynamicData::Aggregate(vec![
            ("a".to_string(), DynamicData::Boolean(false)),
            ("b".to_string(), DynamicData::Boolean(true)),
        ]);
        assert_eq!(Source::render(&one), "{\"a\":false,\"b\":true}");
        assert_eq!(Source::render(&one), Source::render(&other));
        // The order the reader met them in is still there to be read.
        assert_eq!(one.members().map(|m| m[0].0.as_str()), Some("b"));
    }

    /// A map entry is a two-element collection and reads back as one.
    #[test]
    fn a_map_entry_is_a_pair() {
        let entry = <DynamicData as Sink>::map_entry(
            DynamicData::integer(PrimitiveKind::Int32, 1),
            DynamicData::text("one".to_string()),
        );
        let items = entry.as_items().expect("a pair is a collection");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_integer("p"), Ok(1));
        assert_eq!(items[1].as_text(), Some("one"));
    }
}
