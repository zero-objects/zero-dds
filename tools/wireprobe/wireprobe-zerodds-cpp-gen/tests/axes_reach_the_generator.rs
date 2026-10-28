// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! When two configurations put different octets on the wire, does the
//! generator emit different source for them?
//!
//! ## What this is for
//!
//! An option that reaches the wire has to reach the generator, and twice
//! in this work it did not:
//!
//! - a switch was missing from the Python generator that two already
//!   repaired callers were passing to it;
//! - the C driver's shell filtered two flags out of what it forwarded.
//!
//! Both times the codec was right and the **option surface** was the
//! hole, and both times it was found by a column producing plausible
//! octets under a policy nobody had selected. Reasoning about which
//! columns an axis touches is what let them through, so this reasons
//! about nothing: it takes the octets the Rust column actually produces,
//! and requires the generator to have said something different wherever
//! they differ.
//!
//! `crates/wire-probe/tests/reference_columns.rs` does the same one layer
//! out, over a driver's command line. This is the layer below it — the
//! generator's own arguments — and the two together cover the whole trip
//! from a flag to a byte.
//!
//! ## The claim, exactly
//!
//! For one case and one encoding, let *A* and *B* be two of the twelve
//! configurations. If `zerodds_wire_probe::matrix` gives different octets
//! for *A* and *B* in **either** byte order, then the C++ the generator
//! emits under *A* must differ from the C++ it emits under *B*.
//!
//! The converse is not a defect and is not asserted: a policy may change
//! a constant in the source without changing this case's octets. Only the
//! direction that hides a lost option is a failure — identical source,
//! different bytes, which means the generator was never told.
//!
//! Byte order is deliberately outside the comparison.
//! [`zerodds_wireprobe_cpp_gen::combined_sources`] takes it as the
//! compiled program's argument rather than baking it in, so the source is
//! a function of the case, the configuration and the encoding alone; the
//! octets of both orders are folded into one question about that triple.
//! `combined_unit.rs::the_byte_order_is_a_runtime_argument` is what
//! checks that the same claim from the generator's own side.
//!
//! ## Tiers
//!
//! `ZERODDS_AXIS_TIER=full` runs every corpus case. The default runs one
//! case per construct family — the name up to the first `-`, which is how
//! the corpus is organised — because generation is the cost here and the
//! families are what carry the constructs an axis turns on. What the
//! default does not cover is the other cases of each family, which differ
//! from their representative in the parameters of a construct and not in
//! the construct.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::matrix::{self, Case, Configuration};
use zerodds_wire_probe::value::MapLengthPolicy;
use zerodds_wireprobe_cpp_gen::{Args, Emit, combined_sources};

/// The five encodings, under the names the matrix keys its slots by.
const ENCODINGS: [(&str, Encoding); 5] = matrix::ENCODINGS;

fn repository_root() -> PathBuf {
    // `tools/wireprobe/wireprobe-zerodds-cpp-gen` is three below the root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root")
        .to_path_buf()
}

/// One corpus case, with the directory the generator reads its files
/// from.
struct Entry {
    case: Case,
    directory: PathBuf,
}

fn corpus(root: &Path) -> Vec<Entry> {
    let directory = root.join("tests").join("interop-corpus");
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
            Entry {
                case: Case {
                    type_name: read("type").trim().to_string(),
                    idl: read("type.idl"),
                    values: serde_json::from_str(&values_text)
                        .unwrap_or_else(|error| panic!("{name}/values.json: {error}")),
                    name,
                },
                directory,
            }
        })
        .collect()
}

/// One case per construct family, plus every case in [`ALWAYS`].
fn representative(entries: Vec<Entry>) -> Vec<Entry> {
    if std::env::var("ZERODDS_AXIS_TIER").as_deref() == Ok("full") {
        return entries;
    }
    let mut families: BTreeSet<String> = BTreeSet::new();
    entries
        .into_iter()
        .filter(|entry| {
            if ALWAYS.contains(&entry.case.name.as_str()) {
                return true;
            }
            let family = entry
                .case
                .name
                .split('-')
                .next()
                .unwrap_or(&entry.case.name)
                .to_string();
            families.insert(family)
        })
        .collect()
}

