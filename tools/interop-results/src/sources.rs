// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The committed record of what each foreign system produced, read in
//! the shape its own harness wrote it.
//!
//! Read in the harness's shape and not in one normalised on the way in:
//! a file that has been rewritten is a file somebody could have
//! rewritten wrongly. `tests/interop-results/sources/` holds what the
//! sweeps printed, and the normalising into the ledger's shape happens
//! here, in code that a test exercises.
//!
//! The one thing that does not survive into the record is the machine:
//! see [`without_the_machine`], which runs over the recorded files
//! themselves rather than over the reading of them, because the record
//! has to be a function of the case and not of the host.

use std::collections::BTreeMap;
use std::path::Path;

/// One measured cell of a source file, keyed by
/// `<case>/<system>/<version>/<endian>`.
pub type Measured = BTreeMap<String, Answer>;

/// What a source file recorded for one cell.
#[derive(Debug, Clone)]
pub enum Answer {
    /// The octets the system produced, as lower-case hex.
    Bytes(String),
    /// The system ran and would not take the type or the value.
    Refused(String),
    /// The front end has no form for the construct, so no probe of this
    /// family ever saw the case.
    NoForm(String),
    /// The front end has the construct and a documented mapping rule
    /// makes the octets differ. A measurement of the toolchain.
    MappingDivergence(String),
    /// The construct is the target's and a tool of ours has not
    /// written it. Not a measurement of the toolchain at all.
    HarnessGap(String),
}

fn key(case: &str, system: &str, version: &str, endian: &str) -> String {
    format!("{case}/{system}/{version}/{endian}")
}

/// Look one cell up.
#[must_use]
pub fn get<'a>(
    measured: &'a Measured,
    case: &str,
    system: &str,
    version: &str,
    endian: &str,
) -> Option<&'a Answer> {
    measured.get(&key(case, system, version, endian))
}

/// The six-column shape `vendor-corpus-sweep.sh` and
/// `ros-corpus-sweep.sh` write:
/// `case, system, version, endian, status, bytes-or-reason`.
///
/// # Errors
///
/// When a line has the wrong number of columns. A short line is a
/// truncated run, and a truncated run read as a smaller matrix is the
/// failure this whole exercise is about.
pub fn read_six_column(path: &Path) -> Result<Measured, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Measured::new()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let mut measured = Measured::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 {
            return Err(format!(
                "{}:{}: {} columns, not 6",
                path.display(),
                index + 1,
                fields.len()
            ));
        }
        let answer = match fields[4] {
            "wrote" => Answer::Bytes(fields[5].to_string()),
            "refused" => Answer::Refused(fields[5].to_string()),
            other => {
                return Err(format!(
                    "{}:{}: `{other}` is not `wrote` or `refused`",
                    path.display(),
                    index + 1
                ));
            }
        };
        measured.insert(key(fields[0], fields[1], fields[2], fields[3]), answer);
    }
    Ok(measured)
}

