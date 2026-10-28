// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C that puts one JSON sample into a `rosidl` generated structure.
//!
//! A third backend beside `fill.rs` (Cyclone and RTI) and `fill_cpp.rs`
//! (Fast DDS), for the same reason `fill_cpp.rs` is separate:
//! `rosidl_generator_c`'s structures are not the IDL-to-C mapping the
//! two DDS C vendors implement, so nothing above the walk is shared.
//!
//! ## What `rosidl` puts where the C mapping puts something else
//!
//! Measured against `rosidl_generator_c` 3.1.6 on the Linux test host:
//!
//! | IDL | the two C vendors | `rosidl` |
//! |---|---|---|
//! | `string` | `char *` | `rosidl_runtime_c__String` — `data`, `size`, `capacity` |
//! | `wstring` | `wchar_t *` / `DDS_Wchar *` | `rosidl_runtime_c__U16String`, units 16 bits wide |
//! | `sequence<long>` | `_buffer`/`_length`, or `DDS_LongSeq` | `rosidl_runtime_c__int32__Sequence` — `data`, `size`, `capacity` |
//! | `char` | `char` | `signed char` |
//! | `wchar` | `wchar_t` or a 16-bit holder | `uint16_t` |
//! | a struct | `mod_Name` | `zdros__msg__Name` |
//!
//! ## Why the length is passed and not left to `strlen`
//!
//! `rosidl_runtime_c__String__assign` takes a NUL-terminated pointer and
//! measures it. `__assignn` takes a count. This uses `__assignn`, so a
//! corpus string holding an interior NUL goes into the structure whole
//! and `size` is the true length. That matters because the *serialiser*
//! then uses `strlen` — `cdr << str->data` in the generated
//! `_cdr_serialize` — and writes only up to the first NUL. Filling with
//! `__assign` would have hidden that behind an already-truncated
//! structure; filling with `__assignn` makes it a measurable difference
//! between this column and the others, which is what it is.

use crate::contract::{self, Consumed};
use crate::encode::ProbeError;
use crate::ros_idl::{PACKAGE, ros_message_name};
use serde_json::Value;
use std::fmt::Write as _;
use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The statements, ready to `#include` into the ROS probe template.
#[derive(Debug, Default)]
struct Fill {
    /// Declarations that have to precede the assignments — the static
    /// arrays a `wstring`'s code units live in.
    declarations: String,
    statements: String,
    /// A counter, so two `wstring`s in one sample get distinct names.
    next: usize,
    seen: Consumed,
}

impl Fill {
    fn temporary(&mut self, stem: &str) -> String {
        self.next += 1;
        format!("zd_{stem}_{}", self.next)
    }
}

/// Emit the statements that fill `sample` with `values`, for `rosidl`.
///
/// # Errors
/// [`ProbeError`], naming the path in the value.
pub fn fill_ros(ast: &Ast, type_: Id<TypeNode>, values: &Value) -> Result<String, ProbeError> {
    fill_ros_recording(ast, type_, values, &mut Consumed::new())
}

/// As [`fill_ros`], recording which scalars of the value were read.
///
/// # Errors
/// As [`fill_ros`].
pub fn fill_ros_recording(
    ast: &Ast,
    type_: Id<TypeNode>,
    values: &Value,
    seen: &mut Consumed,
) -> Result<String, ProbeError> {
    // Before the first hop; see `contract::reject_alias_cycle`.
    contract::reject_alias_cycle(ast)?;
    let mut out = Fill::default();
    let root = contract::root_path(ast, type_);
    let result = emit(ast, type_, values, "sample", &mut out, &root);
    *seen = core::mem::take(&mut out.seen);
    result?;
    let mut source = out.declarations;
    source.push_str(&out.statements);
    Ok(source)
}

/// The C identifier a named type takes in `rosidl` generated code.
///
/// `zdros__msg__Sample`. Every named type in a package sits in the same
/// `msg` namespace, because that is the only place `rosidl` puts one.
#[must_use]
pub fn ros_c_name(ast: &Ast, type_: Id<TypeNode>) -> String {
    ros_message_name(ast, type_).map_or_else(
        || String::from("UNKNOWN"),
        |name| format!("{PACKAGE}__msg__{name}"),
    )
}

