// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR1 and XCDR2 layout planning. OMG XTypes 1.3 §7.4.
//!
//! Both versions live here because they differ in a handful of rules and
//! agree on everything else; splitting them into two files invites the
//! shared parts to drift. Where they differ, the difference is named:
//!
//! - 64-bit primitives align to 8 under XCDR1 and to 4 under XCDR2
//!   (§7.4.2), so that a delimited type can be skipped without reading
//!   its contents.
//! - XCDR1 has no delimiter header, so `@appendable` plans none.
//! - `@mutable` frames with PID headers under XCDR1 and EMHEADERs under
//!   XCDR2.

use crate::key::{
    CollectionDelimiterPolicy, Endianness, EnumHolderWidth, EnumeratorValue, FixedDigitCount,
    KeyHolderExtensibility, LayoutKey, LengthCodePolicy, MapDelimiter, MapEntryOrder,
    MapKeyOrdering, MustUnderstandPolicy, PlCdr1HeaderPolicy, Purpose, ShortPidRange,
    UnionLabelMatch, WcharPosition,
};
use crate::plan::{
    Extent, LayoutError, LengthCode, MAX_DEPTH, MemberHeader, MemberPlan, Offset, TypeLayout,
    align_to,
};
use alloc::vec::Vec;
use zerodds_ast::{Ast, CharWidth, Extensibility, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};

/// The highest member id a short PL_CDR1 parameter header can carry.
///
/// XTypes 1.3 §7.4.1.2.2 gives the range as "from 0 to 16,128 (0x3F00)
/// **inclusive**", and 0x3F01 is `PID_EXTENDED` itself, so 0x3F00 is the
/// last id below the reserved range and not the first inside it.
///
/// The constant was read as an exclusive threshold and named for one,
/// which cost a member with id 0x3F00 the twelve-octet header instead of
/// the four-octet one. Measured 2026-08-10, case `pid_3f00`,
/// `@mutable struct PidEdge { @id(16127) long a; @id(16128) long b; };`
/// XCDR1 little endian: RTI Connext 7.7 writes
/// `ff3e0400 07000000 003f0400 09000000 027f0000` and Fast DDS the same
/// with its own sentinel — the short form for both ids. Cyclone writes
/// the extended form for every member whatever its id is, so it does not
/// settle the edge.
pub const PID_SHORT_MAX_ID: u16 = 0x3f00;

/// `PID_EXTENDED` — the marker that introduces the long header.
/// XTypes 1.3 §7.4.1.2.2.
pub const PID_EXTENDED: u16 = 0x3f01;

/// `PID_LIST_END` — the sentinel terminating a PL_CDR1 parameter list.
/// XTypes 1.3 Table 34 (§7.4.1.2.1).
pub const PID_LIST_END: u16 = 0x3f02;

/// The longest value a short PL_CDR1 header can describe. A member
/// longer than this needs the extended form whatever its id is.
pub const PID_SHORT_MAX_LEN: u32 = 0xffff;

/// Whether a collection of `element` carries a `DHEADER` under `key`.
///
/// Public because the code generator asks the same question and must
/// get the same answer: a planner and an emitter that each decide this
/// for themselves are one edit away from writing bytes the plan does
/// not describe.
///
/// XTypes 1.3 §7.4.3.5 rules (8) through (13): a collection whose
/// element type is primitive serialises the same way under both
/// versions, and every other collection gets a `DHEADER` under XCDR2.
/// "Primitive" is §7.2.2.2 — the built-in scalars. An enumeration is a
/// constructed type, so `sequence<Colour>` is delimited.
///
/// # Where the vendors part
///
/// Cyclone DDS applies the rule as written:
/// `src/core/cdr/src/dds_cdrstream.c:571`,
/// `is_dheader_needed = !is_primitive_type(type) && xcdrv ==
/// VERSION_2`, with `is_primitive_type` covering the 1/2/4/8-byte
/// scalars plus boolean and wchar, and deliberately *not* enum.
///
/// RTI Connext 7.7 omits the delimiter for an array of a `@final`
/// fixed-size struct. Measured on 2026-08-09 with
/// `@final struct Cell { long a; long b; }; @final struct ArrN
/// { Cell items[2]; };` — Cyclone writes 20 bytes with a leading
/// `10000000`, RTI writes 16 with none. The run is in
/// `tests/fixtures/vendor_bytes.md`. The planner follows the spec and
/// Cyclone by default; [`CollectionDelimiterPolicy`] selects the other
/// two shapes.
#[must_use]
pub fn collection_is_delimited(ast: &Ast, element: Id<TypeNode>, key: LayoutKey) -> bool {
    if !key.encoding.is_v2() {
        return false;
    }
    match key.collection_delimiters {
        CollectionDelimiterPolicy::Never => false,
        CollectionDelimiterPolicy::Specification => !is_primitive_after_aliases(ast, element),
        CollectionDelimiterPolicy::EnumIsPrimitive => {
            !is_primitive_after_aliases(ast, element)
                && !is_enumeration_or_bitmask_after_aliases(ast, element)
        }
    }
}

