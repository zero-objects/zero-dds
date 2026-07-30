// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Central resolved member-id derivation (broad-audit P0-3).
//!
//! Member IDs for `@autoid(HASH)` / `@hashid` / `@id` are computed HERE, once,
//! and every backend consumes the finished ID — no backend re-implements the
//! precedence or the hash. This is the single source of truth the resolved
//! specification carries per member (audit goal: `resolved_member_id` lives in
//! the semantic layer, not in 17 emitters).
//!
//! Precedence, highest first (XTypes 1.3 §7.3.1.2.1):
//! 1. explicit `@id(n)` — the literal value.
//! 2. `@hashid("hint")` — `NameHash` of the hint (a bare `@hashid` hashes the
//!    member's own name), XTypes 1.3 §7.3.1.2.1.4.
//! 3. struct-/union-level `@autoid(HASH)` — `NameHash` of the member name,
//!    XTypes 1.3 §7.3.1.2.1.1.
//! 4. otherwise the caller's sequential fallback (positional index).
//!
//! The `NameHash` derivation (`MD5(name)[0..4]` as little-endian `u32`, masked
//! to the 28-bit member-ID space `& 0x0FFFFFFF`) is `zerodds_types`'
//! `NameHash::member_id_from_name` — the same function the TypeObject builder
//! (`zerodds_types::builder::StructBuilder::resolve_member_ids`) uses, so the
//! member IDs the data wire (XCDR1/XCDR2 EMHEADER/PID) emits are byte-identical
//! to the ones the TypeObject / descriptor carry.
//!
//! Consumers: `zerodds-idl-rust` (`annotations::resolved_member_id` delegates
//! here) and `zerodds-idl-cpp` (`resolved_member_ids`). The TypeObject builder
//! applies the identical precedence in `zerodds_types`.

use crate::ast::Annotation;
use crate::semantics::annotations::{AutoidKind, BuiltinAnnotation, lower_annotations};
use zerodds_types::type_object::common::NameHash;

/// `true` when the container carries `@autoid(HASH)` (struct- or union-level).
/// Defaults to `false` (i.e. `@autoid(SEQUENTIAL)`) — XTypes 1.3 §7.3.1.2.1.1.
#[must_use]
pub fn container_autoid_hash(type_annotations: &[Annotation]) -> bool {
    lower_annotations(type_annotations).is_ok_and(|l| {
        l.builtins
            .iter()
            .any(|a| matches!(a, BuiltinAnnotation::Autoid(AutoidKind::Hash)))
    })
}

/// Effective `@hashid` hint of a member: `Some(hint)` for `@hashid("hint")`,
/// `Some(member_name)` for a bare `@hashid`, `None` when absent.
fn hashid_hint(member_annotations: &[Annotation], member_name: &str) -> Option<String> {
    lower_annotations(member_annotations)
        .ok()?
        .builtins
        .into_iter()
        .find_map(|a| match a {
            BuiltinAnnotation::HashId(hint) => {
                Some(hint.unwrap_or_else(|| member_name.to_string()))
            }
            _ => None,
        })
}

/// Explicit `@id(n)` of a member, if present.
fn explicit_id(member_annotations: &[Annotation]) -> Option<u32> {
    lower_annotations(member_annotations)
        .ok()?
        .builtins
        .into_iter()
        .find_map(|a| match a {
            BuiltinAnnotation::Id(n) => Some(n),
            _ => None,
        })
}

/// The member-ID a member carries when it is fixed by an annotation — `@id(n)`,
/// `@hashid`, or a container `@autoid(HASH)` — independent of position.
/// `None` means the member takes the SEQUENTIAL default, i.e. the caller's
/// running counter (`@autoid(SEQUENTIAL)`).
///
/// Backends that keep a per-declarator sequential counter consume THIS (so
/// their existing counter semantics are untouched and they only stop ignoring
/// `@autoid(HASH)` / `@hashid`); backends that want the finished id in one call
/// use [`resolved_member_id`]. `member_name` is the raw IDL source spelling
/// (never an escaped/mangled backend identifier — the hash is over the wire
/// name).
#[must_use]
pub fn fixed_member_id(
    container_autoid_hash: bool,
    member_annotations: &[Annotation],
    member_name: &str,
) -> Option<u32> {
    if let Some(explicit) = explicit_id(member_annotations) {
        Some(explicit)
    } else if let Some(hint) = hashid_hint(member_annotations, member_name) {
        Some(NameHash::member_id_from_name(&hint))
    } else if container_autoid_hash {
        Some(NameHash::member_id_from_name(member_name))
    } else {
        None
    }
}

