// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada 83 expression one JSON sample is.
//!
//! `wireprobe-zerodds-ada-gen/src/fill.rs` is the template; every row
//! of its dialect table carries over unchanged (a 64-bit integer is a
//! decimal in a string, an absent `@optional` member is a key left
//! out, a map is an array of two-element arrays). What differs is the
//! spelling this column's [`crate::adapter::ada83`] materialisation
//! gives each shape:
//!
//! | IDL | Ada 83 | what this writes |
//! |---|---|---|
//! | `boolean` | `Boolean` | `True` / `False` |
//! | `char` / `wchar` | `Long_Integer` | `Zd_Wire83.Checked_Char/Wchar (<code>)` |
//! | integers | `Long_Integer` | the bare number (universal integer) |
//! | `float`, `double` | `Float` / `Long_Float` | a literal, or `Zd_Wire83.F64_Nan` and kin |
//! | `long double`, `fixed`, `string`, `wstring` | `Zd_Wire83.Text` | `Text_Of ("…")`, or `Text_From_Utf8` past ASCII |
//! | `sequence<T>` | access to array | `new …_Array'(0 => …)`, `(0 .. -1)` empty |
//! | `T[N]` | constrained array | `(0 => …, 1 => …)` |
//! | `map<K,V>` | access to entry array | entries as `(Key => …, Value => …)` |
//! | `enum` | a named constant | the constant's name |
//! | `bitmask` | `Long_Integer` | the sum of the named bits |
//! | `struct`, `union`, `bitset` | records | one aggregate |
//! | `@optional T` | `Zd_Opt_…` | `(Present => True, Value => …)` |
//!
//! ## The one shape that is not an expression
//!
//! A `struct`/`valuetype` with **no members at all**, and a `bitset`
//! with no field, has no aggregate here — RM83 4.3.1 requires at least
//! one `component_association`, and `(null record)`, the spelling later
//! Ada gives the empty case, is an Ada 2005 addition (AI95-287)
//! `zerodds_lang_cst::ada83` does not carry. It needs none: RM83 3.2.1
//! lets an object declaration stand without an initial value, and a
//! record with no components has no component left uninitialised by
//! one. [`fill`] declares one such object per type in a `declare` block
//! (RM83 5.6) around the assignment and names it where an aggregate
//! would have stood; the *type* was never in question, `is null
//! record;` being RM83 3.8.

use serde_json::Value;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::ada83::{
    DISCRIMINANT, PRESENT, VALUE, ada83_pattern, ada83_type_name, component_name,
};

/// How deep an alias chain is followed before the model is taken to be
/// malformed.
const MAX_DEPTH: usize = 64;

/// One record type with no components at all, as the object this
/// filler declares for it: the Ada 83 identifier and its subtype mark.
///
/// See [`empty_object`] for why an object stands where an aggregate
/// would.
type EmptyRecord = (String, String);

/// The statements that build `values` into `into`, whose declaration is
/// the caller's: one whole-value assignment.
///
/// A sample that reaches a record with no components at all carries one
/// `declare` around that assignment, holding one object per such type —
/// RM83 4.3.1 gives the shape no aggregate, and RM83 5.6 puts the
/// declarative part a block statement needs exactly here.
///
/// # Errors
/// A message naming the path in the value.
pub fn fill(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
    into: &str,
) -> Result<Vec<String>, String> {
    let mut empties: Vec<EmptyRecord> = Vec::new();
    let expr = emit(ast, type_, values, "sample", 0, &mut empties)?;
    let assignment = format!("{into} := {expr};");
    if empties.is_empty() {
        return Ok(vec![assignment]);
    }
    let mut out = vec!["declare".to_string()];
    for (name, mark) in &empties {
        out.push(format!("   {name} : {mark};"));
    }
    out.push("begin".to_string());
    out.push(format!("   {assignment}"));
    out.push("end;".to_string());
    Ok(out)
}