/// Whether `type_` is an enumeration or a bitmask once typedefs are seen
/// through.
///
/// Only [`CollectionDelimiterPolicy::EnumIsPrimitive`] asks. The bitmask
/// used to be left out, because nothing measured said what a vendor did
/// with a collection of one. Measured on 2026-08-09 with
/// `@bit_bound(16) bitmask Flags { @position(0) a, @position(3) b };
/// @mutable struct { sequence<Flags> fs; }`: Cyclone writes the
/// DHEADER — `01000050 08000000 02000000 01000800` — and Fast DDS
/// writes none, `01000030 02000000 01000800`. Fast DDS exempts a
/// bitmask exactly as it exempts an enumeration, which is what this
/// policy is named after.
fn is_enumeration_or_bitmask_after_aliases(ast: &Ast, type_: Id<TypeNode>) -> bool {
    let mut current = type_;
    for _ in 0..MAX_DEPTH {
        let Ok(node) = ast.type_(current) else {
            return false;
        };
        match &node.kind {
            TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => return true,
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => current = next,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// Whether an element of a `sequence` is preceded by its own octet
/// length under this key.
///
/// [`crate::key::SequenceElementDelimiter::BeforeEachNonFinalElement`] is the
/// measured departure: a four-octet `UInt32` in front of every element
/// whose type is not `@final`, under both encoding versions. A type
/// with no extensibility of its own — a primitive, a string, an
/// enumeration, a `typedef` — is `@final` by construction and takes
/// none; only the four aggregate kinds can be `@appendable` or
/// `@mutable`.
///
/// The alias is **not** seen through, and that is the measurement.
/// `recursive-typedef-sequence` is `sequence<NodeRef>` with
/// `typedef Node NodeRef` over an `@appendable Node`, and HDDS writes
/// no element length: its generated sequence encoder decides on the
/// declared element type, and a `typedef` carries no `@final`/
/// `@appendable` of its own. A `sequence<Later>` of a forward-declared
/// structure, and a `sequence<Node>` of the structure itself, both carry
/// it.
///
/// `None` for an element id the model does not hold: whether the element
/// carries a length cannot be told, and guessing either way frames every
/// element of the sequence wrong.
///
/// `zerodds_wire_codec`'s sequence walk asks this, and so do the
/// generated Rust codec and the generic emitters of the other twenty
/// columns, so none of them can answer differently about the same
/// element.
#[must_use]
pub fn sequence_element_carries_length(
    ast: &Ast,
    element: Id<TypeNode>,
    key: LayoutKey,
) -> Option<bool> {
    if !key.sequence_element_delimiter.writes_element_length() {
        return Some(false);
    }
    let node = ast.type_(element).ok()?;
    Some(match &node.kind {
        TypeKind::Structure { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. } => extensibility_of(ast, node, key) != Extensibility::Final,
        _ => false,
    })
}

/// The array form of [`collection_is_delimited`] — the element is
/// flattened through aliases *and further arrays* before the policy
/// looks at it.
///
/// XTypes models a multi-dimensional array as one ARRAY_TYPE over the
/// base element with a sequence of bounds (`PlainArrayLElemDefn`'s
/// `array_bound_seq`), so `typedef long Row[3]; Row grid[2];` is an
/// array of `long` and rules (11)–(13) of §7.4.3.5.3 see a primitive
/// element. An answer taken from the unflattened element wrote an
/// inner DHEADER no implementation writes — measured 2026-08-24 on
/// `resolve-array-of-typedef-array`: Cyclone DDS and Fast DDS both
/// serialise the `@appendable` holder as 28 octets with one struct
/// DHEADER, the planner wrote 32 with an array DHEADER inside.
///
/// A *sequence* is not flattened. Its element stays what it is:
/// `sequence<Triple>` keeps the DHEADER, measured on
/// `sequence-of-array` — Fast DDS writes it and the matrix cell
/// byte-matches.
#[must_use]
pub fn array_is_delimited(ast: &Ast, element: Id<TypeNode>, key: LayoutKey) -> bool {
    // OpenDDS's reading: the element is taken as declared, so an alias
    // of an array is a constructed type and the array keeps its
    // DHEADER. `ArrayElementFlattening` carries the measurement.
    if matches!(
        key.array_element_flattening,
        crate::key::ArrayElementFlattening::StopsAtAlias
    ) {
        return collection_is_delimited(ast, element, key);
    }
    let mut current = element;
    for _ in 0..MAX_DEPTH {
        let Ok(node) = ast.type_(current) else {
            break;
        };
        match &node.kind {
            TypeKind::Alias { aliased }
            | TypeKind::Array {
                element: aliased, ..
            } => match aliased.resolved() {
                Some(next) => current = next,
                None => break,
            },
            _ => break,
        }
    }
    collection_is_delimited(ast, current, key)
}

/// Whether `type_` is a primitive once typedefs are seen through.
///
/// A `typedef` is transparent on the wire, so `typedef long L;
/// sequence<L>` is a sequence of a primitive and takes no delimiter.
fn is_primitive_after_aliases(ast: &Ast, type_: Id<TypeNode>) -> bool {
    let mut current = type_;
    for _ in 0..MAX_DEPTH {
        let Ok(node) = ast.type_(current) else {
            return false;
        };
        match &node.kind {
            TypeKind::Primitive(_) => return true,
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => current = next,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// PL_CDR1 pads every member value to this boundary. The length field
/// carries the unpadded length; the padding follows it uncounted.
pub const PL_CDR1_ALIGNMENT: u8 = 4;

/// A sequence is introduced by a four-octet element count, which is what
/// fixes where the sequence itself may start.
const SEQUENCE_LENGTH_ALIGNMENT: u8 = 4;

/// The type a chain of aliases ends at, or the type itself when it is
/// not an alias or the chain does not resolve.
fn through_aliases(ast: &Ast, type_: Id<TypeNode>) -> Id<TypeNode> {
    let mut current = type_;
    for _ in 0..MAX_DEPTH {
        let Ok(node) = ast.type_(current) else {
            return current;
        };
        match &node.kind {
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => current = next,
                None => return current,
            },
            _ => return current,
        }
    }
    current
}

/// Whether planning `element` would re-enter a type this descent is
/// already inside.
///
/// Asked at a terminator only — a collection boundary, or an
/// `@optional` member. That is where a self-naming type is legal and
/// finite, and it is also the only place a cycle can be broken without
/// losing information: everywhere else a type that contains itself has
/// no finite size, and the descent limit reporting it as
/// [`LayoutError::TooDeep`] is the right answer.
///
/// The alias chain is followed because `sequence<Ref>` with
/// `typedef Node Ref` is the same cycle written differently, and the raw
/// id would not match.
fn closes_a_cycle(ast: &Ast, element: Id<TypeNode>, deps: &DepCollector) -> bool {
    deps.is_active(element) || deps.is_active(through_aliases(ast, element))
}

/// Whether planning `member_type` would close a cycle that crossed no
/// terminator on the way — no sequence, no map and no `@optional`
/// member — which is the recursion IDL 4.2 §7.4.1.4.4.4.4 does not
/// admit. A re-occurrence *behind* a barrier is the legal kind and is
/// not reported here; the sequence, map and `@optional` arms break
/// those with [`closes_a_cycle`] instead of descending.
fn closes_a_cycle_without_length(
    ast: &Ast,
    member_type: Id<TypeNode>,
    deps: &DepCollector,
) -> bool {
    deps.is_active_without_barrier(member_type)
        || deps.is_active_without_barrier(through_aliases(ast, member_type))
}

/// Whether an external member can eventually hold a finite value.
/// `@external` is storage indirection, not optionality: a required self
/// pointer alone does not terminate a wire value. A union alternative can.
/// The active path rejects circular witnesses; each successful aggregate
/// needs all required members, while a union needs one terminating branch.
///
/// zerodds-lint: recursion-depth 64 (type-value witness; bounded by MAX_DEPTH)
fn has_finite_value(
    ast: &Ast,
    type_: Id<TypeNode>,
    active: &mut alloc::collections::BTreeSet<Id<TypeNode>>,
    depth: usize,
) -> bool {
    let type_ = through_aliases(ast, type_);
    if depth > MAX_DEPTH || !active.insert(type_) {
        return false;
    }
    let finite = ast.type_(type_).is_ok_and(|node| match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            ast.all_members_of(type_).into_iter().all(|member| {
                ast.member(member).is_ok_and(|member| {
                    member.facets.is_optional()
                        || member
                            .type_
                            .resolved()
                            .is_some_and(|type_| has_finite_value(ast, type_, active, depth + 1))
                })
            })
        }
        TypeKind::Union { .. } => ast.cases_of(type_).iter().any(|case| {
            ast.case(*case)
                .ok()
                .and_then(|case| ast.member(case.member).ok())
                .and_then(|member| member.type_.resolved())
                .is_some_and(|type_| has_finite_value(ast, type_, active, depth + 1))
        }),
        TypeKind::Array { element, .. } => element
            .resolved()
            .is_some_and(|type_| has_finite_value(ast, type_, active, depth + 1)),
        TypeKind::Primitive(_)
        | TypeKind::Fixed { .. }
        | TypeKind::String { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Map { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. } => true,
        // Aliases left by through_aliases are unresolved or cyclic.
        // Other kinds are not used as witnesses for this XTypes rule.
        _ => false,
    });
    active.remove(&type_);
    finite
}

fn finite_external_member(ast: &Ast, member: &zerodds_ast::MemberNode) -> bool {
    member.facets.xtypes.as_ref().is_some_and(|x| x.is_external)
        && member.type_.resolved().is_some_and(|type_| {
            has_finite_value(ast, type_, &mut alloc::collections::BTreeSet::new(), 0)
        })
}

/// Collects the types a plan consulted, so the cache can invalidate
/// precisely rather than wholesale, and carries the descent path so a
/// recursive type is recognised rather than followed forever.
///
/// The two are separate: `seen` only grows and answers "what does this
/// plan depend on", while `active` is pushed and popped with the descent
/// and answers "am I already inside this type". A type appears in
/// `active` at most once — that is the whole cycle test.
#[derive(Debug, Clone, Default)]
pub struct DepCollector {
    seen: Vec<NodeId>,
    /// Each active type with the number of terminator barriers —
    /// sequence or map descents, `@optional` members and productive
    /// external indirections — crossed before it was entered.
    active: Vec<(Id<TypeNode>, usize)>,
    /// How many terminators the walk is currently inside: sequence and
    /// map element descents, optional members and external indirections
    /// whose target has a finite-value witness. A cycle is legal
    /// when at least one of these lies between a type and its
    /// re-occurrence, because each of them is a place where a *value*
    /// can stop — a length of zero, or an absence. IDL 4.2
    /// §7.4.1.4.4.4.4 names the collection half; the `@optional` half
    /// is the same property in the form XTypes 1.3 §7.3.1.2.1.3 gives
    /// it, and Fast DDS carries it (`recursive-optional-external`,
    /// measured 2026-08-26).
    barriers: usize,
}

impl DepCollector {
    /// Empty collector.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            seen: Vec::new(),
            active: Vec::new(),
            barriers: 0,
        }
    }

    /// Note that the plan depends on a type.
    pub fn note(&mut self, node: NodeId) {
        if !self.seen.contains(&node) {
            self.seen.push(node);
        }
    }

    /// Whether the descent is already inside this type — i.e. planning it
    /// again would be a cycle.
    fn is_active(&self, type_: Id<TypeNode>) -> bool {
        self.active.iter().any(|(id, _)| *id == type_)
    }

    /// Whether the descent is inside this type with no sequence or map
    /// crossed since — the cycle that has no length to sit behind.
    fn is_active_without_barrier(&self, type_: Id<TypeNode>) -> bool {
        self.active
            .iter()
            .any(|(id, at)| *id == type_ && *at == self.barriers)
    }

    /// The types the plan consulted.
    #[must_use]
    pub fn into_deps(self) -> Vec<NodeId> {
        self.seen
    }
}

/// Alignment and extent of a primitive under a layout key.
///
/// The two encoding versions differ only for the eight-byte kinds. One
/// kind takes its width from the key rather than from Table 31:
/// `Char16`, whose width is
/// [`crate::key::WstringLengthPolicy::char16_size`]
/// because RTI Connext writes four bytes under XCDR1 and two under
/// XCDR2, in a `wstring` and as a scalar alike. The key and not the
/// encoding is the argument for that reason — a plan that read the
/// width off the encoding could not tell `Dialect::Rti` from
/// `Dialect::Cyclone`, and both would be wrong about one of them.
///
/// `position` is the second such argument and for the same reason: TAO
/// frames a `wchar` member and a `wchar` collection element
/// differently, so a plan that did not know which it was asked about
/// would be wrong about one of them. [`WcharPosition`] carries the
/// measurement.
#[must_use]
pub fn primitive_layout(kind: PrimitiveKind, key: LayoutKey, position: WcharPosition) -> (u8, u32) {
    // A `wchar` is the one primitive whose alignment is not its size.
    // Under CORBA §9.3.1.6's GIOP 1.2 framing it is an octet count and
    // then that many octets, which aligns to one — so it is answered
    // here, before the MAXALIGN cap the rest of this function applies,
    // rather than through it. `@final struct { octet head; wchar c; }` puts `c` at
    // offset 1 under an ORB and at offset 2 under every DDS
    // implementation, and every member behind it moves with it.
    if kind == PrimitiveKind::WChar && key.wchar_framing.has_length_octet(position) {
        // The pair comes back already narrow, so nothing here converts an
        // alignment with a fallback: a fallback would answer some
        // plausible alignment for a width it did not recognise, and a
        // wrong alignment is a wrong offset for every member behind this
        // one.
        return key
            .wchar_framing
            .scalar_layout(position, key.wchar_width.octets());
    }
    let size = match kind {
        PrimitiveKind::Boolean
        | PrimitiveKind::Octet
        | PrimitiveKind::Char
        | PrimitiveKind::Int8
        | PrimitiveKind::UInt8 => 1,
        PrimitiveKind::Int16 | PrimitiveKind::UInt16 => 2,
        // Two by Table 31, four under RTI's XCDR1 form and under both
        // ROS type supports' `wstring` — but **not** under
        // `rmw_cyclonedds_cpp`'s scalar, which is why the width is its
        // own setting and no longer read off the `wstring` policy. See
        // `WcharWidth`.
        PrimitiveKind::WChar => u32::from(key.wchar_width.octets()),
        PrimitiveKind::Int32 | PrimitiveKind::UInt32 | PrimitiveKind::Float32 => 4,
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 | PrimitiveKind::Float64 => 8,
        // `long double` is sixteen bytes on the wire, aligned like an
        // eight-byte primitive. XTypes 1.3 §7.4.1.1.
        PrimitiveKind::Float128 => 16,
    };
    // Alignment is the primitive's own size, capped at the encoding's
    // MAXALIGN — eight under XCDR1, four under XCDR2. XTypes 1.3 §7.4.1,
    // and see `Encoding::max_alignment` for the vendor measurements.
    let align = size.min(u32::from(key.encoding.max_alignment()));
    (u8::try_from(align).unwrap_or(1), size)
}

/// Every type in `ast` that closes a type cycle IDL 4.2 §7.4.1.4.4.4.4
/// does not admit, with the node the refusal names.
///
/// One rule, two callers. The rule already existed and already cited the
/// clause — [`LayoutError::RecursionOutsideCollection`], raised by
/// [`plan`] from the aggregate, union and array arms — but it sat in the
/// layout stage, so `zerodds-idlc check` said nothing about
/// `struct Sa { Sa m; };` and only `generate` refused it. Somebody using
/// `check` to ask whether a file is valid IDL did not find out.
///
/// This is deliberately **not** a second implementation of the cycle
/// test. It runs the same [`plan`] walk and keeps only that one error,
/// because a rule written twice is a rule that drifts, and the walk is
/// where the barrier bookkeeping lives: a cycle is legal exactly when a
/// sequence, a map or an `@optional` member lies on it, and only the
/// descent knows which of those it crossed.
///
/// Every other [`LayoutError`] is dropped. They are real, and they are
/// not this question: an unresolved reference is the resolve pass's to
/// report, and an encoding that cannot carry a type is `generate`'s.
/// Answering more than was asked here would make `check` refuse files
/// over a backend's limits.
///
/// The types are returned in the model's own order, deduplicated, so the
/// result is stable across runs.
#[must_use]
pub fn types_that_close_an_inadmissible_cycle(
    ast: &Ast,
    key: LayoutKey,
) -> Vec<(Id<TypeNode>, NodeId)> {
    let mut found = Vec::new();
    let mut seen = alloc::collections::BTreeSet::new();
    for (id, _) in ast.types() {
        let mut deps = DepCollector::new();
        if let Err(LayoutError::RecursionOutsideCollection { node }) = plan(ast, id, key, &mut deps)
        {
            if seen.insert(node) {
                found.push((id, node));
            }
        }
    }
    found
}

/// Whether holding a member of `member_type` *inside* `enclosing` would
/// make `enclosing` contain itself.
///
/// The companion question to
/// [`types_that_close_an_inadmissible_cycle`], and deliberately a
/// different one. That function asks what makes a **value** finite on
/// the wire, and IDL 4.2 §7.4.1.4.4.4.4 together with XTypes 1.3
/// §7.3.1.2.1.3 answers with three terminators: a sequence length, a
/// map length, and an `@optional` member that can be absent. This asks
/// what makes a **storage layout** finite in a binding whose member is
/// the value itself, and there the three part company: a collection is
/// reached through the buffer it points at, so it terminates a layout
/// as well, while an `@optional` member is a flag beside an inline
/// value in such a binding and terminates nothing. So the barrier set
/// here is the collection half of that clause alone — `sequence`, `map`
/// and `string`, each of which is a pointer in the binding and holds
/// its content elsewhere.
///
/// The two are not one rule written twice. They are one rule asked
/// under two barrier sets, and the set is the whole of the difference:
/// drop `@optional` from what terminates and the same reachability
/// question answers where a binding has to hold a member behind an
/// indirection. A member this reports is exactly a member whose value
/// has no place inside its own aggregate.
///
/// It is sound against the planner: a cycle the planner admits carries
/// at least one `@optional` member (it refuses every other kind by
/// name), and going round such a cycle each member on it reaches its
/// own aggregate through nothing but inline edges — so every member on
/// it is reported here, and breaking them all breaks the cycle. A cycle
/// that crosses a collection is finite in storage already and is
/// reported for no member.
///
/// The alias chain is followed on both sides, for the reason
/// `closes_a_cycle` gives: `typedef Node Ref` names the same type and
/// the raw id would not match.
#[must_use]
pub fn member_held_inline_closes_a_cycle(
    ast: &Ast,
    enclosing: Id<TypeNode>,
    member_type: Id<TypeNode>,
) -> bool {
    let target = through_aliases(ast, enclosing);
    let mut seen = alloc::collections::BTreeSet::new();
    reaches_inline(ast, member_type, target, &mut seen, 0)
}

/// Whether `from` reaches `target` over inline edges only.
///
/// The edges are the ones a binding that holds a member as a value
/// follows into storage: an aggregate's members — a base's included,
/// which is why [`Ast::all_members_of`] is asked rather than the node's
/// own list — a union's branches, an array's element, and an alias,
/// which is the same storage under another name. A sequence, a map, a
/// string and a boxed value are not edges at all: each is a handle, and
/// what it holds lives elsewhere. IDL 4.2 §7.4.7 makes a boxed value a
/// reference, and `crates/lang-rules/src/adapter/c.rs` declares one as
/// a pointer for that reason.
///
/// `seen` makes this terminate on a type graph that has cycles, which is
/// the graph it exists to be asked about; `depth` bounds it a second
/// time against a model whose ids repeat.
///
/// zerodds-lint: recursion-depth 64 (aggregate, union and array nesting;
/// bounded by [`MAX_DEPTH`] and by `seen`)
fn reaches_inline(
    ast: &Ast,
    from: Id<TypeNode>,
    target: Id<TypeNode>,
    seen: &mut alloc::collections::BTreeSet<Id<TypeNode>>,
    depth: usize,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let from = through_aliases(ast, from);
    if from == target {
        return true;
    }
    if !seen.insert(from) {
        return false;
    }
    let Ok(node) = ast.type_(from) else {
        return false;
    };
    match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            ast.all_members_of(from).into_iter().any(|member| {
                ast.member(member)
                    .ok()
                    .and_then(|node| node.type_.resolved())
                    .is_some_and(|type_| reaches_inline(ast, type_, target, seen, depth + 1))
            })
        }
        TypeKind::Union { .. } => ast.cases_of(from).iter().any(|case| {
            ast.case(*case)
                .ok()
                .and_then(|case_node| ast.member(case_node.member).ok())
                .and_then(|member| member.type_.resolved())
                .is_some_and(|type_| reaches_inline(ast, type_, target, seen, depth + 1))
        }),
        TypeKind::Array { element, .. } => element
            .resolved()
            .is_some_and(|element| reaches_inline(ast, element, target, seen, depth + 1)),
        // A sequence, a map, a string and a boxed value hold their
        // content behind a handle, so nothing below them is inside the
        // aggregate. Every other kind holds no type at all.
        TypeKind::Alias { .. }
        | TypeKind::Primitive(_)
        | TypeKind::Fixed { .. }
        | TypeKind::String { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::ValueBox { .. }
        | TypeKind::Map { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Native
        | TypeKind::ValueBase
        | TypeKind::Any
        | TypeKind::AnnotationType { .. }
        | TypeKind::ObjectRef { .. } => false,
    }
}

/// Plan the layout of a type.
///
/// # Errors
/// [`LayoutError::Unresolved`] when a reference in the graph has not
/// been resolved — no size is guessed, because a guessed plan is wrong
/// on the wire. [`LayoutError::KeyMustBeBigEndian`] for a little-endian
/// canonical-holder request ([`KeyHolderExtensibility::ForcedFinal`],
/// the default — see below). [`LayoutError::TooDeep`] past [`MAX_DEPTH`].
///
/// The endianness guard applies only to
/// [`KeyHolderExtensibility::ForcedFinal`]: that is XTypes 1.3 §7.6.8's
/// canonical Key Holder, fixed big-endian because its hash has to match
/// across peers that disagree about byte order, and every existing
/// caller that asks for it also asks for big-endian, so the guard has
/// never fired on anything else. [`KeyHolderExtensibility::AsDeclared`]
/// is a different holder — the type's own framing, kept rather than
/// flattened — and its consumer is a different question: decoding a
/// *wire* `serializedKey` (RTPS 2.5 §9.4.2.11), which a writer serializes
/// in its own active representation and byte order, not the canonical
/// one. Asking for `AsDeclared` in the writer's endianness does not risk
/// the disagreement this guard exists to catch, because nothing here
/// treats the *decoded* value as a hash — the hash is still always
/// computed from the canonical form separately.
pub fn plan(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    deps: &mut DepCollector,
) -> Result<TypeLayout, LayoutError> {
    if key.purpose == Purpose::Key
        && key.key_holder == KeyHolderExtensibility::ForcedFinal
        && key.endianness != Endianness::Big
    {
        return Err(LayoutError::KeyMustBeBigEndian);
    }
    plan_at(ast, type_, key, WcharPosition::Member, deps, 0)
}

/// Plan one type, keeping the descent path in `deps` so the collection
/// branches below can recognise a type they are already inside.
///
/// Push and pop live here and nowhere else: every recursive call goes
/// through this function, so the path is exact no matter which branch
/// returned and no matter whether it returned an error.
///
/// zerodds-lint: recursion-depth 64 (aggregate, collection and typedef
/// nesting, via [`plan_at_inner`]; bounded by [`MAX_DEPTH`], which
/// returns [`LayoutError::TooDeep`])
fn plan_at(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    position: WcharPosition,
    deps: &mut DepCollector,
    depth: usize,
) -> Result<TypeLayout, LayoutError> {
    deps.active.push((type_, deps.barriers));
    let planned = plan_at_inner(ast, type_, key, position, deps, depth);
    deps.active.pop();
    planned
}

/// The body of [`plan_at`], with the descent path already pushed.
///
/// zerodds-lint: recursion-depth 64 (aggregate, collection and typedef
/// nesting, back through [`plan_at`]; the guard below returns
/// [`LayoutError::TooDeep`] past [`MAX_DEPTH`])
fn plan_at_inner(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    position: WcharPosition,
    deps: &mut DepCollector,
    depth: usize,
) -> Result<TypeLayout, LayoutError> {
    let node = ast.type_(type_)?;
    let node_id = node.node_id;
    deps.note(node_id);

    if depth > MAX_DEPTH {
        return Err(LayoutError::TooDeep { node: node_id });
    }

    match &node.kind {
        TypeKind::Primitive(kind) => {
            let (align, size) = primitive_layout(*kind, key, position);
            Ok(TypeLayout::scalar(node_id, align, Extent::Sized(size)))
        }
        TypeKind::Fixed { digits, .. } => {
            // A fixed-point value occupies ceil((digits + 1) / 2) bytes
            // of BCD. IDL 4.2 §7.4.1.4.5 — under the declared digit
            // count. Under `FixedDigitCount::ValuePrecision` the field
            // is as wide as the value needs, so the type is not
            // fixed-size at all and the extent says so; that is why the
            // axis is in `LayoutKey::offset_key`.
            let extent = if key.fixed_digits.is_default() {
                Extent::Sized((u32::from(*digits) + 2) / 2)
            } else {
                Extent::Unbounded
            };
            Ok(TypeLayout::scalar(node_id, 1, extent))
        }
        TypeKind::String { width, .. } => {
            // A length prefix plus content: the alignment is that of the
            // prefix, the extent is never fixed even when bounded,
            // because the written length varies.
            let align = match width {
                CharWidth::Char => 4,
                CharWidth::WChar => 4,
            };
            Ok(TypeLayout::scalar(node_id, align, Extent::Unbounded))
        }
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            // Consulted for its alignment, which the sequence inherits —
            // except when the element is a type this descent is already
            // inside. IDL 4.2 §7.4.1.4.4.4.4 makes a type that names
            // itself through a collection legal, and finite on the wire:
            // the elements live behind a length, so a value terminates
            // even though the type does not. Planning the element again
            // would not.
            let align = if closes_a_cycle(ast, element, deps) {
                SEQUENCE_LENGTH_ALIGNMENT
            } else {
                // The element sits behind this sequence's length, so a
                // re-occurrence of an enclosing type below here is the
                // legal recursion — the barrier count says so.
                deps.barriers += 1;
                let planned = plan_at(
                    ast,
                    element,
                    key,
                    WcharPosition::inside_collection(),
                    deps,
                    depth + 1,
                );
                deps.barriers -= 1;
                planned?.align
            };
            let mut layout = TypeLayout::scalar(
                node_id,
                align.max(SEQUENCE_LENGTH_ALIGNMENT),
                Extent::Unbounded,
            );
            layout.needs_dheader = collection_is_delimited(ast, element, key);
            Ok(layout)
        }
        TypeKind::Array { element, dims } => {
            let element_id = element
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            // An array has no length on the wire, so a cycle through
            // one never terminates — IDL 4.2 §7.4.1.4.4.4.4 admits
            // sequence (and a map carries a length the same way), not
            // this. Named rather than left to `MAX_DEPTH`.
            if closes_a_cycle_without_length(ast, element_id, deps) {
                return Err(LayoutError::RecursionOutsideCollection { node: node_id });
            }
            let inner = if closes_a_cycle(ast, element_id, deps) {
                TypeLayout::scalar(node_id, 1, Extent::Unbounded)
            } else {
                plan_at(
                    ast,
                    element_id,
                    key,
                    WcharPosition::inside_collection(),
                    deps,
                    depth + 1,
                )?
            };
            // Every dimension multiplies the element count; §7.4.3.5
            // treats a multi-dimensional array as one flat run.
            let mut count: Option<u32> = Some(1);
            for dim in dims {
                count = count.and_then(|so_far| {
                    let value = ast.eval_integral(*dim).ok()?;
                    so_far.checked_mul(u32::try_from(value).ok()?)
                });
            }
            // Elements sit consecutively, each aligned. An element whose
            // size is not a multiple of its alignment therefore pads
            // *between* elements but not after the last one.
            //
            // Measured on 2026-08-09 with
            // `@final struct Odd { long a; short b; }; Odd items[2];
            // octet tail;` — all three implementations write
            // `01000000 0200 0000 03000000 0400 77`: six bytes, two of
            // padding, six bytes, then the octet at offset 14.
            //
            // `ElementPadding::Packed` drops that padding and puts the
            // second element at offset six, which is what
            // `tests/interop-results/array-1d-struct-final/hdds/plain-cdr1.le.bin`
            // records.
            let mut total = match (count, inner.extent) {
                (Some(count), Extent::Sized(size)) if count > 0 => {
                    let stride = if key.element_padding.pads_after(size, inner.align) {
                        align_to(size, inner.align)
                    } else {
                        size
                    };
                    stride
                        .checked_mul(count.saturating_sub(1))
                        .and_then(|body| body.checked_add(size))
                        .map_or(Extent::Unbounded, Extent::Sized)
                }
                (Some(0), Extent::Sized(_)) => Extent::Sized(0),
                _ => Extent::Unbounded,
            };
            let dheader = array_is_delimited(ast, element_id, key);
            if dheader {
                // The delimiter is a UInt32 ahead of the elements, so it
                // both adds four bytes and raises the collection's own
                // alignment to four.
                total = match total {
                    Extent::Sized(size) => {
                        size.checked_add(4).map_or(Extent::Unbounded, Extent::Sized)
                    }
                    Extent::Unbounded => Extent::Unbounded,
                };
            }
            let align = if dheader {
                inner.align.max(4)
            } else {
                inner.align
            };
            let mut layout = TypeLayout::scalar(node_id, align, total);
            layout.needs_dheader = dheader;
            Ok(layout)
        }
        TypeKind::Map { key: k, value, .. } => {
            let k = k
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            let v = value
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            // Both halves are behind the same element count as a
            // sequence's, so a self-naming type is legal and finite in
            // either of them; see `closes_a_cycle`.
            let key_align = if closes_a_cycle(ast, k, deps) {
                SEQUENCE_LENGTH_ALIGNMENT
            } else {
                {
                    deps.barriers += 1;
                    let planned = plan_at(
                        ast,
                        k,
                        key,
                        WcharPosition::inside_collection(),
                        deps,
                        depth + 1,
                    );
                    deps.barriers -= 1;
                    planned?.align
                }
            };
            let value_align = if closes_a_cycle(ast, v, deps) {
                SEQUENCE_LENGTH_ALIGNMENT
            } else {
                {
                    deps.barriers += 1;
                    let planned = plan_at(
                        ast,
                        v,
                        key,
                        WcharPosition::inside_collection(),
                        deps,
                        depth + 1,
                    );
                    deps.barriers -= 1;
                    planned?.align
                }
            };
            let mut layout = TypeLayout::scalar(
                node_id,
                key_align.max(value_align).max(SEQUENCE_LENGTH_ALIGNMENT),
                Extent::Unbounded,
            );
            // Rule (14) exempts a map only when *both* halves are
            // primitive; rule (15) delimits every other map under
            // XCDR2. Fast CDR 2.3.5 asks the value alone — a
            // `map<string, long>` carries no delimiter and a
            // `map<long, string>` does — and `MapDelimiter` is which of
            // the two readings this plan takes.
            layout.needs_dheader = match key.map_delimiter {
                MapDelimiter::EitherHalf => {
                    collection_is_delimited(ast, k, key) || collection_is_delimited(ast, v, key)
                }
                MapDelimiter::ValueOnly => collection_is_delimited(ast, v, key),
            };
            Ok(layout)
        }
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            // A typedef is transparent on the wire.
            plan_at(ast, target, key, position, deps, depth + 1)
        }
        TypeKind::Enumeration { .. } => {
            let bits = enumeration_bit_bound(ast, type_, key.enum_holder);
            let size = bit_bound_size(bits);
            Ok(TypeLayout::scalar(
                node_id,
                size,
                Extent::Sized(u32::from(size)),
            ))
        }
        TypeKind::Bitmask { .. } => {
            let bits = node
                .facets
                .xtypes
                .as_ref()
                .and_then(|x| x.bit_bound)
                .unwrap_or(32);
            let size = bit_bound_size(bits);
            Ok(TypeLayout::scalar(
                node_id,
                size,
                Extent::Sized(u32::from(size)),
            ))
        }
        TypeKind::Bitset { .. } => {
            // The base's bitfields, then this bitset's own — the same
            // reason the `Structure` arm below walks the chain. Summing
            // only the declared fields sized `bitset B2 : B1` as if the
            // inherited bits were not there, which is a short anchor and
            // a truncated sample. `Ast::all_bitfields_of` carries the
            // measurement the order comes from.
            let total_bits: u32 = ast
                .all_bitfields_of(type_)
                .iter()
                .filter_map(|f| ast.bitfield(*f).ok())
                .map(|f| u32::from(f.bits))
                .sum();
            let bytes = total_bits.div_ceil(8).max(1);
            // 1, 2, 4 or 8 bytes by total width. Fast CDR 2.3.5 picks
            // the same holder — `Cdr::serialize` has one overload per
            // `std::bitset<N>` band, N<9 to `uint8_t`, N<17 to
            // `uint16_t`, N<33 to `uint32_t`, N<65 to `uint64_t` — and
            // has none past 64, which is why the probe refuses there.
            let holder = bytes.next_power_of_two().min(8);
            let align =
                u8::try_from(holder.min(u32::from(key.encoding.max_alignment()))).unwrap_or(1);
            Ok(TypeLayout::scalar(node_id, align, Extent::Sized(holder)))
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            // The base's members, then this type's own. The walk lives
            // here rather than in each consumer: a plan that leaves the
            // inherited members out is a plan every emitter behind it
            // writes short. XTypes 1.3 §7.4.1.1.5.1.
            let members = ast.all_members_of(type_);
            plan_aggregate(ast, node_id, &members, node, key, deps, depth)
        }
        TypeKind::Union {
            discriminator,
            cases,
            ..
        } => {
            let disc = discriminator
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            let disc_layout = plan_at(ast, disc, key, position.inside(), deps, depth + 1)?;
            // §7.6.8 Step 1 over a union keyed by its discriminator: the
            // branches are not keys, and which branch a sample carries is
            // the discriminator's own answer, so the holder is the
            // discriminator alone. No branch is planned and no delimiter
            // is written — there is one member and its extent is fixed.
            //
            // Only reachable where a front end carried `@key` on the
            // discriminator (`KeyOnUnionDiscriminator::Carried`); a union
            // reached as a `@key` member of a keyed structure still plans
            // whole, because Step 1's "otherwise do not remove any
            // members" is what applies to it.
            if key.purpose == Purpose::Key
                && crate::keyhash::union_key_discriminator(ast, type_).is_some()
            {
                return Ok(TypeLayout::scalar(
                    node_id,
                    disc_layout.align,
                    disc_layout.extent,
                ));
            }
            let extensibility = extensibility_of(ast, node, key);
            // `AggregateDelimiter::Never` frames a union as `@final` too:
            // no DHEADER and, for a `@mutable` union, no MMEMBER headers.
            // Measured on HDDS's `annotation-external-union-branch`,
            // whose `@appendable` union takes no delimiter under XCDR2.
            let mutable = extensibility == Extensibility::Mutable
                && key.aggregate_delimiter.frames_mutable_members();
            let mut align = disc_layout.align;
            // The union's extent is the discriminator plus the widest
            // branch, which is fixed only if every branch is.
            let mut widest = 0u32;
            let mut fixed = disc_layout.extent.is_sized();
            // Rules (27) and (28): a `@mutable` union frames its
            // discriminator and its selected member as `MMEMBER`s. Only
            // one member is ever written, but which one is the value's
            // choice and not the plan's, so every case gets a plan and
            // the walk picks the one it needs.
            let mut plans: Vec<MemberPlan> = Vec::new();
            // The discriminator takes member id 0 and the cases run on
            // from 1. §7.3.1.2.1.1 numbers the members of an aggregated
            // type from 0 and the discriminator is one of them, so this
            // is the only numbering under which the discriminator
            // cannot collide with an implicitly numbered branch; RTI and
            // Fast DDS both write `00000020` for the discriminator and
            // `01000020` for the first case member — `vendor_bytes.md`
            // §8 — and Cyclone has no opinion, its `idlc` refusing a
            // mutable union outright. OpenDDS numbers the branches from
            // 0, which is `MutableUnionFraming::BranchesFromZero`.
            let mut next_id = key.union_framing.first_branch_id();
            for case in cases {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let Some(member_type) = member.type_.resolved() else {
                    return Err(LayoutError::Unresolved { node: node_id });
                };
                deps.note(member.node_id);
                // The same refusal the aggregate members make: a
                // branch of an enclosing type closes a cycle with no
                // sequence or map on it (IDL 4.2 §7.4.1.4.4.4.4).
                let external = finite_external_member(ast, member);
                if !external && closes_a_cycle_without_length(ast, member_type, deps) {
                    return Err(LayoutError::RecursionOutsideCollection {
                        node: member.node_id,
                    });
                }
                let branch = if closes_a_cycle(ast, member_type, deps) {
                    TypeLayout::scalar(member.node_id, 1, Extent::Unbounded)
                } else {
                    if external {
                        deps.barriers += 1;
                    }
                    let planned =
                        plan_at(ast, member_type, key, position.inside(), deps, depth + 1);
                    if external {
                        deps.barriers -= 1;
                    }
                    planned?
                };
                align = align.max(branch.align);
                match branch.extent {
                    Extent::Sized(size) => widest = widest.max(size),
                    Extent::Unbounded => fixed = false,
                }
                let member_id = member.facets.effective_member_id().unwrap_or(next_id);
                next_id = member_id.saturating_add(1);
                if mutable {
                    // Behind the branch's barrier, for the reason the
                    // aggregate arm gives: the length code plans the
                    // branch's type again.
                    if external {
                        deps.barriers += 1;
                    }
                    let header = member_header(
                        ast,
                        member_type,
                        member_id,
                        branch.extent,
                        member_must_understand(&member.facets, key.must_understand),
                        key,
                        deps,
                        depth,
                    );
                    if external {
                        deps.barriers -= 1;
                    }
                    plans.push(MemberPlan {
                        member: member.node_id,
                        member_id,
                        header: header?,
                        // A union writes one branch and the branch is
                        // chosen at runtime, so no case has a fixed
                        // offset even when every one of them is sized.
                        offset: Offset::Dynamic,
                        align: branch.align,
                    });
                }
            }
            let needs_dheader = key.aggregate_delimiter.writes_dheader()
                && key.encoding.is_v2()
                && extensibility != Extensibility::Final;
            // A `@mutable` union carries a header per member, so its
            // length is not the discriminator plus the widest branch
            // whatever the branches are.
            let extent = match (fixed && !mutable, disc_layout.extent) {
                (true, Extent::Sized(disc_size)) => {
                    Extent::Sized(align_to(disc_size, align).saturating_add(widest))
                }
                _ => Extent::Unbounded,
            };
            let discriminator = if mutable {
                // §7.2.2.4.4.4.6: "the discriminator member shall always
                // have the 'must understand' attribute set to true." The
                // clause-by-clause dialect writes it
                // (`MustUnderstandPolicy::DeclaredKeyAndDiscriminator`).
                // RTI and OpenDDS write `00000020`/`20000000`, which is
                // `M_FLAG` **clear**, and the sentinel-free XCDR1 form
                // `00000400` likewise; their dialects keep the clear bit
                // because it is what they were measured writing, and the
                // departure is a named setting rather than the default.
                Some(member_header(
                    ast,
                    disc,
                    0,
                    disc_layout.extent,
                    key.must_understand.discriminator_is_understood(),
                    key,
                    deps,
                    depth,
                )?)
            } else {
                None
            };
            Ok(TypeLayout {
                type_: node_id,
                align,
                extent,
                needs_dheader,
                // Rule (28) closes an XCDR1 `MUNION_TYPE` with
                // `PID_SENTINEL`; one implementation writes none, and
                // that is the other half of
                // `MutableUnionFraming`. The struct arm below is not
                // this question: OpenDDS terminates a `@mutable`
                // struct's list and only a union's is left open.
                needs_sentinel: key.encoding.is_v1()
                    && mutable
                    && key.union_framing.writes_the_sentinel(),
                discriminator,
                members: plans,
            })
        }
        TypeKind::Native | TypeKind::AnnotationType { .. } => {
            Err(LayoutError::UnsupportedForEncoding {
                node: node_id,
                encoding: key.encoding,
                reason: "the type has no defined wire form",
            })
        }
        // Reachable from IDL since `Object` was given its rules, and
        // refused here by name for the same reason `any` is: DDS-XTypes
        // has no object reference and no type identifier for one. CORBA
        // writes an IOR; that is a GIOP question and not an XCDR one.
        TypeKind::ObjectRef { .. } => Err(LayoutError::UnsupportedForEncoding {
            node: node_id,
            encoding: key.encoding,
            reason: "an object reference has no wire form in DDS-XTypes; CORBA writes an IOR \
                     for one, which is GIOP's framing and not XCDR's",
        }),
        // Accepted in the model, refused here, and refused **by name**.
        // Building Block Any is one of the fifteen this project takes,
        // so `any` parses and lowers; DDS-XTypes has no `any` at all, so
        // there is nothing to plan. Saying which construct cannot be
        // encoded is a different statement from failing to parse it, and
        // it is the one the author of the declaration can act on.
        TypeKind::ValueBase => Err(LayoutError::UnsupportedForEncoding {
            node: node_id,
            encoding: key.encoding,
            reason: "`ValueBase` has no wire form in DDS-XTypes, which has no valuetype; CORBA \
                     CDR writes a chunked value with its repository id ahead of it",
        }),
        TypeKind::Any => Err(LayoutError::UnsupportedForEncoding {
            node: node_id,
            encoding: key.encoding,
            reason: "`any` has no wire form in DDS-XTypes; it is encoded only by CORBA CDR, \
                     which writes a TypeCode ahead of the value",
        }),
        TypeKind::ValueBox { boxed } => {
            let target = boxed
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            let inner = plan_at(ast, target, key, position, deps, depth + 1)?;
            // A boxed value carries a presence flag ahead of its content.
            Ok(TypeLayout::scalar(
                node_id,
                inner.align.max(1),
                Extent::Unbounded,
            ))
        }
    }
}

