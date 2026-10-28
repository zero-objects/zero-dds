// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Kotlin backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as every other column,
//! and none of them calls another. Kotlin shares the JVM's value model
//! with [`crate::emit::java`] — no unsigned types, no pointers, a
//! decoder answers with its value rather than writing through an
//! out-parameter — so most of this file repeats that column's answer.
//! What is Kotlin's own:
//!
//! # No exceptions to declare, and no unused-parameter statement either
//!
//! Kotlin has no checked exceptions (Kotlin language specification,
//! "Exceptions"), so a fallible runtime call is written exactly as a
//! plain one — [`Kotlin::call`] and [`Kotlin::runtime`] add nothing —
//! and [`kotlin::FuncDecl`] carries no `throws`. [`Kotlin::mark_used`]
//! answers with nothing for the same reason
//! [`crate::emit::java::Java::mark_used`] does: `kotlinc` raises no
//! error over an unread parameter, so there is nothing to suppress.
//!
//! # Every numeric conversion is written out
//!
//! Kotlin performs **no** implicit numeric widening at all — not even
//! `Int` to `Long` (Kotlin language specification, "Integer types":
//! "there are no implicit widening conversions for numbers"), which is
//! narrower than Java's own rule and the reverse of C's. `readI8()`
//! answers an `Int` and a `Byte` field needs `.toByte()` explicitly, so
//! the shared walk's every `cast` has to reach the source here where
//! `emit/java.rs` could let the compiler do it.
//!
//! The bookkeeping runs the other way all the same. `kotlinc -Werror`
//! fails a build over "redundant call of conversion method" wherever a
//! conversion is called on a value of the target type already —
//! `a.toInt()` for an `Int` `a` is measured, exactly as `javac`'s
//! `-Xlint:cast` fails a redundant cast — so [`Kotlin::cast`] does skip
//! one, and `Kotlin::already` is the small type oracle that decides
//! when. That oracle is the whole of this backend's type knowledge: the
//! walk carries no symbol table, so what can be recognised is a literal,
//! an array's `.size`, a runtime call whose return type
//! `runtime/kotlin/ZdWire.kt` fixes, and the walk's own locals.
//!
//! # `Array(n) { T() }` needs no separate pre-fill loop
//!
//! `emit/java.rs`'s `make_room` allocates `new T[n]` — every reference
//! slot `null` — and then loops constructing an instance at each slot
//! only where the fill does not assign the slot outright (a `map`'s two
//! halves, not a sequence's decoded elements). Kotlin's `Array(n) { T() }`
//! constructor puts a real instance in every slot the moment the array
//! is allocated (Kotlin language specification, "Array types"), so
//! [`Kotlin::make_room`] needs no second pass and ignores
//! `assigned_whole` outright — the one place this column's `make_room`
//! is *shorter* than Java's rather than a plain translation of it.
//!
//! # No implicit bitwise operators
//!
//! Kotlin has no infix `&`, `|`, `^`, `<<`, `>>` or prefix `~` at all;
//! `and`, `or`, `xor`, `shl`, `shr` and `.inv()` stand in (Kotlin
//! language specification, "Bitwise operations"). The shared walk's own
//! [`CodecTarget::binary`]/[`CodecTarget::unary`] never reach one of
//! these — `crates/lang-rules/src/emit/{encode,decode}.rs` name only
//! `&&`, `||`, `+`, `=`, `==`, `<`, `>`, `!` and `~`, all of which
//! Kotlin already spells the same way except the last, which
//! [`Kotlin::unary`] answers with `.inv()`.
//!
//! # A mutable property never smart-casts
//!
//! [`Kotlin::optional_value`] reaches an `@optional` member's value only
//! inside a branch [`CodecTarget::present`] has already guarded, and the
//! member is a `var` — the smart cast from `T?` to `T` that guard would
//! otherwise earn does not apply to a mutable property (Kotlin language
//! specification, "Smart casts": "a property... must be `val`... and
//! must not have a custom getter"), so the value is asserted with `!!`
//! rather than read bare.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::kotlin::{
    kotlin_map_entry_name, kotlin_value_type, property_name, type_tag, unsigned_holder,
};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::kotlin;

/// The Kotlin backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kotlin;

/// The type name [`CodecTarget::named`] is handed for a map key's signed
/// reinterpretation; the member itself is `Int` and the ordering read is
/// `(v as Byte)`... spelled here as `.toByte()`.
const SIGNED_CHAR: &str = "signed char";

impl Kotlin {
    /// `place = Array(count) { start }` with `init` as each slot's start
    /// value, or the element type's own where it is `None`.
    fn allocate(
        base: &str,
        element: kotlin::Type,
        count: kotlin::Expr,
        init: Option<kotlin::Expr>,
        fill: Vec<kotlin::Stmt>,
    ) -> Vec<kotlin::Stmt> {
        let length = Self::to_array_size(count);
        let mut out = vec![kotlin::Stmt::Expr(kotlin::Expr::Binary {
            op: "=",
            lhs: Box::new(<Self as CodecTarget>::whole(base)),
            rhs: Box::new(kotlin::Expr::NewArray {
                element: Box::new(element),
                length: Box::new(length),
                init: init.map(Box::new),
            }),
        })];
        out.extend(fill);
        out
    }

    /// `write_u32` as `writeU32` — the runtime's own convention, and the
    /// walk's spelling is the wire's.
    fn camel(method: &str) -> String {
        let mut out = String::new();
        let mut rising = false;
        for ch in method.chars() {
            if ch == '_' {
                rising = true;
                continue;
            }
            if rising {
                out.extend(ch.to_uppercase());
                rising = false;
            } else {
                out.push(ch);
            }
        }
        out
    }

