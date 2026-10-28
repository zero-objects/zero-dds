// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An array/sequence member's ROS `default_value` — the elements, kept
//! next to the type rather than in the model.
//!
//! XTypes' `@default` (§7.3.4.5.4) carries one `<const_expr>`, never a
//! list, so this does not ask [`zerodds_types::dynamic`] to hold a
//! collection default at all — it parses the text into
//! [`zerodds_types::dynamic::DynamicData`] elements and hands them back
//! for the caller (`super::ZddsDynBuilder`/`ZddsDynType`/`ZddsDynData`)
//! to keep beside the member and apply at
//! `dynamic_data_init_from_dynamic_type*` and at
//! `dynamic_data_clear_value`/`clear_all_values`.
//!
//! ## The text's actual shape
//!
//! `rosidl_adapter`'s `to_idl_literal` writes an array/sequence field's
//! `.msg` default as `repr(tuple(value))` **inside an IDL string
//! literal** (`@default(value="(1, 2, 3)")`) — IDL's annotation grammar
//! has no array-literal parameter, so ROS folds the list into a string
//! instead. `rosidl_parser` reads that IDL string literal back as a
//! Python `str` — the characters `(1, 2, 3)`, quotes and IDL-level
//! backslash-escaping already gone — and
//! `rosidl_generator_type_description`'s `serialize_field` writes it
//! unchanged into `Field.default_value`. So what reaches
//! [`parse`] is Python's `repr()` of a tuple:
//!
//! - `()` — no elements.
//! - `(x,)` — one element (a trailing comma, no space, is Python's own
//!   singleton-tuple spelling and is not a second, empty element).
//! - `(x, y, z)` — two or more (comma-space between, no trailing comma).
//!
//! Each element is itself Python's `repr()` of the value
//! `rosidl_adapter.parser.parse_primitive_value_string` gave the
//! `.msg` text: `True`/`False` for `bool`; a plain decimal integer for
//! every integer kind, `byte` and `char` included — both map to an
//! unsigned 8-bit IDL type in `rosidl_adapter.msg.MSG_TYPE_TO_IDL`,
//! never to a character literal; a decimal-point or scientific-notation
//! float, or `inf`/`-inf`/`nan`, for a floating kind (Rust's own
//! `f32`/`f64` parser already reads exactly those spellings); and a
//! single- or double-quoted, backslash-escaped string otherwise.
//!
//! ## What is refused
//!
//! Anything this cannot read stops the whole member, [`Err`] naming
//! what and where: a non-tuple-shaped text, an element that does not
//! parse as its kind, an integer or a character code outside its
//! kind's range, a string past its bound, and a parsed length that
//! does not match a fixed array's own dimension or exceeds a bounded
//! sequence's own bound. Nothing here silently shortens a list, clamps
//! a value, or truncates a string into place.

use zerodds_ast::PrimitiveKind;
use zerodds_float128::Float128;
use zerodds_types::dynamic::DynamicData;

use super::ZddsDynKindTag;

/// How many elements a collection default's length is allowed to be —
/// the one fact [`parse`] cannot know about a member's shape from the
/// text alone, so the caller states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LengthRule {
    /// `shape == Array`: exactly this many — the array's own dimension.
    Exact(u64),
    /// `shape == BoundedSequence`: at most this many — the sequence's
    /// own bound.
    AtMost(u64),
    /// `shape == UnboundedSequence`: any length at all.
    Any,
}

impl LengthRule {
    fn check(self, len: usize) -> Result<(), String> {
        let len_u64 = u64::try_from(len).unwrap_or(u64::MAX);
        match self {
            Self::Exact(n) if len_u64 == n => Ok(()),
            Self::Exact(n) => Err(format!(
                "{n} element(s) (the array's own dimension), not {len}"
            )),
            Self::AtMost(n) if len_u64 <= n => Ok(()),
            Self::AtMost(n) => Err(format!(
                "at most {n} element(s) (the sequence's own bound), not {len}"
            )),
            Self::Any => Ok(()),
        }
    }
}

