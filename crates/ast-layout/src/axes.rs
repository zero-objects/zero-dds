// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Wire axes: the flags that pick one peer's departure from the
//! specification at a time.
//!
//! An axis is one wire question the implementations answer differently.
//! A dialect is a preset — one peer's answers to every question at once.
//! The flags override one answer at a time: apply the dialect first, then
//! the flags the caller gave.
//!
//! This module lives in `zerodds-ast-layout` so that every tool that
//! parses or applies a dialect and its flags — `wireprobe-zerodds`,
//! `zerodds-idlc generate`, `zerodds-build` — shares one definition. The
//! values a flag accepts are read off the policy type's own `ALL` (or,
//! for [`crate::key::WstringLengthPolicy`], `MEASURED`) and `as_str`
//! rather than copied into a second list here: a value
//! [`crate::key::WcharFraming::ALL`] grows by is a value every caller of
//! [`AxisFlag::help_values`] lists on the next build, not a value a
//! hand-written string here forgot.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_ast::Extensibility;

use crate::key::{
    AggregateDelimiter, CollectionDelimiterPolicy, DefaultedDiscriminatorMember, Dialect,
    ElementPadding, Encoding, Endianness, EnumHolderWidth, EnumeratorValue, FixedDigitCount,
    KeyHolderAlignment, KeyHolderExtensibility, KeyHolderOrder, LayoutKey, LengthCodePolicy,
    MapDelimiter, MapEntryOrder, MutableUnionFraming, PidFlagPolicy, PlCdr1HeaderPolicy, Purpose,
    SequenceElementDelimiter, ShortPidRange, UnionLabelMatch, WcharFraming, WcharWidth,
    WstringCodeUnits, WstringLengthPolicy, WstringUnitOrder,
};

/// The two values `--map-length` accepts, and what each means for
/// [`DialectSettings::map_length_with_delimiter`].
///
/// A `bool` and not [`crate::key::LayoutKey`]'s own type, because this
/// setting has none: `MapLengthPolicy` lives in `zerodds-wire-codec`,
/// which depends on this crate, so this crate cannot name it without a
/// cycle. The two spellings below are the ones
/// `zerodds_wire_codec::value::MapLengthPolicy::as_str` produces —
/// `map_length_spellings_match_the_axis_table` in
/// `crates/wire-codec/src/value.rs` pins both sides so a spelling that
/// changes on one side and not the other fails a test rather than a
/// probe.
const MAP_LENGTH_VALUES: [(&str, bool); 2] =
    [("sequence-equivalent", true), ("rule-15-omits", false)];

/// One wire axis a CLI flag controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisFlag {
    /// `--length-codes`: which `@mutable` EMHEADER length code is chosen.
    LengthCodes,
    /// `--collection-delimiters`: which collections carry a DHEADER under
    /// XCDR2.
    CollectionDelimiters,
    /// `--wstring-length`: what a `wstring`'s length counts.
    WstringLength,
    /// `--wchar-framing`: whether a scalar `wchar` carries an octet count.
    WcharFraming,
    /// `--wchar-width`: how wide a scalar `wchar` is.
    WcharWidth,
    /// `--map-delimiter`: which of a map's halves decides the delimiter.
    MapDelimiter,
    /// `--wstring-units`: byte order of a `wstring`'s code units.
    WstringUnits,
    /// `--pl-cdr1-headers`: `@mutable` XCDR1 member framing.
    PlCdr1Headers,
    /// `--pid-flags`: PL_CDR1 parameter id flags.
    PidFlags,
    /// `--enum-holder`: enumeration holder width.
    EnumHolder,
    /// `--union-labels`: signed-union case label matching.
    UnionLabels,
    /// `--map-entries`: `map` entry order on the wire.
    MapEntries,
    /// `--enumerator-value`: which integer an enumerator serialises as.
    EnumeratorValue,
    /// `--union-framing`: `@mutable` union branch numbering.
    UnionFraming,
    /// `--short-pid-range`: PL_CDR1 short parameter id range.
    ShortPidRange,
    /// `--fixed-digits`: `fixed<P,S>` digit count.
    FixedDigits,
    /// `--default-extensibility`: unannotated aggregate's extensibility.
    DefaultExtensibility,
    /// `--key-holder`: key holder forced-to-final policy.
    KeyHolder,
    /// `--key-holder-order`: key holder member ordering.
    KeyHolderOrder,
    /// `--key-holder-alignment`: key holder member alignment.
    KeyHolderAlignment,
    /// `--defaulted-discriminator-member`: union discriminator default
    /// behaviour.
    DefaultedDiscriminatorMember,
    /// `--map-length`: whether a delimited `map` also carries its entry
    /// count. Not a `LayoutKey` field — see [`DialectSettings`].
    MapLength,
    /// `--element-padding`: whether a collection's elements carry
    /// alignment padding between them.
    ElementPadding,
    /// `--aggregate-delimiter`: whether an aggregate's declared
    /// extensibility reaches the wire.
    AggregateDelimiter,
    /// `--sequence-element-delimiter`: whether a `sequence` element of
    /// a non-`@final` type is preceded by its own octet length.
    SequenceElementDelimiter,
    /// `--wstring-code-units`: which octets stand for a `wstring`'s
    /// characters.
    WstringCodeUnits,
}

