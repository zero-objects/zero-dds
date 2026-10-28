// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java backend of [`CodecTarget`].
//!
//! The same walk as the C column, spelled in Java. What that spelling
//! settles, and why each of these is the backend's answer rather than
//! the walk's:
//!
//! - **No pointers.** A codec takes the sample as a reference, so
//!   [`CodecTarget::member`] is `v.x` and [`CodecTarget::whole`] is `v`.
//!   [`CodecTarget::handoff`] hands the value either way, because there
//!   is no address to take and no array to decay.
//! - **No free functions.** `runtime/c` exports `zdw_write_u32(w, x)`
//!   and `runtime/java` has `w.writeU32(x)`; the walk asks for
//!   `write_u32` and each backend spells it. A codec for a type is a
//!   `static` method rather than a function, for the same reason.
//! - **No forward declarations.** A method may call one declared below
//!   it, so [`CodecTarget::routine`] answers `None` to a prototype and
//!   the walk emits only bodies. That is the whole of what C's
//!   declaration order costs, expressed once.
//! - **No unsigned types.** A counter is `int`; the runtime's
//!   `writeU32` takes what fits and the widening is its business, not
//!   the generated code's.
//!
//! What is *not* different is the wire. `runtime/java` implements
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c` does,
//! and neither calls the other. Where the two columns disagree on an
//! octet, one of them has misread the specification — which is the only
//! reason to have a second column at all.

use super::{BitPatternSource, CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::java::{JavaMapping, assign, assignment, mapping, type_tag};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::java;

/// The Java backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Java;

impl Java {
    /// `write_u32` as `writeU32`.
    ///
    /// The runtime's methods are named by Java's convention and the walk
    /// asks in the wire's, so one of the two has to convert. Doing it
    /// here means a new operation is added to the walk under one name.
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

    fn path(name: &str) -> Path {
        Path::new(&name.split('.').collect::<Vec<_>>())
    }

    /// The reader of a map: `var v = Map.noEntries();`, the entries read
    /// into it, and `return Map.of(v);`.
    fn map_reading_body(class: &Path, body: java::Block) -> java::Block {
        let entries = entries_type(class);
        let on_class = |name: &str, args: Vec<java::Expr>| java::Expr::MethodCall {
            receiver: Some(Box::new(java::Expr::Path(class.clone()))),
            name: Ident::new(name),
            args,
        };
        let mut statements = vec![java::Statement::Declare {
            final_: false,
            type_: entries,
            name: Ident::new("v"),
            value: Some(on_class("noEntries", Vec::new())),
        }];
        statements.extend(body.statements);
        statements.push(java::Statement::Return(Some(on_class(
            "of",
            vec![Self::whole("v")],
        ))));
        java::Block { statements }
    }

    /// A reader's body, with the sample it answers with around it.
    ///
    /// `<T> v = zd_def_<T>(); … return v;`. Two things are settled
    /// here and neither is a spelling:
    ///
    /// - **Where the sample comes from.** Table 9's value, through the
    ///   generated defaulter, and not `new T()` or `null`. Every
    ///   position the stream does not carry is then already what
    ///   DDS-XTypes 1.3 Table 9 says it is, without the walk having to
    ///   reach each one; a `null` there would be a `NullPointerException`
    ///   at the dump instead of a member with a defined value, and
    ///   `new T()` would be a second answer to Table 9 beside
    ///   `default_java.rs`.
    /// - **What the name is.** The defaulter and the decoder are keyed
    ///   the same way — [`crate::emit::decode::decoder_name`] and
    ///   [`crate::default_java::defaulter_name`] differ only in the
    ///   four letters after `zd_` — so the one is the other with the
    ///   verb changed. A name that does not resolve is a `javac` error
    ///   naming the method, which is where a mismatch shows.
    fn reading_body(name: &str, sample: java::Type, body: java::Block) -> java::Block {
        let defaulter = crate::default_java::defaulter_from_decoder(name);
        let mut statements = vec![java::Statement::Declare {
            final_: false,
            type_: sample,
            name: Ident::new("v"),
            value: Some(Self::call(&defaulter, Vec::new())),
        }];
        statements.extend(body.statements);
        statements.push(java::Statement::Return(Some(Self::whole("v"))));
        java::Block { statements }
    }
}

/// The reader of a map works on its entries, so what the walk returns and what it
/// falls back to are entries: `return v;` becomes `return Map.of(v);` and the default
/// `v = zd_def_T();` (a map, as everywhere else) becomes `v = zd_def_T().entries();`.
///
/// zerodds-lint: recursion-depth 8 (the nesting of the statements of one codec routine; each
/// call descends into one block of the statement it is given)
fn map_returns(block: &mut java::Block, class: &Path) {
    let is_v = |expr: &java::Expr| {
        matches!(expr, java::Expr::Path(path)
            if path.segments().len() == 1 && path.segments()[0].as_str() == "v")
    };
    for statement in &mut block.statements {
        match statement {
            java::Statement::Return(Some(value)) if is_v(value) => {
                let entries = value.clone();
                *value = java::Expr::MethodCall {
                    receiver: Some(Box::new(java::Expr::Path(class.clone()))),
                    name: Ident::new("of"),
                    args: vec![entries],
                };
            }
            java::Statement::Expr(java::Expr::Binary { op: "=", lhs, rhs })
                if is_v(lhs)
                    && matches!(rhs.as_ref(), java::Expr::MethodCall { receiver: None, name, .. }
                        if name.as_str().starts_with("zd_def_")) =>
            {
                let call = (**rhs).clone();
                **rhs = java::Expr::MethodCall {
                    receiver: Some(Box::new(call)),
                    name: Ident::new("entries"),
                    args: Vec::new(),
                };
            }
            java::Statement::If {
                then, otherwise, ..
            } => {
                map_returns(then, class);
                if let Some(otherwise) = otherwise {
                    map_returns(otherwise, class);
                }
            }
            java::Statement::ForEach { body, .. }
            | java::Statement::For { body, .. }
            | java::Statement::While { body, .. } => map_returns(body, class),
            java::Statement::Switch { cases, .. } => {
                for case in cases {
                    map_returns(&mut case.body, class);
                }
            }
            _ => {}
        }
    }
}

/// The class path of a sample that is a generated map class
/// (`ZdTypes.ZdMap…`).
/// `ZdEntry_<key>_<value>[]`, the type of `Map.entries()` and `Map.noEntries()`, next
/// to the map class `class`. Spelled out: the generated code compiles for Java 8,
/// where `var` is a restricted type name that javac warns about.
fn entries_type(class: &Path) -> java::Type {
    let segments: Vec<&str> = class.segments().iter().map(Ident::as_str).collect();
    let (last, owner) = segments.split_last().unwrap_or((&"", &[]));
    let entry =
        crate::adapter::java::entry_class_of_map_class(last).unwrap_or_else(|| (*last).to_string());
    let mut path: Vec<&str> = owner.to_vec();
    path.push(&entry);
    java::Type::Array(Box::new(java::Type::named(Path::new(&path))))
}

fn map_class_of(sample: &java::Type) -> Option<Path> {
    let java::Type::Named { path, args } = sample else {
        return None;
    };
    let last = path.segments().last()?;
    (args.is_empty() && last.as_str().starts_with(crate::adapter::java::MAP_PREFIX))
        .then(|| path.clone())
}

impl CodecTarget for Java {
    type Expr = java::Expr;
    type Stmt = java::Statement;
    type Block = java::Block;
    type Type = java::Type;
    type Decl = java::Member;

    // -- types --------------------------------------------------------

    /// An unsigned local goes in the next signed type up, and that is
    /// not a stylistic choice.
    ///
    /// Java has no unsigned types. A `uint8` local declared `byte` holds
    /// 200 as −56, and `Writer.writeU8` takes an `int` because it means
    /// *a number in [0, 255]* — a caller handing it −56 has already lost
    /// the value, before any octet is written. So `uint8` and `uint16`
    /// are `int` and `uint32` is `long`.
    ///
    /// `uint64` is the exception: a `long` holding a **pattern** rather
    /// than a number, because there is no wider type.
    /// `writeU64(-1L)` writes `ffffffffffffffff`, which is
    /// 18446744073709551615.
    ///
    /// The same decision `adapter/java.rs` makes for a member's type and
    /// `runtime/java/Writer.java` for its parameters. Three places have
    /// to agree, and they are named here because a fourth reading is the
    /// one that would quietly disagree.
    fn scalar(kind: Scalar) -> Self::Type {
        java::Type::Primitive(match kind {
            Scalar::Bool => java::Primitive::Boolean,
            Scalar::Char => java::Primitive::Char,
            Scalar::Int8 => java::Primitive::Byte,
            Scalar::Int16 => java::Primitive::Short,
            Scalar::Index | Scalar::Size | Scalar::Int32 | Scalar::UInt8 | Scalar::UInt16 => {
                java::Primitive::Int
            }
            Scalar::Int64 | Scalar::UInt32 | Scalar::UInt64 => java::Primitive::Long,
            Scalar::Float => java::Primitive::Float,
            Scalar::Double => java::Primitive::Double,
            // binary128 has no Java primitive; `runtime/java` carries it
            // as a pair of longs behind `Float128`.
            Scalar::LongDouble => {
                return java::Type::named(Path::new(&["Float128"]));
            }
        })
    }

    fn named(name: &str) -> Self::Type {
        java::Type::named(Self::path(name))
    }

    fn nothing() -> Self::Type {
        java::Type::Void
    }

    /// `runtime/java` publishes each handle as a `public static final`
    /// class nested in `Writer`, so the name a generated file writes is
    /// qualified. Unqualified would compile only where the file happened
    /// to import the nested class by name.
    fn handle(kind: Handle) -> Self::Type {
        java::Type::named(Self::path(match kind {
            Handle::DHeader => "Writer.DHeader",
            Handle::EmHeader => "Writer.EmHeader",
            Handle::Pid => "Writer.Pid",
            // Not nested in either: the behaviour is a policy the
            // caller chooses, and `runtime/java` publishes it beside
            // `WcharWidth` and the rest. The probe imports it, so the
            // simple name is what the generated file writes.
            Handle::TryConstruct => "TryConstruct",
            Handle::ReaderMark => "Reader.Mark",
            Handle::PidRead => "Reader.PidRead",
            Handle::EmHeaderRead => "Reader.EmHeaderRead",
        }))
    }

    /// A Java `char` is already one code unit and widens to the `int`
    /// the runtime takes. The narrowing a `char` needs is the runtime's,
    /// which keeps the low byte — the same octet C reaches by
    /// truncating one step earlier.
    /// No cast: every holder a code point lives in here — `char`,
    /// `byte`, `short` — widens to the runtime's `int` parameter
    /// implicitly (JLS 5.1.2), and the explicit `(int)` this used to
    /// write is the one thing `-Xlint:cast -Werror` rejects on a javac
    /// that flags redundant casts — 68 tao/rti cells stopped compiling
    /// on the Linux test host the day its JDK started to.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// `Long.compareUnsigned` for the eight-octet width, whose value
    /// lives in a `long` as its bit pattern and whose own `<` is
    /// signed. The narrower widths live zero-extended in a wider
    /// signed type, where the inherited `<` is already the unsigned
    /// order.
    fn unsigned_less(octets: u8, left: Self::Expr, right: Self::Expr) -> Self::Expr {
        if octets == 1 {
            // An `octet` is a `byte` and a `uint8` an `int`; the low eight bits are the number
            // for both.
            let low = |value: java::Expr| {
                java::Expr::Paren(Box::new(Self::binary("&", value, Self::int(0xFF))))
            };
            return Self::binary("<", low(left), low(right));
        }
        if octets < 8 {
            return Self::binary("<", left, right);
        }
        Self::binary(
            "<",
            java::Expr::MethodCall {
                receiver: Some(Box::new(java::Expr::Path(Path::new(&["Long"])))),
                name: Ident::new("compareUnsigned"),
                args: vec![left, right],
            },
            Self::int(0),
        )
    }

    /// `false` before `true`, spelled as the conjunction — Java's `<`
    /// does not take two booleans. In the house style of `unary`'s `!`:
    /// the left key is out as `== false`.
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        Self::binary(
            "&&",
            java::Expr::Paren(Box::new(java::Expr::Binary {
                op: "==",
                lhs: Box::new(left),
                rhs: Box::new(java::Expr::Literal(Literal::Bool(false))),
            })),
            right,
        )
    }

    /// Four statics on the runtime's writer, because none of the forms
    /// has a Java expression: a `long` cast of a negative `int`
    /// sign-extends where the pattern needs a mask, a `boolean` has no
    /// conversion to an integer at all, and a float's bits are
    /// `Float.floatToRawIntBits`'s to read. The walk hands the integer
    /// form a `long` already — its `Int64` cast — so `bitPatternInt`
    /// only masks.
    fn bit_pattern(source: BitPatternSource, value: Self::Expr) -> Self::Expr {
        let call = |name: &str, args: Vec<java::Expr>| java::Expr::MethodCall {
            receiver: Some(Box::new(java::Expr::Path(Path::new(&["Writer"])))),
            name: Ident::new(name),
            args,
        };
        match source {
            BitPatternSource::Int { octets } => call(
                "bitPatternInt",
                vec![value, java::Expr::Literal(Literal::Int(octets.to_string()))],
            ),
            BitPatternSource::Bool => call("bitPatternBool", vec![value]),
            BitPatternSource::Float32 => call("bitPatternF32", vec![value]),
            BitPatternSource::Float64 => call("bitPatternF64", vec![value]),
        }
    }

    /// `runtime/java` publishes these as enum constants, one type per
    /// policy, so each name carries its own.
    fn constant(which: Constant) -> Self::Expr {
        java::Expr::Path(Self::path(match which {
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
            Constant::WstringCharactersTerminated2 => "WstringLengthPolicy.CHARACTERS_TERMINATED_2",
            Constant::FixedDeclared => "FixedDigitCount.DECLARED",
            Constant::FixedValuePrecision => "FixedDigitCount.VALUE_PRECISION",
            Constant::FixedValueStringDigits => "FixedDigitCount.VALUE_STRING_DIGITS",
            Constant::ParameterMember => "ParameterRole.MEMBER",
            Constant::ParameterOptional => "ParameterRole.OPTIONAL",
            Constant::TryConstructDiscard => "TryConstruct.DISCARD",
            Constant::TryConstructTrim => "TryConstruct.TRIM",
            Constant::TryConstructUseDefault => "TryConstruct.USE_DEFAULT",
            Constant::StatusOk => "Reader.Status.OK",
            Constant::StatusDiscarded => "Reader.Status.DISCARDED",
            Constant::StatusImpossibleLength => "Reader.Status.IMPOSSIBLE_LENGTH",
            Constant::StatusOutOfRange => "Reader.Status.OUT_OF_RANGE",
            Constant::StatusTruncated => "Reader.Status.TRUNCATED",
        }))
    }

    /// The materialiser's answer, not a second reading of it.
    ///
    /// `adapter/java.rs::unsigned_holder` decides what a `@bit_bound`
    /// set's field is declared as, and a codec that assembled the value
    /// in a different width would either narrow it or need a cast the
    /// generated source does not have.
    fn unsigned_holder(bits: u16) -> Self::Type {
        crate::adapter::java::unsigned_holder(bits)
    }

    /// Java has no free functions; the runtime class's own static
    /// method stands in.
    ///
    /// Which class is not a guess: C exports one namespace and Java has
    /// two, so the operation says which side of the wire it belongs to.
    /// A name this table does not know is written on `Reader` and
    /// `javac` names it — a compile error that stops the probe, rather
    /// than a call that goes to the wrong class and cannot.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let receiver = match method {
            "no_dheader" => "Writer",
            _ => "Reader",
        };
        java::Expr::MethodCall {
            receiver: Some(Box::new(java::Expr::Path(Path::new(&[receiver])))),
            name: Ident::new(&Self::camel(method)),
            args,
        }
    }

    // -- expressions --------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        java::Expr::Path(Self::path(text))
    }

    fn int(value: i128) -> Self::Expr {
        java::Expr::Literal(Literal::int(value))
    }

    fn literal(value: Literal) -> Self::Expr {
        java::Expr::Literal(value)
    }

    /// A free function is a `static` method on the generated class, so
    /// an unqualified call reaches it from another method of the same
    /// class without naming the class.
    /// `strcmp` is the walk's name for a three-way code-unit
    /// comparison; here it is `a.compareTo(b)`, which compares UTF-16
    /// code units. For the corpus's keys — all inside the BMP — that
    /// is the same order as UTF-8 octets; a supplementary code point
    /// mixed with U+E000..U+FFFF would part the two, and no corpus
    /// case declares such a key.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            let mut arguments = args.into_iter();
            if let (Some(left), Some(right)) = (arguments.next(), arguments.next()) {
                return java::Expr::MethodCall {
                    receiver: Some(Box::new(left)),
                    name: Ident::new("compareTo"),
                    args: vec![right],
                };
            }
            return java::Expr::MethodCall {
                receiver: None,
                name: Ident::new("strcmp"),
                args: Vec::new(),
            };
        }
        java::Expr::MethodCall {
            receiver: None,
            name: Ident::new(callee),
            args,
        }
    }

    /// A `fixed<P,S>` is a `java.math.BigDecimal` (CORBA 3.3 part 3 §1.5.6) and the runtime
    /// writes and reads its decimal text: the value goes in as `toPlainString()` and comes
    /// back through `new BigDecimal(text)`.
    fn runtime(receiver: &str, method: &str, mut args: Vec<Self::Expr>) -> Self::Expr {
        if method == "write_fixed" && !args.is_empty() {
            let value = args.remove(0);
            args.insert(
                0,
                java::Expr::MethodCall {
                    receiver: Some(Box::new(value)),
                    name: Ident::new("toPlainString"),
                    args: Vec::new(),
                },
            );
        }
        let call = java::Expr::MethodCall {
            receiver: Some(Box::new(java::Expr::Path(Self::path(receiver)))),
            name: Ident::new(&Self::camel(method)),
            args,
        };
        if method == "read_fixed" {
            return java::Expr::New {
                type_: Box::new(java::Type::named(Path::new(&[
                    "java",
                    "math",
                    "BigDecimal",
                ]))),
                args: vec![call],
            };
        }
        call
    }

    /// `runtime/java` carries the three HDDS axes: `beginPackedElement`
    /// and `finishPackedElement` on the writer and the reader, the
    /// DHEADER pair for a sequence element's own length, and
    /// `writeWstringUtf8Octets` / `readWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// `v.x`, or under [`JavaMapping::Psm`] `v.getX()`, which
    /// [`crate::adapter::java::assign`] turns into `v.setX(value)` where
    /// the walk writes the member.
    fn member(base: &str, field: &str) -> Self::Expr {
        if mapping() == JavaMapping::Psm {
            return crate::adapter::java::accessor(java::Expr::Path(Self::path(base)), field);
        }
        java::Expr::Field {
            value: Box::new(java::Expr::Path(Self::path(base))),
            name: Ident::new(field),
        }
    }

    /// The sample itself, with no dereference to write.
    fn whole(base: &str) -> Self::Expr {
        java::Expr::Path(Self::path(base))
    }

    /// `L` on anything that will not fit an `int`, and the pattern on
    /// anything that will not fit a `long`.
    ///
    /// A `case 16045690984833335998:` over an `unsigned long long` is
    /// past `Long.MAX_VALUE` and Java has no wider integer, so the label
    /// is written as the signed decimal with the same sixty-four bits —
    /// which is how `adapter/java.rs` holds the value it is compared
    /// against and how `Writer.writeU64` takes it. Anything else does
    /// not compile, which is the better failure but still a failure.
    fn wide_int(value: i128) -> Self::Expr {
        let text = if value > i128::from(i64::MAX) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let pattern = value as u64 as i64;
            format!("{pattern}L")
        } else if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
            format!("{value}L")
        } else {
            format!("{value}")
        };
        java::Expr::Literal(Literal::Int(text))
    }

    /// A count, with no suffix: Java has no unsigned literal, and `5u`
    /// is not a token it has at all.
    ///
    /// Wide enough to hold itself, through [`CodecTarget::wide_int`]: a
    /// `sequence<long, 3000000000>` bound is past `int` and the `L` is
    /// what keeps the comparison against a `long` count honest.
    fn unsigned_int(value: i128) -> Self::Expr {
        Self::wide_int(value)
    }

    /// A 64-bit pattern in a `long`, which is where Java keeps one.
    ///
    /// `writeU64(-1L)` writes `ffffffffffffffff`; the same reading in
    /// reverse is what a mask above `Long.MAX_VALUE` is here.
    fn wide_unsigned(value: u64) -> Self::Expr {
        #[allow(clippy::cast_possible_wrap)]
        let pattern = value as i64;
        java::Expr::Literal(Literal::Int(format!("{pattern}L")))
    }

    /// A signed 64-bit constant. Always suffixed: an enumerator's
    /// pattern is compared against a `long` and an `int` literal there
    /// would be widened after the comparison's type was chosen.
    fn wide_signed(value: i128) -> Self::Expr {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let pattern = if value > i128::from(i64::MAX) {
            value as u64 as i64
        } else {
            value as i64
        };
        java::Expr::Literal(Literal::Int(format!("{pattern}L")))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        java::Expr::Field {
            value: Box::new(value),
            name: Ident::new(field),
        }
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        crate::adapter::java::index(value, at)
    }

    /// `.length`, or the bound itself where the materialiser fixed one.
    ///
    /// Both a `sequence<T>` and an IDL array are a Java `T[]` here —
    /// `adapter/java.rs` says so, and `docs/language-limits.md` gives
    /// the reason a `map` is an array of a generated pair rather than a
    /// `java.util.Map`. So the length is a field and never `.size()`;
    /// a backend that asked a list for its size would generate source
    /// against a type the materialiser does not emit.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => java::Expr::Literal(Literal::int(i128::from(bound))),
            // A sequence under `JavaMapping::Psm` may be a `Collection`
            // and has no `.length`; `Sequences.count` is the length of
            // either, chosen by the type of its argument.
            None if mapping() == JavaMapping::Psm => {
                crate::adapter::java::sequences_count(Self::whole(base))
            }
            None => java::Expr::Field {
                value: Box::new(Self::whole(base)),
                name: Ident::new("length"),
            },
        }
    }

    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        java::Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// Java has no `Unary` node, and the three operators the walk uses
    /// — `!`, `~` and `-` — are written as a binary against the
    /// identity that leaves them alone.
    ///
    /// `~x` is `-1L ^ x` and not `0 - x`: the complement is what a
    /// bitmask's undeclared-bit mask is built with, and it is taken in
    /// sixty-four bits because that is the width the mask has.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        match op {
            "!" => java::Expr::Binary {
                op: "==",
                lhs: Box::new(value),
                rhs: Box::new(java::Expr::Literal(Literal::Bool(false))),
            },
            "~" => java::Expr::Paren(Box::new(java::Expr::Binary {
                op: "^",
                lhs: Box::new(java::Expr::Literal(Literal::Int("-1L".to_string()))),
                rhs: Box::new(value),
            })),
            _ => java::Expr::Binary {
                op,
                lhs: Box::new(java::Expr::Literal(Literal::int(0))),
                rhs: Box::new(value),
            },
        }
    }

    /// A bracket, written because the walk asked for one.
    ///
    /// The renderer adds none of its own, and Java's precedence is not
    /// C's where it matters most here: `&` binds *looser* than `!=`, so
    /// `bits & mask != 0` is a type error rather than the comparison
    /// the walk built, and looser than `^`, where it is a different
    /// number with nothing to say so.
    fn paren(value: Self::Expr) -> Self::Expr {
        java::Expr::Paren(Box::new(value))
    }

    /// A widening to `long` is written by the walk and not needed here.
    ///
    /// C casts a bit set's accumulator to `uint64_t` because its holder
    /// may be narrower; Java widens an integer to `long` on its own, and
    /// the cast is a `[cast]` warning — which `-Werror` turns into a
    /// probe that does not compile. Every cast the walk asks for with
    /// `long` as its target has an integer source, so dropping it is
    /// safe; a narrowing to `int` is not, and is kept.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == java::Type::Primitive(java::Primitive::Long) {
            return value;
        }
        // The walk's `signed char` — the C spelling of a map key's
        // signed order — is `byte` here: the cast keeps the low octet
        // and reads it in two's complement, which is the order the
        // corpus was recorded under.
        if type_ == java::Type::named(Self::path("signed char")) {
            return java::Expr::Cast {
                type_: Box::new(java::Type::Primitive(java::Primitive::Byte)),
                value: Box::new(value),
            };
        }
        // A `boolean` discriminator's labels arrive as 0 and 1 — the
        // integers the IDL evaluator gives — and Java has no conversion
        // from either. The label is the value it stands for.
        if type_ == java::Type::Primitive(java::Primitive::Boolean) {
            if let java::Expr::Literal(Literal::Int(text)) = &value {
                return java::Expr::Literal(Literal::Bool(text.trim_end_matches('L') != "0"));
            }
        }
        // `(int) 1` on a union label is redundant and `[cast]` says so.
        // An `int` literal is already an `int`; one carrying `L` is not
        // and keeps its cast.
        if type_ == java::Type::Primitive(java::Primitive::Int)
            && matches!(&value, java::Expr::Literal(Literal::Int(text)) if !text.ends_with('L'))
        {
            return value;
        }
        java::Expr::Cast {
            type_: Box::new(type_),
            value: Box::new(value),
        }
    }

    /// Nothing to take an address of and no array to decay: both forms
    /// hand the value.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    // -- statements ---------------------------------------------------

    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        java::Statement::Declare {
            final_: false,
            type_,
            name: Ident::new(name),
            value,
        }
    }

    /// An expression statement; an assignment to a member or to an
    /// element of a sequence goes through [`assign`].
    fn perform(value: Self::Expr) -> Self::Stmt {
        assignment(java::Statement::Expr(value))
    }

    /// `return v;` where the walk asked for a bare `return`.
    ///
    /// Only the reader leaves a routine early — the encoder emits no
    /// `return` at all — and a Java decoder answers with the sample it
    /// has built rather than writing through a parameter it cannot
    /// have. See [`CodecTarget::read_call`] for why. A bare `return;`
    /// here would leave the caller with whatever it had before, which
    /// is the silent-wrong-value shape.
    ///
    /// If the encoder ever does emit one, `javac` refuses `return v;`
    /// in a `void` method and names the file — loud, and at build time.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        java::Statement::Return(Some(value.unwrap_or_else(|| Self::whole("v"))))
    }

    /// Nothing. `javac` has no unread-parameter warning, and `zd_tc;`
    /// is not a Java statement.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// The call on its own. A method call is already a Java statement,
    /// and `(void) expr;` is not one at all.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        java::Statement::Expr(value)
    }

    /// `place = zd_dec_T(r, behaviour);`
    ///
    /// Java has no address to pass and no out-parameter to pass one in,
    /// so a decoder answers with its value. [`CodecTarget::routine`]
    /// and [`CodecTarget::give_back`] are the other two halves of that
    /// decision.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        assign(place, Self::call(callee, vec![Self::name("r"), behaviour]))
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

    /// A shorter array, because a Java array is its own length.
    ///
    /// C leaves the elements past the bound in the buffer and moves the
    /// count; there is no count here to move.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        if mapping() == JavaMapping::Psm {
            // Arrays.copyOf for an array, the first elements of a list for
            // a Collection: `Sequences.first` takes either.
            return vec![assign(
                Self::whole(base),
                crate::adapter::java::sequences_first(
                    Self::whole(base),
                    Self::cast(java::Type::Primitive(java::Primitive::Int), count),
                ),
            )];
        }
        vec![java::Statement::Expr(java::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::whole(base)),
            rhs: Box::new(java::Expr::MethodCall {
                receiver: Some(Box::new(java::Expr::Path(Path::new(&[
                    "java", "util", "Arrays",
                ])))),
                name: Ident::new("copyOf"),
                // Through `cast`, which drops the `(int)` on a literal
                // that is already one: a bound is written out as a
                // number and `-Xlint:cast -Werror` names a redundant
                // cast on it.
                args: vec![
                    Self::whole(base),
                    Self::cast(java::Type::Primitive(java::Primitive::Int), count),
                ],
            }),
        })]
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        java::Statement::If {
            condition,
            then,
            otherwise,
        }
    }

    /// `for (int i = 0; i < bound; i = i + 1)`.
    ///
    /// The step is written out rather than `i++` for the same reason C's
    /// is: the tree has no increment node, and one spelling in both
    /// backends keeps the two columns' generated source comparable by
    /// eye when their octets disagree.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        java::Statement::For {
            init: Box::new(java::Statement::Declare {
                final_: false,
                type_: java::Type::Primitive(java::Primitive::Int),
                name: Ident::new(counter),
                value: Some(java::Expr::Literal(Literal::int(0))),
            }),
            condition: java::Expr::Binary {
                op: "<",
                lhs: Box::new(java::Expr::Path(Path::new(&[counter]))),
                rhs: Box::new(bound),
            },
            step: java::Expr::Binary {
                op: "=",
                lhs: Box::new(java::Expr::Path(Path::new(&[counter]))),
                rhs: Box::new(java::Expr::Binary {
                    op: "+",
                    lhs: Box::new(java::Expr::Path(Path::new(&[counter]))),
                    rhs: Box::new(java::Expr::Literal(Literal::int(1))),
                }),
            },
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        java::Statement::While { condition, body }
    }

    fn leave() -> Self::Stmt {
        java::Statement::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        java::Statement::Comment(doc)
    }

    // -- assembly -----------------------------------------------------

    /// The array, sized — and its slots given something to be where the
    /// fill writes into them rather than assigning them.
    ///
    /// `new T[n]` throws rather than returning null when it cannot be
    /// served, so there is no guard and the fill follows
    /// unconditionally. An array and not a list, because that is what
    /// the materialiser declares the member as; a `new ArrayList<>(n)`
    /// here would compile against nothing.
    ///
    /// `new T[n]` of a class type is `n` nulls. A sequence's element
    /// decoder answers with a value and the slot is assigned outright,
    /// so nothing is needed; a `map`'s two halves are written into the
    /// entry at `v[i]`, and an entry that was never made is a null
    /// dereference at the first key. That is what `assigned_whole`
    /// separates — not a guess from the element's type, which cannot
    /// tell the two apart.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        if mapping() == JavaMapping::Psm && assigned_whole {
            // A new array or a new list of the kind the sample already is:
            // `Sequences.alloc` answers with the type of its first
            // argument, so the one routine serves an array and a
            // `Collection<E>`.
            let mut out = vec![assign(
                Self::whole(base),
                crate::adapter::java::sequences_alloc(
                    Self::whole(base),
                    Self::cast(java::Type::Primitive(java::Primitive::Int), count),
                ),
            )];
            out.extend(fill);
            return out;
        }
        let mut out = vec![java::Statement::Expr(java::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::whole(base)),
            rhs: Box::new(java::Expr::NewArray {
                element: Box::new(element.clone()),
                // `(int)` and not the count as it stands: a sequence
                // length is a `uint32` on the wire and this column
                // holds one in a `long`, which `new T[..]` does not
                // take. The narrowing is safe because the count was
                // already checked against what is left in the stream —
                // `sequence_body` refuses `IMPOSSIBLE_LENGTH` first.
                // Through `cast`, which keeps an array bound that is
                // already an `int` literal bare — `new long[(int) 4]`
                // is the redundant cast `-Xlint:cast -Werror` names.
                lengths: vec![Self::cast(
                    java::Type::Primitive(java::Primitive::Int),
                    count,
                )],
                elements: Vec::new(),
            }),
        })];
        if !assigned_whole {
            out.push(Self::count_up(
                "zd_e",
                Self::count_of(base, None),
                java::Block {
                    statements: vec![java::Statement::Expr(java::Expr::Binary {
                        op: "=",
                        lhs: Box::new(java::Expr::Index {
                            value: Box::new(Self::whole(base)),
                            index: Box::new(Self::name("zd_e")),
                        }),
                        rhs: Box::new(java::Expr::New {
                            type_: Box::new(element),
                            args: Vec::new(),
                        }),
                    })],
                },
            ));
        }
        out.extend(fill);
        out
    }

    /// The cast the three narrow holders need, and no other.
    ///
    /// `readI8` answers an `int`, because a number in [-128, 127] is an
    /// `int` once Java has widened it, and a `byte` field does not take
    /// one without being told to. The same for `short` and for the
    /// `char` an IDL `char` and `wchar` both map to. Everything else is
    /// already the field's type, and a cast there is what
    /// `-Xlint:cast -Werror` fails the build over.
    ///
    /// One narrowing and not two: where the walk has already asked for
    /// a width — the element form of a `wchar` under TAO's framing —
    /// that cast is folded into this one rather than written inside it.
    fn store(place: Self::Expr, type_: Self::Type, value: Self::Expr) -> Self::Stmt {
        let narrow = matches!(
            type_,
            java::Type::Primitive(java::Primitive::Byte | java::Primitive::Short)
        ) || type_ == java::Type::Primitive(java::Primitive::Char);
        if !narrow {
            return assign(place, value);
        }
        let inner = match value {
            java::Expr::Cast { value, .. } => *value,
            other => other,
        };
        assign(
            place,
            java::Expr::Cast {
                type_: Box::new(type_),
                value: Box::new(inner),
            },
        )
    }

    /// `<T> name = new <T>();` — a Java local of a class type is a
    /// reference to nothing until one is made.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        java::Statement::Declare {
            final_: false,
            type_: type_.clone(),
            name: Ident::new(name),
            value: Some(java::Expr::New {
                type_: Box::new(type_),
                args: Vec::new(),
            }),
        }
    }

    /// `runtime/java` publishes a header's parts behind accessors, so
    /// the field the walk names is a call here.
    ///
    /// A `match` and not a re-casing of the name: `end` is `isEnd()`
    /// and `extent` is computed from the length code, so a rule that
    /// only camel-cased would name two methods that are not there.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        java::Expr::MethodCall {
            receiver: Some(Box::new(value)),
            name: Ident::new(match field {
                "pid" => "pid",
                "end" => "isEnd",
                "ignore" => "ignore",
                "length" => "length",
                "member_id" => "memberId",
                "must_understand" => "mustUnderstand",
                "extent" => "extent",
                // A field the runtime does not publish is a `javac`
                // error naming the method, which is where a header that
                // grew a part shows up.
                other => other,
            }),
            args: Vec::new(),
        }
    }

    /// Absence is the value, so presence is a comparison and not a
    /// second field.
    fn present(base: &str, field: &str) -> Self::Expr {
        java::Expr::Binary {
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
        vec![assign(Self::member(base, field), Self::absent())]
    }

    /// One field per case, directly on the class.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// The sequence is the array.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        java::Expr::Field {
            value: Box::new(entry),
            name: Ident::new(if key { "key" } else { "value" }),
        }
    }

    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `v[i]`, and nothing around it: `adapter/java.rs` gives a sequence
    /// an array, whose elements are real slots at every element type.
    fn sequence_element(
        base: &str,
        index: Self::Expr,
        _element: Self::Type,
    ) -> (Vec<Self::Stmt>, Self::Expr, Vec<Self::Stmt>) {
        if mapping() == JavaMapping::Psm {
            // The slot may be an array's or a `Collection`'s; `assign`
            // writes it as `Sequences.put(v, i, value)`.
            return (
                Vec::new(),
                crate::adapter::java::sequence_slot(Self::whole(base), index),
                Vec::new(),
            );
        }
        (
            Vec::new(),
            Self::index(Self::elements(base), index),
            Vec::new(),
        )
    }

    /// The boxed field itself. Assigning to it is what makes the member
    /// present, so there is nothing to do first.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    fn elements(base: &str) -> Self::Expr {
        if mapping() == JavaMapping::Psm {
            return crate::adapter::java::sequence_view(Self::whole(base));
        }
        Self::whole(base)
    }

    fn absent() -> Self::Expr {
        java::Expr::Literal(Literal::Null)
    }

    /// `null`, because a `String` reference can be one.
    fn materialised(value: Self::Expr) -> Option<Self::Expr> {
        Some(java::Expr::Binary {
            op: "!=",
            lhs: Box::new(value),
            rhs: Box::new(Self::absent()),
        })
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        java::Block { statements }
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block.statements
    }

    /// `private static void encodeT(Writer w, T v)`.
    ///
    /// A prototype gets `None`: a Java method may call one declared
    /// below it, so there is nothing to declare ahead. `tolerate_unused`
    /// is dropped for the same kind of reason — `javac` does not warn on
    /// an uncalled private method, so there is no warning to suppress.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let mut body = spec.body?;
        let mut params = vec![java::Param {
            annotations: Vec::new(),
            final_: false,
            type_: java::Type::named(Path::new(&[if spec.writing { "Writer" } else { "Reader" }])),
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            varargs: false,
        }];
        // A map is a `java.util.Map` to the application (XTypes 1.3
        // §7.5.1.3.5) and its entries, in order, to the walk: the writer
        // takes the map and works on `map.entries()`, the reader works on an
        // array of entries and answers `Map.of(entries)`.
        let map_class = map_class_of(&spec.sample);
        // The writer is handed the sample; the reader answers with one.
        // See `CodecTarget::read_call`: Java has no out-parameter, so
        // the destination is the caller's assignment target and the
        // routine's own `v` is a local.
        if spec.writing {
            let name = if map_class.is_some() { "zd_map" } else { "v" };
            params.push(java::Param {
                annotations: Vec::new(),
                final_: false,
                type_: spec.sample.clone(),
                name: Ident::new(name),
                varargs: false,
            });
            if let Some(class_path) = &map_class {
                body.statements.insert(
                    0,
                    java::Statement::Declare {
                        final_: true,
                        type_: entries_type(class_path),
                        name: Ident::new("v"),
                        value: Some(java::Expr::MethodCall {
                            receiver: Some(Box::new(java::Expr::Path(Path::new(&["zd_map"])))),
                            name: Ident::new("entries"),
                            args: Vec::new(),
                        }),
                    },
                );
            }
        }
        params.extend(spec.extra.into_iter().map(|(type_, name)| java::Param {
            annotations: Vec::new(),
            final_: false,
            type_,
            name: Ident::new(&name),
            varargs: false,
        }));
        let (returns, body) = if spec.writing {
            (java::Type::Void, body)
        } else if let Some(class) = map_class {
            map_returns(&mut body, &class);
            (spec.sample.clone(), Self::map_reading_body(&class, body))
        } else {
            (
                spec.sample.clone(),
                Self::reading_body(&spec.name, spec.sample, body),
            )
        };
        Some(java::Member::Method(java::Method {
            doc: spec.doc,
            annotations: Vec::new(),
            // `public` and not `private`. A probe drops these members
            // into its own class and either reaches them; a generated
            // application does not — `ZdCodec` is the class it calls,
            // and a `private static` member of it is a codec nothing
            // outside the file can encode a sample with. Neither
            // `wireprobe-zerodds-java-gen` nor
            // `crates/endpoint-e2e/tests/java.rs` is affected: both
            // inline the members, where either modifier compiles, and
            // both cut the text at `public class ZdCodec {`, which is
            // the class's own line and not a member's.
            modifiers: vec![java::Modifier::Public, java::Modifier::Static],
            generics: Vec::new(),
            returns,
            name: Ident::new(&spec.name),
            params,
            throws: Vec::new(),
            body: Some(body),
        }))
    }

    // -- the materialiser's boundary ----------------------------------

    /// The materialiser's spelling, which for an aggregate is the
    /// nested path: `ZdTypes.common.First`, not `common_First`.
    ///
    /// `type_name` is the other one and is a *name* — what the codec
    /// method is called after. A codec that took `common_First` would
    /// name a class the materialiser never declares; every declaration
    /// nests inside `ZdTypes`, because Java allows one public top-level
    /// type per file and one file is what a probe compiles.
    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        crate::adapter::java::java_member_type(ast, id, "codec").unwrap_or_else(|_| {
            java::Type::named(Path::new(
                &[format!("UnresolvedType{}", id.slot()).as_str()],
            ))
        })
    }

    /// The Java materialiser's name for a type.
    ///
    /// `type_tag` refuses a type whose name nests past its depth cap and
    /// this signature cannot; the walk asking for a name has already
    /// planned the type's layout, so a refusal here would be the second
    /// opinion of two.
    ///
    /// The fallback carries the arena slot, and that is the whole point
    /// of it. `type_tag`'s own answer for an unresolvable type is the
    /// bare word `unknown`, and two of those in one model would be two
    /// types sharing one codec — a member written by the wrong encoder,
    /// with nothing red anywhere. Slotted, two failures cannot collide,
    /// and `zd_enc_unresolved_type_41` is a name a reader can trace back
    /// to the node that produced it.
    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0).unwrap_or_else(|_| format!("unresolved_type_{}", id.slot()))
    }

    fn escape(name: &str) -> String {
        java::escape_keyword(name)
    }

    /// `ZdTypes.ZdEntry_<key>_<value>`, qualified.
    ///
    /// The entry class is declared at the outer level and a codec that
    /// wrote the bare name would reach a class only a file that
    /// imported it by name would find. `adapter/java.rs` writes the
    /// same path for a `map` member's element type.
    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        crate::adapter::java::map_entry_name(ast, id).map_or_else(
            |_| format!("UnresolvedEntry{}", id.slot()),
            |name| format!("{}.{name}", crate::adapter::java::OUTER),
        )
    }

    /// Java has packages, so a module needs no prefix in a name.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// The member's own `@default`, or Table 9's value for its type.
    ///
    /// # Errors
    /// As [`crate::default_java::member_default`]: a `@default` this
    /// column cannot spell is reported rather than quietly replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_java::member_default(ast, node, type_, place)
    }

    /// `place = zd_def_<T>();` — Table 9, through the generated
    /// defaulter.
    ///
    /// A method and not a literal, for the reason `default_java.rs`
    /// gives: a literal cannot carry an aggregate, and two columns with
    /// two answers to Table 9 is a divergence no test turns red.
    ///
    /// # Errors
    /// Never here. Whether the type *has* a Java default is decided
    /// where the defaulter is built, and refused there by name.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_java::default_into(ast, id, place))
    }

    /// `ZdTypes.<modules>.<Enum>.<LITERAL>`.
    ///
    /// The materialiser declares a class of `public static final int`
    /// per enumeration and every reference is written from the outer
    /// class down; an unqualified `BLUE` names nothing.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        crate::default_java::enumerator_constant(ast, id, enumerator)
    }

    /// An enumeration is a class (CORBA 3.3 part 3 §1.5.9) and its integer is `value()`.
    fn enumerator_ordinal(_ast: &Ast, _id: Id<TypeNode>, value: Self::Expr) -> Self::Expr {
        java::Expr::MethodCall {
            receiver: Some(Box::new(value)),
            name: Ident::new("value"),
            args: Vec::new(),
        }
    }

    /// The label of a union whose discriminator is an enumeration is the instance of that
    /// value (`Color.from_int(1)`); any other discriminator takes the cast integer.
    fn case_label(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Self::Expr {
        let mut current = discriminator;
        for _ in 0..32 {
            match ast.type_(current).map(|node| &node.kind) {
                Ok(zerodds_ast::TypeKind::Alias { aliased }) => match aliased.resolved() {
                    Some(next) => current = next,
                    None => break,
                },
                Ok(zerodds_ast::TypeKind::Enumeration { .. }) => {
                    let owner = crate::adapter::java::declared_path(ast, current)
                        .unwrap_or_else(|_| format!("UnresolvedEnum{}", current.slot()));
                    return java::Expr::MethodCall {
                        receiver: Some(Box::new(java::Expr::Path(Path::new(
                            &owner.split('.').collect::<Vec<_>>(),
                        )))),
                        name: Ident::new("from_int"),
                        args: vec![Self::int(i128::from(i32::try_from(value).unwrap_or_else(
                            // A value above `int` is the bit pattern it occupies.
                            |_| {
                                u32::try_from(value & 0xFFFF_FFFF)
                                    .map_or(0, |bits| i32::from_ne_bytes(bits.to_ne_bytes()))
                            },
                        )))],
                    };
                }
                _ => break,
            }
        }
        Self::cast(Self::value_type(ast, discriminator), Self::wide_int(value))
    }
}