/// Cases kept in the default tier whatever family they belong to.
///
/// Each is the one case in the corpus that turns a particular axis on, so
/// dropping it would leave that axis measured by nothing: a `wstring`
/// outside the Basic Multilingual Plane for the length policies and the
/// unit orders, a `fixed` for the digit count, a `map` for the entry
/// order and the delimiter reading, a `@mutable` aggregate for the PID
/// flags and the short-id range, an `@id` at the reserved boundary, and a
/// narrow signed discriminator with a negative label.
///
/// `@key` on a **keyless** aggregate is here for this column's own reason:
/// it is the only shape that puts the holder's subtree under the
/// data-purpose encoders, which the combined unit already holds from the
/// payload set, and so the one place
/// [`zerodds_wireprobe_cpp_gen::combined_sources`] has to drop a duplicate
/// rather than emit two definitions of one name.
///
/// The list is the C column's own, and it is carried over rather than
/// corrected: a name in it that the corpus does not hold matches nothing
/// and is skipped by [`representative`] — `wstring-non-bmp`, `wstring-basic`
/// and `map-basic` are three such names today, checked against
/// `tests/interop-corpus/` on 2026-10-06 — and the constructs they named
/// are still measured, by whichever case of their family comes first in
/// name order. Correcting the list here and not in
/// `tools/wireprobe/wireprobe-zerodds-c-gen/tests/axes_reach_the_generator.rs`
/// would leave two axes tests out of step, which is what this file exists
/// to prevent.
const ALWAYS: &[&str] = &[
    "wstring-non-bmp",
    "wstring-basic",
    "fixed-basic",
    "fixed-scales",
    "map-basic",
    "struct-mutable",
    "union-mutable",
    "enum-bitbound-8-value",
    "annotation-id-sparse",
    "annotation-hashid-implicit",
    "primitive-wchar",
    "annotation-key-whole-struct",
];

/// The generator's arguments for a case, configuration and encoding.
///
/// Built from [`Configuration`] by hand and not from a second table: this
/// is the trip the test exists to check, so it has to be the trip the
/// driver makes. Every field of `Args` that a configuration can carry is
/// set from it, and the ones a configuration never sets are left `None`
/// so the dialect answers.
fn generator_args(entry: &Entry, settings: &Configuration, encoding: Encoding) -> Args {
    Args {
        idl: entry.directory.join("type.idl"),
        type_name: entry.case.type_name.clone(),
        values: Some(entry.directory.join("values.json")),
        read: false,
        encoding,
        // `combined_sources` ignores both: one unit answers for every
        // byte order and every emit form.
        endianness: Endianness::Little,
        emit: Emit::Payload,
        dialect: settings.dialect,
        length_codes: settings.length_codes,
        collection_delimiters: None,
        wstring_length: settings.wstring_length,
        pid_flags: settings.pid_flags,
        enum_holder: settings.enum_holder,
        union_label: settings.union_label,
        map_entries: settings.map_entries,
        enumerator_value: settings.enumerator_value,
        union_framing: settings.union_framing,
        short_pid_range: settings.short_pid_range,
        fixed_digits: settings.fixed_digits,
        wchar_framing: None,
        wchar_width: None,
        map_delimiter: None,
        wstring_unit_order: settings.wstring_unit_order,
        map_length_with_delimiter: settings
            .map_length
            .map(|policy| policy == MapLengthPolicy::SequenceEquivalent),
        ..Args::default()
    }
}

/// What one configuration produced for one case and encoding: the C++, or
/// the reason there is none.
///
/// A refusal is part of the answer and not a skip. Two configurations
/// that refuse for different reasons have said different things, and a
/// configuration that refuses where the other emits has certainly said
/// something different — both are distinctions, and both count.
fn source_of(entry: &Entry, settings: &Configuration, encoding: Encoding) -> String {
    match combined_sources(&generator_args(entry, settings, encoding)) {
        Ok(sources) => format!("{}\n{}", sources.types, sources.probe),
        Err(error) => format!("!{error}"),
    }
}

/// The octets the Rust column produces for one case, keyed by
/// `<config>/<encoding>/<order>`.
fn octets(case: &Case) -> BTreeMap<String, String> {
    matrix::row(case)
}

/// Whether two configurations put different octets on the wire for one
/// case and encoding, in either byte order.
///
/// The payload alone. The encapsulated form differs from it only in the
/// four-byte identifier, which carries the encoding and the byte order
/// and not the policy, and the KeyHash is a digest of a holder whose plan
/// is the same plan — neither adds a distinction the payload does not
/// already carry, and both would make a difference harder to attribute.
fn octets_differ(
    row: &BTreeMap<String, String>,
    left: &str,
    right: &str,
    encoding_name: &str,
) -> bool {
    matrix::ORDERS.iter().any(|(order, _)| {
        let key = |name: &str| format!("{name}/{encoding_name}/{order}");
        row.get(&key(left)) != row.get(&key(right))
    })
}

/// One case, one encoding, one pair of configurations that the generator
/// could not tell apart.
struct Lost {
    case: String,
    encoding: &'static str,
    left: &'static str,
    right: &'static str,
    /// The first slot whose octets differ, so the reader can see what was
    /// at stake.
    slot: String,
}

