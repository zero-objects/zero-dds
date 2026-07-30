// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR2 TypeSupport code generation for IDL structs.
//!
//! For each `struct` this walker emits a singleton class
//! `<Name>TypeSupport` that implements `org.zerodds.cdr.TopicTypeSupport<T>`.
//! The class provides the §3 surface API
//! (`getTypeName`, `isKeyed`, `getExtensibility`, `encode/decode`,
//! `keyHash`).
//!
//! Spec anchor: zerodds-xcdr2-java-1.0 §3 + §4 + §5 + §6 + §7.
//!
//! Coverage:
//! - Primitive members: boolean/octet/char/wchar/short/long/long long/
//!   float/double + unsigned variants.
//! - String members (UTF-8 length+1+NUL).
//! - Sequence<T> members (uint32 count + element loop).
//! - Nested struct (delegates to `<Type>TypeSupport.INSTANCE`).
//! - Extensibility: @final/@appendable/@mutable.
//! - Annotations: @key, @id(N), @optional.
//!
//! Intentionally out of scope (grows in v1.x):
//! - Map<K,V> members.
//! - fixed/any/wstring (wstring is supported in Writer/Reader; codegen
//!   grows on demand).
//! - Bitset/Bitmask.
//! - Union members (unions are emitted separately as sealed interfaces
//!   by the codegen; TypeSupport for unions is v1.1).

use std::collections::HashMap;
use std::fmt::Write;

use zerodds_idl::ast::{
    CaseLabel, ConstrTypeDecl, Declarator, Definition, FloatingType, IntegerType, Member,
    PrimitiveType, Specification, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec,
    UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind as IdlExtensibility,
};

use crate::JavaGenOptions;
use crate::annotations::lower_or_empty;
use crate::emitter::{JavaFile, fmt_err, indent_unit, wrap_compilation_unit_default};
use crate::error::JavaGenError;
use crate::keywords::sanitize_identifier;
use crate::type_map::primitive_to_java;

/// What a scoped (named) type resolves to. Built once from the whole AST so the
/// encode/decode emitters can decide how a member is framed (enum→int32,
/// typedef→its underlying codec, nested struct/union→delegate to its
/// TypeSupport with its own DHEADER).
#[derive(Debug, Clone)]
enum ResolvedKind {
    /// IDL `enum` — wire form is a SIGNED ordinal whose width (1/2/4 bytes) is
    /// selected by `@bit_bound` (XTypes 1.3 §7.4.5.1 / §7.3.1.2.1.2): N≤8 → 1,
    /// N≤16 → 2, else 4. Cyclone honours this; a fixed int32 broke interop.
    Enum { holder_bytes: u8 },
    /// IDL `typedef T name;` (possibly array). Carries the underlying type and
    /// any array dimensions from the typedef's own declarator.
    Typedef {
        underlying: TypeSpec,
        array_dims: Vec<String>,
    },
    /// IDL `struct` — nested aggregate; delegate to `<Name>TypeSupport`.
    /// Carries the struct's extensibility so a `@mutable` member that embeds
    /// this struct can decide its EMHEADER length-code: an `@appendable` /
    /// `@mutable` nested struct self-delimits with a leading DHEADER (→ LC5
    /// reuses it as the NEXTINT), whereas a `@final` nested struct does not (→
    /// universal LC4). FINDING T1 (nested @mutable). `def` is the struct's
    /// own definition (members + annotations), needed by the KeyHash-specific
    /// walker (`emit_key_field_encode`) to expand a nested `@key` struct
    /// member into that struct's own `@key` subset instead of delegating to
    /// the general `encode` (which — correctly, for normal encoding — writes
    /// ALL of the nested struct's members).
    Struct {
        ext: IdlExtensibility,
        def: StructDef,
    },
    /// IDL `union` — delegate to `<Name>TypeSupport`.
    Union,
    /// IDL `bitmask` — wire form is a single holder integer (NO DHEADER, NOT a
    /// delegated TypeSupport). `holder_bytes` ∈ {1,2,4,8} sized from the effective
    /// `@bit_bound` (default 32 → 4 bytes, XTypes 1.3 §7.3.1.2.1.1), matching
    /// `zerodds-idl-rust`'s `Perm(uN)` holder.
    Bitmask { holder_bytes: u8 },
    /// IDL `bitset` — wire form is a single holder integer sized to the total
    /// declared bitfield width (matches the rust `Flags { storage: uN }`).
    Bitset { holder_bytes: u8 },
}

/// Holder width (in bytes) for a bitmask/bitset given its total bit count.
/// Mirrors `zerodds-idl-rust::bitset_emit::bitset_storage_type`:
/// ≤8 → u8, ≤16 → u16, ≤32 → u32, else u64.
fn holder_bytes_for_bits(total_bits: u32) -> u8 {
    match total_bits {
        0..=8 => 1,
        9..=16 => 2,
        17..=32 => 4,
        _ => 8,
    }
}

/// Total declared bitfield width for a `bitset` (sum of every field width,
/// including anonymous padding). Used to size the wire holder integer.
fn bitset_total_width(b: &zerodds_idl::ast::BitsetDecl) -> u32 {
    let mut total: u32 = 0;
    for bf in &b.bitfields {
        if let zerodds_idl::ast::ConstExpr::Literal(l) = &bf.spec.width {
            if matches!(l.kind, zerodds_idl::ast::LiteralKind::Integer) {
                if let Ok(w) = l.raw.parse::<u32>() {
                    total = total.saturating_add(w);
                }
            }
        }
    }
    total
}

/// Resolution table keyed by the *short* (unqualified) type name. The Java
/// backend already collapses scoped references to their short class name
/// (see `scoped_to_short`), so a short-name table is sufficient for the
/// single-module fixtures and matches how the POJO/enum/union files are named.
type TypeTable = HashMap<String, ResolvedKind>;

