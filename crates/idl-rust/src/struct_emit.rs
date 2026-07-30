// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Emittiert `pub struct X { … }` plus `impl DdsType for X { … }`.
//!
//! Phase A: only final extensibility, primitive fields. Composite types
//! (String, Vec, Array, Option) folgen in Phase B; Extensibility-Modi
//! (appendable, mutable) in Phase C; Keys + KeyHash in Phase D.

use zerodds_idl::ast::types::{ConstExpr, Declarator, LiteralKind, Member, StructDef, TypeSpec};
use zerodds_idl::semantics::annotations::PlacementKind;

use crate::annotations::{StructExtensibility, struct_extensibility};
use crate::error::{Result, RustGenError};
use crate::type_map::escape_keyword;
use crate::type_map::rust_type_for;

/// Returns the Rust identifier of a declarator as an owned `String`,
/// raw-identifier-escaped if needed (spec §6.1 + §6.2).
pub(crate) fn declarator_ident(decl: &Declarator) -> String {
    let raw = match decl {
        Declarator::Simple(n) => &n.text,
        Declarator::Array(a) => &a.name.text,
    };
    escape_keyword(raw)
}

/// Raw (un-escaped) IDL name of a member's first declarator — the string the
/// `@autoid(HASH)` / `@hashid` member-id derivation hashes (findings A31/A32).
/// The name-hash MUST use the source spelling, never the Rust-escaped form.
fn member_raw_name(member: &Member) -> &str {
    member
        .declarators
        .first()
        .map_or("", |d| d.name().text.as_str())
}

/// Wire member-id of `member` at positional index `idx`, honoring
/// `@id`/`@hashid`/`@autoid(HASH)` (findings A31/A32) via the shared resolver
/// so the EMHEADER/PID/key-order ids match the TypeObject. `autoid_hash` is the
/// enclosing struct's `@autoid(HASH)` flag.
fn member_wire_id(autoid_hash: bool, member: &Member, idx: usize) -> u32 {
    crate::annotations::resolved_member_id(
        autoid_hash,
        &member.annotations,
        member_raw_name(member),
        idx as u32,
    )
}

/// The struct's WIRE members — every member except `@non_serialized` ones
/// (broad-audit P0-5, #2) — paired with a COMPACTED positional index (0,1,2,…
/// over the serialized members only). Every wire loop that assigns a positional
/// member id (sequential-autoid EMHEADER/PL_CDR1 ids) iterates THIS so the ids
/// stay gap-free and agree with the TypeObject builder (`resolve_member_ids`),
/// which likewise counts only the emitted members. Filtering before enumerating
/// is what keeps a dropped `@non_serialized` member from shifting the survivors
/// into a wrong/gapped id.
fn serialized_members(s: &StructDef) -> impl Iterator<Item = (usize, &Member)> {
    s.members
        .iter()
        .filter(|m| !crate::annotations::member_is_non_serialized(&m.annotations))
        .enumerate()
}

/// Flattens a struct's base-type inheritance chain into a single member list
/// (finding A10): the oldest ancestor's members first, then each descendant's,
/// then the struct's own members — the XTypes 1.3 §7.2.2.4.4 derived-type wire
/// order (base members precede derived members). Base structs are resolved via
/// the `STRUCT_DEFS` registry; a cycle guard bounds pathological loops. Mirrors
/// idl-cpp's `resolved_wire_members` so the two bindings agree on the wire.
/// zerodds-lint: recursion-depth 32
fn resolved_wire_members(s: &StructDef) -> Vec<Member> {
    let mut chain: Vec<StructDef> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut cur = s.base.clone();
    while let Some(bn) = cur {
        let Some(def) = crate::type_map::struct_def_by_scoped(&bn) else {
            break;
        };
        if !seen.insert(def.name.text.clone()) {
            break;
        }
        cur = def.base.clone();
        chain.push(def);
    }
    let mut out: Vec<Member> = Vec::new();
    for def in chain.into_iter().rev() {
        out.extend(def.members.iter().cloned());
    }
    out.extend(s.members.iter().cloned());
    out
}

/// Returns a copy of `s` whose `members` are the inheritance-flattened wire
/// members ([`resolved_wire_members`]) and whose `base` is cleared. Used for
/// every data-wire emission (fields, encode, decode, `field_value`, key) so a
/// derived struct carries its inherited members; the `base` is cleared so no
/// second pass re-expands it. The TYPE_IDENTIFIER is computed from the ORIGINAL
/// struct (via the shared frontend), matching idl-cpp.
fn flattened_struct(s: &StructDef) -> StructDef {
    let mut flat = s.clone();
    flat.members = resolved_wire_members(s);
    flat.base = None;
    flat
}

/// Computes the concrete Rust type of a member field given both its
/// element [`TypeSpec`] and a specific [`Declarator`]. For a
/// `Declarator::Simple` this is just the element type; for a
/// `Declarator::Array` the element type is wrapped in nested
/// fixed-size arrays `[..; N]` matching the IDL dimensions, innermost
/// dimension last (so `long grid[4][4]` → `[[i32; 4]; 4]`). This MUST
/// agree with the field declaration emitted in `emit_member_field`,
/// otherwise the generated `CdrDecode` call would name a scalar type
/// and fail to type-check / read only one element (bug R2).
pub(crate) fn declarator_rust_type(spec: &TypeSpec, decl: &Declarator) -> Result<String> {
    let elem = rust_type_for(spec)?;
    match decl {
        Declarator::Simple(_) => Ok(elem),
        Declarator::Array(arr) => {
            let mut wrapped = elem;
            for size_expr in arr.sizes.iter().rev() {
                let size = crate::type_map::const_expr_as_usize(size_expr).ok_or(
                    RustGenError::InvalidAnnotation {
                        name: "array-size".to_string(),
                        reason: "non-integer array dimension",
                    },
                )?;
                wrapped = format!("[{wrapped}; {size}]");
            }
            Ok(wrapped)
        }
    }
}

/// Emits a complete Rust struct definition + DdsType impl.
///
/// `module_path` is the list of enclosing IDL modules (raw names). It is
/// used to emit the fully-qualified `TYPE_NAME` `"Module::Sub::Struct"`.
pub fn emit_struct(out: &mut String, s: &StructDef, module_path: &[String]) -> Result<()> {
    emit_struct_with_mode(out, s, module_path, false)
}

/// Like [`emit_struct`], but with a `cdr_only` switch: at `true` the
/// `DdsType` impl (XCDR2 + `field_value`, pulls `zerodds_dcps`/`zerodds_types`)
/// is omitted. Only the struct decl + classic `CdrEncode`/`CdrDecode` remain —
/// exactly what the CORBA/GIOP path needs (no DDS topic pipeline, hence no
/// DdsType dependencies).
pub fn emit_struct_with_mode(
    out: &mut String,
    s: &StructDef,
    module_path: &[String],
    cdr_only: bool,
) -> Result<()> {
    let extensibility = struct_extensibility(&s.annotations);

    // A10: emit against the inheritance-flattened member list (base members
    // first). The TYPE_IDENTIFIER stays computed from the ORIGINAL `s` (shared
    // frontend), so keep both around.
    let flat = flattened_struct(s);

    emit_struct_decl(out, &flat)?;
    out.push('\n');
    if !cdr_only {
        emit_dds_type_impl(out, &flat, extensibility, module_path, s)?;
    }
    // Writer-agnostic CDR impls (`CdrEncode`/`CdrDecode`). These serve TWO
    // call sites with different wire rules, distinguished at RUNTIME by the
    // writer/reader `max_alignment` (XCDR2 caps it at 4, XTypes 1.3 §7.4.3.2.3
    // INIT MAXALIGN; classic CDR / CORBA-GIOP keeps 8):
    //   * classic CDR §15.3 (CORBA/GIOP, max_align 8): plain sequential
    //     fields, NO extensibility DHEADER frame.
    //   * XCDR2 nested (DDS, max_align 4): a `@appendable`/`@mutable`
    //     aggregate carries its OWN DHEADER per XTypes 1.3 §7.4.3.5.3
    //     rule (30) (and rule (21) for mutable). This is what a nested or
    //     `sequence<>`/array/map element struct emits on the wire — the
    //     top-level `DdsType::encode` only frames the OUTERMOST type, so the
    //     per-element frame must live here, in `CdrEncode`.
    emit_cdr_encode_impl(out, &flat, extensibility)?;
    emit_cdr_decode_impl(out, &flat, extensibility)?;
    Ok(())
}

/// Helper: emits the plain sequential field-encode statements into `writer_expr`.
fn emit_plain_field_encodes(
    out: &mut String,
    s: &StructDef,
    indent: &str,
    writer_expr: &str,
) -> Result<()> {
    for (_, member) in serialized_members(s) {
        emit_member_encode_with_writer(out, member, indent, writer_expr)?;
    }
    Ok(())
}

/// `impl CdrEncode for <struct>`.
///
/// For `@final` structs (and on the classic-CDR / CORBA path for any struct)
/// this is plain sequential field encoding. For `@appendable`/`@mutable`
/// structs on the XCDR2 path (writer `max_alignment() == 4`, XTypes 1.3
/// §7.4.3.2.3) the body is wrapped in the cdr-core appendable frame
/// (`struct_enc::encode_appendable` = DHEADER + body) per rule (30) — so a
/// NESTED or collection-element struct of this type carries its own DHEADER on
/// the wire, exactly as the spec virtual machine prescribes. Classic CDR
/// (max_align 8) keeps the plain form, since GIOP/IIOP has no DHEADER.
fn emit_cdr_encode_impl(
    out: &mut String,
    s: &StructDef,
    extensibility: StructExtensibility,
) -> Result<()> {
    let name = escape_keyword(&s.name.text);
    out.push('\n');
    out.push_str(crate::emitter::IMPL_LINT_ALLOW);
    out.push_str(&format!("impl zerodds_cdr::CdrEncode for {name} {{\n"));
    out.push_str(
        "    fn encode(&self, writer: &mut zerodds_cdr::BufferWriter) -> ::core::result::Result<(), zerodds_cdr::EncodeError> {\n",
    );
    let autoid = crate::annotations::struct_autoid_hash(&s.annotations);
    match extensibility {
        StructExtensibility::Final => {
            emit_plain_field_encodes(out, s, "        ", "writer")?;
        }
        StructExtensibility::Appendable => {
            // XCDR2 (max_align 4): self-delimit with a DHEADER frame
            // (XTypes 1.3 §7.4.3.5.3 rule (30); cf. cdr-core
            // `struct_enc::encode_appendable`). Classic CDR: plain.
            out.push_str("        if writer.max_alignment() == 4 {\n");
            out.push_str("            zerodds_cdr::struct_enc::encode_appendable(writer, |w| {\n");
            emit_plain_field_encodes(out, s, "                ", "w")?;
            out.push_str("                Ok(())\n");
            out.push_str("            })?;\n");
            out.push_str("        } else {\n");
            emit_plain_field_encodes(out, s, "            ", "writer")?;
            out.push_str("        }\n");
        }
        StructExtensibility::Mutable => {
            // XCDR2 (max_align 4): DHEADER + EMHEADER-per-member
            // (XTypes 1.3 §7.4.3.5.3 rule (21)/(22); cf. cdr-core
            // `MutableStructEncoder`). Classic CDR: plain.
            let required_list = required_member_ids(s)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str("        if writer.max_alignment() == 4 {\n");
            out.push_str("            zerodds_cdr::struct_enc::encode_appendable(writer, |w| {\n");
            out.push_str(&format!(
                "                let mut enc = zerodds_cdr::struct_enc::MutableStructEncoder::new(w, ::std::vec![{required_list}]);\n"
            ));
            for (idx, member) in serialized_members(s) {
                emit_mutable_member_encode(out, member, idx, autoid, "                ")?;
            }
            out.push_str("                enc.finish()?;\n");
            out.push_str("                Ok(())\n");
            out.push_str("            })?;\n");
            out.push_str("        } else {\n");
            // FINDING E1: classic CDR / XCDR1 (max_align 8) for a `@mutable`
            // struct is PL_CDR1 — a 16-bit-PID/16-bit-length parameter list
            // per member (extended header for ids/lengths that don't fit),
            // terminated by the PID_LIST_END sentinel (XTypes 1.3 §7.4.1.2 /
            // §7.4.2). Previously this emitted plain positional fields (no PID
            // framing, no sentinel) — FastDDS emits proper PL_CDR (36B vs our
            // 23B). Each member's body is its plain field encoding.
            for (idx, member) in serialized_members(s) {
                emit_pl_cdr1_member_encode(out, member, idx, autoid, "            ")?;
            }
            out.push_str("            zerodds_cdr::xcdr1::write_pl_cdr1_sentinel(writer)?;\n");
            out.push_str("        }\n");
        }
    }
    out.push_str("        ::core::result::Result::Ok(())\n");
    out.push_str("    }\n}\n");
    Ok(())
}

