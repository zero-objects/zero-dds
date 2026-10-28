// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The target a codec walk emits into.
//!
//! [`encode`] walks the layout plan and decides, member by
//! member, what the generated program does: which header opens, what the
//! length code is, in what order a `map`'s entries go, where a `DHEADER`
//! is closed. None of that is C. What is C is the shape the decision is
//! written in — `v->x` rather than `v.x`, `&v->x` rather than `v.x` at a
//! call site, a counter declared `uint32_t` rather than `int`.
//!
//! This trait is that seam. A walk written against it emits the same
//! decisions into any language whose runtime implements the wire, and a
//! new column costs a backend rather than a second copy of the walk.
//!
//! ## What this does not share, on purpose
//!
//! Not the runtime. `runtime/c`, `runtime/cpp`, `runtime/java` and
//! `runtime/python` each implement DDS-XTypes 1.3 §7.4 from the clause,
//! and that is what makes a column a second reading of the specification
//! rather than a rendering of the first one's answer. What a backend
//! contributes here is call construction: which method name, which
//! receiver, which argument form. Where two columns disagree on a byte,
//! the disagreement is still between two runtimes.
//!
//! Sharing the walk removes the other thing — four copies of one framing
//! decision, of which a later clause fix reaches three. That failure
//! leaves every test green.
//!
//! ## Moving one of these files
//!
//! Several guards key on the path of a walk rather than on its contents,
//! deliberately: they check that exactly one site in the encoder crosses
//! from a declared bound to the effective one, and a check that searched
//! the whole tree would pass on a site in a test. So a rename breaks
//! them, and it broke them twice while these two walks moved here.
//!
//! What to sweep, both times it was these:
//!
//! - `tools/silent-drop-allowlist.tsv` — keyed on path *and* text, so a
//!   turbofish added by a genericisation counts as a new site. Move the
//!   reasons rather than re-judging: the code in the function did not
//!   change, and a reason written a second time is a reason that has
//!   drifted from what it was written about.
//! - `crates/wire-probe/tests/{enum_holder_width,enumerator_value,
//!   fixed_digit_count,map_entry_order,union_label_match}.rs` — one
//!   `SITES` table each.
//! - `docs/{c-column,wire-feature-inventory,wire-axes-followup}.md` and
//!   the interop-suite spec.
//!
//! `cargo test --workspace` stops after the first failing binary, which
//! is why the first sweep looked like one guard and was five. Use
//! `--no-fail-fast`.
//!
//! ## The scalar set
//!
//! [`Scalar`] is the C set because the C set is the union: every other
//! target has a name for each of these, and none needs one the list has
//! not got. `LongDouble` is the awkward member and is kept abstract for
//! the same reason `codec_name` refuses to name a function after the C
//! spelling — `double` and `long double` share a C type and do not share
//! a wire form.

pub mod ada;
pub mod ada83;
pub mod c;
pub mod cpp;
pub mod csharp;
pub mod d;
pub mod decode;
pub mod elixir;
pub mod encode;
pub mod fsharp;
pub mod go;
pub mod interface_cpp;
pub mod interface_java;
pub mod interface_rust;
pub mod interface_typescript;
pub mod java;
pub mod julia;
pub mod kotlin;
pub mod lua;
pub mod nim;
pub mod node;
pub mod ocaml;
pub mod python;
pub mod swift;
pub mod type_pair_cpp;
pub mod type_pair_java;
pub mod type_pair_rust;
pub mod typescript;
pub mod zig;

use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Literal};

