// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `interop-results` — the result tree and the two tables.
//!
//! ```text
//! interop-results --regenerate    write the tree, the ledger and the tables
//! interop-results --verify        rebuild and compare; the CI job's command
//! interop-results --tally         one line per system, what it reached
//! interop-results --dialects      derive each system's dialect from its octets
//! ```
//!
//! `--verify` is what proves the claim: the bytes committed are what a
//! run produces. It rebuilds every column it can reach, compares the
//! result against the committed tree, and **names every column it could
//! not reach with the number of rows that leaves unverified**. A column
//! it cannot reach is never a failure and never a silent pass.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

use std::path::Path;
use std::time::Instant;

use zerodds_interop_results as results;
use zerodds_interop_results::assemble::{self, Inputs};
use zerodds_interop_results::ledger::{self, Outcome, RowIds};
use zerodds_interop_results::page;
use zerodds_interop_results::systems::SYSTEMS;
use zerodds_interop_results::tables::{self, Direction};
use zerodds_interop_results::tree;

const USAGE: &str = "\
interop-results — the interop result tree, the two tables and the page

    --regenerate    write tests/interop-results/ from the sources and a
                    fresh run of this project's own columns
    --verify        rebuild and compare against what is committed; the
                    command both CI jobs run
    --tally         one line per system: how much of the matrix it
                    reached, and what the rest of its rows say instead
    --page          write tests/interop-results/matrix.html and nothing
                    else, which is the whole tree's answers without the
                    minutes a full regeneration takes
    --normalise     take the machine out of the recorded files: the
                    checkout a sweep ran in, the `mktemp -d` it worked
                    in and the build directory named after a process id.
                    Run before every regeneration, and on its own after
                    a sweep that was run by hand
    --dialects      derive each measured system's dialect from its own
                    recorded octets: per axis, the value that reproduces
                    the rows and whether any row pins it, plus every row
                    the assignment misses. The gate over the same
                    derivation is
                    tools/interop-results/tests/dialect_completeness.rs
    --sources <dir> where the recorded foreign runs are
                    (default: tests/interop-results/sources)
    --base-url <u>  prefix every link on the page with this, for the day
                    the result tree is published; without it the links
                    are relative to the directory the page sits in
";

