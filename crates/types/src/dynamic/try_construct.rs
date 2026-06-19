// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XTypes 1.3 §7.5.4.1.2 — TryConstruct-Apply (C4.7).
//!
//! When a value does not fit the target member on decode or a setter
//! call (e.g. a string longer than the bound, a sequence over its
//! max length, an enum value outside the value range), the
//! `try_construct` strategy decides what happens:
//!
//! - `Discard` — discard the value, the member stays unset.
//! - `UseDefault` — ignore the value, set `member.default_value`.
//! - `Trim` — truncate to the bound (strings + sequences); for other
//!   bound violations fall back to discard.
//!
//! This logic is evaluated **only** when a bound violation actually
//! exists — un-bounded setters (member type without a `bound` limit)
//! stay unchanged.

use alloc::string::ToString;
use alloc::vec::Vec;

use super::data::DynamicValue;
use super::descriptor::{TryConstructKind, TypeKind};
use super::type_::DynamicTypeMember;

/// Result of a try-construct evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum TryConstructOutcome {
    /// The value is within bounds — `set` may write it unchanged.
    Accept(DynamicValue),
    /// The value is discarded — the member stays unset.
    Discard,
    /// The value is replaced by the default_value.
    UseDefault(DynamicValue),
    /// The value is truncated to the bound.
    Trim(DynamicValue),
}

/// Applies the `try_construct` strategy to a setter value.
/// If there is no bound violation, the function returns
/// `Accept(value)` unchanged.
#[must_use]
pub fn apply_try_construct(member: &DynamicTypeMember, value: DynamicValue) -> TryConstructOutcome {
    let descriptor = member.descriptor();
    let bound_max = bound_max_length(member);
    let target_kind = member.dynamic_type().kind();

    let violation = detect_violation(&value, target_kind, bound_max);
    if violation.is_none() {
        return TryConstructOutcome::Accept(value);
    }

    match descriptor.try_construct {
        TryConstructKind::Discard => TryConstructOutcome::Discard,
        TryConstructKind::UseDefault => {
            match parse_default(descriptor.default_value.as_deref(), target_kind) {
                Some(default) => TryConstructOutcome::UseDefault(default),
                None => TryConstructOutcome::Discard,
            }
        }
        TryConstructKind::Trim => match trim_value(value, target_kind, bound_max) {
            Some(trimmed) => TryConstructOutcome::Trim(trimmed),
            None => TryConstructOutcome::Discard,
        },
    }
}

/// Returns the `max_length` bound from the member type, if relevant.
/// `0` as a value (spec §7.5.1.2.4: 0 = unbounded) is treated as
/// `None` — un-bounded setters bypass the apply logic.
fn bound_max_length(member: &DynamicTypeMember) -> Option<usize> {
    let descriptor = member.dynamic_type().descriptor();
    match descriptor.kind {
        TypeKind::String8 | TypeKind::String16 | TypeKind::Sequence | TypeKind::Map => descriptor
            .bound
            .first()
            .copied()
            .filter(|&b| b != 0)
            .map(|b| b as usize),
        TypeKind::Array => {
            // An array has fixed dimensions — the bound is the product of all dims.
            if descriptor.bound.is_empty() {
                None
            } else {
                Some(descriptor.bound.iter().product::<u32>() as usize)
            }
        }
        _ => None,
    }
}

#[derive(Debug, PartialEq)]
enum Violation {
    StringTooLong,
    SequenceTooLong,
    ArrayLengthMismatch,
}

fn detect_violation(
    value: &DynamicValue,
    target_kind: TypeKind,
    bound_max: Option<usize>,
) -> Option<Violation> {
    let max = bound_max?;
    match (value, target_kind) {
        (DynamicValue::String(s), TypeKind::String8) if s.len() > max => {
            Some(Violation::StringTooLong)
        }
        (DynamicValue::WString(s), TypeKind::String16) if s.len() > max => {
            Some(Violation::StringTooLong)
        }
        (DynamicValue::Sequence(s), TypeKind::Sequence) if s.len() > max => {
            Some(Violation::SequenceTooLong)
        }
        (DynamicValue::Sequence(s), TypeKind::Array) if s.len() != max => {
            Some(Violation::ArrayLengthMismatch)
        }
        _ => None,
    }
}