fn bit_bound_size(bits: u16) -> u8 {
    match bits {
        0..=8 => 1,
        9..=16 => 2,
        17..=32 => 4,
        _ => 8,
    }
}

/// Whether a member's `must_understand` attribute is true, under a plan.
///
/// Two questions in one, and the second is the clause the corpus caught
/// this tree not reading. §7.4.1.2.2 and §7.4.3.4.2 put the member's
/// declared attribute on the wire; §7.2.2.4.4.4.8 says what that
/// attribute is for a `@key` member — "Key members shall never be
/// optional, and they shall always have their 'must understand'
/// attribute set to true".
///
/// The implication is the **type system's** and not the data
/// representation's, so it belongs here, before either framing chooses
/// which bit to put it in. Both call sites — a `@mutable` struct member
/// and a `@mutable` union branch — go through this rather than reading
/// `Facets::is_must_understand` straight, so that a site which forgot
/// the implication is a call this function does not have rather than a
/// bit nobody notices.
///
/// [`MustUnderstandPolicy::Omitted`] is applied later, at the header,
/// because it is about whether the bit is *written* and this is about
/// what the attribute *is*.
#[must_use]
pub fn member_must_understand(facets: &zerodds_ast::Facets, policy: MustUnderstandPolicy) -> bool {
    facets.is_must_understand() || (policy.key_implies_the_flag() && facets.is_key())
}