/// `text` — a Python tuple's `repr()`, see the module note — parsed
/// into `length.check()`-many elements of `kind_tag`'s kind.
/// `str_bound` bounds a `String8`/`String16` element the same way
/// [`super::zerodds_dyn_data_set_string`] bounds a scalar one (`0` is
/// unbounded).
///
/// # Errors
/// [`Err`] naming what did not parse, did not fit its kind's range, ran
/// past `str_bound`, or made the wrong number of elements.
pub(super) fn parse(
    kind_tag: ZddsDynKindTag,
    text: &str,
    str_bound: u64,
    length: LengthRule,
) -> Result<Vec<DynamicData>, String> {
    let tokens = split_tuple(text.trim())?;
    length.check(tokens.len())?;
    tokens
        .into_iter()
        .map(|token| parse_element(kind_tag, token, str_bound))
        .collect()
}

/// The parenthesised tuple's own elements, comma-split at the top
/// level only — a comma inside a quoted string does not end an
/// element.
fn split_tuple(text: &str) -> Result<Vec<&str>, String> {
    let Some(inner) = text.strip_prefix('(').and_then(|s| s.strip_suffix(')')) else {
        return Err(format!("`{text}` is not a parenthesised tuple"));
    };
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut tokens = Vec::new();
    let mut start = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, ch) in inner.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            ',' => {
                tokens.push(inner[start..i].trim());
                start = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    if quote.is_some() {
        return Err(format!("`{text}`: an unterminated quoted string"));
    }
    tokens.push(inner[start..].trim());
    // Python's own singleton-tuple spelling, `(x,)`: a trailing comma
    // with nothing after it, and only ever that one — a genuinely empty
    // element is never bare text (an empty string is `''`, two
    // characters, not zero), so a lone trailing empty token is always
    // this spelling and never a real, empty element.
    if tokens.len() == 2 && tokens.last() == Some(&"") {
        tokens.pop();
    }
    Ok(tokens)
}

fn parse_element(
    kind_tag: ZddsDynKindTag,
    token: &str,
    str_bound: u64,
) -> Result<DynamicData, String> {
    match kind_tag {
        ZddsDynKindTag::Boolean => match token {
            "True" => Ok(DynamicData::boolean(true)),
            "False" => Ok(DynamicData::boolean(false)),
            _ => Err(refuse(token, "`True` or `False`")),
        },
        ZddsDynKindTag::Byte => parse_ranged_int(token, 0, i128::from(u8::MAX))
            .map(|v| DynamicData::integer(PrimitiveKind::Octet, v)),
        ZddsDynKindTag::UInt8 => parse_ranged_int(token, 0, i128::from(u8::MAX))
            .map(|v| DynamicData::integer(PrimitiveKind::UInt8, v)),
        ZddsDynKindTag::Int8 => parse_ranged_int(token, i128::from(i8::MIN), i128::from(i8::MAX))
            .map(|v| DynamicData::integer(PrimitiveKind::Int8, v)),
        ZddsDynKindTag::Int16 => {
            parse_ranged_int(token, i128::from(i16::MIN), i128::from(i16::MAX))
                .map(|v| DynamicData::integer(PrimitiveKind::Int16, v))
        }
        ZddsDynKindTag::UInt16 => parse_ranged_int(token, 0, i128::from(u16::MAX))
            .map(|v| DynamicData::integer(PrimitiveKind::UInt16, v)),
        ZddsDynKindTag::Int32 => {
            parse_ranged_int(token, i128::from(i32::MIN), i128::from(i32::MAX))
                .map(|v| DynamicData::integer(PrimitiveKind::Int32, v))
        }
        ZddsDynKindTag::UInt32 => parse_ranged_int(token, 0, i128::from(u32::MAX))
            .map(|v| DynamicData::integer(PrimitiveKind::UInt32, v)),
        ZddsDynKindTag::Int64 => {
            parse_ranged_int(token, i128::from(i64::MIN), i128::from(i64::MAX))
                .map(|v| DynamicData::integer(PrimitiveKind::Int64, v))
        }
        ZddsDynKindTag::UInt64 => parse_ranged_int(token, 0, i128::from(u64::MAX))
            .map(|v| DynamicData::integer(PrimitiveKind::UInt64, v)),
        ZddsDynKindTag::Char8 => {
            let code = parse_ranged_int(token, 0, i128::from(u8::MAX))?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let ch = char::from(code as u8);
            Ok(DynamicData::character(PrimitiveKind::Char, ch))
        }
        ZddsDynKindTag::Char16 => {
            let code = parse_ranged_int(token, 0, i128::from(u16::MAX))?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let code_u32 = code as u32;
            let ch = char::from_u32(code_u32)
                .ok_or_else(|| refuse(token, "a UTF-16 code unit that is a code point"))?;
            Ok(DynamicData::character(PrimitiveKind::WChar, ch))
        }
        ZddsDynKindTag::Float32 => {
            let value: f32 = token
                .parse()
                .map_err(|_| refuse(token, "a `float32` literal"))?;
            Ok(DynamicData::float(PrimitiveKind::Float32, f64::from(value)))
        }
        ZddsDynKindTag::Float64 => {
            let value: f64 = token
                .parse()
                .map_err(|_| refuse(token, "a `float64` literal"))?;
            Ok(DynamicData::float(PrimitiveKind::Float64, value))
        }
        ZddsDynKindTag::Float128 => {
            let value: f64 = token
                .parse()
                .map_err(|_| refuse(token, "a `float128` literal"))?;
            Ok(DynamicData::LongDouble(Float128::from_f64(value)))
        }
        ZddsDynKindTag::String8 => {
            let text = parse_py_string(token)?;
            check_string_bound(token, text.chars().count(), str_bound)?;
            Ok(DynamicData::text(text))
        }
        ZddsDynKindTag::String16 => {
            let text = parse_py_string(token)?;
            check_string_bound(token, text.encode_utf16().count(), str_bound)?;
            Ok(DynamicData::text(text))
        }
        ZddsDynKindTag::Nested => Err(refuse(
            token,
            "a scalar/string element — a struct-typed array/sequence has no ROS default text to parse",
        )),
    }
}

