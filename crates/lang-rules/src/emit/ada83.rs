// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada 83 backend of the shared codec walk.
//!
//! `emit::ada` is the closer relative than any other backend in this
//! crate, and the five decisions its own module documentation lists —
//! a discriminant is not assignable, an `@optional` member is a
//! discriminated record too, a sequence is an access to an
//! unconstrained array, every numeric literal is universal — are RM83's
//! exactly as much as they are Ada 2012's, and this file answers all
//! five the identical way. What differs follows from
//! `zerodds_lang_rules::adapter::ada83`'s materialisation:
//!
//! - **No enumeration type.** An enumerator is a named `Long_Integer`
//!   constant, so [`Ada83::cast`] has no `'Enum_Val`/`'Enum_Rep` arm to
//!   write at all — a decoded wire code and a member's value are the
//!   same `Long_Integer`, with nothing to convert.
//! - **No `Interfaces`, one integer type.** [`AdaType`] carries no
//!   width; every scalar this backend names is `Long_Integer`,
//!   `Boolean`, `Float` or `Long_Float`. A position in the stream is
//!   the exception and is `Natural`, because that is what
//!   `runtime/ada-83`'s `Reader_Position` and `Reader_Remaining` answer
//!   in and RM83 3.5.4 admits no implicit conversion between `Integer`
//!   and `Long_Integer`. [`Ada83::binary`] still has to answer
//!   `>>`/`<<` — the walk still asks for them — and does, in RM83 4.5's
//!   `/`, `*` and `**` rather than `Interfaces.Shift_Right`.
//! - **`string`/`wstring`/`fixed`/`long double` are `Zd_Wire83.Text`,**
//!   an access type, not a private `Unbounded_String`; there is
//!   nothing to allocate through beyond what a sequence already needs.
//! - **No unsigned order either.** An `unsigned long long` is held in
//!   the same signed `Long_Integer`, so the predefined `<` is the wrong
//!   order for a `map` key above 2**63 and
//!   [`Ada83::unsigned_less`](CodecTarget::unsigned_less) routes to
//!   `Zd_Wire83.U64_Less`. `emit::ada` needs nothing here because Ada 95
//!   gave it `Interfaces.Unsigned_64`.
//! - **The Cyclone union overlay's binary64 pattern is a single
//!   `Long_Integer`, signed** — see `runtime/ada-83`'s
//!   `Bit_Pattern_F64_Overlay`: a pattern needing bit 63 is the
//!   negative `Long_Integer` holding those bits, the same carriage
//!   `Write_U64` gives an `unsigned long long` above 2**63 - 1.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::ada83::{ada83_map_entry_name, component_name};
use crate::default_common::resolve_alias;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::ada83;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

/// The Ada 83 backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ada83;

/// The package the generated code names the runtime under.
const RUNTIME: &str = "Zd_Wire83";

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`, taking the same run through `runtime/ada-83`'s `As_Signed`
/// that `emit::ada` takes through `runtime/ada`'s.
const SIGNED_CHAR: &str = "signed char";

/// The subtype [`Ada83::routine`] declares for a decoder's own sample
/// type, so the discriminant assignment can name it.
const OWN_SAMPLE: &str = "Zd83_Sample";

/// The subtype [`Ada83::routine`] declares for the array behind a
/// collection decoder's own access type.
const OWN_ARRAY: &str = "Zd83_Own_Array";

/// The runtime operations the walk names on a reader or a writer that
/// `runtime/ada-83` declares without one.
///
/// The shared walk spells every runtime operation as a method on the
/// stream — `w.bitfield(…)`, `r.trim_utf8(…)` — because `runtime/ada`
/// declares all three with the stream as their first parameter.
/// `runtime/ada-83` does not: its `Bitfield` is arithmetic on a value
/// (`Bitfield (Value, Width, Position)`) and its two trims are
/// arithmetic on a `Text` (`Trim_Utf8 (Value, Octets)`) — none of the
/// three reads or moves the stream, and RM83 6.1 gives a function no
/// `in out` parameter to move it through anyway. Passing the receiver
/// is what GNAT reports as "too many arguments in call to
/// Zd_Wire83.Bitfield". This is the same seam [`Ada83::narrow_integer`]
/// stands on: where the two runtimes' signatures differ, the difference
/// is answered here rather than by widening `runtime/ada-83` with a
/// parameter it has no use for.
const RECEIVERLESS: &[&str] = &["bitfield", "trim_utf8", "trim_utf16"];

/// A type, as this column holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaType {
    /// The subtype mark, dotted.
    pub mark: String,
    /// Whether it is `Boolean` — an integer-literal cast becomes the
    /// Boolean literal.
    pub is_bool: bool,
    /// Whether it resolves to a union — the decoder needs
    /// `OWN_SAMPLE`.
    pub is_union: bool,
    /// The `…_Array` type behind it, where it is a sequence or a map —
    /// the decoder needs `OWN_ARRAY`.
    pub own_array: Option<String>,
    /// Whether the walk asked for this type to hold a *count* — an
    /// element index, a rank, a length — rather than a wire value.
    ///
    /// Both are `Long_Integer` here, and [`Ada83::declare`] moves a
    /// literal-initialised count into `Natural` so it can be compared
    /// against a `for` loop parameter, which RM83 3.6 types `Integer`.
    /// A wire value must not take that move: the walk's bitset
    /// accumulator starts at `0` too and has to stay `Long_Integer`,
    /// because a bitset anchor is up to sixty-four bits wide and
    /// `Natural` is thirty-one.
    pub counts: bool,
}

impl AdaType {
    fn bare(mark: &str) -> Self {
        Self {
            mark: mark.to_string(),
            is_bool: false,
            is_union: false,
            own_array: None,
            counts: false,
        }
    }
}

/// An expression, with the two properties the walk's seams cannot
/// carry — identical in shape to `emit::ada`'s `AExpr`, for the
/// identical reason: an `@optional` wrapper and a union discriminant
/// are both places an ordinary assignment cannot reach directly.
#[derive(Debug, Clone)]
pub enum AExpr {
    /// An ordinary expression.
    Plain(ada83::Expr),
    /// The `Zd_Opt_…` field of an `@optional` member.
    OptionalSlot(ada83::Expr),
    /// `v.D`, assignable only through the whole record (RM83 3.7.1).
    Discriminant(ada83::Expr),
    /// An enumeration literal of a type `Zd_Wire83` declares — a
    /// `Status`, a `Try_Construct`, a `Parameter_Role`, a
    /// `Wstring_Policy`, a `Fixed_Count`, a `Wchar_Framing` or a
    /// `Unit_Order`.
    ///
    /// Carried apart from [`AExpr::Plain`] so that [`Ada83::binary`]
    /// can tell a comparison on such a type from one on a `Standard`
    /// type. RM83 8.3 makes an operator directly visible only inside
    /// the package that declares its type or under a `use` clause, and
    /// Ada 83 has no `use type` to make just the operator visible.
    /// `Zd_Wire83.Reader_Status (r) = Zd_Wire83.Status_Ok` therefore
    /// does not compile — GNAT answers "operator for type Status is not
    /// directly visible" — while the same comparison on `Long_Integer`
    /// does, because `Standard` is visible everywhere.
    RuntimeEnum(ada83::Expr),
    /// The access-typed field of a member held by reference
    /// ([`Ada83::holds_by_reference`]), as a place a decoded value is
    /// about to be allocated for; `mark` is the designated subtype the
    /// allocator names.
    ReferenceSlot {
        /// The field.
        place: ada83::Expr,
        /// `T` in `new T'(…)`.
        mark: Path,
    },
    /// An assignment the walk built through `binary("=", …)`.
    Assignment {
        /// The place.
        place: Box<AExpr>,
        /// The value.
        value: ada83::Expr,
    },
}

