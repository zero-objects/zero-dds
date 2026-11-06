// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `tests/interop-corpus/expected-bytes.tsv` as the oracle of a column
//! whose generated code runs under another toolchain.
//!
//! The table holds the octets other implementations wrote for every
//! corpus case, sample, encoding and byte order (decision 3 of
//! `internal/release/rc8-test-suite.md`): a vendor dialect expects that
//! vendor's own octets, `specification` the octets every DDS
//! implementation that wrote the cell agrees on. No column of this
//! project is read. Each producer checks its own octets against it in
//! its own tier: the Rust codec in `crates/wire-probe/tests/expected_bytes.rs`,
//! the C and the Python column through their matrix drivers
//! (`wireprobe-c-matrix`, `wireprobe-python-matrix`), whose answer
//! [`check`] holds to the table, one configuration at a time.
//!
//! What a column answers per slot is the matrix's alphabet
//! ([`crate::matrix`]): hex, `same`, `different: …`, or `!` and a
//! sentence. [`check`] compares the payload of `plain-cdr1` and
//! `plain-cdr2` — the table's `xcdr1` and `xcdr2`, framed as the
//! dialect frames the type — in both byte orders, and wants the column's
//! read of those octets back to be the sample (`same`): where the octets
//! are the vendor's, reading them back is reading the vendor's.
//!
//! The drivers answer the case's `values.json` only; the table's other
//! samples are counted as not covered. For a vendor whose alignment
//! gaps are undefined (the table's `gaps-undefined` lines) a difference
//! is accepted at octets the layout makes padding and nowhere else; the
//! padding positions come from the codec's own layout of the same
//! request, the octets everywhere else from the vendor.
//!
//! A cell a column does not reproduce is declared in the column's gaps
//! file, with its `OPEN-ITEMS.md` entry or, for a refusal of the
//! language that `docs/language-limits.md` documents, `-`, in the format
//! of `crates/wire-probe/tests/fixtures/expected_bytes_gaps.tsv`. A
//! declared cell that passes is a failure too, so the list cannot
//! outlive the gap.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;
use zerodds_ast::Ast;
use zerodds_ast_core::ScopedName;
use zerodds_ast_layout::{Encoding, Endianness};

use crate::canonical::{canonicalise, text};
use crate::decode::decode;
use crate::encode::{encode, encode_with_padding, model_from_idl_in_dialect};
use crate::matrix::Configuration;
use crate::value::ProbeRequest;

/// One line of the table.
pub struct Row {
    /// The corpus case.
    pub case: String,
    /// The sample of the case (`values`, …).
    pub sample: String,
    /// `xcdr1` or `xcdr2`.
    pub encoding: String,
    /// `le` or `be`.
    pub endian: String,
    /// The dialects that expect these octets.
    pub dialects: Vec<String>,
    /// The octets, as lower-case hex.
    pub hex: String,
}

impl Row {
    /// The matrix slot of this row under `configuration`.
    #[must_use]
    pub fn slot(&self, configuration: &str) -> String {
        let encoding = if self.encoding == "xcdr1" {
            "plain-cdr1"
        } else {
            "plain-cdr2"
        };
        format!("{configuration}/{encoding}/{}", self.endian)
    }

    /// The cell as the gaps files name it: `case/sample encoding endian`.
    #[must_use]
    pub fn cell(&self) -> String {
        format!(
            "{}/{} {} {}",
            self.case, self.sample, self.encoding, self.endian
        )
    }
}

/// The table.
pub struct Table {
    /// The `zerodds-matrix` commit it was generated from.
    pub commit: String,
    /// The dialects whose alignment gaps are undefined.
    pub gaps_undefined: BTreeSet<String>,
    /// Every row.
    pub rows: Vec<Row>,
}