/// The bit bound an **enumeration**'s holder is built from, under a
/// plan.
///
/// The one place the declared `@bit_bound` crosses into a wire width,
/// and the reason it exists at all. Ten call sites over the three
/// language columns used to read `Ast::effective_bit_bound` straight —
/// the planner, the probe's writer and reader, the Rust rule, the two C
/// emitters and the two Python emitters — and each of them answered
/// [`EnumHolderWidth`] by omission, which is exactly how the position
/// defect that `WcharPosition` closed survived: a question nobody asks
/// has one answer everywhere and no way to say so.
///
/// A site that keeps calling `Ast::effective_bit_bound` for an
/// enumeration bypasses the axis, and nothing at run time would say so —
/// the bytes would simply be the declared width under a setting that
/// asked for four octets.
/// `crates/wire-probe/tests/enum_holder_width.rs::the_sites_go_through_the_crossing`
/// is the check that names the sites.
///
/// # Not a bitmask
///
/// `@bit_bound` on a `bitmask` is Table 33 of §7.4.1.1.3.2 and a
/// different construct; M1 measured enumerations only. Callers that
/// size a bitmask keep asking `Ast::effective_bit_bound`, and that is
/// deliberate rather than an omission.
#[must_use]
pub fn enumeration_bit_bound(ast: &Ast, type_: Id<TypeNode>, policy: EnumHolderWidth) -> u16 {
    policy.holder_bound(ast.effective_bit_bound(type_))
}

