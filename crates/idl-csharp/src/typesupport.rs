// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR2 TypeSupport emission for the C# codegen.
//!
//! Spec: `zerodds-xcdr2-csharp-1.0` §3 / §4 / §5 / §6 / §7
//! + `zerodds-xcdr2-bindings-conformance-1.0` §6 (V-1..V-12).
//!
//! For each IDL `struct`, this module emits a `*TypeSupport` class that
//! implements `IDdsTopicType<T>` from `ZeroDDS.Cdr`. Encode/Decode
//! delegate to `Xcdr2Writer` / `Xcdr2Reader`; extensibility drives the
//! DHEADER/EMHEADER layout, and `@key` triggers an MD5 KeyHash via
//! `PlainCdr2BeKeyHolder`.

use std::fmt::Write;

use zerodds_idl::ast::{
    ConstExpr, Declarator, Definition, IntegerType, PrimitiveType, ScopedName, Specification,
    StructDef, TypeSpec,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations, lower_type_annotations,
};

use crate::error::CsGenError;
use crate::keywords::escape_identifier;

/// Context info: the current module path for type-name emission and the
/// IDL struct name (without modules).
pub(crate) struct TsEmitContext<'a> {
    pub module_path: &'a [String],
    pub indent: &'a str,
    pub inner_indent: &'a str,
    pub deeper_indent: &'a str,
}

/// Returns `<Module1>::<Module2>::<Struct>` per spec §5 (type-name convention).
pub(crate) fn make_dds_type_name(module_path: &[String], struct_name: &str) -> String {
    if module_path.is_empty() {
        struct_name.to_string()
    } else {
        format!("{}::{}", module_path.join("::"), struct_name)
    }
}

/// Member info: computed wire characteristics of a struct member.
struct MemberInfo {
    /// Property name in PascalCase (matches the one in `emit_struct_member_property`).
    cs_prop: String,
    /// IDL source type (for Encode/Decode method selection).
    type_spec: TypeSpec,
    /// True if `@key`.
    is_key: bool,
    /// True if `@optional`.
    is_optional: bool,
    /// True if `@must_understand`.
    must_understand: bool,
    /// `@id(N)` if explicitly set; otherwise None (auto-index in mutable).
    explicit_id: Option<u32>,
    /// Fixed-array dimensions from the declarator (`long v[3][4]` → `["3","4"]`).
    /// Empty for a plain (`Declarator::Simple`) member. Each entry is a C#
    /// constant expression for the fixed element count; arrays carry no length
    /// prefix on the wire (XCDR2 §7.4.3.3 — fixed-count, no DHEADER).
    array_dims: Vec<String>,
}

fn pascal_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = true;
    for c in s.chars() {
        if c == '_' {
            upper_next = true;
            continue;
        }
        if upper_next {
            for u in c.to_uppercase() {
                out.push(u);
            }
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        return s.to_string();
    }
    out
}

fn collect_member_info(s: &StructDef) -> Vec<MemberInfo> {
    let mut out = Vec::new();
    for m in &s.members {
        let mut is_key = false;
        let mut is_optional = false;
        let mut must_understand = false;
        let mut explicit_id: Option<u32> = None;
        if let Ok(lowered) = lower_annotations(&m.annotations) {
            for b in &lowered.builtins {
                match b {
                    BuiltinAnnotation::Key => is_key = true,
                    BuiltinAnnotation::Optional => is_optional = true,
                    BuiltinAnnotation::MustUnderstand => must_understand = true,
                    BuiltinAnnotation::Id(n) => explicit_id = Some(*n),
                    _ => {}
                }
            }
        }
        for decl in &m.declarators {
            let raw = &decl.name().text;
            let cs_prop = {
                let pas = pascal_case(raw);
                let escaped = escape_identifier(raw).unwrap_or_else(|_| raw.clone());
                if escaped == pas { escaped } else { pas }
            };
            let array_dims = match decl {
                Declarator::Simple(_) => Vec::new(),
                Declarator::Array(ad) => ad
                    .sizes
                    .iter()
                    .map(crate::emitter::const_expr_to_cs)
                    .collect(),
            };
            out.push(MemberInfo {
                cs_prop,
                type_spec: m.type_spec.clone(),
                is_key,
                is_optional,
                must_understand,
                explicit_id,
                array_dims,
            });
        }
        let _ = m; // silence unused on tail
    }
    out
}

/// Returns the IDL extensibility kind from a type's annotations.
///
/// Default is **Final** when no `@final`/`@appendable`/`@mutable` annotation is
/// present. This matches the canonical zerodds-cdr / idl-rust reference
/// (`crates/idl-rust/src/annotations.rs` returns `StructExtensibility::Final`
/// by default) and the XCDR2 layout in `internal/idl-codegen/xcdr2-canonical-
/// layout.md`: an unannotated nested aggregate (e.g. `Sample`, `Reading`) is
/// `@final` → NO per-element DHEADER (XTypes 1.3 §7.4.3.5.3 rule (17)/(18)
/// FSTRUCT_TYPE, rule (26) FUNION_TYPE). Only types carrying an explicit
/// `@appendable`/`@mutable` get a DHEADER frame (rule (30)/(21)).
fn type_extensibility(annotations: &[zerodds_idl::ast::Annotation]) -> ExtensibilityKind {
    if let Ok(lowered) = lower_type_annotations(annotations) {
        for b in &lowered.builtins {
            if let Some(k) = match b {
                BuiltinAnnotation::Final => Some(ExtensibilityKind::Final),
                BuiltinAnnotation::Appendable => Some(ExtensibilityKind::Appendable),
                BuiltinAnnotation::Mutable => Some(ExtensibilityKind::Mutable),
                BuiltinAnnotation::Extensibility(k) => Some(*k),
                _ => None,
            } {
                return k;
            }
        }
    }
    // SX2: spec default for an unannotated aggregate is APPENDABLE (§7.3.3.1).
    ExtensibilityKind::Appendable
}

fn ext_to_cs(k: ExtensibilityKind) -> &'static str {
    match k {
        ExtensibilityKind::Final => "Final",
        ExtensibilityKind::Appendable => "Appendable",
        ExtensibilityKind::Mutable => "Mutable",
    }
}

fn fmt_err(_: core::fmt::Error) -> CsGenError {
    CsGenError::Internal("string formatting failed".into())
}

/// Main entry: emits a `*TypeSupport` class for `s`.
///
/// `module_path` is the list of enclosing modules (e.g.
/// `["Outer", "Inner"]` for `module Outer { module Inner { struct S }}`).
/// `true` if the XCDR2 codec can be generated for this type. `map`/`fixed`/`any`
/// have no wire codec in idl-csharp yet (cf. idl-java `typespec_supported`), so a
/// struct with such a member gets its data type but **no** TypeSupport — instead
/// of a codec that throws `XcdrException` at runtime.
/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn typespec_xcdr2_codecable(t: &zerodds_idl::ast::TypeSpec) -> bool {
    use zerodds_idl::ast::TypeSpec;
    match t {
        // `map<K,V>` now has an XCDR2 codec (CS-cluster #2): codecable iff both
        // key and value are. `fixed`/`any` still have no wire codec.
        TypeSpec::Map(m) => typespec_xcdr2_codecable(&m.key) && typespec_xcdr2_codecable(&m.value),
        // fixed<P,S> now has a wire codec (CORBA-BCD via WriteFixedBcd/
        // ReadFixedBcd); `any` still has none.
        TypeSpec::Fixed(_) => true,
        TypeSpec::Any => false,
        TypeSpec::Sequence(s) => typespec_xcdr2_codecable(&s.elem),
        TypeSpec::Scoped(s) => match lookup_scoped_kind(s) {
            // Unions now emit a real codec (CS-cluster #3) — no longer gated.
            Some(ScopedKind::Union) => true,
            // Typedef-to-primitive/string/struct/enum AND
            // typedef-to-aggregate (sequence/map) are codecable: encode unwraps
            // `.Value` and recurses; decode decodes the inner aggregate into a
            // temp and re-wraps into the alias record (CS2). Only
            // typedef-to-ARRAY (`typedef long M[3];`) remains unhandled by the
            // unwrap path — gate the containing struct so it gets the data type
            // but no broken codec.
            Some(ScopedKind::Typedef { inner, is_array }) => {
                !is_array && typespec_xcdr2_codecable(&inner)
            }
            _ => true,
        },
        _ => true,
    }
}

/// Codec kind of a scoped type reference (for the nested XCDR2 dispatch).
#[derive(Clone)]
enum ScopedKind {
    /// struct — encoded via `<Name>TypeSupport.Instance.EncodeInto`/`DecodeFrom`.
    /// Carries the struct's extensibility so a `@mutable` member referencing it
    /// can decide its EMHEADER length code: an `@appendable`/`@mutable` nested
    /// struct self-delimits with a leading DHEADER → LC5; a `@final` one has no
    /// DHEADER → LC4 (see `member_body_has_leading_dheader`).
    Struct { ext: ExtensibilityKind },
    /// union — no XCDR2 codec yet; a struct containing one is gated.
    Union,
    /// enum — SIGNED ordinal whose wire width (1/2/4 bytes) is selected by
    /// `@bit_bound` (XTypes §7.4.5.1: N≤8 → 1, N≤16 → 2, else 4). Cyclone
    /// honours this; a fixed int32 broke `@bit_bound` interop.
    Enum { holder_bytes: u8 },
    /// bitmask — the holder is an unsigned integer whose width matches the
    /// cross-vendor-validated `zerodds-cdr` reference: derived from the number
    /// of bit values (`crates/idl-rust/src/bitset_emit.rs`: `bitset_storage_type
    /// (values.len())`), ≤8 → 1 byte, ≤16 → 2, ≤32 → 4, else 8. The C# member is
    /// the `[Flags] enum Name`; encode/decode write/read that holder width.
    /// XTypes 1.3 §7.4.3.4 (bitmask is serialized as its bit-bound integer).
    Bitmask { holder_bytes: u32 },
    /// bitset — the holder is an unsigned integer whose width matches the total
    /// declared bitfield width (`crates/idl-rust/src/bitset_emit.rs`:
    /// `bitset_storage_type(total_width)`). The C# member is the `struct Name`
    /// carrying a `ulong Value`; encode writes `(holder)Value`, decode reads it
    /// back into `Value`. XTypes 1.3 §7.4.3.4.
    Bitset { holder_bytes: u32 },
    /// typedef — resolves to the aliased type. `is_array` is true when the
    /// typedef declarator itself carries array dimensions (`typedef long M[3][3]`).
    Typedef { inner: TypeSpec, is_array: bool },
}

/// Holder byte width for a bitmask/bitset, matching the `zerodds-cdr` reference
/// storage-type buckets (`crates/idl-rust/src/bitset_emit.rs`
/// `bitset_storage_type`): ≤8 bits → 1 byte, ≤16 → 2, ≤32 → 4, else 8.
fn holder_bytes_for_bits(bits: u32) -> u32 {
    match bits {
        0..=8 => 1,
        9..=16 => 2,
        17..=32 => 4,
        _ => 8,
    }
}

