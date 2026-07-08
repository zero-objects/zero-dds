// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Out-of-band type source: parse an IDL file and lower a named type to a
//! [`DynamicType`] for the reflective codec (the `--type-file` path).
//!
//! Lowers the IDL AST **directly** to `DynamicType` (the AST→TypeObject mapper
//! emits Minimal TypeObjects, which the TypeObject→DynamicType bridge does not
//! yet resolve). Covered: struct (extensibility from `@final`/`@appendable`/
//! `@mutable`, default appendable), nested struct, primitives, `string`,
//! `wstring`, `sequence` (incl. `sequence<Struct>` via a resolved element type),
//! fixed array (incl. arrays of struct), enum (mapped to its int32 wire form),
//! and `bitmask`/`bitset` (lowered to their packed unsigned wire integer per
//! `@bit_bound` / total bitfield width), and `union` members + standalone union
//! topics (discriminator at its switch kind; cases carry labels + `default`,
//! feeding the reflective PL_CDR2 union codec). Residual (`Unsupported`): map,
//! fixed-point.

use std::collections::BTreeMap;

use zerodds_idl::ast::types::{Annotation, FloatingType, IntegerType};
use zerodds_idl::ast::types::{
    CaseLabel, ConstExpr, ConstrTypeDecl, Definition, Literal, LiteralKind, ModuleDef,
    PrimitiveType, Specification, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnionDef,
};
use zerodds_idl::config::ParserConfig;
use zerodds_types::dynamic::builder::DynamicTypeBuilderFactory as F;
use zerodds_types::dynamic::collection;
use zerodds_types::dynamic::descriptor::{
    ExtensibilityKind, MemberDescriptor, TypeDescriptor, TypeKind,
};
use zerodds_types::dynamic::type_::DynamicType;

/// Type-source error.
#[derive(Debug)]
pub enum TypeSourceError {
    /// IDL parse failure.
    Parse(String),
    /// The requested type name was not found in the IDL.
    NotFound(String),
    /// A construct the lowering does not yet support.
    Unsupported(String),
    /// A `DynamicType` builder failure.
    Build(String),
}

impl core::fmt::Display for TypeSourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(m) => write!(f, "idl parse: {m}"),
            Self::NotFound(m) => write!(f, "type not found: {m}"),
            Self::Unsupported(m) => write!(f, "unsupported: {m}"),
            Self::Build(m) => write!(f, "build: {m}"),
        }
    }
}
impl std::error::Error for TypeSourceError {}

type R<T> = Result<T, TypeSourceError>;

/// A parsed IDL document, indexing every named struct/enum/bitmask/bitset by
/// qualified name so types can be resolved (incl. nested references) on demand.
pub struct TypeBook {
    structs: BTreeMap<String, StructDef>,
    enums: BTreeMap<String, ()>, // name → exists (lowered to int32 wire form)
    /// bitmask name → holder width in bits (`@bit_bound`, default 32) — lowered
    /// to the matching unsigned wire integer.
    bitmasks: BTreeMap<String, u32>,
    /// bitset name → total declared bits — lowered to the packed wire integer.
    bitsets: BTreeMap<String, u32>,
    /// union name → its definition (built on demand into a union `DynamicType`).
    unions: BTreeMap<String, UnionDef>,
}

impl TypeBook {
    /// Parse IDL source and index its named types.
    ///
    /// # Errors
    /// [`TypeSourceError::Parse`] on a syntax error.
    pub fn from_idl(src: &str) -> R<Self> {
        let spec = zerodds_idl::parse(src, &ParserConfig::default())
            .map_err(|e| TypeSourceError::Parse(format!("{e:?}")))?;
        let mut book = Self {
            structs: BTreeMap::new(),
            enums: BTreeMap::new(),
            bitmasks: BTreeMap::new(),
            bitsets: BTreeMap::new(),
            unions: BTreeMap::new(),
        };
        book.index_spec(&spec, "");
        Ok(book)
    }

