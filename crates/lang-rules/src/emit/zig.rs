// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Zig backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as the columns before
//! it, and none of them calls another. Where two columns disagree on an
//! octet, one has misread the specification — which is the only reason
//! to have a further one.
//!
//! # The five decisions this column turns on
//!
//! **Errors travel as error unions, and the seam writes the `try`.**
//! Zig has no exceptions: a runtime operation that can refuse answers
//! `zdwire.WireError!T`, every generated routine answers
//! `zdwire.WireError!void` or `!T`, and this backend prepends `try` at
//! each fallible call. Which operations are fallible is a contract
//! between this file and `runtime/zig/zdwire.zig` — the two tables
//! `FALLIBLE_EXEMPT_METHODS` and `FALLIBLE_STATICS` are that
//! contract's edge, and a name moved on one side without the other is a
//! Zig compile error (`expected error union type`), never a silent
//! drop.
//!
//! **A member holds its wire width, under Zig's own names.** An IDL
//! `long` is `i32`, a `char` is `u8`, a `wchar` `u16`, so
//! [`CodecTarget::code_point`] is the identity. A `long double` is a
//! real `f128` — the one column whose built-in set reaches it; the
//! adapter's table carries the argument.
//!
//! **There is no one cast operator, so [`CodecTarget::cast`] chooses.**
//! `@as(T, @intCast(v))` between integer widths, `@enumFromInt` into an
//! enumeration (the walk casts a union label into its discriminator's
//! type), `@bitCast` for the walk's `signed char` reading of a map key,
//! and `zdwire.asI64`/`asU64` where the walk widens a value it cannot
//! see the type of — an enumeration sample and a union overlay arm are
//! the two, and a comptime `switch` on `@typeInfo` answers for both.
//!
//! **`@optional` is `?T` and `@external` is `*T`, and the read
//! direction allocates.** A decoded `T` coerces into a `?T` on its own;
//! a `*T` needs storage, which `zdwire.zdPtr(r.heap, …)` takes from the
//! reader's allocator — the same arena everything else the reader
//! materialises comes from.
//!
//! **Zig's unused-and-shadowing rules are settled in [`Zig::routine`],
//! once.** An unused parameter and a pointless discard are both compile
//! errors, and which one a body earns depends on the whole body; the
//! routine assembler scans it and keeps exactly the discards that are
//! needed. A counted loop renders inside its own block, so two
//! sequential loops can share a counter name without Zig's shadowing
//! error — that one lives in the renderer.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::zig::{field_name, zig_map_entry_name, zig_type_name, zig_value_type};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::zig;