/// Whether `id` is an enumeration once typedefs are seen through.
///
/// [`CodecTarget::enumerator_ordinal`] is asked for a map key of an
/// enumeration type and for the integer arms of a union overlay alike,
/// and a target whose override converts an enumeration value (`.rawValue`,
/// `@intFromEnum`) has to leave the integer arms as they are: the overlay
/// never carries an enumeration
/// ([`zerodds_ast_layout::union_negative_matches_nothing`]).
pub(crate) fn resolves_to_enumeration(ast: &Ast, id: Id<TypeNode>) -> bool {
    const MAX_ALIAS_DEPTH: usize = 64;
    let mut current = id;
    for _ in 0..MAX_ALIAS_DEPTH {
        match ast.type_(current).map(|node| &node.kind) {
            Ok(zerodds_ast::TypeKind::Enumeration { .. }) => return true,
            Ok(zerodds_ast::TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(target) => current = target,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// Whether the walk reaches `member` through `T`'s `external_` seams:
/// it carries `@external`, or `T` keeps a member on a type cycle the
/// same way ([`CodecTarget::holds_a_cycle_externally`]). One answer for
/// the four places the walks ask it, so the reader, the writer and the
/// union overlay cannot hold one member two ways.
pub(crate) fn reached_externally<T: CodecTarget>(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
) -> bool {
    if T::holds_a_cycle_externally() {
        // A column that holds `@optional` members behind a pointer of
        // their own (`T *`, §7.5.1.2.4.2) holds one that closes a cycle
        // there too: it is optional before it is external.
        if T::optionals_are_pointers()
            && member.facets.is_optional()
            && !crate::adapter::annotated_external(member)
        {
            return false;
        }
        crate::adapter::held_externally(ast, member)
    } else {
        crate::adapter::annotated_external(member)
    }
}

/// Whether this column keeps a member behind an indirection that
/// [`CodecTarget::behind_indirection`] reaches through: the member
/// closes a cycle, or it is `@external` in a column that keeps externals
/// behind a pointer ([`CodecTarget::externals_are_pointers`]) and its
/// value is not a pointer already.
///
/// `external` is the answer of [`reached_externally`] for the member.
/// One answer for the encoder, the decoder and the defaulter, so that
/// what the header declares and what the codec reaches through cannot
/// part.
pub(crate) fn behind_pointer<T: CodecTarget>(
    ast: &Ast,
    enclosing: Id<TypeNode>,
    member_type: Id<TypeNode>,
    external: bool,
    optional: bool,
) -> bool {
    zerodds_ast_layout::member_held_inline_closes_a_cycle(ast, enclosing, member_type)
        || (((external && T::externals_are_pointers())
            || (optional && !external && T::optionals_are_pointers()))
            && !T::value_is_pointer(ast, member_type))
}

/// How a walk reaches one member of an aggregate: the spelling of its field
/// and the structure that declares it.
pub(crate) struct Reach {
    /// The field, with the path through the base members that a column
    /// binds as `parent` ([`CodecTarget::inherited_path`]).
    pub field: String,
    /// The structure whose declaration holds the member: `aggregate`, or the
    /// base it is inherited from where the column nests the base.
    pub holder: Id<TypeNode>,
}

/// How many structures up the base chain of `aggregate` declare `member`.
pub(crate) fn inheritance_depth(
    ast: &Ast,
    aggregate: Id<TypeNode>,
    member: zerodds_ast_core::NodeId,
) -> usize {
    let mut current = aggregate;
    for depth in 0..64 {
        let declares = ast
            .members_of(current)
            .iter()
            .any(|handle| ast.member(*handle).is_ok_and(|node| node.node_id == member));
        if declares {
            return depth;
        }
        match ast.type_(current).map(|node| &node.kind) {
            Ok(zerodds_ast::TypeKind::Structure {
                base: Some(base), ..
            }) => match base.resolved() {
                Some(next) => current = next,
                None => return 0,
            },
            _ => return 0,
        }
    }
    0
}

/// The structure `depth` steps up the base chain of `aggregate`.
pub(crate) fn base_at(ast: &Ast, aggregate: Id<TypeNode>, depth: usize) -> Id<TypeNode> {
    let mut current = aggregate;
    for _ in 0..depth {
        match ast.type_(current).map(|node| &node.kind) {
            Ok(zerodds_ast::TypeKind::Structure {
                base: Some(base), ..
            }) => match base.resolved() {
                Some(next) => current = next,
                None => return aggregate,
            },
            _ => return aggregate,
        }
    }
    current
}

/// Where the walk of `aggregate` finds `member`, named `name` in IDL.
///
/// XTypes 1.3 §7.5.1.4.1.3: a language with no mapping of its own for
/// inheritance binds a derived structure "as if an instance of the base type
/// were the first member of the subtype with the name 'parent'". A column that
/// does answers [`CodecTarget::inherited_path`], and a member of a base is
/// reached through it; one that does not keeps the base's members in the
/// derived structure, where `field` is the plain name and `holder` is the
/// aggregate itself.
/// The base of a derived structure and the names of every member it has, inherited ones
/// included. XTypes 1.3 §7.5.1.4.1.3: a language with no mapping of its own for inheritance
/// binds the base as the first member `parent`; the code that fills a sample takes these keys
/// out of the flat object it is given and fills the base from them. `None` for a structure
/// with no base.
#[must_use]
pub fn inherited_member_names(
    ast: &Ast,
    aggregate: Id<TypeNode>,
) -> Option<(Id<TypeNode>, Vec<String>)> {
    let base = crate::adapter::c::struct_base(ast, aggregate)?;
    let names = ast
        .all_members_of(base)
        .iter()
        .filter_map(|id| ast.member(*id).ok())
        .map(|member| member.name.clone())
        .collect();
    Some((base, names))
}

/// The field a walk of `aggregate` writes `name` as, through the base members a column binds
/// as `parent` (see `reach`); for the code that fills and prints a sample of a generated type.
#[must_use]
pub fn member_field<T: CodecTarget>(
    ast: &Ast,
    aggregate: Id<TypeNode>,
    member: zerodds_ast_core::NodeId,
    name: &str,
) -> String {
    reach::<T>(ast, aggregate, member, name).field
}

pub(crate) fn reach<T: CodecTarget>(
    ast: &Ast,
    aggregate: Id<TypeNode>,
    member: zerodds_ast_core::NodeId,
    name: &str,
) -> Reach {
    let escaped = T::escape(name);
    let depth = inheritance_depth(ast, aggregate, member);
    match (depth, T::inherited_path(depth)) {
        (0, _) | (_, None) => Reach {
            field: escaped,
            holder: aggregate,
        },
        (depth, Some(path)) => Reach {
            field: format!("{path}{escaped}"),
            holder: base_at(ast, aggregate, depth),
        },
    }
}

/// The enumerator an enumeration takes as its default, by IDL name.
///
/// §7.3.1.2.1.10: the one `@default_literal` names, and "the first
/// literal in the enumeration" where none does. A question about the
/// specification and not about a language, so it is answered once here
/// and each column spells the answer through
/// [`CodecTarget::enumerator_constant`]. `default_c` derives the same
/// thing for its own defaulters and takes it from here, so the reader's
/// fallback and the defaulter's cannot name two different enumerators.
#[must_use]
pub fn default_enumerator_name(ast: &Ast, id: Id<TypeNode>) -> Option<String> {
    let mut first = None;
    for enumerator in ast.enumerators_of(id) {
        let Ok(node) = ast.enumerator(*enumerator) else {
            continue;
        };
        if node
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_default_literal)
        {
            return Some(node.name.clone());
        }
        if first.is_none() {
            first = Some(node.name.clone());
        }
    }
    first
}

/// A scalar the generated program declares a local of.
///
/// Locals only. A member's own type comes from the language's
/// materialiser, which knows things this does not: what a `sequence`
/// widens to, whether an `@optional` is a nullable reference or a
/// wrapper. These are the counters, lengths and saved header positions a
/// codec body needs for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scalar {
    /// The natural type of a `for` counter and a container length.
    ///
    /// `uint32_t` in C, `std::size_t` in C++, `int` in Java — the
    /// backend's own answer, and deliberately not one of the fixed-width
    /// members below. A sequence length is a `uint32` on the wire, so C
    /// counts in that type and `-Wsign-conversion` stays quiet where the
    /// counter meets one; Java has no unsigned type to count in and a
    /// `long` counter would need a cast at every `List.get`.
    Index,
    /// The type a byte count and a stream position are held in.
    ///
    /// `size_t` in C and `int` in Java, and distinct from
    /// [`Scalar::Index`] because C's two are not the same type: a
    /// reader compares a position against a delimiter's end in
    /// `size_t` and counts elements in `uint32_t`, and mapping both to
    /// one of the two is a narrowing `-Wshorten-64-to-32` catches.
    Size,
    /// `bool`
    Bool,
    /// `char`, the octet-wide one.
    Char,
    /// `int8_t`
    Int8,
    /// `int16_t`
    Int16,
    /// `int32_t`
    Int32,
    /// `int64_t`
    Int64,
    /// `uint8_t`
    UInt8,
    /// `uint16_t`
    UInt16,
    /// `uint32_t`
    UInt32,
    /// `uint64_t`
    UInt64,
    /// `float`
    Float,
    /// `double`
    Double,
    /// binary128, whatever the target calls it.
    LongDouble,
}

/// A small value of the runtime's that the generated code declares.
///
/// A header is opened before the value it frames and closed after it,
/// and the close needs to know where the open wrote. Each runtime keeps
/// that in a small value of its own — `zdw_dheader` in C, `DHeader` in
/// Java — and the walk holds one in a local without knowing what it is.
///
/// Naming these in the walk was the last thing C's spelling reached
/// into: a Java backend that inherited `zdw_dheader` as a type name
/// would generate source no compiler has a type for.
///
/// The reading direction adds four, and they were the leak the C++
/// reader found: the walk spelled `zdw_try_construct`,
/// `zdw_reader_mark`, `zdw_pid_read` and `zdw_emheader_read` into
/// [`CodecTarget::named`] directly, which is C's namespace written into
/// what is supposed to be language-neutral. They went unnoticed because
/// the C column was the only one that generated a reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Handle {
    /// What [`CodecTarget::runtime`]'s `write_dheader` returns.
    DHeader,
    /// What `write_emheader` returns.
    EmHeader,
    /// What `write_pid` and `write_pid_extended` return.
    Pid,
    /// The behaviour §7.2.2.7 attaches to a position, as the reader's
    /// extra parameter takes it.
    ///
    /// `zdw_try_construct` in C, `TryConstruct` in Java and C++. A
    /// parameter rather than something the runtime hands back, and here
    /// rather than beside [`Constant`] because the walk needs the
    /// *type* to declare the parameter with; the three values it takes
    /// are `Constant::TryConstruct*`. A walk that wrote C's spelling
    /// would name a type `javac` has never seen.
    TryConstruct,
    /// What `reader_save` returns and `reader_restore` takes.
    ReaderMark,
    /// What `read_pid` and `read_pid_as` return.
    PidRead,
    /// What `read_emheader` returns.
    EmHeaderRead,
}

/// What a codec routine is made of.
///
/// A struct rather than a parameter list because the reader takes a
/// parameter the writer does not — the try-construct behaviour, which
/// DDS-XTypes 1.3 §7.2.2.7 attaches to the position rather than to the
/// type — and eight positional arguments is where a caller starts
/// passing them in the wrong order.
pub struct Routine<T: CodecTarget + ?Sized> {
    /// The comment above it.
    pub doc: Doc,
    /// Its name.
    pub name: String,
    /// The type of the value being coded.
    pub sample: T::Type,
    /// How the sample arrives. See [`Handoff`].
    pub held: Handoff,
    /// Whether this is the writing direction.
    pub writing: bool,
    /// Whether it may legitimately have no caller.
    pub tolerate_unused: bool,
    /// Parameters after the sample, in written order.
    pub extra: Vec<(T::Type, String)>,
    /// The body, absent to ask for a forward declaration.
    pub body: Option<T::Block>,
}

/// A policy value the generated code names.
///
/// Every runtime publishes the same set — they implement the same
/// clauses — and each spells them its own way: `ZDW_WCHAR_WIDTH_2` in C
/// against `WcharWidth.OCTETS_2` in Java. An enum rather than the C
/// string, so a backend that forgets one does not compile instead of
/// emitting a name javac has never seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Constant {
    /// A scalar `wchar` is two octets wide.
    WcharWidth2,
    /// A scalar `wchar` is four octets wide.
    WcharWidth4,
    /// A `wchar` carries no length octet.
    WcharFixed,
    /// A `wchar` carries GIOP's length octet.
    WcharGiopLengthPrefixed,
    /// A `wstring`'s code units go in the stream's byte order.
    WstringUnitsStream,
    /// Big-endian whatever the stream is.
    WstringUnitsBigEndianAlways,
    /// Preceded by a byte-order mark.
    WstringUnitsByteOrderMark,
    /// In the host's order, unswapped.
    WstringUnitsHostUnswapped,
    /// §7.4.1.1.2: octets, no terminator.
    WstringSpecification,
    /// Characters rather than octets.
    WstringCharacterCount,
    /// Characters, and a terminator.
    WstringCharacterCountTerminated,
    /// Octets in wide units.
    WstringSpecificationWideUnits,
    /// What `rosidl` writes.
    WstringRosTypeSupport,
    /// Octets, terminated, two-octet units.
    WstringOctetsTerminated2,
    /// Octets, terminated, four-octet units.
    WstringOctetsTerminated4,
    /// Characters, terminated, two-octet units.
    WstringCharactersTerminated2,
    /// A `fixed` writes the digits its type declared.
    FixedDeclared,
    /// A `fixed` writes as many digits as the value has.
    FixedValuePrecision,
    /// A `fixed` writes the digit count of the input string, padded to odd.
    FixedValueStringDigits,
    /// A PL_CDR1 parameter carries a member.
    ParameterMember,
    /// A PL_CDR1 parameter carries an `@optional` member.
    ParameterOptional,
    /// §7.2.2.7: the sample is discarded.
    TryConstructDiscard,
    /// The value is trimmed to what fits.
    TryConstructTrim,
    /// The member takes Table 9's value.
    TryConstructUseDefault,
    /// The reader is still good.
    StatusOk,
    /// The reader gave up: the sample is discarded.
    StatusDiscarded,
    /// A length no stream that long can carry.
    StatusImpossibleLength,
    /// A value outside what the type can hold.
    StatusOutOfRange,
    /// The stream ended where a value was expected.
    StatusTruncated,
}