/// The five-column shape `tools/corba-oracle/corba-sweep.sh` writes:
/// `case, probe, endian, direction, hex-or-reason`, plus a `NOFORM`
/// line for a case no ORB front end ever saw.
///
/// An ORB has no encoding version 2, so every row is version 1.
///
/// # Errors
///
/// When a line has the wrong number of columns.
pub fn read_corba(path: &Path) -> Result<Measured, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Measured::new()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let mut measured = Measured::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 5 {
            return Err(format!(
                "{}:{}: {} columns, not 5",
                path.display(),
                index + 1,
                fields.len()
            ));
        }
        let (case, probe, endian, detail) = (fields[0], fields[1], fields[2], fields[4]);
        // One line stands for all three ORBs and both orders: the
        // rewrite never handed the case to a front end, so no ORB
        // refused it. Which of the three answers it is comes from the
        // marker the sweep wrote, which came from an exit code —
        // `docs/specs/zerodds-verdicts-1.0.md`.
        let shared = match probe {
            "NOFORM" => Some(Answer::NoForm(detail.to_string())),
            "MAPSDIFFERENTLY" => Some(Answer::MappingDivergence(detail.to_string())),
            "HARNESSGAP" => Some(Answer::HarnessGap(detail.to_string())),
            _ => None,
        };
        if let Some(answer) = shared {
            for orb in ["omniorb", "tao", "jacorb"] {
                for order in crate::ORDERS {
                    measured.insert(key(case, orb, "xcdr1", order), answer.clone());
                }
            }
            continue;
        }
        // The ZeroDDS rows of this sweep are the probe's own bytes
        // under no dialect, which this tool measures for itself. Kept
        // out rather than folded in: one column measured two ways is a
        // column that can disagree with itself.
        if probe == "zerodds" {
            continue;
        }
        let answer = if let Some(reason) = detail.strip_prefix("ERR") {
            Answer::Refused(
                reason
                    .split_once(':')
                    .map_or(detail, |(_, message)| message)
                    .to_string(),
            )
        } else {
            Answer::Bytes(detail.to_string())
        };
        measured.insert(key(case, probe, "xcdr1", endian), answer);
    }
    Ok(measured)
}

/// One of the two foreign ZeroDDS columns, out of the matrix JSON the
/// three-column gate's run leaves behind.
///
/// The JSON is `case → slot → cell`, the slot being
/// `<configuration>/<encoding>/<order>` and the cell hex, `same`,
/// `different: …` or `!` and a sentence. Only the default
/// configuration reaches the tree: the other nine are the
/// mitigation column's search space and are measured by the Rust
/// column in process.
///
/// # Errors
///
/// When the file is not the JSON object that gate writes.
pub fn read_column_matrix(path: &Path) -> Result<ColumnMatrix, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ColumnMatrix::default());
        }
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let parsed: BTreeMap<String, BTreeMap<String, serde_json::Value>> =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut out = ColumnMatrix::default();
    for (case, slots) in parsed {
        for (slot, cell) in slots {
            let Some(rest) = slot.strip_prefix(&format!("{}/", crate::rust_column::DEFAULT)) else {
                continue;
            };
            // `<encoding>/<order>`, `<encoding>/<order>+enc`,
            // `<encoding>/<order>/read` and `<encoding>/keyhash`. The
            // tree holds the payload and the read verdict; the
            // encapsulated form and the KeyHash are the three-column
            // gate's business and are not a row of these tables.
            let text = cell.as_str().unwrap_or_default().to_string();
            if let Some(stem) = rest.strip_suffix("/read") {
                out.read.insert(format!("{case}/{stem}"), text);
            } else if !rest.ends_with("+enc") && !rest.ends_with("/keyhash") {
                out.write.insert(format!("{case}/{rest}"), text);
            }
        }
    }
    Ok(out)
}

/// One foreign ZeroDDS column: the payload and the read verdict, keyed
/// by `<case>/<encoding>/<order>`.
#[derive(Debug, Default)]
pub struct ColumnMatrix {
    /// The payload, keyed by `<case>/<encoding>/<order>`.
    pub write: BTreeMap<String, String>,
    /// The read verdict, under the same keys.
    pub read: BTreeMap<String, String>,
}

