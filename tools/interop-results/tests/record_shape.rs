// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! One build per case per system, asserted rather than remembered.
//!
//! `check` reads what is committed and takes milliseconds. `record`
//! needs the vendor toolchains, and what it costs is decided by one
//! thing: whether the encoding version and the byte order are part of a
//! probe binary's identity or arguments to it.
//!
//! They used to be `sed` substitutions. A probe template carried
//! `@BIG_ENDIAN@` and `@VERSION@`, the shell put the run's values in
//! before the vendor's code generator ran, and the content-keyed cache
//! therefore held one binary per *cell*. Over 335 cases, four
//! combinations and nine systems that is roughly fifteen thousand runs
//! of a code generator and a compiler to answer fifteen thousand
//! questions that 3,015 builds can answer — and it is why a sweep took
//! hours.
//!
//! Every one of them is an argument now, so the ratio is one build per
//! case per system per direction. The three tests below are what keeps
//! it there: a template that reintroduces a compile-time byte order
//! would pass every byte comparison in this crate — the bytes would be
//! right — and quadruple the cost of recording them. Nothing else would
//! notice.
//!
//! One build for the *whole corpus* per system was measured and is not
//! reachable: 307 of the 335 cases declare a top-level type named
//! `Sample`, and 335 IDL files that collide on eight distinct names
//! cannot become one translation unit without renaming the types, which
//! changes the scoped name every vendor's generated code is keyed by.
//! [`the_corpus_cannot_be_one_translation_unit`] records that
//! measurement so the option is not re-argued from memory.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use zerodds_interop_results as results;

fn wireprobe(root: &Path) -> PathBuf {
    root.join("tools").join("wireprobe")
}

/// Every marker that would make a wire setting a compile-time decision.
///
/// `@TYPE@`, `@PACKAGE@` and the rest are not here: the *type* is what a
/// generated probe is legitimately built for, and one build per case is
/// the target, not one build for the corpus.
const SETTING_MARKERS: [&str; 6] = [
    "@BIG_ENDIAN@",
    "@VERSION@",
    "@REPRESENTATION@",
    "@ENDIANNESS@",
    "@STREAM@",
    "@WITH_ENCAPSULATION@",
];

/// No probe template decides a byte order or an encoding version before
/// the compiler runs.
#[test]
fn a_wire_setting_is_never_a_compile_time_substitution() {
    let directory = wireprobe(&results::repository_root());
    let mut offenders: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(&directory).expect("tools/wireprobe");
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let is_template = ["template.c", "template.cpp", "template.java"]
            .iter()
            .any(|suffix| name.ends_with(suffix));
        if !is_template {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for marker in SETTING_MARKERS {
            if text.contains(marker) {
                offenders.push(format!("{name} carries {marker}"));
            }
        }
        // The JacORB templates are spliced by a Python here-document
        // rather than by `sed`, with its own marker syntax.
        if text.contains("#LITTLE_ENDIAN#") {
            offenders.push(format!("{name} carries #LITTLE_ENDIAN#"));
        }
    }
    assert!(
        offenders.is_empty(),
        "a wire setting is decided before the compiler runs, which makes it part of the \
         binary's identity and multiplies every sweep by the number of combinations:\n{}",
        offenders.join("\n")
    );
}