/// The integer an **enumerator** serialises as, under a plan.
///
/// The one place `@value` crosses into a wire value, and the sibling of
/// [`enumeration_bit_bound`] beside it: the two reach the same construct
/// and answer different questions — how wide the holder is, and which
/// integer goes into it. Every site that puts an enumerator on the wire
/// or takes one off it used to call `Ast::effective_enumerator_value`
/// straight, and each of them answered [`EnumeratorValue`] by omission.
///
/// A site that keeps calling `Ast::effective_enumerator_value` for a
/// value that reaches the wire bypasses the axis, and nothing at run
/// time would say so: the octets would simply be the declared value
/// under a setting that asked for the ordinal.
/// `crates/wire-probe/tests/enumerator_value.rs::the_sites_go_through_the_crossing`
/// is the check that names the sites.
///
/// # Not every caller of `effective_enumerator_value`
///
/// The **language mapping** keeps the declared value: a generated C
/// `enum` constant, a Rust discriminant and a Python `IntEnum` member
/// are what the IDL declares, whatever a peer's front end did with the
/// annotation. This crossing answers only for the integer that leaves
/// the process. `crates/lang-rules/src/adapter/c.rs`,
/// `default_c.rs`, `default_rust.rs` and `adapter/cst.rs` therefore keep
/// calling `Ast::effective_enumerator_value`, and that is deliberate
/// rather than an omission.
///
/// # Returns
///
/// `None` where the handle is stale or an explicit `@value` expression
/// does not evaluate — never a substituted `0`, which would give two
/// enumerators the same wire value without saying so. Under
/// [`EnumeratorValue::Ordinal`] the answer is the enumerator's position
/// and cannot fail for an expression it does not read; it still returns
/// `None` for a handle that is not this enumeration's, so that a caller
/// cannot get a position for a node the type does not declare.
#[must_use]
pub fn enumerator_wire_value(
    ast: &Ast,
    enumerator: Id<zerodds_ast::EnumeratorNode>,
    policy: EnumeratorValue,
) -> Option<i128> {
    match policy {
        EnumeratorValue::Declared => ast.effective_enumerator_value(enumerator),
        EnumeratorValue::Ordinal => {
            let node = ast.enumerator(enumerator).ok()?;
            let siblings = ast.enumerators_of(node.parent);
            let at = siblings.iter().position(|id| *id == enumerator)?;
            i128::try_from(at).ok()
        }
    }
}

/// Whether a **negative** case label of this union selects nothing,
/// under a plan.
///
/// The one place [`UnionLabelMatch`] crosses into branch selection, and
/// the reason it exists at all. Six call sites over the three language
/// columns decide which branch a union writes and which one it reads
/// back — the probe's writer and reader, the two C emitters, the C
/// column's JSON dump and the Python emitter — and each of them answers
/// this question by omission if it compares the label straight.
///
/// A site that compares a label against the discriminator without asking
/// bypasses the axis, and nothing at run time would say so: the bytes
/// would simply be the clause's under a setting that asked for
/// Cyclone's.
/// `crates/wire-probe/tests/union_label_match.rs::the_sites_go_through_the_crossing`
/// is the check that names the sites.
///
/// # The discriminator half
///
/// A signed integer primitive narrower than 32 bits — `int8` and
/// `short`, the two the corpus measured Cyclone dropping a branch on.
/// `octet`, `char`, `boolean`, `uint8` and `unsigned short` cannot carry
/// a negative label at all; `long` and `long long` were measured to
/// match one. An **enumeration** discriminator answers false whatever
/// the policy: `union-discriminator-enum` declares no negative `@value`,
/// so nothing measured says what Cyclone does with one.
///
/// A `typedef` of a discriminator is seen through, which is the same
/// reading `discriminator_c_width` takes.
///
/// # The storage half, and the segmentation fault behind it
///
/// Every case member has to be a primitive of one, two, four or eight
/// octets. What the peer writes where a `default:` branch is selected is
/// the union's **storage** read through that branch, and a branch that
/// is a `string` or a `sequence` reads a *pointer* out of that storage.
/// There are no octets to reproduce, and the corpus says so rather than
/// this comment: `union-member-sequence` is
/// `switch (short) { case -1: sequence<long> ids; default: string text; }`,
/// and `tests/interop-results/sources/vendor-corpus.tsv` records Cyclone
/// on it, in both encoding versions and both byte orders, as
/// `refused — the probe was killed: Segmentation fault`. Its `-1` is the
/// union's only label, so every discriminator value falls into
/// `default:`, and Cyclone produces no frame for that type at all.
///
/// So the setting does not reach such a union, and this project writes
/// the clause's bytes there — a complete frame a conforming reader can
/// use, rather than a refusal or an imitation of a crash. It is a
/// restriction on the axis for the same reason
/// [`crate::enumeration_bit_bound`] reaches an enumeration and not a
/// bitmask: a shape nothing measured gives an answer for.
/// `the_shape_with_no_storage_is_left_to_the_clause` is that restriction
/// as a test, and it names the recorded segmentation fault.
///
/// An **enumeration** or a `bitmask` branch is left out with them. Its
/// holder is one, two, four or eight octets under
/// [`EnumHolderWidth`], so it has storage, but the width is a second
/// setting's answer and no corpus case declares one; taking it would be
/// a claim about octets nobody has.
#[must_use]
pub fn union_negative_matches_nothing(
    ast: &Ast,
    union_: Id<TypeNode>,
    policy: UnionLabelMatch,
) -> bool {
    if policy.is_default() {
        return false;
    }
    let Ok(node) = ast.type_(union_) else {
        return false;
    };
    let TypeKind::Union { discriminator, .. } = &node.kind else {
        return false;
    };
    let Some(discriminator) = discriminator.resolved() else {
        return false;
    };
    let Some((signed, bits)) = discriminator_signedness(ast, discriminator) else {
        return false;
    };
    // Asked of a label that is negative by construction: the caller's
    // question is "does a negative label here select nothing", and
    // `label_matches` is the same arithmetic from the other side.
    if policy.label_matches(-1, signed, bits) {
        return false;
    }
    ast.cases_of(union_).iter().all(|id| {
        ast.case(*id)
            .ok()
            .and_then(|case| ast.member(case.member).ok())
            .and_then(|member| member.type_.resolved())
            .is_some_and(|type_| branch_has_storage(ast, type_))
    })
}

/// Whether a union branch occupies storage a peer's C union can be read
/// through — a primitive of one, two, four or eight octets.
///
/// `wchar` is left out because its width is [`WcharFraming`]'s answer
/// and GIOP 1.2's form is three octets, which is not a storage width;
/// `long double` because no C column carries a value for one. Neither is
/// a corpus case's branch.
fn branch_has_storage(ast: &Ast, type_: Id<TypeNode>) -> bool {
    let mut current = type_;
    for _ in 0..MAX_DEPTH {
        let Ok(node) = ast.type_(current) else {
            return false;
        };
        match &node.kind {
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => current = next,
                None => return false,
            },
            TypeKind::Primitive(kind) => {
                return matches!(
                    kind,
                    PrimitiveKind::Boolean
                        | PrimitiveKind::Octet
                        | PrimitiveKind::Char
                        | PrimitiveKind::Int8
                        | PrimitiveKind::UInt8
                        | PrimitiveKind::Int16
                        | PrimitiveKind::UInt16
                        | PrimitiveKind::Int32
                        | PrimitiveKind::UInt32
                        | PrimitiveKind::Int64
                        | PrimitiveKind::UInt64
                        | PrimitiveKind::Float32
                        | PrimitiveKind::Float64
                );
            }
            _ => return false,
        }
    }
    false
}

/// Whether a discriminator is a signed integer, and how wide, or `None`
/// where it is neither.
///
/// `char`, `wchar` and `boolean` are discriminator types IDL 4.2
/// §7.4.1.4.4.4.2 allows and none of them is signed, so each answers
/// `None` rather than a width nobody compares against.
/// How many digit nibbles a `fixed<P,S>` puts on the wire, under a plan.
///
/// The one place [`FixedDigitCount`] crosses into a field width, and the
/// reason it exists at all. Six call sites over the three language
/// columns write or read a `fixed` — the probe's writer and reader, the
/// two C emitters and the two halves of the Python emitter — and each of
/// them answers this question by omission if it takes `P` straight.
///
/// `declared` is the type's `P`; `significant` is the value's digit
/// count with its leading zeros removed; `string_digits` is the digit
/// count of the input string (all digits including leading zeros before
/// the decimal point and trailing zeros after it). Under
/// [`FixedDigitCount::Declared`] the second and third arguments are
/// ignored, which is what makes the crossing callable from a reader that
/// has not seen the value yet.
///
/// `crates/wire-probe/tests/fixed_digit_count.rs::the_sites_go_through_the_crossing`
/// names the sites. The C and Python **runtimes** implement the same
/// rule from the clause rather than calling this — that is what makes
/// the three columns three readings and not one — and the three-column
/// gate is where they are held against each other.
#[must_use]
pub const fn fixed_wire_digits(
    declared: u8,
    significant: u8,
    string_digits: u8,
    policy: FixedDigitCount,
) -> u8 {
    policy.wire_digits(declared, significant, string_digits)
}

/// How a `map`'s entries are ordered on the wire, under a plan.
///
/// The one place [`MapEntryOrder`] crosses into what a writer emits, and
/// the reason it exists at all. Three call sites over the three language
/// columns walk a map's entries — the probe's writer, the C emitter and
/// the Python emitter — and each of them answers this question by
/// omission if it iterates the sample straight.
///
/// A site that walks the entries without asking bypasses the axis, and
/// nothing at run time would say so: the octets would simply be the
/// declaration order under a setting that asked for ascending keys.
/// `crates/wire-probe/tests/map_entry_order.rs::the_sites_go_through_the_crossing`
/// is the check that names the sites.
///
/// # What it answers
///
/// `None` under [`MapEntryOrder::Declaration`], which is every dialect,
/// and `Some(ordering)` under [`MapEntryOrder::AscendingKey`] for a key
/// type whose values have an order — see [`MapKeyOrdering`].
///
/// # The key types it does not reach, and why that is a refusal
///
/// A `float`, a `double` or a `long double` key: IEEE 754 has no total
/// order, a NaN key compares false against everything, and `std::map`'s
/// `std::less` over one is undefined behaviour rather than an order this
/// project could reproduce. A `fixed` key, and any aggregate, union,
/// sequence, array or map key: C++ has no `operator<` for the type
/// `fastddsgen` would generate, so `std::map` over one does not compile
/// and there are no octets on the other side either.
///
/// The answer for those is `None` **under both values**, and the caller's
/// obligation is to refuse rather than to fall back to declaration order.
/// A silent fallback is the failure this project has paid for elsewhere:
/// a setting that quietly writes the default for one key type and the
/// setting for another is a flag that reports success and did nothing.
/// `map_entries_or_refuse` in each column is that obligation, and
/// `the_key_types_with_no_order_are_refused` is the test.
#[must_use]
pub fn map_key_ordering(
    ast: &Ast,
    map: Id<TypeNode>,
    policy: MapEntryOrder,
) -> Option<MapKeyOrdering> {
    if policy.is_default() {
        return None;
    }
    let Ok(node) = ast.type_(map) else {
        return None;
    };
    let TypeKind::Map { key, .. } = &node.kind else {
        return None;
    };
    key_ordering(ast, key.resolved()?)
}