/// Turn the documents a column answered a `/read` slot with into the
/// three words the tree holds.
///
/// `tools/wireprobe/corpus-python.py` answers a `/read` slot with
/// `json:` and the document its own `to_json` produced over what its
/// decoder built. It reaches no verdict, and that is deliberate: a
/// verdict reached in Python is a verdict about Python objects held
/// against that column's own `from_json`, so a value its decoder can
/// build which no writer would emit falls outside it. That is the shape
/// of the `fixed` defect the three-column gate found, where the reader
/// returned the BCD field's leading zeros and the only check re-encoded
/// what it read.
///
/// `crates/wire-probe/src/canonical.rs` decides here instead, over both
/// the document and the case's own sample — the same comparison
/// `crates/wire-probe/tests/three_column_gate.rs` makes, and the same
/// one `wireprobe-c-matrix` makes with the C probe's `--values-out`, so
/// the three columns' read verdicts are three answers to one question.
///
/// Left out, the fallback below reads a document as a verdict it is
/// not: measured on 2026-08-10, 3,330 of the Python column's 3,330 read
/// rows came out `different` while the documents were the sample. A
/// column that says a binding disagrees with itself everywhere is worse
/// than no column, because it is specific.
pub fn resolve_read_documents(cases: &[crate::Case], matrix: &mut ColumnMatrix) {
    use zerodds_ast_core::ScopedName;
    use zerodds_wire_probe::canonical::{canonicalise, text};
    use zerodds_wire_probe::encode::model_from_idl;

    for case in cases {
        // Nothing is skipped quietly: a case whose model or sample will
        // not canonicalise turns every one of its documents into a
        // refusal that names why, rather than into a verdict.
        let prepared = model_from_idl(&case.idl, &format!("{}/type.idl", case.name))
            .map_err(|error| format!("!the IDL does not lower: {error}"))
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
                    .ok_or_else(|| "!the type is not in the model".to_string())?;
                let expected = canonicalise(&ast, type_, &case.values, "sample")
                    .map_err(|error| format!("!the sample does not canonicalise: {error}"))?;
                Ok((ast, type_, text(&expected)))
            });
        let prefix = format!("{}/", case.name);
        for (slot, cell) in &mut matrix.read {
            if !slot.starts_with(&prefix) {
                continue;
            }
            let Some(document) = cell.strip_prefix("json:") else {
                continue;
            };
            *cell = match &prepared {
                Err(refusal) => refusal.clone(),
                Ok((ast, type_, expected)) => {
                    match serde_json::from_str::<serde_json::Value>(document) {
                        Err(error) => format!("!the column's JSON does not parse: {error}"),
                        Ok(found) => match canonicalise(ast, *type_, &found, "sample") {
                            Ok(value) if &text(&value) == expected => "same".to_string(),
                            Ok(value) => format!("different: {} for {expected}", text(&value)),
                            Err(error) => {
                                format!("!what came back is not a value of this type: {error}")
                            }
                        },
                    }
                }
            };
        }
    }
}

/// A column matrix, as the two six-column source TSVs that get
/// committed: one per direction.
///
/// The JSON the gate leaves in `target/` is nine million characters of
/// twelve configurations this tree does not hold. What is committed is the
/// one configuration it does, in the shape the foreign sweeps write, so
/// that one reader takes every source in `sources/`.
///
/// # Errors
///
/// When a read cell is none of the four shapes a column answers with.
/// It is never rendered as a verdict: the one cell shape that used to
/// fall through here was `json:` and a document, and reading a document
/// as `different` produced three thousand disagreements that were not
/// disagreements. An unknown cell is a harness that changed under this
/// reader, and the import stops rather than committing a guess.
pub fn render_column_source(
    system: &str,
    matrix: &ColumnMatrix,
) -> Result<(String, String), String> {
    use std::fmt::Write as _;
    let split = |slot: &str| -> Option<(String, String, String)> {
        let mut parts = slot.rsplitn(3, '/');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(order), Some(encoding), Some(case)) => {
                Some((case.to_string(), encoding.to_string(), order.to_string()))
            }
            _ => None,
        }
    };
    let mut write = String::new();
    for (slot, cell) in &matrix.write {
        let Some((case, encoding, order)) = split(slot) else {
            continue;
        };
        let (status, detail) = cell
            .strip_prefix('!')
            .map_or(("wrote", cell.as_str()), |message| ("refused", message));
        let _ = writeln!(
            write,
            "{case}\t{system}\t{encoding}\t{order}\t{status}\t{}",
            detail.replace('\t', " ")
        );
    }
    let mut read = String::new();
    for (slot, cell) in &matrix.read {
        let Some((case, encoding, order)) = split(slot) else {
            continue;
        };
        let (status, detail) = if cell == "same" {
            ("same", String::new())
        } else if let Some(detail) = cell.strip_prefix("different: ") {
            ("different", detail.to_string())
        } else if let Some(message) = cell.strip_prefix('!') {
            if message.starts_with("unwritten") {
                ("unwritten", message.to_string())
            } else {
                ("refused", message.to_string())
            }
        } else {
            return Err(format!(
                "{system}: {case}/{encoding}/{order} answered `{}`, which is not one of \
                 `same`, `different: …`, `!…` or `json:…`",
                cell.chars().take(60).collect::<String>()
            ));
        };
        let _ = writeln!(
            read,
            "{case}\t{system}\t{encoding}\t{order}\t{status}\t{}",
            detail.replace('\t', " ")
        );
    }
    Ok((write, read))
}

