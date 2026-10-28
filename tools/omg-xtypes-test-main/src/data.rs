// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The sample the run publishes, and the sample the run expects to
//! receive.
//!
//! The suite ships each sample twice — `data/json/<name>.json` and
//! `data/xml/<name>.xml` — and its own C client reads the XML. This
//! client reads the JSON, because `zerodds_wire_codec` already
//! implements its value traits for `serde_json::Value`
//! (`crates/wire-codec/src/json.rs`) and a second value representation
//! here would be a second answer to every question that file settles:
//! how a 64-bit integer is written so that reading it back does not
//! change it, how a `long double` is written, what a NaN is called.
//!
//! Each side loads **its own** `--data-file`, which is what the suite
//! intends: `ext_appendable_struct_3` publishes `struct_num_x1_x2` from
//! an appendable two-member type and subscribes `struct_num_x1` from the
//! one-member type, and the subscriber is right to expect only `x1`.

use std::path::{Path, PathBuf};

use zerodds_ast::{TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The paths the suite's layout puts a data file at, in the order they
/// are tried.
///
/// The JSON comes first because it is the representation this client
/// reads natively. The XML beside it is the fallback and not decoration:
/// the suite ships 49 samples as XML and 43 as JSON, and of the files
/// its **active** cases name, exactly one has no JSON twin —
/// `data/xml/union_bitmask.xml`, the subscriber's sample for
/// `union_uint32_bitmask32` and `union_uint32_bitmask16`. Without the
/// fallback that is a sample this client has none of, which reaches the
/// suite's verdict as `DATA_NOT_CORRECT` rather than as a missing file.
/// The other eight untwinned XML files are named only by cases the suite
/// has commented out.
#[must_use]
pub fn candidate_paths(folder: Option<&str>, file: Option<&str>) -> Vec<PathBuf> {
    let stem = file.unwrap_or("");
    match folder {
        Some(dir) => vec![
            Path::new(dir).join("json").join(format!("{stem}.json")),
            Path::new(dir).join(format!("{stem}.json")),
            Path::new(dir).join("xml").join(format!("{stem}.xml")),
            Path::new(dir).join(format!("{stem}.xml")),
        ],
        None => vec![
            PathBuf::from(format!("{stem}.json")),
            PathBuf::from(format!("{stem}.xml")),
            PathBuf::from(stem),
        ],
    }
}

/// Reads the sample the command line names.
///
/// # Errors
/// A message naming every path that was tried, or the parse error.
pub fn load(folder: Option<&str>, file: Option<&str>) -> Result<serde_json::Value, String> {
    if file.is_none() && folder.is_none() {
        return Err("no --data-folder or --data-file".to_owned());
    }
    let tried = candidate_paths(folder, file);
    let path = tried.iter().find(|p| p.is_file()).ok_or_else(|| {
        format!(
            "no data file; tried {}",
            tried
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )
    })?;
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
    {
        return from_xml(&text).map_err(|e| format!("{}: {e}", path.display()));
    }
    serde_json::from_str(&text).map_err(|e| format!("{} is not JSON: {e}", path.display()))
}

/// One of the suite's XML samples, in the shape its JSON twin has.
///
/// The two representations differ in three ways and in no other:
///
/// * The root element names the type and carries no value —
///   `<union_bitmask>`, `<struct>`, `<strings>` — so it is dropped, as
///   the outermost `{}` of the JSON is.
/// * A repeated element is spelled `<item>`, which is the one child name
///   that means "this is a collection, not an aggregate".
/// * A union writes its discriminator as `<discriminator>`, which is
///   `_d`.
///
/// Every leaf comes back as a string. What each one *is* — a number, a
/// character, an enumerator name — is decided later by [`complete`],
/// which has the type in hand; deciding it here from the spelling alone
/// would turn `data/xml/struct_char_x1.xml`'s `0x01` into the number 1
/// for a `char` member that wants the character.
///
/// # Errors
/// The XML error, for a file that does not parse.
pub fn from_xml(text: &str) -> Result<serde_json::Value, String> {
    let document = zerodds_xml::parse_xml_tree(text).map_err(|e| format!("{e}"))?;
    Ok(xml_value(&document.root))
}

/// The XML element's value: an object, an array, or the leaf's text.
/// zerodds-lint: recursion-depth 64 (element nesting; `parse_xml_tree`
/// runs `precheck_depth` before it builds the tree, so no element deeper
/// than `zerodds_xml::MAX_TREE_DEPTH` reaches this function)
fn xml_value(element: &zerodds_xml::XmlElement) -> serde_json::Value {
    if element.children.is_empty() {
        return serde_json::Value::String(element.text.trim().to_owned());
    }
    if element.children.iter().all(|child| child.name == "item") {
        return serde_json::Value::Array(element.children.iter().map(xml_value).collect());
    }
    let mut out = serde_json::Map::new();
    for child in &element.children {
        let name = if child.name == "discriminator" {
            String::from("_d")
        } else {
            child.name.clone()
        };
        out.insert(name, xml_value(child));
    }
    serde_json::Value::Object(out)
}

/// The sample the file holds, with every union's discriminator filled in.
///
/// The suite ships each sample twice and the two are not the same shape.
/// `data/xml/union_uint32.xml` writes the discriminator out —
/// `<discriminator>0x02</discriminator><x1>128</x1>` — and
/// `data/json/union_uint32.json` is `{"x1":128}`, the branch alone.
/// `zerodds_wire_codec`'s value contract needs `_d` (see
/// `crates/wire-codec/src/canonical.rs`), so the JSON as it stands
/// encodes to `Missing { path: "…._d" }` and no union sample is ever
/// written.
///
/// The branch decides the discriminator, which is what the XML file
/// writes by hand: the named member belongs to exactly one case, and
/// that case's first explicit label is the value that selects it. A case
/// reached only through `default:` takes the smallest non-negative value
/// no explicit label claims, which is the definition of "selects the
/// default branch".
///
/// # Errors
/// A union whose named member belongs to no case, which is a sample for
/// a different type.
pub fn complete(
    ast: &zerodds_ast::Ast,
    type_: Id<TypeNode>,
    value: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    complete_at(ast, type_, value, 0)
}

/// How deep a value walk goes before the type is taken for a cycle. The
/// same bound the model's own reference walks carry.
const MAX_VALUE_DEPTH: usize = 64;

/// zerodds-lint: recursion-depth 64 (member and element nesting; the
/// `depth` parameter is checked against `MAX_VALUE_DEPTH` on entry)
fn complete_at(
    ast: &zerodds_ast::Ast,
    type_: Id<TypeNode>,
    value: &serde_json::Value,
    depth: usize,
) -> Result<serde_json::Value, String> {
    if depth > MAX_VALUE_DEPTH {
        return Ok(value.clone());
    }
    let Some(type_) = resolve_aliases(ast, type_, 0) else {
        return Ok(value.clone());
    };
    let node = ast
        .type_(type_)
        .map_err(|e| format!("the model lost a type under the sample walk: {e}"))?;
    match &node.kind {
        TypeKind::Structure { members, .. } => {
            let serde_json::Value::Object(object) = value else {
                return Ok(value.clone());
            };
            let mut out = object.clone();
            for member in members {
                let Ok(member) = ast.member(*member) else {
                    continue;
                };
                let Some(held) = object.get(&member.name) else {
                    continue;
                };
                let Some(member_type) = member.type_.resolved() else {
                    continue;
                };
                out.insert(
                    member.name.clone(),
                    complete_at(ast, member_type, held, depth + 1)?,
                );
            }
            Ok(serde_json::Value::Object(out))
        }
        TypeKind::Sequence { element, .. } => {
            let serde_json::Value::Array(items) = value else {
                return Ok(value.clone());
            };
            let Some(element) = element.resolved() else {
                return Ok(value.clone());
            };
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(complete_at(ast, element, item, depth + 1)?);
            }
            Ok(serde_json::Value::Array(out))
        }
        TypeKind::Array { element, dims } => {
            let serde_json::Value::Array(items) = value else {
                return Ok(value.clone());
            };
            let Some(element) = element.resolved() else {
                return Ok(value.clone());
            };
            let counts: Vec<usize> = dims
                .iter()
                .filter_map(|dim| ast.eval_integral(*dim).ok())
                .filter_map(|count| usize::try_from(count).ok())
                .collect();
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(complete_at(ast, element, item, depth + 1)?);
            }
            Ok(fold_dimensions(out, &counts))
        }
        TypeKind::Union { .. } => {
            let serde_json::Value::Object(object) = value else {
                return Ok(value.clone());
            };
            let mut out = object.clone();
            // The branch first, so a union inside a union is filled in
            // before its own discriminator is read back.
            let named: Option<String> = object.keys().find(|k| *k != "_d").cloned();
            if let Some(name) = &named {
                if let Some(case_member) = union_member(ast, type_, name) {
                    if let Some(member_type) = case_member.resolved() {
                        if let Some(held) = object.get(name) {
                            out.insert(
                                name.clone(),
                                complete_at(ast, member_type, held, depth + 1)?,
                            );
                        }
                    }
                }
            }
            if let Some(held) = object.get("_d") {
                // A file that writes the discriminator out writes it as
                // an integer — `2` in JSON, `2` or `0x02` in XML — and
                // an enumerated or bitmask discriminator is named.
                if let Some(number) = integer_literal(held) {
                    out.insert(String::from("_d"), discriminator_value(ast, type_, number));
                }
                return Ok(serde_json::Value::Object(out));
            }
            let Some(name) = named else {
                return Err(format!(
                    "the sample for union `{}` names no branch and no `_d`",
                    node.name
                ));
            };
            let discriminant = discriminant_for(ast, type_, &name)
                .ok_or_else(|| format!("union `{}` has no case for branch `{name}`", node.name))?;
            out.insert(
                String::from("_d"),
                discriminator_value(ast, type_, discriminant),
            );
            Ok(serde_json::Value::Object(out))
        }
        _ => Ok(leaf(ast, type_, value)),
    }
}