    fn index_spec(&mut self, spec: &Specification, prefix: &str) {
        for d in &spec.definitions {
            self.index_def(d, prefix);
        }
    }

    fn index_module(&mut self, m: &ModuleDef, prefix: &str) {
        let p = qualify(prefix, &m.name.text);
        for d in &m.definitions {
            self.index_def(d, &p);
        }
    }

    fn index_def(&mut self, d: &Definition, prefix: &str) {
        match d {
            Definition::Module(m) => self.index_module(m, prefix),
            Definition::Type(TypeDecl::Constr(c)) => match c {
                ConstrTypeDecl::Struct(sd) => {
                    if let zerodds_idl::ast::types::StructDcl::Def(s) = sd {
                        self.structs
                            .insert(qualify(prefix, &s.name.text), s.clone());
                    }
                }
                ConstrTypeDecl::Enum(e) => {
                    self.enums.insert(qualify(prefix, &e.name.text), ());
                }
                ConstrTypeDecl::Bitmask(m) => {
                    self.bitmasks
                        .insert(qualify(prefix, &m.name.text), bit_bound_of(&m.annotations));
                }
                ConstrTypeDecl::Bitset(bs) => {
                    self.bitsets
                        .insert(qualify(prefix, &bs.name.text), bitset_total_bits(bs));
                }
                ConstrTypeDecl::Union(ud) => {
                    if let zerodds_idl::ast::types::UnionDcl::Def(u) = ud {
                        self.unions.insert(qualify(prefix, &u.name.text), u.clone());
                    }
                }
            },
            _ => {}
        }
    }

    /// Resolve `name` (simple or qualified `A::B`) to a [`DynamicType`].
    ///
    /// # Errors
    /// [`TypeSourceError`] when the name is unknown or the type uses an
    /// unsupported construct.
    pub fn resolve(&self, name: &str) -> R<DynamicType> {
        if let Ok(key) = self.find_struct_key(name) {
            let s = &self.structs[&key];
            return self.build_struct(s);
        }
        // Standalone union topic.
        let want = name.trim_start_matches("::");
        if let Some((_, u)) = self.unions.iter().find(|(k, _)| name_matches(k, want)) {
            return self.build_union(u);
        }
        Err(TypeSourceError::NotFound(name.to_string()))
    }

