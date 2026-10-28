// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The value-printing code the Fast DDS probe needs to answer with.
//!
//! `dump.rs` writes C, for the two generators whose output is a plain
//! `struct`. Fast DDS generates a C++ class, so every member is read
//! through its getter and the expressions have a shape of their own —
//! the same split `fill.rs` and `fill_cpp.rs` have in the other
//! direction.
//!
//! ```text
//! sample.x                       // Cyclone, RTI
//! sample.x()                     // Fast DDS
//! sample.location.x              // Cyclone, RTI
//! sample.location().x()          // Fast DDS
//! ```
//!
//! ## Why this is not a self-oracle
//!
//! The statements read the *vendor's* object, which the *vendor's*
//! deserialiser filled. A printer that read the wrong member shows up as
//! a disagreement between the implementations, never as a false
//! agreement.
//!
//! ## What the C++ binding has that the C one does not
//!
//! Three constructs `dump.rs` refuses are printable here, because the
//! C++ mapping does define them: a bitset is a struct of public C++
//! bit-fields, a `map` is a `std::map`, and a `wstring` is a
//! `std::wstring` whose element is `wchar_t`. Only `fixed<P, S>` stays
//! refused, and there because `fastddsgen` rejects it in the IDL before
//! any accessor exists to read.

use crate::contract;
use crate::encode::ProbeError;
use crate::fill_cpp::cpp_name;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// Where one value is read from, in the two shapes Fast DDS's generated
/// code offers.
enum Place {
    /// A member of an aggregate, read through its getter.
    Accessor {
        /// The expression naming the aggregate.
        object: String,
        /// The member's name, which is also the accessor's name.
        name: String,
    },
    /// An element of a `std::array`, a `std::vector` or a `std::map`,
    /// which is a plain lvalue.
    Element(String),
}

impl Place {
    fn read(&self) -> String {
        match self {
            Self::Accessor { object, name } => format!("{object}.{name}()"),
            Self::Element(expression) => expression.clone(),
        }
    }
}

#[derive(Debug, Default)]
struct Dump {
    body: String,
    depth: usize,
    next: usize,
    /// The aggregates on a cycle, each of which gets a function of its
    /// own — `zd_rec::dump_<position>` — instead of an inline
    /// expansion, which for a cycle has no end. See
    /// [`crate::recursion::recursive_aggregates`].
    rec: Vec<Id<TypeNode>>,
}

impl Dump {
    fn line(&mut self, text: &str) {
        for _ in 0..self.depth {
            self.body.push_str("    ");
        }
        self.body.push_str(text);
        self.body.push('\n');
    }

    fn open(&mut self, text: &str) {
        self.line(text);
        self.depth += 1;
    }

    fn close(&mut self, text: &str) {
        self.depth = self.depth.saturating_sub(1);
        self.line(text);
    }

    fn raw(&mut self, literal: &str) {
        let escaped = literal.replace('\\', "\\\\").replace('"', "\\\"");
        self.line(&format!("zdj_raw(\"{escaped}\");"));
    }

    /// A distinct name, so two nested loops do not shadow each other in
    /// a way that reads as working and iterates the wrong collection.
    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// Emit the C++ statements that print `sample` as canonical JSON.
///
/// # Errors
/// [`ProbeError`], naming the path in the type.
pub fn dump_fastdds(ast: &Ast, type_: Id<TypeNode>) -> Result<String, ProbeError> {
    // Before `recursive_aggregates`; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Dump {
        rec: crate::recursion::recursive_aggregates(ast, type_),
        ..Dump::default()
    };
    if !out.rec.is_empty() {
        // A local struct, because `dump.inc` lands inside `main` and a
        // plain function cannot be declared there; static members can,
        // and they see each other regardless of order, which is what a
        // mutual cycle needs — the same shape `corba_cpp.rs` emits.
        out.open("struct zd_rec {");
        for position in 0..out.rec.len() {
            let id = out.rec[position];
            let name = crate::fill_cpp::cpp_name(ast, id);
            out.open(&format!(
                "static void dump_{position}(const {name} &zdv) {{"
            ));
            emit_rec_body(ast, id, &mut out, &format!("<{name}>"))?;
            out.close("}");
        }
        out.close("};");
    }
    emit(
        ast,
        type_,
        &Place::Element(String::from("sample")),
        &mut out,
        "",
    )?;
    out.line("zdj_raw(\"\\n\");");
    Ok(out.body)
}