/// A leaf as the codec's value contract wants it written.
///
/// The JSON samples already carry their kinds; the XML samples carry
/// every leaf as a string, because the text alone does not say whether
/// `0x01` is the number 1 or the character it encodes. Here the type
/// says. A value that is not a string is left alone, so a JSON file
/// decides its own leaves and this walk cannot change one.
///
/// `wchar` is the one leaf a `char` rule cannot be reused for: the two
/// take the same spellings and the codec asks for a one-character string
/// for both (`crates/wire-codec/src/decode.rs`, `selector_of`), so they
/// share the arm.
fn leaf(
    ast: &zerodds_ast::Ast,
    type_: Id<TypeNode>,
    value: &serde_json::Value,
) -> serde_json::Value {
    let serde_json::Value::String(text) = value else {
        return value.clone();
    };
    let Ok(node) = ast.type_(type_) else {
        return value.clone();
    };
    use zerodds_ast::PrimitiveKind as P;
    let TypeKind::Primitive(kind) = node.kind else {
        // An enumerator and a bit name are their own spelling, and a
        // string is itself.
        return value.clone();
    };
    match kind {
        P::Boolean => match text.trim() {
            "true" | "TRUE" | "1" => serde_json::Value::Bool(true),
            "false" | "FALSE" | "0" => serde_json::Value::Bool(false),
            _ => value.clone(),
        },
        P::Char | P::WChar => integer_text(text).map_or_else(
            || value.clone(),
            |code| {
                u32::try_from(code)
                    .ok()
                    .and_then(char::from_u32)
                    .map_or_else(|| value.clone(), |ch| serde_json::Value::String(ch.into()))
            },
        ),
        P::Float32 | P::Float64 => text
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map_or_else(|| value.clone(), serde_json::Value::Number),
        // A `long double` is left as it stands. `Source::as_float128`
        // (`crates/wire-codec/src/json.rs`) reads it from a string
        // already, and reads more of them than a route through `f64`
        // could: `data/json/struct_float128_x1.json` is
        // `"0x0000000000000000000000000000fe3f"`, 32 hex digits of
        // binary128, and every decimal that reaches it keeps the digits
        // an `f64` would drop.
        P::Float128 => value.clone(),
        _ => integer_text(text).map_or_else(|| value.clone(), serde_json::Value::from),
    }
}

