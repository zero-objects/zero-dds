// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The whole interop corpus, as Python, through CPython.
//!
//! The counterpart of the `every_corpus_case_that_renders_also_compiles`
//! that `swift_end_to_end.rs`, `go_end_to_end.rs` and eight more carry.
//! The Python column had none: `docs/OPEN-ITEMS.md` records that the six
//! oldest columns lost theirs when `zerodds-idl-<lang>` was removed.
//!
//! **Executed, not only compiled.** `python3 -m py_compile` answers one
//! question — is the text a Python program — and `python_codec.rs`
//! already asks it of the codec walk. It is the weaker half here:
//! Python's block structure *is* its indentation, but a
//! `dataclasses.field` whose default is mutable, an `enum` with two
//! members of one value and a decorator that refuses its class are all
//! accepted by `compile` and all raise when the module body runs. A
//! generated declarations module is a module body and nothing else —
//! every class in it is defined at import time — so running it is what a
//! consumer's `import` does, and it is what this asks. [`RUNNER`] then
//! resolves the annotations, which is the third question and the one
//! neither of the first two reaches.
//!
//! The module is executed under a name of its own in `sys.modules`,
//! which `dataclasses` requires: a `@dataclasses.dataclass` resolves its
//! own module to read `__annotations__` back, and a module that is not
//! registered fails with an `AttributeError` about the harness rather
//! than about the generated code.
//!
//! `print_stderr` is allowed here on purpose: a skipped interpreter
//! check and a frontend refusal both have to be *said*, and standard
//! error is the channel a passing test does not otherwise use.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;

use std::process::Command;

use zerodds_lang_cst::Language;

#[path = "support/corpus.rs"]
mod corpus_harness;
#[path = "support/generate.rs"]
mod generate_harness;
// -- the corpus ----------------------------------------------------------
/// How many corpus cases have to reach the interpreter for this gate to
/// mean anything. 418 cases render today.
const MUST_RENDER: usize = 350;

/// The names the Python column itself refuses, each with the reason.
///
/// A list and not a filter: a case on it that starts rendering fails
/// too.
const REFUSED: &[(&str, &str)] = &[];

/// The cases whose generated Python CPython rejects, each with the
/// message it answers. Empty, and a case that starts failing is a
/// finding rather than an entry to add without one.
const DOES_NOT_COMPILE: &[(&str, &str)] = &[];

// -- the interpreter -----------------------------------------------------

/// The one program the checks below run.
///
/// Reads a file named on the command line, compiles it, executes its
/// body in a module registered under a name of its own, and then
/// resolves every annotation the module declared.
///
/// The last step is not decoration. Since PEP 649 an annotation is not
/// evaluated when the class is created, so a member typed against a name
/// that does not exist survives both `compile` and `exec` — on CPython
/// 3.14 the two halves above say nothing about it. `typing.get_type_hints
/// (cls, include_extras=True)` is what evaluates them, and it is the
/// call the generated module's own docstring tells a consumer to make to
/// read the IDL facts back out of `typing.Annotated`.
const RUNNER: &str = "\
import sys, types, typing, inspect
path = sys.argv[1]
with open(path, encoding='utf-8') as handle:
    text = handle.read()
module = types.ModuleType('zd_generated')
module.__file__ = path
sys.modules['zd_generated'] = module
exec(compile(text, path, 'exec'), module.__dict__)
def visit(namespace):
    for value in list(vars(namespace).values()):
        if inspect.isclass(value) and getattr(value, '__module__', None) == 'zd_generated':
            typing.get_type_hints(value, include_extras=True)
            visit(value)
visit(module)
";