/// No probe's cache key names a byte order or an encoding version.
///
/// The templates and the keys have to agree. A key that still carried
/// the byte order after the template stopped doing so would keep
/// building one binary per cell out of a binary that answers all of
/// them — the slow half of the old arrangement with none of its reason.
#[test]
fn a_cache_key_never_names_a_wire_setting() {
    let directory = wireprobe(&results::repository_root());
    let mut offenders: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(&directory).expect("tools/wireprobe");
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if !name.starts_with("wireprobe-") || path.is_dir() {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for line in text.lines() {
            let Some(rest) = line.split_once("probe_cache_key").map(|(_, rest)| rest) else {
                continue;
            };
            // Only the arguments before `--`; after it are the files
            // whose *content* is hashed, which is the point of the key.
            let arguments = rest.split(" -- ").next().unwrap_or_default();
            for setting in [
                "$big",
                "$version",
                "$representation",
                "$endianness",
                "$stream",
            ] {
                if arguments.contains(setting) {
                    offenders.push(format!("{name}: a cache key names {setting}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a probe binary's cache key names a wire setting, so the cache holds one build per \
         cell rather than one per case:\n{}",
        offenders.join("\n")
    );
}

/// The corpus sweeps ask each probe once per case, with `--matrix`.
///
/// The cache makes one build per case *likely*; `--matrix` makes it
/// true. A sweep run with `WIREPROBE_CACHE` unset — which is the
/// default — would otherwise be back to one build per cell without
/// anything having changed in the tree.
#[test]
fn the_sweeps_ask_once_per_case() {
    let root = results::repository_root();
    for (relative, probes) in [
        (
            "tools/interop-results/vendor-corpus-sweep.sh",
            &["--matrix"][..],
        ),
        (
            "tools/interop-results/ros-corpus-sweep.sh",
            &["--matrix"][..],
        ),
        ("tools/corba-oracle/corba-sweep.sh", &["--matrix"][..]),
    ] {
        let path = root.join(relative);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for wanted in probes {
            assert!(
                text.contains(wanted),
                "{relative} does not pass `{wanted}`, so it asks a probe once per cell and \
                 pays for a build each time"
            );
        }
        assert!(
            text.contains("probe builds for"),
            "{relative} does not report its build count; the ratio of builds to cells is the \
             number that says whether the loop is right, and a number nobody prints is a \
             number nobody checks"
        );
    }
}

/// The corpus is one translation unit per system, and this is what it
/// costs to make it one.
///
/// 307 of the 335 cases declare a top-level type called `Sample` and 18
/// distinct names cover all of them, so concatenating the files is 307
/// redefinitions. `wireprobe-batch` wraps each case in a module of its
/// own to get past that, which changes the type's **scoped name**.
///
/// This test is what says the count is still what the wrapping is for.
/// If every case ever gets a distinct type name the wrapping becomes
/// unnecessary, and a reader should find that out here rather than from
/// a comment that was true once.
#[test]
fn the_corpus_needs_a_module_per_case_to_be_one_translation_unit() {
    let cases = results::corpus(&results::repository_root()).expect("the corpus");
    let distinct: BTreeSet<&str> = cases.iter().map(|case| case.type_name.as_str()).collect();
    assert!(
        distinct.len() < cases.len(),
        "{} cases now have {} distinct top-level type names; the module-per-case wrapping in \
         wireprobe-batch exists only because they collide, and it can go",
        cases.len(),
        distinct.len()
    );
}

/// Every batch driver keeps the three outcomes apart, and none of them
/// can report an empty column as a result.
///
/// A batch is one program for 335 cases, so the failure it invites is
/// the one the per-case arrangement could not have: one bad case taking
/// the other 334 down with it, or a run that produced nothing at all
/// being written out as a column of refusals. Four things are asserted
/// against the driver, because each of them was a defect in it before
/// it was a line of code:
///
///  * it retries an excluded case on its own. A case can be excluded
///    for a reason that is not its own — `module-root-anchored` says
///    `::common::util::Location`, and inside `module zd_c7 { … }` that
///    path does not exist — and four cases were refused by the batch
///    and accepted alone;
///  * its driver is line buffered, so a case that kills the process
///    cannot take the rows of the cases before it with it;
///  * it stops rather than blaming a case it cannot identify;
///  * a column with no bytes at all is a harness failure and exits
///    non-zero.
#[test]
fn the_batch_driver_keeps_the_three_outcomes_apart() {
    let root = results::repository_root();
    let driver = std::fs::read_to_string(wireprobe(&root).join("wireprobe-batch"))
        .expect("tools/wireprobe/wireprobe-batch");
    for (needle, why) in [
        (
            "retried alone",
            "an excluded case is never retried with the per-case probe, so a case the module \
             wrapping broke is recorded as the vendor refusing it",
        ),
        (
            "produced no bytes and nothing reached it",
            "a column with nothing in it and nothing attributable to the vendor is not \
             reported as a harness failure",
        ),
        (
            "so the run measured them",
            "a run in which every case carries the vendor's own refusal is thrown away as a \
             harness failure — which is what recorded `not-measured` for the six \
             `identifier-target-keyword-*` cases Fast DDS had answered, each with \
             fastddsgen's or g++'s own text",
        ),
        (
            "blamed no case; splitting",
            "a failure it cannot attribute does not split the chunk, so it would either stop \
             on a whole sixtieth of the corpus or guess which case to drop — `fastddsgen` \
             fails on the full corpus with a stack trace carrying no line number, and there \
             is nothing in it to blame",
        ),
        (
            "toolchain failed on this case alone",
            "a chunk that has been split down to one case has nowhere left to go, and the \
             failure is not recorded against that case with the toolchain's own text",
        ),
    ] {
        assert!(driver.contains(needle), "wireprobe-batch: {why}");
    }
    for template in [
        "cyclone-batch-template.c",
        "fastdds-batch-template.cpp",
        "opendds-batch-template.cpp",
    ] {
        let text = std::fs::read_to_string(wireprobe(&root).join(template))
            .unwrap_or_else(|error| panic!("{template}: {error}"));
        assert!(
            text.contains("_IOLBF"),
            "{template} is not line buffered; a case that kills the process would take the rows \
             of every case before it, and the driver reads those rows to decide which case to \
             blame"
        );
        assert!(
            text.contains("\\t!"),
            "{template} has no refusal row; a combination the vendor would not take has to be a \
             row that says so, not an absent line"
        );
    }
}

/// A subset re-record keeps every row it did not measure.
///
/// The dangerous shape is not a wrong row, it is a missing file: the
/// sweep writes one row per (case, vendor, encoding, order) and used to
/// `mv` its output over the whole source. A `--cases` run that kept that
/// shape would delete every case it was not asked about, and nothing
/// downstream would notice — `interop-results --verify` compares the
/// tree against the sources, and both would have shrunk together.
///
/// So this drives the worst case on purpose: a sweep asked for one case
/// with **no probe available**, which measures nothing at all. Before
/// the merge existed that run emptied the file. The rows that must
/// survive are every other case's, and the same case's other vendors'.
#[test]
fn a_subset_sweep_does_not_delete_the_cases_it_was_not_asked_about() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().expect("the checkout");
    let work = std::env::temp_dir().join(format!("zd-cases-{}", std::process::id()));
    let corpus = work.join("corpus");
    for case in ["kept-case", "swept-case"] {
        std::fs::create_dir_all(corpus.join(case)).expect("the fake corpus");
        std::fs::write(
            corpus.join(case).join("type.idl"),
            "@final struct Sample { long a; };",
        )
        .expect("the fake IDL");
    }
    let out = work.join("vendor-corpus.tsv");
    let before = "\
kept-case\tcyclone\txcdr1\tle\twrote\tdead\n\
swept-case\tcyclone\txcdr1\tle\twrote\tbeef\n\
swept-case\tfastdds\txcdr1\tle\twrote\tcafe\n";
    std::fs::write(&out, before).expect("the previous run");

    // No probe directory, so the sweep measures nothing. That is the
    // point: an empty measurement must not be an empty file.
    let run = std::process::Command::new("bash")
        .arg(root.join("tools/interop-results/vendor-corpus-sweep.sh"))
        .args(["--vendors", "cyclone", "--cases", "swept-case", "--corpus"])
        .arg(&corpus)
        .arg(&out)
        .env("WIREPROBE_PROBES", work.join("no-probes-here"))
        .env("WIREPROBE_CACHE", work.join("cache"))
        .output()
        .expect("the sweep runs");
    // The exit status is not the assertion — a sweep that reaches no
    // toolchain is allowed to report that. What is asserted is the file.
    let _ = run.status;

    let after = std::fs::read_to_string(&out).expect("the source file still exists");
    assert!(
        after.contains("kept-case\tcyclone"),
        "a case the run was not asked about was deleted:\n{after}"
    );
    assert!(
        after.contains("swept-case\tfastdds"),
        "a vendor the run was not asked about was deleted:\n{after}"
    );
    let _ = std::fs::remove_file(&out);
}

/// The same rule, on the file beside it.
///
/// `toolchain-warnings.tsv` is merged by `probe_diagnostics_merge`,
/// which kept rows by **system** while the byte rows were kept by
/// (case, vendor). One key narrower, so a subset run deleted every
/// diagnostic of the swept vendors for the cases it never touched.
/// Measured on a `--vendors fastdds --cases @25-cases` re-record: 107
/// rows to 16, exit 0, "16 new row(s)" printed. Nothing downstream
/// noticed, for the reason the test above gives.
///
/// Driven the same way — a sweep with no probe, which measures nothing.
#[test]
fn a_subset_sweep_does_not_delete_the_diagnostics_it_was_not_asked_about() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().expect("the checkout");
    let work = std::env::temp_dir().join(format!("zd-diag-{}", std::process::id()));
    let corpus = work.join("corpus");
    for case in ["kept-case", "swept-case"] {
        std::fs::create_dir_all(corpus.join(case)).expect("the fake corpus");
        std::fs::write(
            corpus.join(case).join("type.idl"),
            "@final struct Sample { long a; };",
        )
        .expect("the fake IDL");
    }
    let out = work.join("vendor-corpus.tsv");
    std::fs::write(&out, "swept-case\tcyclone\txcdr1\tle\twrote\tbeef\n")
        .expect("the previous run");
    let warnings = work.join("toolchain-warnings.tsv");
    let before = "\
kept-case\tcyclone\tcc\t1\twarning\tthe row of a case this run is not asked about\n\
swept-case\tcyclone\tcc\t1\twarning\tthe row this run may replace\n\
swept-case\tfastdds\tcc\t1\twarning\tthe row of a vendor this run is not asked about\n\
-\tcyclone\tcc\t1\twarning\ta row that names no case\n";
    std::fs::write(&warnings, before).expect("the previous diagnostics");

    let run = std::process::Command::new("bash")
        .arg(root.join("tools/interop-results/vendor-corpus-sweep.sh"))
        .args(["--vendors", "cyclone", "--cases", "swept-case", "--corpus"])
        .arg(&corpus)
        .arg(&out)
        .env("WIREPROBE_PROBES", work.join("no-probes-here"))
        .env("WIREPROBE_CACHE", work.join("cache"))
        .output()
        .expect("the sweep runs");
    let _ = run.status;

    let after = std::fs::read_to_string(&warnings).expect("the diagnostics file still exists");
    assert!(
        after.contains("kept-case\tcyclone"),
        "a case the run was not asked about lost its diagnostics:\n{after}"
    );
    assert!(
        after.contains("swept-case\tfastdds"),
        "a vendor the run was not asked about lost its diagnostics:\n{after}"
    );
    assert!(
        after.contains("-\tcyclone"),
        "a row that names no case is not one this run replaced:\n{after}"
    );
    let _ = std::fs::remove_file(&warnings);
    let _ = std::fs::remove_file(&out);
}

/// The sweep never replaces its source file wholesale when it was asked
/// for a subset.
///
/// The behavioural test above drives the path where nothing is measured.
/// This one holds the other half, which no test on a machine without a
/// vendor toolchain can reach: that the merge exists at all, and that a
/// subset run does not end in the unconditional `mv` a whole-corpus run
/// ends in.
#[test]
fn a_subset_run_merges_rather_than_moving_over_the_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = std::fs::read_to_string(root.join("tools/interop-results/vendor-corpus-sweep.sh"))
        .expect("the sweep script");
    for (needle, why) in [
        (
            "if [ -f \"$out\" ]; then",
            "a run has no merge branch, so it would move its own rows over every case and every \
             vendor it was not asked about",
        ),
        (
            "row(s) kept from the previous run",
            "the merge does not say how much it kept, so a merge that kept nothing would read \
             like a merge that worked",
        ),
        (
            "$out is left as it was",
            "a run that reaches no vendor does not say it left the source alone, which is the \
             sentence that replaced truncating it",
        ),
        (
            "--cases \"$work/cases\"",
            "the batch driver is not given the case filter, so a subset run measures every case \
             for a batched vendor — which is how 1104 duplicate rows got written",
        ),
        (
            "\"$work/rows.tsv\" | LC_ALL=C sort -u > \"$work/swept-keys\"",
            "the merge keys on the cases it was asked for rather than on the rows it produced; a \
             sweep that measures more than it was asked to then leaves an old row beside the new \
             one for the same cell",
        ),
    ] {
        assert!(script.contains(needle), "vendor-corpus-sweep.sh: {why}");
    }
    assert!(
        !script.contains("nothing was measured\" >&2\n    : > \"$out\""),
        "the sweep truncates its own source when no vendor is present; that deletes every \
         recorded byte in the tree and exits 0"
    );
}

/// A probe keeps what its toolchain said, and keeps it on a cache hit.
///
/// The measured cost of not doing so: the `wchar` fill defect was a
/// multi-character constant, which gcc reports by default under
/// `-Wmultichar`, and that line was printed on every probe build
/// carrying the literal. Every probe wrote it into a `cc.log` inside a
/// `mktemp -d` that a `trap … EXIT` removed, read back `error:` lines
/// and read them only when the build had failed. The `c3a4` the warning
/// explained stood in the results as two vendors' own `wchar` octets —
/// 56 rows — until somebody re-derived it from the bytes.
///
/// Four things have to hold together, and each of the four is a way the
/// line goes missing again:
///
///  * the probe records after the build, not only when it failed;
///  * it publishes the log into the cache entry, or the *second* sweep
///    over a case is silent because it did not compile;
///  * it replays that log on a cache hit, for the same reason;
///  * nothing suppresses the diagnostic before it is printed.
#[test]
fn every_probe_keeps_what_its_toolchain_said() {
    let root = results::repository_root();
    let directory = wireprobe(&root);
    let helper = std::fs::read_to_string(directory.join("diagnostics.sh"))
        .expect("tools/wireprobe/diagnostics.sh");
    for wanted in [
        "probe_diagnostics_record",
        "probe_diagnostics_publish",
        "probe_diagnostics_cached",
        "probe_diagnostics_after",
        "probe_diagnostics_merge",
    ] {
        assert!(
            helper.contains(&format!("{wanted}()")),
            "diagnostics.sh has no {wanted}"
        );
    }

    let mut trouble: Vec<String> = Vec::new();
    // `hdds` is not in this list and that is deliberate: the probe has
    // no cache and no read direction, so the publish/cached pair and
    // the two-calls rule below state premises it does not have. Its one
    // build does record — `probe_diagnostics_after hdds` — and the test
    // after this one holds that call to a system id the matrix knows.
    for probe in [
        "cyclone", "rti", "fastdds", "opendds", "omniorb", "tao", "jacorb", "ros",
    ] {
        let name = format!("wireprobe-{probe}");
        let text = std::fs::read_to_string(directory.join(&name))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        for (needle, why) in [
            (
                ". \"$TOOLS/diagnostics.sh\"",
                "does not source the helper, so nothing it compiles is recorded",
            ),
            (
                "probe_diagnostics_after",
                "does not record after its build, so a run that compiled cleanly and warned \
                 says nothing",
            ),
            (
                "probe_diagnostics_publish",
                "does not put the log in the cache entry, so the next sweep over the case is \
                 silent",
            ),
            (
                "probe_diagnostics_cached",
                "does not replay the cached log, so a re-record loses every warning it did \
                 not rebuild",
            ),
        ] {
            if !text.contains(needle) {
                trouble.push(format!("{name} {why}"));
            }
        }
        // A build has two halves — the write direction and the read one
        // — and both compile. One call would cover one of them.
        for needle in ["probe_diagnostics_after", "probe_diagnostics_cached"] {
            if text.matches(needle).count() < 2 {
                trouble.push(format!(
                    "{name} calls {needle} once; the write and the read direction each build, \
                     and the one that is not covered is silent"
                ));
            }
        }
        for (suppression, why) in [
            (
                " -w\"",
                "compiles with `-w`, which turns every diagnostic off",
            ),
            (
                " -w ",
                "compiles with `-w`, which turns every diagnostic off",
            ),
            (
                "-nowarn",
                "compiles with `-nowarn`, which turns every diagnostic off",
            ),
        ] {
            if text.contains(suppression) && !text.contains(&format!("`{}`", suppression.trim())) {
                trouble.push(format!("{name} {why}"));
            }
        }
    }
    assert!(
        trouble.is_empty(),
        "a probe's toolchain output goes missing, which is how the `wchar` fill defect \
         survived:\n{}",
        trouble.join("\n")
    );
}

/// The two diagnostic sources are read, and neither writes the other's
/// file.
#[test]
fn the_compiler_rows_reach_the_tables() {
    let root = results::repository_root();
    let assemble = std::fs::read_to_string(root.join("tools/interop-results/src/assemble.rs"))
        .expect("assemble.rs");
    assert!(
        assemble.contains("toolchain-warnings.tsv"),
        "the tables read only `warnings.tsv`, so every compiler row a sweep recorded is \
         written and never looked at"
    );
    let sweep = std::fs::read_to_string(root.join("tools/interop-results/warning-sweep.sh"))
        .expect("warning-sweep.sh");
    assert!(
        !sweep.contains("toolchain-warnings.tsv"),
        "the front-end sweep writes the compilers' file too; it `mv`s its output over what it \
         writes, so the compiler rows would live until the next front-end sweep"
    );
    for (relative, systems) in [
        (
            "tools/interop-results/vendor-corpus-sweep.sh",
            "the four DDS vendors",
        ),
        (
            "tools/interop-results/ros-corpus-sweep.sh",
            "the two ROS type supports",
        ),
        ("tools/corba-oracle/corba-sweep.sh", "the three ORBs"),
    ] {
        let text = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("{relative}: {error}"));
        assert!(
            text.contains("WIREPROBE_DIAGNOSTICS"),
            "{relative} does not ask its probes to keep what {systems}' toolchains said"
        );
        assert!(
            text.contains("probe_diagnostics_merge"),
            "{relative} collects nothing, so what its probes recorded is deleted with the \
             sweep's temporary directory"
        );
    }
}

/// A probe records under the system id the matrix has.
///
/// `wireprobe-ros` serves two of them — `ros-fastrtps` and
/// `ros-cyclone` — and the tables key a diagnostic by `<case>/<system>`.
/// A row filed under `ros` would be written, merged, committed and
/// never found by `warnings_of`, which is the same loss as deleting it
/// and harder to see.
#[test]
fn a_diagnostic_is_filed_under_a_system_the_matrix_has() {
    let root = results::repository_root();
    let ids: BTreeSet<&str> = results::systems::SYSTEMS
        .iter()
        .map(|system| system.id)
        .collect();
    let directory = wireprobe(&root);
    let mut trouble: Vec<String> = Vec::new();
    for probe in [
        "cyclone", "rti", "fastdds", "opendds", "hdds", "omniorb", "tao", "jacorb", "ros",
    ] {
        let name = format!("wireprobe-{probe}");
        let text = std::fs::read_to_string(directory.join(&name))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix("probe_diagnostics_after ") else {
                continue;
            };
            let argument = rest.split_whitespace().next().unwrap_or_default();
            // A shell expansion is checked by what it can expand to, which
            // for this one probe is its two type supports.
            if argument.contains('$') {
                let expanded = ["fastrtps", "cyclone"]
                    .iter()
                    .map(|support| {
                        argument.replace("\"ros-$typesupport\"", &format!("ros-{support}"))
                    })
                    .all(|id| ids.contains(id.as_str()));
                if !expanded {
                    trouble.push(format!(
                        "{name}: `{argument}` does not expand to a system id"
                    ));
                }
                continue;
            }
            if !ids.contains(argument.trim_matches('"')) {
                trouble.push(format!(
                    "{name}: files its diagnostics under `{argument}`, which is not a column"
                ));
            }
        }
    }
    assert!(
        trouble.is_empty(),
        "a diagnostic is filed under a system the tables do not have, so nothing ever reads \
         it:\n{}",
        trouble.join("\n")
    );
}

