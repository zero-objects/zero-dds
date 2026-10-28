// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `Ping`/`Pong` types and their XCDR2 codec, in any endpoint language.
//!
//! One entry point for all fourteen language tests. The fifteen
//! `crates/idl-<lang>` generators this crate used to name were removed with
//! the IDL transplant; what replaced them is a single rule layer reached
//! through [`zerodds_lang_rules::generate::source`], the same call
//! `tools/idlc` and `crates/lang-rules/tests/generate_contract.rs` make.
//!
//! ## What the generated code needs
//!
//! The emitted codec is **not** self-contained: it calls a per-language wire
//! runtime (Go's `zdwire`, Python's `zdwire`, …) that lives in `runtime/<lang>`
//! at the workspace root. The old per-language generators inlined their own
//! marshaller instead, which is why every app in `tests/` had to be re-pointed.
//! [`runtime_dir`] resolves that directory; [`endpoints_dir`] resolves the
//! endpoint SDK the framing cases additionally link.
//!
//! ## The wire is unchanged
//!
//! `Ping { seq: 1, msg: "hello from app" }` encodes to the same 23 octets it
//! did before the transplant — `01 00 00 00 | 0f 00 00 00 | "hello from app\0"`
//! — which is what lets the peer in `crate` stay exactly as it was.

use std::path::{Path, PathBuf};

pub use zerodds_lang_cst::Language;

/// The `Ping`/`Pong` declarations **and** codec as source in `language`,
/// XCDR2 little-endian — the encoding the peer decodes with.
///
/// # Panics
/// Panics if [`crate::IDL`] does not parse, lower, resolve or generate. All
/// four are properties of this crate's own constant and the generator, not of
/// anything a test passes in, so a failure here is a broken build, not a
/// failing case.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `zerodds_lang_rules::generate::source`, the rule layer's entry
/// point; `dds_bounded_recursion` compares only the last segment of a
/// call path, so the delegation reads as a self-call.
#[must_use]
pub fn source(language: Language) -> String {
    let parsed = zerodds_idl_parse::parse(crate::IDL);
    assert!(
        parsed.diagnostics().is_empty(),
        "IDL does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower_with(
        &parsed.syntax(),
        "pingpong.idl",
        zerodds_idl_lower::Options::default(),
    );
    assert!(
        lowered.diagnostics.is_empty(),
        "IDL does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve Ping/Pong");
    // The endpoint apps read the public fields of the generated C++ structures: the native
    // mapping, which is a named choice (the default is DDS-PSM-Cxx 1.0 §7.4 and §8.1).
    let _cpp = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    // The endpoint apps read the public fields of the generated Java types:
    // the IDL-to-Java mapping, which is a named choice (the default is Clause 9).
    zerodds_lang_rules::adapter::java::with_mapping(
        zerodds_lang_rules::adapter::java::JavaMapping::Omg,
        || zerodds_lang_rules::generate::source(&ast, language, layout_key()),
    )
    .unwrap_or_else(|error| panic!("{language:?}: {error}"))
}

/// The layout every app and the peer agree on: plain XCDR2, little-endian.
fn layout_key() -> zerodds_ast_layout::LayoutKey {
    zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    )
}

/// The workspace root — `crates/endpoint-e2e` up two.
///
/// # Panics
/// Panics if this crate is not two levels below the workspace root, which
/// would mean the crate was moved without this function.
#[must_use]
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root two above crates/endpoint-e2e")
        .to_path_buf()
}

/// The wire runtime the generated codec calls, e.g. `runtime/go`.
///
/// Canonicalised: Go's `replace` and Python's `sys.path` both need a real
/// path, and a relative one resolved against the app's temporary directory
/// would point nowhere.
///
/// # Panics
/// Panics if `runtime/<name>` does not exist — a missing runtime is a broken
/// checkout, not a language that happens to be absent from this host.
#[must_use]
pub fn runtime_dir(name: &str) -> PathBuf {
    let dir = workspace_root().join("runtime").join(name);
    dir.canonicalize()
        .unwrap_or_else(|error| panic!("runtime/{name}: {error}"))
}

/// The endpoint SDK the framing cases link, e.g. `endpoints/go`.
///
/// # Panics
/// Panics if `endpoints/<name>` does not exist.
#[must_use]
pub fn endpoints_dir(name: &str) -> PathBuf {
    let dir = workspace_root().join("endpoints").join(name);
    dir.canonicalize()
        .unwrap_or_else(|error| panic!("endpoints/{name}: {error}"))
}

/// A fresh, empty scratch directory for one app build, named after `label`.
///
/// Removed first if a previous run left it behind: a stale `main.go` or
/// `.class` from an earlier generator would otherwise be compiled instead of
/// the one this run wrote, and the case would pass on the wrong source.
///
/// # Panics
/// Panics if the directory cannot be created.
#[must_use]
pub fn scratch_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pp_{label}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|error| panic!("mkdir {}: {error}", dir.display()));
    dir
}

/// Is `tool` on `PATH` and runnable?
///
/// The one gate every language case takes. A missing toolchain skips its case
/// with a printed reason — it never reports a pass.
#[must_use]
pub fn tool_available(tool: &str, arg: &str) -> bool {
    std::process::Command::new(tool)
        .arg(arg)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every wired language generates a `Ping`, a `Pong` and a codec for
    /// both. A column that came back without one of them would be a silently
    /// dropped declaration, and the language's own test would fail far from
    /// the cause.
    #[test]
    fn every_language_generates_ping_and_pong() {
        for language in Language::ALL {
            let text = source(language);
            for want in ["Ping", "Pong"] {
                assert!(
                    text.contains(want),
                    "{language:?}: no `{want}` in the generated source:\n{text}"
                );
            }
        }
    }
}
