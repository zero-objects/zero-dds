// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The 321 cases of `tests/interop-corpus/`, as a test.
//!
//! The corpus was runnable only by hand — `tests/interop-corpus/run.sh`
//! appeared in no pipeline and no test consumed the directory, so the
//! bytes each case's README records were a claim about a run somebody
//! did once. `docs/codec-coverage-report.md` measured what that cost:
//! `fill_cpp.rs` at 0 of 846 regions under every harness in the tree
//! including `cargo test --workspace`, and the whole PL_CDR1 extended
//! header and both big-endian arms of the writer unexecuted because the
//! script defaults to `xcdr2 le`.
//!
//! So this file walks the corpus in process, through the library rather
//! than the binary, and asserts against the READMEs:
//!
//! - the reflective encoder in all four `{xcdr1, xcdr2} × {le, be}`
//!   combinations, byte for byte against each case's `## Probe status`
//!   table;
//! - the C emitter for both C vendors and the C++ emitter for Fast DDS,
//!   whose refusals are a recorded set rather than whatever happens;
//! - the KeyHash and the encapsulated payload.
//!
//! A README row and the code disagreeing is a defect in one of them,
//! and which one is a judgement — but it has to be made, not missed.
//! Recording the bytes and never comparing them again is how a corpus
//! rots at the speed of the encoder.
#![allow(
    clippy::expect_used,
    clippy::panic,
    // `rerecord_the_probe_status_tables` prints how many READMEs it
    // rewrote. A recorder that writes 215 files and says nothing is a
    // recorder nobody can tell from one that wrote none.
    clippy::print_stdout,
    clippy::unwrap_used
)]

use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::encode::{ProbeError, encode, hex, model_from_idl};
use zerodds_wire_probe::fill::{Vendor, fill};
use zerodds_wire_probe::fill_cpp::fill_fastdds;
use zerodds_wire_probe::value::{Emit, ProbeRequest};