/// zerodds-lint: recursion-depth 64 (an assignment's place, which is
/// itself an `AExpr`). The value walked is owned and moved in, so it
/// holds no cycle; the walk builds no `AExpr::Assignment` inside
/// another, so the real depth is one.
fn inner(expr: AExpr) -> ada83::Expr {
    match expr {
        AExpr::Plain(e)
        | AExpr::OptionalSlot(e)
        | AExpr::Discriminant(e)
        | AExpr::RuntimeEnum(e) => e,
        AExpr::ReferenceSlot { place, .. } => place,
        AExpr::Assignment { place, value } => ada83::Expr::Binary {
            op: "=",
            lhs: Box::new(inner(*place)),
            rhs: Box::new(value),
        },
    }
}

fn plain(e: ada83::Expr) -> AExpr {
    AExpr::Plain(e)
}

/// A 64-bit literal as the `Long_Integer` whose bits it is.
///
/// `zerodds_lang_rules::adapter::ada83` holds every integer this column
/// declares in `Long_Integer`, which is signed, so a wire value at or
/// above 2**63 has no literal of its own — an `unsigned long long`
/// union label, a `@bit_bound(64)` bitmask's declared set, a bitfield
/// mask sixty-four bits wide are all in that half. Each is written as
/// the negative number whose two's-complement bits it is, which is the
/// carriage `runtime/ada-83` already documents for `Write_U64` and
/// renders back unsigned in `Json_U64`.
///
/// Written as the plain decimal instead, GNAT answers "value not in
/// range of type Standard.Long_Integer" and — because a variant part's
/// choice and a discriminant's value must both be static, RM83 3.7.3
/// and 4.3.1 — "choice given in variant part is not static" and
/// "expression raises exception, cannot be static (RM 4.9(34))".
/// `adapter::ada83::choice_expr` writes the same fold for the
/// declaration side and `wireprobe-zerodds-ada83-gen`'s filler for the
/// sample.
fn pattern_literal(value: i128) -> ada83::Expr {
    /// 2**64, the whole width, as the fold subtracts it.
    const WIDTH: i128 = 1 << 64;
    if value > i128::from(i64::MAX) && value <= i128::from(u64::MAX) {
        return ada83::Expr::int(value - WIDTH);
    }
    ada83::Expr::int(value)
}

/// The `runtime/ada-83` operation the walk's bit operator is, or `None`
/// where the operator is an arithmetic or a relational one Ada already
/// has.
///
/// RM83 4.5.1 declares `and`, `or`, `xor` and `not` for `Boolean` and
/// for a one-dimensional array of `Boolean`, and for nothing else; RM83
/// 4.5 has no shift operator at all. Ada 95's modular types and Annex
/// B's `Interfaces.Shift_Right` are what later Ada answers this with,
/// and this column has neither — so `Zd_Wire83` writes the five in
/// `+`, `*`, `/` and `mod` and the generated code calls them.
///
/// `>>` used to be written here as `lhs / 2 ** distance` and `<<` as
/// `lhs * 2 ** distance`. Both are right for a non-negative `lhs` and
/// wrong for the value this column actually shifts: a bitset anchor is
/// the 64-bit *pattern*, negative once bit 63 is set, and RM83 4.5.5's
/// `/` truncates toward zero while a shift does not, and the `*` of a
/// field that lands on bit 63 is past `Long_Integer'Last`.
const fn bitwise_binary(op: &str) -> Option<&'static str> {
    match op.as_bytes() {
        b"&" => Some("Bits_And"),
        b"|" => Some("Bits_Or"),
        b"^" => Some("Bits_Xor"),
        b">>" => Some("Shift_Right"),
        b"<<" => Some("Shift_Left"),
        _ => None,
    }
}

fn mark_path(mark: &str) -> Path {
    Path::new(&mark.split('.').collect::<Vec<&str>>())
}

impl Ada83 {
    fn runtime_path(name: &str) -> Path {
        Path::new(&[RUNTIME, name])
    }

    /// Every constant this backend names is an enumeration literal of a
    /// type `Zd_Wire83` declares; the two widths the walk asks for as
    /// constants are integers and go through [`ada83::Expr::int`]
    /// instead. See [`AExpr::RuntimeEnum`] for what the distinction
    /// buys.
    fn runtime_constant(name: &str) -> AExpr {
        AExpr::RuntimeEnum(ada83::Expr::Name(Self::runtime_path(name)))
    }

    fn operation(method: &str) -> String {
        ada83::ada_case(method)
    }

