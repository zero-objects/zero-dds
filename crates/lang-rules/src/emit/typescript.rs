// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeScript backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c`,
//! `runtime/java` and the rest do, and none of them calls another.
//! Where two columns disagree on an octet, one has misread the
//! specification — which is the only reason to have a second column.
//!
//! # The three decisions this column turns on
//!
//! **A `number` is a binary64.** So every integer up to `i32` is one
//! and everything wider is a `bigint`. The boundary is the type's and
//! not the wire's: a binary64 holds every `i32` exactly and not every
//! `i64`, and a `long long` that arrived as a `number` would already
//! have lost its low bits before any octet was written.
//!
//! **A member is a property.** `v.field`, not `v->field` and not a
//! getter, which is what `adapter/typescript.rs` declares; the two have
//! to agree and a third reading is the one that would quietly diverge.
//!
//! **A `long double` never becomes a `number`.** `ZdFloat128` carries
//! the sixteen octets, because a binary64 would drop sixty bits of the
//! significand at the assignment.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::typescript as ts;

/// The TypeScript backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeScript;

impl TypeScript {
    /// `write_u32` as `writeU32`.
    ///
    /// The runtime's methods are named by TypeScript's convention and
    /// the walk asks in the wire's, so one of the two has to convert.
    /// Doing it here means a new operation is added to the walk under
    /// one name.
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

    fn path(name: &str) -> Path {
        Path::new(&name.split('.').collect::<Vec<_>>())
    }

    /// A reader's body, with the sample it answers with around it.
    ///
    /// `let v: T = zd_def_T(); … return v;`. Two things are settled here
    /// and neither is a spelling:
    ///
    /// - **Where the sample comes from.** Table 9's value, through the
    ///   generated defaulter, and not `new TImpl()` and not `undefined`.
    ///   Every position the stream does not carry is then already what
    ///   DDS-XTypes 1.3 Table 9 says it is, without the walk having to
    ///   reach each one; a `new TImpl()` would be a second answer to
    ///   Table 9 beside [`crate::default_typescript`], and the two can
    ///   part.
    /// - **What the name is.** The defaulter and the decoder are keyed
    ///   the same way — [`crate::emit::decode::decoder_name`] and
    ///   [`crate::default_typescript::defaulter_name`] differ only in
    ///   the three letters after `zd_` — so the one is the other with
    ///   the verb changed. A name that does not resolve is a `tsc` error
    ///   naming the function.
    ///
    /// Without this the walk's own `return v` referred to a binding
    /// nothing declared: the generated decoder named `v` in every
    /// statement and never introduced it.
    fn reading_body(name: &str, sample: ts::Type, body: ts::Block) -> ts::Block {
        if let Some((key, value)) = readonly_map_halves(&sample) {
            return Self::reading_map_body(key, value, body);
        }
        let defaulter = crate::default_typescript::defaulter_from_decoder(name);
        let mut statements = vec![ts::Statement::Declare {
            // `let` and not `const`: the walk assigns `v` whole for a
            // primitive and for a collection it re-seats.
            kind: ts::VarKind::Let,
            name: Ident::new("v"),
            type_: Some(sample),
            value: Some(Self::call(&defaulter, Vec::new())),
        }];
        statements.extend(body.statements);
        statements.push(ts::Statement::Return(Some(Self::whole("v"))));
        ts::Block { statements }
    }

    /// A map reader's body: the entry array the walk fills, turned into
    /// the `ReadonlyMap` §7.9.1 says the surface is.
    ///
    /// XTypes 1.3 §7.4.1.1.4 puts a map on the wire as a *sequence of
    /// entries*, and the shared walk reads one that way — `v[i].key`,
    /// `v[i].value`, `v.length`. A `Map` has none of those, and the read
    /// direction could not be written against one at all: an entry is
    /// filled in two steps, and there is no half-built entry in a `Map`
    /// to fill. So the walk keeps its array and the conversion happens
    /// once, here, at the boundary where the value leaves the codec.
    ///
    /// Every `return v` the walk emitted — the refusals of
    /// `CodecTarget::give_back` as well as the one at the end — is
    /// rewritten, because each of them is answering with the map.
    fn reading_map_body(key: ts::Type, value: ts::Type, body: ts::Block) -> ts::Block {
        let mut statements = vec![ts::Statement::Declare {
            kind: ts::VarKind::Let,
            name: Ident::new("v"),
            type_: Some(ts::Type::Array(Box::new(entry_object(key, value)))),
            value: Some(ts::Expr::Array(Vec::new())),
        }];
        statements.extend(body.statements.into_iter().map(answer_with_map));
        statements.push(ts::Statement::Return(Some(map_of_entries())));
        ts::Block { statements }
    }
}

/// `{ key: K; value: V }` — the entry the codec stages a map through.
///
/// Structural and not the generated `ZdEntry_k_v` interface by name: the
/// two are the same type to `tsc`, and this one can be built from the
/// `ReadonlyMap`'s own arguments without asking the materialiser for a
/// name a second time.
fn entry_object(key: ts::Type, value: ts::Type) -> ts::Type {
    let property = |name: &str, type_: ts::Type| {
        ts::Signature::Property(ts::Property {
            doc: Doc::none(),
            readonly: false,
            name: ts::PropertyName::ident(name),
            optional: false,
            type_,
        })
    };
    ts::Type::Object(vec![property("key", key), property("value", value)])
}

/// `new Map(v.map((zd_p) => [zd_p.key, zd_p.value] as const))`
///
/// `as const` on the pair and not a bare array literal: the `Map`
/// constructor takes `readonly (readonly [K, V])[]`, and `[a, b]` infers
/// as `(K | V)[]` — which is not a pair and constructs nothing.
fn map_of_entries() -> ts::Expr {
    let half = |name: &str| ts::Expr::Member {
        object: Box::new(ts::Expr::Path(Path::new(&["zd_p"]))),
        name: Ident::new(name),
    };
    ts::Expr::New {
        callee: Path::new(&["Map"]),
        args: vec![ts::Expr::Call {
            callee: Box::new(ts::Expr::Member {
                object: Box::new(ts::Expr::Path(Path::new(&["v"]))),
                name: Ident::new("map"),
            }),
            args: vec![ts::Expr::Arrow {
                params: vec![Ident::new("zd_p")],
                body: Box::new(ts::Expr::AsConst(Box::new(ts::Expr::Array(vec![
                    half("key"),
                    half("value"),
                ])))),
            }],
        }],
    }
}

/// `Array.from(zd_map).map((zd_p) => ({ key: zd_p[0], value: zd_p[1] }))`
///
/// The write direction's conversion: the walk indexes an entry array and
/// the parameter is the `ReadonlyMap` a caller holds, so the pairs are
/// staged once at the top of the function.
fn entries_of_map() -> ts::Expr {
    entries_of(ts::Expr::Path(Path::new(&["zd_map"])))
}