/// How many cases the corpus holds.
///
/// Asserted rather than counted so that a walk which silently found
/// nothing — a moved directory, a filter that matched no name — fails
/// instead of passing with an empty loop. Raise it when cases are
/// added; the number is also in `tests/interop-corpus/README.md`.
/// How many cases the corpus holds, counted rather than written down.
///
/// `ros_corpus.rs::cases` carries the argument: a number that
/// duplicates a countable set goes stale at the next addition, and a
/// stale one here is a red gate over the whole suite rather than a
/// finding about the corpus.
fn cases_on_disk(root: &Path) -> usize {
    let count = std::fs::read_dir(root)
        .unwrap_or_else(|error| panic!("the corpus at {}: {error}", root.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("type.idl").is_file())
        .count();
    assert!(count > 0, "the corpus at {} holds no cases", root.display());
    count
}

/// Above this many bytes a README records the length and not the hex.
///
/// The tables stay readable and a 380-byte case still records that it
/// ran. The test enforces the rule in both directions, so a row cannot
/// quietly drop its bytes to stop failing.
const HEX_LIMIT: usize = 96;

/// One of the four combinations: the README flag string it is recorded
/// under, what to ask the probe for, and the encapsulation identifier
/// the pair takes.
struct Mode {
    /// The flag string the README row is keyed by.
    flag: &'static str,
    encoding: Encoding,
    endianness: Endianness,
    /// DDSI-RTPS 2.5 §10.5's identifier, written out rather than taken
    /// from `encapsulation_header`: a test that asks the code what the
    /// code says proves nothing.
    identifier: [u8; 2],
}

/// All four, which is what the corpus runs in.
const MODES: [Mode; 4] = [
    Mode {
        flag: "--encoding xcdr2 --endian le",
        encoding: Encoding::PlainCdr2,
        endianness: Endianness::Little,
        identifier: [0x00, 0x07],
    },
    Mode {
        flag: "--encoding xcdr2 --endian be",
        encoding: Encoding::PlainCdr2,
        endianness: Endianness::Big,
        identifier: [0x00, 0x06],
    },
    Mode {
        flag: "--encoding xcdr1 --endian le",
        encoding: Encoding::PlainCdr1,
        endianness: Endianness::Little,
        identifier: [0x00, 0x01],
    },
    Mode {
        flag: "--encoding xcdr1 --endian be",
        encoding: Encoding::PlainCdr1,
        endianness: Endianness::Big,
        identifier: [0x00, 0x00],
    },
];

/// The cases the two C emitters refuse, and why.
///
/// Identical for Cyclone and RTI because most of the refusals are
/// properties of the IDL-to-C mapping, not of a vendor: §7.5.1 gives C
/// no `map`, no `bitset` and no `fixed`. A case that leaves this list
/// has either gained an emitter or lost one silently, which is why the
/// list is exact and not a count.
///
/// `sequence-wstring` used to be here on the reasoning that §7.5.1.1.1
/// leaves the C spelling of `Char16` to the vendor and a collection has
/// no field to take it from. It has one: the sequence is the field.
/// Cyclone's `_buffer` gives the element type through `__typeof__` and
/// RTI generates `DDS_WstringSeq`, both measured 2026-08-10. AL-12.
///
/// `long double` is the exception that is a vendor property, and the two
/// vendors reach it by different roads: Cyclone's `idlc` refuses to
/// generate the type at all, and RTI's `DDS_LongDouble` is
/// `struct { char bytes[16]; }` on Linux with no representation named
/// for the sixteen octets. Both were measured; `docs/float128-column.md`
/// §2 has the runs.
const C_REFUSALS: &[&str] = &[
    "array-1d-bitset",
    "array-1d-bitset-unannotated",
    "array-1d-fixed",
    "array-1d-fixed-unannotated",
    "array-1d-map",
    "array-1d-map-unannotated",
    "bitset-basic",
    "bitset-basic-unannotated",
    "bitset-inheritance",
    "bitset-inheritance-unannotated",
    "bitset-multiword",
    "bitset-multiword-unannotated",
    "bitset-typed-fields",
    "bitset-typed-fields-unannotated",
    "evolution-mutable-map",
    "fixed-basic",
    "fixed-basic-unannotated",
    "fixed-scale-equals-digits",
    "fixed-scale-equals-digits-unannotated",
    "fixed-scales",
    "fixed-scales-unannotated",
    "longdouble-framed",
    "map-bounded",
    "map-bounded-unannotated",
    "map-empty",
    "map-empty-unannotated",
    "map-key-boolean",
    "map-key-boolean-unannotated",
    "map-key-char",
    "map-key-char-unannotated",
    "map-key-enum",
    "map-key-enum-unannotated",
    "map-key-long",
    "map-key-long-unannotated",
    "map-key-string",
    "map-key-string-unannotated",
    "map-key-ulonglong",
    "map-key-ulonglong-unannotated",
    "map-key-wstring",
    "map-nested",
    "map-nested-unannotated",
    "map-struct-value",
    "map-value-sequence",
    "map-value-sequence-unannotated",
    "map-value-string",
    "map-value-string-unannotated",
    "map-value-struct",
    "map-value-struct-unannotated",
    "primitive-longdouble",
    "primitive-longdouble-unannotated",
    "recursive-through-map",
    "sequence-bitset",
    "sequence-bitset-unannotated",
    "sequence-double-and-longdouble",
    "sequence-double-and-longdouble-unannotated",
    "sequence-fixed",
    "sequence-fixed-unannotated",
    "sequence-map",
    "sequence-map-unannotated",
    "typedef-of-map",
    "typedef-of-map-unannotated",
];

/// The cases the Fast DDS C++ emitter refuses.
///
/// Far fewer than C's, because `fastddsgen` maps `map` to
/// `std::map` and `bitset` to `std::bitset`. What is left is `fixed`,
/// which `fastddsgen` rejects in the IDL before any statement could be
/// written, and `long double`, which it maps to the compiler's own type
/// — x87 80-bit extended on x86-64, binary128 on aarch64 — so a filled
/// literal would record one architecture's bytes as Fast DDS's.
const CPP_REFUSALS: &[&str] = &[
    "array-1d-fixed",
    "array-1d-fixed-unannotated",
    "fixed-basic",
    "fixed-basic-unannotated",
    "fixed-scale-equals-digits",
    "fixed-scale-equals-digits-unannotated",
    "fixed-scales",
    "fixed-scales-unannotated",
    "longdouble-framed",
    "primitive-longdouble",
    "primitive-longdouble-unannotated",
    "sequence-double-and-longdouble",
    "sequence-double-and-longdouble-unannotated",
    "sequence-fixed",
    "sequence-fixed-unannotated",
];

/// The cases whose *value* is outside what the type can hold.
///
/// Every other case in the corpus is a construct: the emitters either
/// map it or say they cannot, and a `Mismatch` from one of them means
/// the four backends read one value file differently. These two are the
/// other thing — a `char` holding U+0161 and a `wchar` holding U+1F600,
/// which DDS-XTypes 1.3 §7.2.2.2.1.2.2 and §7.2.2.2.1.2.5 give no
/// encoding. All four refuse them, by name and with the same message,
/// and that agreement is the assertion. `contract::character_for` is
/// where it comes from; before it existed the two C emitters wrote a
/// literal their compiler narrowed in silence.
const VALUE_REFUSALS: &[&str] = &["primitive-char-above-latin1", "primitive-wchar-above-bmp"];

/// One case, read off disk.
struct Case {
    name: String,
    type_name: String,
    idl: String,
    values: Value,
    /// The `## Probe status` rows, keyed by the flag string.
    recorded: BTreeMap<String, String>,
}

impl Case {
    /// Whether the IDL front end refuses this case's file outright.
    ///
    /// The two cases of
    /// `zerodds_wire_probe::REFUSED_BY_THE_FRONT_END`, both in the
    /// corpus on purpose and both with a README saying every
    /// implementation must refuse the file. There is no model, so
    /// there is nothing for a walk to encode, print or compare.
    ///
    /// The refusal is measured here rather than assumed: a case that
    /// starts lowering again belongs back in every walk, and this
    /// reports it instead of going on skipping it.
    fn refused_by_the_front_end(&self) -> bool {
        if !zerodds_wire_probe::REFUSED_BY_THE_FRONT_END.contains(&self.name.as_str()) {
            return false;
        }
        assert!(
            model_from_idl(&self.idl, &format!("{}/type.idl", self.name)).is_err(),
            "{}: the front end no longer refuses this file, so it belongs back in the walk",
            self.name
        );
        true
    }

    /// The model, parsed and resolved.
    fn model(&self) -> Ast {
        model_from_idl(&self.idl, &format!("{}/type.idl", self.name))
            .unwrap_or_else(|error| panic!("{}: the IDL does not lower: {error}", self.name))
    }

    /// The top-level type's handle.
    fn top_level(&self, ast: &Ast) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
        let segments: Vec<String> = self
            .type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect();
        ast.find_by_scoped_name(&ScopedName::absolute(segments))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .unwrap_or_else(|| panic!("{}: `{}` is not in the model", self.name, self.type_name))
    }
}