    fn selected(value: ada83::Expr, name: &str) -> ada83::Expr {
        ada83::Expr::Selected {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    fn call_path(path: Path, args: Vec<ada83::Expr>) -> ada83::Expr {
        ada83::Expr::Call { name: path, args }
    }

    /// The eight narrow integer widths, onto the two the runtime has.
    ///
    /// `runtime/ada` declares `Write_I8`, `Write_U8`, `Write_I16` and so
    /// on down to 64 bits, one subprogram per width, because Ada 2012's
    /// `Interfaces` gives it an `Integer_8` and an `Unsigned_16` to
    /// declare them with. Ada 83 has neither, so `runtime/ada-83` takes
    /// the width as a parameter and checks the value against the range
    /// the width admits — `Write_Int (W, Value, Width, Low, High)` and
    /// `Read_Int (R, Width)`. This is where the shared emitter's
    /// `write_i32` becomes that call; without it the generated program
    /// names a `Write_I32` that this runtime does not declare, which is
    /// what GNAT reported before anything else could be checked.
    ///
    /// 64 bits is not here: `Write_I64` and `Read_I64` exist under their
    /// own names, because a 64-bit value does not fit the `Low .. High`
    /// pair `Long_Integer` can state.
    fn narrow_integer(method: &str, arguments: &[ada83::Expr]) -> Option<ada83::Expr> {
        let (width, signed) = match method {
            "write_i8" | "read_i8" => (1, true),
            "write_u8" | "read_u8" => (1, false),
            "write_i16" | "read_i16" => (2, true),
            "write_u16" | "read_u16" => (2, false),
            "write_i32" | "read_i32" => (4, true),
            "write_u32" | "read_u32" => (4, false),
            _ => return None,
        };
        // The range the width admits, as the runtime's own comment puts
        // it: a `long`'s -2**31 differs from an `unsigned long`'s 0, and
        // only the caller knows which of the two it is holding.
        let bits = width * 8;
        let (low, high) = if signed {
            (-(1_i64 << (bits - 1)), (1_i64 << (bits - 1)) - 1)
        } else {
            (0, (1_i64 << bits) - 1)
        };
        let number = |value: i64| ada83::Expr::int(i128::from(value));

        if method.starts_with("read_") {
            let name = if signed { "Read_Int" } else { "Read_Uint" };
            let mut args = arguments.to_vec();
            args.push(number(width));
            return Some(Self::call_path(Self::runtime_path(name), args));
        }
        let mut args = arguments.to_vec();
        args.push(number(width));
        args.push(number(low));
        args.push(number(high));
        Some(Self::call_path(Self::runtime_path("Write_Int"), args))
    }

    /// `Natural (value)`.
    fn to_natural(value: ada83::Expr) -> ada83::Expr {
        ada83::Expr::Convert {
            mark: Path::single("Natural"),
            value: Box::new(value),
        }
    }

    /// `(Present => True, Value => decoded)`.
    fn engaged(value: ada83::Expr) -> ada83::Expr {
        ada83::Expr::Aggregate(vec![
            ada83::Association {
                choice: Ident::new(crate::adapter::ada83::PRESENT),
                value: ada83::Expr::Literal(Literal::Bool(true)),
            },
            ada83::Association {
                choice: Ident::new(crate::adapter::ada83::VALUE),
                value,
            },
        ])
    }

    fn discriminant_record(place: ada83::Expr) -> ada83::Expr {
        match place {
            ada83::Expr::Selected { value, .. } => *value,
            other => other,
        }
    }

    /// The whole-record assignment a discriminant change has to be
    /// (RM83 3.7.1).
    fn assign_discriminant(place: ada83::Expr, value: ada83::Expr) -> ada83::Stmt {
        ada83::Stmt::Declare {
            vars: vec![ada83::VarDecl {
                name: Ident::new("zd_nd"),
                mark: Path::single(OWN_SAMPLE),
                constraint: Some(value),
                value: None,
            }],
            body: vec![ada83::Stmt::Assign {
                place: Self::discriminant_record(place),
                value: ada83::Expr::name("zd_nd"),
            }],
        }
    }
}

impl CodecTarget for Ada83 {
    type Expr = AExpr;
    type Stmt = ada83::Stmt;
    type Block = Vec<ada83::Stmt>;
    type Type = AdaType;
    type Decl = ada83::Decl;

    // -- types --------------------------------------------------------

    /// One integer type for every integer-holding *value*: see the
    /// module documentation. `Index` and `Size` are not values but
    /// positions in the stream, and they are `Natural` — the type
    /// `runtime/ada-83`'s `Reader_Position` and `Reader_Remaining`
    /// answer in, exactly as `emit::ada`'s are. RM83 3.5.4 makes
    /// `Long_Integer` and `Integer` distinct types with no implicit
    /// conversion between them, so a position held in the other one
    /// cannot be compared against what the runtime returns: GNAT
    /// answers "invalid operand types for operator >, left operand has
    /// type Standard.Long_Integer, right operand has type
    /// Standard.Natural". The walk already converts at every seam where
    /// a count crosses into a position, because Ada 2012's
    /// `Interfaces.Unsigned_32` and `Natural` are just as distinct.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => AdaType {
                is_bool: true,
                ..AdaType::bare("Boolean")
            },
            Scalar::Float => AdaType::bare("Float"),
            Scalar::Double => AdaType::bare("Long_Float"),
            // The decimal text; the adapter's table says why.
            Scalar::LongDouble => AdaType::bare("Zd_Wire83.Text"),
            Scalar::Index | Scalar::Size => AdaType::bare("Natural"),
            // The width the walk counts elements in — a sequence
            // length is a `uint32` on the wire and the walk's own
            // element rank and element count are declared in it. See
            // [`AdaType::counts`] for what the mark buys.
            Scalar::UInt32 => AdaType {
                counts: true,
                ..AdaType::bare("Long_Integer")
            },
            _ => AdaType::bare("Long_Integer"),
        }
    }

    fn named(name: &str) -> Self::Type {
        AdaType::bare(name)
    }

    fn nothing() -> Self::Type {
        AdaType::bare("Zd83_No_Type")
    }

    fn handle(kind: Handle) -> Self::Type {
        AdaType::bare(match kind {
            Handle::DHeader => "Zd_Wire83.Dheader",
            Handle::EmHeader => "Zd_Wire83.Emheader",
            Handle::Pid => "Zd_Wire83.Pid_Write",
            Handle::TryConstruct => "Zd_Wire83.Try_Construct",
            Handle::ReaderMark => "Zd_Wire83.Reader_Mark",
            Handle::PidRead => "Zd_Wire83.Pid_Read",
            Handle::EmHeaderRead => "Zd_Wire83.Emheader_Read",
        })
    }

    /// Identity: the member and the runtime's parameter are both
    /// `Long_Integer` here, so there is no widening left to write —
    /// unlike `emit::ada`, whose member is a narrower `Interfaces`
    /// type than the runtime's `Unsigned_32`.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => plain(ada83::Expr::int(2)),
            Constant::WcharWidth4 => plain(ada83::Expr::int(4)),
            Constant::WcharFixed => Self::runtime_constant("Wchar_Fixed"),
            Constant::WcharGiopLengthPrefixed => {
                Self::runtime_constant("Wchar_Giop_Length_Prefixed")
            }
            Constant::WstringUnitsStream => Self::runtime_constant("Unit_Stream"),
            Constant::WstringUnitsBigEndianAlways => {
                Self::runtime_constant("Unit_Big_Endian_Always")
            }
            Constant::WstringUnitsByteOrderMark => Self::runtime_constant("Unit_Byte_Order_Mark"),
            Constant::WstringUnitsHostUnswapped => Self::runtime_constant("Unit_Host_Unswapped"),
            Constant::WstringSpecification => Self::runtime_constant("Wstr_Octets_Unterminated_2"),
            Constant::WstringCharacterCount => {
                Self::runtime_constant("Wstr_Characters_Unterminated_2")
            }
            Constant::WstringCharacterCountTerminated => {
                Self::runtime_constant("Wstr_Characters_Terminated_4")
            }
            Constant::WstringSpecificationWideUnits => {
                Self::runtime_constant("Wstr_Octets_Unterminated_4")
            }
            Constant::WstringRosTypeSupport => {
                Self::runtime_constant("Wstr_Characters_Unterminated_4")
            }
            Constant::WstringOctetsTerminated2 => {
                Self::runtime_constant("Wstr_Octets_Terminated_2")
            }
            Constant::WstringOctetsTerminated4 => {
                Self::runtime_constant("Wstr_Octets_Terminated_4")
            }
            Constant::WstringCharactersTerminated2 => {
                Self::runtime_constant("Wstr_Characters_Terminated_2")
            }
            Constant::FixedDeclared => Self::runtime_constant("Fixed_Declared"),
            Constant::FixedValuePrecision => Self::runtime_constant("Fixed_Value_Precision"),
            Constant::FixedValueStringDigits => Self::runtime_constant("Fixed_Value_String_Digits"),
            Constant::ParameterMember => Self::runtime_constant("Role_Member"),
            Constant::ParameterOptional => Self::runtime_constant("Role_Optional"),
            Constant::TryConstructDiscard => Self::runtime_constant("Tc_Discard"),
            Constant::TryConstructTrim => Self::runtime_constant("Tc_Trim"),
            Constant::TryConstructUseDefault => Self::runtime_constant("Tc_Use_Default"),
            Constant::StatusOk => Self::runtime_constant("Status_Ok"),
            Constant::StatusDiscarded => Self::runtime_constant("Status_Discarded"),
            Constant::StatusImpossibleLength => Self::runtime_constant("Status_Impossible_Length"),
            Constant::StatusOutOfRange => Self::runtime_constant("Status_Out_Of_Range"),
            Constant::StatusTruncated => Self::runtime_constant("Status_Truncated"),
        }
    }

    /// `Long_Integer` regardless of `bits`: see the module
    /// documentation and `runtime/ada-83`'s bitmask/bitset arithmetic,
    /// which does the narrowing this seam would otherwise ask for.
    fn unsigned_holder(_bits: u16) -> Self::Type {
        AdaType::bare("Long_Integer")
    }

    /// `runtime/ada-83` carries the three HDDS axes: `Begin_Packed_Element`
    /// and `Finish_Packed_Element` on the writer and
    /// `Reader_Begin_Packed_Element` / `Reader_Finish_Packed_Element` on the
    /// reader, the DHEADER pair for a sequence element's own length, and
    /// `Write_Wstring_Utf8_Octets` / `Read_Wstring_Utf8_Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        let mark = match crate::adapter::ada83::ada83_value_path(ast, id) {
            Ok(path) => path
                .segments()
                .iter()
                .map(Ident::as_str)
                .collect::<Vec<&str>>()
                .join("."),
            Err(_) => "Zd83_No_Ada_Form".to_string(),
        };
        let resolved = resolve_alias(ast, id);
        let kind = ast.type_(resolved).map(|node| node.kind.clone());
        AdaType {
            mark,
            is_bool: matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Boolean))),
            is_union: matches!(kind, Ok(TypeKind::Union { .. })),
            own_array: match kind {
                Ok(TypeKind::Sequence { .. } | TypeKind::Map { .. }) => Some(format!(
                    "{}_Array",
                    crate::adapter::ada83::ada83_type_name(ast, resolved)
                )),
                _ => None,
            },
            // A declared IDL type is a wire value, never a count.
            counts: false,
        }
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        crate::adapter::ada83::type_tag(ast, id)
    }

    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        ada83_map_entry_name(ast, id)
    }

    /// Nothing: this column's declared types are already flat.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    fn escape(name: &str) -> String {
        component_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", component_name("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        plain(ada83::Expr::name(local_name(text)))
    }

    fn int(value: i128) -> Self::Expr {
        plain(ada83::Expr::int(value))
    }

    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return plain(ada83::Expr::Literal(value));
        };
        match text.as_str() {
            "true" => plain(ada83::Expr::Literal(Literal::Bool(true))),
            "false" => plain(ada83::Expr::Literal(Literal::Bool(false))),
            _ => plain(ada83::Expr::Literal(value)),
        }
    }

    /// The bare number, folded onto `Long_Integer`'s own range where
    /// the walk asks for a 64-bit value the top half of which has no
    /// literal here. See `pattern_literal`.
    fn wide_int(value: i128) -> Self::Expr {
        plain(pattern_literal(value))
    }

    fn unsigned_int(value: i128) -> Self::Expr {
        plain(pattern_literal(value))
    }

    fn wide_unsigned(value: u64) -> Self::Expr {
        plain(pattern_literal(i128::from(value)))
    }

    fn wide_signed(value: i128) -> Self::Expr {
        plain(pattern_literal(value))
    }

    fn member(base: &str, field: &str) -> Self::Expr {
        if field == "_d" {
            return AExpr::Discriminant(Self::selected(
                ada83::Expr::name(local_name(base)),
                crate::adapter::ada83::DISCRIMINANT,
            ));
        }
        plain(Self::selected(ada83::Expr::name(local_name(base)), field))
    }

    fn whole(base: &str) -> Self::Expr {
        plain(ada83::Expr::name(local_name(base)))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        plain(Self::selected(inner(value), field))
    }

    /// A header's field. `end` is `End_Flag`: RM83 2.9 reserves the
    /// word exactly as RM 2.9 does.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        let name = if field == "end" {
            "End_Flag".to_string()
        } else {
            ada83::ada_case(field)
        };
        plain(Self::selected(inner(value), &name))
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        plain(ada83::Expr::Indexed {
            value: Box::new(inner(value)),
            index: Box::new(inner(at)),
        })
    }

    /// `Long_Integer (v.all'Length)`, or the bound where the
    /// materialiser fixed one.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => plain(ada83::Expr::int(i128::from(bound))),
            None => plain(ada83::Expr::Convert {
                mark: mark_path("Long_Integer"),
                value: Box::new(ada83::Expr::Attribute {
                    value: Box::new(ada83::Expr::Deref(Box::new(ada83::Expr::name(local_name(
                        base,
                    ))))),
                    name: Ident::new("Length"),
                    args: Vec::new(),
                }),
            }),
        }
    }

    fn elements(base: &str) -> Self::Expr {
        plain(ada83::Expr::name(local_name(base)))
    }

    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        plain(Self::selected(
            inner(entry),
            if key {
                "Key"
            } else {
                crate::adapter::ada83::VALUE
            },
        ))
    }

    fn absent() -> Self::Expr {
        plain(ada83::Expr::Literal(Literal::Null))
    }

    /// Nothing: a `Text` allocation either succeeds or raises
    /// `Storage_Error`; there is no "produced nothing" state to test —
    /// the identical answer `emit::ada` gives for `Unbounded_String`.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(Self::selected(
            inner(Self::member(base, field)),
            crate::adapter::ada83::VALUE,
        ))
    }

    fn present(base: &str, field: &str) -> Self::Expr {
        plain(Self::selected(
            inner(Self::member(base, field)),
            crate::adapter::ada83::PRESENT,
        ))
    }

    /// The adapter's own answer: `crate::adapter::ada83::held_by_reference`.
    fn holds_by_reference(
        ast: &Ast,
        aggregate: Id<TypeNode>,
        member_type: Id<TypeNode>,
        optional: bool,
        external: bool,
    ) -> bool {
        crate::adapter::ada83::held_by_reference(ast, aggregate, member_type, optional, external)
    }

    /// `v.X /= null`.
    fn reference_present(base: &str, field: &str) -> Self::Expr {
        Self::binary(
            "!=",
            Self::member(base, field),
            plain(ada83::Expr::Literal(Literal::Null)),
        )
    }

    /// `v.X.all`.
    fn reference_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(ada83::Expr::Deref(Box::new(inner(Self::member(
            base, field,
        )))))
    }

    /// The field, marked for [`Ada83::read_call`]'s allocator, which
    /// names the designated subtype.
    fn reference_destination(
        ast: &Ast,
        id: Id<TypeNode>,
        base: &str,
        field: &str,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        let designated = crate::adapter::ada83::reference_designated(ast, id).unwrap_or(id);
        (
            Vec::new(),
            AExpr::ReferenceSlot {
                place: inner(Self::member(base, field)),
                mark: Path::single(&crate::adapter::ada83::ada83_type_name(ast, designated)),
            },
        )
    }

    /// `v.X := null;`
    fn reference_absent(base: &str, field: &str) -> Vec<Self::Stmt> {
        vec![ada83::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: ada83::Expr::Literal(Literal::Null),
        }]
    }

    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![ada83::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: ada83::Expr::Aggregate(vec![ada83::Association {
                choice: Ident::new(crate::adapter::ada83::PRESENT),
                value: ada83::Expr::Literal(Literal::Bool(false)),
            }]),
        }]
    }

    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            AExpr::OptionalSlot(inner(Self::member(base, field))),
        )
    }

    /// As given, save two rewrites: `=` becomes a marked assignment,
    /// and every operator that works on bits rather than on a number
    /// becomes a call on `runtime/ada-83` (see `bitwise_binary`).
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        if op == "=" {
            return AExpr::Assignment {
                place: Box::new(lhs),
                value: inner(rhs),
            };
        }
        if (op == "==" || op == "!=")
            && (matches!(lhs, AExpr::RuntimeEnum(_)) || matches!(rhs, AExpr::RuntimeEnum(_)))
        {
            // The operator named rather than written infix. RM83 8.3
            // makes `=` on a type directly visible only where the
            // package declaring the type is used, and Ada 83 has no
            // `use type` to make the operator visible on its own; a
            // plain `use Zd_Wire83` is worse, because it would also
            // put `Text`, `Status` and every other runtime name into a
            // scope where an IDL is free to declare its own. `/=` is
            // reached as the negation of `=` rather than as
            // `Zd_Wire83."/="`, which is the same shape the Ada 83
            // harness already uses for the status it checks itself.
            let call =
                Self::call_path(Path::new(&[RUNTIME, "\"=\""]), vec![inner(lhs), inner(rhs)]);
            if op == "!=" {
                return plain(ada83::Expr::Unary {
                    op: "not",
                    value: Box::new(call),
                });
            }
            return plain(call);
        }
        if let Some(name) = bitwise_binary(op) {
            let rhs = if op == ">>" || op == "<<" {
                Self::to_natural(inner(rhs))
            } else {
                inner(rhs)
            };
            return plain(Self::call_path(
                Self::runtime_path(name),
                vec![inner(lhs), rhs],
            ));
        }
        plain(ada83::Expr::Binary {
            op,
            lhs: Box::new(inner(lhs)),
            rhs: Box::new(inner(rhs)),
        })
    }

    /// `not` for the Boolean the walk means by `!`, and
    /// `Zd_Wire83.Bits_Not` for the sixty-four bits it means by `~`.
    ///
    /// RM83 4.5.1 spells both with the same word and declares it only
    /// for `Boolean` and for an array of `Boolean`, so the second one
    /// has to be a call. It is the mask side of the bitmask decoder's
    /// "no bit outside the declared set" test, which GNAT reported as
    /// "invalid operand types for operator And, left operand has type
    /// Standard.Long_Integer, right operand has a modular type" — the
    /// modular type being the one GNAT resolved `not <literal>` into,
    /// there being no other in scope.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        if op == "~" {
            return plain(Self::call_path(
                Self::runtime_path("Bits_Not"),
                vec![inner(value)],
            ));
        }
        plain(ada83::Expr::Unary {
            op,
            value: Box::new(inner(value)),
        })
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        value
    }

    /// `Mark (v)`, with the two rewrites this backend still needs — a
    /// `Boolean` target turns an integer label into the literal, and
    /// the walk's `signed char` seam goes through `As_Signed`. Unlike
    /// `emit::ada::cast`, there is no enum arm: this column names no
    /// enumeration type for a conversion to cross into.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_.mark == SIGNED_CHAR {
            return plain(Self::call_path(
                Self::runtime_path("As_Signed"),
                vec![inner(value)],
            ));
        }
        if type_.is_bool {
            if let AExpr::Plain(ada83::Expr::Literal(Literal::Int(text))) = &value {
                return plain(ada83::Expr::Literal(Literal::Bool(text != "0")));
            }
        }
        plain(ada83::Expr::Convert {
            mark: mark_path(&type_.mark),
            value: Box::new(inner(value)),
        })
    }

    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let arguments: Vec<ada83::Expr> = args.into_iter().map(inner).collect();
        if callee == "strcmp" {
            return plain(Self::call_path(Self::runtime_path("Strcmp"), arguments));
        }
        plain(Self::call_path(Path::single(callee), arguments))
    }

    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let mut arguments: Vec<ada83::Expr> = if RECEIVERLESS.contains(&method) {
            Vec::new()
        } else {
            vec![ada83::Expr::name(receiver)]
        };
        arguments.extend(args.into_iter().map(inner));
        if let Some(call) = Self::narrow_integer(method, &arguments) {
            return plain(call);
        }
        plain(Self::call_path(
            Self::runtime_path(&Self::operation(method)),
            arguments,
        ))
    }

    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        plain(Self::call_path(
            Self::runtime_path(&Self::operation(method)),
            args.into_iter().map(inner).collect(),
        ))
    }

    /// `Zd_Wire83.U64_Less` at the eight-octet width, and the
    /// predefined `<` below it.
    ///
    /// This column holds every integer in `Long_Integer`, which is
    /// signed: an `unsigned long long` at or above 2**63 is the
    /// negative value carrying that bit pattern — the same carriage
    /// `Write_U64` gives it — and RM83 has no modular type whose `<`
    /// would read it back unsigned. `emit::ada` needs no override
    /// because Ada 95 gave it `Interfaces.Unsigned_64`; here the order
    /// is a runtime function, in the same place `Bits_And` and the
    /// other pattern operations RM83 has no operator for already are.
    ///
    /// Measured before the override existed: `map-key-ulonglong`
    /// against Fast DDS and OpenDDS, all four encodings, wrote
    /// `deadbeefdeadc2be` ahead of `0000000000000007`, where both peers
    /// write `7` first.
    ///
    /// The narrower widths keep the predefined `<`. An `unsigned long`
    /// tops out at 4294967295 and a `wchar` at 16#FFFF#, both far below
    /// `Long_Integer'Last`, so they are held zero-extended and
    /// non-negative and the signed order *is* the unsigned order —
    /// eight octets is the one width where the holder runs out.
    fn unsigned_less(octets: u8, left: Self::Expr, right: Self::Expr) -> Self::Expr {
        if octets < 8 {
            return Self::binary("<", left, right);
        }
        Self::runtime_static("u64_less", vec![left, right])
    }

    /// The Cyclone union overlay's binary64 pattern: `Bit_Pattern_F64`
    /// answers a Hi/Lo pair `bit_pattern`'s single-`Expr` return cannot
    /// carry, so this backend routes through
    /// `Bit_Pattern_F64_Overlay` instead — a single `Long_Integer`
    /// holding the pattern's own bits, negative where bit 63 is set.
    /// See the module documentation.
    fn bit_pattern(source: super::BitPatternSource, value: Self::Expr) -> Self::Expr {
        match source {
            super::BitPatternSource::Int { octets } => Self::runtime_static(
                "bit_pattern_int",
                vec![value, Self::int(i128::from(octets))],
            ),
            super::BitPatternSource::Bool => Self::runtime_static("bit_pattern_bool", vec![value]),
            super::BitPatternSource::Float32 => {
                Self::runtime_static("bit_pattern_f32", vec![value])
            }
            super::BitPatternSource::Float64 => {
                Self::runtime_static("bit_pattern_f64_overlay", vec![value])
            }
        }
    }

    // -- statements ----------------------------------------------------

    /// A local, under the same two repairs `emit::ada::declare` makes:
    /// the walk's `zd_wire` holder is folded onto the runtime package
    /// name (RM83 2.3 folds case exactly as RM 2.3 does), and a
    /// literal-initialised counter is held in `Natural` because it is
    /// compared against loop parameters — Ada types those `Integer`.
    ///
    /// Only a *count* takes that second repair, which is what
    /// [`AdaType::counts`] says. The walk declares its bitset
    /// accumulator the same shape — a 64-bit holder initialised to `0`
    /// — and `Natural` is thirty-one bits, so moving that one too made
    /// `Zd_Wire83.Bitfield`'s `Long_Integer` result the wrong type to
    /// assign back.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        let counts = type_.counts
            && type_.mark == "Long_Integer"
            && matches!(
                &value,
                Some(AExpr::Plain(ada83::Expr::Literal(Literal::Int(_))))
            );
        ada83::Stmt::Var(ada83::VarDecl {
            name: Ident::new(local_name(name)),
            mark: mark_path(if counts { "Natural" } else { &type_.mark }),
            constraint: None,
            value: value.map(inner),
        })
    }

    fn perform(value: Self::Expr) -> Self::Stmt {
        match value {
            AExpr::Assignment { place, value } => match *place {
                AExpr::OptionalSlot(field) => ada83::Stmt::Assign {
                    place: field,
                    value: Self::engaged(value),
                },
                AExpr::Discriminant(field) => Self::assign_discriminant(field, value),
                other => ada83::Stmt::Assign {
                    place: inner(other),
                    value,
                },
            },
            other => ada83::Stmt::Call(inner(other)),
        }
    }

    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        ada83::Stmt::Return(Some(match value {
            Some(value) => inner(value),
            None => ada83::Expr::name("v"),
        }))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        ada83::Stmt::If {
            condition: inner(condition),
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        ada83::Stmt::For {
            counter: Ident::new(local_name(counter)),
            low: ada83::Expr::int(0),
            high: ada83::Expr::Binary {
                op: "-",
                lhs: Box::new(Self::to_natural(inner(bound))),
                rhs: Box::new(ada83::Expr::int(1)),
            },
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        ada83::Stmt::While {
            condition: inner(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        ada83::Stmt::Exit
    }

    fn note(doc: Doc) -> Self::Stmt {
        ada83::Stmt::Comment(doc)
    }

    /// Ada has no `(void)`: the one value the walk drops — a
    /// collection's own DHEADER — goes through the runtime's
    /// `Discard`.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        ada83::Stmt::Call(Self::call_path(
            Self::runtime_path("Discard"),
            vec![inner(value)],
        ))
    }

    /// Nothing: GNAT's default warning set does not flag an unread
    /// parameter or local under `-gnat83` any more than it does under
    /// `-gnat2012`.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let decoded = Self::call_path(
            Path::single(callee),
            vec![ada83::Expr::name("r"), inner(behaviour)],
        );
        match place {
            AExpr::OptionalSlot(field) => ada83::Stmt::Assign {
                place: field,
                value: Self::engaged(decoded),
            },
            AExpr::Discriminant(field) => Self::assign_discriminant(field, decoded),
            // `v.X := new T'(zd_dec_T (r, zd_tc));` — the allocation is
            // what makes a member held by reference present.
            AExpr::ReferenceSlot { place, mark } => ada83::Stmt::Assign {
                place,
                value: ada83::Expr::New {
                    mark,
                    init: ada83::AllocInit::Qualified(Box::new(decoded)),
                },
            },
            other => ada83::Stmt::Assign {
                place: inner(other),
                value: decoded,
            },
        }
    }

    fn read_forward(callee: &str) -> Self::Stmt {
        ada83::Stmt::Assign {
            place: ada83::Expr::name("v"),
            value: Self::call_path(
                Path::single(callee),
                vec![ada83::Expr::name("r"), ada83::Expr::name("zd_tc")],
            ),
        }
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    fn make_room(
        base: &str,
        _element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![ada83::Stmt::Assign {
            place: ada83::Expr::name(local_name(base)),
            value: ada83::Expr::New {
                mark: Path::single(OWN_ARRAY),
                init: ada83::AllocInit::Range {
                    low: Box::new(ada83::Expr::int(0)),
                    high: Box::new(ada83::Expr::Binary {
                        op: "-",
                        lhs: Box::new(Self::to_natural(inner(count))),
                        rhs: Box::new(ada83::Expr::int(1)),
                    }),
                },
            },
        }];
        out.extend(fill);
        out
    }

    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![ada83::Stmt::Assign {
            place: ada83::Expr::name(local_name(base)),
            value: ada83::Expr::New {
                mark: Path::single(OWN_ARRAY),
                init: ada83::AllocInit::Qualified(Box::new(ada83::Expr::Slice {
                    value: Box::new(ada83::Expr::Deref(Box::new(ada83::Expr::name(local_name(
                        base,
                    ))))),
                    low: Box::new(ada83::Expr::int(0)),
                    high: Box::new(ada83::Expr::Binary {
                        op: "-",
                        lhs: Box::new(Self::to_natural(inner(count))),
                        rhs: Box::new(ada83::Expr::int(1)),
                    }),
                })),
            },
        }]
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

    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        // By access, and therefore `in`. A decoder answers a value and
        // moves the reader on, which in Ada 2012 is an `in out`
        // parameter on a function and in Ada 83 is not: RM83 6.1 admits
        // only `in` parameters there, and GNAT answers "functions can
        // only have in parameters" for every one of them. An access
        // parameter is `in` — the designated reader changes, the
        // parameter does not — so the same shape stands without a
        // language feature Ada 83 lacks. `runtime/ada-83` declares the
        // two access types beside the records.
        let mut params = vec![ada83::Param {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            mode: ada83::Mode::In,
            type_: Self::runtime_path(if spec.writing {
                "Writer_Access"
            } else {
                "Reader_Access"
            }),
        }];
        if spec.writing {
            params.push(ada83::Param {
                name: Ident::new("v"),
                mode: ada83::Mode::In,
                type_: mark_path(&spec.sample.mark),
            });
        }
        for (type_, name) in &spec.extra {
            params.push(ada83::Param {
                name: Ident::new(local_name(name)),
                mode: ada83::Mode::In,
                type_: mark_path(&type_.mark),
            });
        }
        let profile = ada83::Subprogram {
            doc: spec.doc.clone(),
            kind: if spec.writing {
                ada83::SubprogramKind::Procedure
            } else {
                ada83::SubprogramKind::Function
            },
            name: Ident::new(&spec.name),
            params,
            result: if spec.writing {
                None
            } else {
                Some(mark_path(&spec.sample.mark))
            },
        };
        let Some(body) = spec.body else {
            return Some(ada83::Decl::Subprogram(profile));
        };

        if spec.writing {
            return Some(ada83::Decl::SubprogramBody(ada83::SubprogramBody {
                spec: profile,
                body,
            }));
        }

        let mut statements = Vec::new();
        if spec.sample.is_union && names_mark(&body, OWN_SAMPLE) {
            statements.push(ada83::Stmt::Subtype {
                name: Ident::new(OWN_SAMPLE),
                mark: mark_path(&spec.sample.mark),
            });
        }
        if let Some(array) = &spec.sample.own_array {
            if names_mark(&body, OWN_ARRAY) {
                statements.push(ada83::Stmt::Subtype {
                    name: Ident::new(OWN_ARRAY),
                    mark: Path::single(array),
                });
            }
        }
        let defaulter = crate::default_ada83::defaulter_from_decoder(&spec.name);
        statements.push(ada83::Stmt::Var(ada83::VarDecl {
            name: Ident::new("v"),
            mark: mark_path(&spec.sample.mark),
            constraint: None,
            value: Some(Self::call_path(Path::single(&defaulter), Vec::new())),
        }));
        let mut body = body;
        statements.append(&mut body);
        statements.push(ada83::Stmt::Return(Some(ada83::Expr::name("v"))));
        Some(ada83::Decl::SubprogramBody(ada83::SubprogramBody {
            spec: profile,
            body: statements,
        }))
    }

    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        let defaulted = Self::call_path(
            Path::single(&crate::default_ada83::defaulter_name(ast, id)),
            Vec::new(),
        );
        Ok(match place {
            AExpr::OptionalSlot(field) => ada83::Stmt::Assign {
                place: field,
                value: Self::engaged(defaulted),
            },
            AExpr::Discriminant(field) => Self::assign_discriminant(field, defaulted),
            other => ada83::Stmt::Assign {
                place: inner(other),
                value: defaulted,
            },
        })
    }

    /// # Errors
    /// As [`crate::default_ada83::member_default`].
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_ada83::member_default(ast, node, type_, inner(place))
    }

    /// The named constant itself: this column declares no enumeration
    /// type to resolve a literal against, so the constant's name is
    /// the whole answer.
    fn enumerator_constant(_ast: &Ast, _id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        plain(ada83::Expr::name(&component_name(enumerator)))
    }
}