/// Walks the whole spec and records every named type's kind so member codecs
/// can resolve `TypeSpec::Scoped` references.
fn build_type_table(spec: &Specification) -> TypeTable {
    let mut table = TypeTable::new();
    collect_types(&spec.definitions, &mut table);
    table
}

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn collect_types(defs: &[Definition], table: &mut TypeTable) {
    for d in defs {
        match d {
            Definition::Module(m) => collect_types(&m.definitions, table),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                let ext = lower_or_empty(&s.annotations)
                    .extensibility()
                    .unwrap_or(IdlExtensibility::Appendable); // SX2 §7.3.3.1
                table.insert(
                    s.name.text.clone(),
                    ResolvedKind::Struct {
                        ext,
                        def: s.clone(),
                    },
                );
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                let ebound = crate::bitset::extract_int_annotation(&e.annotations, "bit_bound")
                    .filter(|&v| (1..=32).contains(&v))
                    .unwrap_or(32);
                let holder_bytes: u8 = if ebound <= 8 {
                    1
                } else if ebound <= 16 {
                    2
                } else {
                    4
                };
                table.insert(e.name.text.clone(), ResolvedKind::Enum { holder_bytes });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                table.insert(u.name.text.clone(), ResolvedKind::Union);
            }
            // Bitmask holder width = effective `@bit_bound` (XTypes 1.3
            // §7.3.1.2.1.1: DEFAULT bit_bound is 32, NOT the count of declared
            // bit-values), mirroring `zerodds-idl-rust::annotations::bitmask_bit_bound`
            // + `bitset_storage_type`. An unannotated `bitmask Perm { READ, WRITE,
            // EXEC }` is a uint32 holder (4 bytes) on the wire — was Bug XV-bits,
            // which sized it from `values.len()` → 1 byte and diverged from the
            // cross-vendor golden.
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => {
                let bit_bound = crate::bitset::extract_int_annotation(&b.annotations, "bit_bound")
                    .unwrap_or(crate::bitset::DEFAULT_BIT_BOUND);
                let holder_bytes = holder_bytes_for_bits(bit_bound);
                table.insert(b.name.text.clone(), ResolvedKind::Bitmask { holder_bytes });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                let holder_bytes = holder_bytes_for_bits(bitset_total_width(b));
                table.insert(b.name.text.clone(), ResolvedKind::Bitset { holder_bytes });
            }
            Definition::Type(TypeDecl::Typedef(t)) => {
                for decl in &t.declarators {
                    let dims = match decl {
                        Declarator::Array(a) => a
                            .sizes
                            .iter()
                            .map(crate::emitter::const_expr_to_java)
                            .collect(),
                        Declarator::Simple(_) => Vec::new(),
                    };
                    table.insert(
                        decl.name().text.clone(),
                        ResolvedKind::Typedef {
                            underlying: t.type_spec.clone(),
                            array_dims: dims,
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

/// Resolves a scoped reference to its kind, following typedef alias-chains so
/// that e.g. `ChargeCurrentType -> CurrentInAmpsType -> double` lands on the
/// underlying primitive.
fn resolve_scoped<'a>(table: &'a TypeTable, name: &str) -> Option<&'a ResolvedKind> {
    table.get(name)
}

/// Emits one TypeSupport file per top-level struct (and standalone union).
pub(crate) fn emit_typesupport_files(
    spec: &Specification,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    let mut files = Vec::new();
    let pkg = sanitize_package(&opts.root_package);
    let table = build_type_table(spec);
    // #24 / F-TYPES-3: full-spec resolved NameMap for the emitted COMPLETE
    // TypeObject byte constants (`complete_struct_type_object_bytes`). Degrades
    // to an empty map on a build failure — affected structs then emit an empty
    // `typeObject()` (no cross-binding identifier for that type).
    let names = zerodds_idl::semantics::build_type_registry(spec)
        .map(|lowered| lowered.names)
        .unwrap_or_default();
    walk_defs(
        &spec.definitions,
        &pkg,
        &[],
        opts,
        &table,
        &names,
        &mut files,
    )?;
    Ok(files)
}

fn sanitize_package(p: &str) -> String {
    p.trim_matches('.').to_string()
}

/// Shared error for `long double` (IEEE-754 binary128, 16 bytes on the
/// XCDR wire). Java lacks a binary128 primitive and the CDR runtime has
/// no 16-byte float accessor, so any codegen would be wire-incorrect;
/// generation is refused rather than emitting a silently-wrong 8-byte
/// member (see P12).
pub(crate) fn long_double_unsupported() -> JavaGenError {
    JavaGenError::UnsupportedConstruct {
        construct: "long double (IEEE-754 binary128 / 16-byte float)".into(),
        context: Some("no binary128 primitive in Java; awaiting f128-backed wire".into()),
    }
}

/// Emits a Java array allocation `TYPE[] var = new TYPE[dims];`.
///
/// When the element type is generic (e.g. `java.util.List<Integer>` for an
/// `array<sequence<long>>` member), `new java.util.List<Integer>[N]` is a
/// compile error (JLS §15.10.1: generic array creation). We allocate the
/// erased raw type instead and cast back, silencing the unavoidable
/// unchecked-cast warning on the local declaration.
fn emit_array_alloc(
    out: &mut String,
    ind: &str,
    elem_jt: &str,
    empty_brackets: &str,
    brackets: &str,
    var: &str,
) -> Result<(), JavaGenError> {
    if let Some(lt) = elem_jt.find('<') {
        let raw = &elem_jt[..lt];
        writeln!(out, "{ind}@SuppressWarnings(\"unchecked\")").map_err(fmt_err)?;
        writeln!(
            out,
            "{ind}{elem_jt}{empty_brackets} {var} = ({elem_jt}{empty_brackets}) new {raw}{brackets};"
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            out,
            "{ind}{elem_jt}{empty_brackets} {var} = new {elem_jt}{brackets};"
        )
        .map_err(fmt_err)?;
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (IDL module depth-bounded)
fn walk_defs(
    defs: &[Definition],
    pkg: &str,
    module_chain: &[String],
    opts: &JavaGenOptions,
    table: &TypeTable,
    names: &zerodds_idl::semantics::NameMap,
    files: &mut Vec<JavaFile>,
) -> Result<(), JavaGenError> {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let name = sanitize_identifier(&m.name.text)?;
                let lower = name.to_lowercase();
                let sub_pkg = if pkg.is_empty() {
                    lower
                } else {
                    format!("{pkg}.{lower}")
                };
                let mut chain = module_chain.to_vec();
                chain.push(m.name.text.clone());
                walk_defs(&m.definitions, &sub_pkg, &chain, opts, table, names, files)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                let _lowered = lower_or_empty(&s.annotations);
                // FINDING T1: a `@nested` struct is NOT a Topic-Type, but it can
                // still be embedded as a member of another struct (e.g. `@nested
                // @final NestedKey` inside `OuterKey`), whose encode/decode
                // delegates to `NestedKeyTypeSupport.INSTANCE`. The TypeSupport
                // file is a pure codec (encode/decode/getTypeName/isKeyed, no
                // global topic registration), so emitting it for `@nested` types
                // is both harmless and REQUIRED — skipping it left the embedding
                // struct referencing a non-existent class (compile error).
                if !struct_is_typesupport_eligible(s, table) {
                    // Member types still outside the current TypeSupport scope
                    // (`any`, `fixed`) — the codegen silently skips the file.
                    continue;
                }
                let file = emit_typesupport_for_struct(s, pkg, module_chain, opts, table, names)?;
                files.push(file);
            }
            // Bug J #65(5): standalone unions need their own TypeSupport too,
            // otherwise structs that embed them reference a non-existent
            // `<Name>TypeSupport`.
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                // FINDING T1: like @nested structs above, a @nested union can
                // still be a member of another aggregate that delegates to its
                // TypeSupport codec — emit the file regardless of the @nested
                // (topic-type-only) marker.
                if !union_is_typesupport_eligible(u, table) {
                    continue;
                }
                let file = emit_typesupport_for_union(u, pkg, module_chain, opts, table)?;
                files.push(file);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Returns `true` if all member type specs are covered by the TypeSupport
/// codegen. Map, array, nested-struct, enum, typedef, union and nested-sequence
/// members are all supported now (Bug J #65). Only `any` and `fixed` remain out
/// of scope.
fn struct_is_typesupport_eligible(s: &StructDef, table: &TypeTable) -> bool {
    s.members
        .iter()
        .all(|m| typespec_supported(&m.type_spec, table))
}

fn union_is_typesupport_eligible(u: &UnionDef, table: &TypeTable) -> bool {
    u.cases
        .iter()
        .all(|c| typespec_supported(&c.element.type_spec, table))
}

/// zerodds-lint: recursion-depth 32 (TypeSpec recursive via Sequence/Map)
fn typespec_supported(ts: &TypeSpec, table: &TypeTable) -> bool {
    match ts {
        TypeSpec::Primitive(_) | TypeSpec::String(_) => true,
        TypeSpec::Scoped(sn) => {
            // Follow typedef aliases to their underlying type before deciding.
            match resolve_scoped(table, &scoped_to_short(sn)) {
                Some(ResolvedKind::Typedef { underlying, .. }) => {
                    typespec_supported(underlying, table)
                }
                // Enum / Struct / Union, or an unknown external ref — assume a
                // companion TypeSupport / int32 codec exists.
                _ => true,
            }
        }
        TypeSpec::Sequence(seq) => typespec_supported(&seq.elem, table),
        TypeSpec::Map(map) => {
            typespec_supported(&map.key, table) && typespec_supported(&map.value, table)
        }
        // fixed<P,S> now has a wire codec (CORBA-BCD via writeFixedBcd/
        // readFixedBcd); `any` still has none.
        TypeSpec::Fixed(_) => true,
        TypeSpec::Any => false,
    }
}

/// F-TYPES-3 / #24: emits the `typeObject()` override carrying the COMPLETE
/// `TypeObject` serialized (XCDR-LE) by the SHARED
/// `zerodds_idl::semantics::complete_struct_type_object_bytes` — the SAME source
/// `idl-rust`, `idl-cpp` and `idl-csharp` use, so all bindings emit
/// byte-identical bytes (and thus the identical `TypeIdentifier`, derived by the
/// interface's default `typeIdentifier()` as MD5-14 of these bytes).
///
/// A struct whose members cannot all be resolved emits no override (the
/// interface default returns an empty `typeObject()`) — never a codegen error.
fn emit_type_object_method(
    body: &mut String,
    s: &StructDef,
    module_chain: &[String],
    ind: &str,
    names: &zerodds_idl::semantics::NameMap,
) -> Result<(), JavaGenError> {
    let bytes = zerodds_idl::semantics::complete_struct_type_object_bytes(s, module_chain, names)
        .ok()
        .filter(|b| !b.is_empty());
    if let Some(bytes) = bytes {
        // `signed byte` in Java: values > 0x7f wrap negative, so cast each to
        // `(byte)`. Byte-identity is preserved (the JVM stores the same bit
        // pattern); the parity test reads the `0xNN` hex tokens directly.
        writeln!(body, "{ind}@Override").map_err(fmt_err)?;
        writeln!(
            body,
            "{ind}public byte[] typeObject() {{ return TYPE_OBJECT.clone(); }}"
        )
        .map_err(fmt_err)?;
        write!(
            body,
            "{ind}private static final byte[] TYPE_OBJECT = new byte[] {{"
        )
        .map_err(fmt_err)?;
        for (i, b) in bytes.iter().enumerate() {
            if i % 12 == 0 {
                write!(body, "\n{ind}{ind}").map_err(fmt_err)?;
            }
            write!(body, "(byte) 0x{b:02x}, ").map_err(fmt_err)?;
        }
        writeln!(body, "\n{ind}}};").map_err(fmt_err)?;
        writeln!(body).map_err(fmt_err)?;
    }
    Ok(())
}

fn emit_typesupport_for_struct(
    s: &StructDef,
    pkg: &str,
    module_chain: &[String],
    opts: &JavaGenOptions,
    table: &TypeTable,
    names: &zerodds_idl::semantics::NameMap,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&s.name.text)?;
    let support_class = format!("{class}TypeSupport");
    let ind = indent_unit(opts);
    let lowered = lower_or_empty(&s.annotations);
    // XTypes 1.3 §7.3.1.2.1.1 (and the idl-rust default, annotations.rs:49): an
    // aggregate with NO explicit @final/@appendable/@mutable is FINAL. Defaulting
    // to APPENDABLE here was Bug XW — it wrapped @final nested structs (e.g.
    // `combo::Sample` inside `sequence<Sample>`) in a spurious per-element DHEADER
    // (rule (30)), diverging from the cdr-core / rust canonical golden.
    let extensibility = lowered
        .extensibility()
        .unwrap_or(IdlExtensibility::Appendable); // SX2 §7.3.3.1

    let type_name = build_type_name(module_chain, &class);
    let is_keyed = resolved_wire_members(s, table).iter().any(member_has_key);

    let mut body = String::new();
    writeln!(
        body,
        "/** XCDR2 TypeSupport for {class}. Generated by zerodds idl-java. */"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "public final class {support_class} implements org.zerodds.cdr.TopicTypeSupport<{class}> {{"
    )
    .map_err(fmt_err)?;

    writeln!(
        body,
        "{ind}public static final {support_class} INSTANCE = new {support_class}();",
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    writeln!(body, "{ind}private {support_class}() {{}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // getTypeName
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public String getTypeName() {{ return \"{type_name}\"; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // isKeyed
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public boolean isKeyed() {{ return {is_keyed}; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // getExtensibility
    let ext_lit = match extensibility {
        IdlExtensibility::Final => "FINAL",
        IdlExtensibility::Appendable => "APPENDABLE",
        IdlExtensibility::Mutable => "MUTABLE",
    };
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public org.zerodds.cdr.ExtensibilityKind getExtensibility() {{ \
         return org.zerodds.cdr.ExtensibilityKind.{ext_lit}; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // F-TYPES-3 / #24: serialized COMPLETE TypeObject + the derived cross-binding
    // TypeIdentifier (via the interface's default typeIdentifier()).
    emit_type_object_method(&mut body, s, module_chain, &ind, names)?;

    // encode(T)
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public byte[] encode({class} sample) {{ return encode(sample, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN); }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // encode(T, EndianMode) — XCDR2 default; delegates to the representation form.
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public byte[] encode({class} sample, org.zerodds.cdr.EndianMode endian) {{ return encode(sample, endian, 1); }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // encode(T, EndianMode, representation): 1 = XCDR2 (alignment cap 4), 0 =
    // XCDR1 / classic CDR (cap 8, no DHEADER on @final/@appendable, PL_CDR1
    // for @mutable). Symmetric with the representation-aware decode overload.
    writeln!(
        body,
        "{ind}public byte[] encode({class} sample, org.zerodds.cdr.EndianMode endian, int representation) {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}int __maxAlign = representation == 0 ? org.zerodds.cdr.Xcdr2Writer.XCDR1_MAX_ALIGN_VALUE : 4;"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}org.zerodds.cdr.Xcdr2Writer w = new org.zerodds.cdr.Xcdr2Writer(endian, __maxAlign);"
    )
    .map_err(fmt_err)?;
    emit_encode_body(&mut body, s, &ind, extensibility, table)?;
    writeln!(body, "{ind}{ind}return w.toByteArray();").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // decode(byte[])
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes) {{ return decode(bytes, 0, bytes.length); }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // decode(byte[], offset, length) — little-endian default delegator.
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes, int offset, int length) {{ return decode(bytes, offset, length, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN); }}"
    )
    .map_err(fmt_err)?;
    // decode(byte[], offset, length, EndianMode) — big-endian payloads (the byte
    // order the DCPS layer reads from the encapsulation header). Delegates to the
    // representation-aware overload with XCDR2.
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes, int offset, int length, org.zerodds.cdr.EndianMode endian) {{ return decode(bytes, offset, length, endian, 1); }}"
    )
    .map_err(fmt_err)?;
    // decode(.., representation): 1 = XCDR2 (alignment cap 4), 0 = XCDR1 / classic
    // CDR (cap 8, no DHEADER, PL_CDR1 @mutable).
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes, int offset, int length, org.zerodds.cdr.EndianMode endian, int representation) {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}int __maxAlign = representation == 0 ? org.zerodds.cdr.Xcdr2Reader.XCDR1_MAX_ALIGN_VALUE : 4;"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}org.zerodds.cdr.Xcdr2Reader r = new org.zerodds.cdr.Xcdr2Reader(bytes, offset, length, endian, __maxAlign);"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}return decodeFrom(r);").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // decodeFrom(Xcdr2Reader): Bug XW — share the parent reader's cursor so a
    // nested aggregate (whatever its extensibility) reads exactly its own bytes
    // in-place. @final types carry NO DHEADER (XTypes 1.3 §7.4.3.5.3 rule (17)/
    // (26)), so a frame-slice on a length prefix that does not exist would
    // desync; reading directly from the shared cursor is extensibility-correct
    // (the decode body reads its own DHEADER for @appendable/@mutable).
    writeln!(
        body,
        "{ind}public static {class} decodeFrom(org.zerodds.cdr.Xcdr2Reader r) {{"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}{class} v = new {class}();").map_err(fmt_err)?;
    emit_decode_body(&mut body, s, &ind, extensibility, table)?;
    writeln!(body, "{ind}{ind}return v;").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // keyHash
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(body, "{ind}public byte[] keyHash({class} sample) {{").map_err(fmt_err)?;
    if is_keyed {
        // KeyHash uses big-endian PLAIN_CDR2 (XTypes 1.3 §7.6.8.4). Bind a local
        // `endian` so an embedded nested-struct/union key member — whose encode
        // delegates to `<Type>TypeSupport.INSTANCE.encode(expr, endian)` — picks
        // up the big-endian holder writer (it referenced a free `endian` that
        // only exists in the encode overload, a latent compile error first hit
        // by a keyed struct with a nested-struct key member, e.g. OuterKey).
        writeln!(
            body,
            "{ind}{ind}org.zerodds.cdr.EndianMode endian = org.zerodds.cdr.EndianMode.BIG_ENDIAN;"
        )
        .map_err(fmt_err)?;
        writeln!(
            body,
            "{ind}{ind}org.zerodds.cdr.Xcdr2Writer w = new org.zerodds.cdr.Xcdr2Writer(endian);"
        )
        .map_err(fmt_err)?;
        emit_key_extraction(&mut body, s, &ind, table)?;
        // XTypes 1.3 §7.6.8.4: holder ≤ 16 octets -> zero-pad; otherwise MD5.
        writeln!(body, "{ind}{ind}byte[] holder = w.toByteArray();").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}if (holder.length <= 16) {{").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}{ind}byte[] h = new byte[16];").map_err(fmt_err)?;
        writeln!(
            body,
            "{ind}{ind}{ind}System.arraycopy(holder, 0, h, 0, holder.length);"
        )
        .map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}{ind}return h;").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}}}").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}return org.zerodds.cdr.Md5.hash(holder);").map_err(fmt_err)?;
    } else {
        writeln!(body, "{ind}{ind}return new byte[16];").map_err(fmt_err)?;
    }
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    emit_codec_helpers(&mut body, &ind)?;

    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: support_class,
        source,
    })
}

fn build_type_name(module_chain: &[String], class: &str) -> String {
    if module_chain.is_empty() {
        class.to_string()
    } else {
        let mut parts: Vec<String> = module_chain.to_vec();
        parts.push(class.to_string());
        parts.join("::")
    }
}

// ---------------------------------------------------------------------------
// Union TypeSupport (Bug J #65(5))
// ---------------------------------------------------------------------------

/// One resolved case branch: the Java case-record class name (`asLong`→`AsLong`),
/// its single field name + type, the discriminator labels, and whether it is the
/// `default:` branch.
struct UnionBranch<'a> {
    record: String,
    field: String,
    type_spec: &'a TypeSpec,
    labels: Vec<String>,
    is_default: bool,
}

/// Emits a `<Name>TypeSupport` for a top-level union. The wire form is an
/// appendable-delimited `[DHEADER][discriminator][selected member]` (DDS-XTypes
/// §7.4.3); the DHEADER makes the union self-delimiting so a struct member of
/// this union type can be decoded via `readDelimitedFrame`.
fn emit_typesupport_for_union(
    u: &UnionDef,
    pkg: &str,
    module_chain: &[String],
    opts: &JavaGenOptions,
    table: &TypeTable,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&u.name.text)?;
    let support_class = format!("{class}TypeSupport");
    let ind = indent_unit(opts);
    let type_name = build_type_name(module_chain, &class);

    // Bug XW: honor the union's actual extensibility. XTypes 1.3 §7.3.1.2.1.1 +
    // idl-rust default (annotations.rs:49): an unannotated union is FINAL, whose
    // XCDR2 wire form is `[disc][selected member]` with NO DHEADER (rule (26)
    // FUNION_TYPE). Only @appendable/@mutable unions are DHEADER-delimited
    // (rule (21)). The previous hardcoded APPENDABLE DHEADER was the divergence
    // (e.g. `combo::Reading` is @final).
    let u_lowered = lower_or_empty(&u.annotations);
    let extensibility = u_lowered
        .extensibility()
        .unwrap_or(IdlExtensibility::Appendable); // SX2 §7.3.3.1
    let delimited = !matches!(extensibility, IdlExtensibility::Final);
    let ext_lit = match extensibility {
        IdlExtensibility::Final => "FINAL",
        IdlExtensibility::Appendable => "APPENDABLE",
        IdlExtensibility::Mutable => "MUTABLE",
    };

    // Build the resolved branch list (dedup by record name, mirroring the
    // data-class emitter).
    let mut branches: Vec<UnionBranch> = Vec::new();
    for c in &u.cases {
        let field = sanitize_identifier(&c.element.declarator.name().text)?;
        let record = capitalize(&field);
        if branches.iter().any(|b| b.record == record) {
            continue;
        }
        let mut labels = Vec::new();
        let mut is_default = false;
        for label in &c.labels {
            match label {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => labels.push(switch_label_to_java(&u.switch_type, expr)),
            }
        }
        branches.push(UnionBranch {
            record,
            field,
            type_spec: &c.element.type_spec,
            labels,
            is_default,
        });
    }

    let mut body = String::new();
    writeln!(
        body,
        "/** XCDR2 TypeSupport for union {class}. Generated by zerodds idl-java. */"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "public final class {support_class} implements org.zerodds.cdr.TopicTypeSupport<{class}> {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public static final {support_class} INSTANCE = new {support_class}();"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}private {support_class}() {{}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public String getTypeName() {{ return \"{type_name}\"; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(body, "{ind}public boolean isKeyed() {{ return false; }}").map_err(fmt_err)?;
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public org.zerodds.cdr.ExtensibilityKind getExtensibility() {{ return org.zerodds.cdr.ExtensibilityKind.{ext_lit}; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // encode(T)
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public byte[] encode({class} sample) {{ return encode(sample, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN); }}"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public byte[] encode({class} sample, org.zerodds.cdr.EndianMode endian) {{ return encode(sample, endian, 1); }}"
    )
    .map_err(fmt_err)?;
    // representation-aware: 1 = XCDR2, 0 = XCDR1 / classic CDR (max-align 8, no
    // DHEADER on @appendable union). A union has no PL_CDR1 form — the
    // beginAppendable no-op under XCDR1 covers the @appendable case.
    writeln!(
        body,
        "{ind}public byte[] encode({class} sample, org.zerodds.cdr.EndianMode endian, int representation) {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}int __maxAlign = representation == 0 ? org.zerodds.cdr.Xcdr2Writer.XCDR1_MAX_ALIGN_VALUE : 4;"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}org.zerodds.cdr.Xcdr2Writer w = new org.zerodds.cdr.Xcdr2Writer(endian, __maxAlign);"
    )
    .map_err(fmt_err)?;
    if delimited {
        writeln!(body, "{ind}{ind}int __dh = w.beginAppendable();").map_err(fmt_err)?;
    }
    let default_disc = union_default_discriminator(&branches, &u.switch_type);
    let inner = format!("{ind}{ind}");
    let mut first = true;
    for b in &branches {
        let kw = if first { "if" } else { "} else if" };
        first = false;
        writeln!(
            body,
            "{inner}{kw} (sample instanceof {class}.{rec} __cv) {{",
            rec = b.record
        )
        .map_err(fmt_err)?;
        // Discriminator: explicit label for the branch, or the synthesised
        // default value for `default:`.
        let disc = if b.is_default {
            default_disc.clone()
        } else {
            b.labels
                .first()
                .cloned()
                .unwrap_or_else(|| default_disc.clone())
        };
        emit_switch_disc_encode(&mut body, &u.switch_type, &disc, &format!("{inner}    "))?;
        let val = format!("__cv.{}()", b.field);
        emit_typespec_encode(
            &mut body,
            b.type_spec,
            &val,
            &format!("{inner}    "),
            table,
            0,
        )?;
    }
    if !branches.is_empty() {
        writeln!(body, "{inner}}}").map_err(fmt_err)?;
    }
    if delimited {
        writeln!(body, "{ind}{ind}w.endDelimited(__dh);").map_err(fmt_err)?;
    }
    writeln!(body, "{ind}{ind}return w.toByteArray();").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // decode
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes) {{ return decode(bytes, 0, bytes.length); }}"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes, int offset, int length) {{ return decode(bytes, offset, length, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN); }}"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes, int offset, int length, org.zerodds.cdr.EndianMode endian) {{ return decode(bytes, offset, length, endian, 1); }}"
    )
    .map_err(fmt_err)?;
    // representation-aware: 0 = XCDR1 / classic CDR (reader max-align 8, no
    // DHEADER on @appendable union).
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes, int offset, int length, org.zerodds.cdr.EndianMode endian, int representation) {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}int __maxAlign = representation == 0 ? org.zerodds.cdr.Xcdr2Reader.XCDR1_MAX_ALIGN_VALUE : 4;"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}org.zerodds.cdr.Xcdr2Reader r = new org.zerodds.cdr.Xcdr2Reader(bytes, offset, length, endian, __maxAlign);"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}return decodeFrom(r);").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // decodeFrom(Xcdr2Reader): Bug XW — shared-cursor decode for a nested union.
    // A @final union is `[disc][member]` with NO DHEADER (rule (26) FUNION_TYPE);
    // only @appendable/@mutable carries one (rule (21)).
    writeln!(
        body,
        "{ind}public static {class} decodeFrom(org.zerodds.cdr.Xcdr2Reader r) {{"
    )
    .map_err(fmt_err)?;
    if delimited {
        writeln!(body, "{ind}{ind}r.readDHeader();").map_err(fmt_err)?;
    }
    emit_switch_disc_decode(&mut body, &u.switch_type, "__disc", &inner)?;
    let disc_eq = |lbl: &str| switch_disc_compare(&u.switch_type, "__disc", lbl);
    let mut first = true;
    let mut default_branch: Option<&UnionBranch> = None;
    for b in &branches {
        if b.is_default {
            default_branch = Some(b);
            continue;
        }
        let cond = b
            .labels
            .iter()
            .map(|l| disc_eq(l))
            .collect::<Vec<_>>()
            .join(" || ");
        let kw = if first { "if" } else { "} else if" };
        first = false;
        writeln!(body, "{inner}{kw} ({cond}) {{").map_err(fmt_err)?;
        let jt = java_value_type(b.type_spec, table);
        emit_read_into(
            &mut body,
            b.type_spec,
            "__uv",
            &jt,
            &format!("{inner}    "),
            table,
            0,
        )?;
        writeln!(body, "{inner}    return new {class}.{}(__uv);", b.record).map_err(fmt_err)?;
    }
    // Default / fall-through branch.
    if let Some(b) = default_branch {
        if !first {
            writeln!(body, "{inner}}} else {{").map_err(fmt_err)?;
        }
        let jt = java_value_type(b.type_spec, table);
        emit_read_into(
            &mut body,
            b.type_spec,
            "__uv",
            &jt,
            &format!("{inner}    "),
            table,
            0,
        )?;
        writeln!(body, "{inner}    return new {class}.{}(__uv);", b.record).map_err(fmt_err)?;
        if !first {
            writeln!(body, "{inner}}}").map_err(fmt_err)?;
        }
    } else {
        if !first {
            writeln!(body, "{inner}}}").map_err(fmt_err)?;
        }
        writeln!(
            body,
            "{inner}throw new org.zerodds.cdr.XcdrException(\"union {class}: unknown discriminator\");"
        )
        .map_err(fmt_err)?;
    }
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // keyHash (unions are not keyed here)
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public byte[] keyHash({class} sample) {{ return new byte[16]; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    emit_codec_helpers(&mut body, &ind)?;
    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: support_class,
        source,
    })
}

