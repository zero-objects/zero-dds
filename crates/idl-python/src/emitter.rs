// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL → Python codegen — emit logic.
//!
//! Generates, for an IDL specification, a Python module that uses the
//! `@idl_struct(typename=...)`/`@dataclass` convention of the
//! `zerodds-py` crate. Generated classes are usable directly with
//! `zerodds.IdlTopic(MyClass)`.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use zerodds_idl::ast::types::{
    BinaryOp, BitmaskDecl, BitsetDecl, CaseLabel, ConstDecl, ConstExpr, ConstType, ConstrTypeDecl,
    Declarator, Definition, EnumDef, ExceptDecl, FloatingType, Identifier, IntegerType, Literal,
    LiteralKind, Member, ModuleDef, PrimitiveType, ScopedName, SequenceType, Specification,
    StringType, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, TypedefDecl, UnaryOp,
    UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations,
};

use crate::error::{IdlPythonError, Result};

/// Codegen options for Python modules.
#[derive(Debug, Clone, Default)]
pub struct PythonGenOptions {
    /// Optional header comment that lands at the top of the generated
    /// file. Newlines are emitted as separate `#` comment lines.
    pub header_comment: Option<String>,
}

thread_local! {
    /// Enum literal simple-name → discriminant value, matching `emit_enum`
    /// (sequential `0..N-1`). Lets `union switch(EnumT)` resolve its
    /// `case ENUM_LITERAL:` labels to the same integers the generated IntEnum
    /// uses. Populated by [`register_enum_values`] at the start of each run.
    static ENUM_VALUES: std::cell::RefCell<std::collections::BTreeMap<String, i64>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };

    /// Integer `const` values by simple name, so a later `const` (or array
    /// bound) that references an earlier one folds to its value (Bug M / #56).
    static CONST_VALUES: std::cell::RefCell<std::collections::BTreeMap<String, i64>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };

    /// Every named type declaration, stored as its fully-qualified IDL scope
    /// path (e.g. `["combo", "Reading"]`). A reference site resolves a
    /// (possibly partially-qualified) `ScopedName` against the enclosing scope
    /// by walking outward and matching one of these paths, then flattens it the
    /// SAME way the definition site does (`join("_")`). Without this a member
    /// `Reading` inside `module combo` would emit the bare name `Reading`,
    /// but the class is defined flattened as `combo_Reading` (Bug Q-cluster).
    static TYPE_PATHS: std::cell::RefCell<Vec<Vec<String>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Records the fully-qualified path of every named type declaration before
/// emission, so reference-rendering can resolve names against the enclosing
/// scope and produce the same flattened name as the definition site.
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn register_type_paths(defs: &[Definition], scope: &mut Vec<String>) {
    for def in defs {
        let name: Option<&Identifier> = match def {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                register_type_paths(&m.definitions, scope);
                scope.pop();
                None
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                Some(&s.name)
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => Some(&e.name),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => Some(&b.name),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => Some(&b.name),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                Some(&u.name)
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                for d in &td.declarators {
                    let mut path = scope.clone();
                    path.push(d.name().text.clone());
                    TYPE_PATHS.with(|t| t.borrow_mut().push(path));
                }
                None
            }
            Definition::Except(e) => Some(&e.name),
            Definition::Const(c) => Some(&c.name),
            _ => None,
        };
        if let Some(id) = name {
            let mut path = scope.clone();
            path.push(id.text.clone());
            TYPE_PATHS.with(|t| t.borrow_mut().push(path));
        }
    }
}

/// Resolves a referenced `ScopedName` against the enclosing `scope`, returning
/// the flattened Python name (`join("_")`) of the matching declaration. IDL
/// name lookup walks outward from the innermost scope (§7.5.2); we mirror that:
/// for each prefix of `scope` (longest first), check whether
/// `prefix + name.parts` matches a registered type path. The matched path is
/// flattened the same way `python_class_name` flattens definitions, so a
/// reference and its definition always agree.
fn resolve_scoped_name(name: &ScopedName, scope: &[String]) -> String {
    let parts: Vec<String> = name.parts.iter().map(|p| p.text.clone()).collect();
    let known: Vec<Vec<String>> = TYPE_PATHS.with(|t| t.borrow().clone());
    // Walk outward: try the full enclosing scope first, then shorter prefixes,
    // finally the global scope (empty prefix).
    for cut in (0..=scope.len()).rev() {
        let mut candidate = scope[..cut].to_vec();
        candidate.extend(parts.iter().cloned());
        if known.contains(&candidate) {
            return candidate.join("_");
        }
    }
    // No registered match (e.g. a built-in/forward-only name): fall back to the
    // literal flattening of whatever parts were written.
    parts.join("_")
}

/// `true` if the member carries an `@optional` annotation (Bug R5 / #64).
fn member_is_optional(m: &Member) -> bool {
    let Ok(lowered) = lower_annotations(&m.annotations) else {
        return false;
    };
    lowered
        .builtins
        .iter()
        .any(|b| matches!(b, BuiltinAnnotation::Optional))
}