/// `Array.from(source).map((zd_p) => ({ key: zd_p[0], value: zd_p[1] }))`
///
/// The same conversion for any source expression, not just the writer's
/// `zd_map` parameter. A map reader's `UseDefault` branch needs it: the
/// defaulter answers the public `ReadonlyMap`, while `v` holds the pair
/// array the walk fills, and assigning one to the other does not
/// type-check.
fn entries_of(source: ts::Expr) -> ts::Expr {
    let half = |index: i128| ts::Expr::Index {
        object: Box::new(ts::Expr::Path(Path::new(&["zd_p"]))),
        index: Box::new(ts::Expr::Literal(Literal::int(index))),
    };
    ts::Expr::Call {
        callee: Box::new(ts::Expr::Member {
            object: Box::new(ts::Expr::Call {
                callee: Box::new(ts::Expr::Member {
                    object: Box::new(ts::Expr::Path(Path::new(&["Array"]))),
                    name: Ident::new("from"),
                }),
                args: vec![source],
            }),
            name: Ident::new("map"),
        }),
        args: vec![ts::Expr::Arrow {
            params: vec![Ident::new("zd_p")],
            // A concise arrow body starting with `{` is a *block*, so
            // the object literal is parenthesised.
            body: Box::new(ts::Expr::Parenthesised(Box::new(ts::Expr::Object(vec![
                (ts::PropertyName::ident("key"), half(0)),
                (ts::PropertyName::ident("value"), half(1)),
            ])))),
        }],
    }
}

/// The two arguments of a `ReadonlyMap<K, V>`, or `None` for any other
/// type.
fn readonly_map_halves(type_: &ts::Type) -> Option<(ts::Type, ts::Type)> {
    let ts::Type::Named { path, args } = type_ else {
        return None;
    };
    let [key, value] = args.as_slice() else {
        return None;
    };
    let [head] = path.segments() else {
        return None;
    };
    if head.as_str() != "ReadonlyMap" {
        return None;
    }
    Some((key.clone(), value.clone()))
}

/// Whether an expression assigned to a map reader's `v` answers the
/// public `ReadonlyMap` rather than the pair array.
///
/// Only the Table 9 defaulter does. The other assignment in the same
/// body is `v = new Array(zd_n)`, which reserves the pair array's
/// capacity and is already the right shape — converting that one would
/// build `zd_n` objects with undefined halves for the loop below it to
/// overwrite.
fn answers_a_map(value: &ts::Expr) -> bool {
    let ts::Expr::Call { callee, .. } = value else {
        return false;
    };
    let ts::Expr::Path(path) = callee.as_ref() else {
        return false;
    };
    path.segments()
        .last()
        .is_some_and(|segment| segment.as_str().starts_with("zd_def_"))
}

/// `return v` as `return new Map(…)`, wherever it appears in a map
/// reader's body.
///
/// zerodds-lint: recursion-depth 64 (statement nesting; bounded by the walk)
fn answer_with_map(statement: ts::Statement) -> ts::Statement {
    let rewrite = |block: ts::Block| ts::Block {
        statements: block.statements.into_iter().map(answer_with_map).collect(),
    };
    match statement {
        ts::Statement::Return(Some(ts::Expr::Path(ref path)))
            if path.segments().len() == 1
                && path
                    .segments()
                    .first()
                    .is_some_and(|segment| segment.as_str() == "v") =>
        {
            ts::Statement::Return(Some(map_of_entries()))
        }
        // `v = zd_def_X()`: the defaulter answers the public
        // `ReadonlyMap`, `v` is the pair array. Without the conversion the
        // branch does not type-check — TS2740, "Type 'ReadonlyMap' is
        // missing the following properties from type '{ key; value; }[]'".
        // Dropping the statement instead would return whatever was read
        // before the overflow rather than the default.
        ts::Statement::Expr(ts::Expr::Binary { op: "=", lhs, rhs })
            if matches!(
                lhs.as_ref(),
                ts::Expr::Path(path)
                    if path.segments().len() == 1
                        && path
                            .segments()
                            .first()
                            .is_some_and(|segment| segment.as_str() == "v")
            ) && answers_a_map(rhs.as_ref()) =>
        {
            ts::Statement::Expr(ts::Expr::Binary {
                op: "=",
                lhs,
                rhs: Box::new(entries_of(*rhs)),
            })
        }
        ts::Statement::If {
            condition,
            then,
            otherwise,
        } => ts::Statement::If {
            condition,
            then: rewrite(then),
            otherwise: otherwise.map(rewrite),
        },
        ts::Statement::For {
            counter,
            bound,
            body,
        } => ts::Statement::For {
            counter,
            bound,
            body: rewrite(body),
        },
        ts::Statement::While { condition, body } => ts::Statement::While {
            condition,
            body: rewrite(body),
        },
        ts::Statement::ForOf {
            kind,
            binding,
            range,
            body,
        } => ts::Statement::ForOf {
            kind,
            binding,
            range,
            body: rewrite(body),
        },
        ts::Statement::Switch { value, cases } => ts::Statement::Switch {
            value,
            cases: cases
                .into_iter()
                .map(|case| ts::SwitchCase {
                    labels: case.labels,
                    body: rewrite(case.body),
                    breaks: case.breaks,
                })
                .collect(),
        },
        other => other,
    }
}

impl CodecTarget for TypeScript {
    type Expr = ts::Expr;
    type Stmt = ts::Statement;
    type Block = ts::Block;
    type Type = ts::Type;
    type Decl = ts::Decl;

    // -- types --------------------------------------------------------

