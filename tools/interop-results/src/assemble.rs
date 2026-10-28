// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Building the ledger: every case, every encoding, every byte order,
//! every system, both directions.
//!
//! One place decides what a cell means. Every rule about what a system
//! could not be asked — an ORB has no encoding version 2, TAO writes
//! one byte order, a vendor's `--encoding` selects a version and not a
//! framing — is applied here and nowhere else, so a table and a tree
//! cannot disagree about a hole.

use std::collections::BTreeMap;
use std::path::Path;

use crate::ledger::{Entry, Ledger, Outcome};
use crate::rust_column::{self, CaseSweep};
use crate::sources::{self, Answer, ColumnMatrix, Measured};
use crate::systems::{Kind, SYSTEMS};
use crate::{Case, canonical_encoding, encoding_version};

/// Everything the ledger is built from.
pub struct Inputs {
    /// `vendor-corpus.tsv` plus `ros-corpus.tsv`, in the six-column
    /// shape, keyed by version.
    pub foreign: Measured,
    /// `corba-corpus.tsv`, keyed by version `xcdr1`.
    pub orbs: Measured,
    /// The C column, keyed by encoding.
    pub c: ColumnMatrix,
    /// The Python column, keyed by encoding.
    pub python: ColumnMatrix,
    /// The Java column, write direction only.
    pub java: ColumnMatrix,
    /// The C++ column, write direction only.
    pub cpp: ColumnMatrix,
    /// The C# column, both directions.
    pub csharp: ColumnMatrix,
    /// The TypeScript column, write direction only.
    pub typescript: ColumnMatrix,
    /// `read-corpus.tsv`: what each foreign system got back when it
    /// read the payload it wrote, keyed like a write row.
    pub foreign_reads: BTreeMap<String, (String, String)>,
    /// `warnings.tsv` and `toolchain-warnings.tsv`: every line every
    /// front end printed and every line every compiler printed, keyed
    /// by `<case>/<system>`. Two files because two sweeps write them
    /// and each writes its own whole; one map because a compiler is one
    /// more `program`.
    pub diagnostics: crate::warnings::Diagnostics,
}

impl Inputs {
    /// Read every source file under `sources/`.
    ///
    /// A source that is not there is not an error: it is a column
    /// nobody has run here, and every one of its cells says so by name.
    ///
    /// # Errors
    ///
    /// When a source file is there and malformed.
    pub fn read(sources: &Path) -> Result<Self, String> {
        let mut foreign = sources::read_six_column(&sources.join("vendor-corpus.tsv"))?;
        foreign.extend(sources::read_six_column(&sources.join("ros-corpus.tsv"))?);
        Ok(Self {
            foreign,
            orbs: sources::read_corba(&sources.join("corba-corpus.tsv"))?,
            c: read_own(&sources.join("zerodds-c.tsv"))?,
            python: read_own(&sources.join("zerodds-python.tsv"))?,
            java: read_own(&sources.join("zerodds-java.tsv"))?,
            cpp: read_own(&sources.join("zerodds-cpp.tsv"))?,
            csharp: read_own(&sources.join("zerodds-csharp.tsv"))?,
            typescript: read_own(&sources.join("zerodds-typescript.tsv"))?,
            foreign_reads: sources::read_verdicts(&sources.join("read-corpus.tsv"))?,
            diagnostics: crate::warnings::read_both(
                &sources.join("warnings.tsv"),
                &sources.join("toolchain-warnings.tsv"),
            )?,
        })
    }
}

