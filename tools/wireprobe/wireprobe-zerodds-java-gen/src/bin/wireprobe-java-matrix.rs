// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `wireprobe-java-matrix` — the Java column of the interop corpus,
//! whole.
//!
//! `tools/wireprobe/wireprobe-zerodds-java` runs one case in one
//! configuration per invocation, and each invocation launches the
//! generator, compiles `runtime/java`'s sixteen sources and the probe,
//! and throws all of it away.
//! `crates/wire-probe/tests/reference_columns.rs` wants this column over
//! the **whole** matrix — twelve configurations, five encodings, both byte
//! orders, four emit forms, 420 slots per case — and a per-case process
//! costs more than the measurement.
//!
//! This does the same work with five costs removed:
//!
//! 1. **The runtime is compiled once.** `runtime/java`'s sixteen sources
//!    and `tools/wireprobe/Zdj.java` become one class directory at
//!    start-up instead of being recompiled for every probe, and kept in
//!    the cache beside it. Not measured for this column.
//! 2. **The generator runs in process.** [`zerodds_wireprobe_java_gen`] is
//!    a library, so the corpus's tens of thousands of generations cost no
//!    process launches. Not measured for this column.
//! 3. **One program answers seven slots.** The byte order and the emit
//!    form are arguments of the compiled program rather than constants in
//!    it — [`zerodds_wireprobe_java_gen::combined_sources`] has the
//!    argument for both — and a JVM launch costs far more than a C
//!    process launch, so the form is chosen in one run:
//!
//!    ```text
//!    Probe batch <dir>    payload, payload+encapsulation and KeyHash in
//!                        both byte orders: five files
//!    Probe read le|be     the payload on standard input, the document on
//!                        standard output
//!    ```
//!
//!    Three launches answer a unit's seven slots, and not seven. How
//!    much a launch costs against a C process is **not measured** here.
//! 4. **A probe is compiled once ever.** What comes out of the compiler is
//!    kept under `target/wireprobe-java-cache`, keyed by the generated
//!    Java, the compiler and the runtime's sources. Not measured for this
//!    column.
//! 5. **Identical Java is compiled once.** Most of the matrix's
//!    configurations produce the same compilation unit for a given case —
//!    a `@final struct { long a; }` frames the same way under every
//!    dialect — so the sources are hashed and only the distinct ones reach
//!    the compiler. How many of this column's units are distinct is not
//!    measured.
//!
//! ## What it measures
//!
//! Per case and per configuration, in every encoding and both byte
//! orders:
//!
//! - the payload, as hex;
//! - the same behind its encapsulation header, with §7.6.3.1.2's count of
//!   the trailing padding in the header's options field;
//! - the KeyHash, which has no byte order of its own;
//! - the **read** direction: the Java probe's own payload handed back to
//!   the Java probe.
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
//! ## The vocabulary a refusal is written in
//!
//! A slot the Java column cannot answer is refused, never skipped and
//! never filled with a default, in the words the other two drivers and
//! this gate read:
//!
//! | prefix | what it means |
//! |---|---|
//! | `!generator:` | the generator refused the case, and its message is the answer |
//! | `!does not compile:` | the Java it generated is not accepted by `javac`, and the diagnostic is the answer |
//! | `!unwritten:` | nothing was written for a read slot to read |
//! | `!the probe …` | the program did not run, and what it said is the answer |
//!
//! `reference_columns.rs` compares the **class** of a refusal and reports
//! the sentence, so a cell this driver refuses with `does not compile` and
//! a cell the Rust column refuses with `unsupported` are one finding and
//! not two. A defect of the column is reported, not hidden behind a
//! bigger one: a unit whose **decoder** does not compile is retried
//! without it, so the payload it writes perfectly well is still written.
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
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::matrix::{CONFIGURATIONS, Configuration, ENCODINGS, ORDERS};
use zerodds_wire_probe::value::MapLengthPolicy;
use zerodds_wireprobe_java_gen::{
    Args, Emit, Sources, answer_stem, combined_sources, combined_sources_with, declares_a_key,
};

