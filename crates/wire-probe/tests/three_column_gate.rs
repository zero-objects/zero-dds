// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Do the Rust, C and Python columns agree — in both directions, over
//! the whole corpus, in every configuration?
//!
//! Until this file, the answer lived in three agents' reports written at
//! three different times against three different states of the tree.
//! That is how the Python column came to be recorded as measured when it
//! had never been run under a non-default dialect: each sentence was
//! true when it was written and nobody re-ran it. A claim about column
//! equality has to be a test.
//!
//! ## What is compared
//!
//! Every case in `tests/interop-corpus/`, in every one of the
//! [`CONFIGURATIONS`] below, in all five encodings and both byte orders,
//! in both directions, in three emit forms:
//!
//! | slot | what it holds |
//! |---|---|
//! | `<config>/<encoding>/<order>` | the payload, as hex |
//! | `<config>/<encoding>/<order>+enc` | the same behind its encapsulation header |
//! | `<config>/<encoding>/<order>/read` | that column's own bytes read back, compared against the case's sample |
//! | `<config>/<encoding>/keyhash` | the sixteen-byte KeyHash |
//!
//! The read direction compares against the **sample**, never against
//! another column's reading of the same bytes: two decoders wrong in the
//! same way agree with each other and disagree with the sample. The one
//! setting that cannot round-trip by construction is named in
//! [`READS_BACK_DIFFERENTLY`], per configuration and per case, and there
//! the three columns have to be different in the same way.
//!
//! All three comparisons are made in one currency: the canonical text of
//! `crates/wire-probe/src/canonical.rs`. Both foreign columns'
//! matrix drivers — `wireprobe-c-matrix` and `wireprobe-python-matrix` —
//! canonicalise their own read direction against the same module before
//! this gate ever sees it, rather than handing back a bare document for
//! this file to judge. A column that reached its own verdict would be
//! judged against its own idea of what a value is, and
//! a value its decoder can build which no writer of its own would emit
//! would fall outside the comparison entirely. That is not a
//! hypothetical: it is the shape of the `fixed` defect this gate found.
//!
//! ## The refusals are part of the claim
//!
//! Where one column refuses, all three must refuse. A case one column
//! produces and another refuses is the defect this gate exists to catch
//! — it is how the C column's `enum-bitbound-8-value` disagreement
//! surfaced. Three implementations write three sentences for the same
//! refusal, so what is compared is the **class** ([`Reason`]) and, where
//! two messages both carry one, the path. The sentences themselves are
//! reported and not asserted; `crates/wire-probe/tests/fixtures/three_column_matrix.md`
//! records which classes each column reached.
//!
//! ## Two tiers, and what the fast one does not cover
//!
//! Set by `ZERODDS_GATE_TIER`, `fast` by default.
//!
//! - **fast** — the Rust and Python columns over the whole matrix, and
//!   the C column over [`representative`] cases: one per construct
//!   family plus every case any column has ever disagreed on. It is what
//!   `cargo test --workspace` runs.
//! - **full** — the same with the C column over every case.
//!
//! The split is written down rather than left to emerge from a timeout.
//! What `fast` does not cover is named in one place — [`representative`]
//! — and the fixture states it in prose beside the numbers, because a
//! gap nobody wrote down is a gap nobody knows about.
//!
//! `tools/wireprobe/three-column-gate` is the one command that runs
//! either tier and builds what each needs.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{
    Dialect, Encoding, Endianness, EnumHolderWidth, EnumeratorValue, FixedDigitCount,
    LengthCodePolicy, MapEntryOrder, MutableUnionFraming, PidFlagPolicy, ShortPidRange,
    UnionLabelMatch, WstringLengthPolicy, WstringUnitOrder,
};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::matrix::CONFIGURATIONS as SHARED_CONFIGURATIONS;
use zerodds_wire_probe::value::{Emit, MapLengthPolicy, ProbeRequest};

/// The five encodings, under the names all three columns use.
const ENCODINGS: [(&str, Encoding); 5] = [
    ("plain-cdr1", Encoding::PlainCdr1),
    ("pl-cdr1", Encoding::PlCdr1),
    ("plain-cdr2", Encoding::PlainCdr2),
    ("delimited-cdr", Encoding::DelimitedCdr),
    ("pl-cdr2", Encoding::PlCdr2),
];

const ORDERS: [(&str, Endianness); 2] = [("le", Endianness::Little), ("be", Endianness::Big)];

/// One column of the matrix: a dialect, and the policy overrides on top
/// of it.
///
/// The **ten** dialects, then the two settings no dialect selects —
/// `LengthCodePolicy::UniversalNextInt` and
/// `MapLengthPolicy::Rule15Omits`. `crates/ast-layout/src/key.rs`
/// enumerates the policies; those two are the ones [`Dialect::ALL`]
/// never reaches, and a setting nothing exercises is a setting that is
/// only asserted to work.
///
/// It was eleven settings a moment ago, and the eleven were the
/// symptom: `EnumHolderWidth::BitBoundDroppedByFrontEnd`,
/// `UnionLabelMatch::NegativeLabelUnmatchedBelow32`,
/// `MapEntryOrder::AscendingKey`, `FixedDigitCount::ValuePrecision`,
/// `PidFlagPolicy::ImplExtensionOnMembers`,
/// `WstringUnitOrder::HostUnswapped`,
/// `WstringLengthPolicy::ROS_TYPE_SUPPORT` and four more all describe
/// what a **measured product** puts on the wire, and each of them is in
/// that product's dialect now.
///
/// The three CORBA presets are here for the axes they are the only
/// selectors of: `WcharFraming::GiopLengthPrefixed` in all three,
/// `WstringUnitOrder::ByteOrderMark` in `omniorb`,
/// `WstringUnitOrder::BigEndianAlways` in `jacorb` and
/// `WstringLengthPolicy::SPECIFICATION_WIDE_UNITS` in `tao`. An ORB has no
/// encoding version 2, so the XCDR2 half of those rows reproduces no
/// implementation — it is here because the three columns still have to
/// agree with each other, and a policy the C column implemented
/// differently would otherwise be caught only by an ORB nobody runs in
/// this test.
///
/// The list is duplicated in
/// `tools/wireprobe/wireprobe-zerodds-c-gen/src/bin/wireprobe-c-matrix.rs`,
/// because that column is driven by its own program in its own
/// language and cannot read a Rust constant; Python's own matrix driver
/// has no such copy to drift; it imports
/// [`zerodds_wire_probe::matrix::CONFIGURATIONS`] directly. It is not
/// left to trust: [`the_c_column_covers_the_same_configurations`] reads
/// the C list out of its literal and fails if it stops naming the same
/// twelve in the same order, and a second assertion in the same test
/// holds this file's own copy to the shared module's.
struct Configuration {
    name: &'static str,
    dialect: Dialect,
    length_codes: Option<LengthCodePolicy>,
    wstring_length: Option<WstringLengthPolicy>,
    /// The `wstring` code units' byte order, its own axis now.
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

const CONFIGURATIONS: [Configuration; 12] = [
    Configuration {
        name: "specification",
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

/// Which tier this run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    /// The C column over [`representative`] cases only.
    Fast,
    /// Every column over every case.
    Full,
}

impl Tier {
    fn from_environment() -> Self {
        match std::env::var("ZERODDS_GATE_TIER").as_deref() {
            Ok("full") => Self::Full,
            Ok("fast") | Err(_) => Self::Fast,
            Ok(other) => panic!("`{other}` is not a tier; it is `fast` or `full`"),
        }
    }
}

/// One corpus case.
struct Case {
    name: String,
    type_name: String,
    idl: String,
    values: Value,
}

impl Case {
    /// The model, lowered under one dialect.
    ///
    /// Under a dialect and not once for all four: see [`rust_case`].
    ///
    /// `Err` and not a panic: `zerodds_wire_probe::REFUSED_BY_THE_FRONT_END`
    /// names two corpus cases the IDL front end refuses outright, and a
    /// case with no model still owes the row its slots. Filling them
    /// with the refusal is what `zerodds_wire_probe::matrix::row` does
    /// for the same two cases, and the two rows are compared against one
    /// baseline.
    fn model(&self, dialect: Dialect) -> Result<Ast, String> {
        model_from_idl_in_dialect(&self.idl, &format!("{}/type.idl", self.name), dialect)
            .map_err(|error| format!("{error}"))
    }
}

/// Why a column would not produce a slot.
///
/// Three implementations write three sentences for one refusal, so the
/// class is what is compared and the sentence is what is reported. The
/// classes are chosen so that each names a different *kind* of answer, a
/// caller would act on each differently, and no two of them could be
/// confused for one another in a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Reason {
    /// The binding has no shape for the construct: C has no `map`, no
    /// `bitset` and no `fixed`, §7.5.1 gives it none. The generated
    /// source not compiling is the same finding reached later.
    NoMapping,
    /// The sample holds a value the type cannot carry — a `char` above
    /// U+00FF, a `wchar` above the BMP, a string past its bound.
    ValueOutsideTheType,
    /// The type declares no `@key`, so it has no KeyHash. Not a defect:
    /// a keyless type has one instance.
    NoKey,
    /// The read direction had no bytes to read, because the write
    /// direction refused. Says nothing about the reader.
    Unwritten,
    /// A refusal none of the above names. Reported in full; a class that
    /// keeps appearing here is one this enumeration is missing.
    Other,
}

impl Reason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NoMapping => "no mapping",
            Self::ValueOutsideTheType => "value outside the type",
            Self::NoKey => "no @key",
            Self::Unwritten => "unwritten",
            Self::Other => "other",
        }
    }
}