/// An integer the sample writes as a number or as text, decimal or
/// hexadecimal.
///
/// `data/xml/union_uint32.xml` writes `0x02` and
/// `data/xml/union_bitmask.xml` writes `2` for the same discriminator
/// value, so both spellings are one value.
fn integer_literal(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(_) => value.as_i64(),
        serde_json::Value::String(text) => integer_text(text),
        _ => None,
    }
}

/// `2`, `0x02`, `-1` — the integer a piece of sample text spells.
fn integer_text(text: &str) -> Option<i64> {
    let text = text.trim();
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1i64, rest),
        None => (1i64, text),
    };
    let magnitude = match digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        Some(hex) => i64::from_str_radix(hex, 16).ok()?,
        None => digits.parse::<i64>().ok()?,
    };
    Some(sign * magnitude)
}

/// A flat sample laid out over the array's dimensions.
///
/// The suite writes a multi-dimensional array as one flat run:
/// `data/json/array_num_20.json` is twenty numbers, and `types/xml/
/// arrays.xml` gives `int32x10x2` the member `int32 arrayDimensions=
/// "10,2"`. XTypes 1.3 §7.4.3.5 says that run *is* the array, first
/// index varying most slowly, so the file is right; what it does not
/// carry is the nesting `zerodds_wire_codec` asks for, one JSON level
/// per dimension (`crates/wire-codec/src/encode.rs`,
/// `write_array_dimension`). The run is folded here rather than every
/// data file in the suite being rewritten.
///
/// A sample that already carries its nesting is left alone: the fold
/// happens only where the run's length is the product of every
/// dimension and is not already the outermost dimension, which is the
/// one shape a nested sample cannot have.
fn fold_dimensions(items: Vec<serde_json::Value>, counts: &[usize]) -> serde_json::Value {
    let total: usize = counts.iter().product();
    if counts.len() < 2 || items.len() != total || items.len() == counts[0] {
        return serde_json::Value::Array(items);
    }
    let mut level = items;
    for count in counts.iter().skip(1).rev() {
        if *count == 0 {
            return serde_json::Value::Array(level);
        }
        level = level
            .chunks(*count)
            .map(|chunk| serde_json::Value::Array(chunk.to_vec()))
            .collect();
    }
    serde_json::Value::Array(level)
}

