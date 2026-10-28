// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Layout plans. Spec §7.

use crate::key::Encoding;
use alloc::vec::Vec;
use core::fmt;
use zerodds_ast_core::NodeId;

/// EMHEADER length code. OMG XTypes 1.3 §7.4.2.
///
/// The code tells a reader how to skip a member it does not understand,
/// which is what makes `@mutable` evolution work at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LengthCode {
    /// One byte.
    Lc0,
    /// Two bytes.
    Lc1,
    /// Four bytes.
    Lc2,
    /// Eight bytes.
    Lc3,
    /// Length in a following NEXTINT.
    Lc4,
    /// Length as the member's own leading word, which doubles as the
    /// NEXTINT. Required for a variable-length member.
    Lc5,
    /// Length is NEXTINT times four.
    Lc6,
    /// Length is NEXTINT times eight.
    Lc7,
}

impl LengthCode {
    /// Every code XTypes 1.3 defines.
    pub const ALL: [Self; 8] = [
        Self::Lc0,
        Self::Lc1,
        Self::Lc2,
        Self::Lc3,
        Self::Lc4,
        Self::Lc5,
        Self::Lc6,
        Self::Lc7,
    ];

    /// The three-bit code as it appears in the EMHEADER.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::Lc0 => 0,
            Self::Lc1 => 1,
            Self::Lc2 => 2,
            Self::Lc3 => 3,
            Self::Lc4 => 4,
            Self::Lc5 => 5,
            Self::Lc6 => 6,
            Self::Lc7 => 7,
        }
    }

    /// The fixed payload size a code implies, when it implies one.
    #[must_use]
    pub const fn fixed_size(self) -> Option<u32> {
        match self {
            Self::Lc0 => Some(1),
            Self::Lc1 => Some(2),
            Self::Lc2 => Some(4),
            Self::Lc3 => Some(8),
            _ => None,
        }
    }

    /// Whether the member's own leading length word serves as the
    /// NEXTINT, which is what distinguishes LC5 from LC4.
    #[must_use]
    pub const fn reuses_leading_length(self) -> bool {
        matches!(self, Self::Lc5)
    }
}