/// A cache entry carries the mechanism that wrote it.
///
/// Measured, and it is why this test exists: the first full re-record
/// after `diagnostics.sh` landed reported `0 new row(s)` for the three
/// ORBs and both ROS type supports. Nothing was wrong with the probes —
/// every one of their builds was a hit on an entry written before the
/// mechanism, and an entry from before it holds no rows to replay. A
/// run that reports nothing because it did not look is the same shape
/// as a run that reports nothing because there was nothing to say, and
/// the row count cannot tell them apart.
#[test]
fn the_probe_cache_key_carries_the_diagnostics_mechanism() {
    let root = results::repository_root();
    let cache = std::fs::read_to_string(wireprobe(&root).join("probe-cache.sh"))
        .expect("tools/wireprobe/probe-cache.sh");
    assert!(
        cache.contains("PROBE_CACHE_VERSION"),
        "the cache key does not carry a version, so an entry written before a change to what \
         an entry holds is still a hit and still answers with what it does not have"
    );
    let key = cache
        .split("probe_cache_key() {")
        .nth(1)
        .expect("probe_cache_key");
    assert!(
        key.contains("PROBE_CACHE_VERSION"),
        "the version is declared and is not hashed into the key, which is the same as not \
         having it"
    );
    let helper = std::fs::read_to_string(wireprobe(&root).join("diagnostics.sh"))
        .expect("tools/wireprobe/diagnostics.sh");
    assert!(
        helper.contains(".silent-hits"),
        "a cache hit that carried no rows is passed over silently, so a sweep that reported \
         nothing cannot be told from a toolchain that said nothing"
    );
}