/// The discriminator as the codec's value contract wants it written.
///
/// XTypes 1.3 §7.4.3.5.3 rule (5) encodes an enumerated discriminator
/// from its enumerator, and `zerodds_wire_codec` asks for that
/// enumerator by name (`crates/wire-codec/src/encode.rs`,
/// `enumerator_wire_value`). The suite's JSON names the branch and not
/// the discriminator, so the value the branch's label carries is turned
/// back into the enumerator that holds it.
///
/// A bitmask discriminator — which DDS-XTypes 1.3 §7.5.2.6 admits in
/// XML and `types/xml/unions_bitmask.xml` uses — is the same question
/// with a different answer: the codec asks for the *set of bit names*
/// (`crates/wire-codec/src/contract.rs`, `bit_names`), so the label's
/// value is taken apart into the bits it sets. Every other discriminator
/// type stays a number.
fn discriminator_value(
    ast: &zerodds_ast::Ast,
    union: Id<TypeNode>,
    discriminant: i64,
) -> serde_json::Value {
    if let Some(names) = bit_names_of(ast, union, discriminant) {
        return names;
    }
    enumerator_named(ast, union, discriminant).map_or_else(
        || serde_json::Value::Number(discriminant.into()),
        serde_json::Value::String,
    )
}

/// The bit names this discriminant sets, where the discriminator is a
/// bitmask at all.
///
/// A bit the bitmask does not declare has no name to write, and a value
/// carrying one is a sample this type cannot hold — `read_bitmask`
/// refuses the same stream — so the whole conversion is declined and the
/// number stands, which fails loudly at the encoder rather than silently
/// dropping the bit.
fn bit_names_of(
    ast: &zerodds_ast::Ast,
    union: Id<TypeNode>,
    discriminant: i64,
) -> Option<serde_json::Value> {
    let TypeKind::Union { discriminator, .. } = &ast.type_(union).ok()?.kind else {
        return None;
    };
    let discriminator = resolve_aliases(ast, discriminator.resolved()?, 0)?;
    if !matches!(
        ast.type_(discriminator).ok()?.kind,
        TypeKind::Bitmask { .. }
    ) {
        return None;
    }
    let bits = u64::try_from(discriminant).ok()?;
    let mut names = Vec::new();
    let mut accounted: u64 = 0;
    for id in ast.bits_of(discriminator) {
        let node = ast.bit_value(*id).ok()?;
        let position = ast.effective_bit_position(*id)?;
        if position >= 64 {
            continue;
        }
        let mask = 1u64 << position;
        accounted |= mask;
        if bits & mask != 0 {
            names.push(serde_json::Value::String(node.name.clone()));
        }
    }
    if bits & !accounted != 0 {
        return None;
    }
    Some(serde_json::Value::Array(names))
}