/// The Zig backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zig;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The name the generated file imports the runtime under.
const RUNTIME: &str = "zdwire";

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`; the Zig member is a `u8` and the signed reading is
/// `@as(i8, @bitCast(..))`.
const SIGNED_CHAR: &str = "signed char";

/// The reader and writer operations that answer plainly, everything
/// else on a receiver being `WireError!T`.
///
/// One half of the fallibility contract with `runtime/zig/zdwire.zig`;
/// the module documentation says why a mismatch is loud.
const FALLIBLE_EXEMPT_METHODS: &[&str] = &[
    "reader_status",
    "reader_position",
    "reader_remaining",
    "reader_save",
    "reader_restore",
    "reader_refuse",
    "reader_clear_discard",
    "reader_begin_packed_element",
    "reader_finish_packed_element",
    "begin_packed_element",
    "finish_packed_element",
    "finish_pid_read",
    "position",
    "trim_utf8",
    "trim_utf16",
];

/// The free functions of the runtime that answer `WireError!T`;
/// everything else [`CodecTarget::runtime_static`] names answers
/// plainly. The other half of the contract.
const FALLIBLE_STATICS: &[&str] = &["utf16_units"];

/// An expression, with the two properties the walk's seams cannot
/// carry: whether it needs a `try`, and which indirection the place it
/// names lives behind.
#[derive(Debug, Clone)]
pub enum ZigExpr {
    /// An ordinary expression.
    Plain(zig::Expr),
    /// A call that answers `WireError!T`: `try` in a statement or
    /// initialiser position, `(try …)` inside a larger expression.
    Fallible(zig::Expr),
    /// The `?T` field of an `@optional` member, as a place a decoded
    /// `T` is about to be written into — the coercion is Zig's own.
    OptionalSlot(zig::Expr),
    /// The `*T` field of an `@external` member, likewise; the decoded
    /// value goes through `zdwire.zdPtr` to get storage.
    ExternalSlot(zig::Expr),
}

/// A Zig type, with the two properties [`CodecTarget::cast`] needs and
/// the tree cannot carry: whether the type is an enumeration once
/// typedefs are seen through, which decides `@enumFromInt` against
/// `@intCast`, and whether a named type is an integer alias, which
/// decides `@intCast` against a bare `@as`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZigType {
    /// The type as written.
    pub tree: zig::Type,
    /// Whether a cast into it converts from an integer with
    /// `@enumFromInt`.
    pub enum_like: bool,
    /// Whether the type names an integer under its own name — a
    /// bitmask, whose Zig form is `pub const Flags = u8;`. The tree
    /// says `Named("Flags")` and `@as(Flags, zd_bits)` on the walk's
    /// sixty-four-bit holder is `expected type 'u8', found 'u64'`;
    /// the narrowing has to be stated.
    pub integer_alias: bool,
}

impl ZigType {
    fn plain(tree: zig::Type) -> Self {
        Self {
            tree,
            enum_like: false,
            integer_alias: false,
        }
    }
}

/// The expression inside, whichever form it took; a fallible one is
/// bracketed so `try` binds to the call and not to whatever expression
/// it ends up inside.
fn inner(expr: ZigExpr) -> zig::Expr {
    match expr {
        ZigExpr::Plain(e) | ZigExpr::OptionalSlot(e) | ZigExpr::ExternalSlot(e) => e,
        ZigExpr::Fallible(e) => zig::Expr::Paren(Box::new(zig::Expr::try_(e))),
    }
}

/// The expression with a bare `try` where one is due — an initialiser
/// and an assignment right-hand side take it without the bracket.
fn inner_bare(expr: ZigExpr) -> zig::Expr {
    match expr {
        ZigExpr::Plain(e) | ZigExpr::OptionalSlot(e) | ZigExpr::ExternalSlot(e) => e,
        ZigExpr::Fallible(e) => zig::Expr::try_(e),
    }
}

fn plain(e: zig::Expr) -> ZigExpr {
    ZigExpr::Plain(e)
}

/// A local's name, moved off Zig's primitive namespace: the encode walk
/// counts an array's dimensions with `i0`, `i1`, …, which Zig parses as
/// arbitrary-width integer types and refuses as locals. The prefix and
/// not `escape_decl`'s trailing underscore, because the lexer reads
/// `i0_` as a malformed width too ("primitive integer type 'i0_' has
/// leading zero", Zig 0.14.1). Applied at every place a local is
/// written or read, so the two spellings cannot part.
fn local_name(text: &str) -> String {
    if zig::is_primitive_name(text) {
        return format!("zd_{text}");
    }
    text.to_string()
}

impl Zig {
    /// A runtime name, qualified: `zdwire.X`.
    fn runtime_name(name: &str) -> zig::Expr {
        zig::Expr::dot(zig::Expr::Name(Ident::new(RUNTIME)), name)
    }

    /// A runtime operation's Zig spelling: `write_u32` is `writeU32`,
    /// each `_`-separated segment after the first with its first letter
    /// raised — the names `runtime/zig` publishes, because that runtime
    /// was written against this conversion.
    fn camel(method: &str) -> String {
        let mut out = String::new();
        for (index, segment) in method.split('_').enumerate() {
            if index == 0 {
                out.push_str(segment);
                continue;
            }
            let mut chars = segment.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
        out
    }

    fn text(value: &str) -> ZigExpr {
        plain(zig::Expr::Literal(Literal::Str(value.to_string())))
    }

    /// One value of a runtime enumeration: `zdwire.Status.ok`.
    fn runtime_enum(type_: &str, value: &str) -> ZigExpr {
        plain(zig::Expr::dot(Self::runtime_name(type_), value))
    }

    /// `r.heap` — the reader's arena, which is where everything the
    /// read direction materialises comes from.
    fn reader_heap() -> zig::Expr {
        zig::Expr::dot(zig::Expr::Name(Ident::new("r")), "heap")
    }

    /// Whether the resolved type behind `id` is an enumeration.
    fn is_enumeration(ast: &Ast, id: Id<TypeNode>) -> bool {
        super::resolves_to_enumeration(ast, id)
    }

    /// Whether the resolved type behind `id` is a bit mask, whose Zig
    /// form is a `pub const` alias of an unsigned integer rather than a
    /// type of its own.
    fn is_bitmask(ast: &Ast, id: Id<TypeNode>) -> bool {
        let mut current = id;
        for _ in 0..MAX_TYPE_DEPTH {
            match ast.type_(current).map(|node| node.kind.clone()) {
                Ok(TypeKind::Bitmask { .. }) => return true,
                Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                    Some(target) => current = target,
                    None => return false,
                },
                _ => return false,
            }
        }
        false
    }

    /// Whether `tree` is a fixed-width integer or `usize` — the types a
    /// cast reaches with `@intCast`.
    fn is_integer_type(tree: &zig::Type) -> bool {
        match tree {
            zig::Type::Primitive(primitive) => matches!(
                primitive,
                zig::Primitive::I8
                    | zig::Primitive::I16
                    | zig::Primitive::I32
                    | zig::Primitive::I64
                    | zig::Primitive::U8
                    | zig::Primitive::U16
                    | zig::Primitive::U32
                    | zig::Primitive::U64
            ),
            zig::Type::Named(name) => name.as_str() == "usize",
            _ => false,
        }
    }
}

impl CodecTarget for Zig {
    type Expr = ZigExpr;
    type Stmt = zig::Stmt;
    type Block = Vec<zig::Stmt>;
    type Type = ZigType;
    type Decl = zig::Decl;

    // -- types --------------------------------------------------------

    /// The fixed-width Zig type each scalar local takes. `Index` is
    /// `u32` — a sequence length is a `uint32` on the wire, and a `u32`
    /// coerces into a slice index — and `Size` is `usize`, which is
    /// what the reader's positions answer in.
    fn scalar(kind: Scalar) -> Self::Type {
        ZigType::plain(match kind {
            Scalar::Bool => zig::Type::Primitive(zig::Primitive::Bool),
            Scalar::Char | Scalar::UInt8 => zig::Type::Primitive(zig::Primitive::U8),
            Scalar::Int8 => zig::Type::Primitive(zig::Primitive::I8),
            Scalar::Int16 => zig::Type::Primitive(zig::Primitive::I16),
            Scalar::UInt16 => zig::Type::Primitive(zig::Primitive::U16),
            Scalar::Int32 => zig::Type::Primitive(zig::Primitive::I32),
            Scalar::UInt32 | Scalar::Index => zig::Type::Primitive(zig::Primitive::U32),
            Scalar::Int64 => zig::Type::Primitive(zig::Primitive::I64),
            Scalar::UInt64 => zig::Type::Primitive(zig::Primitive::U64),
            Scalar::Float => zig::Type::Primitive(zig::Primitive::F32),
            Scalar::Double => zig::Type::Primitive(zig::Primitive::F64),
            // The real binary128; the adapter's table says why this is
            // the one column that has it.
            Scalar::LongDouble => zig::Type::Primitive(zig::Primitive::F128),
            Scalar::Size => zig::Type::named("usize"),
        })
    }

    fn named(name: &str) -> Self::Type {
        ZigType::plain(zig::Type::named(name))
    }

    fn nothing() -> Self::Type {
        ZigType::plain(zig::Type::Primitive(zig::Primitive::Void))
    }

    /// The runtime's handles, qualified.
    fn handle(kind: Handle) -> Self::Type {
        ZigType::plain(zig::Type::named(match kind {
            Handle::DHeader => "zdwire.DHeader",
            Handle::EmHeader => "zdwire.EmHeader",
            Handle::Pid => "zdwire.Pid",
            Handle::TryConstruct => "zdwire.TryConstruct",
            Handle::ReaderMark => "zdwire.ReaderMark",
            Handle::PidRead => "zdwire.PidRead",
            Handle::EmHeaderRead => "zdwire.EmHeaderRead",
        }))
    }

    /// The member already is the code unit — `u8` for a `char`, `u16`
    /// for a `wchar` — so there is nothing to convert.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// `@intFromEnum(v)` for an enumeration, the value itself for
    /// anything else. `adapter/zig.rs` declares an enumeration as
    /// `enum(i32)` with every enumerator's declared value written out,
    /// and Zig's `<` refuses a pair of them ("operator < not allowed
    /// for type 'main.Colour'", measured in the matrix refresh of
    /// 2026-10-02 on `map-key-enum` under the Fast DDS and OpenDDS
    /// dialects); the tag is
    /// [`zerodds_ast_layout::MapKeyOrdering::EnumeratorValue`]'s order.
    /// The union overlay asks the same seam for every integer arm, so a
    /// type that is not an enumeration passes through.
    fn enumerator_ordinal(ast: &Ast, id: Id<TypeNode>, value: Self::Expr) -> Self::Expr {
        if !Self::is_enumeration(ast, id) {
            return value;
        }
        plain(zig::Expr::call("@intFromEnum", vec![inner(value)]))
    }

    /// `zdwire.boolLess(a, b)` — `false` before `true`. An override
    /// because Zig's own `<` refuses a pair of booleans outright
    /// ("operator < not allowed for type 'bool'"), which is exactly the
    /// class of target the trait documents this seam for.
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        plain(zig::Expr::Call {
            func: Box::new(Self::runtime_name("boolLess")),
            args: vec![inner(left), inner(right)],
        })
    }

    /// The runtime's policy values: integers and strings where
    /// `runtime/zig` takes them so, and the runtime's own enumerations
    /// for the enumerated kinds. One arm per constant and no catch-all.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => plain(zig::Expr::int(2)),
            Constant::WcharWidth4 => plain(zig::Expr::int(4)),
            Constant::WcharFixed => Self::text("fixed"),
            Constant::WcharGiopLengthPrefixed => Self::text("giop-length-prefixed"),
            Constant::WstringUnitsStream => Self::text("stream"),
            Constant::WstringUnitsBigEndianAlways => Self::text("big-endian-always"),
            Constant::WstringUnitsByteOrderMark => Self::text("byte-order-mark"),
            Constant::WstringUnitsHostUnswapped => Self::text("host-unswapped"),
            Constant::WstringSpecification => Self::text("specification"),
            Constant::WstringCharacterCount => Self::text("character-count"),
            Constant::WstringCharacterCountTerminated => Self::text("character-count-terminated"),
            Constant::WstringSpecificationWideUnits => Self::text("specification-wide-units"),
            Constant::WstringRosTypeSupport => Self::text("ros-type-support"),
            Constant::WstringOctetsTerminated2 => Self::text("octets/terminated/2"),
            Constant::WstringOctetsTerminated4 => Self::text("octets/terminated/4"),
            Constant::WstringCharactersTerminated2 => Self::text("characters/terminated/2"),
            Constant::FixedDeclared => Self::text("declared"),
            Constant::FixedValuePrecision => Self::text("value-precision"),
            Constant::FixedValueStringDigits => Self::text("value-string-digits"),
            Constant::ParameterMember => Self::runtime_enum("ParameterRole", "member"),
            Constant::ParameterOptional => Self::runtime_enum("ParameterRole", "optional"),
            Constant::TryConstructDiscard => Self::runtime_enum("TryConstruct", "discard"),
            Constant::TryConstructTrim => Self::runtime_enum("TryConstruct", "trim"),
            Constant::TryConstructUseDefault => Self::runtime_enum("TryConstruct", "use_default"),
            Constant::StatusOk => Self::runtime_enum("Status", "ok"),
            Constant::StatusDiscarded => Self::runtime_enum("Status", "discarded"),
            Constant::StatusImpossibleLength => Self::runtime_enum("Status", "impossible_length"),
            Constant::StatusOutOfRange => Self::runtime_enum("Status", "out_of_range"),
            Constant::StatusTruncated => Self::runtime_enum("Status", "truncated"),
        }
    }

    /// The unsigned type wide enough for `bits` of a bit set — the same
    /// answer the materialiser's `zig_holder` gives its fields.
    fn unsigned_holder(bits: u16) -> Self::Type {
        ZigType::plain(zig::Type::Primitive(crate::adapter::zig::zig_holder(bits)))
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        // A type with no Zig form was refused by the materialiser
        // before a codec walk ran, and `void` makes the impossible fail
        // in the Zig compiler, loudly, rather than compile into a
        // plausible member.
        let tree = match zig_value_type(ast, id, "codec") {
            Ok(tree) => tree,
            Err(_) => zig::Type::Primitive(zig::Primitive::Void),
        };
        ZigType {
            tree,
            enum_like: Self::is_enumeration(ast, id),
            integer_alias: Self::is_bitmask(ast, id),
        }
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0)
    }

    /// The entry name the materialiser declared, and a distinct one
    /// per type when its tag walk refuses: the slot keeps two refused
    /// entries from collapsing onto one declaration, which would merge
    /// two map types under one name.
    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        zig_map_entry_name(ast, id).unwrap_or_else(|_| format!("ZdUnresolvedEntry{}", id.slot()))
    }

    /// Nothing: a declared type's Zig name is already flat — the
    /// adapter folds the modules in — and an enumerator lives in its
    /// enum's own scope, which [`CodecTarget::enumerator_constant`]'s
    /// override reaches.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// The escape and nothing else, exactly as the materialiser spells
    /// a field.
    fn escape(name: &str) -> String {
        field_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", field_name("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        plain(zig::Expr::Name(Ident::new(&local_name(text))))
    }

    fn int(value: i128) -> Self::Expr {
        plain(zig::Expr::int(value))
    }

    /// The raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten into the tree's own boolean where they are one.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return plain(zig::Expr::Literal(value));
        };
        match text.as_str() {
            "true" => plain(zig::Expr::Literal(Literal::Bool(true))),
            "false" => plain(zig::Expr::Literal(Literal::Bool(false))),
            _ => plain(zig::Expr::Literal(value)),
        }
    }

    /// The number, unsuffixed: a Zig integer literal is a
    /// `comptime_int` of arbitrary precision and coerces to whatever
    /// the position wants, so no width suffix exists or is needed.
    fn wide_int(value: i128) -> Self::Expr {
        plain(zig::Expr::int(value))
    }

    /// As [`CodecTarget::wide_int`]: a bare `comptime_int`.
    fn unsigned_int(value: i128) -> Self::Expr {
        plain(zig::Expr::int(value))
    }

    /// The bare number here too; the one operation a `comptime_int`
    /// refuses — the `~` a bitmask's declared-bit check complements —
    /// is given its width at the complement, in [`Zig::unary`].
    fn wide_unsigned(value: u64) -> Self::Expr {
        plain(zig::Expr::int(i128::from(value)))
    }

    fn wide_signed(value: i128) -> Self::Expr {
        plain(zig::Expr::int(value))
    }

    /// `v.x` — and `v._d` as it stands: Zig keeps the IDL-facing
    /// spelling, underscore and all.
    fn member(base: &str, field: &str) -> Self::Expr {
        plain(zig::Expr::dot(zig::Expr::Name(Ident::new(base)), field))
    }

    fn whole(base: &str) -> Self::Expr {
        plain(zig::Expr::Name(Ident::new(&local_name(base))))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        plain(zig::Expr::dot(inner(value), field))
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        plain(zig::Expr::Index {
            value: Box::new(inner(value)),
            index: Box::new(inner(at)),
        })
    }

    /// `@as(u32, @intCast(v.len))`, or the bound where the materialiser
    /// fixed one: `.len` is a `usize` and a sequence's count is a
    /// `uint32` on the wire.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => plain(zig::Expr::int(i128::from(bound))),
            None => plain(zig::Expr::as_(
                zig::Type::Primitive(zig::Primitive::U32),
                zig::Expr::call(
                    "@intCast",
                    vec![zig::Expr::dot(zig::Expr::Name(Ident::new(base)), "len")],
                ),
            )),
        }
    }

    /// The slice itself: a Zig slice is the elements.
    fn elements(base: &str) -> Self::Expr {
        plain(zig::Expr::Name(Ident::new(base)))
    }

    /// `v.x` — a union is a struct carrying its branches directly.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `entry.key` and `entry.value`, the two fields the materialiser
    /// gives a map's entry struct.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        plain(zig::Expr::dot(
            inner(entry),
            if key { "key" } else { "value" },
        ))
    }

    fn absent() -> Self::Expr {
        plain(zig::Expr::Literal(Literal::Null))
    }

    /// Nothing: a Zig slice and string always exist once the runtime
    /// answered — an allocation failure is an error the `try` already
    /// propagated — so there is no absent state for the guard to test.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// `v.x.?` — the value a `?T` holds, given that it holds one. Only
    /// ever reached inside a branch the presence test guarded, where
    /// the unwrap cannot trip.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(zig::Expr::dot(inner(Self::member(base, field)), "?"))
    }

    /// `v.x != null`.
    fn present(base: &str, field: &str) -> Self::Expr {
        plain(zig::Expr::Binary {
            op: "!=",
            lhs: Box::new(inner(Self::member(base, field))),
            rhs: Box::new(zig::Expr::Literal(Literal::Null)),
        })
    }

    /// Nothing on the way in — assigning the value is what makes the
    /// member present, the `T` to `?T` coercion being Zig's own — and
    /// `v.x = null;` on the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![zig::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: zig::Expr::Literal(Literal::Null),
        }]
    }

    /// The marked place; the decoded `T` coerces into the `?T` field on
    /// assignment, so no room has to be made first.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            ZigExpr::OptionalSlot(inner(Self::member(base, field))),
        )
    }

    /// `true`: an `@external` member is a pointer, and a member on a cycle needs
    /// the same: `?T` stores its `T` inline.
    /// [`crate::adapter::held_externally`] is the rule the adapter and
    /// the defaulter ask as well.
    fn holds_a_cycle_externally() -> bool {
        true
    }

    /// `v.x.*` — the pointer dereferenced — and `v.x.?.*` where the
    /// member is also `@optional`: the optional carries the absence and
    /// the pointer the indirection, in that order.
    fn external_value(
        _ast: &Ast,
        _id: Id<TypeNode>,
        base: &str,
        field: &str,
        optional: bool,
    ) -> Self::Expr {
        let mut place = inner(Self::member(base, field));
        if optional {
            place = zig::Expr::dot(place, "?");
        }
        plain(zig::Expr::dot(place, "*"))
    }

    /// The marked place; [`Zig::read_call`] routes the decoded value
    /// through `zdwire.zdPtr`, and the `*T` it answers coerces into a
    /// `?*T` where the member is also `@optional`.
    fn external_destination(
        base: &str,
        field: &str,
        _optional: bool,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            ZigExpr::ExternalSlot(inner(Self::member(base, field))),
        )
    }

    /// `v.x.*` for a branch too: the union arm's field is the same `*T`
    /// a structure's `@external` member is. No union member is
    /// `@optional` (DDS-XTypes 1.3 §7.2.2.4.4.4.7), so no `.?` appears.
    fn external_case_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(zig::Expr::dot(inner(Self::case_member(base, field)), "*"))
    }

    /// The marked place, exactly as [`Zig::external_destination`]
    /// answers for a structure member.
    fn external_case_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            ZigExpr::ExternalSlot(inner(Self::case_member(base, field))),
        )
    }

    /// `v.x != null`: the member is `?*T` wherever this is asked, and
    /// the optional is what carries the absence.
    fn external_present(base: &str, field: &str) -> Self::Expr {
        Self::present(base, field)
    }

    /// Nothing on the way in, `v.x = null;` on the way out — the same
    /// answer the plain optional gives, because the absence lives in
    /// the same `?`.
    fn external_set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        Self::set_present(base, field, present)
    }

    /// The operators, with the two boolean connectives translated:
    /// the walk spells `&&` and `||` and Zig spells `and` and `or`.
    /// `=` never reaches a rendered expression — [`Zig::perform`] turns
    /// it into an assignment statement.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        let spelled = match op {
            "&&" => "and",
            "||" => "or",
            other => other,
        };
        plain(zig::Expr::Binary {
            op: spelled,
            lhs: Box::new(inner(lhs)),
            rhs: Box::new(inner(rhs)),
        })
    }

    /// The operator as given, with one rewrite: `~` on a bare integer
    /// literal becomes `~@as(u64, ..)`, because a `comptime_int` has no
    /// width to complement — the one place the walk builds that is the
    /// bitmask reader's declared-bit check, whose other operand is the
    /// sixty-four-bit holder.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        let operand = inner(value);
        if op == "~" {
            if let zig::Expr::Literal(Literal::Int(_)) = &operand {
                return plain(zig::Expr::Unary {
                    op: "~",
                    value: Box::new(zig::Expr::as_(
                        zig::Type::Primitive(zig::Primitive::U64),
                        operand,
                    )),
                });
            }
        }
        plain(zig::Expr::Unary {
            op,
            value: Box::new(operand),
        })
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        plain(zig::Expr::Paren(Box::new(inner(value))))
    }

    /// The conversion, spelled by what it converts into — the module
    /// documentation carries the table. A `bool` target with an integer
    /// literal becomes the boolean literal, as in the sibling columns.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        let operand = inner(value);
        if type_.tree == zig::Type::named(SIGNED_CHAR) {
            return plain(zig::Expr::as_(
                zig::Type::Primitive(zig::Primitive::I8),
                zig::Expr::call("@bitCast", vec![operand]),
            ));
        }
        if type_.tree == zig::Type::Primitive(zig::Primitive::Bool) {
            if let zig::Expr::Literal(Literal::Int(text)) = &operand {
                return plain(zig::Expr::Literal(Literal::Bool(text != "0")));
            }
        }
        if type_.enum_like {
            return plain(zig::Expr::as_(
                type_.tree,
                zig::Expr::call("@enumFromInt", vec![operand]),
            ));
        }
        if type_.tree == zig::Type::Primitive(zig::Primitive::I64) {
            return plain(zig::Expr::Call {
                func: Box::new(Self::runtime_name("asI64")),
                args: vec![operand],
            });
        }
        if type_.tree == zig::Type::Primitive(zig::Primitive::U64) {
            return plain(zig::Expr::Call {
                func: Box::new(Self::runtime_name("asU64")),
                args: vec![operand],
            });
        }
        if Self::is_integer_type(&type_.tree) || type_.integer_alias {
            // A literal coerces on its own; a runtime value states the
            // narrowing out loud.
            if let zig::Expr::Literal(Literal::Int(_)) = &operand {
                return plain(zig::Expr::as_(type_.tree, operand));
            }
            return plain(zig::Expr::as_(
                type_.tree,
                zig::Expr::call("@intCast", vec![operand]),
            ));
        }
        plain(zig::Expr::as_(type_.tree, operand))
    }

    /// The value, unchanged: Zig hands a slice by header, an array and
    /// a struct by value.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// A call to a generated codec — fallible, so the seam owns the
    /// `try`. `strcmp` is the runtime's, which spells C11 §7.24.4's
    /// unsigned-octet order over two UTF-8 slices, and answers plainly.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let arguments: Vec<zig::Expr> = args.into_iter().map(inner).collect();
        if callee == "strcmp" {
            return plain(zig::Expr::Call {
                func: Box::new(Self::runtime_name("strcmp")),
                args: arguments,
            });
        }
        ZigExpr::Fallible(zig::Expr::call(callee, arguments))
    }

    /// `w.writeU32(x)` — a method on the writer or the reader, marked
    /// fallible unless the contract's table exempts it.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let call = zig::Expr::Call {
            func: Box::new(zig::Expr::dot(
                zig::Expr::Name(Ident::new(receiver)),
                &Self::camel(method),
            )),
            args: args.into_iter().map(inner).collect(),
        };
        if FALLIBLE_EXEMPT_METHODS.contains(&method) {
            return plain(call);
        }
        ZigExpr::Fallible(call)
    }

    /// A free function of the runtime, qualified — except
    /// `pid_is_user_member`, which is a method on the header the reader
    /// handed back.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "pid_is_user_member" {
            let mut arguments = args.into_iter().map(inner);
            if let Some(header) = arguments.next() {
                return plain(zig::Expr::Call {
                    func: Box::new(zig::Expr::dot(header, "isUserMember")),
                    args: Vec::new(),
                });
            }
            // The walk always hands the header; a call without one is a
            // walk that changed under this file, and a function no
            // scope declares is the loudest thing Zig can say here.
            return plain(zig::Expr::Call {
                func: Box::new(Self::runtime_name("isUserMember")),
                args: Vec::new(),
            });
        }
        let call = zig::Expr::Call {
            func: Box::new(Self::runtime_name(&Self::camel(method))),
            args: args.into_iter().map(inner).collect(),
        };
        if FALLIBLE_STATICS.contains(&method) {
            return ZigExpr::Fallible(call);
        }
        plain(call)
    }

    /// `runtime/zig` carries the three HDDS axes: `beginPackedElement` /
    /// `finishPackedElement` on the writer and `readerBeginPackedElement`
    /// / `readerFinishPackedElement` on the reader, the DHEADER pair for
    /// a sequence element's own length, and `writeWstringUtf8Octets` /
    /// `writeWstringUtf8OctetsBounded` / `readWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    // -- statements ----------------------------------------------------

    /// `var name: T = value;` — the type written, so a reader of the
    /// generated file sees the width. Everything starts as `var`;
    /// [`Zig::routine`]'s assembly pass turns what the body never
    /// assigns into `const`, because Zig refuses each where the other
    /// is meant.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        zig::Stmt::Var {
            name: Ident::new(&local_name(name)),
            type_: Some(type_.tree),
            value: value.map(inner_bare),
            mutable: true,
        }
    }

    /// An expression evaluated for its effect — with the `try` a
    /// fallible one carries, and an assignment the walk built as its
    /// own statement.
    fn perform(value: Self::Expr) -> Self::Stmt {
        match value {
            ZigExpr::Plain(zig::Expr::Binary { op: "=", lhs, rhs }) => zig::Stmt::Assign {
                place: *lhs,
                value: *rhs,
            },
            ZigExpr::Fallible(e) => zig::Stmt::Expr(zig::Expr::try_(e)),
            other => zig::Stmt::Expr(inner(other)),
        }
    }

    /// `return v;`: a Zig decoder answers with the sample it built, and
    /// the error union wraps it on its own.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        zig::Stmt::Return(Some(match value {
            Some(value) => inner(value),
            None => zig::Expr::Name(Ident::new("v")),
        }))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        zig::Stmt::If {
            condition: inner(condition),
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// The counter block the renderer scopes: every bound the walk
    /// hands over is a `u32` count off the wire or a literal.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        zig::Stmt::CountUp {
            counter: Ident::new(&local_name(counter)),
            type_: Box::new(zig::Type::Primitive(zig::Primitive::U32)),
            bound: Box::new(inner(bound)),
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        zig::Stmt::While {
            condition: inner(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        zig::Stmt::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        zig::Stmt::Comment(doc)
    }

    /// `_ = try …;` — Zig's spelling of a value read for its effect.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        zig::Stmt::DiscardExpr(inner_bare(value))
    }

    /// `_ = name;` — kept only where [`Zig::routine`]'s assembly pass
    /// finds no other mention of the name: Zig refuses both an unused
    /// parameter and a pointless discard, and which of the two a body
    /// earns is the whole body's question.
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![zig::Stmt::Discard(Ident::new(name))]
    }

    /// `place = try zd_dec_T(r, behaviour);` — and through
    /// `zdwire.zdPtr` where the place is an `@external` member's field:
    /// the decoder answers a `T` and the field holds a pointer. An
    /// `@optional` member's `?T` takes the bare `T` by coercion.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let decoded = zig::Expr::call(
            callee,
            vec![zig::Expr::Name(Ident::new("r")), inner(behaviour)],
        );
        match place {
            ZigExpr::ExternalSlot(field) => zig::Stmt::Assign {
                place: field,
                value: zig::Expr::try_(zig::Expr::Call {
                    func: Box::new(Self::runtime_name("zdPtr")),
                    args: vec![Self::reader_heap(), zig::Expr::try_(decoded)],
                }),
            },
            ZigExpr::OptionalSlot(field) | ZigExpr::Plain(field) | ZigExpr::Fallible(field) => {
                zig::Stmt::Assign {
                    place: field,
                    value: zig::Expr::try_(decoded),
                }
            }
        }
    }

    /// `v = try zd_dec_T(r, zd_tc);` — a `typedef`, handed straight on.
    ///
    /// The trait's default is the C shape, `zd_dec_T(r, v, zd_tc)` with
    /// the value as an out-parameter, and a Zig decoder takes two
    /// arguments and answers the value: `error: expected 2 argument(s),
    /// found 3` on `resolve-typedef-chain-three` and
    /// `sequence-of-typedef`. Every column whose decoders answer a value
    /// rather than fill an out-parameter — C#, D, Go, Java, Python,
    /// TypeScript — overrides it the same way; C and C++ keep the
    /// default.
    fn read_forward(callee: &str) -> Self::Stmt {
        Self::read_call(
            callee,
            Self::whole("v"),
            Handoff::Value,
            Self::name("zd_tc"),
        )
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// `v = try r.heap.alloc(T, n);`, and the fill after it: the slots
    /// are storage the moment the allocator answers, and the fill
    /// assigns every one of them before anything reads it.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![zig::Stmt::Assign {
            place: zig::Expr::Name(Ident::new(base)),
            value: zig::Expr::try_(zig::Expr::Call {
                func: Box::new(zig::Expr::dot(Self::reader_heap(), "alloc")),
                args: vec![zig::Expr::type_(element.tree), inner(count)],
            }),
        }];
        out.extend(fill);
        out
    }

    /// `v = v[0..count];` — §7.3.1.2.1.12.3's TRIM, applied after every
    /// element has been read: a Zig slice shortens by re-slicing.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![zig::Stmt::Assign {
            place: zig::Expr::Name(Ident::new(base)),
            value: zig::Expr::Index {
                value: Box::new(zig::Expr::Name(Ident::new(base))),
                index: Box::new(zig::Expr::Binary {
                    op: "..",
                    lhs: Box::new(zig::Expr::int(0)),
                    rhs: Box::new(inner(count)),
                }),
            },
        }]
    }

    /// The subscript, with nothing around it: a Zig slice element is a
    /// real place at every element type.
    fn sequence_element(
        base: &str,
        index: Self::Expr,
        _element: Self::Type,
    ) -> (Vec<Self::Stmt>, Self::Expr, Vec<Self::Stmt>) {
        (
            Vec::new(),
            Self::index(Self::elements(base), index),
            Vec::new(),
        )
    }

    // -- assembly ------------------------------------------------------

    /// `fn zd_enc_T(w: *zdwire.Writer, v: T) zdwire.WireError!void` and
    /// `fn zd_dec_T(r: *zdwire.Reader, zd_tc: zdwire.TryConstruct)
    /// zdwire.WireError!T`.
    ///
    /// A forward declaration is answered with nothing: a Zig
    /// container-level name is in scope for the whole file however far
    /// down it is written, so a recursive type needs no declaration
    /// ahead of its body.
    ///
    /// This is also where Zig's unused-and-mutability rules are
    /// settled, because both are properties of the whole body: every
    /// local the body never assigns becomes `const`, a parameter the
    /// body never mentions gets a `_ = name;`, and a discard the walk
    /// asked for is kept only when the name has no other mention —
    /// Zig refuses a pointless discard as loudly as an unused
    /// parameter.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![zig::FnParam {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            type_: zig::Type::pointer(zig::Type::named(if spec.writing {
                "zdwire.Writer"
            } else {
                "zdwire.Reader"
            })),
        }];
        if spec.writing {
            params.push(zig::FnParam {
                name: Ident::new("v"),
                type_: spec.sample.tree.clone(),
            });
        }
        for (type_, name) in spec.extra {
            params.push(zig::FnParam {
                name: Ident::new(&name),
                type_: type_.tree,
            });
        }
        let error = zig::Type::named("zdwire.WireError");
        let (returns, mut body) = if spec.writing {
            (
                zig::Type::error_union(error, zig::Type::Primitive(zig::Primitive::Void)),
                body,
            )
        } else {
            // The sample starts as Table 9's value, through the
            // generated defaulter, so every position the stream does
            // not carry is already what the clause says it is.
            let defaulter = crate::default_zig::defaulter_from_decoder(&spec.name);
            let mut statements = vec![zig::Stmt::Var {
                name: Ident::new("v"),
                type_: Some(spec.sample.tree.clone()),
                value: Some(zig::Expr::try_(zig::Expr::call(
                    &defaulter,
                    vec![Self::reader_heap()],
                ))),
                mutable: true,
            }];
            statements.extend(body);
            statements.push(zig::Stmt::Return(Some(zig::Expr::Name(Ident::new("v")))));
            (zig::Type::error_union(error, spec.sample.tree), statements)
        };
        let assigned = assigned_roots(&body);
        fix_mutability(&mut body, &assigned);
        settle_discards(&mut body, &params);
        Some(zig::Decl::Func(zig::FuncDecl {
            doc: spec.doc,
            // `pub`, because the emission is a module and a codec that
            // cannot be called from the file that imports it is not one:
            // a Zig declaration without it is private to its file (Zig
            // language reference, "Keyword pub"), so
            // `@import("gen.zig").zd_enc_Max` was an error. Measured:
            // `zig_endpoint_sync`/`_async` in `crates/endpoint-e2e`
            // build the generated module beside an app that calls
            // `gen.zd_enc_Ping`, and Zig answers `'zd_enc_Ping' is not
            // marked 'pub'`. Uniform over every routine rather than only
            // the top-level ones: `pub` at container level says nothing
            // about the file itself, and a helper the walk hoisted is
            // reachable from the same import.
            is_pub: true,
            name: Ident::new(&spec.name),
            params,
            returns,
            body,
        }))
    }

    /// A call to the generated defaulter, which is Table 9 once per
    /// type rather than once per position. Reached only inside a
    /// decoder, where `r.heap` is the arena a defaulted `@external`
    /// pointer allocates from.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        let defaulted = zig::Expr::call(
            &crate::default_zig::defaulter_name(ast, id),
            vec![Self::reader_heap()],
        );
        // The slot markers route exactly as [`Zig::read_call`]'s do.
        Ok(match place {
            ZigExpr::ExternalSlot(field) => zig::Stmt::Assign {
                place: field,
                value: zig::Expr::try_(zig::Expr::Call {
                    func: Box::new(Self::runtime_name("zdPtr")),
                    args: vec![Self::reader_heap(), zig::Expr::try_(defaulted)],
                }),
            },
            ZigExpr::OptionalSlot(field) | ZigExpr::Plain(field) | ZigExpr::Fallible(field) => {
                zig::Stmt::Assign {
                    place: field,
                    value: zig::Expr::try_(defaulted),
                }
            }
        })
    }

    /// # Errors
    /// As [`crate::default_zig::member_default`]: a `@default` this
    /// column cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_zig::member_default(ast, node, type_, inner(place), Self::reader_heap())
    }

    /// `Mode.IDLE` — the enum's own scope, which is where the
    /// materialiser declared the variant. The trait's default answers a
    /// bare prefixed name, which in Zig names nothing.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        plain(zig::Expr::dot(
            zig::Expr::Name(Ident::new(&zig_type_name(ast, id))),
            &field_name(enumerator),
        ))
    }
}

