// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST → TypeObject Mapper (WP 1.5 T-IDL2).
//!
//! Converts IDL `StructDef` → XTypes `TypeObject`. Primary focus
//! phase 1: struct with primitives, strings, bounded sequences.
//! Nested composite types (struct-in-struct, unions, enums as members)
//! are returned as `TypeIdentifier::EquivalenceHashMinimal(ZERO)` placeholders
//! — the caller must resolve them in the registry.

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::type_object::minimal::MinimalStructType;
use zerodds_types::{PrimitiveKind, TypeIdentifier};

use crate::ast::{
    FloatingType, IntegerType, MapType, PrimitiveType, SequenceType, StringType, StructDef,
    TypeSpec,
};

use super::annotations::{ExtensibilityKind, lower_annotations};

/// Error during mapping.
#[derive(Debug, Clone, PartialEq)]
pub enum MapError {
    /// Non-primitive types are currently mapped as an opaque hash.
    /// If the caller does not allow that, this variant returns a
    /// hint that a real type mapper for composite types is needed.
    UnsupportedTypeSpec(&'static str),
    /// Annotation-lowering error.
    Annotation(String),
    /// Scoped-name reference to another named type (e.g.
    /// `struct S { Foo a; };`). The isolated single-type mapper cannot
    /// resolve forward refs. [`build_type_registry`] resolves them
    /// via a two-stage, topologically sorted pass.
    UnresolvedScoped(alloc::string::String),
    /// Recursive type definition (cycle in the dependency graph). The
    /// named types form a strongly-connected component that
    /// needs XTypes 1.3 §7.3.4.9.2 SCC identifiers.
    RecursiveType(alloc::string::String),
}

/// Maps an IDL `TypeSpec` to a `TypeIdentifier`.
///
/// Primitives + strings + sequences-of-primitives map directly. Scoped
/// refs + other composite types are mapped as null-hash placeholders.
///
/// zerodds-lint: recursion-depth 8
///
/// Recursion on `Sequence<...>` with a nested element TypeSpec. Realistic
/// IDL constructs nest at most 2-3 levels. The cap is not explicitly
/// enforced — the AST comes from the IDL parser (WP 0.3), which
/// itself has grammar-based limits (LR(1) stack).
///
/// # Errors
/// `UnsupportedTypeSpec` for currently non-mappable constructions.
pub fn map_type_spec(spec: &TypeSpec) -> Result<TypeIdentifier, MapError> {
    Ok(match spec {
        TypeSpec::Primitive(p) => TypeIdentifier::Primitive(map_primitive(*p)),
        TypeSpec::String(StringType { wide, bound, .. }) => {
            let bound_u32 = bound
                .as_ref()
                .and_then(|e| {
                    if let crate::ast::ConstExpr::Literal(l) = e {
                        l.raw.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if *wide {
                if bound_u32 <= 255 {
                    TypeIdentifier::String16Small {
                        bound: bound_u32 as u8,
                    }
                } else {
                    TypeIdentifier::String16Large { bound: bound_u32 }
                }
            } else if bound_u32 <= 255 {
                TypeIdentifier::String8Small {
                    bound: bound_u32 as u8,
                }
            } else {
                TypeIdentifier::String8Large { bound: bound_u32 }
            }
        }
        TypeSpec::Sequence(SequenceType { elem, bound, .. }) => {
            let element = alloc::boxed::Box::new(map_type_spec(elem)?);
            let bound_u32 = bound
                .as_ref()
                .and_then(|e| {
                    if let crate::ast::ConstExpr::Literal(l) = e {
                        l.raw.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if bound_u32 <= 255 {
                TypeIdentifier::PlainSequenceSmall {
                    header: zerodds_types::PlainCollectionHeader::for_element(
                        collection_equiv_kind(&element),
                    ),
                    bound: bound_u32 as u8,
                    element,
                }
            } else {
                TypeIdentifier::PlainSequenceLarge {
                    header: zerodds_types::PlainCollectionHeader::for_element(
                        collection_equiv_kind(&element),
                    ),
                    bound: bound_u32,
                    element,
                }
            }
        }
        TypeSpec::Scoped(s) => {
            let path = s
                .parts
                .iter()
                .map(|p| p.text.clone())
                .collect::<alloc::vec::Vec<_>>()
                .join("::");
            return Err(MapError::UnresolvedScoped(path));
        }
        TypeSpec::Map(MapType {
            key, value, bound, ..
        }) => {
            let key_ti = map_type_spec(key)?;
            let value_ti = map_type_spec(value)?;
            make_map_ti(key_ti, value_ti, literal_bound(bound))
        }
        TypeSpec::Fixed(_) => return Err(MapError::UnsupportedTypeSpec("fixed")),
        TypeSpec::Any => return Err(MapError::UnsupportedTypeSpec("any")),
    })
}

/// Builds a `PlainMap` `TypeIdentifier` (XTypes 1.3 §7.3.4.6, TK_MAP) from
/// the resolved key + value `TypeIdentifier`s and the declared bound
/// (`0` = unbounded). The map's `PlainCollectionHeader.equiv_kind` follows
/// the value element (`EK_MINIMAL` when the value is a named-type minimal
/// hash, else plain), mirroring the sequence/array convention.
fn make_map_ti(key: TypeIdentifier, value: TypeIdentifier, bound_u32: u32) -> TypeIdentifier {
    let header = zerodds_types::PlainCollectionHeader::for_element(collection_equiv_kind(&value));
    // `key_flags` is `type_identifier::CollectionElementFlag` (distinct from
    // the `type_object::flags` flag type and not re-exported at the crate
    // root); let the struct-literal infer it via `Default`.
    if bound_u32 <= u32::from(u8::MAX) {
        TypeIdentifier::PlainMapSmall {
            header,
            bound: bound_u32 as u8,
            element: alloc::boxed::Box::new(value),
            key_flags: Default::default(),
            key: alloc::boxed::Box::new(key),
        }
    } else {
        TypeIdentifier::PlainMapLarge {
            header,
            bound: bound_u32,
            element: alloc::boxed::Box::new(value),
            key_flags: Default::default(),
            key: alloc::boxed::Box::new(key),
        }
    }
}

fn map_primitive(p: PrimitiveType) -> PrimitiveKind {
    use FloatingType::*;
    use IntegerType::*;
    match p {
        PrimitiveType::Boolean => PrimitiveKind::Boolean,
        PrimitiveType::Octet => PrimitiveKind::Byte,
        PrimitiveType::Char => PrimitiveKind::Char8,
        PrimitiveType::WideChar => PrimitiveKind::Char16,
        PrimitiveType::Integer(i) => match i {
            Short | Int16 => PrimitiveKind::Int16,
            Long | Int32 => PrimitiveKind::Int32,
            LongLong | Int64 => PrimitiveKind::Int64,
            UShort | UInt16 => PrimitiveKind::UInt16,
            ULong | UInt32 => PrimitiveKind::UInt32,
            ULongLong | UInt64 => PrimitiveKind::UInt64,
            Int8 => PrimitiveKind::Int8,
            UInt8 => PrimitiveKind::UInt8,
        },
        PrimitiveType::Floating(f) => match f {
            Float => PrimitiveKind::Float32,
            Double => PrimitiveKind::Float64,
            LongDouble => PrimitiveKind::Float128,
        },
    }
}

/// Maps an IDL `StructDef` → XTypes `MinimalStructType`.
///
/// Recognizes `@key`, `@id(n)`, `@optional`, `@must_understand`, `@external`
/// on members and `@final`/`@appendable`/`@mutable`/`@nested`/
/// `@extensibility(...)` on the struct.
///
/// # Errors
/// `MapError` for non-mappable constructions.
pub fn lower_struct_to_minimal(s: &StructDef) -> Result<MinimalStructType, MapError> {
    let type_annotations = lower_annotations(&s.annotations)
        .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
    let extensibility = match type_annotations.extensibility() {
        Some(ExtensibilityKind::Final) => Extensibility::Final,
        Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
        _ => Extensibility::Appendable,
    };
    let nested = type_annotations
        .builtins
        .iter()
        .any(|a| matches!(a, super::annotations::BuiltinAnnotation::Nested));
    let autoid_hash = type_annotations.builtins.iter().any(|a| {
        matches!(
            a,
            super::annotations::BuiltinAnnotation::Autoid(super::annotations::AutoidKind::Hash)
        )
    });

    let mut builder =
        TypeObjectBuilder::struct_type(s.name.text.clone()).extensibility(extensibility);
    if nested {
        builder = builder.nested();
    }
    if autoid_hash {
        builder = builder.autoid_hash();
    }

    for m in &s.members {
        // An IDL member can have multiple declarators (e.g. `long a, b;`).
        // We treat each declarator as a separate TypeObject member.
        for decl in &m.declarators {
            let ti = map_type_spec(&m.type_spec)?;
            let member_anns = lower_annotations(&m.annotations)
                .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
            // §7.2.4.4.2 — @non_serialized members are not included in the
            // TypeObject (ABI-only, no wire form, hence also
            // not in the assignability comparison).
            let is_non_serialized = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::NonSerialized));
            if is_non_serialized {
                continue;
            }
            let explicit_id = member_anns.explicit_id();
            let is_key = member_anns.has_key();
            let is_optional = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::Optional));
            let is_must_understand = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::MustUnderstand));
            let is_external = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, super::annotations::BuiltinAnnotation::External));
            let member_name = decl.name().text.clone();
            builder = builder.member(member_name, ti, |mut mb| {
                if is_key {
                    mb = mb.key();
                }
                if is_optional {
                    mb = mb.optional();
                }
                if is_must_understand {
                    mb = mb.must_understand();
                }
                if is_external {
                    mb = mb.external();
                }
                if let Some(id) = explicit_id {
                    mb = mb.id(id);
                }
                mb
            });
        }
    }

    Ok(builder.build_minimal())
}

