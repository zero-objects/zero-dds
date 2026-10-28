// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The configuration matrix every column is measured over, and the Rust
//! column's answers under it.
//!
//! ## Why this is in the library
//!
//! The twelve configurations used to be written out four times — in
//! `crates/wire-probe/tests/three_column_gate.rs`, in
//! `tools/wireprobe/corpus-python.py`, in
//! `tools/wireprobe/wireprobe-zerodds-c-gen/src/bin/wireprobe-c-matrix.rs`
//! and in `tools/interop-results/src/rust_column.rs` — because each
//! column was driven by its own program in its own language, and two of
//! those could not read a Rust constant. `corpus-python.py` is gone with
//! `crates/lang-python`, the column it drove:
//! `tools/wireprobe/wireprobe-zerodds-python-gen`'s own matrix driver,
//! `wireprobe-python-matrix`, imports [`CONFIGURATIONS`] from this
//! module directly, the same way `three_column_gate.rs` does, rather
//! than holding a fifth copy that could drift from it — so the
//! duplication that justified this module's existence is down to two
//! foreign copies now, and no fifth *Rust* copy is needed either: a new
//! column's harness reads this module, and a configuration added here
//! reaches it without anybody remembering to add it.
//!
//! The two foreign copies stay, and stay checked:
//! `the_c_column_covers_the_same_configurations` in the three-column
//! gate reads `wireprobe-c-matrix.rs`'s out of its source text and fails
//! if it stops naming the same twelve in the same order; `rust_column.rs`
//! is checked by the four `the_configuration_is_named_the_same_everywhere`-
//! shaped tests spread over `crates/wire-probe/tests/*.rs`. A driver
//! written for a new column belongs in one of those checks the day it
//! exists — a list nothing compares is a list that drifts.
//!
//! ## What one row holds
//!
//! [`row`] answers 420 slots for one case: twelve configurations, five
//! encodings, two byte orders, and four emit forms.
//!
//! | slot | what it holds |
//! |---|---|
//! | `<config>/<encoding>/<order>` | the payload, as hex |
//! | `<config>/<encoding>/<order>+enc` | the same behind its encapsulation header |
//! | `<config>/<encoding>/<order>/read` | those octets read back, against the case's own sample |
//! | `<config>/<encoding>/keyhash` | the sixteen-byte KeyHash |
//!
//! The KeyHash has no byte-order axis: XTypes 1.3 §7.6.8 makes the
//! holder big-endian whatever the sample is.
//!
//! The read direction compares against the **sample** and never against
//! another column's reading of the same octets: two decoders wrong in
//! the same way agree with each other and disagree with the sample.

use std::collections::BTreeMap;

use serde_json::Value;
use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{
    Dialect, Encoding, Endianness, EnumHolderWidth, EnumeratorValue, FixedDigitCount,
    LengthCodePolicy, MapEntryOrder, MutableUnionFraming, PidFlagPolicy, ShortPidRange,
    UnionLabelMatch, WstringLengthPolicy, WstringUnitOrder,
};

use crate::canonical::{canonicalise, text};
use crate::decode::decode;
use crate::encode::{encode, hex, model_from_idl_in_dialect};
use crate::value::{Emit, MapLengthPolicy, ProbeRequest};

/// The five encodings, under the names every column keys its slots by.
pub const ENCODINGS: [(&str, Encoding); 5] = [
    ("plain-cdr1", Encoding::PlainCdr1),
    ("pl-cdr1", Encoding::PlCdr1),
    ("plain-cdr2", Encoding::PlainCdr2),
    ("delimited-cdr", Encoding::DelimitedCdr),
    ("pl-cdr2", Encoding::PlCdr2),
];

/// Both byte orders, under the names every column keys its slots by.
pub const ORDERS: [(&str, Endianness); 2] = [("le", Endianness::Little), ("be", Endianness::Big)];

