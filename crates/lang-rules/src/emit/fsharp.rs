// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The F# backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as every column before
//! it, and none of them calls another. Where two columns disagree on an
//! octet, one has misread the specification.
//!
//! # The decisions this column turns on
//!
//! **No error union and no `try` at the call site.** Zig has no
//! exceptions, so [`crate::emit::zig::Zig`] threads a `ZigExpr::Fallible`
//! wrapper through every seam and prepends `try`. .NET has exceptions,
//! and `runtime/fsharp/ZdWire.fs`'s module documentation is where the
//! decision to use them is argued: a fallible runtime operation there
//! simply `raise`s `ZdWire.WireError` and unwinds with no syntax at the
//! call site, so this backend's `Self::Expr` is a plain
//! [`FsharpExpr`] with no fallible variant at all — the trait's own
//! defaults for [`crate::emit::CodecTarget::call`] and
//! [`crate::emit::CodecTarget::runtime`] already assume exactly this
//! shape and this backend uses them unchanged.
//!
//! **Decoders answer their value, not an out-parameter.** The trait's
//! documented default for [`crate::emit::CodecTarget::read_call`] is
//! C's: an address is passed and the callee writes through it. F# has
//! no address to pass, so — as C#, D, Go, Java, Python and TypeScript
//! already do, and as Zig does for its own different reason — this
//! backend overrides it: `place = zd_dec_T(r, behaviour)`.
//!
//! **`@external` needs nothing.** `crates/lang-rules/src/adapter/
//! fsharp.rs`'s module documentation carries why: an F# class is a
//! reference already, so every `external_*` seam takes the trait's
//! default, which is the plain member access every other seam already
//! reaches.
//!
//! **`@optional` is `'T option`, and the read direction wraps in
//! `Some`.** A decoded `T` does not coerce into a `'T option` the way it
//! coerces into Zig's `?T`; [`FsharpExpr::OptionalSlot`] is the marker
//! that tells [`Fsharp::read_call`] and [`Fsharp::default_into`] to
//! write `Some(value)` rather than `value`.
//!
//! **There is no one cast operator, so [`CodecTarget::cast`] chooses.**
//! `enum<T> v` into an enumeration — the walk casts a union label into
//! its discriminator's type — a bare numeric conversion function
//! (`int`, `uint32`, `int64`, …) between integer widths, and
//! `ZdWire.asI64`/`ZdWire.asU64` where the walk widens a value it
//! cannot see the type of.
//!
//! **F#'s own `<` already answers every ordering question.** Unlike
//! Java, whose `long` holding an IDL `unsigned long long` sorts signed,
//! F#'s `uint64` is a real unsigned type and its `<` is already the
//! ascending-key order §7.3.1.2.1.19 wants; unlike C#, C++ and
//! TypeScript, F#'s `<` on two `bool` values compiles and answers
//! `false` before `true` — measured (`fsc` 9.0.106) — so neither
//! [`CodecTarget::unsigned_less`] nor [`CodecTarget::bool_less`] is
//! overridden here; the trait's own `binary("<", …)` default is exactly
//! right.
//!
//! **No unused-parameter or discard machinery, unlike Zig's.** Zig's
//! `routine` runs a whole assembly pass to decide `var` against `const`
//! and to keep exactly the discards a body needs, because Zig refuses
//! both an unused parameter and a pointless discard. Measured
//! (`fsc` 9.0.106, `--warnaserror+ --warn:5`): neither an unused
//! function parameter nor an unused `let` binding is diagnosed at all.
//! [`Fsharp::routine`] is therefore the simple assembly Zig's own
//! documentation says a target with no such rule gets.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::fsharp::{
    MODULE_NAME, field_name, fsharp_map_entry_name, fsharp_type_name, fsharp_value_type, typed_int,
};
use crate::default_fsharp;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::fsharp;

/// The F# backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fsharp;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The name the generated file's `ZdWire` value is reached through — the
/// module `runtime/fsharp/ZdWire.fs` declares, `#load`ed by the
/// generator's `Main.fsx` beside the generated `Types.fs`.
const RUNTIME: &str = "ZdWire";

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`; this column's member is a `byte` and the signed reading is
/// `sbyte (int8 v)` — a two-step conversion through the same width, the
/// bit pattern read back as signed.
const SIGNED_CHAR: &str = "signed char";

/// An expression, with the one property the walk's seams cannot carry
/// on a bare [`fsharp::Expr`]: whether the destination is an
/// `@optional` member's `'T option` slot, which needs the decoded value
/// wrapped in `Some`.
#[derive(Debug, Clone)]
pub enum FsharpExpr {
    /// An ordinary expression.
    Plain(fsharp::Expr),
    /// The `'T option` field of an `@optional` member, as a place a
    /// decoded `T` is about to be written into.
    OptionalSlot(fsharp::Expr),
}

/// An F# type, with the two properties [`CodecTarget::cast`] needs and
/// the tree cannot carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsharpType {
    /// The type as written.
    pub tree: fsharp::Type,
    /// Whether a cast into it converts from an integer with `enum<T>`.
    pub enum_like: bool,
    /// The holder, where the type names an integer under its own name
    /// — a bitmask, whose F# form is `type Flags = uint16`. The tree
    /// says `Named("common_Flags")` and F#'s numeric conversion
    /// functions (`uint16`, …) know nothing of that name, only of the
    /// primitive underneath it, so [`CodecTarget::cast`] needs the
    /// width carried here to spell `uint16(zd_bits)` and not a
    /// conversion function that does not exist.
    pub bitmask_holder: Option<fsharp::Primitive>,
}

impl FsharpType {
    fn plain(tree: fsharp::Type) -> Self {
        Self {
            tree,
            enum_like: false,
            bitmask_holder: None,
        }
    }
}

/// The amount of a shift, as `int`.
///
/// F# types the shift amount as `int` whatever the shifted value is —
/// `(<<<) : ^a -> int -> ^a` — while the walk hands the amount in the
/// width of the value beside it, because every other column wants the
/// two operands to agree. A bitset offset therefore arrives as `0UL`,
/// and `zdBits >>> 0UL` is `error FS0001` ("expected int32", measured
/// with `dotnet fsi`, .NET SDK 9.0.106). A literal drops the suffix
/// `crate::adapter::fsharp::typed_int` gave it — a bare integer
/// literal is `int32` in F# — and anything else goes through `int`.
fn shift_amount(value: fsharp::Expr) -> fsharp::Expr {
    if let fsharp::Expr::Literal(Literal::Int(digits)) = &value {
        let bare = digits.trim_end_matches(char::is_alphabetic);
        if !bare.is_empty() && bare != "-" {
            return fsharp::Expr::Literal(Literal::Int(bare.to_string()));
        }
    }
    fsharp::Expr::call("int", vec![value])
}

fn plain(e: fsharp::Expr) -> FsharpExpr {
    FsharpExpr::Plain(e)
}

/// The expression inside, whichever form it took.
fn inner(expr: FsharpExpr) -> fsharp::Expr {
    match expr {
        FsharpExpr::Plain(e) | FsharpExpr::OptionalSlot(e) => e,
    }
}

impl Fsharp {
    /// A runtime name, qualified: `ZdWire.X`.
    fn runtime_name(name: &str) -> fsharp::Expr {
        fsharp::Expr::dot(fsharp::Expr::Name(Ident::new(RUNTIME)), name)
    }

