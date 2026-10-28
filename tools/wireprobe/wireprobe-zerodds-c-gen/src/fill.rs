// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The statements that put one JSON sample into the generated C.
//!
//! The counterpart of `crates/wire-probe/src/fill.rs`, which does the
//! same for the vendors' generated code. This one fills ZeroDDS's own C
//! shapes, so it is the only part of the C column that knows both the
//! JSON dialect and the memory layout.
//!
//! What it must not do is decide anything about the wire. It places
//! values; the codec frames them and `runtime/c` writes them. A filler
//! that computed a bit pattern or a length would move a rule out of the
//! runtime, where the whole point is to have a second implementation of
//! it.
//!
//! The JSON dialect is the one `crates/wire-probe/src/lib.rs` documents,
//! read the same way: a 64-bit integer is a string, an absent
//! `@optional` member is a key left out rather than a `null`, and a map
//! is an array of two-element arrays.

use serde_json::Value;
use std::fmt::Write as _;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_rules::adapter::c::named_member_held_behind_pointer;
use zerodds_lang_rules::adapter::c::{c_map_entry_name, c_type_name, module_prefix};
use zerodds_wireprobe_gen::{Refusal, gap, no_form};

/// Name, type, optional flag, path from the sample and owner type of a member.
type FilledMember = (String, Id<TypeNode>, bool, String, Id<TypeNode>);

/// The C the filler produced.
#[derive(Debug, Default)]
pub struct Fill {
    /// File-scope declarations the statements refer to — the backing
    /// arrays a sequence or a map points at. They are `static` and
    /// outlive the fill, because the sample holds pointers into them
    /// and nothing in the write path copies.
    declarations: String,
    /// The assignments.
    statements: String,
    /// A counter, so two collections in one sample get distinct names.
    next: usize,
}

impl Fill {
    /// The declarations, for the file scope.
    #[must_use]
    pub fn declarations(&self) -> &str {
        &self.declarations
    }

    /// The statements, for the body of the fill function.
    #[must_use]
    pub fn statements(&self) -> &str {
        &self.statements
    }

    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// Emit the statements that fill `sample` with `values`.
///
/// # Errors
/// A message naming the path in the value, in the same shape the Rust
/// probe reports: a sample the two probes read differently is a
/// difference in the readers, not in the serialisers.
pub fn fill(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<Fill, Refusal> {
    let mut out = Fill::default();
    emit(ast, type_, values, "zd_sample", &mut out, "sample")?;
    Ok(out)
}

/// Fill a member the C header holds behind a pointer, and hand back the
/// place the value goes.
///
/// `adapter/c.rs` declares a member whose type reaches its own aggregate
/// back as `T *` — a C structure that held such a member as storage
/// would contain itself. So the fill needs somewhere for the pointer to
/// point: one `static` of the member's type per occurrence, in file
/// scope beside the backing arrays a sequence gets, and for the same
/// reason — the sample holds the pointer and nothing in the write path
/// copies.
fn indirect_slot(ast: &Ast, member_type: Id<TypeNode>, lvalue: &str, out: &mut Fill) -> String {
    let slot = out.temporary("ext");
    let _ = writeln!(
        out.declarations,
        "static {} {slot};",
        c_type_name(ast, member_type)
    );
    let _ = writeln!(out.statements, "    {lvalue} = &{slot};");
    slot
}

/// zerodds-lint: recursion-depth 64 (alias chains; bounded by
/// `zerodds_wireprobe_gen::MAX_ALIAS_CHAIN`, which
/// `zerodds_wireprobe_gen::bounded_alias_chains` enforces over every
/// type in the model once, in `Session::open`, before any generation
/// runs).
///
/// Every other arm descends into a sub-value of the sample — a member,
/// a case, a collection item, a map half — and `serde_json::Value` is
/// an owned tree, so those arms cannot cycle whatever the type graph
/// does. `TypeKind::Alias` is the one arm that hands the same value on,
/// which is why the alias chain is the thing that has to be bounded,
/// and the only thing.
#[allow(clippy::too_many_lines)]
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
) -> Result<(), Refusal> {
    let node = ast.type_(type_).map_err(|error| gap!("{path}: {error}"))?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            let literal = primitive_literal(*kind, value, path)?;
            let _ = writeln!(out.statements, "    {lvalue} = {literal};");
            Ok(())
        }

