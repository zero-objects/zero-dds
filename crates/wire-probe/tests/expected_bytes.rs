// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust column against `tests/interop-corpus/expected-bytes.tsv`.
//!
//! The table holds the octets other implementations wrote for every corpus
//! case, sample, encoding and byte order, generated from one commit of
//! `zerodds-matrix` by `tools/expected-bytes/generate.py` (the commit is in
//! its header). A vendor dialect expects that vendor's own octets; the
//! `specification` dialect expects the octets every DDS implementation that
//! wrote the cell agrees on. Nothing in it is this project's own output.
//!
//! This is the producer check of the reflective codec — the path
//! `wireprobe-zerodds` takes and the matrix's `zerodds-rust` column
//! records: `zerodds_idl_compose` for the model, `zerodds_ast_layout` for
//! the plan, `zerodds_wire_codec` for the octets. Per row and dialect it
//! asserts both directions:
//!
//! - write: the sample, encoded under the dialect, is the vendor's octets.
//!   For a vendor whose alignment gaps are undefined (the table's
//!   `gaps-undefined` lines) a difference is accepted where the codec
//!   reports the octet as padding and nowhere else.
//! - read: the vendor's octets, decoded under the dialect, are the sample,
//!   compared in canonical form.
//!
//! One allowance, the one `tools/interop-results/tests/dialect_read_completeness.rs`
//! makes and for the same rows: a read that does not return the sample is
//! no failure when the dialect suppresses a negative narrow case label
//! (Cyclone's) **and** this project's own writer and reader under the same
//! request do not return it either — the vendor's frame carries no branch for
//! the member the sample names. `the_unreadable_cells_are_cyclones_24` keeps
//! it to those rows.
//!
//! A cell this project does not reproduce yet is declared in
//! `tests/fixtures/expected_bytes_gaps.tsv`, with its `OPEN-ITEMS.md` entry.
//! A declared cell that passes fails the test as well, so the list cannot
//! outlive the gap.
//!
//! One test per dialect, so the list of tests says which dialect broke.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Dialect, Encoding, Endianness, Purpose, UnionLabelMatch};
use zerodds_wire_probe::canonical::{canonicalise, difference};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, encode_with_padding, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::{MapLengthPolicy, ProbeRequest};

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus")
}

/// One line of the table.
struct Row {
    case: String,
    sample: String,
    encoding: Encoding,
    encoding_name: String,
    endianness: Endianness,
    endian_name: String,
    dialects: Vec<String>,
    bytes: Vec<u8>,
}

struct Table {
    commit: String,
    gaps_undefined: BTreeSet<String>,
    rows: Vec<Row>,
}

fn unhex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "odd hex length: {text}");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let path = corpus_root().join("expected-bytes.tsv");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut commit = None;
        let mut gaps_undefined = BTreeSet::new();
        let mut rows = Vec::new();
        let mut header_seen = false;
        for line in text.lines() {
            if let Some(comment) = line.strip_prefix("# ") {
                if let Some(rest) = comment.strip_prefix("matrix-commit: ") {
                    commit = rest.split_whitespace().next().map(str::to_string);
                }
                if let Some(rest) = comment.strip_prefix("gaps-undefined: ") {
                    let vendor = rest.split(':').next().expect("a vendor name");
                    gaps_undefined.insert(vendor.to_string());
                }
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            if !header_seen {
                assert_eq!(line, "case\tsample\tencoding\tendian\tdialects\thex");
                header_seen = true;
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(fields.len(), 6, "a row has six fields: {line}");
            let encoding = match fields[2] {
                "xcdr1" => Encoding::PlainCdr1,
                "xcdr2" => Encoding::PlainCdr2,
                other => panic!("unknown encoding `{other}` in {line}"),
            };
            let endianness = match fields[3] {
                "le" => Endianness::Little,
                "be" => Endianness::Big,
                other => panic!("unknown byte order `{other}` in {line}"),
            };
            rows.push(Row {
                case: fields[0].to_string(),
                sample: fields[1].to_string(),
                encoding,
                encoding_name: fields[2].to_string(),
                endianness,
                endian_name: fields[3].to_string(),
                dialects: fields[4].split(',').map(str::to_string).collect(),
                bytes: unhex(fields[5]),
            });
        }
        let commit = commit.expect("the table names the matrix commit it came from");
        assert!(!rows.is_empty(), "{} holds no rows", path.display());
        Table {
            commit,
            gaps_undefined,
            rows,
        }
    })
}