    /// A runtime **method**'s F# spelling: `write_u32` is `WriteU32`,
    /// every `_`-separated segment with its first letter raised — the
    /// method names `runtime/fsharp/ZdWire.fs`'s `Writer`/`Reader`
    /// classes publish, .NET's own convention and the one this runtime
    /// was written against.
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

    /// A runtime **free function**'s F# spelling: `bit_pattern_int` is
    /// `bitPatternInt` — every segment after the first with its first
    /// letter raised, the first left alone. F#'s own convention for a
    /// module-level `let` binding, unlike a method's, and the one
    /// `runtime/fsharp/ZdWire.fs`'s free functions — `noDheader`,
    /// `strcmp`, `boolLess`, `checkedChar`, `asI64`, … — are all
    /// spelled in.
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

    fn text(value: &str) -> FsharpExpr {
        plain(fsharp::Expr::Literal(Literal::Str(value.to_string())))
    }

    /// One value of a runtime union: `ZdWire.ParameterRole.Member`. This
    /// column's runtime types are F# unions, not enumerations, so a
    /// case is reached the same way a value would be — `ZdWire.Case`,
    /// with no accessor between them.
    fn runtime_case(type_: &str, case: &str) -> FsharpExpr {
        let _ = type_;
        plain(Self::runtime_name(case))
    }

    /// Whether the resolved type behind `id` is an enumeration.
    fn is_enumeration(ast: &Ast, id: Id<TypeNode>) -> bool {
        let mut current = id;
        for _ in 0..MAX_TYPE_DEPTH {
            match ast.type_(current).map(|node| node.kind.clone()) {
                Ok(TypeKind::Enumeration { .. }) => return true,
                Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                    Some(target) => current = target,
                    None => return false,
                },
                _ => return false,
            }
        }
        false
    }

    /// The bitmask holder behind `id`, resolved through any typedef, or
    /// `None` where `id` is not a bitmask at all.
    fn bitmask_holder(ast: &Ast, id: Id<TypeNode>) -> Option<fsharp::Primitive> {
        let mut current = id;
        for _ in 0..MAX_TYPE_DEPTH {
            match ast.type_(current).map(|node| node.kind.clone()) {
                Ok(TypeKind::Bitmask { .. }) => {
                    let bound = ast.effective_bit_bound(current);
                    return Some(crate::adapter::fsharp::fsharp_holder(bound));
                }
                Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                    Some(target) => current = target,
                    None => return None,
                },
                _ => return None,
            }
        }
        None
    }

    /// An integer literal given the width of the slot it is declared
    /// into — `0` into a `uint32` local is `0u`. Anything that is not a
    /// bare integer literal, and any slot that is not a primitive, is
    /// left as it stands: the walk's own `cast` seam already answers
    /// those.
    fn widen_literal(tree: &fsharp::Type, value: fsharp::Expr) -> fsharp::Expr {
        let (fsharp::Type::Primitive(primitive), fsharp::Expr::Literal(Literal::Int(_))) =
            (tree, &value)
        else {
            return value;
        };
        typed_int(*primitive, value)
    }

    /// What a slot the walk declared without an initialiser holds until
    /// the statement that fills it runs.
    fn empty_value(tree: &fsharp::Type) -> fsharp::Expr {
        match tree {
            fsharp::Type::Primitive(primitive) => match primitive {
                fsharp::Primitive::Bool => fsharp::Expr::Literal(Literal::Bool(false)),
                fsharp::Primitive::Float32 | fsharp::Primitive::Double => {
                    fsharp::Expr::Literal(Literal::Float("0.0".to_string()))
                }
                fsharp::Primitive::String => fsharp::Expr::Literal(Literal::Str(String::new())),
                fsharp::Primitive::Unit => fsharp::Expr::Name(Ident::new("()")),
                other => typed_int(*other, fsharp::Expr::int(0)),
            },
            fsharp::Type::Named(name) if name.as_str() == "ZdWire.PidRead" => {
                Self::runtime_nothing("noPidRead")
            }
            fsharp::Type::Named(name) if name.as_str() == "ZdWire.EmHeaderRead" => {
                Self::runtime_nothing("noEmheaderRead")
            }
            _ => fsharp::Expr::None_,
        }
    }

    /// `ZdWire.f ()` — a runtime free function of no arguments, curried
    /// exactly as [`CodecTarget::runtime_static`] renders one.
    fn runtime_nothing(name: &str) -> fsharp::Expr {
        fsharp::Expr::CurriedCall {
            func: Box::new(Self::runtime_name(name)),
            args: Vec::new(),
        }
    }

    /// Whether `tree` is a fixed-width integer this column has a plain
    /// conversion function for.
    fn integer_conversion(tree: &fsharp::Type) -> Option<&'static str> {
        let fsharp::Type::Primitive(primitive) = tree else {
            return None;
        };
        use fsharp::Primitive as P;
        Some(match primitive {
            P::SByte => "sbyte",
            P::Byte => "byte",
            P::Int16 => "int16",
            P::UInt16 => "uint16",
            P::Int32 => "int32",
            P::UInt32 => "uint32",
            P::Int64 => "int64",
            P::UInt64 => "uint64",
            _ => return None,
        })
    }
}

impl CodecTarget for Fsharp {
    type Expr = FsharpExpr;
    type Stmt = fsharp::Stmt;
    type Block = Vec<fsharp::Stmt>;
    type Type = FsharpType;
    type Decl = fsharp::Decl;

    // -- types --------------------------------------------------------

    /// The fixed-width F# type each scalar local takes. `Index` is
    /// `uint32` — a sequence length is a `uint32` on the wire — and
    /// `Size` is `int32`, which is what a .NET array's own `Length` and
    /// a byte position are held in.
    fn scalar(kind: Scalar) -> Self::Type {
        use fsharp::Primitive as P;
        FsharpType::plain(match kind {
            Scalar::Bool => fsharp::Type::Primitive(P::Bool),
            Scalar::Char | Scalar::UInt8 => fsharp::Type::Primitive(P::Byte),
            Scalar::Int8 => fsharp::Type::Primitive(P::SByte),
            Scalar::Int16 => fsharp::Type::Primitive(P::Int16),
            Scalar::UInt16 => fsharp::Type::Primitive(P::UInt16),
            Scalar::Int32 => fsharp::Type::Primitive(P::Int32),
            Scalar::UInt32 | Scalar::Index => fsharp::Type::Primitive(P::UInt32),
            Scalar::Int64 => fsharp::Type::Primitive(P::Int64),
            Scalar::UInt64 => fsharp::Type::Primitive(P::UInt64),
            Scalar::Float => fsharp::Type::Primitive(P::Float32),
            Scalar::Double => fsharp::Type::Primitive(P::Double),
            // The decimal text; the module documentation of
            // `crates/lang-cst/src/fsharp/mod.rs` says why .NET has no
            // narrower workaround needed and no wider type to use.
            Scalar::LongDouble => fsharp::Type::text(),
            Scalar::Size => fsharp::Type::Primitive(P::Int32),
        })
    }

    fn named(name: &str) -> Self::Type {
        FsharpType::plain(fsharp::Type::named(name))
    }

    fn nothing() -> Self::Type {
        FsharpType::plain(fsharp::Type::Primitive(fsharp::Primitive::Unit))
    }