/// A read-direction source file: the same six columns, with a verdict
/// where a write file has `wrote` or `refused`. Keyed like a write
/// file, so one lookup serves both directions.
///
/// # Errors
///
/// When a line has the wrong number of columns or an unknown verdict.
pub fn read_verdicts(path: &Path) -> Result<BTreeMap<String, (String, String)>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let mut out = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 {
            return Err(format!(
                "{}:{}: {} columns, not 6",
                path.display(),
                index + 1,
                fields.len()
            ));
        }
        // The whole vocabulary and not the four words this reader began
        // with. `read-corpus-sweep.sh` answers in the probe's own exit
        // code — `docs/specs/zerodds-verdicts-1.0.md`'s table — so a
        // front end whose generated code does not build is
        // `generator-defect` and a gap in a tool of ours is
        // `harness-gap`, where both used to arrive as `refused`.
        // `Outcome::parse` is what renders them, and a token it does not
        // know is refused here rather than turned into a cell that says
        // something else.
        if crate::ledger::Outcome::parse(fields[4], fields[5]).is_none() {
            return Err(format!(
                "{}:{}: `{}` is not a read verdict; \
                 docs/specs/zerodds-verdicts-1.0.md has the vocabulary",
                path.display(),
                index + 1,
                fields[4]
            ));
        }
        out.insert(
            key(fields[0], fields[1], fields[2], fields[3]),
            (fields[4].to_string(), fields[5].to_string()),
        );
    }
    Ok(out)
}

// -- the machine a row was measured on ------------------------------------

/// A recorded diagnostic with everything that belongs to the *machine*
/// and to the *run* taken out of it, and nothing else touched.
///
/// ## Why the record and not the page
///
/// A probe names the file it was given, and it is given an absolute
/// path. Four shapes of that reached the committed record, and every
/// one of them makes the record something other than a function of the
/// case:
///
///  * `/root/zerodds/tests/interop-corpus/<case>/type.idl` and
///    `/root/zd-orbros/…` — the checkout the sweep ran in. Two hosts
///    recording the same refusal write two different rows;
///  * `/root/zerodds/target/wireprobe-c-cache/build-929565-<digest>-full/gen.h`
///    — `wireprobe-c-matrix` names its build directory after
///    [`std::process::id`], so **two runs on one host** write two
///    different rows. The five committed messages all carry `929565`
///    because they are all from one run;
///  * `/tmp/tmp.y5n477KY11/gen.idl` — a `mktemp -d`, which is random by
///    construction and is `/var/folders/…/T/tmp.…` where `TMPDIR` is
///    not `/tmp`;
///  * `/root/zd-wire/tools/wire-capture/wire-golden.sh: line 585:
///    2588703 Aborted  ( cd … )` — bash's report of a signalled child,
///    which carries a path, a process id and a line number from one
///    revision of one script. `vendor-corpus-sweep.sh`'s `reason_of`
///    already reduces exactly this to the one thing that is a property
///    of the vendor; `wire-golden.sh` has no such step and two rows
///    carry the raw form.
///
/// Rendering it out of the page instead would leave all four in
/// `sources/`, and `sources/` is what `verify-record.sh` diffs a
/// re-record against — so a second run on the same host would report
/// rows as moved that nobody moved. The record is where this belongs,
/// and this is the funnel: `--normalise` writes it over the recorded
/// files, `regenerate.sh` runs it before every regeneration, and
/// `tests/committed_tree.rs::the_record_carries_no_machine` is the
/// gate.
///
/// ## What it never removes
///
/// The file a diagnostic names and the line and column it points at are
/// the statement, and they are kept: `gen.h:48:12: error: field 'next'
/// has incomplete type` says the same thing after this as before it.
/// Only the way to the file goes, and what is left is repo-relative.
#[must_use]
pub fn without_the_machine(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let _ = index;
        let (body, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |body| (body, "\n"));
        out.push_str(&one_line_without_the_machine(body));
        out.push_str(newline);
    }
    out
}

