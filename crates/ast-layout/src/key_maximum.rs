// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Maximum key-holder size, saturated only after the sixteen-byte decision.
//! `Extent` is deliberately not used as a bound for variable-length values.

use crate::key::{LayoutKey, WirePosition, WstringTerminator};
use crate::plan::{Extent, LayoutError, MAX_DEPTH, MemberHeader};
use crate::xcdr::{DepCollector, plan, primitive_layout};
use alloc::vec::Vec;
use zerodds_ast::{Ast, Bound, CharWidth, TypeKind, TypeNode};
use zerodds_ast_core::Id;

const OVER: u32 = 17;
/// The alignment cap of a key holder: PLAIN_CDR2's four, which the actual
/// key-holder writers use even when the selected dialect retains XCDR1
/// framing — unless [`crate::key::KeyHolderAlignment`] says the dialect
/// aligns to its data encoding (Cyclone and RTI under XCDR1) or packs the
/// members (OpenSplice). Do not replace the encoding: its framing,
/// wide-character and parameter-header policies remain independently
/// relevant.
const fn holder_cap(key: LayoutKey) -> u8 {
    key.key_holder_alignment.max_alignment(key.encoding)
}

fn add(a: u32, b: u32) -> u32 {
    a.saturating_add(b).min(OVER)
}
fn align(at: u32, origin: u32, alignment: u8) -> u32 {
    add(
        origin,
        crate::plan::align_to(at.saturating_sub(origin), alignment),
    )
}

fn bound(
    ast: &Ast,
    bound: Bound,
    id: Id<TypeNode>,
    key: LayoutKey,
) -> Result<Option<u32>, LayoutError> {
    match bound {
        Bound::Unbounded => Ok(None),
        Bound::Const(expr) => {
            let number = ast
                .eval_integral(expr)
                .map_err(|_| unsupported(ast, id, key, "unevaluable collection bound"))?;
            u32::try_from(number)
                .map(Some)
                .map_err(|_| unsupported(ast, id, key, "invalid collection bound"))
        }
    }
}

fn unsupported(ast: &Ast, id: Id<TypeNode>, key: LayoutKey, reason: &'static str) -> LayoutError {
    match ast.type_(id) {
        Ok(node) => LayoutError::UnsupportedForEncoding {
            node: node.node_id,
            encoding: key.encoding,
            reason,
        },
        Err(error) => LayoutError::Model(error),
    }
}

/// Maximum of the actual root holder emitted by `key_holder`: selected members
/// directly, or a keyed union's discriminator, without invented root framing.
pub(crate) fn holder(ast: &Ast, id: Id<TypeNode>, key: LayoutKey) -> Result<u32, LayoutError> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        if let TypeKind::Alias { aliased } = &ast.type_(current)?.kind {
            current = aliased.resolved().ok_or(LayoutError::Unresolved {
                node: ast.type_(current)?.node_id,
            })?;
            continue;
        }
        break;
    }
    let mut walk = Walk {
        ast,
        stack: Vec::new(),
    };
    if let Some(discriminator) = crate::keyhash::union_key_discriminator(ast, current) {
        return walk.value(discriminator, key, 0, 0, WirePosition::Member);
    }
    let members = ast.all_members_of(current);
    let keys = crate::keyhash::key_members_with(
        ast,
        current,
        key.key_holder_nesting,
        key.key_holder_order,
    )?;
    let members = members
        .iter()
        .map(|m| ast.member(*m).map_err(LayoutError::Model))
        .collect::<Result<Vec<_>, _>>()?;
    let mut at = 0;
    for node in keys {
        // The shared list includes descendant key members. Their parent's
        // encoder accounts for them; the root holder names only its own fields.
        let Some(member) = members.iter().find(|m| m.node_id == node) else {
            continue;
        };
        let type_ = member
            .type_
            .resolved()
            .ok_or(LayoutError::Unresolved { node })?;
        at = walk.value(
            type_,
            crate::keyhash::member_key(ast, type_, key),
            at,
            0,
            WirePosition::Member,
        )?;
    }
    Ok(at)
}

struct Walk<'a> {
    ast: &'a Ast,
    stack: Vec<Id<TypeNode>>,
}