    /// The runtime's handles, qualified.
    fn handle(kind: Handle) -> Self::Type {
        FsharpType::plain(fsharp::Type::named(match kind {
            Handle::DHeader => "ZdWire.DHeader",
            Handle::EmHeader => "ZdWire.EmHeader",
            Handle::Pid => "ZdWire.Pid",
            Handle::TryConstruct => "ZdWire.TryConstruct",
            Handle::ReaderMark => "ZdWire.ReaderMark",
            Handle::PidRead => "ZdWire.PidRead",
            Handle::EmHeaderRead => "ZdWire.EmHeaderRead",
        }))
    }

    /// A `char`'s member already is the runtime's own type — `byte`,
    /// which is what `ZdWire.Writer.WriteChar` takes — and there is
    /// nothing to convert.
    ///
    /// A `wchar`'s member is a `uint16` and `WriteWchar` takes a code
    /// point as `int`, because the four-octet form of §7.4.1.1.1 Table
    /// 30 is not a `uint16` on the wire. F# converts no numeric type
    /// implicitly, so the conversion is written here — the same
    /// asymmetry the walk spells out in the other direction, where
    /// `crates/lang-rules/src/emit/decode.rs` casts `read_wchar`'s
    /// answer back to a `uint16` itself.
    fn code_point(value: Self::Expr, narrow: bool) -> Self::Expr {
        if narrow {
            return value;
        }
        plain(fsharp::Expr::call("int", vec![inner(value)]))
    }