/// A local's Ada spelling, folding the walk's `zd_wire` holder onto the
/// runtime package `Zd_Wire83` — the identical repair
/// `emit::ada::local_name` makes, RM83 2.3(5/2) folding case exactly as
/// RM 2.3(5/2) does.
fn local_name(text: &str) -> &str {
    if text.eq_ignore_ascii_case("zd_wire") {
        return "zd_onwire";
    }
    text
}

/// Whether any expression in `body` names `mark` as a subtype.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn names_mark(body: &[ada83::Stmt], mark: &str) -> bool {
    body.iter().any(|statement| stmt_names(statement, mark))
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn stmt_names(statement: &ada83::Stmt, mark: &str) -> bool {
    match statement {
        ada83::Stmt::Null | ada83::Stmt::Exit | ada83::Stmt::Comment(_) => false,
        ada83::Stmt::Var(var) => {
            path_is(&var.mark, mark)
                || var.constraint.as_ref().is_some_and(|e| expr_names(e, mark))
                || var.value.as_ref().is_some_and(|e| expr_names(e, mark))
        }
        ada83::Stmt::Subtype { mark: target, .. } => path_is(target, mark),
        ada83::Stmt::Assign { place, value } => expr_names(place, mark) || expr_names(value, mark),
        ada83::Stmt::Call(value) => expr_names(value, mark),
        ada83::Stmt::Return(value) => value.as_ref().is_some_and(|e| expr_names(e, mark)),
        ada83::Stmt::If {
            condition,
            then,
            otherwise,
        } => {
            expr_names(condition, mark)
                || names_mark(then, mark)
                || otherwise
                    .as_ref()
                    .is_some_and(|body| names_mark(body, mark))
        }
        ada83::Stmt::For {
            low, high, body, ..
        } => expr_names(low, mark) || expr_names(high, mark) || names_mark(body, mark),
        ada83::Stmt::While { condition, body } => {
            expr_names(condition, mark) || names_mark(body, mark)
        }
        ada83::Stmt::Declare { vars, body } => {
            vars.iter().any(|var| {
                path_is(&var.mark, mark)
                    || var.constraint.as_ref().is_some_and(|e| expr_names(e, mark))
                    || var.value.as_ref().is_some_and(|e| expr_names(e, mark))
            }) || names_mark(body, mark)
        }
    }
}

