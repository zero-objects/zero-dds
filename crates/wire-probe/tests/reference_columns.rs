// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Does a new language column answer the matrix the way the Rust column
//! already answered it?
//!
//! Two comparisons, and no third:
//!
//! 1. **write** — `tests/interop-corpus/<case>/values.json` in, octets
//!    out, held against the Rust column's octets for the same slot.
//! 2. **read** — those same octets in, values out, held against the same
//!    `values.json` in the canonical form of
//!    `crates/wire-probe/src/canonical.rs`.
//!
//! ## Against the recorded column, not against the other columns
//!
//! `three_column_gate.rs` asks whether three columns resemble **each
//! other**. Three columns that moved the same way still resemble each
//! other. The Rust column is a fixed point instead: 145,740 slots,
//! pinned case by case in
//! `crates/wire-probe/tests/fixtures/rust_column_baseline.txt`, already
//! held against ten foreign systems in `tests/interop-results/`. A new
//! column is measured against it rather than built beside it, and a cell
//! that differs names the first octet, which is a fact a person acts on.
//!
//! [`the_reference_row_is_the_one_the_baseline_was_taken_with`] closes
//! the loop the other way: the Rust row this file compares against has
//! to reproduce that recorded digest, or the reference has moved and
//! every verdict below is against something else.
//!
//! ## The whole matrix, not one configuration
//!
//! `zerodds_wire_probe::matrix` holds it: twelve configurations — the
//! ten dialects plus the two settings no dialect selects — five
//! encodings, both byte orders, four emit forms. 420 slots per case.
//!
//! A column that answered only the default dialect would be a column
//! that can talk to exactly one peer. The 25 axes of
//! `crates/ast-layout/src/key.rs` are what makes a column able to talk
//! to the field, and
//! [`every_configuration_reaches_the_driver_as_flags`] checks that each
//! one survives the trip through the driver's command line — because
//! that is where it has gone wrong twice already: a switch missing from
//! the Python generator that two repaired callers were passing, and two
//! switches the C driver's shell filtered out. Reasoning about which
//! columns an axis touches is what let both through; running the values
//! through is what catches them.
//!
//! ## What a column has to provide
//!
//! One batch driver, because a per-case process that compiles a
//! translation unit costs more than the measurement.
//! `wireprobe-c-matrix` is the measurement that says so. The contract is
//! one JSON document, `case → slot → cell`, in the alphabet the
//! three-column gate already reads:
//!
//! | cell | meaning |
//! |---|---|
//! | lower-case hex | the octets it produced |
//! | `json:` and a document | what its decoder built, with no verdict of its own |
//! | `!` and a sentence | it would not produce this slot |
//!
//! The read direction hands out a **document** and reaches no verdict:
//! a verdict reached inside the column is a verdict against that
//! column's own idea of what a value is, and a value its decoder can
//! build which no writer of its own would emit falls outside the
//! comparison entirely. That is the shape of the `fixed` defect the
//! three-column gate found. The canonicaliser here decides, against the
//! case's own sample.
//!
//! ## Proving the comparison bites
//!
//! A baseline nobody has seen fail is not a baseline. `ZERODDS_MUTATE`
//! is `<offset>:<xor>`; every payload a column produced is corrupted at
//! that offset before it is compared, and the run then has to report a
//! disagreement in every case whose payload reaches it. The Rust
//! column's own baseline was proved this way — `write_i16(x)` as
//! `x ^ 1` moved 88 of 347 cases — and a new column gets the same
//! treatment before anybody quotes its number.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use zerodds_ast_core::ScopedName;
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::encode::model_from_idl_with;
use zerodds_wire_probe::matrix::{self, Case, SLOTS_PER_CASE};

/// One column: its name in `tests/interop-results/`, and the driver that
/// answers for it.
///
/// The driver is named relative to the repository root and takes
/// `--out <file>` and any number of `--case <name>`. A column whose
/// driver is not there is **skipped loudly** — a machine without a C++
/// compiler is not a machine where the C++ column broke, but a gate that
/// skips in silence reads as green.
struct Column {
    id: &'static str,
    driver: &'static str,
}

