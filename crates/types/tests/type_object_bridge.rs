// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeObjects of the interop corpus against the ones CycloneDDS writes.
//!
//! `tests/interop-corpus/cyclone-type-objects.tsv` holds the MINIMAL and
//! COMPLETE TypeObjects Cyclone's `idlc` writes for the type of every corpus
//! case it compiles, with their TypeIdentifiers
//! (`tools/expected-bytes/type_objects.py`, from one commit of
//! `zerodds-matrix`), and `tests/interop-corpus/expected-bytes.tsv` the
//! octets Cyclone writes for each case's sample. Both are Cyclone's own
//! output; nothing here compares this project's encoder with its own
//! decoder.
//!
//! - The TypeIdentifiers this project computes for a case's type, from its
//!   IDL read as Cyclone's `idlc` reads it, are Cyclone's (XTypes 1.3
//!   §7.3.4.6.5, §7.3.4.6.6: the MD5 of the serialized TypeObject,
//!   §7.3.4.5), but where a construct of the case parts them: the four
//!   readings [`Constructs`] names, each with its clause.
//! - The model rebuilt from Cyclone's COMPLETE TypeObject and its
//!   dependencies (`dynamic::dynamic_type`) writes the case's sample as
//!   Cyclone does (§7.3.4.3: back to IDL without loss): every property of
//!   the type that reaches the wire — extensibility, member ids, `@optional`,
//!   `@key`, bounds, dimensions, enumerator values, bit positions — has to
//!   come back from the TypeObject for the octets to be the same. In XCDR2
//!   and XCDR1, little endian: the byte order is no property of a type.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;
use zerodds_ast::{Ast, Extensibility, TypeKind};
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_types::from_ast::{FromAstError, complete_type_object, minimal_type_object};
use zerodds_types::hash::{compute_complete_hash, compute_minimal_hash, hash_bytes};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::EquivalenceHash;
use zerodds_types::{CompleteTypeObject, TypeObject};
use zerodds_wire_codec::encode;
use zerodds_wire_codec::value::ProbeRequest;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/interop-corpus")
        .canonicalize()
        .expect("tests/interop-corpus")
}

fn unhex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "odd hex length");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn hash_of(text: &str) -> EquivalenceHash {
    EquivalenceHash(unhex(text).try_into().expect("a 14-octet hash"))
}

/// Cyclone's TypeObjects of one case.
#[derive(Default)]
struct CycloneTypes {
    topic_minimal: Option<EquivalenceHash>,
    topic_complete: Option<EquivalenceHash>,
    /// Every COMPLETE object of the case's mapping, the topic type's among
    /// them, by its identifier.
    complete: BTreeMap<[u8; 14], CompleteTypeObject>,
}

/// `cyclone-type-objects.tsv`, by case. Each COMPLETE object is read back
/// from Cyclone's octets and its identifier checked: MD5 of the octets.
fn cyclone_types() -> BTreeMap<String, CycloneTypes> {
    let path = corpus_root().join("cyclone-type-objects.tsv");
    let text = std::fs::read_to_string(&path).expect("cyclone-type-objects.tsv");
    let mut out: BTreeMap<String, CycloneTypes> = BTreeMap::new();
    for line in text.lines().filter(|line| !line.starts_with('#')).skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 5, "{line}");
        let (case, equivalence, role, hash, octets) =
            (fields[0], fields[1], fields[2], fields[3], fields[4]);
        let types = out.entry(case.to_string()).or_default();
        let hash = hash_of(hash);
        match (equivalence, role) {
            ("minimal", "topic") => types.topic_minimal = Some(hash),
            ("complete", "topic") => types.topic_complete = Some(hash),
            _ => {}
        }
        if equivalence == "complete" {
            let octets = unhex(octets);
            assert_eq!(hash_bytes(&octets), hash, "{case}: MD5 of the octets");
            let object = match TypeObject::from_bytes_le(&octets) {
                Ok(TypeObject::Complete(object)) => object,
                other => panic!(
                    "{case}: Cyclone's COMPLETE object {}: {other:?}",
                    hex(&hash.0)
                ),
            };
            types.complete.insert(hash.0, object);
        }
    }
    out
}