/// Emits a single PL_CDR1 (`@mutable` XCDR1 / classic CDR) member: an
/// `encode_pl_cdr1_member(writer, <id>, |w| { <field body> Ok(()) })` call.
/// The body is the member's plain field serialization (no DHEADER); the
/// cdr-core helper writes the 16-/32-bit PID header, the body padded to a
/// 4-byte boundary, and chooses the extended header automatically. FINDING E1.
fn emit_pl_cdr1_member_encode(
    out: &mut String,
    member: &Member,
    fallback_id: usize,
    autoid_hash: bool,
    indent: &str,
) -> Result<()> {
    let id = member_wire_id(autoid_hash, member, fallback_id);
    let optional = crate::annotations::member_is_optional(&member.annotations);
    for declarator in &member.declarators {
        let name = declarator_ident(declarator);
        out.push_str(indent);
        out.push_str(&format!(
            "zerodds_cdr::xcdr1::encode_pl_cdr1_member(writer, {id}, |w| {{ "
        ));
        emit_member_bound_checks(out, member, declarator);
        emit_field_encode_with_optional(
            out,
            &member.type_spec,
            &format!("self.{name}"),
            "w",
            optional,
        )?;
        out.push_str(" Ok(()) })?;\n");
    }
    Ok(())
}

/// Collects the member IDs of all non-optional members (mutable framing).
fn required_member_ids(s: &StructDef) -> Vec<u32> {
    let autoid = crate::annotations::struct_autoid_hash(&s.annotations);
    serialized_members(s)
        .filter(|(_, m)| !crate::annotations::member_is_optional(&m.annotations))
        .map(|(idx, m)| member_wire_id(autoid, m, idx))
        .collect()
}

/// `impl CdrDecode for <struct>` — symmetric to [`emit_cdr_encode_impl`].
/// On the XCDR2 path an `@appendable`/`@mutable` struct strips its own DHEADER
/// frame; classic CDR reads the plain sequential layout.
fn emit_cdr_decode_impl(
    out: &mut String,
    s: &StructDef,
    extensibility: StructExtensibility,
) -> Result<()> {
    let name = escape_keyword(&s.name.text);
    out.push('\n');
    out.push_str(crate::emitter::IMPL_LINT_ALLOW);
    out.push_str(&format!("impl zerodds_cdr::CdrDecode for {name} {{\n"));
    out.push_str(
        "    fn decode(reader: &mut zerodds_cdr::BufferReader<'_>) -> ::core::result::Result<Self, zerodds_cdr::DecodeError> {\n",
    );
    match extensibility {
        StructExtensibility::Final => {
            emit_cdr_decode_plain(out, s, "        ", "reader")?;
        }
        StructExtensibility::Appendable => {
            // XCDR2 path: the struct self-delimited with a DHEADER frame on
            // encode, so strip it via decode_appendable (cdr-core); the body
            // is decoded in declaration order (forward-compat: extra trailing
            // bytes inside the frame are skipped by the sub-reader). Classic
            // CDR: plain sequential.
            out.push_str("        if reader.max_alignment() == 4 {\n");
            out.push_str("            zerodds_cdr::struct_enc::decode_appendable(reader, |r| {\n");
            emit_cdr_decode_plain(out, s, "                ", "r")?;
            out.push_str("            })\n");
            out.push_str("        } else {\n");
            emit_cdr_decode_plain(out, s, "            ", "reader")?;
            out.push_str("        }\n");
        }
        StructExtensibility::Mutable => {
            // FINDING T1a: a `@mutable` struct's `CdrDecode` MUST parse the
            // EMHEADER member frame (the same `read_mutable_member` loop the
            // `DdsType::decode` path uses), NOT decode positionally. When a
            // nested `@mutable` member is decoded through this inner-type
            // `CdrDecode`, positional reads consumed the inner struct's first
            // EMHEADER as its first field. The decode_appendable receiver here
            // is the fn param `reader` (already `&mut BufferReader`). On the
            // XCDR1 (max_align 8) path we keep the plain positional layout for
            // now — covered separately (FINDING E1 PL_CDR1 framing).
            out.push_str("        if reader.max_alignment() == 4 {\n");
            emit_mutable_decode_body(out, s, "reader")?;
            out.push('\n');
            out.push_str("        } else {\n");
            // FINDING E1: classic CDR / XCDR1 (max_align 8) reads the PL_CDR1
            // parameter list symmetric to the encode side — `read_pl_cdr1_member`
            // loop + per-member-id match — NOT plain positional reads. Each
            // member body is plain (no DHEADER), decoded with the reader's
            // endianness at classic (max_align 8) alignment.
            emit_pl_cdr1_decode_body(out, s, "            ")?;
            out.push_str("        }\n");
        }
    }
    out.push_str("    }\n}\n");
    Ok(())
}

/// Emits the plain `let <field> = decode(...)?;` sequence + `Ok(Self { .. })`
/// for the given reader expression and indent.
fn emit_cdr_decode_plain(
    out: &mut String,
    s: &StructDef,
    indent: &str,
    reader_expr: &str,
) -> Result<()> {
    for (_, member) in serialized_members(s) {
        emit_member_decode_let_with_reader(out, member, indent, reader_expr)?;
    }
    out.push_str(indent);
    out.push_str("::core::result::Result::Ok(Self {\n");
    emit_plain_self_fields(out, s, &format!("{indent}    "));
    out.push_str(indent);
    out.push_str("})\n");
    Ok(())
}

/// Emits the field list inside a plain (positional) decode's `Ok(Self { .. })`:
/// a serialized member takes the like-named `let` binding produced by the
/// decode-let loop, a `@non_serialized` member takes `Default::default()` since
/// it has no wire slot — its in-memory field stays at the default
/// (broad-audit P0-5, #2).
fn emit_plain_self_fields(out: &mut String, s: &StructDef, indent: &str) {
    for member in &s.members {
        let non_serialized = crate::annotations::member_is_non_serialized(&member.annotations);
        for declarator in &member.declarators {
            let field = declarator_ident(declarator);
            if non_serialized {
                out.push_str(&format!(
                    "{indent}{field}: ::core::default::Default::default(),\n"
                ));
            } else {
                out.push_str(&format!("{indent}{field},\n"));
            }
        }
    }
}

/// Total element count of an array declarator (product of its dimensions),
/// or `None` if any dimension is not a resolvable integer.
fn array_declarator_len(decl: &Declarator) -> Option<usize> {
    match decl {
        Declarator::Simple(_) => None,
        Declarator::Array(arr) => {
            let mut total: usize = 1;
            for size_expr in &arr.sizes {
                total = total.checked_mul(crate::type_map::const_expr_as_usize(size_expr)?)?;
            }
            Some(total)
        }
    }
}

/// `#[derive(Default)]` cannot be used on a struct that owns an array member
/// longer than 32: `std`'s blanket `Default` for `[T; N]` only covers `N <= 32`
/// (a language limitation, not an IDL one). Such a struct gets a hand-written
/// `Default` instead (see [`emit_manual_default`]). ROS 2 `covariance[36]`,
/// SpatialDDS `Mat6x6[36]`/`Mat12x12[144]` used as a plain member, etc.
fn struct_needs_manual_default(s: &StructDef) -> bool {
    s.members.iter().any(|m| {
        m.declarators
            .iter()
            .any(|d| array_declarator_len(d).is_some_and(|n| n > 32))
    })
}

