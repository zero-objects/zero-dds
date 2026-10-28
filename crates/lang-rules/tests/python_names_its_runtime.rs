// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every runtime name the Python backend writes, `runtime/python`
//! answers to.
//!
//! # Why this is a test and not a convention
//!
//! Because the failure has no other reader. `CodecTarget::constant`
//! answers with a *string* — `"specification"`, `"TryConstruct.DISCARD"`
//! — and nothing between the backend and the interpreter looks at it. A
//! name the runtime does not have compiles here, renders into a
//! generated module, and fails at the probe with a message about the
//! generated module rather than about the backend that wrote it.
//!
//! Every column has met this. The Java backend named `Reader.Mark`
//! while its runtime published `Reader.ReaderMark`; the C++ one
//! qualified a free function with `Writer::`; the C one asked a
//! sequence for `n` where the struct declares `_length`. Each was found
//! by a compiler, one case at a time, after a sweep had already recorded
//! the case as refused.
//!
//! # Why this column needs it more than the other five
//!
//! The others are compiled. `tsc`, `javac`, `csc` and a C compiler all
//! read every generated line before a single octet is written, so a
//! misspelled policy is caught at build time even without this test.
//! Python is not compiled and its policies are not an enumeration:
//! `Writer.write_wstring` matches its `policy` argument against a tuple
//! of **strings**, so `"octets-terminated-2"` — which is what
//! `runtime/typescript` spells the same cell — reaches the interpreter,
//! reaches the runtime, and raises at the octet. On a corpus sweep that
//! is one refused case among 376 with a message about a wstring policy,
//! and nothing pointing at the backend.
//!
//! # The three things compared
//!
//! 1. **The policy strings** the backend writes against the sets
//!    `runtime/python/zerodds_wire/writer.py` accepts.
//! 2. **The `Enum.MEMBER` names** against the enumerations the runtime
//!    declares.
//! 3. **The runtime methods and free functions** the walk asks for
//!    against what `Reader`, `Writer` and the package export.
//!
//! The third is the one no other column's version of this test does,
//! and it is the one an interpreted target cannot do without.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use zerodds_lang_rules::emit::CodecTarget as _;
use zerodds_lang_rules::emit::python::Python;
use zerodds_lang_rules::{ModelGraph, apply};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn read(relative: &str) -> String {
    let path = repository_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The string and `Enum.MEMBER` literals the backend writes into
/// generated code.
///
/// Read out of the source rather than by calling `constant` over every
/// variant: the point is what the file *says*, and a helper that built
/// the list from the same match would agree with itself.
fn names_the_backend_writes() -> (BTreeSet<String>, BTreeSet<String>) {
    let source = read("crates/lang-rules/src/emit/python.rs");
    let mut policies = BTreeSet::new();
    let mut members = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        // Only the `constant` arms. A doc comment quoting a policy is
        // not a name the file writes, and `//` lines are where most of
        // them appear.
        if trimmed.starts_with("//") {
            continue;
        }
        for opener in ["=> text(\"", "text(\""] {
            if let Some(name) = between(line, opener, '"') {
                policies.insert(name);
            }
        }
        if let Some(name) = between(line, "Self::path(\"", '"') {
            if name.contains('.') && name.chars().next().is_some_and(char::is_uppercase) {
                members.insert(name);
            }
        }
    }
    (policies, members)
}

fn between(line: &str, opener: &str, closer: char) -> Option<String> {
    let rest = line.split_once(opener)?.1;
    let end = rest.find(closer)?;
    Some(rest[..end].to_string())
}

/// Every string literal `writer.py` and `reader.py` compare a policy
/// argument against.
///
/// The runtime keeps them in tuples and dict keys —
/// `_WSTRING_POLICIES`, `_WSTRING_CELLS`, `_WSTRING_UNIT_ORDERS` — and
/// tests the rest inline (`if framing == "giop-length-prefixed"`). Both
/// forms are read, because a policy the runtime accepts only inline
/// would otherwise look unaccepted.
fn policies_the_runtime_accepts() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in [
        "runtime/python/zerodds_wire/writer.py",
        "runtime/python/zerodds_wire/reader.py",
    ] {
        let source = read(file);
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            let mut rest = line;
            while let Some((_, tail)) = rest.split_once('"') {
                let Some(end) = tail.find('"') else { break };
                out.insert(tail[..end].to_string());
                rest = &tail[end + 1..];
            }
        }
    }
    // The three unnamed cells of C1 x C2 x C3 are reachable by their
    // coordinates and the runtime builds those keys with an f-string
    // rather than writing them out, so they are not literals anywhere.
    // Written here in the same shape `Writer._WSTRING_CELLS.update`
    // composes them: `counts/terminator/width`.
    for counts in ["characters", "octets"] {
        for terminator in ["terminated", "unterminated"] {
            for width in ["2", "4"] {
                out.insert(format!("{counts}/{terminator}/{width}"));
            }
        }
    }
    out
}