/// How a member is framed.
///
/// Five shapes, one per rule. The two `Optional*` ones are what
/// §7.4.3.5.1 calls an `OPT_FMEMBER` — an `@optional` member of a type
/// that is **not** `@mutable` — and they are separate variants rather
/// than a flag on the other two because their absence is written and
/// an `MMEMBER`'s absence is not. A `@mutable` member that is left out
/// leaves no trace in the parameter list; an `OPT_FMEMBER` that is left
/// out still occupies its place in the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberHeader {
    /// No header: the member's bytes follow directly.
    ///
    /// `NOPT_FMEMBER`, XTypes 1.3 §7.4.3.5.3 rule (18).
    NoHeader,
    /// PL_CDR1 parameter header for an `MMEMBER`. XTypes 1.3 §7.4.1.2,
    /// rules (24) and (25).
    ///
    /// The short form is `[id:u16][length:u16]`; the extended form is
    /// `[PID_EXTENDED:u16][8:u16][id:u32][length:u32]`. The length field
    /// carries the **unpadded** value length; the padding to the next
    /// four-byte boundary follows the value and is not counted.
    PidCdr1 {
        /// Parameter id.
        pid: u16,
        /// Whether the extended form is required.
        ///
        /// Mandatory when the member id reaches `PID_EXTENDED_THRESHOLD`
        /// **or** the value is longer than `0xFFFF` bytes — the second
        /// condition is easy to miss and makes a long member unreadable
        /// when it is.
        extended: bool,
        /// Whether a reader must discard the sample rather than skip
        /// this member.
        ///
        /// §7.4.1.2.2: `FLAG_MUST_UNDERSTAND` "shall be set if and only
        /// if the must_understand property of the member is set to
        /// true". Bit 14 of the short header's id, `FLAG_2`
        /// (`0x40000000`) of the extended header's `eMemberHeader`.
        must_understand: bool,
    },
    /// PL_CDR2 EMHEADER for an `MMEMBER`. XTypes 1.3 §7.4.2, rule (22).
    EmHeader {
        /// Length code.
        lc: LengthCode,
        /// Whether a reader must fail rather than skip this member.
        must_understand: bool,
    },
    /// An `OPT_FMEMBER` under XCDR1: the same parameter header as an
    /// `MMEMBER`, and **a declared length of zero when the member takes
    /// no value**. XTypes 1.3 §7.4.3.5.3 rule (19), §7.4.1.2.
    ///
    /// The clause is explicit: "If the member takes no value within that
    /// particular object, the data length indicated by the header shall
    /// be zero." Cyclone, RTI and Fast DDS all write the header either
    /// way — `crates/ast-layout/tests/fixtures/vendor_bytes.md` §1.
    /// Omitting it instead leaves the sample four bytes short of what
    /// every peer counts on and puts every following member at the wrong
    /// offset.
    OptionalCdr1 {
        /// Parameter id.
        pid: u16,
        /// Whether the extended form is required, decided by the same
        /// rule [`MemberHeader::PidCdr1`] uses.
        extended: bool,
        /// Whether a reader must discard the sample rather than skip
        /// this member, carried in the same bit and by the same clause
        /// as [`MemberHeader::PidCdr1`]'s.
        ///
        /// §7.4.1.2.2 conditions the flag on the member's property and
        /// not on the enclosing type's extensibility, so an
        /// `@optional @must_understand` member of a `@final` type
        /// carries it too. No vendor row measures that combination;
        /// this is the reading the clause's "if and only if" gives.
        must_understand: bool,
    },
    /// An `OPT_FMEMBER` under XCDR2: a BOOLEAN `<is_present>`, then the
    /// value when it is present. XTypes 1.3 §7.4.3.5.2, rule (20).
    ///
    /// One byte, not an EMHEADER. The distinction is four bytes wide and
    /// moves the value: `@final struct { long a; @optional octet b;
    /// long c; }` puts `b` at offset 5 behind a one-byte flag, where an
    /// EMHEADER would put it at 8.
    OptionalFlag,
}

impl MemberHeader {
    /// Whether the member's absence is written rather than left out.
    ///
    /// True for the two `OPT_FMEMBER` forms and false for everything
    /// else. An `MMEMBER` that is absent contributes nothing to the
    /// parameter list; an `OPT_FMEMBER` that is absent contributes its
    /// header with a length of zero, or its `<is_present>` byte.
    #[must_use]
    pub const fn writes_absence(self) -> bool {
        matches!(self, Self::OptionalCdr1 { .. } | Self::OptionalFlag)
    }
}

/// Where a member sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offset {
    /// A fixed byte offset from the start of the aggregate.
    Fixed(u32),
    /// Determined at runtime, because something before it is
    /// variable-length.
    Dynamic,
}

/// How much space a type takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// Exactly this many bytes, every time.
    Sized(u32),
    /// Depends on the value.
    Unbounded,
}

impl Extent {
    /// The byte count, when the type is fixed-size.
    #[must_use]
    pub const fn sized(self) -> Option<u32> {
        match self {
            Self::Sized(n) => Some(n),
            Self::Unbounded => None,
        }
    }

    /// Whether the type is fixed-size.
    #[must_use]
    pub const fn is_sized(self) -> bool {
        matches!(self, Self::Sized(_))
    }
}

/// One member's place in the wire form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberPlan {
    /// The member this plan is for.
    pub member: NodeId,
    /// Its effective XTypes member id.
    pub member_id: u32,
    /// How it is framed.
    pub header: MemberHeader,
    /// Where it sits.
    pub offset: Offset,
    /// The alignment it requires.
    pub align: u8,
}