/// Emits `impl Default` for a struct that cannot derive it. Every array field
/// is built with nested `::core::array::from_fn`, which works for any length
/// and any element type (no `Copy`/`N <= 32` requirement); all other fields
/// fall back to `Default::default()`.
fn emit_manual_default(out: &mut String, s: &StructDef) -> Result<()> {
    let name = escape_keyword(&s.name.text);
    out.push_str(&format!("impl ::core::default::Default for {name} {{\n"));
    out.push_str("    fn default() -> Self {\n");
    out.push_str("        Self {\n");
    for member in &s.members {
        for declarator in &member.declarators {
            let field = escape_keyword(&declarator.name().text);
            let expr = match declarator {
                Declarator::Array(arr) => array_default_expr(&arr.sizes),
                Declarator::Simple(_) => "::core::default::Default::default()".to_string(),
            };
            out.push_str(&format!("            {field}: {expr},\n"));
        }
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(())
}

/// A default-value expression for an array with the given (outer-first) sizes,
/// nesting `from_fn` once per dimension so it holds for arrays of any length.
fn array_default_expr(sizes: &[zerodds_idl::ast::types::ConstExpr]) -> String {
    if sizes.is_empty() {
        return "::core::default::Default::default()".to_string();
    }
    format!(
        "::core::array::from_fn(|_| {})",
        array_default_expr(&sizes[1..])
    )
}

fn emit_struct_decl(out: &mut String, s: &StructDef) -> Result<()> {
    let manual_default = struct_needs_manual_default(s);
    out.push_str("/// Generated by `zerodds-idl-rust` from IDL.\n");
    out.push_str(crate::emitter::TYPE_LINT_ALLOW);
    let mut derives = vec!["Debug", "Clone", "PartialEq"];
    if !manual_default {
        derives.push("Default");
    }
    // A23: a struct used as a `map<K,V>` key must be `Ord` to key the generated
    // `BTreeMap` (`Ord: Eq + PartialOrd`, `Eq: PartialEq`). Without these
    // derives `BTreeMap<ThisStruct, _>` does not compile.
    if crate::type_map::struct_is_map_key(&s.name.text) {
        derives.extend(["Eq", "PartialOrd", "Ord"]);
    }
    out.push_str(&format!("#[derive({})]\n", derives.join(", ")));
    out.push_str("pub struct ");
    out.push_str(&escape_keyword(&s.name.text));
    out.push_str(" {\n");
    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_DECLARATION)` as the first
    // element inside the declaration body.
    crate::verbatim::emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::BeginDeclaration);
    for member in &s.members {
        emit_member_field(out, member)?;
    }
    // §7.2.2.4.8 — `@verbatim(placement=END_DECLARATION)` as the last element.
    crate::verbatim::emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::EndDeclaration);
    out.push_str("}\n");
    if manual_default {
        out.push('\n');
        emit_manual_default(out, s)?;
    }
    Ok(())
}

fn emit_member_field(out: &mut String, member: &Member) -> Result<()> {
    let rust_ty = rust_type_for(&member.type_spec)?;
    let optional = crate::annotations::member_is_optional(&member.annotations);
    for declarator in &member.declarators {
        match declarator {
            Declarator::Simple(name) => {
                out.push_str("    pub ");
                out.push_str(&escape_keyword(&name.text));
                out.push_str(": ");
                if optional {
                    out.push_str(&format!("::core::option::Option<{rust_ty}>"));
                } else {
                    out.push_str(&rust_ty);
                }
                out.push_str(",\n");
            }
            Declarator::Array(arr) => {
                let wrapped = declarator_rust_type(&member.type_spec, declarator)?;
                out.push_str("    pub ");
                out.push_str(&escape_keyword(&arr.name.text));
                out.push_str(": ");
                if optional {
                    out.push_str(&format!("::core::option::Option<{wrapped}>"));
                } else {
                    out.push_str(&wrapped);
                }
                out.push_str(",\n");
            }
        }
    }
    Ok(())
}

fn emit_dds_type_impl(
    out: &mut String,
    s: &StructDef,
    extensibility: StructExtensibility,
    module_path: &[String],
    orig: &StructDef,
) -> Result<()> {
    let autoid_hash = crate::annotations::struct_autoid_hash(&s.annotations);
    let key_members: Vec<&Member> = s
        .members
        .iter()
        .filter(|m| crate::annotations::member_is_key(&m.annotations))
        .collect();
    let has_key = !key_members.is_empty();
    let key_holder_max_size = compute_key_holder_max_size(&key_members, autoid_hash);

    out.push_str(crate::emitter::IMPL_LINT_ALLOW);
    out.push_str("impl zerodds_dcps::DdsType for ");
    out.push_str(&escape_keyword(&s.name.text));
    out.push_str(" {\n");
    // TYPE_NAME is the fully qualified IDL scoped name
    // (`Module::Sub::Struct`). Spec: zerodds-xcdr2-bindings-conformance
    // §3 / §5 + V-7 in conjunction with XTypes 1.3 §7.3.4.6. Strictly the
    // OMG IDL format with a `::` separator, NOT Rust syntax.
    out.push_str("    const TYPE_NAME: &'static str = \"");
    for module in module_path {
        out.push_str(module);
        out.push_str("::");
    }
    out.push_str(&s.name.text);
    out.push_str("\";\n");
    // EXTENSIBILITY-Const (zerodds-xcdr2-rust §2.3 + §6).
    let ext_variant = match extensibility {
        StructExtensibility::Final => "Final",
        StructExtensibility::Appendable => "Appendable",
        StructExtensibility::Mutable => "Mutable",
    };
    out.push_str(&format!(
        "    const EXTENSIBILITY: zerodds_dcps::Extensibility = zerodds_dcps::Extensibility::{ext_variant};\n"
    ));
    if has_key {
        out.push_str("    const HAS_KEY: bool = true;\n");
        if let Some(size) = key_holder_max_size {
            out.push_str(&format!(
                "    const KEY_HOLDER_MAX_SIZE: ::core::option::Option<usize> = ::core::option::Option::Some({size});\n"
            ));
        }
    }
    if crate::annotations::struct_is_nested(&s.annotations) {
        out.push_str("    const IS_NESTED: bool = true;\n");
    }
    // F-TYPES-3: TYPE_IDENTIFIER (XTypes 1.3 §7.3.4.2) as a const.
    // Codegen-time-computed EquivalenceHash of the `CompleteStructType`,
    // if all member types are leaf-resolvable; otherwise `None`. Computed from
    // the ORIGINAL struct (with its `base`), not the flattened member list, so
    // the hash matches the shared frontend / idl-cpp (A10).
    let type_id_expr = crate::type_identifier::struct_type_identifier_expr(orig);
    out.push_str(&format!(
        "    const TYPE_IDENTIFIER: zerodds_types::TypeIdentifier = {type_id_expr};\n"
    ));
    out.push('\n');

    // encode
    out.push_str("    fn encode(&self, out: &mut ::std::vec::Vec<u8>) -> ::core::result::Result<(), zerodds_dcps::EncodeError> {\n");
    out.push_str(
        "        let mut writer = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Little).xcdr2();\n",
    );
    emit_encode_body(out, s, extensibility)?;
    out.push_str("        out.extend_from_slice(&writer.into_bytes());\n");
    out.push_str("        Ok(())\n");
    out.push_str("    }\n\n");

    // encode_be — big-endian mirror of `encode` (the encapsulation header
    // declares a *_BE representation id: CDR2_BE 0x06 / D_CDR2_BE 0x08 /
    // PL_CDR2_BE 0x0a). Same body, big-endian writer; the `struct_enc` helpers
    // thread the byte order through the DHEADER/EMHEADER and every member write,
    // symmetric to `decode_be`. Lets the DataWriter emit BE on the wire (not just
    // read it).
    out.push_str("    fn encode_be(&self, out: &mut ::std::vec::Vec<u8>) -> ::core::result::Result<(), zerodds_dcps::EncodeError> {\n");
    out.push_str(
        "        let mut writer = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Big).xcdr2();\n",
    );
    emit_encode_body(out, s, extensibility)?;
    out.push_str("        out.extend_from_slice(&writer.into_bytes());\n");
    out.push_str("        Ok(())\n");
    out.push_str("    }\n\n");

    // decode
    out.push_str(
        "    fn decode(bytes: &[u8]) -> ::core::result::Result<Self, zerodds_dcps::DecodeError> {\n",
    );
    out.push_str("        let mut reader = zerodds_cdr::BufferReader::new(bytes, zerodds_cdr::Endianness::Little).xcdr2();\n");
    emit_decode_body(out, s, extensibility)?;
    out.push_str("    }\n");

    // decode_be — big-endian payload (encapsulation header declared a *_BE
    // representation identifier). Same body, big-endian reader; the reader
    // threads the byte order through every read incl. @mutable + wstring.
    out.push('\n');
    out.push_str(
        "    fn decode_be(bytes: &[u8]) -> ::core::result::Result<Self, zerodds_dcps::DecodeError> {\n",
    );
    out.push_str("        let mut reader = zerodds_cdr::BufferReader::new(bytes, zerodds_cdr::Endianness::Big).xcdr2();\n");
    emit_decode_body(out, s, extensibility)?;
    out.push_str("    }\n");

    // encode_xcdr1 / decode_xcdr1 — classic CDR (XCDR1, max-alignment 8, no
    // DHEADER on @final/@appendable, PL_CDR1 for @mutable). This delegates to
    // the low-level CdrEncode/CdrDecode, whose body is representation-aware
    // (it branches on the writer/reader max_alignment), rather than the XCDR2
    // DdsType body above which always frames @appendable/@mutable with a
    // DHEADER. This is what Cyclone DDS carries in an iceoryx PSMX chunk.
    out.push('\n');
    out.push_str("    fn encode_xcdr1(&self, out: &mut ::std::vec::Vec<u8>) -> ::core::result::Result<(), zerodds_dcps::EncodeError> {\n");
    out.push_str(
        "        let mut writer = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Little);\n",
    );
    out.push_str("        <Self as zerodds_cdr::CdrEncode>::encode(self, &mut writer)?;\n");
    out.push_str("        out.extend_from_slice(&writer.into_bytes());\n");
    out.push_str("        Ok(())\n");
    out.push_str("    }\n\n");
    out.push_str(
        "    fn decode_xcdr1(bytes: &[u8]) -> ::core::result::Result<Self, zerodds_dcps::DecodeError> {\n",
    );
    out.push_str("        let mut reader = zerodds_cdr::BufferReader::new(bytes, zerodds_cdr::Endianness::Little);\n");
    out.push_str(
        "        <Self as zerodds_cdr::CdrDecode>::decode(&mut reader).map_err(::core::convert::Into::into)\n",
    );
    out.push_str("    }\n");

    if has_key {
        out.push('\n');
        emit_key_holder_be(out, &key_members, autoid_hash)?;
    }

    out.push('\n');
    emit_field_value(out, s)?;

    out.push_str("}\n");
    Ok(())
}

/// Emittiert `fn field_value(&self, path: &str) -> Option<zerodds_dcps::FilterValue>`
/// for SQL-filter evaluation (QueryCondition / ContentFilteredTopic).
///
/// DDS 1.4 §B.2.1: filter expressions reference field values via
/// dotted paths (e.g. `"sensor.id"`). Rust DataTypes implement this here
/// as a deterministic match-arm table.
fn emit_field_value(out: &mut String, s: &StructDef) -> Result<()> {
    out.push_str(
        "    fn field_value(&self, path: &str) -> ::core::option::Option<zerodds_dcps::FilterValue> {\n",
    );
    out.push_str("        match path {\n");
    for member in &s.members {
        let optional = crate::annotations::member_is_optional(&member.annotations);
        for declarator in &member.declarators {
            let name = declarator_ident(declarator);
            emit_field_value_arm(out, &member.type_spec, &name, declarator, optional)?;
        }
    }
    out.push_str("            _ => ::core::option::Option::None,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn emit_field_value_arm(
    out: &mut String,
    spec: &TypeSpec,
    name: &str,
    declarator: &Declarator,
    optional: bool,
) -> Result<()> {
    use zerodds_idl::ast::types::{FloatingType, PrimitiveType};

    // Array fields cannot be mapped directly to Value — filter
    // expressions reference element indices (`"arr[0]"`), which is
    // outside the current RC1 scope. We emit nothing and the `_`
    // fallback kicks in.
    if matches!(declarator, Declarator::Array(_)) {
        return Ok(());
    }

    // Scoped field types need resolution: a struct member has `field_value()`
    // and forwards the dotted path, but an enum or a typedef-to-primitive is a
    // terminal value — forwarding those to `.field_value()` does not compile.
    if let TypeSpec::Scoped(s) = spec {
        use crate::type_map::{FieldValueScopedKind as K, field_value_scoped_kind};
        match field_value_scoped_kind(s) {
            K::Struct => {
                // An `@optional` nested struct is `Option<T>`, which has no
                // `field_value`. Forward through the Option: an absent optional
                // (None) yields None (the field has no value), a present one
                // recurses into the inner struct.
                let body = if optional {
                    format!(
                        "self.{name}.as_ref().and_then(|v| v.field_value(&p[\"{name}.\".len()..]))"
                    )
                } else {
                    format!("self.{name}.field_value(&p[\"{name}.\".len()..])")
                };
                out.push_str(&format!(
                    "            p if p.starts_with(\"{name}.\") => {body},\n"
                ));
            }
            // typedef → primitive/string: emit the leaf value of the resolved type.
            K::Leaf(leaf) => return emit_field_value_arm(out, &leaf, name, declarator, optional),
            // C-like enum → terminal integer.
            K::Enum => {
                let expr = if optional {
                    format!(
                        "self.{name}.as_ref().map(|v| zerodds_dcps::FilterValue::Int(*v as i64))"
                    )
                } else {
                    format!(
                        "::core::option::Option::Some(zerodds_dcps::FilterValue::Int(self.{name} as i64))"
                    )
                };
                out.push_str(&format!("            \"{name}\" => {expr},\n"));
            }
            // Unresolvable / not filterable → caller's `_` fallback.
            K::Unknown => {}
        }
        return Ok(());
    }

    // Optional-Pattern:
    //     self.x.as_ref().map(|v| Value::Int(*v as i64))
    // Non-Optional:
    //     ::core::option::Option::Some(Value::Int(self.x as i64))
    let value_expr: Option<String> = match spec {
        TypeSpec::Primitive(
            PrimitiveType::Integer(_)
            | PrimitiveType::Octet
            | PrimitiveType::Char
            | PrimitiveType::WideChar,
        ) => Some(if optional {
            format!("self.{name}.as_ref().map(|v| zerodds_dcps::FilterValue::Int(*v as i64))")
        } else {
            format!(
                "::core::option::Option::Some(zerodds_dcps::FilterValue::Int(self.{name} as i64))"
            )
        }),
        TypeSpec::Primitive(PrimitiveType::Floating(FloatingType::Float)) => Some(if optional {
            format!("self.{name}.as_ref().map(|v| zerodds_dcps::FilterValue::Float(*v as f64))")
        } else {
            format!(
                "::core::option::Option::Some(zerodds_dcps::FilterValue::Float(self.{name} as f64))"
            )
        }),
        TypeSpec::Primitive(PrimitiveType::Floating(_)) => Some(if optional {
            format!("self.{name}.as_ref().map(|v| zerodds_dcps::FilterValue::Float(*v))")
        } else {
            format!("::core::option::Option::Some(zerodds_dcps::FilterValue::Float(self.{name}))")
        }),
        TypeSpec::Primitive(PrimitiveType::Boolean) => Some(if optional {
            format!("self.{name}.as_ref().map(|v| zerodds_dcps::FilterValue::Bool(*v))")
        } else {
            format!("::core::option::Option::Some(zerodds_dcps::FilterValue::Bool(self.{name}))")
        }),
        // A narrow `string` is a Rust `String` (clone directly); a wide
        // `wstring` is `zerodds_cdr::WString` (a newtype over `String`), so
        // `.clone()` would yield a `WString`, not the `String` a
        // `Value::String` needs — go through `.as_str().to_string()`.
        TypeSpec::String(s) => {
            let to_string = |v: &str| -> String {
                if s.wide {
                    format!("{v}.as_str().to_string()")
                } else {
                    format!("{v}.clone()")
                }
            };
            Some(if optional {
                let inner = to_string("v");
                format!("self.{name}.as_ref().map(|v| zerodds_dcps::FilterValue::String({inner}))")
            } else {
                let inner = to_string(&format!("self.{name}"));
                format!("::core::option::Option::Some(zerodds_dcps::FilterValue::String({inner}))")
            })
        }
        // Scoped is resolved and fully handled by the early return above.
        TypeSpec::Scoped(_) => None,
        // Sequence / array / optional: no direct value conversion.
        TypeSpec::Sequence(_) | TypeSpec::Fixed(_) | TypeSpec::Map(_) | TypeSpec::Any => None,
    };

    if let Some(expr) = value_expr {
        out.push_str(&format!("            \"{name}\" => {expr},\n"));
    }
    Ok(())
}

/// Computes the static `KEY_HOLDER_MAX_SIZE` — the maximum serialized size of
/// the big-endian PLAIN_CDR2 KeyHolder (XTypes 1.3 §7.6.8.4 step 5 /
/// DDSI-RTPS §9.6.3.8). The branch decision (≤16 → zero-pad, >16 → MD5) is
/// taken on THIS value, so it must mirror exactly what
/// `PlainCdr2BeKeyHolder` writes: members in ascending member-id order, each
/// primitive aligned to `min(size, 4)` before it is written (FINDING F3 — the
/// old code summed raw sizes and ignored inter-member padding, so a type
/// whose ALIGNED max crosses 16 wrongly took the zero-pad branch).
///
/// Nested-struct `@key` members are expanded recursively into their own
/// `@key` members (FINDING F2). Returns `None` (→ MD5) if any `@key` member is
/// dynamically sized (unbounded string, sequence, map, …).
fn compute_key_holder_max_size(key_members: &[&Member], autoid_hash: bool) -> Option<usize> {
    // member-id order, matching `encode_key_holder_be`.
    let mut ordered: Vec<(u32, &Member)> = key_members
        .iter()
        .enumerate()
        .map(|(idx, m)| (member_wire_id(autoid_hash, m, idx), *m))
        .collect();
    ordered.sort_by_key(|(id, _)| *id);

    let mut offset = 0usize;
    for (_, member) in &ordered {
        for declarator in &member.declarators {
            let count = match declarator {
                Declarator::Simple(_) => 1,
                Declarator::Array(arr) => {
                    let mut n = 1usize;
                    for size_expr in &arr.sizes {
                        let dim = crate::type_map::const_expr_as_usize(size_expr)?;
                        n = n.checked_mul(dim)?;
                    }
                    n
                }
            };
            for _ in 0..count {
                offset = key_holder_atom_size(&member.type_spec, offset)?;
            }
        }
    }
    Some(offset)
}

/// Advances a running KeyHolder byte offset by one occurrence of a `@key`
/// member of type `spec`, applying the BE PLAIN_CDR2 alignment
/// `PlainCdr2BeKeyHolder` uses (max align 4) BEFORE the value. Recurses into
/// nested `@key` structs (FINDING F2). Returns `None` for dynamically sized
/// types (→ MD5 branch).
/// zerodds-lint: recursion-depth 16
fn key_holder_atom_size(spec: &TypeSpec, offset: usize) -> Option<usize> {
    let pad_to = |off: usize, align: usize| -> usize { off + (align - (off % align)) % align };
    match spec {
        TypeSpec::Primitive(p) => {
            let size = crate::type_map::primitive_wire_size(*p);
            let align = size.clamp(1, 4);
            Some(pad_to(offset, align).checked_add(size)?)
        }
        TypeSpec::String(s) => {
            // Bounded string only (unbounded → MD5). Aligned to 4 (length
            // prefix), then `4 + N + 1` (narrow) / `4 + 2N` (wide) bytes.
            let size = crate::type_map::wire_size_bound(spec)?;
            let _ = s;
            Some(pad_to(offset, 4).checked_add(size)?)
        }
        TypeSpec::Scoped(scoped) => {
            // A typedef alias: dealias first (FINDING #20 fix) and recurse on
            // the resolved type — typedef-to-primitive/string then lands in
            // the Primitive/String arms above, typedef-to-struct lands back
            // here with the terminal struct name (struct_def_by_scoped
            // below), typedef-to-seq/map/fixed correctly falls to `None`
            // (MD5) via the matching arm for that TypeSpec variant.
            if let Some(resolved) = crate::type_map::resolve_typedef_to_spec(scoped) {
                return key_holder_atom_size(&resolved, offset);
            }
            // Nested-struct @key: expand its own @key members in member-id
            // order (FINDING F2). A scoped non-struct (genuine enum/unresolved
            // name) is keyed as its underlying integer — but those don't
            // appear as @key aggregate members in the proof corpus; fall
            // back to MD5 (None) if it is not a resolvable struct.
            let sd = crate::type_map::struct_def_by_scoped(scoped)?;
            let nested_keys: Vec<&Member> = sd
                .members
                .iter()
                .filter(|m| crate::annotations::member_is_key(&m.annotations))
                .collect();
            // A nested struct with NO @key members keys on ALL its members
            // (XTypes 1.3 §7.6.8: an aggregate with no key members is keyed
            // in full). Match that.
            let effective: Vec<&Member> = if nested_keys.is_empty() {
                sd.members.iter().collect()
            } else {
                nested_keys
            };
            let nested_autoid = crate::annotations::struct_autoid_hash(&sd.annotations);
            let mut ordered: Vec<(u32, &Member)> = effective
                .iter()
                .enumerate()
                .map(|(idx, m)| (member_wire_id(nested_autoid, m, idx), *m))
                .collect();
            ordered.sort_by_key(|(id, _)| *id);
            let mut off = offset;
            for (_, m) in &ordered {
                for decl in &m.declarators {
                    let count = match decl {
                        Declarator::Simple(_) => 1,
                        Declarator::Array(arr) => {
                            let mut n = 1usize;
                            for e in &arr.sizes {
                                n = n.checked_mul(crate::type_map::const_expr_as_usize(e)?)?;
                            }
                            n
                        }
                    };
                    for _ in 0..count {
                        off = key_holder_atom_size(&m.type_spec, off)?;
                    }
                }
            }
            Some(off)
        }
        // sequence / fixed / map / any @key → dynamically sized → MD5.
        _ => None,
    }
}

fn emit_key_holder_be(out: &mut String, key_members: &[&Member], autoid_hash: bool) -> Result<()> {
    out.push_str(
        "    fn encode_key_holder_be(&self, holder: &mut zerodds_cdr::PlainCdr2BeKeyHolder) {\n",
    );
    // Spec: XTypes 1.3 §7.6.8.3.1.b — members sorted in member-id order.
    // Positional IDs are the default; `@id(N)`/`@hashid`/`@autoid(HASH)` override.
    let mut ordered: Vec<(u32, &Member)> = key_members
        .iter()
        .enumerate()
        .map(|(idx, m)| (member_wire_id(autoid_hash, m, idx), *m))
        .collect();
    ordered.sort_by_key(|(id, _)| *id);
    for (_, member) in &ordered {
        for declarator in &member.declarators {
            let name = declarator_ident(declarator);
            emit_key_field_write(out, &member.type_spec, &format!("self.{name}"))?;
        }
    }
    out.push_str("    }\n");
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn emit_key_field_write(out: &mut String, spec: &TypeSpec, value_expr: &str) -> Result<()> {
    use zerodds_idl::ast::types::{FloatingType, IntegerType, PrimitiveType};
    match spec {
        TypeSpec::Primitive(p) => {
            let method = match p {
                PrimitiveType::Integer(IntegerType::Int8) => "write_i8",
                PrimitiveType::Integer(IntegerType::UInt8) | PrimitiveType::Octet => "write_u8",
                PrimitiveType::Integer(IntegerType::Short | IntegerType::Int16) => "write_i16",
                PrimitiveType::Integer(IntegerType::UShort | IntegerType::UInt16) => "write_u16",
                PrimitiveType::Integer(IntegerType::Long | IntegerType::Int32) => "write_i32",
                PrimitiveType::Integer(IntegerType::ULong | IntegerType::UInt32) => "write_u32",
                PrimitiveType::Integer(IntegerType::LongLong | IntegerType::Int64) => "write_i64",
                PrimitiveType::Integer(IntegerType::ULongLong | IntegerType::UInt64) => "write_u64",
                PrimitiveType::Floating(FloatingType::Float) => "write_f32",
                PrimitiveType::Floating(FloatingType::Double | FloatingType::LongDouble) => {
                    "write_f64"
                }
                PrimitiveType::Boolean => "write_u8",
                PrimitiveType::Char => "write_u8",
                PrimitiveType::WideChar => "write_u16",
            };
            if matches!(
                p,
                PrimitiveType::Char | PrimitiveType::WideChar | PrimitiveType::Boolean
            ) {
                out.push_str(&format!("        holder.{method}({value_expr} as _);\n"));
            } else {
                out.push_str(&format!("        holder.{method}({value_expr});\n"));
            }
        }
        TypeSpec::String(_) => {
            // PlainCdr2BeKeyHolder has a `write_string` method that
            // inserts the UTF-8 bytes directly (spec §7.6.8.4 — strings
            // carry their length prefix in big-endian).
            out.push_str(&format!("        holder.write_string(&{value_expr});\n"));
        }
        TypeSpec::Scoped(scoped) => {
            // A typedef alias: dealias first (FINDING #20 fix) and recurse on
            // the resolved type — typedef-to-primitive/string then emits via
            // the Primitive/String arms above, typedef-to-struct lands back
            // here with the terminal struct name (struct_def_by_scoped
            // below), typedef-to-seq/map/fixed correctly falls into the
            // matching, correctly-labeled error arm for that TypeSpec variant
            // instead of the generic "enum or unresolved nested type" one.
            if let Some(resolved) = crate::type_map::resolve_typedef_to_spec(scoped) {
                return emit_key_field_write(out, &resolved, value_expr);
            }
            // FINDING F2: a nested-struct `@key` member expands recursively
            // into the nested struct's own `@key` members, in member-id order
            // (XTypes 1.3 §7.6.8 step 3) — exactly what CycloneDDS / RTI /
            // FastDDS generate. A nested struct with NO `@key` members is
            // keyed in full (all members). The runtime `PlainCdr2BeKeyHolder`
            // already writes them; we just emit the field accessors.
            if let Some(sd) = crate::type_map::struct_def_by_scoped(scoped) {
                let nested_keys: Vec<&Member> = sd
                    .members
                    .iter()
                    .filter(|m| crate::annotations::member_is_key(&m.annotations))
                    .collect();
                let effective: Vec<&Member> = if nested_keys.is_empty() {
                    sd.members.iter().collect()
                } else {
                    nested_keys
                };
                let nested_autoid = crate::annotations::struct_autoid_hash(&sd.annotations);
                let mut ordered: Vec<(u32, &Member)> = effective
                    .iter()
                    .enumerate()
                    .map(|(idx, m)| (member_wire_id(nested_autoid, m, idx), *m))
                    .collect();
                ordered.sort_by_key(|(id, _)| *id);
                for (_, m) in &ordered {
                    for decl in &m.declarators {
                        // Arrays of nested-key structs are out of the proof
                        // scope; reject explicitly rather than silently
                        // dropping dimensions.
                        if matches!(decl, Declarator::Array(_)) {
                            return Err(RustGenError::Unsupported {
                                what: "array @key field inside a nested-struct key",
                                at: 0,
                            });
                        }
                        let field = declarator_ident(decl);
                        emit_key_field_write(out, &m.type_spec, &format!("{value_expr}.{field}"))?;
                    }
                }
                return Ok(());
            }
            // Not a resolvable struct (enum / unresolved scoped) — leave the
            // complex-key error so the gap is explicit, not silently wrong.
            return Err(RustGenError::Unsupported {
                what: "complex @key field (enum or unresolved nested type)",
                at: 0,
            });
        }
        TypeSpec::Sequence(_) => {
            return Err(RustGenError::Unsupported {
                what: "complex @key field (sequence)",
                at: 0,
            });
        }
        TypeSpec::Fixed(f) => {
            return Err(RustGenError::Unsupported {
                what: "fixed @key",
                at: f.span.start,
            });
        }
        TypeSpec::Map(m) => {
            return Err(RustGenError::Unsupported {
                what: "map @key",
                at: m.span.start,
            });
        }
        TypeSpec::Any => {
            return Err(RustGenError::Unsupported {
                what: "any @key",
                at: 0,
            });
        }
    }
    Ok(())
}

fn emit_encode_body(
    out: &mut String,
    s: &StructDef,
    extensibility: StructExtensibility,
) -> Result<()> {
    match extensibility {
        StructExtensibility::Final => {
            for (_, member) in serialized_members(s) {
                emit_member_encode(out, member, "        ")?;
            }
        }
        StructExtensibility::Appendable => {
            // Phase C: zerodds_cdr::struct_enc::encode_appendable
            out.push_str("        zerodds_cdr::struct_enc::encode_appendable(&mut writer, |w| {\n");
            for (_, member) in serialized_members(s) {
                emit_member_encode_with_writer(out, member, "            ", "w")?;
            }
            out.push_str("            Ok(())\n");
            out.push_str("        })?;\n");
        }
        StructExtensibility::Mutable => {
            // XTypes 1.3 §7.4.3.4.4: @mutable structs MUST be wrapped
            // in a DHEADER frame so nested-mutable readers can skip-by-
            // length when they encounter unknown member-ids. The
            // decode side already uses `decode_appendable` to strip
            // the DHEADER, so the encode side must symmetric-wrap.
            // Spec anchor: zerodds-xcdr2-bindings-conformance §6 V-10
            // (`14 00 00 00` DHEADER + member list).
            let autoid = crate::annotations::struct_autoid_hash(&s.annotations);
            let required_ids: Vec<u32> = serialized_members(s)
                .filter(|(_, m)| !crate::annotations::member_is_optional(&m.annotations))
                .map(|(idx, m)| member_wire_id(autoid, m, idx))
                .collect();
            let required_list = required_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str("        zerodds_cdr::struct_enc::encode_appendable(&mut writer, |w| {\n");
            out.push_str(&format!(
                "            let mut enc = zerodds_cdr::struct_enc::MutableStructEncoder::new(w, ::std::vec![{required_list}]);\n"
            ));
            for (idx, member) in serialized_members(s) {
                emit_mutable_member_encode(out, member, idx, autoid, "            ")?;
            }
            out.push_str("            enc.finish()?;\n");
            out.push_str("            Ok(())\n");
            out.push_str("        })?;\n");
        }
    }
    Ok(())
}

fn emit_member_encode(out: &mut String, member: &Member, indent: &str) -> Result<()> {
    emit_member_encode_with_writer(out, member, indent, "&mut writer")
}

pub(crate) fn emit_member_encode_with_writer(
    out: &mut String,
    member: &Member,
    indent: &str,
    writer_expr: &str,
) -> Result<()> {
    let optional = crate::annotations::member_is_optional(&member.annotations);
    for declarator in &member.declarators {
        let name = declarator_ident(declarator);
        out.push_str(indent);
        emit_member_bound_checks(out, member, declarator);
        emit_field_encode_with_optional(
            out,
            &member.type_spec,
            &format!("self.{name}"),
            writer_expr,
            optional,
        )?;
        out.push('\n');
    }
    Ok(())
}

/// Emits a `zerodds_cdr::CdrEncode::encode` call for a field.
///
/// **Uniform trait API** — all primitives + composite types
/// (String, Vec, [T;N], Option) have `impl CdrEncode` in
/// `zerodds_cdr::encode` / `zerodds_cdr::composite`. The codegen thus
/// need not distinguish between primitive method calls and function
/// calls.
pub fn emit_field_encode(
    out: &mut String,
    spec: &TypeSpec,
    value_expr: &str,
    writer_expr: &str,
) -> Result<()> {
    emit_field_encode_with_optional(out, spec, value_expr, writer_expr, false)
}

/// Like [`emit_field_encode`] but knows whether the member is `@optional`,
/// which only matters for the inline `wstring` path (every other type's
/// `Option<T>` wrapper is handled by the blanket `impl CdrEncode for
/// Option<T>` reached through the uniform trait call).
pub fn emit_field_encode_with_optional(
    out: &mut String,
    spec: &TypeSpec,
    value_expr: &str,
    writer_expr: &str,
    optional: bool,
) -> Result<()> {
    // `wstring` member: emit the XCDR2 wide-string serialization INLINE
    // rather than going through `WString::encode` (which is the CORBA/GIOP
    // form: a 0xFEFF byte-order mark prepended, counted in the length —
    // GIOP 1.2 §15.3.2.7 + §15.3.1.6). The DDS XCDR2 PSM (OMG XTypes 1.3
    // §7.4.1.2.4 / Table 11) serializes a `wstring` as a `UInt32` length
    // **in octets** followed by the UTF-16 code units, with **no BOM and no
    // terminator** — confirmed by the cross-PSM contract in
    // `_interop/CANONICAL.md` ("uint32 byte-length, then the code units, no
    // terminator"). The 4-byte length is already 2-aligned, so the u16
    // units need no extra alignment. This keeps the GIOP `WString` codec
    // untouched (CORBA still relies on it) while the wire form matches every
    // other PSM byte-for-byte.
    if let TypeSpec::String(s) = spec {
        if s.wide {
            if optional {
                // `@optional wstring` (XTypes §7.4.5.1.4): uint8 present flag
                // then the value if present. `value_expr` is `Option<WString>`.
                out.push_str(&format!(
                    "match &{value_expr} {{ ::core::option::Option::Some(__w) => {{ zerodds_cdr::BufferWriter::write_u8({writer_expr}, 1)?; "
                ));
                emit_xcdr2_wstring_encode(out, "(*__w)", writer_expr);
                out.push_str(&format!(
                    " }} ::core::option::Option::None => {{ zerodds_cdr::BufferWriter::write_u8({writer_expr}, 0)?; }} }}"
                ));
            } else {
                emit_xcdr2_wstring_encode(out, value_expr, writer_expr);
            }
            return Ok(());
        }
    }
    // Uniform trait path for primitives + composite + scoped + Fixed +
    // Map + Any (all have CdrEncode impls). Bound enforcement
    // (DDS-XTypes §7.4.3) runs separately at the declarator level via
    // `emit_member_bound_checks` — there it is known whether the field is
    // an array (otherwise `value_expr.len()` would wrongly check the
    // array length instead of the sequence length).
    out.push_str(&format!(
        "<_ as zerodds_cdr::CdrEncode>::encode(&{value_expr}, {writer_expr})?;"
    ));
    Ok(())
}

/// Emits the XCDR2 wide-string ENCODE (OMG XTypes 1.3 §7.4.1.2.4): a
/// `UInt32` octet-length (= UTF-16 code-unit count × 2, NO BOM, NO
/// terminator) followed by the UTF-16LE code units. `value_expr` is a
/// `zerodds_cdr::WString` (newtype over `String`); `writer_expr` is the
/// `&mut BufferWriter` receiver.
fn emit_xcdr2_wstring_encode(out: &mut String, value_expr: &str, writer_expr: &str) {
    if crate::type_map::any_target_corba() {
        // CORBA/GIOP target: route through the runtime `WString` codec whose
        // BOM behaviour is configurable (`zerodds_cdr::set_corba_wstring_bom`,
        // default omniORB/TAO BOM form). This resolves the prior
        // inconsistency where struct `wstring` members used the no-BOM XCDR2
        // form while a standalone `WString` used the BOM form. The XCDR2 PSM
        // branch below stays no-BOM (validated cross-vendor).
        out.push_str(&format!(
            "<zerodds_cdr::WString as zerodds_cdr::CdrEncode>::encode(&{value_expr}, {writer_expr})?"
        ));
        return;
    }
    out.push_str(&format!(
        "{{ let __units = {value_expr}.as_str().encode_utf16().count(); \
         let __octets = u32::try_from(__units.checked_mul(2).ok_or(zerodds_cdr::EncodeError::ValueOutOfRange {{ message: \"wstring length overflow\" }})?).map_err(|_| zerodds_cdr::EncodeError::ValueOutOfRange {{ message: \"wstring length exceeds u32::MAX\" }})?; \
         zerodds_cdr::BufferWriter::write_u32({writer_expr}, __octets)?; \
         for __u in {value_expr}.as_str().encode_utf16() {{ zerodds_cdr::BufferWriter::write_u16({writer_expr}, __u)?; }} }}"
    ));
}

/// Emits the XCDR2 wide-string DECODE (OMG XTypes 1.3 §7.4.1.2.4): read a
/// `UInt32` octet-length, then that many bytes interpreted as UTF-16 code
/// units **in the message byte order** (NO BOM expected on the XCDR2 PSM wire —
/// see `_interop/CANONICAL.md`), reconstructing a `zerodds_cdr::WString`. The
/// units must be read in the stream's endianness to mirror the `write_u16`
/// encode path — a big-endian stream carries big-endian units.
/// `reader_expr` is the `&mut BufferReader` receiver.
fn emit_xcdr2_wstring_decode(out: &mut String, reader_expr: &str) {
    if crate::type_map::any_target_corba() {
        // CORBA/GIOP target: the runtime `WString` decoder is BOM-tolerant
        // (accepts both the omniORB/TAO BOM form and the JacORB no-BOM form),
        // so it round-trips whatever `emit_xcdr2_wstring_encode` produced for
        // CORBA regardless of the configured BOM policy.
        out.push_str(&format!(
            "<zerodds_cdr::WString as zerodds_cdr::CdrDecode>::decode({reader_expr})?"
        ));
        return;
    }
    out.push_str(&format!(
        "{{ let __octets = zerodds_cdr::BufferReader::read_u32({reader_expr})? as usize; \
         if __octets % 2 != 0 {{ return ::core::result::Result::Err(::core::convert::Into::into(zerodds_cdr::DecodeError::LengthExceeded {{ announced: __octets, remaining: zerodds_cdr::BufferReader::remaining({reader_expr}), offset: zerodds_cdr::BufferReader::position({reader_expr}) }})); }} \
         let __off = zerodds_cdr::BufferReader::position({reader_expr}); \
         let __be = matches!(zerodds_cdr::BufferReader::endianness({reader_expr}), zerodds_cdr::Endianness::Big); \
         let __bytes = zerodds_cdr::BufferReader::read_bytes({reader_expr}, __octets)?; \
         let mut __units: ::std::vec::Vec<u16> = ::std::vec::Vec::with_capacity(__octets / 2); \
         let mut __i = 0usize; while __i + 1 < __octets {{ let __p = [__bytes[__i], __bytes[__i + 1]]; __units.push(if __be {{ u16::from_be_bytes(__p) }} else {{ u16::from_le_bytes(__p) }}); __i += 2; }} \
         let __s = ::std::string::String::from_utf16(&__units).map_err(|_| zerodds_cdr::DecodeError::InvalidUtf8 {{ offset: __off }})?; \
         zerodds_cdr::WString(__s) }}"
    ));
}

/// `true` if `spec` itself is a bounded sequence/string with a known
/// (const-evaluable) bound — or recursively contains one (through
/// sequence nesting). Structs/scoped types do NOT count: their own
/// `CdrEncode` impl enforces their field bounds themselves once the
/// element is encoded. Controls whether `emit_bound_checks` emits loops.
fn bound_is_known(bound: &Option<zerodds_idl::ast::types::ConstExpr>) -> bool {
    bound
        .as_ref()
        .and_then(crate::type_map::const_expr_as_usize)
        .is_some()
}

/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn type_has_bounds(spec: &TypeSpec) -> bool {
    match spec {
        TypeSpec::Sequence(seq) => bound_is_known(&seq.bound) || type_has_bounds(&seq.elem),
        // narrow `string<N>` AND wide `wstring<N>` are both enforced now.
        TypeSpec::String(s) => bound_is_known(&s.bound),
        TypeSpec::Map(m) => {
            bound_is_known(&m.bound) || type_has_bounds(&m.value) || type_has_bounds(&m.key)
        }
        _ => false,
    }
}

/// Emits recursive bound checks (DDS-XTypes §7.4.3) for `value_expr` of
/// type `spec`: a bounded `sequence<T,N>` / `string<N>` / `wstring<N>` /
/// `map<K,V,N>` longer than `N` is an ENCODE error
/// (`EncodeError::ValueOutOfRange`) — strict vendors (OpenDDS) reject it
/// on the wire. Descends through sequence/map nesting
/// (`sequence<sequence<octet,4>>` checks both levels, `map<string,seq<T,4>,8>`
/// checks the map bound + each value). `depth` makes the loop variables unique.
///
/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn emit_bound_checks(out: &mut String, spec: &TypeSpec, value_expr: &str, depth: usize) {
    match spec {
        TypeSpec::Sequence(seq) => {
            if let Some(n) = seq
                .bound
                .as_ref()
                .and_then(crate::type_map::const_expr_as_usize)
            {
                out.push_str(&format!(
                    "if {value_expr}.len() > {n} {{ return Err(zerodds_cdr::EncodeError::ValueOutOfRange {{ message: \"bounded sequence length exceeds its IDL bound ({n})\" }}.into()); }} "
                ));
            }
            if type_has_bounds(&seq.elem) {
                let item = format!("__b{depth}");
                out.push_str(&format!("for {item} in {value_expr}.iter() {{ "));
                emit_bound_checks(out, &seq.elem, &item, depth + 1);
                out.push_str("} ");
            }
        }
        TypeSpec::String(s) => {
            if let Some(n) = s
                .bound
                .as_ref()
                .and_then(crate::type_map::const_expr_as_usize)
            {
                if s.wide {
                    // `wstring<N>`: bound is in wide characters = UTF-16 code
                    // units (GIOP 1.2 §15.3.2.7 encodes UTF-16). `WString`
                    // wraps a `String`; count its UTF-16 units.
                    out.push_str(&format!(
                        "if {value_expr}.as_str().encode_utf16().count() > {n} {{ return Err(zerodds_cdr::EncodeError::ValueOutOfRange {{ message: \"bounded wstring length exceeds its IDL bound ({n})\" }}.into()); }} "
                    ));
                } else {
                    // narrow `string<N>`: CDR encodes byte-wise (`as_bytes`),
                    // so the bound guards the UTF-8 byte length (ASCII = chars).
                    out.push_str(&format!(
                        "if {value_expr}.len() > {n} {{ return Err(zerodds_cdr::EncodeError::ValueOutOfRange {{ message: \"bounded string length exceeds its IDL bound ({n})\" }}.into()); }} "
                    ));
                }
            }
        }
        TypeSpec::Map(m) => {
            if let Some(n) = m
                .bound
                .as_ref()
                .and_then(crate::type_map::const_expr_as_usize)
            {
                out.push_str(&format!(
                    "if {value_expr}.len() > {n} {{ return Err(zerodds_cdr::EncodeError::ValueOutOfRange {{ message: \"bounded map length exceeds its IDL bound ({n})\" }}.into()); }} "
                ));
            }
            // Recurse into bounded map values + keys (`map<string<4>, seq<T,8>>`).
            if type_has_bounds(&m.value) {
                let item = format!("__b{depth}");
                out.push_str(&format!("for {item} in {value_expr}.values() {{ "));
                emit_bound_checks(out, &m.value, &item, depth + 1);
                out.push_str("} ");
            }
            if type_has_bounds(&m.key) {
                let item = format!("__k{depth}");
                out.push_str(&format!("for {item} in {value_expr}.keys() {{ "));
                emit_bound_checks(out, &m.key, &item, depth + 1);
                out.push_str("} ");
            }
        }
        _ => {}
    }
}

/// Decode-side mirror of [`emit_bound_checks`] — regression #22 / XTypes 1.3
/// §7.4.3: the IDL bound of a `string<N>` / `wstring<N>` / `sequence<T,N>` /
/// `map<K,V,N>` must be enforced on BOTH encode and decode, not just encode.
/// Before this, a generated `decode` only rejected a value that overran the
/// remaining wire buffer (`zerodds_cdr` composite decoders) — a `string<8>`
/// field decoded a well-formed-but-oversized payload (e.g. 1 MB) without
/// complaint, an untrusted-input DoS vector. Emits the identical recursive
/// shape as the encode-side check (same nesting for `sequence<sequence<T,N>>`
/// / `map<K,V,N>`), but raises `DecodeError::BoundExceeded` instead of
/// `EncodeError::ValueOutOfRange`, since the value already exists in memory
/// by the time this runs (checked post-decode, not pre-encode).
///
/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn emit_decode_bound_checks(out: &mut String, spec: &TypeSpec, value_expr: &str, depth: usize) {
    match spec {
        TypeSpec::Sequence(seq) => {
            if let Some(n) = seq
                .bound
                .as_ref()
                .and_then(crate::type_map::const_expr_as_usize)
            {
                out.push_str(&format!(
                    "if {value_expr}.len() > {n} {{ return ::core::result::Result::Err(zerodds_cdr::DecodeError::BoundExceeded {{ actual: {value_expr}.len(), bound: {n}, message: \"decoded sequence length exceeds its IDL bound ({n})\" }}.into()); }} "
                ));
            }
            if type_has_bounds(&seq.elem) {
                let item = format!("__d{depth}");
                out.push_str(&format!("for {item} in {value_expr}.iter() {{ "));
                emit_decode_bound_checks(out, &seq.elem, &item, depth + 1);
                out.push_str("} ");
            }
        }
        TypeSpec::String(s) => {
            if let Some(n) = s
                .bound
                .as_ref()
                .and_then(crate::type_map::const_expr_as_usize)
            {
                if s.wide {
                    out.push_str(&format!(
                        "if {value_expr}.as_str().encode_utf16().count() > {n} {{ return ::core::result::Result::Err(zerodds_cdr::DecodeError::BoundExceeded {{ actual: {value_expr}.as_str().encode_utf16().count(), bound: {n}, message: \"decoded wstring length exceeds its IDL bound ({n})\" }}.into()); }} "
                    ));
                } else {
                    out.push_str(&format!(
                        "if {value_expr}.len() > {n} {{ return ::core::result::Result::Err(zerodds_cdr::DecodeError::BoundExceeded {{ actual: {value_expr}.len(), bound: {n}, message: \"decoded string length exceeds its IDL bound ({n})\" }}.into()); }} "
                    ));
                }
            }
        }
        TypeSpec::Map(m) => {
            if let Some(n) = m
                .bound
                .as_ref()
                .and_then(crate::type_map::const_expr_as_usize)
            {
                out.push_str(&format!(
                    "if {value_expr}.len() > {n} {{ return ::core::result::Result::Err(zerodds_cdr::DecodeError::BoundExceeded {{ actual: {value_expr}.len(), bound: {n}, message: \"decoded map length exceeds its IDL bound ({n})\" }}.into()); }} "
                ));
            }
            if type_has_bounds(&m.value) {
                let item = format!("__dv{depth}");
                out.push_str(&format!("for {item} in {value_expr}.values() {{ "));
                emit_decode_bound_checks(out, &m.value, &item, depth + 1);
                out.push_str("} ");
            }
            if type_has_bounds(&m.key) {
                let item = format!("__dk{depth}");
                out.push_str(&format!("for {item} in {value_expr}.keys() {{ "));
                emit_decode_bound_checks(out, &m.key, &item, depth + 1);
                out.push_str("} ");
            }
        }
        _ => {}
    }
}

/// Emits bound checks for a value `base_expr` of declared type
/// `type_spec` with `declarator`. For an array declarator
/// (`sequence<octet,4> arr[3]`) `base_expr` is a (possibly
/// multidimensional) array of the element type — it iterates over all
/// array elements so the array length is not wrongly checked against the
/// sequence bound. Shared by struct members (`base = self.{name}`) and
/// union arms (`base = __v`).
///
/// `optional` is `true` for an `@optional` struct member, whose field type
/// is `Option<T>` (or `Option<[T; N]>` for an array declarator) rather than
/// `T` directly — regression #22: `base_expr.len()` on an `Option<String>`
/// does not compile (`Option` has no `.len()`, only `Option::len` on the
/// wrapper itself, which is not what a bound check means here). When
/// `optional` is `true` the checks below run under an
/// `if let Some(ref ..) = base_expr` guard (mirroring idl-cpp's
/// `has_value()` guard, `emitter.rs` around the mutable/appendable bound
/// checks) — an absent optional trivially satisfies any bound. Union arms
/// are never `Option`-wrapped (the match binding is already the arm's raw
/// value), so that call site always passes `false`.
pub(crate) fn emit_bound_checks_decl(
    out: &mut String,
    type_spec: &TypeSpec,
    declarator: &Declarator,
    base_expr: &str,
    optional: bool,
) {
    if !type_has_bounds(type_spec) {
        return;
    }
    if optional {
        out.push_str(&format!(
            "if let ::core::option::Option::Some(ref __zd_opt_b) = {base_expr} {{ "
        ));
        emit_bound_checks_for_declarator(out, type_spec, declarator, "__zd_opt_b");
        out.push_str("} ");
        return;
    }
    emit_bound_checks_for_declarator(out, type_spec, declarator, base_expr);
}

fn emit_bound_checks_for_declarator(
    out: &mut String,
    type_spec: &TypeSpec,
    declarator: &Declarator,
    base_expr: &str,
) {
    match declarator {
        Declarator::Simple(_) => emit_bound_checks(out, type_spec, base_expr, 0),
        Declarator::Array(arr) => {
            let mut expr = base_expr.to_string();
            for (i, _) in arr.sizes.iter().enumerate() {
                let item = format!("__a{i}");
                out.push_str(&format!("for {item} in {expr}.iter() {{ "));
                expr = item;
            }
            emit_bound_checks(out, type_spec, &expr, 0);
            for _ in &arr.sizes {
                out.push_str("} ");
            }
        }
    }
}

fn emit_member_bound_checks(out: &mut String, member: &Member, declarator: &Declarator) {
    let base = format!("self.{}", declarator_ident(declarator));
    let optional = crate::annotations::member_is_optional(&member.annotations);
    emit_bound_checks_decl(out, &member.type_spec, declarator, &base, optional);
}

/// Picks the compact XTypes 1.3 §7.4.3.4.2 length code for a `@mutable`
/// member so the EMHEADER matches what CycloneDDS / RTI Connext / FastDDS
/// emit (2-vendor consensus, L4 oracle): primitive scalars use LC0–3 by
/// wire size, strings/wstrings reuse their `uint32` length prefix via LC5.
/// Returns `None` for members that must fall back to the universal LC4
/// (arrays, optionals, sequences, maps, nested/typedef/enum aggregates,
/// `long double`) — a compact code there is unsafe without resolving the
/// member's own framing, and LC4 is always valid (just less compact).
fn mutable_member_length_code(member: &Member, declarator: &Declarator) -> Option<&'static str> {
    mutable_length_code_for(
        &member.type_spec,
        declarator,
        crate::annotations::member_is_optional(&member.annotations),
    )
}