/// The Ada 83 name standing for a value of a record type with no
/// components at all, declared once per type by [`fill`].
///
/// RM83 4.3.1 requires a record aggregate to carry at least one
/// `component_association`, and `(null record)` — the spelling later
/// Ada gives the empty case — is an Ada 2005 addition (AI95-287)
/// `zerodds_lang_cst::ada83` does not write. None is needed: RM83 3.2.1
/// lets an object declaration stand without an initial value, and a
/// record with no components has no component left uninitialised by
/// one, so the object *is* the whole value of the type. The *type* was
/// never in question — `is null record;` is RM83 3.8.
fn empty_object(ast: &Ast, type_: Id<TypeNode>, empties: &mut Vec<EmptyRecord>) -> String {
    let mark = ada83_type_name(ast, type_);
    let name = format!("zd83_empty_{mark}");
    if !empties.iter().any(|(other, _)| other == &name) {
        empties.push((name.clone(), mark));
    }
    name
}

/// The Ada 83 expression `value` is under `type_`.
/// zerodds-lint: recursion-depth 64 (member, case, collection and alias
/// nesting; bounded by [`MAX_DEPTH`], which refuses with the path in
/// the sample. The walk pairs a type with the part of the sample that
/// fills it, and a recursive IDL type ends only because the sample
/// ends — a termination argument, not a bound, so the bound is the
/// constant.)
#[allow(clippy::too_many_lines)]
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    path: &str,
    depth: usize,
    empties: &mut Vec<EmptyRecord>,
) -> Result<String, String> {
    if depth > MAX_DEPTH {
        return Err(format!("{path}: the type nests past {MAX_DEPTH} levels"));
    }
    let node = ast
        .type_(type_)
        .map_err(|error| format!("{path}: {error}"))?;
    match &node.kind {
        TypeKind::Primitive(kind) => primitive_literal(*kind, value, path),

        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a string — a decimal must not arrive as a JSON number")
            })?;
            Ok(text_expr(text))
        }

        TypeKind::Enumeration { .. } => {
            let name = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected an enumerator name, as a string"))?;
            let mut declared = Vec::new();
            for id in ast.enumerators_of(type_) {
                let Ok(enumerator) = ast.enumerator(*id) else {
                    continue;
                };
                declared.push(enumerator.name.clone());
                if enumerator.name == name {
                    return Ok(component_name(name));
                }
            }
            Err(format!("{path}: `{name}` is not one of {declared:?}"))
        }

        TypeKind::Bitmask { .. } => {
            let names = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected an array of bit names"))?;
            let mut declared = Vec::new();
            for id in ast.bits_of(type_) {
                let Ok(bit) = ast.bit_value(*id) else {
                    continue;
                };
                declared.push(bit.name.clone());
            }
            let mut bits: u128 = 0;
            for entry in names {
                let name = entry
                    .as_str()
                    .ok_or_else(|| format!("{path}: a bit name is a string"))?;
                let mut found = false;
                for id in ast.bits_of(type_) {
                    let Ok(bit) = ast.bit_value(*id) else {
                        continue;
                    };
                    if bit.name != name {
                        continue;
                    }
                    let position = ast.effective_bit_position(*id).ok_or_else(|| {
                        format!("{path}: `{name}` has no position the model can state")
                    })?;
                    if position >= 64 {
                        return Err(format!("{path}: `{name}` sits at bit {position}, past 64"));
                    }
                    bits |= 1u128 << position;
                    found = true;
                    break;
                }
                if !found {
                    return Err(format!("{path}: `{name}` is not one of {declared:?}"));
                }
            }
            // Bit 63 as the two's-complement pattern, not as a refusal:
            // Table 33 admits a `@bit_bound(64)` bitmask and
            // `@position(63)` with it, and `runtime/ada-83`'s
            // `Write_Bitmask` 64-bit arm now carries that value the way
            // `Write_U64` carries an `unsigned long long` past the same
            // ceiling. A single `Long_Integer` holds every one of the
            // sixty-four bits; what it does not hold is the *magnitude*,
            // and nothing here needs one.
            let pattern = i128::try_from(bits)
                .map_err(|_| format!("{path}: the bits set do not fit sixty-four"))?;
            Ok(format!("{}", ada83_pattern(pattern)))
        }

        TypeKind::Bitset { .. } => {
            let fields = ast.all_bitfields_of(type_);
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected an object keyed by bitfield name"))?;
            let declared: Vec<String> = fields
                .iter()
                .filter_map(|id| ast.bitfield(*id).ok())
                .filter_map(|field| field.name.clone())
                .collect();
            for name in object.keys() {
                if !declared.contains(name) {
                    return Err(format!("{path}.{name}: not a bitfield; {declared:?}"));
                }
            }
            let mut parts = Vec::new();
            for id in &fields {
                let Ok(field) = ast.bitfield(*id) else {
                    continue;
                };
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let given = object.get(name).ok_or_else(|| {
                    format!("{path}.{name}: no value, and a bitfield is never optional")
                })?;
                let number = given
                    .as_u64()
                    .ok_or_else(|| format!("{path}.{name}: expected a number"))?;
                parts.push(format!("{} => {number}", component_name(name)));
            }
            if parts.is_empty() {
                return Ok(empty_object(ast, type_, empties));
            }
            Ok(format!("({})", parts.join(", ")))
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected an object"))?;
            let members: Vec<_> = ast
                .all_members_of(type_)
                .iter()
                .filter_map(|id| ast.member(*id).ok())
                .filter_map(|member| {
                    member.type_.resolved().map(|target| {
                        let optional = member.facets.is_optional();
                        let external = member
                            .facets
                            .xtypes
                            .as_ref()
                            .is_some_and(|xtypes| xtypes.is_external);
                        // An access type where the adapter declares
                        // one (`held_by_reference`): the value is an
                        // allocator, absence is `null`.
                        let by_reference = zerodds_lang_rules::adapter::ada83::held_by_reference(
                            ast, type_, target, optional, external,
                        );
                        (member.name.clone(), target, optional, by_reference)
                    })
                })
                .collect();
            let declared: Vec<&str> = members
                .iter()
                .map(|(name, _, _, _)| name.as_str())
                .collect();
            zerodds_wireprobe_gen::fill::refuse_undeclared(object, &declared, path)?;
            let mut parts = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3): it takes the
            // keys of its own members, and this structure the others.
            let inherited = zerodds_wireprobe_gen::fill::Inherited::of(ast, type_, object);
            if let Some(inherited) = &inherited {
                let expr = emit(
                    ast,
                    inherited.base,
                    &inherited.part,
                    &format!("{path}.parent"),
                    depth + 1,
                    empties,
                )?;
                parts.push(format!("{} => {expr}", component_name("parent")));
            }
            let members: Vec<_> = members
                .into_iter()
                .filter(|(name, _, _, _)| {
                    zerodds_wireprobe_gen::fill::writes_member(inherited.as_ref(), name)
                })
                .collect();
            for (name, member_type, optional, by_reference) in members {
                let member_path = format!("{path}.{name}");
                let spelled = component_name(&name);
                match object.get(&name) {
                    Some(Value::Null) => {
                        return Err(format!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        let expr = emit(
                            ast,
                            member_type,
                            member_value,
                            &member_path,
                            depth + 1,
                            empties,
                        )?;
                        if by_reference {
                            let designated =
                                zerodds_lang_rules::adapter::ada83::reference_designated(
                                    ast,
                                    member_type,
                                )
                                .unwrap_or(member_type);
                            parts.push(format!(
                                "{spelled} => new {}'({expr})",
                                ada83_type_name(ast, designated)
                            ));
                        } else if optional {
                            parts.push(format!(
                                "{spelled} => ({PRESENT} => True, {VALUE} => {expr})"
                            ));
                        } else {
                            parts.push(format!("{spelled} => {expr}"));
                        }
                    }
                    // Absence is the key left out; the aggregate still
                    // has to state the component.
                    None if by_reference => {
                        parts.push(format!("{spelled} => null"));
                    }
                    None if optional => {
                        parts.push(format!("{spelled} => ({PRESENT} => False)"));
                    }
                    None => {
                        return Err(format!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            if parts.is_empty() {
                return Ok(empty_object(ast, type_, empties));
            }
            Ok(format!("({})", parts.join(", ")))
        }

        TypeKind::Union { discriminator, .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected an object with `_d` and the member"))?;
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| format!("{path}: the discriminator does not resolve"))?;
            let given = object
                .get("_d")
                .ok_or_else(|| format!("{path}._d: no value"))?;
            let disc = static_discriminant(ast, discriminator, given, &format!("{path}._d"))?;
            let mut parts = vec![format!("{DISCRIMINANT} => {disc}")];
            for (name, member_value) in object {
                if name == "_d" {
                    continue;
                }
                let member = ast
                    .cases_of(type_)
                    .iter()
                    .filter_map(|case| ast.case(*case).ok())
                    .filter_map(|case| ast.member(case.member).ok())
                    .find(|member| &member.name == name)
                    .ok_or_else(|| format!("{path}.{name}: not a case of this union"))?;
                let member_type = member
                    .type_
                    .resolved()
                    .ok_or_else(|| format!("{path}.{name}: the case type does not resolve"))?;
                let expr = emit(
                    ast,
                    member_type,
                    member_value,
                    &format!("{path}.{name}"),
                    depth + 1,
                    empties,
                )?;
                parts.push(format!("{} => {expr}", component_name(name)));
            }
            // The discriminant is always at least one association, so
            // an arm with no components — the selected branch's arm is
            // `others => null;` — leaves the aggregate at `(D => …)`,
            // which RM83 4.3.1 already admits: one association is
            // enough. Unlike the empty struct above, no `<>` is
            // needed here at all.
            Ok(format!("({})", parts.join(", ")))
        }

        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the element type does not resolve"))?;
            let items = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected an array"))?;
            let array = format!("{}_Array", ada83_type_name(ast, type_));
            if items.is_empty() {
                return Ok(format!("new {array} (0 .. -1)"));
            }
            let mut parts = Vec::new();
            for (index, item) in items.iter().enumerate() {
                parts.push(format!(
                    "{index} => {}",
                    emit(
                        ast,
                        element,
                        item,
                        &format!("{path}[{index}]"),
                        depth + 1,
                        empties
                    )?
                ));
            }
            Ok(format!("new {array}'({})", parts.join(", ")))
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| format!("{path}: the element type does not resolve"))?;
            let lengths: Vec<i128> = dims
                .iter()
                .map(|dim| ast.eval_integral(*dim).unwrap_or(-1))
                .collect();
            fill_array(ast, element, value, &lengths, path, depth, empties)
        }

        TypeKind::Map { key, value: v, .. } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| format!("{path}: the key type does not resolve"))?;
            let value_type = v
                .resolved()
                .ok_or_else(|| format!("{path}: the value type does not resolve"))?;
            let entries = value
                .as_array()
                .ok_or_else(|| format!("{path}: a map is an array of two-element arrays"))?;
            let array = format!("{}_Array", ada83_type_name(ast, type_));
            if entries.is_empty() {
                return Ok(format!("new {array} (0 .. -1)"));
            }
            let mut parts = Vec::new();
            for (index, pair) in entries.iter().enumerate() {
                let pair = pair
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or_else(|| format!("{path}[{index}]: expected a two-element array"))?;
                let key_text = emit(
                    ast,
                    key_type,
                    &pair[0],
                    &format!("{path}[{index}].key"),
                    depth + 1,
                    empties,
                )?;
                let value_text = emit(
                    ast,
                    value_type,
                    &pair[1],
                    &format!("{path}[{index}].value"),
                    depth + 1,
                    empties,
                )?;
                parts.push(format!(
                    "{index} => (Key => {key_text}, {VALUE} => {value_text})"
                ));
            }
            Ok(format!("new {array}'({})", parts.join(", ")))
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| format!("{path}: the alias does not resolve"))?;
            emit(ast, target, value, path, depth + 1, empties)
        }

        other => Err(format!(
            "{path}: {other:?} has no Ada 83 form this filler can build"
        )),
    }
}