/// A type's wire layout under one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLayout {
    /// The type this plan is for.
    pub type_: NodeId,
    /// The alignment the whole type requires.
    pub align: u8,
    /// How much space it takes.
    pub extent: Extent,
    /// Whether a DHEADER precedes the members. XCDR2 only.
    pub needs_dheader: bool,
    /// Whether a `PID_LIST_END` sentinel terminates the parameter list.
    /// PL_CDR1 only; XTypes 1.3 §7.4.1.2.4.
    pub needs_sentinel: bool,
    /// How a `@mutable` union's discriminator is framed, and `None`
    /// everywhere else.
    ///
    /// Rules (27) and (28) frame a `@mutable` union's discriminator
    /// exactly like a member — an `MMEMBER` at member id 0 — but a
    /// discriminator is not a member node, so there is nothing for a
    /// [`MemberPlan`] to name. It gets a field of its own rather than a
    /// synthetic `MemberPlan`, because a synthetic node id is a thing
    /// every consumer would have to know not to look up.
    pub discriminator: Option<MemberHeader>,
    /// The members, in wire order.
    ///
    /// For a union these are the case members, one per case, and only
    /// the selected one is written. Populated only for a `@mutable`
    /// union: rules (26) and (30) give a union of any other
    /// extensibility no per-member framing to plan.
    pub members: Vec<MemberPlan>,
}

impl TypeLayout {
    /// A plan for a type with no members.
    #[must_use]
    pub fn scalar(type_: NodeId, align: u8, extent: Extent) -> Self {
        Self {
            type_,
            align,
            extent,
            needs_dheader: false,
            needs_sentinel: false,
            discriminator: None,
            members: Vec::new(),
        }
    }

    /// The plan for one member, by its identity.
    #[must_use]
    pub fn member(&self, node: NodeId) -> Option<&MemberPlan> {
        self.members.iter().find(|m| m.member == node)
    }
}

/// Why a layout could not be computed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayoutError {
    /// A reference in the type graph is not resolved, so the size and
    /// alignment behind it are unknown. Guessing would produce a plan
    /// that is wrong on the wire, which is worse than no plan.
    Unresolved {
        /// The type whose reference could not be followed.
        node: NodeId,
    },
    /// The construct has no representation in the requested encoding.
    UnsupportedForEncoding {
        /// The offending type.
        node: NodeId,
        /// The encoding asked for.
        encoding: Encoding,
        /// Why it does not fit.
        reason: &'static str,
    },
    /// A key-holder plan was asked for in little-endian. The key holder
    /// is defined big-endian; honouring the request would produce a
    /// wrong hash. DDSI-RTPS 2.5 §9.6.3.8.
    KeyMustBeBigEndian,
    /// The type graph nests deeper than the planner will follow.
    TooDeep {
        /// Where the descent stopped.
        node: NodeId,
    },
    /// A member's type closes a cycle with nothing on it that can
    /// terminate a value.
    ///
    /// IDL 4.2 §7.4.1.4.4.4.4 admits a recursive structure or union
    /// "via members that have a sequence type" — "an incomplete type
    /// can only appear as the element type of a sequence definition" —
    /// because the element count is what makes a value finite. A map
    /// carries a count the same way, and an `@optional` member carries
    /// the same property in the other form: the member can be absent,
    /// and an absent member ends the value where a zero-length sequence
    /// would. A cycle over any of the three is planned; a cycle over
    /// none of them has no finite value at all and is refused here, by
    /// name rather than by a depth limit that would report a number and
    /// not a reason. An array is the case that fails it: its element
    /// count is fixed and non-zero, so nothing about a value can stop
    /// the descent.
    ///
    /// XTypes 1.3 §7.3.1.2.1.4 is not a second ground for this refusal.
    /// It refuses a *cyclic value* — "if the graph of objects has
    /// cycles, it is not serializable" — and a recursive type whose
    /// values are trees has none. That distinction is a value-time
    /// question and is answered at value time; see
    /// `zerodds_wire_codec::MAX_VALUE_DEPTH`.
    RecursionOutsideCollection {
        /// The member whose type names an enclosing type.
        node: NodeId,
    },
    /// The model rejected a lookup.
    Model(zerodds_ast::AstError),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unresolved { node } => {
                write!(f, "type {node:?} has an unresolved reference")
            }
            Self::UnsupportedForEncoding {
                node,
                encoding,
                reason,
            } => write!(f, "type {node:?} cannot be {encoding:?}: {reason}"),
            Self::KeyMustBeBigEndian => {
                write!(f, "the key holder is defined big-endian")
            }
            Self::TooDeep { node } => write!(f, "type graph below {node:?} nests too deeply"),
            Self::RecursionOutsideCollection { node } => write!(
                f,
                "{node:?} closes a type cycle with nothing on it that terminates a value — no \
                 sequence, no map and no @optional member — which IDL 4.2 \
                 \u{a7}7.4.1.4.4.4.4 does not admit; XTypes 1.3 \u{a7}7.3.1.2.1.4 refuses only a \
                 cyclic value, which is a different question and is answered at value time"
            ),
            Self::Model(error) => write!(f, "{error}"),
        }
    }
}

