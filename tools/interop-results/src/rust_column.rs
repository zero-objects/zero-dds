// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust column, and the configuration sweep the mitigation column
//! is derived from.
//!
//! In process and not through `wireprobe-zerodds`: thirty-three
//! thousand process launches would measure the loader. It is the same
//! library the binary is a shell around.
//!
//! ## Why twelve configurations and not one
//!
//! The last column of both tables answers "which setting makes this
//! difference disappear". That answer is a **measurement**: for a
//! foreign byte string this sweeps the twelve configurations and
//! reports the one whose bytes are equal to it. A column that named
//! `--dialect fastdds` because the differing peer was Fast DDS would be
//! restating the question.
//!
//! The twelve are the twelve of
//! `crates/wire-probe/tests/three_column_gate.rs` — the **ten dialects**
//! plus the two settings no dialect selects — and
//! `the_configurations_are_the_gates` fails if the two lists stop
//! naming the same twelve.
//!
//! **It was twenty-three.** Eleven of those were a dialect plus one or
//! two overrides, and every one of them existed because some peer's
//! rows were not reachable by naming that peer: `bit-bound-dropped` was
//! Fast DDS's own enumeration holder, `map-ascending-key` its own map
//! order, `negative-label-unmatched` Cyclone's own union rule,
//! `fixed-value-precision` JacORB's own digit count, the two
//! `ros-type-support` columns the two ROS type supports, and six more
//! were OpenDDS, which had no dialect at all. Those settings are in the
//! dialects of the products whose behaviour they are now, and a column
//! that is byte-for-byte a dialect measures nothing the dialect does
//! not. What is left beside the ten is the two points no measured peer
//! writes: an EMHEADER that always carries an explicit length, and a
//! delimited `map` with no entry count.

use std::collections::BTreeMap;

use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{
    Dialect, Encoding, Endianness, EnumHolderWidth, EnumeratorValue, FixedDigitCount,
    LengthCodePolicy, MapEntryOrder, MutableUnionFraming, PidFlagPolicy, ShortPidRange,
    UnionLabelMatch, WstringLengthPolicy, WstringUnitOrder,
};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, encode_with_padding, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::{MapLengthPolicy, ProbeRequest};

use crate::Case;
use crate::ledger::Outcome;

/// One configuration: a dialect, and the policy overrides on top of it.
///
/// Ten of the twelve carry **no** override, because a dialect is now
/// the whole of what a measured peer writes;
/// `tools/interop-results/tests/dialect_completeness.rs` is that claim
/// as a gate. The two that do carry one are the two settings no peer
/// was measured writing, and each carries exactly one.
///
/// Those two sit on [`Dialect::Specification`], which is
/// [`Dialect::default`]. They sat on `Dialect::Cyclone` while that was
/// the default, and moving the default moved them: a column whose job is
/// to measure one point must not also carry a product's two departures,
/// or a row it reproduces is a row three settings could have explained.
pub struct Configuration {
    /// The name the three-column gate and the two foreign columns use.
    pub name: &'static str,
    /// How a reader reproduces it, spelled exactly as
    /// `wireprobe-zerodds` takes it.
    pub flag: &'static str,
    /// What the setting decides, for a reader who has to choose one.
    ///
    /// Beside the flag and not in a document elsewhere: a page that
    /// tells somebody to set a flag and does not say what the flag
    /// changes has moved the question rather than answered it. Each
    /// sentence is the one `crates/ast-layout/src/key.rs` gives for the
    /// policies the preset selects.
    pub what: &'static str,
    dialect: Dialect,
    length_codes: Option<LengthCodePolicy>,
    wstring_length: Option<WstringLengthPolicy>,
    /// The `wstring` code units' byte order, which used to be a value
    /// of `wstring_length` and is its own axis now.
    wstring_unit_order: Option<WstringUnitOrder>,
    /// What flags a PL_CDR1 parameter id carries, per position.
    pid_flags: Option<PidFlagPolicy>,
    /// How wide an enumeration's holder is.
    enum_holder: Option<EnumHolderWidth>,
    /// Whether a negative case label of a narrow signed discriminator
    /// selects its branch.
    union_label: Option<UnionLabelMatch>,
    /// In what order a `map`'s entries go on the wire.
    map_entries: Option<MapEntryOrder>,
    /// Which integer an enumerator serialises as.
    enumerator_value: Option<EnumeratorValue>,
    /// How a `@mutable` union numbers its branches, and whether its
    /// XCDR1 parameter list is terminated.
    union_framing: Option<MutableUnionFraming>,
    /// Which member ids fit a PL_CDR1 short parameter header.
    short_pid_range: Option<ShortPidRange>,
    /// Whether a `fixed<P,S>` carries its declared digit count or the
    /// value's.
    fixed_digits: Option<FixedDigitCount>,
    map_length: Option<MapLengthPolicy>,
}

