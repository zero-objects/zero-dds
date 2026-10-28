// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-python-matrix` — the Python column of the interop corpus,
//! whole, generated through `zerodds-lang-rules` the way
//! `wireprobe-c-matrix` drives the C column.
//!
//! ## The design `wireprobe-c-matrix` cannot share
//!
//! The C column's cost is compiling and freshly linking a binary per
//! distinct translation unit; this column has no compiler at all, so the
//! whole of that file's caching machinery — the digest, the probe cache,
//! the dedup pass — answers a question this column does not have.
//!
//! What it has instead is a process launch per unit, and CPython's own
//! start-up is what dominates at this column's scale: about eighty
//! thousand `python3` launches for the twelve configurations over the
//! whole corpus in both directions. [`python_matrix_worker.py`] is the
//! answer — one interpreter, handed a shard of jobs inline as JSON
//! (`source`, not `path`: that many small files on disk would buy
//! nothing a string does not already give `compile()` for a traceback's
//! sake), `exec()`ing each into a fresh, `sys.modules`-registered module
//! and reporting one result line per job. Its own module doc has the
//! rest, including why the module has to be registered at all —
//! `dataclasses` resolves a frozen class's `ClassVar`/`InitVar` markers
//! against `sys.modules[cls.__module__]` even under `from __future__
//! import annotations`, and a bare `exec(source, {})` raises before the
//! first sample this column measures ever runs.
//!
//! ## What it measures
//!
//! The same as `wireprobe-c-matrix`, minus the one form only the C
//! column carries: the payload, the same behind its encapsulation
//! header, and the read direction — the probe's own bytes handed back to
//! the probe, compared against the case's own sample in the canonical
//! form of `crates/wire-probe/src/canonical.rs`. No KeyHash: of the five
//! `zerodds-wireprobe-gen`-based columns only the C one carries a KeyHash
//! harness (`Column::FORMS` in `tools/wireprobe/wireprobe-zerodds-gen`),
//! and this column is asked to take the shape C# and Java have, which do
//! not.
//!
//! One byte-order pass answers both the bare payload and its
//! encapsulated form: every write job asks for
//! `payload+encapsulation`, whose four-byte header —
//! `encapsulation_header`, the same pure function of the encoding and
//! the byte order every other column computes it with — carries the
//! zero-padding count `crates/wire-codec/src/encode.rs`'s own
//! `Emit::PayloadWithEncapsulation` arm puts in its low two option bits
//! (§7.6.3.1.2). The header and that trailing padding are both stripped
//! off to answer the bare, unpadded payload, so a unit that would
//! otherwise be two probe runs is one.
//!
//! ## Output
//!
//! One JSON object, `case → slot → verdict`, in the shape
//! `wireprobe-c-matrix` produces, so
//! `crates/wire-probe/tests/three_column_gate.rs` compares both without
//! a translation layer in between.
#![forbid(unsafe_code)]
#![allow(
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::exit,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;
use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::encode::model_from_idl_in_dialect;
use zerodds_wire_probe::matrix::{CONFIGURATIONS, Configuration, ENCODINGS, ORDERS};
use zerodds_wire_probe::value::MapLengthPolicy;
use zerodds_wireprobe_python_gen::{Args, source};

const USAGE: &str = "\
wireprobe-python-matrix — the Python column of the interop corpus, whole

    --out <file>        where the JSON goes (required)
    --case <name>       only this case, repeatable (default: all)
    --cases <file>       only the cases named in this file, one per line
    --config <name>      only this configuration, repeatable
    --jobs <n>            how many python3 workers to run at once
    --keep                leave the work directory behind
";