impl Walk<'_> {
    /// zerodds-lint: recursion-depth 64 — legal recursive collections have
    /// unbounded finite values; other cycles are rejected by the initial plan.
    fn value(
        &mut self,
        id: Id<TypeNode>,
        key: LayoutKey,
        at: u32,
        origin: u32,
        position: WirePosition,
    ) -> Result<u32, LayoutError> {
        if self.stack.contains(&id) {
            return Ok(OVER);
        }
        if self.stack.len() >= MAX_DEPTH {
            return Err(LayoutError::TooDeep {
                node: self.ast.type_(id)?.node_id,
            });
        }
        self.stack.push(id);
        let answer = self.inner(id, key, at, origin, position);
        self.stack.pop();
        answer
    }

    fn inner(
        &mut self,
        id: Id<TypeNode>,
        key: LayoutKey,
        mut at: u32,
        origin: u32,
        position: WirePosition,
    ) -> Result<u32, LayoutError> {
        let ast = self.ast;
        let node = ast.type_(id)?;
        let layout = plan(ast, id, key, &mut DepCollector::default())?;
        let resolved = |reference: &zerodds_ast_core::Ref<TypeNode>| {
            reference
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node.node_id })
        };
        match &node.kind {
            TypeKind::Alias { aliased } => {
                return self.value(resolved(aliased)?, key, at, origin, position);
            }
            TypeKind::Primitive(kind) => {
                let (alignment, size) = primitive_layout(*kind, key, position);
                return Ok(add(align(at, origin, alignment.min(holder_cap(key))), size));
            }
            TypeKind::Fixed { digits, .. } => return Ok(add(at, (u32::from(*digits) + 2) / 2)),
            TypeKind::String {
                width,
                bound: limit,
            } => {
                let Some(count) = bound(ast, *limit, id, key)? else {
                    return Ok(OVER);
                };
                let (width, terminal) = match width {
                    CharWidth::Char => (1, 1),
                    CharWidth::WChar => (
                        key.wstring_length.unit_at(position).octets() as u32,
                        u32::from(matches!(
                            key.wstring_length.terminator_at(position),
                            WstringTerminator::Present
                        )),
                    ),
                };
                let content = count
                    .saturating_add(terminal)
                    .saturating_mul(width)
                    .min(OVER);
                return Ok(add(
                    add(align(at, origin, 4.min(holder_cap(key))), 4),
                    content,
                ));
            }
            TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } | TypeKind::Bitset { .. } => {
                let Extent::Sized(size) = layout.extent else {
                    return Err(unsupported(ast, id, key, "scalar maximum is unavailable"));
                };
                return Ok(add(
                    align(at, origin, layout.align.min(holder_cap(key))),
                    size,
                ));
            }
            _ => {}
        }
        if layout.needs_dheader {
            at = add(align(at, origin, 4.min(holder_cap(key))), 4);
        }
        let result = match &node.kind {
            TypeKind::Sequence {
                element,
                bound: limit,
            } => {
                let count = bound(ast, *limit, id, key)?;
                at = add(align(at, origin, 4.min(holder_cap(key))), 4);
                self.repeat(&[resolved(element)?], count, key, at, origin)?
            }
            TypeKind::Map {
                key: map_key,
                value,
                bound: limit,
            } => {
                let count = bound(ast, *limit, id, key)?;
                at = add(align(at, origin, 4.min(holder_cap(key))), 4);
                self.repeat(
                    &[resolved(map_key)?, resolved(value)?],
                    count,
                    key,
                    at,
                    origin,
                )?
            }
            TypeKind::Array { element, dims } => {
                // Only the threshold matters. Saturating counts cannot wrap a
                // huge array into a falsely small one, and zero-size elements
                // reach a fixed point after one iteration.
                let mut count = 1u32;
                for dim in dims {
                    let n =
                        u32::try_from(ast.eval_integral(*dim).map_err(|_| {
                            unsupported(ast, id, key, "unevaluable array dimension")
                        })?)
                        .map_err(|_| unsupported(ast, id, key, "invalid array dimension"))?;
                    count = count.saturating_mul(n);
                }
                self.repeat(&[resolved(element)?], Some(count), key, at, origin)?
            }
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
                let members = ast
                    .all_members_of(id)
                    .iter()
                    .map(|m| ast.member(*m).map_err(LayoutError::Model))
                    .collect::<Result<Vec<_>, _>>()?;
                for member_plan in &layout.members {
                    let member = members
                        .iter()
                        .find(|m| m.node_id == member_plan.member)
                        .ok_or(LayoutError::Unresolved {
                            node: member_plan.member,
                        })?;
                    let target = member.type_.resolved().ok_or(LayoutError::Unresolved {
                        node: member.node_id,
                    })?;
                    at = self.member(
                        target,
                        crate::keyhash::member_key(ast, target, key),
                        member_plan.header,
                        at,
                        origin,
                    )?;
                }
                at
            }
            TypeKind::Union {
                discriminator,
                cases,
                ..
            } => {
                let discriminator = resolved(discriminator)?;
                at = self.member(
                    discriminator,
                    key,
                    layout.discriminator.unwrap_or(MemberHeader::NoHeader),
                    at,
                    origin,
                )?;
                if crate::keyhash::union_key_discriminator(ast, id).is_some()
                    && key.purpose == crate::key::Purpose::Key
                {
                    return Ok(at);
                }
                let mut maximum = at;
                for case in cases {
                    let case = ast.case(*case)?;
                    let member = ast.member(case.member)?;
                    let target = member.type_.resolved().ok_or(LayoutError::Unresolved {
                        node: member.node_id,
                    })?;
                    let header = layout
                        .members
                        .iter()
                        .find(|m| m.member == member.node_id)
                        .map_or(MemberHeader::NoHeader, |m| m.header);
                    maximum = maximum.max(self.member(target, key, header, at, origin)?);
                }
                maximum
            }
            TypeKind::ValueBox { boxed } => {
                self.value(resolved(boxed)?, key, add(at, 1), origin, position)?
            }
            _ => {
                return Err(unsupported(
                    ast,
                    id,
                    key,
                    "maximum key holder is not defined for this construct",
                ));
            }
        };
        Ok(if layout.needs_sentinel {
            add(align(result, origin, 4.min(holder_cap(key))), 4)
        } else {
            result
        })
    }

    fn repeat(
        &mut self,
        elements: &[Id<TypeNode>],
        count: Option<u32>,
        key: LayoutKey,
        mut at: u32,
        origin: u32,
    ) -> Result<u32, LayoutError> {
        let mut left = count.unwrap_or(u32::MAX);
        while left > 0 {
            let before = at;
            for element in elements {
                at = self.value(
                    *element,
                    crate::keyhash::member_key(self.ast, *element, key),
                    at,
                    origin,
                    WirePosition::Element,
                )?;
            }
            if at >= OVER || at == before {
                return Ok(at);
            }
            left -= 1;
        }
        Ok(at)
    }

    fn member(
        &mut self,
        id: Id<TypeNode>,
        key: LayoutKey,
        header: MemberHeader,
        mut at: u32,
        origin: u32,
    ) -> Result<u32, LayoutError> {
        match header {
            MemberHeader::NoHeader => self.value(id, key, at, origin, WirePosition::Member),
            MemberHeader::OptionalFlag => {
                self.value(id, key, add(at, 1), origin, WirePosition::Member)
            }
            MemberHeader::EmHeader { lc, .. } => {
                at = add(
                    align(at, origin, 4.min(holder_cap(key))),
                    if lc == crate::plan::LengthCode::Lc4 {
                        8
                    } else {
                        4
                    },
                );
                self.value(id, key, at, origin, WirePosition::Member)
            }
            MemberHeader::PidCdr1 { extended, .. }
            | MemberHeader::OptionalCdr1 { extended, .. } => {
                at = add(
                    align(at, origin, 4.min(holder_cap(key))),
                    if extended { 12 } else { 4 },
                );
                // A parameter starts a fresh alignment origin. Its unpadded
                // length excludes the padding before the next parameter.
                let end = self.value(id, key, at, at, WirePosition::Member)?;
                Ok(if key.pl_cdr1_headers.pads_after_value() {
                    align(end, origin, 4.min(holder_cap(key)))
                } else {
                    end
                })
            }
        }
    }
}