/// The twelve, in the order the gate lists them. The first is the
/// default this project ships and the one the tree's `zerodds-*` columns
/// hold.
pub const CONFIGURATIONS: [Configuration; 12] = [
    Configuration {
        name: "specification",
        flag: "--dialect specification",
        what: "A clause-by-clause reading of XTypes 1.3, which is what the text says rather \
               than what any product does — and **what this project ships**. An unannotated \
               aggregate is `@appendable` (§7.3.1.2.1.8), collection delimiters follow rules 9 \
               and 12, a member's must-understand bit is written as declared and on every \
               `@key` member, a `wstring`'s code units go out in the stream's byte order, and a \
               negative case label of a narrow signed discriminator selects its branch. **No \
               measured implementation produces it in full**, which is the cost of the default \
               rather than an argument against it: a peer's number in this row is how much of \
               what that peer writes the text also writes.",
        dialect: Dialect::Specification,
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
    },
    Configuration {
        name: "cyclone",
        flag: "--dialect cyclone",
        what: "Eclipse Cyclone DDS 11.0.1 — a dialect like the other eight, and no longer the \
               default. An aggregate whose IDL carried no extensibility annotation is taken to \
               be `@final`, so it gains no delimiter header under encoding version 2; a \
               `@mutable` member under version 1 always takes the extended parameter header; an \
               EMHEADER takes the compact length code; a `wstring`'s code units go out in the \
               host's byte order; a negative case label of a signed discriminator narrower than \
               32 bits selects no branch at all. The last two are why the default moved.",
        dialect: Dialect::Cyclone,
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
    },
    Configuration {
        name: "rti",
        flag: "--dialect rti",
        what: "RTI Connext DDS 7.7. An unannotated aggregate is `@appendable`, so it gains a \
               four-octet delimiter header under encoding version 2; no collection carries a \
               delimiter of its own; a `@mutable` member's EMHEADER reuses the nested \
               delimiter as its length rather than writing one.",
        dialect: Dialect::Rti,
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
    },
    Configuration {
        name: "fastdds",
        flag: "--dialect fastdds",
        what: "eProsima Fast DDS with Fast CDR 2.3.5. An unannotated aggregate is \
               `@appendable`; an enumeration counts as a primitive when deciding whether a \
               collection is delimited; an EMHEADER takes the short length code wherever the \
               length fits in it; a member's must-understand bit is left off the wire.",
        dialect: Dialect::FastDds,
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
    },
    Configuration {
        name: "omniorb",
        flag: "--dialect omniorb",
        what: "omniORB 4.3.3 over CORBA CDR, which *is* encoding version 1 — CORBA 3.3 part 2 \
               §9.3. A two-octet transmission code set, GIOP 1.2's length-prefixed `wchar`, and \
               a byte-order mark inside every `wstring`.",
        dialect: Dialect::OmniOrb,
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
    },
    Configuration {
        name: "tao",
        flag: "--dialect tao",
        what: "TAO 2.5.24 over CORBA CDR. A **four**-octet transmission code set and GIOP 1.2's \
               length-prefixed `wchar`, which together make its `wstring` the one form no DDS \
               implementation writes: an octet count over wide units.",
        dialect: Dialect::Tao,
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
    },
    Configuration {
        name: "jacorb",
        flag: "--dialect jacorb",
        what: "JacORB 3.9 over CORBA CDR. A two-octet code set like omniORB's and GIOP 1.2's \
               `wchar`, with no byte-order mark: it reads the units big-endian whatever the \
               stream says.",
        dialect: Dialect::JacOrb,
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
    },
    Configuration {
        name: "opendds",
        flag: "--dialect opendds",
        what: "OpenDDS 3.34.0 over `OpenDDS::DCPS::Serializer`. Fast DDS's XCDR1 `@mutable` framing \
               with a parameter id no other implementation writes — `PID_IMPL_EXTENSION` and \
               `PID_MUST_UNDERSTAND` set on every member whatever the member declared — an XCDR2 \
               length code that is short where the serialised length fits and never reuses a nested \
               delimiter, `@key` implying must-understand as §7.2.2.4.4.4.8 requires, and a front end \
               that carries neither `@value` nor `@bit_bound` as far as a serialiser.",
        dialect: Dialect::OpenDds,
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
    },
    Configuration {
        name: "ros-fastrtps",
        flag: "--dialect ros-fastrtps",
        what: "`rosidl_typesupport_fastrtps_c` 2.2.2, ROS 2 Humble — not Fast DDS: it generates its own \
               serialisation. Encoding version 1 only, every message `@final` whatever its `.idl` says, \
               a `wstring` counted in characters over four-octet code units whose octets stay in the \
               host's order while the length swaps, and a four-octet scalar `wchar` that does swap.",
        dialect: Dialect::RosFastRtps,
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
    },
    Configuration {
        name: "ros-cyclone",
        flag: "--dialect ros-cyclone",
        what: "`rmw_cyclonedds_cpp` 1.3.4, ROS 2 Humble — not Cyclone: it walks the introspection type \
               support. The same `wstring` cell as `ros-fastrtps` over units in the stream's order, and \
               a **two**-octet scalar `wchar` beside those four-octet units, which is the one place in \
               the nine where the two widths part. It writes the host's byte order and takes no \
               argument for it, so half its column does not exist.",
        dialect: Dialect::RosCyclone,
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
    },
    Configuration {
        name: "universal-nextint",
        flag: "--length-codes universal-nextint",
        what: "One question, changed on its own: every `@mutable` member's EMHEADER carries an \
               explicit four-octet length whatever the member's size or shape. Four octets more \
               per member than any dialect writes, and no measured implementation writes it — \
               reach for it against a peer that expects that form, or to reproduce a recorded \
               capture octet for octet.",
        dialect: Dialect::Specification,
        length_codes: Some(LengthCodePolicy::UniversalNextInt),
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
    },
    Configuration {
        name: "rule-15-omits",
        flag: "--map-length rule-15-omits",
        what: "One question, changed on its own: a delimited `map` omits its entry count, which \
               is rule (15) read as written and is what OpenDDS's generated code does — its \
               `operator<<` writes the delimiter under version 2 and the count under version 1, \
               as alternatives rather than as both.",
        dialect: Dialect::Specification,
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
        map_length: Some(MapLengthPolicy::Rule15Omits),
    },
];

