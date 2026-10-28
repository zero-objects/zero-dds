// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Go backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c`,
//! `runtime/python` and the rest do, and none of them calls another.
//! Where two columns disagree on an octet, one has misread the
//! specification — which is the only reason to have an eighth.
//!
//! # The four decisions this column turns on
//!
//! **A member holds its wire width.** `adapter/go.rs` declares `octet`
//! as `uint8` and `unsigned long long` as `uint64`, so unlike the
//! Python column there is no width to re-state at the call and unlike
//! the Java one there is no signedness to work around: a runtime call
//! takes the member as it is, and the compiler is the range check.
//!
//! **A `char` is `uint8` and a `wchar` is `uint16`.** The generated
//! member holds the code unit, so [`CodecTarget::code_point`] is the
//! identity and a union over a `wchar` compares its discriminator as a
//! number — which is what lets a surrogate case label, refused as a
//! `str` in the Python column, be an ordinary integer here.
//!
//! **A `long double` and a `fixed` are decimal text.** Go has no
//! binary128 and `float64` holds 53 of its 113 significand bits, so
//! `runtime/go`'s `WriteF128` parses the member's decimal — correctly
//! rounded, once — and `ReadF128` answers the exact expansion. The
//! member's type and the codec's agree because both come from
//! `adapter::go::go_value_type`.
//!
//! **An `@optional` member is a pointer, and a pointer needs storage.**
//! `nil` is the absence, so [`CodecTarget::present`] is a nil test and
//! [`CodecTarget::set_present`] writes nothing on the way in — the
//! assignment is what makes it present. The reading direction cannot
//! assign a `T` to a `*T`, and Go has no address-of on a call's result,
//! so [`CodecTarget::optional_destination`] marks the place and
//! [`CodecTarget::read_call`] routes the decoded value through
//! `zdwire.Ptr` — one statement, no temporary whose name two sites in
//! one scope would have to share.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::go::{
    field_name, go_map_entry_name, go_type_name, go_value_type, module_prefix,
};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::go;

