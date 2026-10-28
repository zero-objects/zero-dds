// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Crate `zerodds-wire-codec`. Safety classification: **SAFE**.
//!
//! One layout plan, one value, the bytes between them — in both
//! directions, over any representation of a value.
//!
//! ## What is here and what is not
//!
//! [`crate::encode()`] walks the plan and writes; [`crate::decode()`] walks
//! the same plan and reads. Neither decides a DHEADER, an EMHEADER, a
//! length code or a dialect answer: all of those are
//! `zerodds_ast_layout`'s, and a walk that decided them again would
//! agree with the plan only until one of the two changed.
//!
//! [`crate::contract`] is the single answer to what a value *means* —
//! whether a key names a member, whether `null` is a value, whether an
//! integer is expressible — because three backends ask it and a
//! difference between their answers would be a difference in the sample
//! read as a difference in the serialisers.
//!
//! ## Why it is generic over the value
//!
//! The walk was already generic on the type side: `&Ast`, an
//! `Id<TypeNode>` and a `LayoutKey`. It was concrete only on the value
//! side, against `serde_json::Value`, and a second value representation
//! — a `DynamicData` — would otherwise have needed a second copy of
//! every framing decision this crate makes once.
//!
//! So the value side is two traits: [`source::Source`] for the direction
//! that reads a value and writes bytes, [`sink::Sink`] for the direction
//! that reads bytes and builds a value. `zerodds-wire-probe` implements
//! both for `serde_json::Value`.
//!
//! **Nothing here fixes a coordinate.** A [`zerodds_ast_layout::WirePosition`],
//! a dialect setting and a [`zerodds_ast::PrimitiveKind`] are arguments
//! at every site that has one, including inside the traits — a
//! representation that wants to know the width it is being handed is
//! told, and a representation that does not may ignore it. A coordinate
//! passed as a literal "because it is always the same here" is how the
//! framing defect survived five emitters.
//!
//! ## Layer position
//!
//! A tool over `zerodds-ast-layout` and `zerodds-wire`. It names no
//! parser: turning IDL text into a model is the caller's business.
#![forbid(unsafe_code)]

pub mod canonical;
pub mod contract;
pub mod decode;
pub mod default;
pub mod encode;
#[cfg(feature = "serde-json")]
pub mod json;
pub mod sink;
pub mod source;
pub mod value;

/// How many nested aggregates a single value may hold before either
/// walk refuses it.
///
/// ## What it is for
///
/// A recursive type is legal — IDL 4.2 §7.4.1.4.4.4.4 for a cycle
/// through a collection, and the same property in the other form for a
/// cycle through an `@optional` member — because every *value* of one
/// carries its own terminator: a sequence of length zero, an absent
/// member. The type is a graph and the value is a tree.
///
/// A value whose graph really does have a cycle — `a.next = a` — has no
/// terminator anywhere in it, and XTypes 1.3 §7.3.1.2.1.4 says of
/// exactly that case "if the graph of objects has cycles, it is not
/// serializable". Neither walk can see the cycle: a [`source::Source`]
/// hands the encoder one member at a time and a byte stream hands the
/// decoder one header at a time, and in both directions the shape is
/// only ever known one step ahead. So the walks count, and a value past
/// the count is refused by name — [`encode::ProbeError::ValueTooDeep`]
/// — rather than truncated, looped on, or run into the stack. A silent
/// prefix of an infinite stream is the failure class this project ranks
/// above every other.
///
/// ## Why this number
///
/// Both walks are recursive descent — one `write_aggregate` frame or
/// one `read_aggregate` frame per level — so the real ceiling is the
/// thread's stack, and the limit has to sit below it with room to
/// spare. Measured 2026-08-26 with the guard raised out of the way, in
/// an unoptimized build on the 2 MiB stack the test harness gives a
/// spawned thread, over the `Node` chain of
/// `matrix/cases/recursive-optional-external`:
///
/// | direction | deepest chain that completed | overflowed at |
/// |---|---|---|
/// | decode | 160 | between 160 and 176 |
/// | encode | 208 | between 208 and 224 |
///
/// The decoder is the tighter of the two and the one that faces a
/// hostile peer, so it sets the number: 64 leaves a factor of about
/// 2.5 against the worst measured case, and more than that in a release
/// build or on a main thread's larger stack.
///
/// It is a bound on legitimate values too, and that is stated rather
/// than hidden: a sample carrying a linked list of more than 64 nodes
/// is refused by this walk. The deepest aggregate nesting in the
/// interop corpus is four. Raising the number means raising the
/// measurement above with it, or making the walks iterative.
///
/// It is deliberately *not* `zerodds_ast_layout::plan::MAX_DEPTH`, which
/// bounds a descent through the **type** graph and answers a different
/// question: the type of a thousand-node list is two levels deep.
pub const MAX_VALUE_DEPTH: usize = 64;

pub use canonical::{Difference, canonicalise};
pub use decode::{decode, decode_continuing, decode_prefix};
pub use default::of_type as default_of_type;
pub use encode::{ProbeError, encode, encode_continuing};
pub use sink::{Aggregate, Sink};
pub use source::{Fields, Source};
pub use value::{Emit, ProbeRequest};

#[cfg(feature = "serde-json")]
pub use json::EXACT_JSON_INTEGER;
