// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Ada backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, as `runtime/ada` implements it —
//! the same division as every other column: the walk decides which
//! member goes out under which header, this file decides how that is
//! written in Ada, and `runtime/ada` decides what the octets are.
//!
//! # The five decisions this column turns on
//!
//! **A discriminant is not assignable.** Ada 2012 RM 3.7.1: the
//! discriminant of a record changes only when the whole record is
//! assigned. The walk decodes a union's discriminator straight into
//! `v._d`, so [`Ada::read_call`] routes that one place through a
//! `declare` block — a constrained object of the decoder's own sample
//! subtype (`Zd_Sample`, declared by [`Ada::routine`]), assigned whole.
//! Every branch after it is written under the chain's own `v.D =`
//! guard, where the discriminant check passes.
//!
//! **An `@optional` member is a discriminated record too.** The
//! materialiser's `Zd_Opt_…` carries `Present` and, in its `True` arm,
//! `Value`; both change together or not at all, so a decoded value is
//! assigned as the aggregate `(Present => True, Value => …)` and
//! absence as `(Present => False)`.
//!
//! **A sequence is an access to an unconstrained array**, and the array
//! type's name is not derivable from the element's, so
//! [`Ada::make_room`] and [`Ada::keep_first`] allocate through the
//! fixed name `Zd_Own_Array` — a subtype [`Ada::routine`] declares in
//! any decoder whose sample is a collection, naming the materialiser's
//! `…_Array` type.
//!
//! **An enumeration converts through its representation.** Ada's type
//! conversion does not cross the enum/integer line; `Mark'Enum_Val`
//! turns a wire value into the literal and `Mark'Enum_Rep` the literal
//! into its code (RM 13.4 in Ada 2022; GNAT implements both under
//! `-gnat2012`). [`Ada::cast`] spells the first wherever the target
//! type is an enumeration, and [`Ada::routine`] rewrites the walk's
//! `Int64` cast of the sample into the second inside an enumeration's
//! own encoder.
//!
//! **Every numeric literal is universal.** Ada's integer literals are
//! `universal_integer` and convert to any integer type in context, so
//! [`Ada::wide_int`] and its siblings write the bare number; the width
//! suffixes the C family needs have no Ada counterpart and no gap to
//! cover.
//!
//! # What is refused rather than bent
//!
//! A union arm of an enumeration type under the Cyclone negative-label
//! overlay would need the arm's representation from a seam that only
//! carries the expression; the generated `Interfaces.Integer_64 (…)`
//! conversion is then illegal Ada and GNAT refuses the probe loudly —
//! a compile error, never wrong octets. Recursive `@optional` and
//! `@external` members (the materialiser's `Zd_Ref_…` access types)
//! take the same road: the codec spells the inline accessor, and GNAT
//! names the missing component.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::ada::{ada_holder, ada_map_entry_name, ada_type_name, ada_value_path};
use crate::default_common::resolve_alias;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::ada;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};

/// The Ada backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ada;

/// The package the generated code names the runtime under.
const RUNTIME: &str = "Zd_Wire";

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`; the Ada member is an `Interfaces.Unsigned_8` and the signed
/// reading goes through the runtime's `As_Signed`, because an Ada
/// conversion checks the range where C reinterprets the bits.
const SIGNED_CHAR: &str = "signed char";

/// The subtype [`Ada::routine`] declares for a decoder's own sample
/// type, so the one place that has to name it — the discriminant
/// assignment — can.
const OWN_SAMPLE: &str = "Zd_Sample";

/// The subtype [`Ada::routine`] declares for the array behind a
/// collection decoder's own access type, so the allocators can name it.
const OWN_ARRAY: &str = "Zd_Own_Array";

/// A type, as this column holds it: the subtype mark plus the four
/// properties the seams cannot re-derive without the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaType {
    /// The subtype mark, dotted.
    pub mark: String,
    /// Whether the type resolves to an enumeration — conversions go
    /// through `'Enum_Val` / `'Enum_Rep`.
    pub is_enum: bool,
    /// Whether it is `Boolean` — an integer-literal cast becomes the
    /// Boolean literal.
    pub is_bool: bool,
    /// Whether it resolves to a union — the decoder needs `Zd_Sample`.
    pub is_union: bool,
    /// The `…_Array` type behind it, where it is a sequence or a map —
    /// the decoder needs `Zd_Own_Array`.
    pub own_array: Option<String>,
}

impl AdaType {
    fn bare(mark: &str) -> Self {
        Self {
            mark: mark.to_string(),
            is_enum: false,
            is_bool: false,
            is_union: false,
            own_array: None,
        }
    }
}

/// An expression, with the two properties the walk's seams cannot
/// carry: whether the place it names is an `@optional` member's wrapper
/// record, or a union's discriminant.
#[derive(Debug, Clone)]
pub enum AExpr {
    /// An ordinary expression.
    Plain(ada::Expr),
    /// The `Zd_Opt_…` field of an `@optional` member, as a place a
    /// decoded value is about to be written into.
    OptionalSlot(ada::Expr),
    /// `v.D`, as a place — assignable only through the whole record.
    Discriminant(ada::Expr),
    /// The access-typed field of a member held by reference
    /// ([`Ada::holds_by_reference`]), as a place a decoded value is
    /// about to be allocated for; `mark` is the designated subtype the
    /// allocator names.
    ReferenceSlot {
        /// The field.
        place: ada::Expr,
        /// `T` in `new T'(…)`.
        mark: Path,
    },
    /// An assignment the walk built through `binary("=", …)`, kept
    /// apart so [`Ada::perform`] can see which place it writes.
    Assignment {
        /// The place.
        place: Box<AExpr>,
        /// The value.
        value: ada::Expr,
    },
}

