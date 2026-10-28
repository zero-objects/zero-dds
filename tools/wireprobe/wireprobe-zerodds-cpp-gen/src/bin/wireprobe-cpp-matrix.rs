// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-cpp-matrix` — the C++ column of the interop corpus, whole.
//!
//! `tools/wireprobe/wireprobe-cpp-corpus` runs one case in one
//! configuration per invocation, and each invocation launches the
//! generator, compiles `runtime/cpp`'s four translation units and the
//! probe, and throws all of it away.
//! `crates/wire-probe/tests/reference_columns.rs` wants this column over
//! the **whole** matrix — twelve configurations, five encodings, both byte
//! orders, four emit forms, 420 slots per case — and a per-case process
//! costs more than the measurement.
//!
//! This does the same work with five costs removed:
//!
//! 1. **The runtime is compiled once.** `runtime/cpp`'s four sources
//!    become one archive at start-up instead of being recompiled for every
//!    probe. Not measured for this column.
//! 2. **The generator runs in process.** [`zerodds_wireprobe_cpp_gen`] is
//!    a library, so the corpus's tens of thousands of generations cost no
//!    process launches.
//!    Not measured for this column.
//! 3. **One binary answers eight slots.** The byte order and the emit form
//!    are the compiled program's arguments rather than baked into it —
//!    [`zerodds_wireprobe_cpp_gen::combined_sources`] has the argument for
//!    both, and the C column's is the measurement they were checked
//!    against: on macOS the *first* execution of a freshly linked binary
//!    costs about 235 ms of signature validation against 2 ms for every one
//!    after, and the check does not parallelise, so the binary count and
//!    not the compiler is what a sweep costs.
//! 4. **A probe is compiled once ever.** What comes out of the compiler is
//!    kept under `target/wireprobe-cpp-cache`, keyed by the generated C++,
//!    the compiler and the runtime's sources. Not measured for this column.
//! 5. **Identical C++ is compiled once.** Most of the matrix's
//!    configurations produce the same translation unit for a given case —
//!    a `@final struct { long a; }` frames the same way under every
//!    dialect — so the sources are hashed and only the distinct ones reach
//!    the compiler. How many of this column's units are distinct is not
//!    measured.
//!
//! ## What it measures
//!
//! Per case and per configuration, in every encoding and both byte orders:
//!
//! - the payload, as hex;
//! - the same behind its encapsulation header, with §7.6.3.1.2's count of
//!   the trailing padding in the header's options field;
//! - the KeyHash, which has no byte order of its own;
//! - the **read** direction: the C++ probe's own payload handed back to the
//!   C++ probe.
//!
//! Its own bytes, and not another column's: a column that reads another's
//! says the two agree and not that this one is self-consistent, and the
//! comparison against the sample is what tells the two apart — two
//! decoders wrong in the same way agree with each other and disagree with
//! the sample.
//!
//! ## The read direction hands out a document, and reaches no verdict
//!
//! `reference_columns.rs` compares the read slots by canonicalising what
//! the column hands back against the case's own sample, and it says why:
//! a verdict reached inside the column is a verdict against that column's
//! own idea of what a value is, and a value its decoder can build which no
//! writer of its own would emit falls outside the comparison entirely.
//! So a read slot here is `json:` and the probe's own JSON, unmodified,
//! and this driver decides nothing about it.
//!
//! That is the one place this report is not the C driver's cell for cell:
//! `wireprobe-c-matrix` files `same` or `different: …` there, because
//! `crates/wire-probe/tests/three_column_gate.rs` is what consumes *that*
//! report and it wants the verdict. A slot the C++ column cannot answer
//! is refused in the C driver's vocabulary — `!` and a sentence, and
//! `!unwritten:` for a read slot whose payload was never produced — so
//! that a gate comparing refusal *classes* reads the same two columns the
//! same way.
//!
//! ## Output
//!
//! One JSON object, `case → slot → cell`, in the alphabet
//! `reference_columns.rs` reads: lower-case hex for the octets, `json:`
//! and a document for the read direction, `!` and a sentence for a slot
//! this column will not produce.
#![forbid(unsafe_code)]
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::exit)]