/// Renders a case-label const expr as a Java literal appropriate to the switch
/// type. Enum discriminators evaluate the label to its `<Enum>.<CONST>` form.
fn switch_label_to_java(switch: &SwitchTypeSpec, expr: &zerodds_idl::ast::ConstExpr) -> String {
    match switch {
        SwitchTypeSpec::Scoped(sn) => {
            // Enum label: `K_A` → `Kind.K_A`. The const expr is the scoped name
            // of the enumerator; take its last segment.
            let lbl = crate::emitter::const_expr_to_java(expr);
            let short = scoped_to_short(sn);
            let leaf = lbl.rsplit("::").next().unwrap_or(&lbl);
            let leaf = leaf.rsplit('.').next().unwrap_or(leaf);
            format!("{short}.{leaf}")
        }
        _ => crate::emitter::const_expr_to_java(expr),
    }
}

/// A discriminator value to write for the `default:` branch — one that does not
/// collide with any explicit label. For integral/char switches we pick
/// `max(labels)+1`; for enums there is no spare ordinal in general, so we reuse
/// the first declared label is unsafe — instead we encode the ordinal directly
/// as an int via a sentinel beyond the named set.
fn union_default_discriminator(branches: &[UnionBranch], switch: &SwitchTypeSpec) -> String {
    match switch {
        SwitchTypeSpec::Boolean => "false".to_string(),
        SwitchTypeSpec::Scoped(_) => {
            // Enum discriminator default: encode a sentinel ordinal not used by
            // any case (Integer.MIN_VALUE) so decode falls through to default.
            "__ENUM_DEFAULT__".to_string()
        }
        _ => {
            // Integral / char: max explicit label + 1 (numeric).
            let mut max: i64 = -1;
            for b in branches {
                for l in &b.labels {
                    if let Ok(n) = l.trim_end_matches(['L', 'l']).parse::<i64>() {
                        if n > max {
                            max = n;
                        }
                    }
                }
            }
            (max + 1).to_string()
        }
    }
}

fn emit_switch_disc_encode(
    out: &mut String,
    switch: &SwitchTypeSpec,
    disc: &str,
    ind: &str,
) -> Result<(), JavaGenError> {
    match switch {
        SwitchTypeSpec::Boolean => {
            writeln!(out, "{ind}w.writeBoolean({disc});").map_err(fmt_err)?;
        }
        SwitchTypeSpec::Char => {
            writeln!(out, "{ind}w.writeChar((char) ({disc}));").map_err(fmt_err)?;
        }
        SwitchTypeSpec::Octet => {
            writeln!(out, "{ind}w.writeOctet((byte) ({disc}));").map_err(fmt_err)?;
        }
        SwitchTypeSpec::Integer(i) => {
            let m = int_write_method(*i);
            writeln!(out, "{ind}w.{m}({disc});").map_err(fmt_err)?;
        }
        SwitchTypeSpec::Scoped(_) => {
            // Enum discriminator → int32 ordinal. The sentinel default writes
            // Integer.MIN_VALUE; named labels write `<Enum>.<CONST>.value()`.
            if disc == "__ENUM_DEFAULT__" {
                writeln!(out, "{ind}w.writeInt32(Integer.MIN_VALUE);").map_err(fmt_err)?;
            } else {
                writeln!(out, "{ind}w.writeInt32(({disc}).value());").map_err(fmt_err)?;
            }
        }
    }
    Ok(())
}

fn emit_switch_disc_decode(
    out: &mut String,
    switch: &SwitchTypeSpec,
    var: &str,
    ind: &str,
) -> Result<(), JavaGenError> {
    let (ty, read) = match switch {
        SwitchTypeSpec::Boolean => ("boolean", "r.readBoolean()"),
        SwitchTypeSpec::Char => ("char", "r.readChar()"),
        SwitchTypeSpec::Octet => ("int", "(r.readOctet() & 0xFF)"),
        SwitchTypeSpec::Integer(i) => (int_read_type(*i), int_read_expr(*i)),
        SwitchTypeSpec::Scoped(_) => ("int", "r.readInt32()"),
    };
    writeln!(out, "{ind}{ty} {var} = {read};").map_err(fmt_err)?;
    Ok(())
}

/// Java boolean condition comparing the decoded discriminator `var` to a label.
fn switch_disc_compare(switch: &SwitchTypeSpec, var: &str, label: &str) -> String {
    match switch {
        SwitchTypeSpec::Boolean => format!("{var} == ({label})"),
        SwitchTypeSpec::Scoped(_) => format!("{var} == ({label}).value()"),
        _ => format!("{var} == ({label})"),
    }
}

fn int_write_method(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "writeInt16",
        IntegerType::UShort | IntegerType::UInt16 => "writeUInt16",
        IntegerType::Long | IntegerType::Int32 => "writeInt32",
        IntegerType::ULong | IntegerType::UInt32 => "writeUInt32",
        IntegerType::LongLong | IntegerType::Int64 => "writeInt64",
        IntegerType::ULongLong | IntegerType::UInt64 => "writeUInt64",
        IntegerType::Int8 => "writeOctet",
        IntegerType::UInt8 => "writeUInt8",
    }
}

fn int_read_type(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "short",
        IntegerType::UShort | IntegerType::UInt16 => "int",
        IntegerType::Long | IntegerType::Int32 => "int",
        IntegerType::ULong | IntegerType::UInt32 => "long",
        IntegerType::LongLong | IntegerType::Int64 => "long",
        IntegerType::ULongLong | IntegerType::UInt64 => "long",
        IntegerType::Int8 => "byte",
        IntegerType::UInt8 => "int",
    }
}

fn int_read_expr(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "r.readInt16()",
        IntegerType::UShort | IntegerType::UInt16 => "r.readUInt16()",
        IntegerType::Long | IntegerType::Int32 => "r.readInt32()",
        IntegerType::ULong | IntegerType::UInt32 => "r.readUInt32()",
        IntegerType::LongLong | IntegerType::Int64 => "r.readInt64()",
        IntegerType::ULongLong | IntegerType::UInt64 => "r.readUInt64()",
        IntegerType::Int8 => "r.readOctet()",
        IntegerType::UInt8 => "r.readUInt8()",
    }
}

fn member_has_key(m: &Member) -> bool {
    let lowered = lower_or_empty(&m.annotations);
    lowered
        .builtins
        .iter()
        .any(|b| matches!(b, BuiltinAnnotation::Key))
}

fn member_is_optional(m: &Member) -> bool {
    let lowered = lower_or_empty(&m.annotations);
    lowered
        .builtins
        .iter()
        .any(|b| matches!(b, BuiltinAnnotation::Optional))
}

/// P0-3: the annotation-fixed wire id of a member — explicit `@id(N)`,
/// `@hashid`, or a struct-level `@autoid(HASH)` name-hash — via the ONE central
/// resolver. `None` = SEQUENTIAL default (the caller's running counter). Before
/// this the Java backend read only `@id` and gave `@autoid(HASH)` / `@hashid`
/// members a positional id.
fn member_fixed_id(s: &StructDef, m: &Member) -> Option<u32> {
    let autoid_hash = zerodds_idl::semantics::member_id::container_autoid_hash(&s.annotations);
    let name = m.declarators.first().map_or("", |d| d.name().text.as_str());
    zerodds_idl::semantics::member_id::fixed_member_id(autoid_hash, &m.annotations, name)
}

/// Fully-resolved wire member list for a struct: base-class members FIRST
/// (recursive, multi-level A<-B<-C => A.a, B.b, C.c), then the struct's own
/// members. XTypes 1.3 §7.4.3.4.1 places base members before derived members
/// on the wire; the codec (encode/decode/keyHash/isKeyed) must serialize them
/// in that order. The generated Java class inherits base getters/setters
/// (`class Derived extends Base`), so a base member's `sample.get<Name>()` /
/// `v.set<Name>(...)` resolves through inheritance. Base `StructDef`s come from
/// the resolution `table`; a cycle guard bounds pathological inheritance loops.
fn resolved_wire_members(s: &StructDef, table: &TypeTable) -> Vec<Member> {
    let mut chain: Vec<StructDef> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut cur = s.base.clone();
    while let Some(bn) = cur {
        let name = scoped_to_short(&bn);
        if !seen.insert(name.clone()) {
            break;
        }
        match resolve_scoped(table, &name) {
            Some(ResolvedKind::Struct { def, .. }) => {
                cur = def.base.clone();
                chain.push(def.clone());
            }
            _ => break,
        }
    }
    // `chain` is [parent, grandparent, …]; reverse so the oldest ancestor's
    // members lead, then each descendant's, then the struct's own members.
    let mut out: Vec<Member> = Vec::new();
    for def in chain.into_iter().rev() {
        out.extend(def.members.iter().cloned());
    }
    out.extend(s.members.iter().cloned());
    // Broad-audit P0-5 (#2): drop `@non_serialized` members (XTypes 1.3
    // §7.2.4.4.2) from every wire path — encode, decode, keyHash, and the
    // sequential PL/EMHEADER auto-id counter all consume THIS list, so ids
    // compact over the survivors exactly as the TypeObject builder does. The
    // Java field/getter/setter (emitted from the raw `s.members`) stays; on
    // decode the field keeps its Java default, since nothing sets it.
    out.retain(|m| !zerodds_idl::semantics::annotations::member_is_non_serialized(&m.annotations));
    out
}

// ---------------------------------------------------------------------------
// Encode body emitter
// ---------------------------------------------------------------------------

