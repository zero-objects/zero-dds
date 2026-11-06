// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Out-of-band type source: parse an IDL file and resolve a named type to a
//! [`DynamicType`] for the reflective codec (the `--type-file` path).
//!
//! This used to lower the IDL AST to `DynamicType` by hand, one construct at
//! a time, because a `DynamicType` was something a builder constructed and
//! the only route from IDL emitted Minimal TypeObjects, which carry name
//! hashes rather than names. Seven hundred lines of that translation existed
//! to bridge two representations of the same thing, and it was incomplete
//! where the translation ran out rather than where IDL did: `map` and
//! fixed-point were refused as `Unsupported`.
//!
//! Neither is true now. `zerodds-idl-parse` and `zerodds-idl-lower` produce
//! an [`Ast`], and a `DynamicType` **is** a view into an `Ast` — so resolving
//! a name is an index lookup, and every construct the frontend lowers is
//! reachable without a line here naming it. What this module still decides is
//! the two things a file reader must: which dialect to admit, and what counts
//! as bad enough to refuse.

use std::sync::Arc;

use zerodds_ast::Ast;
use zerodds_idl_parse::Severity;
use zerodds_types::dynamic::DynamicType;

/// Type-source error.
#[derive(Debug)]
pub enum TypeSourceError {
    /// IDL parse or lowering failure.
    Parse(String),
    /// The requested type name was not found in the IDL.
    NotFound(String),
}

impl core::fmt::Display for TypeSourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(m) => write!(f, "idl parse: {m}"),
            Self::NotFound(m) => write!(f, "type not found: {m}"),
        }
    }
}
impl std::error::Error for TypeSourceError {}

type R<T> = Result<T, TypeSourceError>;

/// A parsed IDL document. Every type it declares is resolvable by scoped
/// name, including the ones only named from inside another.
pub struct TypeBook {
    ast: Arc<Ast>,
}

impl TypeBook {
    /// Parse IDL source and build the model behind it.
    ///
    /// Through `zerodds-idl-compose`, the one load path a `build.rs`,
    /// `zerodds-idlc` and the wire probe take. This parsed and lowered on
    /// its own and ran no resolve pass, so a reference the lowering leaves
    /// open — a member typed with a scoped name such as `telemetry::Unit` —
    /// stayed unresolved: the file loaded, and every sample of the type was
    /// then dropped at decode with `NoLayout { "… has an unresolved
    /// reference" }`, a warning per sample and no decoded value.
    ///
    /// `ComposeOptions::default()` admits every vendor extension, which is
    /// what a tool reading files it did not write needs: a `--type-file`
    /// comes from whoever runs the recorder, and refusing a vendor extension
    /// here would refuse the recording rather than the extension.
    ///
    /// # Errors
    /// [`TypeSourceError::Parse`], carrying every error-severity diagnostic
    /// of every stage, an unresolved name among them. Warnings do not stop
    /// it.
    pub fn from_idl(src: &str) -> R<Self> {
        let composed = zerodds_idl_compose::compose_source_with(
            "<type-file>",
            src,
            &zerodds_idl_compose::PathLoader::bare(),
            zerodds_idl_compose::ComposeOptions::default(),
        );
        let errors: Vec<String> = composed
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message.clone())
            .collect();
        if !errors.is_empty() {
            return Err(TypeSourceError::Parse(errors.join("; ")));
        }
        Ok(Self {
            ast: Arc::new(composed.ast),
        })
    }

    /// Resolve a type name — `a::B::C`, `::a::B::C`, or the simple `C`.
    ///
    /// A simple name is accepted because that is what a command line carries:
    /// `--type Track`, not `--type ::cuas::Track`. It resolves when exactly
    /// one declared type ends in it; two make it ambiguous and the error says
    /// how many, because guessing between them would decode the samples of
    /// one topic as the type of another.
    ///
    /// # Errors
    /// [`TypeSourceError::NotFound`] when the model declares no such type,
    /// when the name is ambiguous, and equally when it resolves to something
    /// that is not a type.
    pub fn resolve(&self, name: &str) -> R<DynamicType> {
        if let Ok(t) = DynamicType::find(&self.ast, name) {
            return Ok(t);
        }
        let want = name.trim_start_matches("::");
        let suffix = format!("::{want}");
        let matches: Vec<String> = self
            .ast
            .types()
            .filter_map(|(id, node)| {
                self.ast
                    .scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
            })
            .map(|n| n.segments().join("::"))
            .filter(|n| n == want || n.ends_with(&suffix))
            .collect();
        match matches.as_slice() {
            [one] => DynamicType::find(&self.ast, one)
                .map_err(|_| TypeSourceError::NotFound(name.to_string())),
            [] => Err(TypeSourceError::NotFound(name.to_string())),
            many => Err(TypeSourceError::NotFound(format!(
                "{name} is ambiguous ({} matches)",
                many.len()
            ))),
        }
    }

    /// The model every resolved type is a view into.
    #[must_use]
    pub const fn ast(&self) -> &Arc<Ast> {
        &self.ast
    }
}

/// Convenience: parse `idl_src` and resolve `type_name` to a [`DynamicType`].
///
/// # Errors
/// See [`TypeSourceError`].
pub fn dynamic_type_from_idl(idl_src: &str, type_name: &str) -> R<DynamicType> {
    TypeBook::from_idl(idl_src)?.resolve(type_name)
}