/// The explicit `@id(N)` of a member, if any. A `@mutable` struct frames each
/// member with an EMHEADER carrying this member id (XTypes 1.3 §7.4.3.4.2); the
/// Python runtime needs the exact ids so its EMHEADERs match the cross-vendor
/// reference. Absent ids fall back to the SEQUENTIAL default (1-based) computed
/// by the caller.
fn member_explicit_id(m: &Member) -> Option<u32> {
    let lowered = lower_annotations(&m.annotations).ok()?;
    lowered.builtins.iter().find_map(|b| match b {
        BuiltinAnnotation::Id(n) => Some(*n),
        _ => None,
    })
}

/// The effective `@bit_bound` of a bitmask — the wire holder width in bits.
/// XTypes 1.3 §7.3.1.2.1.1: the DEFAULT bit_bound for a bitmask is **32** (→ a
/// uint32 holder), so a 3-flag bitmask without an explicit `@bit_bound` is still
/// 4 bytes on the wire (matching CycloneDDS/RTI/FastDDS and the rust backend's
/// `bitmask_bit_bound`). The runtime sizes the holder from this value, NOT from
/// the flag count.
fn bitmask_bit_bound(b: &BitmaskDecl) -> u32 {
    lower_annotations(&b.annotations)
        .ok()
        .and_then(|l| {
            l.builtins.iter().find_map(|a| match a {
                BuiltinAnnotation::BitBound(n) => Some(u32::from(*n)),
                _ => None,
            })
        })
        .unwrap_or(32)
}

/// The effective `@bit_bound` of an enum — the signed wire holder width in bits
/// (XTypes 1.3 §7.4.5.1 / §7.3.1.2.1.2): DEFAULT 32. The runtime narrows the
/// holder to int8 (≤8) / int16 (≤16) / int32 from this value; Cyclone honours it.
fn enum_bit_bound(e: &EnumDef) -> u32 {
    lower_annotations(&e.annotations)
        .ok()
        .and_then(|l| {
            l.builtins.iter().find_map(|a| match a {
                BuiltinAnnotation::BitBound(n) => Some(u32::from(*n)),
                _ => None,
            })
        })
        .filter(|&v| (1..=32).contains(&v))
        .unwrap_or(32)
}

/// Computes the per-field member-id list for a `@mutable` struct in declaration
/// order: an explicit `@id(N)` wins; otherwise the XTypes default autoid
/// SEQUENTIAL assigns `prev_id + 1` (starting at 0, so the first un-annotated
/// member is id 0 — §7.2.2.4.4). One id is produced per *declarator* (a single
/// IDL `long a, b;` declares two members). Mirrors the Rust backend's
/// `vec![10, 20, 30]` id list.
fn mutable_member_ids(s: &StructDef) -> Vec<u32> {
    let mut ids = Vec::new();
    let mut next: u32 = 0;
    for m in &s.members {
        let explicit = member_explicit_id(m);
        for _ in &m.declarators {
            let id = explicit.unwrap_or(next);
            ids.push(id);
            next = id.saturating_add(1);
        }
    }
    ids
}

/// Records every enum literal's value before emission, so a union case label
/// referencing an enumerator resolves to the discriminant `emit_enum` assigns.
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn register_enum_values(defs: &[Definition]) {
    for def in defs {
        match def {
            Definition::Module(m) => register_enum_values(&m.definitions),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                for (idx, en) in e.enumerators.iter().enumerate() {
                    ENUM_VALUES.with(|m| {
                        m.borrow_mut().insert(en.name.text.clone(), idx as i64);
                    });
                }
            }
            Definition::Const(c) => {
                // Register integer consts so later consts/array bounds that
                // reference them fold (declaration order is respected: only
                // already-seen consts are visible, matching IDL §7.4.1.4.4).
                if let Some(v) = eval_const_int(&c.value) {
                    CONST_VALUES.with(|m| {
                        m.borrow_mut().insert(c.name.text.clone(), v);
                    });
                }
            }
            _ => {}
        }
    }
}