fn emit_encode_body(
    out: &mut String,
    s: &StructDef,
    ind: &str,
    ext: IdlExtensibility,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    let inner = format!("{ind}{ind}");
    match ext {
        IdlExtensibility::Final => {
            for m in &resolved_wire_members(s, table) {
                emit_member_encode_inline(out, m, &inner, table)?;
            }
        }
        IdlExtensibility::Appendable => {
            writeln!(out, "{inner}int __dh = w.beginAppendable();").map_err(fmt_err)?;
            for m in &resolved_wire_members(s, table) {
                emit_member_encode_inline(out, m, &inner, table)?;
            }
            writeln!(out, "{inner}w.endDelimited(__dh);").map_err(fmt_err)?;
        }
        IdlExtensibility::Mutable => {
            // XCDR1 / classic CDR encodes @mutable as PL_CDR1 (PID/length list),
            // XCDR2 as PL_CDR2 (DHEADER + EMHEADER). Branch at runtime on the
            // writer's representation — symmetric with the decode side.
            writeln!(out, "{inner}if (w.isXcdr1()) {{").map_err(fmt_err)?;
            let mut auto_id: u32 = 0;
            for m in &resolved_wire_members(s, table) {
                let mid = member_fixed_id(s, m).unwrap_or(auto_id);
                auto_id = mid + 1;
                emit_member_encode_pl_cdr1(out, m, &format!("{inner}    "), mid, table)?;
            }
            writeln!(out, "{inner}    w.writePlCdr1Sentinel();").map_err(fmt_err)?;
            writeln!(out, "{inner}}} else {{").map_err(fmt_err)?;
            writeln!(out, "{inner}    int __dh = w.beginMutable();").map_err(fmt_err)?;
            // Auto-id: 0, 1, 2, ... if no @id (XTypes 1.3 §7.3.4.3 @autoid(SEQUENTIAL)
            // default = 0-based declaration order; vendor-confirmed against Cyclone).
            // Honor explicit @id(N).
            let mut auto_id: u32 = 0;
            for m in &resolved_wire_members(s, table) {
                let mid = member_fixed_id(s, m).unwrap_or(auto_id);
                auto_id = mid + 1;
                emit_member_encode_mutable(out, m, &format!("{inner}    "), mid, table)?;
            }
            writeln!(out, "{inner}    w.endDelimited(__dh);").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        }
    }
    Ok(())
}

/// Array dimension list (as Java integer literals) for a declarator, e.g.
/// `long grid[4][4]` → `["4", "4"]`. Empty for non-array declarators.
fn array_dims(decl: &Declarator) -> Vec<String> {
    match decl {
        Declarator::Array(a) => a
            .sizes
            .iter()
            .map(crate::emitter::const_expr_to_java)
            .collect(),
        Declarator::Simple(_) => Vec::new(),
    }
}

fn emit_member_encode_inline(
    out: &mut String,
    m: &Member,
    inner: &str,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let getter = format!("sample.get{}()", capitalize(&name));
        if optional {
            writeln!(
                out,
                "{inner}if ({getter} != null && {getter}.isPresent()) {{"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{inner}    w.writePresenceFlag(true);").map_err(fmt_err)?;
            let val_expr = format!("{getter}.get()");
            emit_declarator_encode(
                out,
                &m.type_spec,
                &array_dims(decl),
                &val_expr,
                &format!("{inner}    "),
                table,
                0,
            )?;
            writeln!(out, "{inner}}} else {{").map_err(fmt_err)?;
            writeln!(out, "{inner}    w.writePresenceFlag(false);").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        } else {
            emit_declarator_encode(
                out,
                &m.type_spec,
                &array_dims(decl),
                &getter,
                inner,
                table,
                0,
            )?;
        }
    }
    Ok(())
}

/// Picks the compact XTypes 1.3 §7.4.3.4.2 length code for a `@mutable` member
/// so the EMHEADER matches what CycloneDDS / RTI Connext / FastDDS emit (was Bug
/// XV-mut: every member used LC4+NEXTINT). Mirrors `zerodds-idl-rust`'s
/// `mutable_member_length_code`: a scalar (non-array, non-optional) primitive uses
/// LC0–3 by wire size (1→LC0, 2→LC1, 4→LC2, 8→LC3); a string/wstring uses LC5,
/// which REUSES the value's own leading `uint32` length prefix as the NEXTINT (no
/// separate NEXTINT on the wire). Returns `None` → the universal LC4 fallback
/// (`beginMutable`/`endDelimited` NEXTINT framing) for arrays, optionals,
/// sequences, maps, nested/typedef/enum/bits aggregates and `long double`.
fn mutable_member_compact_lc(
    m: &Member,
    decl: &Declarator,
    table: &TypeTable,
) -> Option<&'static str> {
    if matches!(decl, Declarator::Array(_)) || member_is_optional(m) {
        return None;
    }
    match &m.type_spec {
        TypeSpec::Primitive(p) => match primitive_wire_size(*p) {
            1 => Some("org.zerodds.cdr.Xcdr2Writer.LC_BYTE"),
            2 => Some("org.zerodds.cdr.Xcdr2Writer.LC_SHORT"),
            4 => Some("org.zerodds.cdr.Xcdr2Writer.LC_INT32"),
            8 => Some("org.zerodds.cdr.Xcdr2Writer.LC_INT64"),
            _ => None, // long double (16) → LC4
        },
        // FINDING T1: a member whose XCDR2 body BEGINS WITH a 4-byte length word
        // — a string/wstring length prefix, a non-primitive `sequence`/`map`
        // DHEADER, or a nested `@appendable`/`@mutable` struct's DHEADER — uses
        // LC5 to REUSE that word as the NEXTINT (no separate NEXTINT on the
        // wire), matching CycloneDDS / RTI / FastDDS. A `@final` nested struct
        // (no DHEADER) and a `sequence<primitive>` (bare element count, not a
        // byte length) fall through to the universal LC4.
        spec if member_body_has_leading_dheader(spec, table) => {
            Some("org.zerodds.cdr.Xcdr2Writer.LC_NEXTINT_4")
        }
        _ => None,
    }
}

/// `true` if the XCDR2 body of a member of type `spec` begins with a 4-byte
/// length word that can serve as the EMHEADER NEXTINT (→ LC5). Mirror of
/// `zerodds-idl-rust::type_map::member_body_has_leading_dheader`:
///   * `string` / `wstring` — uint32 octet-length prefix;
///   * `map<K,V>` — always non-primitive → DHEADER;
///   * `sequence<E>` with a non-primitive element `E` — DHEADER (a
///     `sequence<primitive>` starts with a bare element count, NOT a byte
///     length, so it stays LC4);
///   * a nested struct (or typedef-to-struct) whose extensibility is
///     `@appendable` / `@mutable` — those self-delimit with a leading DHEADER;
///     a `@final` nested struct tight-packs its body → no DHEADER.
///
/// zerodds-lint: recursion-depth 16
fn member_body_has_leading_dheader(spec: &TypeSpec, table: &TypeTable) -> bool {
    match spec {
        TypeSpec::String(_) => true,
        // A `map<K,V>` has a leading DHEADER iff its (key,value) element is
        // non-primitive; `map<long,long>` starts with a bare element count
        // (NOT a byte length) → stays LC4, like `sequence<primitive>`.
        TypeSpec::Map(map) => {
            !(is_wire_primitive(&map.key, table) && is_wire_primitive(&map.value, table))
        }
        TypeSpec::Sequence(seq) => !is_wire_primitive(&seq.elem, table),
        TypeSpec::Scoped(sn) => match resolve_scoped(table, &scoped_to_short(sn)) {
            // A typedef inherits the framing of its underlying type.
            Some(ResolvedKind::Typedef { underlying, .. }) => {
                member_body_has_leading_dheader(underlying, table)
            }
            // A nested struct: leading DHEADER iff @appendable / @mutable.
            Some(ResolvedKind::Struct { ext, .. }) => !matches!(ext, IdlExtensibility::Final),
            // A standalone union is also DHEADER-delimited unless @final, but
            // the rust reference only resolves struct/string/seq/map here; a
            // union member keeps the conservative LC4 fallback (still valid).
            _ => false,
        },
        _ => false,
    }
}

/// Wire size (bytes) of a primitive — mirror of
/// `zerodds-idl-rust::type_map::primitive_wire_size`.
fn primitive_wire_size(p: PrimitiveType) -> usize {
    match p {
        PrimitiveType::Boolean | PrimitiveType::Octet | PrimitiveType::Char => 1,
        PrimitiveType::WideChar => 2,
        PrimitiveType::Integer(i) => match i {
            IntegerType::Int8 | IntegerType::UInt8 => 1,
            IntegerType::Short | IntegerType::Int16 | IntegerType::UShort | IntegerType::UInt16 => {
                2
            }
            IntegerType::Long | IntegerType::Int32 | IntegerType::ULong | IntegerType::UInt32 => 4,
            IntegerType::LongLong
            | IntegerType::Int64
            | IntegerType::ULongLong
            | IntegerType::UInt64 => 8,
        },
        PrimitiveType::Floating(f) => match f {
            FloatingType::Float => 4,
            FloatingType::Double => 8,
            FloatingType::LongDouble => 16,
        },
    }
}

/// XCDR2 PL_CDR2 mutable member (XTypes 1.3 §7.4.3.4.2). A scalar primitive/string
/// member uses a COMPACT length code (LC0–3 fixed-width, or LC5 for strings reusing
/// their leading length word) with NO separate NEXTINT — matching the cross-vendor-
/// validated rust golden. Everything else falls back to the universal LC4 +
/// `beginMutable`/`endDelimited` NEXTINT framing (it reserves a uint32, then patches
/// it with `pos - prefix - 4` = the body size).
fn emit_member_encode_mutable(
    out: &mut String,
    m: &Member,
    inner: &str,
    member_id: u32,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let getter = format!("sample.get{}()", capitalize(&name));
        // Compact LC path: scalar primitive/string member — no NEXTINT, body in
        // place. (Never applies to optionals; those keep the LC4 framing below.)
        if !optional {
            if let Some(lc) = mutable_member_compact_lc(m, decl, table) {
                writeln!(out, "{inner}w.writeEmHeader({member_id}, {lc}, false);")
                    .map_err(fmt_err)?;
                emit_declarator_encode(
                    out,
                    &m.type_spec,
                    &array_dims(decl),
                    &getter,
                    inner,
                    table,
                    0,
                )?;
                continue;
            }
        }
        let lc = "org.zerodds.cdr.Xcdr2Writer.LC_NEXTINT";
        if optional {
            writeln!(
                out,
                "{inner}if ({getter} != null && {getter}.isPresent()) {{"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{inner}    w.writeEmHeader({member_id}, {lc}, false);")
                .map_err(fmt_err)?;
            writeln!(out, "{inner}    int __ni{member_id} = w.beginMutable();").map_err(fmt_err)?;
            let val_expr = format!("{getter}.get()");
            emit_declarator_encode(
                out,
                &m.type_spec,
                &array_dims(decl),
                &val_expr,
                &format!("{inner}    "),
                table,
                0,
            )?;
            writeln!(out, "{inner}    w.endDelimited(__ni{member_id});").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        } else {
            writeln!(out, "{inner}w.writeEmHeader({member_id}, {lc}, false);").map_err(fmt_err)?;
            writeln!(out, "{inner}int __ni{member_id} = w.beginMutable();").map_err(fmt_err)?;
            emit_declarator_encode(
                out,
                &m.type_spec,
                &array_dims(decl),
                &getter,
                inner,
                table,
                0,
            )?;
            writeln!(out, "{inner}w.endDelimited(__ni{member_id});").map_err(fmt_err)?;
        }
    }
    Ok(())
}

/// XCDR1 `@mutable` (PL_CDR1) member encode: every member body is built in a
/// fresh member-relative XCDR1 sub-writer, then framed via
/// `writePlCdr1Member(id, body)` ([u16 PID][u16 length] + body + 4-byte pad).
/// An absent `@optional` member is omitted entirely (no PID), like the EMHEADER
/// path. Mirrors `idl-csharp`'s `emit_encode_member_pl_cdr1` and the verified
/// cdr-core / python reference.
fn emit_member_encode_pl_cdr1(
    out: &mut String,
    m: &Member,
    inner: &str,
    member_id: u32,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let getter = format!("sample.get{}()", capitalize(&name));
        if optional {
            writeln!(
                out,
                "{inner}if ({getter} != null && {getter}.isPresent()) {{"
            )
            .map_err(fmt_err)?;
            let val_expr = format!("{getter}.get()");
            emit_pl_cdr1_member_body(
                out,
                &m.type_spec,
                &array_dims(decl),
                &val_expr,
                &format!("{inner}    "),
                member_id,
                table,
            )?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        } else {
            emit_pl_cdr1_member_body(
                out,
                &m.type_spec,
                &array_dims(decl),
                &getter,
                inner,
                member_id,
                table,
            )?;
        }
    }
    Ok(())
}