/// Cyclone's octets of a case's sample, little endian, by case and
/// encoding (`expected-bytes.tsv`, the rows of the `cyclone` dialect).
fn cyclone_octets() -> BTreeMap<(String, Encoding), Vec<u8>> {
    let path = corpus_root().join("expected-bytes.tsv");
    let text = std::fs::read_to_string(&path).expect("expected-bytes.tsv");
    let mut out = BTreeMap::new();
    for line in text.lines().filter(|line| !line.starts_with('#')).skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 6, "{line}");
        if fields[1] != "values"
            || fields[3] != "le"
            || !fields[4].split(',').any(|dialect| dialect == "cyclone")
        {
            continue;
        }
        let encoding = match fields[2] {
            "xcdr1" => Encoding::PlainCdr1,
            "xcdr2" => Encoding::PlainCdr2,
            other => panic!("encoding {other}"),
        };
        out.insert((fields[0].to_string(), encoding), unhex(fields[5]));
    }
    out
}

fn read(case: &str, file: &str) -> String {
    let path = corpus_root().join(case).join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{case}/{file}: {error}"))
}

/// The case's IDL read as Cyclone's `idlc` reads it: a type whose
/// extensibility the source does not say is `@final`, its default (XTypes
/// 1.3 §7.3.1.2.1.8 leaves the default to the IDL compiler's options; the
/// flags of Cyclone's objects say `IS_FINAL` for every such type,
/// enumerations and bitmasks included). The front end's
/// `default_extensibility` fills every type that carries one.
fn model_as_cyclone_reads_it(case: &str) -> Ast {
    let idl = read(case, "type.idl");
    let parsed = zerodds_idl_parse::parse(&idl);
    assert!(!parsed.has_errors(), "{case}: {:?}", parsed.diagnostics());
    let options = zerodds_idl_lower::Options {
        default_extensibility: Some(Extensibility::Final),
        ..zerodds_idl_lower::Options::default()
    };
    let lowered =
        zerodds_idl_lower::lower_with(&parsed.syntax(), &format!("{case}/type.idl"), options);
    assert!(
        lowered.diagnostics.is_empty(),
        "{case}: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast
        .resolve_all_by_name()
        .unwrap_or_else(|error| panic!("{case}: {error}"));
    assert!(report.is_clean(), "{case}: {:?}", report.broken);
    ast
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

/// Where this project's TypeObjects and Cyclone's part, by the construct
/// in the IDL, measured over the corpus. Each is a reading of XTypes 1.3
/// that this project follows and Cyclone's `idlc` does not:
///
/// - a union: the discriminator's `IS_MUST_UNDERSTAND`, which §7.2.2.4.4.4.6
///   says "shall always" be set (`docs/omg-xtypes-issues.md` §B2: no
///   measured implementation sets it). Both forms.
/// - a `@key` member: its `IS_MUST_UNDERSTAND`, §7.2.2.4.4.4.8: key members
///   "shall always have their must understand attribute set to true".
///   Both forms.
/// - a `@hashid` member: the COMPLETE object carries it in
///   `AppliedBuiltinMemberAnnotations::hash_id` (Annex B), so that the type
///   goes back to its IDL (§7.3.4.3); Cyclone writes none.
/// - `@nested`, `@default_nested` or `@autoid(HASH)`: the MINIMAL object's
///   type flags are masked with Annex B's `TypeFlagMinimalMask`, which
///   takes `IS_NESTED` and `IS_AUTOID_HASH` out; Cyclone keeps them.
struct Constructs {
    union: bool,
    key: bool,
    hashid: bool,
    nested_or_autoid_hash: bool,
}

impl Constructs {
    fn of(ast: &Ast, idl: &str) -> Self {
        let text: String = idl
            .lines()
            .map(|line| line.split("//").next().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        Self {
            union: ast
                .types()
                .any(|(_, node)| matches!(node.kind, TypeKind::Union { .. })),
            key: text.contains("@key") || text.contains("#pragma keylist"),
            hashid: text.contains("@hashid"),
            nested_or_autoid_hash: text.contains("@nested")
                || text.contains("@default_nested")
                || compact.contains("@autoid(HASH)"),
        }
    }

    fn complete_differs(&self) -> bool {
        self.union || self.key || self.hashid
    }

    fn minimal_differs(&self) -> bool {
        self.union || self.key || self.nested_or_autoid_hash
    }
}

#[test]
fn xtypes_7_3_4_6_the_type_identifiers_of_the_corpus_types_are_cyclones() {
    let cyclone = cyclone_types();
    assert!(cyclone.len() >= 333, "{} cases", cyclone.len());
    let mut moved: Vec<String> = Vec::new();
    let mut same = 0usize;
    for (case, types) in &cyclone {
        let ast = model_as_cyclone_reads_it(case);
        let type_name = read(case, "type").trim().to_string();
        let Some(type_) = find(&ast, &type_name) else {
            panic!("{case}: `{type_name}` is not in the model");
        };
        let complete = complete_type_object(&ast, type_)
            .map_err(|error: FromAstError| error.to_string())
            .and_then(|object| compute_complete_hash(&object).map_err(|e| format!("{e:?}")))
            .unwrap_or_else(|error| panic!("{case}: our COMPLETE TypeObject: {error}"));
        let minimal = minimal_type_object(&ast, type_)
            .map_err(|error: FromAstError| error.to_string())
            .and_then(|object| compute_minimal_hash(&object).map_err(|e| format!("{e:?}")))
            .unwrap_or_else(|error| panic!("{case}: our MINIMAL TypeObject: {error}"));
        let constructs = Constructs::of(&ast, &read(case, "type.idl"));
        for (form, ours, theirs, differs) in [
            (
                "COMPLETE",
                complete,
                types.topic_complete,
                constructs.complete_differs(),
            ),
            (
                "MINIMAL",
                minimal,
                types.topic_minimal,
                constructs.minimal_differs(),
            ),
        ] {
            let theirs = theirs.expect("Cyclone's identifier");
            match (ours == theirs, differs) {
                (true, false) => same += 1,
                (false, true) => {}
                (true, true) => moved.push(format!(
                    "  {case} {form}: Cyclone's identifier, where a construct of the case \
                     should part them"
                )),
                (false, false) => moved.push(format!(
                    "  {case} {form}: ours {}, Cyclone's {}",
                    hex(&ours.0),
                    hex(&theirs.0)
                )),
            }
        }
    }
    assert!(
        moved.is_empty(),
        "{} identifiers moved against Cyclone's:\n{}",
        moved.len(),
        moved.join("\n")
    );
    // 263 COMPLETE and 266 MINIMAL identifiers of 333 cases the same as
    // Cyclone's when this was written.
    assert!(same >= 529, "{same} identifiers the same as Cyclone's");
}

#[test]
fn xtypes_7_3_4_3_a_type_rebuilt_from_cyclones_type_object_writes_cyclones_octets() {
    let cyclone = cyclone_types();
    let octets = cyclone_octets();
    let mut compared = 0usize;
    let mut moved: Vec<String> = Vec::new();
    for (case, types) in &cyclone {
        let topic = types.topic_complete.expect("a topic type");
        let mut registry = TypeRegistry::new();
        for (hash, object) in &types.complete {
            registry.insert_complete(EquivalenceHash(*hash), object.clone());
        }
        let object = &types.complete[&topic.0];
        let rebuilt = match zerodds_types::dynamic::dynamic_type(object, &registry) {
            Ok(view) => view,
            Err(error) => {
                moved.push(format!(
                    "  {case}: Cyclone's object does not rebuild: {error}"
                ));
                continue;
            }
        };
        let values: Value = serde_json::from_str(&read(case, "values.json"))
            .unwrap_or_else(|error| panic!("{case}/values.json: {error}"));
        for encoding in [Encoding::PlainCdr2, Encoding::PlainCdr1] {
            let Some(expected) = octets.get(&(case.clone(), encoding)) else {
                continue;
            };
            let request = ProbeRequest::in_dialect(
                &rebuilt.qualified_name(),
                encoding,
                Endianness::Little,
                Dialect::Cyclone,
            );
            match encode::<Value>(rebuilt.ast(), &request, &values) {
                Ok(written) if &written == expected => compared += 1,
                Ok(written) => moved.push(format!(
                    "  {case} {encoding:?}: the rebuilt type wrote {}, Cyclone {}",
                    hex(&written),
                    hex(expected)
                )),
                Err(error) => moved.push(format!(
                    "  {case} {encoding:?}: the rebuilt type does not write the sample: {error}"
                )),
            }
        }
    }
    assert!(
        moved.is_empty(),
        "{} of Cyclone's cells are not written by the type rebuilt from its TypeObject:\n{}",
        moved.len(),
        moved.join("\n")
    );
    // 630 when this was written: the little-endian cells of the 333 cases.
    assert!(compared >= 630, "{compared} cells compared");
}
