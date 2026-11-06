// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Every `Language` against the directories that carry its parts.
//!
//! Four places name a language, and the compiler checks one of them.
//! `coverage.rs` has no catch-all arm, so a variant added to
//! `zerodds_lang_cst::Language` fails the build until every construct
//! has a decision for it — that guard works and has never drifted.
//!
//! The other three are directories, and each has drifted at least once:
//!
//! - Ada 83, F# and Kotlin were built as complete columns — materialiser,
//!   renderer, runtime, generator, their own passing tests — and had no
//!   `Language` variant, so nothing that walks the enum could see them.
//!   Their tests stayed green throughout, because a test that names its
//!   own language cannot notice that the enum does not.
//! - `matrix/gen-zerodds-<lang>.sh` reached seventeen of the columns
//!   with an `exit 9` guard and missed eleven, because the pattern that
//!   inserted it matched one indentation.
//! - `runtime/c` kept an unconditional alignment for two days after the
//!   Rust reader dropped it, and eleven sibling columns read the same
//!   octets correctly while C did not.
//!
//! Each was a claim about "every language" that no single test held. So
//! this one holds it: the enum is the list, the filesystem is the
//! evidence, and a language that gains a variant without its parts —
//! or a part without its variant — stops the build here rather than
//! being discovered in a cell weeks later.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use zerodds_lang_cst::Language;

/// The repository root, from this crate's manifest.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/lang-rules has two ancestors")
        .to_path_buf()
}

/// How a language spells itself in a directory name.
///
/// Mostly its own `as_str`, and the exceptions are the point of this
/// function: `c#` and `f#` are not path components, and Ada 83 is
/// `ada-83` as a directory while its generator is `gen-zerodds-ada83.sh`
/// — one language, two spellings, which is exactly the kind of thing
/// that goes unnoticed until something looks for the wrong one.
fn directory(language: Language) -> &'static str {
    match language {
        Language::CSharp => "csharp",
        Language::FSharp => "fsharp",
        Language::Cpp => "cpp",
        Language::Ada83 => "ada-83",
        other => other.as_str(),
    }
}

/// How a language spells itself in `matrix/gen-zerodds-<name>.sh`.
fn generator_name(language: Language) -> &'static str {
    match language {
        Language::Ada83 => "ada83",
        other => directory(other),
    }
}

/// Rust needs no `runtime/rust`: its codec calls `crates/wire`
/// directly, which is the runtime every other language reimplements for
/// itself. Recorded here so the absence is a decision and not a gap.
const NO_RUNTIME: &[Language] = &[Language::Rust];

/// Every language has one. The list is kept rather than removed: it is
/// where a future exception states its reason, and it was not empty for
/// long — TypeScript stood here until its SDK was written, which is the
/// use this constant is for. An empty list means every language is
/// covered, not that the question stopped being asked.
const NO_ENDPOINT: &[Language] = &[];

#[test]
fn xtypes_7_5_1_every_language_has_a_runtime() {
    let root = root();
    let missing: Vec<&str> = Language::ALL
        .into_iter()
        .filter(|language| !NO_RUNTIME.contains(language))
        .filter(|language| !root.join("runtime").join(directory(*language)).is_dir())
        .map(Language::as_str)
        .collect();
    assert!(
        missing.is_empty(),
        "no `runtime/<lang>` for: {}. A language with a codec and no runtime \
         generates calls into nothing; add the directory, or add the language \
         to NO_RUNTIME with the reason.",
        missing.join(", ")
    );
}

#[test]
fn xtypes_7_5_1_every_language_has_an_endpoint_sdk() {
    let root = root();
    let missing: Vec<&str> = Language::ALL
        .into_iter()
        .filter(|language| !NO_ENDPOINT.contains(language))
        .filter(|language| !root.join("endpoints").join(directory(*language)).is_dir())
        .map(Language::as_str)
        .collect();
    assert!(
        missing.is_empty(),
        "no `endpoints/<lang>` for: {}. Add the SDK, or add the language to \
         NO_ENDPOINT with the reason it shares another one's.",
        missing.join(", ")
    );
}

/// The cells of a column come from its generator in this repository,
/// `tools/wireprobe/wireprobe-zerodds-<lang>-gen`, which writes the program
/// the column's probe builds and runs. The matrix repository's
/// `gen-zerodds-<lang>.sh` calls it; whether that script exists and carries
/// its exit-9 guard is a property of the matrix repository and is checked
/// there, not in a checkout of this one (OPEN-ITEMS 92, the matrix's own
/// checks).
///
/// Rust has no generator crate: its cells are encoded in process by
/// `crates/wire-probe`, which reads the same model.
#[test]
fn xtypes_7_4_every_language_has_a_cell_generator() {
    let root = root();
    let missing: Vec<&str> = Language::ALL
        .into_iter()
        .filter(|language| *language != Language::Rust)
        .filter(|language| {
            !root
                .join("tools/wireprobe")
                .join(format!(
                    "wireprobe-zerodds-{}-gen",
                    generator_name(*language)
                ))
                .join("Cargo.toml")
                .is_file()
        })
        .map(Language::as_str)
        .collect();
    assert!(
        missing.is_empty(),
        "no `tools/wireprobe/wireprobe-zerodds-<lang>-gen` for: {}. Without one the column \
         has no cells, and the matrix reports it as unmeasured rather than as absent.",
        missing.join(", ")
    );
    assert!(
        root.join("crates/wire-probe/Cargo.toml").is_file(),
        "the Rust column's cells come from crates/wire-probe"
    );
}