/// The order the values of one key type have, seen through a typedef
/// chain.
fn key_ordering(ast: &Ast, key: Id<TypeNode>) -> Option<MapKeyOrdering> {
    let mut current = key;
    for _ in 0..MAX_DEPTH {
        match &ast.type_(current).ok()?.kind {
            TypeKind::Alias { aliased } => current = aliased.resolved()?,
            TypeKind::Primitive(kind) => {
                return Some(match kind {
                    // An IDL `char` is a C++ `char`, signed on the host
                    // the corpus was recorded on. Stated rather than
                    // taken from each column's compiler — see
                    // `MapEntryOrder`.
                    PrimitiveKind::Int8
                    | PrimitiveKind::Int16
                    | PrimitiveKind::Int32
                    | PrimitiveKind::Int64
                    | PrimitiveKind::Char => MapKeyOrdering::SignedInteger,
                    PrimitiveKind::Boolean
                    | PrimitiveKind::Octet
                    | PrimitiveKind::UInt8
                    | PrimitiveKind::UInt16
                    | PrimitiveKind::UInt32
                    | PrimitiveKind::UInt64
                    | PrimitiveKind::WChar => MapKeyOrdering::UnsignedInteger,
                    // Float, double, long double: no total order.
                    _ => return None,
                });
            }
            TypeKind::Enumeration { .. } => return Some(MapKeyOrdering::EnumeratorValue),
            TypeKind::String { .. } => return Some(MapKeyOrdering::CodeUnits),
            _ => return None,
        }
    }
    None
}

fn discriminator_signedness(ast: &Ast, discriminator: Id<TypeNode>) -> Option<(bool, u16)> {
    let mut current = discriminator;
    // A typedef chain is finite in a committed model; the bound is
    // belt and braces against a cycle a hand-built model could hold.
    for _ in 0..64 {
        match &ast.type_(current).ok()?.kind {
            TypeKind::Alias { aliased } => current = aliased.resolved()?,
            TypeKind::Primitive(kind) => {
                return Some(match kind {
                    PrimitiveKind::Int8 => (true, 8),
                    PrimitiveKind::Int16 => (true, 16),
                    PrimitiveKind::Int32 => (true, 32),
                    PrimitiveKind::Int64 => (true, 64),
                    PrimitiveKind::UInt8 | PrimitiveKind::Octet => (false, 8),
                    PrimitiveKind::UInt16 => (false, 16),
                    PrimitiveKind::UInt32 => (false, 32),
                    PrimitiveKind::UInt64 => (false, 64),
                    _ => return None,
                });
            }
            _ => return None,
        }
    }
    None
}

/// The extensibility a type takes under a given key.
///
/// Always FINAL in a key holder, whatever the type declares. RTPS 2.5
/// §9.6.4.8 Step 1 and XTypes 1.3 §7.6.8 Step 1 both build
/// `FooKeyHolder` from `Foo` and then say it "shall have extensibility
/// kind 'FINAL'". A holder that kept the original's kind carries the
/// original's framing: a DHEADER for an appendable one, EMHEADERs and an
/// unbounded extent for a mutable one — and an unbounded extent sends
/// Step 5 down the MD5 branch, so `@mutable struct Foo { @key long id; }`
/// hashed to `891a26e0…` where the specification's own Example 1 says
/// `12345678` zero-padded. Two peers that read the clause differently
/// compute different handles for one instance.
///
/// RTI Connext under XCDR1 is the one measured departure, and
/// [`KeyHolderExtensibility`] is where it lives.
///
/// A structure with a base that states no extensibility has its base's
/// (XTypes 1.3 §7.2.2.4.5), and the dialect's default applies only at the
/// root of the chain. Taking the default for the derived structure itself
/// framed `struct Sample : Base` of a `@mutable` base with a DHEADER and no
/// EMHEADERs, where Cyclone 11.0.1 and RTI 7.7.0 write the base's mutable
/// framing (byte gate, `struct_inherit_mut_silent`).
#[must_use]
pub fn extensibility_of(ast: &Ast, node: &TypeNode, key: LayoutKey) -> Extensibility {
    if key.purpose == Purpose::Key && key.key_holder == KeyHolderExtensibility::ForcedFinal {
        return Extensibility::Final;
    }
    let mut current = node;
    // A base chain longer than the planner's nesting bound is refused by
    // the planner itself; here it only has to end.
    for _ in 0..crate::plan::MAX_DEPTH {
        if let Some(extensibility) = current.facets.extensibility() {
            return extensibility;
        }
        let TypeKind::Structure {
            base: Some(base), ..
        } = &current.kind
        else {
            break;
        };
        let Some(base) = base.resolved().and_then(|id| ast.type_(id).ok()) else {
            break;
        };
        current = base;
    }
    // What an unannotated aggregate is taken to be is a setting and not
    // a constant: the implementations disagree, XTypes 1.3 §7.3.1.2.1.8
    // sanctions overriding the clause's appendable, and the dialect in
    // force has already decided. `Dialect::Specification`, the default, reads
    // it as final.
    key.default_extensibility
}

#[allow(clippy::too_many_lines)]
fn plan_aggregate(
    ast: &Ast,
    node_id: NodeId,
    members: &[Id<zerodds_ast::MemberNode>],
    node: &TypeNode,
    key: LayoutKey,
    deps: &mut DepCollector,
    depth: usize,
) -> Result<TypeLayout, LayoutError> {
    let extensibility = extensibility_of(ast, node, key);
    // `AggregateDelimiter::Never` frames every aggregate as `@final`,
    // which takes the member headers and the parameter-list sentinel
    // with it — see the axis for the three recorded rows that measure
    // it and `SequenceElementDelimiter` for the fourth position, which
    // answers the other way.
    let mutable =
        extensibility == Extensibility::Mutable && key.aggregate_delimiter.frames_mutable_members();

    // XCDR1 has no delimiter header at all, so an appendable type plans
    // none; XCDR2 gives one to anything that is not final.
    let needs_dheader = key.aggregate_delimiter.writes_dheader()
        && key.encoding.is_v2()
        && extensibility != Extensibility::Final;

    let mut plans: Vec<MemberPlan> = Vec::new();
    let mut cursor = 0u32;
    let mut align = 1u8;
    let mut fixed = true;
    let mut next_id = 0u32;

    // XTypes 1.3 §7.6.8 Step 1 removes the non-key members from the
    // holder only "if there are any key members. Otherwise do not remove
    // any members" — so an aggregate reached through a `@key` member but
    // declaring no key of its own puts all of them in.
    //
    // Unlike `key_members`, this applies at every depth including the
    // root: a keyless topic type has no KeyHash, but that decision
    // belongs to `key_members`, and both callers take it before they
    // look at this plan. Making it here as well would mean the probe,
    // which re-plans each nested aggregate from the top, could not tell
    // a nested keyless holder from a root one.
    let holder = key.purpose == Purpose::Key;
    let only_keys = holder && crate::keyhash::any_key_member(ast, members);
    let subtree = crate::keyhash::member_key_for(key, holder && !only_keys);

    // Numbering runs over EVERY member in declaration order, Step 1's
    // removal or not, and it runs before the traversal order is chosen: a
    // key holder may be serialized in member-id order (§7.6.8), and
    // numbering along a reordered walk would make the implicit id of a
    // member depend on the order it is written in.
    //
    // It also has to run before the removal, not skip the removed
    // members: an implicit id (`PreviousPlusOne`, §7.3.1.2.1.1) is the
    // type's own numbering of its declared members, key or not — Step 1
    // removes members from the *serialized* holder, not from the
    // identity that numbers them. A writer that keeps only the key
    // members still frames each one under the id its full type gave it;
    // a reader that renumbered the survivors from zero would be looking
    // for a `@mutable` type's key member under an id the writer never
    // used. Measured on `@mutable struct M { long x; @key long id; }`,
    // Eclipse Cyclone 11.0.1's `serializedKey`: `id`'s EMHEADER carries
    // its true position, 1, not the renumbered-survivors 0 — a wire read
    // that skipped `x`'s numbering step found nothing there and silently
    // defaulted `id`.
    let mut ordered: Vec<(Id<zerodds_ast::MemberNode>, u32)> = Vec::new();
    for member in members {
        let Ok(member_node) = ast.member(*member) else {
            continue;
        };
        // The plan reads the member's name, type and facets, so a change
        // to any of them has to evict it.
        deps.note(member_node.node_id);

        let member_id = member_node.facets.effective_member_id().unwrap_or(next_id);
        next_id = member_id.saturating_add(1);
        if only_keys && !member_node.facets.is_key() {
            continue;
        }
        ordered.push((*member, member_id));
    }
    // The holder's order is a dialect setting, and it has to be the one
    // `keyhash::key_members_with` hands the writer: the plan is where a
    // member's offset comes from, so a plan in one order and a writer in
    // another put every member after the first at the wrong offset. A
    // stable sort, so `Declaration` keeps exactly what was collected.
    if holder && key.key_holder_order == crate::key::KeyHolderOrder::AscendingMemberId {
        ordered.sort_by_key(|(_, member_id)| *member_id);
    }

    for (member, member_id) in &ordered {
        let member_id = *member_id;
        let Ok(member_node) = ast.member(*member) else {
            continue;
        };

        let member_type = member_node
            .type_
            .resolved()
            .ok_or(LayoutError::Unresolved { node: node_id })?;
        let optional = member_node.facets.is_optional();
        let terminates = optional || finite_external_member(ast, member_node);
        // A member whose type is one this descent is already inside
        // closes a cycle. Whether that is legal is the question of what
        // terminates a *value*, not of what the type graph looks like:
        // IDL 4.2 §7.4.1.4.4.4.4 admits recursion through a sequence
        // because the element count is what makes a value finite, and
        // an `@optional` member carries the same terminator — the
        // member can be absent, and an absent member ends the descent
        // exactly as a zero-length sequence does. So an `@optional`
        // member is a length barrier here, and only a cycle that
        // crosses no barrier at all is refused.
        //
        // XTypes 1.3 §7.3.1.2.1.4 is the clause that used to be read as
        // forbidding this. It calls a *cyclic value* unserializable —
        // "if the graph of objects has cycles, it is not serializable"
        // — and says nothing against a recursive type whose values are
        // trees. The refusal below is therefore the IDL clause's alone,
        // and it now names only what that clause names: a cycle with no
        // terminator on it. A genuinely cyclic value is caught where a
        // value exists to be cyclic, not here; see
        // `zerodds_wire_codec::MAX_VALUE_DEPTH`.
        //
        // Measured 2026-08-26, case `recursive-optional-external`
        // (`@mutable struct Node { long value; @optional @external Node
        // next; }`): Fast DDS carries the type and round-trips a
        // two-node tree, 28 octets under XCDR1 big-endian.
        if !terminates && closes_a_cycle_without_length(ast, member_type, deps) {
            return Err(LayoutError::RecursionOutsideCollection {
                node: member_node.node_id,
            });
        }
        let inner = if closes_a_cycle(ast, member_type, deps) {
            // The barrier is right here, so there is nothing below to
            // plan: descending would re-enter a type the walk is already
            // inside and the descent would not end. The sequence arm
            // breaks the same edge the same way.
            //
            // Alignment one and an unbounded extent are what a back edge
            // contributes, and both are exact rather than conservative.
            // An aggregate's alignment is the maximum over its members
            // and the back edge's own contribution is that maximum
            // again, so dropping it leaves the fixpoint unchanged; the
            // extent of a recursive type is not fixed by construction,
            // and `Extent::Unbounded` is what says so. Optional framing
            // below remains controlled by `optional`, not by this
            // recursion decision: external adds no wire absence flag.
            TypeLayout::scalar(member_node.node_id, 1, Extent::Unbounded)
        } else {
            // Below an `@optional` member the walk is behind a
            // terminator, exactly as it is below a sequence's length.
            if terminates {
                deps.barriers += 1;
            }
            let planned = plan_at(
                ast,
                member_type,
                subtree,
                WcharPosition::Member,
                deps,
                depth + 1,
            );
            if terminates {
                deps.barriers -= 1;
            }
            planned?
        };

        let must_understand = member_must_understand(&member_node.facets, key.must_understand);

        let header = if mutable {
            // The EMHEADER's length code plans the member's type a second
            // time, and that descent is behind the member's terminator
            // exactly as the layout's above was. Without the barrier a
            // cycle `A -(@optional)-> B -> A` re-entered `A` from here
            // as if nothing terminated it, and `@mutable struct A` was
            // refused with IDL0014 where `@final` passed.
            if terminates {
                deps.barriers += 1;
            }
            let header = member_header(
                ast,
                member_type,
                member_id,
                inner.extent,
                must_understand,
                key,
                deps,
                depth,
            );
            if terminates {
                deps.barriers -= 1;
            }
            header?
        } else if optional {
            // An `OPT_FMEMBER`: an `@optional` member of a type that is
            // not `@mutable`. §7.4.3.5.1 names the case and the two
            // rules below say what marks it, one per encoding version.
            //
            // Neither is an EMHEADER. This planner used to give one to
            // both versions, citing §7.4.2 — which is the three-paragraph
            // introduction to encoding version 2 and says nothing about
            // optional members. §7.4.3.5.2 says the opposite, all three
            // implementations write what it says, and the difference is
            // not a formatting one: an EMHEADER puts an `@optional octet`
            // at offset 8 where the clause and the vendors put it at 5.
            if key.encoding.is_v1() && key.aggregate_delimiter.frames_mutable_members() {
                // Rule (19): an `OPT_FMEMBER` is written as an `MMEMBER`,
                // which under XCDR1 is a parameter header — with a length
                // of zero when the member takes no value (§7.4.1.2).
                MemberHeader::OptionalCdr1 {
                    pid: u16::try_from(member_id).unwrap_or(u16::MAX),
                    extended: needs_extended_pid(
                        member_id,
                        inner.extent,
                        key.pl_cdr1_headers,
                        key.short_pid_range,
                    ) || extended_as_constructed(ast, member_type, key)?,
                    must_understand: must_understand && key.member_pid_carries_must_understand(),
                }
            } else {
                // Rule (20): a BOOLEAN `<is_present>`, then the value if
                // it is there. `AggregateDelimiter::Never` takes it under
                // XCDR1 too: the product that frames every aggregate as
                // `@final` writes the one-octet flag where the clause
                // writes a parameter header — measured on
                // `annotation-optional-present` and its unannotated twin,
                // whose absent member is a lone `00` and whose present one
                // is `01` and three octets of alignment.
                MemberHeader::OptionalFlag
            }
        } else {
            MemberHeader::NoHeader
        };

        align = align.max(inner.align);
        let offset = if fixed && matches!(header, MemberHeader::NoHeader) {
            let at = align_to(cursor, inner.align);
            Offset::Fixed(at)
        } else {
            Offset::Dynamic
        };

        if let (Offset::Fixed(at), Extent::Sized(size)) = (offset, inner.extent) {
            cursor = at.saturating_add(size);
        } else {
            fixed = false;
        }

        plans.push(MemberPlan {
            member: member_node.node_id,
            member_id,
            header,
            offset,
            align: inner.align,
        });
    }

    // No trailing padding. A structure ends after its last member; what
    // follows aligns itself. Measured on 2026-08-09 with
    // `@final struct Odd { long a; short b; }`, which Cyclone, RTI and
    // Fast DDS all serialise as the six bytes `010000000200` under both
    // encoding versions — not eight.
    //
    // A delimited type is still sized: the delimiter is four bytes and
    // the content behind it is what it is. Member offsets stay relative
    // to the content, because §7.4.3.4.1 resets `XCDR.origin` after the
    // DHEADER, so alignment inside is measured from there.
    let extent = if !fixed || mutable {
        Extent::Unbounded
    } else if needs_dheader {
        Extent::Sized(cursor.saturating_add(4))
    } else {
        Extent::Sized(cursor)
    };

    Ok(TypeLayout {
        type_: node_id,
        align,
        extent,
        needs_dheader,
        // A PL_CDR1 parameter list is terminated by PID_LIST_END; a
        // reader that stops at the last member it recognises instead of
        // at the sentinel misreads whatever follows.
        needs_sentinel: mutable && key.encoding.is_v1(),
        // An aggregate has no discriminator; only a union does.
        discriminator: None,
        members: plans,
    })
}