    /// A value, parenthesised first when it is not already a single
    /// token — `(a + b).toInt()` against `5L.inv()`. Kotlin's lexer
    /// reads a bare numeric literal directly followed by `.name` without
    /// ambiguity (measured: `5L.inv()` and `5.inv()` both parse), so a
    /// literal or a name needs no bracket; anything built from an
    /// operator does, the same rule [`CodecTarget::paren`] exists for.
    fn atom(value: kotlin::Expr) -> kotlin::Expr {
        match value {
            // A negative number is not one token: Kotlin has no negative
            // literal, only unary minus in front of one (Kotlin language
            // specification, "Integer literals"), and a call binds
            // tighter than the minus. `-3L.inv()` is `-(3L.inv())`,
            // which compiles and is a different mask; every declared-bit
            // check over a bitmask with bit 63 set was reading it.
            kotlin::Expr::Literal(Literal::Int(ref text) | Literal::Float(ref text))
                if text.starts_with('-') =>
            {
                kotlin::Expr::Paren(Box::new(value))
            }
            kotlin::Expr::Literal(_)
            | kotlin::Expr::Path(_)
            | kotlin::Expr::Call { .. }
            | kotlin::Expr::Field { .. }
            | kotlin::Expr::Index { .. }
            | kotlin::Expr::New { .. }
            | kotlin::Expr::NewArray { .. }
            | kotlin::Expr::ArrayOf { .. }
            | kotlin::Expr::Paren(_)
            | kotlin::Expr::NotNull(_) => value,
            other => kotlin::Expr::Paren(Box::new(other)),
        }
    }

    /// A conversion method call: `value.toInt()`, `value.toByte()`, …
    fn convert(value: kotlin::Expr, method: &str) -> kotlin::Expr {
        kotlin::Expr::method_call(Self::atom(value), method, Vec::new())
    }

    /// `value` narrowed to `Int`, skipping the conversion where `value`
    /// is already one — `kotlinc -Werror` fails a build over
    /// "redundant call of conversion method" the way `-Xlint:cast`
    /// fails Java's, and this is the one place that lint is reachable:
    /// [`Kotlin::count_of`] and a decoded sequence length both flow into
    /// [`Kotlin::count_up`], [`Kotlin::make_room`] and
    /// [`Kotlin::keep_first`], and only the second of the two is ever
    /// not already `Int`. The two shapes [`Kotlin::count_of`] can answer
    /// with — a bare `Int` literal and a bound's own `.size` — are
    /// recognised directly; anything else is a `Long` count read off the
    /// wire and gets `.toInt()`.
    fn to_array_size(value: kotlin::Expr) -> kotlin::Expr {
        Self::cast(kotlin::Type::Primitive(kotlin::Primitive::Int), value)
    }

    /// The reverse of [`Kotlin::to_array_size`]: widen `value` to `Long`
    /// where [`Kotlin::already`] recognises it as one of
    /// [`Kotlin::count_of`]'s two `Int` shapes, and leave everything
    /// else alone. Widening on the other rule — everything not
    /// recognisably a `Long` — put `.toLong()` on a genuine
    /// `unsigned long` member, which is already one, and
    /// `kotlinc -Werror` refuses that as a redundant conversion.
    fn widen_count(value: kotlin::Expr) -> kotlin::Expr {
        if !Self::already(&kotlin::Type::Primitive(kotlin::Primitive::Int), &value) {
            return value;
        }
        match value {
            kotlin::Expr::Literal(_) => Self::suffix_long(value),
            other => Self::cast(kotlin::Type::Primitive(kotlin::Primitive::Long), other),
        }
    }

    /// A shift distance as the `Int` Kotlin's shift functions declare.
    /// A literal loses its `L` rather than earning a `.toInt()`: the
    /// distance is a small number the walk fixed, and a conversion
    /// around a constant is noise in every generated bitset.
    fn narrow_shift(value: kotlin::Expr) -> kotlin::Expr {
        match value {
            kotlin::Expr::Literal(Literal::Int(text)) if text.ends_with('L') => {
                kotlin::Expr::Literal(Literal::Int(text.trim_end_matches('L').to_string()))
            }
            other => Self::cast(kotlin::Type::Primitive(kotlin::Primitive::Int), other),
        }
    }

    /// Whether the bit bound this call names puts its value in a `Long`
    /// — the boundary `adapter/kotlin.rs::unsigned_holder` draws.
    fn wide_bit_bound(bound: Option<&kotlin::Expr>) -> bool {
        match bound {
            Some(kotlin::Expr::Literal(Literal::Int(text))) => text
                .trim_end_matches('L')
                .parse::<u16>()
                .is_ok_and(|bits| unsigned_holder(bits) == kotlin::Primitive::Long),
            _ => false,
        }
    }

    /// `x.toLong()` back to `x`, and anything else unchanged.
    fn drop_to_long(value: kotlin::Expr) -> kotlin::Expr {
        match value {
            kotlin::Expr::Call {
                receiver: Some(inner),
                ref name,
                ref args,
            } if name.as_str() == "toLong" && args.is_empty() => *inner,
            other => other,
        }
    }

    /// Whether `value` is one of the walk's own locals. The shared walk
    /// spells every local it introduces with the `zd_` prefix an IDL
    /// identifier cannot carry — `crates/lang-rules/src/emit/decode.rs`
    /// names the headers `zd_p` and `zd_h` there — so this separates a
    /// field of the runtime's own header from a field of the sample.
    fn walk_local(value: &kotlin::Expr) -> bool {
        match value {
            kotlin::Expr::Path(path) => match path.segments() {
                [single] => single.as_str().starts_with("zd_"),
                _ => false,
            },
            _ => false,
        }
    }

    /// The same integer literal spelled as a `Long`, and anything else
    /// unchanged. A suffix costs nothing where the value already fits an
    /// `Int`; a conversion method would be the redundant call
    /// `kotlinc -Werror` refuses.
    fn suffix_long(value: kotlin::Expr) -> kotlin::Expr {
        match value {
            kotlin::Expr::Literal(Literal::Int(text))
                if text.parse::<i128>().is_ok() && !text.ends_with('L') =>
            {
                kotlin::Expr::Literal(Literal::Int(format!("{text}L")))
            }
            other => other,
        }
    }