        TypeKind::String { width, .. } => {
            let text = value
                .as_str()
                .ok_or_else(|| gap!("{path}: expected a string"))?;
            let _ = width;
            let _ = writeln!(out.statements, "    {lvalue} = {};", c_string(text));
            Ok(())
        }

        TypeKind::Fixed { .. } => {
            let text = value.as_str().ok_or_else(|| {
                format!("{path}: expected a decimal in a string — a JSON number is a double")
            })?;
            let _ = writeln!(out.statements, "    {lvalue} = {};", c_string(text));
            Ok(())
        }

        TypeKind::Enumeration { .. } => {
            let name = value
                .as_str()
                .ok_or_else(|| gap!("{path}: expected an enumerator name, as a string"))?;
            let mut declared = Vec::new();
            let mut found = false;
            for id in ast.enumerators_of(type_) {
                let Ok(enumerator) = ast.enumerator(*id) else {
                    continue;
                };
                declared.push(enumerator.name.clone());
                if enumerator.name == name {
                    found = true;
                }
            }
            if !found {
                return Err(gap!("{path}: `{name}` is not one of {declared:?}"));
            }
            // The enumerator's C name and not its IDL name. `dump.rs`
            // escapes here and this did not, so an enumerator called
            // `auto` produced `zd_Colour_auto` against the declaration's
            // `zd_Colour_auto_` — the C column had no probe at all for
            // the six `identifier-target-keyword-*` cases, in every
            // configuration, and only the full tier ever compiled them.
            let _ = writeln!(
                out.statements,
                "    {lvalue} = {}{};",
                module_prefix(ast, type_),
                zerodds_lang_cst::c::escape_keyword(name)
            );
            Ok(())
        }

        TypeKind::Bitmask { .. } => {
            let names = value
                .as_array()
                .ok_or_else(|| gap!("{path}: expected an array of bit names"))?;
            let declared: Vec<String> = ast
                .bits_of(type_)
                .iter()
                .filter_map(|id| ast.bit_value(*id).ok())
                .map(|bit| bit.name.clone())
                .collect();
            let prefix = module_prefix(ast, type_);
            let mut flags = Vec::new();
            for (index, entry) in names.iter().enumerate() {
                let name = entry
                    .as_str()
                    .ok_or_else(|| gap!("{path}[{index}]: expected a bit name"))?;
                if !declared.iter().any(|bit| bit == name) {
                    return Err(gap!("{path}[{index}]: `{name}` is not one of {declared:?}"));
                }
                flags.push(format!(
                    "{prefix}{}",
                    zerodds_lang_cst::c::escape_keyword(name)
                ));
            }
            // No flags set is the integer zero, not an empty expression.
            let expression = if flags.is_empty() {
                "0".to_string()
            } else {
                flags.join(" | ")
            };
            let _ = writeln!(out.statements, "    {lvalue} = {expression};");
            Ok(())
        }