/// How a value reaches a nested codec.
///
/// The one place the targets genuinely disagree, and the reason it is an
/// enum rather than a `bool`: C hands an aggregate's address and an
/// array's first element, which are written differently and mean the
/// same thing, while a target with references hands the value either
/// way. A caller says which of the two situations it is in and the
/// backend says how that is spelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handoff {
    /// The value is an array, which C adjusts to a pointer on its own.
    Array,
    /// Anything else.
    Value,
}

/// What a union branch's storage octets are read out of.
///
/// The Cyclone union overlay (`union_negative_selects_nothing`): a
/// discriminator that selects a suppressed negative label makes the
/// peer write its union storage *as the default arm*, so the emitted
/// codec has to produce the value's storage bits — the `u64` whose
/// little-endian bytes are the value's little-endian storage octets,
/// zero-extended. Sign extension is the mistake this enum exists to
/// prevent: a target with no unsigned types masks, it does not cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitPatternSource {
    /// An integer, character, enumeration or bitmask holder of this
    /// many octets. The pattern is the value's two's-complement bits,
    /// masked to the width — `-2` in four octets is
    /// `0x00000000FFFFFFFE`. The walk hands the value through the
    /// target's `Int64` cast first, the same seam the enumeration
    /// codec reads its value through, so every target receives its
    /// widest integer and only masks.
    Int {
        /// 1, 2, 4 or 8.
        octets: u8,
    },
    /// A boolean: one octet, 0 or 1.
    Bool,
    /// Binary32: the IEEE 754 bits, zero-extended.
    Float32,
    /// Binary64: the IEEE 754 bits.
    Float64,
}

/// The syntax a codec walk builds.
///
/// Every method is a constructor: the trait holds no state and a backend
/// is a unit type. What varies between backends is spelling, not order —
/// a walk calls these in the sequence the layout plan dictates and the
/// backend never reorders.
pub trait CodecTarget {
    /// An expression.
    type Expr: Clone;
    /// A statement.
    type Stmt: Clone;
    /// A sequence of statements.
    type Block: Clone;
    /// A type, as written in a declaration.
    type Type: Clone;
    /// A top-level declaration — in C a function, in Java a method.
    type Decl;

    // -- types --------------------------------------------------------

    /// A scalar local's type.
    fn scalar(kind: Scalar) -> Self::Type;

    /// A type named by the materialiser.
    fn named(name: &str) -> Self::Type;

    /// The type of a handle the runtime hands back. See [`Handle`].
    fn handle(kind: Handle) -> Self::Type;

    /// A policy value the runtime publishes. See [`Constant`].
    fn constant(which: Constant) -> Self::Expr;

    /// A character's code point, as the runtime's parameter wants it.
    ///
    /// `narrow` is a `char` rather than a `wchar`: C truncates to an
    /// octet and widens again, which is what its runtime is handed
    /// today and what 17 220 recorded slots agree on. A target whose
    /// character type is already a code unit hands it over and lets the
    /// runtime keep the low byte — the same octet, decided one step
    /// later.
    ///
    /// A seam and not a cast, because the width is the backend's:
    /// `Scalar::UInt32` is `uint32_t` in C and `long` in Java, and
    /// `writeChar` takes an `int`.
    fn code_point(value: Self::Expr, narrow: bool) -> Self::Expr;

    /// Whether `left` is less than `right` as **unsigned** integers of
    /// this width.
    ///
    /// The default is the target's own `<`, which is right wherever the
    /// held type is unsigned or strictly wider than the value. Five
    /// columns are not in that position at eight octets, and each
    /// overrides: Java and Kotlin hold an IDL `unsigned long long` as
    /// the bit pattern in a `long` and reach `Long.compareUnsigned`,
    /// Lua reaches `math.ult`, OCaml's `Int64` reaches its runtime's
    /// `u64_less`, and Ada 83 — which has no modular type at all, only
    /// `Long_Integer` — reaches `Zd_Wire83.U64_Less`. Every one of them
    /// sorted `16045690984833335998` ahead of `7` in a `map`'s
    /// ascending-key order before the override existed (measured
    /// against Fast DDS on `map-key-ulonglong`; Ada 83 against Fast DDS
    /// and OpenDDS, all four encodings, 2026-09-04). A target in that
    /// position overrides.
    ///
    /// The narrower widths need no override anywhere: an `unsigned
    /// long` tops out at 4294967295, which every column above holds
    /// zero-extended in a signed type at least 64 bits wide, so the
    /// signed order and the unsigned order are the same order.
    fn unsigned_less(octets: u8, left: Self::Expr, right: Self::Expr) -> Self::Expr {
        let _ = octets;
        Self::binary("<", left, right)
    }