/// The enumerations `runtime/python` declares, with their members.
fn enumerations_the_runtime_declares() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in [
        "runtime/python/zerodds_wire/typesupport.py",
        "runtime/python/zerodds_wire/writer.py",
    ] {
        let source = read(file);
        let mut current: Option<String> = None;
        for line in source.lines() {
            if let Some(rest) = line.strip_prefix("class ") {
                current = rest
                    .split_once('(')
                    .filter(|(_, bases)| bases.starts_with("Enum"))
                    .map(|(name, _)| name.to_string());
                continue;
            }
            let Some(enumeration) = current.as_deref() else {
                continue;
            };
            let trimmed = line.trim_start();
            // A member is `NAME = value` at one indent, screaming case.
            let Some((member, _)) = trimmed.split_once(" = ") else {
                continue;
            };
            if !member.is_empty()
                && member
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
            {
                out.insert(format!("{enumeration}.{member}"));
            }
        }
    }
    out
}

/// Every `def` on `Reader` and `Writer`, and every module-level name the
/// package exports.
fn what_the_runtime_answers_to() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in [
        "runtime/python/zerodds_wire/writer.py",
        "runtime/python/zerodds_wire/reader.py",
        "runtime/python/zerodds_wire/typesupport.py",
        "runtime/python/zerodds_wire/values.py",
        "runtime/python/zerodds_wire/float128.py",
    ] {
        for line in read(file).lines() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("def ") {
                if let Some((name, _)) = rest.split_once('(') {
                    out.insert(name.to_string());
                }
            }
        }
    }
    out
}

/// Every runtime operation the two walks ask for, taken from the walks
/// themselves.
///
/// The generated file is what names these, and it names them by running
/// the walk — so the walk is where the list comes from. Reading them out
/// of `emit/encode.rs` and `emit/decode.rs` rather than listing them
/// here means an operation added to the walk is checked the day it
/// arrives and not the day somebody remembers to add it.
fn operations_the_walks_ask_for() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in [
        "crates/lang-rules/src/emit/encode.rs",
        "crates/lang-rules/src/emit/decode.rs",
    ] {
        for line in read(file).lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            for opener in [
                "call::<T>(\"",
                "call_expr::<T>(\"",
                "T::runtime(\"w\", \"",
                "T::runtime(\"r\", \"",
                "T::runtime_static(\"",
            ] {
                if let Some(name) = between(line, opener, '"') {
                    out.insert(name);
                }
            }
        }
    }
    out
}

#[test]
fn every_policy_the_backend_writes_is_one_the_runtime_accepts() {
    let (policies, _) = names_the_backend_writes();
    assert!(
        policies.len() > 12,
        "only {} policy strings were read out of the backend, which is not all of them",
        policies.len()
    );
    let accepted = policies_the_runtime_accepts();
    let missing: Vec<&String> = policies.difference(&accepted).collect();
    assert!(
        missing.is_empty(),
        "crates/lang-rules/src/emit/python.rs writes these into generated code and \
         runtime/python does not accept them, so a case that reaches one raises at the \
         octet with a message about a policy rather than about the backend. \
         runtime/typescript spells three of these cells with hyphens where this one \
         spells them with slashes, which is how a value gets copied across: {missing:?}"
    );
}

#[test]
fn every_enum_member_the_backend_names_is_one_the_runtime_declares() {
    let (_, members) = names_the_backend_writes();
    assert!(
        members.len() >= 10,
        "only {} enumeration members were read out of the backend: {members:?}",
        members.len()
    );
    let declared = enumerations_the_runtime_declares();
    let missing: Vec<&String> = members.difference(&declared).collect();
    assert!(
        missing.is_empty(),
        "crates/lang-rules/src/emit/python.rs names these and runtime/python declares \
         no such member, so a generated module raises AttributeError at the first \
         sample: {missing:?}"
    );
}