// ============================================================================
// Complete spec → TypeRegistry mapper (two-stage, topological).
// ============================================================================
//
// `map_type_spec` above fails on any scoped reference to a
// named type. The following block resolves all named types of a
// `Specification` together: first all types are collected along with their
// module scope and topologically sorted by dependency,
// then lowered in order — when a type's turn comes,
// the minimal hashes of all its dependencies are already in the
// `NameMap`, so that `map_type_spec_resolved` can insert them as
// `EquivalenceHashMinimal`.

use alloc::collections::{BTreeMap, BTreeSet};

use zerodds_types::MinimalTypeObject;
use zerodds_types::compute_minimal_hash;
use zerodds_types::resolve::TypeRegistry;

use crate::ast::{
    ConstExpr, ConstrTypeDecl, Declarator, Definition, EnumDef, ScopedName, Specification,
    StructDcl, SwitchTypeSpec, TypeDecl, UnionDcl, UnionDef,
};

use super::annotations::BuiltinAnnotation;
use super::const_eval::{SymbolTable, evaluate};

/// Enum FQN → (literal name → ordinal value). Precomputed, so that
/// union case labels (`case Color::RED:`) can be resolved.
type EnumValues = BTreeMap<alloc::string::String, BTreeMap<alloc::string::String, i32>>;

/// EK_MINIMAL — equivalence-kind discriminator for minimal hashes
/// (XTypes 1.3 §7.3.4.5). Identical to `zerodds_types` `kinds::EK_MINIMAL`.
const EK_MINIMAL: u8 = 0xF1;
/// EK_COMPLETE — equivalence-kind discriminator for complete hashes.
const EK_COMPLETE: u8 = 0xF2;
/// EK_BOTH — the element's minimal and complete TypeIdentifiers are
/// identical (true for primitives / strings / nested plain collections).
const EK_BOTH: u8 = 0xF3;

/// Fully-qualified name → `EquivalenceHashMinimal` identifier of a
/// lowered named type.
pub type NameMap = BTreeMap<alloc::string::String, TypeIdentifier>;

/// Result of [`build_type_registry`]: all named types of a
/// `Specification` as minimal `TypeObject`s plus a resolution index.
#[derive(Debug)]
pub struct LoweredSpec {
    /// All lowered minimal `TypeObject`s, addressable by hash.
    pub registry: TypeRegistry,
    /// Fully-qualified type name → minimal-hash identifier.
    pub names: NameMap,
    /// Topological emission order (dependencies first).
    pub order: alloc::vec::Vec<alloc::string::String>,
}

/// A named type along with its module scope: `struct`, `enum`, `union` or
/// a single `typedef` declarator (alias).
enum NamedDef<'a> {
    Struct(&'a StructDef),
    Enum(&'a EnumDef),
    Union(&'a UnionDef),
    /// A `typedef` declarator. `array_sizes` empty = scalar alias,
    /// otherwise array alias with the dimensions.
    Alias {
        underlying: &'a crate::ast::TypeSpec,
        array_sizes: &'a [ConstExpr],
    },
}

struct NamedItem<'a> {
    /// Fully-qualified name (`Outer::Inner::Pose`).
    fqn: alloc::string::String,
    /// Module scope in which the type lives (for relative ref resolution).
    scope: alloc::vec::Vec<alloc::string::String>,
    def: NamedDef<'a>,
}

/// Lowers all named types of a `Specification` into a
/// `TypeRegistry`. Scoped member references are resolved via a
/// topologically sorted two-pass.
///
/// # Errors
/// `RecursiveType` on cyclic definitions; `UnresolvedScoped` /
/// `UnsupportedTypeSpec` for (still) non-mappable constructs.
pub fn build_type_registry(spec: &Specification) -> Result<LoweredSpec, MapError> {
    let mut items: alloc::vec::Vec<NamedItem<'_>> = alloc::vec::Vec::new();
    collect_named(&spec.definitions, &mut alloc::vec::Vec::new(), &mut items);

    let all_fqns: BTreeSet<alloc::string::String> = items.iter().map(|it| it.fqn.clone()).collect();
    let deps: alloc::vec::Vec<alloc::vec::Vec<alloc::string::String>> = items
        .iter()
        .map(|it| dependencies_of(it, &all_fqns))
        .collect();
    let order = topo_sort(&items, &deps)?;

    // Enum literal values upfront — for union case-label resolution.
    let mut enum_values: EnumValues = EnumValues::new();
    for item in &items {
        if let NamedDef::Enum(e) = &item.def {
            let mut vals = BTreeMap::new();
            for (name, value, _) in enum_literal_values(e)? {
                vals.insert(name, value);
            }
            enum_values.insert(item.fqn.clone(), vals);
        }
    }

    let by_fqn: BTreeMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, it)| (it.fqn.as_str(), i))
        .collect();
    let mut registry = TypeRegistry::new();
    let mut names = NameMap::new();
    for fqn in &order {
        let item = &items[by_fqn[fqn.as_str()]];
        let mto = lower_named(item, &names, &enum_values)?;
        let hash = compute_minimal_hash(&mto)
            .map_err(|e| MapError::Annotation(alloc::format!("hash failed: {e:?}")))?;
        registry.insert_minimal(hash, mto);
        names.insert(fqn.clone(), TypeIdentifier::EquivalenceHashMinimal(hash));
    }
    Ok(LoweredSpec {
        registry,
        names,
        order,
    })
}

