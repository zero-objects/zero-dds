// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Whether a sample's type can exceed what a short PL_CDR1 parameter
//! header states, counted the way RTI Connext 7.7 counts it.
//!
//! RTI frames a sample under XCDR1 with the short parameter header
//! until its type's maximum serialized size passes a step, and from
//! there gives `PID_EXTENDED` to every member whose type is not a
//! primitive or an enumeration
//! ([`crate::key::PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples`]).
//! This module is the count, and every constant in it is a measurement
//! on rtiddsgen's type support (`tools/wireprobe/wireprobe-rti`, XCDR1
//! little endian):
//!
//! | sample type | last short | first extended |
//! |---|---|---|
//! | `@mutable { long id; sequence<long, 100> small; sequence<octet, N> mid; }` | 65068 | 65069 |
//! | `@mutable { long id; sequence<octet, N> mid; }` | 65484 | 65485 |
//! | `@mutable { long id; double d; sequence<octet, N> mid; }` | 65464 | 65465 |
//! | `@mutable { long id; string s; sequence<octet, N> mid; }` | 65212 | 65213 |
//! | `@mutable { long id; sequence<long> small; sequence<octet, N> mid; }` | 65068 | 65069 |
//! | `@appendable { long id; @optional sequence<long, 100> small_opt; sequence<octet, N> big; }` | 65107 | 65108 |
//!
//! One count gives all six steps: the four octets of the encapsulation,
//! twelve octets for every parameter header, twelve for the end of a
//! `@mutable` list, rtiddsgen's default bounds — 255 characters for a
//! `string`, 100 elements for a `sequence` — and no alignment at the
//! start of a parameter's value (the `double` row: aligning it to eight
//! would put the step four octets lower). The step is passed when that
//! count exceeds `0xFFFF`. Whether RTI counts the end of the list as an
//! extended header or adds its eight octets elsewhere, the rows cannot
//! tell apart; the sums are the same.
//!
//! Not measured, and counted the way the measured rows suggest: a
//! `map` (bound 100, as a sequence), a `wstring` (bound 255), alignment
//! inside a value that is not `@mutable`, a union that is not
//! `@mutable`.

use crate::key::{LayoutKey, WirePosition, WstringTerminator};
use crate::plan::{Extent, LayoutError, MAX_DEPTH, align_to};
use crate::xcdr::{DepCollector, extensibility_of, plan, primitive_layout};
use alloc::vec::Vec;
use zerodds_ast::{Ast, Bound, CharWidth, Extensibility, TypeKind, TypeNode};
use zerodds_ast_core::Id;

/// The encapsulation identifier and options ahead of the payload.
const ENCAPSULATION: u32 = 4;
/// A parameter header, counted extended whatever form it is written in.
const PARAMETER_HEADER: u32 = 12;
/// The end of a `@mutable` list.
const LIST_END: u32 = 12;
/// rtiddsgen's bound for a `string` or `wstring` that declares none.
const STRING_BOUND: u32 = 255;
/// rtiddsgen's bound for a `sequence` or `map` that declares none.
const SEQUENCE_BOUND: u32 = 100;
/// The largest length a short parameter header states.
const SHORT_LENGTH_MAX: u32 = 0xFFFF;
/// Past the step: a count that reaches it stops counting.
const PAST: u32 = 0x0002_0000;

/// Whether a sample of `root` is past RTI's step.
///
/// # Errors
/// [`LayoutError`] for an unresolved reference, an unevaluable bound or
/// dimension, a nesting past [`MAX_DEPTH`], or a construct no sample
/// holds.
pub(crate) fn exceeds_short_length(
    ast: &Ast,
    root: Id<TypeNode>,
    key: LayoutKey,
) -> Result<bool, LayoutError> {
    let mut walk = Walk {
        ast,
        key,
        stack: Vec::new(),
    };
    let body = walk.value(root, 0, 0)?;
    Ok(add(body, ENCAPSULATION) > SHORT_LENGTH_MAX)
}

