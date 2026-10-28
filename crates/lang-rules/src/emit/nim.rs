// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Nim backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as the columns before
//! it, and none of them calls another. Where two columns disagree on an
//! octet, one has misread the specification — which is the only reason
//! to have another.
//!
//! # The five decisions this column turns on
//!
//! **A member holds its wire width, under Nim's fixed-width names.** An
//! IDL `long` is a Nim `int32` and a `char` a `uint8`, so
//! [`CodecTarget::code_point`] is the identity and a union's
//! discriminator compares as a number. Nim's own `int` appears only as
//! a codec local — a stream position, which is what `len` answers in —
//! and never as a member.
//!
//! **`@optional` is `Option[T]`, `@external` is `ref T`, and the union
//! marks its selected branch with an `Option` of its own.** The
//! materialiser (`adapter/nim.rs`) declares all three, and this backend
//! carries a marked-expression form per shape so the read direction can
//! route a decoded `T` through `some(..)`, `zdwire.zdRef(..)`, or both
//! — Nim has no address-of on a call's result any more than Go does,
//! and an `Option`'s payload is not a place.
//!
//! **A `long double` and a `fixed` are decimal text.** Nim has
//! `float32`, `float64` and no binary128; `runtime/nim` parses the
//! member's decimal correctly rounded, once, and prints the exact
//! expansion.
//!
//! **A conversion is not a cast.** Nim's `T(v)` range-checks at run
//! time, so the two places the walk needs the *bits* — a map key's
//! signed reading of a `uint8` and the union overlay's 64-bit pattern —
//! go through `cast[int8]` and `zdwire.zdI64`, and everything else
//! takes the checked conversion.
//!
//! **Forward declarations are real here.** Nim resolves a module-scope
//! name only after its declaration, so [`CodecTarget::routine`] answers
//! the bodiless form where D and Go answer nothing — the same shape the
//! C column needs, for the same reason.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::nim::{
    field_name, module_prefix, nim_bare_name, nim_enumerator_constant, nim_map_entry_name,
    nim_value_type,
};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::nim;