/// The path characters a diagnostic's path is made of. A space ends a
/// path, which is what lets the prefix of one be found by walking back.
fn is_path_character(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '+' | '~')
}

fn one_line_without_the_machine(line: &str) -> String {
    // The signalled-child report is anchored at the start of the field
    // it is in — a record's line is tab-separated columns and the
    // report is the last of them — so the fields are taken apart here
    // and put back unchanged but for that one.
    let line = line
        .split('\t')
        .map(|field| killed_probe(field).unwrap_or_else(|| field.to_string()))
        .collect::<Vec<_>>()
        .join("\t");
    let line = without_temporary_directory(&line);
    let line = without_build_directory(&line);
    let mut line = line;
    for anchor in ANCHORS {
        line = made_repo_relative(&line, anchor);
    }
    corpus_double_slash(&line)
}

/// A directory of this repository. Anything absolute in front of one of
/// these is the checkout the run happened in, and only that.
///
/// The list is short and named rather than derived, because a rule that
/// stripped everything in front of any `tools/` would also rewrite a
/// vendor's own install path — `/opt/opendds/bin/tao_idl` is a
/// measurement of the toolchain and stays whole.
const ANCHORS: [&str; 4] = [
    "tests/interop-corpus/",
    "tests/interop-results/",
    "tools/wire-capture/",
    "tools/wireprobe/",
];

/// `<absolute prefix><anchor>` → `<anchor>`, at every occurrence.
fn made_repo_relative(line: &str, anchor: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find(anchor) {
        let (before, from_anchor) = rest.split_at(at);
        // Walk back over the path this anchor is the tail of. Only an
        // *absolute* prefix is the machine's; a relative one is already
        // repo-relative and is left alone.
        let start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_path_character(*c))
            .last()
            .map_or(before.len(), |(index, _)| index);
        if before[start..].starts_with('/') {
            out.push_str(&before[..start]);
        } else {
            out.push_str(before);
        }
        out.push_str(anchor);
        rest = &from_anchor[anchor.len()..];
    }
    out.push_str(rest);
    out
}

