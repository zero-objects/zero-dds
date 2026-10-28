// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Field discriminators for changesets. Spec §5.2.
//!
//! Typed constants rather than strings, so a misspelled field is a
//! compile error instead of a branch an observer silently never takes.
//! The numbers are part of the snapshot format — append only.

use zerodds_ast_core::FieldTag;

/// A node's simple name.
pub const NAME: FieldTag = FieldTag(1);
/// A type reference: a member's type, a sequence element, a base.
pub const REFERENCE: FieldTag = FieldTag(2);
/// A type's completeness, when a forward declaration gains a body.
pub const COMPLETENESS: FieldTag = FieldTag(3);
/// The extensibility recorded in the XTypes facet.
pub const EXTENSIBILITY: FieldTag = FieldTag(4);
/// The effective member id recorded in the XTypes facet.
pub const MEMBER_ID: FieldTag = FieldTag(5);
/// Any other facet field, for changes too fine to enumerate.
pub const FACET: FieldTag = FieldTag(6);
/// An aggregate's list of parts: members, cases, enumerators, bits,
/// bitfields. Adding or removing one changes the aggregate too, and a
/// consumer that tracks the aggregate — the layout cache, for one —
/// has to hear about it.
pub const PARTS: FieldTag = FieldTag(7);