/// One dimension of an IDL array as a named aggregate.
///
/// zerodds-lint: recursion-depth 64 (one level per array dimension,
/// then the element through [`emit`]; bounded by [`MAX_DEPTH`])
fn fill_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lengths: &[i128],
    path: &str,
    depth: usize,
    empties: &mut Vec<EmptyRecord>,
) -> Result<String, String> {
    let Some((length, rest)) = lengths.split_first() else {
        return emit(ast, element, value, path, depth + 1, empties);
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("{path}: expected an array of {length}"))?;
    if i128::try_from(items.len()).map_err(|_| format!("{path}: length overflow"))? != *length {
        return Err(format!(
            "{path}: the array holds {length} and the sample gave {}",
            items.len()
        ));
    }
    let mut parts = Vec::new();
    for (index, item) in items.iter().enumerate() {
        parts.push(format!(
            "{index} => {}",
            fill_array(
                ast,
                element,
                item,
                rest,
                &format!("{path}[{index}]"),
                depth + 1,
                empties,
            )?
        ));
    }
    Ok(format!("({})", parts.join(", ")))
}

/// A discriminant value as the static expression an aggregate demands:
/// the bare number for every discriminator type here, including an
/// enumeration — matching `adapter::ada83::choice_expr` and
/// `discriminant_default`, which write a union's own generated
/// declaration the same way. A named constant would itself be static
/// too (RM83 4.9), but the bare number is what the rest of this
/// column already writes for the identical position, and there is no
/// reason for the sample filler to answer differently — and
/// `True`/`False` for a `boolean`.
fn static_discriminant(
    ast: &Ast,
    discriminator: Id<TypeNode>,
    value: &Value,
    path: &str,
) -> Result<String, String> {
    let resolved = resolve_alias(ast, discriminator);
    let kind = ast
        .type_(resolved)
        .map(|node| node.kind.clone())
        .map_err(|error| format!("{path}: {error}"))?;
    match kind {
        TypeKind::Enumeration { .. } => {
            let name = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected an enumerator name, as a string"))?;
            for id in ast.enumerators_of(resolved) {
                let Ok(enumerator) = ast.enumerator(*id) else {
                    continue;
                };
                if enumerator.name == name {
                    return Ok(format!(
                        "{}",
                        ast.effective_enumerator_value(*id).ok_or_else(|| format!(
                            "{path}: `{name}` has no value the model can state"
                        ))?
                    ));
                }
            }
            Err(format!("{path}: `{name}` names no enumerator"))
        }
        TypeKind::Primitive(PrimitiveKind::Boolean) => {
            let flag = value
                .as_bool()
                .ok_or_else(|| format!("{path}: expected a boolean"))?;
            Ok(if flag { "True" } else { "False" }.to_string())
        }
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            Ok(format!("{}", u32::from(one)))
        }
        // The two's-complement pattern where an `unsigned long long`
        // discriminator's label needs bit 63 — the same fold
        // `primitive_literal` makes for a member of that type and
        // `adapter::ada83::choice_expr` makes for the variant part this
        // aggregate has to agree with. RM83 4.3.1 requires the value
        // given for a discriminant to be static, and a decimal past
        // `Long_Integer'Last` is not.
        _ => integer_of(value, path).map(|number| format!("{}", ada83_pattern(number))),
    }
}

