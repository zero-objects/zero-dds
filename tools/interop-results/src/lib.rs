// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-interop-results`. Safety classification: **TOOLING**.
//!
//! The interop result tree, and the two tables generated from it.
//!
//! One row per (case, encoding, byte order); one column per
//! implementation; one file per cell that produced bytes. Two tables of
//! the same shape, one for the write direction and one for the read
//! direction.
//!
//! ## Why this is generated and not written
//!
//! 3,760 rows and sixteen columns is 60,160 cells. A table that size
//! maintained by hand is wrong within a day, and a figure in a table
//! that can disagree with the figure a gate prints is the defect the
//! whole interop effort has been eliminating. So there is one
//! machine-readable record — `Ledger` — every table cell is rendered
//! from it, and the same record is what `--verify` compares a fresh run
//! against.
//!
//! ## Where the bytes come from
//!
//! | column | producer | reachable without a licensed toolchain |
//! |---|---|---|
//! | ZeroDDS Rust | this process, `zerodds_wire_probe::encode` | yes |
//! | ZeroDDS C | `wireprobe-c-matrix`, via the three-column gate | yes, with a C compiler |
//! | ZeroDDS Python | `tools/wireprobe/corpus-python.py --matrix` | yes, with CPython |
//! | Cyclone, RTI, Fast DDS, OpenDDS | `tools/interop-results/vendor-corpus-sweep.sh` | Cyclone and Fast DDS in the `ci-interop` image; RTI never |
//! | omniORB, TAO, JacORB | `tools/corba-oracle/corba-sweep.sh` | no |
//! | the two ROS 2 type supports | `tools/interop-results/ros-corpus-sweep.sh` | no |
//!
//! Each producer writes a TSV under `tests/interop-results/sources/`,
//! and those TSVs are the committed record of what the systems
//! produced. The tree of `.bin` files is rendered from them, which is
//! why regenerating the tree needs no toolchain at all and regenerating
//! a *source* needs the system that wrote it.
#![forbid(unsafe_code)]

pub mod assemble;
pub mod dialect_fit;
pub mod ledger;
pub mod page;
pub mod reproducible;
pub mod rust_column;
pub mod sources;
pub mod systems;
pub mod tables;
pub mod tree;
pub mod warnings;

use std::path::{Path, PathBuf};

/// The five encodings, under the names every probe in this project
/// takes on its `--encoding` flag.
pub const ENCODINGS: [&str; 5] = [
    "plain-cdr1",
    "pl-cdr1",
    "plain-cdr2",
    "delimited-cdr",
    "pl-cdr2",
];

/// The two byte orders, under the names every probe takes on `--endian`.
pub const ORDERS: [&str; 2] = ["le", "be"];

/// Which encoding version an encoding belongs to.
///
/// A vendor's `--encoding` selects the **version**; the framing follows
/// the type's own extensibility, which XTypes 1.3 §7.3.1.2.1.8 requires
/// of it. `wireprobe-cyclone` and `wireprobe-fastdds` both fold
/// `plain-cdr1`/`pl-cdr1` onto version 1 and the other three onto
/// version 2 for that reason. So a foreign column answers two questions
/// and five rows point at those two answers rather than at five
/// identical runs.
#[must_use]
pub fn encoding_version(encoding: &str) -> &'static str {
    match encoding {
        "plain-cdr1" | "pl-cdr1" => "xcdr1",
        _ => "xcdr2",
    }
}

/// The encoding whose run a foreign column's row for `encoding` *is*.
#[must_use]
pub fn canonical_encoding(encoding: &str) -> &'static str {
    match encoding_version(encoding) {
        "xcdr1" => "plain-cdr1",
        _ => "plain-cdr2",
    }
}