std::thread_local! {
    /// Type-name → kind, built once per `generate_csharp` call (keyed by simple
    /// name). Lets the codec tell a nested struct from an enum/union and resolve
    /// typedefs. Without it every scoped member was encoded as empty bytes and
    /// decoded as `default!` (silent corruption).
    static TYPE_REG: core::cell::RefCell<std::collections::BTreeMap<String, ScopedKind>> =
        const { core::cell::RefCell::new(std::collections::BTreeMap::new()) };

    /// Type-name → full `StructDef`, built alongside [`TYPE_REG`]. Lets a
    /// nested-struct `@key` member (`emit_key_encode_scoped`) recurse into
    /// the nested struct's own members to build the correct KeyHolder bytes.
    static STRUCT_REG: core::cell::RefCell<std::collections::BTreeMap<String, StructDef>> =
        const { core::cell::RefCell::new(std::collections::BTreeMap::new()) };

    /// Monotonic counter for array-DHEADER scope variables (`__arrdhN`). The
    /// array recursion depth alone is NOT unique across sibling members (two
    /// array members both open their outer DHEADER at depth 0, declaring
    /// `__arrdh0` twice in the same method body — a C# CS0128 error). This
    /// supplies a fresh suffix per `using`/`BeginDHeader` block.
    static ARRDH_SEQ: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

/// Returns a fresh unique id for an array DHEADER scope variable.
fn next_arrdh_id() -> u32 {
    ARRDH_SEQ.with(|c| {
        let v = c.get();
        c.set(v.wrapping_add(1));
        v
    })
}

/// Populates [`TYPE_REG`] from the spec (recursing modules).
pub(crate) fn build_type_registry(spec: &Specification) {
    use zerodds_idl::ast::{ConstrTypeDecl, StructDcl, TypeDecl, UnionDcl};
    /// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
    fn walk(defs: &[Definition], reg: &mut std::collections::BTreeMap<String, ScopedKind>) {
        for def in defs {
            match def {
                Definition::Module(m) => walk(&m.definitions, reg),
                Definition::Type(TypeDecl::Constr(c)) => match c {
                    ConstrTypeDecl::Struct(StructDcl::Def(s)) => {
                        let ext = type_extensibility(&s.annotations);
                        reg.insert(s.name.text.clone(), ScopedKind::Struct { ext });
                        STRUCT_REG
                            .with(|r| r.borrow_mut().insert(s.name.text.clone(), s.clone()));
                    }
                    ConstrTypeDecl::Union(UnionDcl::Def(u)) => {
                        reg.insert(u.name.text.clone(), ScopedKind::Union);
                    }
                    ConstrTypeDecl::Enum(e) => {
                        let eb = crate::bitset::extract_int_annotation(&e.annotations, "bit_bound")
                            .filter(|&v| (1..=32).contains(&v))
                            .unwrap_or(32);
                        let holder_bytes: u8 = if eb <= 8 {
                            1
                        } else if eb <= 16 {
                            2
                        } else {
                            4
                        };
                        reg.insert(e.name.text.clone(), ScopedKind::Enum { holder_bytes });
                    }
                    ConstrTypeDecl::Bitmask(b) => {
                        // Holder width = effective `@bit_bound`, NOT the declared
                        // flag count. XTypes 1.3 §7.3.1.2.1.1: a bitmask's DEFAULT
                        // bit_bound is 32 → a uint32 holder (4 bytes) on the wire
                        // even when it declares only a few flags. Mirrors the rust
                        // reference `annotations::bitmask_bit_bound` (default 32).
                        // (The earlier `values.len()` produced a 1-byte holder for
                        // a 3-flag bitmask, diverging from the cross-vendor wire.)
                        let bit_bound =
                            crate::bitset::extract_int_annotation(&b.annotations, "bit_bound")
                                .unwrap_or(32);
                        let holder_bytes = holder_bytes_for_bits(bit_bound);
                        reg.insert(b.name.text.clone(), ScopedKind::Bitmask { holder_bytes });
                    }
                    ConstrTypeDecl::Bitset(b) => {
                        // Holder width = total declared bitfield width.
                        let mut total: u32 = 0;
                        for f in &b.bitfields {
                            if let zerodds_idl::ast::ConstExpr::Literal(l) = &f.spec.width {
                                if matches!(l.kind, zerodds_idl::ast::LiteralKind::Integer) {
                                    total = total.saturating_add(l.raw.parse::<u32>().unwrap_or(0));
                                }
                            }
                        }
                        let holder_bytes = holder_bytes_for_bits(total);
                        reg.insert(b.name.text.clone(), ScopedKind::Bitset { holder_bytes });
                    }
                    _ => {}
                },
                Definition::Type(TypeDecl::Typedef(t)) => {
                    for d in &t.declarators {
                        let is_array = matches!(d, Declarator::Array(_));
                        reg.insert(
                            d.name().text.clone(),
                            ScopedKind::Typedef {
                                inner: t.type_spec.clone(),
                                is_array,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
    }
    let mut reg = std::collections::BTreeMap::new();
    walk(&spec.definitions, &mut reg);
    TYPE_REG.with(|r| *r.borrow_mut() = reg);
}

fn lookup_scoped_kind(s: &ScopedName) -> Option<ScopedKind> {
    let key = &s.parts.last()?.text;
    TYPE_REG.with(|r| r.borrow().get(key).cloned())
}

/// If `s` resolves to a non-array typedef whose aliased type is an AGGREGATE
/// (`sequence` or `map`), returns the aliased `TypeSpec`; otherwise `None`.
/// Used by the decode path to special-case typedef-to-aggregate members, whose
/// inner decode is statement-based (a loop) and so cannot ride the single-
/// expression `decode_simple_expr` unwrap path (CS2).
fn typedef_to_aggregate_inner(s: &ScopedName) -> Option<TypeSpec> {
    match lookup_scoped_kind(s) {
        Some(ScopedKind::Typedef {
            inner,
            is_array: false,
        }) if matches!(inner, TypeSpec::Sequence(_) | TypeSpec::Map(_)) => Some(inner),
        _ => None,
    }
}

/// Dotted C# reference for a scoped name (`Module.Sub.Name`), each part escaped.
fn scoped_dotted_cs(s: &ScopedName) -> String {
    s.parts
        .iter()
        .map(|p| escape_identifier(&p.text).unwrap_or_else(|_| p.text.clone()))
        .collect::<Vec<_>>()
        .join(".")
}

/// Whether a struct's XCDR2 TypeSupport can be generated (all members codecable).
pub(crate) fn struct_xcdr2_codecable(s: &StructDef) -> bool {
    s.members
        .iter()
        .all(|m| typespec_xcdr2_codecable(&m.type_spec))
}

/// Whether a union's XCDR2 TypeSupport can be generated (all case elements
/// codecable). Array-typed union branches are not yet supported by the union
/// codec — gate them out so the emitter still produces the data record.
pub(crate) fn union_xcdr2_codecable(u: &zerodds_idl::ast::UnionDef) -> bool {
    u.cases.iter().all(|c| {
        matches!(c.element.declarator, Declarator::Simple(_))
            && typespec_xcdr2_codecable(&c.element.type_spec)
    })
}

/// C# type + reader/writer pair for a union discriminator (switch type).
fn switch_type_codec(s: &zerodds_idl::ast::SwitchTypeSpec) -> (String, &'static str, &'static str) {
    use zerodds_idl::ast::{IntegerType as I, SwitchTypeSpec as S};
    match s {
        S::Boolean => ("bool".into(), "w.WriteBool", "r.ReadBool()"),
        S::Octet => ("byte".into(), "w.WriteOctet", "r.ReadOctet()"),
        S::Char => ("byte".into(), "w.WriteOctet", "r.ReadOctet()"),
        S::Integer(i) => match i {
            I::Short | I::Int16 => ("short".into(), "w.WriteInt16", "r.ReadInt16()"),
            I::UShort | I::UInt16 => ("ushort".into(), "w.WriteUInt16", "r.ReadUInt16()"),
            I::Long | I::Int32 => ("int".into(), "w.WriteInt32", "r.ReadInt32()"),
            I::ULong | I::UInt32 => ("uint".into(), "w.WriteUInt32", "r.ReadUInt32()"),
            I::LongLong | I::Int64 => ("long".into(), "w.WriteInt64", "r.ReadInt64()"),
            I::ULongLong | I::UInt64 => ("ulong".into(), "w.WriteUInt64", "r.ReadUInt64()"),
            I::Int8 => ("sbyte".into(), "w.WriteOctet", "(sbyte)r.ReadOctet()"),
            I::UInt8 => ("byte".into(), "w.WriteOctet", "r.ReadOctet()"),
        },
        // enum discriminator → int32 ordinal; the C# property type is the enum.
        S::Scoped(sc) => (scoped_dotted_cs(sc), "w.WriteInt32", "r.ReadInt32()"),
    }
}

/// Emits a real `*TypeSupport` class for an IDL `union` (CS-cluster #3).
///
/// Wire layout (XCDR2 §7.4.2; appendable default): a DHEADER frames
/// `[discriminator][selected-member]`. The discriminator selects which case
/// element is present; encode/decode dispatch on it. The data record carries
/// `Discriminator` + `object? Value`, so the codec boxes/unboxes `Value` to
/// the per-case element type.
pub(crate) fn emit_union_typesupport_class(
    out: &mut String,
    ctx: &TsEmitContext<'_>,
    u: &zerodds_idl::ast::UnionDef,
) -> Result<(), CsGenError> {
    let union_name = escape_identifier(&u.name.text)?;
    let ts_name = format!("{union_name}TypeSupport");
    let dds_type_name = make_dds_type_name(ctx.module_path, &u.name.text);
    let (disc_cs, disc_write, disc_read) = switch_type_codec(&u.switch_type);
    let is_enum_disc = matches!(u.switch_type, zerodds_idl::ast::SwitchTypeSpec::Scoped(_));
    let ext = type_extensibility(&u.annotations);

    let ind = ctx.indent;
    let inner = ctx.inner_indent;
    let deeper = ctx.deeper_indent;
    let d4 = format!("{deeper}    ");
    let d5 = format!("{deeper}        ");

    writeln!(
        out,
        "{ind}public sealed class {ts_name} : IDdsTopicType<{union_name}>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{ind}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public static readonly {ts_name} Instance = new();"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "{inner}public string TypeName => \"{dds_type_name}\";").map_err(fmt_err)?;
    writeln!(out, "{inner}public bool IsKeyed => false;").map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public ExtensibilityKind Extensibility => ExtensibilityKind.{};",
        ext_to_cs(ext)
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    writeln!(
        out,
        "{inner}public byte[] Encode({union_name} sample) => Encode(sample, EndianMode.LittleEndian);"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public byte[] Encode({union_name} sample, EndianMode endian) => Encode(sample, endian, 1);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public byte[] Encode({union_name} sample, EndianMode endian, int representation)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{deeper}int __maxAlign = representation == 0 ? Xcdr2Writer.Xcdr1MaxAlignmentValue : 4;"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{deeper}var w = new Xcdr2Writer(endian, __maxAlign);").map_err(fmt_err)?;
    writeln!(out, "{deeper}EncodeInto(w, sample);").map_err(fmt_err)?;
    writeln!(out, "{deeper}return w.ToArray();").map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // EncodeInto: [DHEADER?] { discriminator; selected member }.
    // @final union (XTypes 1.3 §7.4.3.5.3 rule (26) FUNION_TYPE): disc +
    // member, NO DHEADER. @appendable/@mutable union (rule (21)): DHEADER frame.
    writeln!(
        out,
        "{inner}public void EncodeInto(Xcdr2Writer w, {union_name} sample)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    let union_final = matches!(ext, ExtensibilityKind::Final);
    let (body_disc, body_sw, body_case) = if union_final {
        (deeper, deeper, d4.as_str())
    } else {
        let begin = match ext {
            ExtensibilityKind::Mutable => "BeginMutable",
            _ => "BeginAppendable",
        };
        writeln!(out, "{deeper}using (var __scope = w.{begin}())").map_err(fmt_err)?;
        writeln!(out, "{deeper}{{").map_err(fmt_err)?;
        (d4.as_str(), d4.as_str(), d5.as_str())
    };
    if is_enum_disc {
        writeln!(out, "{body_disc}{disc_write}((int)sample.Discriminator);").map_err(fmt_err)?;
    } else {
        writeln!(out, "{body_disc}{disc_write}(sample.Discriminator);").map_err(fmt_err)?;
    }
    writeln!(out, "{body_sw}switch (sample.Discriminator)").map_err(fmt_err)?;
    writeln!(out, "{body_sw}{{").map_err(fmt_err)?;
    emit_union_encode_cases(out, body_case, u, &disc_cs, is_enum_disc)?;
    writeln!(out, "{body_sw}}}").map_err(fmt_err)?;
    if !union_final {
        writeln!(out, "{deeper}}}").map_err(fmt_err)?;
    }
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Decode(bytes) — default LE overload + explicit endian overload (big-endian
    // payload as read from the encapsulation header by the DCPS layer).
    writeln!(
        out,
        "{inner}public {union_name} Decode(ReadOnlySpan<byte> bytes) => Decode(bytes, EndianMode.LittleEndian, 1);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public {union_name} Decode(ReadOnlySpan<byte> bytes, EndianMode endian) => Decode(bytes, endian, 1);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public {union_name} Decode(ReadOnlySpan<byte> bytes, EndianMode endian, int representation)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{deeper}int __maxAlign = representation == 0 ? Xcdr2Reader.Xcdr1MaxAlignmentValue : 4;"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{deeper}var r = new Xcdr2Reader(bytes, endian, __maxAlign);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{deeper}return DecodeFrom(ref r);").map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // DecodeFrom(ref r).
    writeln!(
        out,
        "{inner}public {union_name} DecodeFrom(ref Xcdr2Reader r)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    if !union_final {
        writeln!(out, "{deeper}var __scope = r.BeginDHeader();").map_err(fmt_err)?;
    }
    if is_enum_disc {
        writeln!(out, "{deeper}{disc_cs} __disc = ({disc_cs})({disc_read});").map_err(fmt_err)?;
    } else {
        writeln!(out, "{deeper}{disc_cs} __disc = {disc_read};").map_err(fmt_err)?;
    }
    writeln!(out, "{deeper}object? __val = null;").map_err(fmt_err)?;
    writeln!(out, "{deeper}switch (__disc)").map_err(fmt_err)?;
    writeln!(out, "{deeper}{{").map_err(fmt_err)?;
    emit_union_decode_cases(out, &d4, u, &disc_cs, is_enum_disc)?;
    writeln!(out, "{deeper}}}").map_err(fmt_err)?;
    if !union_final {
        writeln!(out, "{deeper}r.EndDHeader(__scope);").map_err(fmt_err)?;
    }
    writeln!(
        out,
        "{deeper}return new {union_name} {{ Discriminator = __disc, Value = __val }};"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // KeyHash: unions are not keyed here.
    writeln!(out, "{inner}public byte[] KeyHash({union_name} sample)").map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    writeln!(out, "{deeper}return new byte[16];").map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;

    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Renders a single `case <label>:` line. For an enum discriminator, a bare
/// enumerator label (`K_A`) must be qualified to `<EnumType>.K_A`; an already
/// dotted/scoped value is left as-is.
fn write_case_label(
    out: &mut String,
    indent: &str,
    expr: &ConstExpr,
    disc_cs: &str,
    is_enum_disc: bool,
) -> Result<(), CsGenError> {
    let v = crate::emitter::const_expr_to_cs(expr);
    if is_enum_disc && !v.contains('.') {
        writeln!(out, "{indent}case {disc_cs}.{v}:").map_err(fmt_err)?;
    } else {
        writeln!(out, "{indent}case {v}:").map_err(fmt_err)?;
    }
    Ok(())
}

/// Emits the `case <label>:` arms of a union encode switch.
fn emit_union_encode_cases(
    out: &mut String,
    indent: &str,
    u: &zerodds_idl::ast::UnionDef,
    disc_cs: &str,
    is_enum_disc: bool,
) -> Result<(), CsGenError> {
    use zerodds_idl::ast::CaseLabel;
    let body = format!("{indent}    ");
    for c in &u.cases {
        let elem_ty = cs_storage_type(&c.element.type_spec);
        for label in &c.labels {
            match label {
                CaseLabel::Default => {
                    writeln!(out, "{indent}default:").map_err(fmt_err)?;
                }
                CaseLabel::Value(expr) => {
                    write_case_label(out, indent, expr, disc_cs, is_enum_disc)?;
                }
            }
        }
        // The branch value is boxed in `object? Value` → cast back to the
        // element type before encoding.
        emit_encode_value(
            out,
            &body,
            &c.element.type_spec,
            &format!("(({elem_ty})sample.Value!)"),
            0,
        )?;
        writeln!(out, "{body}break;").map_err(fmt_err)?;
    }
    Ok(())
}

/// Emits the `case <label>:` arms of a union decode switch (each assigns
/// `__val`).
fn emit_union_decode_cases(
    out: &mut String,
    indent: &str,
    u: &zerodds_idl::ast::UnionDef,
    disc_cs: &str,
    is_enum_disc: bool,
) -> Result<(), CsGenError> {
    use zerodds_idl::ast::CaseLabel;
    let body = format!("{indent}    ");
    let bb = format!("{body}    ");
    for (ci, c) in u.cases.iter().enumerate() {
        let elem_ty = cs_storage_type(&c.element.type_spec);
        for label in &c.labels {
            match label {
                CaseLabel::Default => {
                    writeln!(out, "{indent}default:").map_err(fmt_err)?;
                }
                CaseLabel::Value(expr) => {
                    write_case_label(out, indent, expr, disc_cs, is_enum_disc)?;
                }
            }
        }
        let tmp = format!("__uv{ci}");
        writeln!(out, "{body}{{").map_err(fmt_err)?;
        writeln!(out, "{bb}{elem_ty} {tmp};").map_err(fmt_err)?;
        emit_decode_assign(out, &bb, &c.element.type_spec, &tmp, 0)?;
        writeln!(out, "{bb}__val = {tmp};").map_err(fmt_err)?;
        writeln!(out, "{body}}}").map_err(fmt_err)?;
        writeln!(out, "{body}break;").map_err(fmt_err)?;
    }
    Ok(())
}

pub(crate) fn emit_typesupport_class(
    out: &mut String,
    ctx: &TsEmitContext<'_>,
    s: &StructDef,
) -> Result<(), CsGenError> {
    let struct_name = escape_identifier(&s.name.text)?;
    let ts_name = format!("{struct_name}TypeSupport");
    let ext = type_extensibility(&s.annotations);
    let members = collect_member_info(s);
    let is_keyed = members.iter().any(|m| m.is_key);
    let dds_type_name = make_dds_type_name(ctx.module_path, &s.name.text);

    let ind = ctx.indent;
    let inner = ctx.inner_indent;
    let deeper = ctx.deeper_indent;

    writeln!(
        out,
        "{ind}public sealed class {ts_name} : IDdsTopicType<{struct_name}>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{ind}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public static readonly {ts_name} Instance = new();"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "{inner}public string TypeName => \"{dds_type_name}\";").map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public bool IsKeyed => {};",
        if is_keyed { "true" } else { "false" }
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public ExtensibilityKind Extensibility => ExtensibilityKind.{};",
        ext_to_cs(ext)
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Encode(sample) - LE Default.
    writeln!(
        out,
        "{inner}public byte[] Encode({struct_name} sample) => Encode(sample, EndianMode.LittleEndian);"
    )
    .map_err(fmt_err)?;

    // Encode(sample, endian) — delegates to the representation-aware overload.
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public byte[] Encode({struct_name} sample, EndianMode endian) => Encode(sample, endian, 1);"
    )
    .map_err(fmt_err)?;
    // representation: 1 = XCDR2 (alignment cap 4), 0 = XCDR1 / classic CDR (cap 8,
    // no DHEADER, PL_CDR1 @mutable).
    writeln!(
        out,
        "{inner}public byte[] Encode({struct_name} sample, EndianMode endian, int representation)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{deeper}int __maxAlign = representation == 0 ? Xcdr2Writer.Xcdr1MaxAlignmentValue : 4;"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{deeper}var w = new Xcdr2Writer(endian, __maxAlign);").map_err(fmt_err)?;
    writeln!(out, "{deeper}EncodeInto(w, sample);").map_err(fmt_err)?;
    writeln!(out, "{deeper}return w.ToArray();").map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // EncodeInto(w, sample) — writes the body into a shared writer so this type
    // can be embedded as a nested member (alignment stays relative to the outer
    // CDR stream).
    writeln!(
        out,
        "{inner}public void EncodeInto(Xcdr2Writer w, {struct_name} sample)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    emit_encode_body(out, deeper, &members, ext)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Decode(bytes) — delegates to DecodeFrom on a fresh reader. The default
    // overload assumes little-endian (the canonical XCDR2 wire); the explicit
    // overload lets a caller decode a big-endian payload (the byte order the
    // DCPS layer reads from the encapsulation header).
    writeln!(
        out,
        "{inner}public {struct_name} Decode(ReadOnlySpan<byte> bytes) => Decode(bytes, EndianMode.LittleEndian, 1);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public {struct_name} Decode(ReadOnlySpan<byte> bytes, EndianMode endian) => Decode(bytes, endian, 1);"
    )
    .map_err(fmt_err)?;
    // representation: 1 = XCDR2 (alignment cap 4), 0 = XCDR1 / classic CDR (cap 8,
    // no DHEADER, PL_CDR1 @mutable).
    writeln!(
        out,
        "{inner}public {struct_name} Decode(ReadOnlySpan<byte> bytes, EndianMode endian, int representation)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{deeper}int __maxAlign = representation == 0 ? Xcdr2Reader.Xcdr1MaxAlignmentValue : 4;"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{deeper}var r = new Xcdr2Reader(bytes, endian, __maxAlign);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{deeper}return DecodeFrom(ref r);").map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // DecodeFrom(ref r) — reads the body from a SHARED reader (nested-member
    // entry). `Xcdr2Reader` is a `ref struct` carrying its cursor by value, so
    // it MUST be threaded by ref through every nested decode or the parent's
    // position desyncs (nested struct + seq<struct>). See findings CS-cluster #1.
    writeln!(
        out,
        "{inner}public {struct_name} DecodeFrom(ref Xcdr2Reader r)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    emit_decode_body(out, deeper, &struct_name, &members, ext)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // KeyHash(sample).
    writeln!(out, "{inner}public byte[] KeyHash({struct_name} sample)").map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    if !is_keyed {
        writeln!(out, "{deeper}return new byte[16];").map_err(fmt_err)?;
    } else {
        emit_key_hash_body(out, deeper, &members)?;
    }
    writeln!(out, "{inner}}}").map_err(fmt_err)?;

    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Writes the encode sequence for all members according to the extensibility.
fn emit_encode_body(
    out: &mut String,
    indent: &str,
    members: &[MemberInfo],
    ext: ExtensibilityKind,
) -> Result<(), CsGenError> {
    match ext {
        ExtensibilityKind::Final => {
            // Plain body, no DHEADER.
            for m in members {
                emit_encode_member_plain(out, indent, m)?;
            }
        }
        ExtensibilityKind::Appendable => {
            writeln!(out, "{indent}using (var __scope = w.BeginAppendable())").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let deeper = format!("{indent}    ");
            for m in members {
                emit_encode_member_plain(out, &deeper, m)?;
            }
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        ExtensibilityKind::Mutable => {
            // XCDR1 (classic CDR): @mutable is PL_CDR1 — a [PID][length] list with
            // no outer DHEADER, each member body member-relative. Emit it next to
            // the XCDR2 PL_CDR2 EMHEADER path.
            writeln!(out, "{indent}if (w.IsXcdr1)").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let dd = format!("{indent}    ");
            for (idx, m) in members.iter().enumerate() {
                emit_encode_member_pl_cdr1(out, &dd, m, idx)?;
            }
            writeln!(out, "{dd}w.WritePlCdr1Sentinel();").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
            writeln!(out, "{indent}else").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            writeln!(out, "{indent}using (var __scope = w.BeginMutable())").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let deeper = format!("{indent}    ");
            for (idx, m) in members.iter().enumerate() {
                emit_encode_member_mutable(out, &deeper, m, idx)?;
            }
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
    }
    Ok(())
}

fn emit_encode_member_plain(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
) -> Result<(), CsGenError> {
    if m.is_optional {
        // Final/Appendable: 1-byte present-flag + value-on-true.
        writeln!(
            out,
            "{indent}if (sample.{prop} is null) {{ w.WriteOctet(0); }}",
            prop = m.cs_prop
        )
        .map_err(fmt_err)?;
        writeln!(out, "{indent}else").map_err(fmt_err)?;
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        let deeper = format!("{indent}    ");
        writeln!(out, "{deeper}w.WriteOctet(1);").map_err(fmt_err)?;
        emit_encode_member_value(out, &deeper, m, &optional_value_expr(m))?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
    } else {
        emit_encode_member_value(out, indent, m, &format!("sample.{}", m.cs_prop))?;
    }
    Ok(())
}

/// Encodes a member value, wrapping in nested fixed-count loops when the
/// declarator is an array (`long v[3][4]`). Fixed arrays carry NO length
/// prefix on the wire (XCDR2 §7.4.3.3) — just `dim0 * dim1 * ...` elements.
fn emit_encode_member_value(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    expr: &str,
) -> Result<(), CsGenError> {
    if m.array_dims.is_empty() {
        return emit_encode_value(out, indent, &m.type_spec, expr, 0);
    }
    emit_encode_array(out, indent, &m.type_spec, &m.array_dims, expr, 0)
}

/// Whether a primitive `TypeSpec` element type is a CDR primitive (int / float /
/// bool / char / octet). Non-primitive collection elements (string, struct,
/// sequence, map, another array level, …) get a DHEADER frame under XCDR2.
/// Mirrors `CdrEncode::IS_PRIMITIVE` in `crates/cdr/src/encode.rs`.
fn typespec_is_cdr_primitive(ts: &TypeSpec) -> bool {
    match ts {
        TypeSpec::Primitive(_) => true,
        // A scoped enum is encoded as a 4-byte int → primitive on the wire.
        // Bitmask/bitset are fixed-width integers → primitive. Struct/union/
        // typedef are aggregates → non-primitive.
        TypeSpec::Scoped(sc) => matches!(
            lookup_scoped_kind(sc),
            Some(ScopedKind::Enum { .. })
                | Some(ScopedKind::Bitmask { .. })
                | Some(ScopedKind::Bitset { .. })
        ),
        _ => false,
    }
}

/// Whether the array LEVEL described by `dims` needs an XCDR2 DHEADER.
///
/// XTypes 1.3 §7.4.3.5 rule (8) PARRAY_TYPE: a fixed array whose SCALAR element
/// is a primitive carries NO collection DHEADER — at ANY dimensionality. The
/// rust reference (`crates/cdr/src/composite.rs`) propagates `IS_PRIMITIVE`
/// through `[T;N]` (`const IS_PRIMITIVE: bool = T::IS_PRIMITIVE`), so a multi-dim
/// primitive array `long[2][3]` (T = `[long;3]`, IS_PRIMITIVE = true) gets NO
/// DHEADER at any level. Only an array whose scalar element is non-primitive
/// (struct/union/string/seq/map/typedef) — rule (9) ARRAY_TYPE — is framed, and
/// then at every nested level. So the gate is purely on the SCALAR element type,
/// independent of how many dimensions remain.
///
/// (The earlier `dims.len() > 1` clause emitted a spurious outer DHEADER on
/// `long[2][3]`, diverging from the cross-vendor wire — bug XV-arr.)
fn array_level_needs_dheader(elem_ts: &TypeSpec, _dims: &[String]) -> bool {
    !typespec_is_cdr_primitive(elem_ts)
}

/// zerodds-lint: recursion-depth 64 (bounded by IDL array rank)
fn emit_encode_array(
    out: &mut String,
    indent: &str,
    elem_ts: &TypeSpec,
    dims: &[String],
    expr: &str,
    depth: usize,
) -> Result<(), CsGenError> {
    if dims.is_empty() {
        return emit_encode_value(out, indent, elem_ts, expr, 0);
    }
    // XCDR2 §7.4.3.5 / §7.4.3.3: a fixed array whose element is non-primitive
    // carries a DHEADER (uint32 = body byte length) before its elements; an
    // array of primitives does not. Frame this level when needed.
    let need_dh = array_level_needs_dheader(elem_ts, dims);
    let (body_indent, dh_var) = if need_dh {
        let dh = format!("__arrdh{}", next_arrdh_id());
        writeln!(out, "{indent}using (var {dh} = w.BeginAppendable())").map_err(fmt_err)?;
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        (format!("{indent}    "), Some(dh))
    } else {
        (indent.to_string(), None)
    };
    let indent = body_indent.as_str();
    let iv = format!("__a{depth}");
    let bound = &dims[0];
    writeln!(out, "{indent}for (int {iv} = 0; {iv} < {bound}; {iv}++)").map_err(fmt_err)?;
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    emit_encode_array(
        out,
        &d,
        elem_ts,
        &dims[1..],
        &format!("{expr}[{iv}]"),
        depth + 1,
    )?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    if let Some(_dh) = dh_var {
        // Close the `using (var __arrdhN = w.BeginAppendable())` block (its
        // Dispose patches the DHEADER size). `indent` is the body indent here;
        // step one level out for the closing brace.
        let outer = &indent[..indent.len().saturating_sub(4)];
        writeln!(out, "{outer}}}").map_err(fmt_err)?;
    }
    Ok(())
}

/// CDR wire size in bytes of a primitive type (XCDR2 §7.4.1). `long double`
/// (16) is treated as variable so it falls back to LC4. Mirrors
/// `crates/idl-rust/src/type_map::primitive_wire_size`.
fn primitive_wire_size(p: PrimitiveType) -> u32 {
    use zerodds_idl::ast::FloatingType;
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

/// Compact EMHEADER length code for a `@mutable` member (XTypes 1.3 §7.4.3.4.2).
///
/// Returns the numeric LC (0/1/2/3/5) for members whose framing is known
/// without a separate NEXTINT, or `None` for members that must fall back to the
/// universal LC4 (arrays, optionals, 16-byte `long double`, and any aggregate /
/// collection whose own framing isn't a single leading length word).
///
/// - primitive 1/2/4/8 bytes → LC0/LC1/LC2/LC3 (compact, NO NEXTINT, fixed body)
/// - `string`/`wstring` → LC5 (the body's own uint32 length prefix IS the
///   NEXTINT — no separate NEXTINT goes on the wire; the word stays as the
///   first 4 bytes of the body, reused per §7.4.3.4.2)
///
/// Mirrors the cross-vendor-validated rust reference
/// (`crates/idl-rust/src/struct_emit::mutable_member_length_code`) so the C#
/// wire is byte-identical to the rust golden (bug XV-mut: was hardcoded LC4).
fn mutable_member_lc(m: &MemberInfo) -> Option<u32> {
    // Only scalar (non-array), non-optional members are eligible for a compact
    // LC; arrays/optionals keep their LC4 sub-writer framing.
    if !m.array_dims.is_empty() || m.is_optional {
        return None;
    }
    match &m.type_spec {
        TypeSpec::Primitive(p) => match primitive_wire_size(*p) {
            1 => Some(0),
            2 => Some(1),
            4 => Some(2),
            8 => Some(3),
            _ => None, // long double (16) → LC4
        },
        TypeSpec::String(_) => Some(5),
        // FINDING T1b: a member whose XCDR2 body BEGINS WITH a 4-byte length
        // word — a non-primitive `sequence`/`map` DHEADER, or a nested
        // `@appendable`/`@mutable` struct's DHEADER — uses LC5 to REUSE that
        // word as the EMHEADER NEXTINT (no separate NEXTINT goes on the wire),
        // matching CycloneDDS / RTI / FastDDS and the rust golden. A `@final`
        // nested struct (no DHEADER) and a `sequence<primitive>` (bare element
        // count, not a byte length) stay on the universal LC4.
        spec if member_body_has_leading_dheader(spec) => Some(5),
        _ => None,
    }
}

/// `true` if the XCDR2 body of a member of type `spec` BEGINS WITH a 4-byte
/// length word (a DHEADER or string length prefix), so a `@mutable` EMHEADER
/// can reuse it as the NEXTINT (LC5) instead of serializing a separate one.
///
/// Mirrors the rust reference
/// (`crates/idl-rust/src/type_map::member_body_has_leading_dheader`):
///   * `string`/`wstring` → uint32 octet length prefix → true (handled by the
///     `TypeSpec::String` arm above, kept here for typedef recursion),
///   * `map<K,V>` → always carries a DHEADER → true,
///   * `sequence<E>` → DHEADER iff `E` is **non-primitive** (a
///     `sequence<primitive>` starts with a bare element count, NOT a byte
///     length) → true/false,
///   * a nested struct / typedef-to-struct whose extensibility is
///     `@appendable`/`@mutable` → leading DHEADER → true; a `@final` nested
///     struct is tight-packed (no DHEADER) → false,
///   * a typedef inherits its aliased type's framing.
///
/// zerodds-lint: recursion-depth 16
fn member_body_has_leading_dheader(spec: &TypeSpec) -> bool {
    match spec {
        TypeSpec::String(_) => true,
        // A map has a leading DHEADER iff its (key,value) element is non-primitive
        // (same predicate as the map encoder + `sequence`); `map<long,long>` → LC4.
        TypeSpec::Map(map) => {
            seq_elem_carries_dheader(&map.key) || seq_elem_carries_dheader(&map.value)
        }
        // A `sequence<E>` carries a DHEADER iff its element is non-primitive —
        // and `emit_encode_sequence` decides that with the SAME predicate used
        // here (`!matches!(elem, TypeSpec::Primitive(_))`), so this LC choice
        // stays consistent with the bytes the sequence encoder actually emits.
        // (A `sequence<primitive>` starts with a bare element count, NOT a byte
        // length, so it stays on LC4.)
        TypeSpec::Sequence(seq) => seq_elem_carries_dheader(&seq.elem),
        TypeSpec::Scoped(sc) => match lookup_scoped_kind(sc) {
            Some(ScopedKind::Struct { ext }) => !matches!(ext, ExtensibilityKind::Final),
            // A typedef inherits the framing of its aliased type (a
            // typedef-to-array is not eligible — it is handled as LC4).
            Some(ScopedKind::Typedef {
                inner,
                is_array: false,
            }) => member_body_has_leading_dheader(&inner),
            _ => false,
        },
        _ => false,
    }
}

/// `true` if a `sequence<E>` with element `E` carries a leading XCDR2 DHEADER.
/// MUST mirror `emit_encode_sequence`'s `non_primitive` predicate exactly so
/// the EMHEADER length-code choice (LC5 vs LC4) agrees with the bytes the
/// sequence encoder produces: only a RAW primitive element omits the DHEADER;
/// a struct, string, enum, sequence, map, … element prepends one.
fn seq_elem_carries_dheader(elem: &TypeSpec) -> bool {
    !matches!(elem, TypeSpec::Primitive(_))
}

fn emit_encode_member_mutable(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    idx: usize,
) -> Result<(), CsGenError> {
    let id = m.explicit_id.unwrap_or(idx as u32);
    let mu = if m.must_understand { "true" } else { "false" };
    // XTypes 1.3 §7.4.3.4.2: choose the COMPACT length code per member type so
    // the wire is byte-identical to the cross-vendor-validated rust golden:
    //   - 1/2/4/8-byte primitive → LC0/1/2/3 (no NEXTINT, fixed body)
    //   - string/wstring         → LC5 (the body's own uint32 length prefix is
    //                              reused as the NEXTINT — no separate NEXTINT)
    //   - everything else (array / optional / aggregate / long double) → LC4
    //     (sub-writer + a separately-serialized NEXTINT = body byte length).
    // (Bug XV-mut: the previous hardcoded LC4 for every member produced a
    // non-canonical wire that did NOT match Cyclone/RTI/FastDDS.)
    let compact_lc = mutable_member_lc(m);

    let (open, close, val_expr) = if m.is_optional {
        let prop = &m.cs_prop;
        (
            format!("{indent}if (sample.{prop} is not null)\n{indent}{{"),
            format!("{indent}}}"),
            optional_value_expr(m),
        )
    } else {
        (
            String::new(),
            String::new(),
            format!("sample.{}", m.cs_prop),
        )
    };

    if !open.is_empty() {
        writeln!(out, "{open}").map_err(fmt_err)?;
    }
    let body_indent = if m.is_optional {
        format!("{indent}    ")
    } else {
        indent.to_string()
    };

    if let Some(lc) = compact_lc {
        // LC0/1/2/3 (fixed-size primitive) or LC5 (string/wstring). In BOTH
        // cases the EMHEADER is followed directly by the member body with NO
        // separate NEXTINT: for LC0-3 the body is a fixed 1/2/4/8 bytes; for
        // LC5 the string's own leading uint32 length prefix IS the NEXTINT.
        writeln!(out, "{body_indent}w.WriteEmHeader({id}u, {lc}, {mu});").map_err(fmt_err)?;
        emit_encode_value(out, &body_indent, &m.type_spec, &val_expr, 0)?;
    } else {
        // LC=4 (variable / 8-byte aggregate / array / long double): encode the
        // body in a sub-writer, then EMHEADER(LC=4) + NEXTINT(byte-len) + body.
        writeln!(out, "{body_indent}{{").map_err(fmt_err)?;
        let d = format!("{body_indent}    ");
        // Sub-writer inherits the SHARED writer's endianness. (`EncodeInto` has
        // no `endian` parameter in scope — it threads `w`; referencing a bare
        // `endian` here was a latent compile error that only fired once a
        // @mutable member took the LC=4 sub-writer path, e.g. an 8-byte or
        // optional member.)
        writeln!(out, "{d}var __sub = new Xcdr2Writer(w.Endian);").map_err(fmt_err)?;
        if m.array_dims.is_empty() {
            emit_encode_value_into(out, &d, &m.type_spec, &val_expr, "__sub")?;
        } else {
            emit_encode_array_into(out, &d, &m.type_spec, &m.array_dims, &val_expr, "__sub")?;
        }
        writeln!(out, "{d}var __subBytes = __sub.ToArray();").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteEmHeader({id}u, 4, {mu});").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteUInt32((uint)__subBytes.Length);").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteBytes(__subBytes);").map_err(fmt_err)?;
        writeln!(out, "{body_indent}}}").map_err(fmt_err)?;
    }

    if !close.is_empty() {
        writeln!(out, "{close}").map_err(fmt_err)?;
    }
    Ok(())
}

/// Emits one PL_CDR1 (@mutable XCDR1) member: the value is encoded into a fresh
/// member-relative XCDR1 sub-writer, then framed via `WritePlCdr1Member(id,..)`.
/// (The XCDR1 counterpart of `emit_encode_member_mutable`'s EMHEADER path; all
/// members go through the uniform sub-writer frame, no compact length codes.)
fn emit_encode_member_pl_cdr1(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    idx: usize,
) -> Result<(), CsGenError> {
    let id = m.explicit_id.unwrap_or(idx as u32);
    let (open, close, val_expr) = if m.is_optional {
        let prop = &m.cs_prop;
        (
            format!("{indent}if (sample.{prop} is not null)\n{indent}{{"),
            format!("{indent}}}"),
            optional_value_expr(m),
        )
    } else {
        (
            String::new(),
            String::new(),
            format!("sample.{}", m.cs_prop),
        )
    };
    if !open.is_empty() {
        writeln!(out, "{open}").map_err(fmt_err)?;
    }
    let bi = if m.is_optional {
        format!("{indent}    ")
    } else {
        indent.to_string()
    };
    writeln!(out, "{bi}{{").map_err(fmt_err)?;
    let d = format!("{bi}    ");
    writeln!(
        out,
        "{d}var __sub = new Xcdr2Writer(w.Endian, Xcdr2Writer.Xcdr1MaxAlignmentValue);"
    )
    .map_err(fmt_err)?;
    if m.array_dims.is_empty() {
        emit_encode_value_into(out, &d, &m.type_spec, &val_expr, "__sub")?;
    } else {
        emit_encode_array_into(out, &d, &m.type_spec, &m.array_dims, &val_expr, "__sub")?;
    }
    writeln!(out, "{d}w.WritePlCdr1Member({id}u, __sub.ToArray());").map_err(fmt_err)?;
    writeln!(out, "{bi}}}").map_err(fmt_err)?;
    if !close.is_empty() {
        writeln!(out, "{close}").map_err(fmt_err)?;
    }
    Ok(())
}

/// Like `emit_encode_value`, but writes into a sub-writer with a
/// user-chosen variable name instead of `w`.
fn emit_encode_value_into(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    expr: &str,
    writer_var: &str,
) -> Result<(), CsGenError> {
    // We use a simple string-replace trick: emit into a temporary
    // buffer, then replace `w.` with `<writer_var>.`.
    let mut tmp = String::new();
    emit_encode_value(&mut tmp, indent, ts, expr, 0)?;
    let patched = tmp.replace("w.", &format!("{writer_var}."));
    out.push_str(&patched);
    Ok(())
}

/// Array variant of [`emit_encode_value_into`] — nested fixed loops into a
/// named sub-writer.
fn emit_encode_array_into(
    out: &mut String,
    indent: &str,
    elem_ts: &TypeSpec,
    dims: &[String],
    expr: &str,
    writer_var: &str,
) -> Result<(), CsGenError> {
    let mut tmp = String::new();
    emit_encode_array(&mut tmp, indent, elem_ts, dims, expr, 0)?;
    let patched = tmp.replace("w.", &format!("{writer_var}."));
    out.push_str(&patched);
    Ok(())
}

/// Returns the `Xcdr2Writer` method + the C# cast for a bitmask/bitset holder of
/// `holder_bytes` (1/2/4/8): the holder is an UNSIGNED integer of that width
/// (XTypes 1.3 §7.4.3.4). Matches the `zerodds-cdr` reference, which encodes the
/// holder via the unsigned-int CdrEncode of the storage type.
fn bits_holder_writer(holder_bytes: u32) -> (&'static str, &'static str) {
    match holder_bytes {
        1 => ("w.WriteOctet", "(byte)"),
        2 => ("w.WriteUInt16", "(ushort)"),
        4 => ("w.WriteUInt32", "(uint)"),
        _ => ("w.WriteUInt64", "(ulong)"),
    }
}

/// Returns the `Xcdr2Reader` expression that reads a bitmask/bitset holder of
/// `holder_bytes` as an unsigned integer.
fn bits_holder_reader(holder_bytes: u32) -> &'static str {
    match holder_bytes {
        1 => "r.ReadOctet()",
        2 => "r.ReadUInt16()",
        4 => "r.ReadUInt32()",
        _ => "r.ReadUInt64()",
    }
}

/// zerodds-lint: recursion-depth 64 (emit_encode_value bounded by AST depth)
/// Encode helper for a sample field of the given TypeSpec. `depth` is the
/// current recursion level; it is forwarded to the sequence/map emitters so
/// their temporaries stay unique across nesting (CS-cluster #6).
fn emit_encode_value(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    expr: &str,
    depth: usize,
) -> Result<(), CsGenError> {
    match ts {
        TypeSpec::Primitive(p) => emit_encode_primitive(out, indent, *p, expr),
        TypeSpec::String(s) => {
            // Bounded `string<N>` (DDS-XTypes §7.4.3): reject over-bound on
            // encode. Narrow → UTF-8 byte length (matches the CDR wire); wide
            // `wstring<N>` → UTF-16 unit count (C# string.Length).
            if let Some(b) = &s.bound {
                let bv = crate::emitter::const_expr_to_cs(b);
                if s.wide {
                    writeln!(
                        out,
                        "{indent}if (({expr}) != null && ({expr}).Length > {bv}) throw new System.ArgumentException(\"bounded wstring length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                } else {
                    writeln!(
                        out,
                        "{indent}if (({expr}) != null && System.Text.Encoding.UTF8.GetByteCount({expr}) > {bv}) throw new System.ArgumentException(\"bounded string length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                }
            }
            if s.wide {
                // `wstring` is serialized as uint32 OCTET length (NOT code-unit
                // count, NOT NUL-terminated) followed by the UTF-16LE code units
                // — matching the cross-vendor-validated zerodds-cdr reference
                // (rust-features generated: `write_u32(units*2)` then per-unit
                // `write_u16`). DDS-XTypes 1.3 §7.4.3 / OMG CDR wstring rule.
                emit_encode_wstring(out, indent, expr, "w")?;
            } else {
                writeln!(out, "{indent}w.WriteString({expr});").map_err(fmt_err)?;
            }
            Ok(())
        }
        TypeSpec::Sequence(s) => {
            emit_encode_sequence(out, indent, &s.elem, s.bound.as_ref(), expr, depth)
        }
        TypeSpec::Map(m) => {
            emit_encode_map(out, indent, &m.key, &m.value, m.bound.as_ref(), expr, depth)
        }
        TypeSpec::Scoped(sc) => match lookup_scoped_kind(sc) {
            // Nested struct → encode its body into the shared writer (alignment
            // stays relative to the outer CDR stream).
            Some(ScopedKind::Struct { .. }) => {
                writeln!(
                    out,
                    "{indent}{}TypeSupport.Instance.EncodeInto(w, {expr});",
                    scoped_dotted_cs(sc)
                )
                .map_err(fmt_err)?;
                Ok(())
            }
            // Union → encode via the union's TypeSupport (CS-cluster #3).
            Some(ScopedKind::Union) => {
                writeln!(
                    out,
                    "{indent}{}TypeSupport.Instance.EncodeInto(w, {expr});",
                    scoped_dotted_cs(sc)
                )
                .map_err(fmt_err)?;
                Ok(())
            }
            // Enum → signed ordinal at the @bit_bound width (XTypes §7.4.5.1).
            Some(ScopedKind::Enum { holder_bytes }) => {
                let line = match holder_bytes {
                    1 => format!("{indent}w.WriteOctet((byte)(int){expr});"),
                    2 => format!("{indent}w.WriteInt16((short)(int){expr});"),
                    _ => format!("{indent}w.WriteInt32((int){expr});"),
                };
                writeln!(out, "{line}").map_err(fmt_err)?;
                Ok(())
            }
            // Bitmask → its bit-bound holder integer (XTypes 1.3 §7.4.3.4).
            // The C# member is a `[Flags] enum`; cast to the holder width.
            Some(ScopedKind::Bitmask { holder_bytes }) => {
                let (write, cast) = bits_holder_writer(holder_bytes);
                writeln!(out, "{indent}{write}({cast}{expr});").map_err(fmt_err)?;
                Ok(())
            }
            // Bitset → its declared-width holder integer (XTypes 1.3 §7.4.3.4).
            // The C# member is a `struct` with a `ulong Value`.
            Some(ScopedKind::Bitset { holder_bytes }) => {
                let (write, cast) = bits_holder_writer(holder_bytes);
                writeln!(out, "{indent}{write}({cast}({expr}).Value);").map_err(fmt_err)?;
                Ok(())
            }
            // Typedef → the member is the wrapper record `record class Alias(T
            // Value)`; unwrap to `.Value` and encode the aliased type
            // (CS-cluster #5).
            Some(ScopedKind::Typedef { inner, .. }) => {
                emit_encode_value(out, indent, &inner, &format!("({expr}).Value"), depth)
            }
            // Unresolved scoped ref: treat as an enum-like int32 (best effort).
            None => {
                writeln!(out, "{indent}w.WriteInt32((int){expr});").map_err(fmt_err)?;
                Ok(())
            }
        },
        TypeSpec::Fixed(f) => {
            // fixed<P,S>: CORBA/GIOP §9.3.2.7 packed BCD (the `decimal` field ->
            // (P+2)/2 octets via the runtime helper). No alignment/length/endian.
            let p = crate::emitter::const_expr_to_cs(&f.digits);
            let s = crate::emitter::const_expr_to_cs(&f.scale);
            writeln!(out, "{indent}w.WriteFixedBcd({expr}, {p}, {s});").map_err(fmt_err)?;
            Ok(())
        }
        TypeSpec::Any => {
            // Unreachable: a struct with an any member is gated in `emitter.rs`
            // (no TypeSupport emitted). Defensive safety-net only.
            writeln!(
                out,
                "{indent}throw new XcdrException(\"unsupported codegen TypeSpec for member {expr}\");"
            )
            .map_err(fmt_err)?;
            Ok(())
        }
    }
}

/// Emits the inline `wstring` encode into the writer variable named `wv`
/// (normally `"w"`; a `__sub` for the @mutable LC=4 sub-writer path). uint32
/// octet length (= units*2, NO NUL) + UTF-16LE code units. The codegen embeds
/// this inline (mirroring the rust reference) so it does NOT depend on the C#
/// runtime's `WriteWString`, which length-prefixes by code-unit COUNT.
fn emit_encode_wstring(
    out: &mut String,
    indent: &str,
    expr: &str,
    wv: &str,
) -> Result<(), CsGenError> {
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    writeln!(out, "{d}var __ws = ({expr}) ?? string.Empty;").map_err(fmt_err)?;
    // C# `string` is already UTF-16; `.Length` is the code-unit count, so the
    // octet length is `2 * Length` and each char is one LE code unit.
    writeln!(out, "{d}{wv}.WriteUInt32((uint)(__ws.Length * 2));").map_err(fmt_err)?;
    writeln!(out, "{d}for (int __wi = 0; __wi < __ws.Length; __wi++)").map_err(fmt_err)?;
    writeln!(out, "{d}{{").map_err(fmt_err)?;
    writeln!(out, "{d}    {wv}.WriteUInt16(__ws[__wi]);").map_err(fmt_err)?;
    writeln!(out, "{d}}}").map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    Ok(())
}

fn emit_encode_primitive(
    out: &mut String,
    indent: &str,
    p: PrimitiveType,
    expr: &str,
) -> Result<(), CsGenError> {
    match p {
        PrimitiveType::Boolean => {
            writeln!(out, "{indent}w.WriteBool({expr});").map_err(fmt_err)?;
        }
        PrimitiveType::Octet => {
            writeln!(out, "{indent}w.WriteOctet({expr});").map_err(fmt_err)?;
        }
        PrimitiveType::Char => {
            writeln!(out, "{indent}w.WriteOctet((byte)({expr}));").map_err(fmt_err)?;
        }
        PrimitiveType::WideChar => {
            writeln!(out, "{indent}w.WriteWChar({expr});").map_err(fmt_err)?;
        }
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => {
                writeln!(out, "{indent}w.WriteInt16({expr});").map_err(fmt_err)?;
            }
            IntegerType::UShort | IntegerType::UInt16 => {
                writeln!(out, "{indent}w.WriteUInt16({expr});").map_err(fmt_err)?;
            }
            IntegerType::Long | IntegerType::Int32 => {
                writeln!(out, "{indent}w.WriteInt32({expr});").map_err(fmt_err)?;
            }
            IntegerType::ULong | IntegerType::UInt32 => {
                writeln!(out, "{indent}w.WriteUInt32({expr});").map_err(fmt_err)?;
            }
            IntegerType::LongLong | IntegerType::Int64 => {
                writeln!(out, "{indent}w.WriteInt64({expr});").map_err(fmt_err)?;
            }
            IntegerType::ULongLong | IntegerType::UInt64 => {
                writeln!(out, "{indent}w.WriteUInt64({expr});").map_err(fmt_err)?;
            }
            IntegerType::Int8 => {
                writeln!(out, "{indent}w.WriteOctet((byte)({expr}));").map_err(fmt_err)?;
            }
            IntegerType::UInt8 => {
                writeln!(out, "{indent}w.WriteOctet({expr});").map_err(fmt_err)?;
            }
        },
        PrimitiveType::Floating(f) => match f {
            zerodds_idl::ast::FloatingType::Float => {
                writeln!(out, "{indent}w.WriteFloat32({expr});").map_err(fmt_err)?;
            }
            zerodds_idl::ast::FloatingType::Double => {
                writeln!(out, "{indent}w.WriteFloat64({expr});").map_err(fmt_err)?;
            }
            zerodds_idl::ast::FloatingType::LongDouble => {
                writeln!(
                    out,
                    "{indent}throw new XcdrException(\"long double not in v1.0 codegen surface\");"
                )
                .map_err(fmt_err)?;
            }
        },
    }
    Ok(())
}
/// zerodds-lint: recursion-depth 64 (emit_encode_sequence bounded by AST depth)
/// `depth` makes every emitted local unique per recursion level so that
/// `sequence<sequence<T>>` does not reuse a single `__seq`/`__mat`/`__item`
/// identifier across nested block scopes (CS-cluster #6).
fn emit_encode_sequence(
    out: &mut String,
    indent: &str,
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    depth: usize,
) -> Result<(), CsGenError> {
    let elem_ty = cs_storage_type(elem);
    let seqv = format!("__seq{depth}");
    let matv = format!("__mat{depth}");
    let itemv = format!("__item{depth}");
    let dhv = format!("__seqdh{depth}");
    // XCDR2 §7.4.3.5: non-primitive elements (string, struct, …) →
    // DHEADER (uint32 = byte length of [count + elements]) prepended;
    // primitives not. Verified against Cyclone DDS (V-5 without, V-6 with).
    let non_primitive = !matches!(elem, TypeSpec::Primitive(_));
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    writeln!(
        out,
        "{d}var {seqv} = ({expr}) as System.Collections.Generic.IEnumerable<{elem_ty}>;"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{d}var {matv} = {seqv} is null ? new System.Collections.Generic.List<{elem_ty}>() : new System.Collections.Generic.List<{elem_ty}>({seqv});").map_err(fmt_err)?;
    if non_primitive {
        writeln!(out, "{d}using (var {dhv} = w.BeginAppendable())").map_err(fmt_err)?;
        writeln!(out, "{d}{{").map_err(fmt_err)?;
    }
    let seqd = if non_primitive {
        format!("{d}    ")
    } else {
        d.clone()
    };
    // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode error.
    if let Some(b) = bound {
        let bv = crate::emitter::const_expr_to_cs(b);
        writeln!(
            out,
            "{seqd}if ({matv}.Count > {bv}) throw new System.ArgumentException(\"bounded sequence length exceeds its IDL bound ({bv})\");"
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{seqd}w.WriteSequenceLength({matv}.Count);").map_err(fmt_err)?;
    writeln!(out, "{seqd}foreach (var {itemv} in {matv})").map_err(fmt_err)?;
    writeln!(out, "{seqd}{{").map_err(fmt_err)?;
    let dd = format!("{seqd}    ");
    emit_encode_value(out, &dd, elem, &itemv, depth + 1)?;
    writeln!(out, "{seqd}}}").map_err(fmt_err)?;
    if non_primitive {
        writeln!(out, "{d}}}").map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    Ok(())
}

/// Encodes a `map<K,V>` as XCDR2 (CS-cluster #2): a DHEADER-framed sequence of
/// `(key, value)` pairs — count + interleaved key/value encodes. `depth` keeps
/// the locals unique across nested maps/sequences.
/// zerodds-lint: recursion-depth 64 (bounded by AST depth)
fn emit_encode_map(
    out: &mut String,
    indent: &str,
    key: &TypeSpec,
    value: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    depth: usize,
) -> Result<(), CsGenError> {
    let key_ty = cs_storage_type(key);
    let val_ty = cs_storage_type(value);
    let mapv = format!("__map{depth}");
    let kvv = format!("__kv{depth}");
    let dhv = format!("__mapdh{depth}");
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    writeln!(
        out,
        "{d}var {mapv} = ({expr}) ?? new System.Collections.Generic.Dictionary<{key_ty}, {val_ty}>();"
    )
    .map_err(fmt_err)?;
    if let Some(b) = bound {
        let bv = crate::emitter::const_expr_to_cs(b);
        writeln!(
            out,
            "{d}if ({mapv}.Count > {bv}) throw new System.ArgumentException(\"bounded map length exceeds its IDL bound ({bv})\");"
        )
        .map_err(fmt_err)?;
    }
    // XCDR2 §7.4.3.5: a map carries a DHEADER only when its (key,value) element
    // is non-primitive. `map<long,long>` (both primitive) omits it — matching
    // cdr-core `needs_collection_dheader(.., K::IS_PRIMITIVE && V::IS_PRIMITIVE)`
    // and FastDDS/OpenDDS. (Same rule as `sequence<primitive>` / PARRAY.)
    // Map element is non-primitive (→ DHEADER) iff its key OR value is non-primitive.
    // Uses the SAME per-element predicate as `seq_elem_carries_dheader` so map and
    // sequence agree, and an `enum`/`struct`-valued map keeps its DHEADER (TK_ENUM is
    // NOT a primitive type, XTypes 1.3 §7.4.1).
    let map_dh = seq_elem_carries_dheader(key) || seq_elem_carries_dheader(value);
    let dd = if map_dh {
        writeln!(out, "{d}using (var {dhv} = w.BeginAppendable())").map_err(fmt_err)?;
        writeln!(out, "{d}{{").map_err(fmt_err)?;
        format!("{d}    ")
    } else {
        d.clone()
    };
    writeln!(out, "{dd}w.WriteSequenceLength({mapv}.Count);").map_err(fmt_err)?;
    writeln!(out, "{dd}foreach (var {kvv} in {mapv})").map_err(fmt_err)?;
    writeln!(out, "{dd}{{").map_err(fmt_err)?;
    let ddd = format!("{dd}    ");
    emit_encode_value(out, &ddd, key, &format!("{kvv}.Key"), depth + 1)?;
    emit_encode_value(out, &ddd, value, &format!("{kvv}.Value"), depth + 1)?;
    writeln!(out, "{dd}}}").map_err(fmt_err)?;
    if map_dh {
        writeln!(out, "{d}}}").map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    Ok(())
}

/// Decode body: uses object-initializer syntax.
fn emit_decode_body(
    out: &mut String,
    indent: &str,
    struct_name: &str,
    members: &[MemberInfo],
    ext: ExtensibilityKind,
) -> Result<(), CsGenError> {
    match ext {
        ExtensibilityKind::Final => {
            // Decode sequentially, then object initializer.
            for (i, m) in members.iter().enumerate() {
                emit_decode_member_to_var(out, indent, m, i)?;
            }
            emit_decode_return(out, indent, struct_name, members)?;
        }
        ExtensibilityKind::Appendable => {
            writeln!(out, "{indent}var __scope = r.BeginDHeader();").map_err(fmt_err)?;
            for (i, m) in members.iter().enumerate() {
                emit_decode_member_to_var(out, indent, m, i)?;
            }
            writeln!(out, "{indent}r.EndDHeader(__scope);").map_err(fmt_err)?;
            emit_decode_return(out, indent, struct_name, members)?;
        }
        ExtensibilityKind::Mutable => {
            // Variable order / optionality -> nullable locals,
            // then while loop until the DHEADER end.
            for (i, m) in members.iter().enumerate() {
                let ty = decode_member_local_type_opt(m, m.is_optional);
                writeln!(out, "{indent}{ty} __m{i} = default!;").map_err(fmt_err)?;
            }
            // XCDR1 (classic CDR): @mutable is PL_CDR1 — a [PID][length] parameter
            // list with no outer DHEADER, each member body member-relative
            // aligned. Emit the PL_CDR1 loop alongside the XCDR2 EMHEADER loop.
            writeln!(out, "{indent}if (r.IsXcdr1)").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let p = format!("{indent}    ");
            writeln!(
                out,
                "{p}while (r.BeginPlCdr1Member(out var __id, out var __plscope))"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{p}{{").map_err(fmt_err)?;
            let pp = format!("{p}    ");
            writeln!(out, "{pp}switch (__id)").map_err(fmt_err)?;
            writeln!(out, "{pp}{{").map_err(fmt_err)?;
            let ppp = format!("{pp}    ");
            for (i, m) in members.iter().enumerate() {
                let id = m.explicit_id.unwrap_or(i as u32);
                writeln!(out, "{ppp}case {id}u:").map_err(fmt_err)?;
                let pppp = format!("{ppp}    ");
                emit_decode_member_assign(out, &pppp, m, i)?;
                writeln!(out, "{pppp}break;").map_err(fmt_err)?;
            }
            // Unknown member: EndPlCdr1Member (below) advances past its body.
            writeln!(out, "{ppp}default: break;").map_err(fmt_err)?;
            writeln!(out, "{pp}}}").map_err(fmt_err)?;
            writeln!(out, "{pp}r.EndPlCdr1Member(__plscope);").map_err(fmt_err)?;
            writeln!(out, "{p}}}").map_err(fmt_err)?;
            emit_decode_return_mutable(out, indent, struct_name, members)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
            // XCDR2 PL_CDR2 EMHEADER loop.
            writeln!(out, "{indent}var __scope = r.BeginDHeader();").map_err(fmt_err)?;
            writeln!(out, "{indent}while (!r.DHeaderDone(__scope))").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let d = format!("{indent}    ");
            writeln!(out, "{d}var (__id, __lc, __mu) = r.ReadEmHeader();").map_err(fmt_err)?;
            // NEXTINT consumption: only LC==4 carries a SEPARATELY-serialized
            // NEXTINT (XTypes 1.3 §7.4.3.4.2). LC0..3 are fixed 1/2/4/8-byte
            // bodies with no NEXTINT; LC5/6/7 REUSE the member body's own
            // leading length word (string/dheader length) as the NEXTINT — it
            // stays as the first 4 bytes of the body and the member's own
            // decoder consumes it, so we must NOT consume it here. (Consuming a
            // NEXTINT for LC5 would eat the string's length prefix and corrupt
            // the decode — bug XV-mut decode mirror.)
            writeln!(
                out,
                "{d}if (__lc == 4) {{ var __nx = r.ReadUInt32(); _ = __nx; }}"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{d}switch (__id)").map_err(fmt_err)?;
            writeln!(out, "{d}{{").map_err(fmt_err)?;
            let dd = format!("{d}    ");
            for (i, m) in members.iter().enumerate() {
                let id = m.explicit_id.unwrap_or(i as u32);
                writeln!(out, "{dd}case {id}u:").map_err(fmt_err)?;
                let ddd = format!("{dd}    ");
                emit_decode_member_assign(out, &ddd, m, i)?;
                writeln!(out, "{ddd}break;").map_err(fmt_err)?;
            }
            writeln!(out, "{dd}default:").map_err(fmt_err)?;
            writeln!(
                out,
                "{dd}    throw new XcdrException($\"unknown member id {{__id}}\");"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{d}}}").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
            writeln!(out, "{indent}r.EndDHeader(__scope);").map_err(fmt_err)?;
            emit_decode_return_mutable(out, indent, struct_name, members)?;
        }
    }
    Ok(())
}

fn decode_local_type(ts: &TypeSpec, optional: bool) -> String {
    let base = cs_storage_type(ts);
    if optional && !is_reference_type(ts) {
        format!("{base}?")
    } else {
        base
    }
}

/// Decode local type for a whole member, honoring array declarators: a member
/// `long v[3][4]` is stored as the jagged `int[][]` (matching the property
/// type from `type_for_declarator`).
fn decode_member_local_type(m: &MemberInfo) -> String {
    decode_member_local_type_opt(m, false)
}

/// As [`decode_member_local_type`], but appends a nullable `?` when `optional`
/// is set so an ABSENT `@optional` member decodes to `null` instead of the
/// value-type default (CS2). For an array member the jagged type itself is made
/// nullable (`int[]?`); for a scalar/sequence/map member the base is made
/// nullable via [`decode_local_type`].
fn decode_member_local_type_opt(m: &MemberInfo, optional: bool) -> String {
    if m.array_dims.is_empty() {
        return decode_local_type(&m.type_spec, optional);
    }
    let mut base = cs_storage_type(&m.type_spec);
    for _ in &m.array_dims {
        base = format!("{base}[]");
    }
    if optional { format!("{base}?") } else { base }
}

/// Decodes an array member: allocate the jagged arrays for each rank, then fill
/// with nested fixed-count loops (no length prefix — XCDR2 §7.4.3.3).
/// zerodds-lint: recursion-depth 64 (bounded by IDL array rank)
fn emit_decode_array(
    out: &mut String,
    indent: &str,
    elem_ts: &TypeSpec,
    dims: &[String],
    target: &str,
    depth: usize,
) -> Result<(), CsGenError> {
    if dims.is_empty() {
        return emit_decode_assign(out, indent, elem_ts, target, depth);
    }
    // Skip the DHEADER this level carries when its element is non-primitive
    // (inverse of `emit_encode_array`). The runtime decode advances the cursor
    // past the body; the DHEADER value is only used for skip-unknown semantics,
    // so for a known type we just consume the 4-byte size (XCDR2 §7.4.3.5).
    let need_dh = array_level_needs_dheader(elem_ts, dims);
    let dh_name = if need_dh {
        let dh = format!("__arrdh{}", next_arrdh_id());
        writeln!(out, "{indent}var {dh} = r.BeginDHeader();").map_err(fmt_err)?;
        Some(dh)
    } else {
        None
    };
    // Jagged allocation: the size goes in the FIRST bracket, then one EMPTY
    // `[]` per remaining inner dimension — C# `new int[2][]`, not `new int[][2]`.
    let base_elem = cs_storage_type(elem_ts);
    let inner_brackets = "[]".repeat(dims.len() - 1);
    let bound = &dims[0];
    let iv = format!("__da{depth}");
    writeln!(
        out,
        "{indent}{target} = new {base_elem}[{bound}]{inner_brackets};"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}for (int {iv} = 0; {iv} < {bound}; {iv}++)").map_err(fmt_err)?;
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    emit_decode_array(
        out,
        &d,
        elem_ts,
        &dims[1..],
        &format!("{target}[{iv}]"),
        depth + 1,
    )?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    if let Some(dh) = dh_name {
        writeln!(out, "{indent}r.EndDHeader({dh});").map_err(fmt_err)?;
    }
    Ok(())
}
/// zerodds-lint: recursion-depth 64 (cs_storage_type bounded by AST depth)
fn cs_storage_type(ts: &TypeSpec) -> String {
    match ts {
        // Must agree with the property type from `prim_to_cs_type` (used for the
        // member declaration) so the decode local + object-initializer assignment
        // type-check. IDL `char` → C# `char` (NOT `byte`) — the prior mismatch
        // (`char` property, `byte` decode local) was a CS0266 compile error.
        TypeSpec::Primitive(p) => prim_to_cs_type(*p).to_string(),
        TypeSpec::String(_) => "string".into(),
        TypeSpec::Sequence(s) => {
            let inner = cs_storage_type(&s.elem);
            // The property type from the codegen is `Omg.Types.ISequence<T>` for
            // an unbounded sequence and `Omg.Types.IBoundedSequence<T>` for a
            // bounded one (emitter.rs `typespec_to_cs`). The decode local must
            // match the property type so the object-initializer assignment
            // type-checks (CS2: bounded-member decode-type mismatch) — a
            // `SequenceList<T>` (ISequence) is NOT assignable to an
            // `IBoundedSequence<T>` property.
            if s.bound.is_some() {
                format!("Omg.Types.IBoundedSequence<{inner}>")
            } else {
                format!("Omg.Types.ISequence<{inner}>")
            }
        }
        TypeSpec::Scoped(s) => s
            .parts
            .iter()
            .map(|p| p.text.clone())
            .collect::<Vec<_>>()
            .join("."),
        TypeSpec::Map(m) => {
            // Property type is `IDictionary<K,V>`; decode container is the
            // concrete `Dictionary<K,V>`.
            let k = cs_storage_type(&m.key);
            let v = cs_storage_type(&m.value);
            format!("System.Collections.Generic.IDictionary<{k}, {v}>")
        }
        TypeSpec::Fixed(_) => "decimal".into(),
        TypeSpec::Any => "object".into(),
    }
}

fn prim_to_cs_type(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Octet => "byte",
        // IDL `char` → C# `char`, MATCHING the member property type emitted by
        // `type_map::primitive_to_cs` (also `char`). The decode local + object
        // initializer must agree with the property or the assignment is a
        // CS0266 compile error. The CDR wire form is still a single octet
        // (encode `(byte)Ch`, decode `(char)ReadOctet()`).
        PrimitiveType::Char => "char",
        PrimitiveType::WideChar => "char",
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => "short",
            IntegerType::UShort | IntegerType::UInt16 => "ushort",
            IntegerType::Long | IntegerType::Int32 => "int",
            IntegerType::ULong | IntegerType::UInt32 => "uint",
            IntegerType::LongLong | IntegerType::Int64 => "long",
            IntegerType::ULongLong | IntegerType::UInt64 => "ulong",
            IntegerType::Int8 => "sbyte",
            IntegerType::UInt8 => "byte",
        },
        PrimitiveType::Floating(f) => match f {
            zerodds_idl::ast::FloatingType::Float => "float",
            zerodds_idl::ast::FloatingType::Double => "double",
            zerodds_idl::ast::FloatingType::LongDouble => "decimal",
        },
    }
}

fn is_reference_type(ts: &TypeSpec) -> bool {
    matches!(
        ts,
        TypeSpec::String(_) | TypeSpec::Sequence(_) | TypeSpec::Map(_) | TypeSpec::Any
    )
}

/// Whether an `@optional` member of this type is stored as a NULLABLE VALUE type
/// (`T?` boxed via `Nullable<T>`) and therefore needs `.Value` to read the
/// payload, vs. a NULLABLE REFERENCE type (`string?`, `ISequence<T>?`, a record
/// class, a jagged array) whose nullable annotation does NOT introduce a
/// `Nullable<T>` wrapper — those are dereferenced directly.
///
/// Without this distinction the encode path emitted `sample.Prop.Value` for an
/// `@optional string`/`sequence`/`map`/nested-struct/array, which does not
/// compile (reference types have no `.Value`) — every optional-of-aggregate
/// struct failed to build.
fn optional_uses_dotvalue(m: &MemberInfo) -> bool {
    // Arrays are jagged reference types (`int[]`), never `Nullable<T[]>`.
    if !m.array_dims.is_empty() {
        return false;
    }
    match &m.type_spec {
        TypeSpec::Primitive(_) | TypeSpec::Fixed(_) => true,
        TypeSpec::Scoped(sc) => match lookup_scoped_kind(sc) {
            // Enum → C# enum (value type) → `Enum?` is `Nullable<Enum>`.
            // Bitmask (`[Flags] enum`) and bitset (`struct`) are value types too.
            Some(
                ScopedKind::Enum { .. } | ScopedKind::Bitmask { .. } | ScopedKind::Bitset { .. },
            )
            | None => true,
            // Struct/union (record class) + typedef wrapper (record class) are
            // reference types; the typedef arm unwraps its OWN `.Value`.
            Some(ScopedKind::Struct { .. } | ScopedKind::Union | ScopedKind::Typedef { .. }) => {
                false
            }
        },
        // string / sequence / map / any → reference types.
        _ => false,
    }
}

/// The C# expression that reads an `@optional` member's payload inside the
/// present branch: `sample.Prop.Value` for nullable value types, `sample.Prop`
/// for nullable reference types.
fn optional_value_expr(m: &MemberInfo) -> String {
    if optional_uses_dotvalue(m) {
        format!("sample.{}.Value", m.cs_prop)
    } else {
        format!("sample.{}", m.cs_prop)
    }
}

fn emit_decode_member_to_var(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    idx: usize,
) -> Result<(), CsGenError> {
    // CS2: an absent `@optional` member must round-trip as `null`, not the
    // value-type default (e.g. `0.0`). Type the decode local as nullable when
    // optional so absence stays distinguishable from a present zero. For value
    // types this adds `?`; reference/array types are already nullable, but with
    // `#nullable enable` we annotate them `?` too so the `default!` init is null
    // rather than a non-null assertion. Mirrors the Mutable path which already
    // uses `decode_local_type(.., is_optional)`.
    let ty = if m.is_optional {
        decode_member_local_type_opt(m, true)
    } else {
        decode_member_local_type(m)
    };
    writeln!(out, "{indent}{ty} __m{idx} = default!;").map_err(fmt_err)?;
    if m.is_optional {
        // Final/Appendable: present-flag + value.
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        let d = format!("{indent}    ");
        writeln!(out, "{d}byte __present = r.ReadOctet();").map_err(fmt_err)?;
        writeln!(out, "{d}if (__present != 0)").map_err(fmt_err)?;
        writeln!(out, "{d}{{").map_err(fmt_err)?;
        let dd = format!("{d}    ");
        emit_decode_member_value(out, &dd, m, &format!("__m{idx}"))?;
        writeln!(out, "{d}}}").map_err(fmt_err)?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
    } else {
        emit_decode_member_value(out, indent, m, &format!("__m{idx}"))?;
    }
    Ok(())
}

/// Decodes a member value into `target`, dispatching to the array path when the
/// declarator carries fixed dimensions.
fn emit_decode_member_value(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    target: &str,
) -> Result<(), CsGenError> {
    if m.array_dims.is_empty() {
        emit_decode_assign(out, indent, &m.type_spec, target, 0)
    } else {
        emit_decode_array(out, indent, &m.type_spec, &m.array_dims, target, 0)
    }
}

fn emit_decode_member_assign(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    idx: usize,
) -> Result<(), CsGenError> {
    // Mutable: assign the value directly into the local.
    emit_decode_member_value(out, indent, m, &format!("__m{idx}"))?;
    Ok(())
}
/// zerodds-lint: recursion-depth 64 (emit_decode_assign bounded by AST depth)
/// Emits C# statements that assign `target` the decoded value of `ts`. `depth`
/// keeps the per-level temporaries unique across nesting (CS-cluster #6).
fn emit_decode_assign(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    target: &str,
    depth: usize,
) -> Result<(), CsGenError> {
    // Typedef-to-AGGREGATE (`typedef sequence<T> Vec; Vec v;` /
    // `typedef map<K,V> M; M m;`): the alias resolves to a sequence/map whose
    // decode needs multiple statements (a loop), so it can't go through the
    // single-expression `decode_simple_expr` path (which only re-wraps
    // primitive/string/struct/enum aliases). Decode the inner aggregate into a
    // temp, then re-wrap into the alias record `new Vec(temp)` (CS2 —
    // typedef-to-aggregate codec).
    if let TypeSpec::Scoped(sc) = ts {
        if let Some(inner) = typedef_to_aggregate_inner(sc) {
            let tmpv = format!("__tdagg{depth}");
            let inner_ty = cs_storage_type(&inner);
            writeln!(out, "{indent}{inner_ty} {tmpv};").map_err(fmt_err)?;
            emit_decode_assign(out, indent, &inner, &tmpv, depth + 1)?;
            writeln!(
                out,
                "{indent}{target} = new {}({tmpv});",
                scoped_dotted_cs(sc)
            )
            .map_err(fmt_err)?;
            return Ok(());
        }
    }
    // `wstring`: inline multi-statement decode (uint32 octet length + UTF-16LE
    // code units). Mirrors the encode and the zerodds-cdr reference; cannot ride
    // the single-expression `decode_simple_expr` path.
    if let TypeSpec::String(s) = ts {
        if s.wide {
            return emit_decode_wstring(out, indent, target, depth, s.bound.as_ref());
        }
        // B1 follow-up (#22 decode-side parity): a bounded narrow `string<N>`
        // cannot ride the single-expression `decode_simple_expr` path (it
        // has nowhere to insert a bound check) — mirror the encode-side
        // check (`emit_encode_value`, byte-length via UTF8.GetByteCount) on
        // decode too. XTypes 1.3 §7.4.3 requires enforcement on BOTH sides;
        // `r.ReadString()` only ever validated the wire's remaining bytes.
        if let Some(b) = &s.bound {
            let bv = crate::emitter::const_expr_to_cs(b);
            let tmpv = format!("__bcs{depth}");
            writeln!(out, "{indent}string {tmpv} = r.ReadString();").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}if ({tmpv} != null && System.Text.Encoding.UTF8.GetByteCount({tmpv}) > {bv}) throw new System.ArgumentException(\"decoded string length exceeds its IDL bound ({bv})\");"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{indent}{target} = {tmpv};").map_err(fmt_err)?;
            return Ok(());
        }
    }
    match ts {
        TypeSpec::Primitive(_)
        | TypeSpec::String(_)
        | TypeSpec::Scoped(_)
        | TypeSpec::Fixed(_)
        | TypeSpec::Any => {
            writeln!(
                out,
                "{indent}{target} = {expr};",
                target = target,
                expr = decode_simple_expr(ts)
            )
            .map_err(fmt_err)?;
            Ok(())
        }
        TypeSpec::Map(m) => emit_decode_map(
            out,
            indent,
            &m.key,
            &m.value,
            m.bound.as_ref(),
            target,
            depth,
        ),
        TypeSpec::Sequence(s) => {
            let elem_ty = cs_storage_type(&s.elem);
            let dhv = format!("__seqdh{depth}");
            let cntv = format!("__cnt{depth}");
            let listv = format!("__list{depth}");
            let iv = format!("__i{depth}");
            let ev = format!("__e{depth}");
            // XCDR2 §7.4.3.5: for non-primitive elements, skip over the DHEADER.
            let non_primitive = !matches!(&*s.elem, TypeSpec::Primitive(_));
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let d = format!("{indent}    ");
            if non_primitive {
                writeln!(out, "{d}var {dhv} = r.BeginDHeader();").map_err(fmt_err)?;
            }
            writeln!(out, "{d}int {cntv} = r.ReadSequenceLength();").map_err(fmt_err)?;
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // bound check (`emit_encode_sequence`) — XTypes 1.3 §7.4.3
            // requires the IDL bound enforced on decode too, not just the
            // wire-format validation `ReadSequenceLength` already does.
            if let Some(b) = &s.bound {
                let bv = crate::emitter::const_expr_to_cs(b);
                writeln!(
                    out,
                    "{d}if ({cntv} > {bv}) throw new System.ArgumentException(\"decoded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            // Pick the concrete container matching the property type: an
            // unbounded sequence uses `SequenceList<T>` (ISequence), a bounded
            // `sequence<T, N>` uses `BoundedList<T>(N)` (IBoundedSequence) so the
            // recovered value is assignable to the `IBoundedSequence<T>` property
            // (CS2). The runtime decode already validated the wire length; the
            // bound is the IDL maximum, carried for the contract on the property.
            if let Some(b) = &s.bound {
                let bv = crate::emitter::const_expr_to_cs(b);
                writeln!(
                    out,
                    "{d}var {listv} = new Omg.Types.BoundedList<{elem_ty}>({bv});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{d}var {listv} = new Omg.Types.SequenceList<{elem_ty}>();"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{d}for (int {iv} = 0; {iv} < {cntv}; {iv}++)").map_err(fmt_err)?;
            writeln!(out, "{d}{{").map_err(fmt_err)?;
            let dd = format!("{d}    ");
            // Element decode can itself be recursive.
            writeln!(out, "{dd}{elem_ty} {ev};").map_err(fmt_err)?;
            emit_decode_assign(out, &dd, &s.elem, &ev, depth + 1)?;
            writeln!(out, "{dd}{listv}.Add({ev});").map_err(fmt_err)?;
            writeln!(out, "{d}}}").map_err(fmt_err)?;
            if non_primitive {
                writeln!(out, "{d}r.EndDHeader({dhv});").map_err(fmt_err)?;
            }
            writeln!(out, "{d}{target} = {listv};").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
            Ok(())
        }
    }
}

/// Decodes a `map<K,V>` (inverse of [`emit_encode_map`]): a DHEADER-framed
/// count + interleaved key/value decodes into a `Dictionary<K,V>`.
/// zerodds-lint: recursion-depth 64 (bounded by AST depth)
fn emit_decode_map(
    out: &mut String,
    indent: &str,
    key: &TypeSpec,
    value: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
    depth: usize,
) -> Result<(), CsGenError> {
    let key_ty = cs_storage_type(key);
    let val_ty = cs_storage_type(value);
    let dhv = format!("__mapdh{depth}");
    let cntv = format!("__mcnt{depth}");
    let dictv = format!("__dict{depth}");
    let iv = format!("__mi{depth}");
    let kv = format!("__mk{depth}");
    let vv = format!("__mv{depth}");
    // XCDR2 §7.4.3.5: only a non-primitive map is DHEADER-framed (symmetric with
    // the encode gate). A `map<long,long>` reads count + pairs directly.
    let map_dh = seq_elem_carries_dheader(key) || seq_elem_carries_dheader(value);
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    if map_dh {
        writeln!(out, "{d}var {dhv} = r.BeginDHeader();").map_err(fmt_err)?;
    }
    writeln!(out, "{d}int {cntv} = r.ReadSequenceLength();").map_err(fmt_err)?;
    // B1 follow-up (#22 decode-side parity): mirror the encode-side bound
    // check (`emit_encode_map`) — XTypes 1.3 §7.4.3.
    if let Some(b) = bound {
        let bv = crate::emitter::const_expr_to_cs(b);
        writeln!(
            out,
            "{d}if ({cntv} > {bv}) throw new System.ArgumentException(\"decoded map length exceeds its IDL bound ({bv})\");"
        )
        .map_err(fmt_err)?;
    }
    writeln!(
        out,
        "{d}var {dictv} = new System.Collections.Generic.Dictionary<{key_ty}, {val_ty}>();"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{d}for (int {iv} = 0; {iv} < {cntv}; {iv}++)").map_err(fmt_err)?;
    writeln!(out, "{d}{{").map_err(fmt_err)?;
    let dd = format!("{d}    ");
    writeln!(out, "{dd}{key_ty} {kv};").map_err(fmt_err)?;
    emit_decode_assign(out, &dd, key, &kv, depth + 1)?;
    writeln!(out, "{dd}{val_ty} {vv};").map_err(fmt_err)?;
    emit_decode_assign(out, &dd, value, &vv, depth + 1)?;
    writeln!(out, "{dd}{dictv}[{kv}] = {vv};").map_err(fmt_err)?;
    writeln!(out, "{d}}}").map_err(fmt_err)?;
    if map_dh {
        writeln!(out, "{d}r.EndDHeader({dhv});").map_err(fmt_err)?;
    }
    writeln!(out, "{d}{target} = {dictv};").map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    Ok(())
}

/// Inline `wstring` decode into `target` (inverse of [`emit_encode_wstring`]):
/// uint32 OCTET length, then `length/2` UTF-16LE code units assembled into a C#
/// `string`. Matches the zerodds-cdr reference (byte length, not code-unit
/// count). DDS-XTypes 1.3 §7.4.3 / OMG CDR wstring rule.
///
/// B1 blocker fix (deep review of #22 decode-bounds-cross-backend): a bounded
/// `wstring<N>` must reject an over-bound decode the same way the encode side
/// does (`emit_encode_value`, `s.wide` branch, `.Length > bound`) — this was
/// missing entirely, so decode silently accepted any wide string regardless
/// of its IDL bound. Mirrors the narrow `string<N>` decode check above.
fn emit_decode_wstring(
    out: &mut String,
    indent: &str,
    target: &str,
    depth: usize,
    bound: Option<&ConstExpr>,
) -> Result<(), CsGenError> {
    let octv = format!("__woct{depth}");
    let sbv = format!("__wsb{depth}");
    let iv = format!("__wj{depth}");
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    writeln!(out, "{d}uint {octv} = r.ReadUInt32();").map_err(fmt_err)?;
    writeln!(
        out,
        "{d}var {sbv} = new System.Text.StringBuilder((int)({octv} / 2));"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{d}for (uint {iv} = 0; {iv} + 1 < {octv}; {iv} += 2)").map_err(fmt_err)?;
    writeln!(out, "{d}{{").map_err(fmt_err)?;
    writeln!(out, "{d}    {sbv}.Append((char)r.ReadUInt16());").map_err(fmt_err)?;
    writeln!(out, "{d}}}").map_err(fmt_err)?;
    if let Some(b) = bound {
        let bv = crate::emitter::const_expr_to_cs(b);
        writeln!(
            out,
            "{d}if ({sbv}.Length > {bv}) throw new System.ArgumentException(\"decoded wstring length exceeds its IDL bound ({bv})\");"
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{d}{target} = {sbv}.ToString();").map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn decode_simple_expr(ts: &TypeSpec) -> String {
    match ts {
        TypeSpec::Primitive(p) => decode_primitive_expr(*p).to_string(),
        TypeSpec::String(_) => "r.ReadString()".into(),
        TypeSpec::Scoped(sc) => match lookup_scoped_kind(sc) {
            // Nested struct → decode its body from the shared reader (by ref).
            Some(ScopedKind::Struct { .. }) => {
                format!(
                    "{}TypeSupport.Instance.DecodeFrom(ref r)",
                    scoped_dotted_cs(sc)
                )
            }
            // Union member → decode via the union's TypeSupport (by ref).
            Some(ScopedKind::Union) => {
                format!(
                    "{}TypeSupport.Instance.DecodeFrom(ref r)",
                    scoped_dotted_cs(sc)
                )
            }
            // Enum → cast from the int32 representation.
            Some(ScopedKind::Enum { holder_bytes }) => {
                let read = match holder_bytes {
                    1 => "(sbyte)r.ReadOctet()",
                    2 => "r.ReadInt16()",
                    _ => "r.ReadInt32()",
                };
                format!("({}){read}", scoped_dotted_cs(sc))
            }
            // Bitmask → cast the holder integer back into the `[Flags] enum`.
            Some(ScopedKind::Bitmask { holder_bytes }) => {
                format!(
                    "({}){}",
                    scoped_dotted_cs(sc),
                    bits_holder_reader(holder_bytes)
                )
            }
            // Bitset → read the holder into a fresh `struct { Value }`.
            Some(ScopedKind::Bitset { holder_bytes }) => {
                format!(
                    "new {} {{ Value = {} }}",
                    scoped_dotted_cs(sc),
                    bits_holder_reader(holder_bytes)
                )
            }
            // Typedef → the member field is the wrapper record
            // `record class Alias(T Value)`; decode the aliased type and
            // re-wrap into the alias constructor (CS-cluster #5).
            Some(ScopedKind::Typedef { inner, .. }) => {
                format!(
                    "new {}({})",
                    scoped_dotted_cs(sc),
                    decode_simple_expr(&inner)
                )
            }
            // Unresolved: best effort.
            _ => "default!".into(),
        },
        TypeSpec::Fixed(f) => {
            // fixed<P,S>: read (P+2)/2 packed-BCD octets back into a `decimal`.
            let p = crate::emitter::const_expr_to_cs(&f.digits);
            let s = crate::emitter::const_expr_to_cs(&f.scale);
            format!("r.ReadFixedBcd({p}, {s})")
        }
        TypeSpec::Map(_) | TypeSpec::Any => {
            // Map is decoded via the statement-emitting path; `any` is gated.
            "throw new XcdrException(\"decode unsupported type\")".into()
        }
        TypeSpec::Sequence(_) => "default!".into(),
    }
}

fn decode_primitive_expr(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "r.ReadBool()",
        PrimitiveType::Octet => "r.ReadOctet()",
        // IDL `char` → C# `char` (1-byte CDR octet). Cast the read byte to char
        // so the decode local (`char`) type-checks. Encode casts `(byte)(expr)`.
        PrimitiveType::Char => "(char)r.ReadOctet()",
        PrimitiveType::WideChar => "r.ReadWChar()",
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => "r.ReadInt16()",
            IntegerType::UShort | IntegerType::UInt16 => "r.ReadUInt16()",
            IntegerType::Long | IntegerType::Int32 => "r.ReadInt32()",
            IntegerType::ULong | IntegerType::UInt32 => "r.ReadUInt32()",
            IntegerType::LongLong | IntegerType::Int64 => "r.ReadInt64()",
            IntegerType::ULongLong | IntegerType::UInt64 => "r.ReadUInt64()",
            IntegerType::Int8 => "(sbyte)r.ReadOctet()",
            IntegerType::UInt8 => "r.ReadOctet()",
        },
        PrimitiveType::Floating(f) => match f {
            zerodds_idl::ast::FloatingType::Float => "r.ReadFloat32()",
            zerodds_idl::ast::FloatingType::Double => "r.ReadFloat64()",
            zerodds_idl::ast::FloatingType::LongDouble => "default(decimal)",
        },
    }
}

fn emit_decode_return(
    out: &mut String,
    indent: &str,
    struct_name: &str,
    members: &[MemberInfo],
) -> Result<(), CsGenError> {
    writeln!(out, "{indent}return new {struct_name}").map_err(fmt_err)?;
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    for (i, m) in members.iter().enumerate() {
        writeln!(out, "{d}{prop} = __m{i}!,", prop = m.cs_prop, i = i).map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}};").map_err(fmt_err)?;
    Ok(())
}

fn emit_decode_return_mutable(
    out: &mut String,
    indent: &str,
    struct_name: &str,
    members: &[MemberInfo],
) -> Result<(), CsGenError> {
    writeln!(out, "{indent}return new {struct_name}").map_err(fmt_err)?;
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    for (i, m) in members.iter().enumerate() {
        writeln!(out, "{d}{prop} = __m{i}!,", prop = m.cs_prop, i = i).map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}};").map_err(fmt_err)?;
    Ok(())
}

/// KeyHash: PlainCdr2BeKeyHolder -> MD5 if the KeyHolder's maximum
/// serialized size exceeds 16 bytes, otherwise zero-pad. XTypes 1.3 §7.6.8.4
/// step 5.
///
/// The zero-pad/MD5 branch is decided **statically per topic type**, from
/// the maximum possible size of the `@key` members — NOT from the actual
/// runtime length of a given instance's serialized bytes (`crates/cdr/src/
/// key_hash.rs`'s module doc is the authoritative statement of this rule;
/// the `idl-rust` reference bakes the same static decision into a
/// `KEY_HOLDER_MAX_SIZE` const consumed by `zerodds_cdr::compute_key_hash`).
/// The previous C# codegen branched on `__kb.Length` — the ACTUAL runtime
/// byte count of this specific sample — which only coincides with the
/// static decision when every `@key` member is fixed-size; for a `@key`
/// with a bounded string/sequence, a short instance would wrongly take the
/// zero-pad branch even though the type's static max mandates MD5,
/// diverging from every other backend/vendor for the same type.
fn emit_key_hash_body(
    out: &mut String,
    indent: &str,
    members: &[MemberInfo],
) -> Result<(), CsGenError> {
    writeln!(
        out,
        "{indent}var __kw = new Xcdr2Writer(EndianMode.BigEndian);"
    )
    .map_err(fmt_err)?;
    let key_members: Vec<&MemberInfo> = members.iter().filter(|m| m.is_key).collect();
    // member-id order (explicit `@id(N)`, else declaration index) — XTypes
    // 1.3 §7.6.8.3.1.b. The previous code walked `members` in declaration
    // order, silently diverging from `@id`-reordered structs.
    let ordered = sort_by_member_id(key_members);
    for m in &ordered {
        emit_key_encode_value(out, indent, &m.type_spec, &format!("sample.{}", m.cs_prop))?;
    }
    writeln!(out, "{indent}var __kb = __kw.ToArray();").map_err(fmt_err)?;
    let uses_md5 = match csharp_key_holder_max_size(&ordered) {
        Some(n) => n > 16,
        None => true,
    };
    if uses_md5 {
        writeln!(out, "{indent}return Md5.Hash(__kb);").map_err(fmt_err)?;
    } else {
        writeln!(out, "{indent}var __h = new byte[16];").map_err(fmt_err)?;
        writeln!(
            out,
            "{indent}System.Array.Copy(__kb, 0, __h, 0, __kb.Length);"
        )
        .map_err(fmt_err)?;
        writeln!(out, "{indent}return __h;").map_err(fmt_err)?;
    }
    Ok(())
}

/// Sorts `@key` members into KeyHolder emission order: explicit `@id(N)` if
/// set, else the member's declaration index (XTypes 1.3 §7.6.8.3.1.b).
fn sort_by_member_id(members: Vec<&MemberInfo>) -> Vec<&MemberInfo> {
    let mut ordered: Vec<(u32, &MemberInfo)> = members
        .into_iter()
        .enumerate()
        .map(|(idx, m)| (m.explicit_id.unwrap_or(idx as u32), m))
        .collect();
    ordered.sort_by_key(|(id, _)| *id);
    ordered.into_iter().map(|(_, m)| m).collect()
}

/// Static maximum serialized size (PLAIN_CDR2-BE, max align 4) of a
/// KeyHolder built from `key_members` (already in emission order). Mirrors
/// `crates/idl-rust/src/struct_emit.rs`'s `compute_key_holder_max_size` /
/// `key_holder_atom_size` (byte-identical algorithm, ported to this
/// backend's `TYPE_REG`/`STRUCT_REG`), and the shared reference in
/// `crates/idl/src/keyhash.rs`. `None` => dynamically sized => MD5 branch.
fn csharp_key_holder_max_size(key_members: &[&MemberInfo]) -> Option<usize> {
    let mut offset = 0usize;
    for m in key_members {
        let count: usize = if m.array_dims.is_empty() {
            1
        } else {
            // Array dimensions are C# constant-expression strings here, not
            // resolved integers — array-typed top-level `@key` members are
            // not exercised by any passing case today (the nested-struct
            // key path already rejects array fields explicitly); treat as
            // dynamically sized rather than guess a wrong static size.
            return None;
        };
        for _ in 0..count {
            offset = csharp_key_atom_size(&m.type_spec, offset)?;
        }
    }
    Some(offset)
}

/// Evaluates a bounded-string `@key`'s bound (decimal or `0x` literal, with
/// an optional leading unary `+`) to a `usize`. `None` if non-constant.
/// Mirrors `crates/idl/src/keyhash.rs::const_usize`.
fn const_expr_to_usize(e: &ConstExpr) -> Option<usize> {
    use zerodds_idl::ast::{Literal, LiteralKind, UnaryOp};
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => {
            let t = raw.trim();
            let v = if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                i64::from_str_radix(hex, 16).ok()?
            } else {
                t.parse::<i64>().ok()?
            };
            usize::try_from(v).ok()
        }
        ConstExpr::Unary {
            op: UnaryOp::Plus,
            operand,
            ..
        } => const_expr_to_usize(operand),
        _ => None,
    }
}

/// Advances a running KeyHolder byte offset by one occurrence of a `@key`
/// member of type `ts`, applying the BE PLAIN_CDR2 alignment (max align 4)
/// before the value. Recurses into nested `@key` structs (own `@key`
/// subset, or all members if none — XTypes 1.3 §7.6.8). `None` for a
/// dynamically sized type (unbounded string, sequence, map, enum/union/
/// bitmask/bitset/unresolved scoped — matching the `idl-rust` reference's
/// conservative treatment of non-struct scoped types).
fn csharp_key_atom_size(ts: &TypeSpec, offset: usize) -> Option<usize> {
    let pad_to = |off: usize, align: usize| -> usize { off + (align - (off % align)) % align };
    match ts {
        TypeSpec::Primitive(p) => {
            let size = primitive_wire_size(*p) as usize;
            let align = size.clamp(1, 4);
            pad_to(offset, align).checked_add(size)
        }
        TypeSpec::String(s) => {
            let n = const_expr_to_usize(s.bound.as_ref()?)?;
            let body = if s.wide { 2 * n } else { n + 1 };
            pad_to(offset, 4).checked_add(4)?.checked_add(body)
        }
        TypeSpec::Scoped(scoped) => match lookup_scoped_kind(scoped) {
            Some(ScopedKind::Typedef {
                inner,
                is_array: false,
            }) => csharp_key_atom_size(&inner, offset),
            Some(ScopedKind::Struct { .. }) => {
                let sd = lookup_struct_def(scoped)?;
                let nested = collect_member_info(&sd);
                let nested_keys: Vec<&MemberInfo> = nested.iter().filter(|m| m.is_key).collect();
                let effective: Vec<&MemberInfo> = if nested_keys.is_empty() {
                    nested.iter().collect()
                } else {
                    nested_keys
                };
                let effective = sort_by_member_id(effective);
                let mut off = offset;
                for m in &effective {
                    if !m.array_dims.is_empty() {
                        return None;
                    }
                    off = csharp_key_atom_size(&m.type_spec, off)?;
                }
                Some(off)
            }
            _ => None,
        },
        // sequence / fixed / map / any @key => dynamically sized => MD5.
        _ => None,
    }
}

fn emit_key_encode_value(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    expr: &str,
) -> Result<(), CsGenError> {
    match ts {
        TypeSpec::Primitive(p) => emit_key_encode_primitive(out, indent, *p, expr),
        TypeSpec::String(_) => {
            writeln!(out, "{indent}__kw.WriteString({expr});").map_err(fmt_err)?;
            Ok(())
        }
        TypeSpec::Scoped(scoped) => emit_key_encode_scoped(out, indent, scoped, expr),
        _ => {
            writeln!(
                out,
                "{indent}throw new XcdrException(\"unsupported key type for member {expr}\");"
            )
            .map_err(fmt_err)?;
            Ok(())
        }
    }
}

/// `TypeSpec::Scoped` key member: dealias typedefs (chasing multi-level
/// chains via recursion), expand a nested-struct key into its own `@key`
/// members (or ALL members if it has none — XTypes 1.3 §7.6.8), in
/// member-id order. Enums / unions / bitmasks / bitsets / array-typedef /
/// unresolved names are unsupported `@key` shapes (matching the `idl-rust`
/// reference, which rejects them the same way) — emit a loud runtime
/// `XcdrException`, consistent with the existing catch-all below, instead of
/// the previous silent no-op that emitted zero key bytes for a
/// typedef-aliased or nested-struct `@key` member (a wrong KeyHash on the
/// wire, cross-vendor-interop-breaking).
fn emit_key_encode_scoped(
    out: &mut String,
    indent: &str,
    scoped: &ScopedName,
    expr: &str,
) -> Result<(), CsGenError> {
    match lookup_scoped_kind(scoped) {
        Some(ScopedKind::Typedef {
            inner,
            is_array: false,
        }) => {
            // The member's C# type is the typedef wrapper record `record
            // class Alias(T Value)` (see `emit_encode_value`'s matching
            // Typedef arm) — unwrap `.Value` before recursing on the
            // dealiased inner type.
            emit_key_encode_value(out, indent, &inner, &format!("({expr}).Value"))
        }
        Some(ScopedKind::Struct { .. }) => {
            let Some(sd) = lookup_struct_def(scoped) else {
                return emit_key_unsupported(out, indent, expr, "unresolved nested @key struct");
            };
            let nested = collect_member_info(&sd);
            let nested_keys: Vec<&MemberInfo> = nested.iter().filter(|m| m.is_key).collect();
            let effective: Vec<&MemberInfo> = if nested_keys.is_empty() {
                nested.iter().collect()
            } else {
                nested_keys
            };
            // member-id order (explicit `@id(N)`, else declaration index),
            // matching the KeyHolder emission order used for the outer
            // struct (XTypes 1.3 §7.6.8.3.1.b).
            let effective = sort_by_member_id(effective);
            for m in &effective {
                if !m.array_dims.is_empty() {
                    return emit_key_unsupported(
                        out,
                        indent,
                        expr,
                        "array field inside a nested-struct @key",
                    );
                }
                emit_key_encode_value(out, indent, &m.type_spec, &format!("{expr}.{}", m.cs_prop))?;
            }
            Ok(())
        }
        _ => emit_key_unsupported(
            out,
            indent,
            expr,
            "enum/union/bitmask/bitset/array-typedef/unresolved @key",
        ),
    }
}

/// Emits a loud `XcdrException` throw for a `@key` shape this backend does
/// not (yet) support, instead of silently emitting zero key bytes.
fn emit_key_unsupported(
    out: &mut String,
    indent: &str,
    expr: &str,
    what: &str,
) -> Result<(), CsGenError> {
    writeln!(
        out,
        "{indent}throw new XcdrException(\"unsupported key type ({what}) for member {expr}\");"
    )
    .map_err(fmt_err)?;
    Ok(())
}

/// Looks up the full [`StructDef`] for a scoped nested-struct `@key` member.
/// Populated alongside [`TYPE_REG`] in [`build_type_registry`].
fn lookup_struct_def(s: &ScopedName) -> Option<StructDef> {
    let key = &s.parts.last()?.text;
    STRUCT_REG.with(|r| r.borrow().get(key).cloned())
}

fn emit_key_encode_primitive(
    out: &mut String,
    indent: &str,
    p: PrimitiveType,
    expr: &str,
) -> Result<(), CsGenError> {
    let stmt = match p {
        PrimitiveType::Boolean => format!("__kw.WriteBool({expr});"),
        PrimitiveType::Octet => format!("__kw.WriteOctet({expr});"),
        PrimitiveType::Char => format!("__kw.WriteOctet((byte)({expr}));"),
        PrimitiveType::WideChar => format!("__kw.WriteWChar({expr});"),
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => format!("__kw.WriteInt16({expr});"),
            IntegerType::UShort | IntegerType::UInt16 => format!("__kw.WriteUInt16({expr});"),
            IntegerType::Long | IntegerType::Int32 => format!("__kw.WriteInt32({expr});"),
            IntegerType::ULong | IntegerType::UInt32 => format!("__kw.WriteUInt32({expr});"),
            IntegerType::LongLong | IntegerType::Int64 => format!("__kw.WriteInt64({expr});"),
            IntegerType::ULongLong | IntegerType::UInt64 => format!("__kw.WriteUInt64({expr});"),
            IntegerType::Int8 => format!("__kw.WriteOctet((byte)({expr}));"),
            IntegerType::UInt8 => format!("__kw.WriteOctet({expr});"),
        },
        PrimitiveType::Floating(f) => match f {
            zerodds_idl::ast::FloatingType::Float => format!("__kw.WriteFloat32({expr});"),
            zerodds_idl::ast::FloatingType::Double => format!("__kw.WriteFloat64({expr});"),
            zerodds_idl::ast::FloatingType::LongDouble => {
                "throw new XcdrException(\"long double key not supported\");".into()
            }
        },
    };
    writeln!(out, "{indent}{stmt}").map_err(fmt_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn type_name_root_struct() {
        assert_eq!(make_dds_type_name(&[], "Point"), "Point");
    }

    #[test]
    fn type_name_one_module() {
        assert_eq!(make_dds_type_name(&["Outer".to_string()], "S"), "Outer::S");
    }

    #[test]
    fn type_name_nested_modules() {
        assert_eq!(
            make_dds_type_name(&["Outer".to_string(), "Inner".to_string()], "S"),
            "Outer::Inner::S"
        );
    }
}