/// Codegen entry point — IDL spec → complete Python source.
///
/// # Errors
///
/// `IdlPythonError::Unsupported` for IDL constructs that the Python
/// mapping does not (yet) support: `valuetype`, `interface`,
/// `fixed`, `map`, `any`.
pub fn generate_python_module(spec: &Specification, opts: &PythonGenOptions) -> Result<String> {
    ENUM_VALUES.with(|m| m.borrow_mut().clear());
    CONST_VALUES.with(|m| m.borrow_mut().clear());
    register_enum_values(&spec.definitions);
    TYPE_PATHS.with(|t| t.borrow_mut().clear());
    {
        let mut path_scope: Vec<String> = Vec::new();
        register_type_paths(&spec.definitions, &mut path_scope);
    }
    let mut imports = ImportSet::default();
    collect_imports(&spec.definitions, &mut imports)?;

    let mut out = String::new();
    out.push_str("# SPDX-License-Identifier: Apache-2.0\n");
    if let Some(c) = &opts.header_comment {
        for line in c.lines() {
            out.push_str("# ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("# Auto-generated by `zerodds-idl-python`. Do not edit by hand.\n");
    out.push('\n');

    imports.emit(&mut out);

    let mut scope: Vec<String> = Vec::new();
    emit_definitions(&mut out, &spec.definitions, &mut scope)?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// Import-Sammlung
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ImportSet {
    dataclass: bool,
    int_enum: bool,
    int_flag: bool,
    typing_list: bool,
    typing_dict: bool,
    typing_type_alias: bool,
    zerodds_brands: BTreeSet<&'static str>,
    zerodds_idl_struct: bool,
    zerodds_idl_union: bool,
}

impl ImportSet {
    fn emit(&self, out: &mut String) {
        if self.dataclass {
            out.push_str("from dataclasses import dataclass\n");
        }
        if self.int_enum || self.int_flag {
            out.push_str("from enum import ");
            let mut parts: Vec<&str> = Vec::new();
            if self.int_enum {
                parts.push("IntEnum");
            }
            if self.int_flag {
                parts.push("IntFlag");
            }
            out.push_str(&parts.join(", "));
            out.push('\n');
        }
        if self.typing_list || self.typing_dict || self.typing_type_alias {
            out.push_str("from typing import ");
            let mut parts: Vec<&str> = Vec::new();
            if self.typing_dict {
                parts.push("Dict");
            }
            if self.typing_list {
                parts.push("List");
            }
            if self.typing_type_alias {
                parts.push("TypeAlias");
            }
            out.push_str(&parts.join(", "));
            out.push('\n');
        }
        if self.zerodds_idl_struct || self.zerodds_idl_union || !self.zerodds_brands.is_empty() {
            out.push_str("from zerodds.idl import ");
            let mut parts: Vec<String> = Vec::new();
            if self.zerodds_idl_struct {
                parts.push("idl_struct".into());
            }
            if self.zerodds_idl_union {
                parts.push("idl_union".into());
            }
            for brand in &self.zerodds_brands {
                parts.push((*brand).to_string());
            }
            out.push_str(&parts.join(", "));
            out.push('\n');
        }
        out.push('\n');
    }
}

/// zerodds-lint: recursion-depth 32
fn collect_imports(defs: &[Definition], imports: &mut ImportSet) -> Result<()> {
    for d in defs {
        match d {
            Definition::Module(m) => collect_imports(&m.definitions, imports)?,
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                imports.dataclass = true;
                imports.zerodds_idl_struct = true;
                for m in &s.members {
                    collect_type_imports(&m.type_spec, imports)?;
                    collect_member_brand_imports(m, imports);
                }
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(_))) => {
                imports.int_enum = true;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(_))) => {
                imports.int_flag = true;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(_))) => {
                // Bitset: emit as a `TypeAlias = Bitset[total_bits]` (the runtime
                // brand that selects the spec-correct unsigned holder width) +
                // the `*_Bits` SHIFT/MASK helper. Needs the `Bitset` brand and
                // `TypeAlias` for the alias statement.
                imports.zerodds_brands.insert("Bitset");
                imports.typing_type_alias = true;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                imports.zerodds_idl_union = true;
                imports.int_enum = true;
                collect_switch_type_imports(&u.switch_type, imports);
                for case in &u.cases {
                    collect_type_imports(&case.element.type_spec, imports)?;
                }
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                imports.typing_type_alias = true;
                collect_type_imports(&td.type_spec, imports)?;
                if td
                    .declarators
                    .iter()
                    .any(|d| matches!(d, Declarator::Array(_)))
                {
                    imports.zerodds_brands.insert("Array");
                }
            }
            Definition::Except(e) => {
                imports.dataclass = true;
                imports.zerodds_idl_struct = true;
                for m in &e.members {
                    collect_type_imports(&m.type_spec, imports)?;
                    collect_member_brand_imports(m, imports);
                }
            }
            _ => {
                // Forward decls + const + interface + valuetype + RTI
                // vendor constructs are reported as Unsupported or
                // ignored in emit_definitions. For the import
                // pass we can silently skip them here.
            }
        }
    }
    Ok(())
}

/// Adds the `Array`/`Optional` brand imports a member needs based on its
/// declarators (fixed arrays) and `@optional` annotation. The type-spec import
/// pass alone misses these because they are carried on the declarator/member,
/// not the type-spec.
fn collect_member_brand_imports(m: &Member, imports: &mut ImportSet) {
    if m.declarators
        .iter()
        .any(|d| matches!(d, Declarator::Array(_)))
    {
        imports.zerodds_brands.insert("Array");
    }
    if member_is_optional(m) {
        imports.zerodds_brands.insert("Optional");
    }
}

/// zerodds-lint: recursion-depth 32
fn collect_type_imports(ts: &TypeSpec, imports: &mut ImportSet) -> Result<()> {
    match ts {
        TypeSpec::Primitive(p) => {
            imports.zerodds_brands.insert(brand_for_primitive(*p));
        }
        TypeSpec::String(s) => {
            if s.bound.as_ref().and_then(eval_const_int).is_some() {
                imports.zerodds_brands.insert(if s.wide {
                    "BoundedWString"
                } else {
                    "BoundedString"
                });
            } else {
                imports.zerodds_brands.insert(brand_for_string(s));
            }
        }
        TypeSpec::Sequence(seq) => {
            // A bounded sequence emits the `Sequence[T, N]` brand (not
            // `List[T]`), so it needs the `Sequence` brand imported.
            if seq.bound.as_ref().and_then(eval_const_int).is_some() {
                imports.zerodds_brands.insert("Sequence");
            } else {
                imports.typing_list = true;
            }
            collect_type_imports(&seq.elem, imports)?;
        }
        TypeSpec::Scoped(_) => {
            // Local reference to another type in the same module —
            // no import needed.
        }
        TypeSpec::Map(m) => {
            // A bounded map emits the `Map[K, V, N]` brand (not `Dict[K, V]`).
            if m.bound.as_ref().and_then(eval_const_int).is_some() {
                imports.zerodds_brands.insert("Map");
            } else {
                imports.typing_dict = true;
            }
            collect_type_imports(&m.key, imports)?;
            collect_type_imports(&m.value, imports)?;
        }
        TypeSpec::Fixed(_) => {
            // fixed<P,S> -> the `Fixed[P,S]` runtime brand (CORBA-BCD codec).
            imports.zerodds_brands.insert("Fixed");
        }
        TypeSpec::Any => {
            return Err(IdlPythonError::Unsupported(format!(
                "type spec not yet supported in python codegen: {ts:?}"
            )));
        }
    }
    Ok(())
}