    /// Whether `value`'s own static type is already `type_`, for the
    /// closed set of shapes this backend can tell without a general type
    /// checker. [`Kotlin::cast`]'s documentation says why this exists at
    /// all; extending it to a shape not listed here is safe in the
    /// direction of *not* recognising it — the conversion is then
    /// written out where Kotlin would have accepted it bare, which
    /// compiles, and never in the direction of recognising a shape whose
    /// type is not actually `type_`, which would drop a needed
    /// conversion silently.
    fn already(type_: &kotlin::Type, value: &kotlin::Expr) -> bool {
        use kotlin::{Primitive as P, Type};
        match value {
            kotlin::Expr::Paren(inner) => Self::already(type_, inner),
            kotlin::Expr::Literal(Literal::Int(text)) => {
                // `Long.MIN_VALUE` carries no suffix and is a `Long` all
                // the same; `zerodds_lang_render::kotlin::long_text` says
                // why it is spelled that way and not as a literal.
                let is_long = text.ends_with('L') || text == "Long.MIN_VALUE";
                matches!(type_, Type::Primitive(P::Long)) == is_long
                    && matches!(type_, Type::Primitive(P::Long) | Type::Primitive(P::Int))
            }
            // `count_of`'s unbounded answer: a Kotlin array's `.size` is
            // always `Int`.
            kotlin::Expr::Field { name, .. } if name.as_str() == "size" => {
                *type_ == Type::Primitive(P::Int)
            }
            // A field of a header the reader handed back, whose type
            // `runtime/kotlin/ZdWire.kt` fixes the same way it fixes a
            // read method's. `PidRead.length` and `EmHeaderRead.extent`
            // are octet counts a 32-bit NEXTINT can fill to the top —
            // DDS-XTypes 1.3 §7.4.3.4.2 — so the runtime holds both in a
            // `Long`. The receiver has to be one of the walk's own
            // header locals: an IDL member may itself be called
            // `length`, and reading that one as a `Long` would drop the
            // conversion it needs.
            kotlin::Expr::Field { value, name }
                if matches!(name.as_str(), "length" | "extent") && Self::walk_local(value) =>
            {
                *type_ == Type::Primitive(P::Long)
            }
            // `zd_bits` is the bitset anchor and the bitmask value the
            // walk declares for itself, and it declares both as
            // `Scalar::UInt64` — `crates/lang-rules/src/emit/decode.rs`,
            // where the name is written — which is a `Long` here.
            kotlin::Expr::Path(path) if matches!(path.segments(), [single] if single.as_str() == "zd_bits") => {
                *type_ == Type::Primitive(P::Long)
            }
            // Kotlin's bitwise functions answer in the type of the value
            // they were called on: `Long.and(Long)` and `Long.ushr(Int)`
            // are both `Long` (Kotlin language specification, "Bitwise
            // operations"). The left operand therefore decides, which is
            // how the anchor's width reaches the end of a field's
            // extraction.
            kotlin::Expr::Binary { op, lhs, .. }
                if matches!(*op, "and" | "or" | "ushr" | "shl") =>
            {
                Self::already(type_, lhs)
            }
            // A runtime read, whose return type `runtime/kotlin/ZdWire.kt`
            // fixes — the method name alone decides it, and its
            // arguments (`readWchar`'s framing and order among them) do
            // not change what it returns. A conversion method
            // [`Kotlin::cast`] itself already wrote — `.toChar()` and
            // kin — is the same kind of fact about its own result, and
            // recognising it here is what keeps
            // `crates/lang-rules/src/emit/decode.rs`'s own `cast` around
            // a `char`/`wchar` read from earning a second one from
            // [`Kotlin::store`] on top of it.
            kotlin::Expr::Call {
                name,
                receiver: Some(_),
                ..
            } => {
                let returns = match name.as_str() {
                    "readBool" => Type::Primitive(P::Boolean),
                    "readU8" | "readI8" | "readChar" | "readI16" | "readU16" | "readI32"
                    | "readWchar" | "toInt" => Type::Primitive(P::Int),
                    "readU32" | "readI64" | "readU64" | "readEnum" | "readBitmask"
                    | "readBitset" | "toLong" => Type::Primitive(P::Long),
                    "readF32" | "toFloat" => Type::Primitive(P::Float),
                    "readF64" | "toDouble" => Type::Primitive(P::Double),
                    "readString" | "readStringAlloc" | "readFixed" => Type::named("String"),
                    "readF128" => Type::named("Float128"),
                    "toByte" => Type::Primitive(P::Byte),
                    "toShort" => Type::Primitive(P::Short),
                    "toChar" => Type::Primitive(P::Char),
                    _ => return false,
                };
                returns == *type_
            }
            _ => false,
        }
    }

    fn reader_body(name: &str, sample: kotlin::Type, body: Vec<kotlin::Stmt>) -> Vec<kotlin::Stmt> {
        let defaulter = crate::default_kotlin::defaulter_from_decoder(name);
        let mut statements = vec![kotlin::Stmt::Declare {
            mutable: true,
            type_: sample,
            name: Ident::new("v"),
            value: Some(Self::call(&defaulter, Vec::new())),
        }];
        statements.extend(body);
        statements.push(kotlin::Stmt::Return(Some(Self::whole("v"))));
        statements
    }
}

impl CodecTarget for Kotlin {
    type Expr = kotlin::Expr;
    type Stmt = kotlin::Stmt;
    type Block = Vec<kotlin::Stmt>;
    type Type = kotlin::Type;
    type Decl = kotlin::Decl;

    // -- types --------------------------------------------------------

    /// `octet`, `unsigned short` and `wchar` widen to `Int`; `long long`
    /// and `unsigned long` widen to `Long`. No unsigned type exists on
    /// the JVM, so every wider-than-declared holder carries a **pattern**
    /// and not a number past its own width — the same three-place
    /// agreement `adapter/kotlin.rs` and `runtime/kotlin/ZdWire.kt`
    /// share.
    fn scalar(kind: Scalar) -> Self::Type {
        use kotlin::Primitive as P;
        kotlin::Type::Primitive(match kind {
            Scalar::Bool => P::Boolean,
            Scalar::Char => P::Char,
            Scalar::Int8 => P::Byte,
            Scalar::Int16 => P::Short,
            Scalar::Index | Scalar::Size | Scalar::Int32 | Scalar::UInt8 | Scalar::UInt16 => P::Int,
            Scalar::Int64 | Scalar::UInt32 | Scalar::UInt64 => P::Long,
            Scalar::Float => P::Float,
            Scalar::Double => P::Double,
            // binary128 has no JVM primitive; the runtime carries it as
            // a pair of `Long`s behind `Float128`.
            Scalar::LongDouble => return kotlin::Type::named("Float128"),
        })
    }

    fn named(name: &str) -> Self::Type {
        kotlin::Type::named(name)
    }

    fn nothing() -> Self::Type {
        kotlin::Type::Unit
    }

    /// The runtime's handles, all top-level declarations of
    /// `runtime/kotlin/ZdWire.kt` — a `.kt` file needs no wrapper class
    /// to nest them in the way `runtime/java`'s does, so the unqualified
    /// name resolves once the file's `import zerodds.wire.*` is written.
    fn handle(kind: Handle) -> Self::Type {
        kotlin::Type::named(match kind {
            Handle::DHeader => "DHeader",
            Handle::EmHeader => "EmHeader",
            Handle::Pid => "Pid",
            Handle::TryConstruct => "TryConstruct",
            Handle::ReaderMark => "ReaderMark",
            Handle::PidRead => "PidRead",
            Handle::EmHeaderRead => "EmHeaderRead",
        })
    }