fn main() {
    let mut mode = None;
    let mut sources = None;
    let mut base = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--regenerate" | "--verify" | "--tally" | "--import-columns" | "--page"
            | "--dialects" | "--normalise" => {
                mode = Some(argument);
            }
            "--sources" => sources = arguments.next(),
            "--base-url" => base = arguments.next(),
            "--help" | "-h" => {
                print!("{USAGE}");
                return;
            }
            other => {
                eprintln!("interop-results: `{other}` is not an option\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }
    let Some(mode) = mode else {
        eprintln!(
            "interop-results: one of --regenerate, --verify, --tally, --page, --normalise or \
             --dialects\n\n{USAGE}"
        );
        std::process::exit(2);
    };
    if let Err(message) = run(&mode, sources.as_deref(), base.as_deref().unwrap_or("")) {
        eprintln!("interop-results: {message}");
        std::process::exit(1);
    }
}

fn run(mode: &str, sources: Option<&str>, base: &str) -> Result<(), String> {
    let started = Instant::now();
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let sources_directory =
        sources.map_or_else(|| results_root.join("sources"), |given| root.join(given));

    if mode == "--import-columns" {
        return import_columns(&root, &sources_directory);
    }

    if mode == "--normalise" {
        return normalise_record(&results_root);
    }

    if mode == "--dialects" {
        let cases = results::corpus(&root)?;
        let inputs = Inputs::read(&sources_directory)?;
        print!("{}", derive_dialects(&cases, &inputs));
        return Ok(());
    }

    let cases = results::corpus(&root)?;
    let inputs = Inputs::read(&sources_directory)?;
    let built = assemble::build(&cases, &inputs)?;
    let elapsed = started.elapsed();

    match mode {
        "--tally" => {
            print!("{}", tree::tally(&built.ledger));
            println!(
                "\n{} cases x {} encodings x {} orders = {} rows; {} systems; {} ledger lines",
                cases.len(),
                results::ENCODINGS.len(),
                results::ORDERS.len(),
                cases.len() * results::ENCODINGS.len() * results::ORDERS.len(),
                SYSTEMS.len(),
                built.ledger.entries.len()
            );
            report_unreachable(&built.ledger);
            Ok(())
        }
        "--page" => {
            let mut ids = read_ids(&results_root)?;
            let before = ids.by_key.len();
            let documents = page::documents(&root, &cases)?;
            let html = page::render(
                &built.ledger,
                &cases,
                &built.sweeps,
                &mut ids,
                &documents,
                base,
            )?;
            let path = results_root.join(page::FILE);
            std::fs::write(&path, &html).map_err(|error| format!("{}: {error}", path.display()))?;
            print!(
                "{}",
                page::summary(&built.ledger, &cases, &built.sweeps, &mut RowIds::default())
            );
            println!(
                "{} — {} characters, {} rows, {} systems, in {:.1}s",
                path.display(),
                html.len(),
                built.ledger.entries.len() / SYSTEMS.len(),
                SYSTEMS.len(),
                elapsed.as_secs_f64()
            );
            // A row the page numbers that `row-ids.tsv` does not hold
            // would be a number that moves the next time anything is
            // regenerated, and a citation of it would rot silently.
            if ids.by_key.len() != before {
                return Err(format!(
                    "{} rows have no number in row-ids.tsv; run --regenerate first",
                    ids.by_key.len() - before
                ));
            }
            Ok(())
        }
        "--regenerate" => {
            let written = tree::write(&results_root, &built.ledger)?;
            std::fs::write(
                results_root.join("ledger.tsv"),
                ledger::render(&built.ledger),
            )
            .map_err(|error| format!("ledger.tsv: {error}"))?;

            let mut ids = read_ids(&results_root)?;
            for direction in [Direction::Write, Direction::Read] {
                let table =
                    tables::render(direction, &built.ledger, &cases, &built.sweeps, &mut ids);
                std::fs::write(results_root.join(tables::file(direction)), table)
                    .map_err(|error| format!("{}: {error}", tables::file(direction)))?;
            }
            let documents = page::documents(&root, &cases)?;
            let html = page::render(
                &built.ledger,
                &cases,
                &built.sweeps,
                &mut ids,
                &documents,
                base,
            )?;
            std::fs::write(results_root.join(page::FILE), html)
                .map_err(|error| format!("{}: {error}", page::FILE))?;
            std::fs::write(results_root.join("row-ids.tsv"), ids.render())
                .map_err(|error| format!("row-ids.tsv: {error}"))?;
            std::fs::write(
                results_root.join("SYSTEMS.md"),
                tables::systems_document(&built.ledger),
            )
            .map_err(|error| format!("SYSTEMS.md: {error}"))?;
            std::fs::write(
                results_root.join("WARNINGS.md"),
                zerodds_interop_results::warnings::document(&inputs.diagnostics, &SYSTEMS),
            )
            .map_err(|error| format!("WARNINGS.md: {error}"))?;
            std::fs::write(
                results_root.join("REFUSALS.md"),
                tables::refusals_document(&built.ledger, &results_root),
            )
            .map_err(|error| format!("REFUSALS.md: {error}"))?;

            println!(
                "{} files, {} bytes of content, {} stale removed; {} ledger lines in {:.1}s",
                written.files,
                written.bytes,
                written.stale.len(),
                built.ledger.entries.len(),
                elapsed.as_secs_f64()
            );
            report_unreachable(&built.ledger);
            Ok(())
        }
        "--verify" => {
            let differences = tree::verify(&results_root, &built.ledger)?;
            let mut ids = read_ids(&results_root)?;
            let before = ids.by_key.len();
            let mut stale = Vec::new();
            for direction in [Direction::Write, Direction::Read] {
                let fresh =
                    tables::render(direction, &built.ledger, &cases, &built.sweeps, &mut ids);
                let path = results_root.join(tables::file(direction));
                let committed = std::fs::read_to_string(&path).unwrap_or_default();
                if committed != fresh {
                    stale.push(tables::file(direction).to_string());
                }
            }
            let committed_ledger =
                std::fs::read_to_string(results_root.join("ledger.tsv")).unwrap_or_default();
            if committed_ledger != ledger::render(&built.ledger) {
                stale.push("ledger.tsv".to_string());
            }
            let documents = page::documents(&root, &cases)?;
            for (name, fresh) in [
                ("SYSTEMS.md", tables::systems_document(&built.ledger)),
                (
                    "WARNINGS.md",
                    zerodds_interop_results::warnings::document(&inputs.diagnostics, &SYSTEMS),
                ),
                (
                    "REFUSALS.md",
                    tables::refusals_document(&built.ledger, &results_root),
                ),
                (
                    page::FILE,
                    page::render(
                        &built.ledger,
                        &cases,
                        &built.sweeps,
                        &mut ids,
                        &documents,
                        base,
                    )?,
                ),
            ] {
                let committed =
                    std::fs::read_to_string(results_root.join(name)).unwrap_or_default();
                if committed != fresh {
                    stale.push(name.to_string());
                }
            }
            if ids.by_key.len() != before {
                stale.push(format!(
                    "row-ids.tsv ({} rows have no number)",
                    ids.by_key.len() - before
                ));
            }

            // An octet no re-run reproduces is invisible to every
            // comparison above: they all hold the tree against
            // `sources/`, and `sources/` is what the bad octet is in.
            // This is the one check on the record itself.
            let twins = zerodds_interop_results::reproducible::twin_disagreements(
                &cases,
                &[&inputs.foreign, &inputs.orbs],
            );
            let mut unreproducible = Vec::new();
            for twin in &twins {
                if twin.reaches_no_reader {
                    continue;
                }
                unreproducible.push(twin.line());
            }

            report_unreachable(&built.ledger);
            println!("verified in {:.1}s", elapsed.as_secs_f64());
            if !unreproducible.is_empty() {
                for line in &unreproducible {
                    println!("  {line}");
                }
                return Err(format!(
                    "{} recorded octet(s) are not a function of the sample",
                    unreproducible.len()
                ));
            }
            if differences.is_empty() && stale.is_empty() {
                println!("every regenerated cell is the committed one");
                return Ok(());
            }
            for line in differences.iter().take(40) {
                println!("  {line}");
            }
            if differences.len() > 40 {
                println!("  ... and {} more", differences.len() - 40);
            }
            for name in &stale {
                println!("  {name} is not what a fresh run writes");
            }
            Err(format!(
                "{} cell(s) and {} document(s) differ from the committed tree",
                differences.len(),
                stale.len()
            ))
        }
        other => Err(format!("`{other}` is not a mode")),
    }
}

/// Distil the C and Python columns out of what the three-column gate's
/// run left in `target/`.
///
/// The gate writes nine million characters of twelve configurations. What
/// is committed is the one configuration the tree holds, in the same
/// six-column shape the foreign sweeps write, so that one reader takes
/// every source in `sources/`.
///
/// A JSON that is not there is not an error: it is a column the gate
/// has not run in this checkout, and every one of its cells says so.
fn import_columns(root: &Path, sources_directory: &Path) -> Result<(), String> {
    std::fs::create_dir_all(sources_directory)
        .map_err(|error| format!("{}: {error}", sources_directory.display()))?;
    let cases = results::corpus(root)?;
    for (system, json) in [
        ("zerodds-c", "three-column-c-full.json"),
        ("zerodds-python", "three-column-python.json"),
    ] {
        let path = root.join("target").join(json);
        let mut matrix = zerodds_interop_results::sources::read_column_matrix(&path)?;
        // The Python column answers a read slot with the document its
        // decoder built and no verdict. The verdict is reached here,
        // over the case's own sample, by the same comparison the
        // three-column gate makes — never by treating the document as a
        // disagreement because it is not the word `same`.
        zerodds_interop_results::sources::resolve_read_documents(&cases, &mut matrix);
        if matrix.write.is_empty() {
            println!(
                "{system}: NOT MEASURED HERE — {} holds nothing; run \
                 `tools/wireprobe/three-column-gate --full`",
                path.display()
            );
            continue;
        }
        // A JSON older than the corpus covers fewer cases than the
        // corpus has, and every case it misses becomes `not measured`
        // in the tree — the right word, reached from a stale file
        // rather than from a run. Six new cases arrived this way and
        // nothing said so, which is why the shortfall is named here and
        // is what `record.sh` re-runs the gate on.
        let covered: std::collections::BTreeSet<&str> = matrix
            .write
            .keys()
            .filter_map(|key| key.split('/').next())
            .collect();
        let missing: Vec<&str> = cases
            .iter()
            .map(|case| case.name.as_str())
            .filter(|name| !covered.contains(name))
            .collect();
        if !missing.is_empty() {
            println!(
                "{system}: STALE — {} covers {} of the corpus's {} cases; {} has no run in it: \
                 {}. Run `tools/wireprobe/three-column-gate --full`",
                path.display(),
                covered.len(),
                cases.len(),
                missing.len(),
                missing.join(", ")
            );
        }
        let (write, read) =
            zerodds_interop_results::sources::render_column_source(system, &matrix)?;
        // A probe's diagnostic names the file it was given, and the gate
        // gives it an absolute path — the machine's, not the record's.
        // Stripped here, at the one funnel every gate detail passes,
        // because `the_page_names_no_private_infrastructure` is right:
        // the substance of a refusal is its message, not where it ran.
        //
        // Two steps and not one. This checkout's own root is known here
        // and goes whole; what is left is every other absolute path the
        // gate recorded, and those are the machine's under a name this
        // process cannot know — the build directory `wireprobe-c-matrix`
        // names after its own process id is one, and five of them were
        // committed. `sources::without_the_machine` is the rule, and it
        // is the same one `--normalise` writes over the record.
        let prefix = format!("{}/", root.display());
        let (write, read) = (write.replace(&prefix, ""), read.replace(&prefix, ""));
        let (write, read) = (
            zerodds_interop_results::sources::without_the_machine(&write),
            zerodds_interop_results::sources::without_the_machine(&read),
        );
        for (suffix, text) in [("", &write), ("-read", &read)] {
            let out = sources_directory.join(format!("{system}{suffix}.tsv"));
            std::fs::write(&out, text).map_err(|error| format!("{}: {error}", out.display()))?;
            println!(
                "{system}{suffix}: {} rows into {}",
                text.lines().count(),
                out.display()
            );
        }
    }
    Ok(())
}

/// Every recorded file, with the machine taken out of it.
///
/// The recorded files are the ones a sweep writes: the sources, the
/// logs archived beside them and `wire-goldens.tsv`. Everything else in
/// the tree is rendered from those by `--regenerate`, so a path in a
/// rendered file is a path in a recorded one and is fixed by fixing
/// that one.
///
/// `PROVENANCE.tsv` is not among them on purpose. Its fourth column is
/// the directory a toolchain is installed in — that is the file's
/// content and not the way to it, and a scrubbed provenance record
/// answers nothing.
///
/// # Errors
///
/// When a recorded file cannot be read or written back.
fn normalise_record(results_root: &Path) -> Result<(), String> {
    let mut files: Vec<std::path::PathBuf> = vec![results_root.join("wire-goldens.tsv")];
    let sources = results_root.join("sources");
    collect(&sources, "tsv", &mut files, RECORD_DEPTH)?;
    collect(
        &sources.join("toolchain-logs"),
        "log",
        &mut files,
        RECORD_DEPTH,
    )?;
    files.sort();
    let mut changed = 0usize;
    let mut lines = 0usize;
    for path in &files {
        let Ok(before) = std::fs::read_to_string(path) else {
            continue;
        };
        let after = zerodds_interop_results::sources::without_the_machine(&before);
        if after == before {
            continue;
        }
        let moved = before
            .lines()
            .zip(after.lines())
            .filter(|(one, two)| one != two)
            .count();
        std::fs::write(path, &after).map_err(|error| format!("{}: {error}", path.display()))?;
        println!(
            "{}: {moved} line(s) no longer name the machine",
            path.strip_prefix(results_root).unwrap_or(path).display()
        );
        changed += 1;
        lines += moved;
    }
    if changed == 0 {
        println!(
            "{} recorded file(s); none names the machine it was measured on",
            files.len()
        );
    } else {
        println!(
            "{changed} of {} recorded file(s) rewritten, {lines} line(s); \
             run --regenerate to carry it into the tree",
            files.len()
        );
    }
    Ok(())
}

/// The depth [`collect`] descends, and the shape of the record it is.
///
/// `sources/*.tsv` sits at the root and `sources/toolchain-logs/<dir>/
/// *.log` one level under it — measured over the committed tree, 18
/// files at depth 0 and 479 at depth 1. Two is therefore the whole
/// shape, and stating it is what keeps this from following a symlink
/// out of the record.
const RECORD_DEPTH: usize = 2;

/// Every file under `directory` with this extension, one level deep and
/// then one more, which is the whole shape the record has.
///
/// zerodds-lint: recursion-depth 2 (`RECORD_DEPTH`, enforced by the
/// `remaining` argument below)
fn collect(
    directory: &Path,
    extension: &str,
    into: &mut Vec<std::path::PathBuf>,
    remaining: usize,
) -> Result<(), String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", directory.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", directory.display()))?;
        let path = entry.path();
        if path.is_dir() {
            // A directory below the shape the record has is not skipped
            // quietly: a file the normaliser never opens is a file that
            // keeps naming the machine, and nothing would say so.
            if remaining == 0 {
                return Err(format!(
                    "{}: the record is {RECORD_DEPTH} level(s) deep and this is deeper; \
                     either the tree changed shape, in which case raise `RECORD_DEPTH`, \
                     or something is here that does not belong to it",
                    path.display()
                ));
            }
            collect(&path, extension, into, remaining - 1)?;
        } else if path.extension().is_some_and(|found| found == extension) {
            into.push(path);
        }
    }
    Ok(())
}