fn collect_switch_type_imports(switch: &SwitchTypeSpec, imports: &mut ImportSet) {
    match switch {
        SwitchTypeSpec::Integer(i) => {
            imports.zerodds_brands.insert(brand_for_integer(*i));
        }
        SwitchTypeSpec::Boolean => {
            // bool is a Python builtin, no brand import needed — the
            // `_kind_from_annotation` fallback maps it to Bool.
        }
        SwitchTypeSpec::Octet => {
            imports.zerodds_brands.insert("Octet");
        }
        SwitchTypeSpec::Char => {
            imports.zerodds_brands.insert("Char");
        }
        SwitchTypeSpec::Scoped(_) => {
            // Probably an IntEnum — referenced via the defined type,
            // no extra brand import.
        }
    }
}

// ---------------------------------------------------------------------------
// Definitions-Emit
// ---------------------------------------------------------------------------

/// zerodds-lint: recursion-depth 32
fn emit_definitions(out: &mut String, defs: &[Definition], scope: &mut Vec<String>) -> Result<()> {
    for d in defs {
        match d {
            Definition::Module(m) => emit_module(out, m, scope)?,
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                emit_struct(out, s, scope)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Forward(_)))) => {
                // Forward declarations need no code in Python.
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                emit_enum(out, e, scope)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => {
                emit_bitmask(out, b, scope)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                emit_bitset(out, b, scope)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                emit_union(out, u, scope)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Forward(_)))) => {
                // Forward declarations need no code in Python.
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                emit_typedef(out, td, scope)?;
            }
            Definition::Except(e) => {
                emit_exception(out, e, scope)?;
            }
            Definition::Const(c) => {
                emit_const(out, c, scope)?;
            }
            _ => {
                return Err(IdlPythonError::Unsupported(format!(
                    "definition variant not yet supported: {d:?}"
                )));
            }
        }
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 32
fn emit_module(out: &mut String, m: &ModuleDef, scope: &mut Vec<String>) -> Result<()> {
    scope.push(m.name.text.clone());
    emit_definitions(out, &m.definitions, scope)?;
    scope.pop();
    Ok(())
}

/// Maps the struct's `@final`/`@appendable`/`@mutable`/`@extensibility(...)`
/// annotation to the `extensibility=` kwarg the Python runtime understands. The
/// runtime default (when the kwarg is omitted) is `final`; we only emit the
/// kwarg for the non-default cases so the compact `@final` layout stays
/// untouched (XTypes 1.3 §7.4.3.5.3 — DHEADER framing is driven by this).
fn struct_extensibility_kwarg(s: &StructDef) -> Option<&'static str> {
    let lowered = lower_annotations(&s.annotations).ok()?;
    match lowered.extensibility() {
        Some(ExtensibilityKind::Final) => None, // explicit @final → compact, no kwarg
        Some(ExtensibilityKind::Appendable) => Some("appendable"),
        Some(ExtensibilityKind::Mutable) => Some("mutable"),
        None => Some("appendable"), // SX2: unannotated default §7.3.3.1
    }
}

/// Same mapping as [`struct_extensibility_kwarg`] but for a union — drives the
/// union's DHEADER (appendable/mutable) in the Python runtime's `_IdlUnion`.
fn union_extensibility_kwarg(u: &UnionDef) -> Option<&'static str> {
    let lowered = lower_annotations(&u.annotations).ok()?;
    match lowered.extensibility() {
        Some(ExtensibilityKind::Final) => None,
        Some(ExtensibilityKind::Appendable) => Some("appendable"),
        Some(ExtensibilityKind::Mutable) => Some("mutable"),
        None => Some("appendable"), // SX2 default §7.3.3.1
    }
}

