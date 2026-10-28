// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OCaml that turns one decoded sample back into JSON.
//!
//! The counterpart of [`crate::fill`], which puts a JSON sample into
//! the generated OCaml. This takes one back out, after `runtime/ocaml`
//! and the generated decoder have filled it from the bytes.
//!
//! The dialect is the shared one `crates/wire-probe/src/lib.rs`
//! documents, and two of its rules cannot be recovered by any reader: a
//! 64-bit integer is a **string**, and an absent `@optional` member is
//! a key that is **left out**, never a `null`. It is not canonical form
//! — the caller canonicalises in Rust, so a second canonicaliser cannot
//! part from the first over a spelling.
//!
//! One function per type, keyed the way the decoder is keyed, in one
//! `let rec … and …` chain so a recursive type resolves; the JSON text
//! is built with string concatenation, because the point of this column
//! is the octets and a JSON library would be the first dependency
//! outside the standard library.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_cst::ocaml::ValueKind;
use zerodds_lang_cst::ocaml::lower_ident;
use zerodds_lang_rules::adapter::ocaml::{
    ocaml_enumerator_name, ocaml_map_entry_name, ocaml_value_type,
};
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::ocaml::{OCaml, kind_of, ocaml_type_text};

use crate::fill::ocaml_string_literal;

/// The name of the function that prints a value of `id`: the decoder's
/// name with the verb changed, so the two are keyed identically.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<OCaml>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The OCaml source of one dumper per type reachable from `root`, as a
/// `let rec` chain.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n(* One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         \x20  documents. Not canonical form: the caller canonicalises in Rust. *)\n",
    );
    let mut emitted: Vec<String> = Vec::new();
    let mut first = true;
    for id in reachable {
        // Two structurally equal collections share one tag: the first
        // body emitted under a name is the body.
        let name = dumper_name(ast, id);
        if emitted.contains(&name) {
            continue;
        }
        let keyword = if first { "let rec" } else { "and" };
        // A typedef's own dumper name follows the alias chain, so for a
        // typedef of an aggregate or a collection the target's body
        // already answers to it. A typedef of a *primitive* is the one
        // case the two names differ — the primitive's own tag carries
        // `p_` — so a forwarding body carries the alias's name to the
        // target's.
        if let Ok(TypeKind::Alias { aliased }) = ast.type_(id).map(|node| node.kind.clone()) {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{name}: the typedef target is unresolved"))?;
            let target_name = dumper_name(ast, target);
            if target_name == name {
                continue;
            }
            let parameter = parameter_type(ast, id)?;
            emitted.push(name.clone());
            let _ = write!(
                out,
                "\n{keyword} {name} (v : {parameter}) (path : string) : string =\n\
                 \x20 {target_name} v path\n"
            );
            first = false;
            continue;
        }
        emitted.push(name);
        out.push_str(&function(ast, id, union_label, keyword)?);
        first = false;
    }
    Ok(out)
}

/// Every type a value of `id` can hold, `id` included.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_DEPTH`)
fn collect(ast: &Ast, id: Id<TypeNode>, out: &mut Vec<Id<TypeNode>>, depth: usize) {
    if depth > MAX_DEPTH || out.contains(&id) {
        return;
    }
    let Ok(node) = ast.type_(id) else {
        return;
    };
    out.push(id);
    let inner = |half: Option<Id<TypeNode>>, out: &mut Vec<Id<TypeNode>>| {
        if let Some(target) = half {
            collect(ast, target, out, depth + 1);
        }
    };
    match &node.kind {
        TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => {
            inner(element.resolved(), out);
        }
        TypeKind::Map { key, value, .. } => {
            inner(key.resolved(), out);
            inner(value.resolved(), out);
        }
        TypeKind::Alias { aliased } => inner(aliased.resolved(), out),
        TypeKind::ValueBox { boxed } => inner(boxed.resolved(), out),
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            for member in ast.all_members_of(id) {
                if let Ok(member) = ast.member(member) {
                    inner(member.type_.resolved(), out);
                }
            }
        }
        TypeKind::Union { discriminator, .. } => {
            inner(discriminator.resolved(), out);
            for case in ast.cases_of(id) {
                let Ok(case) = ast.case(*case) else {
                    continue;
                };
                if let Ok(member) = ast.member(case.member) {
                    inner(member.type_.resolved(), out);
                }
            }
        }
        _ => {}
    }
}

fn parameter_type(ast: &Ast, id: Id<TypeNode>) -> Result<String, String> {
    match ocaml_value_type(ast, id) {
        Ok(type_) => Ok(ocaml_type_text(&type_)),
        Err(error) => Err(format!("the type has no OCaml form: {error:?}")),
    }
}