    /// The runtime's policy values: numbers and strings where
    /// `ZdWire.fs` takes them so, and the runtime's own union cases for
    /// the enumerated kinds.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => plain(fsharp::Expr::int(2)),
            Constant::WcharWidth4 => plain(fsharp::Expr::int(4)),
            Constant::WcharFixed => plain(fsharp::Expr::Literal(Literal::Bool(false))),
            Constant::WcharGiopLengthPrefixed => plain(fsharp::Expr::Literal(Literal::Bool(true))),
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
            Constant::ParameterMember => Self::runtime_case("ParameterRole", "Member"),
            Constant::ParameterOptional => Self::runtime_case("ParameterRole", "Optional"),
            Constant::TryConstructDiscard => Self::runtime_case("TryConstruct", "Discard"),
            Constant::TryConstructTrim => Self::runtime_case("TryConstruct", "Trim"),
            Constant::TryConstructUseDefault => Self::runtime_case("TryConstruct", "UseDefault"),
            Constant::StatusOk => Self::runtime_case("Status", "Ok"),
            Constant::StatusDiscarded => Self::runtime_case("Status", "Discarded"),
            Constant::StatusImpossibleLength => Self::runtime_case("Status", "ImpossibleLength"),
            Constant::StatusOutOfRange => Self::runtime_case("Status", "OutOfRange"),
            Constant::StatusTruncated => Self::runtime_case("Status", "Truncated"),
        }
    }

    /// The unsigned type wide enough for `bits` of a bit set — the same
    /// answer the materialiser's `fsharp_holder` gives its fields.
    fn unsigned_holder(bits: u16) -> Self::Type {
        FsharpType::plain(fsharp::Type::Primitive(
            crate::adapter::fsharp::fsharp_holder(bits),
        ))
    }

    /// `runtime/fsharp` carries the three HDDS axes: `BeginPackedElement`
    /// and `FinishPackedElement` on the writer and `ReaderBeginPackedElement`
    /// / `ReaderFinishPackedElement` on the reader, the DHEADER pair for a
    /// sequence element's own length, and `WriteWstringUtf8Octets` /
    /// `ReadWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        // A type with no F# form was refused by the materialiser before a
        // codec walk ran — `fsharp_items` answers `Unevaluated` for the
        // same constructs, and `generate::source` calls `declarations`
        // before `codec`. The trait's signature carries no error, so the
        // unreachable arm names a type F# does not have: `fsc` then stops
        // on an unbound name. `unit` would have been a *valid* type, and a
        // member declared `unit` writes nothing and reads nothing — the
        // member would leave the wire with the file still compiling. The
        // OCaml column answers the same shape the same way
        // (`emit/ocaml.rs`, `zd_no_ocaml_form`).
        let tree = fsharp_value_type(ast, id, "codec")
            .unwrap_or_else(|_| fsharp::Type::named("ZdNoFsharpForm"));
        FsharpType {
            tree,
            enum_like: Self::is_enumeration(ast, id),
            bitmask_holder: Self::bitmask_holder(ast, id),
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
        fsharp_map_entry_name(ast, id).unwrap_or_else(|_| format!("ZdUnresolvedEntry{}", id.slot()))
    }

    /// Nothing: a declared type's F# name is already flat — the adapter
    /// folds the modules in — and an enumerator lives in its enum's own
    /// scope, which [`CodecTarget::enumerator_constant`]'s override
    /// reaches.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    fn escape(name: &str) -> String {
        field_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", field_name("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        plain(fsharp::Expr::Name(Ident::new(text)))
    }

    fn int(value: i128) -> Self::Expr {
        plain(fsharp::Expr::int(value))
    }

    /// The raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten into the tree's own boolean where they are one.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return plain(fsharp::Expr::Literal(value));
        };
        match text.as_str() {
            "true" => plain(fsharp::Expr::Literal(Literal::Bool(true))),
            "false" => plain(fsharp::Expr::Literal(Literal::Bool(false))),
            _ => plain(fsharp::Expr::Literal(value)),
        }
    }

    /// `int64(v)` or `uint64(v)`, whichever the value's own range needs
    /// — a union label the size of `16045690984833335998` does not fit
    /// an `int64`, and the trait's own documentation names that exact
    /// value. F# gives neither width bare literal syntax: measured
    /// (`fsi`, .NET SDK 9.0.106), `let x : int64 = 5` is `error FS0001`
    /// with no suffix, so this seam wraps in the conversion function
    /// `crate::adapter::fsharp::typed_int` names, not a literal suffix
    /// — `crates/lang-cst/src/fsharp/mod.rs`'s tree carries no signed
    /// arbitrary-precision literal form the way Zig's `comptime_int` is
    /// one.
    fn wide_int(value: i128) -> Self::Expr {
        if let Ok(narrow) = i64::try_from(value) {
            plain(typed_int(
                fsharp::Primitive::Int64,
                fsharp::Expr::int(narrow.into()),
            ))
        } else {
            plain(typed_int(
                fsharp::Primitive::UInt64,
                fsharp::Expr::int(value),
            ))
        }
    }

    /// `uint32(v)` — a count, an element index and a member id are all
    /// `uint32` on the wire, and this column's own `Scalar::Index`
    /// answers the same width for the same reason.
    fn unsigned_int(value: i128) -> Self::Expr {
        plain(typed_int(
            fsharp::Primitive::UInt32,
            fsharp::Expr::int(value),
        ))
    }

    /// `uint64(v)`.
    fn wide_unsigned(value: u64) -> Self::Expr {
        plain(typed_int(
            fsharp::Primitive::UInt64,
            fsharp::Expr::int(i128::from(value)),
        ))
    }

    /// `int64(v)`.
    fn wide_signed(value: i128) -> Self::Expr {
        plain(typed_int(
            fsharp::Primitive::Int64,
            fsharp::Expr::int(value),
        ))
    }

    /// `v.x` — and `v._d` as it stands: this column keeps the IDL-facing
    /// spelling, underscore and all.
    fn member(base: &str, field: &str) -> Self::Expr {
        plain(fsharp::Expr::dot(
            fsharp::Expr::Name(Ident::new(base)),
            field,
        ))
    }

    fn whole(base: &str) -> Self::Expr {
        plain(fsharp::Expr::Name(Ident::new(base)))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        plain(fsharp::Expr::dot(inner(value), field))
    }

    /// The runtime's own spelling for a header field the walk asks for
    /// by its wire name.
    ///
    /// `runtime/fsharp/ZdWire.fs` holds `PidRead` and `EmHeaderRead` as
    /// records, and a record field is a name in the language: `end` is a
    /// reserved word of the F# specification §3.4 and `zd_p.end` is
    /// `error FS0599`, "missing qualification after '.'". The other
    /// names differ from the wire's for the same reason the C# column's
    /// do — a .NET member is `PascalCase` — so the mapping is written
    /// once here rather than bent into the runtime.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        plain(fsharp::Expr::dot(
            inner(value),
            match field {
                "pid" | "member_id" => "Id",
                "end" => "ListEnd",
                "ignore" => "Ignore",
                "length" => "Length",
                "must_understand" => "MustUnderstand",
                "extent" => "Extent",
                other => other,
            },
        ))
    }

    /// `value.[int at]` — an F# array index is always `int`, and this
    /// walk's own counters are `uint32` ([`Self::count_up`]'s own
    /// documentation says why); measured (`fsi`, .NET SDK 9.0.106),
    /// `v.[i]` with `i: uint32` is `error FS0001`. The cast is written
    /// here and not at every call site, because every index this walk
    /// ever builds ends up as an array subscript.
    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        plain(fsharp::Expr::Index {
            value: Box::new(inner(value)),
            index: Box::new(fsharp::Expr::call("int", vec![inner(at)])),
        })
    }

    /// `uint32 v.len.Length`, or the bound where the materialiser fixed
    /// one: `.Length` is an `int32` and a sequence's count is a
    /// `uint32` on the wire.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => plain(typed_int(
                fsharp::Primitive::UInt32,
                fsharp::Expr::int(i128::from(bound)),
            )),
            None => plain(fsharp::Expr::call(
                "uint32",
                vec![fsharp::Expr::dot(
                    fsharp::Expr::Name(Ident::new(base)),
                    "Length",
                )],
            )),
        }
    }

    /// The array itself: an F# array value is the elements.
    fn elements(base: &str) -> Self::Expr {
        plain(fsharp::Expr::Name(Ident::new(base)))
    }

    /// `v.x` — a union is a class carrying its branches directly.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `entry.Key` and `entry.Value`, the two properties the
    /// materialiser gives a map's entry class.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        plain(fsharp::Expr::dot(
            inner(entry),
            if key { "Key" } else { "Value" },
        ))
    }

    fn absent() -> Self::Expr {
        plain(fsharp::Expr::None_)
    }

    /// Nothing: an F# array and string always exist once the runtime
    /// answered — an allocation failure is a `.NET` `OutOfMemoryException`
    /// no code here is built to survive anyway — so there is no absent
    /// state for the guard to test.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// `v.x.Value` — the value a `'T option` holds, given that it holds
    /// one. Only ever reached inside a branch the presence test
    /// guarded, where the unwrap cannot trip.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(fsharp::Expr::dot(inner(Self::member(base, field)), "Value"))
    }

    /// `v.x.IsSome`.
    fn present(base: &str, field: &str) -> Self::Expr {
        plain(fsharp::Expr::dot(
            inner(Self::member(base, field)),
            "IsSome",
        ))
    }

    /// Nothing on the way in — assigning `Some value` is what makes the
    /// member present, through [`Self::optional_destination`]'s marker
    /// — and `v.x <- None` on the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![fsharp::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: fsharp::Expr::None_,
        }]
    }

    /// The marked place; [`Self::read_call`] wraps the decoded `T` in
    /// `Some` before the assignment, because unlike Zig's `?T` an F#
    /// `'T option` does not coerce from a bare `T`.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            FsharpExpr::OptionalSlot(inner(Self::member(base, field))),
        )
    }

    // `external_value`, `external_destination`, `external_present`,
    // `external_set_present`, `external_case_value` and
    // `external_case_destination` all take the trait's default: an F#
    // class is a reference already, so `@external` reaches the plain
    // member or case accessor unchanged. The module documentation of
    // `crates/lang-rules/src/adapter/fsharp.rs` carries the reasoning.

    /// The operators, unchanged: F# spells every one of them the same
    /// as the walk does, `&&`/`||` included. `=` never reaches a
    /// rendered expression — [`Self::perform`] turns it into an
    /// assignment statement.
    /// The operator, with the spellings F# does not share with the
    /// walk's own C-family default: the shifts and the three bitwise
    /// operators take a third symbol — `<<` is `<<<`, `>>` is `>>>`,
    /// `&` is `&&&`, `|` is `|||`, `^` is `^^^` — because F# still
    /// parses a bare `&`/`|` as its own long-deprecated boolean
    /// `and`/`or` (measured, `fsc` 9.0.106: `zdBits & mask` on two
    /// `uint64` is `error FS0001`, "expected bool"). `==` is `=`,
    /// F#'s equality operator — `==` is reference equality on a class
    /// and refuses two `int32 []` outright — and `!=` is `<>`.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        let spelled = match op {
            "<<" => "<<<",
            ">>" => ">>>",
            "&" => "&&&",
            "|" => "|||",
            "^" => "^^^",
            "==" => "=",
            "!=" => "<>",
            other => other,
        };
        let right = match op {
            "<<" | ">>" => shift_amount(inner(rhs)),
            _ => inner(rhs),
        };
        plain(fsharp::Expr::Binary {
            op: spelled,
            lhs: Box::new(inner(lhs)),
            rhs: Box::new(right),
        })
    }

    /// `not value` for `!`, `-value` for `-`, `~~~value` for `~` — F#'s
    /// own bitwise complement, spelled with three tildes and not one.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        let operand = inner(value);
        let spelled = match op {
            "!" => "not ",
            "~" => "~~~",
            other => other,
        };
        plain(fsharp::Expr::Unary {
            op: spelled,
            value: Box::new(operand),
        })
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        plain(fsharp::Expr::Paren(Box::new(inner(value))))
    }

    /// The conversion, spelled by what it converts into. A `bool`
    /// target with an integer literal becomes the boolean literal, as
    /// in the sibling columns.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        let operand = inner(value);
        if type_.tree == fsharp::Type::named(SIGNED_CHAR) {
            return plain(fsharp::Expr::call("sbyte", vec![operand]));
        }
        if type_.tree == fsharp::Type::Primitive(fsharp::Primitive::Bool) {
            if let fsharp::Expr::Literal(Literal::Int(text)) = &operand {
                return plain(fsharp::Expr::Literal(Literal::Bool(text != "0")));
            }
        }
        if type_.enum_like {
            let fsharp::Type::Named(name) = &type_.tree else {
                return plain(operand);
            };
            // `enum<T>` takes only `int32`: measured (`fsi`, .NET SDK
            // 9.0.106), `enum<Mode> (int64 0)` is `error FS0001`,
            // "expected 'int32' … 'int64'" — unlike the numeric
            // conversion functions, `enum<T>`'s argument is not widened
            // through .NET's usual numeric SRTP. The value this walk
            // hands a union label through is `Self::wide_int`'s
            // `int64`/`uint64`, so it is narrowed here first.
            return plain(fsharp::Expr::call(
                &format!("enum<{}>", name.as_str()),
                vec![fsharp::Expr::call("int", vec![operand])],
            ));
        }
        if type_.tree == fsharp::Type::Primitive(fsharp::Primitive::Int64) {
            return plain(fsharp::Expr::Call {
                func: Box::new(Self::runtime_name("asI64")),
                args: vec![operand],
            });
        }
        if type_.tree == fsharp::Type::Primitive(fsharp::Primitive::UInt64) {
            return plain(fsharp::Expr::Call {
                func: Box::new(Self::runtime_name("asU64")),
                args: vec![operand],
            });
        }
        if let Some(conversion) = Self::integer_conversion(&type_.tree) {
            return plain(fsharp::Expr::call(conversion, vec![operand]));
        }
        // A bitmask's own name (`common_Flags`) is a type abbreviation
        // and not a conversion function; [`Self::bitmask_holder`]
        // carries the primitive underneath it, which `typed_int` knows
        // how to spell — [`crate::emit::CodecTarget::bitmask_value`]'s
        // default already routes here through this seam.
        if let Some(holder) = type_.bitmask_holder {
            return plain(typed_int(holder, operand));
        }
        plain(operand)
    }

    /// The value, unchanged: an F# array and a class both hand by
    /// header/reference already.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// A call to a generated codec — a curried `let` function, not a
    /// .NET method: [`fsharp::Expr::CurriedCall`]'s documentation
    /// carries the measurement that `f(a, b)` does not compile against
    /// one. `strcmp` is the runtime's, which spells C11 §7.24.4's
    /// unsigned-octet order over two UTF-8 strings, and is a free
    /// function of `runtime/fsharp/ZdWire.fs` too.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let arguments: Vec<fsharp::Expr> = args.into_iter().map(inner).collect();
        if callee == "strcmp" {
            return plain(fsharp::Expr::CurriedCall {
                func: Box::new(Self::runtime_name("strcmp")),
                args: arguments,
            });
        }
        plain(fsharp::Expr::curried_call(callee, arguments))
    }

    /// `w.WriteU32(x)` — a method on the writer or the reader.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        plain(fsharp::Expr::Call {
            func: Box::new(fsharp::Expr::dot(
                fsharp::Expr::Name(Ident::new(receiver)),
                &Self::pascal(method),
            )),
            args: args.into_iter().map(inner).collect(),
        })
    }

    /// A free function of the runtime, qualified — except
    /// `pid_is_user_member`, which is a property on the header the
    /// reader handed back.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if method == "pid_is_user_member" {
            let mut arguments = args.into_iter().map(inner);
            if let Some(header) = arguments.next() {
                return plain(fsharp::Expr::dot(header, "IsUserMember"));
            }
            return plain(fsharp::Expr::dot(
                Self::runtime_name("PidRead"),
                "IsUserMember",
            ));
        }
        // A free function of `ZdWire`, curried, exactly as
        // [`Self::call`] answers for a generated codec — the module
        // documentation of [`fsharp::Expr::CurriedCall`] carries why.
        let call = fsharp::Expr::CurriedCall {
            func: Box::new(Self::runtime_name(&Self::camel(method))),
            args: args.into_iter().map(inner).collect(),
        };
        plain(call)
    }

    // -- statements ----------------------------------------------------

    /// `let name : T = value` — the type written, so a reader of the
    /// generated file sees the width. Every local the body reassigns is
    /// written `let mutable` by [`Self::routine`]'s assembly pass;
    /// [`Self::declare`] itself answers `false` because most locals the
    /// walk declares are never reassigned, and the pass corrects the
    /// minority that are.
    ///
    /// Two coercions the other columns need none of, both because F#
    /// converts no numeric type implicitly and admits no null in a
    /// record:
    ///
    /// - **A bare integer initialiser takes the slot's own width.** The
    ///   walk writes its zeros and ones through one `int` seam — C's
    ///   `uint32_t zd_before = 0` and C#'s `uint zd_before = 0` both
    ///   compile — and F#'s `let zd_before : uint32 = 0` is
    ///   `error FS0001`. [`crate::adapter::fsharp::typed_int`] gives the
    ///   literal its suffix instead.
    /// - **A slot with no initialiser gets the runtime's empty handle.**
    ///   `crates/lang-rules/src/emit/decode.rs` declares `zd_p` and
    ///   `zd_h` with no value, meaning "a header the loop is about to
    ///   read into"; C leaves the struct uninitialised and F# has no
    ///   such state, so the runtime's `noPidRead ()`/`noEmheaderRead ()`
    ///   stand in — the same answer [`Handle::DHeader`] already gets
    ///   from `ZdWire.noDheader ()` through the walk's own seam.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        let value = match value {
            Some(value) => Self::widen_literal(&type_.tree, inner(value)),
            None => Self::empty_value(&type_.tree),
        };
        fsharp::Stmt::Let {
            name: Ident::new(name),
            type_: Some(type_.tree),
            value,
            mutable: false,
        }
    }

    fn perform(value: Self::Expr) -> Self::Stmt {
        match value {
            FsharpExpr::Plain(fsharp::Expr::Binary { op: "=", lhs, rhs }) => fsharp::Stmt::Assign {
                place: *lhs,
                value: *rhs,
            },
            other => fsharp::Stmt::Expr(inner(other)),
        }
    }

    /// The body's trailing expression: an F# decoder's last line is the
    /// sample it built, with no `return` keyword to write.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        fsharp::Stmt::Return(match value {
            Some(value) => inner(value),
            None => fsharp::Expr::Name(Ident::new("v")),
        })
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        fsharp::Stmt::If {
            condition: inner(condition),
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// `for i = 0 to (int bound) - 1 do …` — F#'s own counted loop,
    /// scoped by the language; the module documentation of
    /// `crates/lang-cst/src/fsharp/mod.rs` says why this needs none of
    /// Zig's block-wrapping trick. The counter is `int32` and not
    /// `uint32` — unlike a `let`-bound local, F#'s `for … = … to … do`
    /// syntax takes only `int`: measured (`fsi`, .NET SDK 9.0.106),
    /// `for i = 0u to n - 1u do` is `error FS0001`, "This expression was
    /// expected to have type 'int' … 'uint32'". `bound` is cast to
    /// `int` because [`Self::count_of`]'s own answer is `uint32`, the
    /// wire's own width for a count.
    ///
    /// The walk nevertheless reads a counter as a **count**, at the
    /// width [`Scalar::Index`] gives: `crates/lang-rules/src/emit/
    /// encode.rs`'s `ascending_by_key` declares `zd_before` at
    /// [`Scalar::UInt32`] and compares it against the enclosing
    /// counter, which C# gets away with because C# promotes both to
    /// `long` and F# does not — measured, `if zd_before = zd_rank then`
    /// with an `int` counter is `error FS0001`. So the loop counts in
    /// `int`, as the syntax requires, and the body opens with
    /// `let i = uint32 i`: F# admits the shadow inside the loop's own
    /// scope (measured, `fsi`, .NET SDK 9.0.106) and every use behind it
    /// then sees the width the walk assumed. An index into a .NET array
    /// crosses `Self::element_at`'s own `int` conversion either way.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        let mut inside = Vec::with_capacity(body.len() + 1);
        inside.push(fsharp::Stmt::Let {
            name: Ident::new(counter),
            type_: Some(fsharp::Type::Primitive(fsharp::Primitive::UInt32)),
            value: fsharp::Expr::call("uint32", vec![fsharp::Expr::Name(Ident::new(counter))]),
            mutable: false,
        });
        inside.extend(body);
        fsharp::Stmt::CountUp {
            counter: Ident::new(counter),
            type_: Box::new(fsharp::Type::Primitive(fsharp::Primitive::Int32)),
            bound: Box::new(fsharp::Expr::call("int", vec![inner(bound)])),
            body: inside,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        fsharp::Stmt::Until {
            condition: inner(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        fsharp::Stmt::Leave
    }

    fn note(doc: Doc) -> Self::Stmt {
        fsharp::Stmt::Comment(doc)
    }

    /// `value |> ignore` — F#'s spelling of a value read for its effect
    /// and not kept.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        fsharp::Stmt::Expr(fsharp::Expr::Call {
            func: Box::new(fsharp::Expr::Name(Ident::new("ignore"))),
            args: vec![inner(value)],
        })
    }

    /// No statement: F# diagnoses neither an unused parameter nor an
    /// unused local (measured, `fsc` 9.0.106,
    /// `--warnaserror+ --warn:5`), so there is nothing here to say.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// `place <- zd_dec_T r behaviour` — through `Some` where the place
    /// is an `@optional` member's slot: the decoder answers a bare `T`
    /// and F#'s `'T option` needs it wrapped. Curried, as every call to
    /// a generated codec is — [`fsharp::Expr::CurriedCall`]'s
    /// documentation carries why.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let decoded = fsharp::Expr::curried_call(
            callee,
            vec![fsharp::Expr::Name(Ident::new("r")), inner(behaviour)],
        );
        match place {
            FsharpExpr::OptionalSlot(field) => fsharp::Stmt::Assign {
                place: field,
                value: fsharp::Expr::Some_(Box::new(decoded)),
            },
            FsharpExpr::Plain(field) => fsharp::Stmt::Assign {
                place: field,
                value: decoded,
            },
        }
    }

    /// `v <- zd_dec_T(r, zd_tc);` — a `typedef`, handed straight on.
    ///
    /// The trait's default is C's out-parameter shape; this column's
    /// decoders answer their value, exactly as Zig's, C#'s, D's, Go's,
    /// Java's, Python's and TypeScript's do, for each backend's own
    /// reason.
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

    /// `v <- Array.zeroCreate<T> (int count)`, and the fill after it.
    /// `Array.zeroCreate` answers an array of `T`'s own zero value —
    /// `null` for a class-typed `T`, which the fill then overwrites
    /// slot by slot before anything reads it, exactly as C's
    /// allocate-then-fill does.
    /// `Array.zeroCreate<T> n`, and — where the elements are written
    /// into rather than assigned whole — one constructed instance per
    /// slot.
    ///
    /// `Array.zeroCreate` gives every slot `Unchecked.defaultof<T>`,
    /// which for the classes every aggregate becomes is `null`. A
    /// decoder that then writes `v.[i].Key <- …` dereferences it:
    /// measured on `matrix/cases/map-key-long`, whose three entries came
    /// back as `System.NullReferenceException` and cost the read
    /// direction of the cell. The C# column's `make_room` fills the
    /// slots for the same reason, and `assigned_whole` is the walk's own
    /// answer for when it need not — a primitive element is assigned as
    /// a whole and has no null to dereference.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![fsharp::Stmt::Assign {
            place: fsharp::Expr::Name(Ident::new(base)),
            value: fsharp::Expr::ArrayZeroCreate {
                element: Box::new(element.tree.clone()),
                count: Box::new(fsharp::Expr::call("int", vec![inner(count)])),
            },
        }];
        if !assigned_whole {
            if let fsharp::Type::Named(name) = &element.tree {
                out.push(Self::count_up(
                    "zd_e",
                    Self::count_of(base, None),
                    vec![fsharp::Stmt::Assign {
                        place: fsharp::Expr::Index {
                            value: Box::new(fsharp::Expr::Name(Ident::new(base))),
                            // `int zd_e`: [`Self::count_up`] gives its
                            // counter the width the walk reads a count
                            // at, and a .NET array indexes by `int`.
                            index: Box::new(fsharp::Expr::call(
                                "int",
                                vec![fsharp::Expr::Name(Ident::new("zd_e"))],
                            )),
                        },
                        value: fsharp::Expr::New {
                            type_: name.clone(),
                            args: Vec::new(),
                        },
                    }],
                ));
            }
        }
        out.extend(fill);
        out
    }

    /// `v <- Array.sub v 0 (int count)` — §7.3.1.2.1.12.3's TRIM,
    /// applied after every element has been read. `Array.sub` is
    /// curried, like every F# standard-library array function.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![fsharp::Stmt::Assign {
            place: fsharp::Expr::Name(Ident::new(base)),
            value: fsharp::Expr::curried_call(
                "Array.sub",
                vec![
                    fsharp::Expr::Name(Ident::new(base)),
                    fsharp::Expr::int(0),
                    fsharp::Expr::call("int", vec![inner(count)]),
                ],
            ),
        }]
    }

    /// The subscript, with nothing around it: an F# array element is a
    /// real settable place at every element type.
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

    /// `let zd_enc_T (w: ZdWire.Writer) (v: T) : unit = …` and
    /// `let zd_dec_T (r: ZdWire.Reader) (zd_tc: ZdWire.TryConstruct) : T
    /// = …`.
    ///
    /// A forward declaration is answered with nothing: `module rec`
    /// gives every `let` in the file mutual visibility regardless of
    /// where it is written, so a recursive type needs no declaration
    /// ahead of its body — the module documentation of
    /// `crates/lang-cst/src/fsharp/mod.rs` carries the measurement.
    ///
    /// Unlike [`crate::emit::zig::Zig::routine`], this does not scan the
    /// body for mutability or unused names: F# diagnoses neither, so
    /// every local the walk declares is a plain `let` — a reassignment
    /// [`Self::perform`] built as an [`fsharp::Stmt::Assign`] against a
    /// `let`-bound name is exactly the case the assembly pass below
    /// corrects, by finding every name a body assigns through and
    /// widening its declaration to `let mutable`.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![fsharp::FnParam {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            type_: fsharp::Type::named(if spec.writing {
                "ZdWire.Writer"
            } else {
                "ZdWire.Reader"
            }),
        }];
        if spec.writing {
            params.push(fsharp::FnParam {
                name: Ident::new("v"),
                type_: spec.sample.tree.clone(),
            });
        }
        for (type_, name) in spec.extra {
            params.push(fsharp::FnParam {
                name: Ident::new(&name),
                type_: type_.tree,
            });
        }
        let (returns, body) = if spec.writing {
            (fsharp::Type::Primitive(fsharp::Primitive::Unit), body)
        } else {
            // The sample starts as Table 9's value, through the
            // generated defaulter's call — `Routine` carries no `Ast`
            // and no `Id<TypeNode>`, only the already-materialised
            // type, so the callee is named from the decoder's own
            // name, exactly as `crate::emit::zig::Zig::routine` derives
            // its own defaulter call. `default_fsharp`'s module
            // documentation carries why.
            let defaulter = default_fsharp::defaulter_from_decoder(&spec.name);
            let mut statements = vec![fsharp::Stmt::Let {
                name: Ident::new("v"),
                type_: Some(spec.sample.tree.clone()),
                value: fsharp::Expr::call(&defaulter, Vec::new()),
                mutable: true,
            }];
            // A `return` the shared walk wrote inside a loop body is the
            // one shape `hoist_early_returns` cannot reach: pushing the
            // routine's tail into a loop body would run the tail once
            // per iteration. Those bodies take the exception instead —
            // every `Return` in them becomes `raise (ZdWire.GiveBack())`
            // and the whole body goes inside one `try`, with `v` as the
            // trailing expression after it. Only those: a `try` around
            // a body that never raises would be noise in every other
            // decoder in the file, and the hoisting pass is exact for
            // every other shape.
            if returns_inside_a_loop(&body) {
                let mut guarded = body;
                give_back_instead_of_returning(&mut guarded);
                statements.push(fsharp::Stmt::Guarded { body: guarded });
            } else {
                statements.extend(body);
            }
            statements.push(fsharp::Stmt::Return(fsharp::Expr::Name(Ident::new("v"))));
            (spec.sample.tree, statements)
        };
        let body = hoist_early_returns(body);
        let mut body = body;
        let assigned = assigned_roots(&body);
        fix_mutability(&mut body, &assigned);
        Some(fsharp::Decl::Let(fsharp::LetDecl {
            doc: spec.doc,
            name: Ident::new(&spec.name),
            params,
            type_: Some(returns),
            body,
        }))
    }

    /// `place <- zd_def_T ()` — the generated defaulter, not the value
    /// inlined, so a member the stream skipped and one this same
    /// decoder's `v` already started as (`Self::routine`'s own call to
    /// the same function) always agree, by construction, on what
    /// Table 9's value is.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        let value = fsharp::Expr::call(&default_fsharp::defaulter_name(ast, id), Vec::new());
        Ok(match place {
            FsharpExpr::OptionalSlot(field) => fsharp::Stmt::Assign {
                place: field,
                value: fsharp::Expr::Some_(Box::new(value)),
            },
            FsharpExpr::Plain(field) => fsharp::Stmt::Assign {
                place: field,
                value,
            },
        })
    }

    /// # Errors
    /// As [`default_fsharp::member_value`]: a `@default` this column
    /// cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        let value = default_fsharp::member_value(ast, node, type_)?;
        Ok(match place {
            FsharpExpr::OptionalSlot(field) => vec![fsharp::Stmt::Assign {
                place: field,
                value: fsharp::Expr::Some_(Box::new(value)),
            }],
            FsharpExpr::Plain(field) => vec![fsharp::Stmt::Assign {
                place: field,
                value,
            }],
        })
    }

    /// `Mode.IDLE` — the enum's own scope, which is where the
    /// materialiser declared the case.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        plain(fsharp::Expr::dot(
            fsharp::Expr::Name(Ident::new(&fsharp_type_name(ast, id))),
            &field_name(enumerator),
        ))
    }
}