    /// Find the indexed qualified key whose tail matches `name` (which may be
    /// simple or qualified). Errors if absent or ambiguous.
    fn find_struct_key(&self, name: &str) -> R<String> {
        let want = name.trim_start_matches("::");
        // exact qualified match first
        if self.structs.contains_key(want) {
            return Ok(want.to_string());
        }
        let matches: Vec<&String> = self
            .structs
            .keys()
            .filter(|k| *k == want || k.ends_with(&format!("::{want}")) || k.as_str() == want)
            .collect();
        match matches.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(TypeSourceError::NotFound(name.to_string())),
            _ => Err(TypeSourceError::NotFound(format!(
                "{name} is ambiguous ({} matches)",
                matches.len()
            ))),
        }
    }

    fn build_struct(&self, s: &StructDef) -> R<DynamicType> {
        let mut desc = TypeDescriptor::structure(s.name.text.clone());
        desc.extensibility_kind = struct_extensibility(s);
        let mut b = F::create_type(desc).map_err(|e| TypeSourceError::Build(e.to_string()))?;
        let mut id: u32 = 0;
        for m in &s.members {
            for decl in &m.declarators {
                let member_name = decl.name().text.clone();
                let array_sizes = array_sizes_of(decl);
                self.add_member(&mut b, &member_name, id, &m.type_spec, &array_sizes)?;
                id += 1;
            }
        }
        b.build().map_err(|e| TypeSourceError::Build(e.to_string()))
    }

    /// Resolve a struct member to its fully-resolved [`DynamicType`] (recursing
    /// into nested structs, sequences and composite collection elements) and
    /// attach it via `add_member_resolved` so the codec can walk it.
    fn add_member(
        &self,
        b: &mut zerodds_types::dynamic::builder::DynamicTypeBuilder,
        name: &str,
        id: u32,
        ts: &TypeSpec,
        array_sizes: &[u32],
    ) -> R<()> {
        let base = self.resolve_member_type(ts, name)?;
        let ty = if array_sizes.is_empty() {
            base
        } else {
            collection::array_of(base, array_sizes.to_vec(), name)
        };
        let mut md = MemberDescriptor::new(name, id, ty.descriptor().clone());
        md.index = id;
        b.add_member_resolved(md, ty)
            .map_err(|e| TypeSourceError::Build(e.to_string()))
    }

    /// Lower a (non-array) `TypeSpec` to a fully-resolved [`DynamicType`].
    fn resolve_member_type(&self, ts: &TypeSpec, name: &str) -> R<DynamicType> {
        match ts {
            TypeSpec::Primitive(p) => {
                build_scalar(TypeDescriptor::primitive(primitive_kind(*p), "prim"))
            }
            TypeSpec::String(s) => {
                let bound = s.bound.as_ref().and_then(const_to_u32).unwrap_or(0);
                Ok(if s.wide {
                    F::create_wstring_type(bound)
                } else {
                    F::create_string_type(bound)
                })
            }
            TypeSpec::Sequence(seq) => {
                let elem = self.resolve_member_type(&seq.elem, name)?;
                let bound = seq.bound.as_ref().and_then(const_to_u32).unwrap_or(0);
                Ok(collection::sequence_of(elem, bound))
            }
            TypeSpec::Scoped(sn) => {
                let tail = sn.parts.last().map(|i| i.text.as_str()).unwrap_or("");
                self.resolve_scoped(tail)
            }
            TypeSpec::Map(m) => {
                let key = self.resolve_member_type(&m.key, name)?;
                let value = self.resolve_member_type(&m.value, name)?;
                let bound = m.bound.as_ref().and_then(const_to_u32).unwrap_or(0);
                Ok(collection::map_of(key, value, bound, name))
            }
            TypeSpec::Fixed(_) | TypeSpec::Any => Err(TypeSourceError::Unsupported(format!(
                "fixed/any member {name}"
            ))),
        }
    }

    /// Resolve a scoped (named) type reference: struct → recursive build; enum →
    /// int32 wire form; bitmask/bitset → their packed unsigned wire integer.
    fn resolve_scoped(&self, tail: &str) -> R<DynamicType> {
        if let Ok(key) = self.find_struct_key(tail) {
            let s = self
                .structs
                .get(&key)
                .ok_or_else(|| TypeSourceError::NotFound(key.clone()))?;
            return self.build_struct(s);
        }
        if self.enums.keys().any(|k| name_matches(k, tail)) {
            return build_scalar(TypeDescriptor::primitive(TypeKind::Enumeration, "enum"));
        }
        if let Some((_, bits)) = self.bitmasks.iter().find(|(k, _)| name_matches(k, tail)) {
            return build_scalar(TypeDescriptor::primitive(uint_kind(*bits), "bitmask"));
        }
        if let Some((_, bits)) = self.bitsets.iter().find(|(k, _)| name_matches(k, tail)) {
            return build_scalar(TypeDescriptor::primitive(uint_kind(*bits), "bitset"));
        }
        if let Some((_, u)) = self.unions.iter().find(|(k, _)| name_matches(k, tail)) {
            return self.build_union(u);
        }
        Err(TypeSourceError::Unsupported(format!(
            "scoped member type {tail}"
        )))
    }

    /// Build a union [`DynamicType`] from its IDL definition: the discriminator
    /// (at its switch kind), then one member per case carrying its label set +
    /// `default` flag, with the branch type fully resolved (recursing into
    /// nested structs/unions). Mirrors the codec's union member walk.
    fn build_union(&self, u: &UnionDef) -> R<DynamicType> {
        let disc_kind = switch_kind(&u.switch_type);
        let mut ud = TypeDescriptor::union(
            u.name.text.clone(),
            TypeDescriptor::primitive(disc_kind, "disc"),
        );
        ud.extensibility_kind = ext_from_annotations(&u.annotations);
        let mut b = F::create_type(ud).map_err(|e| TypeSourceError::Build(e.to_string()))?;
        for (idx, case) in u.cases.iter().enumerate() {
            let id = idx as u32;
            let decl = &case.element.declarator;
            let member_name = decl.name().text.clone();
            let array_sizes = array_sizes_of(decl);
            let base = self.resolve_member_type(&case.element.type_spec, &member_name)?;
            let ty = if array_sizes.is_empty() {
                base
            } else {
                collection::array_of(base, array_sizes, &member_name)
            };
            let mut md = MemberDescriptor::new(&member_name, id, ty.descriptor().clone());
            md.index = id;
            for lbl in &case.labels {
                match lbl {
                    CaseLabel::Default => md.is_default_label = true,
                    CaseLabel::Value(e) => {
                        if let Some(v) = const_to_i64(e) {
                            md.label.push(v);
                        }
                    }
                }
            }
            b.add_member_resolved(md, ty)
                .map_err(|e| TypeSourceError::Build(e.to_string()))?;
        }
        b.build().map_err(|e| TypeSourceError::Build(e.to_string()))
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

// --- helpers ---

fn qualify(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}::{name}")
    }
}

