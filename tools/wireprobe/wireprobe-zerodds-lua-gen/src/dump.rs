// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Lua that turns one decoded sample back into JSON text.
//!
//! The counterpart of `wireprobe-zerodds-python-gen/src/dump.rs`, with
//! one difference the platform forces: Lua ships no JSON library, so
//! the dumpers build the JSON **text** directly — a handful of quoting
//! helpers emitted once, then one function per type. The dialect is
//! the shared one (`crates/wire-probe/src/lib.rs`): a 64-bit integer
//! is a string, an absent `@optional` member is a key left out, a map
//! is an array of two-element arrays, an enumerator is its name. Not
//! canonical form — the caller canonicalises in Rust, which is what
//! every sibling column does.
//!
//! One function per type, keyed the way the decoder is keyed
//! (`zd_json_` for `zd_dec_`), living in the same `ZD` table: a name
//! that does not resolve is the keying's fault and not a spelling's.

use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_ast_layout::{UnionLabelMatch, union_negative_matches_nothing};
use zerodds_lang_cst::lua::is_identifier;
use zerodds_lang_render::lua::escape_string;
use zerodds_lang_rules::emit::decode::decoder_name;
use zerodds_lang_rules::emit::lua::Lua;

/// The name of the function that prints a value of `id`: the decoder's
/// with the verb changed, so `octet` and `unsigned long long` — one
/// Lua integer between them — stay apart.
#[must_use]
pub fn dumper_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let decoder = decoder_name::<Lua>(ast, id);
    match decoder.strip_prefix("zd_dec_") {
        Some(rest) => format!("zd_json_{rest}"),
        None => format!("zd_json_{decoder}"),
    }
}

/// How deep the reachability walk follows a chain of collections and
/// typedefs.
const MAX_DEPTH: usize = 64;

