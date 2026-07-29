// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Static KeyHolder max-size analysis (XTypes 1.3 §7.6.8.4 step 5).
//!
//! The KeyHash branch is decided **statically per topic type**: if the maximum
//! serialized size of the `@key` members (PLAIN_CDR2-BE, max align 4, member-id
//! order) is ≤ 16 bytes, the hash is those bytes zero-padded to 16; otherwise it
//! is `MD5(bytes)[0..16]`. This module computes that maximum so every idl-*
//! backend takes the same branch as the runtime [`compute_key_hash`] — i.e.
//! byte-identically to the Rust reference.
//!
//! Returns `None` for a dynamically sized KeyHolder (unbounded string, sequence,
//! map, …) — the MD5 branch.
//!
//! [`compute_key_hash`]: <crate reference `zerodds_cdr::compute_key_hash`>

use std::collections::HashMap;

use crate::ast::types::{
    ConstExpr, Declarator, IntegerType, Literal, LiteralKind, Member, PrimitiveType, StructDef,
    TypeSpec, UnaryOp,
};

/// Wire length of a KeyHash (spec: 16 bytes). The zero-pad/MD5 branch pivots here.
pub const KEY_HASH_LEN: usize = 16;

/// Evaluates a `const_expr` array bound / string bound to a `usize` (decimal or
/// `0x` literal, with a leading unary sign/complement). `None` if non-constant.
/// zerodds-lint: recursion-depth 32
fn const_usize(e: &ConstExpr) -> Option<usize> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => {
            let t = raw.trim();
            let v = if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                i64::from_str_radix(hex, 16).ok()?
            } else {
                t.parse::<i64>().ok()?
            };
            usize::try_from(v).ok()
        }
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_usize(operand)?;
            match op {
                UnaryOp::Plus => Some(v),
                // coverage: justified — a unary minus/complement on an array or
                // string bound is a negative/invalid size; the parser/validator
                // rejects it before KeyHolder analysis, so this is unreachable
                // for a well-formed spec.
                _ => None,
            }
        }
        // coverage: justified — a non-literal, non-unary const-expr bound (named
        // const, expression) is not a fixed size; unreachable for the `@key`
        // array/string bounds that reach this analysis.
        _ => None,
    }
}

/// Wire size of an IDL primitive as the thin backends serialize it (max align 4
/// applies separately). `wchar` is a `wchar32` (4 bytes) in the ZeroDDS bindings.
fn primitive_wire_size(p: PrimitiveType) -> usize {
    match p {
        PrimitiveType::Integer(i) => match i {
            IntegerType::Int8 | IntegerType::UInt8 => 1,
            IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
                2
            }
            IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => 4,
            IntegerType::LongLong
            | IntegerType::ULongLong
            | IntegerType::Int64
            | IntegerType::UInt64 => 8,
        },
        PrimitiveType::Floating(crate::ast::types::FloatingType::Float) => 4,
        PrimitiveType::Floating(crate::ast::types::FloatingType::Double) => 8,
        PrimitiveType::Floating(crate::ast::types::FloatingType::LongDouble) => 16,
        PrimitiveType::Char | PrimitiveType::Boolean | PrimitiveType::Octet => 1,
        PrimitiveType::WideChar => 4,
    }
}

/// zerodds-lint: recursion-depth 32
fn resolve<'a>(t: &'a TypeSpec, typedefs: &'a HashMap<String, TypeSpec>) -> TypeSpec {
    match t {
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            match typedefs.get(&name) {
                Some(u) => resolve(u, typedefs),
                None => t.clone(),
            }
        }
        other => other.clone(),
    }
}

fn is_key(m: &Member) -> bool {
    crate::semantics::annotations::lower_annotations(&m.annotations)
        .map(|l| l.has_key())
        .unwrap_or(false)
}

fn member_id(m: &Member, fallback: u32) -> u32 {
    crate::semantics::annotations::lower_annotations(&m.annotations)
        .ok()
        .and_then(|l| l.explicit_id())
        .unwrap_or(fallback)
}

/// Advances the KeyHolder byte offset by one occurrence of a member of type
/// `spec`, applying the BE PLAIN_CDR2 alignment (max align 4) before the value.
/// Recurses into nested `@key` structs. `None` for a dynamically sized type.
///
/// zerodds-lint: recursion-depth 16 (nested `@key` struct expansion; bounded by
/// the IDL's aggregate nesting depth).
fn atom_size(
    spec: &TypeSpec,
    offset: usize,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Option<usize> {
    let pad_to = |off: usize, align: usize| off + (align - (off % align)) % align;
    match resolve(spec, typedefs) {
        TypeSpec::Primitive(p) => {
            let size = primitive_wire_size(p);
            let align = size.clamp(1, 4);
            Some(pad_to(offset, align) + size)
        }
        TypeSpec::String(s) => {
            // Bounded string: 4-byte length prefix + N+1 (narrow) / 2N (wide);
            // unbounded → dynamically sized → MD5.
            let n = const_usize(s.bound.as_ref()?)?;
            let body = if s.wide { 2 * n } else { n + 1 };
            Some(pad_to(offset, 4) + 4 + body)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone())?;
            let sd = structs.get(&name)?;
            Some(struct_offset(&sd.members, offset, structs, typedefs)?)
        }
        // sequence / fixed-point / map / any → dynamically sized → MD5.
        _ => None,
    }
}