/// Parse `topic=Type` mappings (e.g. from repeated `--map` flags).
///
/// # Errors
/// [`TypeSourceError::Parse`] on a malformed entry.
pub fn parse_topic_type_map(entries: &[String]) -> R<Vec<(String, String)>> {
    entries
        .iter()
        .map(|e| {
            e.split_once('=')
                .map(|(t, ty)| (t.trim().to_string(), ty.trim().to_string()))
                .ok_or_else(|| TypeSourceError::Parse(format!("bad topic=Type mapping: {e}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use zerodds_types::dynamic::descriptor::TypeKind;

    const SHAPE: &str = r"
        module demo {
            @appendable struct ShapeType {
                string color;
                long x;
                long y;
                long shapesize;
            };
        };
    ";

    /// IDL 4.2 §7.5.2: a scoped name names the type through its module,
    /// with or without the leading `::`; a name nobody declared and a
    /// module are no type.
    #[test]
    fn idl_7_5_2_a_scoped_name_names_a_type_and_a_module_is_none() {
        let t = dynamic_type_from_idl(SHAPE, "demo::ShapeType").expect("resolve");
        assert_eq!((t.name(), t.member_count()), ("ShapeType", 4));
        let absolute = dynamic_type_from_idl(SHAPE, "::demo::ShapeType").expect("absolute");
        assert_eq!(absolute.member_count(), 4);
        for name in ["demo::Nope", "demo"] {
            let e = dynamic_type_from_idl(SHAPE, name).expect_err(name);
            assert!(matches!(e, TypeSourceError::NotFound(_)), "{name}: {e:?}");
        }
    }

    /// A map, a fixed-point member and a union resolve to their types (the
    /// first two the hand-written lowering of before refused); the union
    /// is of kind UNION.
    #[test]
    fn a_map_a_fixed_and_a_union_resolve_to_their_types() {
        let idl = r"
            module demo {
                struct WithMap {
                    map<string, long> by_name;
                };
                struct WithFixed {
                    fixed<9,2> amount;
                };
                union U switch (long) {
                    case 1: long a;
                    default: string b;
                };
            };
        ";
        let map = dynamic_type_from_idl(idl, "demo::WithMap").expect("map type");
        assert_eq!(map.member_count(), 1);
        let fixed = dynamic_type_from_idl(idl, "demo::WithFixed").expect("fixed type");
        assert_eq!(fixed.member_count(), 1);
        let union = dynamic_type_from_idl(idl, "demo::U").expect("union");
        assert_eq!(union.kind(), TypeKind::Union);
    }

    /// A member typed with a scoped name resolves at load, and a sample of
    /// the type decodes. The bytes are what a ZeroDDS writer of
    /// `decoded-recorder`'s `sensor.idl` sent (XCDR1, little-endian):
    /// before, the file loaded and every such sample failed with
    /// `NoLayout { "type NodeId(7) has an unresolved reference" }`.
    #[test]
    fn xtypes_7_4_1_1_a_member_typed_with_a_scoped_name_decodes_from_the_xcdr1_octets() {
        use zerodds_ast_layout::{Dialect, Encoding, Endianness};
        use zerodds_types::dynamic::DynamicData;
        use zerodds_wire_codec::ProbeRequest;
        let idl = r"
            module telemetry {
              enum Unit { CELSIUS, KELVIN, FAHRENHEIT };
              @appendable struct Reading {
                @key long       sensor_id;
                string          name;
                telemetry::Unit unit;
                double          value;
                sequence<long>  history;
              };
            };
        ";
        let book = TypeBook::from_idl(idl).expect("load");
        let ty = book.resolve("telemetry::Reading").expect("resolve");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&7i32.to_le_bytes());
        bytes.extend_from_slice(&9u32.to_le_bytes());
        bytes.extend_from_slice(b"intake-0\0\0\0\0");
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&21.5f64.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        for v in [20i32, 21, 22] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(bytes.len(), 48);
        let request = ProbeRequest::in_dialect(
            "telemetry::Reading",
            Encoding::PlainCdr1,
            Endianness::Little,
            Dialect::default(),
        );
        let value =
            zerodds_wire_codec::decode::<DynamicData>(ty.ast(), &request, &bytes).expect("decode");
        assert!(
            matches!(
                value.member("sensor_id"),
                Some(DynamicData::Integer { value: 7, .. })
            ),
            "{value:?}"
        );
        assert!(value.member("unit").is_some(), "{value:?}");
    }

    /// A file that does not compile is refused with the reason: a syntax
    /// error, and a reference that resolves nowhere, by name, instead of a
    /// model every sample of the type then fails against.
    #[test]
    fn a_file_that_does_not_compile_is_refused_with_the_reason() {
        let Err(e) = TypeBook::from_idl("struct Broken { long ;") else {
            panic!("a syntax error must not produce a type book")
        };
        assert!(matches!(e, TypeSourceError::Parse(_)), "got {e:?}");
        let Err(e) = TypeBook::from_idl("module m { struct S { m::Missing x; }; };") else {
            panic!("an unresolved reference must not produce a type book")
        };
        let TypeSourceError::Parse(text) = e else {
            panic!("got {e:?}")
        };
        assert!(text.contains("Missing"), "{text}");
    }

    /// `zerodds-record(1)`: `--map TOPIC=Type`, split at the first `=`, the
    /// two sides trimmed; without `=` it is refused.
    #[test]
    fn a_map_option_splits_at_the_first_equals_sign() {
        let got = parse_topic_type_map(&[String::from(" Square = demo::ShapeType ")]).expect("map");
        assert_eq!(
            got,
            vec![(String::from("Square"), String::from("demo::ShapeType"))]
        );
        parse_topic_type_map(&[String::from("no-equals")]).expect_err("malformed");
    }
}