fn struct_extensibility(s: &StructDef) -> ExtensibilityKind {
    ext_from_annotations(&s.annotations)
}

/// `@final`/`@appendable`/`@mutable` → [`ExtensibilityKind`] (XTypes 1.3 default
/// `@appendable`). Shared by struct + union lowering.
fn ext_from_annotations(anns: &[Annotation]) -> ExtensibilityKind {
    for a in anns {
        match a.name.parts.last().map(|i| i.text.as_str()) {
            Some("final") => return ExtensibilityKind::Final,
            Some("mutable") => return ExtensibilityKind::Mutable,
            Some("appendable") => return ExtensibilityKind::Appendable,
            _ => {}
        }
    }
    ExtensibilityKind::Appendable
}

/// The discriminator [`TypeKind`] of a union `switch(...)` type. A scoped switch
/// (an `enum`) maps to `Enumeration` (int32 wire form), matching the codec.
fn switch_kind(spec: &SwitchTypeSpec) -> TypeKind {
    match spec {
        SwitchTypeSpec::Boolean => TypeKind::Boolean,
        SwitchTypeSpec::Char => TypeKind::Char8,
        SwitchTypeSpec::Octet => TypeKind::Byte,
        SwitchTypeSpec::Scoped(_) => TypeKind::Enumeration,
        SwitchTypeSpec::Integer(i) => match i {
            IntegerType::Int8 => TypeKind::Int8,
            IntegerType::UInt8 => TypeKind::UInt8,
            IntegerType::Short | IntegerType::Int16 => TypeKind::Int16,
            IntegerType::UShort | IntegerType::UInt16 => TypeKind::UInt16,
            IntegerType::Long | IntegerType::Int32 => TypeKind::Int32,
            IntegerType::ULong | IntegerType::UInt32 => TypeKind::UInt32,
            IntegerType::LongLong | IntegerType::Int64 => TypeKind::Int64,
            IntegerType::ULongLong | IntegerType::UInt64 => TypeKind::UInt64,
        },
    }
}

/// Evaluate an integer case-label expression to `i64` (union labels may be
/// negative for signed discriminators).
fn const_to_i64(e: &ConstExpr) -> Option<i64> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => raw.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn array_sizes_of(decl: &zerodds_idl::ast::types::Declarator) -> Vec<u32> {
    match decl {
        zerodds_idl::ast::types::Declarator::Array(a) => {
            a.sizes.iter().filter_map(const_to_u32).collect()
        }
        zerodds_idl::ast::types::Declarator::Simple(_) => Vec::new(),
    }
}

fn const_to_u32(e: &ConstExpr) -> Option<u32> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => raw.trim().parse::<u32>().ok(),
        _ => None,
    }
}

