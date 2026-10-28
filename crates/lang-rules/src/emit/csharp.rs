// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C# backend of the shared codec walk.
//!
//! The same walk as the C, C++, Java and TypeScript columns, spelled in
//! C#. `runtime/csharp` implements DDS-XTypes 1.3 §7.4 from the clause
//! and calls nothing in Rust, so where two columns disagree on an octet
//! one of them has misread the specification — which is the only reason
//! to have a sixth.
//!
//! # The four decisions this column turns on
//!
//! **The unsigned types are real.** This is the whole difference from
//! the Java backend, whose [`CodecTarget::scalar`] has to answer `int`
//! for a `uint8` and `long` for a `uint32` because Java has no unsigned
//! integer. Here every IDL width maps to a C# keyword of the same width
//! and the same signedness, and `ulong` holds `18446744073709551615` as
//! a number rather than as a pattern a caller has to decode.
//!
//! **An enumeration is a real `enum`.** `adapter/csharp.rs` declares
//! `enum Colour : int`, so an enumerator constant is
//! `global::a.Colour.RED` and not a bare identifier — and a C# `enum`
//! has no implicit conversion to an integer, which is why the walk's
//! `(long)` and `(ulong)` casts are kept here where the Java backend
//! drops them.
//!
//! **A member's C# name is folded and its enumerators are not.**
//! `adapter/csharp.rs` writes `venue` as `Venue` and carries the IDL
//! spelling in `[IdlName]`, because §7.3.1.2.1.1 computes a hashed
//! member ID from the IDL name. A type's name and an enumerator's are
//! verbatim, because a type's name reaches the wire.
//! [`CodecTarget::escape`] is therefore the *member* fold alone, and
//! the enumerator seam is [`CodecTarget::enumerator_constant`].
//!
//! **`@optional` is `null`, and a value type's `T?` is a different
//! type from `T`.** That is why [`CodecTarget::optional_value`] takes
//! the member's type: `int?` does not convert to `int` on its own, and
//! a `string?` has no `.Value` to take. No other column needed the
//! parameter and this one cannot be written without it.
//!
//! # `long double` and `fixed`, and what they are not
//!
//! A `long double` is `global::ZeroDds.Idl.Float128`, a sixteen-octet
//! carrier. `decimal` is not a binary128 and is not close to one: it is
//! base ten, has 28 to 29 significant digits against binary128's 34, has
//! no exponent range past ±28 against ±4932, and has no signed zero, no
//! infinity and no NaN.
//!
//! A `fixed<P,S>` is a `string` holding the decimal, for the reason
//! `adapter/csharp.rs` gives and the corpus measures: `fixed-scales`
//! declares `fixed<31,10>` and its value
//! `-12345678901234567890.1234567890` has thirty significant digits,
//! which a `decimal` rounds. The `FixedDigitCount::ValuePrecision` axis
//! exists to measure how many digits a peer writes into the BCD run,
//! and a holder that normalises its leading zeros away has already
//! decided that answer.