/// zerodds-lint: recursion-depth 128 — the sample's own nesting. Every arm
/// but the typedef one descends into a strictly smaller part of the value,
/// so the walk ends because the sample is finite: a recursive type has
/// finite values, because the sequence that closes its cycle is eventually
/// empty. The typedef arm keeps the value and is finite because
/// `contract::reject_alias_cycle` at the entry point has refused a chain
/// that returns to itself. A value file is `serde_json`'s to parse and its
/// `RECURSION_LIMIT` is 128.
fn emit(
    ast: &Ast,
    type_: Id<TypeNode>,
    value: &Value,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
) -> Result<(), ProbeError> {
    let node = ast.type_(type_).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    match &node.kind {
        TypeKind::Primitive(kind) => {
            let literal = primitive_literal(*kind, value, path)?;
            out.seen.scalar(path);
            let _ = writeln!(out.statements, "{lvalue} = {literal};");
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            let bytes = text.as_bytes();
            let _ = writeln!(
                out.statements,
                "ZDROS_STRING(&{lvalue}, {}, {});",
                c_string(text),
                bytes.len()
            );
            Ok(())
        }
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => {
            let text = contract::text(value, path)?;
            out.seen.scalar(path);
            emit_wstring(text, lvalue, out);
            Ok(())
        }
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            emit(ast, target, value, lvalue, out, path)
        }
        TypeKind::Structure { .. } => {
            let object = contract::object(value, path)?;
            let members = contract::members(ast, type_);
            let declared: Vec<String> = members.iter().map(|member| member.name.clone()).collect();
            contract::reject_unknown_keys(object, &declared, path)?;
            for member in &members {
                let Some(member_value) = contract::member_value(object, member, path)? else {
                    // `ros_idl` refuses an `@optional` member, so a
                    // member with no value here is a required one the
                    // sample does not carry — and `member_value` has
                    // already turned that into `Missing`.
                    continue;
                };
                emit(
                    ast,
                    member.type_,
                    member_value,
                    &format!("{lvalue}.{}", member.name),
                    out,
                    &contract::join(path, &member.name),
                )?;
            }
            Ok(())
        }
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the array element is unresolved"),
            })?;
            let counts = contract::array_dimensions(ast, dims, path)?;
            emit_array(ast, element, value, lvalue, out, path, &counts)
        }
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the sequence element is unresolved"),
            })?;
            let items = contract::bounded_items(ast, value, *bound, "elements", path)?;
            emit_sequence(ast, element, items, lvalue, out, path)
        }
        // Everything else has already been refused by `ros_idl`, which
        // runs first and is the single gate on what this column takes.
        // Reaching one here means the two walks disagree, and saying so
        // is better than emitting C for a type the IDL does not have.
        other => Err(ProbeError::Unfilled {
            path: path.to_string(),
            kind: other.as_str(),
        }),
    }
}

/// A `wstring` field, as a static array of 16-bit code units.
///
/// `rosidl`'s holder is `rosidl_runtime_c__U16String`, whose `data` is
/// `uint16_t *` — the width is fixed by the runtime and not left to the
/// platform, so unlike `fill.rs` this does not need `__typeof__`. The
/// units are UTF-16, surrogate pairs included, which is what
/// `__assignn` copies verbatim.
fn emit_wstring(text: &str, lvalue: &str, out: &mut Fill) {
    let buffer = out.temporary("wstr");
    let mut units = String::new();
    let mut count = 0usize;
    for unit in text.encode_utf16() {
        let _ = write!(units, "0x{unit:04x}, ");
        count += 1;
    }
    // A zero-length array is not C. The trailing zero also gives an
    // empty `wstring` a valid pointer to hand `__assignn` with n = 0.
    let _ = writeln!(
        out.declarations,
        "static const uint16_t {buffer}[] = {{ {units}0x0000 }};"
    );
    let _ = writeln!(
        out.statements,
        "ZDROS_WSTRING(&{lvalue}, {buffer}, {count});"
    );
}