/// Emits one PL_CDR1 member frame: a fresh sub-writer, the declarator body
/// (string-rewritten from `w.` to the sub-writer, like idl-csharp's
/// `emit_encode_value_into`), then `writePlCdr1Member`.
fn emit_pl_cdr1_member_body(
    out: &mut String,
    ts: &TypeSpec,
    dims: &[String],
    expr: &str,
    inner: &str,
    member_id: u32,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    let d = format!("{inner}    ");
    let sub = format!("__plw{member_id}");
    writeln!(
        out,
        "{d}org.zerodds.cdr.Xcdr2Writer {sub} = new org.zerodds.cdr.Xcdr2Writer(w.endian(), org.zerodds.cdr.Xcdr2Writer.XCDR1_MAX_ALIGN_VALUE);"
    )
    .map_err(fmt_err)?;
    // Emit the body referencing `w.`, then rewrite to the sub-writer. The body
    // never contains a bare `w.` other than the writer calls (the value
    // expressions are `sample.getX()` / nested `encode(expr, endian)`).
    let mut tmp = String::new();
    emit_declarator_encode(&mut tmp, ts, dims, expr, &d, table, 0)?;
    let patched = tmp.replace("w.", &format!("{sub}."));
    out.push_str(&patched);
    writeln!(
        out,
        "{d}w.writePlCdr1Member({member_id}, {sub}.toByteArray());"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    Ok(())
}

/// `true` when the current array dimension (`depth` into the declarator) must be
/// wrapped by a collection DHEADER. XTypes 1.3 §7.4.3.5 rule (8) PARRAY_TYPE: a
/// multi-dimensional array whose LEAF element is primitive is a single PARRAY and
/// carries NO collection DHEADER at ANY dimension (regardless of dimensionality);
/// only an array whose leaf is non-primitive (rule (9) ARRAY_TYPE — array-of-
/// struct/union/string/seq) carries ONE DHEADER, and it sits at the OUTERMOST
/// dimension (`depth == 0`) only — inner dimensions never get one. This mirrors
/// zerodds-cdr `[T; N]::encode`, where `IS_PRIMITIVE = T::IS_PRIMITIVE` propagates
/// the leaf's primitiveness up through every array level (composite.rs §7.4.3.5).
/// Was Bug XV-arr: the old `dims.len() > 1` test wrongly framed `long grid[2][3]`
/// with a spurious outer DHEADER.
fn array_dim_needs_dheader(
    ts: &TypeSpec,
    dims: &[String],
    depth: usize,
    table: &TypeTable,
) -> bool {
    let _ = dims;
    depth == 0 && !is_wire_primitive(ts, table)
}

/// Bug J #65(6): array members encode as nested fixed-count element loops (XCDR2
/// §7.4.4.5). For a multi-dim `T grid[A][B]` we iterate `A` then `B`, indexing
/// into the Java `T[][]`. A dimension whose element is non-primitive carries a
/// DHEADER (see [`array_dim_element_non_primitive`]); `dims` lists the remaining
/// array dimensions; when empty we fall through to the scalar/aggregate codec.
#[allow(clippy::too_many_arguments)]
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn emit_declarator_encode(
    out: &mut String,
    ts: &TypeSpec,
    dims: &[String],
    expr: &str,
    ind: &str,
    table: &TypeTable,
    depth: usize,
) -> Result<(), JavaGenError> {
    if dims.is_empty() {
        return emit_typespec_encode(out, ts, expr, ind, table, depth);
    }
    let dheader = array_dim_needs_dheader(ts, dims, depth, table);
    let dhv = format!("__adh{depth}");
    // Wrap the DHEADER-delimited dimension in its own block so the `__adh{depth}`
    // local does not collide across two top-level array members (both start at
    // depth 0, e.g. `grid` and `shape` in feat::Arr).
    let (body_ind, dheader) = if dheader {
        writeln!(out, "{ind}{{").map_err(fmt_err)?;
        let bi = format!("{ind}    ");
        writeln!(out, "{bi}int {dhv} = w.beginAppendable();").map_err(fmt_err)?;
        (bi, true)
    } else {
        (ind.to_string(), false)
    };
    let iv = format!("__ai{depth}");
    let elem = format!("{expr}[{iv}]");
    let size = &dims[0];
    writeln!(
        out,
        "{body_ind}for (int {iv} = 0; {iv} < {size}; {iv}++) {{"
    )
    .map_err(fmt_err)?;
    emit_declarator_encode(
        out,
        ts,
        &dims[1..],
        &elem,
        &format!("{body_ind}    "),
        table,
        depth + 1,
    )?;
    writeln!(out, "{body_ind}}}").map_err(fmt_err)?;
    if dheader {
        writeln!(out, "{body_ind}w.endDelimited({dhv});").map_err(fmt_err)?;
        writeln!(out, "{ind}}}").map_err(fmt_err)?;
    }
    Ok(())
}

// Retained for reference: the fixed-width LC short-forms (LC 0..3). The mutable
// member encoder now always uses LC=4 + NEXTINT to match the rust golden's
// PL_CDR2 framing, so this mapping is no longer used by codegen.
#[allow(dead_code)]
fn lc_for_typespec(ts: &TypeSpec) -> &'static str {
    match ts {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean | PrimitiveType::Octet | PrimitiveType::Char => {
                "org.zerodds.cdr.Xcdr2Writer.LC_BYTE"
            }
            PrimitiveType::WideChar => "org.zerodds.cdr.Xcdr2Writer.LC_SHORT",
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short
                | IntegerType::Int16
                | IntegerType::UShort
                | IntegerType::UInt16 => "org.zerodds.cdr.Xcdr2Writer.LC_SHORT",
                IntegerType::Long
                | IntegerType::Int32
                | IntegerType::ULong
                | IntegerType::UInt32 => "org.zerodds.cdr.Xcdr2Writer.LC_INT32",
                IntegerType::LongLong
                | IntegerType::Int64
                | IntegerType::ULongLong
                | IntegerType::UInt64 => "org.zerodds.cdr.Xcdr2Writer.LC_INT64",
                IntegerType::Int8 | IntegerType::UInt8 => "org.zerodds.cdr.Xcdr2Writer.LC_BYTE",
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => "org.zerodds.cdr.Xcdr2Writer.LC_INT32",
                FloatingType::Double | FloatingType::LongDouble => {
                    "org.zerodds.cdr.Xcdr2Writer.LC_INT64"
                }
            },
        },
        // String / Sequence / Scoped → variable, NEXTINT-aware.
        _ => "org.zerodds.cdr.Xcdr2Writer.LC_NEXTINT",
    }
}

/// Emits the encode for a single value of `ts` reachable via the Java
/// expression `expr`. `depth` makes nested temp identifiers unique (so
/// `sequence<sequence<T>>` does not re-declare `__seq`/`__el`).
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn emit_typespec_encode(
    out: &mut String,
    ts: &TypeSpec,
    expr: &str,
    ind: &str,
    table: &TypeTable,
    depth: usize,
) -> Result<(), JavaGenError> {
    match ts {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean => {
                writeln!(out, "{ind}w.writeBoolean({expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Octet => {
                writeln!(out, "{ind}w.writeOctet({expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Char => {
                writeln!(out, "{ind}w.writeChar({expr});").map_err(fmt_err)?;
            }
            PrimitiveType::WideChar => {
                writeln!(out, "{ind}w.writeWChar({expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short | IntegerType::Int16 => {
                    writeln!(out, "{ind}w.writeInt16({expr});").map_err(fmt_err)?;
                }
                IntegerType::UShort | IntegerType::UInt16 => {
                    writeln!(out, "{ind}w.writeUInt16({expr});").map_err(fmt_err)?;
                }
                IntegerType::Long | IntegerType::Int32 => {
                    writeln!(out, "{ind}w.writeInt32({expr});").map_err(fmt_err)?;
                }
                IntegerType::ULong | IntegerType::UInt32 => {
                    writeln!(out, "{ind}w.writeUInt32({expr});").map_err(fmt_err)?;
                }
                IntegerType::LongLong | IntegerType::Int64 => {
                    writeln!(out, "{ind}w.writeInt64({expr});").map_err(fmt_err)?;
                }
                IntegerType::ULongLong | IntegerType::UInt64 => {
                    writeln!(out, "{ind}w.writeUInt64({expr});").map_err(fmt_err)?;
                }
                IntegerType::Int8 => {
                    writeln!(out, "{ind}w.writeOctet({expr});").map_err(fmt_err)?;
                }
                IntegerType::UInt8 => {
                    writeln!(out, "{ind}w.writeUInt8({expr});").map_err(fmt_err)?;
                }
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => {
                    writeln!(out, "{ind}w.writeFloat32({expr});").map_err(fmt_err)?;
                }
                FloatingType::Double => {
                    writeln!(out, "{ind}w.writeFloat64({expr});").map_err(fmt_err)?;
                }
                // `long double` is IEEE-754 binary128 on the wire (16 bytes).
                // Java has no binary128 primitive and the CDR runtime exposes
                // no 16-byte float writer; emitting `writeFloat64` produced an
                // 8-byte member under a 16-byte length code, desynchronising
                // the whole stream. Refuse loudly until an f128-backed path
                // exists (see P12 / long-double bucket).
                FloatingType::LongDouble => return Err(long_double_unsupported()),
            },
        },
        TypeSpec::String(s) => {
            // Bounded `string<N>` (DDS-XTypes §7.4.3): reject over-bound on
            // encode like strict vendors do. Narrow → UTF-8 byte length (matches
            // the CDR wire); wide `wstring<N>` → UTF-16 unit count (String.length).
            if let Some(b) = &s.bound {
                let bv = crate::emitter::const_expr_to_java(b);
                if s.wide {
                    writeln!(
                        out,
                        "{ind}if ({expr} != null && {expr}.length() > {bv}) throw new IllegalArgumentException(\"bounded wstring length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                } else {
                    writeln!(
                        out,
                        "{ind}if ({expr} != null && {expr}.getBytes(java.nio.charset.StandardCharsets.UTF_8).length > {bv}) throw new IllegalArgumentException(\"bounded string length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                }
            }
            // XTypes 1.3 §7.4.3 / CANONICAL.md wstring rule: `wstring` is UTF-16
            // on the wire — `uint32` BYTE-length, then the UTF-16LE code units,
            // NO BOM, NO terminator. (`writeString` is UTF-8+NUL; the runtime
            // `writeWString` prefixes the UNIT count, not the byte length — both
            // diverge from the cross-vendor golden.) Emit the byte-length-prefixed
            // form inline from the available `writeUInt32` + `writeWChar` (UTF-16LE
            // code unit) primitives.
            if s.wide {
                writeln!(out, "{ind}{{").map_err(fmt_err)?;
                writeln!(out, "{ind}    char[] __wc = ({expr}).toCharArray();").map_err(fmt_err)?;
                writeln!(out, "{ind}    w.writeUInt32((long) __wc.length * 2L);")
                    .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{ind}    for (char __wu : __wc) {{ w.writeWChar(__wu); }}"
                )
                .map_err(fmt_err)?;
                writeln!(out, "{ind}}}").map_err(fmt_err)?;
            } else {
                writeln!(out, "{ind}w.writeString({expr});").map_err(fmt_err)?;
            }
        }
        TypeSpec::Sequence(seq) => {
            // XCDR2 §7.4.3.5: non-primitive elements (string, struct, nested
            // sequence, …) → DHEADER (uint32 = byte length of [count + elements])
            // prepended; primitives do not get one.
            let non_primitive = !is_wire_primitive(&seq.elem, table);
            let seqv = format!("__seq{depth}");
            let dhv = format!("__seqdh{depth}");
            let elv = format!("__el{depth}");
            let elem_java = boxed_for_seq_elem(&seq.elem, table);
            writeln!(out, "{ind}{{").map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}    java.util.List<{elem_java}> {seqv} = ({expr} == null) ? java.util.Collections.<{elem_java}>emptyList() : ({expr});"
            )
            .map_err(fmt_err)?;
            // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode error.
            if let Some(b) = &seq.bound {
                let bv = crate::emitter::const_expr_to_java(b);
                writeln!(
                    out,
                    "{ind}    if ({seqv}.size() > {bv}) throw new IllegalArgumentException(\"bounded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            if non_primitive {
                writeln!(out, "{ind}    int {dhv} = w.beginAppendable();").map_err(fmt_err)?;
            }
            writeln!(out, "{ind}    w.writeSequenceCount({seqv}.size());").map_err(fmt_err)?;
            writeln!(out, "{ind}    for ({elem_java} {elv} : {seqv}) {{").map_err(fmt_err)?;
            let inner_indent = format!("{ind}        ");
            emit_typespec_encode(out, &seq.elem, &elv, &inner_indent, table, depth + 1)?;
            writeln!(out, "{ind}    }}").map_err(fmt_err)?;
            if non_primitive {
                writeln!(out, "{ind}    w.endDelimited({dhv});").map_err(fmt_err)?;
            }
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        TypeSpec::Map(map) => emit_map_encode(out, map, expr, ind, table, depth)?,
        TypeSpec::Scoped(s) => {
            let short = scoped_to_short(s);
            match resolve_scoped(table, &short) {
                // Bug J #65(3): enum-in-struct → signed ordinal; T2: narrowed to
                // the enum's @bit_bound width (XTypes §7.4.5.1).
                Some(ResolvedKind::Enum { holder_bytes }) => {
                    let line = match holder_bytes {
                        1 => format!("{ind}w.writeOctet((byte)(({expr}).value()));"),
                        2 => format!("{ind}w.writeInt16((short)(({expr}).value()));"),
                        _ => format!("{ind}w.writeInt32(({expr}).value());"),
                    };
                    writeln!(out, "{line}").map_err(fmt_err)?;
                }
                // Bitmask member → a single holder integer (no DHEADER): OR the
                // declared bit positions present in the Java `EnumSet`. Matches
                // the rust `Perm(uN)` holder (XTypes 1.3 §7.4.3 bitmask wire form).
                Some(ResolvedKind::Bitmask { holder_bytes }) => {
                    writeln!(out, "{ind}{{").map_err(fmt_err)?;
                    writeln!(out, "{ind}    long __bm = 0L;").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{ind}    if ({expr} != null) for ({short}.Flag __f : ({expr}).bits()) {{ __bm |= (1L << __f.position()); }}"
                    )
                    .map_err(fmt_err)?;
                    emit_bits_holder_write(out, *holder_bytes, "__bm", &format!("{ind}    "))?;
                    writeln!(out, "{ind}}}").map_err(fmt_err)?;
                }
                // Bitset member → a single holder integer (no DHEADER): the
                // POJO's packed `rawBits()`. Matches the rust `Flags { storage: uN }`.
                Some(ResolvedKind::Bitset { holder_bytes }) => {
                    writeln!(out, "{ind}{{").map_err(fmt_err)?;
                    writeln!(out, "{ind}    long __bs = ({expr}).rawBits();").map_err(fmt_err)?;
                    emit_bits_holder_write(out, *holder_bytes, "__bs", &format!("{ind}    "))?;
                    writeln!(out, "{ind}}}").map_err(fmt_err)?;
                }
                // Bug J #65(4): typedef member → resolve the alias to the
                // underlying type, unwrapping the Java wrapper's `value()`.
                Some(ResolvedKind::Typedef {
                    underlying,
                    array_dims,
                }) => {
                    let inner = format!("({expr}).value()");
                    emit_declarator_encode(
                        out,
                        underlying,
                        array_dims,
                        &inner,
                        ind,
                        table,
                        depth + 1,
                    )?;
                }
                // Struct / union / unknown external ref → delegate with its own
                // DHEADER frame (the nested TypeSupport.encode produces one).
                //
                // The nested frame leads with a uint32 DHEADER, which the
                // decoder reads via `readDelimitedFrame → readDHeader`, and
                // `readDHeader` aligns to 4 first. `writeBytes` copies raw with
                // NO alignment, so unless we align the cursor to 4 here, a
                // nested struct/union that follows a variable-length value
                // (e.g. a `map<string,Struct>` value after its string key, or a
                // later element of `sequence<sequence<Struct>>`) lands at a
                // non-4 offset and the reader's align(4) desyncs — manifesting
                // as a bogus DHEADER size. Align before the raw copy to match.
                _ => {
                    writeln!(out, "{ind}w.align(4);").map_err(fmt_err)?;
                    // Propagate the representation: under XCDR1 the nested
                    // aggregate is itself classic-CDR (no DHEADER, max-align 8),
                    // else XCDR2. `w.isXcdr1()` reflects the active writer.
                    writeln!(
                        out,
                        "{ind}w.writeBytes({short}TypeSupport.INSTANCE.encode({expr}, endian, w.isXcdr1() ? 0 : 1));"
                    )
                    .map_err(fmt_err)?;
                }
            }
        }
        TypeSpec::Fixed(f) => {
            // fixed<P,S>: CORBA/GIOP §9.3.2.7 packed BCD (the BigDecimal field ->
            // (P+2)/2 octets via the runtime helper). No alignment/length/endian.
            let p = crate::emitter::const_expr_to_java(&f.digits);
            let s = crate::emitter::const_expr_to_java(&f.scale);
            writeln!(out, "{ind}w.writeFixedBcd({expr}, {p}, {s});").map_err(fmt_err)?;
        }
        _ => {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: "typesupport-encode for this TypeSpec".into(),
                context: None,
            });
        }
    }
    Ok(())
}

