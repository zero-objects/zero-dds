// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Node (plain JavaScript) backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c`,
//! `runtime/typescript` and the rest do, and none of them calls another.
//! Where two columns disagree on an octet, one has misread the
//! specification — which is the only reason to have a second column.
//!
//! # The value model, kept from TypeScript
//!
//! Plain JavaScript and TypeScript share one runtime: `number` is an
//! IEEE 754 binary64 and `bigint` is exact and unbounded, which is a
//! fact about the *language*, not about its type checker. So this
//! column draws the same boundary [`crate::emit::typescript::TypeScript::scalar`]
//! does — every integer to `i32` and every `f32`/`f64` is a `number`, a
//! `long long` and an `unsigned long long` are `bigint`, a `char` and a
//! `wchar` are a one-character `string`, and a `long double` is
//! `{ octets: Uint8Array(16) }` and never a `number`, because a binary64
//! would drop sixty bits of its significand at the assignment. Deriving
//! a different model from [`crate::emit::zig`] or [`crate::emit::lua`]
//! instead — a `char` as a code-point integer — would be answering a
//! question the language has already answered the other way, through no
//! choice this column makes.
//!
//! # Where this column is simpler than TypeScript's, and why
//!
//! Every one of these follows from having no compiler holding a
//! `number`, a `bigint` and a discriminated union's literal types apart
//! — not from skipping a check TypeScript's runtime still needs:
//!
//! - **No union-arm narrowing.** [`crate::emit::typescript::TypeScript::case_member`]
//!   builds `(v as Extract<typeof v, { one: unknown }>).one` because
//!   `tsc` refuses `v.one` on a value typed as the whole discriminated
//!   union. There is no such refusal here: [`Node::case_member`] is
//!   `v.one`, exactly as [`crate::emit::lua::Lua::case_member`] is
//!   `v.one` for the same reason.
//! - **No literal-type rewriting.** TypeScript's `cast` rewrites a union
//!   label's literal so its *type* matches what `===` narrows on;
//!   nothing here narrows a type, so a label is the value the walk
//!   handed over, in the holder [`Node::cast`] already puts it in.
//! - **No `noUncheckedIndexedAccess` assertion.** [`Node::index`] is
//!   `v[i]`, not `v[i]!` — that operator does not parse outside
//!   TypeScript, and a plain JavaScript array answers `v[i]` the same
//!   way regardless of what a type checker would have said about it.
//! - **`@external` needs nothing.** Every object this column builds is
//!   already a reference — there is no pointer to take and no allocator
//!   to route it through, which is also why the trait's defaults for
//!   every `external_*` seam are left untouched, exactly as
//!   [`crate::emit::typescript`] leaves them.
//!
//! # Where this column still needs TypeScript's coercion functions
//!
//! [`Node::cast`] still calls {@link zdNumber}, {@link zdBigInt} and
//! {@link zdChar} by name, and [`Node::store`] still wraps every scalar
//! read in [`Node::cast`] before it reaches its member. Not because
//! plain JavaScript refuses a mismatched assignment — it does not — but
//! because the shared decode walk's `primitive_read_at` casts a `wchar`
//! read to [`crate::emit::Scalar::UInt16`] before this seam ever sees
//! it, which for a column that holds `wchar` in a `string` is the wrong
//! holder for the member; [`Node::store`]'s cast is what turns it back.
//! The exact defect this override closes is `crates/lang-rules/src/emit/typescript.rs`'s
//! `store` documentation, verified afresh here rather than assumed: this
//! runtime's `readWchar` and `writeWchar` are one-character strings top
//! to bottom, and the walk's hard-coded `UInt16` narrowing sits between
//! two of the four call sites regardless of which column reads it.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::node::field_name;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::node::{self, Expr, Function, Stat};

