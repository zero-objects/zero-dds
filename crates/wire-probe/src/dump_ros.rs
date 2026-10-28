// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C that prints a `rosidl` structure back as canonical JSON.
//!
//! The read direction of `fill_ros.rs`. After the ROS type support's own
//! `_cdr_deserialize` has filled the structure, these statements print
//! it in the form `crate::canonical` compares — the same JSON the other
//! columns print, so a difference in the document is a difference in
//! what the two deserialisers made of the same bytes.
//!
//! ## Why this is not a self-oracle
//!
//! The statements read the *ROS* structure, which the *ROS*
//! deserialiser filled. A printer that read the wrong field shows up as
//! a disagreement between this column and the others, never as a false
//! agreement.
//!
//! ## The one place a length has to be checked rather than trusted
//!
//! `tools/wireprobe/dumpjson.h`'s `zdj_string` takes a NUL-terminated
//! pointer. A `rosidl_runtime_c__String` carries both a `size` and a
//! terminator, and a payload with an interior NUL makes them disagree —
//! `strlen` would stop early and the case would come back short with
//! nothing said. `ZDROS_STRING_OUT` in the decode template compares the
//! two and fails the run rather than printing the shorter one.

use crate::contract;
use crate::encode::ProbeError;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The statements, ready to `#include` into the ROS decode template.
#[derive(Debug, Default)]
struct Dump {
    body: String,
    depth: usize,
    /// The aggregates on a cycle, which this walk refuses rather than
    /// expands. See [`crate::recursion::recursive_aggregates`] and the
    /// arm in [`emit`] that reads it.
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
}

/// Emit the C statements that print `sample` as canonical JSON.
///
/// # Errors
/// [`ProbeError`], naming the path in the type.
pub fn dump_ros(ast: &Ast, type_: Id<TypeNode>) -> Result<String, ProbeError> {
    // Before `recursive_aggregates`; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Dump {
        rec: crate::recursion::recursive_aggregates(ast, type_),
        ..Dump::default()
    };
    emit(ast, type_, "sample", &mut out, "")?;
    out.line("zdj_raw(\"\\n\");");
    Ok(out.body)
}

/// zerodds-lint: recursion-depth 64 — the type graph's own nesting,
/// which is finite because the walk never enters a cycle: an aggregate
/// on one is refused in the `Structure` arm below, and a typedef chain
/// that returns to itself has been refused by `reject_alias_cycle`
/// before the first hop. Every other arm strictly descends the graph, so
/// the depth is the IDL's own and the corpus's deepest is four.
///
/// This is the walk that has no function to call for a cycle. `dump.rs`,
/// `dump_cpp.rs` and `corba_cpp.rs` emit one function per aggregate on
/// a cycle and call it where they would have unrolled; ROS has no such
/// shape to emit into, and would have none to compare against if it
/// had — `ros_idl::Printer::declare` refuses a recursive type outright,
/// "for which §7.4.1.1 gives no encoding to compare against". Until
/// this arm existed, `wireprobe-dump --ros` did not refuse it either: it
/// followed `A → sequence<B> → B → sequence<A>`
/// until the stack ran out, which the corpus case `recursive-mutual`
/// reproduces. `ros_idl` is the gate for the column, but it is a
/// different process invocation and not a guard on this function.
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            out.line(&primitive_call(*kind, expr));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => {
            out.line(&format!("ZDROS_STRING_OUT(&{expr});"));
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            // `rosidl_runtime_c__U16String` carries its own count, so
            // unlike the two C vendors' `Char16 *` there is no
            // terminator to walk to and no width to discover. `data` is
            // `uint16_t *`, which is `zdj_wstring`'s element type.
            out.line(&format!(
                "zdj_wstring((const unsigned short *){expr}.data, {expr}.size);"
            ));
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            emit(ast, target, expr, out, path)
        }
        TypeKind::Structure { .. } => {
            if out.rec.contains(&type_) {
                return Err(ProbeError::Unsupported {
                    path: path.to_string(),
                    kind: "a recursive type, for which §7.4.1.1 gives no encoding to compare \
                           against — the same refusal `ros_idl` makes, said here because the \
                           expansion of a cycle does not end",
                });
            }
            emit_aggregate(ast, type_, expr, out, path)
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            emit_array(ast, element, expr, out, path, &counts)
        }
        TypeKind::Sequence { element, .. } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            emit_sequence(ast, element, expr, out, path)
        }
        // As in `fill_ros`: `ros_idl` is the gate, and anything reaching
        // here means the two walks disagree.
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, below
/// `emit`, which carries the argument: every member goes back through it,
/// and an aggregate on a cycle is refused there.
fn emit_aggregate(
    ast: &Ast,
    type_: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    out.open("{");
    out.line("int zd_first = 1;");
    out.raw("{");
    for member in contract::members(ast, type_) {
        let member_path = contract::join(path, &member.name);
        out.line(&format!("zdj_key(&zd_first, \"{}\");", member.name));
        emit(
            ast,
            member.type_,
            &format!("{expr}.{}", member.name),
            out,
            &member_path,
        )?;
    }
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
    expr: &str,
    out: &mut Dump,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(ast, element, expr, out, path);
    };
    out.raw("[");
    for index in 0..count {
        if index > 0 {
            out.raw(",");
        }
        emit_array(
            ast,
            element,
            &format!("{expr}[{index}]"),
            out,
            &format!("{path}[{index}]"),
            rest,
        )?;
    }
    out.raw("]");
    Ok(())
}