/// Accumulates the offset over an aggregate's KeyHolder members: its `@key`
/// members, or ALL members if none are keyed (XTypes 1.3 §7.6.8), in member-id
/// order.
fn struct_offset(
    members: &[Member],
    start: usize,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Option<usize> {
    let keyed: Vec<&Member> = members.iter().filter(|m| is_key(m)).collect();
    let effective: Vec<&Member> = if keyed.is_empty() {
        members.iter().collect()
    } else {
        keyed
    };
    offset_over(&effective, start, structs, typedefs)
}

fn offset_over(
    members: &[&Member],
    start: usize,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Option<usize> {
    let mut ordered: Vec<(u32, &&Member)> = members
        .iter()
        .enumerate()
        .map(|(i, m)| (member_id(m, i as u32), m))
        .collect();
    ordered.sort_by_key(|(id, _)| *id);
    let mut off = start;
    for (_, m) in &ordered {
        for decl in &m.declarators {
            let count = match decl {
                Declarator::Simple(_) => 1usize,
                Declarator::Array(arr) => {
                    let mut n = 1usize;
                    for e in &arr.sizes {
                        n = n.checked_mul(const_usize(e)?)?;
                    }
                    n
                }
            };
            for _ in 0..count {
                off = atom_size(&m.type_spec, off, structs, typedefs)?;
            }
        }
    }
    Some(off)
}

/// Maximum serialized size of the KeyHolder built from `key_members` (already
/// the struct's `@key` members). `Some(n)` with `n <= 16` → zero-pad branch;
/// `Some(n)` with `n > 16` or `None` → MD5 branch.
#[must_use]
pub fn key_holder_max_size(
    key_members: &[&Member],
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Option<usize> {
    offset_over(key_members, 0, structs, typedefs)
}

/// Convenience: `true` when the KeyHash must use the MD5 branch (max size > 16
/// or dynamically sized).
#[must_use]
pub fn uses_md5(
    key_members: &[&Member],
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> bool {
    match key_holder_max_size(key_members, structs, typedefs) {
        Some(n) => n > KEY_HASH_LEN,
        None => true,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::ast::types::{ConstrTypeDecl, Definition, StructDcl, TypeDecl};
    use crate::config::ParserConfig;

    /// Parses `idl`, builds the struct/typedef context like the emitters do, and
    /// returns the KeyHolder max size of the named struct's `@key` members.
    fn size(idl: &str, ty: &str) -> Option<usize> {
        let spec = crate::parse(idl, &ParserConfig::default()).expect("parse");
        let structs: HashMap<String, &StructDef> = spec
            .definitions
            .iter()
            .filter_map(|d| match d {
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                    Some((s.name.text.clone(), s))
                }
                _ => None,
            })
            .collect();
        let mut typedefs: HashMap<String, TypeSpec> = HashMap::new();
        for d in &spec.definitions {
            if let Definition::Type(TypeDecl::Typedef(td)) = d {
                for decl in &td.declarators {
                    if let Declarator::Simple(name) = decl {
                        typedefs.insert(name.text.clone(), td.type_spec.clone());
                    }
                }
            }
        }
        let sd = structs.get(ty).expect("struct not found");
        let keys: Vec<&Member> = sd.members.iter().filter(|m| is_key(m)).collect();
        key_holder_max_size(&keys, &structs, &typedefs)
    }

    fn md5(idl: &str, ty: &str) -> bool {
        let spec = crate::parse(idl, &ParserConfig::default()).expect("parse");
        let structs: HashMap<String, &StructDef> = spec
            .definitions
            .iter()
            .filter_map(|d| match d {
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                    Some((s.name.text.clone(), s))
                }
                _ => None,
            })
            .collect();
        let sd = structs.get(ty).expect("struct");
        let keys: Vec<&Member> = sd.members.iter().filter(|m| is_key(m)).collect();
        uses_md5(&keys, &structs, &HashMap::new())
    }

    #[test]
    fn primitive_sizes_and_alignment() {
        // long is 4-aligned: 0→4, 4→8.
        assert_eq!(
            size("@final struct K { @key long a; @key long b; };", "K"),
            Some(8)
        );
        // octet(1) then long(4-aligned): 0→1, pad→4→8.
        assert_eq!(
            size("@final struct K { @key octet a; @key long b; };", "K"),
            Some(8)
        );
        // 4× long = exactly 16 (the zero-pad boundary).
        assert_eq!(
            size(
                "@final struct K { @key long a; @key long b; @key long c; @key long d; };",
                "K"
            ),
            Some(16)
        );
    }

    #[test]
    fn boundary_16_zero_pad_17_md5() {
        // ≤16 → zero-pad, >16 → MD5.
        assert!(!md5(
            "@final struct K { @key long a; @key long b; @key long c; @key long d; };",
            "K"
        ));
        assert!(md5(
            "@final struct K { @key long a; @key long b; @key long c; @key long d; @key long e; };",
            "K"
        ));
    }

    #[test]
    fn member_id_order_changes_offset() {
        // Declaration order octet,long → 8; member-id order long(0),octet(1) → 5.
        assert_eq!(
            size(
                "@final struct K { @id(1) @key octet a; @id(0) @key long b; };",
                "K"
            ),
            Some(5)
        );
    }

    #[test]
    fn bounded_string() {
        // narrow string<8>: 4 (len prefix) + 8 + 1 (nul) = 13.
        assert_eq!(
            size("@final struct K { @key string<8> s; };", "K"),
            Some(13)
        );
        // string<20>: 4 + 21 = 25 > 16 → MD5.
        assert!(md5("@final struct K { @key string<20> s; };", "K"));
    }

    #[test]
    fn wide_bounded_string() {
        // wstring<8>: 4 + 2*8 = 20.
        assert_eq!(
            size("@final struct K { @key wstring<8> s; };", "K"),
            Some(20)
        );
    }

    #[test]
    fn unbounded_string_seq_map_are_dynamic() {
        assert_eq!(size("@final struct K { @key string s; };", "K"), None);
        assert_eq!(
            size("@final struct K { @key sequence<long> s; };", "K"),
            None
        );
        assert_eq!(
            size("@final struct K { @key map<long, long> m; };", "K"),
            None
        );
    }

    #[test]
    fn fixed_arrays() {
        // long a[5] = 5 occurrences → 20.
        assert_eq!(size("@final struct K { @key long a[5]; };", "K"), Some(20));
        // multi-dim short m[2][2] = 4 occurrences of short(2) → 8.
        assert_eq!(
            size("@final struct K { @key short m[2][2]; };", "K"),
            Some(8)
        );
        // hex array bound.
        assert_eq!(
            size("@final struct K { @key long a[0x5]; };", "K"),
            Some(20)
        );
    }

    #[test]
    fn nested_key_struct_expands() {
        let idl = "@final struct Inner { @key long x; @key long y; };\n\
                   @final struct Outer { @key Inner i; @key long z; };";
        // Inner keys x,y → 8; then z → 12.
        assert_eq!(size(idl, "Outer"), Some(12));
    }

    #[test]
    fn nested_struct_without_keys_uses_all_members() {
        let idl = "@final struct Pair { long a; long b; };\n\
                   @final struct Holder { @key Pair p; };";
        // Pair has no @key → all members a,b → 8.
        assert_eq!(size(idl, "Holder"), Some(8));
    }

    #[test]
    fn typedef_resolves_to_underlying() {
        let idl = "typedef long Score;\n@final struct K { @key Score a; @key Score b; };";
        assert_eq!(size(idl, "K"), Some(8));
    }

    #[test]
    fn long_double_is_16_bytes() {
        assert_eq!(
            size("@final struct K { @key long double d; };", "K"),
            Some(16)
        );
        // two long doubles: 16, pad→16→32 → MD5.
        assert!(md5(
            "@final struct K { @key long double d; @key long double e; };",
            "K"
        ));
    }

    #[test]
    fn scoped_enum_key_is_dynamic_like_rust_ref() {
        // A non-struct scoped @key (enum) is not resolved to a struct → None
        // (MD5), matching the Rust reference's KeyHolder analysis.
        let idl = "enum E { A, B };\n@final struct K { @key E e; };";
        assert_eq!(size(idl, "K"), None);
    }

    #[test]
    fn all_primitive_wire_sizes() {
        // Covers every primitive_wire_size arm (int8/int64/float/double/char/
        // wchar/boolean), each aligned to min(size, 4) at offset 0.
        assert_eq!(size("@final struct K { @key int8 a; };", "K"), Some(1));
        assert_eq!(size("@final struct K { @key int64 a; };", "K"), Some(8));
        assert_eq!(size("@final struct K { @key float a; };", "K"), Some(4));
        assert_eq!(size("@final struct K { @key double a; };", "K"), Some(8));
        assert_eq!(size("@final struct K { @key char a; };", "K"), Some(1));
        assert_eq!(size("@final struct K { @key wchar a; };", "K"), Some(4));
        assert_eq!(size("@final struct K { @key boolean a; };", "K"), Some(1));
    }

    #[test]
    fn uses_md5_none_and_over_16() {
        // uses_md5 on a dynamically sized key (unbounded string → None → MD5) and
        // on an enum-containing spec (exercises the non-struct filter arm).
        assert!(md5("@final struct K { @key string s; };", "K"));
        assert!(md5(
            "enum E { A };\n@final struct K { @key long a; @key long b; @key long c; @key long d; @key long e; };",
            "K"
        ));
    }

    #[test]
    fn unary_plus_array_bound() {
        // `[+3]` exercises the ConstExpr::Unary(Plus) path in const_usize.
        assert_eq!(size("@final struct K { @key long a[+3]; };", "K"), Some(12));
    }
}