/// Resolve the wire member-ID of a single member (see module docs for the
/// precedence). `container_autoid_hash` is the enclosing struct's/union's
/// `@autoid(HASH)` flag; `member_name` is the raw IDL source spelling;
/// `fallback_seq` is the caller's sequential id used only when no
/// `@id`/`@hashid`/`@autoid(HASH)` applies. Thin wrapper over
/// [`fixed_member_id`].
#[must_use]
pub fn resolved_member_id(
    container_autoid_hash: bool,
    member_annotations: &[Annotation],
    member_name: &str,
    fallback_seq: u32,
) -> u32 {
    fixed_member_id(container_autoid_hash, member_annotations, member_name).unwrap_or(fallback_seq)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn first_struct(src: &str) -> crate::ast::StructDef {
        let ast = parse(src, &ParserConfig::default()).unwrap();
        for def in &ast.definitions {
            if let crate::ast::Definition::Type(crate::ast::TypeDecl::Constr(
                crate::ast::ConstrTypeDecl::Struct(crate::ast::StructDcl::Def(s)),
            )) = def
            {
                return s.clone();
            }
        }
        panic!("no struct");
    }

    fn raw_name(m: &crate::ast::Member) -> String {
        m.declarators
            .first()
            .map(|d| d.name().text.clone())
            .unwrap_or_default()
    }

    #[test]
    fn explicit_id_wins_over_autoid_and_hashid() {
        let s = first_struct("@autoid(HASH) struct S { @id(42) @hashid(\"x\") long a; };");
        let m = &s.members[0];
        assert_eq!(
            resolved_member_id(true, &m.annotations, &raw_name(m), 0),
            42
        );
    }

    #[test]
    fn hashid_hint_hashes_hint_string() {
        let s = first_struct("struct S { @hashid(\"my_hint\") long a; };");
        let m = &s.members[0];
        assert_eq!(
            resolved_member_id(false, &m.annotations, &raw_name(m), 0),
            NameHash::member_id_from_name("my_hint")
        );
    }

    #[test]
    fn bare_hashid_hashes_member_name() {
        let s = first_struct("struct S { @hashid long color; };");
        let m = &s.members[0];
        assert_eq!(
            resolved_member_id(false, &m.annotations, &raw_name(m), 7),
            NameHash::member_id_from_name("color")
        );
    }

    #[test]
    fn autoid_hash_hashes_member_name() {
        let s = first_struct("@autoid(HASH) struct S { long color; };");
        let m = &s.members[0];
        assert_eq!(
            resolved_member_id(true, &m.annotations, &raw_name(m), 3),
            NameHash::member_id_from_name("color")
        );
        assert!(container_autoid_hash(&s.annotations));
    }

    #[test]
    fn sequential_fallback_used_without_hash() {
        let s = first_struct("struct S { long a; long b; };");
        assert!(!container_autoid_hash(&s.annotations));
        let m1 = &s.members[1];
        assert_eq!(
            resolved_member_id(false, &m1.annotations, &raw_name(m1), 1),
            1
        );
    }

    #[test]
    fn known_md5_vector() {
        // XTypes §7.3.1.2.1.1: id = MD5(name)[0..4] LE u32 & 0x0FFFFFFF.
        assert_eq!(NameHash::member_id_from_name("color"), 0x0FA5_DD70);
        assert_eq!(NameHash::member_id_from_name("my_hint"), 0x026C_50E0);
    }
}