/// One column of the matrix: a dialect, and the policy overrides on top
/// of it.
///
/// Ten of the twelve carry **no** override, because a dialect is the
/// whole of what a measured peer writes. The two that do are the two
/// settings no measured peer writes: an EMHEADER that always carries an
/// explicit length, and a delimited `map` with no entry count. A setting
/// nothing exercises is a setting that is only asserted to work.
pub struct Configuration {
    /// The name the slot is keyed by.
    pub name: &'static str,
    /// The preset every wire policy comes from.
    pub dialect: Dialect,
    /// Each of these overrides one of the dialect's answers, and only
    /// when it is `Some`: a flag left off must leave the dialect alone.
    pub length_codes: Option<LengthCodePolicy>,
    /// How a `wstring`'s length is counted.
    pub wstring_length: Option<WstringLengthPolicy>,
    /// The `wstring` code units' byte order, its own axis.
    pub wstring_unit_order: Option<WstringUnitOrder>,
    /// What flags a PL_CDR1 parameter id carries, per position.
    pub pid_flags: Option<PidFlagPolicy>,
    /// How wide an enumeration's holder is.
    pub enum_holder: Option<EnumHolderWidth>,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    pub union_label: Option<UnionLabelMatch>,
    /// In what order a `map`'s entries go on the wire.
    pub map_entries: Option<MapEntryOrder>,
    /// Which integer an enumerator serialises as.
    pub enumerator_value: Option<EnumeratorValue>,
    /// How a `@mutable` union numbers its branches, and whether its
    /// XCDR1 parameter list is terminated.
    pub union_framing: Option<MutableUnionFraming>,
    /// Which member ids fit a PL_CDR1 short parameter header.
    pub short_pid_range: Option<ShortPidRange>,
    /// Whether a `fixed<P,S>` carries its declared digit count or the
    /// value's.
    pub fixed_digits: Option<FixedDigitCount>,
    /// Whether a delimited `map` also carries an entry count.
    pub map_length: Option<MapLengthPolicy>,
}

impl Configuration {
    /// This configuration as a request: the dialect first, then the
    /// overrides, which is the precedence every column applies.
    #[must_use]
    pub fn request(
        &self,
        type_name: &str,
        encoding: Encoding,
        endianness: Endianness,
    ) -> ProbeRequest {
        let mut request = ProbeRequest::in_dialect(type_name, encoding, endianness, self.dialect);
        if let Some(policy) = self.length_codes {
            request.length_codes = policy;
        }
        if let Some(policy) = self.wstring_length {
            request.wstring_length = policy;
        }
        if let Some(order) = self.wstring_unit_order {
            request.wstring_unit_order = order;
        }
        if let Some(policy) = self.pid_flags {
            request.pid_flags = policy;
        }
        if let Some(policy) = self.enum_holder {
            request.enum_holder = policy;
        }
        if let Some(policy) = self.union_label {
            request.union_label = policy;
        }
        if let Some(policy) = self.map_entries {
            request.map_entries = policy;
        }
        if let Some(policy) = self.enumerator_value {
            request.enumerator_value = policy;
        }
        if let Some(policy) = self.union_framing {
            request.union_framing = policy;
        }
        if let Some(policy) = self.short_pid_range {
            request.short_pid_range = policy;
        }
        if let Some(policy) = self.fixed_digits {
            request.fixed_digits = policy;
        }
        if let Some(policy) = self.map_length {
            request.map_length = policy;
        }
        request
    }

    /// The flags a foreign column's driver takes to reproduce this
    /// configuration, spelled exactly as every probe accepts them.
    ///
    /// Built here rather than written out per column: the C driver
    /// dropped two flags once because the shell around it filtered what
    /// it passed on, and a column that silently plans under the wrong
    /// policy produces octets that look plausible. A caller that hands
    /// these straight through cannot drop one.
    #[must_use]
    pub fn flags(&self) -> Vec<String> {
        let mut out = vec![String::from("--dialect"), self.dialect.as_str().to_string()];
        let mut push = |flag: &str, value: String| {
            out.push(flag.to_string());
            out.push(value);
        };
        if let Some(policy) = self.length_codes {
            push("--length-codes", policy.as_str().to_string());
        }
        if let Some(policy) = self.wstring_length {
            push("--wstring-length", policy.as_str().to_string());
        }
        if let Some(order) = self.wstring_unit_order {
            push("--wstring-units", order.as_str().to_string());
        }
        if let Some(policy) = self.pid_flags {
            push("--pid-flags", policy.as_str().to_string());
        }
        if let Some(policy) = self.enum_holder {
            push("--enum-holder", policy.as_str().to_string());
        }
        if let Some(policy) = self.union_label {
            push("--union-labels", policy.as_str().to_string());
        }
        if let Some(policy) = self.map_entries {
            push("--map-entries", policy.as_str().to_string());
        }
        if let Some(policy) = self.enumerator_value {
            push("--enumerator-value", policy.as_str().to_string());
        }
        if let Some(policy) = self.union_framing {
            push("--union-framing", policy.as_str().to_string());
        }
        if let Some(policy) = self.short_pid_range {
            push("--short-pid-range", policy.as_str().to_string());
        }
        if let Some(policy) = self.fixed_digits {
            push("--fixed-digits", policy.as_str().to_string());
        }
        if let Some(policy) = self.map_length {
            push("--map-length", policy.as_str().to_string());
        }
        out
    }
}