/// The name of the discriminator enumerator carrying this value, where
/// the discriminator is an enumeration at all.
fn enumerator_named(
    ast: &zerodds_ast::Ast,
    union: Id<TypeNode>,
    discriminant: i64,
) -> Option<String> {
    let TypeKind::Union { discriminator, .. } = &ast.type_(union).ok()?.kind else {
        return None;
    };
    let discriminator = resolve_aliases(ast, discriminator.resolved()?, 0)?;
    if !matches!(
        ast.type_(discriminator).ok()?.kind,
        TypeKind::Enumeration { .. }
    ) {
        return None;
    }
    for id in ast.enumerators_of(discriminator) {
        if ast.effective_enumerator_value(*id) == Some(i128::from(discriminant)) {
            return Some(ast.enumerator(*id).ok()?.name.clone());
        }
    }
    None
}

/// zerodds-lint: recursion-depth 64 (alias chains; the `depth`
/// parameter is checked against `MAX_VALUE_DEPTH` on entry)
fn resolve_aliases(
    ast: &zerodds_ast::Ast,
    start: Id<TypeNode>,
    depth: usize,
) -> Option<Id<TypeNode>> {
    if depth > MAX_VALUE_DEPTH {
        return None;
    }
    match &ast.type_(start).ok()?.kind {
        TypeKind::Alias { aliased } => resolve_aliases(ast, aliased.resolved()?, depth + 1),
        _ => Some(start),
    }
}

/// The type of the union branch of this name.
fn union_member(
    ast: &zerodds_ast::Ast,
    union: Id<TypeNode>,
    name: &str,
) -> Option<zerodds_ast_core::Ref<TypeNode>> {
    for case in ast.cases_of(union) {
        let case = ast.case(*case).ok()?;
        let member = ast.member(case.member).ok()?;
        if member.name == name {
            return Some(member.type_.clone());
        }
    }
    None
}

/// The discriminator value that selects the branch of this name.
fn discriminant_for(ast: &zerodds_ast::Ast, union: Id<TypeNode>, name: &str) -> Option<i64> {
    let cases = ast.cases_of(union).to_vec();
    let mut claimed: Vec<i128> = Vec::new();
    let mut wanted: Option<Id<zerodds_ast::CaseNode>> = None;
    for case in &cases {
        let Ok(node) = ast.case(*case) else { continue };
        if ast.member(node.member).is_ok_and(|m| m.name == name) {
            wanted = Some(*case);
        }
        for label in &node.labels {
            if let Some(expr) = label.expr() {
                if let Ok(value) = ast.eval_integral(expr) {
                    claimed.push(value);
                }
            }
        }
    }
    let wanted = ast.case(wanted?).ok()?;
    for label in &wanted.labels {
        if let Some(expr) = label.expr() {
            if let Ok(value) = ast.eval_integral(expr) {
                return i64::try_from(value).ok();
            }
        }
    }
    // A `default:`-only branch: any value no explicit label claims
    // selects it, and the smallest non-negative one is the plainest.
    (0..=i64::from(u16::MAX)).find(|candidate| !claimed.contains(&i128::from(*candidate)))
}

/// Whether the sample that arrived is the sample that was loaded.
///
/// The two are not the same text and are not meant to be. What the file
/// says is what its author wrote; what comes off the wire is what this
/// side's type made of it. `ext_appendable_struct_2` publishes
/// `struct_a1` with `{"x1":1}` and subscribes `struct_a2`, whose second
/// member the writer never sent: XTypes 1.3 §7.2.4.4.4.4 has the reader
/// supply it, so `{"x1":1,"x2":0}` arrives and is correct.
///
/// So the loaded value is completed against this side's type and put
/// through this side's own codec before the comparison:
///
/// 1. `zerodds_wire_codec::default_of_type` builds the type's default
///    value — XTypes 1.3 Table 9, the same defaults the decoder supplies
///    for a member a writer did not send.
/// 2. The loaded file is overlaid on it, member by member. A file that
///    names fewer members than the type has says nothing about the rest,
///    and the type says what the rest are.
/// 3. The result is encoded and decoded again, which settles the
///    spellings — an integer written as `1` against `"1"`, a float
///    written as `30` against `30.0` — by running them through the code
///    under test rather than through a second rule written here.
#[must_use]
pub fn equivalent(expected: &serde_json::Value, found: &serde_json::Value) -> bool {
    mismatch(expected, found).is_none()
}