const USAGE: &str = "\
wireprobe-java-matrix — the Java column of the interop corpus, whole

    --out <file>        where the JSON goes (required)
    --case <name>       only this case, repeatable (default: all)
    --cases <file>      only the cases named in this file, one per line
    --jobs <n>          how many compilers to run at once
    --cache <dir>       where compiled probes are kept between runs
                        (default: target/wireprobe-java-cache)
    --config <name>     only this configuration, repeatable
    --keep              leave the build directory behind, for a look at
                        the Java a failing case produced
";

/// One measurement the driver has to make.
#[derive(Clone)]
struct Slot {
    /// Index into the case list.
    case: usize,
    /// The key the cell is filed under.
    key: String,
    /// What this slot is called inside its unit's answers: the byte
    /// order and the form, with the case, the configuration and the
    /// encoding stripped off because the unit already fixes those.
    suffix: String,
    /// Which compilation unit answers it.
    unit: Unit,
}

impl Slot {
    /// Whether this is a read slot, whose answer is a document rather
    /// than octets.
    fn read(&self) -> bool {
        self.suffix.ends_with("/read")
    }
}

/// What decides the compilation unit: a case, a configuration and an
/// encoding, and nothing else.
///
/// Not the byte order and not the emit form. Both are arguments of the
/// compiled program — see [`zerodds_wireprobe_java_gen::combined_sources`]
/// for why they can be.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Unit {
    configuration: usize,
    encoding: Encoding,
}

