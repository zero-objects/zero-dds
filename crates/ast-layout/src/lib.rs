// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-ast-layout`. Safety classification: **SAFE**.
//!
//! Wire layout planning over the common AST. Produces alignment, extent,
//! header framing and offsets per `(encoding, endianness, purpose)`;
//! codecs execute the plan, this crate never moves sample bytes.
//!
//! Every wire rule lives here exactly once. The duplication across
//! fifteen code emitters that this rebuild removes had its cost in
//! divergence, most recently in the length-code framing of
//! variable-length `@mutable` members.
//!
//! ## Spec
//!
//! - `docs/specs/zerodds-ast-1.0.md` §7 (layout view).
//! - OMG XTypes 1.3 §7.4 (wire encoding), §7.4.1.2 (PL_CDR1),
//!   §7.4.2 (Plain CDR2 / Delimited CDR2 / PL_CDR2), §7.6.8 (KeyHash).
//! - DDSI-RTPS 2.5 §9.6.3.8 (KeyHash), §10 (encapsulation).
//!
//! ## Layer position
//!
//! Layer 2 — over `zerodds-ast`.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

// The observer-facing cache wrapper needs a Mutex, which lives in std;
// the planning core does not.
#[cfg(any(feature = "std", test))]
extern crate std;

pub mod axes;
pub mod cache;
pub mod key;
mod key_maximum;
pub mod keyhash;
mod large_sample;
pub mod plan;
pub mod xcdr;

pub use axes::{
    AXES, AxisFlag, DialectLowering, DialectSettings, dialect_help, dialect_names,
    dialect_settings_from_overrides,
};
pub use cache::LayoutCache;
pub use key::{
    AggregateDelimiter, ArrayElementFlattening, CollectionDelimiterPolicy,
    DefaultedDiscriminatorMember, Dialect, ElementPadding, Encoding, Endianness, EnumHolderWidth,
    EnumeratorValue, FixedDigitCount, KeyHolderAlignment, KeyHolderExtensibility, KeyHolderNesting,
    KeyHolderOrder, LayoutKey, LengthCodePolicy, MapDelimiter, MapEntryOrder, MapKeyOrdering,
    MustUnderstandPolicy, MutableUnionFraming, OffsetKey, PidFlagPolicy, PlCdr1FramingValues,
    PlCdr1HeaderPolicy, PlCdr1Reading, Purpose, SequenceElementDelimiter, ShortPidRange,
    UnionLabelMatch, WcharFraming, WcharPosition, WcharWidth, WirePosition, WstringCodeUnits,
    WstringLengthCounts, WstringLengthPolicy, WstringTerminator, WstringUnitOrder,
    WstringUnitWidth,
};
pub use keyhash::{
    KEY_HASH_LEN, KeyHashForm, holder_keeps_only_keys, key_hash_form, key_holder_plan,
    key_holder_plan_with, key_members, key_members_with, member_key, union_key_discriminator,
};
pub use large_sample::keeps_short_header;
pub use plan::{Extent, LayoutError, LengthCode, MemberHeader, MemberPlan, Offset, TypeLayout};
pub use xcdr::{
    DepCollector, PID_EXTENDED, PID_LIST_END, PID_SHORT_MAX_ID, PID_SHORT_MAX_LEN,
    PL_CDR1_ALIGNMENT, array_is_delimited, collection_is_delimited, enumeration_bit_bound,
    enumerator_wire_value, extensibility_of, fixed_wire_digits, length_code, map_key_ordering,
    member_held_inline_closes_a_cycle, member_must_understand, needs_extended_pid, plan,
    primitive_layout, sequence_element_carries_length, types_that_close_an_inadmissible_cycle,
    union_negative_matches_nothing,
};