/// Whether a member of this type keeps the short header in a sample
/// past the step: a primitive or an enumeration, through any alias.
///
/// `bitmask` is answered with the enumerations and not measured:
/// rtiddsgen 4.7.0 refused the declaration the probe gave it.
///
/// Public because the generated codecs need it: under
/// [`crate::key::PlCdr1HeaderPolicy::ExtendedForConstructedInLargeSamples`]
/// a member whose type answers `false` here takes `PID_EXTENDED` when
/// the *sample* is past the step, and the generated encoder decides
/// that at run time from a flag the sample's root hands down.
///
/// # Errors
/// [`LayoutError::Unresolved`] for an alias that names nothing.
pub fn keeps_short_header(ast: &Ast, id: Id<TypeNode>) -> Result<bool, LayoutError> {
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        let node = ast.type_(current)?;
        match &node.kind {
            TypeKind::Alias { aliased } => {
                current = aliased
                    .resolved()
                    .ok_or(LayoutError::Unresolved { node: node.node_id })?;
            }
            TypeKind::Primitive(_) | TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => {
                return Ok(true);
            }
            _ => return Ok(false),
        }
    }
    Err(LayoutError::TooDeep {
        node: ast.type_(id)?.node_id,
    })
}

fn add(a: u32, b: u32) -> u32 {
    a.saturating_add(b).min(PAST)
}

fn align(at: u32, origin: u32, alignment: u8) -> u32 {
    add(origin, align_to(at.saturating_sub(origin), alignment))
}

struct Walk<'a> {
    ast: &'a Ast,
    key: LayoutKey,
    stack: Vec<Id<TypeNode>>,
}