/// A corpus case as one dialect's front end reads it: the model, the
/// top-level type and the sample in canonical form.
struct Model {
    ast: Ast,
    type_name: String,
    type_id: Id<TypeNode>,
    sample: Value,
}

/// The three front-end answers a dialect controls; two dialects with the
/// same three read a case into the same model.
type Lowering = (bool, bool, bool);

fn lowering(dialect: Dialect) -> Lowering {
    (
        dialect.hashed_id_advances_the_counter(),
        dialect.hashid_reads_the_escaped_spelling(),
        dialect.admits_the_name_of_the_enclosing_scope(),
    )
}

fn load(case: &str, sample: &str, dialect: Dialect) -> Result<Model, String> {
    let dir = corpus_root().join(case);
    let own_idl = dir.join(format!("{sample}.idl"));
    let idl = if own_idl.is_file() {
        own_idl
    } else {
        dir.join("type.idl")
    };
    let source =
        std::fs::read_to_string(&idl).map_err(|error| format!("{}: {error}", idl.display()))?;
    let type_name = std::fs::read_to_string(dir.join("type"))
        .map_err(|error| format!("{case}/type: {error}"))?
        .trim()
        .to_string();
    let values_path = dir.join(format!("{sample}.json"));
    let values: Value = serde_json::from_str(
        &std::fs::read_to_string(&values_path)
            .map_err(|error| format!("{}: {error}", values_path.display()))?,
    )
    .map_err(|error| format!("{}: {error}", values_path.display()))?;
    let ast = model_from_idl_in_dialect(&source, &idl.display().to_string(), dialect)
        .map_err(|error| format!("front end: {error}"))?;
    let segments: Vec<String> = type_name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect();
    let type_id = ast
        .find_by_scoped_name(&ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .ok_or_else(|| format!("`{type_name}` is not in the model"))?;
    let sample = canonicalise(&ast, type_id, &values, &type_name)
        .map_err(|error| format!("the sample: {error}"))?;
    Ok(Model {
        ast,
        type_name,
        type_id,
        sample,
    })
}

/// How one row came out under one dialect.
enum Outcome {
    Same,
    /// Differs only at octets the codec reports as padding, for a vendor
    /// whose gaps are undefined.
    GapsOnly,
    /// Written as the vendor writes it; the frame carries no branch for
    /// the member the sample names (see the module note).
    NothingToRead,
    Failed(String),
}

/// The allowance of the module note: both conditions, measured.
fn nothing_to_read(model: &Model, request: &ProbeRequest) -> bool {
    if request.union_label == UnionLabelMatch::SignExtended {
        return false;
    }
    let Ok(mine) = encode(&model.ast, request, &model.sample) else {
        return false;
    };
    let back = decode(&model.ast, request, &mine)
        .ok()
        .and_then(|value| canonicalise(&model.ast, model.type_id, &value, &model.type_name).ok());
    back.is_none_or(|value| difference(&model.sample, &value, "").is_some())
}

fn check(row: &Row, dialect: Dialect, model: &Model, gaps_undefined: bool) -> Outcome {
    let settings = zerodds_ast_layout::dialect_settings_from_overrides(
        dialect,
        row.encoding,
        row.endianness,
        Purpose::Data,
        &[],
    )
    .expect("a dialect without overrides");
    let map_length = if settings.map_length_with_delimiter {
        MapLengthPolicy::SequenceEquivalent
    } else {
        MapLengthPolicy::Rule15Omits
    };
    let request = ProbeRequest::from_key(&model.type_name, settings.key, map_length);
    let mut failures = Vec::new();
    let mut gaps = false;
    let mut unreadable = false;
    match encode_with_padding(&model.ast, &request, &model.sample) {
        Ok((ours, padding)) if ours != row.bytes => {
            let only_gaps = gaps_undefined
                && ours.len() == row.bytes.len()
                && ours
                    .iter()
                    .zip(&row.bytes)
                    .enumerate()
                    .all(|(i, (a, b))| a == b || padding.get(i).copied().unwrap_or(false));
            if only_gaps {
                gaps = true;
            } else {
                failures.push(format!(
                    "write: expected {}\n           found    {}",
                    hex(&row.bytes),
                    hex(&ours)
                ));
            }
        }
        Ok(_) => {}
        Err(error) => failures.push(format!("write refused: {error}")),
    }
    match decode(&model.ast, &request, &row.bytes) {
        Ok(read) => match canonicalise(&model.ast, model.type_id, &read, &model.type_name) {
            Ok(read) => {
                if let Some(diff) = difference(&model.sample, &read, "") {
                    if nothing_to_read(model, &request) {
                        unreadable = true;
                    } else {
                        failures.push(format!("read: {diff}"));
                    }
                }
            }
            Err(error) => failures.push(format!("read, not canonical: {error}")),
        },
        Err(error) => failures.push(format!("read refused: {error}")),
    }
    if !failures.is_empty() {
        Outcome::Failed(failures.join("\n         "))
    } else if unreadable {
        Outcome::NothingToRead
    } else if gaps {
        Outcome::GapsOnly
    } else {
        Outcome::Same
    }
}

type ModelCache = Mutex<HashMap<(String, String, Lowering), std::sync::Arc<Result<Model, String>>>>;

fn model(case: &str, sample: &str, dialect: Dialect) -> std::sync::Arc<Result<Model, String>> {
    static MODELS: OnceLock<ModelCache> = OnceLock::new();
    let cache = MODELS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (case.to_string(), sample.to_string(), lowering(dialect));
    if let Some(found) = cache.lock().expect("model cache").get(&key) {
        return found.clone();
    }
    let loaded = std::sync::Arc::new(load(case, sample, dialect));
    cache
        .lock()
        .expect("model cache")
        .insert(key, loaded.clone());
    loaded
}

/// The declared cells: `dialect<TAB>case/sample<TAB>encoding<TAB>endian`
/// to the `OPEN-ITEMS.md` entry and the reason.
fn declared_gaps() -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/expected_bytes_gaps.tsv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(fields.len(), 6, "{}: six fields: {line}", path.display());
            (
                format!("{} {} {} {}", fields[0], fields[1], fields[2], fields[3]),
                format!("OPEN-ITEMS {}: {}", fields[4], fields[5]),
            )
        })
        .collect()
}