use super::{BitPatternSource, CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::csharp::{
    csharp_member_type, declared_path, field_name, map_entry_name, type_tag, unsigned_holder,
};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::csharp;

/// The C# backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CSharp;

impl CSharp {
    /// `write_u32` as `WriteU32`.
    ///
    /// The runtime's methods are named by .NET's convention and the
    /// walk asks in the wire's, so one of the two has to convert. Doing
    /// it here means a new operation is added to the walk under one
    /// name. `PascalCase` and not the Java backend's `camelCase`,
    /// because that is what .NET names a public method.
    fn pascal(method: &str) -> String {
        let mut out = String::new();
        let mut rising = true;
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

    /// A dotted name as a path, keeping a `global::` root where one is
    /// written.
    fn path(name: &str) -> Path {
        match name.strip_prefix("global::") {
            Some(rest) => Path::rooted(&rest.split('.').collect::<Vec<_>>()),
            None => Path::new(&name.split('.').collect::<Vec<_>>()),
        }
    }

    /// A reader's body, with the sample it answers with around it.
    ///
    /// `<T> v = zd_def_<T>(); … return v;`, exactly as the Java
    /// backend does and for the same two reasons: the sample starts at
    /// Table 9's value so a position the stream does not carry is
    /// already what DDS-XTypes 1.3 says it is, and the defaulter's name
    /// is the decoder's with the verb changed so the two cannot name
    /// different types.
    fn reading_body(name: &str, sample: csharp::Type, body: csharp::Block) -> csharp::Block {
        let defaulter = crate::default_csharp::defaulter_from_decoder(name);
        let mut statements = vec![csharp::Statement::Declare {
            type_: sample,
            name: Ident::new("v"),
            value: Some(Self::call(&defaulter, Vec::new())),
        }];
        statements.extend(body.statements);
        statements.push(csharp::Statement::Return(Some(Self::whole("v"))));
        csharp::Block { statements }
    }

    /// A type split into its innermost element and how many `[]` it
    /// carries.
    ///
    /// `new int[3][]` is how C# allocates an `int[][]`, and
    /// `new int[][3]` is not an expression. So an allocation needs the
    /// element and the rank apart, which is what
    /// [`csharp::Expr::NewArray::nested`] takes.
    pub(crate) fn peel(type_: csharp::Type) -> (csharp::Type, usize) {
        let mut nested = 0;
        let mut current = type_;
        // Bounded by the materialiser's own `MAX_TYPE_DEPTH`; a chain
        // longer than this did not come from a model it accepted.
        for _ in 0..64 {
            match current {
                csharp::Type::Array { element, rank } => {
                    nested += rank;
                    current = *element;
                }
                other => return (other, nested),
            }
        }
        (current, nested)
    }

    /// Whether a type is one C# converts to and from with a cast.
    ///
    /// Every keyword type but `string` and `object`. A generated
    /// `Float128` is not one, and a cast to it would not compile.
    fn is_castable(type_: &csharp::Type) -> bool {
        match type_ {
            csharp::Type::Predefined(kind) => !matches!(
                kind,
                csharp::Predefined::String
                    | csharp::Predefined::Object
                    | csharp::Predefined::Void
                    | csharp::Predefined::Dynamic
            ),
            _ => false,
        }
    }
}

impl CodecTarget for CSharp {
    type Expr = csharp::Expr;
    type Stmt = csharp::Statement;
    type Block = csharp::Block;
    type Type = csharp::Type;
    type Decl = csharp::Member;

    // -- types --------------------------------------------------------

    /// One IDL width, one C# keyword, same signedness.
    ///
    /// The two counters are the exception and neither is a wire width.
    /// [`Scalar::Index`] is `int`, which is what a C# array's `Length`
    /// is and what `new T[n]` and a subscript take. [`Scalar::Size`] is
    /// `long`, and that is a correctness decision rather than a
    /// spelling: a sequence's declared count is a `uint`, the reader
    /// checks it against `ReaderRemaining()` before allocating, and an
    /// `int` there would make `0xFFFFFFFF` compare as `-1` and pass the
    /// check it exists to fail.
    fn scalar(kind: Scalar) -> Self::Type {
        csharp::Type::Predefined(match kind {
            Scalar::Bool => csharp::Predefined::Bool,
            Scalar::Char => csharp::Predefined::Char,
            Scalar::Int8 => csharp::Predefined::SByte,
            Scalar::UInt8 => csharp::Predefined::Byte,
            Scalar::Int16 => csharp::Predefined::Short,
            Scalar::UInt16 => csharp::Predefined::UShort,
            Scalar::Index | Scalar::Int32 => csharp::Predefined::Int,
            Scalar::UInt32 => csharp::Predefined::UInt,
            Scalar::Size | Scalar::Int64 => csharp::Predefined::Long,
            Scalar::UInt64 => csharp::Predefined::ULong,
            Scalar::Float => csharp::Predefined::Float,
            Scalar::Double => csharp::Predefined::Double,
            // binary128 has no C# primitive and `decimal` is not one:
            // base ten, 28 digits, no infinity and no signed zero.
            Scalar::LongDouble => {
                return csharp::Type::named(Path::rooted(&["ZeroDds", "Idl", "Float128"]));
            }
        })
    }

    fn named(name: &str) -> Self::Type {
        csharp::Type::named(Self::path(name))
    }

    fn nothing() -> Self::Type {
        csharp::Type::Predefined(csharp::Predefined::Void)
    }

    /// `runtime/csharp` publishes each handle as a type of its own in
    /// `ZeroDds.Wire`, and every reference to one is **rooted**.
    ///
    /// Not decoration. C# resolves a simple name against the global
    /// namespace, where every generated type sits, so a corpus case
    /// declaring `enum Status` puts its own type in front of the
    /// runtime's `Status` and `Status.Discarded` becomes CS0117 —
    /// measured on `enum-sparse-values` and its unannotated twin, which
    /// declare exactly that. `global::` resolves at the global namespace
    /// and nothing in any scope can capture it, which is the same reason
    /// `adapter/csharp.rs` roots every path it writes.
    fn handle(kind: Handle) -> Self::Type {
        csharp::Type::named(Path::rooted(&[
            "ZeroDds",
            "Wire",
            match kind {
                Handle::DHeader => "DHeader",
                Handle::EmHeader => "EmHeader",
                Handle::Pid => "Pid",
                Handle::TryConstruct => "TryConstruct",
                Handle::ReaderMark => "ReaderMark",
                Handle::PidRead => "PidRead",
                Handle::EmHeaderRead => "EmHeaderRead",
            },
        ]))
    }

    /// A C# `char` is one UTF-16 code unit, which is what the runtime's
    /// `WriteChar` and `WriteWchar` take. There is nothing to widen and
    /// nothing to narrow, so the value goes through: the narrowing an
    /// IDL `char` needs is the runtime's, which refuses a code unit
    /// above U+00FF rather than keeping its low byte.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// Statics on the runtime's writer, rooted like every other runtime
    /// reference — [`CodecTarget::runtime_static`] names the *reader*
    /// for everything but `no_dheader`, so the default spelling would
    /// call the wrong type.
    fn bit_pattern(source: BitPatternSource, value: Self::Expr) -> Self::Expr {
        let call = |name: &str, args: Vec<csharp::Expr>| csharp::Expr::Invoke {
            receiver: Some(Box::new(csharp::Expr::Path(Path::rooted(&[
                "ZeroDds", "Wire", "Writer",
            ])))),
            name: Ident::new(name),
            args,
        };
        match source {
            BitPatternSource::Int { octets } => call(
                "BitPatternInt",
                vec![
                    value,
                    csharp::Expr::Literal(Literal::int(i128::from(octets))),
                ],
            ),
            BitPatternSource::Bool => call("BitPatternBool", vec![value]),
            BitPatternSource::Float32 => call("BitPatternF32", vec![value]),
            BitPatternSource::Float64 => call("BitPatternF64", vec![value]),
        }
    }

    /// `runtime/csharp` publishes these as enum members, one type per
    /// policy, and every reference is rooted for the reason
    /// [`CodecTarget::handle`] gives.
    fn constant(which: Constant) -> Self::Expr {
        let mut segments: Vec<&str> = vec!["ZeroDds", "Wire"];
        segments.extend(
            match which {
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
                Constant::WstringSpecificationWideUnits => {
                    "WstringLengthPolicy.SpecificationWideUnits"
                }
                Constant::WstringRosTypeSupport => "WstringLengthPolicy.RosTypeSupport",
                Constant::WstringOctetsTerminated2 => "WstringLengthPolicy.OctetsTerminated2",
                Constant::WstringOctetsTerminated4 => "WstringLengthPolicy.OctetsTerminated4",
                Constant::WstringCharactersTerminated2 => {
                    "WstringLengthPolicy.CharactersTerminated2"
                }
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
            }
            .split('.'),
        );
        csharp::Expr::Path(Path::rooted(&segments))
    }

    /// The materialiser's answer, not a second reading of it.
    ///
    /// `adapter/csharp.rs::unsigned_holder` decides what a
    /// `@bit_bound` set's field is declared as, and Table 33's four
    /// widths all have a C# keyword — which is where this parts from
    /// Java, whose holder can only be `int` or `long`.
    fn unsigned_holder(bits: u16) -> Self::Type {
        csharp::Type::Predefined(unsigned_holder(bits))
    }

    /// C# has no free functions; the runtime class's own static method
    /// stands in.
    ///
    /// Which class is not a guess: `no_dheader` is the writer's and
    /// everything else is the reader's. A name this table does not know
    /// is written on `Reader` and `csc` names it — a compile error that
    /// stops the probe, rather than a call that goes to the wrong class
    /// and cannot.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let receiver = if method == "no_dheader" {
            "Writer"
        } else {
            "Reader"
        };
        csharp::Expr::Invoke {
            receiver: Some(Box::new(csharp::Expr::Path(Path::rooted(&[
                "ZeroDds", "Wire", receiver,
            ])))),
            name: Ident::new(&Self::pascal(method)),
            args,
        }
    }

    // -- expressions --------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        csharp::Expr::Path(Self::path(text))
    }

    fn int(value: i128) -> Self::Expr {
        csharp::Expr::Literal(Literal::int(value))
    }

    fn literal(value: Literal) -> Self::Expr {
        csharp::Expr::Literal(value)
    }

    /// A free function is a `static` method of the generated class, so
    /// an unqualified call reaches it from another method of the same
    /// class without naming the class.
    /// `strcmp` is the walk's name for a three-way code-unit
    /// comparison; here it is `string.CompareOrdinal`, which compares
    /// UTF-16 code units — the Java column's caveat about
    /// supplementary code points applies unchanged.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            return csharp::Expr::Invoke {
                receiver: Some(Box::new(csharp::Expr::Path(Path::new(&["string"])))),
                name: Ident::new("CompareOrdinal"),
                args,
            };
        }
        csharp::Expr::Invoke {
            receiver: None,
            name: Ident::new(callee),
            args,
        }
    }

    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        csharp::Expr::Invoke {
            receiver: Some(Box::new(csharp::Expr::Path(Self::path(receiver)))),
            name: Ident::new(&Self::pascal(method)),
            args,
        }
    }

    /// `runtime/csharp` carries the three HDDS axes: `BeginPackedElement`
    /// and `FinishPackedElement` on the writer and the reader, the
    /// DHEADER pair for a sequence element's own length, and
    /// `WriteWstringUtf8Octets` / `ReadWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    fn member(base: &str, field: &str) -> Self::Expr {
        csharp::Expr::Member {
            value: Box::new(csharp::Expr::Path(Self::path(base))),
            name: Ident::new(field),
        }
    }

    /// The sample itself, with no dereference to write.
    fn whole(base: &str) -> Self::Expr {
        csharp::Expr::Path(Self::path(base))
    }

    /// The suffix that keeps a literal's type from being the
    /// compiler's guess.
    ///
    /// ECMA-334 §6.4.5.3 gives an unsuffixed decimal the first of
    /// `int`, `uint`, `long`, `ulong` that holds it, which is already
    /// the right answer for every value — but a `long` label compared
    /// against a `long` discriminator wants the width stated, and
    /// `16045690984833335998` past `long.MaxValue` has to be a `ulong`
    /// where the discriminator is one. The suffix says which, so the
    /// comparison is not decided by the operand the compiler happened
    /// to widen.
    fn wide_int(value: i128) -> Self::Expr {
        let text = if value > i128::from(i64::MAX) {
            format!("{value}UL")
        } else if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
            format!("{value}L")
        } else {
            format!("{value}")
        };
        csharp::Expr::Literal(Literal::Int(text))
    }

    /// A count, wide enough to hold itself and with no `u` suffix.
    ///
    /// C's `5u` is not a C# token; `5U` is, and it is not wanted: a
    /// bound compared against an `int` counter would force the whole
    /// comparison into `long` for no reason. The bounds past `int` get
    /// their `L` from [`CodecTarget::wide_int`].
    fn unsigned_int(value: i128) -> Self::Expr {
        Self::wide_int(value)
    }

    /// A 64-bit mask, as the `ulong` it is.
    ///
    /// Where the Java backend has to write the signed pattern because
    /// Java has no unsigned integer, this is the number with a `UL` on
    /// it — and `~0xFFFFFFFFFFFFFFFFUL` is an unsigned complement
    /// rather than an arithmetic one.
    fn wide_unsigned(value: u64) -> Self::Expr {
        csharp::Expr::Literal(Literal::Int(format!("{value}UL")))
    }

    /// A signed 64-bit constant, always suffixed: an enumerator's
    /// pattern is compared against a `long` and an unsuffixed literal
    /// there would be widened after the comparison's type was chosen.
    fn wide_signed(value: i128) -> Self::Expr {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let pattern = if value > i128::from(i64::MAX) {
            value as u64 as i64
        } else {
            value as i64
        };
        csharp::Expr::Literal(Literal::Int(format!("{pattern}L")))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        csharp::Expr::Member {
            value: Box::new(value),
            name: Ident::new(field),
        }
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        csharp::Expr::Index {
            value: Box::new(value),
            index: Box::new(at),
        }
    }

    /// `(uint)v.Length`, or the bound itself where the materialiser
    /// fixed one.
    ///
    /// The cast is the crossing and is written out. A C# array's
    /// `Length` is an `int`; a sequence's count is a `uint` on the wire
    /// and `Writer.WriteU32` takes one, and there is no implicit
    /// `int` to `uint` conversion. A backend that let the runtime take
    /// a wider signed type instead would be moving a range check out of
    /// the compiler and into a throw.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => csharp::Expr::Literal(Literal::int(i128::from(bound))),
            None => csharp::Expr::Cast {
                type_: Box::new(csharp::Type::Predefined(csharp::Predefined::UInt)),
                value: Box::new(csharp::Expr::Member {
                    value: Box::new(Self::whole(base)),
                    name: Ident::new("Length"),
                }),
            },
        }
    }

    /// The operators as given, except a shift's count: C# takes only an
    /// `int` there (§12.11 in the C# standard names no `ulong` form),
    /// and the walk writes the count in the accumulator's own width —
    /// `zd_bits >> 3UL` is `CS0019`. A literal count sheds its suffix,
    /// any other spelling is cast.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        let rhs = if matches!(op, "<<" | ">>") {
            match rhs {
                csharp::Expr::Literal(Literal::Int(text)) => csharp::Expr::Literal(Literal::Int(
                    text.trim_end_matches(['L', 'U', 'l', 'u']).to_string(),
                )),
                other => csharp::Expr::Cast {
                    type_: Box::new(csharp::Type::Predefined(csharp::Predefined::Int)),
                    value: Box::new(csharp::Expr::Paren(Box::new(other))),
                },
            }
        } else {
            rhs
        };
        csharp::Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// The operator as written. C# has a real unary form for `!`, `~`
    /// and `-`, so none of them needs the `0 <op> a` stand-in the Java
    /// backend writes.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        csharp::Expr::Unary {
            op,
            value: Box::new(value),
        }
    }

    /// A bracket, written because the walk asked for one.
    ///
    /// The renderer adds none of its own, and C#'s precedence is Java's
    /// where it matters here: `&` binds looser than `!=`, so
    /// `bits & mask != 0` is a type error rather than the comparison
    /// the walk built.
    fn paren(value: Self::Expr) -> Self::Expr {
        csharp::Expr::Paren(Box::new(value))
    }

    /// A conversion, kept where Java drops it.
    ///
    /// Two reasons this column writes more casts than that one. A C#
    /// `enum` has no implicit conversion to an integer, so the walk's
    /// `(long)v` on an enumerator and `(ulong)v` on a bitmask are
    /// load-bearing. And `csc` has no redundant-cast diagnostic, so
    /// keeping an identity cast costs nothing where `-Xlint:cast
    /// -Werror` fails the Java build over one.
    ///
    /// The one rewrite is the `bool` discriminator. A union's labels
    /// arrive as the integers the IDL evaluator gives, and C# has no
    /// conversion from `1` to `true`; the label is the value it stands
    /// for.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == csharp::Type::Predefined(csharp::Predefined::Bool) {
            if let csharp::Expr::Literal(Literal::Int(text)) = &value {
                return csharp::Expr::Literal(Literal::Bool(
                    text.trim_end_matches(['L', 'U', 'l', 'u']) != "0",
                ));
            }
        }
        // The walk's `signed char` — the C spelling of a map key's
        // signed order — is `sbyte` here: the cast keeps the low octet
        // and reads it in two's complement, which is the order the
        // corpus was recorded under.
        if type_ == csharp::Type::named(Self::path("signed char")) {
            return csharp::Expr::Cast {
                type_: Box::new(csharp::Type::Predefined(csharp::Predefined::SByte)),
                value: Box::new(value),
            };
        }
        csharp::Expr::Cast {
            type_: Box::new(type_),
            value: Box::new(value),
        }
    }

    /// `false` before `true`, spelled as the conjunction — C#'s `<`
    /// does not take two booleans (`CS0019`).
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        Self::binary(
            "&&",
            csharp::Expr::Paren(Box::new(csharp::Expr::Binary {
                op: "==",
                lhs: Box::new(left),
                rhs: Box::new(csharp::Expr::Literal(Literal::Bool(false))),
            })),
            right,
        )
    }

    /// Nothing to take an address of and no array to decay: both forms
    /// hand the value.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    // -- statements ---------------------------------------------------

    /// A local, with `= default` where the walk asked for no value.
    ///
    /// The walk declares a header handle before the loop that fills it
    /// — `EmHeaderRead zd_h;` — and C#'s definite-assignment rules make
    /// a local read before its first assignment a compile error, which
    /// a loop body the compiler cannot prove runs would be. `default`
    /// on a `readonly struct` costs nothing and settles it.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        csharp::Statement::Declare {
            type_,
            name: Ident::new(name),
            value: Some(value.unwrap_or(csharp::Expr::Default(None))),
        }
    }

    fn perform(value: Self::Expr) -> Self::Stmt {
        csharp::Statement::Expr(value)
    }

    /// `return v;` where the walk asked for a bare `return`.
    ///
    /// Only the reader leaves a routine early, and a C# decoder answers
    /// with the sample it has built rather than writing through a
    /// parameter it would have to take by `ref`. A bare `return;` would
    /// leave the caller with whatever it had before, which is the
    /// silent-wrong-value shape.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        csharp::Statement::Return(Some(value.unwrap_or_else(|| Self::whole("v"))))
    }

    /// `_ = name;`, a discard (C# 7.0): a read the language accepts as
    /// a statement, where `name;` is not one. `csc` has no unread-parameter
    /// diagnostic, but a local that is assigned and never read is CS0219,
    /// and the probe treats warnings as errors: under OpenDDS's XCDR1
    /// framing a union writes no sentinel and nothing reads `zd_sent`
    /// (`union-mutable`, `evolution-union-unselected`, 4 matrix cells).
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![csharp::Statement::Expr(csharp::Expr::Assign {
            target: Box::new(Self::name("_")),
            value: Box::new(Self::name(name)),
        })]
    }

    /// The call on its own. An invocation is already a C# statement,
    /// and `(void) expr;` is not one at all.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        csharp::Statement::Expr(value)
    }

    /// `place = zd_dec_T(r, behaviour);`
    ///
    /// A decoder answers with its value rather than filling a
    /// destination. C# has `ref` and `out`, and neither is usable here:
    /// a property cannot be passed as either (CS0206), an array element
    /// of a `Nullable<T>` member cannot be an `out` of `T`, and the
    /// walk's destinations are expressions rather than variables.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        csharp::Statement::Expr(csharp::Expr::Binary {
            op: "=",
            lhs: Box::new(place),
            rhs: Box::new(Self::call(callee, vec![Self::name("r"), behaviour])),
        })
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

    /// A shorter array, because a C# array's length is fixed at its
    /// allocation.
    ///
    /// §7.3.1.2.1.12.3's TRIM. C leaves the elements past the bound in
    /// the buffer and moves the count, and there is no count here to
    /// move; `Array.Resize` takes a `ref` and the walk's base is an
    /// expression, so the shortening is a copy. Written through
    /// `System.Linq` rather than as three statements with a temporary,
    /// because a temporary would need a name no two calls in one method
    /// could share.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        let take = csharp::Expr::Invoke {
            receiver: Some(Box::new(csharp::Expr::Path(Path::rooted(&[
                "System",
                "Linq",
                "Enumerable",
            ])))),
            name: Ident::new("Take"),
            args: vec![
                Self::whole(base),
                csharp::Expr::Cast {
                    type_: Box::new(csharp::Type::Predefined(csharp::Predefined::Int)),
                    value: Box::new(count),
                },
            ],
        };
        vec![csharp::Statement::Expr(csharp::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::whole(base)),
            rhs: Box::new(csharp::Expr::Invoke {
                receiver: Some(Box::new(csharp::Expr::Path(Path::rooted(&[
                    "System",
                    "Linq",
                    "Enumerable",
                ])))),
                name: Ident::new("ToArray"),
                args: vec![take],
            }),
        })]
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        csharp::Statement::If {
            condition,
            then,
            otherwise,
        }
    }

    /// `for (int i = 0; i < bound; i = i + 1)`.
    ///
    /// The step is written out rather than `i++` for the same reason
    /// the C and Java backends write it out: the tree has no increment
    /// node, and one spelling in every backend keeps the columns'
    /// generated source comparable by eye when their octets disagree.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        csharp::Statement::For {
            init: Box::new(csharp::Statement::Declare {
                type_: csharp::Type::Predefined(csharp::Predefined::Int),
                name: Ident::new(counter),
                value: Some(csharp::Expr::Literal(Literal::int(0))),
            }),
            condition: csharp::Expr::Binary {
                op: "<",
                lhs: Box::new(csharp::Expr::Path(Path::new(&[counter]))),
                rhs: Box::new(bound),
            },
            step: csharp::Expr::Binary {
                op: "=",
                lhs: Box::new(csharp::Expr::Path(Path::new(&[counter]))),
                rhs: Box::new(csharp::Expr::Binary {
                    op: "+",
                    lhs: Box::new(csharp::Expr::Path(Path::new(&[counter]))),
                    rhs: Box::new(csharp::Expr::Literal(Literal::int(1))),
                }),
            },
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        csharp::Statement::While { condition, body }
    }

    fn leave() -> Self::Stmt {
        csharp::Statement::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        csharp::Statement::Comment(doc)
    }

    // -- assembly -----------------------------------------------------

    /// The array, sized — and its slots given something to be where the
    /// fill writes into them rather than assigning them.
    ///
    /// `new T[n]` throws rather than returning null when it cannot be
    /// served, so there is no guard and the fill follows
    /// unconditionally. `new T[n]` takes the count as an `int`, a
    /// `uint`, a `long` or a `ulong`, so the `uint` the reader read is
    /// handed over with no cast at all.
    ///
    /// `new T[n]` of a class type is `n` nulls. A sequence's element
    /// decoder answers with a value and the slot is assigned outright,
    /// so nothing is needed; a `map`'s two halves are written into the
    /// entry at `v[i]`, and an entry that was never made is a null
    /// dereference at the first key. That is what `assigned_whole`
    /// separates.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let (innermost, nested) = Self::peel(element.clone());
        let mut out = vec![csharp::Statement::Expr(csharp::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::whole(base)),
            rhs: Box::new(csharp::Expr::NewArray {
                element: Box::new(innermost),
                length: Some(Box::new(count)),
                elements: Vec::new(),
                nested,
            }),
        })];
        if !assigned_whole {
            out.push(Self::count_up(
                "zd_e",
                Self::count_of(base, None),
                csharp::Block {
                    statements: vec![csharp::Statement::Expr(csharp::Expr::Binary {
                        op: "=",
                        lhs: Box::new(csharp::Expr::Index {
                            value: Box::new(Self::whole(base)),
                            index: Box::new(Self::name("zd_e")),
                        }),
                        rhs: Box::new(csharp::Expr::New {
                            type_: Box::new(element),
                            args: Vec::new(),
                            initialiser: Vec::new(),
                        }),
                    })],
                },
            ));
        }
        out.extend(fill);
        out
    }

    /// The scalar into the place, cast to the place's own type.
    ///
    /// One cast and never two: the walk has already asked for a width
    /// at one site — the element form of a `wchar` under TAO's framing
    /// asks for `ushort` — and a `ushort` does not convert to a `char`
    /// on its own. The outer cast is unwrapped and rewritten to the
    /// field's type, which is the same thing the Java backend does for
    /// its three narrow holders.
    ///
    /// A `Float128` place takes the value unchanged: it is a generated
    /// struct and a cast to it does not compile.
    fn store(place: Self::Expr, type_: Self::Type, value: Self::Expr) -> Self::Stmt {
        if !Self::is_castable(&type_) {
            return csharp::Statement::Expr(csharp::Expr::Binary {
                op: "=",
                lhs: Box::new(place),
                rhs: Box::new(value),
            });
        }
        let inner = match value {
            csharp::Expr::Cast { value, .. } => *value,
            other => other,
        };
        csharp::Statement::Expr(csharp::Expr::Binary {
            op: "=",
            lhs: Box::new(place),
            rhs: Box::new(csharp::Expr::Cast {
                type_: Box::new(type_),
                value: Box::new(inner),
            }),
        })
    }

    /// `<T> name = new <T>();` — a C# local of a class type is a
    /// reference to nothing until one is made.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        csharp::Statement::Declare {
            type_: type_.clone(),
            name: Ident::new(name),
            value: Some(csharp::Expr::New {
                type_: Box::new(type_),
                args: Vec::new(),
                initialiser: Vec::new(),
            }),
        }
    }

    /// `runtime/csharp` publishes a header's parts as fields and
    /// properties in .NET's naming, so the field the walk names is
    /// re-cased here.
    ///
    /// A `match` and not a general re-casing: `end` is `End` and
    /// `extent` is a computed property, so a rule that only capitalised
    /// would name two members that are not there. A name this table
    /// does not know is written as it came, and `csc` reports it.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        csharp::Expr::Member {
            value: Box::new(value),
            name: Ident::new(match field {
                "pid" => "Pid",
                "end" => "End",
                "ignore" => "Ignore",
                "length" => "Length",
                "member_id" => "MemberId",
                "must_understand" => "MustUnderstand",
                "extent" => "Extent",
                other => other,
            }),
        }
    }

    /// Absence is the value, so presence is a comparison and not a
    /// second field. `null` works for both halves of the mapping: a
    /// reference type is nullable already and a value type's `T?` is a
    /// `Nullable<T>`, which compares against `null`.
    fn present(base: &str, field: &str) -> Self::Expr {
        csharp::Expr::Binary {
            op: "!=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        }
    }

    /// Nothing on the way in — assigning the member is what makes it
    /// present — and `null` on the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![csharp::Statement::Expr(csharp::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        })]
    }

    /// One field per case, directly on the class.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `Key` and `Value`, which is what `fields_of` folded the entry
    /// class's two IDL names to.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        csharp::Expr::Member {
            value: Box::new(entry),
            name: Ident::new(if key { "Key" } else { "Value" }),
        }
    }

    /// The member, unwrapped where the mapping wrapped it.
    ///
    /// A value type's `@optional` member is a `Nullable<T>` and does
    /// not convert to `T` on its own, so the write side takes
    /// `.Value` — reached only inside the branch the presence test
    /// guarded, which is where `Nullable<T>.Value` is defined. A
    /// reference type was never wrapped and is handed over as it is.
    fn optional_value(ast: &Ast, id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        if crate::adapter::csharp::is_nullable_wrapped(ast, id) {
            return csharp::Expr::Member {
                value: Box::new(Self::member(base, field)),
                name: Ident::new("Value"),
            };
        }
        Self::member(base, field)
    }

    /// `v[i]`, and nothing around it: `adapter/csharp.rs` gives a
    /// sequence a `T[]`, whose elements are real slots at every element
    /// type.
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

    /// The member itself. Assigning to it is what makes it present, and
    /// a `T` assigned to a `T?` is the implicit conversion C# already
    /// has, so there is nothing to do first.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    fn elements(base: &str) -> Self::Expr {
        Self::whole(base)
    }

    fn absent() -> Self::Expr {
        csharp::Expr::Literal(Literal::Null)
    }

    /// `!= null`, because a `string` and a `T[]` are references here.
    fn materialised(value: Self::Expr) -> Option<Self::Expr> {
        Some(csharp::Expr::Binary {
            op: "!=",
            lhs: Box::new(value),
            rhs: Box::new(Self::absent()),
        })
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        csharp::Block { statements }
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block.statements
    }

    /// `private static void EncodeT(Writer w, T v)`.
    ///
    /// A prototype gets `None`: a C# method may call one declared below
    /// it, so there is nothing to declare ahead. `tolerate_unused` is
    /// dropped for the same kind of reason — `csc` does not warn on an
    /// uncalled private method under the settings the probe builds
    /// with, so there is no warning to suppress.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![csharp::Param {
            attributes: Vec::new(),
            modifier: None,
            type_: csharp::Type::named(Path::rooted(&[
                "ZeroDds",
                "Wire",
                if spec.writing { "Writer" } else { "Reader" },
            ])),
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            default: None,
        }];
        // The writer is handed the sample; the reader answers with one.
        // See `CodecTarget::read_call`.
        if spec.writing {
            params.push(csharp::Param {
                attributes: Vec::new(),
                modifier: None,
                type_: spec.sample.clone(),
                name: Ident::new("v"),
                default: None,
            });
        }
        params.extend(spec.extra.into_iter().map(|(type_, name)| csharp::Param {
            attributes: Vec::new(),
            modifier: None,
            type_,
            name: Ident::new(&name),
            default: None,
        }));
        let (returns, body) = if spec.writing {
            (Self::nothing(), body)
        } else {
            (
                spec.sample.clone(),
                Self::reading_body(&spec.name, spec.sample, body),
            )
        };
        Some(csharp::Member::Method(csharp::Method {
            doc: spec.doc,
            attributes: Vec::new(),
            // `public` and not `private`. `ZdCodec` is a `public
            // static class` and a generated codec's whole purpose is to
            // be called from the application that asked for it; every
            // method in it was `private`, so an assembly holding this
            // file could not call one. The probe harness never noticed:
            // it emits its `Main` into the same class.
            modifiers: vec![csharp::Modifier::Public, csharp::Modifier::Static],
            returns,
            name: Ident::new(&spec.name),
            generics: Vec::new(),
            params,
            constraints: Vec::new(),
            body: Some(body),
            expression: None,
        }))
    }

    // -- the materialiser's boundary ----------------------------------

    /// The materialiser's spelling, rooted: `global::a.b.Point`.
    ///
    /// Not [`CodecTarget::type_name`], which is the codec's tag —
    /// `a_b_Point` — and names a method rather than a type.
    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        csharp_member_type(ast, id, "codec").unwrap_or_else(|_| {
            csharp::Type::named(Path::new(
                &[format!("UnresolvedType{}", id.slot()).as_str()],
            ))
        })
    }

    /// The C# materialiser's tag for a type.
    ///
    /// The fallback carries the arena slot, and that is the point of
    /// it: `type_tag`'s own answer for an unresolvable type is the bare
    /// word `unknown`, and two of those in one model would be two types
    /// sharing one codec — a member written by the wrong encoder, with
    /// nothing red anywhere.
    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0).unwrap_or_else(|_| format!("unresolved_type_{}", id.slot()))
    }

    /// The member fold, and only that.
    ///
    /// `venue` to `Venue`, with a C# keyword escaped to `@venue`. Not
    /// an enumerator's name, which the materialiser leaves verbatim —
    /// [`CodecTarget::enumerator_constant`] is that seam.
    fn escape(name: &str) -> String {
        field_name(name)
    }

    /// `global::ZdEntry_<key>_<value>`.
    ///
    /// The entry class is declared at the global scope and every
    /// reference this column writes is rooted, for the reason
    /// `adapter/csharp.rs` gives: a nested namespace can capture the
    /// first segment of an unrooted path.
    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        map_entry_name(ast, id).map_or_else(
            |_| format!("UnresolvedEntry{}", id.slot()),
            |name| format!("global::{name}"),
        )
    }

    /// C# has namespaces, so a module needs no prefix in a name.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// The member's own `@default`, or Table 9's value for its type.
    ///
    /// # Errors
    /// As [`crate::default_csharp::member_default`].
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_csharp::member_default(ast, node, type_, place)
    }

    /// `place = zd_def_<T>();` — Table 9, through the generated
    /// defaulter.
    ///
    /// # Errors
    /// Never here. Whether the type *has* a C# default is decided where
    /// the defaulter is built, and refused there by name.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_csharp::default_into(ast, id, place))
    }

    /// `global::<modules>.<Enum>.<LITERAL>`.
    ///
    /// The materialiser declares a real `enum` and leaves the
    /// enumerator's name verbatim; a bare `RED` names nothing outside
    /// the enum's own scope, and `Self::escape` would fold it into a
    /// member name the enum does not have.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        crate::default_csharp::enumerator_constant(ast, id, enumerator)
    }
}

/// Where the C# materialiser puts a declaration, as a rooted name.
///
/// Re-exported so the probe generator can name a class without a second
/// derivation of the module chain.
///
/// # Errors
/// As [`declared_path`].
pub fn declared_type_path(
    ast: &Ast,
    id: Id<TypeNode>,
) -> Result<String, crate::adapter::cst::MaterialiseError> {
    declared_path(ast, id)
}