/// One corpus case, read off disk once.
///
/// Its **paths** and not their text: [`Args`] takes the IDL file and the
/// sample as paths, so the generator reads both and this driver holds
/// neither — there is no verdict here that would need either of them in
/// hand.
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
        eprintln!("wireprobe-java-matrix: {message}");
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

    let build = std::env::temp_dir().join(format!("wireprobe-java-matrix-{}", std::process::id()));
    std::fs::create_dir_all(&build).map_err(|error| format!("{}: {error}", build.display()))?;
    // Kept between runs, so a developer who runs this twice pays the
    // compiler once.
    let cache = Cache::open(
        options
            .cache
            .clone()
            .unwrap_or_else(|| root.join("target").join("wireprobe-java-cache")),
        &runtime_directory(&root),
        &root.join("tools").join("wireprobe").join("Zdj.java"),
    )?;
    // A runtime that does not build stops this driver here rather than in
    // twenty thousand compiler runs.
    let runtime = build_runtime(&cache)?;

    let slots = plan(&cases, &options.configurations);
    // One compilation unit per case, configuration and encoding: a fifth
    // of the slots, because the byte order and the emit form are the
    // compiled program's arguments.
    let units: Vec<(usize, Unit)> = {
        let mut seen: Vec<(usize, Unit)> = Vec::new();
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
        "wireprobe-java-matrix: {} case(s), {} slot(s), {} unit(s), {} job(s)",
        cases.len(),
        slots.len(),
        units.len(),
        options.jobs
    );

    let started = std::time::Instant::now();

    // Pass one: generate every unit's Java and keep only its digest, so
    // that the corpus's tens of thousands of compilation units never sit
    // in memory at once.
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

    // One compilation per distinct unit, and one run per distinct unit
    // answering every slot it covers. Two configurations that produced
    // the same Java produce the same octets, which is what makes
    // attributing one run to both sound.
    let mut representative: BTreeMap<u128, usize> = BTreeMap::new();
    for (index, outcome) in digests.iter().enumerate() {
        if let Ok(digest) = outcome {
            representative.entry(*digest).or_insert(index);
        }
    }
    let distinct: Vec<(u128, usize)> = representative.iter().map(|(a, b)| (*a, *b)).collect();
    eprintln!(
        "wireprobe-java-matrix: {} distinct compilation unit(s) to compile after {:.0}s",
        distinct.len(),
        started.elapsed().as_secs_f64()
    );

    let done = AtomicUsize::new(0);
    let measured = each(options.jobs, distinct.len(), |index| {
        let (digest, unit_index) = distinct[index];
        let (case, unit) = units[unit_index];
        let outcome = measure(&cases[case], unit, &runtime, &cache, &build);
        let seen = done.fetch_add(1, Ordering::Relaxed) + 1;
        if seen % 500 == 0 {
            eprintln!(
                "wireprobe-java-matrix: {seen}/{} unit(s) after {:.0}s",
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
                .and_then(|answers| answers.get(&slot.suffix))
                .unwrap_or(&unreachable)
                .as_str(),
            // A unit whose Java could not be generated has no probe at
            // all, so every slot it would have answered carries the
            // generator's reason — and its read slots carry `unwritten`,
            // because nothing was written for them to read.
            // `refused_everywhere` has the argument.
            Err(message) if slot.read() => {
                unwritten = format!("!unwritten: {}", message.trim_start_matches('!'));
                unwritten.as_str()
            }
            // A keyless type has no KeyHash whatever stopped its unit, so
            // its KeyHash slot says that and not the unit's reason
            // (`declares_a_key`).
            Err(message) if slot.suffix == "keyhash" => {
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
    // and C++ drivers use: a run killed while writing leaves the
    // temporary behind and whatever was there before intact, where a
    // direct write leaves a truncated file that still parses as JSON and
    // reads as a shorter matrix — a wrong answer with no error, in the
    // tool whose whole job is to catch those.
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
        "wireprobe-java-matrix: {} slot(s) in {:.0}s to {}",
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
                for (order_name, _) in ORDERS {
                    let stem = format!("{}/{encoding_name}/{order_name}", settings.name);
                    // One tail for both names: the bare payload is that
                    // stem and the other two are the stem with their own
                    // tail, so a key and the name the unit files it under
                    // cannot part.
                    for tail in ["", "+enc", "/read"] {
                        slots.push(Slot {
                            case,
                            key: format!("{stem}{tail}"),
                            suffix: format!("{order_name}{tail}"),
                            unit,
                        });
                    }
                }
                // The KeyHash holder is big-endian whatever the sample is
                // — XTypes 1.3 §7.6.8 — so it has no byte-order axis.
                slots.push(Slot {
                    case,
                    key: format!("{}/{encoding_name}/keyhash", settings.name),
                    suffix: "keyhash".to_string(),
                    unit,
                });
            }
        }
    }
    slots
}

/// The generator's arguments for one compilation unit.
///
/// `endianness` and `emit` are set because [`Args`] has the fields;
/// `combined_sources` ignores both, which is what lets one unit answer
/// for every byte order and every form.
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

/// A 128-bit digest of one compilation unit.
///
/// Two independent 64-bit hashes rather than one: the driver attributes a
/// compiled probe's octets to every slot with the same digest, so a
/// collision would report one case's octets as another's. `SipHash` and
/// not a cryptographic hash because nothing here is adversarial — the
/// inputs are the corpus.
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
/// The digest of the Java, the compiler's identity and the runtime's
/// sources — anything that decides the class files, and nothing about
/// where they will run. A key that missed one of the three would hand
/// back a probe built by another compiler or against another runtime.
struct Cache {
    root: PathBuf,
    /// What the compiler and the runtime contribute to every key.
    toolchain: u64,
}

impl Cache {
    /// The cache under `root`, fingerprinted against this toolchain.
    ///
    /// The runtime's **sources** and not the classes built from them:
    /// a class file carries the compiler's own version in its header and
    /// nothing here knows it, so two builds of identical sources from two
    /// compilers differ and every key would miss.
    fn open(root: PathBuf, runtime: &Path, zdj: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|error| format!("{}: {error}", root.display()))?;
        let javac = javac();
        let version = Command::new(&javac)
            .arg("--version")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default();
        let mut hasher = std::hash::DefaultHasher::new();
        version.hash(&mut hasher);
        javacflags().hash(&mut hasher);
        zdjflags().hash(&mut hasher);
        let mut files = Vec::new();
        files_under(&wire_sources(runtime), &mut files)?;
        // `Zdj.java` is the canonical-JSON dialect and not the wire
        // runtime, and the read direction compiles against it, so it is
        // part of what decides a probe's class files.
        files.push(zdj.to_path_buf());
        for file in files {
            // The path from the repository root and not the leaf: two
            // files of one name under different directories must not hash
            // alike, and a source removed from the runtime changes what
            // links even when nothing that is left changed.
            file.strip_prefix(repository_root())
                .unwrap_or(&file)
                .hash(&mut hasher);
            std::fs::read(&file)
                .map_err(|error| format!("{}: {error}", file.display()))?
                .hash(&mut hasher);
        }
        Ok(Self {
            root,
            toolchain: hasher.finish(),
        })
    }

    /// The class directory one variant of one unit is kept in.
    fn classes(&self, digest: u128, variant: &str) -> PathBuf {
        self.root
            .join(format!("{digest:032x}-{:016x}-{variant}", self.toolchain))
    }

    /// The file one variant's compile failure is kept in.
    fn failure(&self, digest: u128, variant: &str) -> PathBuf {
        self.classes(digest, variant).with_extension("err")
    }

    /// The runtime's own classes, compiled once.
    fn runtime(&self) -> PathBuf {
        self.root.join(format!("runtime-{:016x}", self.toolchain))
    }
}