/// Bug J #65(1): `map<K,V>` codec. XCDR2 represents a map exactly like a
/// `sequence` of `{key, value}` pairs, prefixed with a DHEADER (the pairs are
/// non-primitive aggregates). We iterate the Java `Map.Entry` set.
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn emit_map_encode(
    out: &mut String,
    map: &zerodds_idl::ast::MapType,
    expr: &str,
    ind: &str,
    table: &TypeTable,
    depth: usize,
) -> Result<(), JavaGenError> {
    let kj = boxed_for_seq_elem(&map.key, table);
    let vj = boxed_for_seq_elem(&map.value, table);
    let mv = format!("__map{depth}");
    let dhv = format!("__mapdh{depth}");
    let ev = format!("__ment{depth}");
    writeln!(out, "{ind}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{ind}    java.util.Map<{kj}, {vj}> {mv} = ({expr} == null) ? java.util.Collections.<{kj}, {vj}>emptyMap() : ({expr});"
    )
    .map_err(fmt_err)?;
    if let Some(b) = &map.bound {
        let bv = crate::emitter::const_expr_to_java(b);
        writeln!(
            out,
            "{ind}    if ({mv}.size() > {bv}) throw new IllegalArgumentException(\"bounded map length exceeds its IDL bound ({bv})\");"
        )
        .map_err(fmt_err)?;
    }
    // XCDR2 §7.4.3.5: a map carries a DHEADER only when its (key,value) element
    // is non-primitive. `map<long,long>` (both primitive) omits it — matching
    // cdr-core `needs_collection_dheader(.., K::IS_PRIMITIVE && V::IS_PRIMITIVE)`
    // and FastDDS/OpenDDS. (Same rule as `sequence<primitive>`.)
    let map_dh = !(is_wire_primitive(&map.key, table) && is_wire_primitive(&map.value, table));
    if map_dh {
        writeln!(out, "{ind}    int {dhv} = w.beginAppendable();").map_err(fmt_err)?;
    }
    writeln!(out, "{ind}    w.writeSequenceCount({mv}.size());").map_err(fmt_err)?;
    writeln!(
        out,
        "{ind}    for (java.util.Map.Entry<{kj}, {vj}> {ev} : {mv}.entrySet()) {{"
    )
    .map_err(fmt_err)?;
    let kexpr = format!("{ev}.getKey()");
    let vexpr = format!("{ev}.getValue()");
    let inner = format!("{ind}        ");
    emit_typespec_encode(out, &map.key, &kexpr, &inner, table, depth + 1)?;
    emit_typespec_encode(out, &map.value, &vexpr, &inner, table, depth + 1)?;
    writeln!(out, "{ind}    }}").map_err(fmt_err)?;
    if map_dh {
        writeln!(out, "{ind}    w.endDelimited({dhv});").map_err(fmt_err)?;
    }
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

/// Writes a bitmask/bitset holder integer (`var` is a Java `long` carrying the
/// packed bits) at the holder's wire width. XTypes 1.3 §7.4.3: the holder is an
/// unsigned integer of {1,2,4,8} bytes — exactly the rust `Perm(uN)`/`Flags{uN}`.
fn emit_bits_holder_write(
    out: &mut String,
    holder_bytes: u8,
    var: &str,
    ind: &str,
) -> Result<(), JavaGenError> {
    match holder_bytes {
        1 => writeln!(out, "{ind}w.writeUInt8((int) ({var} & 0xFFL));").map_err(fmt_err)?,
        2 => writeln!(out, "{ind}w.writeUInt16((int) ({var} & 0xFFFFL));").map_err(fmt_err)?,
        4 => writeln!(out, "{ind}w.writeUInt32({var} & 0xFFFFFFFFL);").map_err(fmt_err)?,
        _ => writeln!(out, "{ind}w.writeInt64({var});").map_err(fmt_err)?,
    }
    Ok(())
}

/// Reads a bitmask/bitset holder integer at its wire width into a Java `long`
/// (the inverse of [`emit_bits_holder_write`]).
fn bits_holder_read_expr(holder_bytes: u8) -> &'static str {
    match holder_bytes {
        1 => "(long) r.readUInt8()",
        2 => "(long) r.readUInt16()",
        4 => "r.readUInt32()",
        _ => "r.readInt64()",
    }
}