/// What one column answered for one slot.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Answer {
    /// Bytes, as lower-case hex.
    Bytes(String),
    /// The read direction returned the sample that went in.
    Same,
    /// The read direction returned something else. The detail is
    /// reported and not compared: three columns print a value three
    /// ways, and a difference is a failure whatever it is spelled like.
    Different(String),
    /// The column would not produce this slot.
    Refused { reason: Reason, message: String },
}

impl Answer {
    /// Read one column's JSON cell.
    ///
    /// The three columns write the same alphabet: hex, `same`,
    /// `different: …`, or `!` and a sentence.
    fn parse(cell: &str) -> Self {
        if let Some(message) = cell.strip_prefix('!') {
            return Self::Refused {
                reason: classify(message),
                message: message.to_string(),
            };
        }
        if cell == "same" {
            return Self::Same;
        }
        if let Some(detail) = cell.strip_prefix("different: ") {
            return Self::Different(detail.to_string());
        }
        Self::Bytes(cell.to_string())
    }

    /// What kind of answer this is, for the fixture's tally.
    const fn kind(&self) -> &'static str {
        match self {
            Self::Bytes(_) => "bytes",
            Self::Same => "same",
            Self::Different(_) => "different",
            Self::Refused { .. } => "refused",
        }
    }
}

/// Which class a refusal belongs to.
///
/// Text matching, because the three columns are three programs in three
/// languages and the only thing they share is the English. Each pattern
/// is anchored on a phrase the producing code owns — `ProbeError`'s
/// `Display`, `zdw_status_name`, the Python runtime's exception text —
/// rather than on a substring that could drift into another message.
fn classify(message: &str) -> Reason {
    // Backticks out first: one column quotes the identifier and another
    // does not, and the quoting is not the finding.
    let lower = message.to_ascii_lowercase().replace('`', "");
    if lower.starts_with("unwritten") {
        return Reason::Unwritten;
    }
    if lower.contains("no @key") {
        return Reason::NoKey;
    }
    if lower.contains("has no c mapping")
        || lower.contains("has no mapping")
        || lower.contains("is not supported")
        || lower.contains("unsupported")
        || lower.contains("does not compile")
        || lower.contains("no rule")
    {
        return Reason::NoMapping;
    }
    if lower.contains("expected a character below")
        || lower.contains("multilingual plane")
        // `crates/wire/src/writer.rs`'s own `WireError::OutOfRange` and
        // `runtime/python/zerodds_wire/writer.py`'s `OutOfRange` agree on
        // "a BMP character, not U+…" for a `wchar` above the Basic
        // Multilingual Plane — the runtime's own wording, reached when a
        // column refuses at write time rather than pre-validating the
        // sample the way `wireprobe-zerodds-c-gen/src/fill.rs` does. Both
        // are the same finding under two spellings of the same clause.
        || lower.contains("bmp character")
        || lower.contains("does not fit")
        || lower.contains("is longer than")
        || lower.contains("out of range")
        || lower.contains("outofrange")
    {
        return Reason::ValueOutsideTheType;
    }
    Reason::Other
}

// -- the Rust column ------------------------------------------------------

/// The Rust column, in process, for one case.
///
/// In process and not through `wireprobe-zerodds`: eighty thousand
/// process launches would measure the loader. It is the same library the
/// binary is a shell around.
fn rust_case(case: &Case) -> BTreeMap<String, String> {
    let mut row = BTreeMap::new();
    for settings in &CONFIGURATIONS {
        // One model per configuration, because one wire answer belongs
        // to the **front end** and not to the plan: whether a `@hashid`
        // member advances the sequential counter the next member is
        // numbered from. `Dialect::hashed_id_advances_the_counter` has
        // the measurement; RTI is the one of the four that skips it.
        // Lowering once with the default and planning under four
        // dialects gave the RTI column Cyclone's member ids, and the
        // other two columns — whose generators take the dialect at
        // lowering time — wrote the right ones.
        let ast = match case.model(settings.dialect) {
            Ok(ast) => ast,
            Err(error) => {
                zerodds_wire_probe::matrix::refuse_configuration(
                    &mut row,
                    settings.name,
                    &format!("the IDL does not lower: {error}"),
                );
                continue;
            }
        };
        let type_ = ast
            .find_by_scoped_name(&ScopedName::absolute(
                case.type_name
                    .split("::")
                    .filter(|segment| !segment.is_empty())
                    .map(std::string::ToString::to_string)
                    .collect(),
            ))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type);
        let expected = type_.and_then(|type_| {
            canonicalise(&ast, type_, &case.values, "sample")
                .ok()
                .map(|value| text(&value))
        });
        for (encoding_name, encoding) in ENCODINGS {
            for (order_name, endianness) in ORDERS {
                let request = settings.request(&case.type_name, encoding, endianness);
                let stem = format!("{}/{encoding_name}/{order_name}", settings.name);
                let payload = encode(&ast, &request, &case.values);
                row.insert(
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
                row.insert(
                    format!("{stem}/read"),
                    match &payload {
                        Ok(bytes) => read_back(&ast, &request, bytes, expected.as_deref()),
                        Err(error) => format!("!unwritten: {error}"),
                    },
                );
                row.insert(
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
            row.insert(
                format!("{}/{encoding_name}/keyhash", settings.name),
                match encode(&ast, &request, &case.values) {
                    Ok(bytes) => hex(&bytes),
                    Err(error) => format!("!{error}"),
                },
            );
        }
    }
    row
}

impl Configuration {
    /// This configuration as a request: the dialect first, then the
    /// overrides, which is the precedence every column applies.
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

/// The Rust column's own bytes back into the Rust column.
fn read_back(ast: &Ast, request: &ProbeRequest, bytes: &[u8], expected: Option<&str>) -> String {
    let Some(expected) = expected else {
        return "!the sample does not canonicalise".to_string();
    };
    match decode(ast, request, bytes) {
        Ok(value) if text(&value) == expected => "same".to_string(),
        Ok(value) => format!("different: {} for {expected}", text(&value)),
        Err(error) => format!("!{error}"),
    }
}

// -- the corpus -----------------------------------------------------------

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the repository root above crates/wire-probe")
        .to_path_buf()
}

fn corpus() -> Vec<Case> {
    let directory = repository_root().join("tests").join("interop-corpus");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|entry| entry.expect("a corpus entry").path())
        .filter(|path| path.join("type").is_file())
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|directory| {
            let name = directory
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a case name")
                .to_string();
            let read = |file: &str| {
                std::fs::read_to_string(directory.join(file))
                    .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
            };
            let values_text = read("values.json");
            Case {
                type_name: read("type").trim().to_string(),
                idl: read("type.idl"),
                values: serde_json::from_str(&values_text)
                    .unwrap_or_else(|error| panic!("{name}/values.json: {error}")),
                name,
            }
        })
        .collect()
}

/// Which cases the fast tier runs the C column over.
///
/// One per construct family — the name up to the first `-`, which is how
/// the corpus is organised — plus every case listed in
/// [`EVER_DISAGREED`]. Both halves matter: the families keep a new
/// divergence in an untouched construct from going unseen for a whole
/// night, and the second list keeps a divergence that was found once
/// from coming back unnoticed.
///
/// What this leaves uncovered until the full tier: the other cases of
/// each family, which differ from their representative in the
/// *parameters* of the construct and not in the construct. A `map` whose
/// key is a `char` rather than a `long` is the same C refusal by the
/// same rule; a `sequence<wstring>` at another bound is the same code
/// path at another number. The families where that reasoning is
/// weakest — anything where a parameter changes which branch runs — are
/// why the full tier exists and why it runs nightly rather than never.
fn representative(cases: &[Case]) -> Vec<String> {
    let mut families: BTreeSet<&str> = BTreeSet::new();
    let mut chosen: BTreeSet<String> = EVER_DISAGREED.iter().map(|s| (*s).to_string()).collect();
    for case in cases {
        let family = case.name.split('-').next().unwrap_or(&case.name);
        if families.insert(family) {
            chosen.insert(case.name.clone());
        }
    }
    chosen.retain(|name| cases.iter().any(|case| &case.name == name));
    chosen.into_iter().collect()
}