/// The expression inside, whichever form it took.
///
/// zerodds-lint: recursion-depth 64 (an assignment's place, which is
/// itself an `AExpr`). The value walked is owned and moved in, so it
/// holds no cycle; the walk builds no `AExpr::Assignment` inside
/// another, so the real depth is one.
fn inner(expr: AExpr) -> ada::Expr {
    match expr {
        AExpr::Plain(e) | AExpr::OptionalSlot(e) | AExpr::Discriminant(e) => e,
        AExpr::ReferenceSlot { place, .. } => place,
        // Never a rendered position; rebuilt as the walk spelled it so a
        // misuse is visible in the output rather than lost.
        AExpr::Assignment { place, value } => ada::Expr::Binary {
            op: "=",
            lhs: Box::new(inner(*place)),
            rhs: Box::new(value),
        },
    }
}

fn plain(e: ada::Expr) -> AExpr {
    AExpr::Plain(e)
}

/// A dotted mark as the tree's path.
fn mark_path(mark: &str) -> Path {
    Path::new(&mark.split('.').collect::<Vec<&str>>())
}

impl Ada {
    /// A runtime name, qualified: `Zd_Wire.X`.
    fn runtime_path(name: &str) -> Path {
        Path::new(&[RUNTIME, name])
    }

    fn runtime_constant(name: &str) -> AExpr {
        plain(ada::Expr::Name(Self::runtime_path(name)))
    }

    /// A runtime operation's Ada spelling: `write_u32` is `Write_U32`.
    fn operation(method: &str) -> String {
        ada::ada_case(method)
    }