    /// Whether `left` is less than `right` as **boolean** map keys —
    /// `false` before `true`, the ascending-key order's reading of the
    /// one-octet wire value.
    ///
    /// The default is the target's own `<`, which C, C++, Rust and
    /// Python all give two booleans. A language whose `<` refuses the
    /// pair — Java, C# and TypeScript name a type error — overrides
    /// with a spelling of the same order.
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        Self::binary("<", left, right)
    }

    /// The value's storage bits, zero-extended into the runtime's
    /// 64-bit pattern argument. See [`BitPatternSource`]; consumed by
    /// the runtime's `write_union_overlay`, which writes the low
    /// `octets` of the pattern as one integer of that width in wire
    /// endianness — the Cyclone union overlay.
    ///
    /// The default spells each form as a free function of the runtime —
    /// `bit_pattern_int(value, octets)`, `bit_pattern_bool`,
    /// `bit_pattern_f32`, `bit_pattern_f64` — which fits every target
    /// whose [`CodecTarget::runtime_static`] names free functions. A
    /// target that routes `runtime_static` elsewhere, or that has a
    /// native spelling for a form, overrides.
    fn bit_pattern(source: BitPatternSource, value: Self::Expr) -> Self::Expr {
        match source {
            BitPatternSource::Int { octets } => Self::runtime_static(
                "bit_pattern_int",
                vec![value, Self::int(i128::from(octets))],
            ),
            BitPatternSource::Bool => Self::runtime_static("bit_pattern_bool", vec![value]),
            BitPatternSource::Float32 => Self::runtime_static("bit_pattern_f32", vec![value]),
            BitPatternSource::Float64 => Self::runtime_static("bit_pattern_f64", vec![value]),
        }
    }

    /// The unsigned type wide enough to hold `bits` of a bit set.
    ///
    /// A `@bit_bound(12)` set is assembled in something at least twelve
    /// bits wide, and which type that is belongs to the target: C picks
    /// the smallest `uintN_t` that fits, and a language with no unsigned
    /// types picks the next signed one up and lets the runtime read the
    /// bits. The walk asks for room, not for a spelling.
    fn unsigned_holder(bits: u16) -> Self::Type;

    /// An operation on the runtime that has no receiver.
    ///
    /// `no_dheader` is the only one: the plan says a value takes no
    /// delimiter and the walk still needs a handle to hand the close,
    /// so the runtime supplies one that finishes nothing. C exports it
    /// as a free function and Java as a static method on the writer,
    /// which is the same difference as [`CodecTarget::runtime`] and not
    /// the same call.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr;

    /// What a codec function returns when it returns nothing.
    fn nothing() -> Self::Type;

    // -- expressions --------------------------------------------------

    /// A bare name.
    fn name(text: &str) -> Self::Expr;

    /// An integer literal.
    fn int(value: i128) -> Self::Expr;

    /// Any other literal the common tree can hold.
    fn literal(value: Literal) -> Self::Expr;

    /// A call to a free function or, in a target that has no free
    /// functions, to the static method standing in for one.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr;

    /// An operation on the runtime's writer or reader.
    ///
    /// `method` is the operation's bare name — `write_u32`, `read_pid`,
    /// `finish_dheader` — and the backend spells it: C prefixes the
    /// runtime's single namespace and passes the receiver first,
    /// `zdw_write_u32(w, x)`; a target with methods writes
    /// `w.writeU32(x)`. Separate from [`CodecTarget::call`] because the
    /// two differ in more than the name, and a walk that built the C
    /// form and let a renderer guess at the other would be deciding a
    /// language question in the walk.
    ///
    /// `receiver` is the local the body holds the writer or reader in,
    /// which is `w` and `r` in every backend so far.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr;

    /// Whether this target's runtime carries the three axes
    /// [`zerodds_ast_layout::Dialect::Hdds`] selects.
    ///
    /// The axes are [`zerodds_ast_layout::key::ElementPadding::Packed`],
    /// [`zerodds_ast_layout::key::SequenceElementDelimiter::BeforeEachNonFinalElement`]
    /// and [`zerodds_ast_layout::key::WstringCodeUnits::Utf8`]. The
    /// reference is `zerodds-wire-codec`; a target whose runtime has the
    /// packed-element origin, the element length in DHEADER form and a
    /// UTF-8 `wstring` answers `true` and may be generated under that
    /// dialect. The default is `false`, so a runtime that has not been
    /// given them is refused with the axis named rather than generated in
    /// the specification's form — a different octet string wearing this
    /// key's name.
    fn carries_hdds_axes() -> bool {
        false
    }

    /// A member of a value the codec was handed.
    ///
    /// `v->x` where the parameter is a pointer, `v.x` where it is a
    /// reference. Not [`CodecTarget::field_of`], which reaches into a
    /// value the body already owns.
    fn member(base: &str, field: &str) -> Self::Expr;

    /// The whole value the codec was handed, rather than one member.
    ///
    /// `*v` where the parameter is a pointer, `v` where it is a
    /// reference. What a primitive's codec writes.
    fn whole(base: &str) -> Self::Expr;

    /// An integer literal that has to survive its own width.
    ///
    /// A union label the size of `16045690984833335998` is not an `int`
    /// in any of these languages, and a bare decimal is a constant whose
    /// type the compiler picks — with a warning where the reader is
    /// lucky and a quietly different comparison where they are not. C
    /// writes `UINT64_C(..)`, Java an `L` suffix. Separate from
    /// [`CodecTarget::int`] because most literals are small and the
    /// suffixed form would be noise on all of them.
    fn wide_int(value: i128) -> Self::Expr;

    /// A count, in whatever spelling keeps the target's conversions
    /// quiet.
    ///
    /// A sequence bound, an element count and a member id are all
    /// unsigned on the wire, and C writes them with the `u` suffix so
    /// that a comparison against a `uint32_t` does not trip
    /// `-Wsign-compare`. Java has no unsigned literal and `5u` is not a
    /// token there at all. The walk asks for a count and each backend
    /// spells it.
    fn unsigned_int(value: i128) -> Self::Expr {
        Self::literal(Literal::Int(format!("{value}u")))
    }

    /// A 64-bit pattern, in a spelling wide enough to survive itself.
    ///
    /// A bitmask's declared-bit mask is sixty-four bits and a bare
    /// decimal is a constant whose type the compiler picks. C writes
    /// `UINT64_C(..)`; Java has no unsigned integer and writes the
    /// signed pattern with an `L`.
    fn wide_unsigned(value: u64) -> Self::Expr {
        Self::literal(Literal::Int(format!("UINT64_C({value})")))
    }

    /// A signed 64-bit constant, as [`CodecTarget::wide_unsigned`].
    fn wide_signed(value: i128) -> Self::Expr {
        Self::literal(Literal::Int(format!("INT64_C({value})")))
    }

    /// A member of an expression.
    fn field_of(value: Self::Expr, field: &str) -> Self::Expr;

    /// `a[i]`
    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr;

    /// How many elements the collection in `base` holds.
    ///
    /// `_length` on the C sequence struct, `.length` on a Java array,
    /// and for a fixed-size array the bound the materialiser fixed. The
    /// base name and not an expression, for the reason
    /// [`CodecTarget::elements`] takes one: how a target reaches into
    /// its own collection is the target's.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr;

    /// `a op b`, written as given.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr;

    /// `op a`, written as given.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr;

    /// Parentheses written on purpose.
    ///
    /// The trees carry no precedence, so a walk that needs a bracket
    /// says so and every backend honours it.
    fn paren(value: Self::Expr) -> Self::Expr;

    /// A conversion the target requires and the walk knows is exact.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr;

    /// A value on its way into a nested codec.
    ///
    /// The abstraction of `&v->x` against `v.x`; see [`Handoff`].
    fn handoff(value: Self::Expr, form: Handoff) -> Self::Expr;

    /// All of `count` octet elements of `base` at once, in place of the
    /// element loop of a `sequence<octet>` or a one-dimensional
    /// `octet[N]` (and the same over `uint8`); `None`, the default, keeps
    /// the loop. The octets are the loop's: an octet has alignment 1 and
    /// no byte order (XTypes 1.3 §7.4.3.4.1), so a run of them is the same
    /// bytes however it is copied. What a target gains is the per-element
    /// call — measured in the C++ column: `sequence<octet, 8192>` element
    /// by element cost ~100 µs per roundtrip of four codec passes.
    fn bulk_octets_write(base: &str, count: Self::Expr) -> Option<Self::Stmt> {
        let _ = (base, count);
        None
    }

    /// The read side of [`Self::bulk_octets_write`]: `count` octets into
    /// `base`, which already holds room for them.
    fn bulk_octets_read(base: &str, count: Self::Expr) -> Option<Self::Stmt> {
        let _ = (base, count);
        None
    }

    // -- statements ---------------------------------------------------

    /// A local declaration with an optional initialiser.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt;

    /// An expression evaluated for its effect.
    fn perform(value: Self::Expr) -> Self::Stmt;

    /// `return`, with or without a value.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt;

    /// `if (cond) { .. } else { .. }`
    fn when(condition: Self::Expr, then: Self::Block, otherwise: Option<Self::Block>)
    -> Self::Stmt;

    /// A counted loop from zero to `bound`, exclusive.
    ///
    /// Given rather than assembled from the pieces because the counter's
    /// type is the target's business: C declares `size_t`, Java `int`,
    /// and a walk that wrote the declaration itself would be choosing
    /// for both.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt;

    /// A loop with no counter.
    ///
    /// A `@mutable` aggregate's headers run until the delimiter is used
    /// up and a PL_CDR1 list until its sentinel; neither has a bound,
    /// and a counted loop with the clauses left empty would claim an
    /// induction variable somebody forgot.
    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt;

    /// Leave the enclosing loop.
    fn leave() -> Self::Stmt;

    /// A comment on its own line.
    fn note(doc: Doc) -> Self::Stmt;

    /// A value read for its effect and not kept.
    ///
    /// `(void)zdw_read_dheader(r);` in C, where an ignored return is a
    /// `-Wunused-result` away from a warning and the cast is how a
    /// caller says it meant it. A target where a call is already a
    /// statement writes the call: `(void) expr;` is not a Java
    /// statement at all.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        Self::perform(Self::cast(Self::nothing(), value))
    }

    /// How this target says a parameter is deliberately not read.
    ///
    /// Most decoders have nothing to do with the try-construct
    /// behaviour — a `long` has no bound to miss — and the C probe is
    /// built with `-Wunused-parameter -Werror`, where that fails the
    /// build. `(void)zd_tc;` is C's way of saying so. A target whose
    /// compiler does not warn on an unread parameter answers with no
    /// statement, which is a legitimate answer and not an omission:
    /// `javac` has no such warning, and `zd_tc;` is not a Java
    /// statement at all.
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![Self::perform(Self::cast(Self::nothing(), Self::name(name)))]
    }

    /// `zd_dec_<T>(r, <destination>, <behaviour>);` — one nested
    /// decoder run, with its answer put where `place` says.
    ///
    /// The read direction's counterpart of [`Handoff`], and a whole
    /// method rather than an argument form because the two targets do
    /// not put the destination in the same syntactic position. C passes
    /// its address and the callee writes through it. Java has no
    /// address to pass and no out-parameter to pass it in, so the
    /// decoder answers with the value and the destination is the
    /// assignment's left-hand side — the same decision, one step later.
    ///
    /// A backend that overrides this is saying its decoders return
    /// their value; [`CodecTarget::routine`] has to agree with it, and
    /// so does [`CodecTarget::give_back`].
    fn read_call(
        callee: &str,
        place: Self::Expr,
        form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        Self::perform(Self::call(
            callee,
            vec![Self::name("r"), Self::handoff(place, form), behaviour],
        ))
    }

    /// The whole sample handed straight on to another type's decoder.
    ///
    /// What a `typedef` reads as: transparent on the wire, transparent
    /// to §7.2.2.7, and in C transparent in storage too — the
    /// destination and the behaviour go through untouched, and neither
    /// an address nor a dereference is taken because `v` is already the
    /// one the caller gave. Separate from [`CodecTarget::read_call`]
    /// because there is no `place` here to take: the destination is the
    /// parameter itself.
    fn read_forward(callee: &str) -> Self::Stmt {
        Self::perform(Self::call(
            callee,
            vec![Self::name("r"), Self::name("v"), Self::name("zd_tc")],
        ))
    }

    // -- assembly -----------------------------------------------------

    /// Room for `count` values of `element`, with `fill` inside it.
    ///
    /// The one operation in the reader that is not a spelling. C sets
    /// the length, nulls the buffer, allocates `count * sizeof(element)`
    /// out of the reader's arena, returns if that fails, and runs the
    /// fill inside the guard — five statements. A target whose
    /// allocation throws rather than returning nothing writes one and
    /// has no guard to put the fill in.
    ///
    /// `base` is the local the codec holds the sequence in, not a
    /// finished expression: how a target reaches into it is the
    /// target's own — C's sequence keeps a `_length` and a `_buffer`
    /// behind a pointer, and a language with a list type has neither.
    /// `fill` is the statements that fill it, already built; a backend
    /// may drop the guard but must not drop the fill.
    ///
    /// Shaped from the C column, which is the only caller so far. A
    /// second reader backend is what will say whether the shape holds —
    /// a target that appends rather than writing into a slot will want
    /// the element's place from here too, and it is better to widen this
    /// when that caller exists than to guess at it now.
    /// `assigned_whole` says what `fill` does with each slot: assign it
    /// outright, as a sequence's element decoder does, or write into it,
    /// as a `map`'s two halves do. It is the difference between a
    /// target whose slot *is* storage and one whose slot is a reference
    /// that has to point at some — C ignores it and Java does not.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt>;

    /// [`CodecTarget::make_room`] for a sequence whose element is
    /// `element_id`. A target that cannot spell a slot's start value from
    /// `element` alone overrides it: Kotlin's `Array(n) { T() }` has no
    /// constructor to call where `T` is a `typealias` of `Int`.
    fn make_room_of(
        _ast: &Ast,
        _element_id: Id<TypeNode>,
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        Self::make_room(base, element, count, assigned_whole, fill)
    }

    /// One scalar, as it comes off the runtime, into a place of
    /// `type_`.
    ///
    /// C's runtime hands back the member's own width and the assignment
    /// is the whole of it. Java has no unsigned type and no integer
    /// narrower than `int` in an expression: `readI8` answers an `int`
    /// because that is what a number in [-128, 127] is once it has been
    /// widened, and a `byte` field does not take one without being told
    /// to. Which of its holders need that is the backend's, and a walk
    /// that cast unconditionally would put a redundant cast in the C
    /// column, where `-Wall` names it.
    fn store(place: Self::Expr, type_: Self::Type, value: Self::Expr) -> Self::Stmt {
        let _ = type_;
        Self::perform(Self::binary("=", place, value))
    }

    /// A local the body writes *into* rather than assigns.
    ///
    /// A C local of a struct type is storage the moment it is declared.
    /// A Java local of a class type is a reference to none, and a body
    /// that assigns its fields would be dereferencing a null. Separate
    /// from [`CodecTarget::declare`], which is for a local the body
    /// assigns before it reads — those need no storage of their own and
    /// a target that made some would be allocating for nothing.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        Self::declare(type_, name, None)
    }

    /// One field of a header the runtime handed back.
    ///
    /// `zd_h.member_id` in C. A target whose runtime publishes its
    /// headers behind accessors answers with the call — `zd_h.memberId()`
    /// — and the walk names the field rather than the spelling. Not
    /// [`CodecTarget::field_of`], which reaches into a value of a
    /// *generated* type, where both columns write a field.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        Self::field_of(value, field)
    }

    /// Where a **sequence**'s element goes while it is being read, and
    /// what it takes to get it there: the statements before, the place,
    /// and the statements after.
    ///
    /// Almost always `(nothing, base[index], nothing)`, and it is a seam
    /// because of one container. `std::vector<bool>` is specialised —
    /// [vector.bool] — and its `operator[]` returns a proxy object, not a
    /// `bool &`. A codec that takes its destination by reference has
    /// nowhere to point, so the C++ column reads into a local and assigns
    /// it back. Every other element of every other target is a real
    /// object and answers with the subscript.
    ///
    /// Only a sequence needs it. `T a[2]` is a `std::array`, which is not
    /// specialised for any element, and a `map`'s halves are a
    /// `std::pair`'s members, which are real objects too.
    ///
    /// The write direction has no such problem: reading a proxy is what
    /// a proxy is for, and `write_bool(v[i])` converts.
    fn sequence_element(
        base: &str,
        index: Self::Expr,
        element: Self::Type,
    ) -> (Vec<Self::Stmt>, Self::Expr, Vec<Self::Stmt>);

    /// Keep the first `count` elements of the collection in `base` and
    /// drop the rest.
    ///
    /// §7.3.1.2.1.12.3's TRIM for a bounded collection, which the reader
    /// applies after every element has been read: stopping at the bound
    /// would leave the rest of the sequence in the stream and put every
    /// following member at the wrong offset.
    ///
    /// A statement and not an assignment to [`CodecTarget::count_of`],
    /// which is what this used to be. C's sequence is a `_length` beside
    /// a `_buffer` and the length *is* the answer — the elements past
    /// the bound stay in the buffer, unreferenced. A `std::vector` knows
    /// its own size and `v.size() = 5u` is not an assignment; a
    /// `List` shortens differently again. The walk says how many to
    /// keep and the target says how.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt>;

    /// Whether an `@optional` member is present.
    ///
    /// The last structural assumption C made and no other target
    /// shares. C declares a `boolean _has_m` beside the member and this
    /// reads it; `adapter/java.rs` declares one boxed field and lets
    /// `null` mean absent, "because a `boolean _has_m` and an `m` are
    /// two things that can disagree, and nothing in the type system
    /// stops them". A backend that inherited C's flag would reach a
    /// field the materialiser never emitted.
    fn present(base: &str, field: &str) -> Self::Expr;

    /// Say that an `@optional` member is present, or is not.
    ///
    /// C assigns the parallel flag. A target where absence is the value
    /// assigns nothing on the way in — writing the member is what makes
    /// it present — and assigns [`CodecTarget::absent`] on the way out.
    /// Returning no statement is a legitimate answer.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt>;

    /// Statements that leave every case member of a union unset, before
    /// the member the discriminator selects is read.
    ///
    /// The decoder starts from the union's Table 9 default, which carries
    /// the member the default discriminator selects. A column whose union
    /// has a "not set" for a case field — Python's and Julia's `None` and
    /// `nothing`, Swift's `nil`, Nim's and OCaml's `Option`, a missing
    /// key or property in Lua, Node and TypeScript, Elixir's `nil` — and
    /// promises that at most one case is live has to clear that member,
    /// or a decoded sample carries the default branch's value beside the
    /// one read (measured: `idl-conformance/_interop` `mapenum`, a `Sel`
    /// read with `_d = 2` came back with `n = 9` *and* `z = 0`). Nothing
    /// by default: in C the cases share storage, and where a case field
    /// has no "not set" (C++, C#, Java, Go, D, Zig, F#, Ada, Kotlin) its
    /// default value is what an unselected field holds anyway.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        let _ = (base, fields);
        Vec::new()
    }

    /// Statements that make an array's Table 9 default writable, before
    /// its elements are read into it; `lengths` are its dimensions,
    /// outermost first.
    ///
    /// Nothing by default: every column but one holds an array in
    /// something its elements can be assigned into. Python declares the
    /// member `tuple[T, …]` (`adapter/python.rs`) and a tuple cannot be
    /// written into, so its default is a tuple and the reader turns it
    /// into lists here and back in [`CodecTarget::close_array`].
    fn open_array(base: &str, lengths: &[i128]) -> Vec<Self::Stmt> {
        let _ = (base, lengths);
        Vec::new()
    }

    /// Statements that give a read array the type its member declares,
    /// after its last element is read. Nothing by default; see
    /// [`CodecTarget::open_array`].
    fn close_array(base: &str, lengths: &[i128]) -> Vec<Self::Stmt> {
        let _ = (base, lengths);
        Vec::new()
    }

    /// A union's selected case member.
    ///
    /// C declares the cases in an anonymous `union _u` inside the
    /// struct, so a member is `v->_u.x`; `adapter/java.rs` declares one
    /// field per case directly on the class, so it is `v.x`.
    fn case_member(base: &str, field: &str) -> Self::Expr;

    /// A union's discriminant.
    ///
    /// `_d` in every column whose materialiser names it that, which is
    /// all of them but one: DDS-TS 1.0 §7.5 names the property
    /// `discriminator`, and the TypeScript materialiser declares it
    /// under that name. The default is the name the other columns
    /// share, so a column that does not override generates exactly what
    /// it generated before this seam existed.
    fn discriminant(base: &str) -> Self::Expr {
        Self::member(base, "_d")
    }

    /// The integer an enumeration's value carries, given the value.
    ///
    /// The identity in every column but one, and deliberately so: an
    /// enumeration is a numeric constant at the language surface there,
    /// so the value in the variable *is* the integer — under
    /// [`zerodds_ast_layout::EnumeratorValue::Declared`] it is even the
    /// integer that goes on the wire, which is why
    /// [`encode::enumeration_write`] emits no chain for that setting.
    ///
    /// DDS-TS 1.0 §7.6 makes an enumeration a string-literal union, so
    /// that column has no integer in the variable at all and answers
    /// with a lookup in the generated `<Enum>Ordinal` table. Measured:
    /// without this seam `enum-sparse-values` wrote `70 79 87` — the
    /// first code unit of each enumerator's name — where the ledger has
    /// `65535 1 100`.
    ///
    /// [`encode::enumeration_write`]: crate::emit::encode
    fn enumerator_ordinal(ast: &Ast, id: Id<TypeNode>, value: Self::Expr) -> Self::Expr {
        let _ = (ast, id);
        value
    }

    /// The value a union case label is compared against, in this
    /// target's spelling of the discriminator's type.
    ///
    /// The default is the label folded to an integer and narrowed to the
    /// discriminator's own type, which is what every column with a
    /// numeric enumeration needs. A column whose enumeration is not
    /// numeric at the language surface — DDS-TS 1.0 §7.6 makes it a
    /// string-literal union — cannot compare a discriminant against a
    /// number at all, and answers with its own constant.
    fn case_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Self::Expr {
        Self::cast(Self::value_type(ast, discriminator), Self::wide_int(value))
    }

    /// One half of a `map`'s entry.
    ///
    /// C generates a struct with `key` and `value`; C++'s entry is a
    /// `std::pair`, whose halves are `first` and `second`. The walk asks
    /// for the half and the backend names it.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr;

    /// The place a member is reached at, given that the column could
    /// not put the member's value inside its own aggregate.
    ///
    /// [`zerodds_ast_layout::member_held_inline_closes_a_cycle`] is what
    /// says so: the member's type reaches the aggregate that declares it
    /// over nothing but inline edges, so a column whose member *is* the
    /// value has no size for it. `place` is the place the seams below
    /// would have reached — the member, the `@optional` accessor, the
    /// `@external` one, or a union branch — and the answer is that place
    /// as the column keeps it once the indirection is there.
    ///
    /// The default is the place unchanged, and it is the right answer
    /// for fifteen of the seventeen columns: a member that is already a
    /// reference — Python's `Optional[T]`, Rust's `Option<Box<T>>`,
    /// Java's boxed field — has a size whatever it points at, so nothing
    /// about it changes when it closes a cycle. C is the column that
    /// asks: `T next;` beside a `_has_next` flag is storage that
    /// contains itself, and the member has to become `T *next`.
    ///
    /// Not folded into [`CodecTarget::optional_value`]: that seam is
    /// handed the member's type and not the aggregate that declares it,
    /// and the cycle is a fact about the pair. The call sites hold both
    /// and ask this only where the pair closes one, so a column that
    /// takes the default emits exactly the source it emitted before this
    /// seam existed.
    fn behind_indirection(ast: &Ast, id: Id<TypeNode>, place: Self::Expr) -> Self::Expr {
        let _ = (ast, id);
        place
    }

    /// The reading counterpart of [`CodecTarget::behind_indirection`]:
    /// the same place, and what has to happen before a reader can write
    /// through it.
    ///
    /// A column that keeps an indirection has nothing to write into
    /// until the indirection points somewhere, so this is where the
    /// storage is made. `id` is the member's type, which is what an
    /// allocation needs to know the size of.
    ///
    /// Returns the statements that make room, followed by the place, in
    /// the shape [`CodecTarget::optional_destination`] uses and for the
    /// same reason.
    fn make_indirection(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        let _ = (ast, id);
        (Vec::new(), place)
    }

    /// Where a *reader* puts an `@optional` member's value, and what it
    /// takes to have somewhere to put it.
    ///
    /// The write direction has [`CodecTarget::optional_value`], which
    /// reaches a value that is already there. Reading is the other way
    /// round and the two targets differ in a way no expression covers:
    /// C's optional member is a plain `T` beside a `_has_` flag, so the
    /// destination is the member and nothing has to happen first; C++'s
    /// is a `std::optional<T>` and the member's decoder takes a `T&`, so
    /// the optional has to be *engaged* before there is a `T` to write
    /// into at all.
    ///
    /// Returns the statements that make room, followed by the place. The
    /// statements go immediately before the read; a backend that needs
    /// none returns an empty vector, and the generated source is then
    /// exactly what it was before this seam existed.
    ///
    /// Not folded into [`CodecTarget::set_present`]: that is called
    /// *after* the read at two of the five sites and before it at the
    /// other three, and moving it would reorder the C column's output
    /// for a difference that is not C's.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr);

    /// The value an `@optional` member holds, given that it holds one.
    ///
    /// C's optional member is a plain `T` beside a `_has_` flag, so this
    /// is the member. C++'s is a `std::optional<T>` and the codec takes
    /// a `T`, so it is `.value()`. Java's boxed field is already the
    /// value.
    ///
    /// The member's type is a parameter because one target's answer
    /// depends on it and no other's does. `adapter/csharp.rs` wraps a
    /// *value* type in `Nullable<T>` and leaves a reference type alone,
    /// so the unwrapping is `.Value` for the first and nothing for the
    /// second — and `string?.Value` does not compile any more than
    /// passing an `int?` where an `int` is wanted does. A backend that
    /// answered from the name alone would be right for one half of its
    /// own mapping.
    ///
    /// Only ever reached inside a branch the presence test guarded, so a
    /// target whose accessor throws on an absent value is safe here.
    fn optional_value(ast: &Ast, id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr;

    /// Whether this column holds an `@optional` or `@external` member of
    /// `aggregate`, of type `member_type`, behind a reference of its own
    /// — where the shared walk would reach it through
    /// [`CodecTarget::present`], [`CodecTarget::optional_value`] and
    /// their `external_` counterparts. A member this answers for goes
    /// through [`CodecTarget::reference_present`],
    /// [`CodecTarget::reference_value`],
    /// [`CodecTarget::reference_destination`] and
    /// [`CodecTarget::reference_absent`] instead.
    ///
    /// `false` by default, which leaves every column's output as it
    /// was. Ada answers from its adapter
    /// (`crate::adapter::ada::held_by_reference`): a member whose type is
    /// still incomplete where the aggregate is declared is an access
    /// type there (RM 3.10.1), which depends on the order Ada declares
    /// in and on nothing the layout plans. The generated Ada for
    /// `recursive-optional-external` asked such a member for `Present`
    /// and did not compile (44 matrix cells, 2026-10-01).
    fn holds_by_reference(
        ast: &Ast,
        aggregate: Id<TypeNode>,
        member_type: Id<TypeNode>,
        optional: bool,
        external: bool,
    ) -> bool {
        let _ = (ast, aggregate, member_type, optional, external);
        false
    }

    /// Whether a member [`CodecTarget::holds_by_reference`] answers for
    /// is present. Reached only where that answered `true`.
    fn reference_present(base: &str, field: &str) -> Self::Expr {
        Self::present(base, field)
    }

    /// The value behind such a member, given that it is present.
    fn reference_value(ast: &Ast, id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::optional_value(ast, id, base, field)
    }

    /// Where a reader puts such a member's value, and what it takes to
    /// have somewhere to put it; `id` is the member's type.
    fn reference_destination(
        ast: &Ast,
        id: Id<TypeNode>,
        base: &str,
        field: &str,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        let _ = (ast, id);
        Self::optional_destination(base, field)
    }

    /// The statements that leave such a member absent.
    fn reference_absent(base: &str, field: &str) -> Vec<Self::Stmt> {
        Self::set_present(base, field, false)
    }

    /// Whether this column keeps a member that closes a type cycle the
    /// way it keeps an `@external` one — through the `external_` seams
    /// below, whether or not the member carries the annotation.
    ///
    /// `false` by default, which leaves a column's output as it was. The
    /// columns that answer `true` are the ones whose `@external` member
    /// is an indirection of their own (C++'s `std::shared_ptr`, D's and
    /// Zig's pointer, Nim's `ref`, Swift's `ZdExternal`) and whose plain
    /// member is the value, so a member on a cycle needs the same holder;
    /// [`crate::adapter::held_externally`] is the rule, and their adapters
    /// and defaulters ask it too. Ada decides by its declaration order
    /// ([`CodecTarget::holds_by_reference`]) and C by the pointer it declares
    /// (`behind_pointer`), so neither takes this.
    fn holds_a_cycle_externally() -> bool {
        false
    }

    /// Whether this column holds an `@external` member behind a pointer
    /// of its own, where the member's value is not a pointer already.
    ///
    /// XTypes 1.3 §7.5.1.2.3.1 maps an external member of C to
    /// "pointer-to-X". The member is then reached through
    /// [`CodecTarget::behind_indirection`] and a reader makes storage for
    /// it with [`CodecTarget::make_indirection`], the way both do for a
    /// member that closes a cycle (`behind_pointer` asks both questions
    /// at once). `false` by default; the columns that keep an `@external`
    /// member behind a holder of their own go through the `external_`
    /// seams instead.
    fn externals_are_pointers() -> bool {
        false
    }

    /// Whether this column holds an `@optional` member behind a pointer
    /// of its own, where the member's value is not a pointer already.
    ///
    /// XTypes 1.3 §7.5.1.2.4.1 maps an optional member of C to
    /// "pointer-to-X" and says "A NULL pointer shall indicate an omitted
    /// value"; §7.5.1.2.4.2 does the same for C++. The member is then
    /// reached through [`CodecTarget::behind_indirection`] and a reader
    /// makes storage for it with [`CodecTarget::make_indirection`],
    /// exactly as for an `@external` one; [`CodecTarget::present`] asks
    /// whether the pointer is not NULL. `false` by default: every other
    /// column holds an absent value in its own way.
    fn optionals_are_pointers() -> bool {
        false
    }

    /// Whether the column's value of `id` is a pointer already, so that
    /// an `@external` or `@optional` member of that type needs no pointer
    /// of its own. C's `string`, `wstring` and `fixed` are
    /// `const char *` (XTypes 1.3 §7.5.1.2.3.1); a C++ `std::string` is
    /// a value.
    fn value_is_pointer(ast: &Ast, id: Id<TypeNode>) -> bool {
        crate::adapter::c::value_is_pointer(ast, id)
    }

    /// Statements that stop a writer from going on when an `@external`
    /// member that is not `@optional` has no value at the time of the
    /// write.
    ///
    /// XTypes 1.3 §7.5.1.2.3.1.1 (C) and §7.5.1.2.3.2.1 (C++): "If the
    /// member is only external but not optional, then it is not allowed
    /// for the member to be NULL at the time of a write." `present` says
    /// whether the member has a value. Nothing by default: a column that
    /// holds an external member inline has no unset state to refuse.
    fn refuse_unset_external(present: Self::Expr) -> Vec<Self::Stmt> {
        let _ = present;
        Vec::new()
    }

    /// The value of an `@external` member, as the member's codec takes
    /// it.
    ///
    /// DDS-XTypes 1.3 Table 21: `@external` changes no octet — it says
    /// where the binding keeps the value. Most columns keep it inline,
    /// and the default is then exactly what the walk wrote before this
    /// seam existed: the member, or the optional accessor where the
    /// member is also `@optional`. A column that keeps it behind an
    /// indirection — D's `T*`, which is also what gives a
    /// self-recursive type a size at all — answers with the
    /// dereference, for both. A walk that reached the member directly
    /// would hand a pointer to a codec that takes the value.
    fn external_value(
        ast: &Ast,
        id: Id<TypeNode>,
        base: &str,
        field: &str,
        optional: bool,
    ) -> Self::Expr {
        if optional {
            return Self::optional_value(ast, id, base, field);
        }
        Self::member(base, field)
    }

    /// Where a *reader* puts an `@external` member's value, and what it
    /// takes to have somewhere to put it. The reading counterpart of
    /// [`CodecTarget::external_value`], shaped like
    /// [`CodecTarget::optional_destination`] and for the same reason: a
    /// column whose member is an indirection has to make storage before
    /// there is a value to write into. The default is the walk's old
    /// answer for both combinations.
    fn external_destination(
        base: &str,
        field: &str,
        optional: bool,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        if optional {
            return Self::optional_destination(base, field);
        }
        (Vec::new(), Self::member(base, field))
    }

    /// Whether an `@optional @external` member is present.
    ///
    /// Separate from [`CodecTarget::present`] because the two
    /// annotations can answer differently in one column: D's plain
    /// `@optional` is `Nullable!(T)` and its `isNull`, while
    /// `@optional @external` is one pointer and its `null` — a wrapper
    /// over the pointer would carry a present-but-null state the wire
    /// form has no encoding for.
    fn external_present(base: &str, field: &str) -> Self::Expr {
        Self::present(base, field)
    }

    /// Say that an `@optional @external` member is present, or is not.
    /// The counterpart of [`CodecTarget::set_present`], split for the
    /// reason [`CodecTarget::external_present`] gives.
    fn external_set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        Self::set_present(base, field, present)
    }

    /// The value of an `@external` union branch, as the branch's codec
    /// takes it. The union counterpart of
    /// [`CodecTarget::external_value`], reached through
    /// [`CodecTarget::case_member`] because that is the accessor a
    /// branch has; there is no `optional` parameter because no union
    /// member is `@optional` (DDS-XTypes 1.3 §7.2.2.4.4.4.7). The
    /// default is the accessor itself — the columns that keep the value
    /// inline — and a column that keeps an indirection answers with the
    /// dereference, exactly as it does for a structure member.
    fn external_case_value(ast: &Ast, id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        let _ = (ast, id);
        Self::case_member(base, field)
    }

    /// Where a *reader* puts an `@external` union branch's value, and
    /// what it takes to have somewhere to put it. The union counterpart
    /// of [`CodecTarget::external_destination`], shaped the same way
    /// and defaulted the same way.
    fn external_case_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::case_member(base, field))
    }

    /// Where a sequence keeps the elements themselves.
    ///
    /// C's sequence is a `_length` and a `_buffer` and this is the
    /// buffer; a target whose sequence *is* the array answers the value.
    fn elements(base: &str) -> Self::Expr;

    /// Whether a value the reader had to materialise is there at all.
    ///
    /// C's `zdw_read_string_alloc` answers `NULL` where the reader's
    /// heap is exhausted, and every use of what it produced is guarded
    /// against that — a decoder that measured a null string would read
    /// through the pointer. A target whose string type owns its own
    /// storage has no such state and answers `None`, and the guard is
    /// then not written at all rather than written against a value that
    /// is always there: `std::string != std::nullopt` is not an
    /// expression.
    ///
    /// Distinct from [`CodecTarget::present`], which asks about an
    /// `@optional` member — a property of the sample. This asks whether
    /// the *read* produced anything, which is a property of the runtime.
    fn materialised(value: Self::Expr) -> Option<Self::Expr>;

    /// The value that says a target holds nothing.
    ///
    /// `NULL` in C, `null` in Java and C#, `None` in Python. What a
    /// decoder writes into a pointer before it has allocated, and what
    /// it compares against to find out whether the allocation worked.
    fn absent() -> Self::Expr;

    /// Statements gathered into a body.
    fn block(statements: Vec<Self::Stmt>) -> Self::Block;

    /// A body taken apart again.
    ///
    /// A walk that has built a nested block and then finds it belongs
    /// inline — an array's innermost loop under a `DHEADER`, a union's
    /// if-chain after the last arm — would otherwise have to carry a
    /// `Vec` and a `Block` in parallel and keep them agreeing.
    fn into_statements(block: Self::Block) -> Vec<Self::Stmt>;

    /// One generated name, in the spelling this target publishes it
    /// under.
    ///
    /// The identity everywhere but Go, and it exists because Go has no
    /// visibility keyword: an identifier is exported if and only if its
    /// first character is upper case (Go specification, "Exported
    /// identifiers"), so a name is the only place a Go column can put
    /// the decision. Applied inside
    /// [`crate::emit::encode::codec_name`] and
    /// [`crate::emit::decode::decoder_name`], which is what makes it a
    /// single decision: every caller of either — the walk's own nested
    /// calls, `wireprobe-zerodds-go-gen`'s entry point, a test naming a
    /// function — gets the published spelling without asking.
    ///
    /// A target that overrides this owes the transform an inverse in
    /// name only: it must be injective, or two codecs collide under one
    /// name and no compiler reports that as a collision.
    fn published_name(name: String) -> String {
        name
    }

    /// A codec function or method.
    ///
    /// `body` absent asks for a forward declaration, which C needs ahead
    /// of the bodies so a recursive or mutually recursive type does not
    /// depend on the order the walk visited its types in. A target that
    /// has no such thing returns `None` and the caller emits nothing.
    ///
    /// [`Routine::tolerate_unused`] marks a codec that may legitimately
    /// have no caller. Generated code is compiled with an uncalled
    /// function as an error, and that is worth keeping: normally it
    /// means the generator emitted the wrong thing. The exception is a
    /// codec reached from one position and written inline at another,
    /// where having a caller is a property of the dialect rather than
    /// of the type graph.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl>;

    /// A new reader entry with an explicit, target-derived initializer.
    /// Targets that cannot preserve this contract refuse the new entry only;
    /// existing data routines and their initialization remain unchanged.
    fn initialized_reader(_spec: Routine<Self>, _initializer: Self::Stmt) -> Option<Self::Decl> {
        None
    }

    // -- the materialiser's boundary ----------------------------------

    /// How this target spells the type of `id` where a codec holds it.
    ///
    /// The materialiser's answer, not the walk's: a `sequence<long>` is
    /// a struct with a buffer and a length in C, a `std::vector` in C++
    /// and a `List<Integer>` in Java, and only the target knows which.
    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type;

    /// The 64-bit pattern a bitmask arrives as, in the bitmask's own
    /// declared type.
    ///
    /// One cast to [`Self::value_type`] by default, which every target
    /// with a real conversion operator answers with — `(Flags)zd_bits`
    /// in C, `(int) zd_bits` in Java. TypeScript overrides it: its
    /// declared bitmask type is an alias whose holder is a `number` up
    /// to 32 bits, its `as` is erased and refuses `bigint` to `number`
    /// outright (TS2352), so the narrowing has to be a runtime
    /// conversion through the holder before the alias names it.
    fn bitmask_value(ast: &Ast, id: Id<TypeNode>, value: Self::Expr) -> Self::Expr {
        Self::cast(Self::value_type(ast, id), value)
    }

    /// Table 9's value for `id`, written into `place`.
    ///
    /// What a reader puts where a member the stream did not carry
    /// belongs — DDS-XTypes 1.3 Table 9. The value is the
    /// specification's and the same in every column; what differs is
    /// where a target keeps it. C calls a generated defaulter because a
    /// struct-valued default is not an expression it can assign; a
    /// target with literals for its aggregates assigns one.
    ///
    /// # Errors
    /// When the target has no form for Table 9's value. Fallible
    /// because a backend under construction has to be able to refuse
    /// here: a reader generated with a hole in it and no error is a
    /// reader that fails where it runs rather than where it was built,
    /// and the two are a long way apart.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError>;

    /// The declared `@default` of `node`, written into `place`, or
    /// Table 9's value where the member declares none.
    ///
    /// Fallible on purpose: a `@default` a target cannot spell is
    /// reported rather than quietly replaced by Table 9's zero, which
    /// is a different sample from the one the designer declared.
    ///
    /// # Errors
    /// When the declared default has no form in this target.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError>;

    /// An IDL identifier as this target spells a member or a field.
    ///
    /// Every language escapes the identifiers that collide with its own
    /// keywords, and each escapes them differently — C prefixes, Java
    /// prefixes with a different mark, C# writes `@`. Both halves of a
    /// column have to agree: a member the materialiser declared escaped
    /// and the codec reaches unescaped is a field no compiler finds.
    fn escape(name: &str) -> String;

    /// The path from a derived structure to the base members `depth` levels up,
    /// for a column that binds inheritance as a first member `parent`
    /// (XTypes 1.3 §7.5.1.4.1.3), ending in the separator: `parent.` for the
    /// base, `parent.parent.` for its base. `None`, the default, is a column
    /// that declares the base's members in the derived structure.
    fn inherited_path(depth: usize) -> Option<String> {
        let _ = depth;
        None
    }

    /// The name this target gives a `map`'s entry type.
    ///
    /// A `map<K, V>` has no direct form in most targets and becomes a
    /// collection of a generated pair; what that pair is called is the
    /// materialiser's, and the codec has to call it the same thing.
    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String;

    /// The type a `map`'s entry is *declared* with — the reader's
    /// scratch entry and `make_room`'s element.
    ///
    /// The default names [`CodecTarget::map_entry_name`], which is
    /// right wherever the materialiser declares a type of that name. It
    /// is not right in C++, whose materialiser spells the entry as
    /// `std::pair<K, V>` and declares nothing under the tag —
    /// `map_string_long zd_scratch;` was a name no compiler finds (20
    /// zerodds-cpp read cells across the map family).
    fn map_entry_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        Self::named(&Self::map_entry_name(ast, id))
    }

    /// The constant one enumerator of `id` is named by.
    ///
    /// The materialiser's spelling. C flattens the modules into a
    /// prefix and writes a bare identifier; Java declares a class of
    /// `public static final int` and every use is written from the
    /// outer class down — `ZdTypes.common.Colour.BLUE`. A walk that
    /// built the name from [`CodecTarget::module_prefix`] alone would
    /// emit an unqualified `BLUE`, which in Java names nothing.
    ///
    /// The default is C's and not the general answer: it is
    /// [`CodecTarget::module_prefix`] and [`CodecTarget::escape`] put
    /// together, which works wherever an enumerator sits in the
    /// enclosing scope. C++ declares a *scoped* enumeration, so the
    /// same enumerator is `::a::b::Colour::red` and carries the type's
    /// own name in the middle, which no prefix supplies; that column
    /// overrides.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        Self::name(&format!(
            "{}{}",
            Self::module_prefix(ast, id),
            Self::escape(enumerator)
        ))
    }

    /// The prefix this target puts before a module's declarations.
    ///
    /// C has no namespaces and flattens `module a { module b { … } }`
    /// into a prefix; a target with namespaces answers the empty
    /// string and lets its own scoping do the work.
    fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String;

    /// The name this target gives the type of `id`.
    ///
    /// What a generated codec function is named after, and the reason a
    /// codec name is built from this rather than from the IDL: two IDL
    /// types that collapse to one name in a target must not end up with
    /// one codec, and a target that escapes a keyword has to escape it
    /// the same way in both halves.
    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String;
}