fn emit_struct(out: &mut String, s: &StructDef, scope: &[String]) -> Result<()> {
    let typename = qualified_typename(&s.name, scope);
    let class_name = python_class_name(&s.name, scope);

    match struct_extensibility_kwarg(s) {
        Some("mutable") => {
            // A `@mutable` struct needs the per-member id list so the runtime's
            // PL_CDR2 EMHEADERs (XTypes 1.3 §7.4.3.4.2) carry the right ids.
            let ids = mutable_member_ids(s);
            let id_list = ids
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                out,
                "@idl_struct(typename=\"{typename}\", extensibility=\"mutable\", \
                 member_ids=[{id_list}])"
            )
            .ok();
        }
        Some(ext) => {
            writeln!(
                out,
                "@idl_struct(typename=\"{typename}\", extensibility=\"{ext}\")"
            )
            .ok();
        }
        None => {
            writeln!(out, "@idl_struct(typename=\"{typename}\")").ok();
        }
    }
    writeln!(out, "@dataclass").ok();
    if let Some(base) = &s.base {
        let base_class = resolve_scoped_name(base, scope);
        writeln!(out, "class {class_name}({base_class}):").ok();
    } else {
        writeln!(out, "class {class_name}:").ok();
    }

    if s.members.is_empty() && s.base.is_none() {
        writeln!(out, "    pass").ok();
    } else if s.members.is_empty() {
        // With a base but no new fields: `pass` so the dataclass body
        // is syntactically valid.
        writeln!(out, "    pass").ok();
    } else {
        for m in &s.members {
            emit_members_of(out, m, scope)?;
        }
    }
    out.push('\n');
    Ok(())
}

fn emit_exception(out: &mut String, e: &ExceptDecl, scope: &[String]) -> Result<()> {
    let typename = qualified_typename(&e.name, scope);
    let class_name = python_class_name(&e.name, scope);

    writeln!(out, "@idl_struct(typename=\"{typename}\")").ok();
    writeln!(out, "@dataclass").ok();
    writeln!(out, "class {class_name}(Exception):").ok();

    if e.members.is_empty() {
        writeln!(out, "    pass").ok();
    } else {
        for m in &e.members {
            emit_members_of(out, m, scope)?;
        }
    }
    out.push('\n');
    Ok(())
}

fn emit_members_of(out: &mut String, m: &Member, scope: &[String]) -> Result<()> {
    let py_type = python_type_for(&m.type_spec, scope)?;
    let optional = member_is_optional(m);
    for d in &m.declarators {
        let field_name = escape_python_keyword(d.name().text.as_str());
        let mut t = py_type.clone();
        if let Declarator::Array(arr) = d {
            // Fixed-size array: wrap each dimension in the runtime `Array[T, N]`
            // brand (NOT `List[T]`, which the runtime reads as an unbounded
            // sequence with a length prefix). Innermost dimension first so a
            // multi-dim `grid[4][4]` nests as `Array[Array[Int32, 4], 4]`
            // (Bug Q-cluster fixed-array member).
            for size in arr.sizes.iter().rev() {
                let n = eval_const_int(size).ok_or_else(|| {
                    IdlPythonError::Unsupported(format!(
                        "array bound is not a literal integer: {size:?}"
                    ))
                })?;
                t = format!("Array[{t}, {n}]");
            }
        }
        // `@optional T` → `Optional[T]` so the runtime emits the u8 presence
        // flag instead of treating the member as required (Bug R5 / #64).
        if optional {
            t = format!("Optional[{t}]");
        }
        writeln!(out, "    {field_name}: {t}").ok();
    }
    Ok(())
}

fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) -> Result<()> {
    let class_name = python_class_name(&e.name, scope);
    writeln!(out, "class {class_name}(IntEnum):").ok();
    if e.enumerators.is_empty() {
        writeln!(out, "    pass").ok();
    } else {
        for (idx, en) in e.enumerators.iter().enumerate() {
            writeln!(out, "    {} = {idx}", en.name.text).ok();
        }
    }
    // T2: a non-default `@bit_bound` narrows the enum's signed wire holder; the
    // runtime's `_IdlEnum` reads `_idl_bit_bound`. Emitted only when narrow so
    // default (int32) enums keep byte-identical output (assigned after the class
    // body — inside an `IntEnum` body it would become a spurious member).
    let bound = enum_bit_bound(e);
    if bound != 32 {
        writeln!(out, "{class_name}._idl_bit_bound = {bound}").ok();
    }
    out.push('\n');
    Ok(())
}

fn emit_bitmask(out: &mut String, b: &BitmaskDecl, scope: &[String]) -> Result<()> {
    let class_name = python_class_name(&b.name, scope);
    writeln!(out, "class {class_name}(IntFlag):").ok();
    if b.values.is_empty() {
        // An empty bitmask still carries its (default 32) holder width so the
        // runtime serializes the spec-correct number of bytes.
        writeln!(out, "    pass").ok();
    } else {
        for (idx, v) in b.values.iter().enumerate() {
            writeln!(out, "    {} = 1 << {idx}", v.name.text).ok();
        }
    }
    // Record the effective @bit_bound (default 32 → uint32 holder) so the
    // runtime sizes the bitmask holder by bit_bound, NOT by flag count
    // (XTypes 1.3 §7.3.1.2.1.1; cross-vendor reference). Set as a class
    // attribute AFTER the class body — assigning it inside an `IntFlag` body
    // would make the enum metaclass treat `_idl_bit_bound` as a (spurious) flag
    // member; a post-hoc assignment stays a plain class attribute.
    writeln!(
        out,
        "{class_name}._idl_bit_bound = {}",
        bitmask_bit_bound(b)
    )
    .ok();
    out.push('\n');
    Ok(())
}

fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) -> Result<()> {
    // OMG bitset: a container integer with packed sub-fields. We
    // emit an Int64 alias plus a helper class with
    // SHIFT/MASK constants per named bitfield. The `zerodds-py`
    // runtime sees the alias as an Int64 brand and encodes accordingly;
    // the application reads/writes fields via the helper constants:
    //
    //   value = SensorFlags_Bits.encode(sensor_id=42, quality=2)
    //   sensor_id = (value >> SensorFlags_Bits.SENSOR_ID_SHIFT)
    //               & SensorFlags_Bits.SENSOR_ID_MASK
    let class_name = python_class_name(&b.name, scope);
    // The holder is the smallest unsigned integer fitting the SUM of bitfield
    // widths (XTypes 1.3 §7.4.13; cdr-core `bitset_storage_type`). Emit the
    // alias as `Bitset[total_bits]` so the runtime serializes exactly that
    // width — a plain `Int64` alias would put 8 bytes on the wire instead of
    // the spec-correct holder (e.g. u8 for an 8-bit bitset).
    let total_bits: i64 = b
        .bitfields
        .iter()
        .map(|bf| eval_const_int(&bf.spec.width).unwrap_or(0))
        .sum();
    writeln!(out, "{class_name}: TypeAlias = Bitset[{total_bits}]").ok();
    writeln!(out).ok();
    writeln!(out, "class {class_name}_Bits:").ok();
    if b.bitfields.is_empty() {
        writeln!(out, "    pass").ok();
    } else {
        let mut shift: u64 = 0;
        for bf in &b.bitfields {
            let width = eval_const_int(&bf.spec.width).ok_or_else(|| {
                IdlPythonError::Unsupported(format!(
                    "bitset width is not a literal integer: {:?}",
                    bf.spec.width
                ))
            })?;
            if width <= 0 {
                return Err(IdlPythonError::Unsupported(format!(
                    "bitset width must be > 0, got {width}"
                )));
            }
            let width = width as u64;
            let mask = if width >= 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            if let Some(name) = &bf.name {
                let up = name.text.to_uppercase();
                writeln!(out, "    {up}_SHIFT = {shift}").ok();
                writeln!(out, "    {up}_WIDTH = {width}").ok();
                writeln!(out, "    {up}_MASK  = 0x{mask:x}").ok();
            }
            shift += width;
        }
    }
    out.push('\n');
    Ok(())
}

fn emit_union(out: &mut String, u: &UnionDef, scope: &[String]) -> Result<()> {
    let typename = qualified_typename(&u.name, scope);
    let class_name = python_class_name(&u.name, scope);
    let discriminator_repr = switch_type_python_repr(&u.switch_type, scope);

    // Collect cases: one entry per label in dict[int, tuple[str, Any]].
    // For a `default:` label we collect the case separately for the
    // `default=` parameter of the idl_union constructor.
    let mut case_entries: Vec<(i64, String, String)> = Vec::new();
    let mut default_entry: Option<(String, String)> = None;

    for case in &u.cases {
        let field_name = escape_python_keyword(case.element.declarator.name().text.as_str());
        let py_type = python_type_for(&case.element.type_spec, scope)?;
        for label in &case.labels {
            match label {
                CaseLabel::Value(expr) => {
                    let v = eval_const_int(expr).ok_or_else(|| {
                        IdlPythonError::Unsupported(format!(
                            "union case label is not a literal integer (scoped/enum-ref \
                             references will be supported in a follow-up): {expr:?}"
                        ))
                    })?;
                    case_entries.push((v, field_name.clone(), py_type.clone()));
                }
                CaseLabel::Default => {
                    default_entry = Some((field_name.clone(), py_type.clone()));
                }
            }
        }
    }

    writeln!(out, "{class_name} = idl_union(").ok();
    writeln!(out, "    typename=\"{typename}\",").ok();
    writeln!(out, "    discriminator={discriminator_repr},").ok();
    writeln!(out, "    cases={{").ok();
    for (label_value, field, py_type) in &case_entries {
        writeln!(out, "        {label_value}: (\"{field}\", {py_type}),").ok();
    }
    writeln!(out, "    }},").ok();
    if let Some((field, py_type)) = &default_entry {
        writeln!(out, "    default=(\"{field}\", {py_type}),").ok();
    } else {
        writeln!(out, "    default=None,").ok();
    }
    if let Some(ext) = union_extensibility_kwarg(u) {
        writeln!(out, "    extensibility=\"{ext}\",").ok();
    }
    writeln!(out, ")").ok();
    out.push('\n');
    Ok(())
}

fn emit_typedef(out: &mut String, td: &TypedefDecl, scope: &[String]) -> Result<()> {
    let py_type = python_type_for(&td.type_spec, scope)?;
    for d in &td.declarators {
        let alias_name = python_class_name(d.name(), scope);
        match d {
            Declarator::Simple(_) => {
                writeln!(out, "{alias_name}: TypeAlias = {py_type}").ok();
            }
            Declarator::Array(arr) => {
                // A typedef'd fixed array must use the same `Array[T, N]` brand
                // as struct members, so the runtime omits the length prefix.
                let mut t = py_type.clone();
                for size in arr.sizes.iter().rev() {
                    let n = eval_const_int(size).ok_or_else(|| {
                        IdlPythonError::Unsupported(format!(
                            "array bound is not a literal integer: {size:?}"
                        ))
                    })?;
                    t = format!("Array[{t}, {n}]");
                }
                writeln!(out, "{alias_name}: TypeAlias = {t}").ok();
            }
        }
    }
    out.push('\n');
    Ok(())
}