/// The twelve: the ten dialects, then the two settings no dialect
/// selects.
pub const CONFIGURATIONS: [Configuration; 12] = [
    configuration("specification", Dialect::Specification),
    configuration("cyclone", Dialect::Cyclone),
    configuration("rti", Dialect::Rti),
    configuration("fastdds", Dialect::FastDds),
    configuration("omniorb", Dialect::OmniOrb),
    configuration("tao", Dialect::Tao),
    configuration("jacorb", Dialect::JacOrb),
    configuration("opendds", Dialect::OpenDds),
    configuration("ros-fastrtps", Dialect::RosFastRtps),
    configuration("ros-cyclone", Dialect::RosCyclone),
    Configuration {
        length_codes: Some(LengthCodePolicy::UniversalNextInt),
        ..configuration("universal-nextint", Dialect::Specification)
    },
    Configuration {
        map_length: Some(MapLengthPolicy::Rule15Omits),
        ..configuration("rule-15-omits", Dialect::Specification)
    },
];

/// A configuration that is a dialect and nothing else.
const fn configuration(name: &'static str, dialect: Dialect) -> Configuration {
    Configuration {
        name,
        dialect,
        length_codes: None,
        wstring_length: None,
        wstring_unit_order: None,
        pid_flags: None,
        enum_holder: None,
        union_label: None,
        map_entries: None,
        enumerator_value: None,
        union_framing: None,
        short_pid_range: None,
        fixed_digits: None,
        map_length: None,
    }
}

/// One corpus case, as every driver reads it.
pub struct Case {
    /// The directory name.
    pub name: String,
    /// The top-level type, by scoped name.
    pub type_name: String,
    /// The IDL source.
    pub idl: String,
    /// The sample.
    pub values: Value,
}

/// Every slot of the matrix for one case, as the Rust column answers it.
///
/// In process and not through `wireprobe-zerodds`: 145,740 process
/// launches would measure the loader. It is the same library the binary
/// is a shell around.
///
/// One model **per configuration** and not one for all twelve, because
/// one wire answer belongs to the front end rather than to the plan:
/// whether a `@hashid` member advances the sequential counter the next
/// member is numbered from. `Dialect::hashed_id_advances_the_counter`
/// carries the measurement; RTI is the one of the four that skips it.
/// Lowering once with the default and planning under several dialects
/// gave the RTI column Cyclone's member ids.
///
/// A case whose IDL does not lower fills every slot of that
/// configuration with the front end's own sentence rather than stopping
/// the run. The row still holds [`SLOTS_PER_CASE`] entries, so a caller
/// counting them sees a complete row of refusals instead of a short one
/// it would have to tell from a driver that answered nothing.
#[must_use]
pub fn row(case: &Case) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for settings in &CONFIGURATIONS {
        let ast = match model_from_idl_in_dialect(
            &case.idl,
            &format!("{}/type.idl", case.name),
            settings.dialect,
        ) {
            Ok(ast) => ast,
            Err(error) => {
                refuse_configuration(
                    &mut out,
                    settings.name,
                    &format!("the IDL does not lower: {error}"),
                );
                continue;
            }
        };
        let expected = expected_text(&ast, case);
        for (encoding_name, encoding) in ENCODINGS {
            for (order_name, endianness) in ORDERS {
                let request = settings.request(&case.type_name, encoding, endianness);
                let stem = format!("{}/{encoding_name}/{order_name}", settings.name);
                let payload = encode(&ast, &request, &case.values);
                out.insert(
                    format!("{stem}+enc"),
                    match encode(
                        &ast,
                        &request.clone().emitting(Emit::PayloadWithEncapsulation),
                        &case.values,
                    ) {
                        Ok(bytes) => hex(&bytes),
                        Err(error) => format!("!{error}"),
                    },
                );
                out.insert(
                    format!("{stem}/read"),
                    match &payload {
                        Ok(bytes) => read_back(
                            &ast,
                            &request,
                            bytes,
                            expected.as_deref().map_err(String::as_str),
                        ),
                        Err(error) => format!("!unwritten: {error}"),
                    },
                );
                out.insert(
                    stem,
                    match payload {
                        Ok(bytes) => hex(&bytes),
                        Err(error) => format!("!{error}"),
                    },
                );
            }
            // The holder is big-endian whatever the sample is —
            // XTypes 1.3 §7.6.8 — so the KeyHash has no byte-order axis.
            let request = settings
                .request(&case.type_name, encoding, Endianness::Big)
                .emitting(Emit::KeyHash);
            out.insert(
                format!("{}/{encoding_name}/keyhash", settings.name),
                match encode(&ast, &request, &case.values) {
                    Ok(bytes) => hex(&bytes),
                    Err(error) => format!("!{error}"),
                },
            );
        }
    }
    out
}