#[test]
fn a_configuration_that_changes_the_octets_changes_the_generated_cpp() {
    let root = repository_root();
    let entries = representative(corpus(&root));
    assert!(!entries.is_empty(), "the corpus is empty");

    let mut lost: Vec<Lost> = Vec::new();
    let mut distinguished = 0usize;
    let mut indistinct_and_equal = 0usize;
    let mut refused_both = 0usize;
    let mut pairs = 0usize;
    let started = std::time::Instant::now();

    for entry in &entries {
        let row = octets(&entry.case);
        for (encoding_name, encoding) in ENCODINGS {
            // One generation per configuration, reused across the 66
            // pairs: generation is the cost here.
            let sources: Vec<String> = matrix::CONFIGURATIONS
                .iter()
                .map(|settings| source_of(entry, settings, encoding))
                .collect();
            for (index, left) in matrix::CONFIGURATIONS.iter().enumerate() {
                for (offset, right) in matrix::CONFIGURATIONS.iter().enumerate().skip(index + 1) {
                    pairs += 1;
                    let bytes_differ = octets_differ(&row, left.name, right.name, encoding_name);
                    let source_differs = sources[index] != sources[offset];
                    // A refusal is not source. `source_of` renders one as
                    // `!<error>`, and two refusals are equal for the
                    // trivial reason that neither produced any C++ — there
                    // is nothing for a configuration to have reached.
                    //
                    // Counted separately rather than skipped silently:
                    // this arm firing over a case the generator *should*
                    // carry is a finding, and the count is what makes it
                    // visible.
                    if sources[index].starts_with('!') && sources[offset].starts_with('!') {
                        refused_both += 1;
                        continue;
                    }
                    match (bytes_differ, source_differs) {
                        (true, true) => distinguished += 1,
                        (false, _) => indistinct_and_equal += 1,
                        (true, false) => {
                            let slot = matrix::ORDERS
                                .iter()
                                .map(|(order, _)| format!("{encoding_name}/{order}"))
                                .find(|tail| {
                                    row.get(&format!("{}/{tail}", left.name))
                                        != row.get(&format!("{}/{tail}", right.name))
                                })
                                .unwrap_or_default();
                            lost.push(Lost {
                                case: entry.case.name.clone(),
                                encoding: encoding_name,
                                left: left.name,
                                right: right.name,
                                slot,
                            });
                        }
                    }
                }
            }
        }
    }

    println!(
        "{} case(s), {pairs} configuration pair(s) in {:.0}s: {distinguished} where the \
         octets differ and the C++ differs, {indistinct_and_equal} where the octets agree, \
         {refused_both} where the generator refused the case under both",
        entries.len(),
        started.elapsed().as_secs_f64()
    );

    assert!(
        lost.is_empty(),
        "the generator emitted the same C++ for {} configuration pair(s) whose octets \
         differ; an option that reaches the wire and not the generator is how a column \
         comes to produce plausible bytes under a policy nobody selected:\n{}",
        lost.len(),
        lost.iter()
            .take(30)
            .map(|entry| format!(
                "  {} {} — `{}` and `{}` produce the same C++, and {} differs",
                entry.case, entry.encoding, entry.left, entry.right, entry.slot
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The check bites: a generator blind to one axis fails it.
///
/// A test that has only ever passed proves nothing about what it would
/// catch. Here the generator is deliberately handed the **default**
/// configuration in place of each of the twelve — which is what a driver
/// that dropped its flags does — and the run has to report the pairs it
/// can then no longer tell apart.
///
/// It is the same fault as the two real ones: the codec is untouched and
/// the option never arrives.
#[test]
fn the_check_catches_a_generator_that_ignores_its_configuration() {
    let root = repository_root();
    let entries = representative(corpus(&root));
    let deaf = &matrix::CONFIGURATIONS[0];
    let mut lost = 0usize;
    let mut examined = 0usize;

    for entry in &entries {
        let row = octets(&entry.case);
        for (encoding_name, encoding) in ENCODINGS {
            // Every configuration generated as if it were the first: the
            // shape of a driver whose flags were filtered away. The twelve
            // texts are built the same way the real check builds them, so
            // the comparison below is the same comparison and not a
            // shortcut through what it is expected to find.
            let sources: Vec<String> = matrix::CONFIGURATIONS
                .iter()
                .map(|_| source_of(entry, deaf, encoding))
                .collect();
            for (index, left) in matrix::CONFIGURATIONS.iter().enumerate() {
                for (offset, right) in matrix::CONFIGURATIONS.iter().enumerate().skip(index + 1) {
                    examined += 1;
                    if octets_differ(&row, left.name, right.name, encoding_name)
                        && sources[index] == sources[offset]
                    {
                        lost += 1;
                    }
                }
            }
        }
    }

    assert!(
        lost > 0,
        "a generator that ignores its configuration entirely was not caught over {examined} \
         pair(s); either the corpus subset carries no case whose octets depend on the \
         configuration, or the comparison is not reading the octets"
    );
    println!(
        "a generator deaf to its configuration loses {lost} of {examined} pair(s) — \
         that is what this check is for"
    );
}