fn check_string_bound(token: &str, len: usize, bound: u64) -> Result<(), String> {
    if bound != 0 && u64::try_from(len).unwrap_or(u64::MAX) > bound {
        return Err(refuse(
            token,
            &format!("a string of at most {bound} character(s)"),
        ));
    }
    Ok(())
}

fn parse_ranged_int(token: &str, min: i128, max: i128) -> Result<i128, String> {
    let value: i128 = token.parse().map_err(|_| refuse(token, "an integer"))?;
    if value < min || value > max {
        return Err(refuse(token, &format!("an integer in {min}..={max}")));
    }
    Ok(value)
}

/// One Python string literal — `'...'` or `"..."`, with Python's own
/// backslash escapes. Scanned from the front rather than stripped from
/// the back, so an escaped quote right before the true closing one is
/// never mistaken for it.
fn parse_py_string(token: &str) -> Result<String, String> {
    let mut chars = token.char_indices();
    let Some((_, quote)) = chars.next() else {
        return Err(refuse(token, "a quoted string"));
    };
    if quote != '\'' && quote != '"' {
        return Err(refuse(token, "a quoted string"));
    }
    let mut out = String::with_capacity(token.len());
    let mut closed = false;
    while let Some((_, ch)) = chars.next() {
        if ch == quote {
            closed = true;
            break;
        }
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some((_, escaped)) = chars.next() else {
            return Err(refuse(token, "a complete escape sequence"));
        };
        let resolved = match escaped {
            '\\' => '\\',
            '\'' => '\'',
            '"' => '"',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'a' => '\u{07}',
            'b' => '\u{08}',
            'f' => '\u{0C}',
            'v' => '\u{0B}',
            '0' => '\0',
            'x' => {
                let hex: String = (&mut chars).take(2).map(|(_, c)| c).collect();
                if hex.len() != 2 {
                    return Err(refuse(token, "two hex digits after `\\x`"));
                }
                let code = u8::from_str_radix(&hex, 16)
                    .map_err(|_| refuse(token, "two hex digits after `\\x`"))?;
                code as char
            }
            other => {
                return Err(refuse(
                    token,
                    &format!("a recognised escape (found `\\{other}`)"),
                ));
            }
        };
        out.push(resolved);
    }
    if !closed {
        return Err(refuse(token, "a closing quote"));
    }
    if chars.next().is_some() {
        return Err(refuse(token, "nothing after the closing quote"));
    }
    Ok(out)
}