/// zerodds-lint: recursion-depth 8 — one frame per array dimension.
/// `counts` drops its head at each frame and the innermost element goes to
/// `emit`, which is bounded by the sample.
#[allow(clippy::too_many_arguments)]
fn emit_array(
    ast: &Ast,
    element: Id<TypeNode>,
    value: &Value,
    lvalue: &str,
    out: &mut Fill,
    path: &str,
    counts: &[usize],
) -> Result<(), ProbeError> {
    let Some((&count, rest)) = counts.split_first() else {
        return emit(ast, element, value, lvalue, out, path);
    };
    let items = contract::array(value, path)?;
    if items.len() != count {
        return Err(ProbeError::Mismatch {
            path: path.to_string(),
            expected: format!("exactly {count} elements"),
            found: format!("{}", items.len()),
        });
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

/// A sequence, through the `__Sequence__init` its element type has.
///
/// zerodds-lint: recursion-depth 128 — the sample's own nesting, below
/// `emit`, which carries the argument. Each element is a strictly smaller
/// part of the array this was handed.
fn emit_sequence(
    ast: &Ast,
    element: Id<TypeNode>,
    items: &[Value],
    lvalue: &str,
    out: &mut Fill,
    path: &str,
) -> Result<(), ProbeError> {
    let count = items.len();
    let init = format!("{}__Sequence__init", sequence_stem(ast, element, path)?);
    // `__init(&s, 0)` allocates nothing and sets `size = 0`, which is
    // the empty sequence. It still has to be called: the generated
    // `_cdr_serialize` reads `size` and `data` unconditionally.
    let _ = writeln!(out.statements, "ZDROS_SEQ({init}, &{lvalue}, {count});");
    for (index, item) in items.iter().enumerate() {
        emit(
            ast,
            element,
            item,
            &format!("{lvalue}.data[{index}]"),
            out,
            &format!("{path}[{index}]"),
        )?;
    }
    Ok(())
}

/// The `rosidl_runtime_c__<stem>` or `zdros__msg__<Name>` a sequence of
/// this element type is named after.
///
/// Measured from `rosidl_runtime_c/primitives_sequence.h`: the stems are
/// the IDL keywords with `long double` spelled `long_double`, and a
/// string sequence is `String` or `U16String` rather than a primitive.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop,
/// `contract::MAX_ALIAS_HOPS`. The typedef arm is the only recursive one,
/// and a chain that returns to itself has been refused by
/// `contract::reject_alias_cycle` at the entry of every walk that reaches
/// here.
fn sequence_stem(ast: &Ast, element: Id<TypeNode>, path: &str) -> Result<String, ProbeError> {
    let node = ast.type_(element).map_err(|error| ProbeError::NoLayout {
        detail: format!("{error}"),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => format!("rosidl_runtime_c__{}", primitive_stem(*kind)),
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => "rosidl_runtime_c__String".to_string(),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => "rosidl_runtime_c__U16String".to_string(),
        TypeKind::Structure { .. } => ros_c_name(ast, element),
        TypeKind::Alias { aliased } => {
            let target = aliased.resolved().ok_or_else(|| ProbeError::NoLayout {
                detail: format!("{path}: the typedef target is unresolved"),
            })?;
            return sequence_stem(ast, target, path);
        }
        other => {
            return Err(ProbeError::Unfilled {
                path: path.to_string(),
                kind: other.as_str(),
            });
        }
    })
}

/// The stem `rosidl_runtime_c` gives each primitive's sequence.
const fn primitive_stem(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "boolean",
        PrimitiveKind::Octet => "octet",
        PrimitiveKind::UInt8 => "uint8",
        PrimitiveKind::Int8 => "int8",
        PrimitiveKind::Char => "char",
        PrimitiveKind::WChar => "wchar",
        PrimitiveKind::Int16 => "int16",
        PrimitiveKind::UInt16 => "uint16",
        PrimitiveKind::Int32 => "int32",
        PrimitiveKind::UInt32 => "uint32",
        PrimitiveKind::Int64 => "int64",
        PrimitiveKind::UInt64 => "uint64",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        PrimitiveKind::Float128 => "long_double",
    }
}

/// The C literal for one primitive, in `rosidl`'s holder types.
///
/// A character is written as its code point and not as a C character
/// literal. `rosidl` gives `char` a `signed char` and `wchar` a
/// `uint16_t`; a literal like `'é'` in a source file is neither, and a
/// multi-character literal is implementation-defined. The code point is
/// unambiguous in both holders and `contract::character_for` has already
/// refused anything the width cannot carry.
fn primitive_literal(kind: PrimitiveKind, value: &Value, path: &str) -> Result<String, ProbeError> {
    Ok(match kind {
        PrimitiveKind::Boolean => {
            if contract::boolean(value, path)? {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        PrimitiveKind::Char => {
            let point = u32::from(contract::character_for(kind, value, path)?);
            format!("(signed char)0x{point:02x}")
        }
        PrimitiveKind::WChar => {
            let point = u32::from(contract::character_for(kind, value, path)?);
            format!("(uint16_t)0x{point:04x}")
        }
        PrimitiveKind::Float32 => format!("{:.17e}f", contract::float(value, path)?),
        PrimitiveKind::Float64 => format!("{:.17e}", contract::float(value, path)?),
        // `long double` on this host is the x87 80-bit format in sixteen
        // bytes, not binary128. The literal is written with the `L`
        // suffix so the compiler keeps whatever extra range it has
        // rather than rounding through a `double` first; what the value
        // then *is* on the wire is the measurement.
        PrimitiveKind::Float128 => format!("{:.17e}L", contract::float(value, path)?),
        PrimitiveKind::Int64 => {
            let n = contract::integer_for(kind, value, path)?;
            // C has no literal for INT64_MIN: `-9223372036854775808LL`
            // is a positive constant too large for `long long`, parsed
            // as `unsigned long long` and then negated, and gcc says so
            // — `fill.inc: warning: integer constant is so large that
            // it is unsigned`, on `integer-extremes`, in both ROS
            // columns. The value it ends up with is the right one on
            // every compiler this harness runs, and it gets there
            // through an implementation-defined conversion. `fill.rs`
            // spells it with the same subtraction `<stdint.h>` uses;
            // this filler and `fill_cpp.rs` had not been given it.
            if n == i128::from(i64::MIN) {
                "(-9223372036854775807LL - 1)".to_string()
            } else {
                format!("{n}LL")
            }
        }
        PrimitiveKind::UInt64 => format!("{}ULL", contract::integer_for(kind, value, path)?),
        _ => format!("{}", contract::integer_for(kind, value, path)?),
    })
}

/// A C string literal, with every byte outside printable ASCII escaped.
///
/// Octal and not hex: `\xNN` in C swallows every following hex digit, so
/// `"\xe2" "8"` and `"\xe28"` are different literals and the second is
/// not the one meant. Octal escapes are exactly three digits.
fn c_string(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.as_bytes() {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e => out.push(char::from(*byte)),
            other => {
                let _ = write!(out, "\\{other:03o}");
            }
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::{c_string, fill_ros, ros_c_name};
    use crate::encode::model_from_idl;
    use serde_json::json;
    use zerodds_ast::Ast;
    use zerodds_ast_core::Id;

    fn model(idl: &str, type_name: &str) -> (Ast, Id<zerodds_ast::TypeNode>) {
        let ast = model_from_idl(idl, "fill_ros.idl").expect("the fixture parses");
        let type_ = ast
            .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
                type_name.to_string(),
            ]))
            .ok()
            .and_then(zerodds_ast::AnyId::as_type)
            .expect("the type is in the model");
        (ast, type_)
    }

    #[test]
    fn a_scalar_member_is_assigned_directly() {
        let (ast, type_) = model("@final struct S { long a; double b; };", "S");
        let c = fill_ros(&ast, type_, &json!({"a": -7, "b": 1.5})).expect("a scalar fills");
        assert!(c.contains("sample.a = -7;"), "{c}");
        assert!(c.contains("sample.b = 1.5"), "{c}");
    }

    #[test]
    fn a_string_carries_its_length_so_an_interior_nul_is_not_lost_before_the_serialiser() {
        let (ast, type_) = model("@final struct S { string text; };", "S");
        let c = fill_ros(&ast, type_, &json!({"text": "a\u{0}b"})).expect("a string fills");
        assert!(
            c.contains("ZDROS_STRING(&sample.text, \"a\\000b\", 3);"),
            "{c}"
        );
    }

    #[test]
    fn a_wstring_becomes_sixteen_bit_units_and_not_a_wide_literal() {
        let (ast, type_) = model("@final struct S { wstring w; };", "S");
        let c = fill_ros(&ast, type_, &json!({"w": "ab"})).expect("a wstring fills");
        assert!(c.contains("static const uint16_t"), "{c}");
        assert!(c.contains("0x0061, 0x0062,"), "{c}");
        assert!(c.contains("ZDROS_WSTRING(&sample.w,"), "{c}");
        assert!(c.contains(", 2);"), "{c}");
    }

    #[test]
    fn a_non_bmp_wstring_keeps_both_surrogates() {
        let (ast, type_) = model("@final struct S { wstring w; };", "S");
        let c = fill_ros(&ast, type_, &json!({"w": "\u{1f600}"})).expect("a wstring fills");
        assert!(c.contains("0xd83d, 0xde00,"), "{c}");
        assert!(c.contains(", 2);"), "{c}");
    }

    #[test]
    fn a_sequence_goes_through_the_init_its_element_type_names() {
        let (ast, type_) = model("@final struct S { sequence<long> q; };", "S");
        let c = fill_ros(&ast, type_, &json!({"q": [1, 2]})).expect("a sequence fills");
        assert!(
            c.contains("ZDROS_SEQ(rosidl_runtime_c__int32__Sequence__init, &sample.q, 2);"),
            "{c}"
        );
        assert!(c.contains("sample.q.data[0] = 1;"), "{c}");
        assert!(c.contains("sample.q.data[1] = 2;"), "{c}");
    }

    #[test]
    fn an_empty_sequence_is_still_initialised() {
        let (ast, type_) = model("@final struct S { sequence<long> q; };", "S");
        let c = fill_ros(&ast, type_, &json!({"q": []})).expect("an empty sequence fills");
        assert!(c.contains(", &sample.q, 0);"), "{c}");
    }

    #[test]
    fn a_sequence_of_structs_names_the_generated_sequence_init() {
        let idl = "@final struct Cell { long x; };\n@final struct S { sequence<Cell> q; };";
        let (ast, type_) = model(idl, "S");
        let c = fill_ros(&ast, type_, &json!({"q": [{"x": 3}]})).expect("it fills");
        assert!(
            c.contains("ZDROS_SEQ(zdros__msg__Cell__Sequence__init, &sample.q, 1);"),
            "{c}"
        );
        assert!(c.contains("sample.q.data[0].x = 3;"), "{c}");
    }

    #[test]
    fn a_char_and_a_wchar_are_written_as_code_points_and_not_as_c_literals() {
        let (ast, type_) = model("@final struct S { char c; wchar w; };", "S");
        let c = fill_ros(&ast, type_, &json!({"c": "\u{e9}", "w": "\u{20ac}"}))
            .expect("both characters fill");
        assert!(c.contains("sample.c = (signed char)0xe9;"), "{c}");
        assert!(c.contains("sample.w = (uint16_t)0x20ac;"), "{c}");
    }

    #[test]
    fn an_array_is_filled_element_by_element() {
        let (ast, type_) = model("@final struct S { long a[3]; };", "S");
        let c = fill_ros(&ast, type_, &json!({"a": [1, 2, 3]})).expect("an array fills");
        assert!(c.contains("sample.a[0] = 1;"), "{c}");
        assert!(c.contains("sample.a[2] = 3;"), "{c}");
    }

    #[test]
    fn a_struct_takes_the_double_underscore_name_rosidl_generates() {
        let (ast, type_) = model("@final struct Sample { long a; };", "Sample");
        assert_eq!(ros_c_name(&ast, type_), "zdros__msg__Sample");
    }

    #[test]
    fn a_string_escape_is_octal_so_a_following_digit_cannot_join_it() {
        // `"\xe28"` is one hex escape in C and not what was meant.
        assert_eq!(c_string("\u{e9}8"), "\"\\303\\2518\"");
    }
}