/// Why `found` is not the loaded sample, or `None` when it is: the
/// completed expectation and what its round trip through the codec
/// gave, so that a `DATA_NOT_CORRECT` verdict names the value it was
/// compared against and not only the file's shorthand.
pub fn mismatch(expected: &serde_json::Value, found: &serde_json::Value) -> Option<String> {
    let cx = crate::carrier::context();
    // The union discriminator first, for the same reason the publish
    // side fills it: the suite's JSON names the branch and not `_d`, and
    // a `_d` taken from Table 9's default would select a different
    // branch or none.
    let expected =
        &complete(&cx.model.ast, cx.model.type_, expected).unwrap_or_else(|_| expected.clone());
    let completed = zerodds_wire_codec::default_of_type::<serde_json::Value>(
        &cx.model.ast,
        cx.model.type_,
        &cx.model.type_name,
    )
    .ok()
    .map_or_else(|| expected.clone(), |base| overlay(&base, expected));
    let roundtrip = |v: &serde_json::Value| -> Option<serde_json::Value> {
        let bytes = zerodds_wire_codec::encode(&cx.model.ast, &cx.request, v).ok()?;
        zerodds_wire_codec::decode(&cx.model.ast, &cx.request, &bytes).ok()
    };
    match roundtrip(&completed) {
        Some(want) if &want == found => None,
        Some(want) => Some(format!(
            "expected {want} (completed {completed}) got {found}"
        )),
        // A value that will not round-trip is compared as it stands
        // rather than declared equal.
        None if &completed == found => None,
        None => Some(format!(
            "expected {completed} (did not round-trip through the codec) got {found}"
        )),
    }
}

/// `over` written on top of `base`, member by member, recursively.
///
/// Only the members `over` names are taken; a member it does not name
/// keeps the value `base` has for it. Anything that is not an object on
/// both sides is replaced outright, which is what a scalar, a sequence
/// and a union branch all need.
/// zerodds-lint: recursion-depth 128 (object nesting; both arguments
/// come from `serde_json::from_str`, whose own parser refuses input
/// nested deeper than its recursion limit, so no value this function
/// sees can exceed it)
fn overlay(base: &serde_json::Value, over: &serde_json::Value) -> serde_json::Value {
    match (base, over) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            let mut out = b.clone();
            for (k, v) in o {
                let merged = out.get(k).map_or_else(|| v.clone(), |b| overlay(b, v));
                out.insert(k.clone(), merged);
            }
            serde_json::Value::Object(out)
        }
        _ => over.clone(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// `src/c/test_main.c` builds `<data-folder>/xml/<file>.xml`; the
    /// JSON beside it is what this client reads.
    #[test]
    fn the_suites_layout_is_the_first_candidate() {
        let c = candidate_paths(Some("data"), Some("struct_num_x1"));
        assert_eq!(c[0], PathBuf::from("data/json/struct_num_x1.json"));
    }

    /// A run with neither option says so rather than publishing an empty
    /// sample.
    #[test]
    fn no_data_option_is_an_error() {
        assert!(load(None, None).is_err());
    }

    /// `data/json/array_num_20.json` against `int32 arrayDimensions=
    /// \"10,2\"`: twenty numbers become ten pairs, first index slowest.
    #[test]
    fn a_flat_run_is_folded_over_the_dimensions() {
        let items: Vec<serde_json::Value> = (1..=20).map(serde_json::Value::from).collect();
        let folded = fold_dimensions(items, &[10, 2]);
        let serde_json::Value::Array(rows) = &folded else {
            panic!("not an array: {folded}");
        };
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0], serde_json::json!([1, 2]));
        assert_eq!(rows[9], serde_json::json!([19, 20]));
    }

    /// A sample that already carries its nesting is passed through.
    #[test]
    fn a_nested_run_is_left_alone() {
        let items = vec![serde_json::json!([1, 2]), serde_json::json!([3, 4])];
        assert_eq!(
            fold_dimensions(items, &[2, 2]),
            serde_json::json!([[1, 2], [3, 4]])
        );
    }

    /// One dimension is never folded.
    #[test]
    fn a_single_dimension_is_never_folded() {
        let items: Vec<serde_json::Value> = (1..=3).map(serde_json::Value::from).collect();
        assert_eq!(fold_dimensions(items, &[3]), serde_json::json!([1, 2, 3]));
    }
}