// -- the assembly pass ---------------------------------------------------

/// Every base name the body assigns through — `v` for `v.x = …` and
/// `v[i] = …` alike — which is what decides `var` against `const`.
fn assigned_roots(body: &[zig::Stmt]) -> Vec<String> {
    let mut out = Vec::new();
    collect_assigned(body, &mut out);
    out
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn collect_assigned(body: &[zig::Stmt], out: &mut Vec<String>) {
    for statement in body {
        match statement {
            zig::Stmt::Assign { place, .. } => {
                if let Some(root) = root_name(place) {
                    if !out.contains(&root) {
                        out.push(root);
                    }
                }
            }
            zig::Stmt::If {
                then, otherwise, ..
            } => {
                collect_assigned(then, out);
                if let Some(otherwise) = otherwise {
                    collect_assigned(otherwise, out);
                }
            }
            zig::Stmt::CountUp { body, .. } | zig::Stmt::While { body, .. } => {
                collect_assigned(body, out);
            }
            _ => {}
        }
    }
}

/// The base a place is reached from: `v` for `v.x[i].*`.
///
/// zerodds-lint: recursion-depth 64 (place-expression nesting the walk
/// builds). The tree walked here is the one this generator emitted and
/// owns, so it holds no cycle; the depth is that expression's height.
fn root_name(place: &zig::Expr) -> Option<String> {
    match place {
        zig::Expr::Name(name) => Some(name.as_str().to_string()),
        zig::Expr::Dot { value, .. }
        | zig::Expr::Index { value, .. }
        | zig::Expr::Paren(value)
        | zig::Expr::Try(value) => root_name(value),
        _ => None,
    }
}

/// Turn every `var` the body never assigns into a `const`: Zig refuses
/// a `var` that is never mutated by name.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn fix_mutability(body: &mut [zig::Stmt], assigned: &[String]) {
    for statement in body {
        match statement {
            zig::Stmt::Var {
                name,
                value,
                mutable,
                ..
            } => {
                // A local declared `undefined` is assigned later by
                // definition; the scan sees those assignments too, but
                // keeping the rule explicit costs nothing.
                let is_assigned = assigned.iter().any(|root| root == name.as_str());
                *mutable = is_assigned || value.is_none();
            }
            zig::Stmt::If {
                then, otherwise, ..
            } => {
                fix_mutability(then, assigned);
                if let Some(otherwise) = otherwise {
                    fix_mutability(otherwise, assigned);
                }
            }
            zig::Stmt::CountUp { body, .. } | zig::Stmt::While { body, .. } => {
                fix_mutability(body, assigned);
            }
            _ => {}
        }
    }
}