/// Every probe driver hashes the template it compiles.
///
/// A cache entry is keyed by "everything that goes into it", and for six
/// of the seven drivers the template was not in that everything: the key
/// took the case's IDL and the generated `fill.inc` / `dump.inc` and
/// stopped. So an edit to `rti-template.c`, `omniorb-template.cpp` or
/// `tao-template.cpp` left every key unchanged, and the next re-record
/// with `WIREPROBE_CACHE` set handed back binaries built from the text
/// before the edit — a tree whose probes and whose committed record
/// disagree, which is what `crate::reproducible` exists to refuse and
/// what no verify can see, because a verify reads the record.
///
/// `wireprobe-ros` passed both of its templates from the start, which is
/// what made the other six visible as an inconsistency rather than a
/// design. Found 2026-08-13, in the run that changed three templates at
/// once.
///
/// # A driver with no template
///
/// `wireprobe-zerodds-zig`, `-fsharp` and `-kotlin` declare no
/// `TEMPLATE` at all: their `*-gen` binary writes the whole program —
/// `main.zig`, `Main.fsx`, `ZdTypes.kt` — and the key hashes **that
/// file**, together with the runtime module copied beside it. That is
/// the same guarantee and a stronger one: a generator edit changes the
/// generated text and so changes the key, where a template edit only
/// changes the template. Asking those three for a `$TEMPLATE` asks for
/// a file that does not exist, so the check here is: hash the template
/// where there is one, and hash a generated file under `$work` where
/// there is not. A key that hashes neither is still an offender.
#[test]
fn a_cache_key_hashes_the_template_it_compiles() {
    let directory = wireprobe(&results::repository_root());
    let mut offenders: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(&directory).expect("tools/wireprobe");
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if !name.starts_with("wireprobe-") || path.is_dir() {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        // A `probe_cache_key … -- \` continues on the next line, and the
        // files it hashes are on that continuation. Reading line by line
        // saw an empty file list and reported every such driver.
        let mut joined: Vec<String> = Vec::new();
        for line in text.lines() {
            match joined.last_mut() {
                Some(previous) if previous.ends_with('\\') => {
                    previous.pop();
                    previous.push(' ');
                    previous.push_str(line.trim_start());
                }
                _ => joined.push(line.to_string()),
            }
        }
        for line in &joined {
            let Some(rest) = line.split_once("probe_cache_key").map(|(_, rest)| rest) else {
                continue;
            };
            // The files whose content is hashed are the ones after `--`.
            let Some(files) = rest.split(" -- ").nth(1) else {
                offenders.push(format!("{name}: a cache key hashes no file at all"));
                continue;
            };
            // Which of the two templates the line has to name follows
            // from the direction it is keyed under, so a decode key
            // carrying the encode template is an offender too.
            let expected = if rest.contains(" decode ") {
                "DECODE_TEMPLATE"
            } else {
                "TEMPLATE"
            };
            // A driver that declares no template of that kind has none
            // to name; see the note above. What it owes instead is the
            // generated program, which is under `$work`.
            let declares_template = text.contains(&format!("{expected}="));
            let named = if !declares_template {
                files.contains("$work/")
            } else if expected == "TEMPLATE" {
                files.contains("$TEMPLATE") || files.contains("-template.")
            } else {
                files.contains("$DECODE_TEMPLATE") || files.contains("-decode-template.")
            };
            if !named {
                let owed = if declares_template {
                    format!(
                        "does not hash its {expected}, so an edit to the template is a \
                         cache hit on the binary built before it"
                    )
                } else {
                    "declares no template and hashes no generated file either, so nothing \
                     the generator writes reaches the key"
                        .to_string()
                };
                offenders.push(format!("{name}: a cache key {owed}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a probe binary's cache key does not carry the template compiled around the case, so \
         a re-record after a template change records the octets of the source before it:\n{}",
        offenders.join("\n")
    );
}

/// A sweep that could not measure does not delete what was measured.
///
/// Four sweeps truncated their own output file when the toolchain they
/// need was absent — `: > "$out"` — and exited 0. The line beside it
/// says "nothing was measured in this run", which is true and is not
/// what the truncation does: it deletes the rows a run that *had* the
/// toolchain wrote. `ros-corpus-sweep.sh` took 1388 rows that way on any
/// machine without ROS, `read-corpus-sweep.sh` 7766, and
/// `vendor-corpus-sweep.sh` deleted 4148 rows of three vendors on a
/// `--vendors rti` run that had every toolchain and simply was not asked
/// about them.
///
/// None of it fails a verify, because the verify holds the tree against
/// the sources and the two shrink together — which is the reasoning
/// `vendor-corpus-sweep.sh`'s own header gives for merging, applied to
/// the case subset only. Measured 2026-08-13.
///
/// The check is textual because the condition it guards is textual: an
/// output file is truncated in one place per script and it is always
/// this line.
#[test]
fn a_sweep_that_measured_nothing_keeps_what_was_measured_before() {
    let root = results::repository_root();
    let sweeps = [
        root.join("tools/interop-results/vendor-corpus-sweep.sh"),
        root.join("tools/interop-results/ros-corpus-sweep.sh"),
        root.join("tools/interop-results/read-corpus-sweep.sh"),
        root.join("tools/interop-results/warning-sweep.sh"),
        root.join("tools/corba-oracle/corba-sweep.sh"),
    ];
    let mut offenders: Vec<String> = Vec::new();
    for sweep in sweeps {
        let name = sweep
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(&sweep)
            .unwrap_or_else(|error| panic!("{}: {error}", sweep.display()));
        for (number, line) in text.lines().enumerate() {
            let bare = line.trim();
            if bare.starts_with('#') {
                continue;
            }
            if !bare.contains(": > \"$out\"") {
                continue;
            }
            // Creating the file when there is none is not deleting a
            // measurement. The guard is `[ -f "$out" ]` a few lines up,
            // which is how `vendor-corpus-sweep.sh` already writes it.
            let before: Vec<&str> = text.lines().take(number).collect();
            let guarded = before
                .iter()
                .rev()
                .take(6)
                .any(|earlier| earlier.contains("[ -f \"$out\" ]"));
            if guarded {
                continue;
            }
            offenders.push(format!(
                "{name}:{}: truncates its own output unguarded, so a run without the \
                 toolchain deletes the rows of the run that had it",
                number + 1
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "a sweep deletes a committed source file when it cannot measure:\n{}",
        offenders.join("\n")
    );
}

/// The vendor sweep merges whether the subset is of cases or of vendors.
///
/// `--vendors rti` is the command `docs/OPEN-ITEMS.md` names for
/// re-recording RTI after a probe change, and the merge was gated on
/// `--cases`, so that command overwrote `vendor-corpus.tsv` with RTI's
/// 1388 rows and deleted the other three vendors' 4148.
#[test]
fn the_vendor_sweep_merges_on_a_vendor_subset_too() {
    let root = results::repository_root();
    let text = std::fs::read_to_string(root.join("tools/interop-results/vendor-corpus-sweep.sh"))
        .expect("tools/interop-results/vendor-corpus-sweep.sh");
    assert!(
        !text.contains("if [ -n \"$only\" ] && [ -f \"$out\" ]; then"),
        "the merge is gated on --cases, so a --vendors subset overwrites the file and takes \
         the rows of every vendor it was not asked about with it"
    );
    assert!(
        text.contains("if [ -f \"$out\" ]; then"),
        "the vendor sweep does not merge into an existing output file at all"
    );
}

/// Every recorded payload has a read verdict about that payload.
///
/// `sources/read-corpus.tsv` is a function of the write records: the
/// `--read` sweep builds its input out of `vendor-corpus.tsv`,
/// `ros-corpus.tsv` and `corba-corpus.tsv`, hands each recorded payload
/// back to the system that wrote it, and compares what comes out with
/// the case's own sample. So a write record that changes without the
/// read sweep being re-run leaves rows describing a payload that no
/// longer exists, and rows saying `unwritten` — "the write direction
/// produced nothing" — about a cell where it produced bytes.
///
/// Both had happened. In the tree before this check, **24** cells
/// carried bytes in `vendor-corpus.tsv` and `unwritten` in
/// `read-corpus.tsv` — `identifier-target-keyword-typename` and its twin
/// under RTI, `union-member-sequence` and `union-member-struct` and
/// their twins under Fast DDS — and eight more read `different` about
/// `map-nested` and `map-value-sequence` payloads that had been
/// corrected since. Nothing was red, because nothing compared the two
/// files. Re-recorded 2026-08-13; the residual is zero and this is what
/// keeps it there.
///
/// The check is one-directional on purpose. A read row without a write
/// row is stale in the other sense and is caught by
/// `committed_tree.rs`; what this one refuses is a *claim of absence*
/// standing over a measurement that exists.
#[test]
fn a_recorded_payload_has_a_read_verdict_about_that_payload() {
    let root = results::repository_root();
    let sources = root.join("tests/interop-results/sources");
    let read = std::fs::read_to_string(sources.join("read-corpus.tsv")).unwrap_or_default();
    let mut verdict: std::collections::HashMap<(String, String, String, String), String> =
        std::collections::HashMap::new();
    for line in read.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            continue;
        }
        verdict.insert(
            (
                f[0].to_string(),
                f[1].to_string(),
                f[2].to_string(),
                f[3].to_string(),
            ),
            f[4].to_string(),
        );
    }
    // Only the sweeps whose rows the `--read` step feeds on. The ORB
    // source has a different column shape and is folded in by
    // `record.sh`, not by this key.
    let mut offenders: Vec<String> = Vec::new();
    for file in ["vendor-corpus.tsv", "ros-corpus.tsv"] {
        let text = std::fs::read_to_string(sources.join(file)).unwrap_or_default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 6 || f[4] != "wrote" || f[5].is_empty() {
                continue;
            }
            let key = (
                f[0].to_string(),
                f[1].to_string(),
                f[2].to_string(),
                f[3].to_string(),
            );
            match verdict.get(&key) {
                Some(found) if found != "unwritten" => {}
                found => offenders.push(format!(
                    "{}/{}/{}/{}: the write record has {} octets and the read record says {}",
                    f[0],
                    f[1],
                    f[2],
                    f[3],
                    f[5].len() / 2,
                    found.map_or("nothing at all", String::as_str)
                )),
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "{} cell(s) where the read record contradicts the write record it is built from; \
         `tools/interop-results/record.sh --read` is what re-derives it:\n{}",
        offenders.len(),
        offenders
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