// -- the assembly pass ---------------------------------------------------

/// F# has no early `return`: a function's value is its body's last
/// expression, and nothing before that point can hand the function's
/// answer back and stop. The shared codec walk's decoders do exactly
/// that — an enumeration's decoder, for one, builds a single
/// `if`/`elif`/…/`else` covering every enumerator *and* every
/// §7.2.2.7 try-construct fallback, where some arms end with
/// `v <- <the decoded value>` (falling through to a trailing
/// `give_back`) and others end with a mid-body
/// [`crate::emit::CodecTarget::give_back`] of their own — a
/// [`fsharp::Stmt::Return`] that is not the whole chain's last
/// statement, with a plain `v` still to come after the `if`/`elif`
/// chain closes.
///
/// This pass turns that into what F# can run: [`splice_tail`] pushes
/// whatever textually followed a `Return`-containing `if` down into
/// every one of its branches that does not already end in one,
/// recursively through an `elif` chain's nested `otherwise`s and
/// through loop bodies, until nothing in the function is left dangling
/// after a statement that was supposed to have ended it.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn hoist_early_returns(body: Vec<fsharp::Stmt>) -> Vec<fsharp::Stmt> {
    // A right fold: the tail spliced into statement `i` is everything
    // that followed it, already folded — so a chain of several
    // early-return `if`s each gets exactly what came after *it*, not
    // what came after the first one.
    let mut acc: Vec<fsharp::Stmt> = Vec::new();
    for stmt in body.into_iter().rev() {
        let stmt = recurse_into_nested(stmt);
        // A statement with no `Return` anywhere inside it neither ends
        // the function nor leaves anything dangling after itself, so
        // the tail belongs after it and nowhere else. Handing it to
        // `splice_tail` would reach the same arrangement by copying the
        // whole tail, and this pass runs once per statement, so the
        // copy is what turns a decoder's body quadratic in its own
        // length for no gain.
        if contains_return(core::slice::from_ref(&stmt)) {
            acc = splice_tail(vec![stmt], &acc);
        } else {
            acc.insert(0, stmt);
        }
    }
    acc
}