    fn selected(value: ada::Expr, name: &str) -> ada::Expr {
        ada::Expr::Selected {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    fn call_path(path: Path, args: Vec<ada::Expr>) -> ada::Expr {
        ada::Expr::Call { name: path, args }
    }

    /// `Natural (value)` — the conversion every count and every shift
    /// distance crosses on its way into a `Natural` position.
    fn to_natural(value: ada::Expr) -> ada::Expr {
        ada::Expr::Convert {
            mark: Path::single("Natural"),
            value: Box::new(value),
        }
    }

    /// `(Present => True, Value => decoded)` — how a value enters an
    /// `@optional` member's wrapper record: both halves in one
    /// assignment, because the discriminated record admits no other
    /// (RM 3.7.1).
    fn engaged(value: ada::Expr) -> ada::Expr {
        ada::Expr::Aggregate(vec![
            ada::Association {
                choice: Ident::new(crate::adapter::ada::PRESENT),
                value: ada::Expr::Literal(Literal::Bool(true)),
            },
            ada::Association {
                choice: Ident::new(crate::adapter::ada::VALUE),
                value,
            },
        ])
    }

    /// The record a discriminant place belongs to: `v.D`'s prefix.
    fn discriminant_record(place: ada::Expr) -> ada::Expr {
        match place {
            ada::Expr::Selected { value, .. } => *value,
            other => other,
        }
    }

    /// The whole-record assignment a discriminant change has to be
    /// (RM 3.7.1): a constrained object of the decoder's own sample
    /// subtype, built from the decoded value, assigned whole.
    fn assign_discriminant(place: ada::Expr, value: ada::Expr) -> ada::Stmt {
        ada::Stmt::Declare {
            vars: vec![ada::VarDecl {
                name: Ident::new("zd_nd"),
                mark: Path::single(OWN_SAMPLE),
                constraint: Some(value),
                value: None,
            }],
            body: vec![ada::Stmt::Assign {
                place: Self::discriminant_record(place),
                value: ada::Expr::name("zd_nd"),
            }],
        }
    }
}

impl CodecTarget for Ada {
    type Expr = AExpr;
    type Stmt = ada::Stmt;
    type Block = Vec<ada::Stmt>;
    type Type = AdaType;
    type Decl = ada::Decl;

    // -- types --------------------------------------------------------

    /// The fixed-width Ada type each scalar local takes. `Index` and
    /// `Size` are both `Natural`: the reader's positions and counts
    /// answer in it, and an Ada `for` counter is one anyway.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => AdaType {
                is_bool: true,
                ..AdaType::bare("Boolean")
            },
            Scalar::Char | Scalar::UInt8 => AdaType::bare("Interfaces.Unsigned_8"),
            Scalar::Int8 => AdaType::bare("Interfaces.Integer_8"),
            Scalar::Int16 => AdaType::bare("Interfaces.Integer_16"),
            Scalar::UInt16 => AdaType::bare("Interfaces.Unsigned_16"),
            Scalar::Int32 => AdaType::bare("Interfaces.Integer_32"),
            Scalar::UInt32 => AdaType::bare("Interfaces.Unsigned_32"),
            Scalar::Int64 => AdaType::bare("Interfaces.Integer_64"),
            Scalar::UInt64 => AdaType::bare("Interfaces.Unsigned_64"),
            Scalar::Index | Scalar::Size => AdaType::bare("Natural"),
            Scalar::Float => AdaType::bare("Interfaces.IEEE_Float_32"),
            Scalar::Double => AdaType::bare("Interfaces.IEEE_Float_64"),
            // The decimal text; the adapter's table says why.
            Scalar::LongDouble => AdaType::bare("Ada.Strings.Unbounded.Unbounded_String"),
        }
    }

    fn named(name: &str) -> Self::Type {
        AdaType::bare(name)
    }

    /// Never rendered: every value-returning position is typed, and the
    /// two `(void)` idioms are overridden below. The name is one no
    /// scope declares, so a use that slips through fails in GNAT.
    fn nothing() -> Self::Type {
        AdaType::bare("Zd_No_Type")
    }

    fn handle(kind: Handle) -> Self::Type {
        AdaType::bare(match kind {
            Handle::DHeader => "Zd_Wire.Dheader",
            Handle::EmHeader => "Zd_Wire.Emheader",
            Handle::Pid => "Zd_Wire.Pid_Write",
            Handle::TryConstruct => "Zd_Wire.Try_Construct",
            Handle::ReaderMark => "Zd_Wire.Reader_Mark",
            Handle::PidRead => "Zd_Wire.Pid_Read",
            Handle::EmHeaderRead => "Zd_Wire.Emheader_Read",
        })
    }

    /// The member is the code unit — `Unsigned_8` for a `char`,
    /// `Unsigned_16` for a `wchar` — and the runtime takes the code
    /// point as `Unsigned_32`, so the conversion widens and never
    /// narrows.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        plain(ada::Expr::Convert {
            mark: mark_path("Interfaces.Unsigned_32"),
            value: Box::new(inner(value)),
        })
    }

    /// The runtime's policy values: integers where `runtime/ada` takes
    /// a width, and its own enumeration literals for everything else.
    /// One arm per constant and no catch-all.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => plain(ada::Expr::int(2)),
            Constant::WcharWidth4 => plain(ada::Expr::int(4)),
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

    fn unsigned_holder(bits: u16) -> Self::Type {
        AdaType::bare(ada_holder(bits).as_str())
    }

    /// `runtime/ada` carries the three HDDS axes: `Begin_Packed_Element`
    /// and `Finish_Packed_Element` on the writer and `Reader_Begin_Packed_Element`
    /// / `Reader_Finish_Packed_Element` on the reader, the DHEADER pair for a
    /// sequence element's own length, and `Write_Wstring_Utf8_Octets` /
    /// `Read_Wstring_Utf8_Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        let mark = match ada_value_path(ast, id) {
            Ok(path) => path
                .segments()
                .iter()
                .map(Ident::as_str)
                .collect::<Vec<&str>>()
                .join("."),
            // A type with no Ada form was refused by the materialiser
            // before a codec walk ran; the name makes the impossible
            // fail in GNAT, loudly.
            Err(_) => "Zd_No_Ada_Form".to_string(),
        };
        let resolved = resolve_alias(ast, id);
        let kind = ast.type_(resolved).map(|node| node.kind.clone());
        AdaType {
            mark,
            is_enum: matches!(kind, Ok(TypeKind::Enumeration { .. })),
            is_bool: matches!(kind, Ok(TypeKind::Primitive(PrimitiveKind::Boolean))),
            is_union: matches!(kind, Ok(TypeKind::Union { .. })),
            own_array: match kind {
                Ok(TypeKind::Sequence { .. } | TypeKind::Map { .. }) => {
                    Some(format!("{}_Array", ada_type_name(ast, resolved)))
                }
                _ => None,
            },
        }
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        crate::adapter::ada::type_tag(ast, id)
    }

    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        ada_map_entry_name(ast, id)
    }

    /// Nothing: a declared type's Ada name is already flat, and an
    /// enumeration literal resolves by its expected type (RM 3.5.1).
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// The materialiser's own spelling of a member, exactly.
    fn escape(name: &str) -> String {
        crate::adapter::ada::component_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", crate::adapter::ada::component_name("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        plain(ada::Expr::name(local_name(text)))
    }

    fn int(value: i128) -> Self::Expr {
        plain(ada::Expr::int(value))
    }

    /// The raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten into Ada's boolean literals where they are one.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return plain(ada::Expr::Literal(value));
        };
        match text.as_str() {
            "true" => plain(ada::Expr::Literal(Literal::Bool(true))),
            "false" => plain(ada::Expr::Literal(Literal::Bool(false))),
            _ => plain(ada::Expr::Literal(value)),
        }
    }

    /// The bare number: an Ada integer literal is `universal_integer`
    /// (RM 2.4) and takes any integer type from context, so no width
    /// suffix exists to write.
    fn wide_int(value: i128) -> Self::Expr {
        Self::int(value)
    }

    fn unsigned_int(value: i128) -> Self::Expr {
        Self::int(value)
    }

    fn wide_unsigned(value: u64) -> Self::Expr {
        Self::int(i128::from(value))
    }

    fn wide_signed(value: i128) -> Self::Expr {
        Self::int(value)
    }

    /// `v.X` — and `v._d` as the discriminant `D`, marked, because the
    /// place it names is assignable only through the whole record.
    fn member(base: &str, field: &str) -> Self::Expr {
        if field == "_d" {
            return AExpr::Discriminant(Self::selected(
                ada::Expr::name(local_name(base)),
                crate::adapter::ada::DISCRIMINANT,
            ));
        }
        plain(Self::selected(ada::Expr::name(local_name(base)), field))
    }

    fn whole(base: &str) -> Self::Expr {
        plain(ada::Expr::name(local_name(base)))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        plain(Self::selected(inner(value), field))
    }

    /// A header's field, in the spelling `runtime/ada` publishes.
    /// `end` is `End_Flag`: RM 2.9 reserves the word and a component
    /// cannot carry it.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        let name = if field == "end" {
            "End_Flag".to_string()
        } else {
            ada::ada_case(field)
        };
        plain(Self::selected(inner(value), &name))
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        plain(ada::Expr::Indexed {
            value: Box::new(inner(value)),
            index: Box::new(inner(at)),
        })
    }

    /// `Interfaces.Unsigned_32 (v.all'Length)`, or the bound where the
    /// materialiser fixed one: a sequence is an access to an
    /// unconstrained array and its count is the array's own.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => plain(ada::Expr::int(i128::from(bound))),
            None => plain(ada::Expr::Convert {
                mark: mark_path("Interfaces.Unsigned_32"),
                value: Box::new(ada::Expr::Attribute {
                    value: Box::new(ada::Expr::Deref(Box::new(ada::Expr::name(local_name(
                        base,
                    ))))),
                    name: Ident::new("Length"),
                    args: Vec::new(),
                }),
            }),
        }
    }

    /// The access value itself: indexing dereferences implicitly
    /// (RM 4.1).
    fn elements(base: &str) -> Self::Expr {
        plain(ada::Expr::name(local_name(base)))
    }

    /// `v.X` — the variant part's components sit directly on the
    /// record.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `entry.Key` and `entry.Value`, the two components the
    /// materialiser gives a map's entry record.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        plain(Self::selected(
            inner(entry),
            if key {
                "Key"
            } else {
                crate::adapter::ada::VALUE
            },
        ))
    }

    fn absent() -> Self::Expr {
        plain(ada::Expr::Literal(Literal::Null))
    }

    /// Nothing: an `Unbounded_String` owns its storage and there is no
    /// allocation failure for the guard to test.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// `v.X.Value` — the `True` arm's component. Only ever reached
    /// inside a branch the presence test guarded, where the
    /// discriminant check cannot fire.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(Self::selected(
            inner(Self::member(base, field)),
            crate::adapter::ada::VALUE,
        ))
    }

    /// The adapter's own answer: `crate::adapter::ada::held_by_reference`.
    fn holds_by_reference(
        ast: &Ast,
        aggregate: Id<TypeNode>,
        member_type: Id<TypeNode>,
        optional: bool,
        external: bool,
    ) -> bool {
        crate::adapter::ada::held_by_reference(ast, aggregate, member_type, optional, external)
    }

    /// `v.X /= null`.
    fn reference_present(base: &str, field: &str) -> Self::Expr {
        Self::binary(
            "!=",
            Self::member(base, field),
            plain(ada::Expr::Literal(Literal::Null)),
        )
    }

    /// `v.X.all`.
    fn reference_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(ada::Expr::Deref(Box::new(inner(Self::member(base, field)))))
    }

    /// The field, marked for [`Ada::read_call`]'s allocator, which names
    /// the designated subtype.
    fn reference_destination(
        ast: &Ast,
        id: Id<TypeNode>,
        base: &str,
        field: &str,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        let designated = crate::adapter::ada::reference_designated(ast, id).unwrap_or(id);
        (
            Vec::new(),
            AExpr::ReferenceSlot {
                place: inner(Self::member(base, field)),
                mark: Path::single(&ada_type_name(ast, designated)),
            },
        )
    }

    /// `v.X := null;`
    fn reference_absent(base: &str, field: &str) -> Vec<Self::Stmt> {
        vec![ada::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: ada::Expr::Literal(Literal::Null),
        }]
    }

    /// `v.X.Present` — reading a discriminant is always legal.
    fn present(base: &str, field: &str) -> Self::Expr {
        plain(Self::selected(
            inner(Self::member(base, field)),
            crate::adapter::ada::PRESENT,
        ))
    }

    /// Nothing on the way in — the aggregate assignment in
    /// [`Ada::read_call`] is what makes the member present — and
    /// `(Present => False)` on the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![ada::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: ada::Expr::Aggregate(vec![ada::Association {
                choice: Ident::new(crate::adapter::ada::PRESENT),
                value: ada::Expr::Literal(Literal::Bool(false)),
            }]),
        }]
    }

    /// The marked place; the room is the aggregate [`Ada::read_call`]
    /// assigns.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            AExpr::OptionalSlot(inner(Self::member(base, field))),
        )
    }

    /// The operators as given; the renderer maps the spellings and
    /// parenthesises. `=` is kept as a marked assignment so
    /// [`Ada::perform`] can dispatch on the place.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        if op == "=" {
            return AExpr::Assignment {
                place: Box::new(lhs),
                value: inner(rhs),
            };
        }
        // Ada's modular types have no shift operators; `Interfaces`
        // publishes the intrinsics (RM B.2), and the distance is a
        // `Natural`.
        if op == ">>" || op == "<<" {
            return plain(Self::call_path(
                Path::new(&[
                    "Interfaces",
                    if op == ">>" {
                        "Shift_Right"
                    } else {
                        "Shift_Left"
                    },
                ]),
                vec![inner(lhs), Self::to_natural(inner(rhs))],
            ));
        }
        plain(ada::Expr::Binary {
            op,
            lhs: Box::new(inner(lhs)),
            rhs: Box::new(inner(rhs)),
        })
    }

    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        plain(ada::Expr::Unary {
            op,
            value: Box::new(inner(value)),
        })
    }

    /// Identity: the renderer parenthesises every compound operand, so
    /// a bracket asked for is already there.
    fn paren(value: Self::Expr) -> Self::Expr {
        value
    }

    /// `Mark (v)` — with three rewrites. A `Boolean` target turns an
    /// integer label into the literal; the walk's `signed char` — the C
    /// spelling of a map key's signed order — goes through the
    /// runtime's `As_Signed`, because an Ada conversion checks the
    /// range C reinterprets; and an enumeration target converts through
    /// `'Enum_Val`, because Ada's conversion does not cross the
    /// enum/integer line (RM 4.6).
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_.mark == SIGNED_CHAR {
            return plain(Self::call_path(
                Self::runtime_path("As_Signed"),
                vec![inner(value)],
            ));
        }
        if type_.is_bool {
            if let AExpr::Plain(ada::Expr::Literal(Literal::Int(text))) = &value {
                return plain(ada::Expr::Literal(Literal::Bool(text != "0")));
            }
        }
        if type_.is_enum {
            return plain(ada::Expr::Attribute {
                value: Box::new(ada::Expr::Name(mark_path(&type_.mark))),
                name: Ident::new("Enum_Val"),
                args: vec![inner(value)],
            });
        }
        plain(ada::Expr::Convert {
            mark: mark_path(&type_.mark),
            value: Box::new(inner(value)),
        })
    }

    /// The value, unchanged: Ada passes an `in` parameter as the value
    /// it is, arrays included.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// A call to a generated codec or defaulter. `strcmp` is the
    /// runtime's `Strcmp`, which spells C11 §7.24.4's unsigned-octet
    /// order over the UTF-8 the members hold.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let arguments: Vec<ada::Expr> = args.into_iter().map(inner).collect();
        if callee == "strcmp" {
            return plain(Self::call_path(Self::runtime_path("Strcmp"), arguments));
        }
        plain(Self::call_path(Path::single(callee), arguments))
    }

    /// `Zd_Wire.Write_U32 (w, x)` — the runtime is a package and the
    /// receiver is its first parameter, `in out`.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let mut arguments = vec![ada::Expr::name(receiver)];
        arguments.extend(args.into_iter().map(inner));
        plain(Self::call_path(
            Self::runtime_path(&Self::operation(method)),
            arguments,
        ))
    }

    /// A free function of the runtime, qualified.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        plain(Self::call_path(
            Self::runtime_path(&Self::operation(method)),
            args.into_iter().map(inner).collect(),
        ))
    }

    // -- statements ----------------------------------------------------

    /// A local, under two Ada repairs. The walk's `zd_wire` holder
    /// would hide the runtime package — RM 2.3(5/2) folds case, so
    /// `zd_wire` and `Zd_Wire` are one identifier and RM 8.3 lets the
    /// inner declaration win — and a counter the walk initialises with
    /// a literal is held in `Natural`, because it is compared against
    /// loop parameters, which Ada types `Integer` and refuses to
    /// compare against a modular.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        let counts = type_.mark == "Interfaces.Unsigned_32"
            && matches!(
                &value,
                Some(AExpr::Plain(ada::Expr::Literal(Literal::Int(_))))
            );
        ada::Stmt::Var(ada::VarDecl {
            name: Ident::new(local_name(name)),
            mark: mark_path(if counts { "Natural" } else { &type_.mark }),
            constraint: None,
            value: value.map(inner),
        })
    }

    /// An assignment where the walk built one — routed by the place: an
    /// `@optional` wrapper takes the engaged aggregate and a
    /// discriminant the whole-record form — and a call statement
    /// otherwise.
    fn perform(value: Self::Expr) -> Self::Stmt {
        match value {
            AExpr::Assignment { place, value } => match *place {
                AExpr::OptionalSlot(field) => ada::Stmt::Assign {
                    place: field,
                    value: Self::engaged(value),
                },
                AExpr::Discriminant(field) => Self::assign_discriminant(field, value),
                other => ada::Stmt::Assign {
                    place: inner(other),
                    value,
                },
            },
            other => ada::Stmt::Call(inner(other)),
        }
    }

    /// `return v;`: an Ada decoder answers with the sample it built.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        ada::Stmt::Return(Some(match value {
            Some(value) => inner(value),
            None => ada::Expr::name("v"),
        }))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        ada::Stmt::If {
            condition: inner(condition),
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// `for zd_i in 0 .. Natural (bound) - 1 loop`: the loop parameter
    /// is an integer, which is what an array index wants, and the bound
    /// arrives as whatever unsigned the wire count is.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        ada::Stmt::For {
            counter: Ident::new(local_name(counter)),
            low: ada::Expr::int(0),
            high: ada::Expr::Binary {
                op: "-",
                lhs: Box::new(Self::to_natural(inner(bound))),
                rhs: Box::new(ada::Expr::int(1)),
            },
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        ada::Stmt::While {
            condition: inner(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        ada::Stmt::Exit
    }

    fn note(doc: Doc) -> Self::Stmt {
        ada::Stmt::Comment(doc)
    }

    /// Ada has no `(void)`: a function's answer cannot be left on the
    /// floor, so the one value the walk drops — a collection's own
    /// DHEADER — goes through the runtime's `Discard_U32`.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        ada::Stmt::Call(Self::call_path(
            Self::runtime_path("Discard_U32"),
            vec![inner(value)],
        ))
    }

    /// Nothing: GNAT's default warning set does not flag an unread
    /// parameter or local, and the probe is compiled without `-gnatwa`
    /// on purpose — `docs` in `runtime/ada` carries the reason.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// `place := zd_dec_T (r, behaviour);` — and through the engaged
    /// aggregate or the whole-record form where the place is an
    /// `@optional` member or the discriminant.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let decoded = Self::call_path(
            Path::single(callee),
            vec![ada::Expr::name("r"), inner(behaviour)],
        );
        match place {
            AExpr::OptionalSlot(field) => ada::Stmt::Assign {
                place: field,
                value: Self::engaged(decoded),
            },
            AExpr::Discriminant(field) => Self::assign_discriminant(field, decoded),
            // `v.X := new T'(zd_dec_T (r, zd_tc));` — the allocation is
            // what makes a member held by reference present.
            AExpr::ReferenceSlot { place, mark } => ada::Stmt::Assign {
                place,
                value: ada::Expr::New {
                    mark,
                    init: ada::AllocInit::Qualified(Box::new(decoded)),
                },
            },
            other => ada::Stmt::Assign {
                place: inner(other),
                value: decoded,
            },
        }
    }

    /// `v := zd_dec_T (r, zd_tc);` — a `typedef`, handed straight on:
    /// the decoders of this column answer their value.
    fn read_forward(callee: &str) -> Self::Stmt {
        ada::Stmt::Assign {
            place: ada::Expr::name("v"),
            value: Self::call_path(
                Path::single(callee),
                vec![ada::Expr::name("r"), ada::Expr::name("zd_tc")],
            ),
        }
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// `v := new Zd_Own_Array (0 .. Natural (n) - 1);`, then the fill:
    /// the allocator's elements are default-valued storage. The array
    /// type's name is not derivable from the element's, which is what
    /// the subtype [`Ada::routine`] declares is for.
    fn make_room(
        base: &str,
        _element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![ada::Stmt::Assign {
            place: ada::Expr::name(local_name(base)),
            value: ada::Expr::New {
                mark: Path::single(OWN_ARRAY),
                init: ada::AllocInit::Range {
                    low: Box::new(ada::Expr::int(0)),
                    high: Box::new(ada::Expr::Binary {
                        op: "-",
                        lhs: Box::new(Self::to_natural(inner(count))),
                        rhs: Box::new(ada::Expr::int(1)),
                    }),
                },
            },
        }];
        out.extend(fill);
        out
    }

    /// `v := new Zd_Own_Array'(v.all (0 .. Natural (count) - 1));` —
    /// §7.3.1.2.1.12.3's TRIM: an Ada array cannot shrink in place, so
    /// the kept prefix is a fresh allocation.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![ada::Stmt::Assign {
            place: ada::Expr::name(local_name(base)),
            value: ada::Expr::New {
                mark: Path::single(OWN_ARRAY),
                init: ada::AllocInit::Qualified(Box::new(ada::Expr::Slice {
                    value: Box::new(ada::Expr::Deref(Box::new(ada::Expr::name(local_name(
                        base,
                    ))))),
                    low: Box::new(ada::Expr::int(0)),
                    high: Box::new(ada::Expr::Binary {
                        op: "-",
                        lhs: Box::new(Self::to_natural(inner(count))),
                        rhs: Box::new(ada::Expr::int(1)),
                    }),
                })),
            },
        }]
    }

    /// The subscript, with nothing around it: an Ada array element is a
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

    /// `procedure zd_enc_T (w : in out Zd_Wire.Writer; v : in T)` and
    /// `function zd_dec_T (r : in out Zd_Wire.Reader;
    /// zd_tc : in Zd_Wire.Try_Construct) return T`.
    ///
    /// A forward declaration is answered with the profile: Ada resolves
    /// nothing forward, and a mutually recursive pair needs both specs
    /// ahead of both bodies (RM 3.11).
    ///
    /// The declarative part is also where this column's two named
    /// subtypes land — `Zd_Sample` for the discriminant assignment and
    /// `Zd_Own_Array` for the collection allocators — and where the
    /// walk's `Interfaces.Integer_64 (v)` inside an enumeration's own
    /// encoder is rewritten through `Mark'Enum_Rep`, the one conversion
    /// Ada will not write directly.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let mut params = vec![ada::Param {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            mode: ada::Mode::InOut,
            type_: Self::runtime_path(if spec.writing { "Writer" } else { "Reader" }),
        }];
        if spec.writing {
            params.push(ada::Param {
                name: Ident::new("v"),
                mode: ada::Mode::In,
                type_: mark_path(&spec.sample.mark),
            });
        }
        for (type_, name) in &spec.extra {
            params.push(ada::Param {
                name: Ident::new(local_name(name)),
                mode: ada::Mode::In,
                type_: mark_path(&type_.mark),
            });
        }
        let profile = ada::Subprogram {
            doc: spec.doc.clone(),
            kind: if spec.writing {
                ada::SubprogramKind::Procedure
            } else {
                ada::SubprogramKind::Function
            },
            name: Ident::new(&spec.name),
            params,
            result: if spec.writing {
                None
            } else {
                Some(mark_path(&spec.sample.mark))
            },
            is_abstract: false,
        };
        let Some(mut body) = spec.body else {
            return Some(ada::Decl::Subprogram(profile));
        };

        if spec.writing {
            if spec.sample.is_enum {
                rewrite_enum_holder(&mut body, &spec.sample.mark);
            }
            return Some(ada::Decl::SubprogramBody(ada::SubprogramBody {
                spec: profile,
                body,
            }));
        }

        // The sample starts as Table 9's value, through the generated
        // defaulter, so every position the stream does not carry is
        // already what the clause says it is.
        let mut statements = Vec::new();
        if spec.sample.is_union && names_mark(&body, OWN_SAMPLE) {
            statements.push(ada::Stmt::Subtype {
                name: Ident::new(OWN_SAMPLE),
                mark: mark_path(&spec.sample.mark),
            });
        }
        if let Some(array) = &spec.sample.own_array {
            if names_mark(&body, OWN_ARRAY) {
                statements.push(ada::Stmt::Subtype {
                    name: Ident::new(OWN_ARRAY),
                    mark: Path::single(array),
                });
            }
        }
        let defaulter = crate::default_ada::defaulter_from_decoder(&spec.name);
        statements.push(ada::Stmt::Var(ada::VarDecl {
            name: Ident::new("v"),
            mark: mark_path(&spec.sample.mark),
            constraint: None,
            value: Some(Self::call_path(Path::single(&defaulter), Vec::new())),
        }));
        statements.append(&mut body);
        statements.push(ada::Stmt::Return(Some(ada::Expr::name("v"))));
        Some(ada::Decl::SubprogramBody(ada::SubprogramBody {
            spec: profile,
            body: statements,
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
        let defaulted = Self::call_path(
            Path::single(&crate::default_ada::defaulter_name(ast, id)),
            Vec::new(),
        );
        Ok(match place {
            AExpr::OptionalSlot(field) => ada::Stmt::Assign {
                place: field,
                value: Self::engaged(defaulted),
            },
            AExpr::Discriminant(field) => Self::assign_discriminant(field, defaulted),
            other => ada::Stmt::Assign {
                place: inner(other),
                value: defaulted,
            },
        })
    }

    /// # Errors
    /// As [`crate::default_ada::member_default`]: a `@default` this
    /// column cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_ada::member_default(ast, node, type_, inner(place))
    }

    /// The literal itself: an enumeration literal resolves by the type
    /// the context expects (RM 3.5.1), and the generated declarations
    /// share the probe's one declarative region.
    fn enumerator_constant(_ast: &Ast, _id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        plain(ada::Expr::name(&crate::adapter::ada::component_name(
            enumerator,
        )))
    }
}