/// One of the two foreign ZeroDDS columns, from its two committed TSVs.
fn read_own(path: &Path) -> Result<ColumnMatrix, String> {
    let mut matrix = ColumnMatrix::default();
    let read_path = path.with_file_name(format!(
        "{}-read.tsv",
        path.file_stem().unwrap_or_default().to_string_lossy()
    ));
    for (key, (verdict, detail)) in sources::read_verdicts(&read_path)? {
        // `<case>/<system>/<encoding>/<order>`; the system is in the
        // filename and is not needed twice.
        let mut parts = key.rsplitn(4, '/');
        let (Some(order), Some(encoding), Some(_system), Some(case)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        // The token as well as the detail. `ColumnMatrix` carries a
        // string per cell and `own_cell` parses it back, so a verdict
        // that arrived as `harness-gap` and was written as `!<detail>`
        // came out of that round trip as `refused` — the two words
        // `docs/specs/zerodds-verdicts-1.0.md` exists to keep apart.
        // `unwritten` keeps its bare `!unwritten` shape, which is what
        // `own_cell` already looks for.
        let cell = match verdict.as_str() {
            "same" => "same".to_string(),
            "different" => format!("different: {detail}"),
            "unwritten" => format!("!unwritten {detail}"),
            other => format!("!{other}: {detail}"),
        };
        matrix
            .read
            .insert(format!("{case}/{encoding}/{order}"), cell);
    }
    let measured = sources::read_six_column(path)?;
    for (key, answer) in measured {
        // `<case>/<system>/<encoding>/<order>`; the system is in the
        // filename and is not needed twice.
        let mut parts = key.rsplitn(4, '/');
        let (Some(order), Some(encoding), Some(_system), Some(case)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let slot = format!("{case}/{encoding}/{order}");
        match answer {
            Answer::Bytes(hex) => matrix.write.insert(slot, hex),
            Answer::Refused(message)
            | Answer::NoForm(message)
            | Answer::MappingDivergence(message)
            | Answer::HarnessGap(message) => matrix.write.insert(slot, format!("!{message}")),
        };
    }
    Ok(matrix)
}

/// The ledger, and the configuration sweep the mitigation column needs.
pub struct Built {
    /// Every cell of both tables.
    pub ledger: Ledger,
    /// One sweep per case, kept so that the tables can ask which
    /// configuration reproduces a foreign byte string.
    pub sweeps: BTreeMap<String, CaseSweep>,
}

/// Why a peer's own reader did not return the sample from its own
/// octets, keyed like a read row.
///
/// The read sweeps answer `different` and stop there; that is the right
/// answer to what they measure — the peer read its own payload and
/// handed back something else — and it is not a cause. Sixty-five cells
/// carried it with nothing attached, and a difference nobody has
/// attributed is indistinguishable from one nobody has looked at.
///
/// Two causes, told apart by measurement rather than by a list of case
/// names, and both computed in [`attribute_reads`]:
///
/// - **The frame carries nothing to read.** A dialect that suppresses a
///   negative narrow case label writes the discriminator and stops, so
///   the member the sample names is not on the wire and no reader could
///   hand it back — [`dialect_fit::ReadVerdict::NothingToRead`], which
///   is reached from the plan **and** from this project's own round trip
///   failing the same way. That is a `mapping-divergence`:
///   `docs/specs/zerodds-verdicts-1.0.md`.
/// - **The frame carries it and the peer's own reader lost it.** This
///   project's reader, under that peer's dialect, returns the sample
///   from the *same recorded octets*. The divergence is then in the
///   peer's reader and not in its wire form, and the cell stays
///   `different` with that said in it.
type ReadCauses = BTreeMap<String, Outcome>;

/// The sentence `attribute_reads` adds to a cell whose octets carry
/// the sample and whose peer did not return it.
///
/// A constant because a test reads it:
/// `tests/own_read_completeness.rs` requires every `different` and every
/// `refused` read cell of a foreign column to carry an attribution, and
/// a phrase written twice is a phrase that comes apart.
pub const READER_LOST_IT: &str = "the octets carry the sample: they read back as it under";

/// Work out the cause of every `different` read cell.
///
/// Built only from the cells that are `different`, and only for the
/// systems that have one: `Sweep::new` lowers every case twice, which is
/// most of a second, and `--verify` runs in every CI job. A record with
/// no disagreement in it costs nothing here.
fn attribute_reads(cases: &[Case], inputs: &Inputs) -> ReadCauses {
    use crate::dialect_fit::{self, ReadVerdict};

    // `<case>/<system>/<version>/<endian>` is the read record's key.
    //
    // `different` and `refused`, and both for the same reason: each is a
    // peer that had its own octets in front of it and did not return the
    // sample. `generator-defect` and `harness-gap` are not here —
    // nothing ran, so there is nothing about the frame to establish.
    let mut wanted: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for (key, (verdict, _)) in &inputs.foreign_reads {
        if verdict != "different" && verdict != "refused" {
            continue;
        }
        let mut parts = key.splitn(4, '/');
        let (Some(_case), Some(system)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Some(found) = SYSTEMS.iter().find(|candidate| candidate.id == system) else {
            continue;
        };
        wanted.entry(found.id).or_default().push(key);
    }
    let mut causes = ReadCauses::new();
    if wanted.is_empty() {
        return causes;
    }
    let sweep = dialect_fit::Sweep::new(cases);
    for (system_id, keys) in wanted {
        let Some(system) = SYSTEMS.iter().find(|candidate| candidate.id == system_id) else {
            continue;
        };
        let Some(dialect) = dialect_fit::dialect_of(system) else {
            continue;
        };
        // Every row this system recorded, cut down to the ones in
        // question: the sweep costs per row and there are 65 of them
        // against 7 782.
        let all = dialect_fit::rows(system, cases, &inputs.foreign, &inputs.orbs);
        let keyed: Vec<(String, dialect_fit::Row)> = all
            .into_iter()
            .filter_map(|row| {
                let case = cases.get(row.case)?;
                let key = format!(
                    "{}/{}/{}/{}",
                    case.name,
                    system.id,
                    crate::encoding_version(row.encoding_name),
                    row.endian_name
                );
                keys.contains(&&key).then_some((key, row))
            })
            .collect();
        let rows: Vec<dialect_fit::Row> = keyed.iter().map(|(_, row)| row.clone()).collect();
        let verdicts = sweep.read(cases, &rows, &|encoding| {
            dialect_fit::Assignment::of(dialect, encoding)
        });
        for ((key, _), verdict) in keyed.iter().zip(verdicts) {
            let cause = match verdict {
                // The one cause that is a property of the frame.
                ReadVerdict::NothingToRead(why) => Outcome::MappingDivergence(why),
                // The octets carry the sample; this project's reader
                // returns it from them under the peer's own dialect. So
                // the peer's writer and its reader disagree with each
                // other, which is a finding about the peer and not about
                // the wire form.
                ReadVerdict::Same => Outcome::Different(format!(
                    "{READER_LOST_IT} `--dialect {}`, so this peer's writer and its own \
                     reader disagree",
                    dialect.as_str()
                )),
                // Neither established. Said as such rather than left
                // blank: an unattributed difference that claims an
                // attribution is worse than one that does not.
                ReadVerdict::Different(_) | ReadVerdict::Refused(_) | ReadVerdict::NoSample(_) => {
                    continue;
                }
            };
            causes.insert(key.clone(), cause);
        }
    }
    causes
}

/// Build the whole ledger.
///
/// # Errors
///
/// When a case's IDL does not lower, which is a corpus defect.
pub fn build(cases: &[Case], inputs: &Inputs) -> Result<Built, String> {
    let mut ledger = Ledger::default();
    let mut sweeps = BTreeMap::new();
    let causes = attribute_reads(cases, inputs);
    for case in cases {
        let sweep = rust_column::sweep(case)?;
        // Once per (case, system), because the front end runs once per
        // case whatever the encoding and the byte order turn out to be.
        let mut warned: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for system in &SYSTEMS {
            let lines: Vec<String> =
                crate::warnings::warnings_of(&inputs.diagnostics, &case.name, system.id)
                    .into_iter()
                    .map(|line| format!("{}\t{}", line.program, line.text))
                    .collect();
            if !lines.is_empty() {
                ledger
                    .warnings
                    .insert(crate::warnings::key(&case.name, system.id), lines.clone());
                warned.insert(system.id, lines);
            }
        }
        for encoding in crate::ENCODINGS {
            for endian in crate::ORDERS {
                for system in &SYSTEMS {
                    let (write, read) =
                        cell(case, encoding, endian, system, &sweep, inputs, &causes);
                    ledger.entries.push(Entry {
                        case: case.name.clone(),
                        encoding,
                        endian,
                        system: system.id,
                        write,
                        read,
                        warnings: warned.get(system.id).map_or(0, Vec::len),
                    });
                }
            }
        }
        sweeps.insert(case.name.clone(), sweep);
    }
    Ok(Built { ledger, sweeps })
}

fn cell(
    case: &Case,
    encoding: &'static str,
    endian: &'static str,
    system: &crate::systems::System,
    sweep: &CaseSweep,
    inputs: &Inputs,
    causes: &ReadCauses,
) -> (Outcome, Outcome) {
    match system.kind {
        Kind::Own => own_cell(case, encoding, endian, system.id, sweep, inputs),
        Kind::Dds | Kind::Orb | Kind::Ros => {
            foreign_cell(case, encoding, endian, system, inputs, causes)
        }
    }
}

/// A ZeroDDS column.
///
/// Rust is this process. C and Python are their committed source TSVs,
/// which the three-column gate's run produced; where a source is
/// absent the cell says the column was not run here rather than
/// borrowing the Rust column's answer, which would make three columns
/// out of one measurement.
fn own_cell(
    case: &Case,
    encoding: &str,
    endian: &str,
    system_id: &str,
    sweep: &CaseSweep,
    inputs: &Inputs,
) -> (Outcome, Outcome) {
    let slot = format!("{encoding}/{endian}");
    if system_id == "zerodds-rust" {
        return sweep
            .get(rust_column::DEFAULT)
            .and_then(|slots| slots.get(&slot))
            .cloned()
            .unwrap_or((
                Outcome::NotMeasured("the Rust sweep produced no slot".to_string()),
                Outcome::Unwritten,
            ));
    }
    let (matrix, producer) = match system_id {
        "zerodds-c" => (&inputs.c, "tools/wireprobe/three-column-gate --full"),
        "zerodds-java" => (&inputs.java, "tools/interop-results/java-record.sh"),
        "zerodds-cpp" => (&inputs.cpp, "tools/interop-results/cpp-record.sh"),
        "zerodds-csharp" => (&inputs.csharp, "tools/interop-results/csharp-record.sh"),
        "zerodds-typescript" => (
            &inputs.typescript,
            "tools/interop-results/typescript-record.sh",
        ),
        // Python is the fall-through, which is why every own column
        // above has to be named: a column registered in `systems.rs`
        // with no arm here is served the **Python** column's octets, in
        // a generated table nobody re-derives.
        _ => (&inputs.python, "tools/wireprobe/three-column-gate --full"),
    };
    let key = format!("{}/{encoding}/{endian}", case.name);
    let Some(cell) = matrix.write.get(&key) else {
        return (
            Outcome::NotMeasured(producer.to_string()),
            Outcome::NotMeasured(producer.to_string()),
        );
    };
    let write = cell.strip_prefix('!').map_or_else(
        || Outcome::Bytes(cell.clone()),
        |message| Outcome::Refused(message.to_string()),
    );
    let read = match matrix.read.get(&key).map(String::as_str) {
        None => Outcome::NotMeasured(producer.to_string()),
        Some("same") => Outcome::Same,
        Some(cell) if cell.starts_with("different: ") => {
            Outcome::Different(cell["different: ".len()..].to_string())
        }
        Some(cell) => match cell.strip_prefix('!') {
            Some(message) if message.starts_with("unwritten") => Outcome::Unwritten,
            // `<token>: <detail>`, through `Outcome::parse` so that a
            // `harness-gap` stays one. A message with no token in front
            // of it is one of the older shapes and is a refusal, which
            // is what this arm answered for everything.
            Some(message) => message
                .split_once(": ")
                .and_then(|(token, detail)| Outcome::parse(token, detail))
                .unwrap_or_else(|| Outcome::Refused(message.to_string())),
            None => Outcome::Different(cell.to_string()),
        },
    };
    (write, read)
}

/// A foreign column.
///
/// Four holes are structural and are decided here rather than being
/// discovered as missing rows: no encoding version 2, one byte order,
/// no form for the construct, and the five encodings a vendor answers
/// with two runs.
fn foreign_cell(
    case: &Case,
    encoding: &'static str,
    endian: &'static str,
    system: &crate::systems::System,
    inputs: &Inputs,
    causes: &ReadCauses,
) -> (Outcome, Outcome) {
    let version = encoding_version(encoding);
    let measured = if system.kind == Kind::Orb {
        &inputs.orbs
    } else {
        &inputs.foreign
    };

    // A case the front end never saw is said before anything else: it
    // is not a byte order limit and not an encoding limit, and saying
    // one of those instead would name the wrong reason.
    let no_form = crate::ORDERS.iter().find_map(|order| {
        match sources::get(measured, &case.name, system.id, "xcdr1", order) {
            Some(
                Answer::NoForm(detail)
                | Answer::MappingDivergence(detail)
                | Answer::HarnessGap(detail),
            ) => Some(detail.clone()),
            _ => None,
        }
    });
    if let Some(detail) = no_form {
        return (Outcome::NoForm(detail.clone()), Outcome::NoForm(detail));
    }

    if version == "xcdr2" && !system.has_version_2 {
        return (Outcome::NoVersion2, Outcome::NoVersion2);
    }
    if !system.writes_orders.contains(&endian) {
        return (Outcome::OneOrderOnly, Outcome::OneOrderOnly);
    }
    let read_key = format!("{}/{}/{version}/{endian}", case.name, system.id);
    let read = match inputs.foreign_reads.get(&read_key) {
        // Through `Outcome::parse`, so a token this tree does not know
        // is loud. The arm here used to be `_ => Outcome::Unwritten`,
        // which turned an unrecognised verdict into "the write
        // direction produced nothing" — a sentence about a different
        // direction, printed with no sign that anything was lost.
        // Through `Outcome::parse`, and an unrecognised token becomes
        // a cell that says so rather than a cell that says something
        // else. The arm here used to be `_ => Outcome::Unwritten`,
        // which turned an unknown verdict into "the write direction
        // produced nothing" — a sentence about a direction that was
        // never run, printed with no sign that anything was lost.
        Some((verdict, detail)) => Outcome::parse(verdict, detail).unwrap_or_else(|| {
            Outcome::NotMeasured(format!(
                "`{verdict}` is not a verdict this tree knows; \
                 docs/specs/zerodds-verdicts-1.0.md has the vocabulary"
            ))
        }),
        None => Outcome::NotMeasured(format!(
            "tools/interop-results/read-corpus-sweep.sh over {}",
            system.producer
        )),
    };
    // A `different` or a `refused` with its cause worked out.
    // `attribute_reads` is the one place that decides which of the two
    // it is, and it decides from the plan and from this project's own
    // reader over the peer's own recorded octets — never from a list of
    // case names. The peer's own sentence is kept and the cause is added
    // to it: a verdict names its evidence, and the evidence here is two
    // measurements and not one.
    let read = match (&read, causes.get(&read_key)) {
        (Outcome::Different(measured), Some(Outcome::Different(cause))) => {
            Outcome::Different(format!("{measured} — {cause}"))
        }
        (Outcome::Refused(said), Some(Outcome::Different(cause))) => {
            Outcome::Refused(format!("{said} — {cause}"))
        }
        (Outcome::Different(_) | Outcome::Refused(_), Some(cause)) => cause.clone(),
        _ => read,
    };
    let measurement = match sources::get(measured, &case.name, system.id, version, endian) {
        Some(Answer::Bytes(hex)) => (Outcome::Bytes(hex.clone()), read),
        Some(Answer::Refused(message)) => (Outcome::Refused(message.clone()), Outcome::Unwritten),
        Some(Answer::MappingDivergence(detail)) => (
            Outcome::MappingDivergence(detail.clone()),
            Outcome::MappingDivergence(detail.clone()),
        ),
        Some(Answer::HarnessGap(detail)) => (
            Outcome::HarnessGap(detail.clone()),
            Outcome::HarnessGap(detail.clone()),
        ),
        Some(Answer::NoForm(detail)) => (
            Outcome::NoForm(detail.clone()),
            Outcome::NoForm(detail.clone()),
        ),
        // `opendds` had a `NoProbe` arm here. It was true when it was
        // written and false since `wireprobe-opendds` landed: the
        // toolchain is driven by a probe in this tree, so a missing row
        // is a run nobody has made, and the cell names the command that
        // would make one.
        None => (
            Outcome::NotMeasured(system.producer.to_string()),
            Outcome::NotMeasured(system.producer.to_string()),
        ),
    };

    // The framing follows the type's extensibility and the vendor has
    // no flag for it, so four of the five encodings point at the run
    // the canonical one holds rather than at four identical runs.
    //
    // Only when that run produced bytes. A cell that said `= plain-cdr1`
    // and linked to a file no run wrote would be a link into nothing,
    // and a reader following it would take a 404 for a measurement;
    // where the canonical run refused or was never made, this row says
    // exactly what that row says.
    let canonical = canonical_encoding(encoding);
    if encoding != canonical {
        return match measurement.0 {
            Outcome::Bytes(_) => (
                Outcome::VersionAlias(canonical),
                Outcome::VersionAlias(canonical),
            ),
            other => (other, measurement.1),
        };
    }
    measurement
}