    /// `number` to `i32`, `bigint` beyond it.
    ///
    /// Not a style: `Number.MAX_SAFE_INTEGER` is 2^53 − 1, so a
    /// `number` holds every `i32` and every `u32` exactly and neither
    /// `i64` nor `u64`. An index and a size are `number` because a
    /// collection's length is one.
    ///
    /// The same boundary `adapter/typescript.rs` draws for a member's
    /// declared type and `runtime/typescript` for its parameters.
    /// Three places have to agree.
    ///
    /// [`ts::Type::Keyword`] and not a named path spelled `"number"`.
    /// The two render the same text and are not the same tree, and the
    /// materialiser builds the keyword: a backend that built the path
    /// answered `false` to every `type == ts::Type::number()` a caller
    /// asked, which is how a holder decision gets made twice and
    /// differently.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => ts::Type::Keyword(ts::Keyword::Boolean),
            Scalar::Char => ts::Type::string(),
            Scalar::Int8
            | Scalar::UInt8
            | Scalar::Int16
            | Scalar::UInt16
            | Scalar::Int32
            | Scalar::UInt32
            | Scalar::Index
            | Scalar::Size
            | Scalar::Float
            | Scalar::Double => ts::Type::number(),
            Scalar::Int64 | Scalar::UInt64 => ts::Type::bigint(),
            Scalar::LongDouble => ts::Type::named(Path::new(&["ZdFloat128"])),
        }
    }

    fn named(name: &str) -> Self::Type {
        ts::Type::named(Self::path(name))
    }

    fn nothing() -> Self::Type {
        ts::Type::Keyword(ts::Keyword::Void)
    }

    /// The runtime's handles, by their exported names.
    ///
    /// A generated file imports them, so the simple name is what it
    /// writes; `runtime/typescript` exports each as an interface rather
    /// than nesting it in the writer, because TypeScript has no nested
    /// type in a class the way Java does.
    fn handle(kind: Handle) -> Self::Type {
        ts::Type::named(Path::new(&[match kind {
            Handle::DHeader => "DHeader",
            Handle::EmHeader => "EmHeader",
            Handle::Pid => "Pid",
            Handle::TryConstruct => "TryConstruct",
            Handle::ReaderMark => "ReaderMark",
            Handle::PidRead => "PidRead",
            Handle::EmHeaderRead => "EmHeaderRead",
        }]))
    }

    /// A `string` of one character is what a `char` is here.
    ///
    /// `codePointAt(0)` is the runtime's, not the generated file's: the
    /// narrowing a `char` needs belongs where the octet is written.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's enumerations, in TypeScript's naming.
    ///
    /// One member per constant and no catch-all: a policy this column
    /// has not implemented has to be a compile error here rather than a
    /// value that looks plausible.
    fn constant(which: Constant) -> Self::Expr {
        ts::Expr::Path(Self::path(match which {
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
            Constant::StatusOk => "ZdReaderStatus.Ok",
            Constant::StatusDiscarded => "ZdReaderStatus.Discarded",
            Constant::StatusImpossibleLength => "ZdReaderStatus.ImpossibleLength",
            Constant::StatusOutOfRange => "ZdReaderStatus.OutOfRange",
            Constant::StatusTruncated => "ZdReaderStatus.Truncated",
        }))
    }

    /// An unsigned holder is a `number` to 32 bits and a `bigint` at
    /// 64, for the reason [`Self::scalar`] gives.
    fn unsigned_holder(bits: u16) -> Self::Type {
        crate::adapter::typescript::unsigned_holder(bits)
    }

    /// `zdNumber(zd_bits) as Flags` — the holder conversion first, the
    /// alias after.
    ///
    /// The trait's one cast is `zd_bits as Flags`, and `Flags` here is
    /// `type Flags = number` for a bound to 32 bits while the read
    /// answers a `bigint`: `as` between the two is TS2352, because `as`
    /// is erased and would have left the value a `bigint` besides. The
    /// runtime conversion through [`Self::unsigned_holder`]'s keyword is
    /// the same one every bitset field takes.
    fn bitmask_value(ast: &Ast, id: Id<TypeNode>, value: Self::Expr) -> Self::Expr {
        let bound = ast.effective_bit_bound(id);
        Self::cast(
            Self::value_type(ast, id),
            Self::cast(Self::unsigned_holder(bound), value),
        )
    }

    /// The TypeScript type of a value of `id`.
    ///
    /// Not [`Self::type_name`], which is the codec's tag —
    /// `p_long`, `seq_long` — and names a function, not a type. A
    /// parameter declared `v: long` is a reference to a type
    /// TypeScript does not have, and `tsc` says so; it was what the
    /// first generated codec of this column wrote.
    ///
    /// Not `adapter/typescript.rs`'s `ts_member_type` either: that one
    /// needs the rule application to spell a *member's* declared type,
    /// which a codec does not have and does not need. A structural
    /// walk is enough here, and the two agree on every shape because
    /// both read the same model.
    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        value_type_of(ast, id, 0)
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        crate::adapter::typescript::type_tag(ast, id, 0)
            .unwrap_or_else(|_| format!("unresolved_type_{}", id.slot()))
    }

    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        crate::adapter::typescript::map_entry_name(ast, id)
            .unwrap_or_else(|_| format!("UnresolvedEntry{}", id.slot()))
    }

    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        // TypeScript has namespaces and the materialiser nests in them,
        // so a declaration's own name is enough inside its namespace.
        String::new()
    }

    fn escape(name: &str) -> String {
        ts::escape_member(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some("parent.".repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        ts::Expr::Path(Self::path(text))
    }

    fn int(value: i128) -> Self::Expr {
        ts::Expr::Literal(Literal::int(value))
    }

    /// A `bigint` literal, which is a number with an `n` after it.
    ///
    /// Every value the walk hands here is one a `number` cannot carry —
    /// that is what makes it wide — so the suffix is unconditional.
    fn wide_int(value: i128) -> Self::Expr {
        ts::Expr::Literal(Literal::Int(format!("{value}n")))
    }

    fn literal(value: Literal) -> Self::Expr {
        ts::Expr::Literal(value)
    }

    /// The number, with no suffix.
    ///
    /// The trait's default is C's `123u`. TypeScript has one numeric
    /// type and no suffix for it, and `2u` is not an expression at all:
    /// `tsc` calls it TS1351, "an identifier or keyword cannot
    /// immediately follow a numeric literal". Every array bound and
    /// every string bound the decode walk writes goes through here.
    fn unsigned_int(value: i128) -> Self::Expr {
        ts::Expr::Literal(Literal::int(value))
    }

    /// A field of a header the runtime handed back, in the runtime's
    /// spelling.
    ///
    /// The walk names a header's parts in the wire's convention —
    /// `member_id`, `must_understand` — and `runtime/typescript` spells
    /// them as TypeScript properties. The names are listed rather than
    /// run through `Self::camel` so that a part the runtime does not
    /// publish is visible here: it becomes a `tsc` error naming the
    /// property, which is where a header that grew a part shows up.
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

    /// `strcmp` is the walk's name for a three-way code-unit
    /// comparison; TypeScript's `<`/`>` on strings compare UTF-16 code
    /// units, so the spelling is a ternary — the Java column's caveat
    /// about supplementary code points applies unchanged.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        // A slice pattern rather than a length check and two `next()`
        // calls: the pattern binds both arguments or neither, so no
        // branch is left over to claim it cannot be reached.
        if callee == "strcmp" {
            if let [left, right] = args.as_slice() {
                return ts::Expr::Paren(Box::new(ts::Expr::Ternary {
                    condition: Box::new(ts::Expr::Binary {
                        op: "<",
                        lhs: Box::new(left.clone()),
                        rhs: Box::new(right.clone()),
                    }),
                    then: Box::new(Self::int(-1)),
                    otherwise: Box::new(ts::Expr::Ternary {
                        condition: Box::new(ts::Expr::Binary {
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
        ts::Expr::Call {
            callee: Box::new(ts::Expr::Path(Self::path(callee))),
            args,
        }
    }

    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        ts::Expr::Call {
            callee: Box::new(ts::Expr::Member {
                object: Box::new(ts::Expr::Path(Self::path(receiver))),
                name: Ident::new(&Self::camel(method)),
            }),
            args,
        }
    }

    /// `runtime/typescript` carries the three HDDS axes: `beginPackedElement`
    /// and `finishPackedElement` on the writer and the reader, the DHEADER
    /// pair for a sequence element's own length, and
    /// `writeWstringUtf8Octets` / `readWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// A free function the generated file imports from the runtime.
    ///
    /// TypeScript has module-level functions, so unlike Java there is
    /// no class to hang them on.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        Self::call(&Self::camel(method), args)
    }

    fn member(base: &str, field: &str) -> Self::Expr {
        ts::Expr::Member {
            object: Box::new(ts::Expr::Path(Self::path(base))),
            name: Ident::new(field),
        }
    }

    fn whole(base: &str) -> Self::Expr {
        ts::Expr::Path(Self::path(base))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        ts::Expr::Member {
            object: Box::new(value),
            name: Ident::new(field),
        }
    }

    /// `v[i]!`, with the non-null assertion.
    ///
    /// `runtime/typescript/tsconfig.json` compiles under
    /// `noUncheckedIndexedAccess`, where `v[i]` is `T | undefined`
    /// however the loop is bounded — the checker does not relate `i` to
    /// `v.length`. Every index the walk builds is inside a loop it
    /// bounded itself with that very length, so the assertion states
    /// something the generated code has already established.
    ///
    /// Written here rather than turning the setting off, because a
    /// generated file that only compiles under looser settings is one a
    /// consumer who has that setting on cannot use. It is the one place
    /// this column writes a `!`.
    ///
    /// The read direction will need an lvalue form: `v[i]! = x` is not
    /// an assignment target. `emit/typescript.rs` refuses to generate a
    /// decoder at all today — `default_into` says so by name — and the
    /// module that lifts that refusal has this to solve.
    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        ts::Expr::NonNull(Box::new(ts::Expr::Index {
            object: Box::new(value),
            index: Box::new(at),
        }))
    }

    /// `.length`, or the bound where the materialiser fixed one.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => ts::Expr::Literal(Literal::int(i128::from(bound))),
            None => ts::Expr::Member {
                object: Box::new(Self::whole(base)),
                name: Ident::new("length"),
            },
        }
    }

    fn elements(base: &str) -> Self::Expr {
        Self::whole(base)
    }

    /// `(v as Extract<typeof v, { one: unknown }>).one`
    ///
    /// Not `v.one`. A union is a *discriminated union* here and not a
    /// struct with every branch's field in it, which is what the C and
    /// Java columns have — so a case member lives on one arm only and
    /// reaching it needs the value narrowed to that arm. **Both
    /// directions need it, for different reasons**, and the two were
    /// found independently by the read and the write halves of this
    /// column before they were merged:
    ///
    /// - **Reading.** TypeScript refuses `v.one = …` on a union-typed
    ///   `v`: `one` is in one arm and `_d` is in all of them, so the
    ///   discriminator assigns and the selected member does not. TS2339
    ///   on every union the walk reads.
    /// - **Writing.** Narrowing on the discriminator is what the walk's
    ///   `if` already writes, and it is not enough for either of the two
    ///   shapes the corpus has: a `default:` arm carries the
    ///   discriminator's **whole** type, so `v._d === 1` leaves both
    ///   arms live — TypeScript cannot spell "a `number` that is not 1"
    ///   — and the no-case-selected arm `{ _d: number }` has the same
    ///   effect on every union with no `default:`.
    ///
    /// So this is not a spelling: it is the shape `adapter/typescript.rs`
    /// deliberately chose so that a reader of the value narrows on `_d`.
    /// `Extract<typeof v, { <field>: unknown }>` names the one arm that
    /// has the field, so the access is checked against the member's own
    /// declared type rather than through an `any` that would check
    /// nothing. `typeof v` is the *declared* type and not the narrowed
    /// one, which is what makes it the same expression in both
    /// directions.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        let arm = ts::Type::generic(
            Path::new(&["Extract"]),
            vec![
                ts::Type::TypeOf(Path::new(&[base])),
                ts::Type::Object(vec![ts::Signature::Property(ts::Property {
                    doc: Doc::none(),
                    readonly: false,
                    name: ts::PropertyName::ident(field),
                    optional: false,
                    type_: ts::Type::Keyword(ts::Keyword::Unknown),
                })]),
            ],
        );
        ts::Expr::Member {
            // `Paren` and not `Parenthesised`. The CST has both and the
            // renderer writes `( … )` for each, which is why the two
            // halves of this column reached for a different one and
            // neither noticed. `Paren` is the one the rest of this file
            // already uses.
            object: Box::new(ts::Expr::Paren(Box::new(ts::Expr::As {
                value: Box::new(Self::whole(base)),
                type_: Box::new(arm),
            }))),
            name: Ident::new(field),
        }
    }

    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        ts::Expr::Member {
            object: Box::new(entry),
            name: Ident::new(if key { "key" } else { "value" }),
        }
    }

    /// `v.discriminator`, which is the name DDS-TS 1.0 §7.5 gives the
    /// property and the one `adapter/typescript.rs` declares. The
    /// trait's default is `_d`, which is what the other columns'
    /// materialisers call it.
    fn discriminant(base: &str) -> Self::Expr {
        Self::member(base, crate::adapter::typescript::DISCRIMINANT_PROPERTY)
    }

    /// `a.b.StatusOrdinal[v]` — the wire value an enumerator carries.
    ///
    /// §7.6 makes an enumeration a string-literal union, so this column
    /// has no integer in the variable at all: the other columns' answer
    /// (the value itself) would put the first code unit of the
    /// enumerator's name on the wire. Measured against the ledger,
    /// `enum-sparse-values` wrote `70 79 87` for `FAIL OK WARN` before
    /// this override existed, where the recorded octets are
    /// `65535 1 100`.
    ///
    /// The table is the materialiser's — `adapter/typescript.rs`'s
    /// `ordinal_path` names it — so the codec looks the value up rather
    /// than deriving it a second time from the model.
    ///
    /// The identity answer is **required** and not a safety net. Two of
    /// the three callers pass a type that is often not an enumeration
    /// at all: `encode::write_union_overlay`'s suppressed arm hands
    /// every `BitPatternSource::Int` through this seam — a `long` arm
    /// among them — and `encode::map_key_less` hands a map key through
    /// it whenever the ordering is by enumerator value. For those the
    /// integer is already in the variable and the table would name
    /// nothing.
    ///
    /// The remaining door — an enumeration whose table cannot be
    /// named — is a model `adapter::typescript::ts_items` has already
    /// refused: `ordinal_path` fails there only through `qualified`,
    /// and `assemble`'s `insert` walks the same `module_chain` for
    /// every declaration before a file exists.
    fn enumerator_ordinal(ast: &Ast, id: Id<TypeNode>, value: Self::Expr) -> Self::Expr {
        let Ok(table) = crate::adapter::typescript::ordinal_path(ast, id) else {
            return value;
        };
        ts::Expr::Index {
            object: Box::new(ts::Expr::Path(table)),
            index: Box::new(value),
        }
    }

    /// `a.b.Tag.TAG_A` for an enumeration discriminator, and the trait's
    /// narrowed literal for every other kind.
    ///
    /// §7.6 makes an enumeration a string-literal union, so the ordinal
    /// is not a value of the type: `v.discriminator === 1` against a
    /// `"TAG_A" | "TAG_B"` is TS2367, "the types have no overlap", on
    /// every enumeration-discriminated union. The constant is also what
    /// narrows the arm — comparison against a literal-typed `const` is
    /// what TypeScript reads as a discriminant test.
    ///
    /// A label that names no enumerator falls back to the trait's
    /// answer, which `tsc` then reports: there is no member for it, and
    /// a number written in its place would compile against no arm at
    /// all.
    fn case_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Self::Expr {
        crate::default_typescript::case_label(ast, discriminator, value).unwrap_or_else(|| {
            Self::cast(Self::value_type(ast, discriminator), Self::wide_int(value))
        })
    }

    fn absent() -> Self::Expr {
        ts::Expr::Path(Path::new(&["undefined"]))
    }

    /// `!== undefined`, which is what an absent `@optional` member is
    /// here: `adapter/typescript.rs` declares one `field?: T`, so the
    /// property is missing rather than null.
    fn materialised(value: Self::Expr) -> Option<Self::Expr> {
        Some(ts::Expr::Binary {
            op: "!==",
            lhs: Box::new(value),
            rhs: Box::new(Self::absent()),
        })
    }

    /// The member itself: `field?: T` is the value or nothing, and
    /// there is no wrapper to unwrap the way Java's boxed field has.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    fn present(base: &str, field: &str) -> Self::Expr {
        ts::Expr::Binary {
            op: "!==",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        }
    }

    /// `delete v.field` for absence, and nothing for presence.
    ///
    /// Not `v.field = undefined`. The declaration is `field?: T` and the
    /// tree compiles under `exactOptionalPropertyTypes`, where assigning
    /// `undefined` to an optional property is TS2412 — the property
    /// being *gone* and the property holding `undefined` are two things
    /// there. `delete` is also what [`crate::default_typescript`] writes
    /// for the same clause, so a decoder and a defaulter cannot part
    /// over what absent means.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![ts::Statement::Expr(ts::Expr::Unary {
            op: "delete ",
            value: Box::new(Self::member(base, field)),
        })]
    }

    /// `delete (v as { field?: unknown }).field` for every case.
    ///
    /// DDS-TS 1.0 §7.5 makes a union a discriminated union of object
    /// types, one per case, so a branch the discriminator does not
    /// select is a property the object does not have. The Table 9
    /// default the decoder starts from carries the default branch's
    /// property, and without this a sample read with another
    /// discriminator kept it (measured: `idl-conformance/_interop`
    /// `combo`, `reading` came back with `idleTicks: 0` beside
    /// `activeRate`). The union type names no property shared by every
    /// variant, so the operand is widened to an object whose one
    /// property is optional — the form `delete` accepts (TS2790).
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .map(|field| {
                let optional = ts::Type::Object(vec![ts::Signature::Property(ts::Property {
                    doc: Doc::none(),
                    readonly: false,
                    name: ts::PropertyName::ident(field),
                    optional: true,
                    type_: ts::Type::Keyword(ts::Keyword::Unknown),
                })]);
                ts::Statement::Expr(ts::Expr::Unary {
                    op: "delete ",
                    value: Box::new(ts::Expr::Member {
                        object: Box::new(ts::Expr::Paren(Box::new(ts::Expr::As {
                            value: Box::new(Self::whole(base)),
                            type_: Box::new(optional),
                        }))),
                        name: Ident::new(field),
                    }),
                })
            })
            .collect()
    }

    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    /// `==` becomes `===` and `!=` becomes `!==`.
    ///
    /// Not house style. TypeScript narrows a discriminated union on the
    /// **strict** comparison of its discriminant against a literal, and
    /// a union case member is only reachable through that narrowing:
    /// `if (v._d == 1) { w.writeI32(v.whole) }` is
    /// `TS2339: Property 'whole' does not exist`, and the same line with
    /// `===` compiles. Measured on `array-1d-union`.
    ///
    /// The loose form is also a different operator — `0 == "0"` is true
    /// — so a discriminator that arrived as a string would select an
    /// arm no other column selects.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        ts::Expr::Binary {
            op: match op {
                "==" => "===",
                "!=" => "!==",
                other => other,
            },
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// `!a` as written, and every other operator as itself.
    ///
    /// TypeScript has a real unary form for both `!` and `~`, so
    /// neither needs the `0 <op> a` stand-in that cost the C++ column
    /// twenty-one cases.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        ts::Expr::Unary {
            op,
            value: Box::new(value),
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        // The renderer does not bracket by precedence, and `&` binds
        // more loosely than `!==` here as it does in Java.
        ts::Expr::Paren(Box::new(value))
    }

    /// `false` before `true`, spelled as the conjunction — TypeScript's
    /// `<` does not take two booleans (TS2365).
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        Self::binary(
            "&&",
            ts::Expr::Paren(Box::new(Self::binary(
                "==",
                left,
                ts::Expr::Literal(Literal::Bool(false)),
            ))),
            right,
        )
    }

    /// A conversion where the target is one of the three scalar holders,
    /// a rewritten literal where the value is one, and `as T` elsewhere.
    ///
    /// `as` is **erased**. Every other column's narrowing is a real
    /// operation — `(int)` in Java, `static_cast` in C++ — and writing
    /// `as` for those would be two defects at once. `tsc` refuses the
    /// erased form outright where the holders differ:
    /// `zd_bits as number` on a `bigint` is TS2352, which the decode
    /// walk emits for every bitmask and every bitset field and the
    /// encode walk for every bitmask, bitset and union case. And where
    /// it does not refuse — `x as unknown as number` — the value would
    /// go into the field still a `bigint`, and every later comparison
    /// against it would be false.
    ///
    /// So the three holders take a runtime conversion:
    /// `zdNumber`, `zdBigInt` and `zdChar` in `runtime/typescript`,
    /// which carry the same value into `number`, `bigint` and the
    /// one-character `string` an IDL `char` is here. Each is the
    /// identity on a value already in its holder.
    ///
    /// A **literal** is rewritten at generation time instead, and that
    /// is not an optimisation. Two things need it, and both are the
    /// encode walk's union labels — `cast(value_type(discriminator),
    /// wide_int(label))`:
    ///
    /// - a `boolean` or enumeration discriminator reaches **no**
    ///   coercion arm, because its holder is `boolean` or the
    ///   materialiser's `type Colour = number`, and `1n as boolean` and
    ///   `1n as a.b.Colour` are both TS2352;
    /// - a literal type is what a union arm is narrowed on, and
    ///   `v._d === zdNumber(1n)` narrows nothing while `v._d === 1`
    ///   narrows to the arm.
    ///
    /// The four forms are the four `adapter/typescript.rs`'s
    /// `discriminant_literal` writes, read off the target type rather
    /// than off the model — a third reading of that mapping is how the
    /// declaration and the comparison come to describe different arms.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if let ts::Expr::Literal(Literal::Int(text)) = &value {
            if let Some(literal) = literal_as(&type_, text) {
                return ts::Expr::Literal(literal);
            }
        }
        // The walk's `signed char` — the C spelling of a map key's
        // signed order — has no type here: an IDL `char` is a
        // one-character `string`. `((k.charCodeAt(0) + 128) % 256) -
        // 128` is the two's complement reading of its one octet, the
        // same arithmetic the Python column writes and for the same
        // reason (see `python.rs::signed_char`).
        if type_ == ts::Type::named(Self::path("signed char")) {
            let point = ts::Expr::Call {
                callee: Box::new(ts::Expr::Member {
                    object: Box::new(ts::Expr::Paren(Box::new(value))),
                    name: Ident::new("charCodeAt"),
                }),
                args: vec![Self::int(0)],
            };
            return Self::binary(
                "-",
                ts::Expr::Paren(Box::new(Self::binary(
                    "%",
                    ts::Expr::Paren(Box::new(Self::binary("+", point, Self::int(128)))),
                    Self::int(256),
                ))),
                Self::int(128),
            );
        }
        let coercion = match &type_ {
            ts::Type::Keyword(ts::Keyword::Number) => Some("zdNumber"),
            ts::Type::Keyword(ts::Keyword::BigInt) => Some("zdBigInt"),
            ts::Type::Keyword(ts::Keyword::String) => Some("zdChar"),
            _ => None,
        };
        match coercion {
            // Not twice. The walk narrows and [`Self::store`] narrows
            // again at the destination, and `zdChar(zdChar(x))` is right
            // but says the same thing twice — which reads as though the
            // two were different narrowings.
            Some(callee) if is_call_to(&value, callee) => value,
            Some(callee) => Self::call(callee, vec![value]),
            None => ts::Expr::As {
                value: Box::new(value),
                type_: Box::new(type_),
            },
        }
    }

    /// The same conversion, at the point a value reaches its member.
    ///
    /// [`Self::cast`] answers the walk where the walk asked for a
    /// narrowing; this answers where it did not. A `wchar` is the one
    /// that needs it: the walk narrows the runtime's answer to the
    /// sixteen-bit storage every other column keeps a `wchar` in, and
    /// this column keeps it in a `string`, so the value arrives as a
    /// `number` and the destination is a `string`. Without the second
    /// conversion that is a `tsc` error on every `wchar` member.
    fn store(place: Self::Expr, type_: Self::Type, value: Self::Expr) -> Self::Stmt {
        Self::perform(Self::binary(
            "=",
            assignable(place),
            Self::cast(type_, value),
        ))
    }

    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// `place = zd_dec_T(r, zd_tc);`
    ///
    /// The trait's default hands the destination *to* the callee, which
    /// is C's shape: its decoder takes a `T *` and fills it. A
    /// TypeScript function has no out-parameter, so this column's
    /// decoders answer with the value — [`Self::routine`] declares them
    /// `(r, …extra)` — and the destination is assigned at the call site.
    ///
    /// Without this override the generated call was
    /// `zd_dec_p_long(r, v.a, TryConstruct.Discard)`: three arguments to
    /// a two-parameter function, and — had it compiled — a member read
    /// off the wire and thrown away, leaving Table 9's value in `v.a`
    /// with nothing to say so.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        Self::perform(Self::binary(
            "=",
            assignable(place),
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

    /// `let name: T = value`, with an integer literal spelled the way
    /// `T` reads it.
    ///
    /// `let bits: bigint = 0` does not typecheck — `0` is a `number` —
    /// and the walk declares exactly that for a bitset's accumulator:
    /// `T::declare(T::scalar(Scalar::UInt64), "bits", int(0))`. The `n`
    /// is not a suffix the walk can add, because in C and Java the same
    /// declaration is right as it stands. `adapter/typescript.rs` writes
    /// `k: bigint = 0n` for a member of the same type and for the same
    /// reason.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        let value = value.map(|value| match &value {
            ts::Expr::Literal(Literal::Int(text)) => {
                literal_as(&type_, text).map_or_else(|| value.clone(), ts::Expr::Literal)
            }
            _ => value,
        });
        ts::Statement::Declare {
            kind: ts::VarKind::Let,
            name: Ident::new(name),
            type_: Some(type_),
            value,
        }
    }

    fn perform(value: Self::Expr) -> Self::Stmt {
        ts::Statement::Expr(value)
    }

    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        ts::Statement::Return(Some(value.unwrap_or_else(|| Self::whole("v"))))
    }

    /// Nothing. TypeScript has no unread-parameter diagnostic under the
    /// settings this tree compiles with, and `zd_tc;` is not a
    /// statement worth writing.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// `void v;` — the one form TypeScript has for "read and
    /// discard", and the only place the walk needs it.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        ts::Statement::Expr(ts::Expr::Unary {
            op: "void ",
            value: Box::new(value),
        })
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        ts::Statement::If {
            condition,
            then,
            otherwise,
        }
    }

    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        ts::Statement::For {
            counter: Ident::new(counter),
            bound: Box::new(bound),
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        ts::Statement::While {
            condition: Box::new(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        ts::Statement::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        ts::Statement::Comment(doc)
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        ts::Block { statements }
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block.statements
    }

    /// A new array of `count` elements, which a TypeScript collection
    /// is: there is no reserve-then-fill, so the fill is the loop the
    /// walk hands over.
    ///
    /// `new Array(n)` is `n` holes. A sequence's element decoder answers
    /// with a value and the slot is assigned outright, so nothing more
    /// is needed; a `map`'s two halves are written *into* the entry at
    /// `v[i]`, and an entry that was never made is a `TypeError` at the
    /// first key — measured on `map-key-long`, "Cannot set properties
    /// of undefined". That is what `assigned_whole` separates, exactly
    /// as the Java column's `make_room` says, and the seed is the empty
    /// object whose halves the loop assigns before anything reads them.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![ts::Statement::Expr(ts::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::whole(base)),
            rhs: Box::new(ts::Expr::New {
                callee: Path::new(&["Array"]),
                args: vec![count],
            }),
        })];
        if !assigned_whole {
            out.push(Self::count_up(
                "zd_e",
                Self::count_of(base, None),
                ts::Block {
                    statements: vec![ts::Statement::Expr(ts::Expr::Binary {
                        op: "=",
                        lhs: Box::new(ts::Expr::Index {
                            object: Box::new(Self::whole(base)),
                            index: Box::new(Self::whole("zd_e")),
                        }),
                        rhs: Box::new(ts::Expr::As {
                            value: Box::new(ts::Expr::Object(Vec::new())),
                            type_: Box::new(element),
                        }),
                    })],
                },
            ));
        }
        out.extend(fill);
        out
    }

    /// `let name: T = {} as T;` — somewhere to put a value before its
    /// halves arrive.
    ///
    /// The trait's default declares without a value, which TypeScript
    /// under `strict` reads as "used before assigned" the moment a
    /// map's counting pass writes `zd_scratch.key` — and at run time
    /// the property assignment lands on `undefined`. The empty object
    /// is a seed whose properties the very next statements fill.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        ts::Statement::Declare {
            kind: ts::VarKind::Let,
            name: Ident::new(name),
            type_: Some(type_.clone()),
            value: Some(ts::Expr::As {
                value: Box::new(ts::Expr::Object(Vec::new())),
                type_: Box::new(type_),
            }),
        }
    }

    /// `v.length = n`, which shortens an array in place.
    ///
    /// §7.3.1.2.1.12.3's TRIM. A TypeScript array's length is writable
    /// and assigning a smaller one drops the tail, which is exactly the
    /// operation and is why this column needs no `splice`.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![ts::Statement::Expr(ts::Expr::Binary {
            op: "=",
            lhs: Box::new(ts::Expr::Member {
                object: Box::new(Self::whole(base)),
                name: Ident::new("length"),
            }),
            rhs: Box::new(count),
        })]
    }

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

    /// `export function encodeT(w: Writer, v: T): void`.
    ///
    /// Exported. A generated file is one module, and a module-level
    /// function that is not exported is reachable from nothing outside
    /// that file: `wireprobe-zerodds-typescript-gen` never noticed
    /// because it appends its own `main` to the same module, and an
    /// application generated by `idlc generate` has no way to call a
    /// single one of them. The same defect the Java column carried as
    /// `private static` and was measured out of; the declarations
    /// beside these — `newT`, `isT`, the interfaces — were exported all
    /// along.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![ts::Param {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            type_: Some(ts::Type::named(Path::new(&[if spec.writing {
                "Writer"
            } else {
                "Reader"
            }]))),
            optional: false,
            default: None,
            rest: false,
        }];
        // A map's parameter is the `ReadonlyMap` §7.9.1 declares and the
        // walk's body is written against the entry array XTypes 1.3
        // §7.4.1.1.4 puts on the wire, so the pairs are staged under the
        // name the body uses and the parameter takes another.
        let staged = spec
            .writing
            .then(|| readonly_map_halves(&spec.sample))
            .flatten();
        if spec.writing {
            params.push(ts::Param {
                name: Ident::new(if staged.is_some() { "zd_map" } else { "v" }),
                type_: Some(spec.sample.clone()),
                optional: false,
                default: None,
                rest: false,
            });
        }
        params.extend(spec.extra.into_iter().map(|(type_, name)| ts::Param {
            name: Ident::new(&name),
            type_: Some(type_),
            optional: false,
            default: None,
            rest: false,
        }));
        let (returns, body) = if spec.writing {
            let body = match staged {
                Some((key, value)) => {
                    let mut statements = vec![ts::Statement::Declare {
                        kind: ts::VarKind::Const,
                        name: Ident::new("v"),
                        type_: Some(ts::Type::Array(Box::new(entry_object(key, value)))),
                        value: Some(entries_of_map()),
                    }];
                    statements.extend(body.statements);
                    ts::Block { statements }
                }
                None => body,
            };
            (Self::nothing(), body)
        } else {
            (
                spec.sample.clone(),
                Self::reading_body(&spec.name, spec.sample, body),
            )
        };
        Some(ts::Decl::Function(ts::Function {
            doc: spec.doc,
            exported: true,
            name: Ident::new(&spec.name),
            generics: Vec::new(),
            params,
            returns: Some(returns),
            body: Some(body),
        }))
    }

    /// A call to a generated defaulter and not a literal in place.
    ///
    /// A literal cannot carry an aggregate, and one written at every
    /// position a member can go missing would put the same answer in the
    /// file as many times as the type is reached — where two copies can
    /// differ. [`crate::default_typescript`] carries the whole table and
    /// the argument.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_typescript::default_into(ast, id, place))
    }

    /// # Errors
    /// As [`crate::default_typescript::member_default`]: a `@default`
    /// this column cannot spell as a TypeScript expression is refused
    /// rather than silently replaced by Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_typescript::member_default(ast, node, type_, place)
    }

    /// `a.b.Colour.RED` — the `as const` object the materialiser
    /// declared, written from the file root.
    ///
    /// The trait's default answers the bare enumerator name, which is
    /// right for a target that flattens its modules into a prefix and
    /// wrong here: the TypeScript materialiser nests each declaration in
    /// a namespace per IDL module and a generated codec sits at the file
    /// root, so `RED` there resolves to nothing — or, worse, to whatever
    /// else happened to be in scope.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        crate::default_typescript::enumerator_constant(ast, id, enumerator)
    }

    /// `123n`, which is how TypeScript spells a 64-bit constant.
    ///
    /// The trait's default is C's `UINT64_C(123)`, which is a macro no
    /// TypeScript file declares. Both of these reach the generated
    /// source through the decode walk alone — the bitmask's
    /// undeclared-bit check and the enumerator chain — so a column that
    /// did not override them produced a decoder that did not parse.
    fn wide_unsigned(value: u64) -> Self::Expr {
        Self::literal(Literal::Int(format!("{value}n")))
    }

    /// As [`Self::wide_unsigned`], and the value rather than the
    /// two's-complement pattern: a `bigint` is unbounded, so a negative
    /// enumerator pattern is written as the negative number it is.
    fn wide_signed(value: i128) -> Self::Expr {
        Self::literal(Literal::Int(format!("{value}n")))
    }
}

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The same place, as something that can be assigned to.
///
/// [`TypeScript::index`] writes `v[i]!` because `v[i]` is
/// `T | undefined` under `noUncheckedIndexedAccess`. That assertion is
/// right where the element is **read** and is not an expression that can
/// be assigned to: `v[i]! = x` is a syntax error, where `v[i]!.key = x`
/// is fine because the assertion is on the object of a member access.
///
/// So the assertion is stripped at exactly the two points a place is
/// written — [`TypeScript::store`] and [`TypeScript::read_call`] — and
/// nowhere else. Nothing is asserted away: a destination is not read, so
/// there is no `undefined` to rule out.
fn assignable(place: ts::Expr) -> ts::Expr {
    match place {
        ts::Expr::NonNull(inner) => *inner,
        other => other,
    }
}