/// Length-code decision for an arbitrary `(type_spec, declarator, optional)` —
/// the `Member`-free core of [`mutable_member_length_code`], reused by the
/// `@mutable` union encoder so a union member frames with the SAME compact
/// EMHEADER length code as the identically-typed `@mutable` struct member.
pub(crate) fn mutable_length_code_for(
    type_spec: &TypeSpec,
    declarator: &Declarator,
    optional: bool,
) -> Option<&'static str> {
    use zerodds_idl::ast::types::TypeSpec;
    // Only scalar (non-array), non-optional members are eligible.
    if matches!(declarator, Declarator::Array(_)) || optional {
        return None;
    }
    match type_spec {
        TypeSpec::Primitive(p) => match crate::type_map::primitive_wire_size(*p) {
            1 => Some("Lc0"),
            2 => Some("Lc1"),
            4 => Some("Lc2"),
            8 => Some("Lc3"),
            _ => None, // long double (16) → LC4
        },
        // FINDING T1b: a member whose XCDR2 body begins with a 4-byte length
        // word — a string/wstring length prefix, a non-primitive sequence /
        // map DHEADER, or a nested @appendable/@mutable struct's DHEADER —
        // uses LC5 to REUSE that word as the NEXTINT (no redundant NEXTINT),
        // matching CycloneDDS / RTI / FastDDS. A @final nested struct (no
        // DHEADER) and a sequence<primitive> (bare element count, not a byte
        // length) fall through to the universal LC4.
        spec if crate::type_map::member_body_has_leading_dheader(spec) => Some("Lc5"),
        _ => None,
    }
}