/// The one directory of `runtime/java` a probe compiles against.
///
/// `org/zerodds/wire` and nothing else, for the reason
/// `tools/wireprobe/wireprobe-zerodds-java` gives at length: the tree also
/// carries `org/omg/dds/rpc`, which is the OMG RPC PSM rather than the
/// wire runtime, and six of its files import `org.omg.dds.core`, a package
/// that lives in `crates/java-omgdds/java` behind its own `pom.xml`. A
/// walk over the whole directory pulled those in and every probe failed
/// with "runtime/java did not compile" — measured, four wire-probe test
/// targets red.
fn wire_sources(runtime: &Path) -> PathBuf {
    runtime
        .join("src")
        .join("main")
        .join("java")
        .join("org")
        .join("zerodds")
        .join("wire")
}

/// Every file under `directory`, and under the directories below it.
///
/// Recursive so that [`wire_sources`] can name the package's own
/// directory rather than every file in it: a source added to
/// `org/zerodds/wire` is picked up without a second list here going out
/// of step.
///
/// # Errors
/// A message naming the directory that could not be read.
///
/// zerodds-lint: recursion-depth 3 — one level per directory. The walk
/// cannot cycle: a filesystem entry below a directory is reached once,
/// and `is_dir` follows no symbolic link back up.
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

/// `runtime/java`'s own sources, compiled once into the cache.
///
/// The `org/zerodds/wire` set `runtime/java/run-tests.sh` compiles by its
/// own glob, under that script's warning set, and `Zdj.java` beside them
/// under its own — so a runtime that does not build stops this driver here
/// rather than in twenty thousand compiler runs.
///
/// The two sets are compiled in two invocations and not one because
/// [`zdjflags`] differs from [`javacflags`], which is the whole of the
/// difference: `Zdj` builds its `PrintStream` with the
/// `(OutputStream, boolean, Charset)` constructor, which arrived in Java
/// 10, and `--release 8` refuses it.
///
/// # Errors
/// A message naming the compiler's own output.
fn build_runtime(cache: &Cache) -> Result<PathBuf, String> {
    let root = repository_root();
    let runtime = runtime_directory(&root);
    let classes = cache.runtime();
    // Every source is where it says it is: a runtime that lost a file
    // would otherwise compile into a class directory with a class
    // missing and every probe would fail at run time with a message
    // about a method.
    let mut files = Vec::new();
    files_under(&wire_sources(&runtime), &mut files)?;
    files.retain(|file| {
        file.extension()
            .is_some_and(|extension| extension == "java")
    });
    if files.is_empty() {
        return Err(format!("{}: no runtime sources", runtime.display()));
    }
    let zdj = root.join("tools").join("wireprobe").join("Zdj.java");
    if classes.join("Zdj.class").is_file()
        && files.iter().all(|file| class_of(file, &classes).is_file())
    {
        return Ok(classes);
    }

    // The name carries this process's id, so two drivers filling the same
    // cache and a directory left by a run that was killed are each the
    // one to clear.
    let staging = cache.root.join(format!(
        "runtime-{:016x}.staging-{}",
        cache.toolchain,
        std::process::id()
    ));
    // Built beside the cache rather than in the system temp directory, so
    // the rename into place is a rename and not a copy across
    // filesystems.
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    std::fs::create_dir_all(&staging).map_err(|error| format!("{}: {error}", staging.display()))?;
    let outcome = compile_runtime(&javac(), javacflags(), &files, &staging)
        .and_then(|()| compile_runtime(&javac(), zdjflags(), std::slice::from_ref(&zdj), &staging));
    if let Err(message) = outcome {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(message);
    }
    let _ = std::fs::remove_dir_all(&classes);
    std::fs::rename(&staging, &classes)
        .map_err(|error| format!("{}: {error}", classes.display()))?;
    Ok(classes)
}