/// The body of one cycle function: the aggregate's own expansion, with
/// the value behind the reference parameter.
fn emit_rec_body(
    ast: &Ast,
    type_: Id<TypeNode>,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            emit_aggregate(ast, type_, "zdv", out, path)
        }
        TypeKind::Union { .. } => emit_union(ast, type_, "zdv", out, path),
        _ => Err(ProbeError::NoLayout {
            detail: format!("{path}: a type on a cycle is not an aggregate"),
        }),
    }
}

/// zerodds-lint: recursion-depth 64 — the type graph's own nesting. There
/// is no sample here to end the walk, so the cycle is cut instead: an
/// aggregate named by `crate::recursion::recursive_aggregates` becomes a
/// call to the function emitted for it and is a leaf of this walk, which
/// therefore descends an acyclic graph. The typedef arm is finite because
/// `contract::reject_alias_cycle` at the entry point has refused a chain
/// that returns to itself. The corpus's deepest nesting is four.
#[allow(clippy::too_many_lines)]
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    place: &Place,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let here = place.read();
    match &node.kind {
        // As in `fill_cpp.rs`: printing it as a `double` would report
        // agreement for a value neither side carried.
        TypeKind::Primitive(PrimitiveKind::Float128) => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a long double, whose Fast DDS bytes differ between x86-64 and aarch64",
        }),
        TypeKind::Primitive(kind) => {
            out.line(&primitive_call(*kind, &here));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => {
            out.line(&format!("zdj_string({here}.c_str());"));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            // §7.5.2 fixes `wstring` as `std::wstring`, whose element is
            // a code point and not a UTF-16 code unit. The helper
            // re-splits a code point above the BMP into the pair the
            // canonical form expects, which is what the reader on the
            // other side will have joined.
            out.line(&format!("zdj_wstring_cpp({here});"));
            Ok(())
        }
        TypeKind::Enumeration { .. } => {
            emit_enumeration(ast, type_, &here, out);
            Ok(())
        }
        TypeKind::Bitmask { .. } => {
            emit_bitmask(ast, type_, &here, out);
            Ok(())
        }
        TypeKind::Bitset { .. } => {
            out.open("{");
            out.line("int zd_first = 1;");
            out.raw("{");
            // `contract::bitfield_names` already drops the anonymous
            // padding fields, which carry no value. `all_bitfields_of`
            // adds the base's, because `fastddsgen` flattens an
            // inherited bitset into one struct and an inherited
            // bitfield is addressed like a declared one.
            for name in contract::bitfield_names(ast, &ast.all_bitfields_of(type_)) {
                out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
                out.line(&format!("zdj_u64((unsigned long long)({here}.{name}));"));
            }
            out.raw("}");
            out.close("}");
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            emit(ast, target, place, out, path)
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } | TypeKind::Union { .. } => {
            // A type on a cycle is a call, never an inline expansion —
            // the expansion of a cycle does not end.
            if let Some(position) = out.rec.iter().position(|id| *id == type_) {
                out.line(&format!("zd_rec::dump_{position}({here});"));
                return Ok(());
            }
            if matches!(&node.kind, TypeKind::Union { .. }) {
                emit_union(ast, type_, &here, out, path)
            } else {
                emit_aggregate(ast, type_, &here, out, path)
            }
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            emit_array(ast, element, &here, out, path, &counts)
        }
        TypeKind::Sequence { element, .. } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let index = out.temporary("i");
            let count = out.temporary("n");
            out.open("{");
            out.line(&format!("const size_t {count} = {here}.size();"));
            out.raw("[");
            out.open(&format!(
                "for (size_t {index} = 0; {index} < {count}; {index}++) {{"
            ));
            out.line(&format!("if ({index} > 0) {{ zdj_raw(\",\"); }}"));
            emit(
                ast,
                element,
                &Place::Element(format!("{here}[{index}]")),
                out,
                &format!("{path}[]"),
            )?;
            out.close("}");
            out.raw("]");
            out.close("}");
            Ok(())
        }
        TypeKind::Map {
            key, value: entry, ..
        } => {
            let key_type = key.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map key type is unresolved"),
            })?;
            let value_type = entry.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the map value type is unresolved"),
            })?;
            let first = out.temporary("first");
            let pair = out.temporary("entry");
            // An array of pairs and not an object, the same shape every
            // other probe reads and writes: a JSON object keys on a
            // string and `map<long, float>` does not.
            out.open("{");
            out.line(&format!("int {first} = 1;"));
            out.raw("[");
            out.open(&format!("for (const auto &{pair} : {here}) {{"));
            out.line(&format!("if (!{first}) {{ zdj_raw(\",\"); }} {first} = 0;"));
            out.raw("[");
            emit(
                ast,
                key_type,
                &Place::Element(format!("{pair}.first")),
                out,
                &format!("{path}[].key"),
            )?;
            out.raw(",");
            emit(
                ast,
                value_type,
                &Place::Element(format!("{pair}.second")),
                out,
                &format!("{path}[].value"),
            )?;
            out.raw("]");
            out.close("}");
            out.raw("]");
            out.close("}");
            Ok(())
        }
        TypeKind::Fixed { .. } => Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a fixed, which fastddsgen rejects in the IDL before any \
                   accessor could be read from it",
        }),
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// A structure or a valuetype, as a JSON object in declaration order.
///
/// `fastddsgen` maps inheritance to C++ inheritance, so a base's
/// accessors are reachable through the derived object under their own
/// names and the path stays flat — the same walk `fill_cpp.rs` makes.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, below
/// `emit`, which carries the argument: every member goes back through it,
/// and an aggregate on a cycle is a call to `zd_rec::dump_<n>` there
/// rather than an expansion here.
fn emit_aggregate(
    ast: &Ast,
    type_: Id<TypeNode>,
    here: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    out.open("{");
    out.line("int zd_first = 1;");
    out.raw("{");
    for member in contract::members(ast, type_) {
        let member_type = member.type_;
        let name = member.name.clone();
        let member_path = contract::join(path, &name);
        // `fastddsgen` renders `@external` as
        // `eprosima::fastcdr::external<T>`, which dereferences to the
        // value. Always present, unlike an `@optional`; the getter is
        // the holder and not the value, so reading it as one would print
        // the holder's address.
        if is_external(ast, type_, &name) && !member.optional {
            out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
            emit(
                ast,
                member_type,
                &Place::Element(format!("(*{here}.{name}())")),
                out,
                &member_path,
            )?;
            continue;
        }
        // `fastddsgen` renders `@optional` as `eprosima::fastcdr::
        // optional<T>`. An absent one leaves the key out; writing `null`
        // would be a value the canonical form refuses.
        if member.optional {
            // Behind `@optional @external` the engaged value is the
            // `external` holder, and the sample sits one dereference
            // further in — the mirror of the fill's stacked-wrapper
            // arm.
            let engaged = if is_external(ast, type_, &name) {
                format!("(*{here}.{name}().value())")
            } else {
                format!("{here}.{name}().value()")
            };
            out.open(&format!("if ({here}.{name}().has_value()) {{"));
            out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
            emit(
                ast,
                member_type,
                &Place::Element(engaged),
                out,
                &member_path,
            )?;
            out.close("}");
            continue;
        }
        out.line(&format!("zdj_key(&zd_first, \"{name}\");"));
        emit(
            ast,
            member_type,
            &Place::Accessor {
                object: here.to_string(),
                name,
            },
            out,
            &member_path,
        )?;
    }
    out.raw("}");
    out.close("}");
    Ok(())
}