fn refuse(token: &str, expected: &str) -> String {
    format!("`{token}` does not read as {expected}")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn ints(kind_tag: ZddsDynKindTag, text: &str, rule: LengthRule) -> Vec<i128> {
        parse(kind_tag, text, 0, rule)
            .unwrap_or_else(|error| panic!("{error}"))
            .into_iter()
            .map(|value| match value {
                DynamicData::Integer { value, .. } => value,
                other => panic!("not an integer: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn an_empty_tuple_is_no_elements() {
        assert_eq!(
            parse(ZddsDynKindTag::Int32, "()", 0, LengthRule::Any).unwrap(),
            Vec::new()
        );
    }

    #[test]
    fn a_singleton_tuple_is_one_element_not_two() {
        assert_eq!(
            ints(ZddsDynKindTag::Int32, "(7,)", LengthRule::Exact(1)),
            vec![7]
        );
    }

    #[test]
    fn every_integer_kind_reads_its_own_range() {
        assert_eq!(
            ints(
                ZddsDynKindTag::Int32,
                "(0, 2147483647, -2147483648)",
                LengthRule::Exact(3)
            ),
            vec![0, 2_147_483_647, -2_147_483_648]
        );
        assert_eq!(
            ints(
                ZddsDynKindTag::UInt64,
                "(0, 1, 18446744073709551615)",
                LengthRule::Any
            ),
            vec![0, 1, 18_446_744_073_709_551_615]
        );
        // `byte` reads the same unsigned 8-bit range as `uint8` — see
        // the module note. `char8`'s own text reads the same range too,
        // as the code point it names; that becomes a `Character`, not
        // an `Integer`, and is [`a_char8_integer_becomes_the_code_point_it_names`]'s.
        assert_eq!(
            ints(ZddsDynKindTag::Byte, "(0, 1, 255)", LengthRule::Any),
            vec![0, 1, 255]
        );
    }

    #[test]
    fn an_out_of_range_integer_is_refused() {
        assert!(parse(ZddsDynKindTag::UInt8, "(0, 256)", 0, LengthRule::Any).is_err());
        assert!(parse(ZddsDynKindTag::Int8, "(-129,)", 0, LengthRule::Any).is_err());
    }

    #[test]
    fn a_char8_integer_becomes_the_code_point_it_names() {
        let elements = parse(ZddsDynKindTag::Char8, "(0, 65, 127)", 0, LengthRule::Any).unwrap();
        let chars: Vec<char> = elements
            .into_iter()
            .map(|value| match value {
                DynamicData::Character { value, .. } => {
                    value.chars().next().expect("one character")
                }
                other => panic!("not a character: {other:?}"),
            })
            .collect();
        assert_eq!(chars, vec!['\0', 'A', '\u{7f}']);
    }

    #[test]
    fn floats_read_decimal_scientific_and_the_three_special_spellings() {
        let elements = parse(
            ZddsDynKindTag::Float64,
            "(0.0, -1.125, 1e+20, inf, -inf, nan)",
            0,
            LengthRule::Any,
        )
        .unwrap();
        let values: Vec<f64> = elements
            .into_iter()
            .map(|value| match value {
                DynamicData::Float { value, .. } => value,
                other => panic!("not a float: {other:?}"),
            })
            .collect();
        assert_eq!(values[0], 0.0);
        assert_eq!(values[1], -1.125);
        assert_eq!(values[2], 1e20);
        assert!(values[3].is_infinite() && values[3] > 0.0);
        assert!(values[4].is_infinite() && values[4] < 0.0);
        assert!(values[5].is_nan());
    }

    #[test]
    fn a_singleton_float_matches_arrays_msg_float32_values_default_shape() {
        // `float32[3] float32_values_default [1.125, 0.0, -1.125]` —
        // test_msgs/msg/Arrays.msg's own real declaration.
        let elements = parse(
            ZddsDynKindTag::Float32,
            "(1.125, 0.0, -1.125)",
            0,
            LengthRule::Exact(3),
        )
        .unwrap();
        let values: Vec<f64> = elements
            .into_iter()
            .map(|value| match value {
                DynamicData::Float { value, .. } => value,
                other => panic!("not a float: {other:?}"),
            })
            .collect();
        assert_eq!(values, vec![1.125, 0.0, -1.125]);
    }

    #[test]
    fn strings_read_quotes_and_escapes_on_either_side() {
        let elements = parse(
            ZddsDynKindTag::String8,
            r#"('', 'max value', 'it\'s', "she said \"hi\"")"#,
            0,
            LengthRule::Any,
        )
        .unwrap();
        let texts: Vec<String> = elements
            .into_iter()
            .map(|value| match value {
                DynamicData::Text(text) => text,
                other => panic!("not text: {other:?}"),
            })
            .collect();
        assert_eq!(
            texts,
            vec![
                String::new(),
                "max value".to_string(),
                "it's".to_string(),
                "she said \"hi\"".to_string(),
            ]
        );
    }

    #[test]
    fn arrays_msg_string_values_default_reads_exactly() {
        // `string[3] string_values_default ["", "max value", "min value"]`.
        let elements = parse(
            ZddsDynKindTag::String8,
            "('', 'max value', 'min value')",
            0,
            LengthRule::Exact(3),
        )
        .unwrap();
        let texts: Vec<String> = elements
            .into_iter()
            .map(|value| match value {
                DynamicData::Text(text) => text,
                other => panic!("not text: {other:?}"),
            })
            .collect();
        assert_eq!(texts, vec!["", "max value", "min value"]);
    }

    #[test]
    fn a_string_past_its_bound_is_refused() {
        assert!(parse(ZddsDynKindTag::String8, "('too long',)", 3, LengthRule::Any).is_err());
        assert!(parse(ZddsDynKindTag::String8, "('ok',)", 3, LengthRule::Any).is_ok());
    }

    #[test]
    fn booleans_read_pythons_capitalisation() {
        let elements = parse(
            ZddsDynKindTag::Boolean,
            "(False, True, False)",
            0,
            LengthRule::Exact(3),
        )
        .unwrap();
        let flags: Vec<bool> = elements
            .into_iter()
            .map(|value| match value {
                DynamicData::Boolean(flag) => flag,
                other => panic!("not a boolean: {other:?}"),
            })
            .collect();
        assert_eq!(flags, vec![false, true, false]);
    }

    #[test]
    fn a_float128_element_becomes_a_long_double() {
        let elements = parse(ZddsDynKindTag::Float128, "(3.25,)", 0, LengthRule::Any).unwrap();
        let DynamicData::LongDouble(value) = &elements[0] else {
            panic!("not a long double: {:?}", elements[0]);
        };
        assert_eq!(*value, Float128::from_f64(3.25));
    }

    #[test]
    fn a_non_tuple_text_is_refused() {
        assert!(parse(ZddsDynKindTag::Int32, "[1, 2, 3]", 0, LengthRule::Any).is_err());
        assert!(parse(ZddsDynKindTag::Int32, "1, 2, 3", 0, LengthRule::Any).is_err());
    }

    #[test]
    fn an_unterminated_string_is_refused() {
        assert!(
            parse(
                ZddsDynKindTag::String8,
                "('unterminated,)",
                0,
                LengthRule::Any
            )
            .is_err()
        );
    }

    #[test]
    fn an_unrecognised_escape_is_refused() {
        assert!(parse(ZddsDynKindTag::String8, r"('\q',)", 0, LengthRule::Any).is_err());
    }

    #[test]
    fn the_wrong_element_count_is_refused_for_a_fixed_array() {
        assert!(parse(ZddsDynKindTag::Int32, "(1, 2)", 0, LengthRule::Exact(3)).is_err());
        assert!(
            parse(
                ZddsDynKindTag::Int32,
                "(1, 2, 3, 4)",
                0,
                LengthRule::Exact(3)
            )
            .is_err()
        );
    }

    #[test]
    fn a_bounded_sequence_accepts_at_or_under_its_bound_and_no_more() {
        assert!(parse(ZddsDynKindTag::Int32, "(1, 2, 3)", 0, LengthRule::AtMost(3)).is_ok());
        assert!(parse(ZddsDynKindTag::Int32, "(1, 2)", 0, LengthRule::AtMost(3)).is_ok());
        assert!(
            parse(
                ZddsDynKindTag::Int32,
                "(1, 2, 3, 4)",
                0,
                LengthRule::AtMost(3)
            )
            .is_err()
        );
    }

    #[test]
    fn a_comma_inside_a_quoted_string_does_not_split_the_element() {
        let elements = parse(
            ZddsDynKindTag::String8,
            "('a, b', 'c')",
            0,
            LengthRule::Exact(2),
        )
        .unwrap();
        let texts: Vec<String> = elements
            .into_iter()
            .map(|value| match value {
                DynamicData::Text(text) => text,
                other => panic!("not text: {other:?}"),
            })
            .collect();
        assert_eq!(texts, vec!["a, b".to_string(), "c".to_string()]);
    }
}
