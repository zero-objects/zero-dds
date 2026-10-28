// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A type through its wire form and back, reading the same octets.
//!
//! `crate::from_ast` writes a type as the `CompleteTypeObject` a peer is
//! told over discovery; `dynamic::to_ast` reads one back into a model.
//! The two are inverses only if a `DynamicData` decoded against the
//! **rebuilt** model is the same value, and re-encodes to the same
//! octets, as one decoded against the model the IDL lowered to.
//!
//! That is what this measures, over every case of
//! `tests/interop-corpus/` whose type XTypes §7.3 can carry at all:
//!
//! 1. the case's IDL lowers to a model, and the sample encodes;
//! 2. every type in the model is registered as a `CompleteTypeObject`;
//! 3. the topic type's object goes back through `to_ast` into a *second*
//!    model, built out of nothing but what a peer would have been sent;
//! 4. the same bytes are decoded against that second model, and encoded
//!    again;
//! 5. the two byte strings are compared, octet for octet.
//!
//! ## What this is and is not
//!
//! It is not the foreign measurement. Both objects are this tree's, so
//! it cannot catch a `TypeObject` this project writes and reads
//! consistently and every vendor reads differently — that needs a peer,
//! and `crates/types/src/type_lookup.rs` is what fetches one. See
//! `dynamic::to_ast`'s module note.
//!
//! It is the strongest measurement that runs with nothing installed, and
//! it catches the thing a hand-written bridge always gets wrong: a
//! property that reaches the wire form and does not come back.
//! Extensibility, a member id, an `@optional` flag, a bound, an array's
//! dimensions, an enumerator's value, a bit position, a bitfield's width
//! — drop any one of them on the way back and the second model frames
//! the same value differently, at some offset, in some mode.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_types::dynamic::DynamicData;
use zerodds_types::from_ast::{FromAstError, complete_type_object, register_all};
use zerodds_types::resolve::TypeRegistry;
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_wire_codec::{decode, encode};

/// The four combinations the corpus runs in.
const MODES: [(Encoding, Endianness); 4] = [
    (Encoding::PlainCdr2, Endianness::Little),
    (Encoding::PlainCdr2, Endianness::Big),
    (Encoding::PlainCdr1, Endianness::Little),
    (Encoding::PlainCdr1, Endianness::Big),
];

struct Case {
    name: String,
    type_name: String,
    idl: String,
    values: Value,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repository root, two levels above crates/types")
        .to_path_buf()
}

/// The corpus cases that never reach a model, because the IDL front end
/// refuses the file.
///
/// `resolve-module-named-as-type` is `module common { struct common
/// { … }; }`. OMG IDL 4.2 §7.5.2 does not admit a name redefined within
/// the immediate scope of the construct that carries it, and prints
/// `module M { typedef short M; }` as the clause's own `// Error`
/// example. The case is in the corpus deliberately — it measures which
/// vendors refuse the file and which resolve it anyway — and
/// `tools/idlc/tests/corpus.rs` carries it as an `Intended` diagnostic
/// for that reason. `dynamic_corpus.rs::NOT_LOWERED` is the same list,
/// beside the same corpus.
const NOT_LOWERED: [&str; 1] = ["resolve-module-named-as-type"];

fn corpus() -> Vec<Case> {
    let directory = repository_root().join("tests").join("interop-corpus");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|entry| entry.expect("a corpus entry").path())
        .filter(|path| path.join("type").is_file())
        .collect();
    paths.sort();
    // Exact, so that a name that leaves the corpus — or starts lowering,
    // and belongs back in the walk — is reported rather than silently
    // skipped.
    let mut dropped: Vec<String> = Vec::new();
    paths.retain(|path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        if NOT_LOWERED.contains(&name.as_str()) {
            dropped.push(name);
            return false;
        }
        true
    });
    assert_eq!(
        dropped,
        NOT_LOWERED.to_vec(),
        "the cases the front end refuses are not the cases named"
    );
    paths
        .into_iter()
        .map(|directory| {
            let name = directory
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a case name")
                .to_string();
            let read = |file: &str| {
                std::fs::read_to_string(directory.join(file))
                    .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
            };
            let values_text = read("values.json");
            Case {
                type_name: read("type").trim().to_string(),
                idl: read("type.idl"),
                values: serde_json::from_str(&values_text)
                    .unwrap_or_else(|error| panic!("{name}/values.json: {error}")),
                name,
            }
        })
        .collect()
}