/// Emits an IDL `const` as a module-level Python constant (Bug M / #56).
///
/// `const long MAX = 10;` → `MAX = 10`. Under a module the name is flattened
/// the same way types are (`module conf { const long N = 4; }` → `conf_N = 4`)
/// so references stay consistent with the rest of the generated module.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) -> Result<()> {
    let name = python_class_name(&c.name, scope);
    let value = render_const_value(&c.value, &c.type_, scope)?;
    writeln!(out, "{name} = {value}").ok();
    out.push('\n');
    Ok(())
}

/// Renders a `const_expr` as a Python literal/expression. Integer arithmetic is
/// folded; floats/bools/strings/chars map to their Python literal forms; a
/// scoped reference resolves to an enum value (folded to an int) if known, else
/// to the flattened constant name.
/// zerodds-lint: recursion-depth 32
fn render_const_value(expr: &ConstExpr, ty: &ConstType, scope: &[String]) -> Result<String> {
    // Boolean/string/char/float literals do not survive the integer folder, so
    // handle the literal forms the folder cannot represent first.
    if let ConstExpr::Literal(lit) = expr {
        match lit.kind {
            LiteralKind::Boolean => {
                let v = lit.raw.trim().eq_ignore_ascii_case("true");
                return Ok(if v { "True".into() } else { "False".into() });
            }
            LiteralKind::Floating | LiteralKind::Fixed => {
                return Ok(lit
                    .raw
                    .trim()
                    .trim_end_matches(['f', 'F', 'd', 'D'])
                    .to_string());
            }
            LiteralKind::String | LiteralKind::WideString => {
                // raw already contains the surrounding quotes; emit verbatim.
                return Ok(lit.raw.clone());
            }
            LiteralKind::Char | LiteralKind::WideChar => {
                return Ok(lit.raw.clone());
            }
            LiteralKind::Integer => {}
        }
    }
    // Integer / enum-folded / arithmetic expressions.
    if let Some(n) = eval_const_int(expr) {
        return Ok(n.to_string());
    }
    // Scoped reference that did not fold (non-enum const reference): emit the
    // flattened name so it points at another emitted module constant.
    if let ConstExpr::Scoped(s) = expr {
        return Ok(resolve_scoped_name(s, scope));
    }
    Err(IdlPythonError::Unsupported(format!(
        "const value not representable in python codegen (type {ty:?}): {expr:?}"
    )))
}

// ---------------------------------------------------------------------------
// Type-Mapping
// ---------------------------------------------------------------------------

/// zerodds-lint: recursion-depth 32
fn python_type_for(ts: &TypeSpec, scope: &[String]) -> Result<String> {
    match ts {
        TypeSpec::Primitive(p) => Ok(brand_for_primitive(*p).to_string()),
        TypeSpec::String(s) => {
            // `string<N>` / `wstring<N>` carry a bound that MUST be enforced on
            // the wire (over-bound writes corrupt downstream readers). Emit the
            // runtime `BoundedString[N]` / `BoundedWString[N]` brand so the
            // length is checked; an unbounded string stays the plain brand.
            if let Some(n) = s.bound.as_ref().and_then(eval_const_int) {
                let brand = if s.wide {
                    "BoundedWString"
                } else {
                    "BoundedString"
                };
                Ok(format!("{brand}[{n}]"))
            } else {
                Ok(brand_for_string(s).to_string())
            }
        }
        TypeSpec::Sequence(seq) => {
            let inner = python_type_for_seq_elem(seq, scope)?;
            // `sequence<T, N>` → `Sequence[T, N]` so the element count is
            // enforced; unbounded → `List[T]` (runtime treats it as unbounded).
            if let Some(n) = seq.bound.as_ref().and_then(eval_const_int) {
                Ok(format!("Sequence[{inner}, {n}]"))
            } else {
                Ok(format!("List[{inner}]"))
            }
        }
        TypeSpec::Scoped(name) => Ok(resolve_scoped_name(name, scope)),
        TypeSpec::Map(m) => {
            let k = python_type_for(&m.key, scope)?;
            let v = python_type_for(&m.value, scope)?;
            // `map<K, V, N>` → `Map[K, V, N]` (bounded entry count); unbounded
            // → `Dict[K, V]`.
            if let Some(n) = m.bound.as_ref().and_then(eval_const_int) {
                Ok(format!("Map[{k}, {v}, {n}]"))
            } else {
                Ok(format!("Dict[{k}, {v}]"))
            }
        }
        TypeSpec::Fixed(f) => {
            // fixed<P,S> -> the `Fixed[P,S]` brand; the Python value is the
            // decimal as a `str` (CORBA-BCD on the wire).
            let p = eval_const_int(&f.digits).unwrap_or(0);
            let s = eval_const_int(&f.scale).unwrap_or(0);
            Ok(format!("Fixed[{p}, {s}]"))
        }
        TypeSpec::Any => Err(IdlPythonError::Unsupported(format!(
            "type spec not yet supported in python codegen: {ts:?}"
        ))),
    }
}

/// zerodds-lint: recursion-depth 32
fn python_type_for_seq_elem(seq: &SequenceType, scope: &[String]) -> Result<String> {
    python_type_for(&seq.elem, scope)
}