/// Whether a member of `type_` carries `@external`.
///
/// `contract::Member` does not report it, and the C++ binding needs it:
/// XTypes 1.3 §7.5.2.4 gives an `@external` member a holder rather than
/// the value, so a printer that read the accessor directly would print
/// the holder.
pub(crate) fn is_external(ast: &Ast, type_: Id<TypeNode>, name: &str) -> bool {
    // A union's branches are its case members, which `all_members_of`
    // does not list; `fastddsgen` wraps an `@external` branch in
    // `eprosima::fastcdr::external<T>` as it wraps a structure member.
    let cases = ast
        .cases_of(type_)
        .iter()
        .filter_map(|case| ast.case(*case).ok().map(|case| case.member));
    for id in ast.all_members_of(type_).into_iter().chain(cases) {
        let Ok(member) = ast.member(id) else { continue };
        if member.name == name {
            return member
                .facets
                .xtypes
                .as_ref()
                .is_some_and(|facet| facet.is_external);
        }
    }
    false
}

/// A union, as `{"_d": …}` plus the member the discriminator selects.
///
/// Only the selected member's getter is called. Fast DDS's generated
/// accessors throw `BadParamException` when asked for a member the
/// discriminator does not select, so a printer that read them all would
/// abort on every sample instead of printing one.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, below
/// `emit`, which carries the argument: every case member and the
/// discriminator go back through it.
fn emit_union(
    ast: &Ast,
    type_: Id<TypeNode>,
    here: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    let TypeKind::Union { discriminator, .. } = &node.kind else {
        return Err(ProbeError::Unsupported {
            path: path.to_string(),
            kind: "a union whose kind changed under the emitter",
        });
    };
    let discriminator = discriminator
        .resolved()
        .ok_or_else(|| ProbeError::NoLayout {
            detail: format!("{path}: the discriminator is unresolved"),
        })?;

    out.open("{");
    out.line("int zd_first = 1;");
    out.raw("{");
    out.line("zdj_key(&zd_first, \"_d\");");
    emit(
        ast,
        discriminator,
        &Place::Element(format!("{here}._d()")),
        out,
        &format!("{path}._d"),
    )?;
    // The width is the discriminator's, not always `long long`:
    // `union Choice switch (unsigned long long) { case 16045690984833335998: … }`
    // is a corpus case whose label does not fit a signed 64-bit
    // constant, and a `switch` cast to `long long` would compare it
    // wrongly even where the constant is accepted. `discriminator_c_width`
    // answers both halves at once.
    let (width, suffix) = contract::discriminator_c_width(ast, discriminator);
    out.open(&format!("switch (({width})({here}._d())) {{"));
    for id in ast.cases_of(type_) {
        let Ok(case) = ast.case(*id) else { continue };
        let Ok(member) = ast.member(case.member) else {
            continue;
        };
        let Some(member_type) = member.type_.resolved() else {
            continue;
        };
        for label in &case.labels {
            match label.expr() {
                Some(expr) => match ast.eval_integral(expr) {
                    Ok(value) => out.line(&format!("case {value}{suffix}:")),
                    Err(error) => {
                        return Err(ProbeError::NoLayout {
                            detail: format!("{path}: a case label does not evaluate: {error}"),
                        });
                    }
                },
                None => out.line("default:"),
            }
        }
        out.depth += 1;
        out.line(&format!("zdj_key(&zd_first, \"{}\");", member.name));
        // An `@external` branch is read through the wrapper's pointee.
        let place = if is_external(ast, type_, &member.name) {
            Place::Element(format!("(*{here}.{}())", member.name))
        } else {
            Place::Accessor {
                object: here.to_string(),
                name: member.name.clone(),
            }
        };
        emit(
            ast,
            member_type,
            &place,
            out,
            &format!("{path}.{}", member.name),
        )?;
        out.line("break;");
        out.depth = out.depth.saturating_sub(1);
    }
    out.close("}");
    out.raw("}");
    out.close("}");
    Ok(())
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `emit`.
fn emit_array(
    ast: &Ast,
    element: Id<TypeNode>,
    here: &str,
    out: &mut Dump,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(ast, element, &Place::Element(here.to_string()), out, path);
    };
    out.raw("[");
    for index in 0..count {
        if index > 0 {
            out.raw(",");
        }
        emit_array(
            ast,
            element,
            &format!("{here}[{index}]"),
            out,
            &format!("{path}[{index}]"),
            rest,
        )?;
    }
    out.raw("]");
    Ok(())
}