/// Cases where two columns have been measured disagreeing.
///
/// Kept by name so that a fix which quietly stops covering one of them
/// is visible. Adding to this list is cheap; removing from it is a
/// claim, and the claim is checked every night by the full tier.
const EVER_DISAGREED: &[&str] = &[
    // The C column produced a value the other two refused, under an
    // `@bit_bound(8)` enumeration with an explicit `@value`.
    "enum-bitbound-8-value",
    "enum-bitbound-8-value-unannotated",
    // The Python column's `fixed` came back with the BCD field's
    // leading zeros, which its own `from_json` would never produce.
    "fixed-basic",
    "fixed-scales",
    "array-1d-fixed",
    // The C generator's `fill.rs` and `dump.rs` took a `bitset`'s own
    // bitfields where its codec takes the inherited ones too, so the C
    // column had no probe at all for a case the other two answered.
    "bitset-inheritance",
    "bitset-inheritance-unannotated",
    // The three the register carried, kept after it emptied: the C
    // column's missing `zd_def_*` for a bound overrun, the C column's
    // `long double` that did not compile, and the Python column's RTI
    // KeyHash under XCDR1. Each is one case per defect, so the fast tier
    // measures every one of the three fixes rather than waiting for the
    // nightly run to find out one came back.
    "wstring-bounded-utf8",
    "string-bounded-utf8",
    // The C column had no C for `@optional @external Node next`: a C
    // structure holds its members as storage, so the generated header
    // declared one that contains itself and answered
    // `field 'next' has incomplete type`. The member is a pointer now,
    // and the fast tier measures that it stays one.
    "recursive-optional-external",
    "sequence-bounded-full",
    "combined-everything",
    "evolution-mutable-key",
    "primitive-longdouble",
    "primitive-longdouble-unannotated",
    // Not a register entry — the three columns agree on refusing these
    // two — but the refusal is the one that took the most work to make
    // agree, so the fast tier keeps measuring it.
    "primitive-char-above-latin1",
    "primitive-wchar-above-bmp",
    // The one unsorted `map` sample the fast tier's C column would
    // otherwise never see. `representative` picks one case per family
    // and the `map` family's first is `map-bounded`, whose two entries
    // are already in ascending key order and which therefore writes the
    // same octets under both values of `MapEntryOrder`. **Measured, not
    // reasoned**: with `encode_c.rs`'s `Map` arm asking the crossing and
    // then walking the entries in declaration order anyway, the fast
    // tier stayed green over all 18445 C slots. The full tier catches
    // it, and so does this entry — the difference between the two is a
    // night.
    "map-key-long",
];

/// The (configuration, case) pairs whose read direction **cannot**
/// return the sample, and why.
///
/// A `/read` slot holds each column's own verdict against the case's
/// sample, and [`agree`] treats any two `different` verdicts as a
/// disagreement: a round trip that stops round-tripping is a defect
/// whatever the three columns say about it, and two decoders wrong in
/// the same way must not agree their way past the sample.
///
/// One setting breaks that assumption on purpose.
/// `negative-label-unmatched` reproduces a peer that writes **no**
/// branch where a union has no `default:`, and the union's storage read
/// through the `default:` branch where it has one. Neither frame carries
/// the member the sample named, so no reader of any implementation can
/// hand it back. The verdict is `different` in all three columns, and it
/// is the setting working rather than a defect.
///
/// Named here rather than allowed by class, and checked in both
/// directions: a `different` verdict on any slot this list does not
/// name still fails the gate, and an entry that matches nothing fails it
/// too. Where the list does name a slot, the three columns still have to
/// be different in **the same way** — the verdict carries the value that
/// came back and the sample it was held against, and those strings are
/// compared.
struct ReadsBackDifferently {
    /// The dialect whose own behaviour this is. Every configuration
    /// built on it is covered.
    dialect: Dialect,
    /// The cases it covers, in full.
    cases: &'static [&'static str],
    /// Why no reader can return the sample.
    why: &'static str,
}

/// The cases no reader can return the sample for under
/// `UnionLabelMatch::NegativeLabelUnmatchedBelow32`.
const NEGATIVE_LABEL_CASES: &[&str] = &[
    "union-discriminator-int8",
    "union-discriminator-int8-unannotated",
    "union-discriminator-short",
    "union-discriminator-short-unannotated",
    "combined-everything",
    "combined-everything-unannotated",
];

const READS_BACK_DIFFERENTLY: &[ReadsBackDifferently] = &[ReadsBackDifferently {
    // The **dialect** and not a configuration name: the setting is
    // Cyclone's own behaviour and is in `Dialect::Cyclone`, so every
    // configuration built on that dialect inherits it. Naming one
    // configuration here is what left 120 slots outside the register
    // when the setting moved into the dialect: the other two columns
    // had gained the behaviour and nothing said so.
    //
    // Exactly one configuration sits on this dialect now — `cyclone`
    // itself. `universal-nextint` and `rule-15-omits` sat on it while
    // `Dialect::Cyclone` was `Dialect::default`; they sit on
    // `Dialect::Specification` since the default moved, so they read
    // the negative label back the way the clause does and are not in
    // this register. Written as a dialect all the same: the lookup is
    // what keeps this true without a second edit if a configuration is
    // ever built on Cyclone again.
    dialect: Dialect::Cyclone,
    cases: NEGATIVE_LABEL_CASES,
    why: "the setting suppresses the negative label, so the frame carries no branch at all \
          (`union-discriminator-*`) or the `default:` branch over the union's storage \
          (`combined-everything`). The member the sample named is not on the wire, so a reader \
          that returned it would be inventing it.",
}];

impl ReadsBackDifferently {
    /// Whether this entry covers one slot of one case.
    ///
    /// The slot names a configuration and the entry names a dialect, so
    /// the configuration is looked up rather than string-matched: two
    /// configurations over one dialect answer the read direction the
    /// same way, and a prefix test would have covered one of them.
    fn covers(&self, case: &str, slot: &str) -> bool {
        if !slot.ends_with("/read") || !self.cases.contains(&case) {
            return false;
        }
        let Some(name) = slot.split('/').next() else {
            return false;
        };
        CONFIGURATIONS.iter().any(|configuration| {
            configuration.name == name && configuration.dialect == self.dialect
        })
    }
}

/// A divergence that is known, understood, and not fixed in this change.
///
/// A gate with no register would have to be either green and lying or
/// red and ignored. This is the third option: every entry names a
/// column, a root cause and where the fix belongs, and the register is
/// checked in **both** directions —
///
/// - a divergence that matches no entry fails the gate, which is what
///   makes it a gate;
/// - an entry that matches nothing also fails it, so a fix cannot leave
///   a stale exemption behind that would swallow the next regression in
///   the same place.
///
/// Every entry here was found by this gate. None of them was reported
/// before it existed.
struct Known {
    /// The case names this covers, by prefix. Empty matches any.
    cases: &'static [&'static str],
    /// The slot names this covers, by substring. Empty matches any.
    slots: &'static str,
    /// A phrase the refusing column's own diagnostic carries. Empty
    /// matches any.
    ///
    /// The column's **own** diagnostic and never a tool it happens to
    /// invoke. An entry keyed on `zdw_f128` matched clang's `passing
    /// 'const double' to parameter of incompatible type 'zdw_f128'` and
    /// did not match gcc's `incompatible type for argument 2 of
    /// 'zdw_write_f128'` for the same defect, so the same tree was
    /// registered on macOS and 980 unregistered disagreements on Linux.
    /// A C compiler's wording is not a phrase this repository owns.
    ///
    /// This is the field that keeps an entry honest. A register keyed
    /// only on case names says "these cases are allowed to differ", and
    /// the next case to hit the same defect is a new failure while the
    /// next *different* defect in a listed case passes. Keyed on the
    /// diagnostic it says "this defect is allowed to differ", which is
    /// what the entry actually means — and it is what the first entry
    /// needed: the missing `zd_def_*` reaches `typedef-of-string` and
    /// `sequence-primitives-bounded` as much as it reaches
    /// `wstring-bounded-utf8`, and a name list had to be guessed at
    /// while the diagnostic did not.
    message: &'static str,
    /// A one-line root cause and where the fix belongs.
    why: &'static str,
}