/// Whether `value` is already a call to the unqualified function `name`.
fn is_call_to(value: &ts::Expr, name: &str) -> bool {
    let ts::Expr::Call { callee, .. } = value else {
        return false;
    };
    let ts::Expr::Path(path) = callee.as_ref() else {
        return false;
    };
    path.segments().len() == 1
        && path
            .segments()
            .first()
            .is_some_and(|segment| segment.as_str() == name)
}

/// An integer literal in the spelling `target` calls for, or `None`
/// where the literal is not the thing to change.
///
/// The counterpart of `adapter/typescript.rs`'s `discriminant_literal`,
/// read off the target type instead of the model because that is all a
/// cast has. The two have to produce the same four forms: the
/// declaration says `{ _d: true; … }` and the comparison has to say
/// `v._d === true`, or the arm is unreachable and its member is a
/// property TypeScript says does not exist.
///
/// A **named** target answers like `number`, and that is the enumeration
/// case: the materialiser declares `type Colour = number` beside its
/// `as const` object, so a label cast to `Colour` is the number.
/// `unknown`, `void` and `any` answer `None` — a literal reaching one of
/// those is not a discriminator and rewriting it would hide whatever put
/// it there.
///
/// **The text has to be an integer.** [`Literal::Int`] is not only an
/// integer: `emit/decode.rs`'s `literal::<T>("false")` carries the
/// keyword in one, because that variant is the tree's raw-token escape
/// and `false` is what C, Java and TypeScript all spell a false boolean
/// with. Rewriting it here read `"false"` as "not the digit zero" and
/// declared `let zd_seen0: boolean = true` — every `@mutable` member
/// marked as already seen, so no member the stream omitted took
/// Table 9's value and the decoder answered with a plausible sample.
/// `crates/lang-rules/tests/typescript_reader_generates.rs` caught it by
/// type-checking what it generates:
/// `TS2367: This comparison appears to be unintentional because the
/// types 'true' and 'false' have no overlap`.
fn literal_as(target: &ts::Type, text: &str) -> Option<Literal> {
    let digits = text.strip_suffix('n').unwrap_or(text);
    let body = digits.strip_prefix('-').unwrap_or(digits);
    if body.is_empty() || !body.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    match target {
        ts::Type::Keyword(ts::Keyword::BigInt) => Some(Literal::Int(format!("{digits}n"))),
        ts::Type::Keyword(ts::Keyword::Boolean) => Some(Literal::Bool(digits != "0")),
        // A `char` or `wchar` discriminator. The label evaluated to a
        // code point and the member holds the character.
        ts::Type::Keyword(ts::Keyword::String) => digits
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(Literal::Char),
        ts::Type::Keyword(ts::Keyword::Number) | ts::Type::Named { .. } => {
            Some(Literal::Int(digits.to_string()))
        }
        _ => None,
    }
}

