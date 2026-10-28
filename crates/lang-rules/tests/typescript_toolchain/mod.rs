// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Finding the TypeScript compiler, once, for the five files that need it.
//!
//! Every TypeScript gate in this crate has to answer the same question
//! before it can measure anything: **is there a compiler, and which
//! one?** The answer used to stand five times, copied. It was copied
//! once more when `npx tsc` was taken out of it, and the copy-paste
//! gate reported the growth — which is the right report: five copies of
//! a rule about the toolchain are five places to forget when the rule
//! changes, and the rule had just changed.
//!
//! # What the rule is
//!
//! Three candidates and no `npx`: `$ZERODDS_TSC` — after `ZERODDS_CSC`
//! in `csharp_corpus_compiles.rs` — then the runtime's own
//! `node_modules/.bin/tsc`, then `tsc` on the path.
//!
//! `npx tsc` is deliberately absent. Without a local TypeScript, `npx`
//! resolves the name against the npm registry and runs whatever answers
//! to it; on the CI image that was `tsc@2.0.4`, an unrelated package,
//! and its "This is not the tsc command you are looking for" arrived in
//! the report as a compiler diagnostic. An absent compiler was reported
//! as a defect in the generator.

#![allow(dead_code)]
// The skip goes to standard output, where `cargo test -- --nocapture`
// shows it. Four of the five files that use this already allow it at
// their own root; saying it here means the fifth does not have to.
#![allow(clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root, from this crate's manifest.
pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Whether that program is a compiler that runs.
pub fn answers(program: &Path) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The TypeScript compiler to type-check with, or nothing.
pub fn compiler() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("ZERODDS_TSC") {
        let named = PathBuf::from(named);
        return answers(&named).then_some(named);
    }
    let local = repository_root().join("runtime/typescript/node_modules/.bin/tsc");
    if answers(&local) {
        return Some(local);
    }
    let on_path = PathBuf::from("tsc");
    answers(&on_path).then_some(on_path)
}

/// The compiler, or a said skip — and in CI, a failure.
///
/// `ci/Dockerfile.rust` installs TypeScript, so a missing `tsc` inside a
/// CI job is a stale or broken image and not a host without a toolchain.
/// Skipping there would leave the column unmeasured under a green
/// pipeline, which is the state these files exist to refuse.
///
/// `what` names what would go unmeasured — each caller measures
/// something different, and the sentence has to say which.
pub fn compiler_or_note(what: &str) -> Option<PathBuf> {
    if let Some(program) = compiler() {
        return Some(program);
    }
    assert!(
        std::env::var_os("CI").is_none(),
        "no TypeScript compiler in a CI job: neither $ZERODDS_TSC nor \
         runtime/typescript/node_modules/.bin/tsc nor `tsc` on PATH answers. \
         ci/Dockerfile.rust installs typescript — rebuild the ci-rust image (the \
         `ci-image` job) rather than leaving {what} unmeasured."
    );
    println!(
        "SKIPPED: no `tsc` on this machine (name one with ZERODDS_TSC) — the generated \
         TypeScript was not type-checked"
    );
    None
}

/// The flags every gate here type-checks with.
///
/// `--ignoreConfig` needs TypeScript 6.0 or newer: 5.9.3 answers
/// `error TS5023: Unknown compiler option`. The CI image pins 6.0.3.
pub const STRICT_FLAGS: &[&str] = &[
    "--ignoreConfig",
    "--noEmit",
    "--strict",
    "--target",
    "ES2022",
    "--module",
    "NodeNext",
    "--moduleResolution",
    "NodeNext",
    "--skipLibCheck",
];

/// `tsc --strict` over one generated file beside the runtime.
///
/// Answers the compiler's own output. `extra` carries the flags one
/// gate asks for beyond the shared set — `typescript_side_tables.rs`
/// adds `--noUncheckedIndexedAccess` and `--exactOptionalPropertyTypes`
/// because a side table is indexed and its optional members are the
/// thing under test.
pub fn typecheck(tsc: &Path, name: &str, text: &str, extra: &[&str]) -> String {
    let runtime = repository_root().join("runtime/typescript");
    let scratch = runtime.join("dist-generated");
    std::fs::create_dir_all(&scratch).expect("a directory for the generated file");
    let path = scratch.join(format!("{name}.ts"));
    std::fs::write(&path, text).expect("the generated file");
    said(
        Command::new(tsc)
            .current_dir(&runtime)
            .args(STRICT_FLAGS)
            .args(extra)
            .arg(&path),
    )
}

/// What a compiler run said, both streams, in order.
pub fn said(command: &mut Command) -> String {
    let output = command.output().expect("the TypeScript compiler runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