const COLUMNS: [Column; 2] = [
    Column {
        id: "zerodds-cpp",
        driver: "target/release/wireprobe-cpp-matrix",
    },
    Column {
        id: "zerodds-java",
        driver: "target/release/wireprobe-java-matrix",
    },
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the repository root above crates/wire-probe")
        .to_path_buf()
}

fn corpus(root: &Path) -> Vec<Case> {
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

/// One column's answers: `case → slot → cell`, as its driver wrote them.
type Answers = BTreeMap<String, BTreeMap<String, String>>;

/// What one column said about one slot.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cell {
    /// Octets, as lower-case hex.
    Bytes(String),
    /// The read direction's document, unparsed.
    Document(String),
    /// The column would not produce this slot, and why.
    Refused(String),
    /// The driver answered nothing for this slot at all.
    Absent,
}

impl Cell {
    fn parse(cell: Option<&String>) -> Self {
        match cell {
            None => Self::Absent,
            Some(text) => {
                if let Some(message) = text.strip_prefix('!') {
                    Self::Refused(message.to_string())
                } else if let Some(document) = text.strip_prefix("json:") {
                    Self::Document(document.to_string())
                } else {
                    Self::Bytes(text.clone())
                }
            }
        }
    }
}

/// Why a refusal was refused, as a class rather than as a sentence.
///
/// Two implementations in two languages write two sentences for one
/// refusal, so the class is what is compared and the sentence is what is
/// reported. The classes are the three-column gate's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    NoMapping,
    ValueOutsideTheType,
    NoKey,
    Unwritten,
    Other,
}

fn classify(message: &str) -> Reason {
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
        // The Java and C# columns' sentence for a code point a UTF-16
        // `char` cannot hold, as recorded in `tests/interop-results/`.
        || lower.contains("past the bmp")
        || lower.contains("does not fit")
        || lower.contains("is longer than")
        || lower.contains("out of range")
        || lower.contains("outofrange")
    {
        return Reason::ValueOutsideTheType;
    }
    Reason::Other
}

/// One disagreement, as it goes into the report.
struct Disagreement {
    case: String,
    slot: String,
    what: String,
}

/// Where two hex strings first differ, as a sentence.
///
/// The offset and the two octets, not the two whole strings: a person
/// acting on this needs the position, and a diff of four kilobytes of
/// hex hides it.
fn first_difference(expected: &str, found: &str) -> String {
    let split = |text: &str| -> Vec<String> {
        text.as_bytes()
            .chunks(2)
            .map(|pair| String::from_utf8_lossy(pair).into_owned())
            .collect()
    };
    let (left, right) = (split(expected), split(found));
    for (offset, (a, b)) in left.iter().zip(&right).enumerate() {
        if a != b {
            return format!(
                "octet {offset}: rust {a}, this column {b} (rust {} octets, this column {})",
                left.len(),
                right.len()
            );
        }
    }
    format!(
        "one is a prefix of the other: rust {} octets, this column {}",
        left.len(),
        right.len()
    )
}

/// Run one column's batch driver and read back its answers.
fn drive(root: &Path, column: &Column) -> Option<Answers> {
    let binary = root.join(column.driver);
    if !binary.is_file() {
        println!(
            "SKIPPED: {} is not there; the {} column is unmeasured on this host",
            binary.display(),
            column.id
        );
        return None;
    }
    let out = root
        .join("target")
        .join(format!("reference-{}.json", column.id));
    let started = std::time::Instant::now();
    let run = Command::new(&binary)
        .arg("--out")
        .arg(&out)
        .stderr(std::process::Stdio::inherit())
        .status();
    match run {
        Ok(status) if status.success() => {}
        other => panic!(
            "the {} column's driver did not run ({other:?}); its output is above",
            column.id
        ),
    }
    println!(
        "{} column: {:.0}s",
        column.id,
        started.elapsed().as_secs_f64()
    );
    let text =
        std::fs::read_to_string(&out).unwrap_or_else(|error| panic!("{}: {error}", out.display()));
    Some(serde_json::from_str(&text).expect("the column's JSON parses"))
}

