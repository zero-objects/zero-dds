// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The corpus harness plumbing every language gate shares.
//!
//! What is shared is the plumbing that must not drift between the gates:
//! where the corpus cases come from and how one IDL becomes generated
//! text, with each refusing layer named. What stays per file is every
//! language's own compiler invocation, refusal tables and minimum-render
//! count, because those are the parts the gates are supposed to differ
//! in.
//!
//! Included through `#[path = "support/corpus.rs"]`, the way
//! `corpus_frontend.rs` is: every integration test is its own crate, so
//! the module is compiled once per gate that names it.

use std::path::PathBuf;

/// The repository root, resolved from the test crate's manifest
/// directory — the checkout, not any build directory.
pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Every corpus case, by name, sorted.
///
/// The corpus is one directory of cases, each with its `type.idl`;
/// `values.json` and the extensionless inputs belong to other probes.
pub fn corpus() -> Vec<(String, PathBuf)> {
    let root = repository_root().join("tests/interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).expect("the corpus directory") {
        let entry = entry.expect("a corpus entry");
        let idl = entry.path().join("type.idl");
        if idl.is_file() {
            out.push((entry.file_name().to_string_lossy().into_owned(), idl));
        }
    }
    out.sort();
    out
}

/// Every RPC corpus case, by name, sorted: `tests/interop-corpus-rpc`.
///
/// Beside the data corpus and not in it: a case there is a sample the wire
/// probes and the sweeps read, and an `interface` has none. Each case
/// names its service mapping in its `mapping` file.
#[allow(dead_code)]
pub fn rpc_corpus() -> Vec<(
    String,
    PathBuf,
    zerodds_lang_rules::adapter::interface::ServiceMapping,
)> {
    use zerodds_lang_rules::adapter::interface::ServiceMapping;
    let root = repository_root().join("tests/interop-corpus-rpc");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).expect("the RPC corpus directory") {
        let entry = entry.expect("an RPC corpus entry");
        let idl = entry.path().join("type.idl");
        if !idl.is_file() {
            continue;
        }
        let mapping =
            std::fs::read_to_string(entry.path().join("mapping")).expect("a mapping file");
        let mapping = match mapping.trim() {
            "basic" => ServiceMapping::Basic,
            "enhanced" => ServiceMapping::Enhanced,
            other => panic!("{}: mapping `{other}`", entry.path().display()),
        };
        out.push((
            entry.file_name().to_string_lossy().into_owned(),
            idl,
            mapping,
        ));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