/// The gate is what keeps it honest either way: a divergence with no
/// entry fails, and an entry with no divergence fails too.
const KNOWN: &[Known] = &[
    // The two entries that used to sit here — `recursive-optional` and
    // `recursive-mutual-optional` on `rti/pl-cdr1` and `rti/plain-cdr1` —
    // are retired. The generated columns wrote the short header where RTI
    // and the Rust column write `PID_EXTENDED`, because a per-type plan
    // without the sample cannot know a recursion always passes RTI's step.
    // `crates/lang-rules/src/emit/encode.rs` now hands the sample's step
    // decision down to the encoders of nested types at run time, and C and
    // Python write the same octets the reflective encoder does.
    Known {
        // Keyed on the clause rather than on the case: `empty-struct` is the
        // only corpus case with no members today, and the next one added
        // would hit the same rule and belong under the same entry.
        cases: &[],
        slots: "",
        message: "an aggregate with no members",
        why: "C23 §6.7.2.1p8 leaves a structure with no named member undefined, and the           probe builds with -pedantic-errors. The Rust and Python columns write the           four octets an @appendable empty aggregate frames under XCDR2 — a DHEADER           of zero — and C has nowhere to put the type at all. Not a defect and not           fixable here: it is the language, and C++ carries the same case because           C++ has no such rule. crates/lang-rules/src/adapter/c.rs refuses it by           name; docs/language-limits.md is the standing table.",
    },
    // The entry that used to sit here — keyed on the planner's own phrase,
    // "closes a type cycle with nothing on it that terminates a value",
    // for `recursive-array` — is retired. `wireprobe-zerodds-python-gen`
    // is built on `zerodds_lang_rules::emit::encode::codec_from`, the same
    // shared walk over `zerodds_ast_layout`'s planner C# and Java already
    // used, and not on `crates/lang-python`'s own error handling, so its
    // refusal now carries the planner's exact message — the same one
    // `zerodds_wire_probe::encode::encode` raises — rather than a wording
    // of its own. Rust and Python agree on this case outright, which is
    // what `the register has 1 entry that matched nothing` measured: a
    // fixed divergence has to leave the register, and this is that leaving.
    Known {
        // Keyed on the front end's own phrase. `resolve-module-named-as-type`
        // is `module common { struct common { … }; }` and the file no
        // longer reaches a model in any of the three columns.
        cases: &["resolve-module-named-as-type"],
        slots: "",
        message: "is the name of the scope it is declared in",
        why: "The IDL front end refuses the file: OMG IDL 4.2 \u{a7}7.5.2 does not admit               a name redefined within the immediate scope of the module, structure,               union, map, interface, value type, event type, exception or home that               carries it, and the clause prints module M { typedef short M; } as its               own // Error example. All three columns lose the case from that one               root and classify the loss differently only because each meets the               refusal at a different stage: the Rust column has the sentence, the C               column has no file to compile and the Python column reports the path.               The case is in the corpus on purpose \u{2014} it measures which vendors               refuse the file and which resolve it anyway \u{2014} and               tools/idlc/tests/corpus.rs carries it as an intended diagnostic.",
    },
];

impl Known {
    /// Whether this entry is the explanation for one disagreement.
    fn covers(&self, case: &str, slot: &str, answers: &[Option<&Answer>]) -> bool {
        if !self.cases.is_empty() && !self.cases.iter().any(|prefix| case.starts_with(prefix)) {
            return false;
        }
        if !self.slots.is_empty() && !slot.contains(self.slots) {
            return false;
        }
        if self.message.is_empty() {
            return true;
        }
        answers.iter().flatten().any(|answer| match answer {
            Answer::Refused { message, .. } => message.contains(self.message),
            _ => false,
        })
    }
}

// -- the foreign columns --------------------------------------------------

/// The Python column, through `wireprobe-python-matrix`.
///
/// One process for the whole matrix and not `wireprobe-zerodds-python-walk`
/// once per cell: CPython's own start-up is what a hundred thousand
/// per-cell launches would measure, and `wireprobe-python-matrix`'s own
/// module doc has the design that spends it once per worker instead.
///
/// Its JSON is already in the three words this gate compares — the same
/// shape `wireprobe-c-matrix` answers in — because the matrix
/// canonicalises the read direction itself, against
/// `crates/wire-probe/src/canonical.rs`, before it ever writes a byte of
/// its report. There is no second pass here the way
/// `canonicalise_python_reads` used to be one, over `corpus-python.py`'s
/// bare `json:` documents: a column that answers in the gate's own
/// currency has nothing left for the gate to translate.
fn python_column(
    root: &Path,
    seconds: &mut f64,
) -> Option<BTreeMap<String, BTreeMap<String, String>>> {
    // Checked before any build: the matrix binary's own workers are what
    // run `python3`, so a missing interpreter would otherwise surface as
    // the binary exiting non-zero — a real measurement failure and not a
    // skip — rather than as this same, quiet "unmeasured here" every
    // other column answers a missing toolchain with.
    if Command::new("python3").arg("--version").output().is_err() {
        println!("SKIPPED: python3 could not be started; the Python column is unmeasured here");
        return None;
    }
    let binary = build_directory(root)
        .join("release")
        .join("wireprobe-python-matrix");
    // Rebuilt on every run, the same reason `c_column` rebuilds
    // `wireprobe-c-matrix`: a driver left over from an earlier tree
    // would measure that tree's generator and not this one's, and Cargo
    // is a no-op when nothing changed.
    let built = Command::new("cargo")
        .args([
            "build",
            "-q",
            "--release",
            "--manifest-path",
            &root.join("Cargo.toml").display().to_string(),
            "-p",
            "zerodds-wireprobe-python-gen",
            "--bin",
            "wireprobe-python-matrix",
        ])
        .status();
    match built {
        Ok(status) if status.success() => {}
        other => {
            println!(
                "SKIPPED: wireprobe-python-matrix could not be built ({other:?}); \
                 the Python column is unmeasured here"
            );
            return None;
        }
    }
    if !binary.is_file() {
        println!("SKIPPED: {} is not there after a build", binary.display());
        return None;
    }
    let out = root.join("target").join("three-column-python.json");
    let started = std::time::Instant::now();
    // Its progress goes straight through rather than being captured, the
    // same reason `c_column` inherits it: a gate that showed nothing for
    // three minutes is a gate a human kills.
    let run = Command::new(&binary)
        .arg("--out")
        .arg(&out)
        .stderr(std::process::Stdio::inherit())
        .status();
    let run = match run {
        Ok(run) => run,
        Err(error) => {
            println!("SKIPPED: wireprobe-python-matrix could not be started ({error})");
            return None;
        }
    };
    assert!(
        run.success(),
        "the Python column did not run; its output is above"
    );
    *seconds = started.elapsed().as_secs_f64();
    println!("python column: {seconds:.0}s");
    let text = std::fs::read_to_string(&out).expect("the Python column's JSON");
    Some(serde_json::from_str(&text).expect("the Python column's JSON parses"))
}

/// The C column, through `wireprobe-c-matrix`.
/// Where `cargo build` puts what it builds: `CARGO_TARGET_DIR` when it
/// is set, resolved against the working directory the way Cargo
/// resolves it, and `<root>/target` otherwise.
///
/// The two matrix drivers below are built with `cargo build` and then
/// run from here. Looking only under `<root>/target` reported both
/// columns as not built under a `CARGO_TARGET_DIR`, and the register
/// then failed on entries nothing had been measured against.
fn build_directory(root: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if dir.is_absolute() {
                dir
            } else {
                std::env::current_dir().map_or_else(|_| root.join(&dir), |here| here.join(&dir))
            }
        }
        None => root.join("target"),
    }
}

fn c_column(
    root: &Path,
    tier: Tier,
    wanted: &[String],
    seconds: &mut f64,
) -> Option<BTreeMap<String, BTreeMap<String, String>>> {
    let binary = build_directory(root)
        .join("release")
        .join("wireprobe-c-matrix");
    // Rebuilt on every run and not only when the file is missing.
    // `tools/wireprobe/three-column-gate` builds it first, so the two
    // agreed as long as the gate was run through the script — and
    // `cargo test --workspace` runs this same test without it. A stale
    // driver holds the whole of `crates/lang-rules` as it was when it
    // was last linked, so a C column measured through it reports the C
    // column of some earlier tree: the `long double` fix in
    // `c_value_type` showed as 700 unchanged disagreements here while
    // the same tree was green on a host that had rebuilt. Cargo is a
    // no-op when nothing changed.
    let built = Command::new("cargo")
        .args([
            "build",
            "-q",
            "--release",
            "--manifest-path",
            &root.join("Cargo.toml").display().to_string(),
            "-p",
            "zerodds-wireprobe-c-gen",
            "--bin",
            "wireprobe-c-matrix",
        ])
        .status();
    match built {
        Ok(status) if status.success() => {}
        other => {
            // Loudly, and whether or not a binary happens to be lying
            // there: running the old one would report the old tree.
            println!(
                "SKIPPED: wireprobe-c-matrix could not be built ({other:?}); \
                 the C column is unmeasured here"
            );
            return None;
        }
    }
    if !binary.is_file() {
        println!("SKIPPED: {} is not there after a build", binary.display());
        return None;
    }
    let out = root.join("target").join(match tier {
        Tier::Fast => "three-column-c-fast.json",
        Tier::Full => "three-column-c-full.json",
    });
    let mut command = Command::new(&binary);
    command.arg("--out").arg(&out);
    if tier == Tier::Fast {
        for name in wanted {
            command.arg("--case").arg(name);
        }
    }
    let started = std::time::Instant::now();
    // Its progress goes straight through rather than being captured: the
    // full tier is thousands of C translation units, and a gate that
    // showed nothing for twenty minutes is a gate a human kills.
    let run = match command.stderr(std::process::Stdio::inherit()).status() {
        Ok(run) => run,
        Err(error) => {
            println!("SKIPPED: wireprobe-c-matrix could not be started ({error})");
            return None;
        }
    };
    assert!(
        run.success(),
        "the C column did not run; its output is above"
    );
    *seconds = started.elapsed().as_secs_f64();
    println!("c column: {seconds:.0}s");
    let text = std::fs::read_to_string(&out).expect("the C column's JSON");
    Some(serde_json::from_str(&text).expect("the C column's JSON parses"))
}

