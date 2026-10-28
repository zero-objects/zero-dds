// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Key-holder planning. DDSI-RTPS 2.5 §9.6.3.8, XTypes 1.3 §7.6.8.
//!
//! The key holder is the one place where endianness genuinely changes
//! the plan: it is defined big-endian regardless of what the sample
//! itself uses, because the hash has to match across peers that disagree
//! about byte order.

use crate::key::{Encoding, KeyHolderNesting, KeyHolderOrder, LayoutKey};
use crate::plan::{Extent, LayoutError, TypeLayout};
use crate::xcdr::{DepCollector, plan};
use alloc::vec::Vec;
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};

/// How the KeyHash is derived from the key holder.
/// DDSI-RTPS 2.5 §9.6.3.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyHashForm {
    /// The holder is at most sixteen bytes: it becomes the hash
    /// directly, zero-padded on the right.
    ZeroPadded,
    /// The holder's declared maximum exceeds sixteen bytes or is unbounded:
    /// the hash is its MD5, even when a particular value is shorter.
    Md5,
}

/// The maximum key-holder size that still fits a KeyHash unhashed.
pub const KEY_HASH_LEN: u32 = 16;

/// Plan the key holder of a type.
///
/// Contains exactly the `@key` members, recursing into nested keyed
/// aggregates, and always big-endian.
///
/// # Errors
/// [`LayoutError::KeyMustBeBigEndian`] if a little-endian key is asked
/// for — honouring it would produce a hash that no peer agrees with.
/// Otherwise whatever [`plan`] returns.
pub fn key_holder_plan(
    ast: &Ast,
    type_: Id<TypeNode>,
    encoding: Encoding,
    deps: &mut DepCollector,
) -> Result<TypeLayout, LayoutError> {
    plan(ast, type_, LayoutKey::key(encoding), deps)
}

/// Plan the key holder under a caller-supplied key.
///
/// [`key_holder_plan`] plans in the default dialect, which is what a
/// caller who has not chosen one means. This one exists because the
/// choice reaches the holder: whether an unannotated keyed aggregate
/// carries a DHEADER depends on what unannotated means, and two peers
/// that answer that differently compute different KeyHashes for the
/// same sample. A probe that writes RTI's payload bytes and Cyclone's
/// KeyHash would be reporting a divergence it created itself.
///
/// The purpose and the byte order are overwritten rather than trusted:
/// §7.6.8 fixes both, and a caller who passed a data key would
/// otherwise get every member planned instead of the keyed ones.
///
/// # Errors
/// Whatever [`plan`] returns.
pub fn key_holder_plan_with(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    deps: &mut DepCollector,
) -> Result<TypeLayout, LayoutError> {
    let key = LayoutKey {
        purpose: crate::key::Purpose::Key,
        endianness: crate::key::Endianness::Big,
        ..key
    };
    plan(ast, type_, key, deps)
}

/// Legacy fixed-extent classification, not a maximum-size calculation.
///
/// `Extent::Unbounded` also represents bounded variable-length strings and
/// collections. Use [`key_hash_form_for`] for generated or runtime KeyHashes.
#[must_use]
pub fn key_hash_form(layout: &TypeLayout) -> KeyHashForm {
    match layout.extent {
        Extent::Sized(size) if size <= KEY_HASH_LEN => KeyHashForm::ZeroPadded,
        _ => KeyHashForm::Md5,
    }
}

/// Select KeyHash storage from the holder's maximum, not its actual extent.
/// The walk saturates at seventeen bytes because only the sixteen-byte
/// threshold matters, preserving alignment, bounds and selected key members.
///
/// # Errors
/// Invalid layout, unresolved bounds or unsupported maximum-size semantics.
pub fn key_hash_form_for(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
) -> Result<KeyHashForm, LayoutError> {
    let key = LayoutKey {
        purpose: crate::key::Purpose::Key,
        endianness: crate::key::Endianness::Big,
        ..key
    };
    key_holder_plan_with(ast, type_, key, &mut DepCollector::default())?;
    Ok(
        if crate::key_maximum::holder(ast, type_, key)? <= KEY_HASH_LEN {
            KeyHashForm::ZeroPadded
        } else {
            KeyHashForm::Md5
        },
    )
}

/// Whether a key holder for `type_` keeps only the `@key` members.
///
/// False when the aggregate declares none. XTypes 1.3 §7.6.8 Step 1:
///
/// > If there are any key members, then remove the non-key members from
/// > FooKeyHolder. Otherwise do not remove any members.
///
/// So a `@key` member whose type declares no key of its own contributes
/// *all* of that type's members to the holder, not none. Reading the
/// first half of the sentence alone gave `@key Middle middle;` an empty
/// holder and every instance of such a type one instance handle.
///
/// A non-aggregate has no members to remove and answers false, which is
/// why [`member_key`] asks about the kind before it asks this.
#[must_use]
pub fn holder_keeps_only_keys(ast: &Ast, type_: Id<TypeNode>) -> bool {
    any_key_member(ast, &ast.all_members_of(type_))
}