/// Applies [`hoist_early_returns`] inside a statement's own nested
/// blocks — an `if`'s branches, a loop's body — without touching the
/// statement's place in whatever list holds it.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn recurse_into_nested(stmt: fsharp::Stmt) -> fsharp::Stmt {
    match stmt {
        fsharp::Stmt::If {
            condition,
            then,
            otherwise,
        } => fsharp::Stmt::If {
            condition,
            then: hoist_early_returns(then),
            otherwise: otherwise.map(hoist_early_returns),
        },
        fsharp::Stmt::CountUp {
            counter,
            type_,
            bound,
            body,
        } => fsharp::Stmt::CountUp {
            counter,
            type_,
            bound,
            body: hoist_early_returns(body),
        },
        fsharp::Stmt::Until { condition, body } => fsharp::Stmt::Until {
            condition,
            body: hoist_early_returns(body),
        },
        other => other,
    }
}

/// Appends `tail` to `body`, unless `body` already
/// [`always_returns`] — in which case `tail` is unreachable and is
/// dropped — pushing `tail` down into an unterminated `if`'s branches
/// instead of after it when `body`'s last statement is one.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn splice_tail(mut body: Vec<fsharp::Stmt>, tail: &[fsharp::Stmt]) -> Vec<fsharp::Stmt> {
    if tail.is_empty() || always_returns(&body) {
        return body;
    }
    match body.pop() {
        Some(fsharp::Stmt::If {
            condition,
            then,
            otherwise,
        }) => {
            // Only an `if` that can hand the function its answer needs
            // the tail pushed inside it. An `if` with no `Return` on any
            // path is a plain unit-valued statement that falls through,
            // and `if c then A else B` followed by `T` runs exactly what
            // `if c then A; T else B; T` runs — but the second spelling
            // writes `T` twice, and doing that once per statement is a
            // body that doubles per `if`.
            //
            // The decoders that meet this are every unannotated
            // aggregate's: DDS-XTypes 1.3 §7.3.1.2.1.8 makes an
            // aggregate with no extensibility annotation APPENDABLE, and
            // an APPENDABLE member is read under a guard on whether the
            // stream still carries it, so the walk emits several such
            // fall-through `if`s per member and the copies compound
            // across all of them.
            let returns_inside = contains_return(&then)
                || otherwise
                    .as_ref()
                    .is_some_and(|otherwise| contains_return(otherwise));
            if !returns_inside {
                body.push(fsharp::Stmt::If {
                    condition,
                    then,
                    otherwise,
                });
                body.extend_from_slice(tail);
                return body;
            }
            let then = splice_tail(then, tail);
            let otherwise = splice_tail(otherwise.unwrap_or_default(), tail);
            body.push(fsharp::Stmt::If {
                condition,
                then,
                otherwise: Some(otherwise),
            });
            body
        }
        Some(last) => {
            body.push(last);
            body.extend_from_slice(tail);
            body
        }
        None => tail.to_vec(),
    }
}