// -- the gate -------------------------------------------------------------

/// One disagreement, as it goes into the report.
struct Disagreement {
    case: String,
    slot: String,
    rust: Answer,
    c: Option<Answer>,
    python: Option<Answer>,
}

/// An exclusive claim on the fixed paths under `target/` this gate
/// writes.
///
/// # Why a lock and not a unique path per run
///
/// The gate's intermediate files — `three-column-python.json` and
/// `three-column-c-{fast,full}.json` — are handed to a child process by
/// name and read back by name, and the report is written where a human
/// is told to look for it. Two gates running at once interleave into
/// those files, and the one that reads gets a mixture: some rows from
/// its own run, some from the other's, no error anywhere. That is the
/// defect class this gate exists to catch, occurring in the gate
/// itself, and a gate that can silently report a mixed result is not a
/// gate.
///
/// It was hit twice on 2026-08-10, both times by two agents running the
/// full tier in one checkout; both verdicts were discarded. Waiting for
/// the other run from outside works and is invisible to the next
/// person, which is why the rule lives here instead.
///
/// # What a second run gets
///
/// A named failure, immediately: which process holds the gate and since
/// when. Never a wait that looks like a hang, and never a verdict.
///
/// A lock whose owner is gone is taken over rather than obeyed — a run
/// killed with `SIGKILL` leaves the file behind, and a gate that
/// refused for ever afterwards would be its own trap. The takeover says
/// so on stdout.
struct GateLock {
    path: PathBuf,
}

impl GateLock {
    /// Claim the gate, or say who holds it.
    fn acquire(root: &Path) -> Result<Self, String> {
        let directory = root.join("target");
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        let path = directory.join("three-column-gate.lock");

        for attempt in 0..2 {
            // `create_new` is the atomic half: the file is created only
            // if it does not already exist, in one syscall, so two
            // processes racing here cannot both win.
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    use std::io::Write as _;
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |since| since.as_secs());
                    // Best effort: the contents are a diagnostic for
                    // whoever is refused, and the claim is the file's
                    // existence rather than what is written in it.
                    let _ = writeln!(file, "pid {} since {stamp}", std::process::id());
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let held = std::fs::read_to_string(&path).unwrap_or_default();
                    let owner = held.trim().to_string();
                    if attempt == 0 && !owner_is_alive(&owner) {
                        println!(
                            "three-column-gate: clearing a lock whose owner is gone ({owner})"
                        );
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    return Err(format!(
                        "another three-column gate holds {} ({owner}).\n\
                         The two would interleave into the fixed paths under target/, so this \
                         run stops rather than report a verdict mixed out of both. Wait for \
                         that run to finish, or remove the file if you know it is stale.",
                        path.display()
                    ));
                }
                Err(error) => return Err(format!("{}: {error}", path.display())),
            }
        }
        Err(format!("{}: could not be claimed", path.display()))
    }
}