/// Whether a sequence/collection element is a *wire* primitive (no DHEADER).
/// Enums are int32 (primitive on the wire); typedefs follow their underlying.
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn is_wire_primitive(ts: &TypeSpec, table: &TypeTable) -> bool {
    match ts {
        TypeSpec::Primitive(_) => true,
        TypeSpec::Scoped(s) => match resolve_scoped(table, &scoped_to_short(s)) {
            Some(ResolvedKind::Enum { .. }) => true,
            // Bitmask/bitset are a single holder integer on the wire (no DHEADER),
            // so they are wire-primitive — a `sequence<Perm>` carries no per-
            // element delimiter.
            Some(ResolvedKind::Bitmask { .. }) | Some(ResolvedKind::Bitset { .. }) => true,
            Some(ResolvedKind::Typedef { underlying, .. }) => is_wire_primitive(underlying, table),
            _ => false,
        },
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Decode body emitter
// ---------------------------------------------------------------------------

fn emit_decode_body(
    out: &mut String,
    s: &StructDef,
    ind: &str,
    ext: IdlExtensibility,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    let inner = format!("{ind}{ind}");
    match ext {
        IdlExtensibility::Final => {
            for m in &resolved_wire_members(s, table) {
                emit_member_decode_inline(out, m, &inner, table)?;
            }
        }
        IdlExtensibility::Appendable => {
            writeln!(out, "{inner}int __dhSize = r.readDHeader();").map_err(fmt_err)?;
            writeln!(out, "{inner}int __endPos = r.position() + __dhSize;").map_err(fmt_err)?;
            for m in &resolved_wire_members(s, table) {
                emit_member_decode_inline(out, m, &inner, table)?;
            }
            writeln!(
                out,
                "{inner}while (r.position() < __endPos) {{ r.skip(1); }}"
            )
            .map_err(fmt_err)?;
        }
        IdlExtensibility::Mutable => {
            // XCDR1 (classic CDR): @mutable is PL_CDR1 — a [PID][length] list with
            // no outer DHEADER. Emit it alongside the XCDR2 EMHEADER loop.
            writeln!(out, "{inner}if (r.isXcdr1()) {{").map_err(fmt_err)?;
            writeln!(
                out,
                "{inner}    org.zerodds.cdr.Xcdr2Reader.PlCdr1Member __plm;"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{inner}    while ((__plm = r.beginPlCdr1Member()) != null) {{"
            )
            .map_err(fmt_err)?;
            {
                let mut auto_id1: u32 = 0;
                let mut first1 = true;
                for m in &resolved_wire_members(s, table) {
                    let mid = member_fixed_id(s, m).unwrap_or(auto_id1);
                    auto_id1 = mid + 1;
                    emit_member_decode_mutable_branch(out, m, &inner, mid, first1, table, "__plm")?;
                    first1 = false;
                }
            }
            writeln!(out, "{inner}        r.endPlCdr1Member(__plm);").map_err(fmt_err)?;
            writeln!(out, "{inner}    }}").map_err(fmt_err)?;
            writeln!(out, "{inner}    return v;").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
            writeln!(out, "{inner}int __dhSize = r.readDHeader();").map_err(fmt_err)?;
            writeln!(out, "{inner}int __endPos = r.position() + __dhSize;").map_err(fmt_err)?;
            writeln!(out, "{inner}while (r.position() < __endPos) {{").map_err(fmt_err)?;
            writeln!(
                out,
                "{inner}    org.zerodds.cdr.Xcdr2Reader.EmHeader __em = r.readEmHeader();"
            )
            .map_err(fmt_err)?;
            // XCDR2 PL_CDR2: ONLY LC4 carries a separate explicit NEXTINT byte-
            // length between the EMHEADER and the member body (XTypes 1.3
            // §7.4.3.4.2). Consume it once here; the known-member branches then
            // read the body in-place, and the unknown-member fallback skips `__ni`
            // bytes directly. Compact LC0–3 read fixed-width bodies in place, and
            // LC5/6/7 REUSE the body's own leading uint32 length word as the
            // NEXTINT (no separate one on the wire) — so they must NOT be eagerly
            // consumed here; an unknown LC5/6/7 member is skipped via `skipByLc`,
            // which reads that leading length word itself.
            writeln!(
                out,
                "{inner}    int __ni = (__em.lc == 4) ? r.readNextInt() : -1;"
            )
            .map_err(fmt_err)?;
            // Auto-id: 0, 1, 2, ... if no @id (XTypes 1.3 §7.3.4.3 @autoid(SEQUENTIAL)
            // default = 0-based declaration order; vendor-confirmed against Cyclone).
            let mut auto_id: u32 = 0;
            let mut first = true;
            for m in &resolved_wire_members(s, table) {
                let mid = member_fixed_id(s, m).unwrap_or(auto_id);
                auto_id = mid + 1;
                emit_member_decode_mutable_branch(out, m, &inner, mid, first, table, "__em")?;
                first = false;
            }
            // Default branch: unknown member. For LC>=4 the byte-length was
            // already read into `__ni`, so skip exactly that many bytes; for the
            // fixed short-forms (LC 0..3) skip by the length code.
            writeln!(out, "{inner}    else {{").map_err(fmt_err)?;
            writeln!(out, "{inner}        if (__ni >= 0) {{ r.skip(__ni); }}").map_err(fmt_err)?;
            writeln!(out, "{inner}        else {{ skipByLc(r, __em.lc); }}").map_err(fmt_err)?;
            writeln!(out, "{inner}    }}").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        }
    }
    Ok(())
}

fn emit_member_decode_inline(
    out: &mut String,
    m: &Member,
    inner: &str,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let setter = format!("v.set{}", capitalize(&name));
        if optional {
            writeln!(out, "{inner}if (r.readPresenceFlag()) {{").map_err(fmt_err)?;
            emit_optional_value_decode(
                out,
                &m.type_spec,
                &array_dims(decl),
                &setter,
                &format!("{inner}    "),
                table,
            )?;
            writeln!(out, "{inner}}} else {{").map_err(fmt_err)?;
            writeln!(out, "{inner}    {setter}(java.util.Optional.empty());").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        } else {
            emit_declarator_decode(out, &m.type_spec, &array_dims(decl), &setter, inner, table)?;
        }
    }
    Ok(())
}

/// Decodes a *present* `@optional` member (any TypeSpec, including an aggregate
/// — sequence / nested struct / map / enum / typedef — and possibly an array)
/// and feeds it to its `setter` wrapped in `java.util.Optional.of(...)`.
///
/// The old code used a single `read_expr_for_typespec` call, which only knew
/// primitives + strings, so `@optional sequence<…>`, `@optional NestedStruct`,
/// `@optional map<…>` and `@optional long v[N]` made codegen fail outright.
/// Routing through `emit_read_into` / `emit_array_fill` reuses the full,
/// already-correct member codec for the inner value.
fn emit_optional_value_decode(
    out: &mut String,
    ts: &TypeSpec,
    dims: &[String],
    setter: &str,
    ind: &str,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    writeln!(out, "{ind}{{").map_err(fmt_err)?;
    let inner = format!("{ind}    ");
    if dims.is_empty() {
        let jt = java_value_type(ts, table);
        emit_read_into(out, ts, "__ov", &jt, &inner, table, 0)?;
        writeln!(out, "{inner}{setter}(java.util.Optional.of(__ov));").map_err(fmt_err)?;
    } else {
        let elem_jt = java_value_type(ts, table);
        let brackets: String = dims.iter().map(|d| format!("[{d}]")).collect();
        let empty_brackets: String = dims.iter().map(|_| "[]").collect();
        emit_array_alloc(out, &inner, &elem_jt, &empty_brackets, &brackets, "__oarr")?;
        emit_array_fill(out, ts, dims, "__oarr", &inner, table, 0)?;
        writeln!(out, "{inner}{setter}(java.util.Optional.of(__oarr));").map_err(fmt_err)?;
    }
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

fn emit_member_decode_mutable_branch(
    out: &mut String,
    m: &Member,
    inner: &str,
    member_id: u32,
    first: bool,
    table: &TypeTable,
    id_var: &str,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    let kw = if first { "if" } else { "else if" };
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let setter = format!("v.set{}", capitalize(&name));
        writeln!(out, "{inner}    {kw} ({id_var}.memberId == {member_id}) {{").map_err(fmt_err)?;
        if optional {
            emit_optional_value_decode(
                out,
                &m.type_spec,
                &array_dims(decl),
                &setter,
                &format!("{inner}        "),
                table,
            )?;
        } else {
            emit_declarator_decode(
                out,
                &m.type_spec,
                &array_dims(decl),
                &setter,
                &format!("{inner}        "),
                table,
            )?;
        }
        writeln!(out, "{inner}    }}").map_err(fmt_err)?;
    }
    Ok(())
}

/// Decodes a member declarator (possibly an array) and feeds the result to its
/// `setter`. For arrays we allocate the Java array, fill it with nested
/// fixed-count loops, and pass it on; otherwise we read a single value.
fn emit_declarator_decode(
    out: &mut String,
    ts: &TypeSpec,
    dims: &[String],
    setter: &str,
    ind: &str,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    if dims.is_empty() {
        writeln!(out, "{ind}{{").map_err(fmt_err)?;
        let inner = format!("{ind}    ");
        let jt = java_value_type(ts, table);
        emit_read_into(out, ts, "__rv", &jt, &inner, table, 0)?;
        writeln!(out, "{ind}    {setter}(__rv);").map_err(fmt_err)?;
        writeln!(out, "{ind}}}").map_err(fmt_err)?;
        return Ok(());
    }
    // Allocate the full multidimensional Java array, then fill it.
    let elem_jt = java_value_type(ts, table);
    let brackets: String = dims.iter().map(|d| format!("[{d}]")).collect();
    let empty_brackets: String = dims.iter().map(|_| "[]").collect();
    writeln!(out, "{ind}{{").map_err(fmt_err)?;
    emit_array_alloc(
        out,
        &format!("{ind}    "),
        &elem_jt,
        &empty_brackets,
        &brackets,
        "__arr",
    )?;
    emit_array_fill(out, ts, dims, "__arr", &format!("{ind}    "), table, 0)?;
    writeln!(out, "{ind}    {setter}(__arr);").map_err(fmt_err)?;
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

/// Recursively fills `target[i0][i1]...` from the wire. A dimension whose
/// element is non-primitive is DHEADER-delimited (mirrors the encode side and
/// zerodds-cdr `[T;N]::decode`); we read+discard that uint32 before the loop.
#[allow(clippy::too_many_arguments)]
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn emit_array_fill(
    out: &mut String,
    ts: &TypeSpec,
    dims: &[String],
    target: &str,
    ind: &str,
    table: &TypeTable,
    depth: usize,
) -> Result<(), JavaGenError> {
    if array_dim_needs_dheader(ts, dims, depth, table) {
        writeln!(out, "{ind}r.readDHeader();").map_err(fmt_err)?;
    }
    let iv = format!("__ad{depth}");
    let size = &dims[0];
    let slot = format!("{target}[{iv}]");
    writeln!(out, "{ind}for (int {iv} = 0; {iv} < {size}; {iv}++) {{").map_err(fmt_err)?;
    if dims.len() == 1 {
        let inner = format!("{ind}    ");
        let jt = java_value_type(ts, table);
        emit_read_into(out, ts, &format!("__av{depth}"), &jt, &inner, table, depth)?;
        writeln!(out, "{ind}    {slot} = __av{depth};").map_err(fmt_err)?;
    } else {
        emit_array_fill(
            out,
            ts,
            &dims[1..],
            &slot,
            &format!("{ind}    "),
            table,
            depth + 1,
        )?;
    }
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

/// Reads one value of `ts` and binds it to a fresh `var` of Java type `jt`.
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn emit_read_into(
    out: &mut String,
    ts: &TypeSpec,
    var: &str,
    jt: &str,
    ind: &str,
    table: &TypeTable,
    depth: usize,
) -> Result<(), JavaGenError> {
    match ts {
        // `wstring`: read the uint32 BYTE-length, then byte-length/2 UTF-16LE code
        // units (CANONICAL.md wstring rule). The runtime `readWString` expects a
        // UNIT count, so decode inline to stay byte-exact with the rust golden.
        TypeSpec::String(s) if s.wide => {
            writeln!(out, "{ind}{jt} {var};").map_err(fmt_err)?;
            writeln!(out, "{ind}{{").map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}    int __wn{depth} = (int) (r.readUInt32() / 2);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{ind}    char[] __wb{depth} = new char[__wn{depth}];")
                .map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}    for (int __wi{depth} = 0; __wi{depth} < __wn{depth}; __wi{depth}++) {{ __wb{depth}[__wi{depth}] = r.readWChar(); }}"
            )
            .map_err(fmt_err)?;
            // Bounded `wstring<N>` (B1 blocker fix, deep review of #22
            // decode-bounds-cross-backend): reject an over-bound decode the
            // same way encode does (`emit_typespec_encode`, `s.wide` branch,
            // `.length() > bound`) — this check was missing entirely, so a
            // bounded wide string was silently unenforced on decode. Mirrors
            // the narrow `string<N>` decode check below (XTypes 1.3 §7.4.3).
            if let Some(b) = &s.bound {
                let bv = crate::emitter::const_expr_to_java(b);
                writeln!(
                    out,
                    "{ind}    if (__wn{depth} > {bv}) throw new IllegalArgumentException(\"decoded wstring length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{ind}    {var} = new String(__wb{depth});").map_err(fmt_err)?;
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        // Bounded narrow `string<N>`: cannot ride the single-expression
        // `read_expr_for_typespec` path below (no room to insert a check) —
        // B1 follow-up (#22 decode-side parity): mirror the encode-side
        // UTF-8-byte-length check (`emit_typespec_encode` above) on decode
        // too. XTypes 1.3 §7.4.3 requires enforcement on BOTH sides;
        // `r.readString()` only ever validated the wire's remaining bytes.
        TypeSpec::String(s) if !s.wide && s.bound.is_some() => {
            let bv = s
                .bound
                .as_ref()
                .map(crate::emitter::const_expr_to_java)
                .unwrap_or_default();
            let read = read_expr_for_typespec(ts)?;
            writeln!(out, "{ind}{jt} {var} = {read};").map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}if ({var} != null && {var}.getBytes(java.nio.charset.StandardCharsets.UTF_8).length > {bv}) throw new IllegalArgumentException(\"decoded string length exceeds its IDL bound ({bv})\");"
            )
            .map_err(fmt_err)?;
        }
        // `fixed<P,S>` decodes through the single-expr path too (BigDecimal =
        // r.readFixedBcd(P, S)).
        TypeSpec::Primitive(_) | TypeSpec::String(_) | TypeSpec::Fixed(_) => {
            let read = read_expr_for_typespec(ts)?;
            writeln!(out, "{ind}{jt} {var} = {read};").map_err(fmt_err)?;
        }
        TypeSpec::Sequence(seq) => {
            let elem_ty = boxed_for_seq_elem(&seq.elem, table);
            let non_primitive = !is_wire_primitive(&seq.elem, table);
            let cntv = format!("__cnt{depth}");
            let outv = format!("__out{depth}");
            let iv = format!("__si{depth}");
            let elemv = format!("__se{depth}");
            writeln!(out, "{ind}{jt} {var};").map_err(fmt_err)?;
            writeln!(out, "{ind}{{").map_err(fmt_err)?;
            if non_primitive {
                writeln!(out, "{ind}    r.readDHeader();").map_err(fmt_err)?;
            }
            writeln!(out, "{ind}    int {cntv} = r.readSequenceCount();").map_err(fmt_err)?;
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // bound check — XTypes 1.3 §7.4.3.
            if let Some(b) = &seq.bound {
                let bv = crate::emitter::const_expr_to_java(b);
                writeln!(
                    out,
                    "{ind}    if ({cntv} > {bv}) throw new IllegalArgumentException(\"decoded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            writeln!(
                out,
                "{ind}    java.util.List<{elem_ty}> {outv} = new java.util.ArrayList<>({cntv});"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{ind}    for (int {iv} = 0; {iv} < {cntv}; {iv}++) {{")
                .map_err(fmt_err)?;
            emit_read_into(
                out,
                &seq.elem,
                &elemv,
                &elem_ty,
                &format!("{ind}        "),
                table,
                depth + 1,
            )?;
            writeln!(out, "{ind}        {outv}.add({elemv});").map_err(fmt_err)?;
            writeln!(out, "{ind}    }}").map_err(fmt_err)?;
            writeln!(out, "{ind}    {var} = {outv};").map_err(fmt_err)?;
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        TypeSpec::Map(map) => {
            let kj = boxed_for_seq_elem(&map.key, table);
            let vj = boxed_for_seq_elem(&map.value, table);
            let cntv = format!("__mcnt{depth}");
            let outv = format!("__mout{depth}");
            let iv = format!("__mi{depth}");
            let kv = format!("__mk{depth}");
            let vv = format!("__mv{depth}");
            // XCDR2 §7.4.3.5: only a non-primitive map carries a leading DHEADER.
            // (Symmetric with the encode gate. In a @mutable struct the LC4
            // primitive-map NEXTINT is already consumed at the EMHEADER dispatch,
            // and the LC5 non-primitive map's leading length word is this DHEADER.)
            let map_dh =
                !(is_wire_primitive(&map.key, table) && is_wire_primitive(&map.value, table));
            writeln!(out, "{ind}{jt} {var};").map_err(fmt_err)?;
            writeln!(out, "{ind}{{").map_err(fmt_err)?;
            if map_dh {
                writeln!(out, "{ind}    r.readDHeader();").map_err(fmt_err)?;
            }
            writeln!(out, "{ind}    int {cntv} = r.readSequenceCount();").map_err(fmt_err)?;
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // bound check — XTypes 1.3 §7.4.3.
            if let Some(b) = &map.bound {
                let bv = crate::emitter::const_expr_to_java(b);
                writeln!(
                    out,
                    "{ind}    if ({cntv} > {bv}) throw new IllegalArgumentException(\"decoded map length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            writeln!(
                out,
                "{ind}    java.util.Map<{kj}, {vj}> {outv} = new java.util.LinkedHashMap<>();"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{ind}    for (int {iv} = 0; {iv} < {cntv}; {iv}++) {{")
                .map_err(fmt_err)?;
            emit_read_into(
                out,
                &map.key,
                &kv,
                &kj,
                &format!("{ind}        "),
                table,
                depth + 1,
            )?;
            emit_read_into(
                out,
                &map.value,
                &vv,
                &vj,
                &format!("{ind}        "),
                table,
                depth + 1,
            )?;
            writeln!(out, "{ind}        {outv}.put({kv}, {vv});").map_err(fmt_err)?;
            writeln!(out, "{ind}    }}").map_err(fmt_err)?;
            writeln!(out, "{ind}    {var} = {outv};").map_err(fmt_err)?;
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        TypeSpec::Scoped(s) => {
            let short = scoped_to_short(s);
            match resolve_scoped(table, &short) {
                Some(ResolvedKind::Enum { holder_bytes }) => {
                    // Wire form is a signed ordinal at the @bit_bound width → map
                    // back via the enum's `value()` (linear scan over constants).
                    let read = match holder_bytes {
                        1 => "r.readOctet()",
                        2 => "r.readInt16()",
                        _ => "r.readInt32()",
                    };
                    writeln!(out, "{ind}int __ord{depth} = {read};").map_err(fmt_err)?;
                    writeln!(out, "{ind}{jt} {var} = null;").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{ind}for ({short} __ec{depth} : {short}.values()) {{ if (__ec{depth}.value() == __ord{depth}) {{ {var} = __ec{depth}; break; }} }}"
                    )
                    .map_err(fmt_err)?;
                }
                // Bitmask: read the holder integer, then rebuild the Java
                // `EnumSet`-backed wrapper by testing each declared bit position.
                Some(ResolvedKind::Bitmask { holder_bytes }) => {
                    let read = bits_holder_read_expr(*holder_bytes);
                    writeln!(out, "{ind}long __bmh{depth} = {read};").map_err(fmt_err)?;
                    writeln!(out, "{ind}{jt} {var} = new {jt}();").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{ind}for ({short}.Flag __bf{depth} : {short}.Flag.values()) {{ if ((__bmh{depth} & (1L << __bf{depth}.position())) != 0) {{ {var}.set(__bf{depth}); }} }}"
                    )
                    .map_err(fmt_err)?;
                }
                // Bitset: read the holder integer straight into the packed
                // `long`-backed wrapper (POJO has a `(long bits)` constructor).
                Some(ResolvedKind::Bitset { holder_bytes }) => {
                    let read = bits_holder_read_expr(*holder_bytes);
                    writeln!(out, "{ind}{jt} {var} = new {jt}({read});").map_err(fmt_err)?;
                }
                Some(ResolvedKind::Typedef {
                    underlying,
                    array_dims,
                }) => {
                    if array_dims.is_empty() {
                        let inner_jt = java_value_type(underlying, table);
                        emit_read_into(
                            out,
                            underlying,
                            &format!("__tv{depth}"),
                            &inner_jt,
                            ind,
                            table,
                            depth + 1,
                        )?;
                        writeln!(out, "{ind}{jt} {var} = new {jt}(__tv{depth});")
                            .map_err(fmt_err)?;
                    } else {
                        // typedef to an array (e.g. Matrix3 = long[3][3]):
                        // allocate + fill then wrap.
                        let elem_jt = java_value_type(underlying, table);
                        let brackets: String =
                            array_dims.iter().map(|d| format!("[{d}]")).collect();
                        let empty: String = array_dims.iter().map(|_| "[]").collect();
                        emit_array_alloc(
                            out,
                            ind,
                            &elem_jt,
                            &empty,
                            &brackets,
                            &format!("__ta{depth}"),
                        )?;
                        emit_array_fill(
                            out,
                            underlying,
                            array_dims,
                            &format!("__ta{depth}"),
                            ind,
                            table,
                            depth + 1,
                        )?;
                        writeln!(out, "{ind}{jt} {var} = new {jt}(__ta{depth});")
                            .map_err(fmt_err)?;
                    }
                }
                _ => {
                    // Nested struct/union: decode in-place from the shared
                    // reader. `decodeFrom` reads its own DHEADER iff the nested
                    // type is @appendable/@mutable; a @final nested type carries
                    // none (Bug XW — the old readDelimitedFrame assumed a length
                    // prefix that @final types do not emit).
                    writeln!(out, "{ind}{jt} {var} = {short}TypeSupport.decodeFrom(r);")
                        .map_err(fmt_err)?;
                }
            }
        }
        _ => {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: "typesupport-decode for this TypeSpec".into(),
                context: None,
            });
        }
    }
    Ok(())
}

/// The Java declared type for a *single value* of `ts` (non-array). Mirrors the
/// POJO field types from `emitter::typespec_to_java`, but resolves enums/
/// typedefs to their wrapper class names.
fn java_value_type(ts: &TypeSpec, table: &TypeTable) -> String {
    match ts {
        TypeSpec::Primitive(p) => crate::type_map::primitive_to_java(*p).to_string(),
        TypeSpec::String(_) => "String".into(),
        TypeSpec::Scoped(s) => scoped_to_short(s),
        TypeSpec::Sequence(seq) => {
            format!("java.util.List<{}>", boxed_for_seq_elem(&seq.elem, table))
        }
        TypeSpec::Map(map) => format!(
            "java.util.Map<{}, {}>",
            boxed_for_seq_elem(&map.key, table),
            boxed_for_seq_elem(&map.value, table)
        ),
        // fixed<P,S> -> BigDecimal (matches the struct field type).
        TypeSpec::Fixed(_) => "java.math.BigDecimal".into(),
        _ => "Object".into(),
    }
}

fn read_expr_for_typespec(ts: &TypeSpec) -> Result<String, JavaGenError> {
    Ok(match ts {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean => "r.readBoolean()".into(),
            PrimitiveType::Octet => "r.readOctet()".into(),
            PrimitiveType::Char => "r.readChar()".into(),
            PrimitiveType::WideChar => "r.readWChar()".into(),
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short | IntegerType::Int16 => "r.readInt16()".into(),
                IntegerType::UShort | IntegerType::UInt16 => "r.readUInt16()".into(),
                IntegerType::Long | IntegerType::Int32 => "r.readInt32()".into(),
                IntegerType::ULong | IntegerType::UInt32 => "r.readUInt32()".into(),
                IntegerType::LongLong | IntegerType::Int64 => "r.readInt64()".into(),
                IntegerType::ULongLong | IntegerType::UInt64 => "r.readUInt64()".into(),
                IntegerType::Int8 => "r.readOctet()".into(),
                IntegerType::UInt8 => "(short) r.readUInt8()".into(),
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => "r.readFloat32()".into(),
                FloatingType::Double => "r.readFloat64()".into(),
                // See `emit_typespec_encode`: no 16-byte float in Java/runtime.
                FloatingType::LongDouble => return Err(long_double_unsupported()),
            },
        },
        // Wide strings are decoded inline by `emit_read_into` (byte-length
        // prefixed UTF-16LE per CANONICAL.md), never through this single-expr path.
        TypeSpec::String(_) => "r.readString()".into(),
        TypeSpec::Fixed(f) => {
            // fixed<P,S>: read (P+2)/2 packed-BCD octets back into a BigDecimal.
            let p = crate::emitter::const_expr_to_java(&f.digits);
            let s = crate::emitter::const_expr_to_java(&f.scale);
            format!("r.readFixedBcd({p}, {s})")
        }
        _ => {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: "typesupport-read inline expr".into(),
                context: None,
            });
        }
    })
}

/// The boxed Java element type for a collection element / map key-value
/// (the Java generics slot). Enums and aliases resolve to their class names.
/// `table` is threaded for symmetry with the other emitters and for nested
/// recursion; the short scoped name already names the enum/typedef class.
#[allow(clippy::only_used_in_recursion)]
fn boxed_for_seq_elem(elem: &TypeSpec, table: &TypeTable) -> String {
    match elem {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean => "Boolean".into(),
            PrimitiveType::Octet => "Byte".into(),
            PrimitiveType::Char | PrimitiveType::WideChar => "Character".into(),
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short | IntegerType::Int16 => "Short".into(),
                IntegerType::UShort | IntegerType::UInt16 => "Integer".into(),
                IntegerType::Long | IntegerType::Int32 => "Integer".into(),
                IntegerType::ULong | IntegerType::UInt32 => "Long".into(),
                IntegerType::LongLong | IntegerType::Int64 => "Long".into(),
                IntegerType::ULongLong | IntegerType::UInt64 => "Long".into(),
                IntegerType::Int8 => "Byte".into(),
                IntegerType::UInt8 => "Short".into(),
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => "Float".into(),
                FloatingType::Double | FloatingType::LongDouble => "Double".into(),
            },
        },
        TypeSpec::String(_) => "String".into(),
        TypeSpec::Scoped(s) => scoped_to_short(s),
        TypeSpec::Sequence(seq) => {
            format!("java.util.List<{}>", boxed_for_seq_elem(&seq.elem, table))
        }
        TypeSpec::Map(map) => format!(
            "java.util.Map<{}, {}>",
            boxed_for_seq_elem(&map.key, table),
            boxed_for_seq_elem(&map.value, table)
        ),
        // fixed<P,S> -> BigDecimal (matches the struct field type).
        TypeSpec::Fixed(_) => "java.math.BigDecimal".into(),
        _ => "Object".into(),
    }
}

// ---------------------------------------------------------------------------
// Key-Hash extraction
// ---------------------------------------------------------------------------

fn emit_key_extraction(
    out: &mut String,
    s: &StructDef,
    ind: &str,
    table: &TypeTable,
) -> Result<(), JavaGenError> {
    let inner = format!("{ind}{ind}");
    // XTypes 1.3 §7.6.8.3.1.b: KeyHolder members in ascending member-id
    // order (explicit `@id(N)`, else positional index among the `@key`
    // members) — NOT declaration order.
    let all_members = resolved_wire_members(s, table);
    let key_members: Vec<&Member> = all_members.iter().filter(|m| member_has_key(m)).collect();
    for m in sort_members_by_id(&key_members) {
        for decl in &m.declarators {
            let name = sanitize_identifier(&decl.name().text)?;
            let getter = format!("sample.get{}()", capitalize(&name));
            emit_key_declarator_encode(
                out,
                &m.type_spec,
                &array_dims(decl),
                &getter,
                &inner,
                table,
                0,
            )?;
        }
    }
    Ok(())
}

/// Sorts `members` into ascending member-id order (explicit `@id(N)`, else
/// positional index within `members`) — XTypes 1.3 §7.6.8.3.1.b. Mirrors
/// `idl-rust`'s `emit_key_holder_be` fallback convention (the index is taken
/// within the already-filtered `@key` list, matching the cross-vendor-
/// validated reference), and the same convention this file already uses for
/// `@mutable` EMHEADER auto-id assignment (`member_explicit_id`).
fn sort_members_by_id<'a>(members: &[&'a Member]) -> Vec<&'a Member> {
    let mut ordered: Vec<(u32, &Member)> = members
        .iter()
        .enumerate()
        .map(|(idx, m)| {
            // KeyHash ordering: explicit `@id(N)` else positional index (this
            // path has no struct-level `@autoid` context — see P0-3 limits).
            let id = lower_or_empty(&m.annotations)
                .builtins
                .iter()
                .find_map(|b| match b {
                    BuiltinAnnotation::Id(n) => Some(*n),
                    _ => None,
                })
                .unwrap_or(idx as u32);
            (id, *m)
        })
        .collect();
    ordered.sort_by_key(|(id, _)| *id);
    ordered.into_iter().map(|(_, m)| m).collect()
}