use std::collections::BTreeMap;
use std::hash::{Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::matrix::{CONFIGURATIONS, Configuration, ENCODINGS, ORDERS};
use zerodds_wire_probe::value::MapLengthPolicy;
use zerodds_wireprobe_cpp_gen::{
    Args, Emit, Sources, combined_sources, combined_sources_with, declares_a_key,
};

const USAGE: &str = "\
wireprobe-cpp-matrix — the C++ column of the interop corpus, whole

    --out <file>        where the JSON goes (required)
    --case <name>       only this case, repeatable (default: all)
    --cases <file>      only the cases named in this file, one per line
    --jobs <n>          how many compilers to run at once
    --cache <dir>       where compiled probes are kept between runs
                        (default: target/wireprobe-cpp-cache)
    --config <name>     only this configuration, repeatable
    --keep              leave the build directory behind, for a look at
                        the C++ a failing case produced
";

/// What one probe run asks the compiled program for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    /// The serialised sample.
    Payload,
    /// The same behind its four-byte encapsulation header.
    Encapsulated,
    /// The sixteen-byte KeyHash, which has no byte order of its own.
    KeyHash,
    /// The probe's own payload handed back to the probe.
    Read,
}

impl Form {
    /// The word the compiled program takes on its command line.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Payload => "payload",
            Self::Encapsulated => "payload+encapsulation",
            Self::KeyHash => "keyhash",
            Self::Read => "read",
        }
    }
}

/// One measurement the driver has to make.
#[derive(Clone)]
struct Slot {
    /// Index into the case list.
    case: usize,
    /// The key the cell is filed under.
    key: String,
    /// Which translation unit answers it.
    unit: Unit,
    form: Form,
    endianness: Endianness,
}

impl Slot {
    /// What this slot is called inside its unit's answers: the byte order
    /// and the form, with the case, the configuration and the encoding
    /// stripped off because the unit already fixes those.
    fn suffix(&self) -> String {
        let order = match self.endianness {
            Endianness::Little => "le",
            Endianness::Big => "be",
        };
        match self.form {
            Form::Payload => order.to_string(),
            Form::Encapsulated => format!("{order}+enc"),
            Form::Read => format!("{order}/read"),
            Form::KeyHash => "keyhash".to_string(),
        }
    }
}

/// What decides the translation unit: a case, a configuration and an
/// encoding, and nothing else.
///
/// Not the byte order and not the emit form. Both are arguments to the
/// compiled program — see [`zerodds_wireprobe_cpp_gen::combined_sources`]
/// for why they can be.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Unit {
    configuration: usize,
    encoding: Encoding,
}

/// One corpus case, read off disk once.
///
/// Its **paths** and not their text: [`zerodds_wireprobe_cpp_gen::Args`]
/// takes the IDL file and the sample as paths, so the generator reads both
/// and this driver holds neither — there is no verdict here that would
/// need either of them in hand.
struct Case {
    name: String,
    directory: PathBuf,
    type_name: String,
}