/// The `MMEMBER` header one member of a `@mutable` type takes.
///
/// Shared by [`plan_aggregate`] and the union arm of [`plan_at`] so that
/// rules (21) to (25) and rules (27) and (28) cannot answer differently:
/// the clause frames a union's discriminator and its selected member
/// exactly as it frames a structure's members, and a second derivation
/// here is a second place for that to stop being true.
#[allow(clippy::too_many_arguments)]
fn member_header(
    ast: &Ast,
    member_type: Id<TypeNode>,
    member_id: u32,
    extent: Extent,
    must_understand: bool,
    key: LayoutKey,
    deps: &mut DepCollector,
    depth: usize,
) -> Result<MemberHeader, LayoutError> {
    // Whether the flag reaches the wire is the dialect's.
    // `MustUnderstandPolicy` carries the run — Fast DDS omits it from
    // the short id, from the extended `eMemberHeader` and from the
    // XCDR2 EMHEADER alike. Applying it here rather than in each
    // writer is what keeps the framings from answering differently by
    // accident; where they answer differently *measured*, the framing
    // arm applies its own half: OpenDDS carries `M_FLAG` in the XCDR2
    // EMHEADER and nothing declaration-dependent in a PL_CDR1
    // parameter id — `LayoutKey::member_pid_carries_must_understand`
    // has the run.
    Ok(if key.encoding.is_v1() {
        MemberHeader::PidCdr1 {
            pid: u16::try_from(member_id).unwrap_or(u16::MAX),
            extended: needs_extended_pid(
                member_id,
                extent,
                key.pl_cdr1_headers,
                key.short_pid_range,
            ) || extended_as_constructed(ast, member_type, key)?,
            must_understand: must_understand && key.member_pid_carries_must_understand(),
        }
    } else {
        MemberHeader::EmHeader {
            lc: length_code(ast, member_type, key, deps, depth + 1)?,
            must_understand: must_understand && key.must_understand.writes_flag(),
        }
    })
}

/// Whether a member takes the extended header for what its type is:
/// under [`PlCdr1HeaderPolicy::ExtendedForConstructed`], every member
/// whose type is not a primitive or an enumeration, in a data sample.
///
/// A key holder keeps the short form: RTI's choice was measured on data
/// samples, and [`LayoutKey::for_sample`] is called for one.
///
/// # Errors
/// [`LayoutError::Unresolved`] for an alias that names nothing.
fn extended_as_constructed(
    ast: &Ast,
    member_type: Id<TypeNode>,
    key: LayoutKey,
) -> Result<bool, LayoutError> {
    if !key.pl_cdr1_headers.extends_constructed() || key.purpose != Purpose::Data {
        return Ok(false);
    }
    Ok(!crate::large_sample::keeps_short_header(ast, member_type)?)
}

/// Whether a PL_CDR1 member takes the extended parameter header.
///
/// Two independent reasons make it mandatory, and missing the second
/// one is the easy mistake: a member id **past** `PID_SHORT_MAX_ID`
/// collides with the reserved range, **and** a value longer than
/// `0xFFFF` bytes cannot state its length in the short header's 16
/// bits. XTypes 1.3 §7.4.1.2.
///
/// [`PlCdr1HeaderPolicy::AlwaysExtended`] takes it for every member
/// besides. That is not a third reason it is required, it is Cyclone
/// DDS's choice to use one form throughout, and §7.4.1.2.3 does not
/// reserve the extended form for the cases that need it.
///
/// # An unbounded member takes the short form
///
/// It used to take the extended one, on the reasoning that the plan
/// cannot promise an unbounded value will fit sixteen bits. That is true
/// of a plan and it is not the question: the vendors write the short
/// form and fall back only when a value actually overruns, which is a
/// writer-time decision because the length is known only after the value
/// is written. `Writer::finish_pid` makes it — the same call that
/// patches the length patches the form.
///
/// Measured 2026-08-09, case `mut_string`,
/// `@mutable struct M { @id(1) string s; @id(2) long n; };` with
/// `s = "abc"`, XCDR1 little endian: RTI writes
/// `01000800 04000000 61626300` and Fast DDS the same, 24 bytes each,
/// where this project wrote `017f0800 01000000 08000000 …` for 32. It is
/// only a length difference, and it was applied under `--dialect rti`
/// and `--dialect fastdds` — dialects that exist to reproduce peers that
/// do not do it — to every `string` and every `sequence` of every
/// `@mutable` XCDR1 type.
#[must_use]
pub fn needs_extended_pid(
    member_id: u32,
    extent: Extent,
    policy: PlCdr1HeaderPolicy,
    range: ShortPidRange,
) -> bool {
    if policy.always_extended() {
        return true;
    }
    // Strictly greater: §7.4.1.2.2's range is inclusive of 0x3F00, and
    // the reserved range starts at `PID_EXTENDED` one above it. One
    // implementation carries the whole low fourteen bits instead, which
    // is `ShortPidRange::AllFourteenBits`.
    if member_id > u32::from(range.highest_short_id()) {
        return true;
    }
    match extent {
        Extent::Sized(size) => size > PID_SHORT_MAX_LEN,
        Extent::Unbounded => false,
    }
}