/// The TypeScript type of a value, structurally.
///
/// `number` and `bigint` by the boundary [`TypeScript::scalar`] draws,
/// an array for either collection, the declared name for an aggregate,
/// and the aliased type for a typedef — a `typedef` has no TypeScript
/// form of its own, and `type Ids = number[]` beside it would be a
/// second name for one type. TypeScript's types are structural, so a
/// codec that resolves through an alias and a member declared as the
/// alias are the same type.
///
/// Every named answer is the path from the **file root** and not the
/// declaration's own name. The materialiser nests a declaration in a
/// namespace per IDL module and a generated codec sits at the root, so
/// `Point` there names nothing; `module_prefix` answers the empty string
/// because a target with namespaces has no prefix to add, and that is
/// not the same as having nothing to qualify. Measured from both
/// directions before they were merged: `empty-struct`
/// (`module m { struct S {}; }`) through the write probe is
/// `error TS2304: Cannot find name 'S'`.
///
/// An enumeration, a bitmask and a bitset are named too and not spelled
/// `number`. A `@bit_bound(64)` bitmask is a `bigint` and a bitset is an
/// object with one property per bitfield, so `number` was wrong for two
/// of the three.
///
/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by `MAX_TYPE_DEPTH`, past which the spelling is
/// `unknown` — the same answer a type that does not resolve takes)
fn value_type_of(ast: &Ast, id: Id<TypeNode>, depth: usize) -> ts::Type {
    use zerodds_ast::{PrimitiveKind, TypeKind};
    let unknown = || ts::Type::Keyword(ts::Keyword::Unknown);
    if depth > MAX_TYPE_DEPTH {
        return unknown();
    }
    let Ok(node) = ast.type_(id) else {
        return unknown();
    };
    let declared = || {
        crate::adapter::typescript::declared_path(ast, id)
            .map_or_else(|_| unknown(), ts::Type::named)
    };
    match &node.kind {
        TypeKind::Primitive(kind) => match kind {
            PrimitiveKind::Boolean => TypeScript::scalar(Scalar::Bool),
            PrimitiveKind::Char | PrimitiveKind::WChar => TypeScript::scalar(Scalar::Char),
            PrimitiveKind::Int64 | PrimitiveKind::UInt64 => TypeScript::scalar(Scalar::Int64),
            PrimitiveKind::Float128 => TypeScript::scalar(Scalar::LongDouble),
            _ => TypeScript::scalar(Scalar::Int32),
        },
        TypeKind::String { .. } | TypeKind::Fixed { .. } => ts::Type::string(),
        // An enumeration is declared as `type E = number` beside its
        // `as const` object, and a bitmask as `type M = <holder>`. The
        // *declared name* and not the holder it aliases: the two are the
        // same type to `tsc` and only one of them says what the member
        // is.
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => declared(),
        // A bitset is an object with one property per named bitfield,
        // not an integer. The anchor is the codec's business and never
        // appears in the type — a `number` here made every bitset member
        // an assignment `tsc` refuses.
        TypeKind::Bitset { .. }
        | TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => declared(),
        TypeKind::Sequence { element, .. } => {
            let inner = element
                .resolved()
                .map_or_else(unknown, |target| value_type_of(ast, target, depth + 1));
            ts::Type::Array(Box::new(inner))
        }
        // One `[]` per dimension. `long a[2][3]` is `number[][]`, which
        // is what the materialiser declares and what the walk indexes
        // twice; a single `[]` here made every multi-dimensional array
        // a `number[]` whose elements were then indexed again.
        TypeKind::Array { element, dims } => {
            let mut out = element
                .resolved()
                .map_or_else(unknown, |target| value_type_of(ast, target, depth + 1));
            for _ in dims {
                out = ts::Type::Array(Box::new(out));
            }
            out
        }
        // DDS-TS 1.0 §7.9.1 makes a map a `ReadonlyMap<K', V'>` at the
        // TypeScript surface, which is what a member holds and what a
        // codec function therefore takes and answers with. The entry
        // array XTypes 1.3 §7.4.1.1.4 puts on the wire is the codec's
        // own staging and never leaves it; [`TypeScript::routine`]
        // converts at the two ends.
        TypeKind::Map { key, value, .. } => {
            let half = |half: &zerodds_ast_core::Ref<TypeNode>| {
                half.resolved()
                    .map_or_else(unknown, |target| value_type_of(ast, target, depth + 1))
            };
            ts::Type::generic(Path::new(&["ReadonlyMap"]), vec![half(key), half(value)])
        }
        TypeKind::Alias { aliased } => aliased
            .resolved()
            .map_or_else(unknown, |target| value_type_of(ast, target, depth + 1)),
        // A boxed value is a reference, absent when it is not there, and
        // `undefined` is what TypeScript's absent is.
        TypeKind::ValueBox { boxed } => {
            let inner = boxed
                .resolved()
                .map_or_else(unknown, |target| value_type_of(ast, target, depth + 1));
            ts::Type::Union(vec![inner, ts::Type::Keyword(ts::Keyword::Undefined)])
        }
        // No TypeScript data form at all; `adapter/typescript.rs`
        // refuses a member carrying one by name. `unknown` and not `any`
        // so that a value of it has to be narrowed before it is used.
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => unknown(),
    }
}