fn main() {
    let options = match parse(std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(message) = run(&options) {
        eprintln!("wireprobe-cpp-matrix: {message}");
        std::process::exit(1);
    }
}

struct Options {
    out: PathBuf,
    only: Vec<String>,
    configurations: Vec<String>,
    jobs: usize,
    keep: bool,
    /// Where compiled probes are kept between runs.
    cache: Option<PathBuf>,
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

    let build = std::env::temp_dir().join(format!("wireprobe-cpp-matrix-{}", std::process::id()));
    std::fs::create_dir_all(&build).map_err(|error| format!("{}: {error}", build.display()))?;
    let archive = build_runtime(&root, &build)?;
    // Kept between runs, so a developer who runs this twice pays the
    // compiler once.
    let cache = Cache::open(
        options
            .cache
            .clone()
            .unwrap_or_else(|| root.join("target").join("wireprobe-cpp-cache")),
        &runtime_directory(&root),
    )?;

    let slots = plan(&cases, &options.configurations);
    // One translation unit per case, configuration and encoding: a fifth
    // of the slots, because the byte order and the emit form are the
    // compiled program's arguments.
    let units: Vec<(usize, Unit)> = {
        let mut seen = Vec::new();
        for slot in &slots {
            let entry = (slot.case, slot.unit);
            if seen.last() != Some(&entry) {
                seen.push(entry);
            }
        }
        seen.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        seen
    };
    eprintln!(
        "wireprobe-cpp-matrix: {} case(s), {} slot(s), {} unit(s), {} job(s)",
        cases.len(),
        slots.len(),
        units.len(),
        options.jobs
    );

    let started = std::time::Instant::now();

    // Pass one: generate every unit's C++ and keep only its digest, so
    // that the corpus's tens of thousands of translation units never sit in
    // memory at once.
    // The first unit to reach a digest is the one regenerated and compiled
    // for it.
    let generated = each(options.jobs, units.len(), |index| {
        let (case, unit) = units[index];
        (
            index,
            source_for(&cases[case], unit).map(|sources| digest(&sources)),
        )
    });
    let mut digests: Vec<Result<u128, String>> = vec![Ok(0); units.len()];
    for (index, outcome) in generated {
        digests[index] = outcome;
    }

    let mut representative: BTreeMap<u128, usize> = BTreeMap::new();
    for (index, outcome) in digests.iter().enumerate() {
        if let Ok(digest) = outcome {
            representative.entry(*digest).or_insert(index);
        }
    }
    let distinct: Vec<(u128, usize)> = representative.iter().map(|(a, b)| (*a, *b)).collect();
    eprintln!(
        "wireprobe-cpp-matrix: {} distinct translation unit(s) to compile after {:.0}s",
        distinct.len(),
        started.elapsed().as_secs_f64()
    );

    // Pass two: compile and run one probe per distinct translation unit,
    // once for each byte order and emit form. Two configurations that
    // produced the same C++ produce the same bytes, which is what makes
    // attributing one run to both sound.
    let done = AtomicUsize::new(0);
    let measured = each(options.jobs, distinct.len(), |index| {
        let (digest, unit_index) = distinct[index];
        let (case, unit) = units[unit_index];
        let outcome = measure(&cases[case], unit, &archive, &cache, &build);
        let seen = done.fetch_add(1, Ordering::Relaxed) + 1;
        if seen % 500 == 0 {
            eprintln!(
                "wireprobe-cpp-matrix: {seen}/{} unit(s) after {:.0}s",
                distinct.len(),
                started.elapsed().as_secs_f64()
            );
        }
        (digest, outcome)
    });
    // A driver failure — a directory that could not be made, a compiler
    // that could not be started — is not a verdict about the column, so
    // it stops the run rather than being filed as one case's answer.
    let mut by_digest: BTreeMap<u128, BTreeMap<String, String>> = BTreeMap::new();
    for (digest, outcome) in measured {
        by_digest.insert(digest, outcome?);
    }

    let mut unit_of: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (index, (case, unit)) in units.iter().enumerate() {
        unit_of.insert((*case, unit_index_of(*unit)), index);
    }

    let unreachable = "!the probe was never built".to_string();
    let mut report: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut unwritten: String;
    let mut no_key: String;
    for slot in &slots {
        let index = unit_of[&(slot.case, unit_index_of(slot.unit))];
        let verdict = match &digests[index] {
            Ok(digest) => by_digest
                .get(digest)
                .and_then(|answers| answers.get(&slot.suffix()))
                .unwrap_or(&unreachable)
                .as_str(),
            // A unit whose C++ could not be generated has no probe at
            // all, so every slot it would have answered carries the
            // generator's reason — and its read slots carry `unwritten`,
            // because nothing was written for them to read.
            // `refused_everywhere` has the argument.
            Err(message) if slot.form == Form::Read => {
                unwritten = format!("!unwritten: {}", message.trim_start_matches('!'));
                unwritten.as_str()
            }
            // A keyless type has no KeyHash whatever stopped its unit, so
            // its KeyHash slot says that and not the unit's reason
            // (`declares_a_key`). The probe gives the same sentence when
            // the unit compiles.
            Err(message) if slot.form == Form::KeyHash => {
                let case = &cases[slot.case];
                match declares_a_key(&generator_args(case, slot.unit)) {
                    Ok(false) => {
                        no_key = format!("!{} declares no @key member", case.type_name);
                        no_key.as_str()
                    }
                    Ok(true) | Err(_) => message.as_str(),
                }
            }
            Err(message) => message.as_str(),
        };
        report
            .entry(cases[slot.case].name.clone())
            .or_default()
            .insert(slot.key.clone(), verdict.to_string());
    }

    let text = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("the report does not serialise: {error}"))?;
    // Into a sibling temporary and then one rename, the same shape the C
    // driver uses: a run killed while writing leaves the temporary behind
    // and whatever was there before intact, where a direct write leaves a
    // truncated file that still parses as JSON and reads as a shorter
    // matrix — a wrong answer with no error, in the tool whose whole job
    // is to catch those.
    let temporary = options
        .out
        .with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&temporary, text)
        .map_err(|error| format!("{}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, &options.out)
        .map_err(|error| format!("{}: {error}", options.out.display()))?;
    if !options.keep {
        let _ = std::fs::remove_dir_all(&build);
    }
    eprintln!(
        "wireprobe-cpp-matrix: {} slot(s) in {:.0}s to {}",
        slots.len(),
        started.elapsed().as_secs_f64(),
        options.out.display()
    );
    Ok(())
}

/// A unit's position in the `(configuration, encoding)` grid.
///
/// Used only as a map key beside the case index; the grid is small and
/// fixed, so a number is clearer than a derived `Ord`.
fn unit_index_of(unit: Unit) -> usize {
    let encoding = ENCODINGS
        .iter()
        .position(|(_, encoding)| *encoding == unit.encoding)
        .unwrap_or(0);
    unit.configuration * ENCODINGS.len() + encoding
}

/// Every measurement the matrix holds, in a stable order.
fn plan(cases: &[Case], wanted: &[String]) -> Vec<Slot> {
    let mut slots = Vec::new();
    for case in 0..cases.len() {
        for (configuration, settings) in CONFIGURATIONS.iter().enumerate() {
            if !wanted.is_empty() && !wanted.iter().any(|name| name == settings.name) {
                continue;
            }
            for (encoding_name, encoding) in ENCODINGS {
                let unit = Unit {
                    configuration,
                    encoding,
                };
                for (order_name, endianness) in ORDERS {
                    let stem = format!("{}/{encoding_name}/{order_name}", settings.name);
                    for (key, form) in [
                        (stem.clone(), Form::Payload),
                        (format!("{stem}+enc"), Form::Encapsulated),
                        (format!("{stem}/read"), Form::Read),
                    ] {
                        slots.push(Slot {
                            case,
                            key,
                            unit,
                            form,
                            endianness,
                        });
                    }
                }
                // The KeyHash holder is big-endian whatever the sample is
                // — XTypes 1.3 §7.6.8 — so it has no byte-order axis.
                slots.push(Slot {
                    case,
                    key: format!("{}/{encoding_name}/keyhash", settings.name),
                    unit,
                    form: Form::KeyHash,
                    endianness: Endianness::Big,
                });
            }
        }
    }
    slots
}

/// The generator's arguments for one translation unit.
///
/// `endianness` and `emit` are set because [`Args`] has the fields;
/// `combined_sources` ignores both, which is what lets one unit answer for
/// every byte order and every form.
fn generator_args(case: &Case, unit: Unit) -> Args {
    let settings: &Configuration = &CONFIGURATIONS[unit.configuration];
    Args {
        idl: case.directory.join("type.idl"),
        type_name: case.type_name.clone(),
        values: Some(case.directory.join("values.json")),
        read: false,
        encoding: unit.encoding,
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
        // The framing, the `wchar` width and the map-delimiter half are
        // left to the dialect: every configuration that reaches them
        // reaches them by naming a product, and an override here would be
        // a column the matrix does not have.
        wchar_framing: None,
        wchar_width: None,
        map_delimiter: None,
        wstring_unit_order: settings.wstring_unit_order,
        map_length_with_delimiter: settings
            .map_length
            .map(|policy| policy == MapLengthPolicy::SequenceEquivalent),
        // No column-specific option: `--runtime` is TypeScript's and
        // nothing here has one. Spelled through `Args::default` so a flag
        // added to the stem does not have to be added here to keep this
        // compiling — and, more to the point, so that adding one cannot
        // silently give this driver a value it did not choose.
        ..Args::default()
    }
}

fn source_for(case: &Case, unit: Unit) -> Result<Sources, String> {
    combined_sources(&generator_args(case, unit)).map_err(|error| format!("!generator: {error}"))
}

/// The same unit with the read direction left out.
fn write_only_source_for(case: &Case, unit: Unit) -> Result<Sources, String> {
    combined_sources_with(&generator_args(case, unit), false)
        .map_err(|error| format!("!generator: {error}"))
}

/// A 128-bit digest of one translation unit.
///
/// Two independent 64-bit hashes rather than one: the driver attributes a
/// compiled probe's bytes to every slot with the same digest, so a
/// collision would report one case's bytes as another's. `SipHash` and not
/// a cryptographic hash because nothing here is adversarial — the inputs
/// are the corpus.
fn digest(sources: &Sources) -> u128 {
    let mut low = std::hash::DefaultHasher::new();
    sources.types.hash(&mut low);
    sources.probe.hash(&mut low);
    let mut high = std::hash::DefaultHasher::new();
    // A different seed, reached by hashing the same text in the other
    // order behind a length prefix.
    sources.probe.len().hash(&mut high);
    sources.probe.hash(&mut high);
    sources.types.hash(&mut high);
    (u128::from(high.finish()) << 64) | u128::from(low.finish())
}

/// Where a compiled probe is kept between runs, and what its name is.
///
/// The digest of the C++, the compiler's identity and the runtime's
/// sources — anything that decides the binary, and nothing about where it
/// will run. A key that missed one of the three would hand back a probe
/// built by another compiler or against another runtime.
///
/// # Why a cache at all
///
/// The C column's cost is the compiler but not the number of *freshly
/// linked* binaries, and the first execution of one is the expensive part
/// on macOS. A probe that is already on disk is neither compiled nor
/// freshly linked, so a second run costs what its `fwrite` calls cost and
/// nothing else.
struct Cache {
    root: PathBuf,
    /// What the compiler and the runtime contribute to every key.
    toolchain: u64,
}

impl Cache {
    /// The cache under `root`, fingerprinted against this toolchain.
    ///
    /// The runtime's **sources** and not the archive built from them:
    /// `ar` stamps each member with a modification time and a uid, so two
    /// archives built from identical objects differ byte for byte and
    /// every key would miss.
    fn open(root: PathBuf, runtime: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|error| format!("{}: {error}", root.display()))?;
        let cxx = cxx();
        let version = Command::new(&cxx)
            .arg("--version")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        let mut hasher = std::hash::DefaultHasher::new();
        version.hash(&mut hasher);
        cxxflags().hash(&mut hasher);
        for directory in [runtime.join("src"), runtime.join("include")] {
            let mut files = Vec::new();
            files_under(&directory, &mut files)?;
            for file in files {
                // The path from the runtime's root and not the leaf: two
                // headers of one name under different directories must not
                // hash alike, and a source removed from the runtime changes
                // what links even when nothing that is left changed.
                file.strip_prefix(runtime)
                    .unwrap_or(&file)
                    .hash(&mut hasher);
                std::fs::read(&file)
                    .map_err(|error| format!("{}: {error}", file.display()))?
                    .hash(&mut hasher);
            }
        }
        Ok(Self {
            root,
            toolchain: hasher.finish(),
        })
    }

    /// The file one variant of one unit is kept under.
    fn path(&self, digest: u128, variant: &str, suffix: &str) -> PathBuf {
        self.root.join(format!(
            "{digest:032x}-{:016x}-{variant}{suffix}",
            self.toolchain
        ))
    }
}