/// Build a non-composite (primitive/enum/string/bitmask/bitset) [`DynamicType`]
/// from a leaf descriptor.
fn build_scalar(desc: TypeDescriptor) -> R<DynamicType> {
    F::create_type(desc)
        .and_then(|b| b.build())
        .map_err(|e| TypeSourceError::Build(e.to_string()))
}

/// `true` if qualified key `k` equals `tail` or ends with `::tail`.
fn name_matches(k: &str, tail: &str) -> bool {
    k == tail || k.ends_with(&format!("::{tail}"))
}

/// Unsigned wire-integer kind for a bit width (bitmask `@bit_bound` / bitset
/// total bits): ≤8 → u8, ≤16 → u16, ≤32 → u32, else u64.
const fn uint_kind(bits: u32) -> TypeKind {
    match bits {
        0..=8 => TypeKind::UInt8,
        9..=16 => TypeKind::UInt16,
        17..=32 => TypeKind::UInt32,
        _ => TypeKind::UInt64,
    }
}

/// `@bit_bound(N)` holder width for a bitmask (XTypes §7.3.1.2.1.1; default 32).
fn bit_bound_of(annotations: &[zerodds_idl::ast::types::Annotation]) -> u32 {
    use zerodds_idl::ast::types::AnnotationParams;
    annotations
        .iter()
        .find(|a| a.name.parts.last().map(|i| i.text.as_str()) == Some("bit_bound"))
        .and_then(|a| match &a.params {
            AnnotationParams::Single(e) => const_to_u32(e),
            _ => None,
        })
        .unwrap_or(32)
}

/// Total declared bits of a bitset (sum of bitfield widths) — drives the packed
/// wire-integer width.
fn bitset_total_bits(bs: &zerodds_idl::ast::types::BitsetDecl) -> u32 {
    bs.bitfields
        .iter()
        .filter_map(|f| const_to_u32(&f.spec.width))
        .sum::<u32>()
        .max(1)
}