    /// A `char` member is a Kotlin `Char` and the runtime's parameter is
    /// an `Int` code point, so the narrow form reads `.code`
    /// (Kotlin language specification, "Char"); a `wchar` member is
    /// already an `Int` — `adapter/kotlin.rs`'s memory-shape table says
    /// so — and needs nothing.
    fn code_point(value: Self::Expr, narrow: bool) -> Self::Expr {
        if narrow {
            return kotlin::Expr::field(value, "code");
        }
        value
    }

    /// `java.lang.Long.compareUnsigned` for the eight-octet width, whose
    /// value lives in a `Long` as its bit pattern and whose own `<` is
    /// signed. Kotlin declares no unsigned comparison of its own that
    /// reaches a plain `Long`, so the JVM's own static method is called
    /// by its fully qualified name — legal without an import (Kotlin
    /// language specification, "Package and imports"). The narrower
    /// widths live zero-extended in a wider signed holder, where the
    /// inherited `<` is already the unsigned order.
    fn unsigned_less(octets: u8, left: Self::Expr, right: Self::Expr) -> Self::Expr {
        if octets < 8 {
            return Self::binary("<", left, right);
        }
        Self::binary(
            "<",
            kotlin::Expr::Call {
                receiver: Some(Box::new(kotlin::Expr::Path(Path::new(&[
                    "java", "lang", "Long",
                ])))),
                name: Ident::new("compareUnsigned"),
                args: vec![left, right],
            },
            Self::int(0),
        )
    }