/// The helpers every dumper leans on, emitted once ahead of them.
///
/// `zd_q` writes a JSON string from a UTF-8 Lua string, escaping the
/// two characters JSON requires and the control range; everything else
/// passes through as its own octets, which is JSON. `zd_c` is a code
/// point as a one-character JSON string, with a surrogate written as
/// its `\u` escape because UTF-8 has no encoding for one. `zd_u64` is
/// the unsigned decimal of a 64-bit bit pattern — Lua's own `tostring`
/// reads the pattern signed. `zd_num` refuses the three values JSON
/// cannot write and prints the rest with enough digits to read back as
/// the same binary64.
const HELPERS: &str = r#"
function ZD.zd_q(text)
  local parts = { "\"" }
  for index = 1, #text do
    local byte = string.byte(text, index)
    if byte == 0x22 then
      parts[#parts + 1] = "\\\""
    elseif byte == 0x5C then
      parts[#parts + 1] = "\\\\"
    elseif byte == 0x0A then
      parts[#parts + 1] = "\\n"
    elseif byte == 0x0D then
      parts[#parts + 1] = "\\r"
    elseif byte == 0x09 then
      parts[#parts + 1] = "\\t"
    elseif byte < 0x20 then
      parts[#parts + 1] = string.format("\\u%04X", byte)
    else
      parts[#parts + 1] = string.char(byte)
    end
  end
  parts[#parts + 1] = "\""
  return table.concat(parts)
end

function ZD.zd_c(point, path)
  if point >= 0xD800 and point <= 0xDFFF then
    return string.format("\"\\u%04X\"", point)
  end
  local ok, text = pcall(utf8.char, point)
  if not ok then
    error(path .. ": " .. tostring(point) .. " is not a code point", 0)
  end
  return ZD.zd_q(text)
end

function ZD.zd_u64(value)
  if value >= 0 then
    return tostring(value)
  end
  -- The unsigned reading of the pattern: floor((U / 2) / 5) is
  -- floor(U / 10), and `>> 1` is a logical shift.
  local quotient = (value >> 1) // 5
  local remainder = value - quotient * 10
  return tostring(quotient) .. tostring(remainder)
end

function ZD.zd_num(value, path)
  if value ~= value or value == math.huge or value == -math.huge then
    error(path .. ": " .. tostring(value) .. " has no JSON form", 0)
  end
  return string.format("%.17g", value)
end
"#;

/// The Lua source of one dumper per type reachable from `root`, the
/// helpers ahead of them.
///
/// # Errors
/// A message naming the path in the type.
pub fn dump(ast: &Ast, root: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let mut reachable = Vec::new();
    collect(ast, root, &mut reachable, 0);
    let mut out = String::from(
        "\n-- One dumper per type, in the JSON dialect crates/wire-probe/src/lib.rs\n\
         -- documents. Not canonical form: the caller canonicalises in Rust.\n",
    );
    out.push_str(HELPERS);
    for id in reachable {
        out.push_str(&function(ast, id, union_label)?);
    }
    Ok(out)
}

/// Every type a value of `id` can hold, `id` included — the type graph
/// and not the value, so a dumper exists for an element whether the
/// sample held one or none.
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

/// A JSON object key with its colon, quoted for the generated text.
fn json_key(name: &str) -> String {
    format!("\"\\\"{}\\\":\"", escape_string(name).replace('"', "\\\""))
}

/// `v.name`, or `v["name"]` for a name Lua's dot syntax refuses.
fn field(name: &str) -> String {
    if is_identifier(name) {
        return format!("v.{name}");
    }
    format!("v[{}]", quoted(name))
}

/// `v.parent.name`, or `v.parent["name"]`: a member of `aggregate` as the generated type holds
/// it, through the members that hold its bases.
fn field_in(ast: &Ast, aggregate: Id<TypeNode>, member: &zerodds_ast::MemberNode) -> String {
    let path = zerodds_lang_rules::emit::member_field::<zerodds_lang_rules::emit::lua::Lua>(
        ast,
        aggregate,
        member.node_id,
        &member.name,
    );
    let prefix = &path[..path.len() - member.name.len()];
    if is_identifier(&member.name) {
        return format!("v.{prefix}{}", member.name);
    }
    format!(
        "v.{}[{}]",
        prefix.trim_end_matches('.'),
        quoted(&member.name)
    )
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", escape_string(text))
}

/// One dumper, as a whole function.
#[allow(clippy::too_many_lines)]
fn function(ast: &Ast, id: Id<TypeNode>, union_label: UnionLabelMatch) -> Result<String, String> {
    let name = dumper_name(ast, id);
    let node = ast.type_(id).map_err(|error| format!("{name}: {error}"))?;
    let mut body: Vec<String> = Vec::new();
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            body.push("return v and \"true\" or \"false\"".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::Float32 | PrimitiveKind::Float64) => {
            body.push("return ZD.zd_num(v, path)".to_string());
        }
        // The exact decimal the reader answered, as a JSON string: a
        // JSON number is a double and 60 of a binary128's 113
        // significand bits do not fit one.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            body.push("return ZD.zd_q(v)".to_string());
        }
        // Both 64-bit widths go out as decimal in a string, for every
        // value: a form that changed shape at 2^53 would compare
        // unequal across the boundary. The unsigned width reads its
        // pattern unsigned.
        TypeKind::Primitive(PrimitiveKind::Int64) => {
            body.push("return \"\\\"\" .. tostring(v) .. \"\\\"\"".to_string());
        }
        TypeKind::Primitive(PrimitiveKind::UInt64) => {
            body.push("return \"\\\"\" .. ZD.zd_u64(v) .. \"\\\"\"".to_string());
        }
        // The code point back as the one-character string the dialect
        // carries.
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            body.push("return ZD.zd_c(v, path)".to_string());
        }
        TypeKind::Primitive(_) => {
            body.push("return tostring(v)".to_string());
        }
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            body.push("return ZD.zd_q(v)".to_string());
        }
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{name}: the typedef target is unresolved"))?;
            body.push(format!("return ZD.{}(v, path)", dumper_name(ast, target)));
        }
        // The enumerator's name, chosen by the value; a value with no
        // enumerator has no name and is refused by path.
        TypeKind::Enumeration { .. } => {
            let mut pairs = Vec::new();
            for enumerator in ast.enumerators_of(id) {
                let Ok(node) = ast.enumerator(*enumerator) else {
                    continue;
                };
                let Some(value) = ast.effective_enumerator_value(*enumerator) else {
                    continue;
                };
                pairs.push(format!("[{value}] = {}", quoted(&node.name)));
            }
            body.push(format!("local zd_names = {{ {} }}", pairs.join(", ")));
            body.push("local zd_found = zd_names[v]".to_string());
            body.push("if zd_found == nil then".to_string());
            body.push(
                "  error(path .. \": \" .. tostring(v) .. \" names no enumerator of this type\", 0)"
                    .to_string(),
            );
            body.push("end".to_string());
            body.push("return ZD.zd_q(zd_found)".to_string());
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
            body.push(format!("local zd_outside = v & ~0x{accounted:X}"));
            body.push("if zd_outside ~= 0 then".to_string());
            body.push(
                "  error(path .. \": \" .. string.format(\"0x%x\", zd_outside) .. \
                 \" is not a bit this bitmask declares\", 0)"
                    .to_string(),
            );
            body.push("end".to_string());
            body.push("local zd_out = {}".to_string());
            for (bit, position) in &declared {
                body.push(format!("if (v >> {position}) & 1 == 1 then"));
                body.push(format!("  zd_out[#zd_out + 1] = ZD.zd_q({})", quoted(bit)));
                body.push("end".to_string());
            }
            body.push("return \"[\" .. table.concat(zd_out, \",\") .. \"]\"".to_string());
        }
        // The whole list, base first.
        TypeKind::Bitset { .. } => {
            body.push("local zd_out = {}".to_string());
            for field_id in &ast.all_bitfields_of(id) {
                let Ok(bitfield) = ast.bitfield(*field_id) else {
                    continue;
                };
                let Some(field_name) = bitfield.name.as_ref() else {
                    continue;
                };
                body.push(format!(
                    "zd_out[#zd_out + 1] = {} .. tostring({})",
                    json_key(field_name),
                    field(field_name)
                ));
            }
            body.push("return \"{\" .. table.concat(zd_out, \",\") .. \"}\"".to_string());
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the sequence element is unresolved"))?;
            body.push("local zd_out = {}".to_string());
            body.push("for zd_i = 1, #v do".to_string());
            body.push(format!(
                "  zd_out[#zd_out + 1] = ZD.{}(v[zd_i], path .. \"[\" .. (zd_i - 1) .. \"]\")",
                dumper_name(ast, element)
            ));
            body.push("end".to_string());
            body.push("return \"[\" .. table.concat(zd_out, \",\") .. \"]\"".to_string());
        }
        // One loop per dimension over the declared lengths: an IDL
        // array's length is in the type, and iterating the value would
        // print whatever a short table held without a word.
        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{name}: the array element is unresolved"))?;
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(
                    ast.eval_integral(*dim).map_err(|error| {
                        format!("{name}: a dimension does not evaluate: {error}")
                    })?,
                );
            }
            body.push(nested_array(&dumper_name(ast, element), &lengths));
        }
        TypeKind::Map { key, value, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{name}: the map key type is unresolved"))?;
            let value_type = value
                .resolved()
                .ok_or_else(|| format!("{name}: the map value type is unresolved"))?;
            body.push("local zd_out = {}".to_string());
            body.push("for zd_i = 1, #v do".to_string());
            body.push("  local zd_where = path .. \"[\" .. (zd_i - 1) .. \"]\"".to_string());
            body.push(format!(
                "  zd_out[#zd_out + 1] = \"[\" .. ZD.{}(v[zd_i].key, zd_where .. \".key\") \
                 .. \",\" .. ZD.{}(v[zd_i].value, zd_where .. \".value\") .. \"]\"",
                dumper_name(ast, key_type),
                dumper_name(ast, value_type)
            ));
            body.push("end".to_string());
            body.push("return \"[\" .. table.concat(zd_out, \",\") .. \"]\"".to_string());
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            body.push("local zd_out = {}".to_string());
            for member in ast.all_members_of(id) {
                let Ok(member) = ast.member(member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the member type is unresolved", member.name)
                })?;
                // A member of a base is reached through `parent` (XTypes 1.3 §7.5.1.4.1.3).
                let held = field_in(ast, id, member);
                let call = format!(
                    "ZD.{}({}, path .. {})",
                    dumper_name(ast, member_type),
                    held.as_str(),
                    quoted(&format!(".{}", member.name))
                );
                let append = format!("zd_out[#zd_out + 1] = {} .. {call}", json_key(&member.name));
                if member.facets.is_optional() {
                    // An absent member is a key that is not printed.
                    body.push(format!("if {held} ~= nil then"));
                    body.push(format!("  {append}"));
                    body.push("end".to_string());
                } else {
                    body.push(append);
                }
            }
            body.push("return \"{\" .. table.concat(zd_out, \",\") .. \"}\"".to_string());
        }
        TypeKind::Union { discriminator, .. } => {
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{name}: the discriminator is unresolved"))?;
            body.push("local zd_out = {}".to_string());
            body.push(format!(
                "zd_out[#zd_out + 1] = \"\\\"_d\\\":\" .. ZD.{}(v._d, path .. \"._d\")",
                dumper_name(ast, discriminator)
            ));
            // The branch the discriminator selects, and not every key
            // the table happens to hold: the defaulter fills the
            // default branch before the decoder runs, and printing by
            // presence would return the selected branch and the
            // defaulted one.
            let suppress_negative = union_negative_matches_nothing(ast, id, union_label);
            let mut arms: Vec<(String, String, Id<TypeNode>)> = Vec::new();
            let mut fallback: Option<(String, Id<TypeNode>)> = None;
            for case in ast.cases_of(id) {
                let Ok(case) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case.member) else {
                    continue;
                };
                let member_type = member.type_.resolved().ok_or_else(|| {
                    format!("{name}.{}: the case type is unresolved", member.name)
                })?;
                let mut tests: Vec<String> = Vec::new();
                let mut is_default = false;
                for label in &case.labels {
                    match label.expr() {
                        None => is_default = true,
                        Some(value_expr) => {
                            let value = ast.eval_integral(value_expr).map_err(|error| {
                                format!("{name}: a case label does not evaluate: {error}")
                            })?;
                            if suppress_negative && value < 0 {
                                continue;
                            }
                            tests.push(format!("v._d == {}", lua_label(ast, discriminator, value)));
                        }
                    }
                }
                if is_default {
                    fallback = Some((member.name.clone(), member_type));
                }
                if !tests.is_empty() {
                    arms.push((tests.join(" or "), member.name.clone(), member_type));
                }
            }
            let branch = |member_name: &str, member_type: Id<TypeNode>| {
                format!(
                    "  zd_out[#zd_out + 1] = {} .. ZD.{}({}, path .. {})",
                    json_key(member_name),
                    dumper_name(ast, member_type),
                    field(member_name),
                    quoted(&format!(".{member_name}"))
                )
            };
            for (index, (condition, member_name, member_type)) in arms.iter().enumerate() {
                let keyword = if index == 0 { "if" } else { "elseif" };
                body.push(format!("{keyword} {condition} then"));
                body.push(branch(member_name, *member_type));
            }
            if let Some((member_name, member_type)) = fallback {
                if arms.is_empty() {
                    body.push(branch(&member_name, member_type).trim_start().to_string());
                } else {
                    body.push("else".to_string());
                    body.push(branch(&member_name, member_type));
                }
            }
            if !arms.is_empty() {
                body.push("end".to_string());
            }
            body.push("return \"{\" .. table.concat(zd_out, \",\") .. \"}\"".to_string());
        }
        other => {
            return Err(format!(
                "{name}: {} has no Lua shape to print from",
                other.as_str()
            ));
        }
    }

    let mut out = format!("\nfunction ZD.{name}(v, path)\n");
    for line in body {
        let _ = writeln!(out, "  {line}");
    }
    out.push_str("end\n");
    Ok(out)
}