/// One `javac` run over the runtime, whose failure is the driver's and
/// not a column's.
fn compile_runtime(
    javac: &str,
    flags: Vec<String>,
    files: &[PathBuf],
    classes: &Path,
) -> Result<(), String> {
    let output = Command::new(javac)
        .args(flags)
        .arg("-d")
        .arg(classes)
        .args(files)
        .output()
        .map_err(|error| format!("the Java compiler could not be started: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "runtime/java does not compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Where one runtime source's class file lands.
fn class_of(source: &Path, classes: &Path) -> PathBuf {
    let name = source
        .file_stem()
        .map(|stem| format!("{}.class", stem.to_string_lossy()))
        .unwrap_or_default();
    classes.join("org").join("zerodds").join("wire").join(name)
}

/// A probe that is ready to run, or the reason there is none.
enum Built {
    /// The class directory, and whether its read direction is in it.
    Probe { classes: PathBuf, reads: bool },
    /// Neither variant compiled.
    Broken(String),
}

/// Compile one unit — or find it already compiled — and run it in every
/// form and byte order.
///
/// One compile, one batch run and two read runs on a cold cache,
/// answering all seven slots the unit covers.
fn measure(
    case: &Case,
    unit: Unit,
    runtime: &Path,
    cache: &Cache,
    work: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let digest = digest(&source_for(case, unit)?);
    // A unit whose decoder does not compile is retried without it. One
    // broken form must not take the other three with it: a column that
    // wrote a payload perfectly well and only failed to compile its
    // *reader* would understate itself and hide the actual defect behind a
    // bigger one.
    let built = match build(case, unit, runtime, cache, digest, true)? {
        Built::Probe { classes, .. } => Built::Probe {
            classes,
            reads: true,
        },
        Built::Broken(message) => match build(case, unit, runtime, cache, digest, false)? {
            Built::Probe { classes, .. } => Built::Probe {
                classes,
                reads: false,
            },
            // Both variants failed, and the one that names the defect is
            // the full one: the fallback fails for the same reason plus
            // whatever else.
            Built::Broken(_) => Built::Broken(message),
        },
    };

    let mut answers = BTreeMap::new();
    let (classes, read_failure) = match &built {
        Built::Broken(message) => return Ok(refused_everywhere(message)),
        Built::Probe { classes, reads } => (
            classes.as_path(),
            // The message the full variant gave, recovered from the cache
            // so a warm run reports the same defect a cold one did rather
            // than a vaguer one.
            if *reads {
                None
            } else {
                Some(
                    std::fs::read_to_string(cache.failure(digest, "full"))
                        .unwrap_or_else(|_| "!does not compile".to_string()),
                )
            },
        ),
    };

    // A counter and not the case's name: two threads answering two units
    // of the same case in the same byte order would otherwise share one
    // directory and read each other's answers.
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = work.join(format!("answers-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    if let Err(message) = run_java(classes, runtime, "batch", Some(&directory), None) {
        // The program did not run at all, so nothing was written and
        // nothing can be read: every slot carries what it said, and the
        // read slots say `unwritten`, which is what happened to them.
        let _ = std::fs::remove_dir_all(&directory);
        return Ok(refused_everywhere(&message));
    }

    // The five octet answers, and the refusal of each.
    let mut written: BTreeMap<String, Result<Vec<u8>, String>> = BTreeMap::new();
    for slot in ["le", "be", "le+enc", "be+enc", "keyhash"] {
        let octets = directory.join(format!("{}.bin", answer_stem(slot)));
        if octets.is_file() {
            written.insert(
                slot.to_string(),
                std::fs::read(&octets).map_err(|error| format!("{error}")),
            );
            continue;
        }
        let refusal = directory.join(format!("{}.err", answer_stem(slot)));
        written.insert(
            slot.to_string(),
            Err(match std::fs::read_to_string(&refusal) {
                Ok(message) => format!("!{}", message.trim()),
                // No file and no sentence: the program wrote neither, and
                // a slot answered with nothing is a hole in the report.
                Err(_) => format!("!no answer for `{slot}` and no sentence for one"),
            }),
        );
    }
    for (slot, outcome) in &written {
        answers.insert(
            slot.clone(),
            match outcome {
                Ok(octets) => hex(octets),
                Err(message) => message.clone(),
            },
        );
    }

    // And the two read directions, each its own launch, because the
    // document is the one answer that cannot go to a file: the JSON
    // dialect in `tools/wireprobe/Zdj.java` writes to
    // `FileDescriptor.out`.
    for (order_name, _) in ORDERS {
        let payload = directory.join(format!("{}.bin", answer_stem(order_name)));
        let unwritten = match &written[order_name] {
            Ok(_) => None,
            Err(message) => Some(message.trim_start_matches('!').to_string()),
        };
        answers.insert(
            format!("{order_name}/read"),
            match (&read_failure, &unwritten, payload.is_file()) {
                (Some(message), _, _) => message.clone(),
                // Nothing was written, so there is nothing to read. Told
                // apart from a reader that failed: the two are different
                // findings.
                (None, Some(message), _) => format!("!unwritten: {message}"),
                (None, None, true) => read_back(case, classes, runtime, order_name, &payload),
                (None, None, false) => {
                    "!unwritten: the batch run wrote no payload to read".to_string()
                }
            },
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
    Ok(answers)
}

/// One variant of one unit, from the cache or from the compiler.
fn build(
    case: &Case,
    unit: Unit,
    runtime: &Path,
    cache: &Cache,
    digest: u128,
    with_read: bool,
) -> Result<Built, String> {
    let variant = if with_read { "full" } else { "noread" };
    let classes = cache.classes(digest, variant);
    if classes.join("Probe.class").is_file() {
        return Ok(Built::Probe {
            classes,
            reads: with_read,
        });
    }
    let failure = cache.failure(digest, variant);
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
    let built = directory.join("classes");
    std::fs::create_dir_all(&built).map_err(|error| format!("{}: {error}", built.display()))?;
    std::fs::write(directory.join("ZdTypes.java"), &sources.types)
        .map_err(|error| format!("{error}"))?;
    std::fs::write(directory.join("Probe.java"), &sources.probe)
        .map_err(|error| format!("{error}"))?;
    let outcome = compile(&directory, &built, runtime)?;
    match outcome {
        Ok(()) => {
            // Losing the race is not an error: the other builder produced
            // the same classes from the same inputs, which is what the key
            // says. Either way the build directory goes, and the probe
            // that answers is the one in the cache.
            let _ = std::fs::remove_dir_all(&classes);
            let _ = std::fs::rename(&built, &classes);
            let _ = std::fs::remove_dir_all(&directory);
            Ok(Built::Probe {
                classes,
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
/// because that is what happened to them: no octets were produced, so
/// there was nothing to read and the reader was never asked. The other
/// two columns mark theirs the same way, and a gate that compares the
/// *class* of a refusal would otherwise read "this reader refused" where
/// the truth is "this reader was never reached".
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

/// `javac` over the probe, against the runtime compiled once.
///
/// The outer `Result` is the driver failing; the inner one is the probe
/// failing to compile, which is a verdict and not an error: a generated
/// file that `javac` refuses is a finding about this column, and it has
/// to reach the report rather than stopping the run.
fn compile(directory: &Path, classes: &Path, runtime: &Path) -> Result<Result<(), String>, String> {
    let output = Command::new(javac())
        .args(javacflags())
        .arg("-cp")
        .arg(runtime)
        .arg("-d")
        .arg(classes)
        .arg(directory.join("ZdTypes.java"))
        .arg(directory.join("Probe.java"))
        .output()
        .map_err(|error| format!("the Java compiler could not be started: {error}"))?;
    if output.status.success() {
        return Ok(Ok(()));
    }
    let text = String::from_utf8_lossy(&output.stderr);
    // The line naming a generated file, and not the line saying "error":
    // `javac` writes its diagnostics in the locale it was started in, so
    // a German JDK says `Fehler:` where an English one says `error:` —
    // measured here with javac 21.0.10 under a German locale, where the
    // C++ driver's own `error | warning` test finds nothing at all. The
    // file name is in the line in every locale.
    let first = text
        .lines()
        .find(|line| line.contains("Probe.java") || line.contains("ZdTypes.java"))
        .unwrap_or("no diagnostic")
        .trim();
    // The build directory is named after the process, so leaving the
    // absolute path in would make two runs of the same corpus produce two
    // different reports and a diff between them meaningless. The
    // directory goes first, whatever file the compiler named, and the
    // `Probe.java` line and column go after it, because those are a
    // property of the source this driver generated and not of the case.
    let without_directory = first.replace(&format!("{}/", directory.display()), "");
    let first = without_directory
        .split_once("Probe.java:")
        .map_or(without_directory.as_str(), |(_, rest)| rest)
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == ':');
    Ok(Err(format!("!does not compile: {first}")))
}

/// One run of the compiled probe.
///
/// `Err` carries the refusal, already marked with the `!` the report
/// files a refusal under. `argument` is the form's own second word — a
/// byte order for `read`, the directory to write the answers into for
/// `batch` — and `input` is what the read direction is handed on standard
/// input.
fn run_java(
    classes: &Path,
    runtime: &Path,
    form: &str,
    argument: Option<&Path>,
    input: Option<&Path>,
) -> Result<Vec<u8>, String> {
    // The probe's own classes and the runtime it was compiled against:
    // with the probe's alone every launch ended in
    // `NoClassDefFoundError: org/zerodds/wire/WireException`, measured on
    // the Linux test host as all 176,400 slots of the column.
    let classpath = std::env::join_paths([classes, runtime])
        .map_err(|error| format!("!the class path cannot be formed: {error}"))?;
    let mut command = Command::new(java());
    command.args(jvmflags());
    command.arg("-cp").arg(classpath).arg("Probe").arg(form);
    if let Some(argument) = argument {
        command.arg(argument);
    }
    match input {
        Some(path) => {
            let file = std::fs::File::open(path).map_err(|error| {
                format!("!the payload could not be handed to the probe: {error}")
            })?;
            command.stdin(Stdio::from(file));
        }
        // Nothing to read: an inherited standard input would leave the
        // program waiting on whatever the driver was run from.
        None => {
            command.stdin(Stdio::null());
        }
    }
    let outcome = command.output();
    match outcome {
        Ok(output) if output.status.success() => Ok(output.stdout),
        Ok(output) => Err(format!("!{}", first_line(&output.stderr))),
        Err(error) => Err(format!("!the probe could not be started: {error}")),
    }
}

/// The read direction: the Java probe's own octets back into the Java
/// probe.
///
/// Its own octets, because a column that reads another's says the two
/// agree and not that this one is self-consistent. What comes back is
/// filed as the document it is — `json:` and the probe's own JSON — and
/// this driver reaches no verdict about it: the comparison belongs to
/// `crates/wire-probe/tests/reference_columns.rs`, which canonicalises it
/// against the case's own sample.
///
/// The JSON is parsed here for one reason and one only: a document that
/// does not parse is reported as such instead of being handed on for a
/// comparison to fail somewhere else.
fn read_back(case: &Case, classes: &Path, runtime: &Path, order: &str, payload: &Path) -> String {
    let octets = match run_java(
        classes,
        runtime,
        "read",
        Some(Path::new(order)),
        Some(payload),
    ) {
        Ok(octets) => octets,
        Err(message) => return message,
    };
    let document = String::from_utf8_lossy(&octets).trim().to_string();
    if let Err(error) = serde_json::from_str::<serde_json::Value>(&document) {
        // `case` is in the sentence because the same shape of document is
        // wrong for one case and right for another.
        return format!("!{}: the probe's JSON does not parse: {error}", case.name);
    }
    format!("json:{document}")
}

/// The Java compiler this driver drives.
fn javac() -> String {
    std::env::var("JAVAC").unwrap_or_else(|_| "javac".to_string())
}

/// The JVM this driver drives.
fn java() -> String {
    std::env::var("JAVA").unwrap_or_else(|_| "java".to_string())
}

/// The warning set `runtime/java/run-tests.sh` holds its own sixteen
/// sources and its own three tests to, applied to the generated probe as
/// well.
///
/// The same `-Werror` the C and C++ columns compile with, and for the same
/// reason: a generated file that warns is one whose warnings stop being
/// read. `--release 8` because the runtime targets Java 8 and nothing
/// here needs more, and `-options` is subtracted from the lint set
/// because javac 21 warns that release 8 is obsolete, which is a fact
/// about the compiler and not about this code.
fn javacflags() -> Vec<String> {
    std::env::var("JAVACFLAGS").map_or_else(
        |_| {
            [
                "--release",
                "8",
                "-Xlint:all,-options",
                "-Werror",
                "-encoding",
                "UTF-8",
            ]
            .iter()
            .map(std::string::ToString::to_string)
            .collect()
        },
        |flags| flags.split_whitespace().map(str::to_string).collect(),
    )
}

/// [`javacflags`] without `--release 8`, for `tools/wireprobe/Zdj.java`.
///
/// That one file cannot be compiled for release 8: its `PrintStream` is
/// built with the `(OutputStream, boolean, Charset)` constructor, which
/// arrived in Java 10 — measured with javac 21.0.10, which reports
/// `incompatible types: Charset cannot be converted to String` at
/// `Zdj.java:33`. It is the JSON dialect and not the wire runtime,
/// `wireprobe-zerodds-java` compiles it without `--release` for the same
/// reason, and the class it leaves is read by a release-8 compile without
/// complaint.
fn zdjflags() -> Vec<String> {
    std::env::var("ZDJ_JAVACFLAGS").map_or_else(
        |_| {
            ["-Xlint:all,-options", "-Werror", "-encoding", "UTF-8"]
                .iter()
                .map(std::string::ToString::to_string)
                .collect()
        },
        |flags| flags.split_whitespace().map(str::to_string).collect(),
    )
}

/// The flags every JVM in this driver is launched with.
///
/// The three `wireprobe-zerodds-java` uses, for the reason that script
/// gives: the JVM writes its own warnings to standard output unless told
/// otherwise, and on the read direction standard output **is** the
/// document. Measured in that column's matrix refresh of 2026-10-01: a
/// `Cannot use file … because it is locked by another process` warning
/// from `hsperfdata` became the first 133 octets of a payload. The flags
/// drop that file and send any other warning to standard error, away
/// from the octets.
fn jvmflags() -> Vec<String> {
    std::env::var("JVM_FLAGS").map_or_else(
        |_| {
            [
                "-XX:-UsePerfData",
                "-Xlog:disable",
                "-Xlog:all=warning:stderr",
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

/// Where the Java runtime lives.
fn runtime_directory(root: &Path) -> PathBuf {
    std::env::var_os("ZERODDS_JAVA_RUNTIME")
        .map_or_else(|| root.join("runtime").join("java"), PathBuf::from)
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
        // reported as a whole one is the failure mode this driver exists
        // to prevent, so the process ends rather than the caller getting a
        // report with silent holes in it.
        Err(error) => {
            eprintln!("wireprobe-java-matrix: a worker panicked: {error}");
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
    // `tools/wireprobe/wireprobe-zerodds-java-gen` is three below the root.
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
