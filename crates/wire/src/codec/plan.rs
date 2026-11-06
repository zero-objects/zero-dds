// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The layout plan of a described type, as the serializer executes it.
//!
//! Every field here is an answer the layout planner gave
//! (`zerodds_ast_layout::xcdr::plan` and its crossing functions) for the
//! model the type's IDL composes to; `zerodds-codec` copies the answers
//! in. The serializer and the deserializer only follow them: which header
//! a member takes, whether a collection is delimited, how wide an
//! enumeration's holder is. A runtime that decided any of it would be a
//! second place where the wire format lives.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use super::Primitive;
use crate::ParameterRole;

/// The plans of one type and of every type it reaches, for one
/// encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSet {
    /// One plan per struct.
    pub structs: Vec<StructPlan>,
    /// One plan per union, and one per union a key holder reaches.
    pub unions: Vec<UnionPlan>,
    /// The key holders of the structs a key holder reaches, the root's
    /// among them (XTypes 1.3 §7.6.8 steps 1 and 3).
    pub holders: Vec<HolderPlan>,
    /// The type the set was built for.
    pub root: Root,
    /// The key holder of the root, when it is a keyed type (XTypes 1.3
    /// §7.6.8).
    pub key: Option<KeyPlan>,
    /// The encapsulation's alignment cap, `MAXALIGN` (XTypes 1.3 §7.4.1).
    pub max_align: usize,
}

/// The type a [`PlanSet`] was built for, or its key holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    /// A struct, by its plan's index in [`PlanSet::structs`], or in
    /// [`PlanSet::holders`] for [`KeyPlan::root`].
    Struct(usize),
    /// A union, by its plan's index in [`PlanSet::unions`].
    Union(usize),
}

/// One union, as the planner frames it (XTypes 1.3 §7.4.3.5.3 rules (10),
/// (11), (26) to (30)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionPlan {
    /// The scoped name, for a refusal that names the type.
    pub name: String,
    /// A DHEADER in front of the discriminator (XCDR2 `@appendable` and
    /// `@mutable`).
    pub needs_dheader: bool,
    /// `PID_SENTINEL` behind the member (PL_CDR1).
    pub needs_sentinel: bool,
    /// The discriminator, under member id 0, with the header a `@mutable`
    /// union gives it.
    pub discriminator: MemberFrame,
    /// The key holder of a union keyed by its discriminator: the
    /// discriminator alone, no branch (XTypes 1.3 §7.6.8 step 1 over
    /// `switch (@key D)`).
    pub discriminator_only: bool,
    /// The cases, in declaration order.
    pub branches: Vec<BranchPlan>,
}

impl UnionPlan {
    /// The branch the discriminator value `label` selects: the case whose
    /// labels hold it, else the `default:` case, else none (IDL 4.2
    /// §7.4.1.4.4.4).
    #[must_use]
    pub fn select(&self, label: i128) -> Option<usize> {
        self.branches
            .iter()
            .position(|branch| branch.labels.contains(&label))
            .or_else(|| self.branches.iter().position(|branch| branch.default))
    }
}

/// One case of a union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchPlan {
    /// The values of its `case` labels.
    pub labels: Vec<i128>,
    /// `default:`.
    pub default: bool,
    /// The branch member's framing and value.
    pub frame: MemberFrame,
}

/// One struct, as the planner frames it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructPlan {
    /// The scoped name, for a refusal that names the type.
    pub name: String,
    /// A DHEADER in front of the members (XCDR2 `@appendable` and
    /// `@mutable`).
    pub needs_dheader: bool,
    /// `PID_SENTINEL` behind the members (PL_CDR1).
    pub needs_sentinel: bool,
    /// The members are read by their headers' ids (`@mutable`).
    pub by_member_id: bool,
    /// A writer may stop before the last member (`@appendable`): the
    /// members it left out take their defaults (XTypes 1.3 Table 19).
    pub appendable: bool,
    /// The members, in declaration order.
    pub members: Vec<MemberFrame>,
}

/// One member's framing and value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberFrame {
    /// The member id.
    pub member_id: u32,
    /// The header in front of it.
    pub header: Header,
    /// `@optional`.
    pub optional: bool,
    /// The value.
    pub value: ValuePlan,
}

/// A member's header, as the planner chose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Header {
    /// None: the member sits where the declaration puts it.
    None,
    /// XCDR2 `<is_present>` of an `@optional` member (rule (20)).
    OptionalFlag,
    /// A PL_CDR1 parameter header (rules (19), (24), (25)).
    Pid {
        /// The short header's id.
        pid: u16,
        /// The extended form (§7.4.1.2.2).
        extended: bool,
        /// `FLAG_MUST_UNDERSTAND`.
        must_understand: bool,
        /// A `@mutable` member, or an `@optional` one of another type.
        role: ParameterRole,
    },
    /// An XCDR2 EMHEADER (§7.4.3.4.2).
    EmHeader {
        /// The length code.
        length_code: u8,
        /// `M_FLAG`.
        must_understand: bool,
    },
}

/// One value's shape on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValuePlan {
    /// A primitive.
    Primitive(Primitive),
    /// A `string`, with its bound.
    String {
        /// `N` of `string<N>`.
        bound: Option<usize>,
    },
    /// A sequence.
    Sequence {
        /// The element.
        element: Box<ValuePlan>,
        /// `N` of `sequence<T, N>`.
        bound: Option<usize>,
        /// A DHEADER in front of the length (§7.4.3.5.3 rule (12)).
        delimited: bool,
    },
    /// An array, all its dimensions.
    Array {
        /// The innermost element.
        element: Box<ValuePlan>,
        /// The dimensions, outermost first.
        dims: Vec<usize>,
        /// A DHEADER in front of the elements (rule (9)).
        delimited: bool,
    },
    /// An enumeration, as the integer its holder carries.
    Enum {
        /// The holder's bit bound.
        holder_bits: u16,
        /// The wire value of each enumerator, in declaration order.
        values: Vec<i128>,
    },
    /// A struct, by its plan's index in [`PlanSet::structs`].
    Struct(usize),
    /// A union, by its plan's index in [`PlanSet::unions`].
    Union(usize),
    /// A struct inside a key holder, by its holder's index in
    /// [`PlanSet::holders`].
    Holder(usize),
}

/// The key holder of one struct (XTypes 1.3 §7.6.8 step 1): its key
/// members, or all of its members when it declares none, `@final`, in
/// the holder's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HolderPlan {
    /// The scoped name, for a refusal that names the type.
    pub name: String,
    /// The members in the holder's order: each one's declaration index
    /// and frame (no header, or an `@optional` member's `<is_present>`).
    pub members: Vec<(usize, MemberFrame)>,
}

/// The key holder of a set's root (XTypes 1.3 §7.6.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPlan {
    /// The holder's plan: a struct's in [`PlanSet::holders`], or a
    /// union's in [`PlanSet::unions`].
    pub root: Root,
    /// The holder's alignment cap.
    pub max_align: usize,
    /// Step 5's branch: the octets themselves zero-padded (`true`), or
    /// their MD5.
    pub zero_padded: bool,
}