fn path_is(path: &Path, mark: &str) -> bool {
    path.segments().len() == 1
        && path
            .segments()
            .first()
            .is_some_and(|segment| segment.as_str() == mark)
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn expr_names(expr: &ada83::Expr, mark: &str) -> bool {
    match expr {
        ada83::Expr::Literal(_) => false,
        ada83::Expr::Name(path) => path_is(path, mark),
        ada83::Expr::Call { name, args } => {
            path_is(name, mark) || args.iter().any(|arg| expr_names(arg, mark))
        }
        ada83::Expr::Negate(value)
        | ada83::Expr::Unary { value, .. }
        | ada83::Expr::Deref(value) => expr_names(value, mark),
        ada83::Expr::Binary { lhs, rhs, .. } => expr_names(lhs, mark) || expr_names(rhs, mark),
        ada83::Expr::Selected { value, .. } => expr_names(value, mark),
        ada83::Expr::Indexed { value, index } => expr_names(value, mark) || expr_names(index, mark),
        ada83::Expr::Slice { value, low, high } => {
            expr_names(value, mark) || expr_names(low, mark) || expr_names(high, mark)
        }
        ada83::Expr::Attribute { value, args, .. } => {
            expr_names(value, mark) || args.iter().any(|arg| expr_names(arg, mark))
        }
        ada83::Expr::Convert {
            mark: target,
            value,
        } => path_is(target, mark) || expr_names(value, mark),
        ada83::Expr::Aggregate(parts) => parts.iter().any(|part| expr_names(&part.value, mark)),
        ada83::Expr::New { mark: target, init } => {
            path_is(target, mark)
                || match init {
                    ada83::AllocInit::Range { low, high } => {
                        expr_names(low, mark) || expr_names(high, mark)
                    }
                    ada83::AllocInit::Qualified(value) => expr_names(value, mark),
                }
        }
    }
}