/// Nested loops over the declared dimensions, building nested JSON
/// arrays. §7.4.3.5 treats a multi-dimensional array as one flat run
/// with the first index varying most slowly, and nested arrays in
/// written order are that.
fn nested_array(element_dumper: &str, lengths: &[i128]) -> String {
    let mut out = String::new();
    let depth = lengths.len();
    // Innermost expression: the element at v[i1][i2]…
    let mut access = String::from("v");
    for level in 1..=depth {
        access.push_str(&format!("[zd_a{level}]"));
    }
    out.push_str("local zd_parts0 = {}\n");
    for (level, length) in lengths.iter().enumerate() {
        let indent = "  ".repeat(level);
        let _ = writeln!(out, "{indent}  for zd_a{} = 1, {length} do", level + 1);
        if level + 1 < depth {
            let _ = writeln!(out, "{indent}    local zd_parts{} = {{}}", level + 1);
        }
    }
    let inner_indent = "  ".repeat(depth);
    let _ = writeln!(
        out,
        "{inner_indent}  zd_parts{}[#zd_parts{} + 1] = ZD.{element_dumper}({access}, path)",
        depth - 1,
        depth - 1
    );
    for level in (0..depth).rev() {
        let indent = "  ".repeat(level);
        if level + 1 < depth {
            let _ = writeln!(
                out,
                "{indent}    zd_parts{level}[#zd_parts{level} + 1] = \"[\" .. \
                 table.concat(zd_parts{}, \",\") .. \"]\"",
                level + 1
            );
        }
        let _ = writeln!(out, "{indent}  end");
    }
    out.push_str("  return \"[\" .. table.concat(zd_parts0, \",\") .. \"]\"");
    out
}