/// Whether any path through `body` reaches a [`fsharp::Stmt::Return`]
/// at all, however deeply nested.
///
/// The weaker sibling of [`always_returns`], and the question
/// [`splice_tail`] has to ask before it copies: a statement that cannot
/// return needs nothing pushed into it, because F# runs whatever
/// follows it either way. A loop body counts, because a `Return` there
/// is just as much the function's answer as one at the top level and
/// F# has no more of a way to express it.
///
/// zerodds-lint: recursion-depth 64 (statement nesting, which the walk
/// adds one level of per type level and so bounds by `MAX_TYPE_DEPTH`)
fn contains_return(body: &[fsharp::Stmt]) -> bool {
    body.iter().any(|statement| match statement {
        fsharp::Stmt::Return(_) => true,
        fsharp::Stmt::If {
            then, otherwise, ..
        } => {
            contains_return(then)
                || otherwise
                    .as_ref()
                    .is_some_and(|otherwise| contains_return(otherwise))
        }
        fsharp::Stmt::CountUp { body, .. }
        | fsharp::Stmt::Until { body, .. }
        | fsharp::Stmt::Guarded { body } => contains_return(body),
        fsharp::Stmt::Let { .. }
        | fsharp::Stmt::Assign { .. }
        | fsharp::Stmt::Expr(_)
        | fsharp::Stmt::Leave
        // Already the raise, so no longer a `Return` for this pass to
        // find; `routine` rewrites the ones it cannot hoist and does it
        // after `hoist_early_returns` has run.
        | fsharp::Stmt::GiveBack
        | fsharp::Stmt::Comment(_) => false,
    })
}