/// One primitive value, as an Ada 83 expression.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, String> {
    Ok(match kind {
        PrimitiveKind::Boolean => {
            if value
                .as_bool()
                .ok_or_else(|| format!("{path}: expected a boolean"))?
            {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32
        | PrimitiveKind::Int64 => {
            format!("{}", integer_of(value, path)?)
        }
        // The two's-complement pattern where the value needs bit 63.
        // `adapter::ada83` holds every integer in `Long_Integer`, which
        // is signed 64 bits, so a value at or above 2**63 has no
        // literal of its own; it is written as the negative number
        // whose bits it is, which `Zd_Wire83.Write_U64` puts on the
        // wire unchanged and `Json_U64` reads back unsigned. A value
        // outside 0 .. 2**64 - 1 is not an `unsigned long long` at all
        // and is refused here rather than silently wrapped.
        PrimitiveKind::UInt64 => {
            /// 2**64, the whole width, as the wrap subtracts it.
            const WIDTH: i128 = 1 << 64;
            let given = integer_of(value, path)?;
            if !(0..=i128::from(u64::MAX)).contains(&given) {
                return Err(format!(
                    "{path}: {given} is outside `unsigned long long`'s range"
                ));
            }
            let pattern = if given > i128::from(i64::MAX) {
                given - WIDTH
            } else {
                given
            };
            format!("{pattern}")
        }
        PrimitiveKind::Char | PrimitiveKind::WChar => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: expected a one-character string"))?;
            let mut chars = text.chars();
            let (Some(one), None) = (chars.next(), chars.next()) else {
                return Err(format!("{path}: expected exactly one character"));
            };
            let checker = if kind == PrimitiveKind::Char {
                "Checked_Char"
            } else {
                "Checked_Wchar"
            };
            format!("Zd_Wire83.{checker} ({})", u32::from(one))
        }
        PrimitiveKind::Float32 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            #[allow(clippy::cast_possible_truncation)]
            let narrowed = f64::from(given as f32);
            ada_float(narrowed, "F32")
        }
        PrimitiveKind::Float64 => {
            let given = value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected a number"))?;
            ada_float(given, "F64")
        }
        PrimitiveKind::Float128 => {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{path}: a binary128 arrives as a decimal string"))?;
            text_expr(text)
        }
    })
}