/// A local's Ada spelling. One name moves: the walk's `zd_wire` holder
/// folds onto the runtime package `Zd_Wire` (RM 2.3(5/2)), and the
/// hidden package would take every runtime call in the enumeration
/// encoder down with it.
///
/// Every place the backend spells an identifier the *walk* chose goes
/// through here — the declaration, a mention, a loop parameter, a
/// subprogram's extra parameter, the base of a member, a subscript or
/// an allocator. One site left out is a declaration and a use that
/// disagree, which is not a compile error in a language whose package
/// name is in scope: the use resolves to the package instead.
fn local_name(text: &str) -> &str {
    if text.eq_ignore_ascii_case("zd_wire") {
        return "zd_onwire";
    }
    text
}

/// Whether any expression in `body` names `mark` as a subtype — the
/// scan that decides whether [`Ada::routine`] declares it.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn names_mark(body: &[ada::Stmt], mark: &str) -> bool {
    body.iter().any(|statement| stmt_names(statement, mark))
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn stmt_names(statement: &ada::Stmt, mark: &str) -> bool {
    match statement {
        ada::Stmt::Null | ada::Stmt::Exit | ada::Stmt::Comment(_) => false,
        ada::Stmt::Var(var) => {
            path_is(&var.mark, mark)
                || var.constraint.as_ref().is_some_and(|e| expr_names(e, mark))
                || var.value.as_ref().is_some_and(|e| expr_names(e, mark))
        }
        ada::Stmt::Subtype { mark: target, .. } => path_is(target, mark),
        ada::Stmt::Assign { place, value } => expr_names(place, mark) || expr_names(value, mark),
        ada::Stmt::Call(value) => expr_names(value, mark),
        ada::Stmt::Return(value) => value.as_ref().is_some_and(|e| expr_names(e, mark)),
        ada::Stmt::If {
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
        ada::Stmt::For {
            low, high, body, ..
        } => expr_names(low, mark) || expr_names(high, mark) || names_mark(body, mark),
        ada::Stmt::While { condition, body } => {
            expr_names(condition, mark) || names_mark(body, mark)
        }
        ada::Stmt::Declare { vars, body } => {
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
fn expr_names(expr: &ada::Expr, mark: &str) -> bool {
    match expr {
        ada::Expr::Literal(_) | ada::Expr::BoxDefault => false,
        ada::Expr::Name(path) => path_is(path, mark),
        ada::Expr::Call { name, args } => {
            path_is(name, mark) || args.iter().any(|arg| expr_names(arg, mark))
        }
        ada::Expr::Negate(value) | ada::Expr::Unary { value, .. } | ada::Expr::Deref(value) => {
            expr_names(value, mark)
        }
        ada::Expr::Binary { lhs, rhs, .. } => expr_names(lhs, mark) || expr_names(rhs, mark),
        ada::Expr::Selected { value, .. } => expr_names(value, mark),
        ada::Expr::Indexed { value, index } => expr_names(value, mark) || expr_names(index, mark),
        ada::Expr::Slice { value, low, high } => {
            expr_names(value, mark) || expr_names(low, mark) || expr_names(high, mark)
        }
        ada::Expr::Attribute { value, args, .. } => {
            expr_names(value, mark) || args.iter().any(|arg| expr_names(arg, mark))
        }
        ada::Expr::Convert {
            mark: target,
            value,
        } => path_is(target, mark) || expr_names(value, mark),
        ada::Expr::Aggregate(parts) => parts.iter().any(|part| expr_names(&part.value, mark)),
        ada::Expr::New { mark: target, init } => {
            path_is(target, mark)
                || match init {
                    ada::AllocInit::Range { low, high } => {
                        expr_names(low, mark) || expr_names(high, mark)
                    }
                    ada::AllocInit::Qualified(value) => expr_names(value, mark),
                }
        }
    }
}

/// Rewrite `Interfaces.Integer_64 (v)` into
/// `Interfaces.Integer_64 (Mark'Enum_Rep (v))` throughout an
/// enumeration's own encoder body: the walk reads the sample through
/// the target's `Int64` cast, and Ada's conversion does not take an
/// enumeration operand (RM 4.6). `'Enum_Rep` answers the code the
/// representation clause declared.
///
/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn rewrite_enum_holder(body: &mut [ada::Stmt], mark: &str) {
    for statement in body.iter_mut() {
        rewrite_stmt(statement, mark);
    }
}

/// zerodds-lint: recursion-depth 64 (statement nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that body's height, and a defaulter
/// body is a handful of statements deep.
fn rewrite_stmt(statement: &mut ada::Stmt, mark: &str) {
    match statement {
        ada::Stmt::Null | ada::Stmt::Exit | ada::Stmt::Comment(_) | ada::Stmt::Subtype { .. } => {}
        ada::Stmt::Var(var) => {
            if let Some(value) = &mut var.value {
                rewrite_expr(value, mark);
            }
        }
        ada::Stmt::Assign { place, value } => {
            rewrite_expr(place, mark);
            rewrite_expr(value, mark);
        }
        ada::Stmt::Call(value) => rewrite_expr(value, mark),
        ada::Stmt::Return(value) => {
            if let Some(value) = value {
                rewrite_expr(value, mark);
            }
        }
        ada::Stmt::If {
            condition,
            then,
            otherwise,
        } => {
            rewrite_expr(condition, mark);
            rewrite_enum_holder(then, mark);
            if let Some(body) = otherwise {
                rewrite_enum_holder(body, mark);
            }
        }
        ada::Stmt::For {
            low, high, body, ..
        } => {
            rewrite_expr(low, mark);
            rewrite_expr(high, mark);
            rewrite_enum_holder(body, mark);
        }
        ada::Stmt::While { condition, body } => {
            rewrite_expr(condition, mark);
            rewrite_enum_holder(body, mark);
        }
        ada::Stmt::Declare { vars, body } => {
            for var in vars {
                if let Some(value) = &mut var.value {
                    rewrite_expr(value, mark);
                }
            }
            rewrite_enum_holder(body, mark);
        }
    }
}

/// zerodds-lint: recursion-depth 64 (expression nesting the walk builds).
/// The tree walked here is the one this generator emitted and owns, so
/// it holds no cycle; the depth is that expression's height, which
/// follows the member nesting the type walk already bounded.
fn rewrite_expr(expr: &mut ada::Expr, mark: &str) {
    if let ada::Expr::Convert {
        mark: target,
        value,
    } = expr
    {
        let is_holder = target
            .segments()
            .last()
            .is_some_and(|segment| segment.as_str() == "Integer_64");
        let is_sample = matches!(value.as_ref(), ada::Expr::Name(path)
            if path.segments().len() == 1
                && path.segments().first().is_some_and(|s| s.as_str() == "v"));
        if is_holder && is_sample {
            let sample = core::mem::replace(value.as_mut(), ada::Expr::BoxDefault);
            **value = ada::Expr::Attribute {
                value: Box::new(ada::Expr::Name(mark_path(mark))),
                name: Ident::new("Enum_Rep"),
                args: vec![sample],
            };
            return;
        }
    }
    match expr {
        ada::Expr::Literal(_) | ada::Expr::Name(_) | ada::Expr::BoxDefault => {}
        ada::Expr::Call { args, .. } => {
            for arg in args {
                rewrite_expr(arg, mark);
            }
        }
        ada::Expr::Negate(value) | ada::Expr::Unary { value, .. } | ada::Expr::Deref(value) => {
            rewrite_expr(value, mark);
        }
        ada::Expr::Binary { lhs, rhs, .. } => {
            rewrite_expr(lhs, mark);
            rewrite_expr(rhs, mark);
        }
        ada::Expr::Selected { value, .. } => rewrite_expr(value, mark),
        ada::Expr::Indexed { value, index } => {
            rewrite_expr(value, mark);
            rewrite_expr(index, mark);
        }
        ada::Expr::Slice { value, low, high } => {
            rewrite_expr(value, mark);
            rewrite_expr(low, mark);
            rewrite_expr(high, mark);
        }
        ada::Expr::Attribute { value, args, .. } => {
            rewrite_expr(value, mark);
            for arg in args {
                rewrite_expr(arg, mark);
            }
        }
        ada::Expr::Convert { value, .. } => rewrite_expr(value, mark),
        ada::Expr::Aggregate(parts) => {
            for part in parts {
                rewrite_expr(&mut part.value, mark);
            }
        }
        ada::Expr::New { init, .. } => match init {
            ada::AllocInit::Range { low, high } => {
                rewrite_expr(low, mark);
                rewrite_expr(high, mark);
            }
            ada::AllocInit::Qualified(value) => rewrite_expr(value, mark),
        },
    }
}