/// Where the corpus lives, relative to this crate.
fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus")
}

/// Every case, in name order.
///
/// A directory that is missing one of the four files is a failure and
/// not a skip: a case the walk passes over is a case nobody runs, which
/// is the state this whole file exists to end.
fn cases() -> Vec<Case> {
    let root = corpus_root();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    entries.sort();

    let cases: Vec<Case> = entries
        .iter()
        .map(|dir| {
            let name = dir
                .file_name()
                .expect("a case directory has a name")
                .to_string_lossy()
                .into_owned();
            let read = |file: &str| {
                std::fs::read_to_string(dir.join(file))
                    .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
            };
            let values_text = read("values.json");
            Case {
                type_name: read("type").trim().to_owned(),
                idl: read("type.idl"),
                values: serde_json::from_str(&values_text)
                    .unwrap_or_else(|error| panic!("{name}/values.json: {error}")),
                recorded: probe_status(&read("README.md"), &name),
                name,
            }
        })
        .collect();

    assert_eq!(
        cases.len(),
        cases_on_disk(&root),
        "the corpus walk read a different number of cases than the \
         directory holds"
    );
    cases
}

/// The `## Probe status` rows of one README, keyed by the flag string.
///
/// Only that section. An evolution case — one with an `old.idl` beside
/// its `type.idl` — carries a second table in the same shape, under
/// `## Evolution`, which records what the newer type reads from the
/// older one's bytes. `crates/wire-probe/tests/evolution.rs` is what
/// asserts against that one; reading both here would report each row
/// twice and call the second a duplicate.
fn probe_status(readme: &str, name: &str) -> BTreeMap<String, String> {
    let readme = readme.split("## Evolution").next().unwrap_or(readme);
    let mut rows = BTreeMap::new();
    for line in readme.lines() {
        let Some(rest) = line.strip_prefix("| `--") else {
            continue;
        };
        let Some((flag, cell)) = rest.split_once("` | ") else {
            panic!("{name}/README.md: `{line}` is not a probe-status row");
        };
        let cell = cell
            .strip_suffix(" |")
            .unwrap_or_else(|| panic!("{name}/README.md: `{line}` does not end the row"));
        let previous = rows.insert(format!("--{flag}"), cell.to_owned());
        assert!(
            previous.is_none(),
            "{name}/README.md: two rows for `--{flag}`"
        );
    }
    rows
}