        TypeKind::Bitset { .. } => {
            // `all_bitfields_of` and not the node's own `fields`: a
            // `bitset` that extends another carries the base's
            // bitfields, and `encode_c.rs` writes them. Taking only the
            // derived type's own list refused `bitset-inheritance`'s
            // sample — `sample.value.low: not a bitfield; ["high"]` —
            // for a field the codec beside it was about to write, so
            // the C column had no probe at all for that case in any
            // configuration while the other two produced `8dab`.
            let fields = &ast.all_bitfields_of(type_);
            let object = value
                .as_object()
                .ok_or_else(|| gap!("{path}: expected an object keyed by bitfield name"))?;
            let declared: Vec<String> = fields
                .iter()
                .filter_map(|id| ast.bitfield(*id).ok())
                .filter_map(|field| field.name.clone())
                .collect();
            for name in object.keys() {
                if !declared.contains(name) {
                    return Err(gap!("{path}.{name}: not a bitfield; {declared:?}"));
                }
            }
            for id in fields {
                let Ok(field) = ast.bitfield(*id) else {
                    continue;
                };
                let Some(name) = field.name.as_ref() else {
                    continue;
                };
                let member_value = object
                    .get(name)
                    .ok_or_else(|| gap!("{path}.{name}: no value"))?;
                let raw = integer(member_value, &format!("{path}.{name}"))?;
                let _ = writeln!(
                    out.statements,
                    "    {lvalue}.{} = {raw}u;",
                    zerodds_lang_cst::c::escape_keyword(name)
                );
            }
            Ok(())
        }

        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or_else(|| gap!("{path}: the typedef target is unresolved"))?;
            emit(ast, target, value, lvalue, out, path)
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let object = value
                .as_object()
                .ok_or_else(|| gap!("{path}: expected an object"))?;
            // `all_members_of`, so an inherited member is filled and not
            // left at whatever the zero-initialiser put there. The plan
            // writes it first, and a sample missing it is the right
            // length with the wrong bytes in it.
            //
            // A base is the member `parent` of the derived struct (XTypes
            // 1.3 §7.5.1.4.1.3), so an inherited member is `parent.m`.
            let members: Vec<FilledMember> = zerodds_lang_rules::adapter::c::c_members(ast, type_)
                .into_iter()
                .filter_map(|c_member| {
                    let member = ast.member(c_member.handle).ok()?;
                    Some((
                        member.name.clone(),
                        member.type_.resolved()?,
                        member.facets.is_optional(),
                        c_member.path,
                        c_member.owner,
                    ))
                })
                .collect();
            let declared: Vec<&str> = members
                .iter()
                .map(|(name, _, _, _, _)| name.as_str())
                .collect();
            for name in object.keys() {
                if !declared.contains(&name.as_str()) {
                    return Err(gap!("{path}.{name}: not a member; {declared:?}"));
                }
            }
            for (name, member_type, optional, via, owner) in members {
                let member_path = format!("{path}.{name}");
                match object.get(&name) {
                    Some(Value::Null) => {
                        return Err(gap!(
                            "{member_path}: null — an absent @optional is a key left out"
                        ));
                    }
                    Some(member_value) => {
                        // The member's C name. `adapter/c.rs` declares
                        // it escaped and both halves have to agree, or a
                        // member named `static` produces a statement no
                        // C compiler will parse.
                        let field = zerodds_lang_cst::c::escape_keyword(&name);
                        // A member the header holds behind a pointer is
                        // filled through the slot the pointer is made to
                        // point at, and not through the member.
                        let place =
                            if named_member_held_behind_pointer(ast, owner, &name, member_type)
                                .map_err(|message| gap!("{member_path}: {message}"))?
                            {
                                indirect_slot(
                                    ast,
                                    member_type,
                                    &format!("{lvalue}.{via}{field}"),
                                    out,
                                )
                            } else {
                                format!("{lvalue}.{via}{field}")
                            };
                        emit(ast, member_type, member_value, &place, out, &member_path)?;
                    }
                    None if optional => {
                        // Nothing: the zero-initialised pointer is NULL,
                        // which is how XTypes 1.3 §7.5.1.2.4.1 says an
                        // optional member is absent, and the codec writes
                        // nothing.
                    }
                    None => {
                        return Err(gap!(
                            "{member_path}: no value, and the member is not optional"
                        ));
                    }
                }
            }
            Ok(())
        }

        TypeKind::Union { discriminator, .. } => {
            let object = value.as_object().ok_or_else(|| {
                format!("{path}: expected an object with `_d` and the selected member")
            })?;
            let discriminator_value = object
                .get("_d")
                .ok_or_else(|| gap!("{path}._d: no value"))?;
            let discriminator = discriminator
                .resolved()
                .ok_or_else(|| gap!("{path}: the discriminator is unresolved"))?;
            emit(
                ast,
                discriminator,
                discriminator_value,
                &format!("{lvalue}._d"),
                out,
                &format!("{path}._d"),
            )?;
            let cases: Vec<(String, Id<TypeNode>)> = ast
                .cases_of(type_)
                .iter()
                .filter_map(|id| ast.case(*id).ok())
                .filter_map(|case| ast.member(case.member).ok())
                .filter_map(|member| Some((member.name.clone(), member.type_.resolved()?)))
                .collect();
            for (name, member_value) in object {
                if name == "_d" {
                    continue;
                }
                let member_type = cases
                    .iter()
                    .find(|(case_name, _)| case_name == name)
                    .map(|(_, type_)| *type_)
                    .ok_or_else(|| {
                        let declared: Vec<&str> =
                            cases.iter().map(|(name, _)| name.as_str()).collect();
                        format!("{path}.{name}: not a case; {declared:?}")
                    })?;
                let branch = format!("{lvalue}._u.{}", zerodds_lang_cst::c::escape_keyword(name));
                // A branch the header holds behind a pointer, exactly as
                // a structure member is held.
                let place = if named_member_held_behind_pointer(ast, type_, name, member_type)
                    .map_err(|message| gap!("{path}.{name}: {message}"))?
                {
                    indirect_slot(ast, member_type, &branch, out)
                } else {
                    branch
                };
                emit(
                    ast,
                    member_type,
                    member_value,
                    &place,
                    out,
                    &format!("{path}.{name}"),
                )?;
            }
            Ok(())
        }

        TypeKind::Array { element, dims } => {
            let element = element
                .resolved()
                .ok_or_else(|| gap!("{path}: the array element is unresolved"))?;
            let counts: Result<Vec<usize>, Refusal> = dims
                .iter()
                .map(|dim| {
                    ast.eval_integral(*dim)
                        .ok()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| gap!("{path}: an array length does not evaluate"))
                })
                .collect();
            emit_array(ast, element, value, lvalue, out, path, &counts?)
        }

        TypeKind::Sequence { element, bound } => {
            let element = element
                .resolved()
                .ok_or_else(|| gap!("{path}: the sequence element is unresolved"))?;
            let items = value
                .as_array()
                .ok_or_else(|| gap!("{path}: expected an array"))?;
            if let Some(limit) = bound.expr().and_then(|expr| ast.eval_integral(expr).ok()) {
                if items.len() as i128 > limit {
                    return Err(gap!("{path}: at most {limit} elements"));
                }
            }
            let buffer = out.temporary("seq");
            let element_type = c_type_name(ast, element);
            if items.is_empty() {
                let _ = writeln!(
                    out.statements,
                    "    {lvalue}._maximum = 0; {lvalue}._length = 0; {lvalue}._buffer = NULL; {lvalue}._release = 0;"
                );
                return Ok(());
            }
            let _ = writeln!(
                out.declarations,
                "static {element_type} {buffer}[{}];",
                items.len()
            );
            for (index, item) in items.iter().enumerate() {
                emit(
                    ast,
                    element,
                    item,
                    &format!("{buffer}[{index}]"),
                    out,
                    &format!("{path}[{index}]"),
                )?;
            }
            let _ = writeln!(
                out.statements,
                "    {lvalue}._maximum = {0}; {lvalue}._length = {0}; {lvalue}._buffer = {buffer}; {lvalue}._release = 0;",
                items.len()
            );
            Ok(())
        }

        TypeKind::Map {
            key,
            value: v,
            bound,
        } => {
            let key_type = key
                .resolved()
                .ok_or_else(|| gap!("{path}: the map key type is unresolved"))?;
            let value_type = v
                .resolved()
                .ok_or_else(|| gap!("{path}: the map value type is unresolved"))?;
            // An array of pairs and not an object: a JSON object keys on
            // a string, and `map<long, float>` does not.
            let entries = value
                .as_array()
                .ok_or_else(|| gap!("{path}: expected an array of [key, value] pairs"))?;
            if let Some(limit) = bound.expr().and_then(|expr| ast.eval_integral(expr).ok()) {
                if entries.len() as i128 > limit {
                    return Err(gap!("{path}: at most {limit} entries"));
                }
            }
            let buffer = out.temporary("map");
            if entries.is_empty() {
                let _ = writeln!(
                    out.statements,
                    "    {lvalue}._maximum = 0; {lvalue}._length = 0; {lvalue}._buffer = NULL; {lvalue}._release = 0;"
                );
                return Ok(());
            }
            let _ = writeln!(
                out.declarations,
                "static {} {buffer}[{}];",
                c_map_entry_name(ast, type_),
                entries.len()
            );
            for (index, entry) in entries.iter().enumerate() {
                let entry_path = format!("{path}[{index}]");
                let pair = entry
                    .as_array()
                    .ok_or_else(|| gap!("{entry_path}: expected a two-element array"))?;
                let [entry_key, entry_value] = pair.as_slice() else {
                    return Err(gap!(
                        "{entry_path}: expected a two-element array, `[key, value]`"
                    ));
                };
                emit(
                    ast,
                    key_type,
                    entry_key,
                    &format!("{buffer}[{index}].key"),
                    out,
                    &format!("{entry_path}.key"),
                )?;
                emit(
                    ast,
                    value_type,
                    entry_value,
                    &format!("{buffer}[{index}].value"),
                    out,
                    &format!("{entry_path}.value"),
                )?;
            }
            let _ = writeln!(
                out.statements,
                "    {lvalue}._maximum = {0}; {lvalue}._length = {0}; {lvalue}._buffer = {buffer}; {lvalue}._release = 0;",
                entries.len()
            );
            Ok(())
        }

        other => Err(gap!("{path}: {} has no C shape to fill", other.as_str())),
    }
}