impl Walk<'_> {
    /// The count after a value of `id` written at `at`, aligned from
    /// `origin`.
    ///
    /// zerodds-lint: recursion-depth 64 — the stack bounds it at
    /// `MAX_DEPTH`, and a type that reaches itself has no maximum.
    fn value(&mut self, id: Id<TypeNode>, at: u32, origin: u32) -> Result<u32, LayoutError> {
        if at >= PAST || self.stack.contains(&id) {
            return Ok(PAST);
        }
        if self.stack.len() >= MAX_DEPTH {
            return Err(LayoutError::TooDeep {
                node: self.ast.type_(id)?.node_id,
            });
        }
        self.stack.push(id);
        let answer = self.inner(id, at, origin);
        self.stack.pop();
        answer
    }

    fn inner(&mut self, id: Id<TypeNode>, mut at: u32, origin: u32) -> Result<u32, LayoutError> {
        let ast = self.ast;
        let key = self.key;
        let node = ast.type_(id)?;
        let resolved = |reference: &zerodds_ast_core::Ref<TypeNode>| {
            reference
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node.node_id })
        };
        Ok(match &node.kind {
            TypeKind::Alias { aliased } => self.value(resolved(aliased)?, at, origin)?,
            TypeKind::Primitive(kind) => {
                let (alignment, size) = primitive_layout(*kind, key, WirePosition::Member);
                add(align(at, origin, alignment), size)
            }
            TypeKind::Fixed { digits, .. } => add(at, (u32::from(*digits) + 2) / 2),
            TypeKind::String { width, bound } => {
                let count = self.bound(*bound, STRING_BOUND, id)?;
                let (unit, terminal) = match width {
                    CharWidth::Char => (1, 1),
                    CharWidth::WChar => (
                        key.wstring_length.unit_at(WirePosition::Member).octets() as u32,
                        u32::from(matches!(
                            key.wstring_length.terminator_at(WirePosition::Member),
                            WstringTerminator::Present
                        )),
                    ),
                };
                let content = count.saturating_add(terminal).saturating_mul(unit);
                add(add(align(at, origin, 4), 4), content)
            }
            TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } | TypeKind::Bitset { .. } => {
                let layout = plan(ast, id, key, &mut DepCollector::default())?;
                let Extent::Sized(size) = layout.extent else {
                    return Err(unsupported(node, key, "a scalar without a size"));
                };
                add(align(at, origin, layout.align), size)
            }
            TypeKind::Sequence { element, bound } => {
                let count = self.bound(*bound, SEQUENCE_BOUND, id)?;
                at = add(align(at, origin, 4), 4);
                self.repeat(&[resolved(element)?], count, at, origin)?
            }
            TypeKind::Map {
                key: map_key,
                value,
                bound,
            } => {
                let count = self.bound(*bound, SEQUENCE_BOUND, id)?;
                at = add(align(at, origin, 4), 4);
                self.repeat(&[resolved(map_key)?, resolved(value)?], count, at, origin)?
            }
            TypeKind::Array { element, dims } => {
                let mut count = 1u32;
                for dim in dims {
                    let n = ast
                        .eval_integral(*dim)
                        .ok()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| unsupported(node, key, "an unevaluable array dimension"))?;
                    count = count.saturating_mul(n);
                }
                self.repeat(&[resolved(element)?], count, at, origin)?
            }
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
                let mutable = extensibility_of(ast, node, key) == Extensibility::Mutable;
                for member in ast.all_members_of(id) {
                    let member = ast.member(member)?;
                    let target = member.type_.resolved().ok_or(LayoutError::Unresolved {
                        node: member.node_id,
                    })?;
                    at = if mutable || member.facets.is_optional() {
                        self.parameter(target, at, origin)?
                    } else {
                        self.value(target, at, origin)?
                    };
                }
                if mutable {
                    add(align(at, origin, 4), LIST_END)
                } else {
                    at
                }
            }
            TypeKind::Union {
                discriminator,
                cases,
                ..
            } => {
                let mutable = extensibility_of(ast, node, key) == Extensibility::Mutable;
                let discriminator = resolved(discriminator)?;
                at = if mutable {
                    self.parameter(discriminator, at, origin)?
                } else {
                    self.value(discriminator, at, origin)?
                };
                let mut widest = at;
                for case in cases {
                    let member = ast.member(ast.case(*case)?.member)?;
                    let target = member.type_.resolved().ok_or(LayoutError::Unresolved {
                        node: member.node_id,
                    })?;
                    widest = widest.max(if mutable {
                        self.parameter(target, at, origin)?
                    } else {
                        self.value(target, at, origin)?
                    });
                }
                if mutable {
                    add(align(widest, origin, 4), LIST_END)
                } else {
                    widest
                }
            }
            _ => return Err(unsupported(node, key, "a construct no sample holds")),
        })
    }

    /// A value behind a parameter header: the header counted extended,
    /// and the value aligned from its own start.
    fn parameter(&mut self, id: Id<TypeNode>, at: u32, origin: u32) -> Result<u32, LayoutError> {
        let start = add(align(at, origin, 4), PARAMETER_HEADER);
        self.value(id, start, start)
    }

    fn repeat(
        &mut self,
        elements: &[Id<TypeNode>],
        count: u32,
        mut at: u32,
        origin: u32,
    ) -> Result<u32, LayoutError> {
        for _ in 0..count {
            let before = at;
            for element in elements {
                at = self.value(*element, at, origin)?;
            }
            // Past the step the answer is known; an element that adds
            // nothing adds nothing however often it repeats.
            if at >= PAST || at == before {
                break;
            }
        }
        Ok(at)
    }

    fn bound(&self, bound: Bound, default: u32, id: Id<TypeNode>) -> Result<u32, LayoutError> {
        match bound {
            Bound::Unbounded => Ok(default),
            Bound::Const(expr) => self
                .ast
                .eval_integral(expr)
                .ok()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| match self.ast.type_(id) {
                    Ok(node) => unsupported(node, self.key, "an unevaluable collection bound"),
                    Err(error) => LayoutError::Model(error),
                }),
        }
    }
}

fn unsupported(node: &TypeNode, key: LayoutKey, reason: &'static str) -> LayoutError {
    LayoutError::UnsupportedForEncoding {
        node: node.node_id,
        encoding: key.encoding,
        reason,
    }
}