/// What one dialect's rows came to.
struct Tally {
    checked: usize,
    gaps_only: usize,
    nothing_to_read: usize,
    failed: BTreeMap<String, String>,
}

fn run_dialect(name: &str) -> Tally {
    let dialect = Dialect::parse(name).expect("a dialect of the table");
    let table = table();
    let gaps_undefined = table.gaps_undefined.contains(name);
    let mut checked = 0usize;
    let mut gaps_only = 0usize;
    let mut nothing = 0usize;
    let mut failed: BTreeMap<String, String> = BTreeMap::new();
    for row in table
        .rows
        .iter()
        .filter(|row| row.dialects.iter().any(|d| d == name))
    {
        checked += 1;
        let cell = format!(
            "{}/{} {} {}",
            row.case, row.sample, row.encoding_name, row.endian_name
        );
        let loaded = model(&row.case, &row.sample, dialect);
        match loaded.as_ref() {
            Err(error) => {
                failed.insert(cell, error.clone());
            }
            Ok(model) => match check(row, dialect, model, gaps_undefined) {
                Outcome::Same => {}
                Outcome::GapsOnly => gaps_only += 1,
                Outcome::NothingToRead => nothing += 1,
                Outcome::Failed(why) => {
                    failed.insert(cell, why);
                }
            },
        }
    }
    assert!(
        checked > 0,
        "no row of the table names the dialect `{name}`"
    );
    Tally {
        checked,
        gaps_only,
        nothing_to_read: nothing,
        failed,
    }
}