fn emit_mutable_member_encode(
    out: &mut String,
    member: &Member,
    fallback_id: usize,
    autoid_hash: bool,
    indent: &str,
) -> Result<()> {
    let id = member_wire_id(autoid_hash, member, fallback_id);
    let must_understand = crate::annotations::member_must_understand(&member.annotations);
    let optional = crate::annotations::member_is_optional(&member.annotations);
    for declarator in &member.declarators {
        let name = declarator_ident(declarator);
        out.push_str(indent);
        match mutable_member_length_code(member, declarator) {
            Some(code) => out.push_str(&format!(
                "enc.encode_member_lc({id}, {must_understand}, zerodds_cdr::struct_enc::LengthCode::{code}, |w| {{ "
            )),
            None => out.push_str(&format!(
                "enc.encode_member({id}, {must_understand}, |w| {{ "
            )),
        }
        emit_member_bound_checks(out, member, declarator);
        emit_field_encode_with_optional(
            out,
            &member.type_spec,
            &format!("self.{name}"),
            "w",
            optional,
        )?;
        out.push_str(" Ok(()) })?;\n");
    }
    Ok(())
}

fn emit_decode_body(
    out: &mut String,
    s: &StructDef,
    extensibility: StructExtensibility,
) -> Result<()> {
    match extensibility {
        StructExtensibility::Final => {
            // Decode in declaration order.
            for (_, member) in serialized_members(s) {
                emit_member_decode_let(out, member, "        ")?;
            }
            out.push_str("        Ok(Self {\n");
            emit_plain_self_fields(out, s, "            ");
            out.push_str("        })\n");
        }
        StructExtensibility::Appendable => {
            out.push_str("        zerodds_cdr::struct_enc::decode_appendable(&mut reader, |r| {\n");
            for (_, member) in serialized_members(s) {
                emit_member_decode_let_with_reader(out, member, "            ", "r")?;
            }
            out.push_str("            Ok(Self {\n");
            emit_plain_self_fields(out, s, "                ");
            out.push_str("            })\n");
            out.push_str("        }).map_err(::core::convert::Into::into)\n");
        }
        StructExtensibility::Mutable => {
            // XTypes 1.3 §7.4.3.4.4: mutable decode with arbitrary member
            // order via the read_mutable_member loop + member-id match. The
            // `DdsType::decode` path owns a local `reader`, so the
            // decode_appendable receiver is `&mut reader`; the resulting
            // body returns `zerodds_cdr::DecodeError` which is mapped into
            // the `zerodds_dcps::DecodeError` this fn returns.
            emit_mutable_decode_body(out, s, "&mut reader")?;
            out.push_str(".map_err(::core::convert::Into::into)\n");
        }
    }
    Ok(())
}