/// The Node backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The name the generated file requires the runtime under:
/// `const zdwire = require("./zdwire.js");`.
const RUNTIME: &str = "zdwire";

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`; the member is a one-character `string` and the signed
/// reading is the two's-complement arithmetic [`Node::cast`] writes out.
const SIGNED_CHAR: &str = "signed char";

/// A JavaScript holder, carried alongside the tree so [`Node::cast`] and
/// [`Node::store`] know which of the three coercions — or none — a value
/// needs on its way into a place.
///
/// Not a type in any sense the language enforces: plain JavaScript
/// checks nothing here, and every field but the tag itself is
/// unreachable at run time. It exists for the same reason
/// [`crate::emit::lua::LuaType`] does — "the nominal type a local is
/// annotated with, which in Lua is nothing at all" — with `"boolean"`,
/// `"number"`, `"bigint"` and `"string"` as the tags [`Node::cast`] acts
/// on and every other tag passed through unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeType(pub String);

impl NodeType {
    fn tag(text: &str) -> Self {
        Self(text.to_string())
    }
}

impl Node {
    /// `write_u32` as `writeU32`.
    fn camel(method: &str) -> String {
        let mut out = String::new();
        let mut rising = false;
        for character in method.chars() {
            if character == '_' {
                rising = true;
                continue;
            }
            if rising {
                out.extend(character.to_uppercase());
                rising = false;
            } else {
                out.push(character);
            }
        }
        out
    }

    /// A dotted path off the runtime: `zdwire.WcharWidth.Octets2`.
    fn runtime_name(path: &str) -> Expr {
        let mut segments = path.split('.');
        let mut expr = Expr::member(Expr::name(RUNTIME), segments.next().unwrap_or(path));
        for segment in segments {
            expr = Expr::member(expr, segment);
        }
        expr
    }

    /// A reader's body, with the sample it answers with around it:
    /// `let v = zd_def_T(); … return v;`. Table 9's value through the
    /// generated defaulter, so every position the stream does not carry
    /// is already what the clause says before an octet is read.
    fn reading_body(name: &str, body: Vec<Stat>) -> Vec<Stat> {
        let defaulter = crate::default_node::defaulter_from_decoder(name);
        let mut statements = vec![Stat::Declare {
            doc: Doc::none(),
            kind: node::VarKind::Let,
            name: Ident::new("v"),
            value: Some(Expr::call(&defaulter, Vec::new())),
        }];
        statements.extend(body);
        statements.push(Stat::Return(Some(Expr::name("v"))));
        statements
    }
}

impl CodecTarget for Node {
    type Expr = Expr;
    type Stmt = Stat;
    type Block = Vec<Stat>;
    type Type = NodeType;
    type Decl = Stat;

    // -- types --------------------------------------------------------

    /// `number` to `i32`, `bigint` beyond it, a one-character `string`
    /// for a `char` — the boundary [`crate::emit::typescript::TypeScript::scalar`]
    /// draws, kept because it is the language's own and not TypeScript's.
    fn scalar(kind: Scalar) -> Self::Type {
        NodeType::tag(match kind {
            Scalar::Bool => "boolean",
            Scalar::Char => "string",
            Scalar::Int8
            | Scalar::UInt8
            | Scalar::Int16
            | Scalar::UInt16
            | Scalar::Int32
            | Scalar::UInt32
            | Scalar::Index
            | Scalar::Size
            | Scalar::Float
            | Scalar::Double => "number",
            Scalar::Int64 | Scalar::UInt64 => "bigint",
            Scalar::LongDouble => "float128",
        })
    }

    /// The tag, preserved exactly: this is how [`Node::cast`] recognises
    /// `"signed char"`, and how a call site that names a type by its own
    /// spelling is answered honestly rather than folded into a generic
    /// tag that would hide the collision.
    fn named(name: &str) -> Self::Type {
        NodeType::tag(name)
    }

    fn nothing() -> Self::Type {
        NodeType::tag("nothing")
    }

    /// Every runtime handle is a plain object; nothing casts one.
    fn handle(_kind: Handle) -> Self::Type {
        NodeType::tag("object")
    }

    /// The member already is the code unit — a one-character `string`
    /// for both a `char` and a `wchar` — so there is nothing to convert.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy tables, in `runtime/node/zdwire.js`'s own
    /// spelling — the same strings `runtime/typescript`'s enums answer
    /// with, since both runtimes were written against one clause and
    /// carry the measurements in `crates/ast-layout/src/key.rs` under
    /// one name apiece.
    fn constant(which: Constant) -> Self::Expr {
        Self::runtime_name(match which {
            Constant::WcharWidth2 => "WcharWidth.Octets2",
            Constant::WcharWidth4 => "WcharWidth.Octets4",
            Constant::WcharFixed => "WcharFraming.Fixed",
            Constant::WcharGiopLengthPrefixed => "WcharFraming.GiopLengthPrefixed",
            Constant::WstringUnitsStream => "WstringUnitOrder.Stream",
            Constant::WstringUnitsBigEndianAlways => "WstringUnitOrder.BigEndianAlways",
            Constant::WstringUnitsByteOrderMark => "WstringUnitOrder.ByteOrderMark",
            Constant::WstringUnitsHostUnswapped => "WstringUnitOrder.HostUnswapped",
            Constant::WstringSpecification => "WstringLengthPolicy.Specification",
            Constant::WstringCharacterCount => "WstringLengthPolicy.CharacterCount",
            Constant::WstringCharacterCountTerminated => {
                "WstringLengthPolicy.CharacterCountTerminated"
            }
            Constant::WstringSpecificationWideUnits => "WstringLengthPolicy.SpecificationWideUnits",
            Constant::WstringRosTypeSupport => "WstringLengthPolicy.RosTypeSupport",
            Constant::WstringOctetsTerminated2 => "WstringLengthPolicy.OctetsTerminated2",
            Constant::WstringOctetsTerminated4 => "WstringLengthPolicy.OctetsTerminated4",
            Constant::WstringCharactersTerminated2 => "WstringLengthPolicy.CharactersTerminated2",
            Constant::FixedDeclared => "FixedDigitCount.Declared",
            Constant::FixedValuePrecision => "FixedDigitCount.ValuePrecision",
            Constant::FixedValueStringDigits => "FixedDigitCount.ValueStringDigits",
            Constant::ParameterMember => "ParameterRole.Member",
            Constant::ParameterOptional => "ParameterRole.Optional",
            Constant::TryConstructDiscard => "TryConstruct.Discard",
            Constant::TryConstructTrim => "TryConstruct.Trim",
            Constant::TryConstructUseDefault => "TryConstruct.UseDefault",
            Constant::StatusOk => "Status.Ok",
            Constant::StatusDiscarded => "Status.Discarded",
            Constant::StatusImpossibleLength => "Status.ImpossibleLength",
            Constant::StatusOutOfRange => "Status.OutOfRange",
            Constant::StatusTruncated => "Status.Truncated",
        })
    }

    /// `number` to 32 bits, `bigint` at 64 — the same boundary
    /// [`Node::scalar`] draws for every other integer.
    fn unsigned_holder(bits: u16) -> Self::Type {
        NodeType::tag(if bits > 32 { "bigint" } else { "number" })
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        value_type_of(ast, id, 0)
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0)
    }

    fn map_entry_name(_ast: &Ast, _id: Id<TypeNode>) -> String {
        crate::adapter::node::MAP_ENTRY.to_string()
    }

    /// Nothing: the declared paths this backend writes go through
    /// [`Node::enumerator_constant`], which flattens the whole scoped
    /// name into the one binding it names, so a prefix here would state
    /// the same thing twice.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// The identity, always — [`zerodds_lang_cst::node::escape_member`]'s
    /// documentation carries why.
    fn escape(name: &str) -> String {
        field_name(name)
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        Expr::name(text)
    }

    fn int(value: i128) -> Self::Expr {
        Expr::int(value)
    }

    /// The raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten into the tree's own boolean where they are one.
    fn literal(value: Literal) -> Self::Expr {
        if let Literal::Int(text) = &value {
            match text.as_str() {
                "true" => return Expr::bool(true),
                "false" => return Expr::bool(false),
                _ => {}
            }
        }
        Expr::Literal(value)
    }

    /// A `bigint` literal: the number with a trailing `n`. Every value
    /// the walk hands here is one a `number` cannot carry exactly, so
    /// the suffix is unconditional.
    fn wide_int(value: i128) -> Self::Expr {
        Expr::Literal(Literal::Int(format!("{value}n")))
    }

    /// The number, with no suffix: JavaScript has one numeric literal
    /// grammar and no unsigned spelling — `2u` is not a token.
    fn unsigned_int(value: i128) -> Self::Expr {
        Expr::int(value)
    }

    /// `123n`, [`Node::wide_int`]'s spelling: the trait's default is C's
    /// `UINT64_C(123)`, a macro no JavaScript file declares.
    fn wide_unsigned(value: u64) -> Self::Expr {
        Expr::Literal(Literal::Int(format!("{value}n")))
    }

    /// As [`Node::wide_unsigned`]: a `bigint` is unbounded, so a
    /// negative pattern is written as the negative number it is.
    fn wide_signed(value: i128) -> Self::Expr {
        Expr::Literal(Literal::Int(format!("{value}n")))
    }

    /// A field of a header the runtime handed back, in the runtime's own
    /// spelling — the same camelCase table
    /// `crates/lang-rules/src/emit/typescript.rs`'s override carries,
    /// because `runtime/node/zdwire.js`'s headers are plain objects with
    /// the same property names as `runtime/typescript`'s.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        Self::field_of(
            value,
            match field {
                "pid" => "pid",
                "end" => "end",
                "ignore" => "ignore",
                "length" => "length",
                "member_id" => "memberId",
                "must_understand" => "mustUnderstand",
                "extent" => "extent",
                other => other,
            },
        )
    }

    /// `strcmp` is the walk's name for a three-way code-unit comparison;
    /// JavaScript's `</>` on strings already compare UTF-16 code units
    /// (ECMA-262 §7.2.13), so the spelling is a ternary — the same one
    /// `crates/lang-rules/src/emit/typescript.rs` writes, and the Java
    /// column's caveat about supplementary code points applies
    /// unchanged.
    ///
    /// A ternary and not the shorter `(l < r) - (l > r)`: subtracting
    /// two relational results goes through `Boolean`'s numeric
    /// coercion, where `true - false` is `1` (ECMA-262 §7.1.4), so
    /// that form returns `+1` for the case C11 §7.24.4 gives `-1`.
    /// The walk reads the result as `strcmp(..) < 0`, so the inverted
    /// sign puts a `map`'s entries on the wire in descending key
    /// order.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            if let [left, right] = args.as_slice() {
                return Expr::Paren(Box::new(Expr::Ternary {
                    condition: Box::new(Expr::Binary {
                        op: "<",
                        lhs: Box::new(left.clone()),
                        rhs: Box::new(right.clone()),
                    }),
                    then: Box::new(Self::int(-1)),
                    otherwise: Box::new(Expr::Ternary {
                        condition: Box::new(Expr::Binary {
                            op: ">",
                            lhs: Box::new(left.clone()),
                            rhs: Box::new(right.clone()),
                        }),
                        then: Box::new(Self::int(1)),
                        otherwise: Box::new(Self::int(0)),
                    }),
                }));
            }
        }
        Expr::call(callee, args)
    }

    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        Expr::Call {
            callee: Box::new(Expr::member(Expr::name(receiver), &Self::camel(method))),
            args,
        }
    }

    /// `runtime/node` carries the three HDDS axes: `beginPackedElement` and
    /// `finishPackedElement` on the writer and the reader, the DHEADER pair
    /// for a sequence element's own length, and `writeWstringUtf8Octets` /
    /// `readWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// A free function the generated file reaches through the runtime's
    /// own object: `zdwire.<method>(args)`.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        Expr::Call {
            callee: Box::new(Expr::member(Expr::name(RUNTIME), &Self::camel(method))),
            args,
        }
    }

    /// A field name with dots is a path through the members that hold a base
    /// (`parent.x`, XTypes 1.3 §7.5.1.4.1.3): one member step per segment.
    fn member(base: &str, field: &str) -> Self::Expr {
        let mut place = Expr::name(base);
        for segment in field.split('.') {
            place = Expr::member(place, segment);
        }
        place
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some("parent.".repeat(depth))
    }

    fn whole(base: &str) -> Self::Expr {
        Expr::name(base)
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        Expr::member(value, field)
    }

    /// `v[i]` — a plain subscript; there is no `noUncheckedIndexedAccess`
    /// assertion to write outside TypeScript.
    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        Expr::Index {
            object: Box::new(value),
            index: Box::new(at),
        }
    }

    /// `.length`, or the bound where the materialiser fixed one.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => Expr::int(i128::from(bound)),
            None => Expr::member(Expr::name(base), "length"),
        }
    }

    fn elements(base: &str) -> Self::Expr {
        Expr::name(base)
    }

    /// `v.one` — a union's branches sit beside `_d` on one plain object;
    /// nothing here needs the narrowing
    /// [`crate::emit::typescript::TypeScript::case_member`]'s `Extract<>`
    /// exists for, because nothing checks the type before the property
    /// is read.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        Expr::member(entry, if key { "key" } else { "value" })
    }

    fn absent() -> Self::Expr {
        Expr::Undefined
    }

    /// Nothing: every string and array this runtime materialises either
    /// exists or the read has already thrown — there is no allocator
    /// that can hand back an absent one for a codec to notice, exactly
    /// as `crates/lang-rules/src/emit/lua.rs`'s answer states for Lua's
    /// tables.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// The member itself: `v.x` is the value or the key is not there,
    /// and there is no wrapper to unwrap.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    fn present(base: &str, field: &str) -> Self::Expr {
        Expr::Binary {
            op: "!==",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        }
    }

    /// `delete v.field` for absence, and nothing for presence — writing
    /// the member is what makes it present, exactly as
    /// `crates/lang-rules/src/emit/typescript.rs`'s override reads;
    /// [`crate::default_node`] writes the same shape for the same
    /// clause.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![Stat::Expr(Expr::Unary {
            op: "delete ",
            value: Box::new(Self::member(base, field)),
        })]
    }

    /// `delete v.field` for every case: `adapter/node.rs` puts a union's
    /// branches beside `_d` on one plain object, and a branch the
    /// discriminator does not select is a property that is not there. The same statement as an absent
    /// `@optional` member, [`Self::set_present`] with `false`.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .flat_map(|field| Self::set_present(base, field, false))
            .collect()
    }

    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    /// `==` becomes `===` and `!=` becomes `!==`.
    ///
    /// Not house style: JavaScript's loose equality coerces across
    /// `number` and `bigint` inconsistently — `1 == 1n` is `true` and
    /// `1 === 1n` is `false` — and a discriminator compared with the
    /// loose form could match a label of the wrong holder. `&&` and
    /// `||` are already this language's own spelling and pass through
    /// unchanged, unlike [`crate::emit::lua::Lua::binary`]'s `and`/`or`.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        Expr::Binary {
            op: match op {
                "==" => "===",
                "!=" => "!==",
                other => other,
            },
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// Every operator as given: JavaScript has a real unary form for
    /// both `!` and `~`, so neither needs a stand-in.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        Expr::Unary {
            op,
            value: Box::new(value),
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        Expr::Paren(Box::new(value))
    }

    /// A conversion where the target is one of the three scalar holders
    /// or the signed-`char` marker, a rewritten literal where the value
    /// is a boolean discriminator's, and the value unchanged everywhere
    /// else — there is no `as` to fall back on and none is needed: a
    /// value this seam does not name a coercion for already carries the
    /// representation its destination wants.
    ///
    /// `"signed char"` is the walk's own reading of a `map`'s `char` key
    /// order: the member holds the code point as a `string`, so
    /// `((k.charCodeAt(0) + 128) % 256) - 128` is the two's-complement
    /// wrap over its one octet — the same arithmetic
    /// `crates/lang-rules/src/emit/python.rs`'s `signed_char` writes and
    /// for the same reason.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if let Expr::Literal(Literal::Int(text)) = &value {
            if type_.0 == "boolean" {
                let digits = text.strip_suffix('n').unwrap_or(text);
                if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
                    return Expr::bool(digits != "0");
                }
            }
        }
        if type_.0 == SIGNED_CHAR {
            let point = Expr::Call {
                callee: Box::new(Expr::member(Expr::Paren(Box::new(value)), "charCodeAt")),
                args: vec![Self::int(0)],
            };
            return Self::binary(
                "-",
                Expr::Paren(Box::new(Self::binary(
                    "%",
                    Expr::Paren(Box::new(Self::binary("+", point, Self::int(128)))),
                    Self::int(256),
                ))),
                Self::int(128),
            );
        }
        let coercion = match type_.0.as_str() {
            "number" => Some("zdNumber"),
            "bigint" => Some("zdBigInt"),
            "string" => Some("zdChar"),
            _ => None,
        };
        match coercion {
            Some(callee) if is_call_to(&value, callee) => value,
            Some(callee) => Self::runtime_static(callee, vec![value]),
            None => value,
        }
    }

    /// The same conversion, at the point a value reaches its member: the
    /// module documentation says why a `wchar` needs it here as well as
    /// at [`Node::cast`]'s own call sites.
    fn store(place: Self::Expr, type_: Self::Type, value: Self::Expr) -> Self::Stmt {
        Self::perform(Self::binary("=", place, Self::cast(type_, value)))
    }

    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// `place = zd_dec_T(r, zd_tc);` — this column's decoders answer
    /// with the value, so the destination is the assignment's left-hand
    /// side and never an out-parameter.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        Self::perform(Self::binary(
            "=",
            place,
            Self::call(callee, vec![Self::name("r"), behaviour]),
        ))
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

    // -- statements ----------------------------------------------------

    /// `let name = value;`, with an integer literal spelled the way the
    /// declared holder reads it — `let bits = 0n;` for a `bigint`
    /// accumulator, exactly as the walk's own
    /// `T::declare(T::scalar(Scalar::UInt64), "bits", int(0))` for a
    /// bitset needs.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        let value = value.map(|value| match &value {
            Expr::Literal(Literal::Int(text)) if type_.0 == "bigint" && !text.ends_with('n') => {
                Expr::Literal(Literal::Int(format!("{text}n")))
            }
            _ => value,
        });
        Stat::Declare {
            doc: Doc::none(),
            kind: node::VarKind::Let,
            name: Ident::new(name),
            value,
        }
    }

    fn perform(value: Self::Expr) -> Self::Stmt {
        Stat::Expr(value)
    }

    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        Stat::Return(Some(value.unwrap_or_else(|| Self::whole("v"))))
    }

    /// Nothing: plain JavaScript has no unread-parameter diagnostic, and
    /// `zd_tc;` is not worth writing on its own.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// `void v;` — the one form JavaScript has for "read and discard",
    /// and the only place the walk needs it.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        Stat::Expr(Expr::Unary {
            op: "void ",
            value: Box::new(value),
        })
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        Stat::If {
            condition,
            then,
            otherwise,
        }
    }

    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        Stat::For {
            counter: Ident::new(counter),
            bound,
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        Stat::While { condition, body }
    }

    fn leave() -> Self::Stmt {
        Stat::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        Stat::Comment(doc)
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// `v = new Array(count);`, and — for a `map`, whose entries are
    /// written half at a time — one `{}` per slot ahead of the fill,
    /// because `v[i].key = …` needs the slot object to exist first. A
    /// sequence's element decoder assigns the slot outright and needs
    /// only the outer array.
    fn make_room(
        base: &str,
        _element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![Stat::Assign {
            doc: Doc::none(),
            place: Self::whole(base),
            value: Expr::New {
                callee: Ident::new("Array"),
                args: vec![count],
            },
        }];
        if !assigned_whole {
            out.push(Self::count_up(
                "zd_e",
                Self::count_of(base, None),
                vec![Stat::Assign {
                    doc: Doc::none(),
                    place: Expr::Index {
                        object: Box::new(Self::whole(base)),
                        index: Box::new(Self::whole("zd_e")),
                    },
                    value: Expr::Object(Vec::new()),
                }],
            ));
        }
        out.extend(fill);
        out
    }

    /// `let name = {};` — somewhere to put a value before its halves
    /// arrive, so `zd_scratch.key = …` does not assign onto `undefined`.
    fn storage(_type_: Self::Type, name: &str) -> Self::Stmt {
        Stat::Declare {
            doc: Doc::none(),
            kind: node::VarKind::Let,
            name: Ident::new(name),
            value: Some(Expr::Object(Vec::new())),
        }
    }

    /// `v.length = n;`, which shortens an array in place. §7.3.1.2.1.12.3's
    /// TRIM.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![Stat::Assign {
            doc: Doc::none(),
            place: Expr::member(Self::whole(base), "length"),
            value: count,
        }]
    }

    /// The subscript, with nothing around it: a JavaScript array element
    /// is a real place at every element type.
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

    /// `function zd_enc_T(w, v) { … }`, or the reading form that opens
    /// from Table 9 and returns the sample.
    ///
    /// A top-level declaration, in module scope: nothing this column
    /// generates is `export`ed, because the probe that runs it is a
    /// script and not a package.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![Ident::new(if spec.writing { "w" } else { "r" })];
        if spec.writing {
            params.push(Ident::new("v"));
        }
        for (_, name) in &spec.extra {
            params.push(Ident::new(name));
        }
        let body = if spec.writing {
            body
        } else {
            Self::reading_body(&spec.name, body)
        };
        Some(Stat::Function(Function {
            doc: spec.doc,
            name: Ident::new(&spec.name),
            params,
            body,
        }))
    }

    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_node::default_into(ast, id, place))
    }

    /// # Errors
    /// As [`crate::default_node::member_default`]: a `@default` this
    /// column cannot spell as a JavaScript expression is refused rather
    /// than silently replaced by Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_node::member_default(ast, node, type_, place)
    }

    /// `a_b_Colour.RED` — the flattened frozen object
    /// [`crate::adapter::node`] declared, read from the file's own
    /// top-level scope.
    ///
    /// The trait's default answers the bare enumerator name prefixed by
    /// [`Node::module_prefix`], which is the empty string here — this
    /// column flattens a declared type's whole scoped name into its one
    /// binding instead, and the enumerator lives inside that binding's
    /// own object.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        Expr::member(
            Expr::name(&crate::adapter::node::node_type_name(ast, id)),
            &field_name(enumerator),
        )
    }
}