fn read_ids(results_root: &Path) -> Result<RowIds, String> {
    let text = std::fs::read_to_string(results_root.join("row-ids.tsv")).unwrap_or_default();
    RowIds::read(&text)
}

/// What this run did not reach, by name and with its row count.
///
/// The per-axis derivation, one block per measured system.
///
/// What each block reports is what a dialect for that system has to
/// hold: the value every axis takes, whether the octets **pin** it, and
/// how many rows the assignment reproduces. An unpinned axis is a
/// question the recorded rows do not ask, and it is printed as such
/// rather than left to look like a measurement.
fn derive_dialects(cases: &[results::Case], inputs: &Inputs) -> String {
    use zerodds_interop_results::dialect_fit;
    use zerodds_interop_results::systems::Kind;

    let mut out = String::new();
    for system in SYSTEMS.iter().filter(|system| system.kind != Kind::Own) {
        let all = dialect_fit::rows(system, cases, &inputs.foreign, &inputs.orbs);
        for encoding in [
            zerodds_ast_layout::Encoding::PlainCdr1,
            zerodds_ast_layout::Encoding::PlainCdr2,
        ] {
            let rows: Vec<dialect_fit::Row> = all
                .iter()
                .filter(|row| row.encoding == encoding)
                .map(dialect_fit::Row::clone)
                .collect();
            if rows.is_empty() {
                continue;
            }
            let seeds = dialect_fit::dialect_seeds(encoding);
            let fit = dialect_fit::fit(system, cases, &rows, &seeds);
            out.push_str(&format!(
                "\n## {} {} — {} rows, {} reproduced, {} padding-only, {} missed\n\n",
                system.id,
                if encoding == zerodds_ast_layout::Encoding::PlainCdr1 {
                    "xcdr1"
                } else {
                    "xcdr2"
                },
                fit.rows,
                fit.answers.same(),
                fit.answers.padding_only(),
                fit.answers.missed()
            ));
            out.push_str(&block(&fit, cases, &rows));
        }
    }
    out
}