/// Emits the EMHEADER-framed `@mutable` decode body — the
/// `read_mutable_member` loop with a per-member-id match — wrapped in a
/// `decode_appendable(<reader_expr>, |r| { … })` frame (XTypes 1.3
/// §7.4.3.4.4). This is the SINGLE source of truth for `@mutable` decode,
/// shared by:
///   * `DdsType::decode` (`emit_decode_body`, `reader_expr = "&mut reader"`)
///   * the writer-agnostic `CdrDecode::decode` (`emit_cdr_decode_impl`,
///     `reader_expr = "reader"`, since its param is already `&mut
///     BufferReader`).
///
/// Before this was shared, the `CdrDecode` impl decoded a `@mutable` struct
/// positionally (as if `@appendable`) — so a NESTED `@mutable` member, which
/// is decoded through the inner type's `CdrDecode`, read the inner struct's
/// first EMHEADER straight into its first field (FINDING T1a). Routing both
/// call sites here makes nested `@mutable` round-trip.
///
/// The caller appends the trailing result adapter (`.map_err(Into::into)`
/// for `DdsType`, nothing for `CdrDecode`) since the two paths return
/// different error types.
fn emit_mutable_decode_body(out: &mut String, s: &StructDef, reader_expr: &str) -> Result<()> {
    let autoid = crate::annotations::struct_autoid_hash(&s.annotations);
    out.push_str(&format!(
        "        zerodds_cdr::struct_enc::decode_appendable({reader_expr}, |r| {{\n"
    ));
    // Pre-init all serialized member slots as Option<T>::None. @non_serialized
    // members have no wire slot; they are defaulted directly in Self below.
    for (idx, member) in serialized_members(s) {
        let id = member_wire_id(autoid, member, idx);
        let optional = crate::annotations::member_is_optional(&member.annotations);
        for declarator in &member.declarators {
            let name = declarator_ident(declarator);
            // Slot type must honor array dimensions per declarator
            // (bug R2): an array member's slot is `Option<[T; N]>`.
            let base_type = declarator_rust_type(&member.type_spec, declarator)?;
            let target = if optional {
                format!("::core::option::Option<{base_type}>")
            } else {
                base_type
            };
            out.push_str(&format!(
                "            // member-id {id}\n            let mut {name}: ::core::option::Option<{target}> = ::core::option::Option::None;\n"
            ));
        }
    }
    // Loop over all mutable members on the wire.
    out.push_str("            loop {\n");
    out.push_str("                match zerodds_cdr::struct_enc::read_mutable_member(r)? {\n");
    out.push_str("                    ::core::option::Option::Some(member) => {\n");
    // The member body inherits the PARENT stream's byte order — a big-endian
    // @mutable payload carries big-endian member bodies (length prefixes, ints,
    // wstring units). Hardcoding LE here mis-reads every multi-byte field in a
    // BE stream (e.g. an LC5 string length read as 0x03000000 instead of 3).
    out.push_str("                        let mut body_reader = zerodds_cdr::BufferReader::new(member.body, zerodds_cdr::BufferReader::endianness(r)).xcdr2();\n");
    out.push_str("                        match member.member_id {\n");
    for (idx, member) in serialized_members(s) {
        let id = member_wire_id(autoid, member, idx);
        let optional = crate::annotations::member_is_optional(&member.annotations);
        for declarator in &member.declarators {
            let name = declarator_ident(declarator);
            out.push_str(&format!("                            {id} => {{\n"));
            out.push_str(&format!(
                "                                {name} = ::core::option::Option::Some("
            ));
            emit_field_decode_with_optional(
                out,
                &member.type_spec,
                declarator,
                "&mut body_reader",
                optional,
            )?;
            out.push_str(");\n");
            out.push_str("                            }\n");
        }
    }
    out.push_str("                            _ => {\n");
    out.push_str("                                if member.must_understand {\n");
    out.push_str("                                    return ::core::result::Result::Err(zerodds_cdr::DecodeError::UnknownMustUnderstandMember {\n");
    out.push_str("                                        member_id: member.member_id,\n");
    out.push_str("                                    });\n");
    out.push_str("                                }\n");
    out.push_str("                                // unknown optional member: skip body\n");
    out.push_str("                            }\n");
    out.push_str("                        }\n");
    out.push_str("                    }\n");
    out.push_str("                    ::core::option::Option::None => break,\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    // Self-init: an optional member absent on the wire is `None`; a mandatory
    // member absent on the wire takes its `@default(v)` value if it has one
    // (A33), else fails the decode with `MissingNonOptionalMember`.
    out.push_str("            ::core::result::Result::Ok(Self {\n");
    emit_slot_self_fields(out, s, autoid, "                ");
    out.push_str("            })\n");
    out.push_str("        })");
    Ok(())
}

/// Emits the `Ok(Self { .. })` field list for the slot-based decode paths
/// (`@mutable` EMHEADER + PL_CDR1): a serialized member finalizes its decoded
/// `Option<T>` slot via [`emit_member_slot_finalize`]; a `@non_serialized`
/// member (which has no slot) is set to `Default::default()` so its in-memory
/// field is present but wire-absent (broad-audit P0-5, #2). Serialized members
/// carry the SAME compacted positional index used by the slot/match loops, so
/// the member ids agree with the TypeObject.
fn emit_slot_self_fields(out: &mut String, s: &StructDef, autoid: bool, indent: &str) {
    // Compacted index over serialized members only (see `serialized_members`);
    // advanced manually here because this loop also visits the skipped members.
    let mut sidx = 0usize;
    for member in &s.members {
        if crate::annotations::member_is_non_serialized(&member.annotations) {
            for declarator in &member.declarators {
                let name = declarator_ident(declarator);
                out.push_str(&format!(
                    "{indent}{name}: ::core::default::Default::default(),\n"
                ));
            }
            continue;
        }
        let id = member_wire_id(autoid, member, sidx);
        sidx += 1;
        let optional = crate::annotations::member_is_optional(&member.annotations);
        for declarator in &member.declarators {
            let name = declarator_ident(declarator);
            emit_member_slot_finalize(out, member, declarator, &name, id, optional, indent);
        }
    }
}

/// Emits the `Ok(Self { .. })` field initializer for one decoded member slot
/// (`Option<T>`), shared by the `@mutable` (EMHEADER) and PL_CDR1 decode paths:
/// an optional member collapses an absent slot to `None`; a mandatory member
/// takes its `@default(v)` when absent (finding A33), else errors
/// `MissingNonOptionalMember`.
fn emit_member_slot_finalize(
    out: &mut String,
    member: &Member,
    declarator: &Declarator,
    name: &str,
    id: u32,
    optional: bool,
    indent: &str,
) {
    if optional {
        out.push_str(&format!(
            "{indent}{name}: {name}.unwrap_or(::core::option::Option::None),\n"
        ));
    } else if let Some(default) = member_default_expr(member, declarator) {
        out.push_str(&format!(
            "{indent}{name}: {name}.unwrap_or_else(|| {default}),\n"
        ));
    } else {
        out.push_str(&format!(
            "{indent}{name}: {name}.ok_or(zerodds_cdr::DecodeError::MissingNonOptionalMember {{ member_id: {id} }})?,\n"
        ));
    }
}

/// Renders a member's `@default(value)` as a Rust expression of the member's
/// type, for the decode-side absent-member fallback (finding A33). Returns
/// `None` when the member has no `@default`, the declarator is an array, or the
/// default cannot be rendered for the member type — the caller then keeps the
/// `MissingNonOptionalMember` error, so nothing ill-formed is emitted.
fn member_default_expr(member: &Member, declarator: &Declarator) -> Option<String> {
    use zerodds_idl::ast::types::PrimitiveType;
    if matches!(declarator, Declarator::Array(_)) {
        return None;
    }
    let expr = crate::annotations::member_default(&member.annotations)?;
    match &member.type_spec {
        TypeSpec::Primitive(PrimitiveType::Boolean) => {
            crate::type_map::const_expr_as_i128(&expr).map(|v| (v != 0).to_string())
        }
        // A float default may be an integer literal (`@default(5)`) or a real
        // float literal (`@default(3.14)`); render both as an `f64`-form literal
        // (`5.0`, `3.14`) that infers to the member's `f32`/`f64` slot.
        TypeSpec::Primitive(PrimitiveType::Floating(_)) => render_float_default(&expr),
        TypeSpec::Primitive(_) => crate::type_map::const_expr_as_i128(&expr).map(|v| v.to_string()),
        TypeSpec::String(st) => {
            let raw = string_literal_rust(&expr)?;
            if st.wide {
                Some(format!(
                    "zerodds_cdr::WString(::std::string::String::from({raw}))"
                ))
            } else {
                Some(format!("::std::string::String::from({raw})"))
            }
        }
        _ => None,
    }
}

/// Renders a floating `@default` (integer or float literal, or a folded integer
/// const) as an always-decimal-pointed Rust float literal.
fn render_float_default(expr: &ConstExpr) -> Option<String> {
    if let ConstExpr::Literal(lit) = expr {
        if matches!(lit.kind, LiteralKind::Floating | LiteralKind::Integer) {
            let trimmed = lit.raw.trim_end_matches(['f', 'F', 'd', 'D', 'l', 'L']);
            let f: f64 = trimmed.parse().ok()?;
            return f.is_finite().then(|| format!("{f:?}"));
        }
    }
    crate::type_map::const_expr_as_i128(expr).map(|v| format!("{:?}", v as f64))
}

/// Extracts the Rust `&str` literal (including quotes) from a `@default`
/// string-literal const expression, stripping any wide `L"…"` prefix.
fn string_literal_rust(expr: &ConstExpr) -> Option<String> {
    if let ConstExpr::Literal(lit) = expr {
        if matches!(lit.kind, LiteralKind::String | LiteralKind::WideString) {
            let s = lit.raw.trim();
            let s = s.strip_prefix('L').unwrap_or(s);
            if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Emits the PL_CDR1 (`@mutable` XCDR1 / classic CDR) decode body — a
/// `read_pl_cdr1_member` loop with a per-member-id match, symmetric to
/// [`emit_pl_cdr1_member_encode`] (FINDING E1). Each member body is plain
/// (no DHEADER), decoded with the reader's endianness at classic alignment
/// (max_align 8, i.e. NOT `.xcdr2()`). Unknown member IDs are skipped
/// (PL_CDR1 forward-compat). `reader` (the fn param) is the source.
fn emit_pl_cdr1_decode_body(out: &mut String, s: &StructDef, indent: &str) -> Result<()> {
    let autoid = crate::annotations::struct_autoid_hash(&s.annotations);
    // Pre-init serialized member slots (@non_serialized carries no wire slot).
    for (idx, member) in serialized_members(s) {
        let id = member_wire_id(autoid, member, idx);
        let optional = crate::annotations::member_is_optional(&member.annotations);
        for declarator in &member.declarators {
            let name = declarator_ident(declarator);
            let base_type = declarator_rust_type(&member.type_spec, declarator)?;
            let target = if optional {
                format!("::core::option::Option<{base_type}>")
            } else {
                base_type
            };
            out.push_str(&format!(
                "{indent}// member-id {id}\n{indent}let mut {name}: ::core::option::Option<{target}> = ::core::option::Option::None;\n"
            ));
        }
    }
    out.push_str(&format!(
        "{indent}let __endian = zerodds_cdr::BufferReader::endianness(reader);\n"
    ));
    out.push_str(&format!("{indent}loop {{\n"));
    out.push_str(&format!(
        "{indent}    match zerodds_cdr::xcdr1::read_pl_cdr1_member(reader)? {{\n"
    ));
    out.push_str(&format!(
        "{indent}        ::core::option::Option::Some(member) => {{\n"
    ));
    out.push_str(&format!(
        "{indent}            let mut body_reader = zerodds_cdr::BufferReader::new(&member.body, __endian);\n"
    ));
    out.push_str(&format!("{indent}            match member.member_id {{\n"));
    for (idx, member) in serialized_members(s) {
        let id = member_wire_id(autoid, member, idx);
        let optional = crate::annotations::member_is_optional(&member.annotations);
        for declarator in &member.declarators {
            let name = declarator_ident(declarator);
            out.push_str(&format!("{indent}                {id} => {{\n"));
            out.push_str(&format!(
                "{indent}                    {name} = ::core::option::Option::Some("
            ));
            emit_field_decode_with_optional(
                out,
                &member.type_spec,
                declarator,
                "&mut body_reader",
                optional,
            )?;
            out.push_str(");\n");
            out.push_str(&format!("{indent}                }}\n"));
        }
    }
    out.push_str(&format!(
        "{indent}                _ => {{ /* unknown PL_CDR1 member: skip */ }}\n"
    ));
    out.push_str(&format!("{indent}            }}\n"));
    out.push_str(&format!("{indent}        }}\n"));
    out.push_str(&format!(
        "{indent}        ::core::option::Option::None => break,\n"
    ));
    out.push_str(&format!("{indent}    }}\n"));
    out.push_str(&format!("{indent}}}\n"));
    out.push_str(&format!("{indent}::core::result::Result::Ok(Self {{\n"));
    let field_indent = format!("{indent}    ");
    emit_slot_self_fields(out, s, autoid, &field_indent);
    out.push_str(&format!("{indent}}})\n"));
    Ok(())
}

fn emit_member_decode_let(out: &mut String, member: &Member, indent: &str) -> Result<()> {
    emit_member_decode_let_with_reader(out, member, indent, "&mut reader")
}

pub(crate) fn emit_member_decode_let_with_reader(
    out: &mut String,
    member: &Member,
    indent: &str,
    reader_expr: &str,
) -> Result<()> {
    let optional = crate::annotations::member_is_optional(&member.annotations);
    for declarator in &member.declarators {
        let name = declarator_ident(declarator);
        out.push_str(indent);
        out.push_str(&format!("let {name} = "));
        emit_field_decode_with_optional(out, &member.type_spec, declarator, reader_expr, optional)?;
        out.push_str(";\n");
    }
    Ok(())
}

/// Emits a `zerodds_cdr::CdrDecode::decode` call that reads a value of
/// type `target_type` from the reader.
///
/// We use the **fully-qualified trait form** with an explicit target
/// type so that type inference works even with `let x = ...;` without a
/// right-hand side.
fn emit_field_decode_with_optional(
    out: &mut String,
    spec: &TypeSpec,
    decl: &Declarator,
    reader_expr: &str,
    optional: bool,
) -> Result<()> {
    // Bug R2b: an `array-of-struct` / `array-of-union` member (e.g.
    // `Point shape[2]`, fixture 08_arrays.idl) cannot decode via the
    // `impl CdrDecode for [T; N]` blanket impl in zerodds-cdr — that impl
    // requires `T: Default + Copy`, and generated structs/unions are
    // `Default` but NOT `Copy`. Emit a manual element-wise decoder that
    // mirrors the blanket impl byte-for-byte (same DHEADER decision via
    // `needs_collection_dheader`) and builds each fixed-size array from a
    // `Vec` (no `Copy` bound). Primitive/enum arrays keep the proven
    // blanket impl. `@optional` never co-occurs with array declarators in
    // legal IDL (optionality is a member-level annotation; the array still
    // decodes the same way), but we still honor the `Option` wrap below.
    if let Declarator::Array(arr) = decl {
        if crate::type_map::array_elem_needs_manual_decode(spec) {
            let sizes: Vec<usize> = arr
                .sizes
                .iter()
                .map(|e| {
                    crate::type_map::const_expr_as_usize(e).ok_or(RustGenError::InvalidAnnotation {
                        name: "array-size".to_string(),
                        reason: "non-integer array dimension",
                    })
                })
                .collect::<Result<_>>()?;
            let elem_ty = rust_type_for(spec)?;
            // `is_primitive` per array level: the innermost dimension's
            // element is `spec` (a struct/union → non-primitive); every
            // outer dimension's element is itself an array (non-primitive).
            // So with array-of-struct ALL levels carry a DHEADER under
            // XCDR2 — exactly what `[T; N]: CdrEncode` writes.
            let mut expr = String::new();
            emit_manual_array_decode(&mut expr, &elem_ty, spec, &sizes, 0, reader_expr);
            if optional {
                out.push_str(&format!("::core::option::Option::Some({expr})"));
            } else {
                out.push_str(&expr);
            }
            return Ok(());
        }
    }

    // `wstring` member (simple declarator): decode the XCDR2 wide-string
    // form INLINE — uint32 octet-length + UTF-16LE units, no BOM (OMG
    // XTypes 1.3 §7.4.1.2.4; see `emit_xcdr2_wstring_encode`). The GIOP
    // `WString::decode` would (correctly for CORBA) detect/strip a BOM that
    // the XCDR2 PSM never writes; decoding inline keeps the round-trip
    // byte-exact with every other PSM. Array-of-wstring still falls through
    // to the array path above (which composes element decoders); here we
    // only special-case the simple declarator.
    if let (TypeSpec::String(s), Declarator::Simple(_)) = (spec, decl) {
        if s.wide {
            // Regression #22: a bounded `wstring<N>` must reject an
            // over-bound decoded value (XTypes 1.3 §7.4.3), not just an
            // over-bound encode. `has_bound` gates the extra check so an
            // unbounded `wstring` keeps the original single-expression form.
            let has_bound = type_has_bounds(spec);
            if optional {
                // `@optional wstring`: uint8 present flag + value if present.
                out.push_str(&format!(
                    "if zerodds_cdr::BufferReader::read_u8({reader_expr})? != 0 {{ ::core::option::Option::Some("
                ));
                if has_bound {
                    out.push_str("{ let __zd_dv = ");
                    emit_xcdr2_wstring_decode(out, reader_expr);
                    out.push_str("; ");
                    emit_decode_bound_checks(out, spec, "__zd_dv", 0);
                    out.push_str("__zd_dv }");
                } else {
                    emit_xcdr2_wstring_decode(out, reader_expr);
                }
                out.push_str(") } else { ::core::option::Option::None }");
            } else if has_bound {
                out.push_str("{ let __zd_dv = ");
                emit_xcdr2_wstring_decode(out, reader_expr);
                out.push_str("; ");
                emit_decode_bound_checks(out, spec, "__zd_dv", 0);
                out.push_str("__zd_dv }");
            } else {
                emit_xcdr2_wstring_decode(out, reader_expr);
            }
            return Ok(());
        }
    }

    // The decode target MUST be the declarator-wrapped type so a
    // fixed-size array member (`long vec[3]` → `[i32; 3]`) decodes all
    // dimensions, not just one scalar element (bug R2). For a simple
    // declarator this collapses to the element type.
    let target = declarator_rust_type(spec, decl)?;
    let final_target = if optional {
        format!("::core::option::Option<{target}>")
    } else {
        target
    };
    let raw_decode = format!("<{final_target} as zerodds_cdr::CdrDecode>::decode({reader_expr})?");
    // Regression #22 / XTypes 1.3 §7.4.3: enforce the IDL bound on decode
    // too. By this point `decl` is always `Declarator::Simple` when
    // `type_has_bounds(spec)` holds — an array declarator whose element is
    // a bounded String/Sequence/Map returned earlier via the manual
    // array-decode path above (`array_elem_needs_manual_decode`), so no
    // separate array-of-bounded-element case is possible here.
    if type_has_bounds(spec) {
        if optional {
            out.push_str("{ let __zd_dv = ");
            out.push_str(&raw_decode);
            out.push_str("; if let ::core::option::Option::Some(ref __zd_dvi) = __zd_dv { ");
            emit_decode_bound_checks(out, spec, "__zd_dvi", 0);
            out.push_str("} __zd_dv }");
        } else {
            out.push_str("{ let __zd_dv = ");
            out.push_str(&raw_decode);
            out.push_str("; ");
            emit_decode_bound_checks(out, spec, "__zd_dv", 0);
            out.push_str("__zd_dv }");
        }
    } else {
        out.push_str(&raw_decode);
    }
    Ok(())
}

/// Emits an inline block expression decoding a fixed-size (possibly
/// multidimensional) array of a non-`Copy` element type (`elem_ty`,
/// e.g. `Point`) for the dimensions `sizes[dim..]`. Mirrors
/// `impl CdrDecode for [T; N]` in zerodds-cdr exactly: under XCDR2
/// (`reader.max_alignment() == 4`) the array of a non-primitive element
/// carries a leading uint32 DHEADER, which we skip. Each level decodes
/// `sizes[dim]` items into a `Vec` and converts it to `[_; N]` via
/// `try_into` (no `Copy` bound). `reader_expr` is the reader receiver
/// (`&mut reader` / `&mut body_reader`).
///
/// `elem_spec` is the IDL type spec of the array's element (as opposed to
/// `elem_ty`, its Rust type rendering) — B1 follow-up array-of-bounded-
/// element gap (regression #22 remaining disclosed gap, e.g.
/// `sequence<octet,4> arr[3]`): a bounded element type reaches this manual
/// per-element decode path (its element is a `Vec`/`String`/`BTreeMap`, not
/// `Copy`), which previously decoded each element with no IDL-bound check
/// at all — the recursive [`emit_decode_bound_checks`] only ran on the
/// simple-declarator path in [`emit_field_decode_with_optional`]. When
/// `type_has_bounds(elem_spec)` holds, the innermost per-element decode
/// wraps the freshly decoded element in [`emit_decode_bound_checks`] before
/// it is pushed, mirroring the simple-declarator behavior.
///
/// zerodds-lint: recursion-depth 8
fn emit_manual_array_decode(
    out: &mut String,
    elem_ty: &str,
    elem_spec: &TypeSpec,
    sizes: &[usize],
    dim: usize,
    reader_expr: &str,
) {
    let n = sizes[dim];
    let inner_array_ty = build_array_type(elem_ty, &sizes[dim + 1..]);
    // The element decoded at THIS level.
    out.push_str("{ ");
    // XCDR2 collection DHEADER: the element at this level is non-primitive
    // (either a deeper array or the struct/union itself), so under the
    // 4-byte-cap XCDR2 stream `[T; N]::encode` prepended a uint32 DHEADER.
    out.push_str(&format!(
        "if zerodds_cdr::BufferReader::max_alignment({reader_expr}) == 4 {{ let _ = zerodds_cdr::BufferReader::read_u32({reader_expr})?; }} "
    ));
    out.push_str(&format!(
        "let mut __arr: ::std::vec::Vec<{inner_array_ty}> = ::std::vec::Vec::with_capacity({n}); "
    ));
    out.push_str(&format!("for _ in 0..{n} {{ __arr.push("));
    if dim + 1 < sizes.len() {
        emit_manual_array_decode(out, elem_ty, elem_spec, sizes, dim + 1, reader_expr);
    } else if type_has_bounds(elem_spec) {
        out.push_str(&format!(
            "{{ let __zd_dv = <{elem_ty} as zerodds_cdr::CdrDecode>::decode({reader_expr})?; "
        ));
        emit_decode_bound_checks(out, elem_spec, "__zd_dv", dim + 1);
        out.push_str("__zd_dv }");
    } else {
        out.push_str(&format!(
            "<{elem_ty} as zerodds_cdr::CdrDecode>::decode({reader_expr})?"
        ));
    }
    out.push_str("); } ");
    // We push exactly `n` elements above, so this `try_into` never fails;
    // the error arm names a real variant only to satisfy the type checker.
    out.push_str(&format!(
        "let __arr: [{inner_array_ty}; {n}] = __arr.try_into().map_err(|v: ::std::vec::Vec<{inner_array_ty}>| zerodds_cdr::DecodeError::LengthExceeded {{ announced: {n}, remaining: v.len(), offset: 0 }})?; __arr }}"
    ));
}

/// Builds the Rust type `[[..elem_ty..; N]; M]` for the dimensions in
/// `sizes` (innermost last), matching [`declarator_rust_type`].
fn build_array_type(elem_ty: &str, sizes: &[usize]) -> String {
    let mut wrapped = elem_ty.to_string();
    for size in sizes.iter().rev() {
        wrapped = format!("[{wrapped}; {size}]");
    }
    wrapped
}