/// Every file under `directory`, and under the directories below it.
///
/// Recursive because `runtime/cpp/include` is a tree and not a flat list:
/// `zerodds/wire.hpp` sits under `include/zerodds/` and `dds/rpc.hpp`
/// under `include/dds/`, and a flat walk would try to read a directory as
/// a file and stop the run before it compiled anything.
/// `runtime/c/include` is the one flat header `wireprobe-c-matrix`
/// fingerprints, which is why that driver does not need this.
///
/// # Errors
/// A message naming the directory that could not be read.
///
/// zerodds-lint: recursion-depth 3 — one level per directory, and
/// `runtime/cpp/include`'s deepest file is `dds/rpc/zerodds.hpp` at two
/// below it. The walk cannot cycle: a filesystem entry below a directory
/// is reached once, and `is_dir` follows no symbolic link back up.
fn files_under(directory: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            files_under(&entry, out)?;
        } else {
            out.push(entry);
        }
    }
    Ok(())
}

/// A probe that is ready to run, or the reason there is none.
enum Built {
    /// The executable, and whether its read direction is in it.
    Probe { path: PathBuf, reads: bool },
    /// Neither variant compiled.
    Broken(String),
}

/// Compile one translation unit — or find it already compiled — and run
/// it in every form and byte order.
///
/// One compile and one first execution, on a cold cache, answering all
/// seven slots the unit covers.
fn measure(
    case: &Case,
    unit: Unit,
    archive: &Path,
    cache: &Cache,
    work: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let digest = digest(&source_for(case, unit)?);
    // A unit whose decoder does not compile is retried without it. One
    // broken form must not take the other three with it: a column that
    // wrote a payload perfectly well and only failed to compile its
    // *reader* would understate itself and hide the actual defect behind a
    // bigger one.
    let built = match build(case, unit, archive, cache, digest, true)? {
        Built::Probe { path, .. } => Built::Probe { path, reads: true },
        Built::Broken(message) => match build(case, unit, archive, cache, digest, false)? {
            Built::Probe { path, .. } => Built::Probe { path, reads: false },
            // Both variants failed, and the one that names the defect is
            // the full one: the fallback fails for the same reason plus
            // whatever else.
            Built::Broken(_) => Built::Broken(message),
        },
    };

    let mut answers = BTreeMap::new();
    let (probe, read_failure) = match &built {
        Built::Broken(message) => return Ok(refused_everywhere(message)),
        Built::Probe { path, reads } => (
            path,
            // The message the full variant gave, recovered from the cache
            // so a warm run reports the same defect a cold one did rather
            // than a vaguer one.
            if *reads {
                None
            } else {
                Some(
                    std::fs::read_to_string(cache.path(digest, "full", ".err"))
                        .unwrap_or_else(|_| "!does not compile".to_string()),
                )
            },
        ),
    };

    for (order_name, endianness) in ORDERS {
        let order = match endianness {
            Endianness::Little => "le",
            Endianness::Big => "be",
        };
        let payload = run_probe(probe, Form::Payload, order);
        answers.insert(
            format!("{order_name}+enc"),
            run_probe(probe, Form::Encapsulated, order)
                .map_or_else(|message| message, |bytes| hex(&bytes)),
        );
        answers.insert(
            format!("{order_name}/read"),
            match (&read_failure, &payload) {
                (Some(message), _) => message.clone(),
                (None, Ok(bytes)) => read_back(case, probe, order, bytes, work),
                // Nothing was written, so there is nothing to read. Told
                // apart from a reader that failed: the two are different
                // findings.
                (None, Err(message)) => format!("!unwritten: {}", message.trim_start_matches('!')),
            },
        );
        answers.insert(
            order_name.to_string(),
            payload.map_or_else(|message| message, |bytes| hex(&bytes)),
        );
    }
    answers.insert(
        "keyhash".to_string(),
        run_probe(probe, Form::KeyHash, "be").map_or_else(|message| message, |bytes| hex(&bytes)),
    );
    Ok(answers)
}