/// Whether `python3` is on the path at all.
fn python_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// One generated module, compiled and executed.
fn python_runs(source: &str, tag: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("zd-python-corpus-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let file = dir.join("zd_types.py");
    std::fs::write(&file, source).map_err(|error| error.to_string())?;
    let output = Command::new("python3")
        .arg("-c")
        .arg(RUNNER)
        .arg(&file)
        .output()
        .map_err(|error| error.to_string())?;
    let result = if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn the_interpreter_check_catches_a_deliberate_error() {
    // Two errors and not one, because the two halves of this check are
    // separable: a syntax error is what `compile` answers, and a name
    // that does not exist is what only running the body answers.
    if !python_available() {
        eprintln!("SKIPPED: `python3` is not on the path.");
        return;
    }
    let unparseable = "class S:\n  def f(:\n";
    let why =
        python_runs(unparseable, "deliberate-syntax").expect_err("CPython accepted a bad module");
    assert!(why.contains("SyntaxError"), "{why}");
    let unresolvable =
        "import dataclasses\n\n\n@dataclasses.dataclass\nclass S:\n    x: NoSuchTypeExists\n";
    let why =
        python_runs(unresolvable, "deliberate-name").expect_err("CPython accepted a bad name");
    assert!(why.contains("NoSuchTypeExists"), "{why}");
    // And a good one passes, so the check is not simply always red.
    python_runs(
        "import dataclasses\n\n\n@dataclasses.dataclass\nclass S:\n    x: int\n",
        "control",
    )
    .expect("CPython rejected a good module");
}

/// The whole corpus through CPython, one interpreter start per case.
///
/// `#[ignore]`, because it is one interpreter start per corpus case and
/// each costs about a seventh of a second — a minute for the corpus as
/// it stands. It is what proves the column rather than samples it, so it
/// is run with `--include-ignored` and its result is what the
/// `idl-python-corpus` job records.
#[test]
#[ignore = "one CPython start per corpus case, ~1 minute; run with --include-ignored"]
fn every_corpus_case_that_renders_also_compiles() {
    if !python_available() {
        eprintln!("SKIPPED: `python3` is not on the path.");
        return;
    }
    let mut failures = Vec::new();
    let mut unexpectedly_fine = Vec::new();
    let mut frontend = Vec::new();
    let mut checked = 0_usize;
    for (name, path) in corpus_harness::corpus() {
        let source = std::fs::read_to_string(&path).expect("a corpus source");
        if !corpus_frontend::accepts(&source, &path.display().to_string()) {
            continue;
        }
        let refused = REFUSED.iter().find(|(case, _)| *case == name);
        let text = match (
            generate_harness::generate_declarations(
                &source,
                &path.display().to_string(),
                Language::Python,
            ),
            refused,
        ) {
            (Ok(text), None) => text,
            (Ok(_), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: renders now, but is listed as {why}"));
                continue;
            }
            (Err(why), None) => {
                if why.starts_with("parse:") || why.starts_with("lower:") {
                    frontend.push(format!("{name}: {why}"));
                } else {
                    failures.push(format!("{name}: {why}"));
                }
                continue;
            }
            (Err(_), Some(_)) => continue,
        };
        checked += 1;
        let known = DOES_NOT_COMPILE.iter().find(|(case, _)| *case == name);
        match (python_runs(&text, &name), known) {
            (Ok(()), None) | (Err(_), Some(_)) => {}
            (Ok(()), Some((case, why))) => {
                unexpectedly_fine.push(format!("{case}: runs now, but is listed as {why}"));
            }
            (Err(why), None) => failures.push(format!("{name}: {why}")),
        }
    }
    eprintln!(
        "{checked} corpus case(s) reached CPython; {} failed unexpectedly",
        failures.len()
    );
    if !frontend.is_empty() {
        eprintln!(
            "{} corpus case(s) the frontend refuses, so Python never saw them:\n{frontend:#?}",
            frontend.len()
        );
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} did not run:\n{failures:#?}",
        failures.len()
    );
    assert!(unexpectedly_fine.is_empty(), "{unexpectedly_fine:#?}");
    assert!(
        checked >= MUST_RENDER,
        "only {checked} corpus cases reached the interpreter; the gate says nothing \
         below {MUST_RENDER}"
    );
}