/// zerodds-lint: recursion-depth = the array's declared dimension count
/// (`counts` is one shorter on every call and the element then goes
/// through [`emit`], whose bound the note there states)
fn emit_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
    counts: &[usize],
) -> Result<(), Refusal> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(ast, element, value, lvalue, out, path);
    };
    let items = value
        .as_array()
        .ok_or_else(|| gap!("{path}: expected an array"))?;
    if items.len() != count {
        return Err(gap!(
            "{path}: expected exactly {count} elements, found {}",
            items.len()
        ));
    }
    for (index, item) in items.iter().enumerate() {
        emit_array(
            ast,
            element,
            item,
            &format!("{lvalue}[{index}]"),
            out,
            &format!("{path}[{index}]"),
            rest,
        )?;
    }
    Ok(())
}

/// The C literal for one primitive.
///
/// Ranges are checked here rather than left to C's conversions: an
/// out-of-range value assigned to a narrower type is truncated silently,
/// which would put a different sample on the wire than the one the file
/// describes. The Rust probe refuses the same values.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, Refusal> {
    let ranged = |low: i128, high: i128, suffix: &str| -> Result<String, Refusal> {
        let raw = integer(value, path)?;
        if raw < low || raw > high {
            return Err(gap!("{path}: {raw} is outside {low}..={high}"));
        }
        Ok(format!("{raw}{suffix}"))
    };
    Ok(match kind {
        PrimitiveKind::Boolean => {
            if value
                .as_bool()
                .ok_or_else(|| gap!("{path}: expected true or false"))?
            {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => ranged(0, 0xff, "u")?,
        PrimitiveKind::Int8 => ranged(-128, 127, "")?,
        PrimitiveKind::Int16 => ranged(-32768, 32767, "")?,
        PrimitiveKind::UInt16 => ranged(0, 0xffff, "u")?,
        PrimitiveKind::Int32 => {
            let raw = integer(value, path)?;
            if raw < i128::from(i32::MIN) || raw > i128::from(i32::MAX) {
                return Err(gap!("{path}: {raw} is outside a long"));
            }
            // `INT32_MIN` has no positive counterpart, so the literal
            // `-2147483648` is `-(2147483648)`, which is not an `int`.
            // The cast makes the type the one the member has.
            format!("(int32_t){raw}")
        }
        PrimitiveKind::UInt32 => ranged(0, 0xffff_ffff, "u")?,
        PrimitiveKind::Int64 => {
            let raw = integer(value, path)?;
            if raw < i128::from(i64::MIN) || raw > i128::from(i64::MAX) {
                return Err(gap!("{path}: {raw} is outside a long long"));
            }
            // `INT64_MIN` has no positive counterpart: inside
            // `INT64_C(...)` the digits 9223372036854775808 are read as
            // an unsigned constant before the minus applies, which
            // -Werror rejects. Same shape as the `int32_t` cast above.
            if raw == i128::from(i64::MIN) {
                "INT64_MIN".to_string()
            } else {
                format!("INT64_C({raw})")
            }
        }
        PrimitiveKind::UInt64 => {
            let raw = integer(value, path)?;
            if raw < 0 || raw > i128::from(u64::MAX) {
                return Err(gap!("{path}: {raw} is outside an unsigned long long"));
            }
            format!("UINT64_C({raw})")
        }
        // A code point the C storage cannot hold is refused here and not
        // masked. The runtime refuses one too, but by then the fill has
        // already narrowed it to the member's width — `char` is eight
        // bits and `wchar` sixteen — so the check that matters is this
        // one. Masking wrote `0x61` for U+0161 and reported success:
        // a different character on the wire with nothing said.
        // XTypes 1.3 §7.2.2.2.1.2.2 and §7.2.2.2.1.2.5.
        // Both of these are `no-form` and not a gap in this tree: the
        // carrier `adapter/c.rs` chose is a C `char` and a `uint16_t`,
        // and neither holds the value. That is a statement about what C
        // has, which is what exit 3 records. It is *not* a statement
        // about the wire — TypeScript carries U+0161 in a `string` and
        // its runtime refuses it at run time out of §7.2.2.2.1.2.2,
        // which is a second reading of the clause and the point of a
        // second column; C++ has C's two carriers and answers as C
        // does. This generator answered **1** for both, the code the
        // verdict table gives the command line.
        PrimitiveKind::Char => {
            let ch = u32::from(character(value, path)?);
            if ch > 0xff {
                return Err(no_form!(
                    "{path}: expected a character below U+0100, found U+{ch:04X}"
                ));
            }
            format!("(char)0x{ch:02x}")
        }
        PrimitiveKind::WChar => {
            let ch = u32::from(character(value, path)?);
            if ch > 0xffff {
                return Err(no_form!(
                    "{path}: expected a character in the Basic Multilingual Plane, \
                     found U+{ch:04X}"
                ));
            }
            format!("(uint16_t)0x{ch:04x}")
        }
        // Seventeen significant digits, which is what an IEEE-754 double
        // round-trips through. A shorter literal would put different
        // bytes on the wire than the Rust probe's.
        PrimitiveKind::Float32 => {
            let number = float(value, path)?;
            format!("(float){number:.17e}")
        }
        PrimitiveKind::Float64 => {
            let number = float(value, path)?;
            format!("{number:.17e}")
        }
        // The exact bit pattern, not a decimal literal. A `long double`
        // literal in generated C would be read by whatever `strtold` the
        // target has, and on x86-64 that lands in an x87 80-bit register
        // — the value would be narrowed before the runtime ever saw it,
        // which is the defect this column exists to not have.
        PrimitiveKind::Float128 => {
            let wide = float128(value, path)?;
            let bits = wide.to_bits();
            format!(
                "zdw_f128_from_parts(0x{:016x}ULL, 0x{:016x}ULL)",
                (bits >> 64) as u64,
                bits as u64
            )
        }
    })
}

/// A `long double` from the value file.
///
/// A decimal string, like a 64-bit integer, or a JSON number for the
/// values a double can hold exactly. The same contract
/// `crates/wire-probe/src/contract.rs` states, because a value file that
/// meant one thing to the reflective probe and another to this one would
/// make the two columns incomparable.
fn float128(value: &Value, path: &str) -> Result<zerodds_float128::Float128, Refusal> {
    if let Some(text) = value.as_str() {
        return text
            .parse::<zerodds_float128::Float128>()
            .map_err(|error| gap!("{path}: `{text}` is not a long double — {error}"));
    }
    Ok(zerodds_float128::Float128::from_f64(float(value, path)?))
}

/// An integer, from a JSON number or from a string.
///
/// A string is accepted for every width and required past 2^53: a JSON
/// number is a double, and `9007199254740993` does not survive one.
fn integer(value: &Value, path: &str) -> Result<i128, Refusal> {
    if let Some(text) = value.as_str() {
        return text
            .parse::<i128>()
            .map_err(|_| gap!("{path}: `{text}` is not an integer"));
    }
    value
        .as_i64()
        .map(i128::from)
        .or_else(|| value.as_u64().map(i128::from))
        .ok_or_else(|| gap!("{path}: expected an integer"))
}

fn float(value: &Value, path: &str) -> Result<f64, Refusal> {
    value
        .as_f64()
        .ok_or_else(|| gap!("{path}: expected a number"))
}

fn character(value: &Value, path: &str) -> Result<char, Refusal> {
    let text = value
        .as_str()
        .ok_or_else(|| gap!("{path}: expected a one-character string"))?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(ch),
        _ => Err(gap!("{path}: expected exactly one character")),
    }
}

/// A C string literal holding the UTF-8 bytes of `text`.
///
/// Everything outside printable ASCII goes out as a three-digit octal
/// escape. Not `\x`: a hexadecimal escape in C has no length limit, so
/// `"\xc3a"` is one character and not two, and a multi-byte character
/// followed by a hex digit would silently become something else.
fn c_string(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.as_bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(char::from(*byte)),
            other => {
                let _ = write!(out, "\\{other:03o}");
            }
        }
    }
    out.push('"');
    out
}