/// Whether any [`fsharp::Stmt::Return`] in `body` sits inside a loop.
///
/// The question [`Fsharp::routine`] asks before it reaches for
/// [`fsharp::Stmt::Guarded`]. [`hoist_early_returns`] answers every
/// other shape by pushing what followed a return down into the branches
/// that could reach it; a loop body is where that stops working, because
/// what followed the loop must run once and not once per iteration.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height.
fn returns_inside_a_loop(body: &[fsharp::Stmt]) -> bool {
    body.iter().any(|statement| match statement {
        fsharp::Stmt::CountUp { body, .. } | fsharp::Stmt::Until { body, .. } => {
            contains_return(body)
        }
        fsharp::Stmt::If {
            then, otherwise, ..
        } => {
            returns_inside_a_loop(then)
                || otherwise
                    .as_ref()
                    .is_some_and(|otherwise| returns_inside_a_loop(otherwise))
        }
        fsharp::Stmt::Guarded { body } => returns_inside_a_loop(body),
        _ => false,
    })
}

/// Turns every [`fsharp::Stmt::Return`] in `body` into
/// [`fsharp::Stmt::GiveBack`].
///
/// All of them and not only the ones inside a loop: the guard is around
/// the whole body, so once it exists the cheapest correct rule is that
/// nothing inside it is a trailing expression. Every one of the shared
/// walk's thirteen early returns is `give_back(None)`, which
/// [`Fsharp::give_back`] spells `v` — the same value
/// [`Fsharp::routine`] writes after the guard — so the routine answers
/// what it answered before.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height.
fn give_back_instead_of_returning(body: &mut [fsharp::Stmt]) {
    for statement in body.iter_mut() {
        match statement {
            fsharp::Stmt::Return(_) => *statement = fsharp::Stmt::GiveBack,
            fsharp::Stmt::If {
                then, otherwise, ..
            } => {
                give_back_instead_of_returning(then);
                if let Some(otherwise) = otherwise {
                    give_back_instead_of_returning(otherwise);
                }
            }
            fsharp::Stmt::CountUp { body, .. }
            | fsharp::Stmt::Until { body, .. }
            | fsharp::Stmt::Guarded { body } => give_back_instead_of_returning(body),
            _ => {}
        }
    }
}

/// Whether `body` is certain, on every path through it, to end in a
/// [`fsharp::Stmt::Return`] — so that anything spliced after it would
/// be unreachable.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn always_returns(body: &[fsharp::Stmt]) -> bool {
    match body.last() {
        Some(fsharp::Stmt::Return(_)) => true,
        Some(fsharp::Stmt::If {
            then,
            otherwise: Some(otherwise),
            ..
        }) => always_returns(then) && always_returns(otherwise),
        _ => false,
    }
}

/// Every base name the body assigns through — `v` for `v.x <- …` and
/// `v.[i] <- …` alike — which is what decides `let mutable`.
fn assigned_roots(body: &[fsharp::Stmt]) -> Vec<String> {
    let mut out = Vec::new();
    collect_assigned(body, &mut out);
    out
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn collect_assigned(body: &[fsharp::Stmt], out: &mut Vec<String>) {
    for statement in body {
        match statement {
            fsharp::Stmt::Assign { place, .. } => {
                if let Some(root) = root_name(place) {
                    if !out.contains(&root) {
                        out.push(root);
                    }
                }
            }
            fsharp::Stmt::If {
                then, otherwise, ..
            } => {
                collect_assigned(then, out);
                if let Some(otherwise) = otherwise {
                    collect_assigned(otherwise, out);
                }
            }
            fsharp::Stmt::CountUp { body, .. }
            | fsharp::Stmt::Until { body, .. }
            | fsharp::Stmt::Guarded { body } => {
                collect_assigned(body, out);
            }
            _ => {}
        }
    }
}

/// The base a place is reached from: `v` for `v.x.[i]`.
///
/// zerodds-lint: recursion-depth 64 (place-expression nesting the walk
/// builds). The tree walked here is the one this generator emitted and
/// owns, so it holds no cycle; the depth is that expression's height.
fn root_name(place: &fsharp::Expr) -> Option<String> {
    match place {
        fsharp::Expr::Name(name) => Some(name.as_str().to_string()),
        fsharp::Expr::Dot { value, .. }
        | fsharp::Expr::Index { value, .. }
        | fsharp::Expr::Paren(value) => root_name(value),
        _ => None,
    }
}

/// Turn every `let` the body assigns through into `let mutable`: F#
/// refuses `<-` against a binding that is not one.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn fix_mutability(body: &mut [fsharp::Stmt], assigned: &[String]) {
    for statement in body {
        match statement {
            fsharp::Stmt::Let { name, mutable, .. } => {
                if assigned.iter().any(|root| root == name.as_str()) {
                    *mutable = true;
                }
            }
            fsharp::Stmt::If {
                then, otherwise, ..
            } => {
                fix_mutability(then, assigned);
                if let Some(otherwise) = otherwise {
                    fix_mutability(otherwise, assigned);
                }
            }
            fsharp::Stmt::CountUp { body, .. }
            | fsharp::Stmt::Until { body, .. }
            | fsharp::Stmt::Guarded { body } => {
                fix_mutability(body, assigned);
            }
            _ => {}
        }
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
/// generated F# held no `zd_over` check at all, so `a` accepted a
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
/// and not the F# spelling, structurally for a collection and the
/// flattened F# name for a declaration — the same scheme
/// [`crate::emit::zig::type_tag`] carries, for the reasons its own
/// documentation states.
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
        | TypeKind::Native => fsharp_type_name(ast, id),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}

/// The module the generator's `Types.fs` declares, named identically
/// with `crates/lang-rules/src/adapter/fsharp.rs`'s own [`MODULE_NAME`]
/// so a generated `main.fsx` opens the one the adapter actually wrote.
#[must_use]
pub fn module_name() -> &'static str {
    MODULE_NAME
}