fn main() {
    let options = match parse(std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(message) = run(&options) {
        eprintln!("wireprobe-python-matrix: {message}");
        std::process::exit(1);
    }
}

struct Options {
    out: PathBuf,
    only: Vec<String>,
    configurations: Vec<String>,
    jobs: usize,
    keep: bool,
}

/// One corpus case, read off disk once, with the on-disk paths
/// `wireprobe-zerodds-python-gen::Args` reads its IDL and its sample
/// from — [`source`] takes paths and not text, so a case's own files are
/// what every unit of it is generated from.
struct Case {
    name: String,
    type_name: String,
    idl_path: PathBuf,
    idl_text: String,
    values_path: PathBuf,
    values: Value,
}

/// One case's model under one configuration's dialect, and the case's
/// own sample in canonical form under it — built once per (case,
/// configuration) and read by every one of that case's read-direction
/// units under this configuration, rather than once per unit.
struct CaseModel {
    ast: Ast,
    type_: Id<TypeNode>,
    expected: String,
}

fn case_model(case: &Case, dialect: Dialect) -> Result<CaseModel, String> {
    let ast =
        model_from_idl_in_dialect(&case.idl_text, &format!("{}/type.idl", case.name), dialect)
            .map_err(|error| format!("the IDL does not lower: {error}"))?;
    let name = ScopedName::absolute(
        case.type_name
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
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
    Ok(CaseModel {
        ast,
        type_,
        expected,
    })
}

/// One measurement: a case, an encoding and a byte order, under the
/// configuration [`run_configuration`] is already inside.
struct Unit {
    case: usize,
    encoding: Encoding,
    endianness: Endianness,
    /// `<config>/<encoding>/<order>`, the key every slot of this unit is
    /// built from.
    stem: String,
}

#[allow(clippy::too_many_lines)]
fn run(options: &Options) -> Result<(), String> {
    let root = repository_root();
    let corpus = std::env::var_os("ZERODDS_CORPUS")
        .map_or_else(|| root.join("tests").join("interop-corpus"), PathBuf::from);
    let cases = read_cases(&corpus, &options.only)?;
    if cases.is_empty() {
        return Err(format!("{}: no cases", corpus.display()));
    }

    let runtime = std::env::var_os("ZERODDS_PYTHON_RUNTIME")
        .map_or_else(|| root.join("runtime").join("python"), PathBuf::from);
    if !runtime.join("zerodds_wire").is_dir() {
        return Err(format!(
            "{}: the Python runtime is missing",
            runtime.display()
        ));
    }
    let worker = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("bin")
        .join("python_matrix_worker.py");
    if !worker.is_file() {
        return Err(format!(
            "{}: the worker script is missing",
            worker.display()
        ));
    }
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());

    let build =
        std::env::temp_dir().join(format!("wireprobe-python-matrix-{}", std::process::id()));
    std::fs::create_dir_all(&build).map_err(|error| format!("{}: {error}", build.display()))?;

    let selected: Vec<&Configuration> = CONFIGURATIONS
        .iter()
        .filter(|configuration| {
            options.configurations.is_empty()
                || options
                    .configurations
                    .iter()
                    .any(|name| name == configuration.name)
        })
        .collect();
    if selected.is_empty() {
        return Err("no configuration matched `--config`".to_string());
    }

    let mut report: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for case in &cases {
        report.entry(case.name.clone()).or_default();
    }

    let started = std::time::Instant::now();
    let mut total_slots = 0usize;
    for settings in &selected {
        let config_started = std::time::Instant::now();
        let slots = run_configuration(
            settings,
            &cases,
            &python,
            &worker,
            &runtime,
            &build,
            options.jobs,
            &mut report,
        )?;
        total_slots += slots;
        eprintln!(
            "wireprobe-python-matrix: {} — {slots} slot(s) in {:.0}s",
            settings.name,
            config_started.elapsed().as_secs_f64()
        );
    }

    let text_out = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("the report does not serialise: {error}"))?;
    // Into a sibling temporary and then one rename, the same shape
    // `wireprobe-c-matrix` uses: a run killed while writing leaves the
    // temporary behind and whatever was there before intact, where a
    // direct write leaves a truncated file that still parses as JSON.
    let temporary = options
        .out
        .with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&temporary, text_out)
        .map_err(|error| format!("{}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, &options.out)
        .map_err(|error| format!("{}: {error}", options.out.display()))?;
    if !options.keep {
        let _ = std::fs::remove_dir_all(&build);
    }
    eprintln!(
        "wireprobe-python-matrix: {total_slots} slot(s) in {:.0}s to {}",
        started.elapsed().as_secs_f64(),
        options.out.display()
    );
    Ok(())
}

/// Every measurement of one configuration, over every case, in every
/// encoding and both byte orders, in both directions. Returns how many
/// slots it filled.
fn run_configuration(
    settings: &Configuration,
    cases: &[Case],
    python: &str,
    worker: &Path,
    runtime: &Path,
    build: &Path,
    jobs: usize,
    report: &mut BTreeMap<String, BTreeMap<String, String>>,
) -> Result<usize, String> {
    let mut units = Vec::with_capacity(cases.len() * ENCODINGS.len() * ORDERS.len());
    for case in 0..cases.len() {
        for (encoding_name, encoding) in ENCODINGS {
            for (order_name, endianness) in ORDERS {
                units.push(Unit {
                    case,
                    encoding,
                    endianness,
                    stem: format!("{}/{encoding_name}/{order_name}", settings.name),
                });
            }
        }
    }

    let map_length_with_delimiter = settings
        .map_length
        .map(|policy| policy == MapLengthPolicy::SequenceEquivalent);

    // -- generate every write source, in process --------------------
    let generated: Vec<Result<String, String>> = each(jobs, units.len(), |index| {
        let unit = &units[index];
        let case = &cases[unit.case];
        let args = Args {
            idl: case.idl_path.clone(),
            type_name: case.type_name.clone(),
            values: Some(case.values_path.clone()),
            encoding: unit.encoding,
            endianness: unit.endianness,
            encapsulation: true,
            dialect: settings.dialect,
            length_codes: settings.length_codes,
            map_length_with_delimiter,
            ..Args::default()
        };
        source(&args).map_err(|refusal| format!("!{}", refusal.message()))
    });

    let mut write_jobs: Vec<(usize, String)> = Vec::new();
    for (index, outcome) in generated.into_iter().enumerate() {
        match outcome {
            Ok(text) => write_jobs.push((index, text)),
            Err(message) => {
                let unit = &units[index];
                let unwritten = format!("!unwritten: {}", message.trim_start_matches('!'));
                let row = report.entry(cases[unit.case].name.clone()).or_default();
                row.insert(unit.stem.clone(), message.clone());
                row.insert(format!("{}+enc", unit.stem), message);
                row.insert(format!("{}/read", unit.stem), unwritten);
            }
        }
    }

    let write_manifest: Vec<(usize, String, Option<PathBuf>)> = write_jobs
        .iter()
        .map(|(index, source_text)| (*index, source_text.clone(), None))
        .collect();
    let write_results = run_workers(
        python,
        worker,
        runtime,
        build,
        &write_manifest,
        "write",
        jobs,
    )?;

    // -- payloads and the write-direction slots ----------------------
    let mut payloads: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for (index, _) in &write_jobs {
        let unit = &units[*index];
        let row = report.entry(cases[unit.case].name.clone()).or_default();
        match write_results.get(index) {
            Some(JobResult {
                exit: 0,
                hex: Some(hex_text),
                ..
            }) => {
                let bytes =
                    decode_hex(hex_text).map_err(|message| format!("{}: {message}", unit.stem))?;
                // The four-byte header, then the body, then — inside the
                // encapsulated form only — the zero padding
                // `crates/wire-codec/src/encode.rs`'s own
                // `Emit::PayloadWithEncapsulation` arm appends and counts
                // in the header's low two option bits (§7.6.3.1.2). The
                // bare payload this matrix's own `stem` slot reports is
                // the body alone: `Emit::Payload` stays unpadded, so
                // stripping only the header and keeping the trailing
                // zeros would report padding as part of the serialized
                // data itself.
                let declared_padding = bytes.get(3).copied().unwrap_or(0) & 0x03;
                if bytes.len() < 4 + usize::from(declared_padding) {
                    let message =
                        "!fewer bytes than the header and its declared padding need".to_string();
                    row.insert(unit.stem.clone(), message.clone());
                    row.insert(format!("{}+enc", unit.stem), message.clone());
                    row.insert(
                        format!("{}/read", unit.stem),
                        format!("!unwritten: {}", message.trim_start_matches('!')),
                    );
                    continue;
                }
                let body = &bytes[4..bytes.len() - usize::from(declared_padding)];
                row.insert(format!("{}+enc", unit.stem), hex_of(&bytes));
                row.insert(unit.stem.clone(), hex_of(body));
                payloads.insert(*index, body.to_vec());
            }
            Some(result) => {
                let message = refusal_message(result);
                row.insert(unit.stem.clone(), message.clone());
                row.insert(format!("{}+enc", unit.stem), message.clone());
                row.insert(
                    format!("{}/read", unit.stem),
                    format!("!unwritten: {}", message.trim_start_matches('!')),
                );
            }
            None => {
                let message = "!the probe was never run".to_string();
                row.insert(unit.stem.clone(), message.clone());
                row.insert(format!("{}+enc", unit.stem), message.clone());
                row.insert(
                    format!("{}/read", unit.stem),
                    format!("!unwritten: {}", message.trim_start_matches('!')),
                );
            }
        }
    }

    // -- generate every read source, from this column's own payload -
    let read_units: Vec<usize> = payloads.keys().copied().collect();
    let read_paths: Vec<(PathBuf, PathBuf)> = read_units
        .iter()
        .map(|index| {
            (
                build.join(format!("in-{}-{index}.bin", settings.name)),
                build.join(format!("out-{}-{index}.json", settings.name)),
            )
        })
        .collect();
    for (index, (input_path, _)) in read_units.iter().zip(&read_paths) {
        std::fs::write(input_path, &payloads[index])
            .map_err(|error| format!("{}: {error}", input_path.display()))?;
    }

    let generated_reads: Vec<Result<String, String>> = each(jobs, read_units.len(), |slot| {
        let index = read_units[slot];
        let unit = &units[index];
        let case = &cases[unit.case];
        let (input_path, values_out) = &read_paths[slot];
        let args = Args {
            idl: case.idl_path.clone(),
            type_name: case.type_name.clone(),
            input: Some(input_path.clone()),
            values_out: Some(values_out.clone()),
            encoding: unit.encoding,
            endianness: unit.endianness,
            dialect: settings.dialect,
            length_codes: settings.length_codes,
            map_length_with_delimiter,
            ..Args::default()
        };
        source(&args).map_err(|refusal| format!("!{}", refusal.message()))
    });

    let mut read_jobs: Vec<(usize, String)> = Vec::new();
    for (slot, outcome) in generated_reads.into_iter().enumerate() {
        let index = read_units[slot];
        let unit = &units[index];
        match outcome {
            Ok(text) => read_jobs.push((index, text)),
            Err(message) => {
                let row = report.entry(cases[unit.case].name.clone()).or_default();
                row.insert(format!("{}/read", unit.stem), message);
            }
        }
    }

    let values_out_of: BTreeMap<usize, PathBuf> = read_units
        .iter()
        .zip(&read_paths)
        .map(|(index, (_, values_out))| (*index, values_out.clone()))
        .collect();
    let read_manifest: Vec<(usize, String, Option<PathBuf>)> = read_jobs
        .iter()
        .map(|(index, text)| (*index, text.clone(), Some(values_out_of[index].clone())))
        .collect();
    let read_results = run_workers(python, worker, runtime, build, &read_manifest, "read", jobs)?;

    // Cached per case: `case_model` builds an `Ast` and canonicalises
    // the case's own sample under this configuration's dialect, and every
    // one of that case's read units under this configuration shares it.
    let mut models: BTreeMap<usize, Result<CaseModel, String>> = BTreeMap::new();

    for (index, _) in &read_jobs {
        let unit = &units[*index];
        let case = &cases[unit.case];
        let verdict = match read_results.get(index) {
            Some(JobResult {
                exit: 0,
                json: Some(found_text),
                ..
            }) => {
                let model = models
                    .entry(unit.case)
                    .or_insert_with(|| case_model(case, settings.dialect));
                match model {
                    Ok(model) => match serde_json::from_str::<Value>(found_text) {
                        Ok(found) => {
                            match canonicalise(&model.ast, model.type_, &found, "sample") {
                                Ok(value) if text(&value) == model.expected => "same".to_string(),
                                Ok(value) => {
                                    format!("different: {} for {}", text(&value), model.expected)
                                }
                                Err(error) => {
                                    format!("!what came back is not a value of this type: {error}")
                                }
                            }
                        }
                        Err(error) => format!("!the probe's JSON does not parse: {error}"),
                    },
                    Err(message) => format!("!{message}"),
                }
            }
            Some(result) => refusal_message(result),
            None => "!the probe was never run".to_string(),
        };
        report
            .entry(case.name.clone())
            .or_default()
            .insert(format!("{}/read", unit.stem), verdict);
    }

    // Three slots per unit: the payload, its encapsulated form and the
    // read verdict. Stated rather than counted out of `report` after the
    // fact, so a caller that changed how many this function fills is
    // caught by arithmetic and not by a diff.
    Ok(units.len() * 3)
}

/// One job's answer, from a worker's results line.
struct JobResult {
    exit: i64,
    hex: Option<String>,
    json: Option<String>,
    stderr: Option<String>,
}

/// The `!`-prefixed message a non-zero exit or a missing result answers
/// in — the worker's own captured first line of standard error where
/// there is one, and the bare exit code otherwise.
fn refusal_message(result: &JobResult) -> String {
    match &result.stderr {
        Some(line) if !line.is_empty() => format!("!{line}"),
        _ => format!("!the probe exited {}", result.exit),
    }
}

/// Run every job, sharded across `jobs` `python3` workers, and return
/// each job's answer keyed by the id it was given.
///
/// One process per shard and not one per job: [`python_matrix_worker.py`]
/// is what answers many jobs in one interpreter, and this is the
/// driver's half of that — partition, write each shard's manifest,
/// spawn every worker, then wait for all of them, so the shards run
/// concurrently and not one after another.
fn run_workers(
    python: &str,
    worker: &Path,
    runtime: &Path,
    build: &Path,
    jobs: &[(usize, String, Option<PathBuf>)],
    tag: &str,
    shard_count: usize,
) -> Result<BTreeMap<usize, JobResult>, String> {
    if jobs.is_empty() {
        return Ok(BTreeMap::new());
    }
    let kind = if tag == "write" { "write" } else { "read" };
    let shards = shard_count.max(1).min(jobs.len());
    let chunk = jobs.len().div_ceil(shards);

    struct Spawned {
        child: std::process::Child,
        results_path: PathBuf,
        log_path: PathBuf,
    }
    let mut spawned = Vec::new();
    for (shard, batch) in jobs.chunks(chunk).enumerate() {
        let manifest_path = build.join(format!("{tag}-manifest-{shard}.ndjson"));
        let results_path = build.join(format!("{tag}-results-{shard}.ndjson"));
        let log_path = build.join(format!("{tag}-log-{shard}.txt"));
        {
            let file = std::fs::File::create(&manifest_path)
                .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
            let mut writer = std::io::BufWriter::new(file);
            for (id, source_text, values_out) in batch {
                let line = serde_json::json!({
                    "id": id,
                    "kind": kind,
                    "source": source_text,
                    "values_out": values_out.as_ref().map(|path| path.display().to_string()),
                });
                serde_json::to_writer(&mut writer, &line)
                    .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
                writer
                    .write_all(b"\n")
                    .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
            }
            writer
                .flush()
                .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
        }
        let log = std::fs::File::create(&log_path)
            .map_err(|error| format!("{}: {error}", log_path.display()))?;
        let child = Command::new(python)
            .arg(worker)
            .arg(&manifest_path)
            .arg(&results_path)
            .env("PYTHONPATH", runtime)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|error| format!("the python worker could not be started: {error}"))?;
        spawned.push(Spawned {
            child,
            results_path,
            log_path,
        });
    }

    let mut out = BTreeMap::new();
    for entry in spawned {
        let Spawned {
            mut child,
            results_path,
            log_path,
        } = entry;
        let status = child
            .wait()
            .map_err(|error| format!("the python worker could not be waited on: {error}"))?;
        if !status.success() {
            let log_text = std::fs::read_to_string(&log_path).unwrap_or_default();
            return Err(format!(
                "a python worker exited {status}; its log is {}: {}",
                log_path.display(),
                log_text.lines().take(5).collect::<Vec<_>>().join(" | ")
            ));
        }
        let results_text = std::fs::read_to_string(&results_path)
            .map_err(|error| format!("{}: {error}", results_path.display()))?;
        for line in results_text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)
                .map_err(|error| format!("{}: {error}", results_path.display()))?;
            let id = value["id"]
                .as_u64()
                .ok_or_else(|| format!("{}: a result with no `id`", results_path.display()))?
                as usize;
            out.insert(
                id,
                JobResult {
                    exit: value["exit"].as_i64().unwrap_or(1),
                    hex: value["hex"].as_str().map(str::to_string),
                    json: value["json"].as_str().map(str::to_string),
                    stderr: value["stderr"].as_str().map(str::to_string),
                },
            );
        }
    }
    Ok(out)
}

fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd-length hex".to_string());
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte =
            u8::from_str_radix(&text[index..index + 2], 16).map_err(|error| format!("{error}"))?;
        out.push(byte);
        index += 2;
    }
    Ok(out)
}

fn hex_of(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Run `body` over `0..count` on `jobs` threads and collect the results,
/// each tagged with its index. The same shape `wireprobe-c-matrix`'s own
/// `each` takes: the work items are independent, and a shared counter is
/// enough for items this coarse.
fn each<T, F>(jobs: usize, count: usize, body: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync,
{
    let next = AtomicUsize::new(0);
    let collected: Mutex<Vec<(usize, T)>> = Mutex::new(Vec::with_capacity(count));
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| {
                let mut mine = Vec::new();
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= count {
                        break;
                    }
                    mine.push((index, body(index)));
                }
                if let Ok(mut collected) = collected.lock() {
                    collected.append(&mut mine);
                }
            });
        }
    });
    let mut collected = match collected.into_inner() {
        Ok(results) => results,
        Err(error) => {
            eprintln!("wireprobe-python-matrix: a worker panicked: {error}");
            std::process::exit(1);
        }
    };
    collected.sort_by_key(|(index, _)| *index);
    collected.into_iter().map(|(_, value)| value).collect()
}

fn read_cases(corpus: &Path, only: &[String]) -> Result<Vec<Case>, String> {
    let mut directories: Vec<PathBuf> = std::fs::read_dir(corpus)
        .map_err(|error| format!("{}: {error}", corpus.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.join("type").is_file())
        .collect();
    directories.sort();
    let mut cases = Vec::new();
    for directory in directories {
        let name = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        if !only.is_empty() && !only.contains(&name) {
            continue;
        }
        let read = |file: &str| {
            std::fs::read_to_string(directory.join(file))
                .map_err(|error| format!("{name}/{file}: {error}"))
        };
        let values_text = read("values.json")?;
        cases.push(Case {
            type_name: read("type")?.trim().to_string(),
            idl_path: directory.join("type.idl"),
            idl_text: read("type.idl")?,
            values: serde_json::from_str(&values_text)
                .map_err(|error| format!("{name}/values.json: {error}"))?,
            values_path: directory.join("values.json"),
            name,
        });
    }
    Ok(cases)
}

fn repository_root() -> PathBuf {
    // `tools/wireprobe/wireprobe-zerodds-python-gen` is three below the
    // root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn parse(argv: Vec<String>) -> Result<Options, String> {
    let mut out = None;
    let mut only = Vec::new();
    let mut configurations = Vec::new();
    let mut jobs = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let mut keep = false;
    let mut rest = argv.into_iter();
    while let Some(flag) = rest.next() {
        let mut next = || rest.next().ok_or_else(|| format!("`{flag}` takes a value"));
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(next()?)),
            "--case" => only.push(next()?),
            "--cases" => {
                let path = next()?;
                let text =
                    std::fs::read_to_string(&path).map_err(|error| format!("{path}: {error}"))?;
                only.extend(
                    text.lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_string),
                );
            }
            "--config" => {
                let name = next()?;
                if !CONFIGURATIONS.iter().any(|entry| entry.name == name) {
                    return Err(format!("`{name}` is not a configuration"));
                }
                configurations.push(name);
            }
            "--jobs" => {
                jobs = next()?
                    .parse()
                    .map_err(|error| format!("--jobs takes a number: {error}"))?;
            }
            "--keep" => keep = true,
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("`{other}` is not an option")),
        }
    }
    Ok(Options {
        out: out.ok_or("--out is required")?,
        only,
        configurations,
        jobs,
        keep,
    })
}