/// Read `tests/interop-corpus/expected-bytes.tsv` under `root`.
///
/// # Errors
/// The file cannot be read or a line is not in the table's format.
pub fn table(root: &Path) -> Result<Table, String> {
    let path = root.join("tests/interop-corpus/expected-bytes.tsv");
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut commit = None;
    let mut gaps_undefined = BTreeSet::new();
    let mut rows = Vec::new();
    let mut header_seen = false;
    for line in text.lines() {
        if let Some(comment) = line.strip_prefix("# ") {
            if let Some(rest) = comment.strip_prefix("matrix-commit: ") {
                commit = rest.split_whitespace().next().map(str::to_string);
            }
            if let Some(rest) = comment.strip_prefix("gaps-undefined: ")
                && let Some(vendor) = rest.split(':').next()
            {
                gaps_undefined.insert(vendor.to_string());
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if !header_seen {
            if line != "case\tsample\tencoding\tendian\tdialects\thex" {
                return Err(format!("{}: unexpected header `{line}`", path.display()));
            }
            header_seen = true;
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6
            || !matches!(fields[2], "xcdr1" | "xcdr2")
            || !matches!(fields[3], "le" | "be")
        {
            return Err(format!("{}: not a row: `{line}`", path.display()));
        }
        rows.push(Row {
            case: fields[0].to_string(),
            sample: fields[1].to_string(),
            encoding: fields[2].to_string(),
            endian: fields[3].to_string(),
            dialects: fields[4].split(',').map(str::to_string).collect(),
            hex: fields[5].to_string(),
        });
    }
    let commit = commit.ok_or_else(|| format!("{}: no matrix-commit line", path.display()))?;
    if rows.is_empty() {
        return Err(format!("{}: no rows", path.display()));
    }
    Ok(Table {
        commit,
        gaps_undefined,
        rows,
    })
}

/// The first row of `case`'s `values` sample in `encoding` (`xcdr1` or
/// `xcdr2`) and `endian` (`le` or `be`), with the first dialect it names:
/// the octets a probe run under that dialect has to write, or read back to
/// the sample.
#[must_use]
pub fn row_of<'a>(
    table: &'a Table,
    case: &str,
    encoding: &str,
    endian: &str,
) -> Option<(&'a Row, &'a str)> {
    table
        .rows
        .iter()
        .find(|row| {
            row.case == case
                && row.sample == "values"
                && row.encoding == encoding
                && row.endian == endian
        })
        .and_then(|row| row.dialects.first().map(|dialect| (row, dialect.as_str())))
}

/// A gaps file: `dialect<TAB>case/sample<TAB>encoding<TAB>endian<TAB>OPEN-ITEMS<TAB>reason`,
/// `#` starts a comment; `-` in place of the entry marks a refusal
/// `docs/language-limits.md` documents. Keyed by the dialect and
/// [`Row::cell`].
///
/// # Errors
/// The file cannot be read or a line has not six fields.
pub fn declared(path: &Path) -> Result<BTreeMap<(String, String), String>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut out = BTreeMap::new();
    for line in text
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
    {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 {
            return Err(format!("{}: six fields: `{line}`", path.display()));
        }
        out.insert(
            (
                fields[0].to_string(),
                format!("{} {} {}", fields[1], fields[2], fields[3]),
            ),
            if fields[4] == "-" {
                fields[5].to_string()
            } else {
                format!("OPEN-ITEMS {}: {}", fields[4], fields[5])
            },
        );
    }
    Ok(out)
}

/// What one configuration of a column came to.
#[derive(Debug, Default)]
pub struct Verdict {
    /// Rows of the table checked.
    pub checked: usize,
    /// Rows that differ from the vendor's octets only in undefined gaps.
    pub gaps_only: usize,
    /// Rows whose octets are the vendor's and whose read does not return
    /// the sample, where the codec's own writer and reader under the same
    /// request do not return it either: the frame carries no branch for
    /// the member the sample names (Cyclone's negative narrow case
    /// labels, `crates/wire-probe/tests/expected_bytes.rs`).
    pub nothing_to_read: usize,
    /// Rows of samples the drivers do not answer (not `values`).
    pub not_covered: usize,
    /// Declared cells that failed as declared.
    pub declared: usize,
    /// Every failure, by cell, and every declared cell that passed.
    pub failures: BTreeMap<String, String>,
}

impl Verdict {
    /// The failures as a report, empty when there are none.
    #[must_use]
    pub fn report(&self, column: &str, configuration: &str, table: &Table) -> String {
        if self.failures.is_empty() {
            return String::new();
        }
        let mut out = format!(
            "the {column} column under `{configuration}`: {} of {} cells differ from \
             tests/interop-corpus/expected-bytes.tsv (matrix {}; {} declared, {} in undefined \
             gap octets only, {} carry no branch to read, {} rows of other samples not \
             covered)\n",
            self.failures.len(),
            self.checked,
            table.commit,
            self.declared,
            self.gaps_only,
            self.nothing_to_read,
            self.not_covered,
        );
        for (cell, why) in &self.failures {
            let _ = writeln!(out, "  {cell}\n         {why}");
        }
        out
    }
}

/// One corpus case as the drivers read it.
struct Case {
    type_name: String,
    idl: String,
    values: Value,
}

fn read_case(root: &Path, name: &str) -> Result<Case, String> {
    let directory = root.join("tests/interop-corpus").join(name);
    let read = |file: &str| {
        std::fs::read_to_string(directory.join(file))
            .map_err(|error| format!("{name}/{file}: {error}"))
    };
    Ok(Case {
        type_name: read("type")?.trim().to_string(),
        idl: read("type.idl")?,
        values: serde_json::from_str(&read("values.json")?)
            .map_err(|error| format!("{name}/values.json: {error}"))?,
    })
}

/// The case, its model under the configuration's dialect and the
/// request of the row.
fn model(
    root: &Path,
    row: &Row,
    configuration: &Configuration,
) -> Result<(Case, Ast, ProbeRequest), String> {
    let case = read_case(root, &row.case)?;
    let ast = model_from_idl_in_dialect(
        &case.idl,
        &format!("{}/type.idl", row.case),
        configuration.dialect,
    )
    .map_err(|error| error.to_string())?;
    let encoding = if row.encoding == "xcdr1" {
        Encoding::PlainCdr1
    } else {
        Encoding::PlainCdr2
    };
    let endianness = if row.endian == "le" {
        Endianness::Little
    } else {
        Endianness::Big
    };
    let request = configuration.request(&case.type_name, encoding, endianness);
    Ok((case, ast, request))
}

/// Whether the codec's own writer and reader under the row's request
/// return the sample.
fn the_codec_reads_its_own_back(
    root: &Path,
    row: &Row,
    configuration: &Configuration,
) -> Result<bool, String> {
    let (case, ast, request) = model(root, row, configuration)?;
    let name = ScopedName::absolute(
        case.type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
            .collect(),
    );
    let type_ = ast
        .find_by_scoped_name(&name)
        .map_err(|lookup| format!("the model has no {}: {lookup:?}", case.type_name))?
        .as_type()
        .ok_or_else(|| format!("{} names something that is not a type", case.type_name))?;
    let expected = canonicalise(&ast, type_, &case.values, "sample")
        .map(|value| text(&value))
        .map_err(|error| format!("the sample does not canonicalise: {error}"))?;
    let bytes = encode(&ast, &request, &case.values).map_err(|error| error.to_string())?;
    Ok(decode(&ast, &request, &bytes).is_ok_and(|value| text(&value) == expected))
}

/// Whether `ours` and `theirs` differ only where the codec's layout of
/// the same request puts padding.
fn only_in_gaps(
    root: &Path,
    row: &Row,
    configuration: &Configuration,
    ours: &str,
) -> Result<bool, String> {
    if ours.len() != row.hex.len() {
        return Ok(false);
    }
    let (case, ast, request) = model(root, row, configuration)?;
    let (_, padding) =
        encode_with_padding(&ast, &request, &case.values).map_err(|error| error.to_string())?;
    let ours = ours.as_bytes();
    let theirs = row.hex.as_bytes();
    Ok((0..ours.len() / 2).all(|octet| {
        ours[2 * octet..2 * octet + 2] == theirs[2 * octet..2 * octet + 2]
            || matches!(padding.get(octet), Some(true))
    }))
}

/// Hold one configuration of a column's matrix answer to the table.
///
/// `column` is the driver's JSON, `case → slot → answer`, for this
/// configuration and at least the encodings `plain-cdr1` and
/// `plain-cdr2`. `declared` is the column's gaps file ([`declared`]).
#[must_use]
pub fn check(
    root: &Path,
    table: &Table,
    configuration: &Configuration,
    column: &BTreeMap<String, BTreeMap<String, String>>,
    declared: &BTreeMap<(String, String), String>,
) -> Verdict {
    let name = configuration.name;
    let gaps_undefined = table.gaps_undefined.contains(name);
    let mut verdict = Verdict::default();
    let mut failed: BTreeMap<String, String> = BTreeMap::new();
    for row in table
        .rows
        .iter()
        .filter(|row| row.dialects.iter().any(|dialect| dialect == name))
    {
        if row.sample != "values" {
            verdict.not_covered += 1;
            continue;
        }
        verdict.checked += 1;
        let slot = row.slot(name);
        let answers = column.get(&row.case);
        let written = answers.and_then(|answers| answers.get(&slot));
        let read = answers.and_then(|answers| answers.get(&format!("{slot}/read")));
        let why = match written {
            None => Some("no answer: the driver did not write this slot".to_string()),
            Some(ours) if ours.starts_with('!') => Some(format!("write refused: {}", &ours[1..])),
            Some(ours) if *ours != row.hex => {
                match gaps_undefined.then(|| only_in_gaps(root, row, configuration, ours)) {
                    Some(Ok(true)) => {
                        verdict.gaps_only += 1;
                        None
                    }
                    Some(Err(error)) => Some(format!("the padding positions: {error}")),
                    _ => Some(format!(
                        "write: expected {}\n           found    {ours}",
                        row.hex
                    )),
                }
            }
            Some(_) => None,
        }
        .or_else(|| match read.map(String::as_str) {
            Some("same") => None,
            Some(other)
                if other.starts_with("different: ")
                    && matches!(
                        the_codec_reads_its_own_back(root, row, configuration),
                        Ok(false)
                    ) =>
            {
                verdict.nothing_to_read += 1;
                None
            }
            Some(other) => Some(format!("read back: {other}")),
            None => Some("no answer for the read direction".to_string()),
        });
        if let Some(why) = why {
            failed.insert(row.cell(), why);
        }
    }
    for (cell, why) in &failed {
        if declared.contains_key(&(name.to_string(), cell.clone())) {
            verdict.declared += 1;
        } else {
            verdict.failures.insert(cell.clone(), why.clone());
        }
    }
    for (dialect, cell) in declared.keys() {
        if dialect == name && !failed.contains_key(cell) {
            verdict.failures.insert(
                cell.clone(),
                "declared in the column's gaps file and passing; remove it".to_string(),
            );
        }
    }
    verdict
}