/// Whether `value` is already a call to the unqualified runtime function
/// `name`.
fn is_call_to(value: &Expr, name: &str) -> bool {
    let Expr::Call { callee, .. } = value else {
        return false;
    };
    let Expr::Member {
        object,
        name: field,
    } = callee.as_ref()
    else {
        return false;
    };
    if field.as_str() != name {
        return false;
    }
    matches!(object.as_ref(), Expr::Name(root) if root.as_str() == RUNTIME)
}

/// The Node type of a value, structurally.
///
/// `number` and `bigint` by the boundary [`Node::scalar`] draws, an
/// array for either collection, the flattened declared name for an
/// aggregate that has one, and the aliased type's answer for a typedef —
/// a `typedef` has no JavaScript form of its own and every position that
/// reaches one already reads through it.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn value_type_of(ast: &Ast, id: Id<TypeNode>, depth: usize) -> NodeType {
    use zerodds_ast::PrimitiveKind;
    if depth > MAX_TYPE_DEPTH {
        return NodeType::tag("object");
    }
    let Ok(node) = ast.type_(id) else {
        return NodeType::tag("object");
    };
    match &node.kind {
        TypeKind::Primitive(kind) => match kind {
            PrimitiveKind::Boolean => NodeType::tag("boolean"),
            PrimitiveKind::Char | PrimitiveKind::WChar => NodeType::tag("string"),
            PrimitiveKind::Int64 | PrimitiveKind::UInt64 => NodeType::tag("bigint"),
            PrimitiveKind::Float128 => NodeType::tag("float128"),
            _ => NodeType::tag("number"),
        },
        TypeKind::String { .. } | TypeKind::Fixed { .. } => NodeType::tag("string"),
        // §7.4.3.5.3 rule (5) caps an enumerator's holder at 32 bits, so
        // the constant a declaration reads back is always a `number`.
        TypeKind::Enumeration { .. } => NodeType::tag("number"),
        TypeKind::Bitmask { .. } => NodeType::tag(if ast.effective_bit_bound(id) > 32 {
            "bigint"
        } else {
            "number"
        }),
        TypeKind::Alias { aliased } => aliased.resolved().map_or_else(
            || NodeType::tag("object"),
            |target| value_type_of(ast, target, depth + 1),
        ),
        _ => NodeType::tag("object"),
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
/// generated Node held no `zd_over` check at all, so `a` accepted a
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

/// The tag a codec function is named after, from the IDL kind and never
/// from the JavaScript spelling — ten of the fifteen primitives are
/// `number` here, and a tag built from that would give them one codec.
/// The same scheme [`crate::adapter::lua::type_tag`] uses.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    use zerodds_ast::CharWidth;
    if depth > MAX_TYPE_DEPTH {
        return "unresolved".to_string();
    }
    let Ok(node) = ast.type_(id) else {
        return "unresolved".to_string();
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
        | TypeKind::Native => crate::adapter::node::node_type_name(ast, id),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}