/// The Go backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Go;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The name the generated file imports the runtime under.
const RUNTIME: &str = "zdwire";

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`. `emit/encode.rs`'s `map_key_less` writes it where a `map`'s
/// key is a `char` and the entry order is signed; the Go member is a
/// `uint8` and the signed reading is `int8`.
const SIGNED_CHAR: &str = "signed char";

/// An expression, with the one property the walk's seams cannot carry:
/// whether it is the pointer field an `@optional` member lives behind.
///
/// Only [`CodecTarget::optional_destination`] produces the marked form
/// and only [`CodecTarget::read_call`] and [`CodecTarget::store`] act
/// on it; every other constructor passes the inner expression through.
#[derive(Debug, Clone)]
pub enum GoExpr {
    /// An ordinary expression.
    Plain(go::Expr),
    /// The `*T`-typed field of an `@optional` member, as a place a
    /// decoded `T` is about to be written into.
    OptionalSlot(go::Expr),
}

/// The expression inside, whichever form it took.
fn inner(expr: GoExpr) -> go::Expr {
    match expr {
        GoExpr::Plain(e) | GoExpr::OptionalSlot(e) => e,
    }
}

fn plain(e: go::Expr) -> GoExpr {
    GoExpr::Plain(e)
}

impl Go {
    /// A runtime name, qualified: `zdwire.X`.
    fn runtime_name(name: &str) -> go::Expr {
        go::Expr::selector(go::Expr::name(RUNTIME), name)
    }

    /// A runtime operation's Go spelling: `write_u32` is `WriteU32`.
    /// Each `_`-separated segment keeps its own spelling with the first
    /// letter raised, so `read_pid_as` is `ReadPidAs` and
    /// `write_wstring_utf8` is `WriteWstringUtf8` — the names
    /// `runtime/go` publishes, because that runtime was written against
    /// this conversion.
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

    /// A string literal.
    fn text(value: &str) -> GoExpr {
        plain(go::Expr::Literal(Literal::Str(value.to_string())))
    }

    /// A runtime constant by its exported name.
    fn runtime_constant(name: &str) -> GoExpr {
        plain(Self::runtime_name(name))
    }
}

impl CodecTarget for Go {
    type Expr = GoExpr;
    type Stmt = go::Stmt;
    type Block = Vec<go::Stmt>;
    type Type = go::Type;
    type Decl = go::Decl;

    // -- types --------------------------------------------------------

    /// The fixed-width Go type each scalar local takes. `Index` is
    /// `uint32` — a sequence length is a `uint32` on the wire and a Go
    /// index may be any integer type — and `Size` is `int`, which is
    /// what `len` and the reader's positions answer in.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => go::Type::Primitive(go::Primitive::Bool),
            Scalar::Char => go::Type::Primitive(go::Primitive::UInt8),
            Scalar::Int8 => go::Type::Primitive(go::Primitive::Int8),
            Scalar::Int16 => go::Type::Primitive(go::Primitive::Int16),
            Scalar::Int32 => go::Type::Primitive(go::Primitive::Int32),
            Scalar::Int64 => go::Type::Primitive(go::Primitive::Int64),
            Scalar::UInt8 => go::Type::Primitive(go::Primitive::UInt8),
            Scalar::UInt16 => go::Type::Primitive(go::Primitive::UInt16),
            Scalar::UInt32 | Scalar::Index => go::Type::Primitive(go::Primitive::UInt32),
            Scalar::UInt64 => go::Type::Primitive(go::Primitive::UInt64),
            Scalar::Float => go::Type::Primitive(go::Primitive::Float32),
            Scalar::Double => go::Type::Primitive(go::Primitive::Float64),
            // The decimal text; the module documentation says why.
            Scalar::LongDouble => go::Type::Primitive(go::Primitive::String),
            // `int` deliberately: a stream position is what `len`
            // answers, and the CST's own Primitive set excludes the
            // implementation-sized integers because a *declared member*
            // must not change width between targets — a codec local is
            // not a member.
            Scalar::Size => go::Type::named("int"),
        }
    }

    fn named(name: &str) -> Self::Type {
        go::Type::named(name)
    }

    /// No Go form, and never rendered: a Go function with no results
    /// has an empty result list, and [`Go::routine`] writes one.
    fn nothing() -> Self::Type {
        go::Type::named("")
    }

    /// The runtime's handles, by their exported names.
    fn handle(kind: Handle) -> Self::Type {
        go::Type::named(match kind {
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

    /// The runtime's policy values. Strings and integers where
    /// `runtime/go` takes them so — the spellings are
    /// `WstringLengthPolicy::as_str` and its siblings in
    /// `crates/ast-layout/src/key.rs` — and exported constants for the
    /// enumerated kinds. One arm per constant and no catch-all.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => plain(go::Expr::int(2)),
            Constant::WcharWidth4 => plain(go::Expr::int(4)),
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
    /// answer the materialiser's `go_holder` gives its fields.
    fn unsigned_holder(bits: u16) -> Self::Type {
        go::Type::Primitive(crate::adapter::go::go_holder(bits))
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        // Written as the match `unwrap_or` folds away, so the fallback
        // carries its reason where the silent-drop guard reads it: a
        // type with no Go form was refused by the materialiser before a
        // codec walk ran, and `any` here makes the impossible fail in
        // `go build`, loudly, rather than compile into a plausible
        // member.
        #[allow(clippy::manual_unwrap_or_default, clippy::manual_unwrap_or)]
        match go_value_type(ast, id) {
            Ok(type_) => type_,
            Err(_) => go::Type::Primitive(go::Primitive::Any),
        }
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0)
    }

    /// A name the entry declaration and the codec both reach, and a
    /// distinct one per type when the tag walk refuses: the slot keeps
    /// two refused entries from collapsing onto one declaration, which
    /// would be a silent merge of two map types.
    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        go_map_entry_name(ast, id).unwrap_or_else(|_| format!("ZdUnresolvedEntry{}", id.slot()))
    }

    /// The flattened, exported module path — `Common_Util_` — which is
    /// the same prefix the materialiser puts on a package-scope
    /// constant, taken from the same function.
    fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
        module_prefix(ast, id)
    }

    /// Escaped first and exported second, exactly as the materialiser
    /// spells a field: the codec has to reach the member the
    /// declaration wrote.
    fn escape(name: &str) -> String {
        field_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", field_name("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        plain(go::Expr::name(text))
    }

    fn int(value: i128) -> Self::Expr {
        plain(go::Expr::int(value))
    }

    /// The raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten where Go spells them differently — which for `true`
    /// and `false` it does not, but the tree's `Bool` variant is what
    /// renders them and an `Int` carrying a keyword would be pasted as
    /// digits are.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return plain(go::Expr::Literal(value));
        };
        match text.as_str() {
            "true" => plain(go::Expr::Literal(Literal::Bool(true))),
            "false" => plain(go::Expr::Literal(Literal::Bool(false))),
            _ => plain(go::Expr::Literal(value)),
        }
    }

    /// The number, untyped. A Go untyped constant converts to the
    /// operand type it meets, so a union label needs no suffix; the
    /// conversion the walk wraps around it supplies the type where one
    /// is needed.
    fn wide_int(value: i128) -> Self::Expr {
        plain(go::Expr::int(value))
    }

    /// The number, with no `u` suffix: `5u` is not a Go token, and an
    /// untyped constant already converts against a `uint32` counter.
    fn unsigned_int(value: i128) -> Self::Expr {
        plain(go::Expr::int(value))
    }

    /// `uint64(..)`, typed: `^` on an untyped constant is a negative
    /// constant that does not convert to `uint64`, and a mask the walk
    /// complements has to already be one.
    fn wide_unsigned(value: u64) -> Self::Expr {
        plain(go::Expr::Convert {
            type_: Box::new(go::Type::Primitive(go::Primitive::UInt64)),
            value: Box::new(go::Expr::int(i128::from(value))),
        })
    }

    /// `int64(..)`, typed, for the enumerator patterns `zd_raw` is
    /// compared against.
    fn wide_signed(value: i128) -> Self::Expr {
        plain(go::Expr::Convert {
            type_: Box::new(go::Type::Primitive(go::Primitive::Int64)),
            value: Box::new(go::Expr::int(value)),
        })
    }

    /// `v.X` — and `v.D` for the discriminator, whose `_d` no column
    /// shares: Go cannot export a name beginning with an underscore,
    /// and `adapter/go.rs` declares it `D`.
    fn member(base: &str, field: &str) -> Self::Expr {
        let spelled = if field == "_d" { "D" } else { field };
        plain(go::Expr::selector(go::Expr::name(base), spelled))
    }

    fn whole(base: &str) -> Self::Expr {
        plain(go::Expr::name(base))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        plain(go::Expr::selector(inner(value), field))
    }

    /// A header's field, in the exported spelling `runtime/go`
    /// publishes: `member_id` is `MemberId`, `must_understand` is
    /// `MustUnderstand`.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        plain(go::Expr::selector(inner(value), &Self::pascal(field)))
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        plain(go::Expr::Index {
            value: Box::new(inner(value)),
            index: Box::new(inner(at)),
        })
    }

    /// `uint32(len(v))`, or the bound where the materialiser fixed one.
    /// The conversion is the crossing written out: `len` answers an
    /// `int`, a sequence's count is a `uint32` on the wire, and Go has
    /// no implicit conversion between the two.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => plain(go::Expr::int(i128::from(bound))),
            None => plain(go::Expr::Convert {
                type_: Box::new(go::Type::Primitive(go::Primitive::UInt32)),
                value: Box::new(go::Expr::call(
                    go::Expr::name("len"),
                    vec![go::Expr::name(base)],
                )),
            }),
        }
    }

    /// The slice itself: a Go slice is the elements.
    fn elements(base: &str) -> Self::Expr {
        plain(go::Expr::name(base))
    }

    /// `v.X` — the case fields sit directly on the struct, beside `D`.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `entry.Key` and `entry.Value`, the two fields the materialiser
    /// gives a map's entry struct.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        plain(go::Expr::selector(
            inner(entry),
            if key { "Key" } else { "Value" },
        ))
    }

    fn absent() -> Self::Expr {
        plain(go::Expr::Literal(Literal::Null))
    }

    /// Nothing: a Go string and a Go slice own their storage, and there
    /// is no allocation failure for the guard to test.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// `*v.X` — the pointer dereferenced, because the member's codec
    /// takes the value. Only ever reached inside a branch the presence
    /// test guarded.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(go::Expr::Unary {
            op: "*",
            value: Box::new(inner(Self::member(base, field))),
        })
    }

    /// `v.X != nil`.
    fn present(base: &str, field: &str) -> Self::Expr {
        plain(go::Expr::Binary {
            op: "!=",
            lhs: Box::new(inner(Self::member(base, field))),
            rhs: Box::new(go::Expr::Literal(Literal::Null)),
        })
    }

    /// Nothing on the way in — the assignment through `zdwire.Ptr` is
    /// what makes the member present — and `v.X = nil` on the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![go::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: go::Expr::Literal(Literal::Null),
        }]
    }

    /// The marked place, with nothing to do first: the room is made by
    /// [`Go::read_call`], which routes the value through `zdwire.Ptr`.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            GoExpr::OptionalSlot(inner(Self::member(base, field))),
        )
    }

    /// The operators as given: Go spells `&&`, `||` and the comparisons
    /// the way the walk does. `=` never reaches a rendered expression —
    /// [`Go::perform`] turns it into an assignment statement.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        plain(go::Expr::Binary {
            op,
            lhs: Box::new(inner(lhs)),
            rhs: Box::new(inner(rhs)),
        })
    }

    /// `!a`, and `^a` where the walk wrote C's `~`: Go's bitwise
    /// complement is the unary `^`.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        plain(go::Expr::Unary {
            op: match op {
                "~" => "^",
                other => other,
            },
            value: Box::new(inner(value)),
        })
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        plain(go::Expr::Paren(Box::new(inner(value))))
    }

    /// `!a && b`: Go defines `<` on ordered types only, and `bool` is not
    /// one ("operator < not defined on bool", measured in the matrix
    /// refresh of 2026-10-01 on `map-key-boolean` under the Fast DDS and
    /// OpenDDS dialects, which order map entries by key). `false` before
    /// `true` is [`zerodds_ast_layout::MapKeyOrdering::UnsignedInteger`]'s
    /// order for a `boolean`.
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        Self::binary("&&", Self::unary("!", Self::paren(left)), right)
    }

    /// `T(v)` — a Go conversion, with two rewrites.
    ///
    /// A `bool` discriminator's labels arrive as integers and
    /// `bool(1)` is not a Go conversion; the label becomes the literal.
    /// And the walk's `signed char` — the C spelling of a map key's
    /// signed order — is `int8` here, which reads a `uint8` key as the
    /// two's complement the corpus was recorded under.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == go::Type::named(SIGNED_CHAR) {
            return plain(go::Expr::Convert {
                type_: Box::new(go::Type::Primitive(go::Primitive::Int8)),
                value: Box::new(inner(value)),
            });
        }
        if type_ == go::Type::Primitive(go::Primitive::Bool) {
            if let GoExpr::Plain(go::Expr::Literal(Literal::Int(text))) = &value {
                return plain(go::Expr::Literal(Literal::Bool(text != "0")));
            }
        }
        plain(go::Expr::Convert {
            type_: Box::new(type_),
            value: Box::new(inner(value)),
        })
    }

    /// The value, unchanged: Go hands a slice by header, an array by
    /// value and a struct by value, and the codec's parameter says
    /// which.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// A call to a package-level function — a generated codec or
    /// defaulter. `strcmp` is the one name the walk hands here that is
    /// not one; it is the runtime's `Strcmp`, which spells C11
    /// §7.24.4's unsigned-octet order in the language whose own string
    /// order is exactly that.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let arguments: Vec<go::Expr> = args.into_iter().map(inner).collect();
        if callee == "strcmp" {
            return plain(go::Expr::call(Self::runtime_name("Strcmp"), arguments));
        }
        plain(go::Expr::call(go::Expr::name(callee), arguments))
    }

    /// `w.WriteU32(x)` — a method on the writer or the reader.
    ///
    /// One operation is renamed rather than converted: the walk's
    /// `read_pid()` reads a header **and** its value block is opened by
    /// the later `finish_pid_read`, which is `ReadPidAs` in this
    /// runtime — §7.4.1.2 makes the block a wire question, so the call
    /// may not be left as it stands. The Python column does the same.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "read_pid" && args.is_empty() {
            return Self::runtime(
                receiver,
                "read_pid_as",
                vec![Self::constant(Constant::ParameterMember)],
            );
        }
        plain(go::Expr::call(
            go::Expr::selector(go::Expr::name(receiver), &Self::pascal(method)),
            args.into_iter().map(inner).collect(),
        ))
    }

    /// `runtime/go` carries the three HDDS axes: `BeginPackedElement`
    /// and `FinishPackedElement` on the writer and
    /// `ReaderBeginPackedElement` / `ReaderFinishPackedElement` on the
    /// reader, the DHEADER pair for a sequence element's own length, and
    /// `WriteWstringUtf8Octets` / `WriteWstringUtf8OctetsBounded` /
    /// `ReadWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// A free function of the runtime — `zdwire.NoDheader()`,
    /// `zdwire.Utf8Octets(v)` — except `pid_is_user_member`, which is a
    /// method on the header the reader handed back, the same difference
    /// as C's free function against Java's static.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "pid_is_user_member" {
            let mut arguments = args.into_iter().map(inner);
            if let Some(header) = arguments.next() {
                return plain(go::Expr::call(
                    go::Expr::selector(header, "IsUserMember"),
                    Vec::new(),
                ));
            }
            // The walk always hands the header; a call without one is a
            // walk that changed under this file, and a method on a name
            // no scope declares is the loudest thing Go can say here.
            return plain(go::Expr::call(
                Self::runtime_name("IsUserMember"),
                Vec::new(),
            ));
        }
        plain(go::Expr::call(
            Self::runtime_name(&Self::pascal(method)),
            args.into_iter().map(inner).collect(),
        ))
    }

    // -- statements ----------------------------------------------------

    /// `var name T = value`. The type is written because the walk knows
    /// it and a reader of the generated file then sees the width — and
    /// because a `var` with a type is definite whether or not the value
    /// is there, which the header handle declared before a loop needs.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        go::Stmt::Var {
            name: Ident::new(name),
            type_: Some(type_),
            value: value.map(inner),
        }
    }

    /// An expression evaluated for its effect — or, where the walk
    /// built an assignment, the assignment: `a = b` is a statement in
    /// Go and not an expression, and this is the only place the walk's
    /// `=` arrives.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let GoExpr::Plain(go::Expr::Binary { op: "=", lhs, rhs }) = value {
            return go::Stmt::Assign {
                place: *lhs,
                value: *rhs,
            };
        }
        go::Stmt::Expr(inner(value))
    }

    /// `return v`: a Go decoder answers with the sample it built, so a
    /// bare `return` — C's shape, for a decoder that wrote through a
    /// pointer — would answer nothing at all and not compile.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        go::Stmt::Return(Some(match value {
            Some(value) => inner(value),
            None => go::Expr::name("v"),
        }))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        go::Stmt::If {
            condition: inner(condition),
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// `for i := uint32(0); i < bound; i++`. The counter is a `uint32`
    /// because every bound the walk hands over is one — a count off the
    /// wire, `uint32(len(v))`, or an untyped constant that converts.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        go::Stmt::For {
            counter: Ident::new(counter),
            init: Box::new(go::Expr::Convert {
                type_: Box::new(go::Type::Primitive(go::Primitive::UInt32)),
                value: Box::new(go::Expr::int(0)),
            }),
            bound: Box::new(inner(bound)),
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        go::Stmt::While {
            condition: inner(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        go::Stmt::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        go::Stmt::Comment(doc)
    }

    /// `_ = expr`: Go has no `(void)` cast and a bare call is already a
    /// statement, but a *value* read for its effect — a discarded
    /// DHEADER — has to say it meant the discard, and the blank
    /// identifier is Go's spelling of exactly that.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        go::Stmt::Assign {
            place: go::Expr::name("_"),
            value: inner(value),
        }
    }

    /// `_ = name`: Go rejects a local that is assigned and never read
    /// — "declared and not used" is a compile error, not a warning —
    /// and `zd_sent` under a framing that writes no sentinel is exactly
    /// that local.
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![go::Stmt::Assign {
            place: go::Expr::name("_"),
            value: go::Expr::name(name),
        }]
    }

    /// `place = zd_dec_T(r, behaviour)` — and through `zdwire.Ptr`
    /// where the place is an `@optional` member's pointer field: the
    /// decoder answers a `T`, the field holds a `*T`, and Go has no
    /// address-of on a call's result.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let decoded = go::Expr::call(
            go::Expr::name(callee),
            vec![go::Expr::name("r"), inner(behaviour)],
        );
        match place {
            GoExpr::OptionalSlot(field) => go::Stmt::Assign {
                place: field,
                value: go::Expr::call(Self::runtime_name("Ptr"), vec![decoded]),
            },
            GoExpr::Plain(place) => go::Stmt::Assign {
                place,
                value: decoded,
            },
        }
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

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// `v = make([]T, n)`, and the fill after it: a Go slice's slots
    /// are storage the moment `make` returns, for a sequence's whole
    /// elements and for a map entry's two halves alike, so
    /// `assigned_whole` makes no difference here.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![go::Stmt::Assign {
            place: go::Expr::name(base),
            value: go::Expr::Make {
                type_: Box::new(go::Type::slice(element)),
                length: Box::new(inner(count)),
            },
        }];
        out.extend(fill);
        out
    }

    /// `v = v[:count]` — §7.3.1.2.1.12.3's TRIM, applied after every
    /// element has been read.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![go::Stmt::Assign {
            place: go::Expr::name(base),
            value: go::Expr::SliceTo {
                value: Box::new(go::Expr::name(base)),
                high: Box::new(inner(count)),
            },
        }]
    }

    /// The subscript, with nothing around it: a Go slice element is a
    /// real place at every element type — there is no
    /// `std::vector<bool>` here.
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

    /// `func Zd_enc_T(w *zdwire.Writer, v T)` and
    /// `func Zd_dec_T(r *zdwire.Reader, zd_tc zdwire.TryConstruct) T`.
    /// The upper-case first letter is Go's visibility; see
    /// [`exported_name`].
    ///
    /// A forward declaration is answered with nothing: Go resolves a
    /// package-level name wherever it is declared, so a recursive type
    /// needs no declaration ahead of its body.
    /// Go's visibility is its spelling; see [`exported_name`].
    fn published_name(name: String) -> String {
        exported_name(&name)
    }

    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![go::Param {
            name: Some(Ident::new(if spec.writing { "w" } else { "r" })),
            type_: go::Type::pointer(go::Type::named(if spec.writing {
                "zdwire.Writer"
            } else {
                "zdwire.Reader"
            })),
        }];
        if spec.writing {
            params.push(go::Param {
                name: Some(Ident::new("v")),
                type_: spec.sample.clone(),
            });
        }
        for (type_, name) in spec.extra {
            params.push(go::Param {
                name: Some(Ident::new(&name)),
                type_,
            });
        }
        let (results, body) = if spec.writing {
            (Vec::new(), body)
        } else {
            // The sample starts as Table 9's value, through the
            // generated defaulter, so every position the stream does
            // not carry is already what the clause says it is — the
            // same shape the Python column's `reading_body` writes.
            let defaulter = crate::default_go::defaulter_from_decoder(&spec.name);
            let mut statements = vec![go::Stmt::Var {
                name: Ident::new("v"),
                type_: None,
                value: Some(go::Expr::call(go::Expr::name(&defaulter), Vec::new())),
            }];
            statements.extend(body);
            statements.push(go::Stmt::Return(Some(go::Expr::name("v"))));
            (vec![spec.sample], statements)
        };
        Some(go::Decl::Func(go::FuncDecl {
            doc: spec.doc,
            // Already the published spelling: `codec_name` and
            // `decoder_name` put it through [`Go::published_name`]
            // before the walk ever names the routine.
            name: Ident::new(&spec.name),
            params,
            results,
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
        Ok(go::Stmt::Assign {
            place: inner(place),
            value: go::Expr::call(
                go::Expr::name(&crate::default_go::defaulter_name(ast, id)),
                Vec::new(),
            ),
        })
    }

    /// # Errors
    /// As [`crate::default_go::member_default`]: a `@default` this
    /// column cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_go::member_default(ast, node, type_, inner(place))
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
/// generated Go held no `zd_over` check at all, so `a` accepted a
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

/// The name a codec function takes for the type it codes.
///
/// The IDL kind and not the Go spelling, for the reason the Python
/// column's `type_tag` gives at length: `octet` and `uint8` are one Go
/// type and one wire form here, but `double` and `long double` are
/// `float64` and `string`, and a tag built from the Go type would give
/// a `fixed<5,2>` and a `wstring` one codec. A declared type takes its
/// flattened Go name, which two IDL declarations cannot share —
/// `adapter/go.rs`'s `remember` refuses the collision before a codec is
/// generated.
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
                    // reports the dimension that would not evaluate by
                    // name.
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
        | TypeKind::Native => go_type_name(ast, id),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}

/// A generated name, in the form Go makes visible outside its package.
///
/// Go has no visibility keyword: an identifier is exported if and only
/// if its first character is an upper-case letter (Go specification,
/// "Exported identifiers"). Every generated codec, defaulter and key
/// holder was named `zd_enc_…`, `zd_def_…`, `zd_kenc_…` — all
/// lower-case — so a Go program that imported the generated package
/// could not call a single one of them. The probe harness never found
/// out: it writes its `main` into the same package.
///
/// **The first character only.** `ZdEncMax` would be the idiomatic
/// spelling and it is not injective — dropping the underscores maps two
/// different generated names onto one, and two codecs under one name is
/// a collision no compiler would report as one, because the second
/// declaration would simply be a redeclaration error somewhere else or,
/// worse, not an error at all. Raising the first letter is a bijection
/// on this name set: every generated name begins `zd`, and no two
/// differ only in that letter.
///
/// Names that are not generated pass through: the runtime's are reached
/// through `Go::runtime_name` and never arrive here, and `strcmp` is
/// intercepted by [`Go::call`] before this is asked.
pub fn exported_name(name: &str) -> String {
    // Idempotent: a name already raised passes through, so a second
    // application — `defaulter_name` calling this on a name built from
    // an already-published decoder — cannot raise anything twice or
    // produce a second spelling of one function.
    if !name.starts_with("zd_") {
        return name.to_string();
    }
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