/// Fill one configuration's whole share of a row with one refusal.
///
/// Public because `crates/wire-probe/tests/three_column_gate.rs` builds
/// the same row in process and its baseline is compared against this
/// one, digest for digest: two spellings of the same refusal would make
/// the two rows differ over nothing.
///
/// The read slots say `unwritten` rather than repeating the reason,
/// because that is what happened to them: nothing was written, so there
/// was nothing to read and the reader was never asked. Telling the two
/// apart is what lets a comparison say "this reader refused" only when
/// it did.
pub fn refuse_configuration(out: &mut BTreeMap<String, String>, name: &str, message: &str) {
    let unwritten = format!("!unwritten: {message}");
    for (encoding_name, _) in ENCODINGS {
        for (order_name, _) in ORDERS {
            let stem = format!("{name}/{encoding_name}/{order_name}");
            out.insert(format!("{stem}+enc"), format!("!{message}"));
            out.insert(format!("{stem}/read"), unwritten.clone());
            out.insert(stem, format!("!{message}"));
        }
        out.insert(
            format!("{name}/{encoding_name}/keyhash"),
            format!("!{message}"),
        );
    }
}

/// How many slots one row holds.
///
/// Stated rather than counted after the fact, so a driver that answered
/// a different number is caught by arithmetic and not by a diff.
pub const SLOTS_PER_CASE: usize = CONFIGURATIONS.len() * ENCODINGS.len() * (ORDERS.len() * 3 + 1);

/// The case's own sample in canonical form, or why there is none.
///
/// The two ways this fails are different facts and are kept apart: a type
/// name that does not resolve is a corpus or model defect, and a sample that
/// does not canonicalise is a value defect. Collapsing both to `None` put
/// them in the same cell, where a missing type read as a bad sample.
fn expected_text(ast: &Ast, case: &Case) -> Result<String, String> {
    let name = ScopedName::absolute(
        case.type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect(),
    );
    let type_ = ast
        .find_by_scoped_name(&name)
        .map_err(|lookup| format!("the model has no {}: {lookup:?}", case.type_name))?
        .as_type()
        .ok_or_else(|| format!("{} names something that is not a type", case.type_name))?;
    canonicalise(ast, type_, &case.values, "sample")
        .map(|value| text(&value))
        .map_err(|error| format!("the sample does not canonicalise: {error}"))
}

/// One column's own octets back into that column, against the sample.
fn read_back(
    ast: &Ast,
    request: &ProbeRequest,
    bytes: &[u8],
    expected: Result<&str, &str>,
) -> String {
    let expected = match expected {
        Ok(text) => text,
        Err(why) => return format!("!{why}"),
    };
    match decode(ast, request, bytes) {
        Ok(value) if text(&value) == expected => "same".to_string(),
        Ok(value) => format!("different: {} for {expected}", text(&value)),
        Err(error) => format!("!{error}"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn the_twelve_are_named_once_each_and_in_the_gate_s_order() {
        let names: Vec<&str> = CONFIGURATIONS.iter().map(|entry| entry.name).collect();
        assert_eq!(
            names,
            [
                "specification",
                "cyclone",
                "rti",
                "fastdds",
                "omniorb",
                "tao",
                "jacorb",
                "opendds",
                "ros-fastrtps",
                "ros-cyclone",
                "universal-nextint",
                "rule-15-omits",
            ],
            "the order is the three-column gate's; a column keyed by position \
             would otherwise be measured under the wrong dialect"
        );
    }

    #[test]
    fn a_row_holds_four_hundred_and_twenty_slots() {
        assert_eq!(SLOTS_PER_CASE, 420);
    }

    /// Every axis value the layout key admits reaches the flag list.
    ///
    /// Not by reasoning about which columns touch which axis — that is
    /// how the Python generator came to be missing a switch two repaired
    /// callers already passed. Each override is set, the flags are
    /// built, and the value has to appear in them.
    #[test]
    fn every_override_reaches_the_flags() {
        let base = configuration("probe", Dialect::Specification);
        let with = Configuration {
            length_codes: Some(LengthCodePolicy::UniversalNextInt),
            map_length: Some(MapLengthPolicy::Rule15Omits),
            ..base
        };
        let flags = with.flags();
        assert!(flags.iter().any(|flag| flag == "--length-codes"));
        assert!(flags.iter().any(|flag| flag == "--map-length"));
        assert!(
            flags.iter().any(|flag| flag == "--dialect"),
            "the dialect is a preset and goes first, or the overrides are discarded"
        );
    }
}