impl LoweredSpec {
    /// Iterates the lowered types in topological order
    /// (dependencies first) as `(fqn, &MinimalTypeObject)`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &MinimalTypeObject)> {
        self.order.iter().filter_map(move |fqn| {
            let TypeIdentifier::EquivalenceHashMinimal(hash) = self.names.get(fqn)? else {
                return None;
            };
            self.registry
                .get_minimal(hash)
                .map(|obj| (fqn.as_str(), obj))
        })
    }
}

/// Recursive module walk, collects struct/enum along with the scope.
///
/// zerodds-lint: recursion-depth 64 (parser/AST walk; bounded by IDL nesting)
fn collect_named<'a>(
    defs: &'a [Definition],
    scope: &mut alloc::vec::Vec<alloc::string::String>,
    out: &mut alloc::vec::Vec<NamedItem<'a>>,
) {
    for def in defs {
        match def {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                collect_named(&m.definitions, scope, out);
                scope.pop();
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                out.push(NamedItem {
                    fqn: join_fqn(scope, &s.name.text),
                    scope: scope.clone(),
                    def: NamedDef::Struct(s),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                out.push(NamedItem {
                    fqn: join_fqn(scope, &e.name.text),
                    scope: scope.clone(),
                    def: NamedDef::Enum(e),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                out.push(NamedItem {
                    fqn: join_fqn(scope, &u.name.text),
                    scope: scope.clone(),
                    def: NamedDef::Union(u),
                });
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                // A typedef has 1..n declarators — each is its
                // own named alias.
                for decl in &td.declarators {
                    let (name, sizes): (&str, &[ConstExpr]) = match decl {
                        Declarator::Simple(id) => (&id.text, &[]),
                        Declarator::Array(ad) => (&ad.name.text, &ad.sizes),
                    };
                    out.push(NamedItem {
                        fqn: join_fqn(scope, name),
                        scope: scope.clone(),
                        def: NamedDef::Alias {
                            underlying: &td.type_spec,
                            array_sizes: sizes,
                        },
                    });
                }
            }
            _ => {}
        }
    }
}

/// `["Outer","Inner"] + "Pose"` → `"Outer::Inner::Pose"`.
fn join_fqn(scope: &[alloc::string::String], name: &str) -> alloc::string::String {
    if scope.is_empty() {
        name.into()
    } else {
        alloc::format!("{}::{name}", scope.join("::"))
    }
}

/// FQNs of all named types that `item` directly depends on.
fn dependencies_of(
    item: &NamedItem<'_>,
    all: &BTreeSet<alloc::string::String>,
) -> alloc::vec::Vec<alloc::string::String> {
    let mut refs: alloc::vec::Vec<&ScopedName> = alloc::vec::Vec::new();
    match &item.def {
        NamedDef::Struct(s) => {
            for m in &s.members {
                collect_scoped_refs(&m.type_spec, &mut refs);
            }
        }
        NamedDef::Union(u) => {
            if let SwitchTypeSpec::Scoped(sn) = &u.switch_type {
                refs.push(sn);
            }
            for case in &u.cases {
                collect_scoped_refs(&case.element.type_spec, &mut refs);
            }
        }
        NamedDef::Alias { underlying, .. } => {
            collect_scoped_refs(underlying, &mut refs);
        }
        NamedDef::Enum(_) => {}
    }
    let mut deps = alloc::vec::Vec::new();
    for sn in refs {
        if let Some(fqn) = resolve_fqn(sn, &item.scope, all) {
            if !deps.contains(&fqn) {
                deps.push(fqn);
            }
        }
    }
    deps
}

/// Collects all scoped references in a `TypeSpec` (recursively through
/// sequences + maps).
///
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_scoped_refs<'a>(ts: &'a TypeSpec, out: &mut alloc::vec::Vec<&'a ScopedName>) {
    match ts {
        TypeSpec::Scoped(sn) => out.push(sn),
        TypeSpec::Sequence(s) => collect_scoped_refs(&s.elem, out),
        TypeSpec::Map(m) => {
            collect_scoped_refs(&m.key, out);
            collect_scoped_refs(&m.value, out);
        }
        _ => {}
    }
}

/// String-based FQN resolution: innermost scope first (IDL name-
/// lookup rule). `known` checks whether a candidate is a known name.
fn resolve_str_fqn(
    target: &str,
    absolute: bool,
    scope: &[alloc::string::String],
    known: impl Fn(&str) -> bool,
) -> Option<alloc::string::String> {
    if absolute {
        return known(target).then(|| target.to_string());
    }
    for k in (0..=scope.len()).rev() {
        let candidate = if k == 0 {
            target.to_string()
        } else {
            alloc::format!("{}::{target}", scope[..k].join("::"))
        };
        if known(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Resolves a scoped reference against the set of known FQNs.
fn resolve_fqn(
    sn: &ScopedName,
    scope: &[alloc::string::String],
    all: &BTreeSet<alloc::string::String>,
) -> Option<alloc::string::String> {
    let target = sn
        .parts
        .iter()
        .map(|p| p.text.as_str())
        .collect::<alloc::vec::Vec<_>>()
        .join("::");
    resolve_str_fqn(&target, sn.absolute, scope, |c| all.contains(c))
}

/// Resolves a scoped reference against the `NameMap` — returns the
/// minimal-hash identifier of the target type.
fn resolve_scoped(
    sn: &ScopedName,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Option<TypeIdentifier> {
    let target = sn
        .parts
        .iter()
        .map(|p| p.text.as_str())
        .collect::<alloc::vec::Vec<_>>()
        .join("::");
    if sn.absolute {
        return names.get(&target).cloned();
    }
    for k in (0..=scope.len()).rev() {
        let candidate = if k == 0 {
            target.clone()
        } else {
            alloc::format!("{}::{target}", scope[..k].join("::"))
        };
        if let Some(ti) = names.get(&candidate) {
            return Some(ti.clone());
        }
    }
    None
}

/// Kahn topo-sort. `Err(RecursiveType)` on a cycle.
fn topo_sort(
    items: &[NamedItem<'_>],
    deps: &[alloc::vec::Vec<alloc::string::String>],
) -> Result<alloc::vec::Vec<alloc::string::String>, MapError> {
    let idx: BTreeMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, it)| (it.fqn.as_str(), i))
        .collect();
    let n = items.len();
    let mut indegree = alloc::vec![0usize; n];
    let mut dependents: alloc::vec::Vec<alloc::vec::Vec<usize>> =
        alloc::vec![alloc::vec::Vec::new(); n];
    for (i, ds) in deps.iter().enumerate() {
        for d in ds {
            if let Some(&di) = idx.get(d.as_str()) {
                dependents[di].push(i);
                indegree[i] += 1;
            }
        }
    }
    let mut queue: alloc::vec::Vec<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut order = alloc::vec::Vec::with_capacity(n);
    let mut head = 0;
    while head < queue.len() {
        let cur = queue[head];
        head += 1;
        order.push(items[cur].fqn.clone());
        for &dep in &dependents[cur] {
            indegree[dep] -= 1;
            if indegree[dep] == 0 {
                queue.push(dep);
            }
        }
    }
    if order.len() == n {
        Ok(order)
    } else {
        let cyclic: alloc::vec::Vec<&str> = (0..n)
            .filter(|&i| indegree[i] > 0)
            .map(|i| items[i].fqn.as_str())
            .collect();
        Err(MapError::RecursiveType(cyclic.join(", ")))
    }
}

/// Lowers a collected type into its minimal `TypeObject`.
fn lower_named(
    item: &NamedItem<'_>,
    names: &NameMap,
    enum_values: &EnumValues,
) -> Result<MinimalTypeObject, MapError> {
    match &item.def {
        NamedDef::Struct(s) => Ok(MinimalTypeObject::Struct(lower_struct_minimal_resolved(
            s,
            &item.scope,
            names,
        )?)),
        NamedDef::Enum(e) => Ok(MinimalTypeObject::Enumerated(lower_enum_minimal(e)?)),
        NamedDef::Union(u) => Ok(MinimalTypeObject::Union(lower_union_minimal(
            u,
            &item.scope,
            names,
            enum_values,
        )?)),
        NamedDef::Alias {
            underlying,
            array_sizes,
        } => {
            let simple = item.fqn.rsplit("::").next().unwrap_or(item.fqn.as_str());
            Ok(MinimalTypeObject::Alias(lower_alias_minimal(
                simple,
                underlying,
                array_sizes,
                &item.scope,
                names,
            )?))
        }
    }
}

/// `map_type_spec` with resolution of scoped references via the `NameMap`.
///
/// zerodds-lint: recursion-depth 64 (parser/AST walk; bounded by IDL nesting)
fn map_type_spec_resolved(
    ts: &TypeSpec,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<TypeIdentifier, MapError> {
    match ts {
        TypeSpec::Scoped(sn) => resolve_scoped(sn, scope, names).ok_or_else(|| {
            MapError::UnresolvedScoped(
                sn.parts
                    .iter()
                    .map(|p| p.text.clone())
                    .collect::<alloc::vec::Vec<_>>()
                    .join("::"),
            )
        }),
        TypeSpec::Sequence(SequenceType { elem, bound, .. }) => {
            let element = map_type_spec_resolved(elem, scope, names)?;
            let bound_u32 = literal_bound(bound);
            let header =
                zerodds_types::PlainCollectionHeader::for_element(collection_equiv_kind(&element));
            Ok(if bound_u32 <= u32::from(u8::MAX) {
                TypeIdentifier::PlainSequenceSmall {
                    header,
                    bound: bound_u32 as u8,
                    element: alloc::boxed::Box::new(element),
                }
            } else {
                TypeIdentifier::PlainSequenceLarge {
                    header,
                    bound: bound_u32,
                    element: alloc::boxed::Box::new(element),
                }
            })
        }
        TypeSpec::Map(MapType {
            key, value, bound, ..
        }) => {
            // §7.3.4.6 TK_MAP — resolve key + value against the NameMap so a
            // map<K,V> with named-type key/value emits a proper map
            // TypeIdentifier instead of skipping the whole struct's TypeObject.
            let key_ti = map_type_spec_resolved(key, scope, names)?;
            let value_ti = map_type_spec_resolved(value, scope, names)?;
            Ok(make_map_ti(key_ti, value_ti, literal_bound(bound)))
        }
        // Primitives + strings + Fixed/Any: identical to the isolated mapper
        // (no resolution needed, or unsupported).
        other => map_type_spec(other),
    }
}

/// `bound` `ConstExpr` as `u32` (0 = unbounded), integer literals only.
fn literal_bound(bound: &Option<crate::ast::ConstExpr>) -> u32 {
    bound
        .as_ref()
        .and_then(|e| {
            if let crate::ast::ConstExpr::Literal(l) = e {
                l.raw.parse::<u32>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

/// Equiv kind for a plain-collection header — `EK_MINIMAL` if
/// the element is a minimal hash, otherwise 0 (primitive/plain).
fn collection_equiv_kind(element: &TypeIdentifier) -> u8 {
    match element {
        TypeIdentifier::EquivalenceHashMinimal(_) => EK_MINIMAL,
        TypeIdentifier::EquivalenceHashComplete(_) => EK_COMPLETE,
        // Fully-descriptive elements (primitives, strings, nested plain
        // collections) are identical in the minimal and complete graphs →
        // EK_BOTH (0xF3). Byte-verified against Cyclone + FastDDS.
        _ => EK_BOTH,
    }
}

/// Like [`lower_struct_to_minimal`], but resolves scoped member types
/// via the `NameMap`.
fn lower_struct_minimal_resolved(
    s: &StructDef,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<MinimalStructType, MapError> {
    let type_annotations = lower_annotations(&s.annotations)
        .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
    let extensibility = match type_annotations.extensibility() {
        Some(ExtensibilityKind::Final) => Extensibility::Final,
        Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
        _ => Extensibility::Appendable,
    };
    let nested = type_annotations
        .builtins
        .iter()
        .any(|a| matches!(a, BuiltinAnnotation::Nested));
    let autoid_hash = type_annotations.builtins.iter().any(|a| {
        matches!(
            a,
            BuiltinAnnotation::Autoid(super::annotations::AutoidKind::Hash)
        )
    });

    let mut builder =
        TypeObjectBuilder::struct_type(s.name.text.clone()).extensibility(extensibility);
    if nested {
        builder = builder.nested();
    }
    if autoid_hash {
        builder = builder.autoid_hash();
    }

    for m in &s.members {
        for decl in &m.declarators {
            let ti = map_type_spec_resolved(&m.type_spec, scope, names)?;
            let member_anns = lower_annotations(&m.annotations)
                .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
            if member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::NonSerialized))
            {
                continue;
            }
            let explicit_id = member_anns.explicit_id();
            let is_key = member_anns.has_key();
            let is_optional = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::Optional));
            let is_must_understand = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::MustUnderstand));
            let is_external = member_anns
                .builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::External));
            let member_name = decl.name().text.clone();
            builder = builder.member(member_name, ti, |mut mb| {
                if is_key {
                    mb = mb.key();
                }
                if is_optional {
                    mb = mb.optional();
                }
                if is_must_understand {
                    mb = mb.must_understand();
                }
                if is_external {
                    mb = mb.external();
                }
                if let Some(id) = explicit_id {
                    mb = mb.id(id);
                }
                mb
            });
        }
    }
    Ok(builder.build_minimal())
}

/// Lowers an IDL `enum` into a minimal `MinimalEnumeratedType`.
/// Ordinal values sequentially from 0; `@value(n)` sets explicitly,
/// `@default_literal` marks the default; `@bit_bound(n)` the
/// bit width.
fn lower_enum_minimal(
    e: &EnumDef,
) -> Result<zerodds_types::type_object::minimal::MinimalEnumeratedType, MapError> {
    let type_anns = lower_annotations(&e.annotations)
        .map_err(|err| MapError::Annotation(alloc::format!("{err:?}")))?;
    let bit_bound = type_anns.builtins.iter().find_map(|a| {
        if let BuiltinAnnotation::BitBound(n) = a {
            Some(*n)
        } else {
            None
        }
    });

    let mut builder = TypeObjectBuilder::enum_type(e.name.text.clone());
    if let Some(bits) = bit_bound {
        builder = builder.bit_bound(bits);
    }
    for (name, value, is_default) in enum_literal_values(e)? {
        builder = if is_default {
            builder.default_literal(name, value)
        } else {
            builder.literal(name, value)
        };
    }
    Ok(builder.build_minimal())
}

/// Computes the ordinal values of the enumerators: sequentially from 0,
/// `@value(n)` sets explicitly (subsequent ones count from `n+1`),
/// `@default_literal` marks the default literal.
fn enum_literal_values(
    e: &EnumDef,
) -> Result<alloc::vec::Vec<(alloc::string::String, i32, bool)>, MapError> {
    let mut out = alloc::vec::Vec::with_capacity(e.enumerators.len());
    let mut next: i32 = 0;
    for en in &e.enumerators {
        let anns = lower_annotations(&en.annotations)
            .map_err(|err| MapError::Annotation(alloc::format!("{err:?}")))?;
        let mut value = next;
        let mut is_default = false;
        for a in &anns.builtins {
            match a {
                BuiltinAnnotation::Value(s) => {
                    if let Ok(v) = s.parse::<i32>() {
                        value = v;
                    }
                }
                BuiltinAnnotation::DefaultLiteral => is_default = true,
                _ => {}
            }
        }
        out.push((en.name.text.clone(), value, is_default));
        next = value.wrapping_add(1);
    }
    Ok(out)
}

/// Discriminator `TypeIdentifier` of a `union switch (...)`.
fn switch_type_id(
    sw: &SwitchTypeSpec,
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<TypeIdentifier, MapError> {
    Ok(match sw {
        SwitchTypeSpec::Integer(i) => {
            TypeIdentifier::Primitive(map_primitive(PrimitiveType::Integer(*i)))
        }
        SwitchTypeSpec::Char => TypeIdentifier::Primitive(PrimitiveKind::Char8),
        SwitchTypeSpec::Boolean => TypeIdentifier::Primitive(PrimitiveKind::Boolean),
        SwitchTypeSpec::Octet => TypeIdentifier::Primitive(PrimitiveKind::Byte),
        SwitchTypeSpec::Scoped(sn) => resolve_scoped(sn, scope, names).ok_or_else(|| {
            MapError::UnresolvedScoped(
                sn.parts
                    .iter()
                    .map(|p| p.text.clone())
                    .collect::<alloc::vec::Vec<_>>()
                    .join("::"),
            )
        })?,
    })
}

/// FQN of a union's discriminator enum, if the switch type is a
/// (resolved) enum — for resolving blank case labels.
fn switch_enum_fqn(
    sw: &SwitchTypeSpec,
    scope: &[alloc::string::String],
    enum_values: &EnumValues,
) -> Option<alloc::string::String> {
    let SwitchTypeSpec::Scoped(sn) = sw else {
        return None;
    };
    let target = sn
        .parts
        .iter()
        .map(|p| p.text.as_str())
        .collect::<alloc::vec::Vec<_>>()
        .join("::");
    resolve_str_fqn(&target, sn.absolute, scope, |c| enum_values.contains_key(c))
}

/// Lowers an IDL `union` into a `MinimalUnionType`.
fn lower_union_minimal(
    u: &UnionDef,
    scope: &[alloc::string::String],
    names: &NameMap,
    enum_values: &EnumValues,
) -> Result<zerodds_types::type_object::minimal::MinimalUnionType, MapError> {
    let disc = switch_type_id(&u.switch_type, scope, names)?;
    let disc_enum = switch_enum_fqn(&u.switch_type, scope, enum_values);

    let type_anns = lower_annotations(&u.annotations)
        .map_err(|e| MapError::Annotation(alloc::format!("{e:?}")))?;
    let extensibility = match type_anns.extensibility() {
        Some(ExtensibilityKind::Final) => Extensibility::Final,
        Some(ExtensibilityKind::Mutable) => Extensibility::Mutable,
        _ => Extensibility::Appendable,
    };

    let mut builder =
        TypeObjectBuilder::union_type(u.name.text.clone(), disc).extensibility(extensibility);
    for case in &u.cases {
        let elem_ti = map_type_spec_resolved(&case.element.type_spec, scope, names)?;
        let name = case.element.declarator.name().text.clone();
        if case
            .labels
            .iter()
            .any(|l| matches!(l, crate::ast::CaseLabel::Default))
        {
            builder = builder.default_case(name, elem_ti);
        } else {
            let mut labels = alloc::vec::Vec::new();
            for label in &case.labels {
                if let crate::ast::CaseLabel::Value(expr) = label {
                    labels.push(eval_case_label(
                        expr,
                        scope,
                        disc_enum.as_deref(),
                        enum_values,
                    )?);
                }
            }
            builder = builder.case(name, elem_ti, labels);
        }
    }
    Ok(builder.build_minimal())
}

/// Evaluates a union case label to an `i32`. Integer/char/
/// boolean literals via the const evaluator; enum labels (`Color::RED`
/// or blank `RED`) via the precomputed `EnumValues` map.
fn eval_case_label(
    expr: &ConstExpr,
    scope: &[alloc::string::String],
    disc_enum: Option<&str>,
    enum_values: &EnumValues,
) -> Result<i32, MapError> {
    if let ConstExpr::Scoped(sn) = expr {
        return resolve_enum_label(sn, scope, disc_enum, enum_values);
    }
    let value = evaluate(expr, &SymbolTable::new())
        .map_err(|e| MapError::Annotation(alloc::format!("union case label: {e:?}")))?;
    value
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or(MapError::UnsupportedTypeSpec(
            "union case label is not an integer",
        ))
}

/// Resolves an enum case label. Blank labels (`case RED:`) against
/// the discriminator enum, qualified ones (`case Color::RED:`) against
/// the enum determined via the prefix.
fn resolve_enum_label(
    sn: &ScopedName,
    scope: &[alloc::string::String],
    disc_enum: Option<&str>,
    enum_values: &EnumValues,
) -> Result<i32, MapError> {
    let Some((last, prefix)) = sn.parts.split_last() else {
        return Err(MapError::UnsupportedTypeSpec("empty union case label"));
    };
    let literal = &last.text;
    let enum_fqn = if prefix.is_empty() {
        disc_enum.map(alloc::string::String::from)
    } else {
        let target = prefix
            .iter()
            .map(|p| p.text.as_str())
            .collect::<alloc::vec::Vec<_>>()
            .join("::");
        resolve_str_fqn(&target, sn.absolute, scope, |c| enum_values.contains_key(c))
    };
    enum_fqn
        .as_deref()
        .and_then(|fqn| enum_values.get(fqn))
        .and_then(|m| m.get(literal))
        .copied()
        .ok_or_else(|| MapError::UnresolvedScoped(literal.clone()))
}

/// Lowers a `typedef` declarator into a `MinimalAliasType`.
fn lower_alias_minimal(
    name: &str,
    underlying: &crate::ast::TypeSpec,
    array_sizes: &[ConstExpr],
    scope: &[alloc::string::String],
    names: &NameMap,
) -> Result<zerodds_types::type_object::minimal::MinimalAliasType, MapError> {
    let element = map_type_spec_resolved(underlying, scope, names)?;
    let target = if array_sizes.is_empty() {
        element
    } else {
        make_array_ti(element, array_sizes)?
    };
    Ok(TypeObjectBuilder::alias(name.to_string(), target).build_minimal())
}

/// Builds a `PlainArray` `TypeIdentifier` from element + dimensions.
fn make_array_ti(element: TypeIdentifier, sizes: &[ConstExpr]) -> Result<TypeIdentifier, MapError> {
    let mut dims = alloc::vec::Vec::with_capacity(sizes.len());
    for s in sizes {
        dims.push(eval_const_u32(s)?);
    }
    let header = zerodds_types::PlainCollectionHeader::for_element(collection_equiv_kind(&element));
    Ok(if dims.iter().all(|&d| d <= u32::from(u8::MAX)) {
        TypeIdentifier::PlainArraySmall {
            header,
            array_bounds: dims.iter().map(|&d| d as u8).collect(),
            element: alloc::boxed::Box::new(element),
        }
    } else {
        TypeIdentifier::PlainArrayLarge {
            header,
            array_bounds: dims,
            element: alloc::boxed::Box::new(element),
        }
    })
}

/// Evaluates a `ConstExpr` to a `u32` (array dimension).
fn eval_const_u32(expr: &ConstExpr) -> Result<u32, MapError> {
    let value = evaluate(expr, &SymbolTable::new())
        .map_err(|e| MapError::Annotation(alloc::format!("array bound: {e:?}")))?;
    value
        .as_i64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(MapError::UnsupportedTypeSpec(
            "array dimension is not a non-negative integer",
        ))
}

extern crate alloc;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn first_struct(src: &str) -> StructDef {
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        for def in ast.definitions {
            if let crate::ast::Definition::Type(crate::ast::TypeDecl::Constr(
                crate::ast::ConstrTypeDecl::Struct(crate::ast::StructDcl::Def(s)),
            )) = def
            {
                return s;
            }
        }
        panic!("no struct");
    }

    #[test]
    fn simple_struct_lowers() {
        let s = first_struct("struct S { long id; string text; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 2);
        assert!(matches!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::Primitive(PrimitiveKind::Int32)
        ));
        assert!(matches!(
            st.member_seq[1].common.member_type_id,
            TypeIdentifier::String8Small { .. }
        ));
    }

    #[test]
    fn key_and_id_annotations_carry_through() {
        let s = first_struct("struct S { @key @id(5) long id; long extra; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq[0].common.member_id, 5);
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_KEY)
        );
    }

    #[test]
    fn mutable_extensibility_applied() {
        let s = first_struct("@mutable struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_MUTABLE)
        );
    }

    #[test]
    fn sequence_of_ints_lowers() {
        let s = first_struct("struct S { sequence<long, 10> items; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(matches!(
            st.member_seq[0].common.member_type_id,
            TypeIdentifier::PlainSequenceSmall { bound: 10, .. }
        ));
    }

    #[test]
    fn optional_annotation_sets_flag() {
        let s = first_struct("struct S { @optional long maybe; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_OPTIONAL)
        );
    }

    #[test]
    fn non_serialized_member_is_dropped_from_typeobject() {
        // §7.2.4.4.2 — @non_serialized members are ABI-only, not in the
        // wire form and not in the assignability comparison.
        let s = first_struct(
            "struct S { long visible; @non_serialized long hidden; long also_visible; };",
        );
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 2);
        // Member order is preserved (visible, also_visible).
        // No ID annotation → IDs are allocated; just ensure
        // that the hidden member is missing.
        // Since we cannot easily grab a name list, we check
        // that the type-identifier type is correct and the length fits.
    }

    #[test]
    fn non_serialized_in_otherwise_empty_struct_yields_empty_member_seq() {
        let s = first_struct("struct S { @non_serialized long internal_only; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 0);
    }

    #[test]
    fn non_serialized_member_does_not_block_assignability() {
        // The reader has @non_serialized for a member; the writer does not
        // have it at all. In the wire comparison (assignability), both
        // member sequences should be identical → no blocking
        // mismatch.
        let writer_src = "struct S { long a; long b; };";
        let reader_src = "struct S { long a; @non_serialized long debug_only; long b; };";
        let writer = lower_struct_to_minimal(&first_struct(writer_src)).unwrap();
        let reader = lower_struct_to_minimal(&first_struct(reader_src)).unwrap();
        // Both must have the same count + the same member-identifier sequence.
        assert_eq!(writer.member_seq.len(), reader.member_seq.len());
        for (w, r) in writer.member_seq.iter().zip(&reader.member_seq) {
            assert_eq!(w.common.member_type_id, r.common.member_type_id);
        }
    }

    #[test]
    fn non_serialized_with_other_annotations_still_dropped() {
        // @key + @non_serialized — non_serialized takes precedence, the member is dropped.
        let s = first_struct("struct S { @key @non_serialized long ghost; long real; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 1);
        assert!(
            !st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_KEY)
        );
    }

    // ---- map_type_spec: every Primitive variant --------------------------

    use crate::ast::Identifier;
    use crate::ast::{
        ConstExpr, FixedPtType, FloatingType, IntegerType, Literal, LiteralKind, MapType,
        PrimitiveType, ScopedName, SequenceType, StringType, TypeSpec,
    };
    use crate::errors::Span;

    fn sp() -> Span {
        Span::SYNTHETIC
    }

    fn int_lit(raw: &str) -> ConstExpr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw: raw.to_string(),
            span: sp(),
        })
    }

    fn prim(p: PrimitiveType) -> TypeSpec {
        TypeSpec::Primitive(p)
    }

    #[test]
    fn map_primitive_all_integer_kinds() {
        let cases = [
            (IntegerType::Short, PrimitiveKind::Int16),
            (IntegerType::Long, PrimitiveKind::Int32),
            (IntegerType::LongLong, PrimitiveKind::Int64),
            (IntegerType::UShort, PrimitiveKind::UInt16),
            (IntegerType::ULong, PrimitiveKind::UInt32),
            (IntegerType::ULongLong, PrimitiveKind::UInt64),
            (IntegerType::Int8, PrimitiveKind::Int8),
            (IntegerType::Int16, PrimitiveKind::Int16),
            (IntegerType::Int32, PrimitiveKind::Int32),
            (IntegerType::Int64, PrimitiveKind::Int64),
            (IntegerType::UInt8, PrimitiveKind::UInt8),
            (IntegerType::UInt16, PrimitiveKind::UInt16),
            (IntegerType::UInt32, PrimitiveKind::UInt32),
            (IntegerType::UInt64, PrimitiveKind::UInt64),
        ];
        for (idl, xt) in cases {
            let ti = map_type_spec(&prim(PrimitiveType::Integer(idl))).unwrap();
            assert_eq!(ti, TypeIdentifier::Primitive(xt));
        }
    }

    #[test]
    fn map_primitive_floats_boolean_char_octet() {
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::Float))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float32)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::Double))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float64)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Floating(FloatingType::LongDouble))).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Float128)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Boolean)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Boolean)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Octet)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Byte)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::Char)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Char8)
        );
        assert_eq!(
            map_type_spec(&prim(PrimitiveType::WideChar)).unwrap(),
            TypeIdentifier::Primitive(PrimitiveKind::Char16)
        );
    }

    // ---- String kinds ----------------------------------------------------

    #[test]
    fn map_string_unbounded_narrow_is_small_with_bound_zero() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: None,
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(ti, TypeIdentifier::String8Small { bound: 0 }));
    }

    #[test]
    fn map_string_bounded_narrow_small() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: Some(int_lit("128")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(ti, TypeIdentifier::String8Small { bound: 128 });
    }

    #[test]
    fn map_string_large_narrow_over_255() {
        let ti = map_type_spec(&TypeSpec::String(StringType {
            wide: false,
            bound: Some(int_lit("1000")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(ti, TypeIdentifier::String8Large { bound: 1000 });
    }

    #[test]
    fn map_string_wide_small_and_large() {
        let wide_small = map_type_spec(&TypeSpec::String(StringType {
            wide: true,
            bound: Some(int_lit("64")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(wide_small, TypeIdentifier::String16Small { bound: 64 });

        let wide_large = map_type_spec(&TypeSpec::String(StringType {
            wide: true,
            bound: Some(int_lit("70000")),
            span: sp(),
        }))
        .unwrap();
        assert_eq!(wide_large, TypeIdentifier::String16Large { bound: 70000 });
    }

    // ---- Sequence kinds --------------------------------------------------

    #[test]
    fn map_sequence_small_and_large_bounds() {
        let small = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: Some(int_lit("100")),
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            small,
            TypeIdentifier::PlainSequenceSmall { bound: 100, .. }
        ));

        let large = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: Some(int_lit("1000")),
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            large,
            TypeIdentifier::PlainSequenceLarge { bound: 1000, .. }
        ));
    }

    #[test]
    fn map_sequence_unbounded_is_small_with_bound_zero() {
        let ti = map_type_spec(&TypeSpec::Sequence(SequenceType {
            elem: alloc::boxed::Box::new(prim(PrimitiveType::Boolean)),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            ti,
            TypeIdentifier::PlainSequenceSmall { bound: 0, .. }
        ));
    }

    // ---- Error paths -----------------------------------------------------

    #[test]
    fn map_scoped_returns_unresolved_scoped() {
        let scoped = ScopedName {
            absolute: false,
            parts: alloc::vec![Identifier::new("Ns", sp()), Identifier::new("Inner", sp()),],
            span: sp(),
        };
        let err = map_type_spec(&TypeSpec::Scoped(scoped)).unwrap_err();
        match err {
            MapError::UnresolvedScoped(p) => assert_eq!(p, "Ns::Inner"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn map_fixed_is_unsupported() {
        let err = map_type_spec(&TypeSpec::Fixed(FixedPtType {
            digits: int_lit("10"),
            scale: int_lit("2"),
            span: sp(),
        }))
        .unwrap_err();
        assert_eq!(err, MapError::UnsupportedTypeSpec("fixed"));
    }

    #[test]
    fn map_now_lowers_to_plain_map() {
        // Bug TO (#71): inline map<K,V> previously returned
        // UnsupportedTypeSpec("map (inline IDL map)"); it now lowers to a
        // TK_MAP PlainMap TypeIdentifier (see map_of_primitives_lowers_to_plain_map).
        let ti = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(ti, TypeIdentifier::PlainMapSmall { .. }));
    }

    #[test]
    fn map_any_is_unsupported() {
        let err = map_type_spec(&TypeSpec::Any).unwrap_err();
        assert_eq!(err, MapError::UnsupportedTypeSpec("any"));
    }

    // ---- Bug TO (#71): inline map<K,V> → TK_MAP TypeIdentifier -----------

    #[test]
    fn map_of_primitives_lowers_to_plain_map() {
        // map<long,long> (unbounded) → PlainMapSmall(bound=0) with the key +
        // value TypeIdentifiers populated. Previously this returned
        // UnsupportedTypeSpec("map (inline IDL map)").
        let ti = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        match ti {
            TypeIdentifier::PlainMapSmall {
                bound,
                element,
                key,
                ..
            } => {
                assert_eq!(bound, 0);
                assert_eq!(*key, TypeIdentifier::Primitive(PrimitiveKind::Int32));
                assert_eq!(*element, TypeIdentifier::Primitive(PrimitiveKind::Int32));
            }
            other => panic!("expected PlainMapSmall, got {other:?}"),
        }
    }

    #[test]
    fn map_string_key_long_value_lowers() {
        // map<string,long> — the exact shape of 13_maps / 20_mixed_combo.
        let ti = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(TypeSpec::String(StringType {
                wide: false,
                bound: None,
                span: sp(),
            })),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: None,
            span: sp(),
        }))
        .unwrap();
        match ti {
            TypeIdentifier::PlainMapSmall { key, element, .. } => {
                assert!(matches!(*key, TypeIdentifier::String8Small { .. }));
                assert_eq!(*element, TypeIdentifier::Primitive(PrimitiveKind::Int32));
            }
            other => panic!("expected PlainMapSmall, got {other:?}"),
        }
    }

    #[test]
    fn map_large_bound_uses_plain_map_large() {
        let ti = map_type_spec(&TypeSpec::Map(MapType {
            key: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            value: alloc::boxed::Box::new(prim(PrimitiveType::Integer(IntegerType::Long))),
            bound: Some(int_lit("1000")),
            span: sp(),
        }))
        .unwrap();
        assert!(matches!(
            ti,
            TypeIdentifier::PlainMapLarge { bound: 1000, .. }
        ));
    }

    #[test]
    fn registry_struct_with_map_member_emits_typeobject() {
        // The whole-struct TypeObject must NOT be skipped just because a
        // member is a map. Mirrors 13_maps.idl: a keyed struct with a
        // map<string,long> member must lower to a real TypeObject.
        let lowered =
            registry("@appendable struct Maps { @key long id; map<string,long> counters; };");
        assert!(lowered.names.contains_key("Maps"));
        // And it serialises cleanly (this is what idlc's blob emission does).
        let (_, obj) = lowered.iter().find(|(n, _)| *n == "Maps").unwrap();
        let bytes = obj.to_bytes_le().expect("serialise map-bearing TypeObject");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn registry_map_member_carries_map_type_id() {
        // The map member's TypeIdentifier inside the struct TypeObject is a
        // PlainMap (TK_MAP), not a placeholder/skip.
        let lowered = registry("struct M { map<long,long> table; };");
        let (_, obj) = lowered.iter().find(|(n, _)| *n == "M").unwrap();
        let MinimalTypeObject::Struct(st) = obj else {
            panic!("expected struct TypeObject");
        };
        assert_eq!(st.member_seq.len(), 1);
        assert!(
            matches!(
                st.member_seq[0].common.member_type_id,
                TypeIdentifier::PlainMapSmall { .. } | TypeIdentifier::PlainMapLarge { .. }
            ),
            "map member must carry a TK_MAP TypeIdentifier, got {:?}",
            st.member_seq[0].common.member_type_id
        );
    }

    #[test]
    fn registry_map_of_named_value_resolves_and_sets_equiv_kind() {
        // map<long, Point> — the value is a named struct, so the map's
        // value element must resolve to a minimal hash and the collection
        // header's equiv_kind flips to EK_MINIMAL.
        let lowered =
            registry("struct Point { long x; long y; }; struct Grid { map<long,Point> cells; };");
        let (_, obj) = lowered.iter().find(|(n, _)| *n == "Grid").unwrap();
        let MinimalTypeObject::Struct(st) = obj else {
            panic!("expected struct TypeObject");
        };
        match &st.member_seq[0].common.member_type_id {
            TypeIdentifier::PlainMapSmall {
                header, element, ..
            }
            | TypeIdentifier::PlainMapLarge {
                header, element, ..
            } => {
                assert!(matches!(
                    **element,
                    TypeIdentifier::EquivalenceHashMinimal(_)
                ));
                assert_eq!(header.equiv_kind, EK_MINIMAL);
            }
            other => panic!("expected PlainMap, got {other:?}"),
        }
    }

    // ---- lower_struct_to_minimal struct-level annotations ----------------

    #[test]
    fn struct_autoid_hash_sets_flag() {
        let s = first_struct("@autoid(HASH) struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_AUTOID_HASH)
        );
    }

    #[test]
    fn struct_nested_sets_flag() {
        let s = first_struct("@nested struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_NESTED)
        );
    }

    #[test]
    fn struct_explicit_final_extensibility_sets_flag() {
        let s = first_struct("@extensibility(FINAL) struct S { long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.struct_flags
                .has(zerodds_types::type_object::flags::StructTypeFlag::IS_FINAL)
        );
    }

    #[test]
    fn struct_multi_declarator_expands_to_separate_members() {
        let s = first_struct("struct S { long a, b, c; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert_eq!(st.member_seq.len(), 3);
        for m in &st.member_seq {
            assert!(matches!(
                m.common.member_type_id,
                TypeIdentifier::Primitive(PrimitiveKind::Int32)
            ));
        }
    }

    #[test]
    fn struct_with_must_understand_flag() {
        let s = first_struct("struct S { @must_understand long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_MUST_UNDERSTAND)
        );
    }

    #[test]
    fn struct_with_external_flag() {
        let s = first_struct("struct S { @external long x; };");
        let st = lower_struct_to_minimal(&s).unwrap();
        assert!(
            st.member_seq[0]
                .common
                .member_flags
                .has(zerodds_types::type_object::flags::StructMemberFlag::IS_EXTERNAL)
        );
    }

    #[test]
    fn struct_with_scoped_member_returns_unresolved() {
        let s = first_struct("struct S { Foo x; };");
        let err = lower_struct_to_minimal(&s).unwrap_err();
        match err {
            MapError::UnresolvedScoped(p) => assert_eq!(p, "Foo"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    fn registry(src: &str) -> LoweredSpec {
        let ast = parse(src, &ParserConfig::default()).expect("parse");
        build_type_registry(&ast).expect("build_type_registry")
    }

    #[test]
    fn registry_resolves_struct_of_struct() {
        // The isolated mapper fails here on `Inner` — the
        // registry resolves the forward ref via the two-pass.
        let lowered = registry("struct Inner { long a; }; struct Outer { Inner i; };");
        assert!(lowered.names.contains_key("Inner"));
        assert!(lowered.names.contains_key("Outer"));
        let (min, _) = lowered.registry.len();
        assert_eq!(min, 2);
    }

    #[test]
    fn registry_topo_order_puts_dependency_first() {
        let lowered = registry("struct Inner { long a; }; struct Outer { Inner i; };");
        let inner = lowered.order.iter().position(|n| n == "Inner").unwrap();
        let outer = lowered.order.iter().position(|n| n == "Outer").unwrap();
        assert!(inner < outer, "dependency must be lowered first");
    }

    #[test]
    fn registry_lowers_enum_and_struct_referencing_it() {
        let lowered = registry("enum Color { RED, GREEN }; struct S { Color c; };");
        assert!(lowered.names.contains_key("Color"));
        assert!(lowered.names.contains_key("S"));
    }

    #[test]
    fn registry_resolves_module_scoped_reference() {
        let lowered = registry("module M { struct A { long x; }; }; struct B { M::A a; };");
        assert!(lowered.names.contains_key("M::A"));
        assert!(lowered.names.contains_key("B"));
    }

    #[test]
    fn registry_detects_recursive_type() {
        // sequence<Node> is legal IDL, but creates a cycle
        // Node → Node in the dependency graph.
        let ast = parse(
            "struct Node { long id; sequence<Node> kids; };",
            &ParserConfig::default(),
        )
        .expect("parse");
        match build_type_registry(&ast) {
            Err(MapError::RecursiveType(types)) => assert!(types.contains("Node")),
            other => panic!("expected RecursiveType, got {other:?}"),
        }
    }

    #[test]
    fn registry_distinct_types_get_distinct_hashes() {
        let lowered = registry("struct A { long a; }; struct B { double b; };");
        let a = lowered.names.get("A").unwrap();
        let b = lowered.names.get("B").unwrap();
        assert_ne!(a, b, "structurally different types must hash differently");
    }

    #[test]
    fn registry_lowers_union() {
        let lowered = registry(
            "union U switch (long) { case 1: long x; case 2: double y; default: boolean z; };",
        );
        assert!(lowered.names.contains_key("U"));
    }

    #[test]
    fn registry_union_over_enum_resolves_bare_labels() {
        // Bare case labels (`case RED:`) against the discriminator enum.
        let lowered = registry(
            "enum Color { RED, GREEN, BLUE }; \
             union U switch (Color) { case RED: long r; default: double d; };",
        );
        assert!(lowered.names.contains_key("Color"));
        assert!(lowered.names.contains_key("U"));
    }

    #[test]
    fn registry_struct_with_union_member() {
        let lowered =
            registry("union U switch (long) { case 1: long x; }; struct S { U u; long n; };");
        assert!(lowered.names.contains_key("U"));
        assert!(lowered.names.contains_key("S"));
        let u = lowered.order.iter().position(|n| n == "U").unwrap();
        let s = lowered.order.iter().position(|n| n == "S").unwrap();
        assert!(u < s, "union must be lowered before the struct using it");
    }

    #[test]
    fn registry_lowers_scalar_typedef() {
        let lowered = registry("typedef long MyLong; struct S { MyLong v; };");
        assert!(lowered.names.contains_key("MyLong"));
        assert!(lowered.names.contains_key("S"));
    }

    #[test]
    fn registry_lowers_array_typedef() {
        let lowered = registry("typedef long Matrix[3][3];");
        assert!(lowered.names.contains_key("Matrix"));
    }

    #[test]
    fn registry_multi_declarator_typedef_yields_separate_aliases() {
        let lowered = registry("typedef double Lat, Lon;");
        assert!(lowered.names.contains_key("Lat"));
        assert!(lowered.names.contains_key("Lon"));
    }
}