fn brand_for_primitive(p: PrimitiveType) -> &'static str {
    match p {
        // The runtime exposes the `Bool` brand; emitting the lowercase builtin
        // `bool` would make the codegen try `from zerodds.idl import bool`,
        // which does not exist. `Bool` round-trips identically (both are the
        // 1-byte XCDR2 boolean, §7.4.1.4.1).
        PrimitiveType::Boolean => "Bool",
        PrimitiveType::Octet => "Octet",
        PrimitiveType::Char => "Char",
        PrimitiveType::WideChar => "WChar",
        PrimitiveType::Integer(i) => brand_for_integer(i),
        PrimitiveType::Floating(f) => match f {
            FloatingType::Float => "Float32",
            FloatingType::Double => "Float64",
            FloatingType::LongDouble => "LongDouble",
        },
    }
}

fn brand_for_integer(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "Int16",
        IntegerType::UShort | IntegerType::UInt16 => "UInt16",
        IntegerType::Long | IntegerType::Int32 => "Int32",
        IntegerType::ULong | IntegerType::UInt32 => "UInt32",
        IntegerType::LongLong | IntegerType::Int64 => "Int64",
        IntegerType::ULongLong | IntegerType::UInt64 => "UInt64",
        IntegerType::Int8 => "Int8",
        IntegerType::UInt8 => "UInt8",
    }
}

fn brand_for_string(s: &StringType) -> &'static str {
    if s.wide { "WString" } else { "String" }
}

fn switch_type_python_repr(switch: &SwitchTypeSpec, scope: &[String]) -> String {
    match switch {
        SwitchTypeSpec::Integer(i) => brand_for_integer(*i).to_string(),
        SwitchTypeSpec::Boolean => "bool".to_string(),
        SwitchTypeSpec::Octet => "Octet".to_string(),
        SwitchTypeSpec::Char => "Char".to_string(),
        SwitchTypeSpec::Scoped(s) => resolve_scoped_name(s, scope),
    }
}

fn qualified_typename(name: &Identifier, scope: &[String]) -> String {
    if scope.is_empty() {
        name.text.clone()
    } else {
        format!("{}::{}", scope.join("::"), name.text)
    }
}

fn python_class_name(name: &Identifier, scope: &[String]) -> String {
    if scope.is_empty() {
        name.text.clone()
    } else {
        let mut parts = scope.to_vec();
        parts.push(name.text.clone());
        parts.join("_")
    }
}

fn escape_python_keyword(name: &str) -> String {
    const PYTHON_KEYWORDS: &[&str] = &[
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield", "match", "case",
    ];
    if PYTHON_KEYWORDS.contains(&name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

// ---------------------------------------------------------------------------
// Const-expression evaluation (limited: integer literals + unary +/- only)
// ---------------------------------------------------------------------------

/// zerodds-lint: recursion-depth 32
fn eval_const_int(expr: &ConstExpr) -> Option<i64> {
    match expr {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => parse_integer_literal(raw),
        ConstExpr::Unary { op, operand, .. } => {
            let inner = eval_const_int(operand)?;
            match op {
                UnaryOp::Plus => Some(inner),
                UnaryOp::Minus => Some(-inner),
                UnaryOp::BitNot => Some(!inner),
            }
        }
        // An enum literal as a union case label (`case K_A:`) resolves to the
        // discriminant value `emit_enum` assigns (Bug I).
        ConstExpr::Scoped(s) => {
            let name = s.parts.last()?.text.clone();
            ENUM_VALUES
                .with(|m| m.borrow().get(&name).copied())
                .or_else(|| CONST_VALUES.with(|m| m.borrow().get(&name).copied()))
        }
        // Fold integer arithmetic so `const long N = (A << 2) | 1;` works.
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = eval_const_int(lhs)?;
            let r = eval_const_int(rhs)?;
            match op {
                BinaryOp::Or => Some(l | r),
                BinaryOp::Xor => Some(l ^ r),
                BinaryOp::And => Some(l & r),
                BinaryOp::Shl => Some(l.checked_shl(u32::try_from(r).ok()?)?),
                BinaryOp::Shr => Some(l.checked_shr(u32::try_from(r).ok()?)?),
                BinaryOp::Add => l.checked_add(r),
                BinaryOp::Sub => l.checked_sub(r),
                BinaryOp::Mul => l.checked_mul(r),
                BinaryOp::Div => l.checked_div(r),
                BinaryOp::Mod => l.checked_rem(r),
            }
        }
        _ => None,
    }
}

fn parse_integer_literal(raw: &str) -> Option<i64> {
    let s = raw.trim();
    // OMG IDL allows hex (0x...), octal (0...) and decimal literals.
    // Suffixes like `L`/`u` we ignore generously — the parser
    // has already validated that this is an integer literal.
    let s = s.trim_end_matches(['L', 'l', 'u', 'U']);
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()
    } else if let Some(rest) = s.strip_prefix("-0x").or_else(|| s.strip_prefix("-0X")) {
        i64::from_str_radix(rest, 16).ok().map(|n| -n)
    } else if s.starts_with('0') && s.len() > 1 && !s.contains(|c: char| !c.is_ascii_digit()) {
        // Octal — intentionally strict, because OMG IDL accepts octal.
        i64::from_str_radix(&s[1..], 8).ok()
    } else {
        s.parse::<i64>().ok()
    }
}