/// One dumper, as a whole function of the chain. The parameter type is
/// the materialiser's, so the signature and the decoder agree.
#[allow(clippy::too_many_lines)]
fn function(
    ast: &Ast,
    id: Id<TypeNode>,
    union_label: UnionLabelMatch,
    keyword: &str,
) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let parameter = parameter_type(ast, id).map_err(|error| format!("{name}: {error}"))?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        // A JSON number JSON can write, or a refusal naming the path.
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("Zdwire.json_float v path".to_string());
        }
        // The exact decimal, as the string the dialect writes.
        TypeKind::Primitive(PrimitiveKind::Float128 | PrimitiveKind::UInt64) => {
            body.push("ignore path; Zdwire.json_string v".to_string());
        }
        // Both 64-bit signed widths go out as decimal in a string.
        TypeKind::Primitive(PrimitiveKind::Int64) => {
            body.push("ignore path; \"\\\"\" ^ Int64.to_string v ^ \"\\\"\"".to_string());
        }
        // A 32-bit unsigned member is held widened; its value fits a
        // JSON number.
        TypeKind::Primitive(PrimitiveKind::UInt32) => {
            body.push("ignore path; Int64.to_string v".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Int32) => {
            body.push("ignore path; Int32.to_string v".to_string());
        }
        // The one-character string the dialect writes for a character.
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("ignore path; Zdwire.json_char v".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("ignore path; if v then \"true\" else \"false\"".to_string());
        }
        TypeKind::Primitive(_) => {
            body.push("ignore path; string_of_int v".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("ignore path; Zdwire.json_string v".to_string());
        }
        // The enumerator's name, chosen by the constructor: a variant
        // has no value outside its constructors, so the match is
        // exhaustive by construction.
        TypeKind::Enumeration { .. } => {
            body.push("ignore path;".to_string());
            body.push("(match v with".to_string());
            for enumerator in ast.enumerators_of(id) {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                body.push(format!(
                    " | {} -> {}",
                    ocaml_enumerator_name(ast, id, &enum_node.name),
                    ocaml_string_literal(&format!("\"{}\"", enum_node.name)),
                ));
            }
            body.push(")".to_string());
        }
        // The set bits' names in declaration order; a bit set inside
        // the bound that the type does not declare is refused rather
        // than dropped.
        TypeKind::Bitmask { .. } => {
            let bound = ast.effective_bit_bound(id);
            let mut accounted: u64 = 0;
            let mut declared: Vec<(String, u16)> = Vec::new();
            for bit in ast.bits_of(id) {
                let Ok(value) = ast.bit_value(*bit) else {
                    continue;
                };
                let Some(position) = ast.effective_bit_position(*bit) else {
                    continue;
                };
                if position >= bound {
                    continue;
                }
                let Some(mask) = 1u64.checked_shl(u32::from(position)) else {
                    continue;
                };
                accounted |= mask;
                declared.push((value.name.clone(), position));
            }
            let widen = if bound <= 32 {
                "Int64.logand (Int64.of_int32 v) 0xFFFFFFFFL"
            } else {
                "v"
            };
            #[allow(clippy::cast_possible_wrap)]
            let unaccounted = !accounted as i64;
            body.push(format!("let zd_bits = {widen} in"));
            body.push(format!(
                "if Int64.logand zd_bits ({unaccounted}L) <> 0L then \
                 raise (Zdwire.Wire_error (path ^ \": a bit this bitmask does not declare is \
                 set\"));"
            ));
            let mut parts = Vec::new();
            for (bit, position) in &declared {
                parts.push(format!(
                    "(if Int64.logand (Int64.shift_right_logical zd_bits {position}) 1L = 1L \
                     then [{}] else [])",
                    ocaml_string_literal(&format!("\"{bit}\""))
                ));
            }
            if parts.is_empty() {
                body.push("\"[]\"".to_string());
            } else {
                body.push(format!(
                    "\"[\" ^ String.concat \",\" (List.concat [{}]) ^ \"]\"",
                    parts.join("; ")
                ));
            }
        }
        // The whole list, base first: a derived bitset carries the
        // base's bitfields and the decoder fills them.
        TypeKind::Bitset { .. } => {
            let mut entries = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                let spelled = lower_ident(field_name);
                let shown = match zerodds_lang_rules::adapter::ocaml::ocaml_holder(field_node.bits)
                {
                    zerodds_lang_cst::ocaml::Primitive::Int32 => {
                        format!("Int32.to_string v.{spelled}")
                    }
                    zerodds_lang_cst::ocaml::Primitive::Int64 => {
                        format!("Int64.to_string v.{spelled}")
                    }
                    _ => format!("string_of_int v.{spelled}"),
                };
                entries.push(format!(
                    "{} ^ \":\" ^ {shown}",
                    ocaml_string_literal(&format!("\"{field_name}\""))
                ));
            }
            body.push("ignore path;".to_string());
            if entries.is_empty() {
                body.push("ignore v; \"{}\"".to_string());
            } else {
                body.push(format!(
                    "\"{{\" ^ String.concat \",\" [{}] ^ \"}}\"",
                    entries.join("; ")
                ));
            }
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push(format!(
                "\"[\" ^ String.concat \",\" (Array.to_list (Array.map (fun zd_x -> {} zd_x \
                 path) v)) ^ \"]\"",
                dumper_name(ast, element)
            ));
        }
        // One `Array.map` per dimension: nested JSON arrays in written
        // order. The one bound name `zd_e` is rebound per level, which
        // OCaml's shadowing makes exactly the nesting §7.4.3.5 asks
        // for.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let mut out_expr = format!("{} zd_e path", dumper_name(ast, element));
            for _ in 0..dims.len() {
                out_expr = format!(
                    "\"[\" ^ String.concat \",\" (Array.to_list (Array.map (fun zd_e -> \
                     {out_expr}) zd_e)) ^ \"]\""
                );
            }
            body.push(format!("(fun zd_e -> {out_expr}) v"));
        }
        // An array of two-element arrays, because a JSON object keys on
        // a string and a map key need not be one.
        //
        // The binder carries its type. Every map entry record declares
        // the same two labels, `key` and `value`, so an unannotated
        // `zd_e.value` resolves to whichever entry type was declared
        // last — for `map<long, map<long, long> >` the outer one, whose
        // `value` is an array, and the inner dumper stops compiling.
        // The annotation is OCaml's type-directed disambiguation, and
        // it is what the enclosing `v` already carries.
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            body.push(format!(
                "\"[\" ^ String.concat \",\" (Array.to_list (Array.map (fun (zd_e : {}) -> \"[\" ^ \
                 {} zd_e.key path ^ \",\" ^ {} zd_e.value path ^ \"]\") v)) ^ \"]\"",
                ocaml_map_entry_name(ast, id),
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let members = ast.all_members_of(id);
            if members.is_empty() {
                body.push("ignore path; ignore v; \"{}\"".to_string());
            } else {
                let mut parts = Vec::new();
                for member in &members {
                    let Ok(member_node) = ast.member(*member) else {
                        continue;
                    };
                    let member_type = member_node.type_.resolved().ok_or_else(|| {
                        format!("{name}.{}: the member type is unresolved", member_node.name)
                    })?;
                    // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                    let spelled =
                        zerodds_lang_rules::emit::member_field::<
                            zerodds_lang_rules::emit::ocaml::OCaml,
                        >(ast, id, member_node.node_id, &member_node.name);
                    let key = ocaml_string_literal(&format!("\"{}\":", member_node.name));
                    let where_ = format!(
                        "(path ^ {})",
                        ocaml_string_literal(&format!(".{}", member_node.name))
                    );
                    if member_node.facets.is_optional() {
                        // An absent member is a key that is not printed.
                        parts.push(format!(
                            "(match v.{spelled} with Some zd_x -> [{key} ^ {} zd_x {where_}] | \
                             None -> [])",
                            dumper_name(ast, member_type)
                        ));
                    } else {
                        parts.push(format!(
                            "[{key} ^ {} v.{spelled} {where_}]",
                            dumper_name(ast, member_type)
                        ));
                    }
                }
                body.push(format!(
                    "\"{{\" ^ String.concat \",\" (List.concat [{}]) ^ \"}}\"",
                    parts.join("; ")
                ));
            }
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push(format!(
                "let zd_parts = ref [\"\\\"_d\\\":\" ^ {} v._d (path ^ \"._d\")] in",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every
            // branch the value holds: the defaulter fills the default
            // branch before the decoder runs, so printing every `Some`
            // field would return the selected branch and the defaulted
            // one.
            let suppress_negative = union_negative_matches_nothing(ast, id, union_label);
            let mut arms: Vec<(String, String, Id<TypeNode>)> = Vec::new();
            let mut fallback: Option<(String, Id<TypeNode>)> = None;
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the case type is unresolved", member.name)
                })?;
                let mut tests: Vec<String> = Vec::new();
                let mut is_default = false;
                for label in &case_node.labels {
                    match label.expr() {
                        None => is_default = true,
                        Some(value_expr) => {
                            let value = ast.eval_integral(value_expr).map_err(|error| {
                                format!("{name}: a case label does not evaluate: {error}")
                            })?;
                            if suppress_negative && value < 0 {
                                continue;
                            }
                            tests
                                .push(format!("v._d = {}", ocaml_label(ast, discriminator, value)));
                        }
                    }
                }
                if is_default {
                    fallback = Some((member.name.clone(), member_type));
                }
                if !tests.is_empty() {
                    arms.push((tests.join(" || "), member.name.clone(), member_type));
                }
            }
            let branch = |member_name: &str, member_type: Id<TypeNode>| {
                let spelled = lower_ident(member_name);
                let key = ocaml_string_literal(&format!("\"{member_name}\":"));
                let where_ = format!(
                    "(path ^ {})",
                    ocaml_string_literal(&format!(".{member_name}"))
                );
                format!(
                    "zd_parts := !zd_parts @ [{key} ^ {} (Option.get v.{spelled}) {where_}]",
                    dumper_name(ast, member_type)
                )
            };
            let mut chain = String::new();
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 { "if" } else { "else if" };
                chain.push_str(&format!(
                    "{keyword} {condition} then begin {} end\n",
                    branch(member_name, *member_type)
                ));
            }
            match (&fallback, arms.is_empty()) {
                (Some((member_name, member_type)), true) => {
                    chain = format!("begin {} end", branch(member_name, *member_type));
                }
                (Some((member_name, member_type)), false) => {
                    chain.push_str(&format!(
                        "else begin {} end",
                        branch(member_name, *member_type)
                    ));
                }
                (None, false) => chain.push_str("else ()"),
                (None, true) => chain = "()".to_string(),
            }
            body.push(format!("{chain};"));
            body.push("\"{\" ^ String.concat \",\" !zd_parts ^ \"}\"".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no OCaml shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\n{keyword} {name} (v : {parameter}) (path : string) : string =\n");
    for line in body {
        let _ = writeln!(out, "  {line}");
    }
    Ok(out)
}