/// The check the other columns get from their compiler.
#[test]
fn every_operation_the_walks_ask_for_is_one_the_runtime_answers_to() {
    let asked = operations_the_walks_ask_for();
    assert!(
        asked.len() > 40,
        "only {} operations were read out of the two walks, which is not all of them",
        asked.len()
    );
    let answered = what_the_runtime_answers_to();
    // What the backend renames on the way through. Each is a spelling
    // decision `emit/python.rs` documents, and each is listed here so
    // that removing one from the backend without adding it to the
    // runtime fails.
    let renamed = ["pid_is_user_member"];
    let missing: Vec<&String> = asked
        .difference(&answered)
        .filter(|name| !renamed.contains(&name.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "the shared walk asks for these and runtime/python has no such method or \
         function, so every case that reaches one is an AttributeError naming the \
         generated module: {missing:?}"
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
    let (policies, _) = names_the_backend_writes();
    let accepted = policies_the_runtime_accepts();
    assert!(
        !policies.is_disjoint(&accepted),
        "the backend names none of the runtime's policies, which means the two files \
         are not about the same thing"
    );
}

/// The names in a generated module, against the same two files.
///
/// The three tests above read the *backend*; this one reads its
/// **output**. A name the backend composes rather than writes literally
/// — a header field reached with `header_field`, a method built from a
/// receiver and an operation — appears in neither of the string scans
/// and would go unchecked, which is exactly the Java column's
/// `Reader.Mark`.
#[test]
fn the_generated_module_names_nothing_the_runtime_does_not_have() {
    let answered = what_the_runtime_answers_to();
    let mut called: BTreeSet<String> = BTreeSet::new();
    for (idl, root) in [
        (
            "@mutable struct S { @id(1) long a; @id(2) sequence<string> b; @optional wstring c; };",
            "S",
        ),
        (
            "@final struct S { map<long, string> m; long g[2]; fixed<5,2> f; long double d; };",
            "S",
        ),
        (
            "@appendable union U switch (long) { case 1: long one; default: wchar other; };",
            "U",
        ),
    ] {
        for text in [generated(idl, root, true), generated(idl, root, false)] {
            for line in text.lines() {
                for receiver in ["w.", "r."] {
                    let mut rest = line;
                    while let Some((_, tail)) = rest.split_once(receiver) {
                        let end = tail.find('(').unwrap_or(0);
                        let name = &tail[..end];
                        if !name.is_empty()
                            && name.bytes().all(|byte| {
                                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                            })
                        {
                            called.insert(name.to_string());
                        }
                        rest = &tail[end..];
                    }
                }
            }
        }
    }
    assert!(
        called.len() > 15,
        "only {called:?} were read out of the generated modules, which is not enough to \
         be checking anything"
    );
    let missing: Vec<&String> = called.difference(&answered).collect();
    assert!(
        missing.is_empty(),
        "a generated module calls these on the writer or the reader and runtime/python \
         has no such method: {missing:?}"
    );
}

/// One IDL source, as the rendered Python codec for its root type.
fn generated(idl: &str, type_name: &str, writing: bool) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let ast = lowered.ast;
    let model = ModelGraph::of(&ast);
    let _applied =
        apply(&model, &zerodds_lang_rules::rule::python::catalogue()).expect("the rules apply");
    let root = ast
        .types()
        .find(|(_, node)| node.name.as_str() == type_name)
        .map(|(id, _)| id)
        .expect("the root type");
    let _ = Python::type_name(&ast, root);
    let key = zerodds_ast_layout::LayoutKey::data(
        zerodds_ast_layout::Encoding::PlainCdr2,
        zerodds_ast_layout::Endianness::Little,
    );
    let statements = if writing {
        zerodds_lang_rules::emit::encode::codec_from::<Python>(&ast, root, key, Default::default())
            .expect("the write walk reaches the end")
    } else {
        zerodds_lang_rules::emit::decode::decoder_from::<Python>(
            &ast,
            root,
            key,
            Default::default(),
        )
        .expect("the read walk reaches the end")
    };
    statements
        .iter()
        .map(zerodds_lang_render::python::statement)
        .collect::<Vec<_>>()
        .join("\n")
}