/// Keep exactly the discards a body needs: drop `_ = x;` where `x` has
/// any other mention — Zig calls that a pointless discard — and add one
/// for every parameter the body never mentions, which Zig refuses as an
/// unused parameter.
fn settle_discards(body: &mut Vec<zig::Stmt>, params: &[zig::FnParam]) {
    let mut mentioned: Vec<String> = Vec::new();
    collect_mentions(body, &mut mentioned);
    prune_discards(body, &mentioned);
    let mut fronts = Vec::new();
    for param in params {
        let name = param.name.as_str();
        if !mentioned.iter().any(|m| m == name) {
            fronts.push(zig::Stmt::Discard(Ident::new(name)));
        }
    }
    if !fronts.is_empty() {
        fronts.append(body);
        *body = fronts;
    }
}

/// Every name the body mentions outside a discard — in an expression,
/// as an assignment target, as a loop bound.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn collect_mentions(body: &[zig::Stmt], out: &mut Vec<String>) {
    for statement in body {
        match statement {
            zig::Stmt::Var { value, .. } => {
                if let Some(value) = value {
                    mentions_in_expr(value, out);
                }
            }
            zig::Stmt::Assign { place, value } => {
                mentions_in_expr(place, out);
                mentions_in_expr(value, out);
            }
            zig::Stmt::Expr(value)
            | zig::Stmt::DiscardExpr(value)
            | zig::Stmt::Return(Some(value)) => {
                mentions_in_expr(value, out);
            }
            zig::Stmt::If {
                condition,
                then,
                otherwise,
            } => {
                mentions_in_expr(condition, out);
                collect_mentions(then, out);
                if let Some(otherwise) = otherwise {
                    collect_mentions(otherwise, out);
                }
            }
            zig::Stmt::CountUp { bound, body, .. } => {
                mentions_in_expr(bound, out);
                collect_mentions(body, out);
            }
            zig::Stmt::While { condition, body } => {
                mentions_in_expr(condition, out);
                collect_mentions(body, out);
            }
            zig::Stmt::Discard(_)
            | zig::Stmt::Return(None)
            | zig::Stmt::Break
            | zig::Stmt::Comment(_) => {}
        }
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn mentions_in_expr(expr: &zig::Expr, out: &mut Vec<String>) {
    match expr {
        zig::Expr::Name(name) => {
            let text = name.as_str().to_string();
            if !out.contains(&text) {
                out.push(text);
            }
        }
        zig::Expr::Binary { lhs, rhs, .. } => {
            mentions_in_expr(lhs, out);
            mentions_in_expr(rhs, out);
        }
        zig::Expr::Paren(value)
        | zig::Expr::Unary { value, .. }
        | zig::Expr::Try(value)
        | zig::Expr::Dot { value, .. } => mentions_in_expr(value, out),
        zig::Expr::Index { value, index } => {
            mentions_in_expr(value, out);
            mentions_in_expr(index, out);
        }
        zig::Expr::Call { func, args } => {
            mentions_in_expr(func, out);
            for arg in args {
                mentions_in_expr(arg, out);
            }
        }
        zig::Expr::StructLit(fields) => {
            for (_, value) in fields {
                mentions_in_expr(value, out);
            }
        }
        zig::Expr::ListLit(elements) => {
            for element in elements {
                mentions_in_expr(element, out);
            }
        }
        zig::Expr::Literal(_) | zig::Expr::Type(_) => {}
    }
}

/// Drop every `_ = x;` whose `x` the body mentions anywhere else.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn prune_discards(body: &mut Vec<zig::Stmt>, mentioned: &[String]) {
    body.retain(|statement| match statement {
        zig::Stmt::Discard(name) => !mentioned.iter().any(|m| m == name.as_str()),
        _ => true,
    });
    for statement in body {
        match statement {
            zig::Stmt::If {
                then, otherwise, ..
            } => {
                prune_discards(then, mentioned);
                if let Some(otherwise) = otherwise {
                    prune_discards(otherwise, mentioned);
                }
            }
            zig::Stmt::CountUp { body, .. } | zig::Stmt::While { body, .. } => {
                prune_discards(body, mentioned);
            }
            _ => {}
        }
    }
}