/// An enumeration, printed as the enumerator's name.
///
/// `enum class`, so the enumerator is `Colour::RED` and never a bare
/// `RED` — the same scope `fill_cpp.rs` writes it under.
fn emit_enumeration(ast: &Ast, type_: Id<TypeNode>, here: &str, out: &mut Dump) {
    let scope = cpp_name(ast, type_);
    out.open(&format!("switch ((long long)({here})) {{"));
    for id in ast.enumerators_of(type_) {
        let Ok(node) = ast.enumerator(*id) else {
            continue;
        };
        out.line(&format!("case (long long){scope}::{}:", node.name));
        out.line(&format!("    zdj_raw(\"\\\"{}\\\"\"); break;", node.name));
    }
    out.line(&format!(
        "default: zdj_enum_unknown((long long)({here})); break;"
    ));
    out.close("}");
}

/// A bitmask, printed as the set bits' names in declaration order.
///
/// `fastddsgen` emits `enum <TypeName>Bits : <holder>` beside the
/// typedef, and that enumeration is not an `enum class`, so a flag is
/// named at the enclosing namespace — §7.5.2.2, and the same rule
/// `fill_cpp.rs` writes the flags under.
fn emit_bitmask(ast: &Ast, type_: Id<TypeNode>, here: &str, out: &mut Dump) {
    let prefix = namespace_prefix(ast, type_);
    out.open("{");
    out.line("int zd_first = 1;");
    out.raw("[");
    for id in ast.bits_of(type_) {
        let Ok(node) = ast.bit_value(*id) else {
            continue;
        };
        out.open(&format!(
            "if (((unsigned long long)({here}) & (unsigned long long){prefix}{}) != 0) {{",
            node.name
        ));
        out.line("if (!zd_first) { zdj_raw(\",\"); }");
        out.line("zd_first = 0;");
        out.line(&format!("zdj_raw(\"\\\"{}\\\"\");", node.name));
        out.close("}");
    }
    out.raw("]");
    out.close("}");
}