/// KeyHash-specific declarator writer: identical to `emit_declarator_encode`
/// for array dims (out of scope — the investigation found sequence/array
/// `@key` shapes already correct via the generic per-field encoder), but at
/// the scalar leaf delegates to `emit_key_typespec_encode` instead of
/// `emit_typespec_encode`, so a nested `@key` struct expands to its own
/// `@key` subset instead of being fully encoded.
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn emit_key_declarator_encode(
    out: &mut String,
    ts: &TypeSpec,
    dims: &[String],
    expr: &str,
    ind: &str,
    table: &TypeTable,
    depth: usize,
) -> Result<(), JavaGenError> {
    if dims.is_empty() {
        return emit_key_typespec_encode(out, ts, expr, ind, table, depth);
    }
    let dheader = array_dim_needs_dheader(ts, dims, depth, table);
    let dhv = format!("__adh{depth}");
    let (body_ind, dheader) = if dheader {
        writeln!(out, "{ind}{{").map_err(fmt_err)?;
        let bi = format!("{ind}    ");
        writeln!(out, "{bi}int {dhv} = w.beginAppendable();").map_err(fmt_err)?;
        (bi, true)
    } else {
        (ind.to_string(), false)
    };
    let iv = format!("__ai{depth}");
    let elem = format!("{expr}[{iv}]");
    let size = &dims[0];
    writeln!(
        out,
        "{body_ind}for (int {iv} = 0; {iv} < {size}; {iv}++) {{"
    )
    .map_err(fmt_err)?;
    emit_key_declarator_encode(
        out,
        ts,
        &dims[1..],
        &elem,
        &format!("{body_ind}    "),
        table,
        depth + 1,
    )?;
    writeln!(out, "{body_ind}}}").map_err(fmt_err)?;
    if dheader {
        writeln!(out, "{body_ind}w.endDelimited({dhv});").map_err(fmt_err)?;
        writeln!(out, "{ind}}}").map_err(fmt_err)?;
    }
    Ok(())
}

/// KeyHash-specific value writer (XTypes 1.3 §7.6.8). For a `Scoped`
/// reference that resolves (after dealiasing any typedef chain — see the
/// `Typedef` arm below) to a nested struct, expands to that struct's own
/// `@key` subset (or ALL its members if it declares none — XTypes 1.3
/// §7.6.8: a keyless aggregate is keyed in full), in member-id order,
/// instead of delegating to `<Type>TypeSupport.INSTANCE.encode(...)` (which
/// — correct for normal, non-key encoding — writes the WHOLE nested struct).
/// Every other shape (primitive, string, sequence, map, enum, bitmask,
/// bitset, union, fixed) is unchanged from — and delegates verbatim to —
/// the general `emit_typespec_encode`: the investigation found those
/// already correct via the existing generic per-field encoder.
/// zerodds-lint: recursion-depth 16
fn emit_key_typespec_encode(
    out: &mut String,
    ts: &TypeSpec,
    expr: &str,
    ind: &str,
    table: &TypeTable,
    depth: usize,
) -> Result<(), JavaGenError> {
    if let TypeSpec::Scoped(s) = ts {
        let short = scoped_to_short(s);
        match resolve_scoped(table, &short) {
            // A `@key` member whose declared type is a typedef must dealias
            // BEFORE the nested-struct check below can even see the struct:
            // `typedef Inner InnerAlias; struct Outer { @key InnerAlias i; }`
            // previously fell straight through to the generic
            // `emit_typespec_encode` fallback (this match only recognised
            // `ResolvedKind::Struct` directly), which writes the WHOLE
            // nested struct via `InnerTypeSupport.INSTANCE.encode(...)`
            // instead of just its own `@key` subset — the same class of
            // over-inclusion bug as FINDING #20. Unwrap the Java typedef
            // wrapper's `.value()` and recurse on the underlying type/dims,
            // exactly like the general encoder's Bug J #65(4) arm — then
            // the recursive `emit_key_declarator_encode` call re-enters this
            // function (for a scalar underlying type) and the `Struct` arm
            // below fires on the dealiased type.
            Some(ResolvedKind::Typedef {
                underlying,
                array_dims,
            }) => {
                let inner = format!("({expr}).value()");
                return emit_key_declarator_encode(
                    out,
                    underlying,
                    array_dims,
                    &inner,
                    ind,
                    table,
                    depth + 1,
                );
            }
            Some(ResolvedKind::Struct { def, .. }) => {
                let def = def.clone();
                let nested_keys: Vec<&Member> =
                    def.members.iter().filter(|m| member_has_key(m)).collect();
                let effective: Vec<&Member> = if nested_keys.is_empty() {
                    def.members.iter().collect()
                } else {
                    nested_keys
                };
                for m in sort_members_by_id(&effective) {
                    for decl in &m.declarators {
                        let name = sanitize_identifier(&decl.name().text)?;
                        // Arrays inside a nested-struct key are out of the
                        // proof scope; reject explicitly rather than
                        // silently DHEADER-framing a per-element encode of
                        // the array (which `emit_key_declarator_encode`
                        // would otherwise do unchanged, mixing DHEADER
                        // framing into a KeyHolder that must always be the
                        // FLAT concatenation of key bytes — XTypes 1.3
                        // §7.6.8). Matches every other backend's identical
                        // rejection of this shape (e.g. `idl-rust`'s
                        // `emit_key_field_write`, `idl-cpp`'s
                        // `emit_key_value_write`, `idl-go`'s
                        // `emit_key_struct_member`). A top-level `@key`
                        // array field of this struct's OWN declarator is
                        // unaffected — those dims are consumed by
                        // `emit_key_declarator_encode` before this Struct
                        // arm is ever reached.
                        if matches!(decl, Declarator::Array(_)) {
                            return Err(JavaGenError::UnsupportedConstruct {
                                construct: "array @key field inside a nested-struct key"
                                    .to_string(),
                                context: Some(name),
                            });
                        }
                        let getter = format!("({expr}).get{}()", capitalize(&name));
                        emit_key_declarator_encode(
                            out,
                            &m.type_spec,
                            &array_dims(decl),
                            &getter,
                            ind,
                            table,
                            depth + 1,
                        )?;
                    }
                }
                return Ok(());
            }
            _ => {}
        }
    }
    emit_typespec_encode(out, ts, expr, ind, table, depth)
}

/// Static codec helpers shared by the generated TypeSupport: skip an unknown
/// mutable member by its length-code, and lift a DHEADER-delimited nested
/// aggregate (`[size:uint32-LE][body]`) back out as a self-contained byte[]
/// frame so the nested `<Type>TypeSupport.decode` reads exactly its own bytes
/// (Bug J #65(2): the old code passed `r.readBytes(r.remaining())` and ate the
/// rest of the stream).
fn emit_codec_helpers(out: &mut String, ind: &str) -> Result<(), JavaGenError> {
    writeln!(
        out,
        "{ind}private static void skipByLc(org.zerodds.cdr.Xcdr2Reader r, int lc) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}switch (lc) {{").map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}{ind}case 0: r.skip(1); break;").map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}{ind}case 1: r.skip(2); break;").map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}{ind}case 2: r.skip(4); break;").map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}{ind}case 3: r.skip(8); break;").map_err(fmt_err)?;
    // LC 4..7 carry an explicit NEXTINT byte-length.
    writeln!(
        out,
        "{ind}{ind}{ind}default: {{ int __n = r.readNextInt(); r.skip(__n); break; }}"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}}}").map_err(fmt_err)?;
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    writeln!(
        out,
        "{ind}private static byte[] readDelimitedFrame(org.zerodds.cdr.Xcdr2Reader r) {{"
    )
    .map_err(fmt_err)?;
    // `readDHeader` aligns to 4 *before* reading the uint32, so when this
    // frame follows a variable-length value the cursor may skip 1-3 pad bytes.
    // Those pad bytes are NOT part of the frame: the reconstructed frame is
    // always exactly [4-byte DHEADER][body]. (The old code used
    // `position()-startPos` for the header width, which folded the alignment
    // padding into the frame and shifted the body — corrupting every nested
    // aggregate that sat at a non-4 offset, e.g. a `sequence<union>` element
    // after a string, or an `@optional` struct after a variable-length member.)
    writeln!(out, "{ind}{ind}int __sz = r.readDHeader();").map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}byte[] __body = r.readBytes(__sz);").map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}byte[] __frame = new byte[4 + __sz];").map_err(fmt_err)?;
    // Re-emit the DHEADER (uint32-LE) followed by the body so the nested
    // decoder reads its own self-describing frame.
    writeln!(
        out,
        "{ind}{ind}__frame[0] = (byte) (__sz & 0xFF); __frame[1] = (byte) ((__sz >>> 8) & 0xFF);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{ind}{ind}__frame[2] = (byte) ((__sz >>> 16) & 0xFF); __frame[3] = (byte) ((__sz >>> 24) & 0xFF);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{ind}{ind}System.arraycopy(__body, 0, __frame, 4, __sz);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{ind}{ind}return __frame;").map_err(fmt_err)?;
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn scoped_to_short(s: &zerodds_idl::ast::ScopedName) -> String {
    s.parts.last().map(|p| p.text.clone()).unwrap_or_default()
}

// Suppress warnings for the unused helper.
#[allow(dead_code)]
fn _unused(p: PrimitiveType) -> &'static str {
    primitive_to_java(p)
}

#[allow(dead_code)]
fn _unused_decl(_d: &Declarator) {}