/// The bound a collection carries, as the suffix that keeps two of them
/// apart.
///
/// The tag names the routine a hoisted collection's codec is emitted
/// under, and `decoder_over` emits one body per name. Without the
/// bound, `sequence<long>` and `sequence<long, 4>` are one name and the
/// first body wins: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated Zig held no `zd_over` check at all, so `a` accepted a
/// stream of any length; with the members the other way round both
/// members got the bounded body, so `b` refused — and truncated with
/// `v = v[0 .. 4]` — a length the IDL puts no limit on. `string` beside
/// `string<4>` behaved the same way through `trimUtf8`. Either way the
/// reader answered with a value the IDL does not describe and said
/// nothing, which is why the bound is part of the name.
fn bound_tag(ast: &Ast, bound: &zerodds_ast::Bound) -> String {
    match bound.expr() {
        None => String::new(),
        // `_b` and not a bare `_n`: `seq_long_4` would be an array of
        // four `long` under the arm above, and the two are different
        // wire forms.
        Some(expr) => match ast.eval_integral(expr) {
            Ok(limit) => format!("_b{limit}"),
            // As the array's `_x`: a bound that does not evaluate is
            // still not the unbounded form, and the materialiser
            // reports the expression that would not evaluate.
            Err(_) => "_bx".to_string(),
        },
    }
}

