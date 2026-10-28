// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The D backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as the seven columns
//! before it, and none of them calls another. Where two columns
//! disagree on an octet, one has misread the specification — which is
//! the only reason to have a ninth.
//!
//! # The four decisions this column turns on
//!
//! **A member holds its wire width, under D's own names.** An IDL
//! `long` is a D `int` and an IDL `long long` a D `long`; the adapter's
//! table carries the warning at length. A `char` is `ubyte` and a
//! `wchar` `ushort`, so [`CodecTarget::code_point`] is the identity and
//! a union's discriminator compares as a number.
//!
//! **`@optional` is `Nullable!(T)` and `@external` is `T*`, and the
//! four seams keep them apart.** `!v.m.isNull` against `v.m !is null`,
//! `v.m.get` against `*v.m`, `zdwire.zdNullable(..)` against
//! `zdwire.zdPtr(..)` — and `@optional @external` takes the pointer
//! forms, because a `Nullable` over a pointer would carry a
//! present-but-null state the wire form has no encoding for.
//!
//! **A `long double` and a `fixed` are decimal text.** D's `real` is
//! whatever the target has — 80-bit x87 on x86, plain binary64 on
//! AArch64 — and a binary128 has 113 significand bits, so `runtime/d`
//! parses and prints the member's decimal exactly.
//!
//! **Every runtime name is qualified.** `zdwire.Writer`,
//! `zdwire.TryConstructDiscard`, `zdwire.zdPtr`: a generated
//! declaration and a runtime name share one module scope in D, and a
//! corpus case declaring `struct Writer` would otherwise take the name
//! over.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::d::{d_map_entry_name, d_type_name, d_value_type, field_name};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::d;