/// Every framing question a dialect and its axis flags answer, for a
/// caller that plans a layout and one that lowers a `map`.
///
/// [`crate::key::LayoutKey`] alone is not enough: it is the plan cache's
/// key, so it carries only what the plan depends on, and a `map`'s
/// entry count is decided by the codec that walks the plan, not by the
/// plan itself (see `zerodds_wire_codec::value::MapLengthPolicy`'s own
/// documentation for why the count is a codec question). Bundling the
/// two together is what lets [`AxisFlag::parse_and_apply`] answer for
/// both from one table instead of leaving the second for every caller
/// to wire in by hand.
#[derive(Debug, Clone, Copy)]
pub struct DialectSettings {
    /// The layout key the dialect and its axis overrides add up to.
    pub key: LayoutKey,
    /// Whether a delimited `map` also carries its entry count. `true` is
    /// the sequence-equivalence reading,
    /// `zerodds_wire_codec::value::MapLengthPolicy::SequenceEquivalent`.
    pub map_length_with_delimiter: bool,
    /// The front-end options this dialect settled before any layout key
    /// existed.
    pub lowering: DialectLowering,
}

impl DialectSettings {
    /// The settings one dialect answers, before any axis flag overrides
    /// one of them.
    #[must_use]
    pub fn in_dialect(
        dialect: Dialect,
        encoding: Encoding,
        endianness: Endianness,
        purpose: Purpose,
    ) -> Self {
        Self {
            key: LayoutKey::in_dialect(dialect, encoding, endianness, purpose),
            map_length_with_delimiter: !dialect.map_count_omitted_when_delimited(),
            lowering: DialectLowering::for_dialect(dialect),
        }
    }
}

/// Read every value of a policy's own `values` off its `name` function,
/// pipe-separated, for a help text.
///
/// Not a hand-written list: [`AxisFlag::help_values`] calls this over
/// each policy's `ALL` (or `MEASURED`), so a value a policy gains is a
/// value the next build's help text and `--help` output already name.
fn joined_names<T: Copy, const N: usize>(values: [T; N], name: fn(T) -> &'static str) -> String {
    values.into_iter().map(name).collect::<Vec<_>>().join(" | ")
}