/// One variant of one unit, from the cache or from the compiler.
fn build(
    case: &Case,
    unit: Unit,
    archive: &Path,
    cache: &Cache,
    digest: u128,
    with_read: bool,
) -> Result<Built, String> {
    let variant = if with_read { "full" } else { "noread" };
    let binary = cache.path(digest, variant, ".bin");
    if binary.is_file() {
        return Ok(Built::Probe {
            path: binary,
            reads: with_read,
        });
    }
    let failure = cache.path(digest, variant, ".err");
    if let Ok(message) = std::fs::read_to_string(&failure) {
        return Ok(Built::Broken(message));
    }

    let sources = if with_read {
        source_for(case, unit)?
    } else {
        write_only_source_for(case, unit)?
    };
    // Built beside the cache rather than in the system temp directory, so
    // the rename into place is a rename and not a copy across
    // filesystems.
    let directory = cache.root.join(format!(
        "build-{}-{digest:032x}-{variant}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    std::fs::write(directory.join("zd_types.hpp"), &sources.types)
        .map_err(|error| format!("{error}"))?;
    std::fs::write(directory.join("probe.cpp"), &sources.probe)
        .map_err(|error| format!("{error}"))?;
    let probe = directory.join("probe");
    let outcome = compile(&directory, &probe, archive)?;
    match outcome {
        Ok(()) => {
            // Losing the race is not an error: the other builder produced
            // the same binary from the same inputs, which is what the key
            // says. Either way the build directory goes, and the probe that
            // answers is the one in the cache.
            let _ = std::fs::rename(&probe, &binary);
            let _ = std::fs::remove_dir_all(&directory);
            Ok(Built::Probe {
                path: binary,
                reads: with_read,
            })
        }
        Err(message) => {
            let _ = std::fs::write(&failure, &message);
            let _ = std::fs::remove_dir_all(&directory);
            Ok(Built::Broken(message))
        }
    }
}

/// Every slot of one unit, filled with one refusal.
///
/// The read slots say `unwritten` rather than repeating the reason,
/// because that is what happened to them: no bytes were produced, so there
/// was nothing to read and the reader was never asked. The other two
/// columns mark theirs the same way, and a gate that compares the *class*
/// of a refusal would otherwise read "this reader refused" where the truth
/// is "this reader was never reached".
fn refused_everywhere(message: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let unwritten = format!("!unwritten: {}", message.trim_start_matches('!'));
    for (order, _) in ORDERS {
        out.insert(order.to_string(), message.to_string());
        out.insert(format!("{order}+enc"), message.to_string());
        out.insert(format!("{order}/read"), unwritten.clone());
    }
    out.insert("keyhash".to_string(), message.to_string());
    out
}

/// One run of the compiled probe. `Err` carries the refusal, already
/// marked with the `!` the report files a refusal under.
fn run_probe(probe: &Path, form: Form, order: &str) -> Result<Vec<u8>, String> {
    match Command::new(probe).arg(form.as_str()).arg(order).output() {
        Ok(output) if output.status.success() => Ok(output.stdout),
        Ok(output) => Err(format!("!{}", first_line(&output.stderr))),
        Err(error) => Err(format!("!the probe could not be started: {error}")),
    }
}

/// The read direction: the C++ probe's own bytes back into the C++ probe.
///
/// Its own bytes, because a column that reads another's says the two agree
/// and not that this one is self-consistent. What comes back is filed as
/// the document it is — `json:` and the probe's own JSON — and this driver
/// reaches no verdict about it: the comparison belongs to
/// `crates/wire-probe/tests/reference_columns.rs`, which canonicalises it
/// against the case's own sample.
///
/// The JSON is parsed here for one reason and one only: a document that
/// does not parse is reported as such instead of being handed on for a
/// comparison to fail somewhere else.
fn read_back(case: &Case, probe: &Path, order: &str, payload: &[u8], directory: &Path) -> String {
    // A counter and not the case's name: two threads reading two units of
    // the same case in the same byte order would otherwise write the same
    // file, and each would read the other's payload.
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = directory.join(format!(
        "bytes-{order}-{}.bin",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(error) = std::fs::write(&path, payload) {
        return format!("!the payload could not be handed to the probe: {error}");
    }
    let input = match std::fs::File::open(&path) {
        Ok(input) => input,
        Err(error) => return format!("!the payload could not be handed to the probe: {error}"),
    };
    let output = match Command::new(probe)
        .arg(Form::Read.as_str())
        .arg(order)
        .stdin(std::process::Stdio::from(input))
        .output()
    {
        Ok(output) => output,
        Err(error) => return format!("!the probe could not be started: {error}"),
    };
    if !output.status.success() {
        return format!("!{}", first_line(&output.stderr));
    }
    let document = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if let Err(error) = serde_json::from_str::<serde_json::Value>(&document) {
        // `case` is in the sentence because the same shape of document is
        // wrong for one case and right for another.
        return format!("!{}: the probe's JSON does not parse: {error}", case.name);
    }
    format!("json:{document}")
}

/// `c++` over the probe and the prebuilt runtime archive.
///
/// The outer `Result` is the driver failing; the inner one is the probe
/// failing to compile, which is a verdict and not an error: a generated
/// file that does not compile under `-Werror` is a finding about this
/// column, and it has to reach the report rather than stopping the run.
fn compile(directory: &Path, probe: &Path, archive: &Path) -> Result<Result<(), String>, String> {
    let root = repository_root();
    let runtime = runtime_directory(&root);
    let here = root.join("tools").join("wireprobe");
    let output = Command::new(cxx())
        .args(cxxflags())
        .arg("-o")
        .arg(probe)
        .arg(directory.join("probe.cpp"))
        // `-I tools/wireprobe` is how `dumpjson.hpp` is found: the JSON
        // dialect is shared with every other probe and lives beside the
        // wrappers.
        .arg("-I")
        .arg(directory)
        .arg("-I")
        .arg(&here)
        .arg("-I")
        .arg(runtime.join("include"))
        .arg(archive)
        .arg("-lm")
        .output()
        .map_err(|error| format!("the C++ compiler could not be started: {error}"))?;
    if output.status.success() {
        return Ok(Ok(()));
    }
    let text = String::from_utf8_lossy(&output.stderr);
    let first = text
        .lines()
        .find(|line| line.contains("error") || line.contains("warning"))
        .unwrap_or("no diagnostic")
        .trim();
    // The build directory is named after the process, so leaving the
    // absolute path in would make two runs of the same corpus produce two
    // different reports and a diff between them meaningless. The
    // directory goes first, whatever file the compiler named, and the
    // `probe.cpp` line and column go after it, because those are a
    // property of the source this driver generated and not of the case.
    let without_directory = first.replace(&format!("{}/", directory.display()), "");
    let first = without_directory
        .split_once("probe.cpp:")
        .map_or(without_directory.as_str(), |(_, rest)| rest)
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == ':');
    Ok(Err(format!("!does not compile: {first}")))
}

/// The C++ compiler this driver drives.
fn cxx() -> String {
    std::env::var("CXX").unwrap_or_else(|_| "c++".to_string())
}

/// The warning set `runtime/cpp`'s Makefile compiles its own four sources
/// and its own two tests with, applied to the generated probe as well.
///
/// The same `-Werror` the C column compiles with, and for the same
/// reason: a generated file that warns is one whose warnings stop being
/// read. `-Wconversion` and `-Wsign-conversion` come with it because every
/// silent narrowing this column's C++ could commit is a byte on the wire
/// nobody asked for.
fn cxxflags() -> Vec<String> {
    std::env::var("CXXFLAGS").map_or_else(
        |_| {
            [
                "-std=c++17",
                "-O1",
                "-Wall",
                "-Wextra",
                "-Wpedantic",
                "-Wconversion",
                "-Wsign-conversion",
                "-Werror",
            ]
            .iter()
            .map(std::string::ToString::to_string)
            .collect()
        },
        |flags| flags.split_whitespace().map(str::to_string).collect(),
    )
}

fn first_line(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .next()
        .unwrap_or("the probe said nothing")
        .trim()
        .to_string()
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Where the C++ runtime lives.
fn runtime_directory(root: &Path) -> PathBuf {
    std::env::var_os("ZERODDS_CPP_RUNTIME")
        .map_or_else(|| root.join("runtime").join("cpp"), PathBuf::from)
}

/// `runtime/cpp`'s own sources, as `runtime/cpp/Makefile`'s `SOURCES`.
const RUNTIME_SOURCES: [&str; 4] = ["writer", "reader", "float128", "md5"];

/// `runtime/cpp`, compiled once into an archive every probe links.
///
/// The four translation units the runtime is, by its Makefile's `SOURCES`,
/// under the same warning set its own `make check` uses — so a runtime
/// that does not build stops this driver here rather than in twenty
/// thousand compiler runs.
fn build_runtime(root: &Path, build: &Path) -> Result<PathBuf, String> {
    let runtime = runtime_directory(root);
    let mut objects = Vec::new();
    for unit in RUNTIME_SOURCES {
        let object = build.join(format!("{unit}.o"));
        let output = Command::new(cxx())
            .args(cxxflags())
            .arg("-I")
            .arg(runtime.join("include"))
            .arg("-c")
            .arg(runtime.join("src").join(format!("{unit}.cpp")))
            .arg("-o")
            .arg(&object)
            .output()
            .map_err(|error| format!("the C++ compiler could not be started: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "runtime/cpp/src/{unit}.cpp does not compile:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        objects.push(object);
    }
    let archive = build.join("libzerodds_wire.a");
    let output = Command::new(std::env::var("AR").unwrap_or_else(|_| "ar".to_string()))
        .arg("rcs")
        .arg(&archive)
        .args(&objects)
        .output()
        .map_err(|error| format!("ar could not be started: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "the runtime archive was not made:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(archive)
}

/// Run `body` over `0..count` on `jobs` threads and collect the results.
///
/// A plain index queue rather than a dependency: the work items are
/// independent, the results carry their own index, and a work-stealing
/// pool would buy nothing over a shared counter for items this coarse.
fn each<T, F>(jobs: usize, count: usize, body: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync,
{
    let next = AtomicUsize::new(0);
    let collected: Mutex<Vec<T>> = Mutex::new(Vec::with_capacity(count));
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| {
                let mut mine = Vec::new();
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= count {
                        break;
                    }
                    mine.push(body(index));
                }
                if let Ok(mut collected) = collected.lock() {
                    collected.append(&mut mine);
                }
            });
        }
    });
    match collected.into_inner() {
        Ok(results) => results,
        // A poisoned mutex means a worker panicked. A partial matrix
        // reported as a whole one is the failure mode this driver exists to
        // prevent, so the process ends rather than the caller getting a
        // report with silent holes in it.
        Err(error) => {
            eprintln!("wireprobe-cpp-matrix: a worker panicked: {error}");
            std::process::exit(1);
        }
    }
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
        cases.push(Case {
            type_name: read("type")?.trim().to_string(),
            name,
            directory,
        });
    }
    Ok(cases)
}

fn repository_root() -> PathBuf {
    // `tools/wireprobe/wireprobe-zerodds-cpp-gen` is three below the root.
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
    let mut cache = None;
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
            "--cache" => cache = Some(PathBuf::from(next()?)),
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
        cache,
    })
}