fn integer_of(value: &Value, path: &str) -> Result<i128, String> {
    match value {
        Value::String(text) => text
            .parse::<i128>()
            .map_err(|_| format!("{path}: `{text}` is not an integer")),
        Value::Number(number) => match number.as_i64() {
            Some(v) => Ok(i128::from(v)),
            None => match number.as_u64() {
                Some(v) => Ok(i128::from(v)),
                None => Err(format!("{path}: not an integer")),
            },
        },
        other => Err(format!("{path}: expected a number, got {other}")),
    }
}

/// An Ada floating-point spelling.
fn ada_float(number: f64, half: &str) -> String {
    if number.is_nan() {
        return format!("Zd_Wire83.{half}_Nan");
    }
    if number.is_infinite() {
        return if number > 0.0 {
            format!("Zd_Wire83.{half}_Inf")
        } else {
            format!("-Zd_Wire83.{half}_Inf")
        };
    }
    let written = format!("{number:?}");
    if written.contains('.') {
        return written;
    }
    match written.split_once(['e', 'E']) {
        Some((head, tail)) => format!("{head}.0e{tail}"),
        None => format!("{written}.0"),
    }
}

/// A `Zd_Wire83.Text_Of` call for graphic ASCII, and the UTF-8 octets
/// through `Zd_Wire83.Text_From_Utf8` for everything else.
pub fn text_expr(text: &str) -> String {
    if text.chars().all(|ch| ('\u{20}'..='\u{7e}').contains(&ch)) {
        return format!("Zd_Wire83.Text_Of ({})", ada_string_literal(text));
    }
    let mut octets = Vec::new();
    for byte in text.bytes() {
        octets.push(format!("16#{byte:02X}#"));
    }
    format!("Zd_Wire83.Text_From_Utf8 (({}))", octets.join(", "))
}

/// An Ada string literal: the quotation mark doubles (RM83 2.6).
pub fn ada_string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_DEPTH`)
fn resolve_alias(ast: &Ast, id: Id<TypeNode>) -> Id<TypeNode> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        match ast.type_(current).map(|node| node.kind.clone()) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(target) => current = target,
                None => return current,
            },
            _ => return current,
        }
    }
    current
}