fn primitive_call(kind: PrimitiveKind, here: &str) -> String {
    match kind {
        PrimitiveKind::Boolean => format!("zdj_bool({here});"),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::UInt16
        | PrimitiveKind::UInt32 => format!("zdj_u64((unsigned long long)({here}));"),
        PrimitiveKind::Int8 | PrimitiveKind::Int16 | PrimitiveKind::Int32 => {
            format!("zdj_i64((long long)({here}));")
        }
        PrimitiveKind::Char => format!("zdj_char((char)({here}));"),
        PrimitiveKind::WChar => format!("zdj_wchar((unsigned long)({here}));"),
        PrimitiveKind::Int64 => format!("zdj_i64_string((long long)({here}));"),
        PrimitiveKind::UInt64 => format!("zdj_u64_string((unsigned long long)({here}));"),
        PrimitiveKind::Float32 => format!("zdj_f32((float)({here}));"),
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => {
            format!("zdj_f64((double)({here}));")
        }
    }
}

/// The namespace a type's members are named through, with the trailing
/// `::` when there is one.
fn namespace_prefix(ast: &Ast, type_: Id<TypeNode>) -> String {
    let scoped = match ast.any_of_type(type_) {
        Ok(any) => ast.scoped_name_of(any),
        Err(_) => None,
    };
    let Some(scoped) = scoped else {
        return String::new();
    };
    let segments = scoped.segments();
    if segments.len() <= 1 {
        return String::new();
    }
    let mut prefix = segments[..segments.len() - 1].join("::");
    prefix.push_str("::");
    prefix
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::encode::model_from_idl;

    fn emitted(idl: &str, type_name: &str) -> String {
        let ast = model_from_idl(idl, "dump_cpp.idl").expect("the fixture parses");
        let type_ = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
                type_name.to_string(),
            ]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model");
        dump_fastdds(&ast, type_).expect("the type has a C++ shape")
    }

    #[test]
    fn a_member_is_read_through_its_getter() {
        let cpp = emitted("@final struct S { long a; string text; };", "S");
        assert!(cpp.contains("zdj_i64((long long)(sample.a()));"), "{cpp}");
        assert!(cpp.contains("zdj_string(sample.text().c_str());"), "{cpp}");
    }

    #[test]
    fn a_nested_aggregate_chains_the_getters() {
        let cpp = emitted(
            "@final struct Inner { long x; };
             @final struct S { Inner nested; };",
            "S",
        );
        assert!(cpp.contains("sample.nested().x()"), "{cpp}");
    }

    #[test]
    fn an_optional_member_is_read_only_when_it_has_a_value() {
        let cpp = emitted("@mutable struct S { @optional long b; };", "S");
        assert!(cpp.contains("sample.b().has_value()"), "{cpp}");
        assert!(cpp.contains("sample.b().value()"), "{cpp}");
    }

    #[test]
    fn a_map_prints_as_an_array_of_pairs() {
        let cpp = emitted("@final struct S { map<long, double> table; };", "S");
        assert!(cpp.contains(".first"), "{cpp}");
        assert!(cpp.contains(".second"), "{cpp}");
        assert!(!cpp.contains("zdj_key(&zd_first, \"table\");\n    zdj_raw(\"{\")"));
    }

    #[test]
    fn a_bitset_prints_its_named_fields_and_not_its_padding() {
        let cpp = emitted(
            "@bit_bound(16) bitset Packed { bitfield<3> lo; bitfield<2>; bitfield<3> hi; };
             @final struct S { Packed bits; };",
            "S",
        );
        assert!(cpp.contains("sample.bits().lo"), "{cpp}");
        assert!(cpp.contains("sample.bits().hi"), "{cpp}");
    }

    #[test]
    fn a_fixed_is_refused_because_fastddsgen_rejects_the_idl() {
        let ast = model_from_idl("@final struct S { fixed<7, 2> amount; };", "dump_cpp.idl")
            .expect("the fixture parses");
        let type_ = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
                "S".to_string(),
            ]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model");
        assert!(matches!(
            dump_fastdds(&ast, type_),
            Err(ProbeError::Unsupported { .. })
        ));
    }
}