fn trim_value(
    value: DynamicValue,
    target_kind: TypeKind,
    bound_max: Option<usize>,
) -> Option<DynamicValue> {
    let max = bound_max?;
    match (value, target_kind) {
        (DynamicValue::String(mut s), TypeKind::String8) => {
            // String trim on a byte boundary, but never in the middle of
            // a UTF-8 codepoint. We coerce to the next-smaller char
            // boundary.
            if s.len() > max {
                let mut cut = max;
                while !s.is_char_boundary(cut) && cut > 0 {
                    cut -= 1;
                }
                s.truncate(cut);
            }
            Some(DynamicValue::String(s))
        }
        (DynamicValue::WString(mut s), TypeKind::String16) => {
            if s.len() > max {
                s.truncate(max);
            }
            Some(DynamicValue::WString(s))
        }
        (DynamicValue::Sequence(mut s), TypeKind::Sequence) => {
            if s.len() > max {
                s.truncate(max);
            }
            Some(DynamicValue::Sequence(s))
        }
        // Array length mismatch: no meaningful trim, because an array
        // has an exact dimension. Fall back to discard.
        _ => None,
    }
}

fn parse_default(default_str: Option<&str>, kind: TypeKind) -> Option<DynamicValue> {
    let s = default_str?;
    match kind {
        TypeKind::Boolean => match s {
            "TRUE" | "true" | "1" => Some(DynamicValue::Bool(true)),
            "FALSE" | "false" | "0" => Some(DynamicValue::Bool(false)),
            _ => None,
        },
        TypeKind::Byte | TypeKind::UInt8 => s.parse::<u8>().ok().map(DynamicValue::UInt8),
        TypeKind::Int8 => s.parse::<i8>().ok().map(DynamicValue::Int8),
        TypeKind::Int16 => s.parse::<i16>().ok().map(DynamicValue::Int16),
        TypeKind::UInt16 => s.parse::<u16>().ok().map(DynamicValue::UInt16),
        TypeKind::Int32 | TypeKind::Enumeration => s.parse::<i32>().ok().map(DynamicValue::Int32),
        TypeKind::UInt32 => s.parse::<u32>().ok().map(DynamicValue::UInt32),
        TypeKind::Int64 => s.parse::<i64>().ok().map(DynamicValue::Int64),
        TypeKind::UInt64 => s.parse::<u64>().ok().map(DynamicValue::UInt64),
        TypeKind::Float32 => s.parse::<f32>().ok().map(DynamicValue::Float32),
        TypeKind::Float64 => s.parse::<f64>().ok().map(DynamicValue::Float64),
        TypeKind::String8 => Some(DynamicValue::String(s.to_string())),
        TypeKind::String16 => Some(DynamicValue::WString(s.encode_utf16().collect::<Vec<_>>())),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::dynamic::builder::{DynamicTypeBuilder, DynamicTypeBuilderFactory};
    use crate::dynamic::descriptor::{MemberDescriptor, TypeDescriptor};
    use alloc::boxed::Box;

    fn make_struct_with_bounded_string(
        max_len: u32,
        try_construct: TryConstructKind,
        default_value: Option<&str>,
    ) -> crate::dynamic::DynamicType {
        let mut builder = DynamicTypeBuilder::new(TypeDescriptor::structure("TestStruct"));
        let mut string_desc = TypeDescriptor::primitive(TypeKind::String8, "string");
        string_desc.bound = alloc::vec![max_len];
        let mut member = MemberDescriptor::new("name", 1, string_desc);
        member.try_construct = try_construct;
        member.default_value = default_value.map(|s| s.to_string());
        builder.add_member(member).unwrap();
        builder.build().unwrap()
    }

    fn make_struct_with_bounded_seq(
        max_len: u32,
        try_construct: TryConstructKind,
    ) -> crate::dynamic::DynamicType {
        let mut builder = DynamicTypeBuilder::new(TypeDescriptor::structure("TestSeq"));
        let mut seq_desc = TypeDescriptor::primitive(TypeKind::Sequence, "sequence");
        seq_desc.bound = alloc::vec![max_len];
        seq_desc.element_type = Some(Box::new(TypeDescriptor::primitive(TypeKind::Int32, "long")));
        let mut member = MemberDescriptor::new("ids", 1, seq_desc);
        member.try_construct = try_construct;
        builder.add_member(member).unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn discard_drops_too_long_string() {
        let ty = make_struct_with_bounded_string(5, TryConstructKind::Discard, None);
        let member = ty.member_by_id(1).unwrap();
        let outcome = apply_try_construct(member, DynamicValue::String("toolong".into()));
        assert_eq!(outcome, TryConstructOutcome::Discard);
    }

    #[test]
    fn use_default_replaces_too_long_string() {
        let ty = make_struct_with_bounded_string(5, TryConstructKind::UseDefault, Some("hello"));
        let member = ty.member_by_id(1).unwrap();
        let outcome = apply_try_construct(member, DynamicValue::String("toolong".into()));
        match outcome {
            TryConstructOutcome::UseDefault(DynamicValue::String(s)) => assert_eq!(s, "hello"),
            other => panic!("expected UseDefault(\"hello\"), got {other:?}"),
        }
    }

    #[test]
    fn use_default_falls_back_to_discard_when_no_default() {
        let ty = make_struct_with_bounded_string(5, TryConstructKind::UseDefault, None);
        let member = ty.member_by_id(1).unwrap();
        let outcome = apply_try_construct(member, DynamicValue::String("toolong".into()));
        assert_eq!(outcome, TryConstructOutcome::Discard);
    }

    #[test]
    fn trim_truncates_string_to_bound() {
        let ty = make_struct_with_bounded_string(5, TryConstructKind::Trim, None);
        let member = ty.member_by_id(1).unwrap();
        let outcome = apply_try_construct(member, DynamicValue::String("hello world".into()));
        match outcome {
            TryConstructOutcome::Trim(DynamicValue::String(s)) => assert_eq!(s, "hello"),
            other => panic!("expected Trim(\"hello\"), got {other:?}"),
        }
    }

    #[test]
    fn trim_respects_utf8_codepoint_boundaries() {
        // "héllo" with é = 2 bytes. Bound 3 would trim in the middle of é →
        // must fall back to 2 bytes ("h" + start of é → boundary 1 → "h").
        let ty = make_struct_with_bounded_string(3, TryConstructKind::Trim, None);
        let member = ty.member_by_id(1).unwrap();
        let outcome = apply_try_construct(member, DynamicValue::String("héllo".into()));
        match outcome {
            TryConstructOutcome::Trim(DynamicValue::String(s)) => {
                assert!(s.is_char_boundary(s.len()));
                assert!(s.len() <= 3);
                // Mit "h" (1 byte) + é (2 byte) = 3 byte char-boundary.
                assert_eq!(s, "hé");
            }
            other => panic!("expected Trim, got {other:?}"),
        }
    }

    #[test]
    fn accept_when_value_within_bound() {
        let ty = make_struct_with_bounded_string(10, TryConstructKind::Discard, None);
        let member = ty.member_by_id(1).unwrap();
        let outcome = apply_try_construct(member, DynamicValue::String("ok".into()));
        match outcome {
            TryConstructOutcome::Accept(DynamicValue::String(s)) => assert_eq!(s, "ok"),
            other => panic!("expected Accept, got {other:?}"),
        }
    }

    #[test]
    fn unbounded_string_always_accepts() {
        let ty = make_struct_with_bounded_string(0, TryConstructKind::Discard, None);
        let member = ty.member_by_id(1).unwrap();
        // bound = 0 = unbounded → no violation, no apply.
        let outcome =
            apply_try_construct(member, DynamicValue::String("a-very-long-string".into()));
        match outcome {
            TryConstructOutcome::Accept(_) => {}
            other => panic!("expected Accept (unbounded), got {other:?}"),
        }
    }

    #[test]
    fn discard_drops_too_long_sequence() {
        let ty = make_struct_with_bounded_seq(3, TryConstructKind::Discard);
        let member = ty.member_by_id(1).unwrap();
        let elements: Vec<_> = (0..5)
            .map(|i| {
                let prim = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
                let mut d = crate::dynamic::DynamicData::new(prim.clone());
                d.set_int32_value(0, i).ok();
                d
            })
            .collect();
        let outcome = apply_try_construct(member, DynamicValue::Sequence(elements));
        assert_eq!(outcome, TryConstructOutcome::Discard);
    }

    #[test]
    fn trim_truncates_sequence_to_bound() {
        let ty = make_struct_with_bounded_seq(3, TryConstructKind::Trim);
        let member = ty.member_by_id(1).unwrap();
        let elements: Vec<_> = (0..5)
            .map(|i| {
                let prim = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
                let mut d = crate::dynamic::DynamicData::new(prim.clone());
                d.set_int32_value(0, i).ok();
                d
            })
            .collect();
        let outcome = apply_try_construct(member, DynamicValue::Sequence(elements));
        match outcome {
            TryConstructOutcome::Trim(DynamicValue::Sequence(s)) => assert_eq!(s.len(), 3),
            other => panic!("expected Trim(seq[3]), got {other:?}"),
        }
    }

    #[test]
    fn parse_default_int32_works() {
        let v = parse_default(Some("42"), TypeKind::Int32);
        assert_eq!(v, Some(DynamicValue::Int32(42)));
    }

    #[test]
    fn parse_default_bool_accepts_canonical_forms() {
        assert_eq!(
            parse_default(Some("TRUE"), TypeKind::Boolean),
            Some(DynamicValue::Bool(true))
        );
        assert_eq!(
            parse_default(Some("false"), TypeKind::Boolean),
            Some(DynamicValue::Bool(false))
        );
    }

    #[test]
    fn parse_default_invalid_returns_none() {
        assert_eq!(parse_default(Some("not-a-number"), TypeKind::Int32), None);
    }
}