impl AxisFlag {
    /// The CLI flag this axis answers, with its leading dashes.
    #[must_use]
    pub const fn flag_name(self) -> &'static str {
        match self {
            Self::LengthCodes => "--length-codes",
            Self::CollectionDelimiters => "--collection-delimiters",
            Self::WstringLength => "--wstring-length",
            Self::WcharFraming => "--wchar-framing",
            Self::WcharWidth => "--wchar-width",
            Self::MapDelimiter => "--map-delimiter",
            Self::WstringUnits => "--wstring-units",
            Self::PlCdr1Headers => "--pl-cdr1-headers",
            Self::PidFlags => "--pid-flags",
            Self::EnumHolder => "--enum-holder",
            Self::UnionLabels => "--union-labels",
            Self::MapEntries => "--map-entries",
            Self::EnumeratorValue => "--enumerator-value",
            Self::UnionFraming => "--union-framing",
            Self::ShortPidRange => "--short-pid-range",
            Self::FixedDigits => "--fixed-digits",
            Self::DefaultExtensibility => "--default-extensibility",
            Self::KeyHolder => "--key-holder",
            Self::KeyHolderOrder => "--key-holder-order",
            Self::KeyHolderAlignment => "--key-holder-alignment",
            Self::DefaultedDiscriminatorMember => "--defaulted-discriminator-member",
            Self::MapLength => "--map-length",
            Self::ElementPadding => "--element-padding",
            Self::AggregateDelimiter => "--aggregate-delimiter",
            Self::SequenceElementDelimiter => "--sequence-element-delimiter",
            Self::WstringCodeUnits => "--wstring-code-units",
        }
    }

    /// The valid values, pipe-separated, for a help text.
    ///
    /// Read off the policy type's own `ALL` (or, for
    /// [`WstringLengthPolicy`], `MEASURED` plus
    /// [`WstringLengthPolicy::COORDINATE_FORM_HELP`]) rather than copied
    /// by hand: a policy that grows a variant is a policy whose help
    /// text grows with it, here, on the next build, with nothing to
    /// keep in step by hand.
    #[must_use]
    pub fn help_values(self) -> String {
        match self {
            Self::LengthCodes => joined_names(LengthCodePolicy::ALL, LengthCodePolicy::as_str),
            Self::CollectionDelimiters => joined_names(
                CollectionDelimiterPolicy::ALL,
                CollectionDelimiterPolicy::as_str,
            ),
            Self::WstringLength => format!(
                "{} | {}",
                joined_names(WstringLengthPolicy::MEASURED, WstringLengthPolicy::as_str),
                WstringLengthPolicy::COORDINATE_FORM_HELP
            ),
            Self::WcharFraming => joined_names(WcharFraming::ALL, WcharFraming::as_str),
            Self::WcharWidth => joined_names(WcharWidth::ALL, WcharWidth::as_str),
            Self::MapDelimiter => joined_names(MapDelimiter::ALL, MapDelimiter::as_str),
            Self::WstringUnits => joined_names(WstringUnitOrder::ALL, WstringUnitOrder::as_str),
            Self::PlCdr1Headers => {
                joined_names(PlCdr1HeaderPolicy::ALL, PlCdr1HeaderPolicy::as_str)
            }
            Self::PidFlags => joined_names(PidFlagPolicy::ALL, PidFlagPolicy::as_str),
            Self::EnumHolder => joined_names(EnumHolderWidth::ALL, EnumHolderWidth::as_str),
            Self::UnionLabels => joined_names(UnionLabelMatch::ALL, UnionLabelMatch::as_str),
            Self::MapEntries => joined_names(MapEntryOrder::ALL, MapEntryOrder::as_str),
            Self::EnumeratorValue => joined_names(EnumeratorValue::ALL, EnumeratorValue::as_str),
            Self::UnionFraming => {
                joined_names(MutableUnionFraming::ALL, MutableUnionFraming::as_str)
            }
            Self::ShortPidRange => joined_names(ShortPidRange::ALL, ShortPidRange::as_str),
            Self::FixedDigits => joined_names(FixedDigitCount::ALL, FixedDigitCount::as_str),
            Self::DefaultExtensibility => joined_names(Extensibility::ALL, Extensibility::as_str),
            Self::KeyHolder => {
                joined_names(KeyHolderExtensibility::ALL, KeyHolderExtensibility::as_str)
            }
            Self::KeyHolderOrder => joined_names(KeyHolderOrder::ALL, KeyHolderOrder::as_str),
            Self::KeyHolderAlignment => {
                joined_names(KeyHolderAlignment::ALL, KeyHolderAlignment::as_str)
            }
            Self::DefaultedDiscriminatorMember => joined_names(
                DefaultedDiscriminatorMember::ALL,
                DefaultedDiscriminatorMember::as_str,
            ),
            Self::MapLength => {
                joined_names([MAP_LENGTH_VALUES[0].0, MAP_LENGTH_VALUES[1].0], |name| {
                    name
                })
            }
            Self::ElementPadding => joined_names(ElementPadding::ALL, ElementPadding::as_str),
            Self::AggregateDelimiter => {
                joined_names(AggregateDelimiter::ALL, AggregateDelimiter::as_str)
            }
            Self::SequenceElementDelimiter => joined_names(
                SequenceElementDelimiter::ALL,
                SequenceElementDelimiter::as_str,
            ),
            Self::WstringCodeUnits => joined_names(WstringCodeUnits::ALL, WstringCodeUnits::as_str),
        }
    }

    /// Parse the axis value and return new settings with the axis
    /// applied. The dialect was already in `settings`; this overrides
    /// one of its answers.
    ///
    /// # Errors
    /// Returns a message naming the value if it was not recognised.
    pub fn parse_and_apply(
        self,
        settings: DialectSettings,
        value: &str,
    ) -> Result<DialectSettings, String> {
        let error = |kind: &str| format!("`{value}` is not a {kind}");
        let mut settings = settings;
        match self {
            Self::LengthCodes => {
                settings.key = settings.key.with_length_codes(
                    LengthCodePolicy::parse(value).ok_or_else(|| error("length-code policy"))?,
                );
            }
            Self::CollectionDelimiters => {
                settings.key = settings.key.with_collection_delimiters(
                    CollectionDelimiterPolicy::parse(value)
                        .ok_or_else(|| error("delimiter policy"))?,
                );
            }
            Self::WstringLength => {
                settings.key = settings.key.with_wstring_length(
                    WstringLengthPolicy::parse(value)
                        .ok_or_else(|| error("wstring-length policy"))?,
                );
            }
            Self::WcharFraming => {
                settings.key = settings.key.with_wchar_framing(
                    WcharFraming::parse(value).ok_or_else(|| error("wchar framing"))?,
                );
            }
            Self::WcharWidth => {
                settings.key = settings.key.with_wchar_width(
                    WcharWidth::parse(value).ok_or_else(|| error("wchar width"))?,
                );
            }
            Self::MapDelimiter => {
                settings.key = settings.key.with_map_delimiter(
                    MapDelimiter::parse(value).ok_or_else(|| error("map delimiter policy"))?,
                );
            }
            Self::WstringUnits => {
                settings.key = settings.key.with_wstring_unit_order(
                    WstringUnitOrder::parse(value).ok_or_else(|| error("wstring unit order"))?,
                );
            }
            Self::PlCdr1Headers => {
                settings.key = settings.key.with_pl_cdr1_headers(
                    PlCdr1HeaderPolicy::parse(value)
                        .ok_or_else(|| error("PL_CDR1 header policy"))?,
                );
            }
            Self::PidFlags => {
                settings.key = settings.key.with_pid_flags(
                    PidFlagPolicy::parse(value).ok_or_else(|| error("PID-flag policy"))?,
                );
            }
            Self::EnumHolder => {
                settings.key = settings.key.with_enum_holder(
                    EnumHolderWidth::parse(value).ok_or_else(|| error("enum-holder width"))?,
                );
            }
            Self::UnionLabels => {
                settings.key = settings.key.with_union_label(
                    UnionLabelMatch::parse(value).ok_or_else(|| error("union-label rule"))?,
                );
            }
            Self::MapEntries => {
                settings.key = settings.key.with_map_entries(
                    MapEntryOrder::parse(value).ok_or_else(|| error("map-entry order"))?,
                );
            }
            Self::EnumeratorValue => {
                settings.key = settings.key.with_enumerator_value(
                    EnumeratorValue::parse(value)
                        .ok_or_else(|| error("enumerator-value policy"))?,
                );
            }
            Self::UnionFraming => {
                settings.key = settings.key.with_union_framing(
                    MutableUnionFraming::parse(value).ok_or_else(|| error("union framing"))?,
                );
            }
            Self::ShortPidRange => {
                settings.key = settings.key.with_short_pid_range(
                    ShortPidRange::parse(value).ok_or_else(|| error("short-PID range"))?,
                );
            }
            Self::FixedDigits => {
                settings.key = settings.key.with_fixed_digits(
                    FixedDigitCount::parse(value).ok_or_else(|| error("fixed digit count"))?,
                );
            }
            Self::DefaultExtensibility => {
                settings.key = settings.key.with_default_extensibility(
                    Extensibility::parse(value).ok_or_else(|| error("extensibility"))?,
                );
            }
            Self::KeyHolder => {
                settings.key = settings.key.with_key_holder(
                    KeyHolderExtensibility::parse(value)
                        .ok_or_else(|| error("key-holder policy"))?,
                );
            }
            Self::KeyHolderOrder => {
                settings.key = settings.key.with_key_holder_order(
                    KeyHolderOrder::parse(value).ok_or_else(|| error("key-holder order"))?,
                );
            }
            Self::KeyHolderAlignment => {
                settings.key = settings.key.with_key_holder_alignment(
                    KeyHolderAlignment::parse(value)
                        .ok_or_else(|| error("key-holder alignment"))?,
                );
            }
            Self::DefaultedDiscriminatorMember => {
                settings.key = settings.key.with_defaulted_discriminator_member(
                    DefaultedDiscriminatorMember::parse(value)
                        .ok_or_else(|| error("defaulted-discriminator-member policy"))?,
                );
            }
            Self::MapLength => {
                let (_, with_delimiter) = MAP_LENGTH_VALUES
                    .into_iter()
                    .find(|(name, _)| *name == value)
                    .ok_or_else(|| error("map-length policy"))?;
                settings.map_length_with_delimiter = with_delimiter;
            }
            Self::ElementPadding => {
                settings.key = settings.key.with_element_padding(
                    ElementPadding::parse(value).ok_or_else(|| error("element-padding policy"))?,
                );
            }
            Self::AggregateDelimiter => {
                settings.key = settings.key.with_aggregate_delimiter(
                    AggregateDelimiter::parse(value)
                        .ok_or_else(|| error("aggregate-delimiter policy"))?,
                );
            }
            Self::SequenceElementDelimiter => {
                settings.key = settings.key.with_sequence_element_delimiter(
                    SequenceElementDelimiter::parse(value)
                        .ok_or_else(|| error("sequence-element-delimiter policy"))?,
                );
            }
            Self::WstringCodeUnits => {
                settings.key = settings.key.with_wstring_code_units(
                    WstringCodeUnits::parse(value).ok_or_else(|| error("wstring code units"))?,
                );
            }
        }
        Ok(settings)
    }
}