/// A union label as a Lua literal in the discriminator's own holder: a
/// boolean discriminator holds `true`/`false`, a `char` the code point
/// as an integer — no rewrite — and a 64-bit label wraps to its hex
/// pattern.
fn lua_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> String {
    if matches!(
        resolve_primitive(ast, discriminator, 0),
        Some(PrimitiveKind::Boolean)
    ) {
        return if value == 0 { "false" } else { "true" }.to_string();
    }
    if i64::try_from(value).is_ok() {
        return format!("{value}");
    }
    u64::try_from(value).map_or_else(
        |_| format!("{value}"),
        |unsigned| format!("0x{unsigned:016X}"),
    )
}

/// The primitive kind a discriminator is, once typedefs are seen
/// through.
///
/// zerodds-lint: recursion-depth 64 (typedef chains only —
/// every other kind answers without a descent, so the walk never
/// reaches an aggregate and cannot cycle; bounded by [`MAX_DEPTH`])
fn resolve_primitive(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<PrimitiveKind> {
    if depth > MAX_DEPTH {
        return None;
    }
    match ast.type_(id).map(|node| node.kind.clone()) {
        Ok(TypeKind::Primitive(kind)) => Some(kind),
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .and_then(|target| resolve_primitive(ast, target, depth + 1)),
        _ => None,
    }
}