fn model(case: &Case) -> Ast {
    let parsed = zerodds_idl_parse::parse(&case.idl);
    assert!(
        !parsed.has_errors(),
        "{}: {:?}",
        case.name,
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), &format!("{}/type.idl", case.name));
    assert!(
        lowered.diagnostics.is_empty(),
        "{}: {:?}",
        case.name,
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast
        .resolve_all_by_name()
        .unwrap_or_else(|error| panic!("{}: {error}", case.name));
    assert!(report.is_clean(), "{}: {:?}", case.name, report.broken);
    ast
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The type of one case, by the scoped name the case names.
fn find(ast: &Ast, name: &str) -> Option<zerodds_ast_core::Id<zerodds_ast::TypeNode>> {
    let segments: Vec<String> = name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
}

#[test]
fn a_type_survives_its_own_wire_form() {
    let cases = corpus();
    assert!(!cases.is_empty(), "the corpus is empty");

    let mut compared = 0usize;
    let mut unrepresentable: BTreeSet<String> = BTreeSet::new();
    let mut moved: Vec<String> = Vec::new();

    for case in &cases {
        let ast = model(case);
        let Some(type_) = find(&ast, &case.type_name) else {
            panic!("{}: `{}` is not in the model", case.name, case.type_name);
        };

        // What a peer would be sent: this type's object, and every other
        // type in the model in the registry behind it so that a member's
        // hashed identifier resolves.
        let object = match complete_type_object(&ast, type_) {
            Ok(object) => object,
            // §7.3 has no `TypeIdentifier` for `fixed`, a `valuetype`, a
            // value box, `native`, `any`, `ValueBase` or an object
            // reference, and no single-type identifier for a recursive
            // type. A case whose topic type is one of those has no wire
            // form to come back from, and the refusal is the answer
            // rather than the failure.
            Err(
                FromAstError::Unrepresentable { .. }
                | FromAstError::Recursive { .. }
                | FromAstError::OutOfRange { .. },
            ) => {
                unrepresentable.insert(case.name.clone());
                continue;
            }
            Err(error) => panic!("{}: {error}", case.name),
        };
        let mut registry = TypeRegistry::new();
        let skipped = match register_all(&ast, &mut registry) {
            Ok(skipped) => skipped,
            // One type in the file has no wire form and the topic type
            // may not need it. The rebuild below is what says whether it
            // did: an identifier it cannot resolve is the loud form.
            Err(FromAstError::OutOfRange { .. }) => {
                unrepresentable.insert(case.name.clone());
                continue;
            }
            Err(error) => panic!("{}: {error}", case.name),
        };
        // What `register_all` skipped is a type XTypes cannot carry. If
        // the topic type needed one of them, the rebuild below reports
        // it as an unresolvable identifier, which is the loud form.
        let _ = skipped;

        let rebuilt = match zerodds_types::dynamic::dynamic_type(&object, &registry) {
            Ok(view) => view,
            Err(error) => {
                moved.push(format!(
                    "  {}: the object does not rebuild: {error}",
                    case.name
                ));
                continue;
            }
        };
        let rebuilt_ast = rebuilt.ast().clone();
        let rebuilt_name = rebuilt.qualified_name();

        for (encoding, endianness) in MODES {
            let request = ProbeRequest::new(&case.type_name, encoding, endianness);
            let Ok(bytes) = encode::<Value>(&ast, &request, &case.values) else {
                // The two samples the value contract refuses; see
                // `dynamic_corpus.rs::REFUSED`.
                continue;
            };

            // The same bytes, read against the model that came back out
            // of the wire form and against nothing else.
            let rebuilt_request = ProbeRequest::new(&rebuilt_name, encoding, endianness);
            let data: DynamicData = match decode(&rebuilt_ast, &rebuilt_request, &bytes) {
                Ok(value) => value,
                Err(error) => {
                    moved.push(format!(
                        "  {} {encoding:?}/{endianness:?}: the rebuilt type does not read the \
                         bytes: {error}",
                        case.name
                    ));
                    continue;
                }
            };
            let native: DynamicData = decode(&ast, &request, &bytes)
                .unwrap_or_else(|error| panic!("{} {encoding:?}: {error}", case.name));

            match (
                encode::<DynamicData>(&rebuilt_ast, &rebuilt_request, &data),
                encode::<DynamicData>(&ast, &request, &native),
            ) {
                (Ok(left), Ok(right)) if left == right => compared += 1,
                (Ok(left), Ok(right)) => moved.push(format!(
                    "  {} {encoding:?}/{endianness:?}: the rebuilt type wrote {}, the lowered \
                     type wrote {}",
                    case.name,
                    hex(&left),
                    hex(&right)
                )),
                (left, right) => moved.push(format!(
                    "  {} {encoding:?}/{endianness:?}: rebuilt {left:?}, lowered {right:?}",
                    case.name
                )),
            }
        }
    }

    assert!(
        moved.is_empty(),
        "a type does not survive its own wire form in {} places:\n{}",
        moved.len(),
        moved.join("\n")
    );
    // A floor and not an equality, so that a corpus case added later
    // does not turn this red; a *drop* is the walk quietly doing less.
    // 1,316 when this was written: 331 cases with a wire form, in four
    // modes, less the eight slots of the two samples the value contract
    // refuses.
    assert!(
        compared >= 1316,
        "{compared} comparisons, and 1316 were made when this was written"
    );

    // Exact, so that a construct that gains a wire form — or loses one —
    // is named rather than absorbed.
    let expected: BTreeSet<String> = UNREPRESENTABLE
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    assert_eq!(
        unrepresentable, expected,
        "the set of cases XTypes §7.3 has no TypeIdentifier for moved"
    );
}

/// The cases whose type has no `CompleteTypeObject` to come back from.
///
/// Two reasons, both XTypes §7.3's own and both refused by name in
/// `crate::from_ast`. Listed rather than counted: a case that gains a
/// wire form is a gap that closed and has to be seen.
///
/// **`fixed<P,S>` has no kind.** §7.3.4.2 tabulates no fixed-point type,
/// the IDL-to-type-system table of §7.3.1 maps none, and DDSXTY14-28 is
/// the report asking for one. Ten cases: the eight `fixed-*` and the two
/// collections of one.
///
/// **A case label wider than an `i32`.** §7.3.4.5.3.2 writes
/// `CommonUnionMember::label_seq` as a sequence of `int32`, so a union
/// switching on `unsigned long`, `long long` or `unsigned long long`
/// cannot announce a label outside that range — `union-discriminator-
/// ulong` labels on `0xFFFFFFFF`. The type is serialisable and its
/// *description* is not, which is a hole in §7.3 and not in this tree:
/// the six cases still encode and decode, and `dynamic_corpus.rs`
/// measures them.
// `recursive-union-branch` is unrepresentable today and should not be:
// the strongly-connected-component identifier covers a struct that
// reaches itself, and a recursion whose back edge runs through a union
// branch does not get one from `from_ast` yet. Measured on this tree;
// the import that brought the case pinned this list without it. Same
// family as the docs/OPEN-ITEMS.md `@external` entry. Cyclone's own
// front end does not terminate on this case either (89 CPU minutes,
// 12.8G RSS before the kill), which is a vendor finding, not a licence.
const UNREPRESENTABLE: [&str; 17] = [
    "array-1d-fixed",
    "array-1d-fixed-unannotated",
    "fixed-basic",
    "fixed-basic-unannotated",
    "fixed-scale-equals-digits",
    "fixed-scale-equals-digits-unannotated",
    "fixed-scales",
    "fixed-scales-unannotated",
    "recursive-union-branch",
    "sequence-fixed",
    "sequence-fixed-unannotated",
    "union-discriminator-longlong",
    "union-discriminator-longlong-unannotated",
    "union-discriminator-ulong",
    "union-discriminator-ulong-unannotated",
    "union-discriminator-ulonglong",
    "union-discriminator-ulonglong-unannotated",
];