/// A union label in the discriminator's OCaml form: the constructor
/// for an enumeration, `true`/`false` for a `boolean`, a suffixed
/// literal for the integer holders, a quoted decimal for the text
/// holder, and the bare number for the `int`-held kinds.
fn ocaml_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    if let Some(enumeration) = resolve_enum(ast, discriminator, 0) {
        for enumerator in ast.enumerators_of(enumeration) {
            if ast.effective_enumerator_value(*enumerator) == Some(value) {
                if let Ok(node) = ast.enumerator(*enumerator) {
                    return ocaml_enumerator_name(ast, enumeration, &node.name);
                }
            }
        }
    }
    match kind_of(ast, discriminator, 0) {
        ValueKind::Bool => (if value == 0 { "false" } else { "true" }).to_string(),
        ValueKind::Int32 => format!("({value}l)"),
        ValueKind::Int64 => format!("({value}L)"),
        ValueKind::Text => format!("\"{value}\""),
        _ => {
            if value < 0 {
                format!("({value})")
            } else {
                format!("{value}")
            }
        }
    }
}

/// The enumeration behind a discriminator, if any.
///
/// zerodds-lint: recursion-depth 64 (typedef chains only —
/// every other kind answers without a descent, so the walk never
/// reaches an aggregate and cannot cycle; bounded by [`MAX_DEPTH`])
fn resolve_enum(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<Id<TypeNode>> {
    if depth > MAX_DEPTH {
        return None;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Enumeration { .. }) => Some(id),
        Ok(TypeKind::Alias { aliased }) => resolve_enum(ast, aliased.resolved()?, depth + 1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::dump;
    use zerodds_ast::Ast;
    use zerodds_ast_layout::UnionLabelMatch;

    fn dumpers(idl: &str, root_name: &str) -> String {
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
        let mut ast: Ast = lowered.ast;
        ast.resolve_all_by_name().expect("the names resolve");
        let root = ast
            .types()
            .find(|(_, node)| node.name.as_str() == root_name)
            .map(|(id, _)| id)
            .expect("the root type");
        dump(&ast, root, UnionLabelMatch::default()).expect("the dumpers")
    }

    /// Every map entry record declares `key` and `value`, so an
    /// unannotated binder resolves to whichever was declared last. For
    /// `map<long, map<long, long> >` that is the outer entry, whose
    /// `value` is an array, and the inner dumper then does not compile.
    /// The binder carries its type.
    #[test]
    fn a_nested_map_binds_each_entry_under_its_own_type() {
        let text = dumpers(
            "@final struct Sample { map<long, map<long, long> > entries; long tail; };",
            "Sample",
        );
        assert!(
            !text.contains("(fun zd_e -> \"[\""),
            "a map entry binder is unannotated:\n{text}"
        );
        assert!(
            text.contains("(fun (zd_e : zd_map_entry_long_long) ->"),
            "the inner entry binder does not name its type:\n{text}"
        );
        assert!(
            text.contains("(fun (zd_e : zd_map_entry_long_zd_map_entry_long_long) ->"),
            "the outer entry binder does not name its type:\n{text}"
        );
    }
}