/// Every axis flag, in the order they are documented. The flags override
/// a dialect in this order, which is the order a caller is likely to
/// give them.
pub const AXES: [AxisFlag; 26] = [
    AxisFlag::LengthCodes,
    AxisFlag::CollectionDelimiters,
    AxisFlag::WstringLength,
    AxisFlag::WcharFraming,
    AxisFlag::WcharWidth,
    AxisFlag::MapDelimiter,
    AxisFlag::WstringUnits,
    AxisFlag::PlCdr1Headers,
    AxisFlag::PidFlags,
    AxisFlag::EnumHolder,
    AxisFlag::UnionLabels,
    AxisFlag::MapEntries,
    AxisFlag::EnumeratorValue,
    AxisFlag::UnionFraming,
    AxisFlag::ShortPidRange,
    AxisFlag::FixedDigits,
    AxisFlag::DefaultExtensibility,
    AxisFlag::KeyHolder,
    AxisFlag::KeyHolderOrder,
    AxisFlag::KeyHolderAlignment,
    AxisFlag::DefaultedDiscriminatorMember,
    AxisFlag::MapLength,
    AxisFlag::ElementPadding,
    AxisFlag::AggregateDelimiter,
    AxisFlag::SequenceElementDelimiter,
    AxisFlag::WstringCodeUnits,
];