impl core::error::Error for LayoutError {}

impl From<zerodds_ast::AstError> for LayoutError {
    fn from(error: zerodds_ast::AstError) -> Self {
        Self::Model(error)
    }
}

/// How deep the planner follows a type graph.
///
/// A recursive type is legal in the model, and its layout is unbounded
/// by construction; the limit stops the descent rather than letting a
/// hostile model run it forever.
pub const MAX_DEPTH: usize = 64;

/// Round `offset` up to the next multiple of `align`.
#[must_use]
pub const fn align_to(offset: u32, align: u8) -> u32 {
    let align = align as u32;
    if align <= 1 {
        return offset;
    }
    let remainder = offset % align;
    if remainder == 0 {
        offset
    } else {
        offset.saturating_add(align - remainder)
    }
}

// -- the resolved scalar write ------------------------------------------

/// What one scalar position puts on the wire, with every policy already
/// answered.
///
/// The plan's other half. [`MemberPlan`] says where a member sits and
/// how it is framed; this says what the value itself is, and the two
/// together leave a code generator nothing to decide.
///
/// ## Why this exists
///
/// Twelve wire questions used to be answered where the code was
/// generated. `zerodds-ast-layout` held the *rules* — `enumeration_bit_bound`,
/// `enumerator_wire_value` and their neighbours — but every generator
/// had to remember to ask them, and five of them did, and a test counted
/// the call sites to notice when one stopped.
///
/// A counting test catches a site that stops asking. It does not catch a
/// site that never asked, and it does not catch a site that asks with
/// the wrong argument — `WcharFraming` is asked ten times, each with a
/// position, and nine right against one wrong is invisible to a counter
/// and a different octet on the wire.
///
/// So the answer moves here. A generator matches on this and calls the
/// runtime; there is no axis left for it to read and no argument for it
/// to get wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarWrite {
    /// One octet, 0 or 1.
    Bool,
    /// An unsigned octet.
    U8,
    /// A signed octet.
    I8,
    /// A code point kept to its low octet.
    Char,
    /// A code point, with the three policies a `wchar` carries.
    ///
    /// The width is its own setting and not a reading of the `wstring`
    /// policy: `rmw_cyclonedds_cpp` writes two octets for a scalar
    /// `wchar` and four inside a `wstring`.
    Wchar {
        /// Two octets or four.
        width: crate::key::WcharWidth,
        /// Whether GIOP's length octet precedes it **at this position**.
        /// §7.4.1.1.2; the resolved answer, not the policy.
        length_octet: bool,
        /// Which order its code units go in.
        order: crate::key::WstringUnitOrder,
    },
    /// A signed 16-bit integer.
    I16,
    /// An unsigned 16-bit integer.
    U16,
    /// A signed 32-bit integer.
    I32,
    /// An unsigned 32-bit integer.
    U32,
    /// A signed 64-bit integer.
    I64,
    /// An unsigned 64-bit integer.
    U64,
    /// Binary32.
    F32,
    /// Binary64.
    F64,
    /// Binary128.
    F128,
    /// A narrow string: a length and the octets.
    Str,
    /// A wide string, with the two policies its length and units carry.
    Wstr {
        /// What the length counts and whether a terminator follows.
        policy: crate::key::WstringLengthPolicy,
        /// Which order its code units go in.
        order: crate::key::WstringUnitOrder,
        /// What its characters are — UTF-16 code units or their own
        /// UTF-8 octets. Under [`crate::key::WstringCodeUnits::Utf8`]
        /// the length is the UTF-8 octet count and `policy`'s C1 and C3
        /// have nothing to say; only the terminator still reaches the
        /// wire. Resolved here so the walk reads no axis.
        code_units: crate::key::WstringCodeUnits,
    },
    /// A packed decimal.
    Fixed {
        /// The digits the type declared.
        digits: u8,
        /// The scale the type declared.
        scale: u8,
        /// Whether the wire carries the declared digit count or the
        /// value's own. §9.3.2.8 gives the packing and not the count,
        /// and the four measured implementations split two and two.
        count: crate::key::FixedDigitCount,
    },
    /// An enumeration, in a holder of the resolved width.
    Enum {
        /// How many bits the holder has, after the policy that decides
        /// whether a front end kept the declared `@bit_bound`.
        bits: u16,
    },
    /// A bitmask, in the holder Table 33 gives for its bound.
    Bitmask {
        /// How many bits the holder has.
        bits: u16,
    },
}