/// A sequence, read out of `rosidl`'s `data`/`size` pair.
///
/// The length is the bytes' and not the type's, so the element
/// statements go inside a `for`.
///
/// zerodds-lint: recursion-depth 64 — the type graph's own nesting, below
/// `emit`, which carries the argument: the element goes back through it.
fn emit_sequence(
    ast: &Ast,
    element: Id<TypeNode>,
    expr: &str,
    out: &mut Dump,
    path: &str,
) -> Result<(), ProbeError> {
    out.open("{");
    out.line(&format!("size_t zd_n = {expr}.size;"));
    out.raw("[");
    out.open("for (size_t zd_i = 0; zd_i < zd_n; zd_i++) {");
    out.line("if (zd_i > 0) { zdj_raw(\",\"); }");
    emit(
        ast,
        element,
        &format!("{expr}.data[zd_i]"),
        out,
        &format!("{path}[]"),
    )?;
    out.close("}");
    out.raw("]");
    out.close("}");
    Ok(())
}

/// The `dumpjson.h` call for one primitive, in `rosidl`'s holders.
fn primitive_call(kind: PrimitiveKind, expr: &str) -> String {
    match kind {
        PrimitiveKind::Boolean => format!("zdj_bool(({expr}) ? 1 : 0);"),
        // `char` is a `signed char` in `rosidl`, so a code point above
        // 0x7F arrives negative. The cast to `unsigned char` before the
        // widening is what keeps `0xE9` from printing as `-23`.
        PrimitiveKind::Char => format!("zdj_char((char)(unsigned char)({expr}));"),
        // `wchar` is a `uint16_t`. No sign to undo and no width to
        // discover: `rosidl` fixes both.
        PrimitiveKind::WChar => format!("zdj_wchar((unsigned long)({expr}));"),
        PrimitiveKind::Float32 => format!("zdj_f32((float)({expr}));"),
        PrimitiveKind::Float64 => format!("zdj_f64((double)({expr}));"),
        // The canonical form prints a 128-bit float as a string, and
        // `zdj_f128_string` takes the runtime's own `zdw_f128`. This
        // column's holder is the compiler's `long double`, which on
        // x86-64 is the x87 80-bit format and not binary128, so the
        // value is printed through `double` and the *bytes* are where
        // the divergence is read. `docs/ros-column.md` states it.
        PrimitiveKind::Float128 => format!("zdj_f64((double)({expr}));"),
        PrimitiveKind::Int64 => format!("zdj_i64_string((long long)({expr}));"),
        PrimitiveKind::UInt64 => format!("zdj_u64_string((unsigned long long)({expr}));"),
        PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::UInt16
        | PrimitiveKind::UInt32 => format!("zdj_u64((unsigned long long)({expr}));"),
        PrimitiveKind::Int8 | PrimitiveKind::Int16 | PrimitiveKind::Int32 => {
            format!("zdj_i64((long long)({expr}));")
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::dump_ros;
    use crate::encode::model_from_idl;

    fn emitted(idl: &str, type_name: &str) -> String {
        let ast = model_from_idl(idl, "dump_ros.idl").expect("the fixture parses");
        let type_ = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
                type_name.to_string(),
            ]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model");
        dump_ros(&ast, type_).expect("the type has a rosidl shape")
    }

    #[test]
    fn a_structure_prints_its_members_in_declaration_order() {
        let c = emitted("@final struct S { long a; double b; };", "S");
        let a = c.find("\\\"a\\\"").or_else(|| c.find("\"a\""));
        let b = c.find("\\\"b\\\"").or_else(|| c.find("\"b\""));
        assert!(a < b, "{c}");
        assert!(c.contains("zdj_i64((long long)(sample.a));"), "{c}");
        assert!(c.contains("zdj_f64((double)(sample.b));"), "{c}");
    }

    #[test]
    fn a_string_is_printed_through_the_guard_that_compares_size_with_strlen() {
        let c = emitted("@final struct S { string text; };", "S");
        assert!(c.contains("ZDROS_STRING_OUT(&sample.text);"), "{c}");
    }

    #[test]
    fn a_wstring_takes_its_count_from_the_holder_and_not_from_a_terminator() {
        let c = emitted("@final struct S { wstring w; };", "S");
        assert!(
            c.contains("zdj_wstring((const unsigned short *)sample.w.data, sample.w.size);"),
            "{c}"
        );
    }

    #[test]
    fn a_signed_char_holder_is_widened_through_unsigned_char() {
        let c = emitted("@final struct S { char c; };", "S");
        assert!(
            c.contains("zdj_char((char)(unsigned char)(sample.c));"),
            "{c}"
        );
    }

    #[test]
    fn a_sequence_takes_its_length_from_the_bytes_and_not_from_the_type() {
        let c = emitted("@final struct S { sequence<long> q; };", "S");
        assert!(c.contains("size_t zd_n = sample.q.size;"), "{c}");
        assert!(c.contains("sample.q.data[zd_i]"), "{c}");
    }

    #[test]
    fn an_array_is_unrolled_because_its_length_is_the_types() {
        let c = emitted("@final struct S { long a[3]; };", "S");
        assert!(c.contains("sample.a[0]"), "{c}");
        assert!(c.contains("sample.a[2]"), "{c}");
        assert!(!c.contains("sample.a[3]"), "{c}");
    }

    #[test]
    fn a_sixty_four_bit_integer_prints_as_a_string_like_every_other_column() {
        let c = emitted(
            "@final struct S { long long a; unsigned long long b; };",
            "S",
        );
        assert!(c.contains("zdj_i64_string"), "{c}");
        assert!(c.contains("zdj_u64_string"), "{c}");
    }
}