/// The name a codec function takes for the type it codes: the IDL kind
/// and not the Zig spelling, structurally for a collection and the
/// flattened Zig name for a declaration — the same scheme the D and Go
/// columns carry, for the reasons their `type_tag`s state.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
#[must_use]
pub fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_TYPE_DEPTH {
        return "unresolved".to_string();
    }
    let node = match ast.type_(id) {
        Ok(node) => node,
        Err(_) => return "unresolved".to_string(),
    };
    match &node.kind {
        TypeKind::Primitive(kind) => kind.as_str().replace(' ', "_"),
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => format!("string{}", bound_tag(ast, bound)),
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => format!("wstring{}", bound_tag(ast, bound)),
        TypeKind::Fixed { digits, scale } => format!("fixed_{digits}_{scale}"),
        TypeKind::Sequence { element, bound } => match element.resolved() {
            Some(element) => format!(
                "seq{}_{}",
                bound_tag(ast, bound),
                type_tag(ast, element, depth + 1)
            ),
            None => format!("seq_unresolved{}", bound_tag(ast, bound)),
        },
        TypeKind::Array { element, dims } => {
            let mut name = match element.resolved() {
                Some(element) => format!("arr_{}", type_tag(ast, element, depth + 1)),
                None => "arr_unresolved".to_string(),
            };
            for dim in dims {
                match ast.eval_integral(*dim) {
                    Ok(length) => name.push_str(&format!("_{length}")),
                    // No number rather than a zero: the name has to
                    // differ from a `[0]` array's, and the materialiser
                    // reports the dimension that would not evaluate.
                    Err(_) => name.push_str("_x"),
                }
            }
            name
        }
        TypeKind::Map { key, value, bound } => {
            let half = |half: Option<Id<TypeNode>>| match half {
                Some(target) => type_tag(ast, target, depth + 1),
                None => "unresolved".to_string(),
            };
            format!(
                "map{}_{}_{}",
                bound_tag(ast, bound),
                half(key.resolved()),
                half(value.resolved())
            )
        }
        // An alias is followed through: two IDL names for one shape are
        // one wire form, and two codecs for it would be two chances to
        // differ.
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => type_tag(ast, target, depth + 1),
            None => "unresolved".to_string(),
        },
        TypeKind::ValueBox { boxed } => match boxed.resolved() {
            Some(target) => format!("box_{}", type_tag(ast, target, depth + 1)),
            None => "box_unresolved".to_string(),
        },
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => zig_type_name(ast, id),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}