    /// `!left && right` — `false` before `true`. Kotlin's `<` refuses a
    /// pair of `Boolean`s outright, the same class of target
    /// [`CodecTarget::bool_less`]'s default is documented for.
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        Self::binary("&&", kotlin::Expr::Not(Box::new(left)), right)
    }

    /// `runtime/kotlin/ZdWire.kt` publishes each as an `enum class`
    /// constant — one type per policy — so each name carries its own.
    ///
    /// Qualified with the runtime's package: an IDL type of the same name
    /// is declared in the generated file and wins over the star import.
    /// `enum Status { OK, WARN, FAIL }` (`enum-sparse-values`) turned
    /// `Status.DISCARDED` into an unresolved reference and
    /// `readerStatus() != Status.OK` into a comparison with an `Int`.
    fn constant(which: Constant) -> Self::Expr {
        let qualified = |name: &str| format!("zerodds.wire.{name}");
        kotlin::Expr::Path(Path::new(
            &qualified(match which {
                Constant::WcharWidth2 => "WcharWidth.OCTETS_2",
                Constant::WcharWidth4 => "WcharWidth.OCTETS_4",
                Constant::WcharFixed => "WcharFraming.FIXED",
                Constant::WcharGiopLengthPrefixed => "WcharFraming.GIOP_LENGTH_PREFIXED",
                Constant::WstringUnitsStream => "WstringUnitOrder.STREAM",
                Constant::WstringUnitsBigEndianAlways => "WstringUnitOrder.BIG_ENDIAN_ALWAYS",
                Constant::WstringUnitsByteOrderMark => "WstringUnitOrder.BYTE_ORDER_MARK",
                Constant::WstringUnitsHostUnswapped => "WstringUnitOrder.HOST_UNSWAPPED",
                Constant::WstringSpecification => "WstringLengthPolicy.SPECIFICATION",
                Constant::WstringCharacterCount => "WstringLengthPolicy.CHARACTER_COUNT",
                Constant::WstringCharacterCountTerminated => {
                    "WstringLengthPolicy.CHARACTER_COUNT_TERMINATED"
                }
                Constant::WstringSpecificationWideUnits => {
                    "WstringLengthPolicy.SPECIFICATION_WIDE_UNITS"
                }
                Constant::WstringRosTypeSupport => "WstringLengthPolicy.ROS_TYPE_SUPPORT",
                Constant::WstringOctetsTerminated2 => "WstringLengthPolicy.OCTETS_TERMINATED_2",
                Constant::WstringOctetsTerminated4 => "WstringLengthPolicy.OCTETS_TERMINATED_4",
                Constant::WstringCharactersTerminated2 => {
                    "WstringLengthPolicy.CHARACTERS_TERMINATED_2"
                }
                Constant::FixedDeclared => "FixedDigitCount.DECLARED",
                Constant::FixedValuePrecision => "FixedDigitCount.VALUE_PRECISION",
                Constant::FixedValueStringDigits => "FixedDigitCount.VALUE_STRING_DIGITS",
                Constant::ParameterMember => "ParameterRole.MEMBER",
                Constant::ParameterOptional => "ParameterRole.OPTIONAL",
                Constant::TryConstructDiscard => "TryConstruct.DISCARD",
                Constant::TryConstructTrim => "TryConstruct.TRIM",
                Constant::TryConstructUseDefault => "TryConstruct.USE_DEFAULT",
                Constant::StatusOk => "Status.OK",
                Constant::StatusDiscarded => "Status.DISCARDED",
                Constant::StatusImpossibleLength => "Status.IMPOSSIBLE_LENGTH",
                Constant::StatusOutOfRange => "Status.OUT_OF_RANGE",
                Constant::StatusTruncated => "Status.TRUNCATED",
            })
            .split('.')
            .collect::<Vec<_>>(),
        ))
    }

    /// The materialiser's answer, not a second reading of it: a
    /// `@bit_bound` set is assembled in `Int` up to 32 bits and `Long`
    /// past it, exactly what a bitmask member of the same bound is held
    /// in — `adapter/kotlin.rs::unsigned_holder`.
    fn unsigned_holder(bits: u16) -> Self::Type {
        kotlin::Type::Primitive(unsigned_holder(bits))
    }

    /// Every free function `runtime/kotlin/ZdWire.kt` publishes at the
    /// top level — `noDheader`, `bitPatternInt` and kin — is called
    /// unqualified; there is no second namespace to route between the
    /// way [`crate::emit::java::Java::runtime_static`] routes `Writer`
    /// against `Reader`.
    /// A free function of the runtime, unqualified — except
    /// `pid_is_user_member`, which `runtime/kotlin/ZdWire.kt` publishes
    /// as a method on [`PidRead`](kotlin::Type) itself rather than as a
    /// free function of its own, the same shape [`crate::emit::zig::Zig`]
    /// reaches it through.
    fn runtime_static(method: &str, mut args: Vec<Self::Expr>) -> Self::Expr {
        if method == "pid_is_user_member" && !args.is_empty() {
            let header = args.remove(0);
            return kotlin::Expr::method_call(header, "isUserMember", Vec::new());
        }
        Self::call(&Self::camel(method), args)
    }

    // -- expressions -----------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        kotlin::Expr::name(text)
    }

    fn int(value: i128) -> Self::Expr {
        kotlin::Expr::int(value)
    }

    fn literal(value: Literal) -> Self::Expr {
        kotlin::Expr::Literal(value)
    }

    /// A generated codec is a top-level `fun`, so an unqualified call
    /// reaches it without naming a class. `strcmp` is the walk's name
    /// for a three-way octet-order comparison; here it is
    /// `a.compareTo(b)`, comparing UTF-16 code units — the same reading
    /// `emit/java.rs` gives, and true for the same corpus of
    /// inside-the-BMP keys.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            let mut arguments = args.into_iter();
            if let (Some(left), Some(right)) = (arguments.next(), arguments.next()) {
                return kotlin::Expr::method_call(left, "compareTo", vec![right]);
            }
            return kotlin::Expr::call("strcmp", Vec::new());
        }
        kotlin::Expr::call(callee, args)
    }

    /// `w.writeU32(x)` — a method on the local the body holds the writer
    /// or reader in.
    /// `w.writeU32(x)`, with one width's argument widened first.
    ///
    /// `write_u32` is the one runtime operation the shared walk hands
    /// two different Kotlin types: a genuine `unsigned long` member,
    /// already a `Long` through [`Kotlin::scalar`], and — unlike every
    /// other target, whose count is that width already — a *count*,
    /// which `crates/lang-rules/src/emit/encode.rs` builds from
    /// [`Kotlin::count_of`] and hands to `write_u32` with no cast around
    /// it, because C's `uint32_t` and Java's implicitly-widened `int`
    /// both already fit there. Kotlin widens nothing implicitly (the
    /// module documentation says so), so this seam does what the walk
    /// does not — but only where it has to: `kotlinc -Werror` fails a
    /// build over "redundant call of conversion method" exactly as
    /// readily on `v.x.toLong()` for an already-`Long` `x` as it does on
    /// `v.size.toInt()`, so `Kotlin::to_array_size`'s recognition of
    /// [`Kotlin::count_of`]'s two `Int` shapes is reused here in
    /// reverse: widen only where the argument is recognisably one of
    /// them, and leave a genuine `unsigned long` member's already-`Long`
    /// value untouched.
    ///
    /// `write_i8` and `write_i16` are the same seam from the other side.
    /// DDS-XTypes 1.3 §7.2.2.2.1.2 gives `int8` and `short` eight and
    /// sixteen bits, and [`Kotlin::scalar`] holds them in the JVM types
    /// of exactly those widths, `Byte` and `Short`; `runtime/kotlin/ZdWire.kt`
    /// takes every integer narrower than 32 bits as an `Int`, because
    /// that is the width the JVM computes in. Java's argument
    /// conversion crosses that gap on its own and Kotlin's does not, so
    /// the widening is written here.
    ///
    /// `write_bitmask` is the third: `crates/lang-rules/src/emit/encode.rs`
    /// widens its value to the `unsigned long long` the C runtime takes,
    /// and `adapter/kotlin.rs::unsigned_holder` has already put a
    /// bitmask of more than thirty-two bits — DDS-XTypes 1.3 §7.3.1.2.4
    /// allows a `@bit_bound` up to 64 — in the `Long` that widening
    /// leads to. The bound is the call's own first argument, so the
    /// conversion is dropped where it names one.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let args = match method {
            "write_u32" => args.into_iter().map(Self::widen_count).collect(),
            "write_i8" | "write_i16" => args
                .into_iter()
                .map(|arg| Self::cast(kotlin::Type::Primitive(kotlin::Primitive::Int), arg))
                .collect(),
            "write_bitmask" if Self::wide_bit_bound(args.first()) => {
                args.into_iter().map(Self::drop_to_long).collect()
            }
            // The same, one argument along: `bitfield` takes the value
            // first and the field's width second.
            "bitfield" if Self::wide_bit_bound(args.get(1)) => {
                args.into_iter().map(Self::drop_to_long).collect()
            }
            _ => args,
        };
        kotlin::Expr::method_call(Self::name(receiver), &Self::camel(method), args)
    }

    /// `runtime/kotlin` carries the three HDDS axes: `beginPackedElement`
    /// and `finishPackedElement` on the writer and the reader, the
    /// DHEADER pair for a sequence element's own length, and
    /// `writeWstringUtf8Octets` / `readWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    fn member(base: &str, field: &str) -> Self::Expr {
        kotlin::Expr::field(Self::name(base), field)
    }

    fn whole(base: &str) -> Self::Expr {
        Self::name(base)
    }

    /// `L` on anything that will not fit an `Int`, and the pattern on
    /// anything that will not fit a `Long` — the same reasoning
    /// `emit/java.rs::wide_int` gives. The suffix is spelled the same;
    /// the minimum is not, which is why every `Long` here goes through
    /// [`zerodds_lang_render::kotlin::long_text`].
    fn wide_int(value: i128) -> Self::Expr {
        let text = if value > i128::from(i64::MAX) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let pattern = value as u64 as i64;
            zerodds_lang_render::kotlin::long_text(pattern)
        } else if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
            #[allow(clippy::cast_possible_truncation)]
            let pattern = value as i64;
            zerodds_lang_render::kotlin::long_text(pattern)
        } else {
            format!("{value}")
        };
        kotlin::Expr::Literal(Literal::Int(text))
    }

    /// No unsigned literal exists; wide enough to hold itself, through
    /// [`Kotlin::wide_int`].
    fn unsigned_int(value: i128) -> Self::Expr {
        Self::wide_int(value)
    }

    /// A 64-bit pattern in a `Long`, which is where Kotlin keeps one.
    fn wide_unsigned(value: u64) -> Self::Expr {
        #[allow(clippy::cast_possible_wrap)]
        let pattern = value as i64;
        kotlin::Expr::Literal(Literal::Int(zerodds_lang_render::kotlin::long_text(
            pattern,
        )))
    }

    /// A signed 64-bit constant, always suffixed: an enumerator's
    /// pattern is compared against a `Long` and an unsuffixed literal
    /// there is an `Int` Kotlin refuses to compare against one.
    fn wide_signed(value: i128) -> Self::Expr {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let pattern = if value > i128::from(i64::MAX) {
            value as u64 as i64
        } else {
            value as i64
        };
        kotlin::Expr::Literal(Literal::Int(zerodds_lang_render::kotlin::long_text(
            pattern,
        )))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        kotlin::Expr::field(value, field)
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        kotlin::Expr::Index {
            value: Box::new(value),
            index: Box::new(at),
        }
    }

    /// `.size` — Kotlin's array property, for both a boxed `Array<T>`
    /// and a specialised primitive array alike (Kotlin language
    /// specification, "Array types"); `.length` is Java's and C's
    /// spelling and not this column's. The bound the materialiser fixed
    /// is written out where there is one.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => kotlin::Expr::int(i128::from(bound)),
            None => kotlin::Expr::field(Self::whole(base), "size"),
        }
    }

    /// The operators as given, with the three Kotlin has no token for
    /// translated to the infix functions that stand in: `&` is `and`,
    /// `|` is `or`, and the walk's own `>>` is `ushr` and not `shr` —
    /// every value it shifts is a bit set's anchor or a bitmask's
    /// holder, unsigned in the specification's own terms even though it
    /// lives in a signed JVM type, and `shr` would sign-extend a
    /// negative pattern into the vacated bits rather than filling them
    /// with zero. `&&` and `||` reach Kotlin's own tokens unchanged, as
    /// every other operator the walk names does.
    ///
    /// An integer literal beside an operand `Kotlin::already` knows to
    /// be a `Long` is suffixed. Kotlin's equality operators are not
    /// defined across two integer widths at all — `zd_p.length != 0` is
    /// "operator '!=' cannot be applied to 'Long' and 'Int'" and not a
    /// widening — and the walk writes the bare literal because C and
    /// Java both take one there.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        let spelled = match op {
            "&" => "and",
            "|" => "or",
            ">>" => "ushr",
            other => other,
        };
        // Kotlin declares the shift as `infix fun Long.ushr(bitCount:
        // Int)`, so the distance is an `Int` whatever the value shifted
        // is (Kotlin language specification, "Bitwise operations"). The
        // walk hands it in the anchor's own width —
        // `crates/lang-rules/src/emit/decode.rs` says why — and that is a
        // `Long` here.
        if spelled == "ushr" || spelled == "shl" {
            return kotlin::Expr::Binary {
                op: spelled,
                lhs: Box::new(lhs),
                rhs: Box::new(Self::narrow_shift(rhs)),
            };
        }
        let long = kotlin::Type::Primitive(kotlin::Primitive::Long);
        let (lhs, rhs) = if Self::already(&long, &lhs) {
            (lhs, Self::suffix_long(rhs))
        } else if Self::already(&long, &rhs) {
            (Self::suffix_long(lhs), rhs)
        } else {
            (lhs, rhs)
        };
        kotlin::Expr::Binary {
            op: spelled,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// The walk's own two: `!` — Kotlin's real prefix operator, unlike
    /// Java's, so [`kotlin::Expr::Not`] answers directly — and `~`,
    /// which Kotlin has no operator for at all and reaches through
    /// `.inv()` (Kotlin language specification, "Bitwise operations").
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        match op {
            "!" => kotlin::Expr::Not(Box::new(value)),
            "~" => Self::convert(value, "inv"),
            _ => value,
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        kotlin::Expr::Paren(Box::new(value))
    }

    /// The conversion, spelled by what it converts into. Unlike
    /// `emit/java.rs`'s, every arm is written unconditionally: Kotlin
    /// performs no implicit numeric widening at all (not even `Int` to
    /// `Long`), so there is no redundant cast to avoid the way `javac`'s
    /// `-Xlint:cast` makes Java avoid one, and `5.toInt()` on a value
    /// already an `Int` compiles without complaint.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == kotlin::Type::named(SIGNED_CHAR) {
            return Self::convert(value, "toByte");
        }
        // A `boolean` discriminator's labels arrive as the integers the
        // IDL evaluator gives — 0 and 1 — and this is the only position
        // `cast` is ever asked to reach `Boolean` from; the label *is*
        // the value it stands for.
        if type_ == kotlin::Type::Primitive(kotlin::Primitive::Boolean) {
            if let kotlin::Expr::Literal(Literal::Int(text)) = &value {
                return kotlin::Expr::Literal(Literal::Bool(text.trim_end_matches('L') != "0"));
            }
        }
        // `kotlinc -Werror` fails a build over "redundant call of
        // conversion method" wherever the value's own static type
        // already is the target — `runtime/kotlin/ZdWire.kt`'s own
        // `readWchar` answering an `Int` where the member already is
        // one is the case that was measured. `Kotlin::already` knows
        // the small, closed set of shapes the shared walk hands `cast`
        // that this backend can already tell the type of without a
        // general type checker: a literal, `count_of`'s `.size`, and a
        // runtime read whose return type `runtime/kotlin/ZdWire.kt`
        // fixes.
        if Self::already(&type_, &value) {
            return value;
        }
        match type_ {
            kotlin::Type::Primitive(kotlin::Primitive::Byte) => Self::convert(value, "toByte"),
            kotlin::Type::Primitive(kotlin::Primitive::Short) => Self::convert(value, "toShort"),
            kotlin::Type::Primitive(kotlin::Primitive::Int) => Self::convert(value, "toInt"),
            kotlin::Type::Primitive(kotlin::Primitive::Long) => Self::convert(value, "toLong"),
            kotlin::Type::Primitive(kotlin::Primitive::Float) => Self::convert(value, "toFloat"),
            kotlin::Type::Primitive(kotlin::Primitive::Double) => Self::convert(value, "toDouble"),
            kotlin::Type::Primitive(kotlin::Primitive::Char) => Self::convert(value, "toChar"),
            // A `Boolean` reached by anything but the label form above,
            // an aggregate, `Float128`, an array or `Unit`: none of
            // these has a numeric conversion the walk asks `cast` for,
            // so the value is handed back unchanged.
            _ => value,
        }
    }

    /// Nothing to take an address of and no array to decay: both forms
    /// hand the value, as in `emit/java.rs`.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    // -- statements -------------------------------------------------------

    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        kotlin::Stmt::Declare {
            mutable: true,
            type_,
            name: Ident::new(name),
            value,
        }
    }

    fn perform(value: Self::Expr) -> Self::Stmt {
        kotlin::Stmt::Expr(value)
    }

    /// `return v;` where the walk asked for a bare `return`: only the
    /// reader leaves a routine early, and a Kotlin decoder answers with
    /// the sample it built.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        kotlin::Stmt::Return(Some(value.unwrap_or_else(|| Self::whole("v"))))
    }

    /// Nothing: `kotlinc` raises no error over an unread parameter.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// The call on its own; a method call is already a Kotlin statement.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        kotlin::Stmt::Expr(value)
    }

    /// `place = zd_dec_T(r, behaviour)` — Kotlin has no address to pass
    /// and no out-parameter to pass one in, so a decoder answers with
    /// its value. [`Kotlin::routine`] and [`Kotlin::give_back`] are the
    /// other two halves of that decision.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        kotlin::Stmt::Expr(kotlin::Expr::Binary {
            op: "=",
            lhs: Box::new(place),
            rhs: Box::new(Self::call(callee, vec![Self::name("r"), behaviour])),
        })
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

    /// `place = place.copyOfRange(0, count)` — Kotlin's array-slicing
    /// extension, over both a specialised primitive array and a boxed
    /// one alike.
    ///
    /// **Not `copyOf(count)`.** `Array<T>.copyOf(newSize)` is declared
    /// `Array<T?>` in the Kotlin standard library, because the argument
    /// may be larger than the receiver and the places it adds hold
    /// `null`; assigning that back to the `Array<T>` the walk declared
    /// is `assignment type mismatch: actual type is 'Array<String?>',
    /// but 'Array<String>' was expected`, which is a `kotlinc` error and
    /// not a warning. Every reference element type reaches it —
    /// `string`, `wstring`, an aggregate, and a map's entry holder —
    /// so a bounded `sequence<string, 8>` or `map<long, long, 4>` made
    /// the whole file refuse to compile. `copyOfRange(fromIndex,
    /// toIndex)` is declared `Array<T>` for the same receiver, and
    /// `IntArray.copyOfRange` answers an `IntArray` exactly as
    /// `IntArray.copyOf` does, so the one spelling still serves both
    /// kinds of array.
    ///
    /// `toIndex` past the receiver's end would be
    /// `IndexOutOfBoundsException` where `copyOf` had grown the array
    /// instead, and this seam never reaches that: `emit::decode`'s
    /// bounded-collection narrowing runs only under `zd_over`, which is
    /// `zd_n > count`.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![kotlin::Stmt::Expr(kotlin::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::whole(base)),
            rhs: Box::new(kotlin::Expr::method_call(
                Self::whole(base),
                "copyOfRange",
                vec![
                    Self::to_array_size(Self::unsigned_int(0)),
                    Self::to_array_size(count),
                ],
            )),
        })]
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        kotlin::Stmt::If {
            condition,
            then: kotlin::Block { statements: then },
            otherwise: otherwise.map(|body| kotlin::Block { statements: body }),
        }
    }

    /// `for (i in 0 until bound)`, with `bound` narrowed to `Int` first.
    ///
    /// The counter this renders is always `Int`-ranged — `0 until` an
    /// `Int` literal — because every array this column's codec indexes
    /// is a Kotlin array, whose subscript is `Int`
    /// (Kotlin language specification, "Array types"). A sequence's own
    /// length is a `uint32` on the wire and lives in a `Long` once read
    /// (`Kotlin::scalar`'s `Scalar::UInt32` answer), so a bound built
    /// from one needs the same narrowing [`Kotlin::make_room`]'s length
    /// does, for the identical reason: a JVM array cannot be longer than
    /// `Int.MAX_VALUE` regardless of what the wire declared.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        kotlin::Stmt::CountUp {
            counter: Ident::new(counter),
            bound: Box::new(Self::to_array_size(bound)),
            body: kotlin::Block { statements: body },
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        kotlin::Stmt::While {
            condition,
            body: kotlin::Block { statements: body },
        }
    }

    fn leave() -> Self::Stmt {
        kotlin::Stmt::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        kotlin::Stmt::Comment(doc)
    }

    // -- assembly -----------------------------------------------------

    /// `place = Array(count) { Element() }`, or `IntArray(count)` for a
    /// primitive element — the module documentation says why no
    /// `assigned_whole`-conditioned pre-fill loop is needed here, unlike
    /// `emit/java.rs`'s.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        Self::allocate(base, element, count, None, fill)
    }

    /// `Array(count) { 0 }` where the element is the `typealias` of a
    /// typedef of a plain value, which has no constructor for `T()`
    /// (`default_kotlin::alias_slot_start`); [`Kotlin::make_room`] for
    /// every other element.
    fn make_room_of(
        ast: &Ast,
        element_id: Id<TypeNode>,
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let init = crate::default_kotlin::alias_slot_start(ast, element_id);
        Self::allocate(base, element, count, init, fill)
    }

    /// The narrowing `Byte`, `Short` and `Char` holders need, and no
    /// other: `readI8`/`readI16`/`readChar` all answer an `Int`, and a
    /// narrower field does not take one without being told to.
    fn store(place: Self::Expr, type_: Self::Type, value: Self::Expr) -> Self::Stmt {
        // `crates/lang-rules/src/emit/decode.rs::primitive_read_at`
        // already wraps a `char`/`wchar` read in its own `cast` to
        // `Kotlin::scalar`'s width — a `char` becomes `Char` there and a
        // `wchar` stays the `Int` it already reads as — before this is
        // ever reached, so applying [`Kotlin::already`]'s test here
        // finds the walk's own conversion and leaves it exactly once
        // rather than adding a second, redundant one on top of it.
        if Self::already(&type_, &value) {
            return kotlin::Stmt::Expr(kotlin::Expr::Binary {
                op: "=",
                lhs: Box::new(place),
                rhs: Box::new(value),
            });
        }
        let narrow = matches!(
            type_,
            kotlin::Type::Primitive(
                kotlin::Primitive::Byte | kotlin::Primitive::Short | kotlin::Primitive::Char
            )
        );
        let value = if narrow {
            Self::cast(type_, value)
        } else {
            value
        };
        kotlin::Stmt::Expr(kotlin::Expr::Binary {
            op: "=",
            lhs: Box::new(place),
            rhs: Box::new(value),
        })
    }

    /// `val name = T()` — a Kotlin local of a class type is a reference
    /// to nothing until one is made, and every generated class has a
    /// zero-argument constructor because every one of its properties
    /// carries a default (`adapter/kotlin.rs`'s materialiser gives every
    /// one).
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        kotlin::Stmt::Declare {
            mutable: false,
            type_: type_.clone(),
            name: Ident::new(name),
            value: Some(kotlin::Expr::New {
                type_: Box::new(type_),
                args: Vec::new(),
            }),
        }
    }

    /// `runtime/kotlin/ZdWire.kt` publishes a header's parts as
    /// constructor properties, so the field the walk names is a
    /// property access — `.isEnd`, not `.isEnd()`: Kotlin generates no
    /// getter method beside a `val` the way Java's accessor-per-field
    /// convention does.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        kotlin::Expr::field(
            value,
            match field {
                "pid" => "pid",
                "end" => "isEnd",
                "ignore" => "ignore",
                "length" => "length",
                "member_id" => "memberId",
                "must_understand" => "mustUnderstand",
                "extent" => "extent",
                // A field the runtime does not publish is a `kotlinc`
                // error naming it, which is where a header that grew a
                // part shows up.
                other => other,
            },
        )
    }

    fn present(base: &str, field: &str) -> Self::Expr {
        kotlin::Expr::Binary {
            op: "!=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        }
    }

    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![kotlin::Stmt::Expr(kotlin::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        })]
    }

    /// One property per case, directly on the class.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        kotlin::Expr::field(entry, if key { "key" } else { "value" })
    }

    /// `v.x!!` — the value an `@optional` member holds, given that it
    /// holds one. The module documentation gives the reason the bare
    /// property is not enough: it is a `var`, and Kotlin does not
    /// smart-cast one.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        kotlin::Expr::NotNull(Box::new(Self::member(base, field)))
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

    /// The boxed property itself; assigning to it is what makes the
    /// member present.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    fn elements(base: &str) -> Self::Expr {
        Self::whole(base)
    }

    fn absent() -> Self::Expr {
        kotlin::Expr::Literal(Literal::Null)
    }

    /// Nothing: every value this column's runtime materialises on a
    /// successful read is a real, non-null Kotlin value — `readString`
    /// answers `String` and not `String?` — so there is no "the read
    /// produced nothing" state for a generated decoder to guard against,
    /// the same answer [`crate::emit::zig::Zig::materialised`] gives for
    /// the same reason and unlike Java's, whose `String` reference could
    /// in principle be `null`.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// `fun zd_enc_T(w: Writer, v: T)` and `fun zd_dec_T(r: Reader, zd_tc: TryConstruct): T`.
    ///
    /// A top-level `fun`, needing no forward declaration: Kotlin resolves
    /// a call to a function declared later in the same file (Kotlin
    /// language specification, "Declarations"), so a recursive type
    /// needs no prototype ahead of its body — `body` absent therefore
    /// answers `None` outright, as `emit/java.rs::routine` does for the
    /// same reason.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![kotlin::Param {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            type_: kotlin::Type::named(if spec.writing { "Writer" } else { "Reader" }),
        }];
        if spec.writing {
            params.push(kotlin::Param {
                name: Ident::new("v"),
                type_: spec.sample.clone(),
            });
        }
        params.extend(spec.extra.into_iter().map(|(type_, name)| kotlin::Param {
            name: Ident::new(&name),
            type_,
        }));
        let (returns, body) = if spec.writing {
            (kotlin::Type::Unit, body)
        } else {
            (
                spec.sample.clone(),
                Self::reader_body(&spec.name, spec.sample, body),
            )
        };
        Some(kotlin::Decl::Func(kotlin::FuncDecl {
            doc: spec.doc,
            name: Ident::new(&spec.name),
            params,
            returns,
            body,
        }))
    }

    // -- the materialiser's boundary ------------------------------------

    /// A typedef of a collection, an aggregate or a `long double` is seen
    /// through, so that an array the codec allocates starts each slot with
    /// a value of the target's own form (`IntArray(0)`, `emptyArray<T>()`,
    /// `T()`, `Float128.zero()`); a `typealias` is interchangeable with its
    /// target. A typedef of a plain value keeps its name: the declaration
    /// holds it boxed as `Array<Alias>`, which a specialised `IntArray`
    /// would not match, and [`Kotlin::make_room_of`] writes the slot's
    /// start value, because the alias has no constructor.
    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        let mut seen = id;
        for _ in 0..64 {
            let Ok(zerodds_ast::TypeKind::Alias { aliased }) =
                ast.type_(seen).map(|node| &node.kind)
            else {
                break;
            };
            let Some(target) = aliased.resolved() else {
                break;
            };
            seen = target;
        }
        let plain = matches!(
            ast.type_(seen).map(|node| &node.kind),
            Ok(zerodds_ast::TypeKind::Primitive(kind)) if *kind != zerodds_ast::PrimitiveKind::Float128
        ) || matches!(
            ast.type_(seen).map(|node| &node.kind),
            Ok(zerodds_ast::TypeKind::Enumeration { .. } | zerodds_ast::TypeKind::Bitmask { .. })
        );
        let shown = if plain { id } else { seen };
        kotlin_value_type(ast, shown, "codec")
            .unwrap_or_else(|_| kotlin::Type::named(&format!("UnresolvedType{}", id.slot())))
    }

    /// The materialiser's own naming: `type_tag` refuses nothing this
    /// signature could pass on, and the fallback carries the arena slot
    /// so that two failures cannot collide onto one bare `unknown`.
    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0)
    }

    fn escape(name: &str) -> String {
        property_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", property_name("parent")).repeat(depth))
    }

    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        kotlin_map_entry_name(ast, id)
    }

    /// Nothing: a Kotlin file has one package and no nested modules, so
    /// there is no prefix for a declaration's own scope to add — the
    /// module flattening `adapter/kotlin.rs` already did the whole of
    /// this.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// # Errors
    /// As [`crate::default_kotlin::member_default`]: a `@default` this
    /// column cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_kotlin::member_default(ast, node, type_, place)
    }

    /// `place = zd_def_<T>()` — Table 9, through the generated
    /// defaulter.
    ///
    /// # Errors
    /// Never here; whether the type has a Kotlin default is decided
    /// where the defaulter is built.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_kotlin::default_into(ast, id, place))
    }

    /// `Enum.LITERAL` — no outer path: every `object` the materialiser
    /// declares is already a top-level name.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        crate::default_kotlin::enumerator_constant(ast, id, enumerator)
    }
}