/// Choose the EMHEADER length code for a member's type.
///
/// This is the rule that diverged across the fifteen emitters the
/// rebuild replaces. A **variable-length** member — string, wstring, map,
/// or a sequence whose element is not a primitive — frames as `Lc5`,
/// where the member's own leading length word doubles as the NEXTINT.
/// Using `Lc4` there writes a redundant length and puts every following
/// member at the wrong offset. XTypes 1.3 §7.4.2.
///
/// # Errors
/// [`LayoutError::Unresolved`] when the member's type graph is not
/// resolved far enough to decide.
///
/// zerodds-lint: recursion-depth 64 (typedef and collection nesting;
/// bounded by [`MAX_DEPTH`], which returns [`LayoutError::TooDeep`]. A
/// back edge is answered by `closes_a_cycle` before the descent, so a
/// recursive type does not spend the budget.)
pub fn length_code(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    deps: &mut DepCollector,
    depth: usize,
) -> Result<LengthCode, LayoutError> {
    let node = ast.type_(type_)?;
    let node_id = node.node_id;
    deps.note(node_id);

    if depth > MAX_DEPTH {
        return Err(LayoutError::TooDeep { node: node_id });
    }

    // Under the universal policy every member carries an explicit
    // NEXTINT, whatever its size. XTypes 1.3 §7.4.3.4.2 permits it, and
    // the OMG wire vector V-10 at `e1f3af895` pins both its members —
    // one four-byte int32 and one string — at lc=4.
    if key.length_codes == LengthCodePolicy::UniversalNextInt {
        return Ok(LengthCode::Lc4);
    }

    // A back edge: the member's type is one the descent is already
    // inside. Only a barrier can have put it there — a collection, or
    // an `@optional` member — and there is no plan below to take an
    // extent from, because a recursive type has no fixed one. Every
    // arm past this point either plans the type or reads a plan, so
    // reaching one of them would re-enter the type by a path the
    // barrier bookkeeping does not cross and run the descent to
    // `MAX_DEPTH`; the case `recursive-optional-external` did exactly
    // that under XCDR2 before this guard.
    //
    // The extent being unbounded settles the short-code half of every
    // policy at LC4, so the one question left is whether the type opens
    // with a DHEADER that the code can be spent on — which its
    // extensibility answers without a descent.
    if closes_a_cycle(ast, type_, deps) {
        return Ok(recursive_length_code(ast, type_, key));
    }

    // OpenDDS never spends a code on a word the member already carries:
    // LC0 to LC3 wherever the serialized length is 1, 2, 4 or 8, LC4
    // otherwise, and never LC5, LC6 or LC7. The arms below are the
    // reuse rules, so the whole of them is skipped and the one question
    // left is the short code's — which the plan answers where the type
    // fixes the size and `Writer::finish_emheader` answers for the rest.
    if !key.length_codes.reuses_a_leading_word() {
        let inner = plan_at(ast, type_, key, WcharPosition::Member, deps, depth + 1)?;
        return Ok(short_code_or_nextint(inner.extent, key));
    }

    Ok(match &node.kind {
        // A typedef is transparent: the code follows the aliased type.
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            return length_code(ast, target, key, deps, depth + 1);
        }

        // A string's own length word doubles as the NEXTINT. Both
        // reference implementations do this: Fast-CDR joins them
        // (`CdrSizeCalculator.hpp`, "Join NEXTINT and DHEADER"), and
        // Cyclone returns LENGTH_CODE_ALSO_NEXTINT with the comment
        // "nextint overlaps with length from serialized string data"
        // (`dds_cdrstream.c::get_length_code`).
        TypeKind::String { .. } => LengthCode::Lc5,

        // A map reuses its DHEADER when it has one. When it has not —
        // rule (14) exempts a map whose key and value are both
        // primitive — the leading word is the entry count, which is
        // neither a byte length nor a multiple of one, so the NEXTINT
        // has to be written out. Measured: Fast DDS and OpenDDS both
        // frame `map<long, long>` in a `@mutable` struct as Lc4 with an
        // explicit NEXTINT of 20 over `02000000` plus four longs, and
        // Fast DDS frames `map<long, Cell>` as Lc5 over its DHEADER.
        TypeKind::Map { .. } => {
            let inner = plan_at(ast, type_, key, WcharPosition::Member, deps, depth + 1)?;
            if inner.needs_dheader {
                LengthCode::Lc5
            } else {
                LengthCode::Lc4
            }
        }

        // Sequences are the fine-grained case. A delimited sequence
        // starts with the DHEADER, which is already the byte length of
        // everything after it; an undelimited one starts with the item
        // count, and then the element's width decides whether that
        // count states a length.
        TypeKind::Sequence { element, .. } => {
            let element = element
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            if collection_is_delimited(ast, element, key) {
                LengthCode::Lc5
            } else {
                return sequence_length_code(ast, element, key, 0);
            }
        }

        // An array is the same question with no count in front of it:
        // with a DHEADER the length is already there, without one
        // nothing states it and only a fixed size the code itself can
        // spell avoids a NEXTINT.
        //
        // This arm used to be absent and the fallback below answered
        // for it, which put an explicit NEXTINT *and* a DHEADER in
        // front of every array of a non-primitive element — four bytes
        // no implementation writes. Measured on 2026-08-09:
        // `@mutable struct { Cell items[2]; }` is `06000050 10000000`
        // and then the elements in Cyclone and in Fast DDS, and
        // `06000040 10000000` and then the elements in RTI, which
        // writes no array DHEADER at all. Both spend eight bytes on the
        // frame; this planner spent twelve.
        TypeKind::Array { element, .. } => {
            let element = element
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node_id })?;
            if array_is_delimited(ast, element, key) {
                LengthCode::Lc5
            } else {
                // No delimiter and no count: nothing in the member's own
                // bytes states its length. Fast DDS spends the code
                // itself when the size happens to fit; Cyclone and RTI
                // write LC4 for an array whatever its size — measured,
                // `long ap[2]` is `07000040` from both and `07000030`
                // from Fast DDS. `LengthCodePolicy` has the run.
                let inner = plan_at(ast, type_, key, WcharPosition::Member, deps, depth + 1)?;
                short_code_or_nextint(inner.extent, key)
            }
        }

        // Fixed sizes map straight onto LC0..LC3.
        TypeKind::Primitive(kind) => {
            let (_, size) = primitive_layout(*kind, key, WcharPosition::Member);
            code_for_size(size)
        }
        // Split from `Bitmask` below because only the enumeration's
        // holder crosses [`EnumHolderWidth`]: M1 measured what the front
        // ends do with `@bit_bound` on an enumeration and says nothing
        // about a bitmask.
        TypeKind::Enumeration { .. } => {
            let bits = enumeration_bit_bound(ast, type_, key.enum_holder);
            code_for_size(u32::from(bit_bound_size(bits)))
        }
        TypeKind::Bitmask { .. } => {
            let bits = node
                .facets
                .xtypes
                .as_ref()
                .and_then(|x| x.bit_bound)
                .unwrap_or(32);
            code_for_size(u32::from(bit_bound_size(bits)))
        }

        // A bitset is a scalar of its anchor width, so its length is
        // fixed by its kind exactly as an enumeration's is, and the
        // short code costs nothing under any policy. Fast DDS is the
        // only implementation that parses the keyword; `refused-
        // constructs.md` has the run.
        TypeKind::Bitset { .. } => {
            let inner = plan_at(ast, type_, key, WcharPosition::Member, deps, depth + 1)?;
            match inner.extent {
                Extent::Sized(size) if size <= 8 && size.is_power_of_two() => code_for_size(size),
                _ => LengthCode::Lc4,
            }
        }

        // A union's length depends on the active branch, so nothing in
        // its bytes states it — unless it is `@appendable` or
        // `@mutable`, where its own DHEADER does, and then it is the
        // nested-aggregate question below.
        //
        // Cyclone writes LC4 for a union whatever its extensibility and
        // says so in `dds_cdrstream.c`, in the same arm as the nested
        // aggregate: `case DDS_OP_VAL_UNI: case DDS_OP_VAL_EXT: return
        // LENGTH_CODE_NEXTINT; /* FIXME */`. RTI and Fast DDS reuse a
        // nested aggregate's DHEADER — measured, `mut_lc_nested` — and
        // no measurement covers a union inside a `@mutable` type in
        // either. One clause, one mechanism, one leading word: the two
        // are treated alike here, and this comment is the record that
        // the union half is extrapolated from the aggregate half.
        _ => nested_length_code(ast, type_, key, deps, depth)?,
    })
}

/// The code for a member whose length nothing in its own bytes states,
/// beyond what a short code can spell.
///
/// [`LengthCodePolicy::ShortWhereItFits`] is Fast DDS's, and this is the
/// half of it a plan can answer: a member whose *type* fixes the size at
/// 1, 2, 4 or 8. The half it cannot — a sequence whose count is in the
/// value — is `Writer::finish_emheader`'s.
fn short_code_or_nextint(extent: Extent, key: LayoutKey) -> LengthCode {
    match extent {
        Extent::Sized(size)
            if key.length_codes.short_code_where_it_fits()
                && size <= 8
                && size.is_power_of_two() =>
        {
            code_for_size(size)
        }
        _ => LengthCode::Lc4,
    }
}

/// The code for a member whose type closes a cycle back into the
/// descent.
///
/// The same two questions [`nested_length_code`] asks, answered without
/// a plan because there is no finite one to make: the extent of a
/// recursive type is not fixed, which leaves LC4 everywhere the short
/// code would have applied, and the DHEADER question is extensibility
/// and encoding alone — the same expression [`plan_aggregate`] and the
/// union arm compute `needs_dheader` from.
fn recursive_length_code(ast: &Ast, type_: Id<TypeNode>, key: LayoutKey) -> LengthCode {
    if key.length_codes.reuses_nested_delimiter() && opens_with_dheader(ast, type_, key) {
        LengthCode::Lc5
    } else {
        LengthCode::Lc4
    }
}

/// Whether an aggregate or union opens with its own DHEADER.
///
/// Encoding version 2 delimits everything that is not `@final`
/// (XTypes 1.3 §7.4.3.4.1); version 1 delimits nothing.
fn opens_with_dheader(ast: &Ast, type_: Id<TypeNode>, key: LayoutKey) -> bool {
    let target = through_aliases(ast, type_);
    let Ok(node) = ast.type_(target) else {
        return false;
    };
    if !matches!(
        node.kind,
        TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Valuetype { .. }
    ) {
        return false;
    }
    key.encoding.is_v2() && extensibility_of(ast, node, key) != Extensibility::Final
}

/// The code for a nested aggregate, union or valuetype.
///
/// Two questions in order. Does the member begin with its own DHEADER?
/// Then LC5 reuses it, under every policy but Cyclone's — measured,
/// `mut_lc_nested`: RTI and Fast DDS write `00000050` over an
/// `@appendable` member where Cyclone writes `00000040`. Otherwise the
/// short-code question, which only Fast DDS answers yes.
fn nested_length_code(
    ast: &Ast,
    type_: Id<TypeNode>,
    key: LayoutKey,
    deps: &mut DepCollector,
    depth: usize,
) -> Result<LengthCode, LayoutError> {
    let inner = plan_at(ast, type_, key, WcharPosition::Member, deps, depth + 1)?;
    if inner.needs_dheader && key.length_codes.reuses_nested_delimiter() {
        return Ok(LengthCode::Lc5);
    }
    Ok(short_code_or_nextint(inner.extent, key))
}

/// The length code of a sequence that carries no DHEADER.
///
/// The caller has already settled the delimited case, so what is left
/// begins with the item count and the element's width decides what that
/// count states. Transcribed from Cyclone's `get_length_code_seq`;
/// Fast-CDR arrives at the same split through
/// `get_serialized_member_size`, whose `SERIALIZED_MEMBER_SIZE` / `_4` /
/// `_8` / `NO_...` map onto Lc5 / Lc6 / Lc7 / Lc4.
///
/// zerodds-lint: recursion-depth 64 (typedef chains over the element
/// type; bounded by [`MAX_DEPTH`], which returns
/// [`LayoutError::TooDeep`])
fn sequence_length_code(
    ast: &Ast,
    element: Id<TypeNode>,
    key: LayoutKey,
    depth: usize,
) -> Result<LengthCode, LayoutError> {
    let node = ast.type_(element)?;
    // Lc4 is a code a reader acts on, not a "do not know". Returning it
    // from a failure branch framed the member as if the answer had been
    // worked out, and every member after it landed at the wrong offset.
    // `length_code` next door errors here; so does this.
    if depth > MAX_DEPTH {
        return Err(LayoutError::TooDeep { node: node.node_id });
    }
    Ok(match &node.kind {
        TypeKind::Alias { aliased } => {
            let target = aliased
                .resolved()
                .ok_or(LayoutError::Unresolved { node: node.node_id })?;
            return sequence_length_code(ast, target, key, depth + 1);
        }
        TypeKind::Primitive(kind) => {
            let (_, size) = primitive_layout(*kind, key, WcharPosition::inside_collection());
            code_for_element_size(size)
        }
        // Reached only under a policy that treats an enumeration as
        // primitive for delimiting purposes, because otherwise the
        // sequence has a DHEADER and the caller answered already. Fast
        // DDS is that policy and frames `sequence<Colour>` in a
        // `@mutable` struct as `00000060` over a bare count of two:
        // Lc6, the holder's own width.
        TypeKind::Enumeration { .. } => {
            let bits = enumeration_bit_bound(ast, element, key.enum_holder);
            code_for_element_size(u32::from(bit_bound_size(bits)))
        }
        TypeKind::Bitmask { .. } => {
            let bits = node
                .facets
                .xtypes
                .as_ref()
                .and_then(|x| x.bit_bound)
                .unwrap_or(32);
            code_for_element_size(u32::from(bit_bound_size(bits)))
        }
        // An undelimited sequence of anything else — the `Never` policy,
        // which is RTI's. The count states no length, so the NEXTINT is
        // written out; RTI does the same for its own undelimited
        // `sequence<Cell>`.
        _ => LengthCode::Lc4,
    })
}

/// What a bare item count says about the byte length, per element width.
///
/// One byte and the count *is* the byte count; four and eight and it is
/// a multiple the code can name; two and it says nothing, because there
/// is no `Lc` for a factor of two.
fn code_for_element_size(size: u32) -> LengthCode {
    match size {
        1 => LengthCode::Lc5,
        4 => LengthCode::Lc6,
        8 => LengthCode::Lc7,
        _ => LengthCode::Lc4,
    }
}

fn code_for_size(size: u32) -> LengthCode {
    match size {
        1 => LengthCode::Lc0,
        2 => LengthCode::Lc1,
        4 => LengthCode::Lc2,
        8 => LengthCode::Lc3,
        _ => LengthCode::Lc4,
    }
}