/// The scalar write one position asks for, resolved against the key.
///
/// `None` for a type that is not a scalar — a string, a collection, an
/// aggregate — which the caller frames itself and whose parts come back
/// through here.
///
/// `position` is the one thing a caller still supplies, because it is
/// not a property of the type: the same `wchar` is framed differently as
/// a member and as an element of a sequence, and only the caller knows
/// which of the two it is looking at.
#[must_use]
pub fn scalar_write(
    ast: &zerodds_ast::Ast,
    type_: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    key: crate::key::LayoutKey,
    position: crate::key::WcharPosition,
) -> Option<ScalarWrite> {
    use zerodds_ast::{PrimitiveKind, TypeKind};
    let mut current = type_;
    // Through the aliases, because a `typedef` writes what its target
    // writes and a caller that stopped at the alias would find no
    // scalar where there is one.
    for _ in 0..32 {
        let node = ast.type_(current).ok()?;
        match &node.kind {
            TypeKind::Alias { aliased } => current = aliased.resolved()?,
            TypeKind::Primitive(kind) => {
                return Some(match kind {
                    PrimitiveKind::Boolean => ScalarWrite::Bool,
                    PrimitiveKind::Octet | PrimitiveKind::UInt8 => ScalarWrite::U8,
                    PrimitiveKind::Int8 => ScalarWrite::I8,
                    PrimitiveKind::Char => ScalarWrite::Char,
                    PrimitiveKind::WChar => ScalarWrite::Wchar {
                        width: key.wchar_width,
                        length_octet: key.wchar_framing.has_length_octet(position),
                        order: key.wstring_unit_order,
                    },
                    PrimitiveKind::Int16 => ScalarWrite::I16,
                    PrimitiveKind::UInt16 => ScalarWrite::U16,
                    PrimitiveKind::Int32 => ScalarWrite::I32,
                    PrimitiveKind::UInt32 => ScalarWrite::U32,
                    PrimitiveKind::Int64 => ScalarWrite::I64,
                    PrimitiveKind::UInt64 => ScalarWrite::U64,
                    PrimitiveKind::Float32 => ScalarWrite::F32,
                    PrimitiveKind::Float64 => ScalarWrite::F64,
                    PrimitiveKind::Float128 => ScalarWrite::F128,
                });
            }
            TypeKind::String {
                width: zerodds_ast::CharWidth::Char,
                ..
            } => return Some(ScalarWrite::Str),
            TypeKind::String {
                width: zerodds_ast::CharWidth::WChar,
                ..
            } => {
                return Some(ScalarWrite::Wstr {
                    policy: key.wstring_length,
                    order: key.wstring_unit_order,
                    code_units: key.wstring_code_units,
                });
            }
            TypeKind::Fixed { digits, scale } => {
                return Some(ScalarWrite::Fixed {
                    digits: *digits,
                    scale: *scale,
                    count: key.fixed_digits,
                });
            }
            TypeKind::Enumeration { .. } => {
                return Some(ScalarWrite::Enum {
                    bits: crate::xcdr::enumeration_bit_bound(ast, current, key.enum_holder),
                });
            }
            TypeKind::Bitmask { .. } => {
                return Some(ScalarWrite::Bitmask {
                    bits: ast.effective_bit_bound(current),
                });
            }
            _ => return None,
        }
    }
    None
}