/// Every row that names `name`, checked; panics with every failure that
/// is not declared and every declared cell that did not fail.
fn dialect_matches_the_table(name: &str) {
    let table = table();
    let tally = run_dialect(name);
    let declared: BTreeMap<String, String> = declared_gaps()
        .into_iter()
        .filter_map(|(key, why)| {
            key.strip_prefix(&format!("{name} "))
                .map(|cell| (cell.to_string(), why))
        })
        .collect();
    let undeclared: Vec<(&String, &String)> = tally
        .failed
        .iter()
        .filter(|(cell, _)| !declared.contains_key(*cell))
        .collect();
    let stale: Vec<&String> = declared
        .keys()
        .filter(|cell| !tally.failed.contains_key(*cell))
        .collect();
    if !undeclared.is_empty() || !stale.is_empty() {
        let mut report = format!(
            "dialect `{name}`: {} of {} cells differ from tests/interop-corpus/expected-bytes.tsv \
             (matrix {}; {} declared, {} of them not failing; {} differ in undefined gap octets \
             only, {} carry no branch to read)\n",
            tally.failed.len(),
            tally.checked,
            &table.commit[..11.min(table.commit.len())],
            declared.len(),
            stale.len(),
            tally.gaps_only,
            tally.nothing_to_read,
        );
        for (cell, why) in &undeclared {
            let _ = writeln!(report, "  {cell}\n         {why}");
        }
        for cell in &stale {
            let _ = writeln!(
                report,
                "  {cell}: declared in tests/fixtures/expected_bytes_gaps.tsv and passing; remove it"
            );
        }
        panic!("{report}");
    }
}

#[test]
fn the_unreadable_cells_are_cyclones_24() {
    let counts: BTreeMap<&str, usize> = Dialect::ALL
        .iter()
        .map(|dialect| {
            (
                dialect.as_str(),
                run_dialect(dialect.as_str()).nothing_to_read,
            )
        })
        .filter(|(_, count)| *count > 0)
        .collect();
    assert_eq!(
        counts,
        [("cyclone", 24)].into_iter().collect(),
        "the cells no reader can return the sample from are Cyclone's 24 and no others"
    );
}

#[test]
fn the_table_names_known_dialects_and_corpus_cases() {
    let table = table();
    let known: BTreeSet<&str> = Dialect::ALL.iter().map(|d| d.as_str()).collect();
    let mut cases = BTreeSet::new();
    for row in &table.rows {
        for dialect in &row.dialects {
            assert!(
                known.contains(dialect.as_str()),
                "unknown dialect `{dialect}`"
            );
        }
        cases.insert(row.case.as_str());
    }
    for case in cases {
        assert!(
            corpus_root().join(case).join("type.idl").is_file(),
            "`{case}` is in the table and not in the corpus"
        );
    }
    assert_eq!(table.commit.len(), 40, "a full matrix commit id");
}

#[test]
fn specification() {
    dialect_matches_the_table("specification");
}

#[test]
fn cyclone() {
    dialect_matches_the_table("cyclone");
}

#[test]
fn rti() {
    dialect_matches_the_table("rti");
}

#[test]
fn fastdds() {
    dialect_matches_the_table("fastdds");
}

#[test]
fn opendds() {
    dialect_matches_the_table("opendds");
}

#[test]
fn opensplice() {
    dialect_matches_the_table("opensplice");
}

#[test]
fn hdds() {
    dialect_matches_the_table("hdds");
}

#[test]
fn omniorb() {
    dialect_matches_the_table("omniorb");
}

#[test]
fn tao() {
    dialect_matches_the_table("tao");
}

#[test]
fn jacorb() {
    dialect_matches_the_table("jacorb");
}

#[test]
fn ros_cyclone() {
    dialect_matches_the_table("ros-cyclone");
}

#[test]
fn ros_fastrtps() {
    dialect_matches_the_table("ros-fastrtps");
}