/// Whether any of `members` carries `@key`.
///
/// `all_members_of`, not `members_of`, is what the caller must hand in:
/// a `@key` member the aggregate inherited is part of its key, and a
/// walk that misses it decides the aggregate is keyless and keeps
/// everything. XTypes 1.3 §7.4.1.1.5.1.
pub(crate) fn any_key_member(ast: &Ast, members: &[Id<zerodds_ast::MemberNode>]) -> bool {
    members
        .iter()
        .any(|member| ast.member(*member).is_ok_and(|m| m.facets.is_key()))
}

/// The key the members of `type_` are planned under, given the key
/// `type_` itself was planned under.
///
/// The same key, except under [`KeyHolderNesting::KeylessNestedWhole`]
/// below a keyless aggregate, where the subtree stops being filtered at
/// all and is therefore planned for data.
///
/// It is a function of the type and the key alone, with no accumulated
/// depth, because the callers re-enter the plan at arbitrary points: the
/// probe re-plans every nested aggregate it writes, so a rule that only
/// the root's walk could apply would be lost on the way down.
#[must_use]
pub fn member_key(ast: &Ast, type_: Id<TypeNode>, key: LayoutKey) -> LayoutKey {
    let aggregate = ast.type_(type_).is_ok_and(|node| {
        matches!(
            node.kind,
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. }
        )
    });
    member_key_for(key, aggregate && !holder_keeps_only_keys(ast, type_))
}

/// [`member_key`] once the caller has established that the type is a
/// keyless aggregate.
///
/// Split out so the planner, which has already walked the member list to
/// decide that, does not walk it a second time — and so the rule itself
/// exists once.
pub(crate) const fn member_key_for(key: LayoutKey, keyless_aggregate: bool) -> LayoutKey {
    if !keyless_aggregate
        || !matches!(key.purpose, crate::key::Purpose::Key)
        || !matches!(key.key_holder_nesting, KeyHolderNesting::KeylessNestedWhole)
    {
        return key;
    }
    LayoutKey {
        purpose: crate::key::Purpose::Data,
        ..key
    }
}

/// The discriminator of a union that is keyed by it, or `None`.
///
/// DDS-XTypes 1.3 Table 21 puts `@key` on a union discriminator and
/// §7.2.2.4.4.4.8's own example writes it, so a union can be a keyed
/// topic type without owning a single `@key` **member** — and
/// [`key_members`] walks members, so it answers the empty list for one.
/// Every caller that asks "is this type keyed" has to ask both, which is
/// why this is a function of its own and not folded into that walk.
///
/// §7.6.8's Step 1 over such a union removes everything but the
/// discriminator: the branches are not keys, and which branch a sample
/// carries is the discriminator's own answer. So the holder is the
/// discriminator alone, which is what the planner builds and what
/// `zerodds-wire-codec` writes.
///
/// A typedef in front of the union is transparent, as everywhere else.
///
/// Nothing answers `Some` unless a front end was asked to carry the
/// annotation — `zerodds_idl_lower::KeyOnUnionDiscriminator::Carried` —
/// so under the default this is `None` for every union in the corpus.
///
/// zerodds-lint: recursion-depth 64 — one frame per typedef hop, bounded
/// by [`crate::plan::MAX_DEPTH`].
#[must_use]
pub fn union_key_discriminator(ast: &Ast, type_: Id<TypeNode>) -> Option<Id<TypeNode>> {
    let mut current = type_;
    for _ in 0..crate::plan::MAX_DEPTH {
        let node = ast.type_(current).ok()?;
        match &node.kind {
            TypeKind::Alias { aliased } => current = aliased.resolved()?,
            TypeKind::Union { discriminator, .. } => {
                if !ast.discriminator_is_key(current) {
                    return None;
                }
                return discriminator.resolved();
            }
            _ => return None,
        }
    }
    None
}

/// The members that make up a type's key, in the holder's order.
///
/// Member-id order by default, which is XTypes 1.3 §7.6.8's and what
/// three of the four measured combinations write — Cyclone DDS in both
/// encoding versions, RTI Connext under version 2 and Fast DDS in both.
/// RTI under version 1 keeps declaration order, and two peers that
/// disagree about it compute different hashes for the same sample, so
/// the same instance arrives under two handles. [`KeyHolderOrder`]
/// carries the measurement and the selection.
///
/// # A keyless topic type still reports no key members
///
/// Step 1's "otherwise do not remove any members" reaches nested
/// aggregates, not the type the algorithm was invoked on: §7.6.8 defines
/// the computation for "a keyed type", and every vendor decides a
/// keyless topic has no KeyHash at all before any holder is built —
/// measured in `tests/fixtures/vendor_bytes.md`. So the rule applies
/// below the root and the root keeps answering with an empty list, which
/// is what both callers test for before they emit anything.
///
/// # Errors
/// [`LayoutError::Model`] when a handle in the type graph is stale.
pub fn key_members(ast: &Ast, type_: Id<TypeNode>) -> Result<Vec<NodeId>, LayoutError> {
    key_members_with(
        ast,
        type_,
        KeyHolderNesting::default(),
        KeyHolderOrder::default(),
    )
}