/// What each enumerator of `type_` puts on the wire, resolved.
///
/// The mapping and not the policy. `enumerator_wire_value` is the rule
/// and every generator used to call it with `key.enumerator_value`;
/// asking here means a caller cannot pass the wrong policy, cannot skip
/// the crossing, and cannot answer the axis by omission.
///
/// Ordered by declaration, which is the order both directions take:
/// where two enumerators share a value, the first one wins in each.
///
/// # Errors
/// The name of an enumerator whose wire value does not evaluate under
/// this setting — never a mapping with one silently left out.
pub fn enumerator_writes(
    ast: &zerodds_ast::Ast,
    type_: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    key: crate::key::LayoutKey,
) -> Result<alloc::vec::Vec<(alloc::string::String, i128)>, alloc::string::String> {
    let mut out = alloc::vec::Vec::new();
    for enumerator in ast.enumerators_of(type_) {
        let node = ast.enumerator(*enumerator).map_err(|_| {
            alloc::string::String::from("an enumerator handle the model does not hold")
        })?;
        let on_wire = crate::xcdr::enumerator_wire_value(ast, *enumerator, key.enumerator_value)
            .ok_or_else(|| node.name.clone())?;
        out.push((node.name.clone(), on_wire));
    }
    Ok(out)
}

/// Whether an enumeration's declared values reach the wire unchanged.
///
/// True where the setting is the specification's, and the reason the
/// generated code is a single call rather than a chain: every
/// enumerator already carries the integer it serialises as.
#[must_use]
pub fn enumerators_are_declared(key: crate::key::LayoutKey) -> bool {
    key.enumerator_value.is_default()
}

/// How a `map`'s entries are ordered on the wire, resolved.
///
/// `Ok(None)` is declaration order — the entries go where the sample put
/// them. `Ok(Some(_))` is the ordering a key comparison gives.
/// `Err(())` is the one case a caller has to refuse: a setting asked for
/// ascending order over a key type that has **no value order**.
///
/// The refusal is here and not at the generator because it is the same
/// refusal in every column, and a generator that decided it for itself
/// would be answering a wire question.
///
/// # Errors
/// [`NoKeyOrder`] when ascending order was asked for and the key type
/// has none.
pub fn map_entry_ordering(
    ast: &zerodds_ast::Ast,
    type_: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    key: crate::key::LayoutKey,
) -> Result<Option<crate::key::MapKeyOrdering>, NoKeyOrder> {
    let ordering = crate::map_key_ordering(ast, type_, key.map_entries);
    if !key.map_entries.is_default() && ordering.is_none() {
        return Err(NoKeyOrder);
    }
    Ok(ordering)
}

/// A `map` asked for ascending entry order over a key type that has
/// **no value order**.
///
/// Its own type rather than `()`, so a caller that ignores it has to
/// say so and a reader of the signature knows what went wrong without
/// finding the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoKeyOrder;

/// Whether a negative case label of this union selects nothing.
///
/// One setting reproduces a peer whose real `union` narrows the
/// discriminator before comparing, so a negative label of a signed type
/// narrower than 32 bits matches no value. The resolved answer, so a
/// generator neither re-derives it nor passes the wrong axis.
#[must_use]
pub fn union_negative_selects_nothing(
    ast: &zerodds_ast::Ast,
    type_: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    key: crate::key::LayoutKey,
) -> bool {
    crate::xcdr::union_negative_matches_nothing(ast, type_, key.union_label)
}

/// How wide the holder of `type_`'s enumeration is, resolved.
///
/// The same answer [`ScalarWrite::Enum`] carries, for the two callers
/// that need the width before they have an instruction — a reader
/// sizing the integer it compares patterns against.
#[must_use]
pub fn enumeration_holder_bits(
    ast: &zerodds_ast::Ast,
    type_: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    key: crate::key::LayoutKey,
) -> u16 {
    crate::xcdr::enumeration_bit_bound(ast, type_, key.enum_holder)
}