impl Drop for GateLock {
    fn drop(&mut self) {
        // Runs on the panicking path too, which is the one that matters:
        // a failing gate that left its lock behind would refuse every
        // run after it.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Whether the `pid N since T` line names a process that still exists.
///
/// `kill -0` without taking a `libc` dependency for one call. A line
/// this cannot parse is treated as alive, because that is the safe
/// answer when the question cannot be settled: a wrong "it is gone"
/// steals the lock from a running gate and puts both back into the
/// interleaving this type exists to prevent, where a wrong "it is
/// alive" costs one message.
fn owner_is_alive(owner: &str) -> bool {
    let Some(pid) = owner
        .split_whitespace()
        .nth(1)
        .and_then(|word| word.parse::<u32>().ok())
    else {
        return true;
    };
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Write `bytes` to `path` so that no reader ever sees a partial file.
///
/// Into a sibling temporary first, then one rename. A run killed
/// mid-write leaves the temporary behind and the previous file intact,
/// where a direct write leaves a truncated file that parses as a short
/// answer. The same shape `wireprobe-c-matrix` already uses for its
/// probe cache.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{}: no directory to write into", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    // Beside the destination, so the rename stays inside one filesystem
    // and is therefore atomic; and per process, so two writers cannot
    // collide on the temporary either.
    let temporary = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("{}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path).map_err(|error| format!("{}: {error}", path.display()))
}

/// Whether a written matrix records a run the gate called green.
///
/// The verdict is read out of the file rather than assumed from the
/// process that wrote it, so a writer that drops the disagreement
/// section — or leaves a truncated file — is a failure and not a
/// quieter record. Only two things are read: the tier the run was, and
/// whether `## The disagreements` says `None.`. Everything else in the
/// file is prose for a reader.
///
/// # Errors
///
/// When the file names another tier, when it has no disagreement
/// section, or when that section is not `None.`.
fn matrix_reports_a_green_run(matrix: &str, tier: Tier) -> Result<(), String> {
    let word = match tier {
        Tier::Fast => "fast",
        Tier::Full => "full",
    };
    let prefix = format!("Tier `{word}`.");
    if !matrix.lines().any(|line| line.starts_with(&prefix)) {
        return Err(format!("no line starts with `{prefix}`"));
    }
    let mut lines = matrix.lines();
    while let Some(line) = lines.next() {
        if line == "## The disagreements" {
            let body = lines
                .by_ref()
                .take_while(|line| !line.starts_with("## "))
                .collect::<Vec<_>>()
                .join("\n");
            let body = body.trim();
            return if body == "None." {
                Ok(())
            } else {
                Err(format!("the disagreement section is {body:?}, not `None.`"))
            };
        }
    }
    Err("the matrix has no `## The disagreements` section".to_string())
}

#[test]
#[allow(clippy::too_many_lines)]
fn the_three_columns_agree() {
    let tier = Tier::from_environment();
    let root = repository_root();
    // Exclusive, and loud when it is not. Everything below writes fixed
    // paths under `target/`; two gates sharing them interleave into one
    // file and both verdicts become a mixture neither run would report
    // on its own. See [`GateLock`].
    let _lock = GateLock::acquire(&root).unwrap_or_else(|why| panic!("{why}"));
    let cases = corpus();
    assert!(!cases.is_empty(), "the corpus is empty");

    let started = std::time::Instant::now();
    let rust: BTreeMap<String, BTreeMap<String, String>> = cases
        .iter()
        .map(|case| (case.name.clone(), rust_case(case)))
        .collect();
    let rust_seconds = started.elapsed().as_secs_f64();
    println!("rust column: {rust_seconds:.0}s");

    let wanted = representative(&cases);
    let (mut python_seconds, mut c_seconds) = (0.0, 0.0);
    let python = python_column(&root, &mut python_seconds);
    let c = c_column(&root, tier, &wanted, &mut c_seconds);
    // Per column, because one total hides which of the three cost what
    // — and because the C column's cost depends on whether its probe
    // cache was cold, which a single number would let a reader mistake
    // for a change in the matrix.
    let timing =
        format!("Rust {rust_seconds:.0}s, Python {python_seconds:.0}s, C {c_seconds:.0}s.");

    let mut disagreements: Vec<Disagreement> = Vec::new();
    let mut registered: Vec<Disagreement> = Vec::new();
    let mut matched = [0usize; KNOWN.len()];
    let mut read_differently = vec![0usize; READS_BACK_DIFFERENTLY.len()];
    let mut counted: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut classes: BTreeMap<(&str, Reason), usize> = BTreeMap::new();
    let mut compared = 0usize;
    let mut c_slots = 0usize;
    let mut python_slots = 0usize;

    for case in &cases {
        let ours = &rust[&case.name];
        let theirs_c = c.as_ref().and_then(|rows| rows.get(&case.name));
        let theirs_python = python.as_ref().and_then(|rows| rows.get(&case.name));
        for (slot, cell) in ours {
            let mine = Answer::parse(cell);
            *counted.entry(("rust", mine.kind())).or_default() += 1;
            if let Answer::Refused { reason, .. } = &mine {
                *classes.entry(("rust", *reason)).or_default() += 1;
            }
            let c_answer = theirs_c.and_then(|row| row.get(slot)).map(|cell| {
                let answer = Answer::parse(cell);
                *counted.entry(("c", answer.kind())).or_default() += 1;
                if let Answer::Refused { reason, .. } = &answer {
                    *classes.entry(("c", *reason)).or_default() += 1;
                }
                answer
            });
            let python_answer = theirs_python.and_then(|row| row.get(slot)).map(|cell| {
                let answer = Answer::parse(cell);
                *counted.entry(("python", answer.kind())).or_default() += 1;
                if let Answer::Refused { reason, .. } = &answer {
                    *classes.entry(("python", *reason)).or_default() += 1;
                }
                answer
            });
            c_slots += usize::from(c_answer.is_some());
            python_slots += usize::from(python_answer.is_some());
            compared += 1;

            // The one setting whose read direction cannot return the
            // sample, named by (configuration, case) and never by class.
            let expected_different = READS_BACK_DIFFERENTLY
                .iter()
                .position(|entry| entry.covers(&case.name, slot));
            if let Some(index) = expected_different {
                read_differently[index] += 1;
            }
            let agrees = |other: Option<&Answer>| match other {
                None => true,
                Some(other) => {
                    if expected_different.is_some() {
                        return match (&mine, other) {
                            (Answer::Different(a), Answer::Different(b)) => a == b,
                            (a, b) => agree(slot, a, b),
                        };
                    }
                    agree(slot, &mine, other)
                }
            };
            if !agrees(c_answer.as_ref()) || !agrees(python_answer.as_ref()) {
                let answers = [Some(&mine), c_answer.as_ref(), python_answer.as_ref()];
                let known = KNOWN
                    .iter()
                    .position(|entry| entry.covers(&case.name, slot, &answers));
                if let Some(index) = known {
                    matched[index] += 1;
                }
                let disagreement = Disagreement {
                    case: case.name.clone(),
                    slot: slot.clone(),
                    rust: mine,
                    c: c_answer,
                    python: python_answer,
                };
                if known.is_some() {
                    registered.push(disagreement);
                } else {
                    disagreements.push(disagreement);
                }
            }
        }
    }

    let report = fixture(
        &timing,
        tier,
        &cases,
        &wanted,
        compared,
        c_slots,
        python_slots,
        &counted,
        &classes,
        &disagreements,
        &registered,
        &matched,
        started.elapsed(),
    );
    // The committed fixture is the **full** tier's, because that is the
    // one that is a record of the whole matrix. A fast run writes into
    // `target/` instead: it measures the C column over a fifth of the
    // corpus, and letting it overwrite the record would replace a
    // measurement with a sample of one and leave the tree dirty after
    // every `cargo test --workspace`.
    let path = match tier {
        Tier::Full => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("three_column_matrix.md"),
        Tier::Fast => root.join("target").join("three_column_matrix.fast.md"),
    };
    write_atomically(&path, report.as_bytes()).expect("the matrix fixture");

    // Read back what was just written and fail if the file records a
    // run that disagreed. Until this the fixture was a record and not
    // an input: a red full tier wrote itself into the file and the
    // verdict lived only in this process, so a writer that dropped the
    // disagreement section — or truncated the file — would leave the
    // test green and the artifact wrong, which is the shape
    // `docs/wire-axes-followup.md` calls "a file no check reads". The
    // read is what makes the artifact the CI job's verdict as well as
    // the test's.
    {
        let written = std::fs::read_to_string(&path).expect("the matrix fixture, read back");
        if let Err(why) = matrix_reports_a_green_run(&written, tier) {
            panic!("{} records a failure: {why}", path.display());
        }
    }

    // An entry that matched nothing is a stale exemption, and a stale
    // exemption is a hole the next regression falls into quietly. The
    // check runs in both tiers because [`EVER_DISAGREED`] puts a case
    // from every entry into the fast tier's subset — an exemption the
    // fast tier could not reach would be an exemption nobody checks
    // until the nightly run.
    {
        let stale: Vec<&str> = KNOWN
            .iter()
            .zip(matched)
            .filter(|(_, count)| *count == 0)
            .map(|(entry, _)| entry.why)
            .collect();
        assert!(
            stale.is_empty(),
            "the register has {} entry that matched nothing; a fixed divergence has to \
             leave the register or it will swallow the next one:\n  {}",
            stale.len(),
            stale.join("\n  ")
        );
    }

    {
        let stale: Vec<&str> = READS_BACK_DIFFERENTLY
            .iter()
            .zip(&read_differently)
            .filter(|(_, count)| **count == 0)
            .map(|(entry, _)| entry.why)
            .collect();
        assert!(
            stale.is_empty(),
            "{} entry of READS_BACK_DIFFERENTLY matched no slot; a setting whose read \
             direction started round-tripping again has to leave the list, or the next \
             regression in it passes:\n  {}",
            stale.len(),
            stale.join("\n  ")
        );
    }

    assert!(
        disagreements.is_empty(),
        "{} of {compared} slot(s) disagree outside the register; {} has the whole matrix\n\n{}",
        disagreements.len(),
        path.display(),
        disagreements
            .iter()
            .take(40)
            .map(one_line)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Whether two columns' answers for one slot are the same finding.
///
/// Bytes are compared byte for byte. A read is `same` or it is not. A
/// refusal is compared by class and, when both messages carry one, by
/// path — the sentence is not compared, because three implementations
/// write three sentences and asserting on the English would make the
/// gate fail on a reworded message.
fn agree(slot: &str, mine: &Answer, other: &Answer) -> bool {
    // The KeyHash of a case whose sample is outside its type is refused
    // by all three for two true reasons, and which one a column reaches
    // first is diagnosis order and not a wire finding. Rust and Python
    // answer the *type's* question — this type declares no `@key`, so it
    // has no KeyHash — without looking at the value. The C column has to
    // materialise the sample before it can compile anything, so it
    // reaches the value first: `sample.too_wide: expected a character
    // below U+0100, found U+0161`.
    //
    // Both are properties of the case that no emit form can produce, so
    // the pair is accepted here and nowhere else. The two classes still
    // have to differ from every other class, and on any slot but a
    // KeyHash they still have to match.
    if slot.ends_with("/keyhash") {
        let class = |answer: &Answer| match answer {
            Answer::Refused { reason, .. } => Some(*reason),
            _ => None,
        };
        let pair = (class(mine), class(other));
        if pair == (Some(Reason::NoKey), Some(Reason::ValueOutsideTheType))
            || pair == (Some(Reason::ValueOutsideTheType), Some(Reason::NoKey))
        {
            return true;
        }
    }
    match (mine, other) {
        (Answer::Bytes(a), Answer::Bytes(b)) => a == b,
        (Answer::Same, Answer::Same) => true,
        (
            Answer::Refused {
                reason: a,
                message: left,
            },
            Answer::Refused {
                reason: b,
                message: right,
            },
        ) => {
            if a != b {
                return false;
            }
            match (path_in(left), path_in(right)) {
                (Some(a), Some(b)) => a == b,
                // A column whose message names no path is not evidence
                // that the paths differ. Which columns name one is in
                // the fixture, under "what the refusals carry".
                _ => true,
            }
        }
        _ => false,
    }
}

/// The value path a refusal names, if it names one.
///
/// The canonical form of `crates/wire-probe/src/canonical.rs`: `sample`,
/// then `.member` and `[index]`. All three columns build their paths the
/// same way — the C and Python probes take the Rust probe's spelling
/// because the JSON dialect is a shared contract — so the token is
/// recognisable wherever in a sentence it sits.
fn path_in(message: &str) -> Option<&str> {
    message
        .split(|c: char| c.is_whitespace() || c == '`' || c == ',')
        .find(|word| word.starts_with("sample.") || word.starts_with("sample["))
        .map(|word| word.trim_end_matches(':'))
}

fn one_line(disagreement: &Disagreement) -> String {
    let show = |answer: Option<&Answer>| match answer {
        None => "—".to_string(),
        Some(Answer::Bytes(hex)) => hex.chars().take(48).collect(),
        Some(Answer::Same) => "same".to_string(),
        Some(Answer::Different(detail)) => format!("different: {}", short(detail)),
        Some(Answer::Refused { reason, message }) => {
            format!("refused ({}): {}", reason.as_str(), short(message))
        }
    };
    format!(
        "{} {}\n    rust   {}\n    c      {}\n    python {}",
        disagreement.case,
        disagreement.slot,
        show(Some(&disagreement.rust)),
        show(disagreement.c.as_ref()),
        show(disagreement.python.as_ref())
    )
}

fn short(text: &str) -> String {
    let mut out: String = text.chars().take(96).collect();
    if text.chars().count() > 96 {
        out.push('…');
    }
    out
}

/// The matrix, as `crates/ast-layout/tests/fixtures/vendor_roundtrip.md`
/// shapes one.
#[allow(clippy::too_many_arguments)]
fn fixture(
    timing: &str,
    tier: Tier,
    cases: &[Case],
    wanted: &[String],
    compared: usize,
    c_slots: usize,
    python_slots: usize,
    counted: &BTreeMap<(&str, &str), usize>,
    classes: &BTreeMap<(&str, Reason), usize>,
    disagreements: &[Disagreement],
    registered: &[Disagreement],
    matched: &[usize],
    elapsed: std::time::Duration,
) -> String {
    let _ = elapsed;
    let mut out = String::new();
    out.push_str(
        "# Rust, C and Python, over the whole corpus, in both directions\n\n\
         Written by `crates/wire-probe/tests/three_column_gate.rs`, which is\n\
         the test that produced it and the test that reads it back: a run\n\
         that disagreed writes the disagreement here and fails on it, so a\n\
         full-tier failure is an assertion and not only a file.\n\n",
    );
    let _ = writeln!(
        out,
        "Tier `{}`. {timing}\n",
        match tier {
            Tier::Fast => "fast",
            Tier::Full => "full",
        }
    );

    out.push_str("## The matrix\n\n");
    let _ = writeln!(
        out,
        "{} cases, {} configurations, {} encodings, {} byte orders. Per case and\n\
         configuration: the payload and the encapsulated payload in each\n\
         encoding and order, the read direction for each of those, and one\n\
         KeyHash per encoding — {} slots in the Rust column.\n",
        cases.len(),
        CONFIGURATIONS.len(),
        ENCODINGS.len(),
        ORDERS.len(),
        compared
    );
    let _ = writeln!(out, "| column | slots measured | of the Rust column's |",);
    out.push_str("|---|---:|---:|\n");
    let _ = writeln!(out, "| Rust | {compared} | 100% |");
    let _ = writeln!(
        out,
        "| C | {c_slots} | {:.0}% |",
        percent(c_slots, compared)
    );
    let _ = writeln!(
        out,
        "| Python | {python_slots} | {:.0}% |\n",
        percent(python_slots, compared)
    );

    out.push_str("## What each column answered\n\n");
    out.push_str("| column | bytes | read back the same | read back different | refused |\n");
    out.push_str("|---|---:|---:|---:|---:|\n");
    for column in ["rust", "c", "python"] {
        let get = |kind: &str| counted.get(&(column, kind)).copied().unwrap_or_default();
        let _ = writeln!(
            out,
            "| {column} | {} | {} | {} | {} |",
            get("bytes"),
            get("same"),
            get("different"),
            get("refused")
        );
    }

    out.push_str("\n## What the refusals carry\n\n");
    out.push_str(
        "The class is what the gate compares. The sentence is not: three\n\
         implementations write three sentences for one refusal, and asserting\n\
         on the English would make the gate fail on a rewording. Where two\n\
         columns' messages both name a value path, the paths are compared too.\n\n",
    );
    out.push_str("| class | rust | c | python |\n|---|---:|---:|---:|\n");
    for reason in [
        Reason::NoMapping,
        Reason::ValueOutsideTheType,
        Reason::NoKey,
        Reason::Unwritten,
        Reason::Other,
    ] {
        let get = |column: &str| classes.get(&(column, reason)).copied().unwrap_or_default();
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            reason.as_str(),
            get("rust"),
            get("c"),
            get("python")
        );
    }

    out.push_str("\n## The disagreements\n\n");
    if disagreements.is_empty() {
        out.push_str("None.\n");
    } else {
        let _ = writeln!(out, "{} of {compared} slots.\n", disagreements.len());
        out.push_str("```text\n");
        for disagreement in disagreements.iter().take(200) {
            out.push_str(&one_line(disagreement));
            out.push('\n');
        }
        if disagreements.len() > 200 {
            let _ = writeln!(out, "… and {} more", disagreements.len() - 200);
        }
        out.push_str("```\n");
    }

    out.push_str("\n## Known, understood, and not fixed here\n\n");
    out.push_str(
        "Each entry is a divergence this gate found, with its root cause and\n\
         where the fix belongs. The register is checked in both directions: a\n\
         divergence outside it fails the gate, and — in the full tier — an\n\
         entry that matches nothing fails it too, so a fix cannot leave an\n\
         exemption behind to swallow the next regression in the same place.\n\n",
    );
    out.push_str("| slots | root cause |\n|---:|---|\n");
    for (entry, count) in KNOWN.iter().zip(matched) {
        let _ = writeln!(
            out,
            "| {count} | {} |",
            entry.why.split_whitespace().collect::<Vec<_>>().join(" ")
        );
    }
    if !registered.is_empty() {
        let _ = writeln!(
            out,
            "\n{} slot(s) matched the register. One of each:\n",
            registered.len()
        );
        out.push_str("```text\n");
        let mut shown: BTreeSet<&str> = BTreeSet::new();
        for disagreement in registered {
            if shown.insert(&disagreement.case) {
                out.push_str(&one_line(disagreement));
                out.push('\n');
            }
        }
        out.push_str("```\n");
    }

    out.push_str("\n## What this run did not cover\n\n");
    match tier {
        Tier::Full => out.push_str(
            "Nothing in the matrix above: the full tier runs every column over\n\
             every case.\n\n",
        ),
        Tier::Fast => {
            let _ = writeln!(
                out,
                "The C column ran over {} of {} cases — one per construct family,\n\
                 plus every case any column has been measured disagreeing on. The\n\
                 other {} are covered by the full tier,\n\
                 `ZERODDS_GATE_TIER=full tools/wireprobe/three-column-gate`.\n",
                wanted.len(),
                cases.len(),
                cases.len() - wanted.len()
            );
        }
    }
    out.push_str("Outside the matrix in either tier:\n\n");
    let _ = writeln!(
        out,
        "- **Cross-column reads over one model, because they are already\n\
         \x20 here.** Each column's read slot is that column's reader over the\n\
         \x20 bytes of the payload slot beside it, and every payload slot is\n\
         \x20 compared across the three columns above — {} of them, none\n\
         \x20 differing. So column X's bytes into column Y's reader is Y's own\n\
         \x20 reader over the same input: the same decoder, the same bytes, the\n\
         \x20 same answer. Running it would spend {} more probe launches\n\
         \x20 reproducing a result the byte comparison already carries.\n\
         \n\
         \x20 The cross-column read that is **not** that is the one where the\n\
         \x20 two ends do not share a model, and it is measured:\n\
         \x20 `crates/wire-probe/tests/evolution.rs` runs all fourteen\n\
         \x20 `evolution-*` pairs through all three columns — `old.idl` writes,\n\
         \x20 `type.idl` reads — in four modes each. The reader there is handed\n\
         \x20 bytes its own model would never have produced.\n\
         \n\
         \x20 All three verdicts are now reached in one **currency**. Every\n\
         \x20 column's read slot is the canonical text of\n\
         \x20 `crates/wire-probe/src/canonical.rs`, held against the same\n\
         \x20 canonicalisation of the case's own sample. Python reaches it the\n\
         \x20 way C does: `crates/lang-python` emits a `to_json` beside its\n\
         \x20 `from_json`, `tools/wireprobe/wireprobe-zerodds-python` takes\n\
         \x20 `--in` and `--values-out` like the other two probes, and the\n\
         \x20 document that comes back is canonicalised in Rust. What the old\n\
         \x20 arrangement could not see is what it was replaced for: the\n\
         \x20 Python column compared objects against its own `from_json`, so a\n\
         \x20 value its decoder built which no writer would emit was outside\n\
         \x20 the comparison — the shape of the `fixed` defect, whose reader\n\
         \x20 returned the BCD field's leading zeros while the re-encoded\n\
         \x20 bytes matched nibble for nibble.",
        counted.get(&("rust", "bytes")).copied().unwrap_or_default(),
        // Two more columns over each read slot, and a read slot per case,
        // configuration, encoding and byte order.
        2 * cases.len() * CONFIGURATIONS.len() * ENCODINGS.len() * ORDERS.len()
    );
    out.push_str(concat!(
        "- **The `--pl-cdr1-headers`, `--key-holder` and `--default-extensibility`\n",
        "  overrides on their own.** All three are reachable through the four\n",
        "  dialects, and the C generator takes only the first of them on its\n",
        "  command line, so an isolated column for them would measure two\n",
        "  implementations and not three.\n",
    ));
    out
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let (part, whole) = (part as f64, whole as f64);
    part * 100.0 / whole
}

/// The C column names the same twelve configurations as
/// [`CONFIGURATIONS`].
///
/// Each of the two matrix drivers used to be a program in its own
/// language holding its own copy of the list, and this test is what kept
/// the copies one list: a configuration added here and not to the other
/// would silently go unmeasured in that column while the gate stayed
/// green. Python is no longer one of the copies to check —
/// `wireprobe-python-matrix` imports [`CONFIGURATIONS`] itself, from
/// `zerodds_wire_probe::matrix`, rather than holding a second list that
/// could drift from it, so there is nothing here for a text scan to
/// compare it against. `wireprobe-c-matrix.rs` still holds its own
/// literal — see its own module doc for why it predates the shared one —
/// and this is the half of the check that still means something.
#[test]
fn the_c_column_covers_the_same_configurations() {
    let root = repository_root();
    let ours: Vec<String> = CONFIGURATIONS
        .iter()
        .map(|entry| entry.name.to_string())
        .collect();

    let c = std::fs::read_to_string(
        root.join("tools")
            .join("wireprobe")
            .join("wireprobe-zerodds-c-gen")
            .join("src")
            .join("bin")
            .join("wireprobe-c-matrix.rs"),
    )
    .expect("the C batch driver");

    // The list is read out of its literal and compared whole, rather
    // than each name being searched for. A search passes on a name that
    // appears in a comment, and it says nothing about a configuration
    // the C column has and this one does not — which would be a column
    // measured and never compared.
    assert_eq!(
        c_configurations(&c),
        ours,
        "wireprobe-c-matrix.rs drives a different set of configurations"
    );

    // What the Python column actually reads, checked by the compiler and
    // not by a text scan: `wireprobe-python-matrix` names
    // `zerodds_wire_probe::matrix::CONFIGURATIONS` and nothing else, so
    // this and it are either the same twelve or this fails to build.
    // What still wants checking is whether *this file's own* copy — the
    // one `rust_case` plans every row under — agrees with the one the
    // Python column actually reads; the two are two `const`s and Rust
    // does not check that for you.
    let shared: Vec<String> = SHARED_CONFIGURATIONS
        .iter()
        .map(|entry| entry.name.to_string())
        .collect();
    assert_eq!(
        shared, ours,
        "zerodds_wire_probe::matrix::CONFIGURATIONS — what wireprobe-python-matrix reads — \
         names a different set of configurations from this file's own"
    );
}

/// The configuration names in `wireprobe-c-matrix.rs`'s
/// `CONFIGURATIONS`, in order.
fn c_configurations(text: &str) -> Vec<String> {
    let start = text
        .find("const CONFIGURATIONS: [Configuration;")
        .expect("wireprobe-c-matrix.rs has a CONFIGURATIONS list");
    let block = &text[start..];
    let end = block.find("\n];\n").expect("the list ends");
    block[..end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("name: \""))
        .map(|rest| rest.trim_end_matches("\",").to_string())
        .collect()
}

// -- the Rust column against its own record -------------------------------

/// The Rust column's bytes, recorded, so that moving the machinery that
/// produces them can be shown to have moved nothing else.
///
/// [`the_three_columns_agree`] asks whether the three columns agree with
/// **each other**. That is the question that matters for the format, and
/// it is the wrong question for a refactor: three columns that all
/// changed in the same way still agree. It also needs a compiler and an
/// interpreter, and on a host with neither it compares one column with
/// itself.
///
/// This asks the other question, and only of this column: **are the
/// bytes the same ones as before?** Every corpus case, in all twelve
/// [`CONFIGURATIONS`], five encodings, both byte orders and every emit
/// form the gate compares, reduced to one digest per case.
///
/// ## Why a digest and not the slots
///
/// The slots are some seventy thousand lines of hex. A fixture that
/// large is one nobody reads and a diff in it is one nobody can act on.
/// The digest is exact — MD5 over the case's whole slot map, so one
/// flipped octet moves it — and the failure names the **case**, which is
/// the unit a person then re-runs `wireprobe-dump` on. The slot count
/// sits beside it for the reason `EVOLUTION_CASES` is asserted rather
/// than counted: a walk that silently produced fewer slots would
/// otherwise hash a smaller set and say nothing.
/// Refused slots also include their diagnostic text, not payload octets.
/// The recursive-array IDL0014 wording migration in 57f4d99f is pinned in
/// `reference_columns`: substituting only its former reason must reconstruct
/// the exact previous digest over all 420 unchanged refusal coordinates.
///
/// ## Two cases may share a digest, and that is the format talking
///
/// Sixteen digests in the record appear twice. Every one is a pair the
/// wire is supposed to make indistinguishable: `sequence-of-typedef`
/// against `typedef-of-sequence` (§7.2.4.4.3 — an alias is its target),
/// `annotation-external-primitive` against `primitive-long` and
/// `typedef-chain-primitive` (`@external` and a chain of typedefs over a
/// `long` are a `long`), `string-bounded-empty` against
/// `string-unbounded-empty` (an empty string is its length word under
/// either bound), `module-reopened` against
/// `module-reopened-unqualified` (scoping does not reach the wire), and
/// so on — each with its `-unannotated` twin colliding the same way,
/// which is itself a consistency signal.
///
/// A collision costs this record nothing: each case is compared against
/// **its own** recorded digest and never against another case's. Two
/// cases that agree today and stop agreeing tomorrow both move, and both
/// are named.
///
/// ## It was shown to bite
///
/// Measured, not assumed: `s.w.write_i16(… as i16)` changed to `… as
/// i16 ^ 1` in `encode.rs` moved **88 of the 347** cases — every case
/// carrying a `short`, as a member, as a union discriminator or nested
/// inside one — and the test named all 88. The mutation was reverted.
///
/// ## Regenerating it
///
/// `ZERODDS_REBASELINE_RUST_COLUMN=1 cargo test -p zerodds-wire-probe
/// --test three_column_gate the_rust_column`.
///
/// Regenerate when a wire answer changed **on purpose**, and never to
/// make this test green. A baseline taken after the change it exists to
/// detect measures nothing, which is why this was recorded before the
/// machinery moved and not after.
fn rust_column_record() -> String {
    let cases = corpus();
    assert!(!cases.is_empty(), "the corpus is empty");

    let mut out = String::from(
        "# The Rust column's bytes over the whole corpus, one digest per case.\n\
         # MD5 over `<slot>\\t<value>\\n` in sorted slot order; see\n\
         # `the_rust_column_is_byte_for_byte_what_it_was` in\n\
         # crates/wire-probe/tests/three_column_gate.rs.\n\
         # case\tdigest\tslots\n",
    );
    let mut total = 0usize;
    for case in &cases {
        let row = rust_case(case);
        let mut text = String::new();
        for (slot, value) in &row {
            let _ = writeln!(text, "{slot}\t{value}");
        }
        let mut hex = String::with_capacity(32);
        for byte in zerodds_wire::md5(text.as_bytes()) {
            let _ = write!(hex, "{byte:02x}");
        }
        total += row.len();
        let _ = writeln!(out, "{}\t{hex}\t{}", case.name, row.len());
    }
    let _ = writeln!(out, "# {} cases, {total} slots", cases.len());
    out
}

#[test]
fn the_rust_column_is_byte_for_byte_what_it_was() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rust_column_baseline.txt");
    let measured = rust_column_record();

    if std::env::var_os("ZERODDS_REBASELINE_RUST_COLUMN").is_some() {
        std::fs::write(&path, &measured)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        println!("wrote {}", path.display());
        if std::env::var_os("ZERODDS_PRINT_RERECORDED").is_some() {
            println!("=== BASELINE ===\n{measured}=== end ===");
        }
        return;
    }

    let recorded = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\nTake the baseline first: \
             ZERODDS_REBASELINE_RUST_COLUMN=1 cargo test -p zerodds-wire-probe \
             --test three_column_gate the_rust_column",
            path.display()
        )
    });
    if measured == recorded {
        return;
    }

