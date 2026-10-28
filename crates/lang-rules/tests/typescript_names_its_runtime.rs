// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every runtime name the TypeScript backend writes, `runtime/typescript`
//! exports.
//!
//! # Why this is a test and not a convention
//!
//! Because the failure has no other reader. `CodecTarget::constant`
//! answers with a *string* — `"WcharWidth.Octets2"` — and nothing
//! between the backend and `tsc` looks at it. A name the runtime does
//! not have compiles here, renders into a generated file, and fails at
//! the probe with a message about the generated file rather than about
//! the backend that wrote it.
//!
//! Every column has met this. The Java backend named `Reader.Mark`
//! while its runtime published `Reader.ReaderMark`; the C++ one
//! qualified a free function with `Writer::`; the C one asked a
//! sequence for `n` where the struct declares `_length`. Each was
//! found by a compiler, one case at a time, after a sweep had already
//! recorded the case as refused.
//!
//! This reads the two files and compares them, so the answer arrives
//! at `cargo test` and names both sides.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The `Enum.Member` names the backend writes into generated code.
///
/// Read out of the source rather than by calling `constant` over every
/// variant: the point is what the file *says*, and a helper that built
/// the list from the same match would agree with itself.
fn names_the_backend_writes() -> BTreeSet<String> {
    let source =
        std::fs::read_to_string(repository_root().join("crates/lang-rules/src/emit/typescript.rs"))
            .expect("the backend");
    let mut out = BTreeSet::new();
    for line in source.lines() {
        let Some(open) = line.find("=> \"") else {
            continue;
        };
        let rest = &line[open + 4..];
        let Some(close) = rest.find('"') else {
            continue;
        };
        let name = &rest[..close];
        if name.contains('.') && name.chars().next().is_some_and(char::is_uppercase) {
            out.insert(name.to_string());
        }
    }
    out
}

/// The enumerations `runtime/typescript` exports, with their members.
fn what_the_runtime_exports() -> BTreeSet<String> {
    let source =
        std::fs::read_to_string(repository_root().join("runtime/typescript/src/policy.ts"))
            .expect("the runtime's policies");
    let mut out = BTreeSet::new();
    let mut current: Option<String> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("export enum ") {
            current = rest.split_whitespace().next().map(ToString::to_string);
            continue;
        }
        if trimmed == "}" {
            current = None;
            continue;
        }
        let Some(enumeration) = current.as_deref() else {
            continue;
        };
        let member = trimmed.split(['=', ',']).next().unwrap_or("").trim();
        if !member.is_empty() && member.chars().next().is_some_and(char::is_uppercase) {
            out.insert(format!("{enumeration}.{member}"));
        }
    }
    out
}

#[test]
fn the_backend_names_nothing_the_runtime_does_not_export() {
    let written = names_the_backend_writes();
    assert!(
        written.len() > 20,
        "only {} names were read out of the backend, which is not all of them",
        written.len()
    );
    let exported = what_the_runtime_exports();
    // The generated import renames `Status` to `ZdReaderStatus` —
    // `enum-sparse-values` declares an `enum Status` of its own and the
    // probe and the type share one file (see `runtime_import` in
    // tools/wireprobe/wireprobe-zerodds-typescript-gen/src/lib.rs). The
    // backend writes the alias, the runtime exports the name.
    let missing: Vec<&String> = written
        .difference(&exported)
        .filter(|name| {
            !name
                .strip_prefix("ZdReaderStatus.")
                .is_some_and(|member| exported.contains(&format!("Status.{member}")))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "crates/lang-rules/src/emit/typescript.rs writes these into generated \
         code and runtime/typescript/src/policy.ts does not export them, so a \
         case that reaches one produces a file that names nothing: {missing:?}"
    );
}

/// The other direction, as a note rather than a failure.
///
/// A policy the runtime carries and the backend never names is not a
/// defect — the runtime is written from the clause and the backend
/// reaches what the corpus has — but a large gap between the two is
/// worth seeing, because it is where the next column's work is.
#[test]
fn what_the_runtime_carries_and_the_backend_has_not_reached() {
    let written = names_the_backend_writes();
    let exported = what_the_runtime_exports();
    let unreached: Vec<&String> = exported.difference(&written).collect();
    assert!(
        unreached.len() < exported.len(),
        "the backend reaches none of the runtime's policies, which means the \
         two files are not about the same thing"
    );
}