/// One system's axis table and the rows it misses.
fn block(
    fit: &zerodds_interop_results::dialect_fit::Fit,
    cases: &[results::Case],
    rows: &[zerodds_interop_results::dialect_fit::Row],
) -> String {
    use zerodds_interop_results::dialect_fit::{self, Verdict};
    let mut out = String::new();
    {
        for axis in dialect_fit::AXES {
            let pinned = fit.pinned.get(axis.name).copied().unwrap_or(false);
            let others = fit
                .indifferent
                .get(axis.name)
                .map(|values| values.join(", "))
                .unwrap_or_default();
            out.push_str(&format!(
                "| {:<22} | {:<26} | {} |{}\n",
                axis.name,
                fit.point.value_on(axis),
                if pinned { "pinned" } else { "no row asks" },
                if others.is_empty() {
                    String::new()
                } else {
                    format!(" also: {others}")
                }
            ));
        }
        for (row, verdict) in rows.iter().zip(&fit.answers.verdicts) {
            match verdict {
                Verdict::Same | Verdict::PaddingOnly => {}
                Verdict::Different(mine) => out.push_str(&format!(
                    "  missed {}/{}/{}: {} against {}\n",
                    cases[row.case].name, row.encoding_name, row.endian_name, mine, row.octets
                )),
                Verdict::Refused(why) => out.push_str(&format!(
                    "  refused {}/{}/{}: {why}\n",
                    cases[row.case].name, row.encoding_name, row.endian_name
                )),
            }
        }
    }
    out
}

/// A run that could not reach a toolchain says so and counts the cost.
/// Never a silent skip: a green job that stood for a matrix it never
/// measured would be worse than no job.
fn report_unreachable(ledger: &ledger::Ledger) {
    let mut unmeasured: Vec<(&str, usize, &str)> = Vec::new();
    for system in &SYSTEMS {
        let mut count = 0;
        let mut reason = "";
        for entry in &ledger.entries {
            if entry.system != system.id {
                continue;
            }
            if let Outcome::NotMeasured(detail) = &entry.write {
                count += 1;
                reason = detail;
            } else if matches!(entry.write, Outcome::NoProbe) {
                count += 1;
                reason = "no probe in this tree drives it";
            }
        }
        if count > 0 {
            unmeasured.push((system.id, count, reason));
        }
    }
    if unmeasured.is_empty() {
        println!("every column was reached in this run");
        return;
    }
    for (system, count, reason) in unmeasured {
        println!("{system}: NOT MEASURED HERE — {count} rows; produced by {reason}");
    }
}