    // Name the cases, not the bytes. A diff of two digest tables is
    // unreadable; the list of cases that moved is what a person acts on.
    let rows = |text: &str| -> BTreeMap<String, String> {
        text.lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| line.split_once('\t'))
            .map(|(name, rest)| (name.to_string(), rest.to_string()))
            .collect()
    };
    let (now, before) = (rows(&measured), rows(&recorded));
    let mut moved = Vec::new();
    for (name, cell) in &now {
        match before.get(name) {
            Some(was) if was == cell => {}
            Some(was) => moved.push(format!("  {name}: was {was}, is {cell}")),
            None => moved.push(format!("  {name}: new, {cell}")),
        }
    }
    for name in before.keys() {
        if !now.contains_key(name) {
            moved.push(format!("  {name}: gone"));
        }
    }
    panic!(
        "the Rust column's bytes moved in {} of {} cases:\n{}\n\n\
         If a wire answer changed on purpose, regenerate with \
         ZERODDS_REBASELINE_RUST_COLUMN=1 and say which answer in the commit. \
         If nothing was meant to change, this is the refactor.",
        moved.len(),
        now.len(),
        moved.join("\n")
    );
}

/// The read-back accepts a green record and refuses a red one, at the
/// two places a reader checks.
///
/// The gate's own run cannot leave the file red on purpose without
/// failing, so the reader is tested directly on the shapes it has to
/// tell apart.
#[test]
fn a_red_record_is_not_read_as_green() {
    let green = "# title\n\nTier `full`. Rust 1s, C 2s.\n\n\
                 ## The disagreements\n\nNone.\n\n\
                 ## Known, understood, and not fixed here\n\n| slots | root cause |\n";
    assert_eq!(matrix_reports_a_green_run(green, Tier::Full), Ok(()));

    let red = "# title\n\nTier `full`. Rust 1s, C 2s.\n\n\
               ## The disagreements\n\n3 of 4 slots.\n\n```text\nx\n```\n";
    assert!(
        matrix_reports_a_green_run(red, Tier::Full).is_err(),
        "a red disagreement section was read as green"
    );

    let fast = "# title\n\nTier `fast`. Rust 1s, C 2s.\n\n\
                ## The disagreements\n\nNone.\n";
    assert_eq!(matrix_reports_a_green_run(fast, Tier::Fast), Ok(()));
    assert!(
        matrix_reports_a_green_run(fast, Tier::Full).is_err(),
        "a fast run was read as a full-tier run"
    );
}