/// Build the settings a dialect and a sequence of axis overrides add up
/// to, for the given encoding, endianness and purpose.
///
/// The dialect is applied first; each `(axis, value)` overrides one of
/// its answers. Unknown axis or value names are returned as errors.
///
/// # Errors
/// The first override that failed to parse, with a message naming the
/// value and the axis.
pub fn dialect_settings_from_overrides(
    dialect: Dialect,
    encoding: Encoding,
    endianness: Endianness,
    purpose: Purpose,
    overrides: &[(AxisFlag, &str)],
) -> Result<DialectSettings, String> {
    let mut settings = DialectSettings::in_dialect(dialect, encoding, endianness, purpose);
    for &(axis, value) in overrides {
        settings = axis.parse_and_apply(settings, value)?;
    }
    Ok(settings)
}

/// The valid dialect names, comma-separated, for an error message.
#[must_use]
pub fn dialect_names() -> String {
    Dialect::ALL
        .iter()
        .map(|d| d.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The help text for the `--dialect` flag, listing every dialect.
#[must_use]
pub fn dialect_help() -> String {
    let mut out = String::from("specification (the default)");
    for d in Dialect::ALL {
        if !d.is_default() {
            out.push_str(" | ");
            out.push_str(d.as_str());
        }
    }
    out
}

/// Lowering options a dialect controls, for a caller that needs to parse
/// IDL under a dialect.
///
/// Three lowering facts live on `Dialect` because they change member ids
/// and scoped names — things settled before any `LayoutKey` exists.
#[derive(Debug, Clone, Copy)]
pub struct DialectLowering {
    /// Whether a `@hashid` member advances the sequential counter.
    pub hashed_id_advances_the_counter: bool,
    /// Whether an escaped name is hashed as written.
    pub hashid_reads_the_escaped_spelling: bool,
    /// Whether a name may be the same as the enclosing scope's.
    pub admits_name_of_enclosing_scope: bool,
}

impl DialectLowering {
    /// The lowering options for `dialect`.
    #[must_use]
    pub const fn for_dialect(dialect: Dialect) -> Self {
        Self {
            hashed_id_advances_the_counter: dialect.hashed_id_advances_the_counter(),
            hashid_reads_the_escaped_spelling: dialect.hashid_reads_the_escaped_spelling(),
            admits_name_of_enclosing_scope: dialect.admits_the_name_of_the_enclosing_scope(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    fn default_settings() -> DialectSettings {
        DialectSettings::in_dialect(
            Dialect::default(),
            Encoding::PlainCdr2,
            Endianness::Little,
            Purpose::Data,
        )
    }

    #[test]
    fn every_axis_parses_back_its_own_as_str() {
        let settings = default_settings();
        for axis in AXES {
            // Each axis's help_values lists at least one value. Pick the
            // first one (before the first `|`) and parse it.
            let text = axis.help_values();
            let first_value = text.split(" | ").next().unwrap();
            let result = axis.parse_and_apply(settings, first_value);
            assert!(
                result.is_ok(),
                "axis {:?} failed to parse its own value `{first_value}`: {:?}",
                axis,
                result.err()
            );
        }
    }

    #[test]
    fn an_unknown_value_is_reported() {
        let settings = default_settings();
        let err = AxisFlag::LengthCodes
            .parse_and_apply(settings, "nonsense")
            .expect_err("should fail");
        assert!(err.contains("nonsense"), "{err}");
        assert!(err.contains("length-code"), "{err}");
    }

    #[test]
    fn dialect_first_then_override() {
        // FastDDS writes short-where-it-fits; overriding to compact should win.
        let overrides = [(AxisFlag::LengthCodes, "compact")];
        let settings = dialect_settings_from_overrides(
            Dialect::FastDds,
            Encoding::PlainCdr2,
            Endianness::Little,
            Purpose::Data,
            &overrides,
        )
        .expect("should parse");
        assert_eq!(settings.key.length_codes, LengthCodePolicy::Compact);
    }

    #[test]
    fn every_dialect_name_is_parseable() {
        for d in Dialect::ALL {
            let parsed = Dialect::parse(d.as_str());
            assert_eq!(parsed, Some(d), "{}", d.as_str());
        }
    }

    #[test]
    fn wstring_length_help_names_the_coordinate_form_once() {
        let text = AxisFlag::WstringLength.help_values();
        assert!(text.contains("specification"));
        assert!(text.contains("ros-type-support"));
        assert!(text.contains(WstringLengthPolicy::COORDINATE_FORM_HELP));
    }

    #[test]
    fn wchar_framing_help_names_every_measured_form() {
        // Regression: a hand-written list here once dropped
        // `giop-length-prefixed-member-only`, the third of
        // `WcharFraming::ALL`'s three forms.
        let text = AxisFlag::WcharFraming.help_values();
        for framing in WcharFraming::ALL {
            assert!(text.contains(framing.as_str()), "{text}");
        }
    }

    #[test]
    fn map_length_axis_overrides_the_dialects_answer() {
        let settings = default_settings();
        assert!(settings.map_length_with_delimiter);
        let overridden = AxisFlag::MapLength
            .parse_and_apply(settings, "rule-15-omits")
            .expect("known value");
        assert!(!overridden.map_length_with_delimiter);
        let opendds = DialectSettings::in_dialect(
            Dialect::OpenDds,
            Encoding::PlainCdr2,
            Endianness::Little,
            Purpose::Data,
        );
        assert!(!opendds.map_length_with_delimiter);
    }

    #[test]
    fn map_length_rejects_an_unknown_value() {
        let settings = default_settings();
        let err = AxisFlag::MapLength
            .parse_and_apply(settings, "nonsense")
            .expect_err("should fail");
        assert!(err.contains("nonsense"), "{err}");
    }
}