/// What one row of a README says the probe did.
enum Recorded {
    /// `N bytes`, with the hex when the case is short enough to carry it.
    Bytes { length: usize, hex: Option<String> },
    /// `**no** — <message>`, the probe's own `Display` text.
    Refused(String),
}

/// Read one cell.
fn recorded(cell: &str, where_: &str) -> Recorded {
    if let Some(message) = cell.strip_prefix("**no** — ") {
        return Recorded::Refused(message.to_owned());
    }
    let (length, rest) = cell
        .split_once(" bytes")
        .unwrap_or_else(|| panic!("{where_}: `{cell}` is neither a byte count nor a refusal"));
    let length: usize = length
        .parse()
        .unwrap_or_else(|error| panic!("{where_}: `{length}` is not a length: {error}"));
    let hex = match rest {
        "" => None,
        _ => Some(
            rest.strip_prefix(" — `")
                .and_then(|text| text.strip_suffix('`'))
                .unwrap_or_else(|| panic!("{where_}: `{rest}` is not a hex tail"))
                .to_owned(),
        ),
    };
    Recorded::Bytes { length, hex }
}

/// Rewrite every case's `## Probe status` rows from a fresh run.
///
/// Ignored by default and run by name:
///
/// ```sh
/// cargo test -p zerodds-wire-probe --test interop_corpus -- \
///     --ignored rerecord_the_probe_status_tables
/// ```
///
/// It exists because the tables record **what the default dialect
/// writes**, and the default has moved once: from `Dialect::Cyclone` to
/// `Dialect::Specification`. 321 READMEs cannot be re-derived by hand,
/// and a recording produced by a second implementation of the same walk
/// would be a recording nobody could hold against the checker. So the
/// rewrite and the check read the same cases, call the same `encode`
/// and format the cell the same way — [`cell_for`] is the one place a
/// row's text is decided.
///
/// It touches only rows the four modes and the KeyHash own. A row for a
/// flag the walk does not produce is left where it is rather than
/// dropped, because a README may record a run this file does not make.
#[test]
#[ignore = "rewrites the corpus READMEs; run it deliberately"]
fn rerecord_the_probe_status_tables() {
    let mut written = 0_usize;
    let mut unchanged = 0_usize;
    for case in cases() {
        if case.refused_by_the_front_end() {
            continue;
        }
        let ast = case.model();
        let mut cells: BTreeMap<String, String> = BTreeMap::new();
        for mode in &MODES {
            let request = ProbeRequest::new(&case.type_name, mode.encoding, mode.endianness);
            cells.insert(
                mode.flag.to_owned(),
                cell_for(encode(&ast, &request, &case.values)),
            );
        }
        let request = ProbeRequest::new(&case.type_name, Encoding::PlainCdr2, Endianness::Little)
            .emitting(Emit::KeyHash);
        if let Ok(bytes) = encode(&ast, &request, &case.values) {
            cells.insert("--emit keyhash".to_owned(), cell_for(Ok(bytes)));
        }

        let path = corpus_root().join(&case.name).join("README.md");
        let before = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        // Only above `## Evolution`: that table is the evolution gate's
        // recording and is keyed by the same flag strings.
        let (head, tail) = match before.split_once("## Evolution") {
            Some((head, tail)) => (head.to_owned(), Some(tail.to_owned())),
            None => (before.clone(), None),
        };
        let mut out = String::with_capacity(head.len());
        for line in head.lines() {
            match line
                .strip_prefix("| `--")
                .and_then(|rest| rest.split_once("` | "))
                .map(|(flag, _)| format!("--{flag}"))
                .and_then(|flag| cells.get(&flag).map(|cell| (flag, cell)))
            {
                Some((flag, cell)) => {
                    let _ = writeln!(out, "| `{flag}` | {cell} |");
                }
                None => {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        if let Some(tail) = tail {
            out.push_str("## Evolution");
            out.push_str(&tail);
        }
        if out == before {
            unchanged += 1;
            continue;
        }
        std::fs::write(&path, &out).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        if std::env::var_os("ZERODDS_PRINT_RERECORDED").is_some() {
            // For a host whose checkout is not the one that is committed:
            // the log then carries what was written.
            println!("=== README {} ===\n{out}=== end ===", case.name);
        }
        written += 1;
    }
    println!("rewrote {written} README(s); {unchanged} were already what the probe writes");
}

/// One `## Probe status` cell, in the shape [`recorded`] parses.
///
/// The single place the text of a row is decided, so that the rewrite
/// above and the checks below cannot disagree about the format the way
/// two spellings of the same rule always eventually do.
fn cell_for(produced: Result<Vec<u8>, ProbeError>) -> String {
    match produced {
        Ok(bytes) if bytes.len() <= HEX_LIMIT => {
            format!("{} bytes — `{}`", bytes.len(), hex(&bytes))
        }
        Ok(bytes) => format!("{} bytes", bytes.len()),
        Err(error) => format!("**no** — {error}"),
    }
}

/// Every case, in every encoding and byte order, against the bytes its
/// README records.
///
/// The B → C delta of `docs/codec-coverage-report.md`: the three modes
/// beyond `xcdr2 le` are what reach `write_pid_extended`, the extended
/// arm of `finish_pid` and the big-endian arms of `order2` and `order8`.
/// Running one mode leaves the PL_CDR1 header path unexecuted, and an
/// unexecuted header path is one nobody would notice breaking.
#[test]
fn every_case_produces_the_bytes_its_readme_records_in_all_four_modes() {
    let mut trouble = String::new();
    for case in cases() {
        if case.refused_by_the_front_end() {
            continue;
        }
        let ast = case.model();
        for mode in &MODES {
            let where_ = format!("{} {}", case.name, mode.flag);
            let Some(cell) = case.recorded.get(mode.flag) else {
                writeln!(trouble, "{where_}: no row in the README for this mode")
                    .expect("a String never fails to write");
                continue;
            };
            let request = ProbeRequest::new(&case.type_name, mode.encoding, mode.endianness);
            let produced = encode(&ast, &request, &case.values);
            match (recorded(cell, &where_), produced) {
                (Recorded::Bytes { length, hex: text }, Ok(bytes)) => {
                    if bytes.len() != length {
                        writeln!(
                            trouble,
                            "{where_}: the README records {length} bytes, the probe wrote {}",
                            bytes.len()
                        )
                        .expect("a String never fails to write");
                    }
                    match (text, bytes.len() <= HEX_LIMIT) {
                        (Some(text), true) if text != hex(&bytes) => writeln!(
                            trouble,
                            "{where_}: the README records `{text}`, the probe wrote `{}`",
                            hex(&bytes)
                        )
                        .expect("a String never fails to write"),
                        (Some(_), true) | (None, false) => {}
                        (None, true) => writeln!(
                            trouble,
                            "{where_}: {} bytes fit the table and the README records no hex",
                            bytes.len()
                        )
                        .expect("a String never fails to write"),
                        (Some(_), false) => writeln!(
                            trouble,
                            "{where_}: {} bytes is past the {HEX_LIMIT}-byte limit and the \
                             README records hex anyway",
                            bytes.len()
                        )
                        .expect("a String never fails to write"),
                    }
                }
                (Recorded::Refused(message), Err(error)) => {
                    let produced = format!("{error}");
                    if produced != message {
                        writeln!(
                            trouble,
                            "{where_}: the README records the refusal `{message}`, \
                             the probe said `{produced}`"
                        )
                        .expect("a String never fails to write");
                    }
                }
                (Recorded::Bytes { length, .. }, Err(error)) => writeln!(
                    trouble,
                    "{where_}: the README records {length} bytes, the probe refused: {error}"
                )
                .expect("a String never fails to write"),
                (Recorded::Refused(message), Ok(bytes)) => writeln!(
                    trouble,
                    "{where_}: the README records the refusal `{message}`, \
                     the probe wrote {} bytes",
                    bytes.len()
                )
                .expect("a String never fails to write"),
            }
        }
    }
    assert!(trouble.is_empty(), "\n{trouble}");
}

/// Every case through the C emitter for both C vendors and through the
/// C++ emitter for Fast DDS.
///
/// `fill_cpp.rs` measured 0 of 846 regions under every harness in the
/// tree, `cargo test --workspace` included: nothing imported it and the
/// only caller was a binary nothing invoked. It feeds the Fast DDS
/// column of the interop matrix.
///
/// The emitters take no encoding — the statements that fill a generated
/// structure are the same whatever the sample is later serialised as —
/// so they run once per case rather than once per mode.
#[test]
fn every_case_goes_through_all_three_emitters() {
    let mut trouble = String::new();
    let mut refused: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for case in cases() {
        if case.refused_by_the_front_end() {
            continue;
        }
        let ast = case.model();
        let type_ = case.top_level(&ast);
        let mut check = |vendor: &'static str, produced: Result<String, ProbeError>| match produced
        {
            Ok(source) => {
                // A type with no members has no statements to fill, and
                // `empty-struct` is `struct S { }`. Empty output is the
                // answer there, not the absence of one — named rather
                // than tolerated, so a second empty emitter still fails.
                if source.trim().is_empty() && case.name != "empty-struct" {
                    writeln!(
                        trouble,
                        "{} {vendor}: the emitter succeeded and wrote nothing",
                        case.name
                    )
                    .expect("a String never fails to write");
                }
            }
            // Only `Unsupported` is a refusal. A `Mismatch`, a `Missing`
            // or an `Unknown` from an emitter over a sample the encoder
            // accepts means the two disagree about the value dialect,
            // which is the defect `probe_agreement.rs` is about and not
            // a construct the mapping lacks.
            Err(ProbeError::Unsupported { .. }) => {
                refused.entry(vendor).or_default().push(case.name.clone());
            }
            Err(_) if VALUE_REFUSALS.contains(&case.name.as_str()) => {}
            // `Unfilled` is a gap in **this** repository and not in the
            // vendor's mapping: the filler has no way to write the
            // sample, which `docs/specs/zerodds-verdicts-1.0.md` calls
            // `harness-gap` and gives exit 6. Reporting it as "the
            // emitter failed" names the wrong party — FastDDS carries a
            // `map<wstring, ...>` perfectly well and has never been
            // asked.
            //
            // Named and not swallowed: a second construct arriving here
            // has to show up as a new name, the same rule `REFUSED`
            // carries.
            Err(ProbeError::Unfilled { .. })
                if UNFILLED.contains(&(vendor, case.name.as_str())) => {}
            // `recursive-array` is a type of infinite size — the case
            // exists to record that every implementation refuses it, and
            // this repository's encoder does, by name
            // (`LayoutError::RecursionOutsideCollection`). No finite
            // sample completes the type: whatever the JSON nests, the
            // innermost `pair` has no value and is not optional, so a
            // filler that walks the sample fails there in its own words.
            // "A sample the encoder accepts" is not this case — the
            // encoder refuses it too, which the probe-status table
            // records — so the disagreement machinery has nothing to
            // compare. Named, not wildcarded, the same rule `REFUSED`
            // in `decode_round_trip.rs` follows.
            Err(_) if case.name == "recursive-array" => {}
            Err(error) => writeln!(
                trouble,
                "{} {vendor}: the emitter failed on a sample the encoder accepts: {error}",
                case.name
            )
            .expect("a String never fails to write"),
        };
        for vendor in Vendor::ALL {
            check(vendor.as_str(), fill(&ast, type_, &case.values, vendor));
        }
        check("fastdds", fill_fastdds(&ast, type_, &case.values));
    }

    for (vendor, expected) in [
        ("cyclone", C_REFUSALS),
        ("rti", C_REFUSALS),
        ("fastdds", CPP_REFUSALS),
    ] {
        let seen = refused.remove(vendor).unwrap_or_default();
        if seen != expected {
            writeln!(
                trouble,
                "{vendor}: {} cases refused, {} recorded; unexpected {:?}, no longer refusing {:?}",
                seen.len(),
                expected.len(),
                seen.iter()
                    .filter(|name| !expected.contains(&name.as_str()))
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .filter(|name| !seen.iter().any(|seen| seen == *name))
                    .collect::<Vec<_>>()
            )
            .expect("a String never fails to write");
        }
    }
    assert!(trouble.is_empty(), "\n{trouble}");
}

/// The (vendor, case) pairs whose sample this repository's filler cannot
/// write yet.
///
/// A gap in `crates/wire-codec/src/encode.rs`, not in the vendor. The
/// pair is written out so a second one cannot arrive quietly: the
/// verdict for it is `harness-gap`, exit 6, and it says nothing about
/// what FastDDS can carry.
const UNFILLED: [(&str, &str); 1] = [("fastdds", "map-key-wstring")];

/// The KeyHash and the encapsulated payload, for every case.
///
/// `Emit::PayloadWithEncapsulation` measured T=0/F=1323 — the four
/// bytes that precede a real DDS payload were produced by nothing in
/// the tree.
#[test]
fn every_case_emits_a_keyhash_and_an_encapsulated_payload() {
    let mut trouble = String::new();
    for case in cases() {
        if case.refused_by_the_front_end() {
            continue;
        }
        let ast = case.model();
        for mode in &MODES {
            let where_ = format!("{} {}", case.name, mode.flag);
            let plain = ProbeRequest::new(&case.type_name, mode.encoding, mode.endianness);
            let Ok(payload) = encode(&ast, &plain, &case.values) else {
                // The refusals are the other test's business.
                continue;
            };
            let wrapped = encode(
                &ast,
                &plain.clone().emitting(Emit::PayloadWithEncapsulation),
                &case.values,
            )
            .unwrap_or_else(|error| {
                panic!("{where_}: the payload encoded and the header did not: {error}")
            });
            if wrapped.get(..2) != Some(&mode.identifier[..]) {
                writeln!(
                    trouble,
                    "{where_}: the encapsulation identifier is `{}`, §10.5 says `{}`",
                    hex(wrapped.get(..2).unwrap_or_default()),
                    hex(&mode.identifier)
                )
                .expect("a String never fails to write");
            }
            // §7.4.1.1 pads the serialized data to the next four-byte
            // boundary with zeros, and §7.6.3.1.2 puts the count of
            // those bytes in the low two bits of the second options
            // byte, where a receiver "shall" read it to find where the
            // data ended. Cyclone, RTI and Fast DDS all write both —
            // `value::encapsulation_header` carries the capture.
            let pad = (4 - (payload.len() & 3)) & 3;
            #[allow(clippy::cast_possible_truncation)]
            let expected_options = [0u8, pad as u8];
            if wrapped.get(2..4) != Some(&expected_options[..]) {
                writeln!(
                    trouble,
                    "{where_}: the two option bytes are `{}`, not `{}` for a {}-byte payload",
                    hex(wrapped.get(2..4).unwrap_or_default()),
                    hex(&expected_options),
                    payload.len()
                )
                .expect("a String never fails to write");
            }
            let mut expected = payload.clone();
            expected.resize(payload.len() + pad, 0);
            if wrapped.get(4..) != Some(&expected[..]) {
                writeln!(
                    trouble,
                    "{where_}: the encapsulated payload is not the plain one padded to four"
                )
                .expect("a String never fails to write");
            }
            if wrapped.len() % 4 != 0 {
                writeln!(
                    trouble,
                    "{where_}: the encapsulated form is {} bytes, which §7.4.1.1 does not \
                     leave on a four-byte boundary",
                    wrapped.len()
                )
                .expect("a String never fails to write");
            }
            // And the reader honours the count: a decode of the
            // encapsulated form has to give the same value the plain
            // one does, padding and all.
            let request = plain.clone().emitting(Emit::PayloadWithEncapsulation);
            match zerodds_wire_probe::decode::decode(&ast, &request, &wrapped) {
                Ok(read) => {
                    let plain_read = zerodds_wire_probe::decode::decode(&ast, &plain, &payload);
                    if plain_read.as_ref().ok() != Some(&read) {
                        writeln!(
                            trouble,
                            "{where_}: the encapsulated form read back as something else"
                        )
                        .expect("a String never fails to write");
                    }
                }
                Err(error) => {
                    // A payload this probe cannot read back is the
                    // round-trip test's business, not this one's — but
                    // only if the plain form cannot be read either.
                    if zerodds_wire_probe::decode::decode(&ast, &plain, &payload).is_ok() {
                        writeln!(
                            trouble,
                            "{where_}: the plain payload reads and the encapsulated one does \
                             not: {error}"
                        )
                        .expect("a String never fails to write");
                    }
                }
            }
        }

        // The KeyHash is sixteen bytes for a type that has a key: the
        // key members zero-padded, or their MD5 once they exceed
        // sixteen. DDSI-RTPS 2.5 §9.6.4.8.
        //
        // A type with no `@key` has one instance and no KeyHash, and
        // asking for one is `ProbeError::NoKey`. Read literally Step 1
        // would hash every member of a keyless type; no implementation
        // does, and this probe used to answer with sixteen zero bytes or
        // with the MD5 of nothing depending on whether the empty holder
        // planned sized. Both are gone. `crates/wire-probe/src/encode.rs`
        // carries the vendor measurement.
        let request = ProbeRequest::new(&case.type_name, Encoding::PlainCdr2, Endianness::Little)
            .emitting(Emit::KeyHash);
        // Keyed where the README records a hash; a recorded refusal is
        // matched below like a recorded hash is.
        let keyed = case
            .recorded
            .get("--emit keyhash")
            .is_some_and(|cell| !cell.starts_with("**no** — "));
        match encode(&ast, &request, &case.values) {
            Ok(bytes) if bytes.len() == 16 => {
                let where_ = format!("{} --emit keyhash", case.name);
                let Some(cell) = case.recorded.get("--emit keyhash") else {
                    writeln!(
                        trouble,
                        "{where_}: a hash for a case whose README records none"
                    )
                    .expect("a String never fails to write");
                    continue;
                };
                match recorded(cell, &where_) {
                    Recorded::Bytes {
                        hex: Some(text), ..
                    } if text != hex(&bytes) => writeln!(
                        trouble,
                        "{where_}: the README records `{text}`, the probe wrote `{}`",
                        hex(&bytes)
                    )
                    .expect("a String never fails to write"),
                    Recorded::Bytes { .. } => {}
                    Recorded::Refused(message) => writeln!(
                        trouble,
                        "{where_}: the README records the refusal `{message}`, \
                         the probe wrote a hash"
                    )
                    .expect("a String never fails to write"),
                }
            }
            Ok(bytes) => writeln!(
                trouble,
                "{}: the KeyHash is {} bytes, §9.6.4.8 says sixteen",
                case.name,
                bytes.len()
            )
            .expect("a String never fails to write"),
            Err(ProbeError::NoKey { .. }) if !keyed => {}
            // A refusal the README records is the answer this case is
            // known to give — a keyless type that carries the row, or a
            // type the planner refuses by name: `struct Node { Node
            // pair[2]; }`, which IDL 4.2 §7.4.1.4.4.4.4 does not admit.
            // The probe has to refuse with the recorded words, the same
            // way it has to write the recorded bytes.
            Err(error) => {
                let where_ = format!("{} --emit keyhash", case.name);
                match case
                    .recorded
                    .get("--emit keyhash")
                    .map(|cell| recorded(cell, &where_))
                {
                    Some(Recorded::Refused(message)) if message == error.to_string() => {}
                    Some(Recorded::Refused(message)) => writeln!(
                        trouble,
                        "{where_}: the README records the refusal `{message}`, the probe \
                         refused with `{error}`"
                    )
                    .expect("a String never fails to write"),
                    _ => writeln!(trouble, "{}: no KeyHash: {error}", case.name)
                        .expect("a String never fails to write"),
                }
            }
        }
    }
    assert!(trouble.is_empty(), "\n{trouble}");
}