/// The Nim backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nim;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The module the generated file imports the runtime under.
const RUNTIME: &str = "zdwire";

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`; the Nim member is a `uint8` and the signed reading is a
/// `cast[int8]` — the conversion `int8(..)` would range-check the very
/// values the two's-complement reading exists for.
const SIGNED_CHAR: &str = "signed char";

/// The name a type with no Nim form falls back to.
///
/// Deliberately a name no scope declares: the materialiser refused the
/// type before a codec walk ran, and this makes the impossible fail in
/// the Nim compiler, loudly, rather than compile into a plausible
/// member.
const NO_FORM: &str = "ZdNoNimForm";

/// An expression, with the one property the walk's seams cannot carry:
/// which wrapper the place it names lives behind.
///
/// The `*Slot` forms are destinations: only the destination seams
/// produce them and only [`Nim::read_call`], [`Nim::store`] and
/// [`Nim::default_into`] act on them. [`NimExpr::CaseSlot`] is also a
/// value — the write direction reaches a union branch through the same
/// accessor — and reads as `.get` there.
#[derive(Debug, Clone)]
pub enum NimExpr {
    /// An ordinary expression.
    Plain(nim::Expr),
    /// The `Option[T]` field of an `@optional` member.
    OptionalSlot(nim::Expr),
    /// The `ref T` field of an `@external` member.
    ExternalSlot(nim::Expr),
    /// The `Option[ref T]` field of an `@optional @external` member.
    OptionalExternalSlot(nim::Expr),
    /// The `Option[T]` field of a union branch.
    CaseSlot(nim::Expr),
    /// The `Option[ref T]` field of an `@external` union branch.
    CaseExternalSlot(nim::Expr),
}

/// The expression as a *value*: the marked forms unwrap.
fn value_of(expr: NimExpr) -> nim::Expr {
    match expr {
        NimExpr::Plain(e) => e,
        // Only ever reached inside a branch the presence test guarded.
        NimExpr::OptionalSlot(e) | NimExpr::CaseSlot(e) => nim::Expr::dot(e, "get"),
        NimExpr::ExternalSlot(e) => nim::Expr::Deref(Box::new(e)),
        NimExpr::OptionalExternalSlot(e) | NimExpr::CaseExternalSlot(e) => {
            nim::Expr::Deref(Box::new(nim::Expr::dot(e, "get")))
        }
    }
}

fn plain(e: nim::Expr) -> NimExpr {
    NimExpr::Plain(e)
}

impl Nim {
    /// A runtime name, qualified: `zdwire.x`.
    fn runtime_name(name: &str) -> nim::Expr {
        nim::Expr::dot(nim::Expr::name(RUNTIME), name)
    }

    /// A runtime operation's Nim spelling: `write_u32` is `writeU32` —
    /// the first segment as it stands, every later one with its first
    /// letter raised. Nim compares the rest of an identifier without
    /// case anyway; the camel form is the one `runtime/nim` publishes.
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

    fn text(value: &str) -> NimExpr {
        plain(nim::Expr::Literal(Literal::Str(value.to_string())))
    }

    fn runtime_constant(name: &str) -> NimExpr {
        plain(Self::runtime_name(name))
    }

    /// `some(value)`, `zdwire.zdRef(value)`, or the two nested — how a
    /// decoded or defaulted value enters the field a marked place
    /// names.
    fn into_slot(place: NimExpr, value: nim::Expr) -> nim::Stmt {
        let wrap_some = |inner: nim::Expr| nim::Expr::call(nim::Expr::name("some"), vec![inner]);
        let wrap_ref = |inner: nim::Expr| nim::Expr::call(Self::runtime_name("zdRef"), vec![inner]);
        match place {
            NimExpr::Plain(field) => nim::Stmt::Assign {
                place: field,
                value,
            },
            NimExpr::OptionalSlot(field) | NimExpr::CaseSlot(field) => nim::Stmt::Assign {
                place: field,
                value: wrap_some(value),
            },
            NimExpr::ExternalSlot(field) => nim::Stmt::Assign {
                place: field,
                value: wrap_ref(value),
            },
            NimExpr::OptionalExternalSlot(field) | NimExpr::CaseExternalSlot(field) => {
                nim::Stmt::Assign {
                    place: field,
                    value: wrap_some(wrap_ref(value)),
                }
            }
        }
    }

    /// The Nim spelling of an operator the walk hands over in C's.
    ///
    /// One table, so the logical and the bitwise forms cannot drift:
    /// Nim spells both `and`/`or`, overloaded by type.
    fn op(op: &'static str) -> &'static str {
        match op {
            "&&" | "&" => "and",
            "||" | "|" => "or",
            "^" => "xor",
            "<<" => "shl",
            ">>" => "shr",
            _ => op,
        }
    }
}

impl CodecTarget for Nim {
    type Expr = NimExpr;
    type Stmt = nim::Stmt;
    type Block = Vec<nim::Stmt>;
    type Type = nim::Type;
    type Decl = nim::ProcDecl;

    // -- types --------------------------------------------------------

    /// The fixed-width Nim type each scalar local takes. `Index` is
    /// `uint32` — a sequence length is a `uint32` on the wire — and
    /// `Size` is Nim's `int`, which is what `len` and the reader's
    /// positions answer in. `int` is not in the CST's own `Primitive`
    /// set — a declared *member* must not change width between targets
    /// — and a codec local is not a member.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => nim::Type::Primitive(nim::Primitive::Bool),
            Scalar::Char | Scalar::UInt8 => nim::Type::Primitive(nim::Primitive::UInt8),
            Scalar::Int8 => nim::Type::Primitive(nim::Primitive::Int8),
            Scalar::Int16 => nim::Type::Primitive(nim::Primitive::Int16),
            Scalar::UInt16 => nim::Type::Primitive(nim::Primitive::UInt16),
            Scalar::Int32 => nim::Type::Primitive(nim::Primitive::Int32),
            Scalar::UInt32 | Scalar::Index => nim::Type::Primitive(nim::Primitive::UInt32),
            Scalar::Int64 => nim::Type::Primitive(nim::Primitive::Int64),
            Scalar::UInt64 => nim::Type::Primitive(nim::Primitive::UInt64),
            Scalar::Float => nim::Type::Primitive(nim::Primitive::Float32),
            Scalar::Double => nim::Type::Primitive(nim::Primitive::Float64),
            // The decimal text; the module documentation says why.
            Scalar::LongDouble => nim::Type::Primitive(nim::Primitive::String),
            Scalar::Size => nim::Type::named("int"),
        }
    }

    fn named(name: &str) -> Self::Type {
        nim::Type::named(name)
    }

    /// A name no scope declares: never rendered — [`Nim::routine`]
    /// writes no result for a writer, and the two defaults that cast to
    /// it are overridden — so reaching it is a walk that changed under
    /// this file, and the Nim compiler names it.
    fn nothing() -> Self::Type {
        nim::Type::named("void")
    }

    /// The runtime's handles, qualified.
    fn handle(kind: Handle) -> Self::Type {
        nim::Type::named(match kind {
            Handle::DHeader => "zdwire.DHeader",
            Handle::EmHeader => "zdwire.EmHeader",
            Handle::Pid => "zdwire.Pid",
            Handle::TryConstruct => "zdwire.TryConstruct",
            Handle::ReaderMark => "zdwire.ReaderMark",
            Handle::PidRead => "zdwire.PidRead",
            Handle::EmHeaderRead => "zdwire.EmHeaderRead",
        })
    }

    /// The member already is the code unit — `uint8` for a `char`,
    /// `uint16` for a `wchar` — so there is nothing to convert.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy values: strings and integers where
    /// `runtime/nim` takes them so, and qualified constants for the
    /// enumerated kinds. One arm per constant and no catch-all.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => plain(nim::Expr::int(2)),
            Constant::WcharWidth4 => plain(nim::Expr::int(4)),
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
            Constant::ParameterMember => Self::runtime_constant("ParameterMember"),
            Constant::ParameterOptional => Self::runtime_constant("ParameterOptional"),
            Constant::TryConstructDiscard => Self::runtime_constant("TryConstructDiscard"),
            Constant::TryConstructTrim => Self::runtime_constant("TryConstructTrim"),
            Constant::TryConstructUseDefault => Self::runtime_constant("TryConstructUseDefault"),
            Constant::StatusOk => Self::runtime_constant("StatusOk"),
            Constant::StatusDiscarded => Self::runtime_constant("StatusDiscarded"),
            Constant::StatusImpossibleLength => Self::runtime_constant("StatusImpossibleLength"),
            Constant::StatusOutOfRange => Self::runtime_constant("StatusOutOfRange"),
            Constant::StatusTruncated => Self::runtime_constant("StatusTruncated"),
        }
    }

    /// The unsigned type wide enough for `bits` of a bit set — the same
    /// answer the materialiser's `nim_holder` gives its fields.
    fn unsigned_holder(bits: u16) -> Self::Type {
        nim::Type::Primitive(crate::adapter::nim::nim_holder(bits))
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        // The fallback carries its reason where the silent-drop guard
        // reads it; see [`NO_FORM`].
        #[allow(clippy::manual_unwrap_or_default, clippy::manual_unwrap_or)]
        match nim_value_type(ast, id) {
            Ok(type_) => type_,
            Err(_) => nim::Type::named(NO_FORM),
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
        nim_map_entry_name(ast, id).unwrap_or_else(|_| format!("ZdUnresolvedEntry{}", id.slot()))
    }

    /// The flattened module path with its trailing separator, taken
    /// from the same function the materialiser puts on an enumerator's
    /// field — the one consumer of this in the walk is the enumerator
    /// default this backend overrides anyway.
    fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
        module_prefix(ast, id)
    }

    /// The repair and the strop, exactly as the materialiser spells a
    /// field.
    fn escape(name: &str) -> String {
        field_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", field_name("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        plain(nim::Expr::name(text))
    }

    fn int(value: i128) -> Self::Expr {
        plain(nim::Expr::int(value))
    }

    /// The raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten into the tree's own boolean where they are one.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return plain(nim::Expr::Literal(value));
        };
        match text.as_str() {
            "true" => plain(nim::Expr::Literal(Literal::Bool(true))),
            "false" => plain(nim::Expr::Literal(Literal::Bool(false))),
            _ => plain(nim::Expr::Literal(value)),
        }
    }

    /// A literal wide enough to survive its own width: Nim refuses a
    /// bare decimal past `int64`, so a union label above it carries
    /// `'u64` and one past `int32` carries `'i64` — inside `int32` a
    /// bare literal converts against whatever type it meets.
    fn wide_int(value: i128) -> Self::Expr {
        if value > i128::from(i64::MAX) {
            return plain(nim::Expr::Literal(Literal::Int(format!("{value}'u64"))));
        }
        if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
            return plain(nim::Expr::Literal(Literal::Int(format!("{value}'i64"))));
        }
        plain(nim::Expr::int(value))
    }

    /// The number, unsuffixed: `5u` is not a Nim token, and a bare
    /// literal converts against a `uint32` counter or an `int`
    /// position. A bound past `int32` gets its width from
    /// [`CodecTarget::wide_int`].
    fn unsigned_int(value: i128) -> Self::Expr {
        Self::wide_int(value)
    }

    /// `..'u64`, typed: a mask the walk complements has to already be a
    /// `uint64` — `not` on a bare literal would be an `int`'s
    /// complement, a different pattern.
    fn wide_unsigned(value: u64) -> Self::Expr {
        plain(nim::Expr::Literal(Literal::Int(format!("{value}'u64"))))
    }

    /// `..'i64`, for the enumerator patterns `zd_raw` is compared
    /// against.
    fn wide_signed(value: i128) -> Self::Expr {
        plain(nim::Expr::Literal(Literal::Int(format!("{value}'i64"))))
    }

    /// `v.x` — and `v.d` for the discriminator: `_d` is not a Nim
    /// identifier (the production starts at a letter), so the
    /// materialiser declares `d` and this backend maps the walk's
    /// spelling onto it.
    fn member(base: &str, field: &str) -> Self::Expr {
        let spelled = if field == "_d" {
            crate::adapter::nim::DISCRIMINATOR
        } else {
            field
        };
        plain(nim::Expr::dot(nim::Expr::name(base), spelled))
    }

    fn whole(base: &str) -> Self::Expr {
        plain(nim::Expr::name(base))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        plain(nim::Expr::dot(value_of(value), field))
    }

    /// A header's field, in the spelling `runtime/nim` publishes:
    /// `member_id` is `memberId` — and `end`, which is a Nim keyword,
    /// is `endOfList`.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        let spelled = if field == "end" {
            "endOfList".to_string()
        } else {
            Self::camel(field)
        };
        plain(nim::Expr::dot(value_of(value), &spelled))
    }

    /// `a[int(i)]`: every index the walk hands over is a `uint32`
    /// counter, and Nim's `seq` and `array` subscripts take an `int` —
    /// the conversion is the crossing written out.
    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        plain(nim::Expr::Index {
            value: Box::new(value_of(value)),
            index: Box::new(nim::Expr::Convert {
                type_: Box::new(nim::Type::named("int")),
                value: Box::new(value_of(at)),
            }),
        })
    }

    /// `uint32(v.len)`, or the bound where the materialiser fixed one:
    /// `len` answers an `int` and a sequence's count is a `uint32` on
    /// the wire.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => plain(nim::Expr::int(i128::from(bound))),
            None => plain(nim::Expr::Convert {
                type_: Box::new(nim::Type::Primitive(nim::Primitive::UInt32)),
                value: Box::new(nim::Expr::dot(nim::Expr::name(base), "len")),
            }),
        }
    }

    /// The `seq` itself: a Nim `seq` is the elements.
    fn elements(base: &str) -> Self::Expr {
        plain(nim::Expr::name(base))
    }

    /// `v.x`, marked: a union branch is an `Option[T]` — the write
    /// direction reads it as `.get` and the read direction assigns
    /// through `some(..)`, and the marker is what lets one accessor
    /// serve both.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        NimExpr::CaseSlot(value_of(Self::member(base, field)))
    }

    /// `entry.key` and `entry.value`, the two fields the materialiser
    /// gives a map's entry object.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        plain(nim::Expr::dot(
            value_of(entry),
            if key { "key" } else { "value" },
        ))
    }

    fn absent() -> Self::Expr {
        plain(nim::Expr::Literal(Literal::Null))
    }

    /// Nothing: a Nim `string` and a Nim `seq` own their storage, and
    /// there is no allocation failure for the guard to test.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// `v.x.get` — the value an `Option` holds, given that it holds
    /// one. Only ever reached inside a branch the presence test
    /// guarded, where `get`'s own defect cannot fire.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(nim::Expr::dot(value_of(Self::member(base, field)), "get"))
    }

    /// `v.x.isSome`.
    fn present(base: &str, field: &str) -> Self::Expr {
        plain(nim::Expr::dot(
            value_of(Self::member(base, field)),
            "isSome",
        ))
    }

    /// Nothing on the way in — the assignment through `some(..)` is
    /// what makes the member present — and `reset(v.x)` on the way
    /// out: `reset` puts a location back to its default, which for an
    /// `Option` is the absence, and unlike `none(T)` it needs no type
    /// argument this seam does not carry.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![nim::Stmt::Expr(nim::Expr::call(
            nim::Expr::name("reset"),
            vec![value_of(Self::member(base, field))],
        ))]
    }

    /// `reset(v.x)` for every case: a branch is an `Option[T]`, and only
    /// the selected one is `some`. The same statement as an absent
    /// `@optional` member, [`Self::set_present`] with `false`.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .flat_map(|field| Self::set_present(base, field, false))
            .collect()
    }

    /// The marked place; the room is made by [`Nim::read_call`], which
    /// routes the value through `some(..)`.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            NimExpr::OptionalSlot(value_of(Self::member(base, field))),
        )
    }

    /// `true`: an `@external` member is a `ref`, and a member on a cycle needs
    /// the same: `Option[T]` stores its `T` inline.
    /// [`crate::adapter::held_externally`] is the rule the adapter and
    /// the defaulter ask as well.
    fn holds_a_cycle_externally() -> bool {
        true
    }

    /// `v.x[]` — the value behind the `ref`, or `v.x.get[]` where the
    /// member is also `@optional` and the `Option` wraps the
    /// reference: the member's codec takes the value.
    fn external_value(
        _ast: &Ast,
        _id: Id<TypeNode>,
        base: &str,
        field: &str,
        optional: bool,
    ) -> Self::Expr {
        let member = value_of(Self::member(base, field));
        if optional {
            return plain(nim::Expr::Deref(Box::new(nim::Expr::dot(member, "get"))));
        }
        plain(nim::Expr::Deref(Box::new(member)))
    }

    /// The marked place; [`Nim::read_call`] routes the decoded value
    /// through `zdwire.zdRef(..)`, inside `some(..)` where the member
    /// is also `@optional`.
    fn external_destination(
        base: &str,
        field: &str,
        optional: bool,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        let member = value_of(Self::member(base, field));
        let slot = if optional {
            NimExpr::OptionalExternalSlot(member)
        } else {
            NimExpr::ExternalSlot(member)
        };
        (Vec::new(), slot)
    }

    /// `v.x.isSome` where the pair is `@optional @external` —
    /// `Option[ref T]`, so the `Option` answers — and the seam is only
    /// ever emitted for that pair: a bare `@external` member is always
    /// present and no guard reads it.
    fn external_present(base: &str, field: &str) -> Self::Expr {
        Self::present(base, field)
    }

    /// As [`Nim::set_present`], for the same reason: the `Option` is
    /// the outer form of `@optional @external` too.
    fn external_set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        Self::set_present(base, field, present)
    }

    /// `v.x.get[]` for a branch: the union arm's field is
    /// `Option[ref T]`, and the branch's codec takes the value.
    fn external_case_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(nim::Expr::Deref(Box::new(nim::Expr::dot(
            value_of(Self::member(base, field)),
            "get",
        ))))
    }

    /// The marked place; [`Nim::read_call`] routes the decoded value
    /// through `some(zdwire.zdRef(..))`.
    fn external_case_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            NimExpr::CaseExternalSlot(value_of(Self::member(base, field))),
        )
    }

    /// The operators, respelled: Nim writes `and`, `or`, `xor`, `shl`
    /// and `shr` where the walk hands C's forms, and the logical and
    /// bitwise pairs share one keyword, overloaded by type. `=` never
    /// reaches a rendered expression — [`Nim::perform`] turns it into
    /// an assignment statement.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        plain(nim::Expr::Binary {
            op: Self::op(op),
            lhs: Box::new(value_of(lhs)),
            rhs: Box::new(value_of(rhs)),
        })
    }

    /// `not a`, for both of C's spellings — Nim's `not` is the logical
    /// and the bitwise complement, overloaded by type.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        if op == "*" {
            return plain(nim::Expr::Deref(Box::new(value_of(value))));
        }
        plain(nim::Expr::Unary {
            op: match op {
                "!" | "~" => "not",
                other => other,
            },
            value: Box::new(value_of(value)),
        })
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        plain(nim::Expr::Paren(Box::new(value_of(value))))
    }

    /// `T(v)` — Nim's checked conversion — with three rewrites.
    ///
    /// A `bool` discriminator's integer label becomes the boolean
    /// literal, because `bool(1)` is not the literal the comparison
    /// wants written. The walk's `signed char` — the C spelling of a
    /// map key's signed order — is `cast[int8]`, which reads a `uint8`
    /// key as the two's complement the corpus was recorded under where
    /// `int8(200'u8)` would raise. And the walk's `Int64` widening —
    /// an enumeration's holder, the union overlay's pattern — goes
    /// through `zdwire.zdI64`, which widens an enum or a signed value
    /// by conversion and a `uint64` by `cast`, because `int64(x)` on a
    /// `uint64` above `int64.high` raises where the overlay needs the
    /// bits.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == nim::Type::named(SIGNED_CHAR) {
            return plain(nim::Expr::Cast {
                type_: Box::new(nim::Type::Primitive(nim::Primitive::Int8)),
                value: Box::new(value_of(value)),
            });
        }
        if type_ == nim::Type::Primitive(nim::Primitive::Bool) {
            if let NimExpr::Plain(nim::Expr::Literal(Literal::Int(text))) = &value {
                return plain(nim::Expr::Literal(Literal::Bool(text != "0")));
            }
        }
        if type_ == nim::Type::Primitive(nim::Primitive::Int64) {
            return plain(nim::Expr::call(
                Self::runtime_name("zdI64"),
                vec![value_of(value)],
            ));
        }
        plain(nim::Expr::Convert {
            type_: Box::new(type_),
            value: Box::new(value_of(value)),
        })
    }

    /// The value, unchanged: Nim hands a `seq` by its handle, an
    /// `array` by value and an object by value.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// A call to a module-level routine — a generated codec or
    /// defaulter. `strcmp` is the runtime's, which spells C11 §7.24.4's
    /// unsigned-octet order in a language whose own string order over
    /// UTF-8 is exactly that.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let arguments: Vec<nim::Expr> = args.into_iter().map(value_of).collect();
        if callee == "strcmp" {
            return plain(nim::Expr::call(Self::runtime_name("strcmp"), arguments));
        }
        plain(nim::Expr::call(nim::Expr::name(callee), arguments))
    }

    /// `w.writeU32(x)` — a method on the writer or the reader. One
    /// operation is renamed rather than converted: the walk's
    /// `read_pid()` opens a value block §7.4.1.2 counts alignment
    /// from, which is `readPidAs` in this runtime; the Go and D
    /// columns do the same.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "read_pid" && args.is_empty() {
            return Self::runtime(
                receiver,
                "read_pid_as",
                vec![Self::constant(Constant::ParameterMember)],
            );
        }
        plain(nim::Expr::call(
            nim::Expr::dot(nim::Expr::name(receiver), &Self::camel(method)),
            args.into_iter().map(value_of).collect(),
        ))
    }

    /// A free routine of the runtime, qualified — except
    /// `pid_is_user_member`, which is a method on the header the
    /// reader handed back.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "pid_is_user_member" {
            let mut arguments = args.into_iter().map(value_of);
            if let Some(header) = arguments.next() {
                return plain(nim::Expr::call(
                    nim::Expr::dot(header, "isUserMember"),
                    Vec::new(),
                ));
            }
            // The walk always hands the header; a call without one is a
            // walk that changed under this file, and a routine on a name
            // no scope declares is the loudest thing Nim can say here.
            return plain(nim::Expr::call(
                Self::runtime_name("isUserMember"),
                Vec::new(),
            ));
        }
        plain(nim::Expr::call(
            Self::runtime_name(&Self::camel(method)),
            args.into_iter().map(value_of).collect(),
        ))
    }

    /// `runtime/nim` carries the three HDDS axes: `beginPackedElement` /
    /// `finishPackedElement` on the writer and `readerBeginPackedElement`
    /// / `readerFinishPackedElement` on the reader, the DHEADER pair for
    /// a sequence element's own length, and `writeWstringUtf8Octets` /
    /// `writeWstringUtf8OctetsBounded` / `readWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    // -- statements ----------------------------------------------------

    /// `var name: T = value` — the type written, so a reader of the
    /// generated file sees the width, and definite whether or not the
    /// value is there.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        nim::Stmt::Var {
            name: Ident::new(name),
            type_: Some(type_),
            value: value.map(value_of),
        }
    }

    /// An expression evaluated for its effect — or, where the walk
    /// built an assignment, the assignment as its own statement. Every
    /// call the walk drops here answers nothing, which is what lets
    /// the statement stand without a `discard`.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let NimExpr::Plain(nim::Expr::Binary { op: "=", lhs, rhs }) = value {
            return nim::Stmt::Assign {
                place: *lhs,
                value: *rhs,
            };
        }
        nim::Stmt::Expr(value_of(value))
    }

    /// `return v`: a Nim decoder answers with the sample it built.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        nim::Stmt::Return(Some(match value {
            Some(value) => value_of(value),
            None => nim::Expr::name("v"),
        }))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        nim::Stmt::If {
            condition: value_of(condition),
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// `for i in 0'u32 ..< bound`: every bound the walk hands over is
    /// a `uint32` count off the wire or an integer literal Nim
    /// converts against the range.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        nim::Stmt::For {
            counter: Ident::new(counter),
            bound: Box::new(value_of(bound)),
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        nim::Stmt::While {
            condition: value_of(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        nim::Stmt::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        nim::Stmt::Comment(doc)
    }

    /// `discard expr` — Nim's own statement for a value read for its
    /// effect; an expression statement with a value does not compile.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        nim::Stmt::Discard(value_of(value))
    }

    /// `discard name` — a parameter deliberately not read, valid
    /// whether or not the body goes on to use it.
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![nim::Stmt::Discard(nim::Expr::name(name))]
    }

    /// `place = zd_dec_T(r, behaviour)` — and through `some(..)`,
    /// `zdwire.zdRef(..)` or both where the place is a marked slot:
    /// the decoder answers a `T` and the field holds a wrapper, a
    /// reference, or a wrapper over one.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let decoded = nim::Expr::call(
            nim::Expr::name(callee),
            vec![nim::Expr::name("r"), value_of(behaviour)],
        );
        Self::into_slot(place, decoded)
    }

    /// `v = zd_dec_T(r, zd_tc)` — a `typedef`, handed straight on.
    fn read_forward(callee: &str) -> Self::Stmt {
        Self::read_call(
            callee,
            Self::whole("v"),
            Handoff::Value,
            Self::name("zd_tc"),
        )
    }

    /// One scalar into a place, through the slot markers exactly as
    /// [`Nim::read_call`] routes them — the wchar element form is the
    /// one scalar read that reaches a place directly.
    fn store(place: Self::Expr, _type_: Self::Type, value: Self::Expr) -> Self::Stmt {
        Self::into_slot(place, value_of(value))
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// `v = newSeq[T](int(n))`, and the fill after it: a Nim `seq`'s
    /// slots are storage the moment `newSeq` returns, for a sequence's
    /// elements and a map entry's halves alike, so `assigned_whole`
    /// makes no difference here.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![nim::Stmt::Assign {
            place: nim::Expr::name(base),
            value: nim::Expr::NewSeq {
                element: Box::new(element),
                length: Box::new(nim::Expr::Convert {
                    type_: Box::new(nim::Type::named("int")),
                    value: Box::new(value_of(count)),
                }),
            },
        }];
        out.extend(fill);
        out
    }

    /// `v.setLen(int(count))` — §7.3.1.2.1.12.3's TRIM, applied after
    /// every element has been read.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![nim::Stmt::Expr(nim::Expr::call(
            nim::Expr::dot(nim::Expr::name(base), "setLen"),
            vec![nim::Expr::Convert {
                type_: Box::new(nim::Type::named("int")),
                value: Box::new(value_of(count)),
            }],
        ))]
    }

    /// The subscript, with nothing around it: a Nim `seq` element is a
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

    /// `proc zd_enc_T(w: zdwire.Writer, v: T)` and
    /// `proc zd_dec_T(r: zdwire.Reader, zd_tc: zdwire.TryConstruct): T`.
    ///
    /// A forward declaration is answered with the bodiless form: Nim
    /// resolves a module-scope name only after its declaration, so a
    /// recursive or mutually recursive pair needs the signatures ahead
    /// of the bodies — the same shape the C column emits, and unlike D
    /// and Go, where any order compiles.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let mut params = vec![nim::Param {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            type_: nim::Type::named(if spec.writing {
                "zdwire.Writer"
            } else {
                "zdwire.Reader"
            }),
        }];
        if spec.writing {
            params.push(nim::Param {
                name: Ident::new("v"),
                type_: spec.sample.clone(),
            });
        }
        for (type_, name) in spec.extra {
            params.push(nim::Param {
                name: Ident::new(&name),
                type_,
            });
        }
        let (returns, body) = if spec.writing {
            (None, spec.body)
        } else {
            let body = spec.body.map(|body| {
                // The sample starts as Table 9's value, through the
                // generated defaulter, so every position the stream
                // does not carry is already what the clause says it
                // is.
                let defaulter = crate::default_nim::defaulter_from_decoder(&spec.name);
                let mut statements = vec![nim::Stmt::Var {
                    name: Ident::new("v"),
                    type_: None,
                    value: Some(nim::Expr::call(nim::Expr::name(&defaulter), Vec::new())),
                }];
                statements.extend(body);
                statements.push(nim::Stmt::Return(Some(nim::Expr::name("v"))));
                statements
            });
            (Some(spec.sample), body)
        };
        Some(nim::ProcDecl {
            doc: spec.doc,
            name: Ident::new(&spec.name),
            // Nim's export marker. Without it the codec is visible only
            // inside the generated file — `import gen` would not bring
            // `zd_enc_Max` with it — and the probe harness never saw it
            // because it writes its `main` into that same file.
            exported: true,
            params,
            returns,
            body,
        })
    }

    /// A call to the generated defaulter, which is Table 9 once per
    /// type rather than once per position — routed through the slot
    /// markers exactly as [`Nim::read_call`]'s value is.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        let defaulted = nim::Expr::call(
            nim::Expr::name(&crate::default_nim::defaulter_name(ast, id)),
            Vec::new(),
        );
        Ok(Self::into_slot(place, defaulted))
    }

    /// # Errors
    /// As [`crate::default_nim::member_default`]: a `@default` this
    /// column cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_nim::member_default(ast, node, type_, value_of(place))
    }

    /// `common_Colour_RED` — the module-scope constant the
    /// materialiser declared the enumerator's field under. The trait's
    /// default concatenates the prefix and the escape, which would
    /// escape the segment rather than the joined name.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        plain(nim::Expr::name(&nim_enumerator_constant(
            ast, id, enumerator,
        )))
    }
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// The tag names the routine a hoisted collection's codec is emitted
/// under, and one body is emitted per name. With the bound left out,
/// `sequence<long>` and `sequence<long, 4>` are one name and whichever
/// was declared first decides: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated Nim held no `zd_over` check at all, so `a` accepted a
/// stream of any length; with the members the other way round both got
/// the bounded body, so `b` refused — and truncated — a length the IDL
/// puts no limit on. `string<4>` beside `string` and `wstring<4>`
/// beside `wstring` collapse the same way, onto the trimming call.
/// Either way the reader answered with a value the IDL does not
/// describe and said nothing, which is why the bound is part of the
/// name. The same suffix the Zig column carries.
///
/// An `array` needs nothing here: its `dims` are already in the tag,
/// under the `Array` arm below.
fn bound_tag(ast: &Ast, bound: &zerodds_ast::Bound) -> String {
    match bound.expr() {
        None => String::new(),
        // `_b` and not a bare `_n`: `seq_long_4` is what the `Array`
        // arm answers for an array of four `long`, and the two are
        // different wire forms.
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
/// and not the Nim spelling, structurally for a collection and the
/// flattened bare name for a declaration — the same scheme the D and Go
/// columns carry, for the reasons their `type_tag`s state. The bare
/// name and not [`crate::adapter::nim::nim_type_name`]: a stropped
/// segment inside `zd_enc_..` would not be an identifier, and the
/// `zd_enc_` prefix already keeps the name clear of every keyword.
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
        | TypeKind::Native => nim_bare_name(ast, id),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}