/// The D backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct D;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The module the generated file imports the runtime under.
const RUNTIME: &str = "zdwire";

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`; the D member is a `ubyte` and the signed reading is `byte`.
const SIGNED_CHAR: &str = "signed char";

/// An expression, with the one property the walk's seams cannot carry:
/// which indirection the place it names lives behind.
#[derive(Debug, Clone)]
pub enum DExpr {
    /// An ordinary expression.
    Plain(d::Expr),
    /// The `Nullable!(T)` field of an `@optional` member, as a place a
    /// decoded `T` is about to be written into.
    OptionalSlot(d::Expr),
    /// The `T*` field of an `@external` member, likewise.
    ExternalSlot(d::Expr),
}

/// The expression inside, whichever form it took.
fn inner(expr: DExpr) -> d::Expr {
    match expr {
        DExpr::Plain(e) | DExpr::OptionalSlot(e) | DExpr::ExternalSlot(e) => e,
    }
}

fn plain(e: d::Expr) -> DExpr {
    DExpr::Plain(e)
}

impl D {
    /// A runtime name, qualified: `zdwire.X`.
    fn runtime_name(name: &str) -> d::Expr {
        d::Expr::Dot {
            value: Box::new(d::Expr::Name(Ident::new(RUNTIME))),
            name: Ident::new(name),
        }
    }

    /// A runtime operation's D spelling: `write_u32` is `WriteU32`,
    /// each `_`-separated segment with its first letter raised — the
    /// names `runtime/d` publishes, because that runtime was written
    /// against this conversion.
    fn pascal(method: &str) -> String {
        let mut out = String::new();
        for segment in method.split('_') {
            let mut chars = segment.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
        out
    }

    fn text(value: &str) -> DExpr {
        plain(d::Expr::Literal(Literal::Str(value.to_string())))
    }

    fn runtime_constant(name: &str) -> DExpr {
        plain(Self::runtime_name(name))
    }

    fn dot(value: d::Expr, name: &str) -> d::Expr {
        d::Expr::Dot {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    fn call_expr(func: d::Expr, args: Vec<d::Expr>) -> d::Expr {
        d::Expr::Call {
            func: Box::new(func),
            args,
        }
    }
}

impl CodecTarget for D {
    type Expr = DExpr;
    type Stmt = d::Stmt;
    type Block = Vec<d::Stmt>;
    type Type = d::Type;
    type Decl = d::Decl;

    // -- types --------------------------------------------------------

    /// The fixed-width D type each scalar local takes. `Index` is
    /// `uint` — a sequence length is a `uint32` on the wire — and
    /// `Size` is `int`, which is what the reader's positions answer in.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => d::Type::Primitive(d::Primitive::Bool),
            Scalar::Char | Scalar::UInt8 => d::Type::Primitive(d::Primitive::UByte),
            Scalar::Int8 => d::Type::Primitive(d::Primitive::Byte),
            Scalar::Int16 => d::Type::Primitive(d::Primitive::Short),
            Scalar::UInt16 => d::Type::Primitive(d::Primitive::UShort),
            Scalar::Int32 | Scalar::Size => d::Type::Primitive(d::Primitive::Int),
            Scalar::UInt32 | Scalar::Index => d::Type::Primitive(d::Primitive::UInt),
            Scalar::Int64 => d::Type::Primitive(d::Primitive::Long),
            Scalar::UInt64 => d::Type::Primitive(d::Primitive::ULong),
            Scalar::Float => d::Type::Primitive(d::Primitive::Float),
            Scalar::Double => d::Type::Primitive(d::Primitive::Double),
            // The decimal text; the module documentation says why.
            Scalar::LongDouble => d::Type::Primitive(d::Primitive::String),
        }
    }

    fn named(name: &str) -> Self::Type {
        d::Type::named(name)
    }

    /// `void`, which is what a D function that answers nothing returns.
    fn nothing() -> Self::Type {
        d::Type::Primitive(d::Primitive::Void)
    }

    /// The runtime's handles, qualified.
    fn handle(kind: Handle) -> Self::Type {
        d::Type::named(match kind {
            Handle::DHeader => "zdwire.DHeader",
            Handle::EmHeader => "zdwire.EmHeader",
            Handle::Pid => "zdwire.Pid",
            Handle::TryConstruct => "zdwire.TryConstruct",
            Handle::ReaderMark => "zdwire.ReaderMark",
            Handle::PidRead => "zdwire.PidRead",
            Handle::EmHeaderRead => "zdwire.EmHeaderRead",
        })
    }

    /// The member already is the code unit — `ubyte` for a `char`,
    /// `ushort` for a `wchar` — so there is nothing to convert.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy values: strings and integers where
    /// `runtime/d` takes them so, and qualified constants for the
    /// enumerated kinds. One arm per constant and no catch-all.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => plain(d::Expr::int(2)),
            Constant::WcharWidth4 => plain(d::Expr::int(4)),
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
    /// answer the materialiser's `d_holder` gives its fields.
    fn unsigned_holder(bits: u16) -> Self::Type {
        d::Type::Primitive(crate::adapter::d::d_holder(bits))
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        // Written as the match `unwrap_or` folds away, so the fallback
        // carries its reason where the silent-drop guard reads it: a
        // type with no D form was refused by the materialiser before a
        // codec walk ran, and `void` makes the impossible fail in the D
        // compiler, loudly, rather than compile into a plausible
        // member.
        #[allow(clippy::manual_unwrap_or_default, clippy::manual_unwrap_or)]
        match d_value_type(ast, id) {
            Ok(type_) => type_,
            Err(_) => d::Type::Primitive(d::Primitive::Void),
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
        d_map_entry_name(ast, id).unwrap_or_else(|_| format!("ZdUnresolvedEntry{}", id.slot()))
    }

    /// Nothing: a declared type's D name is already flat —
    /// `d_type_name` folds the modules in — and an enumerator lives in
    /// its enum's own scope, which
    /// [`CodecTarget::enumerator_constant`]'s override reaches.
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
        plain(d::Expr::Name(Ident::new(text)))
    }

    fn int(value: i128) -> Self::Expr {
        plain(d::Expr::int(value))
    }

    /// The raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten into the tree's own boolean where they are one.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return plain(d::Expr::Literal(value));
        };
        match text.as_str() {
            "true" => plain(d::Expr::Literal(Literal::Bool(true))),
            "false" => plain(d::Expr::Literal(Literal::Bool(false))),
            _ => plain(d::Expr::Literal(value)),
        }
    }

    /// A literal wide enough to survive its own width: D types an
    /// unsuffixed integer literal `int`, so a union label past `int`
    /// carries `L` and one past `long` carries `UL`.
    ///
    /// −2^63 is the one value the `L` cannot be written on. D's lexical
    /// grammar gives an integer literal no sign ("Lexical", Integer
    /// Literals), so `-9223372036854775808L` is a minus over a literal
    /// whose `L` asks for a `long` it does not fit, and both compilers
    /// this column is measured with refuse it outright rather than
    /// warn: ldc2 1.40.0 and gdc 14.2.0 both say
    /// `Error: signed integer overflow`. `long.min` is the value under
    /// its own name — the `.min` property D defines for every integral
    /// type ("Properties for Integral Data Types") — and it is a
    /// compile-time constant, so it stands wherever the suffixed
    /// literal did: inside the `cast(long)` of a union comparison, and
    /// as the initialiser of an `enum` manifest constant. The same
    /// answer `emit/kotlin.rs` gives with `Long.MIN_VALUE`, for the
    /// same reason.
    fn wide_int(value: i128) -> Self::Expr {
        if value > i128::from(i64::MAX) {
            return plain(d::Expr::suffixed_int(value, "UL"));
        }
        if value == i128::from(i64::MIN) {
            return plain(d::Expr::Literal(Literal::Int("long.min".to_string())));
        }
        if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
            return plain(d::Expr::suffixed_int(value, "L"));
        }
        plain(d::Expr::int(value))
    }

    /// The number, with no `u` suffix: an `int` literal converts
    /// against a `uint` counter under D's value range propagation, and
    /// a bound past `int` gets its width from [`CodecTarget::wide_int`].
    fn unsigned_int(value: i128) -> Self::Expr {
        Self::wide_int(value)
    }

    /// `..UL`, typed: a mask the walk complements has to already be a
    /// `ulong`, and `~` on an `int` would sign-extend a different
    /// pattern.
    fn wide_unsigned(value: u64) -> Self::Expr {
        plain(d::Expr::suffixed_int(i128::from(value), "UL"))
    }

    /// `..L`, for the enumerator patterns `zd_raw` is compared against.
    fn wide_signed(value: i128) -> Self::Expr {
        plain(d::Expr::suffixed_int(value, "L"))
    }

    /// `v.x` — and `v._d` as it stands: D keeps the IDL-facing
    /// spelling, underscore and all.
    fn member(base: &str, field: &str) -> Self::Expr {
        plain(Self::dot(d::Expr::Name(Ident::new(base)), field))
    }

    fn whole(base: &str) -> Self::Expr {
        plain(d::Expr::Name(Ident::new(base)))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        plain(Self::dot(inner(value), field))
    }

    /// A header's field, in the spelling `runtime/d` publishes:
    /// `member_id` is `MemberId`.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        plain(Self::dot(inner(value), &Self::pascal(field)))
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        plain(d::Expr::Index {
            value: Box::new(inner(value)),
            index: Box::new(inner(at)),
        })
    }

    /// `cast(uint)v.length`, or the bound where the materialiser fixed
    /// one: `.length` is a `size_t` and a sequence's count is a
    /// `uint32` on the wire.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => plain(d::Expr::int(i128::from(bound))),
            None => plain(d::Expr::Cast {
                type_: Box::new(d::Type::Primitive(d::Primitive::UInt)),
                value: Box::new(Self::dot(d::Expr::Name(Ident::new(base)), "length")),
            }),
        }
    }

    /// The slice itself: a D slice is the elements.
    fn elements(base: &str) -> Self::Expr {
        plain(d::Expr::Name(Ident::new(base)))
    }

    /// `v.x` — the anonymous union puts its fields directly on the
    /// enclosing struct.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `entry.key` and `entry.value`, the two fields the materialiser
    /// gives a map's entry struct.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        plain(Self::dot(inner(entry), if key { "key" } else { "value" }))
    }

    fn absent() -> Self::Expr {
        plain(d::Expr::Literal(Literal::Null))
    }

    /// Nothing: a D string and a D slice own their storage, and there
    /// is no allocation failure for the guard to test.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// `v.x.get` — the value a `Nullable` holds, given that it holds
    /// one. Only ever reached inside a branch the presence test
    /// guarded, where `.get`'s own assertion cannot fire.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(Self::dot(inner(Self::member(base, field)), "get"))
    }

    /// `!v.x.isNull`.
    fn present(base: &str, field: &str) -> Self::Expr {
        plain(d::Expr::Unary {
            op: "!",
            value: Box::new(Self::dot(inner(Self::member(base, field)), "isNull")),
        })
    }

    /// Nothing on the way in — the assignment through
    /// `zdwire.zdNullable` is what makes the member present — and
    /// `v.x.nullify();` on the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![d::Stmt::Expr(Self::call_expr(
            Self::dot(inner(Self::member(base, field)), "nullify"),
            Vec::new(),
        ))]
    }

    /// The marked place; the room is made by [`D::read_call`], which
    /// routes the value through `zdwire.zdNullable`.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            DExpr::OptionalSlot(inner(Self::member(base, field))),
        )
    }

    /// `true`: an `@external` member is `T*`, and a member on a cycle needs the
    /// same: `Nullable!(T)` stores its `T` by value.
    /// [`crate::adapter::held_externally`] is the rule the adapter and
    /// the defaulter ask as well.
    fn holds_a_cycle_externally() -> bool {
        true
    }

    /// `*v.x` — the pointer dereferenced, for both `@external` and
    /// `@optional @external`: the member's codec takes the value.
    fn external_value(
        _ast: &Ast,
        _id: Id<TypeNode>,
        base: &str,
        field: &str,
        _optional: bool,
    ) -> Self::Expr {
        plain(d::Expr::Unary {
            op: "*",
            value: Box::new(inner(Self::member(base, field))),
        })
    }

    /// The marked place; [`D::read_call`] routes the decoded value
    /// through `zdwire.zdPtr`.
    fn external_destination(
        base: &str,
        field: &str,
        _optional: bool,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            DExpr::ExternalSlot(inner(Self::member(base, field))),
        )
    }

    /// `*v.x` for a branch too: the union arm's field is the same `T*`
    /// a structure's `@external` member is, and the branch's codec takes
    /// the value.
    fn external_case_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(d::Expr::Unary {
            op: "*",
            value: Box::new(inner(Self::case_member(base, field))),
        })
    }

    /// The marked place, exactly as [`D::external_destination`] answers
    /// for a structure member; [`D::read_call`] routes the decoded value
    /// through `zdwire.zdPtr`.
    fn external_case_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            DExpr::ExternalSlot(inner(Self::case_member(base, field))),
        )
    }

    /// `v.x !is null`: the pointer is the absence, with no `Nullable`
    /// around it to carry a second one.
    fn external_present(base: &str, field: &str) -> Self::Expr {
        plain(d::Expr::Binary {
            op: "!is",
            lhs: Box::new(inner(Self::member(base, field))),
            rhs: Box::new(d::Expr::Literal(Literal::Null)),
        })
    }

    /// Nothing on the way in, `v.x = null;` on the way out.
    fn external_set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![d::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: d::Expr::Literal(Literal::Null),
        }]
    }

    /// The operators as given: D spells `&&`, `||`, `~` and the
    /// comparisons the way the walk does. `=` never reaches a rendered
    /// expression — [`D::perform`] turns it into an assignment
    /// statement.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        plain(d::Expr::Binary {
            op,
            lhs: Box::new(inner(lhs)),
            rhs: Box::new(inner(rhs)),
        })
    }

    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        plain(d::Expr::Unary {
            op,
            value: Box::new(inner(value)),
        })
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        plain(d::Expr::Paren(Box::new(inner(value))))
    }

    /// `cast(T)v`, with two rewrites: a `bool` discriminator's integer
    /// label becomes the boolean literal, and the walk's `signed char`
    /// — the C spelling of a map key's signed order — is `byte` here,
    /// which reads a `ubyte` key as the two's complement the corpus was
    /// recorded under.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == d::Type::named(SIGNED_CHAR) {
            return plain(d::Expr::Cast {
                type_: Box::new(d::Type::Primitive(d::Primitive::Byte)),
                value: Box::new(inner(value)),
            });
        }
        if type_ == d::Type::Primitive(d::Primitive::Bool) {
            if let DExpr::Plain(d::Expr::Literal(Literal::Int(text))) = &value {
                return plain(d::Expr::Literal(Literal::Bool(text != "0")));
            }
        }
        plain(d::Expr::Cast {
            type_: Box::new(type_),
            value: Box::new(inner(value)),
        })
    }

    /// The value, unchanged: D hands a slice by header, a static array
    /// by value and a struct by value.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// A call to a package-level function — a generated codec or
    /// defaulter. `strcmp` is the runtime's `Strcmp`, which spells C11
    /// §7.24.4's unsigned-octet order in a language whose own string
    /// order over UTF-8 is exactly that.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let arguments: Vec<d::Expr> = args.into_iter().map(inner).collect();
        if callee == "strcmp" {
            return plain(Self::call_expr(Self::runtime_name("Strcmp"), arguments));
        }
        plain(Self::call_expr(
            d::Expr::Name(Ident::new(callee)),
            arguments,
        ))
    }

    /// `w.WriteU32(x)` — a method on the writer or the reader. One
    /// operation is renamed rather than converted: the walk's
    /// `read_pid()` opens a value block §7.4.1.2 counts alignment from,
    /// which is `ReadPidAs` in this runtime; the Python and Go columns
    /// do the same.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "read_pid" && args.is_empty() {
            return Self::runtime(
                receiver,
                "read_pid_as",
                vec![Self::constant(Constant::ParameterMember)],
            );
        }
        plain(Self::call_expr(
            Self::dot(d::Expr::Name(Ident::new(receiver)), &Self::pascal(method)),
            args.into_iter().map(inner).collect(),
        ))
    }

    /// A free function of the runtime, qualified — except
    /// `pid_is_user_member`, which is a method on the header the reader
    /// handed back.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "pid_is_user_member" {
            let mut arguments = args.into_iter().map(inner);
            if let Some(header) = arguments.next() {
                return plain(Self::call_expr(
                    Self::dot(header, "IsUserMember"),
                    Vec::new(),
                ));
            }
            // The walk always hands the header; a call without one is a
            // walk that changed under this file, and a method on a name
            // no scope declares is the loudest thing D can say here.
            return plain(Self::call_expr(
                Self::runtime_name("IsUserMember"),
                Vec::new(),
            ));
        }
        plain(Self::call_expr(
            Self::runtime_name(&Self::pascal(method)),
            args.into_iter().map(inner).collect(),
        ))
    }

    /// `runtime/d` carries the three HDDS axes: `BeginPackedElement` /
    /// `FinishPackedElement` on the writer and `ReaderBeginPackedElement`
    /// / `ReaderFinishPackedElement` on the reader, the DHEADER pair for
    /// a sequence element's own length, and `WriteWstringUtf8Octets` /
    /// `WriteWstringUtf8OctetsBounded` / `ReadWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    // -- statements ----------------------------------------------------

    /// `T name = value;` — the type written, so a reader of the
    /// generated file sees the width, and definite whether or not the
    /// value is there.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        d::Stmt::Var {
            name: Ident::new(name),
            type_: Some(type_),
            value: value.map(inner),
        }
    }

    /// An expression evaluated for its effect — or, where the walk
    /// built an assignment, the assignment as its own statement, so the
    /// generated file reads as every other column's does.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let DExpr::Plain(d::Expr::Binary { op: "=", lhs, rhs }) = value {
            return d::Stmt::Assign {
                place: *lhs,
                value: *rhs,
            };
        }
        d::Stmt::Expr(inner(value))
    }

    /// `return v;`: a D decoder answers with the sample it built.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        d::Stmt::Return(Some(match value {
            Some(value) => inner(value),
            None => d::Expr::Name(Ident::new("v")),
        }))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        d::Stmt::If {
            condition: inner(condition),
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// `for (uint i = 0; i < bound; i++)`: every bound the walk hands
    /// over is a `uint` count off the wire or an `int` literal D's
    /// value range propagation converts.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        d::Stmt::For {
            counter: Ident::new(counter),
            type_: Box::new(d::Type::Primitive(d::Primitive::UInt)),
            bound: Box::new(inner(bound)),
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        d::Stmt::While {
            condition: inner(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        d::Stmt::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        d::Stmt::Comment(doc)
    }

    /// `cast(void)expr;` — D's spelling of a value read for its effect,
    /// the same one C writes.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        d::Stmt::Expr(d::Expr::Cast {
            type_: Box::new(d::Type::Primitive(d::Primitive::Void)),
            value: Box::new(inner(value)),
        })
    }

    /// `cast(void)name;` — a parameter deliberately not read, valid
    /// whether or not the body goes on to use it.
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![d::Stmt::Expr(d::Expr::Cast {
            type_: Box::new(d::Type::Primitive(d::Primitive::Void)),
            value: Box::new(d::Expr::Name(Ident::new(name))),
        })]
    }

    /// `place = zd_dec_T(r, behaviour);` — and through
    /// `zdwire.zdNullable` or `zdwire.zdPtr` where the place is an
    /// `@optional` or an `@external` member's field: the decoder
    /// answers a `T` and the field holds a wrapper or a pointer.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let decoded = Self::call_expr(
            d::Expr::Name(Ident::new(callee)),
            vec![d::Expr::Name(Ident::new("r")), inner(behaviour)],
        );
        match place {
            DExpr::OptionalSlot(field) => d::Stmt::Assign {
                place: field,
                value: Self::call_expr(Self::runtime_name("zdNullable"), vec![decoded]),
            },
            DExpr::ExternalSlot(field) => d::Stmt::Assign {
                place: field,
                value: Self::call_expr(Self::runtime_name("zdPtr"), vec![decoded]),
            },
            DExpr::Plain(place) => d::Stmt::Assign {
                place,
                value: decoded,
            },
        }
    }

    /// `v = zd_dec_T(r, zd_tc);` — a `typedef`, handed straight on.
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

    /// `v = new T[](n);`, and the fill after it: a D slice's slots are
    /// storage the moment `new` returns, for a sequence's elements and
    /// a map entry's halves alike, so `assigned_whole` makes no
    /// difference here.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![d::Stmt::Assign {
            place: d::Expr::Name(Ident::new(base)),
            value: d::Expr::NewSlice {
                element: Box::new(element),
                length: Box::new(inner(count)),
            },
        }];
        out.extend(fill);
        out
    }

    /// `v = v[0 .. count];` — §7.3.1.2.1.12.3's TRIM, applied after
    /// every element has been read.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![d::Stmt::Assign {
            place: d::Expr::Name(Ident::new(base)),
            value: d::Expr::SliceTo {
                value: Box::new(d::Expr::Name(Ident::new(base))),
                high: Box::new(inner(count)),
            },
        }]
    }

    /// The subscript, with nothing around it: a D slice element is a
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

    /// `void zd_enc_T(zdwire.Writer w, T v)` and
    /// `T zd_dec_T(zdwire.Reader r, zdwire.TryConstruct zd_tc)`.
    ///
    /// A forward declaration is answered with nothing: a D module-scope
    /// name is in scope for the whole module however far down it is
    /// written, so a recursive type needs no declaration ahead of its
    /// body.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![d::Param {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            type_: d::Type::named(if spec.writing {
                "zdwire.Writer"
            } else {
                "zdwire.Reader"
            }),
        }];
        if spec.writing {
            params.push(d::Param {
                name: Ident::new("v"),
                type_: spec.sample.clone(),
            });
        }
        for (type_, name) in spec.extra {
            params.push(d::Param {
                name: Ident::new(&name),
                type_,
            });
        }
        let (returns, body) = if spec.writing {
            (None, body)
        } else {
            // The sample starts as Table 9's value, through the
            // generated defaulter, so every position the stream does
            // not carry is already what the clause says it is.
            let defaulter = crate::default_d::defaulter_from_decoder(&spec.name);
            let mut statements = vec![d::Stmt::Var {
                name: Ident::new("v"),
                type_: None,
                value: Some(Self::call_expr(
                    d::Expr::Name(Ident::new(&defaulter)),
                    Vec::new(),
                )),
            }];
            statements.extend(body);
            statements.push(d::Stmt::Return(Some(d::Expr::Name(Ident::new("v")))));
            (Some(spec.sample), statements)
        };
        Some(d::Decl::Func(d::FuncDecl {
            doc: spec.doc,
            name: Ident::new(&spec.name),
            params,
            returns,
            body,
        }))
    }

    /// A call to the generated defaulter, which is Table 9 once per
    /// type rather than once per position.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        let defaulted = Self::call_expr(
            d::Expr::Name(Ident::new(&crate::default_d::defaulter_name(ast, id))),
            Vec::new(),
        );
        // The slot markers route exactly as [`D::read_call`]'s do: the
        // defaulter answers a `T` and an `@external` field holds a `T*`,
        // an `@optional` one a `Nullable!(T)`. Assigning the bare value
        // into either is not D.
        Ok(match place {
            DExpr::OptionalSlot(field) => d::Stmt::Assign {
                place: field,
                value: Self::call_expr(Self::runtime_name("zdNullable"), vec![defaulted]),
            },
            DExpr::ExternalSlot(field) => d::Stmt::Assign {
                place: field,
                value: Self::call_expr(Self::runtime_name("zdPtr"), vec![defaulted]),
            },
            DExpr::Plain(field) => d::Stmt::Assign {
                place: field,
                value: defaulted,
            },
        })
    }

    /// # Errors
    /// As [`crate::default_d::member_default`]: a `@default` this
    /// column cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_d::member_default(ast, node, type_, inner(place))
    }

    /// `Mode.IDLE` — the enum's own scope, which is where the
    /// materialiser declared the member. The trait's default answers a
    /// bare prefixed name, which in D names nothing.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        plain(d::Expr::Name(Ident::new(&format!(
            "{}.{}",
            d_type_name(ast, id),
            field_name(enumerator)
        ))))
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
/// generated D held no `zd_over` check at all, so `a` accepted a
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
/// and not the D spelling, structurally for a collection and the
/// flattened D name for a declaration — the same scheme the Go column
/// carries, for the reasons its `type_tag` states.
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
        | TypeKind::Native => d_type_name(ast, id),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}