/// [`key_members`] under a caller-supplied nesting and ordering rule.
///
/// Both are dialect settings because the implementations disagree about
/// them; [`KeyHolderNesting`] and [`KeyHolderOrder`] carry the
/// measurements.
///
/// # Errors
/// [`LayoutError::Model`] when a handle in the type graph is stale.
pub fn key_members_with(
    ast: &Ast,
    type_: Id<TypeNode>,
    nesting: KeyHolderNesting,
    order: KeyHolderOrder,
) -> Result<Vec<NodeId>, LayoutError> {
    let mut out = Vec::new();
    collect_key_members(ast, type_, nesting, order, &mut out, 0)?;
    Ok(out)
}

/// zerodds-lint: recursion-depth 64 (typedef chains and nested key
/// holders; bounded by `plan::MAX_DEPTH`, which returns
/// `LayoutError::TooDeep`)
fn collect_key_members(
    ast: &Ast,
    type_: Id<TypeNode>,
    nesting: KeyHolderNesting,
    order: KeyHolderOrder,
    out: &mut Vec<NodeId>,
    depth: usize,
) -> Result<(), LayoutError> {
    let node = ast.type_(type_)?;
    // Loud, like `plan_at`: a truncated key set is a KeyHash two peers
    // compute differently, and returning `Ok` here would ship it.
    if depth > crate::plan::MAX_DEPTH {
        return Err(LayoutError::TooDeep { node: node.node_id });
    }
    // A typedef is transparent on the wire, so it is transparent to the
    // key set too: the aliased type's keyed members are this member's.
    if let TypeKind::Alias { aliased } = &node.kind {
        let target = aliased
            .resolved()
            .ok_or(LayoutError::Unresolved { node: node.node_id })?;
        return collect_key_members(ast, target, nesting, order, out, depth + 1);
    }
    if !matches!(
        node.kind,
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. }
    ) {
        return Ok(());
    }
    // `all_members_of`, not the type's own list: a `@key` member the
    // aggregate inherited is part of its key, and leaving it out gives
    // two distinct instances one instance handle. The same walk
    // `plan_at` does for the holder plan, which this list has to agree
    // with. XTypes 1.3 §7.4.1.1.5.1, DDSI-RTPS 2.5 §9.6.3.8.
    let members = ast.all_members_of(type_);

    // Step 1 removes the non-key members only "if there are any key
    // members"; a nested aggregate that declares none contributes all of
    // them. The root is exempt — see this function's own documentation —
    // because a keyless topic type has no KeyHash to build a holder for.
    let only_keys = depth == 0 || any_key_member(ast, &members);

    // Collect first, then order by effective member id.
    let mut keyed: Vec<(u32, NodeId, Option<Id<TypeNode>>)> = Vec::new();
    for member in &members {
        let m = ast.member(*member)?;
        if only_keys && !m.facets.is_key() {
            continue;
        }
        // The same rule `plan_aggregate` applies: a member without a
        // written id takes its declaration index, so both sides order
        // the key the same way. Collapsing them all onto 0 would let
        // the declaration order decide, which is not what §9.6.3.8 says.
        let id = m
            .facets
            .effective_member_id()
            .unwrap_or_else(|| position_of(&members, *member));
        keyed.push((id, m.node_id, m.type_.resolved()));
    }
    // A stable sort, so that under `Declaration` — where every entry
    // keeps the position it was collected in — the order is exactly the
    // one the members were declared in.
    if order == KeyHolderOrder::AscendingMemberId {
        keyed.sort_by_key(|(id, _, _)| *id);
    }

    // Below a keyless aggregate RTI's XCDR1 holder stops filtering
    // altogether and takes the subtree as the data has it, so nothing
    // under it is a key member. Everyone else keeps applying Step 1.
    let recurse = only_keys || nesting == KeyHolderNesting::Recursive;

    for (_, node_id, member_type) in keyed {
        out.push(node_id);
        // A member of the holder that is itself an aggregate contributes
        // its own holder's members. XTypes 1.3 §7.6.8 Step 3.
        if let (true, Some(inner)) = (recurse, member_type) {
            collect_key_members(ast, inner, nesting, order, out, depth + 1)?;
        }
    }
    Ok(())
}

/// Where `member` sits in `members`, as its implicit member id.
///
/// `u32::MAX` for a member that is not in the list, which cannot happen
/// for a list this function was handed — and if it ever does, the id
/// sorts last rather than colliding with a real one at 0.
fn position_of(
    members: &[Id<zerodds_ast::MemberNode>],
    member: Id<zerodds_ast::MemberNode>,
) -> u32 {
    members
        .iter()
        .position(|candidate| *candidate == member)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(u32::MAX)
}