/// The default configuration: the first, which is what this project
/// ships and what the `zerodds-*` columns of the tree hold.
///
/// It is `specification` and not a product's name. What that costs and
/// why it is worth it is at `zerodds_ast_layout::Dialect`; what it means
/// here is that the number in a system's row against this configuration
/// is how much of what that system writes the **text** also writes, and
/// not how much of it a shipped product agrees with.
///
/// `tools/interop-results/tests/dialect_completeness.rs::
/// the_default_configuration_is_the_default_dialect` holds this string
/// to `Dialect::default`, so the two cannot part.
pub const DEFAULT: &str = "specification";

impl Configuration {
    fn request(&self, type_name: &str, encoding: Encoding, endianness: Endianness) -> ProbeRequest {
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
}

/// The five encodings under the names every probe takes, beside the
/// values the plan takes.
///
/// One list and not a `match` with an unreachable arm: an encoding
/// added to [`crate::ENCODINGS`] and forgotten here would reach a
/// panic at run time, and [`tests::the_encodings_are_the_crates`]
/// fails at build time instead.
const ENCODINGS: [(&str, Encoding); 5] = [
    ("plain-cdr1", Encoding::PlainCdr1),
    ("pl-cdr1", Encoding::PlCdr1),
    ("plain-cdr2", Encoding::PlainCdr2),
    ("delimited-cdr", Encoding::DelimitedCdr),
    ("pl-cdr2", Encoding::PlCdr2),
];

const ORDERS: [(&str, Endianness); 2] = [("le", Endianness::Little), ("be", Endianness::Big)];

/// What one case answered in one configuration, keyed by
/// `<encoding>/<endian>`.
pub type Slots = BTreeMap<String, (Outcome, Outcome)>;

/// Every configuration's answer for one case.
pub type CaseSweep = BTreeMap<&'static str, Slots>;

/// One case through all twelve configurations, both directions.
///
/// The read direction compares against the case's **sample**, never
/// against another column's reading of the same bytes: two decoders
/// wrong in the same way agree with each other and disagree with the
/// sample.
///
/// # A case the front end refuses
///
/// `zerodds_wire_probe::REFUSED_BY_THE_FRONT_END` names two corpus
/// cases the IDL front end refuses outright — `recursive-array` under
/// IDL 4.2 §7.4.1.4.4.4.4 and `resolve-module-named-as-type` under
/// §7.5.2 — and both are in the corpus on purpose, with a README saying
/// every implementation must refuse the file. That is a **result**, not
/// a corpus defect: the row carries it as `Outcome::Refused` in the
/// write direction and `Outcome::Unwritten` in the read direction, the
/// same shape any other refusal takes. `zerodds_wire_probe::matrix::row`
/// fills its slots the same way for the same two cases.
///
/// Returning `Err` for them stopped the whole ledger from being built
/// over two cells it has a form for.
///
/// # Errors
///
/// When a case's IDL does not lower and the case is not one of those
/// two, which is a defect in the corpus and not a result.
pub fn sweep(case: &Case) -> Result<CaseSweep, String> {
    let refused_by_the_front_end =
        zerodds_wire_probe::REFUSED_BY_THE_FRONT_END.contains(&case.name.as_str());
    let mut out = CaseSweep::new();
    for settings in &CONFIGURATIONS {
        // One model per configuration, because one wire answer belongs
        // to the front end and not to the plan: whether a `@hashid`
        // member advances the sequential counter the next member is
        // numbered from. RTI is the one of the four that skips it, and
        // lowering once with the default would give the RTI column
        // Cyclone's member ids.
        let ast = model_from_idl_in_dialect(
            &case.idl,
            &format!("{}/type.idl", case.name),
            settings.dialect,
        )
        .map_err(|error| format!("{}: the IDL does not lower: {error}", case.name));
        let ast = match ast {
            Ok(ast) => ast,
            Err(message) if refused_by_the_front_end => {
                let mut slots = Slots::new();
                for (encoding, _) in ENCODINGS {
                    for (endian, _) in ORDERS {
                        slots.insert(
                            format!("{encoding}/{endian}"),
                            (Outcome::Refused(message.clone()), Outcome::Unwritten),
                        );
                    }
                }
                out.insert(settings.name, slots);
                continue;
            }
            Err(message) => return Err(message),
        };
        let expected = expected_text(&ast, case);
        let mut slots = Slots::new();
        for (encoding, encoding_value) in ENCODINGS {
            for (endian, endianness) in ORDERS {
                let request = settings.request(&case.type_name, encoding_value, endianness);
                let (write, read) = match encode(&ast, &request, &case.values) {
                    Ok(bytes) => {
                        let read = match expected.as_deref() {
                            None => {
                                Outcome::Refused("the sample does not canonicalise".to_string())
                            }
                            Some(expected) => match decode(&ast, &request, &bytes) {
                                Ok(value) if text(&value) == expected => Outcome::Same,
                                Ok(value) => {
                                    Outcome::Different(format!("{} for {expected}", text(&value)))
                                }
                                Err(error) => Outcome::Refused(error.to_string()),
                            },
                        };
                        (Outcome::Bytes(hex(&bytes)), read)
                    }
                    Err(error) => (Outcome::Refused(error.to_string()), Outcome::Unwritten),
                };
                slots.insert(format!("{encoding}/{endian}"), (write, read));
            }
        }
        out.insert(settings.name, slots);
    }
    Ok(out)
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

/// Which octets of this project's own byte string carry no value,
/// computed from the layout.
///
/// The page's octet rule reads "padding" off a byte-for-byte
/// difference: same length, and every differing octet one this project
/// wrote as `00`. That is an observation about what this project wrote,
/// not a statement about where the fill positions are — `00` is also
/// what a zero value looks like, and what a length of zero looks like.
///
/// This is the other criterion. `Writer::align_to` is the one place the
/// walk emits an octet XTypes 1.3 §7.4.1's alignment rules put there
/// instead of a value, and it notes the span; the mask below is that
/// record, one entry per octet of the byte string, true where the plan
/// says fill. A peer's differing octet outside the mask is a difference
/// in a value however it looks, and a row carrying one is a divergence.
///
/// `None` when this project writes nothing for the case under this
/// configuration — there is then no plan to ask, and the row is not one
/// the comparison classifies either way.
#[must_use]
pub fn padding_mask(
    case: &Case,
    configuration: &str,
    encoding: &str,
    endian: &str,
) -> Option<Vec<bool>> {
    let settings = CONFIGURATIONS
        .iter()
        .find(|configuration_| configuration_.name == configuration)?;
    let (_, encoding_value) = ENCODINGS.iter().find(|(name, _)| *name == encoding)?;
    let (_, endianness) = ORDERS.iter().find(|(name, _)| *name == endian)?;
    let ast = model_from_idl_in_dialect(
        &case.idl,
        &format!("{}/type.idl", case.name),
        settings.dialect,
    )
    .ok()?;
    let request = settings.request(&case.type_name, *encoding_value, *endianness);
    encode_with_padding(&ast, &request, &case.values)
        .ok()
        .map(|(_, mask)| mask)
}

/// Whether every octet at which `theirs` differs from `mine` is a fill
/// position of the plan `mine` was written under.
///
/// The layout half of the padding classification, and independent of
/// the octet half: this asks the plan where the fill is, the octet rule
/// asks what this project happened to write. Both have to hold.
///
/// `false` when the two strings are of different lengths, when there is
/// no plan to ask, or when the plan's byte string is not the one it was
/// given — a mask that does not belong to `mine` answers a different
/// question.
#[must_use]
pub fn differs_only_where_the_plan_pads(
    mine: &str,
    theirs: &str,
    case: &Case,
    configuration: &str,
    encoding: &str,
    endian: &str,
) -> bool {
    if mine.len() != theirs.len() || mine.len() % 2 != 0 {
        return false;
    }
    let Some(mask) = padding_mask(case, configuration, encoding, endian) else {
        return false;
    };
    if mask.len() * 2 != mine.len() {
        return false;
    }
    let (ours, them) = (mine.as_bytes(), theirs.as_bytes());
    (0..mask.len()).all(|octet| {
        ours[octet * 2..octet * 2 + 2] == them[octet * 2..octet * 2 + 2] || mask[octet]
    })
}

/// Whether a foreign byte string decodes, under one configuration, to
/// the case's own sample.
///
/// The measurement behind "this difference reaches no reader". The page
/// classifies a row as padding content from the octets — same length,
/// every differing octet one this project writes as `00` — and that is
/// a claim about what a *reader* sees. This is the claim being checked
/// rather than asserted: the peer's own octets are put through this
/// project's decoder under the plan its bytes were compared against,
/// and the values that come out have to be the case's own.
///
/// `false` when the decode refuses, which is the right answer: a byte
/// string a reader cannot read has certainly reached it.
///
/// # Errors
///
/// None. A case whose IDL does not lower, or whose sample does not
/// canonicalise, answers `false` — a row nothing can be said about is
/// not a row where nothing changes.
#[must_use]
pub fn reads_back_as_the_sample(
    case: &Case,
    configuration: &str,
    encoding: &str,
    endian: &str,
    hex: &str,
) -> bool {
    let Some(settings) = CONFIGURATIONS
        .iter()
        .find(|configuration_| configuration_.name == configuration)
    else {
        return false;
    };
    let Some((_, encoding_value)) = ENCODINGS.iter().find(|(name, _)| *name == encoding) else {
        return false;
    };
    let Some((_, endianness)) = ORDERS.iter().find(|(name, _)| *name == endian) else {
        return false;
    };
    let Ok(ast) = model_from_idl_in_dialect(
        &case.idl,
        &format!("{}/type.idl", case.name),
        settings.dialect,
    ) else {
        return false;
    };
    let Some(expected) = expected_text(&ast, case) else {
        return false;
    };
    let Ok(bytes) = unhex(hex) else {
        return false;
    };
    let request = settings.request(&case.type_name, *encoding_value, *endianness);
    matches!(decode(&ast, &request, &bytes), Ok(value) if text(&value) == expected)
}

/// Octets from their hexadecimal, refusing anything that is not hex.
///
/// A malformed cell has to fail here rather than decode as something
/// shorter: `corba-corpus.tsv` keeps refusals in the same column as the
/// octets behind an `ERR1:` prefix.
fn unhex(hex: &str) -> Result<Vec<u8>, ()> {
    if hex.len() % 2 != 0 {
        return Err(());
    }
    (0..hex.len() / 2)
        .map(|at| u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).map_err(|_| ()))
        .collect()
}

/// Which configuration reproduces `hex` for this case, encoding and
/// byte order, and how a reader asks for it.
///
/// `None` when no configuration does, which is the finding the
/// mitigation column exists to make visible: a divergence a setting
/// covers and a divergence nothing covers are different findings, and
/// only one of them is reproducible.
#[must_use]
pub fn reproduced_by<'a>(
    sweep: &'a CaseSweep,
    encoding: &str,
    endian: &str,
    hex: &str,
) -> Option<&'a Configuration> {
    let slot = format!("{encoding}/{endian}");
    CONFIGURATIONS.iter().find(|configuration| {
        sweep
            .get(configuration.name)
            .and_then(|slots| slots.get(&slot))
            .is_some_and(|(write, _)| matches!(write, Outcome::Bytes(bytes) if bytes == hex))
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::{CONFIGURATIONS, ENCODINGS, ORDERS};

    /// The five encodings here are the five the rest of this tool
    /// names, in the same order.
    #[test]
    fn the_encodings_are_the_crates() {
        let here: Vec<&str> = ENCODINGS.iter().map(|(name, _)| *name).collect();
        assert_eq!(here, crate::ENCODINGS.to_vec());
        let orders: Vec<&str> = ORDERS.iter().map(|(name, _)| *name).collect();
        assert_eq!(orders, crate::ORDERS.to_vec());
    }

    /// The twelve here are the twelve the gate measures.
    ///
    /// The gate's list, `tools/wireprobe/corpus-python.py`'s and
    /// `wireprobe-c-matrix`'s are already held equal to each other by
    /// the gate itself. This is the fourth copy, and it is the one the
    /// mitigation column is derived from — a configuration the tables
    /// know about and the columns do not would name a flag no column
    /// was ever measured under.
    #[test]
    fn the_configurations_are_the_gates() {
        let gate = std::fs::read_to_string(
            crate::repository_root()
                .join("crates")
                .join("wire-probe")
                .join("tests")
                .join("three_column_gate.rs"),
        )
        .expect("the three-column gate");
        for configuration in &CONFIGURATIONS {
            assert!(
                gate.contains(&format!("name: \"{}\"", configuration.name)),
                "`{}` is a configuration here and not in the three-column gate",
                configuration.name
            );
        }
        let in_gate = gate.matches("        name: \"").count();
        assert_eq!(
            in_gate,
            CONFIGURATIONS.len(),
            "the gate has {in_gate} configurations and this has {}",
            CONFIGURATIONS.len()
        );
    }
}