/// The mutation `ZERODDS_MUTATE` asks for: `<offset>:<xor>`.
fn mutation() -> Option<(usize, u8)> {
    let spec = std::env::var("ZERODDS_MUTATE").ok()?;
    let (offset, xor) = spec
        .split_once(':')
        .unwrap_or_else(|| panic!("ZERODDS_MUTATE is `<offset>:<xor>`, not `{spec}`"));
    Some((
        offset.parse().expect("ZERODDS_MUTATE's offset is a number"),
        xor.parse().expect("ZERODDS_MUTATE's xor is a number"),
    ))
}

fn mutate(hex_text: &str, at: usize, xor: u8) -> String {
    let mut out = String::with_capacity(hex_text.len());
    for index in 0..hex_text.len() / 2 {
        let mut byte =
            u8::from_str_radix(&hex_text[index * 2..index * 2 + 2], 16).unwrap_or_default();
        if index == at {
            byte ^= xor;
        }
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// What one column's run came to.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Tally {
    same_bytes: usize,
    same_read: usize,
    /// Read back what the Rust column read back where that is not the
    /// sample: Rust's `different: <found> for <sample>`, matched by this
    /// column's `<found>`. Measured first on the `cyclone` configuration
    /// of the `int8` and `short` union discriminator cases, where the
    /// C++ column read back exactly Rust's document.
    same_different_read: usize,
    /// Refused in a case [`NO_LANGUAGE_FORM`] names for this column.
    no_language_form: usize,
    same_refusal: usize,
    disagreed: usize,
    compared: usize,
}

/// Cases a column has no form for in its own language, and the rule of
/// that language that says so.
///
/// Stated once, so the reason is stated once; the column has to refuse
/// every slot of a listed case, so an entry a column outgrows fails here
/// until it is removed. A list and never a pattern.
const NO_LANGUAGE_FORM: &[(&str, &str, &str)] = &[(
    "zerodds-java",
    "resolve-module-named-as-type",
    "JLS 8.1: a nested class may not share the simple name of an enclosing \
     class, and the Java mapping holds a module's types in a class of the \
     module's name; measured on the Linux test host as 120 slots the Rust column wrote \
     under the four dialects that admit the case",
)];

/// One column against the Rust rows, slot by slot.
///
/// A function and not a loop inside the test, so that
/// [`the_comparison_bites`] can put the Rust column's own answers
/// through it. A comparison that has only ever been run against an
/// absent driver has never been run.
#[allow(clippy::too_many_lines)]
fn measure(
    column: &str,
    cases: &[Case],
    reference: &Answers,
    rows: &Answers,
    mutation: Option<(usize, u8)>,
) -> (Tally, Vec<Disagreement>) {
    let mut tally = Tally::default();
    let mut failures = Vec::new();

    for case in cases {
        let ours = &reference[&case.name];
        let theirs_row = rows.get(&case.name);
        // A case this column has no language form for answers every slot
        // with a refusal, and is counted as that and nothing else; a
        // slot it answers otherwise means the entry is out of date.
        if let Some((_, _, rule)) = NO_LANGUAGE_FORM
            .iter()
            .find(|(id, name, _)| *id == column && *name == case.name)
        {
            for slot in ours.keys() {
                tally.compared += 1;
                match Cell::parse(theirs_row.and_then(|row| row.get(slot))) {
                    Cell::Refused(_) => tally.no_language_form += 1,
                    other => {
                        tally.disagreed += 1;
                        failures.push(Disagreement {
                            case: case.name.clone(),
                            slot: slot.clone(),
                            what: format!(
                                "listed in NO_LANGUAGE_FORM ({rule}) and answered {other:?}"
                            ),
                        });
                    }
                }
            }
            continue;
        }
        let prepared =
            // The sample's shape, for the read slots. Loaded admitting a
            // declaration named like its scope (IDL 4.2 §7.5.2): four
            // dialects take `resolve-module-named-as-type` and the Rust
            // column reads it back in them, and refusing it here judged 40
            // of the C++ column's read slots against no sample at all. No
            // other front-end answer changes a sample's shape.
            model_from_idl_with(
                &case.idl,
                &format!("{}/type.idl", case.name),
                zerodds_idl_lower::Options {
                    name_of_the_enclosing_scope: zerodds_idl_lower::NameOfTheEnclosingScope::Admitted,
                    ..zerodds_idl_lower::Options::default()
                },
            )
            .and_then(|ast| {
                let type_ = ast
                    .find_by_scoped_name(&ScopedName::absolute(
                        case.type_name
                            .split("::")
                            .filter(|segment| !segment.is_empty())
                            .map(std::string::ToString::to_string)
                            .collect(),
                    ))
                    .ok()
                    .and_then(zerodds_ast::AnyId::as_type)
                    .expect("the case's type is in its own model");
                let expected = canonicalise(&ast, type_, &case.values, "sample")?;
                Ok((ast, type_, text(&expected)))
            });

        let mut fail = |slot: &String, what: String| {
            failures.push(Disagreement {
                case: case.name.clone(),
                slot: slot.clone(),
                what,
            });
        };

        for (slot, mine) in ours {
            tally.compared += 1;
            let theirs = Cell::parse(theirs_row.and_then(|row| row.get(slot)));

            // The Rust column refused here, so every column has to. The
            // class is compared and the sentence is reported: two
            // implementations in two languages write two sentences for
            // one refusal.
            if let Some(message) = mine.strip_prefix('!') {
                match &theirs {
                    Cell::Refused(other) if classify(message) == classify(other) => {
                        tally.same_refusal += 1;
                    }
                    other => {
                        tally.disagreed += 1;
                        fail(
                            slot,
                            format!("rust refused ({message}); this column answered {other:?}"),
                        );
                    }
                }
                continue;
            }

            if slot.ends_with("/read") {
                // Rust read its own octets back as the sample. So must
                // this column — judged here, and never there.
                let Cell::Document(document) = &theirs else {
                    tally.disagreed += 1;
                    fail(
                        slot,
                        format!(
                            "rust read these octets back as `{mine}` and this column \
                             answered {theirs:?}"
                        ),
                    );
                    continue;
                };
                let Ok((ast, type_, expected)) = &prepared else {
                    tally.disagreed += 1;
                    fail(
                        slot,
                        String::from("the case's own sample does not canonicalise"),
                    );
                    continue;
                };
                let found: Value = match serde_json::from_str(document) {
                    Ok(value) => value,
                    Err(error) => {
                        tally.disagreed += 1;
                        fail(slot, format!("the document does not parse: {error}"));
                        continue;
                    }
                };
                match canonicalise(ast, *type_, &found, "sample") {
                    Ok(value) if &text(&value) == expected && mine == "same" => {
                        tally.same_read += 1;
                    }
                    // Rust's own read differs from the sample, and this
                    // column read back the same document Rust did: the two
                    // agree, which is the question this file asks.
                    Ok(value) if *mine == format!("different: {} for {expected}", text(&value)) => {
                        tally.same_different_read += 1;
                    }
                    Ok(value) => {
                        tally.disagreed += 1;
                        fail(
                            slot,
                            format!(
                                "read back {} where rust said `{mine}` for {expected}",
                                text(&value)
                            ),
                        );
                    }
                    Err(error) => {
                        tally.disagreed += 1;
                        fail(
                            slot,
                            format!("what came back is not a value of this type: {error}"),
                        );
                    }
                }
                continue;
            }

            // A payload, an encapsulated payload or a KeyHash: hex,
            // compared octet for octet.
            let Cell::Bytes(produced) = &theirs else {
                tally.disagreed += 1;
                fail(
                    slot,
                    format!("rust produced octets and this column answered {theirs:?}"),
                );
                continue;
            };
            let produced = match mutation {
                Some((at, xor)) => mutate(produced, at, xor),
                None => produced.clone(),
            };
            if &produced == mine {
                tally.same_bytes += 1;
            } else {
                tally.disagreed += 1;
                fail(slot, first_difference(mine, &produced));
            }
        }
    }
    (tally, failures)
}

/// The Rust rows for the whole corpus, once.
fn reference_rows(cases: &[Case]) -> Answers {
    let started = std::time::Instant::now();
    let rows: Answers = cases
        .iter()
        .map(|case| (case.name.clone(), matrix::row(case)))
        .collect();
    println!("rust column: {:.1}s", started.elapsed().as_secs_f64());
    rows
}

#[test]
fn the_new_columns_answer_the_matrix_the_rust_column_answered() {
    let root = repository_root();
    let cases = corpus(&root);
    assert!(!cases.is_empty(), "the corpus is empty");

    let drivers: Vec<(&Column, Answers)> = COLUMNS
        .iter()
        .filter_map(|column| drive(&root, column).map(|rows| (column, rows)))
        .collect();
    if drivers.is_empty() {
        println!(
            "SKIPPED: neither the C++ nor the Java driver is built; nothing was measured. \
             Build them with `cargo build --release -p zerodds-wireprobe-cpp-gen \
             -p zerodds-wireprobe-java-gen`."
        );
        return;
    }

    let mutation = mutation();
    let reference = reference_rows(&cases);
    let mut report = String::from(
        "# The C++ and Java columns against the Rust column\n\n\
         Written by `crates/wire-probe/tests/reference_columns.rs`, which is\n\
         the test that produced it. A record, not an input.\n\n",
    );
    let mut failures: Vec<Disagreement> = Vec::new();
    let mut mutation_caught = 0usize;

    for (column, rows) in &drivers {
        let (tally, mut mine) = measure(column.id, &cases, &reference, rows, mutation);
        if mutation.is_some() {
            mutation_caught += tally.disagreed;
        }
        let _ = writeln!(
            report,
            "## {}\n\n\
             | measurement | slots |\n|---|---|\n\
             | wrote the Rust column's octets | {} |\n\
             | read them back as the sample | {} |\n\
             | read them back as the Rust column did, which is not the sample | {} |\n\
             | refused where the Rust column refused | {} |\n\
             | refused because the language has no form for the case (`NO_LANGUAGE_FORM` in the test) | {} |\n\
             | disagreed | {} |\n\
             | compared | {} |\n",
            column.id,
            tally.same_bytes,
            tally.same_read,
            tally.same_different_read,
            tally.same_refusal,
            tally.no_language_form,
            tally.disagreed,
            tally.compared
        );
        failures.append(&mut mine);
    }

    let path = root
        .join("crates")
        .join("wire-probe")
        .join("tests")
        .join("fixtures")
        .join("reference_columns.md");
    let _ = std::fs::write(&path, report.as_bytes());

    if let Some((at, xor)) = mutation {
        assert!(
            mutation_caught > 0,
            "ZERODDS_MUTATE={at}:{xor} corrupted every payload and the comparison reported \
             nothing; a comparison that does not bite is not a comparison"
        );
        println!("ZERODDS_MUTATE={at}:{xor} moved {mutation_caught} slot(s)");
        return;
    }

    assert!(
        failures.is_empty(),
        "{} slot(s) disagree with the Rust column; {} has the tallies\n\n{}",
        failures.len(),
        path.display(),
        failures
            .iter()
            .take(40)
            .map(|failure| format!("{} {}\n    {}", failure.case, failure.slot, failure.what))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The comparison agrees with a column that is right, and reports one
/// that is wrong.
///
/// The Rust column's own rows are put through [`measure`] as if they
/// were a foreign column's — with the read slots turned back into the
/// documents a driver hands out, because a column answers with a
/// document and no verdict. Two runs:
///
/// - untouched, which has to come back with nothing to report and with
///   every slot accounted for;
/// - with one octet of every payload flipped, which has to report every
///   case whose payload reaches that offset.
///
/// Without this the whole file is a comparison nobody has seen fire.
/// `the_new_columns_answer_the_matrix_the_rust_column_answered` skips
/// while no driver is built, and a gate that has only ever skipped is
/// not a gate. It is the same discipline the Rust baseline was given
/// before anybody believed it: an injected fault moved 88 of 347 cases
/// and the test named all 88.
///
/// A subset of the corpus, because this is a check on the comparison and
/// not on the wire: the first twelve cases in name order carry
/// annotations, arrays and both extensibilities between them.
#[test]
fn the_comparison_bites() {
    let root = repository_root();
    let cases: Vec<Case> = corpus(&root).into_iter().take(12).collect();
    assert_eq!(cases.len(), 12, "the corpus is smaller than the subset");
    let reference = reference_rows(&cases);

    // The Rust rows as a driver would have written them: a read slot
    // carries the document, so the sample itself goes back in. That is
    // what a column that read the octets correctly produces.
    let as_a_column: Answers = cases
        .iter()
        .map(|case| {
            let row = reference[&case.name]
                .iter()
                .map(|(slot, value)| {
                    let cell = if slot.ends_with("/read") && value == "same" {
                        format!("json:{}", case.values)
                    } else {
                        value.clone()
                    };
                    (slot.clone(), cell)
                })
                .collect();
            (case.name.clone(), row)
        })
        .collect();

    let (tally, failures) = measure("zerodds-rust", &cases, &reference, &as_a_column, None);
    assert!(
        failures.is_empty(),
        "the comparison reports {} disagreement(s) against the Rust column's own answers, \
         so it would report them against any column:\n{}",
        failures.len(),
        failures
            .iter()
            .take(10)
            .map(|failure| format!("  {} {} — {}", failure.case, failure.slot, failure.what))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(
        tally.compared,
        cases.len() * SLOTS_PER_CASE,
        "every slot of every case has to be reached; a comparison that quietly skips \
         slots reports agreement for measurements it never made"
    );
    assert_eq!(
        tally.same_bytes
            + tally.same_read
            + tally.same_different_read
            + tally.same_refusal
            + tally.no_language_form,
        tally.compared,
        "every slot has to land in exactly one tally"
    );

    // Now flip one octet of every payload.
    let (mutated, caught) = measure(
        "zerodds-rust",
        &cases,
        &reference,
        &as_a_column,
        Some((0, 1)),
    );
    assert!(
        caught.len() > cases.len(),
        "flipping the low bit of octet 0 of every payload produced only {} \
         disagreement(s) over {} cases; the comparison is not reading the octets",
        caught.len(),
        cases.len()
    );
    assert_eq!(
        mutated.compared, tally.compared,
        "the mutated run compares the same slots as the clean one"
    );
    println!(
        "the comparison bites: octet 0 flipped in {} case(s) moved {} of {} slot(s)",
        cases.len(),
        caught.len(),
        mutated.compared
    );
}

/// The reference this file compares against is the one the recorded
/// baseline was taken with.
///
/// Without this, a change to `zerodds_wire_probe::matrix` would move
/// every verdict above and nothing would say so: the new columns would
/// be measured against a reference nobody pinned. The digest is the one
/// `the_rust_column_is_byte_for_byte_what_it_was` writes, so the two
/// files cannot part without one of them failing.
#[test]
fn the_reference_row_is_the_one_the_baseline_was_taken_with() {
    let root = repository_root();
    let path = root
        .join("crates")
        .join("wire-probe")
        .join("tests")
        .join("fixtures")
        .join("rust_column_baseline.txt");
    let recorded = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let expected: BTreeMap<&str, &str> = recorded
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some((parts.next()?, parts.next()?))
        })
        .collect();

    let mut moved: Vec<String> = Vec::new();
    for case in corpus(&root) {
        let row = matrix::row(&case);
        assert_eq!(
            row.len(),
            SLOTS_PER_CASE,
            "{}: the row holds {} slots and the matrix says {SLOTS_PER_CASE}",
            case.name,
            row.len()
        );
        let mut text = String::new();
        for (slot, value) in &row {
            let _ = writeln!(text, "{slot}\t{value}");
        }
        if case.name == "recursive-array" {
            // 57f4d99f changed IDL0014's explanation, not this rejection.
            // Pin the migration at every slot: restoring only the old reason
            // must reproduce the previously recorded MD5 exactly. No payload,
            // slot coordinate, read verdict or other case may move with it.
            const OLD: &str = "— no sequence, no map and no @optional member — which OMG IDL 4.2 §7.4.1.4.4.4.4 does not admit";
            const NEW: &str = "— no collection, @optional member or productive @external indirection — which OMG IDL 4.2 §7.4.1.4.4.4.4 and XTypes recursion rules do not admit";
            // And the place it is reported at: a range of the parser's
            // tree, read as a range of the file, until a node's range was
            // moved through `Parse::source_span`; the bytes of the
            // declaration in `type.idl` since. The second and last thing
            // that moved.
            const OLD_AT: &str = "error [IDL0014] at 30..65:";
            const NEW_AT: &str = "error [IDL0014] at 458..510:";
            assert_eq!(text.matches(NEW).count(), SLOTS_PER_CASE);
            assert_eq!(text.matches(NEW_AT).count(), SLOTS_PER_CASE);
            assert!(row.values().all(|value| value.starts_with('!')));
            let old_text = text.replace(NEW, OLD).replace(NEW_AT, OLD_AT);
            let mut old_digest = String::with_capacity(32);
            for byte in zerodds_wire::md5(old_text.as_bytes()) {
                let _ = write!(old_digest, "{byte:02x}");
            }
            assert_eq!(old_digest, "c4f4f80091a0ae130217407f48f3c32c");
        }
        let mut digest = String::with_capacity(32);
        for byte in zerodds_wire::md5(text.as_bytes()) {
            let _ = write!(digest, "{byte:02x}");
        }
        match expected.get(case.name.as_str()) {
            Some(was) if *was == digest => {}
            Some(was) => moved.push(format!("  {}: baseline {was}, matrix {digest}", case.name)),
            None => moved.push(format!("  {}: not in the baseline", case.name)),
        }
    }
    assert!(
        moved.is_empty(),
        "`zerodds_wire_probe::matrix` no longer reproduces the recorded Rust column in {} \
         case(s). Every verdict in this file is against that row, so a column measured now \
         would be measured against something nobody pinned:\n{}",
        moved.len(),
        moved.join("\n")
    );
}

/// Every configuration reaches a driver as flags it can act on.
///
/// Run through, not reasoned about. The Python generator was once
/// missing a switch that two repaired callers were already passing, and
/// the C driver's shell filtered two out of what it forwarded; both were
/// found by a column producing plausible octets under the wrong policy.
/// A flag list built from the configuration itself cannot lose one.
#[test]
fn every_configuration_reaches_the_driver_as_flags() {
    for settings in &matrix::CONFIGURATIONS {
        let flags = settings.flags();
        assert_eq!(
            flags.first().map(String::as_str),
            Some("--dialect"),
            "{}: the dialect is a preset and not a patch, so it goes first or the \
             overrides it should be overridden by are discarded",
            settings.name
        );
        assert_eq!(
            flags.len() % 2,
            0,
            "{}: every flag takes a value",
            settings.name
        );
        // Each override the configuration carries has to be in the list.
        // Counted rather than named: an axis added to `Configuration`
        // and forgotten in `flags` changes this number.
        let overrides = usize::from(settings.length_codes.is_some())
            + usize::from(settings.wstring_length.is_some())
            + usize::from(settings.wstring_unit_order.is_some())
            + usize::from(settings.pid_flags.is_some())
            + usize::from(settings.enum_holder.is_some())
            + usize::from(settings.union_label.is_some())
            + usize::from(settings.map_entries.is_some())
            + usize::from(settings.enumerator_value.is_some())
            + usize::from(settings.union_framing.is_some())
            + usize::from(settings.short_pid_range.is_some())
            + usize::from(settings.fixed_digits.is_some())
            + usize::from(settings.map_length.is_some());
        assert_eq!(
            flags.len(),
            2 + overrides * 2,
            "{}: {overrides} override(s) and {} flag word(s); one is not being passed on",
            settings.name,
            flags.len()
        );
    }
}