const fn primitive_kind(p: PrimitiveType) -> TypeKind {
    match p {
        PrimitiveType::Boolean => TypeKind::Boolean,
        PrimitiveType::Octet => TypeKind::Byte,
        PrimitiveType::Char => TypeKind::Char8,
        PrimitiveType::WideChar => TypeKind::Char16,
        PrimitiveType::Floating(FloatingType::Float) => TypeKind::Float32,
        PrimitiveType::Floating(FloatingType::Double) => TypeKind::Float64,
        PrimitiveType::Floating(FloatingType::LongDouble) => TypeKind::Float128,
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => TypeKind::Int16,
            IntegerType::Long | IntegerType::Int32 => TypeKind::Int32,
            IntegerType::LongLong | IntegerType::Int64 => TypeKind::Int64,
            IntegerType::UShort | IntegerType::UInt16 => TypeKind::UInt16,
            IntegerType::ULong | IntegerType::UInt32 => TypeKind::UInt32,
            IntegerType::ULongLong | IntegerType::UInt64 => TypeKind::UInt64,
            IntegerType::Int8 => TypeKind::Int8,
            IntegerType::UInt8 => TypeKind::UInt8,
        },
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_types::dynamic::codec::{decode_dynamic, encode_dynamic};
    use zerodds_types::dynamic::data::DynamicData;
    use zerodds_types::dynamic::type_::DynamicType;
    use zerodds_types::dynamic::{DynamicDataFactory, DynamicValue};

    const IDL: &str = r#"
        module cuas {
            @final struct Inner { long a; };
            @final struct Track {
                unsigned long id;
                string name;
                Inner inner;
                sequence<long> xs;
            };
        };
    "#;

    #[test]
    fn resolves_qualified_and_simple_names() {
        let book = TypeBook::from_idl(IDL).expect("parse");
        assert_eq!(book.resolve("cuas::Track").unwrap().name(), "Track");
        assert_eq!(book.resolve("Track").unwrap().name(), "Track");
        assert!(matches!(
            book.resolve("Nope"),
            Err(TypeSourceError::NotFound(_))
        ));
    }

    /// Full IDL with composite-element collections, a fixed struct array, a
    /// wstring, a bitmask and a bitset → resolve + codec byte-exact round-trip.
    /// A struct carrying a `@mutable union` member (the type-following demo
    /// shape) lowers end-to-end and round-trips through the codec — proving the
    /// IDL→DynamicType union path feeds the reflective PL_CDR2 union codec.
    #[test]
    fn struct_with_mutable_union_member_roundtrips() {
        const SRC: &str = r#"
            module dyn {
                @mutable union Cmd switch(long) {
                    case 1: long    speed;
                    case 2: double  angle;
                    default: octet  flag;
                };
                @appendable struct Track {
                    @key long  id;
                    string     name;
                    Cmd        command;
                };
            };
        "#;
        let book = TypeBook::from_idl(SRC).expect("parse");
        let track = book.resolve("dyn::Track").expect("resolve Track");
        // member 2 is the union; switch(long) → int32 discriminator.
        let cmd_ty = track.member_by_index(2).unwrap().dynamic_type().clone();
        assert_eq!(cmd_ty.kind(), TypeKind::Union);
        assert_eq!(
            cmd_ty.descriptor().extensibility_kind,
            ExtensibilityKind::Mutable
        );

        // Round-trip the union through BOTH a primitive branch (case 1: speed)
        // and the default branch, byte-exact, in XCDR1 + XCDR2.
        let run = |make_cmd: &dyn Fn() -> DynamicData, check: &dyn Fn(&DynamicData)| {
            let mut d = DynamicDataFactory::create_data(&track).unwrap();
            d.set_int32_value(0, 42).unwrap();
            d.set_string_value(1, "t1").unwrap();
            d.set_complex_value(2, make_cmd()).unwrap();
            for xcdr2 in [true, false] {
                let bytes = encode_dynamic(&d, xcdr2, false).expect("encode");
                let back = decode_dynamic(&track, &bytes, xcdr2, false).expect("decode");
                assert_eq!(back.get_int32_value(0).unwrap(), 42);
                assert_eq!(back.get_string_value(1).unwrap(), "t1");
                check(back.get_complex_value(2).unwrap());
                assert_eq!(encode_dynamic(&back, xcdr2, false).unwrap(), bytes);
            }
        };
        let cmd_ty2 = cmd_ty.clone();
        run(
            &|| {
                let mut c = DynamicDataFactory::create_data(&cmd_ty2).unwrap();
                c.set_float64_value(1, 1.5).unwrap(); // case 2: angle
                c
            },
            &|c| assert_eq!(c.get_float64_value(1).unwrap(), 1.5),
        );
        run(
            &|| {
                let mut c = DynamicDataFactory::create_data(&cmd_ty).unwrap();
                c.set_byte_value(2, 0xAB).unwrap(); // default: flag (octet)
                c
            },
            &|c| assert_eq!(c.get_byte_value(2).unwrap(), 0xAB),
        );
    }

    #[test]
    fn composite_collections_and_bitfields_roundtrip() {
        const SRC: &str = r#"
            module m {
                @bit_bound(8) bitmask Flags { F0, F1, F2 };
                bitset Bits { bitfield<3> a; bitfield<5> b; };
                @final struct Inner { long a; string s; };
                @final struct Outer {
                    sequence<Inner> items;
                    Inner pair[2];
                    wstring w;
                    Flags flags;
                    Bits bits;
                };
            };
        "#;
        let book = TypeBook::from_idl(SRC).expect("parse");
        let outer = book.resolve("m::Outer").expect("resolve Outer");
        let inner = book.resolve("m::Inner").expect("resolve Inner");

        // flags/bits lowered to their packed unsigned wire integer (u8 here).
        assert_eq!(
            outer.member_by_index(3).unwrap().dynamic_type().kind(),
            TypeKind::UInt8
        );
        assert_eq!(
            outer.member_by_index(4).unwrap().dynamic_type().kind(),
            TypeKind::UInt8
        );

        let mk_inner = |a: i32, s: &str| {
            let mut e = DynamicDataFactory::create_data(&inner).unwrap();
            e.set_int32_value(0, a).unwrap();
            e.set_string_value(1, s).unwrap();
            e
        };
        let mut d = DynamicDataFactory::create_data(&outer).unwrap();
        d.set_sequence_value(
            0,
            vec![mk_inner(1, "x"), mk_inner(2, "y"), mk_inner(3, "z")],
        )
        .unwrap();
        d.set_sequence_value(1, vec![mk_inner(10, "p"), mk_inner(20, "q")])
            .unwrap();
        d.set_value_raw(2, DynamicValue::WString("wíde".encode_utf16().collect()));
        d.set_value_raw(3, DynamicValue::UInt8(0b101));
        d.set_value_raw(4, DynamicValue::UInt8(0xAB));

        for xcdr2 in [true, false] {
            let bytes = encode_dynamic(&d, xcdr2, false).expect("encode");
            let back = decode_dynamic(&outer, &bytes, xcdr2, false).expect("decode");
            let DynamicValue::Sequence(items) = back.get_value(0).unwrap() else {
                panic!("items")
            };
            assert_eq!(items.len(), 3);
            assert_eq!(items[2].get_int32_value(0).unwrap(), 3);
            assert_eq!(items[2].get_string_value(1).unwrap(), "z");
            let DynamicValue::Sequence(pair) = back.get_value(1).unwrap() else {
                panic!("pair")
            };
            assert_eq!(pair.len(), 2);
            assert_eq!(pair[1].get_int32_value(0).unwrap(), 20);
            let DynamicValue::WString(w) = back.get_value(2).unwrap() else {
                panic!("w")
            };
            assert_eq!(String::from_utf16_lossy(w), "wíde");
            assert_eq!(back.get_uint8_value(3).unwrap(), 0b101);
            assert_eq!(back.get_uint8_value(4).unwrap(), 0xAB);
            assert_eq!(
                encode_dynamic(&back, xcdr2, false).unwrap(),
                bytes,
                "byte-exact xcdr2={xcdr2}"
            );
        }
    }

    #[test]
    fn idl_type_roundtrips_through_the_codec() {
        let ty = dynamic_type_from_idl(IDL, "cuas::Track").expect("resolve");
        // Members: 0 id(u32), 1 name(string), 2 inner(Inner), 3 xs(seq<long>)
        let mut d = DynamicDataFactory::create_data(&ty).unwrap();
        d.set_uint32_value(0, 7).unwrap();
        d.set_string_value(1, "alpha").unwrap();

        let inner_ty = ty.member_by_index(2).unwrap().dynamic_type().clone();
        let mut inner = DynamicDataFactory::create_data(&inner_ty).unwrap();
        inner.set_int32_value(0, 42).unwrap();
        d.set_complex_value(2, inner).unwrap();

        let mut xs = Vec::new();
        for v in [10_i32, 20, 30] {
            let mut e = DynamicDataFactory::create_data(&DynamicType::new_primitive(
                zerodds_types::dynamic::descriptor::TypeKind::Int32,
            ))
            .unwrap();
            e.set_value_raw(0, DynamicValue::Int32(v));
            xs.push(e);
        }
        d.set_sequence_value(3, xs).unwrap();

        let bytes = encode_dynamic(&d, true, false).expect("encode");
        let back = decode_dynamic(&ty, &bytes, true, false).expect("decode");
        assert_eq!(back.get_uint32_value(0).unwrap(), 7);
        assert_eq!(back.get_string_value(1).unwrap(), "alpha");
        assert_eq!(
            back.get_complex_value(2)
                .unwrap()
                .get_int32_value(0)
                .unwrap(),
            42
        );
        let DynamicValue::Sequence(v) = back.get_value(3).unwrap() else {
            panic!("seq")
        };
        let got: Vec<i32> = v.iter().map(|e| e.get_int32_value(0).unwrap()).collect();
        assert_eq!(got, vec![10, 20, 30]);
        // byte-exact replay path
        assert_eq!(encode_dynamic(&back, true, false).unwrap(), bytes);
    }
}