/// One corpus case, as the corpus directory holds it.
#[derive(Debug, Clone)]
pub struct Case {
    /// The corpus directory's name, which is also its name in the tree.
    pub name: String,
    /// The top-level type's scoped name.
    pub type_name: String,
    /// The case's `type.idl`, verbatim.
    pub idl: String,
    /// The case's `values.json`, parsed.
    pub values: serde_json::Value,
    /// The one-line description the table's second column carries,
    /// taken from the case's own README so that the table and the case
    /// cannot drift apart.
    pub description: String,
}

/// The repository root, found from this crate's manifest.
///
/// `tools/interop-results` is two directories under it, and the
/// manifest directory is absolute, so the two ancestors are always
/// there. Falling back to the manifest directory rather than panicking
/// keeps a build in an unusual layout producing a diagnosable path
/// instead of a stack trace.
#[must_use]
pub fn repository_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .unwrap_or(manifest)
        .to_path_buf()
}

/// Where the result tree lives.
#[must_use]
pub fn results_root(root: &Path) -> PathBuf {
    root.join("tests").join("interop-results")
}

/// Every corpus case, in the order the tables number them.
///
/// # Errors
///
/// When the corpus directory cannot be read, or a case is missing one
/// of the four files that make it a case.
pub fn corpus(root: &Path) -> Result<Vec<Case>, String> {
    let directory = root.join("tests").join("interop-corpus");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .into_iter()
        .filter(|path| path.join("type").is_file())
        .collect();
    paths.sort();
    paths.into_iter().map(read_case).collect()
}

fn read_case(directory: PathBuf) -> Result<Case, String> {
    let name = directory
        .file_name()
        .ok_or_else(|| format!("{}: no case name", directory.display()))?
        .to_string_lossy()
        .into_owned();
    let read = |file: &str| -> Result<String, String> {
        std::fs::read_to_string(directory.join(file))
            .map_err(|error| format!("{name}/{file}: {error}"))
    };
    let values_text = read("values.json")?;
    Ok(Case {
        description: describe(&read("README.md").unwrap_or_default()),
        type_name: read("type")?.trim().to_string(),
        idl: read("type.idl")?,
        values: serde_json::from_str(&values_text)
            .map_err(|error| format!("{name}/values.json: {error}"))?,
        name,
    })
}

/// The one line a case's README says the case is for.
///
/// The `**Construct**` line names the construct and the paragraph under
/// it says what the case would catch; the table carries the construct
/// and the first sentence of the paragraph. Taken from the README and
/// not written here a second time: two descriptions of one case is one
/// description too many.
fn describe(readme: &str) -> String {
    let mut construct = String::new();
    let mut sentence = String::new();
    let mut after_construct = false;
    for line in readme.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("**Construct** — ") {
            construct = rest.trim().to_string();
            after_construct = true;
            continue;
        }
        if after_construct && !line.is_empty() && !line.starts_with('#') {
            sentence = first_sentence(line);
            break;
        }
    }
    match (construct.is_empty(), sentence.is_empty()) {
        (true, true) => "—".to_string(),
        (true, false) => sentence,
        (false, true) => construct,
        (false, false) => format!("{construct} — {sentence}"),
    }
}

/// The first sentence of a paragraph, with the pipes a table cell
/// cannot carry escaped and the length bounded.
fn first_sentence(text: &str) -> String {
    let mut out = String::new();
    let mut ticks = 0_usize;
    for (index, character) in text.char_indices() {
        if character == '`' {
            ticks += 1;
        }
        out.push(character);
        // A full stop inside backticks is part of a name, not the end
        // of a sentence: `@id(1)` and `3.14` both carry one.
        if character == '.' && ticks % 2 == 0 {
            let next = text[index + 1..].chars().next();
            if next.is_none() || next == Some(' ') {
                break;
            }
        }
    }
    let out = out.trim().to_string();
    let out = if out.chars().count() > 200 {
        let cut: String = out.chars().take(197).collect();
        format!("{cut}…")
    } else {
        out
    };
    out.replace('|', "\\|")
}