/// `…/target/wireprobe-c-cache/build-<pid>-<digest>-<variant>/gen.h` →
/// `gen.h`.
///
/// The whole path goes and not only the absolute part of it: the
/// process id and the digest are the run's identity, the variant is the
/// cache's bookkeeping and the cache directory is where this harness
/// happens to build. What the diagnostic is about is `gen.h` and the
/// line in it, and a bare file name is what every other column's
/// diagnostics carry — `gen.hpp: error: …`, `fill.inc: warning: …`.
/// `compile` in `tools/wireprobe/wireprobe-zerodds-c-gen/src/bin/wireprobe-c-matrix.rs`
/// now writes the same form, so a re-record and this agree.
fn without_build_directory(line: &str) -> String {
    const CACHE: &str = "target/wireprobe-c-cache/";
    let line = made_repo_relative(line, CACHE);
    let mut out = String::with_capacity(line.len());
    let mut rest = line.as_str();
    while let Some(at) = rest.find(CACHE) {
        let (before, from_cache) = rest.split_at(at);
        out.push_str(before);
        let after = &from_cache[CACHE.len()..];
        let segment = after.split('/').next().unwrap_or("");
        if segment.starts_with("build-") && after.len() > segment.len() {
            rest = &after[segment.len() + 1..];
        } else {
            out.push_str(CACHE);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// A `mktemp -d` directory, removed with the absolute path leading to
/// it, so what is left is the path relative to the directory the run
/// worked in: `gen.idl`, or `chunk.000.d/build/gen.idl`.
///
/// `TMPDIR` is not `/tmp` everywhere — macOS gives
/// `/var/folders/…/T/tmp.i4YfZRnLcU` — so the segment is recognised by
/// `mktemp`'s own template and not by the directory it sits in.
fn without_temporary_directory(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find("/tmp.") {
        let after = &rest[at + "/tmp.".len()..];
        let suffix: &str = after.split('/').next().unwrap_or("");
        let is_template = suffix.len() >= 6
            && suffix.chars().all(|c| c.is_ascii_alphanumeric())
            && after.len() > suffix.len();
        if !is_template {
            let step = at + "/tmp.".len();
            out.push_str(&rest[..step]);
            rest = &rest[step..];
            continue;
        }
        let before = &rest[..at];
        let start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| is_path_character(*c))
            .last()
            .map_or(before.len(), |(index, _)| index);
        if before[start..].starts_with('/') {
            out.push_str(&before[..start]);
        } else {
            out.push_str(before);
        }
        rest = &after[suffix.len() + 1..];
    }
    out.push_str(rest);
    out
}

/// `tests/interop-corpus/<case>//type.idl` → one separator.
///
/// `corba-sweep.sh` joins a directory that already ends in a separator
/// with one that begins with one, so the ORB rows record a path that
/// every other sweep records with one slash. Not the machine and not
/// the run — but it is the same statement written two ways, and the two
/// halves of the record are compared against each other.
fn corpus_double_slash(line: &str) -> String {
    const CORPUS: &str = "tests/interop-corpus/";
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find(CORPUS) {
        let (before, from_corpus) = rest.split_at(at);
        out.push_str(before);
        out.push_str(CORPUS);
        let after = &from_corpus[CORPUS.len()..];
        let case: &str = after.split('/').next().unwrap_or("");
        let tail = &after[case.len()..];
        out.push_str(case);
        if let Some(collapsed) = tail.strip_prefix("//") {
            out.push('/');
            rest = collapsed;
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

/// Bash's report of a child that was signalled, with the three things
/// in it that belong to the machine taken off the front.
///
///   `<path>: line <n>: <pid> Segmentation fault      <command>`
///
/// The path is the machine's, the process id is the run's and the line
/// number is one revision of one script's. What happened is that the
/// probe was killed and by what, which is `reason_of`'s reading in
/// `vendor-corpus-sweep.sh` and is why that sweep's rows already say
/// `the probe was killed: Segmentation fault`.
///
/// Only the prefix goes, where `reason_of` also drops the command after
/// it. In `wire-goldens.tsv` the field does not end at the command:
/// omniidl's own four diagnostics are appended to it — `gen.idl:1:
/// Syntax error in union declaration` and three more — and those are
/// the statement the row is about. A rule that ended the field at the
/// signal name would delete them.
fn killed_probe(field: &str) -> Option<String> {
    let at = field.find(": line ")?;
    let path = &field[..at];
    if !path.contains('/') || path.contains(char::is_whitespace) {
        return None;
    }
    let (number, rest) = field[at + ": line ".len()..].split_once(": ")?;
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (pid, rest) = rest.split_once(' ')?;
    if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if rest.trim().is_empty() {
        return None;
    }
    Some(format!("the probe was killed: {rest}"))
}

#[cfg(test)]
mod machine_tests {
    use super::without_the_machine;

    /// Every shape the committed record actually carried, counted out
    /// of `tests/interop-results/` before any of it was rewritten.
    /// Written as cases and not as a rule, so that a rule that stops
    /// covering one of them is a failure and not a quieter file.
    #[test]
    fn the_four_shapes_the_record_carried() {
        for (raw, expected) in [
            (
                "/root/zerodds/tests/interop-corpus/recursive-array/type.idl: the layout could \
                 not be planned",
                "tests/interop-corpus/recursive-array/type.idl: the layout could not be planned",
            ),
            (
                "/root/zd-orbros/tests/interop-corpus/resolve-module-named-as-type//type.idl: \
                 error [IDL0008] at 19..40",
                "tests/interop-corpus/resolve-module-named-as-type/type.idl: error [IDL0008] at \
                 19..40",
            ),
            (
                "does not compile: /root/zerodds/target/wireprobe-c-cache/\
                 build-929565-e14fc33c4cc125c13746d493e67e6188-full/gen.h:48:12: error: field \
                 'next' has incomplete type",
                "does not compile: gen.h:48:12: error: field 'next' has incomplete type",
            ),
            (
                "/tmp/tmp.kCQh0o2dYt/gen.idl:2:1: syntax error",
                "gen.idl:2:1: syntax error",
            ),
            (
                "/tmp/tmp.y5n477KY11/chunk.000.d/build/gen.idl:14:3: error",
                "chunk.000.d/build/gen.idl:14:3: error",
            ),
            (
                "/root/zd-wire/tools/wire-capture/wire-golden.sh: line 585: 2588703 Aborted  \
                 ( cd \"$work\" && omniidl -bcxx gen.idl ) gen.idl:1: Syntax error in union \
                 declaration",
                "the probe was killed: Aborted  ( cd \"$work\" && omniidl -bcxx gen.idl ) \
                 gen.idl:1: Syntax error in union declaration",
            ),
        ] {
            assert_eq!(without_the_machine(raw), expected, "over `{raw}`");
        }
    }

    /// `TMPDIR` is not `/tmp` on every host, and the directory is
    /// recognised by `mktemp`'s template rather than by where it sits.
    #[test]
    fn a_temporary_directory_outside_slash_tmp() {
        assert_eq!(
            without_the_machine(
                "/var/folders/x7/ynrq6821137f62xmkh1wqqym0000gn/T/tmp.i4YfZRnLcU/gen.idl:2:1: \
                 syntax error"
            ),
            "gen.idl:2:1: syntax error"
        );
    }

    /// A vendor's install path is a measurement of the toolchain and is
    /// not the machine's in the sense this removes. It stays whole.
    #[test]
    fn a_vendors_own_path_is_left_alone() {
        for kept in [
            "/opt/opendds/bin/tao_idl: Error - fixed data types are not supported",
            "In file included from /opt/fastdds/include/fastcdr/Cdr.h:22",
            "gen.hpp: error: expected '}' before 'auto'",
            "tests/interop-corpus/recursive-array/type.idl: already relative",
        ] {
            assert_eq!(without_the_machine(kept), kept);
        }
    }

    /// Running it twice changes nothing the first run did not, which is
    /// what lets `regenerate.sh` run it on every regeneration.
    #[test]
    fn it_is_idempotent() {
        let raw = "/root/zerodds/tests/interop-corpus/x/type.idl and /tmp/tmp.abcdefghij/gen.idl";
        let once = without_the_machine(raw);
        assert_eq!(without_the_machine(&once), once);
    }

    /// Line structure is the record's shape and is not touched: a
    /// six-column TSV comes out a six-column TSV.
    #[test]
    fn the_lines_and_the_tabs_survive() {
        let raw = "a\tb\t/root/zerodds/tests/interop-corpus/x/type.idl: no\nc\td\te\n";
        let out = without_the_machine(raw);
        assert_eq!(out.lines().count(), 2);
        assert_eq!(out.matches('\t').count(), 4);
        assert!(out.ends_with('\n'));
    }
}
